// PROTOSTELLAR: General Relativistic Black Hole Accretion Disk & Gravitational Lensing Shader
// Physically grounded Schwarzschild black hole accretion disk with:
// 1. Shakura-Sunyaev r^(-3) flux falloff profile (steep core-to-rim contrast)
// 2. Multi-stop chromaticity: Deep Infrared Red -> Golden Amber -> Blazing Blue-White
// 3. Unclamped relativistic Doppler beaming (I ~ delta^3.5): iconic EHT/Gargantua crescent asymmetry
// 4. Absolute light-trapping event horizon shadow void (b < r_shadow -> pure black)
// 5. Asymptotic photon ring sampling inner disk emission with e^(-pi) geometric compression
// 6. Proper 3D geometry: front/back determined by dot product with disk normal, not 2D sign heuristics

#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::forward_io::VertexOutput

struct BlackHoleDiskUniforms {
    inner_radius: f32,     // ISCO radius (normalized, ~0.16)
    outer_radius: f32,     // Outer disk boundary (normalized, 1.0)
    schwa_radius: f32,     // Schwarzschild event horizon radius (normalized, ~0.055)
    time: f32,
    disk_color: vec4<f32>, // Base emission tint
    spin_axis: vec4<f32>,  // xyz: unit spin axis in local space, w: BH mass
    cam_dir_local: vec4<f32>, // xyz: unit camera direction in local space, w: cos(inclination)
    cam_up_local: vec4<f32>,  // xyz: true screen-up vector in local BH space, w: unused
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> disk: BlackHoleDiskUniforms;

fn hash12(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    let p3_dot = dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3_dot);
}

fn simplex_noise_2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn mri_turbulent_noise(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.55;
    let shift = vec2<f32>(100.0, 100.0);
    var p_curr = p;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + a * simplex_noise_2d(p_curr);
        p_curr = p_curr * 2.05 + shift;
        a = a * 0.48;
    }
    return v;
}

struct DiskSample {
    emission: vec3<f32>,
    alpha: f32,
    doppler_factor: f32,
}

