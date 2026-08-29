// WGSL Symplectic Leapfrog Integration & Thermal Update Compute Shader

struct Particle {
    pos_mass: vec4<f32>,   // x, y, z in AU, w = mass in M_sun
    vel_radius: vec4<f32>, // vx, vy, vz in AU/yr, w = radius in AU
    temp_comp: vec4<f32>,  // x = Temp K, y = silicate, z = ice, w = metal
};

struct MassiveBody {
    pos_mass: vec4<f32>,
};

struct SimUniforms {
    g_const: f32,
    softening_sq: f32,
    num_particles: u32,
    dt: f32,
    num_massive_bodies: u32,
    enable_gas_drag: u32,
    accretion_multiplier: f32,
    gas_density_scale: f32,
    star_params: vec4<f32>, // radius, temp, lum, shockwave_r
    tractor_pos_mass: vec4<f32>,
    massive_bodies: array<MassiveBody, 32>,
};

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<storage, read_write> accelerations: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> uniforms: SimUniforms;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= uniforms.num_particles) {
        return;
    }

    var p = particles[index];
    if (p.pos_mass.w <= 0.0) {
        return;
    }

    let acc = accelerations[index].xyz;
    let dt = uniforms.dt;

    // 1. Leapfrog update with Continuous Solar Wind & Ignition Shockwave
    let r_initial = max(length(p.pos_mass.xyz), 0.05);
    let dir = p.pos_mass.xyz / r_initial;
    let r_sq = r_initial * r_initial;
    
    // Continuous Solar Radiation Pressure & Stellar Wind Force
    // Protostellar T-Tauri wind: gentle radiative pressure keeping particles in stable orbits
    let beta = 0.001;
    let solar_wind_push = (uniforms.g_const * beta / (r_sq + 0.1));
    var total_rad_accel = dir * solar_wind_push;

    // Expanding Ignition Blast Wave (only active during fusion onset!)
    let shockwave_r = uniforms.star_params.w;
    if (shockwave_r > 0.0 && abs(r_initial - shockwave_r) < 1.0) {
        let shock_intensity = max(0.0, 1.0 - (abs(r_initial - shockwave_r) / 1.0));
        let blast_push = (5.0 * uniforms.g_const / (r_sq + 0.1)) * shock_intensity;
        total_rad_accel = total_rad_accel + dir * blast_push;
    }

    let new_vel = p.vel_radius.xyz + (acc + total_rad_accel) * dt;
    let new_pos = p.pos_mass.xyz + new_vel * dt;

    // 2. Radiative Thermodynamics on GPU
    let r = max(length(new_pos), 0.1);
    let star_radius = uniforms.star_params.x;
    let star_temp = uniforms.star_params.y;
    let star_lum = uniforms.star_params.z;

    let equilibrium_temp = star_temp * sqrt(star_radius / (2.0 * r)) * pow(star_lum, 0.25);

    var shock_boost = 0.0;
    if (shockwave_r > 0.0 && abs(r - shockwave_r) < 2.0) {
        shock_boost = 400.0 * (1.0 - abs(r - shockwave_r) / 2.0);
    }

    let new_temp = clamp(equilibrium_temp + shock_boost, 30.0, 4000.0);

    p.pos_mass = vec4<f32>(new_pos, p.pos_mass.w);
    p.vel_radius = vec4<f32>(new_vel, p.vel_radius.w);
    p.temp_comp = vec4<f32>(new_temp, p.temp_comp.y, p.temp_comp.z, p.temp_comp.w);

    particles[index] = p;
}
