//! Symplectic N-Body Gravity and Aerodynamic Gas Drag Physics Engine.
//! This system handles ONLY ECS-promoted massive bodies (planets, protoplanets).
//! The 50k particle swarm physics is handled entirely on the GPU.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::disk_migration::apply_type_i_torque_acc;
use crate::simulation::resources::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::*;

struct PhysicsBodyEntry {
    entity: Entity,
    mass: f64,
    pos: DVec3,
    vel: DVec3,
    acc: DVec3,
    radius: f64,
    body_type: BodyType,
    satellite: Option<SatelliteOf>,
    is_central_star: bool,
    name: String,
}

fn is_body_bound_to_star(pos: DVec3, vel: DVec3, star_pos: DVec3, star_mass: f64) -> bool {
    if star_mass <= 1e-6 {
        return false;
    }
    let r_rel = pos - star_pos;
    let r = r_rel.length();
    if r < 1e-5 || r > 50_000.0 {
        return false;
    }
    let mu = G_ASTRO * star_mass;
    let specific_energy = 0.5 * vel.length_squared() - mu / r;
    if specific_energy >= -1e-9 {
        return false;
    }
    let h_vec = r_rel.cross(vel);
    let h = h_vec.length();
    if h < 1e-8 || !h.is_finite() {
        return false;
    }
    let a = -mu / (2.0 * specific_energy);
    if a <= 1e-5 || !a.is_finite() {
        return false;
    }
    let e_vec = vel.cross(h_vec) / mu - r_rel / r;
    let e = e_vec.length();
    e.is_finite() && e < 1.02
}

