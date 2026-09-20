//! Setup system and layout builders for the Heads-Up Display (HUD).

use bevy::prelude::*;

use super::types::*;

fn spawn_top_left_stats_panel(top_row: &mut ChildSpawnerCommands) {
    top_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            ..default()
        })
        .with_children(|left_col| {
            left_col
                .spawn((
                    HudPanelElement::TopLeftPanel,
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(8.0)),
                        width: Val::Px(280.0),
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
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        })
                        .with_children(|hdr| {
                            hdr.spawn((
                                Text::new("🌌 PROTOSTELLAR // Telemetry"),
                                TextFont {
                                    font_size: FontSize::Px(10.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.4, 0.75, 1.0)),
                            ));
                            create_compact_button(
                                hdr,
                                UiButtonAction::ToggleTopLeftPanel,
                                "🗕",
                                Color::srgba(0.14, 0.08, 0.16, 0.85),
                                Color::srgb(0.9, 0.4, 0.6),
                            );
                        });

                    panel.spawn((
                        Text::new("Initializing Nebula..."),
                        TextFont {
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.95, 1.0)),
                        HudHeaderStatsText,
                    ));
                });

            left_col
                .spawn((
                    Button,
                    UiButtonAction::ToggleTopLeftPanel,
                    HudPanelElement::TopLeftPill,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        display: Display::None,
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
                        Text::new("🌌 PROTOSTELLAR  |  📊 Stats ▼"),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.85, 1.0)),
                        Pickable::IGNORE,
                    ));
                });
        });
}

fn spawn_top_center_quick_bar(top_row: &mut ChildSpawnerCommands) {
    top_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            max_width: Val::Px(640.0),
            ..default()
        })
        .with_children(|center_col| {
            center_col.spawn((
                QuickBodySelectorBar,
                Interaction::default(),
                Node {
                    flex_direction: FlexDirection::Row,
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.5)),
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(3.0)),
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.85)),
                BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.6)),
            ));

            center_col
                .spawn((
                    HudPanelElement::ScenarioPresets,
                    Interaction::default(),
                    Node {
                        flex_direction: FlexDirection::Row,
                        padding: UiRect::axes(Val::Px(3.0), Val::Px(2.0)),
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(3.0)),
                        flex_wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.01, 0.03, 0.07, 0.90)),
                    BorderColor::all(Color::srgba(0.5, 0.3, 0.85, 0.6)),
                ))
                .with_children(|scenario_row| {
                    const SCENARIO_BUTTONS: [(UiButtonAction, &str, Color, Color); 9] = [
                        (
                            UiButtonAction::LoadScenarioSolar,
                            "Solar [F1]",
                            Color::srgba(0.18, 0.14, 0.04, 0.9),
                            Color::srgb(0.9, 0.75, 0.3),
                        ),
                        (
                            UiButtonAction::LoadScenarioTrappist,
                            "TRAPPIST-1 [F2]",
                            Color::srgba(0.22, 0.06, 0.08, 0.9),
                            Color::srgb(1.0, 0.45, 0.45),
                        ),
                        (
                            UiButtonAction::LoadScenarioKepler16,
                            "Kepler-16 [F3]",
                            Color::srgba(0.20, 0.12, 0.04, 0.9),
                            Color::srgb(1.0, 0.7, 0.3),
                        ),
                        (
                            UiButtonAction::LoadScenarioHotJupiter,
                            "Hot Jup [F4]",
                            Color::srgba(0.18, 0.08, 0.22, 0.9),
                            Color::srgb(0.85, 0.45, 1.0),
                        ),
                        (
                            UiButtonAction::LoadScenarioRoguePlanet,
                            "Rogue [F5]",
                            Color::srgba(0.06, 0.16, 0.22, 0.9),
                            Color::srgb(0.4, 0.85, 1.0),
                        ),
                        (
                            UiButtonAction::LoadScenarioLittleRedDot,
                            "LRD [F6]",
                            Color::srgba(0.24, 0.04, 0.06, 0.9),
                            Color::srgb(1.0, 0.35, 0.4),
                        ),
                        (
                            UiButtonAction::LoadScenarioPulsar,
                            "Pulsar [F7]",
                            Color::srgba(0.04, 0.12, 0.26, 0.9),
                            Color::srgb(0.45, 0.85, 1.0),
                        ),
                        (
                            UiButtonAction::LoadScenarioMagnetar,
                            "Magnetar [F9]",
                            Color::srgba(0.20, 0.05, 0.28, 0.9),
                            Color::srgb(0.90, 0.45, 1.0),
                        ),
                        (
                            UiButtonAction::LoadScenarioKozaiTriple,
                            "Kozai [F11]",
                            Color::srgba(0.08, 0.18, 0.14, 0.9),
                            Color::srgb(0.4, 0.95, 0.65),
                        ),
                    ];
                    for (action, label, bg, border) in SCENARIO_BUTTONS {
                        create_compact_button(scenario_row, action, label, bg, border);
                    }
                    create_compact_button(
                        scenario_row,
                        UiButtonAction::ToggleScenariosPanel,
                        "🗕",
                        Color::srgba(0.16, 0.08, 0.12, 0.85),
                        Color::srgb(0.9, 0.4, 0.6),
                    );
                });

            center_col
                .spawn((
                    Button,
                    UiButtonAction::ToggleScenariosPanel,
                    HudPanelElement::ScenarioPresetsPill,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        display: Display::None,
                        border: UiRect::all(Val::Px(1.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.01, 0.03, 0.07, 0.88)),
                    BorderColor::all(Color::srgba(0.5, 0.3, 0.85, 0.6)),
                ))
                .with_children(|pill| {
                    pill.spawn((
                        Text::new("🎬 Scenarios [F1-F9] ▼"),
                        TextFont {
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.65, 1.0)),
                        Pickable::IGNORE,
                    ));
                });
        });
}

