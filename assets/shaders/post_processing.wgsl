#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct PostProcessingSettings {
    exposure: f32,
    bloom_intensity: f32,
    lensing_strength: f32,
    tonemapping_mode: u32,
    black_hole_screen_pos: vec2<f32>,
    hazard_flash_intensity: f32,
    pad: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var source_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var source_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> settings: PostProcessingSettings;

// ACES Filmic Tonemapping Curve (Narkowicz 2015)
fn tonemap_aces(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Reinhard-Jodie Tonemapping (Preserves Luminance and Saturation)
fn tonemap_reinhard(color: vec3<f32>) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let tc = color / (color + vec3<f32>(1.0));
    return mix(color / (1.0 + luma), tc, tc);
}

// AgX Filmic S-curve approximation
fn tonemap_agx(color: vec3<f32>) -> vec3<f32> {
    let x = max(vec3<f32>(0.0), color - 0.004);
    return (x * (6.2 * x + 0.5)) / (x * (6.2 * x + 1.7) + 0.06);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let texel_size = 1.0 / vec2<f32>(textureDimensions(source_texture));

    // 1. Sample HDR Scene Color (with subtle radial chromatic dispersion near relativistic gravity wells)
    var hdr_color = textureSample(source_texture, source_sampler, uv).rgb;
    if (settings.lensing_strength > 0.001) {
        let to_bh = uv - settings.black_hole_screen_pos;
        let bh_dist = length(to_bh);
        let ca_shift = normalize(to_bh + vec2<f32>(1e-5)) * (settings.lensing_strength * 0.004 / max(bh_dist, 0.08));
        let r_ch = textureSample(source_texture, source_sampler, uv + ca_shift).r;
        let b_ch = textureSample(source_texture, source_sampler, uv - ca_shift).b;
        hdr_color = vec3<f32>(r_ch, hdr_color.g, b_ch);
    }

    // 2. High-Pass Bloom + 6-Point Optical Diffraction Starburst & Anamorphic Streak Kernel
    var bloom = vec3<f32>(0.0);
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size * 2.0;
            let sample_col = textureSample(source_texture, source_sampler, uv + offset).rgb;
            let brightness = dot(sample_col, vec3<f32>(0.2126, 0.7152, 0.0722));
            if (brightness > 1.0) {
                bloom += sample_col * (brightness - 1.0);
            }
        }
    }
    bloom *= 0.1111;

    // 6-ray optical diffraction spike & horizontal anamorphic tap along extreme HDR emitters (> 1.8)
    var starburst = vec3<f32>(0.0);
    let dirs = array<vec2<f32>, 3>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.5, 0.866025),
        vec2<f32>(-0.5, 0.866025)
    );
    for (var d = 0; d < 3; d++) {
        let dir = dirs[d];
        for (var s = 1; s <= 4; s++) {
            let radius = f32(s) * 4.5;
            let w = exp(-f32(s) * 0.55);
            let s_pos = textureSample(source_texture, source_sampler, uv + dir * texel_size * radius).rgb;
            let s_neg = textureSample(source_texture, source_sampler, uv - dir * texel_size * radius).rgb;
            let b_pos = max(dot(s_pos, vec3<f32>(0.2126, 0.7152, 0.0722)) - 1.8, 0.0);
            let b_neg = max(dot(s_neg, vec3<f32>(0.2126, 0.7152, 0.0722)) - 1.8, 0.0);
            starburst += (s_pos * b_pos + s_neg * b_neg) * w * 0.045;
        }
    }

    // Add bloom and diffraction starburst into HDR color
    hdr_color += (bloom + starburst) * settings.bloom_intensity;

    // 3. Exposure adjustment
    let exposed = hdr_color * settings.exposure;

    // 4. Selectable Tonemapping Algorithm
    var ldr_color: vec3<f32>;
    if (settings.tonemapping_mode == 1u) {
        ldr_color = tonemap_aces(exposed);
    } else if (settings.tonemapping_mode == 2u) {
        ldr_color = tonemap_reinhard(exposed);
    } else {
        ldr_color = tonemap_agx(exposed);
    }

    // 5. Cosmic Hazard Shockwave Flash overlay (ionizing cyan/violet glow)
    if (settings.hazard_flash_intensity > 0.001) {
        let flash_color = vec3<f32>(0.7, 0.85, 1.0);
        ldr_color = mix(ldr_color, flash_color, clamp(settings.hazard_flash_intensity * 0.75, 0.0, 0.95));
    }

    // 6. Gamma Correction (Linear -> sRGB)
    let final_color = pow(clamp(ldr_color, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));

    return vec4<f32>(final_color, 1.0);
}
