//! Setup system and layout builders for the Heads-Up Display (HUD).

use bevy::prelude::*;

use super::types::*;

fn spawn_fullscreen_and_orbit_toggles(commands: &mut Commands) {
    // Floating Master Fullscreen Toggle (Always visible in top-right)
    commands
        .spawn((
            Button,
            UiButtonAction::ToggleFullScreenHud,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.5)),
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
                Text::new("⛶ Fullscreen [F11]"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.85, 1.0)),
                HudDynamicText::FullScreenBadge,
                Pickable::IGNORE,
            ));
        });

    // Floating Orbit Visualization Toggle (Visible in top-right)
    commands
        .spawn((
            Button,
            UiButtonAction::ToggleOrbitMode,
            HudPanelElement::OrbitModeBadge,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(148.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.5)),
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
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.90, 1.0)),
                HudDynamicText::OrbitModeBadge,
                Pickable::IGNORE,
            ));
        });
}

fn spawn_top_left_stats_panel(top_row: &mut ChildSpawnerCommands) {
    top_row
        .spawn((
            HudPanelElement::TopLeftPanel,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                max_width: Val::Px(320.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.84)),
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
                        Text::new("SYSTEM TELEMETRY"),
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
                Text::new("PROTOSTELLAR // Astrophysics Simulator\nInitializing Nebula..."),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                HudHeaderStatsText,
            ));
        });

    top_row
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
                Text::new("📊 System Stats ▼"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.85, 1.0)),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_top_center_quick_bar(top_row: &mut ChildSpawnerCommands) {
    top_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|center_col| {
            center_col.spawn((
                QuickBodySelectorBar,
                Interaction::default(),
                Node {
                    flex_direction: FlexDirection::Row,
                    padding: UiRect::all(Val::Px(3.0)),
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(4.0)),
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
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
                        padding: UiRect::all(Val::Px(2.5)),
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.01, 0.03, 0.07, 0.90)),
                    BorderColor::all(Color::srgba(0.5, 0.3, 0.85, 0.6)),
                ))
                .with_children(|scenario_row| {
                    create_button(
                        scenario_row,
                        UiButtonAction::LoadScenarioSolar,
                        "Solar [F1]",
                        Color::srgba(0.18, 0.14, 0.04, 0.9),
                        Color::srgb(0.9, 0.75, 0.3),
                    );
                    create_button(
                        scenario_row,
                        UiButtonAction::LoadScenarioTrappist,
                        "TRAPPIST-1 [F2]",
                        Color::srgba(0.22, 0.06, 0.08, 0.9),
                        Color::srgb(1.0, 0.45, 0.45),
                    );
                    create_button(
                        scenario_row,
                        UiButtonAction::LoadScenarioKepler16,
                        "Kepler-16 [F3]",
                        Color::srgba(0.20, 0.12, 0.04, 0.9),
                        Color::srgb(1.0, 0.7, 0.3),
                    );
                    create_button(
                        scenario_row,
                        UiButtonAction::LoadScenarioHotJupiter,
                        "Hot Jupiter [F4]",
                        Color::srgba(0.18, 0.08, 0.22, 0.9),
                        Color::srgb(0.85, 0.45, 1.0),
                    );
                    create_button(
                        scenario_row,
                        UiButtonAction::LoadScenarioRoguePlanet,
                        "Rogue Planet [F5]",
                        Color::srgba(0.06, 0.16, 0.22, 0.9),
                        Color::srgb(0.4, 0.85, 1.0),
                    );
                    create_button(
                        scenario_row,
                        UiButtonAction::LoadScenarioLittleRedDot,
                        "Little Red Dot [F6]",
                        Color::srgba(0.24, 0.04, 0.06, 0.9),
                        Color::srgb(1.0, 0.35, 0.4),
                    );
                    create_button(
                        scenario_row,
                        UiButtonAction::LoadScenarioPulsar,
                        "Pulsar [F7]",
                        Color::srgba(0.04, 0.12, 0.26, 0.9),
                        Color::srgb(0.45, 0.85, 1.0),
                    );
                    create_button(
                        scenario_row,
                        UiButtonAction::LoadScenarioMagnetar,
                        "Magnetar [F9]",
                        Color::srgba(0.20, 0.05, 0.28, 0.9),
                        Color::srgb(0.90, 0.45, 1.0),
                    );
                });
        });
}

