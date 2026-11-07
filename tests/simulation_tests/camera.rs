//! Test module generated from simulation_tests.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;

fn setup_camera_zoom_app() -> (App, Entity, Entity, f32, f32) {
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

    // 1. Spawn Central Star at (0, 0, 0)
    let _star_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "The Protostar (Solar Nebula)".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(1.0),
            Radius(SOLAR_RADIUS_AU),
            SimPosition(DVec3::ZERO),
        ))
        .id();

    // 2. Spawn Earth at (1.0, 0, 0)
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

    let config = app.world().resource::<SimulationConfig>().clone();
    let star_vis_r = config.calc_visual_radius_for_type(SOLAR_RADIUS_AU, BodyType::Protostar);
    let earth_vis_r =
        config.calc_visual_radius_for_type(EARTH_RADIUS_AU, BodyType::TerrestrialPlanet);

    // 3. Spawn Camera looking at origin (star), initially with target_entity = None
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: None,
                focus: Vec3::ZERO,
                target_focus: Vec3::ZERO,
                radius: 16.0,
                target_radius: 16.0,
                ..default()
            },
            Transform::from_xyz(0.0, 5.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
            GlobalTransform::from(
                Transform::from_xyz(0.0, 5.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
            ),
        ))
        .id();

    app.add_systems(Update, update_pan_orbit_camera);
    (app, camera_ent, earth_ent, star_vis_r, earth_vis_r)
}

fn verify_zoom_into_central_star(app: &mut App, camera_ent: Entity, star_vis_r: f32) {
    use bevy::input::mouse::MouseWheel;
    use bevy::prelude::*;
    use protostellar::rendering::camera::PanOrbitCamera;

    // Simulate massive zoom-in scroll
    {
        let mut wheel_events = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel_events.write(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 50.0, // massive scroll in
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }

    // Run multiple frames for damping
    for _ in 0..40 {
        app.update();
    }

    let world = app.world();
    let cam = world
        .get::<PanOrbitCamera>(camera_ent)
        .expect("Camera required");

    // Camera must stop right before the star's surface:
    assert!(
        cam.min_radius > star_vis_r,
        "min_radius ({}) must be strictly greater than star's visual radius ({}) to prevent penetrating the photosphere!",
        cam.min_radius,
        star_vis_r
    );
    assert!(
        cam.radius >= cam.min_radius - 0.0001,
        "Camera radius ({}) must stop at or above min_radius ({})!",
        cam.radius,
        cam.min_radius
    );
    let surface_clearance_star = cam.radius - star_vis_r;
    assert!(
        surface_clearance_star >= 0.005,
        "Camera must maintain safe surface clearance ({}) in front of the star!",
        surface_clearance_star
    );
}

fn verify_zoom_into_earth(app: &mut App, camera_ent: Entity, earth_ent: Entity, earth_vis_r: f32) {
    use bevy::input::mouse::MouseWheel;
    use bevy::prelude::*;
    use protostellar::rendering::camera::PanOrbitCamera;

    {
        let mut cam_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam_mut.target_entity = Some(earth_ent);
        cam_mut.target_focus = Vec3::new(1.0, 0.0, 0.0);
        cam_mut.focus = Vec3::new(1.0, 0.0, 0.0);
        cam_mut.radius = 1.0;
        cam_mut.target_radius = 1.0;

        let mut wheel_events = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel_events.write(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 50.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }

    for _ in 0..40 {
        app.update();
    }

    {
        let cam = app
            .world()
            .get::<PanOrbitCamera>(camera_ent)
            .expect("Camera required");

        assert!(
            cam.min_radius > earth_vis_r,
            "min_radius ({}) must be strictly greater than Earth's visual radius ({})!",
            cam.min_radius,
            earth_vis_r
        );
        assert!(
            cam.radius >= cam.min_radius - 0.0001,
            "Camera radius ({}) must be bounded by min_radius ({})!",
            cam.radius,
            cam.min_radius
        );
        let surface_clearance_earth = cam.radius - earth_vis_r;
        assert!(
            surface_clearance_earth >= 0.001,
            "Surface clearance ({}) must be far greater than camera near clipping plane (0.0001 AU) to prevent near-plane clipping!",
            surface_clearance_earth
        );
    }
}

fn verify_deep_space_zoom(app: &mut App, camera_ent: Entity) {
    use bevy::prelude::*;
    use protostellar::rendering::camera::PanOrbitCamera;

    {
        let mut cam_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam_mut.target_entity = None;
        cam_mut.target_focus = Vec3::new(500.0, 500.0, 0.0);
        cam_mut.focus = Vec3::new(500.0, 500.0, 0.0);
    }
    app.update();

    {
        let cam = app
            .world()
            .get::<PanOrbitCamera>(camera_ent)
            .expect("Camera required");
        assert_eq!(
            cam.min_radius, 0.001,
            "In deep space far from any celestial body, min_radius should allow free zooming down to 0.001 AU!"
        );
    }
}

fn verify_zoom_into_quasi_star(app: &mut App, camera_ent: Entity) {
    use bevy::input::mouse::MouseWheel;
    use bevy::prelude::*;
    use protostellar::rendering::camera::PanOrbitCamera;

    let lrd_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "JWST Little Red Dot (Black Hole Star)".to_string(),
                body_type: BodyType::QuasiStar,
            },
            Mass(100_000.0),
            Radius(60.0),
            SimPosition(DVec3::new(100.0, 0.0, 0.0)),
        ))
        .id();

    {
        let mut cam_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam_mut.target_entity = Some(lrd_ent);
        cam_mut.target_focus = Vec3::new(100.0, 0.0, 0.0);
        cam_mut.focus = Vec3::new(100.0, 0.0, 0.0);
        cam_mut.radius = 200.0;
        cam_mut.target_radius = 200.0;

        let mut wheel_events = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel_events.write(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 500.0, // massive scroll in
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }

    for _ in 0..60 {
        app.update();
    }

    let world = app.world();
    let cam = world
        .get::<PanOrbitCamera>(camera_ent)
        .expect("Camera required");

    assert_eq!(
        cam.min_radius, 0.001,
        "Little Red Dot must have min_radius = 0.001 AU to allow zooming directly into the central black hole!"
    );
    assert!(
        cam.radius <= 0.01,
        "Camera radius ({}) must zoom deep inside the 60 AU cocoon down to the central black hole (<= 0.01 AU)!",
        cam.radius
    );
}

