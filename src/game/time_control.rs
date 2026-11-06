//! Keyboard and UI controls for simulation time warp and stepping.

use bevy::prelude::*;

use crate::simulation::resources::*;

const SHIFT_SPEED_PRESETS: &[(KeyCode, KeyCode, f64, &str)] = &[
    (
        KeyCode::Digit1,
        KeyCode::Numpad1,
        TimeWarp::SPEED_1_MINUTE,
        "▶ Speed: 1s = 1 min (60x Real-time)",
    ),
    (
        KeyCode::Digit2,
        KeyCode::Numpad2,
        TimeWarp::SPEED_5_MINUTES,
        "▶ Speed: 1s = 5 min (300x Real-time)",
    ),
    (
        KeyCode::Digit3,
        KeyCode::Numpad3,
        TimeWarp::SPEED_6_HOURS,
        "▶ Speed: 1s = 6 hr (21,600x Real-time)",
    ),
    (
        KeyCode::Digit4,
        KeyCode::Numpad4,
        TimeWarp::SPEED_12_HOURS,
        "▶ Speed: 1s = 12 hr (43,200x Real-time)",
    ),
    (
        KeyCode::Digit5,
        KeyCode::Numpad5,
        TimeWarp::SPEED_24_HOURS,
        "▶ Speed: 1s = 24 hr (1 day/s)",
    ),
    (
        KeyCode::Digit6,
        KeyCode::Numpad6,
        TimeWarp::SPEED_72_HOURS,
        "▶ Speed: 1s = 72 hr (3 days/s)",
    ),
];

const DIRECT_SPEED_PRESETS: &[(KeyCode, KeyCode, f64, &str)] = &[
    (
        KeyCode::Digit1,
        KeyCode::Numpad1,
        TimeWarp::SPEED_REAL_TIME,
        "▶ Speed: Real-time (1s = 1.0s)",
    ),
    (
        KeyCode::Digit9,
        KeyCode::Numpad9,
        TimeWarp::SPEED_1_MINUTE,
        "▶ Speed: 1s = 1 min (60x Real-time)",
    ),
    (
        KeyCode::Digit0,
        KeyCode::Numpad0,
        TimeWarp::SPEED_5_MINUTES,
        "▶ Speed: 1s = 5 min (300x Real-time)",
    ),
    (
        KeyCode::Digit2,
        KeyCode::Numpad2,
        1.0,
        "▶ Speed: 1.0x (1s = 11.0 days)",
    ),
    (
        KeyCode::Digit3,
        KeyCode::Numpad3,
        10.0,
        "▶ Speed: 10x (1s = 3.6 months)",
    ),
    (
        KeyCode::Digit4,
        KeyCode::Numpad4,
        100.0,
        "⏩ Speed: 100x (1s = 3.0 yr)",
    ),
    (
        KeyCode::Digit5,
        KeyCode::Numpad5,
        1000.0,
        "⏩ Speed: 1,000x (1s = 30.0 yr)",
    ),
    (
        KeyCode::Digit6,
        KeyCode::Numpad6,
        10000.0,
        "⚡ Speed: 10,000x (1s = 300 yr)",
    ),
    (
        KeyCode::Digit7,
        KeyCode::Numpad7,
        100_000.0,
        "⚡ Speed: 100,000x (1s = 3.0k yr)",
    ),
    (
        KeyCode::Digit8,
        KeyCode::Numpad8,
        1_000_000.0,
        "🚀 Speed: 1,000,000x (1s = 30.0k yr)",
    ),
];

fn match_keyboard_speed_preset(keyboard: &ButtonInput<KeyCode>) -> Option<(f64, &'static str)> {
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let table = if shift {
        SHIFT_SPEED_PRESETS
    } else {
        DIRECT_SPEED_PRESETS
    };
    let mut matched = None;
    for &(digit, numpad, preset, msg) in table {
        if keyboard.just_pressed(digit) || keyboard.just_pressed(numpad) {
            matched = Some((preset, msg));
        }
    }
    matched
}

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

    if let Some((preset, msg)) = match_keyboard_speed_preset(&keyboard) {
        time_warp.set_preset(preset);
        toast_msg = Some(msg.to_string());
    }

    if let (Some(msg), Some(ref mut toast)) = (toast_msg, toast_res) {
        toast.message = msg;
        toast.timer = 3.0;
    }
}
