//! Protostellar — Application Entry Point.

use bevy::ecs::error::{BevyError, ErrorContext, FallbackErrorHandler};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use protostellar::game::GamePlugin;
use protostellar::gpu::GpuSimPlugin;
use protostellar::rendering::RenderingPlugin;
use protostellar::simulation::SimulationPlugin;

fn protostellar_error_handler(error: BevyError, ctx: ErrorContext) {
    let msg = error.to_string();
    if msg.contains("Entity despawned") || msg.contains("is invalid; its index now has generation") {
        bevy::log::debug!("Benign accretion collision entity despawn: {error} ({ctx})");
    } else {
        bevy::ecs::error::warn(error, ctx);
    }
}

fn main() {
    App::new()
        // Project-wide error policy: downgrade all entity-command errors (e.g. inserting
        // on a just-despawned entity) to DEBUG instead of panicking or polluting the console.
        .insert_resource(FallbackErrorHandler(protostellar_error_handler))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "PROTOSTELLAR // Solar System Formation Simulator".into(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: false,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin { ..default() }),
        )
        .add_plugins((SimulationPlugin, RenderingPlugin, GamePlugin, GpuSimPlugin))
        .run();
}
