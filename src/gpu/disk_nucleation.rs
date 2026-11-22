//! GPU radial/azimuthal overdensity nucleation pipeline and compact event promotion.
//!
//! Executes a 64x64 log-radial × azimuthal spatial density pass directly in VRAM over
//! `GpuParticleOrbitEngine::particle_buffer` (`particle_nucleate.wgsl`), coagulating dust grains
//! into pebble clumps along the inner silicate trap (`0.35 AU`) and condensation snow line
//! (`2.7` / `3.65 AU`), and emitting compact 272-byte `GpuNucleationReadback` packets to CPU
//! without ever reading back the 24 MB particle storage buffer.

use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::{Render, RenderApp, RenderSystems};
use std::f64::consts::PI;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use wgpu::util::DeviceExt;

use crate::game::ui::types::{CosmicEventLog, NotificationToast};
use crate::gpu::buffers::{
    GPU_NUCLEATION_READBACK_MAGIC, GpuDiskNucleationUniforms, GpuNucleationEvent,
    GpuNucleationReadback, NUCLEATION_CELL_U32S, NUCLEATION_GRID_R, NUCLEATION_NUM_CELLS,
};
use crate::gpu::compute_node::{
    GpuParticleOrbitEngine, GpuSimExtractedParams, STAGING_STATE_IDLE, STAGING_STATE_MAPPED,
    STAGING_STATE_MAPPING, step_gpu_simulation_render_world,
};
use crate::rendering::particle_swarm::spawn_promoted_body;
use crate::simulation::components::*;
use crate::simulation::disk::planetesimals::PlanetesimalSpawner;
use crate::simulation::resources::{DiskParameters, SimTime, SimulationConfig};
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset, ScenarioReloadGeneration};
use crate::utils::constants::*;

/// Cross-world nucleation batch containing generation identity and collapsed overdensity events.
#[derive(Clone, Debug)]
pub struct GpuNucleationBatch {
    pub generation: u64,
    pub events: Vec<GpuNucleationEvent>,
}

/// Main-world receiver draining GPU disk nucleation batches.
#[derive(Resource)]
pub struct GpuNucleationReceiver {
    pub rx: flume::Receiver<GpuNucleationBatch>,
}

/// Render-world sender forwarding compact GPU nucleation batches to the Main world.
#[derive(Resource)]
pub struct GpuNucleationSender {
    pub tx: flume::Sender<GpuNucleationBatch>,
}

/// Render-world WGPU pipelines, 4096-cell grid buffer, and double-buffered 272-byte staging resources.
#[derive(Resource)]
pub struct GpuDiskNucleationEngine {
    pub uniform_buffer: wgpu::Buffer,
    pub cell_buffer: wgpu::Buffer,
    pub output_buffer: wgpu::Buffer,
    pub staging_buffers: [wgpu::Buffer; 2],
    pub staging_states: [Arc<AtomicU8>; 2],
    pub staging_epochs: [u64; 2],
    pub staging_generations: [u64; 2],
    pub current_staging_idx: usize,
    pub bound_buffer_uid: u64,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    pub clear_pipeline: wgpu::ComputePipeline,
    pub deposit_pipeline: wgpu::ComputePipeline,
    pub evaluate_pipeline: wgpu::ComputePipeline,
    pub apply_pipeline: wgpu::ComputePipeline,
    pub finalize_pipeline: wgpu::ComputePipeline,
    pub last_restart_epoch: u64,
    pub last_promotion_sim_yr: f64,
    pub frame_seed: u32,
}

/// Tracks milestone discovery notifications per scenario generation so HUD logs highlight traps cleanly.
#[derive(Default)]
pub struct GpuNucleationToastState {
    reload_gen: u64,
    announced_snowline: bool,
    announced_inner_trap: bool,
    announced_embryo: bool,
}

/// Plugin registering GPU disk nucleation compute passes and Main-world ECS promotion handling.
pub struct DiskNucleationPlugin;

