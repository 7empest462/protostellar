//! Viewport camera, simulation tools, scenario presets, and system hotkey dispatch.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::resources::{
    DiagnosticOverlayMode, OrbitVisualizationMode, PlayerInteractionState, PlayerTool,
    SimulationConfig,
};
use crate::utils::constants::EARTH_MASS_SOLAR;

#[allow(clippy::too_many_arguments, reason = "Modal mutual exclusion helper")]
fn close_other_modals(
    keep_open: crate::game::ui::ActiveModalKind,
    builder: &mut crate::game::ui::PlanetBuilderState,
    telemetry: &mut crate::game::ui::TelemetryPanelState,
    trophies: Option<&mut crate::game::ui::types::TrophiesPanelState>,
    tech_tree: Option<&mut crate::game::ui::types::TechTreePanelState>,
    campaign: Option<&mut crate::game::ui::types::CampaignPanelState>,
    hazards: Option<&mut crate::game::ui::hazards_panel::HazardsPanelState>,
    scrubber: Option<&mut crate::simulation::geology::types::TimelineScrubber>,
    hr_diagram: Option<&mut crate::game::ui::types::HrDiagramPanelState>,
    visibility: Option<&mut crate::game::ui::types::HudVisibilityState>,
) -> bool {
    let mut states = crate::game::ui::ModalPanelStates {
        builder: Some(builder),
        telemetry: Some(telemetry),
        trophies,
        tech_tree,
        campaign,
        hazards,
        scrubber,
        hr_diagram,
        visibility,
    };
    states.close_conflicting_modals(keep_open)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Escape key priority stack helper"
)]
fn handle_escape_priority_stack(
    camera_query: &mut Query<(&Transform, &mut PanOrbitCamera)>,
    player_state: &mut PlayerInteractionState,
    builder: &mut crate::game::ui::PlanetBuilderState,
    telemetry: &mut crate::game::ui::TelemetryPanelState,
    trophies: Option<&mut crate::game::ui::types::TrophiesPanelState>,
    tech_tree: Option<&mut crate::game::ui::types::TechTreePanelState>,
    campaign: Option<&mut crate::game::ui::types::CampaignPanelState>,
    hazards: Option<&mut crate::game::ui::hazards_panel::HazardsPanelState>,
    scrubber: Option<&mut crate::simulation::geology::types::TimelineScrubber>,
    hr_diagram: Option<&mut crate::game::ui::types::HrDiagramPanelState>,
    visibility: Option<&mut crate::game::ui::types::HudVisibilityState>,
    toast: &mut crate::game::ui::NotificationToast,
) {
    // Priority 1: If Surface Horizon Mode is active on any camera, exit surface mode back to orbital view
    let mut surface_mode_exited = false;
    for (_, mut cam) in camera_query.iter_mut() {
        if cam.surface_horizon_mode {
            cam.surface_horizon_mode = false;
            surface_mode_exited = true;
        }
    }
    if surface_mode_exited {
        toast.message = "🛰️ Returned to 3D Orbital Camera View [Esc]".to_string();
        toast.timer = 2.5;
        return;
    }

    // Priority 2: If any floating modal or drawer is open, close them first without deselecting active body
    if close_other_modals(
        crate::game::ui::ActiveModalKind::None,
        builder,
        telemetry,
        trophies,
        tech_tree,
        campaign,
        hazards,
        scrubber,
        hr_diagram,
        visibility,
    ) {
        toast.message = "✕ Closed Drawer / Modal Window [Esc]".to_string();
        toast.timer = 2.5;
        return;
    }

    // Priority 3: Only if no modals or surface views are active, deselect celestial body and clear camera focus target
    if player_state.selected_entity.is_some() {
        player_state.selected_entity = None;
        for (_, mut cam) in camera_query.iter_mut() {
            cam.target_entity = None;
            cam.target_focus = Vec3::ZERO;
        }
        toast.message = "Target Deselected [Esc]".to_string();
        toast.timer = 2.0;
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Hotkey dispatch system parameters"
)]
pub fn handle_viewport_and_tool_hotkeys(
    keyboard: &ButtonInput<KeyCode>,
    config: &mut SimulationConfig,
    player_state: &mut PlayerInteractionState,
    builder_state: &mut crate::game::ui::PlanetBuilderState,
    telemetry_state: &mut crate::game::ui::TelemetryPanelState,
    mut opt_scrubber: Option<&mut crate::simulation::geology::types::TimelineScrubber>,
    mut opt_tech_tree: Option<&mut crate::game::ui::types::TechTreePanelState>,
    mut opt_campaign_panel: Option<&mut crate::game::ui::types::CampaignPanelState>,
    mut opt_hazards_panel: Option<&mut crate::game::ui::hazards_panel::HazardsPanelState>,
    opt_hazard_mgr: Option<&mut crate::simulation::events::CosmicEventManager>,
    mut opt_hr_state: Option<&mut crate::game::ui::types::HrDiagramPanelState>,
    opt_lagrange: Option<&mut crate::simulation::resources::LagrangeOverlayConfig>,
    opt_hz: Option<&mut crate::simulation::resources::HabitableZoneConfig>,
    toast: &mut crate::game::ui::NotificationToast,
    camera_query: &mut Query<(&Transform, &mut PanOrbitCamera)>,
    mut opt_predictor: Option<&mut crate::simulation::predictor::TrajectoryPredictorState>,
    is_star_selected: bool,
    mut opt_trophies: Option<&mut crate::game::ui::types::TrophiesPanelState>,
    mut opt_visibility: Option<&mut crate::game::ui::types::HudVisibilityState>,
) {
    if keyboard.just_pressed(KeyCode::KeyN) && !is_star_selected {
        if let Some(ref mut predictor) = opt_predictor {
            predictor.is_enabled = !predictor.is_enabled;
            toast.message = if predictor.is_enabled {
                "🎯 Trajectory & Encounter Forecast: Active [N]".to_string()
            } else {
                "🎯 Trajectory & Encounter Forecast: Disabled [N]".to_string()
            };
            toast.timer = 2.5;
        }
    }
    handle_modal_toggle_hotkeys(
        keyboard,
        builder_state,
        telemetry_state,
        opt_scrubber.as_deref_mut(),
        opt_trophies.as_deref_mut(),
        opt_tech_tree.as_deref_mut(),
        opt_campaign_panel.as_deref_mut(),
        opt_hazards_panel.as_deref_mut(),
        opt_hr_state.as_deref_mut(),
        opt_visibility.as_deref_mut(),
        toast,
    );
    if keyboard.just_pressed(KeyCode::Comma) {
        config.size_exaggeration = (config.size_exaggeration * 0.7).max(0.1);
        info!("🔬 Size Scale: {:.2}×", config.size_exaggeration);
    }
    if keyboard.just_pressed(KeyCode::Period) {
        config.size_exaggeration = (config.size_exaggeration * 1.4).min(20.0);
        info!("🔬 Size Scale: {:.2}×", config.size_exaggeration);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        handle_escape_priority_stack(
            camera_query,
            player_state,
            builder_state,
            telemetry_state,
            opt_trophies.as_deref_mut(),
            opt_tech_tree.as_deref_mut(),
            opt_campaign_panel.as_deref_mut(),
            opt_hazards_panel.as_deref_mut(),
            opt_scrubber.as_deref_mut(),
            opt_hr_state.as_deref_mut(),
            opt_visibility.as_deref_mut(),
            toast,
        );
    }
    let is_shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if is_shift {
        if keyboard.just_pressed(KeyCode::KeyX) {
            player_state.cutaway_mode = !player_state.cutaway_mode;
            toast.message = if player_state.cutaway_mode {
                "🔬 3D Planetary Cutaway: ENABLED [Shift-X] (Core Cross-Section Active)".to_string()
            } else {
                "🔬 3D Planetary Cutaway: DISABLED [Shift-X] (Surface Restored)".to_string()
            };
            toast.timer = 2.5;
        } else {
            handle_shift_tool_hotkeys(
                keyboard,
                builder_state,
                telemetry_state,
                opt_scrubber,
                opt_tech_tree,
                opt_campaign_panel,
                opt_hazards_panel,
                opt_hazard_mgr,
                opt_lagrange,
                opt_hr_state,
                opt_hz,
                opt_trophies,
                opt_visibility,
                toast,
            );
        }
    } else {
        handle_overlay_and_tool_hotkeys(keyboard, player_state, camera_query, toast);
    }
}

