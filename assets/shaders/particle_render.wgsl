// WGSL Instanced GPU Vertex-Pulling Particle Billboard Shader (500k Zero-Readback Pipeline)
// Pulls particle positions, masses, temperatures, and compositions directly from the active
// GPU compute storage buffer (`particle_orbit.wgsl` or `nbody_galaxy.wgsl`) with zero vertex
// buffers, zero CPU billboard rebuilds, and zero 24 MB/frame GPU -> CPU readback.

struct GpuParticlePullUniforms {
    clip_from_world: mat4x4<f32>,
    camera_right_and_base_r: vec4<f32>,  // xyz: camera right, w: base half-radius (AU)
    camera_up_and_scale: vec4<f32>,      // xyz: camera up, w: global particle_render_scale
    camera_pos_and_base_mass: vec4<f32>, // xyz: camera world pos (AU), w: base_mass (M_sun)
    mode_params: vec4<f32>,              // x: is_galaxy (0/1), y: num_particles, z: alpha_mult, w: reserved
};

struct Particle {
    pos_mass: vec4<f32>,    // xyz: position in AU, w: mass in M_sun (<= 0 means dead/absorbed)
    vel_temp: vec4<f32>,    // xyz: velocity in AU/yr, w: temp in K
    composition: vec4<f32>, // Disk: (silicate, ice, metal, gas); Galaxy: (r, g, b, 1.0)
};

@group(0) @binding(0) var<uniform> pull: GpuParticlePullUniforms;
@group(0) @binding(1) var<storage, read> particles: array<Particle>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

// Planck Blackbody approximation to sRGB matching CPU `blackbody_to_srgb`
fn blackbody_color(temp_k: f32) -> vec3<f32> {
    let t = clamp(temp_k / 100.0, 10.0, 400.0);

    var r: f32 = 255.0;
    if (t > 66.0) {
        let x = max(t - 60.0, 1.0);
        r = clamp(329.698727446 * pow(x, -0.1332047592), 0.0, 255.0);
    }

    var g: f32 = 0.0;
    if (t <= 66.0) {
        g = clamp(99.4708025861 * log(max(t, 1.0)) - 161.1195681661, 0.0, 255.0);
    } else {
        let x = max(t - 60.0, 1.0);
        g = clamp(288.1221695283 * pow(x, -0.0755148492), 0.0, 255.0);
    }

    var b: f32 = 255.0;
    if (t <= 19.0) {
        b = 0.0;
    } else if (t < 66.0) {
        let x = max(t - 10.0, 1.0);
        b = clamp(138.5177312231 * log(x) - 305.0447927307, 0.0, 255.0);
    }

    return vec3<f32>(r / 255.0, g / 255.0, b / 255.0);
}

