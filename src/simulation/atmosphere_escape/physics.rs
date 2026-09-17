//! Pure mathematical and astrophysical formulas for atmospheric escape,
//! hydrodynamic photoevaporation, magnetopause stand-off, and solar wind stripping.

use bevy::prelude::Color;
use std::f64::consts::PI;

use crate::utils::constants::{
    AU_TO_METERS, EARTH_MASS_KG, EARTH_MASS_SOLAR, EARTH_RADIUS_AU, G_SI, SOLAR_LUMINOSITY_WATTS,
    SOLAR_MASS_KG,
};

/// Boltzmann Constant in SI units: $\text{J/K}$
pub const BOLTZMANN_SI: f64 = 1.380_649e-23;

/// Unified Atomic Mass Unit in kilograms: $\text{kg}$
pub const ATOMIC_MASS_UNIT_KG: f64 = 1.660_539_066e-27;

/// Vacuum Magnetic Permeability $\mu_0$ in $\text{H/m}$ ($\text{N/A}^2$)
pub const VACUUM_PERMEABILITY_SI: f64 = 4.0 * PI * 1e-7;

/// Conversion factor from $\text{kg/s}$ to Earth Masses per Million Years ($M_\oplus/\text{Myr}$).
/// $1\text{ Myr} = 3.15576 \times 10^{13}\text{ s}$, $1 M_\oplus = 5.9722 \times 10^{24}\text{ kg}$.
pub const KG_S_TO_M_EARTH_MYR: f64 = 3.155_76e13 / EARTH_MASS_KG;

/// Baseline solar wind dynamic ram pressure at $1\text{ AU}$ in $\text{N/m}^2$.
pub const P_RAM_1AU: f64 = 2.37e-9;

/// Baseline solar wind mass density at $1\text{ AU}$ in $\text{kg/m}^3$.
pub const RHO_1AU: f64 = 1.17e-20;

/// Computes the extreme ultraviolet (EUV) and X-ray stellar luminosity $L_{\text{XUV}}$ in Watts.
///
/// Follows empirical stellar coronal saturation laws (Ribas et al. 2005, Jackson et al. 2012):
/// - Saturated young stars ($\tau \le 100\text{ Myr}$): $\frac{L_{\text{XUV}}}{L_{\text{bol}}} \approx 10^{-3}$
/// - Mature stars ($\tau > 100\text{ Myr}$): decays as $\tau^{-1.25}$, with minimum floor $10^{-6}$.
pub fn calculate_stellar_xuv_luminosity(star_lum_solar: f64, star_age_yr: f64) -> f64 {
    let l_bol_watts = star_lum_solar.max(0.001) * SOLAR_LUMINOSITY_WATTS;
    let tau_yr = star_age_yr.max(1.0e6);
    let tau_sat_yr = 1.0e8; // 100 Myr saturation threshold

    let f_xuv = if tau_yr <= tau_sat_yr {
        1.0e-3
    } else {
        (1.0e-3 * (tau_sat_yr / tau_yr).powf(1.25)).clamp(1.0e-6, 1.0e-3)
    };

    l_bol_watts * f_xuv
}

/// Computes incident extreme ultraviolet flux $F_{\text{XUV}}$ in $\text{W/m}^2$ at a given orbital distance.
pub fn calculate_xuv_flux(l_xuv_watts: f64, dist_au: f64) -> f64 {
    let d_m = dist_au.max(0.005) * AU_TO_METERS;
    l_xuv_watts / (4.0 * PI * d_m * d_m)
}

/// Computes the Erkaev et al. (2007) tidal Roche-lobe reduction factor $K_{\text{tide}}(\xi)$.
///
/// For close-in planets, the gravitational potential well is reduced by stellar tides,
/// accelerating hydrodynamic escape:
/// $$K_{\text{tide}}(\xi) = 1 - \frac{3}{2 \xi} + \frac{1}{2 \xi^3}, \quad \xi = \frac{R_{\text{Hill}}}{R_p}$$
pub fn calculate_erkaev_roche_correction(r_planet_au: f64, r_hill_au: f64) -> f64 {
    let r_p = r_planet_au.max(1e-6);
    let r_h = r_hill_au.max(r_p * 1.01);
    let xi = r_h / r_p;
    let k_tide = 1.0 - (1.5 / xi) + (0.5 / (xi * xi * xi));
    k_tide.clamp(0.10, 1.0)
}

