use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::rendering::bodies::{
    sync_cometary_tails, CometComaPart, CometTailPart, CometTailRoot, VisualAssets,
};
use protostellar::rendering::materials::CometTailMaterial;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::{
    DiagnosticOverlayMode, PlayerInteractionState, SimTime, SimulationConfig,
};
use protostellar::utils::constants::*;

#[test]
fn test_comet_tails_exclude_planets_and_moons_and_scale_with_physics() {
    let mut app = App::new();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CometTailMaterial>>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<PlayerInteractionState>();

    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let fallback_mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));
    app.insert_resource(VisualAssets::dummy(fallback_mesh));

    // 1. Central Star
    let star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::MainSequenceStar,
                name: "Sol".to_string(),
            },
            SimPosition(DVec3::ZERO),
            Transform::from_xyz(0.0, 0.0, 0.0),
            CentralStar,
        ))
        .id();

    // 2. Large Ice-Rich Comet (Hale-Bopp: 30 km nucleus, 80% ice, at 1.2 AU)
    let large_comet = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Comet,
                name: "C/1995 O1 (Hale-Bopp)".to_string(),
            },
            Mass(1e-9 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(1.2, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 6.5)),
            Radius(30.0 / AU_TO_KM),
            Composition {
                ice_frac: 0.80,
                silicate_frac: 0.15,
                metal_frac: 0.0,
                gas_frac: 0.0,
                organics_frac: 0.05,
            },
            Transform::from_xyz(1.2, 0.0, 0.0),
        ))
        .id();

    // 3. Small Dusty Comet (Encke: 2 km nucleus, 25% ice, at 2.2 AU)
    let small_comet = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Comet,
                name: "2P/Encke".to_string(),
            },
            Mass(1e-11 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(2.2, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 5.0)),
            Radius(2.0 / AU_TO_KM),
            Composition {
                ice_frac: 0.25,
                silicate_frac: 0.65,
                metal_frac: 0.05,
                gas_frac: 0.0,
                organics_frac: 0.05,
            },
            Transform::from_xyz(2.2, 0.0, 0.0),
        ))
        .id();

    // 4. Rocky Asteroid (Vesta: rocky, no ice)
    let asteroid = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Asteroid,
                name: "4 Vesta".to_string(),
            },
            Mass(1e-8 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(2.36, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 4.0)),
            Radius(EARTH_RADIUS_AU * 0.04),
            Composition::rocky(),
            Transform::from_xyz(2.36, 0.0, 0.0),
        ))
        .id();

    // 5. Gas Giant Super-Jupiter (with active atmospheric escape tail)
    // Must NEVER receive a cometary tail!
    let super_jupiter = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::GasGiant,
                name: "Super-Jupiter".to_string(),
            },
            Mass(450.0 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(0.86, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 38.0)),
            Radius(EARTH_RADIUS_AU * 11.2),
            Composition::solar_gas(),
            AtmosphericEscapeTail {
                loss_rate_m_earth_per_myr: 0.05,
                tail_length_au: 0.25,
                ion_color: Color::srgba(0.2, 0.8, 1.0, 0.7),
                is_active: true,
            },
            Transform::from_xyz(0.86, 0.0, 0.0),
        ))
        .id();

    // 6. Moon (Io with volcanic / atmospheric escape)
    // Must NEVER receive a cometary tail!
    let moon = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Moon,
                name: "Io".to_string(),
            },
            Mass(0.015 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(0.46, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 46.0)),
            Radius(EARTH_RADIUS_AU * 0.27),
            Composition::rocky(),
            AtmosphericEscapeTail {
                loss_rate_m_earth_per_myr: 0.01,
                tail_length_au: 0.15,
                ion_color: Color::srgba(1.0, 0.8, 0.2, 0.7),
                is_active: true,
            },
            Transform::from_xyz(0.46, 0.0, 0.0),
        ))
        .id();

    app.add_systems(Update, sync_cometary_tails);

    // Test under "No Overlay" mode
    {
        let mut player_state = app.world_mut().resource_mut::<PlayerInteractionState>();
        player_state.overlay_mode = DiagnosticOverlayMode::Hidden;
    }

    app.update();

    // Collect all spawned CometTailRoot entities
    let roots: Vec<(Entity, CometTailRoot)> = app
        .world_mut()
        .query::<(Entity, &CometTailRoot)>()
        .iter(app.world())
        .map(|(e, r)| (e, *r))
        .collect();

    // 1. Assert exactly the two comets have tail roots
    assert_eq!(
        roots.len(),
        2,
        "Exactly 2 comets must have CometTailRoot entities spawned"
    );

    // 2. Assert Planets and Moons NEVER receive comet tail roots
    assert!(
        !roots.iter().any(|(_, r)| r.comet_entity == super_jupiter),
        "Gas giant planets must NEVER have cometary tails!"
    );
    assert!(
        !roots.iter().any(|(_, r)| r.comet_entity == moon),
        "Moons must NEVER have cometary tails!"
    );
    assert!(
        !roots.iter().any(|(_, r)| r.comet_entity == asteroid),
        "Rocky asteroids must not have cometary tails"
    );

    // 3. Verify physical scaling between large and small comet
    let large_root = roots
        .iter()
        .find(|(_, r)| r.comet_entity == large_comet)
        .unwrap()
        .0;
    let small_root = roots
        .iter()
        .find(|(_, r)| r.comet_entity == small_comet)
        .unwrap()
        .0;

    let large_children = app.world().get::<Children>(large_root).unwrap();
    let small_children = app.world().get::<Children>(small_root).unwrap();

    let mut large_tail_len = 0.0f32;
    let mut small_tail_len = 0.0f32;
    let mut large_coma_r = 0.0f32;
    let mut small_coma_r = 0.0f32;

    for part in large_children.iter() {
        if app.world().get::<CometTailPart>(part).is_some() {
            let trans = app.world().get::<Transform>(part).unwrap();
            large_tail_len = trans.scale.y;
        }
        if app.world().get::<CometComaPart>(part).is_some() {
            let trans = app.world().get::<Transform>(part).unwrap();
            large_coma_r = trans.scale.x;
        }
    }

    for part in small_children.iter() {
        if app.world().get::<CometTailPart>(part).is_some() {
            let trans = app.world().get::<Transform>(part).unwrap();
            small_tail_len = trans.scale.y;
        }
        if app.world().get::<CometComaPart>(part).is_some() {
            let trans = app.world().get::<Transform>(part).unwrap();
            small_coma_r = trans.scale.x;
        }
    }

    assert!(
        large_tail_len > small_tail_len * 1.5,
        "Large comet tail length ({:.2}) must be substantially greater than small comet ({:.2})",
        large_tail_len,
        small_tail_len
    );
    assert!(
        large_coma_r > small_coma_r,
        "Large comet coma radius ({:.4}) must be greater than small comet coma ({:.4})",
        large_coma_r,
        small_coma_r
    );

    // 4. Verify procedural mesh has closed apex at (0, 0, 0)
    let envelope = protostellar::rendering::bodies::meshes::generate_comet_tail_envelope_mesh();
    let positions = envelope
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    assert_eq!(
        positions[0],
        [0.0, 0.0, 0.0],
        "Comet tail envelope mesh apex must be at (0, 0, 0) with zero radius"
    );

    // 5. Inactive comets beyond 6 AU despawn their roots
    if let Some(mut pos) = app.world_mut().get_mut::<SimPosition>(large_comet) {
        pos.0 = DVec3::new(10.0, 0.0, 0.0);
    }
    if let Some(mut pos) = app.world_mut().get_mut::<SimPosition>(small_comet) {
        pos.0 = DVec3::new(10.0, 0.0, 0.0);
    }
    app.update();

    let post_roots: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<CometTailRoot>>()
        .iter(app.world())
        .collect();
    assert!(
        post_roots.is_empty(),
        "All comet tail roots must be despawned once comets move beyond sublimation boundary"
    );

    let _ = star_ent;
}

