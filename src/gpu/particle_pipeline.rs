//! Custom Material and Render Pipeline for instanced protoplanetary disk particle rendering.

use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::shader::ShaderRef;

/// Custom PBR material for protoplanetary dust and planetesimal particles.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DiskParticleMaterial {
    #[uniform(0)]
    pub tint: LinearRgba,
    #[uniform(1)]
    pub intensity: f32,
}

impl Default for DiskParticleMaterial {
    fn default() -> Self {
        Self {
            tint: LinearRgba::WHITE,
            intensity: 1.0,
        }
    }
}

impl Material for DiskParticleMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/particle_render.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/particle_render.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Plugin registering the particle material in Bevy's asset and render systems.
pub struct ParticleRenderPlugin;

impl Plugin for ParticleRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<DiskParticleMaterial>::default());
    }
}
