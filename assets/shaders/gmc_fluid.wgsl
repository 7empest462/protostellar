// WGSL Compute Shader for 3D Eulerian Giant Molecular Cloud (GMC) Fluid Dynamics & Jeans Collapse
// Simulates a 96x96x96 grid of gas density, supersonic turbulent velocity, and radiative cooling.

struct GpuSinkParticle {
    world_pos: vec3<f32>,
    sink_radius_au: f32,
    radiation_pressure_factor: f32,
    is_ignited: u32,
    _pad: vec2<u32>,
};

struct GpuJeansCollapseEvent {
    grid_coords: vec3<u32>,
    _pad0: u32,
    world_pos: vec3<f32>,
    local_mass_solar: f32,
    com_velocity: vec3<f32>,
    temperature_k: f32,
};

struct GmcFluidUniforms {
    domain_size: f32,
    grid_dim: u32,
    dt: f32,
    sound_speed: f32,
    damping: f32,
    num_sinks: u32,
    ambient_temp: f32,
    g_astro: f32,
    collapse_threshold: f32,
    vacuum_rate: f32,
    _pad0: f32,
    _pad1: f32,
    sinks: array<GpuSinkParticle, 16>,
};

@group(0) @binding(0) var<storage, read> density_in: array<f32>;
@group(0) @binding(1) var<storage, read_write> density_out: array<f32>;
@group(0) @binding(2) var<storage, read> velocity_in: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> velocity_out: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> temperature: array<f32>;
@group(0) @binding(5) var<uniform> uniforms: GmcFluidUniforms;
@group(0) @binding(6) var<storage, read_write> collapse_events: array<GpuJeansCollapseEvent>;
@group(0) @binding(7) var<storage, read_write> collapse_counter: atomic<u32>;

fn grid_idx(x: u32, y: u32, z: u32, dim: u32) -> u32 {
    return z * dim * dim + y * dim + x;
}

fn sample_d(x: i32, y: i32, z: i32, dim: i32) -> f32 {
    let cx = clamp(x, 0, dim - 1);
    let cy = clamp(y, 0, dim - 1);
    let cz = clamp(z, 0, dim - 1);
    let idx = grid_idx(u32(cx), u32(cy), u32(cz), u32(dim));
    return density_in[idx];
}

fn sample_v(x: i32, y: i32, z: i32, dim: i32) -> vec3<f32> {
    let cx = clamp(x, 0, dim - 1);
    let cy = clamp(y, 0, dim - 1);
    let cz = clamp(z, 0, dim - 1);
    let idx = grid_idx(u32(cx), u32(cy), u32(cz), u32(dim));
    return velocity_in[idx].xyz;
}

// Trilinear density sampling for Semi-Lagrangian advection
fn trilinear_density(pos_grid: vec3<f32>, dim: u32) -> f32 {
    let dim_i = i32(dim);
    let f_pos = clamp(pos_grid, vec3<f32>(0.0), vec3<f32>(f32(dim - 1u)));
    let i_pos = vec3<i32>(floor(f_pos));
    let frac = f_pos - vec3<f32>(i_pos);

    let d000 = sample_d(i_pos.x,     i_pos.y,     i_pos.z,     dim_i);
    let d100 = sample_d(i_pos.x + 1, i_pos.y,     i_pos.z,     dim_i);
    let d010 = sample_d(i_pos.x,     i_pos.y + 1, i_pos.z,     dim_i);
    let d110 = sample_d(i_pos.x + 1, i_pos.y + 1, i_pos.z,     dim_i);
    let d001 = sample_d(i_pos.x,     i_pos.y,     i_pos.z + 1, dim_i);
    let d101 = sample_d(i_pos.x + 1, i_pos.y,     i_pos.z + 1, dim_i);
    let d011 = sample_d(i_pos.x,     i_pos.y + 1, i_pos.z + 1, dim_i);
    let d111 = sample_d(i_pos.x + 1, i_pos.y + 1, i_pos.z + 1, dim_i);

    let dx00 = mix(d000, d100, frac.x);
    let dx10 = mix(d010, d110, frac.x);
    let dx01 = mix(d001, d101, frac.x);
    let dx11 = mix(d011, d111, frac.x);

    let dxy0 = mix(dx00, dx10, frac.y);
    let dxy1 = mix(dx01, dx11, frac.y);

    return mix(dxy0, dxy1, frac.z);
}

