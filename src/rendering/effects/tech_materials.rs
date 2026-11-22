//! Custom procedural GPU materials for metallic megastructures, probes, and planetary shields.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy_shader::ShaderRef;

/// Style selectors matching `pattern_params.x` in `assets/shaders/megastructure.wesl`.
pub mod tech_style {
    pub const DYSON_COLLECTOR: f32 = 0.0;
    pub const SHKADOV_ENGINE: f32 = 1.0;
    pub const ORBITAL_HABITAT: f32 = 2.0;
    pub const ORBITAL_MIRROR: f32 = 3.0;
    pub const SPACE_PROBE: f32 = 4.0;
}

/// GPU uniform block driving `assets/shaders/megastructure.wesl`.
#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct MegastructureUniforms {
    /// `xyz`: host star world position, `w`: `visual_time_secs`.
    pub star_pos_and_time: Vec4,
    /// `xyz`: primary metallic/mirror linear RGB, `w`: metalness `0..1`.
    pub primary_albedo_and_metal: Vec4,
    /// `xyz`: dark titanium chassis/truss linear RGB, `w`: base roughness `0.06..0.85`.
    pub accent_albedo_and_rough: Vec4,
    /// `xyz`: accent beacon/window emissive linear RGB, `w`: HDR intensity scale.
    pub emissive_color_and_intensity: Vec4,
    /// `x`: tech style (`0..4`), `y`: `uv_u`, `z`: `uv_v`, `w`: pulse speed (rad/s).
    pub pattern_params: Vec4,
}

impl Default for MegastructureUniforms {
    fn default() -> Self {
        Self {
            star_pos_and_time: Vec4::ZERO,
            primary_albedo_and_metal: Vec4::new(0.88, 0.68, 0.24, 0.94),
            accent_albedo_and_rough: Vec4::new(0.14, 0.16, 0.20, 0.16),
            emissive_color_and_intensity: Vec4::new(0.35, 0.90, 1.35, 1.4),
            pattern_params: Vec4::new(tech_style::DYSON_COLLECTOR, 4.0, 2.0, 2.2),
        }
    }
}

/// Procedural Cook-Torrance GGX metallic material for 3D megastructures and aerospace hardware.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct MegastructureMaterial {
    #[uniform(0)]
    pub uniforms: MegastructureUniforms,
}

impl Material for MegastructureMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/megastructure.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Back);
        Ok(())
    }
}

/// GPU uniform block driving `assets/shaders/planetary_shield.wesl`.
#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct PlanetaryShieldUniforms {
    /// `xyz`: shield energy RGB, `w`: overdrive boost `0..1`.
    pub color_and_overdrive: Vec4,
    /// `x`: time (s), `y`: hex density, `z`: base rim alpha, `w`: shimmer strength.
    pub params: Vec4,
    /// `xyz`: unit direction from planet toward central star, `w`: harmonic speed.
    pub star_dir_and_pulse: Vec4,
}

impl Default for PlanetaryShieldUniforms {
    fn default() -> Self {
        Self {
            color_and_overdrive: Vec4::new(0.18, 0.72, 1.15, 0.0),
            params: Vec4::new(0.0, 18.0, 0.28, 0.85),
            star_dir_and_pulse: Vec4::new(-1.0, 0.0, 0.0, 2.2),
        }
    }
}

/// Geodesic hexagonal Fresnel energy shield material enclosing defended worlds.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct PlanetaryShieldMaterial {
    #[uniform(0)]
    pub uniforms: PlanetaryShieldUniforms,
}

