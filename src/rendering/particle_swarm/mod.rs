//! High-performance real-time particle swarm renderer for glowing protoplanetary particles.

pub mod billboards;
pub mod promotions;
pub mod setup;
pub mod simulation;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

pub use billboards::{sync_mesh_attributes, update_billboard_mesh_quads};
pub use promotions::{check_clump_promotions, replenish_cleared_particles, spawn_promoted_bodies};
pub use setup::{create_soft_particle_texture, reseed_particle_swarm, setup_particle_swarm};
pub use simulation::{
    apply_particle_accretion_to_bodies, build_spatial_hash_bins, compute_gas_aerodynamic_drift,
    integrate_particles_and_collect_accretions, process_particle_collisions_and_sticking,
    ParticleIntegrationParams,
};

/// Marker component for the 50,000 particle visual swarm mesh.
#[derive(Component)]
pub struct ParticleSwarmMesh;

/// CPU storage for particle state synchronized with visual mesh buffers.
#[derive(Resource)]
pub struct ParticleSwarmData {
    pub positions: Vec<[f32; 3]>,
    pub velocities: Vec<[f32; 3]>,
    pub masses: Vec<f32>,
    pub compositions: Vec<Composition>,
    pub temperatures: Vec<f32>,
    pub colors: Vec<[f32; 4]>,
    pub mesh_positions: Vec<[f32; 3]>,
    pub mesh_colors: Vec<[f32; 4]>,
    pub bin_heads: Vec<i32>,
    pub bin_next: Vec<i32>,
    pub mesh_handle: Handle<Mesh>,
    pub count: usize,
    pub base_mass: f32,
    pub is_dirty: bool,
    pub pending_gpu_accretions: Vec<(usize, f32)>,
}

fn flush_dirty_mesh_if_paused(
    swarm: &mut Option<ResMut<ParticleSwarmData>>,
    camera_query: &Query<&Transform, With<Camera>>,
    disk_params: &DiskParameters,
    render_scale: f32,
    meshes: &mut Assets<Mesh>,
) {
    if let Some(ref mut data) = swarm {
        if data.is_dirty {
            let (cam_right, cam_up) = camera_query.single().map_or((Vec3::X, Vec3::Y), |cam| {
                (cam.right().as_vec3(), cam.up().as_vec3())
            });
            update_billboard_mesh_quads(data, cam_right, cam_up, disk_params, render_scale);
            sync_mesh_attributes(data, meshes);
            data.is_dirty = false;
        }
    }
}

fn resolve_camera_basis(camera_query: &Query<&Transform, With<Camera>>) -> (Vec3, Vec3) {
    camera_query.single().map_or((Vec3::X, Vec3::Y), |cam| {
        (cam.right().as_vec3(), cam.up().as_vec3())
    })
}

type StarAnchorTuple<'a> = (
    &'a SimPosition,
    &'a Mass,
    &'a IgnitionState,
    Option<&'a BlackHoleStarState>,
    &'a CelestialBody,
);

fn resolve_anchor_star<'a>(
    star_query: &'a Query<
        (
            &SimPosition,
            &Mass,
            &IgnitionState,
            Option<&BlackHoleStarState>,
            &CelestialBody,
        ),
        With<CentralStar>,
    >,
    defaults: &'a (SimPosition, Mass, IgnitionState, CelestialBody),
) -> StarAnchorTuple<'a> {
    if let Some((p, m, ign, bhs, body)) = star_query.iter().next() {
        (p, m, ign, bhs, body)
    } else {
        (&defaults.0, &defaults.1, &defaults.2, None, &defaults.3)
    }
}

fn compute_tractor_pos_mass(player_state: &PlayerInteractionState) -> [f32; 4] {
    if let (PlayerTool::GravitationalTractor, Some(pos)) =
        (player_state.active_tool, player_state.tractor_position)
    {
        [
            pos.x as f32,
            pos.y as f32,
            pos.z as f32,
            player_state.tractor_mass as f32,
        ]
    } else {
        [0.0; 4]
    }
}

fn compute_visual_flow_dt(speed_mult: f32) -> f32 {
    if speed_mult < 1.0 {
        (0.002 * speed_mult).max(1e-7)
    } else {
        (0.002 * (1.0 + speed_mult.log10() * 2.0)).min(0.08)
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Protoplanetary disk evolution step"
)]
fn step_protoplanetary_disk_mechanics(
    commands: &mut Commands,
    data: &mut ParticleSwarmData,
    params: &ParticleIntegrationParams,
    massive_query: &mut Query<
        (
            Entity,
            &SimPosition,
            &mut Mass,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
        ),
        Without<CentralStar>,
    >,
    massive_bodies: &[(Entity, DVec3, f64, BodyType)],
    disk_params: &DiskParameters,
    config: &mut SimulationConfig,
    speed_mult: f32,
    elapsed_years: f64,
) {
    let accreted_events = integrate_particles_and_collect_accretions(data, params);
    let mut pending_accretions = std::mem::take(&mut data.pending_gpu_accretions);
    apply_particle_accretion_to_bodies(
        massive_query,
        massive_bodies,
        accreted_events,
        &mut pending_accretions,
        f64::from(params.star_m),
        disk_params,
    );
    data.pending_gpu_accretions = pending_accretions;

    if !params.gpu_active && elapsed_years >= 15.0 {
        build_spatial_hash_bins(data, disk_params);
        process_particle_collisions_and_sticking(data, disk_params, speed_mult);
    }

    let current_ecs_count = massive_bodies.len();
    let is_massive_disk = params.star_m > 10.0;
    let (promotions, active_count) = check_clump_promotions(
        data,
        disk_params,
        current_ecs_count,
        is_massive_disk,
        elapsed_years,
    );
    spawn_promoted_bodies(commands, promotions, disk_params, is_massive_disk);

    replenish_cleared_particles(
        data,
        disk_params,
        config,
        params.star_is_ignited,
        speed_mult,
        active_count,
    );
}

