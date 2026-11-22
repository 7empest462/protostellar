//! Dynamic Cosmic Hazards, Deep-Space Catastrophes, and Sensory Event Orchestration.

pub mod crisis_response;
pub mod spawner;
pub mod systems;
pub mod types;

use bevy::prelude::*;

pub use crisis_response::*;
pub use spawner::*;
pub use systems::*;
pub use types::*;

pub struct CosmicEventsPlugin;

impl Plugin for CosmicEventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CosmicEventManager>()
            .init_resource::<CrisisInterceptorPool>()
            .add_message::<TriggerCosmicEvent>()
            .add_message::<CrisisCountermeasureEvent>()
            .add_message::<CosmicEventStartedEvent>()
            .add_message::<CosmicEventResolvedEvent>()
            .add_systems(
                Update,
                (
                    systems::cosmic_event_scheduler_system,
                    crisis_response::handle_crisis_countermeasure_system
                        .after(systems::cosmic_event_scheduler_system),
                    systems::cosmic_event_lifecycle_system
                        .after(crisis_response::handle_crisis_countermeasure_system)
                        .after(crate::simulation::physics::step_physics_simulation)
                        .after(
                            crate::simulation::space_weather::update_planetary_auroral_ovals_system,
                        ),
                    systems::cosmic_event_sensory_system
                        .after(systems::cosmic_event_lifecycle_system),
                ),
            );
    }
}
