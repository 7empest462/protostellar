//! Automated integration tests for the Impact & Close-Encounter Trajectory Predictor.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::TAU;

use protostellar::simulation::components::*;
use protostellar::simulation::predictor::*;
use protostellar::simulation::resources::*;
use protostellar::utils::constants::*;
use protostellar::utils::math::*;

#[test]
fn test_kepler_propagation_accuracy_circular_orbit() {
    let star_mass = 1.0;
    let r_init = DVec3::new(1.0, 0.0, 0.0);
    let v_init = DVec3::new(0.0, 0.0, TAU); // Circular velocity at 1 AU is 2*pi AU/yr
    let elements = state_vectors_to_orbital_elements(r_init, v_init, star_mass, 1e-6)
        .expect("Orbital elements should be valid for circular orbit");

    // Period of 1 AU orbit around 1 M_sun is 1.0 year
    // At t = 0.25 yr (quarter period), position should be at (0, 0, 1.0)
    let (pos_quarter, vel_quarter) = propagate_kepler_position_velocity(&elements, 0.25, star_mass)
        .expect("quarter orbit propagation");
    assert!(
        (pos_quarter.x).abs() < 1e-3,
        "pos_quarter.x was {}",
        pos_quarter.x
    );
    assert!(
        (pos_quarter.z - 1.0).abs() < 1e-3,
        "pos_quarter.z was {}",
        pos_quarter.z
    );
    assert!(
        (vel_quarter.x + TAU).abs() < 1e-2,
        "vel_quarter.x was {}",
        vel_quarter.x
    );

    // At t = 0.50 yr (half period), position should be at (-1.0, 0, 0)
    let (pos_half, _) = propagate_kepler_position_velocity(&elements, 0.50, star_mass)
        .expect("half orbit propagation");
    assert!(
        (pos_half.x + 1.0).abs() < 1e-3,
        "pos_half.x was {}",
        pos_half.x
    );
    assert!((pos_half.z).abs() < 1e-3, "pos_half.z was {}", pos_half.z);

    // At t = 1.0 yr (full period), position should return to (1.0, 0, 0)
    let (pos_full, _) = propagate_kepler_position_velocity(&elements, 1.00, star_mass)
        .expect("full orbit propagation");
    assert!(
        (pos_full.x - 1.0).abs() < 1e-3,
        "pos_full.x was {}",
        pos_full.x
    );
    assert!((pos_full.z).abs() < 1e-3, "pos_full.z was {}", pos_full.z);
}

#[test]
fn test_kepler_equation_solver_eccentric_convergence() {
    let e = 0.6;
    for step in 0..10 {
        let m = (step as f64) * 0.1 * TAU;
        let big_e = solve_kepler_eccentric(m, e);
        let residual = (big_e - e * big_e.sin() - m).abs();
        assert!(
            residual < 1e-10,
            "Kepler equation did not converge: residual {} for M = {}",
            residual,
            m
        );
    }
}

#[test]
fn test_direct_impact_detection_encounter_classification() {
    let star_mass = 1.0;
    // Target: Earth at 1.0 AU circular orbit
    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let earth_vel = DVec3::new(0.0, 0.0, TAU);
    let earth_elements = state_vectors_to_orbital_elements(earth_pos, earth_vel, star_mass, 1e-6)
        .expect("Earth orbital elements");

    // Earth state at t = 0.25 yr
    let (target_future_pos, target_future_vel) =
        propagate_kepler_position_velocity(&earth_elements, 0.25, star_mass)
            .expect("target future position");

    // Impactor has slightly different velocity at intercept point
    let impactor_intercept_vel = target_future_vel + DVec3::new(0.3, 0.0, -0.4);
    let intercept_elements = state_vectors_to_orbital_elements(
        target_future_pos,
        impactor_intercept_vel,
        star_mass,
        1e-6,
    )
    .expect("Intercept elements");

    // Propagate impactor backwards to t = 0 to get initial state
    let (impactor_pos_0, impactor_vel_0) =
        propagate_kepler_position_velocity(&intercept_elements, -0.25, star_mass)
            .expect("impactor initial state");

    let impactor_elements =
        state_vectors_to_orbital_elements(impactor_pos_0, impactor_vel_0, star_mass, 1e-6)
            .expect("Impactor elements at t=0");

    let earth_mass = EARTH_MASS_SOLAR;
    let earth_radius_au = 6371.0 / AU_TO_KM;

    let targets = vec![TargetCandidate {
        entity: Entity::from_raw_u32(1).unwrap(),
        name: "Earth".to_string(),
        elements: earth_elements,
        radius_au: earth_radius_au,
        mass_solar: earth_mass,
    }];

    let (_, opt_encounter) = find_closest_encounter(
        &impactor_elements,
        earth_radius_au * 0.5,
        star_mass,
        &targets,
        1.0, // 1 year horizon
    );

    assert!(opt_encounter.is_some(), "Encounter must be detected");
    let enc = opt_encounter.unwrap();
    assert_eq!(enc.target_name, "Earth");
    assert!((enc.time_to_encounter_yr - 0.25).abs() < 0.03);
    assert_eq!(enc.encounter_type, EncounterType::DirectImpact);
    assert!(enc.min_distance_au < earth_radius_au * 2.0);
}