// Trilinear velocity sampling for Semi-Lagrangian advection
fn trilinear_velocity(pos_grid: vec3<f32>, dim: u32) -> vec3<f32> {
    let dim_i = i32(dim);
    let f_pos = clamp(pos_grid, vec3<f32>(0.0), vec3<f32>(f32(dim - 1u)));
    let i_pos = vec3<i32>(floor(f_pos));
    let frac = f_pos - vec3<f32>(i_pos);

    let v000 = sample_v(i_pos.x,     i_pos.y,     i_pos.z,     dim_i);
    let v100 = sample_v(i_pos.x + 1, i_pos.y,     i_pos.z,     dim_i);
    let v010 = sample_v(i_pos.x,     i_pos.y + 1, i_pos.z,     dim_i);
    let v110 = sample_v(i_pos.x + 1, i_pos.y + 1, i_pos.z,     dim_i);
    let v001 = sample_v(i_pos.x,     i_pos.y,     i_pos.z + 1, dim_i);
    let v101 = sample_v(i_pos.x + 1, i_pos.y,     i_pos.z + 1, dim_i);
    let v011 = sample_v(i_pos.x,     i_pos.y + 1, i_pos.z + 1, dim_i);
    let v111 = sample_v(i_pos.x + 1, i_pos.y + 1, i_pos.z + 1, dim_i);

    let vx00 = mix(v000, v100, frac.x);
    let vx10 = mix(v010, v110, frac.x);
    let vx01 = mix(v001, v101, frac.x);
    let vx11 = mix(v011, v111, frac.x);

    let vxy0 = mix(vx00, vx10, frac.y);
    let vxy1 = mix(vx01, vx11, frac.y);

    return mix(vxy0, vxy1, frac.z);
}