#[test]
fn test_camera_zoom_stops_safely_before_surface_of_star_and_planets() {
    let (mut app, camera_ent, earth_ent, star_vis_r, earth_vis_r) = setup_camera_zoom_app();
    verify_zoom_into_central_star(&mut app, camera_ent, star_vis_r);
    verify_zoom_into_earth(&mut app, camera_ent, earth_ent, earth_vis_r);
    verify_deep_space_zoom(&mut app, camera_ent);
    verify_zoom_into_quasi_star(&mut app, camera_ent);
}

#[test]
fn test_camera_tracking_selected_planet_zero_drift_at_high_warp() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    // Spawn dummy Window
    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    // Spawn Sun at origin
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Transform::IDENTITY,
    ));

    // Spawn Earth at 1.0 AU with Keplerian orbital velocity
    let v_earth = (G_ASTRO * 1.0 / 1.0).sqrt();
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_earth)),
            SimAcceleration(DVec3::ZERO),
            Transform::from_xyz(1.0, 0.0, 0.0),
        ))
        .id();

    // Spawn Camera focus-locked on Earth
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: Some(earth_ent),
                focus: Vec3::new(1.0, 0.0, 0.0),
                target_focus: Vec3::new(1.0, 0.0, 0.0),
                radius: 0.05,
                target_radius: 0.05,
                yaw: 0.5,
                target_yaw: 0.5,
                pitch: 0.3,
                target_pitch: 0.3,
                ..default()
            },
            Transform::IDENTITY,
            GlobalTransform::IDENTITY,
        ))
        .id();

    // Mock transform sync system matching sync_celestial_transforms logic
    fn sync_transforms(mut query: Query<(&SimPosition, &mut Transform)>) {
        for (pos, mut tf) in query.iter_mut() {
            tf.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        }
    }

    // Schedule: physics -> sync transforms -> camera
    app.add_systems(
        Update,
        (
            step_physics_simulation,
            sync_transforms.after(step_physics_simulation),
            update_pan_orbit_camera.after(sync_transforms),
        ),
    );

    // Test across standard simulation time warp multipliers: 1x, 10x, 100x, 1,000x, 10,000x
    let warps = [1.0, 10.0, 100.0, 1_000.0, 10_000.0];
    for &warp in &warps {
        app.world_mut().resource_mut::<TimeWarp>().multiplier = warp;

        for _ in 0..10 {
            app.update();

            let cam_tf = *app.world().get::<Transform>(camera_ent).unwrap();
            let earth_tf = *app.world().get::<Transform>(earth_ent).unwrap();

            // Transform Earth's position into camera view space:
            let view_matrix = cam_tf.to_matrix().inverse();
            let earth_in_view = view_matrix.transform_point3(earth_tf.translation);

            assert!(
                earth_in_view.x.abs() < 1e-4,
                "At warp {}x, Earth screen X offset ({}) must be 0 (drift detected!)",
                warp,
                earth_in_view.x
            );
            assert!(
                earth_in_view.y.abs() < 1e-4,
                "At warp {}x, Earth screen Y offset ({}) must be 0 (drift detected!)",
                warp,
                earth_in_view.y
            );
            // Z must be negative (in front of the camera, at exactly -radius)
            assert!(
                earth_in_view.z < 0.0,
                "At warp {}x, Earth must be in front of the camera (z = {})",
                warp,
                earth_in_view.z
            );
        }
    }
}

