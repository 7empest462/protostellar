use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy_shader::ShaderRef;

#[derive(Clone, Default, ShaderType, Debug)]
pub struct PlanetUniforms {
    pub planet_type: u32,
    pub temperature: f32,
    pub time: f32,
    /// x: rock (silicate + organics), y: ice (volatiles/water), z: metal, w: gas (atmosphere)
    pub composition: Vec4,
    pub color_seed: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct PlanetMaterialExtension {
    #[uniform(101)]
    pub uniforms: PlanetUniforms,
}

impl MaterialExtension for PlanetMaterialExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/planet.wgsl".into()
    }
}

pub type PlanetMaterial = ExtendedMaterial<StandardMaterial, PlanetMaterialExtension>;

#[derive(Clone, Default, ShaderType, Debug)]
pub struct RingUniforms {
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub optical_depth: f32,
    pub ice_fraction: f32,
    pub ring_color: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct RingMaterial {
    #[uniform(0)]
    pub uniforms: RingUniforms,
}

impl Default for RingMaterial {
    fn default() -> Self {
        Self {
            uniforms: RingUniforms {
                inner_radius: 0.0008,
                outer_radius: 0.0028,
                optical_depth: 0.85,
                ice_fraction: 0.95,
                ring_color: Vec4::ONE,
            },
        }
    }
}

impl Material for RingMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/planetary_rings.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None; // Double-sided rendering
        Ok(())
    }
}
