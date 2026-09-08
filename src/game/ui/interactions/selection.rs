//! Celestial body selection and quick bar filter actions.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::EARTH_MASS_SOLAR;

use super::super::types::*;

fn focus_camera_on_target(
    cam: &mut PanOrbitCamera,
    pos: DVec3,
    radius: f64,
    body_type: BodyType,
    config: &SimulationConfig,
    entity: Entity,
) {
    cam.target_entity = Some(entity);
    let visual_r = config.calc_visual_radius_for_type(radius, body_type);
    cam.target_radius = config.calc_camera_framing_radius(visual_r);
    let target_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    cam.target_focus = target_vec;
    cam.focus = target_vec;
}

fn select_and_toast_body(
    entity: Entity,
    body_name: &str,
    body_type: BodyType,
    radius: f64,
    pos: DVec3,
    mass: f64,
    is_star: bool,
    dist: f64,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    config: &SimulationConfig,
    toast: &mut NotificationToast,
) {
    player_state.selected_entity = Some(entity);
    if let Ok(mut cam) = camera_query.single_mut() {
        focus_camera_on_target(&mut cam, pos, radius, body_type, config, entity);
    }

    let icon = if is_star {
        "☀️"
    } else if body_type == BodyType::GasGiant {
        "🪐"
    } else if body_type == BodyType::IceGiant {
        "❄️"
    } else if body_name.to_lowercase().contains("earth")
        || body_name.to_lowercase().contains("habitable")
        || body_name.to_lowercase().contains("1e")
        || body_name.to_lowercase().contains("1f")
        || body_name.to_lowercase().contains("1g")
    {
        "🌍"
    } else {
        "🪨"
    };

    let m_str = if mass >= 0.01 {
        format!("{mass:.2} M☉")
    } else {
        format!("{:.2} M⊕", mass / EARTH_MASS_SOLAR)
    };

    toast.message = if is_star {
        format!("{icon} Selected: {body_name} (Central Star) | Mass: {m_str}")
    } else {
        format!("{icon} Selected: {body_name} ({dist:.2} AU) | Mass: {m_str}")
    };
    toast.timer = 3.5;
}

fn cycle_target_body(
    selected_query: &SelectedWorldQuery,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    config: &SimulationConfig,
    toast: &mut NotificationToast,
) {
    let worlds = collect_sorted_system_worlds(
        selected_query
            .iter()
            .map(|(e, m, r, p, _v, _c, b, s, ..)| (e, b, p, m, r, s)),
    );

    if worlds.is_empty() {
        return;
    }

    let next_index = if let Some(current_ent) = player_state.selected_entity {
        worlds
            .iter()
            .position(|w| w.entity == current_ent)
            .map_or(0, |idx| (idx + 1) % worlds.len())
    } else {
        0
    };

    let Some(target) = worlds.get(next_index) else {
        return;
    };
    player_state.selected_entity = Some(target.entity);

    if let Ok(item) = selected_query.get(target.entity) {
        let pos = item.3 .0;
        if let Ok(mut cam) = camera_query.single_mut() {
            focus_camera_on_target(
                &mut cam,
                pos,
                target.radius_au,
                target.body_type,
                config,
                target.entity,
            );
        }
    }

    let icon = if target.is_central_star {
        "☀️"
    } else if target.body_type == BodyType::GasGiant {
        "🪐"
    } else if target.body_type == BodyType::IceGiant {
        "❄️"
    } else {
        "🪨"
    };

    let m_str = if target.mass_solar >= 0.01 {
        format!("{:.2} M☉", target.mass_solar)
    } else {
        format!("{:.2} M⊕", target.mass_solar / EARTH_MASS_SOLAR)
    };

    toast.message = format!(
        ">> TARGET: {} {} ({:.3} AU) | Mass: {}",
        icon, target.name, target.distance_au, m_str
    );
    toast.timer = 4.0;
}

