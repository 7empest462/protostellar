use bevy::math::DVec3;
use bevy::prelude::*;
use rand::Rng;
use rayon::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::ParticleSwarmData;

pub struct ParticleIntegrationParams<'a> {
    pub star_pos_f32: [f32; 3],
    pub star_m: f32,
    pub g_const: f32,
    pub shockwave_r: f32,
    pub enable_gas_drag: bool,
    pub gas_scale: f32,
    pub star_is_ignited: bool,
    pub quasar_blown_out: bool,
    pub tractor_pos_mass: [f32; 4],
    pub speed_mult: f32,
    pub visual_flow_dt: f32,
    pub gpu_active: bool,
    pub lhb_active: bool,
    pub lhb_resonance: bool,
    pub disk_params: &'a DiskParameters,
    pub massive_bodies: &'a [(Entity, DVec3, f64, BodyType)],
}

fn step_particle_gpu_mode(
    pos: [f32; 3],
    m: f32,
    temp_slot: &mut f32,
    col_slot: &mut [f32; 4],
    mass_slot: &mut f32,
    pos_slot: &mut [f32; 3],
    params: &ParticleIntegrationParams,
    chunk_accretions: &mut Vec<(Entity, f64)>,
) {
    for &(p_ent, p_pos, p_m, p_type) in params.massive_bodies {
        let is_major_body = p_m >= 0.01 * EARTH_MASS_SOLAR
            && !matches!(p_type, BodyType::Asteroid | BodyType::Comet);

        let pdx = pos[0] - p_pos.x as f32;
        let pdz = pos[2] - p_pos.z as f32;
        let p_dist = (pdx * pdx + pdz * pdz).sqrt().max(0.001);

        let p_dist_au = p_pos.length() as f32;
        let is_inner_terrestrial = p_dist_au < 2.7;
        let warp_sweep = (1.0 + params.speed_mult.log10().max(0.0) * 0.30).min(2.2);
        let is_massive_disk = params.star_m > 10.0;
        let hill_r = p_dist_au * ((p_m / (3.0 * f64::from(params.star_m))).cbrt() as f32);
        let bondi_r = if is_massive_disk {
            (0.04 * (p_m / JUPITER_MASS_SOLAR).sqrt() as f32).clamp(0.02, 1.2)
        } else {
            0.0
        };
        let effective_grav_r = hill_r.max(bondi_r);
        let physical_r = if p_m >= 0.08 {
            (0.00465 * (p_m / 1.0).powf(0.8) as f32).clamp(0.004, 0.20)
        } else {
            (0.005 * (p_m / EARTH_MASS_SOLAR).cbrt() as f32).clamp(0.003, 0.040)
        };

        let acc_r = if is_major_body {
            if is_inner_terrestrial && !is_massive_disk {
                ((physical_r + 0.50 * hill_r) * warp_sweep).clamp(physical_r, 0.150)
            } else if is_massive_disk {
                ((physical_r + 0.35 * effective_grav_r) * warp_sweep).clamp(physical_r, 0.850)
            } else {
                ((physical_r + 0.60 * hill_r) * warp_sweep).clamp(physical_r, 0.350)
            }
        } else {
            ((physical_r * 1.5) * warp_sweep).clamp(0.00005, 0.0015)
        };

        if p_dist < acc_r {
            chunk_accretions.push((p_ent, f64::from(m)));
            *mass_slot = 0.0;
            *pos_slot = [0.0, -5000.0, 0.0];
            return;
        }
    }

    let temp = *temp_slot;
    let (br, bg, bb) = blackbody_to_srgb(f64::from(temp));
    col_slot[0] = (br * 0.4 + col_slot[0] * 0.6).clamp(0.25, 1.0);
    col_slot[1] = (bg * 0.4 + col_slot[1] * 0.6).clamp(0.2, 1.0);
    col_slot[2] = (bb * 0.4 + col_slot[2] * 0.6).clamp(0.2, 1.0);
}

