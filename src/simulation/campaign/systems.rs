//! System implementations for campaign mission objective evaluation and lifecycle events.

use bevy::prelude::*;

use super::missions::{create_mission_objectives, spawn_campaign_mission};
use super::types::*;
use crate::simulation::components::*;
use crate::simulation::economy::ScienceData;
use crate::simulation::probes::SpaceProbe;
use crate::simulation::resources::*;

/// Handles launching new campaign missions, resetting the world and initializing mission objectives.
#[allow(
    clippy::too_many_arguments,
    reason = "Scenario reset requires standard simulation world parameters"
)]
pub fn handle_start_campaign_mission(
    mut commands: Commands,
    mut events: MessageReader<StartCampaignMissionEvent>,
    mut campaign_state: ResMut<CampaignState>,
    mut disk_params: ResMut<DiskParameters>,
    mut config: ResMut<SimulationConfig>,
    mut sim_time: ResMut<SimTime>,
    mut time_warp: ResMut<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    bodies_query: Query<Entity, With<CelestialBody>>,
    mut toast: Option<ResMut<crate::game::ui::NotificationToast>>,
) {
    for event in events.read() {
        let mission_id = event.0;
        info!("🎯 Starting Campaign Mission: {:?}", mission_id);

        // Despawn all existing bodies
        for ent in bodies_query.iter() {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
        }

        // Reset clocks
        sim_time.elapsed_years = 0.0;
        sim_time.current_dt_yr = config.base_dt_yr;
        time_warp.step_once = false;

        // Spawn mission world
        let star_ent =
            spawn_campaign_mission(mission_id, &mut commands, &mut disk_params, &mut config);
        player_state.selected_entity = star_ent;

        // Initialize campaign state
        campaign_state.active_mission = Some(mission_id);
        campaign_state.status = MissionStatus::InProgress;
        campaign_state.elapsed_mission_years = 0.0;
        campaign_state.objectives = create_mission_objectives(mission_id);
        campaign_state.reward_claimed = false;
        campaign_state.failure_reason = None;
        campaign_state.flyby_visited_entities.clear();
        campaign_state.stable_biosphere_years = 0.0;
        campaign_state.evacuated_probe_count = 0;

        if let Some(ref mut t) = toast {
            t.message = format!("🎯 CAMPAIGN MISSION LAUNCHED // {}", mission_id.title());
            t.timer = 5.0;
        }
    }
}

/// Evaluates active campaign mission objectives each frame against the live simulation state.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Comprehensive campaign evaluation queries ECS"
)]
pub fn evaluate_campaign_objectives(
    mut campaign_state: ResMut<CampaignState>,
    mut science_data: ResMut<ScienceData>,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    stars_query: Query<(&Mass, &IgnitionState, Option<&StellarEvolutionState>), With<CentralStar>>,
    bodies_query: Query<(
        Entity,
        &CelestialBody,
        &SimPosition,
        Option<&PlanetaryClimate>,
        Option<&Temperature>,
        Option<&VolatileInventory>,
    )>,
    probes_query: Query<(Entity, &CelestialBody, &SimPosition, &SpaceProbe)>,
    mut complete_events: MessageWriter<CampaignMissionCompletedEvent>,
    mut fail_events: MessageWriter<CampaignMissionFailedEvent>,
) {
    if !campaign_state.is_mission_active() || (time_warp.is_paused && !time_warp.step_once) {
        return;
    }

    let Some(mission_id) = campaign_state.active_mission else {
        return;
    };
    let dt_yr = sim_time.current_dt_yr;
    campaign_state.elapsed_mission_years += dt_yr;

    match mission_id {
        CampaignMissionId::TheStellarForge => {
            eval_stellar_forge(&mut campaign_state, &stars_query, &bodies_query);
        }
        CampaignMissionId::GenesisWorld => {
            eval_genesis_world(&mut campaign_state, dt_yr, &bodies_query);
        }
        CampaignMissionId::TheGrandTour => {
            eval_grand_tour(&mut campaign_state, &probes_query, &bodies_query);
        }
        CampaignMissionId::Exodus => {
            eval_exodus(&mut campaign_state, &stars_query, &probes_query);
        }
    }

    check_mission_completion(
        mission_id,
        &mut campaign_state,
        &mut science_data,
        &mut complete_events,
        &mut fail_events,
    );
}

