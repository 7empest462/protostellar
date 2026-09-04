//! Game Plugin bundling phase state machine, player tools, time controls, and HUD overlay.

pub mod interaction;
pub mod phases;
pub mod time_control;
pub mod ui;

use bevy::prelude::*;

use crate::game::interaction::*;
use crate::game::phases::*;
use crate::game::time_control::*;
use crate::game::ui::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<SystemPhase>()
            .init_resource::<PhaseManager>()
            .init_resource::<LateHeavyBombardmentState>()
            .init_resource::<QuickBarState>()
            .init_resource::<PlanetBuilderState>()
            .add_systems(Startup, setup_hud)
            .add_systems(
                Update,
                (
                    monitor_phase_transitions,
                    handle_time_control_input,
                    handle_player_tools,
                    handle_planet_builder_click_spawn,
                    handle_ui_button_interactions,
                    update_quick_body_selector_bar,
                    update_planet_builder_ui,
                    handle_roche_disruption_toasts,
                    update_hud,
                ),
            );
    }
}
