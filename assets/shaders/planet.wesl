import bevy_pbr::render::{
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_functions::{get_world_from_local, mesh_position_local_to_clip, mesh_position_local_to_world, mesh_normal_local_to_world, mesh_tangent_local_to_world},
    mesh_bindings::mesh,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

import protostellar::rendering::shaders::{
    noise::{
        rotate_y, rotate_z, hash3, noise, fbm, eroded_continent_fbm, fbm_lod,
        eroded_continent_fbm_lod, ridge_noise, vortex_swirl, hash3_vec, voronoi3,
    },
    scattering::{chapman_spherical_airmass, dual_lobe_mie_phase, henyey_greenstein},
    blackbody::{blackbody_temperature_to_rgb, doppler_shift_temperature},
    hypsometry::{
        CraterShadingResult, evaluate_earth_continental_harmonics,
        evaluate_canonical_terrestrial_elevation, compute_crater_radial_slope,
        apply_gas_giant_impact_basin, apply_ice_giant_impact_basin,
        apply_icy_world_impact_basin, apply_rocky_world_impact_basin,
    },
    gas_giants::{PolarHexagonResult, evaluate_polar_hexagon, AnticyclonicVortexResult, evaluate_anticyclonic_vortices},
    cutaway::{CutawayOutput, evaluate_internal_cutaway},
};

struct PlanetExtension {
    planet_type: u32,
    temperature: f32,
    time: f32,
    spin_angle: f32,
    composition: vec4<f32>, // x: rock, y: ice (volatiles/water), z: metal, w: gas (atmosphere)
    color_seed: vec4<f32>,
    climate_and_bio: vec4<f32>, // x: ocean_frac, y: ice_frac, z: biomass_frac, w: cloud_density
    atmosphere_params: vec4<f32>, // x: surface_pressure_bar, y: scale_height, z: haze_density, w: greenhouse_factor
    dynamics_and_mag: vec4<f32>, // x: magnetic_field_gauss, y: lava_fraction, z: storm_intensity, w: axial_tilt_rad
    spin_axis: vec4<f32>, // x, y, z: unit 3D spin axis in world coordinates, w: reserved
    impact_basins_pos: array<vec4<f32>, 4>, // xyz = unit normal in crust space, w = angular radius (rad)
    impact_basins_data: array<vec4<f32>, 4>, // x = melt_glow_fraction, y = elongation, z = rim_height, w = active
    star_dir_and_lum: vec4<f32>, // xyz = unit star direction in world coordinates, w = luminosity factor
    scattering_params: vec4<f32>, // rgb = Rayleigh beta_R, w = Mie forward asymmetry g
    ring_shadow_params: vec4<f32>, // x: inner_ratio, y: outer_ratio, z: optical_depth, w: has_rings (1.0 or 0.0)
    eclipse_moons_pos: array<vec4<f32>, 4>, // xyz = relative moon pos in planet radii, w = moon radius ratio
    eclipse_moons_data: array<vec4<f32>, 4>, // x = active flag (1.0 or 0.0), y = penumbra softness, z = shadow depth, w = reserved
    geological_params: vec4<f32>, // x: age_gyr, y: drift_phase, z: ocean_oxidation, w: vegetation_expansion
    aurora_params: vec4<f32>, // x: oval_colatitude_rad, y: oval_width_rad, z: auroral_intensity, w: geomagnetic_kp_index
    storm_features: vec4<f32>, // x: hex_amplitude, y: hex_wavenumber (e.g. 6.0), z: great_spot_size, w: great_spot_lat_rad
    storm_dynamics: vec4<f32>, // x: great_spot_lon_rad, y: vortex_spin_rate, z: secondary_oval_count, w: zonal_shear_turbulence
    planet_center_and_radius: vec4<f32>, // xyz = planet center in world coordinates, w = visual radius
    civilization_params: vec4<f32>, // x: technosignature (0.0 to 2.0), yzw: flora, biolum, gravity
    tidal_deformation: vec4<f32>, // xyz = unit world vector toward tidal primary, w = roche_stretch (0..0.78) + 2.0 if tidally locked
    cutaway_params: vec4<f32>, // x: cutaway_active (1.0 or 0.0), y: cut_angle, z: core_ratio, w: mantle_ratio
};

@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<uniform> planet: PlanetExtension;

@if(BODY_CLASS_STAR_REMNANT || BODY_CLASS_GIANT)
fn get_vertex_displacement(p_surf: vec3<f32>) -> f32 {
    return 0.0;
}
@else
fn get_vertex_displacement(p_surf: vec3<f32>) -> f32 {
    let p_type = planet.planet_type;
    // Only terrestrial (3), ice worlds (4), super-earths (6), and ice dwarfs (9) get terrain displacement.
    // Small bodies (8) and all others get a flat surface.
    if (p_type != 3u && p_type != 4u && p_type != 6u && p_type != 9u) {
        return 0.0;
    }
    var elev = 0.0;
    var is_ocean_surface = false;
    var active_sea_level = 0.0;
    var clamp_to_sea_level = false;
    let ocean_frac = planet.climate_and_bio.x;
    let has_volatiles = planet.composition.y > 0.001 || ocean_frac > 0.001;
    let pressure_bar = planet.atmosphere_params.x;
    if (p_type == 3u || p_type == 6u) {
        let t_sub = select(0.0, round(planet.color_seed.w), p_type == 3u);
        let is_snowball = p_type == 3u
            && has_volatiles
            && (planet.climate_and_bio.y >= 0.60 || (planet.temperature < 255.0 && (ocean_frac > 0.005 || planet.climate_and_bio.y > 0.10)));
        if (t_sub >= 1.0 && t_sub != 4.0) {
            elev = evaluate_canonical_terrestrial_elevation(p_surf, t_sub);
            let has_water_or_ice_oceans = has_volatiles
                && ocean_frac > 0.01
                && planet.temperature >= 150.0
                && planet.temperature <= 380.0
                && pressure_bar >= 0.006;
            let has_oceans = has_water_or_ice_oceans && !is_snowball;
            let sea_level = clamp(0.18 + ocean_frac * 0.40, 0.18, 0.76);
            if (t_sub == 8.0 && ocean_frac < 0.04 && elev < 0.0) {
                elev = 0.0;
                is_ocean_surface = true;
                active_sea_level = 0.0;
                clamp_to_sea_level = true;
            } else if (has_water_or_ice_oceans) {
                active_sea_level = sea_level;
                clamp_to_sea_level = true;
                if (elev < sea_level) {
                    elev = sea_level + select(0.0, 0.003, is_snowball);
                    is_ocean_surface = !is_snowball;
                }
            }
        } else {
            let drift_phase = planet.spin_axis.w;
            let plate_drift = vec3<f32>(
                sin(drift_phase * 6.283 + p_surf.z * 2.6) * 0.28,
                cos(drift_phase * 3.14159 + p_surf.x * 2.2) * 0.12,
                sin(drift_phase * 4.712 + p_surf.y * 2.4) * 0.28
            );
            let p_tectonic = p_surf + plate_drift;
            let raw_continent_mask = eroded_continent_fbm(p_tectonic * 1.85);
            let earth_harmonics = evaluate_earth_continental_harmonics(p_tectonic);
            let earth_blend = select(0.0, 1.0 - clamp(drift_phase * 2.5, 0.0, 0.85), t_sub == 4.0);
            let continent_mask = mix(
                raw_continent_mask,
                clamp(earth_harmonics + (raw_continent_mask - 0.42) * 0.58, 0.0, 1.0),
                earth_blend
            );
            let regional_hills = fbm(p_tectonic * 5.5) * 0.32;
            let mountain_ridges = ridge_noise(p_tectonic * 8.0) * 0.20;
            let micro_detail = fbm(p_tectonic * 22.0) * 0.10;
            let aggregation = planet.geological_params.y;
            let super_axis = normalize(vec3<f32>(0.65, 0.25, 0.70));
            let cluster_dipole = dot(p_surf, super_axis);
            let cluster_bias = cluster_dipole * (aggregation - 0.20) * 0.14 * (1.0 - earth_blend);

            let land_weight = smoothstep(0.28, 0.48, continent_mask);
            let continental_relief = continent_mask * 0.52
                + (regional_hills + mountain_ridges + micro_detail) * (0.15 + 0.85 * land_weight)
                + cluster_bias;

            // Realistic Oceanic Bathymetry: Spreading Mid-Ocean Ridges, Axial Rifts, and Subduction Trenches
            let oceanic_basin = 1.0 - land_weight;
            let mid_ocean_ridge = ridge_noise(p_tectonic * 6.5 + vec3<f32>(1.2, 3.4, 0.7)) * 0.10;
            let axial_rift = (1.0 - ridge_noise(p_tectonic * 13.0 + vec3<f32>(1.2, 3.4, 0.7))) * 0.025;
            let trench_zone = smoothstep(0.04, 0.0, abs(continent_mask - 0.36) - 0.015);
            let trench_depth = trench_zone * 0.09;

            let has_water_or_ice_oceans = has_volatiles
                && ocean_frac > 0.01
                && planet.temperature <= 380.0
                && (p_type == 6u || planet.temperature >= 150.0)
                && pressure_bar >= 0.006;
            let has_oceans = has_water_or_ice_oceans && !is_snowball;
            let sea_level = select(
                clamp(0.18 + ocean_frac * 0.45, 0.20, 0.80),
                clamp(0.18 + ocean_frac * 0.40, 0.18, 0.76),
                p_type == 3u
            );

            // In oceanic basins, keep floor comfortably below sea_level so ridges never pierce the water
            let ocean_base = sea_level - 0.22;
            let bathymetry_elev = ocean_base + (mid_ocean_ridge - axial_rift) - trench_depth;

            elev = select(
                continental_relief + oceanic_basin * (mid_ocean_ridge - axial_rift) - trench_depth,
                mix(bathymetry_elev, continental_relief, land_weight),
                has_water_or_ice_oceans
            );

            if (has_water_or_ice_oceans) {
                active_sea_level = sea_level;
                clamp_to_sea_level = true;
                if (elev < sea_level) {
                    elev = sea_level + select(0.0, 0.003, is_snowball);
                    is_ocean_surface = !is_snowball;
                }
            }
        }
    } else if (p_type == 4u) {
        let regolith = fbm(p_surf * 14.0);
        let micro_craters = fbm(p_surf * 22.0);
        let boulder_noise = fbm(p_surf * 38.0);
        let variegation = fbm(p_surf * 5.5);
        elev = regolith * 0.3 + micro_craters * 0.15 + boulder_noise * 0.05 + variegation * 0.4;
    } else if (p_type == 9u) {
        let sub = round(planet.color_seed.w);
        if (sub == 1.0) { // Pluto (New Horizons)
            let sp_center = vec3<f32>(1.0, 0.35, 0.0) / length(vec3<f32>(1.0, 0.35, 0.0));
            let d_sp = length(p_surf - sp_center);
            let basin = -0.024 * smoothstep(0.48, 0.16, d_sp);

            let mtn_center = vec3<f32>(0.92, 0.10, -0.36) / length(vec3<f32>(0.92, 0.10, -0.36));
            let d_mtn = length(p_surf - mtn_center);
            let mtn_mask = smoothstep(0.26, 0.04, d_mtn);
            let mtn_relief = (ridge_noise(p_surf * 32.0) * 0.65 + ridge_noise(p_surf * 64.0) * 0.35) * 0.042 * mtn_mask;

            let blade_center = vec3<f32>(0.88, 0.22, 0.42) / length(vec3<f32>(0.88, 0.22, 0.42));
            let d_blade = length(p_surf - blade_center);
            let blade_mask = smoothstep(0.28, 0.05, d_blade);
            let blades = abs(sin(atan2(p_surf.z, p_surf.x) * 44.0)) * 0.018 * blade_mask;

            let crater_relief = (fbm(p_surf * 14.0) - 0.5) * 0.014;
            elev = basin + mtn_relief + blades + crater_relief;
        } else if (sub == 2.0) { // Charon
            let chasma = (1.0 - smoothstep(0.0, 0.06, abs(p_surf.y + 0.05))) * (ridge_noise(p_surf * 18.0) * 0.5 + 0.5);
            let craters = (fbm(p_surf * 16.0) - 0.5) * 0.018;
            elev = craters - chasma * 0.035;
        } else if (sub == 7.0) { // Ceres
            let occ_center = vec3<f32>(0.82, 0.34, 0.45) / length(vec3<f32>(0.82, 0.34, 0.45));
            let d_occ = length(p_surf - occ_center);
            let occ_rim = (smoothstep(0.18, 0.12, d_occ) - smoothstep(0.12, 0.06, d_occ)) * 0.024;
            let occ_floor = -0.016 * smoothstep(0.12, 0.02, d_occ);
            let dome = 0.014 * smoothstep(0.035, 0.0, d_occ);
            elev = (fbm(p_surf * 18.0) - 0.5) * 0.015 + occ_rim + occ_floor + dome;
        } else { // Generic / Eris / Makemake / Sedna
            elev = (fbm(p_surf * 16.0) - 0.5) * 0.016 + ridge_noise(p_surf * 24.0) * 0.010;
        }
    }
    // Evaluate Impact Basins (only on solid land/ice crust; liquid oceans stay at sea-level equipotential)
    if (!is_ocean_surface) {
        for (var i = 0u; i < 4u; i = i + 1u) {
            let b_pos = planet.impact_basins_pos[i];
            let b_data = planet.impact_basins_data[i];
            let scar = clamp(b_data.w, 0.0, 1.0);
            let r_ang = b_pos.w;
            if (scar > 0.02 && r_ang > 0.005) {
                let center_n = normalize(b_pos.xyz);
                let dot_p = dot(p_surf, center_n);
                let dist_ang = acos(clamp(dot_p, -1.0, 1.0));
                if (dist_ang < r_ang * 1.6) {
                    let norm_dist = dist_ang / r_ang;
                    let basin_depth = clamp(r_ang * 1.35, 0.06, 0.28) * scar;
                    if (norm_dist < 1.0) {
                        let floor = -basin_depth * (1.0 - smoothstep(0.7, 1.0, norm_dist));
                        let rim = basin_depth * 0.5 * smoothstep(0.8, 1.0, norm_dist);
                        elev += floor + rim;
                    } else {
                        let ejecta = basin_depth * 0.3 * (1.0 - smoothstep(1.0, 1.6, norm_dist));
                        elev += ejecta;
                    }
                }
            }
        }
        if (clamp_to_sea_level && elev < active_sea_level) {
            elev = active_sea_level;
        }
    }
    return elev;
}



fn get_spun_p_surf(local_normal: vec3<f32>, instance_index: u32) -> vec3<f32> {
    let base_world_normal = normalize(mesh_normal_local_to_world(local_normal, instance_index));
    let surface_dir = base_world_normal;
    
    var s_axis = planet.spin_axis.xyz;
    if (length(s_axis) < 0.1) { s_axis = vec3<f32>(0.0, 1.0, 0.0); } else { s_axis = normalize(s_axis); }
    
    let sin_lat = dot(surface_dir, s_axis);
    let s_ref = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(s_axis.z) > 0.95);
    let s_tangent_x = normalize(cross(s_axis, s_ref));
    let s_tangent_z = cross(s_tangent_x, s_axis);
    let p_tilted = vec3<f32>(dot(surface_dir, s_tangent_x), sin_lat, dot(surface_dir, s_tangent_z));
    
    // Bevy's Quat::from_rotation_y and WGSL's rotate_y have opposite handedness.
    // For planets (which don't physically rotate), this scrolls the texture.
    // For minor bodies (which DO physically rotate), this perfectly cancels the physical rotation,
    // locking the procedural displacement to the mesh!
    return rotate_y(p_tilted, planet.spin_angle);
}


@if(!PREPASS_PIPELINE)
@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    
    let local_n = normalize(vertex.normal);
    let p_surf = get_spun_p_surf(local_n, vertex_in.instance_index);
    let elev = get_vertex_displacement(p_surf);

    let cam_alt_ratio = length(view.world_position - planet.planet_center_and_radius.xyz)
        / max(planet.planet_center_and_radius.w, 1e-6);
    let surface_mode_blend = smoothstep(1.06, 1.38, cam_alt_ratio);

    let displacement_factor = 0.02; // Reduced to 2% to fix jagged shadows and extreme spikes
    var scale = displacement_factor;
    if (planet.planet_type == 4u || planet.planet_type == 9u) { scale = 0.035; }
    scale *= surface_mode_blend;

    var displaced_local_pos = vertex.position + local_n * (elev * scale);

    var world_from_local = get_world_from_local(vertex_in.instance_index);
    let base_world_n = normalize(mesh_normal_local_to_world(local_n, vertex_in.instance_index));

    // --- 1. ROTATIONAL OBLATENESS (Maclaurin Spheroid Squishing for Gas/Ice Giants) ---
    var s_axis = planet.spin_axis.xyz;
    if (length(s_axis) < 0.1) { s_axis = vec3<f32>(0.0, 1.0, 0.0); } else { s_axis = normalize(s_axis); }
    let sin_lat = dot(base_world_n, s_axis);
    let is_giant = (planet.planet_type == 1u || planet.planet_type == 2u);
    let has_saturn_rings = planet.ring_shadow_params.w > 0.5 || planet.storm_features.x > 0.001;
    let oblateness_amount = select(0.065, 0.098, has_saturn_rings);
    let oblateness = select(0.0, oblateness_amount, is_giant);
    let oblate_factor = mix(
        1.0,
        1.0 - oblateness * (1.5 * sin_lat * sin_lat - 0.5),
        surface_mode_blend
    );
    displaced_local_pos *= oblate_factor;

    // Haumea: rapid 3.9-hour rotation flattens it into a scalene triaxial ellipsoid
    if (planet.planet_type == 9u && round(planet.color_seed.w) == 4.0) {
        displaced_local_pos.x *= 1.35;
        displaced_local_pos.z *= 0.95;
        displaced_local_pos.y *= 0.72;
    }

    // --- 2. TIDAL ROCHE TEARDROP DEFORMATION (Stars & Moons Near Black Holes / Giant Planets) ---
    let raw_tidal_w = planet.tidal_deformation.w;
    let roche_stretch = select(raw_tidal_w, raw_tidal_w - 2.0, raw_tidal_w >= 1.5);
    let tidal_dir = planet.tidal_deformation.xyz;
    var deformed_world_n = base_world_n;

    if (roche_stretch > 0.01 && dot(tidal_dir, tidal_dir) > 0.5) {
        let t_dir = normalize(tidal_dir);

        // Project world tidal direction into local mesh space using the world_from_local basis
        let lx = normalize(world_from_local[0].xyz);
        let ly = normalize(world_from_local[1].xyz);
        let lz = normalize(world_from_local[2].xyz);
        let t_local = vec3<f32>(dot(t_dir, lx), dot(t_dir, ly), dot(t_dir, lz));

        let p_par = dot(displaced_local_pos, t_local);
        let p_perp = displaced_local_pos - p_par * t_local;

        // True Affine Prolate Ellipsoid:
        // Volume-preserving: major axis a = 1.0 + roche_stretch, minor axis b = 1.0 / sqrt(1.0 + roche_stretch)
        // Strictly convex everywhere - completely eliminates the artificial dumbbell / peanut waist pinch!
        let a_stretch = 1.0 + roche_stretch;
        let b_compress = 1.0 / sqrt(a_stretch);

        // Asymmetric L1 pointed Roche nozzle cone pointing toward accretor
        let r_mesh = max(length(displaced_local_pos), 0.0001);
        let u_l1 = max(p_par / r_mesh, 0.0);
        let l1_nozzle = t_local * (r_mesh * roche_stretch * 0.35 * u_l1 * u_l1 * u_l1);

        displaced_local_pos = p_perp * b_compress + t_local * (p_par * a_stretch) + l1_nozzle;

        // Transform surface normal to match the ellipsoidal teardrop geometry
        let n_par = dot(base_world_n, t_dir);
        let n_perp = base_world_n - n_par * t_dir;
        let deformed_ellipsoid_n = normalize(n_perp / b_compress + (t_dir * n_par) / a_stretch);
        deformed_world_n = normalize(deformed_ellipsoid_n + t_dir * (roche_stretch * 0.25 * u_l1 * u_l1));
    }

    let local_pos = vec4<f32>(displaced_local_pos, 1.0);
    out.world_position = mesh_position_local_to_world(world_from_local, local_pos);
    out.world_normal = deformed_world_n;
    out.position = mesh_position_local_to_clip(world_from_local, local_pos);
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}







// =========================================================================
// BLACK HOLE STAR (QUASI-STAR / JWST LITTLE RED DOT): PSEUDO-PHOTOSPHERE
// =========================================================================
// Simulates the gargantuan 60 AU hydrogen cocoon powered by central black hole accretion:
// - Multi-scale boiling convective supercells & fine granulation
// - Deep saturated crimson-red & ruby downwelling lanes into burning scarlet rising cores
// - Radiation flow eruption shock plumes & twisting magnetic plasma filament arcs
// - Extended gas cocoon limb darkening & intense glowing coronal limb flare (ragged rim)
// - Convective/radiation pressure equilibrium pulsation
fn render_quasistar_photosphere(
    p_surf: vec3<f32>,
    norm: vec3<f32>,
    view_dir: vec3<f32>,
    t: f32,
) -> vec3<f32> {
    let NdotV = max(dot(norm, view_dir), 0.0);
    
    // 1. Organic domain warping for convective churning
    let warp_dir1 = vec3<f32>(t * 0.08, -t * 0.04, t * 0.06);
    let warp = vec3<f32>(
        fbm(p_surf * 2.5 + warp_dir1),
        fbm(p_surf * 2.5 + warp_dir1.yzx + 4.2),
        fbm(p_surf * 2.5 + warp_dir1.zxy + 8.7)
    ) * 0.35;
    let p_turb = p_surf + warp;
    
    // 2. Multi-scale boiling granulation
    // Primary colossal convective supercells
    let v_super = voronoi3(p_turb * 4.5, t * 0.35);
    let super_lane = smoothstep(0.04, 0.36, v_super.y - v_super.x);
    
    // Secondary fine turbulent granules
    let v_fine = voronoi3(p_turb * 15.0, t * 0.65);
    let fine_lane = smoothstep(0.03, 0.28, v_fine.y - v_fine.x);
    
    let cell_factor = super_lane * 0.65 + fine_lane * 0.35;
    
    // 3. Chromatic color mapping calibrated to deep saturated red sample image
    let col_lane = vec3<f32>(0.26, 0.010, 0.003);   // Deep cool blood-crimson downwelling lanes
    let col_cell = vec3<f32>(0.92, 0.055, 0.006);   // Rich vibrant scarlet-red cell bodies
    let col_core = vec3<f32>(1.12, 0.14, 0.010);   // Saturated burning vermillion centers (keeps green low!)
    
    var color = mix(col_lane, col_cell, cell_factor);
    let core_boost = smoothstep(0.75, 0.15, v_super.x);
    color = mix(color, col_core, core_boost * 0.75);
    
    // 4. Outward Radiation Flow & Accretion Shock Flare Eruptions
    let edd_mult = max(planet.dynamics_and_mag.y, 1.0); // Eddington accretion ratio
    let flare_noise = pow(fbm(p_turb * 7.5 - vec3<f32>(0.0, t * 0.25, t * 0.15)), 2.8);
    let flare_knot_col = vec3<f32>(1.25, 0.22, 0.015); // Fiery scarlet-orange flare knots (no yellow/white desaturation)
    color = mix(color, flare_knot_col, clamp(flare_noise * 1.5 * (edd_mult * 0.35 + 0.65), 0.0, 1.0));
    
    // Twisting magnetic filament arcs & plasma prominence spicules
    let filament = ridge_noise(p_turb * 13.0 + vec3<f32>(t * 0.12, t * 0.08, 0.0));
    if (filament > 0.76) {
        let fil_bright = (filament - 0.76) / 0.24;
        color += vec3<f32>(1.20, 0.15, 0.010) * fil_bright * 1.6;
    }
    
    // 5. Radiation Pressure Equilibrium Breathing (subtle global harmonic oscillation)
    let breathing = 1.0 + 0.04 * sin(t * 0.80);
    color = color * breathing;
    
    // 6. Extended Gas Envelope Limb Darkening
    let limb_dark = pow(NdotV, 0.48);
    color = color * (limb_dark * 0.82 + 0.18);
    
    // 7. Intense Coronal Limb Flare (flaming ragged rim matching reference infographic)
    let rim_grazing = pow(1.0 - NdotV, 3.2);
    let rim_wisps = fbm(p_surf * 28.0 + vec3<f32>(t * 0.55, t * 0.35, 0.0));
    let rim_color = vec3<f32>(1.30, 0.08, 0.008) * (rim_grazing * (0.85 + 0.65 * rim_wisps) * 4.0);
    
    return color * 1.55 + rim_color;
}

