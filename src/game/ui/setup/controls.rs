//! Top controls card, view mode badges, and simulation metrics UI builders.

use bevy::prelude::*;

use crate::game::ui::types::*;
use crate::game::ui::widgets::*;

pub(crate) fn spawn_top_controls_card(right_row: &mut ChildSpawnerCommands) {
    right_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                HudButton,
                UiButtonAction::ToggleTopControlsPanel,
                HudPanelElement::TopControlsPill,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.5)),
                    margin: UiRect::all(Val::Px(1.5)),
                    display: Display::Flex,
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.85)),
                BorderColor::all(Color::srgba(0.25, 0.5, 0.8, 0.6)),
            ))
            .with_children(|pill| {
                pill.spawn((
                    Text::new("🎮 Controls ▼"),
                    TextFont {
                        font_size: FontSize::Px(10.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 0.85, 1.0)),
                    Pickable::IGNORE,
                ));
            });

            col.spawn((
                HudPanelElement::TopControlsPanel,
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.88)),
                BorderColor::all(Color::srgba(0.2, 0.4, 0.7, 0.5)),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new("CONTROLS & VIEWS"),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.75, 1.0)),
                    ));
                    create_compact_button(
                        hdr,
                        UiButtonAction::ToggleTopControlsPanel,
                        "🗕",
                        Color::srgba(0.14, 0.08, 0.16, 0.85),
                        Color::srgb(0.9, 0.4, 0.6),
                    );
                });

                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(2.0),
                    max_width: Val::Px(280.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|grid| {
                    spawn_main_tool_shortcuts(grid);
                    spawn_view_mode_badges(grid);
                });

                // Dedicated named save slot and clipboard system seed exchange row
                spawn_save_slot_row(card);
            });
        });
}

fn spawn_save_slot_row(card: &mut ChildSpawnerCommands) {
    card.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(3.0),
            margin: UiRect::top(Val::Px(4.0)),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
            border: UiRect::top(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(Color::srgba(0.2, 0.4, 0.7, 0.4)),
        Name::new("SaveSlotRow"),
    ))
    .with_children(|slot_row| {
        slot_row.spawn((
            Text::new("Slot:"),
            TextFont {
                font: hud_font_source(),
                font_size: FontSize::Px(9.5),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.8, 1.0)),
        ));
        spawn_hud_text_input(
            slot_row,
            HudTextInputKind::SaveSlotName,
            "quicksave",
            64.0,
            9.5,
        );
        create_compact_button(
            slot_row,
            UiButtonAction::SaveNamedSlot,
            "💾",
            Color::srgba(0.08, 0.16, 0.28, 0.90),
            Color::srgb(0.3, 0.75, 1.0),
        );
        create_compact_button(
            slot_row,
            UiButtonAction::LoadNamedSlot,
            "📂",
            Color::srgba(0.08, 0.16, 0.28, 0.90),
            Color::srgb(0.4, 0.85, 0.7),
        );
        create_compact_button(
            slot_row,
            UiButtonAction::CopySystemSeedClipboard,
            "📋 Seed",
            Color::srgba(0.14, 0.10, 0.24, 0.90),
            Color::srgb(0.75, 0.55, 1.0),
        );
        create_compact_button(
            slot_row,
            UiButtonAction::PasteSystemSeedClipboard,
            "📥 Seed",
            Color::srgba(0.14, 0.10, 0.24, 0.90),
            Color::srgb(0.85, 0.65, 1.0),
        );
    });
}