fn step_particle_cpu_mode(
    mut pos: [f32; 3],
    mut vel: [f32; 3],
    m: f32,
    temp_slot: &mut f32,
    col_slot: &mut [f32; 4],
    mass_slot: &mut f32,
    pos_slot: &mut [f32; 3],
    vel_slot: &mut [f32; 3],
    params: &ParticleIntegrationParams,
    chunk_accretions: &mut Vec<(Entity, f64)>,
) {
    let dx = pos[0] - params.star_pos_f32[0];
    let dz = pos[2] - params.star_pos_f32[2];
    let min_r = (params.disk_params.inner_radius_au as f32 * 0.5).max(0.001);
    let mut r = (dx * dx + dz * dz).sqrt().max(min_r);
    let mut phi = dz.atan2(dx);
    let m_eff = params.star_m * (1.0 - 0.0005);
    let omega = (params.g_const * m_eff / (r * r * r)).sqrt();
    let v_k = omega * r;
    phi = (phi + omega * params.visual_flow_dt) % (2.0 * PI as f32);
    if phi < 0.0 {
        phi += 2.0 * PI as f32;
    }
    if params.enable_gas_drag && params.gas_scale > 0.001 {
        let gas_density = 1.0e-4 * (r / 1.0).powf(-2.25) * params.gas_scale;
        let drag_rate = (0.000_005 * gas_density).min(0.0005);
        let migration = (r * drag_rate * params.visual_flow_dt).min(r * 0.005);
        r = (r - migration).max(params.disk_params.inner_radius_au as f32 * 0.8);
    }
    if params.quasar_blown_out && r < 25.0 {
        *mass_slot = 0.0;
        *pos_slot = [0.0, -5000.0, 0.0];
        if let Some(alpha) = col_slot.get_mut(3) {
            *alpha = 0.0;
        }
        return;
    } else if params.shockwave_r > 0.0 {
        if r < params.shockwave_r {
            let push_rate = 1.80 * params.visual_flow_dt;
            r = (r + push_rate).min(params.disk_params.outer_radius_au as f32);
        } else if (r - params.shockwave_r).abs() < 3.0 {
            let shock_boost = (3.0 - (r - params.shockwave_r).abs()) / 3.0;
            r += shock_boost * 2.2 * params.visual_flow_dt;
        }
    }

    for &(p_ent, p_pos, p_m, p_type) in params.massive_bodies {
        let is_major_body = p_m >= 0.01 * EARTH_MASS_SOLAR
            && !matches!(p_type, BodyType::Asteroid | BodyType::Comet);

        let pdx = pos[0] - p_pos.x as f32;
        let pdz = pos[2] - p_pos.z as f32;
        let p_dist = (pdx * pdx + pdz * pdz).sqrt().max(0.001);

        let p_dist_au = p_pos.length() as f32;
        let is_inner_terrestrial = p_dist_au < 2.7;
        let warp_sweep = (1.0 + params.speed_mult.log10().max(0.0) * 0.30).min(2.2);
        let is_massive_disk = params.star_m > 10.0;
        let hill_r = p_dist_au * ((p_m / (3.0 * f64::from(params.star_m))).cbrt() as f32);
        let bondi_r = if is_massive_disk {
            (0.04 * (p_m / JUPITER_MASS_SOLAR).sqrt() as f32).clamp(0.02, 1.2)
        } else {
            0.0
        };
        let effective_grav_r = hill_r.max(bondi_r);
        let physical_r = if p_m >= 0.08 {
            (0.00465 * (p_m / 1.0).powf(0.8) as f32).clamp(0.004, 0.20)
        } else {
            (0.005 * (p_m / EARTH_MASS_SOLAR).cbrt() as f32).clamp(0.003, 0.040)
        };

        let acc_r = if is_major_body {
            if is_inner_terrestrial && !is_massive_disk {
                ((physical_r + 0.50 * hill_r) * warp_sweep).clamp(physical_r, 0.150)
            } else if is_massive_disk {
                ((physical_r + 0.35 * effective_grav_r) * warp_sweep).clamp(physical_r, 0.850)
            } else {
                ((physical_r + 0.60 * hill_r) * warp_sweep).clamp(physical_r, 0.350)
            }
        } else {
            ((physical_r * 1.5) * warp_sweep).clamp(0.00005, 0.0015)
        };

        if p_dist < acc_r {
            chunk_accretions.push((p_ent, f64::from(m)));
            *mass_slot = 0.0;
            *pos_slot = [0.0, -5000.0, 0.0];
            return;
        }

        if is_major_body {
            let hill_r =
                p_pos.length() as f32 * ((p_m / (3.0 * f64::from(params.star_m))).cbrt() as f32);
            if p_dist < hill_r * 2.5 {
                let kick = (params.g_const * p_m as f32 / (p_dist * p_dist + 0.01))
                    * params.visual_flow_dt.min(0.02);
                r += (pdx * kick * 0.04).clamp(-0.08, 0.08);
            }
        }
    }

    if params.tractor_pos_mass[3] > 0.0 {
        let tdx = params.tractor_pos_mass[0] - pos[0];
        let tdz = params.tractor_pos_mass[2] - pos[2];
        let t_dist = (tdx * tdx + tdz * tdz).sqrt().max(0.1);
        let pull = (params.g_const * params.tractor_pos_mass[3] / (t_dist * t_dist + 0.1))
            * params.visual_flow_dt.min(0.05);
        r += (tdx * pull * 0.05).clamp(-0.2, 0.2);
    }
    if params.lhb_active && params.lhb_resonance && ((2.0..=3.8).contains(&r) || r >= 12.0) {
        r -= (params.visual_flow_dt * 0.05).clamp(0.0, 0.02);
    }
    if params.star_is_ignited && r < 2.15 {
        r += (params.visual_flow_dt * 0.08).clamp(0.0, 0.035);
    }
    r = r.clamp(
        params.disk_params.inner_radius_au as f32 * 0.80,
        params.disk_params.outer_radius_au as f32 * 1.05,
    );
    pos[0] = params.star_pos_f32[0] + r * phi.cos();
    let h_disk = (0.030f32 * r * (r / 1.0f32).powf(0.25f32)).max(0.0002f32);
    pos[1] = (pos[1] * (-0.02 * params.visual_flow_dt).exp()).clamp(-h_disk * 2.5, h_disk * 2.5);
    pos[2] = params.star_pos_f32[2] + r * phi.sin();
    vel[0] = -v_k * phi.sin();
    vel[1] = 0.0;
    vel[2] = v_k * phi.cos();
    let temp = (params.disk_params.reference_temp_1au as f32) * (r / 1.0).powf(-0.5);
    *temp_slot = temp;
    let (br, bg, bb) = blackbody_to_srgb(f64::from(temp));
    col_slot[0] = (br * 0.4 + col_slot[0] * 0.6).clamp(0.25, 1.0);
    col_slot[1] = (bg * 0.4 + col_slot[1] * 0.6).clamp(0.2, 1.0);
    col_slot[2] = (bb * 0.4 + col_slot[2] * 0.6).clamp(0.2, 1.0);
    *pos_slot = pos;
    *vel_slot = vel;
}

