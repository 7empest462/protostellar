//! WGPU Compute Pipeline Engine for 100,000+ particle protoplanetary Keplerian mechanics and gas drag.

use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use std::f64::consts::PI;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use wgpu::util::DeviceExt;

use crate::gpu::buffers::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;
use crate::utils::math::FastRngExt;

pub const STAGING_STATE_IDLE: u8 = 0;
pub const STAGING_STATE_MAPPING: u8 = 1;
pub const STAGING_STATE_MAPPED: u8 = 2;

/// Maximum particle capacity supported by VRAM compute and staging buffers (500k Ultra).
pub const MAX_GPU_PARTICLES: u32 = 500_000;

/// Holds WGPU compute pipeline resources and VRAM-resident particle buffers.
#[derive(Resource)]
pub struct GpuParticleOrbitEngine {
    pub particle_buffer: wgpu::Buffer,
    pub buffer_uid: u64,
    pub uniform_buffer: wgpu::Buffer,
    pub accretion_summary_buffer: wgpu::Buffer,
    pub staging_buffers: [wgpu::Buffer; 2],
    pub staging_states: [Arc<AtomicU8>; 2],
    pub staging_epochs: [u64; 2],
    pub restart_epoch: u64,
    pub last_reload_generation: u64,
    pub current_staging_idx: usize,
    pub bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::ComputePipeline,
    pub num_particles: u32,
    pub base_mass: f32,
    pub is_ready: bool,
    pub last_disk_outer_r: f32,
    pub last_star_mass: f32,
    pub last_elapsed_years: f64,
}

/// Resource in the Main world that receives compact GPU readback data.
#[derive(Resource)]
pub struct GpuReadbackReceiver {
    pub rx: flume::Receiver<Vec<u8>>,
    pub recycle_tx: flume::Sender<Vec<u8>>,
}

/// Extracted simulation parameters passed from the main world to the render sub-app every frame.
#[derive(Resource, Default, Clone)]
pub struct GpuSimExtractedParams {
    pub is_paused: bool,
    pub step_once: bool,
    pub enable_gpu_compute: bool,
    pub is_swarm_visible: bool,
    pub particle_render_scale: f32,
    pub dt: f32,
    pub star_pos: [f32; 3],
    pub star_mass: f32,
    pub gas_scale: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub enable_gas_drag: u32,
    pub ref_temp_1au: f32,
    pub shockwave_radius: f32,
    pub softening_sq: f32,
    pub num_massive_bodies: u32,
    pub tractor_pos_mass: [f32; 4],
    pub massive_bodies: [MassiveBodyGpu; 32],
    pub count: u32,
    pub active_particles: u32,
    pub disk_mass: f32,
    pub elapsed_years: f64,
    pub reload_generation: u64,
    pub snow_line_au: f32,
    pub total_ecs_bodies: u32,
    pub quasar_blown_out: bool,
}

#[allow(
    clippy::type_complexity,
    reason = "Matches extracted Bevy Query filter for massive bodies"
)]
fn pack_top_massive_bodies(
    massive_query: &Query<
        (Entity, &SimPosition, &Mass, &CelestialBody),
        (
            Without<CentralStar>,
            Without<crate::simulation::probes::SpaceProbe>,
        ),
    >,
    scratch: &mut Vec<(Entity, Vec3, f64)>,
) -> ([MassiveBodyGpu; 32], u32, u32) {
    let mut massive_bodies = [MassiveBodyGpu::default(); 32];
    scratch.clear();
    scratch.extend(massive_query.iter().map(|(ent, pos, mass, _body)| {
        (
            ent,
            Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32),
            mass.0,
        )
    }));
    let total_ecs_bodies = scratch.len() as u32;
    scratch.sort_by(|a, b| b.2.total_cmp(&a.2).then_with(|| a.0.cmp(&b.0)));

    let mut num_bodies = 0usize;
    for &(_ent, pos, mass) in scratch.iter().take(32) {
        if let Some(target) = massive_bodies.get_mut(num_bodies) {
            *target = MassiveBodyGpu {
                pos_mass: [pos.x, pos.y, pos.z, mass as f32],
            };
            num_bodies += 1;
        }
    }
    (massive_bodies, num_bodies as u32, total_ecs_bodies)
}

