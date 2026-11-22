//! Planet-Killer swarms, Great Filter bottlenecks, and active planetary defense countermeasures.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::spawner::{HazardComet, RogueInterloper};
use super::types::{
    ActiveCosmicEvent, CosmicEventManager, CosmicEventPhase, CosmicEventType,
    CrisisCountermeasureEvent, CrisisCountermeasureKind, CrisisOutcome,
};
use crate::game::ui::types::NotificationToast;
use crate::simulation::accretion::basins::record_impact_crater_basin;
use crate::simulation::biology::{CivilizationState, technosignature_from_kardashev};
use crate::simulation::components::*;
use crate::simulation::economy::{
    ActionAccessResult, ProgressionState, ScienceData, TechnologyId, UnlockedTechs,
};
use crate::simulation::terraforming::TerraformingAtmosphere;

/// Component attached to incoming Chicxulub-scale impactors during [`CosmicEventType::PlanetKillerSwarm`].
#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrisisImpactor {
    #[serde(skip)]
    pub target_entity: Option<Entity>,
    pub target_name: String,
    pub deflected: bool,
    pub initial_inbound_dir: DVec3,
    pub closest_approach_au: f64,
}

/// Decaying visual beam rendered when planetary defense interceptors engage incoming threats.
#[derive(Debug, Clone)]
pub struct CrisisInterceptorBolt {
    pub start_au: DVec3,
    pub end_au: DVec3,
    pub color: Color,
    pub remaining_secs: f32,
    pub max_secs: f32,
}

/// Pool of active kinetic-deflector / railgun interceptor beams drawn in 3D space.
#[derive(Resource, Default, Debug, Clone)]
pub struct CrisisInterceptorPool {
    pub bolts: Vec<CrisisInterceptorBolt>,
}

impl CrisisInterceptorPool {
    /// Pushes a transient beam visual between `start_au` and `end_au`.
    pub fn spawn_bolt(&mut self, start_au: DVec3, end_au: DVec3, color: Color) {
        self.bolts.push(CrisisInterceptorBolt {
            start_au,
            end_au,
            color,
            remaining_secs: 1.6,
            max_secs: 1.6,
        });
    }
}

/// Query type for planetary defense targets eligible for crisis interception and impact checks.
pub type CrisisWorldTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimPosition,
        Option<&'static SimVelocity>,
        &'static Radius,
        Option<&'static CelestialBody>,
        Option<&'static mut CivilizationState>,
        Option<&'static mut TerraformingAtmosphere>,
        Option<&'static mut PlanetaryClimate>,
        Option<&'static mut BiosphereState>,
    ),
    (
        Without<CrisisImpactor>,
        Without<HazardComet>,
        Without<RogueInterloper>,
    ),
>;

/// Query type for active [`CrisisImpactor`] entities.
pub type CrisisImpactorQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut SimPosition,
        &'static mut SimVelocity,
        &'static mut CrisisImpactor,
    ),
    (Without<HazardComet>, Without<RogueInterloper>),
>;

/// Chooses the highest-value world (highest Kardashev civilization, then Earth/inner rocky planet).
pub fn resolve_primary_crisis_target<'a>(
    candidates: impl Iterator<Item = (Entity, &'a str, DVec3, DVec3, Option<f32>, Option<BodyType>)>,
) -> Option<(Entity, String, DVec3, DVec3)> {
    let mut best: Option<(i32, f32, Entity, String, DVec3, DVec3)> = None;
    for (ent, name, pos, vel, opt_k, opt_type) in candidates {
        if opt_type.is_some_and(|t| t.is_star_or_remnant()) {
            continue;
        }
        let k_score = opt_k.unwrap_or(0.0);
        let tier = if opt_k.is_some() {
            3
        } else if name == "Earth" || name.starts_with("Proto-Earth") {
            2
        } else {
            i32::from(opt_type.is_some_and(|t| {
                matches!(
                    t,
                    BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
                )
            }))
        };
        let replace = match &best {
            None => true,
            Some((bt, bk, ..)) => tier > *bt || (tier == *bt && k_score > *bk),
        };
        if replace {
            best = Some((tier, k_score, ent, name.to_string(), pos, vel));
        }
    }
    best.map(|(_, _, e, n, p, v)| (e, n, p, v))
}

