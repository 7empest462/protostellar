//! Interactive button click and hover dispatch systems.

pub mod builder;
pub mod campaign;
pub mod editor;
pub mod hazards;
pub mod scenarios;
pub mod selection;
pub mod system_tools;
pub mod time_and_panels;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::types::*;
use builder::handle_builder_action;
use campaign::handle_campaign_action;
use editor::handle_body_editor_action;
use hazards::handle_hazard_action;
use scenarios::handle_scenario_action;
use selection::handle_selection_action;
use system_tools::dispatch_system_and_tool_actions;

/// Primary UI resource state bundle for the HUD button interaction system.
#[derive(SystemParam)]
pub struct UiCoreState<'w> {
    pub time_warp: ResMut<'w, TimeWarp>,
    pub player_state: ResMut<'w, PlayerInteractionState>,
    pub toast: ResMut<'w, NotificationToast>,
    pub quick_bar_state: ResMut<'w, QuickBarState>,
    pub builder_state: ResMut<'w, PlanetBuilderState>,
    pub hud_visibility: ResMut<'w, HudVisibilityState>,
    pub telemetry_panel_state: ResMut<'w, TelemetryPanelState>,
    pub trophies_panel_state: ResMut<'w, crate::game::ui::types::TrophiesPanelState>,
    pub telemetry_history: ResMut<'w, crate::simulation::telemetry::SimulationTelemetryHistory>,
}

/// Optional tool and research resources bundled for clean system signatures.
#[derive(SystemParam)]
pub struct UiOptionalState<'w> {
    pub slingshot: Option<ResMut<'w, SlingshotState>>,
    pub predictor: Option<ResMut<'w, crate::simulation::predictor::TrajectoryPredictorState>>,
    pub scrubber: Option<ResMut<'w, crate::simulation::geology::types::TimelineScrubber>>,
    pub tech_tree: Option<ResMut<'w, crate::game::ui::types::TechTreePanelState>>,
    pub science: Option<ResMut<'w, crate::simulation::economy::ScienceData>>,
    pub techs: Option<ResMut<'w, crate::simulation::economy::UnlockedTechs>>,
    pub hazards_panel: Option<ResMut<'w, crate::game::ui::hazards_panel::HazardsPanelState>>,
    pub hazard_mgr: Option<ResMut<'w, crate::simulation::events::CosmicEventManager>>,
    pub hr_diagram: Option<ResMut<'w, HrDiagramPanelState>>,
    pub lagrange_config: Option<ResMut<'w, LagrangeOverlayConfig>>,
    pub hz_config: Option<ResMut<'w, HabitableZoneConfig>>,
}

/// Campaign state and mission event writer bundle.
#[derive(SystemParam)]
pub struct UiCampaignParams<'w> {
    pub panel: Option<ResMut<'w, crate::game::ui::types::CampaignPanelState>>,
    pub state: Option<ResMut<'w, crate::simulation::campaign::CampaignState>>,
    pub start_events:
        Option<MessageWriter<'w, crate::simulation::campaign::StartCampaignMissionEvent>>,
}

/// Simulation resources bundle for UI action handlers.
#[derive(SystemParam)]
pub struct UiSimulationContext<'w> {
    pub disk_params: Res<'w, DiskParameters>,
    pub config: ResMut<'w, SimulationConfig>,
    pub sim_time: Res<'w, SimTime>,
    pub lhb_state: ResMut<'w, crate::game::phases::LateHeavyBombardmentState>,
    pub theia_state: Option<ResMut<'w, crate::simulation::accretion::TheiaImpactState>>,
}

/// UI widget queries for button hover, click styling, and tooltip updates.
#[derive(SystemParam)]
#[allow(
    clippy::type_complexity,
    reason = "Complex Bevy ECS query for interactive HUD button components"
)]
pub struct UiButtonWidgets<'w, 's> {
    pub buttons: Query<
        'w,
        's,
        (
            Entity,
            &'static UiButtonAction,
            &'static mut BackgroundColor,
            &'static mut BorderColor,
            Option<&'static QuickBarButtonBaseColor>,
            Option<&'static bevy::picking::hover::Hovered>,
            Has<bevy::ui::Pressed>,
        ),
        With<HudButton>,
    >,
    pub tooltip_query: Query<'w, 's, &'static mut Text, With<HudActionTooltipText>>,
}

