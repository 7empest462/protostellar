//! Integration tests for long-term scenario orbital stability, TRAPPIST-1 survival,
//! TDE dynamics, GMC boundaries, Oort survival, and relativistic binaries.

use bevy::prelude::*;

#[test]
fn test_trappist1_all_seven_planets_persist_through_simulation() {
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::{
        AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
    };
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::trappist::spawn_trappist_1_system;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    let config = SimulationConfig {
        gas_density_scale: 0.0,
        ..Default::default()
    };
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<LateHeavyBombardmentState>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<MoonFormationEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_systems(
            Update,
            (
                step_physics_simulation,
                process_accretion_and_collisions.after(step_physics_simulation),
            ),
        );

    let mut disk_params = app.world().resource::<DiskParameters>().clone();
    let star_entity = spawn_trappist_1_system(&mut app.world_mut().commands(), &mut disk_params);
    app.insert_resource(disk_params);
    app.insert_resource(ActiveScenarioState {
        current_preset: ScenarioPreset::Trappist1System,
        ..Default::default()
    });

    // Run first frame to spawn commands and initialize ECS
    app.update();

    // Verify initial spawn: exactly 1 star + 7 planets = 8 bodies
    let initial_bodies: Vec<String> = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .map(|b| b.name.clone())
        .collect();
    assert_eq!(
        initial_bodies.len(),
        8,
        "TRAPPIST-1 system must initially spawn 1 star + 7 resonant planets (found {initial_bodies:?})"
    );

    // Run 50 full simulation steps with physics and accretion/collision processing active
    for _ in 0..50 {
        app.update();
        let body_count = app
            .world_mut()
            .query::<&CelestialBody>()
            .iter(app.world())
            .count();
        assert_eq!(
            body_count, 8,
            "All 7 TRAPPIST-1 planets and star must persist on every step"
        );
    }

    // Verify all 7 planets and the central star remain alive and unswallowed
    let surviving_bodies: Vec<String> = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .map(|b| b.name.clone())
        .collect();

    assert_eq!(
        surviving_bodies.len(),
        8,
        "All 7 TRAPPIST-1 planets and star must survive without erroneous collision engulfment (found {surviving_bodies:?})"
    );

    let expected_worlds = [
        "TRAPPIST-1b",
        "TRAPPIST-1c",
        "TRAPPIST-1d",
        "TRAPPIST-1e",
        "TRAPPIST-1f",
        "TRAPPIST-1g",
        "TRAPPIST-1h",
    ];
    for expected in expected_worlds {
        assert!(
            surviving_bodies.iter().any(|n| n == expected),
            "Planet {expected} must survive in TRAPPIST-1 system (active bodies: {surviving_bodies:?})"
        );
    }

    assert!(
        app.world().get_entity(star_entity).is_ok(),
        "Central star TRAPPIST-1 must survive"
    );
}

#[test]
fn test_uranus_and_neptune_retain_ice_giant_classification_and_blue_palette() {
    use protostellar::rendering::bodies::palettes::compute_ice_giant_palette;
    use protostellar::simulation::components::classify_body_by_mass_and_comp;
    use protostellar::simulation::components::{BodyType, Composition};
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    // 1. Verify classify_body_by_mass_and_comp on typical Ice Giant compositions
    // Real Neptune / Uranus: mass ~ 14-17 M_earth, ~60% ices, ~20% rock, ~15% gas
    let comp_neptune = Composition {
        ice_frac: 0.60,
        silicate_frac: 0.20,
        metal_frac: 0.05,
        organics_frac: 0.00,
        gas_frac: 0.15,
    };
    let neptune_type =
        classify_body_by_mass_and_comp(17.15 * EARTH_MASS_SOLAR, &comp_neptune, false);
    assert_eq!(
        neptune_type,
        BodyType::IceGiant,
        "Neptune must be classified as BodyType::IceGiant"
    );

    // High volatile mantle with 40% gas (the user's scenario: 31% water, 23% rock, 6% metal, 40% gas)
    let comp_user_neptune = Composition {
        ice_frac: 0.31,
        silicate_frac: 0.23,
        metal_frac: 0.06,
        organics_frac: 0.00,
        gas_frac: 0.40,
    };
    let user_type =
        classify_body_by_mass_and_comp(20.0 * EARTH_MASS_SOLAR, &comp_user_neptune, false);
    assert_eq!(
        user_type,
        BodyType::IceGiant,
        "Volatile-rich planet with 31% ice and 40% gas must classify as IceGiant, not GasGiant"
    );

    // Pure Gas Giant (Jupiter-like: 88% gas, 5% rock, 5% metal, 2% ice)
    let comp_jupiter = Composition {
        ice_frac: 0.02,
        silicate_frac: 0.05,
        metal_frac: 0.05,
        organics_frac: 0.00,
        gas_frac: 0.88,
    };
    let jupiter_type =
        classify_body_by_mass_and_comp(317.8 * EARTH_MASS_SOLAR, &comp_jupiter, false);
    assert_eq!(
        jupiter_type,
        BodyType::GasGiant,
        "Jupiter must be classified as BodyType::GasGiant"
    );

    // 2. Verify specialized Ice Giant color palettes
    let neptune_color = compute_ice_giant_palette("Proto-Neptune", 60.0);
    let neptune_linear = LinearRgba::from(neptune_color);
    assert!(
        neptune_linear.blue > neptune_linear.red * 2.0,
        "Neptune palette must be predominantly azure blue: {neptune_linear:?}"
    );

    let uranus_color = compute_ice_giant_palette("Proto-Uranus", 75.0);
    let uranus_linear = LinearRgba::from(uranus_color);
    assert!(
        uranus_linear.blue > uranus_linear.red * 1.5
            && uranus_linear.green > uranus_linear.red * 1.5,
        "Uranus palette must be predominantly aquamarine/cyan: {uranus_linear:?}"
    );
}

