//! Protostellar — Application Entry Point.

use bevy::ecs::error::{warn as bevy_warn_handler, FallbackErrorHandler};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use protostellar::game::GamePlugin;
use protostellar::gpu::GpuSimPlugin;
use protostellar::rendering::RenderingPlugin;
use protostellar::simulation::SimulationPlugin;

fn main() {
    App::new()
        // Project-wide error policy: downgrade all entity-command errors (e.g. inserting
        // on a just-despawned entity) to WARN instead of panicking. Individual hot-paths
        // also use `.try_insert()` explicitly for clarity, but this acts as the final
        // safety net for any future call sites.
        .insert_resource(FallbackErrorHandler(bevy_warn_handler))
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
