//! Planet Builder & Custom Spawner drawer panel UI layout.

use bevy::prelude::*;

use super::types::*;

/// Spawns the complete floating Planet Builder & Spawner drawer modal.
pub fn spawn_planet_builder_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Button,
        UiButtonAction::ToggleMinimizePlanetBuilder,
        HudPanelElement::PlanetBuilderPill,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(52.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.028, 0.065, 0.92)),
        BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.85)),
    ))
    .with_children(|pill| {
        pill.spawn((
            Text::new("🛠️ Builder [P] ◀"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.4, 0.85, 1.0)),
            Pickable::IGNORE,
        ));
    });

    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(52.0),
            width: Val::Px(360.0),
            max_height: Val::Vh(85.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            overflow: Overflow::clip_y(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.028, 0.065, 0.96)),
        BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.85)),
        PlanetBuilderPanel,
    ))
    .with_children(|panel| {
        spawn_builder_header(panel);
        spawn_builder_presets_section(panel);
        spawn_builder_adjustments_section(panel);
        spawn_builder_actions_section(panel);
    });
}

fn spawn_builder_header(panel: &mut ChildSpawnerCommands) {
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
                    font_size: FontSize::Px(12.5),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));
            hdr.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|btns| {
                create_compact_button(
                    btns,
                    UiButtonAction::ToggleMinimizePlanetBuilder,
                    "🗕",
                    Color::srgba(0.14, 0.08, 0.16, 0.85),
                    Color::srgb(0.9, 0.4, 0.6),
                );
                create_compact_button(
                    btns,
                    UiButtonAction::TogglePlanetBuilder,
                    "✕ Close [P]",
                    Color::srgba(0.24, 0.08, 0.08, 0.9),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });
        });
}

fn spawn_builder_presets_section(panel: &mut ChildSpawnerCommands) {
    panel.spawn((
        Text::new("── 1. SELECT ARCHETYPE PRESET ──"),
        TextFont {
            font_size: FontSize::Px(9.5),
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
        .with_children(|presets| {
            let preset_buttons = [
                (
                    BuilderPreset::EarthLike,
                    "🌍 Earth",
                    Color::srgba(0.05, 0.20, 0.15, 0.9),
                    Color::srgb(0.3, 0.9, 0.6),
                ),
                (
                    BuilderPreset::SuperEarth,
                    "💎 Super-E",
                    Color::srgba(0.04, 0.22, 0.24, 0.9),
                    Color::srgb(0.3, 0.88, 0.85),
                ),
                (
                    BuilderPreset::JupiterLike,
                    "🪐 Jupiter",
                    Color::srgba(0.22, 0.14, 0.06, 0.9),
                    Color::srgb(0.95, 0.65, 0.3),
                ),
                (
                    BuilderPreset::SuperJupiter,
                    "👑 Super-J",
                    Color::srgba(0.08, 0.22, 0.25, 0.9),
                    Color::srgb(0.2, 0.85, 0.8),
                ),
                (
                    BuilderPreset::HeavySuperJupiter,
                    "🔮 Mega-J",
                    Color::srgba(0.18, 0.08, 0.24, 0.9),
                    Color::srgb(0.8, 0.4, 0.95),
                ),
                (
                    BuilderPreset::BrownDwarf,
                    "🟤 Brown Dwarf",
                    Color::srgba(0.24, 0.10, 0.16, 0.9),
                    Color::srgb(0.85, 0.45, 0.65),
                ),
                (
                    BuilderPreset::WaterWorld,
                    "🌊 Ocean",
                    Color::srgba(0.06, 0.18, 0.28, 0.9),
                    Color::srgb(0.3, 0.75, 1.0),
                ),
                (
                    BuilderPreset::MoltenProtoplanet,
                    "🌋 Magma",
                    Color::srgba(0.25, 0.08, 0.04, 0.9),
                    Color::srgb(1.0, 0.45, 0.2),
                ),
                (
                    BuilderPreset::IceGiant,
                    "❄️ Ice Giant",
                    Color::srgba(0.08, 0.16, 0.26, 0.9),
                    Color::srgb(0.5, 0.8, 1.0),
                ),
                (
                    BuilderPreset::RogueInvader,
                    "☄️ Rogue",
                    Color::srgba(0.26, 0.06, 0.14, 0.9),
                    Color::srgb(1.0, 0.35, 0.5),
                ),
                (
                    BuilderPreset::RedDwarfStar,
                    "☀️ M-Star",
                    Color::srgba(0.28, 0.10, 0.04, 0.9),
                    Color::srgb(1.0, 0.5, 0.2),
                ),
            ];

            for (preset, label, bg, border) in preset_buttons {
                create_compact_button(
                    presets,
                    UiButtonAction::BuilderSelectPreset(preset),
                    label,
                    bg,
                    border,
                );
            }
        });

    panel
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(6.0)),
                margin: UiRect::axes(Val::Px(0.0), Val::Px(3.0)),
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
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.95, 1.0)),
                PlanetBuilderInfoText,
            ));
        });
}

