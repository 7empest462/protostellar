//! WGPU Compute Pipeline Engine for 100,000+ particle protoplanetary Keplerian mechanics and gas drag.

use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::Extract;
use rand::prelude::*;
use rand_distr::Normal;
use std::f64::consts::PI;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::gpu::buffers::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

pub const STAGING_STATE_IDLE: u8 = 0;
pub const STAGING_STATE_MAPPING: u8 = 1;
pub const STAGING_STATE_MAPPED: u8 = 2;

/// Holds WGPU compute pipeline resources and VRAM-resident particle buffers.
#[derive(Resource)]
pub struct GpuParticleOrbitEngine {
    pub particle_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub staging_buffers: [wgpu::Buffer; 2],
    pub staging_states: [Arc<AtomicU8>; 2],
    pub current_staging_idx: usize,
    pub bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::ComputePipeline,
    pub num_particles: u32,
    pub is_ready: bool,
    pub last_disk_outer_r: f32,
    pub last_star_mass: f32,
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
    massive_query: Extract<Query<(&SimPosition, &Mass, &CelestialBody), Without<CentralStar>>>,
) {
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

    for (pos, mass, _body) in massive_query.iter() {
        if let Some(target) = massive_bodies.get_mut(num_bodies) {
            *target = MassiveBodyGpu {
                pos_mass: [pos.x as f32, pos.y as f32, pos.z as f32, mass.0 as f32],
            };
            num_bodies += 1;
        } else {
            break;
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
    n_particles: u32,
    default_params: &DiskParameters,
) -> Vec<GpuParticle> {
    let individual_mass = (default_params.disk_mass / f64::from(n_particles)) as f32;
    let mut rng = rand::rng();
    let mut initial_particles = Vec::with_capacity(n_particles as usize);

    for _ in 0..n_particles {
        let (r, comp_struct) =
            crate::simulation::disk::sample_disk_radius(&mut rng, default_params);
        let phi = rng.random_range(0.0..2.0 * PI);
        let h_scale = (0.030 * r * (r / 1.0).powf(0.25)).max(1e-4);
        let z_height: f64 = if let Ok(dist) = Normal::new(0.0, h_scale) {
            rng.sample(dist)
        } else {
            0.0
        };

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

/// Initializes the 100,000 particle VRAM storage buffers and compiles WGSL compute pipelines in RenderApp.
pub fn setup_gpu_simulation(commands: &mut Commands, render_dev: &RenderDevice) {
    let device = render_dev.wgpu_device();
    let n_particles = 100_000u32;
    let default_params = DiskParameters::default();

    let initial_particles = generate_initial_gpu_particles(n_particles, &default_params);

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

    let staging_buffer_size = u64::from(n_particles) * std::mem::size_of::<GpuParticle>() as u64;
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
        current_staging_idx: 0,
        bind_group,
        pipeline,
        num_particles: n_particles,
        is_ready: true,
        last_disk_outer_r: default_params.outer_radius_au as f32,
        last_star_mass: default_params.central_star_mass as f32,
    });
}

fn check_and_reseed_scenario(
    engine: &mut GpuParticleOrbitEngine,
    params: &GpuSimExtractedParams,
    queue: &RenderQueue,
) {
    if (params.outer_radius - engine.last_disk_outer_r).abs() <= 1.0
        && (params.star_mass - engine.last_star_mass).abs() <= 0.05
    {
        return;
    }

    let mut rng = rand::rng();
    let mut reseed_particles = Vec::with_capacity(engine.num_particles as usize);
    let is_empty_disk = params.disk_mass <= 0.0;
    let disk_mass = if is_empty_disk {
        0.0
    } else if params.star_mass > 10.0 {
        500.0
    } else {
        0.00010
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
            let phi = rng.random_range(0.0..2.0 * PI);
            let h_scale = (0.030 * r * (r / 1.0).powf(0.25)).max(1e-4);
            let z_height: f64 = if let Ok(dist) = Normal::new(0.0, h_scale) {
                rng.sample(dist)
            } else {
                0.0
            };
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
                    let mapped = staging_buf.slice(..).get_mapped_range();
                    let mut buf = sender.recycle_rx.try_recv().unwrap_or_default();
                    if buf.capacity() < mapped.len() {
                        buf.reserve_exact(mapped.len().saturating_sub(buf.capacity()));
                    }
                    buf.clear();
                    buf.extend_from_slice(&mapped);
                    let _ = sender.tx.try_send(buf);
                    drop(mapped);
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
            let buf_size =
                u64::from(engine.num_particles) * std::mem::size_of::<GpuParticle>() as u64;
            encoder.copy_buffer_to_buffer(&engine.particle_buffer, 0, write_buf, 0, buf_size);
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
) {
    let Some(params) = params else {
        return;
    };
    if !params.enable_gpu_compute || (params.is_paused && !params.step_once) {
        return;
    }

    let Some(mut engine) = gpu_engine else {
        setup_gpu_simulation(&mut commands, &render_device);
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

    {
        let particles: &[GpuParticle] = bytemuck::cast_slice(&bytes);
        let n = data.count.min(particles.len());
        let is_scenario_start = sim_time.is_some_and(|t| t.elapsed_years < 0.005);
        let crate::rendering::particle_swarm::ParticleSwarmData {
            masses,
            positions,
            velocities,
            temperatures,
            pending_gpu_accretions,
            ..
        } = &mut **data;

        for (i, p) in particles.iter().enumerate().take(n) {
            let [px, py, pz, pm] = p.pos_mass;
            let [vx, vy, vz, vt] = p.vel_temp;
            let Some(mass) = masses.get_mut(i) else {
                continue;
            };

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
                        pending_gpu_accretions.push((body_idx, m));
                    }
                    *mass = 0.0;
                    if let Some(pos) = positions.get_mut(i) {
                        *pos = [0.0, -5000.0, 0.0];
                    }
                }
                continue;
            }

            if let Some(pos) = positions.get_mut(i) {
                *pos = [px, py, pz];
            }
            if let Some(vel) = velocities.get_mut(i) {
                *vel = [vx, vy, vz];
            }
            *mass = pm;
            if let Some(temp) = temperatures.get_mut(i) {
                *temp = vt;
            }
        }
        data.is_dirty = true;

        if let Some(ref mut cfg) = config {
            cfg.gpu_compute_active = true;
            let mut active = 0u32;
            for p in particles.iter().take(n) {
                if p.pos_mass[3] > 0.0 {
                    active += 1;
                }
            }
            cfg.active_particles = active;
        }
    }

    let _ = receiver.recycle_tx.try_send(bytes);
}
