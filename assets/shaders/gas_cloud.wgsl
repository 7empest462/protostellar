// WGSL Volumetric Nebular Gas Cloud & Turbulent Flow Shader

#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_vertex_output::MeshVertexOutput

struct GasUniforms {
    time_data: vec4<f32>,   // x: time, y: inner_r, z: outer_r, w: gas_scale
    star_params: vec4<f32>, // x: radius, y: temp, z: lum, w: shockwave_r
};

@group(2) @binding(0) var<uniform> gas_time: vec4<f32>;
@group(2) @binding(1) var<uniform> star_params: vec4<f32>;

// Hash & Noise Functions for Procedural Volumetric Turbulence
fn hash(p: vec2<f32>) -> f32 {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(dot(q, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash(i + vec2<f32>(0.0, 0.0));
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v: f32 = 0.0;
    var a: f32 = 0.5;
    var shift = vec2<f32>(100.0, 100.0);
    var pos = p;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + a * noise(pos);
        pos = pos * 2.0 + shift;
        a = a * 0.5;
    }
    return v;
}

@fragment
fn fragment(
    in: MeshVertexOutput,
) -> @location(0) vec4<f32> {
    let pos_world = in.world_position.xyz;
    let r_cyl = length(pos_world.xz);

    let inner_r = gas_time.y;
    let outer_r = gas_time.z;
    let gas_density_scale = gas_time.w;
    let time = gas_time.x;

    // 1. Smooth Radial Boundaries (low < high for WGSL smoothstep specification)
    let inner_fade = smoothstep(inner_r * 0.6, inner_r * 2.0, r_cyl);
    let outer_fade = 1.0 - smoothstep(outer_r * 0.70, outer_r, r_cyl);
    let radial_mask = inner_fade * outer_fade;

    if (radial_mask < 0.001) {
        discard;
    }

    let shockwave_r = star_params.w;
    var shock_mask: f32 = 1.0;
    if (shockwave_r > 0.0) {
        shock_mask = smoothstep(shockwave_r * 0.85, shockwave_r * 1.35, r_cyl);
    }

    // 2. Differential Keplerian Swirling Flow with Domain Warping
    let angle = atan2(pos_world.z, pos_world.x);
    let v_k_rot = 0.4 * time * pow(max(r_cyl, 0.5), -0.75);
    let rot_angle = angle + v_k_rot;

    let spiral_uv = vec2<f32>(
        r_cyl * cos(rot_angle) * 0.12,
        r_cyl * sin(rot_angle) * 0.12
    );

    // Multi-octave domain warping for wispy translucent filaments
    let q = vec2<f32>(
        fbm(spiral_uv + vec2<f32>(time * 0.015, time * 0.008)),
        fbm(spiral_uv + vec2<f32>(4.3, 1.7) + vec2<f32>(-time * 0.01, time * 0.018))
    );
    let r_warp = vec2<f32>(
        fbm(spiral_uv + 3.0 * q + vec2<f32>(1.5, 8.2)),
        fbm(spiral_uv + 3.0 * q + vec2<f32>(7.8, 2.3))
    );
    let wisps = fbm(spiral_uv + 2.5 * r_warp);

    // 3. Vertical Gaussian Thickness Scale Height
    let h_scale = 0.045 * pow(max(r_cyl, 0.5), 1.25);
    let vert_falloff = exp(-pow(pos_world.y / max(h_scale, 0.06), 2.0));

    // 4. Ethereal, Translucent Spectral Color Palette
    let temp = 280.0 * pow(max(r_cyl, 0.2), -0.5);
    var color = vec3<f32>(0.20, 0.45, 0.92); // Deep celestial blue

    if (temp > 700.0) {
        color = vec3<f32>(1.0, 0.65, 0.28); // Warm sunlit peach/amber
    } else if (temp > 280.0) {
        color = vec3<f32>(0.95, 0.80, 0.45); // Warm golden mist
    } else if (temp > 140.0) {
        color = vec3<f32>(0.30, 0.85, 0.88); // Ethereal turquoise / ice line
    } else if (r_cyl > 45.0) {
        color = vec3<f32>(0.38, 0.26, 0.75); // Outer deep violet / lavender
    }

    // 5. Very Wispy, Lite, See-Through Alpha Transparency
    let density = (wisps * 0.50 + 0.20) * vert_falloff * radial_mask * shock_mask * gas_density_scale;
    let alpha = clamp(density * 0.45, 0.0, 0.35); // Delicate see-through nebular mist

    if (alpha < 0.005) {
        discard;
    }

    // Luminous celestial glow that softly illuminates without blocking dust particles
    return vec4<f32>(color * (1.15 + wisps * 0.5), alpha);
}
