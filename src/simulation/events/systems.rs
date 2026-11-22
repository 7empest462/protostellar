//! Lifecycle evaluation, physical impacts, and sensory feedback for Cosmic Hazards.

use bevy::math::DVec3;
use bevy::prelude::*;

use super::crisis_response::{
    CrisisImpactor, CrisisImpactorQuery, CrisisInterceptorPool, CrisisWorldTargetQuery,
    apply_great_filter_crisis_step, apply_planet_killer_swarm_step, evaluate_crisis_outcome,
    resolve_primary_crisis_target, spawn_planet_killer_swarm,
};
use super::spawner::*;
use super::types::*;
use crate::game::ui::types::NotificationToast;
use crate::simulation::biology::CivilizationState;
use crate::simulation::components::*;
use crate::simulation::economy::{ProgressionState, ScienceData, TechnologyId, UnlockedTechs};
use crate::simulation::resources::*;
use crate::simulation::space_weather::types::{
    AuroralOvalState, AuroralStormExcitation, GeomagneticStormLevel,
};

/// Schedules stochastic hazards or ingests manual trigger messages.
#[allow(
    clippy::too_many_arguments,
    reason = "Schedules manual triggers and periodic auto-hazards with tech-enhanced early warning"
)]
pub fn cosmic_event_scheduler_system(
    mut hazard_mgr: ResMut<CosmicEventManager>,
    mut trigger_reader: MessageReader<TriggerCosmicEvent>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    opt_techs: Option<Res<UnlockedTechs>>,
    civ_query: Query<(), With<CivilizationState>>,
) {
    for msg in trigger_reader.read() {
        hazard_mgr.trigger_hazard(msg.0, 1.0);
    }

    if !hazard_mgr.auto_hazards_enabled || (time_warp.is_paused && !time_warp.step_once) {
        return;
    }

    let dt_years = sim_time.current_dt_yr;
    hazard_mgr.hazard_timer_years += dt_years;

    if hazard_mgr.hazard_timer_years >= hazard_mgr.hazard_interval_years {
        hazard_mgr.hazard_timer_years = 0.0;
        let event_cycle = [
            CosmicEventType::RogueStarIncursion,
            CosmicEventType::GammaRayBurst,
            CosmicEventType::CometOutburstSwarm,
            CosmicEventType::SolarSuperflare,
            CosmicEventType::PlanetKillerSwarm,
            CosmicEventType::GreatFilterCrisis,
        ];
        let idx = (hazard_mgr.total_hazards_survived as usize) % event_cycle.len();
        if let Some(&raw_hazard) = event_cycle.get(idx) {
            let hazard = if raw_hazard == CosmicEventType::GreatFilterCrisis && civ_query.is_empty()
            {
                CosmicEventType::PlanetKillerSwarm
            } else {
                raw_hazard
            };
            let has_lens = opt_techs
                .as_deref()
                .is_some_and(|t| t.has(TechnologyId::GravitationalLensArray));
            let warning_yr = if has_lens { 8.0 } else { 4.0 };
            hazard_mgr.trigger_hazard_with_warning(hazard, 1.0, warning_yr);
        }
    }
}

/// Planetary query used by Gamma-Ray Burst atmospheric stripping.
pub type GrbPlanetQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut VolatileInventory,
        Option<&'static mut PlanetaryClimate>,
        Option<&'static mut BiosphereState>,
        Option<&'static ElectromagneticFieldState>,
        Option<&'static CivilizationState>,
    ),
    (Without<HazardComet>, Without<CrisisImpactor>),
>;

/// Applies physical impacts of a Gamma-Ray Burst (ionizing atmospheric stripping and biosphere damage).
fn apply_grb_impacts(
    planets: &mut GrbPlanetQuery,
    has_mag_shield_tech: bool,
    dt_years: f64,
) -> bool {
    let decay_factor = (1.0 - 0.08 * (dt_years.min(0.2) as f32)).clamp(0.0, 1.0);
    let bio_loss = 0.15 * (dt_years.clamp(0.0, 0.2) as f32);
    let mut active_shield_mitigated = false;
    for (mut volatiles, _, mut opt_bio, opt_field, opt_civ) in planets.iter_mut() {
        let local_shield = opt_field.is_some_and(|f| f.magnetic_field_gauss >= 0.1)
            || opt_civ.is_some_and(|c| c.shield_overdrive_secs > 0.0);
        if has_mag_shield_tech || local_shield {
            if has_mag_shield_tech || opt_civ.is_some_and(|c| c.shield_overdrive_secs > 0.0) {
                active_shield_mitigated = true;
            }
            continue;
        }
        volatiles.atmospheric_pressure_bar *= decay_factor;
        if let Some(ref mut bio) = opt_bio {
            bio.technosignature = (bio.technosignature - bio_loss).max(0.0);
        }
    }
    active_shield_mitigated
}

