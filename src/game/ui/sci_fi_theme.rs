//! Sci-Fi Holographic `UiMaterial` shader and automatic HUD frame injector.

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use super::types::{
    CampaignPanelElement, EpochScrubberPanel, HudPanelElement, HudToastContainer,
    PlanetBuilderPanel, TechTreePanelElement, TelemetryGraphPanel, TrophiesPanelElement,
};
use crate::game::ui::hazards_panel::HazardsPanelElement;

#[derive(Asset, TypePath, AsBindGroup, Clone, Copy, Debug)]
pub struct SciFiPanelMaterial {
    #[uniform(0)]
    pub uniforms: SciFiPanelUniforms,
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct SciFiPanelUniforms {
    pub bg_color: LinearRgba,
    pub border_color: LinearRgba,
    /// x: chamfer_px, y: border_px, z: scanline_alpha, w: is_pill (1.0 or 0.0)
    pub params: Vec4,
}

impl UiMaterial for SciFiPanelMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/sci_fi_panel.wgsl".into()
    }
}

/// Marker component so we only inject the sci-fi shader backdrop once per HUD panel.
#[derive(Component)]
pub struct SciFiFramed;

pub struct SciFiUiThemePlugin;

impl Plugin for SciFiUiThemePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<SciFiPanelMaterial>::default())
            .add_systems(Update, inject_sci_fi_panel_frames);
    }
}

/// Automatically upgrades HUD panels, modals, and restore pills with the WGSL Sci-Fi hologram shader
/// while keeping their `Node`, `BackgroundColor`, and `BorderColor` components intact for ECS queries.
#[allow(
    clippy::type_complexity,
    reason = "Matches all HUD panel and modal markers"
)]
pub fn inject_sci_fi_panel_frames(
    mut commands: Commands,
    mut materials: Option<ResMut<Assets<SciFiPanelMaterial>>>,
    mut panels_query: Query<
        (
            Entity,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&HudPanelElement>,
            Has<HudToastContainer>,
        ),
        (
            Without<SciFiFramed>,
            Or<(
                With<HudPanelElement>,
                With<HudToastContainer>,
                With<PlanetBuilderPanel>,
                With<TelemetryGraphPanel>,
                With<EpochScrubberPanel>,
                With<TrophiesPanelElement>,
                With<TechTreePanelElement>,
                With<CampaignPanelElement>,
                With<HazardsPanelElement>,
            )>,
        ),
    >,
) {
    let Some(ref mut materials) = materials else {
        return;
    };

    for (entity, mut bg, mut border, opt_hud_elem, is_toast) in panels_query.iter_mut() {
        // Skip the full-screen invisible RootContainer and small inline mode buttons
        if matches!(
            opt_hud_elem,
            Some(
                HudPanelElement::RootContainer
                    | HudPanelElement::OrbitModeBadge
                    | HudPanelElement::OverlayModeBadge
                    | HudPanelElement::TrajectoryPredictorBadge
            )
        ) {
            commands.entity(entity).insert(SciFiFramed);
            continue;
        }

        let base_bg = bg.0;
        let base_border = border.top;

        // Skip internal content-only containers that have transparent backgrounds
        if base_bg.alpha() < 0.05 {
            commands.entity(entity).insert(SciFiFramed);
            continue;
        }

        let is_pill = is_toast
            || matches!(
                opt_hud_elem,
                Some(
                    HudPanelElement::TopLeftPill
                        | HudPanelElement::TopRightPill
                        | HudPanelElement::InspectorChip
                        | HudPanelElement::ScenarioPresetsPill
                        | HudPanelElement::TopControlsPill
                        | HudPanelElement::BottomRightPill
                        | HudPanelElement::BottomCenterPill
                        | HudPanelElement::PlanetBuilderPill
                        | HudPanelElement::TelemetryPanelPill
                )
            );

        let chamfer = if is_pill { 8.0 } else { 15.0 };
        let border_px = if is_pill { 1.4 } else { 1.6 };
        let scanline_alpha = if is_pill { 0.35 } else { 0.75 };

        let mat_handle = materials.add(SciFiPanelMaterial {
            uniforms: SciFiPanelUniforms {
                bg_color: base_bg.to_linear(),
                border_color: base_border.to_linear(),
                params: Vec4::new(
                    chamfer,
                    border_px,
                    scanline_alpha,
                    if is_pill { 1.0 } else { 0.0 },
                ),
            },
        });

        // Make the original rectangular Node background and border invisible so the
        // 45-degree chamfered WGSL shader backdrop renders cleanly behind the panel's children!
        bg.0 = Color::NONE;
        *border = BorderColor::all(Color::NONE);

        commands
            .entity(entity)
            .insert(SciFiFramed)
            .with_children(|parent| {
                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    MaterialNode(mat_handle),
                    ZIndex(-1),
                    Pickable::IGNORE,
                ));
            });
    }
}