#[test]
fn test_outer_bodies_camera_stability_no_cancellation_jitter() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    // Spawn dummy Window
    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    // 1. Spawn Sun
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        SimPosition(DVec3::ZERO),
        Transform::IDENTITY,
    ));

    // 2. Spawn Planet Nine at 380.0 AU
    let p9_pos = DVec3::new(380.0, 0.0, 0.0);
    let p9_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Planet Nine".to_string(),
                body_type: BodyType::IceGiant,
            },
            Mass(5.5 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 2.3),
            SimPosition(p9_pos),
            Transform::from_translation(Vec3::new(p9_pos.x as f32, 0.0, 0.0)),
        ))
        .id();

    // 3. Spawn Camera focused on Planet Nine at 380 AU
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: Some(p9_ent),
                focus: Vec3::new(380.0, 0.0, 0.0),
                target_focus: Vec3::new(380.0, 0.0, 0.0),
                radius: 0.05,
                target_radius: 0.05,
                yaw: 1.15,
                target_yaw: 1.15,
                pitch: 0.45,
                target_pitch: 0.45,
                ..default()
            },
            Transform::IDENTITY,
            GlobalTransform::IDENTITY,
        ))
        .id();

    app.add_systems(Update, update_pan_orbit_camera);

    // Update multiple frames
    for _ in 0..10 {
        app.update();
    }

    let cam = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    let cam_tf = *app.world().get::<Transform>(camera_ent).unwrap();
    let p9_tf = *app.world().get::<Transform>(p9_ent).unwrap();

    // Verify analytical camera rotation: exact match with Quat(yaw, pitch) without cancellation error
    let expected_rot =
        Quat::from_axis_angle(Vec3::Y, cam.yaw) * Quat::from_axis_angle(Vec3::X, -cam.pitch);
    assert!(
        cam_tf.rotation.abs_diff_eq(expected_rot, 1e-6),
        "Camera rotation at 380 AU must match analytical quaternion with zero noise (found {:?}, expected {:?})",
        cam_tf.rotation,
        expected_rot
    );

    // Camera forward vector in world coordinates (-Z)
    let cam_forward = cam_tf.rotation * -Vec3::Z;
    let dir_to_planet = (p9_tf.translation - cam_tf.translation).normalize();

    // Camera forward must point directly at Planet Nine with dot product 1.0 (zero angular jitter)
    let dot = cam_forward.dot(dir_to_planet);
    assert!(
        (dot - 1.0).abs() < 1e-5,
        "Camera optical axis must align with Planet Nine at 380 AU with dot product ~1.0 (found {:.7})",
        dot
    );
}

fn setup_camera_zoom_test_app() -> (App, Entity) {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    // Spawn Sun
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        SimPosition(DVec3::ZERO),
        Transform::IDENTITY,
    ));

    // Spawn Camera at 16.0 AU
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                radius: 16.0,
                target_radius: 16.0,
                ..default()
            },
            Transform::from_xyz(0.0, 5.0, 16.0),
            GlobalTransform::IDENTITY,
        ))
        .id();

    app.add_systems(Update, update_pan_orbit_camera);
    (app, camera_ent)
}

