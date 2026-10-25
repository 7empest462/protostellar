use bevy::prelude::*;
use protostellar::rendering::effects::impact_debris::update_impact_ejecta;
use protostellar::simulation::accretion::events::{
    AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent,
};
use protostellar::simulation::components::{BodyType, Mass, Radius, SimPosition, SimVelocity};
use protostellar::simulation::resources::{
    ImpactEjectaPool, MoltenDebrisFragment, SimTime, TimeWarp,
};
use protostellar::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU};

#[test]
fn test_molten_debris_blackbody_colors() {
    let mut frag = MoltenDebrisFragment {
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        initial_temperature: 3500.0,
        temperature: 3500.0,
        cooling_rate: 0.1,
        radius: 0.001,
        mass: 1e-8,
        primary_entity: Entity::PLACEHOLDER,
        age: 0.0,
        max_age: 10.0,
        trail: [Vec3::ZERO; 4],
    };

    // 3500 K: White-hot plasma core
    let col_hot = frag.blackbody_color().to_srgba();
    assert!(col_hot.red >= 0.99);
    assert!(col_hot.green >= 0.95);
    assert!(col_hot.blue >= 0.75);

    // 2400 K: Molten gold-yellow magma
    frag.temperature = 2400.0;
    let col_gold = frag.blackbody_color().to_srgba();
    assert!(col_gold.red >= 0.99);
    assert!(col_gold.green > 0.60 && col_gold.green < 0.95);
    assert!(col_gold.blue > 0.10 && col_gold.blue < 0.60);

    // 1600 K: Incandescent orange-red lava
    frag.temperature = 1600.0;
    let col_orange = frag.blackbody_color().to_srgba();
    assert!(col_orange.red >= 0.95);
    assert!(col_orange.green < 0.60);
    assert!(col_orange.blue < 0.15);

    // 400 K: Cooled dark basalt rock
    frag.temperature = 400.0;
    let col_dark = frag.blackbody_color().to_srgba();
    assert!(col_dark.red < 0.50);
    assert!(col_dark.green < 0.35);
    assert!(col_dark.blue < 0.35);
}

#[test]
fn test_moon_formation_ejecta_spawns_fragments_and_rings() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<MoonFormationEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<CollisionBounceEvent>()
        .init_resource::<ImpactEjectaPool>()
        .insert_resource(TimeWarp::default())
        .insert_resource(SimTime::default())
        .add_systems(Update, update_impact_ejecta);

    let earth_ent = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.0, 6.28)),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
        ))
        .id();

    let moon_ent = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(1.01, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.0, 6.4)),
            Mass(0.0123 * EARTH_MASS_SOLAR),
            Radius(0.272 * EARTH_RADIUS_AU),
        ))
        .id();

    // Send MoonFormationEvent
    {
        let mut events = app
            .world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MoonFormationEvent>>();
        events.write(MoonFormationEvent {
            parent_entity: earth_ent,
            moon_entity: moon_ent,
            moon_mass: 0.0123 * EARTH_MASS_SOLAR,
            orbital_radius_au: 0.012,
            orbital_period_years: 0.0748,
        });
    }

    app.update();

    let pool = app.world().resource::<ImpactEjectaPool>();
    assert_eq!(
        pool.rings.len(),
        1,
        "Should spawn one protolunar ejecta ring"
    );
    assert!(
        !pool.fragments.is_empty(),
        "Should spawn molten Keplerian debris fragments"
    );
    assert!(
        pool.fragments.len() >= 48,
        "Expected at least 48 molten fragments in proto-lunar disk"
    );

    // All fragments should be assigned to the parent planet (Earth)
    for frag in &pool.fragments {
        assert_eq!(frag.primary_entity, earth_ent);
        assert!(
            frag.temperature >= 2000.0,
            "Fragments should start molten hot"
        );
    }
}

#[test]
fn test_collision_merger_spawns_ballistic_fragments() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<MoonFormationEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<CollisionBounceEvent>()
        .init_resource::<ImpactEjectaPool>()
        .insert_resource(TimeWarp::default())
        .insert_resource(SimTime::default())
        .add_systems(Update, update_impact_ejecta);

    let target_ent = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.0, 6.28)),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
        ))
        .id();

    let secondary_ent = app.world_mut().spawn_empty().id();

    // Send AccretionMergeEvent
    {
        let mut events = app
            .world_mut()
            .resource_mut::<bevy::ecs::message::Messages<AccretionMergeEvent>>();
        events.write(AccretionMergeEvent {
            primary_entity: target_ent,
            secondary_entity: secondary_ent,
            merged_mass: 1.1 * EARTH_MASS_SOLAR,
            merged_position: bevy::math::DVec3::new(1.0001, 0.0, 0.0),
            merged_velocity: bevy::math::DVec3::new(0.0, 0.0, 6.25),
            new_body_type: BodyType::TerrestrialPlanet,
            energy_released: 1.0e32,
        });
    }

    app.update();

    let pool = app.world().resource::<ImpactEjectaPool>();
    assert!(
        !pool.fragments.is_empty(),
        "Merger should generate molten ejecta fragments"
    );
    for frag in &pool.fragments {
        assert_eq!(frag.primary_entity, target_ent);
        assert!(frag.temperature >= 2400.0);
    }
}

#[test]
fn test_reimpact_detection_culls_subsurface_fragments() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<MoonFormationEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<CollisionBounceEvent>()
        .init_resource::<ImpactEjectaPool>()
        .insert_resource(TimeWarp::default())
        .insert_resource(SimTime::default())
        .add_systems(Update, update_impact_ejecta);

    let planet_pos = Vec3::new(1.0, 0.0, 0.0);
    let planet_rad = EARTH_RADIUS_AU as f32;

    let planet_ent = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::ZERO),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
        ))
        .id();

    // Manually add one fragment inside the planet surface and one in high orbit
    {
        let mut pool = app.world_mut().resource_mut::<ImpactEjectaPool>();
        pool.fragments.push(MoltenDebrisFragment {
            position: planet_pos + Vec3::new(planet_rad * 0.5, 0.0, 0.0), // Deep inside planet
            velocity: Vec3::ZERO,
            initial_temperature: 3000.0,
            temperature: 3000.0,
            cooling_rate: 0.1,
            radius: 0.0005,
            mass: 1e-10,
            primary_entity: planet_ent,
            age: 0.0,
            max_age: 10.0,
            trail: [planet_pos; 4],
        });
        pool.fragments.push(MoltenDebrisFragment {
            position: planet_pos + Vec3::new(planet_rad * 10.0, 0.0, 0.0), // Safe orbital distance
            velocity: Vec3::new(0.0, 0.0, 0.5),
            initial_temperature: 3000.0,
            temperature: 3000.0,
            cooling_rate: 0.1,
            radius: 0.0005,
            mass: 1e-10,
            primary_entity: planet_ent,
            age: 0.0,
            max_age: 10.0,
            trail: [planet_pos; 4],
        });
    }

    app.update();

    let pool = app.world().resource::<ImpactEjectaPool>();
    assert_eq!(
        pool.fragments.len(),
        1,
        "Subsurface fragment should be culled due to re-impact"
    );
    assert!(pool.fragments[0].position.distance(planet_pos) > planet_rad * 2.0);
}
