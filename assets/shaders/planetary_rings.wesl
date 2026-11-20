import bevy_pbr::render::{
    mesh_view_bindings::view,
    forward_io::VertexOutput,
};

struct RingUniforms {
    inner_radius: f32,
    outer_radius: f32,
    optical_depth: f32,
    ice_fraction: f32,
    ring_color: vec4<f32>,
    star_dir_local: vec4<f32>,
    shadow_params: vec4<f32>, // x = ambient_floor, y = penumbra_width, z = shadow_depth, w = elapsed_secs
    star_dir_world_and_oblateness: vec4<f32>, // xyz = unit star dir in world space, w = planet oblateness
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> ring: RingUniforms;

const PI: f32 = 3.14159265359;

fn hash11(p: f32) -> f32 {
    let p3 = fract(p * 0.1031);
    let p4 = p3 * (p3 + 33.33);
    return fract((p3 + p4) * p4);
}

fn ring_noise(r: f32) -> f32 {
    let i = floor(r);
    let f = fract(r);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(hash11(i), hash11(i + 1.0), u);
}

fn fbm_ring(r: f32) -> f32 {
    var val = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var i = 0; i < 4; i++) {
        val += ring_noise(r * freq) * amp;
        freq *= 2.3;
        amp *= 0.45;
    }
    return val;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Local planar model coordinates
    let local_uv = in.uv * 2.0 - 1.0;
    let dist = length(local_uv);

    // Inner boundary: cannot be closer than the planet surface (ring.star_dir_local.w)
    let planet_surface_dist = ring.star_dir_local.w;
    let inner_dist = clamp(max(ring.inner_radius, planet_surface_dist), 0.10, 0.95);
    let outer_dist = 1.0;

    // Strictly discard all pixels outside the outer circle or inside the inner ring boundary
    if (dist > outer_dist || dist < inner_dist) {
        discard;
    }

    // Normalized radial ring coordinate from inner (0.0) to outer (1.0)
    let u = (dist - inner_dist) / max(outer_dist - inner_dist, 0.001);
    if (u < 0.0 || u > 1.0) {
        discard;
    }

    let theta = atan2(local_uv.y, local_uv.x);
    let time = ring.shadow_params.w;

    // View elevation & screen-space derivative LOD to eliminate grazing-angle Moiré speckles
    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let n_world = normalize(in.world_normal);
    let view_elev = dot(view_dir, n_world);
    let graze_lod = smoothstep(0.06, 0.28, abs(view_elev));

    // Micro-ringlet density modulation (vinyl-like concentric fine tracks) with screen-space derivative LOD
    let r_deriv_fine = fwidth(u * 290.0);
    let r_deriv_med = fwidth(u * 96.0);
    let micro_lod = (1.0 - smoothstep(0.42, 1.55, r_deriv_fine) * 0.88) * graze_lod;
    let med_lod = (1.0 - smoothstep(0.45, 1.60, r_deriv_med) * 0.80) * (0.25 + 0.75 * graze_lod);

    let fine_tracks = mix(0.5, fbm_ring(u * 290.0), micro_lod);
    let med_tracks = mix(0.5, fbm_ring(u * 96.0), med_lod);
    let broad_bands = mix(0.5, fbm_ring(u * 30.0), 0.40 + 0.60 * graze_lod);
    let groove_wave = mix(
        0.5,
        0.5 + 0.32 * sin(u * 185.0) + 0.18 * sin(u * 410.0 + 1.3),
        micro_lod
    );

    // Lindblad Resonance Spiral Density Waves (m=2 and m=5 tightly wound logarithmic spirals)
    let log_u = log(max(u + 0.15, 0.15));
    let spiral_m2 = sin(2.0 * theta - 95.0 * log_u - time * 0.35) * 0.5 + 0.5;
    let spiral_m5 = sin(5.0 * theta - 175.0 * log_u - time * 0.65) * 0.5 + 0.5;
    let spiral_wave = (spiral_m2 * 0.6 + spiral_m5 * 0.4) * micro_lod;

    var base_density = 0.0;
    var ring_tone = vec3<f32>(0.88, 0.80, 0.62);
    var fine_dust_fraction = 0.15;