/// Extracts state from the Main App into the Render Sub-App.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "GPU Sim Data Extraction"
)]
pub fn extract_gpu_sim_data(
    mut commands: Commands,
    config: Extract<Res<SimulationConfig>>,
    disk_params: Extract<Res<DiskParameters>>,
    time_warp: Extract<Res<TimeWarp>>,
    sim_time: Extract<Res<SimTime>>,
    player_state: Extract<Res<PlayerInteractionState>>,
    star_query: Extract<
        Query<
            (
                &SimPosition,
                &Mass,
                &Radius,
                &Temperature,
                &Luminosity,
                &IgnitionState,
                Option<&BlackHoleStarState>,
                &CelestialBody,
            ),
            With<CentralStar>,
        >,
    >,
    massive_query: Extract<
        Query<
            (Entity, &SimPosition, &Mass, &CelestialBody),
            (
                Without<CentralStar>,
                Without<crate::simulation::probes::SpaceProbe>,
            ),
        >,
    >,
    swarm_vis_query: Extract<
        Query<&Visibility, With<crate::rendering::particle_swarm::ParticleSwarmMesh>>,
    >,
    cam_query: Extract<Query<&crate::rendering::camera::PanOrbitCamera>>,
    scenario_state: Extract<Option<Res<crate::simulation::scenarios::ActiveScenarioState>>>,
    reload_gen: Extract<Option<Res<crate::simulation::scenarios::ScenarioReloadGeneration>>>,
    mut massive_scratch: Local<Vec<(Entity, Vec3, f64)>>,
) {
    let reload_generation = reload_gen.as_ref().map_or(0, |g| g.0);
    let in_surface_mode = cam_query.iter().any(|c| c.surface_horizon_mode);
    let is_swarm_visible = !in_surface_mode
        && swarm_vis_query
            .iter()
            .next()
            .is_none_or(|v| *v != Visibility::Hidden);
    if scenario_state.as_ref().is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::GalacticCollision
    }) {
        commands.insert_resource(GpuSimExtractedParams {
            is_paused: time_warp.is_paused,
            step_once: time_warp.step_once,
            enable_gpu_compute: false,
            is_swarm_visible: false,
            particle_render_scale: config.particle_render_scale,
            reload_generation,
            snow_line_au: 2.7,
            ..default()
        });
        return;
    }
    let mut star_pos = [0.0f32; 3];
    let mut star_mass = disk_params.central_star_mass as f32;
    let mut shockwave_radius = 0.0f32;
    let mut quasar_blown_out = false;

    if let Ok((pos, mass, _rad, _temp, _lum, ignition, opt_bhs, star_body)) = star_query.single() {
        star_pos = [pos.x as f32, pos.y as f32, pos.z as f32];
        star_mass = mass.0 as f32;
        shockwave_radius = ignition.shockwave_radius as f32;
        quasar_blown_out =
            opt_bhs.is_some_and(|s| s.is_blown_out) || star_body.body_type == BodyType::BlackHole;
    }

    let (massive_bodies, num_massive_bodies, total_ecs_bodies) =
        pack_top_massive_bodies(&massive_query, &mut massive_scratch);

    let tractor_pos_mass = if player_state.active_tool == PlayerTool::GravitationalTractor {
        if let (Some(pos), mass) = (player_state.tractor_position, player_state.tractor_mass) {
            [pos.x as f32, pos.y as f32, pos.z as f32, mass as f32]
        } else {
            [0.0; 4]
        }
    } else {
        [0.0; 4]
    };

    let speed_mult = time_warp.multiplier as f32;
    let visual_flow_dt = if speed_mult < 1.0 {
        (0.002 * speed_mult).max(1e-7)
    } else {
        (0.002 * (1.0 + speed_mult.log10() * 2.0)).min(0.08)
    };

    commands.insert_resource(GpuSimExtractedParams {
        is_paused: time_warp.is_paused,
        step_once: time_warp.step_once,
        enable_gpu_compute: config.enable_gpu_compute,
        is_swarm_visible,
        particle_render_scale: config.particle_render_scale,
        dt: visual_flow_dt,
        star_pos,
        star_mass,
        gas_scale: config.gas_density_scale,
        inner_radius: disk_params.inner_radius_au as f32,
        outer_radius: disk_params.outer_radius_au as f32,
        enable_gas_drag: u32::from(config.enable_gas_drag),
        ref_temp_1au: disk_params.reference_temp_1au as f32,
        shockwave_radius,
        softening_sq: (config.softening_au * config.softening_au) as f32,
        num_massive_bodies,
        tractor_pos_mass,
        massive_bodies,
        count: config.target_particle_count as u32,
        active_particles: config.active_particles,
        disk_mass: disk_params.disk_mass as f32,
        elapsed_years: sim_time.elapsed_years,
        reload_generation,
        snow_line_au: disk_params.snow_line_au as f32,
        total_ecs_bodies,
        quasar_blown_out,
    });
}