// Mirrors `Composition::visual_color_tint` + `calculate_particle_color` from `setup.rs`
fn evaluate_disk_particle_rgb(comp: vec4<f32>, temp_k: f32) -> vec3<f32> {
    let bb = blackbody_color(temp_k);
    let sil = max(comp.x, 0.0);
    let ice = max(comp.y, 0.0);
    let met = max(comp.z, 0.0);
    let gas = max(comp.w, 0.0);

    if (gas > 0.95 && met == 0.0) {
        // JWST Little Red Dot pristine primordial hydrogen cocoon: deep ruby-crimson luminescence
        return vec3<f32>(
            clamp(bb.r * 0.40 + 0.95, 0.6, 1.8),
            clamp(bb.g * 0.15 + 0.18, 0.1, 0.45),
            clamp(bb.b * 0.15 + 0.12, 0.05, 0.35),
        );
    }
    if (gas > 0.35) {
        // Primordial gaseous envelope: ethereal cyan-blue glow
        return vec3<f32>(
            clamp(bb.r * 0.20 + 0.35, 0.2, 1.2),
            clamp(bb.g * 0.20 + 0.75, 0.4, 1.4),
            clamp(bb.b * 0.20 + 1.10, 0.6, 1.5),
        );
    }

    let org = max(1.0 - (sil + ice + met + gas), 0.0);
    let comp_tint = sil * vec3<f32>(0.76, 0.64, 0.50)
        + ice * vec3<f32>(0.72, 0.88, 1.00)
        + met * vec3<f32>(0.68, 0.70, 0.74)
        + org * vec3<f32>(0.52, 0.40, 0.32)
        + gas * vec3<f32>(0.60, 0.82, 0.98);
    let tint_sum = max(sil + ice + met + org + gas, 0.001);
    let norm_tint = comp_tint / tint_sum;
    return vec3<f32>(
        clamp(bb.r * 0.45 + norm_tint.r * 0.85, 0.4, 1.4),
        clamp(bb.g * 0.45 + norm_tint.g * 0.85, 0.35, 1.4),
        clamp(bb.b * 0.45 + norm_tint.b * 0.85, 0.3, 1.4),
    );
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    if (instance_index >= arrayLength(&particles)) {
        out.clip_position = vec4<f32>(0.0, 0.0, 2.0, 1.0);
        out.uv = vec2<f32>(0.0, 0.0);
        out.color = vec4<f32>(0.0);
        return out;
    }

    let p = particles[instance_index];
    let mass = p.pos_mass.w;
    if (mass <= 0.0) {
        // Cull absorbed / dormant particles outside clip volume
        out.clip_position = vec4<f32>(0.0, 0.0, 2.0, 1.0);
        out.uv = vec2<f32>(0.0, 0.0);
        out.color = vec4<f32>(0.0);
        return out;
    }

    let pos_world = p.pos_mass.xyz;
    let temp_k = p.vel_temp.w;
    let is_galaxy = pull.mode_params.x > 0.5;
    let base_render_r = pull.camera_right_and_base_r.w;
    let render_scale = max(pull.camera_up_and_scale.w, 0.0);
    let b_mass = pull.camera_pos_and_base_mass.w;

    // 6-vertex quad covering [-1, 1]^2
    var uv_offsets = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );
    let uv = uv_offsets[vertex_index % 6u];

    // Mass-scaled billboard radius matching `billboards.rs` + near-camera anti-overdraw cap
    let m_factor = select(
        clamp(pow(max(mass / max(b_mass, 1e-12), 1.0), 0.3333333), 1.0, 1.35),
        1.0,
        is_galaxy || b_mass <= 0.0,
    );
    let raw_half_size = base_render_r * m_factor * render_scale;
    let dist_to_cam = length(pos_world - pull.camera_pos_and_base_mass.xyz);
    let half_size = min(raw_half_size, dist_to_cam * 0.022);
    let near_cam_fade = smoothstep(0.003, 0.032, dist_to_cam);

    let cam_right = pull.camera_right_and_base_r.xyz;
    let cam_up = pull.camera_up_and_scale.xyz;
    let vertex_world = pos_world + (cam_right * uv.x + cam_up * uv.y) * half_size;

    // Color & alpha matching `setup.rs` / `billboards.rs`
    var rgb: vec3<f32>;
    var base_alpha: f32;
    if (is_galaxy) {
        if (p.composition.w > 0.5) {
            rgb = p.composition.rgb;
        } else {
            let half_n = pull.mode_params.y * 0.5;
            let is_mw = f32(instance_index) < half_n;
            rgb = select(vec3<f32>(0.85, 0.92, 1.20), vec3<f32>(1.0, 0.95, 0.85), is_mw);
        }
        base_alpha = 0.85;
    } else {
        rgb = evaluate_disk_particle_rgb(p.composition, temp_k);
        if (mass <= 1.1 * b_mass) {
            base_alpha = 0.38;
        } else if (mass <= 3.0 * b_mass) {
            base_alpha = 0.62;
        } else if (mass <= 8.0 * b_mass) {
            base_alpha = 0.85;
        } else {
            base_alpha = 1.0;
        }
    }

    out.clip_position = pull.clip_from_world * vec4<f32>(vertex_world, 1.0);
    out.uv = uv;
    out.color = vec4<f32>(rgb, base_alpha * near_cam_fade * pull.mode_params.z);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist_sq = dot(in.uv, in.uv);
    if (dist_sq > 1.0 || in.color.a <= 0.001) {
        discard;
    }
    let falloff = exp(-2.2 * dist_sq);
    let edge_fade = max(1.0 - dist_sq * dist_sq, 0.0);
    let alpha = in.color.a * falloff * edge_fade;
    return vec4<f32>(in.color.rgb, alpha);
}
