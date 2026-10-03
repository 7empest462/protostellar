// WGSL Volumetric Supernova Blast Wave, Expanding Rayleigh-Taylor Shock Shell, and Core Breakout Shader

#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
}

struct SupernovaUniforms {
    params: vec4<f32>,                 // x: elapsed_s, y: max_timer_s, z: current_radius_au, w: blast_speed_au_s
    core_params: vec4<f32>,            // x: flash_intensity, y: explosion_type (0=Type II, 1=Hypernova, 2=Type Ia, 3=PlanetaryNebula), z: ejecta_mass_solar, w: reserved
    center_and_asphericity: vec4<f32>, // xyz: center_pos_au, w: asphericity_factor
    core_color: vec4<f32>,             // Inner Ni-56/Co-56/Fe radioactive glow (golden-amber)
    mantle_color: vec4<f32>,           // Intermediate O-Si-S forbidden line ionization (cyan/emerald)
    envelope_color: vec4<f32>,         // Outer H-alpha / forward shock boundary (crimson/ruby)
    jet_color: vec4<f32>,              // Relativistic polar jet breakout (sapphire/violet)
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
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
    var shift: vec3<f32> = vec3<f32>(100.0);
    var pos: vec3<f32> = p;
    for (var i: i32 = 0; i < 4; i = i + 1) {
        v = v + a * noise3(pos);
        pos = pos * 2.02 + shift;
        a = a * 0.5;
    }
    return v;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let elapsed = sn.params.x;
    let max_timer = max(sn.params.y, 0.1);
    let progress = clamp(elapsed / max_timer, 0.0, 1.0);
    let explosion_type = sn.core_params.y;
    let flash_intensity = sn.core_params.x;

    // 1. Vector from explosion center to current shell surface point
    let d_pos = in.world_position.xyz - sn.center_and_asphericity.xyz;
    let dist = max(length(d_pos), 0.001);
    let dir = d_pos / dist;

    // View direction and normal for limb brightening
    let cam_pos = view.world_position;
    let V = normalize(cam_pos - in.world_position.xyz);
    let N = normalize(in.world_normal);

    // 2. Limb-Brightening forward shock rim
    // Path length through expanding shell is maximal along glancing grazing angles
    let cos_theta = abs(dot(N, V));
    let limb_rim = pow(1.0 - cos_theta, 1.85);

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
    let opacity_factor = clamp(sn.core_params.w, 0.0, 1.0);
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
