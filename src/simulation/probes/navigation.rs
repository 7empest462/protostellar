use bevy::math::{DQuat, DVec3};
use bevy::prelude::*;

use super::{FlightComputer, ProbeStatus, ProbeType, SpaceProbe};
use crate::game::ui::NotificationToast;
use crate::simulation::components::{CelestialBody, Radius, SimPosition, SimVelocity};
use crate::simulation::resources::{SimTime, SimulationConfig};

pub fn probe_navigation_system(
    mut probes: Query<(
        &mut SimVelocity,
        &mut SimPosition,
        &mut SpaceProbe,
        &mut FlightComputer,
    )>,
    targets: Query<
        (
            &SimPosition,
            &SimVelocity,
            Option<&Radius>,
            &CelestialBody,
            Option<&crate::simulation::components::celestial::SpinState>,
        ),
        Without<SpaceProbe>,
    >,
    sim_time: Res<SimTime>,

    time_warp: Res<crate::simulation::resources::TimeWarp>,
    config: Option<Res<SimulationConfig>>,
    mut toast: Option<ResMut<NotificationToast>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt = sim_time.current_dt_yr;
    if dt <= 0.0 {
        return;
    }

    for (mut probe_vel, mut probe_pos, mut probe, mut computer) in probes.iter_mut() {
        if let Ok((target_pos, target_vel, target_radius, target_body, opt_spin)) =
            targets.get(computer.target_entity)
        {
            let to_target = target_pos.0 - probe_pos.0;
            let distance = to_target.length();
            let radius = target_radius.map_or(0.001, |r| r.0);
            let vis_rad = config.as_ref().map_or(radius, |c| {
                f64::from(c.calc_visual_radius_for_type(radius, target_body.body_type))
            });

            computer.distance_to_target = distance;
            computer.relative_speed = (probe_vel.0 - target_vel.0).length();

            // Orbit/Landing distance threshold scaled with target's visual size
            let threshold = (vis_rad * 2.8).clamp(0.0035, 0.25);
            let mut arrived = distance <= threshold;

            if !arrived && probe.status == ProbeStatus::Transit {
                arrived = steer_probe_transit(
                    &mut probe_pos.0,
                    &mut probe_vel.0,
                    target_pos.0,
                    target_vel.0,
                    distance,
                    threshold,
                    dt,
                );
            } else {
                arrived = true;
            }

            if arrived {
                handle_probe_arrival(
                    &mut probe,
                    &computer,
                    &mut probe_pos,
                    &mut probe_vel,
                    target_pos.0,
                    target_vel.0,
                    vis_rad,
                    dt,
                    sim_time.elapsed_years,
                    opt_spin,
                    &mut toast,
                );
            }
            update_probe_orientation(
                &mut probe,
                probe_pos.0,
                probe_vel.0,
                target_pos.0,
                target_vel.0,
            );
        } else {
            // Target was destroyed or missing: probe coasts freely through space
            probe.status = ProbeStatus::Transit;
            probe_pos.0 += probe_vel.0 * dt;
        }
    }
}

fn steer_probe_transit(
    probe_pos: &mut DVec3,
    probe_vel: &mut DVec3,
    target_pos: DVec3,
    target_vel: DVec3,
    distance: f64,
    threshold: f64,
    dt: f64,
) -> bool {
    let to_target = target_pos - *probe_pos;
    let cruise_speed = (distance * 6.0).clamp(0.4, 5.0);

    let dir = if to_target.length_squared() > 1e-10 {
        to_target.normalize()
    } else {
        DVec3::X
    };

    let brake_zone = threshold * 3.5;
    let speed_multiplier = if distance < brake_zone {
        ((distance - threshold) / (brake_zone - threshold)).clamp(0.15, 1.0)
    } else {
        1.0
    };

    let desired_speed = cruise_speed * speed_multiplier;
    let desired_vel = target_vel + dir * desired_speed;

    let steer_factor = 1.0 - (-dt * 20.0).exp();
    *probe_vel = probe_vel.lerp(desired_vel, steer_factor);

    let old_pos = *probe_pos;
    *probe_pos += *probe_vel * dt;

    let new_to_target = target_pos - *probe_pos;
    if new_to_target.length() <= threshold {
        return true;
    }

    let rel_vel = *probe_vel - target_vel;
    let rel_ray = rel_vel * dt;
    let rel_ray_len = rel_ray.length();
    if rel_ray_len > 1e-10 {
        let rel_ray_dir = rel_ray / rel_ray_len;
        let old_target_pos = target_pos - target_vel * dt;
        let old_rel_pos = old_pos - old_target_pos;

        let t = -old_rel_pos.dot(rel_ray_dir);
        if t > 0.0 && t < rel_ray_len {
            let closest_rel = old_rel_pos + rel_ray_dir * t;
            if closest_rel.length() <= threshold {
                *probe_pos = target_pos + closest_rel;
                return true;
            }
        }
    }

    false
}

