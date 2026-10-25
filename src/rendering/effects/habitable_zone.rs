//! Circumstellar Habitable Zone (HZ) and Volatile Frost Line rendering.
//!
//! Evaluates the host star's effective bolometric luminosity and dynamically computes:
//! - Conservative Habitable Zone (Runaway Greenhouse to Maximum Greenhouse: 0.95 - 1.67 sqrt(L) AU)
//! - Optimistic Habitable Zone (Recent Venus to Early Mars: 0.75 - 1.77 sqrt(L) AU)
//! - Water Ice Snow Line / Frost Line (~170 K volatile condensation boundary: 2.70 sqrt(L) AU)
//! - Volatile Sublimation / Soot Line (0.20 sqrt(L) AU)

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use bevy::prelude::*;

use crate::simulation::components::{
    CentralStar, Luminosity, Mass, Radius, SimPosition, Temperature,
};
use crate::simulation::resources::{
    DiagnosticOverlayMode, HabitableZoneConfig, OrbitVisualizationMode, PlayerInteractionState,
};
use crate::utils::constants::SOLAR_RADIUS_AU;

pub type StarHabitableZoneQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SimPosition,
        &'static Mass,
        Option<&'static Luminosity>,
        Option<&'static Temperature>,
        Option<&'static Radius>,
    ),
    With<CentralStar>,
>;

/// Calculates the stellar bolometric luminosity relative to the Sun ($L / L_\odot$).
pub fn compute_effective_luminosity(
    star_mass_solar: f64,
    opt_lum: Option<&Luminosity>,
    opt_temp: Option<&Temperature>,
    opt_rad: Option<&Radius>,
) -> f32 {
    if let Some(lum) = opt_lum {
        return (lum.0 as f32).max(1e-5);
    }

    if let (Some(temp), Some(rad)) = (opt_temp, opt_rad) {
        let t_ratio = (temp.0 / 5778.0) as f32;
        let r_solar = (rad.0 / SOLAR_RADIUS_AU) as f32;
        return (r_solar * r_solar * t_ratio.powi(4)).clamp(1e-5, 1e7);
    }

    // Standard main-sequence mass-luminosity fallback: L ~ M^3.5
    (star_mass_solar as f32)
        .max(0.01)
        .powf(3.5)
        .clamp(1e-5, 1e7)
}

/// Renders the circumstellar habitable zone and volatile snow line guides around the host star.
pub fn draw_circumstellar_habitable_zone_and_frost_lines(
    gizmos: &mut Gizmos,
    star_pos: Vec3,
    star_mass_solar: f64,
    opt_lum: Option<&Luminosity>,
    opt_temp: Option<&Temperature>,
    opt_rad: Option<&Radius>,
    hz_config: &HabitableZoneConfig,
    overlay_mode: DiagnosticOverlayMode,
) {
    if !hz_config.is_enabled || overlay_mode == DiagnosticOverlayMode::Hidden {
        return;
    }

    let lum = compute_effective_luminosity(star_mass_solar, opt_lum, opt_temp, opt_rad);
    let scale = lum.sqrt();

    let ring_rot = Quat::from_rotation_x(FRAC_PI_2);

    // 1. Conservative Habitable Zone Band (0.95 to 1.67 sqrt(L) AU)
    draw_conservative_habitable_zone(gizmos, star_pos, ring_rot, scale);

    // 2. Optimistic Habitable Zone Shelves (0.75-0.95 and 1.67-1.77 sqrt(L) AU)
    draw_optimistic_habitable_shelves(gizmos, star_pos, ring_rot, scale);

    // 3. Water Ice Snow Line / Frost Line (2.70 sqrt(L) AU)
    draw_water_ice_snow_line(gizmos, star_pos, ring_rot, scale);

    // 4. Inner Scorching / Volatile Sublimation Line (0.20 sqrt(L) AU)
    let r_soot = 0.20 * scale;
    gizmos.circle(
        Isometry3d::new(star_pos, ring_rot),
        r_soot,
        Color::srgba(1.00, 0.40, 0.15, 0.30),
    );
}

