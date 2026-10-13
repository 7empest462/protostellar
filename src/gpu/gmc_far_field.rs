//! GPU far-field gravity for the Giant Molecular Cloud cluster.
//!
//! With thousands of forming systems the CPU all-pairs star gravity dominates frame time. This
//! module offloads the *far field* (every source farther than [`GMC_FAR_FIELD_NEAR_RADIUS_AU`]
//! from a body) to a compute shader. Results are read back asynchronously (one job in flight,
//! no stalls) and consumed by the CPU physics, which only evaluates the exact *near field*
//! through a spatial grid. The far field is smooth, so its small frame latency is harmless.

use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp};
use bytemuck::{Pod, Zeroable};
use hashbrown::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::simulation::components::*;
use crate::simulation::resources::TimeWarp;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::G_ASTRO;

/// Below this body count the exact CPU all-pairs path is cheap enough and is used unchanged.
pub const GMC_FAR_FIELD_MIN_BODIES: usize = 600;
/// Pairs closer than this are integrated exactly on the CPU; farther pairs come from the GPU.
pub const GMC_FAR_FIELD_NEAR_RADIUS_AU: f64 = 50.0;
/// Minimum mass (M_sun) for a non-stellar body to act as a far-field gravity source.
const FAR_FIELD_MIN_SOURCE_MASS: f64 = 0.001;
/// Plummer softening used by the CPU cluster kernel (1.5 AU).
const CLUSTER_SOFTENING_SQ: f32 = 2.25;

/// One completed GPU job: the target entities and their far-field accelerations (AU/yr^2).
pub type FarFieldBatch = (Vec<Entity>, Vec<[f32; 4]>);

/// Main-world receiver for GPU far-field batches.
#[derive(Resource)]
pub struct GmcFarFieldReceiver {
    pub rx: flume::Receiver<FarFieldBatch>,
}

/// Render-world sender for GPU far-field batches.
#[derive(Resource)]
pub struct GmcFarFieldSender {
    pub tx: flume::Sender<FarFieldBatch>,
}

/// Latest far-field acceleration per body, consumed by the CPU physics step.
#[derive(Resource, Default)]
pub struct GmcFarField {
    pub acc: HashMap<Entity, DVec3>,
}

/// Render-world flag that is set while a GPU job is in flight.
#[derive(Resource, Clone)]
struct FarFieldBusy(Arc<AtomicBool>);

/// Snapshot extracted from the main world for one GPU job.
#[derive(Resource)]
struct GmcFarFieldExtracted {
    entities: Vec<Entity>,
    targets: Vec<[f32; 4]>,
    sources: Vec<[f32; 4]>,
}

#[derive(Resource)]
struct GmcFarFieldPipeline {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FarFieldUniforms {
    n_src: u32,
    n_tgt: u32,
    near_sq: f32,
    g_const: f32,
    softening_sq: f32,
    pad: [f32; 3],
}

/// Registers the far-field channel, extraction, dispatch and receive systems.
pub struct GmcFarFieldPlugin;

impl Plugin for GmcFarFieldPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = flume::bounded::<FarFieldBatch>(2);
        app.insert_resource(GmcFarField::default())
            .insert_resource(GmcFarFieldReceiver { rx })
            .add_systems(Update, receive_gmc_far_field);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(GmcFarFieldSender { tx })
            .insert_resource(FarFieldBusy(Arc::new(AtomicBool::new(false))))
            .add_systems(ExtractSchedule, extract_gmc_far_field)
            .add_systems(Render, step_gmc_far_field);
    }
}

/// Main world: keep only the newest GPU batch; drop everything outside the GMC scenario.
pub fn receive_gmc_far_field(
    receiver: Res<GmcFarFieldReceiver>,
    mut far: ResMut<GmcFarField>,
    scenario: Option<Res<ActiveScenarioState>>,
) {
    let mut latest = None;
    while let Ok(batch) = receiver.rx.try_recv() {
        latest = Some(batch);
    }
    let active =
        scenario.is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);
    if !active {
        far.acc.clear();
        return;
    }
    if let Some((entities, accs)) = latest {
        far.acc.clear();
        far.acc.extend(entities.into_iter().zip(accs).map(|(e, a)| {
            (
                e,
                DVec3::new(f64::from(a[0]), f64::from(a[1]), f64::from(a[2])),
            )
        }));
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Bevy Extract Query tuple for physics bodies"
)]
fn extract_gmc_far_field(
    mut commands: Commands,
    busy: Res<FarFieldBusy>,
    time_warp: Extract<Res<TimeWarp>>,
    scenario: Extract<Option<Res<ActiveScenarioState>>>,
    bodies: Extract<
        Query<
            (
                Entity,
                &SimPosition,
                &Mass,
                &CelestialBody,
                Has<CentralStar>,
            ),
            Without<crate::simulation::probes::SpaceProbe>,
        >,
    >,
) {
    commands.remove_resource::<GmcFarFieldExtracted>();

    let active = scenario
        .as_deref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);
    if !active || busy.0.load(Ordering::Acquire) || (time_warp.is_paused && !time_warp.step_once) {
        return;
    }
    let count = bodies.iter().count();
    if count < GMC_FAR_FIELD_MIN_BODIES {
        return;
    }

    let mut entities = Vec::with_capacity(count);
    let mut targets = Vec::with_capacity(count);
    let mut sources = Vec::new();
    for (entity, pos, mass, body, is_central) in bodies.iter() {
        let p = [pos.0.x as f32, pos.0.y as f32, pos.0.z as f32];
        entities.push(entity);
        targets.push([p[0], p[1], p[2], 0.0]);
        // The central star is always handled exactly on the CPU.
        let is_source = !is_central
            && (body.body_type.is_star_or_remnant() || mass.0 >= FAR_FIELD_MIN_SOURCE_MASS);
        if is_source {
            sources.push([p[0], p[1], p[2], mass.0 as f32]);
        }
    }
    commands.insert_resource(GmcFarFieldExtracted {
        entities,
        targets,
        sources,
    });
}

