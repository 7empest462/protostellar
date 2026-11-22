// WGSL Volumetric Supernova Blast Wave, Expanding Rayleigh-Taylor Shock Shell, and Core Breakout Shader

import bevy_pbr::render::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
};


struct SupernovaUniforms {
    params: vec4<f32>,                 // x: elapsed_s, y: max_timer_s, z: current_radius_au, w: blast_speed_au_s
    core_params: vec4<f32>,            // x: flash_intensity, y: explosion_type (0=Type II, 1=Hypernova, 2=Type Ia, 3=PlanetaryNebula), z: ejecta_mass_solar, w: reserved
    center_and_asphericity: vec4<f32>, // xyz: center_pos_au, w: asphericity_factor
    core_color: vec4<f32>,             // Inner Ni-56/Co-56/Fe radioactive glow (golden-amber)
    mantle_color: vec4<f32>,           // Intermediate O-Si-S forbidden line ionization (cyan/emerald)
    envelope_color: vec4<f32>,         // Outer H-alpha / forward shock boundary (crimson/ruby)
    jet_color: vec4<f32>,              // Relativistic polar jet breakout (sapphire/violet)
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> sn: SupernovaUniforms;

const PI: f32 = 3.14159265359;

// Fast 3D Hash and Value Noise
fn hash3(p: vec3<f32>) -> f32 {
    let q = vec3<f32>(
        dot(p, vec3<f32>(127.1, 311.7, 74.7)),
        dot(p, vec3<f32>(269.5, 183.3, 246.1)),
        dot(p, vec3<f32>(113.5, 271.9, 124.6))
    );
    return fract(sin(dot(q, vec3<f32>(1.0, 1.0, 1.0))) * 43758.5453123);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n000 = hash3(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));

    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);

    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);

    return mix(nxy0, nxy1, u.z);
}

// 4-Octave Fractal Brownian Motion for Rayleigh-Taylor filaments
fn fbm3(p: vec3<f32>) -> f32 {
    var v: f32 = 0.0;
    var a: f32 = 0.5;
    let shift: vec3<f32> = vec3<f32>(100.0);
    var pos: vec3<f32> = p;
    for (var i: i32 = 0; i < 4; i = i + 1) {
        v = v + a * noise3(pos);
        pos = pos * 2.02 + shift;
        a = a * 0.5;
    }
    return v;
}

