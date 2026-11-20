//! Planetary Climate, Surface Temperature, Volatile Condensation, and Biosphere Genesis.

use bevy::prelude::*;

use crate::simulation::components::*;

#[derive(Clone, Copy)]
struct SurfaceTemperatureResult {
    surface_temp: f64,
    equilibrium_temp: f64,
    greenhouse_delta: f32,
}

#[derive(Clone, Copy)]
pub struct StellarIrradiance {
    pub lum_solar: f64,
    pub temp_k: f64,
    pub radius_au: f64,
}

impl Default for StellarIrradiance {
    fn default() -> Self {
        Self {
            lum_solar: 1.0,
            temp_k: 5778.0,
            radius_au: crate::utils::constants::SOLAR_RADIUS_AU,
        }
    }
}

pub fn stellar_irradiance_from_mass_or_opt(
    lum: Option<f64>,
    temp_k: Option<f64>,
    radius_au: Option<f64>,
    mass_solar: f64,
) -> StellarIrradiance {
    let star_m = mass_solar.max(0.08);
    let default_r =
        (crate::utils::constants::SOLAR_RADIUS_AU * star_m.powf(0.8)).clamp(0.001, 0.20);
    let default_t = (5778.0 * star_m.powf(0.505)).clamp(2800.0, 50_000.0);
    let r = radius_au.filter(|v| *v > 1e-6).unwrap_or(default_r);
    let t = temp_k.filter(|v| *v > 10.0).unwrap_or(default_t);
    let default_lum = (r / crate::utils::constants::SOLAR_RADIUS_AU).powi(2) * (t / 5778.0).powi(4);
    StellarIrradiance {
        lum_solar: lum.unwrap_or(default_lum).max(0.0),
        temp_k: t,
        radius_au: r,
    }
}

pub(crate) fn compute_equilibrium_temp_k(
    r: f64,
    star_lum: f64,
    star_temp: f64,
    star_r: f64,
    albedo: f32,
) -> f64 {
    let flux_transmission = if star_lum <= 0.0 {
        0.0
    } else {
        let sb_lum = (star_r / crate::utils::constants::SOLAR_RADIUS_AU).powi(2)
            * (star_temp / 5778.0).powi(4);
        if sb_lum > 1e-12 {
            (star_lum / sb_lum).max(0.0)
        } else {
            1.0
        }
    };
    star_temp
        * (star_r / (2.0 * r.max(1e-4))).sqrt()
        * ((1.0 - f64::from(albedo)) * flux_transmission).powf(0.25)
}

#[inline]
fn compute_solar_dry_temp_k(
    r: f64,
    star_lum: f64,
    star_temp: f64,
    star_r: f64,
    dry_pressure_bar: f32,
) -> f64 {
    let eq_temp = compute_equilibrium_temp_k(r, star_lum, star_temp, star_r, 0.28);
    let dry_gh = if dry_pressure_bar > 0.01 {
        33.0 * (dry_pressure_bar / 1.0).powf(0.28) * 0.95
    } else {
        0.0
    };
    eq_temp + f64::from(dry_gh)
}

