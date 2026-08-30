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
pub struct RingMaterialExtension {
    #[uniform(101)]
    pub uniforms: RingUniforms,
}

impl MaterialExtension for RingMaterialExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/planetary_rings.wgsl".into()
    }
}

pub type RingMaterial = ExtendedMaterial<StandardMaterial, RingMaterialExtension>;
