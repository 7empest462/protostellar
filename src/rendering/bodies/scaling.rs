use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Computes a realistic camera-adaptive visual radius that transitions to near-physical
/// scale on close-up and never crowds neighboring orbits in compact systems.
pub fn compute_realistic_visual_radius(
    physical_radius_au: f64,
    body_type: BodyType,
    r_orb: f32,
    min_orbit_r: f32,
    cam_dist: f32,
    config: &SimulationConfig,
) -> f32 {
    let safe_phys_r = if physical_radius_au.is_finite() {
        physical_radius_au.max(1e-9)
    } else {
        1e-6
    };
    let safe_r_orb = if r_orb.is_finite() {
        r_orb.max(0.0)
    } else {
        1.0
    };
    let safe_min_orbit = if min_orbit_r.is_finite() && min_orbit_r > 0.0 {
        min_orbit_r
    } else {
        1.0
    };
    let safe_cam_dist = if cam_dist.is_finite() {
        cam_dist.max(0.0)
    } else {
        25.0
    };

    let r_true = (safe_phys_r as f32).max(1e-6);
    let r_exag =
        config.calc_visual_radius_with_orbit(safe_phys_r, body_type, safe_r_orb, safe_min_orbit);

    let orbit_spacing_cap = if safe_r_orb > 0.0005 && !body_type.is_star_or_remnant() {
        (safe_r_orb * 0.045).max(r_true * 1.25)
    } else if body_type.is_star_or_remnant() && safe_min_orbit < 0.05 {
        (safe_min_orbit * 0.32).max(r_true)
    } else {
        r_exag
    };
    let max_allowed_exag = r_exag.min(orbit_spacing_cap);

    // In single-precision 32-bit float world coordinates, machine epsilon at distance R_orb
    // is ~ R_orb * 5.96e-8 AU. Enforce a distance-dependent precision floor to eliminate faceting.
    let precision_mult = if safe_r_orb > 25.0 { 1.5e-4 } else { 3.6e-5 };
    let float_precision_floor = (safe_r_orb * precision_mult).min(max_allowed_exag);

    let close_scale_floor = if body_type == BodyType::BlackHole {
        max_allowed_exag
    } else if body_type.is_star_or_remnant() {
        (r_true * 1.2)
            .max(float_precision_floor)
            .min(max_allowed_exag)
    } else if body_type == BodyType::Moon {
        let moon_scale = ((r_true / 0.000_042_6_f32).powf(0.45) * 0.46).clamp(0.05, 1.0);
        ((r_true * 2.5).max(0.00018 * moon_scale))
            .max(float_precision_floor * moon_scale)
            .min(max_allowed_exag)
    } else if matches!(
        body_type,
        BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal | BodyType::DustGrain
    ) {
        let minor_precision_floor = (float_precision_floor * 0.35).min(max_allowed_exag);
        (r_true * 2.5)
            .max(0.00010)
            .max(minor_precision_floor)
            .min(max_allowed_exag)
    } else if safe_r_orb > 25.0 {
        (r_true * 2.5)
            .max(max_allowed_exag * 0.65)
            .max(float_precision_floor)
            .min(max_allowed_exag)
    } else {
        (r_true * 2.5)
            .max(0.00018)
            .max(float_precision_floor)
            .min(max_allowed_exag)
    };

    let close_dist = (close_scale_floor * 8.0).max(0.015);
    if safe_cam_dist <= close_dist || max_allowed_exag <= close_scale_floor {
        close_scale_floor
    } else {
        let scaled = close_scale_floor * (safe_cam_dist / close_dist).powf(0.78);
        scaled.min(max_allowed_exag)
    }
}

/// Resolves the exact rendered visual radius of a celestial body in `O(1)` by reading its synced
/// `Transform.scale.x` when valid, or computing `compute_realistic_visual_radius` if unsynced.
#[inline]
pub fn resolve_synced_visual_radius(
    opt_transform: Option<&Transform>,
    physical_radius_au: f64,
    body_type: BodyType,
    r_orb: f32,
    cam_dist: f32,
    config: &SimulationConfig,
) -> f32 {
    if let Some(tf) = opt_transform {
        let sx = tf.scale.x;
        let is_unsynced_default = (sx - 1.0).abs() < 1e-6 && physical_radius_au < 0.25;
        if sx.is_finite() && sx > 1e-6 && !is_unsynced_default {
            return sx;
        }
    }
    compute_realistic_visual_radius(physical_radius_au, body_type, r_orb, 1.0, cam_dist, config)
}