/// Serialization and clipboard message writers for system state persistence.
#[derive(SystemParam)]
pub struct UiSerializationWriters<'w> {
    pub scenario_events: MessageWriter<'w, crate::simulation::scenarios::LoadScenarioEvent>,
    pub save_events: Option<MessageWriter<'w, crate::simulation::serialization::SaveSystemEvent>>,
    pub load_events: Option<MessageWriter<'w, crate::simulation::serialization::LoadSystemEvent>>,
    pub copy_seed_events:
        Option<MessageWriter<'w, crate::simulation::serialization::CopySystemSeedClipboardEvent>>,
    pub paste_seed_events:
        Option<MessageWriter<'w, crate::simulation::serialization::PasteSystemSeedClipboardEvent>>,
    pub slot_state: Option<Res<'w, crate::game::ui::widgets::SaveSlotUiState>>,
}

fn handle_hover_interaction(
    hovered: bool,
    action: &UiButtonAction,
    border_color: &mut BorderColor,
    opt_base: Option<&QuickBarButtonBaseColor>,
    tooltip_query: &mut Query<&mut Text, With<HudActionTooltipText>>,
) {
    if hovered {
        *border_color = BorderColor::all(Color::srgb(0.4, 0.9, 1.0));
        if let Ok(mut tip) = tooltip_query.single_mut() {
            tip.0 = action.tooltip_description().to_string();
        }
    } else if let Some(base) = opt_base {
        *border_color = BorderColor::all(base.border.with_alpha(0.88));
    } else {
        *border_color = BorderColor::all(Color::srgba(0.3, 0.5, 0.8, 0.6));
    }
}

fn enforce_modal_exclusion_for_action(
    action: &UiButtonAction,
    core: &mut UiCoreState,
    optional: &mut UiOptionalState,
    campaign: &mut UiCampaignParams,
) {
    let mut states = crate::game::ui::ModalPanelStates {
        builder: Some(&mut core.builder_state),
        telemetry: Some(&mut core.telemetry_panel_state),
        trophies: Some(&mut core.trophies_panel_state),
        tech_tree: optional.tech_tree.as_deref_mut(),
        campaign: campaign.panel.as_deref_mut(),
        hazards: optional.hazards_panel.as_deref_mut(),
        scrubber: optional.scrubber.as_deref_mut(),
        hr_diagram: optional.hr_diagram.as_deref_mut(),
        visibility: Some(&mut core.hud_visibility),
    };
    states.enforce_mutual_exclusion_for_action(action);
}

