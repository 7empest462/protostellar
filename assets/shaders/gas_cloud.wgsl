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

    // 1. Smooth Radial Boundaries
    let inner_fade = smoothstep(inner_r * 0.7, inner_r * 1.5, r_cyl);
    let outer_fade = 1.0 - smoothstep(outer_r * 0.80, outer_r * 1.15, r_cyl);
    let radial_mask = inner_fade * outer_fade;

    if (radial_mask < 0.005 || gas_density_scale < 0.005) {
        discard;
    }

    // 2. Seamless Continuous Cartesian Keplerian Swirling Flow (100% Smooth, Zero Seams)
    let v_k_rot = 0.28 * time * pow(max(r_cyl, 0.4), -0.75);
    let cos_a = cos(v_k_rot);
    let sin_a = sin(v_k_rot);

    // Continuous 2D rotational mapping in Cartesian space (completely eliminates branch cut seams)
    let p_rot = vec2<f32>(
        pos_world.x * cos_a - pos_world.z * sin_a,
        pos_world.x * sin_a + pos_world.z * cos_a
    );

    let uv1 = p_rot * 0.12;

    // Multi-octave domain warping for organic wispy translucent nebular filaments
    let q = vec2<f32>(
        fbm2(uv1 + vec2<f32>(time * 0.015, time * 0.008)),
        fbm2(uv1 + vec2<f32>(4.3, 1.7) + vec2<f32>(-time * 0.01, time * 0.018))
    );
    let r_warp = vec2<f32>(
        fbm2(uv1 + 2.2 * q + vec2<f32>(1.5, 8.2)),
        fbm2(uv1 + 2.2 * q + vec2<f32>(7.8, 2.3))
    );
    let wisps = fbm2(uv1 + 1.9 * r_warp);

    // 3. Dynamic Annular Planetary Gap Clearing
    // Giant planets carve dark orbital lanes through the gas disk
    let jupiter_gap = smoothstep(0.0, 1.6, abs(r_cyl - 8.5));
    let saturn_gap = smoothstep(0.0, 2.0, abs(r_cyl - 15.5));
    let uranus_gap = smoothstep(0.0, 2.4, abs(r_cyl - 28.0));
    let neptune_gap = smoothstep(0.0, 2.8, abs(r_cyl - 40.0));
    let gap_clearance = clamp(jupiter_gap * saturn_gap * uranus_gap * neptune_gap, 0.15, 1.0);

    // 4. Stellar Wind & Outward Gas Push Compression Wave
    // When the star ignites, radiation sweeps the inner terrestrial zone (r < 2.7 AU),
    // pushing gas outward into a dense swept-up compression ring feeding Jupiter & Saturn (r ~ 4.5 - 10.0 AU).
    let shockwave_r = gas.star_params.w;
    var inner_clear_factor: f32 = 1.0;
    if (shockwave_r > 0.0) {
        if (r_cyl < 2.7) {
            inner_clear_factor = 0.05; // Residual thin atmosphere gas in terrestrial zone
        } else if (r_cyl < shockwave_r + 5.0) {
            // Outward pushed gas compression wave feeding Jupiter and Saturn
            let compression = 1.0 + 1.4 * exp(-pow((r_cyl - (shockwave_r + 1.5)) / 2.5, 2.0));
            inner_clear_factor = compression;
        }
    }

    // 5. Ethereal Spectral Color Palette (matching astrophysical temperature & composition)
    let temp = 280.0 * pow(max(r_cyl, 0.4), -0.5);
    var color = vec3<f32>(0.20, 0.55, 0.95); // Celestial cyan-azure

    if (r_cyl < 25.0) {
        // Inner zone: Warm sunlit golden luminescence (Little Red Dot inner yellow)
        color = vec3<f32>(1.0, 0.72, 0.38);
    } else if (temp > 180.0 || r_cyl < 55.0) {
        // Mid giant zone: Ethereal glowing turquoise
        color = vec3<f32>(0.32, 0.88, 0.82);
    } else {
        // Outer giant & Kuiper zone: Deep cosmic violet / lavender
        color = vec3<f32>(0.42, 0.32, 0.82);
    }

    // 6. 3D Volumetric Flared Scale-Height Vertical Attenuation
    let h_scale = max(0.08 * pow(r_cyl, 1.15), 0.18);
    let y_dist = abs(pos_world.y);
    let vertical_falloff = exp(-0.5 * pow(y_dist / h_scale, 2.0));

    // 7. Very Wispy, Delicate, Translucent Alpha Visibility
    let density = (wisps * 0.55 + 0.20) * radial_mask * gap_clearance * inner_clear_factor * gas_density_scale * vertical_falloff;
    let alpha = clamp(density * 0.18, 0.0, 0.26); // Translucent see-through mist

    if (alpha < 0.003) {
        discard;
    }

    let emissive_glow = color * (1.10 + wisps * 0.5);
    out.color = vec4<f32>(emissive_glow, alpha);
    return out;
}
