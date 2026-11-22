//! Relativistic Tidal Disruption Event (TDE) plasma streamer simulation and 3D volumetric rendering.
//!
//! Models relativistic tidal shearing of stars and planets into infalling spiral plasma ribbons
//! funneled into a black hole's ISCO and accretion disk with canonical Rees fallback luminosity
//! scaling: dM/dt ∝ (1 + t/t_peak)^(-5/3), rendered as a live-updating 3D Frenet-Serret volumetric
//! tube through `roche_stream.wesl` with a blazing pericenter circularization shock core and light.

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use rustc_hash::{FxHashMap, FxHashSet};
use std::f32::consts::TAU;

use crate::rendering::bodies::VisualAssets;
use crate::rendering::effects::roche_stream::{RocheStreamMaterial, RocheStreamUniforms};
use crate::simulation::accretion::TidalDisruptionEvent;
use crate::simulation::components::SimPosition;
use crate::simulation::resources::{TidalDisruptionPool, TidalDisruptionStream};

const TDE_TUBE_SECTORS: usize = 10;

/// Marker component for the 3D volumetric TDE spaghettified plasma ribbon mesh.
#[derive(Component, Debug, Clone)]
pub struct TdeStreamVisual {
    pub bh_entity: Entity,
    pub mesh_handle: Handle<Mesh>,
}

/// Marker component for the incandescent pericenter self-intersection shock core at BH ISCO.
#[derive(Component, Debug, Clone, Copy)]
pub struct TdeShockCore {
    pub bh_entity: Entity,
}

/// Marker component for the dynamic Rees $t^{-5/3}$ TDE flare `PointLight`.
#[derive(Component, Debug, Clone, Copy)]
pub struct TdeFlareLight {
    pub bh_entity: Entity,
}

/// System that collects TDE messages and simulates relativistic infall, Keplerian shear,
/// and azimuthal tidal compression of the plasma stream.
pub fn update_tidal_disruption_streams(
    time: Res<Time>,
    mut tde_pool: ResMut<TidalDisruptionPool>,
    mut tde_reader: MessageReader<TidalDisruptionEvent>,
    bodies_query: Query<&SimPosition>,
) {
    let dt = time.delta_secs();

    for ev in tde_reader.read() {
        let n_nodes = 72;
        let mut stream_nodes = Vec::with_capacity(n_nodes);

        let delta = ev.disruption_pos - ev.bh_pos;
        let base_azimuth = delta.z.atan2(delta.x);
        let r_disrupt = delta.xz().length().max(0.005);

        // ISCO radius calculated from Schwarzschild geometry (3 r_s = 6 G M / c^2)
        let r_schwa = (1.974e-8 * ev.bh_mass_solar) as f32;
        let r_isco_calc = (r_schwa * 3.0).max(ev.isco_radius_au as f32);
        let r_max = (r_disrupt * 0.45).max(0.0001);
        let r_isco = r_isco_calc.clamp(0.0001, r_max);
        let r_tidal = r_disrupt.max(r_isco * 1.5);

        for k in 0..n_nodes {
            let s = k as f32 / (n_nodes as f32); // Infall parameter: 0 at outer tail, 1 at ISCO
            // Logarithmic spiral geodesic from r_tidal down to r_isco
            let node_r = r_isco + (r_tidal - r_isco) * (1.0 - s).powf(1.4);
            // Multi-turn relativistic wrap (unwinds ~3.5 turns)
            let node_azimuth = base_azimuth + s * (TAU * 3.5);
            // Tidal vertical compression towards equatorial plane: h(r) ∝ (r / r_t)^(3/4)
            let vert_scale = (node_r / r_tidal).powf(0.75) * 0.015;
            let z_jitter = ((k as f32 * 2.3).sin() * vert_scale) * (1.0 - s * 0.7);
            let intensity = (1.0 - s * 0.3).clamp(0.0, 1.0);

            stream_nodes.push((s, node_r, node_azimuth, z_jitter, intensity));
        }

        tde_pool.streams.push(TidalDisruptionStream {
            bh_entity: ev.bh_entity,
            bh_pos: ev.bh_pos,
            disruption_pos: ev.disruption_pos,
            r_tidal,
            r_isco,
            timer: 0.0,
            max_timer: 6.0,
            initial_mass_solar: ev.body_mass_solar,
            initial_velocity: ev.initial_velocity,
            stream_nodes,
        });
    }

    tde_pool.streams.retain_mut(|stream| {
        stream.timer += dt;

        if let Ok(pos) = bodies_query.get(stream.bh_entity) {
            stream.bh_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        }

        // Keplerian relativistic differential shear: inner nodes revolve much faster
        for node in &mut stream.stream_nodes {
            let r = node.1.max(0.002);
            let omega = (2.2 / (r * r * r).sqrt()).clamp(1.5, 35.0);
            node.2 += omega * dt;
            // Radial drift inwards toward ISCO
            let drift_rate = (0.25 / r.sqrt()).clamp(0.05, 0.8) * dt;
            node.1 = (node.1 - drift_rate).max(stream.r_isco * 0.95);
        }

        stream.timer < stream.max_timer
    });
}

