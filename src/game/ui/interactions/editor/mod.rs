//! Live celestial body inspector and parameter manipulation action dispatcher.

pub mod orbital_params;
pub mod stellar;
pub mod terraforming_actions;

pub use orbital_params::*;
pub use stellar::*;
pub use terraforming_actions::*;

use bevy::prelude::*;
use rand::prelude::*;

use crate::game::ui::types::*;
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::resources::*;

#[allow(
    clippy::too_many_arguments,
    reason = "Comprehensive body editor dispatcher requires all relevant ECS world handles and simulation resources"
)]
pub fn handle_body_editor_action(
    action: &UiButtonAction,
    player_state: &mut PlayerInteractionState,
    selected_query: &mut SelectedWorldQuery,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    star_mass: f64,
    commands: &mut Commands,
    toast: &mut NotificationToast,
    lhb_state: &mut ResMut<crate::game::phases::LateHeavyBombardmentState>,
    theia_state: Option<&mut crate::simulation::accretion::TheiaImpactState>,
    sim_time_years: f64,
    _rng: &mut impl Rng,
) -> bool {
    if handle_mass_change(action, selected_query, player_state, toast) {
        return true;
    }
    if handle_orbital_adjustment(action, selected_query, player_state, star_mass, toast) {
        return true;
    }
    if handle_velocity_burn_and_fix(action, selected_query, player_state, star_mass, toast) {
        return true;
    }
    if handle_view_and_target(
        action,
        player_state,
        camera_query,
        selected_query,
        commands,
        toast,
    ) {
        return true;
    }
    if handle_hydrosphere_action(action, selected_query, player_state, commands, toast) {
        return true;
    }

    match action {
        UiButtonAction::InjectEmbryo => {
            if let Some(state) = theia_state {
                state.manual_trigger_requested = true;
            }
            toast.message =
                "🌑 THEIA MOON COLLISION TRIGGERED // Intercept trajectory locked for Moon formation!"
                    .to_string();
            toast.timer = 8.0;
            true
        }
        UiButtonAction::IgniteStar => {
            handle_ignite_star(selected_query, toast);
            true
        }
        UiButtonAction::TriggerCoronalMassEjection => {
            handle_trigger_cme(selected_query, commands, toast);
            true
        }
        UiButtonAction::TriggerLhb => {
            lhb_state.is_active = true;
            lhb_state.manual_trigger_requested = true;
            toast.message =
                "☄️ LATE HEAVY BOMBARDMENT TRIGGERED // Cometary ocean-seeding shower inbound!"
                    .to_string();
            toast.timer = 8.0;
            true
        }
        UiButtonAction::AgeStar => {
            handle_age_star(selected_query, toast);
            true
        }
        UiButtonAction::TidalLock => {
            handle_tidal_lock(selected_query, player_state, star_mass, commands, toast);
            true
        }
        UiButtonAction::AccelerateInspiral => {
            handle_accelerate_inspiral(selected_query, player_state, toast);
            true
        }
        UiButtonAction::StripAtmosphere => {
            handle_strip_atmosphere(selected_query, player_state, commands, toast);
            true
        }
        UiButtonAction::BombardComet
        | UiButtonAction::BombardChondrite
        | UiButtonAction::BombardSalvo
        | UiButtonAction::BombardCore => {
            handle_bombardment_trigger(
                action,
                selected_query,
                player_state,
                commands,
                sim_time_years,
                toast,
            );
            true
        }
        _ => handle_shatter_and_life(
            action,
            selected_query,
            player_state,
            commands,
            sim_time_years,
            toast,
        ),
    }
}
