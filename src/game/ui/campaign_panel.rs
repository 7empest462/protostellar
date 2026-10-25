//! Campaign Mode UI: Mission Selector Modal and Floating Objective Tracker HUD.

use bevy::prelude::*;

use super::types::*;
use crate::simulation::campaign::{
    CampaignMissionId, CampaignState, MissionObjective, MissionStatus,
};

#[derive(Resource, Default)]
pub struct CampaignPanelState {
    pub is_open: bool,
    pub is_tracker_minimized: bool,
}

#[derive(Component)]
pub enum CampaignPanelElement {
    ModalContainer,
    ModalContent,
    TrackerContainer,
    TrackerContent,
}

/// Spawns both the Campaign Selector Modal and the Floating Objectives Tracker.
pub fn spawn_campaign_panels(root: &mut ChildSpawnerCommands) {
    spawn_campaign_selector_modal(root);
    spawn_campaign_tracker_panel(root);
}

/// Spawns the central modal for selecting and launching campaign scenario challenges.
fn spawn_campaign_selector_modal(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(25.0),
            top: Val::Percent(10.0),
            width: Val::Percent(50.0),
            max_height: Val::Percent(80.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.96)),
        Pickable::IGNORE,
        BorderColor::all(Color::srgba(0.75, 0.45, 1.0, 0.90)),
        CampaignPanelElement::ModalContainer,
    ))
    .with_children(|modal| {
        // 1. Header Bar
        modal
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|hdr| {
                hdr.spawn((
                    Text::new("🎯 CAMPAIGN & SCENARIO CHALLENGES"),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.60, 1.0)),
                ));

                create_compact_button(
                    hdr,
                    UiButtonAction::ToggleCampaignPanel,
                    "✕",
                    Color::srgba(0.8, 0.2, 0.2, 0.8),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });

        // 2. Lore / Instructions Banner
        modal.spawn((
            Text::new("Execute goal-oriented astrophysics missions. Fulfill all mandatory parameters to earn massive Science Data bounties."),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgb(0.70, 0.75, 0.85)),
        ));

        // 3. Scrollable Missions Grid / List
        modal.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip_y(),
                row_gap: Val::Px(8.0),
                ..default()
            },
            CampaignPanelElement::ModalContent,
        ));
    });
}

/// Spawns the floating on-screen HUD card that tracks live mission progress.
fn spawn_campaign_tracker_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(200.0),
            width: Val::Px(310.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            row_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.92)),
        Pickable::IGNORE,
        BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.80)),
        CampaignPanelElement::TrackerContainer,
    ))
    .with_children(|tracker| {
        // Dynamic Tracker Contents
        tracker.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            CampaignPanelElement::TrackerContent,
        ));
    });
}

/// Spawns an individual mission challenge card inside the selector modal.
fn spawn_mission_card(
    col: &mut ChildSpawnerCommands,
    mission: CampaignMissionId,
    active_mission: Option<CampaignMissionId>,
) {
    let is_active = active_mission == Some(mission);
    let (card_bg, card_border) = if is_active {
        (
            Color::srgba(0.08, 0.04, 0.16, 0.90),
            Color::srgba(0.95, 0.55, 1.0, 0.90),
        )
    } else {
        (
            Color::srgba(0.02, 0.04, 0.08, 0.75),
            Color::srgba(0.35, 0.50, 0.75, 0.50),
        )
    };

    col.spawn(Node {
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Px(8.0)),
        border: UiRect::all(Val::Px(1.0)),
        row_gap: Val::Px(4.0),
        ..default()
    })
    .insert(BackgroundColor(card_bg))
    .insert(BorderColor::all(card_border))
    .with_children(|card| {
        // Row 1: Title, Difficulty, and Science Bounty
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|top| {
            top.spawn((
                Text::new(format!("{} // {}", mission.title(), mission.difficulty())),
                TextFont {
                    font_size: FontSize::Px(11.5),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.85, 0.40)),
            ));

            top.spawn((
                Text::new(format!("⚛️ +{} Science", mission.science_reward())),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.40, 0.95, 0.60)),
            ));
        });

        // Row 2: Mission Briefing Lore & Objectives
        card.spawn((
            Text::new(mission.briefing()),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(Color::srgb(0.75, 0.80, 0.90)),
        ));

        // Row 3: Action Buttons
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(3.0)),
            ..default()
        })
        .with_children(|actions| {
            if is_active {
                actions.spawn((
                    Text::new("● IN PROGRESS"),
                    TextFont {
                        font_size: FontSize::Px(10.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.75, 0.20)),
                ));
            }

            create_compact_button(
                actions,
                UiButtonAction::StartCampaignMission(mission),
                if is_active {
                    "↺ RESTART"
                } else {
                    "🚀 LAUNCH MISSION"
                },
                Color::srgba(0.18, 0.08, 0.28, 0.90),
                Color::srgb(0.95, 0.50, 1.0),
            );
        });
    });
}

