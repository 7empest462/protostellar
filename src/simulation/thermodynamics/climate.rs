//! Planetary Climate, Surface Temperature, Volatile Condensation, and Biosphere Genesis.

use bevy::prelude::*;

use crate::simulation::components::*;

struct SurfaceTemperatureResult {
    surface_temp: f64,
    equilibrium_temp: f64,
    greenhouse_delta: f32,
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
    atm_pressure: f32,
    dt_yr: f64,
) -> SurfaceTemperatureResult {
    // Bare surface albedo: dark oceans (0.08) vs rocky land (0.28)
    let bare_albedo = 0.08 * ocean_frac + 0.28 * (1.0 - ocean_frac);
    // Dynamic ice-albedo positive feedback loop: ice/snow reflects ~68% of insolation
    let effective_ice = current_ice.max(climate_ice_frac);
    let surface_albedo = (1.0 - effective_ice) * bare_albedo + effective_ice * 0.68;
    // Atmospheric and cloud reflection (clouds add ~10% planetary reflection)
    let cloud_refl = if atm_pressure > 0.05 { 0.10 } else { 0.0 };
    let albedo = (surface_albedo + cloud_refl).clamp(0.12, 0.78);

    let equilibrium_temp =
        (star_temp * (star_r / (2.0 * r)).sqrt() * (1.0 - f64::from(albedo)).powf(0.25))
            * star_lum.powf(0.25);

    let shock_boost = if shockwave_r > 0.0 && (r - shockwave_r).abs() < 2.5 {
        800.0 * (1.0 - (r - shockwave_r).abs() / 2.5)
    } else {
        0.0
    };

    let mut greenhouse_delta = if atm_pressure > 0.01 {
        33.0 * (atm_pressure / 1.0).powf(0.28) * (0.80 + ocean_frac * 0.28)
    } else {
        0.0
    };

    if equilibrium_temp + f64::from(greenhouse_delta) > 350.0 && ocean_frac > 0.05 {
        greenhouse_delta = (greenhouse_delta * 3.5).min(450.0);
    }

    let target_temp =
        (equilibrium_temp + f64::from(greenhouse_delta) + shock_boost).clamp(30.0, 5000.0);
    let surface_temp = if current_temp > target_temp + 1.0 {
        // Radiative cooling of magma ocean / impact thermal surplus towards equilibrium
        let cool_rate = 0.08 * (current_temp / 1000.0).powi(3).clamp(0.01, 15.0);
        let k_cool = (1.0 - (-cool_rate * dt_yr).exp()).clamp(0.0, 1.0);
        (current_temp + (target_temp - current_temp) * k_cool).max(target_temp)
    } else {
        target_temp
    };

    SurfaceTemperatureResult {
        surface_temp,
        equilibrium_temp,
        greenhouse_delta,
    }
}

fn update_volatile_condensation(
    opt_vol: &mut Option<&mut VolatileInventory>,
    has_water_volatiles: bool,
    current_ice: f32,
    surface_temp: f64,
) -> (f32, f32) {
    let mut ocean_frac = 0.0;
    let mut atm_pressure = 0.0;
    if let Some(ref mut vol) = opt_vol {
        if has_water_volatiles {
            let max_ocean = if vol.delivered_water_m_earth > 1e-6 {
                ((vol.delivered_water_m_earth / 0.0006) * 0.71).clamp(0.0, 0.85) as f32
            } else {
                vol.ocean_coverage_frac
                    .max((current_ice * 3.0).clamp(0.0, 0.85))
            };

            let condensation_frac = if surface_temp > 380.0 {
                0.0
            } else if surface_temp < 340.0 {
                1.0
            } else {
                ((380.0 - surface_temp as f32) / 40.0).clamp(0.0, 1.0)
            };

            vol.ocean_coverage_frac = max_ocean * condensation_frac;
            ocean_frac = vol.ocean_coverage_frac;

            let steam_pressure = max_ocean * (1.0 - condensation_frac) * 12.0;
            atm_pressure = vol.atmospheric_pressure_bar.max(steam_pressure);
        } else {
            vol.ocean_coverage_frac = 0.0;
        }
    }
    (ocean_frac, atm_pressure)
}

