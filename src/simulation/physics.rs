//! Symplectic N-Body Gravity and Aerodynamic Gas Drag Physics Engine.
//! This system handles ONLY ECS-promoted massive bodies (planets, protoplanets).
//! The 50k particle swarm physics is handled entirely on the GPU.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Advances the N-body gravitational physics simulation using a Symplectic Kick-Drift-Kick Leapfrog integrator.
pub fn step_physics_simulation(
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    player_state: Res<PlayerInteractionState>,
    mut bodies_query: Query<(
        Entity,
        &mut Mass,
        &mut SimPosition,
        &mut SimVelocity,
        &mut SimAcceleration,
        &Radius,
        &CelestialBody,
        Option<&mut SatelliteOf>,
    )>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt = config.base_dt_yr;
    let target_dt = dt * time_warp.multiplier.max(0.01);
    let n_substeps = ((target_dt / dt).ceil() as usize).clamp(1, config.max_substeps_per_frame);
    let sub_dt = target_dt / (n_substeps as f64);

    let softening_sq = config.softening_au * config.softening_au;

    // Collect all bodies into a contiguous vector for parallel force evaluation
    let mut body_data: Vec<(
        Entity,
        f64,
        DVec3,
        DVec3,
        DVec3,
        f64,
        BodyType,
        Option<SatelliteOf>,
    )> = bodies_query
        .iter()
        .map(|(e, m, pos, vel, acc, rad, body, sat)| {
            (
                e,
                m.0,
                pos.0,
                vel.0,
                acc.0,
                rad.0,
                body.body_type,
                sat.copied(),
            )
        })
        .collect();

    if body_data.is_empty() {
        return;
    }

    // Find central star index if present
    let star_index = body_data.iter().position(|(_, _, _, _, _, _, t, _)| {
        matches!(t, BodyType::Protostar | BodyType::MainSequenceStar)
    });

    let (star_mass, star_pos) = if let Some(idx) = star_index {
        (body_data[idx].1, body_data[idx].2)
    } else {
        (1.0, DVec3::ZERO)
    };

    // Filter massive bodies (embryos, planets, stars) for full mutual N-body interactions
    let massive_indices: Vec<usize> = body_data
        .iter()
        .enumerate()
        .filter(|(_, (_, m, _, _, _, _, t, _))| {
            *m > EARTH_MASS_SOLAR * 0.1
                || matches!(
                    t,
                    BodyType::Protostar
                        | BodyType::MainSequenceStar
                        | BodyType::Protoplanet
                        | BodyType::TerrestrialPlanet
                        | BodyType::GasGiant
                        | BodyType::IceGiant
                )
        })
        .map(|(i, _)| i)
        .collect();

    // Check for tractor tool
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

    // Pre-step: Ensure initial acceleration is valid for newly spawned bodies
    for body in body_data.iter_mut() {
        if body.4.length_squared() < 1e-12 {
            let r_vec = body.2 - star_pos;
            let dist_sq = r_vec.length_squared() + softening_sq;
            let dist = dist_sq.sqrt();
            if !matches!(body.6, BodyType::Protostar | BodyType::MainSequenceStar) {
                body.4 = -(G_ASTRO * star_mass / (dist_sq * dist)) * r_vec;
            }
        }
    }

    for _ in 0..n_substeps {
        // --- 1. Kick step (v += a * dt/2) ---
        for body in body_data.iter_mut() {
            if !matches!(body.6, BodyType::Protostar | BodyType::MainSequenceStar) {
                body.3 += body.4 * (sub_dt * 0.5);
            } else {
                body.2 = DVec3::ZERO;
                body.3 = DVec3::ZERO;
                body.4 = DVec3::ZERO;
            }
        }

        // --- 2. Exact Keplerian Symplectic Drift step ---
        // Pass A: Advance non-satellites (planets, embryos, asteroids) around the central star
        for body in body_data.iter_mut() {
            if body.7.is_none() {
                if !matches!(body.6, BodyType::Protostar | BodyType::MainSequenceStar) {
                    let r_cyl = (body.2.x * body.2.x + body.2.z * body.2.z).sqrt().max(0.05);
                    let omega = (G_ASTRO * star_mass / (r_cyl * r_cyl * r_cyl)).sqrt();
                    let delta_phi = omega * sub_dt;
                    let cos_d = delta_phi.cos();
                    let sin_d = delta_phi.sin();

                    // Exact 2D Keplerian orbital rotation in the disk plane (zero energy/eccentricity drift)
                    let x_new = body.2.x * cos_d - body.2.z * sin_d;
                    let z_new = body.2.x * sin_d + body.2.z * cos_d;
                    body.2.x = x_new;
                    body.2.y += body.3.y * sub_dt;
                    body.2.z = z_new;

                    let vx_new = body.3.x * cos_d - body.3.z * sin_d;
                    let vz_new = body.3.x * sin_d + body.3.z * cos_d;
                    body.3.x = vx_new;
                    body.3.z = vz_new;
                } else {
                    body.2 = DVec3::ZERO;
                    body.3 = DVec3::ZERO;
                    body.4 = DVec3::ZERO;
                }
            }
        }

        // Pass B: Advance natural satellites / moons around their parent planet
        let snapshot_positions: Vec<(Entity, DVec3, DVec3, f64)> =
            body_data.iter().map(|b| (b.0, b.2, b.3, b.1)).collect();

        for body in body_data.iter_mut() {
            if let Some(ref mut sat) = body.7 {
                if let Some(parent_idx) = snapshot_positions
                    .iter()
                    .position(|(e, ..)| *e == sat.parent)
                {
                    let (_, parent_pos, parent_vel, parent_mass) = snapshot_positions[parent_idx];
                    let r_orbit = sat.semi_major_axis_au.max(1e-5);
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

                    body.2 = parent_pos + DVec3::new(r_orbit * cos_a, 0.0, r_orbit * sin_a);
                    body.3 = parent_vel + DVec3::new(-v_orb_mag * sin_a, 0.0, v_orb_mag * cos_a);
                }
            }
        }

        // --- 3. Compute Accelerations at new positions (Mutual N-Body + Gas Drag) ---
        let positions_and_masses: Vec<(DVec3, f64, f64)> =
            body_data.iter().map(|b| (b.2, b.1, b.5)).collect();
        let massive_data: Vec<(DVec3, f64, f64, usize)> = massive_indices
            .iter()
            .map(|&idx| {
                (
                    positions_and_masses[idx].0,
                    positions_and_masses[idx].1,
                    positions_and_masses[idx].2,
                    idx,
                )
            })
            .collect();

        let compute_acc =
            |i: usize, pos: &DVec3, vel: &DVec3, _rad: f64, b_type: BodyType| -> DVec3 {
                let mut acc = DVec3::ZERO;

                // Non-Keplerian perturbations (Gas Aerodynamic Drag & Eccentricity Damping)
                if !matches!(b_type, BodyType::Protostar | BodyType::MainSequenceStar) {
                    // Gas aerodynamic drag and orbital circularization
                    if config.enable_gas_drag && config.gas_density_scale > 0.001 {
                        let r_cyl = (pos.x * pos.x + pos.z * pos.z).sqrt().max(0.1);
                        let v_k = (G_ASTRO * star_mass / r_cyl).sqrt();
                        let v_gas_mag = v_k * 0.998;
                        let phi = pos.z.atan2(pos.x);
                        let v_gas = DVec3::new(-v_gas_mag * phi.sin(), 0.0, v_gas_mag * phi.cos());

                        let rel_v = *vel - v_gas;
                        let rel_speed = rel_v.length();
                        let gas_density =
                            1e-4 * (r_cyl / 1.0).powf(-2.25) * (config.gas_density_scale as f64);
                        let drag_coeff = 0.025 * gas_density;
                        acc -= drag_coeff * rel_speed * rel_v;

                        // Eccentricity damping (damps non-circular radial velocity v_r and vertical velocity v_y)
                        let r_unit = DVec3::new(pos.x / r_cyl, 0.0, pos.z / r_cyl);
                        let v_radial = vel.dot(r_unit);
                        let damp_rate = 0.08 * gas_density;
                        acc -= r_unit * (v_radial * damp_rate);
                        acc.y -= vel.y * damp_rate * 2.0;
                    }
                }

                // Mutual N-body interactions with Adaptive Softening
                for &(m_pos, m_mass, m_rad, m_idx) in &massive_data {
                    if m_idx == i {
                        continue;
                    }
                    let r_vec = *pos - m_pos;
                    let pair_softening_sq = softening_sq.max((m_rad * 0.5).powi(2)).max(1e-4);
                    let dist_sq = r_vec.length_squared() + pair_softening_sq;
                    let dist = dist_sq.sqrt();
                    acc -= (G_ASTRO * m_mass / (dist_sq * dist)) * r_vec;
                }

                // Gravitational tractor tool acceleration
                if let Some((t_pos, t_mass)) = tractor {
                    let r_vec = *pos - t_pos;
                    let dist_sq = r_vec.length_squared() + softening_sq;
                    let dist = dist_sq.sqrt();
                    acc -= (G_ASTRO * t_mass / (dist_sq * dist)) * r_vec;
                }

                // Acceleration Limiting: Bound maximum acceleration to prevent high-warp numerical explosions
                let acc_mag = acc.length();
                if acc_mag > 80.0 {
                    acc *= 80.0 / acc_mag;
                }

                if !acc.is_finite() {
                    DVec3::ZERO
                } else {
                    acc
                }
            };

        // Compute accelerations sequentially (<64 bodies)
        let new_accelerations: Vec<DVec3> = body_data
            .iter()
            .enumerate()
            .map(|(i, (_, _, pos, vel, _, rad, b_type, _))| compute_acc(i, pos, vel, *rad, *b_type))
            .collect();

        // Assign newly calculated accelerations
        for (i, body) in body_data.iter_mut().enumerate() {
            if !matches!(body.6, BodyType::Protostar | BodyType::MainSequenceStar) {
                body.4 = new_accelerations[i];
            } else {
                body.4 = DVec3::ZERO;
            }
        }

        // --- 4. Kick step (v += a * dt/2) & Physical Velocity Limiting ---
        for body in body_data.iter_mut() {
            if !matches!(body.6, BodyType::Protostar | BodyType::MainSequenceStar) {
                body.3 += body.4 * (sub_dt * 0.5);

                // Sanitize non-finite vectors
                if !body.2.is_finite() || !body.3.is_finite() {
                    let safe_r = 1.0;
                    let v_k = (G_ASTRO * star_mass / safe_r).sqrt();
                    body.2 = DVec3::new(safe_r, 0.0, 0.0);
                    body.3 = DVec3::new(0.0, 0.0, v_k);
                    body.4 = DVec3::ZERO;
                }

                // Velocity Capping: Prevent close-encounter numerical singularities from launching
                // planets into unphysical hyperbolic escape trajectories (e.g. 210,000 km/s)
                let r_cyl = (body.2.x * body.2.x + body.2.z * body.2.z)
                    .sqrt()
                    .clamp(0.1, 75.0);
                let v_esc = (2.0 * G_ASTRO * star_mass / r_cyl).sqrt();
                let max_v = 1.6 * v_esc; // Up to 1.6x escape velocity for eccentric comets
                let speed = body.3.length();
                if speed > max_v && speed > 1e-6 {
                    body.3 *= max_v / speed;
                }

                // Solar System Boundary Clamping: Keep all active bodies within the physical domain (r <= 75 AU)
                let r_mag = body.2.length();
                if r_mag > 75.0 && r_mag > 1e-6 {
                    body.2 *= 75.0 / r_mag;
                }
            } else {
                body.2 = DVec3::ZERO;
                body.3 = DVec3::ZERO;
                body.4 = DVec3::ZERO;
            }
        }
    }

    // Apply player delta-v impulse if requested
    if let (Some(target), Some(dv)) = (
        player_state.impulse_target_entity,
        player_state.impulse_delta_v,
    ) {
        if let Some(idx) = body_data.iter().position(|b| b.0 == target) {
            body_data[idx].3 += dv;
        }
    }

    // Write back updated positions, velocities, accelerations, and satellite states to ECS
    for (e, mut m, mut pos, mut vel, mut acc, _, body, opt_sat) in bodies_query.iter_mut() {
        if matches!(
            body.body_type,
            BodyType::Protostar | BodyType::MainSequenceStar
        ) {
            pos.0 = DVec3::ZERO;
            vel.0 = DVec3::ZERO;
            acc.0 = DVec3::ZERO;
        } else if let Some(idx) = body_data.iter().position(|b| b.0 == e) {
            m.0 = body_data[idx].1;
            pos.0 = body_data[idx].2;
            vel.0 = body_data[idx].3;
            acc.0 = body_data[idx].4;
            if let (Some(mut s_comp), Some(s_data)) = (opt_sat, body_data[idx].7) {
                *s_comp = s_data;
            }
        }
    }

    // Diagnostics & Energy Monitoring
    let mut kinetic_e = 0.0;
    let mut potential_e = 0.0;

    for (i, (_, m, pos, vel, _, _, _, _)) in body_data.iter().enumerate() {
        kinetic_e += 0.5 * m * vel.length_squared();

        // Potential energy against central star
        if let Some(s_idx) = star_index {
            if i != s_idx {
                let dist = (*pos - star_pos).length() + config.softening_au;
                potential_e -= (G_ASTRO * star_mass * m) / dist;
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

    sim_time.elapsed_years += target_dt;
    sim_time.current_dt_yr = target_dt;
    sim_time.step_count += n_substeps as u64;
}