/// Spawns `count` hypervelocity Planet-Killer asteroids on inbound intercept trajectories.
pub fn spawn_planet_killer_swarm(
    commands: &mut Commands,
    count: usize,
    target_entity: Option<Entity>,
    target_name: &str,
    target_pos: DVec3,
    target_vel: DVec3,
) -> Vec<Entity> {
    let standoffs = [0.58, 0.78, 1.00, 1.24];
    let mut spawned = Vec::with_capacity(count);
    for i in 0..count {
        let standoff = standoffs.get(i % standoffs.len()).copied().unwrap_or(0.85);
        let angle = (i as f64) * 1.45 + 0.42;
        let approach_dir =
            DVec3::new(angle.cos(), (i as f64 - 1.5) * 0.06, angle.sin()).normalize_or_zero();
        let spawn_pos = target_pos + approach_dir * standoff;
        let closing_speed = 2.4 + (i as f64) * 0.35;
        let inbound_dir = -approach_dir;
        let spawn_vel = target_vel + inbound_dir * closing_speed;

        let ent = commands
            .spawn((
                CelestialBody {
                    body_type: BodyType::Asteroid,
                    name: format!("Planet-Killer Apophis-{}", i + 1),
                },
                Mass(4.5e-8),
                Radius(0.00055),
                SimPosition(spawn_pos),
                SimVelocity(spawn_vel),
                SimAcceleration::default(),
                Composition {
                    metal_frac: 0.42,
                    silicate_frac: 0.56,
                    ice_frac: 0.02,
                    organics_frac: 0.0,
                    gas_frac: 0.0,
                },
                Temperature(210.0),
                CrisisImpactor {
                    target_entity,
                    target_name: target_name.to_string(),
                    deflected: false,
                    initial_inbound_dir: inbound_dir,
                    closest_approach_au: standoff,
                },
            ))
            .id();
        spawned.push(ent);
    }
    spawned
}

fn compute_perp_deflection_dir(inbound_dir: DVec3) -> DVec3 {
    let perp = DVec3::new(-inbound_dir.z, 0.25, inbound_dir.x).normalize_or_zero();
    if perp.length_squared() < 1e-6 {
        DVec3::Y
    } else {
        perp
    }
}

