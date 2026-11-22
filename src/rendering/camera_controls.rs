//! Keyboard flight, mouse orbit/pan/zoom, and trackpad pinch input handlers for the orbital camera.

use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::math::DVec3;
use bevy::prelude::*;

use crate::rendering::camera::{CameraTargetsQuery, PanOrbitCamera};
use crate::simulation::resources::{PlayerInteractionState, SimulationConfig};

#[allow(clippy::too_many_arguments, reason = "Keyboard camera flight helper")]
pub(crate) fn handle_camera_keyboard_flight(
    keyboard_input: &ButtonInput<KeyCode>,
    transform: &Transform,
    camera: &mut PanOrbitCamera,
    player_state: &mut ResMut<PlayerInteractionState>,
    targets_query: &CameraTargetsQuery,
    config: &SimulationConfig,
    star_pos: Vec3,
    min_orbit_r: f32,
) {
    if camera.surface_horizon_mode {
        return;
    }

    if keyboard_input.just_pressed(KeyCode::KeyF) {
        if let Some(target) = player_state.selected_entity {
            if let Ok((_, pos, radius, body, mass, _opt_t, opt_comp, opt_diff)) =
                targets_query.get(target)
            {
                let target_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
                let frame_radius = if body.name.starts_with("🚀") || radius.0 <= 1e-11 {
                    let visual_radius =
                        config.calc_visual_radius_for_type(radius.0, body.body_type);
                    0.015f32.max(config.calc_camera_framing_radius(visual_radius))
                } else {
                    crate::rendering::bodies::transforms::compute_close_view_camera_framing_radius(
                        radius.0,
                        body.body_type,
                        mass.0,
                        opt_comp,
                        opt_diff,
                        player_state.show_atmospheres(),
                        (target_vec - star_pos).length(),
                        min_orbit_r,
                        config,
                    )
                };
                camera.initiate_target_transition_au(target, pos.0, frame_radius);
            } else {
                camera.target_entity = Some(target);
            }
        } else {
            camera.set_target_focus_au(DVec3::ZERO);
            camera.target_entity = None;
        }
    }

    let is_shift =
        keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight);
    if !is_shift && keyboard_input.just_pressed(KeyCode::KeyR) {
        camera.set_target_focus_au(DVec3::ZERO);
        camera.target_entity = None;
        player_state.selected_entity = None;
    }

    if !is_shift {
        let move_speed = (camera.target_radius * 0.025).clamp(1e-7, 5.0);
        if keyboard_input.pressed(KeyCode::KeyW) {
            let next =
                camera.target_focus_au + (transform.rotation * -Vec3::Z * move_speed).as_dvec3();
            camera.set_target_focus_au(next);
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyS) {
            let next =
                camera.target_focus_au + (transform.rotation * Vec3::Z * move_speed).as_dvec3();
            camera.set_target_focus_au(next);
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            let next =
                camera.target_focus_au + (transform.rotation * -Vec3::X * move_speed).as_dvec3();
            camera.set_target_focus_au(next);
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            let next =
                camera.target_focus_au + (transform.rotation * Vec3::X * move_speed).as_dvec3();
            camera.set_target_focus_au(next);
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyQ) {
            let next = camera.target_focus_au - DVec3::Y * f64::from(move_speed);
            camera.set_target_focus_au(next);
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyE) {
            let next = camera.target_focus_au + DVec3::Y * f64::from(move_speed);
            camera.set_target_focus_au(next);
            camera.target_entity = None;
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Mouse orbit, zoom, pinch, and pan controls"
)]
pub(crate) fn handle_camera_mouse_controls(
    mouse_buttons: &ButtonInput<MouseButton>,
    keyboard_input: &ButtonInput<KeyCode>,
    mouse_motion_events: &mut MessageReader<MouseMotion>,
    mouse_wheel_events: &mut MessageReader<MouseWheel>,
    pinch_events: Option<&Messages<PinchGesture>>,
    pinch_cursor: &mut bevy::ecs::message::MessageCursor<PinchGesture>,
    camera: &mut PanOrbitCamera,
    transform: &Transform,
    cursor_over_ui: bool,
    text_focused: bool,
    dt: f32,
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
            MouseScrollUnit::Pixel => ev.y / 14.0,
        };
        scroll += delta;
    }

    if let Some(pinches) = pinch_events {
        for ev in pinch_cursor.read(pinches) {
            if !cursor_over_ui {
                scroll += ev.0 * 16.0;
            }
        }
    }

    // Keyboard zoom shortcuts: Alt+= / Alt+-, PageUp / PageDown, or unmodified NumpadAdd / NumpadSubtract
    if !text_focused {
        let is_shift_key = keyboard_input.pressed(KeyCode::ShiftLeft)
            || keyboard_input.pressed(KeyCode::ShiftRight);
        let is_alt_key =
            keyboard_input.pressed(KeyCode::AltLeft) || keyboard_input.pressed(KeyCode::AltRight);
        let key_step = if dt >= 0.002 {
            (dt.clamp(0.002, 0.25) * 60.0) * 0.6
        } else {
            0.6
        };
        if (is_alt_key && keyboard_input.pressed(KeyCode::Equal))
            || keyboard_input.pressed(KeyCode::PageUp)
            || (!is_shift_key && keyboard_input.pressed(KeyCode::NumpadAdd))
        {
            scroll += key_step;
        }
        if (is_alt_key && keyboard_input.pressed(KeyCode::Minus))
            || keyboard_input.pressed(KeyCode::PageDown)
            || (!is_shift_key && keyboard_input.pressed(KeyCode::NumpadSubtract))
        {
            scroll -= key_step;
        }
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
        let next = camera.target_focus_au + (right * delta_pan.x + up * delta_pan.y).as_dvec3();
        camera.set_target_focus_au(next);
    }
}
