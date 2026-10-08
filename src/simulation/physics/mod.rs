//! Symplectic N-Body Gravity and Aerodynamic Gas Drag Physics Engine.
//! This system handles ONLY ECS-promoted massive bodies (planets, protoplanets).
//! The 50k particle swarm physics is handled entirely on the GPU.

pub mod forces;
pub mod integrator;

use bevy::math::DVec3;
use bevy::prelude::*;
use rayon::prelude::*;

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
    elapsed_years: f64,
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

        let cluster_com_mass = if is_gmc_cluster {
            let mut total_m = 0.0;
            for &(_, m_mass, _, _) in &massive_data {
                if m_mass >= 0.5 {
                    total_m += m_mass;
                }
            }
            (DVec3::ZERO, total_m)
        } else {
            (DVec3::ZERO, 0.0)
        };

        let new_accelerations: Vec<DVec3> = body_data
            .par_iter()
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
                    cluster_com_mass,
                    elapsed_years,
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
    let is_gmc_cluster =
        scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);
    let star_index = body_data.iter().position(|b| b.is_central_star);
    let (star_mass, star_pos, is_central_quasi) = if let Some(idx) = star_index {
        if let Some(b) = body_data.get(idx) {
            (b.mass, b.pos, b.body_type == BodyType::QuasiStar)
        } else {
            (1.0, DVec3::ZERO, false)
        }
    } else if is_gmc_cluster {
        (0.0, DVec3::ZERO, false)
    } else {
        (1.0, DVec3::ZERO, false)
    };

    let has_black_hole = body_data.iter().any(|b| b.body_type == BodyType::BlackHole);
    // A cluster naturally grows massive stars and black holes; those must not flip the whole
    // simulation into Little-Red-Dot mode (125x smaller timestep + LRD-specific clamps).
    let is_little_red_dot = !is_gmc_cluster
        && (scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::LittleRedDot)
            || is_central_quasi
            || has_black_hole
            || star_mass >= 15.0);

    let is_compact_system = (star_mass < 0.25 && disk_params.outer_radius_au < 1.0)
        || scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::Trappist1System);
    // GMC clusters have wide open orbits (>= 1.4 AU); a massive central black hole in a cluster
    // must not trigger the relativistic microsecond pericenter clamps used for close-in S-stars.
    let is_smbh = !is_gmc_cluster && star_mass > 100_000.0;
    let max_substeps = if is_gmc_cluster {
        // GMC clusters feature hundreds of stars and gas clumps; clamp substeps to 64 to guarantee
        // smooth frame rates while enabling fast-forwarding up to 5+ years per frame (300+ yr/s at 60 FPS).
        config.max_substeps_per_frame.clamp(1, 64)
    } else if is_smbh || is_little_red_dot || is_compact_system {
        config.max_substeps_per_frame.max(128)
    } else {
        config.max_substeps_per_frame
    };
    let eff_dt = if is_gmc_cluster {
        // GMC clusters feature wide planetary and stellar orbits (>= 1.4 AU).
        // 0.08 yr (~29 days) per leapfrog step provides high symplectic orbital stability
        // while allowing fast-forwarding smoothly without stalling.
        0.08
    } else if is_smbh {
        dt.min(0.000_005)
    } else if is_little_red_dot || is_compact_system {
        dt.min(0.00004)
    } else {
        dt
    };

    // Prevent target_dt from forcing an unsafe sub_dt when fast-forwarding in chaotic GMC clusters
    let safe_target_dt = if is_gmc_cluster {
        target_dt.min(max_substeps as f64 * eff_dt)
    } else {
        target_dt
    };
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
fn enforce_dynamic_galactic_nucleus(
    body_data: &mut [PhysicsBodyEntry],
    is_gmc_cluster: bool,
    commands: &mut Commands,
) {
    if is_gmc_cluster && !body_data.iter().any(|b| b.is_central_star) {
        if let Some(anchor_idx) = body_data
            .iter()
            .enumerate()
            .filter(|(_, b)| b.body_type.is_star_or_remnant())
            .max_by(|a, b| {
                a.1.mass
                    .partial_cmp(&b.1.mass)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
        {
            if let Some(anchor_b) = body_data.get(anchor_idx) {
                let anchor_pos = anchor_b.pos;
                let anchor_vel = anchor_b.vel;
                let anchor_entity = anchor_b.entity;

                // Smoothly translate the entire galaxy so the Nucleus is precisely at DVec3::ZERO!
                for b in body_data.iter_mut() {
                    b.pos -= anchor_pos;
                    b.vel -= anchor_vel;
                }

                if let Some(anchor_mut) = body_data.get_mut(anchor_idx) {
                    anchor_mut.is_central_star = true;
                }
                commands.entity(anchor_entity).insert(CentralStar);
            }
        }
    }
}

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

    // --- DYNAMIC GALACTIC NUCLEUS ANCHOR ---
    let is_gmc_cluster = scenario_state
        .as_deref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);
    enforce_dynamic_galactic_nucleus(&mut body_data, is_gmc_cluster, &mut commands);

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
        sim_time.elapsed_years,
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
