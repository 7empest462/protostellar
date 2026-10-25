use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::Extract;
use bytemuck::{Pod, Zeroable};
use rand::prelude::*;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::gpu::buffers::{GpuParticle, MassiveBodyGpu};
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

pub const STAGING_STATE_IDLE: u8 = 0;
pub const STAGING_STATE_MAPPING: u8 = 1;
pub const STAGING_STATE_MAPPED: u8 = 2;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GalaxyUniforms {
    pub dt: f32,
    pub g_const: f32,
    pub softening_sq: f32,
    pub num_particles: u32,

    // NFW Dark Matter Halo Parameters
    pub halo_mass: f32,
    pub halo_scale_radius: f32,

    // Spiral Density Wave Parameters
    pub pattern_speed: f32,
    pub spiral_arms: f32,
    pub spiral_amplitude: f32,
    pub spiral_pitch: f32,

    pub num_massive_bodies: u32,
    pub pad1: f32,
    pub massive_bodies: [MassiveBodyGpu; 32], // xyz: pos, w: mass
    pub massive_body_velocities: [[f32; 4]; 32], // xyz: vel, w: unused
}

#[derive(Resource)]
pub struct GpuGalaxyOrbitEngine {
    pub particle_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub staging_buffers: [wgpu::Buffer; 2],
    pub staging_states: [Arc<AtomicU8>; 2],
    pub current_staging_idx: usize,
    pub bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::ComputePipeline,
    pub num_particles: u32,
    pub is_ready: bool,
    pub last_elapsed_years: f64,
}

#[derive(Resource, Default, Clone)]
pub struct GpuGalaxyExtractedParams {
    pub count: u32,
    pub is_paused: bool,
    pub step_once: bool,
    pub dt: f32,
    pub elapsed_years: f64,
    pub num_massive_bodies: u32,
    pub massive_bodies: [MassiveBodyGpu; 32],
    pub massive_body_velocities: [[f32; 4]; 32],

    pub halo_mass: f32,
    pub halo_scale_radius: f32,
    pub pattern_speed: f32,
    pub spiral_arms: f32,
    pub spiral_amplitude: f32,
    pub spiral_pitch: f32,
}
#[allow(
    clippy::type_complexity,
    reason = "Bevy Extract Query tuple and multi-subsystem simulation parameters"
)]
pub fn extract_galaxy_sim_data(
    mut commands: Commands,
    time_warp: Extract<Res<TimeWarp>>,
    scenario_state: Extract<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    sim_time: Extract<Res<crate::simulation::resources::SimTime>>,
    massive_query: Extract<
        Query<(
            &SimPosition,
            &Mass,
            (
                Entity,
                Option<&crate::simulation::components::SimVelocity>,
                &CelestialBody,
            ),
        )>,
    >,
    swarm: Extract<Option<Res<crate::rendering::particle_swarm::ParticleSwarmData>>>,
) {
    if scenario_state.current_preset
        != crate::simulation::scenarios::ScenarioPreset::GalacticCollision
    {
        commands.remove_resource::<GpuGalaxyExtractedParams>();
        commands.remove_resource::<GpuGalaxyOrbitEngine>();
        return;
    }

    let mut massive_bodies = [MassiveBodyGpu::default(); 32];
    let mut massive_body_velocities = [[0.0; 4]; 32];
    let mut num_bodies = 0u32;

    let total_massive_query_count = massive_query.iter().count();
    for (pos, mass, (_, vel_maybe, _)) in massive_query.iter() {
        if let (Some(body_slot), Some(vel_slot)) = (
            massive_bodies.get_mut(num_bodies as usize),
            massive_body_velocities.get_mut(num_bodies as usize),
        ) {
            *body_slot = MassiveBodyGpu {
                pos_mass: [
                    pos.0.x as f32,
                    pos.0.y as f32,
                    pos.0.z as f32,
                    mass.0 as f32,
                ],
            };
            let v = vel_maybe.map_or(bevy::math::DVec3::ZERO, |v| v.0);
            let halo_m = if total_massive_query_count <= 1 {
                2.4e12_f32 // Merged Milkomeda halo
            } else if mass.0 > 50_000_000.0 {
                1.4e12_f32 // Andromeda halo
            } else {
                1.0e12_f32 // Milky Way halo
            };
            *vel_slot = [v.x as f32, v.y as f32, v.z as f32, halo_m];
            num_bodies += 1;
        } else {
            break; // Maximum massive bodies reached
        }
    }

    // Use the exact same timestep as the CPU simulation for perfect sync
    let dt = sim_time.current_dt_yr as f32;

    let count = if let Some(s) = swarm.as_ref() {
        s.count as u32
    } else {
        100_000
    };
    commands.insert_resource(GpuGalaxyExtractedParams {
        count,
        is_paused: time_warp.is_paused,
        step_once: time_warp.step_once,
        dt,
        elapsed_years: sim_time.elapsed_years,
        num_massive_bodies: num_bodies,
        massive_bodies,
        massive_body_velocities,

        halo_mass: 1.0e12,           // 1 trillion solar masses
        halo_scale_radius: 50_000.0, // AU proxy for scale radius
        pattern_speed: 0.0,
        spiral_arms: 0.0,
        spiral_amplitude: 0.0,
        spiral_pitch: 0.0,
    });
}

