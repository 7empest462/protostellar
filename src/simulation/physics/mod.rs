//! Symplectic N-Body Gravity and Aerodynamic Gas Drag Physics Engine.
//! This system handles ONLY ECS-promoted massive bodies (planets, protoplanets).
//! The 50k particle swarm physics is handled entirely on the GPU.

pub mod forces;
pub mod integrator;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::disk_migration::apply_type_i_torque_acc;
use crate::simulation::resources::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::*;

pub use forces::*;
pub use integrator::*;

fn update_physics_energy_monitor(
    energy_monitor: &mut EnergyMonitor,
    body_data: &[PhysicsBodyEntry],
    star_index: Option<usize>,
    star_pos: DVec3,
    star_mass: f64,
    softening_au: f64,
) {
    let mut kinetic_e = 0.0;
    let mut potential_e = 0.0;

    for (i, b) in body_data.iter().enumerate() {
        kinetic_e += 0.5 * b.mass * b.vel.length_squared();

        if let Some(s_idx) = star_index {
            if i != s_idx {
                let dist = (b.pos - star_pos).length() + softening_au;
                potential_e -= (G_ASTRO * star_mass * b.mass) / dist;
            }
        }
    }

    let total_e = kinetic_e + potential_e;
    if !energy_monitor.initialized && total_e.abs() > 1e-10 {
        energy_monitor.initial_total_energy = total_e;
        energy_monitor.initialized = true;
    }

    let rel_drift = if energy_monitor.initial_total_energy.abs() > 1e-10 {
        ((total_e - energy_monitor.initial_total_energy) / energy_monitor.initial_total_energy)
            .abs()
    } else {
        0.0
    };

    energy_monitor.kinetic_energy = kinetic_e;
    energy_monitor.potential_energy = potential_e;
    energy_monitor.total_energy = total_e;
    energy_monitor.relative_energy_drift = rel_drift;
}

#[allow(clippy::type_complexity, reason = "Physics State Writeback")]
fn write_back_physics_results(
    commands: &mut Commands,
    body_data: &[PhysicsBodyEntry],
    is_little_red_dot: bool,
    bodies_query: &mut Query<
        (
            Entity,
            &mut Mass,
            &mut SimPosition,
            &mut SimVelocity,
            &mut SimAcceleration,
            &Radius,
            &CelestialBody,
            Option<&mut SatelliteOf>,
            Option<&CentralStar>,
        ),
        Without<crate::simulation::probes::SpaceProbe>,
    >,
) {
    let mut escaped_minor_debris: Vec<Entity> = Vec::new();

    for (e, mut m, mut pos, mut vel, mut acc, _, body, opt_sat, opt_central) in
        bodies_query.iter_mut()
    {
        if opt_central.is_some() {
            pos.0 = DVec3::ZERO;
            vel.0 = DVec3::ZERO;
            acc.0 = DVec3::ZERO;
        } else if let Some(b) = body_data.iter().find(|b| b.entity == e) {
            let r_mag = b.pos.length();
            let debris_escape_radius = if is_little_red_dot { 15_000.0 } else { 2000.0 };
            if r_mag > debris_escape_radius
                && matches!(
                    body.body_type,
                    BodyType::Planetesimal
                        | BodyType::Asteroid
                        | BodyType::Comet
                        | BodyType::DustGrain
                )
            {
                escaped_minor_debris.push(e);
                continue;
            }

            m.0 = b.mass;
            pos.0 = b.pos;
            vel.0 = b.vel;
            acc.0 = b.acc;
            if let Some(s_data) = b.satellite {
                if let Some(mut s_comp) = opt_sat {
                    *s_comp = s_data;
                }
            } else if opt_sat.is_some() {
                if let Ok(mut cmd) = commands.get_entity(e) {
                    cmd.remove::<SatelliteOf>();
                }
            }
        }
    }

    for e in escaped_minor_debris {
        if let Ok(mut cmd) = commands.get_entity(e) {
            cmd.try_despawn();
        }
    }
}

