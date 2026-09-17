//! Atmospheric entry ablation, shock vaporization, and volatile delivery physics.

use super::types::*;
use crate::simulation::components::Composition;
use crate::utils::constants::*;

/// Computes the astrophysical delivery of volatiles, atmospheric gases, and cratering
/// from an inbound projectile striking a planetary target.
pub fn calculate_impact_delivery(
    bombardment_type: BombardmentType,
    impactor_mass_solar: f64,
    impactor_comp: &Composition,
    v_rel_au_yr: f64,
    target_mass_solar: f64,
    target_radius_au: f64,
    current_atm_pressure_bar: f32,
    target_surface_temp_k: f64,
) -> ImpactDeliveryResult {
    let m_target_earth = (target_mass_solar / EARTH_MASS_SOLAR).max(1e-5);
    let r_target_earth = (target_radius_au / EARTH_RADIUS_AU).max(1e-4);
    let m_impactor_earth = impactor_mass_solar / EARTH_MASS_SOLAR;

    // 1. Calculate impact velocity: v_imp = sqrt(v_rel^2 + v_esc^2)
    let v_rel_km_s = v_rel_au_yr * AU_PER_YR_TO_KM_PER_S;
    let v_esc_km_s = 11.186 * (m_target_earth / r_target_earth).sqrt();
    let v_imp_km_s = (v_rel_km_s * v_rel_km_s + v_esc_km_s * v_esc_km_s)
        .sqrt()
        .clamp(5.0, 75.0);

    // 2. Atmospheric entry ablation efficiency based on ambient scale height / pressure
    let atm_shield = (f64::from(current_atm_pressure_bar) / 1.0).clamp(0.0, 5.0);
    let ablation_fraction = match bombardment_type {
        BombardmentType::VolatileAblationSalvo => 0.95,
        BombardmentType::IcyComet => (0.25 + 0.15 * atm_shield).clamp(0.20, 0.85),
        BombardmentType::CarbonaceousChondrite => (0.20 + 0.10 * atm_shield).clamp(0.15, 0.70),
        BombardmentType::IronAsteroid => 0.05,
    };

    // 3. Impact erosion / shock blow-off factor for low-gravity worlds (Mercury/Moon scale)
    let retention_efficiency = if m_target_earth < 0.08 && v_imp_km_s > 20.0 {
        ((m_target_earth / 0.08) * (20.0 / v_imp_km_s)).clamp(0.25, 1.0)
    } else {
        1.0
    };

    // 4. Volatile speciation (Water vs Organics vs Gases)
    let delivered_water_raw = m_impactor_earth * impactor_comp.ice_frac * retention_efficiency;
    let delivered_organics = m_impactor_earth * impactor_comp.organics_frac * retention_efficiency;

    // Vaporization vs liquid deposition: High velocity or surface above boiling point (373 K) flashes to steam
    let is_vaporizing = v_imp_km_s > 18.0 || target_surface_temp_k >= 373.0;
    let steam_fraction = if bombardment_type == BombardmentType::VolatileAblationSalvo {
        0.80
    } else if is_vaporizing {
        0.70
    } else {
        0.20
    };

    let steam_vapor = delivered_water_raw * steam_fraction;
    let liquid_water = delivered_water_raw * (1.0 - steam_fraction);

    // 5. Degassing of Carbon Dioxide and Nitrogen from volatile & organic impact pyrolysis
    let (delta_co2, delta_n2) = match bombardment_type {
        BombardmentType::IcyComet => {
            let co2 = (delivered_organics * 200.0 + delivered_water_raw * 30.0) as f32;
            let n2 = (delivered_organics * 80.0) as f32;
            (co2, n2)
        }
        BombardmentType::CarbonaceousChondrite => {
            let co2 = (delivered_organics * 650.0) as f32;
            let n2 = (delivered_organics * 250.0) as f32;
            (co2, n2)
        }
        BombardmentType::VolatileAblationSalvo => {
            let co2 = (delivered_organics * 400.0 + delivered_water_raw * 50.0) as f32;
            let n2 = (delivered_organics * 150.0) as f32;
            (co2, n2)
        }
        BombardmentType::IronAsteroid => (0.01, 0.005),
    };

    // 6. Impact Dust & Core Heating
    let mass_ratio = (m_impactor_earth / m_target_earth).clamp(1e-7, 1.0);
    let (delta_dust, core_boost_k, crater_angular_radius) = match bombardment_type {
        BombardmentType::VolatileAblationSalvo => {
            // Minimal dust and cratering
            (0.05f32, 5.0, 0.03f32)
        }
        BombardmentType::IronAsteroid => {
            let dust = (mass_ratio * 40.0).clamp(0.2, 3.5) as f32;
            let core_heat = (800.0 * (v_imp_km_s / 15.0) * mass_ratio.cbrt()).clamp(100.0, 2500.0);
            let crater_radius = ((mass_ratio).cbrt() as f32 * 0.45).clamp(0.08, 0.60);
            (dust, core_heat, crater_radius)
        }
        _ => {
            let dust = (mass_ratio * 25.0).clamp(0.1, 2.0) as f32;
            let core_heat = (300.0 * (v_imp_km_s / 15.0) * mass_ratio.cbrt()).clamp(20.0, 800.0);
            let crater_radius = ((mass_ratio).cbrt() as f32 * 0.35).clamp(0.05, 0.45);
            (dust, core_heat, crater_radius)
        }
    };

    ImpactDeliveryResult {
        liquid_water_delivered_m_earth: liquid_water * (1.0 - ablation_fraction * 0.5),
        steam_vapor_delivered_m_earth: steam_vapor + liquid_water * (ablation_fraction * 0.5),
        delta_co2_bar: delta_co2.clamp(0.0, 20.0),
        delta_nitrogen_bar: delta_n2.clamp(0.0, 15.0),
        delta_dust_opacity: delta_dust,
        core_temp_boost_k: core_boost_k,
        crater_angular_radius,
        impact_speed_km_s: v_imp_km_s,
    }
}
