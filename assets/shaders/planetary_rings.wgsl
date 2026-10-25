#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::VertexOutput,
}

struct RingUniforms {
    inner_radius: f32,
    outer_radius: f32,
    optical_depth: f32,
    ice_fraction: f32,
    ring_color: vec4<f32>,
    star_dir_local: vec4<f32>,
    shadow_params: vec4<f32>, // x = ambient_floor, y = penumbra_width, z = shadow_depth, w = elapsed_secs
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
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

    // Micro-ringlet density modulation (thousands of concentric fine tracks) with screen-space derivative LOD
    let r_deriv = fwidth(u * 280.0);
    let micro_lod = 1.0 - smoothstep(0.6, 2.2, r_deriv) * 0.65;
    let fine_tracks = mix(0.5, fbm_ring(u * 280.0), micro_lod);
    let broad_bands = fbm_ring(u * 35.0);

    // Lindblad Resonance Spiral Density Waves (m=2 and m=5 tightly wound logarithmic spirals)
    let log_u = log(max(u + 0.15, 0.15));
    let spiral_m2 = sin(2.0 * theta - 95.0 * log_u - time * 0.35) * 0.5 + 0.5;
    let spiral_m5 = sin(5.0 * theta - 175.0 * log_u - time * 0.65) * 0.5 + 0.5;
    let spiral_wave = (spiral_m2 * 0.6 + spiral_m5 * 0.4) * micro_lod;

    var base_density = 0.0;
    var ring_tone = vec3<f32>(0.92, 0.88, 0.82);
    var fine_dust_fraction = 0.15;

    // 1. C Ring (Inner Crepe Ring: 0.00 to 0.22)
    if (u < 0.22) {
        let t = u / 0.22;
        base_density = mix(0.05, 0.35, t) * (0.6 + fine_tracks * 0.4);
        ring_tone = vec3<f32>(0.45, 0.38, 0.30); // translucent amber/charcoal
        fine_dust_fraction = 0.45;
    }
    // 2. B Ring (Dense Bright Main Ring: 0.22 to 0.65)
    else if (u < 0.65) {
        let t = (u - 0.22) / (0.65 - 0.22);
        base_density = mix(0.75, 0.98, smoothstep(0.0, 0.3, t))
            * (0.72 + fine_tracks * 0.30 + broad_bands * 0.14 + spiral_wave * 0.08);
        ring_tone = vec3<f32>(0.96, 0.92, 0.84); // brilliant reflective ice

        // Keplerian Azimuthal Dusty Spokes (electrostatically levitated micron grains in outer B ring)
        if (u > 0.40 && u < 0.63) {
            let omega_k = 0.22 / pow(max(dist, 0.35), 1.5);
            let spoke_az = theta - time * omega_k;
            let spoke_n = ring_noise(spoke_az * 5.5 + sin(u * 14.0) * 0.35) * ring_noise(spoke_az * 13.0 + 7.1);
            let spoke_env = smoothstep(0.40, 0.48, u) * smoothstep(0.63, 0.54, u);
            let spoke_mask = smoothstep(0.32, 0.68, spoke_n) * spoke_env;
            fine_dust_fraction += spoke_mask * 0.55;
            ring_tone = mix(ring_tone, vec3<f32>(0.88, 0.92, 0.98), spoke_mask * 0.25);
        }
    }
    // 3. Cassini Division (Prominent 2:1 Mimas Resonance Gap: 0.65 to 0.72)
    else if (u < 0.72) {
        let t = (u - 0.65) / (0.72 - 0.65);
        let gap_profile = sin(t * PI);
        base_density = (1.0 - gap_profile * 0.94) * 0.18 * (0.4 + fine_tracks * 0.6);
        ring_tone = vec3<f32>(0.25, 0.22, 0.20); // dark transparent lane
        fine_dust_fraction = 0.50;
    }
    // 4. A Ring (Outer Main Ring: 0.72 to 0.96)
    else if (u < 0.96) {
        var density_a = 0.65 * (0.72 + fine_tracks * 0.28 + spiral_wave * 0.12);
        
        // Encke Gap (Pan shepherd moon lane around u = 0.855 to 0.875)
        if (u >= 0.855 && u <= 0.875) {
            let encke_t = (u - 0.855) / (0.875 - 0.855);
            density_a *= (1.0 - sin(encke_t * PI) * 0.92);
            fine_dust_fraction = 0.55;
        }
        // Keeler Gap (Daphnis shepherd moon lane with sinusoidal edge wakes around u = 0.941 to 0.949)
        let wake_ripple = sin(theta * 18.0 - time * 0.55) * 0.0014;
        let u_keeler = u + wake_ripple;
        if (u_keeler >= 0.941 && u_keeler <= 0.949) {
            let keeler_t = (u_keeler - 0.941) / (0.949 - 0.941);
            density_a *= (1.0 - sin(keeler_t * PI) * 0.94);
            fine_dust_fraction = 0.60;
        }
        base_density = density_a;
        ring_tone = vec3<f32>(0.88, 0.84, 0.78);
    }
    // 5. F Ring / Braided Shepherd-Moon Outer Boundary (0.96 to 1.0)
    else {
        let t = (u - 0.96) / (1.0 - 0.96);
        let braid = sin(theta * 9.0 - time * 0.4 + sin(theta * 3.0) * 1.5) * 0.25 + 0.75;
        base_density = (1.0 - t) * 0.38 * (0.45 + fine_tracks * 0.55) * braid;
        ring_tone = vec3<f32>(0.78, 0.82, 0.90);
        fine_dust_fraction = 0.85;
    }

