//! Track 5 verification tests: First-Person Surface Mode camera stability,
//! tilted spin-frame lock, scale invariance, rover autonomous-wander pause,
//! host planet hi-res mesh promotion, and close-up shader micro-detail.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::rendering::bodies::meshes::sync_selected_body_mesh_lod;
use protostellar::rendering::bodies::transforms::{
    clamp_surface_mode_spin_dt_yr, compute_realistic_visual_radius,
    resolve_surface_mode_visual_radius,
};
use protostellar::rendering::bodies::{VisualAssets, VisualBody};
use protostellar::rendering::camera::PanOrbitCamera;
use protostellar::rendering::surface_camera::{
    compute_surface_tangent_basis, lat_lon_to_unspun_normal, resolve_host_planet_spin_state,
    resolve_surface_world_up_and_heading,
};
use protostellar::simulation::components::celestial::SpinState;
use protostellar::simulation::components::{
    BodyType, CelestialBody, Radius, SimPosition, SimVelocity,
};
use protostellar::simulation::probes::{
    FlightComputer, ProbeStatus, ProbeType, SpaceProbe, navigation::probe_navigation_system,
};
use protostellar::simulation::resources::{
    PlayerInteractionState, SimTime, SimulationConfig, TimeWarp,
};
use protostellar::utils::constants::EARTH_RADIUS_AU;

fn shader_get_spun_p_surf(world_normal: Vec3, spin_axis: Vec3, spin_angle: f32) -> Vec3 {
    let s_axis = spin_axis.normalize();
    let s_ref = if s_axis.z.abs() > 0.95 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let s_tangent_x = s_axis.cross(s_ref).normalize();
    let s_tangent_z = s_tangent_x.cross(s_axis);
    let p_tilted = Vec3::new(
        world_normal.dot(s_tangent_x),
        world_normal.dot(s_axis),
        world_normal.dot(s_tangent_z),
    );
    let (s, c) = spin_angle.sin_cos();
    Vec3::new(
        p_tilted.x * c - p_tilted.z * s,
        p_tilted.y,
        p_tilted.x * s + p_tilted.z * c,
    )
}

#[test]
fn test_surface_mode_tilted_spin_frame_lock_and_zero_heading_precession() {
    let unspun_norm = lat_lon_to_unspun_normal(0.42, -1.15);
    let heading_unspun = Vec3::Y;

    // Test across Earth (23.44 deg), Mars (25.19 deg), Saturn (26.73 deg), and Uranus (97.77 deg)
    for tilt_deg in [0.0_f64, 23.44, 25.19, 26.73, 97.77] {
        let tilt_rad = tilt_deg.to_radians();
        let spin_vec = DVec3::new(tilt_rad.sin(), tilt_rad.cos(), 0.15).normalize();
        let spin_state = SpinState {
            spin_vector: spin_vec,
            rotation_period_hours: 24.0,
            axial_tilt_degrees: tilt_deg,
        };

        let mut prev_heading: Option<Vec3> = None;
        for step in 0..32 {
            let elapsed_yr = f64::from(step) * 0.00019;
            let (spin_dir, spin_angle, spin_axis_v4) =
                resolve_host_planet_spin_state(Some(&spin_state), elapsed_yr, None, None);
            let (world_up, world_heading) = resolve_surface_world_up_and_heading(
                unspun_norm,
                heading_unspun,
                spin_dir,
                spin_angle,
                spin_axis_v4,
                false,
            );

            // 1. Shader round-trip: world_up mapped through planet.wesl's get_spun_p_surf must
            // recover the exact unspun_norm continent/crater coordinate at every spin angle and tilt!
            let recovered_p_surf =
                shader_get_spun_p_surf(world_up, spin_axis_v4.xyz(), spin_angle as f32);
            assert!(
                recovered_p_surf.distance(unspun_norm) < 1e-4,
                "Tilt {tilt_deg}° step {step}: world_up must stay 100% locked to p_surf ({unspun_norm:?} vs {recovered_p_surf:?})"
            );

            // 2. Zero heading precession: world_heading must remain locked to spin_dir without wobbling in a cone
            assert!(
                world_heading.distance(spin_dir.as_vec3()) < 1e-5,
                "Tilt {tilt_deg}° step {step}: world_heading must align with spin_dir without precession cone wobble"
            );
            if let Some(prev) = prev_heading {
                assert!(
                    prev.distance(world_heading) < 1e-5,
                    "world_heading must remain time-invariant across planetary rotation"
                );
            }
            prev_heading = Some(world_heading);

            // 3. Tangent basis must remain strictly orthonormal across all spin angles
            let (east, up, north) = compute_surface_tangent_basis(world_up, world_heading);
            assert!((east.length() - 1.0).abs() < 1e-5);
            assert!((up.length() - 1.0).abs() < 1e-5);
            assert!((north.length() - 1.0).abs() < 1e-5);
            assert!(east.dot(up).abs() < 1e-5);
            assert!(north.dot(up).abs() < 1e-5);
            assert!(east.dot(north).abs() < 1e-5);
        }
    }
}

