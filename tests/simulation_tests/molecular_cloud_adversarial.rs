//! Adversarial and edge-case integration tests for Giant Molecular Cloud (GMC) & Accretion Systems.

#[test]
fn test_adversarial_active_scenario_state_none_resilience() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::planetesimals::{
        auto_spawn_delayed_proto_earth, auto_spawn_planetesimals, PlanetesimalSpawner,
    };
    use protostellar::simulation::pebble_accretion::{
        apply_pebble_accretion, spawn_streaming_instability_minor_bodies,
    };
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::*;

    // Explicitly initialize a standalone Bevy app WITHOUT ActiveScenarioState resource
    let mut app = App::new();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<PlanetesimalSpawner>();

    // Verify ActiveScenarioState is absent from the world
    assert!(
        app.world()
            .get_resource::<protostellar::simulation::scenarios::ActiveScenarioState>()
            .is_none(),
        "ActiveScenarioState must NOT be present in standalone mock test world"
    );

    // Set up disk parameters and spawner state
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 300.0;
        sim_time.current_dt_yr = 1.0;

        let mut disk = app.world_mut().resource_mut::<DiskParameters>();
        disk.central_star_mass = 1.0;
        disk.inner_radius_au = 0.2;
        disk.outer_radius_au = 40.0;
        disk.gas_disk_lifetime_yr = 100_000.0;

        let mut spawner = app.world_mut().resource_mut::<PlanetesimalSpawner>();
        spawner.max_ecs_bodies = 100;
        spawner.last_spawn_yr = 0.0;

        let mut config = app.world_mut().resource_mut::<SimulationConfig>();
        config.enable_accretion = true;
        config.gas_density_scale = 1.0;
    }

    // Spawn a target protoplanet for pebble accretion in giant core formation zone (5.2 AU)
    let initial_mass = 0.5 * EARTH_MASS_SOLAR;
    let target = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Protoplanet,
                name: "Test Protoplanet".to_string(),
            },
            Mass(initial_mass),
            SimPosition(DVec3::new(5.2, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Radius(0.0001),
            Composition::rocky(),
        ))
        .id();

    // Register all guarded systems
    app.add_systems(
        Update,
        (
            auto_spawn_planetesimals,
            apply_pebble_accretion,
            spawn_streaming_instability_minor_bodies,
            auto_spawn_delayed_proto_earth,
        ),
    );

    // Update the app: must execute smoothly WITHOUT any panic
    app.update();

    // Verify planetesimal spawner operated normally despite None scenario state
    let spawner = app.world().resource::<PlanetesimalSpawner>();
    assert!(
        spawner.total_spawned > 0,
        "auto_spawn_planetesimals must successfully spawn planetesimals when ActiveScenarioState is None"
    );

    // Verify pebble accretion updated the target protoplanet without panicking
    let updated_mass = app.world().get::<Mass>(target).unwrap().0;
    assert!(
        updated_mass > initial_mass,
        "apply_pebble_accretion must increase target protoplanet mass without panic when ActiveScenarioState is None"
    );
}