fn spawn_top_right_controls_panel(top_row: &mut ChildSpawnerCommands) {
    top_row
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|right_row| {
            spawn_top_controls_card(right_row);
            spawn_simulation_metrics_column(right_row);
        });
}

fn spawn_top_controls_card(right_row: &mut ChildSpawnerCommands) {
    right_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Button,
                UiButtonAction::ToggleTopControlsPanel,
                HudPanelElement::TopControlsPill,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.5)),
                    margin: UiRect::all(Val::Px(1.5)),
                    display: Display::None,
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
                    column_gap: Val::Px(3.0),
                    align_items: AlignItems::FlexStart,
                    ..default()
                })
                .with_children(|grid| {
                    grid.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|col1| {
                        spawn_main_tool_shortcuts(col1);
                    });

                    grid.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|col2| {
                        spawn_view_mode_badges(col2);
                    });
                });
            });
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
        UiButtonAction::ToggleEpochScrubberPanel,
        "⏳ Epochs [F11]",
        Color::srgba(0.18, 0.14, 0.05, 0.90),
        Color::srgb(1.0, 0.85, 0.40),
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
    // Orbits badge
    col.spawn((
        Button,
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
        Button,
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
        Button,
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
        Button,
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

fn spawn_simulation_metrics_column(right_row: &mut ChildSpawnerCommands) {
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
        Button,
        UiButtonAction::ToggleTopRightPanel,
        HudPanelElement::TopRightPill,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.5)),
            margin: UiRect::all(Val::Px(1.5)),
            display: Display::None,
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

fn spawn_bottom_center_toast_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            margin: UiRect::horizontal(Val::Px(6.0)),
            max_width: Val::Px(520.0),
            ..default()
        })
        .with_children(|center_dock| {
            spawn_toast_banner(center_dock);
            spawn_bottom_center_pill(center_dock);
            spawn_time_controls_dock(center_dock);
        });
}

fn spawn_toast_banner(center_dock: &mut ChildSpawnerCommands) {
    center_dock
        .spawn((
            HudToastContainer,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(3.5)),
                margin: UiRect::bottom(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                max_width: Val::Px(520.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.04, 0.09, 0.90)),
            BorderColor::all(Color::srgba(0.25, 0.60, 0.95, 0.65)),
        ))
        .with_children(|toast_box| {
            toast_box.spawn((
                Text::new(">> PROTOSTELLAR LIVE"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.9, 1.0)),
                HudToastText,
            ));
        });
}

