//! Systems and analytical models for continental drift, Wilson supercontinent cycles,
//! ocean oxidation, and geological epoch scrubbing.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::types::*;

/// Calculates the continental drift Wilson cycle phase in [0.0, 1.0].
///
/// Earth experiences a ~450-500 Myr supercontinent cycle (Kenorland -> Columbia -> Rodinia -> Pangea -> Pangea Ultima).
pub fn calculate_continental_drift_phase(age_gyr: f32) -> f32 {
    (age_gyr / 0.50).fract()
}

/// Calculates supercontinent aggregation index in [0.0, 1.0].
///
/// 1.0 represents a tightly clustered supercontinent (Pangea, Rodinia),
/// while ~0.15 represents dispersed modern continents.
pub fn calculate_supercontinent_aggregation(drift_phase: f32) -> f32 {
    let cycle = (drift_phase * std::f32::consts::TAU).cos();
    (cycle * 0.5 + 0.5).clamp(0.12, 1.0)
}

/// Calculates ocean oxidation progress in [0.0, 1.0].
///
/// Returns ~0.02 for Archean green anoxic waters (high dissolved $Fe^{2+}$),
/// transitioning smoothly through the Great Oxidation Event (~2.4 Ga ago / age ~2.16 Gyr),
/// reaching 1.0 for modern oxidized sapphire blue oceans.
pub fn calculate_ocean_oxidation_progress(age_gyr: f32) -> f32 {
    if age_gyr < 2.00 {
        0.02
    } else if age_gyr < 2.70 {
        let t = (age_gyr - 2.00) / 0.70;
        let smooth = t * t * (3.0 - 2.0 * t);
        0.02 + smooth * 0.85
    } else if age_gyr < 4.00 {
        0.87 + ((age_gyr - 2.70) / 1.30) * 0.11
    } else {
        1.0
    }
}

/// Calculates terrestrial plant colonization factor in [0.0, 1.0].
///
/// Landmasses remained barren rock/craton until the Silurian-Devonian plant explosion
/// (~450-380 Ma ago / age ~4.11-4.18 Gyr).
pub fn calculate_vegetation_expansion(age_gyr: f32) -> f32 {
    if age_gyr < 4.05 {
        0.0
    } else if age_gyr < 4.35 {
        let t = (age_gyr - 4.05) / 0.30;
        t * t * (3.0 - 2.0 * t)
    } else {
        1.0
    }
}

/// Calculates atmospheric oxygen level relative to Present Atmospheric Level (PAL).
pub fn calculate_oxygen_level_pal(age_gyr: f32) -> f32 {
    if age_gyr < 2.10 {
        0.001
    } else if age_gyr < 2.60 {
        let t = (age_gyr - 2.10) / 0.50;
        0.001 + t * 0.10
    } else if age_gyr < 4.00 {
        0.10 + ((age_gyr - 2.60) / 1.40) * 0.15
    } else if age_gyr < 4.30 {
        // Carboniferous oxygen spike
        0.85 + ((age_gyr - 4.00) / 0.30) * 0.45
    } else {
        1.0
    }
}

