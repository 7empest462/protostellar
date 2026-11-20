//! WGPU GPU buffer definitions and uniform layouts for 50k+ particle simulation.

use bytemuck::{Pod, Zeroable};

/// GPU representation of an individual disk simulation particle (48 bytes, 16-byte aligned).
#[repr(C, align(16))]
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
#[repr(C, align(16))]
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

/// Uniforms passed to the particle orbit WGSL compute shader (592 bytes, 16-byte aligned).
#[repr(C, align(16))]
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
    #[allow(clippy::pub_underscore_fields, reason = "padding for alignment")]
    pub _pad: f32,
    pub tractor_pos_mass: [f32; 4],
    pub massive_bodies: [MassiveBodyGpu; 32],
}

/// Magic tag identifying a compact 144-byte GPU accretion/heartbeat readback packet (`"PULL"`).
pub const GPU_COMPACT_READBACK_MAGIC: u32 = 0x5055_4C4C;

/// Number of `u32` atomic slots in the GPU accretion summary buffer:
/// `0..32` = per-massive-body accreted particle counts, `32` = surviving active particles.
pub const GPU_ACCRETION_SUMMARY_SLOTS: usize = 33;

/// Compact 144-byte GPU -> CPU readback packet replacing the 24 MB/frame full-particle copy.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq, Eq)]
pub struct GpuCompactReadback {
    pub magic: u32,
    pub active_particles: u32,
    pub is_galaxy: u32,
    pub generation_lo: u32,
    pub accreted_per_body: [u32; 32],
}

impl Default for GpuCompactReadback {
    fn default() -> Self {
        Self {
            magic: GPU_COMPACT_READBACK_MAGIC,
            active_particles: 0,
            is_galaxy: 0,
            generation_lo: 0,
            accreted_per_body: [0; 32],
        }
    }
}

/// Uniform buffer passed to `particle_render.wgsl` for instanced GPU vertex pulling (128 bytes).
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct GpuParticlePullUniforms {
    /// View-projection matrix (`clip_from_world`).
    pub clip_from_world: [[f32; 4]; 4],
    /// `xyz`: camera right basis vector, `w`: base particle half-radius (AU).
    pub camera_right_and_base_r: [f32; 4],
    /// `xyz`: camera up basis vector, `w`: global particle render scale multiplier.
    pub camera_up_and_scale: [f32; 4],
    /// `xyz`: camera world position (AU), `w`: baseline single-particle mass (`M_sun`).
    pub camera_pos_and_base_mass: [f32; 4],
    /// `x`: `1.0` for `GalacticCollision` star mode else `0.0`, `y`: active instance count,
    /// `z`: global alpha scale, `w`: reserved.
    pub mode_params: [f32; 4],
}

impl Default for GpuParticlePullUniforms {
    fn default() -> Self {
        Self {
            clip_from_world: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            camera_right_and_base_r: [1.0, 0.0, 0.0, 0.08],
            camera_up_and_scale: [0.0, 1.0, 0.0, 0.12],
            camera_pos_and_base_mass: [0.0, 0.0, 10.0, 1e-7],
            mode_params: [0.0, 0.0, 1.0, 0.0],
        }
    }
}

impl Default for GpuOrbitUniforms {
    fn default() -> Self {
        Self {
            star_pos: [0.0, 0.0, 0.0],
            star_mass: 1.0,
            dt: 0.0005,
            gas_scale: 1.0,
            inner_radius: 0.06,
            outer_radius: 35.0,
            g_const: 39.478_418, // 4 * PI^2
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
