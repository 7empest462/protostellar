// PROTOSTELLAR // GPU-Billboarded Unlit Soft Particle Swarm Shader
// Expands camera-facing quads in the vertex shader from particle center (`position`),
// unscaled half-radius (`normal.x`), quad corner (`uv`), and linear RGBA (`color`),
// eliminating per-frame 500k CPU billboard rebuilds and PBR lighting overhead.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    mesh_functions,
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
};

struct ParticleSwarmUniforms {
    /// x: global particle render scale multiplier, y/z/w: reserved
    params: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> material: ParticleSwarmUniforms;

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let world_center = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    ).xyz;

    let cam_right = normalize(view.world_from_view[0].xyz);
    let cam_up = normalize(view.world_from_view[1].xyz);

@if(VERTEX_UVS_A)
    let corner = (vertex.uv - vec2<f32>(0.5, 0.5)) * 2.0;
@else
    let corner = vec2<f32>(0.0, 0.0);

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;

    let to_cam = world_center - view.world_position;
    let dist_to_cam = length(to_cam);

@if(VERTEX_NORMALS)
    let raw_half_size = vertex.normal.x * max(material.params.x, 0.0);
@else
    let raw_half_size = 0.0;

    // Prevent particles right in front of a zoomed-in camera from inflating into full-screen
    // high-overdraw quads while preserving crisp angular size for orbital dust belts.
    let half_size = min(raw_half_size, dist_to_cam * 0.022);
    let near_cam_fade = smoothstep(0.003, 0.032, dist_to_cam);

@if(VERTEX_NORMALS)
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);

    let billboard_pos = world_center + (cam_right * corner.x + cam_up * corner.y) * half_size;
    out.world_position = vec4<f32>(billboard_pos, 1.0);
    out.position = position_world_to_clip(billboard_pos);

@if(VERTEX_COLORS)
    out.color = vec4<f32>(vertex.color.rgb, vertex.color.a * near_cam_fade);
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
@if(VERTEX_COLORS)
    let col = in.color;
@else
    let col = vec4<f32>(1.0, 1.0, 1.0, 1.0);

@if(VERTEX_UVS_A)
    let d = (in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
@else
    let d = vec2<f32>(0.0, 0.0);

    let dist_sq = dot(d, d);
    if dist_sq > 1.0 || col.a <= 0.001 {
        discard;
    }
    let falloff = exp(-2.2 * dist_sq);
    let edge_fade = max(1.0 - dist_sq * dist_sq, 0.0);
    let alpha = col.a * falloff * edge_fade;
    out.color = vec4<f32>(col.rgb, alpha);
    return out;
}
