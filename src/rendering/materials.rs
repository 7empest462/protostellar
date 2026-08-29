use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy_shader::ShaderRef;

use bevy::render::render_resource::ShaderType;

#[derive(Clone, Default, ShaderType, Debug)]
pub struct PlanetUniforms {
    pub planet_type: u32,
    pub temperature: f32,
    pub time: f32,
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
