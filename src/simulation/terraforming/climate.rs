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
    dt_yr: f64,
) {
    // 1. Impact Dust Relaxation (decay half-life ~ 20 simulation years)
    if terra.impact_dust_optical_depth > 0.001 {
        let decay = (-dt_yr / 20.0).exp() as f32;
        terra.impact_dust_optical_depth *= decay;
    } else {
        terra.impact_dust_optical_depth = 0.0;
    }

    // 2. Compute total atmospheric pressure including water vapor
    let p_co2 = terra.co2_pressure_bar;
    let p_n2 = terra.nitrogen_pressure_bar;
    let p_steam = (terra.atmospheric_water_m_earth * 250.0) as f32;
    let p_total = (p_co2 + p_n2 + p_steam).max(0.0001);

    // 3. Clausius-Clapeyron Boiling Point & Condensation vs Evaporation
    let t_surf = climate.surface_temperature_k;
    let t_boil = 373.15 * (p_total / 1.0).powf(0.095);

    let k_phase = (1.0 - (-1.5 * dt_yr).exp()).clamp(0.0, 1.0);

    if t_surf < t_boil {
        // Temperature is below boiling point: atmospheric steam condenses into surface oceans
        if terra.atmospheric_water_m_earth > 1e-6 {
            let condensed = terra.atmospheric_water_m_earth * k_phase;
            terra.atmospheric_water_m_earth -= condensed;
            terra.surface_liquid_water_m_earth += condensed;
        }
    } else {
        // Temperature exceeds boiling point: surface oceans flash-evaporate into steam!
        if terra.surface_liquid_water_m_earth > 1e-6 {
            let boiled = terra.surface_liquid_water_m_earth * k_phase;
            terra.surface_liquid_water_m_earth -= boiled;
            terra.atmospheric_water_m_earth += boiled;
        }
    }

    // 4. Update Surface Liquid Ocean Coverage & Post-Phase Pressures
    let p_steam = (terra.atmospheric_water_m_earth * 250.0) as f32;
    let p_total = (p_co2 + p_n2 + p_steam).max(0.0001);

    let total_water = terra.surface_liquid_water_m_earth + terra.atmospheric_water_m_earth;
    let ocean_cov = (terra.surface_liquid_water_m_earth / 0.0006).clamp(0.0, 0.92) as f32;
    vol.delivered_water_m_earth = total_water;
    vol.ocean_coverage_frac = ocean_cov;
    vol.atmospheric_pressure_bar = p_total;

    // 5. Multi-Gas Radiative Greenhouse Forcing
    let base_gh = 33.0 * (p_total / 1.0).powf(0.28);
    let co2_multiplier = 1.0 + (p_co2 / (p_total + 0.01)) * 1.5;
    let steam_greenhouse = 135.0 * (1.0 - (-p_steam / 0.40).exp());

    let mut greenhouse_delta = base_gh * co2_multiplier + steam_greenhouse;

    // 6. Impact Winter Anti-Greenhouse / Dust Cooling
    let dust_cooling = 45.0 * (1.0 - (-terra.impact_dust_optical_depth).exp());
    greenhouse_delta = (greenhouse_delta - dust_cooling).max(0.0);

    // 7. Feedback onto climate state and regime
    climate.greenhouse_delta_k = greenhouse_delta;
    climate.surface_temperature_k =
        (climate.equilibrium_temperature_k + greenhouse_delta).max(20.0);

    // Update climate regime classification
    if p_total < 0.02 {
        climate.climate_regime = ClimateRegime::AirlessVacuum;
    } else if climate.surface_temperature_k > 360.0 && p_steam > 0.10 {
        climate.climate_regime = ClimateRegime::RunawayVenusian;
    } else if climate.surface_temperature_k < 260.0 {
        climate.climate_regime = ClimateRegime::SnowballIceAge;
        climate.albedo = 0.65;
        climate.ice_coverage_frac = 1.0;
    } else {
        climate.climate_regime = ClimateRegime::TemperateHabitable;
        climate.albedo = 0.30;
        climate.ice_coverage_frac = if climate.surface_temperature_k < 285.0 {
            ((285.0 - climate.surface_temperature_k) / 25.0 * 0.30).clamp(0.0, 0.30)
        } else {
            0.0
        };
    }

    // Keep composition ice fraction in sync with delivered volatiles
    let water_mass_frac = (total_water * crate::utils::constants::EARTH_MASS_SOLAR
        / (comp.ice_frac * 0.001 + 1e-9))
        .clamp(0.0, 0.45);
    if water_mass_frac > comp.ice_frac {
        comp.ice_frac = (comp.ice_frac * 0.95 + water_mass_frac * 0.05).clamp(0.0, 0.50);
    }
}
