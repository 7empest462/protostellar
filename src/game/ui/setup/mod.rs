//! Setup system and layout builders for the Heads-Up Display (HUD).

pub mod controls;
pub mod docks;

pub(crate) use controls::*;
pub(crate) use docks::*;

use bevy::prelude::*;

use crate::rendering::camera::IgnoreUiPicking;

use super::types::*;
use super::widgets::*;

fn spawn_top_left_stats_panel(top_row: &mut ChildSpawnerCommands) {
    top_row
        .spawn((
            IgnoreUiPicking,
            Pickable::IGNORE,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                ..default()
            },
        ))
        .with_children(|left_col| {
            left_col
                .spawn((
                    HudPanelElement::TopLeftPanel,
                    Node {
                        display: Display::None,
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(8.0)),
                        width: Val::Px(280.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.88)),
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
                                Text::new("🌌 PROTOSTELLAR // Telemetry"),
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
                        Text::new("Initializing Nebula..."),
                        TextFont {
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.95, 1.0)),
                        HudHeaderStatsText,
                    ));
                });

            left_col
                .spawn((
                    HudButton,
                    UiButtonAction::ToggleTopLeftPanel,
                    HudPanelElement::TopLeftPill,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        display: Display::Flex,
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
                        Text::new("🌌 PROTOSTELLAR  |  📊 Stats ▼"),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.85, 1.0)),
                        Pickable::IGNORE,
                    ));
                });

            super::event_feed::spawn_event_feed_panel(left_col);
        });
}

fn spawn_top_center_quick_bar(top_row: &mut ChildSpawnerCommands) {
    top_row
        .spawn((
            IgnoreUiPicking,
            Pickable::IGNORE,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|center_col| {
            center_col
                .spawn((
                    HudPanelElement::QuickBarContainer,
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Vw(56.0),
                        max_width: Val::Px(740.0),
                        min_width: Val::Px(380.0),
                        margin: UiRect::bottom(Val::Px(3.0)),
                        padding: UiRect::all(Val::Px(2.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.85)),
                    BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.6)),
                ))
                .with_children(|bar_card| {
                    bar_card
                        .spawn((
                            IgnoreUiPicking,
                            Pickable::IGNORE,
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                width: Val::Percent(100.0),
                                column_gap: Val::Px(2.0),
                                ..default()
                            },
                        ))
                        .with_children(|strip_row| {
                            create_compact_button(
                                strip_row,
                                UiButtonAction::ScrollQuickBarLeft,
                                "◀",
                                Color::srgba(0.05, 0.11, 0.22, 0.88),
                                Color::srgb(0.35, 0.72, 1.0),
                            );
                            strip_row
                                .spawn((
                                    QuickBarScrollViewport,
                                    ScrollPosition::default(),
                                    IgnoreUiPicking,
                                    Pickable::IGNORE,
                                    Node {
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        flex_grow: 1.0,
                                        flex_shrink: 1.0,
                                        min_width: Val::Px(0.0),
                                        overflow: Overflow::scroll_x(),
                                        ..default()
                                    },
                                ))
                                .with_children(|vp| {
                                    vp.spawn((
                                        QuickBodySelectorBar,
                                        IgnoreUiPicking,
                                        Pickable::IGNORE,
                                        Node {
                                            flex_direction: FlexDirection::Row,
                                            padding: UiRect::axes(Val::Px(2.0), Val::Px(1.5)),
                                            align_items: AlignItems::Center,
                                            flex_wrap: FlexWrap::NoWrap,
                                            ..default()
                                        },
                                    ));
                                });
                            create_compact_button(
                                strip_row,
                                UiButtonAction::ScrollQuickBarRight,
                                "▶",
                                Color::srgba(0.05, 0.11, 0.22, 0.88),
                                Color::srgb(0.35, 0.72, 1.0),
                            );
                        });

                    spawn_quick_bar_filter_row(bar_card);
                });
        });
}