impl Plugin for DiskNucleationPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = flume::bounded::<GpuNucleationBatch>(4);
        app.insert_resource(GpuNucleationReceiver { rx })
            .add_systems(
                Update,
                receive_gpu_nucleation_events.after(crate::gpu::compute_node::receive_gpu_readback),
            );

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(GpuNucleationSender { tx })
            .add_systems(
                Render,
                step_gpu_disk_nucleation_render_world
                    .after(step_gpu_simulation_render_world)
                    .in_set(RenderSystems::PrepareResources),
            );
    }
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn create_nucleation_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    particle_buffer: &wgpu::Buffer,
    uniform_buffer: &wgpu::Buffer,
    cell_buffer: &wgpu::Buffer,
    output_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("GPU Disk Nucleation Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: particle_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: cell_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    })
}

fn make_entry_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    label: &str,
    entry: &str,
) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        module,
        entry_point: Some(entry),
        compilation_options: default(),
        cache: None,
    })
}

fn setup_nucleation_engine(
    commands: &mut Commands,
    device: &wgpu::Device,
    orbit_engine: &GpuParticleOrbitEngine,
) {
    let shader_src = include_str!("../../assets/shaders/particle_nucleate.wgsl");
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("GPU Particle Nucleation Module"),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GPU Disk Nucleation Bind Group Layout"),
        entries: &[
            storage_entry(0),
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
            storage_entry(2),
            storage_entry(3),
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("GPU Disk Nucleation Pipeline Layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GPU Disk Nucleation Uniform Buffer"),
        contents: bytemuck::bytes_of(&GpuDiskNucleationUniforms::default()),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let cell_bytes = NUCLEATION_NUM_CELLS * NUCLEATION_CELL_U32S * std::mem::size_of::<u32>();
    let cell_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("GPU Disk Nucleation Cell Grid Buffer"),
        size: cell_bytes as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let output_bytes = std::mem::size_of::<GpuNucleationReadback>() as u64;
    let output_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GPU Disk Nucleation Event Output Buffer"),
        contents: bytemuck::bytes_of(&GpuNucleationReadback::default()),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });

    let staging_buffers = [
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Nucleation Staging Buffer 0"),
            size: output_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }),
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Nucleation Staging Buffer 1"),
            size: output_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }),
    ];

    let bind_group = create_nucleation_bind_group(
        device,
        &bind_group_layout,
        &orbit_engine.particle_buffer,
        &uniform_buffer,
        &cell_buffer,
        &output_buffer,
    );

    commands.insert_resource(GpuDiskNucleationEngine {
        uniform_buffer,
        cell_buffer,
        output_buffer,
        staging_buffers,
        staging_states: [
            Arc::new(AtomicU8::new(STAGING_STATE_IDLE)),
            Arc::new(AtomicU8::new(STAGING_STATE_IDLE)),
        ],
        staging_epochs: [0, 0],
        staging_generations: [orbit_engine.last_reload_generation; 2],
        current_staging_idx: 0,
        bound_buffer_uid: orbit_engine.buffer_uid,
        clear_pipeline: make_entry_pipeline(
            device,
            &pipeline_layout,
            &module,
            "Nucleation Clear Pipeline",
            "clear_cells",
        ),
        deposit_pipeline: make_entry_pipeline(
            device,
            &pipeline_layout,
            &module,
            "Nucleation Deposit Pipeline",
            "deposit_cells",
        ),
        evaluate_pipeline: make_entry_pipeline(
            device,
            &pipeline_layout,
            &module,
            "Nucleation Evaluate Pipeline",
            "evaluate_nucleation",
        ),
        apply_pipeline: make_entry_pipeline(
            device,
            &pipeline_layout,
            &module,
            "Nucleation Apply Pipeline",
            "apply_cells_to_particles",
        ),
        finalize_pipeline: make_entry_pipeline(
            device,
            &pipeline_layout,
            &module,
            "Nucleation Finalize Clumps Pipeline",
            "finalize_clump_leaders",
        ),
        bind_group_layout,
        bind_group,
        last_restart_epoch: orbit_engine.restart_epoch,
        last_promotion_sim_yr: 0.0,
        frame_seed: 1,
    });
}

