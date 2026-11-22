//! Interplanetary camera flight and strictly monotonic apparent angular size regression tests.

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::rendering::bodies::sync_camera_adaptive_body_scales;
use protostellar::rendering::bodies::transforms::{
    compute_close_camera_framing_radius, compute_realistic_visual_radius,
};
use protostellar::rendering::camera::{PanOrbitCamera, update_pan_orbit_camera};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU, SOLAR_RADIUS_AU};

fn assert_monotonic_flight_to_target(
    app: &mut App,
    sched: &mut Schedule,
    cam_ent: Entity,
    target_ent: Entity,
    target_pos: Vec3,
    target_r: f32,
) {
    let start_r = app.world().get::<PanOrbitCamera>(cam_ent).unwrap().radius;
    {
        let mut cam = app.world_mut().get_mut::<PanOrbitCamera>(cam_ent).unwrap();
        cam.initiate_target_transition(target_ent, target_pos, target_r);
    }

    let init_cam_tf = *app.world().get::<Transform>(cam_ent).unwrap();
    let init_body_tf = *app.world().get::<Transform>(target_ent).unwrap();
    let mut prev_dist = init_cam_tf.translation.distance(target_pos);
    let mut prev_theta = init_body_tf.scale.x / prev_dist;
    let mut prev_fov_z: Option<f32> = None;
    let r_min_bound = start_r.min(target_r) - 1e-6;
    let r_max_bound = start_r.max(target_r) + 1e-5;

    for frame in 0..40 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(16));
        sched.run(app.world_mut());

        let cam_tf = *app.world().get::<Transform>(cam_ent).unwrap();
        let cam_state = app.world().get::<PanOrbitCamera>(cam_ent).unwrap();
        let body_tf = *app.world().get::<Transform>(target_ent).unwrap();

        let dist = cam_tf.translation.distance(target_pos);
        let theta = body_tf.scale.x / dist;

        assert!(
            dist <= prev_dist + 1e-6,
            "Frame {frame}: Camera distance backed out ({prev_dist} -> {dist})!"
        );
        assert!(
            dist >= target_r - 1e-6,
            "Frame {frame}: Camera plunged closer than final framing ({dist} < {target_r})!"
        );
        assert!(
            cam_state.radius >= r_min_bound && cam_state.radius <= r_max_bound,
            "Frame {frame}: camera.radius ({}) strayed outside monotonic interval [{r_min_bound}, {r_max_bound}]!",
            cam_state.radius
        );
        assert!(
            theta >= prev_theta - 1e-6,
            "Frame {frame}: Planet apparent angular size deflated ({prev_theta} -> {theta})!"
        );

        let to_target = target_pos - cam_tf.translation;
        let cam_fwd = cam_tf.rotation * -Vec3::Z;
        let cos_angle = to_target.normalize_or_zero().dot(cam_fwd);
        if cos_angle >= 37.0_f32.to_radians().cos() {
            let view_z = to_target.dot(cam_fwd);
            if let Some(pz) = prev_fov_z {
                assert!(
                    view_z <= pz + 1e-5,
                    "Frame {frame}: In-FOV perspective Z-depth backed out ({pz} -> {view_z})!"
                );
            }
            prev_fov_z = Some(view_z);
        }

        prev_dist = dist;
        prev_theta = theta;
    }

    assert!(
        (prev_dist - target_r).abs() < 1e-5,
        "Flight must settle at exact target framing radius ({prev_dist} vs {target_r})"
    );
}

#[test]
fn test_realistic_visual_radius_strictly_monotonic_angular_size() {
    let config = SimulationConfig::default();

    // Analytical check: Theta(d) = R_vis(d) / d must strictly increase as d decreases
    for &(phys_r, btype, r_orb) in &[
        (EARTH_RADIUS_AU, BodyType::TerrestrialPlanet, 1.0_f32),
        (EARTH_RADIUS_AU * 0.532, BodyType::TerrestrialPlanet, 1.52),
        (EARTH_RADIUS_AU * 11.2, BodyType::GasGiant, 5.2),
        (SOLAR_RADIUS_AU, BodyType::MainSequenceStar, 0.0),
    ] {
        let mut prev_theta = 0.0_f32;
        for i in 0..=400 {
            let frac = i as f32 / 400.0;
            // Sweep logarithmically from 30.0 AU down to 0.00025 AU
            let d = 30.0_f32 * (0.00025_f32 / 30.0_f32).powf(frac);
            let r_vis = compute_realistic_visual_radius(phys_r, btype, r_orb, 0.39, d, &config);
            let theta = r_vis / d;
            assert!(
                theta > prev_theta,
                "Apparent angular size Theta(d) must strictly increase as camera approaches {btype:?} (at d={d}: {theta} <= {prev_theta})"
            );
            prev_theta = theta;
        }
    }
}