fn fill_tde_tube_vertices(
    stream: &TidalDisruptionStream,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: Option<&mut Vec<[f32; 2]>>,
) {
    let n = stream.stream_nodes.len();
    if n < 2 {
        return;
    }
    let p = (stream.timer / stream.max_timer.max(0.1)).clamp(0.0, 1.0);
    let base_tube_r = (stream.r_tidal * 0.032).clamp(0.004, 0.14);

    positions.clear();
    normals.clear();
    let mut uv_opt = uvs;
    if let Some(ref mut uv_vec) = uv_opt {
        uv_vec.clear();
    }

    for i in 0..n {
        let Some(&(s, r_i, phi_i, z_i, _)) = stream.stream_nodes.get(i) else {
            continue;
        };
        let center = Vec3::new(r_i * phi_i.cos(), z_i, r_i * phi_i.sin());
        let prev = if i > 0 {
            stream.stream_nodes.get(i - 1).map_or(center, |n_p| {
                Vec3::new(n_p.1 * n_p.2.cos(), n_p.3, n_p.1 * n_p.2.sin())
            })
        } else {
            center
        };
        let next = if i + 1 < n {
            stream.stream_nodes.get(i + 1).map_or(center, |n_n| {
                Vec3::new(n_n.1 * n_n.2.cos(), n_n.3, n_n.1 * n_n.2.sin())
            })
        } else {
            center
        };

        let tangent = (next - prev).normalize_or(Vec3::X);
        let ref_up = if tangent.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::Z
        };
        let binormal = tangent.cross(ref_up).normalize_or(Vec3::Z);
        let normal_axis = binormal.cross(tangent).normalize_or(Vec3::Y);

        // Wide shredded stellar tail -> tight superheated geodesic -> flared ISCO shock collar
        let tail_fan = (1.0 - s).powf(1.5) * 1.15;
        let isco_flare = ((s - 0.72).max(0.0) / 0.28).powi(2) * 0.85;
        let r_tube = base_tube_r * (0.48 + tail_fan + isco_flare) * (1.0 - p * 0.35);

        for j in 0..=TDE_TUBE_SECTORS {
            let u = j as f32 / (TDE_TUBE_SECTORS as f32);
            let (sin_t, cos_t) = (u * TAU).sin_cos();
            // Slightly ribbon-flattened vertically by tidal compression
            let local_n = (normal_axis * cos_t + binormal * (sin_t * 0.68)).normalize_or(Vec3::Y);
            let pos = center + (normal_axis * cos_t + binormal * (sin_t * 0.55)) * r_tube;
            positions.push(pos.to_array());
            normals.push(local_n.to_array());
            if let Some(ref mut uv_vec) = uv_opt {
                uv_vec.push([u, s]);
            }
        }
    }
}

