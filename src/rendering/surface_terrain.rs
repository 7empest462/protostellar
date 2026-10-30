//! Procedural 3D planetary surface terrain generation with canyons, mountain ranges,
//! geological strata, 3D vegetation (tiered conifers, broadleaf crowns, umbrella flora,
//! understory bushes), 3D geological formations (hoodoos, boulders, penitentes, vents),
//! and liquid ocean planes for First-Person Surface & Rover Horizon mode.

use bevy::asset::RenderAssetUsages;
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::PI;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::celestial::SpinState;
use crate::simulation::components::{
    BiosphereState, BodyType, CelestialBody, Composition, FloraPigmentClass, Mass, Radius,
    SimPosition, Temperature, VolatileInventory,
};
use crate::simulation::probes::navigation::compute_planet_spin_frame;
use crate::simulation::probes::{FlightComputer, ProbeStatus, ProbeType, SpaceProbe};
use crate::simulation::resources::{PlayerInteractionState, SimTime, SimulationConfig};
use crate::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU};

use super::surface_camera::{compute_surface_tangent_basis, lat_lon_to_unspun_normal};

/// Root marker component for all 3D surface terrain, vegetation, and water entities.
#[derive(Component)]
pub struct PlanetarySurfaceTerrainRoot;

/// Marker for the 3D high-density terrain heightfield mesh.
#[derive(Component)]
pub struct PlanetarySurfaceTerrainMesh;

/// Marker for the 3D procedural vegetation mesh (trees, canopies, alien flora).
#[derive(Component)]
pub struct PlanetarySurfaceVegetationMesh;

/// Marker for the liquid ocean/water surface plane.
#[derive(Component)]
pub struct PlanetarySurfaceWaterMesh;

/// Tracks the active surface terrain state and cached landing location.
#[derive(Resource, Default)]
pub struct SurfaceTerrainState {
    pub active_host: Option<Entity>,
    pub last_unspun_center: Vec3,
    pub root_entity: Option<Entity>,
}

/// Normalized sea/river water level in `[0.0, 1.0]` relief units.
pub const SURFACE_SEA_LEVEL_NORM: f32 = 0.20;

// ============================================================================
// Procedural Noise & Fractal Math (Strictly Positive Fractional Parts)
// ============================================================================

/// Euclidean fractional part `x - floor(x)` guaranteed to lie in `[0.0, 1.0)`
/// for both positive and negative inputs.
#[inline]
fn fract_pos(x: f32) -> f32 {
    x - x.floor()
}

/// Component-wise Euclidean fractional part `v - v.floor()` in `[0.0, 1.0)^3`.
#[inline]
fn fract_vec3(v: Vec3) -> Vec3 {
    v - v.floor()
}

/// Fast, continuous 3D hash generator returning a value strictly in `[0.0, 1.0)`.
pub(crate) fn hash31(p: Vec3) -> f32 {
    let p3 = fract_vec3(p * 0.1031);
    let p3 = p3 + p3.dot(Vec3::new(p3.y, p3.z, p3.x) + 33.33);
    fract_pos((p3.x + p3.y) * p3.z)
}

/// Smooth trilinear value noise in 3D, strictly bounded in `[0.0, 1.0]`.
fn noise3d(p: Vec3) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec3::splat(3.0) - 2.0 * f);

    let n000 = hash31(i);
    let n100 = hash31(i + Vec3::new(1.0, 0.0, 0.0));
    let n010 = hash31(i + Vec3::new(0.0, 1.0, 0.0));
    let n110 = hash31(i + Vec3::new(1.0, 1.0, 0.0));
    let n001 = hash31(i + Vec3::new(0.0, 0.0, 1.0));
    let n101 = hash31(i + Vec3::new(1.0, 0.0, 1.0));
    let n011 = hash31(i + Vec3::new(0.0, 1.0, 1.0));
    let n111 = hash31(i + Vec3::new(1.0, 1.0, 1.0));

    let x00 = n000 * (1.0 - u.x) + n100 * u.x;
    let x10 = n010 * (1.0 - u.x) + n110 * u.x;
    let x01 = n001 * (1.0 - u.x) + n101 * u.x;
    let x11 = n011 * (1.0 - u.x) + n111 * u.x;

    let y0 = x00 * (1.0 - u.y) + x10 * u.y;
    let y1 = x01 * (1.0 - u.y) + x11 * u.y;

    (y0 * (1.0 - u.z) + y1 * u.z).clamp(0.0, 1.0)
}

