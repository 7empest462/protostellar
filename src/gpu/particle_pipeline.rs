//! Instanced GPU Vertex-Pulling Render Pipeline for 500k Protoplanetary Disk & Galaxy Particles.
//!
//! Draws up to 500,000 camera-facing soft Gaussian particle quads directly from the active GPU
//! compute storage buffer (`GpuParticleOrbitEngine::particle_buffer` or
//! `GpuGalaxyOrbitEngine::particle_buffer`) in a single `Transparent3d` instanced draw call
//! (`pass.draw(0..6, 0..instance_count)`) with zero vertex buffers, zero CPU billboard rebuilds,
//! and zero 24 MB/frame GPU -> CPU readback.

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::core_pipeline::core_3d::{CORE_3D_DEPTH_FORMAT, Transparent3d, TransparentSortingInfo3d};
use bevy::ecs::query::ROQueryItem;
use bevy::ecs::system::SystemParamItem;
use bevy::ecs::system::lifetimeless::{Read, SRes};
use bevy::pbr::{MeshPipelineKey, ViewKeyCache};
use bevy::prelude::*;
use bevy::render::render_phase::{
    AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
    RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
};
use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only_sized, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::sync_world::MainEntity;
use bevy::render::view::{ExtractedView, RetainedViewEntity};
use bevy::render::{GpuResourceAppExt, Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::ShaderRef;
use rustc_hash::FxHashMap;
use wgpu::util::DeviceExt;

use crate::gpu::buffers::GpuParticlePullUniforms;
use crate::gpu::compute_node::{GpuParticleOrbitEngine, GpuSimExtractedParams, MAX_GPU_PARTICLES};
use crate::gpu::nbody_galaxy::{GpuGalaxyExtractedParams, GpuGalaxyOrbitEngine};

static NEXT_GPU_BUFFER_UID: AtomicU64 = AtomicU64::new(1);

/// Allocates a unique monotonic buffer identity for zero-churn bind-group caching.
pub(crate) fn next_gpu_buffer_uid() -> u64 {
    NEXT_GPU_BUFFER_UID.fetch_add(1, Ordering::Relaxed)
}

/// Computes the unscaled billboard base half-radius (AU), matching `billboards.rs` bit-for-bit.
#[must_use]
pub fn compute_gpu_pull_base_radius(is_galaxy: bool, count: u32, outer_radius_au: f32) -> f32 {
    if is_galaxy {
        let scale_mult = (outer_radius_au / 45.0).max(0.02);
        0.16 * scale_mult
    } else {
        let density_scale = if count > 0 {
            (500_000.0 / count as f32).powf(0.25).clamp(1.0, 1.5)
        } else {
            1.0
        };
        let scale_mult = (outer_radius_au / 45.0).clamp(0.02, 1.0);
        0.080 * scale_mult * density_scale
    }
}

/// Legacy public material handle retained for API compatibility.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DiskParticleMaterial {
    #[uniform(0)]
    pub tint: LinearRgba,
    #[uniform(1)]
    pub intensity: f32,
}

impl Default for DiskParticleMaterial {
    fn default() -> Self {
        Self {
            tint: LinearRgba::WHITE,
            intensity: 1.0,
        }
    }
}

impl Material for DiskParticleMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/particle_render.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/particle_render.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(Clone, Resource)]
struct GpuParticlePullPipeline {
    bind_group_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct GpuParticlePullPipelineKey {
    view_key: MeshPipelineKey,
}

impl SpecializedRenderPipeline for GpuParticlePullPipeline {
    type Key = GpuParticlePullPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let format = key.view_key.target_format();
        RenderPipelineDescriptor {
            label: Some("GPU Particle Vertex-Pulling Pipeline".into()),
            layout: vec![self.bind_group_layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vs_main".into()),
                shader_defs: vec![],
                // Pure GPU vertex pulling: zero vertex buffers bound
                buffers: vec![],
                constants: vec![],
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some("fs_main".into()),
                shader_defs: vec![],
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                constants: vec![],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: key.view_key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            ..default()
        }
    }
}

#[derive(Resource)]
struct GpuParticlePullEntity(Entity);

struct ViewPullBindGroup {
    uniform_buffer: wgpu::Buffer,
    bind_group: BindGroup,
    bound_source_uid: u64,
}

#[derive(Resource, Default)]
struct GpuParticlePullRenderData {
    views: FxHashMap<RetainedViewEntity, ViewPullBindGroup>,
    instance_count: u32,
}

fn init_gpu_particle_pull_pipeline(mut commands: Commands, asset_server: Res<AssetServer>) {
    let bind_group_layout = BindGroupLayoutDescriptor::new(
        "GPU Particle Pull Bind Group Layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer_sized(
                    false,
                    NonZeroU64::new(std::mem::size_of::<GpuParticlePullUniforms>() as u64),
                ),
                storage_buffer_read_only_sized(false, None),
            ),
        ),
    );

    let shader: Handle<Shader> = asset_server.load("shaders/particle_render.wgsl");
    let marker = commands.spawn_empty().id();
    commands.insert_resource(GpuParticlePullEntity(marker));
    commands.insert_resource(GpuParticlePullPipeline {
        bind_group_layout,
        shader,
    });
}

