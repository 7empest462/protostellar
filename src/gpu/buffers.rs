//! WGPU GPU buffer definitions and uniform layouts for 50k+ particle simulation.

use bytemuck::{Pod, Zeroable};

/// GPU representation of an individual disk simulation particle (48 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuParticle {
    /// Position (x, y, z in AU) and Mass (w in Solar Masses)
    pub pos_mass: [f32; 4],
    /// Velocity (x, y, z in AU/yr) and Temperature (w in K)
    pub vel_temp: [f32; 4],
    /// Composition: Silicate (x), Ice (y), Metal (z), Gas (w)
    pub composition: [f32; 4],
}

/// GPU representation of a massive body (planet, embryo, or star).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MassiveBodyGpu {
    /// Position (x, y, z in AU) and Mass (w in Solar Masses)
    pub pos_mass: [f32; 4],
}

impl Default for MassiveBodyGpu {
    fn default() -> Self {
        Self {
            pos_mass: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Uniforms passed to the particle orbit WGSL compute shader (592 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuOrbitUniforms {
    pub star_pos: [f32; 3],
    pub star_mass: f32,
    pub dt: f32,
    pub gas_scale: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub g_const: f32,
    pub enable_gas_drag: u32,
    pub num_particles: u32,
    pub ref_temp_1au: f32,
    pub shockwave_radius: f32,
    pub softening_sq: f32,
    pub num_massive_bodies: u32,
    pub _pad: f32,
    pub tractor_pos_mass: [f32; 4],
    pub massive_bodies: [MassiveBodyGpu; 32],
}

impl Default for GpuOrbitUniforms {
    fn default() -> Self {
        Self {
            star_pos: [0.0, 0.0, 0.0],
            star_mass: 1.0,
            dt: 0.0005,
            gas_scale: 1.0,
            inner_radius: 0.65,
            outer_radius: 35.0,
            g_const: 39.478418, // 4 * PI^2
            enable_gas_drag: 1,
            num_particles: 50000,
            ref_temp_1au: 280.0,
            shockwave_radius: 0.0,
            softening_sq: 0.008 * 0.008,
            num_massive_bodies: 1,
            _pad: 0.0,
            tractor_pos_mass: [0.0, 0.0, 0.0, 0.0],
            massive_bodies: [MassiveBodyGpu::default(); 32],
        }
    }
}
