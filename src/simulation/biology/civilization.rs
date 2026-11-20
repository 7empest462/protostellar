use crate::simulation::components::celestial::{
    BodyType, CelestialBody, CentralStar, Mass, SatelliteOf, SimPosition, SimVelocity, Temperature,
    TrackedOrbit,
};
use crate::simulation::components::planetary::{
    BiosphereState, ClimateRegime, FloraPigmentClass, PlanetaryClimate, VolatileInventory,
};
use crate::simulation::resources::{SimTime, TimeWarp};
use crate::simulation::tides::TidalState;
use crate::utils::math::state_vectors_to_orbital_elements;
use bevy::math::DVec3;
use bevy::prelude::*;

#[derive(
    Component, Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize,
)]
pub struct HabitabilityTimer(pub f64);

type HabitabilityQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    Option<&'a SimVelocity>,
    Option<&'a Mass>,
    Option<&'a CelestialBody>,
    Option<&'a SatelliteOf>,
    &'a PlanetaryClimate,
    Option<&'a VolatileInventory>,
    Option<&'a mut BiosphereState>,
    Option<&'a TrackedOrbit>,
    Option<&'a TidalState>,
    Option<&'a mut HabitabilityTimer>,
);

#[derive(Clone, Copy)]
struct StarHostData {
    entity: Entity,
    temp_k: f64,
    pos: DVec3,
    vel: DVec3,
    mass_solar: f64,
}

