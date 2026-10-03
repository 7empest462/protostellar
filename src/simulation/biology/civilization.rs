use crate::simulation::components::celestial::TrackedOrbit;
use crate::simulation::components::planetary::{BiosphereState, PlanetaryClimate};
use crate::simulation::resources::SimTime;
use bevy::prelude::*;

#[derive(Component, Default)]
pub struct HabitabilityTimer(pub f64);

pub fn habitability_tracking_system(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &PlanetaryClimate,
        Option<&mut BiosphereState>,
        Option<&TrackedOrbit>,
        Option<&mut HabitabilityTimer>,
    )>,
    sim_time: Res<SimTime>,
) {
    let dt = sim_time.current_dt_yr;
    for (entity, climate, biosphere, orbits, habitability_timer) in query.iter_mut() {
        let temp = climate.surface_temperature_k;
        let is_habitable_temp = (273.0..=373.0).contains(&temp);

        let stable_orbit = orbits.is_some_and(|o| o.elements.eccentricity < 0.2);
        let age_sufficient = sim_time.elapsed_years > 1_000_000.0;

        let conditions_met = is_habitable_temp && stable_orbit && age_sufficient;

        if let Some(mut bio) = biosphere {
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
