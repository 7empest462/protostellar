//! Screen-space post-processing pipeline for Bevy 0.20 `FullscreenMaterial`:
//! relativistic gravitational-wave ripples, spacetime black hole curvature lensing,
//! dynamic bloom, and hazard shockwaves.

use bevy::camera::Exposure;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::*;
use bevy::shader::ShaderRef;

use crate::rendering::camera::PanOrbitCamera;
use crate::rendering::effects::supernova::{SupernovaDebrisPool, SupernovaType};
use crate::simulation::components::CelestialBody;

/// Uniform parameters driving the fullscreen post-processing shader.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType, Debug, Reflect)]
#[extract_app(RenderApp)]
pub struct PostProcessingSettings {
    pub exposure: f32,
    pub bloom_intensity: f32,
    pub lensing_strength: f32,
    pub tonemapping_mode: u32,
    pub black_hole_screen_pos: Vec2,
    pub hazard_flash_intensity: f32,
    pub time: f32,
    pub shockwave_phase: f32,
    pub shockwave_amplitude: f32,
    pub shockwave_center: Vec2,
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
            time: 0.0,
            shockwave_phase: 0.0,
            shockwave_amplitude: 0.0,
            shockwave_center: Vec2::splat(0.5),
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

impl FullscreenMaterial for PostProcessingSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_processing.wesl".into()
    }
}

/// Smoothly adapts camera `Exposure` (`ev100`) and HDR `Bloom` prefilter knees on the CPU
/// based on solar proximity and outer-system heliocentric distance.
pub fn update_adaptive_exposure_and_bloom(
    time: Res<Time>,
    mut cam_q: Query<(&PanOrbitCamera, &Transform, &mut Exposure, &mut Bloom)>,
    star_q: Query<(&Transform, &CelestialBody), Without<PanOrbitCamera>>,
    supernova_pool: Option<Res<SupernovaDebrisPool>>,
) {
    let dt = time.delta_secs().clamp(0.001, 0.1);
    let lerp_t = (dt * 3.5).clamp(0.0, 1.0);

    let blowout_bloom_boost = supernova_pool.as_deref().map_or(0.0_f32, |pool| {
        pool.explosions
            .iter()
            .filter(|e| e.explosion_type == SupernovaType::QuasarBlowout)
            .map(|e| {
                let env = (1.0 - e.timer / 6.0).clamp(0.0, 1.0);
                (0.35 + 0.65 * e.flash_intensity) * env
            })
            .fold(0.0_f32, f32::max)
    });

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

        let target_threshold = (1.65 + star_close_factor * 0.25
            - outer_system_factor * 0.20
            - closeup_factor * 0.08
            - blowout_bloom_boost * 0.55)
            .max(0.85);
        let target_softness = (0.46 - star_close_factor * 0.06
            + outer_system_factor * 0.08
            + closeup_factor * 0.04
            + blowout_bloom_boost * 0.16)
            .clamp(0.1, 0.9);
        let target_intensity = 0.155 - star_close_factor * 0.02
            + outer_system_factor * 0.025
            + closeup_factor * 0.015
            + blowout_bloom_boost * 0.22;

        exposure.ev100 += (target_ev100 - exposure.ev100) * lerp_t;
        bloom.prefilter.threshold += (target_threshold - bloom.prefilter.threshold) * lerp_t;
        bloom.prefilter.threshold_softness +=
            (target_softness - bloom.prefilter.threshold_softness) * lerp_t;
        bloom.intensity += (target_intensity - bloom.intensity) * lerp_t;
    }
}

