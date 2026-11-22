//! HUD overlay panel visibility and minimization synchronization systems.

use bevy::prelude::*;

use crate::simulation::components::{BodyType, CelestialBody, CentralStar};
use crate::simulation::resources::PlayerInteractionState;

use super::types::{
    CosmicEventLog, HrDiagramPanel, HrDiagramPanelState, HudButton, HudDynamicText,
    HudPanelElement, HudVisibilityState, InspectorExoticHeader, InspectorSection, InspectorTab,
    PlanetBuilderPanel, PlanetBuilderState, QuickBarButtonBaseColor, QuickBarScrollViewport,
    QuickBarState, QuickBodySelectorBar, TelemetryGraphPanel, TelemetryPanelState, UiButtonAction,
};
use crate::simulation::resources::{
    DiagnosticOverlayMode, HabitableZoneConfig, LagrangeOverlayConfig, SimulationConfig,
};

/// Synchronizes visibility for collapsible HUD panels and master full-screen view mode.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Many overlapping queries, each with specific element types and optional HUD state"
)]
pub fn update_hud_visibility(
    hud_visibility: Res<HudVisibilityState>,
    player_state: Res<PlayerInteractionState>,
    builder_state: Res<PlanetBuilderState>,
    telemetry_state: Res<TelemetryPanelState>,
    opt_hr_state: Option<Res<HrDiagramPanelState>>,
    opt_lagrange: Option<Res<LagrangeOverlayConfig>>,
    opt_hz: Option<Res<HabitableZoneConfig>>,
    opt_misc: (
        Option<Res<SimulationConfig>>,
        Option<Res<crate::game::audio::CosmicAudioState>>,
        Option<Res<QuickBarState>>,
        Option<Res<CosmicEventLog>>,
    ),
    mut panel_nodes: ParamSet<(
        Query<(&mut Node, &HudPanelElement)>,
        Query<&mut Node, With<PlanetBuilderPanel>>,
        Query<&mut Node, With<TelemetryGraphPanel>>,
        Query<&mut Node, With<HrDiagramPanel>>,
        Query<&mut Node, With<QuickBarScrollViewport>>,
        Query<&mut Node, With<QuickBodySelectorBar>>,
    )>,
    mut text_query: Query<(&mut Text, Option<&mut TextColor>, &HudDynamicText)>,
    names_query: Query<&CelestialBody>,
    opt_predictor: Option<Res<crate::simulation::predictor::TrajectoryPredictorState>>,
    mut tab_buttons_query: Query<
        (
            &UiButtonAction,
            &QuickBarButtonBaseColor,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<HudButton>,
    >,
) {
    let (opt_config, opt_audio, opt_quick_bar, opt_event_log) = opt_misc;

    update_panel_nodes(
        &mut panel_nodes.p0(),
        &hud_visibility,
        &builder_state,
        &telemetry_state,
        opt_hr_state.as_deref(),
        opt_quick_bar.as_deref(),
    );
    update_quick_bar_viewport_layout(&mut panel_nodes.p4(), opt_quick_bar.as_deref());
    update_quick_bar_wrap_layout(&mut panel_nodes.p5(), opt_quick_bar.as_deref());
    update_builder_drawer(&mut panel_nodes.p1(), &builder_state);
    update_telemetry_drawer(&mut panel_nodes.p2(), &telemetry_state, &hud_visibility);
    update_hr_diagram_drawer(
        &mut panel_nodes.p3(),
        opt_hr_state.as_deref(),
        &hud_visibility,
    );
    update_dynamic_badges(
        &mut text_query,
        &hud_visibility,
        &player_state,
        &names_query,
        opt_predictor.as_deref(),
        opt_lagrange.as_deref(),
        opt_hz.as_deref(),
        opt_config.as_deref(),
        opt_audio.as_deref(),
        opt_event_log.as_deref(),
    );
    update_inspector_tab_highlights(&mut tab_buttons_query, hud_visibility.active_inspector_tab);
}

fn update_inspector_tab_highlights(
    tab_query: &mut Query<
        (
            &UiButtonAction,
            &QuickBarButtonBaseColor,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<HudButton>,
    >,
    active_tab: InspectorTab,
) {
    for (action, base, mut bg, mut border) in tab_query.iter_mut() {
        if let UiButtonAction::SelectInspectorTab(tab) = action {
            let (target_bg, target_border) = if *tab == active_tab {
                (
                    BackgroundColor(Color::srgba(0.08, 0.28, 0.42, 0.95)),
                    BorderColor::all(Color::srgb(0.3, 1.0, 1.0)),
                )
            } else {
                (BackgroundColor(base.bg), BorderColor::all(base.border))
            };
            if *bg != target_bg {
                *bg = target_bg;
            }
            if *border != target_border {
                *border = target_border;
            }
        }
    }
}

fn update_panel_nodes(
    panels_query: &mut Query<(&mut Node, &HudPanelElement)>,
    hud_visibility: &HudVisibilityState,
    builder_state: &PlanetBuilderState,
    telemetry_state: &TelemetryPanelState,
    hr_state: Option<&HrDiagramPanelState>,
    quick_bar_state: Option<&QuickBarState>,
) {
    for (mut node, element) in panels_query.iter_mut() {
        let new_display = match element {
            HudPanelElement::RootContainer
            | HudPanelElement::QuickBarContainer
            | HudPanelElement::OrbitModeBadge
            | HudPanelElement::OverlayModeBadge
            | HudPanelElement::ParticleDensityBadge
            | HudPanelElement::TrajectoryPredictorBadge
            | HudPanelElement::LagrangePointsBadge
            | HudPanelElement::HabitableZoneBadge
            | HudPanelElement::AtmospheresBadge
            | HudPanelElement::CloudsBadge
            | HudPanelElement::AudioMuteBadge => flex_or_none(!hud_visibility.is_full_screen_clean),
            HudPanelElement::TopLeftPanel => flex_or_none(!hud_visibility.top_left_minimized),
            HudPanelElement::TopLeftPill => flex_or_none(hud_visibility.top_left_minimized),
            HudPanelElement::TopRightPanel => flex_or_none(!hud_visibility.top_right_minimized),
            HudPanelElement::TopRightPill => flex_or_none(hud_visibility.top_right_minimized),
            HudPanelElement::InspectorPanel => flex_or_none(!hud_visibility.inspector_minimized),
            HudPanelElement::InspectorChip => flex_or_none(hud_visibility.inspector_minimized),
            HudPanelElement::ScenarioPresets => flex_or_none(
                hud_visibility.scenarios_is_open && !hud_visibility.is_full_screen_clean,
            ),
            HudPanelElement::ScenarioPresetsPill => flex_or_none(
                hud_visibility.scenarios_minimized && !hud_visibility.is_full_screen_clean,
            ),
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
            HudPanelElement::HrDiagramPanelPill => {
                flex_or_none(hr_state.is_some_and(|h| h.is_open && h.is_minimized))
            }
            HudPanelElement::QuickBarFilterRow => flex_or_none(
                quick_bar_state.is_some_and(|qb| qb.show_filter)
                    && !hud_visibility.is_full_screen_clean,
            ),
            HudPanelElement::CinemaRestorePill => flex_or_none(hud_visibility.is_full_screen_clean),
            HudPanelElement::EventFeedPanel => flex_or_none(
                !hud_visibility.event_feed_minimized && !hud_visibility.is_full_screen_clean,
            ),
            HudPanelElement::EventFeedPill => flex_or_none(
                hud_visibility.event_feed_minimized && !hud_visibility.is_full_screen_clean,
            ),
        };
        if node.display != new_display {
            node.display = new_display;
        }
    }
}

fn update_quick_bar_viewport_layout(
    viewport_query: &mut Query<&mut Node, With<QuickBarScrollViewport>>,
    quick_bar_state: Option<&QuickBarState>,
) {
    let wrap_rows = quick_bar_state.is_some_and(|qb| qb.wrap_rows);
    let (target_overflow, target_max_height) = if wrap_rows {
        (Overflow::scroll_y(), Val::Vh(28.0))
    } else {
        (Overflow::scroll_x(), Val::Auto)
    };

    for mut node in viewport_query.iter_mut() {
        if node.overflow != target_overflow {
            node.overflow = target_overflow;
        }
        if node.max_height != target_max_height {
            node.max_height = target_max_height;
        }
    }
}

fn update_quick_bar_wrap_layout(
    selector_query: &mut Query<&mut Node, With<QuickBodySelectorBar>>,
    quick_bar_state: Option<&QuickBarState>,
) {
    let wrap_rows = quick_bar_state.is_some_and(|qb| qb.wrap_rows);
    let target_wrap = if wrap_rows {
        FlexWrap::Wrap
    } else {
        FlexWrap::NoWrap
    };

    for mut node in selector_query.iter_mut() {
        if node.flex_wrap != target_wrap {
            node.flex_wrap = target_wrap;
        }
    }
}

fn update_builder_drawer(
    builder_query: &mut Query<&mut Node, With<PlanetBuilderPanel>>,
    builder_state: &PlanetBuilderState,
) {
    if let Ok(mut node) = builder_query.single_mut() {
        let new_display = flex_or_none(builder_state.is_open && !builder_state.is_minimized);
        if node.display != new_display {
            node.display = new_display;
        }
    }
}

fn update_telemetry_drawer(
    telemetry_query: &mut Query<&mut Node, With<TelemetryGraphPanel>>,
    telemetry_state: &TelemetryPanelState,
    hud_visibility: &HudVisibilityState,
) {
    if let Ok(mut node) = telemetry_query.single_mut() {
        let new_display =
            flex_or_none(telemetry_state.is_open && !hud_visibility.telemetry_minimized);
        if node.display != new_display {
            node.display = new_display;
        }
    }
}

fn update_hr_diagram_drawer(
    hr_query: &mut Query<&mut Node, With<HrDiagramPanel>>,
    hr_state: Option<&HrDiagramPanelState>,
    hud_visibility: &HudVisibilityState,
) {
    if let (Some(state), Ok(mut node)) = (hr_state, hr_query.single_mut()) {
        let new_display = flex_or_none(
            state.is_open && !state.is_minimized && !hud_visibility.is_full_screen_clean,
        );
        if node.display != new_display {
            node.display = new_display;
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Reads optional simulation and HUD states to synchronize badge text labels"
)]
fn update_dynamic_badges(
    text_query: &mut Query<(&mut Text, Option<&mut TextColor>, &HudDynamicText)>,
    hud_visibility: &HudVisibilityState,
    player_state: &PlayerInteractionState,
    names_query: &Query<&CelestialBody>,
    opt_predictor: Option<&crate::simulation::predictor::TrajectoryPredictorState>,
    opt_lagrange: Option<&LagrangeOverlayConfig>,
    opt_hz: Option<&HabitableZoneConfig>,
    opt_config: Option<&SimulationConfig>,
    opt_audio: Option<&crate::game::audio::CosmicAudioState>,
    opt_event_log: Option<&CosmicEventLog>,
) {
    for (mut text, opt_color, dynamic_text) in text_query.iter_mut() {
        let (new_text, new_color): (&str, Option<Color>) = match dynamic_text {
            HudDynamicText::FullScreenBadge => {
                if hud_visibility.is_full_screen_clean {
                    ("👁️ Show HUD [`]", None)
                } else {
                    ("🎬 Cinema [`]", None)
                }
            }
            HudDynamicText::OrbitModeBadge => {
                let s = format!("궤 Orbits: {} [Y]", player_state.orbit_mode.display_label());
                if text.0 != s {
                    text.0 = s;
                }
                continue;
            }
            HudDynamicText::OverlayModeBadge => {
                let s = format!("OVERLAY: {} [V]", player_state.overlay_mode.display_name());
                if text.0 != s {
                    text.0 = s;
                }
                continue;
            }
            HudDynamicText::ParticleDensityBadge => {
                let label = opt_config.map_or("500k [Ultra]", |c| c.particle_density.label());
                let s = format!("✨ Swarm: {label}");
                if text.0 != s {
                    text.0 = s;
                }
                continue;
            }
            HudDynamicText::TrajectoryPredictorBadge => {
                let is_on = opt_predictor.is_none_or(|p| p.is_enabled);
                if is_on {
                    ("🎯 Forecast: ON [N]", None)
                } else {
                    ("🎯 Forecast: OFF [N]", None)
                }
            }
            HudDynamicText::LagrangePointsBadge => {
                let is_on = opt_lagrange.is_some_and(|l| l.show_lagrange_points)
                    || player_state.overlay_mode == DiagnosticOverlayMode::HillSpheresAndGaps;
                if is_on {
                    ("🪐 Lagrange: ON [Shift-L]", None)
                } else {
                    ("🪐 Lagrange: OFF [Shift-L]", None)
                }
            }
            HudDynamicText::HabitableZoneBadge => {
                let is_on = opt_hz.is_none_or(|h| h.is_enabled);
                if is_on {
                    ("🌱 Hab Zone: ON", None)
                } else {
                    ("🌱 Hab Zone: OFF", None)
                }
            }
            HudDynamicText::AtmospheresBadge => {
                let is_on = player_state.show_atmospheres();
                if is_on {
                    ("🌫️ Atmo: ON [Shift-A]", Some(Color::srgb(0.55, 0.88, 1.0)))
                } else {
                    ("🪨 Atmo: OFF [Shift-A]", Some(Color::srgb(1.0, 0.68, 0.35)))
                }
            }
            HudDynamicText::CloudsBadge => {
                let is_on = player_state.show_clouds();
                if is_on {
                    (
                        "☁️ Clouds: ON [Shift-W]",
                        Some(Color::srgb(0.85, 0.92, 1.0)),
                    )
                } else {
                    (
                        "☀️ Clouds: OFF [Shift-W]",
                        Some(Color::srgb(1.0, 0.82, 0.38)),
                    )
                }
            }
            HudDynamicText::AudioMuteBadge => {
                let is_on = opt_audio.is_some_and(|a| a.enabled);
                if is_on {
                    ("🔊 Sound: ON [M]", Some(Color::srgb(0.35, 0.98, 0.65)))
                } else {
                    ("🔇 Sound: OFF [M]", Some(Color::srgb(0.68, 0.76, 0.90)))
                }
            }
            HudDynamicText::InspectorChip => {
                let chip_text = if let Some(target) = player_state.selected_entity {
                    if let Ok(body) = names_query.get(target) {
                        format!("🔍 Inspector: {} ▲ Expand", body.name)
                    } else {
                        "🔍 Inspector ▲ Expand".to_string()
                    }
                } else {
                    "🔍 Inspector (No Selection) ▲ Expand".to_string()
                };
                if text.0 != chip_text {
                    text.0 = chip_text;
                }
                continue;
            }
            HudDynamicText::EventFeedBadge => {
                let (total, unread) =
                    opt_event_log.map_or((0, 0), |log| (log.entries.len(), log.unread_count));
                let badge_text = format!("📡 Events ({total}) [Shift-E]");
                if text.0 != badge_text {
                    text.0 = badge_text;
                }
                let target_color = if unread > 0 {
                    Color::srgb(1.0, 0.85, 0.35)
                } else {
                    Color::srgb(0.5, 0.85, 1.0)
                };
                if let Some(mut color) = opt_color
                    && color.0 != target_color
                {
                    color.0 = target_color;
                }
                continue;
            }
        };

        if text.0 != new_text {
            text.0 = new_text.to_string();
        }
        if let (Some(target_color), Some(mut color)) = (new_color, opt_color)
            && color.0 != target_color
        {
            color.0 = target_color;
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

/// Synchronizes visibility of scenario-contextual buttons and inspector action rows.
#[allow(
    clippy::type_complexity,
    reason = "Queries for contextual buttons, inspector sections, and header text"
)]
pub fn update_scenario_contextual_ui(
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    player_state: Res<PlayerInteractionState>,
    opt_visibility: Option<Res<HudVisibilityState>>,
    target_query: Query<(
        &CelestialBody,
        Option<&CentralStar>,
        Option<&crate::simulation::probes::SpaceProbe>,
    )>,
    mut button_nodes: Query<(&UiButtonAction, &mut Node)>,
    mut section_nodes: Query<(&InspectorSection, &mut Node), Without<UiButtonAction>>,
    mut header_text: Query<&mut Text, With<InspectorExoticHeader>>,
) {
    let preset = scenario_state.as_ref().map_or(
        crate::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn,
        |s| s.current_preset,
    );

    let selected_info = player_state
        .selected_entity
        .and_then(|e| target_query.get(e).ok());
    let is_probe = selected_info.is_some_and(|(_, _, opt_probe)| opt_probe.is_some());
    let is_star_like = selected_info.is_some_and(|(body, opt_star, _)| {
        opt_star.is_some() || body.body_type.is_star_or_remnant()
    });
    let is_active_star = selected_info.is_some_and(|(body, opt_star, _)| {
        !body.body_type.is_remnant() && (opt_star.is_some() || body.body_type.is_star_or_remnant())
    });
    let is_ageable_star = selected_info.is_some_and(|(body, opt_star, _)| {
        (opt_star.is_some() || body.body_type.is_star_or_remnant())
            && !matches!(
                body.body_type,
                BodyType::BlackHole
                    | BodyType::BlackDwarf
                    | BodyType::IronStar
                    | BodyType::BosonStar
                    | BodyType::QuasiStar
            )
    });

    // 1. Synchronize inspector section containers
    for (section, mut node) in section_nodes.iter_mut() {
        let matches_tab =
            opt_visibility
                .as_ref()
                .is_none_or(|vis| match vis.active_inspector_tab {
                    InspectorTab::Overview => matches!(
                        section,
                        InspectorSection::OrbitTracking | InspectorSection::MassComposition
                    ),
                    InspectorTab::GeodynamicsBio => matches!(
                        section,
                        InspectorSection::TerraformingBombardment
                            | InspectorSection::Astrophysics
                            | InspectorSection::HydrosphereBathymetry
                    ),
                    InspectorTab::RelativityFields => {
                        matches!(section, InspectorSection::ExoticExperiments)
                    }
                });

        let target_display = match section {
            InspectorSection::TerraformingBombardment | InspectorSection::HydrosphereBathymetry => {
                // Targeted bombardment (Comet, Chondrite Asteroid, Volatile Salvo, Core Impactor)
                // and hydrosphere bathymetry are universally available across all scenarios
                // for planets, moons, embryos, planetesimals, and minor bodies (any non-star, non-probe body).
                flex_or_none(!is_star_like && !is_probe && matches_tab)
            }
            InspectorSection::ExoticExperiments
            | InspectorSection::OrbitTracking
            | InspectorSection::MassComposition
            | InspectorSection::Astrophysics => flex_or_none(!is_probe && matches_tab),
        };
        if node.display != target_display {
            node.display = target_display;
        }
    }

    // 2. Synchronize individual buttons according to their scenario correlation
    for (action, mut node) in button_nodes.iter_mut() {
        let target_display = match action {
            // Relativistic Binary Scenario actions
            UiButtonAction::AccelerateInspiral => flex_or_none(
                preset == crate::simulation::scenarios::ScenarioPreset::RelativisticBinary,
            ),
            // Solar Nebula MMSN Scenario actions
            UiButtonAction::InjectEmbryo | UiButtonAction::TriggerLhb => flex_or_none(
                preset == crate::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn,
            ),
            // Little Red Dot Scenario actions
            UiButtonAction::ToggleSuperEddington | UiButtonAction::TriggerBlowoutCocoon => {
                flex_or_none(preset == crate::simulation::scenarios::ScenarioPreset::LittleRedDot)
            }
            // Pop-III Hypergiant, Surface Horizon Camera, and 3D Internal Cutaway are UNIVERSAL (always visible)
            UiButtonAction::SpawnInfallPop3Star
            | UiButtonAction::ToggleSurfaceHorizonCamera
            | UiButtonAction::ToggleInternalCutaway => Display::Flex,
            // Molecular Cloud Supernova shockwave trigger
            UiButtonAction::TriggerGmcSupernovaShock => flex_or_none(
                preset == crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster,
            ),
            // Coronal Mass Ejection: Magnetar Outburst scenario or active stars
            UiButtonAction::TriggerCoronalMassEjection => flex_or_none(
                preset == crate::simulation::scenarios::ScenarioPreset::MagnetarOutburst
                    || is_star_like,
            ),
            // Biosphere seeding & universal civilization setup: available on solid non-star, non-probe worlds
            UiButtonAction::SeedLife
            | UiButtonAction::ToggleCivOrbitalRing
            | UiButtonAction::ToggleCivSolarMirrors
            | UiButtonAction::ToggleCivPlanetaryShield
            | UiButtonAction::CycleFloraAndBiolum
            | UiButtonAction::ClearCivilization => flex_or_none(!is_star_like && !is_probe),
            // Stellar ignition: active stars; Aging: also compact remnants until terminal Black Dwarf / Black Hole
            UiButtonAction::IgniteStar => flex_or_none(is_active_star),
            UiButtonAction::AgeStar => flex_or_none(is_ageable_star),
            // Geological Epoch scrubber: scenarios with planetary era timelines
            UiButtonAction::ToggleEpochScrubberPanel => flex_or_none(matches!(
                preset,
                crate::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn
                    | crate::simulation::scenarios::ScenarioPreset::AccretionDiskGenesis
            )),
            _ => node.display,
        };
        if node.display != target_display {
            node.display = target_display;
        }
    }

    // 3. Dynamic header text for exotic experiments
    let target_header = if preset == crate::simulation::scenarios::ScenarioPreset::LittleRedDot {
        "EXOTIC / LITTLE RED DOT:"
    } else {
        "EXOTIC PHENOMENA:"
    };
    for mut text in header_text.iter_mut() {
        if text.0 != target_header {
            text.0 = target_header.to_string();
        }
    }
}
