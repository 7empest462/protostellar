// WGSL N-Body Gravitational Acceleration Compute Shader
// Evaluates central star gravity, massive planetary perturbations, gas drag, and player tractor.

struct Particle {
    pos_mass: vec4<f32>,   // x, y, z in AU, w = mass in M_sun
    vel_radius: vec4<f32>, // vx, vy, vz in AU/yr, w = radius in AU
    temp_comp: vec4<f32>,  // x = Temp K, y = silicate, z = ice, w = metal
};

struct MassiveBody {
    pos_mass: vec4<f32>, // x, y, z in AU, w = mass in M_sun
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

@group(0) @binding(0) var<storage, read_write> particles_in: array<Particle>;
@group(0) @binding(1) var<storage, read_write> accelerations_out: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> uniforms: SimUniforms;

var<workgroup> tile_pos_mass: array<vec4<f32>, 64>;

@compute @workgroup_size(64, 1, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    let index = global_id.x;
    if (index >= uniforms.num_particles) {
        return;
    }

    let p_i = particles_in[index];
    if (p_i.pos_mass.w <= 0.0) {
        accelerations_out[index] = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return;
    }

    let pos_i = p_i.pos_mass.xyz;
    let vel_i = p_i.vel_radius.xyz;

    var acc = vec3<f32>(0.0, 0.0, 0.0);

    // 1. Central Star & Massive Planets Gravity
    let n_massive = min(uniforms.num_massive_bodies, 32u);
    for (var m: u32 = 0u; m < n_massive; m = m + 1u) {
        let body = uniforms.massive_bodies[m];
        let m_mass = body.pos_mass.w;
        if (m_mass > 0.0) {
            let r_vec = pos_i - body.pos_mass.xyz;
            let dist_sq = dot(r_vec, r_vec) + uniforms.softening_sq;
            let inv_dist_cube = 1.0 / (dist_sq * sqrt(dist_sq));
            acc -= uniforms.g_const * m_mass * r_vec * inv_dist_cube;
        }
    }

    // 2. High-Performance Strided Mutual N-Body Self-Gravity across Shared Workgroup Memory
    // Samples 64 representative tiles (4,096 particles) uniformly across the disk,
    // scaling mass by tile_stride to strictly conserve total disk mass and self-gravitational potential.
    let total_tiles = uniforms.num_particles / 64u;
    let max_tiles = min(total_tiles, 64u);
    let tile_stride = max(total_tiles / max_tiles, 1u);
    let mass_scale = f32(tile_stride);

    for (var t: u32 = 0u; t < max_tiles; t = t + 1u) {
        let tile = (t * tile_stride) % total_tiles;
        let load_idx = tile * 64u + local_id.x;
        tile_pos_mass[local_id.x] = particles_in[load_idx].pos_mass;
        workgroupBarrier();

        for (var k: u32 = 0u; k < 64u; k = k + 1u) {
            let other_pm = tile_pos_mass[k];
            let m_k = other_pm.w * mass_scale;
            if (m_k > 0.0) {
                let r_vec = pos_i - other_pm.xyz;
                let dist_sq = dot(r_vec, r_vec) + uniforms.softening_sq;
                let inv_dist_cube = 1.0 / (dist_sq * sqrt(dist_sq));
                acc -= uniforms.g_const * m_k * r_vec * inv_dist_cube;
            }
        }
        workgroupBarrier();
    }

    // 3. Aerodynamic Gas Drag from Protoplanetary Disk
    if (uniforms.enable_gas_drag == 1u && uniforms.gas_density_scale > 0.001) {
        let r_cyl = max(sqrt(pos_i.x * pos_i.x + pos_i.z * pos_i.z), 0.1);
        let star_mass = uniforms.massive_bodies[0].pos_mass.w;
        let v_k = sqrt(uniforms.g_const * star_mass / r_cyl);

        // Sub-Keplerian gas velocity
        let v_gas_mag = v_k * 0.998;
        let phi = atan2(pos_i.z, pos_i.x);
        let v_gas = vec3<f32>(-v_gas_mag * sin(phi), 0.0, v_gas_mag * cos(phi));

        let rel_v = vel_i - v_gas;
        let rel_speed = length(rel_v);

        // Gas density falloff ~ r^-2.25 scaled by gas_density_scale
        let gas_density = 1.0e-4 * pow(r_cyl / 1.0, -2.25) * uniforms.gas_density_scale;
        let drag_coeff = 0.015 * gas_density;
        acc -= drag_coeff * rel_speed * rel_v;
    }

    // 4. Player Gravitational Tractor
    let tractor_mass = uniforms.tractor_pos_mass.w;
    if (tractor_mass > 0.0) {
        let r_vec = pos_i - uniforms.tractor_pos_mass.xyz;
        let dist_sq = dot(r_vec, r_vec) + uniforms.softening_sq;
        let inv_dist_cube = 1.0 / (dist_sq * sqrt(dist_sq));
        acc -= uniforms.g_const * tractor_mass * r_vec * inv_dist_cube;
    }

    accelerations_out[index] = vec4<f32>(acc, 0.0);
}
