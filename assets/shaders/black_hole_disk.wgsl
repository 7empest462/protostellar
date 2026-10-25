// PROTOSTELLAR: General Relativistic Black Hole Accretion Disk & Gravitational Lensing Shader
// Physically grounded Schwarzschild black hole accretion disk with:
// 1. Shakura-Sunyaev r^(-2.5) flux falloff profile (steep core-to-rim contrast)
// 2. Multi-stop chromaticity: Deep Infrared Red -> Golden Amber -> Blazing Blue-White
// 3. Relativistic Doppler beaming with natural, balanced visual contrast
// 4. Absolute light-trapping event horizon shadow void (b < r_shadow -> pure black)
// 5. Symmetric gravitational lensing arches warping over the top and under the bottom of the black hole
// 6. Near-side accretion disk wrapping seamlessly in front of the black hole shadow
// 7. Razor-sharp asymptotic photon ring tracing the event horizon boundary

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

    if (r <= r_shadow * 0.96 || r > r_outer) {
        return result;
    }

    let r_norm = max(r / r_inner, 0.8);

    // A. Flux profile: plunge-region stress + outer fade
    let stress_factor = smoothstep(r_shadow * 0.94, r_inner * 1.30, r);
    let outer_fade = 1.0 - smoothstep(r_outer * 0.65, r_outer, r);
    var disk_flux = pow(r_norm, -2.5) * stress_factor * outer_fade;

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

    // C. Relativistic Keplerian velocity & Doppler beaming
    let beta = clamp(sqrt(0.5 * r_shadow / max(r, r_shadow)), 0.05, 0.65);
    let gamma = 1.0 / sqrt(max(1.0 - beta * beta, 0.01));

    let cos_theta = cos(phi) * sin_i;
    let doppler = 1.0 / (gamma * (1.0 - beta * cos_theta));
    let a_star = clamp(disk.spin_axis.w, -0.999, 0.999);
    let spin_boost = 1.0 + max(a_star, 0.0) * 0.65;
    let beaming = clamp(pow(doppler, 1.8), 0.35, 3.2 * spin_boost);
    result.doppler_factor = beaming;

    let base_intensity = 14.0;
    let accretion_rate = disk.cam_up_local.w;

    result.emission = local_color * disk.disk_color.rgb * disk_flux * beaming * base_intensity * fine_rings * plasma_density * accretion_rate;

    let alpha_fade = smoothstep(r_shadow * 0.95, r_inner, r);
    result.alpha = clamp(alpha_fade * outer_fade * (0.65 + 0.35 * min(beaming, 2.0)), 0.0, 0.98) * accretion_rate;
    return result;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // ─── 1. Screen-space coordinates on billboard quad ───────────────────────
    let u = (mesh.uv.x - 0.5) * 2.0;
    let v = (0.5 - mesh.uv.y) * 2.0;
    let b = length(vec2<f32>(u, v)); // Impact parameter (screen radius)

    // Scale boundaries (normalized to billboard quad extent 1.0):
    // Quad half-extent is 6.0 * visual_radius.
    // Dynamic Kerr parameters from uniforms:
    let r_shadow = max(disk.schwa_radius, 0.04);
    let r_inner  = max(disk.inner_radius, r_shadow * 1.04);
    let r_outer  = max(disk.outer_radius * 0.780, r_inner * 1.5);

    // ─── 2. Observer inclination ─────────────────────────────────────────────
    let cos_i = clamp(disk.cam_dir_local.w, -1.0, 1.0);
    let abs_cos_i = abs(cos_i);
    let sin_i = sqrt(max(1.0 - cos_i * cos_i, 0.0));

    // Disk physical scale height H(r)
    let h_disk = 0.024 + 0.012 * min(b / r_outer, 1.0);

    // semi_minor: projected vertical factor for the flat disk
    // In top-down view (abs_cos_i -> 1.0): semi_minor -> 1.0 (circular disk).
    // In edge-on view (abs_cos_i -> 0.0): semi_minor -> h_disk (finite physical slab thickness).
    let semi_minor = sqrt(abs_cos_i * abs_cos_i + h_disk * h_disk);

    // ─── 3. Direct disk intersection (primary image) ─────────────────────────
    var direct_emission = vec3<f32>(0.0);
    var direct_alpha = 0.0;

    let z_dir = -v / semi_minor;
    let r_direct = sqrt(u * u + z_dir * z_dir);
    let phi_direct = atan2(z_dir, u);

    // Near side passes directly in front of the black hole
    // In top-down view (abs_cos_i > 0.88), the full circular disk faces the observer
    // In inclined/side view, v * cos_i <= 0 represents the near side closer to the camera
    let is_front = (abs_cos_i > 0.88) || (v * cos_i <= 0.0);

    if (r_direct >= r_shadow * 0.95 && r_direct <= r_outer) {
        let sample = sample_accretion_disk(r_direct, phi_direct, r_inner, r_outer, r_shadow, sin_i, disk.time);

        direct_emission = sample.emission;
        direct_alpha    = clamp(sample.alpha, 0.0, 1.0);
    }

    // ─── 4. Lensed arches (secondary image — photons bent around the black hole) ────
    var lensed_emission = vec3<f32>(0.0);
    var lensed_alpha = 0.0;

    let arch_weight = pow(sin_i, 1.25);
    // Gate arches to strictly the upper (v > 0) and lower (v < 0) visual field,
    // smoothly tapering near the equator so the lateral disk is never distorted
    let v_gate = smoothstep(0.015, 0.075, abs(v));

    if (arch_weight > 0.01 && v_gate > 0.001 && abs(u) <= r_outer * 0.98 && b >= r_shadow) {
        let y_arch = sqrt(max(1.0 - (u / r_outer) * (u / r_outer), 0.0));
        let sign_v = select(-1.0, 1.0, v >= 0.0);
        // Looking from above (cos_i > 0), the upper arch (v > 0) is larger and higher
        let tilt_bias = 0.06 * sign_v * cos_i;
        let arch_scale = max((0.22 + tilt_bias) * sin_i, 0.015);
        let b_max = r_shadow + arch_scale * y_arch;

        if (b <= b_max) {
            let t_arch = clamp((b - r_shadow) / max(b_max - r_shadow, 0.001), 0.0, 1.0);
            let r_lensed = r_inner + pow(t_arch, 0.85) * (r_outer - r_inner);
            let z_rear = -sqrt(max(r_lensed * r_lensed - u * u, 0.001));
            let phi_lensed = atan2(z_rear, u);

            let outer_fade = (1.0 - smoothstep(0.72, 1.0, t_arch)) * arch_weight * v_gate;
            let sample = sample_accretion_disk(r_lensed, phi_lensed, r_inner, r_outer, r_shadow, sin_i, disk.time);

            lensed_emission = sample.emission * outer_fade;
            lensed_alpha    = sample.alpha    * outer_fade;
        }
    }

    // ─── 5. Photon Ring (asymptotic, full circumference) ─────────────────────
    var photon_ring_em    = vec3<f32>(0.0);
    var photon_ring_alpha = 0.0;
    let photon_ring_b = r_shadow * 1.018;
    let ring_dist     = abs(b - photon_ring_b);
    let ring_width    = r_shadow * 0.016;
    if (ring_dist < ring_width * 2.5) {
        let ring_profile   = exp(-0.5 * pow(ring_dist / ring_width, 2.0));
        let ring_phi       = atan2(abs(v), u);
        let isco_sample    = sample_accretion_disk(r_inner * 1.05, ring_phi, r_inner, r_outer, r_shadow, sin_i, disk.time);
        photon_ring_em    = isco_sample.emission * ring_profile * 1.5;
        photon_ring_alpha = ring_profile * 0.95;
    }

    // ─── 6. Shadow core & composite ──────────────────────────────────────────
    // in_shadow = 1.0 inside the event horizon shadow (b < r_shadow), smooth transition at boundary.
    // The event horizon shadow is an absolute light trap: it NEVER discards and is NEVER transparent.
    let in_shadow = 1.0 - smoothstep(r_shadow * 0.985, r_shadow * 1.012, b);

    // Foreground: the near side of the accretion disk physically passing in front of the black hole
    let fg_emission = select(vec3<f32>(0.0), direct_emission, is_front);
    let fg_alpha    = select(0.0, direct_alpha, is_front);

    // Background: rear side of the direct disk, lensed arches, and photon ring.
    // All light rays originating behind the event horizon are absorbed by the black hole!
    let rear_disk_em = select(direct_emission, vec3<f32>(0.0), is_front);
    let rear_disk_a  = select(direct_alpha, 0.0, is_front);

    let bg_transmission = 1.0 - in_shadow;
    let bg_emission = (rear_disk_em + lensed_emission + photon_ring_em) * bg_transmission;
    let bg_alpha    = (rear_disk_a + lensed_alpha + photon_ring_alpha) * bg_transmission;

    // Composite foreground (near-side disk) over mid-layer (shadow + background):
    let total_emission = fg_emission + bg_emission * (1.0 - fg_alpha);
    let total_alpha    = clamp(fg_alpha + (in_shadow + bg_alpha) * (1.0 - fg_alpha), 0.0, 1.0);

    // Only discard outside the event horizon shadow where the quad is transparent to the skybox
    if (in_shadow < 0.01 && total_alpha < 0.005) {
        discard;
    }

    return vec4<f32>(total_emission, total_alpha);
}
