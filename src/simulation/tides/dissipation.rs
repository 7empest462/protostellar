//! Pure mathematical and astrophysical equations for gravitational tidal dissipation,
//! orbital circularization, spin-orbit synchronization, and viscoelastic heating.

use std::f64::consts::PI;

use crate::utils::constants::*;

/// Computes mean orbital motion $n = \sqrt{G (M_{\text{host}} + M_{\text{body}}) / a^3}$ in $\text{rad/yr}$.
pub fn calculate_mean_motion(m_host_solar: f64, m_body_solar: f64, semi_major_axis_au: f64) -> f64 {
    let a = semi_major_axis_au.max(1e-5);
    let mu = G_ASTRO * (m_host_solar + m_body_solar).max(1e-6);
    (mu / (a * a * a)).sqrt()
}

/// Computes the orbital circularization rate $de/dt$ in $\text{yr}^{-1}$ (Goldreich & Soter 1966).
///
/// Returns a negative rate indicating eccentricity damping.
pub fn calculate_circularization_rate(
    k2_over_q: f64,
    m_host_solar: f64,
    m_body_solar: f64,
    radius_au: f64,
    semi_major_axis_au: f64,
    eccentricity: f64,
    mean_motion_rad_yr: f64,
) -> f64 {
    if eccentricity <= 1e-6 || semi_major_axis_au <= 1e-4 || m_body_solar <= 1e-12 {
        return 0.0;
    }

    let a = semi_major_axis_au;
    let r_over_a = (radius_au / a).clamp(0.0, 0.5);
    let mass_ratio = (m_host_solar / m_body_solar).clamp(1.0, 1e9);

    // de/dt = - (21/2) * (k2/Q) * (M_host / M_body) * (R / a)^5 * n * e
    let de_dt =
        -10.5 * k2_over_q * mass_ratio * r_over_a.powi(5) * mean_motion_rad_yr * eccentricity;
    de_dt.clamp(-1.0, 0.0)
}

/// Computes theoretical circularization timescale $\tau_{\text{circ}} = e / |de/dt|$ in years.
pub fn calculate_circularization_timescale(
    k2_over_q: f64,
    m_host_solar: f64,
    m_body_solar: f64,
    radius_au: f64,
    semi_major_axis_au: f64,
    mean_motion_rad_yr: f64,
) -> f64 {
    if k2_over_q <= 1e-10 || semi_major_axis_au <= 1e-4 || m_body_solar <= 1e-12 {
        return 1e12;
    }

    let a = semi_major_axis_au;
    let r_over_a = (radius_au / a).clamp(1e-6, 0.5);
    let mass_ratio = (m_body_solar / m_host_solar.max(1e-6)).clamp(1e-9, 1.0);

    // tau_circ = (2/21) * (Q/k2) * (M_body / M_host) * (a / R)^5 / n
    let inv_k2_q = 1.0 / k2_over_q.max(1e-9);
    let inv_r_over_a_5 = 1.0 / r_over_a.powi(5).max(1e-25);
    let tau = (2.0 / 21.0) * inv_k2_q * mass_ratio * inv_r_over_a_5 / mean_motion_rad_yr.max(1e-6);
    tau.clamp(1e2, 1e14)
}

/// Computes the pseudo-synchronous equilibrium rotation frequency $\omega_{\text{eq}}$ (Hut 1981).
///
/// For circular orbits ($e = 0$), $\omega_{\text{eq}} = n$ (1:1 lock).
/// For eccentric orbits with $e \ge 0.15$ where 3:2 resonance capture is stable, returns $1.5 n$.
pub fn calculate_equilibrium_spin_frequency(
    mean_motion_rad_yr: f64,
    eccentricity: f64,
    allow_3_2_resonance: bool,
) -> (f64, f32) {
    let e = eccentricity.clamp(0.0, 0.95);
    let n = mean_motion_rad_yr;

    if allow_3_2_resonance && (0.15..0.35).contains(&e) {
        // Mercury-like 3:2 spin-orbit resonance capture
        (1.50 * n, 1.50)
    } else {
        // Hut (1981) pseudo-synchronous equilibrium formula
        let e2 = e * e;
        let e4 = e2 * e2;
        let e6 = e4 * e2;

        let num = 1.0 + 7.5 * e2 + 5.625 * e4 + 0.3125 * e6;
        let den = (1.0 + 3.0 * e2 + 0.375 * e4) * (1.0 - e2).powf(1.5).max(1e-4);
        let ratio = (num / den).clamp(1.0, 10.0);
        (ratio * n, ratio as f32)
    }
}