fn verify_extreme_time_warp_gmc_cluster() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::planetesimals::{
        auto_spawn_planetesimals, PlanetesimalSpawner,
    };
    use protostellar::simulation::pebble_accretion::{
        apply_pebble_accretion, spawn_streaming_instability_minor_bodies,
    };
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<PlanetesimalSpawner>();
    app.init_resource::<ActiveScenarioState>();

    // Configure maximum time warp (1,000,000x) and advanced simulation time
    {
        let mut time_warp = app.world_mut().resource_mut::<TimeWarp>();
        time_warp.multiplier = 1_000_000.0;
        time_warp.is_paused = false;

        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 500_000.0;
        sim_time.current_dt_yr = 1_000.0;

        let mut disk = app.world_mut().resource_mut::<DiskParameters>();
        disk.gas_disk_lifetime_yr = 1_000_000.0;

        let mut spawner = app.world_mut().resource_mut::<PlanetesimalSpawner>();
        spawner.max_ecs_bodies = 500;
        spawner.last_spawn_yr = 0.0;

        let mut scenario_state = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    }

    // Spawn a candidate non-central seed in GMC
    let initial_seed_mass = 0.05;
    let seed_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Protostar,
                name: "GMC Protostellar Seed".to_string(),
            },
            Mass(initial_seed_mass),
            SimPosition(DVec3::new(25.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 1.2)),
            Radius(0.01),
            Composition::solar_gas(),
        ))
        .id();

    app.add_systems(
        Update,
        (
            auto_spawn_planetesimals,
            apply_pebble_accretion,
            spawn_streaming_instability_minor_bodies,
        ),
    );

    // Step the simulation under 1,000,000x time warp
    app.update();

    // 1. Planetesimal spawner MUST be completely locked out by GMC guard
    let spawner = app.world().resource::<PlanetesimalSpawner>();
    assert_eq!(
        spawner.total_spawned, 0,
        "GMC guard must hold across max dt: zero planetesimals spawned at 1,000,000x warp"
    );

    // 2. No bodies other than our initial seed should exist
    let total_bodies = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();
    assert_eq!(
        total_bodies, 1,
        "GMC guard must prevent any minor bodies or planetesimals from spawning at 1,000,000x warp"
    );

    // 3. Pebble accretion MUST NOT accrete onto the seed
    let seed_mass_after = app.world().get::<Mass>(seed_ent).unwrap().0;
    assert_eq!(
        seed_mass_after, initial_seed_mass,
        "GMC guard must prevent pebble accretion onto GMC seeds across max dt"
    );
}

fn verify_extreme_time_warp_disk_stability() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::planetesimals::{
        auto_spawn_planetesimals, PlanetesimalSpawner,
    };
    use protostellar::simulation::pebble_accretion::apply_pebble_accretion;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
    use protostellar::utils::constants::*;

    let mut disk_app = App::new();
    disk_app.init_resource::<TimeWarp>();
    disk_app.init_resource::<SimTime>();
    disk_app.init_resource::<SimulationConfig>();
    disk_app.init_resource::<DiskParameters>();
    disk_app.init_resource::<PlanetesimalSpawner>();
    disk_app.init_resource::<ActiveScenarioState>();
    disk_app.init_resource::<PlayerInteractionState>();
    disk_app.add_message::<AccretionMergeEvent>();
    disk_app.add_message::<MoonFormationEvent>();
    disk_app.add_message::<CollisionBounceEvent>();
    disk_app.add_message::<RocheDisruptionEvent>();
    disk_app.add_message::<TidalDisruptionEvent>();

    {
        let mut time_warp = disk_app.world_mut().resource_mut::<TimeWarp>();
        time_warp.multiplier = 1_000_000.0;
        time_warp.is_paused = false;

        let mut sim_time = disk_app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 100_000.0;
        sim_time.current_dt_yr = 1_000.0;

        let mut disk = disk_app.world_mut().resource_mut::<DiskParameters>();
        disk.central_star_mass = 1.0;
        disk.outer_radius_au = 50.0;
        disk.gas_disk_lifetime_yr = 5.0e6;

        let mut spawner = disk_app.world_mut().resource_mut::<PlanetesimalSpawner>();
        spawner.max_ecs_bodies = 50;
        spawner.last_spawn_yr = 0.0;

        let mut scenario_state = disk_app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario_state.current_preset = ScenarioPreset::SolarNebulaMmsn;

        let mut config = disk_app.world_mut().resource_mut::<SimulationConfig>();
        config.enable_accretion = true;
        config.gas_density_scale = 1.0;
    }

    // Spawn a protoplanet in disk to test pebble accretion dt clamping (.min(50.0))
    let proto = disk_app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Protoplanet,
                name: "Proto-Jupiter".to_string(),
            },
            Mass(10.0 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(5.2, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.7)),
            Radius(0.0005),
            Composition::rocky(),
        ))
        .id();

    // Spawn two colliding bodies to verify collision dt clamping
    let _b1 = disk_app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Target".to_string(),
            },
            Mass(1.0 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Radius(EARTH_RADIUS_AU),
            Temperature(300.0),
            Composition::rocky(),
        ))
        .id();

    let _b2 = disk_app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Planetesimal,
                name: "Impactor".to_string(),
            },
            Mass(0.01 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(1.00001, 0.0, 0.0)),
            SimVelocity(DVec3::new(-0.01, 0.0, 0.0)),
            Radius(EARTH_RADIUS_AU * 0.1),
            Temperature(250.0),
            Composition::rocky(),
        ))
        .id();

    disk_app.add_systems(
        Update,
        (
            auto_spawn_planetesimals,
            apply_pebble_accretion,
            process_accretion_and_collisions,
        ),
    );

    disk_app.update();

    // Verify pebble accretion remained finite and bounded due to dt clamp (.min(50.0))
    let proto_mass = disk_app.world().get::<Mass>(proto).unwrap().0;
    assert!(
        proto_mass.is_finite(),
        "Pebble accretion mass must remain finite at 1,000,000x warp"
    );
    assert!(
        proto_mass > 10.0 * EARTH_MASS_SOLAR && proto_mass < 500.0 * EARTH_MASS_SOLAR,
        "Pebble accretion mass must be safely bounded by dt clamp at 1,000,000x warp (got {:.4e})",
        proto_mass
    );

    // Verify planetesimal spawning did not exceed max_ecs_bodies despite huge time_since_last
    let spawner_disk = disk_app.world().resource::<PlanetesimalSpawner>();
    assert!(
        spawner_disk.total_spawned <= 50,
        "Planetesimal spawner batch must be clamped and not overflow max_ecs_bodies (got {})",
        spawner_disk.total_spawned
    );
}

