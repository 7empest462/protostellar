//! Thermodynamics, Hayashi Track Protostellar Evolution, Core Dynamos, Planetary Climate, and Biosphere Genesis.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Event triggered when the central protostar's core reaches 10,000,000 K and ignites hydrogen fusion.
#[derive(Message, Debug, Clone)]
pub struct StarIgnitionEvent {
    pub star_entity: Entity,
    pub star_mass: f64,
    pub luminosity_l_sun: f64,
    pub surface_temp_kelvin: f64,
}

/// Updates stellar thermodynamics, core heating, planetary dynamos, greenhouse atmospheres, and biospheres.
pub fn update_thermodynamics(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut config: ResMut<SimulationConfig>,
    mut ignition_events: MessageWriter<StarIgnitionEvent>,
    mut star_query: Query<
        (
            Entity,
            &mut Mass,
            &mut Radius,
            &mut Temperature,
            &mut Luminosity,
            &mut IgnitionState,
            &mut CelestialBody,
        ),
        With<CentralStar>,
    >,
    mut bodies_query: Query<
        (
            Entity,
            &Mass,
            &SimPosition,
            &mut Temperature,
            &mut Composition,
            &CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&SpinState>,
            Option<&mut VolatileInventory>,
            Option<&mut PlanetaryClimate>,
            Option<&mut BiosphereState>,
        ),
        Without<CentralStar>,
    >,
) {
    if (!config.enable_thermodynamics || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr);

    // 1. Process Protostellar Core Heating & Ignition
    for (entity, mass, mut radius, mut temp, mut lum, mut ignition, mut body) in
        star_query.iter_mut()
    {
        if !ignition.is_ignited {
            // Core temperature heats up via gravitational Kelvin-Helmholtz contraction
            let heating_rate_per_yr = 1.0e4 * mass.0; // Naturally ignites around ~1000 years
            ignition.core_temperature += heating_rate_per_yr * dt_yr;

            let ignition_threshold = 1.0e7; // 10 Million Kelvin (Hydrogen P-P Fusion)
            ignition.fusion_fraction =
                (ignition.core_temperature / ignition_threshold).clamp(0.0, 1.0) as f32;

            // Surface temperature increases as star contracts along Hayashi/Henyey track
            let target_surface_temp =
                3200.0 + (5778.0 - 3200.0) * (ignition.fusion_fraction as f64);
            temp.0 = target_surface_temp;

            // Radius contracts down toward main-sequence equilibrium (1.0 Solar Radius)
            let target_radius =
                SOLAR_RADIUS_AU * (1.0 + 2.0 * (1.0 - ignition.fusion_fraction as f64));
            radius.0 = target_radius;

            // Check if ignition threshold reached!
            if ignition.core_temperature >= ignition_threshold {
                ignition.is_ignited = true;
                ignition.fusion_fraction = 1.0;
                body.body_type = BodyType::MainSequenceStar;
                body.name = "The Star (Main Sequence)".to_string();

                // Main Sequence Mass-Luminosity relation: L/L_sun ~ (M/M_sun)^3.5
                let main_seq_lum = mass.0.powf(3.5);
                lum.0 = main_seq_lum;
                temp.0 = 5778.0 * mass.0.powf(0.505); // Solar effective temp ~ 5778 K
                radius.0 = SOLAR_RADIUS_AU;

                ignition_events.write(StarIgnitionEvent {
                    star_entity: entity,
                    star_mass: mass.0,
                    luminosity_l_sun: lum.0,
                    surface_temp_kelvin: temp.0,
                });
            }
        } else {
            // Star is ignited: expand radiation pressure shockwave & photoevaporate gas disk
            let blast_speed = 3.5; // Smooth outward stellar wind (AU/yr)
            ignition.shockwave_radius += blast_speed * dt_yr;

            // Progressive photo-evaporative clearance over 15,000 years
            let time_decay = (1.0 - (sim_time.elapsed_years / 15_000.0)).clamp(0.0, 1.0) as f32;
            config.gas_density_scale = time_decay;
        }

        // 2. Update Disk Body Temperatures & Planetary Thermal Processing
        let star_lum = lum.0;
        let star_temp = temp.0;
        let shockwave_r = ignition.shockwave_radius;

        for (
            body_ent,
            b_mass,
            pos,
            mut p_temp,
            mut comp,
            b_body,
            mut opt_diff,
            opt_spin,
            mut opt_vol,
            mut opt_climate,
            mut opt_bio,
        ) in bodies_query.iter_mut()
        {
            let r = pos.0.length().max(0.1);
            let period_hrs = opt_spin.map(|s| s.rotation_period_hours).unwrap_or(24.0);

            // A. Core Dynamo Convection & Magnetic Field Generation
            let mut magnetic_field_gauss = 0.0f32;
            if let Some(ref mut diff) = opt_diff {
                if diff.is_differentiated {
                    // Molten core convective dynamo physics
                    if diff.core_temp_k > 1800.0 {
                        let temp_factor = ((diff.core_temp_k - 1800.0) / 2000.0).clamp(0.0, 1.5);
                        let spin_factor = (24.0 / period_hrs.max(1.0)).sqrt().clamp(0.2, 3.0);
                        let core_mass_frac = (diff.core_radius_au
                            / (diff.mantle_radius_au.max(1e-5)))
                        .powi(3)
                        .clamp(0.05, 0.60);

                        let b_gauss =
                            (0.35 * core_mass_frac.sqrt() * spin_factor * temp_factor.powf(0.33))
                                .clamp(0.0, 5.0);

                        diff.magnetic_field_gauss = b_gauss;
                        magnetic_field_gauss = b_gauss as f32;
                    } else {
                        diff.magnetic_field_gauss = 0.0; // Core solidified (like Mars)
                    }

                    // Gradual core cooling over geological time (~1.5 K per kyr)
                    diff.core_temp_k = (diff.core_temp_k - (1.5e-3 * dt_yr)).max(300.0);
                }
            }

            // B. Unshielded Solar Wind Atmospheric Stripping
            if ignition.is_ignited && magnetic_field_gauss < 0.10 && r < 3.0 {
                if let Some(ref mut vol) = opt_vol {
                    let strip_rate = (0.0002 * (1.0 / (r * r)) * dt_yr) as f32;
                    vol.atmospheric_pressure_bar =
                        (vol.atmospheric_pressure_bar - strip_rate).max(0.0);
                }
            }

            // C. Coupled Greenhouse & Ice-Albedo Radiative Balance
            let current_ice = comp.ice_frac as f32;
            let albedo = (0.28 * (1.0 - current_ice) + 0.65 * current_ice).clamp(0.15, 0.75);

            let equilibrium_temp =
                (star_temp * (radius.0 / (2.0 * r)).sqrt() * (1.0 - albedo as f64).powf(0.25))
                    * star_lum.powf(0.25);

            let shock_boost = if shockwave_r > 0.0 && (r - shockwave_r).abs() < 2.5 {
                800.0 * (1.0 - (r - shockwave_r).abs() / 2.5)
            } else {
                0.0
            };

            let atm_pressure = opt_vol
                .as_ref()
                .map(|v| v.atmospheric_pressure_bar)
                .unwrap_or(0.0);
            let ocean_frac = opt_vol
                .as_ref()
                .map(|v| v.ocean_coverage_frac)
                .unwrap_or(0.0);

            // Greenhouse heating model
            let mut greenhouse_delta = if atm_pressure > 0.01 {
                33.0 * (atm_pressure / 1.0).powf(0.28) * (1.0 + ocean_frac * 0.25)
            } else {
                0.0
            };

            // Runaway greenhouse trigger if oceans boil (T_eq + GH > 350 K with oceans)
            if equilibrium_temp + greenhouse_delta as f64 > 350.0 && ocean_frac > 0.05 {
                // Water vapor surges into atmosphere, driving runaway greenhouse
                greenhouse_delta = (greenhouse_delta * 3.5).min(450.0);
            }

            let surface_temp =
                (equilibrium_temp + greenhouse_delta as f64 + shock_boost).clamp(30.0, 5000.0);
            p_temp.0 = surface_temp;

            // Classify Climate Regime
            let climate_regime =
                if matches!(b_body.body_type, BodyType::GasGiant | BodyType::IceGiant) {
                    ClimateRegime::GasGiantEnvelope
                } else if atm_pressure < 0.02 {
                    ClimateRegime::AirlessVacuum
                } else if surface_temp < 260.0 {
                    ClimateRegime::SnowballIceAge
                } else if surface_temp > 360.0 {
                    ClimateRegime::RunawayVenusian
                } else {
                    ClimateRegime::TemperateHabitable
                };

            let ice_coverage = match climate_regime {
                ClimateRegime::SnowballIceAge => 1.0,
                ClimateRegime::TemperateHabitable => {
                    ((320.0 - surface_temp as f32) / 60.0).clamp(0.05, 0.40)
                }
                _ => 0.0,
            };

            let cloud_coverage = if atm_pressure > 0.05 {
                (0.35 + ocean_frac * 0.40).clamp(0.1, 0.95)
            } else {
                0.0
            };

            // Update or Insert PlanetaryClimate component
            if let Some(ref mut climate) = opt_climate {
                climate.surface_temperature_k = surface_temp as f32;
                climate.equilibrium_temperature_k = equilibrium_temp as f32;
                climate.greenhouse_delta_k = greenhouse_delta;
                climate.albedo = albedo;
                climate.ice_coverage_frac = ice_coverage;
                climate.cloud_coverage_frac = cloud_coverage;
                climate.climate_regime = climate_regime;
            } else if matches!(
                b_body.body_type,
                BodyType::TerrestrialPlanet | BodyType::Protoplanet
            ) {
                if let Ok(mut cmd) = commands.get_entity(body_ent) {
                    cmd.insert(PlanetaryClimate {
                        surface_temperature_k: surface_temp as f32,
                        equilibrium_temperature_k: equilibrium_temp as f32,
                        greenhouse_delta_k: greenhouse_delta,
                        albedo,
                        ice_coverage_frac: ice_coverage,
                        cloud_coverage_frac: cloud_coverage,
                        climate_regime,
                    });
                }
            }

            // D. Biosphere Habitability & Life Colonization Engine
            if matches!(
                b_body.body_type,
                BodyType::TerrestrialPlanet | BodyType::Protoplanet
            ) {
                // Habitability index calculation
                let temp_score =
                    (1.0 - ((surface_temp as f32 - 288.0) / 45.0).powi(2)).clamp(0.0, 1.0);
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
                    (atm_pressure / 0.2).clamp(0.0, 1.0)
                        * (5.0 / atm_pressure.max(1.0)).clamp(0.0, 1.0)
                };

                let habitability = temp_score * water_score * shield_score * atm_score;

                if let Some(ref mut bio) = opt_bio {
                    bio.habitability_score = habitability;
                    if habitability >= 0.35 {
                        // Biological growth & photosynthesis
                        bio.biomass_coverage_frac = (bio.biomass_coverage_frac
                            + (0.005 * habitability * dt_yr as f32))
                            .clamp(0.0, 0.85);
                        bio.oxygen_fraction = (bio.biomass_coverage_frac * 0.24).clamp(0.0, 0.21);
                        if bio.emergence_year.is_none() && bio.biomass_coverage_frac > 0.05 {
                            bio.emergence_year = Some(sim_time.elapsed_years);
                        }
                    } else {
                        // Biosphere die-back in hostile climate
                        bio.biomass_coverage_frac =
                            (bio.biomass_coverage_frac - 0.02 * dt_yr as f32).max(0.0);
                        bio.oxygen_fraction = (bio.biomass_coverage_frac * 0.24).clamp(0.0, 0.21);
                    }
                } else if habitability >= 0.45 && b_mass.0 >= EARTH_MASS_SOLAR * 0.15 {
                    if let Ok(mut cmd) = commands.get_entity(body_ent) {
                        cmd.insert(BiosphereState {
                            habitability_score: habitability,
                            biomass_coverage_frac: 0.05,
                            oxygen_fraction: 0.01,
                            emergence_year: Some(sim_time.elapsed_years),
                        });
                    }
                }
            }

            // E. Photoevaporation & Volatile Ice Sublimation behind Shockwave
            if shockwave_r > r && r < 2.7 && comp.ice_frac > 0.001 {
                let sublimated = (comp.ice_frac * 0.15 * dt_yr).min(comp.ice_frac);
                comp.ice_frac -= sublimated;
                comp.silicate_frac += sublimated * 0.7;
                comp.metal_frac += sublimated * 0.3;

                let sum = comp.silicate_frac
                    + comp.metal_frac
                    + comp.ice_frac
                    + comp.organics_frac
                    + comp.gas_frac;
                if sum > 0.0 {
                    comp.silicate_frac /= sum;
                    comp.metal_frac /= sum;
                    comp.ice_frac /= sum;
                    comp.organics_frac /= sum;
                    comp.gas_frac /= sum;
                }
            }
        }
    }
}