/// Computes the spin-orbit synchronization timescale $\tau_{\text{sync}}$ in years (Goldreich & Soter 1966).
pub fn calculate_sync_timescale(
    k2_over_q: f64,
    m_host_solar: f64,
    m_body_solar: f64,
    radius_au: f64,
    semi_major_axis_au: f64,
    mean_motion_rad_yr: f64,
) -> f64 {
    if k2_over_q <= 1e-10 || semi_major_axis_au <= 1e-4 || m_body_solar <= 1e-12 {
        return 1e10;
    }

    let a = semi_major_axis_au;
    let r_over_a = (radius_au / a).clamp(1e-6, 0.5);
    let mass_ratio = (m_body_solar / m_host_solar.max(1e-6)).clamp(1e-9, 1.0);

    // tau_sync = (2 alpha / 3) * (Q/k2) * (M_body / M_host) * (a / R)^3 / n
    // Differentiated interior moment of inertia alpha = C / (M R^2) ~ 0.33
    let alpha = 0.33;
    let inv_k2_q = 1.0 / k2_over_q.max(1e-9);
    let inv_r_over_a_3 = 1.0 / r_over_a.powi(3).max(1e-18);
    let tau =
        (2.0 * alpha / 3.0) * inv_k2_q * mass_ratio * inv_r_over_a_3 / mean_motion_rad_yr.max(1e-6);
    tau.clamp(1.0, 1e12)
}

/// Computes internal viscoelastic tidal heating dissipation power $P_{\text{tide}}$ in Watts (Peale & Cassen 1979).
pub fn calculate_tidal_heating_power(
    k2_over_q: f64,
    m_host_solar: f64,
    radius_au: f64,
    semi_major_axis_au: f64,
    eccentricity: f64,
    spin_frequency_rad_yr: f64,
    mean_motion_rad_yr: f64,
) -> f64 {
    if k2_over_q <= 1e-10 || semi_major_axis_au <= 1e-4 || radius_au <= 1e-8 {
        return 0.0;
    }

    // Convert to SI units for standard Watt output
    let m_host_kg = m_host_solar * SOLAR_MASS_KG;
    let r_m = radius_au * AU_TO_METERS;
    let a_m = semi_major_axis_au * AU_TO_METERS;
    let n_si = mean_motion_rad_yr / YEAR_SECONDS;

    let e = eccentricity.clamp(0.0, 0.95);
    let e2 = e * e;

    // Geometric factor G_SI * M_host^2 * R^5 / a^6
    let geom = (G_SI * m_host_kg * m_host_kg * r_m.powi(5)) / a_m.powi(6);

    // Eccentricity tidal dissipation power (synchronous component)
    let p_ecc = 10.5 * k2_over_q * geom * n_si * e2;

    // Non-synchronous rotational dissipation power
    let omega_norm = if mean_motion_rad_yr > 1e-6 {
        ((spin_frequency_rad_yr - mean_motion_rad_yr) / mean_motion_rad_yr).clamp(-5.0, 5.0)
    } else {
        0.0
    };
    let p_rot = 1.5 * k2_over_q * geom * n_si * (omega_norm * omega_norm);

    (p_ecc + p_rot).max(0.0)
}

/// Computes the surface tidal heat flux $F_{\text{tide}} = P_{\text{tide}} / (4 \pi R^2)$ in $\text{W/m}^2$.
pub fn calculate_tidal_heat_flux(power_watts: f64, radius_au: f64) -> f64 {
    let r_m = (radius_au * AU_TO_METERS).max(100.0);
    let surface_area_m2 = 4.0 * PI * r_m * r_m;
    (power_watts / surface_area_m2).max(0.0)
}

/// Evolves rotational frequency $\omega$ and axial tilt $\theta$ towards equilibrium over time step $dt$.
pub fn apply_spin_synchronization_step(
    current_omega_rad_yr: f64,
    target_omega_rad_yr: f64,
    current_axial_tilt_deg: f64,
    sync_timescale_yr: f64,
    dt_yr: f64,
) -> (f64, f64, f32) {
    if sync_timescale_yr <= 1e-6 || dt_yr <= 0.0 {
        return (current_omega_rad_yr, current_axial_tilt_deg, 1.0);
    }

    // Exponential relaxation towards target spin: domega/dt = - (omega - omega_eq) / tau
    let decay_factor = (-dt_yr / sync_timescale_yr).exp();
    let new_omega =
        target_omega_rad_yr + (current_omega_rad_yr - target_omega_rad_yr) * decay_factor;

    // Axial tilt damping towards 0 (spin aligned with orbital plane normal)
    let new_tilt = current_axial_tilt_deg * decay_factor;

    // Locking progress metric: smoothly maps to [0, 1] as frequency converges
    let diff = (new_omega - target_omega_rad_yr).abs();
    let frac = (1.0 - (diff / (target_omega_rad_yr.max(1e-4) + diff))).clamp(0.0, 1.0);
    let progress = if (diff / target_omega_rad_yr.max(1e-4)) < 0.01 && new_tilt < 1.0 {
        1.0
    } else {
        frac as f32
    };

    (new_omega, new_tilt, progress)
}
