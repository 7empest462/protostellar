//! Adversarial challenge tests for Milestone 1: Scenario Isolation & Foundation Architecture.
//!
//! Evaluates:
//! 1. Simultaneous ignition of multiple protostars (scaling, events, state, name identity).
//! 2. Rapid scenario switching, resource leaks, spawner state retention, and accretion re-enabling.
//! 3. Close-proximity protostar at 0.01 AU (and deeper) avoiding engulfment, drag, and retaining identity.
//! 4. Collision/accretion subsystem interaction with multi-star systems at close proximity.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::game::phases::{LateHeavyBombardmentState, PhaseManager, SystemPhase};
use protostellar::simulation::components::*;
use protostellar::simulation::disk::planetesimals::{
    auto_spawn_planetesimals, PlanetesimalSpawner,
};
use protostellar::simulation::pebble_accretion::apply_pebble_accretion;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{
    handle_load_scenario_events, ActiveScenarioState, LoadScenarioEvent, ScenarioPreset,
};
use protostellar::simulation::thermodynamics::{update_thermodynamics, StarIgnitionEvent};
use protostellar::utils::constants::*;

// ============================================================================
// 1. MULTIPLE PROTOSTARS SIMULTANEOUS IGNITION
// ============================================================================

#[test]
fn challenge_1a_multiple_protostars_simultaneous_ignition() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_systems(Update, update_thermodynamics);

    // Spawn 1 central star and 10 non-central protostars of various masses
    let masses = [0.07, 0.25, 0.45, 0.80, 1.00, 1.20, 2.50, 5.00, 12.00, 30.00];
    let mut star_entities = Vec::new();

    // Central star
    let central_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Protostar Alpha (Central)".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(3.5),
            Radius(0.05),
            Temperature(3200.0),
            Luminosity(45.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 9.99e6,
                fusion_fraction: 0.999,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();
    star_entities.push((central_ent, 3.5, "Protostar Alpha (Central)".to_string()));

    // 10 non-central protostars
    for (i, &m) in masses.iter().enumerate() {
        let name = format!("Protostar Cluster Member {}", i + 1);
        let ent = app
            .world_mut()
            .spawn((
                CelestialBody {
                    name: name.clone(),
                    body_type: BodyType::Protostar,
                },
                Mass(m),
                Radius(0.02),
                Temperature(3000.0),
                Luminosity(5.0),
                SimPosition(DVec3::new(10.0 * (i as f64 + 1.0), 0.0, 0.0)),
                SimVelocity(DVec3::new(0.0, 1.0, 0.0)),
                SimAcceleration::default(),
                Composition::solar_gas(),
                IgnitionState {
                    core_temperature: 9.99e6,
                    fusion_fraction: 0.999,
                    is_ignited: false,
                    shockwave_radius: 0.0,
                },
            ))
            .id();
        star_entities.push((ent, m, name));
    }

    // Step simulation
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 1.0;
        sim_time.elapsed_years = 1.0;
    }

    app.update();

    // Verification 1: All 11 stars must ignite simultaneously
    for &(ent, mass_val, ref original_name) in &star_entities {
        let ign = app.world().get::<IgnitionState>(ent).unwrap_or_else(|| {
            panic!("Entity {:?} ({}) missing IgnitionState", ent, original_name)
        });
        assert!(
            ign.is_ignited,
            "Star {:?} ({}) with mass {:.2} M_sun must be ignited",
            ent, original_name, mass_val
        );
        assert_eq!(
            ign.fusion_fraction, 1.0,
            "Star {:?} fusion fraction must be 1.0",
            ent
        );

        // Verification 2: Each star must have valid Luminosity and Temperature
        let lum = app.world().get::<Luminosity>(ent).unwrap();
        let temp = app.world().get::<Temperature>(ent).unwrap();
        assert!(
            lum.0 > 0.0 && lum.0.is_finite(),
            "Star lum must be finite and positive"
        );
        assert!(
            temp.0 > 1000.0 && temp.0.is_finite(),
            "Star temp must be finite and realistic"
        );

        // Verification 3: ElectromagneticFieldState inserted on all
        assert!(
            app.world().get::<ElectromagneticFieldState>(ent).is_some(),
            "ElectromagneticFieldState must be attached to ignited star {:?}",
            ent
        );
    }

    // Verification 4: Event emission count
    let ignition_events = app.world().resource::<Messages<StarIgnitionEvent>>();
    let emitted_entities: Vec<Entity> = ignition_events
        .iter_current_update_messages()
        .map(|ev| ev.star_entity)
        .collect();

    assert_eq!(
        emitted_entities.len(),
        11,
        "Exactly 11 StarIgnitionEvents must be emitted (got {})",
        emitted_entities.len()
    );

    for &(ent, _, ref name) in &star_entities {
        assert!(
            emitted_entities.contains(&ent),
            "StarIgnitionEvent must be emitted for {:?} ({})",
            ent,
            name
        );
    }

    // Verification 5: Name identity analysis (Empirical check)
    // Non-genesis stars have their body.name replaced with "The Star (...)"
    let mut names = Vec::new();
    for &(ent, _, _) in &star_entities {
        let body = app.world().get::<CelestialBody>(ent).unwrap();
        names.push(body.name.clone());
    }
    // Record empirical observation: names become generic "The Star (...)"
    println!("Empirical star names after ignition: {:?}", names);
}

