//! Symplectic N-Body Gravity and Aerodynamic Gas Drag Physics Engine.
//! This system handles ONLY ECS-promoted massive bodies (planets, protoplanets).
//! The 50k particle swarm physics is handled entirely on the GPU.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::*;

/// Advances the N-body gravitational physics simulation using a Symplectic Kick-Drift-Kick Leapfrog integrator.
pub fn step_physics_simulation(
    config: Res<SimulationConfig>,
    _disk_params: Res<DiskParameters>,
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
    let target_dt = dt * time_warp.multiplier.max(0.01);

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

    let (star_mass, star_pos, is_central_quasi) = if let Some(idx) = star_index {
        (
            body_data[idx].1,
            body_data[idx].2,
            body_data[idx].6 == BodyType::QuasiStar,
        )
    } else {
        (1.0, DVec3::ZERO, false)
    };

    // Identify if the active simulation is the JWST Little Red Dot (Quasi-Star with 450,000 M_sun)
    // where extreme supermassive gravity drives orbital velocities of several thousand km/s
    let is_little_red_dot = scenario_state
        .as_ref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::LittleRedDot)
        || is_central_quasi
        || star_mass > 10_000.0;

    // Adaptive substepping: For extreme mass systems (Little Red Dot), scale substep resolution
    // so high time warp multipliers never cause numerical leapfrog tangent blowout.
    let max_substeps = if is_little_red_dot {
        config.max_substeps_per_frame.max(128)
    } else {
        config.max_substeps_per_frame
    };
    let n_substeps = ((target_dt / dt).ceil() as usize).clamp(1, max_substeps);
    let sub_dt = target_dt / (n_substeps as f64);

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

        // --- 2. Symplectic Leapfrog Drift step (r += v * dt) ---
        // Pass A: Advance non-satellites (planets, embryos, asteroids, rogue interlopers)
        for body in body_data.iter_mut() {
            if body.7.is_none() {
                if !body.8 {
                    // True 3D kinematic motion: position advances along velocity vector
                    body.2 += body.3 * sub_dt;
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

        let compute_acc = |i: usize,
                           pos: &DVec3,
                           vel: &DVec3,
                           b_mass: f64,
                           _rad: f64,
                           b_type: BodyType|
         -> DVec3 {
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

            // Mutual N-body interactions with Adaptive Softening (Newton's Shell Theorem)
            for &(m_pos, m_mass, _m_rad, m_idx) in &massive_data {
                if m_idx == i {
                    continue;
                }
                let r_vec = *pos - m_pos;
                // Physical gravitational softening: outside bodies, gravity is exact 1/r^2.
                // Standard softening scale prevents division-by-zero singularities during physical mergers.
                let dist_sq = r_vec.length_squared() + softening_sq;
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

            // Acceleration Limiting: Protect against unphysical singularities during ultra-close contact (r -> 0)
            // For the Little Red Dot (Quasi-Star with 450,000 M_sun), orbital accelerations at 60 AU reach ~4,934 AU/yr^2.
            let acc_mag = acc.length();
            let max_acc = if is_little_red_dot {
                25_000_000.0
            } else {
                500_000.0
            };
            if acc_mag > max_acc {
                acc *= max_acc / acc_mag;
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
                    let safe_r = if is_little_red_dot { 120.0 } else { 1.0 };
                    let v_k = (G_ASTRO * star_mass / safe_r).sqrt();
                    body.2 = DVec3::new(safe_r, 0.0, 0.0);
                    body.3 = DVec3::new(0.0, 0.0, v_k);
                    body.4 = DVec3::ZERO;
                }

                // Universal Cosmic Speed Limit: The Speed of Light in vacuum (c = 299,792.458 km/s ~ 63,241 AU/yr)
                // As a fundamental rule of the universe, NO massive body or planet can ever exceed c.
                let speed = body.3.length();
                let universal_c_limit = SPEED_OF_LIGHT_AU_YR * 0.999; // ~63,177.8 AU/yr (299,492 km/s)
                if speed > universal_c_limit {
                    body.3 *= universal_c_limit / speed;
                } else if !is_little_red_dot {
                    // Standard planetary stellar systems (M < 100 M_sun): Bound planetary orbits have v < 250 AU/yr (~1,185 km/s)
                    let max_planetary_speed = 250.0;
                    if speed > max_planetary_speed {
                        body.3 *= max_planetary_speed / speed;
                    }
                } else {
                    // Little Red Dot / Supermassive Systems (M >= 100,000 M_sun):
                    // At 60 AU, Keplerian orbital speed is sqrt(G * 450,000 / 60) ~ 544 AU/yr (~2,580 km/s).
                    // Sub-relativistic bound limit prevents close-encounter numerical ejections while allowing full orbital speeds.
                    let max_smbh_bound_speed = 10_000.0; // ~47,404 km/s (~0.16 c)
                    if speed > max_smbh_bound_speed {
                        body.3 *= max_smbh_bound_speed / speed;
                    }
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
    let mut escaped_minor_debris: Vec<Entity> = Vec::new();

    for (e, mut m, mut pos, mut vel, mut acc, _, body, opt_sat, opt_central) in
        bodies_query.iter_mut()
    {
        if opt_central.is_some() {
            pos.0 = DVec3::ZERO;
            vel.0 = DVec3::ZERO;
            acc.0 = DVec3::ZERO;
        } else if let Some(idx) = body_data.iter().position(|b| b.0 == e) {
            let r_mag = body_data[idx].2.length();
            // Minor debris or rogue bodies that have escaped to deep interstellar space are retired.
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

            m.0 = body_data[idx].1;
            pos.0 = body_data[idx].2;
            vel.0 = body_data[idx].3;
            acc.0 = body_data[idx].4;
            if let (Some(mut s_comp), Some(s_data)) = (opt_sat, body_data[idx].7) {
                *s_comp = s_data;
            }
        }
    }

    // Despawn escaped minor debris
    for e in escaped_minor_debris {
        if let Ok(mut cmd) = commands.get_entity(e) {
            cmd.despawn();
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
