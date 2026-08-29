//! 3D Orbital Camera with logarithmic zoom, focus-lock, and smooth interpolation.

use bevy::input::mouse::{MouseMotion, MouseWheel};
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
            radius: 40.0, // 40 AU cinematic framing of protoplanetary disk
            target_radius: 40.0,
            yaw: 0.785, // 45 degrees
            target_yaw: 0.785,
            pitch: 0.62, // ~36 degrees inclination
            target_pitch: 0.62,
            target_entity: None,
            min_radius: 0.05,
            max_radius: 450.0,
            orbit_sensitivity: 0.005,
            zoom_sensitivity: 0.15,
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

    // Single camera: 3D scene + UI overlay (canonical Bevy pattern)
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(translation).looking_at(pan_orbit.focus, Vec3::Y),
        pan_orbit,
        IsDefaultUiCamera,
    ));
    info!("✅ setup_camera: Spawned Camera3d with IsDefaultUiCamera");
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
) {
    let Ok((camera_comp, mut camera, mut transform, global_transform)) = camera_query.single_mut()
    else {
        return;
    };

    let Ok(window) = windows.single() else {
        return;
    };

    // Update dynamic minimum zoom radius when locked on a body
    if let Some(target_ent) = camera.target_entity {
        if let Ok((_, pos, _, body, mass)) = targets_query.get(target_ent) {
            let target_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            camera.target_focus = target_vec;

            // Stop right outside the planet's visual surface
            let visual_radius = SimulationConfig::calc_render_radius(mass.0, body.body_type) * config.body_render_scale;
            camera.min_radius = (visual_radius * 1.5).max(0.012);
        } else {
            camera.target_entity = None;
            camera.min_radius = 0.05;
        }
    } else {
        camera.min_radius = 0.05;
    }

    // 1. Pixel-Accurate Screen-Space & 3D Ray Selection of Celestial Bodies (Star & Planets)
    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some(cursor_pos) = window.cursor_position() {
            let mut best_target: Option<Entity> = None;
            let mut best_score = f32::MAX;

            // Try 3D viewport raycast first
            if let Ok(ray) = camera_comp.viewport_to_world(global_transform, cursor_pos) {
                for (entity, pos, _rad, body, _mass) in targets_query.iter() {
                    let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
                    let hit_radius = if matches!(
                        body.body_type,
                        BodyType::Protostar | BodyType::MainSequenceStar
                    ) {
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

            // Fallback to screen space projection distance with generous radius
            if best_target.is_none() {
                let mut min_screen_dist = 220.0f32;
                for (entity, pos, _rad, _body, _mass) in targets_query.iter() {
                    let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
                    if let Ok(screen_pos) = camera_comp.world_to_viewport(global_transform, center)
                    {
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
            }
        }
    }

    let mut delta_yaw = 0.0;
    let mut delta_pitch = 0.0;
    let mut delta_pan = Vec2::ZERO;

    // Check for focus reset key
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        if let Some(target) = player_state.selected_entity {
            camera.target_entity = Some(target);
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

    // Tab Key: Cycle selection through all orbiting celestial bodies
    if keyboard_input.just_pressed(KeyCode::Tab) {
        let mut targets_with_mass: Vec<(Entity, f64)> = targets_query
            .iter()
            .map(|(e, _, _, _, mass)| (e, mass.0))
            .collect();
            
        // Sort descending by mass to always cycle from Star -> Biggest Planet -> Smallest Moon
        targets_with_mass.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        let targets: Vec<Entity> = targets_with_mass.into_iter().map(|(e, _)| e).collect();
        
        if !targets.is_empty() {
            let next_target = if let Some(cur) = player_state.selected_entity {
                if let Some(idx) = targets.iter().position(|&e| e == cur) {
                    targets[(idx + 1) % targets.len()]
                } else {
                    targets[0]
                }
            } else {
                targets[0]
            };
            player_state.selected_entity = Some(next_target);
            camera.target_entity = Some(next_target);
        }
    }

    // Free-fly keyboard navigation (WASD + QE)
    let move_speed = (camera.target_radius * 0.025).clamp(0.1, 5.0);
    if keyboard_input.pressed(KeyCode::KeyW) {
        let fwd = transform.rotation * -Vec3::Z;
        camera.target_focus += fwd * move_speed;
        camera.target_entity = None;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        let fwd = transform.rotation * Vec3::Z;
        camera.target_focus += fwd * move_speed;
        camera.target_entity = None;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        let right = transform.rotation * -Vec3::X;
        camera.target_focus += right * move_speed;
        camera.target_entity = None;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        let right = transform.rotation * Vec3::X;
        camera.target_focus += right * move_speed;
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

    // Mouse Controls: Orbit / Pan
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

    // Scroll zoom input (Logarithmic scale)
    let mut scroll = 0.0;
    for ev in mouse_wheel_events.read() {
        scroll += ev.y;
    }

    if scroll.abs() > 0.0 {
        // Use exponential decay to prevent overshooting from fast trackpad scrolls
        let zoom_factor = (-scroll * camera.zoom_sensitivity).exp();
        camera.target_radius =
            (camera.target_radius * zoom_factor).clamp(camera.min_radius, camera.max_radius);
    }

    // Apply rotation to target
    camera.target_yaw += delta_yaw;
    camera.target_pitch = (camera.target_pitch + delta_pitch).clamp(-1.54, 1.54);

    // Free panning
    if camera.target_entity.is_none() && delta_pan != Vec2::ZERO {
        let right = transform.rotation * Vec3::X;
        let up = transform.rotation * Vec3::Y;
        camera.target_focus += right * delta_pan.x + up * delta_pan.y;
    }

    // Smooth cinematic exponential damping interpolation
    camera.yaw += (camera.target_yaw - camera.yaw) * 0.22;
    camera.pitch += (camera.target_pitch - camera.pitch) * 0.22;
    camera.radius = (camera.radius + (camera.target_radius - camera.radius) * 0.18).clamp(camera.min_radius, camera.max_radius);

    // When focus-locked on a moving celestial body, lock immediately without lag so it stays dead center
    if camera.target_entity.is_some() {
        camera.focus = camera.target_focus;
    } else {
        camera.focus = camera.focus.lerp(camera.target_focus, 0.14);
    }

    // Recompute camera transform
    let rot =
        Quat::from_axis_angle(Vec3::Y, camera.yaw) * Quat::from_axis_angle(Vec3::X, -camera.pitch);
    let translation = camera.focus + rot * Vec3::new(0.0, 0.0, camera.radius);

    transform.translation = translation;
    transform.look_at(camera.focus, Vec3::Y);
}
