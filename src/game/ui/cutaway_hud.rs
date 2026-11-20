//! Floating holographic HUD callout tags for 3D planetary cutaways.
//!
//! Projects real-time labels and physical stratum descriptions (Crust, Silicate Mantle,
//! Outer Core, Solid Inner Core, and Theia LLSVPs) onto the screen beside the 3D callout
//! leader lines when internal cutaway mode is active.

use bevy::prelude::*;

use crate::rendering::camera::{IgnoreUiPicking, PanOrbitCamera};
use crate::rendering::effects::planetary::CutawayStrata;
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
            Pickable::IGNORE,
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
            GlobalZIndex(30),
        ))
        .with_children(|card| {
            // Title
            card.spawn((
                CutawayField::Title,
                IgnoreUiPicking,
                Pickable::IGNORE,
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
                Pickable::IGNORE,
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

/// Projects a 3D world space position into 2D viewport screen coordinates.
/// Falls back to analytical perspective projection if camera viewport rect is not yet initialized.
fn project_world_to_viewport(
    camera: &Camera,
    cam_gt: &GlobalTransform,
    pos: Vec3,
    win_w: f32,
    win_h: f32,
) -> Option<Vec2> {
    if let Ok(sp) = camera.world_to_viewport(cam_gt, pos) {
        return Some(sp);
    }
    if win_w <= 0.0 || win_h <= 0.0 {
        return None;
    }
    let cam_mat = cam_gt.to_matrix();
    let view_pos = cam_mat.inverse().transform_point3(pos);
    // In camera space (right-handed), the camera looks along -Z
    if view_pos.z >= -1e-4 {
        return None; // Behind camera
    }
    let aspect = win_w / win_h;
    let (sin_fov, cos_fov) = (0.5 * std::f32::consts::FRAC_PI_4).sin_cos();
    let h = cos_fov / sin_fov;
    let w = h / aspect;
    let proj = Mat4::from_cols(
        Vec4::new(w, 0.0, 0.0, 0.0),
        Vec4::new(0.0, h, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, -1.0),
        Vec4::new(0.0, 0.0, 0.0001, 0.0),
    );
    let clip = proj.project_point3(view_pos);
    if !clip.is_finite() {
        return None;
    }
    let screen_x = (clip.x * 0.5 + 0.5) * win_w;
    let screen_y = (-clip.y * 0.5 + 0.5) * win_h;
    Some(Vec2::new(screen_x, screen_y))
}

/// Resolves which celestial body is currently targeted for 3D cutaway inspection.
fn resolve_active_cutaway_target(
    player_state: &PlayerInteractionState,
    opt_pan: Option<&PanOrbitCamera>,
) -> Option<Entity> {
    let is_active = player_state.is_cutaway_active()
        && (player_state.cutaway_mode
            || player_state.overlay_mode != DiagnosticOverlayMode::Hidden);

    if is_active {
        player_state
            .selected_entity
            .or(player_state.hovered_entity)
            .or_else(|| opt_pan.and_then(|p| p.target_entity))
    } else {
        None
    }
}

/// Computes anchor points in 3D world space matching `draw_cutaway_callout_leaders`.
fn compute_cutaway_anchors(
    body_vec: Vec3,
    cam_rot: Quat,
    strata: &CutawayStrata,
) -> [(Vec3, bool, Color); 4] {
    let right = cam_rot * Vec3::X;
    let up = cam_rot * Vec3::Y;

    let diag_up_right = (right * 0.85 + up * 0.55).normalize();
    let diag_up_left = (-right * 0.85 + up * 0.55).normalize();
    let diag_down_right = (right * 0.85 - up * 0.55).normalize();
    let diag_down_left = (-right * 0.85 - up * 0.55).normalize();

    let offset_scale = strata.crust_r * 1.35;

    [
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
    ]
}

/// Projects and positions a single floating callout pill card node, updating its labels.
#[allow(
    clippy::too_many_arguments,
    reason = "Pill card projection and layout synchronization requires camera, window, node, and text query"
)]
fn sync_cutaway_pill_card(
    pill_ent: Entity,
    idx: usize,
    anchors: &[(Vec3, bool, Color); 4],
    labels: &[(String, String); 4],
    camera: &Camera,
    cam_gt: &GlobalTransform,
    window: &Window,
    node: &mut Mut<'_, Node>,
    border_col: &mut Mut<'_, BorderColor>,
    text_query: &mut Query<(&ChildOf, &CutawayField, &mut Text, &mut TextColor)>,
) {
    let Some(&(anchor_pos, is_left, stratum_col)) = anchors.get(idx) else {
        if node.display != Display::None {
            node.display = Display::None;
        }
        return;
    };
    let Some((title, sub)) = labels.get(idx) else {
        if node.display != Display::None {
            node.display = Display::None;
        }
        return;
    };
    if title.is_empty() {
        if node.display != Display::None {
            node.display = Display::None;
        }
        return;
    }

    if let Some(screen_pos) =
        project_world_to_viewport(camera, cam_gt, anchor_pos, window.width(), window.height())
    {
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

        if node.display != Display::Flex {
            node.display = Display::Flex;
        }
        let target_left = Val::Px(clamped_x);
        if node.left != target_left {
            node.left = target_left;
        }
        let target_top = Val::Px(clamped_y);
        if node.top != target_top {
            node.top = target_top;
        }
        let target_border = BorderColor::all(stratum_col);
        border_col.set_if_neq(target_border);

        // Sync texts
        for (child_of, field, mut text, mut text_col) in text_query.iter_mut() {
            if child_of.parent() == pill_ent {
                match field {
                    CutawayField::Title => {
                        if text.0 != *title {
                            text.0.clone_from(title);
                        }
                        if text_col.0 != stratum_col {
                            text_col.0 = stratum_col;
                        }
                    }
                    CutawayField::Subtitle => {
                        if text.0 != *sub {
                            text.0.clone_from(sub);
                        }
                    }
                }
            }
        }
    } else if node.display != Display::None {
        node.display = Display::None;
    }
}

/// Dynamic system updating floating cutaway callout positions and physical text.
#[allow(
    clippy::type_complexity,
    reason = "Cutaway HUD requires camera projection, celestial components, and UI nodes"
)]
pub fn update_cutaway_hud_ui(
    windows: Query<&Window>,
    camera_query: Query<(
        &Camera,
        &GlobalTransform,
        &Transform,
        Option<&PanOrbitCamera>,
    )>,
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
        Option<&Transform>,
        Option<&Mass>,
        Option<&Temperature>,
        Option<&IgnitionState>,
        Option<&StellarEvolutionState>,
    )>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some((camera, cam_gt, cam_tf, opt_pan)) = camera_query.iter().next() else {
        return;
    };

    let target_entity = resolve_active_cutaway_target(&player_state, opt_pan);
    let target_data = target_entity
        .and_then(|ent| bodies_query.get(ent).ok())
        .filter(|t| t.2.body_type != BodyType::BlackHole);

    let Some((
        sim_pos,
        radius,
        body,
        opt_diff,
        opt_comp,
        opt_tf,
        opt_mass,
        opt_temp,
        opt_ignition,
        opt_evo,
    )) = target_data
    else {
        // Hide all 4 callout cards
        for (_, _, mut node, _) in &mut pill_query {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
        return;
    };

    let body_vec = Vec3::new(sim_pos.x as f32, sim_pos.y as f32, sim_pos.z as f32);
    let cam_dist = cam_tf.translation.distance(body_vec);
    let show_atmospheres = player_state.show_atmospheres();
    let (eff_rad, eff_type) = if let (Some(comp), Some(mass)) = (opt_comp, opt_mass) {
        crate::rendering::bodies::view_overrides::resolve_view_body_radius_and_type(
            mass.0,
            radius.0,
            body.body_type,
            comp,
            opt_diff,
            show_atmospheres,
        )
    } else {
        (radius.0, body.body_type)
    };
    let mut eff_body = body.clone();
    eff_body.body_type = eff_type;
    let visual_r = opt_tf.map_or_else(
        || {
            crate::rendering::bodies::transforms::compute_view_visual_radius(
                radius.0,
                body.body_type,
                opt_mass.map_or(0.0, |m| m.0),
                opt_comp,
                opt_diff,
                show_atmospheres,
                body_vec.length(),
                1.0,
                cam_dist,
                &config,
            )
        },
        |tf| tf.scale.x,
    );
    let strata = crate::rendering::effects::planetary::resolve_cutaway_strata_with_visual_radius(
        &eff_body, eff_rad, visual_r, opt_diff,
    );
    let anchors = compute_cutaway_anchors(body_vec, cam_tf.rotation, &strata);
    let labels = resolve_cutaway_labels_with_telemetry(
        &eff_body,
        eff_rad,
        opt_diff,
        opt_temp,
        opt_ignition,
        opt_evo,
    );

    for (pill_ent, pill, mut node, mut border_col) in &mut pill_query {
        sync_cutaway_pill_card(
            pill_ent,
            pill.0,
            &anchors,
            &labels,
            camera,
            cam_gt,
            window,
            &mut node,
            &mut border_col,
            &mut text_query,
        );
    }
}