fn generate_initial_gpu_particles(
    capacity: u32,
    active_count: u32,
    default_params: &DiskParameters,
) -> Vec<GpuParticle> {
    let active = active_count.min(capacity);
    let individual_mass = (default_params.disk_mass / f64::from(active.max(1))) as f32;
    let mut rng = fastrand::Rng::new();
    let mut initial_particles = Vec::with_capacity(capacity as usize);

    for _ in 0..active {
        let (r, comp_struct) =
            crate::simulation::disk::sample_disk_radius(&mut rng, default_params);
        let phi = rng.f64_range(0.0, 2.0 * PI);
        let h_scale = (0.030 * r * (r / 1.0).powf(0.25)).max(1e-4);
        let z_height: f64 = rng.normal_f64(0.0, h_scale);

        let pos = [
            (r * phi.cos()) as f32,
            z_height as f32,
            (r * phi.sin()) as f32,
            individual_mass,
        ];

        let v_k = (G_ASTRO * 1.0 / r).sqrt();
        let v_phi = v_k as f32;
        let vel = [
            (-v_phi * phi.sin() as f32),
            0.0,
            (v_phi * phi.cos() as f32),
            (280.0 * (r / 1.0).powf(-0.5)) as f32,
        ];

        let comp = [
            comp_struct.silicate_frac as f32,
            comp_struct.ice_frac as f32,
            comp_struct.metal_frac as f32,
            comp_struct.gas_frac as f32,
        ];

        initial_particles.push(GpuParticle {
            pos_mass: pos,
            vel_temp: vel,
            composition: comp,
        });
    }

    for _ in active..capacity {
        initial_particles.push(GpuParticle {
            pos_mass: [0.0, -5000.0, 0.0, 0.0],
            vel_temp: [0.0, 0.0, 0.0, 0.0],
            composition: [0.0, 0.0, 0.0, 0.0],
        });
    }

    initial_particles
}

fn create_gpu_orbit_pipeline(
    device: &wgpu::Device,
) -> (wgpu::BindGroupLayout, wgpu::ComputePipeline) {
    let orbit_shader_src = include_str!("../../assets/shaders/particle_orbit.wgsl");
    let orbit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Particle Orbit Compute Module"),
        source: wgpu::ShaderSource::Wgsl(orbit_shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Protostellar Orbit Compute Bind Group Layout"),
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
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Protostellar Orbit Pipeline Layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Particle Orbit Pipeline"),
        layout: Some(&pipeline_layout),
        module: &orbit_module,
        entry_point: Some("main"),
        compilation_options: default(),
        cache: None,
    });

    (bind_group_layout, pipeline)
}

