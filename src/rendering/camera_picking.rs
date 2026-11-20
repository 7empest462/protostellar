//! UI hit-testing, scroll-container zoom isolation, and 3D celestial body picking helpers.

use bevy::prelude::*;

use crate::rendering::camera::{CameraTargetsQuery, PanOrbitCamera, resolve_camera_body_radii};
use crate::simulation::resources::*;

/// Marker component for UI elements (such as floating holographic hover cards) that should not
/// block 3D raycasting, camera drag controls, or entity hover targeting.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct IgnoreUiPicking;

pub(crate) type UiPickingNodesQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedNode,
        Option<&'static bevy::ui::UiGlobalTransform>,
        Option<&'static GlobalTransform>,
        Option<&'static Node>,
        Option<&'static InheritedVisibility>,
        Option<&'static ScrollPosition>,
    ),
    (With<Node>, Without<IgnoreUiPicking>),
>;

pub(crate) type UiInteractionStateQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static bevy::picking::hover::Hovered,
        Has<bevy::ui::Pressed>,
        Option<&'static Node>,
        Option<&'static ScrollPosition>,
        Option<&'static ComputedNode>,
    ),
>;

#[inline]
pub(crate) fn is_ui_interaction_blocking(
    hovered: &bevy::picking::hover::Hovered,
    pressed: bool,
    opt_node: Option<&Node>,
    opt_scroll: Option<&ScrollPosition>,
    opt_computed: Option<&ComputedNode>,
) -> bool {
    if !hovered.get() && !pressed {
        return false;
    }
    if opt_node.is_some_and(|n| n.display == Display::None) {
        return false;
    }
    // Non-overflowing scroll wrappers should not hijack camera zoom unless actively pressed.
    if !pressed
        && opt_scroll.is_some()
        && opt_computed.is_some_and(|c| c.size().x > 1.0 && c.content_size().y <= c.size().y + 0.5)
    {
        return false;
    }
    true
}

#[inline]
fn is_cursor_inside_ui_node(
    cursor_pos: Vec2,
    cnode: &ComputedNode,
    opt_ui_gt: Option<&bevy::ui::UiGlobalTransform>,
    opt_gt: Option<&GlobalTransform>,
    opt_node: Option<&Node>,
    opt_ivis: Option<&InheritedVisibility>,
) -> bool {
    if opt_node.is_some_and(|n| n.display == Display::None) {
        return false;
    }
    if opt_ivis.is_some_and(|v| !v.get()) {
        return false;
    }
    let inv = cnode.inverse_scale_factor();
    let size = cnode.size() * inv;
    if size.x <= 1.0 || size.y <= 1.0 {
        return false;
    }
    let center = if let Some(ui_gt) = opt_ui_gt {
        ui_gt.translation * inv
    } else if let Some(gt) = opt_gt {
        gt.translation().truncate() * inv
    } else {
        return false;
    };
    let half = size * 0.5;
    cursor_pos.x >= center.x - half.x
        && cursor_pos.x <= center.x + half.x
        && cursor_pos.y >= center.y - half.y
        && cursor_pos.y <= center.y + half.y
}

pub(crate) fn is_cursor_over_scroll_container(
    cursor_pos: Option<Vec2>,
    ui_nodes_query: &UiPickingNodesQuery,
) -> bool {
    let Some(cursor_pos) = cursor_pos else {
        return false;
    };
    ui_nodes_query.iter().any(
        |(cnode, opt_ui_gt, opt_gt, opt_node, opt_ivis, opt_scroll)| {
            opt_scroll.is_some()
                && cnode.content_size().y > cnode.size().y + 0.5
                && is_cursor_inside_ui_node(
                    cursor_pos, cnode, opt_ui_gt, opt_gt, opt_node, opt_ivis,
                )
        },
    )
}

pub(crate) fn is_cursor_over_active_ui(
    window: &Window,
    cursor_pos: Vec2,
    ui_interaction_query: &UiInteractionStateQuery,
    ui_nodes_query: &UiPickingNodesQuery,
    _opt_visibility: Option<&crate::game::ui::types::HudVisibilityState>,
) -> bool {
    let full_w = window.width() * 0.95;
    let full_h = window.height() * 0.95;
    ui_interaction_query
        .iter()
        .any(|(h, p, n, s, c)| is_ui_interaction_blocking(h, p, n, s, c))
        || ui_nodes_query
            .iter()
            .any(|(cnode, opt_ui_gt, opt_gt, opt_node, opt_ivis, _)| {
                let inv = cnode.inverse_scale_factor();
                let size = cnode.size() * inv;
                if size.x >= full_w && size.y >= full_h {
                    return false;
                }
                is_cursor_inside_ui_node(cursor_pos, cnode, opt_ui_gt, opt_gt, opt_node, opt_ivis)
            })
}

