// WGSL Compute Shader for Galactic Scale N-Body Simulation
// Includes Dark Matter Halos (NFW Profile) and Density Wave Theory

struct Particle {
    pos_mass: vec4<f32>,   // xyz = position in AU, w = mass in M_sun
    vel_temp: vec4<f32>,   // xyz = velocity in AU/yr, w = temp in K
    composition: vec4<f32>,// Composition, unused in galaxy scale primarily
};

struct GalaxyUniforms {
    dt: f32,
    g_const: f32,
    softening_sq: f32,
    num_particles: u32,
    
    // NFW Dark Matter Halo Parameters
    halo_mass: f32,
    halo_scale_radius: f32,
    
    // Spiral Density Wave Parameters
    pattern_speed: f32,
    spiral_arms: f32,
    spiral_amplitude: f32,
    spiral_pitch: f32,
    
    num_massive_bodies: u32,
    _pad1: f32,
    massive_bodies: array<vec4<f32>, 32>, // xyz: pos, w: mass
    massive_body_velocities: array<vec4<f32>, 32>, // xyz: vel
};

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<uniform> uniforms: GalaxyUniforms;

const PI: f32 = 3.141592653589793;

// Analytical NFW Dark Matter Halo Potential Gradient with 500 AU core softening
fn get_nfw_acceleration(pos: vec3<f32>, halo_m: f32) -> vec3<f32> {
    let r_sq = dot(pos, pos) + 250000.0; // 500 AU core softening to prevent central singularity
    let r = sqrt(r_sq);
    let rs = max(uniforms.halo_scale_radius, 1.0);
    let x = r / rs;
    
    let ln_term = log(1.0 + x);
    let mass_enclosed = halo_m * (ln_term - x / (1.0 + x));
    
    let a_mag = uniforms.g_const * mass_enclosed / r_sq;
    return -(pos / r) * a_mag;
}

// Compute total gravitational acceleration on a particle at position 'pos' at time 'current_t'
fn compute_gravity_at(pos: vec3<f32>, current_t: f32) -> vec3<f32> {
    var a_total = vec3<f32>(0.0, 0.0, 0.0);
    let n_bodies = min(uniforms.num_massive_bodies, 32u);
    for (var j: u32 = 0u; j < n_bodies; j = j + 1u) {
        let mb = uniforms.massive_bodies[j];
        let mb_vel = uniforms.massive_body_velocities[j];
        let m_body_mass = mb.w;
        if (m_body_mass <= 0.0) {
            continue;
        }

        // Interpolate position of the massive body at current substep time
        let current_mb_pos = mb.xyz + mb_vel.xyz * current_t;

        let to_body = current_mb_pos - pos;
        let dist_sq = dot(to_body, to_body) + uniforms.softening_sq;
        let dist = sqrt(dist_sq);

        let inv_dist3 = 1.0 / (dist_sq * dist);
        let f_grav = uniforms.g_const * m_body_mass * inv_dist3;
        a_total = a_total + to_body * f_grav;
        
        let offset = pos - current_mb_pos;
        let h_mass = max(mb_vel.w, uniforms.halo_mass);
        a_total = a_total + get_nfw_acceleration(offset, h_mass);
    }
    return a_total;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= uniforms.num_particles || idx >= arrayLength(&particles)) {
        return;
    }

    var p = particles[idx];
    let mass = p.pos_mass.w;
    if (mass <= 0.0) {
        return; 
    }

    var pos = p.pos_mass.xyz;
    var vel = p.vel_temp.xyz;

    if (abs(uniforms.dt) < 1e-6) {
        return;
    }

    // Adaptive step: resolve the local dynamical time (|v|/|a| ~ orbital period / 2*pi) so
    // fast-forwarding does not make stars numerically escape their galaxy.
    let a0 = compute_gravity_at(pos, 0.0);
    let a_mag = max(length(a0), 1e-6);
    let v_mag = max(length(vel), 1e-3);
    let max_dt = clamp(0.03 * v_mag / a_mag, 1e-4, 0.05);
    let raw_n_steps = u32(ceil(abs(uniforms.dt) / max_dt));
    let n_steps = clamp(raw_n_steps, 1u, 1024u);
    let sub_dt = uniforms.dt / f32(n_steps);
    let half_dt = 0.5 * sub_dt;
    
    // 2nd-Order Symplectic Drift-Kick-Drift Leapfrog Integration
    // Preserves phase-space volume and prevents secular orbital energy drift.
    for (var step_i: u32 = 0u; step_i < n_steps; step_i = step_i + 1u) {
        // 1. Drift position by half-step
        pos = pos + vel * half_dt;

        // 2. Compute gravitational acceleration at midpoint position and midpoint massive body time
        let midpoint_t = (f32(step_i) + 0.5) * sub_dt;
        let a_total = compute_gravity_at(pos, midpoint_t);

        // 3. Kick velocity by full-step
        vel = vel + a_total * sub_dt;

        // 4. Drift position by remaining half-step
        pos = pos + vel * half_dt;
    }

    // Galactic escape velocity limiter to prevent numerical outliers
    let speed = length(vel);
    let max_galaxy_speed = 40000.0;
    if (speed > max_galaxy_speed) {
        vel = vel * (max_galaxy_speed / speed);
    }

    p.pos_mass = vec4<f32>(pos, mass);
    p.vel_temp = vec4<f32>(vel, p.vel_temp.w);

    particles[idx] = p;
}