#[allow(
    clippy::too_many_arguments,
    reason = "Dispatches pressed button action across subsystem handlers"
)]
fn dispatch_pressed_button_action(
    action: &UiButtonAction,
    core: &mut UiCoreState,
    save_events: Option<&mut MessageWriter<crate::simulation::serialization::SaveSystemEvent>>,
    load_events: Option<&mut MessageWriter<crate::simulation::serialization::LoadSystemEvent>>,
    copy_seed_events: Option<
        &mut MessageWriter<crate::simulation::serialization::CopySystemSeedClipboardEvent>,
    >,
    paste_seed_events: Option<
        &mut MessageWriter<crate::simulation::serialization::PasteSystemSeedClipboardEvent>,
    >,
    slot_name: &str,
    optional: &mut UiOptionalState,
    campaign: &mut UiCampaignParams,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    selected_query: &mut SelectedWorldQuery,
    config: &mut SimulationConfig,
    star_mass: f64,
    commands: &mut Commands,
    scenario_events: &mut MessageWriter<crate::simulation::scenarios::LoadScenarioEvent>,
    quasi_star_query: &mut Query<&mut BlackHoleStarState>,
    lhb_state: &mut ResMut<crate::game::phases::LateHeavyBombardmentState>,
    theia_state: Option<&mut crate::simulation::accretion::TheiaImpactState>,
    sim_time_years: f64,
) {
    if *action == UiButtonAction::CycleParticleDensity {
        config.particle_density = config.particle_density.cycle();
        config.target_particle_count = config.particle_density.particle_count() as usize;
        core.toast.message = format!(
            "✨ Particle Swarm Density: {}",
            config.particle_density.label()
        );
        core.toast.timer = 3.0;
        return;
    }

    if dispatch_system_and_tool_actions(
        action,
        save_events,
        load_events,
        copy_seed_events,
        paste_seed_events,
        slot_name,
        &mut core.quick_bar_state,
        &mut core.time_warp,
        &mut core.hud_visibility,
        &mut core.telemetry_panel_state,
        &mut core.trophies_panel_state,
        optional.tech_tree.as_deref_mut(),
        optional.science.as_deref_mut(),
        optional.techs.as_deref_mut(),
        &mut core.telemetry_history,
        optional.scrubber.as_deref_mut(),
        &mut core.player_state,
        optional.slingshot.as_deref_mut(),
        optional.predictor.as_deref_mut(),
        optional.hr_diagram.as_deref_mut(),
        optional.lagrange_config.as_deref_mut(),
        optional.hz_config.as_deref_mut(),
        &mut core.toast,
    ) {
        enforce_modal_exclusion_for_action(action, core, optional, campaign);
        return;
    }
    if handle_campaign_action(
        action,
        campaign.panel.as_deref_mut(),
        campaign.state.as_deref_mut(),
        campaign.start_events.as_mut(),
        optional.science.as_deref_mut(),
        &mut core.toast,
    ) {
        enforce_modal_exclusion_for_action(action, core, optional, campaign);
        return;
    }
    if handle_hazard_action(
        action,
        optional.hazards_panel.as_deref_mut(),
        optional.hazard_mgr.as_deref_mut(),
        &mut core.toast,
    ) {
        enforce_modal_exclusion_for_action(action, core, optional, campaign);
        return;
    }
    if handle_selection_action(
        action,
        &mut core.player_state,
        camera_query,
        selected_query,
        config,
        &mut core.toast,
        &mut core.quick_bar_state,
    ) {
        return;
    }
    if handle_spawn_oort_swarm_action(action, selected_query, star_mass, commands, core) {
        return;
    }
    if handle_builder_action(
        action,
        &mut core.builder_state,
        &mut core.toast,
        star_mass,
        &mut core.player_state,
        camera_query,
        selected_query,
        commands,
    ) {
        enforce_modal_exclusion_for_action(action, core, optional, campaign);
        return;
    }
    if handle_scenario_action(
        action,
        scenario_events,
        &mut core.toast,
        quasi_star_query,
        commands,
        &mut core.player_state,
        camera_query,
    ) {
        core.hud_visibility.scenarios_is_open = false;
        core.hud_visibility.scenarios_minimized = true;
        return;
    }
    handle_body_editor_action(
        action,
        &mut core.player_state,
        selected_query,
        camera_query,
        star_mass,
        commands,
        &mut core.toast,
        lhb_state,
        theia_state,
        sim_time_years,
    );
}

fn handle_spawn_oort_swarm_action(
    action: &UiButtonAction,
    selected_query: &SelectedWorldQuery,
    star_mass: f64,
    commands: &mut Commands,
    core: &mut UiCoreState,
) -> bool {
    if *action != UiButtonAction::SpawnOortCloudSwarm {
        return false;
    }
    let mut s_pos = bevy::math::DVec3::ZERO;
    for (_, _, _, p, _, _, b, s, ..) in selected_query.iter() {
        if s.is_some() || b.body_type.is_star_or_remnant() {
            s_pos = p.0;
            break;
        }
    }
    let oort_config = crate::simulation::oort::OortCloudConfig::default();
    let mut rng = fastrand::Rng::new();
    crate::simulation::oort::spawn_oort_cloud_swarm(
        commands,
        &oort_config,
        if star_mass > 0.0 { star_mass } else { 1.0 },
        s_pos,
        48,
        &mut rng,
    );
    core.quick_bar_state.show_minor_bodies = true;
    core.quick_bar_state
        .expanded_belts
        .insert(crate::simulation::disk::belts::BeltZone::OortCloud);
    core.toast.message =
        "☄️ OORT CLOUD SWARM // Procedurally spawned 48 cometary bodies into Hills & Outer Oort reservoirs (200 - 25,000 AU)!"
            .to_string();
    core.toast.timer = 6.0;
    true
}