/// Initializes the 100k-500k particle VRAM storage buffers and compiles WGSL compute pipelines in RenderApp.
pub fn setup_gpu_simulation(
    commands: &mut Commands,
    render_dev: &RenderDevice,
    count: u32,
    reload_generation: u64,
) {
    let device = render_dev.wgpu_device();
    let n_particles = count.clamp(50_000, MAX_GPU_PARTICLES);
    let default_params = DiskParameters::default();

    let initial_particles =
        generate_initial_gpu_particles(MAX_GPU_PARTICLES, n_particles, &default_params);

    let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Protostellar Particle Storage Buffer"),
        contents: bytemuck::cast_slice(&initial_particles),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });

    let initial_uniforms = GpuOrbitUniforms {
        star_pos: [0.0, 0.0, 0.0],
        star_mass: 1.0,
        dt: 0.0005,
        gas_scale: 1.0,
        inner_radius: 0.06,
        outer_radius: 35.0,
        g_const: G_ASTRO as f32,
        enable_gas_drag: 1,
        num_particles: n_particles,
        ref_temp_1au: 280.0,
        shockwave_radius: 0.0,
        softening_sq: 0.008 * 0.008,
        num_massive_bodies: 1,
        snow_line_au: default_params.snow_line_au as f32,
        tractor_pos_mass: [0.0, 0.0, 0.0, 0.0],
        massive_bodies: [MassiveBodyGpu::default(); 32],
    };

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Protostellar Orbit Uniform Buffer"),
        contents: bytemuck::bytes_of(&initial_uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let summary_bytes = (GPU_ACCRETION_SUMMARY_SLOTS * std::mem::size_of::<u32>()) as u64;
    let accretion_summary_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Protostellar Accretion Summary Buffer"),
        contents: bytemuck::cast_slice(&[0u32; GPU_ACCRETION_SUMMARY_SLOTS]),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });

    let (bind_group_layout, pipeline) = create_gpu_orbit_pipeline(device);

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Protostellar Orbit Compute Bind Group"),
        layout: &bind_group_layout,
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
                resource: accretion_summary_buffer.as_entire_binding(),
            },
        ],
    });

    // Compact 132-byte staging buffers replace the legacy 24 MB/frame full-particle readback
    let staging_buffer_0 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Protostellar Compact Accretion Staging Buffer 0"),
        size: summary_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let staging_buffer_1 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Protostellar Compact Accretion Staging Buffer 1"),
        size: summary_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let staging_states = [
        Arc::new(AtomicU8::new(STAGING_STATE_IDLE)),
        Arc::new(AtomicU8::new(STAGING_STATE_IDLE)),
    ];

    let base_mass = (default_params.disk_mass / f64::from(n_particles.max(1))) as f32;

    commands.insert_resource(GpuParticleOrbitEngine {
        particle_buffer,
        buffer_uid: crate::gpu::particle_pipeline::next_gpu_buffer_uid(),
        uniform_buffer,
        accretion_summary_buffer,
        staging_buffers: [staging_buffer_0, staging_buffer_1],
        staging_states,
        staging_epochs: [0, 0],
        restart_epoch: 0,
        last_reload_generation: reload_generation,
        current_staging_idx: 0,
        bind_group,
        pipeline,
        num_particles: n_particles,
        base_mass,
        is_ready: true,
        last_disk_outer_r: default_params.outer_radius_au as f32,
        last_star_mass: default_params.central_star_mass as f32,
        last_elapsed_years: 0.0,
    });
}

