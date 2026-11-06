//! Screen-space post-processing pipeline for ACES/Reinhard/AgX tonemapping, dynamic bloom, and hazard shockwaves.

use bevy::camera::Exposure;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::*;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::CelestialBody;

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

/// Smoothly adapts camera `Exposure` (`ev100`) and HDR `Bloom` prefilter knees on the CPU
/// based on solar proximity and outer-system heliocentric distance.
///
/// This provides natural human-eye / photographic dynamic range adaptation (stopping down near
/// blinding stellar photospheres and opening up for dimly lit Kuiper Belt / Planet Nine worlds)
/// without invoking GPU 3D histogram LUT pipelines that conflict with SMAA on Metal.
pub fn update_adaptive_exposure_and_bloom(
    time: Res<Time>,
    mut cam_q: Query<(&PanOrbitCamera, &Transform, &mut Exposure, &mut Bloom)>,
    star_q: Query<(&Transform, &CelestialBody), Without<PanOrbitCamera>>,
) {
    let dt = time.delta_secs().clamp(0.001, 0.1);
    let lerp_t = (dt * 3.5).clamp(0.0, 1.0);

    let mut nearest_star_pos = Vec3::ZERO;
    let mut min_star_dist_sq = f32::MAX;

    for (pan_orbit, cam_tf, mut exposure, mut bloom) in &mut cam_q {
        for (s_tf, body) in &star_q {
            if body.body_type.is_star_or_remnant() {
                let d_sq = cam_tf.translation.distance_squared(s_tf.translation);
                if d_sq < min_star_dist_sq {
                    min_star_dist_sq = d_sq;
                    nearest_star_pos = s_tf.translation;
                }
            }
        }

        let cam_to_star = if min_star_dist_sq < f32::MAX {
            min_star_dist_sq.sqrt()
        } else {
            cam_tf.translation.length()
        };
        let focus_helio_dist = pan_orbit.focus.distance(nearest_star_pos);

        // 1. Proximity stop-down when inspecting a star up close (< 0.45 AU)
        let star_close_factor = (1.0 - (cam_to_star - 0.03) / 0.42).clamp(0.0, 1.0);

        // 2. Outer-system aperture boost when exploring beyond Jupiter/Saturn (> 6 AU out to 400+ AU)
        let outer_system_factor = ((focus_helio_dist - 6.0) / 34.0).clamp(0.0, 1.0);

        // 3. Planetary close-up factor when zoomed near a planet's atmosphere/surface
        let closeup_factor = (1.0 - (pan_orbit.radius - 0.005) / 0.15).clamp(0.0, 1.0);

        // Target EV100: 9.7 baseline; up to 10.15 near star; down to 9.18 in deep outer system
        let target_ev100 = 9.70 + star_close_factor * 0.45 - outer_system_factor * 0.52;

        // Target Bloom: softer threshold & gentle halation in outer system / planetary close-ups;
        // tighter threshold near bright stellar photospheres to preserve granulation detail.
        let target_threshold =
            1.65 + star_close_factor * 0.25 - outer_system_factor * 0.20 - closeup_factor * 0.08;
        let target_softness =
            0.46 - star_close_factor * 0.06 + outer_system_factor * 0.08 + closeup_factor * 0.04;
        let target_intensity =
            0.155 - star_close_factor * 0.02 + outer_system_factor * 0.025 + closeup_factor * 0.015;

        exposure.ev100 += (target_ev100 - exposure.ev100) * lerp_t;
        bloom.prefilter.threshold += (target_threshold - bloom.prefilter.threshold) * lerp_t;
        bloom.prefilter.threshold_softness +=
            (target_softness - bloom.prefilter.threshold_softness) * lerp_t;
        bloom.intensity += (target_intensity - bloom.intensity) * lerp_t;
    }
}

pub struct PostProcessingPlugin;

impl Plugin for PostProcessingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<PostProcessingMaterial>::default())
            .register_type::<PostProcessingSettings>()
            .add_systems(Update, update_adaptive_exposure_and_bloom);
    }
}
