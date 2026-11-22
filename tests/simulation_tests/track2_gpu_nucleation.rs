//! Track 2 tests: GPU Mode Genesis Planetesimal Nucleation, dual snow-line / silicate trap
//! aerodynamics, WGSL Naga validation, compact 272-byte readback, and CPU/GPU coordination.

use bevy::prelude::*;
use protostellar::game::ui::types::{CosmicEventLog, NotificationToast};
use protostellar::gpu::buffers::{
    GPU_NUCLEATION_READBACK_MAGIC, GpuCompactReadback, GpuDiskNucleationUniforms,
    GpuNucleationEvent, GpuNucleationReadback, GpuOrbitUniforms, NUCLEATION_CELL_U32S,
    NUCLEATION_NUM_CELLS,
};
use protostellar::gpu::compute_node::{
    GpuReadbackReceiver, GpuSimExtractedParams, receive_gpu_readback,
};
use protostellar::gpu::disk_nucleation::{
    GpuNucleationBatch, GpuNucleationReceiver, build_nucleation_uniforms,
    receive_gpu_nucleation_events,
};
use protostellar::rendering::particle_swarm::{
    ParticleSwarmMaterial, setup_particle_swarm, update_particle_swarm,
};
use protostellar::simulation::components::*;
use protostellar::simulation::disk::planetesimals::{
    PlanetesimalSpawner, auto_spawn_planetesimals,
};
use protostellar::simulation::pebble_accretion::spawn_streaming_instability_minor_bodies;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{
    ActiveScenarioState, ScenarioPreset, ScenarioReloadGeneration,
};
use protostellar::utils::constants::*;

#[test]
fn test_track2_gpu_nucleation_buffer_layouts_and_shader_guards() {
    assert_eq!(std::mem::size_of::<GpuOrbitUniforms>(), 592);
    assert_eq!(std::mem::align_of::<GpuOrbitUniforms>(), 16);
    assert!((GpuOrbitUniforms::default().snow_line_au - 2.7).abs() < 1e-5);
    assert_eq!(std::mem::size_of::<GpuCompactReadback>(), 144);

    assert_eq!(std::mem::size_of::<GpuDiskNucleationUniforms>(), 592);
    assert_eq!(std::mem::align_of::<GpuDiskNucleationUniforms>(), 16);
    assert_eq!(std::mem::size_of::<GpuNucleationEvent>(), 64);
    assert_eq!(std::mem::align_of::<GpuNucleationEvent>(), 16);
    assert_eq!(std::mem::size_of::<GpuNucleationReadback>(), 272);
    assert_eq!(std::mem::align_of::<GpuNucleationReadback>(), 16);
    assert_eq!(NUCLEATION_NUM_CELLS, 4096);
    assert_eq!(NUCLEATION_CELL_U32S, 20);

    super::shader_guardrails::assert_shader_patterns(
        "assets/shaders/particle_orbit.wgsl",
        &[
            ("snow_line_au: f32,", "SimUniforms must expose snow_line_au"),
            (
                "let inner_trap_r = max(uniforms.inner_radius * 2.0, 0.35);",
                "particle_orbit must anchor inner silicate sublimation trap at 0.35 AU",
            ),
            (
                "let snow_trap = 1.0 - 1.85 * exp(-(dr_snow * dr_snow)",
                "particle_orbit must trap pebbles at snow_line_au",
            ),
        ],
        &[],
    );

    super::shader_guardrails::assert_shader_patterns(
        "assets/shaders/particle_nucleate.wgsl",
        &[
            (
                "fn clear_cells(",
                "particle_nucleate must define clear_cells",
            ),
            (
                "fn deposit_cells(",
                "particle_nucleate must define deposit_cells",
            ),
            (
                "fn evaluate_nucleation(",
                "particle_nucleate must define evaluate_nucleation",
            ),
            (
                "fn apply_cells_to_particles(",
                "particle_nucleate must define apply_cells_to_particles",
            ),
            (
                "fn finalize_clump_leaders(",
                "particle_nucleate must define finalize_clump_leaders",
            ),
        ],
        &[],
    );
}

