// WGSL Multi-Pass GPU Disk Overdensity Nucleation & Streaming-Instability Coagulation Shader
// Runs directly on the VRAM particle storage buffer alongside `particle_orbit.wgsl`:
//   1. `clear_cells`: resets the 64x64 log-radial x azimuthal grid & compact 272-byte event output
//   2. `deposit_cells`: accumulates alive disk particle mass, COM position/velocity & composition
//   3. `evaluate_nucleation`: detects local 3x3 overdensities, snow-line & 0.35 AU silicate traps,
//      respects existing massive-body Hill spheres, and emits `GpuNucleationEvent`s or pebble coagulation
//   4. `apply_cells_to_particles`: consumes collapsed cells & donor grains atomically in VRAM
//   5. `finalize_clump_leaders`: credits coagulated donor mass onto surviving pebble leaders

struct Particle {
    pos_mass: vec4<f32>,    // xyz = position in AU, w = mass in M_sun
    vel_temp: vec4<f32>,    // xyz = velocity in AU/yr, w = temp in K
    composition: vec4<f32>, // silicate, ice, metal, gas
};

struct NucleationUniforms {
    star_pos: vec3<f32>,
    star_mass: f32,
    inner_radius: f32,
    outer_radius: f32,
    snow_line_au: f32,
    base_mass: f32,
    gas_scale: f32,
    elapsed_years: f32,
    v_ref_scale: f32,
    num_particles: u32,
    enable_promotion: u32,
    max_promotions: u32,
    min_peak_particles: u32,
    frame_seed: u32,
    min_clump_mass_solar: f32,
    coagulate_enabled: u32,
    num_massive_bodies: u32,
    _pad: u32,
    massive_bodies: array<vec4<f32>, 32>,
};

struct NucleationCell {
    particle_count: atomic<u32>,
    mass_fp: atomic<u32>,
    weight_sum: atomic<u32>,
    px_fp: atomic<i32>,
    py_fp: atomic<i32>,
    pz_fp: atomic<i32>,
    vx_fp: atomic<i32>,
    vy_fp: atomic<i32>,
    vz_fp: atomic<i32>,
    silicate_fp: atomic<u32>,
    ice_fp: atomic<u32>,
    metal_fp: atomic<u32>,
    gas_fp: atomic<u32>,
    temp_fp: atomic<u32>,
    leader_idx: atomic<u32>,
    promote_flag: atomic<u32>,
    coag_donors: atomic<u32>,
    coag_mass_fp: atomic<u32>,
    max_mass_fp: atomic<u32>,
    _pad: atomic<u32>,
};

struct NucleationEvent {
    pos_mass: vec4<f32>,
    vel_temp: vec4<f32>,
    composition: vec4<f32>,
    cell_meta: vec4<u32>,
};

struct NucleationOutput {
    event_count: atomic<u32>,
    magic: u32,
    generation_lo: u32,
    _pad: u32,
    events: array<NucleationEvent, 4>,
};

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<uniform> nu: NucleationUniforms;
@group(0) @binding(2) var<storage, read_write> cells: array<NucleationCell, 4096>;
@group(0) @binding(3) var<storage, read_write> promo_out: NucleationOutput;

const TWO_PI: f32 = 6.283185307179586;
const GRID_DIM: u32 = 64u;
const NUM_CELLS: u32 = 4096u;
const G_ASTRO: f32 = 39.478418;
const INVALID_LEADER: u32 = 0xFFFFFFFFu;
const NUCL_MAGIC: u32 = 0x4E55434Cu;

fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn cell_center_local_offset(cell_idx: u32, r_min: f32, r_max: f32) -> vec4<f32> {
    let r_bin = cell_idx / GRID_DIM;
    let phi_bin = cell_idx % GRID_DIM;
    let log_span = max(log(r_max / r_min), 1e-4);
    let r_cell = r_min * exp(((f32(r_bin) + 0.5) / f32(GRID_DIM)) * log_span);
    let phi_cell = ((f32(phi_bin) + 0.5) / f32(GRID_DIM)) * TWO_PI;
    let local_scale = max(r_cell * 0.25, 0.02);
    return vec4<f32>(r_cell * cos(phi_cell), 0.0, r_cell * sin(phi_cell), local_scale);
}

