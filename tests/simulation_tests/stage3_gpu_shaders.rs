//! Stage 3 GPU & Shader Architecture Upgrade Regression Tests:
//! - 3.1: Planet cloud-deck fragment shader fast path (`planet.wesl`)
//! - 3.2: Gas cloud distance LOD & zero-density culling (`gas_clouds.rs`)
//! - 3.3: Live 96^3 GMC 3D volume binding & GPU-to-GPU pack (`volumetric_nebula.rs`, `gmc_volume_pack.wgsl`)
//! - 3.4: 500k Particle Swarm GPU Vertex Pulling & compact 144-byte accretion readback

use bevy::prelude::*;
use bevy::render::render_resource::{TextureDimension, TextureFormat, TextureUsages};

use protostellar::gpu::GpuReadbackSender;
use protostellar::gpu::buffers::{
    GPU_ACCRETION_SUMMARY_SLOTS, GPU_COMPACT_READBACK_MAGIC, GpuCompactReadback,
    GpuParticlePullUniforms,
};
use protostellar::gpu::compute_node::{GpuReadbackReceiver, receive_gpu_readback};
use protostellar::gpu::gmc_fluid::GMC_GRID_DIM;
use protostellar::gpu::particle_pipeline::compute_gpu_pull_base_radius;
use protostellar::rendering::gas_clouds::{
    GAS_CLOUD_CULL_DENSITY_THRESHOLD, GasCloudDisk, GasCloudLayerLod, GasCloudMaterial,
    compute_gas_cloud_max_lod_tier, setup_gas_cloud_disk, update_gas_cloud_material,
};
use protostellar::rendering::materials::VolumetricNebulaMaterial;
use protostellar::rendering::particle_swarm::{
    ParticleSwarmData, ParticleSwarmEmptyMesh, ParticleSwarmMaterial, ParticleSwarmMesh,
    setup_particle_swarm, update_particle_swarm,
};
use protostellar::rendering::volumetric_nebula::{
    GmcLiveVolumeHandle, VolumetricNebulaVolume, create_gmc_live_volume_image,
    sync_volumetric_nebula,
};
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{
    ActiveScenarioState, ScenarioPreset, ScenarioReloadGeneration,
};

#[test]
fn test_stage3_1_planet_cloud_deck_fast_path_guards_and_shader_compile() {
    let source = std::fs::read_to_string("assets/shaders/planet.wesl")
        .expect("assets/shaders/planet.wesl must exist");
    assert!(
        source.contains("if (total_alpha >= 0.015 && ndotl >= -0.15)"),
        "planet.wesl must guard the 4 cloud bump / self-shadow noise fetches on clear sky and deep nightside"
    );
    assert!(
        source.contains("let bump_gate = smoothstep(0.015, 0.06, total_alpha)"),
        "planet.wesl cloud fast path must smoothly blend across the threshold to prevent terminator seams"
    );
    super::shader_guardrails::compile_and_validate_wesl_shader("assets/shaders/planet.wesl", &[]);
}