    // Blend composition: Water Ice (bright white/cyan pearl) vs Silicate/Metal (warm dust)
    let ice_col = mix(vec3<f32>(0.85, 0.75, 0.60), vec3<f32>(0.96, 0.97, 1.00), ring.ice_fraction);
    let base_albedo = ring_tone * ice_col * ring.ring_color.rgb;

    // Edge feathering to avoid harsh polygonal borders
    let inner_feather = smoothstep(0.0, 0.04, u);
    let outer_feather = 1.0 - smoothstep(0.96, 1.0, u);
    let tau = base_density * ring.optical_depth * inner_feather * outer_feather;

    // Planetary shadow calculation: Host planet casts umbra and penumbra onto ring plane
    var star_dir = ring.star_dir_local.xyz;
    if (dot(star_dir, star_dir) < 0.01) {
        star_dir = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        star_dir = normalize(star_dir);
    }

    // Normalized 3D position of ring fragment in local space: X = local_uv.x, Y = 0, Z = local_uv.y
    let p_ring = vec3<f32>(local_uv.x, 0.0, local_uv.y);

    // Ray towards central star: R(s) = p_ring + s * star_dir (for s > 0)
    let s_closest = -dot(p_ring, star_dir);
    var shadow_factor = 1.0;

    if (s_closest > 0.0) {
        let p_closest = p_ring + s_closest * star_dir;
        let d_closest = length(p_closest);

        let planet_rad = max(ring.star_dir_local.w, 0.15);
        let penumbra = max(ring.shadow_params.y, 0.022);
        let umbra_r = max(planet_rad - penumbra, 0.0);
        let penumbra_r = planet_rad + penumbra;

        shadow_factor = smoothstep(umbra_r, penumbra_r, d_closest);
    }

    let ambient_floor = max(ring.shadow_params.x, 0.025);
    let shadow_depth = select(0.97, ring.shadow_params.z, ring.shadow_params.z > 0.0);
    let lit_mult = mix(ambient_floor, 1.0, mix(1.0 - shadow_depth, 1.0, shadow_factor));

    // Camera view vector & phase angle optics (Cassini forward-scattering Mie halo + Seeliger opposition surge)
    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let sun_dir_world = select(
        normalize(-in.world_position.xyz),
        vec3<f32>(0.0, 1.0, 0.0),
        dot(in.world_position.xyz, in.world_position.xyz) < 1e-6
    );
    let n_world = normalize(in.world_normal);

    // cos_phase -> +1 when looking through the rings toward the star (high-phase backlighting)
    // cos_phase -> -1 when star is directly behind the camera (zero-phase opposition)
    let cos_phase = dot(-view_dir, sun_dir_world);

    // 1. High-Phase Forward-Scattering Henyey-Greenstein Mie Halo (g = 0.80)
    let gf = 0.80;
    let gf2 = gf * gf;
    let fwd_denom = max(1.0 + gf2 - 2.0 * gf * cos_phase, 1e-3);
    let hg_forward = ((1.0 - gf2) / (fwd_denom * sqrt(fwd_denom))) * 0.075;
    let forward_trans = exp(-tau * 0.55) * (0.35 + fine_dust_fraction * 0.95);
    let opal_tint = mix(vec3<f32>(0.95, 0.88, 0.76), vec3<f32>(0.85, 0.94, 1.12), ring.ice_fraction);
    let forward_glow = opal_tint * hg_forward * forward_trans * shadow_factor;

    // 2. Seeliger-Hapke Coherent Backscatter Opposition Surge (cos_phase -> -1)
    let back_align = max(-cos_phase, 0.0);
    let opposition_surge = 1.0 + 0.52 * pow(back_align, 7.0) + 0.22 * pow(back_align, 2.0);

    // 3. Lit-face vs Unlit-face Radiative Transfer
    let view_elev = dot(view_dir, n_world);
    let same_side = (view_elev * star_dir.y) >= -0.02;
    let unlit_trans = mix(exp(-tau * 1.15) * 0.65 + 0.18, 1.0, select(0.0, 1.0, same_side));

    let sun_elev = abs(star_dir.y);
    let ring_illumination = max(sun_elev, 0.22) * opposition_surge * unlit_trans;

    // Slant optical depth boosts apparent alpha at grazing view angles (1 / |sin(elevation)|)
    let slant_boost = clamp(1.0 / max(abs(view_elev), 0.28), 1.0, 2.4);
    let alpha = clamp((1.0 - exp(-tau * slant_boost * 1.35)) + length(forward_glow) * 0.35, 0.0, 0.94);

    if (alpha < 0.01) {
        discard;
    }

    let shaded_color = base_albedo * lit_mult * ring_illumination + forward_glow;
    return vec4<f32>(shaded_color, alpha);
}
