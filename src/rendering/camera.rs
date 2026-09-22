//! 3D Orbital Camera with logarithmic zoom, focus-lock, and smooth interpolation.

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Orbital camera controller component with smooth cinematic inertial damping.
#[derive(Component, Debug, Clone)]
pub struct PanOrbitCamera {
    /// 3D focus point in simulation render coordinates (AU)
    pub focus: Vec3,
    pub target_focus: Vec3,
    /// Distance from focus point in AU
    pub radius: f32,
    pub target_radius: f32,
    /// Horizontal rotation (radians)
    pub yaw: f32,
    pub target_yaw: f32,
    /// Vertical pitch (radians)
    pub pitch: f32,
    pub target_pitch: f32,
    /// Target entity to track (if focus-locked)
    pub target_entity: Option<Entity>,
    /// Minimum zoom distance in AU (~10,000 km)
    pub min_radius: f32,
    /// Maximum zoom distance in AU (150 AU outer solar system)
    pub max_radius: f32,
    /// Sensitivity multipliers
    pub orbit_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub pan_sensitivity: f32,
}

impl Default for PanOrbitCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            target_focus: Vec3::ZERO,
            radius: 16.0, // 16 AU framing of inner and giant solar system
            target_radius: 16.0,
            yaw: 0.785, // 45 degrees
            target_yaw: 0.785,
            pitch: 0.62, // ~36 degrees inclination
            target_pitch: 0.62,
            target_entity: None,
            min_radius: 0.005,
            max_radius: 250_000.0,
            orbit_sensitivity: 0.005,
            zoom_sensitivity: 0.055,
            pan_sensitivity: 0.02,
        }
    }
}

/// Spawns the 3D camera.
pub fn setup_camera(mut commands: Commands) {
    let pan_orbit = PanOrbitCamera::default();

    let rot = Quat::from_axis_angle(Vec3::Y, pan_orbit.yaw)
        * Quat::from_axis_angle(Vec3::X, -pan_orbit.pitch);
    let translation = pan_orbit.focus + rot * Vec3::new(0.0, 0.0, pan_orbit.radius);

    // Single camera: 3D scene + UI overlay with HDR bloom and filmic tonemapping.
    //
    // In Bevy 0.19, the `Hdr` component activates floating-point render targets,
    // allowing fragment colours > 1.0 to accumulate without clamping. The bloom
    // prefilter threshold (1.8) is set above the SDR ceiling (1.0), so only genuinely
    // incandescent surfaces (stars, lava, aurorae, accretion disks) contribute to the
    // bloom halo. Threshold-softness (0.4) avoids a hard pop-in at the knee.
    // TonyMcMapface is a filmic display transform that preserves hue even at extreme
    // HDR values — critical for accurate star colours. DebandDither prevents gradient banding.
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Camera::default(),
        Projection::Perspective(PerspectiveProjection {
            near: 0.0001,
            far: 2_000_000.0,
            ..default()
        }),
        Tonemapping::TonyMcMapface,
        DebandDither::Enabled,
        Bloom {
            // Controls how much overall glow energy the star emits.
            // 0.28 gives a radiant corona without washing out nearby planets.
            intensity: 0.28,
            // Low-frequency broad halo (mimics real optical lens glow &
            // coronagraph diffraction rings around bright stars).
            low_frequency_boost: 0.55,
            low_frequency_boost_curvature: 0.88,
            // High-frequency tight glints (specular ocean sunglint, lightning,
            // lava fountains, aurora curtains).
            high_pass_frequency: 1.0,
            prefilter: BloomPrefilter {
                // Only emit bloom above SDR white (> 1.8 nits) — stars and
                // lava surpass this; dark rocky planets never trigger it.
                threshold: 1.8,
                threshold_softness: 0.40,
            },
            composite_mode: BloomCompositeMode::Additive,
            ..default()
        },
        Transform {
            translation,
            rotation: rot,
            ..default()
        },
        pan_orbit,
        IsDefaultUiCamera,
    ));
    info!("✅ setup_camera: Spawned Camera3d with IsDefaultUiCamera, HDR Bloom, TonyMcMapface");
}

