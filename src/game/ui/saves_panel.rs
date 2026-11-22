//! Multi-Save Slot Browser & Scenario Persistence Manager UI.
//!
//! Lets players create named saves or auto-numbered scenario snapshots, load
//! any saved system across scenarios or custom sandboxes, overwrite existing
//! slots, filter by the currently running scenario, and safely delete old saves.

use bevy::prelude::*;

use crate::simulation::scenarios::{ActiveScenarioState, LoadScenarioEvent, ScenarioPreset};
use crate::simulation::serialization::{
    LoadSystemEvent, SaveSlotEntry, SaveSystemEvent, list_save_slot_entries,
    sanitize_save_slot_name,
};

use super::types::{UiButtonAction, create_compact_button};
use super::widgets::{HudTextInputKind, SaveSlotUiState, hud_text_font, spawn_hud_text_input};

/// Resource tracking visibility, filters, and cached save-slot metadata for the Saves modal.
#[derive(Resource, Default, Debug)]
pub struct SavesPanelState {
    pub is_open: bool,
    pub dirty: bool,
    pub pending_refresh_ticks: u8,
    pub filter_active_scenario: bool,
    pub pending_delete_idx: Option<usize>,
    pub entries: Vec<SaveSlotEntry>,
    pub total_saved_count: usize,
}

impl SavesPanelState {
    /// Re-scans `saves/` and refreshes visible entries for the optional active scenario filter.
    pub fn refresh_entries(&mut self, active_preset: Option<ScenarioPreset>) {
        let all_entries = list_save_slot_entries();
        self.total_saved_count = all_entries.len();
        self.entries = if self.filter_active_scenario {
            all_entries
                .into_iter()
                .filter(|entry| entry.scenario_preset == active_preset)
                .collect()
        } else {
            all_entries
        };
        self.pending_delete_idx = None;
        self.dirty = false;
    }
}

/// Marker component for entities belonging to the Saves Manager modal.
#[derive(Component)]
pub enum SavesPanelElement {
    Container,
    SummaryText,
    Content,
}

fn format_epoch_label(epoch_yr: f64) -> String {
    let abs_yr = epoch_yr.abs();
    if abs_yr >= 1.0e9 {
        format!("{:.2} Gyr", epoch_yr / 1.0e9)
    } else if abs_yr >= 1.0e6 {
        format!("{:.2} Myr", epoch_yr / 1.0e6)
    } else if abs_yr >= 1.0e3 {
        format!("{:.1} kyr", epoch_yr / 1.0e3)
    } else {
        format!("{epoch_yr:.1} yr")
    }
}

fn scenario_badge_style(entry: &SaveSlotEntry) -> (&'static str, Color) {
    let Some(preset) = entry.scenario_preset else {
        return ("Custom System", Color::srgb(0.75, 0.82, 0.92));
    };
    let color = match preset {
        ScenarioPreset::SolarNebulaMmsn
        | ScenarioPreset::AccretionDiskGenesis
        | ScenarioPreset::Trappist1System
        | ScenarioPreset::Kepler16Circumbinary => Color::srgb(0.40, 0.88, 1.00),
        ScenarioPreset::HotJupiterMigration
        | ScenarioPreset::RoguePlanetFlyby
        | ScenarioPreset::KozaiLidovTriple => Color::srgb(1.00, 0.78, 0.35),
        ScenarioPreset::LittleRedDot
        | ScenarioPreset::SagittariusAStar
        | ScenarioPreset::TidalDisruptionEvent => Color::srgb(0.95, 0.50, 1.00),
        ScenarioPreset::PulsarSystem
        | ScenarioPreset::MagnetarOutburst
        | ScenarioPreset::RelativisticBinary
        | ScenarioPreset::MolecularCloudCluster
        | ScenarioPreset::GalacticCollision => Color::srgb(0.42, 0.95, 0.72),
    };
    (preset.short_label(), color)
}

/// Spawns the floating Multi-Save Manager modal window (hidden by default).
pub fn spawn_saves_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(21.0),
            top: Val::Percent(11.0),
            width: Val::Percent(58.0),
            max_height: Val::Vh(78.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.96)),
        bevy::ui::FocusPolicy::Block,
        bevy::picking::hover::Hovered::default(),
        BorderColor::all(Color::srgba(0.35, 0.80, 1.0, 0.90)),
        SavesPanelElement::Container,
    ))
    .with_children(|modal| {
        spawn_saves_panel_header(modal);
        spawn_saves_panel_controls(modal);
        modal.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: Val::Px(120.0),
                max_height: Val::Vh(52.0),
                overflow: Overflow::scroll_y(),
                row_gap: Val::Px(6.0),
                padding: UiRect::right(Val::Px(4.0)),
                ..default()
            },
            ScrollPosition::default(),
            SavesPanelElement::Content,
        ));
    });
}

