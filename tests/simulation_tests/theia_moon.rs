//! Tests for Theia-Earth Moon Formation and collision regimes.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
use protostellar::simulation::accretion::events::{
    AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
};
use protostellar::simulation::accretion::theia::{update_theia_rendezvous, TheiaImpactState};
use protostellar::simulation::components::*;
use protostellar::simulation::disk::planetesimals::auto_spawn_delayed_proto_earth;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use protostellar::utils::constants::*;

fn setup_theia_precision_test_app() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<TheiaImpactState>()
        .add_systems(Update, update_theia_rendezvous);

    // Spawn central star
    let star_pos = DVec3::ZERO;
    let star_mass = 1.0;
    app.world_mut().spawn((
        SimPosition(star_pos),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(star_mass),
        Radius(0.00465),
        CentralStar,
    ));

    // Spawn Proto-Earth at 1.00 AU
    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let v_circ = (G_ASTRO * star_mass / 1.0).sqrt();
    let earth_vel = DVec3::new(0.0, 0.0, v_circ);
    let earth_mass = 0.88 * EARTH_MASS_SOLAR;
    let earth_rad = EARTH_RADIUS_AU * 0.94;
    let comp = Composition::rocky();
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(earth_mass, earth_rad, &comp);

    let earth_ent = app
        .world_mut()
        .spawn((
            SimPosition(earth_pos),
            SimVelocity(earth_vel),
            SimAcceleration(DVec3::ZERO),
            Mass(earth_mass),
            Radius(earth_rad),
            Temperature(288.0),
            comp.clone(),
            diff,
            CelestialBody {
                name: "Proto-Earth".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState {
                spin_vector: DVec3::new(0.0, 1e-12, 0.0),
                rotation_period_hours: 24.0,
                axial_tilt_degrees: 0.0,
            },
        ))
        .id();

    // Spawn Theia at 1.15 AU
    let theia_pos = DVec3::new(1.10, 0.0, 0.08);
    let theia_vel = DVec3::new(0.0, 0.0, v_circ * 0.95);
    let theia_mass = 0.12 * EARTH_MASS_SOLAR;
    let theia_rad = EARTH_RADIUS_AU * 0.53;
    let mut theia_diff = InternalDifferentiation::default();
    theia_diff.recalculate(theia_mass, theia_rad, &comp);

    let theia_ent = app
        .world_mut()
        .spawn((
            SimPosition(theia_pos),
            SimVelocity(theia_vel),
            SimAcceleration(DVec3::ZERO),
            Mass(theia_mass),
            Radius(theia_rad),
            Temperature(270.0),
            comp,
            theia_diff,
            CelestialBody {
                name: "Theia".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    (app, earth_ent, theia_ent)
}

#[test]
fn test_theia_precision_rendezvous_and_moon_formation() {
    let (mut app, earth_ent, _theia_ent) = setup_theia_precision_test_app();

    // At T = 0 yr, intercept should be idle
    app.update();
    let state = app.world().resource::<TheiaImpactState>();
    assert!(!state.intercept_active);
    assert!(!state.moon_formed);

    // Advance to T = 60 yr (inside the automatic T ~ 50-100 yr window)
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 60.0;
        sim_time.current_dt_yr = 0.05; // large time warp step
    }

    // Step the simulation
    app.update();
    let state = app.world().resource::<TheiaImpactState>();
    assert!(
        state.intercept_active || state.moon_formed,
        "Intercept guidance must activate at T = 60 yr"
    );

    // Run additional steps to allow trajectory rendezvous to execute giant impact
    for _ in 0..10 {
        app.world_mut().resource_mut::<SimTime>().elapsed_years += 0.05;
        app.update();
    }

    let final_state = app.world().resource::<TheiaImpactState>();
    assert!(
        final_state.moon_formed,
        "The Moon must be formed via giant impact collision"
    );

    // Verify Earth state
    let (earth_body, earth_diff, earth_spin) = app
        .world_mut()
        .query::<(&CelestialBody, &InternalDifferentiation, Option<&SpinState>)>()
        .get(app.world(), earth_ent)
        .expect("Earth entity must exist");

    assert_eq!(earth_body.name, "Earth");
    assert_eq!(earth_body.body_type, BodyType::TerrestrialPlanet);
    assert!(
        earth_diff.has_theia_llsvp,
        "Earth mantle must be enriched with Theia basal LLSVP remnants"
    );
    assert_eq!(earth_diff.llsvp_density_contrast, 0.028);
    if let Some(spin) = earth_spin {
        assert!(
            spin.rotation_period_hours <= 12.0,
            "Giant impact must spin up Earth"
        );
    }

    // Verify Moon state
    let mut moon_found = false;
    let mut moon_query = app
        .world_mut()
        .query::<(&CelestialBody, &Mass, Option<&SatelliteOf>)>();
    for (body, mass, sat) in moon_query.iter(app.world()) {
        if body.name == "The Moon" {
            moon_found = true;
            assert_eq!(body.body_type, BodyType::Moon);
            assert!(
                mass.0 > 0.010 * EARTH_MASS_SOLAR && mass.0 < 0.015 * EARTH_MASS_SOLAR,
                "Moon mass ({:.4} M_earth) must match canonical lunar mass (~0.0123 M_earth)",
                mass.0 / EARTH_MASS_SOLAR
            );
            assert!(
                sat.is_some(),
                "The Moon must have SatelliteOf orbiting Earth"
            );
            let sat_of = sat.unwrap();
            assert_eq!(sat_of.parent, earth_ent);
            assert!(
                sat_of.semi_major_axis_au > 0.005 && sat_of.semi_major_axis_au < 0.015,
                "Lunar orbital radius must be stable outside visual mesh (found {})",
                sat_of.semi_major_axis_au
            );
        }
    }
    assert!(moon_found, "The Moon entity must exist in the world");
}

#[test]
fn test_theia_on_demand_trigger_and_auto_spawn() {
    let mut app = App::new();
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<TheiaImpactState>()
        .add_systems(Update, update_theia_rendezvous);

    // Spawn central star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CentralStar,
    ));

    // Set on-demand trigger at T = 5.0 yr (outside automatic window)
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 5.0;
        sim_time.current_dt_yr = 0.02;
        let mut state = app.world_mut().resource_mut::<TheiaImpactState>();
        state.manual_trigger_requested = true;
    }

    // Run steps: Theia and Earth are auto-spawned and guided to giant impact
    for _ in 0..15 {
        app.world_mut().resource_mut::<SimTime>().elapsed_years += 0.05;
        app.update();
    }

    let final_state = app.world().resource::<TheiaImpactState>();
    assert!(
        final_state.moon_formed,
        "On-demand trigger must reliably form The Moon"
    );

    let mut bodies = app.world_mut().query::<&CelestialBody>();
    let moon_count = bodies
        .iter(app.world())
        .filter(|b| b.name == "The Moon")
        .count();
    let earth_count = bodies
        .iter(app.world())
        .filter(|b| b.name == "Earth")
        .count();
    assert_eq!(moon_count, 1, "Exactly one Moon must be created");
    assert_eq!(earth_count, 1, "Exactly one Earth must exist");
}

