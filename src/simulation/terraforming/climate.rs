//! Dynamic climate, greenhouse equilibrium, condensation, and impact winter relaxation.

use super::types::*;
use crate::simulation::components::*;

/// Updates dynamic phase transitions (condensation vs evaporation), multi-species greenhouse
/// forcing, and impact dust decay for a world undergoing terraforming.
pub fn update_body_terraforming_climate(
    vol: &mut VolatileInventory,
    terra: &mut TerraformingAtmosphere,
    climate: &mut PlanetaryClimate,
    comp: &mut Composition,
    planet_mass_solar: f64,
    dt_yr: f64,
) {
    // 1. Impact Dust Relaxation (decay half-life ~ 20 simulation years)
    if terra.impact_dust_optical_depth > 0.001 {
        let decay = (-dt_yr / 20.0).exp() as f32;
        terra.impact_dust_optical_depth *= decay;
    } else {
        terra.impact_dust_optical_depth = 0.0;
    }

    // 2. Compute total atmospheric pressure from true hydrostatic column weight
    let planet_radius_au = crate::simulation::accretion::impact_regimes::radius_from_mass_density(
        planet_mass_solar.max(1e-12),
        comp.average_density(),
    );
    let p_co2 = terra.co2_pressure_bar.max(0.0);
    let p_n2 = terra.nitrogen_pressure_bar.max(0.0);
    let p_steam_init = crate::simulation::thermodynamics::hydrostatic_pressure_bar(
        terra.atmospheric_water_m_earth,
        planet_mass_solar,
        planet_radius_au,
    );
    let p_total_init = (p_co2 + p_n2 + p_steam_init).max(0.0001);

    // 3. Clausius-Clapeyron phase transition conserving total liquid + vapor H2O mass
    let t_surf = f64::from(climate.surface_temperature_k);
    let p_sat = crate::simulation::thermodynamics::water_saturation_vapor_pressure_bar(t_surf);
    let t_boil = crate::simulation::thermodynamics::water_boiling_point_k(p_total_init);
    let m_sat_earth = crate::simulation::thermodynamics::gas_mass_for_pressure_bar(
        p_sat,
        planet_mass_solar,
        planet_radius_au,
    );
    let total_water =
        (terra.surface_liquid_water_m_earth + terra.atmospheric_water_m_earth).max(0.0);

    let target_vapor = if t_surf >= 647.096 {
        total_water
    } else if t_surf < 273.16 {
        m_sat_earth.min(total_water)
    } else if p_total_init < 0.006_116_57 {
        total_water
    } else if t_surf >= t_boil || terra.atmospheric_water_m_earth > m_sat_earth {
        m_sat_earth.min(total_water)
    } else {
        (terra.atmospheric_water_m_earth + (m_sat_earth - terra.atmospheric_water_m_earth) * 0.25)
            .min(total_water)
    };

    // Convective rainout of supersaturated steam acts much faster (~weeks) than magma/boiling relaxation
    let rate = if target_vapor < terra.atmospheric_water_m_earth {
        20.0
    } else {
        1.5
    };
    let k_phase = (1.0 - (-rate * dt_yr.max(0.0)).exp()).clamp(0.0, 1.0);

    let new_vapor = (terra.atmospheric_water_m_earth
        + (target_vapor - terra.atmospheric_water_m_earth) * k_phase)
        .clamp(0.0, total_water);
    terra.atmospheric_water_m_earth = new_vapor;
    terra.surface_liquid_water_m_earth = (total_water - new_vapor).max(0.0);

    // 4. Update Surface Liquid Ocean Coverage & Post-Phase Pressures
    let p_steam = crate::simulation::thermodynamics::hydrostatic_pressure_bar(
        terra.atmospheric_water_m_earth,
        planet_mass_solar,
        planet_radius_au,
    );
    let p_total = (p_co2 + p_n2 + p_steam).max(0.0001);

    let ocean_cov = if p_total >= 0.006_116_57 && (273.16..647.096).contains(&t_surf) {
        (terra.surface_liquid_water_m_earth / 0.0006).clamp(0.0, 0.92) as f32
    } else {
        0.0
    };
    vol.delivered_water_m_earth = total_water;
    vol.ocean_coverage_frac = ocean_cov;
    vol.atmospheric_pressure_bar = p_total;

    // 5. Multi-Gas Radiative Greenhouse Forcing
    let base_gh = 33.0 * (p_total / 1.0).powf(0.28);
    let co2_multiplier = 1.0 + (p_co2 / (p_total + 0.01)) * 1.5;
    let steam_greenhouse = 135.0 * (1.0 - (-p_steam / 0.40).exp());

    let greenhouse_delta = base_gh * co2_multiplier + steam_greenhouse;

    // 6. Impact Winter Anti-Greenhouse / Stratospheric Dust Cooling reduces effective insolation
    let dust_cooling = 45.0 * (1.0 - (-terra.impact_dust_optical_depth).exp());
    let effective_eq_temp = (climate.equilibrium_temperature_k - dust_cooling).max(20.0);

    // 7. Feedback onto climate state and regime
    climate.greenhouse_delta_k = greenhouse_delta;
    climate.surface_temperature_k = (effective_eq_temp + greenhouse_delta).max(20.0);

    // Update climate regime classification
    if p_total < 0.02 {
        climate.climate_regime = ClimateRegime::AirlessVacuum;
        climate.cloud_coverage_frac = 0.0;
    } else if climate.surface_temperature_k > 360.0 {
        climate.climate_regime = ClimateRegime::RunawayVenusian;
        if p_steam > 0.10 {
            climate.cloud_coverage_frac = (0.75 + (p_steam / 2.0).clamp(0.0, 0.23)).min(0.98);
            climate.albedo = 0.72;
        } else {
            // Hot dry greenhouse (dense CO2 / arid desert) — never TemperateHabitable
            climate.cloud_coverage_frac = (0.12 + (p_co2 / 25.0).clamp(0.0, 0.60)).min(0.72);
            climate.albedo = (0.28 + climate.cloud_coverage_frac * 0.45).clamp(0.28, 0.70);
        }
        climate.ice_coverage_frac = 0.0;
    } else if climate.surface_temperature_k < 260.0 {
        let has_water_or_ice = total_water > 1e-6 || comp.ice_frac > 0.001;
        climate.climate_regime = ClimateRegime::SnowballIceAge;
        climate.albedo = if has_water_or_ice { 0.65 } else { 0.30 };
        climate.ice_coverage_frac = if has_water_or_ice { 1.0 } else { 0.0 };
        climate.cloud_coverage_frac = if has_water_or_ice { 0.25 } else { 0.05 };
    } else {
        climate.climate_regime = ClimateRegime::TemperateHabitable;
        climate.albedo = 0.30;
        climate.ice_coverage_frac = if climate.surface_temperature_k < 285.0 {
            ((285.0 - climate.surface_temperature_k) / 25.0 * 0.30).clamp(0.0, 0.30)
        } else {
            0.0
        };
        climate.cloud_coverage_frac = (0.35 + ocean_cov * 0.30).clamp(0.15, 0.75);
    }

    // Keep composition ice fraction in sync with delivered volatiles
    let water_mass_frac = (total_water * crate::utils::constants::EARTH_MASS_SOLAR
        / planet_mass_solar.max(1e-9))
    .clamp(0.0, 0.45);
    if water_mass_frac > comp.ice_frac {
        comp.ice_frac = (comp.ice_frac * 0.95 + water_mass_frac * 0.05).clamp(0.0, 0.50);
        *comp = comp.normalized();
    }
}
