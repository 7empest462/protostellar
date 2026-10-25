//! Campaign mode, scenario objectives, and mission progress evaluation.

pub mod missions;
pub mod systems;
pub mod types;

use bevy::prelude::*;

pub use missions::*;
pub use systems::*;
pub use types::*;

pub struct CampaignPlugin;

impl Plugin for CampaignPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CampaignState>()
            .add_message::<StartCampaignMissionEvent>()
            .add_message::<CampaignMissionCompletedEvent>()
            .add_message::<CampaignMissionFailedEvent>()
            .add_systems(
                Update,
                (
                    handle_start_campaign_mission,
                    evaluate_campaign_objectives
                        .after(crate::simulation::physics::step_physics_simulation)
                        .after(crate::simulation::thermodynamics::update_thermodynamics),
                ),
            );
    }
}