/// Updates geological state components on terrestrial worlds.
#[allow(
    clippy::type_complexity,
    reason = "Geological epoch synchronization query"
)]
pub fn sync_geological_evolution_system(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    opt_player_state: Option<Res<PlayerInteractionState>>,
    mut scrubber: ResMut<TimelineScrubber>,
    mut query: Query<(
        Entity,
        &CelestialBody,
        Option<&mut GeologicalState>,
        Option<&mut VolatileInventory>,
        Option<&mut PlanetaryClimate>,
        Option<&mut Temperature>,
    )>,
) {
    let current_epoch = scrubber.active_epoch;

    // Check if player selected a terrestrial world or protoplanet
    if let Some(ref player_state) = opt_player_state {
        if player_state.is_changed() {
            if let Some(selected) = player_state.selected_entity {
                if let Ok((entity, body, ref opt_sel_geo, _, _, _)) = query.get(selected) {
                    let is_terrestrial = matches!(
                        body.body_type,
                        BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
                    );
                    if let Some(archetype) = EpochTargetPlanet::try_detect_from_name(&body.name) {
                        if is_terrestrial && scrubber.target_entity != Some(entity) {
                            scrubber.target_entity = Some(entity);
                            scrubber.target_planet = archetype;
                            if let Some(target_geo) = opt_sel_geo {
                                scrubber.active_epoch = target_geo.epoch;
                                scrubber.scrubbed_age_gyr = target_geo.geological_age_gyr;
                            } else {
                                let def_epoch = GeologicalEpoch::epochs_for_planet(archetype)
                                    .first()
                                    .copied()
                                    .unwrap_or(GeologicalEpoch::Modern);
                                scrubber.active_epoch = def_epoch;
                                scrubber.scrubbed_age_gyr = def_epoch.canonical_age_gyr();
                            }
                        }
                    }
                }
            }
        }
    }

    // Ensure target_entity aligns with target_planet chosen in UI
    let target_needs_sync = scrubber.target_entity.is_none_or(|ent| {
        query.get(ent).map_or(true, |(_, body, _, _, _, _)| {
            !matches!(
                body.body_type,
                BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
            ) || EpochTargetPlanet::try_detect_from_name(&body.name) != Some(scrubber.target_planet)
        })
    });
    if target_needs_sync {
        if let Some((ent, _, opt_geo, _, _, _)) = query.iter().find(|(_, body, _, _, _, _)| {
            matches!(
                body.body_type,
                BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
            ) && EpochTargetPlanet::try_detect_from_name(&body.name) == Some(scrubber.target_planet)
        }) {
            scrubber.target_entity = Some(ent);
            if scrubber.auto_advance {
                if let Some(geo) = opt_geo {
                    scrubber.active_epoch = geo.epoch;
                    scrubber.scrubbed_age_gyr = geo.geological_age_gyr;
                }
            }
        } else {
            scrubber.target_entity = None;
        }
    }

    for (entity, body, mut opt_geo, mut opt_vol, mut opt_climate, mut opt_temp) in query.iter_mut()
    {
        let is_terrestrial = matches!(
            body.body_type,
            BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
        );

        let Some(archetype) = EpochTargetPlanet::try_detect_from_name(&body.name) else {
            continue;
        };

        if !is_terrestrial {
            continue;
        }

        if scrubber.target_entity.is_none() && scrubber.target_planet == archetype {
            scrubber.target_entity = Some(entity);
        }

        let Some(ref mut geo) = opt_geo else {
            commands
                .entity(entity)
                .insert(GeologicalState::default_for_planet(archetype));
            continue;
        };

        let is_target = scrubber.target_entity == Some(entity);

        if scrubber.auto_advance {
            advance_organic_epoch_step(
                geo,
                archetype,
                sim_time.elapsed_years,
                is_target,
                &mut scrubber,
                opt_vol.as_deref_mut(),
                opt_climate.as_deref_mut(),
                opt_temp.as_deref_mut(),
            );
        } else if is_target {
            apply_manual_scrub_step(
                geo,
                current_epoch,
                &scrubber,
                opt_vol.as_deref_mut(),
                opt_climate.as_deref_mut(),
                opt_temp.as_deref_mut(),
            );
        } else {
            // Unselected planets also sustain their chosen epoch state firmly
            let canonical_temp = geo.epoch.canonical_temperature_k();
            if let Some(ref mut temp) = opt_temp {
                temp.0 = canonical_temp;
            }
            if let Some(ref mut vol) = opt_vol {
                apply_epoch_volatiles(geo.epoch, vol);
            }
            if let Some(ref mut climate) = opt_climate {
                apply_epoch_climate(geo.epoch, climate);
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Organic epoch evolution step helper"
)]
fn advance_organic_epoch_step(
    geo: &mut GeologicalState,
    archetype: EpochTargetPlanet,
    elapsed_years: f64,
    is_target: bool,
    scrubber: &mut TimelineScrubber,
    opt_vol: Option<&mut VolatileInventory>,
    opt_climate: Option<&mut PlanetaryClimate>,
    opt_temp: Option<&mut Temperature>,
) {
    let sim_gyr = (elapsed_years / 1e9) as f32;
    let current_age = (geo.geological_age_gyr + sim_gyr * 0.1).clamp(0.0, 6.0);
    let prev_epoch = geo.epoch;
    geo.geological_age_gyr = current_age;
    geo.epoch = GeologicalEpoch::from_age_and_planet(current_age, archetype);
    geo.continental_drift_phase = calculate_continental_drift_phase(current_age);
    geo.supercontinent_aggregation = geo.epoch.canonical_aggregation();
    geo.ocean_oxidation_progress = calculate_ocean_oxidation_progress(current_age);
    geo.terrestrial_vegetation_fraction = calculate_vegetation_expansion(current_age);
    geo.oxygen_level_pal = calculate_oxygen_level_pal(current_age);

    if geo.epoch != prev_epoch {
        apply_epoch_to_world(geo.epoch, geo, opt_vol, opt_climate, opt_temp);
        geo.geological_age_gyr = current_age;
        geo.continental_drift_phase = calculate_continental_drift_phase(current_age);
        geo.supercontinent_aggregation = geo.epoch.canonical_aggregation();
    }

    if is_target {
        scrubber.active_epoch = geo.epoch;
        scrubber.scrubbed_age_gyr = geo.geological_age_gyr;
    }
}

fn apply_manual_scrub_step(
    geo: &mut GeologicalState,
    current_epoch: GeologicalEpoch,
    scrubber: &TimelineScrubber,
    opt_vol: Option<&mut VolatileInventory>,
    opt_climate: Option<&mut PlanetaryClimate>,
    opt_temp: Option<&mut Temperature>,
) {
    let target_age = scrubber.scrubbed_age_gyr;
    let target_epoch = scrubber.active_epoch;

    if (geo.geological_age_gyr - target_age).abs() > 1e-4 || geo.epoch != target_epoch {
        let prev_epoch = geo.epoch;
        geo.geological_age_gyr = target_age;
        geo.epoch = target_epoch;
        geo.continental_drift_phase = calculate_continental_drift_phase(target_age);
        geo.supercontinent_aggregation = geo.epoch.canonical_aggregation();
        geo.ocean_oxidation_progress = calculate_ocean_oxidation_progress(target_age);
        geo.terrestrial_vegetation_fraction = calculate_vegetation_expansion(target_age);
        geo.oxygen_level_pal = calculate_oxygen_level_pal(target_age);

        if geo.epoch != prev_epoch || geo.epoch != current_epoch {
            apply_epoch_to_world(geo.epoch, geo, opt_vol, opt_climate, opt_temp);
            geo.geological_age_gyr = target_age;
            geo.continental_drift_phase = calculate_continental_drift_phase(target_age);
            geo.supercontinent_aggregation = geo.epoch.canonical_aggregation();
        }
    } else {
        // Sustain chosen epoch temperature, volatiles, and climate on target planet
        let canonical_temp = geo.epoch.canonical_temperature_k();
        if let Some(temp) = opt_temp {
            temp.0 = canonical_temp;
        }
        if let Some(vol) = opt_vol {
            apply_epoch_volatiles(geo.epoch, vol);
        }
        if let Some(climate) = opt_climate {
            apply_epoch_climate(geo.epoch, climate);
        }
    }
}

/// Sets a world's physical and geological state to canonical conditions of `epoch`.
pub fn apply_epoch_to_world(
    epoch: GeologicalEpoch,
    geo: &mut GeologicalState,
    opt_vol: Option<&mut VolatileInventory>,
    opt_climate: Option<&mut PlanetaryClimate>,
    mut opt_temp: Option<&mut Temperature>,
) {
    *geo = GeologicalState::new_for_epoch(epoch);

    let canonical_temp = epoch.canonical_temperature_k();
    if let Some(ref mut temp) = opt_temp {
        temp.0 = canonical_temp;
    }

    if let Some(vol) = opt_vol {
        apply_epoch_volatiles(epoch, vol);
    }

    if let Some(climate) = opt_climate {
        apply_epoch_climate(epoch, climate);
    }
}

/// Applies canonical volatile inventory for the specified epoch.
#[allow(
    clippy::match_same_arms,
    reason = "Distinct planetary epochs may share volatile parameters"
)]
fn apply_epoch_volatiles(epoch: GeologicalEpoch, vol: &mut VolatileInventory) {
    match epoch {
        // Earth
        GeologicalEpoch::Hadean => {
            vol.delivered_water_m_earth = 0.0004;
            vol.ocean_coverage_frac = 0.0;
            vol.atmospheric_pressure_bar = 25.0;
        }
        GeologicalEpoch::Archean => {
            vol.delivered_water_m_earth = 0.000_693;
            vol.ocean_coverage_frac = 0.82;
            vol.atmospheric_pressure_bar = 1.8;
        }
        GeologicalEpoch::Proterozoic => {
            vol.delivered_water_m_earth = 0.000_608;
            vol.ocean_coverage_frac = 0.72;
            vol.atmospheric_pressure_bar = 1.2;
        }
        GeologicalEpoch::SnowballEarth => {
            vol.delivered_water_m_earth = 0.0006;
            vol.ocean_coverage_frac = 0.05;
            vol.atmospheric_pressure_bar = 0.50;
        }
        GeologicalEpoch::Phanerozoic | GeologicalEpoch::Modern => {
            vol.delivered_water_m_earth = 0.0006;
            vol.ocean_coverage_frac = 0.70;
            vol.atmospheric_pressure_bar = 1.0;
        }
        GeologicalEpoch::Future => {
            vol.delivered_water_m_earth = 0.000_435;
            vol.ocean_coverage_frac = 0.45;
            vol.atmospheric_pressure_bar = 0.8;
        }
        // Mars
        GeologicalEpoch::MarsPreNoachian => {
            vol.delivered_water_m_earth = 0.000_150;
            vol.ocean_coverage_frac = 0.10;
            vol.atmospheric_pressure_bar = 1.50;
        }
        GeologicalEpoch::MarsNoachian => {
            vol.delivered_water_m_earth = 0.000_250;
            vol.ocean_coverage_frac = 0.35;
            vol.atmospheric_pressure_bar = 0.80;
        }
        GeologicalEpoch::MarsHesperian => {
            vol.delivered_water_m_earth = 0.000_080;
            vol.ocean_coverage_frac = 0.10;
            vol.atmospheric_pressure_bar = 0.15;
        }
        GeologicalEpoch::MarsAmazonian => {
            vol.delivered_water_m_earth = 0.000_020;
            vol.ocean_coverage_frac = 0.0;
            vol.atmospheric_pressure_bar = 0.006;
        }
        GeologicalEpoch::MarsFuture => {
            vol.delivered_water_m_earth = 0.000_005;
            vol.ocean_coverage_frac = 0.0;
            vol.atmospheric_pressure_bar = 0.010;
        }
        // Venus
        GeologicalEpoch::VenusPrimordial => {
            vol.delivered_water_m_earth = 0.000_300;
            vol.ocean_coverage_frac = 0.0;
            vol.atmospheric_pressure_bar = 40.0;
        }
        GeologicalEpoch::VenusTemperate => {
            vol.delivered_water_m_earth = 0.000_340;
            vol.ocean_coverage_frac = 0.40;
            vol.atmospheric_pressure_bar = 1.50;
        }
        GeologicalEpoch::VenusRunaway => {
            vol.delivered_water_m_earth = 0.000_100;
            vol.ocean_coverage_frac = 0.05;
            vol.atmospheric_pressure_bar = 15.0;
        }
        GeologicalEpoch::VenusModern => {
            vol.delivered_water_m_earth = 0.0;
            vol.ocean_coverage_frac = 0.0;
            vol.atmospheric_pressure_bar = 92.0;
        }
        GeologicalEpoch::VenusFuture => {
            vol.delivered_water_m_earth = 0.0;
            vol.ocean_coverage_frac = 0.0;
            vol.atmospheric_pressure_bar = 100.0;
        }
    }
}

