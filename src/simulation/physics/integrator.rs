//! Symplectic Kick-Drift-Kick leapfrog integration and Kepler drift routines.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::{BodyType, SatelliteOf};
use crate::utils::constants::*;

pub const ROLE_MOON_NAMED: u8 = 1 << 0;
pub const ROLE_JUPITER: u8 = 1 << 1;
pub const ROLE_SATURN: u8 = 1 << 2;
pub const ROLE_ICE_GIANT_NAMED: u8 = 1 << 3;
pub const ROLE_INTERLOPER: u8 = 1 << 4;

pub fn compute_body_role_flags(name: &str) -> u8 {
    let mut flags = 0u8;
    if crate::utils::math::contains_ascii_case_insensitive(name, "(moon)") {
        flags |= ROLE_MOON_NAMED;
    }
    if name.eq_ignore_ascii_case("jupiter")
        || name.eq_ignore_ascii_case("proto-jupiter")
        || name.eq_ignore_ascii_case("hot jupiter")
    {
        flags |= ROLE_JUPITER;
    } else if name.eq_ignore_ascii_case("saturn") || name.eq_ignore_ascii_case("proto-saturn") {
        flags |= ROLE_SATURN;
    }
    if crate::utils::math::contains_ascii_case_insensitive(name, "uranus")
        || crate::utils::math::contains_ascii_case_insensitive(name, "neptune")
    {
        flags |= ROLE_ICE_GIANT_NAMED;
    }
    if name.contains("Rogue") || name.contains("Interloper") {
        flags |= ROLE_INTERLOPER;
    }
    flags
}

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
    pub role_flags: u8,
}

/// Shared predicate selecting bodies that participate as significant N-body gravitational sources
/// across both CPU near-field evaluation and GPU `gmc_far_field` extraction.
#[inline]
pub fn is_massive_gravity_source(mass_solar: f64, body_type: BodyType, is_central: bool) -> bool {
    mass_solar > EARTH_MASS_SOLAR * 0.1
        || is_central
        || body_type.is_star_or_remnant()
        || body_type.is_planet()
}