fn check_and_reseed_scenario(
    engine: &mut GpuParticleOrbitEngine,
    params: &GpuSimExtractedParams,
    queue: &RenderQueue,
) {
    let clamped_count = params.count.clamp(50_000, MAX_GPU_PARTICLES);
    let count_changed = params.count > 0 && clamped_count != engine.num_particles;
    let scenario_restarted = params.reload_generation != engine.last_reload_generation
        || params.elapsed_years < engine.last_elapsed_years;
    engine.last_reload_generation = params.reload_generation;
    engine.last_elapsed_years = params.elapsed_years;
    if !scenario_restarted
        && !count_changed
        && (params.outer_radius - engine.last_disk_outer_r).abs() <= 1.0
        && (params.star_mass - engine.last_star_mass).abs() <= 0.05
    {
        return;
    }

    if count_changed {
        engine.num_particles = clamped_count;
    }
    engine.restart_epoch = engine.restart_epoch.wrapping_add(1);
    for (state, staging_buf) in std::iter::zip(&engine.staging_states, &engine.staging_buffers) {
        if state.load(Ordering::Acquire) == STAGING_STATE_MAPPED {
            staging_buf.unmap();
            state.store(STAGING_STATE_IDLE, Ordering::Release);
        }
    }

    let mut rng = fastrand::Rng::new();
    let mut reseed_particles = Vec::with_capacity(MAX_GPU_PARTICLES as usize);
    let is_empty_disk = params.disk_mass <= 0.0;
    let disk_mass = if is_empty_disk {
        0.0
    } else if params.star_mass > 10.0 {
        500.0
    } else {
        f64::from(params.disk_mass)
    };

    if is_empty_disk {
        for _ in 0..engine.num_particles {
            reseed_particles.push(GpuParticle {
                pos_mass: [0.0, -5000.0, 0.0, 0.0],
                vel_temp: [0.0, 0.0, 0.0, 0.0],
                composition: [0.0, 0.0, 0.0, 0.0],
            });
        }
    } else {
        let disk_params = DiskParameters {
            central_star_mass: f64::from(params.star_mass),
            disk_mass,
            inner_radius_au: f64::from(params.inner_radius),
            outer_radius_au: f64::from(params.outer_radius),
            snow_line_au: f64::from(params.snow_line_au),
            reference_temp_1au: f64::from(params.ref_temp_1au),
            ..default()
        };
        let individual_mass = (disk_mass / f64::from(engine.num_particles)) as f32;
        for _ in 0..engine.num_particles {
            let (r, comp_struct) =
                crate::simulation::disk::sample_disk_radius(&mut rng, &disk_params);
            let phi = rng.f64_range(0.0, 2.0 * PI);
            let h_scale = (0.030 * r * (r / 1.0).powf(0.25)).max(1e-4);
            let z_height: f64 = rng.normal_f64(0.0, h_scale);
            let pos = [
                (r * phi.cos()) as f32,
                z_height as f32,
                (r * phi.sin()) as f32,
                individual_mass,
            ];
            let v_k = (G_ASTRO * f64::from(params.star_mass) / r).sqrt();
            let v_phi = v_k as f32;
            let vel = [
                (-v_phi * phi.sin() as f32),
                0.0,
                (v_phi * phi.cos() as f32),
                (f64::from(params.ref_temp_1au) * (r / 1.0).powf(-0.5)) as f32,
            ];
            let comp = [
                comp_struct.silicate_frac as f32,
                comp_struct.ice_frac as f32,
                comp_struct.metal_frac as f32,
                comp_struct.gas_frac as f32,
            ];
            reseed_particles.push(GpuParticle {
                pos_mass: pos,
                vel_temp: vel,
                composition: comp,
            });
        }
    }
    for _ in reseed_particles.len()..(MAX_GPU_PARTICLES as usize) {
        reseed_particles.push(GpuParticle {
            pos_mass: [0.0, -5000.0, 0.0, 0.0],
            vel_temp: [0.0, 0.0, 0.0, 0.0],
            composition: [0.0, 0.0, 0.0, 0.0],
        });
    }
    queue.write_buffer(
        &engine.particle_buffer,
        0,
        bytemuck::cast_slice(&reseed_particles),
    );
    engine.base_mass = if is_empty_disk {
        0.0
    } else {
        (disk_mass / f64::from(engine.num_particles.max(1))) as f32
    };
    engine.last_disk_outer_r = params.outer_radius;
    engine.last_star_mass = params.star_mass;
}

