//! Visual mesh generation, PBR materials, and real-time transform synchronization.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Marker for an entity that has its visual mesh and material spawned.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct VisualBody;

/// Common shared meshes and materials cache.
#[derive(Resource)]
pub struct VisualAssets {
    pub star_mesh: Handle<Mesh>,
    pub planet_mesh: Handle<Mesh>,
    pub asteroid_potato_mesh: Handle<Mesh>,
    pub asteroid_rubble_mesh: Handle<Mesh>,
    pub comet_bilobate_mesh: Handle<Mesh>,
    pub particle_mesh: Handle<Mesh>,
    pub ring_mesh: Handle<Mesh>,
    pub beam_core_mesh: Handle<Mesh>,
    pub beam_sheath_mesh: Handle<Mesh>,
    pub accretion_disk_mesh: Handle<Mesh>,
    pub pulsar_beam_mesh: Handle<Mesh>,
    pub magnetar_ring_mesh: Handle<Mesh>,
    pub magnetar_field_loops_mesh: Handle<Mesh>,
}

/// Recomputes smooth vertex normals for procedural meshes.
fn recompute_mesh_normals(mesh: &mut Mesh) {
    if let (Some(VertexAttributeValues::Float32x3(pos)), Some(Indices::U32(indices))) =
        (mesh.attribute(Mesh::ATTRIBUTE_POSITION), mesh.indices())
    {
        let mut normals = vec![Vec3::ZERO; pos.len()];
        for chunk in indices.chunks(3) {
            let i0 = chunk[0] as usize;
            let i1 = chunk[1] as usize;
            let i2 = chunk[2] as usize;
            let v0 = Vec3::from_array(pos[i0]);
            let v1 = Vec3::from_array(pos[i1]);
            let v2 = Vec3::from_array(pos[i2]);
            let normal = (v1 - v0).cross(v2 - v0);
            normals[i0] += normal;
            normals[i1] += normal;
            normals[i2] += normal;
        }
        let normalized: Vec<[f32; 3]> = normals
            .into_iter()
            .map(|n| n.normalize_or_zero().to_array())
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normalized);
    }
}

/// Procedurally generates an irregular, non-spherical 3D asteroid mesh with triaxial elongation,
/// multi-octave harmonic noise displacement, and realistic impact crater depressions.
fn generate_irregular_asteroid_mesh(
    elongation: Vec3,
    noise_strength: f32,
    is_bilobate: bool,
    seed: f32,
) -> Mesh {
    let mut sphere = Sphere::new(1.0).mesh().ico(4).unwrap();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        sphere.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return sphere;
    };

    let mut new_positions = Vec::with_capacity(positions.len());
    let craters = [
        (Vec3::new(0.6, 0.7, 0.3).normalize(), 0.55f32, 0.18f32),
        (Vec3::new(-0.7, 0.2, 0.65).normalize(), 0.45f32, 0.14f32),
        (Vec3::new(0.1, -0.9, 0.4).normalize(), 0.60f32, 0.20f32),
        (Vec3::new(-0.4, -0.4, -0.8).normalize(), 0.38f32, 0.12f32),
    ];

    for p in positions {
        let v = Vec3::from_array(*p).normalize();

        // 1. Bilobate contact binary deformation (e.g. 67P / Arrokoth)
        let base_pos = if is_bilobate {
            if v.x > 0.1 {
                let lobe1 =
                    (v - Vec3::new(0.45, 0.0, 0.0)).normalize() * 0.85 + Vec3::new(0.45, 0.0, 0.0);
                lobe1 * elongation
            } else if v.x < -0.1 {
                let lobe2 =
                    (v + Vec3::new(0.40, 0.0, 0.0)).normalize() * 0.70 - Vec3::new(0.40, 0.0, 0.0);
                lobe2 * elongation
            } else {
                v * elongation * 0.65 // Neck constriction
            }
        } else {
            v * elongation
        };

        // 2. Isotropic spherical surface harmonics (avoids Cartesian coordinate-axis cubic alignment)
        let k1 = v.dot(Vec3::new(0.577, 0.577, 0.577));
        let k2 = v.dot(Vec3::new(-0.707, 0.0, 0.707));
        let k3 = v.dot(Vec3::new(0.267, -0.802, 0.534));
        let k4 = v.dot(Vec3::new(-0.408, 0.816, -0.408));

        let d1 = (k1 * 3.2 + seed).sin() * 0.12;
        let d2 = (k2 * 5.4 + seed * 1.6).sin() * 0.06;
        let d3 = (k3 * 8.1 + seed * 2.4).sin() * 0.03;
        let d4 = (k4 * 12.3 + seed * 3.5).cos() * 0.015;
        let mut disp = 1.0 + (d1 + d2 + d3 + d4) * noise_strength;

        // 3. Impact Crater Depressions with Elevated Rims
        for &(c_center, c_rad, c_depth) in &craters {
            let angle = (v.dot(c_center)).clamp(-1.0, 1.0).acos();
            if angle < c_rad {
                let norm_dist = angle / c_rad;
                let crater_pit = c_depth * (1.0 - norm_dist * norm_dist);
                let rim_boost = (c_depth * 0.35) * (-((norm_dist - 0.95) / 0.15).powi(2)).exp();
                disp = disp - crater_pit + rim_boost;
            }
        }

        let final_p = base_pos * disp.max(0.2);
        new_positions.push(final_p.to_array());
    }

    sphere.insert_attribute(Mesh::ATTRIBUTE_POSITION, new_positions);
    recompute_mesh_normals(&mut sphere);
    sphere
}