fn resolve_stellar_cutaway_labels(
    body_type: BodyType,
    surf_k: f64,
    core_k: f64,
    opt_ignition: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
) -> [(String, String); 4] {
    match body_type {
        BodyType::QuasiStar => [
            (
                "QUASI-STAR ENVELOPE".to_string(),
                format!("Reddened Cocoon ({surf_k:.0} K)"),
            ),
            (
                "CONVECTIVE H-SHELL".to_string(),
                "Super-Eddington Outflow Envelope".to_string(),
            ),
            (
                "ACCRETION CAVITY".to_string(),
                "Radiation-Supported Torus Boundary".to_string(),
            ),
            (
                "CENTRAL BLACK HOLE".to_string(),
                "Relativistic Event Horizon & Core".to_string(),
            ),
        ],
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar | BodyType::QuarkStar => [
            (
                "MAGNETOSPHERE & CRUST".to_string(),
                format!("Crystalline Iron Lattice ({surf_k:.0} K)"),
            ),
            (
                "OUTER NEUTRON SHELL".to_string(),
                "Superfluid Neutron-Proton Pasta".to_string(),
            ),
            (
                "INNER FLUID MANTLE".to_string(),
                "Superconducting Relativistic Matter".to_string(),
            ),
            (
                if body_type == BodyType::QuarkStar {
                    "STRANGE QUARK CORE".to_string()
                } else {
                    "ULTRA-DENSE CORE".to_string()
                },
                "Hyperon / Deconfined QCD Plasma".to_string(),
            ),
        ],
        BodyType::WhiteDwarf => [
            (
                "RADIATIVE ENVELOPE".to_string(),
                format!("H / He Photosphere ({surf_k:.0} K)"),
            ),
            (
                "DEGENERATE SHELL".to_string(),
                "Electron Degeneracy Pressure Fluid".to_string(),
            ),
            (
                "C-O FLUID MANTLE".to_string(),
                "Isothermal Heat-Conducting Layer".to_string(),
            ),
            (
                "CRYSTALLINE CORE".to_string(),
                format!("Carbon-Oxygen Lattice ({core_k:.0} K)"),
            ),
        ],
        BodyType::BrownDwarf => [
            (
                "SUB-STELLAR CLOUDS".to_string(),
                format!("Methane / Silicate Haze ({surf_k:.0} K)"),
            ),
            (
                "MOLECULAR H2 ENVELOPE".to_string(),
                "Convective Cool Gas Shell".to_string(),
            ),
            (
                "METALLIC H+ LAYER".to_string(),
                "Degenerate Conductive Hydrogen".to_string(),
            ),
            (
                "ROCK-ICE / DEUTERIUM CORE".to_string(),
                format!("Partially Degenerate ({core_k:.0} K)"),
            ),
        ],
        _ => {
            let fusion_desc = if let Some(evo) = opt_evo {
                match evo.phase {
                    StellarEvolutionPhase::RedGiantBranch
                    | StellarEvolutionPhase::HeliumFlashAgb
                    | StellarEvolutionPhase::RedSupergiantBranch => {
                        format!("He / Shell Fusion ({core_k:.0} K)")
                    }
                    StellarEvolutionPhase::MainSequence => {
                        if core_k > 1.8e7 {
                            format!("CNO-Cycle Fusion ({core_k:.0} K)")
                        } else {
                            format!("p-p Chain Fusion ({core_k:.0} K)")
                        }
                    }
                    _ => format!("Gravitational / Core ({core_k:.0} K)"),
                }
            } else if opt_ignition.is_some_and(|i| !i.is_ignited) {
                format!("Kelvin-Helmholtz Contraction ({core_k:.0} K)")
            } else {
                format!("Thermonuclear Fusion ({core_k:.0} K)")
            };
            [
                (
                    "PHOTOSPHERE & CORONA".to_string(),
                    format!("Radiating Surface ({surf_k:.0} K)"),
                ),
                (
                    "CONVECTIVE ZONE".to_string(),
                    "Plasma Granulation & Dynamo".to_string(),
                ),
                (
                    "RADIATIVE ZONE".to_string(),
                    "Photon Diffusion & Thermal Transfer".to_string(),
                ),
                ("THERMONUCLEAR CORE".to_string(), fusion_desc),
            ]
        }
    }
}