/// Multi-octave Fractal Brownian Motion (FBM) with inter-octave rotation.
fn fbm3d(mut p: Vec3, octaves: usize) -> f32 {
    let mut total = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for _ in 0..octaves {
        total += noise3d(p * freq) * amp;
        freq *= 2.03;
        amp *= 0.5;
        p = Vec3::new(p.y * 0.866 + p.z * 0.5, p.z * 0.866 - p.y * 0.5, p.x);
    }
    total
}

/// Evaluates canyon depth and terraced strata step at a given unit sphere position.
/// Combines a primary meandering grand canyon chasm with branching tributary ravines.
/// Returns `(canyon_depth_factor 0.0..1.0, wall_slope_factor 0.0..1.0)`.
pub fn evaluate_canyon_carve(coord: Vec3) -> (f32, f32) {
    let p = coord * 16.0;
    // Domain warping creates natural river-like meanders and tectonic rifts
    let warp = Vec3::new(
        noise3d(p + Vec3::new(1.2, 3.4, 5.6)) - 0.5,
        noise3d(p + Vec3::new(7.8, 9.1, 2.3)) - 0.5,
        noise3d(p + Vec3::new(4.5, 6.7, 8.9)) - 0.5,
    ) * 0.65;

    let p_warped = p + warp;
    let main_channel = (noise3d(p_warped) * 2.0 - 1.0).abs();
    let main_width = 0.28;

    // Secondary dendritic tributary side-canyons
    let p_trib = coord * 38.0 + warp * 1.4;
    let trib_channel = (noise3d(p_trib) * 2.0 - 1.0).abs();
    let trib_width = 0.16;

    let main_t = if main_channel < main_width {
        (1.0 - main_channel / main_width).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let trib_t = if trib_channel < trib_width {
        (1.0 - trib_channel / trib_width).clamp(0.0, 1.0) * 0.62
    } else {
        0.0
    };

    let t = main_t.max(trib_t);
    if t > 0.0 {
        let raw_depth = t.max(0.0).powf(1.50);

        // 10-stepped horizontal benches & sedimentary ledges (Grand Canyon / Valles Marineris strata)
        let k = raw_depth * 10.0;
        let step = (k.floor() + ((fract_pos(k) - 0.65) / 0.35).clamp(0.0, 1.0)) / 10.0;
        let final_depth = (0.70 * step + 0.30 * raw_depth).clamp(0.0, 1.0);
        (final_depth, t)
    } else {
        (0.0, 0.0)
    }
}

/// Evaluates rolling hills, continental plateaus, and alpine mountain massifs in `[0.0, 1.0]`.
pub fn evaluate_mountain_and_hills(coord: Vec3) -> f32 {
    let swell = fbm3d(coord * 5.0, 2) * 0.28;
    let foothills = (fbm3d(coord * 16.0, 3) - 0.45) * 0.32;
    let peak_noise = (1.0 - (noise3d(coord * 32.0) * 2.0 - 1.0).abs()).clamp(0.0, 1.0);
    let alpine_peaks = peak_noise.powf(2.3) * 0.48;
    (swell + foothills + alpine_peaks).clamp(-0.15, 0.65)
}

/// Returns the maximum vertical relief scale in visual units for a body.
pub fn max_surface_relief(
    body_type: BodyType,
    temp_k: f32,
    comp: &Composition,
    vis_rad: f32,
) -> f32 {
    let is_extreme = temp_k > 500.0 || (temp_k < 220.0 && comp.ice_frac > 0.25);
    let factor = match body_type {
        BodyType::SuperEarth => 0.038,
        BodyType::Moon | BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal => 0.030,
        _ if is_extreme => 0.036,
        _ => 0.034,
    };
    vis_rad * factor
}

/// Evaluates the true radial surface height offset $\Delta r > 0$ above `vis_rad`.
///
/// The datum is structured so that even the deepest canyon floor (`norm_h >= 0.05`) sits
/// strictly above the planet's base orbital sphere (`1.000 * vis_rad`), preventing the
/// orbital sphere from ever clipping or occluding canyon chasms, while keeping the highest
/// alpine peaks (`norm_h <= 1.00`) safely inside the atmospheric limb shell.
pub fn evaluate_surface_elevation(
    unspun_dir: Vec3,
    comp: &Composition,
    temp_k: f32,
    body_type: BodyType,
    vis_rad: f32,
) -> f32 {
    let base_plateau = 0.48;
    let uplands = evaluate_mountain_and_hills(unspun_dir);
    let (canyon_depth, _) = evaluate_canyon_carve(unspun_dir);

    let norm_h = (base_plateau + uplands - canyon_depth * 0.46).clamp(0.05, 1.00);
    norm_h * max_surface_relief(body_type, temp_k, comp, vis_rad)
}

/// Computes the radial sea/river water offset above `vis_rad` when surface liquids exist.
pub fn evaluate_sea_level_offset(
    body_type: BodyType,
    temp_k: f32,
    comp: &Composition,
    vis_rad: f32,
) -> f32 {
    SURFACE_SEA_LEVEL_NORM * max_surface_relief(body_type, temp_k, comp, vis_rad)
}

/// Determines whether a world possesses surface liquid water or hydrocarbon seas.
pub fn planet_has_surface_liquids(
    body_type: BodyType,
    comp: &Composition,
    temp_k: f32,
    opt_bio: Option<&BiosphereState>,
    opt_vol: Option<&VolatileInventory>,
) -> bool {
    if matches!(body_type, BodyType::GasGiant) {
        return false;
    }
    let has_water_ocean = (250.0..=395.0).contains(&temp_k)
        && (opt_vol.is_some_and(|v| v.ocean_coverage_frac > 0.02)
            || opt_bio
                .is_some_and(|b| b.biomass_coverage_frac > 0.01 || b.habitability_score > 0.15)
            || comp.ice_frac > 0.05);
    let has_methane_sea = comp.organics_frac > 0.10 && (85.0..=140.0).contains(&temp_k);
    has_water_ocean || has_methane_sea
}

// ============================================================================
// 3D Terrain Heightfield Mesh Generator
// ============================================================================

/// Generates a high-density 3D terrain heightfield mesh conforming to the planet's
/// spherical horizon, carved with canyons, rolling hills, and sedimentary rock strata.
pub fn generate_surface_terrain_mesh(
    unspun_center: Vec3,
    vis_rad: f32,
    comp: &Composition,
    temp_k: f32,
    body_type: BodyType,
    opt_bio: Option<&BiosphereState>,
    patch_radius: f32,
) -> Mesh {
    let grid_size = 120; // 121 x 121 vertices = 14,641 vertices; 28,800 triangles
    let (u_east, u_up, u_north) = compute_surface_tangent_basis(unspun_center, Vec3::Y);
    let relief_max = max_surface_relief(body_type, temp_k, comp, vis_rad).max(1e-6);

    let total_verts = (grid_size + 1) * (grid_size + 1);
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(total_verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(total_verts);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_verts);
    let mut indices: Vec<u32> = Vec::with_capacity(grid_size * grid_size * 6);

    let active_bio =
        opt_bio.filter(|b| b.biomass_coverage_frac > 0.01 || b.habitability_score > 0.10);
    let pigment = opt_bio.map_or(FloraPigmentClass::SolarChlorophyll, |b| b.flora_pigment);
    let sun_dir = (u_up * 0.65 + u_east * 0.55 + u_north * 0.52).normalize_or_zero();

    for j in 0..=grid_size {
        let v = (j as f32 / grid_size as f32) * 2.0 - 1.0;
        for i in 0..=grid_size {
            let u = (i as f32 / grid_size as f32) * 2.0 - 1.0;

            // Smooth circular taper at the far horizon boundary so the patch blends into the sphere
            let r_sq = u * u + v * v;
            let edge_blend = (1.0 - (r_sq - 0.72) / 0.28).clamp(0.0, 1.0);

            let tangent_offset = (u_east * u + u_north * v) * patch_radius;
            let sphere_dir = (u_up + tangent_offset / vis_rad).normalize_or_zero();

            let raw_h = evaluate_surface_elevation(sphere_dir, comp, temp_k, body_type, vis_rad);
            let h = raw_h * edge_blend;

            let vertex_pos = sphere_dir * (vis_rad + h);
            positions.push(vertex_pos.to_array());
            uvs.push([i as f32 / grid_size as f32, j as f32 / grid_size as f32]);

            // Outward surface normal via finite difference
            let eps = 0.0025;
            let dir_u = (sphere_dir + u_east * eps).normalize_or_zero();
            let dir_v = (sphere_dir + u_north * eps).normalize_or_zero();
            let h_u =
                evaluate_surface_elevation(dir_u, comp, temp_k, body_type, vis_rad) * edge_blend;
            let h_v =
                evaluate_surface_elevation(dir_v, comp, temp_k, body_type, vis_rad) * edge_blend;

            let pos_u = dir_u * (vis_rad + h_u);
            let pos_v = dir_v * (vis_rad + h_v);
            let mut norm = (pos_u - vertex_pos)
                .cross(pos_v - vertex_pos)
                .normalize_or_zero();
            if norm.length_squared() < 0.5 {
                norm = sphere_dir;
            } else if norm.dot(sphere_dir) < 0.0 {
                norm = -norm;
            }
            normals.push(norm.to_array());

            // Slope, normalized elevation, canyon depth, and topographic hillshading
            let slope = 1.0 - norm.dot(sphere_dir).clamp(0.0, 1.0);
            let norm_h = (raw_h / relief_max).clamp(0.0, 1.0);
            let (canyon_depth, _) = evaluate_canyon_carve(sphere_dir);
            let hillshade =
                (0.62 + 0.38 * norm.dot(sun_dir).clamp(0.0, 1.0)) * (1.0 - canyon_depth * 0.25);

            let vertex_col = compute_terrain_vertex_color(
                slope,
                norm_h,
                canyon_depth,
                hillshade,
                comp,
                temp_k,
                active_bio.is_some(),
                pigment,
            );
            colors.push(vertex_col);
        }
    }

    for j in 0..grid_size {
        for i in 0..grid_size {
            let row1 = (j * (grid_size + 1) + i) as u32;
            let row2 = ((j + 1) * (grid_size + 1) + i) as u32;

            indices.push(row1);
            indices.push(row2);
            indices.push(row1 + 1);

            indices.push(row2);
            indices.push(row2 + 1);
            indices.push(row1 + 1);
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

/// Computes terrain vertex color including banded canyon strata, shorelines, biomes, and hillshading.
#[allow(
    clippy::too_many_arguments,
    reason = "Evaluates elevation, slope, strata, and biosphere palette"
)]
fn compute_terrain_vertex_color(
    slope: f32,
    norm_h: f32,
    canyon_depth: f32,
    hillshade: f32,
    comp: &Composition,
    temp_k: f32,
    has_bio: bool,
    pigment: FloraPigmentClass,
) -> [f32; 4] {
    // 1. Canyon wall rock strata (banded horizontal sedimentary layers on steep slopes or carved gorges)
    if slope > 0.16 || canyon_depth > 0.32 {
        let band_freq = norm_h * 22.0 * PI;
        let band = (band_freq.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let sub_band = ((band_freq * 3.5).cos() * 0.5 + 0.5).clamp(0.0, 1.0);

        let (strata_a, strata_b, strata_c) = if temp_k <= 230.0 && comp.ice_frac >= 0.20 {
            (
                LinearRgba::new(0.28, 0.68, 0.88, 1.0),
                LinearRgba::new(0.75, 0.92, 1.00, 1.0),
                LinearRgba::new(0.18, 0.38, 0.58, 1.0),
            )
        } else {
            (
                LinearRgba::new(0.72, 0.30, 0.16, 1.0), // Iron-oxide sandstone terracotta
                LinearRgba::new(0.85, 0.74, 0.54, 1.0), // Buff limestone bench
                LinearRgba::new(0.30, 0.22, 0.18, 1.0), // Dark shale stratum
            )
        };
        let mixed = strata_a
            .mix(&strata_b, band)
            .mix(&strata_c, sub_band * 0.48);
        return [
            mixed.red * hillshade,
            mixed.green * hillshade,
            mixed.blue * hillshade,
            1.0,
        ];
    }

    // 2. High alpine snowcaps & glacial summits
    if norm_h > 0.74 {
        let snow = LinearRgba::new(0.94, 0.97, 1.0, 1.0);
        let rock = LinearRgba::new(0.50, 0.48, 0.46, 1.0);
        let t = ((norm_h - 0.74) / 0.26).clamp(0.0, 1.0);
        let col = rock.mix(&snow, t);
        return [
            col.red * hillshade,
            col.green * hillshade,
            col.blue * hillshade,
            1.0,
        ];
    }

    // 3. Lush biosphere flatlands, riverbanks & valleys
    if has_bio {
        // Sandy / alluvial canyon floor & shoreline near water level
        if norm_h < SURFACE_SEA_LEVEL_NORM + 0.04 {
            let sand = LinearRgba::new(0.76, 0.68, 0.50, 1.0);
            return [
                sand.red * hillshade,
                sand.green * hillshade,
                sand.blue * hillshade,
                1.0,
            ];
        }

        let (turf_base, turf_highlight) = match pigment {
            FloraPigmentClass::SolarChlorophyll => (
                LinearRgba::new(0.16, 0.64, 0.24, 1.0),
                LinearRgba::new(0.36, 0.86, 0.42, 1.0),
            ),
            FloraPigmentClass::RedDwarfCrimson => (
                LinearRgba::new(0.70, 0.12, 0.26, 1.0),
                LinearRgba::new(0.94, 0.28, 0.46, 1.0),
            ),
            FloraPigmentClass::OrangeDwarfCarotenoid => (
                LinearRgba::new(0.84, 0.50, 0.12, 1.0),
                LinearRgba::new(1.00, 0.76, 0.26, 1.0),
            ),
            FloraPigmentClass::HighUvIndigo => (
                LinearRgba::new(0.20, 0.56, 0.90, 1.0),
                LinearRgba::new(0.56, 0.34, 0.98, 1.0),
            ),
        };
        let t = ((norm_h - 0.24) / 0.50).clamp(0.0, 1.0);
        let turf = turf_base.mix(&turf_highlight, t * 0.65);
        return [
            turf.red * hillshade,
            turf.green * hillshade,
            turf.blue * hillshade,
            1.0,
        ];
    }

    // 4. Volcanic lava world
    if temp_k >= 500.0 {
        let basalt = LinearRgba::new(0.16, 0.14, 0.14, 1.0);
        let magma = LinearRgba::new(1.0, 0.40, 0.06, 1.0);
        let glow = (1.0 - norm_h * 2.2).clamp(0.0, 1.0);
        let col = basalt.mix(&magma, glow * 0.88);
        return [col.red, col.green, col.blue, 1.0];
    }

    // 5. Cryogenic ice world
    if temp_k <= 230.0 && comp.ice_frac >= 0.20 {
        let ice = LinearRgba::new(0.40, 0.84, 0.96, 1.0);
        let firn = LinearRgba::new(0.88, 0.95, 1.00, 1.0);
        let col = ice.mix(&firn, norm_h);
        return [
            col.red * hillshade,
            col.green * hillshade,
            col.blue * hillshade,
            1.0,
        ];
    }

    // 6. Barren rocky regolith (Mars/Moon)
    let dust = LinearRgba::new(0.70, 0.36, 0.20, 1.0);
    let bedrock = LinearRgba::new(0.46, 0.40, 0.36, 1.0);
    let col = dust.mix(&bedrock, norm_h);
    [
        col.red * hillshade,
        col.green * hillshade,
        col.blue * hillshade,
        1.0,
    ]
}

// ============================================================================
// 3D Procedural Vegetation & Geological Formations Mesh Generator
// ============================================================================

pub use super::surface_vegetation::generate_surface_vegetation_mesh;

// ============================================================================
// Liquid Ocean & Canyon River Water Plane Mesh Generator
// ============================================================================

/// Generates a curved spherical water plane at `vis_rad + sea_level_offset` filling
/// lowlands and deep canyon floors without Z-fighting against the base sphere.
pub fn generate_surface_water_mesh(unspun_center: Vec3, vis_rad: f32, patch_radius: f32) -> Mesh {
    generate_surface_water_mesh_at_offset(unspun_center, vis_rad, vis_rad * 0.0068, patch_radius)
}

/// Generates a curved spherical water plane at an explicit radial offset above `vis_rad`.
pub fn generate_surface_water_mesh_at_offset(
    unspun_center: Vec3,
    vis_rad: f32,
    sea_offset: f32,
    patch_radius: f32,
) -> Mesh {
    let sectors = 48;
    let rings = 16;
    let (u_east, u_up, u_north) = compute_surface_tangent_basis(unspun_center, Vec3::Y);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let water_r = vis_rad + sea_offset.max(vis_rad * 0.002);
    let water_col = [0.08, 0.48, 0.74, 0.84];

    positions.push((u_up * water_r).to_array());
    normals.push(u_up.to_array());
    uvs.push([0.5, 0.5]);
    colors.push(water_col);

    for r in 1..=rings {
        let r_frac = r as f32 / rings as f32;
        let rad = r_frac * patch_radius * 0.88;
        let edge_blend = (1.0 - (r_frac * r_frac - 0.72) / 0.28).clamp(0.15, 1.0);
        let ring_water_r = vis_rad + sea_offset.max(vis_rad * 0.002) * edge_blend;

        for s in 0..sectors {
            let theta = (s as f32 / sectors as f32) * 2.0 * PI;
            let (st, ct) = theta.sin_cos();

            let offset = (u_east * ct + u_north * st) * rad;
            let dir = (u_up + offset / vis_rad).normalize_or_zero();
            let pos = dir * ring_water_r;

            positions.push(pos.to_array());
            normals.push(dir.to_array());
            uvs.push([0.5 + ct * r_frac * 0.5, 0.5 + st * r_frac * 0.5]);
            colors.push(water_col);

            let curr = (1 + (r - 1) * sectors + s) as u32;
            let next_s = (s + 1) % sectors;
            let curr_next = (1 + (r - 1) * sectors + next_s) as u32;

            if r == 1 {
                indices.push(0);
                indices.push(curr);
                indices.push(curr_next);
            } else {
                let prev = (1 + (r - 2) * sectors + s) as u32;
                let prev_next = (1 + (r - 2) * sectors + next_s) as u32;

                indices.push(prev);
                indices.push(curr);
                indices.push(curr_next);

                indices.push(prev);
                indices.push(curr_next);
                indices.push(prev_next);
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

// ============================================================================
// ECS Lifecycle & Terrain Sync System
// ============================================================================

type TerrainWorldQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    &'a Radius,
    &'a Mass,
    &'a Temperature,
    &'a Composition,
    &'a CelestialBody,
    Option<&'a SpinState>,
    Option<&'a BiosphereState>,
    Option<&'a VolatileInventory>,
    Option<&'a Transform>,
);

/// Helper to recursively despawn the surface terrain root and all its children.
fn despawn_terrain_root(commands: &mut Commands, root: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(root) {
        for child in children.iter() {
            if let Ok(mut c_cmd) = commands.get_entity(child) {
                c_cmd.try_despawn();
            }
        }
    }
    if let Ok(mut cmd) = commands.get_entity(root) {
        cmd.try_despawn();
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Spawns terrain, vegetation, and ocean child meshes"
)]
fn spawn_surface_terrain_patch(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    std_materials: &mut Assets<StandardMaterial>,
    unspun_norm: Vec3,
    vis_rad: f32,
    patch_radius: f32,
    comp: &Composition,
    temp_k: f32,
    body_type: BodyType,
    opt_bio: Option<&BiosphereState>,
    opt_vol: Option<&VolatileInventory>,
    surface_g: f32,
    planet_pos: Vec3,
    spin_rot: Quat,
) -> Entity {
    let terrain_mesh = generate_surface_terrain_mesh(
        unspun_norm,
        vis_rad,
        comp,
        temp_k,
        body_type,
        opt_bio,
        patch_radius,
    );
    let terrain_mat = std_materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(0.18, 0.18, 0.20, 1.0),
        perceptual_roughness: 0.86,
        reflectance: 0.24,
        cull_mode: None,
        ..default()
    });

    let veg_mesh = generate_surface_vegetation_mesh(
        unspun_norm,
        vis_rad,
        comp,
        temp_k,
        body_type,
        opt_bio,
        surface_g,
        patch_radius,
    );
    let has_bio =
        opt_bio.is_some_and(|b| b.biomass_coverage_frac > 0.01 || b.habitability_score > 0.10);
    let (veg_emissive, veg_roughness, veg_reflectance) = if has_bio {
        let biolum = opt_bio.map_or(0.0, |b| b.bioluminescence_intensity);
        if biolum > 0.05 {
            (LinearRgba::new(0.25, 0.45, 0.35, 1.0) * biolum, 0.65, 0.40)
        } else {
            (LinearRgba::new(0.04, 0.05, 0.04, 1.0), 0.75, 0.30)
        }
    } else {
        // Barren planetary bodies (Moon, Mercury, Mars, etc.): natural rocky regolith
        // Zero emissive self-illumination, high roughness, low reflectance
        (LinearRgba::BLACK, 0.92, 0.16)
    };

    let veg_mat = std_materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: veg_emissive,
        perceptual_roughness: veg_roughness,
        reflectance: veg_reflectance,
        cull_mode: None,
        ..default()
    });

    let has_water = planet_has_surface_liquids(body_type, comp, temp_k, opt_bio, opt_vol);
    let sea_offset = evaluate_sea_level_offset(body_type, temp_k, comp, vis_rad);
    let opt_water_mesh = if has_water {
        Some(generate_surface_water_mesh_at_offset(
            unspun_norm,
            vis_rad,
            sea_offset,
            patch_radius,
        ))
    } else {
        None
    };

    commands
        .spawn((
            PlanetarySurfaceTerrainRoot,
            Transform::from_translation(planet_pos).with_rotation(spin_rot),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                PlanetarySurfaceTerrainMesh,
                Mesh3d(meshes.add(terrain_mesh)),
                MeshMaterial3d(terrain_mat),
                Transform::default(),
            ));

            parent.spawn((
                PlanetarySurfaceVegetationMesh,
                Mesh3d(meshes.add(veg_mesh)),
                MeshMaterial3d(veg_mat),
                Transform::default(),
            ));

            if let Some(water_m) = opt_water_mesh {
                let is_methane = comp.organics_frac > 0.10 && temp_k < 180.0;
                let base_color = if is_methane {
                    Color::srgba(0.65, 0.42, 0.14, 0.86)
                } else {
                    Color::srgba(0.08, 0.46, 0.70, 0.84)
                };
                let water_mat = std_materials.add(StandardMaterial {
                    base_color,
                    emissive: LinearRgba::new(0.04, 0.14, 0.22, 1.0),
                    perceptual_roughness: 0.06,
                    metallic: 0.18,
                    reflectance: 0.58,
                    alpha_mode: AlphaMode::Blend,
                    cull_mode: None,
                    ..default()
                });

                parent.spawn((
                    PlanetarySurfaceWaterMesh,
                    Mesh3d(meshes.add(water_m)),
                    MeshMaterial3d(water_mat),
                    Transform::default(),
                ));
            }
        })
        .id()
}