/// Procedurally generates a flaring conical lighthouse beam mesh with
/// central laser filament, outer frustum sheath, and internal radial fan-ray striations,
/// with vertex colors that fade out to 0 alpha at the tip to eliminate any flat cylinder caps.
fn generate_pulsar_beam_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let length = 8.0f32; // 8 AU
    let base_r = 0.0012f32; // Ultra-thin needle emission at pulsar pole, strictly smaller than the star
    let tip_r = 0.65f32; // Flares out gracefully into orbital space
    let num_rings = 24;
    let num_segments = 24;

    // 1. Conical outer sheath frustum with trumpet-bevel flare
    let sheath_start_idx = positions.len() as u32;
    for ring in 0..=num_rings {
        let t = (ring as f32) / (num_rings as f32);
        let y = t * length;
        // Non-linear trumpet flare curve: stays needle-thin near the star's polar cap, then cones out into space
        let flare = t.powf(1.70);
        let r = base_r + flare * (tip_r - base_r);
        let alpha = (1.0 - t).powi(2) * 0.55;
        let color = [0.45, 0.85, 1.0, alpha];

        for seg in 0..=num_segments {
            let phi = (seg as f32) * (std::f32::consts::TAU / (num_segments as f32));
            let x = r * phi.cos();
            let z = r * phi.sin();

            positions.push([x, y, z]);
            normals.push([phi.cos(), 0.1, phi.sin()]);
            colors.push(color);
        }
    }

    for ring in 0..num_rings {
        for seg in 0..num_segments {
            let stride = (num_segments + 1) as u32;
            let i0 = sheath_start_idx + ring as u32 * stride + seg as u32;
            let i1 = i0 + 1;
            let i2 = sheath_start_idx + (ring + 1) as u32 * stride + seg as u32;
            let i3 = i2 + 1;

            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]); // Double-sided
        }
    }

    // 2. High-intensity narrow central core cone
    let core_start_idx = positions.len() as u32;
    let core_base_r = 0.0004f32; // Needle-fine central emission core at pole
    let core_tip_r = 0.065f32;
    for ring in 0..=num_rings {
        let t = (ring as f32) / (num_rings as f32);
        let y = t * length;
        let flare = t.powf(1.70);
        let r = core_base_r + flare * (core_tip_r - core_base_r);
        let alpha = (1.0 - t).powf(1.4) * 0.95;
        let color = [0.92, 0.98, 1.0, alpha];

        for seg in 0..=num_segments {
            let phi = (seg as f32) * (std::f32::consts::TAU / (num_segments as f32));
            let x = r * phi.cos();
            let z = r * phi.sin();

            positions.push([x, y, z]);
            normals.push([phi.cos(), 0.05, phi.sin()]);
            colors.push(color);
        }
    }

    for ring in 0..num_rings {
        for seg in 0..num_segments {
            let stride = (num_segments + 1) as u32;
            let i0 = core_start_idx + ring as u32 * stride + seg as u32;
            let i1 = i0 + 1;
            let i2 = core_start_idx + (ring + 1) as u32 * stride + seg as u32;
            let i3 = i2 + 1;

            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]); // Double-sided
        }
    }

    // 3. Radial fan-ray striations (8 intersecting god-ray ribbon planes fanning out from axis)
    let num_rays = 8;
    for ray in 0..num_rays {
        let phi = (ray as f32) * (std::f32::consts::PI / (num_rays as f32));
        let cos_phi = phi.cos();
        let sin_phi = phi.sin();

        for seg in 0..num_rings {
            let t0 = (seg as f32) / (num_rings as f32);
            let t1 = ((seg + 1) as f32) / (num_rings as f32);
            let y0 = t0 * length;
            let y1 = t1 * length;
            let flare0 = t0.powf(1.70);
            let flare1 = t1.powf(1.70);
            let r0 = base_r + flare0 * (tip_r - base_r);
            let r1 = base_r + flare1 * (tip_r - base_r);

            let a0 = (1.0 - t0).powi(2) * 0.60;
            let a1 = (1.0 - t1).powi(2) * 0.60;

            let idx = positions.len() as u32;

            positions.push([-r0 * cos_phi, y0, -r0 * sin_phi]);
            normals.push([-sin_phi, 0.0, cos_phi]);
            colors.push([0.55, 0.85, 1.0, a0 * 0.35]);

            positions.push([r0 * cos_phi, y0, r0 * sin_phi]);
            normals.push([-sin_phi, 0.0, cos_phi]);
            colors.push([0.55, 0.85, 1.0, a0 * 0.35]);

            positions.push([r1 * cos_phi, y1, r1 * sin_phi]);
            normals.push([-sin_phi, 0.0, cos_phi]);
            colors.push([0.55, 0.85, 1.0, a1 * 0.35]);

            positions.push([-r1 * cos_phi, y1, -r1 * sin_phi]);
            normals.push([-sin_phi, 0.0, cos_phi]);
            colors.push([0.55, 0.85, 1.0, a1 * 0.35]);

            indices.extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
            indices.extend_from_slice(&[idx, idx + 2, idx + 1, idx, idx + 3, idx + 2]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Procedurally generates a thin, glowing flat annular equatorial ring mesh for the Magnetar,
/// with vertex colors fading from incandescent white-cyan at the inner rim to translucent violet.
fn generate_magnetar_ring_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let inner_r = 0.16f32;
    let outer_r = 0.60f32;
    let num_rings = 8;
    let num_segments = 48;

    for ring in 0..=num_rings {
        let t = (ring as f32) / (num_rings as f32);
        let r = inner_r + t * (outer_r - inner_r);

        let color = if t < 0.20 {
            let u = t / 0.20;
            [1.0, 1.0, 1.0, 0.95 - u * 0.20]
        } else if t < 0.60 {
            let u = (t - 0.20) / 0.40;
            [0.35 + 0.35 * u, 0.85 - 0.50 * u, 1.0, 0.75 - u * 0.35]
        } else {
            let u = (t - 0.60) / 0.40;
            [
                0.70 + 0.25 * u,
                0.35 - 0.20 * u,
                1.0 - 0.25 * u,
                0.40 * (1.0 - u),
            ]
        };

        for seg in 0..=num_segments {
            let phi = (seg as f32) * (std::f32::consts::TAU / (num_segments as f32));
            let x = r * phi.cos();
            let z = r * phi.sin();

            positions.push([x, 0.0, z]);
            normals.push([0.0, 1.0, 0.0]);
            colors.push(color);
        }
    }

    for ring in 0..num_rings {
        for seg in 0..num_segments {
            let stride = (num_segments + 1) as u32;
            let i0 = ring as u32 * stride + seg as u32;
            let i1 = i0 + 1;
            let i2 = (ring + 1) as u32 * stride + seg as u32;
            let i3 = i2 + 1;

            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]); // Double-sided
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Procedurally generates a 3D mesh containing 24 towering magnetic dipole flux ribbons
/// looping up to 5.5 AU into space, with cross-ribbon profiles for omnidirectional visibility
/// and vertex colors transitioning from electric cyan near the poles to royal violet and magenta at the apex.
fn generate_magnetar_field_loops_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let tiers: [(f32, usize, [f32; 4]); 3] = [
        (0.55, 8, [0.15, 0.92, 1.0, 0.85]), // Electric cyan inner loops
        (1.85, 8, [0.12, 0.68, 1.0, 0.80]), // Neon azure / cerulean mid arches
        (4.60, 8, [0.22, 0.58, 1.0, 0.75]), // Deep electric blue space-spanning arches
    ];

    let num_pts = 32;

    for &(r_0, count, apex_color) in &tiers {
        for k in 0..count {
            let base_phi = (k as f32) * (std::f32::consts::TAU / (count as f32));
            let twist = if k % 2 == 0 { 0.08 } else { -0.08 };
            let loop_rot = Quat::from_rotation_y(base_phi) * Quat::from_rotation_x(twist);

            // Delicate, razor-thin magnetic plasma filaments
            let half_w = (0.0035 + 0.0025 * r_0.sqrt()).max(0.0035);

            let mut pts = Vec::with_capacity(num_pts + 1);
            let mut tangents = Vec::with_capacity(num_pts + 1);
            let mut col_list = Vec::with_capacity(num_pts + 1);

            for s in 0..=num_pts {
                let t = (s as f32) / (num_pts as f32);
                let theta = 0.22 + t * (std::f32::consts::PI - 0.44);
                let sin_t = theta.sin();
                let cos_t = theta.cos();
                let r = r_0 * sin_t * sin_t;

                let x_l = r * sin_t;
                let y_l = r * cos_t;

                let dr_dtheta = 2.0 * r_0 * sin_t * cos_t;
                let tx = dr_dtheta * sin_t + r * cos_t;
                let ty = dr_dtheta * cos_t - r * sin_t;
                let t_len = (tx * tx + ty * ty).sqrt().max(1e-5);
                let tang = Vec3::new(tx / t_len, ty / t_len, 0.0);

                let local_p = Vec3::new(x_l, y_l, 0.0);
                pts.push(loop_rot * local_p);
                tangents.push(loop_rot * tang);

                let mid_factor = (t * std::f32::consts::PI).sin();
                let cyan = [0.10, 0.95, 1.0, 0.90];
                let r_c = cyan[0] + (apex_color[0] - cyan[0]) * mid_factor;
                let g_c = cyan[1] + (apex_color[1] - cyan[1]) * mid_factor;
                let b_c = cyan[2] + (apex_color[2] - cyan[2]) * mid_factor;
                let a_c = cyan[3] + (apex_color[3] - cyan[3]) * mid_factor;
                col_list.push([r_c, g_c, b_c, a_c]);
            }

            let b_vec = loop_rot * Vec3::Z;

            // Strip 1: Binormal ribbon (perpendicular to loop plane)
            let start_idx1 = positions.len() as u32;
            for s in 0..=num_pts {
                let p = pts[s];
                let c = col_list[s];
                let offset = b_vec * half_w;

                positions.push((p - offset).to_array());
                normals.push(b_vec.to_array());
                colors.push(c);

                positions.push((p + offset).to_array());
                normals.push(b_vec.to_array());
                colors.push(c);
            }

            for s in 0..num_pts {
                let i0 = start_idx1 + (s * 2) as u32;
                let i1 = i0 + 1;
                let i2 = i0 + 2;
                let i3 = i0 + 3;
                indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
                indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]); // Double-sided
            }

            // Strip 2: Normal ribbon (in loop plane perpendicular to curve)
            let start_idx2 = positions.len() as u32;
            for s in 0..=num_pts {
                let p = pts[s];
                let tang = tangents[s];
                let n_vec = tang.cross(b_vec).normalize_or_zero();
                let c = col_list[s];
                let offset = n_vec * half_w;

                positions.push((p - offset).to_array());
                normals.push(n_vec.to_array());
                colors.push(c);

                positions.push((p + offset).to_array());
                normals.push(n_vec.to_array());
                colors.push(c);
            }

            for s in 0..num_pts {
                let i0 = start_idx2 + (s * 2) as u32;
                let i1 = i0 + 1;
                let i2 = i0 + 2;
                let i3 = i0 + 3;
                indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
                indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]); // Double-sided
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn setup_visual_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    // High-resolution icospheres with smooth vertex normals for flawless spherical silhouettes
    let star_mesh = meshes.add(Sphere::new(1.0).mesh().ico(6).unwrap());
    let planet_mesh = meshes.add(Sphere::new(1.0).mesh().ico(6).unwrap());
    let particle_mesh = meshes.add(Sphere::new(1.0).mesh().ico(4).unwrap());
    let ring_mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0).build());

    // Misshapen irregular asteroid & comet archetype meshes (natural triaxial ellipsoids without cubic box artifacts)
    let asteroid_potato =
        generate_irregular_asteroid_mesh(Vec3::new(1.18, 1.05, 0.92), 0.45, false, 1.25);
    let asteroid_rubble =
        generate_irregular_asteroid_mesh(Vec3::new(1.08, 0.96, 1.04), 0.35, false, 4.80);
    let comet_bilobate =
        generate_irregular_asteroid_mesh(Vec3::new(1.22, 0.90, 0.82), 0.45, true, 8.40);

    let asteroid_potato_mesh = meshes.add(asteroid_potato);
    let asteroid_rubble_mesh = meshes.add(asteroid_rubble);
    let comet_bilobate_mesh = meshes.add(comet_bilobate);

    let beam_core_mesh = meshes.add(Cylinder::new(1.0, 1.0));
    let beam_sheath_mesh = meshes.add(Cylinder::new(1.0, 1.0));
    let accretion_disk_mesh = meshes.add(Cylinder::new(1.0, 1.0));

    let pulsar_beam_mesh = meshes.add(generate_pulsar_beam_mesh());
    let magnetar_ring_mesh = meshes.add(generate_magnetar_ring_mesh());
    let magnetar_field_loops_mesh = meshes.add(generate_magnetar_field_loops_mesh());

    commands.insert_resource(VisualAssets {
        star_mesh,
        planet_mesh,
        asteroid_potato_mesh,
        asteroid_rubble_mesh,
        comet_bilobate_mesh,
        particle_mesh,
        ring_mesh,
        beam_core_mesh,
        beam_sheath_mesh,
        accretion_disk_mesh,
        pulsar_beam_mesh,
        magnetar_ring_mesh,
        magnetar_field_loops_mesh,
    });
}

