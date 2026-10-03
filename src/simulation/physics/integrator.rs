//! Symplectic Kick-Drift-Kick leapfrog integration and Kepler drift routines.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::{BodyType, SatelliteOf};
use crate::utils::constants::*;

pub struct PhysicsBodyEntry {
    pub entity: Entity,
    pub mass: f64,
    pub pos: DVec3,
    pub vel: DVec3,
    pub acc: DVec3,
    pub radius: f64,
    pub body_type: BodyType,
    pub satellite: Option<SatelliteOf>,
    pub is_central_star: bool,
    pub name: String,
}

pub fn is_body_bound_to_star(pos: DVec3, vel: DVec3, star_pos: DVec3, star_mass: f64) -> bool {
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
    e.is_finite() && e < 0.999
}

fn kepler_drift_body(pos: &mut DVec3, vel: &mut DVec3, star_pos: DVec3, star_mass: f64, dt: f64) {
    if star_mass <= 1e-6 || dt.abs() < 1e-12 {
        *pos += *vel * dt;
        return;
    }
    let r_rel = *pos - star_pos;
    let r = r_rel.length();
    let mu = G_ASTRO * star_mass;
    let v_sq = vel.length_squared();
    let specific_energy = 0.5 * v_sq - mu / r;
    if r < 1e-5 || r > 50_000.0 || specific_energy >= -1e-9 {
        *pos += *vel * dt;
        return;
    }
    let a = -mu / (2.0 * specific_energy);
    let h_vec = r_rel.cross(*vel);
    let h = h_vec.length();
    if a <= 1e-5 || !a.is_finite() || h < 1e-8 || !h.is_finite() {
        *pos += *vel * dt;
        return;
    }
    let e_vec = vel.cross(h_vec) / mu - r_rel / r;
    let e_len = e_vec.length();
    if !e_len.is_finite() || e_len >= 0.999 {
        *pos += *vel * dt;
        return;
    }
    let p_hat = if e_len > 1e-6 {
        e_vec / e_len
    } else {
        r_rel / r
    };
    let e = e_len;

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

    let mut e1 = if e > 0.8 {
        let sign = if m1_norm < 0.0 { -1.0 } else { 1.0 };
        let abs_m = m1_norm.abs();
        if abs_m < 0.5 {
            sign * (6.0 * abs_m / e).cbrt()
        } else {
            m1_norm + sign * 0.85 * e
        }
    } else {
        m1_norm + e * m1_norm.sin()
    };
    for _ in 0..25 {
        let (s, c) = e1.sin_cos();
        let f = e1 - e * s - m1_norm;
        let f_prime = (1.0 - e * c).abs().max(1e-7);
        let f_double = e * s;
        let denom = f_prime - 0.5 * f * f_double / f_prime;
        let delta = if denom.abs() > 1e-12 {
            (f / denom).clamp(-0.8, 0.8)
        } else {
            (f / f_prime).clamp(-0.8, 0.8)
        };
        e1 -= delta;
        if delta.abs() < 1e-13 {
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

pub fn advance_symplectic_leapfrog_drift(
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

pub fn apply_pert_kick(body: &mut PhysicsBodyEntry, half_dt: f64, star_mass: f64, is_bound: bool) {
    if !is_bound {
        body.vel += body.acc * half_dt;
        return;
    }

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

    let max_bound_factor = if is_major_planet && star_mass < 15.0 {
        Some(0.995)
    } else if star_mass > 100_000.0 {
        Some(0.9999)
    } else if is_bound && star_mass >= 15.0 {
        Some(0.9995)
    } else {
        None
    };
    if let Some(factor) = max_bound_factor {
        let v_esc = (2.0 * G_ASTRO * star_mass / r).sqrt();
        let max_bound_speed = factor * v_esc;
        let speed = body.vel.length();
        if speed > max_bound_speed && max_bound_speed > 0.0 {
            body.vel *= max_bound_speed / speed;
        }
    }
}

pub fn apply_velocity_kick_and_limits(
    body_data: &mut [PhysicsBodyEntry],
    sub_dt: f64,
    star_mass: f64,
    star_pos: DVec3,
    is_little_red_dot: bool,
    is_gmc_cluster: bool,
) {
    for body in body_data.iter_mut() {
        if body.is_central_star {
            body.pos = DVec3::ZERO;
            body.vel = DVec3::ZERO;
            body.acc = DVec3::ZERO;
        } else {
            let is_bound = is_body_bound_to_star(body.pos, body.vel, star_pos, star_mass);
            apply_pert_kick(body, sub_dt * 0.5, star_mass, is_bound);

            if !body.pos.is_finite() || !body.vel.is_finite() {
                let safe_r = if is_little_red_dot { 120.0 } else { 1.0 };
                let v_k = (G_ASTRO * star_mass / safe_r).sqrt();
                body.pos = DVec3::new(safe_r, 0.0, 0.0);
                body.vel = DVec3::new(0.0, 0.0, v_k);
                body.acc = DVec3::ZERO;
            }

            let speed = body.vel.length();
            if is_gmc_cluster {
                let max_gmc_speed = 8.0; // ~38 km/s (physical escape velocity in a 24 M_sun cloud is ~29 km/s)
                if speed > max_gmc_speed {
                    body.vel *= max_gmc_speed / speed;
                }
                // Cloud boundary tethering: prevent stars from flying out to millions of AU where floating point distortion occurs
                let r = body.pos.length();
                if r > 240.0 {
                    let inward = -body.pos.normalize();
                    body.vel = inward * body.vel.length().clamp(1.5, 4.0);
                    body.pos = body.pos.normalize() * 240.0;
                }
            } else {
                let universal_c_limit = SPEED_OF_LIGHT_AU_YR * 0.999;
                if speed > universal_c_limit {
                    body.vel *= universal_c_limit / speed;
                } else if !is_little_red_dot {
                    let max_planetary_speed = 3500.0;
                    if speed > max_planetary_speed {
                        body.vel *= max_planetary_speed / speed;
                    }
                } else {
                    let max_smbh_bound_speed = SPEED_OF_LIGHT_AU_YR * 0.95;
                    if speed > max_smbh_bound_speed {
                        body.vel *= max_smbh_bound_speed / speed;
                    }
                }
            }
        }
    }
}