fn create_galaxy_orbit_pipeline(
    device: &wgpu::Device,
) -> (wgpu::BindGroupLayout, wgpu::ComputePipeline) {
    let shader_src = include_str!("../../assets/shaders/nbody_galaxy.wgsl");
    crate::gpu::create_two_buffer_compute_pipeline(
        device,
        shader_src,
        "Galaxy N-Body Compute Module",
        "Galaxy Compute Bind Group Layout",
        "Galaxy Orbit Pipeline",
    )
}

struct GalaxyDiskConfig {
    count: u32,
    smbh_mass: f32,
    center_pos: [f32; 3],
    center_vel: [f32; 3],
    max_radius: f32,
    halo_mass: f32,
}

fn generate_galaxy_particles(cfg: &GalaxyDiskConfig, initial_particles: &mut Vec<GpuParticle>) {
    let mut rng = rand::rng();
    let rs = 50_000.0_f32;
    for _ in 0..cfg.count {
        let r = rng.random_range(5000.0..cfg.max_radius);
        let phi = rng.random_range(0.0..std::f32::consts::TAU);
        let z = rng.random_range(-500.0..500.0);
        let pos = [
            cfg.center_pos[0] + r * phi.cos(),
            cfg.center_pos[1] + z,
            cfg.center_pos[2] + r * phi.sin(),
            1.0,
        ];

        let x = r / rs;
        let mass_enclosed = cfg.halo_mass * ((1.0 + x).ln() - x / (1.0 + x));
        let v_circ =
            (crate::utils::constants::G_ASTRO as f32 * (cfg.smbh_mass + mass_enclosed) / r).sqrt();

        let vel = [
            cfg.center_vel[0] - v_circ * phi.sin(),
            cfg.center_vel[1],
            cfg.center_vel[2] + v_circ * phi.cos(),
            5000.0,
        ];
        initial_particles.push(GpuParticle {
            pos_mass: pos,
            vel_temp: vel,
            composition: [0.0, 0.0, 0.0, 0.0],
        });
    }
}

fn generate_colliding_galaxies(n_particles: u32) -> Vec<GpuParticle> {
    let mut particles = Vec::with_capacity(n_particles as usize);
    let n_mw = n_particles / 2;
    let n_m31 = n_particles - n_mw;

    generate_galaxy_particles(
        &GalaxyDiskConfig {
            count: n_mw,
            smbh_mass: 4_300_000.0,
            center_pos: [-45_000.0, -6_000.0, -22_500.0],
            center_vel: [90.0, 12.0, 45.0],
            max_radius: 50_000.0,
            halo_mass: 1.0e12,
        },
        &mut particles,
    );
    generate_galaxy_particles(
        &GalaxyDiskConfig {
            count: n_m31,
            smbh_mass: 140_000_000.0,
            center_pos: [45_000.0, 6_000.0, 22_500.0],
            center_vel: [-90.0, -12.0, -45.0],
            max_radius: 65_000.0,
            halo_mass: 1.4e12,
        },
        &mut particles,
    );

    particles
}