#[test]
fn test_surface_mode_visual_radius_clamps_orbital_inflation_and_spin_dt_cap() {
    let config = SimulationConfig::default();
    let close_r = compute_realistic_visual_radius(
        EARTH_RADIUS_AU,
        BodyType::TerrestrialPlanet,
        1.0,
        1.0,
        0.0,
        &config,
    );

    // 1. An inflated orbital transform scale (e.g. 0.0029 AU from far orbital view) must be clamped
    // to close_r so the surface camera never bounces when the planet moves along its orbit.
    let inflated_tf = Transform::from_scale(Vec3::splat(0.0029));
    let resolved_inflated = resolve_surface_mode_visual_radius(
        Some(&inflated_tf),
        EARTH_RADIUS_AU,
        BodyType::TerrestrialPlanet,
        1.0,
        &config,
    );
    assert!(
        (resolved_inflated - close_r).abs() < 1e-6,
        "Surface mode visual radius must clamp out orbital scale inflation ({resolved_inflated} vs {close_r})"
    );

    // 2. A smaller stripped-core transform scale must be preserved
    let stripped_tf = Transform::from_scale(Vec3::splat(close_r * 0.65));
    let resolved_stripped = resolve_surface_mode_visual_radius(
        Some(&stripped_tf),
        EARTH_RADIUS_AU,
        BodyType::TerrestrialPlanet,
        1.0,
        &config,
    );
    assert!(
        (resolved_stripped - close_r * 0.65).abs() < 1e-6,
        "Surface mode visual radius must preserve smaller stripped-core scale"
    );

    // 3. Spin dt freeze on the active surface host (eliminating 1 AU f32 mantissa staircase bounce)
    let earth_spin = SpinState {
        spin_vector: DVec3::Y,
        rotation_period_hours: 24.0,
        axial_tilt_degrees: 23.44,
    };
    let raw_dt_yr = 0.80; // High 50x orbital time-warp frame step
    let capped_dt = clamp_surface_mode_spin_dt_yr(raw_dt_yr, Some(&earth_spin), true);
    assert_eq!(
        capped_dt, 0.0,
        "Surface host spin dt must be frozen (0.0) while standing on the surface to prevent f32 ULP staircase bounce"
    );
    let uncapped_dt = clamp_surface_mode_spin_dt_yr(raw_dt_yr, Some(&earth_spin), false);
    assert_eq!(
        uncapped_dt, raw_dt_yr,
        "Non-surface bodies must retain full orbital spin dt"
    );
}

#[test]
fn test_surface_mode_rover_pauses_autonomous_wandering_and_upgrades_host_hires_mesh() {
    let mut app = App::new();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<Assets<Mesh>>();

    let low_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Sphere::new(1.0).mesh().uv(8, 8));
    let hi_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Sphere::new(1.0).mesh().uv(16, 16));
    let mut va = VisualAssets::dummy(low_mesh.clone());
    va.planet_mesh = low_mesh.clone();
    va.planet_mesh_hires = hi_mesh.clone();
    app.insert_resource(va);

    let earth = app
        .world_mut()
        .spawn((
            VisualBody,
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Radius(EARTH_RADIUS_AU),
            Mesh3d(low_mesh.clone()),
            Transform::from_scale(Vec3::splat(0.00018)),
        ))
        .id();

    let initial_unspun = DVec3::new(0.0, 0.6, 0.8).normalize();
    let initial_heading = DVec3::new(1.0, 0.0, 0.0);
    let rover = app
        .world_mut()
        .spawn((
            SpaceProbe {
                probe_type: ProbeType::Rover,
                status: ProbeStatus::Landed,
                id: 1,
                orientation: bevy::math::DQuat::IDENTITY,
                landed_unspun_pos: Some(initial_unspun),
                orbit_axis: None,
                arrival_time: Some(0.0),
                rover_heading_unspun: Some(initial_heading),
                distance_driven_km: 10.0,
                samples_collected: 2,
                traverse_trail_unspun: vec![initial_unspun],
                latest_assay: String::new(),
            },
            FlightComputer {
                target_entity: earth,
                target_name: "Earth".to_string(),
                launch_pos: DVec3::ZERO,
                distance_to_target: 0.0,
                relative_speed: 0.0,
            },
            SimPosition(DVec3::new(1.0, 0.0001, 0.00015)),
            SimVelocity(DVec3::ZERO),
        ))
        .id();

    // Select the Rover (not Earth directly) and enable surface_horizon_mode
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(rover);
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.25;
    app.world_mut().spawn(PanOrbitCamera {
        target_entity: Some(rover),
        surface_horizon_mode: true,
        ..default()
    });

    app.add_systems(
        Update,
        (probe_navigation_system, sync_selected_body_mesh_lod),
    );
    app.update();

    // 1. Landed rover must NOT wander autonomously or wiggle heading while surface_horizon_mode is true
    let probe_after = app.world().get::<SpaceProbe>(rover).unwrap();
    assert!(
        probe_after
            .landed_unspun_pos
            .unwrap()
            .distance(initial_unspun)
            < 1e-9,
        "Rover must not autonomously wander while player is in First-Person Surface Mode"
    );
    assert!(
        probe_after
            .rover_heading_unspun
            .unwrap()
            .distance(initial_heading)
            < 1e-9,
        "Rover heading must not autonomously oscillate while player is in First-Person Surface Mode"
    );

    // 2. Host planet (Earth) must be upgraded to planet_mesh_hires even when the Rover is the selected entity
    let earth_mesh = app.world().get::<Mesh3d>(earth).unwrap();
    assert_eq!(
        earth_mesh.0, hi_mesh,
        "Host planet must upgrade to planet_mesh_hires when its landed Rover is selected"
    );
}