fn spawn_saves_panel_header(modal: &mut ChildSpawnerCommands) {
    modal
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            padding: UiRect::bottom(Val::Px(6.0)),
            border: UiRect::bottom(Val::Px(1.0)),
            ..default()
        })
        .insert(BorderColor::all(Color::srgba(0.25, 0.55, 0.85, 0.45)))
        .with_children(|header| {
            header.spawn((
                Text::new("🗂️ SAVED SYSTEMS & SCENARIO SLOTS"),
                hud_text_font(13.5),
                TextColor(Color::srgb(0.45, 0.90, 1.0)),
            ));
            create_compact_button(
                header,
                UiButtonAction::ToggleSavesPanel,
                "✕",
                Color::srgba(0.30, 0.08, 0.08, 0.85),
                Color::srgb(1.0, 0.4, 0.4),
            );
        });
}

fn spawn_saves_panel_controls(modal: &mut ChildSpawnerCommands) {
    // Row 1: Custom Slot Name input + Save Actions
    modal
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(4.0),
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new("Active Slot:"),
                hud_text_font(10.5),
                TextColor(Color::srgb(0.75, 0.88, 1.0)),
            ));
            spawn_hud_text_input(
                row,
                HudTextInputKind::SaveSlotName,
                "quicksave",
                175.0,
                10.5,
            );
            create_compact_button(
                row,
                UiButtonAction::SaveNamedSlot,
                "💾 Save Slot",
                Color::srgba(0.06, 0.22, 0.36, 0.92),
                Color::srgb(0.35, 0.85, 1.0),
            );
            create_compact_button(
                row,
                UiButtonAction::LoadNamedSlot,
                "📂 Load Slot",
                Color::srgba(0.06, 0.22, 0.20, 0.92),
                Color::srgb(0.35, 0.95, 0.72),
            );
            create_compact_button(
                row,
                UiButtonAction::SaveNewScenarioSlot,
                "✨ + New Scenario Save",
                Color::srgba(0.16, 0.10, 0.30, 0.94),
                Color::srgb(0.82, 0.58, 1.0),
            );
        });

    // Row 2: Summary Status + Filter / Refresh / Seed Tools
    modal
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(4.0),
                width: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.08, 0.16, 0.75)),
            BorderColor::all(Color::srgba(0.25, 0.55, 0.85, 0.40)),
        ))
        .with_children(|toolbar| {
            toolbar.spawn((
                Text::new("Scanning saved slots..."),
                hud_text_font(10.0),
                TextColor(Color::srgb(0.75, 0.90, 1.0)),
                SavesPanelElement::SummaryText,
            ));

            toolbar
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    ..default()
                })
                .with_children(|btns| {
                    create_compact_button(
                        btns,
                        UiButtonAction::ToggleSavesScenarioFilter,
                        "🔍 Filter Scenario",
                        Color::srgba(0.08, 0.14, 0.26, 0.90),
                        Color::srgb(0.45, 0.80, 1.0),
                    );
                    create_compact_button(
                        btns,
                        UiButtonAction::RefreshSaveSlots,
                        "↻ Refresh",
                        Color::srgba(0.08, 0.14, 0.24, 0.90),
                        Color::srgb(0.55, 0.85, 1.0),
                    );
                    create_compact_button(
                        btns,
                        UiButtonAction::CopySystemSeedClipboard,
                        "📋 Copy Seed",
                        Color::srgba(0.14, 0.10, 0.24, 0.90),
                        Color::srgb(0.75, 0.55, 1.0),
                    );
                    create_compact_button(
                        btns,
                        UiButtonAction::PasteSystemSeedClipboard,
                        "📥 Import Seed",
                        Color::srgba(0.14, 0.10, 0.24, 0.90),
                        Color::srgb(0.85, 0.65, 1.0),
                    );
                });
        });
}