fn identify_giant_planets(
    body_data: &[PhysicsBodyEntry],
) -> (Option<Entity>, Option<Entity>, hashbrown::HashSet<Entity>) {
    let mut jupiter_entity = None;
    let mut saturn_entity = None;
    let mut ice_giant_entities = hashbrown::HashSet::new();

    for b in body_data {
        let r = (b.pos.x * b.pos.x + b.pos.z * b.pos.z).sqrt();
        let name_lower = b.name.to_lowercase();
        if name_lower.contains("jupiter") {
            jupiter_entity = Some(b.entity);
        } else if name_lower.contains("saturn") {
            saturn_entity = Some(b.entity);
        } else if matches!(b.body_type, BodyType::GasGiant) || b.mass >= JUPITER_MASS_SOLAR * 0.05 {
            if r < 7.5 && jupiter_entity.is_none() {
                jupiter_entity = Some(b.entity);
            } else if (7.5..16.0).contains(&r) && saturn_entity.is_none() {
                saturn_entity = Some(b.entity);
            }
        }
        if matches!(b.body_type, BodyType::IceGiant)
            || name_lower.contains("uranus")
            || name_lower.contains("neptune")
        {
            ice_giant_entities.insert(b.entity);
        }
    }
    (jupiter_entity, saturn_entity, ice_giant_entities)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Physics kernel requires all integration state explicitly for performance"
)]
fn run_physics_substeps(
    body_data: &mut [PhysicsBodyEntry],
    n_substeps: usize,
    sub_dt: f64,
    massive_indices: &[usize],
    config: &SimulationConfig,
    star_mass: f64,
    star_pos: DVec3,
    star_index: Option<usize>,
    softening_sq: f64,
    tractor: Option<(DVec3, f64)>,
    is_little_red_dot: bool,
    is_compact_system: bool,
    is_gmc_cluster: bool,
    lhb_state: &mut crate::game::phases::LateHeavyBombardmentState,
    jupiter_entity: Option<Entity>,
    saturn_entity: Option<Entity>,
    ice_giant_entities: &hashbrown::HashSet<Entity>,
) {
    for _ in 0..n_substeps {
        for body in body_data.iter_mut() {
            if body.is_central_star {
                body.pos = DVec3::ZERO;
                body.vel = DVec3::ZERO;
                body.acc = DVec3::ZERO;
            } else {
                let is_bound = is_body_bound_to_star(body.pos, body.vel, star_pos, star_mass);
                apply_pert_kick(body, sub_dt * 0.5, star_mass, is_bound);
            }
        }

        advance_symplectic_leapfrog_drift(body_data, sub_dt, star_pos, star_mass);

        let massive_data: Vec<(DVec3, f64, f64, usize)> = massive_indices
            .iter()
            .filter_map(|&idx| {
                let b = body_data.get(idx)?;
                Some((b.pos, b.mass, b.radius, idx))
            })
            .collect();

        let new_accelerations: Vec<DVec3> = body_data
            .iter()
            .enumerate()
            .map(|(i, b)| {
                compute_single_body_acc(
                    i,
                    b,
                    config,
                    star_mass,
                    star_index,
                    softening_sq,
                    &massive_data,
                    tractor,
                    is_little_red_dot,
                    is_gmc_cluster,
                )
            })
            .collect();

        for (body, &new_acc) in body_data.iter_mut().zip(&new_accelerations) {
            if body.is_central_star {
                body.acc = DVec3::ZERO;
            } else {
                body.acc = new_acc;
            }
        }

        // Type-I / II migration while nebular gas remains
        if config.gas_density_scale > 0.001 {
            for body in body_data.iter_mut() {
                if body.is_central_star {
                    continue;
                }
                apply_type_i_torque_acc(
                    body.pos,
                    body.vel,
                    body.mass,
                    body.body_type,
                    body.is_central_star,
                    body.satellite.is_some(),
                    star_mass,
                    config.gas_density_scale,
                    &mut body.acc,
                );
            }
        }

        apply_planetary_migration_and_lhb(
            body_data,
            lhb_state,
            jupiter_entity,
            saturn_entity,
            ice_giant_entities,
            sub_dt,
            is_compact_system,
        );

        apply_velocity_kick_and_limits(
            body_data,
            sub_dt,
            star_mass,
            star_pos,
            is_little_red_dot,
            is_gmc_cluster,
        );
    }
}

