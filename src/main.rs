//! Protostellar — Application Entry Point.

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use protostellar::game::GamePlugin;
use protostellar::rendering::RenderingPlugin;
use protostellar::simulation::SimulationPlugin;

fn main() {
    App::new()
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
        .add_plugins((SimulationPlugin, RenderingPlugin, GamePlugin))
        .run();
}
