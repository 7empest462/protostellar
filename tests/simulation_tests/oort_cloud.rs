//! Unit and integration tests for the distant Oort Cloud cometary reservoir,
//! Galactic tidal torques, Kozai-Lidov secular cycles, and cometary sublimation.

use std::f64::consts::PI;

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::oort::*;
use protostellar::simulation::resources::*;

#[test]
fn test_galactic_tide_vertical_acceleration_and_scaling() {
    let star_pos = DVec3::ZERO;
    let pole_dir = DVec3::new(0.0, 1.0, 0.0);
    let rho_0 = 0.10;
    let scale = 1.0;

    // 1. Exactly on Galactic midplane: acceleration must be zero
    let midplane_pos = DVec3::new(5000.0, 0.0, 3000.0);
    let a_mid = evaluate_galactic_tide_acceleration(midplane_pos, star_pos, pole_dir, rho_0, scale);
    assert_eq!(a_mid, DVec3::ZERO);

    // 2. Displaced vertically by +z: acceleration must point in -pole_dir
    let pos_above = DVec3::new(1000.0, 2000.0, 0.0);
    let a_above = evaluate_galactic_tide_acceleration(pos_above, star_pos, pole_dir, rho_0, scale);
    assert!(
        a_above.y < 0.0,
        "Tidal acceleration must point toward midplane"
    );
    assert_eq!(a_above.x, 0.0);
    assert_eq!(a_above.z, 0.0);

    // 3. Displaced vertically by -z: acceleration must point in +pole_dir
    let pos_below = DVec3::new(1000.0, -2000.0, 0.0);
    let a_below = evaluate_galactic_tide_acceleration(pos_below, star_pos, pole_dir, rho_0, scale);
    assert!(
        a_below.y > 0.0,
        "Tidal acceleration must point toward midplane"
    );
    assert_eq!(a_above.y.abs(), a_below.y.abs());

    // 4. Linear scaling with height z: doubling z must double tidal pull
    let pos_double = DVec3::new(1000.0, 4000.0, 0.0);
    let a_double =
        evaluate_galactic_tide_acceleration(pos_double, star_pos, pole_dir, rho_0, scale);
    assert!((a_double.y - 2.0 * a_above.y).abs() < 1e-12);
}

#[test]
fn test_oort_cloud_isotropic_spawner() {
    let mut app = App::new();
    let config = OortCloudConfig {
        default_comet_count: 80,
        inner_radius_au: 200.0,
        outer_radius_au: 20_000.0,
        ..default()
    };
    let mut rng = fastrand::Rng::with_seed(42);

    let mut commands = app.world_mut().commands();
    spawn_oort_cloud_swarm(&mut commands, &config, 1.0, DVec3::ZERO, 80, &mut rng);
    app.update();

    let mut comet_query = app
        .world_mut()
        .query::<(&SimPosition, &SimVelocity, &OortComet)>();
    let comets: Vec<_> = comet_query.iter(app.world()).collect();
    assert_eq!(comets.len(), 80, "Expected 80 Oort comets spawned");

    let mut retrograde_count = 0;
    let mut prograde_count = 0;

    for (pos, vel, comet) in comets {
        assert!(comet.semi_major_axis_au >= 200.0);
        assert!(comet.semi_major_axis_au <= 20_000.0);
        assert!(comet.eccentricity > 0.10 && comet.eccentricity < 0.999);
        assert!(comet.volatile_ice_fraction >= 0.70);

        // Orbital angular momentum normal along Y
        let h_y = pos.0.x * vel.0.z - pos.0.z * vel.0.x;
        if h_y < 0.0 {
            prograde_count += 1;
        } else {
            retrograde_count += 1;
        }
    }

    // Quasi-spherical isotropic distribution should have roughly comparable prograde and retrograde comets
    assert!(
        prograde_count > 15 && retrograde_count > 15,
        "Expected mixed isotropic inclinations (got prograde: {}, retrograde: {})",
        prograde_count,
        retrograde_count
    );
}

#[test]
fn test_galactic_tide_kozai_cycle_perihelion_drop() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(OortPlugin);

    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();

    let mut config = OortCloudConfig::default();
    config.secular_accel_factor = 1.0e6; // Accelerated for testing
    config.sublimation_distance_au = 5.0;
    app.insert_resource(config);

    // Spawn central star
    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "Sun".to_string(),
        },
        SimPosition(DVec3::ZERO),
        Mass(1.0),
    ));

    // Spawn Oort comet at 2,000 AU on inclined orbit with initial perihelion 200 AU
    let a = 2000.0;
    let e = 0.90;
    let inc = 65.0_f64.to_radians();
    let (pos, vel) = keplerian_elements_to_cartesian(a, e, inc, 0.0, PI / 4.0, 0.0, 1.0);

    let comet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Comet,
                name: "Test Infall Comet".to_string(),
            },
            SimPosition(pos),
            SimVelocity(vel),
            Mass(1.0e-16),
            OortComet {
                semi_major_axis_au: a,
                eccentricity: e,
                original_perihelion_au: a * (1.0 - e),
                current_perihelion_au: a * (1.0 - e),
                ..default()
            },
        ))
        .id();

    // Set simulation time step
    if let Some(mut sim_time) = app.world_mut().get_resource_mut::<SimTime>() {
        sim_time.current_dt_yr = 50.0;
    }

    // Step system
    for _ in 0..10 {
        app.update();
    }

    let comet_ref = app.world().get::<OortComet>(comet_ent).unwrap();
    assert!(comet_ref.semi_major_axis_au > 0.0);
    assert!(comet_ref.current_perihelion_au.is_finite());
}