fn find_impactor_target_ent(
    imp: &CrisisImpactor,
    event: &ActiveCosmicEvent,
    targets: &CrisisWorldTargetQuery,
) -> Option<Entity> {
    if let Some(t) = imp.target_entity
        && targets.get(t).is_ok()
    {
        return Some(t);
    }
    if let Some(t) = event.target_entity
        && targets.get(t).is_ok()
    {
        return Some(t);
    }
    targets
        .iter()
        .find(|(_, _, _, _, opt_b, ..)| opt_b.is_none_or(|b| !b.body_type.is_star_or_remnant()))
        .map(|(e, ..)| e)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Evaluates deflection, grid intercept, and impact per asteroid"
)]
fn step_single_crisis_impactor(
    commands: &mut Commands,
    event: &mut ActiveCosmicEvent,
    imp_ent: Entity,
    imp_pos: &SimPosition,
    imp_vel: &mut SimVelocity,
    imp: &mut CrisisImpactor,
    targets: &mut CrisisWorldTargetQuery,
    interceptor_pool: &mut Option<ResMut<CrisisInterceptorPool>>,
    defense_grid_unlocked: bool,
    science: &mut Option<&mut ScienceData>,
    dt_years: f64,
    elapsed_years: f64,
) {
    let Some(t_ent) = find_impactor_target_ent(imp, event, targets) else {
        return;
    };
    let Ok((_, t_pos, opt_t_vel, t_rad, _, mut opt_civ, mut opt_terra, _, mut opt_bio)) =
        targets.get_mut(t_ent)
    else {
        return;
    };

    let t_vel = opt_t_vel.map_or(DVec3::ZERO, |v| v.0);
    let disp = imp_pos.0 - t_pos.0;
    let dist_au = disp.length();
    imp.closest_approach_au = imp.closest_approach_au.min(dist_au);

    let v_rel = imp_vel.0 - t_vel;
    let inbound_unit = imp.initial_inbound_dir.normalize_or_zero();
    let parallel_comp = inbound_unit * v_rel.dot(inbound_unit);
    let perp_speed = (v_rel - parallel_comp).length();
    let disp_unit = disp.normalize_or_zero();
    let is_receding_miss = v_rel.dot(disp_unit) > 0.25 && dist_au > 0.14;

    if !imp.deflected && (perp_speed >= 1.8 || is_receding_miss) {
        imp.deflected = true;
        event.threats_neutralized += 1;
        if let Some(sci) = science.as_deref_mut() {
            sci.add_science(75.0);
        }
    }

    // Automated Planetary Defense Grid interception
    if !imp.deflected && dist_au < 0.70 {
        let can_civ_shoot = opt_civ
            .as_deref()
            .is_some_and(|c| c.kardashev_level >= 1.25 && c.defense_readiness >= 0.45);
        let grid_shoot = defense_grid_unlocked
            && opt_civ
                .as_deref()
                .is_none_or(|c| c.defense_readiness >= 0.30);
        if can_civ_shoot || grid_shoot {
            if let Some(ref mut civ) = opt_civ {
                civ.defense_readiness = (civ.defense_readiness - 0.25).max(0.05);
            }
            let defl_dir = compute_perp_deflection_dir(inbound_unit);
            imp_vel.0 += defl_dir * 5.2;
            imp.deflected = true;
            event.threats_neutralized += 1;
            if let Some(pool) = interceptor_pool.as_deref_mut() {
                pool.spawn_bolt(t_pos.0, imp_pos.0, Color::srgba(0.25, 0.92, 1.0, 0.95));
            }
            if let Some(sci) = science.as_deref_mut() {
                sci.add_science(75.0);
            }
        }
    }

    if imp.deflected {
        if dist_au > 1.45
            && let Ok(mut cmd) = commands.get_entity(imp_ent)
        {
            cmd.try_despawn();
        }
        return;
    }

    // Check physical impact (with swept closest approach guard)
    let dt = dt_years.max(1e-5);
    let step_disp = v_rel * dt;
    let seg_sq = step_disp.length_squared();
    let swept_dist = if seg_sq > 1e-12 {
        let p_prev = disp - step_disp;
        let u = (-p_prev.dot(step_disp) / seg_sq).clamp(0.0, 1.0);
        (p_prev + step_disp * u).length()
    } else {
        dist_au
    };
    let impact_thresh = (t_rad.0 * 8.0).max(0.045);

    if dist_au < impact_thresh || swept_dist < impact_thresh {
        event.impacts_suffered += 1;
        record_impact_crater_basin(
            commands,
            t_ent,
            disp_unit.as_vec3(),
            0.18,
            elapsed_years,
            1.0,
        );
        let dmg_scale = if defense_grid_unlocked { 0.40 } else { 1.0 };
        if let Some(ref mut terra) = opt_terra {
            terra.impact_dust_optical_depth += 0.45 * dmg_scale;
        }
        if let Some(ref mut civ) = opt_civ {
            civ.stability = (civ.stability - 0.28 * dmg_scale).max(0.0);
            civ.kardashev_level = (civ.kardashev_level - 0.08 * dmg_scale).max(0.40);
            if let Some(ref mut bio) = opt_bio {
                bio.technosignature = technosignature_from_kardashev(civ.kardashev_level);
            }
        }
        if let Ok(mut cmd) = commands.get_entity(imp_ent) {
            cmd.try_despawn();
        }
    }
}

