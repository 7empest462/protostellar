use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;

/// Computes the `tidal_deformation` shader uniform (`xyz` = unit world vector toward dominant
/// tidal perturber, `w` = `roche_stretch` in `0.0..0.78` plus `+2.0` if tidally locked).
pub fn compute_tidal_deformation_uniform(
    world_pos: Vec3,
    primary_pos: Vec3,
    primary_mass_solar: f64,
    body_mass_solar: f64,
    body_radius_au: f64,
    opt_tidal: Option<&TidalState>,
    opt_roche: Option<&crate::simulation::accretion::RocheLobeOverflow>,
    is_satellite: bool,
) -> Vec4 {
    let to_primary = primary_pos - world_pos;
    let dist_au = f64::from(to_primary.length());
    let tidal_dir = to_primary.normalize_or_zero();

    // Eggleton / Fluid Roche limit estimate: r_roche ~ 2.44 * R_body * (M_primary / M_body)^(1/3)
    let mass_ratio = primary_mass_solar / body_mass_solar.max(1e-12);
    let r_roche_au = 2.44 * body_radius_au * mass_ratio.cbrt();

    let mut stretch = if let Some(roche) = opt_roche {
        let overflow_boost = (roche.mass_transfer_rate * 2.5e3).clamp(0.0, 0.35) as f32;
        (0.42 + overflow_boost).clamp(0.35, 0.78)
    } else if dist_au > 1e-5 && r_roche_au > 0.0 {
        let prox = (r_roche_au / dist_au) as f32;
        if prox > 0.33 {
            ((prox - 0.33) * 0.65).clamp(0.0, 0.72)
        } else {
            0.0
        }
    } else {
        0.0
    };

    if !is_satellite && opt_tidal.is_some_and(|t| t.is_tidally_locked) {
        stretch += 2.0;
    }

    Vec4::new(tidal_dir.x, tidal_dir.y, tidal_dir.z, stretch)
}

/// Computes the flux-weighted incoming starlight direction for a celestial body,
/// smoothly blending illumination across multi-star and circumbinary systems.
pub fn resolve_star_direction(
    entity: Entity,
    body_pos: Vec3,
    star_pos: Vec3,
    stars: &[(Entity, Vec3, f32)],
) -> Vec3 {
    if stars.is_empty() {
        Vec3::Y
    } else if stars.len() == 1 {
        (star_pos - body_pos).normalize_or_zero()
    } else {
        let mut weighted_dir = Vec3::ZERO;
        let mut nearest_dir = Vec3::Y;
        let mut nearest_dist_sq = f32::MAX;

        for &(s_ent, s_pos, lum_weight) in stars {
            if s_ent == entity {
                continue;
            }
            let to_star = s_pos - body_pos;
            let dist_sq = to_star.length_squared();
            if dist_sq < 1e-12 {
                continue;
            }
            let dir = to_star / dist_sq.sqrt();
            if dist_sq < nearest_dist_sq {
                nearest_dist_sq = dist_sq;
                nearest_dir = dir;
            }
            let flux = lum_weight.max(1e-4) / dist_sq.max(1e-6);
            weighted_dir += dir * flux;
        }

        if weighted_dir.length_squared() > 1e-12 {
            weighted_dir.normalize()
        } else {
            nearest_dir
        }
    }
}

/// Computes a zoom-invariant, physically proportioned 1:1 visual radius across all distances.
pub fn compute_realistic_visual_radius(
    physical_radius_au: f64,
    body_type: BodyType,
    _r_orb: f32,
    _min_orbit_r: f32,
    _cam_dist: f32,
    config: &SimulationConfig,
) -> f32 {
    let safe_phys_r = if physical_radius_au.is_finite() {
        physical_radius_au.max(1e-9)
    } else {
        1e-6
    };
    config.calc_visual_radius_for_type(safe_phys_r, body_type)
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
        if sx.is_finite() && sx > 1e-10 && !is_unsynced_default {
            return sx;
        }
    }
    compute_realistic_visual_radius(physical_radius_au, body_type, r_orb, 1.0, cam_dist, config)
}

/// Resolves the stable close-up visual radius (`cam_dist = 0.0`) for a First-Person Surface Mode host body.
/// Preserves smaller stripped-core scales (`show_atmospheres == false`) or custom unit-test scales.
#[inline]
pub fn resolve_surface_mode_visual_radius(
    opt_transform: Option<&Transform>,
    physical_radius_au: f64,
    body_type: BodyType,
    r_orb: f32,
    config: &SimulationConfig,
) -> f32 {
    let close_vis_r =
        compute_realistic_visual_radius(physical_radius_au, body_type, r_orb, 1.0, 0.0, config);
    if let Some(tf) = opt_transform {
        let sx = tf.scale.x;
        let is_unsynced_default = (sx - 1.0).abs() < 1e-6 && physical_radius_au < 0.25;
        if sx.is_finite() && sx > 1e-10 && !is_unsynced_default {
            return sx.min(close_vis_r);
        }
    }
    close_vis_r
}

/// Computes visual radius, proportionally shrinking gas-enveloped
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
    let precision_floor = 1e-9_f32.min(full_vis_r);
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