/// Synchronizes the 3D high-detail terrain, vegetation, and ocean meshes when the player
/// enters or moves in First-Person Surface & Rover Horizon mode.
#[allow(
    clippy::too_many_arguments,
    reason = "Coordinates 3D terrain assets, materials, mesh generation, and camera lifecycle"
)]
pub fn sync_surface_terrain_system(
    mut commands: Commands,
    player_state: Option<Res<PlayerInteractionState>>,
    camera_query: Query<&PanOrbitCamera>,
    worlds_query: Query<TerrainWorldQueryItem, (Without<SpaceProbe>, Without<PanOrbitCamera>)>,
    probes_query: Query<
        (Entity, &SpaceProbe, &FlightComputer, &SimPosition),
        Without<PanOrbitCamera>,
    >,
    children_query: Query<&Children>,
    mut terrain_state: ResMut<SurfaceTerrainState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    sim_time: Option<Res<SimTime>>,
    config: Res<SimulationConfig>,
) {
    let Ok(camera) = camera_query.single() else {
        return;
    };

    if !camera.surface_horizon_mode {
        if let Some(root_ent) = terrain_state.root_entity.take() {
            despawn_terrain_root(&mut commands, root_ent, &children_query);
        }
        terrain_state.active_host = None;
        return;
    }

    let candidate_target = player_state
        .as_deref()
        .and_then(|ps| ps.selected_entity)
        .or(camera.target_entity);
    let Some((host_ent, unspun_norm)) = resolve_terrain_vantage(
        candidate_target,
        camera.surface_lat_lon,
        &worlds_query,
        &probes_query,
    ) else {
        return;
    };

    let Ok((_, pos, radius, mass, temp, comp, body, opt_spin, opt_bio, opt_vol, opt_t)) =
        worlds_query.get(host_ent)
    else {
        return;
    };

    let vis_rad = opt_t.map_or_else(
        || config.calc_visual_radius_for_type(radius.0, body.body_type),
        |t| t.scale.x,
    );
    let patch_radius = vis_rad * 0.24;

    let needs_rebuild = terrain_state.root_entity.is_none()
        || terrain_state.active_host != Some(host_ent)
        || terrain_state.last_unspun_center.angle_between(unspun_norm) > 0.035;

    let elapsed_years = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);
    let (spin_dir, spin_angle) = compute_planet_spin_frame(opt_spin, elapsed_years);
    let spin_rot = Quat::from_axis_angle(spin_dir.as_vec3(), spin_angle as f32);
    let planet_pos = pos.0.as_vec3();

    if needs_rebuild {
        if let Some(old_root) = terrain_state.root_entity.take() {
            despawn_terrain_root(&mut commands, old_root, &children_query);
        }

        let m_earth = (mass.0 / EARTH_MASS_SOLAR).max(0.01);
        let r_earth = (radius.0 / EARTH_RADIUS_AU).max(0.1);
        let surface_g = (m_earth / (r_earth * r_earth)).clamp(0.15, 3.5) as f32;

        let root = spawn_surface_terrain_patch(
            &mut commands,
            &mut meshes,
            &mut std_materials,
            unspun_norm,
            vis_rad,
            patch_radius,
            comp,
            temp.0 as f32,
            body.body_type,
            opt_bio,
            opt_vol,
            surface_g,
            planet_pos,
            spin_rot,
        );

        terrain_state.root_entity = Some(root);
        terrain_state.active_host = Some(host_ent);
        terrain_state.last_unspun_center = unspun_norm;
    } else if let Some(root_ent) = terrain_state.root_entity {
        commands
            .entity(root_ent)
            .insert(Transform::from_translation(planet_pos).with_rotation(spin_rot));
    }
}

