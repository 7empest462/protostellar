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
    let inner_fade = smoothstep(inner_r * 0.5, inner_r * 1.8, r_cyl);
    let outer_fade = 1.0 - smoothstep(outer_r * 0.75, outer_r * 1.15, r_cyl);
    let radial_mask = inner_fade * outer_fade;

    if (radial_mask < 0.005 || gas_density_scale < 0.01) {
        discard;
    }

    let shockwave_r = gas.star_params.w;
    var shock_mask: f32 = 1.0;
    if (shockwave_r > 0.0) {
        shock_mask = smoothstep(shockwave_r * 0.85, shockwave_r * 1.35, r_cyl);
    }

    // 2. Differential Keplerian Swirling Flow with Spiral Filaments
    let angle = atan2(pos_world.z, pos_world.x);
    let v_k_rot = 0.35 * time * pow(max(r_cyl, 0.5), -0.75);
    let rot_angle = angle + v_k_rot;

    let spiral_uv = vec2<f32>(
        r_cyl * cos(rot_angle) * 0.18,
        r_cyl * sin(rot_angle) * 0.18
    );

    // Multi-octave domain warping for wispy translucent nebular ribbons
    let q = vec2<f32>(
        fbm2(spiral_uv + vec2<f32>(time * 0.02, time * 0.01)),
        fbm2(spiral_uv + vec2<f32>(4.3, 1.7) + vec2<f32>(-time * 0.015, time * 0.025))
    );
    let r_warp = vec2<f32>(
        fbm2(spiral_uv + 2.5 * q + vec2<f32>(1.5, 8.2)),
        fbm2(spiral_uv + 2.5 * q + vec2<f32>(7.8, 2.3))
    );
    let wisps = fbm2(spiral_uv + 2.0 * r_warp);

    // 3. Ethereal Spectral Color Palette (matching astrophysical temperature & composition)
    let temp = 280.0 * pow(max(r_cyl, 0.4), -0.5);
    var color = vec3<f32>(0.22, 0.55, 0.95); // Deep celestial cyan-blue

    if (temp > 450.0) {
        // Inner terrestrial zone: Warm golden sunlit peach
        color = vec3<f32>(1.0, 0.72, 0.35);
    } else if (temp > 200.0) {
        // Transition / snow line: Ethereal glowing turquoise
        color = vec3<f32>(0.35, 0.90, 0.85);
    } else if (r_cyl > 25.0) {
        // Outer giant & Kuiper zone: Deep cosmic violet / lavender
        color = vec3<f32>(0.45, 0.35, 0.85);
    }

    // 4. Very Wispy, Ethereal, Translucent Alpha Visibility
    let density = (wisps * 0.65 + 0.35) * radial_mask * shock_mask * gas_density_scale;
    let alpha = clamp(density * 0.28, 0.0, 0.38); // Distinct yet see-through mist

    if (alpha < 0.01) {
        discard;
    }

    let emissive_glow = color * (1.20 + wisps * 0.8);
    out.color = vec4<f32>(emissive_glow, alpha);
    return out;
}