#[test]
fn test_stage3_2_gas_cloud_distance_lod_and_zero_density_culling() {
    // Pure LOD tier unit checks
    assert_eq!(
        compute_gas_cloud_max_lod_tier(0.0, 45.0, Some(Vec3::new(0.0, 5.0, 15.0))),
        None
    );
    assert_eq!(
        compute_gas_cloud_max_lod_tier(
            GAS_CLOUD_CULL_DENSITY_THRESHOLD * 0.5,
            45.0,
            Some(Vec3::new(0.0, 5.0, 15.0)),
        ),
        None
    );
    assert_eq!(
        compute_gas_cloud_max_lod_tier(1.0, 45.0, Some(Vec3::new(0.0, 8.0, 30.0))),
        Some(2)
    );
    assert_eq!(
        compute_gas_cloud_max_lod_tier(1.0, 45.0, Some(Vec3::new(0.0, 20.0, 80.0))),
        Some(1)
    );
    assert_eq!(
        compute_gas_cloud_max_lod_tier(1.0, 45.0, Some(Vec3::new(0.0, 40.0, 200.0))),
        Some(0)
    );
    assert_eq!(
        compute_gas_cloud_max_lod_tier(1.0, 45.0, Some(Vec3::new(0.0, 95.0, 10.0))),
        Some(0),
        "Steep overhead view past mid-range must collapse vertical strata to 1 core plane"
    );

    // End-to-end ECS check: 7 layers close-up -> 3 mid-range -> 1 far -> 0 culled
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimTime>()
        .init_resource::<SimulationConfig>()
        .init_resource::<DiskParameters>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<GasCloudMaterial>>()
        .add_systems(Startup, setup_gas_cloud_disk)
        .add_systems(Update, update_gas_cloud_material);

    let cam = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(Vec3::new(0.0, 6.0, 25.0)),
        ))
        .id();

    let count_visible = |world: &mut World| -> usize {
        world
            .query_filtered::<&Visibility, With<GasCloudDisk>>()
            .iter(world)
            .filter(|&&v| v != Visibility::Hidden)
            .count()
    };

    // Close-up: all 7 strata visible
    app.update();
    assert_eq!(count_visible(app.world_mut()), 7);
    assert_eq!(
        app.world_mut()
            .query::<&GasCloudLayerLod>()
            .iter(app.world())
            .count(),
        7
    );

    // Mid-range: 3 layers (core + inner pair)
    *app.world_mut()
        .entity_mut(cam)
        .get_mut::<GlobalTransform>()
        .unwrap() = GlobalTransform::from_translation(Vec3::new(0.0, 20.0, 85.0));
    app.update();
    assert_eq!(count_visible(app.world_mut()), 3);

    // Far view: 1 core midplane layer
    *app.world_mut()
        .entity_mut(cam)
        .get_mut::<GlobalTransform>()
        .unwrap() = GlobalTransform::from_translation(Vec3::new(0.0, 40.0, 220.0));
    app.update();
    assert_eq!(count_visible(app.world_mut()), 1);

    // Zero-density / dissipated disk: 0 layers visible
    app.world_mut()
        .resource_mut::<SimulationConfig>()
        .gas_density_scale = 0.002;
    app.update();
    assert_eq!(count_visible(app.world_mut()), 0);
}

#[test]
fn test_stage3_3_gmc_live_96_cubed_volume_image_and_material_binding() {
    let img = create_gmc_live_volume_image();
    assert_eq!(img.texture_descriptor.size.width, GMC_GRID_DIM);
    assert_eq!(img.texture_descriptor.size.height, GMC_GRID_DIM);
    assert_eq!(
        img.texture_descriptor.size.depth_or_array_layers,
        GMC_GRID_DIM
    );
    assert_eq!(img.texture_descriptor.dimension, TextureDimension::D3);
    assert_eq!(img.texture_descriptor.format, TextureFormat::Rgba16Float);
    assert!(
        img.texture_descriptor
            .usage
            .contains(TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING),
        "Live GMC volume texture must support both STORAGE_BINDING (compute write) and TEXTURE_BINDING (raymarch sample)"
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::MolecularCloudCluster,
            ..default()
        })
        .init_resource::<SimTime>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<VolumetricNebulaMaterial>>()
        .add_systems(Update, sync_volumetric_nebula);

    app.update();

    let vol_handle = app
        .world()
        .get_resource::<GmcLiveVolumeHandle>()
        .expect("GMC scenario must create persistent GmcLiveVolumeHandle")
        .0
        .clone();

    let mat_handle = app
        .world_mut()
        .query_filtered::<&MeshMaterial3d<VolumetricNebulaMaterial>, With<VolumetricNebulaVolume>>()
        .single(app.world())
        .expect("VolumetricNebulaVolume must spawn in GMC")
        .0
        .clone();

    let mat = app
        .world()
        .resource::<Assets<VolumetricNebulaMaterial>>()
        .get(&mat_handle)
        .expect("VolumetricNebulaMaterial asset must exist");
    assert_eq!(
        mat.density_volume.as_ref(),
        Some(&vol_handle),
        "VolumetricNebulaMaterial must bind the persistent 96^3 GmcLiveVolumeHandle"
    );

    // Leaving GMC tears down the cuboid while preserving the persistent 3D image handle
    app.world_mut()
        .resource_mut::<ActiveScenarioState>()
        .current_preset = ScenarioPreset::default();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&VolumetricNebulaVolume>()
            .iter(app.world())
            .count(),
        0
    );
    assert!(app.world().get_resource::<GmcLiveVolumeHandle>().is_some());

    super::shader_guardrails::compile_and_validate_wesl_shader(
        "assets/shaders/volumetric_nebula.wesl",
        &[],
    );
}