fn cell_index_for_pos(pos: vec3<f32>) -> i32 {
    if (pos.y < -1000.0) {
        return -1;
    }
    let dx = pos.x - nu.star_pos.x;
    let dz = pos.z - nu.star_pos.z;
    let r = sqrt(dx * dx + dz * dz);
    let r_min = max(nu.inner_radius, 0.06);
    let r_max = max(nu.outer_radius, r_min * 2.0);
    if (r < r_min || r > r_max * 1.05) {
        return -1;
    }

    let log_span = max(log(r_max / r_min), 1e-4);
    let r_norm = clamp(log(max(r, r_min) / r_min) / log_span, 0.0, 0.9999);
    let r_bin = min(u32(r_norm * f32(GRID_DIM)), GRID_DIM - 1u);

    let raw_phi = atan2(dz, dx);
    var phi = select(raw_phi, raw_phi + TWO_PI, raw_phi < 0.0);
    if (phi >= TWO_PI) {
        phi = phi - TWO_PI;
    }
    let phi_norm = clamp(phi / TWO_PI, 0.0, 0.9999);
    let phi_bin = min(u32(phi_norm * f32(GRID_DIM)), GRID_DIM - 1u);

    return i32(r_bin * GRID_DIM + phi_bin);
}

@compute @workgroup_size(64)
fn clear_cells(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cell_idx = gid.x;
    if (cell_idx >= NUM_CELLS) {
        return;
    }

    atomicStore(&cells[cell_idx].particle_count, 0u);
    atomicStore(&cells[cell_idx].mass_fp, 0u);
    atomicStore(&cells[cell_idx].weight_sum, 0u);
    atomicStore(&cells[cell_idx].px_fp, 0i);
    atomicStore(&cells[cell_idx].py_fp, 0i);
    atomicStore(&cells[cell_idx].pz_fp, 0i);
    atomicStore(&cells[cell_idx].vx_fp, 0i);
    atomicStore(&cells[cell_idx].vy_fp, 0i);
    atomicStore(&cells[cell_idx].vz_fp, 0i);
    atomicStore(&cells[cell_idx].silicate_fp, 0u);
    atomicStore(&cells[cell_idx].ice_fp, 0u);
    atomicStore(&cells[cell_idx].metal_fp, 0u);
    atomicStore(&cells[cell_idx].gas_fp, 0u);
    atomicStore(&cells[cell_idx].temp_fp, 0u);
    atomicStore(&cells[cell_idx].leader_idx, INVALID_LEADER);
    atomicStore(&cells[cell_idx].promote_flag, 0u);
    atomicStore(&cells[cell_idx].coag_donors, 0u);
    atomicStore(&cells[cell_idx].coag_mass_fp, 0u);
    atomicStore(&cells[cell_idx].max_mass_fp, 0u);
    atomicStore(&cells[cell_idx]._pad, 0u);

    if (cell_idx < 4u) {
        promo_out.events[cell_idx].pos_mass = vec4<f32>(0.0);
        promo_out.events[cell_idx].vel_temp = vec4<f32>(0.0);
        promo_out.events[cell_idx].composition = vec4<f32>(0.0);
        promo_out.events[cell_idx].cell_meta = vec4<u32>(0u);
    }

    if (cell_idx == 0u) {
        atomicStore(&promo_out.event_count, 0u);
        promo_out.magic = NUCL_MAGIC;
        promo_out.generation_lo = 0u;
        promo_out._pad = 0u;
    }
}

