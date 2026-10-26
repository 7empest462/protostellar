//! Floating holographic HUD callout tags for 3D planetary cutaways.
//!
//! Projects real-time labels and physical stratum descriptions (Crust, Silicate Mantle,
//! Outer Core, Solid Inner Core, and Theia LLSVPs) onto the screen beside the 3D callout
//! leader lines when internal cutaway mode is active.

use bevy::prelude::*;

use crate::rendering::camera::{IgnoreUiPicking, PanOrbitCamera};
use crate::rendering::effects::planetary::resolve_cutaway_strata;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::AU_TO_KM;

/// Marker for the floating cutaway callout cards.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutawayCalloutPill(pub usize);

/// Field selector enum identifying text components within each callout card.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutawayField {
    Title,
    Subtitle,
}

/// Spawns the 4 glassmorphic floating cutaway callout cards in the UI root.
pub fn spawn_cutaway_hud(root: &mut ChildSpawnerCommands) {
    for i in 0..4 {
        root.spawn((
            CutawayCalloutPill(i),
            IgnoreUiPicking,
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                min_width: Val::Px(155.0),
                max_width: Val::Px(240.0),
                border: UiRect::all(Val::Px(1.2)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                row_gap: Val::Px(1.5),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.05, 0.11, 0.92)),
            BorderColor::all(Color::srgba(0.25, 0.82, 0.98, 0.75)),
        ))
        .with_children(|card| {
            // Title
            card.spawn((
                CutawayField::Title,
                IgnoreUiPicking,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.98, 1.0)),
            ));

            // Subtitle
            card.spawn((
                CutawayField::Subtitle,
                IgnoreUiPicking,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.85, 0.95)),
            ));
        });
    }
}

/// Dynamic system updating floating cutaway callout positions and physical text.
#[allow(
    clippy::type_complexity,
    reason = "Cutaway HUD requires camera projection, celestial components, and UI nodes"
)]
pub fn update_cutaway_hud_ui(
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform, &Transform), With<PanOrbitCamera>>,
    player_state: Res<PlayerInteractionState>,
    config: Res<SimulationConfig>,
    mut pill_query: Query<(Entity, &CutawayCalloutPill, &mut Node, &mut BorderColor)>,
    mut text_query: Query<(&ChildOf, &CutawayField, &mut Text, &mut TextColor)>,
    bodies_query: Query<(
        &SimPosition,
        &Radius,
        &CelestialBody,
        Option<&InternalDifferentiation>,
        Option<&Composition>,
    )>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_gt, cam_tf)) = camera_query.single() else {
        return;
    };

    let is_active = player_state.is_cutaway_active()
        && player_state.overlay_mode != DiagnosticOverlayMode::Hidden;

    let target_entity = if is_active {
        player_state
            .selected_entity
            .or(player_state.hovered_entity)
    } else {
        None
    };

    let target_data = target_entity.and_then(|ent| bodies_query.get(ent).ok());

    let Some((sim_pos, radius, body, opt_diff, _opt_comp)) = target_data else {
        // Hide all 4 callout cards
        for (_, _, mut node, _) in &mut pill_query {
            node.display = Display::None;
        }
        return;
    };
    let body_vec = Vec3::new(sim_pos.x as f32, sim_pos.y as f32, sim_pos.z as f32);
    let strata = resolve_cutaway_strata(body, radius.0, opt_diff, &config);

    // Compute anchor points in 3D world space (matching draw_cutaway_callout_leaders)
    let cam_rot = cam_tf.rotation;
    let right = cam_rot * Vec3::X;
    let up = cam_rot * Vec3::Y;

    let diag_up_right = (right * 0.85 + up * 0.55).normalize();
    let diag_up_left = (-right * 0.85 + up * 0.55).normalize();
    let diag_down_right = (right * 0.85 - up * 0.55).normalize();
    let diag_down_left = (-right * 0.85 - up * 0.55).normalize();

    let offset_scale = strata.crust_r * 1.35;

    let anchors = [
        // 0: Crust (Up-Left)
        (
            body_vec + diag_up_left * (offset_scale * 1.35),
            true, // is_left
            strata.crust_col,
        ),
        // 1: Mantle (Up-Right)
        (
            body_vec + diag_up_right * (offset_scale * 1.25),
            false,
            strata.mantle_col,
        ),
        // 2: Outer Core (Down-Right)
        (
            body_vec + diag_down_right * (offset_scale * 1.05),
            false,
            strata.outer_core_col,
        ),
        // 3: Inner Core (Down-Left)
        (
            body_vec + diag_down_left * (offset_scale * 0.85),
            true,
            strata.inner_core_col,
        ),
    ];

    let labels = resolve_cutaway_labels(body, radius.0, opt_diff);

    for (pill_ent, pill, mut node, mut border_col) in &mut pill_query {
        let idx = pill.0;
        let Some(&(anchor_pos, is_left, stratum_col)) = anchors.get(idx) else {
            node.display = Display::None;
            continue;
        };
        let Some((title, sub)) = labels.get(idx) else {
            node.display = Display::None;
            continue;
        };

        if let Ok(screen_pos) = camera.world_to_viewport(cam_gt, anchor_pos) {
            let card_w = 175.0;
            let card_h = 36.0;

            let target_x = if is_left {
                screen_pos.x - card_w - 10.0
            } else {
                screen_pos.x + 10.0
            };
            let target_y = screen_pos.y - card_h * 0.5;

            // Clamp inside viewport
            let clamped_x = target_x.clamp(10.0, (window.width() - card_w - 10.0).max(10.0));
            let clamped_y = target_y.clamp(10.0, (window.height() - card_h - 10.0).max(10.0));

            node.display = Display::Flex;
            node.left = Val::Px(clamped_x);
            node.top = Val::Px(clamped_y);
            *border_col = BorderColor::all(stratum_col);

            // Sync texts
            for (child_of, field, mut text, mut text_col) in &mut text_query {
                if child_of.parent() == pill_ent {
                    match field {
                        CutawayField::Title => {
                            text.0.clone_from(title);
                            text_col.0 = stratum_col;
                        }
                        CutawayField::Subtitle => {
                            text.0.clone_from(sub);
                        }
                    }
                }
            }
        } else {
            node.display = Display::None;
        }
    }
}