fn spawn_top_right_controls_panel(top_row: &mut ChildSpawnerCommands) {
    top_row
        .spawn((
            HudPanelElement::TopRightPanel,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                align_items: AlignItems::FlexEnd,
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::right(Val::Px(110.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
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
                    create_compact_button(
                        hdr,
                        UiButtonAction::ToggleTopRightPanel,
                        "🗕",
                        Color::srgba(0.14, 0.08, 0.16, 0.85),
                        Color::srgb(0.9, 0.4, 0.6),
                    );
                    hdr.spawn((
                        Text::new("SIMULATION METRICS"),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.75, 1.0)),
                    ));
                });

            panel.spawn((
                Text::new("SPEED: 1.0x\nSteps: 0"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.9, 1.0)),
                HudTimeWarpText,
            ));

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|btn_row| {
                    create_button(
                        btn_row,
                        UiButtonAction::TimePause,
                        "Pause",
                        Color::srgba(0.18, 0.08, 0.08, 0.85),
                        Color::srgb(0.9, 0.4, 0.4),
                    );
                    create_button(
                        btn_row,
                        UiButtonAction::TimeSpeed1,
                        "1x",
                        Color::srgba(0.06, 0.14, 0.22, 0.85),
                        Color::srgb(0.3, 0.7, 0.9),
                    );
                    create_button(
                        btn_row,
                        UiButtonAction::TimeSpeed100,
                        "100x",
                        Color::srgba(0.06, 0.14, 0.22, 0.85),
                        Color::srgb(0.3, 0.7, 0.9),
                    );
                    create_button(
                        btn_row,
                        UiButtonAction::TimeSpeed10k,
                        "10k/s",
                        Color::srgba(0.06, 0.14, 0.22, 0.85),
                        Color::srgb(0.3, 0.7, 0.9),
                    );
                    create_button(
                        btn_row,
                        UiButtonAction::TimeSpeed1M,
                        "1M/s",
                        Color::srgba(0.12, 0.08, 0.22, 0.85),
                        Color::srgb(0.7, 0.4, 0.95),
                    );
                });
        });

    top_row
        .spawn((
            Button,
            UiButtonAction::ToggleTopRightPanel,
            HudPanelElement::TopRightPill,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                display: Display::None,
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::right(Val::Px(110.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.85)),
            BorderColor::all(Color::srgba(0.25, 0.5, 0.8, 0.6)),
        ))
        .with_children(|pill| {
            pill.spawn((
                Text::new("⏱️ Speed Controls ▼"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.85, 1.0)),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_inspector_toolbar_rows(actions: &mut ChildSpawnerCommands) {
    // Row 1: Camera Focus & Mass Alteration
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row1| {
            create_compact_button(
                row1,
                UiButtonAction::FocusLock,
                "Track Cam [F]",
                Color::srgba(0.08, 0.16, 0.24, 0.9),
                Color::srgb(0.4, 0.8, 1.0),
            );
            create_compact_button(
                row1,
                UiButtonAction::IncreaseMass,
                "Accrete (+25%) [U]",
                Color::srgba(0.06, 0.18, 0.12, 0.9),
                Color::srgb(0.3, 0.9, 0.55),
            );
            create_compact_button(
                row1,
                UiButtonAction::DecreaseMass,
                "Strip (-20%) [J]",
                Color::srgba(0.18, 0.08, 0.08, 0.9),
                Color::srgb(0.95, 0.4, 0.4),
            );
            create_compact_button(
                row1,
                UiButtonAction::IgniteStar,
                "Ignite Star [I]",
                Color::srgba(0.24, 0.16, 0.04, 0.9),
                Color::srgb(1.0, 0.85, 0.3),
            );
            create_compact_button(
                row1,
                UiButtonAction::DeselectBody,
                "Deselect [Esc]",
                Color::srgba(0.15, 0.15, 0.18, 0.9),
                Color::srgb(0.7, 0.7, 0.8),
            );
        });

    // Row 2: Orbital Maneuvers (Radius & Delta-V)
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row2| {
            create_compact_button(
                row2,
                UiButtonAction::FixOrbit,
                "Fix Orbit [Z]",
                Color::srgba(0.06, 0.22, 0.22, 0.9),
                Color::srgb(0.3, 0.95, 0.85),
            );
            create_compact_button(
                row2,
                UiButtonAction::ExpandOrbit,
                "+10% Orbit [O]",
                Color::srgba(0.06, 0.15, 0.24, 0.9),
                Color::srgb(0.3, 0.75, 0.95),
            );
            create_compact_button(
                row2,
                UiButtonAction::ContractOrbit,
                "-10% Orbit [L]",
                Color::srgba(0.06, 0.15, 0.24, 0.9),
                Color::srgb(0.3, 0.75, 0.95),
            );
            create_compact_button(
                row2,
                UiButtonAction::BoostDeltaV,
                "Boost +dv [B]",
                Color::srgba(0.18, 0.15, 0.06, 0.9),
                Color::srgb(0.95, 0.85, 0.3),
            );
            create_compact_button(
                row2,
                UiButtonAction::BrakeDeltaV,
                "Brake -dv [K]",
                Color::srgba(0.18, 0.12, 0.06, 0.9),
                Color::srgb(0.95, 0.65, 0.25),
            );
        });

    // Row 3: Material, Spawning, Science & Lifecycle
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row3| {
            create_compact_button(
                row3,
                UiButtonAction::TogglePlanetBuilder,
                "🪐 Builder [P]",
                Color::srgba(0.10, 0.22, 0.36, 0.95),
                Color::srgb(0.4, 0.85, 1.0),
            );
            create_compact_button(
                row3,
                UiButtonAction::ToggleOrbitMode,
                "궤 Orbits [Y]",
                Color::srgba(0.08, 0.20, 0.32, 0.95),
                Color::srgb(0.4, 0.85, 1.0),
            );
            create_compact_button(
                row3,
                UiButtonAction::CycleComposition,
                "Material [C]",
                Color::srgba(0.14, 0.10, 0.24, 0.9),
                Color::srgb(0.75, 0.55, 1.0),
            );
            create_compact_button(
                row3,
                UiButtonAction::InjectEmbryo,
                "Moon [M]",
                Color::srgba(0.08, 0.18, 0.24, 0.9),
                Color::srgb(0.4, 0.85, 1.0),
            );
            create_compact_button(
                row3,
                UiButtonAction::TriggerLhb,
                "LHB [G]",
                Color::srgba(0.24, 0.12, 0.04, 0.9),
                Color::srgb(1.0, 0.65, 0.2),
            );
            create_compact_button(
                row3,
                UiButtonAction::ShatterIntoRings,
                "Rings [X]",
                Color::srgba(0.18, 0.14, 0.06, 0.9),
                Color::srgb(1.0, 0.85, 0.35),
            );
            create_compact_button(
                row3,
                UiButtonAction::SeedLife,
                "Life [E]",
                Color::srgba(0.04, 0.20, 0.08, 0.9),
                Color::srgb(0.35, 1.0, 0.45),
            );
            create_compact_button(
                row3,
                UiButtonAction::AgeStar,
                "Age [N]",
                Color::srgba(0.24, 0.08, 0.16, 0.9),
                Color::srgb(1.0, 0.45, 0.75),
            );
            create_compact_button(
                row3,
                UiButtonAction::ToggleTractor,
                "Tractor [T]",
                Color::srgba(0.22, 0.08, 0.22, 0.9),
                Color::srgb(0.95, 0.45, 0.95),
            );
            create_compact_button(
                row3,
                UiButtonAction::VaporizeBody,
                "Dust [Del]",
                Color::srgba(0.28, 0.05, 0.05, 0.9),
                Color::srgb(1.0, 0.3, 0.3),
            );
        });

    // Row 4: JWST Little Red Dot / Black Hole Star Experiments
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row4| {
            create_compact_button(
                row4,
                UiButtonAction::ToggleSuperEddington,
                "Hyper-Accretion [X]",
                Color::srgba(0.24, 0.06, 0.08, 0.9),
                Color::srgb(1.0, 0.4, 0.5),
            );
            create_compact_button(
                row4,
                UiButtonAction::TriggerBlowoutCocoon,
                "Blowout (Quasar) [B]",
                Color::srgba(0.26, 0.08, 0.22, 0.9),
                Color::srgb(1.0, 0.45, 0.95),
            );
            create_compact_button(
                row4,
                UiButtonAction::SpawnInfallPop3Star,
                "Pop-III TDE [T]",
                Color::srgba(0.08, 0.16, 0.28, 0.9),
                Color::srgb(0.4, 0.85, 1.0),
            );
        });
}

