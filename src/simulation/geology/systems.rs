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
pub fn sync_geological_evolution_system(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    mut scrubber: ResMut<TimelineScrubber>,
    mut query: Query<(
        Entity,
        &CelestialBody,
        Option<&mut GeologicalState>,
        Option<&mut VolatileInventory>,
        Option<&mut PlanetaryClimate>,
    )>,
) {
    let current_epoch = scrubber.active_epoch;

    for (entity, body, mut opt_geo, mut opt_vol, mut opt_climate) in query.iter_mut() {
        let is_terrestrial = matches!(
            body.body_type,
            BodyType::TerrestrialPlanet | BodyType::SuperEarth
        );

        if scrubber.target_entity.is_none() && is_terrestrial {
            scrubber.target_entity = Some(entity);
        }

        let Some(ref mut geo) = opt_geo else {
            if is_terrestrial {
                commands.entity(entity).insert(GeologicalState::default());
            }
            continue;
        };

        if scrubber.auto_advance {
            // Geological age tracks base epoch age plus elapsed simulation time in Gyr
            let sim_gyr = (sim_time.elapsed_years / 1e9) as f32;
            let current_age = (geo.geological_age_gyr + sim_gyr * 0.1).clamp(0.0, 6.0);
            geo.geological_age_gyr = current_age;
            geo.epoch = GeologicalEpoch::from_geological_age_gyr(current_age);
            geo.continental_drift_phase = calculate_continental_drift_phase(current_age);
            geo.supercontinent_aggregation =
                calculate_supercontinent_aggregation(geo.continental_drift_phase);
            geo.ocean_oxidation_progress = calculate_ocean_oxidation_progress(current_age);
            geo.terrestrial_vegetation_fraction = calculate_vegetation_expansion(current_age);
            geo.oxygen_level_pal = calculate_oxygen_level_pal(current_age);

            if scrubber.target_entity == Some(entity) {
                scrubber.active_epoch = geo.epoch;
                scrubber.scrubbed_age_gyr = geo.geological_age_gyr;
            }
        } else {
            // Manual scrub mode
            let target_age = scrubber.scrubbed_age_gyr;
            if (geo.geological_age_gyr - target_age).abs() > 1e-4 || geo.epoch != current_epoch {
                let prev_epoch = geo.epoch;
                geo.geological_age_gyr = target_age;
                geo.epoch = GeologicalEpoch::from_geological_age_gyr(target_age);
                geo.continental_drift_phase = calculate_continental_drift_phase(target_age);
                geo.supercontinent_aggregation =
                    calculate_supercontinent_aggregation(geo.continental_drift_phase);
                geo.ocean_oxidation_progress = calculate_ocean_oxidation_progress(target_age);
                geo.terrestrial_vegetation_fraction = calculate_vegetation_expansion(target_age);
                geo.oxygen_level_pal = calculate_oxygen_level_pal(target_age);

                if geo.epoch != prev_epoch {
                    apply_epoch_to_world(
                        geo.epoch,
                        geo,
                        opt_vol.as_deref_mut(),
                        opt_climate.as_deref_mut(),
                    );
                    // Re-apply specific scrubbed age since apply_epoch_to_world defaults age to epoch canonical age
                    geo.geological_age_gyr = target_age;
                    geo.continental_drift_phase = calculate_continental_drift_phase(target_age);
                    geo.supercontinent_aggregation =
                        calculate_supercontinent_aggregation(geo.continental_drift_phase);
                    geo.ocean_oxidation_progress = calculate_ocean_oxidation_progress(target_age);
                    geo.terrestrial_vegetation_fraction =
                        calculate_vegetation_expansion(target_age);
                    geo.oxygen_level_pal = calculate_oxygen_level_pal(target_age);
                }
            }
        }
    }
}

/// Sets a world's physical and geological state to canonical conditions of `epoch`.
pub fn apply_epoch_to_world(
    epoch: GeologicalEpoch,
    geo: &mut GeologicalState,
    opt_vol: Option<&mut VolatileInventory>,
    opt_climate: Option<&mut PlanetaryClimate>,
) {
    *geo = GeologicalState::new_for_epoch(epoch);

    if let Some(vol) = opt_vol {
        match epoch {
            GeologicalEpoch::Hadean => {
                vol.delivered_water_m_earth = 0.0004;
                vol.ocean_coverage_frac = 0.15; // Primordial pooling
                vol.atmospheric_pressure_bar = 25.0; // Superdense steam/CO2
            }
            GeologicalEpoch::Archean => {
                vol.delivered_water_m_earth = 0.0006;
                vol.ocean_coverage_frac = 0.82; // Global anoxic ocean
                vol.atmospheric_pressure_bar = 1.8;
            }
            GeologicalEpoch::Proterozoic => {
                vol.delivered_water_m_earth = 0.0006;
                vol.ocean_coverage_frac = 0.72;
                vol.atmospheric_pressure_bar = 1.2;
            }
            GeologicalEpoch::Phanerozoic | GeologicalEpoch::Modern => {
                vol.delivered_water_m_earth = 0.0006;
                vol.ocean_coverage_frac = 0.70;
                vol.atmospheric_pressure_bar = 1.0;
            }
            GeologicalEpoch::Future => {
                vol.delivered_water_m_earth = 0.0003;
                vol.ocean_coverage_frac = 0.45; // Evaporating oceans
                vol.atmospheric_pressure_bar = 0.8;
            }
        }
    }

    if let Some(climate) = opt_climate {
        match epoch {
            GeologicalEpoch::Hadean => {
                climate.surface_temperature_k = 550.0;
                climate.ice_coverage_frac = 0.0;
                climate.cloud_coverage_frac = 0.95;
            }
            GeologicalEpoch::Archean => {
                climate.surface_temperature_k = 315.0; // Warm methane greenhouse
                climate.ice_coverage_frac = 0.0;
                climate.cloud_coverage_frac = 0.65;
            }
            GeologicalEpoch::Proterozoic => {
                climate.surface_temperature_k = 265.0; // Huronian/Cryogenian glaciations
                climate.ice_coverage_frac = 0.55;
                climate.cloud_coverage_frac = 0.45;
            }
            GeologicalEpoch::Phanerozoic | GeologicalEpoch::Modern => {
                climate.surface_temperature_k = 288.0;
                climate.ice_coverage_frac = 0.10;
                climate.cloud_coverage_frac = 0.50;
            }
            GeologicalEpoch::Future => {
                climate.surface_temperature_k = 345.0;
                climate.ice_coverage_frac = 0.0;
                climate.cloud_coverage_frac = 0.70;
            }
        }
    }
}
