// PROTOSTELLAR // Geodesic Hexagonal Magnetospheric & Planetary Defense Energy Shield Shader
// Triplanar hexagonal lattice (polar-pinch-free), sharp Fresnel edge falloff,
// traveling magnetic field harmonics, and near-clear center so the planet stays crisp inside.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
};

struct PlanetaryShieldUniforms {
    color_and_overdrive: vec4<f32>, // xyz: primary shield cyan-blue, w: overdrive boost (0..1)
    params: vec4<f32>,              // x: time, y: hex_density, z: base_rim_alpha, w: shimmer_amp
    star_dir_and_pulse: vec4<f32>,  // xyz: unit star direction, w: wave speed
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> shield: PlanetaryShieldUniforms;

fn hex_border_dist(p: vec2<f32>) -> f32 {
    let r = vec2<f32>(1.0, 1.7320508);
    let h = r * 0.5;
    let a = (p / r - floor(p / r) - 0.5) * r;
    let b = ((p - h) / r - floor((p - h) / r) - 0.5) * r;
    let gv = select(b, a, dot(a, a) < dot(b, b));
    let edge_d = max(abs(gv.x), dot(abs(gv), normalize(vec2<f32>(1.0, 1.7320508))));
    return 0.5 - edge_d;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let n = normalize(in.world_normal);
    let v = normalize(view.world_position - in.world_position.xyz);
    let ndotv = clamp(abs(dot(n, v)), 0.0, 1.0);
    let fresnel = pow(1.0 - ndotv, 2.75);

    // Triplanar projection aligned with local sphere normal (computed outside branches for derivatives)
    let density = max(shield.params.y, 6.0);
    let abs_n = pow(abs(n), vec3<f32>(6.0));
    let w_tri = abs_n / max(abs_n.x + abs_n.y + abs_n.z, 1e-4);

    let hx_yz = hex_border_dist(n.yz * density);
    let hx_xz = hex_border_dist(n.xz * density);
    let hx_xy = hex_border_dist(n.xy * density);
    let hex_dist = hx_yz * w_tri.x + hx_xz * w_tri.y + hx_xy * w_tri.z;

    let fw = fwidth(n.x * density) + fwidth(n.y * density) + fwidth(n.z * density);
    let hex_aa = clamp(0.038 + fw * 0.35, 0.038, 0.14);
    let hex_wire = 1.0 - smoothstep(0.015, 0.015 + hex_aa, hex_dist);

    // Traveling pole-to-pole magnetic harmonic ripple + sun-facing bow-shock brightening
    let t = shield.params.x;
    let wave_phase = n.y * 14.0 - t * shield.star_dir_and_pulse.w;
    let harmonic = pow(0.5 + 0.5 * sin(wave_phase), 3.0);

    let star_dir = normalize(select(vec3<f32>(0.0, 1.0, 0.0), shield.star_dir_and_pulse.xyz, dot(shield.star_dir_and_pulse.xyz, shield.star_dir_and_pulse.xyz) > 0.01));
    let sunward = smoothstep(-0.2, 0.9, dot(n, star_dir));

    let overdrive = clamp(shield.color_and_overdrive.w, 0.0, 1.0);
    let base_col = shield.color_and_overdrive.rgb;
    let hot_col = mix(base_col, vec3<f32>(0.65, 0.96, 1.45), overdrive * 0.65 + harmonic * 0.35);

    let rim_alpha = fresnel * (shield.params.z + 0.22 * overdrive + 0.16 * sunward);
    let lattice_alpha = hex_wire * (0.14 + 0.48 * fresnel + 0.28 * harmonic * shield.params.w + 0.25 * overdrive);
    let alpha = clamp(rim_alpha + lattice_alpha, 0.0, 0.78);

    if (alpha < 0.008) {
        discard;
    }

    let radiance = (1.15 + 1.25 * fresnel + 0.85 * hex_wire + 0.95 * overdrive) * (0.85 + 0.35 * harmonic);
    out.color = vec4<f32>(hot_col * radiance, alpha);
    return out;
}