fn spawn_bottom_center_pill(center_dock: &mut ChildSpawnerCommands) {
    center_dock
        .spawn((
            Button,
            UiButtonAction::ToggleBottomCenterPanel,
            HudPanelElement::BottomCenterPill,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                display: Display::None,
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.035, 0.075, 0.88)),
            BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.6)),
        ))
        .with_children(|pill| {
            pill.spawn((
                Text::new("⏱️ Time Controls ▲"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.85, 0.4)),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_time_controls_dock(center_dock: &mut ChildSpawnerCommands) {
    center_dock
        .spawn((
            HudPanelElement::BottomCenterPanel,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                min_width: Val::Px(320.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.035, 0.075, 0.92)),
            BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.7)),
        ))
        .with_children(|time_box| {
            time_box
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new("T+ 0.00 yr | PAUSED"),
                        TextFont {
                            font_size: FontSize::Px(12.5),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.92, 0.4)),
                        HudBottomTimerText,
                    ));
                    create_compact_button(
                        hdr,
                        UiButtonAction::ToggleBottomCenterPanel,
                        "🗕",
                        Color::srgba(0.14, 0.08, 0.16, 0.85),
                        Color::srgb(0.9, 0.4, 0.6),
                    );
                });

            time_box
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::top(Val::Px(3.0)),
                    ..default()
                })
                .with_children(|speed_row| {
                    create_compact_button(
                        speed_row,
                        UiButtonAction::TimePause,
                        "Pause [Space]",
                        Color::srgba(0.24, 0.08, 0.08, 0.9),
                        Color::srgb(1.0, 0.4, 0.4),
                    );
                    create_compact_button(
                        speed_row,
                        UiButtonAction::TimeSpeedRealtime,
                        "Real [1]",
                        Color::srgba(0.08, 0.20, 0.16, 0.9),
                        Color::srgb(0.4, 1.0, 0.7),
                    );
                    create_compact_button(
                        speed_row,
                        UiButtonAction::TimeSpeed1,
                        "11d [2]",
                        Color::srgba(0.08, 0.16, 0.24, 0.9),
                        Color::srgb(0.4, 0.8, 1.0),
                    );
                    create_compact_button(
                        speed_row,
                        UiButtonAction::TimeSpeed100,
                        "100x [4]",
                        Color::srgba(0.08, 0.16, 0.24, 0.9),
                        Color::srgb(0.4, 0.8, 1.0),
                    );
                    create_compact_button(
                        speed_row,
                        UiButtonAction::TimeSpeed10k,
                        "10k [6]",
                        Color::srgba(0.12, 0.18, 0.28, 0.9),
                        Color::srgb(0.5, 0.85, 1.0),
                    );
                    create_compact_button(
                        speed_row,
                        UiButtonAction::TimeSpeed1M,
                        "1M [8]",
                        Color::srgba(0.18, 0.14, 0.32, 0.9),
                        Color::srgb(0.7, 0.6, 1.0),
                    );
                });
        });
}

fn spawn_bottom_right_controls_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            ..default()
        })
        .with_children(|right_col| {
            right_col
                .spawn((
                    Button,
                    UiButtonAction::ToggleBottomRightPanel,
                    HudPanelElement::BottomRightPill,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        display: Display::None,
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
                        Text::new("⌨ Shortcuts [?] ▲"),
                        TextFont {
                            font_size: FontSize::Px(9.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.85, 1.0)),
                        Pickable::IGNORE,
                    ));
                });

            right_col
                .spawn((
                    HudPanelElement::BottomRightPanel,
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(6.0)),
                        max_width: Val::Px(240.0),
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
                            margin: UiRect::bottom(Val::Px(3.0)),
                            ..default()
                        })
                        .with_children(|hdr| {
                            hdr.spawn((
                                Text::new("⌨ SHORTCUTS & HELP"),
                                TextFont {
                                    font_size: FontSize::Px(9.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.4, 0.75, 1.0)),
                            ));
                            create_compact_button(
                                hdr,
                                UiButtonAction::ToggleBottomRightPanel,
                                "🗕",
                                Color::srgba(0.14, 0.08, 0.16, 0.85),
                                Color::srgb(0.9, 0.4, 0.6),
                            );
                        });

                    panel.spawn((
                        Text::new("⌨ NAVIGATION & SHORTCUTS:\n[R-Drag] 360 Orbit | [WASD] Pan | [Scroll] Zoom\n[Click / Tab] Select | [F] Track | [Esc] Deselect\n[Space] Pause | [1..4] Speed | [F10] Telemetry | [F11] Epochs"),
                        TextFont {
                            font_size: FontSize::Px(9.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.75, 0.82, 0.95)),
                    ));
                });
        });
}

/// Sets up the complete HUD overlay interface with interactive on-screen toolbars.
pub fn setup_hud(mut commands: Commands) {
    commands.init_resource::<NotificationToast>();
    commands.init_resource::<HudVisibilityState>();

    commands
        .spawn((
            HudPanelElement::RootContainer,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            // TOP ROW: Clean header containing Telemetry (left), Quick Selector & Scenarios (center), Badges & Metrics (right)
            root.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                ..default()
            })
            .with_children(|top_row| {
                spawn_top_left_stats_panel(top_row);
                spawn_top_center_quick_bar(top_row);
                spawn_top_right_controls_panel(top_row);
            });

            // BOTTOM ROW: Target Inspector (left), Toast & Playbar (center), Navigation Help (right)
            root.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|bottom_row| {
                super::inspector_panel::spawn_bottom_left_inspector_panel(bottom_row);
                spawn_bottom_center_toast_panel(bottom_row);
                spawn_bottom_right_controls_panel(bottom_row);
            });

            // PLANET BUILDER MODAL DRAWER
            super::builder_panel::spawn_planet_builder_panel(root);

            // TELEMETRY & CLIMATE GRAPHING DRAWER
            super::telemetry_panel::spawn_telemetry_panel(root);

            // DEEP-TIME GEOLOGICAL EPOCH SCRUBBER DRAWER
            super::epoch_scrubber::spawn_epoch_scrubber_panel(root);
        });
}
