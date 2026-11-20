//! Symplectic N-Body Gravity and Aerodynamic Gas Drag Physics Engine.
//! This system handles ONLY ECS-promoted massive bodies (planets, protoplanets).
//! The 50k particle swarm physics is handled entirely on the GPU.

pub mod diagnostics;
pub mod forces;
pub mod integrator;

use bevy::math::DVec3;
use bevy::prelude::*;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::gpu::gmc_far_field::{GMC_FAR_FIELD_MIN_BODIES, GMC_FAR_FIELD_NEAR_RADIUS_AU};
use crate::simulation::components::*;
use crate::simulation::disk_migration::apply_type_i_torque_acc;
use crate::simulation::resources::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::*;

use diagnostics::update_physics_energy_monitor;
pub use diagnostics::{enforce_dynamic_galactic_nucleus, update_simulation_visual_time};
pub use forces::*;
pub use integrator::*;

#[allow(clippy::type_complexity, reason = "Physics State Writeback")]
fn write_back_physics_results(
    commands: &mut Commands,
    body_data: &[PhysicsBodyEntry],
    entity_to_idx: &mut FxHashMap<Entity, usize>,
    allow_wide_debris: bool,
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
            Has<crate::simulation::oort::OortComet>,
        ),
        (
            Without<crate::simulation::probes::SpaceProbe>,
            Without<crate::simulation::terraforming::BombardmentProjectile>,
        ),
    >,
) {
    let mut escaped_minor_debris: SmallVec<[Entity; 16]> = SmallVec::new();
    entity_to_idx.clear();
    entity_to_idx.extend(body_data.iter().enumerate().map(|(i, b)| (b.entity, i)));

    for (e, mut m, mut pos, mut vel, mut acc, _, body, opt_sat, opt_central, is_oort_comet) in
        bodies_query.iter_mut()
    {
        let body_entry = entity_to_idx.get(&e).and_then(|&idx| body_data.get(idx));
        let is_central = body_entry.map_or(opt_central.is_some(), |b| b.is_central_star);
        if is_central && !is_galaxy_collision {
            pos.0 = DVec3::ZERO;
            vel.0 = DVec3::ZERO;
            acc.0 = DVec3::ZERO;
        } else if let Some(b) = body_entry {
            let r_mag = b.pos.length();
            let debris_escape_radius = if is_oort_comet {
                100_000.0
            } else if allow_wide_debris {
                15_000.0
            } else {
                2000.0
            };
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
            } else if opt_sat.is_some()
                && let Ok(mut cmd) = commands.get_entity(e)
            {
                cmd.remove::<SatelliteOf>();
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
    ice_giant_entities: &mut FxHashSet<Entity>,
) -> (Option<Entity>, Option<Entity>) {
    let mut jupiter_entity = None;
    let mut saturn_entity = None;
    ice_giant_entities.clear();

    for b in body_data {
        if !b.body_type.is_planet() || b.body_type == BodyType::Moon {
            continue;
        }
        let r = (b.pos.x * b.pos.x + b.pos.z * b.pos.z).sqrt();
        if b.role_flags & ROLE_MOON_NAMED != 0 {
            continue;
        }
        if b.role_flags & ROLE_JUPITER != 0 {
            jupiter_entity = Some(b.entity);
        } else if b.role_flags & ROLE_SATURN != 0 {
            saturn_entity = Some(b.entity);
        } else if matches!(b.body_type, BodyType::GasGiant) || b.mass >= JUPITER_MASS_SOLAR * 0.05 {
            if r < 7.5 && jupiter_entity.is_none() {
                jupiter_entity = Some(b.entity);
            } else if (7.5..16.0).contains(&r) && saturn_entity.is_none() {
                saturn_entity = Some(b.entity);
            }
        }
        if matches!(b.body_type, BodyType::IceGiant) || b.role_flags & ROLE_ICE_GIANT_NAMED != 0 {
            ice_giant_entities.insert(b.entity);
        }
    }
    (jupiter_entity, saturn_entity)
}

fn build_gmc_near_field_grid<'a>(
    grid: &'a mut FxHashMap<(i32, i32, i32), SmallVec<[usize; 8]>>,
    far_field: Option<&'a FxHashMap<Entity, DVec3>>,
    is_gmc_cluster: bool,
    massive_data: &[(DVec3, DVec3, f64, f64, usize)],
    star_index: Option<usize>,
) -> Option<GmcNearField<'a>> {
    let far_acc =
        far_field.filter(|_| is_gmc_cluster && massive_data.len() >= GMC_FAR_FIELD_MIN_BODIES)?;
    let inv_cell = 1.0 / GMC_FAR_FIELD_NEAR_RADIUS_AU;
    if grid.len() > massive_data.len().saturating_mul(4).max(128) {
        grid.clear();
    } else {
        for v in grid.values_mut() {
            v.clear();
        }
    }
    if grid.capacity() < massive_data.len() {
        grid.reserve(massive_data.len().saturating_sub(grid.len()));
    }
    let mut star_slot = None;
    for (k, &(p, _, _, _, idx)) in massive_data.iter().enumerate() {
        if Some(idx) == star_index {
            star_slot = Some(k);
        } else {
            grid.entry(grid_cell(p, inv_cell)).or_default().push(k);
        }
    }
    Some(GmcNearField {
        inv_cell,
        near_sq: GMC_FAR_FIELD_NEAR_RADIUS_AU * GMC_FAR_FIELD_NEAR_RADIUS_AU,
        grid,
        star_slot,
        far_acc,
    })
}

