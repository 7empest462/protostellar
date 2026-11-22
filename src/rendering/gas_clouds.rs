//! Volumetric Protoplanetary Gas Cloud Material and Mesh Spawner.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy_shader::ShaderRef;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Uniform parameters passed to the nebular gas cloud shader (64 bytes, 16-byte aligned).
#[derive(Clone, Default, ShaderType, Debug, PartialEq)]
pub struct GasUniforms {
    /// x: time, y: inner_radius, z: outer_radius, w: gas_density_scale
    pub time_data: Vec4,
    /// x: star_radius, y: star_temp, z: star_lum, w: shockwave_radius
    pub star_params: Vec4,
    /// xyz: camera simulation position (AU), w: close-up zoom contrast fade (`0.18..=1.0`)
    pub camera_params: Vec4,
    /// xyz: render-origin shifted disk center (AU), w: reserved
    pub disk_center: Vec4,
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

/// Vertical simulation-space offset (`AU`) for each stratified gas disk layer.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct GasCloudBaseY(pub f32);

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

/// Computes a smooth close-up contrast fade factor (`0.18..=1.0`) for background gas disk layers
/// so zooming tight onto a planet (e.g. Saturn or Earth inside a young protoplanetary disk) keeps
/// rings and deep-space contrast crisp instead of washing them out behind a bright beige veil.
#[must_use]
pub fn compute_gas_closeup_contrast_fade(
    orbit_radius_au: Option<f32>,
    focus_helio_dist_au: Option<f32>,
    disk_outer_au: f32,
) -> f32 {
    let Some(zoom_r) = orbit_radius_au else {
        return 1.0;
    };
    let outer = disk_outer_au.max(1.0);
    let focus_dist = focus_helio_dist_au.unwrap_or(0.0);
    // Only apply strong backdrop softening when zoomed tight on a body embedded inside the disk
    let in_disk_focus = if focus_dist > 0.05 && focus_dist < outer * 1.15 {
        1.0_f32
    } else {
        0.35_f32
    };
    let t = ((zoom_r - outer * 0.006) / (outer * 0.115)).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    let min_fade = 1.0 - 0.82 * in_disk_focus;
    min_fade + (1.0 - min_fade) * smooth
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
                camera_params: Vec4::new(0.0, 15.0, 35.0, 1.0),
                disk_center: Vec4::ZERO,
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
            GasCloudBaseY(y_offset),
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

fn compute_gas_disk_scale_and_density(
    disk_params: &DiskParameters,
    config: &SimulationConfig,
    is_massive: bool,
    is_compact: bool,
    is_remnant_or_pulsar: bool,
) -> (f32, f32) {
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

    (disk_outer, density_scale)
}

/// Updates gas cloud animation time, density clearance, distance-LOD visibility, and star ignition shockwave.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Gas cloud material, LOD tier, and floating render-origin synchronization"
)]
pub fn update_gas_cloud_material(
    sim_time: Res<SimTime>,
    config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
    mut materials: ResMut<Assets<GasCloudMaterial>>,
    mut gas_query: Query<
        (
            &mut Transform,
            &MeshMaterial3d<GasCloudMaterial>,
            Option<&GasCloudLayerLod>,
            Option<&GasCloudBaseY>,
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
    cam_query: Query<
        (&Transform, &GlobalTransform, Option<&PanOrbitCamera>),
        (With<Camera3d>, Without<GasCloudDisk>),
    >,
    supernova_pool: Option<Res<crate::rendering::effects::supernova::SupernovaDebrisPool>>,
) {
    // The GMC scenario renders gas through the 3D volumetric nebula; the planar disk must not appear.
    let is_gmc = scenario_state.is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster
    });
    let is_massive = disk_params.central_star_mass > 10.0 || disk_params.outer_radius_au > 100.0;
    let is_compact = disk_params.outer_radius_au < 2.0;
    let is_remnant_or_pulsar = star_query.single().is_ok_and(|(_, _, _, _, _, star_body)| {
        star_body.body_type == BodyType::Pulsar || star_body.body_type == BodyType::Magnetar
    });
    let (disk_outer, density_scale) = compute_gas_disk_scale_and_density(
        &disk_params,
        &config,
        is_massive,
        is_compact,
        is_remnant_or_pulsar,
    );

    let origin_v = opt_origin
        .as_deref()
        .copied()
        .unwrap_or_default()
        .0
        .as_vec3();
    let (cam_pos_sim, orbit_radius, focus_dist, in_surface_mode) =
        cam_query
            .iter()
            .next()
            .map_or((None, None, None, false), |(tf, gtf, opt_pan)| {
                let cam_render = if opt_pan.is_none() && tf.translation == Vec3::ZERO {
                    gtf.translation()
                } else {
                    tf.translation
                };
                (
                    Some(cam_render + origin_v),
                    opt_pan.map(|p| p.radius),
                    opt_pan.map(|p| p.focus.length()),
                    opt_pan.is_some_and(|p| p.surface_horizon_mode),
                )
            });
    let max_lod_tier = if is_gmc || in_surface_mode {
        None
    } else {
        compute_gas_cloud_max_lod_tier(density_scale, disk_outer, cam_pos_sim)
    };

    let mut shared_handle: Option<Handle<GasCloudMaterial>> = None;
    for (mut tf, mat_handle, opt_lod, opt_base_y, mut vis) in &mut gas_query {
        if shared_handle.is_none() {
            shared_handle = Some(mat_handle.0.clone());
        }
        let base_y = opt_base_y.map_or(tf.translation.y, |b| b.0);
        let target_t = Vec3::new(-origin_v.x, base_y - origin_v.y, -origin_v.z);
        if tf.translation != target_t {
            tf.translation = target_t;
        }
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

    let cam_p = cam_pos_sim.unwrap_or(Vec3::new(0.0, 15.0, 35.0));
    let contrast_fade = compute_gas_closeup_contrast_fade(orbit_radius, focus_dist, disk_outer);
    let new_uniforms = GasUniforms {
        time_data: Vec4::new(anim_time, target_inner, disk_outer, density_scale),
        star_params,
        camera_params: Vec4::new(cam_p.x, cam_p.y, cam_p.z, contrast_fade),
        disk_center: Vec4::new(-origin_v.x, -origin_v.y, -origin_v.z, 0.0),
    };

    // All layers share the same material handle; guard update to prevent redundant buffer uploads
    if let Some(handle) = shared_handle
        && let Some(mat) = materials.get(&handle)
        && mat.extension.uniforms != new_uniforms
        && let Some(mut mat_mut) = materials.get_mut(&handle)
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
                    .after(crate::rendering::origin::update_render_origin)
                    .after(crate::rendering::camera::update_pan_orbit_camera),
            );
    }
}
