//! Interactive UI panel for the Science Economy and Technology Tree.

use super::types::*;
use super::update::{set_display_if_changed, set_text_if_changed};
use crate::simulation::campaign::CampaignState;
use crate::simulation::economy::{
    ALL_TECH_NODES, GameMode, ProgressionRank, ProgressionState, ScienceData, TechNode,
    TechnologyCategory, UnlockedTechs,
};
use bevy::prelude::*;

/// Marker for the live spendable science points summary line.
#[derive(Component)]
pub struct TechTreeSummaryPointsText;

/// Marker for the lifetime cumulative science summary line.
#[derive(Component)]
pub struct TechTreeSummaryLifetimeText;

/// Marker for the active Sandbox / Progression rank & mode status line.
#[derive(Component)]
pub struct TechTreeModeRankText;

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
        bevy::ui::FocusPolicy::Block,
        bevy::picking::hover::Hovered::default(),
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
                    TechTreeSummaryPointsText,
                ));
                banner.spawn((
                    Text::new("Lifetime: 100.0"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.65, 0.75, 0.85)),
                    TechTreeSummaryLifetimeText,
                ));
            });

        // 2b. Progression Mode Toggle & Mastery Rank Banner
        panel
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    margin: UiRect::bottom(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.18, 0.75)),
                BorderColor::all(Color::srgba(0.45, 0.70, 1.0, 0.40)),
            ))
            .with_children(|mode_row| {
                mode_row.spawn((
                    Text::new("Rank: I. Primordial Observer | Sandbox"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.85, 0.45)),
                    TechTreeModeRankText,
                ));
                crate::game::ui::types::create_compact_button(
                    mode_row,
                    UiButtonAction::ToggleGameMode,
                    "⇄ Mode",
                    Color::srgba(0.10, 0.18, 0.34, 0.90),
                    Color::srgb(0.45, 0.88, 1.0),
                );
            });

        // 3. Scrollable Tech Tree Content Container
        panel.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                row_gap: Val::Px(8.0),
                ..default()
            },
            TechTreePanelElement::Content,
        ));
    });
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
        (
            TechnologyCategory::PlanetaryDefense,
            "🛡️ PLANETARY DEFENSE & CIVILIZATIONS",
            Color::srgb(0.55, 0.88, 1.0),
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

fn compute_tech_tree_signature(unlocked_techs: &UnlockedTechs, science_points: f64) -> (u32, u32) {
    let mut unlocked_mask = 0u32;
    let mut affordable_mask = 0u32;
    for (idx, node) in ALL_TECH_NODES.iter().enumerate() {
        if idx >= 32 {
            break;
        }
        let bit = 1u32 << idx;
        if unlocked_techs.has(node.id) {
            unlocked_mask |= bit;
        } else if unlocked_techs.prerequisites_met(node.id) && science_points >= node.cost {
            affordable_mask |= bit;
        }
    }
    (unlocked_mask, affordable_mask)
}

/// Updates panel visibility, in-place summary text, and tech card tree only when unlock/affordability state changes.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Updates tech tree visibility, in-place summary banner, and gated content cards"
)]
pub fn update_tech_tree_panel(
    panel_state: Res<TechTreePanelState>,
    science_data: Res<ScienceData>,
    unlocked_techs: Res<UnlockedTechs>,
    opt_progression: Option<Res<ProgressionState>>,
    opt_campaign: Option<Res<CampaignState>>,
    mut container_query: Query<(&mut Node, &TechTreePanelElement)>,
    mut points_text_query: Query<
        &mut Text,
        (
            With<TechTreeSummaryPointsText>,
            Without<TechTreeSummaryLifetimeText>,
            Without<TechTreeModeRankText>,
        ),
    >,
    mut lifetime_text_query: Query<
        &mut Text,
        (
            With<TechTreeSummaryLifetimeText>,
            Without<TechTreeSummaryPointsText>,
            Without<TechTreeModeRankText>,
        ),
    >,
    mut mode_rank_text_query: Query<
        &mut Text,
        (
            With<TechTreeModeRankText>,
            Without<TechTreeSummaryPointsText>,
            Without<TechTreeSummaryLifetimeText>,
        ),
    >,
    content_query: Query<(Entity, &TechTreePanelElement), Without<Text>>,
    mut commands: Commands,
    mut last_tree_sig: Local<Option<(u32, u32)>>,
) {
    let prog_changed = opt_progression
        .as_ref()
        .is_some_and(DetectChanges::is_changed);
    if !panel_state.is_changed()
        && !science_data.is_changed()
        && !unlocked_techs.is_changed()
        && !prog_changed
    {
        return;
    }

    let target_display = if panel_state.is_open {
        Display::Flex
    } else {
        Display::None
    };
    for (mut node, element) in container_query.iter_mut() {
        if matches!(element, TechTreePanelElement::Container) {
            set_display_if_changed(&mut node, target_display);
        }
    }

    if !panel_state.is_open {
        return;
    }

    if let Ok(mut text) = points_text_query.single_mut() {
        set_text_if_changed(
            &mut text,
            format!(
                "⚛️ {:.1} Science Data (+{:.1}/s)",
                science_data.points, science_data.passive_rate_per_sec
            ),
        );
    }
    if let Ok(mut text) = lifetime_text_query.single_mut() {
        set_text_if_changed(
            &mut text,
            format!("Lifetime: {:.0}", science_data.lifetime_points),
        );
    }
    if let Ok(mut text) = mode_rank_text_query.single_mut() {
        let unlocked_n = unlocked_techs.unlocked.len();
        let missions_n = opt_campaign
            .as_deref()
            .map_or(0, |c| c.completed_missions.len());
        let (mode_lbl, rank) = opt_progression.as_deref().map_or(
            (
                GameMode::Sandbox.short_label(),
                ProgressionRank::PrimordialObserver,
            ),
            |p| (p.mode.short_label(), p.compute_rank(unlocked_n, missions_n)),
        );
        set_text_if_changed(&mut text, format!("{mode_lbl} | Rank: {}", rank.title()));
    }

    let sig = compute_tech_tree_signature(&unlocked_techs, science_data.points);
    if last_tree_sig.as_ref() == Some(&sig) {
        return;
    }
    *last_tree_sig = Some(sig);

    for (entity, element) in content_query.iter() {
        if matches!(element, TechTreePanelElement::Content) {
            commands.entity(entity).despawn_children();
            commands.entity(entity).with_children(|col| {
                rebuild_content_tree(col, &unlocked_techs, &science_data);
            });
        }
    }
}