fn spawn_main_tool_shortcuts(col: &mut ChildSpawnerCommands) {
    create_compact_button(
        col,
        UiButtonAction::ToggleScenariosPanel,
        "🌌 Scenarios",
        Color::srgba(0.12, 0.06, 0.20, 0.90),
        Color::srgb(0.75, 0.45, 1.0),
    );
    create_compact_button(
        col,
        UiButtonAction::TogglePlanetBuilder,
        "🪐 Builder [P]",
        Color::srgba(0.08, 0.18, 0.30, 0.90),
        Color::srgb(0.40, 0.85, 1.0),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleTelemetryPanel,
        "📈 Telemetry [F10]",
        Color::srgba(0.05, 0.18, 0.15, 0.90),
        Color::srgb(0.35, 0.95, 0.70),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleHrDiagram,
        "🌟 H-R Diagram [Shift-R]",
        Color::srgba(0.20, 0.12, 0.04, 0.90),
        Color::srgb(1.0, 0.80, 0.35),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleTrophiesPanel,
        "🏆 Trophies",
        Color::srgba(0.20, 0.15, 0.05, 0.90),
        Color::srgb(0.95, 0.75, 0.25),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleTechTreePanel,
        "🔬 Tech [Shift-T]",
        Color::srgba(0.06, 0.16, 0.28, 0.90),
        Color::srgb(0.35, 0.85, 1.0),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleCampaignPanel,
        "🎯 Campaign [Shift-M]",
        Color::srgba(0.20, 0.08, 0.28, 0.90),
        Color::srgb(0.95, 0.50, 1.0),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleHazardsPanel,
        "⚡ Hazards [Shift-H]",
        Color::srgba(0.24, 0.10, 0.04, 0.90),
        Color::srgb(1.0, 0.60, 0.25),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleEpochScrubberPanel,
        "⏳ Epochs [F11]",
        Color::srgba(0.18, 0.14, 0.05, 0.90),
        Color::srgb(1.0, 0.85, 0.40),
    );
    create_compact_button(
        col,
        UiButtonAction::TriggerLhb,
        "☄️ LHB [G]",
        Color::srgba(0.24, 0.12, 0.04, 0.90),
        Color::srgb(1.0, 0.65, 0.20),
    );
    create_compact_button(
        col,
        UiButtonAction::TriggerBlowoutCocoon,
        "💥 Blowout [B]",
        Color::srgba(0.26, 0.08, 0.22, 0.90),
        Color::srgb(1.0, 0.45, 0.95),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleSuperEddington,
        "⚡ Accrete [X]",
        Color::srgba(0.24, 0.06, 0.08, 0.90),
        Color::srgb(1.0, 0.40, 0.50),
    );
    create_compact_button(
        col,
        UiButtonAction::AccelerateInspiral,
        "⚡ Inspiral",
        Color::srgba(0.18, 0.08, 0.24, 0.90),
        Color::srgb(0.85, 0.50, 1.0),
    );
    create_compact_button(
        col,
        UiButtonAction::TriggerCoronalMassEjection,
        "⚡ Flare",
        Color::srgba(0.28, 0.14, 0.02, 0.90),
        Color::srgb(1.0, 0.70, 0.15),
    );
    create_compact_button(
        col,
        UiButtonAction::SpawnInfallPop3Star,
        "🌟 Pop-III [T]",
        Color::srgba(0.08, 0.16, 0.28, 0.90),
        Color::srgb(0.40, 0.85, 1.0),
    );
    create_compact_button(
        col,
        UiButtonAction::TriggerGmcSupernovaShock,
        "💥 Supernova",
        Color::srgba(0.28, 0.08, 0.08, 0.90),
        Color::srgb(1.0, 0.45, 0.30),
    );
    create_compact_button(
        col,
        UiButtonAction::ToggleSlingshotMode,
        "🎯 Slingshot [K]",
        Color::srgba(0.20, 0.10, 0.04, 0.90),
        Color::srgb(1.0, 0.75, 0.30),
    );
    create_compact_button(
        col,
        UiButtonAction::QuickSave,
        "💾 Save [F12]",
        Color::srgba(0.08, 0.14, 0.22, 0.90),
        Color::srgb(0.50, 0.85, 1.0),
    );
    create_compact_button(
        col,
        UiButtonAction::QuickLoad,
        "📂 Load [S-F12]",
        Color::srgba(0.12, 0.12, 0.20, 0.90),
        Color::srgb(0.70, 0.80, 1.0),
    );
}

fn spawn_view_mode_badges(col: &mut ChildSpawnerCommands) {
    spawn_core_view_badges(col);
    spawn_astrodynamic_view_badges(col);
}

