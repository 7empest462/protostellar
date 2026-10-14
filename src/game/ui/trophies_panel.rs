use super::types::*;
use crate::game::phases::{GlobalTrophies, MilestoneId};
use bevy::prelude::*;

pub fn spawn_trophies_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(368.0),
            top: Val::Px(52.0),
            width: Val::Px(385.0),
            max_height: Val::Vh(82.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.02, 0.05, 0.95)),
        Pickable::IGNORE,
        BorderColor::all(Color::srgba(0.95, 0.75, 0.25, 0.90)),
        TrophiesPanelElement::Container,
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            })
            .with_children(|header| {
                header.spawn((
                    Text::new("🏆 GLOBAL TROPHIES & MILESTONES"),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.75, 0.25)),
                ));

                crate::game::ui::types::create_compact_button(
                    header,
                    UiButtonAction::ToggleTrophiesPanel,
                    "✕",
                    Color::srgba(0.8, 0.2, 0.2, 0.8),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });

        panel.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip_y(),
                row_gap: Val::Px(6.0),
                ..default()
            },
            TrophiesPanelElement::Content,
        ));
    });
}

pub fn update_trophies_panel(
    panel_state: Res<TrophiesPanelState>,
    global_trophies: Res<GlobalTrophies>,
    mut container_query: Query<&mut Node, With<TrophiesPanelElement>>,
    content_query: Query<(Entity, &TrophiesPanelElement), Without<Text>>,
    mut commands: Commands,
) {
    if !panel_state.is_changed() && !global_trophies.is_changed() {
        return;
    }

    for mut node in container_query.iter_mut() {
        node.display = if panel_state.is_open {
            Display::Flex
        } else {
            Display::None
        };
    }

    if panel_state.is_open {
        for (entity, element) in content_query.iter() {
            if matches!(element, TrophiesPanelElement::Content) {
                commands.entity(entity).despawn_children();
                commands.entity(entity).with_children(|col| {
                    if global_trophies.unlocked.is_empty() {
                        col.spawn((
                            Text::new("No trophies unlocked yet. Keep exploring!"),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.5, 0.5, 0.5)),
                        ));
                    } else {
                        for trophy in &global_trophies.unlocked {
                            let title = match trophy {
                                MilestoneId::DustCoagulation => "🌱 Dust Coagulation",
                                MilestoneId::PlanetesimalGrowth => "☄️ Embryo Growth",
                                MilestoneId::StellarIgnition => "⭐ Stellar Core Ignition",
                                MilestoneId::GapClearing => "🪐 Annular Gap Clearing",
                                MilestoneId::CoreDifferentiation => "🌍 Core Differentiation",
                                MilestoneId::StableMultiPlanet => "🌟 Multi-Planet Architecture",
                                MilestoneId::GiantPlanetResonance => "🪐 2:1 Giant Resonance",
                                MilestoneId::LateHeavyBombardment => "☄️ Late Heavy Bombardment",
                                MilestoneId::VolatileOceanDelivery => "🌊 Volatile Ocean Genesis",
                                MilestoneId::PlanetaryRingGenesis => "🪐 Planetary Ring Genesis",
                                MilestoneId::DynamoMagneticShield => "🛡️ Dynamo Magnetic Shield",
                                MilestoneId::BiosphereGenesis => "🧬 Biosphere Genesis",
                                MilestoneId::StellarMetamorphosis => "🌟 Stellar Metamorphosis",
                                MilestoneId::GmcJeansCollapse => "🌌 Turbulent Fragmentation",
                                MilestoneId::GmcProtostar => "⭐ Protostar Ignition",
                                MilestoneId::GmcStarCluster => "✨ Open Cluster",
                                MilestoneId::GmcRadiationFeedback => "💨 Ionization Cavity Blowout",
                                MilestoneId::GmcClusterNucleus => "🕳️ Runaway Cluster Core / IMBH",
                                MilestoneId::TrappistSurvival => "🪐 TRAPPIST Resonant Stability",
                                MilestoneId::HotJupiterParked => "🛑 Hot Jupiter Orbital Parking",
                                MilestoneId::SgrACloseApproach => "🕳️ Sgr A* Extreme Perihelion",
                                MilestoneId::QuasarIgnition => {
                                    "🔥 Quasar Awakening / Feeding Frenzy"
                                }
                                MilestoneId::SpiralArmsEmergence => "🌀 Spiral Arms Emergence",
                                MilestoneId::TidalDisruption => "🍝 Spaghettification TDE",
                                MilestoneId::KozaiTiltExchange => "🔄 Kozai-Lidov Tilt Exchange",
                            };
                            col.spawn(Node {
                                padding: UiRect::all(Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            })
                            .insert(BorderColor::all(Color::srgba(0.95, 0.75, 0.25, 0.5)))
                            .insert(BackgroundColor(Color::srgba(0.95, 0.75, 0.25, 0.1)))
                            .with_children(|item| {
                                item.spawn((
                                    Text::new(title),
                                    TextFont {
                                        font_size: FontSize::Px(11.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.9, 0.8, 0.4)),
                                ));
                            });
                        }
                    }
                });
            }
        }
    }
}