fn apply_substep_type_i_migration(
    body_data: &mut [PhysicsBodyEntry],
    star_mass: f64,
    gas_density_scale: f32,
    disk_params: &DiskParameters,
    is_gmc_cluster: bool,
    is_galaxy_collision: bool,
) {
    if is_gmc_cluster
        || is_galaxy_collision
        || gas_density_scale <= 0.001
        || disk_params.disk_mass <= 1e-6
    {
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
            disk_params,
            &mut body.acc,
        );
    }
}

/// Persistent scratch buffers reused across frames and substeps in `step_physics_simulation`
/// to eliminate per-frame and per-substep heap allocations.
#[derive(Default)]
pub struct PhysicsScratchBuffers {
    pub body_data: Vec<PhysicsBodyEntry>,
    pub massive_data: Vec<(DVec3, DVec3, f64, f64, usize)>,
    pub snapshot_buffer: Vec<(Entity, DVec3, DVec3, f64, f64)>,
    pub new_accelerations: Vec<DVec3>,
    pub entity_to_idx: FxHashMap<Entity, usize>,
    pub gmc_grid: FxHashMap<(i32, i32, i32), SmallVec<[usize; 8]>>,
    pub massive_indices: Vec<usize>,
    pub ice_giant_entities: FxHashSet<Entity>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Substep force evaluation dispatches across massive sources and environment fields"
)]
fn compute_substep_accelerations(
    ctx: &PhysicsSystemContext,
    body_data: &mut [PhysicsBodyEntry],
    new_accelerations: &mut [DVec3],
    massive_data: &[(DVec3, DVec3, f64, f64, usize)],
    config: &SimulationConfig,
    softening_sq: f64,
    tractor: Option<(DVec3, f64)>,
    cluster_com_mass: (DVec3, f64),
    substep_elapsed_years: f64,
    near_field_ref: Option<&GmcNearField<'_>>,
) {
    let eval_acc = |(i, (acc_out, b)): (usize, (&mut DVec3, &PhysicsBodyEntry))| {
        *acc_out = compute_single_body_acc_with_companion(
            i,
            b,
            config,
            ctx.star_mass,
            ctx.star_index,
            softening_sq,
            massive_data,
            tractor,
            ctx.is_little_red_dot,
            ctx.is_gmc_cluster,
            cluster_com_mass,
            substep_elapsed_years,
            near_field_ref,
            ctx.inner_companion_radius,
        );
    };

    if body_data.len() < 32 {
        new_accelerations
            .iter_mut()
            .zip(body_data.iter())
            .enumerate()
            .for_each(eval_acc);
    } else {
        new_accelerations
            .par_iter_mut()
            .zip(body_data.par_iter())
            .enumerate()
            .for_each(eval_acc);
    }

    for (body, &new_acc) in body_data.iter_mut().zip(new_accelerations.iter()) {
        body.acc = if body.is_central_star && !ctx.is_galaxy_collision {
            DVec3::ZERO
        } else {
            new_acc
        };
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Physics kernel requires all integration state explicitly for performance"
)]
fn run_physics_substeps(
    scratch: &mut PhysicsScratchBuffers,
    ctx: &PhysicsSystemContext,
    config: &SimulationConfig,
    disk_params: &DiskParameters,
    softening_sq: f64,
    tractor: Option<(DVec3, f64)>,
    lhb_state: &mut crate::game::phases::LateHeavyBombardmentState,
    jupiter_entity: Option<Entity>,
    saturn_entity: Option<Entity>,
    elapsed_years: f64,
    far_field: Option<&FxHashMap<Entity, DVec3>>,
) {
    let PhysicsScratchBuffers {
        body_data,
        massive_data,
        snapshot_buffer,
        new_accelerations,
        gmc_grid,
        massive_indices,
        ice_giant_entities,
        ..
    } = scratch;
    massive_data.clear();
    snapshot_buffer.clear();
    new_accelerations.clear();
    new_accelerations.resize(body_data.len(), DVec3::ZERO);

    for step_idx in 0..ctx.n_substeps {
        let substep_elapsed_years = elapsed_years + (step_idx as f64) * ctx.sub_dt;

        // Snapshot pre-kick parent velocities so bound satellites stay co-moving with
        // non-Kepler-split parents across the initial half-kick (zero rel_vel bias).
        let has_satellites = body_data.iter().any(|b| b.satellite.is_some());
        if has_satellites {
            snapshot_buffer.clear();
            snapshot_buffer.extend(
                body_data
                    .iter()
                    .filter(|b| b.satellite.is_none())
                    .map(|b| (b.entity, b.pos, b.vel, b.mass, b.radius)),
            );
            snapshot_buffer.sort_unstable_by_key(|&(e, ..)| e);
        }

        apply_initial_substep_half_kicks(
            body_data,
            ctx.sub_dt,
            ctx.star_pos,
            ctx.star_mass,
            ctx.is_gmc_cluster,
            ctx.is_galaxy_collision,
            ctx.inner_companion_radius,
        );

        if has_satellites {
            sync_satellite_velocities_to_parents(body_data, snapshot_buffer);
        }

        advance_symplectic_leapfrog_drift_with_companion(
            body_data,
            ctx.sub_dt,
            ctx.star_pos,
            ctx.star_mass,
            snapshot_buffer,
            ctx.is_galaxy_collision,
            ctx.is_gmc_cluster,
            ctx.inner_companion_radius,
        );

        massive_data.clear();
        massive_data.extend(massive_indices.iter().filter_map(|&idx| {
            let b = body_data.get(idx)?;
            Some((b.pos, b.vel, b.mass, b.radius, idx))
        }));

        let cluster_com_mass = compute_cluster_com_mass(ctx.is_gmc_cluster, massive_data);

        let near_field = build_gmc_near_field_grid(
            gmc_grid,
            far_field,
            ctx.is_gmc_cluster,
            massive_data,
            ctx.star_index,
        );
        compute_substep_accelerations(
            ctx,
            body_data,
            new_accelerations,
            massive_data,
            config,
            softening_sq,
            tractor,
            cluster_com_mass,
            substep_elapsed_years,
            near_field.as_ref(),
        );

        apply_substep_type_i_migration(
            body_data,
            ctx.star_mass,
            config.gas_density_scale,
            disk_params,
            ctx.is_gmc_cluster,
            ctx.is_galaxy_collision,
        );

        apply_planetary_migration_and_lhb(
            body_data,
            lhb_state,
            jupiter_entity,
            saturn_entity,
            ice_giant_entities,
            ctx.sub_dt,
            step_idx,
            ctx.is_compact_system,
            ctx.is_gmc_cluster,
            ctx.is_galaxy_collision,
        );

        apply_velocity_kick_and_limits_with_companion(
            body_data,
            ctx.sub_dt,
            ctx.star_mass,
            ctx.star_pos,
            ctx.is_little_red_dot,
            ctx.is_gmc_cluster,
            ctx.is_galaxy_collision,
            ctx.inner_companion_radius,
        );
        sync_satellite_velocities_to_parents(body_data, snapshot_buffer);
    }
}