fn spawn_quick_bar_filter_row(bar_card: &mut ChildSpawnerCommands) {
    bar_card
        .spawn((
            HudPanelElement::QuickBarFilterRow,
            IgnoreUiPicking,
            Pickable::IGNORE,
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            Name::new("QuickBarFilterRow"),
        ))
        .with_children(|filter_row| {
            filter_row.spawn((
                Text::new("🔍"),
                TextFont {
                    font: hud_font_source(),
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.85, 1.0)),
                Pickable::IGNORE,
            ));
            spawn_hud_text_input(
                filter_row,
                HudTextInputKind::QuickBarFilter,
                "",
                140.0,
                10.0,
            );
            filter_row
                .spawn((
                    HudButton,
                    UiButtonAction::ClearQuickBarFilter,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.5)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.16, 0.28, 0.85)),
                    BorderColor::all(Color::srgb(0.25, 0.5, 0.75)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("✕"),
                        TextFont {
                            font: hud_font_source(),
                            font_size: FontSize::Px(9.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.7, 0.85, 1.0)),
                        Pickable::IGNORE,
                    ));
                });
        });
}

pub fn setup_hud(mut commands: Commands) {
    commands.init_resource::<NotificationToast>();
    commands.init_resource::<HudVisibilityState>();

    commands
        .spawn((
            HudPanelElement::RootContainer,
            IgnoreUiPicking,
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            // TOP LEFT: Telemetry & Stats
            root.spawn((
                IgnoreUiPicking,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    left: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|wrapper| {
                spawn_top_left_stats_panel(wrapper);
            });

            // TOP CENTER: Quick Selector Bar & Scenarios
            root.spawn((
                IgnoreUiPicking,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::FlexStart,
                    ..default()
                },
            ))
            .with_children(|wrapper| {
                spawn_top_center_quick_bar(wrapper);
            });

            // TOP RIGHT: Simulation Metrics
            root.spawn((
                IgnoreUiPicking,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    right: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|wrapper| {
                spawn_simulation_metrics_column(wrapper);
            });

            // MIDDLE RIGHT: Controls & Badges
            root.spawn((
                IgnoreUiPicking,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Vh(40.0), // Safely below top right
                    right: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|wrapper| {
                spawn_top_controls_card(wrapper);
            });

            // BOTTOM LEFT: Target Inspector
            root.spawn((
                IgnoreUiPicking,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(10.0),
                    left: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|wrapper| {
                super::inspector_panel::spawn_bottom_left_inspector_panel(wrapper);
            });

            // BOTTOM CENTER: Toast & Playbar
            root.spawn((
                IgnoreUiPicking,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(10.0),
                    left: Val::Percent(45.0), // Shifted left per user request
                    margin: UiRect::left(Val::Px(-260.0)), // Center offset for 520px width
                    ..default()
                },
            ))
            .with_children(|wrapper| {
                spawn_bottom_center_toast_panel(wrapper);
            });

            // BOTTOM RIGHT: Navigation Help
            root.spawn((
                IgnoreUiPicking,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(10.0),
                    right: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|wrapper| {
                super::navigation_panel::spawn_bottom_right_controls_panel(wrapper);
            });

            spawn_hud_modal_drawers(root);
        });

    spawn_cinema_restore_pill(&mut commands);
}

fn spawn_hud_modal_drawers(root: &mut ChildSpawnerCommands) {
    super::presets_panel::spawn_scenario_presets_panel(root);
    super::saves_panel::spawn_saves_panel(root);
    super::builder_panel::spawn_planet_builder_panel(root);
    super::telemetry_panel::spawn_telemetry_panel(root);
    super::trophies_panel::spawn_trophies_panel(root);
    super::tech_tree_panel::spawn_tech_tree_panel(root);
    super::campaign_panel::spawn_campaign_panels(root);
    super::hazards_panel::spawn_hazards_panel(root);
    super::crisis_banner::spawn_crisis_alert_banner(root);
    super::epoch_scrubber::spawn_epoch_scrubber_panel(root);
    super::hr_diagram_panel::spawn_hr_diagram_panel(root);
    super::hover_card::spawn_hover_card(root);
    super::cutaway_hud::spawn_cutaway_hud(root);
}

fn spawn_cinema_restore_pill(commands: &mut Commands) {
    // Floating restore pill outside RootContainer so Cinema Mode remains one-click reversible
    commands
        .spawn((
            HudButton,
            UiButtonAction::ToggleFullScreenHud,
            HudPanelElement::CinemaRestorePill,
            ZIndex(90),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                display: Display::None,
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.50)),
            BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.45)),
        ))
        .with_children(|pill| {
            pill.spawn((
                Text::new("👁️ Show HUD [` / Esc]"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgba(0.75, 0.92, 1.0, 0.85)),
                Pickable::IGNORE,
            ));
        });
}
