use bevy::math::DVec3;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;

#[test]
fn test_ui_button_interactions_query_schedule_no_aliasing_conflict() {
    use bevy::prelude::*;
    use protostellar::game::ui::*;
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::LoadScenarioEvent;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_message::<LoadScenarioEvent>();
    app.init_resource::<QuickBarState>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<TelemetryPanelState>();
    app.init_resource::<protostellar::simulation::telemetry::SimulationTelemetryHistory>();
    app.init_resource::<HudVisibilityState>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();

    app.add_systems(Update, handle_ui_button_interactions);

    // Spawn Little Red Dot entity with BlackHoleStarState, CelestialBody, Mass, Radius, SimPosition
    app.world_mut().spawn((
        CelestialBody {
            name: "JWST Little Red Dot (Black Hole Star)".to_string(),
            body_type: BodyType::QuasiStar,
        },
        Mass(450_000.0),
        Radius(60.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Composition::pure_hydrogen(),
        BlackHoleStarState::default(),
    ));

    // Spawn camera
    app.world_mut()
        .spawn((PanOrbitCamera::default(), Transform::default()));

    // Update must initialize and run schedule with zero B0001 query aliasing panics!
    app.update();
}

#[test]
fn test_system_worlds_numerical_ordering_and_reindexing() {
    use bevy::prelude::*;
    use protostellar::game::ui::collect_sorted_system_worlds;
    use protostellar::simulation::components::*;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let mut world = World::new();

    // 1. Central Star (Sun at 0, 0, 0)
    let star_ent = world
        .spawn((
            CelestialBody {
                name: "Sol (Central Star)".to_string(),
                body_type: BodyType::MainSequenceStar,
            },
            SimPosition(DVec3::ZERO),
            Mass(1.0),
            Radius(0.00465),
            CentralStar,
        ))
        .id();

    // 2. Planet 1 (Mercury at 0.387 AU)
    let mercury_ent = world
        .spawn((
            CelestialBody {
                name: "Mercury".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(0.387, 0.0, 0.0)),
            Mass(0.055 * EARTH_MASS_SOLAR),
            Radius(0.000016),
        ))
        .id();

    // 3. Planet 2 (Venus at 0.723 AU)
    let venus_ent = world
        .spawn((
            CelestialBody {
                name: "Venus".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(0.723, 0.0, 0.0)),
            Mass(0.815 * EARTH_MASS_SOLAR),
            Radius(0.000040),
        ))
        .id();

    // 4. Planet 3 (Earth at 1.000 AU)
    let earth_ent = world
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.000, 0.0, 0.0)),
            Mass(EARTH_MASS_SOLAR),
            Radius(0.0000426),
        ))
        .id();

    // 5. Minor debris fragment (should be excluded from major system worlds)
    let debris_ent = world
        .spawn((
            CelestialBody {
                name: "debris-chunk-99".to_string(),
                body_type: BodyType::Planetesimal,
            },
            SimPosition(DVec3::new(0.500, 0.0, 0.0)),
            Mass(1e-8),
            Radius(1e-6),
        ))
        .id();

    // Helper closure to query and sort
    let query_and_sort = |w: &mut World| {
        let mut query = w.query::<(
            Entity,
            &CelestialBody,
            &SimPosition,
            &Mass,
            &Radius,
            Option<&CentralStar>,
        )>();
        let items: Vec<_> = query.iter(w).collect();
        collect_sorted_system_worlds(items)
    };

    let worlds = query_and_sort(&mut world);

    // Verify exactly 4 major worlds (debris excluded)
    assert_eq!(worlds.len(), 4);
    // Index 0: Sun (Central Star)
    assert_eq!(worlds[0].entity, star_ent);
    assert_eq!(worlds[0].index, 0);
    assert!(worlds[0].is_central_star);

    // Index 1: Mercury (0.387 AU)
    assert_eq!(worlds[1].entity, mercury_ent);
    assert_eq!(worlds[1].index, 1);
    assert!((worlds[1].distance_au - 0.387).abs() < 1e-4);

    // Index 2: Venus (0.723 AU)
    assert_eq!(worlds[2].entity, venus_ent);
    assert_eq!(worlds[2].index, 2);
    assert!((worlds[2].distance_au - 0.723).abs() < 1e-4);

    // Index 3: Earth (1.000 AU)
    assert_eq!(worlds[3].entity, earth_ent);
    assert_eq!(worlds[3].index, 3);
    assert!((worlds[3].distance_au - 1.000).abs() < 1e-4);

    // SIMULATE MERGER / DESPAWN: Mercury is swallowed or merges into Venus
    world.despawn(mercury_ent);
    world.despawn(debris_ent);

    let worlds_after_merger = query_and_sort(&mut world);
    assert_eq!(worlds_after_merger.len(), 3);

    // Index 0 remains Sun
    assert_eq!(worlds_after_merger[0].entity, star_ent);
    assert_eq!(worlds_after_merger[0].index, 0);

    // Index 1 now seamlessly becomes Venus!
    assert_eq!(worlds_after_merger[1].entity, venus_ent);
    assert_eq!(worlds_after_merger[1].index, 1);

    // Index 2 now seamlessly becomes Earth!
    assert_eq!(worlds_after_merger[2].entity, earth_ent);
    assert_eq!(worlds_after_merger[2].index, 2);
}