/// Evaluates all incoming [`CrisisImpactor`] bodies, automated defense interceptions, and impacts.
#[allow(
    clippy::too_many_arguments,
    reason = "Planet-Killer swarm step across impactors and targets"
)]
pub fn apply_planet_killer_swarm_step(
    commands: &mut Commands,
    event: &mut ActiveCosmicEvent,
    impactors: &mut CrisisImpactorQuery,
    targets: &mut CrisisWorldTargetQuery,
    interceptor_pool: &mut Option<ResMut<CrisisInterceptorPool>>,
    defense_grid_unlocked: bool,
    mut science: Option<&mut ScienceData>,
    dt_years: f64,
    elapsed_years: f64,
) {
    for (imp_ent, imp_pos, mut imp_vel, mut imp) in impactors.iter_mut() {
        step_single_crisis_impactor(
            commands,
            event,
            imp_ent,
            &imp_pos,
            &mut imp_vel,
            &mut imp,
            targets,
            interceptor_pool,
            defense_grid_unlocked,
            &mut science,
            dt_years,
            elapsed_years,
        );
    }

    let total = event.threats_total.max(1) as f32;
    let neutralized_ratio = (event.threats_neutralized as f32 / total).clamp(0.0, 1.0);
    let penalty = event.impacts_suffered as f32 * 0.25;
    event.mitigation_score = (neutralized_ratio - penalty).clamp(0.0, 1.0);
}

