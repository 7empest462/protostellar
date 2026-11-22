//! Target inspector panel and context-sensitive action toolbars.

use bevy::prelude::*;

use super::types::*;
use super::widgets::{
    HudSliderKind, HudTextInputKind, hud_font_source, spawn_hud_slider, spawn_hud_text_input,
};

/// Spawns the bottom-left Target Inspector and actions panel.
pub fn spawn_bottom_left_inspector_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            flex_shrink: 1.0,
            min_height: Val::Px(0.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                HudPanelElement::InspectorPanel,
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    width: Val::Px(380.0),
                    max_height: Val::Vh(60.0),
                    flex_shrink: 1.0,
                    min_height: Val::Px(0.0),
                    border: UiRect::all(Val::Px(1.5)),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.09, 0.94)),
                BorderColor::all(Color::srgba(0.25, 0.55, 0.95, 0.75)),
            ))
            .with_children(|panel| {
                spawn_inspector_header(panel);
                spawn_inspector_tab_bar(panel);
                spawn_inspector_scrollable_body(panel);
            });

            spawn_inspector_chip(col);
        });
}

fn spawn_inspector_header(panel: &mut ChildSpawnerCommands) {
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
                Text::new("🔍 TARGET INSPECTOR & ACTIONS"),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.8, 1.0)),
            ));
            hdr.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|right_hdr| {
                create_compact_button(
                    right_hdr,
                    UiButtonAction::ToggleSurfaceHorizonCamera,
                    "🎥 Surface [Shift-V]",
                    Color::srgba(0.06, 0.24, 0.18, 0.92),
                    Color::srgb(0.35, 1.0, 0.78),
                );
                create_compact_button(
                    right_hdr,
                    UiButtonAction::ToggleInternalCutaway,
                    "🔬 Cutaway [Shift-X]",
                    Color::srgba(0.24, 0.12, 0.05, 0.92),
                    Color::srgb(1.0, 0.75, 0.35),
                );
                right_hdr.spawn((
                    Text::new("↕ Scroll"),
                    TextFont {
                        font_size: FontSize::Px(8.5),
                        ..default()
                    },
                    TextColor(Color::srgba(0.4, 0.75, 1.0, 0.8)),
                ));
                create_compact_button(
                    right_hdr,
                    UiButtonAction::ToggleInspectorPanel,
                    "🗕",
                    Color::srgba(0.16, 0.08, 0.12, 0.85),
                    Color::srgb(0.9, 0.4, 0.6),
                );
            });
        });
}

fn spawn_inspector_tab_bar(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            margin: UiRect::bottom(Val::Px(4.0)),
            width: Val::Percent(100.0),
            column_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|bar| {
            const TABS: [(InspectorTab, &str); 3] = [
                (InspectorTab::Overview, "📊 Overview"),
                (InspectorTab::GeodynamicsBio, "🌍 Geo & Bio"),
                (InspectorTab::RelativityFields, "⚡ Fields & GR"),
            ];
            for (tab, label) in TABS {
                super::buttons::create_compact_selectable_button(
                    bar,
                    UiButtonAction::SelectInspectorTab(tab),
                    label,
                    Color::srgba(0.04, 0.08, 0.16, 0.85),
                    Color::srgba(0.25, 0.55, 0.85, 0.6),
                    Color::srgba(0.04, 0.08, 0.16, 0.85),
                    Color::srgba(0.25, 0.55, 0.85, 0.6),
                );
            }
        });
}

fn spawn_inspector_scrollable_body(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn((
            ScrollableInspector,
            ScrollPosition::default(),
            bevy::picking::hover::Hovered::default(),
            Node {
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                min_height: Val::Px(0.0),
                width: Val::Percent(100.0),
                padding: UiRect::right(Val::Px(3.0)),
                ..default()
            },
        ))
        .with_children(|scroll_box| {
            spawn_inspector_text_and_forecast(scroll_box);

            scroll_box
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|actions| {
                    spawn_inspector_toolbar_rows(actions);

                    actions
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
                                margin: UiRect::top(Val::Px(2.0)),
                                border: UiRect::left(Val::Px(3.0)),
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
                                    font_size: FontSize::Px(9.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.75, 0.90, 1.0)),
                                HudActionTooltipText,
                            ));
                        });
                });
        });
}

