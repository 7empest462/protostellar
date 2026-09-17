//! Analytical and secular perturbation physics for the von Zeipel–Lidov–Kozai (ZLK) resonance mechanism.

use std::f64::consts::PI;

use bevy::math::DVec3;

use super::types::{KozaiRegime, KOZAI_CRITICAL_ANGLE_RAD};
use crate::utils::constants::{G_ASTRO, SPEED_OF_LIGHT_AU_YR};

/// Computes the mutual inclination angle between inner and outer orbital planes in radians.
///
/// $$\cos(i_{\text{mut}}) = \frac{\vec{h}_{\text{inner}} \cdot \vec{h}_{\text{outer}}}{|\vec{h}_{\text{inner}}| |\vec{h}_{\text{outer}}|}$$
pub fn compute_mutual_inclination(h_inner: DVec3, h_outer: DVec3) -> f64 {
    let len_in = h_inner.length();
    let len_out = h_outer.length();
    if len_in <= 1e-12 || len_out <= 1e-12 {
        return 0.0;
    }
    let cos_i = (h_inner.dot(h_outer) / (len_in * len_out)).clamp(-1.0, 1.0);
    cos_i.acos()
}

/// Returns whether a given mutual inclination angle (in radians) activates the Kozai-Lidov resonance.
///
/// Resonance is active when: $i_{\text{crit}} < i_{\text{mut}} < \pi - i_{\text{crit}}$,
/// where $i_{\text{crit}} = \arccos(\sqrt{3/5}) \approx 39.23^\circ$.
pub fn is_in_kozai_resonance(i_mut_rad: f64) -> bool {
    i_mut_rad > KOZAI_CRITICAL_ANGLE_RAD && i_mut_rad < (PI - KOZAI_CRITICAL_ANGLE_RAD)
}

/// Computes the theoretical maximum eccentricity $e_{\max}$ reachable via secular Kozai pumping:
///
/// $$e_{\max} = \sqrt{\max\left(0, 1 - \frac{5}{3} (1 - e_0^2) \cos^2(i_0)\right)}$$
///
/// For an initially circular orbit ($e_0 = 0$), this simplifies to $\sqrt{1 - \frac{5}{3} \cos^2(i_0)}$.
pub fn compute_max_eccentricity(e_0: f64, i_mut_rad: f64) -> f64 {
    if !is_in_kozai_resonance(i_mut_rad) {
        return e_0.clamp(0.0, 0.999);
    }
    let cos_i = i_mut_rad.cos();
    let one_minus_e0_sq = (1.0 - e_0 * e_0).max(0.0);
    let term = (5.0 / 3.0) * one_minus_e0_sq * cos_i * cos_i;
    let e_max_sq = (1.0 - term).clamp(0.0, 0.999 * 0.999);
    e_max_sq.sqrt().max(e_0)
}

/// Computes the minimum periastron distance $q_{\min} = a (1 - e_{\max})$ in AU.
pub fn compute_min_periastron(semi_major_axis_au: f64, e_max: f64) -> f64 {
    semi_major_axis_au * (1.0 - e_max.clamp(0.0, 0.9999))
}

/// Computes the secular Kozai-Lidov cycle timescale $\tau_{\text{KL}}$ in years:
///
/// $$\tau_{\text{KL}} \approx \frac{2 P_2^2}{3 \pi P_1} \frac{M_0 + m_1 + m_2}{m_2} (1 - e_2^2)^{3/2}$$
pub fn compute_kozai_timescale(
    a_inner_au: f64,
    a_outer_au: f64,
    m_central_solar: f64,
    m_perturber_solar: f64,
    e_outer: f64,
) -> f64 {
    if a_inner_au <= 1e-6 || a_outer_au <= a_inner_au || m_perturber_solar <= 1e-9 {
        return f64::INFINITY;
    }

    let m_central = m_central_solar.max(1e-6);
    let m_total = m_central + m_perturber_solar;

    // Inner and outer Keplerian orbital periods in years (P = a^1.5 / sqrt(M))
    let p_inner = a_inner_au.powf(1.5) / m_central.sqrt();
    let p_outer = a_outer_au.powf(1.5) / m_total.sqrt();

    let e_term = (1.0 - e_outer.clamp(0.0, 0.99).powi(2)).powf(1.5);
    let mass_ratio = m_total / m_perturber_solar;

    let tau_kl = (2.0 / (3.0 * PI)) * (p_outer * p_outer / p_inner) * mass_ratio * e_term;
    tau_kl.max(1e-3)
}

/// Computes the General Relativistic 1PN precession timescale $\tau_{\text{GR}}$ in Earth years:
///
/// $$\tau_{\text{GR}} = \frac{2\pi}{\dot{\varpi}_{\text{GR}}} = \frac{c^2 a^{5/2} (1 - e^2)}{3 (G M)^{3/2}} 2\pi$$
pub fn compute_gr_precession_timescale(
    a_inner_au: f64,
    m_central_solar: f64,
    eccentricity: f64,
) -> f64 {
    if a_inner_au <= 1e-6 || m_central_solar <= 1e-6 {
        return f64::INFINITY;
    }
    let gm = G_ASTRO * m_central_solar;
    let c = SPEED_OF_LIGHT_AU_YR;
    let one_minus_e2 = (1.0 - eccentricity.clamp(0.0, 0.99).powi(2)).max(1e-4);

    let rate_rad_yr = (3.0 * gm.powf(1.5)) / (c * c * a_inner_au.powf(2.5) * one_minus_e2);
    if rate_rad_yr <= 1e-20 {
        return f64::INFINITY;
    }
    (2.0 * PI) / rate_rad_yr
}