#[test]
fn test_dragged_comet10_planetesimal_spawns_tail_at_sub_au_distance() {
    let mut app = App::new();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CometTailMaterial>>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<PlayerInteractionState>();

    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let fallback_mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));
    app.insert_resource(VisualAssets::dummy(fallback_mesh));

    // 1. Central Star (Sun)
    let _star = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::MainSequenceStar,
                name: "The Star".to_string(),
            },
            SimPosition(DVec3::ZERO),
            CentralStar,
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();

    // 2. The exact body from the user's screenshot: COMET #10 [PLANETESIMAL] dragged to 0.64 AU
    // Mass: 0.0057 M_earth, Radius: 4443 km, 16% water, 70% gas, 12% rock, 2% metal
    let comet10 = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Planetesimal,
                name: "COMET #10".to_string(),
            },
            Mass(0.0057 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(0.64, 0.0, 0.0)),
            SimVelocity(DVec3::new(
                0.0,
                0.0,
                37.3 * 1000.0 / (AU_TO_METERS / YEAR_SECONDS),
            )),
            Radius(4443.0 / AU_TO_KM),
            Composition {
                ice_frac: 0.16,
                gas_frac: 0.70,
                silicate_frac: 0.12,
                metal_frac: 0.02,
                organics_frac: 0.0,
            },
            AtmosphericEscapeTail {
                loss_rate_m_earth_per_myr: 0.01,
                tail_length_au: 3.67,
                ion_color: Color::srgb(0.2, 0.8, 1.0),
                is_active: true,
            },
            Transform::from_xyz(0.64, 0.0, 0.0),
        ))
        .id();

    app.add_systems(Update, sync_cometary_tails);
    app.update();

    let roots: Vec<(Entity, &CometTailRoot)> = app
        .world_mut()
        .query::<(Entity, &CometTailRoot)>()
        .iter(app.world())
        .collect();

    let comet10_root = roots.iter().find(|(_, r)| r.comet_entity == comet10);
    assert!(
        comet10_root.is_some(),
        "COMET #10 [PLANETESIMAL] with 16% water and active sublimation at 0.64 AU MUST spawn a CometTailRoot!"
    );
}
