//! HUD overlay panel visibility and minimization synchronization systems.

use bevy::prelude::*;

use crate::simulation::components::CelestialBody;
use crate::simulation::resources::PlayerInteractionState;

use super::types::{
    HudDynamicText, HudPanelElement, HudVisibilityState, PlanetBuilderPanel, PlanetBuilderState,
    TelemetryGraphPanel, TelemetryPanelState,
};

/// Synchronizes visibility for collapsible HUD panels and master full-screen view mode.
#[allow(
    clippy::type_complexity,
    reason = "Many overlapping queries, each with specific element types"
)]
pub fn update_hud_visibility(
    hud_visibility: Res<HudVisibilityState>,
    player_state: Res<PlayerInteractionState>,
    builder_state: Res<PlanetBuilderState>,
    telemetry_state: Res<TelemetryPanelState>,
    mut panel_nodes: ParamSet<(
        Query<(&mut Node, &HudPanelElement)>,
        Query<&mut Node, With<PlanetBuilderPanel>>,
        Query<&mut Node, With<TelemetryGraphPanel>>,
    )>,
    mut text_query: Query<(&mut Text, &HudDynamicText)>,
    names_query: Query<&CelestialBody>,
    opt_predictor: Option<Res<crate::simulation::predictor::TrajectoryPredictorState>>,
) {
    update_panel_nodes(
        &mut panel_nodes.p0(),
        &hud_visibility,
        &builder_state,
        &telemetry_state,
    );
    update_builder_drawer(&mut panel_nodes.p1(), &builder_state);
    update_telemetry_drawer(&mut panel_nodes.p2(), &telemetry_state, &hud_visibility);
    update_dynamic_badges(
        &mut text_query,
        &hud_visibility,
        &player_state,
        &names_query,
        opt_predictor.as_deref(),
    );
}

fn update_panel_nodes(
    panels_query: &mut Query<(&mut Node, &HudPanelElement)>,
    hud_visibility: &HudVisibilityState,
    builder_state: &PlanetBuilderState,
    telemetry_state: &TelemetryPanelState,
) {
    for (mut node, element) in panels_query.iter_mut() {
        node.display = match element {
            HudPanelElement::RootContainer
            | HudPanelElement::OrbitModeBadge
            | HudPanelElement::OverlayModeBadge
            | HudPanelElement::TrajectoryPredictorBadge => {
                flex_or_none(!hud_visibility.is_full_screen_clean)
            }
            HudPanelElement::TopLeftPanel => flex_or_none(!hud_visibility.top_left_minimized),
            HudPanelElement::TopLeftPill => flex_or_none(hud_visibility.top_left_minimized),
            HudPanelElement::TopRightPanel => flex_or_none(!hud_visibility.top_right_minimized),
            HudPanelElement::TopRightPill => flex_or_none(hud_visibility.top_right_minimized),
            HudPanelElement::InspectorPanel => flex_or_none(!hud_visibility.inspector_minimized),
            HudPanelElement::InspectorChip => flex_or_none(hud_visibility.inspector_minimized),
            HudPanelElement::ScenarioPresets => flex_or_none(!hud_visibility.scenarios_minimized),
            HudPanelElement::ScenarioPresetsPill => {
                flex_or_none(hud_visibility.scenarios_minimized)
            }
            HudPanelElement::TopControlsPanel => {
                flex_or_none(!hud_visibility.top_controls_minimized)
            }
            HudPanelElement::TopControlsPill => flex_or_none(hud_visibility.top_controls_minimized),
            HudPanelElement::BottomRightPanel => {
                flex_or_none(!hud_visibility.bottom_right_minimized)
            }
            HudPanelElement::BottomRightPill => flex_or_none(hud_visibility.bottom_right_minimized),
            HudPanelElement::BottomCenterPanel => {
                flex_or_none(!hud_visibility.bottom_center_minimized)
            }
            HudPanelElement::BottomCenterPill => {
                flex_or_none(hud_visibility.bottom_center_minimized)
            }
            HudPanelElement::PlanetBuilderPill => {
                flex_or_none(builder_state.is_open && builder_state.is_minimized)
            }
            HudPanelElement::TelemetryPanelPill => {
                flex_or_none(telemetry_state.is_open && hud_visibility.telemetry_minimized)
            }
        };
    }
}

fn update_builder_drawer(
    builder_query: &mut Query<&mut Node, With<PlanetBuilderPanel>>,
    builder_state: &PlanetBuilderState,
) {
    if let Ok(mut node) = builder_query.single_mut() {
        node.display = flex_or_none(builder_state.is_open && !builder_state.is_minimized);
    }
}

fn update_telemetry_drawer(
    telemetry_query: &mut Query<&mut Node, With<TelemetryGraphPanel>>,
    telemetry_state: &TelemetryPanelState,
    hud_visibility: &HudVisibilityState,
) {
    if let Ok(mut node) = telemetry_query.single_mut() {
        node.display = flex_or_none(telemetry_state.is_open && !hud_visibility.telemetry_minimized);
    }
}

fn update_dynamic_badges(
    text_query: &mut Query<(&mut Text, &HudDynamicText)>,
    hud_visibility: &HudVisibilityState,
    player_state: &PlayerInteractionState,
    names_query: &Query<&CelestialBody>,
    opt_predictor: Option<&crate::simulation::predictor::TrajectoryPredictorState>,
) {
    for (mut text, dynamic_text) in text_query.iter_mut() {
        match dynamic_text {
            HudDynamicText::FullScreenBadge => {
                text.0 = if hud_visibility.is_full_screen_clean {
                    "👁️ Show HUD".to_string()
                } else {
                    "⛶ Fullscreen".to_string()
                };
            }
            HudDynamicText::OrbitModeBadge => {
                text.0 = format!("궤 Orbits: {} [Y]", player_state.orbit_mode.display_label());
            }
            HudDynamicText::OverlayModeBadge => {
                text.0 = format!("OVERLAY: {} [V]", player_state.overlay_mode.display_name());
            }
            HudDynamicText::TrajectoryPredictorBadge => {
                let is_on = opt_predictor.is_none_or(|p| p.is_enabled);
                text.0 = if is_on {
                    "🎯 Forecast: ON [N]".to_string()
                } else {
                    "🎯 Forecast: OFF [N]".to_string()
                };
            }
            HudDynamicText::InspectorChip => {
                if let Some(target) = player_state.selected_entity {
                    if let Ok(body) = names_query.get(target) {
                        text.0 = format!("🔍 Inspector: {} ▲ Expand", body.name);
                    } else {
                        text.0 = "🔍 Inspector ▲ Expand".to_string();
                    }
                } else {
                    text.0 = "🔍 Inspector (No Selection) ▲ Expand".to_string();
                }
            }
        }
    }
}

fn flex_or_none(visible: bool) -> Display {
    if visible {
        Display::Flex
    } else {
        Display::None
    }
}
