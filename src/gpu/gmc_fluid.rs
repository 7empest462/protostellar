//! 3D Eulerian WGPU Compute Fluid Simulation for Giant Molecular Clouds (GMC).
//!
//! Simulates a 96^3 grid (884,736 cells) spanning 1100 AU with Semi-Lagrangian advection,
//! pressure gradients, stellar point-mass gravity, turbulent decay, and Jeans collapse peak detection.

use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp};
use bytemuck::{Pod, Zeroable};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::rendering::effects::remnants::PersistentRemnantPool;
use crate::rendering::effects::supernova::SupernovaDebrisPool;
use crate::simulation::components::*;
use crate::simulation::resources::{SimTime, TimeWarp};
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::G_ASTRO;

pub const GMC_DOMAIN_SIZE_AU: f32 = 1100.0;
pub const GMC_GRID_DIM: u32 = 96;
pub const GMC_TOTAL_CELLS: usize = (GMC_GRID_DIM * GMC_GRID_DIM * GMC_GRID_DIM) as usize;
pub const GMC_CELL_DX_AU: f32 = GMC_DOMAIN_SIZE_AU / GMC_GRID_DIM as f32;
pub const GMC_CELL_VOLUME_AU3: f32 = GMC_CELL_DX_AU * GMC_CELL_DX_AU * GMC_CELL_DX_AU;

pub const STAGING_STATE_IDLE: u8 = 0;
pub const STAGING_STATE_MAPPING: u8 = 1;
pub const STAGING_STATE_MAPPED: u8 = 2;

/// CPU -> GPU sink particle for mass accretion and radiation pressure cavity clearing.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct GpuSinkParticle {
    pub world_pos: [f32; 3],
    pub sink_radius_au: f32,
    pub radiation_pressure_factor: f32,
    pub is_ignited: u32,
    pub mass_solar: f32,
    #[allow(clippy::pub_underscore_fields, reason = "WGSL memory layout alignment")]
    pub _pad: u32,
}

impl Default for GpuSinkParticle {
    fn default() -> Self {
        Self {
            world_pos: [0.0, 0.0, 0.0],
            sink_radius_au: 15.0,
            radiation_pressure_factor: 1.0,
            is_ignited: 0,
            mass_solar: 1.0,
            _pad: 0,
        }
    }
}

/// Active expanding supernova blast wave for fluid shock compression and nucleosynthetic enrichment.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct GpuSupernovaBlast {
    pub world_pos: [f32; 3],
    pub current_radius_au: f32,
    pub metals_mass_solar: f32,
    pub blast_speed_au_s: f32,
    pub ejecta_mass_solar: f32,
    pub is_active: u32,
}

impl Default for GpuSupernovaBlast {
    fn default() -> Self {
        Self {
            world_pos: [0.0, 0.0, 0.0],
            current_radius_au: 0.0,
            metals_mass_solar: 0.0,
            blast_speed_au_s: 0.0,
            ejecta_mass_solar: 0.0,
            is_active: 0,
        }
    }
}

/// GPU -> CPU Jeans collapse event signaling structure.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct GpuJeansCollapseEvent {
    pub grid_coords: [u32; 3],
    pub metallicity: f32,
    pub world_pos: [f32; 3],
    pub local_mass_solar: f32,
    pub com_velocity: [f32; 3],
    pub temperature_k: f32,
}

impl Default for GpuJeansCollapseEvent {
    fn default() -> Self {
        Self {
            grid_coords: [48, 48, 48],
            metallicity: 0.0,
            world_pos: [0.0, 0.0, 0.0],
            local_mass_solar: 1.0,
            com_velocity: [0.0, 0.0, 0.0],
            temperature_k: 15.0,
        }
    }
}

/// Uniforms buffer layout for the 3D GMC compute shader (816 bytes, 16-byte aligned).
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GmcFluidUniforms {
    pub domain_size: f32,
    pub grid_dim: u32,
    pub dt: f32,
    pub sound_speed: f32,
    pub damping: f32,
    pub num_sinks: u32,
    pub ambient_temp: f32,
    pub g_astro: f32,
    pub collapse_threshold: f32,
    pub vacuum_rate: f32,
    pub num_supernovae: u32,
    pub elapsed_years: f32,
    pub sinks: [GpuSinkParticle; 16],
    pub supernovae: [GpuSupernovaBlast; 8],
}