#[test]
fn test_distant_theia_at_156_au_recovers_and_forms_moon() {
    let mut app = App::new();
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<TheiaImpactState>()
        .add_systems(Update, update_theia_rendezvous);

    // Spawn central star
    let star_mass = 1.0;
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(star_mass),
        Radius(0.00465),
        CentralStar,
    ));

    // Spawn Proto-Earth at 1.00 AU
    let earth_mass = 0.88 * EARTH_MASS_SOLAR;
    let earth_rad = EARTH_RADIUS_AU * 0.94;
    let comp = Composition::rocky();
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(earth_mass, earth_rad, &comp);

    let earth_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            SimAcceleration(DVec3::ZERO),
            Mass(earth_mass),
            Radius(earth_rad),
            Temperature(288.0),
            comp,
            diff,
            CelestialBody {
                name: "Proto-Earth".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // Spawn Theia at 156.77 AU (simulating the elongated orbit from a previous bounce)
    let theia_mass = 0.28 * EARTH_MASS_SOLAR;
    let theia_rad = EARTH_RADIUS_AU * 0.65;
    let mut theia_diff = InternalDifferentiation::default();
    theia_diff.recalculate(theia_mass, theia_rad, &comp);

    app.world_mut().spawn((
        SimPosition(DVec3::new(156.77, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, 0.65)),
        SimAcceleration(DVec3::ZERO),
        Mass(theia_mass),
        Radius(theia_rad),
        Temperature(50.0),
        comp,
        theia_diff,
        CelestialBody {
            name: "Theia".to_string(),
            body_type: BodyType::Protoplanet,
        },
        VolatileInventory::default(),
        SpinState::default(),
    ));

    // Time is T = 50.64 yr (matching user test)
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 50.64;
        sim_time.current_dt_yr = 0.05;
    }

    // Step 1: Recovers Theia from 156 AU to co-orbital intercept
    app.update();

    // Step 2..6: Completes rendezvous and executes Moon formation
    for _ in 0..10 {
        app.world_mut().resource_mut::<SimTime>().elapsed_years += 0.05;
        app.update();
    }

    let theia_state = app.world().resource::<TheiaImpactState>();
    assert!(
        theia_state.moon_formed,
        "Theia must recover from distant orbit and form The Moon"
    );

    // Verify Earth is renamed from Proto-Earth to Earth
    let earth_body = app
        .world_mut()
        .query::<&CelestialBody>()
        .get(app.world(), earth_ent)
        .unwrap();
    assert_eq!(earth_body.name, "Earth");
}

