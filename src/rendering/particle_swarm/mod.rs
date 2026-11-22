//! High-performance real-time particle swarm renderer for glowing protoplanetary particles.

pub mod billboards;
pub mod material;
pub mod promotions;
pub mod setup;
pub mod simulation;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

pub use billboards::{
    sync_mesh_attributes, update_billboard_mesh_quads, update_gpu_billboard_mesh,
    update_gpu_billboard_mesh_selective, update_gpu_billboard_mesh_with_normals,
};
pub use material::{ParticleSwarmMaterial, ParticleSwarmUniforms};
pub use promotions::{
    check_clump_promotions, determine_promoted_body_type_and_name, replenish_cleared_particles,
    spawn_promoted_bodies, spawn_promoted_body,
};
pub use setup::{create_soft_particle_texture, reseed_particle_swarm, setup_particle_swarm};
pub use simulation::{
    ParticleIntegrationParams, apply_particle_accretion_to_bodies, build_spatial_hash_bins,
    compute_gas_aerodynamic_drift, integrate_particles_and_collect_accretions,
    process_particle_collisions_and_sticking,
};

/// Marker component for the 50,000 particle visual swarm mesh.
#[derive(Component)]
pub struct ParticleSwarmMesh;

/// Zero-vertex placeholder mesh swapped onto [`ParticleSwarmMesh`] while GPU Vertex Pulling
/// renders all 500k particles directly from VRAM, eliminating redundant legacy mesh draws
/// without mutating scenario-controlled [`Visibility`].
#[derive(Resource, Debug, Clone)]
pub struct ParticleSwarmEmptyMesh(pub Handle<Mesh>);

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

fn sync_swarm_mesh_handle_for_gpu_pulling(
    gpu_active: bool,
    data: &mut ParticleSwarmData,
    empty_mesh: Option<&ParticleSwarmEmptyMesh>,
    mesh_query: &mut Query<(&mut Mesh3d, &mut Transform), With<ParticleSwarmMesh>>,
) {
    let target = if gpu_active && let Some(empty) = empty_mesh {
        &empty.0
    } else {
        &data.mesh_handle
    };
    for (mut mesh3d, _) in mesh_query.iter_mut() {
        if mesh3d.0 != *target {
            mesh3d.0 = target.clone();
            if !gpu_active {
                data.is_dirty = true;
            }
        }
    }
}

fn sync_swarm_material_render_scale(
    render_scale: f32,
    opt_materials: &mut Option<ResMut<Assets<ParticleSwarmMaterial>>>,
    mat_query: &Query<&MeshMaterial3d<ParticleSwarmMaterial>, With<ParticleSwarmMesh>>,
) {
    let Some(materials) = opt_materials.as_deref_mut() else {
        return;
    };
    for handle in mat_query.iter() {
        if let Some(mat) = materials.get(&handle.0)
            && (mat.uniforms.params.x - render_scale).abs() <= 1e-5
        {
            continue;
        }
        if let Some(mut mat) = materials.get_mut(&handle.0) {
            mat.uniforms.params.x = render_scale;
        }
    }
}

fn flush_dirty_mesh_if_paused(
    swarm: &mut Option<ResMut<ParticleSwarmData>>,
    disk_params: &DiskParameters,
    meshes: &mut Assets<Mesh>,
) {
    if let Some(data) = swarm
        && data.is_dirty
    {
        update_gpu_billboard_mesh(data, disk_params, meshes);
        data.is_dirty = false;
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
            Option<&mut VolatileInventory>,
        ),
        Without<CentralStar>,
    >,
    massive_bodies: &[(Entity, DVec3, f64, BodyType)],
    disk_params: &DiskParameters,
    config: &mut SimulationConfig,
    speed_mult: f32,
    elapsed_years: f64,
) -> bool {
    let accreted_events = if params.gpu_active {
        Vec::new()
    } else {
        integrate_particles_and_collect_accretions(data, params)
    };
    let had_accretions = !accreted_events.is_empty() || !data.pending_gpu_accretions.is_empty();
    let mut pending_accretions = std::mem::take(&mut data.pending_gpu_accretions);
    apply_particle_accretion_to_bodies(
        massive_query,
        massive_bodies,
        accreted_events,
        &mut pending_accretions,
        f64::from(params.star_m),
        params.gas_scale,
        disk_params,
    );
    data.pending_gpu_accretions = pending_accretions;

    // When GPU Vertex Pulling is active, particle positions stay purely in VRAM (no 24 MB readback
    // and no 500k CPU loop); only compact GPU accretion events are applied on CPU.
    if params.gpu_active {
        return had_accretions;
    }

    if elapsed_years >= 15.0 {
        build_spatial_hash_bins(data, disk_params);
        process_particle_collisions_and_sticking(data, disk_params, speed_mult);
    }

    let prev_active = config.active_particles;
    let current_ecs_count = massive_bodies.len();
    let is_massive_disk = params.star_m > 10.0;
    let (promotions, active_count) = check_clump_promotions(
        data,
        disk_params,
        current_ecs_count,
        is_massive_disk,
        elapsed_years,
    );
    let had_promotions = !promotions.is_empty();
    spawn_promoted_bodies(commands, promotions, disk_params, is_massive_disk);

    if had_accretions || had_promotions {
        data.is_dirty = true;
    }

    replenish_cleared_particles(
        data,
        disk_params,
        config,
        params.star_is_ignited,
        speed_mult,
        active_count,
    );

    had_accretions || had_promotions || config.active_particles != prev_active
}