#[test]
fn test_trappist1_compact_disk_particle_sampling() {
    let mut rng = rand::rng();
    let disk_params = protostellar::simulation::resources::DiskParameters {
        central_star_mass: 0.0898,
        inner_radius_au: 0.005,
        outer_radius_au: 0.15,
        disk_mass: 0.0001,
        ..Default::default()
    };

    let mut min_r = f64::INFINITY;
    let mut max_r = f64::NEG_INFINITY;
    let mut inner_count = 0;
    let mut outer_count = 0;

    for _ in 0..10_000 {
        let (r, comp) = protostellar::simulation::disk::sample_disk_radius(&mut rng, &disk_params);
        assert!(
            (0.005..=0.150001).contains(&r),
            "Sampled radius {} out of bounds [0.005, 0.15]",
            r
        );
        if r < min_r {
            min_r = r;
        }
        if r > max_r {
            max_r = r;
        }
        if r < 0.07 {
            inner_count += 1;
            assert!(comp.silicate_frac > 0.3 || comp.metal_frac > 0.3);
        } else {
            outer_count += 1;
        }
    }

    assert!(min_r < 0.02, "Expected inner particles down to ~0.005 AU");
    assert!(max_r > 0.13, "Expected outer particles up to ~0.15 AU");
    assert!(inner_count > 3000, "Expected substantial inner particles");
    assert!(outer_count > 2000, "Expected substantial outer particles");
}

#[test]
fn test_earth_spawns_at_1_earth_mass_in_solar_nebula() {
    use bevy::prelude::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::spawn_solar_nebula_mmsn;

    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let _star_ent = spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let mut earth_found = false;
    let mut query = app.world_mut().query::<(&CelestialBody, &Mass, &Radius)>();
    for (body, mass, radius) in query.iter(app.world()) {
        if body.name == "Earth" {
            earth_found = true;
            assert_eq!(body.body_type, BodyType::TerrestrialPlanet);
            let m_earth = mass.0 / EARTH_MASS_SOLAR;
            assert!(
                (m_earth - 1.00).abs() < 1e-4,
                "Earth must spawn at 1.00 M_earth, got {:.4}",
                m_earth
            );
            let r_earth = radius.0 / EARTH_RADIUS_AU;
            assert!(
                (r_earth - 1.00).abs() < 1e-4,
                "Earth radius must be 1.00 R_earth, got {:.4}",
                r_earth
            );
        }
    }
    assert!(
        earth_found,
        "Earth entity must spawn in Hayashi Solar Nebula scenario"
    );
}

