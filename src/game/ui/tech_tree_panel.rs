//! Interactive UI panel for the Science Economy and Technology Tree.

use super::types::*;
use crate::simulation::economy::{
    ScienceData, TechNode, TechnologyCategory, UnlockedTechs, ALL_TECH_NODES,
};
use bevy::prelude::*;

/// Spawns the full Tech Tree panel container hierarchy.
pub fn spawn_tech_tree_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(368.0),
            top: Val::Px(52.0),
            width: Val::Px(460.0),
            max_height: Val::Vh(84.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.96)),
        Pickable::IGNORE,
        BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.90)),
        TechTreePanelElement::Container,
    ))
    .with_children(|panel| {
        // 1. Header Bar
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            })
            .with_children(|header| {
                header.spawn((
                    Text::new("🔬 SCIENCE & TECHNOLOGY TREE"),
                    TextFont {
                        font_size: FontSize::Px(12.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.35, 0.85, 1.0)),
                ));

                crate::game::ui::types::create_compact_button(
                    header,
                    UiButtonAction::ToggleTechTreePanel,
                    "✕",
                    Color::srgba(0.8, 0.2, 0.2, 0.8),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });

        // 2. Science Points & Telemetry Income Summary Banner
        panel
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                    margin: UiRect::bottom(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.12, 0.24, 0.75)),
                BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.45)),
                TechTreePanelElement::Summary,
            ))
            .with_children(|banner| {
                banner.spawn((
                    Text::new("⚛️ 100.0 Science Data (+0.0/s)"),
                    TextFont {
                        font_size: FontSize::Px(11.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.40, 0.90, 1.0)),
                ));
                banner.spawn((
                    Text::new("Lifetime: 100.0"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.65, 0.75, 0.85)),
                ));
            });

        // 3. Scrollable Tech Tree Content Container
        panel.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip_y(),
                row_gap: Val::Px(8.0),
                ..default()
            },
            TechTreePanelElement::Content,
        ));
    });
}

fn rebuild_summary_banner(banner: &mut ChildSpawnerCommands, science_data: &ScienceData) {
    banner.spawn((
        Text::new(format!(
            "⚛️ {:.1} Science Data (+{:.1}/s)",
            science_data.points, science_data.passive_rate_per_sec
        )),
        TextFont {
            font_size: FontSize::Px(11.5),
            ..default()
        },
        TextColor(Color::srgb(0.40, 0.90, 1.0)),
    ));
    banner.spawn((
        Text::new(format!("Lifetime: {:.0}", science_data.lifetime_points)),
        TextFont {
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(Color::srgb(0.65, 0.75, 0.85)),
    ));
}

fn spawn_tech_card(
    col: &mut ChildSpawnerCommands,
    node: &TechNode,
    unlocked_techs: &UnlockedTechs,
    science_data: &ScienceData,
) {
    let is_unlocked = unlocked_techs.has(node.id);
    let prereqs_met = unlocked_techs.prerequisites_met(node.id);
    let can_afford = science_data.points >= node.cost;

    let (card_bg, card_border) = if is_unlocked {
        (
            Color::srgba(0.02, 0.12, 0.08, 0.75),
            Color::srgba(0.25, 0.85, 0.50, 0.55),
        )
    } else if prereqs_met {
        (
            Color::srgba(0.03, 0.08, 0.16, 0.75),
            Color::srgba(0.35, 0.75, 1.0, 0.45),
        )
    } else {
        (
            Color::srgba(0.02, 0.03, 0.05, 0.70),
            Color::srgba(0.3, 0.3, 0.35, 0.30),
        )
    };

    col.spawn(Node {
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Px(6.0)),
        border: UiRect::all(Val::Px(1.0)),
        row_gap: Val::Px(3.0),
        ..default()
    })
    .insert(BackgroundColor(card_bg))
    .insert(BorderColor::all(card_border))
    .with_children(|card| {
        // Top row: Title and Action/Status Button
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|top_row| {
            top_row.spawn((
                Text::new(node.name),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(if is_unlocked {
                    Color::srgb(0.5, 1.0, 0.7)
                } else if prereqs_met {
                    Color::srgb(0.9, 0.95, 1.0)
                } else {
                    Color::srgb(0.55, 0.55, 0.60)
                }),
            ));

            if is_unlocked {
                top_row.spawn((
                    Text::new("✓ RESEARCHED"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.3, 0.95, 0.5)),
                ));
            } else if prereqs_met && can_afford {
                crate::game::ui::types::create_compact_button(
                    top_row,
                    UiButtonAction::ResearchTech(node.id),
                    &format!("🔬 Research ({:.0})", node.cost),
                    Color::srgba(0.08, 0.28, 0.45, 0.90),
                    Color::srgb(0.45, 0.90, 1.0),
                );
            } else if prereqs_met {
                top_row.spawn((
                    Text::new(format!("🔒 Need {:.0} Science", node.cost)),
                    TextFont {
                        font_size: FontSize::Px(9.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.55, 0.35)),
                ));
            } else {
                let prereq_names = node
                    .prerequisites
                    .iter()
                    .filter_map(|req_id| {
                        ALL_TECH_NODES
                            .iter()
                            .find(|n| n.id == *req_id)
                            .map(|n| n.name)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                top_row.spawn((
                    Text::new(format!("🔒 Req: {prereq_names}")),
                    TextFont {
                        font_size: FontSize::Px(9.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 0.5, 0.55)),
                ));
            }
        });

        // Bottom row: Description
        card.spawn((
            Text::new(node.description),
            TextFont {
                font_size: FontSize::Px(9.5),
                ..default()
            },
            TextColor(Color::srgb(0.70, 0.75, 0.80)),
        ));
    });
}

