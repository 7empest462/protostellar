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
    pub elapsed_years: f64,
}

/// Extracts state from the Main App into the Render Sub-App.
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
        if num_bodies >= 32 {
            break;
        }
        massive_bodies[num_bodies] = MassiveBodyGpu {
            pos_mass: [pos.x as f32, pos.y as f32, pos.z as f32, mass.0 as f32],
        };
        num_bodies += 1;
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
    let visual_flow_dt = (0.002 * (1.0 + speed_mult.log10().max(0.0) * 2.0)).min(0.08);

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
        enable_gas_drag: if config.enable_gas_drag { 1 } else { 0 },
        ref_temp_1au: disk_params.reference_temp_1au as f32,
        shockwave_radius,
        softening_sq: (config.softening_au * config.softening_au) as f32,
        num_massive_bodies: num_bodies as u32,
        tractor_pos_mass,
        massive_bodies,
        count: config.target_particle_count as u32,
        elapsed_years: sim_time.elapsed_years,
    });
}

/// Initializes the 100,000 particle VRAM storage buffers and compiles WGSL compute pipelines in RenderApp.
pub fn setup_gpu_simulation(commands: &mut Commands, render_dev: &RenderDevice) {
    let device = render_dev.wgpu_device();

    let n_particles = 100_000u32;
    let individual_mass = (0.02 / (n_particles as f64)) as f32;

    // 1. Generate 100,000 multi-zone astrophysical particles for GPU VRAM
    let mut rng = rand::rng();
    let default_params = DiskParameters::default();
    let mut initial_particles = Vec::with_capacity(n_particles as usize);

    for _ in 0..n_particles {
        let (r, comp_struct) =
            crate::simulation::disk::sample_disk_radius(&mut rng, &default_params);
        let phi = rng.random_range(0.0..2.0 * PI);

        let h_scale = (0.030 * r * (r / 1.0).powf(0.25)).max(1e-4);
        let normal_dist =
            Normal::new(0.0, h_scale).unwrap_or_else(|_| Normal::new(0.0, 1e-3).unwrap());
        let z_height: f64 = rng.sample(normal_dist);

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

    // 2. Create GPU Storage & Uniform Buffers
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

    // 3. Compile WGSL Compute Shader
    let orbit_shader_src = include_str!("../../assets/shaders/particle_orbit.wgsl");
    let orbit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Particle Orbit Compute Module"),
        source: wgpu::ShaderSource::Wgsl(orbit_shader_src.into()),
    });

    // 4. Create Bind Group Layout & Pipeline
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

    // Create double-buffered non-blocking staging buffers for GPU→CPU readback
    let staging_buffer_size = (n_particles as u64) * std::mem::size_of::<GpuParticle>() as u64;
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

    if !params.enable_gpu_compute {
        return;
    }

    if params.is_paused && !params.step_once {
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

    // Check if scenario changed (e.g. Little Red Dot disk 250 AU vs Solar 45 AU, Trappist 0.25 AU, etc.)
    if (params.outer_radius - engine.last_disk_outer_r).abs() > 1.0
        || (params.star_mass - engine.last_star_mass).abs() > 0.05
    {
        let mut rng = rand::rng();
        let mut reseed_particles = Vec::with_capacity(engine.num_particles as usize);
        let disk_params = DiskParameters {
            central_star_mass: params.star_mass as f64,
            disk_mass: 0.02,
            inner_radius_au: params.inner_radius as f64,
            outer_radius_au: params.outer_radius as f64,
            reference_temp_1au: params.ref_temp_1au as f64,
            ..default()
        };
        let individual_mass = (0.02 / (engine.num_particles as f64)) as f32;
        for _ in 0..engine.num_particles {
            let (r, comp_struct) =
                crate::simulation::disk::sample_disk_radius(&mut rng, &disk_params);
            let phi = rng.random_range(0.0..2.0 * PI);
            let h_scale = (0.030 * r * (r / 1.0).powf(0.25)).max(1e-4);
            let normal_dist =
                Normal::new(0.0, h_scale).unwrap_or_else(|_| Normal::new(0.0, 1e-3).unwrap());
            let z_height: f64 = rng.sample(normal_dist);
            let pos = [
                (r * phi.cos()) as f32,
                z_height as f32,
                (r * phi.sin()) as f32,
                individual_mass,
            ];
            let v_k = (G_ASTRO * params.star_mass as f64 / r).sqrt();
            let v_phi = v_k as f32;
            let vel = [
                (-v_phi * phi.sin() as f32),
                0.0,
                (v_phi * phi.cos() as f32),
                (params.ref_temp_1au as f64 * (r / 1.0).powf(-0.5)) as f32,
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
        queue.write_buffer(
            &engine.particle_buffer,
            0,
            bytemuck::cast_slice(&reseed_particles),
        );
        engine.last_disk_outer_r = params.outer_radius;
        engine.last_star_mass = params.star_mass;
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
        _pad: 0.0,
        tractor_pos_mass: params.tractor_pos_mass,
        massive_bodies: params.massive_bodies,
    };

    queue.write_buffer(&engine.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

    // 1. Asynchronous read from any buffer that finished mapping
    for i in 0..2 {
        if engine.staging_states[i].load(Ordering::Acquire) == STAGING_STATE_MAPPED {
            if let Some(ref sender) = sender {
                let mapped = engine.staging_buffers[i].slice(..).get_mapped_range();
                let _ = sender.tx.try_send(mapped.to_vec());
                drop(mapped);
            }
            engine.staging_buffers[i].unmap();
            engine.staging_states[i].store(STAGING_STATE_IDLE, Ordering::Release);
        }
    }

    // 2. Encode & Dispatch Compute Pass on GPU
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

    // 3. Try to copy to an IDLE staging buffer
    let write_idx = engine.current_staging_idx;
    let mut copy_initiated = false;

    if engine.staging_states[write_idx]
        .compare_exchange(
            STAGING_STATE_IDLE,
            STAGING_STATE_MAPPING,
            Ordering::AcqRel,
            Ordering::Relaxed,
        )
        .is_ok()
    {
        let buf_size = (engine.num_particles as u64) * std::mem::size_of::<GpuParticle>() as u64;
        encoder.copy_buffer_to_buffer(
            &engine.particle_buffer,
            0,
            &engine.staging_buffers[write_idx],
            0,
            buf_size,
        );
        copy_initiated = true;
    }

    queue.submit(Some(encoder.finish()));

    if copy_initiated {
        let state_flag = engine.staging_states[write_idx].clone();
        engine.staging_buffers[write_idx]
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_ok() {
                    state_flag.store(STAGING_STATE_MAPPED, Ordering::Release);
                } else {
                    state_flag.store(STAGING_STATE_IDLE, Ordering::Release);
                }
            });

        // Advance to alternate staging buffer
        engine.current_staging_idx = write_idx ^ 1;
    }

    // Non-blocking driver poll
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
        latest = Some(bytes);
    }

    let Some(bytes) = latest else {
        return;
    };

    let particles: &[GpuParticle] = bytemuck::cast_slice(&bytes);
    let n = data.count.min(particles.len());
    let is_scenario_start = sim_time.map(|t| t.elapsed_years < 0.005).unwrap_or(false);

    for (i, p) in particles.iter().enumerate().take(n) {
        // Never resurrect a particle that the CPU has already accreted/killed
        if !is_scenario_start && data.masses[i] <= 0.0 {
            continue;
        }

        // If GPU marked particle as dead, mark it dead on CPU permanently
        if p.pos_mass[3] <= 0.0 {
            data.masses[i] = 0.0;
            data.positions[i] = [0.0, -5000.0, 0.0];
            continue;
        }

        data.positions[i] = [p.pos_mass[0], p.pos_mass[1], p.pos_mass[2]];
        data.velocities[i] = [p.vel_temp[0], p.vel_temp[1], p.vel_temp[2]];
        data.masses[i] = p.pos_mass[3];
        data.temperatures[i] = p.vel_temp[3];
    }
    data.is_dirty = true;

    if let Some(ref mut cfg) = config {
        cfg.gpu_compute_active = true;
        cfg.active_particles = n as u32;
    }
}
