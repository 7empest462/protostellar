// ============================================================================
// PROTOSTELLAR // 3D Volumetric Circumstellar Environments & Relativistic Halo Shader
// ============================================================================
// Regimes (env_type = u32(env.mode_and_time.y + 0.5)):
// 0: LBV Homunculus Bipolar Eruption Nebula (Eta Carinae peanut lobes + equatorial skirt)
// 1: Wolf-Rayet Colliding-Wind Pinwheel Spiral Nebula (WR 104 Archimedean spiral plume)
// 2: AGB Carbon Star / Yellow Hypergiant / Thorne-Zytkow Detached Shells
// 3: Boson Star & Quark Star Relativistic Einstein Lensing Ring & Electrosphere Halo
// 4: Herbig Ae/Be & T Tauri Flared Protoplanetary Accretion Torus & Outflow Cavity
// 5: Population III Lyman-Alpha Ionization Bubble & Dark Star WIMP Annihilation Halo

#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
}

struct CircumstellarUniforms {
    center_and_outer_r: vec4<f32>,      // xyz = star world pos, w = outer envelope radius (AU)
    mode_and_time: vec4<f32>,           // x = inner star radius (AU), y = env_type, z = sub_mode, w = time (s)
    spin_axis_and_asymmetry: vec4<f32>, // xyz = unit 3D spin axis, w = bipolarity / asymmetry
    primary_color: vec4<f32>,           // rgba = inner ionized / reflection emission
    secondary_color: vec4<f32>,         // rgba = outer shock / soot / caustic emission
    kinematics: vec4<f32>,              // x = spiral/shell freq, y = clumping, z = optical density, w = pulse rate
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> env: CircumstellarUniforms;

const PI: f32 = 3.14159265359;

fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n000 = hash31(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash31(i + vec3<f32>(1.0, 0.0, 0.0) + vec3<f32>(0.0, 0.0, 1.0));
    let n011 = hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash31(i + vec3<f32>(1.0, 1.0, 1.0));

    let x00 = mix(n000, n100, u.x);
    let x10 = mix(n010, n110, u.x);
    let x01 = mix(n001, n101, u.x);
    let x11 = mix(n011, n111, u.x);

    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    var val = 0.0;
    var amp = 0.5;
    var pos = p;
    for (var i = 0; i < 4; i = i + 1) {
        val += amp * noise3(pos);
        pos = pos * 2.03 + vec3<f32>(13.7, 29.3, 17.1);
        amp *= 0.5;
    }
    return val;
}

fn ridge3(p: vec3<f32>) -> f32 {
    var val = 0.0;
    var amp = 0.5;
    var pos = p;
    for (var i = 0; i < 3; i = i + 1) {
        let n = noise3(pos);
        let r = 1.0 - abs(n * 2.0 - 1.0);
        val += amp * r * r;
        pos = pos * 2.05 + vec3<f32>(23.1, 11.7, 31.9);
        amp *= 0.5;
    }
    return val;
}

fn ray_sphere_intersect(ro: vec3<f32>, rd: vec3<f32>, center: vec3<f32>, radius: f32) -> vec2<f32> {
    let oc = ro - center;
    let b = dot(oc, rd);
    let c = dot(oc, oc) - radius * radius;
    let disc = b * b - c;
    if (disc < 0.0) {
        return vec2<f32>(-1.0, -1.0);
    }
    let s = sqrt(disc);
    return vec2<f32>(-b - s, -b + s);
}

fn to_spin_local(v: vec3<f32>, spin_axis: vec3<f32>) -> vec3<f32> {
    let up = normalize(spin_axis);
    let ref_vec = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(up.x) > 0.9);
    let right = normalize(cross(up, ref_vec));
    let fwd = cross(right, up);
    return vec3<f32>(dot(v, right), dot(v, up), dot(v, fwd));
}

