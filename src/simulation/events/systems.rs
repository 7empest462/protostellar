//! Lifecycle evaluation, physical impacts, and sensory feedback for Cosmic Hazards.

use bevy::prelude::*;

use super::spawner::*;
use super::types::*;
use crate::game::ui::types::NotificationToast;
use crate::simulation::components::*;
use crate::simulation::economy::ScienceData;
use crate::simulation::resources::*;
use crate::simulation::space_weather::types::{AuroralOvalState, GeomagneticStormLevel};

/// Schedules stochastic hazards or ingests manual trigger messages.
pub fn cosmic_event_scheduler_system(
    mut hazard_mgr: ResMut<CosmicEventManager>,
    mut trigger_reader: MessageReader<TriggerCosmicEvent>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
) {
    for msg in trigger_reader.read() {
        hazard_mgr.trigger_hazard(msg.0, 1.0);
    }

    if !hazard_mgr.auto_hazards_enabled || time_warp.is_paused {
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
        ];
        let idx = (hazard_mgr.total_hazards_survived as usize) % event_cycle.len();
        if let Some(&hazard) = event_cycle.get(idx) {
            hazard_mgr.trigger_hazard(hazard, 1.0);
        }
    }
}

/// Applies physical impacts of a Gamma-Ray Burst (ionizing atmospheric stripping and biosphere damage).
fn apply_grb_impacts(
    planets: &mut Query<
        (
            &mut VolatileInventory,
            Option<&mut PlanetaryClimate>,
            Option<&mut BiosphereState>,
            Option<&ElectromagneticFieldState>,
        ),
        Without<HazardComet>,
    >,
    dt_years: f64,
) {
    let decay_factor = (1.0 - 0.08 * (dt_years.min(0.2) as f32)).clamp(0.0, 1.0);
    for (mut volatiles, _, mut opt_bio, opt_field) in planets.iter_mut() {
        let is_shielded = opt_field.is_some_and(|f| f.magnetic_field_gauss >= 0.1);
        if !is_shielded {
            volatiles.atmospheric_pressure_bar *= decay_factor;
            if let Some(ref mut bio) = opt_bio {
                bio.technosignature = (bio.technosignature - 0.15).max(0.0);
            }
        }
    }
}

