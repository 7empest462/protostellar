//! Keyboard and UI controls for simulation time warp and stepping.

use bevy::prelude::*;

use crate::simulation::resources::*;

/// Handles keyboard input for granular time acceleration and navigation.
pub fn handle_time_control_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut time_warp: ResMut<TimeWarp>,
    toast_res: Option<ResMut<crate::game::ui::types::NotificationToast>>,
) {
    let mut toast_msg = None;

    // Space: Toggle pause
    if keyboard.just_pressed(KeyCode::Space) {
        time_warp.is_paused = !time_warp.is_paused;
        toast_msg = Some(if time_warp.is_paused {
            "⏸ Simulation Paused".to_string()
        } else {
            format!(
                "▶ Simulation Resumed ({})",
                time_warp.human_readable_speed()
            )
        });
    }

    // Step once (when paused with Period / .)
    time_warp.step_once = keyboard.just_pressed(KeyCode::Period);

    // Continuous Granular Speed Controls ([ / ] or - / +)
    if keyboard.just_pressed(KeyCode::BracketRight) || keyboard.just_pressed(KeyCode::Equal) {
        time_warp.speed_up(1.5);
        toast_msg = Some(format!("⏩ Speed: {}", time_warp.human_readable_speed()));
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) || keyboard.just_pressed(KeyCode::Minus) {
        time_warp.slow_down(1.5);
        toast_msg = Some(format!("⏪ Speed: {}", time_warp.human_readable_speed()));
    }

    // Speed warp presets:
    // [1]: Real-time 1:1 (1s = 1s)
    // [2]: 1.0x (1s = 11.0 days)
    // [3]: 10.0x (1s = ~3.6 months)
    // [4]: 100.0x (1s = ~3.0 yr)
    // [5]: 1,000.0x (1s = ~30.0 yr)
    // [6]: 10,000.0x (1s = ~300.0 yr)
    // [7]: 100,000.0x (1s = ~3.0k yr)
    // [8]: 1,000,000.0x (1s = ~30.0k yr)
    if keyboard.just_pressed(KeyCode::Digit1) || keyboard.just_pressed(KeyCode::Numpad1) {
        time_warp.set_preset(TimeWarp::SPEED_REAL_TIME);
        toast_msg = Some("▶ Speed: Real-time (1s = 1.0s)".to_string());
    }
    if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::Numpad2) {
        time_warp.set_preset(1.0);
        toast_msg = Some("▶ Speed: 1.0x (1s = 11.0 days)".to_string());
    }
    if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::Numpad3) {
        time_warp.set_preset(10.0);
        toast_msg = Some("▶ Speed: 10x (1s = 3.6 months)".to_string());
    }
    if keyboard.just_pressed(KeyCode::Digit4) || keyboard.just_pressed(KeyCode::Numpad4) {
        time_warp.set_preset(100.0);
        toast_msg = Some("⏩ Speed: 100x (1s = 3.0 yr)".to_string());
    }
    if keyboard.just_pressed(KeyCode::Digit5) || keyboard.just_pressed(KeyCode::Numpad5) {
        time_warp.set_preset(1000.0);
        toast_msg = Some("⏩ Speed: 1,000x (1s = 30.0 yr)".to_string());
    }
    if keyboard.just_pressed(KeyCode::Digit6) || keyboard.just_pressed(KeyCode::Numpad6) {
        time_warp.set_preset(10000.0);
        toast_msg = Some("⚡ Speed: 10,000x (1s = 300 yr)".to_string());
    }
    if keyboard.just_pressed(KeyCode::Digit7) || keyboard.just_pressed(KeyCode::Numpad7) {
        time_warp.set_preset(100_000.0);
        toast_msg = Some("⚡ Speed: 100,000x (1s = 3.0k yr)".to_string());
    }
    if keyboard.just_pressed(KeyCode::Digit8) || keyboard.just_pressed(KeyCode::Numpad8) {
        time_warp.set_preset(1_000_000.0);
        toast_msg = Some("🚀 Speed: 1,000,000x (1s = 30.0k yr)".to_string());
    }

    if let (Some(msg), Some(ref mut toast)) = (toast_msg, toast_res) {
        toast.message = msg;
        toast.timer = 3.0;
    }
}