fn collect_sorted_massive_bodies(
    massive_query: &Query<
        (
            Entity,
            &SimPosition,
            &mut Mass,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
            Option<&mut VolatileInventory>,
        ),
        Without<CentralStar>,
    >,
) -> Vec<(Entity, DVec3, f64, BodyType)> {
    let mut massive_bodies: Vec<(Entity, DVec3, f64, BodyType)> = massive_query
        .iter()
        .map(|(e, p, m, _, _, body, _)| (e, p.0, m.0, body.body_type))
        .collect();
    massive_bodies.sort_by(|a, b| b.2.total_cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    massive_bodies
}

fn build_particle_integration_params<'a>(
    star: (
        &SimPosition,
        &Mass,
        &IgnitionState,
        Option<&BlackHoleStarState>,
        &CelestialBody,
    ),
    sim: (
        &SimulationConfig,
        &PlayerInteractionState,
        &TimeWarp,
        Option<&crate::game::phases::LateHeavyBombardmentState>,
    ),
    disk_params: &'a DiskParameters,
    massive_bodies: &'a [(Entity, DVec3, f64, BodyType)],
    (base_mass, gpu_active): (f32, bool),
) -> ParticleIntegrationParams<'a> {
    let (star_pos, star_mass, ignition, opt_bhs, star_body) = star;
    let (config, player_state, time_warp, lhb_state) = sim;
    let quasar_blown_out =
        opt_bhs.is_some_and(|s| s.is_blown_out) || star_body.body_type == BodyType::BlackHole;
    let speed_mult = time_warp.multiplier as f32;
    let (lhb_active, lhb_resonance) =
        lhb_state.map_or((false, false), |lhb| (lhb.is_active, lhb.resonance_crossed));
    ParticleIntegrationParams {
        star_pos_f32: [star_pos.x as f32, star_pos.y as f32, star_pos.z as f32],
        star_m: star_mass.0 as f32,
        g_const: G_ASTRO as f32,
        shockwave_r: ignition.shockwave_radius as f32,
        enable_gas_drag: config.enable_gas_drag,
        gas_scale: config.gas_density_scale,
        star_is_ignited: ignition.is_ignited,
        quasar_blown_out,
        tractor_pos_mass: compute_tractor_pos_mass(player_state),
        speed_mult,
        visual_flow_dt: compute_visual_flow_dt(speed_mult),
        gpu_active,
        lhb_active,
        lhb_resonance,
        disk_params,
        massive_bodies,
        base_mass,
    }
}

fn sync_swarm_render_origin(
    opt_origin: Option<&crate::rendering::origin::RenderOrigin>,
    swarm_mesh_query: &mut Query<(&mut Mesh3d, &mut Transform), With<ParticleSwarmMesh>>,
) {
    let target_t = -opt_origin.copied().unwrap_or_default().0.as_vec3();
    for (_mesh, mut tf) in swarm_mesh_query.iter_mut() {
        if tf.translation != target_t {
            tf.translation = target_t;
        }
    }
}

