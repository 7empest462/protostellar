//! Track 1 Visual Spectacle Pass Regression Tests:
//! - 1A: 3D volumetric Kilonova & BBH GW ringdown shells + fullscreen GW post-FX
//! - 1B: 3D volumetric Tidal Disruption Event (TDE) Frenet-Serret plasma ribbon + shock core + light
//! - 1C: 3D Dyson Swarm metallic-gold collector lattice rings & Giant Impact molten plume/torus visuals
//! - 1D: Gas disk close-up planet contrast fade & near-camera softness (`gas_cloud.wesl`)

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::rendering::bodies::VisualAssets;
use protostellar::rendering::camera::PanOrbitCamera;
use protostellar::rendering::effects::impact_debris::{
    ImpactFlashLight, ImpactPlumeVisual, sync_impact_plume_visuals,
};
use protostellar::rendering::effects::kilonova::{
    KilonovaLight, KilonovaPool, KilonovaVisualLayer, sync_kilonova_visuals,
    update_kilonova_effects,
};
use protostellar::rendering::effects::megastructures::{
    DysonRingVisual, DysonSwarm, MAX_DYSON_VISUAL_RINGS, generate_dyson_collector_ring_mesh,
    sync_dyson_swarm_visuals,
};
use protostellar::rendering::effects::roche_stream::RocheStreamMaterial;
use protostellar::rendering::effects::tde::{
    TdeFlareLight, TdeShockCore, TdeStreamVisual, build_tde_spiral_tube_mesh, sync_tde_visuals,
    update_tidal_disruption_streams,
};
use protostellar::rendering::gas_clouds::{
    GasCloudMaterial, GasUniforms, compute_gas_closeup_contrast_fade, setup_gas_cloud_disk,
    update_gas_cloud_material,
};
use protostellar::rendering::materials::SupernovaMaterial;
use protostellar::rendering::post_processing::{
    PostProcessingSettings, update_post_processing_effects,
};
use protostellar::simulation::accretion::TidalDisruptionEvent;
use protostellar::simulation::components::{BodyType, SimPosition};
use protostellar::simulation::relativity::GravitationalWaveMergerEvent;
use protostellar::simulation::resources::{
    DiskParameters, ImpactEjectaPool, ImpactEjectaRing, SimTime, SimulationConfig,
    TidalDisruptionPool,
};

#[test]
fn test_track1a_kilonova_and_bbh_volumetric_shells_and_post_fx() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<GravitationalWaveMergerEvent>()
        .init_resource::<KilonovaPool>()
        .insert_resource(VisualAssets::dummy(Handle::default()))
        .init_resource::<Assets<SupernovaMaterial>>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(120),
        ))
        .add_systems(
            Update,
            (
                update_kilonova_effects,
                sync_kilonova_visuals.after(update_kilonova_effects),
                update_post_processing_effects.after(update_kilonova_effects),
            ),
        );

    let cam = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            GlobalTransform::from(
                Transform::from_xyz(0.0, 20.0, 80.0).looking_at(Vec3::ZERO, Vec3::Y),
            ),
            PostProcessingSettings::default(),
            Tonemapping::AgX,
        ))
        .id();

    let ns1 = app.world_mut().spawn_empty().id();
    let ns2 = app.world_mut().spawn_empty().id();
    app.world_mut().write_message(GravitationalWaveMergerEvent {
        primary_entity: ns1,
        companion_entity: ns2,
        merged_entity: ns1,
        position: DVec3::ZERO,
        peak_gw_frequency_hz: 1200.0,
        radiated_gw_mass_solar: 0.08,
        remnant_type: BodyType::BlackHole,
        remnant_mass_solar: 2.7,
    });

    app.update();
    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&KilonovaVisualLayer>()
            .iter(app.world())
            .count(),
        3,
        "NS merger kilonova must spawn 3 volumetric HDR shells (fireball, equatorial torus, GRB jets)"
    );
    assert_eq!(
        app.world_mut()
            .query::<&KilonovaLight>()
            .iter(app.world())
            .count(),
        1
    );
    let pp = app
        .world()
        .entity(cam)
        .get::<PostProcessingSettings>()
        .unwrap();
    assert!(pp.shockwave_amplitude > 0.2);
    assert!(pp.hazard_flash_intensity > 0.1);

    // Clear and trigger a pure Binary Black Hole merger (2 volumetric GW shells, no gamma flash)
    app.world_mut().resource_mut::<KilonovaPool>().clear();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&KilonovaVisualLayer>()
            .iter(app.world())
            .count(),
        0
    );

    let bh1 = app.world_mut().spawn_empty().id();
    let bh2 = app.world_mut().spawn_empty().id();
    app.world_mut().write_message(GravitationalWaveMergerEvent {
        primary_entity: bh1,
        companion_entity: bh2,
        merged_entity: bh1,
        position: DVec3::ZERO,
        peak_gw_frequency_hz: 240.0,
        radiated_gw_mass_solar: 3.1,
        remnant_type: BodyType::BlackHole,
        remnant_mass_solar: 62.0,
    });
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&KilonovaVisualLayer>()
            .iter(app.world())
            .count(),
        2,
        "BBH merger must spawn 2 volumetric GW ringdown shells (QNM core + strain disk)"
    );
}

