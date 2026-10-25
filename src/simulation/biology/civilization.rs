use crate::simulation::components::celestial::{
    CelestialBody, CentralStar, SatelliteOf, SimPosition, Temperature, TrackedOrbit,
};
use crate::simulation::components::planetary::{
    BiosphereState, FloraPigmentClass, PlanetaryClimate,
};
use crate::simulation::resources::{SimTime, TimeWarp};
use crate::simulation::tides::TidalState;
use bevy::prelude::*;

#[derive(Component, Default)]
pub struct HabitabilityTimer(pub f64);

type HabitabilityQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    Option<&'a SatelliteOf>,
    &'a PlanetaryClimate,
    Option<&'a mut BiosphereState>,
    Option<&'a TrackedOrbit>,
    Option<&'a TidalState>,
    Option<&'a mut HabitabilityTimer>,
);

pub fn habitability_tracking_system(
    mut commands: Commands,
    stars: Query<(
        Entity,
        &Temperature,
        Option<&SimPosition>,
        &CelestialBody,
        Option<&CentralStar>,
    )>,
    mut query: Query<HabitabilityQueryItem>,
    time_warp: Option<Res<TimeWarp>>,
    sim_time: Option<Res<SimTime>>,
) {
    if time_warp
        .as_ref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once)
    {
        return;
    }

    let dt = sim_time.as_ref().map_or(1.0, |st| st.current_dt_yr);
    let elapsed_years = sim_time.as_ref().map_or(0.0, |st| st.elapsed_years);

    let stars_data: Vec<(Entity, f64, bevy::math::DVec3, bool)> = stars
        .iter()
        .filter(|(_, _, _, body, _)| body.body_type.is_star_or_remnant())
        .map(|(e, t, opt_p, _, opt_c)| {
            (
                e,
                t.0,
                opt_p.map_or(bevy::math::DVec3::ZERO, |p| p.0),
                opt_c.is_some(),
            )
        })
        .collect();

    for (entity, pos, opt_sat, climate, biosphere, orbits, opt_tidal, habitability_timer) in
        query.iter_mut()
    {
        let host_temp_k = if let Some(sat) = opt_sat {
            stars_data
                .iter()
                .find(|(e, ..)| *e == sat.parent)
                .map(|(_, t, ..)| *t)
                .or_else(|| {
                    stars_data
                        .iter()
                        .min_by(|a, b| {
                            (pos.0 - a.2)
                                .length_squared()
                                .partial_cmp(&(pos.0 - b.2).length_squared())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(_, t, ..)| *t)
                })
                .unwrap_or(5778.0)
        } else {
            stars_data
                .iter()
                .min_by(|a, b| {
                    (pos.0 - a.2)
                        .length_squared()
                        .partial_cmp(&(pos.0 - b.2).length_squared())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map_or(5778.0, |(_, t, ..)| *t)
        };

        let adapted_pigment = FloraPigmentClass::from_star_temperature(host_temp_k);
        let temp = climate.surface_temperature_k;
        let is_habitable_temp = (273.0..=373.0).contains(&temp);

        let stable_orbit = orbits.is_some_and(|o| o.elements.eccentricity < 0.2);
        let can_advance_civ = is_habitable_temp && stable_orbit;
        let age_sufficient = elapsed_years > 1_000_000.0;
        let conditions_met = can_advance_civ && age_sufficient;
        let is_tidally_locked = opt_tidal.is_some_and(|t| t.is_tidally_locked);

        if let Some(mut bio) = biosphere {
            bio.flora_pigment = adapted_pigment;
            if is_tidally_locked && bio.biomass_coverage_frac > 0.05 {
                bio.bioluminescence_intensity = (bio
                    .bioluminescence_intensity
                    .max(0.65 * bio.biomass_coverage_frac))
                .clamp(0.0, 1.0);
            }
            if can_advance_civ {
                bio.technosignature = (bio.technosignature + (0.005 * dt as f32)).min(2.0);
            }
        } else if conditions_met {
            if let Some(mut timer) = habitability_timer {
                timer.0 += dt;
                if timer.0 > 100_000.0 {
                    if let Ok(mut ecmd) = commands.get_entity(entity) {
                        ecmd.insert(BiosphereState {
                            habitability_score: 0.1,
                            emergence_year: Some(elapsed_years),
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