fn spawn_bottom_left_inspector_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn((
            HudPanelElement::InspectorPanel,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                min_width: Val::Px(460.0),
                max_width: Val::Px(520.0),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.09, 0.94)),
            BorderColor::all(Color::srgba(0.25, 0.55, 0.95, 0.75)),
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
                        Text::new("🔍 CELESTIAL INSPECTOR & ACTION TOOLBAR"),
                        TextFont {
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.8, 1.0)),
                    ));
                    create_compact_button(
                        hdr,
                        UiButtonAction::ToggleInspectorPanel,
                        "🗕 Collapse",
                        Color::srgba(0.16, 0.08, 0.12, 0.85),
                        Color::srgb(0.9, 0.4, 0.6),
                    );
                });

            panel.spawn((
                Text::new(
                    "No celestial body selected. Click on the Star or Planets to inspect & edit.",
                ),
                TextFont {
                    font_size: FontSize::Px(11.5),
                    ..default()
                },
                TextColor(Color::srgb(0.90, 0.94, 1.0)),
                HudInspectorText,
            ));

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    margin: UiRect::top(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|actions| {
                    spawn_inspector_toolbar_rows(actions);

                    actions
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                                margin: UiRect::top(Val::Px(2.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                width: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.01, 0.02, 0.05, 0.95)),
                            BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.4)),
                        ))
                        .with_children(|tip_box| {
                            tip_box.spawn((
                                Text::new("Hover over buttons for descriptions."),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.75, 0.90, 1.0)),
                                HudActionTooltipText,
                            ));
                        });
                });
        });

    bottom_row
        .spawn((
            Button,
            UiButtonAction::ToggleInspectorPanel,
            HudPanelElement::InspectorChip,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                display: Display::None,
                border: UiRect::all(Val::Px(1.5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.09, 0.92)),
            BorderColor::all(Color::srgba(0.25, 0.55, 0.95, 0.75)),
        ))
        .with_children(|chip| {
            chip.spawn((
                Text::new("🔍 Inspector ▲ Expand"),
                TextFont {
                    font_size: FontSize::Px(11.5),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.85, 1.0)),
                HudDynamicText::InspectorChip,
                Pickable::IGNORE,
            ));
        });
}

