// WGSL Soft Protoplanetary Particle Billboard & Circular Falloff Shader

import bevy_pbr::render::forward_io::VertexOutput;

struct SoftParticleUniforms {
    // x: global_size, y: soft_factor, z: alpha_scale, w: intensity
    params: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> material: SoftParticleUniforms;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    // Center UVs from [0.0, 1.0] -> [-1.0, 1.0]
    let uv_centered = (in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
    let dist_sq = dot(uv_centered, uv_centered);

    if (dist_sq > 1.0) {
        discard;
    }

    // Smooth Gaussian falloff with bright circular core
    let falloff = exp(-2.2 * dist_sq);
    let edge_fade = 1.0 - smoothstep(0.7, 1.0, dist_sq);
    let glow = falloff * edge_fade;

@if(VERTEX_COLORS)
    let v_color = in.color;
@else
    let v_color = vec4<f32>(1.0);

    let alpha_scale = max(material.params.z, 0.1);
    let final_alpha = clamp(v_color.a * glow * alpha_scale, 0.0, 1.0);

    if (final_alpha < 0.008) {
        discard;
    }

    let intensity = max(material.params.w, 0.5);
    let final_rgb = v_color.rgb * (0.85 + 0.45 * glow) * intensity;

    return vec4<f32>(final_rgb, final_alpha);
}
