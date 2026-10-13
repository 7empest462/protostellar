use bevy::math::{DQuat, DVec3};
use bevy::prelude::*;

use super::{FlightComputer, ProbeStatus, ProbeType, SpaceProbe};
use crate::game::ui::NotificationToast;
use crate::simulation::components::celestial::SpinState;
use crate::simulation::components::{
    BiosphereState, CelestialBody, Composition, PlanetaryClimate, Radius, SimPosition, SimVelocity,
};
use crate::simulation::resources::{SimTime, SimulationConfig};
use crate::utils::constants::AU_TO_KM;

type NavigationTargetData<'a> = (
    &'a SimPosition,
    &'a SimVelocity,
    Option<&'a Radius>,
    &'a CelestialBody,
    Option<&'a SpinState>,
    Option<&'a Composition>,
    Option<&'a PlanetaryClimate>,
    Option<&'a mut BiosphereState>,
);

pub fn probe_navigation_system(
    mut probes: Query<(
        &mut SimVelocity,
        &mut SimPosition,
        &mut SpaceProbe,
        &mut FlightComputer,
    )>,
    mut targets: Query<NavigationTargetData, Without<SpaceProbe>>,
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
        if let Ok((
            target_pos,
            target_vel,
            target_radius,
            target_body,
            opt_spin,
            opt_comp,
            opt_climate,
            mut opt_bio,
        )) = targets.get_mut(computer.target_entity)
        {
            let to_target = target_pos.0 - probe_pos.0;
            let distance = to_target.length();
            let radius = target_radius.map_or(0.001, |r| r.0);
            let vis_rad = config.as_ref().map_or(radius, |c| {
                f64::from(c.calc_visual_radius_for_type(radius, target_body.body_type))
            });

            computer.distance_to_target = distance;
            computer.relative_speed = (probe_vel.0 - target_vel.0).length();

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
                    radius,
                    vis_rad,
                    dt,
                    sim_time.elapsed_years,
                    opt_spin,
                    opt_comp,
                    opt_climate,
                    &mut opt_bio,
                    &mut toast,
                );
            }
            update_probe_orientation(
                &mut probe,
                probe_pos.0,
                probe_vel.0,
                target_pos.0,
                target_vel.0,
                opt_spin,
                sim_time.elapsed_years,
            );
        } else {
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

pub fn compute_planet_spin_frame(opt_spin: Option<&SpinState>, elapsed_years: f64) -> (DVec3, f64) {
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

    (spin_dir, spin_angle)
}

fn update_probe_orientation(
    probe: &mut SpaceProbe,
    probe_pos: DVec3,
    probe_vel: DVec3,
    target_pos: DVec3,
    target_vel: DVec3,
    opt_spin: Option<&SpinState>,
    elapsed_years: f64,
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
            if let Some(heading_unspun) = probe.rover_heading_unspun {
                let (spin_dir, spin_angle) = compute_planet_spin_frame(opt_spin, elapsed_years);
                let world_heading = (DQuat::from_axis_angle(spin_dir, spin_angle) * heading_unspun)
                    .normalize_or_zero();
                let right = world_heading.cross(up).normalize_or_zero();
                if right.length_squared() > 0.5 {
                    let fwd = up.cross(right).normalize_or_zero();
                    let rot_mat = bevy::math::DMat3::from_cols(right, up, -fwd);
                    probe.orientation = DQuat::from_mat3(&rot_mat);
                    return;
                }
            }
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
    reason = "Arrival dispatcher routes to orbiter or rover surface navigation helpers"
)]
fn handle_probe_arrival(
    probe: &mut SpaceProbe,
    computer: &FlightComputer,
    probe_pos: &mut SimPosition,
    probe_vel: &mut SimVelocity,
    target_pos: DVec3,
    target_vel: DVec3,
    physical_rad_au: f64,
    vis_rad: f64,
    dt: f64,
    elapsed_years: f64,
    opt_spin: Option<&SpinState>,
    opt_comp: Option<&Composition>,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: &mut Option<Mut<'_, BiosphereState>>,
    toast: &mut Option<ResMut<NotificationToast>>,
) {
    let just_arrived = probe.status == ProbeStatus::Transit;
    let to_target = if just_arrived {
        target_pos - probe_pos.0
    } else {
        let old_target_pos = target_pos - target_vel * dt;
        old_target_pos - probe_pos.0
    };

    match probe.probe_type {
        ProbeType::Orbiter => handle_orbiter_arrival(
            probe,
            computer,
            probe_pos,
            probe_vel,
            target_pos,
            target_vel,
            to_target,
            vis_rad,
            elapsed_years,
            just_arrived,
            opt_comp,
            opt_climate,
            opt_bio,
            toast,
        ),
        ProbeType::Rover => handle_rover_arrival_and_traverse(
            probe,
            computer,
            probe_pos,
            probe_vel,
            target_pos,
            target_vel,
            to_target,
            physical_rad_au,
            vis_rad,
            dt,
            elapsed_years,
            just_arrived,
            opt_spin,
            opt_comp,
            opt_climate,
            opt_bio,
            toast,
        ),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Orbital insertion and remote-sensing spectrometry helper"
)]
fn handle_orbiter_arrival(
    probe: &mut SpaceProbe,
    computer: &FlightComputer,
    probe_pos: &mut SimPosition,
    probe_vel: &mut SimVelocity,
    target_pos: DVec3,
    target_vel: DVec3,
    to_target: DVec3,
    vis_rad: f64,
    elapsed_years: f64,
    just_arrived: bool,
    opt_comp: Option<&Composition>,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: &mut Option<Mut<'_, BiosphereState>>,
    toast: &mut Option<ResMut<NotificationToast>>,
) {
    probe.status = ProbeStatus::InOrbit;
    let target_orbit_distance = (vis_rad * 1.6).clamp(0.003, 0.20);
    let orbit_speed = 0.5;
    let ang_speed = orbit_speed / target_orbit_distance;

    if just_arrived {
        let arrival_dir = if to_target.length_squared() > 1e-10 {
            -to_target.normalize()
        } else {
            DVec3::X
        };
        let mut orbit_axis = arrival_dir
            .cross(probe_vel.0 - target_vel)
            .normalize_or_zero();
        if orbit_axis.length_squared() < 0.5 {
            orbit_axis = DVec3::Y;
        }

        probe.landed_unspun_pos = Some(arrival_dir);
        probe.orbit_axis = Some(orbit_axis);
        probe.arrival_time = Some(elapsed_years);
        probe.samples_collected = 1;
        if let Some(ref mut bio) = opt_bio {
            bio.surface_samples_collected = bio.surface_samples_collected.saturating_add(1);
        }
        probe.latest_assay = synthesize_probe_assay(
            ProbeType::Orbiter,
            probe.samples_collected,
            opt_comp,
            opt_climate,
            opt_bio.as_deref(),
        );
    }

    let axis = probe.orbit_axis.unwrap_or(DVec3::Y);
    let base_dir = probe.landed_unspun_pos.unwrap_or(DVec3::X);
    let t_since_arrival = elapsed_years - probe.arrival_time.unwrap_or(elapsed_years);

    let current_phase = ang_speed * t_since_arrival;
    let rot = DQuat::from_axis_angle(axis, current_phase);
    let offset = rot * (base_dir * target_orbit_distance);

    probe_pos.0 = target_pos + offset;
    let tangent = axis.cross(offset.normalize()).normalize_or_zero();
    probe_vel.0 = target_vel + tangent * orbit_speed;

    let completed_orbits = ((current_phase / std::f64::consts::TAU).floor() as u32).clamp(1, 500);
    if completed_orbits > probe.samples_collected {
        probe.samples_collected = completed_orbits;
        if let Some(ref mut bio) = opt_bio {
            bio.surface_samples_collected = bio.surface_samples_collected.saturating_add(1);
        }
        probe.latest_assay = synthesize_probe_assay(
            ProbeType::Orbiter,
            probe.samples_collected,
            opt_comp,
            opt_climate,
            opt_bio.as_deref(),
        );
    }

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

#[allow(
    clippy::too_many_arguments,
    reason = "Rover touchdown, geodesic traversal, and regolith core-drilling helper"
)]
fn handle_rover_arrival_and_traverse(
    probe: &mut SpaceProbe,
    computer: &FlightComputer,
    probe_pos: &mut SimPosition,
    probe_vel: &mut SimVelocity,
    target_pos: DVec3,
    target_vel: DVec3,
    to_target: DVec3,
    physical_rad_au: f64,
    vis_rad: f64,
    dt: f64,
    elapsed_years: f64,
    just_arrived: bool,
    opt_spin: Option<&SpinState>,
    opt_comp: Option<&Composition>,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: &mut Option<Mut<'_, BiosphereState>>,
    toast: &mut Option<ResMut<NotificationToast>>,
) {
    probe.status = ProbeStatus::Landed;
    let (spin_dir, spin_angle) = compute_planet_spin_frame(opt_spin, elapsed_years);

    if just_arrived {
        let arrival_dir = if to_target.length_squared() > 1e-10 {
            -to_target.normalize()
        } else {
            DVec3::Y
        };
        let unspun =
            (DQuat::from_axis_angle(spin_dir, -spin_angle) * arrival_dir).normalize_or_zero();
        let ref_axis = if unspun.y.abs() < 0.9 {
            DVec3::Y
        } else {
            DVec3::X
        };
        let base_tangent = unspun.cross(ref_axis).normalize_or_zero();
        let azimuth_rot = DQuat::from_axis_angle(unspun, (probe.id as f64) * 1.15);
        let initial_heading = (azimuth_rot * base_tangent).normalize_or_zero();

        probe.landed_unspun_pos = Some(unspun);
        probe.rover_heading_unspun = Some(initial_heading);
        probe.arrival_time = Some(elapsed_years);
        probe.traverse_trail_unspun.clear();
        probe.traverse_trail_unspun.push(unspun);
        probe.samples_collected = 1;
        if let Some(ref mut bio) = opt_bio {
            bio.surface_samples_collected = bio.surface_samples_collected.saturating_add(1);
        }
        probe.latest_assay = synthesize_probe_assay(
            ProbeType::Rover,
            probe.samples_collected,
            opt_comp,
            opt_climate,
            opt_bio.as_deref(),
        );
    } else {
        advance_rover_geodesic_step(
            probe,
            physical_rad_au,
            dt,
            elapsed_years,
            opt_comp,
            opt_climate,
            opt_bio,
        );
    }

    let unspun_pos = probe.landed_unspun_pos.unwrap_or(DVec3::Y);
    let rotated_dir = DQuat::from_axis_angle(spin_dir, spin_angle) * unspun_pos;

    probe_pos.0 = target_pos + rotated_dir * (vis_rad * 1.015);
    probe_vel.0 = target_vel;

    if just_arrived {
        if let Some(ref mut t) = toast {
            t.message = format!(
                "🛬 Rover #{} touched down on {}! Autonomous surface roving & core drilling active.",
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

fn advance_rover_geodesic_step(
    probe: &mut SpaceProbe,
    physical_rad_au: f64,
    dt: f64,
    elapsed_years: f64,
    opt_comp: Option<&Composition>,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: &mut Option<Mut<'_, BiosphereState>>,
) {
    let unspun_pos = probe
        .landed_unspun_pos
        .unwrap_or(DVec3::Y)
        .normalize_or_zero();
    let ref_axis = if unspun_pos.y.abs() < 0.9 {
        DVec3::Y
    } else {
        DVec3::X
    };
    let mut heading = probe
        .rover_heading_unspun
        .unwrap_or_else(|| unspun_pos.cross(ref_axis).normalize_or_zero());

    // Gentle terrain-avoidance steering + bounded geodesic angular step per frame
    let d_theta = (dt * 1.8).clamp(0.0, 0.032);
    if d_theta <= 1e-8 {
        return;
    }

    let steer_yaw = (elapsed_years * 3.7 + probe.id as f64 * 1.3).sin() * d_theta * 0.45;
    heading = (DQuat::from_axis_angle(unspun_pos, steer_yaw) * heading)
        .reject_from(unspun_pos)
        .normalize_or_zero();

    let great_circle_axis = unspun_pos.cross(heading).normalize_or_zero();
    if great_circle_axis.length_squared() > 0.5 {
        let step_rot = DQuat::from_axis_angle(great_circle_axis, d_theta);
        let new_unspun = (step_rot * unspun_pos).normalize_or_zero();
        let new_heading = (step_rot * heading)
            .reject_from(new_unspun)
            .normalize_or_zero();

        probe.landed_unspun_pos = Some(new_unspun);
        probe.rover_heading_unspun = Some(new_heading);

        let planet_rad_km = (physical_rad_au * AU_TO_KM).max(150.0);
        probe.distance_driven_km += d_theta * planet_rad_km;

        let should_record_waypoint = probe
            .traverse_trail_unspun
            .last()
            .is_none_or(|last| last.angle_between(new_unspun) >= 0.045);

        if should_record_waypoint {
            if probe.traverse_trail_unspun.len() >= 24 {
                probe.traverse_trail_unspun.remove(0);
            }
            probe.traverse_trail_unspun.push(new_unspun);
            probe.samples_collected = probe.samples_collected.saturating_add(1);
            if let Some(ref mut bio) = opt_bio {
                bio.surface_samples_collected = bio.surface_samples_collected.saturating_add(1);
            }
            probe.latest_assay = synthesize_probe_assay(
                ProbeType::Rover,
                probe.samples_collected,
                opt_comp,
                opt_climate,
                opt_bio.as_deref(),
            );
        }
    }
}

fn synthesize_probe_assay(
    probe_type: ProbeType,
    sample_index: u32,
    opt_comp: Option<&Composition>,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
) -> String {
    if let Some(bio) = opt_bio {
        if bio.biomass_coverage_frac > 0.05 {
            let pigment_str = bio.flora_pigment.label();
            return match probe_type {
                ProbeType::Rover => format!(
                    "Core #{sample_index}: Organic microfossils & {pigment_str} biosignatures confirmed (O2: {:.1}%)",
                    bio.oxygen_fraction * 100.0
                ),
                ProbeType::Orbiter => format!(
                    "Pass #{sample_index}: Red-edge reflectance & O2/CH4 disequilibrium ({pigment_str})"
                ),
            };
        }
    }

    let temp_k = opt_climate.map_or(280.0, |c| c.surface_temperature_k);
    let comp = opt_comp.copied().unwrap_or_else(Composition::rocky);
    match probe_type {
        ProbeType::Rover => {
            if comp.ice_frac > 0.20 || temp_k < 245.0 {
                format!(
                    "Core #{sample_index}: Cryogenic H2O/CO2 clathrate ice & hydrated phyllosilicates ({temp_k:.0} K)"
                )
            } else if comp.metal_frac > 0.35 {
                format!(
                    "Core #{sample_index}: High-density Fe-Ni metallic regolith & olivine basalts ({temp_k:.0} K)"
                )
            } else {
                format!(
                    "Core #{sample_index}: Basaltic silicate crust & ancient evaporite sulfates ({temp_k:.0} K)"
                )
            }
        }
        ProbeType::Orbiter => {
            if comp.gas_frac > 0.30 {
                format!(
                    "Pass #{sample_index}: Deep H2/He convective bands & ammonia-ice cloud decks ({temp_k:.0} K)"
                )
            } else {
                format!(
                    "Pass #{sample_index}: Gravimetric crustal mapping & IR thermal emission survey ({temp_k:.0} K)"
                )
            }
        }
    }
}
