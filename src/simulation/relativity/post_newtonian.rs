//! Post-Newtonian (1PN) gravitational corrections and Peters (1964) gravitational wave quadrupole radiation formulas.

use std::f64::consts::PI;

use bevy::math::DVec3;

use crate::utils::constants::{
    AU_TO_METERS, G_ASTRO, SOLAR_MASS_KG, SPEED_OF_LIGHT_AU_YR, YEAR_SECONDS,
};

/// Conversion factor: 1 radian in arcseconds.
pub const RAD_TO_ARCSEC: f64 = 180.0 * 3600.0 / PI;

/// Conversion factor: 1 Astrophysical Power Unit ($M_\odot \cdot \text{AU}^2 / \text{yr}^3$) to Watts.
pub const ASTRO_POWER_TO_WATTS: f64 =
    (SOLAR_MASS_KG * AU_TO_METERS * AU_TO_METERS) / (YEAR_SECONDS * YEAR_SECONDS * YEAR_SECONDS);

/// 1 kiloparsec in Astronomical Units (AU).
pub const KPC_TO_AU: f64 = 206_264_806.247;

/// Computes the General Relativistic 1PN periastron advance per orbital revolution $\Delta \varpi$ in radians.
///
/// $$\Delta \varpi = \frac{6 \pi G M}{c^2 a (1 - e^2)}$$
pub fn calculate_1pn_precession_per_orbit(
    m_central_solar: f64,
    a_au: f64,
    eccentricity: f64,
) -> f64 {
    if m_central_solar <= 1e-9 || a_au <= 1e-9 {
        return 0.0;
    }
    let e = eccentricity.clamp(0.0, 0.999);
    let one_minus_e2 = (1.0 - e * e).max(1e-6);
    let c = SPEED_OF_LIGHT_AU_YR;

    (6.0 * PI * G_ASTRO * m_central_solar) / (c * c * a_au * one_minus_e2)
}

/// Computes the 1PN relativistic periastron advance rate:
/// - First element: rate in radians per year ($\text{rad/yr}$)
/// - Second element: rate in arcseconds per century ($\text{arcsec/century}$)
///
/// $$\dot{\varpi} = \frac{3 (G M)^{3/2}}{c^2 a^{5/2} (1 - e^2)}$$
pub fn calculate_1pn_precession_rate(
    m_central_solar: f64,
    a_au: f64,
    eccentricity: f64,
) -> (f64, f64) {
    if m_central_solar <= 1e-9 || a_au <= 1e-9 {
        return (0.0, 0.0);
    }
    let e = eccentricity.clamp(0.0, 0.999);
    let one_minus_e2 = (1.0 - e * e).max(1e-6);
    let c = SPEED_OF_LIGHT_AU_YR;

    let gm = G_ASTRO * m_central_solar;
    let rate_rad_yr = (3.0 * gm.powf(1.5)) / (c * c * a_au.powf(2.5) * one_minus_e2);
    let rate_arcsec_century = rate_rad_yr * RAD_TO_ARCSEC * 100.0;

    (rate_rad_yr, rate_arcsec_century)
}

/// Computes the Post-Newtonian (1PN) acceleration correction vector $\vec{a}_{1\text{PN}}$ in $\text{AU/yr}^2$.
///
/// Standard Einstein-Infeld-Hoffmann / Post-Newtonian relative coordinate acceleration:
/// $$\vec{a}_{1\text{PN}} = \frac{G M}{c^2 r^3} \left[ \left( \frac{4 G M}{r} - v^2 \right) \vec{r} + 4 (\vec{r} \cdot \vec{v}) \vec{v} \right]$$
pub fn calculate_1pn_acceleration(rel_pos: DVec3, rel_vel: DVec3, m_central_solar: f64) -> DVec3 {
    let r_sq = rel_pos.length_squared();
    if r_sq < 1e-12 || m_central_solar <= 1e-9 {
        return DVec3::ZERO;
    }
    let r = r_sq.sqrt();
    let c = SPEED_OF_LIGHT_AU_YR;
    let c_sq = c * c;
    let gm = G_ASTRO * m_central_solar;
    let v_sq = rel_vel.length_squared();
    let r_dot_v = rel_pos.dot(rel_vel);

    let factor = gm / (c_sq * r_sq * r);
    let r_term = (4.0 * gm / r) - v_sq;
    let v_term = 4.0 * r_dot_v;

    factor * (r_term * rel_pos + v_term * rel_vel)
}

