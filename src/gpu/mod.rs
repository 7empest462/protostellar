//! GPU Acceleration, custom WGPU compute passes, and instanced particle rendering.

pub mod buffers;
pub mod compute_node;
pub mod gmc_collapse;
pub mod gmc_far_field;
pub mod gmc_fluid;
pub mod nbody_galaxy;
pub mod particle_pipeline;

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, Render, RenderApp};

use crate::gpu::compute_node::*;
use crate::gpu::particle_pipeline::*;

/// Sender resource inserted into the RenderApp world.
#[derive(Resource)]
pub struct GpuReadbackSender {
    pub tx: flume::Sender<Vec<u8>>,
    pub recycle_rx: flume::Receiver<Vec<u8>>,
}

/// Plugin registering GPU computing resources, compute pipelines, and instanced particle rendering.
pub struct GpuSimPlugin;

impl Plugin for GpuSimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ParticleRenderPlugin,
            gmc_fluid::GmcFluidPlugin,
            gmc_far_field::GmcFarFieldPlugin,
        ));

        // Create cross-world readback channel and recycling channel
        let (tx, rx) = flume::bounded::<Vec<u8>>(2);
        let (recycle_tx, recycle_rx) = flume::bounded::<Vec<u8>>(2);

        // Main-world: insert receiver + readback system
        app.insert_resource(GpuReadbackReceiver { rx, recycle_tx });
        app.add_systems(Update, receive_gpu_readback);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        // RenderApp: insert sender + compute systems
        render_app.insert_resource(GpuReadbackSender { tx, recycle_rx });
        render_app
            .add_systems(
                ExtractSchedule,
                (
                    extract_gpu_sim_data,
                    crate::gpu::nbody_galaxy::extract_galaxy_sim_data,
                ),
            )
            .add_systems(
                Render,
                (
                    step_gpu_simulation_render_world,
                    crate::gpu::nbody_galaxy::step_galaxy_simulation_render_world,
                ),
            );
    }
}

/// Creates a standard 2-buffer compute pipeline (Storage buffer at binding 0, Uniform buffer at binding 1).
pub fn create_two_buffer_compute_pipeline(
    device: &wgpu::Device,
    shader_src: &str,
    module_label: &str,
    layout_label: &str,
    pipeline_label: &str,
) -> (wgpu::BindGroupLayout, wgpu::ComputePipeline) {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(module_label),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(layout_label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{pipeline_label} Layout")),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(pipeline_label),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    (bind_group_layout, pipeline)
}