/// Rebuilds the tracker header and objective checklist.
fn rebuild_tracker_content(
    col: &mut ChildSpawnerCommands,
    state: &CampaignState,
    is_minimized: bool,
) {
    let Some(mission) = state.active_mission else {
        return;
    };

    // Header Row
    col.spawn(Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|hdr| {
        hdr.spawn((
            Text::new(format!("🎯 {}", mission.title())),
            TextFont {
                font_size: FontSize::Px(11.5),
                ..default()
            },
            TextColor(Color::srgb(0.85, 0.60, 1.0)),
        ));

        create_compact_button(
            hdr,
            UiButtonAction::ToggleCampaignTracker,
            if is_minimized { "[+]" } else { "[-]" },
            Color::srgba(0.10, 0.15, 0.25, 0.85),
            Color::srgb(0.50, 0.85, 1.0),
        );
    });

    if is_minimized {
        return;
    }

    // Elapsed Mission Time
    col.spawn((
        Text::new(format!(
            "Mission Clock: {:.1} yr",
            state.elapsed_mission_years
        )),
        TextFont {
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(Color::srgb(0.55, 0.75, 0.95)),
    ));

    // Objectives List
    for obj in &state.objectives {
        spawn_objective_row(col, obj);
    }

    // Status Banner & Mission Actions
    match state.status {
        MissionStatus::Victory => {
            col.spawn((
                Text::new("🏆 MISSION ACCOMPLISHED!"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.30, 0.95, 0.50)),
            ));

            if state.reward_claimed {
                col.spawn((
                    Text::new("✓ Reward Claimed"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.40, 0.80, 0.50)),
                ));
            } else {
                create_compact_button(
                    col,
                    UiButtonAction::ClaimCampaignReward,
                    &format!("⚛️ CLAIM BOUNTY (+{} SCIENCE)", mission.science_reward()),
                    Color::srgba(0.08, 0.22, 0.12, 0.95),
                    Color::srgb(0.35, 0.95, 0.55),
                );
            }
        }
        MissionStatus::Defeat => {
            col.spawn((
                Text::new(format!(
                    "✗ MISSION FAILED: {}",
                    state
                        .failure_reason
                        .as_deref()
                        .unwrap_or("Critical parameters breached")
                )),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.35, 0.35)),
            ));

            create_compact_button(
                col,
                UiButtonAction::StartCampaignMission(mission),
                "↺ RETRY MISSION",
                Color::srgba(0.24, 0.08, 0.08, 0.95),
                Color::srgb(1.0, 0.45, 0.45),
            );
        }
        MissionStatus::InProgress | MissionStatus::NotStarted => {
            create_compact_button(
                col,
                UiButtonAction::AbandonCampaignMission,
                "✕ ABORT MISSION",
                Color::srgba(0.18, 0.06, 0.06, 0.80),
                Color::srgb(0.85, 0.40, 0.40),
            );
        }
    }
}