    // 1. C Ring (Translucent Inner Crepe Ring: 0.00 to 0.24)
    if (u < 0.24) {
        let t = u / 0.24;
        var dens_c = mix(0.05, 0.23, smoothstep(0.0, 0.85, t)) * (0.55 + med_tracks * 0.30 + fine_tracks * 0.25);
        // Colombo & Maxwell narrow translucent gaps + eccentric ringlets inside C Ring
        let maxwell_gap = 1.0 - 0.85 * exp(-pow((u - 0.165) / 0.0055, 2.0));
        let colombo_gap = 1.0 - 0.72 * exp(-pow((u - 0.065) / 0.0045, 2.0));
        dens_c *= maxwell_gap * colombo_gap;
        base_density = dens_c;
        ring_tone = mix(vec3<f32>(0.29, 0.23, 0.16), vec3<f32>(0.45, 0.37, 0.25), t * 0.70 + med_tracks * 0.30);
        fine_dust_fraction = 0.52;
    }
    // 2. B Ring (Dense Luminous Main Ring: 0.24 to 0.64)
    else if (u < 0.64) {
        let t = (u - 0.24) / (0.64 - 0.24);
        // Steep inner ramp from C Ring into high-albedo, richly striated B Ring
        let b_envelope = mix(0.64, 1.04, smoothstep(0.0, 0.18, t)) * (0.92 + 0.10 * sin(t * PI));
        // Rich vinyl-groove radial optical depth modulation across B1/B2/B3/B4 zones
        let b_macro = 0.74 + 0.24 * broad_bands + 0.20 * med_tracks + 0.16 * fine_tracks + 0.12 * groove_wave + 0.05 * spiral_wave;
        base_density = b_envelope * b_macro;
        let b_warm_sand = vec3<f32>(0.58, 0.46, 0.26);
        let b_champagne = vec3<f32>(0.92, 0.80, 0.52);
        let b_bright_ivory = vec3<f32>(1.04, 0.95, 0.70);
        let b_tone_mix = clamp(0.22 + t * 0.44 + (broad_bands - 0.5) * 0.62 + (med_tracks - 0.5) * 0.42 + (groove_wave - 0.5) * 0.24, 0.0, 1.0);
        ring_tone = mix(b_warm_sand, mix(b_champagne, b_bright_ivory, smoothstep(0.45, 0.92, b_tone_mix)), b_tone_mix);

        // Keplerian Azimuthal Dusty Spokes (periodic integer harmonics so no branch-cut seam at ±PI)
        if (u > 0.38 && u < 0.62) {
            let omega_k = 0.22 / pow(max(dist, 0.35), 1.5);
            let spoke_az = theta - time * omega_k;
            let spoke_h1 = sin(spoke_az * 6.0 + sin(u * 14.0) * 0.35) * 0.5 + 0.5;
            let spoke_h2 = cos(spoke_az * 11.0 + 2.1) * 0.5 + 0.5;
            let spoke_n = spoke_h1 * spoke_h2;
            let spoke_env = smoothstep(0.38, 0.46, u) * smoothstep(0.62, 0.54, u) * graze_lod;
            let spoke_mask = smoothstep(0.38, 0.74, spoke_n) * spoke_env;
            fine_dust_fraction += spoke_mask * 0.42;
            ring_tone = mix(ring_tone, vec3<f32>(0.84, 0.78, 0.62), spoke_mask * 0.14);
        }
    }
    // 3. Cassini Division (Prominent Sharp 2:1 Mimas Resonance Gap: 0.64 to 0.715)
    else if (u < 0.715) {
        let t = (u - 0.64) / (0.715 - 0.64);
        // Steep razor-crisp edges at B-ring outer boundary and A-ring inner boundary
        let edge_wall = smoothstep(0.0, 0.055, t) * (1.0 - smoothstep(0.945, 1.0, t));
        // Subtle narrow Huygens Ringlet nestled just inside the inner Cassini edge
        let huygens_ringlet = 0.15 * exp(-pow((t - 0.15) / 0.032, 2.0));
        let floor_dust = (1.0 - edge_wall * 0.985) * 0.10 + 0.014 * (0.5 + fine_tracks * 0.5);
        base_density = floor_dust + huygens_ringlet;
        ring_tone = mix(vec3<f32>(0.24, 0.20, 0.14), vec3<f32>(0.64, 0.54, 0.36), clamp(huygens_ringlet * 4.0, 0.0, 1.0));
        fine_dust_fraction = 0.55;
    }
    // 4. A Ring (Outer Main Ring: 0.715 to 0.935)
    else if (u < 0.935) {
        let t = (u - 0.715) / (0.935 - 0.715);
        let a_inner_bright = 1.0 + 0.16 * exp(-pow((t - 0.07) / 0.09, 2.0));
        var density_a = 0.62 * a_inner_bright * (0.68 + med_tracks * 0.24 + fine_tracks * 0.20 + groove_wave * 0.12 + spiral_wave * 0.10);

        // Encke Gap (Pan shepherd moon razor-clean lane around u = 0.865 to 0.879)
        if (u >= 0.864 && u <= 0.879) {
            let encke_t = (u - 0.864) / (0.879 - 0.864);
            let encke_wall = smoothstep(0.0, 0.15, encke_t) * (1.0 - smoothstep(0.85, 1.0, encke_t));
            density_a *= (1.0 - encke_wall * 0.99);
            fine_dust_fraction = 0.60;
        }
        // Keeler Gap (Daphnis shepherd moon lane with sinusoidal edge wakes near outer A edge: u = 0.922 to 0.929)
        let wake_ripple = sin(theta * 18.0 - time * 0.55) * 0.0011 * micro_lod;
        let u_keeler = u + wake_ripple;
        if (u_keeler >= 0.922 && u_keeler <= 0.929) {
            let keeler_t = (u_keeler - 0.922) / (0.929 - 0.922);
            density_a *= (1.0 - sin(keeler_t * PI) * 0.97);
            fine_dust_fraction = 0.65;
        }
        // Crisp outer A-ring edge taper right before the Roche Division
        density_a *= (1.0 - smoothstep(0.930, 0.935, u));
        base_density = density_a;
        let a_taupe = vec3<f32>(0.54, 0.46, 0.32);
        let a_pale = vec3<f32>(0.78, 0.69, 0.49);
        ring_tone = mix(a_taupe, a_pale, clamp(0.28 + broad_bands * 0.50 + (med_tracks - 0.5) * 0.30 + (1.0 - t) * 0.18, 0.0, 1.0));
    }
    // 5. Roche Division (Clean Dark Empty Lane: 0.935 to 0.974)
    else if (u < 0.974) {
        base_density = 0.004 * fine_tracks;
        ring_tone = vec3<f32>(0.28, 0.24, 0.18);
        fine_dust_fraction = 0.75;
    }
    // 6. F Ring / Delicate Gossamer Shepherd-Moon Outer Thread (0.974 to 1.00)
    else {
        let braid = mix(0.88, sin(theta * 9.0 - time * 0.4 + sin(theta * 3.0) * 1.5) * 0.22 + 0.78, graze_lod);
        let f_core = exp(-pow((u - 0.985) / 0.0032, 2.0));
        base_density = f_core * 0.34 * (0.65 + fine_tracks * 0.35) * braid;
        ring_tone = vec3<f32>(0.84, 0.74, 0.52);
        fine_dust_fraction = 0.92;
    }

