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

    // 1. Boundary & Shockwave Clearance
    if (r_cyl < inner_r || r_cyl > outer_r) {
        discard;
    }

    let shockwave_r = star_params.w;
    if (shockwave_r > 0.0 && r_cyl < shockwave_r) {
        discard; // Blown away by stellar ignition wind
    }

    // 2. Multi-Octave Keplerian Swirling Noise
    let angle = atan2(pos_world.z, pos_world.x);
    let v_k_rot = 1.2 * time * pow(r_cyl / 1.0, -0.75);
    let rot_angle = angle + v_k_rot;

    let sample_pos = vec2<f32>(
        r_cyl * cos(rot_angle) * 0.8,
        r_cyl * sin(rot_angle) * 0.8
    );

    let turb = fbm(sample_pos + vec2<f32>(time * 0.1, time * 0.05));

    // 3. Vertical Scale Height Falloff H(r) = 0.035 * r^1.25
    let h_scale = 0.035 * pow(r_cyl, 1.25);
    let vert_falloff = exp(-pow(pos_world.y / max(h_scale, 0.05), 2.0));

    // 4. Temperature-Based Color Gradient
    let temp = 280.0 * pow(r_cyl, -0.5);
    var color = vec3<f32>(0.2, 0.4, 0.8); // Cold outer blue

    if (temp > 800.0) {
        color = vec3<f32>(1.0, 0.5, 0.15); // Hot glowing inner orange
    } else if (temp > 300.0) {
        color = vec3<f32>(0.9, 0.7, 0.3); // Warm amber
    } else if (temp > 150.0) {
        color = vec3<f32>(0.4, 0.7, 0.9); // Cyan ice line
    }

    let density = turb * vert_falloff * gas_density_scale;
    let alpha = clamp(density * 0.45, 0.0, 0.75);

    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(color * (1.0 + turb * 0.5), alpha);
}
