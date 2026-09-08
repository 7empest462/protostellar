use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use rayon::prelude::*;

use crate::simulation::resources::DiskParameters;

use super::ParticleSwarmData;

pub fn update_billboard_mesh_quads(
    data: &mut ParticleSwarmData,
    cam_right: Vec3,
    cam_up: Vec3,
    disk_params: &DiskParameters,
    p_render_scale: f32,
) {
    let scale_mult = (disk_params.outer_radius_au / 45.0).clamp(0.02, 1.0) as f32;
    let base_render_r = 0.080 * p_render_scale * scale_mult;
    let n = data.count;
    let b_mass = data.base_mass;

    let masses = &data.masses;
    let positions = &data.positions;
    let colors = &data.colors;

    data.mesh_positions
        .par_chunks_mut(4 * 2048)
        .zip(data.mesh_colors.par_chunks_mut(4 * 2048))
        .enumerate()
        .for_each(|(chunk_idx, (pos_out, col_out))| {
            let start_particle = chunk_idx * 2048;
            let end_particle = (start_particle + 2048).min(n);

            let particle_count = end_particle.saturating_sub(start_particle);
            for (offset, (quad_pos, quad_col)) in pos_out
                .chunks_exact_mut(4)
                .zip(col_out.chunks_exact_mut(4))
                .take(particle_count)
                .enumerate()
            {
                let local_i = start_particle + offset;
                let Some(&m) = masses.get(local_i) else {
                    continue;
                };

                if let ([p0, p1, p2, p3], [c0, c1, c2, c3]) = (quad_pos, quad_col) {
                    if m <= 0.0 {
                        *p0 = [0.0, -5000.0, 0.0];
                        *p1 = [0.0, -5000.0, 0.0];
                        *p2 = [0.0, -5000.0, 0.0];
                        *p3 = [0.0, -5000.0, 0.0];
                        *c0 = [0.0, 0.0, 0.0, 0.0];
                        *c1 = [0.0, 0.0, 0.0, 0.0];
                        *c2 = [0.0, 0.0, 0.0, 0.0];
                        *c3 = [0.0, 0.0, 0.0, 0.0];
                        continue;
                    }

                    let Some(&pos_val) = positions.get(local_i) else {
                        continue;
                    };
                    let p = Vec3::from_array(pos_val);
                    let m_factor = (m / b_mass).cbrt().clamp(1.0, 8.0);
                    let r_v = base_render_r * m_factor;

                    let right = cam_right * r_v;
                    let up = cam_up * r_v;

                    *p0 = (p - right - up).to_array();
                    *p1 = (p + right - up).to_array();
                    *p2 = (p + right + up).to_array();
                    *p3 = (p - right + up).to_array();

                    let alpha = if m <= 1.1 * b_mass {
                        0.65f32
                    } else if m <= 3.0 * b_mass {
                        0.82f32
                    } else if m <= 8.0 * b_mass {
                        0.95f32
                    } else {
                        1.0f32
                    };

                    let c = colors.get(local_i).copied().unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let final_col = [c[0], c[1], c[2], alpha];
                    *c0 = final_col;
                    *c1 = final_col;
                    *c2 = final_col;
                    *c3 = final_col;
                }
            }
        });
}

pub fn sync_mesh_attributes(data: &ParticleSwarmData, meshes: &mut Assets<Mesh>) {
    if let Some(mut mesh) = meshes.get_mut(&data.mesh_handle) {
        if let Some(VertexAttributeValues::Float32x3(mesh_pos)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            mesh_pos.copy_from_slice(&data.mesh_positions);
        }
        if let Some(VertexAttributeValues::Float32x4(mesh_col)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
        {
            mesh_col.copy_from_slice(&data.mesh_colors);
        }
    }
}