/// Sends a compact 144-byte [`GpuCompactReadback`] packet over the recycled channel.
pub(crate) fn send_compact_readback(
    sender: &crate::gpu::GpuReadbackSender,
    packet: &GpuCompactReadback,
) {
    let bytes = bytemuck::bytes_of(packet);
    let mut buf = sender.recycle_rx.try_recv().unwrap_or_default();
    if buf.capacity() < bytes.len() {
        buf.reserve_exact(bytes.len().saturating_sub(buf.capacity()));
    }
    buf.clear();
    buf.extend_from_slice(bytes);
    let _ = sender.tx.try_send(buf);
}

fn drain_mapped_staging_buffers(
    engine: &GpuParticleOrbitEngine,
    sender: Option<&Res<crate::gpu::GpuReadbackSender>>,
) {
    let expected_bytes = GPU_ACCRETION_SUMMARY_SLOTS * std::mem::size_of::<u32>();
    for i in 0..2 {
        if let (Some(state), Some(staging_buf)) =
            (engine.staging_states.get(i), engine.staging_buffers.get(i))
            && state.load(Ordering::Acquire) == STAGING_STATE_MAPPED
        {
            let is_fresh = engine.staging_epochs.get(i).copied() == Some(engine.restart_epoch);
            if is_fresh
                && let Some(sender) = sender
                && let Ok(mapped) = staging_buf.slice(..).get_mapped_range()
                && let Some(slice) = mapped.get(..expected_bytes)
            {
                let counts: &[u32] = bytemuck::cast_slice(slice);
                let mut packet = GpuCompactReadback {
                    magic: GPU_COMPACT_READBACK_MAGIC,
                    active_particles: counts.get(32).copied().unwrap_or(0),
                    is_galaxy: 0,
                    generation_lo: engine.last_reload_generation as u32,
                    accreted_per_body: [0; 32],
                };
                if let Some(body_counts) = counts.get(..32) {
                    packet.accreted_per_body.copy_from_slice(body_counts);
                }
                send_compact_readback(sender, &packet);
            }
            staging_buf.unmap();
            state.store(STAGING_STATE_IDLE, Ordering::Release);
        }
    }
}

fn dispatch_compute_and_stage(
    device: &wgpu::Device,
    queue: &RenderQueue,
    engine: &mut GpuParticleOrbitEngine,
) {
    // Reset the 132-byte atomic accretion + active-count summary buffer before dispatching
    queue.write_buffer(
        &engine.accretion_summary_buffer,
        0,
        bytemuck::cast_slice(&[0u32; GPU_ACCRETION_SUMMARY_SLOTS]),
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Protostellar GPU Orbit Compute Encoder"),
    });

    let workgroups = engine.num_particles.div_ceil(64);
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Protostellar Particle Orbit Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&engine.pipeline);
        compute_pass.set_bind_group(0, &engine.bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroups, 1, 1);
    }

    let write_idx = engine.current_staging_idx;
    let mut copy_initiated = false;

    if let (Some(write_state), Some(write_buf)) = (
        engine.staging_states.get(write_idx),
        engine.staging_buffers.get(write_idx),
    ) {
        if write_state
            .compare_exchange(
                STAGING_STATE_IDLE,
                STAGING_STATE_MAPPING,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            let summary_bytes = (GPU_ACCRETION_SUMMARY_SLOTS * std::mem::size_of::<u32>()) as u64;
            encoder.copy_buffer_to_buffer(
                &engine.accretion_summary_buffer,
                0,
                write_buf,
                0,
                summary_bytes,
            );
            if let Some(epoch_slot) = engine.staging_epochs.get_mut(write_idx) {
                *epoch_slot = engine.restart_epoch;
            }
            copy_initiated = true;
        }

        queue.submit(Some(encoder.finish()));

        if copy_initiated {
            let state_flag = write_state.clone();
            write_buf
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    if result.is_ok() {
                        state_flag.store(STAGING_STATE_MAPPED, Ordering::Release);
                    } else {
                        state_flag.store(STAGING_STATE_IDLE, Ordering::Release);
                    }
                });

            engine.current_staging_idx = write_idx ^ 1;
        }
    } else {
        queue.submit(Some(encoder.finish()));
    }
}