impl Default for GmcFluidUniforms {
    fn default() -> Self {
        Self {
            domain_size: GMC_DOMAIN_SIZE_AU,
            grid_dim: GMC_GRID_DIM,
            dt: 0.001,
            sound_speed: 0.058, // ~0.274 km/s in AU/yr
            damping: 0.9995,
            num_sinks: 0,
            ambient_temp: 15.0,
            g_astro: G_ASTRO as f32,
            collapse_threshold: 6.5e-11, // Lower threshold allows gas to condense and ignite naturally
            vacuum_rate: 1.0e-11,
            num_supernovae: 0,
            elapsed_years: 0.0,
            sinks: [GpuSinkParticle::default(); 16],
            supernovae: [GpuSupernovaBlast::default(); 8],
        }
    }
}

/// Holds WGPU compute pipeline resources and ping-pong 3D grid buffers in the RenderApp.
#[derive(Resource)]
pub struct GpuGmcFluidEngine {
    pub density_buffers: [wgpu::Buffer; 2],
    pub velocity_buffers: [wgpu::Buffer; 2],
    pub temperature_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub collapse_event_buffer: wgpu::Buffer,
    pub collapse_counter_buffer: wgpu::Buffer,
    pub staging_buffer: wgpu::Buffer,
    pub staging_state: Arc<AtomicU8>,
    pub bind_groups: [wgpu::BindGroup; 2],
    pub pipeline: wgpu::ComputePipeline,
    pub ping_pong_idx: usize,
    pub is_initialized: bool,
}

/// Extracted parameters passed from Main App to RenderApp every frame.
#[derive(Resource, Default, Clone)]
pub struct GmcFluidExtractedParams {
    pub is_active: bool,
    pub is_paused: bool,
    pub dt: f32,
    pub elapsed_years: f64,
    pub sinks: [GpuSinkParticle; 16],
    pub num_sinks: u32,
    pub supernovae: [GpuSupernovaBlast; 8],
    pub num_supernovae: u32,
}

/// Receiver in Main App for GPU-to-CPU collapse events.
#[derive(Resource)]
pub struct GmcCollapseEventReceiver {
    pub rx: flume::Receiver<Vec<GpuJeansCollapseEvent>>,
}

/// Sender in RenderApp for GPU-to-CPU collapse events.
#[derive(Resource)]
pub struct GmcCollapseEventSender {
    pub tx: flume::Sender<Vec<GpuJeansCollapseEvent>>,
}

/// Plugin registering GMC fluid compute passes and cross-world event handling.
pub struct GmcFluidPlugin;

impl Plugin for GmcFluidPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = flume::bounded::<Vec<GpuJeansCollapseEvent>>(4);

        app.insert_resource(GmcCollapseEventReceiver { rx });
        app.add_systems(Update, super::gmc_collapse::receive_gmc_collapse_events);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.insert_resource(GmcCollapseEventSender { tx });
        render_app.init_resource::<GmcFluidExtractedParams>();
        render_app
            .add_systems(ExtractSchedule, extract_gmc_fluid_data)
            .add_systems(Render, step_gmc_fluid_simulation);
    }
}

/// Prioritizes and formats up to 16 sink particles for the GMC fluid simulation.
/// Sinks are sorted by mass descending so massive black holes and giant stars always take priority
/// over lower-mass protostars. Remnants do not radiate or vacuum gas.
pub fn build_gmc_sinks<'a, I>(candidate_bodies: I) -> ([GpuSinkParticle; 16], u32)
where
    I: IntoIterator<
        Item = (
            &'a SimPosition,
            &'a Mass,
            &'a Luminosity,
            Option<&'a IgnitionState>,
            &'a CelestialBody,
        ),
    >,
{
    let mut candidates: Vec<(
        &SimPosition,
        &Mass,
        &Luminosity,
        Option<&IgnitionState>,
        &CelestialBody,
    )> = candidate_bodies
        .into_iter()
        .filter(|(_, _, _, _, body)| body.body_type.is_star_or_remnant())
        .collect();

    // Sort by mass descending
    candidates.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0));

    let mut sinks = [GpuSinkParticle::default(); 16];
    let mut num_sinks = 0u32;

    for (pos, mass, lum, opt_ign, body) in candidates.into_iter().take(16) {
        let is_remnant = body.body_type.is_remnant();
        let is_ignited = !is_remnant && opt_ign.is_some_and(|ign| ign.is_ignited);
        let rad_factor = if is_remnant {
            0.0
        } else {
            (lum.0 as f32).clamp(0.0, 100.0)
        };
        let sink_radius_au = if is_remnant {
            3.0
        } else {
            (25.0 * rad_factor.cbrt().max(1.0)).clamp(10.0, 120.0)
        };

        if let Some(sink_slot) = sinks.get_mut(num_sinks as usize) {
            *sink_slot = GpuSinkParticle {
                world_pos: [pos.x as f32, pos.y as f32, pos.z as f32],
                sink_radius_au,
                radiation_pressure_factor: rad_factor,
                is_ignited: u32::from(is_ignited),
                mass_solar: mass.0 as f32,
                _pad: 0,
            };
            num_sinks += 1;
        }
    }

    (sinks, num_sinks)
}