fn spawn_bottom_center_toast_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            margin: UiRect::horizontal(Val::Px(8.0)),
            ..default()
        })
        .with_children(|center_dock| {
            center_dock
                .spawn((
                    HudToastContainer,
                    Node {
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                        margin: UiRect::bottom(Val::Px(4.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        max_width: Val::Px(750.0),
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
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.9, 1.0)),
                        HudToastText,
                    ));
                });

            center_dock
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        min_width: Val::Px(340.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.015, 0.035, 0.075, 0.92)),
                    BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.7)),
                ))
                .with_children(|time_box| {
                    time_box.spawn((
                        Text::new("T+ 0.00 yr | PAUSED"),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.92, 0.4)),
                        HudBottomTimerText,
                    ));

                    time_box
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            margin: UiRect::top(Val::Px(4.0)),
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
                                UiButtonAction::TimeSpeed1,
                                "1x [1]",
                                Color::srgba(0.08, 0.16, 0.24, 0.9),
                                Color::srgb(0.4, 0.8, 1.0),
                            );
                            create_compact_button(
                                speed_row,
                                UiButtonAction::TimeSpeed100,
                                "100x [2]",
                                Color::srgba(0.08, 0.16, 0.24, 0.9),
                                Color::srgb(0.4, 0.8, 1.0),
                            );
                            create_compact_button(
                                speed_row,
                                UiButtonAction::TimeSpeed10k,
                                "10k [3]",
                                Color::srgba(0.12, 0.18, 0.28, 0.9),
                                Color::srgb(0.5, 0.85, 1.0),
                            );
                            create_compact_button(
                                speed_row,
                                UiButtonAction::TimeSpeed1M,
                                "1M [4]",
                                Color::srgba(0.18, 0.14, 0.32, 0.9),
                                Color::srgb(0.7, 0.6, 1.0),
                            );
                        });
                });
        });
}

fn spawn_bottom_right_controls_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.88)),
            BorderColor::all(Color::srgba(0.2, 0.4, 0.7, 0.5)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("360 NAVIGATION & CONTROLS:\n[Right-Drag] 360 Orbit View  |  [WASD / QE] Free-Fly Pan  |  [Scroll] Smooth Zoom\n[Left-Click / Tab] Select Celestial Body  |  [F] Focus-Lock  |  [Esc / R] Deselect\n[F11] Master Fullscreen  |  [Click Any Button] Instant Mouse Action"),
                TextFont { font_size: FontSize::Px(9.5), ..default() },
                TextColor(Color::srgb(0.75, 0.82, 0.95)),
            ));
        });
}