#[test]
fn test_theia_earth_collision_in_accretion_never_bounces() {
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

    // Star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CentralStar,
    ));

    // Proto-Earth
    let earth_mass = 0.88 * EARTH_MASS_SOLAR;
    let earth_rad = EARTH_RADIUS_AU * 1.0;
    let comp = Composition::rocky();
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(earth_mass, earth_rad, &comp);

    app.world_mut().spawn((
        SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
        SimAcceleration(DVec3::ZERO),
        Mass(earth_mass),
        Radius(earth_rad),
        Temperature(288.0),
        comp,
        diff,
        CelestialBody {
            name: "Proto-Earth".to_string(),
            body_type: BodyType::Protoplanet,
        },
        VolatileInventory::default(),
        SpinState::default(),
    ));

    // Theia at glancing contact with high relative velocity (previously would HitAndRun bounce)
    let theia_mass = 0.28 * EARTH_MASS_SOLAR;
    let theia_rad = EARTH_RADIUS_AU * 0.65;
    let mut theia_diff = InternalDifferentiation::default();
    theia_diff.recalculate(theia_mass, theia_rad, &comp);

    app.world_mut().spawn((
        SimPosition(DVec3::new(1.0 + earth_rad * 0.8, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, 9.5)), // High relative speed
        SimAcceleration(DVec3::ZERO),
        Mass(theia_mass),
        Radius(theia_rad),
        Temperature(300.0),
        comp,
        theia_diff,
        CelestialBody {
            name: "Theia".to_string(),
            body_type: BodyType::Protoplanet,
        },
        VolatileInventory::default(),
        SpinState::default(),
    ));

    app.update();

    let bounce_events = app.world().resource::<Messages<CollisionBounceEvent>>();
    assert_eq!(
        bounce_events.len(),
        0,
        "Theia and Proto-Earth must NEVER bounce in collisions"
    );

    let moon_events = app.world().resource::<Messages<MoonFormationEvent>>();
    assert_eq!(
        moon_events.len(),
        1,
        "Theia and Proto-Earth collision must unconditionally emit MoonFormationEvent"
    );
}

fn spawn_test_moon(
    world: &mut World,
    parent: Entity,
    name: &str,
    mass: f64,
    radius: f64,
    pos: DVec3,
    vel: DVec3,
    semi_major_axis_au: f64,
    orbital_period_years: f64,
) -> Entity {
    let comp = Composition::rocky();
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(mass, radius, &comp);
    world
        .spawn((
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration(DVec3::ZERO),
            Mass(mass),
            Radius(radius),
            Temperature(250.0),
            comp,
            diff,
            CelestialBody {
                name: name.to_string(),
                body_type: BodyType::Moon,
            },
            SatelliteOf {
                parent,
                semi_major_axis_au,
                orbital_period_years,
                true_anomaly: 0.0,
            },
        ))
        .id()
}