/// Resolves human-readable scientific layer titles and physical descriptions.
pub fn resolve_cutaway_labels_with_telemetry(
    body: &CelestialBody,
    rad_au: f64,
    opt_diff: Option<&InternalDifferentiation>,
    opt_temp: Option<&Temperature>,
    opt_ignition: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
) -> [(String, String); 4] {
    if body.body_type.is_star_or_remnant() {
        let surf_k = opt_temp.map_or(5_778.0, |t| t.0).max(0.0);
        let core_k = opt_ignition.map_or(15_000_000.0, |i| i.core_temperature);
        return resolve_stellar_cutaway_labels(
            body.body_type,
            surf_k,
            core_k,
            opt_ignition,
            opt_evo,
        );
    }

    if body.body_type == BodyType::GasGiant {
        return [
            (
                "ATMOSPHERE & CHROMO".to_string(),
                "H2-He Cloud Decks & Jets".to_string(),
            ),
            (
                "MOLECULAR HYDROGEN".to_string(),
                "Supercritical Fluid Envelope".to_string(),
            ),
            (
                "METALLIC H+ DYNAMO".to_string(),
                "Liquid Metallic Hydrogen (B-Field)".to_string(),
            ),
            (
                "HEAVY ELEMENT CORE".to_string(),
                "Dense Silicate & Iron Seed".to_string(),
            ),
        ];
    }
    if body.body_type == BodyType::IceGiant {
        return [
            (
                "ATMOSPHERE & HAZE".to_string(),
                "H2-He-CH4 Methane Clouds".to_string(),
            ),
            (
                "SUPERCRITICAL ICE MANTLE".to_string(),
                "Ionic Fluid H2O-NH3-CH4 Slush".to_string(),
            ),
            (
                "TRANSITION BOUNDARY".to_string(),
                "High-Pressure Mantle Discontinuity".to_string(),
            ),
            (
                "SILICATE / METAL CORE".to_string(),
                "Dense Earth-Mass Rocky Core".to_string(),
            ),
        ];
    }

    let is_undifferentiated = if let Some(d) = opt_diff {
        !d.is_differentiated || d.core_radius_au <= 1e-10
    } else {
        matches!(
            body.body_type,
            BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal
        )
    };
    if is_undifferentiated {
        let total_km = (rad_au * AU_TO_KM).max(0.1);
        return [
            (
                format!("REGOLITH SHELL [{:.1} km]", (total_km * 0.1).max(0.05)),
                "Impact-Gardened Primitive Surface".to_string(),
            ),
            (
                format!("UNDIFFERENTIATED INTERIOR [{total_km:.1} km]"),
                "Homogeneous Chondritic Rock & Ice".to_string(),
            ),
            (String::new(), String::new()),
            (String::new(), String::new()),
        ];
    }

    // Differentiated Terrestrial Planet / Moon
    let (_core_km, mantle_km, crust_km, inner_core_km, outer_core_km, core_k, has_theia) = opt_diff
        .map_or_else(
            || {
                let total_km = rad_au * AU_TO_KM;
                let c = total_km * 0.55;
                let m = total_km * 0.40;
                let cr = (total_km * 0.05).max(10.0);
                let ic = c * 0.38;
                let oc = c - ic;
                (c, m, cr, ic, oc, 5_500.0, false)
            },
            |d| {
                let c = d.core_radius_au * AU_TO_KM;
                let m = (d.mantle_radius_au - d.core_radius_au).max(0.0) * AU_TO_KM;
                let cr = (rad_au - d.mantle_radius_au).max(0.0) * AU_TO_KM;
                let ic = c * 0.38;
                let oc = c - ic;
                (
                    c,
                    m,
                    cr.max(5.0),
                    ic,
                    oc,
                    d.core_temp_k.max(200.0),
                    d.has_theia_llsvp,
                )
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
            format!("Molten Fe-Ni Dynamo ({:.0} K)", core_k * 0.82),
        ),
        (
            format!("INNER CORE [{inner_core_km:.0} km]"),
            format!("Solid Fe-Ni Crystal ({core_k:.0} K)"),
        ),
    ]
}

#[allow(
    dead_code,
    reason = "Convenience wrapper kept for existing unit callers"
)]
fn resolve_cutaway_labels(
    body: &CelestialBody,
    rad_au: f64,
    opt_diff: Option<&InternalDifferentiation>,
) -> [(String, String); 4] {
    resolve_cutaway_labels_with_telemetry(body, rad_au, opt_diff, None, None, None)
}
