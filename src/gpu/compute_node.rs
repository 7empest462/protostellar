//! WGPU Compute Pipeline Engine for 100,000+ particle protoplanetary Keplerian mechanics and gas drag.

use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::Extract;
use rayon::prelude::*;
use std::f64::consts::PI;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
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
    pub uniform_buffer: wgpu::Buffer,
    pub staging_buffers: [wgpu::Buffer; 2],
    pub staging_states: [Arc<AtomicU8>; 2],
    pub staging_counts: [u32; 2],
    pub current_staging_idx: usize,
    pub bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::ComputePipeline,
    pub num_particles: u32,
    pub is_ready: bool,
    pub last_disk_outer_r: f32,
    pub last_star_mass: f32,
    pub last_elapsed_years: f64,
}

/// Resource in the Main world that receives GPU particle readback data.
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
    pub disk_mass: f32,
    pub elapsed_years: f64,
}

/// Extracts state from the Main App into the Render Sub-App.
#[allow(clippy::type_complexity, reason = "GPU Sim Data Extraction")]
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
    scenario_state: Extract<Option<Res<crate::simulation::scenarios::ActiveScenarioState>>>,
) {
    if let Some(scenario_state) = scenario_state.as_ref() {
        if scenario_state.current_preset
            == crate::simulation::scenarios::ScenarioPreset::GalacticCollision
        {
            commands.insert_resource(GpuSimExtractedParams {
                is_paused: time_warp.is_paused,
                step_once: time_warp.step_once,
                enable_gpu_compute: false, // DISABLE PREVIOUS COMPUTE NODE
                dt: 0.0,
                star_pos: [0.0; 3],
                star_mass: 0.0,
                gas_scale: 0.0,
                inner_radius: 0.0,
                outer_radius: 0.0,
                enable_gas_drag: 0,
                ref_temp_1au: 0.0,
                shockwave_radius: 0.0,
                softening_sq: 0.0,
                num_massive_bodies: 0,
                tractor_pos_mass: [0.0; 4],
                massive_bodies: [MassiveBodyGpu::default(); 32],
                count: 0,
                disk_mass: 0.0,
                elapsed_years: 0.0,
            });
            return;
        }
    }
    let mut star_pos = [0.0f32; 3];
    let mut star_mass = disk_params.central_star_mass as f32;
    let mut shockwave_radius = 0.0f32;
    let mut massive_bodies = [MassiveBodyGpu::default(); 32];
    let mut num_bodies = 0usize;

    if let Ok((pos, mass, _rad, _temp, _lum, ignition)) = star_query.single() {
        star_pos = [pos.x as f32, pos.y as f32, pos.z as f32];
        star_mass = mass.0 as f32;
        shockwave_radius = ignition.shockwave_radius as f32;
    }

    let mut sorted_bodies: Vec<(Entity, &SimPosition, &Mass)> = massive_query
        .iter()
        .map(|(ent, pos, mass, _body)| (ent, pos, mass))
        .collect();
    sorted_bodies.sort_by(|a, b| b.2 .0.total_cmp(&a.2 .0).then_with(|| a.0.cmp(&b.0)));

    for (_ent, pos, mass) in sorted_bodies.into_iter().take(32) {
        if let Some(target) = massive_bodies.get_mut(num_bodies) {
            *target = MassiveBodyGpu {
                pos_mass: [pos.x as f32, pos.y as f32, pos.z as f32, mass.0 as f32],
            };
            num_bodies += 1;
        }
    }

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
        num_massive_bodies: num_bodies as u32,
        tractor_pos_mass,
        massive_bodies,
        count: config.target_particle_count as u32,
        disk_mass: disk_params.disk_mass as f32,
        elapsed_years: sim_time.elapsed_years,
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
pub fn setup_gpu_simulation(commands: &mut Commands, render_dev: &RenderDevice, count: u32) {
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
        _pad: 0.0,
        tractor_pos_mass: [0.0, 0.0, 0.0, 0.0],
        massive_bodies: [MassiveBodyGpu::default(); 32],
    };

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Protostellar Orbit Uniform Buffer"),
        contents: bytemuck::bytes_of(&initial_uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
        ],
    });

    let staging_buffer_size =
        u64::from(MAX_GPU_PARTICLES) * std::mem::size_of::<GpuParticle>() as u64;
    let staging_buffer_0 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Protostellar Orbit Staging Readback Buffer 0"),
        size: staging_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let staging_buffer_1 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Protostellar Orbit Staging Readback Buffer 1"),
        size: staging_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let staging_states = [
        Arc::new(AtomicU8::new(STAGING_STATE_IDLE)),
        Arc::new(AtomicU8::new(STAGING_STATE_IDLE)),
    ];

    commands.insert_resource(GpuParticleOrbitEngine {
        particle_buffer,
        uniform_buffer,
        staging_buffers: [staging_buffer_0, staging_buffer_1],
        staging_states,
        staging_counts: [0, 0],
        current_staging_idx: 0,
        bind_group,
        pipeline,
        num_particles: n_particles,
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
    let count_changed = params.count > 0 && params.count != engine.num_particles;
    let scenario_restarted = params.elapsed_years < engine.last_elapsed_years;
    engine.last_elapsed_years = params.elapsed_years;
    if !scenario_restarted
        && !count_changed
        && (params.outer_radius - engine.last_disk_outer_r).abs() <= 1.0
        && (params.star_mass - engine.last_star_mass).abs() <= 0.05
    {
        return;
    }

    if count_changed {
        engine.num_particles = params.count.clamp(50_000, MAX_GPU_PARTICLES);
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
    engine.last_disk_outer_r = params.outer_radius;
    engine.last_star_mass = params.star_mass;
}

fn drain_mapped_staging_buffers(
    engine: &GpuParticleOrbitEngine,
    sender: Option<&Res<crate::gpu::GpuReadbackSender>>,
) {
    for i in 0..2 {
        if let (Some(state), Some(staging_buf)) =
            (engine.staging_states.get(i), engine.staging_buffers.get(i))
        {
            if state.load(Ordering::Acquire) == STAGING_STATE_MAPPED {
                if let Some(sender) = sender {
                    if let Ok(mapped) = staging_buf.slice(..).get_mapped_range() {
                        let count = engine
                            .staging_counts
                            .get(i)
                            .copied()
                            .unwrap_or(0)
                            .min(MAX_GPU_PARTICLES);
                        let active_bytes = usize::try_from(count).unwrap_or(0)
                            * std::mem::size_of::<GpuParticle>();
                        let copy_len = if active_bytes == 0 {
                            mapped.len()
                        } else {
                            active_bytes.min(mapped.len())
                        };
                        if let Some(slice) = mapped.get(..copy_len) {
                            let mut buf = sender.recycle_rx.try_recv().unwrap_or_default();
                            if buf.capacity() < copy_len {
                                buf.reserve_exact(copy_len.saturating_sub(buf.capacity()));
                            }
                            buf.clear();
                            buf.extend_from_slice(slice);
                            let _ = sender.tx.try_send(buf);
                        }
                    }
                }
                staging_buf.unmap();
                state.store(STAGING_STATE_IDLE, Ordering::Release);
            }
        }
    }
}

fn dispatch_compute_and_stage(
    device: &wgpu::Device,
    queue: &RenderQueue,
    engine: &mut GpuParticleOrbitEngine,
) {
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
            let particle_count = engine.num_particles.min(MAX_GPU_PARTICLES);
            let buf_size = u64::from(particle_count) * std::mem::size_of::<GpuParticle>() as u64;
            encoder.copy_buffer_to_buffer(&engine.particle_buffer, 0, write_buf, 0, buf_size);
            if let Some(slot) = engine.staging_counts.get_mut(write_idx) {
                *slot = particle_count;
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

/// Dispatches GPU compute passes inside the RenderApp every frame with zero-stall double buffering.
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
    if !params.enable_gpu_compute || (params.is_paused && !params.step_once) {
        return;
    }

    if galaxy_params.is_some() {
        return;
    }

    let Some(mut engine) = gpu_engine else {
        setup_gpu_simulation(&mut commands, &render_device, params.count);
        return;
    };
    let queue = &render_queue;
    let device = render_device.wgpu_device();

    if !engine.is_ready {
        return;
    }

    check_and_reseed_scenario(&mut engine, &params, queue);

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
        _pad: 0.0,
        tractor_pos_mass: params.tractor_pos_mass,
        massive_bodies: params.massive_bodies,
    };

    queue.write_buffer(&engine.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

    drain_mapped_staging_buffers(&engine, sender.as_ref());

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

fn sync_dormant_particle_color(
    comp_slot: &mut Composition,
    color_slot: &mut [f32; 4],
    raw_comp: [f32; 4],
    temp: f32,
) {
    if color_slot[3] > 0.0 {
        return;
    }
    let s = f64::from(raw_comp[0]);
    let ice = f64::from(raw_comp[1]);
    let m = f64::from(raw_comp[2]);
    let g = f64::from(raw_comp[3]);
    let total = s + ice + m + g;
    let comp = Composition {
        silicate_frac: s,
        ice_frac: ice,
        metal_frac: m,
        gas_frac: g,
        organics_frac: if total > 0.0 {
            (1.0 - total).max(0.0)
        } else {
            0.0
        },
    };
    *comp_slot = comp;
    *color_slot = crate::rendering::particle_swarm::setup::calculate_particle_color(&comp, temp);
}

/// Main-world system that receives GPU readback data and updates ParticleSwarmData positions.
/// This bridges GPU compute results → CPU visual mesh and accretion without frame stalls.
pub fn receive_gpu_readback(
    receiver: Option<Res<GpuReadbackReceiver>>,
    mut swarm: Option<ResMut<crate::rendering::particle_swarm::ParticleSwarmData>>,
    mut config: Option<ResMut<SimulationConfig>>,
    sim_time: Option<Res<SimTime>>,
) {
    let Some(receiver) = receiver else {
        return;
    };
    let Some(ref mut data) = swarm else {
        return;
    };

    let mut latest: Option<Vec<u8>> = None;
    while let Ok(bytes) = receiver.rx.try_recv() {
        if let Some(old) = latest {
            let _ = receiver.recycle_tx.try_send(old);
        }
        latest = Some(bytes);
    }

    let Some(bytes) = latest else {
        return;
    };

    if let Some(ref mut cfg) = config {
        adjust_particle_count(cfg.target_particle_count, data);
    }

    if data.base_mass <= 0.0 {
        let _ = receiver.recycle_tx.try_send(bytes);
        return;
    }

    {
        let particles: &[GpuParticle] = bytemuck::cast_slice(&bytes);
        if particles.len() != data.count {
            let _ = receiver.recycle_tx.try_send(bytes);
            return;
        }
        if let Some(first_active) = particles.iter().find(|p| p.pos_mass[3] > 0.0) {
            let is_galaxy_buf = first_active.pos_mass[3] >= 0.5;
            let is_galaxy_swarm = data.base_mass >= 0.5;
            if is_galaxy_buf != is_galaxy_swarm {
                let _ = receiver.recycle_tx.try_send(bytes);
                return;
            }
        }
        let is_scenario_start = sim_time.is_some_and(|t| t.elapsed_years < 0.005);
        let active = unpack_gpu_readback_particles(particles, data, is_scenario_start);
        data.is_dirty = true;

        if let Some(ref mut cfg) = config {
            cfg.gpu_compute_active = true;
            cfg.active_particles = active;
        }
    }

    let _ = receiver.recycle_tx.try_send(bytes);
}

fn unpack_gpu_readback_particles(
    particles: &[GpuParticle],
    data: &mut crate::rendering::particle_swarm::ParticleSwarmData,
    is_scenario_start: bool,
) -> u32 {
    let n = data
        .count
        .min(particles.len())
        .min(data.masses.len())
        .min(data.positions.len())
        .min(data.velocities.len())
        .min(data.temperatures.len())
        .min(data.compositions.len())
        .min(data.colors.len());
    let crate::rendering::particle_swarm::ParticleSwarmData {
        masses,
        positions,
        velocities,
        compositions,
        temperatures,
        colors,
        pending_gpu_accretions,
        ..
    } = data;

    let (
        Some(masses_slice),
        Some(pos_slice),
        Some(vel_slice),
        Some(temp_slice),
        Some(comp_slice),
        Some(col_slice),
        Some(p_slice),
    ) = (
        masses.get_mut(..n),
        positions.get_mut(..n),
        velocities.get_mut(..n),
        temperatures.get_mut(..n),
        compositions.get_mut(..n),
        colors.get_mut(..n),
        particles.get(..n),
    )
    else {
        return 0;
    };

    let (chunk_accretions, active): (Vec<(usize, f32)>, u32) = masses_slice
        .par_chunks_mut(4096)
        .zip(pos_slice.par_chunks_mut(4096))
        .zip(vel_slice.par_chunks_mut(4096))
        .zip(temp_slice.par_chunks_mut(4096))
        .zip(comp_slice.par_chunks_mut(4096))
        .zip(col_slice.par_chunks_mut(4096))
        .zip(p_slice.par_chunks(4096))
        .map(
            |(
                (((((m_chunk, pos_chunk), vel_chunk), temp_chunk), comp_chunk), col_chunk),
                p_chunk,
            )| {
                let mut local_accretions = Vec::new();
                let mut local_active = 0u32;
                for ((((((mass, pos), vel), temp), comp), col), p) in m_chunk
                    .iter_mut()
                    .zip(pos_chunk.iter_mut())
                    .zip(vel_chunk.iter_mut())
                    .zip(temp_chunk.iter_mut())
                    .zip(comp_chunk.iter_mut())
                    .zip(col_chunk.iter_mut())
                    .zip(p_chunk.iter())
                {
                    let [px, py, pz, pm] = p.pos_mass;
                    let [vx, vy, vz, vt] = p.vel_temp;

                    if pm > 0.0 && (*mass > 0.0 || is_scenario_start) {
                        local_active += 1;
                    }

                    // Never resurrect a particle that the CPU has already accreted/killed
                    if !is_scenario_start && *mass <= 0.0 {
                        continue;
                    }

                    // If GPU marked particle as dead/accreted, record its mass before zeroing
                    if pm <= 0.0 {
                        if *mass > 0.0 {
                            let m = *mass;
                            let code = -pm;
                            if (0.99..=33.5).contains(&code) {
                                let body_idx = (code - 1.0).round() as usize;
                                local_accretions.push((body_idx, m));
                            }
                            *mass = 0.0;
                            *pos = [0.0, -5000.0, 0.0];
                        }
                        continue;
                    }

                    *pos = [px, py, pz];
                    *vel = [vx, vy, vz];
                    *mass = pm;
                    *temp = vt;
                    sync_dormant_particle_color(comp, col, p.composition, vt);
                }
                (local_accretions, local_active)
            },
        )
        .reduce(
            || (Vec::new(), 0u32),
            |(mut acc_a, cnt_a), (acc_b, cnt_b)| {
                acc_a.extend(acc_b);
                (acc_a, cnt_a + cnt_b)
            },
        );
    pending_gpu_accretions.extend(chunk_accretions);
    active
}