#[test]
fn test_adversarial_max_time_warp_million_x_stability_and_guarding() {
    verify_extreme_time_warp_gmc_cluster();
    verify_extreme_time_warp_disk_stability();
}

fn verify_solar_nebula_mmsn_spawning() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::planetesimals::{
        auto_spawn_delayed_proto_earth, auto_spawn_planetesimals, PlanetesimalSpawner,
    };
    use protostellar::simulation::pebble_accretion::spawn_streaming_instability_minor_bodies;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<PlanetesimalSpawner>();
    app.init_resource::<ActiveScenarioState>();

    {
        let mut scenario = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario.current_preset = ScenarioPreset::SolarNebulaMmsn;

        let mut disk = app.world_mut().resource_mut::<DiskParameters>();
        disk.central_star_mass = 1.0;
        disk.inner_radius_au = 0.2;
        disk.outer_radius_au = 50.0;
        disk.gas_disk_lifetime_yr = 5.0e6;

        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 600.0; // Past delayed Proto-Earth spawn threshold (500 yr)
        sim_time.current_dt_yr = 1.0;

        let mut spawner = app.world_mut().resource_mut::<PlanetesimalSpawner>();
        spawner.max_ecs_bodies = 200;
        spawner.last_spawn_yr = 0.0;

        let mut config = app.world_mut().resource_mut::<SimulationConfig>();
        config.enable_accretion = true;
        config.gas_density_scale = 1.0;
    }

    // Spawn central sun at origin
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "Sun".to_string(),
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Composition::solar_gas(),
    ));

    app.add_systems(
        Update,
        (
            auto_spawn_planetesimals,
            spawn_streaming_instability_minor_bodies,
            auto_spawn_delayed_proto_earth,
        ),
    );

    app.update();

    // 1. Verify planetesimals spawned in MMSN
    let spawner = app.world().resource::<PlanetesimalSpawner>();
    assert!(
        spawner.total_spawned > 0,
        "auto_spawn_planetesimals must actively spawn planetesimals in SolarNebulaMmsn"
    );

    // 2. Verify delayed Proto-Earth spawned in MMSN
    let mut earth_query = app
        .world_mut()
        .query_filtered::<&CelestialBody, With<CelestialBody>>();
    let earth_found = earth_query
        .iter(app.world())
        .any(|b| b.name.contains("Proto-Earth") || b.name == "Earth");
    assert!(
        earth_found,
        "auto_spawn_delayed_proto_earth must spawn Proto-Earth in SolarNebulaMmsn scenario"
    );
}

