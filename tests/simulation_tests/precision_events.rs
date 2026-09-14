//! Precision Event Tests for Theia-Earth Moon Formation & Late Heavy Bombardment.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::game::phases::LateHeavyBombardmentState;
use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
use protostellar::simulation::accretion::events::{
    AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
};
use protostellar::simulation::accretion::theia::{update_theia_rendezvous, TheiaImpactState};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::utils::constants::*;

fn setup_theia_integrated_physics_app() -> (App, Entity, Entity) {
    use protostellar::simulation::physics::step_physics_simulation;

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .init_resource::<TheiaImpactState>()
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
                update_theia_rendezvous.after(process_accretion_and_collisions),
            ),
        );

    // Star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
    ));

    // Proto-Earth at 1.0 AU
    let earth_mass = 0.88 * EARTH_MASS_SOLAR;
    let earth_rad = EARTH_RADIUS_AU * 0.94;
    let comp = Composition::rocky();
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(earth_mass, earth_rad, &comp);
    let v_circ = (G_ASTRO * 1.0 / 1.0).sqrt();

    let earth_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_circ)),
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
            SpinState::default(),
        ))
        .id();

    // Theia at 1.18 AU
    let theia_mass = 0.37 * EARTH_MASS_SOLAR;
    let theia_rad = EARTH_RADIUS_AU * 0.53;
    let mut theia_diff = InternalDifferentiation::default();
    theia_diff.recalculate(theia_mass, theia_rad, &comp);
    let v_theia = (G_ASTRO * 1.0 / 1.18).sqrt();

    let theia_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.18, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_theia)),
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
fn test_theia_rendezvous_integrated_with_physics_and_collisions() {
    let (mut app, earth_ent, theia_ent) = setup_theia_integrated_physics_app();

    // Set time to T = 60.0 (inside intercept window)
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 60.0;
        sim_time.current_dt_yr = 0.02;
    }

    // Step the simulation through intercept and post-impact orbital evolution
    for _ in 0..50 {
        app.update();
    }

    let final_state = app.world().resource::<TheiaImpactState>();
    assert!(
        final_state.moon_formed,
        "Moon must be formed during integrated physics and collision pass"
    );

    // Verify Earth entity
    let (earth_body, earth_diff, earth_spin) = app
        .world_mut()
        .query::<(&CelestialBody, &InternalDifferentiation, Option<&SpinState>)>()
        .get(app.world(), earth_ent)
        .expect("Earth entity must exist and not be despawned");

    assert_eq!(
        earth_body.name, "Earth",
        "Earth must retain its canonical name"
    );
    assert_eq!(earth_body.body_type, BodyType::TerrestrialPlanet);
    assert!(
        earth_diff.has_theia_llsvp,
        "Earth mantle must contain Theia LLSVP remnants"
    );
    if let Some(spin) = earth_spin {
        assert!(
            spin.rotation_period_hours <= 12.0,
            "Giant impact must spin Earth up"
        );
    }

    // Verify Moon entity survives and is orbiting Earth
    let (moon_body, moon_mass, moon_sat) = app
        .world_mut()
        .query::<(&CelestialBody, &Mass, Option<&SatelliteOf>)>()
        .get(app.world(), theia_ent)
        .expect("Theia entity must survive as The Moon");

    assert_eq!(moon_body.name, "The Moon");
    assert_eq!(moon_body.body_type, BodyType::Moon);
    assert!(
        moon_mass.0 > 0.010 * EARTH_MASS_SOLAR && moon_mass.0 < 0.050 * EARTH_MASS_SOLAR,
        "Moon mass must be preserved"
    );
    let sat_of = moon_sat.expect("The Moon must have SatelliteOf component");
    assert_eq!(sat_of.parent, earth_ent);
    assert!(
        sat_of.semi_major_axis_au >= 0.005,
        "Moon semi-major axis must be safely outside Earth's visual radius"
    );

    // Verify zero bounces occurred
    let bounce_events = app.world().resource::<Messages<CollisionBounceEvent>>();
    assert_eq!(
        bounce_events.len(),
        0,
        "Theia and Earth must never bounce into deep space"
    );
}