// =========================================================================
// UNIVERSAL PROCEDURAL STELLAR PHOTOSPHERE ENGINE (planet_type == 0u)
// =========================================================================
// Distinct astrophysical procedural shading for all 26 stellar classifications:
// 0: Main Sequence Yellow Dwarf (G2V Sun)
// 1: Red Dwarf (M-type TRAPPIST-1 / Proxima)
// 2: Brown Dwarf (L/T-type Sub-Stellar)
// 3: Red Giant & Red Supergiant (Betelgeuse)
// 4: Blue Main Sequence, Blue Giant, Supergiant & Hypergiant (O/B-type)
// 5: Neutron Star
// 6: Pulsar
// 7: Magnetar
// 8: White Dwarf (DA/DB Degenerate C-O)
// 9: Protostar & T Tauri Star (Hayashi Track PMS)
// 10: Wolf-Rayet Star (Stripped He/C Core)
// 11: Dark Star (WIMP Annihilation Puffy Halo)
// 12: Boson Star (Transparent Scalar Field Soliton)
// 13: Orange Dwarf (K-type Main Sequence)
// 14: Yellow-White Dwarf (F-type Procyon)
// 15: White Main Sequence (A-type Sirius / Vega)
// 16: Herbig Ae/Be Star (Intermediate-Mass PMS)
// 17: Yellow Hypergiant (Rho Cassiopeiae)
// 18: Carbon Star (Soot-Shrouded AGB Ruby Giant)
// 19: Luminous Blue Variable (Eruptive Eta Carinae)
// 20: Population III Hypergiant (Primordial Z=0 Furnace)
// 21: Blue Straggler (Rejuvenated Merger Star)
// 22: Quark Star (Bare Strange-Quark Matter Surface)
// 23: Thorne-Zytkow Object (Red Supergiant + Neutron Core)
// 24: Black Dwarf (Cooled Crystallized C-O Diamond)
// 25: Iron Star (Cold-Fusion Fe-56 Relic)
fn render_stellar_photosphere(
    p_surf: vec3<f32>,
    norm: vec3<f32>,
    view_dir: vec3<f32>,
    t: f32,
) -> vec4<f32> {
    let subtype = u32(planet.composition.x + 0.5);
    let cell_scale = max(planet.composition.y, 4.0);
    let flare_int = planet.composition.z;
    let pulse_freq = planet.composition.w;
    let seed_col = planet.color_seed.rgb;
    let NdotV = max(dot(norm, view_dir), 0.0);
    
    var final_col = seed_col;
    var final_alpha = 1.0;
    
    // =========================================================================
    // 0. Main Sequence Yellow Dwarf (The Sun)
    // =========================================================================
    if (subtype == 0u) {
        // Organic domain warp to break straight Voronoi polygon cell boundaries (no soccer-ball / honeycomb lines)
        let warp = vec3<f32>(
            fbm(p_surf * 3.5 + vec3<f32>(0.0, t * 0.008, 0.0)),
            fbm(p_surf * 3.5 + vec3<f32>(4.3, 0.0, t * 0.008)),
            fbm(p_surf * 3.5 + vec3<f32>(0.0, 8.7, t * 0.008))
        ) * 0.16;
        let p_gran = p_surf + warp;

        // 1. Slower, stately motion: real granules evolve over ~8-15 minutes (gentle crawl, not boiling soup)
        let v_big = voronoi3(p_gran * (cell_scale * 0.60), t * 0.08);
        let v_fine = voronoi3(p_gran * (cell_scale * 1.40), t * 0.20);

        // 2. Soft, organic convective cell boundaries and rising 3D mounds
        // Narrow, well-defined cool downwelling intergranular lanes
        let lane_big = smoothstep(0.02, 0.30, v_big.y - v_big.x);
        let lane_fine = smoothstep(0.02, 0.24, v_fine.y - v_fine.x);
        let lane = lane_big * 0.65 + lane_fine * 0.35;

        // Hot upwelling convective mound centers
        let mound_big = smoothstep(0.65, 0.15, v_big.x);
        let mound_fine = smoothstep(0.55, 0.12, v_fine.x);
        let mound = mound_big * 0.65 + mound_fine * 0.35;

        // 3. Warm cream/gold solar photosphere palette (5778 K blackbody)
        // Deep warm amber downwelling lanes into rich solar cream-gold body into incandescent core
        let dark_lane = vec3<f32>(0.62, 0.42, 0.16);
        let gran_body = vec3<f32>(1.04, 0.88, 0.52);
        let bright_core = vec3<f32>(1.26, 1.14, 0.82);

        var gran_col = mix(dark_lane, gran_body, lane);
        gran_col = mix(gran_col, bright_core, mound * 0.65);

        // Subtle micro-cellular turbulent mottling across convective mounds
        let micro = fbm(p_gran * (cell_scale * 3.0) + vec3<f32>(0.0, t * 0.02, 0.0));
        gran_col *= (0.95 + micro * 0.10);

        // 4. Distance-based detail (Screen-space derivative LOD):
        // Only blends high-frequency granulation away when cells become sub-pixel in distant system view.
        // In close-up (pixel_cell_deriv < 0.40), lod is 0.0, keeping full crisp, readable granulation.
        let pixel_cell_deriv = length(fwidth(p_gran * cell_scale));
        let lod = smoothstep(0.40, 1.15, pixel_cell_deriv);
        let system_view_disk = vec3<f32>(1.06, 0.90, 0.55);
        gran_col = mix(gran_col, system_view_disk, lod * 0.90);

        // 5. Sunspot groups with dark umbra, striated penumbra, and bright magnetic faculae plages
        let spot_noise = fbm(p_surf * 4.6);
        let spot_coverage = select(0.08, planet.atmosphere_params.z, planet.atmosphere_params.z > 0.001);
        let spot_thresh = 0.74 - spot_coverage * 0.50;

        if (spot_noise > spot_thresh) {
            let penumbra = smoothstep(spot_thresh, spot_thresh + 0.08, spot_noise);
            let umbra = smoothstep(spot_thresh + 0.08, spot_thresh + 0.16, spot_noise);
            let umbra_col = vec3<f32>(0.07, 0.04, 0.015);
            let penumbra_col = vec3<f32>(0.46, 0.30, 0.12);
            gran_col = mix(gran_col, penumbra_col, penumbra * 0.90);
            gran_col = mix(gran_col, umbra_col, umbra * 0.96);
        } else if (spot_noise > spot_thresh - 0.12) {
            // Brilliant magnetic faculae / plages (hotter than photosphere, contrast increases toward limb)
            let faculae = smoothstep(spot_thresh - 0.12, spot_thresh, spot_noise);
            let faculae_boost = faculae * (1.15 - 0.35 * NdotV);
            gran_col += vec3<f32>(0.38, 0.32, 0.16) * faculae_boost;
        }

        // Restrained coronal micro-flares localized strictly near active sunspot regions
        if (spot_noise > spot_thresh - 0.05 && flare_int > 0.03) {
            let flare_glow = (spot_noise - (spot_thresh - 0.05)) * flare_int * 2.2;
            gran_col += vec3<f32>(1.5, 1.2, 0.6) * flare_glow;
        }

        // 6. Quadratic Eddington solar limb darkening (center bright, limb smoothly darker)
        let limb_u = 0.64;
        let limb_v = 0.22;
        let limb_dark = 1.0 - limb_u * (1.0 - NdotV) - limb_v * (1.0 - sqrt(max(NdotV, 0.0)));
        gran_col *= clamp(limb_dark, 0.20, 1.0);

        // 7. Soft glowing chromospheric grazing fringe (soft luminous rim, eliminates hard cutout circle)
        let rim_grazing = pow(1.0 - NdotV, 3.8);
        let fringe = vec3<f32>(1.25, 0.85, 0.30) * (rim_grazing * 1.6);

        final_col = gran_col * 2.85 + fringe;
    }
    // =========================================================================
    // 1. Red Dwarf (M-Star / TRAPPIST-1 / Proxima)
    // =========================================================================
    else if (subtype == 1u) {
        // Fully-convective churning: organic domain-warped turbulent cells
        let warp = vec3<f32>(
            fbm(p_surf * 3.2 + vec3<f32>(0.0, t * 0.008, 0.0)),
            fbm(p_surf * 3.2 + vec3<f32>(3.1, 0.0, t * 0.008)),
            fbm(p_surf * 3.2 + vec3<f32>(0.0, 6.2, t * 0.008))
        ) * 0.18;
        let p_cell = p_surf + warp;

        let v = voronoi3(p_cell * cell_scale, t * 0.12);
        let lane = smoothstep(0.03, 0.30, v.y - v.x);
        let dark_lane = vec3<f32>(0.42, 0.08, 0.02);
        let cell_body = vec3<f32>(1.02, 0.36, 0.08);
        let cell_core = vec3<f32>(1.22, 0.62, 0.16);
        var gran_col = mix(dark_lane, cell_body, lane);
        gran_col = mix(gran_col, cell_core, smoothstep(0.65, 0.15, v.x) * 0.6);
        
        // Enormous starspot coverage (15-35%)
        let spot_noise = fbm(p_surf * 3.8);
        if (spot_noise > 0.52) {
            let spot_mask = smoothstep(0.52, 0.64, spot_noise);
            let starspot_col = vec3<f32>(0.14, 0.02, 0.01);
            gran_col = mix(gran_col, starspot_col, spot_mask * 0.90);
        }
        
        // Violent magnetic flare eruptions (explosive white-blue/gold flash knots)
        let flare_wave = sin(t * 1.8 + p_surf.x * 6.0) * 0.5 + 0.5;
        let flare_knot = pow(fbm(p_surf * 8.5 + vec3<f32>(0.0, t * 0.2, 0.0)), 3.5);
        if (flare_knot > 0.45 && flare_wave > 0.65) {
            let flare_amp = (flare_knot - 0.45) * 4.0;
            gran_col += vec3<f32>(1.5, 1.3, 0.9) * flare_amp * flare_int;
        }
        
        let limb_dark = pow(NdotV, 0.45);
        gran_col *= (limb_dark * 0.75 + 0.25);
        let rim = pow(1.0 - NdotV, 3.5) * vec3<f32>(1.1, 0.25, 0.04) * 2.0;
        final_col = gran_col * 2.6 + rim;
    }
    // =========================================================================
    // 2. Brown Dwarf (Sub-Stellar Transition World)
    // =========================================================================
    else if (subtype == 2u) {
        // Plum-maroon base with differential latitudinal iron/silicate cloud bands
        let lat = norm.y;
        let jet = sin(lat * 12.0) * (t * 0.08);
        let p_band = rotate_y(p_surf, jet);
        let band_val = sin(lat * 10.0 + fbm(p_band * 5.0) * 2.0) * 0.5 + 0.5;
        
        let deep_plum = vec3<f32>(0.42, 0.10, 0.28);
        let dark_maroon = vec3<f32>(0.58, 0.16, 0.36);
        var base_dwarf = mix(deep_plum, dark_maroon, band_val);
        
        // Glowing thermal infrared convective rift fissures
        let fissure = ridge_noise(p_surf * cell_scale + vec3<f32>(0.0, t * 0.03, 0.0));
        if (fissure > 0.65) {
            let fissure_amp = (fissure - 0.65) / 0.35;
            let thermal_glow = vec3<f32>(1.15, 0.45, 0.08) * fissure_amp * 2.2;
            base_dwarf += thermal_glow;
        }
        
        let limb = pow(NdotV, 0.60);
        final_col = base_dwarf * (limb * 0.8 + 0.2) * 2.2;
    }
    // =========================================================================
    // 3. Red Giant & Red Supergiant (Betelgeuse)
    // =========================================================================
    else if (subtype == 3u) {
        // Colossal, sluggish convective supergranules
        let warp = vec3<f32>(fbm(p_surf * 2.0), fbm(p_surf * 2.0 + 3.1), fbm(p_surf * 2.0 + 6.2)) * 0.4;
        let v = voronoi3((p_surf + warp) * cell_scale, t * 0.15);
        let lane = smoothstep(0.05, 0.38, v.y - v.x);
        
        let deep_garnet = vec3<f32>(0.42, 0.05, 0.02);
        let vermillion = vec3<f32>(0.96, 0.34, 0.08);
        let hot_amber = vec3<f32>(1.25, 0.65, 0.14);
        var giant_col = mix(deep_garnet, vermillion, lane);
        giant_col = mix(giant_col, hot_amber, smoothstep(0.80, 0.20, v.x) * 0.65);
        
        // Irregular cool dust obscuration patches
        let dust_obscuration = fbm(p_surf * 3.5 + vec3<f32>(t * 0.02, 0.0, 0.0));
        if (dust_obscuration > 0.58) {
            giant_col *= (1.0 - (dust_obscuration - 0.58) * 1.4);
        }
        
        // Slow pulsation breathing
        let pulsation = 1.0 + 0.06 * sin(t * 0.45);
        giant_col *= pulsation;
        
        let limb_dark = pow(NdotV, 0.50);
        let rim = pow(1.0 - NdotV, 3.0) * vec3<f32>(1.2, 0.30, 0.05) * 3.0;
        final_col = giant_col * (limb_dark * 0.8 + 0.2) * 2.8 + rim;
    }
    // =========================================================================
    // 4. Blue Hyper Giant & Supergiant (O-type / Pop-III)
    // =========================================================================
    else if (subtype == 4u) {
        // High-energy radiation-driven supersonic plasma ripples
        let ripple1 = sin(dot(p_surf, vec3<f32>(14.0, 10.0, 16.0)) + t * 3.5);
        let ripple2 = cos(dot(p_surf, vec3<f32>(-12.0, 15.0, 9.0)) - t * 2.8);
        let turbulence = fbm(p_surf * 18.0 + vec3<f32>(t * 0.4, 0.0, 0.0));
        let wave = (ripple1 * 0.35 + ripple2 * 0.35 + turbulence * 0.30) * 0.5 + 0.5;
        
        let deep_cyan = vec3<f32>(0.65, 0.82, 1.35);
        let diamond_white = vec3<f32>(1.15, 1.25, 1.65);
        var blue_col = mix(deep_cyan, diamond_white, wave);
        
        // Equatorial gravity darkening from rapid rotation (poles hotter/brighter)
        let gravity_dark = 0.80 + 0.40 * abs(norm.y);
        blue_col *= gravity_dark;
        
        // Intense electric blue coronal limb halo
        let rim = pow(1.0 - NdotV, 2.5) * vec3<f32>(0.5, 0.8, 1.6) * 4.0;
        final_col = blue_col * 3.2 + rim;
    }
    // =========================================================================
    // 5. Neutron Star
    // =========================================================================
    else if (subtype == 5u) {
        // Ultra-dense degenerate star: extreme gravitational limb darkening
        let limb = pow(NdotV, 0.28);
        let base_ns = vec3<f32>(1.15, 1.35, 1.95);
        
        // Crystalline nuclear crust with glowing quantum magnetic stress fractures
        let crust = ridge_noise(p_surf * cell_scale);
        var crust_glow = vec3<f32>(0.0);
        if (crust > 0.70) {
            crust_glow = vec3<f32>(0.4, 0.8, 1.8) * ((crust - 0.70) / 0.30) * 2.0;
        }
        
        let rim = pow(1.0 - NdotV, 1.8) * vec3<f32>(0.8, 1.2, 2.4) * 3.5;
        final_col = (base_ns * limb + crust_glow) * 3.5 + rim;
    }
    // =========================================================================
    // 6. Pulsar (Rapidly Spinning Magnetized Neutron Star)
    // =========================================================================
    else if (subtype == 6u) {
        let base_core = vec3<f32>(1.10, 1.30, 1.85);
        let pole_align = abs(norm.y);
        
        // Dual magnetic polar hot spots
        let polar_cap = smoothstep(0.82, 0.98, pole_align);
        let polar_hotspot = vec3<f32>(2.4, 2.6, 3.8) * polar_cap;
        
        // Periodic pulse modulation synchronized with rotation
        let pulse = 0.75 + 0.50 * pow(sin(t * pulse_freq + p_surf.x * 3.14159), 6.0);
        
        // Relativistic synchrotron auroral rings circling magnetic poles
        let ring_angle = abs(pole_align - 0.75);
        var auroral_ring = vec3<f32>(0.0);
        if (ring_angle < 0.08) {
            auroral_ring = vec3<f32>(0.3, 0.9, 1.6) * (1.0 - ring_angle / 0.08) * 2.5;
        }
        
        let rim = pow(1.0 - NdotV, 2.2) * vec3<f32>(0.7, 1.0, 2.2) * 3.0;
        final_col = (base_core * pulse + polar_hotspot + auroral_ring) * 3.2 + rim;
    }
    // =========================================================================
    // 7. Magnetar (Ultra-Magnetized Neutron Star: 10^14 - 10^15 G)
    // =========================================================================
    else if (subtype == 7u) {
        let deep_violet = vec3<f32>(0.85, 0.55, 1.75);
        
        // Starquake crustal fracture web (glowing neon-cyan fissures)
        let crack = ridge_noise(p_surf * cell_scale + vec3<f32>(0.0, sin(t * 0.8) * 0.1, 0.0));
        var starquake = vec3<f32>(0.0);
        if (crack > 0.62) {
            let crack_amp = (crack - 0.62) / 0.38;
            starquake = vec3<f32>(0.3, 1.4, 1.8) * crack_amp * 3.5;
        }
        
        // Magnetic reconnection flare nodes along fracture lines
        let flare_node = pow(fbm(p_surf * 12.0 + vec3<f32>(t * 0.5, 0.0, 0.0)), 4.0);
        if (flare_node > 0.35) {
            starquake += vec3<f32>(2.0, 2.2, 3.2) * (flare_node - 0.35) * 5.0;
        }
        
        let rim = pow(1.0 - NdotV, 2.0) * vec3<f32>(1.2, 0.6, 2.4) * 4.0;
        final_col = (deep_violet + starquake) * 3.4 + rim;
    }
    // =========================================================================
    // 8. White Dwarf
    // =========================================================================
    else if (subtype == 8u) {
        // Degenerate matter: brilliant diamond blue-white, micro-scale shallow ripples
        let v = voronoi3(p_surf * cell_scale, t * 0.40);
        let lane = smoothstep(0.02, 0.20, v.y - v.x);
        let wd_dark = vec3<f32>(1.05, 1.15, 1.45);
        let wd_bright = vec3<f32>(1.30, 1.40, 1.75);
        let micro_gran = mix(wd_dark, wd_bright, lane);
        
        let limb = pow(NdotV, 0.38);
        let rim = pow(1.0 - NdotV, 2.5) * vec3<f32>(0.7, 0.9, 1.6) * 2.5;
        final_col = micro_gran * (limb * 0.85 + 0.15) * 3.2 + rim;
    }
    // =========================================================================
    // 9. Protostar (T Tauri / Pre-Main-Sequence Fully-Convective Star)
    // =========================================================================
    else if (subtype == 9u) {
        // 1. Organic domain warping: breaks polygon boundaries, turning geometric soccer-ball mesh
        // into turbulent, fluid, convective plasma swirls and giant Hayashi supergranules.
        let warp = vec3<f32>(
            fbm(p_surf * 2.8 + vec3<f32>(0.0, t * 0.006, 0.0)),
            fbm(p_surf * 2.8 + vec3<f32>(3.7, 0.0, t * 0.006)),
            fbm(p_surf * 2.8 + vec3<f32>(0.0, 7.4, t * 0.006))
        ) * 0.22;
        let p_proto = p_surf + warp;

        // 2. Colossal Hayashi-track convective super-cells with secondary turbulent granules
        let v_macro = voronoi3(p_proto * (cell_scale * 0.55), t * 0.06);
        let v_micro = voronoi3(p_proto * (cell_scale * 1.25), t * 0.15);

        let lane_macro = smoothstep(0.02, 0.32, v_macro.y - v_macro.x);
        let lane_micro = smoothstep(0.02, 0.25, v_micro.y - v_micro.x);
        let lane = lane_macro * 0.65 + lane_micro * 0.35;

        let mound_macro = smoothstep(0.68, 0.14, v_macro.x);
        let mound_micro = smoothstep(0.58, 0.12, v_micro.x);
        let mound = mound_macro * 0.65 + mound_micro * 0.35;

        // 3. Deep pre-main-sequence blackbody palette (~3,800 - 4,500 K)
        // Deep glowing bronze-amber lanes -> warm fiery gold-amber body -> incandescent core
        let dark_lane = vec3<f32>(0.56, 0.24, 0.07);
        let cell_body = vec3<f32>(1.08, 0.66, 0.22);
        let cell_core = vec3<f32>(1.32, 0.98, 0.48);

        var proto_col = mix(dark_lane, cell_body, lane);
        proto_col = mix(proto_col, cell_core, mound * 0.60);

        // Turbulent mottling across plasma mounds
        let micro_turb = fbm(p_proto * (cell_scale * 2.6) + vec3<f32>(0.0, t * 0.02, 0.0));
        proto_col *= (0.94 + micro_turb * 0.12);

        // 4. Distance-based Screen-Space Derivative LOD (eliminates aliasing in orbital view)
        let pixel_deriv = length(fwidth(p_proto * cell_scale));
        let lod = smoothstep(0.40, 1.15, pixel_deriv);
        let system_view = vec3<f32>(1.05, 0.68, 0.24);
        proto_col = mix(proto_col, system_view, lod * 0.88);

        // 5. Infalling circumstellar dust filaments (dense infalling accretion streamers)
        let dust_vein = ridge_noise(p_surf * 5.2 + vec3<f32>(0.0, t * 0.04, 0.0));
        if (dust_vein > 0.66) {
            let veil = smoothstep(0.66, 0.88, dust_vein);
            let cool_soot = vec3<f32>(0.12, 0.04, 0.015);
            proto_col = mix(proto_col, cool_soot, veil * 0.85);
        }

        // 6. Magnetic accretion shock hot spots (where funneling gas slams into surface)
        let shock_noise = fbm(p_surf * 6.5 + vec3<f32>(t * 0.08, 0.0, 0.0));
        if (shock_noise > 0.72) {
            let shock_amp = smoothstep(0.72, 0.88, shock_noise);
            let shock_glow = vec3<f32>(1.60, 1.35, 0.90) * shock_amp * (flare_int * 1.5 + 0.5);
            proto_col += shock_glow;
        }

        // 7. Quadratic limb darkening & warm chromospheric grazing fringe
        let limb_u = 0.58;
        let limb_v = 0.26;
        let limb_dark = 1.0 - limb_u * (1.0 - NdotV) - limb_v * (1.0 - sqrt(max(NdotV, 0.0)));
        proto_col *= clamp(limb_dark, 0.22, 1.0);

        let rim_grazing = pow(1.0 - NdotV, 3.6);
        let fringe = vec3<f32>(1.25, 0.65, 0.18) * (rim_grazing * 1.7);

        final_col = proto_col * 2.75 + fringe;
    }
    // =========================================================================
    // 10. Wolf-Rayet Star (Stripped Helium/Carbon Core: WR 102 / WR 124)
    // =========================================================================
    else if (subtype == 10u) {
        // Logarithmic spiral colliding wind shock fronts + clumpy He II / C IV emission knots
        let azim = atan2(p_surf.z, p_surf.x);
        let cyl_r = length(p_surf.xz);
        let spiral_phase = azim * 2.0 + cyl_r * 9.5 - t * 1.4;
        let wind_flow = fbm(p_surf * cell_scale + vec3<f32>(t * 0.7, t * 0.35, 0.0));
        let wind_wave = sin(norm.y * 14.0 + spiral_phase + wind_flow * 4.5) * 0.5 + 0.5;
        let clump_knot = smoothstep(0.58, 0.86, ridge_noise(p_surf * (cell_scale * 0.75) - vec3<f32>(0.0, t * 0.45, 0.0)));

        let wr_cyan = vec3<f32>(0.45, 0.90, 1.48);
        let wr_magenta = vec3<f32>(1.35, 0.42, 1.05);
        let wr_core = vec3<f32>(1.65, 1.35, 1.95);
        var wr_col = mix(wr_magenta, wr_cyan, wind_wave);
        wr_col = mix(wr_col, wr_core, clump_knot * 0.65);

        let pixel_deriv = length(fwidth(p_surf * cell_scale));
        let lod = smoothstep(0.45, 1.25, pixel_deriv);
        wr_col = mix(wr_col, vec3<f32>(1.05, 0.68, 1.35), lod * 0.75);

        let rim = pow(1.0 - NdotV, 2.1) * vec3<f32>(1.25, 0.65, 1.65) * 4.2;
        final_col = wr_col * 3.25 + rim;
    }
    // =========================================================================
    // 11. Dark Star (Early-Universe WIMP Dark Matter Annihilation Star)
    // =========================================================================
    else if (subtype == 11u) {
        // Diffuse 5-10 AU golden-amber envelope heated volumetrically by WIMP self-annihilation
        let p_outer = p_surf * (cell_scale * 0.75) + vec3<f32>(0.0, t * 0.04, 0.0);
        let p_inner = (p_surf - view_dir * 0.22) * (cell_scale * 1.15) - vec3<f32>(t * 0.06, 0.0, 0.0);

        let outer_cloud = fbm(p_outer);
        let inner_caustic = ridge_noise(p_inner);
        let annih_pulse = 0.92 + 0.14 * sin(t * 0.8 + inner_caustic * 5.0);

        let warm_peach = vec3<f32>(0.82, 0.36, 0.08);
        let golden_amber = vec3<f32>(1.28, 0.74, 0.22);
        let wimp_caustic = vec3<f32>(1.55, 1.15, 0.72);
        let violet_annih = vec3<f32>(1.15, 0.65, 1.45);

        var ds_col = mix(warm_peach, golden_amber, outer_cloud);
        ds_col = mix(ds_col, wimp_caustic, smoothstep(0.52, 0.85, inner_caustic) * 0.75);
        ds_col += violet_annih * smoothstep(0.74, 0.95, inner_caustic) * 0.65;
        ds_col *= annih_pulse;

        // Volumetric core brightening (brightest along central line of sight through the WIMP halo)
        let core_depth = pow(NdotV, 0.65);
        let halo_rim = pow(1.0 - NdotV, 2.6) * vec3<f32>(1.30, 0.68, 0.20) * 2.2;
        final_col = ds_col * (core_depth * 0.85 + 0.25) * 2.65 + halo_rim;
        final_alpha = clamp(pow(NdotV, 0.45) * 0.88 + 0.12, 0.18, 0.96);
    }
    // =========================================================================
    // 12. Boson Star (Transparent Scalar-Field Gravitational Soliton)
    // =========================================================================
    else if (subtype == 12u) {
        // Surface-free Bose-Einstein scalar condensate with spherical Klein-Gordon standing waves
        // and an internal Einstein gravitational lensing ring.
        let b = sqrt(max(1.0 - NdotV * NdotV, 0.0));
        let soliton_core = exp(-b * b * 6.8);

        // Concentric scalar wave-function interference shells |Phi(r, t)|^2
        let harmonic = fbm(p_surf * 6.0 + vec3<f32>(t * 0.18, -t * 0.14, 0.0));
        let wave_shells = pow(sin(b * 20.0 - t * 2.2 + harmonic * 2.5) * 0.5 + 0.5, 2.4) * exp(-b * 1.4);

        // Internal Einstein photon-deflection ring at critical impact parameter b ~ 0.68
        let einstein_ring = exp(-pow((b - 0.68) / 0.065, 2.0));
        let chromatic_inner = exp(-pow((b - 0.64) / 0.055, 2.0));
        let chromatic_outer = exp(-pow((b - 0.72) / 0.055, 2.0));

        let core_cyan = vec3<f32>(0.75, 1.55, 1.90);
        let shell_aqua = vec3<f32>(0.28, 1.05, 1.38);
        let lens_dispersion = vec3<f32>(
            chromatic_outer * 1.15 + einstein_ring * 0.85,
            einstein_ring * 1.55,
            chromatic_inner * 1.65 + einstein_ring * 1.25
        );

        final_col = (core_cyan * soliton_core * 1.85 + shell_aqua * wave_shells * 1.25 + lens_dispersion * 1.75) * 2.1;
        final_alpha = clamp(soliton_core * 0.76 + einstein_ring * 0.68 + wave_shells * 0.30 + 0.08, 0.10, 0.90);
    }
    // =========================================================================
    // 13. Orange Dwarf (K-Type Main Sequence Star: Alpha Centauri B / Eps Eri)
    // =========================================================================
    else if (subtype == 13u) {
        let warp = vec3<f32>(
            fbm(p_surf * 3.4 + vec3<f32>(0.0, t * 0.008, 0.0)),
            fbm(p_surf * 3.4 + vec3<f32>(3.8, 0.0, t * 0.008)),
            fbm(p_surf * 3.4 + vec3<f32>(0.0, 7.6, t * 0.008))
        ) * 0.16;
        let p_gran = p_surf + warp;

        let v_big = voronoi3(p_gran * (cell_scale * 0.70), t * 0.09);
        let v_fine = voronoi3(p_gran * (cell_scale * 1.55), t * 0.22);
        let lane = smoothstep(0.02, 0.28, v_big.y - v_big.x) * 0.65
                 + smoothstep(0.02, 0.22, v_fine.y - v_fine.x) * 0.35;
        let mound = smoothstep(0.62, 0.14, v_big.x) * 0.65
                  + smoothstep(0.52, 0.12, v_fine.x) * 0.35;

        // 4500 K K2V orange-tangerine blackbody palette
        let dark_lane = vec3<f32>(0.54, 0.22, 0.06);
        let gran_body = vec3<f32>(1.08, 0.60, 0.18);
        let bright_core = vec3<f32>(1.30, 0.88, 0.40);

        var k_col = mix(dark_lane, gran_body, lane);
        k_col = mix(k_col, bright_core, mound * 0.62);

        let pixel_deriv = length(fwidth(p_gran * cell_scale));
        let lod = smoothstep(0.40, 1.15, pixel_deriv);
        k_col = mix(k_col, vec3<f32>(1.08, 0.62, 0.20), lod * 0.88);

        // K-star active spot belt & Ca II H/K chromospheric plages
        let spot_noise = fbm(p_surf * 4.2);
        let spot_cov = select(0.14, planet.atmosphere_params.z, planet.atmosphere_params.z > 0.001);
        let spot_thresh = 0.72 - spot_cov * 0.50;
        if (spot_noise > spot_thresh) {
            let penumbra = smoothstep(spot_thresh, spot_thresh + 0.08, spot_noise);
            let umbra = smoothstep(spot_thresh + 0.08, spot_thresh + 0.16, spot_noise);
            k_col = mix(k_col, vec3<f32>(0.42, 0.16, 0.04), penumbra * 0.88);
            k_col = mix(k_col, vec3<f32>(0.09, 0.025, 0.008), umbra * 0.95);
        } else if (spot_noise > spot_thresh - 0.12) {
            let faculae = smoothstep(spot_thresh - 0.12, spot_thresh, spot_noise) * (1.15 - 0.35 * NdotV);
            k_col += vec3<f32>(0.42, 0.26, 0.10) * faculae;
        }

        let limb_dark = 1.0 - 0.72 * (1.0 - NdotV) - 0.18 * (1.0 - sqrt(max(NdotV, 0.0)));
        k_col *= clamp(limb_dark, 0.18, 1.0);
        let fringe = vec3<f32>(1.28, 0.56, 0.14) * (pow(1.0 - NdotV, 3.6) * 1.75);
        final_col = k_col * 2.75 + fringe;
    }
    // =========================================================================
    // 14. Yellow-White Dwarf (F-Type Main Sequence Star: Procyon A, 6800 K)
    // =========================================================================
    else if (subtype == 14u) {
        // Vigorous, shallow, high-velocity convective granulation with bright magnetic filigree
        let warp = vec3<f32>(
            fbm(p_surf * 4.0 + vec3<f32>(0.0, t * 0.012, 0.0)),
            fbm(p_surf * 4.0 + vec3<f32>(4.1, 0.0, t * 0.012)),
            fbm(p_surf * 4.0 + vec3<f32>(0.0, 8.2, t * 0.012))
        ) * 0.18;
        let p_gran = p_surf + warp;

        let v_big = voronoi3(p_gran * (cell_scale * 0.65), t * 0.14);
        let v_fine = voronoi3(p_gran * (cell_scale * 1.50), t * 0.30);
        let lane = smoothstep(0.02, 0.26, v_big.y - v_big.x) * 0.60
                 + smoothstep(0.02, 0.22, v_fine.y - v_fine.x) * 0.40;
        let mound = smoothstep(0.60, 0.12, v_big.x) * 0.65
                  + smoothstep(0.50, 0.10, v_fine.x) * 0.35;

        let warm_lane = vec3<f32>(0.76, 0.70, 0.50);
        let ivory_body = vec3<f32>(1.18, 1.15, 0.98);
        let dazzling_peak = vec3<f32>(1.42, 1.40, 1.28);

        var f_col = mix(warm_lane, ivory_body, lane);
        f_col = mix(f_col, dazzling_peak, mound * 0.68);

        // Bright intergranular magnetic filigree network
        let filigree = smoothstep(0.78, 0.94, ridge_noise(p_gran * (cell_scale * 1.2)));
        f_col += vec3<f32>(0.28, 0.30, 0.34) * filigree * (1.1 - 0.3 * NdotV);

        let pixel_deriv = length(fwidth(p_gran * cell_scale));
        let lod = smoothstep(0.40, 1.15, pixel_deriv);
        f_col = mix(f_col, vec3<f32>(1.20, 1.16, 1.00), lod * 0.88);

        let limb_dark = 1.0 - 0.52 * (1.0 - NdotV) - 0.20 * (1.0 - sqrt(max(NdotV, 0.0)));
        f_col *= clamp(limb_dark, 0.28, 1.0);
        let fringe = vec3<f32>(1.30, 1.25, 1.08) * (pow(1.0 - NdotV, 3.4) * 1.85);
        final_col = f_col * 2.95 + fringe;
    }
    // =========================================================================
    // 15. White Main Sequence (A-Type Star: Sirius A / Vega, 9500 K)
    // =========================================================================
    else if (subtype == 15u) {
        // Pure radiative envelope: dazzling diamond-white with rapid-rotation von Zeipel
        // equatorial gravity darkening and non-radial p-mode acoustic shimmer ripples.
        let p_mode1 = sin(dot(p_surf, vec3<f32>(18.0, 12.0, 16.0)) + t * 2.6);
        let p_mode2 = cos(dot(p_surf, vec3<f32>(-14.0, 20.0, 11.0)) - t * 2.1);
        let micro_shimmer = fbm(p_surf * (cell_scale * 0.85) + vec3<f32>(t * 0.25, 0.0, 0.0));
        let wave = (p_mode1 * 0.30 + p_mode2 * 0.30 + micro_shimmer * 0.40) * 0.5 + 0.5;

        let sapphire_pearl = vec3<f32>(0.98, 1.12, 1.45);
        let sirius_diamond = vec3<f32>(1.35, 1.44, 1.72);
        var a_col = mix(sapphire_pearl, sirius_diamond, wave);

        // von Zeipel rapid-rotation gravity darkening: polar caps hotter/whiter, equator cooler sapphire
        let pole_heat = abs(norm.y);
        let eq_cool = 1.0 - pole_heat;
        a_col = mix(a_col * vec3<f32>(0.86, 0.92, 1.08), a_col * vec3<f32>(1.14, 1.14, 1.18), pole_heat);
        a_col *= (0.86 + 0.28 * pole_heat);

        let limb_dark = 1.0 - 0.44 * (1.0 - NdotV) - 0.18 * (1.0 - sqrt(max(NdotV, 0.0)));
        a_col *= clamp(limb_dark, 0.32, 1.0 + eq_cool * 0.0);
        let rim = pow(1.0 - NdotV, 2.8) * vec3<f32>(0.85, 1.08, 1.65) * 2.6;
        final_col = a_col * 3.10 + rim;
    }
    // =========================================================================
    // 16. Herbig Ae/Be Star (Hot Pre-Main-Sequence Star: AB Aurigae, 9500 K)
    // =========================================================================
    else if (subtype == 16u) {
        // Hot blue-white young radiative star wrapped in spiraling circumstellar dust veins
        // and intense ultraviolet polar magnetospheric accretion shocks.
        let azim = atan2(p_surf.z, p_surf.x);
        let p_swirl = rotate_y(p_surf, norm.y * 2.4 + t * 0.12);
        let base_wave = fbm(p_swirl * cell_scale + vec3<f32>(t * 0.18, 0.0, 0.0));

        let ae_blue = vec3<f32>(0.88, 1.08, 1.48);
        let ae_white = vec3<f32>(1.28, 1.38, 1.68);
        var herbig_col = mix(ae_blue, ae_white, base_wave);

        // Spiraling circumstellar protoplanetary dust filaments (dark core + warm IR illuminated rim)
        let dust_spiral = ridge_noise(p_swirl * 5.8 + vec3<f32>(cos(azim), norm.y * 3.0, sin(azim)));
        if (dust_spiral > 0.62) {
            let ir_rim = smoothstep(0.62, 0.74, dust_spiral) * (1.0 - smoothstep(0.74, 0.86, dust_spiral));
            let dark_core = smoothstep(0.72, 0.90, dust_spiral);
            herbig_col += vec3<f32>(1.25, 0.52, 0.16) * ir_rim * 1.4;
            herbig_col = mix(herbig_col, vec3<f32>(0.10, 0.06, 0.08), dark_core * 0.82);
        }

        // Polar magnetospheric funnel shock hotspots
        let pole_cap = smoothstep(0.72, 0.94, abs(norm.y)) * fbm(p_surf * 9.0 + vec3<f32>(0.0, t * 0.35, 0.0));
        herbig_col += vec3<f32>(1.65, 1.90, 2.55) * pole_cap * (flare_int + 0.6);

        let limb_dark = pow(NdotV, 0.42);
        let rim = pow(1.0 - NdotV, 2.8) * vec3<f32>(0.80, 1.05, 1.65) * 2.8;
        final_col = herbig_col * (limb_dark * 0.82 + 0.18) * 3.0 + rim;
    }
    // =========================================================================
    // 17. Yellow Hypergiant (Rho Cassiopeiae / IRC +10420, 6000 K)
    // =========================================================================
    else if (subtype == 17u) {
        // Colossal, unstable super-Eddington convective plumes + detached shock shells
        let warp = vec3<f32>(
            fbm(p_surf * 2.2 + vec3<f32>(0.0, t * 0.025, 0.0)),
            fbm(p_surf * 2.2 + vec3<f32>(3.4, 0.0, t * 0.025)),
            fbm(p_surf * 2.2 + vec3<f32>(0.0, 6.8, t * 0.025))
        ) * 0.36;
        let p_plume = p_surf + warp;

        let v = voronoi3(p_plume * (cell_scale * 0.35), t * 0.18);
        let lane = smoothstep(0.04, 0.36, v.y - v.x);
        let plume_core = smoothstep(0.75, 0.15, v.x);

        let bronze_trough = vec3<f32>(0.62, 0.30, 0.06);
        let hyper_gold = vec3<f32>(1.18, 0.88, 0.34);
        let super_plume = vec3<f32>(1.52, 1.32, 0.82);

        var yhg_col = mix(bronze_trough, hyper_gold, lane);
        yhg_col = mix(yhg_col, super_plume, plume_core * 0.75);

        // Eruptive circumstellar shockwave shells propagating across the photosphere
        let shock_ring = pow(sin(length(p_plume * 5.5) - t * 1.2) * 0.5 + 0.5, 4.0);
        yhg_col += vec3<f32>(1.35, 0.75, 0.25) * shock_ring * 0.45;

        let pulsation = 1.0 + 0.08 * sin(t * 0.65);
        let limb_dark = pow(NdotV, 0.48);
        let rim = pow(1.0 - NdotV, 2.6) * vec3<f32>(1.45, 0.85, 0.24) * 3.4;
        final_col = yhg_col * pulsation * (limb_dark * 0.80 + 0.20) * 3.05 + rim;
    }
    // =========================================================================
    // 18. Carbon Star (AGB Soot-Shrouded Ruby Giant: R Leporis, 2600 K)
    // =========================================================================
    else if (subtype == 18u) {
        // Deepest ruby-crimson convective giant veiled by pitch-black amorphous carbon / SiC soot clouds
        let warp = vec3<f32>(fbm(p_surf * 2.1), fbm(p_surf * 2.1 + 3.3), fbm(p_surf * 2.1 + 6.6)) * 0.35;
        let v = voronoi3((p_surf + warp) * cell_scale, t * 0.12);
        let lane = smoothstep(0.04, 0.36, v.y - v.x);

        let dark_blood = vec3<f32>(0.34, 0.02, 0.01);
        let ruby_crimson = vec3<f32>(0.98, 0.12, 0.03);
        let cherry_core = vec3<f32>(1.35, 0.34, 0.06);
        var carb_col = mix(dark_blood, ruby_crimson, lane);
        carb_col = mix(carb_col, cherry_core, smoothstep(0.75, 0.18, v.x) * 0.70);

        // Thick drifting C2 / SiC amorphous carbon soot clouds with incandescent scarlet rift rims
        let soot_field = fbm(p_surf * 3.6 + vec3<f32>(t * 0.03, -t * 0.015, 0.0));
        let soot_ridge = ridge_noise(p_surf * 6.2 - vec3<f32>(0.0, t * 0.04, 0.0));
        let soot_density = soot_field * 0.70 + soot_ridge * 0.30;

        if (soot_density > 0.52) {
            let ember_edge = smoothstep(0.52, 0.62, soot_density) * (1.0 - smoothstep(0.62, 0.74, soot_density));
            let opaque_soot = smoothstep(0.58, 0.78, soot_density);
            carb_col += vec3<f32>(1.40, 0.24, 0.04) * ember_edge * 1.5;
            carb_col = mix(carb_col, vec3<f32>(0.025, 0.006, 0.004), opaque_soot * 0.92);
        }

        let limb_dark = pow(NdotV, 0.55);
        let rim = pow(1.0 - NdotV, 3.0) * vec3<f32>(1.15, 0.12, 0.02) * 2.8;
        final_col = carb_col * (limb_dark * 0.80 + 0.20) * 2.75 + rim;
    }
    // =========================================================================
    // 19. Luminous Blue Variable (Eruptive Eta Carinae / P Cygni Hypergiant)
    // =========================================================================
    else if (subtype == 19u) {
        // Super-Eddington blue hypergiant with violent bipolar Homunculus-style eruptive filaments
        let pole_lat = abs(norm.y);
        let wind_turb = fbm(p_surf * (cell_scale * 0.75) + vec3<f32>(t * 0.55, t * 0.35, 0.0));
        let shock_braid = ridge_noise(p_surf * (cell_scale * 0.55) - vec3<f32>(0.0, t * 0.65, 0.0));

        let lbv_azure = vec3<f32>(0.58, 0.88, 1.48);
        let lbv_white = vec3<f32>(1.28, 1.45, 1.85);
        var lbv_col = mix(lbv_azure, lbv_white, wind_turb);

        // Bipolar Homunculus eruptive breakout lobes & super-Eddington continuum knots
        let bipolar_eruption = smoothstep(0.45, 0.92, pole_lat) * smoothstep(0.55, 0.88, shock_braid);
        let eddington_flash = pow(max(shock_braid, 0.0), 3.2) * (0.85 + 0.35 * sin(t * 2.2));
        lbv_col += vec3<f32>(1.45, 1.85, 2.45) * (bipolar_eruption * 1.15 + eddington_flash * 0.75);

        let rim = pow(1.0 - NdotV, 2.1) * vec3<f32>(0.65, 1.10, 1.85) * 4.4;
        final_col = lbv_col * 3.35 + rim;
    }
    // =========================================================================
    // 20. Population III Star (Primordial Zero-Metallicity Z=0 Hypergiant, 70,000 K)
    // =========================================================================
    else if (subtype == 20u) {
        // Blinding actinic UV-violet-white nuclear furnace with pair-instability radial shocks
        let r_wave1 = sin(dot(p_surf, vec3<f32>(16.0, 14.0, 18.0)) + t * 4.2);
        let r_wave2 = cos(dot(p_surf, vec3<f32>(-15.0, 17.0, -12.0)) - t * 3.6);
        let core_conv = ridge_noise(p_surf * (cell_scale * 0.80) + vec3<f32>(0.0, t * 0.60, 0.0));
        let plasma_state = (r_wave1 * 0.30 + r_wave2 * 0.30 + core_conv * 0.40) * 0.5 + 0.5;

        let uv_violet = vec3<f32>(0.78, 0.95, 1.75);
        let actinic_white = vec3<f32>(1.55, 1.68, 2.35);
        var pop3_col = mix(uv_violet, actinic_white, plasma_state);

        // Pair-production (gamma + gamma -> e- + e+) high-energy shock arcs
        let pair_shock = smoothstep(0.72, 0.95, core_conv);
        pop3_col += vec3<f32>(1.40, 1.25, 2.20) * pair_shock * 0.95;

        let rim = pow(1.0 - NdotV, 2.0) * vec3<f32>(0.75, 0.90, 2.10) * 4.8;
        final_col = pop3_col * 3.55 + rim;
    }
    // =========================================================================
    // 21. Blue Straggler (Rejuvenated Stellar-Merger B/A Star)
    // =========================================================================
    else if (subtype == 21u) {
        // High-angular-momentum merged star with spiral differential-rotation shear bands
        // and a hot equatorial coalescence excretion belt.
        let azim = atan2(p_surf.z, p_surf.x);
        let shear_angle = azim * 3.0 + norm.y * 9.0 - t * 1.1;
        let shear_turb = fbm(rotate_y(p_surf, norm.y * 3.5) * (cell_scale * 0.65) + vec3<f32>(t * 0.3, 0.0, 0.0));
        let spiral_band = sin(shear_angle + shear_turb * 3.5) * 0.5 + 0.5;

        let bss_sapphire = vec3<f32>(0.62, 0.92, 1.52);
        let bss_diamond = vec3<f32>(1.28, 1.48, 1.85);
        var bss_col = mix(bss_sapphire, bss_diamond, spiral_band);

        // Super-heated equatorial merger accretion/decretion belt
        let eq_belt = exp(-norm.y * norm.y * 14.0) * (0.75 + 0.50 * shear_turb);
        bss_col += vec3<f32>(0.95, 1.35, 1.95) * eq_belt * 0.70;

        let rim = pow(1.0 - NdotV, 2.4) * vec3<f32>(0.60, 1.00, 1.75) * 3.6;
        final_col = bss_col * 3.20 + rim;
    }
    // =========================================================================
    // 22. Quark Star (Bare Strange-Quark Matter CFL Surface)
    // =========================================================================
    else if (subtype == 22u) {
        // Ultra-dense self-bound strange-matter sphere: mirror-like liquid-metallic violet-silver
        // surface with colour-flavour-locked (CFL) vortex lattice & 10^17 V/cm electrosphere arcs.
        let limb = pow(NdotV, 0.24);
        let v_cfl = voronoi3(p_surf * (cell_scale * 0.75), t * 0.25);
        let vortex_node = smoothstep(0.28, 0.04, v_cfl.x);

        let strange_silver = vec3<f32>(1.15, 1.02, 1.78);
        let cfl_violet = vec3<f32>(1.55, 1.15, 2.35);
        var qs_col = mix(strange_silver, cfl_violet, vortex_node);

        // Mirror-like liquid strange-matter specular sheen
        let liquid_spec = pow(NdotV, 14.0) * vec3<f32>(1.80, 1.70, 2.55);

        // Electrosphere e+ / e- Schwinger pair-production lightning arcs
        let elec_arc = ridge_noise(p_surf * (cell_scale * 0.95) + vec3<f32>(t * 1.4, -t * 1.1, 0.0));
        var arc_glow = vec3<f32>(0.0);
        if (elec_arc > 0.72) {
            arc_glow = vec3<f32>(0.65, 1.65, 2.45) * ((elec_arc - 0.72) / 0.28) * 3.4;
        }

        let rim = pow(1.0 - NdotV, 1.7) * vec3<f32>(1.10, 0.85, 2.50) * 4.0;
        final_col = (qs_col * limb + liquid_spec * 0.65 + arc_glow) * 3.45 + rim;
    }
    // =========================================================================
    // 23. Thorne-Zytkow Object (Red Supergiant Envelope + Buried Neutron Core)
    // =========================================================================
    else if (subtype == 23u) {
        // Massive deep-crimson convective supergranules whose deep intergranular rifts are
        // back-lit by high-frequency cyan-violet X-ray pulses from the internal neutron star core!
        let warp = vec3<f32>(fbm(p_surf * 2.2), fbm(p_surf * 2.2 + 3.1), fbm(p_surf * 2.2 + 6.2)) * 0.38;
        let v = voronoi3((p_surf + warp) * cell_scale, t * 0.16);
        let lane = smoothstep(0.06, 0.40, v.y - v.x);
        let deep_rift = 1.0 - lane;

        let dark_garnet = vec3<f32>(0.38, 0.04, 0.02);
        let rsg_vermillion = vec3<f32>(0.98, 0.28, 0.07);
        let rsg_amber = vec3<f32>(1.28, 0.62, 0.15);
        var tzo_col = mix(dark_garnet, rsg_vermillion, lane);
        tzo_col = mix(tzo_col, rsg_amber, smoothstep(0.78, 0.18, v.x) * 0.60);

        // Buried neutron core X-ray & irp-process nucleosynthesis pulse shining through convective rifts
        let ns_pulse = 0.65 + 0.55 * pow(sin(t * 6.5 + p_surf.y * 4.0) * 0.5 + 0.5, 3.0);
        let core_xray = vec3<f32>(0.45, 1.35, 2.25) * deep_rift * ns_pulse * 1.85;
        let polar_jet_glow = vec3<f32>(0.75, 0.95, 2.10) * smoothstep(0.78, 0.98, abs(norm.y)) * ns_pulse * 1.15;

        let limb_dark = pow(NdotV, 0.50);
        let rim = pow(1.0 - NdotV, 2.8) * vec3<f32>(1.25, 0.28, 0.45) * 3.2;
        final_col = (tzo_col * (limb_dark * 0.80 + 0.20) + core_xray + polar_jet_glow) * 2.85 + rim;
    }
    // =========================================================================
    // 24. Black Dwarf (Cooled Crystallized Carbon-Oxygen Diamond Relic, < 100 K)
    // =========================================================================
    else if (subtype == 24u) {
        // Cold, non-incandescent BCC crystallized carbon-oxygen diamond sphere.
        // Uses faceted Voronoi crystal cleavage planes and starlight/headlight specular glints.
        let v_macro = voronoi3(p_surf * 14.0, 0.0);
        let v_micro = voronoi3(p_surf * 36.0, 0.0);
        let cleavage = smoothstep(0.01, 0.12, v_macro.y - v_macro.x);
        let micro_facet = smoothstep(0.01, 0.10, v_micro.y - v_micro.x);

        let facet_grad = normalize(vec3<f32>(
            v_macro.x - 0.5 + (v_micro.x - 0.5) * 0.35,
            v_macro.y - 0.5,
            (v_micro.y - 0.5) * 0.5
        ));
        let facet_norm = normalize(mix(norm, facet_grad, 0.22));

        var key_dir = planet.star_dir_and_lum.xyz;
        if (dot(key_dir, key_dir) < 0.01) {
            key_dir = normalize(vec3<f32>(0.55, 0.72, 0.42));
        } else {
            key_dir = normalize(key_dir);
        }
        let half_key = normalize(key_dir + view_dir);
        let NdotL = max(dot(facet_norm, key_dir), 0.0);
        let spec_key = pow(max(dot(facet_norm, half_key), 0.0), 48.0);
        let spec_head = pow(max(dot(facet_norm, view_dir), 0.0), 32.0);

        let obsidian_carbon = vec3<f32>(0.022, 0.026, 0.042);
        let indigo_lattice = vec3<f32>(0.065, 0.085, 0.145);
        var diamond_col = mix(indigo_lattice, obsidian_carbon, cleavage * 0.75 + micro_facet * 0.25);

        let diffuse = 0.22 + 0.78 * NdotL;
        let glint = vec3<f32>(0.55, 0.72, 1.10) * (spec_key * 0.95 + spec_head * 0.45);
        let fresnel = pow(1.0 - NdotV, 3.8) * vec3<f32>(0.12, 0.18, 0.32);

        final_col = diamond_col * diffuse + glint + fresnel;
    }
    // =========================================================================
    // 25. Iron Star (Far-Future Cold-Fusion Fe-56 Degenerate Relic)
    // =========================================================================
    else if (subtype == 25u) {
        // Cold gunmetal-silver Fe-56 sphere with etched hexagonal Widmanstatten crystalline
        // iron-nickel lamellae, ancient micro-meteoroid pitting, and metallic specular sheen.
        let ax1 = normalize(vec3<f32>(1.0, 1.0, 1.0));
        let ax2 = normalize(vec3<f32>(-1.0, 1.0, 1.0));
        let ax3 = normalize(vec3<f32>(1.0, -1.0, 1.0));
        let ax4 = normalize(vec3<f32>(1.0, 1.0, -1.0));

        let w1 = abs(sin(dot(p_surf, ax1) * 42.0));
        let w2 = abs(sin(dot(p_surf, ax2) * 42.0));
        let w3 = abs(sin(dot(p_surf, ax3) * 42.0));
        let w4 = abs(sin(dot(p_surf, ax4) * 42.0));
        let widmanstatten = clamp((w1 * w2 + w3 * w4) * 0.85, 0.0, 1.0);

        // Ancient micro-meteoroid pitting
        let pits = voronoi3(p_surf * 28.0, 0.0);
        let crater_pit = smoothstep(0.22, 0.05, pits.x);

        var key_dir = planet.star_dir_and_lum.xyz;
        if (dot(key_dir, key_dir) < 0.01) {
            key_dir = normalize(vec3<f32>(0.55, 0.72, 0.42));
        } else {
            key_dir = normalize(key_dir);
        }
        let half_key = normalize(key_dir + view_dir);
        let NdotL = max(dot(norm, key_dir), 0.0);
        let aniso_spec = pow(max(dot(norm, half_key), 0.0), 20.0) * (0.55 + 0.65 * widmanstatten);
        let head_spec = pow(NdotV, 16.0) * (0.40 + 0.60 * widmanstatten);

        let dark_kamacite = vec3<f32>(0.085, 0.092, 0.108);
        let bright_taenite = vec3<f32>(0.21, 0.23, 0.27);
        var iron_col = mix(dark_kamacite, bright_taenite, widmanstatten);
        iron_col *= (1.0 - crater_pit * 0.45);

        let diffuse = 0.25 + 0.75 * NdotL;
        let metal_glint = vec3<f32>(0.38, 0.42, 0.48) * (aniso_spec * 0.85 + head_spec * 0.35);
        let rim = pow(1.0 - NdotV, 3.5) * vec3<f32>(0.14, 0.16, 0.20);

        final_col = iron_col * diffuse + metal_glint + rim;
    }
    // Fallback standard star
    else {
        let limb = pow(NdotV, 0.50);
        final_col = seed_col * (limb * 0.8 + 0.2) * 3.0;
    }

    // von Zeipel Gravity Darkening on Tidally Stretched Stars (Roche Lobe Overflow):
    // Applied only to hot luminous plasma stars (subtype < 24u), excluding cold dead relics.
    let raw_tidal_w = planet.tidal_deformation.w;
    let roche_stretch = select(raw_tidal_w, raw_tidal_w - 2.0, raw_tidal_w >= 1.5);
    if (subtype < 24u && roche_stretch > 0.05 && dot(planet.tidal_deformation.xyz, planet.tidal_deformation.xyz) > 0.5) {
        let t_dir = normalize(planet.tidal_deformation.xyz);
        let cos_t = dot(norm, t_dir);
        let l1_cone = smoothstep(0.25, 0.92, cos_t);
        let waist_band = 1.0 - abs(cos_t);
        let funnel_tint = vec3<f32>(1.55, 0.42, 0.14) * max(length(final_col) * 0.55, 1.5);
        final_col = mix(
            final_col * (1.0 + waist_band * roche_stretch * 0.22),
            funnel_tint,
            l1_cone * clamp(roche_stretch * 0.65, 0.0, 0.55)
        );
    }
    
    return vec4<f32>(final_col, final_alpha);
}








