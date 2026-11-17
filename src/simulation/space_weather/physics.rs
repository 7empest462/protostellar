//! Analytical astrophysical formulas for magnetopause standoff, auroral oval geometry,
//! geomagnetic Kp indices, and multi-spectral ionospheric emission profiles.

use bevy::prelude::*;

use crate::simulation::components::Composition;
use crate::simulation::space_weather::types::{AuroralOvalState, GeomagneticStormLevel};

/// Computes the auroral oval central co-latitude $\theta_A$ and angular Gaussian width $\Delta \theta_A$ (in radians).
///
/// Under a dipolar planetary magnetic field $B(r, \theta) \propto \frac{\mu_0 M}{4\pi r^3}$, field lines satisfy:
/// $$r = L R_p \sin^2\theta$$
/// Mapping the subsolar magnetopause boundary $L \approx R_{\text{mp}} / R_p$ to the planetary ionosphere ($r = R_p$):
/// $$\sin\theta_A = \sqrt{\frac{R_p}{R_{\text{mp}}}}$$
///
/// - High magnetopause standoff ($R_{\text{mp}} \approx 10 - 15\,R_p$): $\theta_A \approx 15^\circ - 18^\circ$ (high latitude $72^\circ - 75^\circ$).
/// - Compressed storm conditions ($R_{\text{mp}} \approx 4 - 6\,R_p$): $\theta_A \approx 24^\circ - 30^\circ$ (mid-latitudes $60^\circ - 66^\circ$).
pub fn calculate_auroral_oval_geometry(standoff_rp: f32) -> (f32, f32) {
    let rp_ratio = (1.0 / standoff_rp.max(1.05)).clamp(0.01, 0.95);
    let sin_theta = rp_ratio.sqrt();
    // Clamp to maximum 0.45 rad (~25.8° colatitude, or 64.2° latitude).
    // Ensures auroras remain strictly polar and sub-polar even under severe solar/stellar wind compression.
    let colatitude = sin_theta.asin().clamp(0.10, 0.45);

    // Oval half-width broadens slightly during storms (from ~2.0° up to ~4.2° / 0.035 to 0.075 rad)
    let half_width = (0.040 + 0.035 * (colatitude / 0.45)).clamp(0.035, 0.075);

    (colatitude, half_width)
}

/// Maps a planetary geomagnetic Kp index ($0.0 \le \text{Kp} \le 9.0$) to its NOAA storm tier.
pub fn classify_kp_storm_level(kp: f32) -> GeomagneticStormLevel {
    if kp < 2.0 {
        GeomagneticStormLevel::Quiet
    } else if kp < 4.0 {
        GeomagneticStormLevel::Unsettled
    } else if kp < 5.0 {
        GeomagneticStormLevel::MinorStorm
    } else if kp < 6.0 {
        GeomagneticStormLevel::ModerateStorm
    } else if kp < 7.0 {
        GeomagneticStormLevel::StrongStorm
    } else if kp < 8.5 {
        GeomagneticStormLevel::SevereStorm
    } else {
        GeomagneticStormLevel::ExtremeStorm
    }
}

/// Derives the planetary geomagnetic Kp index ($0.0 \le \text{Kp} \le 9.0$) and storm tier
/// from magnetopause compression and solar wind dynamic ram pressure amplification.
pub fn calculate_geomagnetic_kp(
    standoff_rp: f32,
    cme_density_mult: f32,
    b_gauss: f32,
) -> (f32, GeomagneticStormLevel) {
    if b_gauss < 0.05 {
        return (0.0, GeomagneticStormLevel::Quiet);
    }

    // Baseline Earth magnetopause is ~10.5 R_p
    let compression = (10.5 / standoff_rp.max(2.0)).max(0.6);
    let ram_boost = cme_density_mult.max(1.0).powf(0.35);

    // Scaling relation: Kp ~ 1.35 * compression^1.4 * ram_boost
    let raw_kp = 1.35 * compression.powf(1.4) * ram_boost;
    let kp = raw_kp.clamp(0.0, 9.0);

    (kp, classify_kp_storm_level(kp))
}