#[test]
fn test_stage3_4_gpu_vertex_pulling_compact_readback_and_mesh_swap() {
    // 1. Validate 16-byte WGSL uniform/packet layouts
    assert_eq!(std::mem::size_of::<GpuCompactReadback>(), 144);
    assert_eq!(std::mem::align_of::<GpuCompactReadback>(), 16);
    assert_eq!(std::mem::size_of::<GpuParticlePullUniforms>(), 128);
    assert_eq!(std::mem::align_of::<GpuParticlePullUniforms>(), 16);
    assert_eq!(GPU_ACCRETION_SUMMARY_SLOTS, 33);

    // 2. Validate sizing parity with billboards.rs
    let r_disk = compute_gpu_pull_base_radius(false, 500_000, 45.0);
    assert!((r_disk - 0.080).abs() < 1e-5);
    let r_gal = compute_gpu_pull_base_radius(true, 500_000, 45_000.0);
    assert!((r_gal - 160.0).abs() < 1e-3);

    // 3. Validate compact 144-byte readback accumulation, generation filtering, and empty-mesh swap
    let (tx, rx) = flume::bounded::<Vec<u8>>(4);
    let (recycle_tx, recycle_rx) = flume::bounded::<Vec<u8>>(4);
    let sender = GpuReadbackSender { tx, recycle_rx };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimTime>()
        .init_resource::<TimeWarp>()
        .insert_resource(SimulationConfig {
            target_particle_count: 50_000,
            enable_gpu_compute: true,
            gpu_compute_active: false,
            ..default()
        })
        .init_resource::<DiskParameters>()
        .init_resource::<PlayerInteractionState>()
        .insert_resource(ScenarioReloadGeneration(3))
        .insert_resource(GpuReadbackReceiver { rx, recycle_tx })
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<ParticleSwarmMaterial>>()
        .add_systems(Startup, setup_particle_swarm)
        .add_systems(
            Update,
            (
                receive_gpu_readback,
                update_particle_swarm.after(receive_gpu_readback),
            ),
        );

    // First update syncs Local generation counter (`last_gen` = 3)
    app.update();

    let orig_mesh = app
        .world()
        .resource::<ParticleSwarmData>()
        .mesh_handle
        .clone();
    let empty_mesh = app.world().resource::<ParticleSwarmEmptyMesh>().0.clone();
    let base_mass = app.world().resource::<ParticleSwarmData>().base_mass;
    assert_ne!(orig_mesh, empty_mesh);

    // Send 2 consecutive compact packets + 1 stale-generation packet (which must be ignored)
    let mut pkt_stale = GpuCompactReadback {
        magic: GPU_COMPACT_READBACK_MAGIC,
        active_particles: 999,
        is_galaxy: 0,
        generation_lo: 2,
        accreted_per_body: [0; 32],
    };
    pkt_stale.accreted_per_body[0] = 500;
    sender
        .tx
        .try_send(bytemuck::bytes_of(&pkt_stale).to_vec())
        .unwrap();

    let mut pkt_1 = GpuCompactReadback {
        magic: GPU_COMPACT_READBACK_MAGIC,
        active_particles: 49_990,
        is_galaxy: 0,
        generation_lo: 3,
        accreted_per_body: [0; 32],
    };
    pkt_1.accreted_per_body[0] = 6;
    pkt_1.accreted_per_body[2] = 4;
    sender
        .tx
        .try_send(bytemuck::bytes_of(&pkt_1).to_vec())
        .unwrap();

    let mut pkt_2 = GpuCompactReadback {
        magic: GPU_COMPACT_READBACK_MAGIC,
        active_particles: 49_985,
        is_galaxy: 0,
        generation_lo: 3,
        accreted_per_body: [0; 32],
    };
    pkt_2.accreted_per_body[0] = 5;
    sender
        .tx
        .try_send(bytemuck::bytes_of(&pkt_2).to_vec())
        .unwrap();

    // Pause simulation so pending_gpu_accretions aren't drained yet during assertion
    app.world_mut().resource_mut::<TimeWarp>().is_paused = true;
    app.update();

    let cfg = app.world().resource::<SimulationConfig>();
    assert!(
        cfg.gpu_compute_active,
        "Compact readback must activate GPU vertex pulling"
    );
    assert_eq!(cfg.active_particles, 49_985);

    let swarm = app.world().resource::<ParticleSwarmData>();
    let body0_mass: f32 = swarm
        .pending_gpu_accretions
        .iter()
        .filter(|(idx, _)| *idx == 0)
        .map(|(_, m)| *m)
        .sum();
    let body2_mass: f32 = swarm
        .pending_gpu_accretions
        .iter()
        .filter(|(idx, _)| *idx == 2)
        .map(|(_, m)| *m)
        .sum();
    assert!(
        (body0_mass - 11.0 * base_mass).abs() < 1e-8,
        "Body 0 must sum both fresh packets (6 + 5 = 11 particles), ignoring stale packet"
    );
    assert!((body2_mass - 4.0 * base_mass).abs() < 1e-8);

    // ParticleSwarmMesh must swap to empty_mesh_handle while preserving Visibility::Inherited
    let (mesh3d, vis) = app
        .world_mut()
        .query_filtered::<(&Mesh3d, &Visibility), With<ParticleSwarmMesh>>()
        .single(app.world())
        .unwrap();
    assert_eq!(mesh3d.0, empty_mesh);
    assert_eq!(*vis, Visibility::Inherited);

    // Disabling GPU compute ([G] toggle) must immediately restore the CPU billboard mesh
    app.world_mut()
        .resource_mut::<SimulationConfig>()
        .enable_gpu_compute = false;
    app.update();
    let mesh3d_after = app
        .world_mut()
        .query_filtered::<&Mesh3d, With<ParticleSwarmMesh>>()
        .single(app.world())
        .unwrap();
    assert_eq!(mesh3d_after.0, orig_mesh);
}

