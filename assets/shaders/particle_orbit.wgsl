// WGSL Compute Shader for 50,000 Particle Keplerian Orbital Mechanics, Planetary Resonances & Gas Drag

struct Particle {
    position: vec3<f32>,
    mass: f32,
    velocity: vec3<f32>,
    temperature: f32,
    composition: vec4<f32>, // x: silicate, y: ice, z: metal, w: gas
};

struct SimUniforms {
    star_pos: vec3<f32>,
    star_mass: f32,
    dt: f32,
    gas_scale: f32,
    inner_radius: f32,
    outer_radius: f32,
    g_const: f32,
    enable_gas_drag: u32,
    num_particles: u32,
    ref_temp_1au: f32,
    shockwave_radius: f32,
    softening_sq: f32,
    num_massive_bodies: u32,
    _pad: f32,
    tractor_pos_mass: vec4<f32>,
    massive_bodies: array<vec4<f32>, 32>, // xyz: pos, w: mass
};

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<uniform> uniforms: SimUniforms;

const PI: f32 = 3.141592653589793;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= uniforms.num_particles || idx >= arrayLength(&particles)) {
        return;
    }

    var p = particles[idx];
    if (p.mass <= 0.0) {
        return; // Absorbed or dead particle slot
    }

    let star = uniforms.star_pos;
    var pos = p.position;
    let dx = pos.x - star.x;
    let dz = pos.z - star.z;
    var r = max(sqrt(dx * dx + dz * dz), 0.08);
    var phi = atan2(dz, dx);

    // Effective stellar mass (radiation pressure reduces effective gravity by ~0.05%)
    let m_eff = uniforms.star_mass * (1.0 - 0.0005);
    let omega = sqrt(uniforms.g_const * m_eff / (r * r * r));
    let v_k = omega * r;

    // Smooth orbital phase advance
    phi = phi + omega * uniforms.dt;
    phi = phi - floor(phi / (2.0 * PI)) * (2.0 * PI);

    // Aerodynamic Gas Drag & Secular Inward Drift
    if (uniforms.enable_gas_drag != 0u && uniforms.gas_scale > 0.001) {
        let gas_density = 1.0e-4 * pow(r / 1.0, -2.25) * uniforms.gas_scale;
        let drag_rate = min(0.000005 * gas_density, 0.0005);
        let migration = min(r * drag_rate * uniforms.dt, r * 0.005);
        r = max(r - migration, uniforms.inner_radius * 0.8);
    }

    // Stellar Blast Shockwave & Photo-evaporative Dust Clearing
    if (uniforms.shockwave_radius > 0.0) {
        if (r < uniforms.shockwave_radius) {
            if (r < 1.6 || r < uniforms.shockwave_radius * 0.85) {
                p.mass = 0.0;
                p.position = vec3<f32>(0.0, -5000.0, 0.0);
                particles[idx] = p;
                return;
            } else {
                r = min(uniforms.shockwave_radius + 0.15, uniforms.outer_radius);
            }
        } else if (abs(r - uniforms.shockwave_radius) < 1.0) {
            let shock_boost = (1.0 - abs(r - uniforms.shockwave_radius)) / 1.0;
            r = r + shock_boost * 1.5 * uniforms.dt;
        }
    }

    // Bound particles within the disk
    r = clamp(r, uniforms.inner_radius * 0.75, uniforms.outer_radius * 1.05);

    // Baseline coordinates
    pos.x = star.x + r * cos(phi);
    pos.y = pos.y * exp(-0.005 * uniforms.dt);
    pos.z = star.z + r * sin(phi);

    // Baseline circular velocity
    var vel = vec3<f32>(-v_k * sin(phi), 0.0, v_k * cos(phi));

    // Phase 2: Planetary Perturbations & Hill-Sphere Resonant Clearing
    var a_pert = vec3<f32>(0.0, 0.0, 0.0);
    let n_bodies = min(uniforms.num_massive_bodies, 32u);
    for (var j: u32 = 0u; j < n_bodies; j = j + 1u) {
        let mb = uniforms.massive_bodies[j];
        let m_body_mass = mb.w;
        if (m_body_mass <= 0.0) {
            continue;
        }

        let to_body = mb.xyz - pos;
        let dist_sq = dot(to_body, to_body) + uniforms.softening_sq;
        let dist = sqrt(dist_sq);

        // Planetary Hill-Sphere resonance
        let inv_dist3 = 1.0 / (dist_sq * dist);
        let f_grav = uniforms.g_const * m_body_mass * inv_dist3;
        a_pert = a_pert + to_body * f_grav;
    }

    // Phase 2: Player Gravitational Tractor
    if (uniforms.tractor_pos_mass.w > 0.0) {
        let to_trac = uniforms.tractor_pos_mass.xyz - pos;
        let dist_sq = dot(to_trac, to_trac) + 0.005 * 0.005;
        let dist = sqrt(dist_sq);
        let f_trac = uniforms.g_const * uniforms.tractor_pos_mass.w / (dist_sq * dist);
        a_pert = a_pert + to_trac * f_trac;
    }

    // Apply acceleration perturbations smoothly
    vel = vel + a_pert * uniforms.dt;
    pos = pos + a_pert * (0.5 * uniforms.dt * uniforms.dt);

    p.velocity = vel;
    p.position = pos;

    // Blackbody temperature calculation
    p.temperature = uniforms.ref_temp_1au * pow(r / 1.0, -0.5);

    particles[idx] = p;
}