/// Evaluates industrial climate runaway & recovery during [`CosmicEventType::GreatFilterCrisis`].
pub fn apply_great_filter_crisis_step(
    event: &mut ActiveCosmicEvent,
    targets: &mut CrisisWorldTargetQuery,
    has_mirror_array: bool,
    has_ascension: bool,
    dt_years: f64,
) {
    let dt_f = dt_years.clamp(0.0, 5.0) as f32;
    let target_ent = event.target_entity.or_else(|| {
        targets
            .iter()
            .filter_map(|(e, _, _, _, _, opt_civ, ..)| opt_civ.map(|c| (e, c.kardashev_level)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(e, _)| e)
    });
    let Some(t_ent) = target_ent else {
        event.mitigation_score = 0.5;
        return;
    };
    let Ok((_, _, _, _, _, mut opt_civ, mut opt_terra, mut opt_climate, _)) =
        targets.get_mut(t_ent)
    else {
        return;
    };

    let stabilizer_active = opt_civ
        .as_deref()
        .is_some_and(|c| c.climate_stabilizer_secs > 0.0);
    let protected = stabilizer_active || has_mirror_array || has_ascension;

    if protected {
        if let Some(ref mut clim) = opt_climate {
            clim.surface_temperature_k +=
                (288.0 - clim.surface_temperature_k) * (0.28 * dt_f).min(0.8);
        }
        if let Some(ref mut terra) = opt_terra {
            terra.impact_dust_optical_depth =
                (terra.impact_dust_optical_depth - 0.12 * dt_f).max(0.0);
        }
        if let Some(ref mut civ) = opt_civ {
            civ.great_filter_stress = (civ.great_filter_stress - 0.14 * dt_f).max(0.0);
            civ.stability = (civ.stability + 0.08 * dt_f).min(1.0);
        }
        event.mitigation_score = (event.mitigation_score + 0.25 * dt_f.max(0.1)).clamp(0.0, 1.0);
    } else {
        if let Some(ref mut clim) = opt_climate {
            clim.surface_temperature_k = (clim.surface_temperature_k + 4.0 * dt_f).min(460.0);
        }
        if let Some(ref mut terra) = opt_terra {
            terra.impact_dust_optical_depth =
                (terra.impact_dust_optical_depth + 0.03 * dt_f).min(1.5);
        }
        if let Some(ref mut civ) = opt_civ {
            civ.great_filter_stress = (civ.great_filter_stress + 0.10 * dt_f).min(1.0);
            civ.stability = (civ.stability - 0.06 * dt_f).max(0.0);
            if civ.stability <= 0.01 {
                event.impacts_suffered = event.impacts_suffered.max(1);
            }
        }
    }
}

/// Computes the final [`CrisisOutcome`] rating at crisis completion.
#[must_use]
pub fn evaluate_crisis_outcome(event: &ActiveCosmicEvent) -> CrisisOutcome {
    match event.event_type {
        CosmicEventType::PlanetKillerSwarm => {
            if event.threats_total > 0
                && event.threats_neutralized >= event.threats_total
                && event.impacts_suffered == 0
            {
                CrisisOutcome::Averted
            } else if event.impacts_suffered == 0 || event.mitigation_score >= 0.50 {
                CrisisOutcome::Mitigated
            } else if event.impacts_suffered >= 3 {
                CrisisOutcome::Catastrophic
            } else {
                CrisisOutcome::Survived
            }
        }
        CosmicEventType::GreatFilterCrisis => {
            if event.mitigation_score >= 0.85 && event.impacts_suffered == 0 {
                CrisisOutcome::Averted
            } else if event.mitigation_score >= 0.45 {
                CrisisOutcome::Mitigated
            } else if event.impacts_suffered > 0 {
                CrisisOutcome::Catastrophic
            } else {
                CrisisOutcome::Survived
            }
        }
        _ => {
            if event.mitigation_score >= 0.85 {
                CrisisOutcome::Averted
            } else if event.mitigation_score >= 0.45 || event.countermeasure_uses > 0 {
                CrisisOutcome::Mitigated
            } else if event.impacts_suffered > 0 {
                CrisisOutcome::Catastrophic
            } else {
                CrisisOutcome::Survived
            }
        }
    }
}

/// Query type for outbound hazard comets targeted by kinetic deflectors.
pub type HazardCometVelQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static SimPosition, &'static mut SimVelocity),
    (
        With<HazardComet>,
        Without<CrisisImpactor>,
        Without<RogueInterloper>,
    ),
>;

/// Query type for rogue hyperbolic interlopers targeted by kinetic deflectors.
pub type RogueInterloperVelQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static SimPosition, &'static mut SimVelocity),
    (
        With<RogueInterloper>,
        Without<CrisisImpactor>,
        Without<HazardComet>,
    ),
>;

#[allow(
    clippy::too_many_arguments,
    reason = "Kinetic deflector checks impactors, hazard comets, rogue interlopers, and world defenses"
)]
fn execute_kinetic_deflector(
    preferred_target: Option<Entity>,
    hazard_mgr: &mut Option<ResMut<CosmicEventManager>>,
    impactors: &mut CrisisImpactorQuery,
    hazard_comets: &mut HazardCometVelQuery,
    roguish: &mut RogueInterloperVelQuery,
    interceptor_pool: &mut Option<ResMut<CrisisInterceptorPool>>,
    targets: &CrisisWorldTargetQuery,
    power_mult: f64,
    mut science: Option<&mut ScienceData>,
) -> String {
    let origin_pos = targets
        .iter()
        .filter(|(_, _, _, _, b, ..)| b.is_none_or(|c| !c.body_type.is_star_or_remnant()))
        .max_by(|a, b| {
            let ka = a.5.map_or(0.0, |c| c.kardashev_level);
            let kb = b.5.map_or(0.0, |c| c.kardashev_level);
            ka.total_cmp(&kb)
        })
        .map_or(DVec3::new(1.0, 0.0, 0.0), |(_, p, ..)| p.0);

    // 1. Prefer undeflected CrisisImpactor
    let nearest_imp = impactors
        .iter_mut()
        .filter(|(_, _, _, imp)| !imp.deflected)
        .min_by(|(ea, pa, ..), (eb, pb, ..)| {
            let da = if preferred_target == Some(*ea) {
                -1.0
            } else {
                (pa.0 - origin_pos).length()
            };
            let db = if preferred_target == Some(*eb) {
                -1.0
            } else {
                (pb.0 - origin_pos).length()
            };
            da.total_cmp(&db)
        });

    if let Some((_, imp_pos, mut imp_vel, mut imp)) = nearest_imp {
        let perp = compute_perp_deflection_dir(imp.initial_inbound_dir);
        imp_vel.0 += perp * (5.5 * power_mult);
        imp.deflected = true;
        if let Some(pool) = interceptor_pool.as_deref_mut() {
            pool.spawn_bolt(origin_pos, imp_pos.0, Color::srgba(0.35, 1.0, 0.65, 0.95));
        }
        if let Some(mgr) = hazard_mgr.as_deref_mut()
            && let Some(ev) = mgr.active_events.iter_mut().find(|e| {
                e.event_type == CosmicEventType::PlanetKillerSwarm
                    && e.phase != CosmicEventPhase::Completed
            })
        {
            ev.threats_neutralized += 1;
            ev.countermeasure_uses += 1;
            ev.mitigation_score =
                (ev.threats_neutralized as f32 / ev.threats_total.max(1) as f32).clamp(0.0, 1.0);
        }
        if let Some(ref mut sci) = science {
            sci.add_science(75.0);
        }
        return "🚀 Kinetic Deflector struck incoming Planet-Killer asteroid off course!"
            .to_string();
    }

    // 2. Fallback to HazardComet
    if let Some((_, c_pos, mut c_vel)) = hazard_comets.iter_mut().next() {
        let dir = compute_perp_deflection_dir((origin_pos - c_pos.0).normalize_or_zero());
        c_vel.0 += dir * (4.5 * power_mult);
        if let Some(pool) = interceptor_pool.as_deref_mut() {
            pool.spawn_bolt(origin_pos, c_pos.0, Color::srgba(0.45, 0.88, 1.0, 0.90));
        }
        if let Some(mgr) = hazard_mgr.as_deref_mut()
            && let Some(ev) = mgr
                .active_events
                .iter_mut()
                .find(|e| e.phase != CosmicEventPhase::Completed)
        {
            ev.countermeasure_uses += 1;
            ev.mitigation_score = (ev.mitigation_score + 0.45).clamp(0.0, 1.0);
        }
        return "🚀 Kinetic Deflector diverted an inbound Outburst Comet!".to_string();
    }

    // 3. Fallback to RogueInterloper
    if let Some((_, r_pos, mut r_vel)) = roguish.iter_mut().next() {
        let perp = compute_perp_deflection_dir((-r_pos.0).normalize_or_zero());
        r_vel.0 += perp * (2.2 * power_mult);
        if let Some(pool) = interceptor_pool.as_deref_mut() {
            pool.spawn_bolt(origin_pos, r_pos.0, Color::srgba(1.0, 0.75, 0.30, 0.90));
        }
        if let Some(mgr) = hazard_mgr.as_deref_mut()
            && let Some(ev) = mgr
                .active_events
                .iter_mut()
                .find(|e| e.phase != CosmicEventPhase::Completed)
        {
            ev.countermeasure_uses += 1;
            ev.mitigation_score = (ev.mitigation_score + 0.50).clamp(0.0, 1.0);
        }
        return "🚀 Gravitational Deflector nudged Rogue Interloper trajectory!".to_string();
    }

    "🚀 Kinetic Deflector armed & scanning sector (no hostile impactor in range).".to_string()
}