fn draw_conservative_habitable_zone(
    gizmos: &mut Gizmos,
    star_pos: Vec3,
    ring_rot: Quat,
    scale: f32,
) {
    // Stepped concentric rings to form a lush semi-transparent green annular band
    let steps = [
        (0.95, Color::srgba(0.20, 0.90, 0.45, 0.40)),
        (1.07, Color::srgba(0.22, 0.94, 0.50, 0.42)),
        (1.20, Color::srgba(0.25, 0.98, 0.55, 0.48)),
        (1.35, Color::srgba(0.20, 0.92, 0.52, 0.45)),
        (1.50, Color::srgba(0.15, 0.86, 0.58, 0.40)),
        (1.67, Color::srgba(0.10, 0.80, 0.62, 0.38)),
    ];

    for (fraction, color) in steps {
        let r = fraction * scale;
        gizmos.circle(Isometry3d::new(star_pos, ring_rot), r, color);
    }
}

fn draw_optimistic_habitable_shelves(
    gizmos: &mut Gizmos,
    star_pos: Vec3,
    ring_rot: Quat,
    scale: f32,
) {
    // Recent Venus inner optimistic boundary (0.75 - 0.95)
    let r_opt_in = 0.75 * scale;
    gizmos.circle(
        Isometry3d::new(star_pos, ring_rot),
        r_opt_in,
        Color::srgba(0.85, 0.90, 0.25, 0.25),
    );

    // Early Mars outer optimistic boundary (1.67 - 1.77)
    let r_opt_out = 1.77 * scale;
    gizmos.circle(
        Isometry3d::new(star_pos, ring_rot),
        r_opt_out,
        Color::srgba(0.20, 0.75, 0.92, 0.25),
    );
}

fn draw_water_ice_snow_line(gizmos: &mut Gizmos, star_pos: Vec3, ring_rot: Quat, scale: f32) {
    let r_snow = 2.70 * scale;

    // Double concentric frosty cyan ring
    gizmos.circle(
        Isometry3d::new(star_pos, ring_rot),
        r_snow,
        Color::srgba(0.40, 0.85, 1.00, 0.65),
    );
    gizmos.circle(
        Isometry3d::new(star_pos, ring_rot),
        r_snow * 1.015,
        Color::srgba(0.55, 0.90, 1.00, 0.35),
    );

    // 8 Ice-crystal tick markers placed radially along the frost line
    let tick_length = (0.06 * scale).clamp(0.04, 0.8);
    for k in 0..8 {
        let angle = (k as f32) * FRAC_PI_4;
        let dir_x = angle.cos();
        let dir_z = angle.sin();

        let pt_inner = star_pos
            + Vec3::new(
                dir_x * (r_snow - tick_length),
                0.0,
                dir_z * (r_snow - tick_length),
            );
        let pt_outer = star_pos
            + Vec3::new(
                dir_x * (r_snow + tick_length),
                0.0,
                dir_z * (r_snow + tick_length),
            );

        gizmos.line(pt_inner, pt_outer, Color::srgba(0.65, 0.92, 1.00, 0.75));

        // Transverse diamond tick
        let perp_x = -dir_z;
        let perp_z = dir_x;
        let pt_center = star_pos + Vec3::new(dir_x * r_snow, 0.0, dir_z * r_snow);
        let perp_offset = Vec3::new(perp_x, 0.0, perp_z) * (tick_length * 0.4);
        gizmos.line(
            pt_center - perp_offset,
            pt_center + perp_offset,
            Color::srgba(0.65, 0.92, 1.00, 0.75),
        );
    }
}

/// Dedicated Bevy render system for circumstellar habitable zones and snow lines.
pub fn draw_habitable_zone_gizmos(
    mut gizmos: Gizmos,
    hz_config: Res<HabitableZoneConfig>,
    player_state: Res<PlayerInteractionState>,
    star_query: StarHabitableZoneQuery<'_, '_>,
) {
    if !hz_config.is_enabled
        || player_state.overlay_mode == DiagnosticOverlayMode::Hidden
        || player_state.orbit_mode == OrbitVisualizationMode::Off
    {
        return;
    }

    for (s_pos, s_mass, opt_lum, opt_temp, opt_rad) in star_query.iter() {
        let star_pos = Vec3::new(s_pos.x as f32, s_pos.y as f32, s_pos.z as f32);
        draw_circumstellar_habitable_zone_and_frost_lines(
            &mut gizmos,
            star_pos,
            s_mass.0,
            opt_lum,
            opt_temp,
            opt_rad,
            &hz_config,
            player_state.overlay_mode,
        );
    }
}
