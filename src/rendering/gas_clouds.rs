//! Volumetric Protoplanetary Gas Cloud Material and Mesh Spawner.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy_shader::ShaderRef;

use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Uniform parameters passed to the nebular gas cloud shader.
#[derive(Clone, Default, ShaderType, Debug)]
pub struct GasUniforms {
    /// x: time, y: inner_radius, z: outer_radius, w: gas_density_scale
    pub time_data: Vec4,
    /// x: star_radius, y: star_temp, z: star_lum, w: shockwave_radius
    pub star_params: Vec4,
}

/// Custom Material Extension for the gaseous component of the protoplanetary nebula.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct GasCloudExtension {
    #[uniform(101)]
    pub uniforms: GasUniforms,
}

impl MaterialExtension for GasCloudExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/gas_cloud.wgsl".into()
    }
}

pub type GasCloudMaterial = ExtendedMaterial<StandardMaterial, GasCloudExtension>;

/// Marker component for the volumetric gas cloud entity.
#[derive(Component)]
pub struct GasCloudDisk;

/// Spawns the volumetric gas disk geometry with multi-layered ethereal depth and double-sided visibility.
pub fn setup_gas_cloud_disk(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GasCloudMaterial>>,
    disk_params: Res<DiskParameters>,
) {
    let plane_size = 1200.0f32;
    let plane_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(plane_size)));

    let gas_material = materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None, // Double-sided rendering
            ..default()
        },
        extension: GasCloudExtension {
            uniforms: GasUniforms {
                time_data: Vec4::new(
                    0.0,
                    0.5,
                    (disk_params.outer_radius_au as f32).max(45.0),
                    1.0,
                ),
                star_params: Vec4::new(0.013, 3600.0, 1.8, 0.0),
            },
        },
    });

    // Multi-layered stratified 3D geometry spanning vertical flared scale-height
    // This gives a thick, volumetric, glowing cloud with smooth Gaussian decay at every height.
    let layer_offsets = [
        -3.50f32, -2.40, -1.60, -1.00, -0.55, -0.22, -0.06, 0.0, 0.06, 0.22, 0.55, 1.00, 1.60,
        2.40, 3.50,
    ];

    for y_offset in layer_offsets {
        commands.spawn((
            Mesh3d(plane_mesh.clone()),
            MeshMaterial3d(gas_material.clone()),
            Transform::from_translation(Vec3::new(0.0, y_offset, 0.0)),
            NotShadowCaster,
            NotShadowReceiver,
            GasCloudDisk,
        ));
    }
}

/// Updates gas cloud animation time, density clearance, and star ignition shockwave.
pub fn update_gas_cloud_material(
    time: Res<Time>,
    sim_time: Res<SimTime>,
    config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    mut materials: ResMut<Assets<GasCloudMaterial>>,
    gas_query: Query<&MeshMaterial3d<GasCloudMaterial>, With<GasCloudDisk>>,
    star_query: Query<
        (
            &Radius,
            &Temperature,
            &Luminosity,
            &IgnitionState,
            Option<&BlackHoleStarState>,
            &CelestialBody,
        ),
        With<CentralStar>,
    >,
) {
    let is_massive = disk_params.central_star_mass > 10.0 || disk_params.outer_radius_au > 100.0;
    let is_compact = disk_params.outer_radius_au < 2.0;
    let is_remnant_or_pulsar = if let Ok((_, _, _, _, _, star_body)) = star_query.single() {
        star_body.body_type == BodyType::Pulsar || star_body.body_type == BodyType::Magnetar
    } else {
        false
    };

    let disk_outer = if is_massive {
        (disk_params.outer_radius_au as f32).max(280.0)
    } else if is_remnant_or_pulsar {
        // Faint local fallback envelope matching the scenario scale (e.g. 2.5 - 7.5 AU)
        (disk_params.outer_radius_au as f32 * 1.25).clamp(2.5, 8.0)
    } else if is_compact {
        // Compact system (e.g. TRAPPIST-1 with 0.15 AU disk)
        (disk_params.outer_radius_au as f32 * 1.2).max(0.18)
    } else {
        (disk_params.outer_radius_au as f32).max(45.0)
    };

    let density_scale = if is_remnant_or_pulsar {
        config.gas_density_scale * 0.15 // Faint, subtle, translucent wisps
    } else if is_compact && disk_params.disk_mass <= 0.0005 {
        config.gas_density_scale * 0.05 // Very subtle, faint primordial veil
    } else {
        config.gas_density_scale
    };

    let anim_time = (sim_time.elapsed_years * 60.0) as f32 + time.elapsed_secs() * 0.40;

    for handle in gas_query.iter() {
        if let Some(mut mat) = materials.get_mut(handle) {
            mat.extension.uniforms.time_data.x = anim_time;
            mat.extension.uniforms.time_data.w = density_scale;
            mat.extension.uniforms.time_data.z = disk_outer;

            if let Ok((rad, temp, lum, ignition, opt_bhs, star_body)) = star_query.single() {
                let is_blown_out = opt_bhs.is_some_and(|s| s.is_blown_out);
                let is_black_hole = star_body.body_type == BodyType::BlackHole;

                let target_inner = if is_massive {
                    if is_blown_out || is_black_hole {
                        1.2f32
                    } else {
                        // Wraps right outside the central quasi-star (~2.0 AU)
                        (disk_params.inner_radius_au as f32).clamp(1.2, 3.5)
                    }
                } else if is_blown_out || is_black_hole {
                    let blowout_p = opt_bhs.map_or(1.0, |s| s.blowout_progress);
                    0.5 + blowout_p * 24.5
                } else if is_compact {
                    (disk_params.inner_radius_au as f32).clamp(0.003, 0.015)
                } else {
                    (disk_params.inner_radius_au as f32).clamp(0.05, 5.0)
                };
                mat.extension.uniforms.time_data.y = target_inner;

                mat.extension.uniforms.star_params = Vec4::new(
                    rad.0 as f32,
                    temp.0 as f32,
                    lum.0 as f32,
                    ignition.shockwave_radius as f32,
                );
            } else {
                mat.extension.uniforms.time_data.y = if is_massive { 2.0 } else { 0.5 };
            }
        }
    }
}

/// Plugin registering the gas cloud rendering pass.
pub struct GasCloudPlugin;

impl Plugin for GasCloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<GasCloudMaterial>::default())
            .add_systems(Startup, setup_gas_cloud_disk)
            .add_systems(Update, update_gas_cloud_material);
    }
}
