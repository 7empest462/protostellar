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

fn handle_hover_interaction(
    interaction: &Interaction,
    action: &UiButtonAction,
    border_color: &mut BorderColor,
    opt_base: Option<&QuickBarButtonBaseColor>,
    tooltip_query: &mut Query<&mut Text, With<HudActionTooltipText>>,
) {
    match *interaction {
        Interaction::Hovered => {
            *border_color = BorderColor::all(Color::srgb(0.4, 0.9, 1.0));
            if let Ok(mut tip) = tooltip_query.single_mut() {
                tip.0 = action.tooltip_description().to_string();
            }
        }
        Interaction::None => {
            if let Some(base) = opt_base {
                *border_color = BorderColor::all(base.border.with_alpha(0.88));
            } else {
                *border_color = BorderColor::all(Color::srgba(0.3, 0.5, 0.8, 0.6));
            }
        }
        Interaction::Pressed => {}
    }
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
    optional: &mut UiOptionalState,
    campaign: &mut UiCampaignParams,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    selected_query: &mut SelectedWorldQuery,
    config: &SimulationConfig,
    star_mass: f64,
    commands: &mut Commands,
    scenario_events: &mut MessageWriter<crate::simulation::scenarios::LoadScenarioEvent>,
    quasi_star_query: &mut Query<&mut BlackHoleStarState>,
    lhb_state: &mut ResMut<crate::game::phases::LateHeavyBombardmentState>,
    theia_state: Option<&mut crate::simulation::accretion::TheiaImpactState>,
    sim_time_years: f64,
    rng: &mut rand::rngs::ThreadRng,
) {
    if dispatch_system_and_tool_actions(
        action,
        save_events,
        load_events,
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
        return;
    }
    if handle_hazard_action(
        action,
        optional.hazards_panel.as_deref_mut(),
        optional.hazard_mgr.as_deref_mut(),
        &mut core.toast,
    ) {
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
        rng,
    );
}

/// Handles interactive mouse clicks and hover highlights on all on-screen HUD buttons.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "HUD button interaction system dispatches across simulation controls and views"
)]
pub fn handle_ui_button_interactions(
    mut interaction_query: Query<
        (
            &Interaction,
            &UiButtonAction,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&QuickBarButtonBaseColor>,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut tooltip_query: Query<&mut Text, With<HudActionTooltipText>>,
    mut core: UiCoreState,
    disk_params: Res<DiskParameters>,
    config: Res<SimulationConfig>,
    mut selected_query: SelectedWorldQuery,
    mut camera_query: Query<&mut PanOrbitCamera>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    mut theia_state: Option<ResMut<crate::simulation::accretion::TheiaImpactState>>,
    (mut scenario_events, mut save_events, mut load_events): (
        MessageWriter<crate::simulation::scenarios::LoadScenarioEvent>,
        Option<MessageWriter<crate::simulation::serialization::SaveSystemEvent>>,
        Option<MessageWriter<crate::simulation::serialization::LoadSystemEvent>>,
    ),
    sim_time: Res<SimTime>,
    mut commands: Commands,
    mut quasi_star_query: Query<&mut BlackHoleStarState>,
    mut optional: UiOptionalState,
    mut campaign: UiCampaignParams,
) {
    let mut rng = rand::rng();
    let star_mass = disk_params.central_star_mass;

    for (interaction, action, mut bg_color, mut border_color, opt_base) in
        interaction_query.iter_mut()
    {
        handle_hover_interaction(
            interaction,
            action,
            &mut border_color,
            opt_base,
            &mut tooltip_query,
        );

        if *interaction == Interaction::None {
            if let Some(base) = opt_base {
                *bg_color = BackgroundColor(base.bg);
            }
        }

        if *interaction == Interaction::Pressed {
            *bg_color = BackgroundColor(Color::srgba(0.2, 0.5, 0.9, 0.95));
            *border_color = BorderColor::all(Color::srgb(1.0, 1.0, 1.0));

            dispatch_pressed_button_action(
                action,
                &mut core,
                save_events.as_mut(),
                load_events.as_mut(),
                &mut optional,
                &mut campaign,
                &mut camera_query,
                &mut selected_query,
                &config,
                star_mass,
                &mut commands,
                &mut scenario_events,
                &mut quasi_star_query,
                &mut lhb_state,
                theia_state.as_deref_mut(),
                sim_time.elapsed_years,
                &mut rng,
            );
        }
    }
}
