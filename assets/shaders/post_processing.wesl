import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct PostProcessingSettings {
    exposure: f32,
    bloom_intensity: f32,
    lensing_strength: f32,
    tonemapping_mode: u32,
    black_hole_screen_pos: vec2<f32>,
    hazard_flash_intensity: f32,
    time: f32,
    shockwave_phase: f32,
    shockwave_amplitude: f32,
    shockwave_center: vec2<f32>,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: PostProcessingSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var uv = in.uv;

    // 1. Relativistic Spacetime Curvature & Black Hole Gravitational Lensing
    var ca_shift = vec2<f32>(0.0);
    if (settings.lensing_strength > 0.001) {
        let to_bh = uv - settings.black_hole_screen_pos;
        let bh_dist = length(to_bh);
        let deflection = (settings.lensing_strength * 0.004) / max(bh_dist, 0.06);
        let dir = normalize(to_bh + vec2<f32>(1e-5));
        // Deflect UVs towards the black hole center (Einstein ring warping)
        uv = uv - dir * clamp(deflection, 0.0, 0.25);
        // Chromatic dispersion around the photon ring
        ca_shift += dir * (deflection * 0.45);
    }

    // 2. Relativistic Gravitational-Wave Ripples & Metric Perturbations (h+ / hx strain waves)
    if (settings.shockwave_amplitude > 0.001) {
        let to_shock = uv - settings.shockwave_center;
        let shock_dist = length(to_shock);
        let wave_phase = shock_dist * 38.0 - settings.shockwave_phase * 12.0;
        let ripple_falloff = exp(-shock_dist * 3.5);
        let ripple = sin(wave_phase) * ripple_falloff * settings.shockwave_amplitude * 0.035;
        let shock_dir = normalize(to_shock + vec2<f32>(1e-5));
        uv = uv + shock_dir * ripple;
        ca_shift += shock_dir * (abs(ripple) * 1.8);
    }

    // 3. Chromatic Aberration Sampling on HDR Scene
    var hdr_color: vec3<f32>;
    if (dot(ca_shift, ca_shift) > 1e-7) {
        let r_ch = textureSample(screen_texture, screen_sampler, uv + ca_shift).r;
        let g_ch = textureSample(screen_texture, screen_sampler, uv).g;
        let b_ch = textureSample(screen_texture, screen_sampler, uv - ca_shift).b;
        hdr_color = vec3<f32>(r_ch, g_ch, b_ch);
    } else {
        hdr_color = textureSample(screen_texture, screen_sampler, uv).rgb;
    }

    // 4. Cosmic Hazard Shockwave Flash (Ionizing radiation burst overlay)
    if (settings.hazard_flash_intensity > 0.001) {
        let flash_glow = vec3<f32>(1.4, 2.1, 3.2); // Blinding Cherenkov / ionizing cyan-violet
        hdr_color = mix(hdr_color, flash_glow, clamp(settings.hazard_flash_intensity * 0.75, 0.0, 0.95));
    }

    return vec4<f32>(hdr_color, 1.0);
}