    // Blend composition: Water Ice (warm champagne-ivory pearl) vs Silicate/Metal (amber dust)
    let ice_col = mix(vec3<f32>(0.80, 0.68, 0.46), vec3<f32>(1.00, 0.96, 0.86), ring.ice_fraction);
    let base_albedo = ring_tone * ice_col * ring.ring_color.rgb;

    // Edge feathering to avoid harsh polygonal borders
    let inner_feather = smoothstep(0.0, 0.025, u);
    let outer_feather = 1.0 - smoothstep(0.994, 1.0, u);
    let tau = base_density * ring.optical_depth * inner_feather * outer_feather;

    // Planetary shadow calculation: Host planet casts razor-sharp oblate umbra & tight penumbra onto ring plane
    var star_dir = ring.star_dir_local.xyz;
    if (dot(star_dir, star_dir) < 0.01) {
        star_dir = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        star_dir = normalize(star_dir);
    }

    // Normalized 3D position of ring fragment in local space: X = local_uv.x, Y = 0, Z = local_uv.y
    let p_ring = vec3<f32>(local_uv.x, 0.0, local_uv.y);

    // Exact ray-ellipsoid shadow test for true oblate spheroid (eq_scale = 1.0, pol_scale = 1.0 - oblate)
    let oblate = clamp(ring.star_dir_world_and_oblateness.w, 0.0, 0.15);
    let pol_scale = max(1.0 - oblate, 0.5);
    let star_dir_ell = normalize(vec3<f32>(star_dir.x, star_dir.y / pol_scale, star_dir.z));
    let s_closest = -dot(p_ring, star_dir_ell);
    var shadow_factor = 1.0;