#[inline]
fn cloud_reflection_albedo(atm_pressure: f32, cloud_coverage: f32) -> f32 {
    if atm_pressure > 0.01 {
        0.22 * cloud_coverage.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[allow(clippy::too_many_arguments, reason = "Thermal calculation helper")]
fn compute_surface_temperature(
    r: f64,
    star_lum: f64,
    star_temp: f64,
    star_r: f64,
    shockwave_r: f64,
    current_ice: f32,
    climate_ice_frac: f32,
    current_temp: f64,
    ocean_frac: f32,
    cloud_coverage: f32,
    water_reservoir_m_earth: f64,
    atm_pressure: f32,
    dt_yr: f64,
) -> SurfaceTemperatureResult {
    let albedo = reported_planetary_albedo(
        ocean_frac,
        current_ice.max(climate_ice_frac),
        atm_pressure,
        cloud_coverage,
    );

    let equilibrium_temp = compute_equilibrium_temp_k(r, star_lum, star_temp, star_r, albedo);

    let shock_boost = if shockwave_r > 0.0 && shockwave_r < 29.5 && (r - shockwave_r).abs() < 0.35 {
        15.0 * (1.0 - (r - shockwave_r).abs() / 0.35)
    } else {
        0.0
    };

    let mut greenhouse_delta = if atm_pressure > 0.01 {
        33.0 * (atm_pressure / 1.0).powf(0.28) * (0.80 + ocean_frac * 0.28)
    } else {
        0.0
    };

    // Smooth runaway steam feedback over 330..390 K weighted by total H2O inventory
    let vapor_weight = ((water_reservoir_m_earth / 0.0002).clamp(0.0, 1.0) as f32).max(ocean_frac);
    if vapor_weight > 0.0 {
        let base_temp = equilibrium_temp + f64::from(greenhouse_delta);
        let t_ramp = ((base_temp - 330.0) / 60.0).clamp(0.0, 1.0) as f32;
        let smooth_runaway = t_ramp * t_ramp * (3.0 - 2.0 * t_ramp);
        greenhouse_delta =
            (greenhouse_delta * (1.0 + 2.5 * smooth_runaway * vapor_weight)).min(450.0);
    }

    let target_temp =
        (equilibrium_temp + f64::from(greenhouse_delta) + shock_boost).clamp(30.0, 5000.0);
    let surface_temp = if current_temp > target_temp + 1.0 {
        // Radiative cooling of magma ocean / impact thermal surplus towards equilibrium
        let cool_rate = if current_temp > 340.0 {
            0.38 * (current_temp / 900.0).powi(2).clamp(0.85, 4.0)
        } else {
            0.08 * (current_temp / 1000.0).powi(3).clamp(0.01, 15.0)
        };
        let k_cool = (1.0 - (-cool_rate * dt_yr).exp()).clamp(0.0, 1.0);
        let cooled = (current_temp + (target_temp - current_temp) * k_cool).max(target_temp);
        if current_temp > 340.0 && cooled <= 360.0 && target_temp < 320.0 {
            target_temp
        } else {
            cooled
        }
    } else {
        target_temp
    };

    SurfaceTemperatureResult {
        surface_temp,
        equilibrium_temp,
        greenhouse_delta,
    }
}

/// Computes surface hydrostatic column pressure in bars for an atmospheric volatile mass:
/// $P = M_{\text{gas}} g / (4 \pi R^2)$, where $g = G M_p / R^2$.
pub fn hydrostatic_pressure_bar(gas_mass_earth: f64, mass_solar: f64, radius_au: f64) -> f32 {
    use crate::utils::constants::{AU_TO_METERS, EARTH_MASS_KG, G_SI, SOLAR_MASS_KG};
    if !gas_mass_earth.is_finite()
        || !mass_solar.is_finite()
        || !radius_au.is_finite()
        || gas_mass_earth <= 0.0
        || mass_solar <= 0.0
        || radius_au <= 0.0
    {
        return 0.0;
    }
    let r_m = (radius_au * AU_TO_METERS).max(1.0e3);
    let m_planet_kg = mass_solar * SOLAR_MASS_KG;
    let m_gas_kg = gas_mass_earth * EARTH_MASS_KG;
    let area_m2 = 4.0 * std::f64::consts::PI * r_m * r_m;
    let g_ms2 = (G_SI * m_planet_kg) / (r_m * r_m);
    let p_pa = (m_gas_kg * g_ms2) / area_m2;
    (p_pa / 1.0e5).clamp(0.0, 1.0e6) as f32
}

/// Inverse of [`hydrostatic_pressure_bar`]: atmospheric mass (in $M_\oplus$) required to exert
/// `pressure_bar` on a world of mass `mass_solar` and radius `radius_au`.
pub fn gas_mass_for_pressure_bar(pressure_bar: f32, mass_solar: f64, radius_au: f64) -> f64 {
    use crate::utils::constants::{AU_TO_METERS, EARTH_MASS_KG, G_SI, SOLAR_MASS_KG};
    if !pressure_bar.is_finite()
        || !mass_solar.is_finite()
        || !radius_au.is_finite()
        || pressure_bar <= 0.0
        || mass_solar <= 0.0
        || radius_au <= 0.0
    {
        return 0.0;
    }
    let r_m = (radius_au * AU_TO_METERS).max(1.0e3);
    let m_planet_kg = mass_solar * SOLAR_MASS_KG;
    let area_m2 = 4.0 * std::f64::consts::PI * r_m * r_m;
    let g_ms2 = (G_SI * m_planet_kg) / (r_m * r_m);
    let p_pa = f64::from(pressure_bar) * 1.0e5;
    let m_gas_kg = (p_pa * area_m2) / g_ms2.max(1e-12);
    (m_gas_kg / EARTH_MASS_KG).max(0.0)
}

const H2O_TRIPLE_TEMP_K: f64 = 273.16;
const H2O_TRIPLE_PRESSURE_BAR: f64 = 0.006_116_57;
const H2O_BOIL_1ATM_TEMP_K: f64 = 373.15;
const H2O_BOIL_1ATM_PRESSURE_BAR: f64 = 1.013_25;
const H2O_CRITICAL_TEMP_K: f64 = 647.096;
const H2O_CRITICAL_PRESSURE_BAR: f64 = 220.64;
const H2O_SUBLIMATION_COEFF: f64 = 6149.0;
const H2O_VAPOR_LOW_COEFF: f64 = 5209.105;
const H2O_VAPOR_HIGH_COEFF: f64 = 4745.03;

fn compute_steam_condensation_fraction(surface_temp: f64, total_pressure_bar: f32) -> f32 {
    if surface_temp < H2O_TRIPLE_TEMP_K || surface_temp >= H2O_CRITICAL_TEMP_K {
        return 0.0;
    }
    if total_pressure_bar < H2O_TRIPLE_PRESSURE_BAR as f32 {
        return 0.0;
    }
    let t_boil = water_boiling_point_k(total_pressure_bar);
    if surface_temp >= t_boil { 0.0 } else { 1.0 }
}

/// Computes minimum atmospheric pressure (bar) from retained primordial H2/He gas
/// on worlds massive enough to gravitationally bind a dense envelope (`>= 0.5 M_earth`, `>= 1.5%` gas).
pub fn retained_gas_envelope_bar(mass_solar: f64, gas_frac: f32) -> f32 {
    if mass_solar >= 0.5 * crate::utils::constants::EARTH_MASS_SOLAR && gas_frac >= 0.015 {
        let mass_earth = mass_solar / crate::utils::constants::EARTH_MASS_SOLAR;
        (mass_earth * f64::from(gas_frac) * 25.0).clamp(0.5, 500.0) as f32
    } else {
        0.0
    }
}

/// Computes the Clausius-Clapeyron saturation vapor pressure (bar) of pure $H_2O$
/// anchored across triple point ($273.16\text{ K}$, $0.00611657\text{ bar}$),
/// standard boiling point ($373.15\text{ K}$, $1.01325\text{ bar}$), and critical point ($647.096\text{ K}$, $220.64\text{ bar}$).
pub fn water_saturation_vapor_pressure_bar(temp_k: f64) -> f32 {
    if temp_k <= 50.0 || !temp_k.is_finite() {
        return 0.0;
    }
    if temp_k <= H2O_TRIPLE_TEMP_K {
        let exponent = H2O_SUBLIMATION_COEFF * (1.0 / H2O_TRIPLE_TEMP_K - 1.0 / temp_k);
        (H2O_TRIPLE_PRESSURE_BAR * exponent.exp()).clamp(0.0, H2O_TRIPLE_PRESSURE_BAR) as f32
    } else if temp_k <= H2O_BOIL_1ATM_TEMP_K {
        let exponent = H2O_VAPOR_LOW_COEFF * (1.0 / H2O_TRIPLE_TEMP_K - 1.0 / temp_k);
        (H2O_TRIPLE_PRESSURE_BAR * exponent.exp()).clamp(0.0, H2O_CRITICAL_PRESSURE_BAR) as f32
    } else {
        let clamped_t = temp_k.min(H2O_CRITICAL_TEMP_K);
        let exponent = H2O_VAPOR_HIGH_COEFF * (1.0 / H2O_BOIL_1ATM_TEMP_K - 1.0 / clamped_t);
        (H2O_BOIL_1ATM_PRESSURE_BAR * exponent.exp()).clamp(0.0, H2O_CRITICAL_PRESSURE_BAR) as f32
    }
}

/// Computes the equilibrium $H_2O$ boiling / sublimation point (K) at a given surface pressure (bar),
/// matching [`water_saturation_vapor_pressure_bar`].
pub fn water_boiling_point_k(pressure_bar: f32) -> f64 {
    let p = f64::from(pressure_bar.max(1e-12));
    if p >= H2O_CRITICAL_PRESSURE_BAR {
        H2O_CRITICAL_TEMP_K
    } else if p >= H2O_BOIL_1ATM_PRESSURE_BAR {
        let inv_t = 1.0 / H2O_BOIL_1ATM_TEMP_K
            - (p / H2O_BOIL_1ATM_PRESSURE_BAR).ln() / H2O_VAPOR_HIGH_COEFF;
        (1.0 / inv_t).clamp(H2O_BOIL_1ATM_TEMP_K, H2O_CRITICAL_TEMP_K)
    } else if p >= H2O_TRIPLE_PRESSURE_BAR {
        let inv_t =
            1.0 / H2O_TRIPLE_TEMP_K - (p / H2O_TRIPLE_PRESSURE_BAR).ln() / H2O_VAPOR_LOW_COEFF;
        (1.0 / inv_t).clamp(H2O_TRIPLE_TEMP_K, H2O_BOIL_1ATM_TEMP_K)
    } else {
        let inv_t =
            1.0 / H2O_TRIPLE_TEMP_K - (p / H2O_TRIPLE_PRESSURE_BAR).ln() / H2O_SUBLIMATION_COEFF;
        (1.0 / inv_t).clamp(50.0, H2O_TRIPLE_TEMP_K)
    }
}

/// Returns whether a non-giant world gravitationally retains a long-lived secondary
/// outgassed $N_2$/$CO_2$/$H_2O$ atmosphere against thermal/XUV escape and solar wind stripping.
/// Warm bodies require sufficient gravity ($\ge 0.15\,M_\oplus$); cryogenic outer-disk bodies
/// ($< 140\text{ K}$) like Titan ($\ge 0.020\,M_\oplus$) can hold cold heavy $N_2$/$CH_4$.
pub fn can_retain_secondary_atmosphere(
    body_type: BodyType,
    mass_solar: f64,
    surface_temp: f64,
    has_water_volatiles: bool,
) -> bool {
    if body_type.is_star_or_remnant()
        || matches!(
            body_type,
            BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal | BodyType::DustGrain
        )
    {
        return false;
    }
    if surface_temp < 140.0 {
        has_water_volatiles && mass_solar >= 0.020 * crate::utils::constants::EARTH_MASS_SOLAR
    } else {
        mass_solar >= 0.15 * crate::utils::constants::EARTH_MASS_SOLAR
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Volatile condensation computes reversible H2O phase balance from gravity, pressure, and escape"
)]
fn update_volatile_condensation(
    opt_vol: &mut Option<&mut VolatileInventory>,
    body_type: BodyType,
    b_mass_solar: f64,
    b_radius_au: f64,
    has_water_volatiles: bool,
    current_ice: f32,
    comp_gas_frac: f32,
    surface_temp: f64,
    prev_steam_bar: f32,
    active_escape_rate: f64,
) -> (f32, f32, f32) {
    let retained_gas_bar = retained_gas_envelope_bar(b_mass_solar, comp_gas_frac);
    let can_retain_secondary_atm =
        can_retain_secondary_atmosphere(body_type, b_mass_solar, surface_temp, has_water_volatiles);
    let Some(vol) = opt_vol else {
        return (0.0, retained_gas_bar, 0.0);
    };
    if retained_gas_bar > 0.0 {
        vol.atmospheric_pressure_bar = vol.atmospheric_pressure_bar.max(retained_gas_bar);
    }
    if !has_water_volatiles {
        vol.ocean_coverage_frac = 0.0;
        return (0.0, vol.atmospheric_pressure_bar, 0.0);
    }

    let max_ocean = if vol.delivered_water_m_earth > 1e-6 {
        (vol.delivered_water_m_earth / 0.0005).clamp(0.0, 0.85) as f32
    } else if can_retain_secondary_atm || (surface_temp < 175.0 && current_ice > 0.15) {
        let ice_ocean = vol
            .ocean_coverage_frac
            .max((current_ice * 3.0).clamp(0.0, 0.85));
        if ice_ocean > 0.0 {
            vol.delivered_water_m_earth = f64::from(ice_ocean) * 0.0005;
        }
        ice_ocean
    } else {
        0.0
    };

    let dry_base = (vol.atmospheric_pressure_bar - prev_steam_bar).max(retained_gas_bar);
    let dry_atm = if can_retain_secondary_atm && max_ocean > 0.005 && active_escape_rate < 1e-7 {
        let outgassed_n2_co2_bar = (0.08 + max_ocean * 1.15).clamp(0.08, 1.50);
        dry_base.max(outgassed_n2_co2_bar)
    } else {
        dry_base
    };

    let eff_radius_au = effective_body_radius_au(b_radius_au, b_mass_solar);
    let total_h2o_bar =
        hydrostatic_pressure_bar(vol.delivered_water_m_earth, b_mass_solar, eff_radius_au);
    let p_sat = water_saturation_vapor_pressure_bar(surface_temp);

    let (steam_pressure, liquid_m_earth) = if !can_retain_secondary_atm {
        let transient_steam = if surface_temp >= 175.0 && vol.delivered_water_m_earth > 1e-6 {
            p_sat.min(total_h2o_bar).min(0.018)
        } else {
            0.0
        };
        let total_p = (dry_atm + transient_steam).max(0.0);
        let liq = if total_p >= 0.02
            && (H2O_TRIPLE_TEMP_K..H2O_CRITICAL_TEMP_K).contains(&surface_temp)
        {
            (vol.delivered_water_m_earth
                - gas_mass_for_pressure_bar(transient_steam, b_mass_solar, eff_radius_au))
            .max(0.0)
        } else {
            0.0
        };
        (transient_steam, liq)
    } else if surface_temp >= H2O_CRITICAL_TEMP_K {
        (total_h2o_bar.clamp(0.0, 450.0), 0.0)
    } else if surface_temp < H2O_TRIPLE_TEMP_K {
        let ice_vapor = p_sat.min(total_h2o_bar);
        (ice_vapor, 0.0)
    } else {
        let vapor_bar = p_sat.min(total_h2o_bar);
        let total_p = dry_atm + vapor_bar;
        let cond_frac = compute_steam_condensation_fraction(surface_temp, total_p);
        let liq = if cond_frac > 0.0 {
            (vol.delivered_water_m_earth
                - gas_mass_for_pressure_bar(vapor_bar, b_mass_solar, eff_radius_au))
            .max(0.0)
                * f64::from(cond_frac)
        } else {
            0.0
        };
        (vapor_bar, liq)
    };

    let atm_pressure = (dry_atm + steam_pressure).clamp(0.0, 500.0);
    let recorded_steam = (atm_pressure - dry_atm).max(0.0);
    vol.atmospheric_pressure_bar = atm_pressure;

    let ocean_frac = if atm_pressure >= H2O_TRIPLE_PRESSURE_BAR as f32
        && (can_retain_secondary_atm || atm_pressure >= 0.02)
        && liquid_m_earth > 1e-9
    {
        (liquid_m_earth / 0.0005).clamp(0.0, 0.85) as f32
    } else {
        0.0
    };
    vol.ocean_coverage_frac = ocean_frac;
    (ocean_frac, atm_pressure, recorded_steam)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Determines climate regime, ice fraction, and cloud coverage from surface state"
)]
fn determine_climate_coverage(
    body_type: BodyType,
    atm_pressure: f32,
    surface_temp: f64,
    has_water_volatiles: bool,
    delivered_water_m_earth: f64,
    ocean_frac: f32,
    current_ice: f32,
    comp_gas_frac: f32,
) -> (ClimateRegime, f32, f32, f32) {
    let effective_pressure = if atm_pressure >= 0.02 {
        atm_pressure
    } else if comp_gas_frac > 0.005 {
        (comp_gas_frac * 2.0).max(atm_pressure)
    } else {
        atm_pressure
    };

    let climate_regime =
        if comp_gas_frac > 0.20 || matches!(body_type, BodyType::GasGiant | BodyType::IceGiant) {
            ClimateRegime::GasGiantEnvelope
        } else if effective_pressure < 0.02 {
            ClimateRegime::AirlessVacuum
        } else if surface_temp < 250.0 {
            ClimateRegime::SnowballIceAge
        } else if surface_temp > 360.0 {
            ClimateRegime::RunawayVenusian
        } else {
            ClimateRegime::TemperateHabitable
        };

    let has_water = has_water_volatiles
        && (ocean_frac > 0.005 || current_ice > 0.002 || delivered_water_m_earth > 1e-6);
    let ice_coverage = if has_water {
        match climate_regime {
            ClimateRegime::SnowballIceAge => 1.0,
            ClimateRegime::TemperateHabitable => {
                if surface_temp < 255.0 {
                    1.0
                } else if surface_temp < 275.0 {
                    let t_frac = (275.0 - surface_temp as f32) / 20.0;
                    (0.35 + t_frac * 0.60).clamp(0.35, 0.95)
                } else if surface_temp < 298.0 {
                    ((298.0 - surface_temp as f32) / 23.0 * 0.25).clamp(0.0, 0.35)
                } else {
                    0.0
                }
            }
            // In vacuum without gas, solid water ice stays stable on the surface when cold (< 175 K),
            // whereas warm vacuum causes exposed ice to sublimate away.
            ClimateRegime::AirlessVacuum if surface_temp < 175.0 && current_ice > 0.15 => {
                (current_ice * 1.4).clamp(0.25, 1.0)
            }
            _ => 0.0,
        }
    } else {
        0.0
    };

    let polar_ice_lat = (90.0 * (1.0 - ice_coverage)).clamp(0.0, 90.0);

    let pressure_clouds = if effective_pressure > 15.0 {
        ((effective_pressure - 15.0) / 60.0).clamp(0.45, 0.95)
    } else {
        0.0
    };
    let cloud_coverage = if effective_pressure < 0.02 && comp_gas_frac <= 0.005 {
        0.0
    } else if has_water_volatiles {
        let base_water_clouds = (0.28 + ocean_frac * 0.34).clamp(0.10, 0.90);
        if surface_temp >= 647.0 {
            // Supercritical steam + CO2 envelope above the critical point of water
            0.95
        } else if surface_temp > 380.0 {
            let boil_frac = ((surface_temp as f32 - 380.0) / 55.0).clamp(0.0, 1.0);
            (base_water_clouds + boil_frac * 0.65).clamp(0.10, 0.98)
        } else {
            base_water_clouds
        }
        .max(pressure_clouds)
    } else if effective_pressure > 15.0 {
        // Crushing dry Venus-like atmosphere with dense sulfuric/aerosol cloud decks
        pressure_clouds
    } else if comp_gas_frac > 0.005 || effective_pressure >= 0.05 {
        // Volatile gas envelope on a desiccated surface: visible dynamic cloud bands & fronts (25%..68%),
        // without turning into a 100% opaque boiling-ocean whiteout
        (0.24 + comp_gas_frac * 6.5 + (effective_pressure / 12.0).clamp(0.0, 0.20))
            .clamp(0.22, 0.68)
    } else {
        0.0
    };

    (climate_regime, ice_coverage, cloud_coverage, polar_ice_lat)
}

fn effective_body_radius_au(b_radius_au: f64, b_mass_solar: f64) -> f64 {
    if b_radius_au > 1e-9 {
        b_radius_au
    } else {
        crate::simulation::accretion::impact_regimes::radius_from_mass_density(
            b_mass_solar.max(1e-12),
            crate::utils::constants::DENSITY_ROCK_ASTRO,
        )
    }
}

fn reported_planetary_albedo(
    ocean_frac: f32,
    ice_coverage: f32,
    atm_pressure: f32,
    cloud_coverage: f32,
) -> f32 {
    let bare_albedo = 0.08 * ocean_frac + 0.28 * (1.0 - ocean_frac);
    let surface_albedo = (1.0 - ice_coverage) * bare_albedo + ice_coverage * 0.68;
    (surface_albedo + cloud_reflection_albedo(atm_pressure, cloud_coverage)).clamp(0.12, 0.78)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Infers uncached steam portion when loading saves with boiled volatile atmospheres"
)]
fn infer_uncached_steam_bar(
    initial_atm_pressure: f32,
    prev_steam_bar: f32,
    current_temp_k: f64,
    has_water_volatiles: bool,
    water_reservoir: f64,
    b_mass_solar: f64,
    eff_radius_au: f64,
    comp_gas_frac: f32,
    body_type: BodyType,
) -> f32 {
    if prev_steam_bar > 0.0 {
        return prev_steam_bar;
    }
    // Only infer uncached boiled steam when the planet is hot enough to sustain dense vapor;
    // cool/temperate worlds with >3 bar dry CO2/N2 atmospheres must not have dry gas stripped.
    if has_water_volatiles
        && current_temp_k >= 340.0
        && water_reservoir > 1e-5
        && comp_gas_frac <= 0.15
        && !matches!(body_type, BodyType::GasGiant | BodyType::IceGiant)
        && initial_atm_pressure > 3.0
    {
        let retained = retained_gas_envelope_bar(b_mass_solar, comp_gas_frac);
        let max_ocean = (water_reservoir / 0.0005).clamp(0.0, 0.85) as f32;
        let expected_dry = retained.max((0.08 + max_ocean * 1.15).clamp(0.08, 1.50));
        let max_h2o_bar = hydrostatic_pressure_bar(water_reservoir, b_mass_solar, eff_radius_au)
            .clamp(0.0, 450.0);
        return (initial_atm_pressure - expected_dry).clamp(0.0, max_h2o_bar);
    }
    0.0
}