#[test]
fn test_surface_mode_closeup_shader_micro_detail_and_horizon_ground_intersection() {
    let planet_src = std::fs::read_to_string("assets/shaders/planet.wesl")
        .expect("planet.wesl must be readable");
    assert!(
        planet_src.contains("fn evaluate_surface_closeup_micro_height(")
            && planet_src.contains("fn apply_surface_closeup_ground_detail(")
            && planet_src.contains("let closeup_relief_sun =")
            && planet_src.contains("let u_warped = mix(u, u * u * sqrt(u), closeup_warp);")
            && planet_src.contains("if (planet.planet_type != 8u && surface_closeup_norm < 0.5)")
            && planet_src.contains("if (orbital_cloud_vis > 0.005)"),
        "planet.wesl must include spherical cap vertex concentration, close-up micro-heightfield, and surface-mode fast paths"
    );

    let atmo_src = std::fs::read_to_string("assets/shaders/atmosphere.wesl")
        .expect("atmosphere.wesl must be readable");
    assert!(
        atmo_src.contains("if (!is_surface_view && hit_planet.x > 0.0)")
            && !atmo_src.contains("view_elev_sin < -0.10")
            && atmo_src.contains("@builtin(front_facing) is_front: bool")
            && atmo_src.contains("if (!is_front && cam_dist > r_atmo * max_stretch * 1.32)"),
        "atmosphere.wesl must march surface-view rays smoothly and discard occluded external backfaces early"
    );

    let circum_src = std::fs::read_to_string("assets/shaders/circumstellar.wesl")
        .expect("circumstellar.wesl must be readable");
    assert!(
        circum_src.contains("@builtin(front_facing) is_front: bool")
            && circum_src.contains("if (!is_front && length(cam_pos - center) > r_outer * 0.99)"),
        "circumstellar.wesl must discard external backfaces before raymarching"
    );

    let noise_src = std::fs::read_to_string("src/rendering/shaders/noise.wesl")
        .expect("noise.wesl must be readable");
    assert!(
        noise_src.contains("if (blend <= 0.001)"),
        "noise.wesl fbm_lod and eroded_continent_fbm_lod must early-exit when octaves blend to 0.5"
    );

    let skybox_src =
        std::fs::read_to_string("assets/shaders/skybox.wesl").expect("skybox.wesl must read");
    assert!(
        skybox_src.contains("if (abs_b < 0.42)")
            && skybox_src.contains("smoothstep(0.42, 0.24, abs_b)")
            && skybox_src.contains("if (sgr_a_dist2 < 1.6)")
            && skybox_src.contains("smoothstep(1.6, 0.9, sgr_a_dist2)"),
        "skybox.wesl must smoothly taper galactic-plane dust/nebula FBM at high latitude and evaluate the Sagittarius A* bulge with a smooth spherical falloff"
    );

    let neb_src = std::fs::read_to_string("assets/shaders/volumetric_nebula.wesl")
        .expect("volumetric_nebula.wesl must be readable");
    assert!(
        neb_src.contains("if (density > 1e-4 || live_shock_heat > 1e-4)")
            && neb_src.contains("if (t_spiral > 0.001)"),
        "volumetric_nebula.wesl must guard empty-space star lighting and early spiral trig"
    );
}

