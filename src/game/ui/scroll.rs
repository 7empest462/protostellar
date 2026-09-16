//! Mouse-wheel scrolling and scroll reset handlers for UI panels.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::simulation::resources::PlayerInteractionState;

use super::types::ScrollableInspector;

/// Handles mouse wheel scrolling for the bottom-left Target Inspector panel.
pub fn handle_inspector_scroll(
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut scroll_query: Query<(Entity, &mut ScrollPosition, &Node, &ComputedNode), With<ScrollableInspector>>,
    interaction_query: Query<(Entity, &Interaction)>,
    parent_query: Query<&ChildOf>,
) {
    let Ok((scroll_entity, mut scroll_position, _node, computed)) = scroll_query.single_mut() else {
        return;
    };

    let is_hovered = is_inspector_hovered(scroll_entity, &interaction_query, &parent_query);
    if !is_hovered {
        return;
    }

    let max_offset_y = ((computed.content_size().y - computed.size().y) * computed.inverse_scale_factor).max(0.0);

    for ev in mouse_wheel.read() {
        let delta_y = match ev.unit {
            MouseScrollUnit::Line => -ev.y * 28.0,
            MouseScrollUnit::Pixel => -ev.y,
        };

        scroll_position.y = (scroll_position.y + delta_y).clamp(0.0, max_offset_y);
    }
}

/// Resets the inspector vertical scroll offset back to top when switching celestial body selection.
pub fn reset_inspector_scroll_on_target_change(
    player_state: Res<PlayerInteractionState>,
    mut last_target: Local<Option<Entity>>,
    mut scroll_query: Query<&mut ScrollPosition, With<ScrollableInspector>>,
) {
    if player_state.selected_entity != *last_target {
        *last_target = player_state.selected_entity;
        for mut scroll_pos in &mut scroll_query {
            scroll_pos.y = 0.0;
        }
    }
}

fn is_inspector_hovered(
    target_scroll_entity: Entity,
    interaction_query: &Query<(Entity, &Interaction)>,
    parent_query: &Query<&ChildOf>,
) -> bool {
    for (entity, interaction) in interaction_query {
        if *interaction != Interaction::Hovered && *interaction != Interaction::Pressed {
            continue;
        }

        let mut current = entity;
        loop {
            if current == target_scroll_entity {
                return true;
            }
            if let Ok(child_of) = parent_query.get(current) {
                current = child_of.parent();
            } else {
                break;
            }
        }
    }
    false
}