fn resolve_effective_greenhouse_pressure(
    dry_initial: f32,
    has_water_volatiles: bool,
    solar_dry_temp: f64,
    water_reservoir: f64,
    b_mass_solar: f64,
    eff_radius_au: f64,
) -> f32 {
    if !has_water_volatiles {
        return dry_initial;
    }
    let max_h2o_bar =
        hydrostatic_pressure_bar(water_reservoir, b_mass_solar, eff_radius_au).clamp(0.0, 450.0);
    let current_steam = if solar_dry_temp >= H2O_CRITICAL_TEMP_K {
        max_h2o_bar
    } else if solar_dry_temp >= 330.0 {
        water_saturation_vapor_pressure_bar(solar_dry_temp).min(max_h2o_bar)
    } else {
        water_saturation_vapor_pressure_bar(solar_dry_temp)
            .min(max_h2o_bar)
            .min(0.05)
    };
    (dry_initial + current_steam).max(dry_initial)
}

struct PreparedGreenhouseInputs {
    has_water_volatiles: bool,
    eff_radius_au: f64,
    effective_prev_steam: f32,
    solar_dry_temp: f64,
    effective_atm: f32,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Combines uncached steam inference, solar dry temp, and steam greenhouse pressure"
)]
fn prepare_greenhouse_inputs(
    body_type: BodyType,
    b_mass_solar: f64,
    b_radius_au: f64,
    comp: &Composition,
    water_reservoir: f64,
    initial_atm_pressure: f32,
    prev_steam_bar: f32,
    current_temp_k: f64,
    r: f64,
    star: StellarIrradiance,
) -> PreparedGreenhouseInputs {
    let has_water_volatiles = (comp.ice_frac as f32) > 0.001 || water_reservoir > 1e-6;
    let eff_radius_au = effective_body_radius_au(b_radius_au, b_mass_solar);
    let effective_prev_steam = infer_uncached_steam_bar(
        initial_atm_pressure,
        prev_steam_bar,
        current_temp_k,
        has_water_volatiles,
        water_reservoir,
        b_mass_solar,
        eff_radius_au,
        comp.gas_frac as f32,
        body_type,
    );
    let dry_initial = (initial_atm_pressure - effective_prev_steam).max(0.0);
    let solar_dry_temp =
        compute_solar_dry_temp_k(r, star.lum_solar, star.temp_k, star.radius_au, dry_initial);
    let effective_atm = resolve_effective_greenhouse_pressure(
        dry_initial,
        has_water_volatiles,
        solar_dry_temp,
        water_reservoir,
        b_mass_solar,
        eff_radius_au,
    );
    PreparedGreenhouseInputs {
        has_water_volatiles,
        eff_radius_au,
        effective_prev_steam,
        solar_dry_temp,
        effective_atm,
    }
}

