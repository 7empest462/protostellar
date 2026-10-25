//! Simulation lifecycle, save/load, time warping, and diagnostic tool action handlers.

use bevy::prelude::*;

use super::super::telemetry_panel::handle_telemetry_action;
use super::super::types::*;
use super::time_and_panels::{
    handle_instrument_action, handle_panel_toggle_action, handle_time_action,
};
use crate::simulation::resources::*;

/// Handles quicksave and quickload serialization trigger buttons.
pub fn handle_save_load_action(
    action: &UiButtonAction,
    save_events: Option<&mut MessageWriter<crate::simulation::serialization::SaveSystemEvent>>,
    load_events: Option<&mut MessageWriter<crate::simulation::serialization::LoadSystemEvent>>,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::QuickSave => {
            if let Some(writer) = save_events {
                writer.write(crate::simulation::serialization::SaveSystemEvent {
                    filename: "saves/quicksave.json".to_string(),
                });
            }
            toast.message = "💾 Quick Saved solar system state [saves/quicksave.json]".to_string();
            toast.timer = 4.0;
            true
        }
        UiButtonAction::QuickLoad => {
            if let Some(writer) = load_events {
                writer.write(crate::simulation::serialization::LoadSystemEvent {
                    filename: "saves/quicksave.json".to_string(),
                });
            }
            toast.message =
                "📂 Quick Loading solar system state [saves/quicksave.json]...".to_string();
            toast.timer = 4.0;
            true
        }
        _ => false,
    }
}

/// Dispatches time controls, panel toggles, telemetry, trophies, tech tree, scrubber, and tools.
#[allow(
    clippy::too_many_arguments,
    reason = "UI button interaction system dispatches across active simulation controls and tools"
)]
pub fn dispatch_system_and_tool_actions(
    action: &UiButtonAction,
    save_events: Option<&mut MessageWriter<crate::simulation::serialization::SaveSystemEvent>>,
    load_events: Option<&mut MessageWriter<crate::simulation::serialization::LoadSystemEvent>>,
    time_warp: &mut TimeWarp,
    hud_visibility: &mut HudVisibilityState,
    telemetry_panel_state: &mut TelemetryPanelState,
    trophies_panel_state: &mut crate::game::ui::types::TrophiesPanelState,
    opt_tech_tree: Option<&mut crate::game::ui::types::TechTreePanelState>,
    opt_science: Option<&mut crate::simulation::economy::ScienceData>,
    opt_techs: Option<&mut crate::simulation::economy::UnlockedTechs>,
    telemetry_history: &mut crate::simulation::telemetry::SimulationTelemetryHistory,
    opt_scrubber: Option<&mut crate::simulation::geology::types::TimelineScrubber>,
    player_state: &mut PlayerInteractionState,
    opt_slingshot: Option<&mut SlingshotState>,
    opt_predictor: Option<&mut crate::simulation::predictor::TrajectoryPredictorState>,
    mut opt_hr_diagram: Option<&mut HrDiagramPanelState>,
    mut opt_lagrange: Option<&mut LagrangeOverlayConfig>,
    mut opt_hz: Option<&mut HabitableZoneConfig>,
    toast: &mut NotificationToast,
) -> bool {
    if handle_save_load_action(action, save_events, load_events, toast) {
        return true;
    }
    if handle_time_action(action, time_warp, toast) {
        return true;
    }
    if handle_panel_toggle_action(action, hud_visibility, toast) {
        return true;
    }
    if handle_telemetry_action(action, telemetry_panel_state, telemetry_history, toast) {
        return true;
    }
    if let Some(ref mut hr) = opt_hr_diagram {
        if super::super::hr_diagram_panel::handle_hr_diagram_action(action, hr, toast) {
            return true;
        }
    }
    if matches!(action, UiButtonAction::ToggleLagrangePoints) {
        if let Some(ref mut lagrange) = opt_lagrange {
            lagrange.show_lagrange_points = !lagrange.show_lagrange_points;
            toast.message = if lagrange.show_lagrange_points {
                "🪐 Lagrange Points (L1-L5) & Hill Sphere: ENABLED [Shift-L]".to_string()
            } else {
                "🪐 Lagrange Points: DISABLED [Shift-L]".to_string()
            };
            toast.timer = 3.0;
        }
        return true;
    }
    if matches!(action, UiButtonAction::ToggleHabitableZone) {
        if let Some(ref mut hz) = opt_hz {
            hz.is_enabled = !hz.is_enabled;
            toast.message = if hz.is_enabled {
                "🌱 Circumstellar Habitable Zone & Frost Line: ENABLED".to_string()
            } else {
                "🌱 Habitable Zone: DISABLED".to_string()
            };
            toast.timer = 3.0;
        }
        return true;
    }
    if matches!(action, UiButtonAction::ToggleTrophiesPanel) {
        trophies_panel_state.is_open = !trophies_panel_state.is_open;
        toast.message = if trophies_panel_state.is_open {
            "🏆 Trophies & Milestones Tracker Opened".to_string()
        } else {
            "🏆 Trophies Tracker Closed".to_string()
        };
        toast.timer = 2.5;
        return true;
    }
    if matches!(action, UiButtonAction::ToggleTechTreePanel) {
        if let Some(tech_tree_panel_state) = opt_tech_tree {
            tech_tree_panel_state.is_open = !tech_tree_panel_state.is_open;
            toast.message = if tech_tree_panel_state.is_open {
                "🔬 Science & Technology Tree Opened".to_string()
            } else {
                "🔬 Tech Tree Closed".to_string()
            };
            toast.timer = 2.5;
        }
        return true;
    }
    if let UiButtonAction::ResearchTech(tech_id) = action {
        if let (Some(science), Some(techs)) = (opt_science, opt_techs) {
            if techs.unlock(*tech_id, science) {
                let tech_name = crate::simulation::economy::ALL_TECH_NODES
                    .iter()
                    .find(|n| n.id == *tech_id)
                    .map_or("Technology", |n| n.name);
                toast.message = format!("✨ Researched: {tech_name}!");
                toast.timer = 4.0;
            } else {
                toast.message =
                    "🔒 Cannot research: Insufficient Science Data or missing prerequisites."
                        .to_string();
                toast.timer = 3.0;
            }
        }
        return true;
    }
    if super::super::epoch_scrubber::handle_epoch_scrubber_action(action, opt_scrubber, toast) {
        return true;
    }
    if handle_instrument_action(action, player_state, toast, opt_slingshot, opt_predictor) {
        return true;
    }
    false
}