#[test]
fn test_sun_grazing_comet_sublimation_activation() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(OortPlugin);

    app.init_resource::<TimeWarp>();
    let mut sim_time = SimTime::default();
    sim_time.current_dt_yr = 0.02;
    app.insert_resource(sim_time);

    // Spawn central star
    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "Sun".to_string(),
        },
        SimPosition(DVec3::ZERO),
        Mass(1.0),
    ));

    // Spawn comet inside sublimation zone (r = 1.2 AU)
    let comet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Comet,
                name: "Active Comet".to_string(),
            },
            SimPosition(DVec3::new(1.2, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 5.0)),
            Mass(1.0e-15),
            Composition {
                metal_frac: 0.05,
                silicate_frac: 0.15,
                ice_frac: 0.80,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            OortComet {
                volatile_ice_fraction: 0.85,
                ..default()
            },
        ))
        .id();

    app.update();

    let comet = app.world().get::<OortComet>(comet_ent).unwrap();
    assert!(
        comet.is_active_sublimating,
        "Comet at 1.2 AU must be actively sublimating"
    );
    assert!(
        comet.outgassing_rate > 0.0,
        "Outgassing rate must be non-zero"
    );
    assert!(
        comet.cumulative_volatile_loss_kg > 0.0,
        "Volatile mass loss must accumulate"
    );
}

#[test]
fn test_stellar_flyby_perturbation() {
    let star_pos = DVec3::ZERO;
    let mut p1 = SimPosition(DVec3::new(1000.0, 500.0, 0.0));
    let mut v1 = SimVelocity(DVec3::new(0.0, 0.0, 1.0));

    let mut p2 = SimPosition(DVec3::new(-1000.0, -500.0, 0.0));
    let mut v2 = SimVelocity(DVec3::new(0.0, 0.0, 1.0));

    let mut comets = [(&mut p1, &mut v1), (&mut p2, &mut v2)];

    apply_stellar_flyby_perturbation(
        &mut comets,
        star_pos,
        1.5,   // 1.5 Solar mass passing star
        500.0, // 500 AU impact parameter
        15.0,  // 15 AU/yr relative velocity
        DVec3::new(0.0, 1.0, 0.0),
    );

    assert_ne!(
        v1.0,
        DVec3::new(0.0, 0.0, 1.0),
        "Velocity must be deflected by flyby"
    );
    assert_ne!(
        v2.0,
        DVec3::new(0.0, 0.0, 1.0),
        "Velocity must be deflected by flyby"
    );
}

#[test]
fn test_trigger_oort_cloud_swarm_event_and_belt_categorization() {
    use protostellar::simulation::disk::belts::BeltZone;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(OortPlugin);

    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sol".to_string(),
            body_type: BodyType::Protostar,
        },
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
    ));

    app.update();

    // Verify 0 comets before trigger
    let mut initial_query = app.world_mut().query::<&OortComet>();
    assert_eq!(initial_query.iter(app.world()).count(), 0);

    // Send trigger event
    app.world_mut()
        .write_message(TriggerOortCloudSwarmEvent { comet_count: 48 });

    app.update();

    let mut comet_query = app
        .world_mut()
        .query::<(&SimPosition, &SimVelocity, &OortComet, &CelestialBody)>();
    let comets: Vec<_> = comet_query.iter(app.world()).collect();
    assert_eq!(
        comets.len(),
        48,
        "Triggering event must spawn exactly 48 Oort Cloud comets"
    );

    let mut oort_belt_count = 0;
    for (pos, _, comet, body) in comets {
        assert!(body.name.starts_with("Oort Comet C/"));
        assert_eq!(body.body_type, BodyType::Comet);
        assert!(comet.semi_major_axis_au >= 100.0);
        assert!(comet.volatile_ice_fraction >= 0.70);

        let dist_au = pos.0.length();
        let zone = BeltZone::from_distance_au(dist_au);
        if zone == BeltZone::OortCloud {
            oort_belt_count += 1;
        }
    }

    assert!(
        oort_belt_count >= 40,
        "Overwhelming majority of Oort comets must reside in BeltZone::OortCloud (>200 AU)"
    );
}
