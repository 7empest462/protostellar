#import bevy_pbr::{
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_functions::{get_world_from_local, mesh_position_local_to_clip, mesh_position_local_to_world, mesh_normal_local_to_world},
    mesh_bindings::mesh,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

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

@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var<uniform> planet: PlanetExtension;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
#ifdef VERTEX_TANGENTS
    @location(3) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(4) color: vec4<f32>,
#endif
};


fn get_vertex_displacement(p_surf: vec3<f32>) -> f32 {
    let p_type = planet.planet_type;
    // Only terrestrial (3), ice worlds (4), and super-earths (6) get terrain displacement.
    // Small bodies (8) and all others get a flat surface.
    if (p_type != 3u && p_type != 4u && p_type != 6u) {
        return 0.0;
    }
    var elev = 0.0;
    let ocean_frac = planet.climate_and_bio.x;
    if (p_type == 3u || p_type == 6u) {
        let drift_phase = planet.spin_axis.w;
        let plate_drift = vec3<f32>(
            sin(drift_phase * 6.283 + p_surf.z * 2.6) * 0.28,
            cos(drift_phase * 3.14159 + p_surf.x * 2.2) * 0.12,
            sin(drift_phase * 4.712 + p_surf.y * 2.4) * 0.28
        );
        let p_tectonic = p_surf + plate_drift;
        let continent_mask = eroded_continent_fbm(p_tectonic * 1.85);
        let regional_hills = fbm(p_tectonic * 5.5) * 0.32;
        let mountain_ridges = ridge_noise(p_tectonic * 8.0) * 0.20;
        let micro_detail = fbm(p_tectonic * 22.0) * 0.10;
        let aggregation = planet.geological_params.y;
        let super_axis = normalize(vec3<f32>(0.65, 0.25, 0.70));
        let cluster_dipole = dot(p_surf, super_axis);
        let cluster_bias = cluster_dipole * (aggregation - 0.20) * 0.14;

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

        let has_oceans = ocean_frac > 0.01 && planet.temperature <= 380.0;
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
            has_oceans
        );

        if (has_oceans && elev < sea_level) {
            elev = sea_level;
        }
    } else if (p_type == 4u) {
        let regolith = fbm(p_surf * 14.0);
        let micro_craters = fbm(p_surf * 22.0);
        let boulder_noise = fbm(p_surf * 38.0);
        let variegation = fbm(p_surf * 5.5);
        elev = regolith * 0.3 + micro_craters * 0.15 + boulder_noise * 0.05 + variegation * 0.4;
    }
    // Evaluate Impact Basins
    for (var i = 0u; i < 4u; i = i + 1u) {
        let b_pos = planet.impact_basins_pos[i];
        let b_data = planet.impact_basins_data[i];
        if (b_data.w > 0.5) {
            let center_n = b_pos.xyz;
            let r_ang = b_pos.w;
            let dot_p = dot(p_surf, center_n);
            let dist_ang = acos(clamp(dot_p, -1.0, 1.0));
            if (dist_ang < r_ang * 1.6) {
                let norm_dist = dist_ang / r_ang;
                let basin_depth = b_data.z;
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


@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    let local_n = normalize(vertex.normal);
    let p_surf = get_spun_p_surf(local_n, vertex.instance_index);
    let elev = get_vertex_displacement(p_surf);
    
    let displacement_factor = 0.02; // Reduced to 2% to fix jagged shadows and extreme spikes
    var scale = displacement_factor;
    if (planet.planet_type == 4u) { scale = 0.04; }
    
    var displaced_local_pos = vertex.position + local_n * (elev * scale);
    
    var world_from_local = get_world_from_local(vertex.instance_index);
    let base_world_n = normalize(mesh_normal_local_to_world(local_n, vertex.instance_index));

    // --- 1. ROTATIONAL OBLATENESS (Maclaurin Spheroid Squishing for Gas/Ice Giants) ---
    var s_axis = planet.spin_axis.xyz;
    if (length(s_axis) < 0.1) { s_axis = vec3<f32>(0.0, 1.0, 0.0); } else { s_axis = normalize(s_axis); }
    let sin_lat = dot(base_world_n, s_axis);
    let is_giant = (planet.planet_type == 1u || planet.planet_type == 2u);
    let has_saturn_rings = planet.ring_shadow_params.w > 0.5 || planet.storm_features.x > 0.001;
    let oblateness_amount = select(0.065, 0.098, has_saturn_rings);
    let oblateness = select(0.0, oblateness_amount, is_giant);
    let oblate_factor = 1.0 - oblateness * (1.5 * sin_lat * sin_lat - 0.5);
    displaced_local_pos *= oblate_factor;

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
    out.uv = vertex.uv;
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
    return out;
}





// 3D coordinate rotations
fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(p.x * c - p.z * s, p.y, p.x * s + p.z * c);
}

fn rotate_z(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(p.x * c - p.y * s, p.x * s + p.y * c, p.z);
}

// 3D hash
fn hash3(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.x + r.y) * r.z);
}

// 3D value noise
fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    let n000 = hash3(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));
    
    let mx0 = mix(n000, n100, u.x);
    let mx1 = mix(n010, n110, u.x);
    let mx2 = mix(n001, n101, u.x);
    let mx3 = mix(n011, n111, u.x);
    
    let my0 = mix(mx0, mx1, u.y);
    let my1 = mix(mx2, mx3, u.y);
    
    return mix(my0, my1, u.z);
}

// Fractal Brownian Motion (5 octaves)
fn fbm(p: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var shift = vec3<f32>(100.0);
    var pos = p;
    for (var i = 0; i < 5; i = i + 1) {
        v += a * noise(pos);
        pos = pos * 2.02 + shift;
        a *= 0.5;
    }
    return v;
}

// Derivative-warped hydraulic erosion FBM for realistic dendritic mountain ranges and flat valleys
fn eroded_continent_fbm(p: vec3<f32>) -> f32 {
    var val = 0.0;
    var amp = 0.50;
    var pos = p;
    var warp_accum = vec3<f32>(0.0);
    let eps = 0.03;

    for (var i = 0; i < 5; i = i + 1) {
        let sample_p = pos - warp_accum * 0.16;
        let n0 = noise(sample_p);
        let nx = noise(sample_p + vec3<f32>(eps, 0.0, 0.0)) - n0;
        let ny = noise(sample_p + vec3<f32>(0.0, eps, 0.0)) - n0;
        let nz = noise(sample_p + vec3<f32>(0.0, 0.0, eps)) - n0;
        let grad = vec3<f32>(nx, ny, nz) / eps;
        warp_accum += grad * amp;

        let slope_mag = length(warp_accum);
        let erosion_weight = 1.0 / (1.0 + slope_mag * 0.45);
        val += amp * n0 * erosion_weight;

        pos = pos * 2.04 + vec3<f32>(43.1, 17.7, 89.3);
        amp *= 0.49;
    }
    return val;
}

// High-frequency ridge noise for tectonic rift cracks & volcanic fissures
fn ridge_noise(p: vec3<f32>) -> f32 {
    let n = noise(p);
    return 1.0 - abs(n * 2.0 - 1.0);
}

// Vortex swirl distortion for anticyclonic storms and hurricanes
fn vortex_swirl(p: vec3<f32>, center: vec3<f32>, radius: f32, strength: f32) -> vec3<f32> {
    let d = distance(p, center);
    if (d < radius) {
        let factor = (1.0 - d / radius);
        let angle = factor * factor * strength;
        let s = sin(angle);
        let c = cos(angle);
        let rel = p - center;
        return center + vec3<f32>(rel.x * c - rel.z * s, rel.y, rel.x * s + rel.z * c);
    }
    return p;
}

// 3D vector hash for cellular noise
fn hash3_vec(p: vec3<f32>) -> vec3<f32> {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.xxy + r.yxx) * r.zyx);
}