fn spawn_inspector_chip(col: &mut ChildSpawnerCommands) {
    col.spawn((
        HudButton,
        UiButtonAction::ToggleInspectorPanel,
        HudPanelElement::InspectorChip,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            display: Display::Flex,
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
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.4, 0.85, 1.0)),
            HudDynamicText::InspectorChip,
            Pickable::IGNORE,
        ));
    });
}

fn spawn_inspector_toolbar_rows(actions: &mut ChildSpawnerCommands) {
    spawn_orbit_tracking_row(actions);
    spawn_mass_composition_row(actions);
    spawn_hydrosphere_bathymetry_row(actions);
    spawn_terraforming_bombardment_row(actions);
    spawn_astrophysics_actions_row(actions);
    spawn_exotic_experiments_row(actions);
}

fn spawn_hydrosphere_bathymetry_row(actions: &mut ChildSpawnerCommands) {
    actions
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            InspectorSection::HydrosphereBathymetry,
        ))
        .with_children(|section| {
            section.spawn((
                Text::new("HYDROSPHERE & BATHYMETRY:"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.25, 0.75, 1.0)),
            ));
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    const BUTTONS: [(UiButtonAction, &str, Color, Color); 4] = [
                        (
                            UiButtonAction::AddOceanWater,
                            "+10% Water []]",
                            Color::srgba(0.04, 0.16, 0.28, 0.9),
                            Color::srgb(0.25, 0.75, 1.0),
                        ),
                        (
                            UiButtonAction::DrainOceanWater,
                            "-10% Drain [[",
                            Color::srgba(0.18, 0.12, 0.06, 0.9),
                            Color::srgb(1.0, 0.65, 0.25),
                        ),
                        (
                            UiButtonAction::DryBasin,
                            "Dry Basin [S-[]",
                            Color::srgba(0.22, 0.08, 0.06, 0.9),
                            Color::srgb(1.0, 0.35, 0.25),
                        ),
                        (
                            UiButtonAction::EarthSeaLevel,
                            "Earth (71%)",
                            Color::srgba(0.06, 0.20, 0.16, 0.9),
                            Color::srgb(0.35, 0.95, 0.65),
                        ),
                    ];
                    for (act, label, bg, border) in BUTTONS {
                        create_compact_button(row, act, label, bg, border);
                    }
                });
        });
}

fn spawn_terraforming_bombardment_row(actions: &mut ChildSpawnerCommands) {
    actions
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            InspectorSection::TerraformingBombardment,
        ))
        .with_children(|section| {
            section.spawn((
                Text::new("TERRAFORMING & BOMBARDMENT:"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.35, 0.90, 0.95)),
            ));
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    const BUTTONS: [(UiButtonAction, &str, Color, Color); 4] = [
                        (
                            UiButtonAction::BombardComet,
                            "Comet (H₂O)",
                            Color::srgba(0.06, 0.18, 0.24, 0.9),
                            Color::srgb(0.35, 0.85, 1.0),
                        ),
                        (
                            UiButtonAction::BombardChondrite,
                            "Chondrite (Atm)",
                            Color::srgba(0.22, 0.14, 0.06, 0.9),
                            Color::srgb(1.0, 0.65, 0.25),
                        ),
                        (
                            UiButtonAction::BombardSalvo,
                            "Volatile Salvo",
                            Color::srgba(0.18, 0.08, 0.24, 0.9),
                            Color::srgb(0.80, 0.50, 1.0),
                        ),
                        (
                            UiButtonAction::BombardCore,
                            "Core Impactor",
                            Color::srgba(0.24, 0.08, 0.06, 0.9),
                            Color::srgb(1.0, 0.35, 0.25),
                        ),
                    ];
                    for (act, label, bg, border) in BUTTONS {
                        create_compact_button(row, act, label, bg, border);
                    }
                });
        });
}

