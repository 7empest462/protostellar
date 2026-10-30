//! Scenario presets modal drawer UI for the Heads-Up Display (HUD).

use bevy::prelude::*;

use super::types::*;

pub const SCENARIOS: [(UiButtonAction, &str, &str, Color, Color); 13] = [
    (
        UiButtonAction::LoadScenarioSolar,
        "Solar System [F1]",
        "Hayashi MMSN protoplanetary disk, 8 major planets & asteroid belts.",
        Color::srgba(0.18, 0.14, 0.04, 0.90),
        Color::srgb(0.95, 0.80, 0.35),
    ),
    (
        UiButtonAction::LoadScenarioGenesis,
        "Disk Genesis [Disk]",
        "Pristine gas disk without planets; SPH gas drag, dust settling & accretion.",
        Color::srgba(0.04, 0.18, 0.16, 0.90),
        Color::srgb(0.25, 0.95, 0.80),
    ),
    (
        UiButtonAction::LoadScenarioTrappist,
        "TRAPPIST-1 [F2]",
        "Ultracool red dwarf with 7 resonant terrestrial planets (b–h, 3 in HZ).",
        Color::srgba(0.22, 0.06, 0.08, 0.90),
        Color::srgb(1.00, 0.45, 0.45),
    ),
    (
        UiButtonAction::LoadScenarioKepler16,
        "Kepler-16 [F3]",
        "Tatooine-like circumbinary system: gas giant orbiting close stellar binary.",
        Color::srgba(0.20, 0.12, 0.04, 0.90),
        Color::srgb(1.00, 0.70, 0.30),
    ),
    (
        UiButtonAction::LoadScenarioHotJupiter,
        "Hot Jupiter [F4]",
        "Gas giant undergoing Type II inward migration into 0.05 AU torch orbit.",
        Color::srgba(0.18, 0.08, 0.22, 0.90),
        Color::srgb(0.85, 0.45, 1.00),
    ),
    (
        UiButtonAction::LoadScenarioRoguePlanet,
        "Rogue Planet [F5]",
        "Unbound hyperbolic nomad world plunging through an existing planetary system.",
        Color::srgba(0.06, 0.16, 0.22, 0.90),
        Color::srgb(0.40, 0.85, 1.00),
    ),
    (
        UiButtonAction::LoadScenarioLittleRedDot,
        "Little Red Dot [F6]",
        "High-z JWST quasi-star with super-Eddington accretion and cocoon blowout.",
        Color::srgba(0.24, 0.04, 0.06, 0.90),
        Color::srgb(1.00, 0.35, 0.40),
    ),
    (
        UiButtonAction::LoadScenarioPulsar,
        "Pulsar System [F7]",
        "PSR B1257+12 millisecond pulsar hosting 3 surviving zombie planets.",
        Color::srgba(0.04, 0.12, 0.26, 0.90),
        Color::srgb(0.45, 0.85, 1.00),
    ),
    (
        UiButtonAction::LoadScenarioMagnetar,
        "Magnetar [F9]",
        "SGR 1806-20 with 10^15 Gauss B-field, crust-quakes, and extreme hyper-flares.",
        Color::srgba(0.20, 0.05, 0.28, 0.90),
        Color::srgb(0.90, 0.45, 1.00),
    ),
    (
        UiButtonAction::LoadScenarioKozaiTriple,
        "Kozai Triple [F11]",
        "Hierarchical 3-body system demonstrating eccentricity-inclination cycles.",
        Color::srgba(0.08, 0.18, 0.14, 0.90),
        Color::srgb(0.40, 0.95, 0.65),
    ),
    (
        UiButtonAction::LoadScenarioSagittariusAStar,
        "Sagittarius A* [F10]",
        "Galactic Center: 4.3M M☉ supermassive black hole & relativistic S-stars.",
        Color::srgba(0.25, 0.05, 0.20, 0.90),
        Color::srgb(1.00, 0.50, 0.80),
    ),
    (
        UiButtonAction::LoadScenarioMolecularCloud,
        "GMC Cluster",
        "Giant Molecular Cloud with turbulent fragmentation and protostar collapse.",
        Color::srgba(0.06, 0.16, 0.22, 0.90),
        Color::srgb(0.45, 0.85, 1.00),
    ),
    (
        UiButtonAction::LoadScenarioGalacticCollision,
        "N-Body Galaxy",
        "Milky Way and Andromeda colliding disk galaxies with central SMBHs.",
        Color::srgba(0.12, 0.08, 0.24, 0.90),
        Color::srgb(0.70, 0.55, 1.00),
    ),
];

/// Spawns the centered floating scenario presets modal drawer.
pub fn spawn_scenario_presets_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        HudPanelElement::ScenarioPresets,
        Interaction::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(25.0),
            top: Val::Percent(15.0),
            width: Val::Percent(50.0),
            max_height: Val::Percent(75.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            overflow: Overflow::scroll_y(),
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.96)),
        BorderColor::all(Color::srgba(0.55, 0.35, 0.90, 0.85)),
    ))
    .with_children(|modal| {
        // 1. Header Bar
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
            .with_children(|header| {
                header.spawn((
                    Text::new("🌌 ASTROPHYSICAL SCENARIOS & BENCHMARKS"),
                    TextFont {
                        font_size: FontSize::Px(13.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.65, 1.0)),
                ));

                create_compact_button(
                    header,
                    UiButtonAction::ToggleScenariosPanel,
                    "✕",
                    Color::srgba(0.28, 0.08, 0.12, 0.85),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });

        // 2. Subtitle
        modal.spawn((
            Text::new(
                "Select a celestial sandbox preset, multi-star system, or extreme relativistic benchmark to simulate."
            ),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.75, 0.82, 0.92)),
        ));

        // 3. Scenario Cards Grid
        modal
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Percent(100.0),
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|grid| {
                for (action, title, desc, bg, border) in SCENARIOS {
                    grid.spawn((
                        Button,
                        action,
                        Node {
                            flex_direction: FlexDirection::Column,
                            width: Val::Percent(49.0),
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                            border: UiRect::new(
                                Val::Px(2.5),
                                Val::Px(0.5),
                                Val::Px(0.5),
                                Val::Px(0.5),
                            ),
                            border_radius: BorderRadius {
                                top_left: Val::Px(1.5),
                                top_right: Val::Px(6.0),
                                bottom_right: Val::Px(1.5),
                                bottom_left: Val::Px(6.0),
                            },
                            row_gap: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(bg.with_alpha(0.78)),
                        BorderColor::all(border.with_alpha(0.85)),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new(title),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(border),
                            Pickable::IGNORE,
                        ));
                        card.spawn((
                            Text::new(desc),
                            TextFont {
                                font_size: FontSize::Px(9.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.78, 0.84, 0.92)),
                            Pickable::IGNORE,
                        ));
                    });
                }
            });
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
