use crate::simulation::components::celestial::{CentralStar, Temperature, TrackedOrbit};
use crate::simulation::components::planetary::{
    BiosphereState, FloraPigmentClass, PlanetaryClimate,
};
use crate::simulation::resources::SimTime;
use crate::simulation::tides::TidalState;
use bevy::prelude::*;

#[derive(Component, Default)]
pub struct HabitabilityTimer(pub f64);

type HabitabilityQueryItem<'a> = (
    Entity,
    &'a PlanetaryClimate,
    Option<&'a mut BiosphereState>,
    Option<&'a TrackedOrbit>,
    Option<&'a TidalState>,
    Option<&'a mut HabitabilityTimer>,
);

pub fn habitability_tracking_system(
    mut commands: Commands,
    stars: Query<&Temperature, With<CentralStar>>,
    mut query: Query<HabitabilityQueryItem>,
    sim_time: Res<SimTime>,
) {
    let dt = sim_time.current_dt_yr;
    let host_star_temp_k = stars.iter().next().map_or(5778.0, |t| t.0);
    let adapted_pigment = FloraPigmentClass::from_star_temperature(host_star_temp_k);

    for (entity, climate, biosphere, orbits, opt_tidal, habitability_timer) in query.iter_mut() {
        let temp = climate.surface_temperature_k;
        let is_habitable_temp = (273.0..=373.0).contains(&temp);

        let stable_orbit = orbits.is_some_and(|o| o.elements.eccentricity < 0.2);
        let age_sufficient = sim_time.elapsed_years > 1_000_000.0;

        let conditions_met = is_habitable_temp && stable_orbit && age_sufficient;
        let is_tidally_locked = opt_tidal.is_some_and(|t| t.is_tidally_locked);

        if let Some(mut bio) = biosphere {
            bio.flora_pigment = adapted_pigment;
            if is_tidally_locked && bio.biomass_coverage_frac > 0.05 {
                bio.bioluminescence_intensity = (bio
                    .bioluminescence_intensity
                    .max(0.65 * bio.biomass_coverage_frac))
                .clamp(0.0, 1.0);
            }
            if conditions_met {
                bio.technosignature = (bio.technosignature + (0.005 * dt as f32)).min(2.0);
            }
        } else if conditions_met {
            if let Some(mut timer) = habitability_timer {
                timer.0 += dt;
                if timer.0 > 100_000.0 {
                    if let Ok(mut ecmd) = commands.get_entity(entity) {
                        ecmd.insert(BiosphereState {
                            habitability_score: 0.1,
                            emergence_year: Some(sim_time.elapsed_years),
                            flora_pigment: adapted_pigment,
                            bioluminescence_intensity: if is_tidally_locked { 0.35 } else { 0.15 },
                            ..Default::default()
                        });
                        ecmd.remove::<HabitabilityTimer>();
                    }
                }
            } else if let Ok(mut ecmd) = commands.get_entity(entity) {
                ecmd.insert(HabitabilityTimer(0.0));
            }
        } else if let Some(mut timer) = habitability_timer {
            timer.0 = (timer.0 - dt).max(0.0);
        }
    }
}