#[allow(
    clippy::too_many_arguments,
    reason = "3D raycast picking and screen-space fallback helper"
)]
pub(crate) fn handle_camera_target_picking(
    window: &Window,
    camera_comp: &Camera,
    global_transform: &GlobalTransform,
    targets_query: &CameraTargetsQuery,
    player_state: &mut PlayerInteractionState,
    camera: &mut PanOrbitCamera,
    config: &SimulationConfig,
    ui_interaction_query: &UiInteractionStateQuery,
    ui_nodes_query: &UiPickingNodesQuery,
    mouse_buttons: &ButtonInput<MouseButton>,
    star_pos: Vec3,
    min_orbit_r: f32,
    opt_visibility: Option<&crate::game::ui::types::HudVisibilityState>,
) {
    if player_state.just_selected_via_ui {
        player_state.just_selected_via_ui = false;
        player_state.hovered_entity = None;
        return;
    }

    if mouse_buttons.pressed(MouseButton::Right) || mouse_buttons.pressed(MouseButton::Middle) {
        player_state.hovered_entity = None;
    }

    let Some(cursor_pos) = window.cursor_position() else {
        player_state.hovered_entity = None;
        return;
    };

    if is_cursor_over_active_ui(
        window,
        cursor_pos,
        ui_interaction_query,
        ui_nodes_query,
        opt_visibility,
    ) {
        player_state.hovered_entity = None;
        return;
    }

    let mut best_target: Option<Entity> = None;
    let mut best_score = f32::MAX;

    if let Ok(ray) = camera_comp.viewport_to_world(global_transform, cursor_pos) {
        let show_atmospheres = player_state.show_atmospheres();
        for (entity, pos, rad, body, mass, opt_t, opt_comp, opt_diff) in targets_query.iter() {
            let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            let (vis_r, _) = resolve_camera_body_radii(
                opt_t,
                rad.0,
                body.body_type,
                mass.0,
                opt_comp,
                opt_diff,
                show_atmospheres,
                (center - star_pos).length(),
                min_orbit_r,
                config,
            );
            let hit_radius = (vis_r * 4.0).clamp(0.015, 3.0);
            let to_center = center - ray.origin;
            let proj = to_center.dot(*ray.direction);
            if proj > 0.0 {
                let perp_dist = (to_center - *ray.direction * proj).length();
                let score = perp_dist / hit_radius;
                if perp_dist < hit_radius && score < best_score {
                    best_score = score;
                    best_target = Some(entity);
                }
            }
        }
    }

    if best_target.is_none() {
        let mut min_screen_dist = 42.0f32;
        for (entity, pos, _rad, _body, _mass, _opt_t, _, _) in targets_query.iter() {
            let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            if let Ok(screen_pos) = camera_comp.world_to_viewport(global_transform, center) {
                let dist = cursor_pos.distance(screen_pos);
                if dist < min_screen_dist {
                    min_screen_dist = dist;
                    best_target = Some(entity);
                }
            }
        }
    }

    player_state.hovered_entity = best_target;

    if !mouse_buttons.just_pressed(MouseButton::Left)
        || player_state.active_tool != PlayerTool::Inspect
    {
        return;
    }

    if let Some(hit_entity) = best_target.or(player_state.hovered_entity) {
        player_state.selected_entity = Some(hit_entity);
        if camera.target_entity != Some(hit_entity) {
            if let Ok((_, pos, radius, body, mass, _opt_t, opt_comp, opt_diff)) =
                targets_query.get(hit_entity)
            {
                let target_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
                let frame_radius = if body.name.starts_with("🚀") || radius.0 <= 1e-6 {
                    let visual_radius =
                        config.calc_visual_radius_for_type(radius.0, body.body_type);
                    0.015f32.max(config.calc_camera_framing_radius(visual_radius))
                } else {
                    crate::rendering::bodies::transforms::compute_close_view_camera_framing_radius(
                        radius.0,
                        body.body_type,
                        mass.0,
                        opt_comp,
                        opt_diff,
                        player_state.show_atmospheres(),
                        (target_vec - star_pos).length(),
                        min_orbit_r,
                        config,
                    )
                };
                camera.initiate_target_transition(hit_entity, target_vec, frame_radius);
            } else {
                camera.target_entity = Some(hit_entity);
            }
        }
    }
}
