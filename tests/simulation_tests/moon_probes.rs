//! Lunar probe landing and civilized probe launch integration tests.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::ActiveScenarioState;
use protostellar::utils::constants::*;

#[test]
fn test_rover_accurately_intercepts_and_lands_on_the_moon() {
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::probes::{
        FlightComputer, ProbeStatus, ProbeType, SpaceProbe, navigation::probe_navigation_system,
    };

    let mut app = App::new();
    let config = SimulationConfig {
        enable_gpu_compute: false,
        ..Default::default()
    };
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<DiskParameters>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<protostellar::game::phases::LateHeavyBombardmentState>()
        .init_resource::<ActiveScenarioState>()
        .add_systems(
            Update,
            (
                step_physics_simulation,
                probe_navigation_system.after(step_physics_simulation),
            ),
        );

    // Central star (Sun)
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::MainSequenceStar,
        },
        CentralStar,
    ));

    // Earth at 1 AU
    let earth_vel = DVec3::new(0.0, 0.0, (G_ASTRO * 1.0 / 1.0_f64).sqrt());
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(earth_vel),
            SimAcceleration(DVec3::ZERO),
        ))
        .id();

    // The Moon at 0.0120 AU separation with physical 27.32-day period (0.0748 yr)
    let orbit_dist_au = 0.0120;
    let p_moon_yr = 0.0748;
    let v_moon_orb = (std::f64::consts::TAU / p_moon_yr) * orbit_dist_au;
    let moon_radius = EARTH_RADIUS_AU * 0.272;
    let mut moon_spin = SpinState::default();
    moon_spin.set_period_and_tilt(
        p_moon_yr * YEAR_SECONDS / 3600.0,
        1.54,
        0.0123 * EARTH_MASS_SOLAR,
        moon_radius,
    );
    let moon_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "The Moon".to_string(),
                body_type: BodyType::Moon,
            },
            Mass(0.0123 * EARTH_MASS_SOLAR),
            Radius(moon_radius),
            Composition::rocky(),
            SimPosition(DVec3::new(1.0 + orbit_dist_au, 0.0, 0.0)),
            SimVelocity(earth_vel + DVec3::new(0.0, 0.0, v_moon_orb)),
            SimAcceleration(DVec3::ZERO),
            moon_spin,
            SatelliteOf {
                parent: earth_ent,
                semi_major_axis_au: orbit_dist_au,
                orbital_period_years: p_moon_yr,
                true_anomaly: 0.0,
            },
        ))
        .id();

    // Spawn a Rover near Earth aimed at The Moon
    let start_pos = DVec3::new(1.001, 0.0, 0.0);
    let rover_ent = app
        .world_mut()
        .spawn((
            SpaceProbe::new(ProbeType::Rover, 1),
            FlightComputer {
                target_entity: moon_ent,
                target_name: "The Moon".to_string(),
                launch_pos: start_pos,
                distance_to_target: orbit_dist_au - 0.001,
                relative_speed: 0.8,
            },
            SimPosition(start_pos),
            SimVelocity(earth_vel + DVec3::new(0.8, 0.0, v_moon_orb)),
            SimAcceleration::default(),
            Mass(0.0),
            Radius(1e-7),
            CelestialBody {
                name: "Rover #1 (to The Moon)".to_string(),
                body_type: BodyType::Asteroid,
            },
        ))
        .id();

    // Advance simulation in small real-time-like steps until touchdown
    for _ in 0..60 {
        app.update();
    }

    let probe = app.world().get::<SpaceProbe>(rover_ent).unwrap();
    assert_eq!(
        probe.status,
        ProbeStatus::Landed,
        "Rover launched from Earth must intercept and land on The Moon instead of missing it"
    );

    let probe_pos = app.world().get::<SimPosition>(rover_ent).unwrap().0;
    let moon_pos = app.world().get::<SimPosition>(moon_ent).unwrap().0;
    let moon_vis_rad = f64::from(
        app.world()
            .resource::<SimulationConfig>()
            .calc_visual_radius_for_type(moon_radius, BodyType::Moon),
    );
    let dist_from_moon_center = (probe_pos - moon_pos).length();
    let expected_surface_dist = moon_vis_rad * 1.008;
    assert!(
        (dist_from_moon_center - expected_surface_dist).abs() < 1e-5,
        "Landed rover must sit directly on The Moon's surface (expected {expected_surface_dist:.6} AU, got {dist_from_moon_center:.6} AU)"
    );
}

#[test]
fn test_probe_launch_request_selects_civilized_origin_and_launches() {
    use protostellar::simulation::probes::{
        FlightComputer, LaunchProbeRequest, ProbeCounter, ProbeStatus, ProbesPlugin, SpaceProbe,
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .add_plugins(ProbesPlugin);

    app.world_mut().spawn((
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        CentralStar,
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
    ));

    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            SimAcceleration(DVec3::ZERO),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            BiosphereState {
                habitability_score: 0.92,
                biomass_coverage_frac: 0.70,
                oxygen_fraction: 0.21,
                emergence_year: Some(100.0),
                technosignature: 0.65,
                ..default()
            },
        ))
        .id();

    let mars_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Mars".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.52, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 5.1)),
            SimAcceleration(DVec3::ZERO),
            Mass(0.107 * EARTH_MASS_SOLAR),
            Radius(0.53 * EARTH_RADIUS_AU),
        ))
        .id();

    let req_ent = app
        .world_mut()
        .spawn(LaunchProbeRequest { target: mars_ent })
        .id();

    app.update();

    assert!(
        app.world().get_entity(req_ent).is_err(),
        "Processed LaunchProbeRequest entity must be despawned"
    );
    assert_eq!(app.world().resource::<ProbeCounter>().0, 1);

    let mut q = app
        .world_mut()
        .query::<(&SpaceProbe, &FlightComputer, &SimPosition)>();
    let (probe, computer, probe_pos) = q
        .iter(app.world())
        .next()
        .expect("LaunchProbeRequest must spawn a SpaceProbe with FlightComputer");
    assert_eq!(probe.status, ProbeStatus::Transit);
    assert_eq!(computer.target_entity, mars_ent);
    assert_eq!(computer.target_name, "Mars");
    assert!(
        (probe_pos.0.x - 1.0).abs() < 0.20,
        "Probe must launch near highest-scoring habitable origin Earth (x=1.0, got {})",
        probe_pos.0.x
    );
    assert!(
        (probe_pos.0 - DVec3::new(1.0, 0.0, 0.0)).dot(DVec3::X) > 0.0,
        "Launch offset must point outward from Earth toward Mars (target {mars_ent:?}, origin {earth_ent:?})"
    );
}