fn spawn_core_view_badges(col: &mut ChildSpawnerCommands) {
    // Orbits badge
    col.spawn((
        HudButton,
        UiButtonAction::ToggleOrbitMode,
        HudPanelElement::OrbitModeBadge,
        Node {
            padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
            margin: UiRect::all(Val::Px(1.5)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.05, 0.12, 0.88)),
        BorderColor::all(Color::srgba(0.3, 0.7, 1.0, 0.65)),
    ))
    .with_children(|badge| {
        badge.spawn((
            Text::new("궤 Orbits: Selected [Y]"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.45, 0.90, 1.0)),
            HudDynamicText::OrbitModeBadge,
            Pickable::IGNORE,
        ));
    });

    // Overlay badge
    col.spawn((
        HudButton,
        UiButtonAction::CycleOverlayMode,
        HudPanelElement::OverlayModeBadge,
        Node {
            padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
            margin: UiRect::all(Val::Px(1.5)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.05, 0.12, 0.88)),
        BorderColor::all(Color::srgba(0.3, 0.7, 1.0, 0.65)),
    ))
    .with_children(|badge| {
        badge.spawn((
            Text::new("OVERLAY: Realistic [V]"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.45, 0.90, 1.0)),
            HudDynamicText::OverlayModeBadge,
            Pickable::IGNORE,
        ));
    });

    // Fullscreen badge
    col.spawn((
        HudButton,
        UiButtonAction::ToggleFullScreenHud,
        Node {
            padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
            margin: UiRect::all(Val::Px(1.5)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.05, 0.12, 0.88)),
        BorderColor::all(Color::srgba(0.3, 0.7, 1.0, 0.65)),
    ))
    .with_children(|badge| {
        badge.spawn((
            Text::new("⛶ Fullscreen"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.5, 0.85, 1.0)),
            HudDynamicText::FullScreenBadge,
            Pickable::IGNORE,
        ));
    });

    // Trajectory Predictor / Forecast badge
    col.spawn((
        HudButton,
        UiButtonAction::ToggleTrajectoryPredictor,
        HudPanelElement::TrajectoryPredictorBadge,
        Node {
            padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
            margin: UiRect::all(Val::Px(1.5)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.05, 0.12, 0.88)),
        BorderColor::all(Color::srgba(0.3, 0.7, 1.0, 0.65)),
    ))
    .with_children(|badge| {
        badge.spawn((
            Text::new("🎯 Forecast: ON [N]"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.45, 0.90, 1.0)),
            HudDynamicText::TrajectoryPredictorBadge,
            Pickable::IGNORE,
        ));
    });
}

fn spawn_astrodynamic_view_badges(col: &mut ChildSpawnerCommands) {
    // Lagrange Points badge
    col.spawn((
        HudButton,
        UiButtonAction::ToggleLagrangePoints,
        HudPanelElement::LagrangePointsBadge,
        Node {
            padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
            margin: UiRect::all(Val::Px(1.5)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.05, 0.12, 0.88)),
        BorderColor::all(Color::srgba(0.3, 0.7, 1.0, 0.65)),
    ))
    .with_children(|badge| {
        badge.spawn((
            Text::new("🪐 Lagrange: OFF [Shift-L]"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.45, 0.90, 1.0)),
            HudDynamicText::LagrangePointsBadge,
            Pickable::IGNORE,
        ));
    });

    // Habitable Zone badge
    col.spawn((
        HudButton,
        UiButtonAction::ToggleHabitableZone,
        HudPanelElement::HabitableZoneBadge,
        Node {
            padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
            margin: UiRect::all(Val::Px(1.5)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.05, 0.12, 0.88)),
        BorderColor::all(Color::srgba(0.3, 0.7, 1.0, 0.65)),
    ))
    .with_children(|badge| {
        badge.spawn((
            Text::new("🌱 Hab Zone: ON"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.35, 0.95, 0.60)),
            HudDynamicText::HabitableZoneBadge,
            Pickable::IGNORE,
        ));
    });
}

pub(crate) fn spawn_simulation_metrics_column(right_row: &mut ChildSpawnerCommands) {
    right_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            ..default()
        })
        .with_children(|col| {
            spawn_top_right_pill(col);
            spawn_simulation_metrics_card(col);
        });
}

fn spawn_top_right_pill(col: &mut ChildSpawnerCommands) {
    col.spawn((
        HudButton,
        UiButtonAction::ToggleTopRightPanel,
        HudPanelElement::TopRightPill,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.5)),
            margin: UiRect::all(Val::Px(1.5)),
            display: Display::Flex,
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.85)),
        BorderColor::all(Color::srgba(0.25, 0.5, 0.8, 0.6)),
    ))
    .with_children(|pill| {
        pill.spawn((
            Text::new("📊 Metrics ▼"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.5, 0.85, 1.0)),
            Pickable::IGNORE,
        ));
    });
}

fn spawn_simulation_metrics_card(right_col: &mut ChildSpawnerCommands) {
    right_col
        .spawn((
            HudPanelElement::TopRightPanel,
            Node {
                display: Display::None,
                width: Val::Px(280.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.88)),
            BorderColor::all(Color::srgba(0.2, 0.4, 0.7, 0.5)),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(3.0)),
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new("SIMULATION METRICS"),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.75, 1.0)),
                    ));
                    create_compact_button(
                        hdr,
                        UiButtonAction::ToggleTopRightPanel,
                        "🗕",
                        Color::srgba(0.14, 0.08, 0.16, 0.85),
                        Color::srgb(0.9, 0.4, 0.6),
                    );
                });

            panel.spawn((
                Text::new("SPEED: 1.0x\nSteps: 0"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.9, 1.0)),
                HudTimeWarpText,
            ));
        });
}
