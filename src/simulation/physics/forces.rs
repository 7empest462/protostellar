//! Gravitational acceleration, aerodynamic drag, and planetary migration forces.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::integrator::*;

fn compute_gmc_cluster_acc(
    pos: DVec3,
    vel: DVec3,
    b_mass: f64,
    b_type: BodyType,
    cluster_com_mass: (DVec3, f64),
    elapsed_years: f64,
) -> DVec3 {
    let mut pert_acc = DVec3::ZERO;
    let (cluster_com, cluster_stellar_mass) = cluster_com_mass;
    let r_vec = pos - cluster_com;
    let r_len = r_vec.length();
    let r_core = 35.0;
    let total_core_mass = 50.0 + cluster_stellar_mass * 0.6;
    let cloud_pull =
        -(G_ASTRO * total_core_mass / (r_len * r_len + r_core * r_core).powf(1.5)) * r_vec;
    pert_acc += cloud_pull;

    // Dark Matter NFW Halo to hold the galaxy structure together in the outer reaches
    let dark_matter_mass = 8000.0;
    let r_s = 150.0; // scale radius
    let x = r_len / r_s;
    if x > 1e-4 {
        let ln_1p_x = (1.0 + x).ln();
        let mass_enclosed = dark_matter_mass * (ln_1p_x - x / (1.0 + x));
        pert_acc += -(G_ASTRO * mass_enclosed / (r_len * r_len)) * (r_vec / r_len);
    }

    if b_type.is_remnant() || b_mass >= 2.0 {
        pert_acc += gmc_dynamical_friction_acc(b_type, pos, vel, b_mass, cluster_com);
    }

    // Lin-Shu logarithmic spiral density wave gravitational perturbation
    pert_acc += gmc_spiral_density_wave_acc(pos - cluster_com, elapsed_years);
    pert_acc
}

fn compute_gas_drag_acc(
    pos: DVec3,
    vel: DVec3,
    b_mass: f64,
    b_type: BodyType,
    config: &SimulationConfig,
    star_mass: f64,
) -> DVec3 {
    let mut pert_acc = DVec3::ZERO;
    if config.enable_gas_drag && config.gas_density_scale > 0.001 {
        let r_cyl = (pos.x * pos.x + pos.z * pos.z).sqrt().max(0.005);
        let v_k = (G_ASTRO * star_mass / r_cyl).sqrt();
        let v_gas_mag = v_k * 0.998;
        let phi = pos.z.atan2(pos.x);
        let v_gas = DVec3::new(-v_gas_mag * phi.sin(), 0.0, v_gas_mag * phi.cos());

        let rel_v = vel - v_gas;
        let rel_speed = rel_v.length();
        let gas_density = 1e-4 * (r_cyl / 1.0).powf(-2.25) * f64::from(config.gas_density_scale);

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
            let push_mag =
                0.35 * (1.0 - (r_cyl / 2.0)).max(0.0) / (1.0 + b_mass / (EARTH_MASS_SOLAR * 0.001));
            pert_acc += r_unit * push_mag;
        }
    }
    pert_acc
}