fn eval_stellar_forge(
    state: &mut CampaignState,
    stars_query: &Query<(&Mass, &IgnitionState, Option<&StellarEvolutionState>), With<CentralStar>>,
    bodies_query: &Query<(
        Entity,
        &CelestialBody,
        &SimPosition,
        Option<&PlanetaryClimate>,
        Option<&Temperature>,
        Option<&VolatileInventory>,
    )>,
) {
    // 1. Check central star mass and ignition
    if let Some((star_mass, ignition, _)) = stars_query.iter().next() {
        if let Some(obj) = state.objectives.iter_mut().find(|o| o.id == "forge_mass") {
            obj.current_value = star_mass.0;
            if star_mass.0 >= obj.target_value {
                obj.is_completed = true;
            }
        }
        if let Some(obj) = state
            .objectives
            .iter_mut()
            .find(|o| o.id == "forge_ignition")
        {
            obj.current_value = if ignition.is_ignited { 1.0 } else { 0.0 };
            if ignition.is_ignited {
                obj.is_completed = true;
            }
        }
    }

    // 2. Check preservation of rocky protoplanet embryos
    let surviving_embryos = bodies_query
        .iter()
        .filter(|(_, body, pos, _, _, _)| {
            body.name.contains("Forge Rocky Embryo") && pos.0.length() < 20.0
        })
        .count();

    let ignition_done = state
        .objectives
        .iter()
        .any(|o| o.id == "forge_ignition" && o.is_completed);
    if let Some(obj) = state
        .objectives
        .iter_mut()
        .find(|o| o.id == "forge_preserve_embryos")
    {
        obj.current_value = surviving_embryos as f64;
        if surviving_embryos < 2 {
            obj.is_failed = true;
        } else if ignition_done {
            obj.is_completed = true;
        }
    }
    if surviving_embryos < 2 {
        state.failure_reason = Some(
            "A rocky protoplanet embryo was destroyed or ejected from the system!".to_string(),
        );
    }
}

fn eval_genesis_world(
    state: &mut CampaignState,
    dt_yr: f64,
    bodies_query: &Query<(
        Entity,
        &CelestialBody,
        &SimPosition,
        Option<&PlanetaryClimate>,
        Option<&Temperature>,
        Option<&VolatileInventory>,
    )>,
) {
    let candidate = bodies_query
        .iter()
        .find(|(_, body, _, _, _, _)| body.name.contains("Candidate Genesis"));

    let Some((_, _, _, opt_climate, opt_temp, opt_volatiles)) = candidate else {
        if let Some(obj) = state
            .objectives
            .iter_mut()
            .find(|o| o.id == "genesis_biosphere")
        {
            obj.is_failed = true;
        }
        state.failure_reason = Some("Candidate Genesis was destroyed or ejected!".to_string());
        return;
    };

    let temp_k = opt_climate.map_or_else(
        || opt_temp.map_or(160.0, |t| t.0),
        |c| f64::from(c.surface_temperature_k),
    );
    let water_frac = opt_volatiles.map_or_else(
        || opt_climate.map_or(0.0, |c| f64::from((1.0 - c.ice_coverage_frac).max(0.0))),
        |v| f64::from(v.ocean_coverage_frac),
    );

    let is_temp_habitable = (273.0..=320.0).contains(&temp_k);
    if let Some(obj) = state.objectives.iter_mut().find(|o| o.id == "genesis_temp") {
        obj.current_value = temp_k;
        if is_temp_habitable {
            obj.is_completed = true;
        }
    }

    let is_water_habitable = water_frac >= 0.20;
    if let Some(obj) = state
        .objectives
        .iter_mut()
        .find(|o| o.id == "genesis_oceans")
    {
        obj.current_value = water_frac;
        if is_water_habitable {
            obj.is_completed = true;
        }
    }

    if is_temp_habitable && is_water_habitable {
        state.stable_biosphere_years += dt_yr;
    }

    if let Some(obj) = state
        .objectives
        .iter_mut()
        .find(|o| o.id == "genesis_biosphere")
    {
        obj.current_value = state.stable_biosphere_years;
        if state.stable_biosphere_years >= obj.target_value {
            obj.is_completed = true;
        }
    }
}