/// Updates real-time screen-space coordinates, tonemapping,
/// and gravitational-wave shockwave ripples for the fullscreen post-processing pass.
pub fn update_post_processing_effects(
    time: Res<Time>,
    mut cam_q: Query<(
        &Camera,
        &GlobalTransform,
        &mut PostProcessingSettings,
        &mut Tonemapping,
    )>,
    hazard_mgr: Option<Res<crate::simulation::events::CosmicEventManager>>,
    supernova_pool: Option<Res<SupernovaDebrisPool>>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    let active_blowout = supernova_pool.as_deref().and_then(|pool| {
        pool.explosions
            .iter()
            .find(|e| e.explosion_type == SupernovaType::QuasarBlowout && e.timer < 3.2)
    });

    for (camera, cam_gtf, mut settings, mut tonemapping) in &mut cam_q {
        settings.time = elapsed;

        // 1. Synchronize tonemapping mode with hazard manager & camera display transform
        let mut target_flash = 0.0_f32;
        if let Some(ref mgr) = hazard_mgr {
            settings.tonemapping_mode = mgr.tonemapping_mode.shader_mode_id();
            let target_tm = match mgr.tonemapping_mode {
                crate::simulation::events::TonemappingDisplayMode::AgX => Tonemapping::AgX,
                crate::simulation::events::TonemappingDisplayMode::ACES => Tonemapping::AcesFitted,
                crate::simulation::events::TonemappingDisplayMode::Reinhard => {
                    Tonemapping::Reinhard
                }
            };
            if *tonemapping != target_tm {
                *tonemapping = target_tm;
            }
            target_flash = mgr.hazard_flash_intensity;
            if mgr.hazard_flash_intensity > 0.1 && settings.shockwave_amplitude < 0.05 {
                settings.shockwave_amplitude = mgr.hazard_flash_intensity * 0.8;
                settings.shockwave_phase = 0.0;
                settings.shockwave_center = Vec2::splat(0.5);
            }
        }

        // 2. Relativistic Quasar Blowout fullscreen distortion shockwave & prompt flash
        if let Some(exp) = active_blowout {
            let screen_uv = camera
                .world_to_viewport(cam_gtf, exp.center)
                .ok()
                .zip(camera.logical_viewport_size())
                .map_or(Vec2::splat(0.5), |(px, size)| {
                    Vec2::new(
                        (px.x / size.x.max(1.0)).clamp(0.05, 0.95),
                        (px.y / size.y.max(1.0)).clamp(0.05, 0.95),
                    )
                });
            settings.shockwave_center = screen_uv;

            // Primary shockwave crest at breakout + secondary chromatic ripple echo
            let primary_wave = if exp.timer < 2.4 {
                let rise = (exp.timer / 0.14).clamp(0.0, 1.0);
                rise * (-1.15 * exp.timer).exp() * 0.95
            } else {
                0.0
            };
            let secondary_wave = if exp.timer > 0.45 && exp.timer < 2.8 {
                let t2 = exp.timer - 0.45;
                let rise2 = (t2 / 0.12).clamp(0.0, 1.0);
                rise2 * (-1.35 * t2).exp() * 0.65
            } else {
                0.0
            };
            settings.shockwave_amplitude = settings
                .shockwave_amplitude
                .max(primary_wave.max(secondary_wave));
            settings.shockwave_phase = exp.timer * 1.45;

            let blowout_flash = if exp.timer < 1.35 {
                let ramp = (exp.timer / 0.10).clamp(0.0, 1.0);
                ramp * (-2.8 * (exp.timer - 0.08).max(0.0)).exp() * 0.68
            } else {
                0.0
            };
            target_flash = target_flash.max(blowout_flash);
        } else if settings.shockwave_amplitude > 0.0001 {
            // Standard gravitational-wave shockwave progression & decay
            settings.shockwave_phase += dt;
            settings.shockwave_amplitude = (settings.shockwave_amplitude - dt * 0.45).max(0.0);
        }
        settings.hazard_flash_intensity = target_flash;

        // 3. Reset screen-space black hole lensing: black holes compute physical
        // General Relativistic lensing in 3D geometry via black_hole_disk.wesl.
        settings.lensing_strength = 0.0;
        settings.black_hole_screen_pos = Vec2::splat(0.5);
    }
}

pub struct PostProcessingPlugin;

impl Plugin for PostProcessingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<PostProcessingSettings>::default())
            .register_type::<PostProcessingSettings>()
            .add_systems(
                Update,
                (
                    update_adaptive_exposure_and_bloom
                        .after(crate::rendering::camera::update_pan_orbit_camera),
                    update_post_processing_effects,
                ),
            );
    }
}
