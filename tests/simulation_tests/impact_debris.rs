use bevy::prelude::*;
use protostellar::rendering::effects::impact_debris::update_impact_ejecta;
use protostellar::simulation::accretion::events::{
    AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent,
};
use protostellar::simulation::components::{BodyType, Mass, Radius, SimPosition, SimVelocity};
use protostellar::simulation::resources::{
    ImpactEjectaPool, MoltenDebrisFragment, SimTime, TimeWarp,
};
use protostellar::simulation::terraforming::{BombardmentEvent, BombardmentType};
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
        local_position: Vec3::ZERO,
        local_velocity: Vec3::ZERO,
        local_trail: [Vec3::ZERO; 4],
        color_tint: None,
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
        .add_message::<BombardmentEvent>()
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
        .add_message::<BombardmentEvent>()
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
        .add_message::<BombardmentEvent>()
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
            local_position: Vec3::new(planet_rad * 0.5, 0.0, 0.0),
            local_velocity: Vec3::ZERO,
            local_trail: [Vec3::new(planet_rad * 0.5, 0.0, 0.0); 4],
            color_tint: None,
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
            local_position: Vec3::new(planet_rad * 10.0, 0.0, 0.0),
            local_velocity: Vec3::new(0.0, 0.0, 0.5),
            local_trail: [Vec3::new(planet_rad * 10.0, 0.0, 0.0); 4],
            color_tint: None,
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

#[test]
fn test_comet_impact_ejecta_blasts_into_space_and_gets_reabsorbed() {
    use protostellar::simulation::components::{ImpactBasin, PlanetaryBasins};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<MoonFormationEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<BombardmentEvent>()
        .init_resource::<ImpactEjectaPool>()
        .insert_resource(TimeWarp::default())
        .insert_resource(SimTime::default())
        .add_systems(Update, update_impact_ejecta);

    let planet_pos = Vec3::new(1.0, 0.0, 0.0);
    let visual_radius = 0.03_f32;
    let impact_normal = Vec3::new(-0.7071, 0.7071, 0.0).normalize();

    let planet_ent = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::ZERO),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Transform::from_translation(planet_pos).with_scale(Vec3::splat(visual_radius)),
            PlanetaryBasins {
                basins: vec![ImpactBasin {
                    surface_normal: impact_normal,
                    angular_radius: 0.12,
                    formation_time_yr: 0.0,
                    melt_glow_fraction: 1.0,
                    elongation: 1.0,
                    scar_intensity: 1.0,
                }],
            },
        ))
        .id();

    // Step 1: First update detects the fresh impact basin and spawns the 3D ballistic ejecta fountain
    app.update();

    let initial_count = {
        let pool = app.world().resource::<ImpactEjectaPool>();
        assert!(
            pool.fragments.len() >= 18,
            "Fresh comet/asteroid impact basin must spawn 3D ballistic ejecta fragments (got {})",
            pool.fragments.len()
        );
        for frag in &pool.fragments {
            assert_eq!(frag.primary_entity, planet_ent);
            let d = frag.position.distance(planet_pos);
            assert!(
                d >= visual_radius,
                "Ejecta must spawn outside the planet's visual mesh radius ({d} vs {visual_radius})"
            );
        }
        pool.fragments.len()
    };

    // Decay melt_glow_fraction slightly so it doesn't keep spawning new fountains on subsequent frames
    app.world_mut()
        .entity_mut(planet_ent)
        .get_mut::<PlanetaryBasins>()
        .unwrap()
        .basins[0]
        .melt_glow_fraction = 0.85;

    // Step 2: Advance over ~240 frames (~4 seconds of ballistic flight) and track peak altitude & reabsorption
    let mut peak_altitude_ratio = 1.0_f32;
    for _ in 0..240 {
        app.update();
        let pool = app.world().resource::<ImpactEjectaPool>();
        for frag in &pool.fragments {
            let ratio = frag.position.distance(planet_pos) / visual_radius;
            if ratio > peak_altitude_ratio {
                peak_altitude_ratio = ratio;
            }
        }
    }

    assert!(
        peak_altitude_ratio > 1.35,
        "Ejecta must blast significantly out into space above the planet surface (peak ratio: {peak_altitude_ratio:.3})"
    );

    let remaining = app.world().resource::<ImpactEjectaPool>().fragments.len();
    assert!(
        remaining < initial_count / 4,
        "Sub-orbital ballistic ejecta must curve back under planet gravity and get reabsorbed into the planet (initial: {initial_count}, remaining: {remaining})"
    );
}

