//! Screen-space post-processing pipeline for ACES/Reinhard/AgX tonemapping, dynamic bloom, and hazard shockwaves.

use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::*;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};

/// Component attached to the post-processing full-screen quad camera.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType, Debug, Reflect)]
pub struct PostProcessingSettings {
    pub exposure: f32,
    pub bloom_intensity: f32,
    pub lensing_strength: f32,
    pub tonemapping_mode: u32,
    pub black_hole_screen_pos: Vec2,
    pub hazard_flash_intensity: f32,
    pub pad: f32,
}

impl Default for PostProcessingSettings {
    fn default() -> Self {
        Self {
            exposure: 1.15,
            bloom_intensity: 0.30,
            lensing_strength: 0.0,
            tonemapping_mode: 0,
            black_hole_screen_pos: Vec2::splat(0.5),
            hazard_flash_intensity: 0.0,
            pad: 0.0,
        }
    }
}

impl PostProcessingSettings {
    #[must_use]
    pub fn new(exposure: f32, bloom_intensity: f32) -> Self {
        Self {
            exposure,
            bloom_intensity,
            ..default()
        }
    }
}

/// Custom material for the screen-space post-processing fragment shader.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PostProcessingMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub source_texture: Handle<Image>,
    #[uniform(2)]
    pub settings: PostProcessingSettings,
}

impl Material2d for PostProcessingMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_processing.wgsl".into()
    }
}

pub struct PostProcessingPlugin;

impl Plugin for PostProcessingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<PostProcessingMaterial>::default())
            .register_type::<PostProcessingSettings>();
    }
}
