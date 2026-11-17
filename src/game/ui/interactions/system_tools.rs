//! Simulation lifecycle, save/load, time warping, and diagnostic tool action handlers.

use bevy::prelude::*;

use super::super::telemetry_panel::handle_telemetry_action;
use super::super::types::*;
use super::time_and_panels::{
    handle_instrument_action, handle_panel_toggle_action, handle_time_action,
};
use crate::game::ui::saves_panel::SavesPanelState;
use crate::game::ui::widgets::SaveSlotUiState;
use crate::simulation::resources::*;
use crate::simulation::scenarios::ActiveScenarioState;
use crate::simulation::serialization::{
    CopySystemSeedClipboardEvent, LoadSystemEvent, PasteSystemSeedClipboardEvent, SaveSystemEvent,
    delete_save_slot_file, resolve_save_path_for_read, resolve_save_path_for_write,
    sanitize_save_slot_name, suggest_next_slot_name,
};

fn set_toast(toast: &mut NotificationToast, msg: String, duration: f32) {
    toast.message = msg;
    toast.timer = duration;
}

fn mark_saves_dirty(saves_panel: Option<&mut SavesPanelState>) {
    if let Some(saves) = saves_panel {
        saves.dirty = true;
        saves.pending_refresh_ticks = 2;
        saves.pending_delete_idx = None;
    }
}

fn handle_slot_panel_controls(
    action: &UiButtonAction,
    save_events: Option<&mut MessageWriter<SaveSystemEvent>>,
    slot_state: Option<&mut SaveSlotUiState>,
    mut saves_panel: Option<&mut SavesPanelState>,
    active_preset: Option<crate::simulation::scenarios::ScenarioPreset>,
    toast: &mut NotificationToast,
) -> Option<bool> {
    match action {
        UiButtonAction::ToggleSavesPanel => {
            if let Some(ref mut saves) = saves_panel {
                saves.is_open = !saves.is_open;
                if saves.is_open {
                    saves.refresh_entries(active_preset);
                    set_toast(
                        toast,
                        "🗂️ Multi-Save & Scenario Slot Manager Opened".to_string(),
                        2.5,
                    );
                } else {
                    saves.pending_delete_idx = None;
                    set_toast(toast, "🗂️ Save Slot Manager Closed".to_string(), 2.5);
                }
            }
            Some(true)
        }
        UiButtonAction::RefreshSaveSlots => {
            if let Some(ref mut saves) = saves_panel {
                saves.refresh_entries(active_preset);
                let count = saves.entries.len();
                set_toast(toast, format!("↻ Found {count} saved system(s)"), 2.5);
            }
            Some(true)
        }
        UiButtonAction::ToggleSavesScenarioFilter => {
            if let Some(ref mut saves) = saves_panel {
                saves.filter_active_scenario = !saves.filter_active_scenario;
                saves.refresh_entries(active_preset);
                let msg = if saves.filter_active_scenario {
                    format!(
                        "🔍 Filtering saves to [{}]",
                        active_preset.map_or("Custom", |p| p.short_label())
                    )
                } else {
                    "🔍 Showing saves across all scenarios".to_string()
                };
                set_toast(toast, msg, 2.5);
            }
            Some(true)
        }
        UiButtonAction::SaveNewScenarioSlot => {
            let next_name = suggest_next_slot_name(active_preset);
            let filename = resolve_save_path_for_write(&next_name);
            if let Some(slot) = slot_state {
                slot.slot_name.clone_from(&next_name);
            }
            if let Some(writer) = save_events {
                writer.write(SaveSystemEvent {
                    filename: filename.clone(),
                });
            }
            mark_saves_dirty(saves_panel);
            set_toast(
                toast,
                format!("✨ Created scenario save [{next_name}] ({filename})"),
                4.0,
            );
            Some(true)
        }
        _ => None,
    }
}

fn handle_delete_save_slot_action(
    idx: usize,
    saves: &mut SavesPanelState,
    active_preset: Option<crate::simulation::scenarios::ScenarioPreset>,
    toast: &mut NotificationToast,
) {
    let Some(entry) = saves.entries.get(idx).cloned() else {
        saves.pending_delete_idx = None;
        return;
    };
    if saves.pending_delete_idx == Some(idx) {
        match delete_save_slot_file(&entry.file_path) {
            Ok(removed) => {
                saves.refresh_entries(active_preset);
                set_toast(
                    toast,
                    format!("🗑 Deleted save slot [{}] ({removed})", entry.slot_name),
                    3.5,
                );
            }
            Err(err) => {
                saves.pending_delete_idx = None;
                set_toast(toast, format!("⚠️ {err}"), 3.5);
            }
        }
    } else {
        saves.pending_delete_idx = Some(idx);
        set_toast(
            toast,
            format!("⚠️ Click Delete again to remove [{}]", entry.slot_name),
            3.5,
        );
    }
}