/// Applies physical impacts of a Solar Superflare (G5 auroras and space probe telemetry disruptions).
fn apply_superflare_impacts(
    auroras: &mut Query<(&mut AuroralOvalState, Option<&mut AuroralStormExcitation>)>,
    probes: &mut Query<&mut crate::simulation::probes::FlightComputer>,
    has_mag_shield_tech: bool,
    has_shield_overdrive: bool,
) -> bool {
    for (mut oval, opt_exc) in auroras.iter_mut() {
        oval.geomagnetic_kp_index = 9.0;
        oval.storm_level = GeomagneticStormLevel::ExtremeStorm;
        oval.auroral_intensity = 3.5;
        if let Some(mut exc) = opt_exc {
            exc.excitation = exc.excitation.max(3.8);
        }
    }
    let dampen = if has_mag_shield_tech || has_shield_overdrive {
        0.998
    } else {
        0.98
    };
    for mut flight_comp in probes.iter_mut() {
        flight_comp.relative_speed = (flight_comp.relative_speed * dampen).max(0.0);
    }
    has_mag_shield_tech || has_shield_overdrive
}

pub type CometDeliveryTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimPosition,
        Option<&'static SimVelocity>,
        &'static mut VolatileInventory,
        &'static Radius,
        Option<&'static CelestialBody>,
    ),
    (Without<HazardComet>, Without<CrisisImpactor>),
>;

pub type HazardCometQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut SimPosition,
        Option<&'static mut SimVelocity>,
        &'static VolatileInventory,
    ),
    (
        With<HazardComet>,
        Without<CrisisImpactor>,
        Without<RogueInterloper>,
    ),
>;

/// Collects candidate rocky/terrestrial target planets sorted with Earth first.
fn collect_comet_target_worlds(targets: &CometDeliveryTargetQuery) -> Vec<(Entity, DVec3, DVec3)> {
    let mut list: Vec<(i32, Entity, DVec3, DVec3)> = Vec::new();
    for (ent, pos, opt_vel, _, _, opt_body) in targets.iter() {
        if opt_body.is_some_and(|b| b.body_type.is_star_or_remnant()) {
            continue;
        }
        let priority = if let Some(b) = opt_body {
            if (b.name == "Earth" || b.name.starts_with("Proto-Earth") || b.name.contains("Earth"))
                && !b.name.contains("Super-Earth")
                && !b.name.contains("Planet Nine")
            {
                0
            } else if matches!(
                b.body_type,
                BodyType::TerrestrialPlanet | BodyType::Protoplanet | BodyType::SuperEarth
            ) && pos.0.length() < 2.2
            {
                1
            } else if b.body_type.is_planet() {
                2
            } else {
                3
            }
        } else {
            1
        };
        let vel = opt_vel.map_or(DVec3::ZERO, |v| v.0);
        list.push((priority, ent, pos.0, vel));
    }
    list.sort_by_key(|(prio, ..)| *prio);
    list.into_iter().map(|(_, e, p, v)| (e, p, v)).collect()
}