#[test]
fn test_track2_build_nucleation_uniforms_cadence_and_guards() {
    let base_mass = (0.00035 / 500_000.0) as f32;
    let params = GpuSimExtractedParams {
        enable_gpu_compute: true,
        star_mass: 1.33,
        gas_scale: 1.0,
        inner_radius: 0.12,
        outer_radius: 55.0,
        snow_line_au: 3.65,
        disk_mass: 0.00035,
        elapsed_years: 10.0,
        total_ecs_bodies: 0,
        ..default()
    };

    // 1. Before t = 15 yr: coagulate is active past t >= 5 yr, but promotion stays locked (0 starter planets)
    let mut last_yr = 0.0;
    let u_early = build_nucleation_uniforms(&params, 500_000, base_mass, &mut last_yr, 1);
    assert_eq!(u_early.coagulate_enabled, 1);
    assert_eq!(u_early.enable_promotion, 0);
    assert_eq!(u_early.max_promotions, 0);
    assert_eq!(last_yr, 0.0);

    // 2. Past t = 15 yr: promotion unlocks for one burst and updates cooldown timestamp
    let mut p_ready = params.clone();
    p_ready.elapsed_years = 16.0;
    let u_ready = build_nucleation_uniforms(&p_ready, 500_000, base_mass, &mut last_yr, 2);
    assert_eq!(u_ready.enable_promotion, 1);
    assert_eq!(u_ready.max_promotions, 3);
    assert_eq!(u_ready.snow_line_au, 3.65);
    assert_eq!(last_yr, 16.0);

    // 3. Immediately next frame (1 yr later): cooldown prevents duplicate async burst
    p_ready.elapsed_years = 17.0;
    let u_cooldown = build_nucleation_uniforms(&p_ready, 500_000, base_mass, &mut last_yr, 3);
    assert_eq!(u_cooldown.enable_promotion, 0);
    assert_eq!(u_cooldown.max_promotions, 0);

    // 4. Quasar blowout or empty gas disables both coagulation and promotion
    let mut p_blown = p_ready.clone();
    p_blown.elapsed_years = 100.0;
    p_blown.quasar_blown_out = true;
    let u_blown = build_nucleation_uniforms(&p_blown, 500_000, base_mass, &mut last_yr, 4);
    assert_eq!(u_blown.coagulate_enabled, 0);
    assert_eq!(u_blown.enable_promotion, 0);

    // 5. Massive circum-nuclear disk raises mass floor to 0.25 M_J
    let mut p_massive = params;
    p_massive.star_mass = 1_000.0;
    p_massive.disk_mass = 500.0;
    p_massive.elapsed_years = 40.0;
    let mut massive_last = 0.0;
    let u_massive = build_nucleation_uniforms(&p_massive, 500_000, 0.001, &mut massive_last, 5);
    assert_eq!(u_massive.coagulate_enabled, 0);
    assert!(u_massive.min_clump_mass_solar >= (0.24 * JUPITER_MASS_SOLAR) as f32);
}

#[test]
fn test_track2_receive_gpu_nucleation_events_promotes_genesis_traps_and_excludes_overlap() {
    let (tx, rx) = flume::bounded::<GpuNucleationBatch>(4);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(SimTime {
            elapsed_years: 24.0,
            ..default()
        })
        .insert_resource(SimulationConfig {
            enable_gpu_compute: true,
            gpu_compute_active: true,
            active_particles: 500_000,
            ..default()
        })
        .insert_resource(DiskParameters {
            central_star_mass: 1.33,
            inner_radius_au: 0.12,
            outer_radius_au: 55.0,
            snow_line_au: 3.65,
            disk_mass: 0.00035,
            reference_temp_1au: 325.0,
            ..default()
        })
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::AccretionDiskGenesis,
            scenario_time_years: 24.0,
            ..default()
        })
        .insert_resource(ScenarioReloadGeneration(7))
        .init_resource::<PlanetesimalSpawner>()
        .init_resource::<NotificationToast>()
        .init_resource::<CosmicEventLog>()
        .insert_resource(GpuNucleationReceiver { rx })
        .add_systems(Update, receive_gpu_nucleation_events);

    // Send one stale-generation batch that MUST be discarded, plus one fresh Genesis batch
    tx.try_send(GpuNucleationBatch {
        generation: 6,
        events: vec![GpuNucleationEvent {
            pos_mass: [1.0, 0.0, 0.0, (0.05 * EARTH_MASS_SOLAR) as f32],
            vel_temp: [0.0, 0.0, std::f32::consts::TAU, 300.0],
            composition: [0.8, 0.0, 0.2, 0.0],
            meta: [100, 40, 0, GPU_NUCLEATION_READBACK_MAGIC],
        }],
    })
    .unwrap();

    tx.try_send(GpuNucleationBatch {
        generation: 7,
        events: vec![
            // Inner silicate trap planetesimal (~0.35 AU)
            GpuNucleationEvent {
                pos_mass: [0.35, 0.004, 0.0, (0.004 * EARTH_MASS_SOLAR) as f32],
                vel_temp: [0.0, 0.0, 12.2, 540.0],
                composition: [0.70, 0.0, 0.30, 0.0],
                meta: [704, 48, 1, 0],
            },
            // Overlapping duplicate inside 0.08 AU exclusion radius -> must be skipped
            GpuNucleationEvent {
                pos_mass: [0.36, 0.0, 0.01, (0.004 * EARTH_MASS_SOLAR) as f32],
                vel_temp: [0.0, 0.0, 12.0, 535.0],
                composition: [0.70, 0.0, 0.30, 0.0],
                meta: [705, 45, 1, 0],
            },
            // Snow-line trap planetesimal (~3.65 AU)
            GpuNucleationEvent {
                pos_mass: [0.0, -0.01, 3.65, (0.012 * EARTH_MASS_SOLAR) as f32],
                vel_temp: [-3.8, 0.0, 0.0, 170.0],
                composition: [0.25, 0.60, 0.05, 0.10],
                meta: [2240, 82, 2, 0],
            },
            // Embryo (~1.15 AU, 0.04 M_earth)
            GpuNucleationEvent {
                pos_mass: [-1.15, 0.0, 0.0, (0.04 * EARTH_MASS_SOLAR) as f32],
                vel_temp: [0.0, 0.0, -6.75, 300.0],
                composition: [0.68, 0.02, 0.30, 0.0],
                meta: [1420, 120, 0, 0],
            },
        ],
    })
    .unwrap();

    app.update();

    let mut q = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &Mass, &SimVelocity)>();
    let spawned: Vec<(String, BodyType, f64, f64, f64)> = q
        .iter(app.world())
        .map(|(b, p, m, v)| {
            (
                b.name.clone(),
                b.body_type,
                p.0.xz().length(),
                m.0,
                v.0.length(),
            )
        })
        .collect();

    assert_eq!(
        spawned.len(),
        3,
        "Stale batch + overlapping duplicate must be filtered leaving 3 nucleated bodies: {spawned:?}"
    );
    assert!(
        spawned.iter().any(|(n, t, r, _, _)| {
            *t == BodyType::Planetesimal && (*r - 0.35).abs() < 0.02 && n.contains("Rocky")
        }),
        "Must spawn inner 0.35 AU silicate trap planetesimal: {spawned:?}"
    );
    assert!(
        spawned.iter().any(|(n, t, r, _, _)| {
            *t == BodyType::Planetesimal && (*r - 3.65).abs() < 0.02 && n.contains("Snowline")
        }),
        "Must spawn 3.65 AU snowline planetesimal: {spawned:?}"
    );
    assert!(
        spawned
            .iter()
            .any(|(_, t, r, _, v)| *t == BodyType::Protoplanet
                && (*r - 1.15).abs() < 0.02
                && *v > 1.0),
        "Must spawn 1.15 AU planetary embryo: {spawned:?}"
    );

    assert_eq!(
        app.world().resource::<PlanetesimalSpawner>().total_spawned,
        3
    );
    assert!(
        !app.world().resource::<CosmicEventLog>().entries.is_empty(),
        "GPU nucleation milestones must log into CosmicEventLog"
    );
    assert!(
        !app.world()
            .resource::<NotificationToast>()
            .message
            .is_empty(),
        "GPU nucleation milestone must populate NotificationToast"
    );
}

