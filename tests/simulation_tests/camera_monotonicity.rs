//! Interplanetary camera flight and strictly monotonic apparent angular size regression tests.

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::rendering::bodies::sync_camera_adaptive_body_scales;
use protostellar::rendering::bodies::transforms::{
    compute_close_camera_framing_radius, compute_realistic_visual_radius,
};
use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
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
    {
        let mut cam = app.world_mut().get_mut::<PanOrbitCamera>(cam_ent).unwrap();
        cam.initiate_target_transition(target_ent, target_pos, target_r);
    }

    let init_cam_tf = *app.world().get::<Transform>(cam_ent).unwrap();
    let init_body_tf = *app.world().get::<Transform>(target_ent).unwrap();
    let mut prev_dist = init_cam_tf.translation.distance(target_pos);
    let mut prev_theta = init_body_tf.scale.x / prev_dist;
    let mut prev_fov_z: Option<f32> = None;

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
            cam_state.radius <= target_r + 1e-5,
            "Frame {frame}: camera.radius bounced outward to {} (expected <= {target_r})!",
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
                "Apparent angular size Theta(d) must strictly increase as camera approaches {:?} (at d={d}: {theta} <= {prev_theta})",
                btype
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
        Transform::from_scale(Vec3::splat(0.015)),
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
            Transform::from_translation(earth_pos).with_scale(Vec3::splat(0.00018)),
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
            Transform::from_translation(mars_pos).with_scale(Vec3::splat(0.0029)),
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