@compute @workgroup_size(64)
fn deposit_cells(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= nu.num_particles || idx >= arrayLength(&particles)) {
        return;
    }

    let p = particles[idx];
    let mass = p.pos_mass.w;
    if (mass <= 0.0) {
        return;
    }

    let c_i32 = cell_index_for_pos(p.pos_mass.xyz);
    if (c_i32 < 0) {
        return;
    }
    let cell_idx = u32(c_i32);

    let safe_base = max(nu.base_mass, 1e-12);
    let m_rel = clamp(mass / safe_base, 0.01, 256.0);
    let mass_fp_u = clamp(u32(round(m_rel * 256.0)), 1u, 65536u);
    let w_q = clamp(u32(round(m_rel * 4.0)), 1u, 128u);

    let r_min = max(nu.inner_radius, 0.06);
    let r_max = max(nu.outer_radius, r_min * 2.0);
    let c_info = cell_center_local_offset(cell_idx, r_min, r_max);
    let p_rel = clamp((p.pos_mass.xyz - nu.star_pos - c_info.xyz) / c_info.w, vec3<f32>(-2.0), vec3<f32>(2.0));
    let v_scale = max(nu.v_ref_scale, 1.0);
    let v_rel = clamp(p.vel_temp.xyz / v_scale, vec3<f32>(-2.0), vec3<f32>(2.0));

    let wi = i32(w_q);
    atomicAdd(&cells[cell_idx].particle_count, 1u);
    atomicAdd(&cells[cell_idx].mass_fp, mass_fp_u);
    atomicAdd(&cells[cell_idx].weight_sum, w_q);
    atomicAdd(&cells[cell_idx].px_fp, i32(round(p_rel.x * 512.0)) * wi);
    atomicAdd(&cells[cell_idx].py_fp, i32(round(p_rel.y * 512.0)) * wi);
    atomicAdd(&cells[cell_idx].pz_fp, i32(round(p_rel.z * 512.0)) * wi);
    atomicAdd(&cells[cell_idx].vx_fp, i32(round(v_rel.x * 128.0)) * wi);
    atomicAdd(&cells[cell_idx].vy_fp, i32(round(v_rel.y * 128.0)) * wi);
    atomicAdd(&cells[cell_idx].vz_fp, i32(round(v_rel.z * 128.0)) * wi);

    atomicAdd(&cells[cell_idx].silicate_fp, u32(round(clamp(p.composition.x, 0.0, 1.0) * 1024.0)));
    atomicAdd(&cells[cell_idx].ice_fp, u32(round(clamp(p.composition.y, 0.0, 1.0) * 1024.0)));
    atomicAdd(&cells[cell_idx].metal_fp, u32(round(clamp(p.composition.z, 0.0, 1.0) * 1024.0)));
    atomicAdd(&cells[cell_idx].gas_fp, u32(round(clamp(p.composition.w, 0.0, 1.0) * 1024.0)));
    atomicAdd(&cells[cell_idx].temp_fp, u32(round(clamp(p.vel_temp.w, 1.0, 20000.0))));

    atomicMin(&cells[cell_idx].leader_idx, idx);
    atomicMax(&cells[cell_idx].max_mass_fp, mass_fp_u);
}