#[test]
fn test_hill_sphere_and_roche_lobe_detection() {
    let star_mass = 1.0;
    let jupiter_mass = JUPITER_MASS_SOLAR;
    let jupiter_dist = 5.2;
    let jupiter_pos = DVec3::new(jupiter_dist, 0.0, 0.0);
    let jupiter_v_circ = (star_mass / jupiter_dist).sqrt() * TAU;
    let jupiter_vel = DVec3::new(0.0, 0.0, jupiter_v_circ);

    let jupiter_elements =
        state_vectors_to_orbital_elements(jupiter_pos, jupiter_vel, star_mass, 1e-6)
            .expect("Jupiter elements");

    let r_hill = jupiter_dist * (jupiter_mass / (3.0 * star_mass)).cbrt();

    // Impactor designed to pass at ~0.5 * r_hill from Jupiter at t = 0.5 yr
    let (jup_future_pos, _) =
        propagate_kepler_position_velocity(&jupiter_elements, 0.50, star_mass)
            .expect("jupiter future position");
    let flyby_pos = jup_future_pos + DVec3::new(0.0, r_hill * 0.45, 0.0);

    let impactor_init_pos = DVec3::new(5.0, 0.0, -1.0);
    let impactor_init_vel = (flyby_pos - impactor_init_pos) / 0.50;
    let impactor_elements =
        state_vectors_to_orbital_elements(impactor_init_pos, impactor_init_vel, star_mass, 1e-6)
            .expect("Flyby impactor elements");

    let jupiter_radius_au = 69911.0 / AU_TO_KM;
    let targets = vec![TargetCandidate {
        entity: Entity::from_raw_u32(2).unwrap(),
        name: "Jupiter".to_string(),
        elements: jupiter_elements,
        radius_au: jupiter_radius_au,
        mass_solar: jupiter_mass,
    }];

    let (_, opt_encounter) =
        find_closest_encounter(&impactor_elements, 0.001, star_mass, &targets, 2.0);

    assert!(
        opt_encounter.is_some(),
        "Hill sphere encounter must be detected"
    );
    let enc = opt_encounter.unwrap();
    assert_eq!(enc.target_name, "Jupiter");
    assert!(
        enc.encounter_type == EncounterType::HillSpherePenetration
            || enc.encounter_type == EncounterType::RocheLobeCrossing
    );
    assert!(enc.min_distance_au < r_hill * 1.1);
}

#[test]
fn test_slingshot_trajectory_prediction_integration() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(SimulationConfig::default());
    app.insert_resource(DiskParameters {
        central_star_mass: 1.0,
        ..default()
    });
    app.insert_resource(PlayerInteractionState::default());
    app.init_resource::<TrajectoryPredictorState>();

    // Active Slingshot setup
    let slingshot = SlingshotState {
        is_active: true,
        drag_origin: Some(DVec3::new(1.0, 0.0, 0.0)),
        drag_current: Some(DVec3::new(1.0, 0.0, -0.5)),
        velocity_scale: 8.0,
        ..default()
    };
    app.insert_resource(slingshot);

    // Spawn central star
    app.world_mut().spawn((
        CentralStar,
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CelestialBody {
            name: "The Star".to_string(),
            body_type: BodyType::Protostar,
        },
    ));

    // Spawn a target planet
    app.world_mut().spawn((
        SimPosition(DVec3::new(2.0, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, TAU / (2.0_f64).sqrt())),
        Mass(EARTH_MASS_SOLAR),
        Radius(6371.0 / AU_TO_KM),
        CelestialBody {
            name: "Target Planet".to_string(),
            body_type: BodyType::TerrestrialPlanet,
        },
    ));

    app.add_systems(Update, update_trajectory_predictor);
    app.update();

    let predictor = app.world().resource::<TrajectoryPredictorState>();
    assert!(predictor.is_enabled);
    assert!(
        predictor.trajectory_points.len() > 10,
        "Expected generated trajectory points, got {}",
        predictor.trajectory_points.len()
    );

    // First point should be near launch position (1.0, 0.0, 0.0)
    let first_pt = predictor.trajectory_points[0];
    assert!(
        (first_pt.x - 1.0).abs() < 0.1,
        "First point x was {}",
        first_pt.x
    );
}

#[test]
fn test_hyperbolic_kepler_near_asymptote_stability() {
    let star_mass = 1.0;
    // Hyperbolic orbit: e = 1.5, a = -2.0 AU
    // Asymptote is at cos(nu_inf) = -1/e = -1/1.5 = -0.66667 (nu_inf ~ 2.3005 rad)
    let e: f64 = 1.5;
    let nu_inf = (-1.0 / e).acos();
    // Test true anomalies very close to the asymptote
    for delta_nu in [1e-2, 1e-4, 1e-6] {
        let nu_near_asymptote = nu_inf - delta_nu;
        let elements = OrbitalElements {
            semi_major_axis: -2.0,
            eccentricity: e,
            true_anomaly: nu_near_asymptote,
            periapsis_dir: DVec3::X,
            semilatus_dir: DVec3::Z,
            ..Default::default()
        };

        let result = propagate_kepler_position_velocity(&elements, 0.05, star_mass);
        assert!(
            result.is_some(),
            "Propagation near asymptote should succeed"
        );
        let (pos, vel) = result.expect("propagation result");
        assert!(
            pos.is_finite(),
            "Position near asymptote must be finite, got {:?}",
            pos
        );
        assert!(
            vel.is_finite(),
            "Velocity near asymptote must be finite, got {:?}",
            vel
        );
    }
}