#[test]
fn test_camera_zoom_sensitivity_mac_trackpad_and_modifiers() {
    use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::PanOrbitCamera;

    let (mut app, camera_ent) = setup_camera_zoom_test_app();

    // 1. Test Line scroll vs Pixel scroll calibration:
    // A 1-line scroll in (zoom in)
    {
        let mut wheel = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel.write(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }
    app.update();

    let target_r_line = app
        .world()
        .get::<PanOrbitCamera>(camera_ent)
        .unwrap()
        .target_radius;
    // 16.0 * exp(-1.0 * 0.055) = ~15.14 AU
    assert!(
        (target_r_line - 15.14).abs() < 0.05,
        "1-line scroll must zoom gently by ~5.4% (got target_radius {})",
        target_r_line
    );

    // Reset back to 16.0 AU
    {
        let mut cam = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam.radius = 16.0;
        cam.target_radius = 16.0;
    }

    // A 24-pixel trackpad gesture (macOS smooth scroll) must yield the EXACT same gentle 1-line zoom!
    {
        let mut wheel = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel.write(MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: 24.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }
    app.update();

    let target_r_pixel = app
        .world()
        .get::<PanOrbitCamera>(camera_ent)
        .unwrap()
        .target_radius;
    assert!(
        (target_r_pixel - target_r_line).abs() < 1e-4,
        "24-pixel macOS trackpad gesture ({}) must match 1-line scroll ({}) precisely!",
        target_r_pixel,
        target_r_line
    );

    // 2. Test Shift precision micro-zoom
    {
        let mut cam = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam.radius = 16.0;
        cam.target_radius = 16.0;

        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ShiftLeft);

        let mut wheel = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel.write(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }
    app.update();

    let target_r_shift = app
        .world()
        .get::<PanOrbitCamera>(camera_ent)
        .unwrap()
        .target_radius;
    assert!(
        (target_r_shift - 15.69).abs() < 0.05,
        "Shift-held scroll must provide precision micro-zoom (~1.9% delta, got target_radius {})",
        target_r_shift
    );
}

#[test]
#[allow(
    deprecated,
    reason = "Camera UI suppression test checks Interaction::Hovered in Bevy 0.20"
)]
fn test_camera_zoom_ui_interaction_suppression() {
    use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::PanOrbitCamera;

    let (mut app, camera_ent) = setup_camera_zoom_test_app();

    // Spawn a hovered UI element
    let ui_node = app
        .world_mut()
        .spawn((Node::default(), Interaction::Hovered))
        .id();

    let mut wheel = app.world_mut().resource_mut::<Messages<MouseWheel>>();
    wheel.write(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 5.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });

    app.update();

    let target_r_ui = app
        .world()
        .get::<PanOrbitCamera>(camera_ent)
        .unwrap()
        .target_radius;
    assert_eq!(
        target_r_ui, 16.0,
        "Mouse wheel over UI must be suppressed to prevent accidental background camera zoom!"
    );

    app.world_mut().despawn(ui_node);
}

