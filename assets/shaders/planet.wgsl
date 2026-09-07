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

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let norm = normalize(in.world_normal);
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

    let ocean_frac = max(planet.climate_and_bio.x, ice);
    let ice_frac = planet.climate_and_bio.y;
    let biomass = planet.climate_and_bio.z;
    let cloud_density = max(planet.climate_and_bio.w, gas * 0.5);
    let pressure_bar = planet.atmosphere_params.x;
    let mag_gauss = planet.dynamics_and_mag.x;
    let lava_frac = planet.dynamics_and_mag.y;

    var color = pbr_input.material.base_color.rgb;

    // =========================================================================
    // 1. Gas Giant (Jupiter / Saturn / Super-Jupiters / Hot Jupiters / Brown Dwarfs)
    // =========================================================================
    if (planet.planet_type == 1u) {
        let mass_jup = max(planet.dynamics_and_mag.z, 0.1);
        
        // Differential counter-rotating latitudinal jet streams (faster on massive worlds)
        let jet_stream = sin(lat * (16.0 + min(mass_jup, 6.0) * 2.0)) * (t * 0.12);
        var p_gas = rotate_y(p_tilted, t * (spin * 0.8) + jet_stream);
        
        // Anticyclonic Great Red Spot / Primary Storm Vortex
        let spot_center = vec3<f32>(0.65, -0.28, 0.65);
        p_gas = vortex_swirl(p_gas, spot_center, 0.42, 3.2 + sin(t * 0.5) * 0.8);
        
        // Secondary Counter-Rotating Anticyclone for Super-Jupiters (> 1.8 M_jup)
        if (mass_jup > 1.8) {
            let spot2_center = vec3<f32>(-0.62, 0.35, -0.58);
            p_gas = vortex_swirl(p_gas, spot2_center, 0.36, -2.6 - cos(t * 0.4) * 0.7);
        }
        
        let band_lat = p_gas.y * (14.0 + min(mass_jup, 10.0) * 1.6);
        let flow = fbm(p_gas * 6.5 + vec3<f32>(t * 0.04, 0.0, -t * 0.02));
        let storm = fbm(p_gas * 18.0 + vec3<f32>(t * 0.08, 0.0, 0.0));
        
        let band_val = sin(band_lat + flow * 2.4) * 0.5 + 0.5;
        
        // Dynamic palette derivation: dark belts (c1) vs light zones (c2)
        let seed = planet.color_seed.rgb;
        let c1 = seed * 0.75;
        let c2 = seed * 1.35 + vec3<f32>(0.08, 0.08, 0.08);
        let c3 = mix(c1, c2, band_val);
        
        // Primary Great Red Spot / Storm Feature
        let spot_dist = distance(p_gas, spot_center);
        let spot_mask = smoothstep(0.35, 0.05, spot_dist);
        
        // Dynamic storm color matching palette tier
        var spot_color = vec3<f32>(0.90, 0.32, 0.12); // Classic Jovian brick-red
        if (mass_jup > 6.0) {
            spot_color = vec3<f32>(0.85, 0.20, 0.65); // Radiant magenta storm eye
        } else if (mass_jup > 3.5) {
            spot_color = vec3<f32>(0.20, 0.75, 0.95); // Lapis-azure storm
        } else if (mass_jup > 1.8) {
            spot_color = vec3<f32>(0.15, 0.88, 0.70); // Glowing emerald-aquamarine storm
        }
        
        let white_ovals = smoothstep(0.72, 0.88, storm) * smoothstep(0.6, -0.6, abs(lat));
        let base_gas = mix(c3, spot_color, spot_mask * 0.90);
        color = mix(base_gas, vec3<f32>(0.96, 0.94, 0.90), white_ovals * 0.60);
        
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
        
        color = mix(methane_veil, vec3<f32>(0.92, 0.96, 1.0), white_clouds * 0.55);
    }
    // =========================================================================
    // 6. Super-Earth (Dedicated Mega-Terrestrial World: Vast Oceans, Continents, Storms)
    // =========================================================================
    else if (planet.planet_type == 6u) {
        // High surface gravity and massive lithosphere produce sprawling mega-continents,
        // deep abyssal oceans, towering folded cordilleras, and intense cyclonic weather fronts.
        let elev = fbm(p_surf * 3.4);
        let ridge = ridge_noise(p_surf * 7.2);
        let combined_elev = elev * 0.65 + ridge * 0.35;
        let polar_angle = abs(p_surf.y);
        
        let sea_level = 0.46; // Balanced global ocean-to-continent ratio (~68% water, ~32% land)
        let ice_cap_thresh = clamp(0.92 - (273.0 / max(temp, 160.0)) * 0.08, 0.64, 0.98);
        
        // Polar Ice Shields & Glacial Calving Shelves
        if (polar_angle > ice_cap_thresh) {
            let frost = fbm(p_surf * 14.0);
            let pack_ice = vec3<f32>(0.94, 0.97, 1.0);
            let glacial_blue = vec3<f32>(0.65, 0.82, 0.98);
            color = mix(pack_ice, glacial_blue, frost * 0.45);
            pbr_input.material.perceptual_roughness = 0.25;
        }
        // Vast Sapphire Oceans & Coastal Turquoise Continental Shelves
        else if (combined_elev < sea_level) {
            let depth = (sea_level - combined_elev) / sea_level;
            let abyssal_trench = vec3<f32>(0.01, 0.04, 0.24);
            let deep_sapphire = vec3<f32>(0.02, 0.14, 0.48);
            let continental_shelf = vec3<f32>(0.06, 0.48, 0.75);
            let coastal_lagoon = vec3<f32>(0.12, 0.72, 0.84);
            
            let ocean_tone = mix(continental_shelf, abyssal_trench, clamp(depth * 2.4, 0.0, 1.0));
            let shore_blend = smoothstep(sea_level - 0.035, sea_level, combined_elev);
            color = mix(ocean_tone, coastal_lagoon, shore_blend * 0.75);
            
            // Specular Liquid Glint
            pbr_input.material.perceptual_roughness = 0.05;
            pbr_input.material.metallic = 0.02;
        }
        // Continents, Mountain Ranges & Biomes
        else {
            let rel_elev = (combined_elev - sea_level) / (1.0 - sea_level);
            pbr_input.material.perceptual_roughness = 0.84;
            
            let terrain_var = fbm(p_surf * 8.5);
            
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
        }
        
        // Massive Multi-Scale Atmospheric Cloud Circulation & Storm Vortices
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
    // =========================================================================
    // 3. Terrestrial Rocky / Ocean / Biosphere Planet / Protoplanet
    // =========================================================================
    else if (planet.planet_type == 3u || planet.planet_type == 4u) {
        let elev = fbm(p_surf * 3.8);
        let polar_angle = abs(p_surf.y);

        // A. Molten Magma Ocean Planet (temp >= 600K or young accretion embryo)
        if (temp >= 600.0 || lava_frac > 0.05) {
            let crust_drift = fbm(p_surf * 4.2 + vec3<f32>(t * 0.01, 0.0, t * 0.01));
            let fissure = ridge_noise(p_surf * 11.0 + vec3<f32>(0.0, t * 0.015, 0.0));
            let pulse = sin(t * 2.2 + fissure * 6.283) * 0.25 + 0.75;
            
            let is_crust = (elev > 0.38) && (crust_drift > 0.32) && (fissure < 0.68);
            if (is_crust && temp < 1100.0) {
                let dark_basalt = vec3<f32>(0.08, 0.06, 0.06);
                let hot_crust = vec3<f32>(0.22, 0.10, 0.06);
                color = mix(dark_basalt, hot_crust, fissure * 0.5);
            } else {
                let yellow_core = vec3<f32>(1.0, 0.85, 0.35);
                let orange_magma = vec3<f32>(1.0, 0.42, 0.08);
                let deep_crimson = vec3<f32>(0.45, 0.08, 0.04);
                
                let lava_col = mix(orange_magma, yellow_core, clamp(fissure * 1.5 - 0.5, 0.0, 1.0));
                color = mix(deep_crimson, lava_col, clamp(fissure, 0.0, 1.0)) * pulse;
                
                let glow_intensity = clamp(((temp - 600.0) / 400.0) + 1.0, 1.0, 6.0);
                pbr_input.material.emissive = vec4<f32>(color * glow_intensity * pulse, 1.0);
            }
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
        else if (ice_frac >= 0.60 || (temp < 255.0 && ocean_frac > 0.05)) {
            let frost = fbm(p_surf * 8.0);
            let glaciers = mix(vec3<f32>(0.85, 0.92, 0.99), vec3<f32>(0.45, 0.75, 0.92), elev);
            let pack_ice = vec3<f32>(0.94, 0.97, 1.00);
            color = mix(glaciers, pack_ice, smoothstep(0.3, 0.7, frost));
            pbr_input.material.perceptual_roughness = 0.22;
        }
        // D. Temperate Water-Bearing / Habitable Biosphere World
        else if (ocean_frac >= 0.04 && temp >= 250.0 && temp <= 380.0) {
            let sea_level = clamp(0.40 + ocean_frac * 0.45, 0.42, 0.80);
            let ice_cap_thresh = clamp(0.94 - (ice_frac * 0.50) - (273.0 / max(temp, 150.0)) * 0.05, 0.60, 0.98);
            
            // Polar Ice Caps
            if (polar_angle > ice_cap_thresh) {
                color = vec3<f32>(0.94, 0.97, 1.0);
                pbr_input.material.perceptual_roughness = 0.25;
            }
            // Oceans & Liquid Seas
            else if (elev < sea_level) {
                let depth = (sea_level - elev) / max(sea_level, 0.1);
                let deep_ocean = vec3<f32>(0.01, 0.08, 0.38);
                let shallow_lagoon = vec3<f32>(0.04, 0.42, 0.72);
                let coastal_cyan = vec3<f32>(0.10, 0.62, 0.75);
                
                let water_color = mix(shallow_lagoon, deep_ocean, clamp(depth * 1.8, 0.0, 1.0));
                let shore_blend = smoothstep(sea_level - 0.04, sea_level, elev);
                color = mix(water_color, coastal_cyan, shore_blend * 0.65);
                
                // Specular Ocean Glint (Smooth liquid water reflectiveness)
                pbr_input.material.perceptual_roughness = 0.08;
                pbr_input.material.metallic = 0.02;
            }
            // Continents & Landmasses
            else {
                let rel_elev = elev - sea_level;
                pbr_input.material.perceptual_roughness = 0.82;
                
                // Active Photosynthetic Biosphere
                if (biomass > 0.02) {
                    let bio_noise = fbm(p_surf * 9.0);
                    let lush_canopy = vec3<f32>(0.10, 0.50, 0.16); // emerald rainforest
                    let savanna_meadow = vec3<f32>(0.26, 0.60, 0.20); // temperate grasslands
                    let highland_taiga = vec3<f32>(0.16, 0.40, 0.18);
                    let alpine_peaks = vec3<f32>(0.75, 0.72, 0.70);
                    
                    if (rel_elev > 0.25) {
                        color = mix(highland_taiga, alpine_peaks, (rel_elev - 0.25) * 4.0);
                    } else if (rel_elev > 0.10) {
                        let veg = mix(savanna_meadow, lush_canopy, bio_noise);
                        color = mix(vec3<f32>(0.55, 0.45, 0.30), veg, clamp(biomass * 1.4, 0.0, 1.0));
                    } else {
                        let coastal_veg = mix(lush_canopy, savanna_meadow, bio_noise);
                        color = mix(vec3<f32>(0.72, 0.62, 0.42), coastal_veg, clamp(biomass * 1.5, 0.0, 1.0));
                    }
                } else {
                    if (rel_elev > 0.22) {
                        color = mix(vec3<f32>(0.45, 0.38, 0.25), vec3<f32>(0.75, 0.72, 0.70), (rel_elev - 0.22) * 4.0);
                    } else if (rel_elev > 0.10) {
                        color = vec3<f32>(0.55, 0.42, 0.28);
                    } else {
                        color = vec3<f32>(0.68, 0.52, 0.35);
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
                
                let cloud_thresh = 0.52 - cloud_density * 0.14;
                if (total_clouds > cloud_thresh) {
                    let cloud_alpha = clamp((total_clouds - cloud_thresh) * 2.8 * clamp(cloud_density * 1.5, 0.2, 1.0), 0.0, 0.92);
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
        // F. Asteroid / Comet / Planetesimal (planet_type == 4)
        else if (planet.planet_type == 4u) {
            let regolith = fbm(p_surf * 14.0);
            let micro_craters = fbm(p_surf * 22.0);
            let boulder_noise = fbm(p_surf * 40.0);
            
            // Dark carbonaceous / chondritic basalt regolith
            var base_reg = vec3<f32>(0.09, 0.09, 0.10);
            if (metal > 0.35) {
                // Metallic M-type asteroid (Psyche type): dark specular iron-nickel flecks
                let iron_sheen = fbm(p_surf * 18.0);
                base_reg = mix(vec3<f32>(0.15, 0.15, 0.16), vec3<f32>(0.48, 0.46, 0.44), iron_sheen * 0.75);
                pbr_input.material.metallic = 0.65;
                pbr_input.material.perceptual_roughness = 0.45;
            } else if (ice > 0.35) {
                // Pristine icy comet nucleus: dark sublimation crust + exposed bright water ice
                let ice_fissures = fbm(p_surf * 16.0);
                let dark_crust = vec3<f32>(0.06, 0.06, 0.07);
                let bright_ice = vec3<f32>(0.72, 0.82, 0.95);
                base_reg = mix(dark_crust, bright_ice, smoothstep(0.60, 0.85, ice_fissures));
                pbr_input.material.perceptual_roughness = 0.65;
            } else {
                // S-type rocky / C-type carbonaceous chondrite
                let chondrule = mix(vec3<f32>(0.18, 0.16, 0.14), vec3<f32>(0.07, 0.07, 0.08), regolith);
                base_reg = mix(chondrule, vec3<f32>(0.28, 0.26, 0.24), boulder_noise * 0.35);
                pbr_input.material.perceptual_roughness = 0.95;
            }
            color = base_reg * (0.80 + micro_craters * 0.40);
        }
        // G. Barren Dry Silicate Rock (Moon / Mars)
        else {
            let craters = fbm(p_surf * 8.0);
            let highlands = fbm(p_surf * 3.5);
            
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
        
        // Measure starlight illuminance factor relative to surface albedo
        let starlight_factor = lit_lum / base_lum;
        
        // Tone-map diffuse lighting with an energy-conserving soft-knee curve:
        // Prevents daylight starlight from blowing out craters, continents, and atmospheres
        // into solid blinding white, while preserving 100% of the procedural terrain detail.
        let diffuse_intensity = clamp(starlight_factor / (starlight_factor + 0.95) * 1.05, 0.0, 1.0);
        let diffuse_lit = base_col * diffuse_intensity;
        
        // Extract ocean / ice specular highlight and tone-map smoothly
        let specular_raw = max(lit.rgb - base_col * starlight_factor, vec3<f32>(0.0));
        let specular_glint = specular_raw / (specular_raw + vec3<f32>(1.2)) * 0.65;
        
        let balanced_lit = diffuse_lit + specular_glint;
        
        let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0);
        let fresnel = pow(1.0 - NdotV, 2.8);
        let ambient_boost = base_col * 0.08;
        
        // Rayleigh & Mie atmospheric scattering with golden/crimson sunset terminators (planets only)
        var atmospheric_haze = vec3<f32>(0.0);
        if (planet.planet_type != 4u && (pressure_bar > 0.02 || cloud_density > 0.05 || planet.planet_type == 1u || planet.planet_type == 2u || planet.planet_type == 6u)) {
            let NdotL = dot(pbr_input.N, pbr_input.V); // Grazing light angle
            let twilight = exp(-NdotL * NdotL * 14.0); // Concentrated at terminator
            
            let sunset_hue = vec3<f32>(1.0, 0.42, 0.12);
            var day_rayleigh = vec3<f32>(0.30, 0.68, 1.0);
            if (planet.planet_type == 6u) {
                // Brilliant, radiant cyan-cerulean atmospheric halo for Super-Earths
                day_rayleigh = vec3<f32>(0.22, 0.72, 1.0);
            } else if (planet.planet_type == 2u) {
                day_rayleigh = vec3<f32>(0.20, 0.70, 1.0);
            } else if (temp >= 380.0) {
                day_rayleigh = vec3<f32>(0.95, 0.75, 0.35);
            }
            
            let limb_scatter = mix(day_rayleigh, sunset_hue, twilight * 0.75);
            let haze_scale = clamp(pressure_bar * 0.4 + 0.35, 0.25, 1.3);
            atmospheric_haze = limb_scatter * fresnel * haze_scale;
        }
        
        // Polar Auroral Curtains (Night-side magnetic excitation)
        var aurora_glow = vec3<f32>(0.0);
        if (mag_gauss > 0.15 && abs(lat) > 0.72 && NdotV < 0.6) {
            let aurora_wave = fbm(p_tilted * 10.0 + vec3<f32>(t * 0.25, 0.0, t * 0.20));
            if (aurora_wave > 0.55) {
                let emerald_curtain = vec3<f32>(0.15, 0.95, 0.40) * (aurora_wave - 0.55) * 3.0;
                aurora_glow = emerald_curtain * clamp(mag_gauss, 0.1, 1.5);
            }
        }
        
        out.color = vec4<f32>(balanced_lit + ambient_boost + atmospheric_haze + aurora_glow, 1.0);
    }
    
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