fn spawn_save_slot_card(
    list: &mut ChildSpawnerCommands,
    idx: usize,
    entry: &SaveSlotEntry,
    active_sanitized: &str,
    pending_delete: bool,
) {
    let is_active = entry.slot_name.eq_ignore_ascii_case(active_sanitized);
    let (badge_label, badge_color) = scenario_badge_style(entry);
    let (card_bg, card_border) = if is_active {
        (
            Color::srgba(0.03, 0.12, 0.20, 0.86),
            Color::srgba(0.45, 0.92, 1.0, 0.85),
        )
    } else {
        (
            Color::srgba(0.02, 0.05, 0.10, 0.82),
            Color::srgba(0.24, 0.45, 0.68, 0.50),
        )
    };

    list.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            column_gap: Val::Px(8.0),
            width: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(card_bg),
        BorderColor::all(card_border),
    ))
    .with_children(|card| {
        // Left metadata column
        card.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|info| {
            // Top badge line: Slot name + Scenario pill + Format tag
            info.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|title_row| {
                let title = if is_active {
                    format!("● {}", entry.slot_name)
                } else {
                    entry.slot_name.clone()
                };
                title_row.spawn((
                    Text::new(title),
                    hud_text_font(11.5),
                    TextColor(if is_active {
                        Color::srgb(0.45, 0.96, 1.0)
                    } else {
                        Color::srgb(0.92, 0.96, 1.0)
                    }),
                ));
                title_row.spawn((
                    Text::new(format!("[{badge_label}]")),
                    hud_text_font(9.5),
                    TextColor(badge_color),
                ));
                title_row.spawn((
                    Text::new(entry.format_tag),
                    hud_text_font(8.5),
                    TextColor(Color::srgb(0.55, 0.68, 0.80)),
                ));
            });

            // Stats + path subtitle
            let epoch_str = format_epoch_label(entry.timestamp_epoch_yr);
            info.spawn((
                Text::new(format!(
                    "{} bodies  •  T+ {}  •  step {}  •  {}",
                    entry.body_count, epoch_str, entry.step_count, entry.file_path
                )),
                hud_text_font(9.0),
                TextColor(Color::srgb(0.68, 0.78, 0.88)),
            ));
        });

        // Right action buttons
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|actions| {
            create_compact_button(
                actions,
                UiButtonAction::LoadSaveSlot(idx),
                "📂 Load",
                Color::srgba(0.05, 0.22, 0.18, 0.92),
                Color::srgb(0.35, 0.95, 0.70),
            );
            create_compact_button(
                actions,
                UiButtonAction::OverwriteSaveSlot(idx),
                "💾 Save",
                Color::srgba(0.06, 0.18, 0.32, 0.92),
                Color::srgb(0.35, 0.82, 1.0),
            );
            create_compact_button(
                actions,
                UiButtonAction::SelectSaveSlot(idx),
                "✎ Select",
                Color::srgba(0.14, 0.14, 0.24, 0.90),
                Color::srgb(0.75, 0.80, 1.0),
            );
            let (del_label, del_bg, del_border) = if pending_delete {
                (
                    "Confirm?",
                    Color::srgba(0.45, 0.08, 0.08, 0.95),
                    Color::srgb(1.0, 0.30, 0.30),
                )
            } else {
                (
                    "🗑",
                    Color::srgba(0.24, 0.06, 0.08, 0.88),
                    Color::srgb(0.95, 0.40, 0.45),
                )
            };
            create_compact_button(
                actions,
                UiButtonAction::DeleteSaveSlot(idx),
                del_label,
                del_bg,
                del_border,
            );
        });
    });
}

fn rebuild_saves_list(
    list: &mut ChildSpawnerCommands,
    panel: &SavesPanelState,
    active_sanitized: &str,
) {
    if panel.entries.is_empty() {
        let empty_msg = if panel.filter_active_scenario && panel.total_saved_count > 0 {
            "No saves match the current scenario filter. Click '🔍 Filter Scenario' to view all slots, or '✨ + New Scenario Save' to save this system."
        } else {
            "No saved systems found in saves/ yet. Click '✨ + New Scenario Save' above or press [F12] to create your first save."
        };
        list.spawn((
            Node {
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                width: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.09, 0.80)),
            BorderColor::all(Color::srgba(0.25, 0.45, 0.70, 0.40)),
        ))
        .with_children(|empty| {
            empty.spawn((
                Text::new(empty_msg),
                hud_text_font(10.5),
                TextColor(Color::srgb(0.72, 0.82, 0.94)),
            ));
        });
        return;
    }

    for (idx, entry) in panel.entries.iter().enumerate() {
        let pending_delete = panel.pending_delete_idx == Some(idx);
        spawn_save_slot_card(list, idx, entry, active_sanitized, pending_delete);
    }
}

fn consume_refresh_events(
    panel_state: &mut ResMut<SavesPanelState>,
    save_events: &mut MessageReader<SaveSystemEvent>,
    load_events: &mut MessageReader<LoadSystemEvent>,
    scenario_events: &mut MessageReader<LoadScenarioEvent>,
) {
    let events_fired =
        !save_events.is_empty() || !load_events.is_empty() || !scenario_events.is_empty();
    save_events.clear();
    load_events.clear();
    scenario_events.clear();

    if events_fired {
        panel_state.pending_refresh_ticks = 2;
        panel_state.dirty = true;
    } else if panel_state.pending_refresh_ticks > 0 {
        panel_state.pending_refresh_ticks = panel_state.pending_refresh_ticks.saturating_sub(1);
        panel_state.dirty = true;
    }
}