/// Applies physical impacts of a Solar Superflare (G5 auroras and space probe telemetry disruptions).
fn apply_superflare_impacts(
    auroras: &mut Query<&mut AuroralOvalState>,
    probes: &mut Query<&mut crate::simulation::probes::FlightComputer>,
) {
    for mut oval in auroras.iter_mut() {
        oval.geomagnetic_kp_index = 9.0;
        oval.storm_level = GeomagneticStormLevel::ExtremeStorm;
        oval.auroral_intensity = 3.5;
    }
    for mut flight_comp in probes.iter_mut() {
        flight_comp.relative_speed = (flight_comp.relative_speed * 0.98).max(0.0);
    }
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
    Without<HazardComet>,
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
    With<HazardComet>,
>;

/// Collects candidate rocky/terrestrial target planets sorted with Earth first.
fn collect_comet_target_worlds(
    targets: &CometDeliveryTargetQuery,
) -> Vec<(Entity, bevy::math::DVec3, bevy::math::DVec3)> {
    let mut list: Vec<(i32, Entity, bevy::math::DVec3, bevy::math::DVec3)> = Vec::new();
    for (ent, pos, opt_vel, _, _, opt_body) in targets.iter() {
        let is_star = opt_body.is_some_and(|b| b.body_type.is_star_or_remnant());
        if is_star {
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
        let vel = opt_vel.map_or(bevy::math::DVec3::ZERO, |v| v.0);
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
        let mut best_target: Option<(bevy::math::DVec3, bevy::math::DVec3, f64, i32)> = None;

        for (planet_ent, planet_pos, opt_p_vel, mut planet_vol, rad, opt_body) in targets.iter_mut()
        {
            if opt_body.is_some_and(|b| b.body_type.is_star_or_remnant()) {
                continue;
            }
            let p_vel = opt_p_vel.map_or(bevy::math::DVec3::ZERO, |v| v.0);
            let c_vel = opt_comet_vel
                .as_ref()
                .map_or(bevy::math::DVec3::ZERO, |v| v.0);
            let disp = comet_pos.0 - planet_pos.0;
            let dist_au = disp.length();
            let encounter_threshold = (rad.0 * 8.0).max(0.05);

            // Swept-segment closest approach so high time-warp never tunnels past Earth
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
                let cov_from_water = ((planet_vol.delivered_water_m_earth / 0.0006) * 0.71) as f32;
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

        if !hit_occurred {
            if let Some((t_pos, t_vel, dist_au, _)) = best_target {
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
}

/// Spawns required entities and dispatches notifications when a hazard initiates.
fn initialize_active_event(
    event: &mut ActiveCosmicEvent,
    commands: &mut Commands,
    opt_toast: Option<&mut NotificationToast>,
    started_events: &mut MessageWriter<CosmicEventStartedEvent>,
    hazard_mgr: &mut CosmicEventManager,
    comet_targets: &[(Entity, bevy::math::DVec3, bevy::math::DVec3)],
) {
    match event.event_type {
        CosmicEventType::RogueStarIncursion => {
            let ent = spawn_rogue_star(commands, event.intensity);
            event.associated_entities.push(ent);
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
    }

    if let Some(toast) = opt_toast {
        toast.message = format!(
            "⚠️ COSMIC HAZARD // {}! {}",
            event.event_type.title(),
            event.event_type.description()
        );
        toast.timer = 5.0;
    }

    started_events.write(CosmicEventStartedEvent {
        event_type: event.event_type,
        description: event.event_type.description().to_string(),
    });

    event.notification_sent = true;
    event.phase = CosmicEventPhase::Active;
}

/// Finalizes a hazard event, granting science bounties and cleaning up entities.
fn resolve_active_event(
    event: &mut ActiveCosmicEvent,
    commands: &mut Commands,
    opt_toast: Option<&mut NotificationToast>,
    opt_science: Option<&mut ScienceData>,
    resolved_events: &mut MessageWriter<CosmicEventResolvedEvent>,
    sim_time_years: f64,
) {
    for ent in &event.associated_entities {
        if let Ok(mut cmd) = commands.get_entity(*ent) {
            cmd.try_despawn();
        }
    }
    event.associated_entities.clear();

    let bounty = event.event_type.science_bounty();
    if let Some(science) = opt_science {
        science.record_discovery(
            format!("Survived: {}", event.event_type.title()),
            event.event_type.description(),
            bounty,
            sim_time_years,
        );
    }

    if let Some(toast) = opt_toast {
        toast.message = format!(
            "✅ COSMIC HAZARD RESOLVED // {} (+{:.0} Science Data)",
            event.event_type.title(),
            bounty
        );
        toast.timer = 4.5;
    }

    resolved_events.write(CosmicEventResolvedEvent {
        event_type: event.event_type,
        science_reward: bounty,
    });

    event.phase = CosmicEventPhase::Completed;
}

/// Main evaluation system managing hazard phases, physics interactions, and rewards.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Cosmic hazard lifecycle evaluates across simulation components, toasts, and science economy"
)]
pub fn cosmic_event_lifecycle_system(
    mut commands: Commands,
    mut hazard_mgr: ResMut<CosmicEventManager>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut opt_toast: Option<ResMut<NotificationToast>>,
    mut opt_science: Option<ResMut<ScienceData>>,
    mut started_events: MessageWriter<CosmicEventStartedEvent>,
    mut resolved_events: MessageWriter<CosmicEventResolvedEvent>,
    mut volatile_queries: ParamSet<(
        Query<
            (
                &mut VolatileInventory,
                Option<&mut PlanetaryClimate>,
                Option<&mut BiosphereState>,
                Option<&ElectromagneticFieldState>,
            ),
            Without<HazardComet>,
        >,
        CometDeliveryTargetQuery,
    )>,
    mut auroral_query: Query<&mut AuroralOvalState>,
    mut probe_query: Query<&mut crate::simulation::probes::FlightComputer>,
    mut comet_query: HazardCometQuery,
    interloper_query: Query<(Entity, &SimPosition), (With<RogueInterloper>, Without<HazardComet>)>,
) {
    if time_warp.is_paused {
        return;
    }
    let dt_years = sim_time.current_dt_yr;

    // Phase 1: Initialize pending events
    let mut to_init = Vec::new();
    for (idx, event) in hazard_mgr.active_events.iter().enumerate() {
        if event.phase == CosmicEventPhase::Initializing {
            to_init.push(idx);
        }
    }
    let comet_targets = if to_init.is_empty() {
        Vec::new()
    } else {
        let q1 = volatile_queries.p1();
        collect_comet_target_worlds(&q1)
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
            );
            if let Some(slot) = hazard_mgr.active_events.get_mut(idx) {
                *slot = event;
            }
        }
    }

    // Phase 2: Update active events and apply physics
    let mut to_resolve = Vec::new();
    for (idx, event) in hazard_mgr.active_events.iter_mut().enumerate() {
        if event.phase != CosmicEventPhase::Active {
            continue;
        }
        event.elapsed_years += dt_years;

        match event.event_type {
            CosmicEventType::GammaRayBurst => {
                let mut q0 = volatile_queries.p0();
                apply_grb_impacts(&mut q0, dt_years);
            }
            CosmicEventType::SolarSuperflare => {
                apply_superflare_impacts(&mut auroral_query, &mut probe_query);
            }
            CosmicEventType::CometOutburstSwarm => {
                let mut q1 = volatile_queries.p1();
                apply_comet_delivery(
                    &mut commands,
                    &mut comet_query,
                    &mut q1,
                    dt_years,
                    sim_time.elapsed_years,
                );
            }
            CosmicEventType::RogueStarIncursion => {
                if let Some((_, pos)) = interloper_query.iter().next() {
                    // If the rogue star has receded past 150 AU, conclude event
                    if pos.0.length() > 150.0 && event.elapsed_years > 5.0 {
                        to_resolve.push(idx);
                        continue;
                    }
                }
            }
        }

        if event.elapsed_years >= event.duration_years {
            to_resolve.push(idx);
        }
    }

    // Phase 3: Resolve finished events
    for idx in to_resolve {
        if let Some(event_slot) = hazard_mgr.active_events.get_mut(idx) {
            let mut event = event_slot.clone();
            resolve_active_event(
                &mut event,
                &mut commands,
                opt_toast.as_deref_mut(),
                opt_science.as_deref_mut(),
                &mut resolved_events,
                sim_time.elapsed_years,
            );
            hazard_mgr.total_hazards_survived += 1;
            if let Some(slot) = hazard_mgr.active_events.get_mut(idx) {
                *slot = event;
            }
        }
    }

    // Phase 4: Purge completed events
    hazard_mgr
        .active_events
        .retain(|e| e.phase != CosmicEventPhase::Completed);
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