#[allow(clippy::too_many_arguments, reason = "Particle swarm ECS queries")]
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
            Option<&mut VolatileInventory>,
        ),
        Without<CentralStar>,
    >,
    lhb_state: Option<Res<crate::game::phases::LateHeavyBombardmentState>>,
    mut swarm: Option<ResMut<ParticleSwarmData>>,
    (empty_mesh, opt_origin): (
        Option<Res<ParticleSwarmEmptyMesh>>,
        Option<Res<crate::rendering::origin::RenderOrigin>>,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    mut opt_swarm_materials: Option<ResMut<Assets<ParticleSwarmMaterial>>>,
    swarm_mat_query: Query<&MeshMaterial3d<ParticleSwarmMaterial>, With<ParticleSwarmMesh>>,
    mut swarm_mesh_query: Query<(&mut Mesh3d, &mut Transform), With<ParticleSwarmMesh>>,
    mut last_size_state: Local<(usize, f32, f64, bool, usize, u32)>,
) {
    sync_swarm_render_origin(opt_origin.as_deref(), &mut swarm_mesh_query);

    sync_swarm_material_render_scale(
        config.particle_render_scale,
        &mut opt_swarm_materials,
        &swarm_mat_query,
    );

    let gpu_active = config.enable_gpu_compute && config.gpu_compute_active;
    if let Some(ref mut data) = swarm {
        sync_swarm_mesh_handle_for_gpu_pulling(
            gpu_active,
            data,
            empty_mesh.as_deref(),
            &mut swarm_mesh_query,
        );
    }

    if time_warp.is_paused && !time_warp.step_once {
        if !gpu_active {
            flush_dirty_mesh_if_paused(&mut swarm, &disk_params, &mut meshes);
        }
        return;
    }
    let Some(mut data) = swarm else {
        return;
    };

    let is_galaxy_collision = disk_params.outer_radius_au >= 10_000.0;
    if is_galaxy_collision {
        if gpu_active {
            data.is_dirty = false;
            return;
        }
        if data.is_dirty {
            update_gpu_billboard_mesh(&mut data, &disk_params, &mut meshes);
            last_size_state.4 = data.count;
            data.is_dirty = false;
        }
        return;
    }

    if gpu_active && data.pending_gpu_accretions.is_empty() {
        data.is_dirty = false;
        return;
    }

    let fallback = if star_query.is_empty() {
        Some(default_central_star_fallback(disk_params.central_star_mass))
    } else {
        None
    };
    let star_ref = if let Some(s) = star_query.iter().next() {
        s
    } else if let Some(ref d) = fallback {
        (&d.0, &d.1, &d.2, None, &d.3)
    } else {
        return;
    };

    let massive_bodies = collect_sorted_massive_bodies(&massive_query);
    let speed_mult = time_warp.multiplier as f32;
    let params = build_particle_integration_params(
        star_ref,
        (&config, &player_state, &time_warp, lhb_state.as_deref()),
        &disk_params,
        &massive_bodies,
        (data.base_mass, gpu_active),
    );

    let masses_changed = step_protoplanetary_disk_mechanics(
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

    if gpu_active {
        data.is_dirty = false;
        return;
    }

    sync_swarm_billboard_mesh_selective(
        &mut data,
        &disk_params,
        &mut meshes,
        &mut last_size_state,
        masses_changed,
        gpu_active,
        sim_time.elapsed_years,
    );
}

fn default_central_star_fallback(
    central_star_mass: f64,
) -> (SimPosition, Mass, IgnitionState, CelestialBody) {
    (
        SimPosition(DVec3::ZERO),
        Mass(central_star_mass),
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
    )
}

fn sync_swarm_billboard_mesh_selective(
    data: &mut ParticleSwarmData,
    disk_params: &DiskParameters,
    meshes: &mut Assets<Mesh>,
    last_size_state: &mut (usize, f32, f64, bool, usize, u32),
    masses_changed: bool,
    gpu_active: bool,
    elapsed_years: f64,
) {
    let size_state = (
        data.count,
        data.base_mass,
        disk_params.outer_radius_au,
        gpu_active,
    );
    let size_params_changed = (
        last_size_state.0,
        last_size_state.1,
        last_size_state.2,
        last_size_state.3,
    ) != size_state;
    let count_changed = last_size_state.4 != data.count;

    // In GPU compute mode, avoid rebuilding and re-uploading the 500k mesh on frames
    // where neither GPU readback nor CPU mechanics dirtied particle state.
    if !gpu_active || data.is_dirty {
        let update_normals =
            masses_changed || size_params_changed || count_changed || elapsed_years < 0.05;
        let update_colors = update_normals || last_size_state.5.is_multiple_of(4);
        let full_buffer_sync = last_size_state.4 == 0 || count_changed || size_params_changed;
        update_gpu_billboard_mesh_selective(
            data,
            disk_params,
            meshes,
            update_colors,
            update_normals,
            full_buffer_sync,
        );
        if update_normals {
            last_size_state.0 = size_state.0;
            last_size_state.1 = size_state.1;
            last_size_state.2 = size_state.2;
            last_size_state.3 = size_state.3;
        }
        last_size_state.4 = data.count;
        last_size_state.5 = last_size_state.5.wrapping_add(1);
        data.is_dirty = false;
    }
}

/// Plugin registering the 50,000 particle visual swarm.
pub struct ParticleSwarmPlugin;

impl Plugin for ParticleSwarmPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_particle_swarm)
            .add_systems(Update, update_particle_swarm);
    }
}