fn finalize_post_thermal_climate(
    opt_vol: &mut Option<&mut VolatileInventory>,
    body_type: BodyType,
    b_mass_solar: f64,
    comp: &Composition,
    thermal: SurfaceTemperatureResult,
    prep: &PreparedGreenhouseInputs,
    active_escape_rate: f64,
) -> (PlanetaryClimate, f32, f32, f32) {
    let current_ice = comp.ice_frac as f32;
    let gas_frac = comp.gas_frac as f32;
    let (ocean_frac, atm_pressure, new_steam_bar) = update_volatile_condensation(
        opt_vol,
        body_type,
        b_mass_solar,
        prep.eff_radius_au,
        prep.has_water_volatiles,
        current_ice,
        gas_frac,
        thermal.surface_temp,
        prep.effective_prev_steam,
        active_escape_rate,
    );
    let post_water = opt_vol.as_ref().map_or(0.0, |v| v.delivered_water_m_earth);
    let (climate_regime, ice_coverage, cloud_coverage, polar_ice_lat) = determine_climate_coverage(
        body_type,
        atm_pressure,
        thermal.surface_temp,
        prep.has_water_volatiles,
        post_water,
        ocean_frac,
        current_ice,
        gas_frac,
    );
    let snapshot = PlanetaryClimate {
        surface_temperature_k: thermal.surface_temp as f32,
        equilibrium_temperature_k: thermal.equilibrium_temp as f32,
        greenhouse_delta_k: thermal.greenhouse_delta,
        albedo: reported_planetary_albedo(ocean_frac, ice_coverage, atm_pressure, cloud_coverage),
        ice_coverage_frac: ice_coverage,
        cloud_coverage_frac: cloud_coverage,
        climate_regime,
        polar_ice_cap_latitude_deg: polar_ice_lat,
    };
    (snapshot, ocean_frac, atm_pressure, new_steam_bar)
}