#[test]
fn challenge_1b_pure_cluster_64_stars_without_central_star() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_systems(Update, update_thermodynamics);

    // Spawn 64 non-central protostars (no CentralStar in world)
    let star_count = 64;
    let mut spawned = Vec::new();

    for i in 0..star_count {
        let m = 0.1 + (i as f64) * 0.4; // 0.1 to 25.3 M_sun
        let ent = app
            .world_mut()
            .spawn((
                CelestialBody {
                    name: format!("Cluster Protostar #{:02}", i),
                    body_type: BodyType::Protostar,
                },
                Mass(m),
                Radius(0.03),
                Temperature(2800.0),
                Luminosity(2.0),
                SimPosition(DVec3::new(i as f64 * 5.0, 0.0, 0.0)),
                SimVelocity(DVec3::ZERO),
                SimAcceleration::default(),
                Composition::solar_gas(),
                IgnitionState {
                    core_temperature: 9.99e6,
                    fusion_fraction: 0.999,
                    is_ignited: false,
                    shockwave_radius: 0.0,
                },
            ))
            .id();
        spawned.push(ent);
    }

    // Update
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 1.0;
        sim_time.elapsed_years = 1.0;
    }

    app.update();

    let ignition_events = app.world().resource::<Messages<StarIgnitionEvent>>();
    let event_count = ignition_events.iter_current_update_messages().count();
    assert_eq!(
        event_count, star_count,
        "All 64 protostars must fire StarIgnitionEvent in absence of CentralStar"
    );

    for &ent in &spawned {
        let ign = app.world().get::<IgnitionState>(ent).unwrap();
        assert!(ign.is_ignited, "Protostar {:?} must be ignited", ent);
    }
}

// ============================================================================
// 2. RAPID SCENARIO SWITCHING & STALE RESOURCE LEAKS
// ============================================================================