@compute @workgroup_size(64)
fn evaluate_nucleation(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cell_idx = gid.x;
    if (cell_idx >= NUM_CELLS || nu.gas_scale <= 0.01) {
        return;
    }

    let my_count = atomicLoad(&cells[cell_idx].particle_count);
    let my_mass = atomicLoad(&cells[cell_idx].mass_fp);
    if (my_count < 2u || my_mass == 0u) {
        return;
    }

    let r_bin = cell_idx / GRID_DIM;
    let phi_bin = cell_idx % GRID_DIM;
    let r_min = max(nu.inner_radius, 0.06);
    let r_max = max(nu.outer_radius, r_min * 2.0);
    let log_span = max(log(r_max / r_min), 1e-4);
    let r_cell = r_min * exp(((f32(r_bin) + 0.5) / f32(GRID_DIM)) * log_span);

    let is_massive = nu.star_mass > 10.0;
    let snow_line = select(2.7, nu.snow_line_au, nu.snow_line_au > 0.1);
    let inner_trap_r = max(nu.inner_radius * 2.0, 0.35);

    let in_inner_trap = !is_massive && abs(r_cell - inner_trap_r) <= 0.25;
    let in_snow_trap = !is_massive && abs(r_cell - snow_line) <= 0.55;
    let in_belt = !is_massive && ((r_cell >= 2.0 && r_cell <= 4.5) || r_cell >= 15.0);
    let in_trap = in_inner_trap || in_snow_trap;

    var zone_kind: u32 = 0u;
    if (in_inner_trap) {
        zone_kind = 1u;
    } else if (in_snow_trap) {
        zone_kind = 2u;
    } else if (in_belt) {
        zone_kind = 3u;
    }

    // Check 3x3 neighborhood around (r_bin, phi_bin) for local overdensity peak & contrast
    var nb_sum: u32 = 0u;
    var nb_count: u32 = 0u;
    var is_strict_peak = true;

    for (var dr: i32 = -1i; dr <= 1i; dr = dr + 1i) {
        let nr = i32(r_bin) + dr;
        if (nr < 0i || nr >= i32(GRID_DIM)) {
            continue;
        }
        for (var dp: i32 = -1i; dp <= 1i; dp = dp + 1i) {
            if (dr == 0i && dp == 0i) {
                continue;
            }
            let np = u32((i32(phi_bin) + dp + i32(GRID_DIM)) % i32(GRID_DIM));
            let nb_idx = u32(nr) * GRID_DIM + np;
            let nb_m = atomicLoad(&cells[nb_idx].mass_fp);
            nb_sum = nb_sum + nb_m;
            nb_count = nb_count + 1u;
            if (nb_m > my_mass || (nb_m == my_mass && nb_idx < cell_idx)) {
                is_strict_peak = false;
            }
        }
    }

    let bg_mean = f32(nb_sum) / max(f32(nb_count), 1.0);
    let contrast = f32(my_mass) / max(bg_mean, 1.0);
    let max_grain_fp = atomicLoad(&cells[cell_idx].max_mass_fp);
    let has_evolved_pebble = max_grain_fp >= (8u * 256u);

    let safe_base = max(nu.base_mass, 1e-12);
    let cell_mass_solar = (f32(my_mass) / 256.0) * safe_base;

    let min_promo_r = select(0.15, 65.0, is_massive);
    let max_promo_r = select(min(nu.outer_radius * 0.95, 45.0), nu.outer_radius * 0.95, is_massive);
    let zone_min_particles = select(
        nu.min_peak_particles,
        max(nu.min_peak_particles / 2u, 2u),
        in_trap || r_cell >= 10.0,
    );
    let req_contrast = select(select(1.22, 1.05, in_trap), 1.0, has_evolved_pebble);

    let cell_hash = pcg_hash(cell_idx ^ (nu.frame_seed * 747796405u));

    // Check promotion into full ECS planetesimal / protoplanet if budget & thresholds are satisfied
    if (nu.enable_promotion != 0u
        && nu.max_promotions > 0u
        && is_strict_peak
        && contrast >= req_contrast
        && my_count >= zone_min_particles
        && cell_mass_solar >= max(nu.min_clump_mass_solar, 2.0 * safe_base)
        && r_cell >= min_promo_r
        && r_cell <= max_promo_r)
    {
        // Priority bias guarantees snow-line & 0.35 AU traps compete fairly across all radii
        let pass_thresh = select(28u, 260u, in_trap);
        if ((cell_hash & 1023u) < pass_thresh) {
            let w_sum = max(f32(atomicLoad(&cells[cell_idx].weight_sum)), 1.0);
            let inv_w = 1.0 / w_sum;
            let c_info = cell_center_local_offset(cell_idx, r_min, r_max);
            let v_scale = max(nu.v_ref_scale, 1.0);

            let pos_offset = vec3<f32>(
                f32(atomicLoad(&cells[cell_idx].px_fp)),
                f32(atomicLoad(&cells[cell_idx].py_fp)),
                f32(atomicLoad(&cells[cell_idx].pz_fp)),
            ) * (c_info.w * inv_w / 512.0);
            let ev_pos = nu.star_pos + c_info.xyz + pos_offset;

            let xz = ev_pos.xz - nu.star_pos.xz;
            let ev_r = length(xz);
            if (ev_r >= min_promo_r && ev_r <= max_promo_r) {
                // Skip cells already inside an existing planet's Hill clearing zone
                var inside_hill = false;
                let n_bodies = min(nu.num_massive_bodies, 32u);
                for (var j: u32 = 0u; j < n_bodies; j = j + 1u) {
                    let mb = nu.massive_bodies[j];
                    if (mb.w <= 0.0) {
                        continue;
                    }
                    let mb_r = max(length(mb.xyz - nu.star_pos), 0.01);
                    let hill_r = mb_r * pow(mb.w / (3.0 * max(nu.star_mass, 0.01)), 0.3333333);
                    let excl_r = select(max(hill_r * 2.2, 0.14), max(hill_r * 1.5, 4.0), is_massive);
                    let d = length(ev_pos - mb.xyz);
                    if (d < excl_r) {
                        inside_hill = true;
                        break;
                    }
                }

                if (!inside_hill) {
                    let raw_vel = vec3<f32>(
                        f32(atomicLoad(&cells[cell_idx].vx_fp)),
                        f32(atomicLoad(&cells[cell_idx].vy_fp)),
                        f32(atomicLoad(&cells[cell_idx].vz_fp)),
                    ) * (v_scale * inv_w / 128.0);

                    let v_k = sqrt(G_ASTRO * max(nu.star_mass, 0.01) / max(ev_r, 0.01));
                    let phi = atan2(xz.y, xz.x);
                    let v_circ = vec3<f32>(-v_k * sin(phi), 0.0, v_k * cos(phi));
                    let raw_speed = length(raw_vel);
                    let ev_vel = select(
                        v_circ,
                        raw_vel,
                        raw_speed > v_k * 0.55 && raw_speed < v_k * 1.45,
                    );

                    let inv_c = 1.0 / max(f32(my_count), 1.0);
                    let sil = clamp(f32(atomicLoad(&cells[cell_idx].silicate_fp)) * inv_c / 1024.0, 0.0, 1.0);
                    let ice = clamp(f32(atomicLoad(&cells[cell_idx].ice_fp)) * inv_c / 1024.0, 0.0, 1.0);
                    let met = clamp(f32(atomicLoad(&cells[cell_idx].metal_fp)) * inv_c / 1024.0, 0.0, 1.0);
                    let gas = clamp(f32(atomicLoad(&cells[cell_idx].gas_fp)) * inv_c / 1024.0, 0.0, 1.0);
                    let temp_k = clamp(f32(atomicLoad(&cells[cell_idx].temp_fp)) * inv_c, 3.0, 20000.0);

                    let slot = atomicAdd(&promo_out.event_count, 1u);
                    if (slot < min(nu.max_promotions, 4u)) {
                        promo_out.events[slot].pos_mass = vec4<f32>(ev_pos, cell_mass_solar);
                        promo_out.events[slot].vel_temp = vec4<f32>(ev_vel, temp_k);
                        promo_out.events[slot].composition = vec4<f32>(sil, ice, met, gas);
                        promo_out.events[slot].cell_meta = vec4<u32>(cell_idx, my_count, zone_kind, 0u);
                        atomicStore(&cells[cell_idx].promote_flag, 1u);
                        return;
                    }
                }
            }
        }
    }

    // Otherwise run lightweight local pebble coagulation in trap/overdense filaments
    if (nu.coagulate_enabled != 0u
        && !is_massive
        && my_count >= 4u
        && max_grain_fp < (14u * 256u)
        && (in_trap || contrast >= 1.08))
    {
        let coag_thresh = select(24u, 110u, in_trap);
        if (((cell_hash >> 10u) & 1023u) < coag_thresh) {
            atomicStore(&cells[cell_idx].promote_flag, 2u);
        }
    }
}

