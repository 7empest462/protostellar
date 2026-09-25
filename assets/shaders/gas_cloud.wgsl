// WGSL Volumetric Nebular Gas Cloud & Turbulent Flow Shader

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

struct GasUniforms {
    time_data: vec4<f32>,   // x: time, y: inner_r, z: outer_r, w: gas_scale
    star_params: vec4<f32>, // x: radius, y: temp, z: lum, w: shockwave_r
};

@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var<uniform> gas: GasUniforms;

// Hash & Noise Functions for Procedural Volumetric Turbulence
fn hash2(p: vec2<f32>) -> f32 {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(dot(q, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash2(i + vec2<f32>(0.0, 0.0));
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var v: f32 = 0.0;
    var a: f32 = 0.5;
    var shift = vec2<f32>(100.0, 100.0);
    var pos = p;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + a * noise2(pos);
        pos = pos * 2.0 + shift;
        a = a * 0.5;
    }
    return v;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var out: FragmentOutput;

    let pos_world = in.world_position.xyz;
    let r_cyl = length(pos_world.xz);

    let inner_r = gas.time_data.y;
    let outer_r = gas.time_data.z;
    let gas_density_scale = gas.time_data.w;
    let time = gas.time_data.x;

    let is_massive = outer_r > 100.0 || gas.star_params.x > 10.0;

    // 1. Smooth Radial Boundaries
    let inner_fade = select(
        smoothstep(inner_r * 0.70, inner_r * 1.35, r_cyl),
        smoothstep(inner_r * 0.88, inner_r * 1.04, r_cyl),
        is_massive
    );
    let outer_fade = select(
        1.0 - smoothstep(outer_r * 0.80, outer_r * 1.15, r_cyl),
        1.0 - smoothstep(outer_r * 0.82, outer_r * 1.04, r_cyl),
        is_massive
    );
    let radial_mask = inner_fade * outer_fade;

    if (radial_mask < 0.005 || gas_density_scale < 0.005) {
        discard;
    }

    // 2. Gravitational Differential Keplerian Swirling & Frame-Dragging Vortex Flow
    // In a massive black hole system, inner gas swirls much faster than outer gas:
    // Omega(r) ~ r^-0.75 in projected space, creating intense spiral winding and frame dragging.
    let omega = select(
        0.35 * pow(max(r_cyl, 0.4), -0.75),
        0.85 * pow(max(r_cyl / 6.0, 0.5), -0.75),
        is_massive
    );
    let v_k_rot = time * omega;
    let cos_a = cos(v_k_rot);
    let sin_a = sin(v_k_rot);

    // Continuous 2D rotational mapping in Cartesian space (completely eliminates branch cut seams)
    let p_rot = vec2<f32>(
        pos_world.x * cos_a - pos_world.z * sin_a,
        pos_world.x * sin_a + pos_world.z * cos_a
    );

    // Logarithmic gravitational spiral density waves and frame-dragging twist across the disk:
    let angle_rot = atan2(p_rot.y, p_rot.x);
    let spiral_k = select(1.5, 2.6, is_massive);
    let spiral_phase = angle_rot - spiral_k * log(max(r_cyl, 0.4));
    // 2-armed logarithmic gravitational spiral density wave (smooth, 2pi-periodic, zero seams)
    let grav_spiral = 0.5 + 0.5 * cos(2.0 * spiral_phase);

    let uv_scale = select(0.12, 0.045, is_massive);
    // Displace UVs along the logarithmic spiral twist so filaments stretch along gravitational field lines
    let uv1 = (p_rot + vec2<f32>(cos(spiral_phase), sin(spiral_phase)) * (1.2 * grav_spiral)) * uv_scale;

    // Multi-octave domain warping for organic wispy translucent nebular filaments
    let q = vec2<f32>(
        fbm2(uv1 + vec2<f32>(time * 0.020, time * 0.012)),
        fbm2(uv1 + vec2<f32>(4.3, 1.7) + vec2<f32>(-time * 0.015, time * 0.022))
    );
    let r_warp = vec2<f32>(
        fbm2(uv1 + 2.2 * q + vec2<f32>(1.5, 8.2)),
        fbm2(uv1 + 2.2 * q + vec2<f32>(7.8, 2.3))
    );
    let base_wisps = fbm2(uv1 + 1.9 * r_warp);
    let wisps = mix(base_wisps, base_wisps * (0.55 + 0.85 * grav_spiral), select(0.35, 0.70, is_massive));

    // 3. Dynamic Annular Planetary Gap Clearing
    var gap_clearance: f32 = 1.0;
    if (is_massive) {
        // Circum-nuclear disk resonant gaps carved by S-Cluster stars, black hole, and primordial worlds:
        // Micro-Quasar alpha (88 AU), Star alpha (120 AU), Prime-b (155 AU), Star beta (190 AU), Prime-c (225 AU), Star gamma (260 AU)
        let g1 = smoothstep(0.0, 4.5, abs(r_cyl - 88.0));
        let g2 = smoothstep(0.0, 5.0, abs(r_cyl - 120.0));
        let g3 = smoothstep(0.0, 4.0, abs(r_cyl - 155.0));
        let g4 = smoothstep(0.0, 4.5, abs(r_cyl - 190.0));
        let g5 = smoothstep(0.0, 3.5, abs(r_cyl - 225.0));
        let g6 = smoothstep(0.0, 4.0, abs(r_cyl - 260.0));
        gap_clearance = clamp(g1 * g2 * g3 * g4 * g5 * g6, 0.20, 1.0);
    } else {
        // Solar system planetary gaps
        let jupiter_gap = smoothstep(0.0, 1.6, abs(r_cyl - 8.5));
        let saturn_gap = smoothstep(0.0, 2.0, abs(r_cyl - 15.5));
        let uranus_gap = smoothstep(0.0, 2.4, abs(r_cyl - 28.0));
        let neptune_gap = smoothstep(0.0, 2.8, abs(r_cyl - 40.0));
        gap_clearance = clamp(jupiter_gap * saturn_gap * uranus_gap * neptune_gap, 0.15, 1.0);
    }

    // 4. Stellar Wind & Outward Gas Push Compression Wave
    let shockwave_r = gas.star_params.w;
    var inner_clear_factor: f32 = 1.0;
    if (shockwave_r > 0.0) {
        if (!is_massive) {
            let clear_edge = min(shockwave_r, 2.7);
            if (r_cyl < clear_edge) {
                inner_clear_factor = clamp((r_cyl / max(clear_edge, 0.01)) * 0.4, 0.05, 0.4);
            } else if (r_cyl < shockwave_r + 5.0) {
                let compression = 1.0 + 1.4 * exp(-pow((r_cyl - (shockwave_r + 1.5)) / 2.5, 2.0));
                inner_clear_factor = compression;
            }
        } else {
            if (r_cyl < shockwave_r) {
                inner_clear_factor = 0.02;
            } else if (r_cyl < shockwave_r + 15.0) {
                let compression = 1.0 + 1.8 * exp(-pow((r_cyl - (shockwave_r + 5.0)) / 8.0, 2.0));
                inner_clear_factor = compression;
            }
        }
    }

    // 5. Ethereal Spectral Color Palette (matching astrophysical temperature & composition)
    var color: vec3<f32>;
    if (is_massive) {
        // JWST Little Red Dot Primordial Infall & Circum-Nuclear Disk Colors:
        let span = max(outer_r - inner_r, 1.0);
        let r_norm = clamp((r_cyl - inner_r) / span, 0.0, 1.0);

        if (r_norm < 0.20) {
            // Inner Accretion Stream: Ruby-amber & sunlit golden accretion glow
            let t = r_norm / 0.20;
            color = mix(vec3<f32>(1.0, 0.45, 0.18), vec3<f32>(1.0, 0.82, 0.35), t);
        } else if (r_norm < 0.58) {
            // Mid Circumstellar Reservoir: Luminous glowing turquoise & radiant emerald-cyan
            let t = (r_norm - 0.20) / 0.38;
            color = mix(vec3<f32>(0.28, 0.92, 0.82), vec3<f32>(0.20, 0.65, 0.98), t);
        } else {
            // Outer Gas Reservoir: Radiant celestial azure & deep cosmic violet
            let t = (r_norm - 0.58) / 0.42;
            color = mix(vec3<f32>(0.32, 0.52, 0.98), vec3<f32>(0.55, 0.35, 0.92), t);
        }
    } else {
        let temp = 280.0 * pow(max(r_cyl, 0.4), -0.5);
        if (r_cyl < 25.0) {
            color = vec3<f32>(1.0, 0.72, 0.38);
        } else if (temp > 180.0 || r_cyl < 55.0) {
            color = vec3<f32>(0.32, 0.88, 0.82);
        } else {
            color = vec3<f32>(0.42, 0.32, 0.82);
        }
    }

    // 6. 3D Volumetric Flared Scale-Height Vertical Attenuation
    let h_scale = select(
        max(0.08 * pow(r_cyl, 1.15), 0.18),
        max(0.038 * r_cyl, 1.5),
        is_massive
    );
    let y_dist = abs(pos_world.y);
    let vertical_falloff = exp(-0.5 * pow(y_dist / h_scale, 2.0));

    // 7. Dynamic Alpha & Luminous Visibility
    let base_density = (wisps * 0.55 + 0.25) * radial_mask * gap_clearance * inner_clear_factor * gas_density_scale * vertical_falloff;
    let alpha_mul = select(0.18, 0.26, is_massive);
    let alpha_cap = select(0.26, 0.38, is_massive);
    let alpha = clamp(base_density * alpha_mul, 0.0, alpha_cap);

    if (alpha < 0.003) {
        discard;
    }

    // Relativistic Doppler beaming asymmetry across the accretion disk
    let doppler = select(1.0, 1.0 + 0.18 * (p_rot.x / max(r_cyl, 1.0)), is_massive);
    let emissive_boost = select(1.10, 1.30, is_massive);
    let emissive_glow = color * (emissive_boost + wisps * 0.65) * doppler;
    out.color = vec4<f32>(emissive_glow, alpha);
    return out;
}