fn spawn_orbit_tracking_row(actions: &mut ChildSpawnerCommands) {
    actions
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            InspectorSection::OrbitTracking,
        ))
        .with_children(|section| {
            section.spawn((
                Text::new("ORBIT & TRACKING:"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.75, 1.0)),
            ));
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    const BUTTONS: [(UiButtonAction, &str, Color, Color); 13] = [
                        (
                            UiButtonAction::FocusLock,
                            "Track [F]",
                            Color::srgba(0.08, 0.16, 0.24, 0.9),
                            Color::srgb(0.4, 0.8, 1.0),
                        ),
                        (
                            UiButtonAction::ToggleSurfaceHorizonCamera,
                            "🎥 Surface [Shift-V]",
                            Color::srgba(0.06, 0.24, 0.18, 0.9),
                            Color::srgb(0.35, 1.0, 0.78),
                        ),
                        (
                            UiButtonAction::ToggleInternalCutaway,
                            "🔬 Cutaway [Shift-X]",
                            Color::srgba(0.24, 0.12, 0.05, 0.9),
                            Color::srgb(1.0, 0.75, 0.35),
                        ),
                        (
                            UiButtonAction::FixOrbit,
                            "Fix Orbit [Z]",
                            Color::srgba(0.06, 0.22, 0.22, 0.9),
                            Color::srgb(0.3, 0.95, 0.85),
                        ),
                        (
                            UiButtonAction::TidalLock,
                            "⚡ Lock",
                            Color::srgba(0.12, 0.14, 0.28, 0.9),
                            Color::srgb(0.65, 0.75, 1.0),
                        ),
                        (
                            UiButtonAction::AccelerateInspiral,
                            "⚡ Inspiral",
                            Color::srgba(0.18, 0.08, 0.24, 0.9),
                            Color::srgb(0.85, 0.50, 1.0),
                        ),
                        (
                            UiButtonAction::ExpandOrbit,
                            "+10% [O]",
                            Color::srgba(0.06, 0.15, 0.24, 0.9),
                            Color::srgb(0.3, 0.75, 0.95),
                        ),
                        (
                            UiButtonAction::ContractOrbit,
                            "-10% [L]",
                            Color::srgba(0.06, 0.15, 0.24, 0.9),
                            Color::srgb(0.3, 0.75, 0.95),
                        ),
                        (
                            UiButtonAction::BoostDeltaV,
                            "+Δv [B]",
                            Color::srgba(0.18, 0.15, 0.06, 0.9),
                            Color::srgb(0.95, 0.85, 0.3),
                        ),
                        (
                            UiButtonAction::BrakeDeltaV,
                            "-Δv [Shift-B]",
                            Color::srgba(0.18, 0.12, 0.06, 0.9),
                            Color::srgb(0.95, 0.65, 0.25),
                        ),
                        (
                            UiButtonAction::ToggleTrajectoryPredictor,
                            "Forecast [N]",
                            Color::srgba(0.08, 0.20, 0.25, 0.9),
                            Color::srgb(0.4, 0.9, 1.0),
                        ),
                        (
                            UiButtonAction::ToggleLagrangePoints,
                            "Lagrange [Shift-L]",
                            Color::srgba(0.10, 0.20, 0.30, 0.9),
                            Color::srgb(0.4, 0.85, 1.0),
                        ),
                        (
                            UiButtonAction::DeselectBody,
                            "Deselect [Esc]",
                            Color::srgba(0.15, 0.15, 0.18, 0.9),
                            Color::srgb(0.7, 0.7, 0.8),
                        ),
                    ];
                    for (act, label, bg, border) in BUTTONS {
                        create_compact_button(row, act, label, bg, border);
                    }
                });
            spawn_hud_slider(
                section,
                HudSliderKind::InspectorOrbitAu,
                1.0,
                "Orbit:",
                180.0,
            );
            spawn_hud_slider(
                section,
                HudSliderKind::InspectorAxialTiltDeg,
                23.4,
                "Tilt:",
                180.0,
            );
        });
}

fn spawn_inspector_text_and_forecast(scroll_box: &mut ChildSpawnerCommands) {
    scroll_box
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            margin: UiRect::bottom(Val::Px(4.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new("Name:"),
                TextFont {
                    font: hud_font_source(),
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.8, 1.0)),
            ));
            spawn_hud_text_input(
                row,
                HudTextInputKind::InspectorBodyName,
                "Selected Body",
                160.0,
                10.5,
            );
        });

    scroll_box.spawn((
        Text::new("No celestial body selected. Click on the Star or Planets to inspect & edit."),
        TextFont {
            font: hud_font_source(),
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(Color::srgb(0.90, 0.94, 1.0)),
        HudInspectorText,
    ));

    scroll_box
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.5)),
                margin: UiRect::top(Val::Px(3.0)),
                border: UiRect::left(Val::Px(3.0)),
                width: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.05, 0.10, 0.92)),
            BorderColor::all(Color::srgba(0.3, 0.7, 1.0, 0.45)),
        ))
        .with_children(|fc_box| {
            fc_box.spawn((
                Text::new("🎯 FORECAST [N]: Scanning orbital path..."),
                TextFont {
                    font_size: FontSize::Px(9.5),
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.90, 1.0)),
                HudEncounterForecastText,
            ));
        });
}