/// Evaluates local volumetric density and emissive color at normalized position `p` (|p| in 0..1).
fn evaluate_circumstellar_volume(
    p: vec3<f32>,
    view_dir: vec3<f32>,
    env_type: u32,
    sub_mode: u32,
    t: f32,
) -> vec4<f32> {
    let r = length(p);
    if (r < 0.05 || r > 1.0) {
        return vec4<f32>(0.0);
    }
    let dir = p / r;
    let prim_col = env.primary_color.rgb;
    let sec_col = env.secondary_color.rgb;
    let freq = env.kinematics.x;
    let clump_amp = env.kinematics.y;
    let base_density = env.kinematics.z;
    let pulse_rate = env.kinematics.w;
    let edge_fade = smoothstep(1.0, 0.50, r) * smoothstep(0.18, 0.30, r);

    // =========================================================================
    // 0. LBV Homunculus Bipolar Eruption Nebula (Eta Carinae)
    // =========================================================================
    if (env_type == 0u) {
        let cos_th = abs(dir.y);
        let sin_th = sqrt(max(1.0 - dir.y * dir.y, 0.0));

        // Double prolate bipolar lobes centered at y = +/- 0.42
        let lobe_center = vec3<f32>(0.0, sign(p.y) * 0.42, 0.0);
        let d_lobe = length((p - lobe_center) * vec3<f32>(1.45, 1.05, 1.45));
        let shell_wall = exp(-pow((d_lobe - 0.44) / 0.095, 2.0));

        // Cauliflower Rayleigh-Taylor instability clumping on the Homunculus lobes
        let cauli = ridge3(p * freq - dir * (t * 0.18));
        let lobe_density = shell_wall * (0.45 + 0.85 * cauli) * smoothstep(0.08, 0.35, cos_th);

        // Pinched equatorial ragged dust skirt + radial ejecta spokes
        let eq_plane = exp(-pow(p.y / 0.055, 2.0)) * smoothstep(0.24, 0.45, r) * smoothstep(0.95, 0.55, r);
        let azim = atan2(p.z, p.x);
        let spokes = pow(sin(azim * 14.0 + cauli * 3.0) * 0.5 + 0.5, 2.5);
        let skirt_density = eq_plane * (0.55 + 0.75 * spokes);

        let total_d = (lobe_density + skirt_density * 1.15) * edge_fade * base_density;
        let inner_scatter = prim_col * smoothstep(0.75, 0.25, r) * 1.65;
        let outer_shock = sec_col * (cauli * 1.15 + skirt_density * 0.90);
        let col = mix(outer_shock, inner_scatter, smoothstep(0.68, 0.28, r));
        return vec4<f32>(col * 2.2, total_d);
    }

    // =========================================================================
    // 1. Wolf-Rayet Colliding-Wind Pinwheel Spiral Nebula (WR 104 / WR 140)
    // =========================================================================
    if (env_type == 1u) {
        let cyl_r = length(p.xz);
        let azim = atan2(p.z, p.x);
        let turb = fbm3(p * 6.5 - dir * (t * 0.35));

        // 2-arm Archimedean colliding-wind spiral plume in the orbital plane
        let spiral_wave = sin(azim * 2.0 - cyl_r * freq + t * pulse_rate + turb * 2.6) * 0.5 + 0.5;
        let spiral_arm = pow(spiral_wave, 3.2) * exp(-pow(p.y / (0.14 + r * 0.18), 2.0));

        // Outer wind-blown filamentary ring shell (NGC 6888 style)
        let ring_shell = exp(-pow((r - 0.76) / 0.085, 2.0)) * ridge3(p * 7.5 + vec3<f32>(0.0, t * 0.15, 0.0));

        let total_d = (spiral_arm * 1.25 + ring_shell * 0.75) * edge_fade * base_density;
        let col = mix(prim_col, sec_col, smoothstep(0.30, 0.82, r) * 0.75 + ring_shell * 0.35)
                * (1.25 + clump_amp * turb);
        return vec4<f32>(col * 2.35, total_d);
    }

    // =========================================================================
    // 2. AGB Carbon Star / Yellow Hypergiant / Thorne-Zytkow Detached Shells
    // =========================================================================
    if (env_type == 2u) {
        let turb = fbm3(p * 5.2 - dir * (t * 0.12));
        let ridge_clump = ridge3(p * 6.8 + vec3<f32>(t * 0.08, 0.0, 0.0));

        // 3 concentric thermal-pulse / mass-ejection detached shells
        let shell_wave = pow(sin(r * freq - t * pulse_rate + turb * 2.2) * 0.5 + 0.5, 3.6);
        var shell_d = shell_wave * (0.45 + 0.75 * ridge_clump) * edge_fade * base_density;
        var col = mix(sec_col, prim_col, smoothstep(0.85, 0.25, r)) * (0.95 + 0.65 * ridge_clump);

        // Sub-mode 2: Thorne-Zytkow Object — buried neutron star X-ray ionization cones breaking out!
        if (sub_mode == 2u) {
            let polar_cone = smoothstep(0.76, 0.96, abs(dir.y));
            let xray_pulse = 0.65 + 0.55 * pow(sin(t * 6.5 - r * 8.0) * 0.5 + 0.5, 3.0);
            let xray_col = vec3<f32>(0.45, 1.35, 2.35) * polar_cone * xray_pulse * 2.4;
            col += xray_col;
            shell_d += polar_cone * xray_pulse * edge_fade * 0.65;
        }
        return vec4<f32>(col * 2.0, shell_d);
    }

    // =========================================================================
    // 3. Boson Star & Quark Star Relativistic Lensing Ring & Electrosphere
    // =========================================================================
    if (env_type == 3u) {
        // Impact parameter perpendicular to camera line of sight
        let p_dot_v = dot(p, view_dir);
        let p_perp = p - p_dot_v * view_dir;
        let b = length(p_perp);

        // Relativistic Einstein Photon Ring at b ~ 0.46
        let einstein_ring = exp(-pow((b - 0.46) / 0.048, 2.0)) * exp(-pow(p_dot_v / 0.22, 2.0));
        let inner_chr = exp(-pow((b - 0.43) / 0.040, 2.0)) * exp(-pow(p_dot_v / 0.22, 2.0));
        let outer_chr = exp(-pow((b - 0.49) / 0.040, 2.0)) * exp(-pow(p_dot_v / 0.22, 2.0));

        if (sub_mode == 0u) {
            // Boson Star: Concentric 3D Klein-Gordon scalar standing-wave interference shells
            let scalar_wave = pow(sin(r * freq - t * pulse_rate) * 0.5 + 0.5, 3.0) * exp(-r * 1.8);
            let d = (einstein_ring * 1.45 + scalar_wave * 0.55) * smoothstep(1.0, 0.72, r) * base_density;
            let disp_col = vec3<f32>(
                outer_chr * 0.95 + einstein_ring * 0.65,
                einstein_ring * 1.45 + scalar_wave * 0.95,
                inner_chr * 1.75 + scalar_wave * 1.25
            );
            return vec4<f32>(disp_col * 2.25, d);
        } else {
            // Quark Star: Schwinger pair-production electrosphere lightning & bipolar X-ray arcs
            let arc_noise = ridge3(p * 9.5 + vec3<f32>(t * 1.6, -t * 1.2, 0.0));
            let dipole_shell = exp(-pow((r - (0.35 + 0.42 * (1.0 - dir.y * dir.y))) / 0.075, 2.0));
            let lightning = smoothstep(0.64, 0.92, arc_noise) * dipole_shell;
            let d = (einstein_ring * 1.20 + lightning * 1.35) * edge_fade * base_density;
            let col = prim_col * einstein_ring * 1.6 + sec_col * lightning * 2.2;
            return vec4<f32>(col * 2.4, d);
        }
    }

    // =========================================================================
    // 4. Herbig Ae/Be & T Tauri Flared Protoplanetary Accretion Envelope
    // =========================================================================
    if (env_type == 4u) {
        let cyl_r = max(length(p.xz), 0.05);
        let azim = atan2(p.z, p.x);
        // Flared disk scale height H(R) = 0.08 * R^1.25
        let h_scale = 0.08 + 0.22 * pow(cyl_r, 1.25);
        let torus = exp(-pow(p.y / h_scale, 2.0)) * smoothstep(0.24, 0.38, cyl_r);
        let spiral = sin(azim * 3.0 - cyl_r * freq + t * 0.6 + fbm3(p * 6.0) * 2.5) * 0.5 + 0.5;

        // Bipolar Herbig-Haro outflow cone walls
        let cone_angle = abs(abs(dir.y) - 0.78);
        let outflow_wall = exp(-pow(cone_angle / 0.065, 2.0)) * ridge3(p * 7.0 - dir * (t * 0.45));

        let d = (torus * (0.55 + 0.65 * spiral) + outflow_wall * 0.60) * edge_fade * base_density;
        let inner_wall_glow = prim_col * smoothstep(0.60, 0.24, cyl_r) * 1.75;
        let dust_body = sec_col * (0.65 + 0.55 * spiral);
        let col = mix(dust_body, inner_wall_glow, smoothstep(0.55, 0.26, cyl_r)) + prim_col * outflow_wall * 1.2;
        return vec4<f32>(col * 2.0, d);
    }

    // =========================================================================
    // 5. Population III Ionization Bubble & Dark Star WIMP Annihilation Halo
    // =========================================================================
    let filaments = ridge3(p * freq - dir * (t * 0.25));
    let shock_boundary = exp(-pow((r - 0.80) / 0.095, 2.0));
    let inner_caustic = pow(filaments, 2.2) * exp(-r * 1.2);
    let d = (shock_boundary * (0.55 + 0.80 * filaments) + inner_caustic * 0.85) * edge_fade * base_density;
    let col = mix(prim_col, sec_col, shock_boundary * 0.75 + (1.0 - r) * 0.25) * (1.1 + 0.8 * filaments);
    return vec4<f32>(col * 2.3, d);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let cam_pos = view.world_position;
    let world_pos = in.world_position.xyz;
    let ray_vec = world_pos - cam_pos;
    let ray_len = length(ray_vec);
    if (ray_len < 1e-5) {
        out.color = vec4<f32>(0.0);
        return out;
    }
    let rd = ray_vec / ray_len;

    let center = env.center_and_outer_r.xyz;
    let r_outer = max(env.center_and_outer_r.w, 0.001);
    let r_inner = env.mode_and_time.x;
    let env_type = u32(env.mode_and_time.y + 0.5);
    let sub_mode = u32(env.mode_and_time.z + 0.5);
    let t = env.mode_and_time.w;

    let outer_hit = ray_sphere_intersect(cam_pos, rd, center, r_outer);
    if (outer_hit.y <= 0.0) {
        out.color = vec4<f32>(0.0);
        return out;
    }

    let t_start = max(outer_hit.x, 0.0);
    var t_end = outer_hit.y;

    // For opaque central stars (all except translucent Boson Star sub_mode 0 of env_type 3),
    // stop the raymarch where the ray hits the star's front photosphere!
    let is_translucent_soliton = (env_type == 3u && sub_mode == 0u);
    if (!is_translucent_soliton) {
        let inner_hit = ray_sphere_intersect(cam_pos, rd, center, r_inner * 0.98);
        if (inner_hit.x > 0.0) {
            t_end = min(t_end, inner_hit.x);
        }
    }

    let march_dist = t_end - t_start;
    if (march_dist <= 1e-5) {
        out.color = vec4<f32>(0.0);
        return out;
    }

    let spin_axis = normalize(select(vec3<f32>(0.0, 1.0, 0.0), env.spin_axis_and_asymmetry.xyz, length(env.spin_axis_and_asymmetry.xyz) > 0.1));
    let view_local = to_spin_local(-rd, spin_axis);

    let steps = 14;
    let dt = march_dist / f32(steps);
    let dt_norm = dt / r_outer;

    var accum_rgb = vec3<f32>(0.0);
    var transmittance = 1.0;

    for (var i = 0; i < steps; i = i + 1) {
        let sample_t = t_start + (f32(i) + 0.5) * dt;
        let p_world = cam_pos + rd * sample_t;
        let p_rel_norm = (p_world - center) / r_outer;
        let p_local = to_spin_local(p_rel_norm, spin_axis);

        let sample_val = evaluate_circumstellar_volume(p_local, view_local, env_type, sub_mode, t);
        let sample_density = sample_val.a * dt_norm * 1.90;
        if (sample_density > 0.0005) {
            let step_alpha = 1.0 - exp(-sample_density);
            accum_rgb += transmittance * sample_val.rgb * step_alpha;
            transmittance *= (1.0 - step_alpha);
            if (transmittance < 0.03) {
                break;
            }
        }
    }

    let final_alpha = clamp(1.0 - transmittance, 0.0, 0.58);
    out.color = vec4<f32>(accum_rgb, final_alpha);
    return out;
}