fn write_planetary_climate_outputs(
    commands: &mut Commands,
    body_ent: Entity,
    body_type: BodyType,
    opt_climate: &mut Option<Mut<'_, PlanetaryClimate>>,
    snapshot: PlanetaryClimate,
) {
    if let Some(climate) = opt_climate {
        **climate = snapshot;
    } else if matches!(
        body_type,
        BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet | BodyType::Moon
    ) && let Ok(mut cmd) = commands.get_entity(body_ent)
    {
        cmd.insert(snapshot);
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Planetary climate & biosphere update system"
)]
pub fn update_body_climate_and_biosphere(
    commands: &mut Commands,
    body_ent: Entity,
    b_mass_solar: f64,
    b_radius_au: f64,
    b_body: &CelestialBody,
    comp: &Composition,
    p_temp: &mut Temperature,
    mut opt_vol: Option<&mut VolatileInventory>,
    opt_climate: &mut Option<Mut<'_, PlanetaryClimate>>,
    opt_bio: &mut Option<Mut<'_, BiosphereState>>,
    r: f64,
    star_lum: f64,
    star_temp: f64,
    star_r: f64,
    shockwave_r: f64,
    magnetic_field_gauss: f32,
    prev_steam_bar: f32,
    active_escape_rate: f64,
    dt_yr: f64,
    elapsed_years: f64,
) -> f32 {
    let current_ice = comp.ice_frac as f32;
    let initial_climate_ice = opt_climate.as_ref().map_or(0.0, |c| c.ice_coverage_frac);
    let (init_ocean, init_atm, water_reservoir) = opt_vol.as_ref().map_or((0.0, 0.0, 0.0), |v| {
        (
            v.ocean_coverage_frac,
            v.atmospheric_pressure_bar,
            v.delivered_water_m_earth,
        )
    });
    let init_clouds = opt_climate.as_ref().map_or_else(
        || {
            if init_atm >= 0.02 {
                (0.28 + init_ocean * 0.34).clamp(0.15, 0.90)
            } else {
                0.0
            }
        },
        |c| c.cloud_coverage_frac,
    );
    let star = StellarIrradiance {
        lum_solar: star_lum,
        temp_k: star_temp,
        radius_au: star_r,
    };
    let prep = prepare_greenhouse_inputs(
        b_body.body_type,
        b_mass_solar,
        b_radius_au,
        comp,
        water_reservoir,
        init_atm,
        prev_steam_bar,
        p_temp.0,
        r,
        star,
    );
    let thermal = compute_surface_temperature(
        r,
        star_lum,
        star_temp,
        star_r,
        shockwave_r,
        current_ice,
        initial_climate_ice,
        p_temp.0,
        init_ocean,
        init_clouds,
        water_reservoir,
        prep.effective_atm,
        dt_yr,
    );
    p_temp.0 = thermal.surface_temp;

    let (snapshot, ocean_frac, atm_pressure, new_steam_bar) = finalize_post_thermal_climate(
        &mut opt_vol,
        b_body.body_type,
        b_mass_solar,
        comp,
        thermal,
        &prep,
        active_escape_rate,
    );
    write_planetary_climate_outputs(commands, body_ent, b_body.body_type, opt_climate, snapshot);

    if matches!(
        b_body.body_type,
        BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet | BodyType::Moon
    ) {
        update_body_biosphere(
            commands,
            body_ent,
            b_mass_solar,
            thermal.surface_temp,
            star_temp,
            ocean_frac,
            magnetic_field_gauss,
            atm_pressure,
            opt_bio,
            dt_yr,
            elapsed_years,
        );
    }
    new_steam_bar
}

