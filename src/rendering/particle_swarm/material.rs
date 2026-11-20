//! Custom unlit GPU-billboarded material for the 500k particle swarm.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy_shader::ShaderRef;

/// GPU uniform parameters for [`ParticleSwarmMaterial`].
#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct ParticleSwarmUniforms {
    /// `x`: particle render scale multiplier, `y..w`: reserved.
    pub params: Vec4,
}

/// Custom unlit alpha-blended material that expands camera-facing particle quads on the GPU.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ParticleSwarmMaterial {
    #[uniform(0)]
    pub uniforms: ParticleSwarmUniforms,
}

impl ParticleSwarmMaterial {
    /// Creates a particle swarm material initialized with the given render scale multiplier.
    pub fn new(render_scale: f32) -> Self {
        Self {
            uniforms: ParticleSwarmUniforms {
                params: Vec4::new(render_scale, 0.0, 0.0, 0.0),
            },
        }
    }
}

impl Material for ParticleSwarmMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/particle_swarm.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/particle_swarm.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}