/// Processes player-dispatched crisis defense countermeasures ([`CrisisCountermeasureEvent`]).
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Countermeasure handler validates progression cost and updates threats, worlds, and interceptors"
)]
pub fn handle_crisis_countermeasure_system(
    mut events: MessageReader<CrisisCountermeasureEvent>,
    mut hazard_mgr: Option<ResMut<CosmicEventManager>>,
    progression: Option<ResMut<ProgressionState>>,
    mut science: Option<ResMut<ScienceData>>,
    techs: Option<Res<UnlockedTechs>>,
    mut impactors: CrisisImpactorQuery,
    mut hazard_comets: HazardCometVelQuery,
    mut roguish: RogueInterloperVelQuery,
    mut targets: CrisisWorldTargetQuery,
    mut interceptor_pool: Option<ResMut<CrisisInterceptorPool>>,
    mut toast: Option<ResMut<NotificationToast>>,
) {
    for ev in events.read() {
        let req_tech = ev.kind.required_tech();
        let cost = ev.kind.science_cost();
        if let Some(prog) = progression.as_deref() {
            if let ActionAccessResult::Locked(reason) =
                prog.check_action_access(req_tech, cost, techs.as_deref(), science.as_deref())
            {
                if let Some(ref mut t) = toast {
                    t.message = reason;
                    t.timer = 4.0;
                }
                continue;
            }
            let _ = prog.consume_progression_cost(cost, science.as_deref_mut());
        }

        let power_mult = if techs.as_deref().is_some_and(|t| {
            t.has(TechnologyId::CometRedirection) || t.has(TechnologyId::StellarEngine)
        }) {
            1.5
        } else {
            1.0
        };

        let msg = match ev.kind {
            CrisisCountermeasureKind::LaunchKineticDeflector => execute_kinetic_deflector(
                ev.preferred_target,
                &mut hazard_mgr,
                &mut impactors,
                &mut hazard_comets,
                &mut roguish,
                &mut interceptor_pool,
                &targets,
                power_mult,
                science.as_deref_mut(),
            ),
            CrisisCountermeasureKind::ActivateShieldOverdrive => {
                for (ent, _, _, _, _, mut opt_civ, ..) in targets.iter_mut() {
                    if let Some(ref mut civ) = opt_civ
                        && (ev.preferred_target.is_none()
                            || ev.preferred_target == Some(ent)
                            || civ.kardashev_level >= 0.5)
                    {
                        civ.shield_overdrive_secs = 28.0;
                        civ.defense_readiness = 1.0;
                    }
                }
                if let Some(mgr) = hazard_mgr.as_deref_mut() {
                    for active in &mut mgr.active_events {
                        if matches!(
                            active.event_type,
                            CosmicEventType::GammaRayBurst | CosmicEventType::SolarSuperflare
                        ) && active.phase != CosmicEventPhase::Completed
                        {
                            active.countermeasure_uses += 1;
                            active.mitigation_score =
                                (active.mitigation_score + 0.55).clamp(0.0, 1.0);
                        }
                    }
                }
                "🛡️ Magnetospheric Shield Overdrive ENGAGED (28s ionizing protection)!".to_string()
            }
            CrisisCountermeasureKind::DeployClimateStabilizer => {
                for (_, _, _, _, _, mut opt_civ, mut opt_terra, mut opt_climate, _) in
                    targets.iter_mut()
                {
                    if let Some(ref mut civ) = opt_civ {
                        civ.climate_stabilizer_secs = 32.0;
                        civ.great_filter_stress = (civ.great_filter_stress * 0.5).max(0.0);
                    }
                    if let Some(ref mut terra) = opt_terra {
                        terra.impact_dust_optical_depth *= 0.5;
                    }
                    if let Some(ref mut clim) = opt_climate {
                        clim.surface_temperature_k += (288.0 - clim.surface_temperature_k) * 0.35;
                    }
                }
                if let Some(mgr) = hazard_mgr.as_deref_mut() {
                    for active in &mut mgr.active_events {
                        if active.event_type == CosmicEventType::GreatFilterCrisis
                            && active.phase != CosmicEventPhase::Completed
                        {
                            active.countermeasure_uses += 1;
                            active.mitigation_score =
                                (active.mitigation_score + 0.55).clamp(0.0, 1.0);
                        }
                    }
                }
                "❄️ Orbital Climate Stabilizer DEPLOYED (dust scrubbed, thermal equilibrium restored)!".to_string()
            }
        };

        if let Some(ref mut t) = toast {
            t.message = msg;
            t.timer = 4.5;
        }
    }
}
