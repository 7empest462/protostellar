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
        -1.60f32, -1.20, -0.85, -0.55, -0.30, -0.12, -0.04, 0.0, 0.04, 0.12, 0.30, 0.55, 0.85,
        1.20, 1.60,
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
    for handle in gas_query.iter() {
        if let Some(mut mat) = materials.get_mut(handle) {
            mat.extension.uniforms.time_data.x = (sim_time.elapsed_years * 12.0) as f32;
            mat.extension.uniforms.time_data.w = config.gas_density_scale;
            mat.extension.uniforms.time_data.z = (disk_params.outer_radius_au as f32).max(45.0);

            if let Ok((rad, temp, lum, ignition, opt_bhs, star_body)) = star_query.single() {
                let is_blown_out = opt_bhs.map(|s| s.is_blown_out).unwrap_or(false);
                let is_black_hole = star_body.body_type == BodyType::BlackHole;

                if is_blown_out || is_black_hole {
                    // Blow out ONLY the center yellow part (inner cavity expanding out to 25 AU).
                    // The rest of the outer disk (cyan and lavender nebula) stays fully visible!
                    let blowout_p = opt_bhs.map(|s| s.blowout_progress).unwrap_or(1.0);
                    let target_inner = 0.5 + blowout_p * 24.5;
                    mat.extension.uniforms.time_data.y = target_inner;
                } else {
                    // Before blowout: Cloud spans with the warm golden yellow on the inside!
                    // This lets the 2.5 MegaGauss electromagnetic waves propagate through the glowing cloud.
                    mat.extension.uniforms.time_data.y = 0.5;
                }

                mat.extension.uniforms.star_params = Vec4::new(
                    rad.0 as f32,
                    temp.0 as f32,
                    lum.0 as f32,
                    ignition.shockwave_radius as f32,
                );
            } else {
                mat.extension.uniforms.time_data.y = 0.5;
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
