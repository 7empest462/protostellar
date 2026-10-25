//! Dynamic Cosmic Hazards, Deep-Space Catastrophes, and Sensory Event Orchestration.

pub mod spawner;
pub mod systems;
pub mod types;

use bevy::prelude::*;

pub use spawner::*;
pub use systems::*;
pub use types::*;

pub struct CosmicEventsPlugin;

impl Plugin for CosmicEventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CosmicEventManager>()
            .add_message::<TriggerCosmicEvent>()
            .add_message::<CosmicEventStartedEvent>()
            .add_message::<CosmicEventResolvedEvent>()
            .add_systems(
                Update,
                (
                    systems::cosmic_event_scheduler_system,
                    systems::cosmic_event_lifecycle_system
                        .after(systems::cosmic_event_scheduler_system)
                        .after(crate::simulation::physics::step_physics_simulation),
                    systems::cosmic_event_sensory_system
                        .after(systems::cosmic_event_lifecycle_system),
                ),
            );
    }
}
