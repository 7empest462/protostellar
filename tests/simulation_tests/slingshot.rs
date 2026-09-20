//! Automated tests for Interactive Orbital Slingshot Launcher.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::game::slingshot::spawn_slingshot_world;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::utils::constants::*;
use protostellar::utils::math::*;

#[test]
fn test_slingshot_state_initialization_and_archetype_cycling() {
    let mut state = SlingshotState::default();
    assert!(!state.is_active);
    assert_eq!(state.archetype, SlingshotArchetype::Asteroid);
    assert!((state.velocity_scale - 8.0).abs() < 1e-6);

    // Test cycling through archetypes
    state.archetype = state.archetype.cycle();
    assert_eq!(state.archetype, SlingshotArchetype::Comet);
    assert_eq!(state.archetype.icon(), "☄️");
    assert_eq!(state.archetype.display_name(), "Comet");

    state.archetype = state.archetype.cycle();
    assert_eq!(state.archetype, SlingshotArchetype::TerrestrialPlanet);
    assert_eq!(state.archetype.icon(), "🌍");
    assert_eq!(state.archetype.display_name(), "Terrestrial Planet");

    state.archetype = state.archetype.cycle();
    assert_eq!(state.archetype, SlingshotArchetype::WaterWorld);
    assert_eq!(state.archetype.icon(), "🌊");

    state.archetype = state.archetype.cycle();
    assert_eq!(state.archetype, SlingshotArchetype::GasGiant);
    assert_eq!(state.archetype.icon(), "🪐");

    state.archetype = state.archetype.cycle();
    assert_eq!(state.archetype, SlingshotArchetype::RoguePlanet);
    assert_eq!(state.archetype.icon(), "🚀");

    state.archetype = state.archetype.cycle();
    assert_eq!(state.archetype, SlingshotArchetype::Asteroid);
    assert_eq!(state.archetype.icon(), "🪨");
}

#[test]
fn test_slingshot_keplerian_orbit_forecast_bound_and_hyperbolic() {
    let star_mass = 1.0;
    let pos = DVec3::new(1.0, 0.0, 0.0);

    // 1. Circular velocity launch v = 2*pi AU/yr along Z axis
    let v_circ = std::f64::consts::TAU;
    let vel_circ = DVec3::new(0.0, 0.0, v_circ);

    let elements_circ = state_vectors_to_orbital_elements(pos, vel_circ, star_mass, 1e-6)
        .expect("Keplerian elements must solve for circular velocity");

    assert!((elements_circ.semi_major_axis - 1.0).abs() < 0.05);
    assert!(elements_circ.eccentricity < 0.05);
    assert!(elements_circ.periapsis > 0.90 && elements_circ.periapsis < 1.10);

    // 2. Hyperbolic escape launch (v > v_escape = sqrt(2) * 2*pi ~ 8.88 AU/yr)
    let v_escape = 15.0; // AU/yr
    let vel_escape = DVec3::new(0.0, 0.0, v_escape);

    let elements_hyp = state_vectors_to_orbital_elements(pos, vel_escape, star_mass, 1e-6)
        .expect("Keplerian elements must solve for hyperbolic escape");

    assert!(elements_hyp.eccentricity > 1.0);
    assert!(elements_hyp.specific_energy > 0.0);
}

#[test]
fn test_slingshot_world_spawning_all_archetypes() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let archetypes = [
        (SlingshotArchetype::Asteroid, BodyType::Asteroid, false),
        (SlingshotArchetype::Comet, BodyType::Comet, true),
        (
            SlingshotArchetype::TerrestrialPlanet,
            BodyType::TerrestrialPlanet,
            false,
        ),
        (SlingshotArchetype::WaterWorld, BodyType::SuperEarth, false),
        (SlingshotArchetype::GasGiant, BodyType::GasGiant, false),
        (SlingshotArchetype::RoguePlanet, BodyType::GasGiant, false),
    ];

    for (archetype, expected_body_type, expected_tail) in archetypes {
        let origin = DVec3::new(1.5, 0.0, 0.0);
        let launch_vel = DVec3::new(0.0, 0.0, 4.5);

        let entity = spawn_slingshot_world(
            &mut app.world_mut().commands(),
            archetype,
            origin,
            launch_vel,
            1.0,
        );

        app.update();

        let world = app.world();
        let entity_ref = world.entity(entity);

        assert!(entity_ref.contains::<SimPosition>());
        assert!(entity_ref.contains::<SimVelocity>());
        assert!(entity_ref.contains::<Mass>());
        assert!(entity_ref.contains::<Radius>());
        assert!(entity_ref.contains::<CelestialBody>());
        assert!(entity_ref.contains::<Composition>());

        let body = entity_ref.get::<CelestialBody>().unwrap();
        assert_eq!(body.body_type, expected_body_type);

        let pos = entity_ref.get::<SimPosition>().unwrap();
        assert_eq!(pos.0, origin);

        let vel = entity_ref.get::<SimVelocity>().unwrap();
        assert_eq!(vel.0, launch_vel);

        let tail = entity_ref.get::<AtmosphericEscapeTail>();
        assert_eq!(tail.is_some(), expected_tail);
    }
}