fn rebuild_content_tree(
    col: &mut ChildSpawnerCommands,
    unlocked_techs: &UnlockedTechs,
    science_data: &ScienceData,
) {
    let categories = [
        (
            TechnologyCategory::Propulsion,
            "🚀 DEEP-SPACE PROPULSION",
            Color::srgb(0.4, 0.85, 1.0),
        ),
        (
            TechnologyCategory::Terraforming,
            "🌍 PLANETARY TERRAFORMING",
            Color::srgb(0.35, 0.95, 0.65),
        ),
        (
            TechnologyCategory::Megastructures,
            "☀️ MEGASTRUCTURES & ASTROPHYSICS",
            Color::srgb(1.0, 0.80, 0.35),
        ),
    ];

    for (cat, cat_title, cat_color) in categories {
        // Category Header
        col.spawn((
            Text::new(cat_title),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(cat_color),
        ));

        // Tech Cards in this category
        for node in ALL_TECH_NODES.iter().filter(|n| n.category == cat) {
            spawn_tech_card(col, node, unlocked_techs, science_data);
        }
    }
}

/// Updates panel visibility, telemetry summary numbers, and the interactive tech nodes list.
pub fn update_tech_tree_panel(
    panel_state: Res<TechTreePanelState>,
    science_data: Res<ScienceData>,
    unlocked_techs: Res<UnlockedTechs>,
    mut container_query: Query<(&mut Node, &TechTreePanelElement)>,
    content_query: Query<(Entity, &TechTreePanelElement), Without<Text>>,
    mut commands: Commands,
) {
    if !panel_state.is_changed() && !science_data.is_changed() && !unlocked_techs.is_changed() {
        return;
    }

    // Toggle container flex display
    for (mut node, element) in container_query.iter_mut() {
        if matches!(element, TechTreePanelElement::Container) {
            node.display = if panel_state.is_open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    if !panel_state.is_open {
        return;
    }

    // Rebuild Summary Banner and Tech Cards
    for (entity, element) in content_query.iter() {
        match element {
            TechTreePanelElement::Summary => {
                commands.entity(entity).despawn_children();
                commands.entity(entity).with_children(|banner| {
                    rebuild_summary_banner(banner, &science_data);
                });
            }
            TechTreePanelElement::Content => {
                commands.entity(entity).despawn_children();
                commands.entity(entity).with_children(|col| {
                    rebuild_content_tree(col, &unlocked_techs, &science_data);
                });
            }
            TechTreePanelElement::Container => {}
        }
    }
}
