//! GPU Acceleration, custom WGPU compute passes, and instanced particle rendering.

pub mod buffers;
pub mod compute_node;
pub mod particle_pipeline;

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, Render, RenderApp};

use crate::gpu::compute_node::*;
use crate::gpu::particle_pipeline::*;

/// Sender resource inserted into the RenderApp world.
#[derive(Resource)]
pub struct GpuReadbackSender {
    pub tx: flume::Sender<Vec<u8>>,
}

/// Plugin registering GPU computing resources, compute pipelines, and instanced particle rendering.
pub struct GpuSimPlugin;

impl Plugin for GpuSimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ParticleRenderPlugin);

        // Create cross-world readback channel
        let (tx, rx) = flume::bounded::<Vec<u8>>(2);

        // Main-world: insert receiver + readback system
        app.insert_resource(GpuReadbackReceiver { rx });
        app.add_systems(Update, receive_gpu_readback);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        // RenderApp: insert sender + compute systems
        render_app.insert_resource(GpuReadbackSender { tx });
        render_app
            .add_systems(ExtractSchedule, extract_gpu_sim_data)
            .add_systems(Render, step_gpu_simulation_render_world);
    }
}