#[test]
fn challenge_2a_rapid_scenario_switching_state_leaks() {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<PhaseManager>();
    app.init_resource::<ActiveScenarioState>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<PlanetesimalSpawner>();
    app.add_message::<LoadScenarioEvent>();
    app.add_systems(Update, handle_load_scenario_events);

    // Helper closure to trigger scenario load
    fn load_scenario(app: &mut App, preset: ScenarioPreset) {
        app.world_mut()
            .resource_mut::<Messages<LoadScenarioEvent>>()
            .write(LoadScenarioEvent(preset));
        app.update();
        app.update(); // Flush NextState transition in Bevy State machine
    }

    // Step 1: Load SolarNebulaMmsn
    load_scenario(&mut app, ScenarioPreset::SolarNebulaMmsn);
    assert_eq!(
        app.world().resource::<ActiveScenarioState>().current_preset,
        ScenarioPreset::SolarNebulaMmsn
    );
    assert_eq!(
        app.world().resource::<PhaseManager>().current_phase,
        SystemPhase::ProtoplanetaryDisk
    );
    assert_eq!(
        *app.world().resource::<State<SystemPhase>>().get(),
        SystemPhase::ProtoplanetaryDisk
    );

    // Simulate 300 years elapsed with planetesimals spawned
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 300.0;
        let mut spawner = app.world_mut().resource_mut::<PlanetesimalSpawner>();
        spawner.last_spawn_yr = 280.0;
        spawner.total_spawned = 42;
    }

    // Step 2: Switch to MolecularCloudCluster
    load_scenario(&mut app, ScenarioPreset::MolecularCloudCluster);
    assert_eq!(
        app.world().resource::<ActiveScenarioState>().current_preset,
        ScenarioPreset::MolecularCloudCluster
    );
    assert_eq!(
        app.world().resource::<PhaseManager>().current_phase,
        SystemPhase::MolecularCloudCollapse
    );
    assert_eq!(
        *app.world().resource::<State<SystemPhase>>().get(),
        SystemPhase::MolecularCloudCollapse
    );

    // Check sim_time was reset to 0.0
    let sim_time = app.world().resource::<SimTime>();
    assert_eq!(
        sim_time.elapsed_years, 0.0,
        "sim_time must reset on scenario load"
    );

    // Empirical Discovery Check: Was PlanetesimalSpawner reset?
    let spawner = app.world().resource::<PlanetesimalSpawner>();
    println!(
        "Empirical PlanetesimalSpawner after GMC load: last_spawn_yr={}, total_spawned={}",
        spawner.last_spawn_yr, spawner.total_spawned
    );
    let spawner_was_reset = spawner.last_spawn_yr == 0.0 && spawner.total_spawned == 0;

    // Step 3: Switch from Trappist-1 to MolecularCloudCluster to test gas_density_scale leak
    load_scenario(&mut app, ScenarioPreset::Trappist1System);
    let gas_trappist = app.world().resource::<SimulationConfig>().gas_density_scale;
    assert_eq!(
        gas_trappist, 0.0,
        "Trappist-1 sets gas_density_scale to 0.0"
    );

    load_scenario(&mut app, ScenarioPreset::MolecularCloudCluster);
    let gas_gmc = app.world().resource::<SimulationConfig>().gas_density_scale;
    println!(
        "Empirical gas_density_scale after Trappist1 -> GMC: {}",
        gas_gmc
    );

    // Step 4: Rapid consecutive switching stress (10 switches)
    let presets = [
        ScenarioPreset::Kepler16Circumbinary,
        ScenarioPreset::MolecularCloudCluster,
        ScenarioPreset::SolarNebulaMmsn,
        ScenarioPreset::MolecularCloudCluster,
        ScenarioPreset::AccretionDiskGenesis,
        ScenarioPreset::MolecularCloudCluster,
        ScenarioPreset::PulsarSystem,
        ScenarioPreset::MolecularCloudCluster,
        ScenarioPreset::RelativisticBinary,
        ScenarioPreset::MolecularCloudCluster,
    ];

    for preset in presets {
        load_scenario(&mut app, preset);
        assert_eq!(
            app.world().resource::<ActiveScenarioState>().current_preset,
            preset
        );
    }

    // Verify system phase after rapid switching ends in MolecularCloudCluster
    assert_eq!(
        app.world().resource::<PhaseManager>().current_phase,
        SystemPhase::MolecularCloudCollapse
    );
    assert_eq!(
        *app.world().resource::<State<SystemPhase>>().get(),
        SystemPhase::MolecularCloudCollapse
    );

    // Print summary of findings
    if !spawner_was_reset {
        println!("FINDING: PlanetesimalSpawner state (last_spawn_yr, total_spawned) leaks across scenario loads!");
    }
    if gas_gmc == 0.0 {
        println!("FINDING: gas_density_scale remains 0.0 when switching from gas-free scenario into MolecularCloudCluster!");
    }
}