/// Returns the procedural shader subtype for active stars and degenerate remnants.
pub fn star_subtype_from_body_type(body_type: BodyType) -> f32 {
    match body_type {
        BodyType::YellowDwarf | BodyType::MainSequenceStar => 0.0,
        BodyType::RedDwarf => 1.0,
        BodyType::BrownDwarf => 2.0,
        BodyType::RedGiant | BodyType::RedSupergiant => 3.0,
        BodyType::BlueGiant | BodyType::BlueSupergiant | BodyType::Hypergiant => 4.0,
        BodyType::NeutronStar => 5.0,
        BodyType::Pulsar => 6.0,
        BodyType::Magnetar => 7.0,
        BodyType::WhiteDwarf => 8.0,
        BodyType::Protostar => 9.0,
        BodyType::WolfRayet => 10.0,
        _ => 0.0,
    }
}

/// Computes realistic astrophysical color palette for Stars and Stellar Remnants.
pub fn compute_stellar_palette(body_type: BodyType, temp_k: f64) -> Color {
    match body_type {
        BodyType::BlackHole => Color::srgb(0.01, 0.01, 0.01),
        BodyType::QuasiStar => Color::srgb(1.0, 0.10, 0.02),
        BodyType::BrownDwarf => Color::srgb(0.48, 0.12, 0.32),
        BodyType::NeutronStar => Color::srgb(0.72, 0.86, 1.0),
        BodyType::Pulsar => Color::srgb(0.65, 0.78, 1.0),
        BodyType::Magnetar => Color::srgb(0.72, 0.45, 1.0),
        BodyType::WhiteDwarf => Color::srgb(0.85, 0.92, 1.0),
        BodyType::WolfRayet => Color::srgb(0.95, 0.45, 0.85),
        BodyType::BlueGiant | BodyType::BlueSupergiant | BodyType::Hypergiant => {
            Color::srgb(0.68, 0.82, 1.0)
        }
        BodyType::RedGiant | BodyType::RedSupergiant => Color::srgb(0.95, 0.22, 0.04),
        BodyType::RedDwarf => Color::srgb(1.0, 0.42, 0.12),
        BodyType::Protostar => Color::srgb(0.95, 0.58, 0.15),
        BodyType::YellowDwarf | BodyType::MainSequenceStar => {
            let (br, bg, bb) = blackbody_to_srgb(temp_k);
            Color::srgb(br, bg, bb)
        }
        _ => {
            let (br, bg, bb) = blackbody_to_srgb(temp_k);
            Color::srgb(br, bg, bb)
        }
    }
}

/// Computes realistic astrophysical color palette for Gas Giants based on
/// mass tier (Jupiter vs Super-Jupiter vs Brown Dwarf) and equilibrium temperature.
pub fn compute_gas_giant_palette(mass_solar: f64, temp_k: f64, name: &str) -> Color {
    let mass_jup = mass_solar / crate::utils::constants::JUPITER_MASS_SOLAR;
    let lower = name.to_lowercase();

    // 1. Saturn Preset
    if lower.contains("saturn") {
        return Color::srgb(0.92, 0.82, 0.58); // Butterscotch golden-sand
    }

    // 2. Hot Jupiter (Sudarsky Class IV/V: Alkali / Silicate cloud hazes)
    if temp_k > 800.0 || lower.contains("hot jupiter") {
        return Color::srgb(0.38, 0.16, 0.10); // Fiery carbonaceous bronze/amber
    }

    // 3. Named Jupiter preset or standard 1.0 M_jup
    if (lower.contains("jupiter") && !lower.contains("super") && !lower.contains("hot"))
        || (0.7..=1.8).contains(&mass_jup)
    {
        return Color::srgb(0.86, 0.65, 0.42); // Iconic Jovian amber-ochre
    }

    // 4. Super-Jupiters by mass variations:
    if mass_jup > 12.0 {
        // Brown Dwarf Transition: Incandescent plum-maroon & dark violet
        Color::srgb(0.45, 0.12, 0.32)
    } else if mass_jup > 6.0 {
        // Heavy Super-Jupiter (6-12 M_jup): Royal Plum-Purple with Midnight Navy belts
        Color::srgb(0.32, 0.20, 0.48)
    } else if mass_jup > 3.5 {
        // Massive Super-Jupiter (3.5-6 M_jup): Deep Lapis-Indigo and Sapphire-Cyan
        Color::srgb(0.16, 0.36, 0.62)
    } else if mass_jup > 1.8 {
        // Super-Jupiter (1.8-3.5 M_jup): Exotic Emerald-Teal & Aquamarine
        Color::srgb(0.18, 0.52, 0.50)
    } else if mass_jup < 0.6 {
        // Sub-Saturn / Warm Gas Dwarf: Pale Cream-Straw
        Color::srgb(0.85, 0.78, 0.56)
    } else {
        // Standard Jupiter size: Classic Jovian ochre-amber
        Color::srgb(0.86, 0.65, 0.42)
    }
}

