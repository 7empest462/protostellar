//! E2E Test Harness for Protostellar Phase 4: Giant Molecular Clouds & Star Formation.
//!
//! Provides mock structures, astrophysical math models, and headless Bevy ECS test harnesses
//! derived strictly from PROJECT.md interface contracts and ORIGINAL_REQUEST.md requirements.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Domain size of the GMC Eulerian fluid grid in AU (1100 AU).
pub const GMC_DOMAIN_SIZE_AU: f32 = 1100.0;

/// Number of cells per axis in the 3D Eulerian grid (96^3).
pub const GMC_GRID_DIM: u32 = 96;

/// Total number of cells in the 3D Eulerian fluid grid (96^3 = 884,736).
pub const GMC_TOTAL_CELLS: usize = (GMC_GRID_DIM * GMC_GRID_DIM * GMC_GRID_DIM) as usize;

/// Physical cell width dx in AU (~11.4583 AU).
pub const GMC_CELL_DX_AU: f32 = GMC_DOMAIN_SIZE_AU / GMC_GRID_DIM as f32;

/// Physical volume of a single grid cell in AU^3.
pub const GMC_CELL_VOLUME_AU3: f32 = GMC_CELL_DX_AU * GMC_CELL_DX_AU * GMC_CELL_DX_AU;

/// Hydrogen mass in kg for sound speed derivations.
pub const M_H_KG: f64 = 1.673_557_5e-27;

/// Boltzmann constant in J/K.
pub const K_B_SI: f64 = 1.380_649e-23;

/// Mean molecular weight for cold molecular cloud gas (H2 + He).
pub const MU_MOLECULAR: f64 = 2.3;

/// Adiabatic index for molecular hydrogen at low temperatures (diatomic gas).
pub const GAMMA_GAS: f64 = 1.4;

/// Protostellar core hydrogen ignition temperature in Kelvin (10 MK).
pub const PROTOSTAR_IGNITION_TEMP_K: f64 = 1.0e7;

/// GPU -> CPU Jeans collapse event signaling structure (Interface Contract 3).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GpuJeansCollapseEvent {
    pub grid_coords: [u32; 3],
    pub _pad0: u32,
    pub world_pos: [f32; 3],
    pub local_mass_solar: f32,
    pub com_velocity: [f32; 3],
    pub temperature_k: f32,
}

impl Default for GpuJeansCollapseEvent {
    fn default() -> Self {
        Self {
            grid_coords: [48, 48, 48],
            _pad0: 0,
            world_pos: [0.0, 0.0, 0.0],
            local_mass_solar: 1.0,
            com_velocity: [0.0, 0.0, 0.0],
            temperature_k: 15.0,
        }
    }
}

/// CPU -> GPU sink particle for gas mass vacuuming and radiation pressure (Interface Contract 3).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GpuSinkParticle {
    pub world_pos: [f32; 3],
    pub sink_radius_au: f32,
    pub radiation_pressure_factor: f32,
    pub is_ignited: u32,
    pub _pad: [u32; 2],
}

impl Default for GpuSinkParticle {
    fn default() -> Self {
        Self {
            world_pos: [0.0, 0.0, 0.0],
            sink_radius_au: 15.0,
            radiation_pressure_factor: 1.0,
            is_ignited: 0,
            _pad: [0, 0],
        }
    }
}

/// Volumetric nebula raymarching uniform buffer (Interface Contract 4).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct VolumetricNebulaUniform {
    pub box_min: [f32; 3],
    pub step_count: u32,
    pub box_max: [f32; 3],
    pub absorption_coefficient: f32,
    pub scattering_albedo: f32,
    pub phase_g: f32,
    pub num_stars: u32,
    pub _pad: u32,
}

impl Default for VolumetricNebulaUniform {
    fn default() -> Self {
        Self {
            box_min: [-550.0, -550.0, -550.0],
            step_count: 128,
            box_max: [550.0, 550.0, 550.0],
            absorption_coefficient: 0.015,
            scattering_albedo: 0.85,
            phase_g: 0.65,
            num_stars: 1,
            _pad: 0,
        }
    }
}

/// Converts 3D grid cell indices (x, y, z) into a 1D linear buffer index.
pub fn grid_index_1d(x: u32, y: u32, z: u32) -> usize {
    (z * GMC_GRID_DIM * GMC_GRID_DIM + y * GMC_GRID_DIM + x) as usize
}

/// Converts a 3D cell coordinate to world space position in AU.
pub fn grid_to_world_pos(x: u32, y: u32, z: u32) -> Vec3 {
    let half_domain = GMC_DOMAIN_SIZE_AU * 0.5;
    Vec3::new(
        (x as f32 + 0.5) * GMC_CELL_DX_AU - half_domain,
        (y as f32 + 0.5) * GMC_CELL_DX_AU - half_domain,
        (z as f32 + 0.5) * GMC_CELL_DX_AU - half_domain,
    )
}

/// Computes the Courant-Friedrichs-Lewy (CFL) maximum stable time step in years.
pub fn calculate_cfl_max_dt(dx_au: f32, max_velocity_au_yr: f32, sound_speed_au_yr: f32) -> f32 {
    let total_speed = (max_velocity_au_yr + sound_speed_au_yr).max(1e-6);
    // Typical CFL safety number C_cfl ~ 0.5
    0.5 * (dx_au / total_speed)
}

/// Evaluates Henyey-Greenstein anisotropic phase function for scattering angle cos_theta.
pub fn henyey_greenstein_phase(cos_theta: f32, g: f32) -> f32 {
    let denom = (1.0 + g * g - 2.0 * g * cos_theta).powf(1.5).max(1e-7);
    (1.0 - g * g) / (4.0 * std::f32::consts::PI * denom)
}

/// Ray-box slab intersection test against an axis-aligned bounding box [box_min, box_max].
/// Returns Some((t_near, t_far)) if the ray intersects the box ahead or through the origin.
pub fn intersect_ray_aabb(
    ray_origin: Vec3,
    ray_dir: Vec3,
    box_min: Vec3,
    box_max: Vec3,
) -> Option<(f32, f32)> {
    let inv_dir = Vec3::new(
        if ray_dir.x.abs() > 1e-8 {
            1.0 / ray_dir.x
        } else {
            f32::INFINITY
        },
        if ray_dir.y.abs() > 1e-8 {
            1.0 / ray_dir.y
        } else {
            f32::INFINITY
        },
        if ray_dir.z.abs() > 1e-8 {
            1.0 / ray_dir.z
        } else {
            f32::INFINITY
        },
    );

    let t1 = (box_min - ray_origin) * inv_dir;
    let t2 = (box_max - ray_origin) * inv_dir;

    let t_min = t1.min(t2);
    let t_max = t1.max(t2);

    let t_near = t_min.x.max(t_min.y).max(t_min.z);
    let t_far = t_max.x.min(t_max.y).min(t_max.z);

    if t_near <= t_far && t_far >= 0.0 {
        Some((t_near.max(0.0), t_far))
    } else {
        None
    }
}

/// Calculates Beer-Lambert optical transmittance for an optical depth tau: T = exp(-tau).
pub fn beer_lambert_transmittance(tau: f32) -> f32 {
    (-tau.max(0.0)).exp()
}

/// Re-export the production calculate_ionization_cavity_radius_au function.
pub use protostellar::rendering::volumetric_nebula::calculate_ionization_cavity_radius_au;