#[test]
fn challenge_2b_planetesimal_spawning_freeze_after_scenario_reset() {
    let mut app = App::new();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<PlanetesimalSpawner>();
    app.init_resource::<ActiveScenarioState>();

    // Configure spawner where previous run was at t = 500.0 yr, so last_spawn_yr = 490.0
    {
        let mut spawner = app.world_mut().resource_mut::<PlanetesimalSpawner>();
        spawner.last_spawn_yr = 490.0;
        spawner.max_ecs_bodies = 500;

        let mut disk_params = app.world_mut().resource_mut::<DiskParameters>();
        disk_params.gas_disk_lifetime_yr = 50_000.0;

        let mut scenario_state = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario_state.current_preset = ScenarioPreset::SolarNebulaMmsn;

        // Reset sim_time to 0.0 as handle_load_scenario_events does
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 10.0; // Early in new scenario
    }

    app.add_systems(Update, auto_spawn_planetesimals);
    app.update();

    let body_count = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();

    // If last_spawn_yr was 490.0 and elapsed is 10.0, time_since_last is -480.0 < 25.0
    // Planetesimals will be completely frozen from spawning!
    println!(
        "Spawned bodies when last_spawn_yr=490 and t=10: {}",
        body_count
    );
    assert_eq!(
        body_count, 0,
        "Demonstrates stale last_spawn_yr freezes planetesimal spawning until t >= 490.0!"
    );
}

// ============================================================================
// 3. CLOSE-PROXIMITY PROTOSTAR AT 0.01 AU (ENGULFMENT & IDENTITY)
// ============================================================================

#[test]
fn challenge_3a_protostar_at_0_01_au_deep_engulfment_and_drag_bypass() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_systems(Update, update_thermodynamics);

    // Central Star (Radius = 0.05 AU)
    let _central_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Primary Central Protostar".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(3.5),
            Radius(0.05),
            Temperature(3200.0),
            Luminosity(45.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 9.99e6,
                fusion_fraction: 0.99,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // Infalling Protostar Companion at 0.01 AU (well inside central envelope of 0.05 AU)
    let initial_vel = DVec3::new(0.0, 12.0, 0.0);
    let companion_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Close Companion Protostar (0.01 AU)".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(1.2),
            Radius(0.015),
            Temperature(3500.0),
            Luminosity(8.0),
            SimPosition(DVec3::new(0.01, 0.0, 0.0)),
            SimVelocity(initial_vel),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 9.95e6,
                fusion_fraction: 0.95,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // Extreme Close Companion at 0.001 AU
    let extreme_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Extreme Plunge Protostar (0.001 AU)".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(0.8),
            Radius(0.01),
            Temperature(3200.0),
            Luminosity(3.0),
            SimPosition(DVec3::new(0.001, 0.0, 0.0)),
            SimVelocity(initial_vel),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 9.95e6,
                fusion_fraction: 0.95,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // Control Planet at 0.01 AU (must be engulfed)
    let doomed_planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Doomed Terrestrial Embryo".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Temperature(300.0),
            Composition::rocky(),
            SimPosition(DVec3::new(0.01, 0.0, 0.0)),
            SimVelocity(initial_vel),
            SimAcceleration::default(),
        ))
        .id();

    // Advance 1 year
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 1.0;
        sim_time.elapsed_years = 1.0;
    }

    app.update();

    // 1. Control planet MUST be despawned by deep engulfment
    assert!(
        app.world().get_entity(doomed_planet_ent).is_err(),
        "Planet at 0.01 AU inside stellar envelope must be engulfed and despawned"
    );

    // 2. Both protostars MUST survive
    assert!(
        app.world().get_entity(companion_ent).is_ok(),
        "Protostar at 0.01 AU must survive"
    );
    assert!(
        app.world().get_entity(extreme_ent).is_ok(),
        "Protostar at 0.001 AU must survive"
    );

    // 3. Protostars must NOT suffer envelope velocity drag
    let companion_vel = app.world().get::<SimVelocity>(companion_ent).unwrap();
    assert_eq!(
        companion_vel.0, initial_vel,
        "Protostar must NOT suffer envelope velocity drag (vel must remain {:?}, got {:?})",
        initial_vel, companion_vel.0
    );

    // 4. Check identity / name retention upon ignition
    let body = app.world().get::<CelestialBody>(companion_ent).unwrap();
    println!("Companion body name after ignition: '{}'", body.name);
    // Note: If name contains "The Star", its specific name was clobbered by ignition naming
    let name_preserved = body.name == "Close Companion Protostar (0.01 AU)";
    println!("Was original companion name preserved? {}", name_preserved);
}