/// Immediately equilibrates a non-stellar body's `Temperature`, `VolatileInventory`, and
/// `PlanetaryClimate` to its orbital distance `r_au` around `star`.
#[allow(
    clippy::too_many_arguments,
    reason = "Orbit equilibration updates temperature, volatile phase balance, and climate snapshot"
)]
pub fn equilibrate_body_temperature_for_orbit(
    r_au: f64,
    b_mass_solar: f64,
    b_radius_au: f64,
    body_type: BodyType,
    comp: &Composition,
    star: StellarIrradiance,
    temp: &mut Temperature,
    mut opt_vol: Option<&mut VolatileInventory>,
    opt_climate: Option<&mut PlanetaryClimate>,
) -> Option<PlanetaryClimate> {
    if body_type.is_star_or_remnant() || body_type == BodyType::BrownDwarf {
        return None;
    }

    let r = r_au.max(1e-4);
    let current_ice = comp.ice_frac as f32;
    let water_reservoir = opt_vol.as_ref().map_or(0.0, |v| v.delivered_water_m_earth);
    let initial_atm = opt_vol.as_ref().map_or(0.0, |v| v.atmospheric_pressure_bar);
    let prep = prepare_greenhouse_inputs(
        body_type,
        b_mass_solar,
        b_radius_au,
        comp,
        water_reservoir,
        initial_atm,
        0.0,
        temp.0,
        r,
        star,
    );

    let init_ocean = if prep.has_water_volatiles && (255.0..=360.0).contains(&prep.solar_dry_temp) {
        (water_reservoir / 0.0005).clamp(0.0, 0.85) as f32
    } else {
        0.0
    };
    let init_ice = if prep.has_water_volatiles && prep.solar_dry_temp < 255.0 {
        1.0
    } else if prep.has_water_volatiles && prep.solar_dry_temp < 290.0 {
        ((290.0 - prep.solar_dry_temp) / 40.0).clamp(0.0, 0.9) as f32
    } else {
        0.0
    };
    let init_clouds = if prep.effective_atm >= 0.02 {
        (0.28 + init_ocean * 0.34).clamp(0.15, 0.90)
    } else {
        0.0
    };

    let thermal = compute_surface_temperature(
        r,
        star.lum_solar,
        star.temp_k,
        star.radius_au,
        0.0,
        current_ice,
        init_ice,
        0.0,
        init_ocean,
        init_clouds,
        water_reservoir,
        prep.effective_atm,
        1.0,
    );
    temp.0 = thermal.surface_temp;

    let (updated, ..) = finalize_post_thermal_climate(
        &mut opt_vol,
        body_type,
        b_mass_solar,
        comp,
        thermal,
        &prep,
        0.0,
    );
    if let Some(climate) = opt_climate {
        *climate = updated;
    }
    Some(updated)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Biosphere evolution helper requires climate, magnetic, and stellar parameters"
)]
pub(super) fn update_body_biosphere(
    _commands: &mut Commands,
    _body_ent: Entity,
    _b_mass_solar: f64,
    surface_temp: f64,
    star_temp: f64,
    ocean_frac: f32,
    magnetic_field_gauss: f32,
    atm_pressure: f32,
    opt_bio: &mut Option<Mut<'_, BiosphereState>>,
    dt_yr: f64,
    elapsed_years: f64,
) {
    let temp_score = (1.0 - ((surface_temp as f32 - 288.0) / 45.0).powi(2)).clamp(0.0, 1.0);
    let water_score = if ocean_frac > 0.10 && ocean_frac < 0.90 {
        1.0
    } else if ocean_frac >= 0.90 {
        0.75
    } else {
        ocean_frac * 5.0
    };
    let shield_score = (magnetic_field_gauss / 0.20).clamp(0.1, 1.0);
    let atm_score = if (0.2..=3.0).contains(&atm_pressure) {
        1.0
    } else {
        (atm_pressure / 0.2).clamp(0.0, 1.0) * (5.0 / atm_pressure.max(1.0)).clamp(0.0, 1.0)
    };

    let habitability = temp_score * water_score * shield_score * atm_score;

    if let Some(bio) = opt_bio {
        bio.habitability_score = habitability;
        if habitability >= 0.35 {
            bio.biomass_coverage_frac = (bio.biomass_coverage_frac
                + (0.005 * habitability * dt_yr as f32))
                .clamp(0.0, 0.85);
            if bio.emergence_year.is_none() && bio.biomass_coverage_frac > 0.05 {
                bio.emergence_year = Some(elapsed_years);
            }
        } else {
            bio.biomass_coverage_frac = (bio.biomass_coverage_frac - 0.02 * dt_yr as f32).max(0.0);
        }
        bio.oxygen_fraction = (bio.biomass_coverage_frac * 0.24).clamp(0.0, 0.21);
        if star_temp > 100.0 {
            bio.flora_pigment = FloraPigmentClass::from_star_temperature(star_temp);
        }
        if bio.biomass_coverage_frac > 0.02 {
            let spectral_biolum = match bio.flora_pigment {
                FloraPigmentClass::RedDwarfCrimson => 0.85,
                FloraPigmentClass::OrangeDwarfCarotenoid => 0.55,
                FloraPigmentClass::HighUvIndigo => 0.90,
                FloraPigmentClass::SolarChlorophyll => 0.35,
            };
            bio.bioluminescence_intensity =
                (bio.biomass_coverage_frac * spectral_biolum).clamp(0.0, 1.0);
        } else {
            bio.bioluminescence_intensity = 0.0;
        }
    }
}