/// Resolves the host planet entity and current surface vantage point, matching `surface_camera`.
fn resolve_terrain_vantage(
    target_ent: Option<Entity>,
    surface_lat_lon: Vec2,
    worlds_query: &Query<TerrainWorldQueryItem, (Without<SpaceProbe>, Without<PanOrbitCamera>)>,
    probes_query: &Query<
        (Entity, &SpaceProbe, &FlightComputer, &SimPosition),
        Without<PanOrbitCamera>,
    >,
) -> Option<(Entity, Vec3)> {
    if let Some(ent) = target_ent {
        if let Ok((_, probe, comp, _)) = probes_query.get(ent) {
            let u = probe
                .landed_unspun_pos
                .unwrap_or(bevy::math::DVec3::Y)
                .as_vec3()
                .normalize_or_zero();
            return Some((comp.target_entity, u));
        }

        if let Ok((.., body, _, _, _, _)) = worlds_query.get(ent) {
            if !body.body_type.is_star_or_remnant() {
                for (_, probe, comp, _) in probes_query.iter() {
                    if comp.target_entity == ent
                        && probe.status == ProbeStatus::Landed
                        && probe.probe_type == ProbeType::Rover
                    {
                        let u = probe
                            .landed_unspun_pos
                            .unwrap_or(bevy::math::DVec3::Y)
                            .as_vec3()
                            .normalize_or_zero();
                        return Some((ent, u));
                    }
                }
                let u = lat_lon_to_unspun_normal(surface_lat_lon.x, surface_lat_lon.y);
                return Some((ent, u));
            }
        }
    }

    for require_landed in [true, false] {
        for (_, probe, comp, _) in probes_query.iter() {
            if probe.probe_type == ProbeType::Rover
                && (!require_landed || probe.status == ProbeStatus::Landed)
                && worlds_query
                    .get(comp.target_entity)
                    .is_ok_and(|(.., b, _, _, _, _)| !b.body_type.is_star_or_remnant())
            {
                let u = probe
                    .landed_unspun_pos
                    .unwrap_or(bevy::math::DVec3::Y)
                    .as_vec3()
                    .normalize_or_zero();
                return Some((comp.target_entity, u));
            }
        }
    }

    for (world_ent, .., body, _, _, _, _) in worlds_query.iter() {
        if !body.body_type.is_star_or_remnant() {
            let u = lat_lon_to_unspun_normal(surface_lat_lon.x, surface_lat_lon.y);
            return Some((world_ent, u));
        }
    }

    None
}