fn eval_grand_tour(
    state: &mut CampaignState,
    probes_query: &Query<(Entity, &CelestialBody, &SimPosition, &SpaceProbe)>,
    bodies_query: &Query<(
        Entity,
        &CelestialBody,
        &SimPosition,
        Option<&PlanetaryClimate>,
        Option<&Temperature>,
        Option<&VolatileInventory>,
    )>,
) {
    let voyager = probes_query
        .iter()
        .find(|(_, body, _, _)| body.name.contains("Voyager"));

    let Some((_, _, probe_pos, _)) = voyager else {
        if let Some(obj) = state
            .objectives
            .iter_mut()
            .find(|o| o.id == "tour_probe_survival")
        {
            obj.is_failed = true;
        }
        state.failure_reason = Some("Voyager III probe was destroyed or lost!".to_string());
        return;
    };

    // Flyby proximity threshold is 0.50 AU
    for (planet_ent, planet_body, planet_pos, _, _, _) in bodies_query.iter() {
        if planet_body.name.starts_with("Planet ") {
            let dist = (probe_pos.0 - planet_pos.0).length();
            if dist <= 0.50 {
                state.flyby_visited_entities.insert(planet_ent);
            }
        }
    }

    let flyby_count = state.flyby_visited_entities.len();
    let flyby_done = flyby_count >= 4;
    if let Some(obj) = state.objectives.iter_mut().find(|o| o.id == "tour_flybys") {
        obj.current_value = flyby_count as f64;
        if flyby_done {
            obj.is_completed = true;
        }
    }

    if let Some(obj) = state
        .objectives
        .iter_mut()
        .find(|o| o.id == "tour_probe_survival")
    {
        obj.current_value = 1.0;
        if flyby_done {
            obj.is_completed = true;
        }
    }
}

fn eval_exodus(
    state: &mut CampaignState,
    stars_query: &Query<(&Mass, &IgnitionState, Option<&StellarEvolutionState>), With<CentralStar>>,
    probes_query: &Query<(Entity, &CelestialBody, &SimPosition, &SpaceProbe)>,
) {
    // 1. Count escaped ships (> 200 AU)
    let escaped_count = probes_query
        .iter()
        .filter(|(_, body, pos, _)| {
            body.name.contains("Ark Colony Ship") && pos.0.length() >= 200.0
        })
        .count();

    state.evacuated_probe_count = escaped_count;
    if let Some(obj) = state
        .objectives
        .iter_mut()
        .find(|o| o.id == "exodus_evacuate")
    {
        obj.current_value = escaped_count as f64;
        if escaped_count >= obj.target_value as usize {
            obj.is_completed = true;
        }
    }

    // 2. Check if supernova occurred before evacuation completed
    let is_supernova = stars_query.iter().any(|(_, _, opt_evo)| {
        opt_evo.is_some_and(|e| {
            matches!(
                e.phase,
                StellarEvolutionPhase::SupernovaExplosion
                    | StellarEvolutionPhase::NeutronStarPulsar
                    | StellarEvolutionPhase::MagnetarRemnant
                    | StellarEvolutionPhase::BlackHoleRemnant
            )
        })
    });

    let evacuation_done = state
        .objectives
        .iter()
        .any(|o| o.id == "exodus_evacuate" && o.is_completed);

    if let Some(obj) = state
        .objectives
        .iter_mut()
        .find(|o| o.id == "exodus_outrun_supernova")
    {
        if is_supernova && !evacuation_done {
            obj.is_failed = true;
            state.failure_reason = Some(
                "The red supergiant exploded before the colony ships escaped beyond 200 AU!"
                    .to_string(),
            );
        } else if evacuation_done {
            obj.current_value = 1.0;
            obj.is_completed = true;
        }
    }
}

fn check_mission_completion(
    mission: CampaignMissionId,
    state: &mut CampaignState,
    science_data: &mut ScienceData,
    complete_events: &mut MessageWriter<CampaignMissionCompletedEvent>,
    fail_events: &mut MessageWriter<CampaignMissionFailedEvent>,
) {
    let has_failure = state.objectives.iter().any(|o| o.is_failed);
    let all_completed = state
        .objectives
        .iter()
        .all(|o| !o.is_mandatory || o.is_completed);

    if has_failure && state.status != MissionStatus::Defeat {
        state.status = MissionStatus::Defeat;
        let reason = state
            .failure_reason
            .clone()
            .unwrap_or_else(|| "Mission objective failed".to_string());
        fail_events.write(CampaignMissionFailedEvent { mission, reason });
    } else if all_completed && state.status != MissionStatus::Victory {
        state.status = MissionStatus::Victory;
        state.completed_missions.insert(mission);

        if !state.reward_claimed {
            let reward = mission.reward_science();
            science_data.points += reward;
            science_data.lifetime_points += reward;
            state.reward_claimed = true;

            complete_events.write(CampaignMissionCompletedEvent {
                mission,
                science_reward: reward,
            });
        }
    }
}
