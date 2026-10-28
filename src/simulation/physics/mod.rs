//! Symplectic N-Body Gravity and Aerodynamic Gas Drag Physics Engine.
//! This system handles ONLY ECS-promoted massive bodies (planets, protoplanets).
//! The 50k particle swarm physics is handled entirely on the GPU.

pub mod forces;
pub mod integrator;

use bevy::math::DVec3;
use bevy::prelude::*;
use rayon::prelude::*;

use crate::gpu::gmc_far_field::{GMC_FAR_FIELD_MIN_BODIES, GMC_FAR_FIELD_NEAR_RADIUS_AU};
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
    massive_indices: &[usize],
    star_index: Option<usize>,
    star_pos: DVec3,
    star_mass: f64,
    softening_au: f64,
) {
    let softening_sq = softening_au * softening_au;
    let mut kinetic_e = 0.0;
    let mut potential_e = 0.0;

    for (i, b) in body_data.iter().enumerate() {
        kinetic_e += 0.5 * b.mass * b.vel.length_squared();

        if let Some(s_idx) = star_index {
            if i != s_idx {
                let dist_sq = (b.pos - star_pos).length_squared() + softening_sq;
                let dist = dist_sq.sqrt();
                potential_e -= (G_ASTRO * star_mass * b.mass) / dist;
            }
        }
    }

    // Include mutual potential energy between massive non-central bodies (binaries, giant planets, SMBHs)
    if massive_indices.len() > 1 {
        for (idx_a, &i) in massive_indices.iter().enumerate() {
            if Some(i) == star_index {
                continue;
            }
            let Some(b_i) = body_data.get(i) else {
                continue;
            };
            for &j in massive_indices.iter().skip(idx_a + 1) {
                if Some(j) == star_index {
                    continue;
                }
                let Some(b_j) = body_data.get(j) else {
                    continue;
                };
                let dist_sq = (b_i.pos - b_j.pos).length_squared() + softening_sq;
                let dist = dist_sq.sqrt();
                potential_e -= (G_ASTRO * b_i.mass * b_j.mass) / dist;
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
    is_galaxy_collision: bool,
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
    let body_map: hashbrown::HashMap<Entity, &PhysicsBodyEntry> =
        body_data.iter().map(|b| (b.entity, b)).collect();

    for (e, mut m, mut pos, mut vel, mut acc, _, body, opt_sat, opt_central) in
        bodies_query.iter_mut()
    {
        if opt_central.is_some() && !is_galaxy_collision {
            pos.0 = DVec3::ZERO;
            vel.0 = DVec3::ZERO;
            acc.0 = DVec3::ZERO;
        } else if let Some(&b) = body_map.get(&e) {
            let r_mag = b.pos.length();
            let debris_escape_radius = if is_little_red_dot { 15_000.0 } else { 2000.0 };
            let is_minor_debris = matches!(
                body.body_type,
                BodyType::Planetesimal
                    | BodyType::Asteroid
                    | BodyType::Comet
                    | BodyType::DustGrain
                    | BodyType::DebrisRing
            );
            if !is_galaxy_collision
                && is_minor_debris
                && r_mag > debris_escape_radius
                && b.vel.dot(b.pos) > 0.0
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

type GmcGridData<'a> = (
    hashbrown::HashMap<(i32, i32, i32), Vec<usize>>,
    Option<usize>,
    &'a hashbrown::HashMap<Entity, DVec3>,
    f64,
);

fn build_gmc_near_field_grid<'a>(
    far_field: Option<&'a hashbrown::HashMap<Entity, DVec3>>,
    is_gmc_cluster: bool,
    massive_data: &[(DVec3, DVec3, f64, f64, usize)],
    star_index: Option<usize>,
) -> Option<GmcGridData<'a>> {
    far_field
        .filter(|_| is_gmc_cluster && massive_data.len() >= GMC_FAR_FIELD_MIN_BODIES)
        .map(|far_acc| {
            let inv_cell = 1.0 / GMC_FAR_FIELD_NEAR_RADIUS_AU;
            let mut grid: hashbrown::HashMap<(i32, i32, i32), Vec<usize>> =
                hashbrown::HashMap::with_capacity(massive_data.len());
            let mut star_slot = None;
            for (k, &(p, _, _, _, idx)) in massive_data.iter().enumerate() {
                if Some(idx) == star_index {
                    star_slot = Some(k);
                } else {
                    grid.entry(grid_cell(p, inv_cell)).or_default().push(k);
                }
            }
            (grid, star_slot, far_acc, inv_cell)
        })
}

fn apply_substep_type_i_migration(
    body_data: &mut [PhysicsBodyEntry],
    star_mass: f64,
    gas_density_scale: f32,
) {
    if gas_density_scale <= 0.001 {
        return;
    }
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
            gas_density_scale,
            &mut body.acc,
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
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
    is_galaxy_collision: bool,
    lhb_state: &mut crate::game::phases::LateHeavyBombardmentState,
    jupiter_entity: Option<Entity>,
    saturn_entity: Option<Entity>,
    ice_giant_entities: &hashbrown::HashSet<Entity>,
    elapsed_years: f64,
    far_field: Option<&hashbrown::HashMap<Entity, DVec3>>,
) {
    let mut massive_data = Vec::with_capacity(massive_indices.len());
    let mut snapshot_buffer = Vec::with_capacity(body_data.len());
    let mut new_accelerations = vec![DVec3::ZERO; body_data.len()];

    for step_idx in 0..n_substeps {
        let substep_elapsed_years = elapsed_years + (step_idx as f64) * sub_dt;

        for body in body_data.iter_mut() {
            if body.is_central_star && !is_galaxy_collision {
                body.pos = DVec3::ZERO;
                body.vel = DVec3::ZERO;
                body.acc = DVec3::ZERO;
            } else {
                let is_bound =
                    is_body_bound_to_star(body.pos, body.vel, star_pos, star_mass, body.mass);
                apply_pert_kick(body, sub_dt * 0.5, star_pos, star_mass, is_bound);
            }
        }

        advance_symplectic_leapfrog_drift(
            body_data,
            sub_dt,
            star_pos,
            star_mass,
            &mut snapshot_buffer,
            is_galaxy_collision,
        );

        massive_data.clear();
        massive_data.extend(massive_indices.iter().filter_map(|&idx| {
            let b = body_data.get(idx)?;
            Some((b.pos, b.vel, b.mass, b.radius, idx))
        }));

        let cluster_com_mass = if is_gmc_cluster {
            let mut total_m = 0.0;
            for &(_, _, m_mass, _, _) in &massive_data {
                if m_mass >= 0.5 {
                    total_m += m_mass;
                }
            }
            (DVec3::ZERO, total_m)
        } else {
            (DVec3::ZERO, 0.0)
        };

        let near_grid =
            build_gmc_near_field_grid(far_field, is_gmc_cluster, &massive_data, star_index);
        let near_field = near_grid
            .as_ref()
            .map(|(grid, star_slot, far_acc, inv_cell)| GmcNearField {
                inv_cell: *inv_cell,
                near_sq: GMC_FAR_FIELD_NEAR_RADIUS_AU * GMC_FAR_FIELD_NEAR_RADIUS_AU,
                grid,
                star_slot: *star_slot,
                far_acc,
            });
        let near_field_ref = near_field.as_ref();

        new_accelerations
            .par_iter_mut()
            .zip(body_data.par_iter())
            .enumerate()
            .for_each(|(i, (acc_out, b))| {
                *acc_out = compute_single_body_acc(
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
                    substep_elapsed_years,
                    near_field_ref,
                );
            });

        for (body, &new_acc) in body_data.iter_mut().zip(&new_accelerations) {
            if body.is_central_star && !is_galaxy_collision {
                body.acc = DVec3::ZERO;
            } else {
                body.acc = new_acc;
            }
        }

        apply_substep_type_i_migration(body_data, star_mass, config.gas_density_scale);

        apply_planetary_migration_and_lhb(
            body_data,
            lhb_state,
            jupiter_entity,
            saturn_entity,
            ice_giant_entities,
            sub_dt,
            step_idx,
            is_compact_system,
        );

        apply_velocity_kick_and_limits(
            body_data,
            sub_dt,
            star_mass,
            star_pos,
            is_little_red_dot,
            is_gmc_cluster,
            is_galaxy_collision,
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
    is_galaxy_collision: bool,
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
    let is_galaxy_collision = scenario_state
        .is_some_and(|s| s.current_preset == ScenarioPreset::GalacticCollision)
        || disk_params.outer_radius_au >= 10_000.0;
    let star_index = if is_galaxy_collision {
        None
    } else {
        body_data.iter().position(|b| b.is_central_star)
    };
    let (star_mass, star_pos, is_central_quasi) = if let Some(idx) = star_index {
        if let Some(b) = body_data.get(idx) {
            (b.mass, b.pos, b.body_type == BodyType::QuasiStar)
        } else {
            (1.0, DVec3::ZERO, false)
        }
    } else if is_gmc_cluster || is_galaxy_collision {
        (0.0, DVec3::ZERO, false)
    } else {
        (1.0, DVec3::ZERO, false)
    };

    let has_black_hole = body_data.iter().any(|b| b.body_type == BodyType::BlackHole);
    // A cluster naturally grows massive stars and black holes; those must not flip the whole
    // simulation into Little-Red-Dot mode (125x smaller timestep + LRD-specific clamps).
    let is_little_red_dot = !is_gmc_cluster
        && !is_galaxy_collision
        && (scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::LittleRedDot)
            || is_central_quasi
            || has_black_hole
            || star_mass >= 15.0);

    let is_compact_system = (star_mass < 0.25 && disk_params.outer_radius_au < 1.0)
        || scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::Trappist1System);
    // GMC clusters have wide open orbits (>= 1.4 AU); a massive central black hole in a cluster
    // must not trigger the relativistic microsecond pericenter clamps used for close-in S-stars.
    let is_smbh = !is_gmc_cluster && !is_galaxy_collision && star_mass > 100_000.0;
    let max_substeps = if is_galaxy_collision {
        64
    } else if is_gmc_cluster {
        // GMC clusters feature hundreds of stars and gas clumps; clamp substeps to 64 to guarantee
        // smooth frame rates while enabling fast-forwarding up to 5+ years per frame (300+ yr/s at 60 FPS).
        config.max_substeps_per_frame.clamp(1, 64)
    } else if is_smbh || is_little_red_dot || is_compact_system {
        config.max_substeps_per_frame.max(128)
    } else {
        config.max_substeps_per_frame
    };
    let eff_dt = if is_galaxy_collision {
        0.25
    } else if is_gmc_cluster {
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

    let dyn_limit =
        compute_dynamical_dt_limit(body_data, &massive_indices, star_index, star_pos, star_mass);

    // Prevent target_dt from forcing an unsafe sub_dt when fast-forwarding in chaotic GMC clusters or non-Keplerian interactions
    let safe_target_dt = if is_galaxy_collision {
        target_dt.min(10.0)
    } else if is_gmc_cluster {
        target_dt.min(max_substeps as f64 * eff_dt)
    } else if let Some(cap) = dyn_limit {
        target_dt.min(max_substeps as f64 * cap)
    } else {
        target_dt
    };
    let n_substeps = ((safe_target_dt / eff_dt).ceil() as usize).clamp(1, max_substeps);
    let sub_dt = safe_target_dt / (n_substeps as f64);

    for body in body_data {
        if body.acc.length_squared() < 1e-12 && !body.is_central_star {
            let r_vec = body.pos - star_pos;
            let dist_sq = r_vec.length_squared() + softening_sq;
            let dist = dist_sq.sqrt();
            if is_body_bound_to_star(body.pos, body.vel, star_pos, star_mass, body.mass) {
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
        is_galaxy_collision,
        n_substeps,
        sub_dt,
        massive_indices,
    }
}

fn compute_dynamical_dt_limit(
    body_data: &[PhysicsBodyEntry],
    massive_indices: &[usize],
    star_index: Option<usize>,
    star_pos: DVec3,
    star_mass: f64,
) -> Option<f64> {
    let mut min_dt_limit: Option<f64> = None;

    // 1. Multi-star / Compact Binary:
    // Only applies if there is a third massive body (massive_indices.len() > 2)
    // and a companion star/remnant with mass >= 1e-3 * star_mass.
    if massive_indices.len() > 2 && star_mass > 1e-6 {
        for &idx in massive_indices {
            if Some(idx) == star_index {
                continue;
            }
            if let Some(b) = body_data.get(idx) {
                if b.body_type.is_star_or_remnant() && b.mass >= 1e-3 * star_mass {
                    let r = (b.pos - star_pos).length().max(1e-4);
                    let mu_bin = G_ASTRO * (star_mass + b.mass);
                    let t_bin = 2.0 * std::f64::consts::PI * (r * r * r / mu_bin).sqrt();
                    let step_cap = (0.02 * t_bin).max(1e-7);
                    min_dt_limit = Some(min_dt_limit.map_or(step_cap, |c| c.min(step_cap)));
                }
            }
        }
    }

    // 2. Unbound massive interloper (Rogue Planet):
    // Only applies to incoming rogue planets/interlopers approaching the system from deep space
    // to ensure high-speed close encounters are numerically resolved without tunneling.
    for &idx in massive_indices {
        if Some(idx) == star_index {
            continue;
        }
        if let Some(b) = body_data.get(idx) {
            let is_interloper = b.name.contains("Rogue") || b.name.contains("Interloper");
            if is_interloper && b.mass >= EARTH_MASS_SOLAR * 0.1 {
                let is_bound = is_body_bound_to_star(b.pos, b.vel, star_pos, star_mass, b.mass);
                let rel_pos = b.pos - star_pos;
                let is_inbound = rel_pos.dot(b.vel) < 0.0;
                if !is_bound && is_inbound {
                    let speed = b.vel.length();
                    if speed > 1e-5 {
                        let r = rel_pos.length().max(0.1);
                        let crossing_time = r / speed;
                        let step_cap = (0.05 * crossing_time).max(1e-4);
                        min_dt_limit = Some(min_dt_limit.map_or(step_cap, |c| c.min(step_cap)));
                    }
                }
            }
        }
    }

    min_dt_limit
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
pub fn enforce_dynamic_galactic_nucleus(
    body_data: &mut [PhysicsBodyEntry],
    is_gmc_cluster: bool,
    commands: &mut Commands,
) {
    if !is_gmc_cluster {
        return;
    }

    let candidate_anchor = body_data
        .iter()
        .enumerate()
        .filter(|(_, b)| b.body_type.is_star_or_remnant())
        .max_by(|a, b| {
            a.1.mass
                .partial_cmp(&b.1.mass)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, b)| (i, b.entity, b.mass));

    let Some((cand_idx, cand_entity, cand_mass)) = candidate_anchor else {
        return;
    };

    let current_central = body_data
        .iter()
        .enumerate()
        .find(|(_, b)| b.is_central_star)
        .map(|(i, b)| (i, b.entity, b.mass));

    let should_transfer = match current_central {
        None => true,
        Some((_curr_idx, curr_entity, curr_mass)) => {
            curr_entity != cand_entity
                && (cand_mass > curr_mass * 1.20 || (curr_mass < 5.0 && cand_mass >= 6.0))
        }
    };

    if should_transfer {
        if let Some((curr_idx, curr_entity, _)) = current_central {
            if curr_entity != cand_entity {
                if let Some(old_b) = body_data.get_mut(curr_idx) {
                    old_b.is_central_star = false;
                }
                if let Ok(mut cmd) = commands.get_entity(curr_entity) {
                    cmd.remove::<CentralStar>();
                }
            }
        }

        if let Some(anchor_b) = body_data.get(cand_idx) {
            let anchor_pos = anchor_b.pos;
            let anchor_vel = anchor_b.vel;

            // Smoothly translate the entire cluster so the Nucleus is precisely at DVec3::ZERO!
            for b in body_data.iter_mut() {
                b.pos -= anchor_pos;
                b.vel -= anchor_vel;
            }

            if let Some(anchor_mut) = body_data.get_mut(cand_idx) {
                anchor_mut.is_central_star = true;
            }
            if let Ok(mut cmd) = commands.get_entity(cand_entity) {
                cmd.insert(CentralStar);
            }
        }
    }
}

fn compute_effective_softening_au(
    scenario_state: Option<&ActiveScenarioState>,
    disk_params: &DiskParameters,
    config: &SimulationConfig,
) -> f64 {
    let is_compact_system = scenario_state
        .is_some_and(|s| s.current_preset == ScenarioPreset::Trappist1System)
        || (disk_params.central_star_mass < 0.25 && disk_params.outer_radius_au < 1.0);
    if is_compact_system {
        0.00005
    } else {
        config.softening_au
    }
}

fn active_tractor_beam(player_state: &PlayerInteractionState) -> Option<(DVec3, f64)> {
    if player_state.active_tool == PlayerTool::GravitationalTractor {
        player_state
            .tractor_position
            .filter(|_| player_state.tractor_mass > 0.0)
            .map(|pos| (pos, player_state.tractor_mass))
    } else {
        None
    }
}

fn apply_player_velocity_impulse(
    body_data: &mut [PhysicsBodyEntry],
    player_state: &PlayerInteractionState,
) {
    if let (Some(target), Some(dv)) = (
        player_state.impulse_target_entity,
        player_state.impulse_delta_v,
    ) {
        if let Some(b) = body_data.iter_mut().find(|b| b.entity == target) {
            b.vel += dv;
        }
    }
}

#[allow(clippy::type_complexity, reason = "N-body Simulation State")]
pub fn step_physics_simulation(
    far_field: Option<Res<crate::gpu::gmc_far_field::GmcFarField>>,
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
    let eff_softening_au =
        compute_effective_softening_au(scenario_state.as_deref(), &disk_params, &config);
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

    let tractor = active_tractor_beam(&player_state);
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
        ctx.is_galaxy_collision,
        &mut lhb_state,
        jupiter_entity,
        saturn_entity,
        &ice_giant_entities,
        sim_time.elapsed_years,
        far_field
            .as_deref()
            .filter(|_| ctx.is_gmc_cluster)
            .map(|f| &f.acc),
    );

    apply_player_velocity_impulse(&mut body_data, &player_state);

    write_back_physics_results(
        &mut commands,
        &body_data,
        ctx.is_little_red_dot,
        ctx.is_galaxy_collision,
        &mut bodies_query,
    );

    update_physics_energy_monitor(
        &mut energy_monitor,
        &body_data,
        &ctx.massive_indices,
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