pub fn integrate_particles_and_collect_accretions(
    data: &mut ParticleSwarmData,
    params: &ParticleIntegrationParams,
) -> Vec<(Entity, f64)> {
    data.positions
        .par_chunks_mut(4096)
        .zip(data.velocities.par_chunks_mut(4096))
        .zip(data.masses.par_chunks_mut(4096))
        .zip(data.temperatures.par_chunks_mut(4096))
        .zip(data.colors.par_chunks_mut(4096))
        .map(
            |((((pos_chunk, vel_chunk), mass_chunk), temp_chunk), col_chunk)| {
                let mut chunk_accretions = Vec::new();
                for ((((pos_slot, vel_slot), mass_slot), temp_slot), col_slot) in pos_chunk
                    .iter_mut()
                    .zip(vel_chunk.iter_mut())
                    .zip(mass_chunk.iter_mut())
                    .zip(temp_chunk.iter_mut())
                    .zip(col_chunk.iter_mut())
                {
                    let m = *mass_slot;
                    if m <= 0.0 {
                        continue;
                    }
                    let pos = *pos_slot;
                    let vel = *vel_slot;

                    if params.gpu_active {
                        step_particle_gpu_mode(
                            pos,
                            m,
                            temp_slot,
                            col_slot,
                            mass_slot,
                            pos_slot,
                            params,
                            &mut chunk_accretions,
                        );
                    } else {
                        step_particle_cpu_mode(
                            pos,
                            vel,
                            m,
                            temp_slot,
                            col_slot,
                            mass_slot,
                            pos_slot,
                            vel_slot,
                            params,
                            &mut chunk_accretions,
                        );
                    }
                }
                chunk_accretions
            },
        )
        .flatten()
        .collect()
}