@compute @workgroup_size(8, 8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dim = uniforms.grid_dim;
    if (gid.x >= dim || gid.y >= dim || gid.z >= dim) {
        return;
    }

    let idx = grid_idx(gid.x, gid.y, gid.z, dim);
    let dx = uniforms.domain_size / f32(dim);
    let half_domain = uniforms.domain_size * 0.5;

    // World position of current cell center in AU
    let world_pos = (vec3<f32>(gid) + 0.5) * dx - half_domain;

    let v_curr = velocity_in[idx].xyz;
    let rho_curr = density_in[idx];
    let temp_curr = temperature[idx];

    // 1. Semi-Lagrangian Backtrack Advection
    let back_world = world_pos - v_curr * uniforms.dt;
    let back_grid = (back_world + half_domain) / dx - 0.5;

    var rho_adv = max(trilinear_density(back_grid, dim), 0.0);
    var v_adv = trilinear_velocity(back_grid, dim);

    // 2. Velocity Divergence & Compressible Continuity: D(rho)/Dt = -rho * div(v)
    let dim_i = i32(dim);
    let gx = i32(gid.x);
    let gy = i32(gid.y);
    let gz = i32(gid.z);

    let v_xp = sample_v(gx + 1, gy, gz, dim_i).x;
    let v_xm = sample_v(gx - 1, gy, gz, dim_i).x;
    let v_yp = sample_v(gx, gy + 1, gz, dim_i).y;
    let v_ym = sample_v(gx, gy - 1, gz, dim_i).y;
    let v_zp = sample_v(gx, gy, gz + 1, dim_i).z;
    let v_zm = sample_v(gx, gy, gz - 1, dim_i).z;

    let div_v = ((v_xp - v_xm) + (v_yp - v_ym) + (v_zp - v_zm)) / (2.0 * dx);

    // Compressible continuity: converging flows (div_v < 0) compress gas, amplifying density toward Jeans collapse
    rho_adv = rho_adv * exp(-clamp(div_v * uniforms.dt, -0.35, 0.35));

    // 3. Pressure Gradient via Central Differences
    let rho_xp = sample_d(gx + 1, gy, gz, dim_i);
    let rho_xm = sample_d(gx - 1, gy, gz, dim_i);
    let rho_yp = sample_d(gx, gy + 1, gz, dim_i);
    let rho_ym = sample_d(gx, gy - 1, gz, dim_i);
    let rho_zp = sample_d(gx, gy, gz + 1, dim_i);
    let rho_zm = sample_d(gx, gy, gz - 1, dim_i);

    let grad_rho = vec3<f32>(
        (rho_xp - rho_xm) / (2.0 * dx),
        (rho_yp - rho_ym) / (2.0 * dx),
        (rho_zp - rho_zm) / (2.0 * dx)
    );

    let c_s = uniforms.sound_speed;
    let a_pressure = -(c_s * c_s / max(rho_curr, 1e-15)) * grad_rho;

    // 4. Gravitational Acceleration (Cloud Global Core + Local Clump Self-Gravity + Stellar Sinks) & Radiation Pressure
    let r_len = length(world_pos);
    let r_core = 160.0;
    let r_core_sq = r_core * r_core;
    let g_cloud = (uniforms.g_astro * 24.0) / pow(r_len * r_len + r_core_sq, 1.5);
    var a_grav = -world_pos * g_cloud;

    // Local clump self-gravity: accelerates gas toward local density peaks (driving Jeans runaway collapse)
    let a_local_grav = (uniforms.g_astro * 2.5e6) * grad_rho;
    a_grav = a_grav + a_local_grav;
    var a_rad = vec3<f32>(0.0);

    for (var i = 0u; i < uniforms.num_sinks; i = i + 1u) {
        let sink = uniforms.sinks[i];
        let r_vec = sink.world_pos - world_pos;
        let dist = length(r_vec);
        let dist_safe = max(dist, 1.0);

        // Point-mass gravity towards sink (softened at 15 AU core)
        let g_force = (uniforms.g_astro * 1.5) / (dist_safe * dist_safe + 225.0);
        a_grav = a_grav + (r_vec / dist_safe) * g_force;

        // Stellar radiation pressure & ionization cavity clearing
        if (sink.is_ignited != 0u && dist < sink.sink_radius_au) {
            let outward = -r_vec / dist_safe;
            let rad_strength = sink.radiation_pressure_factor / (dist_safe * dist_safe + 10.0);
            a_rad = a_rad + outward * rad_strength;

            // Gas mass vacuuming / ionization cavity dispersal
            rho_adv = max(rho_adv - uniforms.vacuum_rate * uniforms.dt, 0.0);
        }
    }

    // 4. Update Velocity with Forces & Continuous Physical Turbulent Dissipation
    let total_accel = a_pressure + a_grav + a_rad;
    let damp_factor = exp(-uniforms.dt / 6000.0);
    var v_next = (v_adv + total_accel * uniforms.dt) * damp_factor;

    // Outflow boundary dampening and dissipation at domain edges to eliminate wall-reflection pileup
    let edge_dist = min(
        min(min(world_pos.x + half_domain, half_domain - world_pos.x),
            min(world_pos.y + half_domain, half_domain - world_pos.y)),
        min(world_pos.z + half_domain, half_domain - world_pos.z)
    );
    if (edge_dist < 60.0) {
        v_next = v_next * (edge_dist / 60.0);
        rho_adv = rho_adv * (edge_dist / 60.0);
    }

    // 5. Radiative Cooling towards Isothermal Background
    let cooling_rate = clamp(uniforms.dt * 0.02, 0.0, 1.0);
    let temp_next = mix(temp_curr, uniforms.ambient_temp, cooling_rate);

    // 6. Jeans Collapse Peak Detection (Restricted to Molecular Cloud Core)
    // Stars only form in the dense central core (r < 200 AU), never outside the cloud or at boundary edges
    if (r_len < 200.0 && rho_adv > uniforms.collapse_threshold) {
        // Verify local maximum to avoid duplicate spawns in adjacent cells
        if (rho_adv >= rho_xp && rho_adv >= rho_xm && rho_adv >= rho_yp && rho_adv >= rho_ym && rho_adv >= rho_zp && rho_adv >= rho_zm) {
            let slot = atomicAdd(&collapse_counter, 1u);
            if (slot < 32u) {
                let cell_volume = dx * dx * dx;
                let clump_mass = rho_adv * cell_volume;
                let clamped_vel = clamp(v_next, vec3<f32>(-3.0), vec3<f32>(3.0));

                collapse_events[slot] = GpuJeansCollapseEvent(
                    gid,
                    0u,
                    world_pos,
                    clump_mass,
                    clamped_vel,
                    temp_next
                );
                // Vacuum gas mass from grid down to floor upon protostar genesis
                rho_adv = 1.0e-14;
            }
        }
    }

    // 7. Write Out Buffers
    density_out[idx] = max(rho_adv, 0.0);
    velocity_out[idx] = vec4<f32>(v_next, 0.0);
    temperature[idx] = temp_next;
}
