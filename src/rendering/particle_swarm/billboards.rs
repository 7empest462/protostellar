use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use rayon::prelude::*;

use crate::simulation::resources::DiskParameters;

use super::ParticleSwarmData;

#[inline]
fn compute_particle_sizing_params(
    n: usize,
    disk_params: &DiskParameters,
    p_render_scale: f32,
) -> (bool, f32) {
    let is_galaxy = disk_params.outer_radius_au >= 10_000.0;
    let density_scale = if n > 0 && !is_galaxy {
        (500_000.0 / n as f32).powf(0.25).clamp(1.0, 1.5)
    } else {
        1.0
    };
    let scale_mult = if is_galaxy {
        (disk_params.outer_radius_au / 45.0).max(0.02) as f32
    } else {
        (disk_params.outer_radius_au / 45.0).clamp(0.02, 1.0) as f32
    };
    let base_render_r = if is_galaxy {
        0.16 * p_render_scale * scale_mult
    } else {
        0.080 * p_render_scale * scale_mult * density_scale
    };
    (is_galaxy, base_render_r)
}

#[inline]
fn compute_particle_radius(is_galaxy: bool, base_render_r: f32, m: f32, b_mass: f32) -> f32 {
    let m_factor = if is_galaxy || b_mass <= 0.0 {
        1.0
    } else {
        (m / b_mass).cbrt().clamp(1.0, 1.35)
    };
    base_render_r * m_factor
}

#[inline]
fn compute_particle_alpha(is_galaxy: bool, m: f32, b_mass: f32) -> f32 {
    if is_galaxy {
        0.85f32
    } else if m <= 1.1 * b_mass {
        0.38f32
    } else if m <= 3.0 * b_mass {
        0.62f32
    } else if m <= 8.0 * b_mass {
        0.85f32
    } else {
        1.0f32
    }
}

pub fn update_billboard_mesh_quads(
    data: &mut ParticleSwarmData,
    cam_right: Vec3,
    cam_up: Vec3,
    disk_params: &DiskParameters,
    p_render_scale: f32,
) {
    let n = data.count;
    let (is_galaxy, base_render_r) = compute_particle_sizing_params(n, disk_params, p_render_scale);
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
                    let r_v = compute_particle_radius(is_galaxy, base_render_r, m, b_mass);

                    let right = cam_right * r_v;
                    let up = cam_up * r_v;

                    *p0 = (p - right - up).to_array();
                    *p1 = (p + right - up).to_array();
                    *p2 = (p + right + up).to_array();
                    *p3 = (p - right + up).to_array();

                    let alpha = compute_particle_alpha(is_galaxy, m, b_mass);
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

/// Populates GPU-billboarded center positions, RGBA colors, and (when `update_normals`
/// is true) unscaled quad half-radii (`ATTRIBUTE_NORMAL.x`) for shader expansion.
pub fn update_gpu_billboard_mesh_with_normals(
    data: &mut ParticleSwarmData,
    disk_params: &DiskParameters,
    meshes: &mut Assets<Mesh>,
    update_normals: bool,
) {
    update_gpu_billboard_mesh_selective(data, disk_params, meshes, true, update_normals, true);
}

/// Synchronizes GPU-billboarded particle center positions, and selectively skips color/normal
/// updates and dormant tail copies when only active orbital positions moved.
pub fn update_gpu_billboard_mesh_selective(
    data: &mut ParticleSwarmData,
    disk_params: &DiskParameters,
    meshes: &mut Assets<Mesh>,
    update_colors: bool,
    update_normals: bool,
    full_buffer_sync: bool,
) {
    let n = data.count;
    let (is_galaxy, base_render_r) = compute_particle_sizing_params(n, disk_params, 1.0);
    let active_verts = if full_buffer_sync {
        data.mesh_positions.len()
    } else {
        (n * 4).min(data.mesh_positions.len())
    };

    if update_colors {
        if !update_billboard_positions_and_colors(data, n, active_verts, is_galaxy) {
            return;
        }
    } else {
        update_billboard_positions_only(data, n, active_verts);
    }

    sync_mesh_attributes_selective(data, meshes, update_colors, active_verts);

    if update_normals {
        update_billboard_normals(data, meshes, n, is_galaxy, base_render_r);
    }
}

fn update_billboard_positions_and_colors(
    data: &mut ParticleSwarmData,
    n: usize,
    active_verts: usize,
    is_galaxy: bool,
) -> bool {
    let masses = &data.masses;
    let positions = &data.positions;
    let colors = &data.colors;
    let b_mass = data.base_mass;
    let Some(pos_slice) = data.mesh_positions.get_mut(..active_verts) else {
        return false;
    };
    let Some(col_slice) = data.mesh_colors.get_mut(..active_verts) else {
        return false;
    };
    pos_slice
        .par_chunks_mut(4 * 2048)
        .zip(col_slice.par_chunks_mut(4 * 2048))
        .enumerate()
        .for_each(|(chunk_idx, (pos_out, col_out))| {
            let start_particle = chunk_idx * 2048;
            let particle_count = (start_particle + 2048)
                .min(n)
                .saturating_sub(start_particle);
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
                    let Some(&center) = positions.get(local_i) else {
                        continue;
                    };
                    *p0 = center;
                    *p1 = center;
                    *p2 = center;
                    *p3 = center;

                    let alpha = compute_particle_alpha(is_galaxy, m, b_mass);
                    let c = colors.get(local_i).copied().unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let final_col = [c[0], c[1], c[2], alpha];
                    *c0 = final_col;
                    *c1 = final_col;
                    *c2 = final_col;
                    *c3 = final_col;
                }
            }
        });
    true
}

