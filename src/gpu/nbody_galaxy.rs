use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::gpu::buffers::{
    GPU_COMPACT_READBACK_MAGIC, GpuCompactReadback, GpuParticle, MassiveBodyGpu,
};
use crate::gpu::compute_node::{MAX_GPU_PARTICLES, send_compact_readback};
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::scenarios::ScenarioReloadGeneration;
use crate::utils::constants::*;
use crate::utils::math::FastRngExt;

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
    pub massive_body_velocities: [[f32; 4]; 32], // xyz: vel, w: halo_mass
}

/// GPU-resident N-body galaxy simulation engine rendered directly via GPU Vertex Pulling
/// (zero 24 MB staging buffers and zero GPU -> CPU particle readback).
#[derive(Resource)]
pub struct GpuGalaxyOrbitEngine {
    pub particle_buffer: wgpu::Buffer,
    pub buffer_uid: u64,
    pub uniform_buffer: wgpu::Buffer,
    pub restart_epoch: u64,
    pub last_reload_generation: u64,
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
    pub enable_gpu_compute: bool,
    pub is_swarm_visible: bool,
    pub particle_render_scale: f32,
    pub outer_radius: f32,
    pub dt: f32,
    pub elapsed_years: f64,
    pub reload_generation: u64,
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
    clippy::too_many_arguments,
    reason = "Bevy Extract Query tuple and multi-subsystem simulation parameters"
)]
pub fn extract_galaxy_sim_data(
    mut commands: Commands,
    config: Extract<Res<SimulationConfig>>,
    disk_params: Extract<Res<DiskParameters>>,
    time_warp: Extract<Res<TimeWarp>>,
    scenario_state: Extract<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    reload_gen: Extract<Option<Res<ScenarioReloadGeneration>>>,
    sim_time: Extract<Res<SimTime>>,
    massive_query: Extract<
        Query<(
            &SimPosition,
            &Mass,
            (Entity, Option<&SimVelocity>, &CelestialBody),
        )>,
    >,
    swarm_vis_query: Extract<
        Query<&Visibility, With<crate::rendering::particle_swarm::ParticleSwarmMesh>>,
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

    let smbh_count = massive_query
        .iter()
        .filter(|(_, m, (_, _, b))| b.body_type == BodyType::BlackHole && m.0 >= 100_000.0)
        .count();
    for (pos, mass, (_, vel_maybe, body)) in massive_query.iter() {
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
            let halo_m = if body.body_type != BodyType::BlackHole || mass.0 < 100_000.0 {
                0.0_f32
            } else if smbh_count <= 1 {
                2.4e12_f32 // Merged Milkomeda halo
            } else if mass.0 > 50_000_000.0 {
                1.4e12_f32 // Andromeda halo
            } else {
                1.0e12_f32 // Milky Way halo
            };
            *vel_slot = [v.x as f32, v.y as f32, v.z as f32, halo_m];
            num_bodies += 1;
        } else {
            break;
        }
    }

    let dt = sim_time.current_dt_yr as f32;
    let count = swarm.as_ref().map_or(500_000, |s| s.count as u32);
    let is_swarm_visible = swarm_vis_query
        .iter()
        .next()
        .is_none_or(|v| *v != Visibility::Hidden);

    commands.insert_resource(GpuGalaxyExtractedParams {
        count,
        is_paused: time_warp.is_paused,
        step_once: time_warp.step_once,
        enable_gpu_compute: config.enable_gpu_compute,
        is_swarm_visible,
        particle_render_scale: config.particle_render_scale,
        outer_radius: disk_params.outer_radius_au as f32,
        dt,
        elapsed_years: sim_time.elapsed_years,
        reload_generation: reload_gen.as_ref().map_or(0, |g| g.0),
        num_massive_bodies: num_bodies,
        massive_bodies,
        massive_body_velocities,
        halo_mass: 1.0e12,
        halo_scale_radius: 50_000.0,
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
    base_col: [f32; 3],
}

fn generate_galaxy_particles(cfg: &GalaxyDiskConfig, initial_particles: &mut Vec<GpuParticle>) {
    let mut rng = fastrand::Rng::new();
    let rs = 50_000.0_f32;
    for _ in 0..cfg.count {
        let r = rng.f32_range(5000.0, cfg.max_radius);
        let phi = rng.f32_range(0.0, std::f32::consts::TAU);
        let z = rng.f32_range(-500.0, 500.0);
        let pos = [
            cfg.center_pos[0] + r * phi.cos(),
            cfg.center_pos[1] + z,
            cfg.center_pos[2] + r * phi.sin(),
            1.0,
        ];

        let x = r / rs;
        let mass_enclosed = cfg.halo_mass * ((1.0 + x).ln() - x / (1.0 + x));
        let v_circ = (G_ASTRO as f32 * (cfg.smbh_mass + mass_enclosed) / r).sqrt();

        let vel = [
            cfg.center_vel[0] - v_circ * phi.sin(),
            cfg.center_vel[1],
            cfg.center_vel[2] + v_circ * phi.cos(),
            5000.0,
        ];
        let col_var = rng.f32_range(0.85, 1.15);
        // Pack star RGB color directly into `composition` (`w = 1.0` sentinel) for GPU vertex pulling
        let color = [
            (cfg.base_col[0] * col_var).clamp(0.2, 1.5),
            (cfg.base_col[1] * col_var).clamp(0.2, 1.5),
            (cfg.base_col[2] * col_var).clamp(0.2, 1.5),
            1.0,
        ];
        initial_particles.push(GpuParticle {
            pos_mass: pos,
            vel_temp: vel,
            composition: color,
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
            base_col: [1.0, 0.95, 0.85],
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
            base_col: [0.85, 0.92, 1.20],
        },
        &mut particles,
    );

    particles
}

fn generate_colliding_galaxies_padded(active_particles: u32, capacity: u32) -> Vec<GpuParticle> {
    let active = active_particles.min(capacity);
    let mut particles = generate_colliding_galaxies(active);
    particles.resize(
        capacity as usize,
        GpuParticle {
            pos_mass: [0.0, -5000.0, 0.0, 0.0],
            vel_temp: [0.0, 0.0, 0.0, 0.0],
            composition: [0.0, 0.0, 0.0, 0.0],
        },
    );
    particles
}

pub fn setup_galaxy_gpu_simulation(
    commands: &mut Commands,
    render_dev: &RenderDevice,
    count: u32,
    reload_generation: u64,
) {
    let device = render_dev.wgpu_device();
    let n_particles = count.clamp(50_000, MAX_GPU_PARTICLES);
    let initial_particles = generate_colliding_galaxies_padded(n_particles, MAX_GPU_PARTICLES);

    let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Galaxy Particle Buffer"),
        contents: bytemuck::cast_slice(&initial_particles),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let initial_uniforms = GalaxyUniforms {
        dt: 0.0,
        g_const: G_ASTRO as f32,
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

    commands.insert_resource(GpuGalaxyOrbitEngine {
        particle_buffer,
        buffer_uid: crate::gpu::particle_pipeline::next_gpu_buffer_uid(),
        uniform_buffer,
        restart_epoch: 0,
        last_reload_generation: reload_generation,
        bind_group,
        pipeline,
        num_particles: n_particles,
        is_ready: true,
        last_elapsed_years: 0.0,
    });
}

fn dispatch_galaxy_substeps(
    device: &wgpu::Device,
    queue: &RenderQueue,
    engine: &GpuGalaxyOrbitEngine,
    params: &GpuGalaxyExtractedParams,
) {
    // Subdivide high-dt frames on the host so the shader's 32-substep TDR cap never forces
    // coarse steps near Andromeda's SMBH (each dispatch handles <= 0.25 yr).
    let n_dispatches = ((params.dt.abs() / 0.25).ceil() as u32).clamp(1, 16);
    let dispatch_dt = params.dt / (n_dispatches as f32);
    let workgroups = engine.num_particles.div_ceil(64);

    for sub_idx in 0..n_dispatches {
        let elapsed_offset = (sub_idx as f32) * dispatch_dt;
        let mut shifted_bodies = params.massive_bodies;
        for (body, vel) in shifted_bodies
            .iter_mut()
            .zip(&params.massive_body_velocities)
            .take(params.num_massive_bodies as usize)
        {
            body.pos_mass[0] += vel[0] * elapsed_offset;
            body.pos_mass[1] += vel[1] * elapsed_offset;
            body.pos_mass[2] += vel[2] * elapsed_offset;
        }

        let uniforms = GalaxyUniforms {
            dt: dispatch_dt,
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
            massive_bodies: shifted_bodies,
            massive_body_velocities: params.massive_body_velocities,
        };
        queue.write_buffer(&engine.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Galaxy Compute Encoder"),
        });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Galaxy Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&engine.pipeline);
            compute_pass.set_bind_group(0, &engine.bind_group, &[]);
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }
        queue.submit(Some(encoder.finish()));
    }
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
    if !params.enable_gpu_compute {
        return;
    }

    let Some(mut engine) = gpu_engine else {
        setup_galaxy_gpu_simulation(
            &mut commands,
            &render_device,
            params.count,
            params.reload_generation,
        );
        return;
    };

    let clamped_count = params.count.clamp(50_000, MAX_GPU_PARTICLES);
    let count_changed = params.count > 0 && clamped_count != engine.num_particles;
    let scenario_restarted = params.reload_generation != engine.last_reload_generation
        || params.elapsed_years < engine.last_elapsed_years;
    engine.last_reload_generation = params.reload_generation;
    engine.last_elapsed_years = params.elapsed_years;

    let queue = &render_queue;
    if scenario_restarted || count_changed {
        engine.num_particles = clamped_count;
        engine.restart_epoch = engine.restart_epoch.wrapping_add(1);
        let initial_particles =
            generate_colliding_galaxies_padded(engine.num_particles, MAX_GPU_PARTICLES);
        queue.write_buffer(
            &engine.particle_buffer,
            0,
            bytemuck::cast_slice(&initial_particles),
        );
    }

    // Lightweight 144-byte heartbeat keeps CPU `gpu_compute_active` and HUD active count synced
    // with zero 24 MB GPU -> CPU particle readback.
    if let Some(ref s) = sender {
        send_compact_readback(
            s,
            &GpuCompactReadback {
                magic: GPU_COMPACT_READBACK_MAGIC,
                active_particles: engine.num_particles,
                is_galaxy: 1,
                generation_lo: engine.last_reload_generation as u32,
                accreted_per_body: [0; 32],
            },
        );
    }

    if (params.is_paused && !params.step_once) || !engine.is_ready {
        return;
    }

    dispatch_galaxy_substeps(render_device.wgpu_device(), queue, &engine, &params);
}