/// Guides outburst comets into Earth and inner rocky worlds, delivering water oceans and impact craters.
pub fn apply_comet_delivery(
    commands: &mut Commands,
    comets: &mut HazardCometQuery,
    targets: &mut CometDeliveryTargetQuery,
    dt_years: f64,
    elapsed_years: f64,
) {
    let dt = dt_years.max(1e-5);
    for (comet_ent, mut comet_pos, mut opt_comet_vel, comet_vol) in comets.iter_mut() {
        let mut hit_occurred = false;
        let mut best_target: Option<(DVec3, DVec3, f64, i32)> = None;

        for (planet_ent, planet_pos, opt_p_vel, mut planet_vol, rad, opt_body) in targets.iter_mut()
        {
            if opt_body.is_some_and(|b| b.body_type.is_star_or_remnant()) {
                continue;
            }
            let p_vel = opt_p_vel.map_or(DVec3::ZERO, |v| v.0);
            let c_vel = opt_comet_vel.as_ref().map_or(DVec3::ZERO, |v| v.0);
            let disp = comet_pos.0 - planet_pos.0;
            let dist_au = disp.length();
            let encounter_threshold = (rad.0 * 8.0).max(0.05);

            let v_rel = c_vel - p_vel;
            let step_disp = v_rel * dt;
            let seg_len_sq = step_disp.length_squared();
            let swept_dist = if seg_len_sq > 1e-12 {
                let p_prev = disp - step_disp;
                let u = (-p_prev.dot(step_disp) / seg_len_sq).clamp(0.0, 1.0);
                (p_prev + step_disp * u).length()
            } else {
                dist_au
            };

            if dist_au < encounter_threshold || swept_dist < encounter_threshold {
                let d_water = (comet_vol.delivered_water_m_earth * 0.15).max(0.00018);
                planet_vol.delivered_water_m_earth += d_water;
                planet_vol.cometary_impact_count += 1;
                let cov_from_water =
                    (planet_vol.delivered_water_m_earth / 0.0005).clamp(0.0, 0.92) as f32;
                planet_vol.ocean_coverage_frac = (planet_vol.ocean_coverage_frac + 0.14)
                    .max(cov_from_water)
                    .clamp(0.0, 0.98);
                planet_vol.atmospheric_pressure_bar =
                    (planet_vol.atmospheric_pressure_bar + 0.08).clamp(0.01, 90.0);

                let norm = disp.normalize_or_zero().as_vec3();
                crate::simulation::accretion::basins::record_impact_crater_basin(
                    commands,
                    planet_ent,
                    norm,
                    0.12,
                    elapsed_years,
                    1.0,
                );
                if let Ok(mut c_cmd) = commands.get_entity(comet_ent) {
                    c_cmd.try_despawn();
                }
                hit_occurred = true;
                break;
            }

            let is_earth = opt_body.is_some_and(|b| {
                (b.name == "Earth" || b.name.starts_with("Proto-Earth") || b.name.contains("Earth"))
                    && !b.name.contains("Super-Earth")
                    && !b.name.contains("Planet Nine")
            });
            let prio = i32::from(!is_earth);
            let replace = match best_target {
                None => true,
                Some((_, _, best_d, best_p)) => {
                    prio < best_p || (prio == best_p && dist_au < best_d)
                }
            };
            if replace {
                best_target = Some((planet_pos.0, p_vel, dist_au, prio));
            }
        }

        if !hit_occurred && let Some((t_pos, t_vel, dist_au, _)) = best_target {
            let dir_to_target = (t_pos - comet_pos.0).normalize_or_zero();
            let desired_closing = (dist_au * 8.0).clamp(4.5, 12.0);
            let desired_vel = t_vel + dir_to_target * desired_closing;
            if let Some(ref mut c_vel) = opt_comet_vel {
                let blend = (dt / 0.08).clamp(0.25, 0.85);
                c_vel.0 = c_vel.0.lerp(desired_vel, blend);
            } else {
                comet_pos.0 += dir_to_target * (desired_closing * dt).min(dist_au * 0.9);
            }
        }
    }
}