#[allow(
    clippy::too_many_arguments,
    reason = "Physics kernel requires all integration state explicitly for performance"
)]
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
    cluster_com_mass: (DVec3, f64),
    elapsed_years: f64,
) -> DVec3 {
    let mut pert_acc = DVec3::ZERO;
    let mut star_acc = DVec3::ZERO;
    let pos = body.pos;
    let vel = body.vel;
    let b_mass = body.mass;
    let b_type = body.body_type;

    if is_gmc_cluster {
        pert_acc +=
            compute_gmc_cluster_acc(pos, vel, b_mass, b_type, cluster_com_mass, elapsed_years);
    }

    if !b_type.is_star_or_remnant() {
        pert_acc += compute_gas_drag_acc(pos, vel, b_mass, b_type, config, star_mass);
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
    let max_acc = if is_gmc_cluster {
        40.0 // Bounded to realistic acceleration in an open cluster (prevents slingshot explosions)
    } else if star_mass > 100_000.0 {
        1.0e12
    } else if is_little_red_dot {
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

/// Computes Chandrasekhar dynamical friction deceleration for massive stars and remnants
/// moving through the Giant Molecular Cloud background gas.
///
/// Formula: a_DF = -4 * pi * G^2 * M * rho(r) * ln(Lambda) * [erf(X) - 2X/sqrt(pi) * exp(-X^2)] / v^3 * v
pub fn gmc_dynamical_friction_acc(
    b_type: BodyType,
    pos: DVec3,
    vel: DVec3,
    mass_solar: f64,
    center_pos: DVec3,
) -> DVec3 {
    let speed = vel.length();
    if speed < 1e-6 || mass_solar < 2.0 {
        return DVec3::ZERO;
    }

    let r_vec = pos - center_pos;
    let r_len = r_vec.length();
    let r_core = 35.0;

    // Plummer gas density profile: rho(r) = rho_0 / (1 + (r/r_c)^2)^1.5
    let rho_0 = 3.4e-11; // M_sun / AU^3
    let rho = rho_0 / (1.0 + (r_len / r_core).powi(2)).powf(1.5);

    // Typical cloud turbulent velocity dispersion: sigma_v ~ 0.15 AU/yr (~0.7 km/s)
    let sigma_v = 0.15;
    let x = speed / (std::f64::consts::SQRT_2 * sigma_v);

    // Error function approximation: erf(x) ~ tanh(1.1987 * x)
    let erf_x = (1.1987 * x).tanh();
    let f_x = (erf_x - (2.0 * x / std::f64::consts::PI.sqrt()) * (-x * x).exp()).max(0.0);

    let ln_lambda = 3.2; // Coulomb logarithm

    // Massive black holes sink very rapidly to form a galactic nucleus,
    // while ordinary stars only experience a small amount of realistic drag.
    let boost = if b_type == BodyType::BlackHole {
        80_000.0 // Supermassive black hole seeds experience extreme dynamical friction
    } else if mass_solar > 20.0 {
        800.0
    } else {
        2.0
    };

    // a_DF = -4 * pi * G^2 * M * rho * ln_lambda * (F(X) / v^3) * v
    let factor =
        4.0 * std::f64::consts::PI * G_ASTRO * G_ASTRO * mass_solar * rho * ln_lambda * boost;

    let drag_mag = factor * (f_x / (speed * speed * speed));

    if r_len > 1e-4 {
        let r_hat = r_vec / r_len;
        let v_radial = vel.dot(r_hat);
        let v_tangential = vel - r_hat * v_radial;

        // 1. Tangential drag: rapidly removes orbital angular momentum, causing orbits to plunge inward
        let a_tan = -v_tangential * drag_mag;

        // 2. Radial motion:
        // - If moving outward (v_radial > 0), decelerate the outward escape
        // - If falling inward (v_radial <= 0), DO NOT oppose inward gravitational collapse!
        let a_rad = if v_radial > 0.0 {
            -r_hat * (v_radial * drag_mag)
        } else {
            DVec3::ZERO
        };

        // 3. Dynamical mass segregation drift: massive remnants experience inward gravitational drift toward the center of mass
        let seg_mass = if b_type == BodyType::BlackHole {
            mass_solar.max(30.0) // Black holes do not get clamped, they sink fast
        } else {
            mass_solar.clamp(2.0, 30.0)
        };
        let a_seg = -r_hat * (G_ASTRO * seg_mass * 0.18 / (r_len * r_len + 36.0));

        let mut a_tot = a_tan + a_rad + a_seg;

        // Extreme artificial orbital decay for Black Holes to ensure they plunge and merge within human gameplay time!
        if b_type == BodyType::BlackHole {
            // Remove 5% of tangential velocity per year to guarantee rapid orbit decay
            a_tot -= v_tangential * 0.20;

            // Also damp radial velocity slightly to prevent excessive slingshotting if it misses the exact center
            if v_radial < 0.0 {
                a_tot += r_hat * (v_radial * 0.02);
            }
        }

        // When inside the central core, damp remaining speed to settle and park firmly at the COM
        let capture_radius = if b_type == BodyType::BlackHole {
            25.0
        } else {
            2.5
        };
        if r_len < capture_radius {
            // Massive damping for BHs hitting the center so they instantly park and merge
            let damping_factor = if b_type == BodyType::BlackHole {
                5.0
            } else {
                0.85
            };
            a_tot -= vel * damping_factor;
        }

        let a_mag = a_tot.length();
        if a_mag > 30.0 {
            a_tot * (30.0 / a_mag)
        } else {
            a_tot
        }
    } else {
        let a_origin = -vel * drag_mag.min(5.0);
        let a_mag = a_origin.length();
        if a_mag > 30.0 {
            a_origin * (30.0 / a_mag)
        } else {
            a_origin
        }
    }
}
/// Computes the Lin-Shu logarithmic spiral density wave gravitational perturbation.
///
/// In disk galaxies, spiral arms are rotating non-axisymmetric density wave perturbations:
///   Phi_spiral(R, phi, y, t) = -A(R, t) * cos(m * (phi - Omega_p * t) - k * ln(R / R_0)) * sech^2(y / z_0)
///
/// The acceleration is a = -grad(Phi_spiral), guiding stellar orbits into trailing grand-design spiral arms.
pub fn gmc_spiral_density_wave_acc(pos: DVec3, elapsed_years: f64) -> DVec3 {
    let r_cyl = (pos.x * pos.x + pos.z * pos.z).sqrt();
    if r_cyl < 15.0 || r_cyl > 550.0 || elapsed_years < 600.0 {
        return DVec3::ZERO;
    }

    // Pattern speed: ~0.00045 rad/year (rotation period ~14,000 years)
    let omega_p = 0.00045;
    let pattern_angle = omega_p * elapsed_years;

    // m = 2 (Two-armed grand design spiral)
    let m = 2.0;

    // Pitch angle i ~ 18 deg -> k = 1 / tan(i) ~ 3.08
    let k = 3.08;
    let r_0 = 60.0; // Reference radius in AU

    let phi = pos.z.atan2(pos.x);
    // Spiral phase: xi = m * (phi - Omega_p * t) - k * ln(R / R_0)
    let xi = m * (phi - pattern_angle) - k * (r_cyl / r_0).ln();
    let sin_xi = xi.sin();
    let cos_xi = xi.cos();

    // Vertical profile: sech^2(y / z_0) = 1 / cosh^2(y / z_0)
    let z_0 = 35.0; // Vertical scale height in AU
    let y_scaled = pos.y / z_0;
    let cosh_y = y_scaled.cosh();
    let sech2_y = 1.0 / (cosh_y * cosh_y).max(1e-6);

    // Radial amplitude: vanishes at center (protects bulge/SMBH), peaks at ~180-220 AU, tapers at edge
    let r_scale = 120.0;
    let r_disk = 320.0;
    let u = r_cyl / r_scale;
    let radial_profile = (u / (1.0 + u * u)) * (-r_cyl / r_disk).exp();

    // Temporal emergence: starts gentle at ~600 yr and reaches full strength by ~2,000 yr
    let t_factor = ((elapsed_years - 600.0) / 1400.0).clamp(0.0, 1.0);
    let smooth_t = t_factor * t_factor * (3.0 - 2.0 * t_factor); // smoothstep

    if smooth_t <= 0.001 {
        return DVec3::ZERO;
    }

    // Perturbation amplitude (~12-15% of background NFW halo force)
    let a_0 = G_ASTRO * 550.0 * smooth_t;
    let phi_amp = a_0 * radial_profile * sech2_y;

    // Derivative d(radial_profile)/dr:
    let d_rad_prof = (1.0 - u * u) / (r_scale * (1.0 + u * u).powi(2)) - radial_profile / r_disk;
    let d_phi_dr = a_0 * sech2_y * (d_rad_prof * cos_xi + radial_profile * sin_xi * (k / r_cyl));

    // (1 / R) * dPhi/dphi = - (m / R) * phi_amp * sin(xi)
    let inv_r_d_phi_dphi = -(m / r_cyl) * phi_amp * sin_xi;

    // dPhi/dy = - 2 / z_0 * tanh(y / z_0) * phi_amp * cos(xi)
    let d_phi_dy = -(2.0 / z_0) * y_scaled.tanh() * phi_amp * cos_xi;

    // a = -grad(Phi)
    let a_r = -d_phi_dr;
    let a_phi = -inv_r_d_phi_dphi;
    let a_y = -d_phi_dy;

    // Convert cylindrical (a_r, a_phi) to Cartesian (a_x, a_z) in the XZ galactic plane:
    let cos_phi = pos.x / r_cyl;
    let sin_phi = pos.z / r_cyl;

    let a_x = a_r * cos_phi - a_phi * sin_phi;
    let a_z = a_r * sin_phi + a_phi * cos_phi;

    DVec3::new(a_x, a_y, a_z)
}