fn drain_nucleation_staging(
    engine: &GpuDiskNucleationEngine,
    orbit_engine: &GpuParticleOrbitEngine,
    sender: Option<&GpuNucleationSender>,
) {
    let expected_bytes = std::mem::size_of::<GpuNucleationReadback>();
    for i in 0..2 {
        let Some(state) = engine.staging_states.get(i) else {
            continue;
        };
        let Some(buf) = engine.staging_buffers.get(i) else {
            continue;
        };
        if state.load(Ordering::Acquire) != STAGING_STATE_MAPPED {
            continue;
        }
        let is_fresh = engine.staging_epochs.get(i).copied() == Some(orbit_engine.restart_epoch);
        if is_fresh
            && let Some(tx) = sender
            && let Ok(mapped) = buf.slice(..).get_mapped_range()
            && let Some(slice) = mapped.get(..expected_bytes)
        {
            let pkt: &GpuNucleationReadback = bytemuck::from_bytes(slice);
            if pkt.magic == GPU_NUCLEATION_READBACK_MAGIC && pkt.event_count > 0 {
                let valid: Vec<GpuNucleationEvent> = pkt
                    .events
                    .iter()
                    .take((pkt.event_count as usize).min(pkt.events.len()))
                    .copied()
                    .filter(|ev| ev.pos_mass[3] > 0.0 && ev.pos_mass[3].is_finite())
                    .collect();
                if !valid.is_empty() {
                    let generation = engine
                        .staging_generations
                        .get(i)
                        .copied()
                        .unwrap_or(orbit_engine.last_reload_generation);
                    let _ = tx.tx.try_send(GpuNucleationBatch {
                        generation,
                        events: valid,
                    });
                }
            }
        }
        buf.unmap();
        state.store(STAGING_STATE_IDLE, Ordering::Release);
    }
}

/// Computes uniform parameters and cadence gating for a GPU disk nucleation dispatch.
pub fn build_nucleation_uniforms(
    params: &GpuSimExtractedParams,
    num_particles: u32,
    base_mass: f32,
    last_promotion_sim_yr: &mut f64,
    frame_seed: u32,
) -> GpuDiskNucleationUniforms {
    let is_massive = params.star_mass > 10.0;
    let max_ecs = if is_massive { 32u32 } else { 1024u32 };
    let interval_yr = if params.total_ecs_bodies < 8 {
        15.0
    } else if params.total_ecs_bodies < 32 {
        30.0
    } else if params.total_ecs_bodies < 128 {
        55.0
    } else {
        90.0
    };

    let can_run = !params.quasar_blown_out
        && params.disk_mass > 0.0
        && params.gas_scale > 0.01
        && num_particles > 0
        && base_mass > 0.0;
    let cooldown_ready = params.elapsed_years >= 15.0
        && (params.elapsed_years - *last_promotion_sim_yr) >= interval_yr
        && params.total_ecs_bodies < max_ecs;

    let max_promotions = if can_run && cooldown_ready {
        *last_promotion_sim_yr = params.elapsed_years;
        max_ecs.saturating_sub(params.total_ecs_bodies).min(3)
    } else {
        0
    };

    let min_clump_mass_solar = if is_massive {
        (0.25 * JUPITER_MASS_SOLAR) as f32
    } else {
        (0.0018 * EARTH_MASS_SOLAR) as f32
    };
    let min_peak_particles = (num_particles / 8192).max(4);
    let v_ref_scale = ((G_ASTRO * f64::from(params.star_mass).max(0.01)
        / f64::from(params.inner_radius).max(0.06))
    .sqrt() as f32)
        .clamp(5.0, 200.0);

    GpuDiskNucleationUniforms {
        star_pos: params.star_pos,
        star_mass: params.star_mass,
        inner_radius: params.inner_radius,
        outer_radius: params.outer_radius,
        snow_line_au: params.snow_line_au,
        base_mass,
        gas_scale: params.gas_scale,
        elapsed_years: params.elapsed_years as f32,
        v_ref_scale,
        num_particles,
        enable_promotion: u32::from(can_run && max_promotions > 0),
        max_promotions,
        min_peak_particles,
        frame_seed,
        min_clump_mass_solar,
        coagulate_enabled: u32::from(can_run && params.elapsed_years >= 5.0 && !is_massive),
        num_massive_bodies: params.num_massive_bodies,
        _pad: 0,
        massive_bodies: params.massive_bodies,
    }
}