/// Spawns visual meshes and point lights for newly created celestial bodies.
pub fn spawn_missing_visuals(
    mut commands: Commands,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    unspawned_query: Query<
        (
            Entity,
            &SimPosition,
            &Mass,
            &Radius,
            &Temperature,
            &Composition,
            &CelestialBody,
            Option<&CentralStar>,
        ),
        Without<VisualBody>,
    >,
) {
    for (entity, pos, mass, radius, temp, comp, body, is_star) in unspawned_query.iter() {
        let (br, bg, bb) = blackbody_to_srgb(temp.0);
        let (cr, cg, cb) = comp.visual_color_tint();

        let is_star_like = is_star.is_some() || body.body_type.is_star_or_remnant();
        let is_gas_giant =
            body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.30;
        let base_color = if is_star_like {
            compute_stellar_palette(body.body_type, temp.0)
        } else if is_gas_giant {
            compute_gas_giant_palette(mass.0, temp.0, &body.name)
        } else {
            Color::srgb(
                (br * 0.25 + cr * 0.75).clamp(0.1, 1.0),
                (bg * 0.25 + cg * 0.75).clamp(0.1, 1.0),
                (bb * 0.25 + cb * 0.75).clamp(0.1, 1.0),
            )
        };

        let trans = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);

        let Ok(mut entity_cmd) = commands.get_entity(entity) else {
            continue;
        };

        let visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);

        if is_star_like {
            // Central Star & Stellar Remnants: Emissive unlit glow + point light
            let (p_type, unlit_flag, emissive_val) = match body.body_type {
                BodyType::BlackHole => (5u32, false, LinearRgba::BLACK),
                BodyType::QuasiStar => (
                    7u32,
                    true,
                    LinearRgba::from(Color::srgb(1.0, 0.10, 0.02)) * 32.0,
                ),
                BodyType::WhiteDwarf => (0u32, true, LinearRgba::from(base_color) * 35.0),
                BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => {
                    (0u32, true, LinearRgba::from(base_color) * 45.0)
                }
                BodyType::Protostar => (0u32, true, LinearRgba::from(base_color) * 16.0),
                _ => (0u32, true, LinearRgba::from(base_color) * 28.0),
            };

            let star_subtype = star_subtype_from_body_type(body.body_type);
            let material = materials.add(PlanetMaterial {
                base: StandardMaterial {
                    base_color,
                    emissive: emissive_val,
                    unlit: unlit_flag,
                    ..default()
                },
                extension: PlanetMaterialExtension {
                    uniforms: PlanetUniforms {
                        planet_type: p_type,
                        temperature: temp.0 as f32,
                        time: 0.0,
                        spin_rate: 0.15,
                        composition: Vec4::new(star_subtype, 20.0, 0.5, 1.0),
                        color_seed: LinearRgba::from(base_color).to_vec4(),
                        climate_and_bio: Vec4::ZERO,
                        atmosphere_params: Vec4::new(0.60, 0.85, 0.08, 0.0),
                        dynamics_and_mag: Vec4::new(100.0, 1.0, mass.0 as f32, 0.0),
                    },
                },
            });

            entity_cmd
                .try_insert((
                    VisualBody,
                    Mesh3d(visual_assets.star_mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(trans).with_scale(Vec3::splat(visual_radius)),
                    Visibility::default(),
                ))
                .with_children(|parent| {
                    // Omnidirectional solar illumination
                    parent.spawn((
                        PointLight {
                            color: base_color,
                            intensity: if body.body_type == BodyType::BlackHole {
                                1_500_000.0
                            } else if body.body_type == BodyType::QuasiStar {
                                15_000_000.0
                            } else {
                                2_500_000.0
                            },
                            range: 500.0,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::IDENTITY,
                    ));
                });
        } else {
            // Planets / Protoplanets / Planetesimals: Composition-tailored PBR materials
            let norm_comp = comp.normalized();
            let (metallic, roughness) = if norm_comp.metal_frac > 0.4 {
                (0.85, 0.25)
            } else if norm_comp.ice_frac > 0.4 {
                (0.05, 0.18)
            } else if norm_comp.gas_frac > 0.5 {
                (0.0, 0.85)
            } else {
                (0.15, 0.75)
            };

            let emissive = if temp.0 > 600.0 {
                LinearRgba::from(base_color) * ((temp.0 as f32 - 600.0) / 600.0).clamp(0.0, 5.0)
            } else {
                LinearRgba::BLACK
            };

            let material = materials.add(PlanetMaterial {
                base: StandardMaterial {
                    base_color,
                    metallic,
                    perceptual_roughness: roughness,
                    emissive,
                    ..default()
                },
                extension: PlanetMaterialExtension {
                    uniforms: PlanetUniforms {
                        planet_type: match body.body_type {
                            BodyType::GasGiant => 1,
                            BodyType::IceGiant => 2,
                            BodyType::SuperEarth => 6,
                            BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
                                if norm_comp.ice_frac > 0.40 {
                                    2 // Ice Giant / Icy world
                                } else {
                                    3 // Terrestrial
                                }
                            }
                            _ => 4, // Moon / Asteroid / Comet / Planetesimal
                        },
                        temperature: temp.0 as f32,
                        time: 0.0,
                        spin_rate: 0.15,
                        composition: Vec4::new(
                            norm_comp.silicate_frac as f32 + norm_comp.organics_frac as f32,
                            norm_comp.ice_frac as f32,
                            norm_comp.metal_frac as f32,
                            norm_comp.gas_frac as f32,
                        ),
                        color_seed: LinearRgba::from(base_color).to_vec4(),
                        climate_and_bio: Vec4::ZERO,
                        atmosphere_params: Vec4::new(
                            (norm_comp.gas_frac as f32 * 2.0).max(0.01),
                            0.08,
                            norm_comp.gas_frac as f32,
                            1.0,
                        ),
                        dynamics_and_mag: Vec4::new(
                            0.0,
                            0.0,
                            (mass.0 / crate::utils::constants::JUPITER_MASS_SOLAR) as f32,
                            0.08,
                        ),
                    },
                },
            });

            let mesh_handle = match body.body_type {
                BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::SuperEarth
                | BodyType::TerrestrialPlanet
                | BodyType::Protoplanet
                | BodyType::Planetesimal
                | BodyType::Moon => visual_assets.planet_mesh.clone(),
                BodyType::Comet => visual_assets.comet_bilobate_mesh.clone(),
                BodyType::Asteroid => {
                    let hash: usize = body.name.bytes().map(|b| b as usize).sum();
                    if hash.is_multiple_of(2) {
                        visual_assets.asteroid_potato_mesh.clone()
                    } else {
                        visual_assets.asteroid_rubble_mesh.clone()
                    }
                }
                BodyType::DustGrain => visual_assets.particle_mesh.clone(),
                _ => visual_assets.planet_mesh.clone(),
            };

            entity_cmd.try_insert((
                VisualBody,
                Mesh3d(mesh_handle),
                MeshMaterial3d(material),
                Transform::from_translation(trans).with_scale(Vec3::splat(visual_radius)),
                Visibility::default(),
            ));
        }
    }
}