fn apply_initial_substep_half_kicks(
    body_data: &mut [PhysicsBodyEntry],
    sub_dt: f64,
    star_pos: DVec3,
    star_mass: f64,
    is_gmc_cluster: bool,
    is_galaxy_collision: bool,
    inner_companion_radius: Option<f64>,
) {
    for body in body_data.iter_mut() {
        if body.is_central_star && !is_galaxy_collision && !is_gmc_cluster {
            body.pos = DVec3::ZERO;
            body.vel = DVec3::ZERO;
            body.acc = DVec3::ZERO;
        } else {
            let use_kepler = !is_gmc_cluster
                && should_use_kepler_splitting(
                    body.pos,
                    body.vel,
                    body.mass,
                    body.body_type,
                    star_pos,
                    star_mass,
                    inner_companion_radius,
                );
            apply_pert_kick(body, sub_dt * 0.5, star_pos, star_mass, use_kepler);
        }
    }
}

fn compute_cluster_com_mass(
    is_gmc_cluster: bool,
    massive_data: &[(DVec3, DVec3, f64, f64, usize)],
) -> (DVec3, f64) {
    if !is_gmc_cluster {
        return (DVec3::ZERO, 0.0);
    }
    let mut total_m = 0.0;
    for &(_, _, m_mass, _, _) in massive_data {
        if m_mass >= 0.5 {
            total_m += m_mass;
        }
    }
    (DVec3::ZERO, total_m)
}

