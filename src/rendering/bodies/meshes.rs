use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use std::f32::consts::PI;

use super::VisualAssets;

/// Recomputes smooth vertex normals for procedural meshes.
pub fn recompute_mesh_normals(mesh: &mut Mesh) {
    if let (Some(VertexAttributeValues::Float32x3(pos)), Some(Indices::U32(indices))) =
        (mesh.attribute(Mesh::ATTRIBUTE_POSITION), mesh.indices())
    {
        let mut normals = vec![Vec3::ZERO; pos.len()];
        for chunk in indices.chunks(3) {
            if let [c0, c1, c2] = chunk {
                let i0 = *c0 as usize;
                let i1 = *c1 as usize;
                let i2 = *c2 as usize;
                if let (Some(&p0), Some(&p1), Some(&p2)) = (pos.get(i0), pos.get(i1), pos.get(i2)) {
                    let v0 = Vec3::from_array(p0);
                    let v1 = Vec3::from_array(p1);
                    let v2 = Vec3::from_array(p2);
                    let normal = (v1 - v0).cross(v2 - v0);
                    if let Some(n) = normals.get_mut(i0) {
                        *n += normal;
                    }
                    if let Some(n) = normals.get_mut(i1) {
                        *n += normal;
                    }
                    if let Some(n) = normals.get_mut(i2) {
                        *n += normal;
                    }
                }
            }
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
pub fn generate_irregular_asteroid_mesh(
    elongation: Vec3,
    noise_strength: f32,
    is_bilobate: bool,
    seed: f32,
) -> Mesh {
    let mut sphere = Sphere::new(1.0)
        .mesh()
        .ico(4)
        .unwrap_or_else(|_| Sphere::new(1.0).mesh().uv(16, 10));
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

        let n1 = (v.x * 2.1 + seed).sin() * (v.y * 2.3).cos() * (v.z * 1.9 + seed * 0.5).sin();
        let n2 = (v.x * 4.7 - seed).cos() * (v.y * 5.1 + seed).sin() * (v.z * 4.3).cos();
        let n3 = (v.x * 9.3).sin() * (v.y * 8.7 - seed).cos() * (v.z * 9.8 + seed).sin();
        let noise = n1 * 0.55 + n2 * 0.30 + n3 * 0.15;

        let mut r = 1.0 + noise * noise_strength;

        if is_bilobate {
            let neck_pinch = (v.z * PI).cos().abs();
            let waist = 1.0 - 0.45 * (1.0 - neck_pinch).powf(2.0);
            r *= waist;
        }

        for (c_dir, c_rad, c_depth) in craters {
            let angle = v.dot(c_dir).clamp(-1.0, 1.0).acos();
            if angle < c_rad {
                let norm_dist = angle / c_rad;
                let bowl = (1.0 - norm_dist * norm_dist).powf(1.5);
                let rim = if norm_dist > 0.75 {
                    ((norm_dist - 0.75) / 0.25 * PI).sin() * 0.05
                } else {
                    0.0
                };
                r -= bowl * c_depth;
                r += rim;
            }
        }

        let shaped = v * elongation * r;
        new_positions.push(shaped.to_array());
    }

    sphere.insert_attribute(Mesh::ATTRIBUTE_POSITION, new_positions);
    recompute_mesh_normals(&mut sphere);
    sphere
}

/// Procedurally generates a volumetric conical lighthouse beam mesh for the Pulsar.
pub fn generate_pulsar_beam_mesh() -> Mesh {
    let rings = 32;
    let sectors = 48;
    let base_radius = 0.0004;
    let max_radius = 6.5;
    let total_length = 50.0;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for i in 0..=rings {
        let t = i as f32 / rings as f32;
        let y = t * total_length;
        let r = base_radius + (max_radius - base_radius) * t.powf(1.10);

        let axial_fade = if t < 0.04 {
            0.65 + (t / 0.04) * 0.35
        } else {
            ((1.0 - t) / 0.96).powf(1.4)
        };

        for j in 0..=sectors {
            let u = j as f32 / sectors as f32;
            let theta = u * 2.0 * PI;

            let x = theta.cos() * r;
            let z = theta.sin() * r;

            positions.push([x, y, z]);
            normals.push([theta.cos(), 0.1, theta.sin()]);
            uvs.push([u, t]);

            let ray_striation = (theta * 8.0).cos().powi(2) * 0.4 + 0.6;
            let alpha = (axial_fade * ray_striation * 0.75).clamp(0.0, 1.0);
            colors.push([1.0, 1.0, 1.0, alpha]);
        }
    }

    for i in 0..rings {
        for j in 0..sectors {
            let row1 = (i * (sectors + 1) + j) as u32;
            let row2 = ((i + 1) * (sectors + 1) + j) as u32;

            indices.push(row1);
            indices.push(row2);
            indices.push(row1 + 1);

            indices.push(row1 + 1);
            indices.push(row2);
            indices.push(row2 + 1);
        }
    }

    let core_steps = 32;
    let core_subdivisions = 16;
    for i in 0..=core_steps {
        let t = i as f32 / core_steps as f32;
        let y = t * total_length * 0.85;
        let r = 0.00015 + 0.14 * t;
        let alpha = (1.0 - t).powi(2) * 0.95;

        for j in 0..=core_subdivisions {
            let u = j as f32 / core_subdivisions as f32;
            let theta = u * 2.0 * PI;
            let x = theta.cos() * r;
            let z = theta.sin() * r;

            positions.push([x, y, z]);
            normals.push([theta.cos(), 0.0, theta.sin()]);
            uvs.push([u, t]);
            colors.push([1.0, 1.0, 1.0, alpha]);
        }
    }

    let core_offset = ((rings + 1) * (sectors + 1)) as u32;
    for i in 0..core_steps {
        for j in 0..core_subdivisions {
            let row1 = core_offset + (i * (core_subdivisions + 1) + j) as u32;
            let row2 = core_offset + ((i + 1) * (core_subdivisions + 1) + j) as u32;

            indices.push(row1);
            indices.push(row2);
            indices.push(row1 + 1);

            indices.push(row1 + 1);
            indices.push(row2);
            indices.push(row2 + 1);
        }
    }

    let fan_offset = positions.len() as u32;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.5]);
    colors.push([1.0, 1.0, 1.0, 0.9]);

    for j in 0..sectors {
        let theta = (j as f32 / sectors as f32) * 2.0 * PI;
        let p_end = [
            theta.cos() * max_radius * 1.05,
            total_length,
            theta.sin() * max_radius * 1.05,
        ];
        positions.push(p_end);
        normals.push([theta.cos(), 0.2, theta.sin()]);
        uvs.push([j as f32 / sectors as f32, 1.0]);
        colors.push([1.0, 1.0, 1.0, 0.15]);
    }

    for j in 0..sectors {
        let v1 = fan_offset;
        let v2 = fan_offset + 1 + j as u32;
        let v3 = fan_offset + 1 + ((j + 1) % sectors) as u32;
        indices.push(v1);
        indices.push(v2);
        indices.push(v3);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Procedurally generates a razor-thin equatorial plasma ring mesh for active Magnetars.
pub fn generate_magnetar_ring_mesh() -> Mesh {
    let r_inner = 0.045f32;
    let r_outer = 0.420f32;
    let segments = 64;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((segments + 1) * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity((segments + 1) * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity((segments + 1) * 2);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity((segments + 1) * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(segments * 6);

    for i in 0..=segments {
        let theta = (i as f32 / segments as f32) * 2.0 * PI;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        positions.push([cos_t * r_inner, 0.0, sin_t * r_inner]);
        positions.push([cos_t * r_outer, 0.0, sin_t * r_outer]);

        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);

        uvs.push([0.0, i as f32 / segments as f32]);
        uvs.push([1.0, i as f32 / segments as f32]);

        colors.push([1.0, 1.0, 1.0, 0.95]);
        colors.push([1.0, 1.0, 1.0, 0.10]);

        if i < segments {
            let base = (i * 2) as u32;
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);

            indices.push(base + 1);
            indices.push(base + 3);
            indices.push(base + 2);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Procedurally generates a 3D magnetic field loop mesh (poloidal magnetic flux tube bundle).
/// Constructed as an axisymmetric dipole field with multiple concentric radial L-shell tiers
/// evenly distributed around all azimuths, ensuring balanced magnetic symmetry in all directions.
pub fn generate_magnetar_field_loops_mesh() -> Mesh {
    let tiers: [f32; 4] = [1.2, 2.4, 3.8, 5.5];
    let num_azimuths = 8;
    let points_per_loop = 64;
    let ribbon_width = 0.012;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for (tier_idx, &l_max) in tiers.iter().enumerate() {
        let tier_fraction = tier_idx as f32 / (tiers.len() - 1) as f32;
        let loop_fade = (1.0 - tier_fraction * 0.45).clamp(0.4, 1.0);

        for a in 0..num_azimuths {
            let phi = (a as f32 / num_azimuths as f32) * 2.0 * PI;
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();

            let loop_start_idx = positions.len() as u32;

            for p in 0..=points_per_loop {
                let t = p as f32 / points_per_loop as f32;
                let theta = t * PI;

                let sin_t = theta.sin();
                let cos_t = theta.cos();

                let r = (l_max * sin_t.powi(2)).max(0.015);
                let y = r * cos_t;
                let rho = r * sin_t;

                let x = rho * cos_phi;
                let z = rho * sin_phi;

                let binormal_x = -sin_phi * ribbon_width;
                let binormal_z = cos_phi * ribbon_width;

                positions.push([x - binormal_x, y, z - binormal_z]);
                positions.push([x + binormal_x, y, z + binormal_z]);

                normals.push([cos_phi * sin_t, cos_t, sin_phi * sin_t]);
                normals.push([cos_phi * sin_t, cos_t, sin_phi * sin_t]);

                uvs.push([0.0, t]);
                uvs.push([1.0, t]);

                let footpoint_bright = if t < 0.15 {
                    1.0 + (1.0 - t / 0.15) * 2.5
                } else if t > 0.85 {
                    1.0 + ((t - 0.85) / 0.15) * 2.5
                } else {
                    1.0
                };

                let alpha = (0.75 * footpoint_bright * loop_fade).clamp(0.05, 1.0);
                colors.push([1.0, 1.0, 1.0, alpha]);
                colors.push([1.0, 1.0, 1.0, alpha]);

                if p < points_per_loop {
                    let base = loop_start_idx + (p * 2) as u32;
                    indices.push(base);
                    indices.push(base + 1);
                    indices.push(base + 2);

                    indices.push(base + 1);
                    indices.push(base + 3);
                    indices.push(base + 2);
                }
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Initializes shared visual assets and caches procedural geometries.
pub fn setup_visual_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let star_mesh = meshes.add(
        Sphere::new(1.0)
            .mesh()
            .ico(6)
            .unwrap_or_else(|_| Sphere::new(1.0).mesh().uv(64, 48)),
    );
    let planet_mesh = meshes.add(
        Sphere::new(1.0)
            .mesh()
            .ico(6)
            .unwrap_or_else(|_| Sphere::new(1.0).mesh().uv(48, 36)),
    );
    let particle_mesh = meshes.add(
        Sphere::new(1.0)
            .mesh()
            .ico(1)
            .unwrap_or_else(|_| Sphere::new(1.0).mesh().uv(6, 4)),
    );
    let asteroid_potato = meshes.add(generate_irregular_asteroid_mesh(
        Vec3::new(1.45, 0.95, 0.75),
        0.28,
        false,
        1.337,
    ));
    let asteroid_rubble = meshes.add(generate_irregular_asteroid_mesh(
        Vec3::new(1.15, 1.10, 0.85),
        0.35,
        false,
        4.242,
    ));
    let comet_bilobate = meshes.add(generate_irregular_asteroid_mesh(
        Vec3::new(1.85, 0.80, 0.70),
        0.22,
        true,
        7.777,
    ));
    let ring_mesh = meshes.add(Mesh::from(Plane3d::default().mesh().size(1.0, 1.0)));
    let beam_core_mesh = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16));
    let beam_sheath_mesh = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(24));
    let accretion_disk_mesh = meshes.add(Mesh::from(Plane3d::default().mesh().size(1.0, 1.0)));
    let pulsar_beam_mesh = meshes.add(generate_pulsar_beam_mesh());
    let magnetar_ring_mesh = meshes.add(generate_magnetar_ring_mesh());
    let magnetar_field_loops_mesh = meshes.add(generate_magnetar_field_loops_mesh());

    commands.insert_resource(VisualAssets {
        star_mesh,
        planet_mesh,
        asteroid_potato_mesh: asteroid_potato,
        asteroid_rubble_mesh: asteroid_rubble,
        comet_bilobate_mesh: comet_bilobate,
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