/// Updates 3D translations, scales, and emissive temperatures of celestial bodies each frame.
pub fn sync_celestial_transforms(
    time: Res<Time>,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    mut light_query: Query<&mut PointLight>,
    mut query: Query<(
        &SimPosition,
        &Mass,
        &Radius,
        &Temperature,
        &Composition,
        &CelestialBody,
        &mut Transform,
        &MeshMaterial3d<PlanetMaterial>,
        &mut Mesh3d,
        (
            Option<&PlanetaryClimate>,
            Option<&BiosphereState>,
            Option<&VolatileInventory>,
        ),
        (
            Option<&SpinState>,
            Option<&ElectromagneticFieldState>,
            Option<&BlackHoleStarState>,
            Option<&Children>,
        ),
    )>,
) {
    for (
        pos,
        mass,
        radius,
        temp,
        comp,
        body,
        mut transform,
        mat_handle,
        mut mesh,
        (opt_climate, opt_bio, opt_vol),
        (opt_spin, opt_em, opt_bhs, opt_children),
    ) in query.iter_mut()
    {
        transform.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);

        let is_blown_out = opt_bhs.map(|s| s.is_blown_out).unwrap_or(false);

        // Black Hole emits zero omnidirectional point light into space
        if let Some(children) = opt_children {
            for child in children.iter() {
                if let Ok(mut light) = light_query.get_mut(child) {
                    if body.body_type == BodyType::BlackHole {
                        light.intensity = 1_500_000.0;
                    } else if body.body_type == BodyType::QuasiStar {
                        light.color = Color::srgb(1.0, 0.10, 0.02);
                        light.intensity = if is_blown_out {
                            1_500_000.0
                        } else {
                            12_000_000.0
                        };
                    } else if body.body_type == BodyType::Pulsar {
                        // Pulsar Lighthouse Strobe: Rapid periodic flashes illuminating the system as the tilted beam sweeps across!
                        let strobe = (time.elapsed_secs() * 24.0).sin().abs().powi(4);
                        light.color = Color::srgb(0.70, 0.90, 1.0);
                        light.intensity = 3_000_000.0 + strobe * 25_000_000.0;
                    } else if body.body_type == BodyType::Magnetar {
                        // Magnetar Reconnection Flares: Intense magnetic reconnection burst flickers
                        let flare = ((time.elapsed_secs() * 5.0).sin()
                            * (time.elapsed_secs() * 13.0).cos())
                        .abs()
                        .powf(1.8);
                        light.color = Color::srgb(0.85, 0.60, 1.0);
                        light.intensity = 4_000_000.0 + flare * 30_000_000.0;
                    }
                }
            }
        }

        // Unified visual scaling with minor body downscaling for realistic belt proportions
        let visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
        transform.scale = Vec3::splat(visual_radius);

        // Sync Mesh Level of Detail (LOD) based on Body Type
        let target_mesh = if body.body_type.is_star_or_remnant() {
            visual_assets.star_mesh.clone()
        } else {
            match body.body_type {
                BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::SuperEarth
                | BodyType::TerrestrialPlanet
                | BodyType::Protoplanet
                | BodyType::Planetesimal
                | BodyType::Moon => visual_assets.planet_mesh.clone(),
                BodyType::Comet => visual_assets.comet_bilobate_mesh.clone(),
                BodyType::Asteroid => {
                    let hash: usize = body.name.bytes().map(|b| b as usize).sum();
                    if hash.is_multiple_of(2) {
                        visual_assets.asteroid_potato_mesh.clone()
                    } else {
                        visual_assets.asteroid_rubble_mesh.clone()
                    }
                }
                BodyType::DustGrain => visual_assets.particle_mesh.clone(),
                _ => visual_assets.planet_mesh.clone(),
            }
        };
        if mesh.0 != target_mesh {
            mesh.0 = target_mesh;
        }

        // Update material emissive color and PBR properties dynamically
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            let (br, bg, bb) = blackbody_to_srgb(temp.0);
            let (cr, cg, cb) = comp.visual_color_tint();

            let is_star_like = body.body_type.is_star_or_remnant();
            let is_gas_giant =
                body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.30;
            let color = if is_star_like {
                compute_stellar_palette(body.body_type, temp.0)
            } else if is_gas_giant {
                compute_gas_giant_palette(mass.0, temp.0, &body.name)
            } else {
                Color::srgb(
                    (br * 0.25 + cr * 0.75).clamp(0.1, 1.0),
                    (bg * 0.25 + cg * 0.75).clamp(0.1, 1.0),
                    (bb * 0.25 + cb * 0.75).clamp(0.1, 1.0),
                )
            };
            let norm_comp = comp.normalized();
            let ocean_frac = opt_vol
                .map(|v| v.ocean_coverage_frac)
                .unwrap_or(norm_comp.ice_frac as f32);
            let ice_frac = opt_climate.map(|c| c.ice_coverage_frac).unwrap_or(0.0);
            let biomass_frac = opt_bio.map(|b| b.biomass_coverage_frac).unwrap_or(0.0);
            let cloud_density = opt_climate
                .map(|c| c.cloud_coverage_frac)
                .unwrap_or(norm_comp.gas_frac as f32);

            let spin_rate = opt_spin
                .map(|s| (24.0 / s.rotation_period_hours.max(0.1)) as f32 * 0.15)
                .unwrap_or(0.15);
            let axial_tilt = opt_spin
                .map(|s| (s.axial_tilt_degrees as f32).to_radians())
                .unwrap_or(0.08);

            let pressure_bar = opt_vol
                .map(|v| v.atmospheric_pressure_bar)
                .unwrap_or((norm_comp.gas_frac as f32 * 2.0).max(0.01));
            let scale_height =
                (0.08f32 * (temp.0 as f32 / 288.0f32).sqrt()).clamp(0.02f32, 0.25f32);
            let haze_density = opt_climate
                .map(|c| (c.cloud_coverage_frac * 1.2).clamp(0.0, 1.0))
                .unwrap_or((norm_comp.gas_frac as f32 * 1.5).clamp(0.0, 1.0));
            let greenhouse = opt_climate.map(|c| c.greenhouse_delta_k).unwrap_or(33.0);

            let mag_gauss = opt_em.map(|e| e.magnetic_field_gauss as f32).unwrap_or(0.0);
            let lava_frac = if temp.0 > 600.0 {
                ((temp.0 as f32 - 600.0) / 900.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mass_jup = (mass.0 / crate::utils::constants::JUPITER_MASS_SOLAR) as f32;

            mat.base.base_color = color;
            mat.extension.uniforms.color_seed = LinearRgba::from(color).to_vec4();
            mat.extension.uniforms.temperature = temp.0 as f32;
            mat.extension.uniforms.time = time.elapsed_secs();
            mat.extension.uniforms.spin_rate = spin_rate;
            mat.extension.uniforms.climate_and_bio =
                Vec4::new(ocean_frac, ice_frac, biomass_frac, cloud_density);
            mat.extension.uniforms.atmosphere_params =
                Vec4::new(pressure_bar, scale_height, haze_density, greenhouse);
            mat.extension.uniforms.dynamics_and_mag =
                Vec4::new(mag_gauss, lava_frac, mass_jup, axial_tilt);

            if is_star_like {
                if body.body_type == BodyType::BlackHole
                    || (body.body_type == BodyType::QuasiStar && is_blown_out)
                {
                    // Gravitational singularity event horizon + photon ring
                    mat.extension.uniforms.planet_type = 5;
                    mat.base.unlit = false;
                    mat.base.emissive = LinearRgba::BLACK;
                } else if body.body_type == BodyType::QuasiStar {
                    // Intact Quasi-Star (Black Hole Star): Dedicated Photosphere Shader
                    mat.extension.uniforms.planet_type = 7;
                    mat.base.unlit = true;
                    mat.base.emissive = LinearRgba::from(Color::srgb(1.0, 0.10, 0.02)) * 32.0;
                    mat.extension.uniforms.color_seed = Vec4::new(1.0, 0.10, 0.02, 1.0);
                    let edd_ratio = opt_bhs.map(|s| s.eddington_ratio as f32).unwrap_or(3.5);
                    mat.extension.uniforms.dynamics_and_mag =
                        Vec4::new(mag_gauss.max(1.0e6), edd_ratio, mass.0 as f32, axial_tilt);
                    mat.extension.uniforms.atmosphere_params = Vec4::new(0.65, 1.25, 0.0, 1.0);
                    mat.extension.uniforms.composition = Vec4::new(0.0, 4.5, 1.5, 0.8);
                } else {
                    // Standard Stars: Universal Photosphere Shader with Unique Subtypes
                    mat.extension.uniforms.planet_type = 0;
                    mat.base.unlit = true;
                    let star_subtype = star_subtype_from_body_type(body.body_type);
                    let mult = match body.body_type {
                        BodyType::WhiteDwarf => 35.0,
                        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => 45.0,
                        BodyType::Protostar => 14.0,
                        _ => 30.0,
                    };
                    mat.base.emissive = LinearRgba::from(color) * mult;

                    // Tune granulation cell scale, flare intensity, and pulse frequency by star archetype
                    let (cell_scale, flare_intensity, pulse_freq) = match body.body_type {
                        BodyType::RedGiant | BodyType::RedSupergiant => (4.0, 0.35, 0.2), // colossal sluggish supergranules
                        BodyType::RedDwarf => (14.0, 1.4, 0.6), // violent convective churning with high flares
                        BodyType::BrownDwarf => (8.0, 0.2, 0.3), // banded atmospheric fissures
                        BodyType::BlueGiant | BodyType::BlueSupergiant | BodyType::Hypergiant => {
                            (22.0, 0.8, 1.5)
                        } // supersonic plasma ripples
                        BodyType::NeutronStar => (40.0, 1.5, 2.0), // relativistic micro-ripples
                        BodyType::Pulsar => (35.0, 2.2, 6.5),   // high periodic pulse frequency
                        BodyType::Magnetar => (16.0, 3.8, 1.0), // extreme magnetic fracture webs
                        BodyType::WhiteDwarf => (30.0, 0.2, 0.5), // ultra-dense micro-granulation
                        BodyType::Protostar => (10.0, 0.9, 0.4), // dust-veined convective churning
                        BodyType::WolfRayet => (18.0, 2.0, 2.5), // clumpy stellar wind knots
                        _ => (24.0, 0.5, 0.3),                  // classic solar granulation
                    };
                    mat.extension.uniforms.composition =
                        Vec4::new(star_subtype, cell_scale, flare_intensity, pulse_freq);

                    let spot_coverage = match body.body_type {
                        BodyType::RedDwarf => 0.28,
                        BodyType::YellowDwarf | BodyType::MainSequenceStar => 0.08,
                        BodyType::Protostar => 0.18,
                        _ => 0.0,
                    };
                    mat.extension.uniforms.atmosphere_params =
                        Vec4::new(0.60, 0.85, spot_coverage, 0.0);
                    mat.extension.uniforms.dynamics_and_mag =
                        Vec4::new(mag_gauss, 1.0, mass.0 as f32, axial_tilt);
                }
            } else {
                mat.base.unlit = false;
                mat.extension.uniforms.composition = Vec4::new(
                    norm_comp.silicate_frac as f32 + norm_comp.organics_frac as f32,
                    norm_comp.ice_frac as f32,
                    norm_comp.metal_frac as f32,
                    norm_comp.gas_frac as f32,
                );
                mat.extension.uniforms.planet_type = match body.body_type {
                    BodyType::GasGiant => 1,
                    BodyType::IceGiant => 2,
                    BodyType::SuperEarth => 6,
                    BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
                        if norm_comp.ice_frac > 0.40 {
                            2
                        } else {
                            3
                        }
                    }
                    _ => 4,
                };

                if comp.metal_frac > 0.4 {
                    mat.base.metallic = 0.85;
                    mat.base.perceptual_roughness = 0.25;
                } else if comp.ice_frac > 0.4 {
                    mat.base.metallic = 0.05;
                    mat.base.perceptual_roughness = 0.18;
                } else if comp.gas_frac > 0.5 {
                    mat.base.metallic = 0.0;
                    mat.base.perceptual_roughness = 0.85;
                } else {
                    mat.base.metallic = 0.15;
                    mat.base.perceptual_roughness = 0.75;
                }

                if temp.0 > 600.0 {
                    mat.base.emissive =
                        LinearRgba::from(color) * ((temp.0 as f32 - 600.0) / 600.0).clamp(0.0, 5.0);
                } else {
                    mat.base.emissive = LinearRgba::BLACK;
                }
            }
        }
    }
}

