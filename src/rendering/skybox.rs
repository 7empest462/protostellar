//! Deep space environment, scenario-specific procedural celestial skybox, and cosmic lighting.
//!
//! Features:
//! - Modern Milky Way & Star Clusters (Normal scenarios: Solar Nebula, TRAPPIST-1, Kepler-16, etc.)
//! - Early Universe High-Redshift Cosmic Web (JWST Little Red Dot Quasi-Star scenario, z ~ 8.5)
//! - Camera-locked celestial sphere tracking (infinite depth, zero parallax, no clipping)
//! - Smooth scenario morphing and adaptive ambient cosmic dawn lighting

use bevy::prelude::*;

use crate::rendering::materials::SkyboxMaterial;
use crate::simulation::components::{
    BlackHoleStarState, BodyType, CelestialBody, CentralStar, Mass, Radius, SimPosition,
};
use crate::simulation::resources::SimulationConfig;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

/// Marker component for the procedural celestial skybox sphere.
#[derive(Component, Debug)]
pub struct CelestialSkybox;

/// Spawns the procedural celestial sphere and ambient cosmic lighting.
pub fn setup_space_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkyboxMaterial>>,
) {
    // 1. Ambient cosmic illumination (dim background starlight)
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.08, 0.09, 0.14),
        brightness: 120.0,
        ..default()
    });

    // 2. Distant galactic directional light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.92, 0.95, 1.0),
            illuminance: 350.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.8, 0.0)),
    ));

    // 3. Procedural Celestial Sphere (1,000,000 AU radius, camera-centered)
    let skybox_mesh = meshes.add(Sphere::new(1_000_000.0).mesh().ico(5).unwrap());
    let skybox_material = materials.add(SkyboxMaterial::default());

    commands.spawn((
        Mesh3d(skybox_mesh),
        MeshMaterial3d(skybox_material),
        Transform::from_translation(Vec3::ZERO),
        CelestialSkybox,
    ));

    info!("🌌 setup_space_environment: Procedural Celestial Skybox spawned (1,000,000 AU)");
}

/// Keeps the celestial sphere locked to the camera position so the viewer is always at the center,
/// completely eliminating depth clipping and artificial parallax.
pub fn sync_skybox_to_camera(
    camera_query: Query<&Transform, (With<Camera3d>, Without<CelestialSkybox>)>,
    mut skybox_query: Query<&mut Transform, With<CelestialSkybox>>,
) {
    if let Ok(camera_tf) = camera_query.single() {
        for mut skybox_tf in skybox_query.iter_mut() {
            skybox_tf.translation = camera_tf.translation;
        }
    }
}