/// Handles interactive clicks, tab navigation, and hover highlights on all on-screen HUD buttons.
pub fn handle_ui_button_interactions(
    mut widgets: UiButtonWidgets,
    mut activated_buttons: Option<ResMut<crate::game::ui::widgets::ActivatedHudButtons>>,
    mut core: UiCoreState,
    mut sim: UiSimulationContext,
    mut selected_query: SelectedWorldQuery,
    mut camera_query: Query<&mut PanOrbitCamera>,
    mut ser: UiSerializationWriters,
    mut commands: Commands,
    mut quasi_star_query: Query<&mut BlackHoleStarState>,
    mut optional: UiOptionalState,
    mut campaign: UiCampaignParams,
) {
    let star_mass = sim.disk_params.central_star_mass;
    let mut actions_to_dispatch = Vec::new();

    for (ent, action, mut bg_color, mut border_color, opt_base, opt_hovered, pressed) in
        widgets.buttons.iter_mut()
    {
        let is_hovered = opt_hovered.is_some_and(bevy::picking::hover::Hovered::get);

        if pressed {
            *bg_color = BackgroundColor(Color::srgba(0.2, 0.5, 0.9, 0.95));
            *border_color = BorderColor::all(Color::srgb(1.0, 1.0, 1.0));
            if activated_buttons.is_none() {
                actions_to_dispatch.push((ent, *action));
            }
        } else {
            handle_hover_interaction(
                is_hovered,
                action,
                &mut border_color,
                opt_base,
                &mut widgets.tooltip_query,
            );

            if !is_hovered {
                let is_active_selectable = match action {
                    UiButtonAction::SelectInspectorTab(tab) => {
                        *tab == core.hud_visibility.active_inspector_tab
                    }
                    _ => false,
                };
                if !is_active_selectable {
                    if let Some(base) = opt_base {
                        *bg_color = BackgroundColor(base.bg);
                        *border_color = BorderColor::all(base.border.with_alpha(0.88));
                    } else {
                        *border_color = BorderColor::all(Color::srgba(0.3, 0.5, 0.8, 0.6));
                    }
                }
            }
        }
    }

    if let Some(ref mut activated) = activated_buttons {
        for ent in activated.0.drain(..) {
            if let Ok((_, action, ..)) = widgets.buttons.get(ent) {
                actions_to_dispatch.push((ent, *action));
            }
        }
    } else {
        for (ent, _) in &actions_to_dispatch {
            commands.entity(*ent).remove::<bevy::ui::Pressed>();
        }
    }

    let default_slot = "quicksave".to_string();
    let slot_name = ser
        .slot_state
        .as_ref()
        .map_or(default_slot.as_str(), |s| s.slot_name.as_str());

    for (_ent, action) in actions_to_dispatch {
        dispatch_pressed_button_action(
            &action,
            &mut core,
            ser.save_events.as_mut(),
            ser.load_events.as_mut(),
            ser.copy_seed_events.as_mut(),
            ser.paste_seed_events.as_mut(),
            slot_name,
            &mut optional,
            &mut campaign,
            &mut camera_query,
            &mut selected_query,
            &mut sim.config,
            star_mass,
            &mut commands,
            &mut ser.scenario_events,
            &mut quasi_star_query,
            &mut sim.lhb_state,
            sim.theia_state.as_deref_mut(),
            sim.sim_time.elapsed_years,
        );
    }
}