fn update_billboard_positions_only(data: &mut ParticleSwarmData, n: usize, active_verts: usize) {
    let masses = &data.masses;
    let positions = &data.positions;
    if let Some(pos_slice) = data.mesh_positions.get_mut(..active_verts) {
        pos_slice
            .par_chunks_mut(4 * 2048)
            .enumerate()
            .for_each(|(chunk_idx, pos_out)| {
                let start_particle = chunk_idx * 2048;
                let particle_count = (start_particle + 2048)
                    .min(n)
                    .saturating_sub(start_particle);
                for (offset, quad_pos) in
                    pos_out.chunks_exact_mut(4).take(particle_count).enumerate()
                {
                    let local_i = start_particle + offset;
                    let Some(&m) = masses.get(local_i) else {
                        continue;
                    };
                    if let [p0, p1, p2, p3] = quad_pos {
                        if m <= 0.0 {
                            *p0 = [0.0, -5000.0, 0.0];
                            *p1 = [0.0, -5000.0, 0.0];
                            *p2 = [0.0, -5000.0, 0.0];
                            *p3 = [0.0, -5000.0, 0.0];
                            continue;
                        }
                        let Some(&center) = positions.get(local_i) else {
                            continue;
                        };
                        *p0 = center;
                        *p1 = center;
                        *p2 = center;
                        *p3 = center;
                    }
                }
            });
    }
}

fn update_billboard_normals(
    data: &ParticleSwarmData,
    meshes: &mut Assets<Mesh>,
    n: usize,
    is_galaxy: bool,
    base_render_r: f32,
) {
    let masses = &data.masses;
    let b_mass = data.base_mass;
    let Some(mut mesh) = meshes.get_mut(&data.mesh_handle) else {
        return;
    };
    let Some(VertexAttributeValues::Float32x3(mesh_norm)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
    else {
        return;
    };
    let norm_verts = (n * 4).min(mesh_norm.len());
    if let Some(active_slice) = mesh_norm.get_mut(..norm_verts) {
        active_slice
            .par_chunks_mut(4 * 2048)
            .enumerate()
            .for_each(|(chunk_idx, norm_out)| {
                let start_particle = chunk_idx * 2048;
                let particle_count = (start_particle + 2048)
                    .min(n)
                    .saturating_sub(start_particle);
                for (offset, quad_norm) in norm_out
                    .chunks_exact_mut(4)
                    .take(particle_count)
                    .enumerate()
                {
                    let local_i = start_particle + offset;
                    let m = masses.get(local_i).copied().unwrap_or(0.0);
                    let size_attr = if m <= 0.0 {
                        [0.0, 0.0, 0.0]
                    } else {
                        [
                            compute_particle_radius(is_galaxy, base_render_r, m, b_mass),
                            0.0,
                            0.0,
                        ]
                    };
                    quad_norm.fill(size_attr);
                }
            });
    }
    if let Some(dormant_slice) = mesh_norm.get_mut(norm_verts..) {
        dormant_slice.fill([0.0, 0.0, 0.0]);
    }
}

/// Populates GPU-billboarded center positions, RGBA colors, and unscaled quad half-radii
/// (`ATTRIBUTE_NORMAL.x`) so camera orbit/zoom needs zero CPU billboard rebuilds.
pub fn update_gpu_billboard_mesh(
    data: &mut ParticleSwarmData,
    disk_params: &DiskParameters,
    meshes: &mut Assets<Mesh>,
) {
    update_gpu_billboard_mesh_with_normals(data, disk_params, meshes, true);
}

pub fn sync_mesh_attributes(data: &ParticleSwarmData, meshes: &mut Assets<Mesh>) {
    sync_mesh_attributes_selective(data, meshes, true, data.mesh_positions.len());
}

fn sync_mesh_attributes_selective(
    data: &ParticleSwarmData,
    meshes: &mut Assets<Mesh>,
    sync_colors: bool,
    vertex_limit: usize,
) {
    if let Some(mut mesh) = meshes.get_mut(&data.mesh_handle) {
        if let Some(VertexAttributeValues::Float32x3(mesh_pos)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            let copy_len = vertex_limit
                .min(mesh_pos.len())
                .min(data.mesh_positions.len());
            if let (Some(dst), Some(src)) = (
                mesh_pos.get_mut(..copy_len),
                data.mesh_positions.get(..copy_len),
            ) {
                dst.copy_from_slice(src);
            }
        }
        if sync_colors {
            if let Some(VertexAttributeValues::Float32x4(mesh_col)) =
                mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
            {
                let copy_len = vertex_limit.min(mesh_col.len()).min(data.mesh_colors.len());
                if let (Some(dst), Some(src)) = (
                    mesh_col.get_mut(..copy_len),
                    data.mesh_colors.get(..copy_len),
                ) {
                    dst.copy_from_slice(src);
                }
            }
        }
    }
}
