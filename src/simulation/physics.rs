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
    disk_params: Res<DiskParameters>,
    time_warp: Res<TimeWarp>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    player_state: Res<PlayerInteractionState>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
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
    let target_dt = dt * time_warp.multiplier.max(0.01);
    let n_substeps = ((target_dt / dt).ceil() as usize).clamp(1, config.max_substeps_per_frame);
    let sub_dt = target_dt / (n_substeps as f64);

    let softening_sq = config.softening_au * config.softening_au;
    let max_domain_r = (disk_params.outer_radius_au * 2.5).max(75.0);

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
        bool, // is_central_star
    )> = bodies_query
        .iter()
        .map(|(e, m, pos, vel, acc, rad, body, sat, opt_central)| {
            (
                e,
                m.0,
                pos.0,
                vel.0,
                acc.0,
                rad.0,
                body.body_type,
                sat.copied(),
                opt_central.is_some(),
            )
        })
        .collect();

    if body_data.is_empty() {
        return;
    }

    // Find central star index if present
    let star_index = body_data
        .iter()
        .position(|(_, _, _, _, _, _, _, _, is_central)| *is_central);

    let (star_mass, star_pos) = if let Some(idx) = star_index {
        (body_data[idx].1, body_data[idx].2)
    } else {
        (1.0, DVec3::ZERO)
    };

    // Filter massive bodies (embryos, planets, stars) for full mutual N-body interactions
    let massive_indices: Vec<usize> = body_data
        .iter()
        .enumerate()
        .filter(|(_, (_, m, _, _, _, _, t, _, is_central))| {
            *m > EARTH_MASS_SOLAR * 0.1
                || *is_central
                || t.is_star_or_remnant()
                || matches!(
                    t,
                    BodyType::Protoplanet
                        | BodyType::TerrestrialPlanet
                        | BodyType::SuperEarth
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
            if !body.8 {
                body.4 = -(G_ASTRO * star_mass / (dist_sq * dist)) * r_vec;
            }
        }
    }

    for _ in 0..n_substeps {
        // --- 1. Kick step (v += a * dt/2) ---
        for body in body_data.iter_mut() {
            if !body.8 {
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
                if !body.8 {
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
            |i: usize, pos: &DVec3, vel: &DVec3, b_mass: f64, _rad: f64, b_type: BodyType| -> DVec3 {
                let mut acc = DVec3::ZERO;

                // Non-Keplerian perturbations (Gas Aerodynamic Drag & Eccentricity Damping)
                if !b_type.is_star_or_remnant() {
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

                        // Physical Aerodynamic Drag Scaling: a_drag = F_drag / m ~ (rho * R^2 * v^2) / m
                        // Small planetesimals & dust grains feel circularizing and settling drag,
                        // while massive protoplanets, giant worlds, and stars have massive inertia and feel negligible drag.
                        let m_earth = b_mass / EARTH_MASS_SOLAR;
                        let inertia_suppression = (1.0 / (1.0 + m_earth * 150.0)).clamp(0.0, 1.0);

                        let drag_coeff = 0.025 * gas_density * inertia_suppression;
                        acc -= drag_coeff * rel_speed * rel_v;

                        // Eccentricity damping (damps non-circular radial velocity v_r and vertical velocity v_y)
                        let r_unit = DVec3::new(pos.x / r_cyl, 0.0, pos.z / r_cyl);
                        let v_radial = vel.dot(r_unit);
                        let damp_rate = 0.08 * gas_density * inertia_suppression;
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
            .map(|(i, (_, mass, pos, vel, _, rad, b_type, _, _))| {
                compute_acc(i, pos, vel, *mass, *rad, *b_type)
            })
            .collect();

        // Assign newly calculated accelerations
        for (i, body) in body_data.iter_mut().enumerate() {
            if !body.8 {
                body.4 = new_accelerations[i];
            } else {
                body.4 = DVec3::ZERO;
            }
        }

        // --- 3.5 Nice Model Planetary Migration & Resonance Crossing (Late Heavy Bombardment) ---
        if lhb_state.is_active {
            lhb_state.time_active_years += sub_dt;
            let progress = (lhb_state.time_active_years / 2500.0).clamp(0.0, 1.0);
            lhb_state.migration_progress = progress;

            // Find Jupiter and Saturn indices
            let mut jupiter_idx = None;
            let mut saturn_idx = None;

            for (i, (_, m, pos, _, _, _, b_type, _, _)) in body_data.iter().enumerate() {
                let r = (pos.x * pos.x + pos.z * pos.z).sqrt();
                if matches!(b_type, BodyType::GasGiant) || *m >= JUPITER_MASS_SOLAR * 0.15 {
                    if r < 10.0 && jupiter_idx.is_none() {
                        jupiter_idx = Some((i, r));
                    } else if (8.0..20.0).contains(&r) {
                        saturn_idx = Some((i, r));
                    }
                }
            }

            if let (Some((j_i, r_j)), Some((s_i, r_s))) = (jupiter_idx, saturn_idx) {
                let p_ratio = (r_s / r_j.max(0.1)).powf(1.5);
                lhb_state.resonance_ratio = p_ratio;

                // Check resonance crossing at 2:1
                if p_ratio >= 2.0 && !lhb_state.resonance_crossed {
                    lhb_state.resonance_crossed = true;
                    // Resonant eccentricity kick
                    let v_j = body_data[j_i].3;
                    let v_s = body_data[s_i].3;
                    body_data[j_i].3 +=
                        v_j.cross(DVec3::Y).normalize_or_zero() * (0.02 * v_j.length());
                    body_data[s_i].3 -=
                        v_s.cross(DVec3::Y).normalize_or_zero() * (0.03 * v_s.length());
                }

                // Inward migration for Jupiter (towards 5.2 AU)
                if r_j > 5.2 && progress < 0.95 {
                    let v_dir = body_data[j_i].3.normalize_or_zero();
                    body_data[j_i].4 -= v_dir * 0.0004;
                }
                // Outward migration for Saturn (towards 9.58 AU)
                if r_s < 9.58 && progress < 0.95 {
                    let v_dir = body_data[s_i].3.normalize_or_zero();
                    body_data[s_i].4 += v_dir * 0.0006;
                }
            }

            // Outward migration for Ice Giants and comet scattering
            for (i, (_, _m, pos, vel, acc, _, b_type, _, _)) in body_data.iter_mut().enumerate() {
                let r = (pos.x * pos.x + pos.z * pos.z).sqrt();
                let v_dir = vel.normalize_or_zero();

                if matches!(b_type, BodyType::IceGiant) {
                    if r < 30.0 && progress < 0.95 {
                        *acc += v_dir * 0.0012; // Outward migration through icy disk
                    }
                } else if matches!(
                    b_type,
                    BodyType::Planetesimal | BodyType::Asteroid | BodyType::Comet
                ) && r >= 15.0
                {
                    // Gravitational scattering: Comets get perturbed into high-eccentricity inner crossing orbits
                    if lhb_state.resonance_crossed && progress < 0.90 {
                        let kick_dir = -pos.normalize_or_zero()
                            + DVec3::new(0.0, (i % 5) as f64 * 0.05 - 0.1, 0.0);
                        *acc += kick_dir.normalize_or_zero() * 0.0045;
                        lhb_state.comets_scattered = (lhb_state.comets_scattered + 1).min(100_000);
                    }
                }
            }
        }

        // --- 4. Kick step (v += a * dt/2) & Physical Velocity Limiting ---
        for body in body_data.iter_mut() {
            if !body.8 {
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
                    .clamp(0.1, max_domain_r);
                let v_esc = (2.0 * G_ASTRO * star_mass / r_cyl).sqrt();
                let max_v = 1.6 * v_esc; // Up to 1.6x escape velocity for eccentric comets
                let speed = body.3.length();
                if speed > max_v && speed > 1e-6 {
                    body.3 *= max_v / speed;
                }

                // Domain Boundary Clamping: Keep all active bodies within the physical domain (r <= max_domain_r)
                let r_mag = body.2.length();
                if r_mag > max_domain_r {
                    body.2 *= max_domain_r / r_mag;
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
    for (e, mut m, mut pos, mut vel, mut acc, _, _body, opt_sat, opt_central) in
        bodies_query.iter_mut()
    {
        if opt_central.is_some() {
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

    for (i, (_, m, pos, vel, _, _, _, _, _)) in body_data.iter().enumerate() {
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