/// Marker component for an instantiated visual planetary ring entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualRingChild;

/// Synchronizes 3D planetary ring system meshes, materials, and axial tilt transforms.
pub fn sync_planetary_rings(
    mut commands: Commands,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut ring_materials: ResMut<Assets<RingMaterial>>,
    planets_with_rings_query: Query<(
        Entity,
        &PlanetaryRingSystem,
        &Radius,
        &CelestialBody,
        Option<&SpinState>,
        Option<&Children>,
    )>,
    mut ring_children_query: Query<
        (&mut Transform, &MeshMaterial3d<RingMaterial>),
        With<VisualRingChild>,
    >,
) {
    for (planet_entity, ring_sys, radius, _body, opt_spin, opt_children) in
        planets_with_rings_query.iter()
    {
        let planet_render_rad = config.calc_visual_radius(radius.0);
        let ring_outer_scale = (planet_render_rad * 2.85).max(0.015);

        let tilt_degrees = opt_spin
            .map(|s| s.axial_tilt_degrees as f32)
            .unwrap_or(26.7);
        let ring_rotation = Quat::from_rotation_z(tilt_degrees.to_radians());

        let mut found_child = false;
        if let Some(children) = opt_children {
            for child in children.iter() {
                if let Ok((mut transform, mat_handle)) = ring_children_query.get_mut(child) {
                    found_child = true;
                    transform.scale = Vec3::splat(ring_outer_scale);
                    transform.rotation = ring_rotation;

                    if let Some(mut mat) = ring_materials.get_mut(&mat_handle.0) {
                        mat.uniforms.inner_radius = ring_sys.inner_radius_au;
                        mat.uniforms.outer_radius = ring_sys.outer_radius_au;
                        mat.uniforms.optical_depth = ring_sys.optical_depth;
                        mat.uniforms.ice_fraction = ring_sys.ice_fraction;
                        mat.uniforms.ring_color = calc_ring_color(ring_sys.ice_fraction);
                    }
                }
            }
        }

        if !found_child {
            let ring_color = calc_ring_color(ring_sys.ice_fraction);
            let material = ring_materials.add(RingMaterial {
                uniforms: RingUniforms {
                    inner_radius: ring_sys.inner_radius_au,
                    outer_radius: ring_sys.outer_radius_au,
                    optical_depth: ring_sys.optical_depth,
                    ice_fraction: ring_sys.ice_fraction,
                    ring_color,
                },
            });

            if let Ok(mut p_cmd) = commands.get_entity(planet_entity) {
                p_cmd.with_children(|parent| {
                    parent.spawn((
                        VisualRingChild,
                        Mesh3d(visual_assets.ring_mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::from_scale(Vec3::splat(ring_outer_scale))
                            .with_rotation(ring_rotation),
                        NotShadowCaster,
                    ));
                });
            }
        }
    }
}

/// Dynamically calculates ring albedo and tone based on water ice vs silicate/metal composition.
fn calc_ring_color(ice_fraction: f32) -> Vec4 {
    if ice_fraction >= 0.70 {
        // High ice fraction (>= 70%): brilliant silver-white (Saturn-like)
        Vec4::new(0.96, 0.97, 1.0, 0.95)
    } else if ice_fraction >= 0.35 {
        // Mixed ice & dust (35-70%): warm sand-cream tone
        Vec4::new(0.85, 0.78, 0.68, 0.85)
    } else {
        // Silicate / carbonaceous (< 35%): dark anthracite / charcoal (Uranus / Jovian-like)
        Vec4::new(0.38, 0.35, 0.32, 0.65)
    }
}

/// Root marker for the 3D Quasar Laser Beam system.
#[derive(Component, Debug, Clone, Copy)]
pub struct QuasarBeamRoot;

/// Sub-parts of the 3D Quasar Laser Beam.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuasarBeamPart {
    NorthCore,
    NorthSheath,
    NorthLobe,
    SouthCore,
    SouthSheath,
    SouthLobe,
}