/// Extracts GMC simulation parameters and protostellar sinks from Main App to RenderApp.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Bevy Extract Query tuple and multi-subsystem simulation parameters"
)]
pub fn extract_gmc_fluid_data(
    mut commands: Commands,
    scenario_state: Extract<Option<Res<ActiveScenarioState>>>,
    time_warp: Extract<Res<TimeWarp>>,
    sim_time: Extract<Res<SimTime>>,
    debris_pool: Extract<Option<Res<SupernovaDebrisPool>>>,
    remnant_pool: Extract<Option<Res<PersistentRemnantPool>>>,
    sink_query: Extract<
        Query<
            (
                &SimPosition,
                &Mass,
                &Luminosity,
                Option<&IgnitionState>,
                &CelestialBody,
            ),
            With<CelestialBody>,
        >,
    >,
) {
    let is_active = scenario_state
        .as_ref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);

    let is_paused = time_warp.is_paused && !time_warp.step_once;
    let dt = sim_time.current_dt_yr as f32;
    let elapsed_years = sim_time.elapsed_years;

    let mut sinks = [GpuSinkParticle::default(); 16];
    let mut num_sinks = 0u32;
    let mut supernovae = [GpuSupernovaBlast::default(); 8];
    let mut num_supernovae = 0u32;

    if is_active {
        let (extracted_sinks, extracted_num) = build_gmc_sinks(sink_query.iter());
        sinks = extracted_sinks;
        num_sinks = extracted_num;

        // Collect prompt supernova explosions
        if let Some(debris) = debris_pool.as_ref() {
            for exp in &debris.explosions {
                if num_supernovae >= 8 {
                    break;
                }
                if let Some(sn_slot) = supernovae.get_mut(num_supernovae as usize) {
                    *sn_slot = GpuSupernovaBlast {
                        world_pos: [exp.center.x, exp.center.y, exp.center.z],
                        current_radius_au: exp.current_radius_au,
                        metals_mass_solar: exp.metals_mass_solar,
                        blast_speed_au_s: exp.blast_speed_au_s,
                        ejecta_mass_solar: exp.ejecta_mass_solar,
                        is_active: 1,
                    };
                    num_supernovae += 1;
                }
            }
        }

        // Collect persistent remnant blast shells
        if let Some(remnants) = remnant_pool.as_ref() {
            for rem in &remnants.remnants {
                if num_supernovae >= 8 {
                    break;
                }
                if let Some(sn_slot) = supernovae.get_mut(num_supernovae as usize) {
                    *sn_slot = GpuSupernovaBlast {
                        world_pos: [rem.center.x, rem.center.y, rem.center.z],
                        current_radius_au: rem.current_radius_au,
                        metals_mass_solar: rem.metals_mass_solar,
                        blast_speed_au_s: rem.expansion_rate_au_yr * 0.1,
                        ejecta_mass_solar: rem.ejecta_mass_solar,
                        is_active: 1,
                    };
                    num_supernovae += 1;
                }
            }
        }
    }

    commands.insert_resource(GmcFluidExtractedParams {
        is_active,
        is_paused,
        dt,
        elapsed_years,
        sinks,
        num_sinks,
        supernovae,
        num_supernovae,
    });
}