/// Builds a fresh 3D Frenet-Serret volumetric plasma tube mesh wrapping `stream.stream_nodes`.
#[must_use]
pub fn build_tde_spiral_tube_mesh(stream: &TidalDisruptionStream) -> Mesh {
    let n = stream.stream_nodes.len();
    let stride = TDE_TUBE_SECTORS + 1;
    let vert_cap = n * stride;
    let mut positions = Vec::with_capacity(vert_cap);
    let mut normals = Vec::with_capacity(vert_cap);
    let mut uvs = Vec::with_capacity(vert_cap);

    fill_tde_tube_vertices(stream, &mut positions, &mut normals, Some(&mut uvs));

    let mut indices = Vec::with_capacity(n.saturating_sub(1) * TDE_TUBE_SECTORS * 6);
    for i in 0..n.saturating_sub(1) as u32 {
        for j in 0..TDE_TUBE_SECTORS as u32 {
            let a = i * stride as u32 + j;
            let b = (i + 1) * stride as u32 + j;
            let c = (i + 1) * stride as u32 + (j + 1);
            let d = i * stride as u32 + (j + 1);
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn update_tde_spiral_tube_mesh(mesh: &mut Mesh, stream: &TidalDisruptionStream) {
    let expected = stream.stream_nodes.len() * (TDE_TUBE_SECTORS + 1);
    let mut positions = Vec::with_capacity(expected);
    let mut normals = Vec::with_capacity(expected);
    fill_tde_tube_vertices(stream, &mut positions, &mut normals, None);

    if let Some(VertexAttributeValues::Float32x3(pos_attr)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        && pos_attr.len() == positions.len()
    {
        pos_attr.copy_from_slice(&positions);
    }
    if let Some(VertexAttributeValues::Float32x3(norm_attr)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
        && norm_attr.len() == normals.len()
    {
        norm_attr.copy_from_slice(&normals);
    }
}

fn tde_fallback_envelope(stream: &TidalDisruptionStream) -> f32 {
    let p = (stream.timer / stream.max_timer.max(0.1)).clamp(0.0, 1.0);
    let fallback_lum = (1.0 + 3.0 * p).powf(-5.0 / 3.0);
    let fade = if p < 0.08 {
        p / 0.08
    } else if p > 0.75 {
        (1.0 - p) / 0.25
    } else {
        1.0
    };
    (fallback_lum * fade).clamp(0.0, 1.0)
}

fn build_tde_stream_uniforms(stream: &TidalDisruptionStream) -> RocheStreamUniforms {
    let env = tde_fallback_envelope(stream);
    RocheStreamUniforms {
        // Fiery superheated crimson-gold stellar tail feeding into white-cyan ISCO shock
        color_and_intensity: Vec4::new(1.15, 0.42, 0.14, 1.2 + 1.55 * env),
        stream_params: Vec4::new(stream.timer * 1.4, 3.6, 1.0, env),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Spawns missing TDE stream tube, shock core, and flare light assets"
)]
fn spawn_missing_tde_visuals(
    commands: &mut Commands,
    bh_entity: Entity,
    stream: &TidalDisruptionStream,
    bh_render_pos: Vec3,
    has_stream: bool,
    has_core: bool,
    has_light: bool,
    assets: &VisualAssets,
    meshes: &mut Assets<Mesh>,
    roche_mats: &mut Assets<RocheStreamMaterial>,
    std_mats: &mut Assets<StandardMaterial>,
) {
    if !has_stream {
        let mesh_handle = meshes.add(build_tde_spiral_tube_mesh(stream));
        let mat_handle = roche_mats.add(RocheStreamMaterial {
            uniforms: build_tde_stream_uniforms(stream),
        });
        commands.spawn((
            TdeStreamVisual {
                bh_entity,
                mesh_handle: mesh_handle.clone(),
            },
            Mesh3d(mesh_handle),
            MeshMaterial3d(mat_handle),
            Transform::from_translation(bh_render_pos),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
    if !has_core {
        let shock_mat = std_mats.add(StandardMaterial {
            base_color: Color::linear_rgba(8.5, 11.5, 18.0, 0.72),
            emissive: LinearRgba::new(8.5, 11.5, 18.0, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        let core_r = (stream.r_isco * 1.65).clamp(0.015, 0.28);
        commands.spawn((
            TdeShockCore { bh_entity },
            Mesh3d(assets.star_mesh.clone()),
            MeshMaterial3d(shock_mat),
            Transform::from_translation(bh_render_pos).with_scale(Vec3::new(
                core_r * 1.45,
                core_r * 0.42,
                core_r * 1.45,
            )),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
    if !has_light {
        commands.spawn((
            TdeFlareLight { bh_entity },
            PointLight {
                color: Color::srgb(0.85, 0.92, 1.0),
                intensity: 6.5e8,
                range: 120.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(bh_render_pos),
        ));
    }
}

/// Synchronizes 3D volumetric spaghettified TDE plasma ribbons (`RocheStreamMaterial`),
/// pericenter shock-flare cores, and dynamic Rees $t^{-5/3}$ `PointLight`s.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "TDE 3D volumetric sync coordinates live tube mesh, shock core, and flare light"
)]
pub fn sync_tde_visuals(
    mut commands: Commands,
    tde_pool: Option<Res<TidalDisruptionPool>>,
    visual_assets: Option<Res<VisualAssets>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut roche_mats: Option<ResMut<Assets<RocheStreamMaterial>>>,
    mut std_mats: Option<ResMut<Assets<StandardMaterial>>>,
    mut stream_query: Query<(
        Entity,
        &TdeStreamVisual,
        &mut Transform,
        &MeshMaterial3d<RocheStreamMaterial>,
    )>,
    mut core_query: Query<
        (Entity, &TdeShockCore, &mut Transform),
        (Without<TdeStreamVisual>, Without<TdeFlareLight>),
    >,
    mut light_query: Query<
        (Entity, &TdeFlareLight, &mut PointLight, &mut Transform),
        (Without<TdeStreamVisual>, Without<TdeShockCore>),
    >,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let (Some(pool), Some(assets), Some(meshes), Some(roche_mats), Some(std_mats)) = (
        tde_pool,
        visual_assets,
        meshes.as_deref_mut(),
        roche_mats.as_deref_mut(),
        std_mats.as_deref_mut(),
    ) else {
        return;
    };
    let origin = opt_origin.as_deref().copied().unwrap_or_default();

    let mut active: FxHashMap<Entity, &TidalDisruptionStream> = FxHashMap::default();
    for stream in &pool.streams {
        active.insert(stream.bh_entity, stream);
    }

    let mut has_stream: FxHashSet<Entity> = FxHashSet::default();
    for (ent, vis, mut tf, mat_handle) in stream_query.iter_mut() {
        let Some(&stream) = active.get(&vis.bh_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        has_stream.insert(vis.bh_entity);
        let bh_render_pos = origin.shift_vec3(stream.bh_pos);
        if tf.translation != bh_render_pos {
            tf.translation = bh_render_pos;
        }
        if let Some(mut mesh) = meshes.get_mut(&vis.mesh_handle) {
            update_tde_spiral_tube_mesh(&mut mesh, stream);
        }
        let next_u = build_tde_stream_uniforms(stream);
        if let Some(mat) = roche_mats.get(&mat_handle.0)
            && mat.uniforms != next_u
            && let Some(mut mat_mut) = roche_mats.get_mut(&mat_handle.0)
        {
            mat_mut.uniforms = next_u;
        }
    }

    let mut has_core: FxHashSet<Entity> = FxHashSet::default();
    for (ent, core, mut tf) in core_query.iter_mut() {
        let Some(&stream) = active.get(&core.bh_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        has_core.insert(core.bh_entity);
        let bh_render_pos = origin.shift_vec3(stream.bh_pos);
        let env = tde_fallback_envelope(stream);
        let pulse = 1.0 + 0.16 * (stream.timer * 14.0).sin();
        let core_r = (stream.r_isco * 1.65).clamp(0.015, 0.28) * (0.45 + 0.85 * env) * pulse;
        let target_scale = Vec3::new(core_r * 1.45, core_r * 0.42, core_r * 1.45);
        if tf.translation != bh_render_pos {
            tf.translation = bh_render_pos;
        }
        if tf.scale != target_scale {
            tf.scale = target_scale;
        }
    }

    let mut has_light: FxHashSet<Entity> = FxHashSet::default();
    for (ent, fl, mut light, mut tf) in light_query.iter_mut() {
        let Some(&stream) = active.get(&fl.bh_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        has_light.insert(fl.bh_entity);
        let bh_render_pos = origin.shift_vec3(stream.bh_pos);
        let env = tde_fallback_envelope(stream);
        let target_intensity = 6.5e8 * env;
        if tf.translation != bh_render_pos {
            tf.translation = bh_render_pos;
        }
        if (light.intensity - target_intensity).abs() > 1e5 {
            light.intensity = target_intensity;
        }
    }

    for (&bh_entity, &stream) in &active {
        let bh_render_pos = origin.shift_vec3(stream.bh_pos);
        spawn_missing_tde_visuals(
            &mut commands,
            bh_entity,
            stream,
            bh_render_pos,
            has_stream.contains(&bh_entity),
            has_core.contains(&bh_entity),
            has_light.contains(&bh_entity),
            &assets,
            meshes,
            roche_mats,
            std_mats,
        );
    }
}

/// Legacy gizmo hook retained for API compatibility; TDE spaghettification streams now render
/// as live-updating 3D volumetric plasma ribbons via [`sync_tde_visuals`].
pub fn draw_tidal_disruption_streams(_gizmos: &mut Gizmos, _tde_pool: &TidalDisruptionPool) {}