fn spawn_mass_composition_row(actions: &mut ChildSpawnerCommands) {
    actions
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            InspectorSection::MassComposition,
        ))
        .with_children(|section| {
            section.spawn((
                Text::new("MASS & COMPOSITION:"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.3, 0.85, 0.6)),
            ));
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    const BUTTONS: [(UiButtonAction, &str, Color, Color); 4] = [
                        (
                            UiButtonAction::IncreaseMass,
                            "Accrete (+25%) [U]",
                            Color::srgba(0.06, 0.18, 0.12, 0.9),
                            Color::srgb(0.3, 0.9, 0.55),
                        ),
                        (
                            UiButtonAction::DecreaseMass,
                            "Strip (-20%) [J]",
                            Color::srgba(0.18, 0.08, 0.08, 0.9),
                            Color::srgb(0.95, 0.4, 0.4),
                        ),
                        (
                            UiButtonAction::CycleComposition,
                            "Material [C]",
                            Color::srgba(0.14, 0.10, 0.24, 0.9),
                            Color::srgb(0.75, 0.55, 1.0),
                        ),
                        (
                            UiButtonAction::VaporizeBody,
                            "Dust [Del]",
                            Color::srgba(0.28, 0.05, 0.05, 0.9),
                            Color::srgb(1.0, 0.3, 0.3),
                        ),
                    ];
                    for (act, label, bg, border) in BUTTONS {
                        create_compact_button(row, act, label, bg, border);
                    }
                });
            spawn_hud_slider(
                section,
                HudSliderKind::InspectorLogMassEarth,
                0.0,
                "Mass:",
                180.0,
            );
        });
}

fn spawn_astrophysics_actions_row(actions: &mut ChildSpawnerCommands) {
    actions
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            InspectorSection::Astrophysics,
        ))
        .with_children(|section| {
            section.spawn((
                Text::new("ASTROPHYSICS & PHENOMENA:"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.75, 0.3)),
            ));
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    const BUTTONS: [(UiButtonAction, &str, Color, Color); 8] = [
                        (
                            UiButtonAction::SeedLife,
                            "Life [E]",
                            Color::srgba(0.04, 0.20, 0.08, 0.9),
                            Color::srgb(0.35, 1.0, 0.45),
                        ),
                        (
                            UiButtonAction::AdvanceCivilization,
                            "Adv. Civ",
                            Color::srgba(0.04, 0.15, 0.25, 0.9),
                            Color::srgb(0.4, 0.9, 1.0),
                        ),
                        (
                            UiButtonAction::ShatterIntoRings,
                            "Rings [X]",
                            Color::srgba(0.18, 0.14, 0.06, 0.9),
                            Color::srgb(1.0, 0.85, 0.35),
                        ),
                        (
                            UiButtonAction::StripAtmosphere,
                            "Strip Atm",
                            Color::srgba(0.20, 0.10, 0.26, 0.9),
                            Color::srgb(0.80, 0.55, 1.0),
                        ),
                        (
                            UiButtonAction::TriggerLhb,
                            "LHB [G]",
                            Color::srgba(0.24, 0.12, 0.04, 0.9),
                            Color::srgb(1.0, 0.65, 0.2),
                        ),
                        (
                            UiButtonAction::InjectEmbryo,
                            "Theia [Alt-M]",
                            Color::srgba(0.08, 0.18, 0.24, 0.9),
                            Color::srgb(0.4, 0.85, 1.0),
                        ),
                        (
                            UiButtonAction::ToggleSurfaceHorizonCamera,
                            "🎥 Surface [Shift-V]",
                            Color::srgba(0.06, 0.24, 0.18, 0.9),
                            Color::srgb(0.35, 1.0, 0.78),
                        ),
                        (
                            UiButtonAction::ToggleInternalCutaway,
                            "🔬 Cutaway [Shift-X]",
                            Color::srgba(0.24, 0.12, 0.05, 0.9),
                            Color::srgb(1.0, 0.75, 0.35),
                        ),
                    ];
                    for (act, label, bg, border) in BUTTONS {
                        create_compact_button(row, act, label, bg, border);
                    }
                });
        });
}

