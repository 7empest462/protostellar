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
    /// Condensation snow-line radius in AU (default 2.7 AU, 3.65 AU in Disk Genesis).
    pub snow_line_au: f32,
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
            snow_line_au: 2.7,
            tractor_pos_mass: [0.0, 0.0, 0.0, 0.0],
            massive_bodies: [MassiveBodyGpu::default(); 32],
        }
    }
}

/// Log-radial resolution of the GPU polar nucleation grid.
pub const NUCLEATION_GRID_R: u32 = 64;
/// Azimuthal resolution of the GPU polar nucleation grid.
pub const NUCLEATION_GRID_PHI: u32 = 64;
/// Total polar spatial cells in VRAM (`64 x 64 = 4096`).
pub const NUCLEATION_NUM_CELLS: usize = (NUCLEATION_GRID_R * NUCLEATION_GRID_PHI) as usize;
/// Number of 32-bit atomic slots per VRAM nucleation cell (`80 bytes` per cell).
pub const NUCLEATION_CELL_U32S: usize = 20;
/// Maximum simultaneous nucleation collapse events emitted per compact GPU readback packet.
pub const MAX_GPU_NUCLEATION_EVENTS: usize = 4;
/// Magic header tag identifying a compact GPU disk nucleation readback packet (`"NUCL"`).
pub const GPU_NUCLEATION_READBACK_MAGIC: u32 = 0x4E55_434C;

/// Uniform buffer driving the GPU radial/azimuthal overdensity nucleation passes (592 bytes, 16-byte aligned).
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuDiskNucleationUniforms {
    pub star_pos: [f32; 3],
    pub star_mass: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub snow_line_au: f32,
    pub base_mass: f32,
    pub gas_scale: f32,
    pub elapsed_years: f32,
    pub v_ref_scale: f32,
    pub num_particles: u32,
    pub enable_promotion: u32,
    pub max_promotions: u32,
    pub min_peak_particles: u32,
    pub frame_seed: u32,
    pub min_clump_mass_solar: f32,
    pub coagulate_enabled: u32,
    pub num_massive_bodies: u32,
    #[allow(clippy::pub_underscore_fields, reason = "WGSL 16-byte struct padding")]
    pub _pad: u32,
    pub massive_bodies: [MassiveBodyGpu; 32],
}

impl Default for GpuDiskNucleationUniforms {
    fn default() -> Self {
        Self {
            star_pos: [0.0, 0.0, 0.0],
            star_mass: 1.0,
            inner_radius: 0.12,
            outer_radius: 55.0,
            snow_line_au: 2.7,
            base_mass: 7.0e-10,
            gas_scale: 1.0,
            elapsed_years: 0.0,
            v_ref_scale: 30.0,
            num_particles: 50_000,
            enable_promotion: 0,
            max_promotions: 0,
            min_peak_particles: 6,
            frame_seed: 0,
            min_clump_mass_solar: 5.4e-9,
            coagulate_enabled: 0,
            num_massive_bodies: 0,
            _pad: 0,
            massive_bodies: [MassiveBodyGpu::default(); 32],
        }
    }
}

/// Single GPU-detected disk overdensity collapse event ready for Main-world ECS promotion (64 bytes).
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct GpuNucleationEvent {
    /// Mass-weighted position (`xyz` in AU) and collapsed mass (`w` in solar masses).
    pub pos_mass: [f32; 4],
    /// Mass-weighted orbital velocity (`xyz` in AU/yr) and temperature (`w` in K).
    pub vel_temp: [f32; 4],
    /// Average particle composition (`silicate`, `ice`, `metal`, `gas`).
    pub composition: [f32; 4],
    /// Metadata: `[0] cell_idx`, `[1] consumed_particles`, `[2] zone_kind`, `[3] reserved`.
    pub meta: [u32; 4],
}

impl Default for GpuNucleationEvent {
    fn default() -> Self {
        Self {
            pos_mass: [0.0; 4],
            vel_temp: [0.0; 4],
            composition: [0.0; 4],
            meta: [0; 4],
        }
    }
}

/// Compact 272-byte GPU -> CPU readback packet carrying promoted disk planetesimal/embryo events.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct GpuNucleationReadback {
    pub event_count: u32,
    pub magic: u32,
    pub generation_lo: u32,
    #[allow(clippy::pub_underscore_fields, reason = "WGSL 16-byte struct padding")]
    pub _pad: u32,
    pub events: [GpuNucleationEvent; MAX_GPU_NUCLEATION_EVENTS],
}

impl Default for GpuNucleationReadback {
    fn default() -> Self {
        Self {
            event_count: 0,
            magic: GPU_NUCLEATION_READBACK_MAGIC,
            generation_lo: 0,
            _pad: 0,
            events: [GpuNucleationEvent::default(); MAX_GPU_NUCLEATION_EVENTS],
        }
    }
}
