use crate::rendering::bodies::compute_stellar_palette;
use crate::simulation::accretion::RocheLobeOverflow;
use crate::simulation::components::{
    BodyType, CelestialBody, Radius, SimPosition, SimVelocity, Temperature,
};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy_shader::ShaderRef;
use hashbrown::HashSet;
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RocheStreamUniforms {
    pub color_and_intensity: Vec4,
    /// x: time, y: turbulence_scale, z: stream_width, w: unused
    pub stream_params: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RocheStreamMaterial {
    #[uniform(0)]
    pub uniforms: RocheStreamUniforms,
}

impl Material for RocheStreamMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/roche_stream.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Component)]
pub struct RocheStreamVisual {
    pub donor: Entity,
    pub accretor: Entity,
}

/// Cached procedural curved spiral tube mesh for Roche lobe overflow streams.
#[derive(Resource)]
pub struct RocheStreamMeshCache(pub Handle<Mesh>);

/// Generates an open-ended 3D curved ballistic Roche stream tube along Y in [0.0, 1.0].
/// - y = 0.0: Donor star surface (wide tidal funnel envelope)
/// - y = 0.25: L1 Lagrange nozzle pinch
/// - y = 0.25..1.0: Coriolis-deflected spiral arc flaring into the accretion disk
///
/// Generated in world-proportioned coordinates so uniform/near-uniform scaling
/// preserves true 3D Frenet-Serret normals from all camera angles.
pub fn generate_curved_roche_tube_mesh() -> Mesh {
    let rings = 64usize;
    let sectors = 24usize;

    let mut positions = Vec::with_capacity((rings + 1) * (sectors + 1));
    let mut normals = Vec::with_capacity((rings + 1) * (sectors + 1));
    let mut uvs = Vec::with_capacity((rings + 1) * (sectors + 1));
    let mut indices = Vec::with_capacity(rings * sectors * 6);

    let spine_point = |s: f32| -> Vec3 {
        // Clamp sin() to >= 0.0 before powf() because f32::consts::PI > true PI,
        // which makes sin(1.0 * PI) slightly negative (-8.74e-8) and would cause powf() -> NaN!
        let sin_s = (s * std::f32::consts::PI).sin().max(0.0);
        // Ballistic Coriolis deflection in +X
        let coriolis_x = sin_s.powf(1.15) * 0.30;
        // Subtle 3D corkscrew frame-dragging near the accretor (s > 0.5)
        let spiral_z = (s * 4.5).sin() * 0.035 * s * s;
        Vec3::new(coriolis_x, s, spiral_z)
    };

    for i in 0..=rings {
        let s = i as f32 / rings as f32;
        let center = spine_point(s);

        let ds = 0.005;
        let s_next = (s + ds).min(1.0);
        let s_prev = (s - ds).max(0.0);
        let tangent = (spine_point(s_next) - spine_point(s_prev)).normalize_or_zero();
        let ref_vec = if tangent.z.abs() < 0.9 {
            Vec3::Z
        } else {
            Vec3::X
        };
        let binormal = tangent.cross(ref_vec).normalize_or_zero();
        let normal_axis = binormal.cross(tangent).normalize_or_zero();

        // Teardrop radius profile:
        // Fits snugly inside the donor star's photosphere at s=0 (r=0.050),
        // pinches down through the L1 nozzle at s=0.25 (r=0.026),
        // widens gently along the tidal stream (s=0.75 -> r=0.038),
        // and tapers into the accretion disk at s=1.0 (r=0.024).
        let donor_cone = (1.0 - s / 0.26).max(0.0).powf(1.35) * 0.024;
        let disk_fan = (((s - 0.24).max(0.0) / 0.76) * std::f32::consts::PI)
            .sin()
            .max(0.0)
            * 0.014;
        let r_tube = 0.026 + donor_cone + disk_fan;

        for j in 0..=sectors {
            let u = j as f32 / sectors as f32;
            let theta = u * TAU;
            let (sin_t, cos_t) = theta.sin_cos();

            let local_n = (normal_axis * cos_t + binormal * sin_t).normalize_or_zero();
            let offset = local_n * r_tube;

            let pos = center + offset;
            positions.push(pos.to_array());
            normals.push(local_n.to_array());
            uvs.push([u, s]);
        }
    }

    let stride = (sectors + 1) as u32;
    for i in 0..rings as u32 {
        for j in 0..sectors as u32 {
            let a = i * stride + j;
            let b = (i + 1) * stride + j;
            let c = (i + 1) * stride + (j + 1);
            let d = i * stride + (j + 1);

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

fn compute_roche_stream_pose(
    p_donor: Vec3,
    p_accretor: Vec3,
    v_dir: Vec3,
    donor_vis_r: f32,
    acc_vis_r: f32,
    is_black_hole: bool,
) -> Option<(Vec3, Quat, f32, f32)> {
    let to_accretor = p_accretor - p_donor;
    let dist = to_accretor.length();
    if dist < 1e-4 {
        return None;
    }
    let y_axis = to_accretor / dist;

    // Orient +X of the curved mesh along the donor's orbital velocity (Coriolis deflection)
    let mut z_axis = y_axis.cross(v_dir).normalize_or_zero();
    if z_axis.length_squared() < 0.01 {
        z_axis = if y_axis.y.abs() < 0.9 {
            y_axis.cross(Vec3::Y).normalize_or_zero()
        } else {
            y_axis.cross(Vec3::X).normalize_or_zero()
        };
    }
    let x_axis = v_dir
        .reject_from(y_axis)
        .try_normalize()
        .unwrap_or_else(|| z_axis.cross(y_axis).normalize_or_zero());
    let z_axis = x_axis.cross(y_axis).normalize_or_zero();
    let rotation = Quat::from_mat3(&Mat3::from_cols(x_axis, y_axis, z_axis));

    // Start inside the donor's visual photosphere and end at the accretion disk ring
    let start_offset = (donor_vis_r * 0.45).min(dist * 0.15);
    let end_offset = if is_black_hole {
        (acc_vis_r * 0.78).min(dist * 0.16)
    } else {
        (acc_vis_r * 0.50).min(dist * 0.16)
    };
    let span = (dist - start_offset - end_offset).max(dist * 0.55);
    let start_pos = p_donor + y_axis * start_offset;

    // Scale cross-section so the star-side cone (r_tube = 0.050) connects to the L1 teardrop nozzle
    // (~32% of donor_vis_r, pinching to a sleek ~16% filament), preventing gigantic megatubes in wide orbits.
    let target_star_fit = (donor_vis_r * 0.32) / 0.050;
    let max_cross = (span * 0.25).max(donor_vis_r * 4.0);
    let min_cross = (donor_vis_r * 1.5).min(span * 0.10).max(0.001);
    let cross_scale = target_star_fit.clamp(min_cross, max_cross);
    Some((start_pos, rotation, span, cross_scale))
}

fn spawn_roche_stream_visual(
    commands: &mut Commands,
    materials: &mut Assets<RocheStreamMaterial>,
    stream_mesh: Handle<Mesh>,
    donor: Entity,
    accretor: Entity,
    start_pos: Vec3,
    rotation: Quat,
    span: f32,
    cross_scale: f32,
    color_and_intensity: Vec4,
) {
    let mat = materials.add(RocheStreamMaterial {
        uniforms: RocheStreamUniforms {
            color_and_intensity,
            stream_params: Vec4::new(0.0, 5.0, cross_scale, 1.0),
        },
    });

    commands.spawn((
        RocheStreamVisual { donor, accretor },
        Mesh3d(stream_mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(start_pos)
            .with_rotation(rotation)
            .with_scale(Vec3::new(cross_scale, span, cross_scale)),
        bevy::camera::visibility::NoFrustumCulling,
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
    ));
}

#[allow(
    clippy::disallowed_types,
    clippy::type_complexity,
    reason = "Needed to update streams"
)]
pub fn update_roche_streams(
    mut commands: Commands,
    query_overflows: Query<(
        Entity,
        &RocheLobeOverflow,
        &SimPosition,
        &SimVelocity,
        &Radius,
        &Temperature,
        &CelestialBody,
    )>,
    query_targets: Query<(&SimPosition, &Radius, &CelestialBody)>,
    mut query_visuals: Query<(
        Entity,
        &RocheStreamVisual,
        &mut Transform,
        &MeshMaterial3d<RocheStreamMaterial>,
    )>,
    mut materials: ResMut<Assets<RocheStreamMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_cache: Option<Res<RocheStreamMeshCache>>,
    time: Res<Time>,
    config: Res<crate::simulation::resources::SimulationConfig>,
) {
    let stream_mesh = if let Some(cache) = mesh_cache {
        cache.0.clone()
    } else {
        let handle = meshes.add(generate_curved_roche_tube_mesh());
        commands.insert_resource(RocheStreamMeshCache(handle.clone()));
        handle
    };

    let mut active_streams = HashSet::new();

    for (donor_ent, overflow, donor_pos, donor_vel, donor_radius, donor_temp, donor_body) in
        query_overflows.iter()
    {
        if let Ok((accretor_pos, accretor_rad, accretor_body)) =
            query_targets.get(overflow.companion)
        {
            active_streams.insert(donor_ent);

            let donor_vis_r =
                config.calc_visual_radius_for_type(donor_radius.0, donor_body.body_type);
            let acc_vis_r =
                config.calc_visual_radius_for_type(accretor_rad.0, accretor_body.body_type);
            let Some((start_pos, rotation, span, cross_scale)) = compute_roche_stream_pose(
                donor_pos.0.as_vec3(),
                accretor_pos.0.as_vec3(),
                donor_vel.0.as_vec3().normalize_or_zero(),
                donor_vis_r,
                acc_vis_r,
                accretor_body.body_type == BodyType::BlackHole,
            ) else {
                continue;
            };

            let base_col = compute_stellar_palette(donor_body.body_type, donor_temp.0).to_srgba();
            let intensity = (overflow.mass_transfer_rate.log10() + 10.5).clamp(1.4, 3.0) as f32;
            let color_and_intensity =
                Vec4::new(base_col.red, base_col.green, base_col.blue, intensity);

            let mut found = false;
            for (_vis_ent, visual, mut transform, mat_handle) in query_visuals.iter_mut() {
                if visual.donor == donor_ent && visual.accretor == overflow.companion {
                    found = true;
                    transform.translation = start_pos;
                    transform.rotation = rotation;
                    transform.scale = Vec3::new(cross_scale, span, cross_scale);

                    if let Some(mut mat) = materials.get_mut(mat_handle.id()) {
                        mat.uniforms.stream_params.x += time.delta_secs() * 1.8;
                        mat.uniforms.stream_params.z = cross_scale;
                        mat.uniforms.color_and_intensity = color_and_intensity;
                    }
                    break;
                }
            }

            if !found {
                spawn_roche_stream_visual(
                    &mut commands,
                    &mut materials,
                    stream_mesh.clone(),
                    donor_ent,
                    overflow.companion,
                    start_pos,
                    rotation,
                    span,
                    cross_scale,
                    color_and_intensity,
                );
            }
        }
    }

    for (vis_ent, visual, _, _) in query_visuals.iter() {
        if !active_streams.contains(&visual.donor) {
            commands.entity(vis_ent).despawn();
        }
    }
}
