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
fn test_surface_horizon_camera_pose_and_rover_drive() {
    use protostellar::game::ui::NotificationToast;
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::rendering::surface_camera::{
        apply_manual_rover_drive, compute_surface_horizon_camera_pose,
        compute_surface_tangent_basis, toggle_surface_horizon_mode,
    };
    use protostellar::simulation::probes::{ProbeStatus, ProbeType, SpaceProbe};

    let (east, up, north) =
        compute_surface_tangent_basis(Vec3::new(0.6, 0.8, 0.0), Vec3::new(0.0, 0.0, 1.0));
    assert!((up.length() - 1.0).abs() < 1e-5);
    assert!((east.length() - 1.0).abs() < 1e-5);
    assert!((north.length() - 1.0).abs() < 1e-5);
    assert!(east.dot(up).abs() < 1e-5);
    assert!(north.dot(up).abs() < 1e-5);
    assert!(east.dot(north).abs() < 1e-5);

    let planet_pos = Vec3::new(1.0, 0.0, 0.0);
    let vis_rad = 0.025;
    let (eye_pos, eye_rot) =
        compute_surface_horizon_camera_pose(planet_pos, vis_rad, up, north, 0.022, 0.0, 0.25);
    let radial_dist = eye_pos.distance(planet_pos);
    assert!(
        (radial_dist - vis_rad * 1.022).abs() < 1e-5,
        "Surface horizon camera must sit at (1 + altitude_scale) * vis_rad above the planet center"
    );
    let cam_forward = eye_rot * -Vec3::Z;
    assert!(
        cam_forward.dot(up) > 0.15,
        "Positive elevation angle must tilt camera view upward toward the sky dome"
    );

    let mut cam = PanOrbitCamera::default();
    let mut toast = NotificationToast::default();
    let dummy_ent = Entity::from_bits(42);
    toggle_surface_horizon_mode(&mut cam, Some(dummy_ent), &mut toast);
    assert!(cam.surface_horizon_mode);
    assert_eq!(cam.target_entity, Some(dummy_ent));

    let mut rover = SpaceProbe {
        id: 1,
        probe_type: ProbeType::Rover,
        status: ProbeStatus::Landed,
        orientation: bevy::math::DQuat::IDENTITY,
        orbit_axis: Some(DVec3::Y),
        arrival_time: Some(1.0),
        landed_unspun_pos: Some(DVec3::X),
        rover_heading_unspun: Some(DVec3::Z),
        distance_driven_km: 0.0,
        samples_collected: 1,
        latest_assay: "Basaltic regolith".to_string(),
        traverse_trail_unspun: vec![DVec3::X],
    };
    apply_manual_rover_drive(&mut rover, 0.05, 0.02, EARTH_RADIUS_AU);
    assert!(
        rover.distance_driven_km > 100.0,
        "Manual WASD rover drive must advance distance_driven_km along the spherical surface"
    );
    assert!(
        rover.traverse_trail_unspun.len() >= 2,
        "Driving beyond angular threshold must record a new unspun breadcrumb waypoint"
    );

    // Verify ECS system parameter disjointness (no B0001 conflict) and runtime pose update
    let (mut app, camera_ent, earth_ent, _, _) = setup_camera_zoom_app();
    app.init_resource::<NotificationToast>();
    app.world_mut()
        .entity_mut(earth_ent)
        .insert((Temperature(288.0), Composition::default()));
    app.world_mut()
        .entity_mut(camera_ent)
        .insert(Projection::Perspective(PerspectiveProjection::default()));
    if let Some(mut cam_comp) = app.world_mut().get_mut::<PanOrbitCamera>(camera_ent) {
        cam_comp.target_entity = Some(earth_ent);
        cam_comp.surface_horizon_mode = true;
    }
    app.add_systems(
        Update,
        protostellar::rendering::surface_camera::update_surface_horizon_camera
            .after(protostellar::rendering::camera::update_pan_orbit_camera),
    );
    app.update();
    let cam_after = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    assert!(cam_after.surface_horizon_mode);
}