fn kepler_drift_body(pos: &mut DVec3, vel: &mut DVec3, star_pos: DVec3, star_mass: f64, dt: f64) {
    if star_mass <= 1e-6 || dt.abs() < 1e-12 {
        *pos += *vel * dt;
        return;
    }

    let r_rel = *pos - star_pos;
    let r = r_rel.length();
    if r < 1e-5 || r > 10_000.0 {
        *pos += *vel * dt;
        return;
    }

    let mu = G_ASTRO * star_mass;
    let v_sq = vel.length_squared();
    let specific_energy = 0.5 * v_sq - mu / r;

    if specific_energy >= -1e-9 {
        *pos += *vel * dt;
        return;
    }

    let a = -mu / (2.0 * specific_energy);
    if a <= 1e-5 || !a.is_finite() {
        *pos += *vel * dt;
        return;
    }

    let h_vec = r_rel.cross(*vel);
    let h = h_vec.length();
    if h < 1e-8 || !h.is_finite() {
        *pos += *vel * dt;
        return;
    }

    let e_vec = vel.cross(h_vec) / mu - r_rel / r;
    let e_len = e_vec.length();
    if !e_len.is_finite() {
        *pos += *vel * dt;
        return;
    }
    let p_hat = if e_len > 1e-6 {
        e_vec / e_len
    } else {
        r_rel / r
    };
    let mut e = e_len;
    if e >= 0.999 {
        e = 0.985;
    }

    let w_hat = h_vec / h;
    let q_hat = w_hat.cross(p_hat);

    let x0 = r_rel.dot(p_hat);
    let y0 = r_rel.dot(q_hat);
    let sqrt_1_minus_e2 = (1.0 - e * e).max(1e-12).sqrt();

    let cos_e0 = ((x0 / a) + e).clamp(-1.0, 1.0);
    let sin_e0 = (y0 / (a * sqrt_1_minus_e2)).clamp(-1.0, 1.0);
    let e0 = sin_e0.atan2(cos_e0);
    let m0 = e0 - e * sin_e0;

    let n = (mu / (a * a * a)).sqrt();
    let m1 = m0 + n * dt;
    let m1_norm =
        (m1 + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI;

    let mut e1 = m1_norm + e * m1_norm.sin();
    for _ in 0..5 {
        let f = e1 - e * e1.sin() - m1_norm;
        let f_prime = (1.0 - e * e1.cos()).abs().max(1e-7);
        let delta = f / f_prime;
        e1 -= delta;
        if delta.abs() < 1e-12 {
            break;
        }
    }

    let (sin_e1, cos_e1) = e1.sin_cos();
    let r1 = a * (1.0 - e * cos_e1);
    if r1 < 1e-5 || !r1.is_finite() {
        *pos += *vel * dt;
        return;
    }

    let x1 = a * (cos_e1 - e);
    let y1 = a * sqrt_1_minus_e2 * sin_e1;
    let new_pos = star_pos + x1 * p_hat + y1 * q_hat;

    let v_factor = (mu * a).sqrt() / r1;
    let vx1 = -v_factor * sin_e1;
    let vy1 = v_factor * sqrt_1_minus_e2 * cos_e1;
    let new_vel = vx1 * p_hat + vy1 * q_hat;

    if new_pos.is_finite() && new_vel.is_finite() {
        *pos = new_pos;
        *vel = new_vel;
    } else {
        *pos += *vel * dt;
    }
}

fn advance_symplectic_leapfrog_drift(
    body_data: &mut [PhysicsBodyEntry],
    sub_dt: f64,
    star_pos: DVec3,
    star_mass: f64,
) {
    for body in body_data.iter_mut() {
        if body.satellite.is_none() {
            if body.is_central_star {
                body.pos = DVec3::ZERO;
                body.vel = DVec3::ZERO;
                body.acc = DVec3::ZERO;
            } else if is_body_bound_to_star(body.pos, body.vel, star_pos, star_mass) {
                kepler_drift_body(&mut body.pos, &mut body.vel, star_pos, star_mass, sub_dt);
            } else {
                body.pos += body.vel * sub_dt;
            }
        }
    }

    let snapshot_positions: Vec<(Entity, DVec3, DVec3, f64, f64)> = body_data
        .iter()
        .map(|b| (b.entity, b.pos, b.vel, b.mass, b.radius))
        .collect();

    for body in body_data.iter_mut() {
        if let Some(ref mut sat) = body.satellite {
            if let Some(&(_, parent_pos, parent_vel, parent_mass, parent_radius)) =
                snapshot_positions.iter().find(|(e, ..)| *e == sat.parent)
            {
                let r_orbit = sat.semi_major_axis_au.max(1e-5);
                if r_orbit < parent_radius * 1.05 {
                    body.satellite = None;
                    body.pos += body.vel * sub_dt;
                } else {
                    let omega_moon = if sat.orbital_period_years > 1e-8 {
                        2.0 * std::f64::consts::PI / sat.orbital_period_years
                    } else {
                        (G_ASTRO * parent_mass / (r_orbit * r_orbit * r_orbit)).sqrt()
                    };

                    sat.true_anomaly = (sat.true_anomaly + omega_moon * sub_dt)
                        .rem_euclid(2.0 * std::f64::consts::PI);
                    let cos_a = sat.true_anomaly.cos();
                    let sin_a = sat.true_anomaly.sin();
                    let v_orb_mag = (G_ASTRO * parent_mass / r_orbit).sqrt();

                    body.pos = parent_pos + DVec3::new(r_orbit * cos_a, 0.0, r_orbit * sin_a);
                    body.vel = parent_vel + DVec3::new(-v_orb_mag * sin_a, 0.0, v_orb_mag * cos_a);
                }
            } else {
                body.satellite = None;
                body.pos += body.vel * sub_dt;
            }
        }
    }
}

fn compute_single_body_acc(
    i: usize,
    body: &PhysicsBodyEntry,
    config: &SimulationConfig,
    star_mass: f64,
    star_index: Option<usize>,
    softening_sq: f64,
    massive_data: &[(DVec3, f64, f64, usize)],
    tractor: Option<(DVec3, f64)>,
    is_little_red_dot: bool,
) -> DVec3 {
    let mut pert_acc = DVec3::ZERO;
    let mut star_acc = DVec3::ZERO;
    let pos = body.pos;
    let vel = body.vel;
    let b_mass = body.mass;
    let b_type = body.body_type;

    if !b_type.is_star_or_remnant() {
        if config.enable_gas_drag && config.gas_density_scale > 0.001 {
            let r_cyl = (pos.x * pos.x + pos.z * pos.z).sqrt().max(0.005);
            let v_k = (G_ASTRO * star_mass / r_cyl).sqrt();
            let v_gas_mag = v_k * 0.998;
            let phi = pos.z.atan2(pos.x);
            let v_gas = DVec3::new(-v_gas_mag * phi.sin(), 0.0, v_gas_mag * phi.cos());

            let rel_v = vel - v_gas;
            let rel_speed = rel_v.length();
            let gas_density =
                1e-4 * (r_cyl / 1.0).powf(-2.25) * f64::from(config.gas_density_scale);

            let m_earth = b_mass / EARTH_MASS_SOLAR;
            let inertia_suppression = (1.0 / (1.0 + m_earth * 150.0)).clamp(0.0, 1.0);
            let drag_coeff = 0.025 * gas_density * inertia_suppression;
            pert_acc -= drag_coeff * rel_speed * rel_v;

            let r_unit = DVec3::new(pos.x / r_cyl, 0.0, pos.z / r_cyl);
            let v_radial = vel.dot(r_unit);
            let damp_rate = 0.08 * gas_density * inertia_suppression;
            pert_acc -= r_unit * (v_radial * damp_rate);
            pert_acc.y -= vel.y * damp_rate * 2.0;
        }

        if matches!(b_type, BodyType::DustGrain) {
            let r_cyl = (pos.x * pos.x + pos.z * pos.z).sqrt().max(0.005);
            if r_cyl < 2.0 && config.gas_density_scale < 0.95 {
                let r_unit = DVec3::new(pos.x / r_cyl, 0.0, pos.z / r_cyl);
                let push_mag = 0.35 * (1.0 - (r_cyl / 2.0)).max(0.0)
                    / (1.0 + b_mass / (EARTH_MASS_SOLAR * 0.001));
                pert_acc += r_unit * push_mag;
            }
        }
    }

    for &(m_pos, m_mass, _m_rad, m_idx) in massive_data {
        if m_idx == i {
            continue;
        }
        let r_vec = pos - m_pos;
        let dist_sq = r_vec.length_squared() + softening_sq;
        let dist = dist_sq.sqrt();
        let pull = -(G_ASTRO * m_mass / (dist_sq * dist)) * r_vec;

        if let Some(s_idx) = star_index {
            if m_idx == s_idx {
                star_acc += pull;
            } else {
                pert_acc += pull;
                if i != s_idx {
                    let m_r_sq = m_pos.length_squared() + softening_sq;
                    let m_dist = m_r_sq.sqrt();
                    pert_acc -= (G_ASTRO * m_mass / (m_r_sq * m_dist)) * m_pos;
                }
            }
        } else {
            pert_acc += pull;
        }
    }

    if let Some((t_pos, t_mass)) = tractor {
        let r_vec = pos - t_pos;
        let dist_sq = r_vec.length_squared() + softening_sq;
        let dist = dist_sq.sqrt();
        pert_acc -= (G_ASTRO * t_mass / (dist_sq * dist)) * r_vec;
    }

    let star_pos = if let Some(s_idx) = star_index {
        massive_data
            .iter()
            .find(|(.., idx)| *idx == s_idx)
            .map_or(DVec3::ZERO, |(p, ..)| *p)
    } else {
        DVec3::ZERO
    };

    let is_bound = is_body_bound_to_star(pos, vel, star_pos, star_mass);
    let mut acc = if is_bound {
        pert_acc
    } else {
        pert_acc + star_acc
    };

    let acc_mag = acc.length();
    let max_acc = if is_little_red_dot {
        25_000_000.0
    } else {
        500_000.0
    };
    if acc_mag > max_acc {
        acc *= max_acc / acc_mag;
    }

    if acc.is_finite() {
        acc
    } else {
        DVec3::ZERO
    }
}

fn apply_planetary_migration_and_lhb(
    body_data: &mut [PhysicsBodyEntry],
    lhb_state: &mut crate::game::phases::LateHeavyBombardmentState,
    jupiter_entity: Option<Entity>,
    saturn_entity: Option<Entity>,
    ice_giant_entities: &hashbrown::HashSet<Entity>,
    sub_dt: f64,
    is_compact_system: bool,
) {
    if is_compact_system || !lhb_state.is_active {
        return;
    }
    lhb_state.time_active_years += sub_dt;
    let progress = (lhb_state.time_active_years / 2500.0).clamp(0.0, 1.0);
    lhb_state.migration_progress = progress;

    let jupiter_idx = jupiter_entity.and_then(|je| {
        body_data
            .iter()
            .enumerate()
            .find(|(_, b)| b.entity == je)
            .map(|(i, b)| (i, (b.pos.x * b.pos.x + b.pos.z * b.pos.z).sqrt()))
    });
    let saturn_idx = saturn_entity.and_then(|se| {
        body_data
            .iter()
            .enumerate()
            .find(|(_, b)| b.entity == se)
            .map(|(i, b)| (i, (b.pos.x * b.pos.x + b.pos.z * b.pos.z).sqrt()))
    });

    if let (Some((j_i, r_j)), Some((s_i, r_s))) = (jupiter_idx, saturn_idx) {
        let p_ratio = (r_s / r_j.max(0.1)).powf(1.5);
        lhb_state.resonance_ratio = p_ratio;

        if (p_ratio >= 2.0 || progress >= 0.05) && !lhb_state.resonance_crossed {
            lhb_state.resonance_crossed = true;
            let v_j = body_data.get(j_i).map(|b| b.vel);
            let v_s = body_data.get(s_i).map(|b| b.vel);
            if let Some(v_j) = v_j {
                if let Some(j_b) = body_data.get_mut(j_i) {
                    j_b.vel += v_j.cross(DVec3::Y).normalize_or_zero() * (0.02 * v_j.length());
                }
            }
            if let Some(v_s) = v_s {
                if let Some(s_b) = body_data.get_mut(s_i) {
                    s_b.vel -= v_s.cross(DVec3::Y).normalize_or_zero() * (0.03 * v_s.length());
                }
            }
        }

        if r_j > 5.2 && progress < 0.95 {
            if let Some(j_b) = body_data.get_mut(j_i) {
                let v_dir = j_b.vel.normalize_or_zero();
                j_b.acc -= v_dir * 0.0004;
            }
        }
        if r_s < 9.58 && progress < 0.95 {
            if let Some(s_b) = body_data.get_mut(s_i) {
                let v_dir = s_b.vel.normalize_or_zero();
                s_b.acc += v_dir * 0.0006;
            }
        }
    } else if progress >= 0.05 && !lhb_state.resonance_crossed {
        lhb_state.resonance_crossed = true;
    }

    for (i, b) in body_data.iter_mut().enumerate() {
        let r = (b.pos.x * b.pos.x + b.pos.z * b.pos.z).sqrt();
        let v_dir = b.vel.normalize_or_zero();

        if matches!(b.body_type, BodyType::IceGiant) || ice_giant_entities.contains(&b.entity) {
            if r < 32.0 && progress < 0.95 {
                b.acc += v_dir * 0.0012;
            }
        } else if matches!(b.body_type, BodyType::Asteroid | BodyType::Planetesimal)
            && (1.9..=3.8).contains(&r)
        {
            if lhb_state.resonance_crossed && progress < 0.92 {
                let inward = -b.pos.normalize_or_zero();
                let retrograde = -v_dir;
                let inclination_perturb = DVec3::new(0.0, ((i % 7) as f64 - 3.0) * 0.05, 0.0);
                let kick_dir =
                    (inward * 0.65 + retrograde * 0.35 + inclination_perturb).normalize_or_zero();
                b.acc += kick_dir * 0.006;
                lhb_state.comets_scattered = (lhb_state.comets_scattered + 1).min(100_000);
            }
        } else if matches!(
            b.body_type,
            BodyType::Comet | BodyType::Planetesimal | BodyType::Asteroid
        ) && r >= 12.0
            && lhb_state.resonance_crossed
            && progress < 0.92
        {
            let inward = -b.pos.normalize_or_zero();
            let retrograde = -v_dir;
            let inclination_perturb = DVec3::new(0.0, (i % 5) as f64 * 0.08 - 0.16, 0.0);
            let kick_dir =
                (inward * 0.80 + retrograde * 0.20 + inclination_perturb).normalize_or_zero();
            b.acc += kick_dir * 0.0055;
            lhb_state.comets_scattered = (lhb_state.comets_scattered + 1).min(100_000);
        }
    }
}

fn apply_pert_kick(body: &mut PhysicsBodyEntry, half_dt: f64, star_mass: f64) {
    let r = body.pos.length().max(0.01);
    let v_k = (G_ASTRO * star_mass / r).sqrt();
    let t_orbit = 2.0 * std::f64::consts::PI * r / v_k.max(1e-6);
    let eff_dt = half_dt.min(t_orbit * 0.05);
    let kick = body.acc * eff_dt;
    let kick_mag = kick.length();

    let is_major_planet = matches!(
        body.body_type,
        BodyType::GasGiant
            | BodyType::IceGiant
            | BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::Protoplanet
    );

    let max_kick = if is_major_planet {
        0.0002 * v_k
    } else {
        0.001 * v_k
    };

    if kick_mag > max_kick && max_kick > 0.0 {
        body.vel += kick * (max_kick / kick_mag);
    } else {
        body.vel += kick;
    }

    if is_major_planet {
        let v_esc = (2.0 * G_ASTRO * star_mass / r).sqrt();
        let max_bound_speed = 0.92 * v_esc;
        let speed = body.vel.length();
        if speed > max_bound_speed && max_bound_speed > 0.0 {
            body.vel *= max_bound_speed / speed;
        }
    }
}

fn apply_velocity_kick_and_limits(
    body_data: &mut [PhysicsBodyEntry],
    sub_dt: f64,
    star_mass: f64,
    is_little_red_dot: bool,
) {
    for body in body_data.iter_mut() {
        if body.is_central_star {
            body.pos = DVec3::ZERO;
            body.vel = DVec3::ZERO;
            body.acc = DVec3::ZERO;
        } else {
            apply_pert_kick(body, sub_dt * 0.5, star_mass);

            if !body.pos.is_finite() || !body.vel.is_finite() {
                let safe_r = if is_little_red_dot { 120.0 } else { 1.0 };
                let v_k = (G_ASTRO * star_mass / safe_r).sqrt();
                body.pos = DVec3::new(safe_r, 0.0, 0.0);
                body.vel = DVec3::new(0.0, 0.0, v_k);
                body.acc = DVec3::ZERO;
            }

            let speed = body.vel.length();
            let universal_c_limit = SPEED_OF_LIGHT_AU_YR * 0.999;
            if speed > universal_c_limit {
                body.vel *= universal_c_limit / speed;
            } else if !is_little_red_dot {
                let max_planetary_speed = 1500.0;
                if speed > max_planetary_speed {
                    body.vel *= max_planetary_speed / speed;
                }
            } else {
                let max_smbh_bound_speed = 10_000.0;
                if speed > max_smbh_bound_speed {
                    body.vel *= max_smbh_bound_speed / speed;
                }
            }
        }
    }
}

fn update_physics_energy_monitor(
    energy_monitor: &mut EnergyMonitor,
    body_data: &[PhysicsBodyEntry],
    star_index: Option<usize>,
    star_pos: DVec3,
    star_mass: f64,
    softening_au: f64,
) {
    let mut kinetic_e = 0.0;
    let mut potential_e = 0.0;

    for (i, b) in body_data.iter().enumerate() {
        kinetic_e += 0.5 * b.mass * b.vel.length_squared();

        if let Some(s_idx) = star_index {
            if i != s_idx {
                let dist = (b.pos - star_pos).length() + softening_au;
                potential_e -= (G_ASTRO * star_mass * b.mass) / dist;
            }
        }
    }

    let total_e = kinetic_e + potential_e;
    if !energy_monitor.initialized && total_e.abs() > 1e-10 {
        energy_monitor.initial_total_energy = total_e;
        energy_monitor.initialized = true;
    }

    let rel_drift = if energy_monitor.initial_total_energy.abs() > 1e-10 {
        ((total_e - energy_monitor.initial_total_energy) / energy_monitor.initial_total_energy)
            .abs()
    } else {
        0.0
    };

    energy_monitor.kinetic_energy = kinetic_e;
    energy_monitor.potential_energy = potential_e;
    energy_monitor.total_energy = total_e;
    energy_monitor.relative_energy_drift = rel_drift;
}

#[allow(clippy::type_complexity, reason = "Physics State Writeback")]
fn write_back_physics_results(
    commands: &mut Commands,
    body_data: &[PhysicsBodyEntry],
    is_little_red_dot: bool,
    bodies_query: &mut Query<(
        Entity,
        &mut Mass,
        &mut SimPosition,
        &mut SimVelocity,
        &mut SimAcceleration,
        &Radius,
        &CelestialBody,
        Option<&mut SatelliteOf>,
        Option<&CentralStar>,
    )>,
) {
    let mut escaped_minor_debris: Vec<Entity> = Vec::new();

    for (e, mut m, mut pos, mut vel, mut acc, _, body, opt_sat, opt_central) in
        bodies_query.iter_mut()
    {
        if opt_central.is_some() {
            pos.0 = DVec3::ZERO;
            vel.0 = DVec3::ZERO;
            acc.0 = DVec3::ZERO;
        } else if let Some(b) = body_data.iter().find(|b| b.entity == e) {
            let r_mag = b.pos.length();
            let debris_escape_radius = if is_little_red_dot { 15_000.0 } else { 2000.0 };
            if r_mag > debris_escape_radius
                && matches!(
                    body.body_type,
                    BodyType::Planetesimal
                        | BodyType::Asteroid
                        | BodyType::Comet
                        | BodyType::DustGrain
                )
            {
                escaped_minor_debris.push(e);
                continue;
            }

            m.0 = b.mass;
            pos.0 = b.pos;
            vel.0 = b.vel;
            acc.0 = b.acc;
            if let Some(s_data) = b.satellite {
                if let Some(mut s_comp) = opt_sat {
                    *s_comp = s_data;
                }
            } else if opt_sat.is_some() {
                if let Ok(mut cmd) = commands.get_entity(e) {
                    cmd.remove::<SatelliteOf>();
                }
            }
        }
    }

    for e in escaped_minor_debris {
        if let Ok(mut cmd) = commands.get_entity(e) {
            cmd.despawn();
        }
    }
}

fn identify_giant_planets(
    body_data: &[PhysicsBodyEntry],
) -> (Option<Entity>, Option<Entity>, hashbrown::HashSet<Entity>) {
    let mut jupiter_entity = None;
    let mut saturn_entity = None;
    let mut ice_giant_entities = hashbrown::HashSet::new();

    for b in body_data {
        let r = (b.pos.x * b.pos.x + b.pos.z * b.pos.z).sqrt();
        let name_lower = b.name.to_lowercase();
        if name_lower.contains("jupiter") {
            jupiter_entity = Some(b.entity);
        } else if name_lower.contains("saturn") {
            saturn_entity = Some(b.entity);
        } else if matches!(b.body_type, BodyType::GasGiant) || b.mass >= JUPITER_MASS_SOLAR * 0.05 {
            if r < 7.5 && jupiter_entity.is_none() {
                jupiter_entity = Some(b.entity);
            } else if (7.5..16.0).contains(&r) && saturn_entity.is_none() {
                saturn_entity = Some(b.entity);
            }
        }
        if matches!(b.body_type, BodyType::IceGiant)
            || name_lower.contains("uranus")
            || name_lower.contains("neptune")
        {
            ice_giant_entities.insert(b.entity);
        }
    }
    (jupiter_entity, saturn_entity, ice_giant_entities)
}

fn run_physics_substeps(
    body_data: &mut [PhysicsBodyEntry],
    n_substeps: usize,
    sub_dt: f64,
    massive_indices: &[usize],
    config: &SimulationConfig,
    star_mass: f64,
    star_pos: DVec3,
    star_index: Option<usize>,
    softening_sq: f64,
    tractor: Option<(DVec3, f64)>,
    is_little_red_dot: bool,
    is_compact_system: bool,
    lhb_state: &mut crate::game::phases::LateHeavyBombardmentState,
    jupiter_entity: Option<Entity>,
    saturn_entity: Option<Entity>,
    ice_giant_entities: &hashbrown::HashSet<Entity>,
) {
    for _ in 0..n_substeps {
        for body in body_data.iter_mut() {
            if body.is_central_star {
                body.pos = DVec3::ZERO;
                body.vel = DVec3::ZERO;
                body.acc = DVec3::ZERO;
            } else {
                apply_pert_kick(body, sub_dt * 0.5, star_mass);
            }
        }

        advance_symplectic_leapfrog_drift(body_data, sub_dt, star_pos, star_mass);

        let massive_data: Vec<(DVec3, f64, f64, usize)> = massive_indices
            .iter()
            .filter_map(|&idx| {
                let b = body_data.get(idx)?;
                Some((b.pos, b.mass, b.radius, idx))
            })
            .collect();

        let new_accelerations: Vec<DVec3> = body_data
            .iter()
            .enumerate()
            .map(|(i, b)| {
                compute_single_body_acc(
                    i,
                    b,
                    config,
                    star_mass,
                    star_index,
                    softening_sq,
                    &massive_data,
                    tractor,
                    is_little_red_dot,
                )
            })
            .collect();

        for (body, &new_acc) in body_data.iter_mut().zip(&new_accelerations) {
            if body.is_central_star {
                body.acc = DVec3::ZERO;
            } else {
                body.acc = new_acc;
            }
        }

        // Type-I / II migration while nebular gas remains
        if config.gas_density_scale > 0.001 {
            for body in body_data.iter_mut() {
                if body.is_central_star {
                    continue;
                }
                apply_type_i_torque_acc(
                    body.pos,
                    body.vel,
                    body.mass,
                    body.body_type,
                    body.is_central_star,
                    body.satellite.is_some(),
                    star_mass,
                    config.gas_density_scale,
                    &mut body.acc,
                );
            }
        }

        apply_planetary_migration_and_lhb(
            body_data,
            lhb_state,
            jupiter_entity,
            saturn_entity,
            ice_giant_entities,
            sub_dt,
            is_compact_system,
        );

        apply_velocity_kick_and_limits(body_data, sub_dt, star_mass, is_little_red_dot);
    }
}

struct PhysicsSystemContext {
    star_index: Option<usize>,
    star_mass: f64,
    star_pos: DVec3,
    is_little_red_dot: bool,
    is_compact_system: bool,
    n_substeps: usize,
    sub_dt: f64,
    massive_indices: Vec<usize>,
}

fn analyze_physics_system(
    body_data: &mut [PhysicsBodyEntry],
    scenario_state: Option<&ActiveScenarioState>,
    disk_params: &DiskParameters,
    config: &SimulationConfig,
    dt: f64,
    target_dt: f64,
    softening_sq: f64,
) -> PhysicsSystemContext {
    let star_index = body_data.iter().position(|b| b.is_central_star);
    let (star_mass, star_pos, is_central_quasi) = if let Some(idx) = star_index {
        if let Some(b) = body_data.get(idx) {
            (b.mass, b.pos, b.body_type == BodyType::QuasiStar)
        } else {
            (1.0, DVec3::ZERO, false)
        }
    } else {
        (1.0, DVec3::ZERO, false)
    };

    let is_little_red_dot = scenario_state
        .is_some_and(|s| s.current_preset == ScenarioPreset::LittleRedDot)
        || is_central_quasi
        || star_mass > 10_000.0;

    let is_compact_system = (star_mass < 0.25 && disk_params.outer_radius_au < 1.0)
        || scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::Trappist1System);
    let max_substeps = if is_little_red_dot || is_compact_system {
        config.max_substeps_per_frame.max(128)
    } else {
        config.max_substeps_per_frame
    };
    let eff_dt = if is_compact_system {
        dt.min(0.00004)
    } else {
        dt
    };
    let n_substeps = ((target_dt / eff_dt).ceil() as usize).clamp(1, max_substeps);
    let sub_dt = target_dt / (n_substeps as f64);

    let massive_indices: Vec<usize> = body_data
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            b.mass > EARTH_MASS_SOLAR * 0.1
                || b.is_central_star
                || b.body_type.is_star_or_remnant()
                || matches!(
                    b.body_type,
                    BodyType::Protoplanet
                        | BodyType::TerrestrialPlanet
                        | BodyType::SuperEarth
                        | BodyType::GasGiant
                        | BodyType::IceGiant
                )
        })
        .map(|(i, _)| i)
        .collect();

    for body in body_data {
        if body.acc.length_squared() < 1e-12 && !body.is_central_star {
            let r_vec = body.pos - star_pos;
            let dist_sq = r_vec.length_squared() + softening_sq;
            let dist = dist_sq.sqrt();
            if is_body_bound_to_star(body.pos, body.vel, star_pos, star_mass) {
                body.acc = DVec3::ZERO;
            } else {
                body.acc = -(G_ASTRO * star_mass / (dist_sq * dist)) * r_vec;
            }
        }
    }

    PhysicsSystemContext {
        star_index,
        star_mass,
        star_pos,
        is_little_red_dot,
        is_compact_system,
        n_substeps,
        sub_dt,
        massive_indices,
    }
}