    if (s_closest > 0.0) {
        let p_closest = p_ring + s_closest * star_dir_ell;
        let d_closest = length(p_closest);

        let planet_rad = max(ring.star_dir_local.w, 0.15);
        let penumbra = max(ring.shadow_params.y, 0.006);
        let umbra_r = max(planet_rad - penumbra, 0.0);
        let penumbra_r = planet_rad + penumbra;

        shadow_factor = smoothstep(umbra_r, penumbra_r, d_closest);
    }

    let ambient_floor = max(ring.shadow_params.x, 0.004);
    let shadow_depth = select(0.995, ring.shadow_params.z, ring.shadow_params.z > 0.0);
    let lit_mult = mix(ambient_floor, 1.0, mix(1.0 - shadow_depth, 1.0, shadow_factor));

    // Camera view vector & phase angle optics (Cassini forward-scattering Mie halo + Seeliger opposition surge)
    let raw_sun_dir = ring.star_dir_world_and_oblateness.xyz;
    var sun_dir_world = vec3<f32>(0.0, 1.0, 0.0);
    if (dot(raw_sun_dir, raw_sun_dir) > 0.01) {
        sun_dir_world = normalize(raw_sun_dir);
    }

    // cos_phase -> +1 when looking through the rings toward the star (high-phase backlighting)
    // cos_phase -> -1 when star is directly behind the camera (zero-phase opposition)
    let cos_phase = dot(-view_dir, sun_dir_world);

    // 1. High-Phase Forward-Scattering Henyey-Greenstein Mie Halo (warm golden Cassini backlit glow)
    let gf = 0.78;
    let gf2 = gf * gf;
    let fwd_denom = max(1.0 + gf2 - 2.0 * gf * cos_phase, 1e-3);
    let hg_forward = ((1.0 - gf2) / (fwd_denom * sqrt(fwd_denom))) * 0.095;
    let forward_trans = exp(-tau * 0.45) * (0.38 + fine_dust_fraction * 0.95);
    let opal_tint = mix(vec3<f32>(0.98, 0.86, 0.64), vec3<f32>(1.04, 0.95, 0.78), ring.ice_fraction);
    let forward_glow = opal_tint * hg_forward * forward_trans * shadow_factor;

    // 2. Seeliger-Hapke Coherent Backscatter Opposition Surge (cos_phase -> -1)
    let back_align = max(-cos_phase, 0.0);
    let opposition_surge = 1.0 + 0.36 * pow(back_align, 6.0) + 0.16 * pow(back_align, 2.0);

    // 3. Lit-face vs Unlit-face Radiative Transfer (3D crystalline ice particles scatter strongly even at moderate solar elevation)
    let same_side = (view_elev * star_dir.y) >= -0.02;
    let unlit_trans = mix(exp(-tau * 0.95) * 0.72 + 0.24, 1.0, select(0.0, 1.0, same_side));

    let sun_elev = abs(star_dir.y);
    let sun_elev_illum = mix(0.42, 0.96, smoothstep(0.02, 0.34, sun_elev));
    let ring_illumination = sun_elev_illum * opposition_surge * unlit_trans;

    // Slant optical depth boosts apparent alpha at grazing view angles (1 / |sin(elevation)|)
    let slant_boost = clamp(1.0 / max(abs(view_elev), 0.28), 1.0, 2.5);
    let alpha = clamp((1.0 - exp(-tau * slant_boost * 1.65)) + length(forward_glow) * 0.30, 0.0, 0.96);

    if (alpha < 0.008) {
        discard;
    }

    let shaded_color = base_albedo * lit_mult * ring_illumination + forward_glow;
    return vec4<f32>(shaded_color, alpha);
}
