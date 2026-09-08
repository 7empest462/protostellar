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

use super::types::*;
use builder::handle_builder_action;
use editor::handle_body_editor_action;
use scenarios::handle_scenario_action;
use selection::handle_selection_action;
use time_and_panels::{handle_instrument_action, handle_panel_toggle_action, handle_time_action};

/// Handles interactive mouse clicks and hover highlights on all on-screen HUD buttons.
#[allow(
    clippy::too_many_arguments,
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
    ): (
        ResMut<TimeWarp>,
        ResMut<PlayerInteractionState>,
        ResMut<NotificationToast>,
        ResMut<QuickBarState>,
        ResMut<PlanetBuilderState>,
        ResMut<HudVisibilityState>,
    ),
    disk_params: Res<DiskParameters>,
    config: Res<SimulationConfig>,
    mut selected_query: SelectedWorldQuery,
    mut camera_query: Query<&mut PanOrbitCamera>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    mut scenario_events: MessageWriter<crate::simulation::scenarios::LoadScenarioEvent>,
    sim_time: Res<SimTime>,
    mut commands: Commands,
    mut quasi_star_query: Query<&mut BlackHoleStarState>,
) {
    let mut rng = rand::rng();
    let star_mass = disk_params.central_star_mass;

    for (interaction, action, mut bg_color, mut border_color) in interaction_query.iter_mut() {
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
            Interaction::Pressed => {
                *bg_color = BackgroundColor(Color::srgba(0.2, 0.5, 0.9, 0.95));
                *border_color = BorderColor::all(Color::srgb(1.0, 1.0, 1.0));

                if handle_time_action(action, &mut time_warp, &mut toast) {
                    continue;
                }
                if handle_panel_toggle_action(action, &mut hud_visibility, &mut toast) {
                    continue;
                }
                if handle_instrument_action(action, &mut player_state, &mut toast) {
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
                    sim_time.elapsed_years,
                    &mut rng,
                );
            }
        }
    }
}