#[test]
fn test_trappist1_long_term_coplanar_stability() {
    use bevy::math::DVec3;
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::trappist::spawn_trappist_1_system;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    let config = SimulationConfig {
        gas_density_scale: 0.0,
        ..Default::default()
    };
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<PlayerInteractionState>()
        .add_message::<protostellar::simulation::thermodynamics::StarIgnitionEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_message::<MoonFormationEvent>()
        .add_systems(
            Update,
            (
                step_physics_simulation,
                process_accretion_and_collisions.after(step_physics_simulation),
            ),
        );

    let mut disk_params = DiskParameters::default();
    let _star_entity = spawn_trappist_1_system(&mut app.world_mut().commands(), &mut disk_params);
    *app.world_mut().resource_mut::<DiskParameters>() = disk_params;
    app.world_mut()
        .resource_mut::<ActiveScenarioState>()
        .current_preset = ScenarioPreset::Trappist1System;

    // Run for 300 steps (equivalent to multiple orbits of all 7 planets)
    for _ in 0..300 {
        app.update();
    }

    // Verify all 7 planets and star survive
    let mut planets_query = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &SimVelocity)>();
    let bodies: Vec<(String, DVec3, DVec3)> = planets_query
        .iter(app.world())
        .map(|(b, pos, vel)| (b.name.clone(), pos.0, vel.0))
        .collect();

    assert_eq!(
        bodies.len(),
        8,
        "All 7 planets and the central star must survive"
    );

    // Verify orbits remain strictly coplanar (y ~ 0) and bounded within authentic semi-major axes
    for (name, pos, _vel) in bodies {
        if name.contains("TRAPPIST-1b")
            || name.contains("TRAPPIST-1c")
            || name.contains("TRAPPIST-1d")
            || name.contains("TRAPPIST-1e")
            || name.contains("TRAPPIST-1f")
            || name.contains("TRAPPIST-1g")
            || name.contains("TRAPPIST-1h")
        {
            assert!(
                pos.y.abs() < 1e-4,
                "Planet {name} must remain strictly coplanar (y={:.6} AU)",
                pos.y
            );
            let r = (pos.x * pos.x + pos.z * pos.z).sqrt();
            assert!(
                (0.008..0.08).contains(&r),
                "Planet {name} orbit must remain stable inside TRAPPIST-1 system (r={r:.4} AU)"
            );
        }
    }
}

#[test]
fn test_tde_scenario_has_complete_physics_components_and_orbits() {
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::relativity::RelativisticState;
    use protostellar::simulation::relativity::kerr::KerrBlackHole;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{
        ActiveScenarioState, ScenarioPreset, spawn_tde_scenario,
    };

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<PlayerInteractionState>()
        .add_systems(Update, step_physics_simulation);

    let mut disk_params = DiskParameters::default();
    let bh_ent = spawn_tde_scenario(&mut app.world_mut().commands(), &mut disk_params);
    *app.world_mut().resource_mut::<DiskParameters>() = disk_params;
    app.world_mut()
        .resource_mut::<ActiveScenarioState>()
        .current_preset = ScenarioPreset::TidalDisruptionEvent;
    app.update();

    let world = app.world();
    assert!(world.get::<SimAcceleration>(bh_ent).is_some());
    assert!(world.get::<CentralStar>(bh_ent).is_some());
    assert!(world.get::<KerrBlackHole>(bh_ent).is_some());
    assert!(world.get::<RelativisticState>(bh_ent).is_some());
    assert!(world.get::<StellarEvolutionState>(bh_ent).is_some());

    let mut q = app.world_mut().query::<(
        &CelestialBody,
        &Radius,
        &SimPosition,
        &SimVelocity,
        Option<&SimAcceleration>,
    )>();
    let rg = q
        .iter(app.world())
        .find(|(b, ..)| b.name == "Doomed Red Giant")
        .expect("Doomed Red Giant should exist");
    assert!(
        rg.1.0 < 2.0,
        "Red Giant radius must be physical (~0.75 AU), got {}",
        rg.1.0
    );
    assert!(rg.4.is_some(), "Red Giant must carry SimAcceleration");
    let start_pos = rg.2.0;

    for _ in 0..40 {
        app.update();
    }
    let end_pos = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition)>()
        .iter(app.world())
        .find(|(b, _)| b.name == "Doomed Red Giant")
        .unwrap()
        .1
        .0;
    assert!(
        (end_pos - start_pos).length() > 1e-4,
        "Doomed Red Giant must integrate forward in time"
    );
}

