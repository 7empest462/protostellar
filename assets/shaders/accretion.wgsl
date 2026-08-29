// WGSL Planetesimal Accretion, Gravitational Fusion & Roche Disruption Compute Shader

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

    var p_i = particles[index];
    if (p_i.pos_mass.w <= 0.0) {
        return;
    }

    let pos_i = p_i.pos_mass.xyz;
    let vel_i = p_i.vel_radius.xyz;
    let mass_i = p_i.pos_mass.w;
    let rad_i = p_i.vel_radius.w;

    // 1. Check Collisions with Central Star & Roche Limit
    let star = uniforms.massive_bodies[0];
    let r_star_dist = length(pos_i - star.pos_mass.xyz);
    let star_radius = uniforms.star_params.x;

    // Swallowed by star
    if (r_star_dist < star_radius * 1.2) {
        p_i.pos_mass.w = 0.0;
        particles[index] = p_i;
        return;
    }

    // 2. Check Accretion onto Massive Planets
    let n_massive = min(uniforms.num_massive_bodies, 32u);
    for (var m: u32 = 1u; m < n_massive; m = m + 1u) {
        let planet = uniforms.massive_bodies[m];
        let p_mass = planet.pos_mass.w;
        if (p_mass > 0.0) {
            let dist_p = length(pos_i - planet.pos_mass.xyz);
            // Effective Hill sphere / capture radius
            let r_hill = max(pow(p_mass / (3.0 * star.pos_mass.w), 0.3333) * length(planet.pos_mass.xyz), 0.005);
            if (dist_p < r_hill * 0.15) {
                p_i.pos_mass.w = 0.0;
                particles[index] = p_i;
                return;
            }
        }
    }

    // 3. Planetesimal-Planetesimal Gravitational Fusion (Local Particle Window)
    let search_window = 32u;
    let start_idx = (index / search_window) * search_window;
    let end_idx = min(start_idx + search_window, uniforms.num_particles);

    for (var j: u32 = start_idx; j < end_idx; j = j + 1u) {
        if (j == index) {
            continue;
        }

        let p_j = particles[j];
        let mass_j = p_j.pos_mass.w;
        if (mass_j <= 0.0) {
            continue;
        }

        let pos_j = p_j.pos_mass.xyz;
        let diff = pos_i - pos_j;
        let dist = length(diff);

        let rad_j = p_j.vel_radius.w;
        let vel_j = p_j.vel_radius.xyz;
        let v_rel = length(vel_i - vel_j);

        // Safronov Gravitational Focusing Cross-Section
        let v_esc = sqrt(2.0 * uniforms.g_const * (mass_i + mass_j) / max(rad_i + rad_j, 1.0e-5));
        let focusing_factor = sqrt(1.0 + (v_esc * v_esc) / max(v_rel * v_rel, 0.001));
        let r_capture = (rad_i + rad_j) * uniforms.accretion_multiplier * focusing_factor;

        if (dist < r_capture && v_rel < v_esc * 1.4) {
            // Collision and Fusion! Lower index absorbs the higher index
            if (index < j) {
                // p_i absorbs p_j
                let total_mass = mass_i + mass_j;
                let new_vel = (vel_i * mass_i + vel_j * mass_j) / total_mass;
                let new_rad = rad_i * pow(total_mass / mass_i, 0.3333);

                // Blend composition
                let comp_i = p_i.temp_comp.yzw;
                let comp_j = p_j.temp_comp.yzw;
                let new_comp = (comp_i * mass_i + comp_j * mass_j) / total_mass;

                p_i.pos_mass.w = total_mass;
                p_i.vel_radius = vec4<f32>(new_vel, min(new_rad, 0.05));
                p_i.temp_comp = vec4<f32>(p_i.temp_comp.x, new_comp.x, new_comp.y, new_comp.z);
            } else {
                // p_i is absorbed by p_j
                p_i.pos_mass.w = 0.0;
                break;
            }
        }
    }

    particles[index] = p_i;
}