struct PhysicsSystemContext {
    star_index: Option<usize>,
    star_mass: f64,
    star_pos: DVec3,
    is_little_red_dot: bool,
    is_compact_system: bool,
    is_gmc_cluster: bool,
    n_substeps: usize,
    sub_dt: f64,
    massive_indices: Vec<usize>,
}

fn analyze_physics_system(
    body_data: &mut [PhysicsBodyEntry],
    scenario_state: Option<&ActiveScenarioState>,
    disk_params: &DiskParameters,
    config: &SimulationConfig,
    dt: f64,
    target_dt: f64,
    softening_sq: f64,
) -> PhysicsSystemContext {
    let star_index = body_data.iter().position(|b| b.is_central_star);
    let (star_mass, star_pos, is_central_quasi) = if let Some(idx) = star_index {
        if let Some(b) = body_data.get(idx) {
            (b.mass, b.pos, b.body_type == BodyType::QuasiStar)
        } else {
            (1.0, DVec3::ZERO, false)
        }
    } else {
        (1.0, DVec3::ZERO, false)
    };

    let has_black_hole = body_data.iter().any(|b| b.body_type == BodyType::BlackHole);
    let is_little_red_dot = scenario_state
        .is_some_and(|s| s.current_preset == ScenarioPreset::LittleRedDot)
        || is_central_quasi
        || has_black_hole
        || star_mass >= 15.0;

    let is_compact_system = (star_mass < 0.25 && disk_params.outer_radius_au < 1.0)
        || scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::Trappist1System);
    let is_gmc_cluster =
        scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);
    let is_smbh = star_mass > 100_000.0;
    let max_substeps = if is_smbh {
        config.max_substeps_per_frame.max(256)
    } else if is_little_red_dot || is_compact_system || is_gmc_cluster {
        config.max_substeps_per_frame.max(128)
    } else {
        config.max_substeps_per_frame
    };
    let eff_dt = if is_smbh {
        dt.min(0.000_005)
    } else if is_little_red_dot || is_compact_system {
        dt.min(0.00004)
    } else if is_gmc_cluster {
        dt.min(0.0005)
    } else {
        dt
    };

    // CRITICAL: Prevent target_dt from forcing an unsafe sub_dt when fast-forwarding!
    let safe_target_dt = target_dt.min(max_substeps as f64 * eff_dt);
    let n_substeps = ((safe_target_dt / eff_dt).ceil() as usize).clamp(1, max_substeps);
    let sub_dt = safe_target_dt / (n_substeps as f64);

    let massive_indices: Vec<usize> = body_data
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            b.mass > EARTH_MASS_SOLAR * 0.1
                || b.is_central_star
                || b.body_type.is_star_or_remnant()
                || matches!(
                    b.body_type,
                    BodyType::Protoplanet
                        | BodyType::TerrestrialPlanet
                        | BodyType::SuperEarth
                        | BodyType::GasGiant
                        | BodyType::IceGiant
                )
        })
        .map(|(i, _)| i)
        .collect();

    for body in body_data {
        if body.acc.length_squared() < 1e-12 && !body.is_central_star {
            let r_vec = body.pos - star_pos;
            let dist_sq = r_vec.length_squared() + softening_sq;
            let dist = dist_sq.sqrt();
            if is_body_bound_to_star(body.pos, body.vel, star_pos, star_mass) {
                body.acc = DVec3::ZERO;
            } else {
                body.acc = -(G_ASTRO * star_mass / (dist_sq * dist)) * r_vec;
            }
        }
    }

    PhysicsSystemContext {
        star_index,
        star_mass,
        star_pos,
        is_little_red_dot,
        is_compact_system,
        is_gmc_cluster,
        n_substeps,
        sub_dt,
        massive_indices,
    }
}

