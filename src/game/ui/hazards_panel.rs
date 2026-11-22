//! Cosmic Hazards & Sensory Controls UI modal.

use bevy::prelude::*;

use super::types::*;
use crate::simulation::events::{CosmicEventType, CrisisCountermeasureKind};

/// Resource tracking visibility of the Cosmic Hazards panel.
#[derive(Resource, Default)]
pub struct HazardsPanelState {
    pub is_open: bool,
}

/// Marker component for hazard UI modal elements.
#[derive(Component)]
pub enum HazardsPanelElement {
    ModalContainer,
}

const HAZARDS: [(CosmicEventType, &str, &str, Color, Color); 6] = [
    (
        CosmicEventType::RogueStarIncursion,
        "☄️ Rogue Star Incursion",
        "Hyperbolic 0.045 M☉ brown dwarf passes within 15 AU, perturbing planetary orbits.",
        Color::srgba(0.25, 0.12, 0.04, 0.90),
        Color::srgb(1.0, 0.65, 0.30),
    ),
    (
        CosmicEventType::GammaRayBurst,
        "⚡ Gamma-Ray Burst (GRB)",
        "Relativistic ionizing beam strips unshielded atmospheres and tests magnetic dynamos.",
        Color::srgba(0.20, 0.05, 0.28, 0.90),
        Color::srgb(0.95, 0.45, 1.00),
    ),
    (
        CosmicEventType::CometOutburstSwarm,
        "☄️ Cometary Outburst Swarm",
        "Retrograde Oort comets plunge inward, bombarding inner worlds with water ice.",
        Color::srgba(0.04, 0.16, 0.25, 0.90),
        Color::srgb(0.35, 0.85, 1.00),
    ),
    (
        CosmicEventType::SolarSuperflare,
        "☀️ Solar Superflare & CME",
        "Carrington x100 reconnection flare drives G5 auroral curtains and probe EMP disruptions.",
        Color::srgba(0.28, 0.18, 0.02, 0.90),
        Color::srgb(1.00, 0.85, 0.35),
    ),
    (
        CosmicEventType::PlanetKillerSwarm,
        "🪨 Planet-Killer Asteroid Swarm",
        "4 hypervelocity impactors target your top world; deflect via Kinetic Grid or Tractor.",
        Color::srgba(0.30, 0.06, 0.06, 0.90),
        Color::srgb(1.00, 0.40, 0.35),
    ),
    (
        CosmicEventType::GreatFilterCrisis,
        "🌡️ Planetary Great Filter Crisis",
        "Industrial thermal runaway challenges rising civilizations; deploy Climate Stabilizers.",
        Color::srgba(0.24, 0.10, 0.02, 0.90),
        Color::srgb(1.00, 0.58, 0.25),
    ),
];

fn spawn_hazards_cards_list(modal: &mut ChildSpawnerCommands) {
    modal
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            overflow: Overflow::scroll_y(),
            max_height: Val::Vh(42.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|list| {
            for (hazard_type, title, desc, bg, border) in HAZARDS {
                list.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.85)),
                    BorderColor::all(border),
                ))
                .with_children(|card| {
                    card.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        row_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|text_col| {
                        text_col.spawn((
                            Text::new(title),
                            TextFont {
                                font_size: FontSize::Px(11.5),
                                ..default()
                            },
                            TextColor(border),
                        ));
                        text_col.spawn((
                            Text::new(desc),
                            TextFont {
                                font_size: FontSize::Px(9.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.70, 0.78, 0.88)),
                        ));
                    });

                    create_compact_button(
                        card,
                        UiButtonAction::TriggerCosmicHazard(hazard_type),
                        "🚀 Trigger",
                        bg,
                        border,
                    );
                });
            }
        });
}

fn spawn_hazards_defense_footer(modal: &mut ChildSpawnerCommands) {
    modal
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            column_gap: Val::Px(4.0),
            padding: UiRect::top(Val::Px(6.0)),
            border: UiRect::top(Val::Px(1.0)),
            ..default()
        })
        .with_children(|footer| {
            create_compact_button(
                footer,
                UiButtonAction::DeployPlanetaryDefense(
                    CrisisCountermeasureKind::LaunchKineticDeflector,
                ),
                "🚀 Deflect",
                Color::srgba(0.06, 0.20, 0.30, 0.90),
                Color::srgb(0.35, 0.95, 0.75),
            );
            create_compact_button(
                footer,
                UiButtonAction::DeployPlanetaryDefense(
                    CrisisCountermeasureKind::ActivateShieldOverdrive,
                ),
                "🛡️ Shield",
                Color::srgba(0.06, 0.16, 0.32, 0.90),
                Color::srgb(0.35, 0.80, 1.0),
            );
            create_compact_button(
                footer,
                UiButtonAction::DeployPlanetaryDefense(
                    CrisisCountermeasureKind::DeployClimateStabilizer,
                ),
                "❄️ Stabilize",
                Color::srgba(0.08, 0.22, 0.22, 0.90),
                Color::srgb(0.55, 0.95, 1.0),
            );
            create_compact_button(
                footer,
                UiButtonAction::ToggleAutoHazards,
                "🎲 Auto-Hazards",
                Color::srgba(0.08, 0.16, 0.12, 0.85),
                Color::srgb(0.40, 0.95, 0.65),
            );
            create_compact_button(
                footer,
                UiButtonAction::CycleTonemappingMode,
                "🎨 Tonemap [S-C]",
                Color::srgba(0.12, 0.08, 0.22, 0.85),
                Color::srgb(0.85, 0.50, 1.00),
            );
        });
}

/// Spawns the Cosmic Hazards control modal.
pub fn spawn_hazards_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(28.0),
            top: Val::Percent(15.0),
            width: Val::Percent(44.0),
            max_height: Val::Percent(70.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.96)),
        bevy::ui::FocusPolicy::Block,
        bevy::picking::hover::Hovered::default(),
        BorderColor::all(Color::srgba(1.0, 0.45, 0.25, 0.90)),
        HazardsPanelElement::ModalContainer,
    ))
    .with_children(|modal| {
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
                    Text::new("⚡ COSMIC HAZARDS & SENSORY LAB"),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.75, 0.40)),
                ));

                create_compact_button(
                    header,
                    UiButtonAction::ToggleHazardsPanel,
                    "✕",
                    Color::srgba(0.3, 0.08, 0.08, 0.85),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });

        modal.spawn((
            Text::new("Trigger astrophysical events to test planetary magnetospheres, atmospheres, and biosafety."),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.75, 0.82, 0.92)),
        ));

        spawn_hazards_cards_list(modal);
        spawn_hazards_defense_footer(modal);
    });
}

/// Updates visibility of the Cosmic Hazards panel based on user toggle state.
pub fn update_hazards_ui(
    panel_state: Option<Res<HazardsPanelState>>,
    mut query: Query<&mut Node, With<HazardsPanelElement>>,
) {
    let Some(panel) = panel_state else { return };
    if !panel.is_changed() {
        return;
    }

    let target_display = if panel.is_open {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in query.iter_mut() {
        if node.display != target_display {
            node.display = target_display;
        }
    }
}