#[test]
fn test_3d_planetary_surface_terrain_and_vegetation() {
    use bevy::render::mesh::VertexAttributeValues;
    use protostellar::rendering::surface_terrain::*;

    let unspun_center = Vec3::Y;
    let vis_rad = 0.025f32;
    let comp = Composition {
        silicate_frac: 0.70,
        metal_frac: 0.20,
        ice_frac: 0.10,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let bio = BiosphereState {
        habitability_score: 0.95,
        biomass_coverage_frac: 0.75,
        flora_pigment: FloraPigmentClass::SolarChlorophyll,
        ..default()
    };

    // 1. Test Elevation & Canyon Carving across all 8 sphere octants (including negative coordinates)
    for sx in [-1.0_f32, 1.0] {
        for sy in [-1.0_f32, 1.0] {
            for sz in [-1.0_f32, 1.0] {
                let dir = Vec3::new(sx, sy, sz).normalize();
                let elev = evaluate_surface_elevation(
                    dir,
                    &comp,
                    288.0,
                    BodyType::TerrestrialPlanet,
                    vis_rad,
                );
                assert!(
                    elev.is_finite(),
                    "Surface elevation must be finite in octant {:?}",
                    (sx, sy, sz)
                );
                assert!(
                    elev > 0.0,
                    "Surface elevation must stay above base sphere radius (got {elev})"
                );
            }
        }
    }

    let (canyon_d, _) = evaluate_canyon_carve(Vec3::new(-0.5, -0.5, -0.5).normalize());
    assert!(
        canyon_d >= 0.0 && canyon_d.is_finite(),
        "Canyon depth factor must be finite and non-negative"
    );

    // Verify rocky Earth (ice_frac == 0.0) detects liquid oceans via BiosphereState / VolatileInventory
    let rocky_earth = Composition::rocky();
    assert!(
        planet_has_surface_liquids(
            BodyType::TerrestrialPlanet,
            &rocky_earth,
            288.0,
            Some(&bio),
            None
        ),
        "Rocky planet with active biosphere in liquid water range must detect surface liquids"
    );

    // 2. Test 3D Terrain Mesh Generation (at negative octant to verify Euclidean fract_pos)
    let neg_center = Vec3::new(-0.577, -0.577, -0.577).normalize();
    let terrain_mesh = generate_surface_terrain_mesh(
        neg_center,
        vis_rad,
        &comp,
        288.0,
        BodyType::TerrestrialPlanet,
        Some(&bio),
        vis_rad * 0.24,
    );
    assert_eq!(
        terrain_mesh.count_vertices(),
        121 * 121,
        "Terrain grid must have 121x121 vertices"
    );
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        terrain_mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        for pos in positions {
            assert!(
                pos[0].is_finite() && pos[1].is_finite() && pos[2].is_finite(),
                "Terrain vertex position must not be NaN: {pos:?}"
            );
        }
    }
    if let Some(VertexAttributeValues::Float32x4(cols)) =
        terrain_mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    {
        assert_eq!(cols.len(), 121 * 121);
        for col in cols {
            assert!(
                col[3] > 0.9,
                "Terrain vertex colors must have full alpha opacity"
            );
        }
    } else {
        panic!("Terrain mesh must contain ATTRIBUTE_COLOR");
    }

    // 3. Test 3D Vegetation Mesh Generation
    let veg_mesh = generate_surface_vegetation_mesh(
        unspun_center,
        vis_rad,
        &comp,
        288.0,
        BodyType::TerrestrialPlanet,
        Some(&bio),
        1.0,
        vis_rad * 0.24,
    );
    assert!(
        veg_mesh.count_vertices() > 50,
        "Vegetation mesh must generate 3D trunks and foliage vertices for living biosphere"
    );
    assert!(
        veg_mesh.indices().is_some(),
        "Vegetation mesh must include triangle indices"
    );

    // 4. Test 3D Water Plane Generation
    let water_mesh = generate_surface_water_mesh(unspun_center, vis_rad, vis_rad * 0.24);
    assert!(
        water_mesh.count_vertices() > 0,
        "Water plane mesh must have non-zero vertices"
    );
}

#[test]
fn test_3d_planetary_surface_ecs_lifecycle() {
    use protostellar::game::ui::NotificationToast;
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::rendering::surface_terrain::*;

    let comp = Composition {
        silicate_frac: 0.70,
        metal_frac: 0.20,
        ice_frac: 0.10,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let bio = BiosphereState {
        habitability_score: 0.95,
        biomass_coverage_frac: 0.75,
        flora_pigment: FloraPigmentClass::SolarChlorophyll,
        ..default()
    };

    let (mut app, camera_ent, earth_ent, _, _) = setup_camera_zoom_app();
    app.init_resource::<NotificationToast>();
    app.init_resource::<SurfaceTerrainState>();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.world_mut()
        .entity_mut(earth_ent)
        .insert((Temperature(288.0), comp, bio));

    // Turn surface horizon mode ON
    if let Some(mut cam_comp) = app.world_mut().get_mut::<PanOrbitCamera>(camera_ent) {
        cam_comp.target_entity = Some(earth_ent);
        cam_comp.surface_horizon_mode = true;
    }
    app.add_systems(
        Update,
        (
            protostellar::rendering::surface_camera::update_surface_horizon_camera,
            sync_surface_terrain_system,
        )
            .chain(),
    );
    app.update();

    let terrain_state = app.world().resource::<SurfaceTerrainState>();
    assert!(
        terrain_state.root_entity.is_some(),
        "Entering surface horizon mode must spawn PlanetarySurfaceTerrainRoot"
    );
    let spawned_root = terrain_state.root_entity.unwrap();
    assert!(
        app.world().get_entity(spawned_root).is_ok(),
        "Spawned root entity must exist in the world"
    );

    // Turn surface horizon mode OFF -> Must cleanly despawn
    if let Some(mut cam_comp) = app.world_mut().get_mut::<PanOrbitCamera>(camera_ent) {
        cam_comp.surface_horizon_mode = false;
    }
    app.update();

    let terrain_state_after = app.world().resource::<SurfaceTerrainState>();
    assert!(
        terrain_state_after.root_entity.is_none(),
        "Exiting surface horizon mode must reset root_entity"
    );
    assert!(
        app.world().get_entity(spawned_root).is_err(),
        "Exiting surface horizon mode must cleanly despawn PlanetarySurfaceTerrainRoot and children"
    );
}