/// Computes camera-adaptive visual radius, proportionally shrinking gas-enveloped
/// bodies down to their true solid/icy core diameter when `show_atmospheres` is false.
#[allow(
    clippy::too_many_arguments,
    reason = "Visual radius helper shares orbital, differentiation, and camera context"
)]
pub fn compute_view_visual_radius(
    physical_radius_au: f64,
    body_type: BodyType,
    mass_solar: f64,
    opt_comp: Option<&Composition>,
    opt_diff: Option<&InternalDifferentiation>,
    show_atmospheres: bool,
    r_orb: f32,
    min_orbit_r: f32,
    cam_dist: f32,
    config: &SimulationConfig,
) -> f32 {
    let full_vis_r = compute_realistic_visual_radius(
        physical_radius_au,
        body_type,
        r_orb,
        min_orbit_r,
        cam_dist,
        config,
    );
    if show_atmospheres || body_type.is_star_or_remnant() {
        return full_vis_r;
    }
    let Some(comp) = opt_comp else {
        return full_vis_r;
    };
    let stripped_r = super::view_overrides::stripped_interior_radius_au(
        mass_solar,
        physical_radius_au,
        body_type,
        comp,
        opt_diff,
    );
    if !physical_radius_au.is_finite()
        || physical_radius_au <= 1e-12
        || stripped_r >= physical_radius_au * 0.999
    {
        return full_vis_r;
    }
    let ratio = (stripped_r / physical_radius_au) as f32;
    let safe_r_orb = if r_orb.is_finite() {
        r_orb.max(0.0)
    } else {
        1.0
    };
    let precision_floor = (safe_r_orb * 8.0e-6).max(1e-6).min(full_vis_r);
    (full_vis_r * ratio).max(precision_floor).min(full_vis_r)
}

/// Computes the close-up camera framing distance (AU) accounting for active view overrides
/// such as atmosphere/gas-envelope stripping.
#[allow(
    clippy::too_many_arguments,
    reason = "Close-up framing accounts for stripped core radius and orbital clearance"
)]
pub fn compute_close_view_camera_framing_radius(
    physical_radius_au: f64,
    body_type: BodyType,
    mass_solar: f64,
    opt_comp: Option<&Composition>,
    opt_diff: Option<&InternalDifferentiation>,
    show_atmospheres: bool,
    r_orb: f32,
    min_orbit_r: f32,
    config: &SimulationConfig,
) -> f32 {
    let r0 = compute_view_visual_radius(
        physical_radius_au,
        body_type,
        mass_solar,
        opt_comp,
        opt_diff,
        show_atmospheres,
        r_orb,
        min_orbit_r,
        0.002,
        config,
    );
    let d1 = config.calc_camera_framing_radius(r0);
    let r1 = compute_view_visual_radius(
        physical_radius_au,
        body_type,
        mass_solar,
        opt_comp,
        opt_diff,
        show_atmospheres,
        r_orb,
        min_orbit_r,
        d1,
        config,
    );
    config.calc_camera_framing_radius(r1)
}

/// Computes the self-consistent close-up camera framing distance (AU) for focusing on a body,
/// accounting for the camera-adaptive visual radius reduction as the camera approaches.
pub fn compute_close_camera_framing_radius(
    physical_radius_au: f64,
    body_type: BodyType,
    r_orb: f32,
    min_orbit_r: f32,
    config: &SimulationConfig,
) -> f32 {
    let r0 = compute_realistic_visual_radius(
        physical_radius_au,
        body_type,
        r_orb,
        min_orbit_r,
        0.002,
        config,
    );
    let d1 = config.calc_camera_framing_radius(r0);
    let r1 = compute_realistic_visual_radius(
        physical_radius_au,
        body_type,
        r_orb,
        min_orbit_r,
        d1,
        config,
    );
    config.calc_camera_framing_radius(r1)
}