/// Resolves human-readable scientific layer titles and physical descriptions.
fn resolve_cutaway_labels(
    body: &CelestialBody,
    rad_au: f64,
    opt_diff: Option<&InternalDifferentiation>,
) -> [(String, String); 4] {
    if body.body_type.is_star_or_remnant() {
        [
            (
                "PHOTOSPHERE & CORONA".to_string(),
                "Radiating Surface (5,778 K)".to_string(),
            ),
            (
                "CONVECTIVE ZONE".to_string(),
                "Plasma Granulation & Dynamo".to_string(),
            ),
            (
                "RADIATIVE ZONE".to_string(),
                "Photon Diffusion (~100k yr)".to_string(),
            ),
            (
                "THERMONUCLEAR CORE".to_string(),
                "p-p Fusion (15,000,000 K)".to_string(),
            ),
        ]
    } else if body.body_type == BodyType::GasGiant {
        [
            (
                "ATMOSPHERE & CHROMO".to_string(),
                "H₂-He Cloud Decks & Jets".to_string(),
            ),
            (
                "MOLECULAR HYDROGEN".to_string(),
                "Supercritical Fluid Envelope".to_string(),
            ),
            (
                "METALLIC H⁺ DYNAMO".to_string(),
                "Liquid Metallic Hydrogen (B-Field)".to_string(),
            ),
            (
                "HEAVY ELEMENT CORE".to_string(),
                "Dense Silicate & Iron Seed".to_string(),
            ),
        ]
    } else if body.body_type == BodyType::IceGiant {
        [
            (
                "ATMOSPHERE & HAZE".to_string(),
                "H₂-He-CH₄ Methane Clouds".to_string(),
            ),
            (
                "SUPERCRITICAL ICE MANTLE".to_string(),
                "Ionic Fluid H₂O-NH₃-CH₄ Slush".to_string(),
            ),
            (
                "TRANSITION BOUNDARY".to_string(),
                "High-Pressure Mantle Discontinuity".to_string(),
            ),
            (
                "SILICATE / METAL CORE".to_string(),
                "Dense Earth-Mass Rocky Core".to_string(),
            ),
        ]
    } else {
        // Terrestrial Planet / Moon
        let (_core_km, mantle_km, crust_km, inner_core_km, outer_core_km, has_theia) =
            opt_diff.map_or_else(
                || {
                    let total_km = rad_au * AU_TO_KM;
                    let c = total_km * 0.55;
                    let m = total_km * 0.40;
                    let cr = (total_km * 0.05).max(10.0);
                    let ic = c * 0.38;
                    let oc = c - ic;
                    (c, m, cr, ic, oc, false)
                },
                |d| {
                    let c = d.core_radius_au * AU_TO_KM;
                    let m = (d.mantle_radius_au - d.core_radius_au).max(0.0) * AU_TO_KM;
                    let cr = (rad_au - d.mantle_radius_au).max(0.0) * AU_TO_KM;
                    let ic = c * 0.38;
                    let oc = c - ic;
                    (c, m, cr.max(5.0), ic, oc, d.has_theia_llsvp)
                },
            );

        let mantle_sub = if has_theia {
            "Bridgmanite & Theia LLSVPs".to_string()
        } else {
            "Ductile Bridgmanite / Peridotite".to_string()
        };

        [
            (
                format!("CRUST [{crust_km:.0} km]"),
                "Rigid Lithosphere & Plates".to_string(),
            ),
            (format!("SILICATE MANTLE [{mantle_km:.0} km]"), mantle_sub),
            (
                format!("OUTER CORE [{outer_core_km:.0} km]"),
                "Molten Fe-Ni Dynamo (4,500 K)".to_string(),
            ),
            (
                format!("INNER CORE [{inner_core_km:.0} km]"),
                "Solid Fe-Ni Crystal (5,500 K)".to_string(),
            ),
        ]
    }
}