/// Spawns a single objective item row with mini progress bar.
fn spawn_objective_row(col: &mut ChildSpawnerCommands, obj: &MissionObjective) {
    let (status_icon, icon_color) = if obj.is_completed {
        ("✓", Color::srgb(0.30, 0.95, 0.50))
    } else if obj.is_failed {
        ("✗", Color::srgb(1.0, 0.35, 0.35))
    } else {
        ("○", Color::srgb(0.40, 0.85, 1.0))
    };

    let frac = (obj.progress_fraction() * 100.0).clamp(0.0, 100.0) as f32;

    col.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(2.0),
        margin: UiRect::axes(Val::Px(0.0), Val::Px(2.0)),
        ..default()
    })
    .with_children(|item| {
        // Text Row
        item.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|txt_row| {
            txt_row.spawn((
                Text::new(format!("{status_icon} {}", obj.title)),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(icon_color),
            ));

            txt_row.spawn((
                Text::new(format!(
                    "{:.2}/{:.2} {} ({:.0}%)",
                    obj.current_value, obj.target_value, obj.unit, frac
                )),
                TextFont {
                    font_size: FontSize::Px(9.5),
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.80, 0.90)),
            ));
        });

        // Mini Progress Bar
        item.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(3.0),
                border: UiRect::all(Val::Px(0.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.80)),
            BorderColor::all(Color::srgba(0.30, 0.40, 0.60, 0.40)),
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    width: Val::Percent(frac),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(if obj.is_completed {
                    Color::srgb(0.30, 0.95, 0.50)
                } else if obj.is_failed {
                    Color::srgb(0.95, 0.30, 0.30)
                } else {
                    Color::srgb(0.35, 0.75, 1.0)
                }),
            ));
        });
    });
}

/// Dynamic system updating visibility, modal content, and live objective tracker.
#[allow(
    clippy::type_complexity,
    reason = "Campaign UI system queries container nodes and content elements"
)]
pub fn update_campaign_ui(
    panel_state: Res<CampaignPanelState>,
    campaign_state: Res<CampaignState>,
    mut container_query: Query<(&mut Node, &CampaignPanelElement)>,
    content_query: Query<(Entity, &CampaignPanelElement), Without<Text>>,
    mut commands: Commands,
) {
    if !panel_state.is_changed() && !campaign_state.is_changed() {
        return;
    }

    // 1. Update Container Displays
    for (mut node, elem) in container_query.iter_mut() {
        match elem {
            CampaignPanelElement::ModalContainer => {
                node.display = if panel_state.is_open {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            CampaignPanelElement::TrackerContainer => {
                node.display = if campaign_state.active_mission.is_some() {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            _ => {}
        }
    }

    // 2. Rebuild Modal Content if open
    if panel_state.is_open {
        for (entity, elem) in content_query.iter() {
            if matches!(elem, CampaignPanelElement::ModalContent) {
                commands.entity(entity).despawn_children();
                commands.entity(entity).with_children(|col| {
                    let missions = [
                        CampaignMissionId::TheStellarForge,
                        CampaignMissionId::GenesisWorld,
                        CampaignMissionId::TheGrandTour,
                        CampaignMissionId::Exodus,
                    ];
                    for m in missions {
                        spawn_mission_card(col, m, campaign_state.active_mission);
                    }
                });
            }
        }
    }

    // 3. Rebuild Live Tracker Content if mission active
    if campaign_state.active_mission.is_some() {
        for (entity, elem) in content_query.iter() {
            if matches!(elem, CampaignPanelElement::TrackerContent) {
                commands.entity(entity).despawn_children();
                commands.entity(entity).with_children(|col| {
                    rebuild_tracker_content(col, &campaign_state, panel_state.is_tracker_minimized);
                });
            }
        }
    }
}