pub fn handle_selection_action(
    action: &UiButtonAction,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    selected_query: &SelectedWorldQuery,
    config: &SimulationConfig,
    toast: &mut NotificationToast,
    quick_bar_state: &mut QuickBarState,
) -> bool {
    match action {
        UiButtonAction::SelectEntity(target_ent) => {
            let entity = *target_ent;
            if let Ok(item) = selected_query.get(entity) {
                let body_name = item.6.name.clone();
                let body_type = item.6.body_type;
                let radius = item.2 .0;
                let pos = item.3 .0;
                let mass = item.1 .0;
                let is_star = item.7.is_some() || body_type.is_star_or_remnant();
                let dist = if pos.is_finite() { pos.length() } else { 0.0 };

                select_and_toast_body(
                    entity,
                    &body_name,
                    body_type,
                    radius,
                    pos,
                    mass,
                    is_star,
                    dist,
                    player_state,
                    camera_query,
                    config,
                    toast,
                );
            }
            true
        }
        UiButtonAction::SelectStar => {
            let worlds = collect_sorted_system_worlds(
                selected_query
                    .iter()
                    .map(|(e, m, r, p, _v, _c, b, s, ..)| (e, b, p, m, r, s)),
            );
            if let Some(target) = worlds.first() {
                player_state.selected_entity = Some(target.entity);
                if let Ok(item) = selected_query.get(target.entity) {
                    let pos = item.3 .0;
                    if let Ok(mut cam) = camera_query.single_mut() {
                        focus_camera_on_target(
                            &mut cam,
                            pos,
                            target.radius_au,
                            target.body_type,
                            config,
                            target.entity,
                        );
                    }
                }
                toast.message = format!("☀️ Selected: {} (Central Star)", target.name);
                toast.timer = 3.5;
            }
            true
        }
        UiButtonAction::SelectMercury => {
            select_planetary_target(
                selected_query,
                player_state,
                camera_query,
                config,
                toast,
                "mercury",
                "ceres",
            );
            true
        }
        UiButtonAction::SelectEarth => {
            select_planetary_target(
                selected_query,
                player_state,
                camera_query,
                config,
                toast,
                "earth",
                "trappist-1e",
            );
            true
        }
        UiButtonAction::SelectJupiter => {
            select_planetary_target(
                selected_query,
                player_state,
                camera_query,
                config,
                toast,
                "jupiter",
                "gas",
            );
            true
        }
        UiButtonAction::SelectKuiper => {
            select_kuiper_target(selected_query, player_state, camera_query, config, toast);
            true
        }
        UiButtonAction::CycleTarget => {
            cycle_target_body(selected_query, player_state, camera_query, config, toast);
            true
        }
        UiButtonAction::ToggleMinimizeQuickBar => {
            quick_bar_state.is_minimized = !quick_bar_state.is_minimized;
            toast.message = if quick_bar_state.is_minimized {
                "🗕 Body Bar Minimized (Press [H] or click to Expand)".to_string()
            } else {
                "🗖 Body Bar Expanded".to_string()
            };
            toast.timer = 3.0;
            true
        }
        UiButtonAction::ToggleMinorBodies => {
            quick_bar_state.show_minor_bodies = !quick_bar_state.show_minor_bodies;
            toast.message = if quick_bar_state.show_minor_bodies {
                "🪐 Showing All Asteroids & Planetesimals".to_string()
            } else {
                "🪐 Showing Major Worlds Only".to_string()
            };
            toast.timer = 3.0;
            true
        }
        UiButtonAction::ToggleEmbryos => {
            quick_bar_state.show_embryos = !quick_bar_state.show_embryos;
            toast.message = if quick_bar_state.show_embryos {
                "🌱 Showing Protoplanetary Embryos".to_string()
            } else {
                "🌱 Hiding Protoplanetary Embryos".to_string()
            };
            toast.timer = 3.0;
            true
        }
        _ => false,
    }
}

fn select_planetary_target(
    selected_query: &SelectedWorldQuery,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    config: &SimulationConfig,
    toast: &mut NotificationToast,
    primary_name: &str,
    fallback_name: &str,
) {
    let mut sorted: Vec<_> = selected_query
        .iter()
        .filter(|item| item.7.is_none() && !item.6.body_type.is_star_or_remnant())
        .collect();
    sorted.sort_by(|a, b| {
        a.3 .0
            .length()
            .partial_cmp(&b.3 .0.length())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let target = sorted
        .iter()
        .find(|item| {
            let n = item.6.name.to_lowercase();
            n.contains(primary_name) || n.contains(fallback_name)
        })
        .or_else(|| sorted.first());

    if let Some(item) = target {
        let ent = item.0;
        let body_name = item.6.name.clone();
        let pos = item.3 .0;
        player_state.selected_entity = Some(ent);
        if let Ok(mut cam) = camera_query.single_mut() {
            focus_camera_on_target(&mut cam, pos, item.2 .0, item.6.body_type, config, ent);
        }
        toast.message = format!("🪐 Selected: {body_name}");
        toast.timer = 4.0;
    }
}

fn select_kuiper_target(
    selected_query: &SelectedWorldQuery,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    config: &SimulationConfig,
    toast: &mut NotificationToast,
) {
    let mut sorted: Vec<_> = selected_query
        .iter()
        .filter(|item| item.7.is_none() && !item.6.body_type.is_star_or_remnant())
        .collect();
    sorted.sort_by(|a, b| {
        b.3 .0
            .length()
            .partial_cmp(&a.3 .0.length())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let target = sorted
        .iter()
        .find(|item| {
            let n = item.6.name.to_lowercase();
            n.contains("kuiper") || n.contains("pluto")
        })
        .or_else(|| sorted.first());

    if let Some(item) = target {
        let ent = item.0;
        let body_name = item.6.name.clone();
        let pos = item.3 .0;
        player_state.selected_entity = Some(ent);
        if let Ok(mut cam) = camera_query.single_mut() {
            focus_camera_on_target(&mut cam, pos, item.2 .0, item.6.body_type, config, ent);
        }
        toast.message = format!("🧊 Selected: {body_name}");
        toast.timer = 4.0;
    }
}