fn handle_slot_entry_action(
    action: &UiButtonAction,
    save_events: Option<&mut MessageWriter<SaveSystemEvent>>,
    load_events: Option<&mut MessageWriter<LoadSystemEvent>>,
    slot_state: Option<&mut SaveSlotUiState>,
    mut saves_panel: Option<&mut SavesPanelState>,
    active_preset: Option<crate::simulation::scenarios::ScenarioPreset>,
    toast: &mut NotificationToast,
) -> Option<bool> {
    let idx = match action {
        UiButtonAction::LoadSaveSlot(idx)
        | UiButtonAction::OverwriteSaveSlot(idx)
        | UiButtonAction::SelectSaveSlot(idx)
        | UiButtonAction::DeleteSaveSlot(idx) => *idx,
        _ => return None,
    };
    if matches!(action, UiButtonAction::DeleteSaveSlot(_)) {
        if let Some(saves) = saves_panel.as_deref_mut() {
            handle_delete_save_slot_action(idx, saves, active_preset, toast);
        }
        return Some(true);
    }
    let Some(entry) = saves_panel
        .as_deref()
        .and_then(|s| s.entries.get(idx))
        .cloned()
    else {
        return Some(true);
    };
    if let Some(slot) = slot_state {
        slot.slot_name.clone_from(&entry.slot_name);
    }
    match action {
        UiButtonAction::LoadSaveSlot(_) => {
            if let Some(writer) = load_events {
                writer.write(LoadSystemEvent {
                    filename: entry.file_path,
                });
            }
            if let Some(saves) = saves_panel {
                saves.is_open = false;
                saves.pending_delete_idx = None;
            }
            set_toast(
                toast,
                format!("📂 Loading saved slot [{}]...", entry.slot_name),
                4.0,
            );
        }
        UiButtonAction::OverwriteSaveSlot(_) => {
            if let Some(writer) = save_events {
                writer.write(SaveSystemEvent {
                    filename: entry.file_path,
                });
            }
            mark_saves_dirty(saves_panel);
            set_toast(
                toast,
                format!("💾 Overwrote save slot [{}]", entry.slot_name),
                4.0,
            );
        }
        UiButtonAction::SelectSaveSlot(_) => {
            if let Some(saves) = saves_panel {
                saves.pending_delete_idx = None;
            }
            set_toast(
                toast,
                format!("✎ Active slot set to [{}]", entry.slot_name),
                2.5,
            );
        }
        _ => {}
    }
    Some(true)
}

fn dispatch_slot_browser_action(
    action: &UiButtonAction,
    mut save_events: Option<&mut MessageWriter<SaveSystemEvent>>,
    load_events: Option<&mut MessageWriter<LoadSystemEvent>>,
    mut slot_state: Option<&mut SaveSlotUiState>,
    mut saves_panel: Option<&mut SavesPanelState>,
    scenario_state: Option<&ActiveScenarioState>,
    toast: &mut NotificationToast,
) -> Option<bool> {
    let active_preset = scenario_state.map(|s| s.current_preset);
    if let Some(handled) = handle_slot_panel_controls(
        action,
        save_events.as_deref_mut(),
        slot_state.as_deref_mut(),
        saves_panel.as_deref_mut(),
        active_preset,
        toast,
    ) {
        return Some(handled);
    }
    handle_slot_entry_action(
        action,
        save_events,
        load_events,
        slot_state,
        saves_panel,
        active_preset,
        toast,
    )
}