#[test]
fn test_track2_cpu_spawners_yield_while_gpu_nucleation_drives_active_disk() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(SimTime {
            elapsed_years: 200.0,
            ..default()
        })
        .init_resource::<TimeWarp>()
        .insert_resource(SimulationConfig {
            enable_gpu_compute: true,
            gpu_compute_active: true,
            active_particles: 500_000,
            gas_density_scale: 1.0,
            ..default()
        })
        .insert_resource(DiskParameters {
            disk_mass: 0.00035,
            snow_line_au: 3.65,
            gas_disk_lifetime_yr: 5.0e6,
            ..default()
        })
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::AccretionDiskGenesis,
            scenario_time_years: 200.0,
            ..default()
        })
        .init_resource::<PlanetesimalSpawner>()
        .add_systems(
            Update,
            (
                auto_spawn_planetesimals,
                spawn_streaming_instability_minor_bodies,
            ),
        );

    app.update();
    assert_eq!(
        app.world().resource::<PlanetesimalSpawner>().total_spawned,
        0,
        "CPU statistical spawners must yield while GPU disk nucleation drives 500k VRAM swarm"
    );

    // Turning off GPU mode resumes CPU spawner cleanly
    app.world_mut()
        .resource_mut::<SimulationConfig>()
        .gpu_compute_active = false;
    app.update();
    assert!(
        app.world().resource::<PlanetesimalSpawner>().total_spawned > 0,
        "CPU fallback must resume cleanly when GPU compute is inactive"
    );
}

#[test]
fn test_track2_all_gpu_nucleation_and_swarm_systems_initialize_without_query_conflicts() {
    let (_tx, rx) = flume::bounded::<Vec<u8>>(2);
    let (recycle_tx, recycle_rx) = flume::bounded::<Vec<u8>>(2);
    let (_ntx, nrx) = flume::bounded::<GpuNucleationBatch>(2);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimTime>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimulationConfig>()
        .init_resource::<DiskParameters>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<ScenarioReloadGeneration>()
        .init_resource::<PlanetesimalSpawner>()
        .init_resource::<NotificationToast>()
        .init_resource::<CosmicEventLog>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<ParticleSwarmMaterial>>()
        .insert_resource(GpuReadbackReceiver { rx, recycle_tx })
        .insert_resource(GpuNucleationReceiver { rx: nrx })
        .add_systems(Startup, setup_particle_swarm)
        .add_systems(
            Update,
            (
                receive_gpu_readback,
                receive_gpu_nucleation_events.after(receive_gpu_readback),
                update_particle_swarm.after(receive_gpu_nucleation_events),
                auto_spawn_planetesimals,
                spawn_streaming_instability_minor_bodies,
            ),
        );
    drop(recycle_rx);
    app.update();
}