/// Evaluates instantaneous magnetopause standoff, auroral oval geometry, Kp index, and visible intensity
/// given solar wind ram standoff, transient storm excitation (`0.0..4.0`), and moon-torus currents.
///
/// In quiet solar wind (`Kp < 2.0`, `storm_excitation ~ 0`), optical emission stays very faint (~0.02-0.12)
/// so sunlight washes out the oval on the dayside and only a delicate shimmer exists at night.
/// During flares, CMEs, ignition shockwaves, or substorms, excitation pushes Kp into 4-9 and surges luminance.
pub fn evaluate_auroral_storm_state(
    base_standoff_rp: f32,
    ram_multiplier: f32,
    b_gauss: f32,
    storm_excitation: f32,
    moon_torus_boost: f32,
) -> (f32, f32, f32, f32, f32, GeomagneticStormLevel) {
    let exc = storm_excitation.max(0.0);
    let effective_standoff_rp = (base_standoff_rp / (1.0 + exc * 0.35)).clamp(1.8, 60.0);
    let (colatitude, half_width) = calculate_auroral_oval_geometry(effective_standoff_rp);
    let (base_kp, _) = calculate_geomagnetic_kp(effective_standoff_rp, ram_multiplier, b_gauss);

    let kp = (base_kp + exc * 2.15 + moon_torus_boost * 1.2).clamp(0.0, 9.0);
    let level = classify_kp_storm_level(kp);

    let b_scale = (b_gauss / 0.5).clamp(0.35, 2.2);
    // Quiet optical floor stays low (~0.03-0.12) so planets do not show bright permanent white daylight stickers.
    let quiet_floor =
        ((kp / 2.0).clamp(0.0, 1.0) * 0.055 * b_scale + moon_torus_boost * 0.16).clamp(0.015, 0.22);
    let storm_norm = ((kp - 1.6) / 7.4).clamp(0.0, 1.0);
    let storm_luminance = if kp >= 2.0 || exc >= 0.08 {
        (0.28 + 2.95 * storm_norm.powf(1.30)) * b_scale.max(0.75) + exc * 0.22
    } else {
        0.0
    };
    let intensity = (quiet_floor + storm_luminance).clamp(0.015, 3.8);

    (
        effective_standoff_rp,
        colatitude,
        half_width,
        kp,
        intensity,
        level,
    )
}

/// Packs `(oval_colatitude_rad, oval_width_rad, auroral_intensity, kp_index)` for planet and atmosphere shaders.
pub fn calculate_aurora_shader_uniforms(
    opt_aurora: Option<&AuroralOvalState>,
    mag_gauss: f32,
) -> Vec4 {
    opt_aurora.map_or_else(
        || {
            if mag_gauss > 0.1 {
                Vec4::new(0.315, 0.055, 0.04, 1.2)
            } else {
                Vec4::ZERO
            }
        },
        |a| {
            Vec4::new(
                a.oval_colatitude_rad,
                a.oval_width_rad,
                a.auroral_intensity,
                a.geomagnetic_kp_index,
            )
        },
    )
}

/// Emission spectrum colors for the planetary auroral oval based on composition and atmospheric pressure.
/// Returns `(core_color, lower_border_color, upper_curtain_color)`.
pub fn calculate_auroral_emission_colors(
    comp: &Composition,
    is_gas_giant: bool,
) -> (Vec3, Vec3, Vec3) {
    if is_gas_giant {
        // Jovian / Saturnian Hydrogen-Helium auroras (H-alpha, H_3^+ vibrational transitions)
        (
            Vec3::new(0.92, 0.28, 0.80), // Luminous pink/crimson H-alpha core
            Vec3::new(0.38, 0.12, 0.90), // Deep energetic UV/violet
            Vec3::new(0.20, 0.85, 0.92), // Cyan upper altitude haze
        )
    } else {
        let norm = comp.normalized();
        if norm.organics_frac > 0.15 {
            // Titan / Tholin hydrocarbon smog: golden amber and electric purple
            (
                Vec3::new(0.98, 0.65, 0.15),
                Vec3::new(0.60, 0.15, 0.85),
                Vec3::new(0.85, 0.30, 0.20),
            )
        } else {
            // Terrestrial Earth-like / N2-O2 ionosphere:
            // 557.7 nm Oxygen green, 391.4 nm N2+ violet, 630.0 nm Oxygen red
            (
                Vec3::new(0.12, 0.98, 0.40), // Emerald green forbidden oxygen line
                Vec3::new(0.40, 0.20, 0.95), // Ionized molecular nitrogen violet fringe
                Vec3::new(0.95, 0.15, 0.22), // High-altitude atomic oxygen ruby red
            )
        }
    }
}