#[allow(clippy::too_many_arguments, reason = "Modal toggle hotkey helper")]
fn handle_modal_toggle_hotkeys(
    keyboard: &ButtonInput<KeyCode>,
    builder_state: &mut crate::game::ui::PlanetBuilderState,
    telemetry_state: &mut crate::game::ui::TelemetryPanelState,
    mut opt_scrubber: Option<&mut crate::simulation::geology::types::TimelineScrubber>,
    mut opt_trophies: Option<&mut crate::game::ui::types::TrophiesPanelState>,
    mut opt_tech_tree: Option<&mut crate::game::ui::types::TechTreePanelState>,
    mut opt_campaign_panel: Option<&mut crate::game::ui::types::CampaignPanelState>,
    mut opt_hazards_panel: Option<&mut crate::game::ui::hazards_panel::HazardsPanelState>,
    mut opt_hr_state: Option<&mut crate::game::ui::types::HrDiagramPanelState>,
    mut opt_visibility: Option<&mut crate::game::ui::types::HudVisibilityState>,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        builder_state.is_open = !builder_state.is_open;
        if builder_state.is_open {
            close_other_modals(
                crate::game::ui::ActiveModalKind::PlanetBuilder,
                builder_state,
                telemetry_state,
                opt_trophies.as_deref_mut(),
                opt_tech_tree.as_deref_mut(),
                opt_campaign_panel.as_deref_mut(),
                opt_hazards_panel.as_deref_mut(),
                opt_scrubber.as_deref_mut(),
                opt_hr_state.as_deref_mut(),
                opt_visibility.as_deref_mut(),
            );
        }
        toast.message = if builder_state.is_open {
            "🛠️ Planet Builder & Spawner Opened [P]".to_string()
        } else {
            "🛠️ Planet Builder Closed [P]".to_string()
        };
        toast.timer = 2.5;
    }
    let is_shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if keyboard.just_pressed(KeyCode::F10) && !is_shift {
        telemetry_state.is_open = !telemetry_state.is_open;
        if telemetry_state.is_open {
            close_other_modals(
                crate::game::ui::ActiveModalKind::Telemetry,
                builder_state,
                telemetry_state,
                opt_trophies.as_deref_mut(),
                opt_tech_tree.as_deref_mut(),
                opt_campaign_panel.as_deref_mut(),
                opt_hazards_panel.as_deref_mut(),
                opt_scrubber.as_deref_mut(),
                opt_hr_state.as_deref_mut(),
                opt_visibility.as_deref_mut(),
            );
        }
        toast.message = if telemetry_state.is_open {
            "📈 Telemetry & Habitability Graph Opened [F10]".to_string()
        } else {
            "📈 Telemetry Graph Closed [F10]".to_string()
        };
        toast.timer = 2.5;
    }
    if keyboard.just_pressed(KeyCode::F11) && !is_shift {
        if let Some(ref mut scrubber) = opt_scrubber {
            scrubber.is_open = !scrubber.is_open;
            if scrubber.is_open {
                close_other_modals(
                    crate::game::ui::ActiveModalKind::EpochScrubber,
                    builder_state,
                    telemetry_state,
                    opt_trophies,
                    opt_tech_tree,
                    opt_campaign_panel,
                    opt_hazards_panel,
                    Some(scrubber),
                    opt_hr_state,
                    opt_visibility,
                );
            }
            toast.message = if scrubber.is_open {
                "⏳ Geological Epoch Scrubber Opened [F11]".to_string()
            } else {
                "⏳ Geological Epoch Scrubber Closed [F11]".to_string()
            };
            toast.timer = 2.5;
        }
    }
}