/// Configures Bevy's Gizmo rendering parameters:
/// - depth_bias: -1.0 so that orbit ribbons, conic tracks, targeting reticles, and AU rings
///   are never occluded or clipped by 3D volumetric gas planes, planet spheres, or depth buffer limits.
/// - line.width: 2.5 for crisp, radiant, high-visibility vector graphics.
pub fn setup_gizmo_configuration(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;
    config.line.width = 2.5;
    info!("✅ setup_gizmo_configuration: depth_bias set to -1.0, line.width set to 2.5");
}

fn is_little_red_dot(body: &CelestialBody) -> bool {
    body.body_type == BodyType::QuasiStar || body.name.to_lowercase().contains("little red dot")
}

fn update_camera_min_zoom_bounds(
    camera: &mut PanOrbitCamera,
    config: &SimulationConfig,
    targets_query: &Query<(Entity, &SimPosition, &Radius, &CelestialBody, &Mass)>,
) {
    let focused_info = if let Some(target_ent) = camera.target_entity {
        if let Ok((_, pos, radius, body, _mass)) = targets_query.get(target_ent) {
            let target_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            camera.target_focus = target_vec;
            let is_lrd = is_little_red_dot(body);
            Some((
                config.calc_visual_radius_for_type(radius.0, body.body_type),
                is_lrd,
            ))
        } else {
            camera.target_entity = None;
            None
        }
    } else {
        None
    };

    let effective_info = if let Some(info) = focused_info {
        Some(info)
    } else {
        targets_query
            .iter()
            .map(|(_, pos, radius, body, _)| {
                let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
                let dist = camera.target_focus.distance(center);
                let vis_rad = config.calc_visual_radius_for_type(radius.0, body.body_type);
                let is_lrd = is_little_red_dot(body);
                (dist, vis_rad, is_lrd)
            })
            .filter(|(dist, vis_rad, _)| *dist <= (*vis_rad * 2.5).max(0.35))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, vis_rad, is_lrd)| (vis_rad, is_lrd))
    };

    if let Some((vis_rad, is_lrd)) = effective_info {
        if is_lrd {
            camera.min_radius = 0.001;
        } else {
            camera.min_radius = config.calc_camera_min_zoom_radius(vis_rad);
        }
    } else {
        camera.min_radius = 0.001;
    }

    camera.target_radius = camera
        .target_radius
        .clamp(camera.min_radius, camera.max_radius);
    camera.radius = camera.radius.clamp(camera.min_radius, camera.max_radius);
}

fn handle_camera_target_picking(
    window: &Window,
    camera_comp: &Camera,
    global_transform: &GlobalTransform,
    targets_query: &Query<(Entity, &SimPosition, &Radius, &CelestialBody, &Mass)>,
    player_state: &mut PlayerInteractionState,
    camera: &mut PanOrbitCamera,
    config: &SimulationConfig,
    ui_interaction_query: &Query<&Interaction>,
    mouse_buttons: &ButtonInput<MouseButton>,
) {
    let cursor_over_ui = ui_interaction_query
        .iter()
        .any(|i| *i == Interaction::Pressed || *i == Interaction::Hovered);

    if cursor_over_ui
        || !mouse_buttons.just_pressed(MouseButton::Left)
        || player_state.active_tool != PlayerTool::Inspect
    {
        return;
    }

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let mut best_target: Option<Entity> = None;
    let mut best_score = f32::MAX;

    if let Ok(ray) = camera_comp.viewport_to_world(global_transform, cursor_pos) {
        for (entity, pos, _rad, body, _mass) in targets_query.iter() {
            let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            let hit_radius = if body.body_type.is_star_or_remnant() {
                4.50f32
            } else if matches!(body.body_type, BodyType::GasGiant | BodyType::IceGiant) {
                2.50f32
            } else {
                1.50f32
            };
            let to_center = center - ray.origin;
            let proj = to_center.dot(*ray.direction);
            if proj > 0.0 {
                let perp_dist = (to_center - *ray.direction * proj).length();
                let score = perp_dist / hit_radius;
                if perp_dist < hit_radius && score < best_score {
                    best_score = score;
                    best_target = Some(entity);
                }
            }
        }
    }

    if best_target.is_none() {
        let mut min_screen_dist = 220.0f32;
        for (entity, pos, _rad, _body, _mass) in targets_query.iter() {
            let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            if let Ok(screen_pos) = camera_comp.world_to_viewport(global_transform, center) {
                let dist = cursor_pos.distance(screen_pos);
                if dist < min_screen_dist {
                    min_screen_dist = dist;
                    best_target = Some(entity);
                }
            }
        }
    }

    if let Some(hit_entity) = best_target {
        player_state.selected_entity = Some(hit_entity);
        camera.target_entity = Some(hit_entity);
        if let Ok((_, _, radius, body, _)) = targets_query.get(hit_entity) {
            let visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
            camera.target_radius = config.calc_camera_framing_radius(visual_radius);
        }
    }
}

