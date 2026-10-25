//! Target world selection and Tab cycling.

use bevy::prelude::*;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::celestial::BodyType;
use crate::simulation::resources::{PlayerInteractionState, SimulationConfig};
use crate::utils::constants::EARTH_MASS_SOLAR;

use super::body_tools::SelectedBodyQuery;

#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
pub fn handle_tab_selection(
    keyboard: &ButtonInput<KeyCode>,
    config: &SimulationConfig,
    player_state: &mut PlayerInteractionState,
    toast: &mut crate::game::ui::NotificationToast,
    camera_query: &mut Query<(&Transform, &mut PanOrbitCamera)>,
    selected_query: &SelectedBodyQuery,
) {
    if !keyboard.just_pressed(KeyCode::Tab) {
        return;
    }
    let worlds = crate::game::ui::collect_sorted_system_worlds(
        selected_query
            .iter()
            .map(|item| (item.0, item.7, item.4, item.1, item.2, item.12)),
        false,
    );
    if worlds.is_empty() {
        return;
    }
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let len = worlds.len();
    let next_idx = if let Some(curr) = player_state.selected_entity {
        if let Some(curr_idx) = worlds.iter().position(|w| w.entity == curr) {
            if shift {
                (curr_idx + len - 1) % len
            } else {
                (curr_idx + 1) % len
            }
        } else {
            0
        }
    } else if shift {
        len - 1
    } else {
        0
    };

    let Some(target) = worlds.get(next_idx) else {
        return;
    };
    player_state.selected_entity = Some(target.entity);
    if let Ok((_, mut cam)) = camera_query.single_mut() {
        cam.surface_horizon_mode = false;
        cam.target_entity = Some(target.entity);
        let visual_r = config.calc_visual_radius_for_type(target.radius_au, target.body_type);
        let frame_r = if target.body_type == BodyType::BlackHole && target.mass_solar > 1_000_000.0
        {
            100_000.0 // Supermassive Black Holes (Galactic Centers) should zoom out to see the galaxy
        } else if target.name.starts_with("🚀") || target.radius_au <= 1e-6 {
            0.015f32.max(config.calc_camera_framing_radius(visual_r))
        } else {
            config.calc_camera_framing_radius(visual_r)
        };
        cam.target_radius = frame_r;
    }

    let m_str = if target.mass_solar >= 0.01 {
        format!("{:.2} M☉", target.mass_solar)
    } else {
        format!("{:.2} M⊕", target.mass_solar / EARTH_MASS_SOLAR)
    };
    let icon = if target.is_central_star || target.body_type.is_star_or_remnant() {
        "☀️"
    } else if target.name.to_lowercase().contains("earth")
        || target.name.to_lowercase().contains("habitable")
        || target.name.to_lowercase().contains("1e")
        || target.name.to_lowercase().contains("1f")
        || target.name.to_lowercase().contains("1g")
    {
        "🌍"
    } else if target.body_type == BodyType::GasGiant
        || target.name.to_lowercase().contains("jupiter")
        || target.name.to_lowercase().contains("saturn")
    {
        "🪐"
    } else if target.name.to_lowercase().contains("rogue")
        || target.name.to_lowercase().contains("nemesis")
    {
        "☄️"
    } else {
        "🪨"
    };
    toast.message = format!(
        ">> TARGET: {} {} ({:.3} AU) | Mass: {}",
        icon, target.name, target.distance_au, m_str
    );
    toast.timer = 4.0;
}