/// Spawns required entities and dispatches notifications when a hazard initiates.
fn initialize_active_event(
    event: &mut ActiveCosmicEvent,
    commands: &mut Commands,
    opt_toast: Option<&mut NotificationToast>,
    started_events: &mut MessageWriter<CosmicEventStartedEvent>,
    hazard_mgr: &mut CosmicEventManager,
    comet_targets: &[(Entity, DVec3, DVec3)],
    primary_target: Option<&(Entity, String, DVec3, DVec3)>,
) {
    if event.target_entity.is_none()
        && let Some((t_ent, t_name, _, _)) = primary_target
    {
        event.target_entity = Some(*t_ent);
        event.target_name.clone_from(t_name);
    }

    match event.event_type {
        CosmicEventType::RogueStarIncursion => {
            let ent = spawn_rogue_star(commands, event.intensity);
            event.associated_entities.push(ent);
            event.threats_total = 1;
            hazard_mgr.hazard_flash_intensity = 0.35;
        }
        CosmicEventType::GammaRayBurst => {
            hazard_mgr.hazard_flash_intensity = 1.0;
        }
        CosmicEventType::CometOutburstSwarm => {
            let ents = spawn_comet_outburst_swarm_targeted(commands, 6, comet_targets);
            event.associated_entities.extend(ents);
            hazard_mgr.hazard_flash_intensity = 0.25;
        }
        CosmicEventType::SolarSuperflare => {
            hazard_mgr.hazard_flash_intensity = 0.85;
        }
        CosmicEventType::PlanetKillerSwarm => {
            let (t_ent, t_name, t_pos, t_vel) = match primary_target {
                Some((e, n, p, v)) => (Some(*e), n.as_str(), *p, *v),
                None => (None, "Inner World", DVec3::new(1.0, 0.0, 0.0), DVec3::ZERO),
            };
            let ents = spawn_planet_killer_swarm(commands, 4, t_ent, t_name, t_pos, t_vel);
            event.threats_total = ents.len() as u32;
            event.associated_entities.extend(ents);
            hazard_mgr.hazard_flash_intensity = 0.45;
        }
        CosmicEventType::GreatFilterCrisis => {
            hazard_mgr.hazard_flash_intensity = 0.55;
        }
    }

    if let Some(toast) = opt_toast {
        toast.message = format!(
            "⚠️ COSMIC CRISIS // {}! {}",
            event.event_type.title(),
            event.event_type.description()
        );
        toast.timer = 5.0;
    }

    if !event.notification_sent {
        started_events.write(CosmicEventStartedEvent {
            event_type: event.event_type,
            description: event.event_type.description().to_string(),
            target_entity: event.target_entity,
        });
        event.notification_sent = true;
    }
    event.phase = CosmicEventPhase::Active;
}

/// Finalizes a hazard event, granting scaled science bounties, updating stats, and cleaning up.
#[allow(
    clippy::too_many_arguments,
    reason = "Resolves crisis rewards and progression counters"
)]
fn resolve_active_event(
    event: &mut ActiveCosmicEvent,
    commands: &mut Commands,
    opt_toast: Option<&mut NotificationToast>,
    opt_science: Option<&mut ScienceData>,
    opt_prog: Option<&mut ProgressionState>,
    resolved_events: &mut MessageWriter<CosmicEventResolvedEvent>,
    sim_time_years: f64,
    has_lens_array: bool,
    has_solar_sail: bool,
) {
    for ent in &event.associated_entities {
        if let Ok(mut cmd) = commands.get_entity(*ent) {
            cmd.try_despawn();
        }
    }
    event.associated_entities.clear();

    let outcome = evaluate_crisis_outcome(event);
    event.outcome = Some(outcome);

    let lens_mult = if has_lens_array { 1.5 } else { 1.0 };
    let sail_mult = if has_solar_sail && event.event_type == CosmicEventType::SolarSuperflare {
        1.35
    } else {
        1.0
    };
    let bounty =
        (event.event_type.science_bounty() * outcome.bounty_multiplier() * lens_mult * sail_mult)
            .round();

    if let Some(prog) = opt_prog {
        match outcome {
            CrisisOutcome::Averted => prog.crises_averted += 1,
            CrisisOutcome::Mitigated => prog.crises_mitigated += 1,
            CrisisOutcome::Catastrophic => prog.crises_catastrophic += 1,
            CrisisOutcome::Survived => {}
        }
    }

    if let Some(science) = opt_science {
        let title = if event.target_name.is_empty() {
            format!("{}: {}", outcome.label(), event.event_type.title())
        } else {
            format!(
                "{} ({}): {}",
                outcome.label(),
                event.event_type.title(),
                event.target_name
            )
        };
        science.record_discovery(
            title,
            event.event_type.description(),
            bounty,
            sim_time_years,
        );
    }

    if let Some(toast) = opt_toast {
        toast.message = format!(
            "✅ CRISIS {} // {} (+{:.0} Science Data)",
            outcome.label(),
            event.event_type.title(),
            bounty
        );
        toast.timer = 4.5;
    }

    resolved_events.write(CosmicEventResolvedEvent {
        event_type: event.event_type,
        science_reward: bounty,
        outcome,
        target_entity: event.target_entity,
    });

    event.phase = CosmicEventPhase::Completed;
}