struct PhysicsSystemContext {
    star_index: Option<usize>,
    star_mass: f64,
    star_pos: DVec3,
    inner_companion_radius: Option<f64>,
    is_little_red_dot: bool,
    is_compact_system: bool,
    is_gmc_cluster: bool,
    is_galaxy_collision: bool,
    n_substeps: usize,
    sub_dt: f64,
}

fn compute_inner_stellar_companion_radius(
    body_data: &[PhysicsBodyEntry],
    star_pos: DVec3,
    star_mass: f64,
) -> Option<f64> {
    if star_mass <= 1e-6 {
        return None;
    }
    body_data
        .iter()
        .filter(|b| {
            !b.is_central_star && b.body_type.is_star_or_remnant() && b.mass >= 0.05 * star_mass
        })
        .map(|b| (b.pos - star_pos).length())
        .min_by(f64::total_cmp)
}

fn analyze_physics_system(
    body_data: &mut [PhysicsBodyEntry],
    scenario_state: Option<&ActiveScenarioState>,
    disk_params: &DiskParameters,
    config: &SimulationConfig,
    dt: f64,
    target_dt: f64,
    softening_sq: f64,
    massive_indices: &mut Vec<usize>,
) -> PhysicsSystemContext {
    let is_gmc_cluster =
        scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);
    let is_galaxy_collision = scenario_state
        .is_some_and(|s| s.current_preset == ScenarioPreset::GalacticCollision)
        || disk_params.outer_radius_au >= 10_000.0;
    if is_gmc_cluster {
        // GMC clusters have no pinned central star: every star is a free N-body member.
        for b in body_data.iter_mut() {
            b.is_central_star = false;
        }
    }
    let star_index = if is_galaxy_collision || is_gmc_cluster {
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
    // Restrict Little-Red-Dot mode exclusively to genuine LRD / Quasi-Star configurations so
    // non-LRD massive stars and SMBH scenarios keep standard integration & cull scales.
    let is_little_red_dot = !is_gmc_cluster
        && !is_galaxy_collision
        && (scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::LittleRedDot)
            || is_central_quasi);

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
    } else if is_smbh
        || is_little_red_dot
        || is_compact_system
        || (has_black_hole && !is_gmc_cluster)
    {
        config.max_substeps_per_frame.max(128)
    } else if body_data.len() > 300 {
        // High-body systems (e.g. Solar System with 1,000+ belt minor bodies):
        // Keplerian drift is analytically exact for 2-body orbits. Clamping substeps to 16
        // prevents Rayon barrier overhead and thread starvation, ensuring fluid 60-120 FPS.
        config.max_substeps_per_frame.clamp(1, 16)
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
    } else if is_little_red_dot || is_compact_system || has_black_hole {
        dt.min(0.00004)
    } else if body_data.len() > 300 {
        // Slightly larger substep intervals (~17.5 hours) for minor body swarms where perturbations are small
        (dt * 2.5).max(0.002)
    } else {
        dt
    };

    massive_indices.clear();
    massive_indices.extend(
        body_data
            .iter()
            .enumerate()
            .filter(|(_, b)| is_massive_gravity_source(b.mass, b.body_type, b.is_central_star))
            .map(|(i, _)| i),
    );

    let dyn_limit =
        compute_dynamical_dt_limit(body_data, massive_indices, star_index, star_pos, star_mass);

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
    let step_dt = if is_galaxy_collision || is_gmc_cluster {
        eff_dt
    } else {
        dyn_limit.map_or(eff_dt, |cap| eff_dt.min(cap))
    }
    .max(1e-12);
    let n_substeps = ((safe_target_dt / step_dt).ceil() as usize).clamp(1, max_substeps);
    let sub_dt = safe_target_dt / (n_substeps as f64);

    let inner_companion_radius =
        compute_inner_stellar_companion_radius(body_data, star_pos, star_mass);

    initialize_zero_body_accelerations(
        body_data,
        star_pos,
        star_mass,
        softening_sq,
        is_gmc_cluster,
        inner_companion_radius,
    );

    PhysicsSystemContext {
        star_index,
        star_mass,
        star_pos,
        inner_companion_radius,
        is_little_red_dot,
        is_compact_system,
        is_gmc_cluster,
        is_galaxy_collision,
        n_substeps,
        sub_dt,
    }
}

