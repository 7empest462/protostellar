//! Procedural 3D vegetation (tiered conifers, broadleaf crowns, alien umbrella flora,
//! understory bushes) and 3D geological outcrops (canyon hoodoos, faceted boulders,
//! ice penitentes, hydrothermal vents) for planetary surface rendering.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::PI;

use crate::simulation::components::{BiosphereState, BodyType, Composition, FloraPigmentClass};

use super::surface_camera::compute_surface_tangent_basis;
use super::surface_terrain::{
    evaluate_sea_level_offset, evaluate_surface_elevation, hash31, max_surface_relief,
};

/// Procedurally generates a combined 3D mesh of alien trees (tiered conifers, broadleaf
/// crowns, umbrella flora, understory bushes) or 3D geological outcrops (canyon hoodoos,
/// faceted boulders, ice penitentes, hydrothermal vents) across the terrain patch.
#[allow(
    clippy::too_many_arguments,
    reason = "Generates 3D botanical and geological meshes"
)]
pub fn generate_surface_vegetation_mesh(
    unspun_center: Vec3,
    vis_rad: f32,
    comp: &Composition,
    temp_k: f32,
    body_type: BodyType,
    opt_bio: Option<&BiosphereState>,
    surface_g: f32,
    patch_radius: f32,
) -> Mesh {
    let (u_east, u_up, u_north) = compute_surface_tangent_basis(unspun_center, Vec3::Y);
    let has_bio =
        opt_bio.is_some_and(|b| b.biomass_coverage_frac > 0.01 || b.habitability_score > 0.10);
    let pigment = opt_bio.map_or(FloraPigmentClass::SolarChlorophyll, |b| b.flora_pigment);
    let relief_max = max_surface_relief(body_type, temp_k, comp, vis_rad).max(1e-6);
    let sea_h = evaluate_sea_level_offset(body_type, temp_k, comp, vis_rad);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let plant_count = 150;
    for idx in 0..plant_count {
        let f_idx = idx as f32;
        // Golden-angle spiral distribution within the high-detail inner patch (r <= 0.82)
        let r_rel = 0.04 + ((f_idx + 0.5) / plant_count as f32).sqrt() * 0.78;
        let theta = f_idx * 2.399_963_1;
        let (sin_t, cos_t) = theta.sin_cos();

        let u = cos_t * r_rel;
        let v = sin_t * r_rel;
        let r_sq = u * u + v * v;
        let edge_blend = (1.0 - (r_sq - 0.72) / 0.28).clamp(0.0, 1.0);

        let tangent_offset = (u_east * u + u_north * v) * patch_radius;
        let sphere_dir = (u_up + tangent_offset / vis_rad).normalize_or_zero();

        let raw_h = evaluate_surface_elevation(sphere_dir, comp, temp_k, body_type, vis_rad);
        let h = raw_h * edge_blend;
        let norm_h = (raw_h / relief_max).clamp(0.0, 1.0);
        let base_pos = sphere_dir * (vis_rad + h);

        // Full 2D tangent gradient slope check so trees never grow on sheer canyon walls
        let eps = 0.0025;
        let dir_u = (sphere_dir + u_east * eps).normalize_or_zero();
        let dir_v = (sphere_dir + u_north * eps).normalize_or_zero();
        let dh_u = evaluate_surface_elevation(dir_u, comp, temp_k, body_type, vis_rad) - raw_h;
        let dh_v = evaluate_surface_elevation(dir_v, comp, temp_k, body_type, vis_rad) - raw_h;
        let slope_grad = dh_u.hypot(dh_v) / (vis_rad * eps);

        let hash = hash31(sphere_dir * 123.45);
        if has_bio {
            // Skip submerged lake/ocean floors, sheer canyon cliffs, and frozen alpine peaks
            if raw_h < sea_h + vis_rad * 0.0006 || slope_grad > 0.36 || norm_h > 0.72 {
                continue;
            }
            let tree_height = vis_rad * (0.007 + 0.009 * hash) / surface_g.sqrt();
            let canopy_radius = vis_rad * (0.0038 + 0.0045 * hash) * surface_g.powf(0.30);

            append_3d_tree(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut colors,
                &mut indices,
                base_pos,
                sphere_dir,
                u_north,
                tree_height,
                canopy_radius,
                idx,
                pigment,
            );
        } else {
            let boulder_r = vis_rad * (0.0022 + 0.0038 * hash);
            append_3d_geological_outcrop(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut colors,
                &mut indices,
                base_pos,
                sphere_dir,
                boulder_r,
                temp_k,
                comp.ice_frac as f32,
                idx,
            );
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

/// Appends a 3D procedural plant (conifer tiers, broadleaf crown, alien umbrella mushroom,
/// or understory bush) to the vegetation mesh buffers.
#[allow(clippy::too_many_arguments, reason = "3D botanical mesh builder")]
fn append_3d_tree(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    base_pos: Vec3,
    up: Vec3,
    heading: Vec3,
    height: f32,
    canopy_radius: f32,
    tree_idx: usize,
    pigment: FloraPigmentClass,
) {
    let (east, _, north) = compute_surface_tangent_basis(up, heading);
    let (col_a, col_b) = match pigment {
        FloraPigmentClass::SolarChlorophyll => (
            LinearRgba::new(0.14, 0.74, 0.28, 1.0),
            LinearRgba::new(0.38, 0.94, 0.50, 1.0),
        ),
        FloraPigmentClass::RedDwarfCrimson => (
            LinearRgba::new(0.80, 0.10, 0.28, 1.0),
            LinearRgba::new(1.00, 0.36, 0.54, 1.0),
        ),
        FloraPigmentClass::OrangeDwarfCarotenoid => (
            LinearRgba::new(0.92, 0.54, 0.10, 1.0),
            LinearRgba::new(1.00, 0.82, 0.30, 1.0),
        ),
        FloraPigmentClass::HighUvIndigo => (
            LinearRgba::new(0.22, 0.62, 0.98, 1.0),
            LinearRgba::new(0.64, 0.36, 1.00, 1.0),
        ),
    };
    let mix_t = ((tree_idx as f32 * 0.41).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let lower_col = col_a.mix(&col_b, mix_t * 0.4);
    let upper_col = col_a.mix(&col_b, (mix_t * 0.5 + 0.5).clamp(0.0, 1.0));
    let lower_rgba = [lower_col.red, lower_col.green, lower_col.blue, 1.0];
    let upper_rgba = [upper_col.red, upper_col.green, upper_col.blue, 1.0];

    let archetype = tree_idx % 4;
    if archetype < 3 {
        let trunk_r_base = canopy_radius * 0.20;
        let trunk_r_top = trunk_r_base * 0.48;
        let bark_color = if archetype == 2 {
            [0.42, 0.36, 0.32, 1.0]
        } else {
            [0.30, 0.19, 0.12, 1.0]
        };
        append_tapered_cylinder(
            positions,
            normals,
            uvs,
            colors,
            indices,
            base_pos,
            up,
            east,
            north,
            trunk_r_base,
            trunk_r_top,
            height,
            6,
            bark_color,
        );
    }

    append_tree_canopy(
        positions,
        normals,
        uvs,
        colors,
        indices,
        base_pos,
        up,
        east,
        north,
        height,
        canopy_radius,
        archetype,
        lower_rgba,
        upper_rgba,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Dispatches botanical canopy archetype"
)]
fn append_tree_canopy(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    base_pos: Vec3,
    up: Vec3,
    east: Vec3,
    north: Vec3,
    height: f32,
    canopy_radius: f32,
    archetype: usize,
    lower_rgba: [f32; 4],
    upper_rgba: [f32; 4],
) {
    let top_pos = base_pos + up * height;
    match archetype {
        0 => {
            // Archetype 0: 3-tier layered Conifer / Pine spire
            for tier in 0..3 {
                let tf = tier as f32;
                let tier_base = base_pos + up * (height * (0.45 + tf * 0.32));
                let tier_r = canopy_radius * (1.15 - tf * 0.26);
                let tier_h = canopy_radius * (1.10 - tf * 0.15);
                append_foliage_cone(
                    positions, normals, uvs, colors, indices, tier_base, up, east, north, tier_r,
                    tier_h, 8, lower_rgba, upper_rgba,
                );
            }
        }
        1 => {
            // Archetype 1: Broadleaf Deciduous Dome + 2 side-branch foliage clusters
            append_foliage_dome(
                positions,
                normals,
                uvs,
                colors,
                indices,
                top_pos,
                up,
                east,
                north,
                canopy_radius * 1.15,
                canopy_radius * 0.95,
                lower_rgba,
                upper_rgba,
            );
            for side in [-1.0f32, 1.0f32] {
                let branch_pos = base_pos
                    + up * (height * 0.72)
                    + (east * side + north * (side * 0.4)) * (canopy_radius * 0.65);
                append_foliage_dome(
                    positions,
                    normals,
                    uvs,
                    colors,
                    indices,
                    branch_pos,
                    up,
                    east,
                    north,
                    canopy_radius * 0.72,
                    canopy_radius * 0.62,
                    lower_rgba,
                    upper_rgba,
                );
            }
        }
        2 => {
            // Archetype 2: Alien Umbrella / Bioluminescent Mushroom Canopy
            append_foliage_cone(
                positions,
                normals,
                uvs,
                colors,
                indices,
                top_pos - up * (canopy_radius * 0.12),
                up,
                east,
                north,
                canopy_radius * 1.45,
                canopy_radius * 0.55,
                10,
                lower_rgba,
                upper_rgba,
            );
        }
        _ => {
            // Archetype 3: Low Understory Bush / Giant Fern Cluster on ground
            append_foliage_dome(
                positions,
                normals,
                uvs,
                colors,
                indices,
                base_pos,
                up,
                east,
                north,
                canopy_radius * 0.95,
                canopy_radius * 0.65,
                lower_rgba,
                upper_rgba,
            );
        }
    }
}

/// Appends a tapered polygonal cylinder (tree trunk, stalk, or basalt column).
#[allow(clippy::too_many_arguments, reason = "Low-level cylinder primitive")]
fn append_tapered_cylinder(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    base_pos: Vec3,
    up: Vec3,
    east: Vec3,
    north: Vec3,
    r_bot: f32,
    r_top: f32,
    height: f32,
    sides: usize,
    color: [f32; 4],
) {
    let base_v = positions.len() as u32;
    let top_pos = base_pos + up * height;

    for i in 0..sides {
        let theta = (i as f32 / sides as f32) * 2.0 * PI;
        let (st, ct) = theta.sin_cos();
        let radial = east * ct + north * st;

        positions.push((base_pos + radial * r_bot).to_array());
        normals.push(radial.to_array());
        uvs.push([i as f32 / sides as f32, 0.0]);
        colors.push(color);

        positions.push((top_pos + radial * r_top).to_array());
        normals.push(radial.to_array());
        uvs.push([i as f32 / sides as f32, 1.0]);
        colors.push(color);

        let cur_bot = base_v + (i * 2) as u32;
        let cur_top = cur_bot + 1;
        let next_bot = base_v + (((i + 1) % sides) * 2) as u32;
        let next_top = next_bot + 1;

        indices.push(cur_bot);
        indices.push(next_bot);
        indices.push(cur_top);

        indices.push(next_bot);
        indices.push(next_top);
        indices.push(cur_top);
    }
}

/// Appends a shaded polygonal cone (conifer tier or umbrella cap).
#[allow(clippy::too_many_arguments, reason = "Low-level cone primitive")]
fn append_foliage_cone(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    base_pos: Vec3,
    up: Vec3,
    east: Vec3,
    north: Vec3,
    radius: f32,
    height: f32,
    sides: usize,
    rim_color: [f32; 4],
    apex_color: [f32; 4],
) {
    let apex_idx = positions.len() as u32;
    let apex_pos = base_pos + up * height;

    positions.push(apex_pos.to_array());
    normals.push(up.to_array());
    uvs.push([0.5, 1.0]);
    colors.push(apex_color);

    for i in 0..sides {
        let theta = (i as f32 / sides as f32) * 2.0 * PI;
        let (st, ct) = theta.sin_cos();
        let radial = east * ct + north * st;
        let rim_pos = base_pos + radial * radius;

        positions.push(rim_pos.to_array());
        normals.push((radial + up * 0.55).normalize_or_zero().to_array());
        uvs.push([i as f32 / sides as f32, 0.0]);
        colors.push(rim_color);

        let cur_rim = apex_idx + 1 + i as u32;
        let next_rim = apex_idx + 1 + ((i + 1) % sides) as u32;
        indices.push(apex_idx);
        indices.push(cur_rim);
        indices.push(next_rim);
    }
}

/// Appends a 2-ring geodesic foliage dome (broadleaf tree crown or bush).
#[allow(clippy::too_many_arguments, reason = "Low-level dome primitive")]
fn append_foliage_dome(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    center_pos: Vec3,
    up: Vec3,
    east: Vec3,
    north: Vec3,
    radius: f32,
    height: f32,
    lower_color: [f32; 4],
    upper_color: [f32; 4],
) {
    let mid_pos = center_pos + up * (height * 0.45);
    append_foliage_cone(
        positions,
        normals,
        uvs,
        colors,
        indices,
        mid_pos,
        up,
        east,
        north,
        radius,
        height * 0.65,
        8,
        lower_color,
        upper_color,
    );
    append_tapered_cylinder(
        positions,
        normals,
        uvs,
        colors,
        indices,
        center_pos - up * (height * 0.15),
        up,
        east,
        north,
        radius * 0.72,
        radius,
        height * 0.60,
        8,
        lower_color,
    );
}

/// Appends a 3D geological formation (canyon hoodoo spire, faceted boulder, ice penitente,
/// or hydrothermal vent) on barren, cryogenic, or volcanic worlds.
#[allow(clippy::too_many_arguments, reason = "3D geological outcrop builder")]
fn append_3d_geological_outcrop(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    base_pos: Vec3,
    up: Vec3,
    radius: f32,
    temp_k: f32,
    ice_frac: f32,
    idx: usize,
) {
    let (east, _, north) = compute_surface_tangent_basis(up, Vec3::Y);
    if temp_k >= 500.0 {
        let h = radius * 2.8;
        append_tapered_cylinder(
            positions,
            normals,
            uvs,
            colors,
            indices,
            base_pos,
            up,
            east,
            north,
            radius * 0.85,
            radius * 0.42,
            h,
            6,
            [0.18, 0.15, 0.14, 1.0],
        );
        append_foliage_cone(
            positions,
            normals,
            uvs,
            colors,
            indices,
            base_pos + up * h,
            up,
            east,
            north,
            radius * 0.45,
            radius * 0.55,
            6,
            [0.95, 0.32, 0.05, 1.0],
            [1.00, 0.75, 0.18, 1.0],
        );
    } else if temp_k <= 230.0 && ice_frac >= 0.20 {
        append_foliage_cone(
            positions,
            normals,
            uvs,
            colors,
            indices,
            base_pos,
            up,
            east,
            north,
            radius * 0.65,
            radius * 3.4,
            6,
            [0.35, 0.80, 0.96, 1.0],
            [0.88, 0.97, 1.00, 1.0],
        );
    } else if idx.is_multiple_of(4) {
        let h = radius * 2.6;
        append_tapered_cylinder(
            positions,
            normals,
            uvs,
            colors,
            indices,
            base_pos,
            up,
            east,
            north,
            radius * 0.75,
            radius * 0.55,
            h,
            6,
            [0.68, 0.32, 0.18, 1.0],
        );
        append_foliage_cone(
            positions,
            normals,
            uvs,
            colors,
            indices,
            base_pos + up * h,
            up,
            east,
            north,
            radius * 0.68,
            radius * 0.55,
            6,
            [0.80, 0.68, 0.48, 1.0],
            [0.62, 0.52, 0.38, 1.0],
        );
    } else {
        append_foliage_dome(
            positions,
            normals,
            uvs,
            colors,
            indices,
            base_pos,
            up,
            east,
            north,
            radius,
            radius * 1.15,
            [0.48, 0.40, 0.34, 1.0],
            [0.64, 0.54, 0.46, 1.0],
        );
    }
}