/// Combined planetary query set cleanly segmented to prevent ECS aliasing across hazard handlers.
#[derive(bevy::ecs::system::SystemParam)]
pub struct CosmicLifecycleTargets<'w, 's> {
    #[allow(
        clippy::type_complexity,
        reason = "ParamSet disambiguates overlapping planet queries for cosmic hazard phases"
    )]
    pub sets: ParamSet<
        'w,
        's,
        (
            GrbPlanetQuery<'w, 's>,
            CometDeliveryTargetQuery<'w, 's>,
            CrisisWorldTargetQuery<'w, 's>,
        ),
    >,
}

fn advance_warning_phase(
    hazard_mgr: &mut CosmicEventManager,
    primary_target: Option<&(Entity, String, DVec3, DVec3)>,
    mut opt_toast: Option<&mut NotificationToast>,
    started_events: &mut MessageWriter<CosmicEventStartedEvent>,
    dt_years: f64,
) {
    for event in &mut hazard_mgr.active_events {
        if event.phase != CosmicEventPhase::Warning {
            continue;
        }
        if event.target_entity.is_none()
            && let Some((t_ent, t_name, _, _)) = primary_target
        {
            event.target_entity = Some(*t_ent);
            event.target_name.clone_from(t_name);
        }
        if !event.notification_sent {
            if let Some(ref mut toast) = opt_toast {
                toast.message = format!(
                    "🚨 CRISIS WARNING ({:.1} yr) // {} inbound!",
                    event.warning_remaining_years,
                    event.event_type.title()
                );
                toast.timer = 5.0;
            }
            started_events.write(CosmicEventStartedEvent {
                event_type: event.event_type,
                description: event.event_type.description().to_string(),
                target_entity: event.target_entity,
            });
            event.notification_sent = true;
        }
        event.warning_remaining_years = (event.warning_remaining_years - dt_years).max(0.0);
        if event.warning_remaining_years <= 0.0 {
            event.phase = CosmicEventPhase::Initializing;
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Resolves finished crisis events cleanly"
)]
fn finalize_finished_crises(
    hazard_mgr: &mut CosmicEventManager,
    to_resolve: &[usize],
    commands: &mut Commands,
    mut opt_toast: Option<&mut NotificationToast>,
    mut opt_science: Option<&mut ScienceData>,
    mut opt_prog: Option<&mut ProgressionState>,
    resolved_events: &mut MessageWriter<CosmicEventResolvedEvent>,
    elapsed_years: f64,
    has_lens: bool,
    has_solar_sail: bool,
) -> bool {
    let mut had_non_catastrophic = false;
    for &idx in to_resolve {
        if let Some(event_slot) = hazard_mgr.active_events.get_mut(idx) {
            let mut event = event_slot.clone();
            resolve_active_event(
                &mut event,
                commands,
                opt_toast.as_deref_mut(),
                opt_science.as_deref_mut(),
                opt_prog.as_deref_mut(),
                resolved_events,
                elapsed_years,
                has_lens,
                has_solar_sail,
            );
            hazard_mgr.total_hazards_survived += 1;
            if event.outcome != Some(CrisisOutcome::Catastrophic) {
                had_non_catastrophic = true;
            }
            if let Some(slot) = hazard_mgr.active_events.get_mut(idx) {
                *slot = event;
            }
        }
    }
    hazard_mgr
        .active_events
        .retain(|e| e.phase != CosmicEventPhase::Completed);
    had_non_catastrophic
}

#[allow(
    clippy::too_many_arguments,
    reason = "Advances all active crises for one simulation tick"
)]
fn step_active_crises(
    hazard_mgr: &mut CosmicEventManager,
    commands: &mut Commands,
    target_sets: &mut CosmicLifecycleTargets,
    auroral_query: &mut Query<(&mut AuroralOvalState, Option<&mut AuroralStormExcitation>)>,
    probe_query: &mut Query<&mut crate::simulation::probes::FlightComputer>,
    comet_query: &mut HazardCometQuery,
    impactor_query: &mut CrisisImpactorQuery,
    interloper_query: &Query<
        (Entity, &SimPosition),
        (
            With<RogueInterloper>,
            Without<HazardComet>,
            Without<CrisisImpactor>,
        ),
    >,
    interceptor_pool: &mut Option<ResMut<CrisisInterceptorPool>>,
    opt_science: &mut Option<ResMut<ScienceData>>,
    tech_flags: (bool, bool, bool, bool, bool),
    dt_years: f64,
    elapsed_years: f64,
) -> Vec<usize> {
    let (has_defense_grid, has_mag_shield, has_mirror, has_ascension, has_shield_overdrive) =
        tech_flags;
    let mut to_resolve = Vec::new();
    for idx in 0..hazard_mgr.active_events.len() {
        let Some(mut event) = hazard_mgr.active_events.get(idx).cloned() else {
            continue;
        };
        if event.phase != CosmicEventPhase::Active {
            continue;
        }
        event.elapsed_years += dt_years;
        match event.event_type {
            CosmicEventType::GammaRayBurst => {
                if apply_grb_impacts(&mut target_sets.sets.p0(), has_mag_shield, dt_years) {
                    event.mitigation_score = (event.mitigation_score
                        + 0.45 * (dt_years.max(0.05) as f32))
                        .clamp(0.0, 1.0);
                }
            }
            CosmicEventType::SolarSuperflare => {
                if apply_superflare_impacts(
                    auroral_query,
                    probe_query,
                    has_mag_shield,
                    has_shield_overdrive,
                ) {
                    event.mitigation_score = (event.mitigation_score
                        + 0.35 * (dt_years.max(0.05) as f32))
                        .clamp(0.0, 1.0);
                }
            }
            CosmicEventType::CometOutburstSwarm => apply_comet_delivery(
                commands,
                comet_query,
                &mut target_sets.sets.p1(),
                dt_years,
                elapsed_years,
            ),
            CosmicEventType::RogueStarIncursion => {
                if let Some((_, pos)) = interloper_query.iter().next()
                    && pos.0.length() > 150.0
                    && event.elapsed_years > 5.0
                {
                    to_resolve.push(idx);
                }
            }
            CosmicEventType::PlanetKillerSwarm => {
                apply_planet_killer_swarm_step(
                    commands,
                    &mut event,
                    impactor_query,
                    &mut target_sets.sets.p2(),
                    interceptor_pool,
                    has_defense_grid,
                    opt_science.as_deref_mut(),
                    dt_years,
                    elapsed_years,
                );
                if event.threats_total > 0
                    && event.threats_neutralized >= event.threats_total
                    && event.impacts_suffered == 0
                {
                    to_resolve.push(idx);
                }
            }
            CosmicEventType::GreatFilterCrisis => {
                apply_great_filter_crisis_step(
                    &mut event,
                    &mut target_sets.sets.p2(),
                    has_mirror,
                    has_ascension,
                    dt_years,
                );
                if event.mitigation_score >= 0.95 && event.impacts_suffered == 0 {
                    to_resolve.push(idx);
                }
            }
        }
        if event.elapsed_years >= event.duration_years && !to_resolve.contains(&idx) {
            to_resolve.push(idx);
        }
        if let Some(slot) = hazard_mgr.active_events.get_mut(idx) {
            *slot = event;
        }
    }
    to_resolve
}