fn dispatch_nucleation_passes(
    device: &wgpu::Device,
    queue: &RenderQueue,
    engine: &mut GpuDiskNucleationEngine,
    orbit_engine: &GpuParticleOrbitEngine,
    uniforms: &GpuDiskNucleationUniforms,
) {
    queue.write_buffer(&engine.uniform_buffer, 0, bytemuck::bytes_of(uniforms));

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("GPU Disk Nucleation Compute Encoder"),
    });

    let cell_wgs = NUCLEATION_GRID_R; // 64 workgroups * 64 threads == 4096 cells
    let part_wgs = orbit_engine.num_particles.div_ceil(64);
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("GPU Disk Nucleation Pass"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &engine.bind_group, &[]);

        pass.set_pipeline(&engine.clear_pipeline);
        pass.dispatch_workgroups(cell_wgs, 1, 1);

        pass.set_pipeline(&engine.deposit_pipeline);
        pass.dispatch_workgroups(part_wgs, 1, 1);

        pass.set_pipeline(&engine.evaluate_pipeline);
        pass.dispatch_workgroups(cell_wgs, 1, 1);

        pass.set_pipeline(&engine.apply_pipeline);
        pass.dispatch_workgroups(part_wgs, 1, 1);

        pass.set_pipeline(&engine.finalize_pipeline);
        pass.dispatch_workgroups(cell_wgs, 1, 1);
    }

    let write_idx = engine.current_staging_idx;
    let mut copy_initiated = false;
    if uniforms.enable_promotion != 0
        && let (Some(write_state), Some(write_buf)) = (
            engine.staging_states.get(write_idx),
            engine.staging_buffers.get(write_idx),
        )
    {
        if write_state
            .compare_exchange(
                STAGING_STATE_IDLE,
                STAGING_STATE_MAPPING,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            let readback_bytes = std::mem::size_of::<GpuNucleationReadback>() as u64;
            encoder.copy_buffer_to_buffer(&engine.output_buffer, 0, write_buf, 0, readback_bytes);
            if let Some(epoch_slot) = engine.staging_epochs.get_mut(write_idx) {
                *epoch_slot = orbit_engine.restart_epoch;
            }
            if let Some(gen_slot) = engine.staging_generations.get_mut(write_idx) {
                *gen_slot = orbit_engine.last_reload_generation;
            }
            copy_initiated = true;
        }
        queue.submit(Some(encoder.finish()));
        if copy_initiated {
            let flag = write_state.clone();
            write_buf
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |res| {
                    flag.store(
                        if res.is_ok() {
                            STAGING_STATE_MAPPED
                        } else {
                            STAGING_STATE_IDLE
                        },
                        Ordering::Release,
                    );
                });
            engine.current_staging_idx = write_idx ^ 1;
        }
    } else {
        queue.submit(Some(encoder.finish()));
    }
}

/// Render-world compute system dispatching radial/azimuthal overdensity nucleation passes.
#[allow(
    clippy::too_many_arguments,
    reason = "Coordinates GPU nucleation passes with orbit engine and galaxy guard"
)]
pub fn step_gpu_disk_nucleation_render_world(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    sender: Option<Res<GpuNucleationSender>>,
    orbit_engine: Option<Res<GpuParticleOrbitEngine>>,
    nuc_engine: Option<ResMut<GpuDiskNucleationEngine>>,
    params: Option<Res<GpuSimExtractedParams>>,
    galaxy_params: Option<Res<crate::gpu::nbody_galaxy::GpuGalaxyExtractedParams>>,
) {
    let (Some(params), Some(orbit_engine)) = (params, orbit_engine) else {
        return;
    };
    if !params.enable_gpu_compute || galaxy_params.is_some() || !orbit_engine.is_ready {
        return;
    }

    let device = render_device.wgpu_device();
    let Some(mut engine) = nuc_engine else {
        setup_nucleation_engine(&mut commands, device, &orbit_engine);
        return;
    };

    if orbit_engine.buffer_uid != engine.bound_buffer_uid {
        engine.bind_group = create_nucleation_bind_group(
            device,
            &engine.bind_group_layout,
            &orbit_engine.particle_buffer,
            &engine.uniform_buffer,
            &engine.cell_buffer,
            &engine.output_buffer,
        );
        engine.bound_buffer_uid = orbit_engine.buffer_uid;
    }

    if orbit_engine.restart_epoch != engine.last_restart_epoch {
        engine.last_restart_epoch = orbit_engine.restart_epoch;
        engine.last_promotion_sim_yr = 0.0;
        engine.frame_seed = 1;
        for (state, buf) in std::iter::zip(&engine.staging_states, &engine.staging_buffers) {
            if state.load(Ordering::Acquire) == STAGING_STATE_MAPPED {
                buf.unmap();
                state.store(STAGING_STATE_IDLE, Ordering::Release);
            }
        }
    }

    drain_nucleation_staging(&engine, &orbit_engine, sender.as_deref());

    if (params.is_paused && !params.step_once)
        || params.disk_mass <= 0.0
        || params.active_particles == 0
    {
        return;
    }

    let seed = engine.frame_seed;
    engine.frame_seed = engine.frame_seed.wrapping_add(1);
    let mut last_yr = engine.last_promotion_sim_yr;
    let uniforms = build_nucleation_uniforms(
        &params,
        orbit_engine.num_particles,
        orbit_engine.base_mass,
        &mut last_yr,
        seed,
    );
    engine.last_promotion_sim_yr = last_yr;

    dispatch_nucleation_passes(device, &render_queue, &mut engine, &orbit_engine, &uniforms);
    let _ = render_device.poll(wgpu::PollType::Poll);
}