#[test]
fn test_inner_planet_nebular_gas_and_atmosphere_accretion() {
    use bevy::prelude::*;
    use protostellar::simulation::accretion::direct_nebular_gas_accretion;
    use protostellar::simulation::resources::*;

    let mut app = App::new();

    let config = SimulationConfig {
        enable_accretion: true,
        accretion_rate_multiplier: 120.0,
        gas_density_scale: 1.0,
        base_dt_yr: 0.01, // 3.65 days per step
        ..Default::default()
    };
    app.insert_resource(config);

    let time_warp = TimeWarp {
        multiplier: 1.0,
        is_paused: false,
        ..Default::default()
    };
    app.insert_resource(time_warp);

    let sim_time = SimTime {
        elapsed_years: 0.5,
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let disk_params = DiskParameters {
        central_star_mass: 1.0,
        inner_radius_au: 0.20,
        outer_radius_au: 45.0,
        gas_disk_lifetime_yr: 60_000.0,
        ..Default::default()
    };
    app.insert_resource(disk_params);

    // Spawn central star (unignited protostar)
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            body_type: BodyType::Protostar,
            name: "The Protostar".to_string(),
        },
        IgnitionState {
            core_temperature: 4.0e6,
            fusion_fraction: 0.4,
            is_ignited: false,
            shockwave_radius: 0.0,
        },
    ));

    // Spawn Earth at 1.0 AU inside the gas cloud
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Earth".to_string(),
            },
            Mass(1.00 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.10,
                cometary_impact_count: 0,
            },
        ))
        .id();

    app.add_systems(Update, direct_nebular_gas_accretion);

    // Run 50 simulation steps inside the gas cloud before star ignites
    for _ in 0..50 {
        app.update();
    }

    let world = app.world();
    let mass = world.get::<Mass>(earth_ent).expect("Mass required");
    let comp = world
        .get::<Composition>(earth_ent)
        .expect("Composition required");
    let vol = world
        .get::<VolatileInventory>(earth_ent)
        .expect("Volatiles required");
    let body = world
        .get::<CelestialBody>(earth_ent)
        .expect("Body required");

    let m_earth = mass.0 / EARTH_MASS_SOLAR;
    assert!(
        m_earth > 1.0001,
        "Earth must accumulate nebular gas mass from circumstellar gas cloud! Got {:.6}",
        m_earth
    );
    assert!(
        m_earth < 1.05,
        "Earth should not undergo runaway gas accumulation into a gas giant! Got {:.6}",
        m_earth
    );
    assert!(
        comp.gas_frac > 0.0001 && comp.gas_frac <= 0.035,
        "Gas fraction must increase into a realistic secondary atmosphere, got {:.6}",
        comp.gas_frac
    );
    assert!(
        vol.atmospheric_pressure_bar > 0.10,
        "Atmospheric pressure must rise from accreted nebular gas, got {:.3} bar",
        vol.atmospheric_pressure_bar
    );
    assert_eq!(
        body.name, "Earth",
        "Canonical planet name 'Earth' must be preserved and not overwritten with generic 'Planet-1AU'"
    );
    assert_eq!(
        body.body_type,
        BodyType::TerrestrialPlanet,
        "Earth must remain a TerrestrialPlanet"
    );
}

#[test]
fn test_ui_button_click_prevents_camera_3d_raycast_hijacking() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    // Spawn a dummy Window
    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    // Spawn Central Star at (0,0,0)
    let star_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "The Sun".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            Mass(1.0),
            Radius(SOLAR_RADIUS_AU),
            SimPosition(DVec3::ZERO),
        ))
        .id();

    // Spawn Earth at (1,0,0)
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.00 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        ))
        .id();

    // Spawn UI button that is currently clicked (Interaction::Pressed)
    app.world_mut().spawn((Button, Interaction::Pressed));

    // Spawn Camera looking at the scene, currently targeting Earth
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: Some(earth_ent),
                focus: Vec3::new(1.0, 0.0, 0.0),
                target_focus: Vec3::new(1.0, 0.0, 0.0),
                ..default()
            },
            Transform::from_xyz(1.0, 0.5, 3.0).looking_at(Vec3::new(1.0, 0.0, 0.0), Vec3::Y),
            GlobalTransform::from(
                Transform::from_xyz(1.0, 0.5, 3.0).looking_at(Vec3::new(1.0, 0.0, 0.0), Vec3::Y),
            ),
        ))
        .id();

    // Simulate Left mouse button press
    let mut mouse_buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse_buttons.press(MouseButton::Left);

    app.add_systems(Update, update_pan_orbit_camera);
    app.update();

    let world = app.world();
    let cam = world
        .get::<PanOrbitCamera>(camera_ent)
        .expect("Camera required");

    // Camera target_entity MUST remain Earth and not be hijacked to the Central Star!
    assert_eq!(
        cam.target_entity,
        Some(earth_ent),
        "Camera target_entity must remain Earth and NOT be hijacked to the star or background raycast target when clicking a UI button!"
    );
    assert_ne!(
        cam.target_entity,
        Some(star_ent),
        "Camera target_entity must NOT bounce back to the star!"
    );
}
