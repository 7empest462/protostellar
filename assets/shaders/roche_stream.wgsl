#import bevy_pbr::mesh_view_bindings as view_bindings
#import bevy_pbr::mesh_bindings as mesh_bindings
#import bevy_pbr::mesh_functions as mesh_functions
#import bevy_core_pipeline::tonemapping::tone_mapping

struct RocheStreamMaterial {
    color_and_intensity: vec4<f32>,
    stream_params: vec4<f32>, // x: progress, y: turb_scale, z: width, w: unused
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: RocheStreamMaterial;

@fragment
fn fragment(
    @builtin(position) frag_coord: vec4<f32>,
    @builtin(front_facing) is_front: bool,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>
) -> @location(0) vec4<f32> {
    
    // uv.x goes from 0 to 1 along the stream from donor to accretor.
    // L1 point is roughly around 0.4 - 0.6 depending on mass ratio, but we can fake the teardrop shape.
    
    let x = uv.y;
    let y = uv.x * 2.0 - 1.0;
    
    // Narrow in the middle (L1 point), wider at the ends (donor surface and accretor disk)
    let envelope = 1.0 - sin(x * 3.14159) * 0.8;
    let vertical_profile = 1.0;
    
    // Moving noise for plasma flow
    let time = material.stream_params.x;
    let flow_u = x * material.stream_params.y - time;
    let flow_v = uv.x * material.stream_params.y;
    
    // Simple pseudo-random turbulence
    let n1 = sin(flow_u * 12.0 + flow_v * 8.0) * cos(flow_u * 5.0 - flow_v * 15.0);
    let n2 = sin(flow_u * 20.0 + time * 0.5) * cos(flow_v * 20.0);
    let turbulence = (n1 + n2) * 0.25 + 0.5;
    
    let base_alpha = vertical_profile * turbulence;
    
    // Color ramps up in brightness at the L1 choke point and at the accretor disk
    let heat = (1.0 - envelope) * 2.0;
    let final_color = material.color_and_intensity.rgb * (1.0 + heat) * material.color_and_intensity.w;
    
    // Alpha fades at the very tips to blend smoothly with the star surfaces
    let tip_fade = smoothstep(0.0, 0.1, x) * smoothstep(1.0, 0.9, x);
    
    let alpha = clamp(base_alpha * tip_fade, 0.0, 1.0);
    if alpha < 0.01 {
        discard;
    }
    
    return vec4<f32>(final_color, alpha);
}