fn verify_accretion_disk_genesis_spawning() {
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::planetesimals::{
        auto_spawn_delayed_proto_earth, auto_spawn_planetesimals, PlanetesimalSpawner,
    };
    use protostellar::simulation::pebble_accretion::{
        apply_pebble_accretion, spawn_streaming_instability_minor_bodies,
    };
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{
        spawn_accretion_disk_genesis, ActiveScenarioState, ScenarioPreset,
    };

    let mut app = App::new();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<PlanetesimalSpawner>();
    app.init_resource::<ActiveScenarioState>();

    // Set preset to AccretionDiskGenesis
    {
        let mut scenario = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario.current_preset = ScenarioPreset::AccretionDiskGenesis;

        let mut disk = app.world_mut().resource_mut::<DiskParameters>();
        disk.central_star_mass = 1.33;
        disk.inner_radius_au = 0.12;
        disk.outer_radius_au = 55.0;
        disk.gas_disk_lifetime_yr = 5.0e6;

        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 500.0;
        sim_time.current_dt_yr = 1.0;

        let mut spawner = app.world_mut().resource_mut::<PlanetesimalSpawner>();
        spawner.max_ecs_bodies = 200;
        spawner.last_spawn_yr = 0.0;

        let mut config = app.world_mut().resource_mut::<SimulationConfig>();
        config.enable_accretion = true;
        config.gas_density_scale = 1.0;
    }

    // Spawn Genesis protostar
    let mut disk_params = app.world().resource::<DiskParameters>().clone();
    spawn_accretion_disk_genesis(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    // Register systems
    app.add_systems(
        Update,
        (
            auto_spawn_planetesimals,
            spawn_streaming_instability_minor_bodies,
            apply_pebble_accretion,
            auto_spawn_delayed_proto_earth,
        ),
    );

    app.update();

    // 1. Verify planetesimals spawned in Genesis
    let spawner = app.world().resource::<PlanetesimalSpawner>();
    assert!(
        spawner.total_spawned > 0,
        "auto_spawn_planetesimals must actively spawn planetesimals in AccretionDiskGenesis"
    );

    // 2. Verify all spawned bodies have valid physical parameters (positive mass/radius, finite pos)
    let mut bodies_query = app
        .world_mut()
        .query_filtered::<(&Mass, &Radius, &SimPosition), (With<CelestialBody>, Without<CentralStar>)>();
    let non_star_count = bodies_query.iter(app.world()).count();
    assert!(
        non_star_count > 0,
        "AccretionDiskGenesis must populate non-star bodies dynamically"
    );
    for (mass, radius, pos) in bodies_query.iter(app.world()) {
        assert!(mass.0 > 0.0, "Body mass must be positive");
        assert!(radius.0 > 0.0, "Body radius must be positive");
        assert!(pos.0.x.is_finite() && pos.0.y.is_finite() && pos.0.z.is_finite());
    }

    // 3. Verify Delayed Proto-Earth is NEVER spawned in AccretionDiskGenesis
    let mut earth_query = app
        .world_mut()
        .query_filtered::<&CelestialBody, With<CelestialBody>>();
    let earth_found = earth_query
        .iter(app.world())
        .any(|b| b.name.contains("Proto-Earth") || b.name == "Earth");
    assert!(
        !earth_found,
        "auto_spawn_delayed_proto_earth must NOT spawn canned Proto-Earth in AccretionDiskGenesis"
    );
}

#[test]
fn test_adversarial_planetesimal_spawning_solar_nebula_and_genesis_regression() {
    verify_solar_nebula_mmsn_spawning();
    verify_accretion_disk_genesis_spawning();
}