/// Handles quicksave, named slots, multi-save manager actions, and clipboard seed serialization.
#[allow(
    clippy::too_many_arguments,
    reason = "Dispatches save/load/slot/seed serialization actions"
)]
pub fn handle_save_load_action(
    action: &UiButtonAction,
    save_events: Option<&mut MessageWriter<SaveSystemEvent>>,
    load_events: Option<&mut MessageWriter<LoadSystemEvent>>,
    copy_seed_events: Option<&mut MessageWriter<CopySystemSeedClipboardEvent>>,
    paste_seed_events: Option<&mut MessageWriter<PasteSystemSeedClipboardEvent>>,
    mut slot_state: Option<&mut SaveSlotUiState>,
    mut saves_panel: Option<&mut SavesPanelState>,
    scenario_state: Option<&ActiveScenarioState>,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::QuickSave => {
            if let Some(writer) = save_events {
                writer.write(SaveSystemEvent {
                    filename: "saves/quicksave.json".to_string(),
                });
            }
            if let Some(ref mut saves) = saves_panel {
                saves.dirty = true;
                saves.pending_refresh_ticks = 2;
            }
            toast.message = "💾 Quick Saved solar system state [saves/quicksave.json]".to_string();
            toast.timer = 4.0;
            true
        }
        UiButtonAction::QuickLoad => {
            if let Some(writer) = load_events {
                writer.write(LoadSystemEvent {
                    filename: "saves/quicksave.json".to_string(),
                });
            }
            toast.message =
                "📂 Quick Loading solar system state [saves/quicksave.json]...".to_string();
            toast.timer = 4.0;
            true
        }
        UiButtonAction::SaveNamedSlot => {
            let raw = slot_state
                .as_deref()
                .map_or("quicksave", |s| s.slot_name.as_str());
            let sanitized = sanitize_save_slot_name(raw);
            let filename = resolve_save_path_for_write(raw);
            if let Some(ref mut slot) = slot_state {
                slot.slot_name = sanitized;
            }
            if let Some(writer) = save_events {
                writer.write(SaveSystemEvent {
                    filename: filename.clone(),
                });
            }
            if let Some(ref mut saves) = saves_panel {
                saves.dirty = true;
                saves.pending_refresh_ticks = 2;
            }
            toast.message = format!("💾 Saved to slot [{filename}]");
            toast.timer = 4.0;
            true
        }
        UiButtonAction::LoadNamedSlot => {
            let raw = slot_state
                .as_deref()
                .map_or("quicksave", |s| s.slot_name.as_str());
            let sanitized = sanitize_save_slot_name(raw);
            let filename = resolve_save_path_for_read(raw);
            if let Some(ref mut slot) = slot_state {
                slot.slot_name = sanitized;
            }
            if let Some(writer) = load_events {
                writer.write(LoadSystemEvent {
                    filename: filename.clone(),
                });
            }
            toast.message = format!("📂 Loading from slot [{filename}]...");
            toast.timer = 4.0;
            true
        }
        UiButtonAction::CopySystemSeedClipboard => {
            if let Some(writer) = copy_seed_events {
                writer.write(CopySystemSeedClipboardEvent);
            }
            true
        }
        UiButtonAction::PasteSystemSeedClipboard => {
            if let Some(writer) = paste_seed_events {
                writer.write(PasteSystemSeedClipboardEvent);
            }
            true
        }
        _ => dispatch_slot_browser_action(
            action,
            save_events,
            load_events,
            slot_state,
            saves_panel,
            scenario_state,
            toast,
        )
        .unwrap_or(false),
    }
}

/// Dispatches time controls, panel toggles, telemetry, trophies, tech tree, scrubber, and tools.
#[allow(
    clippy::too_many_arguments,
    reason = "UI button interaction system dispatches across active simulation controls and tools"
)]
pub fn dispatch_system_and_tool_actions(
    action: &UiButtonAction,
    save_events: Option<&mut MessageWriter<SaveSystemEvent>>,
    load_events: Option<&mut MessageWriter<LoadSystemEvent>>,
    copy_seed_events: Option<&mut MessageWriter<CopySystemSeedClipboardEvent>>,
    paste_seed_events: Option<&mut MessageWriter<PasteSystemSeedClipboardEvent>>,
    slot_state: Option<&mut SaveSlotUiState>,
    saves_panel: Option<&mut SavesPanelState>,
    scenario_state: Option<&ActiveScenarioState>,
    quick_bar_state: &mut QuickBarState,
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
    opt_audio: Option<&mut crate::game::audio::CosmicAudioState>,
    toast: &mut NotificationToast,
) -> bool {
    if handle_save_load_action(
        action,
        save_events,
        load_events,
        copy_seed_events,
        paste_seed_events,
        slot_state,
        saves_panel,
        scenario_state,
        toast,
    ) {
        return true;
    }
    if matches!(action, UiButtonAction::ClearQuickBarFilter) {
        quick_bar_state.filter_text.clear();
        toast.message = "🔍 Quick Bar filter cleared".to_string();
        toast.timer = 2.0;
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
    if let Some(ref mut hr) = opt_hr_diagram
        && super::super::hr_diagram_panel::handle_hr_diagram_action(action, hr, toast)
    {
        return true;
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
    if matches!(action, UiButtonAction::ToggleAudioMute) {
        if let Some(audio) = opt_audio {
            crate::game::audio::toggle_cosmic_audio_mute(audio, Some(toast));
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