fn apply_impact_craters_and_basins(
    p_surf: vec3<f32>,
    in_color: vec3<f32>,
    in_roughness: f32,
    in_metallic: f32,
    time: f32,
    water_mask: f32,
) -> CraterShadingResult {
    var out_res: CraterShadingResult;
    out_res.color = in_color;
    out_res.roughness = in_roughness;
    out_res.metallic = in_metallic;
    out_res.emissive = vec3<f32>(0.0);
    out_res.normal_delta = vec3<f32>(0.0);

    let is_gas_giant = planet.planet_type == 1u || planet.composition.w > 0.40;
    let is_ice_giant = planet.planet_type == 2u;
    let is_icy_world = !is_gas_giant && !is_ice_giant && planet.composition.y > 0.35;

    for (var i = 0u; i < 4u; i = i + 1u) {
        let b_pos = planet.impact_basins_pos[i];
        let b_data = planet.impact_basins_data[i];
        let radius = b_pos.w;
        let scar_intensity = clamp(b_data.w, 0.0, 1.0);
        let melt_glow = b_data.x;
        let seismic_phase = clamp(b_data.z, 0.0, 1.0);

        let land_mask = select(1.0, 0.0, water_mask > 0.0);
        let effective_scar = scar_intensity * land_mask;
        if (radius <= 0.005 || effective_scar <= 0.005) {
            continue;
        }

        let center = normalize(b_pos.xyz);
        let elongation = max(b_data.y, 1.0);

        let cos_dist = clamp(dot(p_surf, center), -1.0, 1.0);
        let dist = acos(cos_dist);
        let raw_radial = p_surf - center * cos_dist;
        let radial_dir = select(vec3<f32>(0.0, 1.0, 0.0), normalize(raw_radial), dot(raw_radial, raw_radial) > 1e-6);

        var effective_dist = dist;
        if (elongation > 1.05) {
            let tangent = normalize(cross(center, vec3<f32>(0.0, 1.0, 0.001)));
            let d_tangent = dot(p_surf - center, tangent);
            let d_norm = sqrt(max(dist * dist - d_tangent * d_tangent, 0.0));
            effective_dist = sqrt((d_tangent / elongation) * (d_tangent / elongation) + d_norm * d_norm);
        }

        if (is_gas_giant) {
            out_res = apply_gas_giant_impact_basin(p_surf, center, radius, effective_scar, melt_glow, seismic_phase, time, out_res);
        } else if (is_ice_giant) {
            out_res = apply_ice_giant_impact_basin(p_surf, center, radius, effective_scar, melt_glow, seismic_phase, time, out_res);
        } else if (is_icy_world) {
            out_res = apply_icy_world_impact_basin(p_surf, center, radial_dir, effective_dist, radius, effective_scar, melt_glow, time, out_res);
        } else {
            out_res = apply_rocky_world_impact_basin(p_surf, center, radial_dir, effective_dist, radius, effective_scar, melt_glow, time, water_mask, out_res);
        }
    }

    return out_res;
}

// Procedural Tidal Volcanic Calderas & Fissure Hotspots (Io Analogs)
struct VolcanicCalderaResult {
    color: vec3<f32>,
    emissive: vec3<f32>,
    roughness: f32,
};

fn evaluate_volcanic_calderas(
    p_surf: vec3<f32>,
    lava_frac: f32,
    is_sulfur_world: bool,
    time: f32,
    in_color: vec3<f32>,
) -> VolcanicCalderaResult {
    var res: VolcanicCalderaResult;
    res.color = in_color;
    res.emissive = vec3<f32>(0.0);
    res.roughness = 0.85;

    // Multi-frequency cellular noise to distribute volcanic calderas & paterae
    let cal_noise = fbm(p_surf * 8.5 + vec3<f32>(0.0, time * 0.008, 0.0));
    let cal_cell = ridge_noise(p_surf * 18.0);
    let cal_mask = smoothstep(0.68, 0.92, cal_noise) * smoothstep(0.38, 0.76, cal_cell);

    if (cal_mask > 0.005) {
        let caldera_active = clamp(cal_mask * (lava_frac * 3.5 + select(0.0, 0.25, is_sulfur_world)), 0.0, 1.0);

        if (is_sulfur_world) {
            // Io-class sulfur volcanism & allotrope fallout rings
            // Chalky white SO2 frost plains
            let so2_frost = vec3<f32>(0.92, 0.94, 0.92);
            // Rich sulfur yellow & mustard plains (S8)
            let sulfur_yellow = vec3<f32>(0.86, 0.76, 0.22);
            // Brilliant orange-red and umber sulfur allotrope fallout halos (Pele/Prometheus rings)
            let fallout_halo_color = mix(vec3<f32>(0.92, 0.38, 0.08), vec3<f32>(0.68, 0.16, 0.06), cal_cell);
            // Jet-black silicate basalt caldera floor (e.g. Loki Patera)
            let basalt_dark = vec3<f32>(0.05, 0.05, 0.06);
            // Pulsating incandescent molten ultramafic lava lakes (>1500 K)
            let lava_pulse = 2.6 + sin(time * 3.6 + cal_cell * 12.0) * 0.75;
            let lava_core = vec3<f32>(1.45, 0.52, 0.06) * lava_pulse;

            // Concentric spatial zones:
            // 1. Wide diffuse fallout halo ring around caldera
            let fallout_halo = smoothstep(0.04, 0.28, caldera_active) * (1.0 - smoothstep(0.28, 0.65, caldera_active));
            // 2. Basalt caldera rim / floor
            let caldera_floor = smoothstep(0.25, 0.60, caldera_active) * (1.0 - smoothstep(0.60, 0.86, caldera_active));
            // 3. Molten incandescent magma pit
            let molten_pit = smoothstep(0.60, 0.98, caldera_active);

            // Blend sulfur plains, fallout rings, basalt floor, and lava
            res.color = mix(res.color, mix(so2_frost, sulfur_yellow, 0.70), smoothstep(0.02, 0.15, caldera_active) * 0.4);
            res.color = mix(res.color, fallout_halo_color, fallout_halo * 0.90);
            res.color = mix(res.color, basalt_dark, caldera_floor * 0.95);
            res.color = mix(res.color, lava_core * 0.5, molten_pit);

            res.emissive = lava_core * molten_pit * 3.2;
            res.roughness = mix(0.85, 0.18, molten_pit);
        } else {
            // Silicate basaltic volcanism
            let sulfur_rim = vec3<f32>(0.58, 0.50, 0.18);
            let basalt_dark = vec3<f32>(0.07, 0.07, 0.08);
            let lava_pulse = 2.8 + sin(time * 3.2 + cal_cell * 8.0) * 0.6;
            let lava_core = vec3<f32>(1.35, 0.52, 0.08) * lava_pulse;

            let rim = smoothstep(0.12, 0.55, caldera_active) * (1.0 - smoothstep(0.55, 0.88, caldera_active));
            let pit = smoothstep(0.55, 0.98, caldera_active);

            res.color = mix(res.color, mix(sulfur_rim, basalt_dark, 0.65), rim);
            res.color = mix(res.color, lava_core * 0.5, pit);
            res.emissive = lava_core * pit * 2.8;
            res.roughness = mix(0.85, 0.22, pit);
        }
    }
    return res;
}

// Procedural Cryotectonic Cycloidal Lineae & South Polar Tiger Stripes (Europa & Enceladus)
struct CryoFractureResult {
    color: vec3<f32>,
    emissive: vec3<f32>,
    roughness: f32,
};

fn evaluate_cryotectonic_fractures(
    p_surf: vec3<f32>,
    ice_frac: f32,
    lava_frac: f32,
    time: f32,
    in_color: vec3<f32>,
) -> CryoFractureResult {
    var res: CryoFractureResult;
    res.color = in_color;
    res.emissive = vec3<f32>(0.0);
    res.roughness = 0.40;

    // 1. Global Cycloidal Arcuate Fracture Lineae (Europa style)
    let lineae_freq = p_surf * 14.5;
    let r1 = ridge_noise(lineae_freq);
    let r2 = ridge_noise(lineae_freq + vec3<f32>(0.12, 0.08, 0.15));
    let double_ridge = smoothstep(0.52, 0.88, r1) * smoothstep(0.48, 0.85, r2);
    let micro_cracks = smoothstep(0.72, 0.95, ridge_noise(p_surf * 42.0));

    let salt_stain_color = vec3<f32>(0.56, 0.29, 0.16);
    let dark_tholin_vein = vec3<f32>(0.38, 0.16, 0.09);

    let fracture_intensity = clamp(double_ridge * 1.3 + micro_cracks * 0.45, 0.0, 1.0);
    if (fracture_intensity > 0.01) {
        let vein_blend = mix(salt_stain_color, dark_tholin_vein, micro_cracks);
        res.color = mix(res.color, vein_blend, fracture_intensity * 0.82);
        res.roughness = mix(res.roughness, 0.88, fracture_intensity);
    }

    // 2. South Polar Tiger Stripes & Hydrothermal Cryo-Vent Glow (Enceladus style)
    let south_polar_weight = smoothstep(-0.62, -0.82, p_surf.y);
    if (south_polar_weight > 0.01) {
        let stripe_coords = vec2<f32>(p_surf.x * 12.0 + sin(p_surf.z * 18.0) * 0.4, p_surf.z * 12.0);
        let stripe_wave = abs(sin(stripe_coords.x * 0.65 + stripe_coords.y * 0.35));
        let stripe_fissure = smoothstep(0.82, 0.98, 1.0 - stripe_wave) * south_polar_weight;

        if (stripe_fissure > 0.02) {
            let vent_pulse = 1.0 + sin(time * 2.8 + p_surf.x * 10.0) * 0.25;
            let vent_cyan = vec3<f32>(0.18, 0.82, 0.96) * vent_pulse * 1.8;
            let frost_shoulder = vec3<f32>(0.85, 0.94, 1.0);

            res.color = mix(res.color, frost_shoulder, stripe_fissure * 0.75);
            let active_vent = smoothstep(0.45, 0.95, stripe_fissure) * (0.4 + lava_frac * 8.0);
            res.emissive += vent_cyan * active_vent;
            res.roughness = mix(res.roughness, 0.15, stripe_fissure);
        }
    }

    return res;
}

// Procedural Dynamic Viscous Magma Ocean & Crustal Solidification
struct MagmaSurfaceOutput {
    color: vec3<f32>,
    emissive: vec3<f32>,
    roughness: f32,
    metallic: f32,
    crust_mask: f32,
};

fn evaluate_magma_ocean_surface(
    p_surf: vec3<f32>,
    elev: f32,
    temp: f32,
    lava_frac: f32,
    t: f32,
) -> MagmaSurfaceOutput {
    // 1. Convective mantle plume drift
    let plume_flow = vec3<f32>(
        sin(t * 0.035 + p_surf.z * 2.8),
        cos(t * 0.028 + p_surf.x * 2.2),
        sin(t * 0.032 + p_surf.y * 2.6)
    ) * 0.065;
    let p_convect = p_surf + plume_flow;

    // 2. Multi-scale Voronoi / ridge tectonic spreading rifts
    let rift_large = ridge_noise(p_convect * 6.5);
    let rift_med = ridge_noise(p_convect * 14.0 + vec3<f32>(0.0, t * 0.012, 0.0));
    let rift_fine = ridge_noise(p_convect * 30.0);
    let fissure = rift_large * 0.52 + rift_med * 0.34 + rift_fine * 0.14;

    // 3. Basaltic crust rafting & crystallization
    let crust_noise = fbm(p_surf * 3.8 + vec3<f32>(t * 0.006, 0.0, t * 0.006));

    // Crust coverage fraction: 0.0 at >= 1800K, 0.5 at 1200K, 0.92 at 650K, 1.0 at <= 450K
    let effective_temp = max(temp, 450.0 + lava_frac * 1350.0);
    let crust_fraction = clamp(1.0 - (effective_temp - 450.0) / 1350.0, 0.0, 1.0);

    let plate_threshold = mix(0.10, 0.86, crust_fraction);
    let is_crust = (crust_noise + elev * 0.28) > plate_threshold && (fissure < (0.62 + crust_fraction * 0.28));

    // 4. Blackbody incandescent color gradient
    let core_incandescent = vec3<f32>(1.0, 0.95, 0.65);
    let molten_orange = vec3<f32>(1.0, 0.42, 0.07);
    let deep_crimson = vec3<f32>(0.55, 0.09, 0.03);

    let rift_heat = clamp(fissure * 1.6 - 0.4, 0.0, 1.0);
    let high_t = clamp((effective_temp - 1200.0) / 800.0, 0.0, 1.0);
    let top_tone = mix(molten_orange, core_incandescent, high_t);
    let lava_tone = mix(deep_crimson, top_tone, rift_heat);

    // Dark komatiite/basalt plate raft with olivine flecks
    let basalt_base = vec3<f32>(0.07, 0.065, 0.065);
    let warm_crust = vec3<f32>(0.20, 0.10, 0.06);
    let plate_color = mix(basalt_base, warm_crust, fissure * 0.4);

    let pulse = sin(t * 2.0 + fissure * 6.283) * 0.12 + 0.88;

    var res: MagmaSurfaceOutput;
    if (is_crust && effective_temp < 1350.0) {
        res.color = plate_color;
        res.emissive = vec3<f32>(0.0);
        res.roughness = 0.88;
        res.metallic = 0.12;
        res.crust_mask = 1.0;
    } else {
        res.color = lava_tone * pulse;
        let glow_scale = clamp((effective_temp - 450.0) / 450.0, 0.0, 6.0);
        res.emissive = lava_tone * glow_scale * pulse;
        res.roughness = 0.35;
        res.metallic = 0.05;
        res.crust_mask = 0.0;
    }
    return res;
}

// =========================================================================
// ATMOSPHERIC STORM HEXAGONS & CLOUD VORTICES (Feature 3.4)
// =========================================================================






// ============================================================================
// MULTI-LAYER VOLUMETRIC CLOUD DECKS, CORIOLIS CYCLONES & CHAPMAN ATMOSPHERE
// ============================================================================



/// Advects atmospheric cloud coordinates through Coriolis cyclonic low-pressure vortices and
/// evaluates an organic, FBM-warped Central Dense Overcast (CDO) & baroclinic comma shield.
/// Never uses geometric rings or fixed m=2 sine arms; instead swirls the actual fractal cloud field!
struct CycloneAdvection {
    p_swirled: vec3<f32>,
    cdo_boost: f32,
    eye_clearing: f32,
    tower_height: f32,
};

fn evaluate_coriolis_cyclones(p_rot: vec3<f32>, t: f32, seed: vec3<f32>) -> CycloneAdvection {
    var p_adv = p_rot;
    var cdo_boost = 0.0;
    var eye_clearing = 1.0;
    var max_tower = 0.0;

    for (var ci = 0; ci < 2; ci = ci + 1) {
        let f_ci = f32(ci);
        let hemi = select(1.0, -1.0, ci == 1);
        let c_lat = hemi * (0.32 + 0.16 * fract(seed.r * 3.7 + f_ci * 0.61) + 0.035 * sin(t * 0.045 + f_ci * 2.1));
        let c_lon = (fract(seed.g * 5.1 + seed.b * 2.9 + f_ci * 0.47) - 0.5) * 6.2831853 - t * 0.032 * hemi;
        let cos_lat = cos(c_lat);
        let center = vec3<f32>(cos_lat * cos(c_lon), sin(c_lat), cos_lat * sin(c_lon));

        let d_vec = p_adv - center;
        let r = length(d_vec);
        let max_r = 0.34;
        if (r < max_r) {
            // 1. Swirl the 3D coordinate field using a smooth Rankine-like vortex + radial inflow
            let norm_r = r / max_r;
            let swirl_angle = hemi * (2.6 + 0.42 * sin(t * 0.07 + f_ci));
            p_adv = vortex_swirl(p_adv, center, max_r, swirl_angle);

            // Subtle low-pressure radial convergence pulling cloud filaments inward toward the core
            let inflow = (1.0 - norm_r) * smoothstep(0.0, 0.12, norm_r) * 0.055;
            p_adv = normalize(p_adv - d_vec * inflow);

            // 2. Evaluate turbulent FBM on the swirled coordinate to warp the storm canopy organically
            // Uniform time translation vectors evolve the spiral rainbands without altering fwidth(p_adv)
            let swirl_fbm_coarse = fbm_lod(p_adv * 9.5 + vec3<f32>(f_ci * 2.3 + t * 0.032, t * 0.040, -f_ci * 1.7 - t * 0.028));
            let swirl_fbm_fine = fbm_lod(p_adv * 21.0 - vec3<f32>(t * 0.055, f_ci * 1.1 - t * 0.042, t * 0.045));

            // Asymmetric baroclinic comma-cloud tail (elongated equatorward & trailing)
            let up_ref = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(center.y) > 0.9);
            let tang_u = normalize(cross(up_ref, center));
            let tang_v = cross(center, tang_u);
            let u = dot(d_vec, tang_u);
            let v = dot(d_vec, tang_v) * hemi;
            let comma_asymmetry = clamp((-u * 0.65 - v * 0.55) / max_r, -0.25, 0.35);

            // Heavily domain-warped radial profile so the storm is never a geometric circle
            let r_warped = norm_r
                + (swirl_fbm_coarse - 0.5) * 0.36
                + (swirl_fbm_fine - 0.5) * 0.18
                - comma_asymmetry * 0.28;

            // Broad, filled Central Dense Overcast (CDO) and feeder canopy modulated by fractal clouds
            let cdo_envelope = smoothstep(0.72, 0.06, r_warped);
            let feeder_texture = smoothstep(0.28, 0.82, swirl_fbm_coarse * 0.65 + swirl_fbm_fine * 0.35);
            let storm_mass = cdo_envelope * (0.35 + 0.65 * feeder_texture);

            // Tiny, subtle, noise-warped pinhole eye ONLY in the very center (norm_r < 0.035)
            let eye_warp = norm_r + (swirl_fbm_fine - 0.5) * 0.016;
            let pinhole_eye = smoothstep(0.008, 0.032, eye_warp);
            // Soft convective eyewall mound around the pinhole eye (heavily modulated by fine FBM)
            let eyewall_mound = exp(-pow((eye_warp - 0.042) / 0.038, 2.0)) * (0.65 + 0.35 * swirl_fbm_fine) * pinhole_eye;

            cdo_boost = max(cdo_boost, (storm_mass * 0.28 + eyewall_mound * 0.16) * pinhole_eye);
            eye_clearing = min(eye_clearing, mix(0.25, 1.0, pinhole_eye));
            max_tower = max(max_tower, (storm_mass * 0.65 + eyewall_mound * 0.55) * pinhole_eye);
        }
    }

    var res: CycloneAdvection;
    res.p_swirled = p_adv;
    res.cdo_boost = cdo_boost;
    res.eye_clearing = eye_clearing;
    res.tower_height = max_tower;
    return res;
}

