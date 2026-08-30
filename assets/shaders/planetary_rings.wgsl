#import bevy_pbr::pbr_types
#import bevy_pbr::pbr_functions
#import bevy_pbr::pbr_bindings
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::pbr_fragment::pbr_input_from_vertex_output

struct RingUniforms {
    inner_radius: f32,
    outer_radius: f32,
    optical_depth: f32,
    ice_fraction: f32,
    ring_color: vec4<f32>,
};

@group(2) @binding(101)
var<uniform> ring: RingUniforms;

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
    let pos_2d = in.world_position.xz - in.world_normal.xz * 0.0; // ring planar radius
    let local_uv = in.uv * 2.0 - 1.0;
    let dist = length(local_uv);

    // Normalized radial ring coordinate from inner (0.0) to outer (1.0)
    let u = (dist - 0.28) / (1.0 - 0.28);
    if (u < 0.0 || u > 1.0) {
        discard;
    }

    // Micro-ringlet density modulation (thousands of concentric fine tracks)
    let fine_tracks = fbm_ring(u * 280.0);
    let broad_bands = fbm_ring(u * 35.0);

    var base_density = 0.0;
    var ring_tone = vec3<f32>(0.92, 0.88, 0.82);

    // 1. C Ring (Inner Crepe Ring: 0.00 to 0.22)
    if (u < 0.22) {
        let t = u / 0.22;
        base_density = mix(0.05, 0.35, t) * (0.6 + fine_tracks * 0.4);
        ring_tone = vec3<f32>(0.45, 0.38, 0.30); // translucent amber/charcoal
    }
    // 2. B Ring (Dense Bright Main Ring: 0.22 to 0.65)
    else if (u < 0.65) {
        let t = (u - 0.22) / (0.65 - 0.22);
        base_density = mix(0.75, 0.98, smoothstep(0.0, 0.3, t)) * (0.75 + fine_tracks * 0.35 + broad_bands * 0.15);
        ring_tone = vec3<f32>(0.96, 0.92, 0.84); // brilliant reflective ice
    }
    // 3. Cassini Division (Prominent Gap: 0.65 to 0.72)
    else if (u < 0.72) {
        let t = (u - 0.65) / (0.72 - 0.65);
        let gap_profile = sin(t * 3.14159265);
        base_density = (1.0 - gap_profile * 0.94) * 0.18 * (0.4 + fine_tracks * 0.6);
        ring_tone = vec3<f32>(0.25, 0.22, 0.20); // dark transparent lane
    }
    // 4. A Ring (Outer Main Ring: 0.72 to 0.96)
    else if (u < 0.96) {
        let t = (u - 0.72) / (0.96 - 0.72);
        var density_a = 0.65 * (0.75 + fine_tracks * 0.35);
        
        // Encke Gap (Sharp clear lane around u = 0.86 to 0.88)
        if (u >= 0.855 && u <= 0.875) {
            let encke_t = (u - 0.855) / (0.875 - 0.855);
            density_a *= (1.0 - sin(encke_t * 3.14159265) * 0.92);
        }
        base_density = density_a;
        ring_tone = vec3<f32>(0.88, 0.84, 0.78);
    }
    // 5. F Ring / Diffuse Outer Boundary (0.96 to 1.0)
    else {
        let t = (u - 0.96) / (1.0 - 0.96);
        base_density = (1.0 - t) * 0.35 * (0.5 + fine_tracks * 0.5);
        ring_tone = vec3<f32>(0.65, 0.60, 0.55);
    }

    // Blend composition: Water Ice (bright white/cyan pearl) vs Silicate/Metal (warm dust)
    let ice_col = mix(vec3<f32>(0.85, 0.75, 0.60), vec3<f32>(0.96, 0.97, 1.00), ring.ice_fraction);
    let final_color = ring_tone * ice_col * ring.ring_color.rgb;

    // Edge feathering to avoid harsh polygonal borders
    let inner_feather = smoothstep(0.0, 0.04, u);
    let outer_feather = 1.0 - smoothstep(0.96, 1.0, u);
    let alpha = clamp(base_density * ring.optical_depth * inner_feather * outer_feather, 0.0, 0.92);

    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(final_color, alpha);
}
