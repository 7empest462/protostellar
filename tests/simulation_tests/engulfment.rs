//! Tests for stellar engulfment and close planet-planet collision mergers.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
use protostellar::simulation::accretion::events::{
    AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::thermodynamics::{update_thermodynamics, StarIgnitionEvent};
use protostellar::utils::constants::*;

#[test]
fn test_planet_entering_star_photosphere_is_devoured_by_accretion() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<MoonFormationEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_systems(Update, process_accretion_and_collisions);

    let star_mass = 1.0;
    let star_rad = 0.00465; // AU
    let star_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Mass(star_mass),
            Radius(star_rad),
            Temperature(5778.0),
            Composition::default(),
            CentralStar,
            CelestialBody {
                name: "The Sun".to_string(),
                body_type: BodyType::YellowDwarf,
            },
        ))
        .id();

    // Planet spawned at 0.020 AU (inside the Sun's visual photosphere of 0.025 AU, but outside physical 0.00465 AU)
    let planet_mass = 1.0 * EARTH_MASS_SOLAR;
    let planet_rad = EARTH_RADIUS_AU;
    let planet_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(0.020, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 0.0)),
            SimAcceleration(DVec3::ZERO),
            Mass(planet_mass),
            Radius(planet_rad),
            Temperature(300.0),
            Composition::rocky(),
            CelestialBody {
                name: "Falling Embryo".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
        ))
        .id();

    app.update();

    let merge_events = app.world().resource::<Messages<AccretionMergeEvent>>();
    assert_eq!(
        merge_events.len(),
        1,
        "Body penetrating inside the star's photosphere must be devoured via AccretionMergeEvent"
    );

    // Planet must be despawned
    assert!(
        app.world().get_entity(planet_ent).is_err(),
        "Devoured planet must be despawned from the ECS"
    );

    // Star mass must increase by planet's mass
    let star_m = app.world().get::<Mass>(star_ent).unwrap().0;
    assert!(
        (star_m - (star_mass + planet_mass)).abs() < 1e-10,
        "Star must absorb the devoured planet's mass"
    );
}

#[test]
fn test_planets_passing_halfway_through_each_other_trigger_collision_damage_and_merge() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<MoonFormationEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_systems(Update, process_accretion_and_collisions);

    // Two Earth-sized planets (visual radius ~0.0029 AU each, combined visual radius ~0.0058 AU)
    // Placed at distance 0.0020 AU (more than halfway through each other!)
    let p1 = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            SimAcceleration(DVec3::ZERO),
            Mass(1.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Temperature(300.0),
            Composition::rocky(),
            CelestialBody {
                name: "Planet Alpha".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
        ))
        .id();

    let p2 = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.0020, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            SimAcceleration(DVec3::ZERO),
            Mass(0.8 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.93),
            Temperature(300.0),
            Composition::rocky(),
            CelestialBody {
                name: "Planet Beta".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
        ))
        .id();

    app.update();

    let merge_events = app.world().resource::<Messages<AccretionMergeEvent>>();
    let bounce_events = app.world().resource::<Messages<CollisionBounceEvent>>();

    // The collision MUST be detected and resolved (either merge or bounce) — never ghost through each other
    let collision_occurred = merge_events.len() > 0 || bounce_events.len() > 0;
    assert!(
        collision_occurred,
        "Planets passing halfway through each other visually must collide and cause damage"
    );

    // If merged, one body is absorbed into the primary, resulting in 1 surviving entity with combined mass
    if merge_events.len() > 0 {
        assert!(
            app.world().get_entity(p2).is_err(),
            "Secondary planet must be merged away"
        );
        let m1 = app.world().get::<Mass>(p1).unwrap().0;
        assert!(
            (m1 - 1.8 * EARTH_MASS_SOLAR).abs() < 1e-10,
            "Merged planet must have combined mass (got {m1})"
        );
        let t1 = app.world().get::<Temperature>(p1).unwrap().0;
        assert!(
            t1 > 300.0,
            "Merged planet must experience thermal shock heating (got {t1} K)"
        );
    }
}

#[test]
fn test_sun_grazing_planet_takes_extreme_thermal_damage_and_is_engulfed() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .add_message::<PlanetaryEngulfmentEvent>()
        .add_message::<SupernovaEvent>()
        .add_message::<StarIgnitionEvent>()
        .add_systems(Update, update_thermodynamics);

    // Spawn central star (The Sun)
    let star_rad = SOLAR_RADIUS_AU; // 0.00465 AU
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(star_rad),
        Temperature(5778.0),
        Luminosity(1.0),
        Composition::solar_gas(),
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 0.0,
        },
        StellarEvolutionState::default(),
    ));

    // Planet spawned at 0.0030 AU (deep inside the Sun's physical radius of 0.00465 AU)
    let planet_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(0.0030, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 10.0)),
            SimAcceleration(DVec3::ZERO),
            Mass(1.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Temperature(300.0),
            Composition::rocky(),
            CelestialBody {
                name: "Deep Plunging Planet".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
        ))
        .id();

    app.update();

    let engulf_events = app.world().resource::<Messages<PlanetaryEngulfmentEvent>>();
    assert_eq!(
        engulf_events.len(),
        1,
        "Planet orbiting inside the star must be engulfed via PlanetaryEngulfmentEvent"
    );

    // The plunging planet must be despawned from the ECS
    assert!(
        app.world().get_entity(planet_ent).is_err(),
        "Engulfed planet must be despawned from the ECS"
    );
}
