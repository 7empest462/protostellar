//! GPU-to-GPU 3D volume texture packing for the live `96^3` Giant Molecular Cloud fluid grid.
//!
//! Copies the active ping-pong `density_buffers`, `velocity_buffers`, and `temperature_buffer`
//! directly into the persistent `Rgba16Float` 3D `Image` bound by `VolumetricNebulaMaterial`
//! with zero CPU readback.

use bevy::asset::AssetId;
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::render_asset::RenderAssets;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;

use crate::gpu::gmc_fluid::{GmcFluidExtractedParams, GpuGmcFluidEngine};
use crate::rendering::volumetric_nebula::GmcLiveVolumeHandle;

/// Render-world handle tracking the persistent `96^3` 3D texture asset bound to the nebula shader.
#[derive(Resource, Clone, Copy)]
pub struct ExtractedGmcLiveVolume(pub AssetId<Image>);

/// Cached GPU compute pipeline for packing GMC storage buffers into the `96^3` 3D texture.
#[derive(Resource)]
pub struct GmcVolumePackPipeline {
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    last_packed_ping_pong: usize,
    last_packed_epoch: u64,
    last_packed_elapsed: f64,
    last_volume_id: Option<AssetId<Image>>,
}

/// Extracts [`GmcLiveVolumeHandle`] from the Main World into the Render World.
pub fn extract_gmc_live_volume(
    mut commands: Commands,
    volume_handle: Extract<Option<Res<GmcLiveVolumeHandle>>>,
) {
    if let Some(handle) = volume_handle.as_ref() {
        commands.insert_resource(ExtractedGmcLiveVolume(handle.0.id()));
    } else {
        commands.remove_resource::<ExtractedGmcLiveVolume>();
    }
}

fn create_gmc_volume_pack_pipeline(device: &wgpu::Device) -> GmcVolumePackPipeline {
    let shader_src = include_str!("../../assets/shaders/gmc_volume_pack.wgsl");
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("GMC Volume Pack Compute Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GMC Volume Pack Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D3,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("GMC Volume Pack Pipeline Layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("GMC Volume Pack Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: default(),
        cache: None,
    });

    GmcVolumePackPipeline {
        bind_group_layout,
        pipeline,
        last_packed_ping_pong: usize::MAX,
        last_packed_epoch: u64::MAX,
        last_packed_elapsed: f64::NAN,
        last_volume_id: None,
    }
}

/// Dispatches the GPU-to-GPU pack pass writing the live `96^3` GMC fluid buffers directly into
/// the 3D `Rgba16Float` texture bound to `VolumetricNebulaMaterial`.
#[allow(
    clippy::too_many_arguments,
    reason = "Render-world GMC volume texture pack system"
)]
pub fn pack_gmc_live_volume_texture(
    mut commands: Commands,
    pack_state_opt: Option<ResMut<GmcVolumePackPipeline>>,
    engine_opt: Option<Res<GpuGmcFluidEngine>>,
    params: Res<GmcFluidExtractedParams>,
    volume_opt: Option<Res<ExtractedGmcLiveVolume>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    render_dev: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    if !params.is_active {
        return;
    }
    let (Some(engine), Some(volume)) = (engine_opt, volume_opt) else {
        return;
    };
    let Some(gpu_image) = gpu_images.get(volume.0) else {
        return;
    };

    let device = render_dev.wgpu_device();
    let Some(mut pack_state) = pack_state_opt else {
        commands.insert_resource(create_gmc_volume_pack_pipeline(device));
        return;
    };

    // Skip redundant volume repacking when paused and the active fluid state is unchanged
    let already_synced = pack_state.last_volume_id == Some(volume.0)
        && pack_state.last_packed_ping_pong == engine.ping_pong_idx
        && pack_state.last_packed_epoch == engine.restart_epoch
        && (pack_state.last_packed_elapsed - engine.last_elapsed_years).abs() <= 1e-9;
    if params.is_paused && already_synced {
        return;
    }

    let (Some(density_buf), Some(velocity_buf)) = (
        engine.density_buffers.get(engine.ping_pong_idx),
        engine.velocity_buffers.get(engine.ping_pong_idx),
    ) else {
        return;
    };

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("GMC Volume Pack Bind Group"),
        layout: &pack_state.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: density_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: velocity_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: engine.temperature_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&gpu_image.texture_view),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("GMC Live Volume Pack Encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("GMC Live Volume Pack Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pack_state.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        // 96x96x96 grid with @workgroup_size(8, 8, 4) -> (12, 12, 24) workgroups
        pass.dispatch_workgroups(12, 12, 24);
    }

    render_queue.submit(Some(encoder.finish()));

    pack_state.last_packed_ping_pong = engine.ping_pong_idx;
    pack_state.last_packed_epoch = engine.restart_epoch;
    pack_state.last_packed_elapsed = engine.last_elapsed_years;
    pack_state.last_volume_id = Some(volume.0);
}