fn update_growing_body_physics(
    mass: &mut Mass,
    radius: &mut Radius,
    comp: &mut Composition,
    body: &mut CelestialBody,
    gain: f64,
    r_au: f64,
    star_mass: f64,
    disk_params: &DiskParameters,
) {
    if matches!(body.body_type, BodyType::Asteroid | BodyType::Comet) {
        mass.0 = (mass.0 + gain).min(0.0005 * EARTH_MASS_SOLAR);
        let avg_density = comp.average_density();
        radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI)).cbrt();
        return;
    }

    let is_beyond_snowline = r_au >= 2.7;
    let is_massive_disk = star_mass > 10.0;

    if !is_beyond_snowline && !is_massive_disk {
        if mass.0 < 1.05 * EARTH_MASS_SOLAR {
            let m_earth_ratio = (mass.0 / EARTH_MASS_SOLAR).clamp(0.1, 1.0);
            let runaway_mult = 1.0 + 0.35 * m_earth_ratio;
            mass.0 = (mass.0 + gain * runaway_mult).min(1.02 * EARTH_MASS_SOLAR);
        }
    } else if !is_massive_disk {
        let max_giant_mass = 2.5 * JUPITER_MASS_SOLAR;
        if mass.0 < max_giant_mass {
            let m_earth = mass.0 / EARTH_MASS_SOLAR;
            let runaway_mult = if m_earth < 10.0 {
                1.0 + 0.05 * m_earth
            } else {
                1.5 + 0.15 * m_earth.clamp(10.0, 350.0).powf(0.30)
            };
            mass.0 = (mass.0 + gain * runaway_mult).min(max_giant_mass);
        }
    } else {
        let max_ring_mass = crate::simulation::accretion::circum_nuclear_ring_mass_capacity(
            r_au,
            disk_params.inner_radius_au,
            disk_params.outer_radius_au,
        );
        if mass.0 < max_ring_mass {
            let growth_factor = (1.0 - (mass.0 / max_ring_mass)).clamp(0.0, 1.0);
            let mult = if mass.0 < 0.08 {
                let m_earth = (mass.0 / EARTH_MASS_SOLAR).clamp(0.1, 3000.0);
                1.0 + 0.10 * m_earth.powf(0.18)
            } else {
                1.0 + 0.25 * growth_factor
            };
            mass.0 = (mass.0 + gain * mult).min(max_ring_mass);
        }
    }

    let updated_type = if body.body_type == BodyType::BlackHole {
        BodyType::BlackHole
    } else {
        crate::simulation::components::classify_body_by_mass_and_comp(mass.0, comp, false)
    };
    body.body_type = updated_type;

    let new_radius = if updated_type == BodyType::BlackHole {
        (1.97e-8 * mass.0).max(1e-6)
    } else if mass.0 >= 0.08 {
        (0.00465 * (mass.0 / 1.0).powf(0.8)).clamp(0.003, 10.0)
    } else {
        let avg_density = comp.average_density();
        ((3.0 * mass.0 / avg_density) / (4.0 * PI))
            .cbrt()
            .max(EARTH_RADIUS_AU * 0.2)
    };
    radius.0 = new_radius;

    if is_beyond_snowline && mass.0 >= 6.0 * EARTH_MASS_SOLAR && comp.gas_frac < 0.40 {
        comp.gas_frac = (comp.gas_frac + 0.08).min(0.92);
        comp.ice_frac = (comp.ice_frac * 0.90).max(0.04);
    }
    if mass.0 >= 13.0 * JUPITER_MASS_SOLAR {
        comp.gas_frac = (comp.gas_frac + 0.15).min(0.99);
    }
    if !is_beyond_snowline && !is_massive_disk && (mass.0 >= 0.02 * EARTH_MASS_SOLAR) {
        comp.gas_frac = comp.gas_frac.min(0.035);
    }
}