/// Updates procedural skybox uniforms, advancing time, smoothly morphing between
/// the Modern Milky Way (0.0) and the Early Universe Cosmic Web (1.0), and computing
/// General Relativistic gravitational lensing parameters for massive black holes.
pub fn update_skybox_uniforms(
    time: Res<Time>,
    config: Res<SimulationConfig>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    mut materials: ResMut<Assets<SkyboxMaterial>>,
    skybox_query: Query<&MeshMaterial3d<SkyboxMaterial>, With<CelestialSkybox>>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<CelestialSkybox>)>,
    star_query: Query<
        (
            &SimPosition,
            &Mass,
            &Radius,
            &CelestialBody,
            Option<&BlackHoleStarState>,
        ),
        With<CentralStar>,
    >,
    mut ambient_light: Option<ResMut<GlobalAmbientLight>>,
) {
    let dt = time.delta_secs();

    // 1. Determine target scenario blend: 1.0 for Little Red Dot (Early Universe), 0.0 for Milky Way
    let target_blend = if let Some(ref state) = scenario_state {
        if state.current_preset == ScenarioPreset::LittleRedDot {
            1.0_f32
        } else {
            0.0_f32
        }
    } else {
        0.0_f32
    };

    // 2. Gravitational Lensing Parameters Calculation
    let camera_pos = camera_query
        .single()
        .map(|tf| tf.translation)
        .unwrap_or(Vec3::ZERO);
    let mut lens_pos_and_mass = Vec4::ZERO;
    let mut lens_params = Vec4::ZERO;

    if let Ok((star_pos, star_mass, star_radius, star_body, opt_bhs)) = star_query.single() {
        let is_quasi = star_body.body_type == BodyType::QuasiStar;
        let is_massive_bh = star_body.body_type == BodyType::BlackHole && star_mass.0 > 100.0;

        if is_quasi || is_massive_bh {
            let bh_pos = Vec3::new(star_pos.x as f32, star_pos.y as f32, star_pos.z as f32);
            let bh_rel = bh_pos - camera_pos;
            let dist_to_bh = bh_rel.length().max(0.01);

            let visual_r = config.calc_visual_radius_for_type(star_radius.0, star_body.body_type);
            let is_blown_out = opt_bhs.map(|s| s.is_blown_out).unwrap_or(is_massive_bh);
            let blowout_p = opt_bhs
                .map(|s| s.blowout_progress)
                .unwrap_or(if is_blown_out { 1.0 } else { 0.0 });

            // Effective gravitational Einstein radius (physical + visual aesthetic scaling)
            // Pre-blowout: spans outside the 60 AU envelope (R ~ 72 AU).
            // Post-blowout: focuses onto the 2.5 AU event horizon with a 1.5x photon sphere (R ~ 4.2 AU).
            let effective_lens_r = if is_blown_out {
                (visual_r * 1.85).max(3.8)
            } else {
                let r_cocoon_lens = 72.0f32;
                let r_bh_lens = (visual_r * 1.85).max(3.8);
                r_cocoon_lens + (r_bh_lens - r_cocoon_lens) * blowout_p
            };

            let theta_e = (effective_lens_r / dist_to_bh).atan();
            let theta_shadow = ((visual_r * 0.98).max(0.01) / dist_to_bh).atan();
            let photon_ring_width = (theta_shadow * 0.045).clamp(0.002, 0.06);

            lens_pos_and_mass = Vec4::new(bh_rel.x, bh_rel.y, bh_rel.z, theta_e);
            lens_params = Vec4::new(theta_shadow, photon_ring_width, 1.0, 1.0);
        }
    }

    for handle in skybox_query.iter() {
        if let Some(mut mat) = materials.get_mut(handle) {
            // Smooth exponential lerp toward target mode (transition speed ~ 2.2 / sec)
            let current_blend = mat.uniforms.params.y;
            let blend_speed = 2.2;
            let new_blend =
                current_blend + (target_blend - current_blend) * (dt * blend_speed).min(1.0);

            // Update parameters
            mat.uniforms.params.x += dt; // Animation time
            mat.uniforms.params.y = new_blend; // Mode blend: 0.0 = Milky Way, 1.0 = Early Universe
            mat.uniforms.params.z = 1.30; // Exposure
            mat.uniforms.params.w = 1.0; // Star twinkle intensity

            // Fine-tune web and filament parameters
            mat.uniforms.tuning.x = 1.0; // Star density
            mat.uniforms.tuning.y = 1.0; // Nebula intensity
            mat.uniforms.tuning.z = 1.0; // Cosmic web scale
            mat.uniforms.tuning.w = 1.0; // Filament emission brightness

            // Update gravitational lensing parameters
            mat.uniforms.lens_pos_and_mass = lens_pos_and_mass;
            mat.uniforms.lens_params = lens_params;

            // Dynamically adapt ambient cosmic lighting to match the era
            if let Some(ref mut amb) = ambient_light {
                // Milky Way: cool starlight silver-blue (0.08, 0.09, 0.14)
                // Early Universe: warm redshifted cosmic dawn amber-crimson (0.15, 0.05, 0.07)
                let r = 0.08 + new_blend * 0.07;
                let g = 0.09 - new_blend * 0.04;
                let b = 0.14 - new_blend * 0.07;
                amb.color = Color::srgb(r, g, b);
                amb.brightness = 120.0 + new_blend * 30.0;
            }
        }
    }
}