struct CloudLayerSample {
    low_cumulus: f32,
    mid_frontal: f32,
    high_cirrus: f32,
    combined: f32,
    tower_height: f32,
};

/// Evaluates the 3-tier dynamic atmospheric cloud density field at a unit-sphere coordinate `p_pos`
/// (in the planet's tilted spin frame). Used identically for view-parallaxed cloud rendering and
/// 3D solar-ray-projected ground/ocean shadow casting.
fn sample_multilayer_cloud_decks(
    p_pos: vec3<f32>,
    spin_angle: f32,
    t: f32,
    seed: vec3<f32>,
    is_super_earth: bool,
) -> CloudLayerSample {
    let lat = p_pos.y;
    // Bounded spatial wave (never multiplied by t so fwidth(p) stays invariant and clouds never shear into stripes)
    let zonal_trade = sin(lat * 6.283185) * 0.15;
    let freq_scale = select(1.0, 0.92, is_super_earth);

    // Advect atmospheric coordinates through Coriolis cyclonic vortices using rigid-body differential rotation
    let p_base_rot = rotate_y(p_pos, spin_angle + t * 0.062 + zonal_trade);
    let cyc = evaluate_coriolis_cyclones(p_base_rot, t, seed);

    // 1. Low-altitude marine stratocumulus & convective cumulus humilis (h_1, lightly swirled by cyclones)
    let p_low = mix(
        rotate_y(p_pos, spin_angle + t * 0.038 + zonal_trade * 0.75),
        cyc.p_swirled,
        0.45
    );
    let c_low_raw = fbm_lod(p_low * (8.8 * freq_scale) + vec3<f32>(1.3 + t * 0.028, -0.7 - t * 0.022, 2.1 + t * 0.025));
    let itcz_convergence = exp(-pow(lat / 0.16, 2.0)) * 0.10;
    let low_cumulus = clamp(c_low_raw * 0.92 + itcz_convergence, 0.0, 1.0) * cyc.eye_clearing;

    // 2. Mid-tropospheric baroclinic frontal systems & fluid-advected cyclones (h_2)
    let p_mid = cyc.p_swirled;
    // Domain-warp mid-level clouds so fronts and cyclone rainbands have organic billowy edges that evolve over time
    let warp_q = vec3<f32>(
        fbm_lod(p_mid * (3.6 * freq_scale) + vec3<f32>(0.0, t * 0.024, 1.7 - t * 0.018)) - 0.5,
        fbm_lod(p_mid * (3.6 * freq_scale) + vec3<f32>(2.3 - t * 0.020, -1.1, t * 0.024)) - 0.5,
        fbm_lod(p_mid * (3.6 * freq_scale) + vec3<f32>(-1.9 + t * 0.022, 3.1 - t * 0.018, 0.0)) - 0.5
    ) * 0.22;
    let p_mid_warped = p_mid + warp_q;
    let c_mid_main = fbm_lod(p_mid_warped * (4.8 * freq_scale) + vec3<f32>(t * 0.024, -t * 0.018, t * 0.020));
    let c_mid_eddy = fbm_lod(p_mid_warped * (10.5 * freq_scale) - vec3<f32>(t * 0.042, 0.0, t * 0.042));
    let baroclinic_wave = sin(lat * 9.5 + c_mid_main * 2.4 - t * 0.07) * 0.5 + 0.5;

    // Couple cyclone CDO boost to the synoptic cloud field so a cyclone never appears as an isolated stamp
    let synoptic_gate = smoothstep(0.22, 0.52, c_mid_main + c_mid_eddy * 0.35);
    let mid_frontal = clamp(
        (c_mid_main * 0.60 + c_mid_eddy * 0.28 + baroclinic_wave * 0.12 * select(0.4, 1.0, is_super_earth) + cyc.cdo_boost * synoptic_gate) * cyc.eye_clearing,
        0.0,
        1.0
    );

    // 3. High-altitude ice-crystal cirrus & jet-stream anvil streaks (h_3, with upper-tropospheric anticyclonic outflow)
    let jet_super = sin(lat * 11.0) * 0.22;
    let p_high = mix(rotate_y(p_pos, spin_angle - t * 0.048 + jet_super), cyc.p_swirled, 0.30);
    let c_cirrus_fbm = fbm_lod(vec3<f32>(p_high.x * 6.5, p_high.y * 16.0, p_high.z * 6.5) + vec3<f32>(t * 0.034, 0.0, -t * 0.030));
    let c_cirrus_fil = ridge_noise(p_high * 12.5 + vec3<f32>(c_cirrus_fbm * 0.8 + t * 0.040, 0.0, 0.0));
    let high_cirrus = smoothstep(0.56, 0.88, c_cirrus_fbm * 0.65 + c_cirrus_fil * 0.35);

    let combined = clamp(mid_frontal * 0.56 + low_cumulus * 0.30 + high_cirrus * 0.24, 0.0, 1.0);
    let tower_height = clamp(mid_frontal * 0.68 + low_cumulus * 0.32 + cyc.tower_height * synoptic_gate * 0.35, 0.0, 1.0);

    var res: CloudLayerSample;
    res.low_cumulus = low_cumulus;
    res.mid_frontal = mid_frontal;
    res.high_cirrus = high_cirrus;
    res.combined = combined;
    res.tower_height = tower_height;
    return res;
}

struct MultiLayerCloudResult {
    total_alpha: f32,
    cloud_color: vec3<f32>,
    ground_shadow_mult: f32,
    cloud_normal_world: vec3<f32>,
    cloud_roughness: f32,
};

/// Evaluates 3-layer parallaxed clouds, 3D ray-projected solar shadows onto land & ocean,
/// Beer-Powder cloud-top self-shadowing, silver-lining forward scattering, and 3D cumulus normals.
fn evaluate_multilayer_clouds_and_shadows(
    p_tilted: vec3<f32>,
    surface_dir: vec3<f32>,
    v_world: vec3<f32>,
    to_star_world: vec3<f32>,
    s_tangent_x: vec3<f32>,
    s_axis: vec3<f32>,
    s_tangent_z: vec3<f32>,
    spin_angle: f32,
    t: f32,
    effective_cloud_density: f32,
    boil_steam_factor: f32,
    substellar_cloud_boost: f32,
    seed: vec3<f32>,
    is_super_earth: bool,
) -> MultiLayerCloudResult {
    // Transform view and star vectors into the planet's tilted spin frame
    let v_tilted = vec3<f32>(dot(v_world, s_tangent_x), dot(v_world, s_axis), dot(v_world, s_tangent_z));
    let star_tilted = vec3<f32>(dot(to_star_world, s_tangent_x), dot(to_star_world, s_axis), dot(to_star_world, s_tangent_z));

    let ndotv = clamp(dot(p_tilted, v_tilted), 0.18, 1.0);
    let ndotl = dot(p_tilted, star_tilted);

    // View tangent vector for stereoscopic altitude parallax across the 3 cloud decks
    let v_tangent = (v_tilted - p_tilted * dot(p_tilted, v_tilted)) / max(ndotv, 0.24);
    let h_low = 0.011;
    let h_mid = 0.022;
    let h_high = 0.035;

    let p_view_low = normalize(p_tilted + v_tangent * h_low);
    let p_view_mid = normalize(p_tilted + v_tangent * h_mid);
    let p_view_high = normalize(p_tilted + v_tangent * h_high);

    let s_low = sample_multilayer_cloud_decks(p_view_low, spin_angle, t, seed, is_super_earth);
    let s_mid = sample_multilayer_cloud_decks(p_view_mid, spin_angle, t, seed, is_super_earth);
    let s_high = sample_multilayer_cloud_decks(p_view_high, spin_angle, t, seed, is_super_earth);

    let base_thresh = select(0.49, 0.44, is_super_earth) - effective_cloud_density * 0.24 - substellar_cloud_boost;
    let dens_scale = clamp(effective_cloud_density * 1.40, 0.28, 1.0);

    let alpha_low = clamp((s_low.low_cumulus - (base_thresh + 0.03)) * 2.6 * dens_scale, 0.0, 0.72);
    let alpha_mid = clamp((s_mid.mid_frontal - base_thresh) * 3.0 * dens_scale, 0.0, 0.90);
    let alpha_high = clamp((s_high.high_cirrus - (base_thresh - 0.05)) * 1.9 * dens_scale, 0.0, 0.52);

    // Front-to-back multi-layer compositing (high cirrus over mid frontal/cyclone over low cumulus)
    let alpha_low_mid = 1.0 - (1.0 - alpha_low) * (1.0 - alpha_mid);
    let raw_total_alpha = 1.0 - (1.0 - alpha_low_mid) * (1.0 - alpha_high);
    let max_alpha = mix(select(0.84, 0.93, is_super_earth), 0.98, boil_steam_factor);
    let total_alpha = clamp(raw_total_alpha, 0.0, max_alpha);

    // -------------------------------------------------------------------------
    // 3D Solar-Ray-Projected Ground & Ocean Cloud Shadows
    // -------------------------------------------------------------------------
    // Project from the surface point `p_tilted` toward `star_tilted` at each cloud deck's altitude.
    // Lower sun angles near the terminator stretch the shadow projection distance across the globe!
    let sun_obliquity = 1.0 / clamp(ndotl + 0.08, 0.18, 1.0);
    let star_tangent = (star_tilted - p_tilted * ndotl) * min(sun_obliquity, 3.8);

    let p_sh_low = normalize(p_tilted + star_tangent * (h_low * 1.40));
    let p_sh_mid = normalize(p_tilted + star_tangent * (h_mid * 1.50));
    let p_sh_high = normalize(p_tilted + star_tangent * (h_high * 1.60));

    let sh_low_s = sample_multilayer_cloud_decks(p_sh_low, spin_angle, t, seed, is_super_earth);
    let sh_mid_s = sample_multilayer_cloud_decks(p_sh_mid, spin_angle, t, seed, is_super_earth);
    let sh_high_s = sample_multilayer_cloud_decks(p_sh_high, spin_angle, t, seed, is_super_earth);

    let tau_low = max(sh_low_s.low_cumulus - base_thresh, 0.0) * 2.2;
    let tau_mid = max(sh_mid_s.mid_frontal - (base_thresh - 0.02), 0.0) * 3.2;
    let tau_high = max(sh_high_s.high_cirrus - base_thresh, 0.0) * 1.2;
    let tau_shadow = (tau_low + tau_mid + tau_high) * dens_scale * min(sun_obliquity * 0.65 + 0.35, 1.85);

    // Daylight gate so shadows fade smoothly into the twilight terminator
    let daylight_shadow_gate = smoothstep(-0.04, 0.14, ndotl);
    let shadow_transmittance = exp(-tau_shadow * 2.05);
    // Preserve ambient sky light inside the umbra of the cloud shadow (floor ~ 0.24)
    let ground_shadow_mult = mix(1.0, mix(0.24, 1.0, shadow_transmittance), daylight_shadow_gate);

    // -------------------------------------------------------------------------
    // 3D Cloud-Top Self-Shadowing, Silver-Lining Mie Phase & Cumulus Bump Normals
    // -------------------------------------------------------------------------
    let eps_c = 0.0045;
    let s_sunward = sample_multilayer_cloud_decks(
        normalize(p_view_mid + star_tilted * eps_c),
        spin_angle,
        t,
        seed,
        is_super_earth
    );
    // Positive when moving toward the star encounters thicker/higher cloud towers (self-occluded flank)
    let sun_slope = (s_sunward.tower_height - s_mid.tower_height) / eps_c;
    let self_shadow = clamp(1.0 - max(sun_slope, 0.0) * 0.14, 0.58, 1.08);
    let sunlit_ridge = clamp(1.0 + max(-sun_slope, 0.0) * 0.12, 1.0, 1.16);

    // Beer-Powder multiple-scattering boost at cloud edges and thin cirrus
    let powder_effect = 1.0 - 0.42 * exp(-total_alpha * 3.2);

    // Forward-scattering silver lining when viewing clouds toward low sun
    let cos_view_sun = dot(-v_world, to_star_world);
    let silver_lining = pow(max(cos_view_sun, 0.0), 6.0) * (1.0 - total_alpha * 0.55) * smoothstep(-0.12, 0.35, ndotl) * 0.38;

    // Warm sunset/sunrise Rayleigh reddening on cloud tops near the terminator
    let twilight_cloud_zone = smoothstep(0.28, -0.06, ndotl) * smoothstep(-0.24, -0.02, ndotl);
    let cloud_base_tint = mix(
        vec3<f32>(0.78, 0.84, 0.93), // Cool ambient blue-grey cloud base in shadow/troughs
        vec3<f32>(0.98, 0.99, 1.00), // Brilliant sunlit cumulus anvil top
        clamp(self_shadow * sunlit_ridge * (0.55 + 0.45 * powder_effect), 0.0, 1.0)
    );
    let sunset_coral = vec3<f32>(1.08, 0.62, 0.40);
    var lit_cloud_col = mix(cloud_base_tint, cloud_base_tint * sunset_coral, twilight_cloud_zone * 0.68);
    lit_cloud_col += vec3<f32>(1.0, 0.95, 0.86) * silver_lining;
    lit_cloud_col = mix(lit_cloud_col, vec3<f32>(0.99, 1.0, 1.0), boil_steam_factor * 0.65);

    // 3D Cumulus normal perturbation in world space
    let sx = sample_multilayer_cloud_decks(normalize(p_view_mid + vec3<f32>(eps_c, 0.0, 0.0)), spin_angle, t, seed, is_super_earth).tower_height;
    let sy = sample_multilayer_cloud_decks(normalize(p_view_mid + vec3<f32>(0.0, eps_c, 0.0)), spin_angle, t, seed, is_super_earth).tower_height;
    let sz = sample_multilayer_cloud_decks(normalize(p_view_mid + vec3<f32>(0.0, 0.0, eps_c)), spin_angle, t, seed, is_super_earth).tower_height;
    let grad_tilted = vec3<f32>(sx - s_mid.tower_height, sy - s_mid.tower_height, sz - s_mid.tower_height) / eps_c;
    let grad_world = grad_tilted.x * s_tangent_x + grad_tilted.y * s_axis + grad_tilted.z * s_tangent_z;
    let cloud_normal_world = normalize(surface_dir - grad_world * 0.028 * total_alpha);

    var out_res: MultiLayerCloudResult;
    out_res.total_alpha = total_alpha;
    out_res.cloud_color = lit_cloud_col;
    out_res.ground_shadow_mult = ground_shadow_mult;
    out_res.cloud_normal_world = cloud_normal_world;
    out_res.cloud_roughness = mix(0.88, 0.72, alpha_high * 0.5);
    return out_res;
}

struct CanonicalSurfaceResult {
    color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    emissive: vec3<f32>,
    water_mask: f32,
    cloud_alpha: f32,
};

fn evaluate_mars_surface_landmarks(
    p_surf: vec3<f32>,
    p_tilted: vec3<f32>,
    elev: f32,
    cloud_density: f32,
    season_subsolar: f32,
    t: f32,
) -> CanonicalSurfaceResult {
    let p = normalize(p_surf);
    let micro = fbm_lod(p * 18.0);
    let meso = fbm_lod(p * 6.5);

    // 1. Bright ferric-oxide (nanophase hematite / goethite) dusty highlands vs ochre plains
    let bright_dust = vec3<f32>(0.82, 0.48, 0.28);
    let rust_ochre = vec3<f32>(0.68, 0.32, 0.17);
    var col = mix(rust_ochre, bright_dust, clamp((elev - 0.30) * 1.8 + meso * 0.35, 0.0, 1.0));

    // 2. Dark wind-scoured basaltic sand seas (Syrtis Major, Acidalia, Sinus Meridiani, Solis Lacus)
    let d_syrtis = length(p - vec3<f32>(0.28, 0.16, 0.95));
    let d_acidalia = length(p - vec3<f32>(0.76, 0.62, -0.18));
    let d_meridiani = length(p - vec3<f32>(0.96, -0.06, -0.26));
    let d_solis = length(p - vec3<f32>(0.52, -0.38, -0.76));
    let dark_albedo_raw = smoothstep(0.30, 0.08, d_syrtis) * 0.85
        + smoothstep(0.36, 0.10, d_acidalia) * 0.80
        + smoothstep(0.30, 0.08, d_meridiani) * 0.75
        + smoothstep(0.28, 0.08, d_solis) * 0.70;
    let wind_streaks = fbm_lod(vec3<f32>(p.x * 9.0 + p.z * 4.0, p.y * 18.0, p.z * 9.0));
    let dark_basalt_mask = clamp(dark_albedo_raw * (0.65 + 0.55 * wind_streaks), 0.0, 0.92);
    let syrtis_basalt = vec3<f32>(0.24, 0.16, 0.14);
    col = mix(col, syrtis_basalt, dark_basalt_mask);

    // 3. Valles Marineris stratified canyon walls & deep chasma floor
    let vm_center = vec3<f32>(0.89, -0.11, -0.44);
    let vm_along = dot(p - vm_center, vec3<f32>(0.42, -0.06, 0.90));
    let vm_across = abs(dot(p - vm_center, vec3<f32>(0.10, 0.99, 0.04)) + 0.015 * sin(vm_along * 18.0));
    let in_valles = smoothstep(0.28, 0.16, abs(vm_along)) * smoothstep(0.048, 0.012, vm_across);
    let strata_band = sin(elev * 150.0 + micro * 4.0) * 0.5 + 0.5;
    let canyon_wall = mix(vec3<f32>(0.34, 0.18, 0.12), vec3<f32>(0.76, 0.44, 0.24), strata_band);
    col = mix(col, canyon_wall, in_valles * 0.85);

    // 4. Hellas Planitia pale dust-settled floor & winter frost haze
    let d_hellas = length(p - vec3<f32>(-0.56, -0.67, 0.48));
    let in_hellas = smoothstep(0.28, 0.10, d_hellas);
    let hellas_dust = vec3<f32>(0.85, 0.62, 0.40);
    col = mix(col, hellas_dust, in_hellas * 0.72);

    // 5. Olympus Mons & Tharsis Montes high basaltic calderas + orographic water-ice clouds
    let d_olympus = length(p - vec3<f32>(0.64, 0.31, -0.70));
    let d_asc = length(p - vec3<f32>(0.78, 0.19, -0.59));
    let d_pav = length(p - vec3<f32>(0.80, 0.02, -0.60));
    let d_ars = length(p - vec3<f32>(0.79, -0.15, -0.59));
    let volcano_flank = smoothstep(0.12, 0.02, d_olympus)
        + 0.75 * (smoothstep(0.075, 0.015, d_asc) + smoothstep(0.075, 0.015, d_pav) + smoothstep(0.080, 0.015, d_ars));
    let caldera_lava = vec3<f32>(0.38, 0.20, 0.14);
    col = mix(col, caldera_lava, clamp(volcano_flank * 0.65, 0.0, 0.75));

    var rough = mix(0.88, 0.76, dark_basalt_mask);
    var cloud_a = 0.0;
    if (cloud_density > 0.005) {
        let orographic_wisp = clamp(volcano_flank, 0.0, 1.0) * smoothstep(0.35, 0.78, fbm_lod(p * 14.0 + vec3<f32>(t * 0.02, 0.0, 0.0)));
        let valles_fog = in_valles * smoothstep(0.28, 0.18, elev) * 0.45;
        cloud_a = clamp((orographic_wisp * 0.55 + valles_fog) * clamp(cloud_density * 6.0, 0.25, 1.0), 0.0, 0.65);
        col = mix(col, vec3<f32>(0.92, 0.94, 0.98), cloud_a);
    }

    // 6. Planum Boreum & Planum Australe Polar Ice Caps with Coriolis Spiral Troughs
    let pole_r = length(p_tilted.xz);
    let pole_theta = atan2(p_tilted.z, p_tilted.x);
    let n_cap_radius = clamp(0.22 - season_subsolar * 0.06, 0.14, 0.30);
    let s_cap_radius = clamp(0.18 + season_subsolar * 0.08, 0.10, 0.28);
    let cap_radius = select(s_cap_radius, n_cap_radius, p_tilted.y > 0.0);
    let in_polar_cap = smoothstep(cap_radius + 0.03, cap_radius - 0.02, pole_r + (meso - 0.5) * 0.04) * step(0.75, abs(p_tilted.y));
    if (in_polar_cap > 0.01) {
        // Spiral Chasma Boreale / Australe troughs exposing dark layered dust inside the white ice cap
        let spiral_phase = pole_theta * 5.0 + sign(p_tilted.y) * pole_r * 42.0 + meso * 2.2;
        let spiral_trough = smoothstep(0.58, 0.88, sin(spiral_phase)) * smoothstep(0.03, 0.08, pole_r) * smoothstep(cap_radius, cap_radius * 0.45, pole_r);
        let cap_ice = mix(vec3<f32>(0.94, 0.95, 0.97), vec3<f32>(0.52, 0.28, 0.18), spiral_trough * 0.78);
        col = mix(col, cap_ice, in_polar_cap);
        rough = mix(rough, 0.28, in_polar_cap * (1.0 - spiral_trough * 0.6));
    }

    col *= (0.90 + micro * 0.20);
    var res: CanonicalSurfaceResult;
    res.color = col;
    res.roughness = rough;
    res.metallic = 0.04;
    res.emissive = vec3<f32>(0.0);
    res.water_mask = 0.0;
    res.cloud_alpha = cloud_a;
    return res;
}

fn evaluate_moon_and_mercury_landmarks(
    p_surf: vec3<f32>,
    t_sub: f32,
    elev: f32,
) -> CanonicalSurfaceResult {
    let p = normalize(p_surf);
    let micro = fbm_lod(p * 22.0);
    let craters = fbm_lod(p * 11.0);
    var col = vec3<f32>(0.5);
    var rough = 0.88;
    var metal_val = 0.04;

    if (t_sub == 2.0) {
        // THE MOON / LUNA (Nearside Basaltic Maria, Farside Anorthositic Highlands, Feathery Crater Ejecta)
        let warp_vec = vec3<f32>(
            fbm_lod(p * 3.6 + vec3<f32>(1.1, -0.7, 0.4)) - 0.5,
            fbm_lod(p * 3.6 + vec3<f32>(-0.9, 1.4, -0.6)) - 0.5,
            fbm_lod(p * 3.6 + vec3<f32>(0.5, -1.2, 1.7)) - 0.5
        ) * 0.22;
        let pw = normalize(p + warp_vec);

        let highland_albedo = mix(vec3<f32>(0.50, 0.49, 0.47), vec3<f32>(0.68, 0.67, 0.65), clamp((elev - 0.38) * 2.4 + craters * 0.25, 0.0, 1.0));
        let d_imb = length(pw - vec3<f32>(0.76, 0.54, -0.36));
        let d_proc = length(pw - vec3<f32>(0.62, 0.18, -0.76));
        let d_ser = length(pw - vec3<f32>(0.82, 0.45, 0.35));
        let d_tran = length(pw - vec3<f32>(0.83, 0.15, 0.54));
        let d_cris = length(pw - vec3<f32>(0.48, 0.29, 0.83));
        let d_nub = length(pw - vec3<f32>(0.76, -0.38, -0.52));
        let maria_olive_mask = clamp(
            smoothstep(0.27, 0.09, d_imb) + smoothstep(0.42, 0.13, d_proc) * 0.85
            + smoothstep(0.20, 0.06, d_ser) + smoothstep(0.15, 0.04, d_cris) + smoothstep(0.24, 0.07, d_nub) * 0.80,
            0.0, 1.0
        );
        let maria_ti_mask = clamp(smoothstep(0.22, 0.06, d_tran) + smoothstep(0.34, 0.11, d_proc) * 0.45, 0.0, 1.0);
        let maria_flow_tex = fbm_lod(p * 16.0);
        let basalt_olive = mix(vec3<f32>(0.18, 0.17, 0.16), vec3<f32>(0.25, 0.24, 0.23), maria_flow_tex);
        let basalt_titanium = mix(vec3<f32>(0.17, 0.19, 0.23), vec3<f32>(0.24, 0.25, 0.29), maria_flow_tex);
        col = mix(highland_albedo, basalt_olive, maria_olive_mask * 0.86);
        col = mix(col, basalt_titanium, maria_ti_mask * 0.80);

        // South Pole-Aitken (SPA) farside mafic lower-crustal floor
        let d_spa = length(pw - vec3<f32>(-0.42, -0.82, -0.38));
        col = mix(col, vec3<f32>(0.32, 0.30, 0.29), smoothstep(0.46, 0.15, d_spa) * 0.68);

        // Global cellular crater rims & fresh ejecta halos across both nearside and farside
        let v_crat = voronoi3(p * 15.0 + vec3<f32>(2.3, -1.7, 0.9), 0.0);
        let fresh_rim = exp(-pow((v_crat.x - 0.20) / 0.045, 2.0)) * smoothstep(0.48, 0.78, fbm_lod(p * 9.0));
        col = mix(col, vec3<f32>(0.75, 0.75, 0.76), fresh_rim * 0.35);

        // Feathery, non-singular ejecta blankets around Tycho, Copernicus, Aristarchus & farside Jackson
        // (Mixing 3D surface position `p` with radial direction avoids geometric point-pinch starburst spokes)
        let ejecta_clump = smoothstep(0.38, 0.78, fbm_lod(p * 26.0));

        let tycho_c = vec3<f32>(0.71, -0.68, -0.18);
        let d_tycho = length(p - tycho_c);
        let dir_tycho = normalize(p - tycho_c + vec3<f32>(1e-5));
        let tex_tycho = fbm_lod(dir_tycho * 3.8 + p * 14.0);
        let ray_tycho = smoothstep(0.50, 0.78, tex_tycho) * ejecta_clump * smoothstep(0.46, 0.06, d_tycho) * smoothstep(0.032, 0.075, d_tycho);
        let rim_tycho = exp(-pow((d_tycho - 0.035) / 0.012, 2.0));

        let cop_c = vec3<f32>(0.92, 0.17, -0.34);
        let d_cop = length(p - cop_c);
        let dir_cop = normalize(p - cop_c + vec3<f32>(1e-5));
        let tex_cop = fbm_lod(dir_cop * 4.0 + p * 16.0);
        let ray_cop = smoothstep(0.48, 0.76, tex_cop) * ejecta_clump * smoothstep(0.32, 0.05, d_cop) * smoothstep(0.034, 0.070, d_cop);
        let rim_cop = exp(-pow((d_cop - 0.037) / 0.012, 2.0));

        let d_aris = length(p - vec3<f32>(0.62, 0.40, -0.67));
        let aris_bright = exp(-pow((d_aris - 0.024) / 0.018, 2.0)) * (0.5 + 0.5 * ejecta_clump);

        let jack_c = vec3<f32>(-0.86, 0.37, -0.35);
        let d_jack = length(p - jack_c);
        let dir_jack = normalize(p - jack_c + vec3<f32>(1e-5));
        let ray_jack = smoothstep(0.50, 0.78, fbm_lod(dir_jack * 3.8 + p * 15.0)) * ejecta_clump * smoothstep(0.36, 0.05, d_jack) * smoothstep(0.028, 0.065, d_jack);

        let ray_total = clamp(
            ray_tycho * 0.38 + rim_tycho * 0.52 + ray_cop * 0.34 + rim_cop * 0.48 + aris_bright * 0.55 + ray_jack * 0.34,
            0.0, 0.68
        );
        col = mix(col, vec3<f32>(0.82, 0.83, 0.85), ray_total);
        rough = mix(0.92, 0.78, clamp(maria_olive_mask + maria_ti_mask, 0.0, 1.0));
    } else {
        // MERCURY (Caloris Planitia Buff Plains, LRM Graphite Basins, Lobate Scarps, Hokusai/Debussy Ejecta)
        let inter_plains = vec3<f32>(0.38, 0.36, 0.34);
        let lrm_graphite = vec3<f32>(0.21, 0.22, 0.26); // Low-Reflectance Material (graphite-rich primary crust)
        let lrm_mask = smoothstep(0.45, 0.72, fbm_lod(p * 3.2 + vec3<f32>(1.4, -0.8, 0.5)));
        col = mix(inter_plains, lrm_graphite, lrm_mask * 0.75);

        // Caloris Planitia warm buff-gold volcanic interior & Pantheon Fossae
        let caloris_c = vec3<f32>(-0.82, 0.50, 0.28);
        let d_caloris = length(p - caloris_c);
        let in_caloris = smoothstep(0.27, 0.12, d_caloris);
        let caloris_buff = vec3<f32>(0.52, 0.45, 0.36);
        col = mix(col, caloris_buff, in_caloris * 0.85);

        // Hokusai & Debussy feathery crater ejecta blankets (without point-singularity starburst spokes)
        let ejecta_clump = smoothstep(0.38, 0.78, fbm_lod(p * 24.0));
        let hok_c = vec3<f32>(0.38, 0.85, 0.36);
        let d_hok = length(p - hok_c);
        let dir_hok = normalize(p - hok_c + vec3<f32>(1e-5));
        let ray_hok = smoothstep(0.50, 0.78, fbm_lod(dir_hok * 4.0 + p * 14.0)) * ejecta_clump * smoothstep(0.38, 0.04, d_hok) * smoothstep(0.025, 0.060, d_hok);
        let deb_c = vec3<f32>(-0.22, -0.56, 0.80);
        let d_deb = length(p - deb_c);
        let dir_deb = normalize(p - deb_c + vec3<f32>(1e-5));
        let ray_deb = smoothstep(0.50, 0.78, fbm_lod(dir_deb * 4.0 + p * 14.5)) * ejecta_clump * smoothstep(0.34, 0.04, d_deb) * smoothstep(0.025, 0.060, d_deb);
        col = mix(col, vec3<f32>(0.74, 0.76, 0.80), clamp(ray_hok * 0.42 + ray_deb * 0.38, 0.0, 0.65));
        rough = 0.68;
        metal_val = 0.28;
    }

    col *= (0.88 + micro * 0.24);
    var res: CanonicalSurfaceResult;
    res.color = col;
    res.roughness = rough;
    res.metallic = metal_val;
    res.emissive = vec3<f32>(0.0);
    res.water_mask = 0.0;
    res.cloud_alpha = 0.0;
    return res;
}