fn create_far_field_pipeline(device: &wgpu::Device) -> GmcFarFieldPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("GMC Far Field Module"),
        source: wgpu::ShaderSource::Wgsl(
            include_str!("../../assets/shaders/gmc_far_field.wgsl").into(),
        ),
    });

    let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GMC Far Field Layout"),
        entries: &[
            storage(0, true),
            storage(1, true),
            storage(2, false),
            wgpu::BindGroupLayoutEntry {
                binding: 3,
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
        label: Some("GMC Far Field Pipeline Layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("GMC Far Field Pipeline"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    GmcFarFieldPipeline { layout, pipeline }
}

fn buffer_init(
    device: &wgpu::Device,
    label: &str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage,
    })
}

/// Render world: run one far-field job (if idle) and asynchronously read back the result.
fn step_gmc_far_field(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    sender: Option<Res<GmcFarFieldSender>>,
    busy: Res<FarFieldBusy>,
    extracted: Option<Res<GmcFarFieldExtracted>>,
    pipeline: Option<Res<GmcFarFieldPipeline>>,
) {
    let Some(extracted) = extracted else {
        return;
    };
    commands.remove_resource::<GmcFarFieldExtracted>();
    let (Some(sender), Some(pipeline)) = (
        sender,
        pipeline.or_else(|| {
            commands.insert_resource(create_far_field_pipeline(render_device.wgpu_device()));
            None
        }),
    ) else {
        return;
    };
    if extracted.sources.is_empty() || extracted.targets.is_empty() {
        return;
    }
    if busy
        .0
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }

    let device = render_device.wgpu_device();
    let n_tgt = extracted.targets.len();
    let out_size = (n_tgt * std::mem::size_of::<[f32; 4]>()) as u64;

    let src_buf = buffer_init(
        device,
        "GMC Far Field Sources",
        bytemuck::cast_slice(&extracted.sources),
        wgpu::BufferUsages::STORAGE,
    );
    let tgt_buf = buffer_init(
        device,
        "GMC Far Field Targets",
        bytemuck::cast_slice(&extracted.targets),
        wgpu::BufferUsages::STORAGE,
    );
    let out_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("GMC Far Field Output"),
        size: out_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("GMC Far Field Staging"),
        size: out_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let uniforms = FarFieldUniforms {
        n_src: extracted.sources.len() as u32,
        n_tgt: n_tgt as u32,
        near_sq: (GMC_FAR_FIELD_NEAR_RADIUS_AU * GMC_FAR_FIELD_NEAR_RADIUS_AU) as f32,
        g_const: G_ASTRO as f32,
        softening_sq: CLUSTER_SOFTENING_SQ,
        pad: [0.0; 3],
    };
    let uniform_buf = buffer_init(
        device,
        "GMC Far Field Uniforms",
        bytemuck::bytes_of(&uniforms),
        wgpu::BufferUsages::UNIFORM,
    );

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("GMC Far Field Bind Group"),
        layout: &pipeline.layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: src_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: tgt_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: uniform_buf.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("GMC Far Field Encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("GMC Far Field Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups((n_tgt as u32).div_ceil(64), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buf, 0, &staging, 0, out_size);
    render_queue.submit(Some(encoder.finish()));

    let entities = extracted.entities.clone();
    let tx = sender.tx.clone();
    let busy_flag = busy.0.clone();
    let staging_cb = staging.clone();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            if result.is_ok() {
                let accs: Vec<[f32; 4]> = {
                    let data = staging_cb.slice(..).get_mapped_range();
                    bytemuck::pod_collect_to_vec::<u8, [f32; 4]>(&data)
                };
                staging_cb.unmap();
                let _ = tx.try_send((entities, accs));
            }
            busy_flag.store(false, Ordering::Release);
        });
    let _ = render_device.poll(wgpu::PollType::Poll);
}
