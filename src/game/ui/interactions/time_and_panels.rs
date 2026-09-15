//! Time flow controls, scientific overlays, and HUD panel collapsibility handlers.

use super::super::types::*;
use crate::simulation::resources::*;

pub fn handle_time_action(
    action: &UiButtonAction,
    time_warp: &mut TimeWarp,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::TimePause => {
            time_warp.is_paused = !time_warp.is_paused;
            toast.message = if time_warp.is_paused {
                "⏸ Simulation Paused".to_string()
            } else {
                format!(
                    "▶ Simulation Resumed ({})",
                    time_warp.human_readable_speed()
                )
            };
            toast.timer = 3.5;
            true
        }
        UiButtonAction::TimeSpeed1 => {
            time_warp.multiplier = 1.0;
            time_warp.is_paused = false;
            toast.message = "▶ Speed: 1.0x (Real-time flow)".to_string();
            toast.timer = 3.5;
            true
        }
        UiButtonAction::TimeSpeed100 => {
            time_warp.multiplier = 100.0;
            time_warp.is_paused = false;
            toast.message = "⏩ Speed: 100x Accelerated".to_string();
            toast.timer = 3.5;
            true
        }
        UiButtonAction::TimeSpeed10k => {
            time_warp.multiplier = 10_000.0;
            time_warp.is_paused = false;
            toast.message = "⚡ Speed: 10,000x (~10 kyr/sec)".to_string();
            toast.timer = 3.5;
            true
        }
        UiButtonAction::TimeSpeed1M => {
            time_warp.multiplier = 1_000_000.0;
            time_warp.is_paused = false;
            toast.message = "🚀 Speed: 1,000,000x (~1 Myr/sec)".to_string();
            toast.timer = 3.5;
            true
        }
        _ => false,
    }
}

pub fn handle_panel_toggle_action(
    action: &UiButtonAction,
    hud_visibility: &mut HudVisibilityState,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::ToggleFullScreenHud => {
            hud_visibility.is_full_screen_clean = !hud_visibility.is_full_screen_clean;
            toast.message = if hud_visibility.is_full_screen_clean {
                "⛶ Clean Fullscreen Mode (Press [F11] or click top-right badge to restore HUD)"
                    .to_string()
            } else {
                "👁️ Standard HUD View Restored".to_string()
            };
            toast.timer = 2.5;
            true
        }
        UiButtonAction::ToggleTopLeftPanel => {
            hud_visibility.top_left_minimized = !hud_visibility.top_left_minimized;
            true
        }
        UiButtonAction::ToggleTopRightPanel => {
            hud_visibility.top_right_minimized = !hud_visibility.top_right_minimized;
            true
        }
        UiButtonAction::ToggleInspectorPanel => {
            hud_visibility.inspector_minimized = !hud_visibility.inspector_minimized;
            true
        }
        UiButtonAction::ToggleScenariosPanel => {
            hud_visibility.scenarios_minimized = !hud_visibility.scenarios_minimized;
            true
        }
        _ => false,
    }
}

pub fn handle_instrument_action(
    action: &UiButtonAction,
    player_state: &mut PlayerInteractionState,
    toast: &mut NotificationToast,
    mut slingshot: Option<&mut SlingshotState>,
) -> bool {
    match action {
        UiButtonAction::ToggleOrbitMode => {
            player_state.orbit_mode = player_state.orbit_mode.cycle();
            // When toggling to cinematic/Hidden, also hide diagnostic overlays
            if player_state.orbit_mode == OrbitVisualizationMode::Off {
                player_state.overlay_mode = DiagnosticOverlayMode::Hidden;
            }

            toast.message = match player_state.orbit_mode {
                OrbitVisualizationMode::All => "궤 Orbit Visualization: All Worlds [Y]".to_string(),
                OrbitVisualizationMode::SelectedOnly => {
                    "궤 Orbit Visualization: Selected Target Only [Y]".to_string()
                }
                OrbitVisualizationMode::Off => {
                    "궤 Orbit Visualization: Hidden (Cinematic) [Y]".to_string()
                }
            };
            toast.timer = 2.5;
            true
        }
        UiButtonAction::CycleOverlayMode => {
            player_state.overlay_mode = player_state.overlay_mode.cycle();
            toast.message = format!(
                "📊 Diagnostic Overlay: {}",
                player_state.overlay_mode.display_name()
            );
            toast.timer = 3.5;
            true
        }
        UiButtonAction::ToggleTractor => {
            if player_state.active_tool == PlayerTool::GravitationalTractor {
                player_state.active_tool = PlayerTool::Inspect;
                player_state.tractor_position = None;
                player_state.tractor_mass = 0.0;
                toast.message = "🔬 Switched to Inspector Tool".to_string();
            } else {
                player_state.active_tool = PlayerTool::GravitationalTractor;
                player_state.tractor_position = Some(bevy::math::DVec3::new(10.0, 0.0, 10.0));
                player_state.tractor_mass = crate::utils::constants::EARTH_MASS_SOLAR * 5.0;
                toast.message =
                    "🧲 Gravitational Tractor Active [Drag to redirect bodies]".to_string();
            }
            toast.timer = 3.5;
            true
        }
        UiButtonAction::ToggleSlingshotMode => {
            if let Some(ref mut sl) = slingshot {
                sl.is_active = !sl.is_active;
                player_state.active_tool = if sl.is_active {
                    PlayerTool::Slingshot
                } else {
                    PlayerTool::Inspect
                };
                toast.message = if sl.is_active {
                    format!(
                        "🎯 Slingshot Launcher Active [{} {}] [Click & Drag to Aim]",
                        sl.archetype.icon(),
                        sl.archetype.display_name()
                    )
                } else {
                    "🎯 Slingshot Launcher Closed".to_string()
                };
                toast.timer = 3.5;
            }
            true
        }
        UiButtonAction::CycleSlingshotArchetype => {
            if let Some(ref mut sl) = slingshot {
                sl.archetype = sl.archetype.cycle();
                toast.message = format!(
                    "🎯 Slingshot Archetype: {} {}",
                    sl.archetype.icon(),
                    sl.archetype.display_name()
                );
                toast.timer = 2.5;
            }
            true
        }
        _ => false,
    }
}
