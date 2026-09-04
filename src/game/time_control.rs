//! Keyboard and UI controls for simulation time warp and stepping.

use bevy::prelude::*;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Handles keyboard input for granular time acceleration and navigation.
pub fn handle_time_control_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut time_warp: ResMut<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut camera_query: Query<&mut PanOrbitCamera>,
    massive_bodies_query: Query<(Entity, &CelestialBody)>,
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

    // Speed warp presets (1 to 7)
    if keyboard.just_pressed(KeyCode::Digit1) {
        time_warp.set_preset(1.0);
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        time_warp.set_preset(10.0);
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        time_warp.set_preset(100.0);
    }
    if keyboard.just_pressed(KeyCode::Digit4) {
        time_warp.set_preset(1000.0);
    }
    if keyboard.just_pressed(KeyCode::Digit5) {
        time_warp.set_preset(10000.0);
    }
    if keyboard.just_pressed(KeyCode::Digit6) {
        time_warp.set_preset(100000.0);
    }
    if keyboard.just_pressed(KeyCode::Digit7) {
        time_warp.set_preset(1000000.0);
    }

    // Tab: Cycle through massive bodies / planets
    if keyboard.just_pressed(KeyCode::Tab) {
        let massive_entities: Vec<Entity> = massive_bodies_query
            .iter()
            .filter(|(_, b)| {
                matches!(
                    b.body_type,
                    BodyType::Protostar
                        | BodyType::MainSequenceStar
                        | BodyType::TerrestrialPlanet
                        | BodyType::SuperEarth
                        | BodyType::GasGiant
                        | BodyType::IceGiant
                        | BodyType::Protoplanet
                )
            })
            .map(|(e, _)| e)
            .collect();

        if !massive_entities.is_empty() {
            let current_idx = player_state
                .selected_entity
                .and_then(|curr| massive_entities.iter().position(|&e| e == curr))
                .unwrap_or(0);

            let next_idx = (current_idx + 1) % massive_entities.len();
            let next_entity = massive_entities[next_idx];

            player_state.selected_entity = Some(next_entity);
            if let Ok(mut cam) = camera_query.single_mut() {
                cam.target_entity = Some(next_entity);
            }
        }
    }
}