#[test]
fn test_sibling_moons_orbiting_same_planet_collide_and_merge_when_crossing_paths() {
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

    // Star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CentralStar,
    ));

    // Earth
    let earth_mass = 1.0 * EARTH_MASS_SOLAR;
    let earth_rad = EARTH_RADIUS_AU * 1.0;
    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let v_circ = (G_ASTRO * 1.0 / 1.0).sqrt();
    let earth_vel = DVec3::new(0.0, 0.0, v_circ);
    let comp = Composition::rocky();
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(earth_mass, earth_rad, &comp);

    let earth_ent = app
        .world_mut()
        .spawn((
            SimPosition(earth_pos),
            SimVelocity(earth_vel),
            SimAcceleration(DVec3::ZERO),
            Mass(earth_mass),
            Radius(earth_rad),
            Temperature(288.0),
            comp,
            diff,
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // Moon 1: "The Moon" at 0.0075 AU
    let moon1_mass = 0.0123 * EARTH_MASS_SOLAR;
    let moon1_rad = EARTH_RADIUS_AU * 0.272;
    let moon1_pos = earth_pos + DVec3::new(0.0075, 0.0, 0.0);
    let moon1_vel = earth_vel + DVec3::new(0.0, 0.0, 0.2);
    let moon1_ent = spawn_test_moon(
        app.world_mut(),
        earth_ent,
        "The Moon",
        moon1_mass,
        moon1_rad,
        moon1_pos,
        moon1_vel,
        0.0075,
        0.074,
    );

    // Moon 2: "Earth I (Moon)" at 0.0080 AU (distance 0.0005 AU, within visual contact radius ~0.0036 AU)
    let moon2_mass = 0.0050 * EARTH_MASS_SOLAR;
    let moon2_rad = EARTH_RADIUS_AU * 0.20;
    let moon2_pos = earth_pos + DVec3::new(0.0080, 0.0, 0.0);
    let moon2_vel = earth_vel + DVec3::new(0.0, 0.0, 0.2);
    let moon2_ent = spawn_test_moon(
        app.world_mut(),
        earth_ent,
        "Earth I (Moon)",
        moon2_mass,
        moon2_rad,
        moon2_pos,
        moon2_vel,
        0.0080,
        0.078,
    );

    app.update();

    let merge_events = app.world().resource::<Messages<AccretionMergeEvent>>();
    assert_eq!(
        merge_events.len(),
        1,
        "Sibling moons crossing paths within visual contact must merge instead of ghosting through each other"
    );

    let surviving_query = app
        .world_mut()
        .query::<(&CelestialBody, Option<&SatelliteOf>, &Mass)>()
        .iter(app.world())
        .collect::<Vec<_>>();

    let moons = surviving_query
        .iter()
        .filter(|(b, ..)| b.body_type == BodyType::Moon)
        .collect::<Vec<_>>();

    assert_eq!(
        moons.len(),
        1,
        "Exactly one moon must remain after inelastic merger"
    );

    assert!(
        app.world().get_entity(moon1_ent).is_ok(),
        "Primary moon (moon1) must remain alive"
    );
    assert!(
        app.world().get_entity(moon2_ent).is_err(),
        "Secondary moon (moon2) must be despawned"
    );

    let (surviving_body, opt_sat, surviving_mass) = moons[0];
    assert_eq!(
        surviving_body.body_type,
        BodyType::Moon,
        "Surviving moon must retain BodyType::Moon"
    );
    assert!(
        opt_sat.is_some(),
        "Surviving moon must retain its SatelliteOf component"
    );
    assert_eq!(
        opt_sat.unwrap().parent,
        earth_ent,
        "Surviving moon's SatelliteOf parent must point to Earth"
    );
    assert!(
        (surviving_mass.0 - (moon1_mass + moon2_mass)).abs() < 1e-12,
        "Surviving moon must have the combined mass of both moons"
    );
}

#[test]
fn test_trappist_scenario_does_not_auto_spawn_proto_earth_or_theia() {
    let mut app = App::new();
    app.init_resource::<TimeWarp>()
        .insert_resource(SimTime {
            elapsed_years: 60.0,
            ..default()
        })
        .insert_resource(DiskParameters {
            central_star_mass: 0.0898,
            outer_radius_au: 0.1,
            ..default()
        })
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::Trappist1System,
            ..default()
        })
        .init_resource::<TheiaImpactState>()
        .add_systems(
            Update,
            (auto_spawn_delayed_proto_earth, update_theia_rendezvous),
        );

    // Spawn TRAPPIST-1 central star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(0.0898),
        Radius(0.00056),
        CentralStar,
    ));

    app.update();

    let bodies = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .collect::<Vec<_>>();

    assert!(
        !bodies.iter().any(|b| b.name.contains("Earth")),
        "TRAPPIST-1 scenario must never auto-spawn Proto-Earth"
    );
    assert!(
        !bodies.iter().any(|b| b.name.contains("Theia")),
        "TRAPPIST-1 scenario must never auto-spawn Theia"
    );
    assert!(
        !bodies.iter().any(|b| b.name.contains("Moon")),
        "TRAPPIST-1 scenario must never auto-spawn The Moon"
    );
}