fn update_growing_body_name(body: &mut CelestialBody, mass: f64, r_au: f64) {
    let is_canonical_solar = body.name == "Earth"
        || body.name == "Venus"
        || body.name == "Mars"
        || body.name == "Mercury"
        || body.name.starts_with("Proto-")
        || body.name.starts_with("Theia")
        || body.name == "Jupiter"
        || body.name == "Saturn"
        || body.name == "Uranus"
        || body.name == "Neptune";

    if is_canonical_solar {
        return;
    }

    body.name = match body.body_type {
        BodyType::BlackHole => {
            if mass >= 100.0 {
                format!("Intermediate Black Hole ({mass:.1} M☉)")
            } else {
                format!("Orbiting Stellar Black Hole ({mass:.1} M☉)")
            }
        }
        BodyType::Hypergiant => format!("Pop-III Hypergiant ({mass:.1} M☉)"),
        BodyType::BlueSupergiant => format!("Pop-III Blue Supergiant ({mass:.1} M☉)"),
        BodyType::BlueGiant => format!("Pop-III Blue Giant ({mass:.1} M☉)"),
        BodyType::YellowDwarf => format!("Pop-III Yellow Star ({mass:.2} M☉)"),
        BodyType::RedDwarf => format!("Red Dwarf ({mass:.2} M☉)"),
        BodyType::BrownDwarf => format!("Brown Dwarf ({:.1} M_J)", mass / JUPITER_MASS_SOLAR),
        BodyType::GasGiant => {
            if mass >= JUPITER_MASS_SOLAR {
                format!("Super-Jupiter ({:.1} M_J)", mass / JUPITER_MASS_SOLAR)
            } else {
                format!("Planet-{r_au:.0}AU (Gas Giant)")
            }
        }
        BodyType::IceGiant => format!("Planet-{r_au:.0}AU (Ice Giant)"),
        BodyType::SuperEarth => format!("Planet-{r_au:.0}AU (Super-Earth)"),
        BodyType::TerrestrialPlanet => format!("Planet-{r_au:.0}AU (Terrestrial)"),
        BodyType::Protoplanet => format!("Protoplanet-{r_au:.0}AU"),
        BodyType::Planetesimal => format!("Planetesimal-{r_au:.0}AU"),
        BodyType::Comet => format!("Comet-{:.0}AU", r_au * 10.0),
        _ => body.name.clone(),
    };
}

pub fn apply_particle_accretion_to_bodies(
    massive_query: &mut Query<
        (
            Entity,
            &SimPosition,
            &mut Mass,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
        ),
        Without<CentralStar>,
    >,
    massive_bodies: &[(Entity, DVec3, f64, BodyType)],
    accreted_events: Vec<(Entity, f64)>,
    pending_gpu_accretions: &mut Vec<(usize, f32)>,
    star_mass: f64,
    disk_params: &DiskParameters,
) {
    let mut mass_gains: hashbrown::HashMap<Entity, f64> = hashbrown::HashMap::new();
    for (ent, delta_m) in accreted_events {
        *mass_gains.entry(ent).or_insert(0.0) += delta_m;
    }
    for (body_idx, delta_m) in pending_gpu_accretions.drain(..) {
        if let Some(&(ent, ..)) = massive_bodies.get(body_idx) {
            *mass_gains.entry(ent).or_insert(0.0) += f64::from(delta_m);
        }
    }

    for (ent, pos, mut mass, mut radius, mut comp, mut body) in massive_query.iter_mut() {
        if let Some(&gain) = mass_gains.get(&ent) {
            let r_au = pos.0.length();
            update_growing_body_physics(
                &mut mass,
                &mut radius,
                &mut comp,
                &mut body,
                gain,
                r_au,
                star_mass,
                disk_params,
            );
            update_growing_body_name(&mut body, mass.0, r_au);
        }
    }
}