fn sanitize_gpu_nucleation_composition(raw: [f32; 4]) -> Composition {
    let sil = f64::from(raw[0].clamp(0.0, 1.0));
    let ice = f64::from(raw[1].clamp(0.0, 1.0));
    let met = f64::from(raw[2].clamp(0.0, 1.0));
    let gas = f64::from(raw[3].clamp(0.0, 1.0));
    let sum = sil + ice + met + gas;
    if sum > 1.0 && sum.is_finite() {
        Composition {
            silicate_frac: sil / sum,
            ice_frac: ice / sum,
            metal_frac: met / sum,
            organics_frac: 0.0,
            gas_frac: gas / sum,
        }
    } else if sum > 1e-4 && sum.is_finite() {
        Composition {
            silicate_frac: sil,
            ice_frac: ice,
            metal_frac: met,
            organics_frac: (1.0 - sum).max(0.0),
            gas_frac: gas,
        }
    } else {
        Composition::carbonaceous()
    }
}

fn announce_gpu_nucleation(
    entity: Entity,
    zone_kind: u32,
    body_type: BodyType,
    r_au: f64,
    elapsed_years: f64,
    state: &mut GpuNucleationToastState,
    toast: &mut Option<ResMut<NotificationToast>>,
    event_log: &mut Option<ResMut<CosmicEventLog>>,
) {
    let msg_opt = if zone_kind == 2 && !state.announced_snowline {
        state.announced_snowline = true;
        Some((
            "❄️",
            format!(
                "❄️ GPU Snowline Nucleation! Icy dust grains collapsed into a planetesimal at {r_au:.2} AU"
            ),
        ))
    } else if zone_kind == 1 && !state.announced_inner_trap {
        state.announced_inner_trap = true;
        Some((
            "🪨",
            format!(
                "🪨 Silicate Trap Coagulation! Inner rocky planetesimal nucleated at {r_au:.2} AU"
            ),
        ))
    } else if body_type == BodyType::Protoplanet && !state.announced_embryo {
        state.announced_embryo = true;
        Some((
            "🪐",
            format!("🪐 Planetary Embryo Emerged! Runaway clump nucleated at {r_au:.2} AU"),
        ))
    } else {
        None
    };

    if let Some((icon, msg)) = msg_opt {
        if let Some(log) = event_log.as_deref_mut() {
            log.push(elapsed_years, icon, msg.clone(), Some(entity));
        }
        if let Some(t) = toast.as_deref_mut() {
            t.message = msg;
            t.timer = 6.0;
        }
    }
}