#[test]
fn test_hires_icosphere_mesh_density_and_idle_tools_change_detection() {
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::game::ui::{NotificationToast, PlanetBuilderState, TelemetryPanelState};
    use protostellar::rendering::bodies::meshes::setup_visual_assets;
    use protostellar::simulation::resources::DiskParameters;
    use protostellar::simulation::scenarios::LoadScenarioEvent;

    // 1. Verify setup_visual_assets creates genuinely smooth ico(48) / ico(16) spheres
    let mut mesh_app = App::new();
    mesh_app.init_resource::<Assets<Mesh>>();
    mesh_app.add_systems(Startup, setup_visual_assets);
    mesh_app.update();

    let va = mesh_app.world().resource::<VisualAssets>();
    let meshes = mesh_app.world().resource::<Assets<Mesh>>();
    let hires_verts = meshes.get(&va.planet_mesh_hires).unwrap().count_vertices();
    let planet_verts = meshes.get(&va.planet_mesh).unwrap().count_vertices();
    let atmo_verts = meshes.get(&va.atmosphere_mesh).unwrap().count_vertices();
    assert!(
        hires_verts >= 20_000,
        "planet_mesh_hires (ico 48) must have >= 20,000 vertices for circular horizons, got {hires_verts}"
    );
    assert!(
        planet_verts >= 2_500 && atmo_verts >= 2_500,
        "Standard planet and atmosphere meshes (ico 16) must have >= 2,500 vertices, got {planet_verts}/{atmo_verts}"
    );

    // 2. Verify handle_player_tools does NOT mark UI/simulation resources changed when idle
    let mut tools_app = App::new();
    tools_app.init_resource::<ButtonInput<KeyCode>>();
    tools_app.init_resource::<DiskParameters>();
    tools_app.init_resource::<SimulationConfig>();
    tools_app.init_resource::<PlayerInteractionState>();
    tools_app.init_resource::<LateHeavyBombardmentState>();
    tools_app.init_resource::<PlanetBuilderState>();
    tools_app.init_resource::<TelemetryPanelState>();
    tools_app.init_resource::<NotificationToast>();
    tools_app.add_message::<LoadScenarioEvent>();
    tools_app.add_systems(Update, protostellar::game::interaction::handle_player_tools);

    // Frame 1 clears initial Added/Changed flags via clear_trackers on frame advance
    tools_app.update();
    tools_app.update();
    assert!(
        !tools_app
            .world()
            .is_resource_changed::<PlayerInteractionState>(),
        "Idle handle_player_tools must not poison PlayerInteractionState change detection"
    );
    assert!(
        !tools_app.world().is_resource_changed::<SimulationConfig>(),
        "Idle handle_player_tools must not poison SimulationConfig change detection"
    );
    assert!(
        !tools_app
            .world()
            .is_resource_changed::<PlanetBuilderState>(),
        "Idle handle_player_tools must not poison PlanetBuilderState change detection"
    );
}

#[test]
fn test_surface_mode_latches_f32_world_position_against_orbital_ulp_bounce() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use protostellar::game::ui::NotificationToast;
    use protostellar::rendering::camera::update_pan_orbit_camera;
    use protostellar::rendering::surface_camera::update_surface_horizon_camera;
    use protostellar::simulation::components::{Composition, Mass, Temperature};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<PlayerInteractionState>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.world_mut().spawn(Window::default());

    let earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(3.0e-6),
            Radius(EARTH_RADIUS_AU),
            Temperature(385.0),
            Composition::rocky(),
            SimPosition(DVec3::new(0.97, 0.0, 0.0)),
            Transform::from_xyz(0.97, 0.0, 0.0).with_scale(Vec3::splat(0.00018)),
        ))
        .id();

    let cam_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: Some(earth),
                focus: Vec3::new(0.97, 0.0, 0.0),
                target_focus: Vec3::new(0.97, 0.0, 0.0),
                surface_horizon_mode: true,
                entry_transition_progress: 1.0,
                ..default()
            },
            Transform::from_xyz(0.97018, 0.0, 0.0),
            GlobalTransform::default(),
            Projection::Perspective(PerspectiveProjection::default()),
        ))
        .id();

    app.add_systems(
        Update,
        (update_pan_orbit_camera, update_surface_horizon_camera).chain(),
    );
    app.update();

    let initial_cam_pos = app.world().get::<Transform>(cam_ent).unwrap().translation;

    // Advance Earth's f64 SimPosition at 30.4 km/s (2.03e-7 AU/s) across 60 frames (~1 second),
    // crossing multiple f32 ULP boundaries (5.96e-8 AU at 0.97 AU).
    for frame in 1..=60 {
        let mut e_pos = app.world_mut().get_mut::<SimPosition>(earth).unwrap();
        e_pos.0.x = 0.97 + f64::from(frame) * 3.4e-9;
        e_pos.0.z = f64::from(frame) * 3.4e-9;
        app.update();
        let cam_pos = app.world().get::<Transform>(cam_ent).unwrap().translation;
        assert_eq!(
            cam_pos, initial_cam_pos,
            "Frame {frame}: standing-still surface camera must stay bit-for-bit stationary without f32 ULP bounce"
        );
    }
}