#[test]
fn test_bombardment_event_ejecta_comoves_with_orbiting_planet() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<MoonFormationEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<BombardmentEvent>()
        .init_resource::<ImpactEjectaPool>()
        .insert_resource(TimeWarp::default())
        .insert_resource(SimTime::default())
        .add_systems(Update, update_impact_ejecta);

    let initial_planet_pos = Vec3::new(1.0, 0.0, 0.0);
    let visual_radius = 0.02_f32;
    let surface_normal = Vec3::new(0.0, 1.0, 0.0);

    let planet_ent = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.0, 6.28)),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Transform::from_translation(initial_planet_pos).with_scale(Vec3::splat(visual_radius)),
        ))
        .id();

    // Fire BombardmentEvent
    {
        let mut events = app
            .world_mut()
            .resource_mut::<bevy::ecs::message::Messages<BombardmentEvent>>();
        events.write(BombardmentEvent {
            target_entity: planet_ent,
            target_name: "Earth".to_string(),
            bombardment_type: BombardmentType::IcyComet,
            water_delivered_m_earth: 0.0001,
            gas_added_bar: 0.05,
            impact_velocity_km_s: 35.0,
            surface_normal,
            crater_angular_radius: 0.08,
        });
    }

    // Frame 1: Impact event is processed immediately
    app.update();

    {
        let pool = app.world().resource::<ImpactEjectaPool>();
        assert!(
            !pool.fragments.is_empty(),
            "BombardmentEvent must spawn ejecta immediately without waiting for basin polling"
        );
        for frag in &pool.fragments {
            assert_eq!(frag.primary_entity, planet_ent);
            assert!(frag.color_tint.is_some(), "Icy comet must have color tint");
            let d = frag.position.distance(initial_planet_pos);
            assert!(
                d >= visual_radius * 0.95 && d <= visual_radius * 1.5,
                "Ejecta must erupt directly from the planet's visible surface (d={d})"
            );
        }
    }

    // Simulate planet orbiting ahead along its orbital trajectory
    let orbit_displacement = Vec3::new(0.35, 0.0, 0.45);
    let new_planet_pos = initial_planet_pos + orbit_displacement;
    {
        let mut entity_mut = app.world_mut().entity_mut(planet_ent);
        entity_mut.get_mut::<SimPosition>().unwrap().0 += bevy::math::DVec3::new(0.35, 0.0, 0.45);
        entity_mut.get_mut::<Transform>().unwrap().translation = new_planet_pos;
    }

    // Frame 2: Next frame after orbit step
    app.update();

    {
        let pool = app.world().resource::<ImpactEjectaPool>();
        assert!(!pool.fragments.is_empty());
        for frag in &pool.fragments {
            let dist_to_planet = frag.position.distance(new_planet_pos);
            let dist_to_old_pos = frag.position.distance(initial_planet_pos);

            assert!(
                dist_to_planet <= visual_radius * 2.5,
                "Ejecta must co-move seamlessly with the orbiting planet (dist_to_planet={dist_to_planet})"
            );
            assert!(
                dist_to_old_pos > orbit_displacement.length() * 0.5,
                "Ejecta must NOT lag behind at old orbital position (dist_to_old_pos={dist_to_old_pos})"
            );
        }
    }
}