fn handle_overlay_and_tool_hotkeys(
    keyboard: &ButtonInput<KeyCode>,
    player_state: &mut PlayerInteractionState,
    camera_query: &Query<(&Transform, &mut PanOrbitCamera)>,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if keyboard.just_pressed(KeyCode::KeyV) {
        player_state.overlay_mode = player_state.overlay_mode.cycle();
    }
    if keyboard.just_pressed(KeyCode::KeyY) {
        player_state.orbit_mode = player_state.orbit_mode.cycle();
        if player_state.orbit_mode == OrbitVisualizationMode::Off {
            player_state.overlay_mode = DiagnosticOverlayMode::Hidden;
        }

        toast.message = match player_state.orbit_mode {
            OrbitVisualizationMode::All => "궤 Orbit Visualization: All Worlds [Y]".to_string(),
            OrbitVisualizationMode::SelectedOnly => {
                "궤 Orbit Visualization: Selected Target Only [Y]".to_string()
            }
            OrbitVisualizationMode::Off => {
                "궤 Orbit Visualization: Hidden (Cinematic) [Y]".to_string()
            }
        };
        toast.timer = 2.5;
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        if player_state.active_tool == PlayerTool::GravitationalTractor {
            player_state.active_tool = PlayerTool::Inspect;
            player_state.tractor_position = None;
            player_state.tractor_mass = 0.0;
        } else {
            player_state.active_tool = PlayerTool::GravitationalTractor;
            if let Ok((cam_trans, _)) = camera_query.single() {
                let tractor_pt = cam_trans.translation + cam_trans.forward() * 15.0;
                player_state.tractor_position = Some(DVec3::new(
                    f64::from(tractor_pt.x),
                    f64::from(tractor_pt.y),
                    f64::from(tractor_pt.z),
                ));
                player_state.tractor_mass = EARTH_MASS_SOLAR * 5.0;
            }
        }
    }
}