fn spawn_planet_builder_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(65.0),
            width: Val::Px(380.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.028, 0.065, 0.96)),
        BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.85)),
        PlanetBuilderPanel,
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            })
            .with_children(|hdr| {
                hdr.spawn((
                    Text::new("🛠️ PLANET BUILDER & SPAWNER"),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.95, 1.0)),
                ));
                create_button(
                    hdr,
                    UiButtonAction::TogglePlanetBuilder,
                    "✕ Close [P]",
                    Color::srgba(0.24, 0.08, 0.08, 0.9),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });

        panel.spawn((
            Text::new("── 1. SELECT ARCHETYPE PRESET ──"),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(Color::srgb(0.5, 0.8, 1.0)),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::axes(Val::Px(0.0), Val::Px(4.0)),
                ..default()
            })
            .with_children(|presets| {
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::EarthLike),
                    "🌍 Earth",
                    Color::srgba(0.05, 0.20, 0.15, 0.9),
                    Color::srgb(0.3, 0.9, 0.6),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::SuperEarth),
                    "💎 Super-E",
                    Color::srgba(0.04, 0.22, 0.24, 0.9),
                    Color::srgb(0.3, 0.88, 0.85),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::JupiterLike),
                    "🪐 Jupiter",
                    Color::srgba(0.22, 0.14, 0.06, 0.9),
                    Color::srgb(0.95, 0.65, 0.3),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::SuperJupiter),
                    "👑 Super-Jup",
                    Color::srgba(0.08, 0.22, 0.25, 0.9),
                    Color::srgb(0.2, 0.85, 0.8),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::HeavySuperJupiter),
                    "🔮 Mega-Jov",
                    Color::srgba(0.18, 0.08, 0.24, 0.9),
                    Color::srgb(0.8, 0.4, 0.95),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::BrownDwarf),
                    "🟤 Brown Dwarf",
                    Color::srgba(0.24, 0.10, 0.16, 0.9),
                    Color::srgb(0.85, 0.45, 0.65),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::WaterWorld),
                    "🌊 Ocean",
                    Color::srgba(0.06, 0.18, 0.28, 0.9),
                    Color::srgb(0.3, 0.75, 1.0),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::MoltenProtoplanet),
                    "🌋 Magma",
                    Color::srgba(0.25, 0.08, 0.04, 0.9),
                    Color::srgb(1.0, 0.45, 0.2),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::IceGiant),
                    "❄️ Ice Giant",
                    Color::srgba(0.08, 0.16, 0.26, 0.9),
                    Color::srgb(0.5, 0.8, 1.0),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::RogueInvader),
                    "☄️ Rogue",
                    Color::srgba(0.26, 0.06, 0.14, 0.9),
                    Color::srgb(1.0, 0.35, 0.5),
                );
                create_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(BuilderPreset::RedDwarfStar),
                    "☀️ M-Star",
                    Color::srgba(0.28, 0.10, 0.04, 0.9),
                    Color::srgb(1.0, 0.5, 0.2),
                );
            });

        panel
            .spawn((
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    margin: UiRect::axes(Val::Px(0.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.008, 0.016, 0.035, 0.95)),
                BorderColor::all(Color::srgba(0.25, 0.55, 0.85, 0.5)),
            ))
            .with_children(|box_node| {
                box_node.spawn((
                    Text::new("Initializing Planet Builder..."),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.95, 1.0)),
                    PlanetBuilderInfoText,
                ));
            });

        panel.spawn((
            Text::new("── 2. ADJUST MASS & ORBIT ──"),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(Color::srgb(0.5, 0.8, 1.0)),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::axes(Val::Px(0.0), Val::Px(3.0)),
                ..default()
            })
            .with_children(|row| {
                create_button(
                    row,
                    UiButtonAction::BuilderMassStep(-2),
                    "Mass ÷10",
                    Color::srgba(0.18, 0.08, 0.08, 0.9),
                    Color::srgb(0.9, 0.4, 0.4),
                );
                create_button(
                    row,
                    UiButtonAction::BuilderMassStep(-1),
                    "Mass ÷2",
                    Color::srgba(0.15, 0.10, 0.08, 0.9),
                    Color::srgb(0.9, 0.6, 0.4),
                );
                create_button(
                    row,
                    UiButtonAction::BuilderMassStep(1),
                    "Mass ×2",
                    Color::srgba(0.08, 0.16, 0.10, 0.9),
                    Color::srgb(0.4, 0.9, 0.5),
                );
                create_button(
                    row,
                    UiButtonAction::BuilderMassStep(2),
                    "Mass ×10",
                    Color::srgba(0.06, 0.18, 0.12, 0.9),
                    Color::srgb(0.3, 0.95, 0.6),
                );
            });

        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::axes(Val::Px(0.0), Val::Px(3.0)),
                ..default()
            })
            .with_children(|row| {
                create_button(
                    row,
                    UiButtonAction::BuilderDistanceStep(-2),
                    "-1.0 AU",
                    Color::srgba(0.06, 0.14, 0.22, 0.9),
                    Color::srgb(0.3, 0.7, 0.9),
                );
                create_button(
                    row,
                    UiButtonAction::BuilderDistanceStep(-1),
                    "-0.2 AU",
                    Color::srgba(0.06, 0.14, 0.22, 0.9),
                    Color::srgb(0.3, 0.7, 0.9),
                );
                create_button(
                    row,
                    UiButtonAction::BuilderDistanceStep(1),
                    "+0.2 AU",
                    Color::srgba(0.06, 0.14, 0.22, 0.9),
                    Color::srgb(0.3, 0.7, 0.9),
                );
                create_button(
                    row,
                    UiButtonAction::BuilderDistanceStep(2),
                    "+1.0 AU",
                    Color::srgba(0.06, 0.14, 0.22, 0.9),
                    Color::srgb(0.3, 0.7, 0.9),
                );
            });

        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::axes(Val::Px(0.0), Val::Px(3.0)),
                ..default()
            })
            .with_children(|row| {
                create_button(
                    row,
                    UiButtonAction::BuilderCycleEccentricity,
                    "Cycle Ecc",
                    Color::srgba(0.12, 0.10, 0.22, 0.9),
                    Color::srgb(0.7, 0.5, 0.95),
                );
                create_button(
                    row,
                    UiButtonAction::BuilderCycleComposition,
                    "Cycle Mix",
                    Color::srgba(0.10, 0.16, 0.22, 0.9),
                    Color::srgb(0.4, 0.75, 0.95),
                );
            });

        panel.spawn((
            Text::new("── 3. SPAWN WORLD ──"),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(Color::srgb(0.5, 0.8, 1.0)),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::top(Val::Px(4.0)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|row| {
                create_button(
                    row,
                    UiButtonAction::BuilderExecuteSpawn,
                    "🚀 Insert into Orbit",
                    Color::srgba(0.06, 0.24, 0.14, 0.95),
                    Color::srgb(0.3, 0.95, 0.6),
                );
                create_button(
                    row,
                    UiButtonAction::BuilderToggleClickSpawn,
                    "🎯 Click-in-3D Mode",
                    Color::srgba(0.20, 0.12, 0.04, 0.95),
                    Color::srgb(1.0, 0.75, 0.3),
                );
            });

        panel.spawn((
            Text::new("── 4. ASTROPHYSICAL EXPERIMENTS ──"),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(Color::srgb(0.5, 0.8, 1.0)),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                create_button(
                    row,
                    UiButtonAction::SpawnSubRocheMoon,
                    "💥 Insert Sub-Roche Moon",
                    Color::srgba(0.28, 0.08, 0.16, 0.95),
                    Color::srgb(1.0, 0.45, 0.65),
                );
            });
    });
}

/// Sets up the complete HUD overlay interface with interactive on-screen toolbars.
pub fn setup_hud(mut commands: Commands) {
    commands.init_resource::<NotificationToast>();
    commands.init_resource::<HudVisibilityState>();

    spawn_fullscreen_and_orbit_toggles(&mut commands);

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
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            // TOP ROW
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

            // BOTTOM ROW
            root.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|bottom_row| {
                spawn_bottom_left_inspector_panel(bottom_row);
                spawn_bottom_center_toast_panel(bottom_row);
                spawn_bottom_right_controls_panel(bottom_row);
            });

            // PLANET BUILDER MODAL
            spawn_planet_builder_panel(root);
        });
}