fn handle_camera_keyboard_flight(
    keyboard_input: &ButtonInput<KeyCode>,
    transform: &Transform,
    camera: &mut PanOrbitCamera,
    player_state: &mut PlayerInteractionState,
    targets_query: &Query<(Entity, &SimPosition, &Radius, &CelestialBody, &Mass)>,
    config: &SimulationConfig,
) {
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        if let Some(target) = player_state.selected_entity {
            camera.target_entity = Some(target);
            if let Ok((_, _, radius, body, _)) = targets_query.get(target) {
                let visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
                camera.target_radius = config.calc_camera_framing_radius(visual_radius);
            }
        } else {
            camera.target_focus = Vec3::ZERO;
            camera.target_entity = None;
        }
    }

    if keyboard_input.just_pressed(KeyCode::KeyR) || keyboard_input.just_pressed(KeyCode::Escape) {
        camera.target_focus = Vec3::ZERO;
        camera.target_entity = None;
        player_state.selected_entity = None;
    }

    let move_speed = (camera.target_radius * 0.025).clamp(0.1, 5.0);
    if keyboard_input.pressed(KeyCode::KeyW) {
        camera.target_focus += transform.rotation * -Vec3::Z * move_speed;
        camera.target_entity = None;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        camera.target_focus += transform.rotation * Vec3::Z * move_speed;
        camera.target_entity = None;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        camera.target_focus += transform.rotation * -Vec3::X * move_speed;
        camera.target_entity = None;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        camera.target_focus += transform.rotation * Vec3::X * move_speed;
        camera.target_entity = None;
    }
    if keyboard_input.pressed(KeyCode::KeyQ) {
        camera.target_focus.y -= move_speed;
        camera.target_entity = None;
    }
    if keyboard_input.pressed(KeyCode::KeyE) {
        camera.target_focus.y += move_speed;
        camera.target_entity = None;
    }
}