#[test]
fn test_stage3_4_empty_swarm_placeholder_mesh_and_shader_guard() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<DiskParameters>()
        .init_resource::<SimulationConfig>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<ParticleSwarmMaterial>>()
        .add_systems(Startup, setup_particle_swarm);
    app.update();

    let empty_handle = app.world().resource::<ParticleSwarmEmptyMesh>().0.clone();
    let empty_asset = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&empty_handle)
        .expect("ParticleSwarmEmptyMesh asset must exist");
    assert!(
        empty_asset.count_vertices() >= 1,
        "ParticleSwarmEmptyMesh must have >= 1 degenerate vertex so Bevy's mesh allocator never warns"
    );
    assert!(empty_asset.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
    assert!(empty_asset.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
    assert!(empty_asset.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
    assert!(empty_asset.attribute(Mesh::ATTRIBUTE_COLOR).is_some());

    let src = std::fs::read_to_string("assets/shaders/particle_swarm.wesl")
        .expect("particle_swarm.wesl must exist");
    assert!(
        src.contains("@if(VERTEX_POSITIONS)"),
        "particle_swarm.wesl must guard vertex.position behind @if(VERTEX_POSITIONS)"
    );
    super::shader_guardrails::compile_and_validate_wesl_shader(
        "assets/shaders/particle_swarm.wesl",
        &[],
    );
}

#[test]
fn test_stage3_systems_initialize_without_query_conflicts() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_systems(
        Update,
        (
            update_gas_cloud_material,
            sync_volumetric_nebula,
            receive_gpu_readback,
            update_particle_swarm,
        ),
    );
    app.world_mut()
        .try_schedule_scope(Update, |world, schedule| {
            schedule
                .initialize(world)
                .expect("All Stage 3 Update systems must initialize without B0001 query conflicts");
        })
        .expect("Update schedule must exist");
}