#[test]
fn test_camera_smooth_interplanetary_flight_on_selection() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.init_resource::<Time>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    let jupiter_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Jupiter".to_string(),
                body_type: BodyType::GasGiant,
            },
            Mass(0.000954),
            Radius(0.000477),
            SimPosition(bevy::math::DVec3::new(5.2, 0.0, 0.0)),
            Transform::from_xyz(5.2, 0.0, 0.0),
        ))
        .id();

    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                focus: Vec3::ZERO,
                target_focus: Vec3::ZERO,
                radius: 16.0,
                target_radius: 16.0,
                focus_transition_progress: 1.0,
                focus_transition_start: Vec3::ZERO,
                ..default()
            },
            Transform::from_xyz(0.0, 5.0, 16.0),
            GlobalTransform::IDENTITY,
        ))
        .id();

    let mut sched = Schedule::default();
    sched.add_systems(update_pan_orbit_camera);

    // Initial frame at origin
    {
        let mut t = app.world_mut().resource_mut::<Time>();
        t.advance_by(std::time::Duration::from_millis(16));
    }
    sched.run(app.world_mut());
    let cam = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    assert_eq!(cam.focus, Vec3::ZERO);

    // Select Jupiter (> 0.05 AU away)
    {
        let mut cam_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam_mut.target_entity = Some(jupiter_ent);
    }

    // Step 1: Camera begins interplanetary smooth flight
    {
        let mut t = app.world_mut().resource_mut::<Time>();
        t.advance_by(std::time::Duration::from_millis(16));
    }
    sched.run(app.world_mut());
    let cam_step1 = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    assert!(
        cam_step1.focus_transition_progress < 1.0,
        "Camera must initiate smooth transition when targeting distant body"
    );
    assert_eq!(cam_step1.focus_transition_start, Vec3::ZERO);
    assert!(
        cam_step1.focus.x >= 0.0 && cam_step1.focus.x <= 5.2,
        "Focus must interpolate towards Jupiter"
    );

    // Step 2: Advance frames until transition completes (35 * 16ms = 0.56s > 0.45s)
    for _ in 0..35 {
        {
            let mut t = app.world_mut().resource_mut::<Time>();
            t.advance_by(std::time::Duration::from_millis(16));
        }
        sched.run(app.world_mut());
    }

    let cam_final = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    assert_eq!(
        cam_final.focus_transition_progress, 1.0,
        "Transition must complete and lock to target"
    );
    assert!(
        (cam_final.focus.x - 5.2).abs() < 1e-4,
        "Camera focus must lock tightly onto Jupiter at 5.2 AU"
    );
}

#[test]
fn test_surface_horizon_atmospheric_entry_transition() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::game::ui::NotificationToast;
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::rendering::surface_camera::{
        toggle_surface_horizon_mode, update_surface_horizon_camera,
    };
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.init_resource::<Time>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<NotificationToast>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(3e-6),
            Radius(4.25875e-5),
            Temperature(288.0),
            Composition::default(),
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
        ))
        .id();

    let orbital_pos = Vec3::new(1.0, 0.05, 0.1);
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: Some(earth_ent),
                focus: Vec3::new(1.0, 0.0, 0.0),
                target_focus: Vec3::new(1.0, 0.0, 0.0),
                radius: 0.1,
                target_radius: 0.1,
                surface_horizon_mode: false,
                entry_transition_progress: 1.0,
                ..default()
            },
            Transform::from_translation(orbital_pos),
            Projection::Perspective(PerspectiveProjection::default()),
        ))
        .id();

    let mut sched = Schedule::default();
    sched.add_systems(update_surface_horizon_camera);

    // Activate Surface Horizon Mode via toggle
    {
        let mut toast = NotificationToast::default();
        let mut cam = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        toggle_surface_horizon_mode(&mut cam, Some(earth_ent), &mut toast);
        assert!(cam.surface_horizon_mode);
        assert_eq!(cam.entry_transition_progress, 0.0);
    }

    // Step 1: Camera begins atmospheric entry descent
    {
        let mut t = app.world_mut().resource_mut::<Time>();
        t.advance_by(std::time::Duration::from_millis(16));
    }
    sched.run(app.world_mut());
    let cam1 = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    assert!(
        cam1.entry_transition_progress > 0.0 && cam1.entry_transition_progress <= 1.0,
        "Atmospheric entry progress must advance during descent"
    );

    // Step 2: Complete descent over subsequent frames (70 * 16ms = 1.12s > 1.0s)
    for _ in 0..70 {
        {
            let mut t = app.world_mut().resource_mut::<Time>();
            t.advance_by(std::time::Duration::from_millis(16));
        }
        sched.run(app.world_mut());
    }

    let cam_landed = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    let tf_landed = *app.world().get::<Transform>(camera_ent).unwrap();
    assert_eq!(
        cam_landed.entry_transition_progress, 1.0,
        "Entry descent must complete and reach 1.0"
    );
    // Camera has descended significantly closer to Earth center than the orbital vantage point
    let dist_landed = tf_landed.translation.distance(Vec3::new(1.0, 0.0, 0.0));
    let dist_orbital = orbital_pos.distance(Vec3::new(1.0, 0.0, 0.0));
    assert!(
        dist_landed < dist_orbital * 0.5,
        "Landed surface camera ({dist_landed}) must descend well below orbital vantage ({dist_orbital})"
    );
}