#[test]
fn test_the_moon_retains_spherical_planet_mesh_under_gas_and_pebble_accretion() {
    use protostellar::rendering::bodies::meshes::select_body_mesh;
    use protostellar::rendering::bodies::VisualAssets;
    use protostellar::simulation::accretion::gas::direct_nebular_gas_accretion;
    use protostellar::simulation::pebble_accretion::apply_pebble_accretion;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_resource::<Assets<Mesh>>();

    let (planet_mesh, asteroid_mesh) = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        (
            meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        )
    };

    let mut visual_assets = VisualAssets::dummy(asteroid_mesh.clone());
    visual_assets.planet_mesh = planet_mesh.clone();
    app.insert_resource(visual_assets);

    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .add_systems(
            Update,
            (direct_nebular_gas_accretion, apply_pebble_accretion),
        );

    // Spawn central star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CentralStar,
        IgnitionState {
            is_ignited: true,
            fusion_fraction: 1.0,
            core_temperature: 15_000_000.0,
            shockwave_radius: 0.0,
        },
    ));

    // Spawn Earth
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            SimAcceleration(DVec3::ZERO),
        ))
        .id();

    // Spawn The Moon (mass: 0.0123 M_earth, orbit ~0.0025 AU around Earth)
    let moon_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "The Moon".to_string(),
                body_type: BodyType::Moon,
            },
            Mass(0.0123 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.272),
            Composition::rocky(),
            SimPosition(DVec3::new(1.0025, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU + 0.2)),
            SimAcceleration(DVec3::ZERO),
            SatelliteOf {
                parent: earth_ent,
                semi_major_axis_au: 0.0025,
                orbital_period_years: 0.0748,
                true_anomaly: 0.0,
            },
        ))
        .id();

    // Step simulation
    app.update();

    let moon_body = app.world().get::<CelestialBody>(moon_ent).unwrap();
    assert_eq!(
        moon_body.body_type,
        BodyType::Moon,
        "The Moon must retain BodyType::Moon and not be demoted to Planetesimal or Asteroid"
    );
    assert_eq!(
        moon_body.name, "The Moon",
        "The Moon must retain its canonical name"
    );

    // Mesh selection check: must choose the spherical planet mesh, NOT an irregular asteroid or contact-binary comet mesh
    let vis = app.world().resource::<VisualAssets>();
    let selected_mesh = select_body_mesh(moon_body, vis);
    assert_eq!(
        selected_mesh, planet_mesh,
        "The Moon must be assigned the spherical planet_mesh, not a comet or asteroid mesh"
    );

    // Even if named 'Earth I (Moon)', it must still select planet_mesh
    let sibling_moon = CelestialBody {
        name: "Earth I (Moon)".to_string(),
        body_type: BodyType::Moon,
    };
    assert_eq!(
        select_body_mesh(&sibling_moon, vis),
        planet_mesh,
        "Sibling moons must always select planet_mesh"
    );
}

