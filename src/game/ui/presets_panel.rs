//! Scenario presets panel and pill UI builders for the Heads-Up Display (HUD).

use bevy::prelude::*;

use super::types::*;

pub fn spawn_scenario_presets_panel(center_col: &mut ChildSpawnerCommands) {
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
            const SCENARIO_BUTTONS: [(UiButtonAction, &str, Color, Color); 13] = [
                (
                    UiButtonAction::LoadScenarioSolar,
                    "Solar [F1]",
                    Color::srgba(0.18, 0.14, 0.04, 0.9),
                    Color::srgb(0.9, 0.75, 0.3),
                ),
                (
                    UiButtonAction::LoadScenarioGenesis,
                    "Genesis [Disk]",
                    Color::srgba(0.04, 0.18, 0.16, 0.9),
                    Color::srgb(0.25, 0.95, 0.8),
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
                (
                    UiButtonAction::LoadScenarioSagittariusAStar,
                    "Sgr A* [F10]",
                    Color::srgba(0.25, 0.05, 0.20, 0.9),
                    Color::srgb(1.0, 0.5, 0.8),
                ),
                (
                    UiButtonAction::LoadScenarioMolecularCloud,
                    "GMC Cluster",
                    Color::srgba(0.06, 0.16, 0.22, 0.9),
                    Color::srgb(0.45, 0.85, 1.0),
                ),
                (
                    UiButtonAction::LoadScenarioGalacticCollision,
                    "N-Body Galaxy",
                    Color::srgba(0.06, 0.16, 0.22, 0.9),
                    Color::srgb(0.45, 0.85, 1.0),
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
}

pub fn spawn_scenario_presets_pill(center_col: &mut ChildSpawnerCommands) {
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
}