#[test]
fn test_slingshot_impulse_application_to_existing_body() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let target_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 6.0)),
            SimAcceleration(DVec3::ZERO),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            CelestialBody {
                name: "Target Planet".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
        ))
        .id();

    // Verify initial velocity
    let initial_vel = app.world().get::<SimVelocity>(target_ent).unwrap().0;
    assert_eq!(initial_vel, DVec3::new(0.0, 0.0, 6.0));

    // Apply slingshot velocity impulse
    let delta_v = DVec3::new(1.0, 0.0, 2.5);
    let mut vel = app.world_mut().get_mut::<SimVelocity>(target_ent).unwrap();
    vel.0 += delta_v;

    let updated_vel = app.world().get::<SimVelocity>(target_ent).unwrap().0;
    assert_eq!(updated_vel, DVec3::new(1.0, 0.0, 8.5));
}

#[test]
fn test_slingshot_placement_in_inner_system_does_not_snap_to_sun_or_planets() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let star = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Mass(1.0),
            Radius(0.00465),
            CelestialBody {
                name: "Sun".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            CentralStar,
        ))
        .id();

    let earth = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
        ))
        .id();

    let test_points = [
        (DVec3::new(0.3, 0.0, 0.0), None),
        (DVec3::new(0.7, 0.0, 0.0), None),
        (DVec3::new(1.2, 0.0, 0.0), None),
        (DVec3::new(1.5, 0.0, 0.0), None),
        // Direct click on Earth within 0.03 AU targets Earth
        (DVec3::new(1.01, 0.0, 0.0), Some(earth)),
        // Direct click on Sun within 0.005 AU targets Sun
        (DVec3::new(0.005, 0.0, 0.0), Some(star)),
    ];

    let mut query = app
        .world_mut()
        .query::<(Entity, &SimPosition, &Radius, &CelestialBody)>();

    for (click_pt, expected_target) in test_points {
        let mut nearest: Option<Entity> = None;
        let mut min_dist = 0.05f64;

        for (ent, pos, rad, _body) in query.iter(app.world()) {
            let hit_radius = (rad.0 * 1.5).max(0.02).min(0.05);
            let d = (pos.0 - click_pt).length();
            if d < hit_radius && d < min_dist {
                min_dist = d;
                nearest = Some(ent);
            }
        }

        assert_eq!(
            nearest, expected_target,
            "Click at {:?} should resolve to {:?}, got {:?}",
            click_pt, expected_target, nearest
        );
    }
}

#[test]
fn test_slingshot_circular_velocity_and_drag_distance_calibration() {
    let star_mass = 1.0;
    let r_launch = 1.0; // 1 AU
    let v_scale = 8.0;

    // Circular velocity v_circ = sqrt(G * M / r)
    // With G_ASTRO = 4*pi^2, for M = 1, r = 1: v_circ = 2*pi AU/yr
    let v_circ_au_yr = (G_ASTRO * star_mass / r_launch).sqrt();
    assert!((v_circ_au_yr - std::f64::consts::TAU).abs() < 1e-5);

    let v_circ_kms = v_circ_au_yr * AU_PER_YR_TO_KM_PER_S;
    assert!((v_circ_kms - 29.78).abs() < 0.2);

    // Drag distance to achieve circular velocity
    let d_circ = v_circ_au_yr / v_scale;
    assert!((d_circ - (std::f64::consts::TAU / 8.0)).abs() < 1e-5);

    // Escape drag distance d_esc = d_circ * sqrt(2)
    let d_esc = d_circ * std::f64::consts::SQRT_2;
    let v_esc_au_yr = d_esc * v_scale;
    assert!((v_esc_au_yr - std::f64::consts::TAU * std::f64::consts::SQRT_2).abs() < 1e-5);
}