pub fn update_particle_swarm(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    mut config: ResMut<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    player_state: Res<PlayerInteractionState>,
    star_query: Query<
        (
            &SimPosition,
            &Mass,
            &IgnitionState,
            Option<&BlackHoleStarState>,
            &CelestialBody,
        ),
        With<CentralStar>,
    >,
    mut massive_query: Query<
        (
            Entity,
            &SimPosition,
            &mut Mass,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
        ),
        Without<CentralStar>,
    >,
    camera_query: Query<&Transform, With<Camera>>,
    lhb_state: Option<Res<crate::game::phases::LateHeavyBombardmentState>>,
    mut swarm: Option<ResMut<ParticleSwarmData>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        flush_dirty_mesh_if_paused(
            &mut swarm,
            &camera_query,
            &disk_params,
            config.particle_render_scale,
            &mut meshes,
        );
        return;
    }
    let Some(mut data) = swarm else {
        return;
    };

    let (cam_right, cam_up) = resolve_camera_basis(&camera_query);

    let is_galaxy_collision = disk_params.outer_radius_au >= 10_000.0;
    if is_galaxy_collision {
        update_billboard_mesh_quads(
            &mut data,
            cam_right,
            cam_up,
            &disk_params,
            config.particle_render_scale,
        );
        sync_mesh_attributes(&data, &mut meshes);
        data.is_dirty = false;
        return;
    }

    let defaults = (
        SimPosition(DVec3::ZERO),
        Mass(disk_params.central_star_mass),
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 0.0,
        },
        CelestialBody {
            name: "Central Star".to_string(),
            body_type: BodyType::YellowDwarf,
        },
    );
    let (star_pos, star_mass, ignition, opt_bhs, star_body) =
        resolve_anchor_star(&star_query, &defaults);

    let massive_bodies: Vec<(Entity, DVec3, f64, BodyType)> = massive_query
        .iter()
        .map(|(e, p, m, _, _, body)| (e, p.0, m.0, body.body_type))
        .collect();

    let quasar_blown_out =
        opt_bhs.is_some_and(|s| s.is_blown_out) || star_body.body_type == BodyType::BlackHole;
    let tractor_pos_mass = compute_tractor_pos_mass(&player_state);
    let speed_mult = time_warp.multiplier as f32;
    let visual_flow_dt = compute_visual_flow_dt(speed_mult);
    let gpu_active = config.enable_gpu_compute && config.gpu_compute_active;
    let (lhb_active, lhb_resonance) = lhb_state
        .as_ref()
        .map_or((false, false), |lhb| (lhb.is_active, lhb.resonance_crossed));

    let params = ParticleIntegrationParams {
        star_pos_f32: [star_pos.x as f32, star_pos.y as f32, star_pos.z as f32],
        star_m: star_mass.0 as f32,
        g_const: G_ASTRO as f32,
        shockwave_r: ignition.shockwave_radius as f32,
        enable_gas_drag: config.enable_gas_drag,
        gas_scale: config.gas_density_scale,
        star_is_ignited: ignition.is_ignited,
        quasar_blown_out,
        tractor_pos_mass,
        speed_mult,
        visual_flow_dt,
        gpu_active,
        lhb_active,
        lhb_resonance,
        disk_params: &disk_params,
        massive_bodies: &massive_bodies,
        base_mass: data.base_mass,
    };

    step_protoplanetary_disk_mechanics(
        &mut commands,
        &mut data,
        &params,
        &mut massive_query,
        &massive_bodies,
        &disk_params,
        &mut config,
        speed_mult,
        sim_time.elapsed_years,
    );

    update_billboard_mesh_quads(
        &mut data,
        cam_right,
        cam_up,
        &disk_params,
        config.particle_render_scale,
    );
    sync_mesh_attributes(&data, &mut meshes);
}

/// Plugin registering the 50,000 particle visual swarm.
pub struct ParticleSwarmPlugin;

impl Plugin for ParticleSwarmPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_particle_swarm)
            .add_systems(Update, update_particle_swarm);
    }
}
