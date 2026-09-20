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
    apply_particle_accretion_to_bodies, build_spatial_hash_bins,
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

pub fn update_particle_swarm(
    mut commands: Commands,
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
    swarm: Option<ResMut<ParticleSwarmData>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }
    let Some(mut data) = swarm else {
        return;
    };
    let Ok((star_pos, star_mass, ignition, opt_bhs, star_body)) = star_query.single() else {
        return;
    };

    let (cam_right, cam_up) = if let Ok(cam_t) = camera_query.single() {
        (cam_t.right().as_vec3(), cam_t.up().as_vec3())
    } else {
        (Vec3::X, Vec3::Y)
    };

    let massive_bodies: Vec<(Entity, DVec3, f64, BodyType)> = massive_query
        .iter()
        .map(|(e, p, m, _, _, body)| (e, p.0, m.0, body.body_type))
        .collect();

    let quasar_blown_out =
        opt_bhs.is_some_and(|s| s.is_blown_out) || star_body.body_type == BodyType::BlackHole;

    let tractor_pos_mass = if let (PlayerTool::GravitationalTractor, Some(pos)) =
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
    };

    let speed_mult = time_warp.multiplier as f32;
    let visual_flow_dt = if speed_mult < 1.0 {
        (0.002 * speed_mult).max(1e-7)
    } else {
        (0.002 * (1.0 + speed_mult.log10() * 2.0)).min(0.08)
    };
    let gpu_active = config.enable_gpu_compute && config.gpu_compute_active;
    let (lhb_active, lhb_resonance) = if let Some(ref lhb) = lhb_state {
        (lhb.is_active, lhb.resonance_crossed)
    } else {
        (false, false)
    };

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
    };

    let accreted_events = integrate_particles_and_collect_accretions(&mut data, &params);
    let mut pending_accretions = std::mem::take(&mut data.pending_gpu_accretions);
    apply_particle_accretion_to_bodies(
        &mut massive_query,
        &massive_bodies,
        accreted_events,
        &mut pending_accretions,
        star_mass.0,
        &disk_params,
    );
    data.pending_gpu_accretions = pending_accretions;

    build_spatial_hash_bins(&mut data, &disk_params);
    process_particle_collisions_and_sticking(&mut data, &disk_params, speed_mult);

    let current_ecs_count = massive_bodies.len();
    let is_massive_disk = star_mass.0 > 10.0;
    let (promotions, active_count) =
        check_clump_promotions(&mut data, &disk_params, current_ecs_count, is_massive_disk);
    spawn_promoted_bodies(&mut commands, promotions, &disk_params, is_massive_disk);

    replenish_cleared_particles(
        &mut data,
        &disk_params,
        &mut config,
        ignition.is_ignited,
        speed_mult,
        active_count,
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
