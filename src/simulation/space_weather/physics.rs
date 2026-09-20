//! Analytical astrophysical formulas for magnetopause standoff, auroral oval geometry,
//! geomagnetic Kp indices, and multi-spectral ionospheric emission profiles.

use bevy::prelude::*;
use std::f32::consts::PI;

use crate::simulation::components::Composition;
use crate::simulation::space_weather::types::GeomagneticStormLevel;

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
    let colatitude = sin_theta.asin().clamp(0.10, PI * 0.48);

    // Oval half-width broadens as the oval expands equatorward during storms
    let half_width = (0.05 + 0.08 * (colatitude / 0.50)).clamp(0.04, 0.16);

    (colatitude, half_width)
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

    // Scaling relation: Kp ~ 1.5 * compression^1.4 * ram_boost^0.5
    let raw_kp = 1.35 * compression.powf(1.4) * ram_boost;
    let kp = raw_kp.clamp(0.0, 9.0);

    let level = if kp < 2.0 {
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
    };

    (kp, level)
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
