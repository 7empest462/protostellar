//! Volumetric Protoplanetary Gas Cloud Material and Mesh Spawner.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy_shader::ShaderRef;

use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Uniform parameters passed to the nebular gas cloud shader.
#[derive(Clone, Default, ShaderType, Debug, PartialEq)]
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
        "shaders/gas_cloud.wesl".into()
    }
}

pub type GasCloudMaterial = ExtendedMaterial<StandardMaterial, GasCloudExtension>;

/// Marker component for the volumetric gas cloud entity.
#[derive(Component)]
pub struct GasCloudDisk;

/// Distance-LOD tier for each stratified gas disk plane (`0` = core midplane,
/// `1` = inner vertical pair, `2` = outer vertical strata).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GasCloudLayerLod(pub u8);

/// Minimum gas density scale below which all gas disk layers are hidden to eliminate transparent fill-rate.
pub const GAS_CLOUD_CULL_DENSITY_THRESHOLD: f32 = 0.005;

/// Computes the highest visible [`GasCloudLayerLod`] tier for the current camera distance and angle,
/// or `None` when the disk has dissipated below [`GAS_CLOUD_CULL_DENSITY_THRESHOLD`].
///
/// - `None`: 0 layers visible (`density_scale < 0.005` or GMC scenario)
/// - `Some(0)`: 1 core midplane layer (far view or steep overhead view)
/// - `Some(1)`: 3 layers (mid-range view)
/// - `Some(2)`: all 7 vertical strata (close-up / oblique view)
#[must_use]
pub fn compute_gas_cloud_max_lod_tier(
    density_scale: f32,
    disk_outer_au: f32,
    cam_pos: Option<Vec3>,
) -> Option<u8> {
    if !density_scale.is_finite() || density_scale < GAS_CLOUD_CULL_DENSITY_THRESHOLD {
        return None;
    }
    let Some(pos) = cam_pos else {
        return Some(2);
    };
    let outer = disk_outer_au.max(1.0);
    let dist = pos.length();
    let overhead_sin = if dist > 1e-4 {
        (pos.y.abs() / dist).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if dist > outer * 3.5 || (dist > outer * 1.6 && overhead_sin > 0.78) {
        Some(0)
    } else if dist > outer * 1.4 || overhead_sin > 0.65 {
        Some(1)
    } else {
        Some(2)
    }
}

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

    // Multi-layered stratified 3D geometry spanning vertical flared scale-height,
    // tagged by distance-LOD tier (0 = core plane, 1 = mid pair, 2 = outer 4 strata).
    let layer_specs: [(f32, u8); 7] = [
        (-2.50, 2),
        (-1.20, 2),
        (-0.40, 1),
        (0.0, 0),
        (0.40, 1),
        (1.20, 2),
        (2.50, 2),
    ];

    for (y_offset, lod_tier) in layer_specs {
        commands.spawn((
            Mesh3d(plane_mesh.clone()),
            MeshMaterial3d(gas_material.clone()),
            Transform::from_translation(Vec3::new(0.0, y_offset, 0.0)),
            Visibility::default(),
            NotShadowCaster,
            NotShadowReceiver,
            GasCloudDisk,
            GasCloudLayerLod(lod_tier),
        ));
    }
}