impl Material for PlanetaryShieldMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/planetary_shield.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Back);
        if let Some(ref mut ds) = descriptor.depth_stencil {
            ds.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// Gold-silicon Dyson Swarm photovoltaic collector material.
#[must_use]
pub fn dyson_collector_material() -> MegastructureMaterial {
    MegastructureMaterial {
        uniforms: MegastructureUniforms {
            star_pos_and_time: Vec4::ZERO,
            primary_albedo_and_metal: Vec4::new(0.92, 0.68, 0.22, 0.95),
            accent_albedo_and_rough: Vec4::new(0.11, 0.13, 0.17, 0.16),
            emissive_color_and_intensity: Vec4::new(0.28, 0.88, 1.35, 1.5),
            pattern_params: Vec4::new(tech_style::DYSON_COLLECTOR, 3.0, 2.0, 2.4),
        },
    }
}

/// Shkadov Stellar Engine champagne-gold mirror & armored titanium hull material.
#[must_use]
pub fn shkadov_engine_material() -> MegastructureMaterial {
    MegastructureMaterial {
        uniforms: MegastructureUniforms {
            star_pos_and_time: Vec4::ZERO,
            primary_albedo_and_metal: Vec4::new(0.96, 0.82, 0.44, 0.98),
            accent_albedo_and_rough: Vec4::new(0.14, 0.16, 0.21, 0.10),
            emissive_color_and_intensity: Vec4::new(0.32, 0.86, 1.35, 1.35),
            pattern_params: Vec4::new(tech_style::SHKADOV_ENGINE, 8.0, 4.0, 2.0),
        },
    }
}

/// Orbital Habitat Ring brushed titanium & warm interior deck viewport material.
#[must_use]
pub fn orbital_habitat_material() -> MegastructureMaterial {
    MegastructureMaterial {
        uniforms: MegastructureUniforms {
            star_pos_and_time: Vec4::ZERO,
            primary_albedo_and_metal: Vec4::new(0.62, 0.68, 0.76, 0.90),
            accent_albedo_and_rough: Vec4::new(0.14, 0.17, 0.22, 0.24),
            emissive_color_and_intensity: Vec4::new(0.30, 0.92, 1.40, 1.45),
            pattern_params: Vec4::new(tech_style::ORBITAL_HABITAT, 4.0, 2.0, 3.0),
        },
    }
}

/// JWST-inspired gold-Kapton hexagonal heliostat mirror material.
#[must_use]
pub fn orbital_mirror_material() -> MegastructureMaterial {
    MegastructureMaterial {
        uniforms: MegastructureUniforms {
            star_pos_and_time: Vec4::ZERO,
            primary_albedo_and_metal: Vec4::new(1.02, 0.74, 0.20, 0.98),
            accent_albedo_and_rough: Vec4::new(0.11, 0.13, 0.17, 0.11),
            emissive_color_and_intensity: Vec4::new(0.35, 0.92, 1.30, 1.25),
            pattern_params: Vec4::new(tech_style::ORBITAL_MIRROR, 2.0, 2.0, 2.6),
        },
    }
}

/// Space probe gold Kapton MLI foil & titanium hardware material.
#[must_use]
pub fn probe_hull_material() -> MegastructureMaterial {
    MegastructureMaterial {
        uniforms: MegastructureUniforms {
            star_pos_and_time: Vec4::ZERO,
            primary_albedo_and_metal: Vec4::new(0.95, 0.72, 0.24, 0.93),
            accent_albedo_and_rough: Vec4::new(0.28, 0.32, 0.38, 0.26),
            emissive_color_and_intensity: Vec4::new(0.30, 0.88, 1.40, 1.5),
            pattern_params: Vec4::new(tech_style::SPACE_PROBE, 3.0, 3.0, 2.5),
        },
    }
}

/// Returns passive or overdrive hexagonal energy shield material.
#[must_use]
pub fn planetary_shield_material(overdrive: bool) -> PlanetaryShieldMaterial {
    PlanetaryShieldMaterial {
        uniforms: PlanetaryShieldUniforms {
            color_and_overdrive: if overdrive {
                Vec4::new(0.30, 0.88, 1.45, 1.0)
            } else {
                Vec4::new(0.18, 0.68, 1.15, 0.0)
            },
            params: if overdrive {
                Vec4::new(0.0, 20.0, 0.42, 1.2)
            } else {
                Vec4::new(0.0, 18.0, 0.24, 0.75)
            },
            star_dir_and_pulse: Vec4::new(-1.0, 0.0, 0.0, if overdrive { 4.2 } else { 2.0 }),
        },
    }
}