#[allow(clippy::too_many_arguments, reason = "Shift-tool hotkey dispatch")]
fn handle_shift_tool_hotkeys(
    keyboard: &ButtonInput<KeyCode>,
    builder: &mut crate::game::ui::PlanetBuilderState,
    telemetry: &mut crate::game::ui::TelemetryPanelState,
    mut opt_scrubber: Option<&mut crate::simulation::geology::types::TimelineScrubber>,
    mut opt_tech_tree: Option<&mut crate::game::ui::types::TechTreePanelState>,
    mut opt_campaign_panel: Option<&mut crate::game::ui::types::CampaignPanelState>,
    mut opt_hazards_panel: Option<&mut crate::game::ui::hazards_panel::HazardsPanelState>,
    mut opt_hazard_mgr: Option<&mut crate::simulation::events::CosmicEventManager>,
    mut opt_lagrange: Option<&mut crate::simulation::resources::LagrangeOverlayConfig>,
    mut opt_hr_state: Option<&mut crate::game::ui::types::HrDiagramPanelState>,
    mut opt_hz: Option<&mut crate::simulation::resources::HabitableZoneConfig>,
    mut opt_trophies: Option<&mut crate::game::ui::types::TrophiesPanelState>,
    mut opt_visibility: Option<&mut crate::game::ui::types::HudVisibilityState>,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if keyboard.just_pressed(KeyCode::KeyT) {
        if let Some(ref mut tech_state) = opt_tech_tree {
            tech_state.is_open = !tech_state.is_open;
            if tech_state.is_open {
                close_other_modals(
                    crate::game::ui::ActiveModalKind::TechTree,
                    builder,
                    telemetry,
                    opt_trophies.as_deref_mut(),
                    Some(tech_state),
                    opt_campaign_panel.as_deref_mut(),
                    opt_hazards_panel.as_deref_mut(),
                    opt_scrubber.as_deref_mut(),
                    opt_hr_state.as_deref_mut(),
                    opt_visibility.as_deref_mut(),
                );
            }
            toast.message = if tech_state.is_open {
                "🔬 Science & Technology Tree Opened [Shift-T]".to_string()
            } else {
                "🔬 Tech Tree Closed [Shift-T]".to_string()
            };
            toast.timer = 2.5;
        }
    } else if keyboard.just_pressed(KeyCode::KeyM) {
        if let Some(ref mut campaign_panel) = opt_campaign_panel {
            campaign_panel.is_open = !campaign_panel.is_open;
            if campaign_panel.is_open {
                close_other_modals(
                    crate::game::ui::ActiveModalKind::Campaign,
                    builder,
                    telemetry,
                    opt_trophies.as_deref_mut(),
                    opt_tech_tree.as_deref_mut(),
                    Some(campaign_panel),
                    opt_hazards_panel.as_deref_mut(),
                    opt_scrubber.as_deref_mut(),
                    opt_hr_state.as_deref_mut(),
                    opt_visibility.as_deref_mut(),
                );
            }
            toast.message = if campaign_panel.is_open {
                "🎯 Campaign & Scenario Challenges Opened [Shift-M]".to_string()
            } else {
                "🎯 Campaign Panel Closed [Shift-M]".to_string()
            };
            toast.timer = 2.5;
        }
    } else if keyboard.just_pressed(KeyCode::KeyH) {
        if let Some(ref mut hazards_panel) = opt_hazards_panel {
            hazards_panel.is_open = !hazards_panel.is_open;
            if hazards_panel.is_open {
                close_other_modals(
                    crate::game::ui::ActiveModalKind::Hazards,
                    builder,
                    telemetry,
                    opt_trophies.as_deref_mut(),
                    opt_tech_tree.as_deref_mut(),
                    opt_campaign_panel.as_deref_mut(),
                    Some(hazards_panel),
                    opt_scrubber.as_deref_mut(),
                    opt_hr_state.as_deref_mut(),
                    opt_visibility.as_deref_mut(),
                );
            }
            toast.message = if hazards_panel.is_open {
                "⚡ Cosmic Hazards & Sensory Lab Opened [Shift-H]".to_string()
            } else {
                "⚡ Hazards Panel Closed [Shift-H]".to_string()
            };
            toast.timer = 2.5;
        }
    } else if keyboard.just_pressed(KeyCode::KeyC) {
        if let Some(ref mut hazard_mgr) = opt_hazard_mgr {
            hazard_mgr.tonemapping_mode = hazard_mgr.tonemapping_mode.cycle();
            toast.message = format!(
                "🎨 Post-Processing Tonemap: {}",
                hazard_mgr.tonemapping_mode.label()
            );
            toast.timer = 3.0;
        }
    } else if keyboard.just_pressed(KeyCode::KeyL) {
        if let Some(ref mut lagrange) = opt_lagrange {
            lagrange.show_lagrange_points = !lagrange.show_lagrange_points;
            toast.message = if lagrange.show_lagrange_points {
                "🪐 Lagrange Points (L1-L5) & Hill Sphere: ENABLED [Shift-L]".to_string()
            } else {
                "🪐 Lagrange Points: DISABLED [Shift-L]".to_string()
            };
            toast.timer = 2.5;
        }
    } else if keyboard.just_pressed(KeyCode::KeyR) {
        if let Some(ref mut hr) = opt_hr_state {
            hr.is_open = !hr.is_open;
            if hr.is_open {
                close_other_modals(
                    crate::game::ui::ActiveModalKind::HrDiagram,
                    builder,
                    telemetry,
                    opt_trophies,
                    opt_tech_tree,
                    opt_campaign_panel,
                    opt_hazards_panel,
                    opt_scrubber,
                    Some(hr),
                    opt_visibility,
                );
            }
            toast.message = if hr.is_open {
                "🌟 Hertzsprung–Russell Stellar Diagram Opened [Shift-R]".to_string()
            } else {
                "🌟 H-R Diagram Closed [Shift-R]".to_string()
            };
            toast.timer = 2.5;
        }
    } else if keyboard.just_pressed(KeyCode::KeyZ) {
        if let Some(ref mut hz) = opt_hz {
            hz.is_enabled = !hz.is_enabled;
            toast.message = if hz.is_enabled {
                "🌱 Circumstellar Habitable Zone & Frost Line: ENABLED [Shift-Z]".to_string()
            } else {
                "🌱 Habitable Zone: DISABLED [Shift-Z]".to_string()
            };
            toast.timer = 2.5;
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Scenario and system hotkey handler"
)]
pub fn handle_scenario_and_system_hotkeys(
    keyboard: &ButtonInput<KeyCode>,
    config: &mut SimulationConfig,
    lhb_state: &mut crate::game::phases::LateHeavyBombardmentState,
    scenario_events: &mut MessageWriter<crate::simulation::scenarios::LoadScenarioEvent>,
    mut save_events: Option<&mut MessageWriter<crate::simulation::serialization::SaveSystemEvent>>,
    mut load_events: Option<&mut MessageWriter<crate::simulation::serialization::LoadSystemEvent>>,
    mut glitch_events: Option<
        &mut MessageWriter<crate::simulation::neutron_stars::TriggerManualGlitchOrStarquakeEvent>,
    >,
    mut oort_events: Option<
        &mut MessageWriter<crate::simulation::oort::TriggerOortCloudSwarmEvent>,
    >,
    toast: &mut crate::game::ui::NotificationToast,
    scenario_state: Option<&crate::simulation::scenarios::ActiveScenarioState>,
) {
    let is_shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if is_shift && keyboard.just_pressed(KeyCode::KeyO) {
        if let Some(ref mut oe) = oort_events {
            oe.write(crate::simulation::oort::TriggerOortCloudSwarmEvent { comet_count: 48 });
        }
        toast.message =
            "☄️ OORT CLOUD SWARM // Procedurally spawned 48 cometary bodies into Hills & Outer Oort reservoirs (200 - 25,000 AU)!"
                .to_string();
        toast.timer = 6.0;
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        if is_shift {
            if let Some(ref mut ge) = glitch_events {
                ge.write(
                    crate::simulation::neutron_stars::TriggerManualGlitchOrStarquakeEvent::default(
                    ),
                );
            }
            toast.message =
                "⚡ RELATIVISTIC EVENT TRIGGERED // Pulsar glitch / Magnetar starquake & Alfvén burst launched!"
                    .to_string();
            toast.timer = 6.0;
        } else {
            let is_mmsn = scenario_state.is_none_or(|s| {
                s.current_preset == crate::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn
            });
            if is_mmsn {
                lhb_state.is_active = true;
                lhb_state.manual_trigger_requested = true;
                toast.message =
                    "☄️ LATE HEAVY BOMBARDMENT TRIGGERED // Cometary ocean-seeding shower inbound!"
                        .to_string();
                toast.timer = 8.0;
            } else {
                toast.message =
                    "ℹ️ Late Heavy Bombardment epoch is exclusive to the Solar Nebula (MMSN) scenario."
                        .to_string();
                toast.timer = 5.0;
            }
        }
    }
    if keyboard.just_pressed(KeyCode::F1) {
        let preset = if is_shift {
            crate::simulation::scenarios::ScenarioPreset::AccretionDiskGenesis
        } else {
            crate::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn
        };
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(preset));
    }
    if is_shift && keyboard.just_pressed(KeyCode::F10) {
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
            crate::simulation::scenarios::ScenarioPreset::SagittariusAStar,
        ));
    }
    if is_shift && keyboard.just_pressed(KeyCode::F11) {
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
            crate::simulation::scenarios::ScenarioPreset::KozaiLidovTriple,
        ));
    }
    let preset_map = [
        (
            KeyCode::F2,
            crate::simulation::scenarios::ScenarioPreset::Trappist1System,
        ),
        (
            KeyCode::F3,
            crate::simulation::scenarios::ScenarioPreset::Kepler16Circumbinary,
        ),
        (
            KeyCode::F4,
            crate::simulation::scenarios::ScenarioPreset::HotJupiterMigration,
        ),
        (
            KeyCode::F5,
            crate::simulation::scenarios::ScenarioPreset::RoguePlanetFlyby,
        ),
        (
            KeyCode::F6,
            crate::simulation::scenarios::ScenarioPreset::LittleRedDot,
        ),
        (
            KeyCode::F7,
            crate::simulation::scenarios::ScenarioPreset::PulsarSystem,
        ),
        (
            KeyCode::F9,
            crate::simulation::scenarios::ScenarioPreset::MagnetarOutburst,
        ),
    ];
    for (key, preset) in preset_map {
        if keyboard.just_pressed(key) {
            scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(preset));
            break;
        }
    }
    if keyboard.just_pressed(KeyCode::F8) {
        config.enable_gpu_compute = !config.enable_gpu_compute;
        if !config.enable_gpu_compute {
            config.gpu_compute_active = false;
        }
        let status = if config.enable_gpu_compute {
            "⚡ GPU COMPUTE: Enabled (WGPU Compute Dispatch @ 100,000 Particles)"
        } else {
            "🖥️ CPU FALLBACK: Active (Rayon Multithreading)"
        };
        toast.message = status.to_string();
        toast.timer = 4.0;
    }
    if keyboard.just_pressed(KeyCode::F12) {
        if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
            if let Some(ref mut events) = load_events {
                events.write(crate::simulation::serialization::LoadSystemEvent {
                    filename: "saves/quicksave.json".to_string(),
                });
            }
            toast.message =
                "📂 Quick Loading solar system state [saves/quicksave.json]...".to_string();
            toast.timer = 4.0;
        } else {
            if let Some(ref mut events) = save_events {
                events.write(crate::simulation::serialization::SaveSystemEvent {
                    filename: "saves/quicksave.json".to_string(),
                });
            }
            toast.message = "💾 Quick Saved solar system state [saves/quicksave.json]".to_string();
            toast.timer = 4.0;
        }
    }
}

pub fn handle_theia_hotkey(
    keyboard: &ButtonInput<KeyCode>,
    mut theia_state: Option<&mut crate::simulation::accretion::TheiaImpactState>,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if keyboard.just_pressed(KeyCode::KeyM) {
        if let Some(ref mut state) = theia_state {
            state.manual_trigger_requested = true;
        }
        toast.message =
            "🌑 THEIA MOON COLLISION TRIGGERED // Intercept trajectory locked for Moon formation!"
                .to_string();
        toast.timer = 8.0;
    }
}