pub fn is_body_bound_to_star(
    pos: DVec3,
    vel: DVec3,
    star_pos: DVec3,
    star_mass: f64,
    body_mass: f64,
) -> bool {
    if star_mass <= 1e-6 {
        return false;
    }
    let r_rel = pos - star_pos;
    let r = r_rel.length();
    if r < 1e-5 || r > 50_000.0 {
        return false;
    }
    let eff_body_mass = body_mass.max(0.0);
    let mu = G_ASTRO * (star_mass + eff_body_mass);
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

/// Determines whether a body should integrate via single-star Kepler splitting (`kepler_drift_body`
/// around `star_mass`) or full Cartesian symplectic Kick-Drift-Kick.
///
/// Circumbinary planets/debris orbiting outside a massive stellar companion do not follow a
/// 2-body Kepler ellipse around `star_mass` alone and receive large indirect companion accelerations
/// that must not be clamped against single-star orbital speed.
#[inline]
pub fn should_use_kepler_splitting(
    pos: DVec3,
    vel: DVec3,
    body_mass: f64,
    body_type: BodyType,
    star_pos: DVec3,
    star_mass: f64,
    inner_companion_radius: Option<f64>,
) -> bool {
    let is_circumbinary = !body_type.is_star_or_remnant()
        && inner_companion_radius.is_some_and(|r_comp| (pos - star_pos).length() > r_comp * 1.15);
    !is_circumbinary && is_body_bound_to_star(pos, vel, star_pos, star_mass, body_mass)
}

fn kepler_drift_body(
    pos: &mut DVec3,
    vel: &mut DVec3,
    star_pos: DVec3,
    star_mass: f64,
    body_mass: f64,
    dt: f64,
) {
    if star_mass <= 1e-6 || dt.abs() < 1e-12 {
        *pos += *vel * dt;
        return;
    }
    let r_rel = *pos - star_pos;
    let r = r_rel.length();
    let eff_body_mass = body_mass.max(0.0);
    let mu = G_ASTRO * (star_mass + eff_body_mass);
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
    snapshot_buffer: &mut Vec<(Entity, DVec3, DVec3, f64, f64)>,
    is_galaxy_collision: bool,
) {
    advance_symplectic_leapfrog_drift_with_companion(
        body_data,
        sub_dt,
        star_pos,
        star_mass,
        snapshot_buffer,
        is_galaxy_collision,
        false,
        None,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Leapfrog drift requires companion separation and scenario flags"
)]
pub fn advance_symplectic_leapfrog_drift_with_companion(
    body_data: &mut [PhysicsBodyEntry],
    sub_dt: f64,
    star_pos: DVec3,
    star_mass: f64,
    snapshot_buffer: &mut Vec<(Entity, DVec3, DVec3, f64, f64)>,
    is_galaxy_collision: bool,
    is_gmc_cluster: bool,
    inner_companion_radius: Option<f64>,
) {
    let has_satellites = body_data.iter().any(|b| b.satellite.is_some());
    if has_satellites {
        // 1. Capture pre-drift parent states so satellite plane normals & relative velocities
        //    are measured BEFORE the parent planet drifts forward over sub_dt.
        snapshot_buffer.clear();
        snapshot_buffer.extend(
            body_data
                .iter()
                .filter(|b| b.satellite.is_none())
                .map(|b| (b.entity, b.pos, b.vel, b.mass, b.radius)),
        );
        snapshot_buffer.sort_unstable_by_key(|&(e, ..)| e);

        for body in body_data.iter_mut() {
            if let Some(ref mut sat) = body.satellite {
                if let Ok(parent_idx) =
                    snapshot_buffer.binary_search_by_key(&sat.parent, |&(e, ..)| e)
                {
                    let Some(&(_, pre_p_pos, pre_p_vel, parent_mass, parent_radius)) =
                        snapshot_buffer.get(parent_idx)
                    else {
                        continue;
                    };
                    let r_orbit = sat.semi_major_axis_au.max(1e-5);
                    if r_orbit < parent_radius * 1.05 {
                        // Loop 2 drifts every detached (satellite.is_none()) body once this substep.
                        body.satellite = None;
                    } else {
                        let omega_moon = if sat.orbital_period_years > 1e-8 {
                            2.0 * std::f64::consts::PI / sat.orbital_period_years
                        } else {
                            (G_ASTRO * (parent_mass + body.mass.max(0.0))
                                / (r_orbit * r_orbit * r_orbit))
                                .sqrt()
                        };
                        let d_theta = omega_moon * sub_dt;
                        let v_orb_mag = omega_moon * r_orbit;
                        sat.true_anomaly =
                            (sat.true_anomaly + d_theta).rem_euclid(2.0 * std::f64::consts::PI);

                        // Advance in pre-drift parent-relative frame using conserved h_hat = (r x v).normalize()
                        // so 3D orbital inclination never decays at ascending/descending nodes.
                        let rel_pos = body.pos - pre_p_pos;
                        let rel_vel = body.vel - pre_p_vel;
                        let h_vec = rel_pos.cross(rel_vel);
                        if rel_pos.length_squared() > 1e-12 && h_vec.length_squared() > 1e-18 {
                            let h_hat = h_vec.normalize();
                            let r_hat = rel_pos.normalize();
                            let t_hat = h_hat.cross(r_hat).normalize_or_zero();
                            let (s, c) = d_theta.sin_cos();
                            let r_new_hat = (r_hat * c + t_hat * s).normalize();
                            let t_new_hat = h_hat.cross(r_new_hat).normalize_or_zero();
                            body.pos = r_new_hat * r_orbit;
                            body.vel = t_new_hat * v_orb_mag;
                        } else {
                            let (sin_a, cos_a) = sat.true_anomaly.sin_cos();
                            body.pos = DVec3::new(r_orbit * cos_a, 0.0, r_orbit * sin_a);
                            body.vel = DVec3::new(-v_orb_mag * sin_a, 0.0, v_orb_mag * cos_a);
                        }
                    }
                } else {
                    // Missing parent: detach and let Loop 2 perform the single drift step.
                    body.satellite = None;
                }
            }
        }
    }

    for body in body_data.iter_mut() {
        if body.satellite.is_none() {
            if body.is_central_star && !is_galaxy_collision {
                body.pos = DVec3::ZERO;
                body.vel = DVec3::ZERO;
                body.acc = DVec3::ZERO;
            } else if !is_gmc_cluster
                && should_use_kepler_splitting(
                    body.pos,
                    body.vel,
                    body.mass,
                    body.body_type,
                    star_pos,
                    star_mass,
                    inner_companion_radius,
                )
            {
                kepler_drift_body(
                    &mut body.pos,
                    &mut body.vel,
                    star_pos,
                    star_mass,
                    body.mass,
                    sub_dt,
                );
            } else {
                body.pos += body.vel * sub_dt;
            }
        }
    }

    if !has_satellites {
        return;
    }

    // 2. Capture post-drift parent positions/velocities & re-anchor satellites.
    //    `snapshot_buffer` retains post-drift parent velocities for `sync_satellite_velocities_to_parents`.
    snapshot_buffer.clear();
    snapshot_buffer.extend(
        body_data
            .iter()
            .filter(|b| b.satellite.is_none())
            .map(|b| (b.entity, b.pos, b.vel, b.mass, b.radius)),
    );
    snapshot_buffer.sort_unstable_by_key(|&(e, ..)| e);

    for body in body_data.iter_mut() {
        if let Some(sat) = body.satellite
            && let Ok(parent_idx) = snapshot_buffer.binary_search_by_key(&sat.parent, |&(e, ..)| e)
            && let Some(&(_, post_p_pos, post_p_vel, ..)) = snapshot_buffer.get(parent_idx)
        {
            body.pos += post_p_pos;
            body.vel += post_p_vel;
        }
    }
}

/// Keeps bound satellite world velocities locked to their parent's second half-kick delta.
pub fn sync_satellite_velocities_to_parents(
    body_data: &mut [PhysicsBodyEntry],
    snapshot_buffer: &mut [(Entity, DVec3, DVec3, f64, f64)],
) {
    if snapshot_buffer.is_empty() || !body_data.iter().any(|b| b.satellite.is_some()) {
        return;
    }

    for b in body_data.iter() {
        if b.satellite.is_none()
            && let Ok(idx) = snapshot_buffer.binary_search_by_key(&b.entity, |&(e, ..)| e)
            && let Some(slot) = snapshot_buffer.get_mut(idx)
        {
            slot.2 = b.vel - slot.2;
        }
    }

    for b in body_data.iter_mut() {
        if let Some(sat) = b.satellite
            && let Ok(idx) = snapshot_buffer.binary_search_by_key(&sat.parent, |&(e, ..)| e)
            && let Some(&(_, _, delta_v, ..)) = snapshot_buffer.get(idx)
        {
            b.vel += delta_v;
        }
    }
}

pub fn apply_pert_kick(
    body: &mut PhysicsBodyEntry,
    half_dt: f64,
    star_pos: DVec3,
    star_mass: f64,
    is_bound: bool,
) {
    if body.satellite.is_some() {
        return;
    }
    if !is_bound {
        body.vel += body.acc * half_dt;
        return;
    }

    let r_rel = body.pos - star_pos;
    let r = r_rel.length().max(0.0005);
    let eff_body_mass = body.mass.max(0.0);
    let mu = G_ASTRO * (star_mass + eff_body_mass);
    let v_k = (mu / r).sqrt();
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

    let orbit_step_frac = (eff_dt / t_orbit.max(1e-9)).clamp(0.002, 0.05);
    let max_kick = if is_major_planet {
        0.08 * orbit_step_frac * v_k
    } else {
        0.06 * orbit_step_frac * v_k
    };
    if kick_mag > max_kick && max_kick > 0.0 {
        body.vel += kick * (max_kick / kick_mag);
    } else {
        body.vel += kick;
    }
}

pub fn apply_velocity_kick_and_limits(
    body_data: &mut [PhysicsBodyEntry],
    sub_dt: f64,
    star_mass: f64,
    star_pos: DVec3,
    is_little_red_dot: bool,
    is_gmc_cluster: bool,
    is_galaxy_collision: bool,
) {
    apply_velocity_kick_and_limits_with_companion(
        body_data,
        sub_dt,
        star_mass,
        star_pos,
        is_little_red_dot,
        is_gmc_cluster,
        is_galaxy_collision,
        None,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Symplectic binary-safe kick and velocity clamping pass"
)]
pub fn apply_velocity_kick_and_limits_with_companion(
    body_data: &mut [PhysicsBodyEntry],
    sub_dt: f64,
    star_mass: f64,
    star_pos: DVec3,
    is_little_red_dot: bool,
    is_gmc_cluster: bool,
    is_galaxy_collision: bool,
    inner_companion_radius: Option<f64>,
) {
    let has_massive_gmc_anchor =
        is_gmc_cluster && (star_mass > 50.0 || body_data.iter().any(|b| b.mass > 50.0));
    for body in body_data.iter_mut() {
        if body.is_central_star && !is_galaxy_collision {
            body.pos = DVec3::ZERO;
            body.vel = DVec3::ZERO;
            body.acc = DVec3::ZERO;
        } else {
            let use_kepler = !is_gmc_cluster
                && should_use_kepler_splitting(
                    body.pos,
                    body.vel,
                    body.mass,
                    body.body_type,
                    star_pos,
                    star_mass,
                    inner_companion_radius,
                );
            apply_pert_kick(body, sub_dt * 0.5, star_pos, star_mass, use_kepler);

            if !body.pos.is_finite() || !body.vel.is_finite() {
                let safe_r = if is_little_red_dot || star_mass > 100_000.0 {
                    120.0
                } else {
                    1.0
                };
                let v_k = (G_ASTRO * star_mass / safe_r).sqrt();
                body.pos = DVec3::new(safe_r, 0.0, 0.0);
                body.vel = DVec3::new(0.0, 0.0, v_k);
                body.acc = DVec3::ZERO;
            }

            let speed = body.vel.length();
            if is_gmc_cluster {
                let max_gmc_speed = if has_massive_gmc_anchor { 1000.0 } else { 8.0 }; // relax limit once SMBH forms
                if speed > max_gmc_speed {
                    body.vel *= max_gmc_speed / speed;
                }
                // Cloud boundary tethering: prevent stars from flying out beyond the expanded GMC envelope while preserving tangential angular momentum
                let r = body.pos.length();
                let max_gmc_radius =
                    crate::simulation::scenarios::molecular_cloud::GMC_CORE_RADIUS_AU + 50.0;
                if r > max_gmc_radius && r.is_finite() {
                    let outward = body.pos / r;
                    let v_rad = body.vel.dot(outward);
                    if v_rad > 0.0 {
                        body.vel -= outward * (v_rad * 1.5);
                    }
                    body.pos = outward * max_gmc_radius;
                }
            } else {
                let universal_c_limit = SPEED_OF_LIGHT_AU_YR * 0.999;
                if speed > universal_c_limit {
                    body.vel *= universal_c_limit / speed;
                } else if is_galaxy_collision {
                    let max_galaxy_speed = 60_000.0;
                    if speed > max_galaxy_speed {
                        body.vel *= max_galaxy_speed / speed;
                    }
                } else if !(is_little_red_dot || star_mass > 100.0 || body.body_type.is_remnant()) {
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
