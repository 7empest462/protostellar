//! Deep Geological Time, Continental Drift, and Epoch Scrubbing module.

pub mod systems;
pub mod types;

use bevy::prelude::*;

pub use systems::{
    apply_epoch_to_world, calculate_continental_drift_phase, calculate_ocean_oxidation_progress,
    calculate_oxygen_level_pal, calculate_supercontinent_aggregation,
    calculate_vegetation_expansion, sync_geological_evolution_system,
};
pub use types::{EpochTargetPlanet, GeologicalEpoch, GeologicalState, TimelineScrubber};

/// Plugin managing deep-time geological evolution, continental drift, and timeline scrubbing.
pub struct GeologyPlugin;

impl Plugin for GeologyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimelineScrubber>()
            .add_systems(Update, sync_geological_evolution_system);
    }
}