fn evaluate_quasar_blowout_layer(
    dir: vec3<f32>,
    cos_theta: f32,
    elapsed: f32,
    progress: f32,
    explosion_type: f32,
    flash_intensity: f32,
    opacity_factor: f32,
) -> vec4<f32> {
    let rim_base = max(1.0 - cos_theta, 0.0);
    let limb_rim = pow(rim_base, 1.55);
    let sharp_rim = pow(rim_base, 3.2);

    // 3D domain-warped turbulence advected violently outward
    let warp_coord = dir * 3.8 + vec3<f32>(elapsed * 0.22, -elapsed * 0.35, elapsed * 0.18);
    let q = vec3<f32>(
        fbm3(warp_coord),
        fbm3(warp_coord + vec3<f32>(4.3, 1.7, 8.2)),
        fbm3(warp_coord + vec3<f32>(2.1, 7.5, 3.4))
    );
    let filament = fbm3(warp_coord * 1.65 + 2.1 * q);
    let ridge_crack = 1.0 - smoothstep(0.0, 0.12, abs(filament - 0.50));
    let clump = smoothstep(0.26, 0.78, filament);
    let knots = pow(smoothstep(0.58, 0.93, filament), 2.8) * 3.2;

    // Layer 4.0: Outer Shattering Primordial Hydrogen Cocoon Fireball
    if (explosion_type < 4.5) {
        let crack_phase = 1.0 - smoothstep(0.02, 0.14, progress);
        let dark_crimson = vec3<f32>(0.55, 0.04, 0.02);
        let h_alpha_ruby = sn.envelope_color.rgb * 1.35;
        let molten_gold = sn.core_color.rgb * 1.55;
        let o_iii_cyan = sn.mantle_color.rgb * 1.25;
        let eddington_white = vec3<f32>(2.8, 3.6, 5.2);

        // Cocoon fractures with blinding blue-white fissures before tearing into Rayleigh-Taylor plumes
        var shell_col = mix(dark_crimson, h_alpha_ruby, clump);
        shell_col = mix(shell_col, molten_gold, smoothstep(0.45, 0.85, filament) * (1.0 - progress * 0.45));
        shell_col = mix(shell_col, o_iii_cyan, smoothstep(0.60, 0.94, q.x) * progress * 0.75);
        let fissure_glow = eddington_white * ridge_crack * (1.8 * crack_phase + 0.55 * (1.0 - progress));
        let shock_edge = mix(molten_gold, eddington_white, flash_intensity * 0.75) * (limb_rim * 3.4 + sharp_rim * 4.5);

        let energy_decay = mix(1.0, pow(max(1.0 - progress, 0.0), 0.75), 0.78);
        let final_rgb = (shell_col * (1.45 + clump * 1.85 + knots) + fissure_glow + shock_edge)
            * (1.0 + flash_intensity * 2.6) * energy_decay * max(opacity_factor, 0.30);

        let shell_density = mix(
            clamp(0.72 + ridge_crack * 0.45 + limb_rim * 0.65, 0.0, 1.0),
            clamp(limb_rim * 0.82 + clump * 0.58 + knots * 0.35 + ridge_crack * 0.25, 0.0, 1.0),
            1.0 - crack_phase
        );
        let alpha_fade = mix(pow(max(1.0 - progress, 0.0), 1.05), 1.0, clamp(opacity_factor * 0.75, 0.0, 0.90));
        let final_alpha = clamp(shell_density * alpha_fade * opacity_factor * 0.95, 0.0, 1.0);
        return vec4<f32>(final_rgb, final_alpha);
    }

    // Layer 5.0: Inner Relativistic Photon Cavern & Cherenkov Radiation Core
    if (explosion_type < 5.5) {
        let caustic = pow(smoothstep(0.38, 0.86, fbm3(dir * 5.2 - vec3<f32>(0.0, elapsed * 0.8, 0.0) + q * 1.6)), 2.0);
        let cherenkov_blue = vec3<f32>(0.35, 0.82, 2.4);
        let ultraviolet_violet = vec3<f32>(0.95, 0.32, 2.1);
        let blinding_core = vec3<f32>(3.4, 3.8, 4.8);

        var core_col = mix(ultraviolet_violet, cherenkov_blue, smoothstep(0.25, 0.80, q.y));
        core_col = mix(core_col, blinding_core, clamp(flash_intensity * 0.85 + caustic * 0.65, 0.0, 1.0));
        let final_rgb = core_col * (limb_rim * 4.2 + caustic * 3.0 + flash_intensity * 5.5) * max(1.0 - progress * 0.85, 0.15);
        let base_a = clamp(limb_rim * 0.85 + caustic * 0.55 + flash_intensity * 0.90, 0.0, 1.0);
        let final_alpha = clamp(base_a * pow(max(1.0 - progress, 0.0), 1.25) * opacity_factor * 0.92, 0.0, 1.0);
        return vec4<f32>(final_rgb, final_alpha);
    }

    // Layer 6.0: Equatorial Supersonic Mach Shock Torus / Accretion Blast Disk
    if (explosion_type < 6.5) {
        let eq_band = pow(max(1.0 - abs(dir.y), 0.0), 2.2);
        let azimuth = atan2(dir.z, dir.x);
        let spiral_wave = 0.5 + 0.5 * sin(azimuth * 7.0 - elapsed * 4.8 + filament * 6.5);
        let mach_crest = pow(eq_band, 2.0) * (0.65 + 0.65 * spiral_wave);

        let inner_gold = vec3<f32>(2.6, 1.25, 0.25);
        let outer_cyan = vec3<f32>(0.25, 1.45, 2.4);
        let crimson_wake = vec3<f32>(1.5, 0.22, 0.10);

        var ring_col = mix(crimson_wake, inner_gold, smoothstep(0.25, 0.78, spiral_wave));
        ring_col = mix(ring_col, outer_cyan, sharp_rim * 0.85 + (1.0 - clump) * 0.35);
        ring_col = mix(ring_col, vec3<f32>(3.0, 3.2, 3.8), flash_intensity * 0.70);

        let final_rgb = ring_col * (mach_crest * 4.8 + sharp_rim * 5.2 + knots * 0.8) * (1.0 + flash_intensity * 2.2);
        let base_a = clamp((eq_band * 0.72 + sharp_rim * 0.65) * (0.45 + 0.55 * spiral_wave + clump * 0.35), 0.0, 1.0);
        let final_alpha = clamp(base_a * pow(max(1.0 - progress, 0.0), 1.10) * opacity_factor * 0.92, 0.0, 1.0);
        return vec4<f32>(final_rgb, final_alpha);
    }

    // Layer 7.0: Bipolar Relativistic Jet Breakout Cones
    let abs_y = abs(dir.y);
    let polar_cone = smoothstep(0.42, 0.90, abs_y);
    let azimuth = atan2(dir.z, dir.x);
    let helix = 0.5 + 0.5 * sin(azimuth * 5.0 + abs_y * 16.0 - elapsed * 9.5 + q.x * 4.0);
    let bow_shock = smoothstep(0.84, 0.985, abs_y);

    let jet_sapphire = sn.jet_color.rgb * 1.9;
    let jet_violet = vec3<f32>(0.85, 0.28, 2.2);
    let bow_white = vec3<f32>(3.6, 4.2, 5.5);

    var jet_col = mix(jet_violet, jet_sapphire, helix);
    jet_col = mix(jet_col, bow_white, clamp(bow_shock * 0.85 + flash_intensity * 0.65, 0.0, 1.0));

    let final_rgb = jet_col * polar_cone * (2.6 + helix * 2.8 + bow_shock * 5.0 + limb_rim * 2.4);
    let base_a = clamp(polar_cone * (0.42 + helix * 0.45 + bow_shock * 0.65 + limb_rim * 0.35), 0.0, 1.0);
    let final_alpha = clamp(base_a * pow(max(1.0 - progress, 0.0), 1.15) * opacity_factor * 0.92, 0.0, 1.0);
    return vec4<f32>(final_rgb, final_alpha);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let elapsed = sn.params.x;
    let max_timer = max(sn.params.y, 0.1);
    let progress = clamp(elapsed / max_timer, 0.0, 1.0);
    let explosion_type = sn.core_params.y;
    let flash_intensity = sn.core_params.x;
    let opacity_factor = clamp(sn.core_params.w, 0.0, 1.0);

    // 1. Vector from explosion center to current shell surface point
    let d_pos = in.world_position.xyz - sn.center_and_asphericity.xyz;
    let dist = max(length(d_pos), 1e-9);
    let dir = d_pos / dist;

    // View direction and normal for limb brightening
    let cam_pos = view.world_position;
    let V = normalize(cam_pos - in.world_position.xyz);
    let N = normalize(in.world_normal);
    let cos_theta = abs(dot(N, V));

    // Dedicated multi-layer cinematic Little Red Dot Quasar Blowout rendering
    if (explosion_type >= 3.5) {
        let blowout_rgba = evaluate_quasar_blowout_layer(
            dir,
            cos_theta,
            elapsed,
            progress,
            explosion_type,
            flash_intensity,
            opacity_factor,
        );
        if (blowout_rgba.a < 0.003) {
            discard;
        }
        out.color = blowout_rgba;
        return out;
    }

    // 2. Limb-Brightening forward shock rim
    // Path length through expanding shell is maximal along glancing grazing angles
    let limb_rim = pow(max(1.0 - cos_theta, 0.0), 1.85);

    // 3. Domain-Warped Rayleigh-Taylor Instability Filaments
    let warp_coord = dir * 3.2 + vec3<f32>(0.0, elapsed * 0.15, 0.0);
    let q = vec3<f32>(
        fbm3(warp_coord),
        fbm3(warp_coord + vec3<f32>(4.3, 1.7, 8.2)),
        fbm3(warp_coord + vec3<f32>(2.1, 7.5, 3.4))
    );
    let filament_pattern = fbm3(warp_coord * 1.5 + 1.8 * q);

    // Cellular thresholding: creates knot clumps, webbed filaments, and voids
    let filament_contrast = smoothstep(0.32, 0.78, filament_pattern);
    let knot_spikes = pow(smoothstep(0.55, 0.92, filament_pattern), 3.0) * 2.5;

    // 4. Multi-Layer Spectral Composition Synthesis with Forbidden Cooling Lines:
    // Red [S II] 671.6 nm (dense compression shock fronts)
    // Teal [O III] 500.7 nm (high-energy forward ionization zone)
    // Violet-blue synchrotron & H-beta (shocked magnetic filaments)
    let s_ii_red = vec3<f32>(0.96, 0.18, 0.12);
    let o_iii_teal = vec3<f32>(0.10, 0.95, 0.72);
    let synchrotron_blue = vec3<f32>(0.28, 0.44, 0.98);

    let core_weight = smoothstep(0.65, 0.95, filament_pattern) * (1.0 - progress * 0.7);
    let mantle_weight = smoothstep(0.40, 0.75, filament_pattern);
    let envelope_weight = clamp(1.0 - core_weight - mantle_weight * 0.5, 0.0, 1.0);

    let elemental_lines = mix(o_iii_teal, s_ii_red, smoothstep(0.40, 0.85, knot_spikes * 0.4 + limb_rim * 0.6));
    let spectral_emission = mix(elemental_lines, synchrotron_blue, smoothstep(0.65, 0.98, q.x));

    var base_color = sn.envelope_color.rgb * envelope_weight
                   + sn.mantle_color.rgb * mantle_weight
                   + sn.core_color.rgb * core_weight;
    base_color = mix(base_color, spectral_emission, 0.35 + 0.35 * progress);

    // 5. Relativistic Polar Jet Breakout for Hypernova (Collapsars)
    var polar_boost: f32 = 0.0;
    if (explosion_type > 0.5 && explosion_type < 1.5) {
        let abs_y = abs(dir.y);
        if (abs_y > 0.72) {
            let jet_factor = smoothstep(0.72, 0.96, abs_y);
            polar_boost = jet_factor * 2.8;
            base_color = mix(base_color, sn.jet_color.rgb * 1.8, jet_factor * 0.85);
        }
    }

    // 6. Prompt Nuclear Breakout Flash
    // Blinding blue-white flash decaying as exp(-3.5 * t)
    let flash_white = vec3<f32>(1.2, 1.35, 1.6);
    let current_color = mix(base_color, flash_white * (1.0 + flash_intensity * 3.5), flash_intensity * 0.92);

    // 7. Radiance & Emission Amplification
    let emission_mult = (limb_rim * 2.8 + filament_contrast * 1.6 + knot_spikes + polar_boost + flash_intensity * 4.0);
    let final_rgb = current_color * emission_mult * max(opacity_factor, 0.25);

    // 8. Dynamic Transparency & Recombination Dissipation
    let alpha_decay = mix(pow(max(1.0 - progress, 0.0), 1.15), 1.0, clamp(opacity_factor * 0.8, 0.0, 0.95));
    let base_alpha = clamp((limb_rim * 0.75 + filament_contrast * 0.55 + knot_spikes * 0.35 + flash_intensity * 0.85 + polar_boost * 0.3), 0.0, 1.0);
    let final_alpha = clamp(base_alpha * alpha_decay * opacity_factor * 0.92, 0.0, 1.0);

    // Smooth early exit for faint fragments
    if (final_alpha < 0.003) {
        discard;
    }

    out.color = vec4<f32>(final_rgb, final_alpha);
    return out;
}