pub fn setup_galaxy_gpu_simulation(commands: &mut Commands, render_dev: &RenderDevice, count: u32) {
    let device = render_dev.wgpu_device();
    let n_particles = count;
    let initial_particles = generate_colliding_galaxies(n_particles);

    let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Galaxy Particle Buffer"),
        contents: bytemuck::cast_slice(&initial_particles),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });

    let initial_uniforms = GalaxyUniforms {
        dt: 0.0,
        g_const: crate::utils::constants::G_ASTRO as f32,
        softening_sq: 1_000_000.0,
        num_particles: n_particles,
        halo_mass: 1.0e12,
        halo_scale_radius: 50_000.0,
        pattern_speed: 0.0,
        spiral_arms: 0.0,
        spiral_amplitude: 0.0,
        spiral_pitch: 0.0,
        num_massive_bodies: 0,
        pad1: 0.0,
        massive_bodies: [MassiveBodyGpu::default(); 32],
        massive_body_velocities: [[0.0; 4]; 32],
    };

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Galaxy Uniform Buffer"),
        contents: bytemuck::bytes_of(&initial_uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let (bind_group_layout, pipeline) = create_galaxy_orbit_pipeline(device);

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Galaxy Compute Bind Group"),
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
        label: Some("Galaxy Staging Buffer 0"),
        size: staging_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let staging_buffer_1 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Galaxy Staging Buffer 1"),
        size: staging_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    commands.insert_resource(GpuGalaxyOrbitEngine {
        particle_buffer,
        uniform_buffer,
        staging_buffers: [staging_buffer_0, staging_buffer_1],
        staging_states: [
            std::sync::Arc::new(std::sync::atomic::AtomicU8::new(STAGING_STATE_IDLE)),
            std::sync::Arc::new(std::sync::atomic::AtomicU8::new(STAGING_STATE_IDLE)),
        ],
        current_staging_idx: 0,
        bind_group,
        pipeline,
        num_particles: n_particles,
        is_ready: true,
        last_elapsed_years: 0.0,
    });
}

pub fn step_galaxy_simulation_render_world(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    sender: Option<Res<crate::gpu::GpuReadbackSender>>,
    gpu_engine: Option<ResMut<GpuGalaxyOrbitEngine>>,
    params: Option<Res<GpuGalaxyExtractedParams>>,
) {
    let Some(params) = params else {
        return;
    };

    let Some(mut engine) = gpu_engine else {
        setup_galaxy_gpu_simulation(&mut commands, &render_device, params.count);
        return;
    };

    let scenario_restarted = params.elapsed_years < engine.last_elapsed_years;
    engine.last_elapsed_years = params.elapsed_years;

    let queue = &render_queue;
    if scenario_restarted {
        let initial_particles = generate_colliding_galaxies(engine.num_particles);
        queue.write_buffer(
            &engine.particle_buffer,
            0,
            bytemuck::cast_slice(&initial_particles),
        );
    }

    if params.is_paused && !params.step_once {
        return;
    }
    let device = render_device.wgpu_device();

    if !engine.is_ready {
        return;
    }

    let uniforms = GalaxyUniforms {
        dt: params.dt,
        g_const: G_ASTRO as f32,
        softening_sq: 1_000_000.0,
        num_particles: engine.num_particles,
        halo_mass: params.halo_mass,
        halo_scale_radius: params.halo_scale_radius,
        pattern_speed: params.pattern_speed,
        spiral_arms: params.spiral_arms,
        spiral_amplitude: params.spiral_amplitude,
        spiral_pitch: params.spiral_pitch,
        num_massive_bodies: params.num_massive_bodies,
        pad1: 0.0,
        massive_bodies: params.massive_bodies,
        massive_body_velocities: params.massive_body_velocities,
    };

    queue.write_buffer(&engine.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

    // Drain staging
    for i in 0..2 {
        if let (Some(state), Some(staging_buf)) =
            (engine.staging_states.get(i), engine.staging_buffers.get(i))
        {
            if state.load(Ordering::Acquire) == STAGING_STATE_MAPPED {
                if let Some(sender) = sender.as_ref() {
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

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Galaxy Compute Encoder"),
    });

    let workgroups = engine.num_particles.div_ceil(64);
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Galaxy Compute Pass"),
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

    let _ = render_device.poll(wgpu::PollType::Poll);
}
