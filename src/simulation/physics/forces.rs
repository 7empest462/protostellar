//! Gravitational acceleration, aerodynamic drag, and planetary migration forces.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::integrator::*;

#[allow(clippy::too_many_arguments)]
pub fn compute_single_body_acc(
    i: usize,
    body: &PhysicsBodyEntry,
    config: &SimulationConfig,
    star_mass: f64,
    star_index: Option<usize>,
    softening_sq: f64,
    massive_data: &[(DVec3, f64, f64, usize)],
    tractor: Option<(DVec3, f64)>,
    is_little_red_dot: bool,
    is_gmc_cluster: bool,
) -> DVec3 {
    let mut pert_acc = DVec3::ZERO;
    let mut star_acc = DVec3::ZERO;
    let pos = body.pos;
    let vel = body.vel;
    let b_mass = body.mass;
    let b_type = body.body_type;

    if is_gmc_cluster {
        let r_len = pos.length();
        let r_core = 160.0;
        let cloud_pull = -(G_ASTRO * 24.0 / (r_len * r_len + r_core * r_core).powf(1.5)) * pos;
        pert_acc += cloud_pull;
    }

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

    let eff_softening_sq = if is_gmc_cluster {
        2.25 // 1.5 AU Plummer softening between stars in the open cluster
    } else {
        softening_sq
    };

    for &(m_pos, m_mass, _m_rad, m_idx) in massive_data {
        if m_idx == i {
            continue;
        }
        let r_vec = pos - m_pos;
        let dist_sq = r_vec.length_squared() + eff_softening_sq;
        let dist = dist_sq.sqrt();
        let pull = -(G_ASTRO * m_mass / (dist_sq * dist)) * r_vec;

        if let Some(s_idx) = star_index {
            if m_idx == s_idx {
                star_acc += pull;
            } else {
                pert_acc += pull;
                if i != s_idx {
                    let m_r_sq = m_pos.length_squared() + eff_softening_sq;
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
        let dist_sq = r_vec.length_squared() + eff_softening_sq;
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
    let max_acc = if star_mass > 100_000.0 {
        1.0e12
    } else if is_little_red_dot {
        25_000_000.0
    } else if is_gmc_cluster {
        40.0 // Bounded to realistic acceleration in an open cluster (prevents slingshot explosions)
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

pub fn apply_planetary_migration_and_lhb(
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