fn setup_planet_nine_test_world() -> (App, Entity, Entity, Entity) {
    let mut app = App::new();
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<TheiaImpactState>()
        .add_systems(Update, update_theia_rendezvous);

    // 1. Central Star
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

    // 2. Planet Nine at 380 AU with "(Super-Earth / Ice Giant)" in its name!
    let planet_nine_pos = DVec3::new(380.0, 0.0, 0.0);
    let v_p9 = (G_ASTRO * star_mass / 380.0).sqrt();
    let planet_nine_vel = DVec3::new(0.0, 0.0, v_p9);
    let planet_nine_ent = app
        .world_mut()
        .spawn((
            SimPosition(planet_nine_pos),
            SimVelocity(planet_nine_vel),
            SimAcceleration(DVec3::ZERO),
            Mass(10.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 3.5),
            Temperature(40.0),
            Composition::icy(),
            InternalDifferentiation::default(),
            CelestialBody {
                name: "Planet Nine (Super-Earth / Ice Giant)".to_string(),
                body_type: BodyType::IceGiant,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // 3. Proto-Earth at 1.00 AU
    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let v_c = (G_ASTRO * star_mass / 1.0).sqrt();
    let earth_vel = DVec3::new(0.0, 0.0, v_c);
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

    // 4. Theia at 1.04 AU near Earth
    let theia_pos = DVec3::new(1.03, 0.0, 0.02);
    let theia_vel = DVec3::new(0.0, 0.0, v_c * 0.98);
    let theia_mass = 0.12 * EARTH_MASS_SOLAR;
    let theia_rad = EARTH_RADIUS_AU * 0.53;
    let mut theia_diff = InternalDifferentiation::default();
    theia_diff.recalculate(theia_mass, theia_rad, &Composition::rocky());

    let theia_ent = app
        .world_mut()
        .spawn((
            SimPosition(theia_pos),
            SimVelocity(theia_vel),
            SimAcceleration(DVec3::ZERO),
            Mass(theia_mass),
            Radius(theia_rad),
            Temperature(270.0),
            Composition::rocky(),
            theia_diff,
            CelestialBody {
                name: "Theia".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    (app, planet_nine_ent, earth_ent, theia_ent)
}

#[test]
fn test_theia_intercept_with_planet_nine_at_380_au_selects_earth_and_forms_moon() {
    let (mut app, planet_nine_ent, earth_ent, theia_ent) = setup_planet_nine_test_world();

    // Trigger manual Moon-forming intercept
    app.world_mut()
        .resource_mut::<TheiaImpactState>()
        .manual_trigger_requested = true;

    // Advance 15 steps
    for _ in 0..15 {
        app.update();
    }

    // Verify Planet Nine was NOT targeted or modified
    let world = app.world();
    let p9_body = world
        .get::<CelestialBody>(planet_nine_ent)
        .expect("Planet Nine must exist");
    let p9_pos = world
        .get::<SimPosition>(planet_nine_ent)
        .expect("Planet Nine must have position")
        .0;
    let p9_sat = world.get::<SatelliteOf>(planet_nine_ent);
    assert_eq!(p9_body.name, "Planet Nine (Super-Earth / Ice Giant)");
    assert!(
        p9_pos.length() > 300.0,
        "Planet Nine must remain at ~380 AU, found at {}",
        p9_pos.length()
    );
    assert!(
        p9_sat.is_none(),
        "Planet Nine must not have acquired Theia as a satellite!"
    );

    // Verify Earth successfully formed at 1.0 AU
    let earth_body = world
        .get::<CelestialBody>(earth_ent)
        .expect("Earth entity must exist");
    let earth_pos = world
        .get::<SimPosition>(earth_ent)
        .expect("Earth must have position")
        .0;
    let earth_diff = world
        .get::<InternalDifferentiation>(earth_ent)
        .expect("Earth must have differentiation");
    assert_eq!(earth_body.name, "Earth");
    assert_eq!(earth_body.body_type, BodyType::TerrestrialPlanet);
    assert!(
        (earth_pos.length() - 1.0).abs() < 0.2,
        "Earth must remain at ~1.0 AU, found at {}",
        earth_pos.length()
    );
    assert!(earth_diff.has_theia_llsvp);

    // Verify The Moon formed and is bound to Earth (NOT out at 380 AU!)
    let moon_body = world
        .get::<CelestialBody>(theia_ent)
        .expect("The Moon entity must exist");
    let moon_pos = world
        .get::<SimPosition>(theia_ent)
        .expect("The Moon must have position")
        .0;
    let moon_sat = world.get::<SatelliteOf>(theia_ent);
    assert_eq!(moon_body.name, "The Moon");
    assert_eq!(moon_body.body_type, BodyType::Moon);
    let sat = moon_sat.expect("The Moon must be bound to Earth with SatelliteOf");
    assert_eq!(sat.parent, earth_ent);
    let moon_dist_to_earth = (moon_pos - earth_pos).length();
    assert!(
        moon_dist_to_earth < 0.05,
        "The Moon must orbit close to Earth (< 0.05 AU), found dist: {} AU",
        moon_dist_to_earth
    );
}

struct FullSolarSystemEntities {
    merc_ent: Entity,
    venus_ent: Entity,
    earth_ent: Entity,
    embryo_ent: Entity,
    theia_ent: Entity,
    mars_ent: Entity,
    p9_ent: Entity,
}

fn spawn_inner_solar_system(app: &mut App, star_mass: f64) -> (Entity, Entity, Entity, Entity) {
    // 2. Proto-Mercury at 0.39 AU
    let r_merc = 0.39;
    let v_merc = (G_ASTRO * star_mass / r_merc).sqrt();
    let merc_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(r_merc, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_merc)),
            SimAcceleration(DVec3::ZERO),
            Mass(0.055 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.38),
            Temperature(440.0),
            Composition::rocky(),
            InternalDifferentiation::default(),
            CelestialBody {
                name: "Proto-Mercury".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // 3. Proto-Venus at 0.72 AU (must NEVER be chosen as Earth!)
    let r_ven = 0.72;
    let v_ven = (G_ASTRO * star_mass / r_ven).sqrt();
    let venus_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(r_ven, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_ven)),
            SimAcceleration(DVec3::ZERO),
            Mass(0.815 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.95),
            Temperature(730.0),
            Composition::rocky(),
            InternalDifferentiation::default(),
            CelestialBody {
                name: "Proto-Venus".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // 4. Proto-Earth at 1.00 AU (the canonical target)
    let r_earth = 1.00;
    let v_earth = (G_ASTRO * star_mass / r_earth).sqrt();
    let mut earth_diff = InternalDifferentiation::default();
    earth_diff.recalculate(
        0.88 * EARTH_MASS_SOLAR,
        EARTH_RADIUS_AU * 0.94,
        &Composition::rocky(),
    );
    let earth_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(r_earth, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_earth)),
            SimAcceleration(DVec3::ZERO),
            Mass(0.88 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.94),
            Temperature(288.0),
            Composition::rocky(),
            earth_diff,
            CelestialBody {
                name: "Proto-Earth".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // 5. Embryo-1.0AU at 0.98 AU (must not hijack Earth or be chosen as Moon)
    let r_emb = 0.98;
    let v_emb = (G_ASTRO * star_mass / r_emb).sqrt();
    let embryo_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(r_emb, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_emb)),
            SimAcceleration(DVec3::ZERO),
            Mass(0.02 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.25),
            Temperature(280.0),
            Composition::rocky(),
            InternalDifferentiation::default(),
            CelestialBody {
                name: "Embryo-1.0AU".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    (merc_ent, venus_ent, earth_ent, embryo_ent)
}

fn spawn_outer_solar_system(app: &mut App, star_mass: f64) -> (Entity, Entity, Entity) {
    // 6. Theia at 1.18 AU
    let r_theia = 1.18;
    let v_theia = (G_ASTRO * star_mass / r_theia).sqrt();
    let theia_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(r_theia, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_theia)),
            SimAcceleration(DVec3::ZERO),
            Mass(0.12 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.53),
            Temperature(260.0),
            Composition::rocky(),
            InternalDifferentiation::default(),
            CelestialBody {
                name: "Theia".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // 7. Proto-Mars at 1.52 AU
    let r_mars = 1.52;
    let v_mars = (G_ASTRO * star_mass / r_mars).sqrt();
    let mars_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(r_mars, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_mars)),
            SimAcceleration(DVec3::ZERO),
            Mass(0.107 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.53),
            Temperature(215.0),
            Composition::rocky(),
            InternalDifferentiation::default(),
            CelestialBody {
                name: "Proto-Mars".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // 8. Planet Nine at 380 AU
    let r_p9 = 380.0;
    let v_p9 = (G_ASTRO * star_mass / r_p9).sqrt();
    let p9_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(r_p9, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_p9)),
            SimAcceleration(DVec3::ZERO),
            Mass(10.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 3.5),
            Temperature(40.0),
            Composition::icy(),
            InternalDifferentiation::default(),
            CelestialBody {
                name: "Planet Nine (Super-Earth / Ice Giant)".to_string(),
                body_type: BodyType::IceGiant,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    (theia_ent, mars_ent, p9_ent)
}

fn spawn_full_test_solar_system(app: &mut App) -> FullSolarSystemEntities {
    // 1. Central Star
    let star_mass = 1.0;
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(star_mass),
        Radius(0.00465),
        CentralStar,
    ));

    let (merc_ent, venus_ent, earth_ent, embryo_ent) = spawn_inner_solar_system(app, star_mass);
    let (theia_ent, mars_ent, p9_ent) = spawn_outer_solar_system(app, star_mass);

    FullSolarSystemEntities {
        merc_ent,
        venus_ent,
        earth_ent,
        embryo_ent,
        theia_ent,
        mars_ent,
        p9_ent,
    }
}

#[test]
fn test_theia_intercept_with_full_solar_system_selects_earth_not_venus_or_embryo() {
    let mut app = App::new();
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<TheiaImpactState>()
        .add_systems(Update, update_theia_rendezvous);

    let FullSolarSystemEntities {
        merc_ent,
        venus_ent,
        earth_ent,
        embryo_ent,
        theia_ent,
        mars_ent,
        p9_ent,
    } = spawn_full_test_solar_system(&mut app);

    // Request on-demand Moon formation
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 48.0;
        sim_time.current_dt_yr = 0.02;
        let mut state = app.world_mut().resource_mut::<TheiaImpactState>();
        state.manual_trigger_requested = true;
    }

    // Step the simulation
    for _ in 0..25 {
        app.world_mut().resource_mut::<SimTime>().elapsed_years += 0.05;
        app.update();
    }

    let final_state = app.world().resource::<TheiaImpactState>();
    assert!(
        final_state.moon_formed,
        "Moon must form reliably when Earth, Venus, and embryos all exist"
    );

    let world = app.world();

    // 1. Verify Proto-Venus is UNTOUCHED and did NOT get hijacked!
    let venus_body = world.get::<CelestialBody>(venus_ent).expect("Venus exists");
    assert_eq!(
        venus_body.name, "Proto-Venus",
        "Venus must NOT be renamed to Earth!"
    );
    let venus_sat = world.get::<SatelliteOf>(venus_ent);
    assert!(
        venus_sat.is_none(),
        "Venus must NOT have acquired a satellite!"
    );
    let venus_diff = world.get::<InternalDifferentiation>(venus_ent).unwrap();
    assert!(
        !venus_diff.has_theia_llsvp,
        "Venus must NOT have Theia LLSVPs!"
    );
    let venus_pos = world.get::<SimPosition>(venus_ent).unwrap().0;
    let venus_dist = (venus_pos.x * venus_pos.x + venus_pos.z * venus_pos.z).sqrt();
    assert!(
        (venus_dist - 0.72).abs() < 0.05,
        "Venus must remain in Venus orbit ~0.72 AU, found: {} AU",
        venus_dist
    );

    // 2. Verify Embryo-1.0AU is UNTOUCHED
    let embryo_body = world
        .get::<CelestialBody>(embryo_ent)
        .expect("Embryo exists");
    assert_eq!(embryo_body.name, "Embryo-1.0AU");
    assert!(world.get::<SatelliteOf>(embryo_ent).is_none());

    // 3. Verify Proto-Mars and Proto-Mercury are untouched
    assert_eq!(
        world.get::<CelestialBody>(merc_ent).unwrap().name,
        "Proto-Mercury"
    );
    assert_eq!(
        world.get::<CelestialBody>(mars_ent).unwrap().name,
        "Proto-Mars"
    );
    assert_eq!(
        world.get::<CelestialBody>(p9_ent).unwrap().name,
        "Planet Nine (Super-Earth / Ice Giant)"
    );

    // 4. Verify Proto-Earth is transformed into Earth
    let earth_body = world.get::<CelestialBody>(earth_ent).expect("Earth exists");
    assert_eq!(earth_body.name, "Earth");
    assert_eq!(earth_body.body_type, BodyType::TerrestrialPlanet);
    let earth_diff_res = world.get::<InternalDifferentiation>(earth_ent).unwrap();
    assert!(
        earth_diff_res.has_theia_llsvp,
        "Earth must have Theia LLSVP mantle remnants"
    );
    let earth_pos = world.get::<SimPosition>(earth_ent).unwrap().0;
    let earth_dist = (earth_pos.x * earth_pos.x + earth_pos.z * earth_pos.z).sqrt();
    assert!(
        (earth_dist - 1.00).abs() < 0.10,
        "Earth must be in 1.00 AU orbit, found: {} AU",
        earth_dist
    );

    // 5. Verify Theia is transformed into The Moon in orbit around Earth
    let moon_body = world
        .get::<CelestialBody>(theia_ent)
        .expect("The Moon exists");
    assert_eq!(moon_body.name, "The Moon");
    assert_eq!(moon_body.body_type, BodyType::Moon);
    let moon_sat = world
        .get::<SatelliteOf>(theia_ent)
        .expect("The Moon must have SatelliteOf");
    assert_eq!(
        moon_sat.parent, earth_ent,
        "The Moon must be satellite of Earth, NOT Venus!"
    );
    let moon_pos = world.get::<SimPosition>(theia_ent).unwrap().0;
    let moon_earth_dist = (moon_pos - earth_pos).length();
    assert!(
        moon_earth_dist < 0.05,
        "The Moon must be near Earth (< 0.05 AU), found: {} AU",
        moon_earth_dist
    );

    // 6. Verify count of Earths in the entire simulation is EXACTLY ONE!
    let mut bodies_query = app.world_mut().query::<&CelestialBody>();
    let earth_count = bodies_query
        .iter(app.world())
        .filter(|b| b.name == "Earth")
        .count();
    let moon_count = bodies_query
        .iter(app.world())
        .filter(|b| b.name == "The Moon")
        .count();
    assert_eq!(
        earth_count, 1,
        "Exactly one Earth must exist in the simulation!"
    );
    assert_eq!(
        moon_count, 1,
        "Exactly one Moon must exist in the simulation!"
    );
}
