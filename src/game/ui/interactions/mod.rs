//! Interactive button click and hover dispatch systems.

pub mod builder;
pub mod editor;
pub mod scenarios;
pub mod selection;
pub mod time_and_panels;

use bevy::prelude::*;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::telemetry_panel::handle_telemetry_action;
use super::types::*;
use builder::handle_builder_action;
use editor::handle_body_editor_action;
use scenarios::handle_scenario_action;
use selection::handle_selection_action;
use time_and_panels::{handle_instrument_action, handle_panel_toggle_action, handle_time_action};

fn handle_hover_interaction(
    interaction: &Interaction,
    action: &UiButtonAction,
    border_color: &mut BorderColor,
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
            *border_color = BorderColor::all(Color::srgba(0.3, 0.5, 0.8, 0.6));
        }
        Interaction::Pressed => {}
    }
}

fn handle_save_load_action(
    action: &UiButtonAction,
    save_events: Option<&mut MessageWriter<crate::simulation::serialization::SaveSystemEvent>>,
    load_events: Option<&mut MessageWriter<crate::simulation::serialization::LoadSystemEvent>>,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::QuickSave => {
            if let Some(writer) = save_events {
                writer.write(crate::simulation::serialization::SaveSystemEvent {
                    filename: "saves/quicksave.json".to_string(),
                });
            }
            toast.message = "💾 Quick Saved solar system state [saves/quicksave.json]".to_string();
            toast.timer = 4.0;
            true
        }
        UiButtonAction::QuickLoad => {
            if let Some(writer) = load_events {
                writer.write(crate::simulation::serialization::LoadSystemEvent {
                    filename: "saves/quicksave.json".to_string(),
                });
            }
            toast.message =
                "📂 Quick Loading solar system state [saves/quicksave.json]...".to_string();
            toast.timer = 4.0;
            true
        }
        _ => false,
    }
}

/// Handles interactive mouse clicks and hover highlights on all on-screen HUD buttons.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "HUD button interaction system dispatches across all active simulation controls, cameras, and states"
)]
pub fn handle_ui_button_interactions(
    mut interaction_query: Query<
        (
            &Interaction,
            &UiButtonAction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut tooltip_query: Query<&mut Text, With<HudActionTooltipText>>,
    (
        mut time_warp,
        mut player_state,
        mut toast,
        mut quick_bar_state,
        mut builder_state,
        mut hud_visibility,
        mut telemetry_panel_state,
        mut telemetry_history,
    ): (
        ResMut<TimeWarp>,
        ResMut<PlayerInteractionState>,
        ResMut<NotificationToast>,
        ResMut<QuickBarState>,
        ResMut<PlanetBuilderState>,
        ResMut<HudVisibilityState>,
        ResMut<TelemetryPanelState>,
        ResMut<crate::simulation::telemetry::SimulationTelemetryHistory>,
    ),
    disk_params: Res<DiskParameters>,
    config: Res<SimulationConfig>,
    mut selected_query: SelectedWorldQuery,
    mut camera_query: Query<&mut PanOrbitCamera>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    mut theia_state: Option<ResMut<crate::simulation::accretion::TheiaImpactState>>,
    mut scenario_events: MessageWriter<crate::simulation::scenarios::LoadScenarioEvent>,
    mut save_events: Option<MessageWriter<crate::simulation::serialization::SaveSystemEvent>>,
    mut load_events: Option<MessageWriter<crate::simulation::serialization::LoadSystemEvent>>,
    sim_time: Res<SimTime>,
    mut commands: Commands,
    mut quasi_star_query: Query<&mut BlackHoleStarState>,
    mut opt_slingshot: Option<ResMut<SlingshotState>>,
) {
    let mut rng = rand::rng();
    let star_mass = disk_params.central_star_mass;

    for (interaction, action, mut bg_color, mut border_color) in interaction_query.iter_mut() {
        handle_hover_interaction(interaction, action, &mut border_color, &mut tooltip_query);

        if *interaction == Interaction::Pressed {
            *bg_color = BackgroundColor(Color::srgba(0.2, 0.5, 0.9, 0.95));
            *border_color = BorderColor::all(Color::srgb(1.0, 1.0, 1.0));

            if handle_save_load_action(
                action,
                save_events.as_mut(),
                load_events.as_mut(),
                &mut toast,
            ) {
                continue;
            }

            if handle_time_action(action, &mut time_warp, &mut toast) {
                continue;
            }
            if handle_panel_toggle_action(action, &mut hud_visibility, &mut toast) {
                continue;
            }
            if handle_telemetry_action(
                action,
                &mut telemetry_panel_state,
                &mut telemetry_history,
                &mut toast,
            ) {
                continue;
            }
            if handle_instrument_action(
                action,
                &mut player_state,
                &mut toast,
                opt_slingshot.as_deref_mut(),
            ) {
                continue;
            }
            if handle_selection_action(
                action,
                &mut player_state,
                &mut camera_query,
                &selected_query,
                &config,
                &mut toast,
                &mut quick_bar_state,
            ) {
                continue;
            }
            if handle_builder_action(
                action,
                &mut builder_state,
                &mut toast,
                star_mass,
                &mut player_state,
                &mut camera_query,
                &selected_query,
                &mut commands,
            ) {
                continue;
            }
            if handle_scenario_action(
                action,
                &mut scenario_events,
                &mut toast,
                &mut quasi_star_query,
                &mut commands,
                &mut player_state,
                &mut camera_query,
            ) {
                continue;
            }
            handle_body_editor_action(
                action,
                &mut player_state,
                &mut selected_query,
                &mut camera_query,
                star_mass,
                &mut commands,
                &mut toast,
                &mut lhb_state,
                theia_state.as_deref_mut(),
                sim_time.elapsed_years,
                &mut rng,
            );
        }
    }
}
