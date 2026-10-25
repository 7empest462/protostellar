//! Cosmic Hazards and Sensory Controls interaction handlers.

use bevy::prelude::*;

use crate::game::ui::hazards_panel::HazardsPanelState;
use crate::game::ui::types::{NotificationToast, UiButtonAction};
use crate::simulation::events::CosmicEventManager;

/// Dispatches button actions for toggling hazards panel, triggering events, and cycling tonemapping.
pub fn handle_hazard_action(
    action: &UiButtonAction,
    opt_hazards_panel: Option<&mut HazardsPanelState>,
    opt_hazard_mgr: Option<&mut CosmicEventManager>,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::ToggleHazardsPanel => {
            if let Some(panel) = opt_hazards_panel {
                panel.is_open = !panel.is_open;
                toast.message = if panel.is_open {
                    "⚡ Cosmic Hazards & Sensory Lab Opened [Shift-H]".to_string()
                } else {
                    "⚡ Hazards Panel Closed".to_string()
                };
                toast.timer = 2.5;
            }
            true
        }
        UiButtonAction::TriggerCosmicHazard(hazard_type) => {
            if let Some(mgr) = opt_hazard_mgr {
                mgr.trigger_hazard(*hazard_type, 1.0);
                if let Some(panel) = opt_hazards_panel {
                    panel.is_open = false;
                }
            }
            true
        }
        UiButtonAction::ToggleAutoHazards => {
            if let Some(mgr) = opt_hazard_mgr {
                mgr.auto_hazards_enabled = !mgr.auto_hazards_enabled;
                toast.message = if mgr.auto_hazards_enabled {
                    "🎲 Background Cosmic Hazards: ACTIVE".to_string()
                } else {
                    "🛑 Background Cosmic Hazards: DISABLED".to_string()
                };
                toast.timer = 3.0;
            }
            true
        }
        UiButtonAction::CycleTonemappingMode => {
            if let Some(mgr) = opt_hazard_mgr {
                mgr.tonemapping_mode = mgr.tonemapping_mode.cycle();
                toast.message = format!(
                    "🎨 Post-Processing Tonemap: {}",
                    mgr.tonemapping_mode.label()
                );
                toast.timer = 3.0;
            }
            true
        }
        _ => false,
    }
}