fn render_saves_panel_dom(
    panel: &SavesPanelState,
    active_preset: Option<ScenarioPreset>,
    active_sanitized: &str,
    container_query: &mut Query<(&mut Node, &SavesPanelElement), Without<Text>>,
    summary_query: &mut Query<(&mut Text, &SavesPanelElement)>,
    content_query: &Query<(Entity, &SavesPanelElement), Without<Text>>,
    commands: &mut Commands,
) {
    let display = if panel.is_open {
        Display::Flex
    } else {
        Display::None
    };
    for (mut node, elem) in container_query.iter_mut() {
        if matches!(elem, SavesPanelElement::Container) && node.display != display {
            node.display = display;
        }
    }
    if !panel.is_open {
        return;
    }

    let filter_desc = if panel.filter_active_scenario {
        active_preset.map_or("Custom Only", |p| p.short_label())
    } else {
        "All Scenarios"
    };
    let scenario_now = active_preset.map_or("Custom", |p| p.short_label());
    let summary_line = format!(
        "Showing {} of {} saves  •  Filter: {}  •  Current: {}",
        panel.entries.len(),
        panel.total_saved_count,
        filter_desc,
        scenario_now
    );
    for (mut text, elem) in summary_query.iter_mut() {
        if matches!(elem, SavesPanelElement::SummaryText) && text.0 != summary_line {
            text.0.clone_from(&summary_line);
        }
    }

    for (entity, elem) in content_query.iter() {
        if matches!(elem, SavesPanelElement::Content) {
            commands.entity(entity).despawn_children();
            commands.entity(entity).with_children(|list| {
                rebuild_saves_list(list, panel, active_sanitized);
            });
        }
    }
}

/// Keeps Saves Manager slot list, summary banner, and modal visibility synchronized.
#[allow(
    clippy::too_many_arguments,
    reason = "UI panel synchronization reads active scenario, save events, and HUD nodes"
)]
pub fn update_saves_panel(
    mut panel_state: Option<ResMut<SavesPanelState>>,
    slot_state: Option<Res<SaveSlotUiState>>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    mut save_events: MessageReader<SaveSystemEvent>,
    mut load_events: MessageReader<LoadSystemEvent>,
    mut scenario_events: MessageReader<LoadScenarioEvent>,
    mut container_query: Query<(&mut Node, &SavesPanelElement), Without<Text>>,
    mut summary_query: Query<(&mut Text, &SavesPanelElement)>,
    content_query: Query<(Entity, &SavesPanelElement), Without<Text>>,
    mut commands: Commands,
) {
    let has_events =
        !save_events.is_empty() || !load_events.is_empty() || !scenario_events.is_empty();
    let has_pending = panel_state
        .as_deref()
        .is_some_and(|p| p.pending_refresh_ticks > 0);
    if has_events || has_pending {
        if let Some(ref mut panel) = panel_state {
            consume_refresh_events(
                panel,
                &mut save_events,
                &mut load_events,
                &mut scenario_events,
            );
        }
    } else {
        save_events.clear();
        load_events.clear();
        scenario_events.clear();
    }

    let slot_changed = slot_state
        .as_ref()
        .is_some_and(bevy::ecs::change_detection::DetectChanges::is_changed);
    let scenario_changed = scenario_state
        .as_ref()
        .is_some_and(bevy::ecs::change_detection::DetectChanges::is_changed);

    let needs_render = panel_state
        .as_ref()
        .is_some_and(|p| p.is_changed() || p.dirty)
        || slot_changed
        || scenario_changed;
    if !needs_render {
        return;
    }

    let Some(ref mut panel) = panel_state else {
        return;
    };

    let active_preset = scenario_state.as_ref().map(|s| s.current_preset);
    if panel.is_open && (panel.dirty || (panel.filter_active_scenario && scenario_changed)) {
        panel.refresh_entries(active_preset);
    }

    let active_sanitized = if panel.is_open {
        slot_state.as_ref().map_or_else(
            || "quicksave".to_string(),
            |s| sanitize_save_slot_name(&s.slot_name),
        )
    } else {
        String::new()
    };
    render_saves_panel_dom(
        panel,
        active_preset,
        &active_sanitized,
        &mut container_query,
        &mut summary_query,
        &content_query,
        &mut commands,
    );
}