pub fn build_spatial_hash_bins(data: &mut ParticleSwarmData, disk_params: &DiskParameters) {
    let n = data.count;
    let r_min = disk_params.inner_radius_au as f32;
    let r_max = disk_params.outer_radius_au as f32;
    let r_span = (r_max - r_min).max(1.0);
    data.bin_heads.fill(-1);

    for i in 0..n {
        let Some(&m) = data.masses.get(i) else {
            continue;
        };
        if m <= 0.0 {
            continue;
        }
        let Some(&pos) = data.positions.get(i) else {
            continue;
        };
        let r = (pos[0] * pos[0] + pos[2] * pos[2]).sqrt();
        let phi = pos[2].atan2(pos[0]).rem_euclid(2.0 * PI as f32);
        let r_norm = ((r - r_min) / r_span).clamp(0.0, 0.999);
        let phi_norm = (phi / (2.0 * PI as f32)).clamp(0.0, 0.999);
        let bin_idx = ((r_norm * 64.0) as usize) * 64 + ((phi_norm * 64.0) as usize);
        if let Some(&head) = data.bin_heads.get(bin_idx) {
            if let Some(next) = data.bin_next.get_mut(i) {
                *next = head;
            }
        }
        #[allow(clippy::cast_possible_wrap)]
        let i_i32 = i as i32;
        if let Some(head) = data.bin_heads.get_mut(bin_idx) {
            *head = i_i32;
        }
    }
}

pub fn process_particle_collisions_and_sticking(
    data: &mut ParticleSwarmData,
    disk_params: &DiskParameters,
    speed_mult: f32,
) {
    let b_mass = data.base_mass;
    for r_b in 0..64usize {
        for p_b in 0..64usize {
            let bin_idx = r_b * 64 + p_b;
            let mut curr = data.bin_heads.get(bin_idx).copied().unwrap_or(-1);

            while curr >= 0 {
                let idx_a = curr as usize;
                let mut neighbor = data.bin_next.get(idx_a).copied().unwrap_or(-1);
                let mut depth = 0;

                while neighbor >= 0 && depth < 8 {
                    let idx_b = neighbor as usize;

                    let (m_a_opt, m_b_opt) = (
                        data.masses.get(idx_a).copied(),
                        data.masses.get(idx_b).copied(),
                    );
                    if let (Some(m_a), Some(m_b)) = (m_a_opt, m_b_opt) {
                        if m_a > 0.0 && m_b > 0.0 {
                            let (pos_a_opt, pos_b_opt) = (
                                data.positions.get(idx_a).copied(),
                                data.positions.get(idx_b).copied(),
                            );
                            if let (Some(pos_a), Some(pos_b)) = (pos_a_opt, pos_b_opt) {
                                let dx = pos_a[0] - pos_b[0];
                                let dy = pos_a[1] - pos_b[1];
                                let dz = pos_a[2] - pos_b[2];
                                let dist_sq = dx * dx + dy * dy + dz * dz;

                                let r_body = (pos_a[0] * pos_a[0] + pos_a[2] * pos_a[2]).sqrt();
                                let is_beyond_snowline = r_body > disk_params.snow_line_au as f32;

                                let comp_a_val = data.compositions.get(idx_a).copied();
                                let sticky_boost = if is_beyond_snowline {
                                    2.5 * (1.0
                                        + comp_a_val.map_or(0.0, |c| c.ice_frac as f32) * 1.5)
                                } else {
                                    1.0
                                };
                                let zone_boost = (r_body / 1.0).powf(0.55).clamp(1.0, 4.5);
                                let mass_factor = (m_a / b_mass).cbrt().clamp(1.0, 6.0);
                                let warp_stick_boost =
                                    (1.0 + speed_mult.log10().max(0.0) * 0.22).min(1.8);
                                let r_acc = (0.012
                                    * sticky_boost
                                    * zone_boost
                                    * mass_factor
                                    * warp_stick_boost)
                                    .clamp(0.005, 0.080);

                                if dist_sq < r_acc * r_acc {
                                    try_merge_particles(
                                        data,
                                        idx_a,
                                        idx_b,
                                        m_a,
                                        m_b,
                                        b_mass,
                                        is_beyond_snowline,
                                    );
                                }
                            }
                        }
                    }
                    neighbor = data.bin_next.get(neighbor as usize).copied().unwrap_or(-1);
                    depth += 1;
                }
                curr = data.bin_next.get(curr as usize).copied().unwrap_or(-1);
            }
        }
    }
}