// 3D Voronoi / Cellular noise with dynamic time-dependent cell boiling
// Returns vec2<f32>:
//   x: d1 = distance to nearest cell center (hot rising convective core)
//   y: d2 = distance to second nearest cell center
//   (d2 - d1): distance to cell boundary (cool sinking intergranular lane)
fn voronoi3(p: vec3<f32>, anim_time: f32) -> vec2<f32> {
    let i = floor(p);
    let f = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    
    for (var z = -1; z <= 1; z = z + 1) {
        for (var y = -1; y <= 1; y = y + 1) {
            for (var x = -1; x <= 1; x = x + 1) {
                let neighbor = vec3<f32>(f32(x), f32(y), f32(z));
                let h = hash3_vec(i + neighbor);
                // Dynamic cell boiling motion: centers wander in 3D over time
                let point = 0.5 + 0.38 * sin(h * 6.28318 + vec3<f32>(anim_time));
                let diff = neighbor + point - f;
                let dist = length(diff);
                if (dist < d1) {
                    d2 = d1;
                    d1 = dist;
                } else if (dist < d2) {
                    d2 = dist;
                }
            }
        }
    }
    return vec2<f32>(d1, d2);
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

struct CraterShadingResult {
    color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    emissive: vec3<f32>,
    normal_delta: vec3<f32>,
};

// -----------------------------------------------------------------------------
// Specialized Planetary Impact Regimes: Gas Giants, Ice Giants, Icy Moons, Rocky Worlds
// -----------------------------------------------------------------------------

fn compute_crater_radial_slope(u: f32, radius: f32) -> f32 {
    // Inner terraced wall (0.70..0.98) tilts normal inward (-radial_dir);
    // Outer rampart slope (0.98..1.28) tilts normal outward (+radial_dir).
    let inner_wall = smoothstep(0.68, 0.90, u) * smoothstep(0.99, 0.90, u);
    let outer_flank = smoothstep(0.97, 1.06, u) * smoothstep(1.32, 1.06, u);
    var slope = -inner_wall * 0.85 + outer_flank * 0.48;

    // Central rebound peak (u < 0.24)
    if (radius > 0.10 && u < 0.24) {
        let peak_flank = smoothstep(0.02, 0.11, u) * smoothstep(0.24, 0.11, u);
        slope += peak_flank * 0.65;
    }

    // Multi-ring inner scarp for large impact basins (Orientale / Caloris style)
    if (radius > 0.18 && u > 0.42 && u < 0.64) {
        let ring_in = smoothstep(0.42, 0.51, u) * smoothstep(0.54, 0.51, u);
        let ring_out = smoothstep(0.52, 0.57, u) * smoothstep(0.64, 0.57, u);
        slope += -ring_in * 0.45 + ring_out * 0.30;
    }
    return slope;
}

fn apply_gas_giant_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    seismic_phase: f32,
    time: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    // Zonal wind shear: strong equatorial and mid-latitude jet streams (100-150 m/s)
    // shear the impact cloud horizontally along latitude bands into an elongated oval plume.
    let tangent = normalize(cross(center, vec3<f32>(0.0, 1.0, 0.001)));
    let d_east = dot(p_surf - center, tangent);
    let d_north = length((p_surf - center) - tangent * d_east);
    let sheared_dist = sqrt((d_east / 1.7) * (d_east / 1.7) + d_north * d_north);
    let u = sheared_dist / radius;

    if (u < 2.2) {
        let turb = fbm(p_surf * 22.0 + vec3<f32>(time * 0.06, 0.0, 0.0));

        // Shoemaker-Levy 9 dark carbonaceous & sulfurous aerosol soot core
        let soot_mask = smoothstep(0.70, 0.18, u) * (0.80 + turb * 0.20) * effective_scar;
        let soot_color = vec3<f32>(0.04, 0.025, 0.02);

        // Concentric acoustic gravity wave shock ring / stratospheric fallout halo
        let halo_mask = smoothstep(0.75, 1.0, u) * smoothstep(1.45, 1.0, u) * (0.45 + turb * 0.25) * effective_scar;
        let halo_color = vec3<f32>(0.12, 0.08, 0.06);

        out_res.color = mix(out_res.color, halo_color, halo_mask * 0.70);
        out_res.color = mix(out_res.color, soot_color, soot_mask * 0.95);
        out_res.roughness = mix(out_res.roughness, 0.75, soot_mask * 0.5);

        // Propagating atmospheric acoustic gravity-wave ring when fresh
        let active_melt = melt_glow * effective_scar;
        if (active_melt > 0.05) {
            let acoustic_u = 0.8 + seismic_phase * 1.35;
            let acoustic_ring = exp(-pow((u - acoustic_u) / 0.14, 2.0)) * active_melt * (1.0 - seismic_phase * 0.7);
            out_res.emissive += vec3<f32>(1.15, 0.55, 0.15) * acoustic_ring * 2.2;
        }

        // Stratospheric incandescent fireball flash when fresh
        if (active_melt > 0.005 && u < 0.85) {
            let fireball = smoothstep(0.85, 0.15, u) * active_melt;
            let flash_core = vec3<f32>(1.0, 0.65, 0.18) * (4.2 + sin(time * 3.0 + u * 6.0) * 0.6);
            out_res.color = mix(out_res.color, vec3<f32>(1.0, 0.85, 0.5), fireball * 0.65);
            out_res.emissive += flash_core * fireball;
        }
    }
    return out_res;
}

fn apply_ice_giant_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    seismic_phase: f32,
    time: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    // Neptune/Uranus supersonic retrograde zonal winds shear the anvil plume along cloud bands
    let tangent = normalize(cross(center, vec3<f32>(0.0, 1.0, 0.001)));
    let d_east = dot(p_surf - center, tangent);
    let d_north = length((p_surf - center) - tangent * d_east);
    let sheared_dist = sqrt((d_east / 1.55) * (d_east / 1.55) + d_north * d_north);
    let u = sheared_dist / radius;

    if (u < 2.1) {
        let cirrus_turb = fbm(p_surf * 26.0 + vec3<f32>(time * 0.05, 0.0, 0.0));

        // Deep dark indigo-black atmospheric vortex cavity (excavates deep tholins)
        let vortex_mask = smoothstep(0.55, 0.15, u) * effective_scar;
        let vortex_color = vec3<f32>(0.015, 0.06, 0.18);

        // High-albedo methane & ammonia ice cirrus anvil plume ringing the impact site
        let cirrus_mask = smoothstep(0.40, 0.75, u) * smoothstep(1.40, 0.85, u)
            * (0.65 + cirrus_turb * 0.35) * effective_scar;
        let cirrus_color = vec3<f32>(0.92, 0.98, 1.0);

        out_res.color = mix(out_res.color, vortex_color, vortex_mask * 0.90);
        out_res.color = mix(out_res.color, cirrus_color, cirrus_mask * 0.85);

        let active_melt = melt_glow * effective_scar;
        if (active_melt > 0.05) {
            let wave_u = 0.75 + seismic_phase * 1.25;
            let shock_ring = exp(-pow((u - wave_u) / 0.13, 2.0)) * active_melt * (1.0 - seismic_phase * 0.7);
            out_res.emissive += vec3<f32>(0.35, 0.88, 1.15) * shock_ring * 2.0;
        }

        // High-energy atmospheric cyan/blue shock glow when fresh
        if (active_melt > 0.005 && u < 0.85) {
            let shock_flare = smoothstep(0.85, 0.10, u) * active_melt;
            let flare_core = vec3<f32>(0.35, 0.85, 1.0) * (3.8 + sin(time * 3.2 + u * 8.0) * 0.6);
            out_res.color = mix(out_res.color, vec3<f32>(0.65, 0.92, 1.0), shock_flare * 0.60);
            out_res.emissive += flare_core * shock_flare;
        }
    }
    return out_res;
}

fn apply_icy_world_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radial_dir: vec3<f32>,
    dist: f32,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    time: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    let u = dist / radius;

    if (u < 2.2) {
        let ray_angle = atan2(p_surf.x - center.x, p_surf.z - center.z);
        let ray_noise = sin(ray_angle * 16.0 + fbm(p_surf * 20.0) * 4.0) * 0.5 + 0.5;

        // High-albedo shattered crystalline ice ejecta blanket and rays
        let ejecta_ice = smoothstep(2.1, 0.90, u) * ray_noise * 0.62 * effective_scar;
        let pure_ice_ray = vec3<f32>(0.90, 0.95, 1.0);

        // Raised frosty ice rim
        let rim_factor = smoothstep(0.75, 0.98, u) * smoothstep(1.25, 0.98, u) * effective_scar;
        let rim_ice = vec3<f32>(0.82, 0.88, 0.96);

        // Subsurface cryo-rock / tholin-tinted basin floor
        let floor_factor = smoothstep(0.85, 0.70, u) * effective_scar;
        let cryo_floor = vec3<f32>(0.22, 0.24, 0.32);

        // Central rebound peak of crystalline water-ice
        var central_peak = 0.0;
        if (radius > 0.10 && u < 0.22) {
            central_peak = smoothstep(0.22, 0.05, u) * 0.75 * effective_scar;
        }

        var crater_col = mix(out_res.color, pure_ice_ray, ejecta_ice);
        crater_col = mix(crater_col, rim_ice, rim_factor);
        crater_col = mix(crater_col, cryo_floor, floor_factor);
        crater_col = mix(crater_col, pure_ice_ray, central_peak);

        out_res.color = crater_col;
        out_res.roughness = mix(out_res.roughness, 0.35, (rim_factor * 0.6 + ejecta_ice * 0.8));
        out_res.metallic = mix(out_res.metallic, 0.02, floor_factor * 0.5);

        let slope = compute_crater_radial_slope(u, radius);
        out_res.normal_delta += radial_dir * (slope * effective_scar * 0.55);

        // Slushy cryomagma pool (water-ammonia slush) glowing with pale cyan/blue luminescence
        let active_melt = melt_glow * effective_scar;
        if (active_melt > 0.005 && u < 0.80) {
            let pool_mask = smoothstep(0.80, 0.50, u);
            let slush_cracks = fbm(p_surf * 30.0 + vec3<f32>(time * 0.04, 0.0, 0.0));
            let cryo_core = vec3<f32>(0.25, 0.75, 0.98) * (3.0 + sin(time * 2.6 + u * 8.0) * 0.5);
            let frozen_slush = vec3<f32>(0.35, 0.48, 0.60);

            let is_open_slush = smoothstep(0.35, 0.65, slush_cracks);
            let slush_mix = mix(frozen_slush, cryo_core, is_open_slush);

            out_res.color = mix(out_res.color, slush_mix, pool_mask * active_melt);
            out_res.emissive += cryo_core * pool_mask * active_melt * (is_open_slush * 0.85 + 0.15);
            out_res.roughness = mix(out_res.roughness, 0.18, pool_mask * active_melt);
        }

        // Thermal cryomagma halo through shattered crystalline ice crust
        if (active_melt > 0.05 && u >= 0.80 && u < 2.15) {
            let halo_decay = exp(-2.4 * (u - 0.80));
            let halo_heat = active_melt * halo_decay * effective_scar;
            let halo_fissure = smoothstep(0.48, 0.85, ridge_noise(p_surf * 24.0));
            let cryo_halo = vec3<f32>(0.20, 0.70, 0.95) * halo_fissure * halo_heat * 2.2;
            out_res.emissive += cryo_halo;
            out_res.color = mix(out_res.color, vec3<f32>(0.45, 0.65, 0.80), halo_heat * 0.45);
        }
    }
    return out_res;
}

