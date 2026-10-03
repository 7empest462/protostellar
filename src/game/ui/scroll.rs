//! Mouse-wheel scrolling and scroll reset handlers for UI panels.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::simulation::resources::PlayerInteractionState;

use super::types::ScrollableInspector;

/// Handles mouse wheel scrolling for any scrollable UI panel (Inspector, Quick Selector, etc).
pub fn handle_inspector_scroll(
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut scroll_query: Query<(Entity, &mut ScrollPosition, &ComputedNode)>,
    interaction_query: Query<(Entity, &Interaction)>,
    parent_query: Query<&ChildOf>,
) {
    let mut total_delta_y = 0.0;
    for ev in mouse_wheel.read() {
        total_delta_y += match ev.unit {
            MouseScrollUnit::Line => -ev.y * 28.0,
            MouseScrollUnit::Pixel => -ev.y,
        };
    }

    if total_delta_y == 0.0 {
        return;
    }

    for (scroll_entity, mut scroll_position, computed) in scroll_query.iter_mut() {
        if is_inspector_hovered(scroll_entity, &interaction_query, &parent_query) {
            // ComputedNode sizes are in the same logical pixel space. No scale factor needed.
            let max_offset_y = (computed.content_size().y - computed.size().y).max(0.0);
            scroll_position.y = (scroll_position.y + total_delta_y).clamp(0.0, max_offset_y);
        }
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