/// Dispatches GPU compute passes inside the RenderApp every frame with compact 132-byte readback.
pub fn step_gpu_simulation_render_world(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    sender: Option<Res<crate::gpu::GpuReadbackSender>>,
    gpu_engine: Option<ResMut<GpuParticleOrbitEngine>>,
    params: Option<Res<GpuSimExtractedParams>>,
    galaxy_params: Option<Res<crate::gpu::nbody_galaxy::GpuGalaxyExtractedParams>>,
) {
    let Some(params) = params else {
        return;
    };
    if !params.enable_gpu_compute || galaxy_params.is_some() {
        return;
    }

    let Some(mut engine) = gpu_engine else {
        setup_gpu_simulation(
            &mut commands,
            &render_device,
            params.count,
            params.reload_generation,
        );
        return;
    };
    let queue = &render_queue;
    let device = render_device.wgpu_device();

    if !engine.is_ready {
        return;
    }

    // Always reseed on scenario switch / count change even while paused so GPU vertex pulling
    // immediately displays the newly loaded scenario.
    let prev_epoch = engine.restart_epoch;
    check_and_reseed_scenario(&mut engine, &params, queue);
    let reseeded = engine.restart_epoch != prev_epoch;

    drain_mapped_staging_buffers(&engine, sender.as_ref());

    if reseeded && let Some(ref s) = sender {
        let active_particles = if params.disk_mass <= 0.0 {
            0
        } else {
            engine.num_particles
        };
        send_compact_readback(
            s,
            &GpuCompactReadback {
                magic: GPU_COMPACT_READBACK_MAGIC,
                active_particles,
                is_galaxy: 0,
                generation_lo: engine.last_reload_generation as u32,
                accreted_per_body: [0; 32],
            },
        );
    }

    if (params.is_paused && !params.step_once)
        || params.disk_mass <= 0.0
        || (params.active_particles == 0 && !reseeded)
    {
        let _ = render_device.poll(wgpu::PollType::Poll);
        return;
    }

    let uniforms = GpuOrbitUniforms {
        star_pos: params.star_pos,
        star_mass: params.star_mass,
        dt: params.dt,
        gas_scale: params.gas_scale,
        inner_radius: params.inner_radius,
        outer_radius: params.outer_radius,
        g_const: G_ASTRO as f32,
        enable_gas_drag: params.enable_gas_drag,
        num_particles: engine.num_particles,
        ref_temp_1au: params.ref_temp_1au,
        shockwave_radius: params.shockwave_radius,
        softening_sq: params.softening_sq,
        num_massive_bodies: params.num_massive_bodies,
        snow_line_au: params.snow_line_au,
        tractor_pos_mass: params.tractor_pos_mass,
        massive_bodies: params.massive_bodies,
    };

    queue.write_buffer(&engine.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

    dispatch_compute_and_stage(device, queue, &mut engine);

    let _ = render_device.poll(wgpu::PollType::Poll);
}

fn adjust_particle_count(
    target_count: usize,
    data: &mut crate::rendering::particle_swarm::ParticleSwarmData,
) {
    let min_count = if data.positions.len() >= 50_000 {
        50_000
    } else {
        1
    };
    let new_count = target_count
        .clamp(min_count, MAX_GPU_PARTICLES as usize)
        .min(data.positions.len());
    if new_count == 0 || new_count == data.count {
        return;
    }
    let old_count = data.count;
    data.count = new_count;
    if old_count > 0 && data.base_mass > 0.0 {
        data.base_mass *= (old_count as f32) / (new_count as f32);
    }
    if new_count < old_count {
        for i in new_count..old_count {
            if let Some(m) = data.masses.get_mut(i) {
                *m = 0.0;
            }
            if let Some(p) = data.positions.get_mut(i) {
                *p = [0.0, -5000.0, 0.0];
            }
            if let Some(v) = data.velocities.get_mut(i) {
                *v = [0.0, 0.0, 0.0];
            }
            if let Some(c) = data.colors.get_mut(i) {
                *c = [0.0, 0.0, 0.0, 0.0];
            }
            let v_idx = i * 4;
            for k in 0..4 {
                if let Some(mp) = data.mesh_positions.get_mut(v_idx + k) {
                    *mp = [0.0, -5000.0, 0.0];
                }
                if let Some(mc) = data.mesh_colors.get_mut(v_idx + k) {
                    *mc = [0.0, 0.0, 0.0, 0.0];
                }
            }
        }
    } else if new_count > old_count {
        for i in old_count..new_count {
            if let Some(m) = data.masses.get_mut(i) {
                *m = data.base_mass.max(1e-12);
            }
        }
    }
    data.is_dirty = true;
}

/// Main-world system receiving compact 144-byte GPU accretion & active-count readbacks.
/// Replaces the legacy 24 MB/frame full-particle copy while preserving exact planetary mass
/// accretion and live active-particle telemetry for GPU Vertex Pulling.
pub fn receive_gpu_readback(
    receiver: Option<Res<GpuReadbackReceiver>>,
    mut swarm: Option<ResMut<crate::rendering::particle_swarm::ParticleSwarmData>>,
    mut config: Option<ResMut<SimulationConfig>>,
    reload_gen: Option<Res<crate::simulation::scenarios::ScenarioReloadGeneration>>,
    mut last_gen: Local<u64>,
) {
    let Some(receiver) = receiver else {
        return;
    };
    let current_gen = reload_gen.as_deref().map_or(0, |g| g.0);
    if current_gen != *last_gen {
        *last_gen = current_gen;
        while let Ok(bytes) = receiver.rx.try_recv() {
            let _ = receiver.recycle_tx.try_send(bytes);
        }
        return;
    }
    let Some(ref mut data) = swarm else {
        return;
    };

    let packet_size = std::mem::size_of::<GpuCompactReadback>();
    let mut received_any = false;
    let mut latest_active = 0u32;
    let mut total_accreted = [0u32; 32];

    while let Ok(bytes) = receiver.rx.try_recv() {
        if bytes.len() == packet_size {
            let packet: &GpuCompactReadback = bytemuck::from_bytes(&bytes);
            if packet.magic == GPU_COMPACT_READBACK_MAGIC
                && packet.generation_lo == (current_gen as u32)
            {
                let is_galaxy_swarm = data.base_mass >= 0.5;
                let is_galaxy_pkt = packet.is_galaxy != 0;
                if is_galaxy_pkt == is_galaxy_swarm || data.base_mass <= 0.0 {
                    received_any = true;
                    latest_active = packet.active_particles;
                    for (dst, &src) in total_accreted.iter_mut().zip(&packet.accreted_per_body) {
                        *dst = dst.saturating_add(src);
                    }
                }
            }
        }
        let _ = receiver.recycle_tx.try_send(bytes);
    }

    if !received_any {
        return;
    }

    if let Some(ref mut cfg) = config {
        adjust_particle_count(cfg.target_particle_count, data);
        cfg.gpu_compute_active = true;
        cfg.active_particles = latest_active;
    }

    let base_mass = data.base_mass;
    if base_mass > 0.0 {
        for (body_idx, &count) in total_accreted.iter().enumerate() {
            if count > 0 {
                data.pending_gpu_accretions
                    .push((body_idx, count as f32 * base_mass));
            }
        }
    }
}
