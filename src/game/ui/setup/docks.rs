//! Bottom center toast banner and simulation time controls dock.

use bevy::prelude::*;

use crate::game::ui::types::*;
use crate::game::ui::SurfaceCompassNeedle;

pub(crate) fn spawn_bottom_center_toast_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            margin: UiRect::horizontal(Val::Px(6.0)),
            max_width: Val::Px(520.0),
            ..default()
        })
        .with_children(|center_dock| {
            spawn_toast_banner(center_dock);
            spawn_bottom_center_pill(center_dock);
            spawn_time_controls_dock(center_dock);
        });
}

fn spawn_toast_banner(center_dock: &mut ChildSpawnerCommands) {
    center_dock
        .spawn((
            HudToastContainer,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(3.5)),
                margin: UiRect::bottom(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                max_width: Val::Px(520.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.04, 0.09, 0.90)),
            BorderColor::all(Color::srgba(0.25, 0.60, 0.95, 0.65)),
        ))
        .with_children(|toast_box| {
            toast_box.spawn((
                Text::new(">> PROTOSTELLAR LIVE"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.9, 1.0)),
                HudToastText,
            ));
        });
}

fn spawn_bottom_center_pill(center_dock: &mut ChildSpawnerCommands) {
    center_dock
        .spawn((
            HudButton,
            UiButtonAction::ToggleBottomCenterPanel,
            HudPanelElement::BottomCenterPill,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                display: Display::Flex,
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.035, 0.075, 0.88)),
            BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.6)),
        ))
        .with_children(|pill| {
            pill.spawn((
                Text::new("⏱️ 0y 00m 00d | 00h 00m 00s  [Time Controls] ▲"),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.88, 0.45)),
                HudBottomTimerText(true),
                Pickable::IGNORE,
            ));
            pill.spawn((
                SurfaceCompassNeedle,
                UiTransform::default(),
                Node {
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    margin: UiRect::left(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.4, 0.85, 1.0, 0.75)),
                BackgroundColor(Color::srgba(0.1, 0.4, 0.8, 0.4)),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_time_controls_dock(center_dock: &mut ChildSpawnerCommands) {
    center_dock
        .spawn((
            HudPanelElement::BottomCenterPanel,
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                min_width: Val::Px(320.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.035, 0.075, 0.92)),
            BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.7)),
        ))
        .with_children(|time_box| {
            time_box
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new("⏱️ CLOCK: 0y 00m 00d | 00h 00m 00s | PAUSED"),
                        TextFont {
                            font_size: FontSize::Px(12.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.92, 0.4)),
                        HudBottomTimerText(false),
                    ));
                    create_compact_button(
                        hdr,
                        UiButtonAction::ToggleBottomCenterPanel,
                        "🗕",
                        Color::srgba(0.14, 0.08, 0.16, 0.85),
                        Color::srgb(0.9, 0.4, 0.6),
                    );
                });

            time_box
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(2.0),
                    margin: UiRect::top(Val::Px(3.0)),
                    ..default()
                })
                .with_children(|speed_row| {
                    let buttons = [
                        (
                            UiButtonAction::TimePause,
                            "Pause [Space]",
                            Color::srgba(0.24, 0.08, 0.08, 0.9),
                            Color::srgb(1.0, 0.4, 0.4),
                        ),
                        (
                            UiButtonAction::TimeSpeedRealtime,
                            "Real [1]",
                            Color::srgba(0.08, 0.20, 0.16, 0.9),
                            Color::srgb(0.4, 1.0, 0.7),
                        ),
                        (
                            UiButtonAction::TimeSpeed1Min,
                            "1m [9]",
                            Color::srgba(0.08, 0.20, 0.22, 0.9),
                            Color::srgb(0.35, 0.95, 0.85),
                        ),
                        (
                            UiButtonAction::TimeSpeed5Min,
                            "5m [0]",
                            Color::srgba(0.08, 0.18, 0.24, 0.9),
                            Color::srgb(0.4, 0.9, 0.95),
                        ),
                        (
                            UiButtonAction::TimeSpeed6Hr,
                            "6h [S-3]",
                            Color::srgba(0.08, 0.18, 0.25, 0.9),
                            Color::srgb(0.42, 0.88, 0.98),
                        ),
                        (
                            UiButtonAction::TimeSpeed12Hr,
                            "12h [S-4]",
                            Color::srgba(0.08, 0.17, 0.25, 0.9),
                            Color::srgb(0.44, 0.86, 0.98),
                        ),
                        (
                            UiButtonAction::TimeSpeed24Hr,
                            "24h [S-5]",
                            Color::srgba(0.08, 0.17, 0.26, 0.9),
                            Color::srgb(0.46, 0.84, 1.0),
                        ),
                        (
                            UiButtonAction::TimeSpeed72Hr,
                            "72h [S-6]",
                            Color::srgba(0.08, 0.16, 0.26, 0.9),
                            Color::srgb(0.48, 0.82, 1.0),
                        ),
                        (
                            UiButtonAction::TimeSpeed1,
                            "11d [2]",
                            Color::srgba(0.08, 0.16, 0.24, 0.9),
                            Color::srgb(0.4, 0.8, 1.0),
                        ),
                        (
                            UiButtonAction::TimeSpeed100,
                            "100x [4]",
                            Color::srgba(0.08, 0.16, 0.24, 0.9),
                            Color::srgb(0.4, 0.8, 1.0),
                        ),
                        (
                            UiButtonAction::TimeSpeed10k,
                            "10k [6]",
                            Color::srgba(0.12, 0.18, 0.28, 0.9),
                            Color::srgb(0.5, 0.85, 1.0),
                        ),
                        (
                            UiButtonAction::TimeSpeed1M,
                            "1M [8]",
                            Color::srgba(0.18, 0.14, 0.32, 0.9),
                            Color::srgb(0.7, 0.6, 1.0),
                        ),
                    ];
                    for (action, label, bg, fg) in buttons {
                        create_compact_button(speed_row, action, label, bg, fg);
                    }
                });
        });
}