fn handle_camera_mouse_controls(
    mouse_buttons: &ButtonInput<MouseButton>,
    keyboard_input: &ButtonInput<KeyCode>,
    mouse_motion_events: &mut MessageReader<MouseMotion>,
    mouse_wheel_events: &mut MessageReader<MouseWheel>,
    camera: &mut PanOrbitCamera,
    transform: &Transform,
    cursor_over_ui: bool,
) {
    let mut delta_yaw = 0.0;
    let mut delta_pitch = 0.0;
    let mut delta_pan = Vec2::ZERO;

    if mouse_buttons.pressed(MouseButton::Right) && !keyboard_input.pressed(KeyCode::ShiftLeft) {
        for ev in mouse_motion_events.read() {
            delta_yaw -= ev.delta.x * camera.orbit_sensitivity;
            delta_pitch -= ev.delta.y * camera.orbit_sensitivity;
        }
    } else if mouse_buttons.pressed(MouseButton::Middle)
        || (mouse_buttons.pressed(MouseButton::Right) && keyboard_input.pressed(KeyCode::ShiftLeft))
    {
        for ev in mouse_motion_events.read() {
            delta_pan.x -= ev.delta.x * camera.pan_sensitivity * (camera.target_radius / 30.0);
            delta_pan.y += ev.delta.y * camera.pan_sensitivity * (camera.target_radius / 30.0);
        }
        camera.target_entity = None;
    } else {
        mouse_motion_events.clear();
    }

    let mut scroll = 0.0;
    for ev in mouse_wheel_events.read() {
        if cursor_over_ui {
            continue;
        }
        let delta = match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 24.0,
        };
        scroll += delta;
    }

    // Keyboard zoom shortcuts (+ / -)
    if keyboard_input.pressed(KeyCode::Equal) || keyboard_input.pressed(KeyCode::NumpadAdd) {
        scroll += 0.6;
    }
    if keyboard_input.pressed(KeyCode::Minus) || keyboard_input.pressed(KeyCode::NumpadSubtract) {
        scroll -= 0.6;
    }

    if scroll.abs() > 0.0 {
        let sensitivity = if keyboard_input.pressed(KeyCode::ShiftLeft)
            || keyboard_input.pressed(KeyCode::ShiftRight)
        {
            camera.zoom_sensitivity * 0.35 // Micro-zoom precision mode
        } else if keyboard_input.pressed(KeyCode::ControlLeft)
            || keyboard_input.pressed(KeyCode::ControlRight)
        {
            camera.zoom_sensitivity * 2.2 // Rapid macro-zoom mode
        } else {
            camera.zoom_sensitivity
        };

        let zoom_factor = (-scroll * sensitivity).exp();
        camera.target_radius =
            (camera.target_radius * zoom_factor).clamp(camera.min_radius, camera.max_radius);
    }

    camera.target_yaw += delta_yaw;
    camera.target_pitch = (camera.target_pitch + delta_pitch).clamp(-1.54, 1.54);

    if camera.target_entity.is_none() && delta_pan != Vec2::ZERO {
        let right = transform.rotation * Vec3::X;
        let up = transform.rotation * Vec3::Y;
        camera.target_focus += right * delta_pan.x + up * delta_pan.y;
    }
}

/// Handles user mouse, keyboard camera control, and 3D raycast click selection of celestial bodies.
pub fn update_pan_orbit_camera(
    windows: Query<&Window>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    config: Res<SimulationConfig>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mut player_state: ResMut<PlayerInteractionState>,
    targets_query: Query<(Entity, &SimPosition, &Radius, &CelestialBody, &Mass)>,
    mut camera_query: Query<
        (
            &Camera,
            &mut PanOrbitCamera,
            &mut Transform,
            &GlobalTransform,
        ),
        With<PanOrbitCamera>,
    >,
    ui_interaction_query: Query<&Interaction>,
) {
    let Ok((camera_comp, mut camera, mut transform, global_transform)) = camera_query.single_mut()
    else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    update_camera_min_zoom_bounds(&mut camera, &config, &targets_query);

    let cursor_over_ui = ui_interaction_query
        .iter()
        .any(|i| *i == Interaction::Pressed || *i == Interaction::Hovered);

    handle_camera_target_picking(
        window,
        camera_comp,
        global_transform,
        &targets_query,
        &mut player_state,
        &mut camera,
        &config,
        &ui_interaction_query,
        &mouse_buttons,
    );

    handle_camera_keyboard_flight(
        &keyboard_input,
        &transform,
        &mut camera,
        &mut player_state,
        &targets_query,
        &config,
    );

    handle_camera_mouse_controls(
        &mouse_buttons,
        &keyboard_input,
        &mut mouse_motion_events,
        &mut mouse_wheel_events,
        &mut camera,
        &transform,
        cursor_over_ui,
    );

    camera.yaw += (camera.target_yaw - camera.yaw) * 0.22;
    camera.pitch += (camera.target_pitch - camera.pitch) * 0.22;
    camera.radius = (camera.radius + (camera.target_radius - camera.radius) * 0.28)
        .clamp(camera.min_radius, camera.max_radius);

    if camera.target_entity.is_some() {
        camera.focus = camera.target_focus;
    } else {
        camera.focus = camera.focus.lerp(camera.target_focus, 0.14);
    }

    let rot =
        Quat::from_axis_angle(Vec3::Y, camera.yaw) * Quat::from_axis_angle(Vec3::X, -camera.pitch);
    let translation = camera.focus + rot * Vec3::new(0.0, 0.0, camera.radius);

    transform.translation = translation;
    transform.rotation = rot;
}