#[test]
fn test_tab_planet_transition_strictly_monotonic_apparent_angular_size() {
    let config = SimulationConfig::default();
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<SimulationConfig>()
        .init_resource::<PlayerInteractionState>()
        .add_message::<MouseMotion>()
        .add_message::<MouseWheel>();

    app.world_mut().spawn(Window {
        title: "Monotonicity Test Window".to_string(),
        ..default()
    });

    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::MainSequenceStar,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        SimPosition(DVec3::ZERO),
        Transform::from_scale(Vec3::splat(SOLAR_RADIUS_AU as f32)),
    ));

    let earth_pos = Vec3::new(0.0, 0.0, 1.0);
    let mars_pos = Vec3::new(0.0, 0.0, 1.52);
    let mars_rad_au = EARTH_RADIUS_AU * 0.532;

    let earth_frame_r = compute_close_camera_framing_radius(
        EARTH_RADIUS_AU,
        BodyType::TerrestrialPlanet,
        1.0,
        1.0,
        &config,
    );
    let mars_frame_r = compute_close_camera_framing_radius(
        mars_rad_au,
        BodyType::TerrestrialPlanet,
        1.52,
        1.0,
        &config,
    );

    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(0.0, 0.0, 1.0)),
            Transform::from_translation(earth_pos).with_scale(Vec3::splat(EARTH_RADIUS_AU as f32)),
        ))
        .id();

    let mars_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Mars".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR * 0.107),
            Radius(mars_rad_au),
            SimPosition(DVec3::new(0.0, 0.0, 1.52)),
            Transform::from_translation(mars_pos).with_scale(Vec3::splat(mars_rad_au as f32)),
        ))
        .id();

    let init_cam = PanOrbitCamera {
        target_entity: Some(earth_ent),
        previous_target_entity: Some(earth_ent),
        focus: earth_pos,
        target_focus: earth_pos,
        radius: earth_frame_r,
        target_radius: earth_frame_r,
        yaw: 0.0,
        target_yaw: 0.0,
        pitch: 0.62,
        target_pitch: 0.62,
        ..default()
    };
    let init_eye = init_cam.current_orbital_eye();
    let cam_ent = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            Transform::from_translation(init_eye).with_rotation(init_cam.orbital_rotation()),
            GlobalTransform::IDENTITY,
            init_cam,
        ))
        .id();

    let mut sched = Schedule::default();
    sched.add_systems((update_pan_orbit_camera, sync_camera_adaptive_body_scales).chain());

    // Settle initial frame on Earth
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(16));
    sched.run(app.world_mut());

    // Verify 100% monotonic distance, angular size, and in-FOV Z-depth in both hemispheres
    assert_monotonic_flight_to_target(
        &mut app,
        &mut sched,
        cam_ent,
        mars_ent,
        mars_pos,
        mars_frame_r,
    );
    assert_monotonic_flight_to_target(
        &mut app,
        &mut sched,
        cam_ent,
        earth_ent,
        earth_pos,
        earth_frame_r,
    );
}

#[test]
fn test_render_origin_centers_focused_planet_and_preserves_earth_moon_separation() {
    use protostellar::rendering::bodies::{
        VisualAssets, VisualBody, transforms::sync_celestial_transforms,
    };
    use protostellar::rendering::origin::{RenderOrigin, update_render_origin};

    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<SimulationConfig>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<RenderOrigin>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<protostellar::rendering::materials::PlanetMaterial>>();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1.0, 1.0, 1.0));
    app.insert_resource(VisualAssets::dummy(mesh));
    app.add_systems(
        Update,
        (update_render_origin, sync_celestial_transforms).chain(),
    );

    let earth_sim = DVec3::new(1.0, 0.0, 0.0);
    let moon_offset = DVec3::new(0.00257, 0.0, 0.0);
    let planet_nine_sim = DVec3::new(380.0, 0.0, -145.0);

    let earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(earth_sim),
            Radius(EARTH_RADIUS_AU),
            Transform::default(),
            VisualBody,
        ))
        .id();
    let moon = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "The Moon".to_string(),
                body_type: BodyType::Moon,
            },
            SimPosition(earth_sim + moon_offset),
            Radius(EARTH_RADIUS_AU * 0.272),
            Transform::default(),
            VisualBody,
        ))
        .id();
    let planet_nine = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Planet Nine".to_string(),
                body_type: BodyType::IceGiant,
            },
            SimPosition(planet_nine_sim),
            Radius(EARTH_RADIUS_AU * 3.2),
            Transform::default(),
            VisualBody,
        ))
        .id();

    let mut cam = PanOrbitCamera {
        target_entity: Some(earth),
        previous_target_entity: Some(earth),
        focus_transition_progress: 1.0,
        ..default()
    };
    cam.set_focus_au(earth_sim);
    cam.set_target_focus_au(earth_sim);
    let cam_ent = app.world_mut().spawn((Camera3d::default(), cam)).id();

    app.update();

    let origin = *app.world().resource::<RenderOrigin>();
    let earth_r = origin.to_render(earth_sim);
    let moon_r = origin.to_render(earth_sim + moon_offset);
    assert!(
        earth_r.length() < 1e-9,
        "Focused Earth must sit at render-space origin, got {earth_r:?}"
    );
    assert!(
        (moon_r.distance(earth_r) - 0.00257).abs() < 1e-7,
        "Earth-Moon render distance must be 0.00257 AU, got {}",
        moon_r.distance(earth_r)
    );

    {
        let mut cam_mut = app.world_mut().get_mut::<PanOrbitCamera>(cam_ent).unwrap();
        cam_mut.target_entity = Some(planet_nine);
        cam_mut.previous_target_entity = Some(planet_nine);
        cam_mut.focus_transition_progress = 1.0;
        cam_mut.set_focus_au(planet_nine_sim);
        cam_mut.set_target_focus_au(planet_nine_sim);
    }
    app.update();

    let origin = *app.world().resource::<RenderOrigin>();
    assert!(
        origin.to_render(planet_nine_sim).length() < 1e-9,
        "Focused Planet Nine at 380 AU must sit at render-space origin"
    );
    let _ = (moon, earth);
}