/// Synchronizes 3D volumetric laser beam columns for active Quasars / Black Hole Stars.
pub fn sync_quasar_beams(
    mut commands: Commands,
    visual_assets: Option<Res<VisualAssets>>,
    config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    quasi_query: Query<
        (
            &SimPosition,
            &CelestialBody,
            &Mass,
            &Radius,
            Option<&BlackHoleStarState>,
        ),
        Or<(With<CentralStar>, With<BlackHoleStarState>)>,
    >,
    mut root_query: Query<(Entity, &mut Transform), With<QuasarBeamRoot>>,
    mut part_query: Query<(&mut Transform, &QuasarBeamPart), Without<QuasarBeamRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    // Find any active Quasar / Quasi-Star in the simulation
    let target = quasi_query.iter().find(|(_, body, mass, _, opt_qs)| {
        opt_qs.is_some()
            || body.body_type == BodyType::QuasiStar
            || body.name.contains("Quasar")
            || (body.body_type == BodyType::BlackHole && mass.0 > 500.0)
    });

    let Some((pos, body, _mass, radius, opt_qs)) = target else {
        // No active quasar/quasi-star: despawn beam visual if present
        for (ent, _) in root_query.iter() {
            commands.entity(ent).despawn();
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let is_blown_out =
        opt_qs.map(|qs| qs.is_blown_out).unwrap_or(false) || body.name.contains("Quasar");
    let light_dist = opt_qs
        .map(|qs| qs.jet_travel_distance_au as f32)
        .unwrap_or(0.0);

    // CRITICAL: Do NOT spawn or render quasar laser beams before the cocoon has blown out!
    // The laser beams emerge ONLY after blowout, and travel outward across space at the speed of light c continuously!
    if !is_blown_out || light_dist <= 0.1 {
        for (ent, _) in root_query.iter() {
            commands.entity(ent).despawn();
        }
        return;
    }

    // Continuous propagation at the speed of light c (~63,241 AU/yr)
    let jet_len = light_dist;

    // Dynamically anchor beam start to the black hole's actual visual radius
    // as it shrinks from 60 AU down to ~2.5 AU, eliminating any floating gap!
    let current_visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
    let pole_start = (current_visual_radius * 0.90).max(0.05);
    let beam_len = (jet_len - pole_start).max(0.1);
    let beam_center = pole_start + beam_len * 0.5;

    // Slim, highly-collimated laser beam proportions
    let core_r = 0.06f32; // Razor-thin brilliant white laser filament
    let sheath_r = 0.18f32; // Subtle translucent cyan relativistic plasma sheath
    let lobe_r = 0.55f32; // Sleek bow shock cap at the light front

    if let Some((_, mut root_trans)) = root_query.iter_mut().next() {
        root_trans.translation = world_pos;
        for (mut part_trans, part) in part_query.iter_mut() {
            match part {
                QuasarBeamPart::NorthCore => {
                    part_trans.translation = Vec3::new(0.0, beam_center, 0.0);
                    part_trans.scale = Vec3::new(core_r, beam_len, core_r);
                }
                QuasarBeamPart::NorthSheath => {
                    part_trans.translation = Vec3::new(0.0, beam_center, 0.0);
                    part_trans.scale = Vec3::new(sheath_r, beam_len, sheath_r);
                }
                QuasarBeamPart::NorthLobe => {
                    part_trans.translation = Vec3::new(0.0, jet_len, 0.0);
                    part_trans.scale = Vec3::splat(lobe_r);
                }
                QuasarBeamPart::SouthCore => {
                    part_trans.translation = Vec3::new(0.0, -beam_center, 0.0);
                    part_trans.scale = Vec3::new(core_r, beam_len, core_r);
                }
                QuasarBeamPart::SouthSheath => {
                    part_trans.translation = Vec3::new(0.0, -beam_center, 0.0);
                    part_trans.scale = Vec3::new(sheath_r, beam_len, sheath_r);
                }
                QuasarBeamPart::SouthLobe => {
                    part_trans.translation = Vec3::new(0.0, -jet_len, 0.0);
                    part_trans.scale = Vec3::splat(lobe_r);
                }
            }
        }
    } else {
        // Spawn 3D beam hierarchy
        let core_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(75.0, 75.0, 90.0, 1.0),
            unlit: true,
            ..default()
        });
        let sheath_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.20, 0.75, 1.0, 0.22),
            emissive: LinearRgba::new(2.5, 8.0, 20.0, 0.30),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let lobe_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.40, 0.85, 1.0, 0.60),
            emissive: LinearRgba::new(8.0, 18.0, 35.0, 0.6),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });

        commands
            .spawn((
                QuasarBeamRoot,
                Transform::from_translation(world_pos),
                Visibility::default(),
            ))
            .with_children(|parent| {
                // North Polar Jet (Core + Sheath + Terminal Lobe)
                parent.spawn((
                    QuasarBeamPart::NorthCore,
                    Mesh3d(assets.beam_core_mesh.clone()),
                    MeshMaterial3d(core_mat.clone()),
                    Transform::from_xyz(0.0, beam_center, 0.0)
                        .with_scale(Vec3::new(core_r, beam_len, core_r)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    QuasarBeamPart::NorthSheath,
                    Mesh3d(assets.beam_sheath_mesh.clone()),
                    MeshMaterial3d(sheath_mat.clone()),
                    Transform::from_xyz(0.0, beam_center, 0.0)
                        .with_scale(Vec3::new(sheath_r, beam_len, sheath_r)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    QuasarBeamPart::NorthLobe,
                    Mesh3d(assets.star_mesh.clone()),
                    MeshMaterial3d(lobe_mat.clone()),
                    Transform::from_xyz(0.0, jet_len, 0.0).with_scale(Vec3::splat(lobe_r)),
                    NotShadowCaster,
                ));

                // South Polar Jet (Core + Sheath + Terminal Lobe)
                parent.spawn((
                    QuasarBeamPart::SouthCore,
                    Mesh3d(assets.beam_core_mesh.clone()),
                    MeshMaterial3d(core_mat.clone()),
                    Transform::from_xyz(0.0, -beam_center, 0.0)
                        .with_scale(Vec3::new(core_r, beam_len, core_r)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    QuasarBeamPart::SouthSheath,
                    Mesh3d(assets.beam_sheath_mesh.clone()),
                    MeshMaterial3d(sheath_mat.clone()),
                    Transform::from_xyz(0.0, -beam_center, 0.0)
                        .with_scale(Vec3::new(sheath_r, beam_len, sheath_r)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    QuasarBeamPart::SouthLobe,
                    Mesh3d(assets.star_mesh.clone()),
                    MeshMaterial3d(lobe_mat.clone()),
                    Transform::from_xyz(0.0, -jet_len, 0.0).with_scale(Vec3::splat(lobe_r)),
                    NotShadowCaster,
                ));
            });
    }
}

/// Root marker for the 3D Pulsar Relativistic Lighthouse Beam system.
#[derive(Component, Debug, Clone, Copy)]
pub struct PulsarBeamRoot;

/// Sub-parts of the 3D Pulsar Beam hierarchy.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulsarBeamPart {
    NorthBeam,
    SouthBeam,
}

/// Synchronizes 3D volumetric relativistic lighthouse beams for active Pulsars.
pub fn sync_pulsar_beams(
    mut commands: Commands,
    time: Res<Time>,
    visual_assets: Option<Res<VisualAssets>>,
    config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pulsar_query: Query<
        (&SimPosition, &CelestialBody, &Radius),
        Or<(With<CentralStar>, With<CelestialBody>)>,
    >,
    mut root_query: Query<(Entity, &mut Transform), With<PulsarBeamRoot>>,
    mut part_query: Query<(&mut Transform, &PulsarBeamPart), Without<PulsarBeamRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    // Find active Pulsar in the simulation
    let target = pulsar_query
        .iter()
        .find(|(_, body, _)| body.body_type == BodyType::Pulsar || body.name.contains("Pulsar"));

    let Some((pos, body, radius)) = target else {
        // No active pulsar: despawn beam visual if present
        for (ent, _) in root_query.iter() {
            commands.entity(ent).despawn();
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let elapsed = time.elapsed_secs();

    // Fast rotation ~4 Hz (24 rad/s) with ~22-degree magnetic tilt
    let spin_rate = 24.0;
    let beam_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.38);

    let current_visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
    let pole_start = (current_visual_radius * 0.90).max(0.002);

    if let Some((_, mut root_trans)) = root_query.iter_mut().next() {
        root_trans.translation = world_pos;
        root_trans.rotation = beam_rot;

        for (mut part_trans, part) in part_query.iter_mut() {
            match part {
                PulsarBeamPart::NorthBeam => {
                    part_trans.translation = Vec3::new(0.0, pole_start, 0.0);
                }
                PulsarBeamPart::SouthBeam => {
                    part_trans.translation = Vec3::new(0.0, -pole_start, 0.0);
                    part_trans.rotation = Quat::from_rotation_x(std::f32::consts::PI);
                }
            }
        }
    } else {
        // Spawn 3D beam hierarchy with additive translucent glow and internal fan-ray striations
        let beam_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.85, 0.95, 1.0, 0.70),
            emissive: LinearRgba::new(55.0, 70.0, 95.0, 0.70),
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            unlit: true,
            ..default()
        });

        commands
            .spawn((
                PulsarBeamRoot,
                Transform::from_translation(world_pos).with_rotation(beam_rot),
                Visibility::default(),
            ))
            .with_children(|parent| {
                // North Polar Lighthouse Beam (Conical God-Ray Frustum with Fan Rays)
                parent.spawn((
                    PulsarBeamPart::NorthBeam,
                    Mesh3d(assets.pulsar_beam_mesh.clone()),
                    MeshMaterial3d(beam_mat.clone()),
                    Transform::from_xyz(0.0, pole_start, 0.0),
                    NotShadowCaster,
                ));

                // South Polar Lighthouse Beam
                parent.spawn((
                    PulsarBeamPart::SouthBeam,
                    Mesh3d(assets.pulsar_beam_mesh.clone()),
                    MeshMaterial3d(beam_mat.clone()),
                    Transform::from_xyz(0.0, -pole_start, 0.0)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
                    NotShadowCaster,
                ));
            });
    }
}