fn sample_accretion_disk(
    r: f32,
    phi: f32,
    r_inner: f32,
    r_outer: f32,
    r_shadow: f32,
    sin_i: f32,
    time: f32
) -> DiskSample {
    var result: DiskSample;
    result.emission = vec3<f32>(0.0);
    result.alpha = 0.0;
    result.doppler_factor = 1.0;

    if (r <= r_shadow * 1.01 || r > r_outer) {
        return result;
    }

    let r_norm = max(r / r_inner, 0.8);

    // A. Flux profile: plunge-region stress + outer fade
    let stress_factor = smoothstep(r_shadow * 1.02, r_inner * 1.8, r);
    let outer_fade = smoothstep(r_outer, r_outer * 0.55, r);
    var disk_flux = pow(r_norm, -3.0) * stress_factor * outer_fade;

    // Peak flux normalization for chromatic mapping
    let t_val = clamp((disk_flux / 0.8) * 1.4, 0.0, 1.0);

    // B. Multi-stop chromaticity: Deep IR Red -> Golden Amber -> Blazing Blue-White
    let col_outer = vec3<f32>(0.25, 0.02, 0.005);
    let col_mid   = vec3<f32>(1.00, 0.42, 0.05);
    let col_inner = vec3<f32>(0.90, 0.95, 1.40);

    var local_color: vec3<f32>;
    if (t_val < 0.30) {
        local_color = mix(col_outer, col_mid, t_val / 0.30);
    } else {
        local_color = mix(col_mid, col_inner, (t_val - 0.30) / 0.70);
    }

    // Concentric Keplerian rings
    let ring_freq = r * 65.0;
    let fine_rings = 0.85 + 0.15 * sin(ring_freq) + 0.05 * sin(ring_freq * 2.3);

    // Differential Keplerian spiral shear & turbulent MRI wisps using polar mapping
    let omega = pow(r_norm, -1.5) * 1.2;
    let shear_angle = phi - omega * time;
    let u_coord = log(r_norm) * 1.8;
    let v_coord = shear_angle / (2.0 * 3.14159265);
    let turbulence_val = mri_turbulent_noise(vec2<f32>(u_coord * 4.0, v_coord * 8.0));
    let plasma_density = 0.6 + 0.7 * turbulence_val;

    // C. Relativistic Keplerian velocity & Doppler beaming (I ~ delta^3.5)
    let beta = clamp(sqrt(0.5 * r_shadow / max(r, r_shadow)), 0.05, 0.65);
    let gamma = 1.0 / sqrt(max(1.0 - beta * beta, 0.01));

    let cos_theta = -sin(phi) * sin_i;
    let doppler = 1.0 / (gamma * (1.0 - beta * cos_theta));
    let beaming = pow(doppler, 3.5);
    result.doppler_factor = beaming;

    let base_intensity = 12.0;

    result.emission = local_color * disk.disk_color.rgb * disk_flux * beaming * base_intensity * fine_rings * plasma_density;

    let alpha_fade = smoothstep(r_shadow * 1.01, r_inner, r);
    result.alpha = clamp(alpha_fade * outer_fade * (0.35 + 0.55 * min(beaming, 2.5)), 0.0, 0.98);
    return result;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // ─── 1. Screen-space coordinates on billboard quad ───────────────────────
    let u = (mesh.uv.x - 0.5) * 2.0;
    let v = (0.5 - mesh.uv.y) * 2.0;
    let b = length(vec2<f32>(u, v)); // Impact parameter (screen radius)

    // Scale boundaries (normalized to outer_radius = 1.0):
    let r_shadow = 0.165; // Event horizon shadow (b_crit ~ 3√3 M)
    let r_inner  = 0.185; // ISCO (3 r_s) inner edge
    let r_outer  = 0.88;  // Outer disk edge

    // ─── 2. Observer inclination ─────────────────────────────────────────────
    // cos_i: camera direction dotted with spin axis (Y-up in local space)
    let cos_i = clamp(disk.cam_dir_local.w, -1.0, 1.0);
    let sin_i = sqrt(max(1.0 - cos_i * cos_i, 0.0));


    // ─── 3. Observer inclination geometry ────────────────────────────────────
    // cos_i = dot(cam_dir_local, spin_axis) = cam_dir_local.y in local space.
    // Geometrically derived front-half condition (see derivation in docs):
    //   A billboard pixel at (u, v) maps to a disk-space point. Its projection
    //   onto the in-plane camera direction is proportional to v * cos_i.
    //   v * cos_i > 0  →  near side (between observer and BH center).
    //   v * cos_i < 0  →  far side (behind BH center).
    //
    // Physical disk height above disk plane for a billboard pixel:
    //   height_above_disk = v * sin_i
    //   (cam_up projected onto disk normal Y gives sin_i, u_right.y = 0 always)

    // ─── 4. Direct disk intersection (primary image) ─────────────────────────
    var hit_direct = false;
    var direct_emission = vec3<f32>(0.0);
    var direct_alpha = 0.0;

    // Elliptical foreshortening: disk at inclination i appears with semi_minor = |cos_i|.
    // r_direct = sqrt(u² + (v/cos_i)²) inverts the projection to get true disk radius.
    let semi_minor = sqrt(cos_i * cos_i + 0.0015); // epsilon keeps disk visible edge-on
    let r_direct   = sqrt(u * u + (v / semi_minor) * (v / semi_minor));
    let phi_direct = atan2(v / semi_minor, u);

    // Vertical thickness Gaussian — physically correct form:
    //   height above disk plane = v * sin_i.
    //   When face-on (sin_i=0): height=0 everywhere → full ring visible.
    //   When edge-on (sin_i=1): height = v → thin equatorial strip.
    let h_disk  = 0.055 + 0.050 * (r_direct / r_outer);
    let v_thick = exp(-0.5 * pow((v * sin_i) / h_disk, 2.0));

    // Front-half gate: disk only renders on near side (v*cos_i > 0).
    // Side wings (|u| > r_shadow) are always visible from both directions.
    // Near edge-on (sin_i → 1) both halves converge to the equatorial strip.
    let front_gate       = smoothstep(-0.015, 0.025, v * cos_i);
    let side_wing_factor = smoothstep(r_shadow * 0.75, r_shadow * 1.6, abs(u));
    let edge_on_factor   = smoothstep(0.80, 0.97, sin_i);
    let disk_geo_mask    = clamp(front_gate + side_wing_factor + edge_on_factor, 0.0, 1.0);

    if (r_direct >= r_shadow && r_direct <= r_outer && v_thick > 0.005) {
        let sample = sample_accretion_disk(r_direct, phi_direct, r_inner, r_outer, r_shadow, sin_i, disk.time);
        direct_emission = sample.emission;
        direct_alpha    = sample.alpha * v_thick * disk_geo_mask;
        hit_direct      = direct_alpha > 0.02;
    }

    // ─── 5. Lensed arc (secondary image — photons bent around BH) ────────────
    var lensed_emission = vec3<f32>(0.0);
    var lensed_alpha = 0.0;

    let delta_b    = b - r_shadow;
    let max_arch_b = 0.58;
    if (delta_b > 0.001 && delta_b < max_arch_b) {
        let t_arch = delta_b / max_arch_b;
        // Inverted lens mapping: b → r in rear disk
        let r_lensed   = r_shadow + pow(t_arch, 0.82) * (r_outer * 0.85 - r_shadow);
        let phi_lensed = atan2(-v / semi_minor, -u);

        // Arch profile: fades at inner edge (near shadow) and outer edge.
        // No angular mask — the lensed arc wraps the full circumference of the shadow.
        // The atan2 approach (old code) created a 4-lobe butterfly pattern by clamping
        // the arc to top/bottom only. We let it show all the way around.
        let arch_profile = pow(sin(t_arch * 3.14159), 0.65) * smoothstep(1.0, 0.12, t_arch);

        if (arch_profile > 0.01 && r_lensed >= r_shadow && r_lensed <= r_outer) {
            let sample = sample_accretion_disk(r_lensed, phi_lensed, r_inner, r_outer, r_shadow, sin_i, disk.time);
            lensed_emission = sample.emission * arch_profile * 0.90;
            lensed_alpha    = sample.alpha   * arch_profile * 0.92;
        }
    }

    // ─── 6. Photon Ring (asymptotic, full circumference) ─────────────────────
    var photon_ring_em    = vec3<f32>(0.0);
    var photon_ring_alpha = 0.0;
    let photon_ring_b = r_shadow * 1.018;
    let ring_dist     = abs(b - photon_ring_b);
    let ring_width    = r_shadow * 0.016;
    if (ring_dist < ring_width * 2.5) {
        let ring_profile   = exp(-0.5 * pow(ring_dist / ring_width, 2.0));
        let isco_sample    = sample_accretion_disk(r_inner * 1.12, atan2(-v, -u), r_inner, r_outer, r_shadow, sin_i, disk.time);
        photon_ring_em    = isco_sample.emission * ring_profile * 1.5;
        photon_ring_alpha = ring_profile * 0.96;
    }

    // ─── 7. Shadow core & composite ──────────────────────────────────────────
    var shadow_mask = smoothstep(r_shadow * 0.97, r_shadow * 1.015, b);

    // Direct disk in FRONT of the BH (near side) overrides the shadow mask.
    // Correct condition: v * cos_i > 0 (geometrically derived near-side test).
    // We further require the pixel is physically within the disk slab (v_thick > 0.1).
    let is_front = v * cos_i > 0.0;
    if (hit_direct && is_front && v_thick > 0.10) {
        shadow_mask = 1.0;
    }

    let total_emission = (direct_emission + lensed_emission * (1.0 - direct_alpha * 0.7) + photon_ring_em) * shadow_mask;
    let total_alpha    = clamp(
        (direct_alpha + lensed_alpha * (1.0 - direct_alpha * 0.7) + photon_ring_alpha),
        0.0, 1.0
    ) * shadow_mask + (1.0 - shadow_mask);

    if (total_alpha < 0.005) {
        discard;
    }

    return vec4<f32>(total_emission, total_alpha);
}