fn try_merge_particles(
    data: &mut ParticleSwarmData,
    idx_a: usize,
    idx_b: usize,
    m_a: f32,
    m_b: f32,
    b_mass: f32,
    is_beyond_snowline: bool,
) {
    let (vel_a_opt, vel_b_opt) = (
        data.velocities.get(idx_a).copied(),
        data.velocities.get(idx_b).copied(),
    );
    let (Some(vel_a), Some(vel_b)) = (vel_a_opt, vel_b_opt) else {
        return;
    };

    let dvx = vel_a[0] - vel_b[0];
    let dvy = vel_a[1] - vel_b[1];
    let dvz = vel_a[2] - vel_b[2];
    let v_rel_sq = dvx * dvx + dvy * dvy + dvz * dvz;

    let v_crit = if is_beyond_snowline { 0.85 } else { 0.50 };
    if v_rel_sq >= v_crit * v_crit {
        return;
    }

    let mut rng = rand::rng();
    let stick_prob = if is_beyond_snowline { 0.90 } else { 0.75 };
    if rng.random_range(0.0..1.0f32) >= stick_prob {
        return;
    }

    let merged_vel = [
        (vel_a[0] * m_a + vel_b[0] * m_b) / (m_a + m_b),
        (vel_a[1] * m_a + vel_b[1] * m_b) / (m_a + m_b),
        (vel_a[2] * m_a + vel_b[2] * m_b) / (m_a + m_b),
    ];

    let comp_a_val = data.compositions.get(idx_a).copied();
    let comp_b_val = data.compositions.get(idx_b).copied();
    let (Some(comp_a), Some(comp_b)) = (comp_a_val, comp_b_val) else {
        return;
    };

    let mut merged_comp = comp_a.mass_weighted_merge(f64::from(m_a), &comp_b, f64::from(m_b));
    if is_beyond_snowline && (m_a + m_b) > (2.0 * b_mass) {
        merged_comp.gas_frac = (merged_comp.gas_frac + 0.06).min(0.92);
        merged_comp.ice_frac = (merged_comp.ice_frac * 0.95).max(0.05);
    }

    let temp_a = data.temperatures.get(idx_a).copied().unwrap_or(300.0);
    let temp_b = data.temperatures.get(idx_b).copied().unwrap_or(300.0);
    let merged_temp = temp_a.max(temp_b) + 75.0;

    if let Some(m_slot) = data.masses.get_mut(idx_a) {
        *m_slot += m_b;
    }
    if let Some(v_slot) = data.velocities.get_mut(idx_a) {
        *v_slot = merged_vel;
    }
    if let Some(c_slot) = data.compositions.get_mut(idx_a) {
        *c_slot = merged_comp;
    }
    if let Some(t_slot) = data.temperatures.get_mut(idx_a) {
        *t_slot = merged_temp;
    }

    let (br, bg, bb) = blackbody_to_srgb(f64::from(merged_temp));
    let (cr, cg, cb) = merged_comp.visual_color_tint();
    if let Some(col_slot) = data.colors.get_mut(idx_a) {
        *col_slot = [
            (br * 0.4 + cr * 0.85).clamp(0.4, 1.4),
            (bg * 0.4 + cg * 0.85).clamp(0.35, 1.4),
            (bb * 0.4 + cb * 0.85).clamp(0.3, 1.4),
            1.0,
        ];
    }

    if let Some(m_slot) = data.masses.get_mut(idx_b) {
        *m_slot = 0.0;
    }
    if let Some(p_slot) = data.positions.get_mut(idx_b) {
        *p_slot = [0.0, -5000.0, 0.0];
    }
    if let Some(v_slot) = data.velocities.get_mut(idx_b) {
        *v_slot = [0.0, 0.0, 0.0];
    }
}