#[allow(clippy::too_many_arguments, reason = "Gas disk star-uniform helper")]
fn compute_gas_star_uniforms(
    disk_params: &DiskParameters,
    is_massive: bool,
    is_compact: bool,
    star_entry: Option<(
        &Radius,
        &Temperature,
        &Luminosity,
        &IgnitionState,
        Option<&BlackHoleStarState>,
        &CelestialBody,
    )>,
    supernova_pool: Option<&crate::rendering::effects::supernova::SupernovaDebrisPool>,
) -> (f32, Vec4) {
    let Some((rad, temp, lum, ignition, opt_bhs, star_body)) = star_entry else {
        return (
            if is_massive { 2.0 } else { 0.5 },
            Vec4::new(0.013, 3600.0, 1.8, 0.0),
        );
    };

    let is_blown_out = opt_bhs.is_some_and(|s| s.is_blown_out);
    let is_black_hole = star_body.body_type == BodyType::BlackHole;

    let inner = if is_massive {
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

    let blowout_shock_r = supernova_pool
        .and_then(|pool| {
            pool.explosions
                .iter()
                .find(|e| {
                    e.explosion_type
                        == crate::rendering::effects::supernova::SupernovaType::QuasarBlowout
                })
                .map(|e| e.current_radius_au.min(240.0))
        })
        .unwrap_or(0.0);
    let effective_shock_r = if blowout_shock_r > 0.0 {
        (ignition.shockwave_radius as f32).max(blowout_shock_r)
    } else if is_massive && is_blown_out {
        // Leave a radiation-cleared AGN feedback cavity after the blowout front passes
        (ignition.shockwave_radius as f32).max(16.0)
    } else {
        ignition.shockwave_radius as f32
    };

    let star_p = Vec4::new(rad.0 as f32, temp.0 as f32, lum.0 as f32, effective_shock_r);
    (inner, star_p)
}

/// Updates gas cloud animation time, density clearance, distance-LOD visibility, and star ignition shockwave.
#[allow(
    clippy::too_many_arguments,
    reason = "Gas cloud LOD + uniform sync queries"
)]
pub fn update_gas_cloud_material(
    sim_time: Res<SimTime>,
    config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    mut materials: ResMut<Assets<GasCloudMaterial>>,
    mut gas_query: Query<
        (
            &MeshMaterial3d<GasCloudMaterial>,
            Option<&GasCloudLayerLod>,
            &mut Visibility,
        ),
        With<GasCloudDisk>,
    >,
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
    cam_query: Query<&GlobalTransform, With<Camera3d>>,
    supernova_pool: Option<Res<crate::rendering::effects::supernova::SupernovaDebrisPool>>,
) {
    // The GMC scenario renders gas through the 3D volumetric nebula; the planar disk must not appear.
    let is_gmc = scenario_state.is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster
    });
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

    let cam_pos = cam_query.iter().next().map(GlobalTransform::translation);
    let max_lod_tier = if is_gmc {
        None
    } else {
        compute_gas_cloud_max_lod_tier(density_scale, disk_outer, cam_pos)
    };

    for (_, opt_lod, mut vis) in &mut gas_query {
        let tier = opt_lod.map_or(0, |l| l.0);
        let target_vis = if max_lod_tier.is_some_and(|max_t| tier <= max_t) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        vis.set_if_neq(target_vis);
    }
    if max_lod_tier.is_none() {
        return;
    }

    let anim_time = (sim_time.elapsed_years * 60.0) as f32 + sim_time.visual_time_secs * 0.40;
    let (target_inner, star_params) = compute_gas_star_uniforms(
        &disk_params,
        is_massive,
        is_compact,
        star_query.single().ok(),
        supernova_pool.as_deref(),
    );

    let new_uniforms = GasUniforms {
        time_data: Vec4::new(anim_time, target_inner, disk_outer, density_scale),
        star_params,
    };

    // All layers share the same material handle; guard update to prevent redundant buffer uploads
    if let Some((handle, _, _)) = gas_query.iter().next()
        && let Some(mat) = materials.get(handle)
        && mat.extension.uniforms != new_uniforms
        && let Some(mut mat_mut) = materials.get_mut(handle)
    {
        mat_mut.extension.uniforms = new_uniforms;
    }
}

/// Plugin registering the gas cloud rendering pass.
pub struct GasCloudPlugin;

impl Plugin for GasCloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<GasCloudMaterial>::default())
            .add_systems(Startup, setup_gas_cloud_disk)
            .add_systems(
                Update,
                update_gas_cloud_material
                    .after(crate::simulation::physics::update_simulation_visual_time),
            );
    }
}
