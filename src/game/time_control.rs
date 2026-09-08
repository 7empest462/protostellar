//! Keyboard and UI controls for simulation time warp and stepping.

use bevy::prelude::*;

use crate::simulation::resources::*;

/// Handles keyboard input for granular time acceleration and navigation.
pub fn handle_time_control_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut time_warp: ResMut<TimeWarp>,
) {
    // Space: Toggle pause
    if keyboard.just_pressed(KeyCode::Space) {
        time_warp.is_paused = !time_warp.is_paused;
    }

    // Step once (when paused with Period / .)
    time_warp.step_once = keyboard.just_pressed(KeyCode::Period);

    // Continuous Granular Speed Controls ([ / ] or - / +)
    if keyboard.just_pressed(KeyCode::BracketRight) || keyboard.just_pressed(KeyCode::Equal) {
        time_warp.speed_up(1.5);
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) || keyboard.just_pressed(KeyCode::Minus) {
        time_warp.slow_down(1.5);
    }

    // Speed warp presets (1 to 7 and Numpad 1 to 7)
    if keyboard.just_pressed(KeyCode::Digit1) || keyboard.just_pressed(KeyCode::Numpad1) {
        time_warp.set_preset(1.0);
    }
    if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::Numpad2) {
        time_warp.set_preset(10.0);
    }
    if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::Numpad3) {
        time_warp.set_preset(100.0);
    }
    if keyboard.just_pressed(KeyCode::Digit4) || keyboard.just_pressed(KeyCode::Numpad4) {
        time_warp.set_preset(1000.0);
    }
    if keyboard.just_pressed(KeyCode::Digit5) || keyboard.just_pressed(KeyCode::Numpad5) {
        time_warp.set_preset(10000.0);
    }
    if keyboard.just_pressed(KeyCode::Digit6) || keyboard.just_pressed(KeyCode::Numpad6) {
        time_warp.set_preset(100_000.0);
    }
    if keyboard.just_pressed(KeyCode::Digit7) || keyboard.just_pressed(KeyCode::Numpad7) {
        time_warp.set_preset(1_000_000.0);
    }
}
