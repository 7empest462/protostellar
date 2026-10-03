//! Bottom-right navigation shortcuts and keybindings help panel.

use bevy::prelude::*;

use super::types::*;

pub fn spawn_bottom_right_controls_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            ..default()
        })
        .with_children(|right_col| {
            right_col
                .spawn((
                    Button,
                    UiButtonAction::ToggleBottomRightPanel,
                    HudPanelElement::BottomRightPill,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        display: Display::None,
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
                        Text::new("⌨ Shortcuts [?] ▲"),
                        TextFont {
                            font_size: FontSize::Px(9.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.85, 1.0)),
                        Pickable::IGNORE,
                    ));
                });

            right_col
                .spawn((
                    HudPanelElement::BottomRightPanel,
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(6.0)),
                        max_width: Val::Px(240.0),
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
                            margin: UiRect::bottom(Val::Px(3.0)),
                            ..default()
                        })
                        .with_children(|hdr| {
                            hdr.spawn((
                                Text::new("⌨ SHORTCUTS & HELP"),
                                TextFont {
                                    font_size: FontSize::Px(9.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.4, 0.75, 1.0)),
                            ));
                            create_compact_button(
                                hdr,
                                UiButtonAction::ToggleBottomRightPanel,
                                "🗕",
                                Color::srgba(0.14, 0.08, 0.16, 0.85),
                                Color::srgb(0.9, 0.4, 0.6),
                            );
                        });

                    panel.spawn((
                        Text::new("⌨ NAVIGATION & SHORTCUTS:\n[R-Drag] 360 Orbit | [WASD] Pan | [Scroll] Zoom\n[Click / Tab] Select | [F] Track | [Esc] Deselect\n[Space] Pause | [1..8] Speed | [F10] Telemetry | [F11] Epochs"),
                        TextFont {
                            font_size: FontSize::Px(9.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.75, 0.82, 0.95)),
                    ));
                });
        });
}