struct ActivePullSource<'a> {
    particle_buffer: &'a wgpu::Buffer,
    source_uid: u64,
    instance_count: u32,
    is_galaxy: bool,
    outer_radius_au: f32,
    render_scale: f32,
    base_mass: f32,
}

fn resolve_active_pull_source<'a>(
    disk_params: Option<&GpuSimExtractedParams>,
    disk_engine: Option<&'a GpuParticleOrbitEngine>,
    galaxy_params: Option<&GpuGalaxyExtractedParams>,
    galaxy_engine: Option<&'a GpuGalaxyOrbitEngine>,
) -> Option<ActivePullSource<'a>> {
    if let (Some(gp), Some(ge)) = (galaxy_params, galaxy_engine) {
        if gp.enable_gpu_compute && gp.is_swarm_visible && ge.is_ready && ge.num_particles > 0 {
            return Some(ActivePullSource {
                particle_buffer: &ge.particle_buffer,
                source_uid: ge.buffer_uid,
                instance_count: ge.num_particles.min(MAX_GPU_PARTICLES),
                is_galaxy: true,
                outer_radius_au: gp.outer_radius.max(10_000.0),
                render_scale: gp.particle_render_scale,
                base_mass: 1.0,
            });
        }
        return None;
    }

    if let (Some(dp), Some(de)) = (disk_params, disk_engine)
        && dp.enable_gpu_compute
        && dp.is_swarm_visible
        && dp.disk_mass > 0.0
        && de.is_ready
        && de.num_particles > 0
    {
        return Some(ActivePullSource {
            particle_buffer: &de.particle_buffer,
            source_uid: de.buffer_uid,
            instance_count: de.num_particles.min(MAX_GPU_PARTICLES),
            is_galaxy: false,
            outer_radius_au: dp.outer_radius.max(0.1),
            render_scale: dp.particle_render_scale,
            base_mass: de.base_mass,
        });
    }

    None
}

