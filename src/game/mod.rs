//! Game Plugin bundling phase state machine, player tools, time controls, and HUD overlay.

pub mod interaction;
pub mod phases;
pub mod slingshot;
pub mod time_control;
pub mod ui;

use bevy::prelude::*;

use crate::game::interaction::*;
use crate::game::phases::*;
use crate::game::slingshot::*;
use crate::game::time_control::*;
use crate::game::ui::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<SystemPhase>()
            .init_resource::<PhaseManager>()
            .init_resource::<phases::GlobalTrophies>()
            .init_resource::<LateHeavyBombardmentState>()
            .init_resource::<crate::simulation::accretion::TheiaImpactState>()
            .init_resource::<QuickBarState>()
            .init_resource::<PlanetBuilderState>()
            .init_resource::<TelemetryPanelState>()
            .init_resource::<crate::game::ui::types::TrophiesPanelState>()
            .init_resource::<crate::game::ui::types::TechTreePanelState>()
            .init_resource::<crate::game::ui::types::CampaignPanelState>()
            .init_resource::<crate::game::ui::hazards_panel::HazardsPanelState>()
            .init_resource::<crate::game::ui::types::HrDiagramPanelState>()
            .init_resource::<HudVisibilityState>()
            .add_plugins(SciFiUiThemePlugin)
            .add_systems(Startup, setup_hud)
            .add_systems(
                Update,
                (
                    monitor_phase_transitions,
                    handle_time_control_input,
                    handle_player_tools,
                    handle_planet_builder_click_spawn,
                    handle_slingshot_input,
                    handle_ui_button_interactions
                        .before(crate::rendering::camera::update_pan_orbit_camera),
                    update_quick_body_selector_bar.after(handle_ui_button_interactions),
                    update_quick_bar_highlights.after(update_quick_body_selector_bar),
                ),
            )
            .add_systems(
                Update,
                (
                    update_planet_builder_ui,
                    update_telemetry_graph_ui,
                    ui::trophies_panel::update_trophies_panel,
                    ui::tech_tree_panel::update_tech_tree_panel,
                    update_campaign_ui,
                    update_hazards_ui,
                    update_epoch_scrubber_ui,
                    update_hr_diagram_ui,
                    handle_roche_disruption_toasts,
                ),
            )
            .add_systems(
                Update,
                (
                    handle_inspector_scroll,
                    reset_inspector_scroll_on_target_change,
                    update_hud_visibility,
                    update_scenario_contextual_ui,
                    update_hud,
                    ui::hover_card::update_hover_card_ui,
                ),
            );
    }
}
