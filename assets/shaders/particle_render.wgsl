// WGSL Instanced Protoplanetary Particle Billboard Vertex & Fragment Shader

struct ViewUniforms {
    view_proj: mat4x4<f32>,
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
};

struct Particle {
    pos_mass: vec4<f32>,   // x, y, z in AU, w = mass in M_sun
    vel_radius: vec4<f32>, // vx, vy, vz in AU/yr, w = radius in AU
    temp_comp: vec4<f32>,  // x = Temp K, y = silicate, z = ice, w = metal
};

@group(0) @binding(0) var<uniform> view: ViewUniforms;
@group(0) @binding(1) var<storage, read> particles: array<Particle>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) temperature: f32,
    @location(2) mass: f32,
    @location(3) composition: vec3<f32>, // rock, ice, metal
};

// Planck Blackbody approximation to sRGB
fn blackbody_color(temp_k: f32) -> vec3<f32> {
    let t = clamp(temp_k / 100.0, 10.0, 400.0);

    var r: f32 = 255.0;
    if (t > 66.0) {
        let x = t - 60.0;
        r = clamp(329.698727446 * pow(x, -0.1332047592), 0.0, 255.0);
    }

    var g: f32 = 0.0;
    if (t <= 66.0) {
        g = clamp(99.4708025861 * log(t) - 161.1195681661, 0.0, 255.0);
    } else {
        let x = t - 60.0;
        g = clamp(288.1221695283 * pow(x, -0.0755148492), 0.0, 255.0);
    }

    var b: f32 = 255.0;
    if (t <= 19.0) {
        b = 0.0;
    } else if (t < 66.0) {
        let x = t - 10.0;
        b = clamp(138.5177312231 * log(x) - 305.0447927307, 0.0, 255.0);
    }

    return vec3<f32>(r / 255.0, g / 255.0, b / 255.0);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let p = particles[instance_index];
    let mass = p.pos_mass.w;
    let pos_world = p.pos_mass.xyz;
    let temp_k = p.temp_comp.x;

    var out: VertexOutput;

    // Cull absorbed / dead particles by outputting zero-area clip coords
    if (mass <= 0.0) {
        out.clip_position = vec4<f32>(0.0, 0.0, 2.0, 1.0); // Outside clip volume
        out.uv = vec2<f32>(0.0, 0.0);
        out.temperature = 0.0;
        out.mass = 0.0;
        out.composition = vec3<f32>(0.0, 0.0, 0.0);
        return out;
    }

    // Standard 6-vertex quad: (0, 1, 2) and (2, 3, 0)
    var uv_offsets = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );

    let uv = uv_offsets[vertex_index % 6u];

    // Scale billboard size dynamically with mass growth & temperature glow
    let individual_mass = 1.0e-7; // Base particle mass
    let mass_ratio = max(mass / individual_mass, 1.0);
    let mass_factor = pow(mass_ratio, 0.3333);

    // Fine microscopic dust (~0.0035 AU) vs Growing Planetesimals (~0.015 - 0.04 AU)
    let base_size = clamp(0.0035 * mass_factor, 0.0025, 0.040);
    let glow_scale = 1.0 + clamp((temp_k - 800.0) / 1200.0, 0.0, 1.5);
    let size = base_size * glow_scale;

    // Billboard displacement oriented toward camera
    let right = view.camera_right.xyz * (uv.x * size * 0.5);
    let up = view.camera_up.xyz * (uv.y * size * 0.5);
    let vertex_world = pos_world + right + up;

    out.clip_position = view.view_proj * vec4<f32>(vertex_world, 1.0);
    out.uv = uv;
    out.temperature = temp_k;
    out.mass = mass;
    out.composition = p.temp_comp.yzw;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.mass <= 0.0) {
        discard;
    }

    let dist_sq = dot(in.uv, in.uv);
    if (dist_sq > 1.0) {
        discard;
    }

    // Radial Gaussian soft glow profile
    let glow = exp(-3.5 * dist_sq);
    let base_rgb = blackbody_color(in.temperature);

    // Emissive intensity multiplier for hot inner disk and massive planetesimals
    let mass_boost = clamp(in.mass * 1.0e5, 1.0, 3.0);
    let emissive_boost = (1.0 + clamp((in.temperature - 500.0) / 500.0, 0.0, 4.0)) * mass_boost;
    let final_rgb = base_rgb * (glow * emissive_boost);
    let alpha = glow * 0.85;

    return vec4<f32>(final_rgb, alpha);
}