/// Advances accumulated visual simulation time in seconds for shaders and rendering animations.
/// Pauses when time_warp is paused, freezing planetary rotation, cloud movement, and stellar boiling.
pub fn update_simulation_visual_time(
    time: Res<Time>,
    time_warp: Res<TimeWarp>,
    mut sim_time: ResMut<SimTime>,
) {
    if !time_warp.is_paused || time_warp.step_once {
        sim_time.visual_time_secs += time.delta_secs();
    }
}

/// Advances the N-body gravitational physics simulation using a Symplectic Kick-Drift-Kick Leapfrog integrator.
#[allow(clippy::type_complexity, reason = "N-body Simulation State")]
pub fn step_physics_simulation(
    config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    time_warp: Res<TimeWarp>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    player_state: Res<PlayerInteractionState>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    mut commands: Commands,
    mut bodies_query: Query<(
        Entity,
        &mut Mass,
        &mut SimPosition,
        &mut SimVelocity,
        &mut SimAcceleration,
        &Radius,
        &CelestialBody,
        Option<&mut SatelliteOf>,
        Option<&CentralStar>,
    )>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt = config.base_dt_yr;
    let target_dt = dt * time_warp.multiplier.max(TimeWarp::MIN_SPEED);
    let is_compact_system = scenario_state
        .as_deref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::Trappist1System)
        || (disk_params.central_star_mass < 0.25 && disk_params.outer_radius_au < 1.0);
    let eff_softening_au = if is_compact_system {
        0.00005
    } else {
        config.softening_au
    };
    let softening_sq = eff_softening_au * eff_softening_au;

    let mut body_data: Vec<PhysicsBodyEntry> = bodies_query
        .iter()
        .map(
            |(e, m, pos, vel, acc, rad, body, sat, opt_central)| PhysicsBodyEntry {
                entity: e,
                mass: m.0,
                pos: pos.0,
                vel: vel.0,
                acc: acc.0,
                radius: rad.0,
                body_type: body.body_type,
                satellite: sat.copied(),
                is_central_star: opt_central.is_some(),
                name: body.name.clone(),
            },
        )
        .collect();

    if body_data.is_empty() {
        return;
    }

    let ctx = analyze_physics_system(
        &mut body_data,
        scenario_state.as_deref(),
        &disk_params,
        &config,
        dt,
        target_dt,
        softening_sq,
    );

    let tractor = if player_state.active_tool == PlayerTool::GravitationalTractor {
        if let (Some(pos), mass) = (player_state.tractor_position, player_state.tractor_mass) {
            if mass > 0.0 {
                Some((pos, mass))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let (jupiter_entity, saturn_entity, ice_giant_entities) = identify_giant_planets(&body_data);

    run_physics_substeps(
        &mut body_data,
        ctx.n_substeps,
        ctx.sub_dt,
        &ctx.massive_indices,
        &config,
        ctx.star_mass,
        ctx.star_pos,
        ctx.star_index,
        softening_sq,
        tractor,
        ctx.is_little_red_dot,
        ctx.is_compact_system,
        &mut lhb_state,
        jupiter_entity,
        saturn_entity,
        &ice_giant_entities,
    );

    if let (Some(target), Some(dv)) = (
        player_state.impulse_target_entity,
        player_state.impulse_delta_v,
    ) {
        if let Some(b) = body_data.iter_mut().find(|b| b.entity == target) {
            b.vel += dv;
        }
    }

    write_back_physics_results(
        &mut commands,
        &body_data,
        ctx.is_little_red_dot,
        &mut bodies_query,
    );

    update_physics_energy_monitor(
        &mut energy_monitor,
        &body_data,
        ctx.star_index,
        ctx.star_pos,
        ctx.star_mass,
        eff_softening_au,
    );

    sim_time.elapsed_years += target_dt;
    sim_time.current_dt_yr = target_dt;
    sim_time.step_count += ctx.n_substeps as u64;
}