/// Computes hydrodynamic energy-limited photoevaporative mass loss rate $\dot{M}_{\text{photo}}$
/// in Earth masses per million years ($M_\oplus/\text{Myr}$).
///
/// $$\dot{M}_{\text{photo}} = \frac{\eta \pi R_p R_{\text{XUV}}^2 F_{\text{XUV}}}{4 G M_p K_{\text{tide}}}$$
pub fn calculate_photoevaporation_rate(
    m_planet_solar: f64,
    r_planet_au: f64,
    xuv_flux_w_m2: f64,
    efficiency_eta: f64,
    r_hill_au: f64,
) -> f64 {
    let m_kg = m_planet_solar.max(1e-7) * SOLAR_MASS_KG;
    let r_m = r_planet_au.max(1e-6) * AU_TO_METERS;
    let r_xuv_m = r_m * 1.25; // XUV optical depth unity base

    let k_tide = calculate_erkaev_roche_correction(r_planet_au, r_hill_au);
    let eta = efficiency_eta.clamp(0.01, 0.40);

    let numerator = eta * PI * r_m * r_xuv_m * r_xuv_m * xuv_flux_w_m2.max(0.0);
    let denominator = 4.0 * G_SI * m_kg * k_tide;

    let loss_rate_kg_s = numerator / denominator.max(1e-10);
    (loss_rate_kg_s * KG_S_TO_M_EARTH_MYR).clamp(0.0, 10_000.0)
}

/// Computes the stellar wind dynamic ram pressure $P_{\text{ram}} = \rho_{\text{sw}} v_{\text{sw}}^2$
/// in $\text{N/m}^2$ and plasma density $\rho_{\text{sw}}$ in $\text{kg/m}^3$.
pub fn calculate_solar_wind_pressure(dist_au: f64, star_mass_solar: f64) -> (f64, f64) {
    let d = dist_au.max(0.01);
    let inv_d2 = 1.0 / (d * d);
    let mass_factor = star_mass_solar.max(0.01).sqrt();

    let p_ram = P_RAM_1AU * inv_d2 * mass_factor;
    let rho_sw = RHO_1AU * inv_d2 * mass_factor;
    (p_ram, rho_sw)
}

/// Computes the magnetopause stand-off radius $R_{\text{mp}}$ in Astronomical Units ($\text{AU}$).
///
/// Balances dipolar magnetic pressure $\frac{B^2}{2 \mu_0}$ with stellar wind ram pressure $P_{\text{ram}}$:
/// $$R_{\text{mp}} = R_p \left( \frac{B_0^2}{2 \mu_0 P_{\text{ram}}} \right)^{1/6}$$
pub fn calculate_magnetopause_radius(
    r_planet_au: f64,
    b_surface_gauss: f64,
    ram_pressure_n_m2: f64,
) -> f64 {
    let r_p = r_planet_au.max(1e-6);
    let b_tesla = b_surface_gauss.max(0.0) * 1e-4;

    if b_tesla <= 1e-6 || ram_pressure_n_m2 <= 1e-15 {
        return r_p; // No magnetic field or unphysical pressure: magnetopause at surface
    }

    let b_sq = b_tesla * b_tesla;
    let magnetic_pressure = b_sq / (2.0 * VACUUM_PERMEABILITY_SI);
    let ratio = (magnetic_pressure / ram_pressure_n_m2.max(1e-12)).max(1.0);
    let stand_off_ratio = ratio.powf(1.0 / 6.0).clamp(1.0, 50.0);

    r_p * stand_off_ratio
}

/// Computes the geodynamo magnetic shielding effectiveness fraction $S_{\text{mag}} \in [0.0, 1.0]$.
pub fn calculate_magnetic_shielding_factor(r_planet_au: f64, r_magnetopause_au: f64) -> f32 {
    let r_p = r_planet_au.max(1e-6);
    let zeta = (r_magnetopause_au / r_p).max(1.0);

    if zeta <= 1.05 {
        return 0.0; // Unshielded (Mars / Venus-type direct solar wind ion pick-up)
    }

    let exponent = -((zeta - 1.0) * 0.75);
    (1.0 - exponent.exp()).clamp(0.0, 1.0) as f32
}