/// Root marker for the 3D Magnetar Magnetic Arches and Equatorial Ring system.
#[derive(Component, Debug, Clone, Copy)]
pub struct MagnetarStructureRoot;

/// Sub-parts of the 3D Magnetar Structure hierarchy.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagnetarStructurePart {
    EquatorialRing,
    MagneticFieldLoops,
}

/// Synchronizes 3D volumetric magnetic field loops and incandescent equatorial ring for active Magnetars.
pub fn sync_magnetar_structures(
    mut commands: Commands,
    time: Res<Time>,
    visual_assets: Option<Res<VisualAssets>>,
    _config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    magnetar_query: Query<
        (
            &SimPosition,
            &CelestialBody,
            Option<&ElectromagneticFieldState>,
        ),
        Or<(With<CentralStar>, With<CelestialBody>)>,
    >,
    mut root_query: Query<(Entity, &mut Transform), With<MagnetarStructureRoot>>,
    _part_query: Query<(&mut Transform, &MagnetarStructurePart), Without<MagnetarStructureRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    // Find active Magnetar in the simulation strictly (prioritizing BodyType::Magnetar,
    // then extreme EM field >= 10^14 G, then explicit 'magnetar' name without ejecta/clump words).
    // NEVER match 'SGR' alone because cluster clumps/ejecta also share the SGR catalogue prefix.
    let target = magnetar_query
        .iter()
        .find(|(_, body, _)| body.body_type == BodyType::Magnetar)
        .or_else(|| {
            magnetar_query.iter().find(|(_, _, opt_em)| {
                opt_em.is_some_and(|em| em.magnetic_field_gauss >= 1.0e14)
            })
        })
        .or_else(|| {
            magnetar_query.iter().find(|(_, body, _)| {
                body.name.to_lowercase().contains("magnetar")
                    && !body.name.contains("Clump")
                    && !body.name.contains("Ejecta")
            })
        });

    let Some((pos, _body, _)) = target else {
        // No active magnetar: despawn visuals if present
        for (ent, _) in root_query.iter() {
            commands.entity(ent).despawn();
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let elapsed = time.elapsed_secs();

    // Majestic rotation ~1.2 rad/s with 15-degree magnetic inclination tilt
    let spin_rate = 1.2;
    let mag_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.26);

    if let Some((_, mut root_trans)) = root_query.iter_mut().next() {
        root_trans.translation = world_pos;
        root_trans.rotation = mag_rot;
    } else {
        // Spawn 3D magnetar structure hierarchy
        let ring_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(20.0, 45.0, 75.0, 0.85),
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            unlit: true,
            ..default()
        });

        let loops_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(6.0, 22.0, 42.0, 0.85),
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            unlit: true,
            ..default()
        });

        commands
            .spawn((
                MagnetarStructureRoot,
                Transform::from_translation(world_pos).with_rotation(mag_rot),
                Visibility::default(),
            ))
            .with_children(|parent| {
                // Incandescent Thin Flat Equatorial Ring (Saturn-like plasma annulus)
                parent.spawn((
                    MagnetarStructurePart::EquatorialRing,
                    Mesh3d(assets.magnetar_ring_mesh.clone()),
                    MeshMaterial3d(ring_mat.clone()),
                    Transform::IDENTITY,
                    NotShadowCaster,
                ));

                // Towering 3D Magnetic Field Line Ribbon Arches (spanning out to ~5.5 AU)
                parent.spawn((
                    MagnetarStructurePart::MagneticFieldLoops,
                    Mesh3d(assets.magnetar_field_loops_mesh.clone()),
                    MeshMaterial3d(loops_mat.clone()),
                    Transform::IDENTITY,
                    NotShadowCaster,
                ));
            });
    }
}