/// Builds initial density, velocity, and temperature fields for the GMC core.
fn generate_initial_gmc_fields() -> (Vec<f32>, Vec<[f32; 4]>, Vec<f32>) {
    let mut densities = Vec::with_capacity(GMC_TOTAL_CELLS);
    let mut velocities = Vec::with_capacity(GMC_TOTAL_CELLS);
    let mut temperatures = Vec::with_capacity(GMC_TOTAL_CELLS);

    let half_domain = GMC_DOMAIN_SIZE_AU * 0.5;
    let dx = GMC_CELL_DX_AU;

    for z in 0..GMC_GRID_DIM {
        for y in 0..GMC_GRID_DIM {
            for x in 0..GMC_GRID_DIM {
                let wx = (x as f32 + 0.5) * dx - half_domain;
                let wy = (y as f32 + 0.5) * dx - half_domain;
                let wz = (z as f32 + 0.5) * dx - half_domain;
                let r_au = (wx * wx + wy * wy + wz * wz).sqrt();

                // Extended GMC Disk with multiple dense nodes
                let r_cyl = (wx * wx + wz * wz).sqrt().max(0.1);

                // Base exponential disk profile
                let disk_scale_r = 250.0;
                let disk_scale_h = 45.0;
                let rho_disk =
                    1.8e-11 * f32::exp(-r_cyl / disk_scale_r) * f32::exp(-wy.abs() / disk_scale_h);

                // Add several offset clumps/nodes
                let c1_dist = ((wx - 150.0).powi(2) + wy.powi(2) + (wz + 120.0).powi(2)).sqrt();
                let c2_dist = ((wx + 180.0).powi(2) + wy.powi(2) + (wz - 80.0).powi(2)).sqrt();
                let c3_dist = ((wx - 80.0).powi(2) + wy.powi(2) + (wz - 200.0).powi(2)).sqrt();
                let c4_dist = ((wx + 50.0).powi(2) + wy.powi(2) + (wz + 240.0).powi(2)).sqrt();

                let clump1 = 2.0e-11 / (1.0 + (c1_dist / 60.0).powi(2)).powf(1.5);
                let clump2 = 1.8e-11 / (1.0 + (c2_dist / 70.0).powi(2)).powf(1.5);
                let clump3 = 1.5e-11 / (1.0 + (c3_dist / 65.0).powi(2)).powf(1.5);
                let clump4 = 1.7e-11 / (1.0 + (c4_dist / 55.0).powi(2)).powf(1.5);

                let central_core = 2.5e-11 / (1.0 + (r_au / 80.0).powi(2)).powf(1.5);

                let base_rho = rho_disk + central_core + clump1 + clump2 + clump3 + clump4;

                // Supersonic turbulent density fluctuations (filaments)
                let turb_rho = 1.0
                    + 0.25 * (wx * 0.03).sin() * (wz * 0.02).cos()
                    + 0.20 * (wy * 0.04).cos() * (wx * 0.025).sin()
                    + 0.15 * (wz * 0.035).sin() * (wy * 0.03).cos();
                let rho = (base_rho * turb_rho.max(0.1)).max(1.0e-14);

                // Rotational velocity (Keplerian-ish) + Turbulence
                let phase_x = (wx * 0.025).sin() * (wz * 0.018).cos();
                let phase_y = (wy * 0.022).cos() * (wx * 0.015).sin();
                let phase_z = (wz * 0.028).sin() * (wy * 0.020).cos();

                // Balance the global inward pull (g_cloud + NFW Dark Matter halo)
                let v_rot = if r_cyl > 15.0 {
                    let r_s = 150.0;
                    let x = r_cyl / r_s;
                    let mass_dm = 8000.0 * ((1.0 + x).ln() - x / (1.0 + x));
                    let total_mass = 40.0 + mass_dm;
                    (crate::utils::constants::G_ASTRO as f32 * total_mass / r_cyl).sqrt() * 0.95
                } else {
                    0.0
                };

                let vx = (-wz / r_cyl * v_rot) + phase_x * 0.044;
                let vy = phase_y * 0.020;
                let vz = (wx / r_cyl * v_rot) + phase_z * 0.040;

                densities.push(rho);
                velocities.push([vx, vy, vz, 0.0]);
                temperatures.push(15.0);
            }
        }
    }

    (densities, velocities, temperatures)
}

struct GmcFluidBuffers {
    density: [wgpu::Buffer; 2],
    velocity: [wgpu::Buffer; 2],
    temperature: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    collapse_event: wgpu::Buffer,
    collapse_counter: wgpu::Buffer,
    staging: wgpu::Buffer,
}