fn evaluate_outer_moon_landmarks(
    p_surf: vec3<f32>,
    p_tilted: vec3<f32>,
    norm: vec3<f32>,
    v_world: vec3<f32>,
    to_star: vec3<f32>,
    t_sub: f32,
    elev: f32,
) -> CanonicalSurfaceResult {
    let p = normalize(p_surf);
    let micro = fbm_lod(p * 20.0);
    var col = vec3<f32>(0.5);
    var rough = 0.65;
    var emissive = vec3<f32>(0.0);
    var w_mask = 0.0;
    var c_alpha = 0.0;

    if (t_sub == 6.0) {
        // GANYMEDE (Dark Galileo/Marius Regio Polygons vs Bright Woven Uruk/Tiamat Sulci & Polar Frost Caps)
        let d_galileo = length(p - vec3<f32>(-0.62, 0.42, 0.66));
        let d_marius = length(p - vec3<f32>(0.74, 0.14, -0.66));
        let regio_poly = smoothstep(0.46, 0.60, fbm_lod(p * 2.8 + vec3<f32>(0.4, 1.1, -0.7))
            + smoothstep(0.45, 0.15, d_galileo) * 0.35 + smoothstep(0.38, 0.12, d_marius) * 0.30);
        let dark_regio = vec3<f32>(0.26, 0.22, 0.19) * (0.86 + micro * 0.28);
        let sulci_lanes = ridge_noise(p * 28.0) * 0.5 + 0.5;
        let bright_sulci = mix(vec3<f32>(0.58, 0.55, 0.50), vec3<f32>(0.76, 0.74, 0.70), sulci_lanes);
        col = mix(bright_sulci, dark_regio, regio_poly);

        // Magnetospheric open-field-line Polar Frost Caps (above +-40 deg latitude)
        let polar_frost = smoothstep(0.58, 0.76, abs(p_tilted.y) + (fbm_lod(p * 8.0) - 0.5) * 0.08);
        col = mix(col, vec3<f32>(0.80, 0.88, 0.95), polar_frost * 0.58);

        // Tros & Osiris gleaming water-ice ray craters
        let d_tros = length(p - vec3<f32>(0.84, 0.20, 0.50));
        let d_osiris = length(p - vec3<f32>(-0.48, -0.62, -0.62));
        let ice_rays = smoothstep(0.18, 0.015, d_tros) + smoothstep(0.20, 0.015, d_osiris);
        col = mix(col, vec3<f32>(0.94, 0.97, 1.0), clamp(ice_rays * 0.75, 0.0, 0.88));
        rough = mix(0.42, 0.78, regio_poly);
    } else if (t_sub == 7.0) {
        // CALLISTO (Ancient Dark Carbonaceous-Ice Crust + Valhalla & Asgard Multi-Ring Palimpsests)
        let dark_crust = vec3<f32>(0.22, 0.20, 0.18) * (0.85 + micro * 0.30);
        let frost_pockmarks = smoothstep(0.72, 0.90, ridge_noise(p * 26.0));
        col = mix(dark_crust, vec3<f32>(0.78, 0.80, 0.84), frost_pockmarks * 0.55);

        // Valhalla (3,800 km concentric multi-ring impact basin) & Asgard palimpsest
        let d_val = length(p - vec3<f32>(0.78, 0.26, -0.57));
        let val_core = smoothstep(0.14, 0.03, d_val);
        let val_rings = smoothstep(0.25, 0.75, sin(d_val * 58.0) * 0.5 + 0.5) * smoothstep(0.45, 0.06, d_val);
        let d_asg = length(p - vec3<f32>(-0.52, 0.52, -0.68));
        let asg_rings = smoothstep(0.25, 0.75, sin(d_asg * 65.0) * 0.5 + 0.5) * smoothstep(0.30, 0.03, d_asg);
        let palimpsest_col = vec3<f32>(0.68, 0.63, 0.55);
        col = mix(col, palimpsest_col, clamp(val_core * 0.82 + val_rings * 0.52 + asg_rings * 0.48, 0.0, 0.88));
        rough = mix(0.76, 0.45, clamp(val_core + frost_pockmarks, 0.0, 1.0));
    } else {
        // TITAN (Shangri-La Hydrocarbon Dunes, Xanadu Plateau, Kraken/Ligeia Mare + Golden Tholin Veil)
        let dune_belt = smoothstep(0.42, 0.12, abs(p_tilted.y)) * smoothstep(0.35, 0.65, fbm_lod(p * 3.5));
        let d_xanadu = length(p - vec3<f32>(-0.42, -0.18, -0.89));
        let xanadu_mask = smoothstep(0.34, 0.10, d_xanadu);
        let ice_bedrock = vec3<f32>(0.54, 0.44, 0.30) * (0.88 + micro * 0.24);
        let shangri_la_dunes = vec3<f32>(0.22, 0.15, 0.10);
        let xanadu_bright = vec3<f32>(0.68, 0.58, 0.42);
        var surf = mix(ice_bedrock, shangri_la_dunes, dune_belt * (1.0 - xanadu_mask));
        surf = mix(surf, xanadu_bright, xanadu_mask * 0.80);

        // North Polar Liquid Methane/Ethane Seas (Kraken Mare, Ligeia Mare, Punga Mare)
        if (elev < 0.0) {
            surf = vec3<f32>(0.04, 0.06, 0.08);
            w_mask = 0.82;
            rough = 0.08;
        } else {
            rough = 0.62;
        }

        // Stratospheric Golden-Amber Tholin Photochemical Veil + Detached Cyan Polar/Limb Haze
        let ndotv = max(dot(norm, v_world), 0.0);
        let limb_factor = pow(1.0 - ndotv, 2.2);
        let golden_haze = mix(vec3<f32>(0.82, 0.56, 0.24), vec3<f32>(0.72, 0.46, 0.18), smoothstep(-0.2, 0.7, p_tilted.y));
        let haze_opacity = clamp(0.56 + limb_factor * 0.36, 0.0, 0.92);
        col = mix(surf, golden_haze, haze_opacity);
        c_alpha = haze_opacity * 0.65;

        let detached_blue_limb = pow(1.0 - ndotv, 4.0) * max(dot(norm, to_star) * 0.5 + 0.5, 0.0);
        let polar_hood = smoothstep(0.68, 0.92, abs(p_tilted.y)) * 0.35;
        emissive = vec3<f32>(0.22, 0.58, 0.95) * (detached_blue_limb * 0.55 + polar_hood * detached_blue_limb);
    }

    var res: CanonicalSurfaceResult;
    res.color = col;
    res.roughness = rough;
    res.metallic = 0.03;
    res.emissive = emissive;
    res.water_mask = w_mask;
    res.cloud_alpha = c_alpha;
    return res;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let norm = normalize(in.world_normal);
    // On celestial spheres and ellipsoids, world_normal carries full 24-bit precision without
    // the catastrophic float cancellation of subtracting world positions at hundreds of AU.
    let surface_dir = norm;
    let to_star = normalize(-in.world_position.xyz);
    let stellar_insolation = max(dot(norm, to_star), 0.0);

    // Normalized 3D physical spin axis in world coordinates
    var s_axis = planet.spin_axis.xyz;
    if (length(s_axis) < 0.1) {
        s_axis = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        s_axis = normalize(s_axis);
    }

    // Rotational latitude: dot product with physical 3D spin axis
    let sin_lat = dot(surface_dir, s_axis);
    let polar_angle = abs(sin_lat);

    // Orthonormal basis aligned with physical 3D spin axis (Y = s_axis)
    // Ensures surface bands, polar storms/hexagons, and auroras are 100% co-aligned with true spin/magnetic poles.
    let s_ref = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(s_axis.z) > 0.95);
    let s_tangent_x = normalize(cross(s_axis, s_ref));
    let s_tangent_z = cross(s_tangent_x, s_axis);
    let p_tilted = vec3<f32>(dot(surface_dir, s_tangent_x), sin_lat, dot(surface_dir, s_tangent_z));
    
    let t = planet.time;
    let temp = planet.temperature;
    
    // 1. Solid surface coordinate
    // Bevy Quat and WGSL rotate_y are opposite handedness.
    // For planets, this rotates the texture over the static mesh.
    // For minor bodies (which physically rotate), this perfectly cancels the physical rotation!
    let p_surf = rotate_y(p_tilted, planet.spin_angle);

@if(BODY_CLASS_STAR_REMNANT) {
@if(PREPASS_PIPELINE) {
    return deferred_output(in, pbr_input);
} @else {
    var star_out: FragmentOutput;
    if (planet.planet_type == 7u) {
        let quasi_photosphere = render_quasistar_photosphere(p_surf, norm, pbr_input.V, t);
        star_out.color = vec4<f32>(quasi_photosphere, 1.0);
    } else if (planet.planet_type == 5u) {
        star_out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    } else {
        star_out.color = render_stellar_photosphere(p_surf, norm, pbr_input.V, t);
    }
    star_out.color = main_pass_post_lighting_processing(pbr_input, star_out.color);
    return star_out;
}
}


@if(BODY_CLASS_STAR_REMNANT || BODY_CLASS_GIANT) {
    pbr_input.N = norm;
} @else {
    // Evaluate high-frequency normal in fragment shader with tectonic micro-ridge relief
    // (Culled entirely for stars, gas/ice giants, black holes, and quasi-stars)
    let surf_fw = length(fwidth(p_surf * 16.0));
    let is_solid_crust = (planet.planet_type == 3u || planet.planet_type == 4u || planet.planet_type == 6u || planet.planet_type == 8u || planet.planet_type == 9u);
    if (is_solid_crust) {
        let eps_frag = 0.002;
        let micro_lod = 1.0 - smoothstep(0.16, 0.60, surf_fw) * 0.70;
        let p_dx = normalize(p_surf + vec3<f32>(eps_frag, 0.0, 0.0));
        let p_dy = normalize(p_surf + vec3<f32>(0.0, eps_frag, 0.0));
        let p_dz = normalize(p_surf + vec3<f32>(0.0, 0.0, eps_frag));

        var h_base = ridge_noise(p_surf * 16.0) * 0.0022 * micro_lod;
        var h_x = ridge_noise(p_dx * 16.0) * 0.0022 * micro_lod;
        var h_y = ridge_noise(p_dy * 16.0) * 0.0022 * micro_lod;
        var h_z = ridge_noise(p_dz * 16.0) * 0.0022 * micro_lod;

        if (planet.planet_type != 8u) {
            h_base += get_vertex_displacement(p_surf);
            h_x += get_vertex_displacement(p_dx);
            h_y += get_vertex_displacement(p_dy);
            h_z += get_vertex_displacement(p_dz);
        }

        let grad_spun = vec3<f32>(h_x - h_base, h_y - h_base, h_z - h_base) / eps_frag;
        let grad_tilted = rotate_y(grad_spun, -planet.spin_angle);
        let grad_world = grad_tilted.x * s_tangent_x + grad_tilted.y * s_axis + grad_tilted.z * s_tangent_z;
        let bump_scale = select(0.052, 0.085, planet.planet_type == 4u || planet.planet_type == 9u);
        pbr_input.N = normalize(surface_dir - grad_world * bump_scale);
    } else {
        pbr_input.N = norm;
    }
}

    
    // 2. Cloud and atmospheric coordinate with zonal trade winds
    let lat = p_tilted.y;
    let zonal_drift = sin(lat * 3.14159 * 2.0) * 0.15;
    let p_cloud = rotate_y(p_tilted, planet.spin_angle + t * 0.005 + zonal_drift);
    let p_cloud_sub = rotate_y(p_tilted, planet.spin_angle - t * 0.0035 - zonal_drift * 0.8);
    
    let rock = planet.composition.x;
    let ice = planet.composition.y;
    let metal = planet.composition.z;
    let gas = planet.composition.w;

    let has_volatiles = (ice > 0.002 || planet.climate_and_bio.x > 0.005 || planet.climate_and_bio.y > 0.005 || gas > 0.01);
    let ocean_frac = planet.climate_and_bio.x;
    let ice_frac = planet.climate_and_bio.y;
    let biomass = planet.climate_and_bio.z;
    let pressure_bar = planet.atmosphere_params.x;
    let has_water_volatiles = (ocean_frac > 0.005 || ice > 0.005 || ice_frac > 0.005);
    let cloud_density = select(
        0.0,
        max(planet.climate_and_bio.w, clamp(gas * 10.0, 0.0, 0.68)),
        pressure_bar >= 0.02 || gas > 0.005
    );
    let mag_gauss = planet.dynamics_and_mag.x;
    let lava_frac = planet.dynamics_and_mag.y;
    let geo_age = planet.geological_params.x;
    let drift_phase = planet.spin_axis.w;
    let aggregation = planet.geological_params.y;
    let ocean_oxidation = planet.geological_params.z;
    let vegetation_mult = planet.geological_params.w;

    var color = pbr_input.material.base_color.rgb;
    var water_mask: f32 = 0.0;
    var cloud_cover_alpha: f32 = 0.0;
    var cloud_shadow_transmittance: f32 = 1.0;

    var star_world_norm = planet.star_dir_and_lum.xyz;
    if (dot(star_world_norm, star_world_norm) < 0.01) {
        star_world_norm = to_star;
    } else {
        star_world_norm = normalize(star_world_norm);
    }
    let star_tilted = vec3<f32>(
        dot(star_world_norm, s_tangent_x),
        dot(star_world_norm, s_axis),
        dot(star_world_norm, s_tangent_z)
    );

@if(!BODY_CLASS_TERRESTRIAL && !BODY_CLASS_MINOR) {
    // =========================================================================
    // 1. Gas Giant (Jupiter / Saturn / Super-Jupiters / Hot Jupiters / Brown Dwarfs)
    // =========================================================================
    if (planet.planet_type == 1u) {
        let mass_jup = max(planet.dynamics_and_mag.z, 0.1);
        let lat_rad = asin(clamp(p_tilted.y, -1.0, 1.0));
        let abs_lat = abs(lat_rad);
        
        // Physical zonal jet wind profile:
        // Prograde equatorial super-rotation (+120 to +400 m/s) with alternating mid-latitude jets
        let eq_jet = exp(-pow(lat_rad / 0.18, 2.0)) * 1.8;
        let mid_jets = sin(lat_rad * (16.0 + min(mass_jup, 6.0) * 1.8)) * 0.70;
        let polar_damp = smoothstep(1.25, 0.75, abs_lat);
        let jet_stream = (eq_jet + mid_jets) * polar_damp * (t * 0.006);
        var p_gas = rotate_y(p_tilted, planet.spin_angle + jet_stream);

        // Atmospheric storm parameters from uniforms or procedural seed
        let hex_amp = planet.storm_features.x;
        let hex_k = planet.storm_features.y;
        var spot_size = planet.storm_features.z;
        var spot_lat = planet.storm_features.w;
        var spot_lon = planet.storm_dynamics.x;
        var vortex_spin = planet.storm_dynamics.y;
        var sec_ovals = planet.storm_dynamics.z;
        let shear_turb = max(planet.storm_dynamics.w, 0.65);

        // Procedural Great Spot & Ovals fallback for unconfigured gas giants (e.g. PLANET-5AU)
        if (spot_size <= 0.01 && hex_amp <= 0.001) {
            spot_size = 0.38;
            spot_lat = -0.384; // Canonical -22° Jovian anticyclone latitude
            // Deterministic longitude offset from planet color seed
            spot_lon = fract(planet.color_seed.r * 5.3 + planet.color_seed.g * 8.7) * 6.2831853;
            vortex_spin = 2.4;
            sec_ovals = 3.0;
        }

        // Dynamic vortex swirl distortion on atmospheric coordinates
        if (spot_size > 0.01) {
            let spot_y = sin(spot_lat);
            let spot_r = cos(spot_lat);
            let spot_center = vec3<f32>(spot_r * cos(spot_lon), spot_y, spot_r * sin(spot_lon));
            p_gas = vortex_swirl(p_gas, spot_center, spot_size * 1.35, vortex_spin * 0.80 + sin(t * 0.4) * 0.35);
        }

        // Secondary Counter-Rotating Anticyclone for Super-Jupiters (> 1.8 M_jup)
        if (mass_jup > 1.8 && hex_amp <= 0.001) {
            let spot2_center = vec3<f32>(-0.62, 0.35, -0.58);
            p_gas = vortex_swirl(p_gas, spot2_center, 0.34, -2.4 - cos(t * 0.4) * 0.6);
        }

        let g_lat = asin(clamp(p_gas.y, -1.0, 1.0));
        let g_abs_lat = abs(g_lat);

        // Identify canonical Saturn and Jupiter configurations for physical stratigraphy
        let seed = planet.color_seed.rgb;
        let is_saturn = (hex_amp > 0.001) || (planet.ring_shadow_params.w > 0.5) || (seed.r > 0.70 && seed.g > 0.55 && seed.b < 0.40 && seed.r > seed.b * 1.8);
        let is_jupiter = (!is_saturn) && (
            (spot_lat < -0.30 && spot_size > 0.30) ||
            (seed.r > 0.65 && seed.g > 0.35 && seed.g < 0.65 && seed.b < 0.30)
        );

        // Multi-tier anisotropic fluid turbulence with domain warping:
        // Physical zonal jets stretch turbulence along longitudes (x, z) while
        // maintaining rapid shear across latitudes (y).
        let p_zonal_1 = vec3<f32>(p_gas.x * 2.0, p_gas.y * 8.0, p_gas.z * 2.0);
        let p_zonal_2 = vec3<f32>(p_gas.x * 5.0, p_gas.y * 20.0, p_gas.z * 5.0);
        let p_zonal_3 = vec3<f32>(p_gas.x * 12.0, p_gas.y * 48.0, p_gas.z * 12.0);
        let p_zonal_4 = vec3<f32>(p_gas.x * 26.0, p_gas.y * 104.0, p_gas.z * 26.0);

        let flow_coarse = fbm_lod(p_zonal_1 + vec3<f32>(t * 0.015, 0.0, -t * 0.01));
        
        // Zonal shear advects eddies horizontally along jet boundaries
        let warp_mid = vec3<f32>((flow_coarse - 0.5) * 1.5, 0.0, (flow_coarse - 0.5) * 1.5);
        let flow_mid = fbm_lod(p_zonal_2 + warp_mid + vec3<f32>(-t * 0.025, 0.0, t * 0.015));

        // High-octave Kelvin-Helmholtz shear billows and curling filaments
        let warp_fine = vec3<f32>((flow_mid - 0.5) * 2.4, (flow_coarse - 0.5) * 0.45, (flow_mid - 0.5) * 2.4);
        let flow_fine = fbm_lod(p_zonal_3 + warp_fine + vec3<f32>(t * 0.04, 0.0, 0.0));

        // Close-up micro-convective cloud texture (LOD derivative filtered)
        let nano_lod = 1.0 - smoothstep(0.015, 0.12, length(fwidth(p_gas * 35.0)));
        let flow_ultra = noise(p_zonal_4 + warp_fine * 2.0) * nano_lod;

        // Zonal frequency: Saturn has finer, denser striations (26 bands); Jupiter has ~15 major belts/zones
        let band_freq = select(
            14.0 + min(mass_jup, 6.0) * 1.4,
            26.0,
            is_saturn
        );

        // Screen-space band frequency derivative: smoothstep fade prevents high-frequency Moiré shimmer
        let band_pixel_deriv = length(fwidth(g_lat * band_freq));
        let band_lod = 1.0 - smoothstep(0.35, 1.10, band_pixel_deriv);

        // Fluid domain warping of latitude creates dynamic meandering jet streams
        let warp_lat = g_lat + ((flow_coarse - 0.5) * 0.14 * shear_turb + (flow_mid - 0.5) * 0.05 * shear_turb) * band_lod;

        // Kelvin-Helmholtz shear instabilities along jet boundaries
        let kh_shear = ((flow_mid - 0.5) * 0.35 * shear_turb + (flow_fine - 0.5) * 0.14 * shear_turb) * band_lod;

        // Equatorial Festoon plumes: dark loops extending from the tropical belt into equatorial zone
        let in_festoon_lat = smoothstep(0.20, 0.06, g_lat) * smoothstep(-0.02, 0.06, g_lat);
        let festoon_flow = sin(p_gas.x * 12.0 + flow_mid * 3.2) * cos(p_gas.z * 12.0);
        let festoon_mask = in_festoon_lat * smoothstep(0.36, 0.72, flow_coarse + festoon_flow * 0.36) * band_lod;

        // Crisp, anti-aliased latitudinal banding (sharper step transitions when close, filtered when distant):
        let s_prim = sin(warp_lat * band_freq + kh_shear);
        let b_primary = mix(0.5, smoothstep(-0.48, 0.48, s_prim), band_lod);

        // Mid-frequency harmonic for secondary belts and subtle sub-divisions (NEBn/NEBs, SEBn/SEBs)
        let b_harmonic = sin(warp_lat * band_freq * 2.0 + flow_mid * 1.4) * 0.22 * band_lod;

        // Anti-aliased micro-striations for close-up fidelity (derivative-filtered)
        let b_micro = sin(warp_lat * band_freq * 4.2 + flow_fine * 0.8) * 0.09 * nano_lod * band_lod;

        // Fine convective eddy billows along shear margins
        let b_filament = (flow_ultra - 0.5) * 0.10 * nano_lod * band_lod;

        var raw_bands = clamp(b_primary * 0.62 + 0.19 + b_harmonic + b_micro + b_filament, 0.0, 1.0);

        // Circumpolar vortex transition:
        // At latitudes > 64° (|g_lat| > 1.12 rad), linear zonal jet streams dissolve into
        // chaotic circumpolar cyclonic swirls and the polar hood, eliminating concentric target rings!
        let polar_transition = smoothstep(1.05, 1.34, g_abs_lat);
        if (polar_transition > 0.0) {
            let circumpolar_cyclones = fbm(vec3<f32>(p_gas.x * 8.0, p_gas.y * 4.0, p_gas.z * 8.0) + vec3<f32>(0.0, t * 0.02, 0.0));
            raw_bands = mix(raw_bands, circumpolar_cyclones, polar_transition * 0.75);
        }
        let band_composite = raw_bands;

        // ---------------------------------------------------------------------
        // Sudarsky Atmospheric Strata Palette Synthesizer
        // ---------------------------------------------------------------------
        var c_zone: vec3<f32>;
        var c_belt_light: vec3<f32>;
        var c_belt_dark: vec3<f32>;
        var c_rift: vec3<f32>;
        var c_polar: vec3<f32>;
        var c_festoon: vec3<f32>;

        if (temp >= 1400.0) {
            // Sudarsky Class V: Silicate Vapor / Ultra-Hot Jovian
            c_zone       = vec3<f32>(1.0, 0.90, 0.72);
            c_belt_light = vec3<f32>(0.95, 0.52, 0.16);
            c_belt_dark  = vec3<f32>(0.42, 0.14, 0.06);
            c_rift       = vec3<f32>(1.25, 0.72, 0.20); // Incandescent silicate fissure
            c_polar      = vec3<f32>(0.72, 0.32, 0.10);
            c_festoon    = vec3<f32>(0.65, 0.24, 0.08);
        } else if (temp > 800.0) {
            // Sudarsky Class IV: Alkali Vapor / Hot Jovian (Smoked Bronze & Titanium)
            c_zone       = vec3<f32>(0.52, 0.32, 0.20);
            c_belt_light = vec3<f32>(0.38, 0.20, 0.10);
            c_belt_dark  = vec3<f32>(0.18, 0.08, 0.04);
            c_rift       = vec3<f32>(0.85, 0.28, 0.06); // Nightside thermal rift
            c_polar      = vec3<f32>(0.26, 0.20, 0.18);
            c_festoon    = vec3<f32>(0.28, 0.14, 0.08);
        } else if (is_saturn) {
            // Authentic Saturnian Strata (Cassini-calibrated butterscotch, golden-sand & warm ochre)
            // Ammonia cirrus cloud zones: pale warm ivory cream
            c_zone       = vec3<f32>(0.96, 0.92, 0.80);
            // Mid-level cloud deck: rich warm honey butterscotch
            c_belt_light = vec3<f32>(0.88, 0.78, 0.56);
            // Chromophore-rich downwelling belts: warm caramel amber-ochre
            c_belt_dark  = vec3<f32>(0.70, 0.54, 0.34);
            // Deep sub-cloud thermal rift lanes: roasted chestnut
            c_rift       = vec3<f32>(0.48, 0.34, 0.18);
            // North Polar Hood / Hexagon surrounds: warm olive-gold to smoky slate
            c_polar      = vec3<f32>(0.56, 0.52, 0.38);
            // Subtle equatorial festoons
            c_festoon    = vec3<f32>(0.52, 0.46, 0.36);
        } else if (is_jupiter) {
            // Authentic Jovian Strata (Juno/Cassini-calibrated warm ivory zones, rich terracotta belts)
            // Crisp warm cream ammonia ice upwelling:
            c_zone       = vec3<f32>(0.95, 0.92, 0.84);
            // Warm honey-amber mid-level deck (tropical belts & rift margins):
            c_belt_light = vec3<f32>(0.86, 0.68, 0.46);
            // Rich warm terracotta/mahogany chromophore belts (NEB/SEB):
            c_belt_dark  = vec3<f32>(0.66, 0.34, 0.16);
            // Scorched dark umber rift trenches & barges:
            c_rift       = vec3<f32>(0.36, 0.16, 0.06);
            // Muted smoky olive-slate polar hood:
            c_polar      = vec3<f32>(0.42, 0.44, 0.42);
            // Striking dark slate-cyan equatorial festoons:
            c_festoon    = vec3<f32>(0.24, 0.36, 0.48);
        } else if (temp >= 360.0) {
            // Sudarsky Class III: Cloudless Azure / Methane & Rayleigh Scattering
            c_zone       = vec3<f32>(0.22, 0.56, 0.90);
            c_belt_light = vec3<f32>(0.14, 0.38, 0.74);
            c_belt_dark  = vec3<f32>(0.08, 0.20, 0.48);
            c_rift       = vec3<f32>(0.04, 0.12, 0.32);
            c_polar      = vec3<f32>(0.18, 0.16, 0.45);
            c_festoon    = vec3<f32>(0.10, 0.28, 0.60);
        } else if (temp >= 160.0) {
            // Sudarsky Class II: Water Cloud Jovian (Dense tropospheric water ice)
            c_zone       = vec3<f32>(0.96, 0.97, 0.98);
            c_belt_light = mix(vec3<f32>(0.84, 0.86, 0.88), seed * 1.10, 0.45);
            c_belt_dark  = mix(vec3<f32>(0.42, 0.52, 0.65), seed * 0.70, 0.45);
            c_rift       = mix(vec3<f32>(0.18, 0.25, 0.38), seed * 0.40, 0.45);
            c_polar      = mix(vec3<f32>(0.55, 0.66, 0.78), seed * 0.55, 0.45);
            c_festoon    = mix(vec3<f32>(0.32, 0.44, 0.58), seed * 0.50, 0.45);
        } else {
            // Sudarsky Class I: Ammonia Ice Clouds (< 160 K: general cold gas giants)
            c_zone       = mix(vec3<f32>(0.96, 0.94, 0.88), seed * 1.30, 0.25);
            c_belt_light = seed * 1.05 + vec3<f32>(0.04, 0.02, -0.02);
            c_belt_dark  = vec3<f32>(seed.r * 0.90 + 0.08, seed.g * 0.52, seed.b * 0.30);
            c_rift       = vec3<f32>(seed.r * 0.50 + 0.04, seed.g * 0.26, seed.b * 0.14);
            c_polar      = mix(vec3<f32>(0.26, 0.34, 0.44), seed * 0.60, 0.35);
            c_festoon    = vec3<f32>(0.22, 0.34, 0.46);
        }

        // Multi-tier crisp strata blending with smoothstep transitions
        var c3 = c_belt_light;
        if (band_composite < 0.26) {
            let t_ramp = smoothstep(0.0, 1.0, band_composite / 0.26);
            c3 = mix(c_rift, c_belt_dark, t_ramp);
        } else if (band_composite < 0.54) {
            let t_ramp = smoothstep(0.0, 1.0, (band_composite - 0.26) / 0.28);
            c3 = mix(c_belt_dark, c_belt_light, t_ramp);
        } else {
            let t_ramp = smoothstep(0.0, 1.0, (band_composite - 0.54) / 0.46);
            c3 = mix(c_belt_light, c_zone, t_ramp);
        }

        // Equatorial festoons
        if (festoon_mask > 0.01) {
            let f_int = select(0.85, 0.35, is_saturn);
            c3 = mix(c3, c_festoon, festoon_mask * f_int);
        }

        // Polar hood transitions and circumpolar vortices
        if (polar_transition > 0.0) {
            let polar_eddy = fbm(p_gas * 10.0 + vec3<f32>(0.0, t * 0.025, 0.0));
            let polar_pattern = mix(c_polar, c_belt_dark, smoothstep(0.38, 0.72, polar_eddy) * 0.35);
            c3 = mix(c3, polar_pattern, polar_transition * 0.85);
        }

        // Evaluate primary anticyclonic storm (Great Red Spot / Dark Spot) and secondary ovals
        if (spot_size > 0.01) {
            let vortex_res = evaluate_anticyclonic_vortices(
                p_gas, t, spot_size, spot_lat, spot_lon, vortex_spin, sec_ovals, mass_jup, is_saturn
            );
            if (vortex_res.is_active) {
                c3 = mix(c3, vortex_res.color, vortex_res.spot_mask * 0.96);
                c3 = mix(c3, c_rift, vortex_res.wake_mask * 0.75);
                // White ovals with dark collar
                let oval_core = smoothstep(0.35, 0.90, vortex_res.oval_mask);
                let oval_rim = smoothstep(0.05, 0.35, vortex_res.oval_mask) * (1.0 - oval_core);
                c3 = mix(c3, c_rift, oval_rim * 0.70);
                c3 = mix(c3, vec3<f32>(0.99, 0.98, 0.95), oval_core * 0.95);
            }
        }

        // Evaluate standing Rossby wave polar hexagon (Saturn's North Polar Hexagon)
        if (hex_amp > 0.001) {
            let hex_res = evaluate_polar_hexagon(p_gas, t, hex_amp, hex_k);
            if (hex_res.is_active) {
                let hex_blend = max(hex_res.inside_mask * 0.88, hex_res.jet_mask * 0.96);
                c3 = mix(c3, hex_res.color, hex_blend);
                if (hex_res.eye_mask > 0.0) {
                    c3 = mix(c3, vec3<f32>(0.24, 0.12, 0.05), hex_res.eye_mask * 0.95);
                }
            }
        }

        // Strings of Pearls: chains of high-pressure anticyclonic white storm ovals along temperate belts
        let temperate_band = exp(-pow((g_abs_lat - 0.62) / 0.06, 2.0));
        let lon_rad = atan2(p_gas.z, p_gas.x);
        let pearl_phase = sin(lon_rad * 8.0 + t * 0.08);
        let pearl_rim = temperate_band * smoothstep(0.68, 0.84, pearl_phase);
        let pearl_oval = temperate_band * smoothstep(0.84, 0.98, pearl_phase);
        c3 = mix(c3, c_rift, pearl_rim * 0.40);
        c3 = mix(c3, vec3<f32>(0.99, 0.98, 0.96), pearl_oval * 0.90);

        // 3D Multi-Deck Cloud Stratigraphy Shadows & Normal Relief on Gas Giants:
        // High-altitude white ammonia zones (band_composite > 0.54) and storm collars stand above
        // the deeper terracotta/umber belts, casting subtle drop shadows and catching grazing terminator light.
        let sun_lat_grad = cos(warp_lat * band_freq + kh_shear) * band_freq * star_tilted.y * band_lod;
        let belt_shadow = clamp(-sun_lat_grad * 0.016, 0.0, 0.22) * (1.0 - smoothstep(0.35, 0.68, band_composite));
        let zone_sun_ridge = clamp(sun_lat_grad * 0.012, 0.0, 0.14) * smoothstep(0.45, 0.80, band_composite);
        c3 = c3 * (1.0 - belt_shadow + zone_sun_ridge);
        pbr_input.N = normalize(surface_dir - s_axis * (cos(warp_lat * band_freq + kh_shear) * 0.022 * band_lod));

        // Atmospheric Rayleigh/Mie limb brightening (soft luminous haze along the limb)
        let NdotV = max(dot(norm, pbr_input.V), 0.0);
        let limb_factor = pow(1.0 - NdotV, 3.8);
        let limb_glow = mix(c_zone, c_polar, 0.50) * 0.40;
        c3 = mix(c3, limb_glow, limb_factor * 0.38);

        color = c3;

        // Thermal night-side infrared emission for Brown Dwarfs and ultra-hot Super-Jupiters
        if (mass_jup > 10.0 || temp > 800.0) {
            let thermal_intensity = clamp((temp - 800.0) / 1000.0, 0.0, 2.5) + max(mass_jup - 10.0, 0.0) * 0.2;
            let rift_thermal = smoothstep(0.55, 0.95, 1.0 - band_composite) * smoothstep(0.35, 0.85, flow_coarse);
            let thermal_color = mix(vec3<f32>(1.0, 0.22, 0.04), vec3<f32>(1.0, 0.65, 0.15), clamp((temp - 1200.0) / 800.0, 0.0, 1.0));
            color += thermal_color * rift_thermal * (1.5 * thermal_intensity);
        }
    }
    // =========================================================================
    // 2. Ice Giant (Uranus / Neptune / Sub-Neptunes)
    // =========================================================================
    if (planet.planet_type == 2u) {
        let jet_stream = sin(lat * 10.0) * (t * 0.012);
        let p_ice_gas = rotate_y(p_tilted, planet.spin_angle + jet_stream);
        let swirl = fbm_lod(p_ice_gas * 4.5 + vec3<f32>(t * 0.004, 0.0, t * 0.002));
        let ice_band_deriv = length(fwidth(lat * 8.0));
        let ice_band_lod = 1.0 - smoothstep(0.35, 1.10, ice_band_deriv);
        let band = mix(0.5, sin(lat * 8.0 + swirl * (1.2 * ice_band_lod)) * 0.5 + 0.5, ice_band_lod);
        
        let deep_cyan = vec3<f32>(0.08, 0.42, 0.75);
        let bright_azure = vec3<f32>(0.32, 0.72, 0.96);
        let methane_veil = mix(deep_cyan, bright_azure, band);
        
        // High-altitude methane cirrus clouds with fast prograde drift & Voyager-2 drop shadows onto lower deck
        let cirrus_coord = rotate_y(p_tilted, planet.spin_angle + t * 0.008 + jet_stream * 1.5);
        let cirrus = fbm_lod(cirrus_coord * 14.0);
        let white_clouds = smoothstep(0.65, 0.85, cirrus) * ice_band_lod;

        let ice_ndotl = dot(p_tilted, star_tilted);
        let ice_sun_tangent = (star_tilted - p_tilted * ice_ndotl) / clamp(ice_ndotl + 0.10, 0.18, 1.0);
        let cirrus_sh_coord = rotate_y(normalize(p_tilted + ice_sun_tangent * 0.011), planet.spin_angle + t * 0.008 + jet_stream * 1.5);
        let cirrus_sh = smoothstep(0.63, 0.84, fbm_lod(cirrus_sh_coord * 14.0)) * ice_band_lod * smoothstep(-0.04, 0.16, ice_ndotl);
        let shadowed_methane_veil = methane_veil * (1.0 - cirrus_sh * (1.0 - white_clouds) * 0.42);
        
        var base_ice = mix(shadowed_methane_veil, vec3<f32>(0.94, 0.97, 1.0), white_clouds * 0.62);

        // Neptune's Great Dark Spot and companion cirrus "Scooter"
        let spot_size = planet.storm_features.z;
        if (spot_size > 0.01) {
            let dark_spot = evaluate_anticyclonic_vortices(
                p_ice_gas, t, spot_size, planet.storm_features.w,
                planet.storm_dynamics.x, planet.storm_dynamics.y, planet.storm_dynamics.z, 0.8, false
            );
            if (dark_spot.is_active) {
                // Deep royal-indigo / navy vortex
                let dark_spot_col = vec3<f32>(0.02, 0.08, 0.24);
                base_ice = mix(base_ice, dark_spot_col, dark_spot.spot_mask * 0.88);
                // Companion bright white methane cirrus clouds ("Scooter")
                let scooter_clouds = dark_spot.collar_mask * fbm(p_ice_gas * 28.0 + vec3<f32>(t * 0.2, 0.0, 0.0));
                base_ice = mix(base_ice, vec3<f32>(0.98, 0.99, 1.0), smoothstep(0.40, 0.80, scooter_clouds) * 0.92);
            }
        }

        color = base_ice;
    }
}
@if(!BODY_CLASS_GIANT && !BODY_CLASS_MINOR) {
    // =========================================================================
    // 6. Super-Earth (Dedicated Mega-Terrestrial World: Vast Oceans, Continents, Storms)
    // =========================================================================
    if (planet.planet_type == 6u) {
        // High surface gravity and massive lithosphere produce sprawling mega-continents,
        // deep abyssal oceans, towering folded cordilleras, and intense cyclonic weather fronts.
        let plate_drift = vec3<f32>(
            sin(drift_phase * 6.283 + p_surf.z * 2.6) * 0.28,
            cos(drift_phase * 3.14159 + p_surf.x * 2.2) * 0.12,
            sin(drift_phase * 4.712 + p_surf.y * 2.4) * 0.28
        );
        let p_tectonic = p_surf + plate_drift;
        let elev = eroded_continent_fbm_lod(p_tectonic * 3.4);
        let ridge = ridge_noise(p_tectonic * 7.2);

        let land_weight = smoothstep(0.28, 0.50, elev);
        let oceanic_basin = 1.0 - land_weight;
        let mid_ocean_ridge = ridge_noise(p_tectonic * 7.5 + vec3<f32>(2.1, 1.4, 0.9)) * 0.11;
        let axial_rift = (1.0 - ridge_noise(p_tectonic * 15.0 + vec3<f32>(2.1, 1.4, 0.9))) * 0.03;
        let trench_zone = smoothstep(0.05, 0.0, abs(elev - 0.38) - 0.02);
        let trench_depth = trench_zone * 0.10;

        let has_oceans = has_volatiles && ocean_frac > 0.01 && temp <= 380.0 && pressure_bar >= 0.006;
        let sea_level = clamp(0.18 + ocean_frac * 0.45, 0.20, 0.80);
        let primordial_shelf_elev = 0.48;

        let continental_elev = elev * 0.65 + ridge * 0.35 * (0.20 + 0.80 * land_weight);
        let ocean_base = sea_level - 0.24;
        let bathymetry_elev = ocean_base + (mid_ocean_ridge - axial_rift) - trench_depth;

        let combined_elev = select(
            continental_elev + oceanic_basin * (mid_ocean_ridge - axial_rift) - trench_depth,
            mix(bathymetry_elev, continental_elev, land_weight),
            has_oceans
        );
        let has_volatile_ice = has_volatiles && (ice_frac > 0.005 || ocean_frac > 0.005);
        let is_ice_cold = temp < 320.0 && has_volatile_ice;
        let base_ice_thresh = clamp(1.0 - ice_frac * 1.05, 0.0, 0.98);
        let is_tidally_locked = planet.tidal_deformation.w >= 1.5;
        let substellar_cos = dot(norm, to_star);

        // Seasonal Milankovitch orbital insolation shift
        let season_subsolar = dot(planet.star_dir_and_lum.xyz, s_axis);
        let ice_thresh_north = clamp(base_ice_thresh + season_subsolar * 0.15, 0.0, 1.0);
        let ice_thresh_south = clamp(base_ice_thresh - season_subsolar * 0.15, 0.0, 1.0);

        let ice_edge_noise = fbm_lod(p_surf * 10.0) * 0.06;
        let in_north_cap = (p_tilted.y > 0.0) && (polar_angle + ice_edge_noise > ice_thresh_north);
        let in_south_cap = (p_tilted.y < 0.0) && (polar_angle + ice_edge_noise > ice_thresh_south);
        let eyeball_ice = is_tidally_locked && has_volatile_ice && (temp < 345.0) && ((substellar_cos + ice_edge_noise) < (-0.15 + clamp(ice_frac - 0.15, -0.25, 0.55)));
        let in_ice_shield = has_volatile_ice && select(is_ice_cold && (in_north_cap || in_south_cap), eyeball_ice || (is_ice_cold && (in_north_cap || in_south_cap)), is_tidally_locked);

        // Polar / Eyeball Nightside Ice Shields & Glacial Calving Shelves:
        if (in_ice_shield) {
            let frost = fbm(p_surf * 14.0);
            let pack_ice = vec3<f32>(0.94, 0.97, 1.0);
            let glacial_blue = vec3<f32>(0.68, 0.84, 0.98);
            color = mix(pack_ice, glacial_blue, frost * 0.40);
            pbr_input.material.perceptual_roughness = 0.25;
            pbr_input.material.metallic = 0.0;
        }
        // Vast Sapphire Oceans & Coastal Turquoise Continental Shelves (ONLY if world has oceans!)
        else if (has_oceans && combined_elev < sea_level) {
            water_mask = clamp((sea_level - combined_elev) / 0.03, 0.002, 1.0);
            let depth = clamp((sea_level - combined_elev) / max(sea_level, 0.08), 0.0, 1.0);
            let abyssal_trench = vec3<f32>(0.005, 0.015, 0.10);
            let deep_sapphire = vec3<f32>(0.015, 0.06, 0.24);
            let mid_ocean = vec3<f32>(0.03, 0.20, 0.48);
            let continental_shelf = vec3<f32>(0.07, 0.50, 0.68);
            let coastal_lagoon = vec3<f32>(0.12, 0.74, 0.84);

            let submerged_ridge_glow = oceanic_basin * smoothstep(0.07, 0.14, mid_ocean_ridge) * (1.0 - smoothstep(0.0, 0.42, depth));
            
            var modern_ocean = mix(coastal_lagoon, continental_shelf, smoothstep(0.0, 0.12, depth));
            modern_ocean = mix(modern_ocean, mid_ocean, smoothstep(0.12, 0.35, depth));
            modern_ocean = mix(modern_ocean, deep_sapphire, smoothstep(0.35, 0.75, depth));
            modern_ocean = mix(modern_ocean, abyssal_trench, smoothstep(0.75, 1.0, depth));
            modern_ocean = mix(modern_ocean, continental_shelf * 1.15, submerged_ridge_glow * 0.45);

            // Thermal boiling frothing and steam venting at T > 340 K (flash evaporation)
            if (temp > 340.0) {
                let boil_heat = clamp((temp - 340.0) / 45.0, 0.0, 1.0);
                let boil_bubbles = fbm(p_surf * 45.0 + vec3<f32>(0.0, t * 0.7, 0.0));
                let boil_froth = smoothstep(0.38, 0.72, boil_bubbles) * boil_heat;
                let shore_steam = smoothstep(sea_level - 0.03, sea_level, combined_elev) * boil_heat;
                let steam_white = vec3<f32>(0.94, 0.96, 1.0);
                modern_ocean = mix(modern_ocean, steam_white, clamp(boil_froth * 0.75 + shore_steam * 0.60, 0.0, 1.0));
            }

            let shore_blend = smoothstep(sea_level - 0.025, sea_level, combined_elev);
            let archean_iron_sea = mix(vec3<f32>(0.07, 0.24, 0.16), vec3<f32>(0.02, 0.09, 0.06), clamp(depth * 2.4, 0.0, 1.0));
            color = mix(archean_iron_sea, modern_ocean, clamp(ocean_oxidation, 0.0, 1.0));
            
            // Specular Liquid Glint
            pbr_input.material.perceptual_roughness = mix(0.05, 0.15, shore_blend);
            pbr_input.material.metallic = 0.02;
        }
        // Continents, Mountain Ranges & Biomes (or Exposed Bathymetry & Dry Super-Earth Lithosphere)
        else {
            let terrain_var = fbm(p_surf * 8.5);
            let is_exposed_seabed = !has_oceans && (combined_elev < primordial_shelf_elev);

            if (is_exposed_seabed) {
                let seabed_depth = (primordial_shelf_elev - combined_elev) / (primordial_shelf_elev - 0.18);
                let shelf_sand = vec3<f32>(0.65, 0.60, 0.48);
                let dried_mud = vec3<f32>(0.42, 0.36, 0.28);
                let abyssal_basalt = vec3<f32>(0.16, 0.14, 0.13);
                let salt_flat = vec3<f32>(0.88, 0.90, 0.92);
                let ridge_fissure = oceanic_basin * smoothstep(0.07, 0.14, mid_ocean_ridge);
                let hydrothermal_sulfide = vec3<f32>(0.56, 0.26, 0.12);
                let salt_mask = smoothstep(0.45, 0.70, seabed_depth) * smoothstep(0.55, 0.40, mid_ocean_ridge);

                var seabed_color = mix(shelf_sand, dried_mud, smoothstep(0.0, 0.25, seabed_depth));
                seabed_color = mix(seabed_color, abyssal_basalt, smoothstep(0.25, 0.65, seabed_depth));
                seabed_color = mix(seabed_color, salt_flat, salt_mask * 0.75);
                seabed_color = mix(seabed_color, hydrothermal_sulfide, ridge_fissure * 0.55);

                if (temp > 340.0) {
                    let dry_heat = clamp((temp - 340.0) / 45.0, 0.0, 1.0);
                    seabed_color = mix(seabed_color, salt_flat * 0.95, dry_heat * 0.35);
                }

                color = seabed_color;
                pbr_input.material.perceptual_roughness = mix(0.70, 0.88, seabed_depth);
                pbr_input.material.metallic = select(0.02, 0.12, ridge_fissure > 0.4);
            } else if (has_oceans) {
                let rel_elev = (combined_elev - sea_level) / (1.0 - sea_level);
                pbr_input.material.perceptual_roughness = 0.84;
                
                // Biome Palette
                let rainforest = vec3<f32>(0.08, 0.46, 0.16); // Lush emerald jungle
                let savanna = vec3<f32>(0.34, 0.58, 0.22);    // Verdant grassland
                let temperate_forest = vec3<f32>(0.14, 0.40, 0.18); // Mixed woodland
                let steppe = vec3<f32>(0.58, 0.52, 0.35);      // Golden-tan plains
                let mountain_basalt = vec3<f32>(0.32, 0.30, 0.28); // Jagged rock
                let snow_peaks = vec3<f32>(0.92, 0.95, 1.0);   // Snowcaps
                
                var land_color = vec3<f32>(0.0);
                if (rel_elev > 0.42) {
                    // Alpine mountain ranges with glacier crowns
                    land_color = mix(mountain_basalt, snow_peaks, smoothstep(0.42, 0.72, rel_elev));
                } else if (rel_elev > 0.22) {
                    // Highland plateau
                    land_color = mix(temperate_forest, mountain_basalt, (rel_elev - 0.22) * 5.0);
                } else if (polar_angle > 0.52) {
                    // High-latitude tundra & boreal forest
                    land_color = mix(temperate_forest, steppe, terrain_var);
                } else if (polar_angle < 0.24) {
                    // Equatorial mega-rainforest belt
                    land_color = mix(rainforest, savanna, terrain_var * 0.5);
                } else {
                    // Temperate fertile plains & woodlands
                    land_color = mix(savanna, temperate_forest, terrain_var);
                }
                color = land_color;
            } else {
                // Dry Super-Earth: Sprawling arid lithosphere, volcanic basalt plains, terracotta plateaus
                let rel_elev = combined_elev;
                pbr_input.material.perceptual_roughness = 0.88;
                pbr_input.material.metallic = 0.12;
                
                let volcanic_basalt = vec3<f32>(0.16, 0.14, 0.13); // Dark solidified mare / rift valley
                let ironstone_lowland = vec3<f32>(0.28, 0.22, 0.17); // Lowland depression
                let terracotta_plateau = vec3<f32>(0.48, 0.34, 0.22); // Oxidized desert highlands
                let craggy_cordillera = vec3<f32>(0.60, 0.48, 0.36); // Folded mountain ridges
                let granite_summit = vec3<f32>(0.68, 0.64, 0.58);   // Silicate granite peaks
                
                var rock_color = vec3<f32>(0.0);
                if (rel_elev < 0.35) {
                    rock_color = mix(volcanic_basalt, ironstone_lowland, rel_elev / 0.35);
                } else if (rel_elev < 0.60) {
                    let t_mid = (rel_elev - 0.35) / 0.25;
                    rock_color = mix(ironstone_lowland, terracotta_plateau, t_mid + terrain_var * 0.2 - 0.1);
                } else if (rel_elev < 0.82) {
                    let t_high = (rel_elev - 0.60) / 0.22;
                    rock_color = mix(terracotta_plateau, craggy_cordillera, t_high);
                } else {
                    let t_summit = (rel_elev - 0.82) / 0.18;
                    rock_color = mix(craggy_cordillera, granite_summit, clamp(t_summit, 0.0, 1.0));
                }
                color = rock_color;
            }
        }
        
        // Massive 3-Layer Atmospheric Cloud Circulation, Coriolis Cyclones, Cast Shadows & Runaway Steam Deck
        if ((has_volatiles || gas > 0.05 || ocean_frac > 0.005) && (pressure_bar >= 0.02 || gas > 0.15)) {
            let boil_steam_factor = clamp((temp - 345.0) / 35.0, 0.0, 1.0) * select(0.0, 1.0, has_water_volatiles);
            let effective_cloud_density = mix(cloud_density, 1.0, boil_steam_factor);
            let substellar_cloud_boost = select(0.0, smoothstep(-0.10, 0.65, substellar_cos) * 0.08, is_tidally_locked);

            if (effective_cloud_density > 0.02) {
                let cloud_res = evaluate_multilayer_clouds_and_shadows(
                    p_tilted,
                    surface_dir,
                    pbr_input.V,
                    star_world_norm,
                    s_tangent_x,
                    s_axis,
                    s_tangent_z,
                    planet.spin_angle,
                    t,
                    effective_cloud_density,
                    boil_steam_factor,
                    substellar_cloud_boost,
                    planet.color_seed.rgb,
                    true,
                );
                let cam_alt_ratio = length(view.world_position - planet.planet_center_and_radius.xyz)
                    / max(planet.planet_center_and_radius.w, 1e-6);
                let orbital_cloud_vis = smoothstep(1.15, 1.42, cam_alt_ratio);
                let surface_cloud_alpha = cloud_res.total_alpha * orbital_cloud_vis;

                // Apply cast 3D cloud shadows across both continents and oceans before compositing cloud decks
                let exposed_shadow = mix(cloud_res.ground_shadow_mult, 1.0, surface_cloud_alpha * 0.76);
                color = color * exposed_shadow;
                cloud_shadow_transmittance = exposed_shadow;

                // Sub-cloud lower-troposphere Rayleigh aerial perspective so the surface & ground shadows
                // visibly sit beneath a cushion of blue atmospheric air while the clouds float above
                let sub_ndotv = clamp(dot(surface_dir, pbr_input.V), 0.16, 1.0);
                let sub_day = smoothstep(-0.14, 0.26, dot(surface_dir, star_world_norm));
                let sub_air_veil = clamp((1.0 / sub_ndotv - 0.82) * 0.14 * clamp(pressure_bar, 0.0, 2.2), 0.0, 0.32) * orbital_cloud_vis;
                let sub_rayleigh_col = vec3<f32>(0.24, 0.52, 0.92) * sub_day;
                color = mix(color, color * 0.72 + sub_rayleigh_col * 0.55, sub_air_veil * (1.0 - surface_cloud_alpha * 0.85));

                if (surface_cloud_alpha > 0.005) {
                    cloud_cover_alpha = max(cloud_cover_alpha, surface_cloud_alpha);
                    color = mix(color, cloud_res.cloud_color, surface_cloud_alpha);
                    pbr_input.material.perceptual_roughness = mix(
                        pbr_input.material.perceptual_roughness,
                        cloud_res.cloud_roughness,
                        surface_cloud_alpha
                    );
                    pbr_input.N = normalize(mix(pbr_input.N, cloud_res.cloud_normal_world, surface_cloud_alpha * 0.75));
                }
            }
        }

        // Super-Earth Magma Ocean & Solidification Overlay
        if (temp >= 1100.0 || lava_frac > 0.65) {
            let magma = evaluate_magma_ocean_surface(p_surf, combined_elev, temp, lava_frac, t);
            color = magma.color;
            pbr_input.material.emissive = vec4<f32>(magma.emissive, 1.0);
            pbr_input.material.perceptual_roughness = magma.roughness;
            pbr_input.material.metallic = magma.metallic;
        } else if (temp > 450.0 || lava_frac > 0.02) {
            let magma = evaluate_magma_ocean_surface(p_surf, combined_elev, temp, lava_frac, t);
            let magma_blend = clamp((temp - 450.0) / 650.0 + lava_frac * 0.5, 0.0, 1.0);
            let fissure_mask = 1.0 - magma.crust_mask;
            let active_vent = clamp(magma_blend * 0.70 + fissure_mask * 0.88, 0.0, 1.0);
            color = mix(color, magma.color, active_vent);
            pbr_input.material.emissive = vec4<f32>(
                pbr_input.material.emissive.rgb + magma.emissive * (magma_blend * 0.60 + fissure_mask * 0.75),
                1.0
            );
            pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, magma.roughness, active_vent * 0.5);
        }
    }
}
    // =========================================================================