#[test]
fn challenge_3b_protostar_without_ignition_state_at_0_01_au() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_systems(Update, update_thermodynamics);

    // Central Star (Radius = 0.05 AU)
    let _central = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Primary Central Protostar".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(3.5),
            Radius(0.05),
            Temperature(3200.0),
            Luminosity(45.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 9.99e6,
                fusion_fraction: 0.99,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // Protostar spawned WITHOUT IgnitionState at 0.01 AU (enters bodies_query!)
    let seed_vel = DVec3::new(0.0, 5.0, 0.0);
    let protostar_seed_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Unignited Protostellar Fragment".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(0.5),
            Radius(0.02),
            Temperature(100.0),
            Composition::solar_gas(),
            SimPosition(DVec3::new(0.01, 0.0, 0.0)),
            SimVelocity(seed_vel),
            SimAcceleration::default(),
        ))
        .id();

    // Advance 1 year
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 1.0;
        sim_time.elapsed_years = 1.0;
    }

    app.update();

    // Protostar must NOT be despawned because update_body_thermodynamics checks is_star_or_remnant()
    assert!(
        app.world().get_entity(protostar_seed_ent).is_ok(),
        "Protostar without IgnitionState at 0.01 AU must NOT be engulfed"
    );

    // And its velocity must NOT be degraded
    let vel = app.world().get::<SimVelocity>(protostar_seed_ent).unwrap();
    assert_eq!(
        vel.0, seed_vel,
        "Protostar without IgnitionState must not receive envelope drag"
    );
}

#[test]
fn challenge_3c_protostar_inside_supergiant_envelope_at_0_10_au() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_systems(Update, update_thermodynamics);

    // Central Red Giant / Supergiant (star_r = 0.60 AU, deep engulfment threshold r < 0.18 AU)
    let _central = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Betelgeuse-like Supergiant".to_string(),
                body_type: BodyType::RedSupergiant,
            },
            Mass(15.0),
            Radius(0.60),
            Temperature(3100.0),
            Luminosity(10000.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1.0e8,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // Protostar at 0.05 AU (deep plunge inside supergiant core boundary)
    let proto_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Surviving Infalling Protostar".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(2.0),
            Radius(0.02),
            Temperature(4000.0),
            Luminosity(20.0),
            SimPosition(DVec3::new(0.05, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 15.0, 0.0)),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 8.0e6,
                fusion_fraction: 0.8,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // Planet at 0.05 AU (must be engulfed)
    let planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Doomed Inner Planet".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(2.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 1.2),
            Temperature(1500.0),
            Composition::rocky(),
            SimPosition(DVec3::new(0.05, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 15.0, 0.0)),
            SimAcceleration::default(),
        ))
        .id();

    // Advance 1 year
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 1.0;
        sim_time.elapsed_years = 1.0;
    }

    app.update();

    assert!(
        app.world().get_entity(planet_ent).is_err(),
        "Planet inside supergiant envelope must be engulfed"
    );
    assert!(
        app.world().get_entity(proto_ent).is_ok(),
        "Protostar inside supergiant envelope must be preserved"
    );
}