/// Computes the Peters (1964) gravitational wave quadrupole radiation power in Watts.
///
/// $$P_{\text{GW}} = \frac{32}{5} \frac{G^4}{c^5} \frac{m_1^2 m_2^2 (m_1 + m_2)}{a^5 (1 - e^2)^{7/2}} \left(1 + \frac{73}{24} e^2 + \frac{37}{96} e^4\right)$$
pub fn calculate_peters_gw_power(
    m1_solar: f64,
    m2_solar: f64,
    a_au: f64,
    eccentricity: f64,
) -> f64 {
    if m1_solar <= 1e-9 || m2_solar <= 1e-9 || a_au <= 1e-9 {
        return 0.0;
    }
    let e = eccentricity.clamp(0.0, 0.999);
    let e2 = e * e;
    let e4 = e2 * e2;
    let one_minus_e2 = (1.0 - e2).max(1e-6);
    let f_e = 1.0 + (73.0 / 24.0) * e2 + (37.0 / 96.0) * e4;

    let c = SPEED_OF_LIGHT_AU_YR;
    let g = G_ASTRO;
    let g4 = g * g * g * g;
    let c5 = c.powi(5);

    let m_factor = (m1_solar * m1_solar) * (m2_solar * m2_solar) * (m1_solar + m2_solar);
    let denom = a_au.powi(5) * one_minus_e2.powf(3.5);

    let power_astro = (32.0 / 5.0) * (g4 / c5) * (m_factor / denom) * f_e;
    power_astro * ASTRO_POWER_TO_WATTS
}

/// Computes the Peters (1964) orbital semi-major axis shrinkage rate $da/dt$ in $\text{AU/yr}$.
///
/// $$\frac{da}{dt} = -\frac{64}{5} \frac{G^3}{c^5} \frac{m_1 m_2 (m_1 + m_2)}{a^3 (1 - e^2)^{7/2}} \left(1 + \frac{73}{24} e^2 + \frac{37}{96} e^4\right)$$
pub fn calculate_peters_da_dt(m1_solar: f64, m2_solar: f64, a_au: f64, eccentricity: f64) -> f64 {
    if m1_solar <= 1e-9 || m2_solar <= 1e-9 || a_au <= 1e-9 {
        return 0.0;
    }
    let e = eccentricity.clamp(0.0, 0.999);
    let e2 = e * e;
    let e4 = e2 * e2;
    let one_minus_e2 = (1.0 - e2).max(1e-6);
    let f_e = 1.0 + (73.0 / 24.0) * e2 + (37.0 / 96.0) * e4;

    let c = SPEED_OF_LIGHT_AU_YR;
    let g3 = G_ASTRO * G_ASTRO * G_ASTRO;
    let c5 = c.powi(5);

    let m_factor = m1_solar * m2_solar * (m1_solar + m2_solar);
    let denom = a_au.powi(3) * one_minus_e2.powf(3.5);

    -(64.0 / 5.0) * (g3 / c5) * (m_factor / denom) * f_e
}

/// Computes the Peters (1964) eccentricity damping rate $de/dt$ in $\text{yr}^{-1}$.
///
/// $$\frac{de}{dt} = -\frac{304}{15} \frac{G^3}{c^5} \frac{m_1 m_2 (m_1 + m_2) e}{a^4 (1 - e^2)^{5/2}} \left(1 + \frac{121}{304} e^2\right)$$
pub fn calculate_peters_de_dt(m1_solar: f64, m2_solar: f64, a_au: f64, eccentricity: f64) -> f64 {
    if m1_solar <= 1e-9 || m2_solar <= 1e-9 || a_au <= 1e-9 || eccentricity <= 1e-6 {
        return 0.0;
    }
    let e = eccentricity.clamp(0.0, 0.999);
    let e2 = e * e;
    let one_minus_e2 = (1.0 - e2).max(1e-6);
    let g_e = 1.0 + (121.0 / 304.0) * e2;

    let c = SPEED_OF_LIGHT_AU_YR;
    let g3 = G_ASTRO * G_ASTRO * G_ASTRO;
    let c5 = c.powi(5);

    let m_factor = m1_solar * m2_solar * (m1_solar + m2_solar);
    let denom = a_au.powi(4) * one_minus_e2.powf(2.5);

    -(304.0 / 15.0) * (g3 / c5) * (m_factor * e / denom) * g_e
}