fn apply_rocky_world_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radial_dir: vec3<f32>,
    dist: f32,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    time: f32,
    water_mask: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    let u = dist / radius;

    if (u < 2.6) {
        let ray_angle = atan2(p_surf.x - center.x, p_surf.z - center.z);
        let ray_noise = sin(ray_angle * 14.0 + fbm(p_surf * 18.0) * 4.0) * 0.5 + 0.5;
        let ejecta_blanket = smoothstep(2.4, 0.95, u) * ray_noise * 0.48 * effective_scar;
        let ejecta_dust = vec3<f32>(0.52, 0.48, 0.44);

        let rim_factor = smoothstep(0.75, 0.98, u) * smoothstep(1.25, 0.98, u) * effective_scar;
        let rim_rock = vec3<f32>(0.40, 0.37, 0.34);

        let floor_factor = smoothstep(0.85, 0.70, u) * effective_scar;
        let basalt_mare = vec3<f32>(0.09, 0.085, 0.08);

        var central_peak = 0.0;
        if (radius > 0.12 && u < 0.22) {
            central_peak = smoothstep(0.22, 0.05, u) * 0.75 * effective_scar;
        }

        var crater_col = mix(out_res.color, ejecta_dust, ejecta_blanket);
        crater_col = mix(crater_col, rim_rock, rim_factor);
        crater_col = mix(crater_col, basalt_mare, floor_factor);
        crater_col = mix(crater_col, rim_rock, central_peak);

        out_res.color = crater_col;
        out_res.roughness = mix(out_res.roughness, 0.92, (floor_factor * 0.8 + rim_factor * 0.5) * (1.0 - water_mask));
        out_res.metallic = mix(out_res.metallic, 0.08, floor_factor * 0.7 * (1.0 - water_mask));

        let slope = compute_crater_radial_slope(u, radius);
        out_res.normal_delta += radial_dir * (slope * effective_scar * (1.0 - water_mask * 0.85) * 0.65);

        let active_melt = melt_glow * effective_scar;
        // Incandescent ballistic ejecta blanket streaks around fresh impacts (Universe Sandbox style)
        if (active_melt > 0.15 && u >= 0.82 && u < 2.55) {
            let hot_streaks = pow(ray_noise, 2.6) * smoothstep(2.55, 0.92, u) * smoothstep(0.15, 0.95, active_melt);
            let streak_fissure = smoothstep(0.52, 0.85, ridge_noise(p_surf * 28.0));
            let ejecta_glow = vec3<f32>(1.35, 0.45, 0.08) * hot_streaks * (0.45 + streak_fissure * 0.85) * 2.4;
            out_res.emissive += ejecta_glow;
            out_res.color = mix(out_res.color, vec3<f32>(0.95, 0.42, 0.10), clamp(hot_streaks * 0.55, 0.0, 0.8));
        }

        // Thermal Conduction Magma Halo extending into the fractured bedrock surrounding the crater
        if (active_melt > 0.05 && u >= 0.82 && u < 2.25) {
            let halo_decay = exp(-2.2 * (u - 0.82));
            let halo_heat = active_melt * halo_decay * effective_scar;
            let halo_fissure = smoothstep(0.48, 0.82, ridge_noise(p_surf * 26.0 + vec3<f32>(time * 0.02, 0.0, 0.0)));
            let halo_emissive = vec3<f32>(1.25, 0.42, 0.08) * halo_fissure * halo_heat * 2.8;
            out_res.emissive += halo_emissive;
            out_res.color = mix(out_res.color, vec3<f32>(0.14, 0.09, 0.07), halo_heat * 0.65);
        }

        if (active_melt > 0.005 && u < 0.86) {
            let pool_mask = smoothstep(0.86, 0.52, u);
            let convection_cracks = fbm(p_surf * 32.0 + vec3<f32>(time * 0.05, 0.0, 0.0));
            let ring_fissure = ridge_noise(p_surf * 24.0);
            let lava_core = vec3<f32>(1.15, 0.52, 0.09) * (4.2 + sin(time * 2.8 + u * 10.0) * 0.6);
            let white_hot = vec3<f32>(1.65, 1.35, 0.85) * smoothstep(0.70, 1.0, active_melt) * smoothstep(0.45, 0.05, u);
            let dark_crust = vec3<f32>(0.12, 0.09, 0.08);

            let is_open_lava = clamp(smoothstep(0.35, 0.62, convection_cracks) + smoothstep(0.68, 0.90, ring_fissure) * 0.6, 0.0, 1.0);
            let lava_mix = mix(dark_crust, lava_core + white_hot, is_open_lava);

            out_res.color = mix(out_res.color, lava_mix, pool_mask * active_melt);
            out_res.emissive += (lava_core + white_hot) * pool_mask * active_melt * (is_open_lava * 0.88 + 0.18);
            out_res.roughness = mix(out_res.roughness, 0.22, pool_mask * active_melt);
        }
    }
    return out_res;
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

        let effective_scar = scar_intensity * (1.0 - water_mask * (1.0 - melt_glow * 0.4));
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

        // Propagating 3D Crustal Rayleigh Seismic Shockwave & Antipodal Convergence on solid worlds
        if (!is_gas_giant && !is_ice_giant && melt_glow > 0.18) {
            if (seismic_phase > 0.005 && seismic_phase < 0.98) {
                let wave_front = mix(radius * 1.05, 3.05, seismic_phase);
                let wave_width = mix(0.04, 0.11, seismic_phase);
                let d_wave = dist - wave_front;
                let wave_env = exp(-0.5 * (d_wave * d_wave) / (wave_width * wave_width));
                if (wave_env > 0.01) {
                    let shock_atten = (1.0 - seismic_phase * 0.72) * smoothstep(0.18, 0.85, melt_glow);
                    let crust_fracture = smoothstep(0.45, 0.82, ridge_noise(p_surf * 26.0 + vec3<f32>(time * 0.2, 0.0, 0.0)));
                    let shock_hue = select(vec3<f32>(1.35, 0.45, 0.08), vec3<f32>(0.30, 0.85, 1.15), is_icy_world);
                    out_res.emissive += shock_hue * wave_env * (0.35 + crust_fracture * 0.95) * shock_atten * 3.2;
                    out_res.normal_delta += radial_dir * (-d_wave / wave_width) * wave_env * shock_atten * 0.55;
                }
            }
            // Antipodal Seismic Focusing (Caloris / Weird Terrain chaotic crustal fracturing opposite major impacts)
            if (radius > 0.18 && dist > (3.14159265 - radius * 1.45)) {
                let anti_u = (3.14159265 - dist) / (radius * 1.45);
                let anti_mask = smoothstep(1.0, 0.15, anti_u) * smoothstep(0.18, 0.65, radius);
                let chaotic_rift = smoothstep(0.58, 0.88, ridge_noise(p_surf * 30.0));
                let anti_glow = select(vec3<f32>(1.15, 0.35, 0.06), vec3<f32>(0.25, 0.75, 1.0), is_icy_world);
                out_res.emissive += anti_glow * chaotic_rift * anti_mask * melt_glow * 1.85;
                out_res.normal_delta += radial_dir * (chaotic_rift - 0.5) * anti_mask * 0.35;
            }
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

struct PolarHexagonResult {
    inside_mask: f32,
    jet_mask: f32,
    eye_mask: f32,
    color: vec3<f32>,
    is_active: bool,
};

// Evaluates Saturn-like standing Rossby wave polar hexagon (k=6 wavenumber)
// with high-speed jet perimeter, deep cyclonic eye, and golden interior haze.
fn evaluate_polar_hexagon(
    p_gas: vec3<f32>,
    t: f32,
    hex_amplitude: f32,
    hex_wavenumber: f32,
) -> PolarHexagonResult {
    var res: PolarHexagonResult;
    res.inside_mask = 0.0;
    res.jet_mask = 0.0;
    res.eye_mask = 0.0;
    res.color = vec3<f32>(0.0);
    res.is_active = false;

    // Hexagon is localized to northern polar region (lat > ~50 deg N, colatitude < 0.70 rad)
    if (hex_amplitude < 0.001 || p_gas.y < 0.65) {
        return res;
    }

    let colatitude = acos(clamp(p_gas.y, -1.0, 1.0));
    let azimuth = atan2(p_gas.z, p_gas.x);
    let k = max(round(hex_wavenumber), 3.0);

    // Standing Rossby wave perimeter at ~77.5 deg North (colatitude ~ 0.218 rad)
    let r0 = 0.218;
    // Extremely slow planetary wave drift
    let wave_phase = k * azimuth - t * 0.006;
    // Primary Rossby harmonic + secondary harmonic for crisp geometric polygon straight edges
    let r_hex = r0 * (1.0 + hex_amplitude * cos(wave_phase) + (hex_amplitude * 0.22) * cos(2.0 * wave_phase));

    let delta_r = colatitude - r_hex;

    // High-speed jet perimeter ribbon (Gaussian width ~ 0.018 rad)
    let jet = exp(-pow(delta_r / 0.018, 2.0));

    // Interior warm polar haze (smooth step across boundary)
    let inside = smoothstep(0.025, -0.025, delta_r);

    // Deep central polar cyclonic eye at colatitude < 0.055 rad (~3 degrees from pole)
    let eye = smoothstep(0.055, 0.018, colatitude);
    let eyewall = smoothstep(0.015, 0.038, colatitude) * smoothstep(0.065, 0.042, colatitude);

    // Dynamic turbulent interior cloud structure
    let interior_turb = fbm(p_gas * 14.0 + vec3<f32>(t * 0.03, 0.0, -t * 0.03));

    // Saturn's golden-amber interior haze vs bright jet ribbon vs dark eye core
    let haze_gold = vec3<f32>(0.88, 0.68, 0.30) * (0.85 + interior_turb * 0.30);
    let jet_cream = vec3<f32>(0.98, 0.91, 0.70);
    let eye_dark = vec3<f32>(0.28, 0.14, 0.05);
    let eyewall_bright = vec3<f32>(0.95, 0.82, 0.52);

    var storm_col = mix(haze_gold, jet_cream, jet * 0.85);
    storm_col = mix(storm_col, eyewall_bright, eyewall * 0.90);
    storm_col = mix(storm_col, eye_dark, eye * 0.95);

    res.inside_mask = inside;
    res.jet_mask = jet;
    res.eye_mask = eye;
    res.color = storm_col;
    res.is_active = true;
    return res;
}

struct AnticyclonicVortexResult {
    spot_mask: f32,
    collar_mask: f32,
    eye_mask: f32,
    wake_mask: f32,
    oval_mask: f32,
    color: vec3<f32>,
    is_active: bool,
};

// Evaluates Jovian Great Red Spot anticyclone (aspect ratio ~2.2:1),
// high-speed outer collar, calm core, Kelvin-Helmholtz wake, Oval BA & white ovals.
fn evaluate_anticyclonic_vortices(
    p_gas: vec3<f32>,
    t: f32,
    spot_size: f32,
    spot_lat: f32,
    spot_lon: f32,
    vortex_spin_rate: f32,
    secondary_count: f32,
    mass_jup: f32,
    is_saturn: bool,
) -> AnticyclonicVortexResult {
    var res: AnticyclonicVortexResult;
    res.spot_mask = 0.0;
    res.collar_mask = 0.0;
    res.eye_mask = 0.0;
    res.wake_mask = 0.0;
    res.oval_mask = 0.0;
    res.color = vec3<f32>(0.0);
    res.is_active = false;

    if (spot_size < 0.01) {
        return res;
    }

    let lat = asin(clamp(p_gas.y, -1.0, 1.0));
    let lon = atan2(p_gas.z, p_gas.x);

    // 1. Primary Great Red Spot (GRS) - Elliptic vortex (aspect ratio ~2.2:1)
    let d_lat = lat - spot_lat;
    var d_lon = lon - spot_lon;
    // Wrap longitude difference to [-PI, PI]
    d_lon = d_lon - 6.2831853 * round(d_lon / 6.2831853);

    // Elliptic metric: semi-major a ~ 1.50, semi-minor b ~ 0.68
    let u = d_lon / (1.50 * spot_size);
    let v = d_lat / (0.68 * spot_size);
    let r_ell = sqrt(u * u + v * v);

    // High-speed outer collar wind peaks at r_ell ~ 0.88 with sharp margin
    let collar = exp(-pow((r_ell - 0.88) / 0.15, 2.0));

    // Crisp core body edge (clean anticyclonic vortex margin)
    let body = smoothstep(1.04, 0.84, r_ell);

    // Calm central eye (inner core radius ~ 0.35)
    let eye = smoothstep(0.35, 0.12, r_ell);

    // Anticyclonic counter-clockwise spiral arms with convective marbled filaments
    let vortex_angle = atan2(v, u);
    let spiral_angle = vortex_angle * 3.0 - vortex_spin_rate * t * 0.45 - r_ell * 7.0;
    let core_filaments = sin(spiral_angle + fbm(p_gas * 18.0) * 1.5) * 0.5 + 0.5;

    // Trailing Kelvin-Helmholtz turbulent wake downstream (eastward/westward shear flow)
    var wake = 0.0;
    if (d_lon > (0.5 * spot_size) && d_lon < (3.6 * spot_size) && abs(d_lat) < (0.9 * spot_size)) {
        let wake_x = (d_lon - 0.5 * spot_size) / (3.1 * spot_size);
        let wake_y = d_lat / (0.9 * spot_size);
        let wake_envelope = (1.0 - wake_x) * exp(-wake_y * wake_y * 3.0);
        let wake_turb = fbm(p_gas * 24.0 + vec3<f32>(t * 0.12, 0.0, 0.0));
        wake = wake_envelope * smoothstep(0.38, 0.82, wake_turb);
    }

    // Dynamic storm coloration based on planet & mass tier
    var spot_core_col = vec3<f32>(0.92, 0.26, 0.10); // Classic Jovian crimson/brick-red
    var collar_col = vec3<f32>(0.84, 0.45, 0.20);    // Lighter orange-tan collar
    var eye_col = vec3<f32>(0.72, 0.16, 0.08);       // Deep darker calm eye

    if (is_saturn) {
        // Saturn: Brilliant white/pearl ammonia convective upwelling (Great White Spot)
        spot_core_col = vec3<f32>(0.98, 0.98, 0.95);
        collar_col    = vec3<f32>(0.92, 0.86, 0.74);
        eye_col       = vec3<f32>(0.85, 0.76, 0.60);
    } else if (mass_jup > 6.0) {
        spot_core_col = vec3<f32>(0.85, 0.18, 0.65);
        collar_col    = vec3<f32>(0.70, 0.30, 0.85);
        eye_col       = vec3<f32>(0.55, 0.10, 0.45);
    } else if (mass_jup > 3.5) {
        spot_core_col = vec3<f32>(0.18, 0.65, 0.95);
        collar_col    = vec3<f32>(0.35, 0.80, 0.98);
        eye_col       = vec3<f32>(0.10, 0.45, 0.75);
    } else if (mass_jup > 1.8) {
        spot_core_col = vec3<f32>(0.15, 0.88, 0.65);
        collar_col    = vec3<f32>(0.30, 0.95, 0.80);
        eye_col       = vec3<f32>(0.08, 0.65, 0.48);
    }

    var grs_col = mix(spot_core_col, collar_col, collar * 0.55);
    grs_col = mix(grs_col, eye_col, eye * 0.75 + core_filaments * 0.20);
    // Spiral filaments texture
    grs_col = grs_col * (0.90 + core_filaments * 0.20);

    // 2. Secondary Anticyclones: Oval BA and White Ovals
    var oval_mask = 0.0;
    if (secondary_count >= 1.0) {
        // Oval BA ("Red Spot Jr.") at -33 deg latitude, trailing GRS
        let ba_lat = spot_lat - 0.18;
        var ba_dlon = lon - (spot_lon + 1.65);
        ba_dlon = ba_dlon - 6.2831853 * round(ba_dlon / 6.2831853);
        let ba_u = ba_dlon / (0.85 * spot_size);
        let ba_v = (lat - ba_lat) / (0.48 * spot_size);
        let ba_r = sqrt(ba_u * ba_u + ba_v * ba_v);
        let ba_body = smoothstep(1.04, 0.60, ba_r);
        if (ba_body > 0.0) {
            oval_mask = max(oval_mask, ba_body * 0.95);
        }

        // Additional White Ovals spaced along South Temperate Belt
        if (secondary_count >= 2.0) {
            for (var oi = 1; oi <= 2; oi = oi + 1) {
                let off_lon = spot_lon - f32(oi) * 1.95;
                var o_dlon = lon - off_lon;
                o_dlon = o_dlon - 6.2831853 * round(o_dlon / 6.2831853);
                let o_u = o_dlon / (0.55 * spot_size);
                let o_v = (lat - (spot_lat - 0.24)) / (0.35 * spot_size);
                let o_r = sqrt(o_u * o_u + o_v * o_v);
                let o_body = smoothstep(1.04, 0.55, o_r);
                oval_mask = max(oval_mask, o_body * 0.85);
            }
        }
    }

    res.spot_mask = body;
    res.collar_mask = collar;
    res.eye_mask = eye;
    res.wake_mask = wake;
    res.oval_mask = oval_mask;
    res.color = grs_col;
    res.is_active = (body > 0.001 || wake > 0.001 || oval_mask > 0.001);
    return res;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let norm = normalize(in.world_normal);
    let surface_dir = normalize(in.world_position.xyz - planet.planet_center_and_radius.xyz);
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

    // Evaluate high-frequency normal in fragment shader with tectonic micro-ridge relief
    let eps_frag = 0.002;
    let micro_lod = 1.0 - smoothstep(0.16, 0.60, length(fwidth(p_surf * 16.0))) * 0.70;
    let r_base = ridge_noise(p_surf * 16.0) * 0.0022 * micro_lod;
    let r_x = ridge_noise(normalize(p_surf + vec3<f32>(eps_frag, 0.0, 0.0)) * 16.0) * 0.0022 * micro_lod;
    let r_y = ridge_noise(normalize(p_surf + vec3<f32>(0.0, eps_frag, 0.0)) * 16.0) * 0.0022 * micro_lod;
    let r_z = ridge_noise(normalize(p_surf + vec3<f32>(0.0, 0.0, eps_frag)) * 16.0) * 0.0022 * micro_lod;

    let h_base = get_vertex_displacement(p_surf) + r_base;
    let h_x = get_vertex_displacement(normalize(p_surf + vec3<f32>(eps_frag, 0.0, 0.0))) + r_x;
    let h_y = get_vertex_displacement(normalize(p_surf + vec3<f32>(0.0, eps_frag, 0.0))) + r_y;
    let h_z = get_vertex_displacement(normalize(p_surf + vec3<f32>(0.0, 0.0, eps_frag))) + r_z;
    
    let grad_spun = vec3<f32>(h_x - h_base, h_y - h_base, h_z - h_base) / eps_frag;
    let grad_tilted = rotate_y(grad_spun, -planet.spin_angle);
    let grad_world = grad_tilted.x * s_tangent_x + grad_tilted.y * s_axis + grad_tilted.z * s_tangent_z;
    
    let bump_scale = select(0.052, 0.085, planet.planet_type == 4u);
    let pixel_normal = normalize(surface_dir - grad_world * bump_scale);
    pbr_input.N = select(pixel_normal, norm, planet.planet_type == 1u || planet.planet_type == 2u);

    
    // 2. Cloud and atmospheric coordinate with zonal trade winds
    let lat = p_tilted.y;
    let zonal_drift = sin(lat * 3.14159 * 2.0) * 0.15;
    let p_cloud = rotate_y(p_tilted, planet.spin_angle * 1.25 + t * 0.04 + zonal_drift);
    let p_cloud_sub = rotate_y(p_tilted, planet.spin_angle * 0.85 - t * 0.03 - zonal_drift * 0.8);
    
    let rock = planet.composition.x;
    let ice = planet.composition.y;
    let metal = planet.composition.z;
    let gas = planet.composition.w;

    let has_volatiles = (ice > 0.002 || planet.climate_and_bio.x > 0.005 || planet.climate_and_bio.y > 0.005 || gas > 0.01);
    let ocean_frac = planet.climate_and_bio.x;
    let ice_frac = planet.climate_and_bio.y;
    let biomass = planet.climate_and_bio.z;
    let cloud_density = max(planet.climate_and_bio.w, gas * 0.5);
    let pressure_bar = planet.atmosphere_params.x;
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
        let jet_stream = (eq_jet + mid_jets) * polar_damp * (t * 0.035);
        var p_gas = rotate_y(p_tilted, planet.spin_angle * 0.85 + jet_stream);

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

        let flow_coarse = fbm(p_zonal_1 + vec3<f32>(t * 0.015, 0.0, -t * 0.01));
        
        // Zonal shear advects eddies horizontally along jet boundaries
        let warp_mid = vec3<f32>((flow_coarse - 0.5) * 1.5, 0.0, (flow_coarse - 0.5) * 1.5);
        let flow_mid = fbm(p_zonal_2 + warp_mid + vec3<f32>(-t * 0.025, 0.0, t * 0.015));

        // High-octave Kelvin-Helmholtz shear billows and curling filaments
        let warp_fine = vec3<f32>((flow_mid - 0.5) * 2.4, (flow_coarse - 0.5) * 0.45, (flow_mid - 0.5) * 2.4);
        let flow_fine = fbm(p_zonal_3 + warp_fine + vec3<f32>(t * 0.04, 0.0, 0.0));

        // Close-up micro-convective cloud texture (LOD derivative filtered)
        let nano_lod = 1.0 - smoothstep(0.015, 0.12, length(fwidth(p_gas * 35.0)));
        let flow_ultra = noise(p_zonal_4 + warp_fine * 2.0) * nano_lod;

        // Zonal frequency: Saturn has finer, denser striations (26 bands); Jupiter has ~15 major belts/zones
        let band_freq = select(
            14.0 + min(mass_jup, 6.0) * 1.4,
            26.0,
            is_saturn
        );

        // Fluid domain warping of latitude creates dynamic meandering jet streams
        let warp_lat = g_lat + (flow_coarse - 0.5) * 0.14 * shear_turb + (flow_mid - 0.5) * 0.05 * shear_turb;

        // Kelvin-Helmholtz shear instabilities along jet boundaries
        let kh_shear = (flow_mid - 0.5) * 0.35 * shear_turb + (flow_fine - 0.5) * 0.14 * shear_turb;

        // Equatorial Festoon plumes: dark loops extending from the tropical belt into equatorial zone
        let in_festoon_lat = smoothstep(0.20, 0.06, g_lat) * smoothstep(-0.02, 0.06, g_lat);
        let festoon_flow = sin(p_gas.x * 12.0 + flow_mid * 3.2) * cos(p_gas.z * 12.0);
        let festoon_mask = in_festoon_lat * smoothstep(0.36, 0.72, flow_coarse + festoon_flow * 0.36);

        // Crisp, anti-aliased latitudinal banding (sharper step transitions):
        let s_prim = sin(warp_lat * band_freq + kh_shear);
        let b_primary = smoothstep(-0.48, 0.48, s_prim);

        // Mid-frequency harmonic for secondary belts and subtle sub-divisions (NEBn/NEBs, SEBn/SEBs)
        let b_harmonic = sin(warp_lat * band_freq * 2.0 + flow_mid * 1.4) * 0.22;

        // Anti-aliased micro-striations for close-up fidelity (derivative-filtered)
        let b_micro = sin(warp_lat * band_freq * 4.2 + flow_fine * 0.8) * 0.09 * nano_lod;

        // Fine convective eddy billows along shear margins
        let b_filament = (flow_ultra - 0.5) * 0.10 * nano_lod;

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
    else if (planet.planet_type == 2u) {
        let jet_stream = sin(lat * 10.0) * (t * 0.08);
        let p_ice_gas = rotate_y(p_tilted, planet.spin_angle + jet_stream);
        let swirl = fbm(p_ice_gas * 4.5 + vec3<f32>(t * 0.02, 0.0, t * 0.01));
        let band = sin(lat * 8.0 + swirl * 1.2) * 0.5 + 0.5;
        
        let deep_cyan = vec3<f32>(0.08, 0.42, 0.75);
        let bright_azure = vec3<f32>(0.32, 0.72, 0.96);
        let methane_veil = mix(deep_cyan, bright_azure, band);
        
        // High-altitude cirrus clouds with fast prograde drift
        let cirrus_coord = rotate_y(p_tilted, planet.spin_angle * 1.35 + jet_stream * 1.5);
        let cirrus = fbm(cirrus_coord * 14.0);
        let white_clouds = smoothstep(0.65, 0.85, cirrus);
        
        var base_ice = mix(methane_veil, vec3<f32>(0.92, 0.96, 1.0), white_clouds * 0.55);

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
    // =========================================================================
    // 6. Super-Earth (Dedicated Mega-Terrestrial World: Vast Oceans, Continents, Storms)
    // =========================================================================
    else if (planet.planet_type == 6u) {
        // High surface gravity and massive lithosphere produce sprawling mega-continents,
        // deep abyssal oceans, towering folded cordilleras, and intense cyclonic weather fronts.
        let plate_drift = vec3<f32>(
            sin(drift_phase * 6.283 + p_surf.z * 2.6) * 0.28,
            cos(drift_phase * 3.14159 + p_surf.x * 2.2) * 0.12,
            sin(drift_phase * 4.712 + p_surf.y * 2.4) * 0.28
        );
        let p_tectonic = p_surf + plate_drift;
        let elev = eroded_continent_fbm(p_tectonic * 3.4);
        let ridge = ridge_noise(p_tectonic * 7.2);

        let land_weight = smoothstep(0.28, 0.50, elev);
        let oceanic_basin = 1.0 - land_weight;
        let mid_ocean_ridge = ridge_noise(p_tectonic * 7.5 + vec3<f32>(2.1, 1.4, 0.9)) * 0.11;
        let axial_rift = (1.0 - ridge_noise(p_tectonic * 15.0 + vec3<f32>(2.1, 1.4, 0.9))) * 0.03;
        let trench_zone = smoothstep(0.05, 0.0, abs(elev - 0.38) - 0.02);
        let trench_depth = trench_zone * 0.10;

        let has_oceans = has_volatiles && ocean_frac > 0.01 && temp <= 380.0;
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

        let ice_edge_noise = fbm(p_surf * 10.0) * 0.06;
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
            water_mask = clamp((sea_level - combined_elev) / 0.03, 0.0, 1.0);
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
        
        // Massive Multi-Scale Atmospheric Cloud Circulation & Runaway Steam Deck
        if (has_volatiles || gas > 0.05 || ocean_frac > 0.005) {
            let boil_steam_factor = clamp((temp - 345.0) / 35.0, 0.0, 1.0);
            let effective_cloud_density = mix(cloud_density, 1.0, boil_steam_factor * select(0.0, 1.0, has_volatiles || ocean_frac > 0.005));

            let p_cloud_rot = rotate_y(p_tilted, planet.spin_angle * 1.12 + zonal_drift);
            let cloud_main = fbm(p_cloud_rot * 4.6);
            let cloud_spirals = fbm(p_cloud_rot * 9.5 + vec3<f32>(0.0, t * 0.02, 0.0));
            let storm_bands = sin(lat * 10.0 + cloud_main * 2.2) * 0.5 + 0.5;
            let super_clouds = cloud_main * 0.60 + cloud_spirals * 0.25 + storm_bands * 0.15;
            
            // Soft cloud drop shadows on land and ocean surfaces
            let shadow_rot = rotate_y(p_tilted, planet.spin_angle * 1.12 + zonal_drift) + vec3<f32>(0.025, 0.015, 0.025);
            let shadow_val = fbm(shadow_rot * 4.6);
            if (shadow_val > 0.50) {
                color = color * (1.0 - (shadow_val - 0.50) * 0.45);
            }
            
            let substellar_cloud_boost = select(0.0, smoothstep(-0.10, 0.65, substellar_cos) * 0.08, is_tidally_locked);
            let cloud_thresh = 0.46 - effective_cloud_density * 0.22 - substellar_cloud_boost;
            if (super_clouds > cloud_thresh) {
                let cloud_alpha = clamp(
                    (super_clouds - cloud_thresh) * 2.8 * clamp(effective_cloud_density * 1.2, 0.2, 1.0),
                    0.0,
                    mix(0.92, 0.98, boil_steam_factor)
                );
                cloud_cover_alpha = max(cloud_cover_alpha, cloud_alpha);
                let cloud_white = mix(vec3<f32>(0.96, 0.98, 1.0), vec3<f32>(1.0, 1.0, 1.0), boil_steam_factor);
                color = mix(color, cloud_white, cloud_alpha);
                pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.92, cloud_alpha);
                pbr_input.N = normalize(mix(pbr_input.N, surface_dir, cloud_alpha * 0.65));
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
    // =========================================================================
    // =========================================================================
    // 4. Minor Bodies (Asteroids, Comets, Planetesimals)
    // =========================================================================
    // 4. Minor Bodies (Asteroids, Comets, Planetesimals)
    // =========================================================================
    else if (planet.planet_type == 4u) {
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
    // 3. Terrestrial Rocky / Ocean / Biosphere Planet / Protoplanet
    // =========================================================================
    else if (planet.planet_type == 3u) {
        let plate_drift = vec3<f32>(
            sin(drift_phase * 6.283 + p_surf.z * 2.6) * 0.28,
            cos(drift_phase * 3.14159 + p_surf.x * 2.2) * 0.12,
            sin(drift_phase * 4.712 + p_surf.y * 2.4) * 0.28
        );
        let p_tectonic = p_surf + plate_drift;

        // Multi-scale elevation: eroded continents → hills → ridges → micro-roughness
        let continent_mask = eroded_continent_fbm(p_tectonic * 1.85);
        let regional_hills = fbm(p_tectonic * 5.5) * 0.32;
        let mountain_ridges = ridge_noise(p_tectonic * 8.0) * 0.20;
        let micro_detail = fbm(p_tectonic * 22.0) * 0.10;

        // Supercontinent Wilson-cycle clustering
        let super_axis = normalize(vec3<f32>(0.65, 0.25, 0.70));
        let cluster_dipole = dot(p_surf, super_axis);
        let cluster_bias = cluster_dipole * (aggregation - 0.20) * 0.14;

        let sea_level = clamp(0.18 + ocean_frac * 0.40, 0.18, 0.76);
        let primordial_shelf_elev = 0.46;
        let has_oceans = has_volatiles && ocean_frac > 0.01 && temp <= 380.0;

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

        let elev = select(
            continental_relief + oceanic_basin * (mid_ocean_ridge - axial_rift) - trench_depth,
            mix(bathymetry_elev, continental_relief, land_weight),
            has_oceans
        );

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
        // B. Venusian runaway greenhouse deck
        else if (temp >= 380.0 && (gas > 0.05 || cloud_density > 0.5 || pressure_bar > 5.0)) {
            let super_rot = rotate_y(p_tilted, planet.spin_angle * 3.5);
            let clouds = fbm(super_rot * 4.5 + vec3<f32>(t * 0.05, 0.0, t * 0.05));
            let band = sin(lat * 8.0 + clouds * 1.8) * 0.5 + 0.5;
            let sulfur_deck = mix(vec3<f32>(0.78, 0.70, 0.42), vec3<f32>(0.90, 0.84, 0.62), band);
            color = sulfur_deck * (0.88 + clouds * 0.18);
            pbr_input.material.perceptual_roughness = 0.55;
        }
        // C. Snowball / glacial
        else if (has_volatiles && (ice_frac >= 0.60 || (temp < 255.0 && ocean_frac > 0.05))) {
            let frost = fbm(p_surf * 10.0);
            let glaciers = mix(vec3<f32>(0.72, 0.80, 0.88), vec3<f32>(0.40, 0.58, 0.72), elev);
            let pack_ice = vec3<f32>(0.90, 0.93, 0.97);
            color = mix(glaciers, pack_ice, smoothstep(0.35, 0.72, frost));
            pbr_input.material.perceptual_roughness = 0.20;
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

            let ice_edge_noise = fbm(p_surf * 10.0) * 0.06;
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
                water_mask = clamp((sea_level - elev) / 0.025, 0.0, 1.0);
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

                    var land: vec3<f32>;
                    if (rel_elev > treeline) {
                        land = mix(rock, alpine, clamp((rel_elev - treeline) * 3.5, 0.0, 1.0));
                    } else if (rel_elev > 0.08) {
                        let veg = mix(meadow, lush, moisture);
                        land = mix(scrub, veg, clamp(effective_bio * 1.3, 0.0, 1.0));
                    } else {
                        // Coastal plain
                        let coastal = mix(lush, meadow, moisture);
                        land = mix(vec3<f32>(0.42, 0.36, 0.24), coastal, clamp(effective_bio * 1.4, 0.0, 1.0));
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

            // Dual-layer clouds + soft ground shadow + Runaway Steam Deck
            let boil_steam_factor = clamp((temp - 345.0) / 35.0, 0.0, 1.0);
            let effective_cloud_density = mix(cloud_density, 1.0, boil_steam_factor * select(0.0, 1.0, has_volatiles || ocean_frac > 0.005));

            if (effective_cloud_density > 0.02) {
                let c_main = fbm(p_cloud * 5.2);
                let c_sub = fbm(p_cloud_sub * 9.5);
                let total_clouds = c_main * 0.65 + c_sub * 0.35;

                let shadow_coord = rotate_y(p_tilted, planet.spin_angle * 1.25 + t * 0.04 + zonal_drift)
                    + vec3<f32>(0.03, 0.02, 0.03);
                let shadow_val = fbm(shadow_coord * 5.2);
                if (shadow_val > 0.55 && elev >= sea_level) {
                    color = color * (1.0 - (shadow_val - 0.55) * 0.45);
                }

                let substellar_cloud_boost = select(0.0, smoothstep(-0.10, 0.65, substellar_cos) * 0.08, is_tidally_locked);
                let cloud_thresh = 0.55 - effective_cloud_density * 0.22 - substellar_cloud_boost;
                if (total_clouds > cloud_thresh) {
                    let cloud_alpha = clamp(
                        (total_clouds - cloud_thresh) * 2.8 * clamp(effective_cloud_density * 1.2, 0.2, 1.0),
                        0.0,
                        mix(0.78, 0.98, boil_steam_factor)
                    );
                    cloud_cover_alpha = max(cloud_cover_alpha, cloud_alpha);
                    let steam_white = mix(vec3<f32>(0.93, 0.95, 0.97), vec3<f32>(0.99, 1.0, 1.0), boil_steam_factor);
                    color = mix(color, steam_white, cloud_alpha);
                    pbr_input.material.perceptual_roughness = mix(
                        pbr_input.material.perceptual_roughness,
                        0.90,
                        cloud_alpha
                    );
                    pbr_input.N = normalize(mix(pbr_input.N, surface_dir, cloud_alpha * 0.65));
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
        color = crater_res.color;
        pbr_input.material.perceptual_roughness = crater_res.roughness;
        pbr_input.material.metallic = crater_res.metallic;
        pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb + crater_res.emissive, 1.0);
        if (dot(crater_res.normal_delta, crater_res.normal_delta) > 1e-6) {
            let c_norm_tilted = rotate_y(crater_res.normal_delta, -planet.spin_angle);
            let c_norm_world = c_norm_tilted.x * s_tangent_x + c_norm_tilted.y * s_axis + c_norm_tilted.z * s_tangent_z;
            pbr_input.N = normalize(pbr_input.N + c_norm_world * (1.0 - cloud_cover_alpha * 0.80));
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
        let is_cryotectonic = (planet.composition.y > 0.32 || planet.climate_and_bio.y > 0.35) && (planet.planet_type == 4u || planet.planet_type == 3u || planet.planet_type == 8u);
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

    pbr_input.material.base_color = vec4<f32>(color, 1.0);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    
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

        // 1. Ring Shadow Calculation: Equatorial ring plane intersects ray from surface towards star
        var ring_shadow = 1.0;
        if (planet.ring_shadow_params.w > 0.5) {
            let r_inner = planet.ring_shadow_params.x;
            let r_outer = planet.ring_shadow_params.y;
            let opt_depth = planet.ring_shadow_params.z;

            let L_dot_S = dot(star_dir, s_axis);
            let P_dot_S = dot(pbr_input.N, s_axis);

            if (abs(L_dot_S) > 1e-4) {
                let t_ring = -P_dot_S / L_dot_S;
                let N_dot_L = dot(pbr_input.N, star_dir);
                if (t_ring > 0.0 && N_dot_L > 0.0) {
                    let p_int = pbr_input.N + t_ring * star_dir;
                    let r_int = length(p_int);

                    if (r_int >= r_inner && r_int <= r_outer) {
                        let u = (r_int - r_inner) / (r_outer - r_inner);

                        // Ring density profile matching planetary_rings.wgsl
                        var ring_density = 0.0;
                        if (u < 0.22) {
                            // C Ring: faint translucent crepe ring
                            ring_density = mix(0.10, 0.35, u / 0.22);
                        } else if (u < 0.65) {
                            // B Ring: dense opaque main ring
                            ring_density = 0.95;
                        } else if (u < 0.72) {
                            // Cassini Division: prominent clear gap transmitting sunlight!
                            let gap_t = (u - 0.65) / (0.72 - 0.65);
                            ring_density = (1.0 - sin(gap_t * 3.14159265)) * 0.12;
                        } else if (u < 0.96) {
                            // A Ring
                            var d_a = 0.75;
                            if (u >= 0.855 && u <= 0.875) {
                                // Encke Gap
                                let encke_t = (u - 0.855) / (0.875 - 0.855);
                                d_a *= (1.0 - sin(encke_t * 3.14159265) * 0.88);
                            }
                            ring_density = d_a;
                        } else {
                            // F Ring / Outer diffuse edge
                            ring_density = (1.0 - (u - 0.96) / 0.04) * 0.35;
                        }

                        let edge_feather = smoothstep(r_inner, r_inner + 0.03, r_int) * (1.0 - smoothstep(r_outer - 0.03, r_outer, r_int));
                        let shadow_atten = clamp(ring_density * opt_depth * edge_feather, 0.0, 0.96);
                        ring_shadow = 1.0 - shadow_atten;
                    }
                }
            }
        }

        // 2. Moon Solar Eclipse Calculation: Transiting moons cast umbra and penumbra spots
        var eclipse_light = 1.0;
        let geom_N = normalize(in.world_normal);
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
        
        // Measure starlight illuminance factor relative to surface albedo, modulated by shadows
        let raw_starlight = lit_lum / base_lum;
        let starlight_factor = raw_starlight * total_shadow;
        
        // Tone-map diffuse lighting with an energy-conserving soft-knee curve
        let diffuse_intensity = clamp(starlight_factor / (starlight_factor + 0.95) * 1.05, 0.0, 1.0);
        let diffuse_lit = base_col * diffuse_intensity;
        
        // Physically accurate neutral specular highlight for dielectrics (rock, ice, dust)
        // Avoids chromatic inversion (which subtracted high-red diffuse from starlight and turned orange/warm planet highlights purple/blue)
        let H_vec = normalize(star_dir + pbr_input.V);
        let NdotH = max(dot(pbr_input.N, H_vec), 0.0);
        let star_facing = max(dot(pbr_input.N, star_dir), 0.0);
        let roughness = clamp(pbr_input.material.perceptual_roughness, 0.04, 1.0);
        let shininess = mix(128.0, 4.0, roughness * roughness);
        let spec_power = pow(NdotH, shininess) * star_facing;
        let spec_color = mix(vec3<f32>(1.0, 1.0, 1.0), base_col, pbr_input.material.metallic);
        let specular_glint = spec_color * spec_power * (1.0 - roughness * 0.65) * 0.45 * total_shadow;

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
                * total_shadow;
        }
        
        let balanced_lit = diffuse_lit + specular_glint + cox_munk_sunglint;
        
        let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0);
        let fresnel = pow(1.0 - NdotV, 2.8);
        let ambient_boost = base_col * 0.08;
        let NdotL = dot(pbr_input.N, star_dir); // Physical solar zenith angle
        
        // Phase 2: High-Fidelity Volumetric Raymarched Atmospheric Scattering (with Chappuis Ozone & Twilight Grazing)
        var atmospheric_haze = vec3<f32>(0.0);
        var surface_attenuation = vec3<f32>(1.0);
        
        if (planet.planet_type != 4u && (pressure_bar > 0.005 || cloud_density > 0.05 || planet.planet_type == 1u || planet.planet_type == 2u || planet.planet_type == 6u)) {
            let center = planet.planet_center_and_radius.xyz;
            let r_planet = max(planet.planet_center_and_radius.w, 0.001);
            
            let h_r = max(planet.atmosphere_params.y * r_planet, 0.002);
            let h_m = h_r * 0.4;
            let r_atmo = r_planet + h_r * 4.5;
            
            let ray_origin = view.world_position.xyz;
            let ray_dir = -pbr_input.V;
            
            // Ray intersects atmosphere shell
            let oc = ray_origin - center;
            let b = dot(oc, ray_dir);
            let c_atmo = dot(oc, oc) - r_atmo * r_atmo;
            let disc_atmo = b * b - c_atmo;
            
            if (disc_atmo > 0.0) {
                let t_enter_atmo = max(-b - sqrt(disc_atmo), 0.0);
                let t_surface = length(in.world_position.xyz - ray_origin);
                
                let t_start = t_enter_atmo;
                let t_end = t_surface;
                
                if (t_end > t_start) {
                    let step_count = 5;
                    let step_size = (t_end - t_start) / f32(step_count);
                    
                    let beta_r = planet.scattering_params.rgb;
                    let beta_m = beta_r * clamp(cloud_density * 2.0 + pressure_bar * 0.15, 0.1, 1.5);
                    // Chappuis Ozone (O3) absorption band in green/yellow (550-600nm) for oxygen/ocean worlds
                    let has_ozone = select(0.0, 1.0, (planet.planet_type == 3u || planet.planet_type == 6u) && ocean_frac > 0.02 && temp < 360.0);
                    let beta_o3 = vec3<f32>(0.65, 1.88, 0.08) * 0.42 * has_ozone;
                    let g = clamp(planet.scattering_params.w, 0.50, 0.95);
                    
                    let cos_theta = dot(ray_dir, star_dir);
                    let cos2_theta = cos_theta * cos_theta;
                    let p_rayleigh = (3.0 / (16.0 * 3.1415926)) * (1.0 + cos2_theta);
                    
                    let g2 = g * g;
                    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
                    let p_mie = (3.0 * (1.0 - g2) / (8.0 * 3.1415926 * (2.0 + g2))) * (1.0 + cos2_theta) / (denom * sqrt(max(denom, 1e-4)));
                    
                    var opt_depth_r = 0.0;
                    var opt_depth_m = 0.0;
                    var opt_depth_o3 = 0.0;
                    var in_scatter = vec3<f32>(0.0);
                    
                    for (var i = 0; i < step_count; i = i + 1) {
                        let t_sample = t_start + (f32(i) + 0.5) * step_size;
                        let sample_pos = ray_origin + ray_dir * t_sample;
                        let p_rel = sample_pos - center;
                        let sample_r = length(p_rel);
                        let alt = max(sample_r - r_planet, 0.0);
                        
                        let d_r = exp(-alt / h_r) * step_size;
                        let d_m = exp(-alt / h_m) * step_size;
                        let o3_profile = exp(-pow((alt - h_r * 1.15) / (h_r * 0.75), 2.0));
                        let d_o3 = o3_profile * step_size;
                        opt_depth_r += d_r;
                        opt_depth_m += d_m;
                        opt_depth_o3 += d_o3;
                        
                        // Sunlight optical depth from sample to outer atmosphere with Chapman grazing enhancement
                        let sb = dot(p_rel, star_dir);
                        let sc = dot(p_rel, p_rel) - r_atmo * r_atmo;
                        let sdisc = sb * sb - sc;
                        var sun_path = 0.0;
                        if (sdisc > 0.0) { 
                            sun_path = max(-sb + sqrt(sdisc), 0.0); 
                        }
                        let mu_s = sb / max(sample_r, 0.0001);
                        let twilight_gate = smoothstep(-0.26, 0.08, mu_s);
                        let chapman_factor = 1.0 / max(mu_s + 0.18, 0.06);
                        
                        let s_depth_r = exp(-alt / h_r) * sun_path * min(chapman_factor * 0.45, 3.2);
                        let s_depth_m = exp(-alt / h_m) * sun_path;
                        let s_depth_o3 = o3_profile * sun_path * min(chapman_factor * 0.65, 4.0);
                        
                        let tau = beta_r * (opt_depth_r + s_depth_r)
                            + beta_m * (opt_depth_m + s_depth_m)
                            + beta_o3 * (opt_depth_o3 + s_depth_o3);
                        let attenuation = exp(-tau) * twilight_gate;
                        
                        in_scatter += (beta_r * d_r * p_rayleigh + beta_m * d_m * p_mie) * attenuation;
                    }
                    
                    let sun_visible = mix(0.15, 1.0, total_shadow);
                    atmospheric_haze = in_scatter * sun_visible * 6.2;
                    surface_attenuation = exp(-(beta_r * opt_depth_r + beta_m * opt_depth_m + beta_o3 * opt_depth_o3 * 0.5));
                }
            }
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
    
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    // 3D Planetary Internal Cutaway Mode: render internal geological cross-section in +X / +Z quadrant
    if (planet.cutaway_params.x > 0.5 && p_tilted.x > 0.0 && p_tilted.z > 0.0) {
        let inner_core_ratio = planet.cutaway_params.y;
        let core_ratio = planet.cutaway_params.z;
        let mantle_ratio = planet.cutaway_params.w;

        // View direction in tilted coordinate frame
        let v_world = -pbr_input.V;
        let v_tilted = vec3<f32>(dot(v_world, s_tangent_x), dot(v_world, s_axis), dot(v_world, s_tangent_z));

        // Intersect ray with the two orthogonal cut planes:
        // Plane 1: Z = 0 (valid for X >= 0)
        // Plane 2: X = 0 (valid for Z >= 0)
        var hit_r = 1.0;
        var hit_dist = 1e6;
        var hit_pos = p_tilted;

        if (abs(v_tilted.z) > 1e-4) {
            let t_z = -p_tilted.z / v_tilted.z;
            if (t_z > 0.0) {
                let p_hit = p_tilted + t_z * v_tilted;
                if (p_hit.x >= -0.01 && length(p_hit) <= 1.02) {
                    hit_dist = t_z;
                    hit_r = length(p_hit);
                    hit_pos = p_hit;
                }
            }
        }

        if (abs(v_tilted.x) > 1e-4) {
            let t_x = -p_tilted.x / v_tilted.x;
            if (t_x > 0.0 && t_x < hit_dist) {
                let p_hit = p_tilted + t_x * v_tilted;
                if (p_hit.z >= -0.01 && length(p_hit) <= 1.02) {
                    hit_dist = t_x;
                    hit_r = length(p_hit);
                    hit_pos = p_hit;
                }
            }
        }

        // Ray-sphere intersection with the 3D incandescent inner core sphere
        var is_inner_core_sphere = false;
        var n_ic = vec3<f32>(0.0, 1.0, 0.0);
        let b_ic = dot(p_tilted, v_tilted);
        let c_ic = dot(p_tilted, p_tilted) - inner_core_ratio * inner_core_ratio;
        let disc_ic = b_ic * b_ic - c_ic;
        if (disc_ic > 0.0) {
            let t_ic = -b_ic - sqrt(disc_ic);
            if (t_ic > 0.0 && t_ic < hit_dist) {
                let p_ic = p_tilted + t_ic * v_tilted;
                if (p_ic.x >= -0.005 && p_ic.z >= -0.005) {
                    hit_dist = t_ic;
                    hit_r = inner_core_ratio * 0.96;
                    hit_pos = p_ic;
                    is_inner_core_sphere = true;
                    n_ic = normalize(p_ic);
                }
            }
        }

        // Color based on physical internal geological strata:
        var cut_col = vec3<f32>(0.92, 0.42, 0.15); // Silicate Mantle
        var cut_glow = vec3<f32>(0.0);

        let r_trans = core_ratio + (mantle_ratio - core_ratio) * 0.70;

        if (hit_r < inner_core_ratio) {
            // 1. Solid Inner Core: crystalline solid Fe-Ni (T ~ 5,500 K, golden incandescence)
            let rad_grain = sin(atan2(hit_pos.y, hit_pos.x + hit_pos.z) * 12.0) * 0.06 + 0.94;
            cut_col = vec3<f32>(1.0, 0.95, 0.82) * rad_grain;
            if (is_inner_core_sphere) {
                let diff_ic = max(dot(n_ic, vec3<f32>(0.5, 0.7, 0.3)), 0.0);
                cut_col = cut_col * (0.65 + 0.35 * diff_ic);
            }
            cut_glow = vec3<f32>(1.5, 1.25, 0.65) * 2.2;
        } else if (hit_r < core_ratio) {
            // 2. Liquid Outer Core: convective molten Fe-Ni geodynamo (T ~ 4,500 K, amber glow)
            let core_frac = (hit_r - inner_core_ratio) / max(core_ratio - inner_core_ratio, 0.005);
            let swirl = sin(hit_r * 32.0 - planet.time * 1.5 + hit_pos.y * 10.0) * 0.12;
            let base_molten = mix(vec3<f32>(1.0, 0.68, 0.15), vec3<f32>(0.82, 0.28, 0.04), clamp(core_frac + swirl, 0.0, 1.0));
            cut_col = base_molten;
            cut_glow = vec3<f32>(1.1, 0.45, 0.08) * 1.2;
        } else if (hit_r < r_trans) {
            // 3. Lower Mantle (Mesosphere): dense bridgmanite / ferropericlase (T ~ 2,800 K)
            let lower_frac = (hit_r - core_ratio) / max(r_trans - core_ratio, 0.005);
            let base_lower = mix(vec3<f32>(0.78, 0.28, 0.10), vec3<f32>(0.65, 0.35, 0.18), lower_frac);
            cut_col = base_lower;
            cut_glow = vec3<f32>(0.3, 0.08, 0.0) * (1.0 - lower_frac) * 0.6;

            // Sunken Theia LLSVP Plumes at Core-Mantle Boundary (Gutenberg Discontinuity)
            let cmb_dist = hit_r - core_ratio;
            let blob_pat = sin(atan2(hit_pos.y, hit_pos.x) * 5.0 + hit_pos.z * 15.0);
            if (cmb_dist < 0.08 && blob_pat > 0.05) {
                let blob_w = smoothstep(0.08, 0.0, cmb_dist) * smoothstep(0.05, 0.6, blob_pat);
                cut_col = mix(cut_col, vec3<f32>(0.92, 0.12, 0.18), blob_w * 0.85);
                cut_glow = mix(cut_glow, vec3<f32>(0.8, 0.1, 0.1), blob_w * 0.7);
            }
        } else if (hit_r < mantle_ratio) {
            // 4. Upper Mantle (Asthenosphere): ductile peridotite / olivine rock (T ~ 1,600 K)
            let upper_frac = (hit_r - r_trans) / max(mantle_ratio - r_trans, 0.005);
            cut_col = mix(vec3<f32>(0.65, 0.35, 0.18), vec3<f32>(0.50, 0.40, 0.22), upper_frac);
            cut_glow = vec3<f32>(0.0);
        } else {
            // 5. Rigid Lithospheric Crust & Hydrosphere (Stratified rock layers)
            let crust_depth = (hit_r - mantle_ratio) / max(1.0 - mantle_ratio, 0.005);
            let rock_band = sin(crust_depth * 25.0) * 0.5 + 0.5;
            cut_col = mix(vec3<f32>(0.32, 0.30, 0.28), vec3<f32>(0.58, 0.50, 0.42), rock_band);
            cut_glow = vec3<f32>(0.0);

            // Hydrosphere: surface liquid oceans if present
            if (planet.climate_and_bio.x > 0.05 && hit_r > 0.992) {
                cut_col = vec3<f32>(0.08, 0.38, 0.75);
                cut_glow = vec3<f32>(0.02, 0.12, 0.25);
            }
        }

        // Thin glowing holographic boundary lines between geological strata
        let d_inner = abs(hit_r - inner_core_ratio);
        let d_core = abs(hit_r - core_ratio);
        let d_trans = abs(hit_r - r_trans);
        let d_mantle = abs(hit_r - mantle_ratio);
        let d_surface = abs(hit_r - 1.0);

        if (d_inner < 0.010) {
            // Lehmann Discontinuity (Inner Core Boundary): brilliant gold
            cut_col = vec3<f32>(1.0, 0.92, 0.45);
            cut_glow = vec3<f32>(1.8, 1.5, 0.6) * 1.8;
        } else if (d_core < 0.009) {
            // Gutenberg Discontinuity (Core-Mantle Boundary): fiery amber
            cut_col = vec3<f32>(1.0, 0.55, 0.10);
            cut_glow = vec3<f32>(1.5, 0.6, 0.1) * 1.5;
        } else if (d_trans < 0.006) {
            // Mantle Transition Zone: subtle division
            cut_col = vec3<f32>(0.85, 0.65, 0.30);
            cut_glow = vec3<f32>(0.5, 0.3, 0.1) * 0.8;
        } else if (d_mantle < 0.008) {
            // Mohorovičić Discontinuity (Moho): vibrant cyan
            cut_col = vec3<f32>(0.25, 0.92, 1.0);
            cut_glow = vec3<f32>(0.2, 0.8, 1.0) * 1.6;
        } else if (d_surface < 0.007) {
            // Exterior Crust Perimeter: cyan highlight
            cut_col = vec3<f32>(0.30, 0.95, 1.0);
            cut_glow = vec3<f32>(0.2, 0.8, 1.0) * 1.2;
        }

        out.color = vec4<f32>(cut_col + cut_glow, 1.0);
    }

    return out;
}