@compute @workgroup_size(64)
fn apply_cells_to_particles(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= nu.num_particles || idx >= arrayLength(&particles)) {
        return;
    }

    var p = particles[idx];
    let mass = p.pos_mass.w;
    if (mass <= 0.0) {
        return;
    }

    let c_i32 = cell_index_for_pos(p.pos_mass.xyz);
    if (c_i32 < 0) {
        return;
    }
    let cell_idx = u32(c_i32);
    let flag = atomicLoad(&cells[cell_idx].promote_flag);

    if (flag == 1u) {
        // Consumed into a nucleated ECS planetesimal / protoplanet
        p.pos_mass = vec4<f32>(0.0, -5000.0, 0.0, 0.0);
        p.vel_temp = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        particles[idx] = p;
        return;
    }

    if (flag == 2u) {
        let leader = atomicLoad(&cells[cell_idx].leader_idx);
        let safe_base = max(nu.base_mass, 1e-12);
        if (idx != leader && mass < 3.5 * safe_base) {
            let p_hash = pcg_hash(idx ^ (nu.frame_seed * 1664525u));
            if ((p_hash & 7u) == 0u) {
                let donor_slot = atomicAdd(&cells[cell_idx].coag_donors, 1u);
                if (donor_slot < 3u) {
                    let mass_fp_u = clamp(u32(round((mass / safe_base) * 256.0)), 1u, 1024u);
                    atomicAdd(&cells[cell_idx].coag_mass_fp, mass_fp_u);
                    p.pos_mass = vec4<f32>(0.0, -5000.0, 0.0, 0.0);
                    p.vel_temp = vec4<f32>(0.0, 0.0, 0.0, 0.0);
                    particles[idx] = p;
                }
            }
        }
    }
}

@compute @workgroup_size(64)
fn finalize_clump_leaders(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cell_idx = gid.x;
    if (cell_idx >= NUM_CELLS) {
        return;
    }
    if (atomicLoad(&cells[cell_idx].promote_flag) != 2u) {
        return;
    }

    let leader = atomicLoad(&cells[cell_idx].leader_idx);
    let donated_fp = atomicLoad(&cells[cell_idx].coag_mass_fp);
    if (leader >= nu.num_particles || leader >= arrayLength(&particles) || donated_fp == 0u) {
        return;
    }

    var lp = particles[leader];
    if (lp.pos_mass.w > 0.0) {
        lp.pos_mass.w = lp.pos_mass.w + (f32(donated_fp) / 256.0) * max(nu.base_mass, 1e-12);
        lp.vel_temp.w = min(lp.vel_temp.w + 35.0, 1800.0);
        particles[leader] = lp;
    }
}