fn create_gmc_fluid_buffers(device: &wgpu::Device) -> GmcFluidBuffers {
    let (init_d, init_v, init_t) = generate_initial_gmc_fields();

    let d_buffer_0 = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GMC Density Buffer 0"),
        contents: bytemuck::cast_slice(&init_d),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });
    let d_buffer_1 = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GMC Density Buffer 1"),
        contents: bytemuck::cast_slice(&init_d),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });

    let v_buffer_0 = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GMC Velocity Buffer 0"),
        contents: bytemuck::cast_slice(&init_v),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let v_buffer_1 = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GMC Velocity Buffer 1"),
        contents: bytemuck::cast_slice(&init_v),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let t_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GMC Temperature Buffer"),
        contents: bytemuck::cast_slice(&init_t),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let initial_uniforms = GmcFluidUniforms::default();
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GMC Fluid Uniform Buffer"),
        contents: bytemuck::bytes_of(&initial_uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let collapse_event_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("GMC Collapse Event Buffer"),
        size: 32 * std::mem::size_of::<GpuJeansCollapseEvent>() as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let initial_counter = [0u32];
    let collapse_counter_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("GMC Collapse Counter Buffer"),
        contents: bytemuck::cast_slice(&initial_counter),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("GMC Staging Readback Buffer"),
        size: 16 + 32 * std::mem::size_of::<GpuJeansCollapseEvent>() as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    GmcFluidBuffers {
        density: [d_buffer_0, d_buffer_1],
        velocity: [v_buffer_0, v_buffer_1],
        temperature: t_buffer,
        uniforms: uniform_buffer,
        collapse_event: collapse_event_buffer,
        collapse_counter: collapse_counter_buffer,
        staging: staging_buffer,
    }
}

fn create_gmc_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("GMC Fluid Bind Group Layout"),
        entries: &[
            // 0: density_in
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
            // 1: density_out
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 2: velocity_in
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
            // 3: velocity_out
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 4: temperature
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 5: uniforms
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 6: collapse_events
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 7: collapse_counter
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn create_gmc_compute_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let shader_source = include_str!("../../assets/shaders/gmc_fluid.wgsl");
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("GMC Fluid Compute Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("GMC Fluid Pipeline Layout"),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("GMC Fluid Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: default(),
        cache: None,
    })
}

fn create_gmc_bind_groups(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: &GmcFluidBuffers,
) -> [wgpu::BindGroup; 2] {
    let bind_group_0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("GMC Fluid Bind Group 0 (Read 0, Write 1)"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.density[0].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: buffers.density[1].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buffers.velocity[0].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: buffers.velocity[1].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: buffers.temperature.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: buffers.uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: buffers.collapse_event.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: buffers.collapse_counter.as_entire_binding(),
            },
        ],
    });

    let bind_group_1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("GMC Fluid Bind Group 1 (Read 1, Write 0)"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.density[1].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: buffers.density[0].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buffers.velocity[1].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: buffers.velocity[0].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: buffers.temperature.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: buffers.uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: buffers.collapse_event.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: buffers.collapse_counter.as_entire_binding(),
            },
        ],
    });

    [bind_group_0, bind_group_1]
}

/// Compiles the GMC fluid compute pipeline and sets up ping-pong buffers.
fn setup_gmc_fluid_pipeline(commands: &mut Commands, device: &wgpu::Device) {
    let buffers = create_gmc_fluid_buffers(device);
    let bind_group_layout = create_gmc_bind_group_layout(device);
    let pipeline = create_gmc_compute_pipeline(device, &bind_group_layout);
    let bind_groups = create_gmc_bind_groups(device, &bind_group_layout, &buffers);

    commands.insert_resource(GpuGmcFluidEngine {
        density_buffers: buffers.density,
        velocity_buffers: buffers.velocity,
        temperature_buffer: buffers.temperature,
        uniform_buffer: buffers.uniforms,
        collapse_event_buffer: buffers.collapse_event,
        collapse_counter_buffer: buffers.collapse_counter,
        staging_buffer: buffers.staging,
        staging_state: Arc::new(AtomicU8::new(STAGING_STATE_IDLE)),
        bind_groups,
        pipeline,
        ping_pong_idx: 0,
        is_initialized: true,
    });
}