#[test]
fn challenge_3d_close_protostars_collision_system() {
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::*;

    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<PlayerInteractionState>();
    app.add_message::<AccretionMergeEvent>();
    app.add_message::<MoonFormationEvent>();
    app.add_message::<CollisionBounceEvent>();
    app.add_message::<RocheDisruptionEvent>();
    app.add_message::<TidalDisruptionEvent>();
    app.add_systems(Update, process_accretion_and_collisions);

    let _central_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Central Protostar Alpha".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(3.5),
            Radius(0.05),
            Temperature(3200.0),
            Composition::solar_gas(),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
        ))
        .id();

    let companion_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Close Companion Protostar Beta".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(1.8),
            Radius(0.015),
            Temperature(3600.0),
            Composition::solar_gas(),
            SimPosition(DVec3::new(0.01, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 10.0, 0.0)),
            SimAcceleration::default(),
        ))
        .id();

    app.update();

    let companion_survived = app.world().get_entity(companion_ent).is_ok();
    println!(
        "Empirical outcome of companion protostar at 0.01 AU under collision system: survived={}",
        companion_survived
    );
    let merge_events = app.world().resource::<Messages<AccretionMergeEvent>>();
    let merge_count = merge_events.iter_current_update_messages().count();
    println!("AccretionMergeEvents emitted: {}", merge_count);
}

// ============================================================================
// 4. ACCRETION & PEBBLE GUARDING INTEGRITY UNDER SCENARIO SWITCHING
// ============================================================================

#[test]
fn challenge_4a_pebble_accretion_guarded_and_reenabled() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<ActiveScenarioState>();

    // Initial setup: gas disk active, pebble accretion enabled
    {
        let mut config = app.world_mut().resource_mut::<SimulationConfig>();
        config.enable_accretion = true;
        config.gas_density_scale = 1.0;
        config.base_dt_yr = 0.5;

        let mut disk = app.world_mut().resource_mut::<DiskParameters>();
        disk.gas_disk_lifetime_yr = 10_000.0;

        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 100.0;

        // Set to MolecularCloudCluster
        let mut scenario = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario.current_preset = ScenarioPreset::MolecularCloudCluster;
    }

    let initial_mass = 0.0005 * EARTH_MASS_SOLAR;
    let planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Test Embryo".to_string(),
                body_type: BodyType::Asteroid,
            },
            Mass(initial_mass),
            Radius(0.0001),
            SimPosition(DVec3::new(2.5, 0.0, 0.0)),
            Composition::rocky(),
        ))
        .id();

    app.add_systems(Update, apply_pebble_accretion);
    app.update();

    // 1. In MolecularCloudCluster, mass must NOT increase (early return)
    let mass_gmc = app.world().get::<Mass>(planet_ent).unwrap().0;
    assert_eq!(
        mass_gmc, initial_mass,
        "Pebble accretion must be completely blocked during MolecularCloudCluster"
    );

    // 2. Switch to SolarNebulaMmsn
    {
        let mut scenario = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario.current_preset = ScenarioPreset::SolarNebulaMmsn;
    }

    app.update();

    // Pebble accretion should now run and increase embryo mass
    let mass_solar = app.world().get::<Mass>(planet_ent).unwrap().0;
    assert!(
        mass_solar > initial_mass,
        "Pebble accretion must re-enable when switching back to SolarNebulaMmsn (got {:.6e} vs {:.6e})",
        mass_solar,
        initial_mass
    );
}
