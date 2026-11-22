//! Science economy, telemetry harvesting, and technology tree progression.

pub mod harvest;
pub mod progression;
pub mod types;

use bevy::prelude::*;

pub use harvest::*;
pub use progression::*;
pub use types::*;

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScienceData>()
            .init_resource::<UnlockedTechs>()
            .init_resource::<ProgressionState>()
            .add_systems(
                Update,
                (
                    harvest::harvest_telemetry_system
                        .after(crate::simulation::probes::navigation::probe_navigation_system),
                    harvest::sync_milestone_discoveries_system,
                ),
            );
    }
}