/// Computes the Peters (1964) gravitational wave coalescence lifetime $\tau_{\text{merge}}$ in years.
///
/// Circular lifetime:
/// $$\tau_0 = \frac{5}{256} \frac{c^5}{G^3} \frac{a^4}{m_1 m_2 (m_1 + m_2)}$$
///
/// Eccentric correction:
/// $$\tau(e) \approx \tau_0 \cdot (1 - e^2)^{7/2}$$
pub fn calculate_gw_coalescence_time(
    m1_solar: f64,
    m2_solar: f64,
    a_au: f64,
    eccentricity: f64,
) -> f64 {
    if m1_solar <= 1e-9 || m2_solar <= 1e-9 || a_au <= 1e-9 {
        return f64::INFINITY;
    }
    let e = eccentricity.clamp(0.0, 0.999);
    let one_minus_e2 = (1.0 - e * e).max(1e-6);

    let c = SPEED_OF_LIGHT_AU_YR;
    let g3 = G_ASTRO * G_ASTRO * G_ASTRO;
    let c5 = c.powi(5);

    let m_factor = m1_solar * m2_solar * (m1_solar + m2_solar);
    let tau_0 = (5.0 / 256.0) * (c5 / g3) * (a_au.powi(4) / m_factor);

    (tau_0 * one_minus_e2.powf(3.5)).max(1e-6)
}

/// Computes the Schwarzschild radius $r_s$ in AU for a given mass in Solar Masses.
///
/// $$r_s = \frac{2 G M}{c^2}$$
pub fn calculate_schwarzschild_radius(mass_solar: f64) -> f64 {
    let c = SPEED_OF_LIGHT_AU_YR;
    (2.0 * G_ASTRO * mass_solar.max(0.0)) / (c * c)
}

/// Computes the Innermost Stable Circular Orbit (ISCO) radius in AU ($r_{\text{ISCO}} = 3 r_s = \frac{6 G M}{c^2}$).
pub fn calculate_isco_radius(mass_solar: f64) -> f64 {
    3.0 * calculate_schwarzschild_radius(mass_solar)
}

/// Computes the quadrupole gravitational wave frequency $f_{\text{GW}} = 2 f_{\text{orb}}$:
/// - First element: frequency in $\text{yr}^{-1}$
/// - Second element: frequency in Hertz (Hz)
pub fn calculate_gw_frequency(m_total_solar: f64, a_au: f64) -> (f64, f64) {
    if m_total_solar <= 1e-9 || a_au <= 1e-9 {
        return (0.0, 0.0);
    }
    let f_orb_yr = (G_ASTRO * m_total_solar / (a_au * a_au * a_au)).sqrt() / (2.0 * PI);
    let f_gw_yr = 2.0 * f_orb_yr;
    let f_gw_hz = f_gw_yr / YEAR_SECONDS;
    (f_gw_yr, f_gw_hz)
}

/// Computes the dimensionless characteristic gravitational wave strain $h$ at a given distance in kiloparsecs (kpc).
pub fn calculate_gw_strain(chirp_mass_solar: f64, f_gw_hz: f64, distance_kpc: f64) -> f64 {
    if chirp_mass_solar <= 1e-9 || f_gw_hz <= 1e-12 || distance_kpc <= 1e-6 {
        return 0.0;
    }
    let c = SPEED_OF_LIGHT_AU_YR;
    let d_au = distance_kpc * KPC_TO_AU;
    let f_gw_yr = f_gw_hz * YEAR_SECONDS;

    // Dimensionless strain order of magnitude: h ~ (4 G / c^2 d) * (G M_chirp / c^2)^(2/3) * (pi f)^(2/3)
    let g_m_over_c2 = (G_ASTRO * chirp_mass_solar) / (c * c);
    let omega_factor = (PI * f_gw_yr).powf(2.0 / 3.0);
    let h = (4.0 / d_au) * g_m_over_c2.powf(5.0 / 3.0) * omega_factor;
    h.clamp(0.0, 1.0)
}