/// Advances accumulated visual simulation time in seconds for shaders and rendering animations.
/// Pauses when time_warp is paused, freezing planetary rotation, cloud movement, and stellar boiling.
pub fn update_simulation_visual_time(
    time: Res<Time>,
    time_warp: Res<TimeWarp>,
    mut sim_time: ResMut<SimTime>,
) {
    if !time_warp.is_paused || time_warp.step_once {
        sim_time.visual_time_secs += time.delta_secs();
    }
}

/// Advances the N-body gravitational physics simulation using a Symplectic Kick-Drift-Kick Leapfrog integrator.
#[allow(clippy::type_complexity, reason = "N-body Simulation State")]
pub fn step_physics_simulation(
    config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    time_warp: Res<TimeWarp>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    player_state: Res<PlayerInteractionState>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    mut commands: Commands,
    mut bodies_query: Query<
        (
            Entity,
            &mut Mass,
            &mut SimPosition,
            &mut SimVelocity,
            &mut SimAcceleration,
            &Radius,
            &CelestialBody,
            Option<&mut SatelliteOf>,
            Option<&CentralStar>,
        ),
        Without<crate::simulation::probes::SpaceProbe>,
    >,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt = config.base_dt_yr;
    let target_dt = dt * time_warp.multiplier.max(TimeWarp::MIN_SPEED);
    let is_compact_system = scenario_state
        .as_deref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::Trappist1System)
        || (disk_params.central_star_mass < 0.25 && disk_params.outer_radius_au < 1.0);
    let eff_softening_au = if is_compact_system {
        0.00005
    } else {
        config.softening_au
    };
    let softening_sq = eff_softening_au * eff_softening_au;

    let mut body_data: Vec<PhysicsBodyEntry> = bodies_query
        .iter()
        .map(
            |(e, m, pos, vel, acc, rad, body, sat, opt_c)| PhysicsBodyEntry {
                entity: e,
                mass: m.0,
                pos: pos.0,
                vel: vel.0,
                acc: acc.0,
                radius: rad.0,
                body_type: body.body_type,
                satellite: sat.copied(),
                is_central_star: opt_c.is_some(),
                name: body.name.clone(),
            },
        )
        .collect();

    if body_data.is_empty() {
        sim_time.elapsed_years += target_dt;
        sim_time.current_dt_yr = target_dt;
        sim_time.step_count += 1;
        return;
    }

    let ctx = analyze_physics_system(
        &mut body_data,
        scenario_state.as_deref(),
        &disk_params,
        &config,
        dt,
        target_dt,
        softening_sq,
    );

    let tractor = if player_state.active_tool == PlayerTool::GravitationalTractor {
        player_state
            .tractor_position
            .filter(|_| player_state.tractor_mass > 0.0)
            .map(|pos| (pos, player_state.tractor_mass))
    } else {
        None
    };

    let (jupiter_entity, saturn_entity, ice_giant_entities) = identify_giant_planets(&body_data);

    run_physics_substeps(
        &mut body_data,
        ctx.n_substeps,
        ctx.sub_dt,
        &ctx.massive_indices,
        &config,
        ctx.star_mass,
        ctx.star_pos,
        ctx.star_index,
        softening_sq,
        tractor,
        ctx.is_little_red_dot,
        ctx.is_compact_system,
        ctx.is_gmc_cluster,
        &mut lhb_state,
        jupiter_entity,
        saturn_entity,
        &ice_giant_entities,
    );

    if let (Some(target), Some(dv)) = (
        player_state.impulse_target_entity,
        player_state.impulse_delta_v,
    ) {
        if let Some(b) = body_data.iter_mut().find(|b| b.entity == target) {
            b.vel += dv;
        }
    }

    write_back_physics_results(
        &mut commands,
        &body_data,
        ctx.is_little_red_dot,
        &mut bodies_query,
    );

    update_physics_energy_monitor(
        &mut energy_monitor,
        &body_data,
        ctx.star_index,
        ctx.star_pos,
        ctx.star_mass,
        eff_softening_au,
    );

    let actual_integrated_time = ctx.sub_dt * ctx.n_substeps as f64;
    sim_time.elapsed_years += actual_integrated_time;
    sim_time.current_dt_yr = actual_integrated_time;
    sim_time.step_count += ctx.n_substeps as u64;
}