fn spawn_exotic_experiments_row(actions: &mut ChildSpawnerCommands) {
    actions
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            InspectorSection::ExoticExperiments,
        ))
        .with_children(|section| {
            section.spawn((
                Text::new("EXOTIC / LITTLE RED DOT:"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.4, 0.6)),
                InspectorExoticHeader,
            ));
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    const BUTTONS: [(UiButtonAction, &str, Color, Color); 8] = [
                        (
                            UiButtonAction::ToggleSuperEddington,
                            "Hyper-Accretion [X]",
                            Color::srgba(0.24, 0.06, 0.08, 0.9),
                            Color::srgb(1.0, 0.4, 0.5),
                        ),
                        (
                            UiButtonAction::TriggerBlowoutCocoon,
                            "Quasar Blowout [B]",
                            Color::srgba(0.26, 0.08, 0.22, 0.9),
                            Color::srgb(1.0, 0.45, 0.95),
                        ),
                        (
                            UiButtonAction::SpawnInfallPop3Star,
                            "Pop-III TDE [T]",
                            Color::srgba(0.08, 0.16, 0.28, 0.9),
                            Color::srgb(0.4, 0.85, 1.0),
                        ),
                        (
                            UiButtonAction::IgniteStar,
                            "Ignite [I]",
                            Color::srgba(0.24, 0.16, 0.04, 0.9),
                            Color::srgb(1.0, 0.85, 0.3),
                        ),
                        (
                            UiButtonAction::AgeStar,
                            "Age [N]",
                            Color::srgba(0.24, 0.08, 0.16, 0.9),
                            Color::srgb(1.0, 0.45, 0.75),
                        ),
                        (
                            UiButtonAction::TriggerCoronalMassEjection,
                            "⚡ CME Flare",
                            Color::srgba(0.28, 0.14, 0.02, 0.9),
                            Color::srgb(1.0, 0.70, 0.15),
                        ),
                        (
                            UiButtonAction::ToggleTractor,
                            "Tractor [T]",
                            Color::srgba(0.22, 0.08, 0.22, 0.9),
                            Color::srgb(0.95, 0.45, 0.95),
                        ),
                        (
                            UiButtonAction::LaunchProbe,
                            "Launch Probe",
                            Color::srgba(0.1, 0.2, 0.4, 0.9),
                            Color::srgb(0.5, 0.7, 1.0),
                        ),
                    ];
                    for (act, label, bg, border) in BUTTONS {
                        create_compact_button(row, act, label, bg, border);
                    }
                });
            section.spawn((
                Text::new("CIVILIZATION & BIOSPHERE SETUP:"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.92, 0.85)),
            ));
            section
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    const CIV_BUTTONS: [(UiButtonAction, &str, Color, Color); 5] = [
                        (
                            UiButtonAction::ToggleCivOrbitalRing,
                            "🛸 Ring",
                            Color::srgba(0.06, 0.16, 0.24, 0.9),
                            Color::srgb(0.45, 0.88, 1.0),
                        ),
                        (
                            UiButtonAction::ToggleCivSolarMirrors,
                            "🪞 Mirrors",
                            Color::srgba(0.22, 0.18, 0.05, 0.9),
                            Color::srgb(1.0, 0.85, 0.32),
                        ),
                        (
                            UiButtonAction::ToggleCivPlanetaryShield,
                            "🛡️ Shield",
                            Color::srgba(0.05, 0.14, 0.26, 0.9),
                            Color::srgb(0.35, 0.78, 1.0),
                        ),
                        (
                            UiButtonAction::CycleFloraAndBiolum,
                            "🌿 Flora/Glow",
                            Color::srgba(0.05, 0.20, 0.14, 0.9),
                            Color::srgb(0.30, 0.95, 0.65),
                        ),
                        (
                            UiButtonAction::ClearCivilization,
                            "✖ Clear Civ",
                            Color::srgba(0.22, 0.06, 0.08, 0.9),
                            Color::srgb(0.95, 0.40, 0.45),
                        ),
                    ];
                    for (act, label, bg, border) in CIV_BUTTONS {
                        create_compact_button(row, act, label, bg, border);
                    }
                });
        });
}

pub use super::inspector_telemetry::{
    append_circumstellar_disk_telemetry, append_relativistic_jet_telemetry,
    append_space_weather_telemetry, format_composition_water_ice, format_kozai_lidov_telemetry,
};