#[test]
fn test_track1b_tde_volumetric_spiral_tube_and_shock_core() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<TidalDisruptionEvent>()
        .init_resource::<TidalDisruptionPool>()
        .insert_resource(VisualAssets::dummy(Handle::default()))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<RocheStreamMaterial>>()
        .init_resource::<Assets<StandardMaterial>>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ))
        .add_systems(
            Update,
            (
                update_tidal_disruption_streams,
                sync_tde_visuals.after(update_tidal_disruption_streams),
            ),
        );

    let bh = app.world_mut().spawn(SimPosition(DVec3::ZERO)).id();
    let victim = app.world_mut().spawn_empty().id();
    app.world_mut().write_message(TidalDisruptionEvent {
        bh_entity: bh,
        disrupted_entity: victim,
        bh_pos: Vec3::ZERO,
        disruption_pos: Vec3::new(0.8, 0.0, 0.0),
        bh_mass_solar: 4_000_000.0,
        body_mass_solar: 1.0,
        tidal_radius_au: 0.8,
        isco_radius_au: 0.12,
        initial_velocity: Vec3::new(0.0, 0.0, 5.0),
        bh_name: "Sagittarius A*".to_string(),
        disrupted_name: "Victim Star".to_string(),
    });

    app.update();
    app.update();

    let stream = app
        .world()
        .resource::<TidalDisruptionPool>()
        .streams
        .first()
        .expect("TDE stream must be active");
    let sample_mesh = build_tde_spiral_tube_mesh(stream);
    assert!(sample_mesh.count_vertices() >= 700);
    assert!(sample_mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
    assert!(sample_mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());

    assert_eq!(
        app.world_mut()
            .query::<&TdeStreamVisual>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&TdeShockCore>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&TdeFlareLight>()
            .iter(app.world())
            .count(),
        1
    );

    // Clearing pool must despawn all TDE visual entities cleanly
    app.world_mut()
        .resource_mut::<TidalDisruptionPool>()
        .streams
        .clear();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&TdeStreamVisual>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&TdeShockCore>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&TdeFlareLight>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn test_track1c_dyson_swarm_3d_rings_and_giant_impact_plumes() {
    let collector_mesh = generate_dyson_collector_ring_mesh();
    assert!(collector_mesh.count_vertices() >= 300);
    assert!(collector_mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimTime>()
        .init_resource::<ImpactEjectaPool>()
        .insert_resource(VisualAssets::dummy(Handle::default()))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<protostellar::rendering::effects::MegastructureMaterial>>()
        .init_resource::<Assets<SupernovaMaterial>>()
        .add_systems(
            Update,
            (sync_dyson_swarm_visuals, sync_impact_plume_visuals),
        );

    let star = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::ZERO),
            DysonSwarm {
                completion_fraction: 0.5,
                mirror_count: 50_000,
            },
        ))
        .id();

    let planet = app
        .world_mut()
        .spawn(Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)))
        .id();
    app.world_mut()
        .resource_mut::<ImpactEjectaPool>()
        .rings
        .push(ImpactEjectaRing {
            primary_entity: planet,
            primary_pos: Vec3::new(1.0, 0.0, 0.0),
            impact_pos: Vec3::new(1.01, 0.0, 0.0),
            ring_mass: 1e-7,
            color: Color::srgb(1.0, 0.55, 0.2),
            fragments: vec![(0.04, 0.0, 1.0, 0.0), (0.06, 1.5, 1.0, 0.0)],
            timer: 0.2,
            max_timer: 6.0,
        });

    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&DysonRingVisual>()
            .iter(app.world())
            .count(),
        MAX_DYSON_VISUAL_RINGS / 2,
        "50% Dyson Swarm completion must render half of MAX_DYSON_VISUAL_RINGS"
    );
    assert_eq!(
        app.world_mut()
            .query::<&ImpactPlumeVisual>()
            .iter(app.world())
            .count(),
        2,
        "Giant impact ring must spawn molten plume shell + coalescing protolunar torus"
    );
    assert_eq!(
        app.world_mut()
            .query::<&ImpactFlashLight>()
            .iter(app.world())
            .count(),
        1
    );

    // Completing Dyson Swarm scales up to all MAX_DYSON_VISUAL_RINGS; clearing impact ring despawns plumes
    app.world_mut()
        .entity_mut(star)
        .get_mut::<DysonSwarm>()
        .unwrap()
        .completion_fraction = 1.0;
    app.world_mut()
        .resource_mut::<ImpactEjectaPool>()
        .rings
        .clear();
    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&DysonRingVisual>()
            .iter(app.world())
            .count(),
        MAX_DYSON_VISUAL_RINGS
    );
    assert_eq!(
        app.world_mut()
            .query::<&ImpactPlumeVisual>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&ImpactFlashLight>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn test_track1d_gas_disk_closeup_contrast_fade_and_shader_validation() {
    assert_eq!(std::mem::size_of::<GasUniforms>() % 16, 0);

    // Tight close-up on Saturn (10.15 AU inside 45 AU disk) must strongly attenuate background glare
    let close_fade = compute_gas_closeup_contrast_fade(Some(0.15), Some(10.15), 45.0);
    assert!(
        close_fade < 0.25,
        "Close-up planet zoom must attenuate gas disk backdrop glare (< 0.25), got {close_fade}"
    );
    // Wide system orbital view must preserve full 1.0 nebula luminance
    let wide_fade = compute_gas_closeup_contrast_fade(Some(25.0), Some(0.0), 45.0);
    assert!((wide_fade - 1.0).abs() < 1e-5);

    // End-to-end ECS check: PanOrbitCamera zoom drives GasUniforms.camera_params.w
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimTime>()
        .init_resource::<SimulationConfig>()
        .init_resource::<DiskParameters>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<GasCloudMaterial>>()
        .add_systems(Startup, setup_gas_cloud_disk)
        .add_systems(Update, update_gas_cloud_material);

    app.world_mut().spawn((
        Camera3d::default(),
        PanOrbitCamera {
            focus: Vec3::new(10.15, 0.0, 0.0),
            radius: 0.15,
            target_radius: 0.15,
            ..default()
        },
        GlobalTransform::from_translation(Vec3::new(10.15, 0.04, 0.15)),
    ));
    app.update();

    let mats = app.world().resource::<Assets<GasCloudMaterial>>();
    let mat = mats.iter().next().expect("GasCloudMaterial must exist").1;
    assert!(
        mat.extension.uniforms.camera_params.w < 0.25,
        "GasUniforms.camera_params.w must reflect close-up planet contrast fade"
    );

    super::shader_guardrails::compile_and_validate_wesl_shader(
        "assets/shaders/gas_cloud.wesl",
        &[],
    );
}

#[test]
fn test_track1_all_visual_sync_systems_initialize_without_query_conflicts() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_systems(
        Update,
        (
            sync_kilonova_visuals,
            sync_tde_visuals,
            sync_dyson_swarm_visuals,
            sync_impact_plume_visuals,
            update_gas_cloud_material,
            update_post_processing_effects,
        ),
    );
    app.world_mut()
        .try_schedule_scope(Update, |world, schedule| {
            schedule
                .initialize(world)
                .expect("All Track 1 visual sync systems must initialize without B0001 conflicts");
        })
        .expect("Update schedule must exist");
}