/// Main-world ECS system draining compact GPU nucleation batches and promoting clumps into full N-body entities.
#[allow(
    clippy::too_many_arguments,
    reason = "Coordinates GPU nucleation readbacks with scenario guards, N-body exclusion, and HUD logs"
)]
pub fn receive_gpu_nucleation_events(
    mut commands: Commands,
    receiver: Option<Res<GpuNucleationReceiver>>,
    sim_time: Option<Res<SimTime>>,
    config: Option<Res<SimulationConfig>>,
    disk_params: Option<Res<DiskParameters>>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    reload_gen: Option<Res<ScenarioReloadGeneration>>,
    mut spawner: Option<ResMut<PlanetesimalSpawner>>,
    existing_bodies: Query<&SimPosition, (With<CelestialBody>, Without<CentralStar>)>,
    mut toast: Option<ResMut<NotificationToast>>,
    mut event_log: Option<ResMut<CosmicEventLog>>,
    mut toast_state: Local<GpuNucleationToastState>,
) {
    let Some(receiver) = receiver else {
        return;
    };
    let current_gen = reload_gen.as_deref().map_or(0, |g| g.0);
    if current_gen != toast_state.reload_gen {
        *toast_state = GpuNucleationToastState {
            reload_gen: current_gen,
            ..default()
        };
    }
    if receiver.rx.is_empty() {
        return;
    }

    let elapsed_years = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);
    let default_disk = DiskParameters::default();
    let disk_params = disk_params.as_deref().unwrap_or(&default_disk);
    let blocked_preset = scenario_state.as_ref().is_some_and(|s| {
        matches!(
            s.current_preset,
            ScenarioPreset::MolecularCloudCluster
                | ScenarioPreset::GalacticCollision
                | ScenarioPreset::TidalDisruptionEvent
        )
    });
    let gpu_allowed = config
        .as_deref()
        .is_some_and(|c| c.enable_gpu_compute && c.gpu_compute_active);
    if blocked_preset || !gpu_allowed || disk_params.disk_mass <= 0.0 || elapsed_years < 15.0 {
        while receiver.rx.try_recv().is_ok() {}
        return;
    }

    let is_massive_disk = disk_params.central_star_mass > 10.0;
    let max_ecs_bodies = if is_massive_disk { 32 } else { 1024 };
    let min_sep_sq = if is_massive_disk { 9.0 } else { 0.08 * 0.08 };
    let mut occupied: Vec<DVec3> = existing_bodies.iter().map(|p| p.0).collect();

    while let Ok(batch) = receiver.rx.try_recv() {
        if batch.generation != current_gen {
            continue;
        }
        for ev in batch.events {
            if occupied.len() >= max_ecs_bodies {
                break;
            }
            let mass = f64::from(ev.pos_mass[3]);
            if !mass.is_finite() || mass <= 0.0 {
                continue;
            }
            let pos = DVec3::new(
                f64::from(ev.pos_mass[0]),
                f64::from(ev.pos_mass[1]),
                f64::from(ev.pos_mass[2]),
            );
            let r_au = pos.xz().length();
            if !pos.is_finite() || r_au < 0.12 || r_au > disk_params.outer_radius_au.max(1.0) * 1.05
            {
                continue;
            }
            if occupied
                .iter()
                .any(|p| (*p - pos).length_squared() < min_sep_sq)
            {
                continue;
            }

            let raw_vel = DVec3::new(
                f64::from(ev.vel_temp[0]),
                f64::from(ev.vel_temp[1]),
                f64::from(ev.vel_temp[2]),
            );
            let v_k = (G_ASTRO * disk_params.central_star_mass.max(0.01) / r_au).sqrt();
            let vel = if raw_vel.is_finite() && raw_vel.length() > v_k * 0.4 {
                raw_vel
            } else {
                let phi = pos.z.atan2(pos.x);
                DVec3::new(-v_k * phi.sin(), 0.0, v_k * phi.cos())
            };

            let comp = sanitize_gpu_nucleation_composition(ev.composition);
            let avg_density = comp.average_density().max(0.1);
            let radius = ((3.0 * mass / avg_density) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.18);

            let (entity, body_type, _name) = spawn_promoted_body(
                &mut commands,
                pos,
                vel,
                mass,
                radius,
                comp,
                disk_params,
                is_massive_disk,
            );
            occupied.push(pos);
            if let Some(sp) = spawner.as_deref_mut() {
                sp.total_spawned += 1;
                sp.name_counter += 1;
            }
            announce_gpu_nucleation(
                entity,
                ev.meta[2],
                body_type,
                r_au,
                elapsed_years,
                &mut toast_state,
                &mut toast,
                &mut event_log,
            );
        }
    }
}
