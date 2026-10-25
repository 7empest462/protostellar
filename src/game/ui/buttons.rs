//! Helper functions for spawning styled sci-fi telemetry buttons and toolbar widgets.

use bevy::prelude::*;

use super::types::UiButtonAction;

/// Stores the resting colors of a Quick Bar button so it can be dynamically highlighted without recreating the UI.
#[derive(Component, Debug, Clone, Copy)]
pub struct QuickBarButtonBaseColor {
    pub bg: Color,
    pub border: Color,
}

/// Helper function to create sci-fi telemetry button style (left accent bar + asymmetric bevel)
pub fn create_button(
    parent: &mut ChildSpawnerCommands,
    action: UiButtonAction,
    label: &str,
    bg_color: Color,
    border_color: Color,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                padding: UiRect::new(Val::Px(9.0), Val::Px(8.0), Val::Px(4.5), Val::Px(4.5)),
                margin: UiRect::all(Val::Px(2.0)),
                border: UiRect::new(Val::Px(2.5), Val::Px(0.5), Val::Px(0.5), Val::Px(0.5)),
                border_radius: BorderRadius {
                    top_left: Val::Px(1.5),
                    top_right: Val::Px(7.0),
                    bottom_right: Val::Px(1.5),
                    bottom_left: Val::Px(7.0),
                },
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(border_color.with_alpha(0.85)),
            BackgroundColor(bg_color.with_alpha(0.82)),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.97, 1.0)),
                Pickable::IGNORE,
            ));
        });
}

/// Helper function to create compact sci-fi selectable button with base color memory for dense toolbars
pub fn create_compact_selectable_button(
    parent: &mut ChildSpawnerCommands,
    action: UiButtonAction,
    label: &str,
    bg_color: Color,
    border_color: Color,
    base_bg: Color,
    base_border: Color,
) {
    parent
        .spawn((
            Button,
            action,
            QuickBarButtonBaseColor {
                bg: base_bg,
                border: base_border,
            },
            Node {
                padding: UiRect::new(Val::Px(7.0), Val::Px(6.0), Val::Px(2.5), Val::Px(2.5)),
                margin: UiRect::all(Val::Px(1.5)),
                border: UiRect::new(Val::Px(2.5), Val::Px(0.5), Val::Px(0.5), Val::Px(0.5)),
                border_radius: BorderRadius {
                    top_left: Val::Px(1.5),
                    top_right: Val::Px(6.0),
                    bottom_right: Val::Px(1.5),
                    bottom_left: Val::Px(6.0),
                },
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(border_color.with_alpha(0.88)),
            BackgroundColor(bg_color.with_alpha(0.80)),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.97, 1.0)),
                Pickable::IGNORE,
            ));
        });
}

/// Helper function to create compact sci-fi button for dense toolbars
pub fn create_compact_button(
    parent: &mut ChildSpawnerCommands,
    action: UiButtonAction,
    label: &str,
    bg_color: Color,
    border_color: Color,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                padding: UiRect::new(Val::Px(7.0), Val::Px(6.0), Val::Px(2.5), Val::Px(2.5)),
                margin: UiRect::all(Val::Px(1.5)),
                border: UiRect::new(Val::Px(2.5), Val::Px(0.5), Val::Px(0.5), Val::Px(0.5)),
                border_radius: BorderRadius {
                    top_left: Val::Px(1.5),
                    top_right: Val::Px(6.0),
                    bottom_right: Val::Px(1.5),
                    bottom_left: Val::Px(6.0),
                },
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(border_color.with_alpha(0.85)),
            BackgroundColor(bg_color.with_alpha(0.78)),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.97, 1.0)),
                Pickable::IGNORE,
            ));
        });
}
