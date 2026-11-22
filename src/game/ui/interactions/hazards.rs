//! Cosmic Hazards and Sensory Controls interaction handlers.

use bevy::prelude::*;

use crate::game::ui::hazards_panel::HazardsPanelState;
use crate::game::ui::types::{NotificationToast, SelectedWorldQuery, UiButtonAction};
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::events::{
    CosmicEventManager, CrisisCountermeasureEvent, CrisisCountermeasureKind, CrisisImpactor,
};
use crate::simulation::resources::PlayerInteractionState;

fn handle_focus_active_crisis(
    opt_hazard_mgr: Option<&CosmicEventManager>,
    impactors: &Query<(Entity, &CrisisImpactor)>,
    selected_query: &SelectedWorldQuery,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    toast: &mut NotificationToast,
) {
    let Some(ev) = opt_hazard_mgr.and_then(CosmicEventManager::primary_active_event) else {
        toast.message = "⌖ No active cosmic crisis to focus.".to_string();
        toast.timer = 2.5;
        return;
    };
    let threat_ent = ev
        .associated_entities
        .iter()
        .copied()
        .find(|e| impactors.get(*e).is_ok_and(|(_, imp)| !imp.deflected))
        .or_else(|| {
            ev.associated_entities
                .iter()
                .copied()
                .find(|e| selected_query.get(*e).is_ok())
        })
        .or(ev.target_entity);

    if let Some(ent) = threat_ent {
        player_state.selected_entity = Some(ent);
        for mut cam in camera_query.iter_mut() {
            cam.target_entity = Some(ent);
            cam.surface_horizon_mode = false;
            cam.target_radius = cam.target_radius.clamp(0.15, 2.5);
        }
        let label = selected_query
            .get(ent)
            .map_or(ev.event_type.title(), |(_, _, _, _, _, _, body, ..)| {
                body.name.as_str()
            });
        toast.message = format!("⌖ Camera locked onto crisis threat: {label}");
        toast.timer = 3.5;
    } else {
        toast.message = format!("⚠️ Crisis {} is system-wide.", ev.event_type.title());
        toast.timer = 3.0;
    }
}

/// Dispatches button actions for toggling hazards panel, triggering events, countermeasures, and tonemapping.
#[allow(
    clippy::too_many_arguments,
    reason = "Hazards dispatcher handles panel toggles, focusing active threats, and firing countermeasures"
)]
pub fn handle_hazard_action(
    action: &UiButtonAction,
    opt_hazards_panel: Option<&mut HazardsPanelState>,
    opt_hazard_mgr: Option<&mut CosmicEventManager>,
    opt_countermeasure_writer: Option<&mut MessageWriter<CrisisCountermeasureEvent>>,
    impactors: &Query<(Entity, &CrisisImpactor)>,
    selected_query: &SelectedWorldQuery,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
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
        UiButtonAction::FocusActiveCrisis => {
            handle_focus_active_crisis(
                opt_hazard_mgr.as_deref(),
                impactors,
                selected_query,
                player_state,
                camera_query,
                toast,
            );
            true
        }
        UiButtonAction::ExecuteCrisisCountermeasure => {
            if let (Some(mgr), Some(writer)) =
                (opt_hazard_mgr.as_deref(), opt_countermeasure_writer)
                && let Some(ev) = mgr.primary_active_event()
            {
                writer.write(CrisisCountermeasureEvent {
                    kind: CrisisCountermeasureKind::recommended_for(ev.event_type),
                    preferred_target: player_state.selected_entity.or(ev.target_entity),
                });
            }
            true
        }
        UiButtonAction::DeployPlanetaryDefense(kind) => {
            if let Some(writer) = opt_countermeasure_writer {
                let target = player_state.selected_entity.or_else(|| {
                    opt_hazard_mgr
                        .as_deref()
                        .and_then(CosmicEventManager::primary_active_event)
                        .and_then(|e| e.target_entity)
                });
                writer.write(CrisisCountermeasureEvent {
                    kind: *kind,
                    preferred_target: target,
                });
            }
            true
        }
        _ => false,
    }
}