fn determine_climate_coverage(
    body_type: BodyType,
    atm_pressure: f32,
    surface_temp: f64,
    has_water_volatiles: bool,
    ocean_frac: f32,
    current_ice: f32,
    comp_gas_frac: f32,
) -> (ClimateRegime, f32, f32, f32) {
    let climate_regime = if matches!(body_type, BodyType::GasGiant | BodyType::IceGiant) {
        ClimateRegime::GasGiantEnvelope
    } else if atm_pressure < 0.02 {
        ClimateRegime::AirlessVacuum
    } else if surface_temp < 250.0 {
        ClimateRegime::SnowballIceAge
    } else if surface_temp > 360.0 {
        ClimateRegime::RunawayVenusian
    } else {
        ClimateRegime::TemperateHabitable
    };

    let has_water = has_water_volatiles && (ocean_frac > 0.01 || current_ice > 0.005);
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
            _ => 0.0,
        }
    } else {
        0.0
    };

    let polar_ice_lat = (90.0 * (1.0 - ice_coverage)).clamp(0.0, 90.0);

    let cloud_coverage = if atm_pressure > 0.05 && (has_water_volatiles || comp_gas_frac > 0.02) {
        (0.35 + ocean_frac * 0.40).clamp(0.1, 0.95)
    } else {
        0.0
    };

    (climate_regime, ice_coverage, cloud_coverage, polar_ice_lat)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Planetary climate & biosphere update system"
)]
pub fn update_body_climate_and_biosphere(
    commands: &mut Commands,
    body_ent: Entity,
    b_mass_solar: f64,
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
    dt_yr: f64,
    elapsed_years: f64,
) {
    let current_ice = comp.ice_frac as f32;
    let initial_climate_ice = opt_climate.as_ref().map_or(0.0, |c| c.ice_coverage_frac);
    let (initial_ocean_frac, initial_atm_pressure) = if let Some(ref vol) = opt_vol {
        (vol.ocean_coverage_frac, vol.atmospheric_pressure_bar)
    } else {
        (0.0, 0.0)
    };

    let thermal = compute_surface_temperature(
        r,
        star_lum,
        star_temp,
        star_r,
        shockwave_r,
        current_ice,
        initial_climate_ice,
        p_temp.0,
        initial_ocean_frac,
        initial_atm_pressure,
        dt_yr,
    );
    p_temp.0 = thermal.surface_temp;

    let has_water_volatiles = current_ice > 0.001
        || opt_vol
            .as_ref()
            .is_some_and(|v| v.delivered_water_m_earth > 1e-6);

    let (ocean_frac, atm_pressure) = update_volatile_condensation(
        &mut opt_vol,
        has_water_volatiles,
        current_ice,
        thermal.surface_temp,
    );

    let (climate_regime, ice_coverage, cloud_coverage, polar_ice_lat) = determine_climate_coverage(
        b_body.body_type,
        atm_pressure,
        thermal.surface_temp,
        has_water_volatiles,
        ocean_frac,
        current_ice,
        comp.gas_frac as f32,
    );

    let bare_albedo = 0.08 * ocean_frac + 0.28 * (1.0 - ocean_frac);
    let final_surface_albedo = (1.0 - ice_coverage) * bare_albedo + ice_coverage * 0.68;
    let cloud_refl = if atm_pressure > 0.05 { 0.10 } else { 0.0 };
    let reported_albedo = (final_surface_albedo + cloud_refl).clamp(0.12, 0.78);

    if let Some(ref mut climate) = opt_climate {
        climate.surface_temperature_k = thermal.surface_temp as f32;
        climate.equilibrium_temperature_k = thermal.equilibrium_temp as f32;
        climate.greenhouse_delta_k = thermal.greenhouse_delta;
        climate.albedo = reported_albedo;
        climate.ice_coverage_frac = ice_coverage;
        climate.cloud_coverage_frac = cloud_coverage;
        climate.climate_regime = climate_regime;
        climate.polar_ice_cap_latitude_deg = polar_ice_lat;
    } else if matches!(
        b_body.body_type,
        BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
    ) {
        if let Ok(mut cmd) = commands.get_entity(body_ent) {
            cmd.insert(PlanetaryClimate {
                surface_temperature_k: thermal.surface_temp as f32,
                equilibrium_temperature_k: thermal.equilibrium_temp as f32,
                greenhouse_delta_k: thermal.greenhouse_delta,
                albedo: reported_albedo,
                ice_coverage_frac: ice_coverage,
                cloud_coverage_frac: cloud_coverage,
                climate_regime,
                polar_ice_cap_latitude_deg: polar_ice_lat,
            });
        }
    }

    if matches!(
        b_body.body_type,
        BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
    ) {
        update_body_biosphere(
            commands,
            body_ent,
            b_mass_solar,
            thermal.surface_temp,
            ocean_frac,
            magnetic_field_gauss,
            atm_pressure,
            opt_bio,
            dt_yr,
            elapsed_years,
        );
    }
}

fn update_body_biosphere(
    _commands: &mut Commands,
    _body_ent: Entity,
    _b_mass_solar: f64,
    surface_temp: f64,
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

    if let Some(ref mut bio) = opt_bio {
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
    }
}