#[allow(
    clippy::too_many_arguments,
    reason = "Transparent3d GPU particle vertex-pulling queue system"
)]
fn queue_gpu_pulled_particles(
    draw_functions: Res<DrawFunctions<Transparent3d>>,
    pull_pipeline: Option<Res<GpuParticlePullPipeline>>,
    pull_entity: Option<Res<GpuParticlePullEntity>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<GpuParticlePullPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<&ExtractedView>,
    view_key_cache: Res<ViewKeyCache>,
    disk_params: Option<Res<GpuSimExtractedParams>>,
    disk_engine: Option<Res<GpuParticleOrbitEngine>>,
    galaxy_params: Option<Res<GpuGalaxyExtractedParams>>,
    galaxy_engine: Option<Res<GpuGalaxyOrbitEngine>>,
) {
    let (Some(pull_pipeline), Some(pull_entity)) = (pull_pipeline, pull_entity) else {
        return;
    };
    let Some(source) = resolve_active_pull_source(
        disk_params.as_deref(),
        disk_engine.as_deref(),
        galaxy_params.as_deref(),
        galaxy_engine.as_deref(),
    ) else {
        return;
    };
    if source.instance_count == 0 {
        return;
    }

    let Some(draw_function) = draw_functions.read().get_id::<DrawGpuPulledParticles>() else {
        return;
    };

    for view in &views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };
        let Some(&view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };

        let pipeline = pipelines.specialize(
            &pipeline_cache,
            &pull_pipeline,
            GpuParticlePullPipelineKey { view_key },
        );

        transparent_phase.add_transient(Transparent3d {
            sorting_info: TransparentSortingInfo3d::Sorted {
                mesh_center: Vec3::ZERO,
                depth_bias: 0.0,
            },
            entity: (pull_entity.0, MainEntity::from(pull_entity.0)),
            draw_function,
            pipeline,
            distance: 0.0,
            batch_range: 0..1,
            extra_index: PhaseItemExtraIndex::None,
            indexed: false,
        });
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Render-world per-view uniform and bind-group preparation for GPU vertex pulling"
)]
fn prepare_gpu_particle_pull_resources(
    pull_pipeline: Option<Res<GpuParticlePullPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    views: Query<&ExtractedView>,
    disk_params: Option<Res<GpuSimExtractedParams>>,
    disk_engine: Option<Res<GpuParticleOrbitEngine>>,
    galaxy_params: Option<Res<GpuGalaxyExtractedParams>>,
    galaxy_engine: Option<Res<GpuGalaxyOrbitEngine>>,
    mut render_data: ResMut<GpuParticlePullRenderData>,
) {
    let Some(pull_pipeline) = pull_pipeline else {
        render_data.instance_count = 0;
        return;
    };
    let Some(source) = resolve_active_pull_source(
        disk_params.as_deref(),
        disk_engine.as_deref(),
        galaxy_params.as_deref(),
        galaxy_engine.as_deref(),
    ) else {
        render_data.instance_count = 0;
        return;
    };

    render_data.instance_count = source.instance_count;
    let base_render_r = compute_gpu_pull_base_radius(
        source.is_galaxy,
        source.instance_count,
        source.outer_radius_au,
    );
    let device = render_device.wgpu_device();
    let layout = pipeline_cache.get_bind_group_layout(&pull_pipeline.bind_group_layout);

    for view in &views {
        let clip_from_world = view
            .clip_from_world
            .unwrap_or_else(|| view.clip_from_view * view.world_from_view.to_matrix().inverse());
        let cam_right = view.world_from_view.right().as_vec3();
        let cam_up = view.world_from_view.up().as_vec3();
        let cam_pos = view.world_from_view.translation();

        let uniforms = GpuParticlePullUniforms {
            clip_from_world: clip_from_world.to_cols_array_2d(),
            camera_right_and_base_r: [cam_right.x, cam_right.y, cam_right.z, base_render_r],
            camera_up_and_scale: [cam_up.x, cam_up.y, cam_up.z, source.render_scale],
            camera_pos_and_base_mass: [cam_pos.x, cam_pos.y, cam_pos.z, source.base_mass],
            mode_params: [
                if source.is_galaxy { 1.0 } else { 0.0 },
                source.instance_count as f32,
                1.0,
                0.0,
            ],
        };

        let needs_rebuild = render_data
            .views
            .get(&view.retained_view_entity)
            .is_none_or(|entry| entry.bound_source_uid != source.source_uid);

        if needs_rebuild {
            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GPU Particle Pull Uniform Buffer"),
                contents: bytemuck::bytes_of(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let bind_group = render_device.create_bind_group(
                Some("GPU Particle Pull Bind Group"),
                &layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: source.particle_buffer.as_entire_binding(),
                    },
                ],
            );
            render_data.views.insert(
                view.retained_view_entity,
                ViewPullBindGroup {
                    uniform_buffer,
                    bind_group,
                    bound_source_uid: source.source_uid,
                },
            );
        } else if let Some(entry) = render_data.views.get(&view.retained_view_entity) {
            render_queue.write_buffer(&entry.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        }
    }
}

struct DrawGpuParticlePullQuads;

impl<P: PhaseItem> RenderCommand<P> for DrawGpuParticlePullQuads {
    type Param = SRes<GpuParticlePullRenderData>;
    type ViewQuery = Read<ExtractedView>;
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        _item: &P,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        render_data: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let data = render_data.into_inner();
        if data.instance_count == 0 {
            return RenderCommandResult::Success;
        }
        let Some(entry) = data.views.get(&view.retained_view_entity) else {
            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(0, &entry.bind_group, &[]);
        pass.draw(0..6, 0..data.instance_count);
        RenderCommandResult::Success
    }
}

type DrawGpuPulledParticles = (SetItemPipeline, DrawGpuParticlePullQuads);

/// Plugin registering the instanced GPU vertex-pulling particle pipeline in Bevy's `RenderApp`.
pub struct ParticleRenderPlugin;

impl Plugin for ParticleRenderPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_command::<Transparent3d, DrawGpuPulledParticles>()
            .init_gpu_resource::<SpecializedRenderPipelines<GpuParticlePullPipeline>>()
            .init_resource::<GpuParticlePullRenderData>()
            .add_systems(RenderStartup, init_gpu_particle_pull_pipeline)
            .add_systems(
                Render,
                queue_gpu_pulled_particles.in_set(RenderSystems::Queue),
            )
            .add_systems(
                Render,
                prepare_gpu_particle_pull_resources.in_set(RenderSystems::PrepareBindGroups),
            );
    }
}