#[test]
fn test_gmc_outer_tether_allows_spiral_arms_up_to_1100_au() {
    use bevy::math::DVec3;
    use protostellar::simulation::components::BodyType;
    use protostellar::simulation::physics::{PhysicsBodyEntry, apply_velocity_kick_and_limits};
    use protostellar::simulation::scenarios::molecular_cloud::GMC_CORE_RADIUS_AU;

    let arm_star = PhysicsBodyEntry {
        entity: Entity::from_bits(1),
        mass: 2.0,
        pos: DVec3::new(650.0, 0.0, 0.0),
        vel: DVec3::new(0.0, 0.0, 5.5),
        acc: DVec3::ZERO,
        radius: 0.01,
        body_type: BodyType::MainSequenceStar,
        satellite: None,
        is_central_star: false,
        role_flags: 0,
    };
    let escaped_star = PhysicsBodyEntry {
        entity: Entity::from_bits(2),
        mass: 1.5,
        pos: DVec3::new(1220.0, 0.0, 0.0),
        vel: DVec3::new(3.0, 0.0, 4.0),
        acc: DVec3::ZERO,
        radius: 0.01,
        body_type: BodyType::MainSequenceStar,
        satellite: None,
        is_central_star: false,
        role_flags: 0,
    };
    let mut bodies = vec![arm_star, escaped_star];
    apply_velocity_kick_and_limits(&mut bodies, 0.01, 20.0, DVec3::ZERO, false, true, false);

    assert!(
        (bodies[0].pos.x - 650.0).abs() < 1e-9,
        "Outer arm star at 650 AU must not be clamped at 240 AU, got {}",
        bodies[0].pos.x
    );
    assert!(
        bodies[1].pos.length() <= GMC_CORE_RADIUS_AU + 50.0 + 1e-6,
        "Far star beyond GMC boundary must be contained near {GMC_CORE_RADIUS_AU} AU"
    );
    assert!(
        bodies[1].vel.z.abs() > 3.5,
        "Outer containment reflection must preserve tangential rotation, got vel={:?}",
        bodies[1].vel
    );
}

#[test]
fn test_oort_comet_survives_beyond_2000_au() {
    use bevy::math::DVec3;
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::components::*;
    use protostellar::simulation::oort::OortComet;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<PlayerInteractionState>()
        .add_systems(Update, step_physics_simulation);
    app.world_mut()
        .resource_mut::<ActiveScenarioState>()
        .current_preset = ScenarioPreset::SolarNebulaMmsn;

    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::MainSequenceStar,
            name: "Sun".into(),
        },
        CentralStar,
        Mass(1.0),
        Radius(0.00465),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration::default(),
    ));
    let oort_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Comet,
                name: "Oort Comet C/1000".into(),
            },
            Mass(1e-16),
            Radius(1e-7),
            SimPosition(DVec3::new(10_000.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.05, 0.0, 0.05)),
            SimAcceleration::default(),
            OortComet::default(),
        ))
        .id();

    for _ in 0..10 {
        app.update();
    }
    assert!(
        app.world().get_entity(oort_ent).is_ok(),
        "OortComet at 10,000 AU must not be culled by the 2,000 AU debris limit"
    );
}

#[test]
fn test_relativistic_binary_remains_bound_across_orbit() {
    use bevy::math::DVec3;
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::{is_body_bound_to_star, step_physics_simulation};
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{
        ActiveScenarioState, ScenarioPreset, spawn_relativistic_binary_scenario,
    };

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<PlayerInteractionState>()
        .add_systems(Update, step_physics_simulation);

    let mut disk_params = DiskParameters::default();
    let (_pulsar, companion) =
        spawn_relativistic_binary_scenario(&mut app.world_mut().commands(), &mut disk_params);
    *app.world_mut().resource_mut::<DiskParameters>() = disk_params;
    app.world_mut()
        .resource_mut::<ActiveScenarioState>()
        .current_preset = ScenarioPreset::RelativisticBinary;

    for _ in 0..150 {
        app.update();
    }

    let pos = app.world().get::<SimPosition>(companion).unwrap().0;
    let vel = app.world().get::<SimVelocity>(companion).unwrap().0;
    let m2 = app.world().get::<Mass>(companion).unwrap().0;
    assert!(
        is_body_bound_to_star(pos, vel, DVec3::ZERO, 1.44, m2),
        "Equal-mass relativistic binary (q ~ 0.96) must remain Kepler-bound"
    );
    assert!(
        pos.length() < 0.035,
        "PSR B1913+16 companion apastron must stay near ~0.021 AU, got {}",
        pos.length()
    );
}