/// Applies canonical climate conditions for the specified epoch.
#[allow(
    clippy::match_same_arms,
    reason = "Distinct planetary epochs may share climate parameters"
)]
fn apply_epoch_climate(epoch: GeologicalEpoch, climate: &mut PlanetaryClimate) {
    match epoch {
        // Earth
        GeologicalEpoch::Hadean => {
            climate.surface_temperature_k = 550.0;
            climate.ice_coverage_frac = 0.0;
            climate.cloud_coverage_frac = 0.95;
            climate.climate_regime = ClimateRegime::RunawayVenusian;
        }
        GeologicalEpoch::Archean => {
            climate.surface_temperature_k = 315.0;
            climate.ice_coverage_frac = 0.0;
            climate.cloud_coverage_frac = 0.65;
            climate.climate_regime = ClimateRegime::TemperateHabitable;
        }
        GeologicalEpoch::Proterozoic => {
            climate.surface_temperature_k = 265.0;
            climate.ice_coverage_frac = 0.35;
            climate.cloud_coverage_frac = 0.45;
            climate.climate_regime = ClimateRegime::TemperateHabitable;
        }
        GeologicalEpoch::SnowballEarth => {
            climate.surface_temperature_k = 220.0;
            climate.ice_coverage_frac = 0.95;
            climate.cloud_coverage_frac = 0.30;
            climate.climate_regime = ClimateRegime::SnowballIceAge;
        }
        GeologicalEpoch::Phanerozoic | GeologicalEpoch::Modern => {
            climate.surface_temperature_k = 288.0;
            climate.ice_coverage_frac = 0.10;
            climate.cloud_coverage_frac = 0.50;
            climate.climate_regime = ClimateRegime::TemperateHabitable;
        }
        GeologicalEpoch::Future => {
            climate.surface_temperature_k = 345.0;
            climate.ice_coverage_frac = 0.0;
            climate.cloud_coverage_frac = 0.70;
            climate.climate_regime = ClimateRegime::TemperateHabitable;
        }
        // Mars
        GeologicalEpoch::MarsPreNoachian => {
            climate.surface_temperature_k = 295.0;
            climate.ice_coverage_frac = 0.0;
            climate.cloud_coverage_frac = 0.50;
            climate.climate_regime = ClimateRegime::TemperateHabitable;
        }
        GeologicalEpoch::MarsNoachian => {
            climate.surface_temperature_k = 280.0;
            climate.ice_coverage_frac = 0.15;
            climate.cloud_coverage_frac = 0.40;
            climate.climate_regime = ClimateRegime::TemperateHabitable;
        }
        GeologicalEpoch::MarsHesperian => {
            climate.surface_temperature_k = 240.0;
            climate.ice_coverage_frac = 0.40;
            climate.cloud_coverage_frac = 0.25;
            climate.climate_regime = ClimateRegime::SnowballIceAge;
        }
        GeologicalEpoch::MarsAmazonian => {
            climate.surface_temperature_k = 215.0;
            climate.ice_coverage_frac = 0.15;
            climate.cloud_coverage_frac = 0.08;
            climate.climate_regime = ClimateRegime::SnowballIceAge;
        }
        GeologicalEpoch::MarsFuture => {
            climate.surface_temperature_k = 230.0;
            climate.ice_coverage_frac = 0.05;
            climate.cloud_coverage_frac = 0.02;
            climate.climate_regime = ClimateRegime::SnowballIceAge;
        }
        // Venus
        GeologicalEpoch::VenusPrimordial => {
            climate.surface_temperature_k = 600.0;
            climate.ice_coverage_frac = 0.0;
            climate.cloud_coverage_frac = 0.98;
            climate.climate_regime = ClimateRegime::RunawayVenusian;
        }
        GeologicalEpoch::VenusTemperate => {
            climate.surface_temperature_k = 295.0;
            climate.ice_coverage_frac = 0.05;
            climate.cloud_coverage_frac = 0.60;
            climate.climate_regime = ClimateRegime::TemperateHabitable;
        }
        GeologicalEpoch::VenusRunaway => {
            climate.surface_temperature_k = 480.0;
            climate.ice_coverage_frac = 0.0;
            climate.cloud_coverage_frac = 0.90;
            climate.climate_regime = ClimateRegime::RunawayVenusian;
        }
        GeologicalEpoch::VenusModern => {
            climate.surface_temperature_k = 735.0;
            climate.ice_coverage_frac = 0.0;
            climate.cloud_coverage_frac = 1.00;
            climate.climate_regime = ClimateRegime::RunawayVenusian;
        }
        GeologicalEpoch::VenusFuture => {
            climate.surface_temperature_k = 780.0;
            climate.ice_coverage_frac = 0.0;
            climate.cloud_coverage_frac = 1.00;
            climate.climate_regime = ClimateRegime::RunawayVenusian;
        }
    }
}