fn drain_mapped_gmc_staging_buffer(engine: &GpuGmcFluidEngine, sender: &GmcCollapseEventSender) {
    if engine.staging_state.load(Ordering::Acquire) == STAGING_STATE_MAPPED {
        let slice = engine.staging_buffer.slice(..);
        let view = slice.get_mapped_range();
        let num_events = view
            .get(..4)
            .map_or(0u32, |bytes| *bytemuck::from_bytes::<u32>(bytes));
        let count = (num_events as usize).min(32);

        if count > 0 {
            let bytes_len = count * std::mem::size_of::<GpuJeansCollapseEvent>();
            if let Some(event_bytes) = view.get(16..16 + bytes_len) {
                let events: &[GpuJeansCollapseEvent] = bytemuck::cast_slice(event_bytes);
                let _ = sender.tx.try_send(events.to_vec());
            }
        }

        drop(view);
        engine.staging_buffer.unmap();
        engine
            .staging_state
            .store(STAGING_STATE_IDLE, Ordering::Release);
    }
}

/// Steps the GMC 3D fluid simulation in the RenderApp.
pub fn step_gmc_fluid_simulation(
    mut commands: Commands,
    engine_opt: Option<ResMut<GpuGmcFluidEngine>>,
    params: Res<GmcFluidExtractedParams>,
    render_dev: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    sender: Res<GmcCollapseEventSender>,
) {
    if !params.is_active {
        return;
    }

    let Some(mut engine) = engine_opt else {
        setup_gmc_fluid_pipeline(&mut commands, render_dev.wgpu_device());
        return;
    };

    if params.is_paused {
        return;
    }

    let device = render_dev.wgpu_device();
    let queue = &render_queue;

    // 1. Drain previously mapped staging buffer if ready
    drain_mapped_gmc_staging_buffer(&engine, &sender);

    // 2. Update uniforms buffer (calculate substepping for time warp scaling)
    // Fluid Eulerian grid requires bounded CFL step (<= 0.12 yr) so density filaments
    // condense coherently into Jeans collapse peaks even at extreme fast-forward speeds.
    let num_substeps = if params.dt > 0.10 { 2 } else { 1 };
    let sub_dt = (params.dt / num_substeps as f32).clamp(0.0001, 0.12);

    let uniforms = GmcFluidUniforms {
        dt: sub_dt,
        num_sinks: params.num_sinks.min(16),
        sinks: params.sinks,
        num_supernovae: params.num_supernovae.min(8),
        supernovae: params.supernovae,
        elapsed_years: params.elapsed_years as f32,
        ..Default::default()
    };

    queue.write_buffer(&engine.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

    // Reset collapse counter on GPU
    let zero_counter = [0u32];
    queue.write_buffer(
        &engine.collapse_counter_buffer,
        0,
        bytemuck::cast_slice(&zero_counter),
    );

    // 3. Dispatch compute passes (substepping across ping-pong buffers)
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("GMC Fluid Compute Encoder"),
    });

    for _ in 0..num_substeps {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("GMC Fluid Compute Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&engine.pipeline);
        if let Some(bind_group) = engine.bind_groups.get(engine.ping_pong_idx) {
            pass.set_bind_group(0, bind_group, &[]);
        }

        // 96 / 8 = 12 workgroups along each axis
        pass.dispatch_workgroups(12, 12, 12);
        drop(pass);

        // Ping-pong buffer indices between consecutive substeps
        engine.ping_pong_idx = 1 - engine.ping_pong_idx;
    }

    // 4. Asynchronous staging readback (safe zero-stall handoff)
    let mut copy_initiated = false;
    if engine
        .staging_state
        .compare_exchange(
            STAGING_STATE_IDLE,
            STAGING_STATE_MAPPING,
            Ordering::AcqRel,
            Ordering::Relaxed,
        )
        .is_ok()
    {
        encoder.copy_buffer_to_buffer(
            &engine.collapse_counter_buffer,
            0,
            &engine.staging_buffer,
            0,
            4,
        );
        encoder.copy_buffer_to_buffer(
            &engine.collapse_event_buffer,
            0,
            &engine.staging_buffer,
            16,
            32 * std::mem::size_of::<GpuJeansCollapseEvent>() as u64,
        );
        copy_initiated = true;
    }

    // Submit compute and (if initiated) copy commands to queue
    queue.submit(Some(encoder.finish()));

    // Map the staging buffer AFTER queue.submit has been executed
    if copy_initiated {
        let state_clone = Arc::clone(&engine.staging_state);
        engine
            .staging_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    state_clone.store(STAGING_STATE_MAPPED, Ordering::Release);
                } else {
                    state_clone.store(STAGING_STATE_IDLE, Ordering::Release);
                }
            });
    }

    // Drive WGPU submission queue to process map_async callbacks and eliminate shutdown timeout
    let _ = render_dev.poll(wgpu::PollType::Poll);
}