@if(!BODY_CLASS_GIANT && !BODY_CLASS_TERRESTRIAL) {
    // =========================================================================
    // 4. Minor Bodies (Asteroids, Comets, Planetesimals)
    // =========================================================================
    if (planet.planet_type == 4u) {
        let regolith = fbm(p_surf * 14.0);
        let micro_craters = fbm(p_surf * 22.0);
        let boulder_noise = fbm(p_surf * 38.0);
        let variegation = fbm(p_surf * 5.5);
        let base_tint = planet.color_seed.rgb;

        let is_comet = ice > 0.22 || (ice > 0.06 && base_tint.r < 0.08);

        if (is_comet) {
            // Pristine ultra-dark sublimation mantle (albedo 0.03 - 0.05) with organic tholin variegation
            let tholin_crust = mix(base_tint * 0.80, base_tint * 1.35, variegation);
            
            // Fresh exposed crystalline volatile ice cliffs on shadowed scarps & crater walls
            let ice_fissures = fbm(p_surf * 18.0 + vec3<f32>(0.5, -0.2, 0.3));
            let ice_scarps = smoothstep(0.68, 0.88, ice_fissures);
            let bright_ice = vec3<f32>(0.72, 0.84, 0.96);
            
            var nucleus_col = mix(tholin_crust, bright_ice, ice_scarps * 0.70);
            
            // Active Sublimation Gas Vents (Diatomic Carbon C2 / Cyanogen fluorescent green-cyan glow)
            if (temp > 120.0) {
                let vent_noise = fbm(p_surf * 26.0);
                let vent_spots = smoothstep(0.82, 0.96, vent_noise) * smoothstep(0.50, 0.90, ice_fissures);
                let vent_glow = vec3<f32>(0.18, 0.95, 0.72) * vent_spots * clamp((temp - 120.0) / 120.0, 0.0, 1.0);
                pbr_input.material.emissive = vec4<f32>(vent_glow * 1.8, 1.0);
                nucleus_col += vent_glow * 0.5;
            }
            
            pbr_input.material.perceptual_roughness = mix(0.92, 0.35, ice_scarps);
            pbr_input.material.metallic = 0.02;
            color = nucleus_col * (0.85 + micro_craters * 0.30);
        } else if (metal > 0.35) {
            // Metallic M-type Asteroid (Psyche type): dark specular iron-nickel flecks with metallic luster
            let iron_sheen = fbm(p_surf * 18.0);
            let metal_base = mix(base_tint * 0.80, base_tint * 1.30, iron_sheen);
            let specular_flecks = smoothstep(0.65, 0.95, boulder_noise);
            
            pbr_input.material.metallic = 0.75;
            pbr_input.material.perceptual_roughness = 0.38 + regolith * 0.25;
            color = mix(metal_base, vec3<f32>(0.65, 0.63, 0.60), specular_flecks * 0.35) * (0.82 + micro_craters * 0.36);
        } else {
            // S-type (Stony), C-type (Carbonaceous), V-type (Basaltic), D/P-type (Primitive)
            let chondrule = mix(base_tint * 0.75, base_tint * 1.30, regolith);
            let boulder_field = mix(chondrule, base_tint * 1.60 + vec3<f32>(0.05, 0.05, 0.05), boulder_noise * 0.35);
            
            if (ice > 0.04) {
                // Carbonaceous asteroid with subsurface frost and exposed icy patches
                let frost = smoothstep(0.70, 0.90, fbm(p_surf * 24.0));
                color = mix(boulder_field, vec3<f32>(0.65, 0.75, 0.86), frost * 0.40);
                pbr_input.material.perceptual_roughness = mix(0.92, 0.55, frost);
            } else {
                color = boulder_field * (0.80 + micro_craters * 0.40);
                pbr_input.material.perceptual_roughness = 0.92;
            }
            pbr_input.material.metallic = clamp(metal * 1.2, 0.02, 0.30);
        }
    }
    // =========================================================================
    // 9. Ice Dwarf Planets & Cryogenic Worlds (Pluto, Charon, Eris, Haumea, Makemake, Sedna, Ceres)
    // =========================================================================
    if (planet.planet_type == 9u) {
        let dwarf_sub = round(planet.color_seed.w);
        let base_tint = planet.color_seed.rgb;
        let lat_coord = asin(clamp(p_surf.y, -1.0, 1.0));
        let lon_coord = atan2(p_surf.z, p_surf.x);
        let micro_relief = fbm_lod(p_surf * 18.0);
        let fine_crater = fbm_lod(p_surf * 28.0);

        if (dwarf_sub == 1.0) {
            // -----------------------------------------------------------------
            // PLUTO (Authentic New Horizons Reconnaissance Data)
            // -----------------------------------------------------------------
            // 1. Tombaugh Regio / Sputnik Planitia (Nitrogen / Methane Ice Heart)
            // Centered on the anti-Charon hemisphere (~0 lon, +20 deg lat)
            let sp_center = vec3<f32>(1.0, 0.35, 0.0) / length(vec3<f32>(1.0, 0.35, 0.0));
            let d_sp = length(p_surf - sp_center);
            
            // Heart geometry: west lobe (Sputnik Planitia) + east lobe (elevated plains)
            let west_lobe_center = vec3<f32>(0.96, 0.36, -0.16) / length(vec3<f32>(0.96, 0.36, -0.16));
            let east_lobe_center = vec3<f32>(0.94, 0.30, 0.22) / length(vec3<f32>(0.94, 0.30, 0.22));
            let d_west = length(p_surf - west_lobe_center);
            let d_east = length(p_surf - east_lobe_center);
            let heart_dist = min(d_west * 1.05, d_east * 1.25);
            let heart_mask = smoothstep(0.44, 0.32, heart_dist);

            // Active solid nitrogen convection cells in Sputnik Planitia (polygonal cells)
            let v_cell = voronoi3(p_surf * 26.0, 0.0);
            let cell_trough = smoothstep(0.035, 0.16, v_cell.y - v_cell.x);
            let cell_center_col = vec3<f32>(0.96, 0.93, 0.88); // Brilliant ivory N2/CO ice
            let cell_trough_col = vec3<f32>(0.76, 0.68, 0.58); // Trough with accumulated tholins
            let sputnik_cell_ice = mix(cell_trough_col, cell_center_col, cell_trough);
            let sputnik_col = mix(sputnik_cell_ice, vec3<f32>(0.92, 0.86, 0.78), micro_relief * 0.15);

            // 2. Cthulhu Macula ("The Whale" - Equatorial Tholin Belt)
            // Heavy dark reddish-brown / chocolate tholins west of Sputnik Planitia
            let in_equator = smoothstep(0.32, 0.05, abs(lat_coord));
            let cthulhu_lon = smoothstep(-0.30, -0.80, lon_coord) * smoothstep(-2.8, -2.2, lon_coord);
            let cthulhu_belt = in_equator * cthulhu_lon * (1.0 - heart_mask * 0.95);
            let cthulhu_col = vec3<f32>(0.26, 0.12, 0.07) * (0.80 + micro_relief * 0.40);

            // 3. Hillary Montes & Tenzing Montes (Water Ice Bedrock Mountains)
            // Rugged towers fringing the southwest margin of Sputnik Planitia
            let mtn_center = vec3<f32>(0.92, 0.10, -0.36) / length(vec3<f32>(0.92, 0.10, -0.36));
            let d_mtn = length(p_surf - mtn_center);
            let mtn_mask = smoothstep(0.24, 0.04, d_mtn);
            let mtn_crest = smoothstep(0.55, 0.85, ridge_noise(p_surf * 36.0));
            let bedrock_slate = vec3<f32>(0.42, 0.44, 0.48); // Pure water-ice granite
            let methane_snow = vec3<f32>(0.95, 0.97, 1.0);   // Gleaming crest frost
            let mtn_col = mix(bedrock_slate, methane_snow, mtn_crest * 0.75);

            // 4. Lowell Regio (North Polar Ochre Cap)
            let polar_mask = smoothstep(0.55, 0.82, lat_coord);
            let polar_ochre = vec3<f32>(0.84, 0.76, 0.52) * (0.90 + micro_relief * 0.20);

            // 5. Eastern Uplands & Tartarus Dorsa (Bladed Methane Ice)
            let east_upland_col = vec3<f32>(0.72, 0.58, 0.46) * (0.85 + micro_relief * 0.30);

            // Composite Pluto terrain layers
            var pluto_surf = mix(east_upland_col, cthulhu_col, cthulhu_belt);
            pluto_surf = mix(pluto_surf, polar_ochre, polar_mask * (1.0 - heart_mask * 0.6));
            pluto_surf = mix(pluto_surf, mtn_col, mtn_mask);
            pluto_surf = mix(pluto_surf, sputnik_col, heart_mask);

            // Fine impact craters on ancient terrains (not in Sputnik Planitia)
            let crater_mod = (1.0 - heart_mask * 0.90) * (fine_crater * 0.18 - 0.09);
            pluto_surf += vec3<f32>(crater_mod);

            // Delicate Atmospheric Blue Rayleigh Haze on Limb
            let NdotV_haze = max(dot(norm, pbr_input.V), 0.0);
            let limb_haze = pow(1.0 - NdotV_haze, 3.5);
            let solar_grazing = max(dot(norm, to_star) * 0.5 + 0.5, 0.0);
            let blue_haze = vec3<f32>(0.25, 0.65, 1.0) * (limb_haze * solar_grazing * 0.65);

            pbr_input.material.perceptual_roughness = mix(0.72, 0.28, heart_mask);
            pbr_input.material.metallic = 0.03;
            pbr_input.material.emissive = vec4<f32>(blue_haze, 1.0);
            color = pluto_surf;
        } else if (dwarf_sub == 2.0) {
            // -----------------------------------------------------------------
            // CHARON (Pluto's Massive Binary Companion)
            // -----------------------------------------------------------------
            // Mordor Macula (dark reddish-brown north polar cap from captured tholins)
            let mordor_mask = smoothstep(0.55, 0.85, lat_coord);
            let mordor_col = vec3<f32>(0.38, 0.20, 0.15) * (0.85 + micro_relief * 0.30);

            // Serenity Chasma (colossal equatorial rift canyon system)
            let chasma_mask = smoothstep(0.045, 0.0, abs(p_surf.y + 0.05));
            let chasma_floor = vec3<f32>(0.16, 0.16, 0.18);

            // Rolling neutral gray water-ice plains (Vulcan Planitia in south, Oz Terra in north)
            let gray_crust = vec3<f32>(0.50, 0.50, 0.52) * (0.80 + micro_relief * 0.40) + vec3<f32>(fine_crater * 0.15 - 0.07);

            var charon_surf = mix(gray_crust, mordor_col, mordor_mask);
            charon_surf = mix(charon_surf, chasma_floor, chasma_mask * 0.85);

            pbr_input.material.perceptual_roughness = 0.70;
            pbr_input.material.metallic = 0.04;
            color = charon_surf;
        } else if (dwarf_sub == 3.0) {
            // -----------------------------------------------------------------
            // ERIS (Pristine High-Albedo Methane-Nitrogen Frost World)
            // -----------------------------------------------------------------
            // Geometric albedo 0.96: brilliant sparkling pearlescent crust
            let frost_glitter = smoothstep(0.72, 0.95, ridge_noise(p_surf * 32.0));
            let eris_white = vec3<f32>(0.94, 0.96, 0.99) * (0.92 + micro_relief * 0.12);
            color = mix(eris_white, vec3<f32>(1.0, 1.0, 1.0), frost_glitter * 0.35);
            pbr_input.material.perceptual_roughness = 0.22;
            pbr_input.material.metallic = 0.02;
        } else if (dwarf_sub == 4.0) {
            // -----------------------------------------------------------------
            // HAUMEA (Fast-Spinning Crystalline Water Ice with Dark Red Spot)
            // -----------------------------------------------------------------
            let haumea_ice = vec3<f32>(0.86, 0.89, 0.93) * (0.88 + micro_relief * 0.24);
            // Prominent dark reddish-brown impact spot on equatorial flank
            let spot_center = vec3<f32>(0.85, 0.10, 0.51) / length(vec3<f32>(0.85, 0.10, 0.51));
            let spot_mask = smoothstep(0.22, 0.06, length(p_surf - spot_center));
            let red_spot = vec3<f32>(0.50, 0.22, 0.15) * (0.85 + fine_crater * 0.30);
            color = mix(haumea_ice, red_spot, spot_mask);
            pbr_input.material.perceptual_roughness = 0.38;
            pbr_input.material.metallic = 0.03;
        } else if (dwarf_sub == 5.0) {
            // -----------------------------------------------------------------
            // MAKEMAKE (Terracotta Tholins with Methane Ice Slabs)
            // -----------------------------------------------------------------
            let terracotta = vec3<f32>(0.74, 0.38, 0.22) * (0.82 + micro_relief * 0.36);
            let methane_slabs = smoothstep(0.62, 0.82, fbm_lod(p_surf * 10.0));
            let bright_methane = vec3<f32>(0.92, 0.88, 0.80);
            color = mix(terracotta, bright_methane, methane_slabs * 0.60);
            pbr_input.material.perceptual_roughness = mix(0.75, 0.35, methane_slabs);
            pbr_input.material.metallic = 0.03;
        } else if (dwarf_sub == 6.0) {
            // -----------------------------------------------------------------
            // SEDNA (Inner Oort Cloud Deep Garnet/Crimson Primordial Crust)
            // -----------------------------------------------------------------
            let crimson_tholin = vec3<f32>(0.64, 0.16, 0.12) * (0.80 + micro_relief * 0.40);
            let dark_crags = vec3<f32>(0.32, 0.08, 0.06);
            color = mix(crimson_tholin, dark_crags, smoothstep(0.4, 0.8, fine_crater));
            pbr_input.material.perceptual_roughness = 0.80;
            pbr_input.material.metallic = 0.02;
        } else if (dwarf_sub == 7.0) {
            // -----------------------------------------------------------------
            // CERES (Carbonaceous Clay with Occator Cerealia Facula Salt Dome)
            // -----------------------------------------------------------------
            let clay_crust = vec3<f32>(0.24, 0.24, 0.25) * (0.82 + micro_relief * 0.36) + vec3<f32>(fine_crater * 0.12 - 0.06);
            // Occator Crater & Cerealia Facula bright sodium carbonate deposit
            let occ_center = vec3<f32>(0.82, 0.34, 0.45) / length(vec3<f32>(0.82, 0.34, 0.45));
            let d_occ = length(p_surf - occ_center);
            let occ_floor = smoothstep(0.14, 0.04, d_occ);
            let salt_dome = smoothstep(0.04, 0.01, d_occ);
            let salt_col = vec3<f32>(0.96, 0.98, 1.0);
            let floor_col = vec3<f32>(0.16, 0.16, 0.17);
            var ceres_surf = mix(clay_crust, floor_col, occ_floor * 0.60);
            ceres_surf = mix(ceres_surf, salt_col, salt_dome);
            color = ceres_surf;
            pbr_input.material.perceptual_roughness = mix(0.90, 0.25, salt_dome);
            pbr_input.material.metallic = 0.04;
        } else {
            // -----------------------------------------------------------------
            // GENERIC KUIPER BELT OBJECT / ICE DWARF (Procedural Cryo-World)
            // -----------------------------------------------------------------
            let tholin_crust = base_tint * (0.80 + micro_relief * 0.40);
            let frost_sheet = smoothstep(0.55, 0.80, fbm_lod(p_surf * 8.0));
            let bright_frost = vec3<f32>(0.88, 0.92, 0.98);
            color = mix(tholin_crust, bright_frost, frost_sheet * 0.55);
            pbr_input.material.perceptual_roughness = mix(0.80, 0.40, frost_sheet);
            pbr_input.material.metallic = 0.03;
        }
    }
}
@if(!BODY_CLASS_GIANT && !BODY_CLASS_MINOR) {
    // =========================================================================
    // 3. Terrestrial Rocky / Ocean / Biosphere Planet / Protoplanet
    // =========================================================================
    if (planet.planet_type == 3u) {
        let t_sub = round(planet.color_seed.w);
        let plate_drift = vec3<f32>(
            sin(drift_phase * 6.283 + p_surf.z * 2.6) * 0.28,
            cos(drift_phase * 3.14159 + p_surf.x * 2.2) * 0.12,
            sin(drift_phase * 4.712 + p_surf.y * 2.4) * 0.28
        );
        let p_tectonic = p_surf + plate_drift;

        // Multi-scale elevation: eroded continents → hills → ridges → micro-roughness
        let raw_continent_mask = eroded_continent_fbm_lod(p_tectonic * 1.85);
        let earth_harmonics = evaluate_earth_continental_harmonics(p_tectonic);
        let earth_blend = select(0.0, 1.0 - clamp(drift_phase * 2.5, 0.0, 0.85), t_sub == 4.0);
        let continent_mask = mix(
            raw_continent_mask,
            clamp(earth_harmonics + (raw_continent_mask - 0.42) * 0.58, 0.0, 1.0),
            earth_blend
        );
        let regional_hills = fbm_lod(p_tectonic * 5.5) * 0.32;
        let mountain_ridges = ridge_noise(p_tectonic * 8.0) * 0.20;
        let micro_detail = fbm_lod(p_tectonic * 22.0) * 0.10;

        // Supercontinent Wilson-cycle clustering
        let super_axis = normalize(vec3<f32>(0.65, 0.25, 0.70));
        let cluster_dipole = dot(p_surf, super_axis);
        let cluster_bias = cluster_dipole * (aggregation - 0.20) * 0.14 * (1.0 - earth_blend);

        let sea_level = clamp(0.18 + ocean_frac * 0.40, 0.18, 0.76);
        let primordial_shelf_elev = 0.46;
        let is_snowball = has_volatiles && (ice_frac >= 0.60 || (temp < 255.0 && (ocean_frac > 0.005 || ice_frac > 0.10)));
        let has_water_or_ice_oceans = has_volatiles && ocean_frac > 0.01 && temp <= 380.0 && pressure_bar >= 0.006;
        let has_oceans = has_water_or_ice_oceans && !is_snowball;

        let land_weight = smoothstep(0.28, 0.48, continent_mask);
        let continental_relief = continent_mask * 0.52
            + (regional_hills + mountain_ridges + micro_detail) * (0.15 + 0.85 * land_weight)
            + cluster_bias;

        // Realistic Bathymetry (Mid-Ocean Ridges, Axial Rifts, Abyssal Plains, and Subduction Trenches)
        let oceanic_basin = 1.0 - land_weight;
        let mid_ocean_ridge = ridge_noise(p_tectonic * 6.5 + vec3<f32>(1.2, 3.4, 0.7)) * 0.10;
        let axial_rift = (1.0 - ridge_noise(p_tectonic * 13.0 + vec3<f32>(1.2, 3.4, 0.7))) * 0.025;
        let trench_zone = smoothstep(0.04, 0.0, abs(continent_mask - 0.36) - 0.015);
        let trench_depth = trench_zone * 0.09;

        // In oceanic basins, keep floor comfortably below sea_level so ridges never pierce the water
        let ocean_base = sea_level - 0.22;
        let bathymetry_elev = ocean_base + (mid_ocean_ridge - axial_rift) - trench_depth;

        let procedural_elev = select(
            continental_relief + oceanic_basin * (mid_ocean_ridge - axial_rift) - trench_depth,
            mix(bathymetry_elev, continental_relief, land_weight),
            has_water_or_ice_oceans
        );
        var elev = select(
            procedural_elev,
            evaluate_canonical_terrestrial_elevation(p_surf, t_sub),
            t_sub >= 1.0 && t_sub != 4.0
        );
        if (has_water_or_ice_oceans && elev < sea_level) {
            elev = sea_level + select(0.0, 0.003, is_snowball);
        }

        // Slight albedo desaturation helper (kills candy primaries)
        let desat = 0.18;

        // A. Molten Magma Ocean
        if ((temp >= 1100.0 || lava_frac > 0.65) && (!has_volatiles || ocean_frac < 0.04 || temp > 380.0)) {
            let magma = evaluate_magma_ocean_surface(p_surf, elev, temp, lava_frac, t);
            color = magma.color;
            pbr_input.material.emissive = vec4<f32>(magma.emissive, 1.0);
            pbr_input.material.perceptual_roughness = magma.roughness;
            pbr_input.material.metallic = magma.metallic;
        }
        // B. Venusian runaway greenhouse deck (with Akatsuki / Mariner 10 UV Bow-Shaped "Y-Wave")
        else if (temp >= 380.0 && (gas > 0.05 || cloud_density > 0.5 || pressure_bar > 5.0)) {
            let super_rot = rotate_y(p_tilted, planet.spin_angle * 4.0 + t * 0.008);
            let clouds = fbm_lod(super_rot * 4.5 + vec3<f32>(t * 0.006, 0.0, t * 0.006));
            let v_band_deriv = length(fwidth(lat * 8.0));
            let v_band_lod = 1.0 - smoothstep(0.35, 1.10, v_band_deriv);
            let lon_v = atan2(super_rot.z, super_rot.x);
            let y_wave = cos(lon_v - abs(lat) * 2.6 + clouds * 1.1) * 0.5 + 0.5;
            let bow_chevron = smoothstep(0.35, 0.82, y_wave) * smoothstep(0.78, 0.12, abs(lat)) * v_band_lod;
            let band = mix(0.5, sin(lat * 8.0 + clouds * (1.8 * v_band_lod)) * 0.5 + 0.5, v_band_lod);
            let dark_uv_absorber = vec3<f32>(0.68, 0.58, 0.34);
            let bright_sulfur_deck = vec3<f32>(0.93, 0.87, 0.65);
            let sulfur_deck = mix(dark_uv_absorber, bright_sulfur_deck, clamp(band * 0.55 + (1.0 - bow_chevron) * 0.45, 0.0, 1.0));
            let polar_collar = smoothstep(0.68, 0.90, abs(lat));
            color = mix(sulfur_deck * (0.88 + clouds * 0.18 * v_band_lod), vec3<f32>(0.96, 0.92, 0.76), polar_collar * 0.55);
            pbr_input.material.perceptual_roughness = 0.52;
        }
        // B2. Canonical Solar System Landmarks (Mars, Moon, Mercury, Ganymede, Callisto, Titan)
        else if ((t_sub == 1.0 || t_sub == 2.0 || t_sub == 3.0 || t_sub == 6.0 || t_sub == 7.0 || t_sub == 8.0) && ocean_frac < 0.04 && biomass < 0.02) {
            let season_subsolar = dot(planet.star_dir_and_lum.xyz, s_axis);
            var can_res: CanonicalSurfaceResult;
            if (t_sub == 1.0) {
                can_res = evaluate_mars_surface_landmarks(p_surf, p_tilted, elev, cloud_density, season_subsolar, t);
            } else if (t_sub == 2.0 || t_sub == 3.0) {
                can_res = evaluate_moon_and_mercury_landmarks(p_surf, t_sub, elev);
            } else {
                can_res = evaluate_outer_moon_landmarks(p_surf, p_tilted, norm, pbr_input.V, to_star, t_sub, elev);
            }
            color = can_res.color;
            pbr_input.material.perceptual_roughness = can_res.roughness;
            pbr_input.material.metallic = can_res.metallic;
            pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb + can_res.emissive, 1.0);
            water_mask = can_res.water_mask;
            cloud_cover_alpha = max(cloud_cover_alpha, can_res.cloud_alpha);
        }
        // C. Snowball / glacial planetary surface (Cryogenic ice sheets, pack ice & nunataks)
        else if (is_snowball) {
            // Multi-scale cryogenic sastrugi (wind-carved snow dunes) and firn variegation
            let sastrugi_macro = fbm_lod(p_surf * 8.0);
            let sastrugi_micro = fbm(p_surf * 22.0);
            let sastrugi = sastrugi_macro * 0.70 + sastrugi_micro * 0.30;

            // Ice palette: pristine pack ice, compacted glacial blue ice, and translucent cryo-leads
            let pack_ice = vec3<f32>(0.93, 0.96, 0.99);       // Brilliant high-albedo snow / pack ice
            let glacial_blue = vec3<f32>(0.76, 0.88, 0.98);   // Dense crystalline blue ice dome
            let crevasse_tint = vec3<f32>(0.60, 0.80, 0.94);  // Faint cyan pressure ridge / lead
            let polar_firn = vec3<f32>(0.96, 0.98, 1.00);     // Cold dry polar snow

            // Base ice blend over the planetary sphere
            var ice_base = mix(glacial_blue, pack_ice, smoothstep(0.28, 0.68, sastrugi));
            ice_base = mix(ice_base, crevasse_tint, smoothstep(0.78, 0.95, sastrugi_micro) * 0.35);

            // Continental ice sheets vs. ocean pack ice:
            let is_continental = elev > sea_level + 0.005;
            let rel_elev = max(elev - sea_level, 0.0);

            // On continents: thick ice domes with alpine nunatak peaks piercing through high elevations
            if (is_continental) {
                // Continental ice sheets are thicker and brighter
                ice_base = mix(ice_base, polar_firn, clamp(rel_elev * 2.5, 0.0, 0.65));

                // Nunatak threshold: tallest mountain ridges / cratons breach the ice sheet
                let nunatak_thresh = 0.22;
                if (rel_elev > nunatak_thresh) {
                    let nunatak_noise = fbm(p_tectonic * 16.0);
                    let mtn_crest = smoothstep(0.40, 0.85, nunatak_noise * 0.5 + (rel_elev - nunatak_thresh) * 3.0);
                    let nunatak_rock = vec3<f32>(0.32, 0.29, 0.27); // Dark frost-scoured permafrost / granite
                    let nunatak_snow = vec3<f32>(0.88, 0.92, 0.96);
                    let nunatak_col = mix(nunatak_rock, nunatak_snow, smoothstep(0.60, 0.90, nunatak_noise));
                    ice_base = mix(ice_base, nunatak_col, mtn_crest * 0.82);
                }
            } else {
                // Oceanic sea ice / pack ice floes: subtle leads and flat gleaming pack ice
                let floe_lead = smoothstep(0.72, 0.90, fbm(p_surf * 14.0));
                ice_base = mix(ice_base, crevasse_tint, floe_lead * 0.40);
            }

            // Equatorial cryo-desert: hyper-arid sublimation snow has subtle wind-blown pale titanium tint
            let eq_sublimation = exp(-pow(sin_lat / 0.32, 2.0)) * (1.0 - smoothstep(0.35, 0.70, sastrugi_macro));
            let cryo_dust = vec3<f32>(0.90, 0.88, 0.84);
            color = mix(ice_base, cryo_dust, eq_sublimation * 0.18);

            pbr_input.material.perceptual_roughness = mix(0.18, 0.32, sastrugi_macro);
            pbr_input.material.metallic = 0.0;
        }
        // D. Temperate water-bearing / biosphere / exposed bathymetry
        else if (temp >= 200.0 && temp <= 450.0) {
            let has_volatile_ice = has_volatiles && (ice_frac > 0.005 || ocean_frac > 0.005);
            let is_ice_cold = temp < 320.0 && has_volatile_ice;
            let base_ice_thresh = clamp(1.0 - ice_frac * 1.05, 0.0, 0.98);
            let is_tidally_locked = planet.tidal_deformation.w >= 1.5;
            let substellar_cos = dot(norm, to_star);

            // Seasonal Milankovitch orbital insolation shift
            let season_subsolar = dot(planet.star_dir_and_lum.xyz, s_axis);
            let ice_thresh_north = clamp(base_ice_thresh + season_subsolar * 0.15, 0.0, 1.0);
            let ice_thresh_south = clamp(base_ice_thresh - season_subsolar * 0.15, 0.0, 1.0);

            let ice_edge_noise = fbm_lod(p_surf * 10.0) * 0.06;
            let in_north_cap = (p_tilted.y > 0.0) && (polar_angle + ice_edge_noise > ice_thresh_north);
            let in_south_cap = (p_tilted.y < 0.0) && (polar_angle + ice_edge_noise > ice_thresh_south);
            // Eyeball-Earth climate on tidally locked worlds: permanent nightside hemisphere freezes over
            let eyeball_ice = is_tidally_locked && has_volatile_ice && (temp < 345.0) && ((substellar_cos + ice_edge_noise) < (-0.15 + clamp(ice_frac - 0.15, -0.25, 0.55)));
            let in_ice_cap = has_volatile_ice && select(is_ice_cold && (in_north_cap || in_south_cap), eyeball_ice || (is_ice_cold && (in_north_cap || in_south_cap)), is_tidally_locked);

            // Polar caps, eyeball nightside ice sheet & dynamic continental glaciers:
            if (in_ice_cap) {
                let frost_var = fbm(p_surf * 12.0);
                let pack_ice = vec3<f32>(0.92, 0.95, 0.98);
                let glacial_blue = vec3<f32>(0.72, 0.85, 0.96);
                color = mix(pack_ice, glacial_blue, frost_var * 0.35);
                pbr_input.material.perceptual_roughness = 0.22;
                pbr_input.material.metallic = 0.0;
            }
            // Oceans with Multi-Tier Bathymetry & Boiling Froth
            else if (has_oceans && elev < sea_level) {
                water_mask = clamp((sea_level - elev) / 0.025, 0.002, 1.0);
                let depth = clamp((sea_level - elev) / max(sea_level, 0.08), 0.0, 1.0);

                let lagoon = vec3<f32>(0.12, 0.74, 0.82);      // Ultra-shallow turquoise coastal lagoon / reef
                let shelf = vec3<f32>(0.07, 0.48, 0.62);       // Shallow continental shelf (< 200m)
                let mid_ocean = vec3<f32>(0.03, 0.18, 0.42);   // Sunlit pelagic mid-depths
                let deep_abyss = vec3<f32>(0.012, 0.05, 0.20); // Dark abyssal plain (> 3000m)
                let trench_ink = vec3<f32>(0.004, 0.015, 0.09);// Subduction chasm / Mariana trench

                let submerged_ridge_glow = oceanic_basin * smoothstep(0.06, 0.13, mid_ocean_ridge) * (1.0 - smoothstep(0.0, 0.40, depth));

                var water_color = mix(lagoon, shelf, smoothstep(0.0, 0.10, depth));
                water_color = mix(water_color, mid_ocean, smoothstep(0.10, 0.35, depth));
                water_color = mix(water_color, deep_abyss, smoothstep(0.35, 0.75, depth));
                water_color = mix(water_color, trench_ink, smoothstep(0.75, 1.0, depth));
                water_color = mix(water_color, shelf * 1.15, submerged_ridge_glow * 0.45);

                // Thermal boiling frothing and steam venting at T > 340 K (flash evaporation)
                if (temp > 340.0) {
                    let boil_heat = clamp((temp - 340.0) / 45.0, 0.0, 1.0);
                    let boil_bubbles = fbm(p_surf * 42.0 + vec3<f32>(0.0, t * 0.7, 0.0));
                    let boil_froth = smoothstep(0.38, 0.72, boil_bubbles) * boil_heat;
                    let shore_steam = smoothstep(sea_level - 0.03, sea_level, elev) * boil_heat;
                    let steam_white = vec3<f32>(0.94, 0.96, 1.0);
                    water_color = mix(water_color, steam_white, clamp(boil_froth * 0.75 + shore_steam * 0.60, 0.0, 1.0));
                }

                // Archean iron-rich anoxic ocean transition
                let archean = mix(vec3<f32>(0.07, 0.22, 0.15), vec3<f32>(0.018, 0.07, 0.04), depth);
                color = mix(archean, water_color, clamp(ocean_oxidation, 0.0, 1.0));

                let shore = smoothstep(sea_level - 0.015, sea_level, elev);
                pbr_input.material.perceptual_roughness = mix(0.05, 0.15, shore);
                pbr_input.material.metallic = 0.0;
            }
            // Continents, Biomes or Exposed Bathymetry
            else {
                let is_exposed_seabed = !has_oceans && (elev < primordial_shelf_elev);
                let land_micro = fbm(p_tectonic * 18.0);

                if (is_exposed_seabed) {
                    let seabed_depth = (primordial_shelf_elev - elev) / (primordial_shelf_elev - 0.18);
                    
                    // Exposed Continental Shelf: pale sand terrace, fossil coral reefs, dry coastal lagoons
                    let shelf_sand = vec3<f32>(0.66, 0.62, 0.50);
                    let dried_mud = vec3<f32>(0.44, 0.38, 0.30);
                    let abyssal_basalt = vec3<f32>(0.18, 0.16, 0.15);
                    let salt_flat = vec3<f32>(0.88, 0.90, 0.92); // Halite/gypsum salt pan (Messinian salinity crisis)

                    // Volcanic hydrothermal minerals on exposed mid-ocean ridges
                    let ridge_fissure = oceanic_basin * smoothstep(0.07, 0.13, mid_ocean_ridge);
                    let hydrothermal_sulfide = vec3<f32>(0.55, 0.24, 0.12); // Rust-orange chalcopyrite / sulfur

                    // Salt pans form in low flat abyssal depressions
                    let salt_mask = smoothstep(0.45, 0.70, seabed_depth) * smoothstep(0.55, 0.40, mid_ocean_ridge);

                    var seabed_color = mix(shelf_sand, dried_mud, smoothstep(0.0, 0.25, seabed_depth));
                    seabed_color = mix(seabed_color, abyssal_basalt, smoothstep(0.25, 0.65, seabed_depth));
                    seabed_color = mix(seabed_color, salt_flat, salt_mask * 0.75);
                    seabed_color = mix(seabed_color, hydrothermal_sulfide, ridge_fissure * 0.55);

                    if (temp > 340.0) {
                        let dry_heat = clamp((temp - 340.0) / 45.0, 0.0, 1.0);
                        seabed_color = mix(seabed_color, salt_flat * 0.95, dry_heat * 0.35);
                    }

                    color = seabed_color * (0.92 + land_micro * 0.16);
                    pbr_input.material.perceptual_roughness = mix(0.68, 0.88, seabed_depth);
                    pbr_input.material.metallic = select(0.02, 0.12, ridge_fissure > 0.4);
                } else {
                    let rel_elev = elev - sea_level;
                    pbr_input.material.perceptual_roughness = mix(0.72, 0.92, clamp(rel_elev * 2.5, 0.0, 1.0));
                    pbr_input.material.metallic = 0.0;

                let moisture = fbm(p_tectonic * 3.8 + vec3<f32>(2.1, 0.0, 5.3));
                let land_micro = fbm(p_tectonic * 18.0);

                let effective_bio = biomass * vegetation_mult;
                let flora_type = planet.civilization_params.y;
                let biolum = planet.civilization_params.z;
                let gravity_g = max(planet.civilization_params.w, 0.25);

                if (effective_bio > 0.02) {
                    var lush = vec3<f32>(0.08, 0.28, 0.10);
                    var meadow = vec3<f32>(0.18, 0.34, 0.12);
                    var scrub = vec3<f32>(0.32, 0.30, 0.16);

                    if (flora_type > 2.5) {
                        // High-UV / Remnant Scytonemin Indigo-Cyan flora
                        lush = vec3<f32>(0.06, 0.18, 0.34);
                        meadow = vec3<f32>(0.12, 0.30, 0.42);
                        scrub = vec3<f32>(0.22, 0.26, 0.36);
                    } else if (flora_type > 1.5) {
                        // K-Dwarf / Binary Carotenoid-Phycobilin Amber-Gold flora
                        lush = vec3<f32>(0.36, 0.22, 0.05);
                        meadow = vec3<f32>(0.48, 0.34, 0.08);
                        scrub = vec3<f32>(0.40, 0.28, 0.14);
                    } else if (flora_type > 0.5) {
                        // M-Dwarf NIR Anthocyanin Crimson-Burgundy flora (e.g. TRAPPIST-1)
                        lush = vec3<f32>(0.34, 0.05, 0.14);
                        meadow = vec3<f32>(0.48, 0.12, 0.20);
                        scrub = vec3<f32>(0.38, 0.20, 0.22);
                    }

                    let rock = vec3<f32>(0.30, 0.28, 0.26);
                    let alpine = vec3<f32>(0.55, 0.54, 0.52);
                    let treeline = 0.22 / clamp(sqrt(gravity_g), 0.65, 1.65);

                    // Subtropical Hadley-cell arid desert belts (14°..34° latitude) + Earth Sahara/Arabia/Outback ergs
                    let hadley_belt = exp(-pow((abs(sin_lat) - 0.38) / 0.16, 2.0));
                    let sahara_arabia_boost = select(
                        0.0,
                        smoothstep(0.55, 0.15, length(p_tectonic - vec3<f32>(0.82, 0.36, 0.42))) * 0.44
                            + smoothstep(0.35, 0.10, length(p_tectonic - vec3<f32>(-0.63, -0.42, 0.65))) * 0.40,
                        t_sub == 4.0
                    );
                    let aridity = clamp((1.0 - moisture * 1.35) * hadley_belt * 1.45 + sahara_arabia_boost, 0.0, 1.0);
                    let desert_sand = vec3<f32>(0.76, 0.60, 0.38);
                    let red_erg = vec3<f32>(0.68, 0.42, 0.25);
                    let desert_biome = mix(desert_sand, red_erg, smoothstep(0.35, 0.72, ridge_noise(p_tectonic * 18.0)));
                    let snow_peak = vec3<f32>(0.93, 0.96, 0.99);

                    var land: vec3<f32>;
                    if (rel_elev > treeline) {
                        let high_mtn = mix(rock, alpine, clamp((rel_elev - treeline) * 3.5, 0.0, 1.0));
                        let snow_cap = smoothstep(treeline + 0.05, treeline + 0.15, rel_elev + abs(sin_lat) * 0.08);
                        land = mix(high_mtn, snow_peak, snow_cap * 0.88);
                    } else if (rel_elev > 0.08) {
                        let veg = mix(meadow, lush, moisture);
                        let bio_land = mix(scrub, veg, clamp(effective_bio * 1.3, 0.0, 1.0));
                        land = mix(bio_land, desert_biome, smoothstep(0.28, 0.68, aridity));
                    } else {
                        // Coastal plain
                        let coastal = mix(lush, meadow, moisture);
                        let bio_coast = mix(vec3<f32>(0.42, 0.36, 0.24), coastal, clamp(effective_bio * 1.4, 0.0, 1.0));
                        land = mix(bio_coast, desert_biome, smoothstep(0.34, 0.75, aridity) * 0.85);
                    }

                    // Micro mottling so large fields aren't flat paint
                    land = land * (0.92 + land_micro * 0.16);

                    let luma = dot(land, vec3<f32>(0.299, 0.587, 0.114));
                    color = mix(land, vec3<f32>(luma), desat * 0.7);
                } else {
                    // Barren / early Earth cratons — muted oxidized silicates
                    let lowlands = vec3<f32>(0.40, 0.30, 0.20);
                    let iron_shield = vec3<f32>(0.32, 0.20, 0.14);
                    let highlands = vec3<f32>(0.48, 0.42, 0.36);
                    let peaks = vec3<f32>(0.58, 0.54, 0.50);

                    var land: vec3<f32>;
                    if (rel_elev > 0.24) {
                        land = mix(highlands, peaks, clamp((rel_elev - 0.24) * 4.0, 0.0, 1.0));
                    } else if (rel_elev > 0.10) {
                        land = mix(iron_shield, lowlands, moisture);
                    } else {
                        land = mix(iron_shield, lowlands, 0.35 + moisture * 0.4);
                    }

                    land = land * (0.90 + land_micro * 0.18);
                    let luma = dot(land, vec3<f32>(0.299, 0.587, 0.114));
                    color = mix(land, vec3<f32>(luma), desat);
                }

                // Narrow beach only at the waterline
                let beach = smoothstep(sea_level, sea_level + 0.012, elev)
                    * (1.0 - smoothstep(sea_level + 0.012, sea_level + 0.035, elev));
                if (beach > 0.01 && has_oceans) {
                    let sand = vec3<f32>(0.52, 0.46, 0.34);
                    color = mix(color, sand, beach * 0.85);
                    pbr_input.material.perceptual_roughness = mix(
                        pbr_input.material.perceptual_roughness,
                        0.55,
                        beach
                    );
                }

                // Phase 7: Nocturnal & twilight terminator bioluminescent forest canopies
                if (biolum > 0.02 && effective_bio > 0.05 && rel_elev < 0.18) {
                    let bio_night = 1.0 - smoothstep(-0.18, 0.12, dot(norm, to_star));
                    if (bio_night > 0.01) {
                        let canopy_vein = smoothstep(0.58, 0.86, fbm(p_tectonic * 24.0 + vec3<f32>(0.7, -1.9, 3.1)));
                        let coastal_bloom = 1.0 - smoothstep(0.0, 0.12, rel_elev);
                        let polar_guard = 1.0 - smoothstep(0.55, 0.82, abs(sin_lat));
                        let biolum_mask = canopy_vein * (0.45 + coastal_bloom * 0.55) * polar_guard * clamp(effective_bio, 0.0, 1.0);
                        var biolum_hue = vec3<f32>(0.12, 0.95, 0.68);
                        if (flora_type > 0.5 && flora_type < 1.5) {
                            biolum_hue = vec3<f32>(0.92, 0.22, 0.68);
                        } else if (flora_type > 1.5 && flora_type < 2.5) {
                            biolum_hue = vec3<f32>(0.30, 0.92, 0.82);
                        }
                        pbr_input.material.emissive += vec4<f32>(biolum_hue * biolum_mask * bio_night * biolum * 1.35, 0.0);
                    }
                }
            }
        }


            // Dynamic civilization city lights on the night side
            let tech_level = planet.civilization_params.x;
            if (tech_level > 0.0) {
                // Hard ocean mask: no lights underwater, regardless of ice cap state
                let is_ocean = elev < sea_level;
                
                // Polar exclusion: cities don't exist on polar ice caps
                // sin_lat goes 0 at equator to 1.0 at pole; ice caps typically > 65° lat
                let polar_fade = 1.0 - smoothstep(0.55, 0.80, abs(sin_lat));
                
                // Ice cap exclusion: fade cities where ice covers the surface
                let ice_cap_mask = mix(1.0, polar_fade, clamp(ice_frac * 4.0, 0.0, 1.0));

                // Noise to cluster cities along coasts and plains (avoid mountain peaks)
                let rel_elev = max(0.0, elev - sea_level);
                let valley_mask = 1.0 - smoothstep(0.0, 0.15, rel_elev);
                
                // High frequency noise for city light clusters
                let pop_noise = fbm(p_surf * 45.0 + vec3<f32>(1.2, 5.5, -2.1));
                let cluster_noise = fbm(p_surf * 8.0);
                
                let base_density = smoothstep(0.65 - tech_level * 0.2, 1.0, pop_noise * cluster_noise);
                let population_density = base_density * valley_mask * ice_cap_mask
                    * select(1.0, 0.0, is_ocean); // zero on ocean
                
                // Only visible on the night side (fades in just past terminator)
                let night_factor = 1.0 - smoothstep(-0.1, 0.05, dot(norm, to_star));
                
                if (population_density > 0.0) {
                    // Warm amber (low tech) → blue-white LED (high tech)
                    let warm_city = vec3<f32>(1.0, 0.75, 0.40);
                    let advanced_city = vec3<f32>(0.85, 0.95, 1.0);
                    let city_color = mix(warm_city, advanced_city, clamp(tech_level - 1.0, 0.0, 1.0));
                    
                    let light_intensity = population_density * night_factor * clamp(tech_level * 3.0, 0.0, 8.0);
                    pbr_input.material.emissive += vec4<f32>(city_color * light_intensity * 2.5, 0.0);
                }
            }

            // 3-Layer Dynamic Clouds (Cumulus, Frontal/Coriolis Cyclones, Ice Cirrus) + 3D Ray-Cast Ground & Ocean Shadows
            let boil_steam_factor = clamp((temp - 345.0) / 35.0, 0.0, 1.0) * select(0.0, 1.0, has_water_volatiles && (pressure_bar >= 0.02 || gas > 0.005));
            let effective_cloud_density = select(0.0, mix(cloud_density, 1.0, boil_steam_factor), pressure_bar >= 0.02 || gas > 0.005);
            let substellar_cloud_boost = select(0.0, smoothstep(-0.10, 0.65, substellar_cos) * 0.08, is_tidally_locked);

            if (effective_cloud_density > 0.02) {
                let cloud_res = evaluate_multilayer_clouds_and_shadows(
                    p_tilted,
                    surface_dir,
                    pbr_input.V,
                    star_world_norm,
                    s_tangent_x,
                    s_axis,
                    s_tangent_z,
                    planet.spin_angle,
                    t,
                    effective_cloud_density,
                    boil_steam_factor,
                    substellar_cloud_boost,
                    planet.color_seed.rgb,
                    false,
                );
                let cam_alt_ratio = length(view.world_position - planet.planet_center_and_radius.xyz)
                    / max(planet.planet_center_and_radius.w, 1e-6);
                let orbital_cloud_vis = smoothstep(1.15, 1.42, cam_alt_ratio);
                let surface_cloud_alpha = cloud_res.total_alpha * orbital_cloud_vis;

                // Cast 3D solar-projected cloud shadows onto BOTH land and ocean bathymetry
                let exposed_shadow = mix(cloud_res.ground_shadow_mult, 1.0, surface_cloud_alpha * 0.76);
                color = color * exposed_shadow;
                cloud_shadow_transmittance = exposed_shadow;

                // Sub-cloud lower-troposphere Rayleigh aerial perspective so the surface & ground shadows
                // visibly sit beneath a cushion of blue atmospheric air while the clouds float above
                let sub_ndotv = clamp(dot(surface_dir, pbr_input.V), 0.16, 1.0);
                let sub_day = smoothstep(-0.14, 0.26, dot(surface_dir, star_world_norm));
                let sub_air_veil = clamp((1.0 / sub_ndotv - 0.82) * 0.14 * clamp(pressure_bar, 0.0, 2.2), 0.0, 0.32) * orbital_cloud_vis;
                let sub_rayleigh_col = vec3<f32>(0.24, 0.52, 0.92) * sub_day;
                color = mix(color, color * 0.72 + sub_rayleigh_col * 0.55, sub_air_veil * (1.0 - surface_cloud_alpha * 0.85));

                if (surface_cloud_alpha > 0.005) {
                    cloud_cover_alpha = max(cloud_cover_alpha, surface_cloud_alpha);
                    color = mix(color, cloud_res.cloud_color, surface_cloud_alpha);
                    pbr_input.material.perceptual_roughness = mix(
                        pbr_input.material.perceptual_roughness,
                        cloud_res.cloud_roughness,
                        surface_cloud_alpha
                    );
                    pbr_input.N = normalize(mix(pbr_input.N, cloud_res.cloud_normal_world, surface_cloud_alpha * 0.75));
                }
            }
        }
        // E. Metal-rich (Mercury-type)
        else if (metal > 0.42) {
            let sheen = fbm(p_surf * 7.0);
            let craters = fbm(p_surf * 11.0);
            let dark_graphite = vec3<f32>(0.16, 0.16, 0.18);
            let nickel_iron = vec3<f32>(0.55, 0.52, 0.48);
            color = mix(dark_graphite, nickel_iron, sheen * 0.55 + craters * 0.35);
            pbr_input.material.perceptual_roughness = 0.45;
            pbr_input.material.metallic = 0.55;
        }
        // G. Barren dry silicate (Moon / Mars / Mercury / Terrestrial Protoplanets)
        else {
            let base_tint = planet.color_seed.rgb;
            let craters = fbm(p_surf * 9.0);
            let highlands = fbm(p_surf * 3.2);
            let micro = fbm(p_surf * 20.0);
            let maria_basins = fbm(p_surf * 1.6);

            pbr_input.material.perceptual_roughness = mix(0.78, 0.95, highlands);
            pbr_input.material.metallic = select(0.04, 0.35, metal > 0.30);

            // Contrast between lowland plains/maria and ancient highland cratons
            let lowlands = base_tint * 0.48;
            let peaks = min(base_tint * 1.32 + vec3<f32>(0.06, 0.06, 0.08), vec3<f32>(1.0));

            let basin_mask = smoothstep(0.48, 0.32, maria_basins);
            let surface_blend = highlands * 0.55 + craters * 0.30 + micro * 0.15;

            var crust_color = mix(lowlands, peaks, surface_blend);
            // Lowland impact basins & basalt flood plains (dark maria floors)
            crust_color = mix(crust_color, lowlands * 0.80, basin_mask * 0.60);
            crust_color = crust_color * (0.88 + micro * 0.24);

            // Fresh crater ejecta blankets & bright ray systems (e.g. lunar Tycho rays)
            let ray_noise = ridge_noise(p_surf * 14.0);
            let fresh_ejecta = smoothstep(0.70, 0.90, ray_noise) * smoothstep(0.52, 0.85, craters);
            let ejecta_color = min(base_tint * 1.65 + vec3<f32>(0.12, 0.12, 0.15), vec3<f32>(0.96));
            color = mix(crust_color, ejecta_color, fresh_ejecta * 0.42);

            let luma = dot(color, vec3<f32>(0.299, 0.587, 0.114));
            color = mix(color, vec3<f32>(luma), 0.08);
        }

        // Magma cooling / active volcanism overlay
        if (temp < 1100.0 && (temp > 450.0 || lava_frac > 0.02)) {
            let magma = evaluate_magma_ocean_surface(p_surf, elev, temp, lava_frac, t);
            let magma_blend = clamp((temp - 450.0) / 650.0 + lava_frac * 0.5, 0.0, 1.0);
            let fissure_mask = 1.0 - magma.crust_mask;
            let subaerial_mask = 1.0 - clamp(water_mask, 0.0, 1.0);
            let active_vent = clamp(magma_blend * 0.70 + fissure_mask * 0.88, 0.0, 1.0) * subaerial_mask;
            color = mix(color, magma.color, active_vent);
            pbr_input.material.emissive = vec4<f32>(
                pbr_input.material.emissive.rgb
                    + magma.emissive * (magma_blend * 0.60 + fissure_mask * 0.75) * subaerial_mask,
                1.0
            );
            pbr_input.material.perceptual_roughness = mix(
                pbr_input.material.perceptual_roughness,
                magma.roughness,
                active_vent * 0.5
            );
        }
    }

    // Liquid water surfaces form a smooth gravitational equipotential level with gentle animated capillary swell
    if (water_mask > 0.005) {
        let open_water = clamp(water_mask * (1.0 - cloud_cover_alpha * 0.88), 0.0, 1.0);
        let wave_t = planet.time * 1.2;
        let wave_coord = p_surf * 65.0;
        let wave_slope_x = cos(wave_coord.x * 2.1 + wave_coord.z * 1.4 + wave_t) * 0.0042;
        let wave_slope_z = sin(wave_coord.x * 1.2 - wave_coord.z * 2.4 + wave_t * 0.8) * 0.0042;
        let wave_pert_spun = vec3<f32>(wave_slope_x, 0.0, wave_slope_z);
        let wave_pert_tilted = rotate_y(wave_pert_spun, -planet.spin_angle);
        let wave_pert_world = wave_pert_tilted.x * s_tangent_x + wave_pert_tilted.y * s_axis + wave_pert_tilted.z * s_tangent_z;
        let ocean_normal = normalize(surface_dir + wave_pert_world);
        pbr_input.N = normalize(mix(pbr_input.N, ocean_normal, open_water));
    }

    if (planet.planet_type != 0u && planet.planet_type != 5u && planet.planet_type != 7u) {
        let crater_res = apply_impact_craters_and_basins(
            p_surf,
            color,
            pbr_input.material.perceptual_roughness,
            pbr_input.material.metallic,
            planet.time,
            water_mask,
        );
        let crater_heat_punch = clamp(length(crater_res.emissive) * 0.42, 0.0, 1.0);
        let crater_cloud_vis = clamp(1.0 - cloud_cover_alpha * 0.88 + crater_heat_punch, 0.0, 1.0);
        color = mix(color, crater_res.color, crater_cloud_vis);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, crater_res.roughness, crater_cloud_vis);
        pbr_input.material.metallic = mix(pbr_input.material.metallic, crater_res.metallic, crater_cloud_vis);
        let emissive_vis = clamp(1.0 - cloud_cover_alpha * 0.35 + crater_heat_punch, 0.0, 1.0);
        pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb + crater_res.emissive * emissive_vis, 1.0);
        if (dot(crater_res.normal_delta, crater_res.normal_delta) > 1e-6) {
            let c_norm_tilted = rotate_y(crater_res.normal_delta, -planet.spin_angle);
            let c_norm_world = c_norm_tilted.x * s_tangent_x + c_norm_tilted.y * s_axis + c_norm_tilted.z * s_tangent_z;
            pbr_input.N = normalize(pbr_input.N + c_norm_world * crater_cloud_vis);
        }

        // Active tidal volcanism & caldera hotspots (Io analogs)
        let is_sulfur_world = (planet.composition.x > 0.40 && planet.composition.y < 0.10 && (lava_frac > 0.02 || (planet.color_seed.r > 0.72 && planet.color_seed.g > 0.62 && planet.color_seed.b < 0.42)));
        if (planet.planet_type != 1u && planet.planet_type != 2u && (lava_frac > 0.012 || is_sulfur_world) && water_mask < 0.2) {
            let caldera_res = evaluate_volcanic_calderas(p_surf, lava_frac, is_sulfur_world, t, color);
            color = caldera_res.color;
            pbr_input.material.emissive += vec4<f32>(caldera_res.emissive * (1.0 - cloud_cover_alpha * 0.85), 0.0);
            pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, caldera_res.roughness, 0.5);
        }

        // Active cryotectonic fractures & south polar geyser vents (Europa & Enceladus analogs)
        let is_cryotectonic = planet.composition.y > 0.30
            && (planet.planet_type == 4u
                || (planet.planet_type == 3u && round(planet.color_seed.w) < 0.5)
                || planet.planet_type == 8u
                || (planet.planet_type == 9u && lava_frac > 0.01));
        if (is_cryotectonic) {
            let cryo_res = evaluate_cryotectonic_fractures(p_surf, planet.climate_and_bio.y, lava_frac, t, color);
            color = cryo_res.color;
            pbr_input.material.emissive += vec4<f32>(cryo_res.emissive * (1.0 - cloud_cover_alpha * 0.85), 0.0);
            pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, cryo_res.roughness, 0.6);
        }

        // Dynamic Surface Heatmap & Stefan-Boltzmann Blackbody Glow
        let is_tidally_locked = planet.tidal_deformation.w >= 1.5;
        let sun_cos = dot(surface_dir, normalize(planet.star_dir_and_lum.xyz));
        let pressure_bar = planet.atmosphere_params.x;
        let atm_insulation = clamp(pressure_bar / 1.0, 0.05, 0.92);

        // Subsolar heating vs. nightside radiative cooling
        let day_heat = 0.35 + 1.25 * pow(max(sun_cos, 0.0), 0.25);
        let night_cool = 0.25 + 0.65 * atm_insulation;
        let local_temp_mult = mix(night_cool, day_heat, smoothstep(-0.15, 0.30, sun_cos));
        let local_temp = temp * local_temp_mult + lava_frac * 650.0;

        // Tidally locked permanent subsolar magma ocean ("Magma Eyeball")
        if (is_tidally_locked && local_temp > 850.0 && sun_cos > 0.35 && planet.planet_type != 1u && planet.planet_type != 2u) {
            let eyeball_melt = smoothstep(0.35, 0.92, sun_cos) * smoothstep(850.0, 1200.0, local_temp);
            let magma_eye = evaluate_magma_ocean_surface(p_surf, 0.5, local_temp, 0.85, t);
            color = mix(color, magma_eye.color, eyeball_melt);
            pbr_input.material.emissive += vec4<f32>(magma_eye.emissive * eyeball_melt * (1.0 - cloud_cover_alpha * 0.85), 0.0);
            pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, magma_eye.roughness, eyeball_melt);
        }
        // Physical Stefan-Boltzmann thermal blackbody radiation for surfaces exceeding 520 K
        else if (local_temp > 520.0 && planet.planet_type != 1u && planet.planet_type != 2u) {
            let t_norm = clamp((local_temp - 520.0) / 950.0, 0.0, 3.5);
            let thermal_int = pow(t_norm, 2.8) * 0.35;
            let deep_cherry = vec3<f32>(0.65, 0.10, 0.03);
            let orange_gold = vec3<f32>(1.15, 0.48, 0.08);
            let white_core = vec3<f32>(1.65, 1.35, 0.85);
            let thermal_hue = mix(deep_cherry, mix(orange_gold, white_core, clamp((local_temp - 1200.0) / 800.0, 0.0, 1.0)), clamp((local_temp - 750.0) / 600.0, 0.0, 1.0));
            pbr_input.material.emissive += vec4<f32>(thermal_hue * thermal_int * (1.0 - cloud_cover_alpha * 0.80), 0.0);
        }
    }
}

    pbr_input.material.base_color = vec4<f32>(color, 1.0);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );

@if(PREPASS_PIPELINE)
    let out = deferred_output(in, pbr_input);
@if(!PREPASS_PIPELINE)
    var out: FragmentOutput;
    
@if(!PREPASS_PIPELINE)
    if (planet.planet_type == 0u) {
        // Universal Procedural Stellar Photosphere Engine
        out.color = render_stellar_photosphere(p_surf, norm, pbr_input.V, t);
    } else if (planet.planet_type == 7u) {
        // Dedicated Black Hole Star (Quasi-Star / JWST Little Red Dot) Pseudo-Photosphere
        let quasi_photosphere = render_quasistar_photosphere(p_surf, norm, pbr_input.V, t);
        out.color = vec4<f32>(quasi_photosphere, 1.0);
    } else if (planet.planet_type == 5u) {
        // Gravitational singularity event horizon: rendered by the dedicated relativistic black hole accretion disk shader.
        // Fully transparent (material uses AlphaMode::Blend, so no depth write) so the billboard disk is never occluded.
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    } else {
        let lit = apply_pbr_lighting(pbr_input);
        
        let base_col = pbr_input.material.base_color.rgb;
        let base_lum = max(dot(base_col, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.001);
        let lit_lum = dot(lit.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));

        var star_dir = planet.star_dir_and_lum.xyz;
        if (dot(star_dir, star_dir) < 0.01) {
            star_dir = vec3<f32>(0.0, 1.0, 0.0);
        } else {
            star_dir = normalize(star_dir);
        }

        let geom_N = normalize(in.world_normal);
        let geom_NdotL = dot(geom_N, star_dir); // Macro-sphere solar zenith cosine

        // 1. Ring Shadow Calculation: Equatorial ring plane intersects ray from oblate surface towards star
        var ring_shadow = 1.0;
        if (planet.ring_shadow_params.w > 0.5) {
            let r_inner = planet.ring_shadow_params.x;
            let r_outer = planet.ring_shadow_params.y;
            let opt_depth = planet.ring_shadow_params.z;

            // Project twilight points (geom_NdotL < 0) onto the grazing solar limb circle so that:
            // (a) Cloud/surface normal perturbations in pbr_input.N never pollute the ray origin with speckles.
            // (b) Ring shadows continue smoothly through the atmospheric twilight zone without a bright step at NdotL = 0.
            // (c) Rays past the terminator never pass through the planet's interior to form a closed bullseye loop.
            let p_sun_facing = select(
                normalize(geom_N - geom_NdotL * star_dir),
                geom_N,
                geom_NdotL >= 0.0
            );
            let sin_lat_sh = dot(p_sun_facing, s_axis);
            let is_giant_sh = (planet.planet_type == 1u || planet.planet_type == 2u);
            let oblateness_sh = select(
                0.0,
                select(0.065, 0.098, planet.ring_shadow_params.w > 0.5 || planet.storm_features.x > 0.001),
                is_giant_sh
            );
            let oblate_scale_sh = 1.0 - oblateness_sh * (1.5 * sin_lat_sh * sin_lat_sh - 0.5);
            let p_oblate = p_sun_facing * oblate_scale_sh;

            let L_dot_S = dot(star_dir, s_axis);
            let P_dot_S = dot(p_oblate, s_axis);

            if (abs(L_dot_S) > 1e-4) {
                let t_ring = -P_dot_S / L_dot_S;
                if (t_ring > 0.0 && geom_NdotL > -0.35) {
                    let p_int = p_oblate + t_ring * star_dir;
                    let r_int = length(p_int);

                    if (r_int >= r_inner && r_int <= r_outer) {
                        let u = clamp((r_int - r_inner) / max(r_outer - r_inner, 0.001), 0.0, 1.0);

                        // Smooth penumbral ring density profile (C Ring, B Ring, Cassini Division, A Ring, Encke Gap, F Ring)
                        let c_ring = mix(0.10, 0.34, clamp(u / 0.22, 0.0, 1.0)) * smoothstep(0.0, 0.035, u) * (1.0 - smoothstep(0.195, 0.245, u));
                        let b_ring = mix(0.78, 0.96, smoothstep(0.22, 0.38, u)) * smoothstep(0.195, 0.245, u) * (1.0 - smoothstep(0.625, 0.665, u));
                        let cassini_floor = 0.08 * smoothstep(0.625, 0.665, u) * (1.0 - smoothstep(0.705, 0.745, u));
                        let encke_dip = 1.0 - 0.78 * exp(-pow((u - 0.865) / 0.011, 2.0));
                        let a_ring = 0.74 * encke_dip * smoothstep(0.705, 0.745, u) * (1.0 - smoothstep(0.935, 0.975, u));
                        let f_ring = 0.28 * smoothstep(0.935, 0.968, u) * (1.0 - smoothstep(0.968, 1.0, u));
                        let ring_density = c_ring + b_ring + cassini_floor + a_ring + f_ring;

                        let edge_feather = smoothstep(r_inner, r_inner + 0.035, r_int) * (1.0 - smoothstep(r_outer - 0.035, r_outer, r_int));
                        let shadow_atten = clamp(ring_density * opt_depth * edge_feather, 0.0, 0.95);
                        ring_shadow = 1.0 - shadow_atten;
                    }
                }
            }
        }

        // 2. Moon Solar Eclipse Calculation: Transiting moons cast umbra and penumbra spots
        var eclipse_light = 1.0;
        for (var i = 0u; i < 4u; i = i + 1u) {
            if (planet.eclipse_moons_data[i].x > 0.5) {
                let m_pos = planet.eclipse_moons_pos[i].xyz;
                let m_rad = planet.eclipse_moons_pos[i].w;
                let V = m_pos - geom_N;
                let t_close = dot(V, star_dir);
                if (t_close > 0.0) {
                    let d_sq = dot(V, V) - t_close * t_close;
                    let max_r = m_rad * 1.45;
                    if (d_sq < max_r * max_r) {
                        let d_perp = sqrt(max(d_sq, 0.0));
                        let umbra_r = m_rad * 0.70;
                        let penumbra_r = m_rad * 1.25;
                        let spot_shadow = smoothstep(umbra_r, penumbra_r, d_perp);
                        let shadow_depth = select(0.96, planet.eclipse_moons_data[i].z, planet.eclipse_moons_data[i].z > 0.0);
                        let moon_illum = mix(1.0 - shadow_depth, 1.0, spot_shadow);
                        eclipse_light = min(eclipse_light, moon_illum);
                    }
                }
            }
        }

        let total_shadow = ring_shadow * eclipse_light;
        let NdotL = dot(pbr_input.N, star_dir); // Micro-surface solar zenith cosine

        // Has significant atmosphere or photochemical haze (including Pluto subtype 1.0)
        let has_atmo_wrap = (planet.planet_type != 4u && (pressure_bar > 0.005 || cloud_density > 0.05 || planet.planet_type == 1u || planet.planet_type == 2u || planet.planet_type == 6u))
            || (planet.planet_type == 9u && abs(planet.color_seed.w - 1.0) < 0.25);

        // Soft penumbral & atmospheric twilight terminator wraparound:
        // Prevents a harsh Lambertian step at NdotL = 0 by wrapping multiple-scattered twilight
        // and finite solar-disk penumbra smoothly into the civil/nautical twilight zone.
        let wrap_width = select(0.11, 0.26, has_atmo_wrap);
        let wrapped_ndotl = clamp((NdotL + wrap_width) / (1.0 + wrap_width), 0.0, 1.0);
        let terminator_soft_wrap = smoothstep(-wrap_width, wrap_width * 1.15, geom_NdotL)
            * wrapped_ndotl
            * select(0.14, 0.28, has_atmo_wrap)
            * total_shadow;

        // Warm Rayleigh-reddened twilight belt + cool stratospheric scatter along the terminator
        let twilight_band = exp(-pow((geom_NdotL + 0.02) / select(0.10, 0.16, has_atmo_wrap), 2.0));
        let twilight_tint = select(
            vec3<f32>(0.95, 0.88, 0.80),
            mix(vec3<f32>(1.08, 0.56, 0.30), vec3<f32>(0.48, 0.74, 1.05), smoothstep(0.05, -0.18, geom_NdotL)),
            has_atmo_wrap
        );
        let twilight_scatter = base_col * twilight_tint * twilight_band * max(wrapped_ndotl, 0.18) * select(0.08, 0.22, has_atmo_wrap) * total_shadow;

        // Measure starlight illuminance factor relative to surface albedo, modulated by shadows
        let raw_starlight = lit_lum / base_lum;
        let starlight_factor = raw_starlight * total_shadow + terminator_soft_wrap;
        
        // Tone-map diffuse lighting with an energy-conserving soft-knee curve
        let diffuse_intensity = clamp(starlight_factor / (starlight_factor + 0.95) * 1.05, 0.0, 1.0);
        let diffuse_lit = base_col * diffuse_intensity + twilight_scatter;
        
        // Physically accurate neutral specular highlight for dielectrics (rock, ice, dust)
        // Avoids chromatic inversion (which subtracted high-red diffuse from starlight and turned orange/warm planet highlights purple/blue)
        let H_vec = normalize(star_dir + pbr_input.V);
        let NdotH = max(dot(pbr_input.N, H_vec), 0.0);
        let star_facing = max(dot(pbr_input.N, star_dir), 0.0);
        let roughness = clamp(pbr_input.material.perceptual_roughness, 0.04, 1.0);
        let shininess = mix(128.0, 4.0, roughness * roughness);
        let spec_power = pow(NdotH, shininess) * star_facing;
        let spec_color = mix(vec3<f32>(1.0, 1.0, 1.0), base_col, pbr_input.material.metallic);
        let specular_glint = spec_color * spec_power * (1.0 - roughness * 0.65) * 0.45 * total_shadow * cloud_shadow_transmittance;

        // Anisotropic Cox-Munk Solar Sunglint & Fresnel Sunset Sheen on liquid oceans/seas
        var cox_munk_sunglint = vec3<f32>(0.0);
        if (water_mask > 0.01 && cloud_cover_alpha < 0.92) {
            let VdotH = max(dot(pbr_input.V, H_vec), 0.0);
            let schlick_f0 = 0.024;
            let water_fresnel = schlick_f0 + (1.0 - schlick_f0) * pow(1.0 - VdotH, 5.0);
            let sun_zenith = dot(surface_dir, star_dir);
            let sun_elev_pos = smoothstep(-0.04, 0.16, sun_zenith);
            let glint_core = pow(NdotH, 110.0) * 1.55;
            let glint_trail = pow(NdotH, 20.0) * 0.42;
            let sunset_tint = mix(
                vec3<f32>(1.0, 0.46, 0.15),
                vec3<f32>(1.0, 0.96, 0.88),
                clamp(sun_zenith * 2.6, 0.0, 1.0)
            );
            cox_munk_sunglint = sunset_tint
                * (glint_core + glint_trail)
                * (0.35 + water_fresnel * 2.4)
                * water_mask
                * (1.0 - cloud_cover_alpha * 0.90)
                * sun_elev_pos
                * total_shadow
                * cloud_shadow_transmittance;
        }
        
        let balanced_lit = diffuse_lit + specular_glint + cox_munk_sunglint;
        
        let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0);
        let fresnel = pow(1.0 - NdotV, 2.8);

        // Night-Side Illumination: Directional Reflected Planetshine + Relief-Preserving Starlight + Nocturnal Airglow
        let night_mask = smoothstep(0.18, -0.25, geom_NdotL);
        let has_companion = planet.eclipse_moons_data[0].x > 0.5 && dot(planet.eclipse_moons_pos[0].xyz, planet.eclipse_moons_pos[0].xyz) > 0.001;
        let planetshine_dir = select(
            normalize(-star_dir * 0.82 + geom_N * 0.35 + vec3<f32>(0.18, 0.24, -0.15)),
            normalize(planet.eclipse_moons_pos[0].xyz),
            has_companion
        );
        let ps_ndotl = clamp(dot(pbr_input.N, planetshine_dir) * 0.65 + 0.35, 0.0, 1.0);
        let normal_relief = clamp(dot(pbr_input.N, geom_N), 0.35, 1.0);
        let planetshine_tint = vec3<f32>(0.68, 0.82, 1.06);
        let planetshine_strength = select(0.055, 0.095, has_companion || planet.composition.y > 0.25);
        let night_planetshine = base_col * planetshine_tint * ps_ndotl * normal_relief * planetshine_strength * night_mask;
        let night_airglow = select(vec3<f32>(0.0), vec3<f32>(0.22, 0.42, 0.78) * fresnel * 0.045 * night_mask, has_atmo_wrap);
        let ambient_boost = base_col * (0.042 * normal_relief) + night_planetshine + night_airglow;
        
        // Phase 2: Dimensionless Spherical Chapman + Multi-Scattering Atmospheric Integrator
        // Operates in exact unit-sphere local coordinates (R_planet = 1.0) around `geom_N`,
        // preserving 100% 24-bit floating-point precision at any distance in the solar system.
        var atmospheric_haze = vec3<f32>(0.0);
        var surface_attenuation = vec3<f32>(1.0);
        
        let is_hazy_dwarf = (planet.planet_type == 9u) && (pressure_bar > 0.0001 || planet.atmosphere_params.z > 0.01 || abs(planet.color_seed.w - 1.0) < 0.25);
        if (planet.planet_type != 4u && (is_hazy_dwarf || (planet.planet_type != 9u && (pressure_bar > 0.005 || cloud_density > 0.05 || planet.planet_type == 1u || planet.planet_type == 2u || planet.planet_type == 6u)))) {
            let h_r = clamp(planet.atmosphere_params.y, 0.012, 0.14);
            let h_m = h_r * 0.38;
            let r_atmo = 1.0 + h_r * 4.8;
            let r_over_hr = 1.0 / h_r;
            let r_over_hm = 1.0 / h_m;

            let ray_dir = -pbr_input.V;
            // Intersect view ray arriving at unit surface point `geom_N` with outer atmosphere sphere `r_atmo`
            // Backward along `pbr_input.V` from `geom_N`: |geom_N + s * V|^2 = r_atmo^2
            let b_v = dot(geom_N, pbr_input.V);
            let c_v = 1.0 - r_atmo * r_atmo;
            let disc_v = max(b_v * b_v - c_v, 0.0);
            let path_len = max(-b_v + sqrt(disc_v), 0.001);

            let step_count = 6;
            let step_size = path_len / f32(step_count);

            let beta_r = select(planet.scattering_params.rgb, vec3<f32>(0.22, 0.58, 1.05) * 0.18, is_hazy_dwarf);
            let mie_scale = select(clamp(cloud_density * 2.0 + pressure_bar * 0.15, 0.1, 1.5), 0.25, is_hazy_dwarf);
            let beta_m = beta_r * mie_scale;
            // Chappuis Ozone (O3) stratospheric absorption band (550-600nm) for oxygen/ocean worlds
            let has_ozone = select(0.0, 1.0, (planet.planet_type == 3u || planet.planet_type == 6u) && ocean_frac > 0.02 && temp < 360.0);
            let beta_o3 = vec3<f32>(0.65, 1.88, 0.08) * 0.45 * has_ozone;
            let g_mie = clamp(planet.scattering_params.w, 0.55, 0.90);

            let cos_theta = dot(ray_dir, star_dir);
            let cos2_theta = cos_theta * cos_theta;
            let p_rayleigh = (3.0 / (16.0 * 3.14159265)) * (1.0 + cos2_theta);
            let p_mie = dual_lobe_mie_phase(cos_theta, g_mie);
            let p_iso = 1.0 / (4.0 * 3.14159265);

            var opt_depth_r = 0.0;
            var opt_depth_m = 0.0;
            var opt_depth_o3 = 0.0;
            var in_scatter = vec3<f32>(0.0);

            for (var i = 0; i < step_count; i = i + 1) {
                // March from top of atmosphere (s = path_len) down to surface (s = 0)
                let s_back = path_len - (f32(i) + 0.5) * step_size;
                let p_rel = geom_N + pbr_input.V * s_back;
                let sample_r = max(length(p_rel), 1.0);
                let alt = max(sample_r - 1.0, 0.0);

                let rho_r = exp(-alt / h_r);
                let rho_m = exp(-alt / h_m);
                let o3_profile = exp(-pow((alt - h_r * 1.15) / (h_r * 0.72), 2.0));

                let d_r = rho_r * step_size;
                let d_m = rho_m * step_size;
                let d_o3 = o3_profile * step_size;

                opt_depth_r += d_r;
                opt_depth_m += d_m;
                opt_depth_o3 += d_o3;

                let mu_s = dot(p_rel / sample_r, star_dir);
                let twilight_gate = smoothstep(-0.28, 0.06, mu_s);
                let ch_r = chapman_spherical_airmass(r_over_hr, mu_s);
                let ch_m = chapman_spherical_airmass(r_over_hm, mu_s);

                let s_depth_r = rho_r * h_r * min(ch_r, 18.0);
                let s_depth_m = rho_m * h_m * min(ch_m, 14.0);
                let s_depth_o3 = o3_profile * h_r * min(ch_r * 0.85, 16.0);

                let tau_sun = beta_r * s_depth_r + beta_m * s_depth_m + beta_o3 * s_depth_o3;
                let tau_view = beta_r * opt_depth_r + beta_m * opt_depth_m + beta_o3 * opt_depth_o3;
                let trans_single = exp(-(tau_sun + tau_view)) * twilight_gate;

                // Crepuscular volumetric god-rays: low-troposphere air parcels below the cloud deck
                // are modulated by the 3D ray-cast cloud shadow transmittance!
                let godray_mod = mix(cloud_shadow_transmittance, 1.0, clamp(alt / (h_r * 1.6), 0.0, 1.0));

                let single_slice = (beta_r * d_r * p_rayleigh + beta_m * d_m * p_mie) * trans_single * godray_mod;

                // Hillaire 2nd/3rd-order isotropic multiple-scattering energy transfer approximation
                let albedo_ms = vec3<f32>(0.88, 0.90, 0.94);
                let ms_trans = exp(-(tau_sun * 0.38 + tau_view * 0.45)) * smoothstep(-0.34, 0.14, mu_s);
                let multi_slice = (beta_r * d_r + beta_m * d_m * 0.65) * albedo_ms * p_iso * ms_trans * 0.55;

                in_scatter += single_slice + multi_slice;
            }

            let sun_visible = mix(0.15, 1.0, total_shadow);
            atmospheric_haze = in_scatter * sun_visible * 5.8;
            surface_attenuation = exp(-(beta_r * opt_depth_r + beta_m * opt_depth_m + beta_o3 * opt_depth_o3 * 0.5));
        }
        
        // Dynamic Polar Auroral Ovals & Coronal Excitation
        var aurora_glow = vec3<f32>(0.0);
        let oval_colat = planet.aurora_params.x;
        let oval_width = max(planet.aurora_params.y, 0.028); // tighter default
        let aurora_int = planet.aurora_params.z;
        let kp_index = planet.aurora_params.w;
        let hex_amp = planet.storm_features.x;

        let effective_aurora_int = select(
            aurora_int,
            clamp(mag_gauss * 0.8, 0.0, 2.0),
            aurora_int <= 0.01 && mag_gauss > 0.15
        );
        let effective_colat = clamp(select(oval_colat, 0.28, oval_colat <= 0.01), 0.16, 0.44);

        let is_giant_atmo = (planet.planet_type == 1u || planet.planet_type == 2u);

        // Terrestrial visible-light auroral curtains (oxygen 557.7nm / nitrogen 391.4nm) on rocky worlds.
        // Gas giants & ice giants (H2/He) emit predominantly far-UV and x-ray, invisible in optical daylight.
        if (effective_aurora_int > 0.08
            && pressure_bar >= 0.05
            && !is_giant_atmo)
        {
            // Symmetrical angle from nearest magnetic/rotational pole:
            // polar_angle is abs(sin_lat) = abs(dot(norm, s_axis)), which is cos(colatitude from nearest pole).
            let polar_co = acos(clamp(polar_angle, 0.0, 1.0));
            let d_oval = abs(polar_co - effective_colat);
            // Crisp Gaussian curtain
            let oval_ring = exp(-0.5 * (d_oval * d_oval) / (oval_width * oval_width * 0.45));

            // CRITICAL FIX: The auroral oval is an annulus. Fade out smoothly towards the polar center
            // to completely eliminate the coordinate singularity pinwheel / flower pattern!
            let pole_fade = smoothstep(0.04, 0.14, polar_co);

            // On gas giants with a standing Rossby hexagon (Saturn), completely silence aurora at the north pole
            let hex_suppression = select(1.0, 0.0, is_giant_atmo && hex_amp > 0.001 && dot(norm, s_axis) > 0.0);

            if (oval_ring * pole_fade * hex_suppression > 0.005) {
                // Symmetrical azimuthal coordinate around true 3D spin axis
                let az = atan2(p_tilted.z, p_tilted.x);
                let ray_noise = fbm(vec3<f32>(az * 16.0 + t * 0.35, sin_lat * 24.0, t * 0.20));
                let curtain_rays = smoothstep(0.32, 0.70, ray_noise);

                let edge_ratio = d_oval / oval_width;

                // Color palette: Earth has emerald oxygen 557.7nm + ruby nitrogen.
                // Gas / Ice giants (H2/He) emit predominantly UV and faint visible H3+ violet/cyan
                var auroral_col: vec3<f32>;
                if (is_giant_atmo) {
                    let violet_h3 = vec3<f32>(0.35, 0.30, 0.90);
                    let cyan_h2 = vec3<f32>(0.20, 0.75, 0.95);
                    auroral_col = mix(violet_h3, cyan_h2, smoothstep(0.2, 0.8, edge_ratio));
                } else {
                    let emerald_core = vec3<f32>(0.12, 0.98, 0.42);
                    let violet_edge = vec3<f32>(0.45, 0.20, 0.95);
                    let ruby_fringe = vec3<f32>(0.95, 0.15, 0.25);
                    auroral_col = mix(
                        emerald_core,
                        mix(violet_edge, ruby_fringe, smoothstep(0.6, 1.2, edge_ratio)),
                        smoothstep(0.3, 0.9, edge_ratio)
                    );
                }

                // In broad daylight (NdotL > 0.05), auroras are washed out by sunlight!
                // For gas giants, daylight washes out faint auroras completely; on nightside it glows softly.
                let night_side_mult = select(
                    smoothstep(0.15, -0.35, NdotL) * 1.15 + 0.10,
                    smoothstep(0.00, -0.40, NdotL) * 0.75,
                    is_giant_atmo
                );

                let storm_flicker = sin(t * 4.0 + az * 8.0) * 0.10 + 0.90;

                // Quiet ~0.8, strong storm ~2.2
                let intensity_scale = mix(0.8, 2.2, clamp(kp_index / 9.0, 0.0, 1.0));

                aurora_glow = auroral_col
                    * (oval_ring
                        * pole_fade
                        * hex_suppression
                        * curtain_rays
                        * effective_aurora_int
                        * night_side_mult
                        * storm_flicker
                        * intensity_scale);

                // Strictly polar factor: zero out below 45° latitude, full strength at > 62° latitude
                let polar_factor = smoothstep(0.65, 0.88, polar_angle);
                aurora_glow = aurora_glow * polar_factor;
            }
        }

        // HDR scene-linear radiance accumulation. All emissive sources (lava pools, comet
        // gas vents, biomass phosphorescence) are pre-scaled at their HDR intensities above.
        out.color = vec4<f32>((balanced_lit + ambient_boost + pbr_input.material.emissive.rgb) * surface_attenuation + atmospheric_haze + aurora_glow, 1.0);
    }
    
@if(!PREPASS_PIPELINE)
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

    // 3D Planetary Internal Cutaway Mode: render internal geological cross-section in +X / +Z quadrant
@if(!PREPASS_PIPELINE && CUTAWAY_ENABLED)
    if (planet.cutaway_params.x > 0.5) {
        let cut_res = evaluate_internal_cutaway(
            p_tilted,
            -pbr_input.V,
            s_tangent_x,
            s_axis,
            s_tangent_z,
            planet.cutaway_params.y,
            planet.cutaway_params.z,
            planet.cutaway_params.w,
            planet.time,
            planet.climate_and_bio.x,
        );
        if (cut_res.is_cut) {
            out.color = vec4<f32>(cut_res.color + cut_res.emissive, 1.0);
        }
    }

    return out;
}