fn initialize_zero_body_accelerations(
    body_data: &mut [PhysicsBodyEntry],
    star_pos: DVec3,
    star_mass: f64,
    softening_sq: f64,
    is_gmc_cluster: bool,
    inner_companion_radius: Option<f64>,
) {
    for body in body_data {
        // Initialize strictly zeroed startup accelerations only; never erase small distant accelerations (< 1e-6 AU/yr^2)
        if body.acc == DVec3::ZERO && !body.is_central_star && star_mass > 1e-12 {
            let r_vec = body.pos - star_pos;
            let dist_sq = r_vec.length_squared() + softening_sq;
            let dist = dist_sq.sqrt();
            if dist > 1e-12 {
                if !is_gmc_cluster
                    && should_use_kepler_splitting(
                        body.pos,
                        body.vel,
                        body.mass,
                        body.body_type,
                        star_pos,
                        star_mass,
                        inner_companion_radius,
                    )
                {
                    body.acc = DVec3::ZERO;
                } else {
                    body.acc = -(G_ASTRO * star_mass / (dist_sq * dist)) * r_vec;
                }
            }
        }
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
            if let Some(b) = body_data.get(idx)
                && b.body_type.is_star_or_remnant()
                && b.mass >= 1e-3 * star_mass
            {
                let r = (b.pos - star_pos).length().max(1e-4);
                let mu_bin = G_ASTRO * (star_mass + b.mass);
                let t_bin = 2.0 * std::f64::consts::PI * (r * r * r / mu_bin).sqrt();
                let step_cap = (0.02 * t_bin).max(1e-7);
                min_dt_limit = Some(min_dt_limit.map_or(step_cap, |c| c.min(step_cap)));
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
            let is_interloper = b.role_flags & ROLE_INTERLOPER != 0;
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
    energy_monitor: &mut EnergyMonitor,
) {
    if let (Some(target), Some(dv)) = (
        player_state.impulse_target_entity,
        player_state.impulse_delta_v,
    ) && let Some(b) = body_data.iter_mut().find(|b| b.entity == target)
    {
        b.vel += dv;
        energy_monitor.request_rebaseline();
    }
}

fn calculate_target_dt(
    config: &SimulationConfig,
    time_warp: &TimeWarp,
    time: Option<&Time>,
    has_window: bool,
) -> f64 {
    let dt = config.base_dt_yr;
    let wall_frame_scale = if time_warp.step_once {
        1.0
    } else if has_window {
        time.map(Time::delta_secs_f64)
            .filter(|&d| (0.001..=0.5).contains(&d))
            .map_or(1.0, |d| (d * 60.0).clamp(0.05, 4.0))
    } else {
        time.map(Time::delta_secs_f64)
            .filter(|&d| (0.016..=0.5).contains(&d))
            .map_or(1.0, |d| (d * 60.0).max(1.0))
    };
    dt * time_warp.multiplier.max(TimeWarp::MIN_SPEED) * wall_frame_scale
}

type PhysicsBodiesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Mass,
        &'static mut SimPosition,
        &'static mut SimVelocity,
        &'static mut SimAcceleration,
        &'static Radius,
        &'static CelestialBody,
        Option<&'static mut SatelliteOf>,
        Option<&'static CentralStar>,
        Has<crate::simulation::oort::OortComet>,
    ),
    (
        Without<crate::simulation::probes::SpaceProbe>,
        Without<crate::simulation::terraforming::BombardmentProjectile>,
    ),
>;

fn populate_physics_bodies(
    bodies_query: &PhysicsBodiesQuery<'_, '_>,
    body_data: &mut Vec<PhysicsBodyEntry>,
) {
    body_data.clear();
    body_data.extend(bodies_query.iter().map(
        |(e, m, pos, vel, acc, rad, body, sat, opt_c, _is_oort)| PhysicsBodyEntry {
            entity: e,
            mass: m.0,
            pos: pos.0,
            vel: vel.0,
            acc: acc.0,
            radius: rad.0,
            body_type: body.body_type,
            satellite: sat.copied(),
            is_central_star: opt_c.is_some(),
            role_flags: compute_body_role_flags(&body.name),
        },
    ));
}

#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "N-body Simulation State"
)]
pub fn step_physics_simulation(
    time: Option<Res<Time>>,
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
    mut bodies_query: PhysicsBodiesQuery<'_, '_>,
    windows: Query<(), With<Window>>,
    mut scratch: Local<PhysicsScratchBuffers>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        sim_time.current_dt_yr = 0.0;
        return;
    }

    let dt = config.base_dt_yr;
    let target_dt = calculate_target_dt(&config, &time_warp, time.as_deref(), !windows.is_empty());
    let eff_softening_au =
        compute_effective_softening_au(scenario_state.as_deref(), &disk_params, &config);
    let softening_sq = eff_softening_au * eff_softening_au;

    populate_physics_bodies(&bodies_query, &mut scratch.body_data);

    let is_gmc_cluster = scenario_state
        .as_deref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);
    enforce_dynamic_galactic_nucleus(&mut scratch.body_data, is_gmc_cluster, &mut commands);

    if scratch.body_data.is_empty() {
        sim_time.elapsed_years += target_dt;
        sim_time.current_dt_yr = target_dt;
        sim_time.step_count += 1;
        return;
    }

    let scratch = &mut *scratch;
    let ctx = analyze_physics_system(
        &mut scratch.body_data,
        scenario_state.as_deref(),
        &disk_params,
        &config,
        dt,
        target_dt,
        softening_sq,
        &mut scratch.massive_indices,
    );

    let tractor = active_tractor_beam(&player_state);
    let (jupiter_entity, saturn_entity) =
        identify_giant_planets(&scratch.body_data, &mut scratch.ice_giant_entities);

    run_physics_substeps(
        scratch,
        &ctx,
        &config,
        &disk_params,
        softening_sq,
        tractor,
        &mut lhb_state,
        jupiter_entity,
        saturn_entity,
        sim_time.elapsed_years,
        far_field
            .as_deref()
            .filter(|_| ctx.is_gmc_cluster)
            .map(|f| &f.acc),
    );

    apply_player_velocity_impulse(&mut scratch.body_data, &player_state, &mut energy_monitor);

    let bufs = &mut *scratch;
    write_back_physics_results(
        &mut commands,
        &bufs.body_data,
        &mut bufs.entity_to_idx,
        ctx.is_little_red_dot || ctx.star_mass > 100_000.0,
        ctx.is_galaxy_collision,
        &mut bodies_query,
    );

    update_physics_energy_monitor(
        &mut energy_monitor,
        &scratch.body_data,
        &scratch.massive_indices,
        ctx.star_index,
        ctx.star_pos,
        ctx.star_mass,
        eff_softening_au,
        ctx.is_gmc_cluster,
        ctx.is_galaxy_collision,
    );

    let actual_integrated_time = ctx.sub_dt * ctx.n_substeps as f64;
    sim_time.elapsed_years += actual_integrated_time;
    sim_time.current_dt_yr = actual_integrated_time;
    sim_time.step_count += ctx.n_substeps as u64;
}
