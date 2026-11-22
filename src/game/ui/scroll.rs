//! Mouse-wheel scrolling and scroll reset handlers for UI panels.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::simulation::resources::PlayerInteractionState;

use super::types::{QuickBarScrollViewport, QuickBarState, ScrollableInspector};

fn compute_scroll_hover(
    scroll_entity: Entity,
    computed: &ComputedNode,
    opt_ui_gt: Option<&bevy::ui::UiGlobalTransform>,
    cursor_pos: Option<Vec2>,
    interaction_query: &Query<(
        Entity,
        &bevy::picking::hover::Hovered,
        Has<bevy::ui::Pressed>,
    )>,
    parent_query: &Query<&ChildOf>,
) -> bool {
    let over_rect = cursor_pos.zip(opt_ui_gt).is_some_and(|(pos, ui_gt)| {
        let inv = computed.inverse_scale_factor();
        let half = computed.size() * inv * 0.5;
        let center = ui_gt.translation * inv;
        pos.x >= center.x - half.x
            && pos.x <= center.x + half.x
            && pos.y >= center.y - half.y
            && pos.y <= center.y + half.y
    });
    over_rect || is_inspector_hovered(scroll_entity, interaction_query, parent_query)
}

/// Handles mouse wheel scrolling and chevron nudges for scrollable vertical/horizontal UI containers.
#[allow(clippy::too_many_arguments, reason = "UI scroll container handler")]
pub fn handle_inspector_scroll(
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut quick_bar_state: Option<ResMut<QuickBarState>>,
    mut scroll_query: Query<(
        Entity,
        &mut ScrollPosition,
        &ComputedNode,
        Option<&bevy::ui::UiGlobalTransform>,
        Option<&Node>,
        Has<QuickBarScrollViewport>,
    )>,
    interaction_query: Query<(
        Entity,
        &bevy::picking::hover::Hovered,
        Has<bevy::ui::Pressed>,
    )>,
    parent_query: Query<&ChildOf>,
    windows: Query<&Window>,
) {
    let mut total_delta_x = 0.0;
    let mut total_delta_y = 0.0;
    for ev in mouse_wheel.read() {
        let (dx, dy) = match ev.unit {
            MouseScrollUnit::Line => (-ev.x * 28.0, -ev.y * 28.0),
            MouseScrollUnit::Pixel => (-ev.x, -ev.y),
        };
        total_delta_x += dx;
        total_delta_y += dy;
    }

    let pending_qb_x = quick_bar_state
        .as_deref()
        .map_or(0.0, |q| q.pending_scroll_delta_x);
    if pending_qb_x != 0.0
        && let Some(ref mut qb) = quick_bar_state
    {
        qb.pending_scroll_delta_x = 0.0;
    }

    if total_delta_x == 0.0 && total_delta_y == 0.0 && pending_qb_x == 0.0 {
        return;
    }

    let cursor_pos = windows.single().ok().and_then(Window::cursor_position);

    for (scroll_entity, mut scroll_position, computed, opt_ui_gt, opt_node, is_quick_bar_vp) in
        scroll_query.iter_mut()
    {
        if opt_node.is_some_and(|n| n.display == Display::None) {
            continue;
        }
        let inv = computed.inverse_scale_factor();
        let max_offset_x = (computed.content_size().x - computed.size().x).max(0.0) * inv;
        let max_offset_y = (computed.content_size().y - computed.size().y).max(0.0) * inv;

        if is_quick_bar_vp && pending_qb_x != 0.0 && max_offset_x > 0.5 {
            scroll_position.x = (scroll_position.x + pending_qb_x).clamp(0.0, max_offset_x);
        }

        if total_delta_x == 0.0 && total_delta_y == 0.0 {
            continue;
        }
        if !compute_scroll_hover(
            scroll_entity,
            computed,
            opt_ui_gt,
            cursor_pos,
            &interaction_query,
            &parent_query,
        ) {
            continue;
        }

        let allows_x = opt_node.is_some_and(|n| n.overflow.x == OverflowAxis::Scroll);
        let allows_y = if is_quick_bar_vp {
            opt_node.is_some_and(|n| n.overflow.y == OverflowAxis::Scroll)
        } else {
            !allows_x || opt_node.is_some_and(|n| n.overflow.y == OverflowAxis::Scroll)
        };
        if max_offset_y > 0.5 && allows_y && total_delta_y != 0.0 {
            scroll_position.y = (scroll_position.y + total_delta_y).clamp(0.0, max_offset_y);
        } else if max_offset_x > 0.5 && allows_x {
            let delta = if total_delta_x.abs() > 0.01 {
                total_delta_x
            } else {
                total_delta_y
            };
            scroll_position.x = (scroll_position.x + delta).clamp(0.0, max_offset_x);
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
    interaction_query: &Query<(
        Entity,
        &bevy::picking::hover::Hovered,
        Has<bevy::ui::Pressed>,
    )>,
    parent_query: &Query<&ChildOf>,
) -> bool {
    for (entity, hovered, pressed) in interaction_query {
        if !hovered.get() && !pressed {
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