/// Computes non-thermal stellar wind ion pick-up and sputtering mass loss rate
/// in Earth masses per million years ($M_\oplus/\text{Myr}$).
pub fn calculate_solar_wind_stripping_rate(
    r_planet_au: f64,
    ram_pressure_n_m2: f64,
    shielding_factor: f32,
    m_planet_solar: f64,
) -> f64 {
    let unshielded = (1.0 - f64::from(shielding_factor)).clamp(0.0, 1.0);
    if unshielded <= 0.001 {
        return 0.0;
    }

    let r_earth = (r_planet_au / EARTH_RADIUS_AU).max(0.05);
    let m_earth = (m_planet_solar / EARTH_MASS_SOLAR).max(0.01);
    let surf_gravity = (m_earth / (r_earth * r_earth)).max(0.05);

    // Baseline unshielded loss rate: ~0.0003 M_earth / Myr at 1 AU for Mars-like body
    let base_rate = 0.0003 * (ram_pressure_n_m2 / 2.37e-9) * (r_earth * r_earth) / surf_gravity;
    (base_rate * unshielded).clamp(0.0, 500.0)
}

/// Computes the thermal Jeans escape parameter $\lambda_{\text{esc}}$ and mass loss rate ($M_\oplus/\text{Myr}$).
///
/// $$\lambda_{\text{esc}} = \frac{G M_p \mu m_u}{k_B T_{\text{exo}} R_p}$$
pub fn calculate_jeans_escape(
    m_planet_solar: f64,
    r_planet_au: f64,
    t_exobase_k: f64,
    molecular_mass_amu: f64,
    surface_pressure_bar: f64,
) -> (f64, f64) {
    let m_kg = m_planet_solar.max(1e-7) * SOLAR_MASS_KG;
    let r_m = r_planet_au.max(1e-6) * AU_TO_METERS;
    let t_exo = t_exobase_k.clamp(50.0, 50_000.0);
    let mu_kg = molecular_mass_amu.max(1.0) * ATOMIC_MASS_UNIT_KG;

    let lambda_esc = (G_SI * m_kg * mu_kg) / (BOLTZMANN_SI * t_exo * r_m);

    // Jeans loss rate scaling
    let loss_rate_m_earth_per_myr = if lambda_esc < 2.0 {
        // Hydrodynamic blow-off
        0.05 * surface_pressure_bar.clamp(0.0, 100.0)
    } else if lambda_esc < 25.0 {
        // Active thermal leakage
        (0.05 * surface_pressure_bar.clamp(0.0, 100.0)) * (-lambda_esc).exp() * (1.0 + lambda_esc)
    } else {
        0.0 // Tightly gravitationally bound
    };

    (lambda_esc, loss_rate_m_earth_per_myr)
}

/// Computes cometary volatile sublimation tail length (AU), mass loss rate ($M_\oplus/\text{Myr}$),
/// and ion glow emission color.
pub fn calculate_cometary_sublimation(
    dist_au: f64,
    star_lum_solar: f64,
    ice_frac: f64,
) -> (f32, f32, Color) {
    let snow_line_au = 2.7 * star_lum_solar.max(0.01).sqrt();
    let d = dist_au.max(0.02);

    if d > snow_line_au * 1.5 || ice_frac < 0.05 {
        return (0.0, 0.0, Color::srgba(0.3, 0.7, 1.0, 0.0));
    }

    let prox = (snow_line_au / d).powf(1.1);
    let tail_len_au = (prox * 0.75 * star_lum_solar.min(5.0).powf(0.25)).clamp(0.25, 6.0) as f32;
    let loss_rate = (0.01 * prox * ice_frac).clamp(0.001, 10.0) as f32;

    let ion_color = if ice_frac > 0.40 {
        Color::srgba(0.35, 0.85, 1.0, 0.85) // Electric Cyan (H2O / CO+ ions)
    } else if ice_frac > 0.15 {
        Color::srgba(0.60, 0.85, 1.0, 0.80) // Soft Ice Blue
    } else {
        Color::srgba(1.0, 0.65, 0.20, 0.85) // Warm Sodium/Dust Amber
    };

    (tail_len_au, loss_rate, ion_color)
}