fn update_probe_orientation(
    probe: &mut SpaceProbe,
    probe_pos: DVec3,
    probe_vel: DVec3,
    target_pos: DVec3,
    target_vel: DVec3,
) {
    match probe.status {
        ProbeStatus::Transit => {
            if probe_vel.length_squared() > 1e-10 {
                let fwd = probe_vel.normalize();
                probe.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, fwd);
            }
        }
        ProbeStatus::Landed => {
            let up = (probe_pos - target_pos).normalize_or_zero();
            probe.orientation = DQuat::from_rotation_arc(DVec3::Y, up);
        }
        ProbeStatus::InOrbit => {
            let fwd = (probe_vel - target_vel).normalize_or_zero();
            let up = (probe_pos - target_pos).normalize_or_zero();
            let right = up.cross(fwd).normalize_or_zero();
            let actual_fwd = right.cross(up).normalize_or_zero();

            let rot_mat = bevy::math::DMat3::from_cols(-right, up, -actual_fwd);
            probe.orientation = DQuat::from_mat3(&rot_mat);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Arrival handler mutates probe state and handles orbital insertion mathematics"
)]
fn handle_probe_arrival(
    probe: &mut SpaceProbe,
    computer: &FlightComputer,
    probe_pos: &mut SimPosition,
    probe_vel: &mut SimVelocity,
    target_pos: DVec3,
    target_vel: DVec3,
    vis_rad: f64,
    dt: f64,
    elapsed_years: f64,
    opt_spin: Option<&crate::simulation::components::celestial::SpinState>,
    toast: &mut Option<ResMut<NotificationToast>>,
) {
    // Arrived at destination world!
    let just_arrived = probe.status == ProbeStatus::Transit;

    // Recompute to_target. If we were already tracking the target, use the target's old position
    // to prevent the probe from being dragged backwards along the target's velocity vector when dt is large.
    let to_target = if just_arrived {
        target_pos - probe_pos.0
    } else {
        let old_target_pos = target_pos - target_vel * dt;
        old_target_pos - probe_pos.0
    };

    match probe.probe_type {
        ProbeType::Orbiter => {
            probe.status = ProbeStatus::InOrbit;
            let target_orbit_distance = (vis_rad * 1.6).clamp(0.003, 0.20);
            let orbit_speed = 0.5; // Visual speed (AU/year roughly)
            let ang_speed = orbit_speed / target_orbit_distance;

            if just_arrived {
                let arrival_dir = if to_target.length_squared() > 1e-10 {
                    -to_target.normalize()
                } else {
                    DVec3::X
                };

                // Determine a great circle orbital plane based on incoming velocity
                // This prevents weird 'halo' orbits around the Y axis
                let mut orbit_axis = arrival_dir
                    .cross(probe_vel.0 - target_vel)
                    .normalize_or_zero();
                if orbit_axis.length_squared() < 0.5 {
                    orbit_axis = DVec3::Y;
                }

                probe.landed_unspun_pos = Some(arrival_dir);
                probe.orbit_axis = Some(orbit_axis);
                probe.arrival_time = Some(elapsed_years);
            }

            // Calculate exact deterministic orbital position to prevent floating point accumulation truncation
            let axis = probe.orbit_axis.unwrap_or(DVec3::Y);
            let base_dir = probe.landed_unspun_pos.unwrap_or(DVec3::X);
            let t_since_arrival = elapsed_years - probe.arrival_time.unwrap_or(elapsed_years);

            let current_phase = ang_speed * t_since_arrival;
            let rot = DQuat::from_axis_angle(axis, current_phase);
            let offset = rot * (base_dir * target_orbit_distance);

            probe_pos.0 = target_pos + offset;

            // Derive current velocity from the deterministic circle so orientation doesn't break
            let tangent = axis.cross(offset.normalize()).normalize_or_zero();
            probe_vel.0 = target_vel + tangent * orbit_speed;

            if just_arrived {
                if let Some(ref mut t) = toast {
                    t.message = format!(
                        "🛰️ Orbiter #{} has safely entered orbit around {}!",
                        probe.id, computer.target_name
                    );
                    t.timer = 5.0;
                }
                bevy::log::info!(
                    "🛰️ Orbiter #{} reached orbit around {}",
                    probe.id,
                    computer.target_name
                );
            }
        }
        ProbeType::Rover => {
            probe.status = ProbeStatus::Landed;

            let spin_angle = if let Some(spin) = opt_spin {
                let rotations_per_year = 8766.0 / spin.rotation_period_hours.max(0.01);
                (elapsed_years * rotations_per_year * std::f64::consts::TAU) % std::f64::consts::TAU
            } else {
                (elapsed_years * 100.0 * std::f64::consts::TAU) % std::f64::consts::TAU
            };

            let spin_dir = if let Some(spin) = opt_spin {
                if spin.spin_vector.length_squared() > 1e-12
                    && (spin.spin_vector.x.abs() > 1e-6 || spin.spin_vector.z.abs() > 1e-6)
                {
                    spin.spin_vector.normalize()
                } else {
                    let tilt_rad = spin.axial_tilt_degrees.to_radians();
                    DVec3::new(tilt_rad.sin(), tilt_rad.cos(), 0.0)
                }
            } else {
                DVec3::new(0.08_f64.sin(), 0.08_f64.cos(), 0.0)
            };

            if just_arrived {
                let arrival_dir = if to_target.length_squared() > 1e-10 {
                    -to_target.normalize()
                } else {
                    DVec3::Y
                };
                // Store the base surface normal relative to the un-spun planet frame
                let unspun = DQuat::from_axis_angle(spin_dir, -spin_angle) * arrival_dir;
                probe.landed_unspun_pos = Some(unspun);
            }

            // Deterministically rotate the rover every frame using the absolute physical time!
            // This prevents floating-point accumulation truncation when dt is very small.
            let unspun_pos = probe.landed_unspun_pos.unwrap_or(DVec3::Y);
            let rotated_dir = DQuat::from_axis_angle(spin_dir, spin_angle) * unspun_pos;

            // Anchor firmly to the world's visible surface
            probe_pos.0 = target_pos + rotated_dir * (vis_rad * 1.015);
            probe_vel.0 = target_vel;

            if just_arrived {
                if let Some(ref mut t) = toast {
                    t.message = format!(
                        "🛬 Rover #{} touched down on the surface of {}!",
                        probe.id, computer.target_name
                    );
                    t.timer = 5.0;
                }
                bevy::log::info!(
                    "🛬 Rover #{} touched down on {}",
                    probe.id,
                    computer.target_name
                );
            }
        }
    }
}
