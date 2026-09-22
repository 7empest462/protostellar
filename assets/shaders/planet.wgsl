#import bevy_pbr::{
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
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
    spin_rate: f32,
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
    eclipse_moons_pos: array<vec4<f32>, 2>, // xyz = relative moon pos in planet radii, w = moon radius ratio
    eclipse_moons_data: array<vec4<f32>, 2>, // x = active flag (1.0 or 0.0), y = penumbra softness, z = shadow depth, w = reserved
    geological_params: vec4<f32>, // x: age_gyr, y: drift_phase, z: ocean_oxidation, w: vegetation_expansion
    aurora_params: vec4<f32>, // x: oval_colatitude_rad, y: oval_width_rad, z: auroral_intensity, w: geomagnetic_kp_index
    storm_features: vec4<f32>, // x: hex_amplitude, y: hex_wavenumber (e.g. 6.0), z: great_spot_size, w: great_spot_lat_rad
    storm_dynamics: vec4<f32>, // x: great_spot_lon_rad, y: vortex_spin_rate, z: secondary_oval_count, w: zonal_shear_turbulence
};

@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var<uniform> planet: PlanetExtension;

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
// Distinct astrophysical procedural shading for each stellar classification:
// 0: Main Sequence Yellow Dwarf (Sun)
// 1: Red Dwarf (TRAPPIST-1)
// 2: Brown Dwarf
// 3: Red Giant & Supergiant (Betelgeuse)
// 4: Blue Hyper Giant & Supergiant (O-type / Pop-III)
// 5: Neutron Star
// 6: Pulsar
// 7: Magnetar
// 8: White Dwarf
// 9: Protostar
// 10: Wolf-Rayet Star
fn render_stellar_photosphere(
    p_surf: vec3<f32>,
    norm: vec3<f32>,
    view_dir: vec3<f32>,
    t: f32,
) -> vec3<f32> {
    let subtype = u32(planet.composition.x + 0.5);
    let cell_scale = max(planet.composition.y, 4.0);
    let flare_int = planet.composition.z;
    let pulse_freq = planet.composition.w;
    let seed_col = planet.color_seed.rgb;
    let NdotV = max(dot(norm, view_dir), 0.0);
    
    var final_col = seed_col;
    
    // =========================================================================
    // 0. Main Sequence Yellow Dwarf (The Sun)
    // =========================================================================
    if (subtype == 0u) {
        // Crisp solar granulation
        let v = voronoi3(p_surf * cell_scale, t * 0.30);
        let lane = smoothstep(0.04, 0.30, v.y - v.x);
        let dark_lane = vec3<f32>(0.55, 0.38, 0.12);
        let bright_granule = vec3<f32>(1.12, 0.96, 0.65);
        var gran_col = mix(dark_lane, bright_granule, lane);
        
        // Sunspots: dark umbra + striated penumbra + bright faculae plages
        let spot_noise = fbm(p_surf * 5.0);
        if (spot_noise > 0.60) {
            let umbra = smoothstep(0.68, 0.74, spot_noise);
            let penumbra = smoothstep(0.60, 0.68, spot_noise);
            let umbra_col = vec3<f32>(0.08, 0.05, 0.02);
            let penumbra_col = vec3<f32>(0.42, 0.30, 0.12);
            gran_col = mix(gran_col, penumbra_col, penumbra);
            gran_col = mix(gran_col, umbra_col, umbra);
        } else if (spot_noise > 0.52) {
            // Bright magnetic faculae / plages
            let faculae = (spot_noise - 0.52) / 0.08;
            gran_col += vec3<f32>(0.35, 0.30, 0.15) * faculae;
        }
        
        // Incandescent magnetic coronal loops & reconnection prominence arches
        let loop_coord = p_surf * 14.0 + vec3<f32>(t * 0.15, sin(t * 0.35 + p_surf.x * 4.0), t * 0.10);
        let loop_flux = sin(loop_coord.x * 2.5 + sin(loop_coord.y * 3.0)) * cos(loop_coord.z * 2.2);
        if (abs(loop_flux) < 0.15 && spot_noise > 0.44) {
            let loop_glow = (1.0 - abs(loop_flux) / 0.15) * (spot_noise - 0.44) * 3.5;
            let loop_col = vec3<f32>(1.6, 0.85, 0.25) * (1.0 + flare_int * 2.5);
            gran_col += loop_col * loop_glow;
        }
        
        // Quadratic Eddington solar limb darkening
        let limb_u = 0.60;
        let limb_v = 0.20;
        let limb_dark = 1.0 - limb_u * (1.0 - NdotV) - limb_v * (1.0 - sqrt(NdotV));
        gran_col *= clamp(limb_dark, 0.15, 1.0);
        
        // Chromospheric golden fringe at grazing angles
        let fringe = pow(1.0 - NdotV, 4.0) * vec3<f32>(1.2, 0.8, 0.2) * 1.5;
        final_col = gran_col * 2.8 + fringe;
    }
    // =========================================================================
    // 1. Red Dwarf (M-Star / TRAPPIST-1 / Proxima)
    // =========================================================================
    else if (subtype == 1u) {
        // Fully-convective churning: large turbulent cells
        let v = voronoi3(p_surf * cell_scale, t * 0.50);
        let lane = smoothstep(0.04, 0.32, v.y - v.x);
        let dark_lane = vec3<f32>(0.32, 0.05, 0.01);
        let cell_body = vec3<f32>(0.96, 0.30, 0.06);
        let cell_core = vec3<f32>(1.15, 0.55, 0.12);
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
    // 9. Protostar
    // =========================================================================
    else if (subtype == 9u) {
        // Fiery amber base with dark circumstellar dust veins across the entire star
        let v = voronoi3(p_surf * cell_scale, t * 0.40);
        let lane = smoothstep(0.04, 0.30, v.y - v.x);
        let amber_base = mix(vec3<f32>(0.45, 0.15, 0.04), vec3<f32>(1.05, 0.52, 0.14), lane);
        
        // Dark filamentary dust veins crisscrossing the surface
        let dust_vein = ridge_noise(p_surf * 6.5 + vec3<f32>(0.0, t * 0.06, 0.0));
        var proto_col = amber_base;
        if (dust_vein > 0.70) {
            proto_col *= (1.0 - (dust_vein - 0.70) * 2.2);
        }
        
        let limb = pow(NdotV, 0.55);
        final_col = proto_col * (limb * 0.8 + 0.2) * 2.5;
    }
    // =========================================================================
    // 10. Wolf-Rayet Star
    // =========================================================================
    else if (subtype == 10u) {
        // Neon magenta-cyan with expanding clumpy plasma wind knots
        let wind_flow = fbm(p_surf * cell_scale + vec3<f32>(t * 0.8, t * 0.4, 0.0));
        let wind_wave = sin(norm.y * 16.0 + wind_flow * 4.0) * 0.5 + 0.5;
        let wr_cyan = vec3<f32>(0.45, 0.85, 1.35);
        let wr_magenta = vec3<f32>(1.25, 0.40, 0.95);
        let wr_col = mix(wr_magenta, wr_cyan, wind_wave);
        
        let rim = pow(1.0 - NdotV, 2.2) * vec3<f32>(1.1, 0.6, 1.5) * 4.0;
        final_col = wr_col * 3.2 + rim;
    }
    // Fallback standard star
    else {
        let limb = pow(NdotV, 0.50);
        final_col = seed_col * (limb * 0.8 + 0.2) * 3.0;
    }
    
    return final_col;
}

struct CraterShadingResult {
    color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    emissive: vec3<f32>,
};

// -----------------------------------------------------------------------------
// Specialized Planetary Impact Regimes: Gas Giants, Ice Giants, Icy Moons, Rocky Worlds
// -----------------------------------------------------------------------------

fn apply_gas_giant_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
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

    if (u < 1.7) {
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

        // Stratospheric incandescent fireball flash when fresh
        let active_melt = melt_glow * effective_scar;
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

    if (u < 1.7) {
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

        // High-energy atmospheric cyan/blue shock glow when fresh
        let active_melt = melt_glow * effective_scar;
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
    dist: f32,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    time: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    let u = dist / radius;

    if (u < 1.7) {
        let ray_angle = atan2(p_surf.x - center.x, p_surf.z - center.z);
        let ray_noise = sin(ray_angle * 16.0 + fbm(p_surf * 20.0) * 4.0) * 0.5 + 0.5;

        // High-albedo shattered crystalline ice ejecta blanket and rays
        let ejecta_ice = smoothstep(1.7, 0.90, u) * ray_noise * 0.60 * effective_scar;
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
    }
    return out_res;
}

fn apply_rocky_world_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
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

    if (u < 1.6) {
        let ray_angle = atan2(p_surf.x - center.x, p_surf.z - center.z);
        let ray_noise = sin(ray_angle * 14.0 + fbm(p_surf * 18.0) * 4.0) * 0.5 + 0.5;
        let ejecta_blanket = smoothstep(1.6, 0.95, u) * ray_noise * 0.45 * effective_scar;
        let ejecta_dust = vec3<f32>(0.48, 0.45, 0.42);

        let rim_factor = smoothstep(0.75, 0.98, u) * smoothstep(1.25, 0.98, u) * effective_scar;
        let rim_rock = vec3<f32>(0.38, 0.35, 0.33);

        let floor_factor = smoothstep(0.85, 0.70, u) * effective_scar;
        let basalt_mare = vec3<f32>(0.09, 0.085, 0.08);

        var central_peak = 0.0;
        if (radius > 0.12 && u < 0.22) {
            central_peak = smoothstep(0.22, 0.05, u) * 0.7 * effective_scar;
        }

        var crater_col = mix(out_res.color, ejecta_dust, ejecta_blanket);
        crater_col = mix(crater_col, rim_rock, rim_factor);
        crater_col = mix(crater_col, basalt_mare, floor_factor);
        crater_col = mix(crater_col, rim_rock, central_peak);

        out_res.color = crater_col;
        out_res.roughness = mix(out_res.roughness, 0.92, (floor_factor * 0.8 + rim_factor * 0.5) * (1.0 - water_mask));
        out_res.metallic = mix(out_res.metallic, 0.08, floor_factor * 0.7 * (1.0 - water_mask));

        let active_melt = melt_glow * effective_scar;
        if (active_melt > 0.005 && u < 0.82) {
            let pool_mask = smoothstep(0.82, 0.55, u);
            let convection_cracks = fbm(p_surf * 32.0 + vec3<f32>(time * 0.05, 0.0, 0.0));
            let lava_core = vec3<f32>(1.0, 0.48, 0.08) * (3.8 + sin(time * 2.8 + u * 10.0) * 0.6);
            let dark_crust = vec3<f32>(0.12, 0.09, 0.08);

            let is_open_lava = smoothstep(0.38, 0.62, convection_cracks);
            let lava_mix = mix(dark_crust, lava_core, is_open_lava);

            out_res.color = mix(out_res.color, lava_mix, pool_mask * active_melt);
            out_res.emissive += lava_core * pool_mask * active_melt * (is_open_lava * 0.85 + 0.15);
            out_res.roughness = mix(out_res.roughness, 0.25, pool_mask * active_melt);
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

    let is_gas_giant = planet.planet_type == 1u || planet.composition.w > 0.40;
    let is_ice_giant = planet.planet_type == 2u;
    let is_icy_world = !is_gas_giant && !is_ice_giant && planet.composition.y > 0.35;

    for (var i = 0u; i < 4u; i = i + 1u) {
        let b_pos = planet.impact_basins_pos[i];
        let b_data = planet.impact_basins_data[i];
        let radius = b_pos.w;
        let scar_intensity = clamp(b_data.w, 0.0, 1.0);
        let melt_glow = b_data.x;

        // If covered by liquid ocean, water fills the basin and hides the cooled crater scar.
        // Active molten magma (melt_glow) can boil and flash through water upon impact,
        // but once cooled, the ocean completely covers and conceals the scar.
        let effective_scar = scar_intensity * (1.0 - water_mask * (1.0 - melt_glow * 0.4));
        if (radius <= 0.005 || effective_scar <= 0.005) {
            continue;
        }

        let center = normalize(b_pos.xyz);
        let elongation = max(b_data.y, 1.0);

        let cos_dist = clamp(dot(p_surf, center), -1.0, 1.0);
        let dist = acos(cos_dist);

        var effective_dist = dist;
        if (elongation > 1.05) {
            let tangent = normalize(cross(center, vec3<f32>(0.0, 1.0, 0.001)));
            let d_tangent = dot(p_surf - center, tangent);
            let d_norm = sqrt(max(dist * dist - d_tangent * d_tangent, 0.0));
            effective_dist = sqrt((d_tangent / elongation) * (d_tangent / elongation) + d_norm * d_norm);
        }

        if (is_gas_giant) {
            out_res = apply_gas_giant_impact_basin(p_surf, center, radius, effective_scar, melt_glow, time, out_res);
        } else if (is_ice_giant) {
            out_res = apply_ice_giant_impact_basin(p_surf, center, radius, effective_scar, melt_glow, time, out_res);
        } else if (is_icy_world) {
            out_res = apply_icy_world_impact_basin(p_surf, center, effective_dist, radius, effective_scar, melt_glow, time, out_res);
        } else {
            out_res = apply_rocky_world_impact_basin(p_surf, center, effective_dist, radius, effective_scar, melt_glow, time, water_mask, out_res);
        }
    }

    return out_res;
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
    spin: f32,
    spot_size: f32,
    spot_lat: f32,
    spot_lon: f32,
    spin_rate: f32,
    secondary_count: f32,
    mass_jup: f32,
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

    // High-speed outer collar wind peaks at r_ell ~ 0.85
    let collar = exp(-pow((r_ell - 0.85) / 0.20, 2.0));

    // Core body (steep edge at r_ell = 1.0)
    let body = smoothstep(1.08, 0.45, r_ell);

    // Calm central eye (inner core radius ~ 0.35)
    let eye = smoothstep(0.38, 0.10, r_ell);

    // Anticyclonic counter-clockwise spiral arms
    let vortex_angle = atan2(v, u);
    let spiral = sin(vortex_angle * 3.0 - spin_rate * t * 0.4 - r_ell * 5.0) * 0.5 + 0.5;

    // Trailing Kelvin-Helmholtz turbulent wake downstream (eastward/westward shear flow)
    var wake = 0.0;
    if (d_lon > (0.6 * spot_size) && d_lon < (3.6 * spot_size) && abs(d_lat) < (0.9 * spot_size)) {
        let wake_x = (d_lon - 0.6 * spot_size) / (3.0 * spot_size);
        let wake_y = d_lat / (0.9 * spot_size);
        let wake_envelope = (1.0 - wake_x) * exp(-wake_y * wake_y * 3.0);
        let wake_turb = fbm(p_gas * 22.0 + vec3<f32>(t * 0.12, 0.0, 0.0));
        wake = wake_envelope * smoothstep(0.40, 0.85, wake_turb);
    }

    // Dynamic storm coloration based on mass/tier
    var spot_core_col = vec3<f32>(0.92, 0.26, 0.10); // Classic Jovian crimson/brick-red
    var collar_col = vec3<f32>(0.82, 0.42, 0.18);    // Lighter orange-tan collar
    var eye_col = vec3<f32>(0.74, 0.18, 0.08);       // Deep darker calm eye
    if (mass_jup > 6.0) {
        spot_core_col = vec3<f32>(0.85, 0.18, 0.65);
        collar_col = vec3<f32>(0.70, 0.30, 0.85);
        eye_col = vec3<f32>(0.55, 0.10, 0.45);
    } else if (mass_jup > 3.5) {
        spot_core_col = vec3<f32>(0.18, 0.65, 0.95);
        collar_col = vec3<f32>(0.35, 0.80, 0.98);
        eye_col = vec3<f32>(0.10, 0.45, 0.75);
    } else if (mass_jup > 1.8) {
        spot_core_col = vec3<f32>(0.15, 0.88, 0.65);
        collar_col = vec3<f32>(0.30, 0.95, 0.80);
        eye_col = vec3<f32>(0.08, 0.65, 0.48);
    }

    var grs_col = mix(spot_core_col, collar_col, collar * 0.65);
    grs_col = mix(grs_col, eye_col, eye * 0.75);
    // Spiral filaments texture
    grs_col = grs_col * (0.88 + spiral * 0.24);

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
        let ba_body = smoothstep(1.05, 0.35, ba_r);
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
                let o_body = smoothstep(1.05, 0.30, o_r);
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
    let sin_lat = dot(norm, s_axis);
    let polar_angle = abs(sin_lat);

    let tilt = planet.dynamics_and_mag.w;
    let p_tilted = rotate_z(norm, -tilt);
    
    let spin = planet.spin_rate;
    let t = planet.time;
    let temp = planet.temperature;
    
    // 1. Solid surface coordinate (drifts with planetary rotation period)
    let p_surf = rotate_y(p_tilted, t * spin);
    
    // 2. Cloud and atmospheric coordinate with zonal trade winds
    let lat = p_tilted.y;
    let zonal_drift = sin(lat * 3.14159 * 2.0) * 0.15;
    let p_cloud = rotate_y(p_tilted, t * (spin * 1.25 + 0.04) + zonal_drift);
    let p_cloud_sub = rotate_y(p_tilted, t * (spin * 0.85 - 0.03) - zonal_drift * 0.8);
    
    let rock = planet.composition.x;
    let ice = planet.composition.y;
    let metal = planet.composition.z;
    let gas = planet.composition.w;

    let has_volatiles = (ice > 0.002 || planet.climate_and_bio.x > 0.01 || planet.climate_and_bio.y > 0.005);
    let ocean_frac = select(0.0, max(planet.climate_and_bio.x, ice), has_volatiles);
    let ice_frac = select(0.0, planet.climate_and_bio.y, has_volatiles);
    let biomass = select(0.0, planet.climate_and_bio.z, has_volatiles);
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

    // =========================================================================
    // 1. Gas Giant (Jupiter / Saturn / Super-Jupiters / Hot Jupiters / Brown Dwarfs)
    // =========================================================================
    if (planet.planet_type == 1u) {
        let mass_jup = max(planet.dynamics_and_mag.z, 0.1);
        
        // Differential counter-rotating latitudinal jet streams (faster on massive worlds)
        let jet_stream = sin(lat * (16.0 + min(mass_jup, 6.0) * 2.0)) * (t * 0.12);
        var p_gas = rotate_y(p_tilted, t * (spin * 0.8) + jet_stream);
        
        // Atmospheric storm parameters from uniforms
        let hex_amp = planet.storm_features.x;
        let hex_k = planet.storm_features.y;
        let spot_size = planet.storm_features.z;
        let spot_lat = planet.storm_features.w;
        let spot_lon = planet.storm_dynamics.x;
        let vortex_spin = planet.storm_dynamics.y;
        let sec_ovals = planet.storm_dynamics.z;
        let shear_turb = planet.storm_dynamics.w;

        // Dynamic vortex swirl distortion on coordinates
        if (spot_size > 0.01) {
            let spot_y = sin(spot_lat);
            let spot_r = cos(spot_lat);
            let spot_center = vec3<f32>(spot_r * cos(spot_lon), spot_y, spot_r * sin(spot_lon));
            p_gas = vortex_swirl(p_gas, spot_center, spot_size * 1.5, vortex_spin * 0.85 + sin(t * 0.5) * 0.4);
        } else if (hex_amp <= 0.001) {
            // Anticyclonic Great Red Spot / Primary Storm Vortex fallback for default worlds
            let spot_center = vec3<f32>(0.65, -0.28, 0.65);
            p_gas = vortex_swirl(p_gas, spot_center, 0.42, 3.2 + sin(t * 0.5) * 0.8);
        }
        
        // Secondary Counter-Rotating Anticyclone for Super-Jupiters (> 1.8 M_jup)
        if (mass_jup > 1.8) {
            let spot2_center = vec3<f32>(-0.62, 0.35, -0.58);
            p_gas = vortex_swirl(p_gas, spot2_center, 0.36, -2.6 - cos(t * 0.4) * 0.7);
        }
        
        let band_lat = p_gas.y * (14.0 + min(mass_jup, 10.0) * 1.6);
        let flow = fbm(p_gas * 6.5 + vec3<f32>(t * 0.04, 0.0, -t * 0.02));
        let storm = fbm(p_gas * 18.0 + vec3<f32>(t * 0.08, 0.0, 0.0));
        
        // Kelvin-Helmholtz shear billows across zonal jet boundaries
        let kh_shear = sin(p_gas.x * 24.0 + sin(band_lat * 2.0) * 3.5) * cos(p_gas.z * 24.0) * shear_turb * 0.22;
        let band_val = sin(band_lat + flow * 2.4 + kh_shear) * 0.5 + 0.5;
        
        // Dynamic palette derivation: dark belts (c1) vs light zones (c2)
        let seed = planet.color_seed.rgb;
        let c1 = seed * 0.75;
        let c2 = seed * 1.35 + vec3<f32>(0.08, 0.08, 0.08);
        var c3 = mix(c1, c2, band_val);
        
        // Evaluate primary anticyclonic storm (Great Red Spot) and secondary ovals
        if (spot_size > 0.01) {
            let vortex_res = evaluate_anticyclonic_vortices(
                p_gas, t, spin, spot_size, spot_lat, spot_lon, vortex_spin, sec_ovals, mass_jup
            );
            if (vortex_res.is_active) {
                c3 = mix(c3, vortex_res.color, vortex_res.spot_mask * 0.95);
                c3 = mix(c3, c1 * 0.55, vortex_res.wake_mask * 0.70);
                c3 = mix(c3, vec3<f32>(0.96, 0.94, 0.90), vortex_res.oval_mask * 0.85);
            }
        } else if (hex_amp <= 0.001) {
            // Fallback spot for unconfigured custom gas giants
            let spot_center = vec3<f32>(0.65, -0.28, 0.65);
            let spot_dist = distance(p_gas, spot_center);
            let spot_mask = smoothstep(0.35, 0.05, spot_dist);
            
            var spot_color = vec3<f32>(0.90, 0.32, 0.12);
            if (mass_jup > 6.0) {
                spot_color = vec3<f32>(0.85, 0.20, 0.65);
            } else if (mass_jup > 3.5) {
                spot_color = vec3<f32>(0.20, 0.75, 0.95);
            } else if (mass_jup > 1.8) {
                spot_color = vec3<f32>(0.15, 0.88, 0.70);
            }
            c3 = mix(c3, spot_color, spot_mask * 0.90);
        }

        // Evaluate standing Rossby wave polar hexagon (Saturn's North Polar Hexagon)
        if (hex_amp > 0.001) {
            let hex_res = evaluate_polar_hexagon(p_gas, t, hex_amp, hex_k);
            if (hex_res.is_active) {
                let hex_blend = max(hex_res.inside_mask * 0.85, hex_res.jet_mask * 0.95);
                c3 = mix(c3, hex_res.color, hex_blend);
                if (hex_res.eye_mask > 0.0) {
                    c3 = mix(c3, vec3<f32>(0.28, 0.14, 0.05), hex_res.eye_mask * 0.95);
                }
            }
        }
        
        let white_ovals = smoothstep(0.72, 0.88, storm) * smoothstep(0.6, -0.6, abs(lat));
        color = mix(c3, vec3<f32>(0.96, 0.94, 0.90), white_ovals * 0.55);
        
        // Thermal night-side infrared emission for Brown Dwarfs and ultra-hot Super-Jupiters
        if (mass_jup > 10.0 || temp > 800.0) {
            let thermal_emission = vec3<f32>(1.0, 0.38, 0.08) * smoothstep(0.60, 0.95, flow) * 1.5;
            color += thermal_emission;
        }
    }
    // =========================================================================
    // 2. Ice Giant (Uranus / Neptune / Sub-Neptunes)
    // =========================================================================
    else if (planet.planet_type == 2u) {
        let jet_stream = sin(lat * 10.0) * (t * 0.08);
        let p_ice_gas = rotate_y(p_tilted, t * spin + jet_stream);
        let swirl = fbm(p_ice_gas * 4.5 + vec3<f32>(t * 0.02, 0.0, t * 0.01));
        let band = sin(lat * 8.0 + swirl * 1.2) * 0.5 + 0.5;
        
        let deep_cyan = vec3<f32>(0.08, 0.42, 0.75);
        let bright_azure = vec3<f32>(0.32, 0.72, 0.96);
        let methane_veil = mix(deep_cyan, bright_azure, band);
        
        // High-altitude cirrus clouds with fast prograde drift
        let cirrus_coord = rotate_y(p_tilted, t * (spin * 1.35) + jet_stream * 1.5);
        let cirrus = fbm(cirrus_coord * 14.0);
        let white_clouds = smoothstep(0.65, 0.85, cirrus);
        
        var base_ice = mix(methane_veil, vec3<f32>(0.92, 0.96, 1.0), white_clouds * 0.55);

        // Neptune's Great Dark Spot and companion cirrus "Scooter"
        let spot_size = planet.storm_features.z;
        if (spot_size > 0.01) {
            let dark_spot = evaluate_anticyclonic_vortices(
                p_ice_gas, t, spin, spot_size, planet.storm_features.w,
                planet.storm_dynamics.x, planet.storm_dynamics.y, planet.storm_dynamics.z, 0.8
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
        let elev = fbm(p_tectonic * 3.4);
        let ridge = ridge_noise(p_tectonic * 7.2);
        let combined_elev = elev * 0.65 + ridge * 0.35;
        
        let has_oceans = has_volatiles && ocean_frac > 0.02;
        let sea_level = clamp(0.20 + ocean_frac * 0.45, 0.25, 0.80);
        let is_ice_cold = temp < 285.0; // Ice caps melt completely above 12 °C
        let ice_cap_thresh = clamp(0.95 - (ice_frac * 0.40) - (273.0 / max(temp, 160.0)) * 0.04, 0.70, 0.99);
        
        // Polar Ice Shields & Glacial Calving Shelves:
        // Form ONLY if planet has volatiles, is cold enough, at the true 3D spin poles,
        // and not facing baking direct sunlight (insolation < 0.35)
        if (has_volatiles && is_ice_cold && polar_angle > ice_cap_thresh && stellar_insolation < 0.35) {
            let frost = fbm(p_surf * 14.0);
            let pack_ice = vec3<f32>(0.94, 0.97, 1.0);
            let glacial_blue = vec3<f32>(0.65, 0.82, 0.98);
            color = mix(pack_ice, glacial_blue, frost * 0.45);
            pbr_input.material.perceptual_roughness = 0.25;
        }
        // Vast Sapphire Oceans & Coastal Turquoise Continental Shelves (ONLY if world has oceans!)
        else if (has_oceans && combined_elev < sea_level) {
            water_mask = clamp((sea_level - combined_elev) / 0.05, 0.0, 1.0);
            let depth = (sea_level - combined_elev) / sea_level;
            let abyssal_trench = vec3<f32>(0.01, 0.04, 0.24);
            let deep_sapphire = vec3<f32>(0.02, 0.14, 0.48);
            let continental_shelf = vec3<f32>(0.06, 0.48, 0.75);
            let coastal_lagoon = vec3<f32>(0.12, 0.72, 0.84);
            
            let modern_ocean = mix(continental_shelf, abyssal_trench, clamp(depth * 2.4, 0.0, 1.0));
            let shore_blend = smoothstep(sea_level - 0.035, sea_level, combined_elev);
            let modern_sea = mix(modern_ocean, coastal_lagoon, shore_blend * 0.75);

            let archean_iron_sea = mix(vec3<f32>(0.07, 0.24, 0.16), vec3<f32>(0.02, 0.09, 0.06), clamp(depth * 2.4, 0.0, 1.0));
            color = mix(archean_iron_sea, modern_sea, clamp(ocean_oxidation, 0.0, 1.0));
            
            // Specular Liquid Glint
            pbr_input.material.perceptual_roughness = 0.05;
            pbr_input.material.metallic = 0.02;
        }
        // Continents, Mountain Ranges & Biomes (or Dry Super-Earth Lithosphere)
        else {
            let terrain_var = fbm(p_surf * 8.5);
            
            if (has_oceans) {
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
        
        // Massive Multi-Scale Atmospheric Cloud Circulation & Storm Vortices (only if world has atmosphere/volatiles)
        if (has_volatiles || gas > 0.05) {
            let p_cloud_rot = rotate_y(p_tilted, t * (spin * 1.12) + zonal_drift);
            let cloud_main = fbm(p_cloud_rot * 4.6);
            let cloud_spirals = fbm(p_cloud_rot * 9.5 + vec3<f32>(0.0, t * 0.02, 0.0));
            let storm_bands = sin(lat * 10.0 + cloud_main * 2.2) * 0.5 + 0.5;
            let super_clouds = cloud_main * 0.60 + cloud_spirals * 0.25 + storm_bands * 0.15;
            
            // Soft cloud drop shadows on land and ocean surfaces
            let shadow_rot = rotate_y(p_tilted, t * (spin * 1.12) + zonal_drift) + vec3<f32>(0.025, 0.015, 0.025);
            let shadow_val = fbm(shadow_rot * 4.6);
            if (shadow_val > 0.50) {
                color = color * (1.0 - (shadow_val - 0.50) * 0.45);
            }
            
            let cloud_thresh = 0.46;
            if (super_clouds > cloud_thresh) {
                let cloud_alpha = clamp((super_clouds - cloud_thresh) * 2.8, 0.0, 0.94);
                let cloud_white = vec3<f32>(0.96, 0.98, 1.0);
                color = mix(color, cloud_white, cloud_alpha);
                pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.92, cloud_alpha);
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

        // Multi-scale bimodal crustal elevation:
        // 1. Broad continental lithospheric shields vs deep abyssal basins
        let continent_mask = fbm(p_tectonic * 2.1);
        // 2. Tectonic cordillera mountain belts and rift valleys
        let mountain_ridges = ridge_noise(p_tectonic * 6.5);
        let detail_hills = fbm(p_tectonic * 4.8);
        
        // Supercontinent Wilson cycle aggregation dipole:
        // When aggregation is high (Pangea, Rodinia, Vaalbara, Pangea Ultima), continents congregate
        // into a massive unified landmass surrounded by the Panthalassa superocean.
        // When aggregation is low (Modern Earth), continents disperse across both hemispheres.
        let super_axis = normalize(vec3<f32>(0.65, 0.25, 0.70));
        let cluster_dipole = dot(p_surf, super_axis);
        let cluster_bias = cluster_dipole * (aggregation - 0.20) * 0.14;
        let elev = continent_mask * 0.62 + detail_hills * 0.24 + mountain_ridges * 0.14 + cluster_bias;

        // A. Molten Magma Ocean Planet (temp >= 1100K or high lava_frac)
        if (temp >= 1100.0 || lava_frac > 0.65) {
            let magma = evaluate_magma_ocean_surface(p_surf, elev, temp, lava_frac, t);
            color = magma.color;
            pbr_input.material.emissive = vec4<f32>(magma.emissive, 1.0);
            pbr_input.material.perceptual_roughness = magma.roughness;
            pbr_input.material.metallic = magma.metallic;
        }
        // B. Superheated Venusian Runaway Greenhouse (temp >= 380K with dense atmosphere)
        else if (temp >= 380.0 && (gas > 0.05 || cloud_density > 0.5 || pressure_bar > 5.0)) {
            let super_rot = rotate_y(p_tilted, t * (spin * 3.5));
            let clouds = fbm(super_rot * 4.5 + vec3<f32>(t * 0.05, 0.0, t * 0.05));
            let band = sin(lat * 8.0 + clouds * 1.8) * 0.5 + 0.5;
            let sulfur_deck = mix(vec3<f32>(0.86, 0.78, 0.50), vec3<f32>(0.96, 0.90, 0.70), band);
            color = sulfur_deck * (0.90 + clouds * 0.20);
        }
        // C. Frozen Snowball Glacial World (ice_frac >= 0.60 or temp < 255K with water)
        else if (has_volatiles && (ice_frac >= 0.60 || (temp < 255.0 && ocean_frac > 0.05))) {
            let frost = fbm(p_surf * 8.0);
            let glaciers = mix(vec3<f32>(0.85, 0.92, 0.99), vec3<f32>(0.45, 0.75, 0.92), elev);
            let pack_ice = vec3<f32>(0.94, 0.97, 1.00);
            color = mix(glaciers, pack_ice, smoothstep(0.3, 0.7, frost));
            pbr_input.material.perceptual_roughness = 0.22;
        }
        // D. Temperate Water-Bearing / Habitable Biosphere World
        else if (has_volatiles && ocean_frac >= 0.04 && temp >= 240.0 && temp <= 380.0) {
            let sea_level = clamp(0.41 + ocean_frac * 0.24, 0.25, 0.75);
            let is_ice_cold = temp < 288.0; // Polar ice caps melt if global temp exceeds 15 °C (Earth ~288 K)
            let ice_cap_thresh = clamp(0.95 - (ice_frac * 0.50) - (273.0 / max(temp, 150.0)) * 0.05, 0.68, 0.99);
            
            // Polar Ice Caps: requires cold temperature, 3D spin pole latitude, and low insolation
            if (is_ice_cold && polar_angle > ice_cap_thresh && stellar_insolation < 0.35) {
                color = vec3<f32>(0.94, 0.97, 1.0);
                pbr_input.material.perceptual_roughness = 0.25;
            }
            // Oceans & Liquid Seas
            else if (elev < sea_level) {
                water_mask = clamp((sea_level - elev) / 0.05, 0.0, 1.0);
                let depth = (sea_level - elev) / max(sea_level, 0.1);
                let deep_ocean = vec3<f32>(0.01, 0.08, 0.38);
                let shallow_lagoon = vec3<f32>(0.04, 0.42, 0.72);
                let coastal_cyan = vec3<f32>(0.10, 0.62, 0.75);
                
                let water_color = mix(shallow_lagoon, deep_ocean, clamp(depth * 1.8, 0.0, 1.0));
                let shore_blend = smoothstep(sea_level - 0.04, sea_level, elev);
                let modern_sea = mix(water_color, coastal_cyan, shore_blend * 0.65);

                // Archean anoxic iron-rich murky sea-green ocean
                let archean_iron_sea = mix(vec3<f32>(0.08, 0.26, 0.18), vec3<f32>(0.02, 0.10, 0.07), clamp(depth * 1.8, 0.0, 1.0));
                color = mix(archean_iron_sea, modern_sea, clamp(ocean_oxidation, 0.0, 1.0));
                
                // Specular Ocean Glint (Smooth liquid water reflectiveness)
                pbr_input.material.perceptual_roughness = 0.08;
                pbr_input.material.metallic = 0.02;
            }
            // Continents & Landmasses
            else {
                let rel_elev = elev - sea_level;
                pbr_input.material.perceptual_roughness = 0.82;
                
                // Active Photosynthetic Biosphere (gated by terrestrial vegetation expansion)
                let effective_bio = biomass * vegetation_mult;
                if (effective_bio > 0.02) {
                    let bio_noise = fbm(p_tectonic * 9.0);
                    let lush_canopy = vec3<f32>(0.10, 0.50, 0.16); // emerald rainforest
                    let savanna_meadow = vec3<f32>(0.26, 0.60, 0.20); // temperate grasslands
                    let highland_taiga = vec3<f32>(0.16, 0.40, 0.18);
                    let alpine_peaks = vec3<f32>(0.75, 0.72, 0.70);
                    
                    if (rel_elev > 0.25) {
                        color = mix(highland_taiga, alpine_peaks, (rel_elev - 0.25) * 4.0);
                    } else if (rel_elev > 0.10) {
                        let veg = mix(savanna_meadow, lush_canopy, bio_noise);
                        color = mix(vec3<f32>(0.55, 0.45, 0.30), veg, clamp(effective_bio * 1.4, 0.0, 1.0));
                    } else {
                        let coastal_veg = mix(lush_canopy, savanna_meadow, bio_noise);
                        color = mix(vec3<f32>(0.72, 0.62, 0.42), coastal_veg, clamp(effective_bio * 1.5, 0.0, 1.0));
                    }
                } else {
                    // Pre-Phanerozoic barren cratons (reddish oxidized silicates and ancient granite shields)
                    let craton_lowlands = vec3<f32>(0.48, 0.35, 0.24);
                    let craton_iron_shield = vec3<f32>(0.38, 0.22, 0.16);
                    let craton_highlands = vec3<f32>(0.65, 0.58, 0.52);

                    if (rel_elev > 0.22) {
                        color = mix(craton_iron_shield, craton_highlands, (rel_elev - 0.22) * 4.0);
                    } else if (rel_elev > 0.10) {
                        color = craton_lowlands;
                    } else {
                        color = craton_iron_shield;
                    }
                }
            }
            
            // Dual-Layer Atmospheric Water-Vapor Clouds & Cyclones
            if (cloud_density > 0.02) {
                let c_main = fbm(p_cloud * 5.2);
                let c_sub = fbm(p_cloud_sub * 9.5);
                let total_clouds = c_main * 0.65 + c_sub * 0.35;
                
                // Soft cloud shadows cast onto the ground
                let shadow_coord = rotate_y(p_tilted, t * (spin * 1.25 + 0.04) + zonal_drift) + vec3<f32>(0.03, 0.02, 0.03);
                let shadow_val = fbm(shadow_coord * 5.2);
                if (shadow_val > 0.55 && elev >= sea_level) {
                    color = color * (1.0 - (shadow_val - 0.55) * 0.5);
                }
                
                let cloud_thresh = 0.55 - cloud_density * 0.10;
                if (total_clouds > cloud_thresh) {
                    let cloud_alpha = clamp((total_clouds - cloud_thresh) * 2.6 * clamp(cloud_density * 1.1, 0.2, 0.85), 0.0, 0.78);
                    color = mix(color, vec3<f32>(0.96, 0.98, 1.0), cloud_alpha);
                    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.90, cloud_alpha);
                }
            }
        }
        // E. Metal-Rich World (Mercury type)
        else if (metal > 0.42) {
            let sheen = fbm(p_surf * 7.0);
            let craters = fbm(p_surf * 9.5);
            let dark_graphite = vec3<f32>(0.18, 0.18, 0.20);
            let nickel_iron = vec3<f32>(0.65, 0.62, 0.58);
            color = mix(dark_graphite, nickel_iron, sheen * 0.6 + craters * 0.4);
        }
        // G. Barren Dry Silicate Rock (Moon / Mars)
        else {
            let craters = fbm(p_surf * 8.0);
            let highlands = fbm(p_surf * 3.5);
            
            pbr_input.material.perceptual_roughness = 0.88;
            pbr_input.material.metallic = 0.10;
            
            if (temp > 280.0) {
                let lowlands = vec3<f32>(0.42, 0.25, 0.15);
                let peaks = vec3<f32>(0.72, 0.48, 0.28);
                color = mix(lowlands, peaks, highlands * 0.7 + craters * 0.3);
            } else {
                let lowlands = vec3<f32>(0.22, 0.22, 0.24);
                let peaks = vec3<f32>(0.55, 0.54, 0.52);
                color = mix(lowlands, peaks, highlands * 0.7 + craters * 0.3);
            }
        }

        // Magma ocean cooling & crustal solidification overlay (450K <= temp < 1100K or active volcanism)
        if (temp < 1100.0 && (temp > 450.0 || lava_frac > 0.02)) {
            let magma = evaluate_magma_ocean_surface(p_surf, elev, temp, lava_frac, t);
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
    }

    pbr_input.material.base_color = vec4<f32>(color, 1.0);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    
    if (planet.planet_type == 0u) {
        // Universal Procedural Stellar Photosphere Engine
        let star_photosphere = render_stellar_photosphere(p_surf, norm, pbr_input.V, t);
        out.color = vec4<f32>(star_photosphere, 1.0);
    } else if (planet.planet_type == 7u) {
        // Dedicated Black Hole Star (Quasi-Star / JWST Little Red Dot) Pseudo-Photosphere
        let quasi_photosphere = render_quasistar_photosphere(p_surf, norm, pbr_input.V, t);
        out.color = vec4<f32>(quasi_photosphere, 1.0);
    } else if (planet.planet_type == 5u) {
        // Gravitational singularity event horizon + photon ring
        let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0);
        let photon_ring = pow(1.0 - NdotV, 6.0);
        let ring_color = vec3<f32>(1.0, 0.65, 0.25) * photon_ring * 8.0;
        out.color = vec4<f32>(ring_color, 1.0);
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
        for (var i = 0u; i < 2u; i = i + 1u) {
            if (planet.eclipse_moons_data[i].x > 0.5) {
                let m_pos = planet.eclipse_moons_pos[i].xyz;
                let m_rad = planet.eclipse_moons_pos[i].w;
                let V = m_pos - pbr_input.N;
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
        
        // Extract ocean / ice specular highlight and tone-map smoothly (shadowed by rings & moons)
        let specular_raw = max(lit.rgb - base_col * raw_starlight, vec3<f32>(0.0)) * total_shadow;
        let specular_glint = specular_raw / (specular_raw + vec3<f32>(1.2)) * 0.65;
        
        let balanced_lit = diffuse_lit + specular_glint;
        
        let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0);
        let fresnel = pow(1.0 - NdotV, 2.8);
        let ambient_boost = base_col * 0.08;
        let NdotL = dot(pbr_input.N, star_dir); // Physical solar zenith angle
        
        // Rayleigh & Mie atmospheric scattering with golden/crimson sunset terminators (planets only)
        var atmospheric_haze = vec3<f32>(0.0);
        if (planet.planet_type != 4u && (pressure_bar > 0.005 || cloud_density > 0.05 || planet.planet_type == 1u || planet.planet_type == 2u || planet.planet_type == 6u)) {
            let twilight = exp(-NdotL * NdotL * 16.0); // Concentrated along day-night terminator line
            
            // Forward Mie aerosol scattering when looking towards the star
            let forward_mie = pow(max(dot(-pbr_input.V, star_dir), 0.0), 8.0) * 0.45;
            
            let day_rayleigh = planet.scattering_params.rgb;
            let sunset_hue = vec3<f32>(1.0, 0.38, 0.08) * (1.0 + forward_mie);
            let ground_scatter = mix(day_rayleigh, sunset_hue, twilight);
            let haze_scale = clamp(pressure_bar * 0.4 + 0.30, 0.15, 1.4);
            
            let day_factor = clamp(NdotL * 3.0 + 0.2, 0.0, 1.0) * mix(0.20, 1.0, total_shadow);
            atmospheric_haze = ground_scatter * (fresnel * haze_scale + forward_mie * 0.5) * day_factor;
        }
        
        // Dynamic Polar Auroral Ovals & Coronal Excitation
        var aurora_glow = vec3<f32>(0.0);
        let oval_colat = planet.aurora_params.x;
        let oval_width = max(planet.aurora_params.y, 0.04);
        let aurora_int = planet.aurora_params.z;
        let kp_index = planet.aurora_params.w;

        let effective_aurora_int = select(aurora_int, clamp(mag_gauss * 0.8, 0.0, 2.0), aurora_int <= 0.01 && mag_gauss > 0.15);
        let effective_colat = select(oval_colat, 0.315, oval_colat <= 0.01);

        if (effective_aurora_int > 0.05 && (pressure_bar >= 0.001 || planet.planet_type == 1u || planet.planet_type == 2u)) {
            // Magnetic polar co-latitude in tilted rotating frame
            let polar_co = min(acos(clamp(p_tilted.y, -1.0, 1.0)), acos(clamp(-p_tilted.y, -1.0, 1.0)));
            let d_oval = abs(polar_co - effective_colat);
            let oval_ring = exp(-0.5 * (d_oval * d_oval) / (oval_width * oval_width));

            if (oval_ring > 0.01) {
                // Birkeland field-aligned current rayed curtains (azimuthal striations)
                let az = atan2(p_tilted.z, p_tilted.x);
                let ray_noise = fbm(vec3<f32>(az * 16.0 + t * 0.35, p_tilted.y * 24.0, t * 0.20));
                let curtain_rays = smoothstep(0.32, 0.70, ray_noise);

                // Multi-spectral emission: 557.7nm emerald green core, 391.4nm violet edge, 630.0nm ruby red
                let edge_ratio = d_oval / oval_width;
                let emerald_core = vec3<f32>(0.12, 0.98, 0.42);
                let violet_edge = vec3<f32>(0.45, 0.20, 0.95);
                let ruby_fringe = vec3<f32>(0.95, 0.15, 0.25);
                let auroral_col = mix(emerald_core, mix(violet_edge, ruby_fringe, smoothstep(0.6, 1.2, edge_ratio)), smoothstep(0.3, 0.9, edge_ratio));

                // Magnetotail reconnection boost on the nightside
                let night_side_mult = smoothstep(0.20, -0.25, NdotL) * 1.6 + 0.30;
                let storm_flicker = sin(t * 4.0 + az * 8.0) * 0.10 + 0.90;

                // Boosted to 6.5: intense aurora (Kp >= 8, strong magnetosphere) surpasses the
                // bloom threshold (1.8) and emits a neon curtain glow corona. Faint aurora stays
                // sub-threshold and reads as pure emissive colour with no halo.
                aurora_glow = auroral_col * (oval_ring * curtain_rays * effective_aurora_int * night_side_mult * storm_flicker * 6.5);
            }
        }

        // HDR scene-linear radiance accumulation. All emissive sources (lava pools, comet
        // gas vents, biomass phosphorescence) are pre-scaled at their HDR intensities above.
        out.color = vec4<f32>(balanced_lit + ambient_boost + atmospheric_haze + aurora_glow + pbr_input.material.emissive.rgb, 1.0);
    }
    
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