fn find_nearest_star(stars_data: &[StarHostData], pos: DVec3) -> Option<StarHostData> {
    stars_data
        .iter()
        .min_by(|a, b| {
            (pos - a.pos)
                .length_squared()
                .partial_cmp(&(pos - b.pos).length_squared())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
}

fn is_orbit_habitable_stable(
    pos: DVec3,
    opt_vel: Option<&SimVelocity>,
    opt_mass: Option<&Mass>,
    opt_sat: Option<&SatelliteOf>,
    opt_orbit: Option<&TrackedOrbit>,
    host_star: Option<StarHostData>,
    body_lookup: &rustc_hash::FxHashMap<Entity, (DVec3, DVec3, f64)>,
) -> bool {
    if let Some(orbit) = opt_orbit {
        return orbit.elements.eccentricity.is_finite()
            && orbit.elements.eccentricity < 0.2
            && orbit.elements.semi_major_axis.is_finite()
            && orbit.elements.semi_major_axis > 0.0;
    }

    let body_mass = opt_mass.map_or(crate::utils::constants::EARTH_MASS_SOLAR, |m| m.0);
    if let Some(sat) = opt_sat {
        if sat.semi_major_axis_au <= 0.0 {
            return false;
        }
        if let Some(&(p_pos, p_vel, p_mass)) = body_lookup.get(&sat.parent) {
            let parent_stable = host_star.is_none_or(|star| {
                state_vectors_to_orbital_elements(
                    p_pos - star.pos,
                    p_vel - star.vel,
                    star.mass_solar,
                    p_mass,
                )
                .is_some_and(|el| el.eccentricity < 0.25 && el.semi_major_axis > 0.0)
            });
            if !parent_stable {
                return false;
            }
            if let Some(vel) = opt_vel {
                return state_vectors_to_orbital_elements(
                    pos - p_pos,
                    vel.0 - p_vel,
                    p_mass,
                    body_mass,
                )
                .is_some_and(|el| el.eccentricity < 0.25 && el.semi_major_axis > 0.0);
            }
            return true;
        }
    }

    if let (Some(vel), Some(star)) = (opt_vel, host_star) {
        return state_vectors_to_orbital_elements(
            pos - star.pos,
            vel.0 - star.vel,
            star.mass_solar,
            body_mass,
        )
        .is_some_and(|el| el.eccentricity < 0.25 && el.semi_major_axis > 0.0);
    }

    false
}

fn populate_civilization_body_lookup(
    body_lookup: &mut rustc_hash::FxHashMap<Entity, (DVec3, DVec3, f64)>,
    all_bodies: &Query<
        (Entity, &SimPosition, Option<&SimVelocity>, Option<&Mass>),
        With<CelestialBody>,
    >,
    query: &Query<HabitabilityQueryItem>,
) {
    body_lookup.clear();
    for (entity, pos, opt_vel, opt_mass) in all_bodies.iter() {
        body_lookup.insert(
            entity,
            (
                pos.0,
                opt_vel.map_or(DVec3::ZERO, |v| v.0),
                opt_mass.map_or(crate::utils::constants::EARTH_MASS_SOLAR, |m| {
                    m.0.max(1e-12)
                }),
            ),
        );
    }
    for (entity, pos, opt_vel, opt_mass, ..) in query.iter() {
        body_lookup.entry(entity).or_insert((
            pos.0,
            opt_vel.map_or(DVec3::ZERO, |v| v.0),
            opt_mass.map_or(crate::utils::constants::EARTH_MASS_SOLAR, |m| {
                m.0.max(1e-12)
            }),
        ));
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Tracks biosphere emergence, stellar pigments, and orbital habitability"
)]
pub fn habitability_tracking_system(
    mut commands: Commands,
    mut body_lookup: Local<rustc_hash::FxHashMap<Entity, (DVec3, DVec3, f64)>>,
    stars: Query<(
        Entity,
        &Temperature,
        Option<&SimPosition>,
        Option<&SimVelocity>,
        Option<&Mass>,
        &CelestialBody,
        Option<&CentralStar>,
    )>,
    all_bodies: Query<
        (Entity, &SimPosition, Option<&SimVelocity>, Option<&Mass>),
        With<CelestialBody>,
    >,
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

    let stars_data: smallvec::SmallVec<[StarHostData; 4]> = stars
        .iter()
        .filter(|(_, _, _, _, _, body, _)| body.body_type.is_star_or_remnant())
        .map(|(entity, temp, opt_p, opt_v, opt_m, _, _)| StarHostData {
            entity,
            temp_k: temp.0,
            pos: opt_p.map_or(DVec3::ZERO, |p| p.0),
            vel: opt_v.map_or(DVec3::ZERO, |v| v.0),
            mass_solar: opt_m.map_or(1.0, |m| m.0.max(1e-6)),
        })
        .collect();

    populate_civilization_body_lookup(&mut body_lookup, &all_bodies, &query);

    for (
        entity,
        pos,
        opt_vel,
        opt_mass,
        opt_body,
        opt_sat,
        climate,
        opt_vol,
        biosphere,
        orbits,
        opt_tidal,
        habitability_timer,
    ) in query.iter_mut()
    {
        let host_star = opt_sat
            .and_then(|sat| stars_data.iter().find(|s| s.entity == sat.parent).copied())
            .or_else(|| find_nearest_star(&stars_data, pos.0));
        let host_temp_k = host_star.map_or(5778.0, |s| s.temp_k);

        let adapted_pigment = FloraPigmentClass::from_star_temperature(host_temp_k);
        let temp = climate.surface_temperature_k;
        let is_solid_habitable = opt_body.is_none_or(|b| {
            matches!(
                b.body_type,
                BodyType::TerrestrialPlanet
                    | BodyType::SuperEarth
                    | BodyType::Protoplanet
                    | BodyType::Moon
            )
        });
        let is_habitable_climate =
            matches!(climate.climate_regime, ClimateRegime::TemperateHabitable)
                && (273.0..=373.0).contains(&temp);
        let has_habitable_volatiles = opt_vol
            .is_none_or(|v| v.ocean_coverage_frac > 0.02 && v.atmospheric_pressure_bar >= 0.02);
        let stable_orbit = is_orbit_habitable_stable(
            pos.0,
            opt_vel,
            opt_mass,
            opt_sat,
            orbits,
            host_star,
            &body_lookup,
        );
        let can_advance_civ =
            is_solid_habitable && is_habitable_climate && has_habitable_volatiles && stable_orbit;
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
                if timer.0 > 100_000.0
                    && let Ok(mut ecmd) = commands.get_entity(entity)
                {
                    ecmd.insert(BiosphereState {
                        habitability_score: 0.1,
                        emergence_year: Some(elapsed_years),
                        flora_pigment: adapted_pigment,
                        bioluminescence_intensity: if is_tidally_locked { 0.35 } else { 0.15 },
                        ..Default::default()
                    });
                    ecmd.remove::<HabitabilityTimer>();
                }
            } else if let Ok(mut ecmd) = commands.get_entity(entity) {
                ecmd.insert(HabitabilityTimer(0.0));
            }
        } else if let Some(mut timer) = habitability_timer {
            timer.0 = (timer.0 - dt).max(0.0);
        }
    }
}