/// Main evaluation system managing hazard warning, active physics, defense, and resolution.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Cosmic hazard lifecycle evaluates across simulation components, toasts, progression, and science economy"
)]
pub fn cosmic_event_lifecycle_system(
    mut commands: Commands,
    mut hazard_mgr: ResMut<CosmicEventManager>,
    (time_warp, sim_time): (Res<TimeWarp>, Res<SimTime>),
    opt_techs: Option<Res<UnlockedTechs>>,
    mut opt_prog: Option<ResMut<ProgressionState>>,
    mut opt_toast: Option<ResMut<NotificationToast>>,
    mut opt_science: Option<ResMut<ScienceData>>,
    mut interceptor_pool: Option<ResMut<CrisisInterceptorPool>>,
    (mut started_events, mut resolved_events): (
        MessageWriter<CosmicEventStartedEvent>,
        MessageWriter<CosmicEventResolvedEvent>,
    ),
    mut target_sets: CosmicLifecycleTargets,
    mut auroral_query: Query<(&mut AuroralOvalState, Option<&mut AuroralStormExcitation>)>,
    mut probe_query: Query<&mut crate::simulation::probes::FlightComputer>,
    mut comet_query: HazardCometQuery,
    mut impactor_query: CrisisImpactorQuery,
    interloper_query: Query<
        (Entity, &SimPosition),
        (
            With<RogueInterloper>,
            Without<HazardComet>,
            Without<CrisisImpactor>,
        ),
    >,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }
    let dt_years = sim_time.current_dt_yr;

    let has = |id| opt_techs.as_deref().is_some_and(|t| t.has(id));
    let (has_defense_grid, has_mag_shield, has_mirror, has_lens, has_solar_sail, has_ascension) = (
        has(TechnologyId::PlanetaryDefenseGrid),
        has(TechnologyId::MagnetosphericShielding),
        has(TechnologyId::OrbitalMirrorArray),
        has(TechnologyId::GravitationalLensArray),
        has(TechnologyId::SolarSail),
        has(TechnologyId::KardashevAscension),
    );

    let primary_target = resolve_primary_crisis_target(target_sets.sets.p2().iter().map(
        |(e, pos, opt_v, _, opt_b, opt_civ, ..)| {
            (
                e,
                opt_b.map_or("World", |b| b.name.as_str()),
                pos.0,
                opt_v.map_or(DVec3::ZERO, |v| v.0),
                opt_civ.map(|c| c.kardashev_level),
                opt_b.map(|b| b.body_type),
            )
        },
    ));
    let has_shield_overdrive = target_sets
        .sets
        .p2()
        .iter()
        .any(|(_, _, _, _, _, opt_civ, ..)| opt_civ.is_some_and(|c| c.shield_overdrive_secs > 0.0));

    advance_warning_phase(
        &mut hazard_mgr,
        primary_target.as_ref(),
        opt_toast.as_deref_mut(),
        &mut started_events,
        dt_years,
    );

    let to_init: Vec<usize> = hazard_mgr
        .active_events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| (e.phase == CosmicEventPhase::Initializing).then_some(i))
        .collect();
    let comet_targets = if to_init.is_empty() {
        Vec::new()
    } else {
        collect_comet_target_worlds(&target_sets.sets.p1())
    };
    for idx in to_init {
        if let Some(event_slot) = hazard_mgr.active_events.get_mut(idx) {
            let mut event = event_slot.clone();
            initialize_active_event(
                &mut event,
                &mut commands,
                opt_toast.as_deref_mut(),
                &mut started_events,
                &mut hazard_mgr,
                &comet_targets,
                primary_target.as_ref(),
            );
            if let Some(slot) = hazard_mgr.active_events.get_mut(idx) {
                *slot = event;
            }
        }
    }

    let to_resolve = step_active_crises(
        &mut hazard_mgr,
        &mut commands,
        &mut target_sets,
        &mut auroral_query,
        &mut probe_query,
        &mut comet_query,
        &mut impactor_query,
        &interloper_query,
        &mut interceptor_pool,
        &mut opt_science,
        (
            has_defense_grid,
            has_mag_shield,
            has_mirror,
            has_ascension,
            has_shield_overdrive,
        ),
        dt_years,
        sim_time.elapsed_years,
    );

    if finalize_finished_crises(
        &mut hazard_mgr,
        &to_resolve,
        &mut commands,
        opt_toast.as_deref_mut(),
        opt_science.as_deref_mut(),
        opt_prog.as_deref_mut(),
        &mut resolved_events,
        sim_time.elapsed_years,
        has_lens,
        has_solar_sail,
    ) {
        for (_, _, _, _, _, mut opt_civ, ..) in target_sets.sets.p2().iter_mut() {
            if let Some(ref mut civ) = opt_civ
                && civ.stability > 0.05
            {
                civ.crises_survived += 1;
            }
        }
    }
}

/// Sensory post-processing system smoothly decaying screen shockwave flashes.
pub fn cosmic_event_sensory_system(
    mut hazard_mgr: ResMut<CosmicEventManager>,
    opt_time: Option<Res<Time>>,
    mut post_processing_query: Query<
        &mut crate::rendering::post_processing::PostProcessingSettings,
    >,
) {
    let dt = opt_time.map_or(0.016, |t| t.delta_secs());
    let decay_rate = 2.0;
    hazard_mgr.hazard_flash_intensity =
        (hazard_mgr.hazard_flash_intensity - dt * decay_rate).max(0.0);

    for mut settings in post_processing_query.iter_mut() {
        settings.tonemapping_mode = hazard_mgr.tonemapping_mode.shader_mode_id();
        settings.hazard_flash_intensity = hazard_mgr.hazard_flash_intensity;
    }
}