fn spawn_builder_adjustments_section(panel: &mut ChildSpawnerCommands) {
    panel.spawn((
        Text::new("── 2. ADJUST MASS & ORBIT ──"),
        TextFont {
            font_size: FontSize::Px(9.5),
            ..default()
        },
        TextColor(Color::srgb(0.5, 0.8, 1.0)),
    ));

    // Mass Steppers
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            margin: UiRect::axes(Val::Px(0.0), Val::Px(2.5)),
            ..default()
        })
        .with_children(|row| {
            create_compact_button(
                row,
                UiButtonAction::BuilderMassStep(-2),
                "Mass ÷10",
                Color::srgba(0.18, 0.08, 0.08, 0.9),
                Color::srgb(0.9, 0.4, 0.4),
            );
            create_compact_button(
                row,
                UiButtonAction::BuilderMassStep(-1),
                "Mass ÷2",
                Color::srgba(0.15, 0.10, 0.08, 0.9),
                Color::srgb(0.9, 0.6, 0.4),
            );
            create_compact_button(
                row,
                UiButtonAction::BuilderMassStep(1),
                "Mass ×2",
                Color::srgba(0.08, 0.16, 0.10, 0.9),
                Color::srgb(0.4, 0.9, 0.5),
            );
            create_compact_button(
                row,
                UiButtonAction::BuilderMassStep(2),
                "Mass ×10",
                Color::srgba(0.06, 0.18, 0.12, 0.9),
                Color::srgb(0.3, 0.95, 0.6),
            );
        });

    // Distance Steppers
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            margin: UiRect::axes(Val::Px(0.0), Val::Px(2.5)),
            ..default()
        })
        .with_children(|row| {
            create_compact_button(
                row,
                UiButtonAction::BuilderDistanceStep(-2),
                "-1.0 AU",
                Color::srgba(0.06, 0.14, 0.22, 0.9),
                Color::srgb(0.3, 0.7, 0.9),
            );
            create_compact_button(
                row,
                UiButtonAction::BuilderDistanceStep(-1),
                "-0.2 AU",
                Color::srgba(0.06, 0.14, 0.22, 0.9),
                Color::srgb(0.3, 0.7, 0.9),
            );
            create_compact_button(
                row,
                UiButtonAction::BuilderDistanceStep(1),
                "+0.2 AU",
                Color::srgba(0.06, 0.14, 0.22, 0.9),
                Color::srgb(0.3, 0.7, 0.9),
            );
            create_compact_button(
                row,
                UiButtonAction::BuilderDistanceStep(2),
                "+1.0 AU",
                Color::srgba(0.06, 0.14, 0.22, 0.9),
                Color::srgb(0.3, 0.7, 0.9),
            );
        });

    // Eccentricity & Composition Cyclers
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            margin: UiRect::axes(Val::Px(0.0), Val::Px(2.5)),
            ..default()
        })
        .with_children(|row| {
            create_compact_button(
                row,
                UiButtonAction::BuilderCycleEccentricity,
                "Cycle Ecc",
                Color::srgba(0.12, 0.10, 0.22, 0.9),
                Color::srgb(0.7, 0.5, 0.95),
            );
            create_compact_button(
                row,
                UiButtonAction::BuilderCycleComposition,
                "Cycle Mix",
                Color::srgba(0.10, 0.16, 0.22, 0.9),
                Color::srgb(0.4, 0.75, 0.95),
            );
        });
}

fn spawn_builder_actions_section(panel: &mut ChildSpawnerCommands) {
    panel.spawn((
        Text::new("── 3. SPAWN WORLD ──"),
        TextFont {
            font_size: FontSize::Px(9.5),
            ..default()
        },
        TextColor(Color::srgb(0.5, 0.8, 1.0)),
    ));

    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            margin: UiRect::top(Val::Px(3.0)),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            create_compact_button(
                row,
                UiButtonAction::BuilderExecuteSpawn,
                "🚀 Insert into Orbit",
                Color::srgba(0.06, 0.24, 0.14, 0.95),
                Color::srgb(0.3, 0.95, 0.6),
            );
            create_compact_button(
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
            font_size: FontSize::Px(9.5),
            ..default()
        },
        TextColor(Color::srgb(0.5, 0.8, 1.0)),
    ));

    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            margin: UiRect::top(Val::Px(3.0)),
            ..default()
        })
        .with_children(|row| {
            create_compact_button(
                row,
                UiButtonAction::SpawnSubRocheMoon,
                "💥 Insert Sub-Roche Moon",
                Color::srgba(0.28, 0.08, 0.16, 0.95),
                Color::srgb(1.0, 0.45, 0.65),
            );
        });
}