/// Evaluates General Relativistic 1PN resonance suppression / detuning:
/// Returns `(ratio, is_suppressed, effective_e_max)`.
pub fn compute_gr_resonance_suppression(
    tau_kl_yr: f64,
    tau_gr_yr: f64,
    unsuppressed_e_max: f64,
) -> (f64, bool, f64) {
    if !tau_kl_yr.is_finite() || !tau_gr_yr.is_finite() || tau_gr_yr <= 1e-6 {
        return (0.0, false, unsuppressed_e_max);
    }

    let ratio = tau_kl_yr / tau_gr_yr;
    let is_suppressed = ratio > 1.0;

    let effective_e_max = if is_suppressed {
        // Detuning damps the peak eccentricity excursion
        unsuppressed_e_max / (1.0 + ratio * ratio).sqrt()
    } else {
        unsuppressed_e_max
    };

    (ratio, is_suppressed, effective_e_max)
}

/// Computes the fluid Roche disruption radius in AU:
///
/// $$R_{\text{Roche}} \approx 2.456 R_{\text{inner}} \left(\frac{M_{\text{central}}}{m_{\text{inner}}}\right)^{1/3}$$
pub fn compute_roche_disruption_radius(
    r_inner_au: f64,
    m_central_solar: f64,
    m_inner_solar: f64,
) -> f64 {
    if m_inner_solar <= 1e-12 || r_inner_au <= 1e-12 {
        return 0.0;
    }
    2.456 * r_inner_au * (m_central_solar / m_inner_solar).cbrt()
}

/// Performs a secular quadrupole step updating eccentricity, mutual inclination, and argument of periapsis:
///
/// $$\frac{de}{dt} = \frac{15}{8 \tau_{\text{KL}}} e \sqrt{1 - e^2} \sin^2(i_{\text{mut}}) \sin(2\omega)$$
/// $$\frac{di_{\text{mut}}}{dt} = - \frac{15}{16 \tau_{\text{KL}}} \frac{e^2}{\sqrt{1 - e^2}} \sin(2 i_{\text{mut}}) \sin(2\omega)$$
/// $$\frac{d\omega}{dt} = \frac{3}{4 \tau_{\text{KL}} \sqrt{1 - e^2}} [2 (1 - e^2) + 5 \sin^2\omega (e^2 - \sin^2 i)] + \dot{\omega}_{\text{GR}}$$
pub fn evaluate_secular_kozai_step(
    mut e: f64,
    mut i_mut_rad: f64,
    mut omega_rad: f64,
    tau_kl_yr: f64,
    dt_yr: f64,
    gr_rate_rad_yr: f64,
) -> (f64, f64, f64) {
    if tau_kl_yr <= 1e-4 || dt_yr.abs() <= 1e-12 {
        return (e, i_mut_rad, omega_rad);
    }

    let e_clamped = e.clamp(1e-4, 0.999);
    let sqrt_term = (1.0 - e_clamped * e_clamped).sqrt();
    let sin_i = i_mut_rad.sin();
    let sin_2i = (2.0 * i_mut_rad).sin();
    let sin_2w = (2.0 * omega_rad).sin();
    let sin_w = omega_rad.sin();

    let de_dt = (15.0 / (8.0 * tau_kl_yr)) * e_clamped * sqrt_term * sin_i * sin_i * sin_2w;
    let di_dt =
        -(15.0 / (16.0 * tau_kl_yr)) * (e_clamped * e_clamped / sqrt_term) * sin_2i * sin_2w;

    let dw_secular = (3.0 / (4.0 * tau_kl_yr * sqrt_term))
        * (2.0 * (1.0 - e_clamped * e_clamped)
            + 5.0 * sin_w * sin_w * (e_clamped * e_clamped - sin_i * sin_i));
    let dw_dt = dw_secular + gr_rate_rad_yr;

    e = (e + de_dt * dt_yr).clamp(1e-4, 0.999);
    i_mut_rad = (i_mut_rad + di_dt * dt_yr).clamp(0.0, PI);
    omega_rad = (omega_rad + dw_dt * dt_yr).rem_euclid(2.0 * PI);

    (e, i_mut_rad, omega_rad)
}

/// Classifies the secular regime for a body given its orbital geometry and parameters.
pub fn classify_kozai_regime(
    is_in_resonance: bool,
    is_gr_suppressed: bool,
    q_min_au: f64,
    roche_limit_au: f64,
    omega_rad: f64,
) -> KozaiRegime {
    if !is_in_resonance {
        return KozaiRegime::Inactive;
    }
    if is_gr_suppressed {
        return KozaiRegime::SuppressedByGR;
    }
    if q_min_au <= roche_limit_au {
        return KozaiRegime::TidalDisruptionRisk;
    }

    // Libration occurs when argument of periapsis oscillates near 90 or 270 deg (within +/- 35 deg)
    let w_deg = omega_rad.to_degrees().rem_euclid(360.0);
    let dist_to_90 = (w_deg - 90.0).abs();
    let dist_to_270 = (w_deg - 270.0).abs();
    if dist_to_90 < 35.0 || dist_to_270 < 35.0 {
        KozaiRegime::Libration
    } else {
        KozaiRegime::Circulation
    }
}
