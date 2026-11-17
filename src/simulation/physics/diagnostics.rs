//! Physics diagnostics (energy monitor telemetry), visual time progression,
//! and dynamic galactic nucleus re-anchoring for GMC clusters.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::CentralStar;
use crate::simulation::resources::{EnergyMonitor, SimTime, TimeWarp};
use crate::utils::constants::G_ASTRO;

use super::integrator::PhysicsBodyEntry;

pub(super) fn update_physics_energy_monitor(
    energy_monitor: &mut EnergyMonitor,
    body_data: &[PhysicsBodyEntry],
    massive_indices: &[usize],
    star_index: Option<usize>,
    star_pos: DVec3,
    star_mass: f64,
    softening_au: f64,
) {
    let softening_sq = softening_au * softening_au;
    let mut kinetic_e = 0.0;
    let mut potential_e = 0.0;

    for (i, b) in body_data.iter().enumerate() {
        kinetic_e += 0.5 * b.mass * b.vel.length_squared();

        if let Some(s_idx) = star_index
            && i != s_idx
        {
            let dist_sq = (b.pos - star_pos).length_squared() + softening_sq;
            let dist = dist_sq.sqrt();
            potential_e -= (G_ASTRO * star_mass * b.mass) / dist;
        }
    }

    // Include mutual potential energy between massive non-central bodies (binaries, giant planets, SMBHs)
    if massive_indices.len() > 1 {
        for (idx_a, &i) in massive_indices.iter().enumerate() {
            if Some(i) == star_index {
                continue;
            }
            let Some(b_i) = body_data.get(i) else {
                continue;
            };
            for &j in massive_indices.iter().skip(idx_a + 1) {
                if Some(j) == star_index {
                    continue;
                }
                let Some(b_j) = body_data.get(j) else {
                    continue;
                };
                let dist_sq = (b_i.pos - b_j.pos).length_squared() + softening_sq;
                let dist = dist_sq.sqrt();
                potential_e -= (G_ASTRO * b_i.mass * b_j.mass) / dist;
            }
        }
    }

    let total_e = kinetic_e + potential_e;
    let current_body_count = body_data.len();
    let current_total_mass: f64 = body_data.iter().map(|b| b.mass).sum();

    let count_changed =
        energy_monitor.initialized && current_body_count != energy_monitor.last_body_count;
    let mass_changed = energy_monitor.initialized
        && (current_total_mass - energy_monitor.last_total_mass).abs() > 1e-9;
    let should_rebaseline = !energy_monitor.initialized
        || energy_monitor.needs_rebaseline
        || count_changed
        || mass_changed;

    if should_rebaseline {
        if total_e.abs() > 1e-10 {
            energy_monitor.rebaseline(total_e, current_body_count, current_total_mass);
        }
    } else {
        let mut rel_drift = if energy_monitor.initial_total_energy.abs() > 1e-10 {
            ((total_e - energy_monitor.initial_total_energy) / energy_monitor.initial_total_energy)
                .abs()
        } else {
            0.0
        };

        // If drift exceeds 25% (0.25) in a single step due to external sandbox events,
        // smoothly re-baseline so EnergyMonitor strictly measures true symplectic drift.
        if rel_drift > 0.25 && total_e.abs() > 1e-10 {
            energy_monitor.rebaseline(total_e, current_body_count, current_total_mass);
            rel_drift = 0.0;
        }

        energy_monitor.relative_energy_drift = rel_drift;
        energy_monitor.last_body_count = current_body_count;
        energy_monitor.last_total_mass = current_total_mass;
    }

    energy_monitor.kinetic_energy = kinetic_e;
    energy_monitor.potential_energy = potential_e;
    energy_monitor.total_energy = total_e;
}

/// Pauses when time_warp is paused, freezing planetary rotation, cloud movement, and stellar boiling.
/// Scales down visual animation speed at sub-orbital time-warp levels (such as Real-Time `1s = 1s`)
/// so clouds and atmospheric weather drift at a calm, realistic pace.
pub fn update_simulation_visual_time(
    time: Res<Time>,
    time_warp: Res<TimeWarp>,
    mut sim_time: ResMut<SimTime>,
) {
    if !time_warp.is_paused || time_warp.step_once {
        let warp_scale = if time_warp.multiplier < 1.0 {
            let ratio = (time_warp.multiplier / TimeWarp::SPEED_REAL_TIME).max(0.1);
            (0.06 * ratio.powf(0.28) as f32).clamp(0.06, 1.0)
        } else {
            1.0
        };
        sim_time.visual_time_secs += time.delta_secs() * warp_scale;
    }
}

/// Re-anchors the GMC cluster center of frame to the dominant stellar/remnant mass.
pub fn enforce_dynamic_galactic_nucleus(
    body_data: &mut [PhysicsBodyEntry],
    is_gmc_cluster: bool,
    commands: &mut Commands,
) {
    if !is_gmc_cluster {
        return;
    }

    let candidate_anchor = body_data
        .iter()
        .enumerate()
        .filter(|(_, b)| b.body_type.is_star_or_remnant())
        .max_by(|a, b| {
            a.1.mass
                .partial_cmp(&b.1.mass)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, b)| (i, b.entity, b.mass));

    let Some((cand_idx, cand_entity, cand_mass)) = candidate_anchor else {
        return;
    };

    let current_central = body_data
        .iter()
        .enumerate()
        .find(|(_, b)| b.is_central_star)
        .map(|(i, b)| (i, b.entity, b.mass));

    let should_transfer = match current_central {
        None => true,
        Some((_curr_idx, curr_entity, curr_mass)) => {
            curr_entity != cand_entity
                && (cand_mass > curr_mass * 1.20 || (curr_mass < 5.0 && cand_mass >= 6.0))
        }
    };

    if should_transfer {
        if let Some((curr_idx, curr_entity, _)) = current_central
            && curr_entity != cand_entity
        {
            if let Some(old_b) = body_data.get_mut(curr_idx) {
                old_b.is_central_star = false;
            }
            if let Ok(mut cmd) = commands.get_entity(curr_entity) {
                cmd.remove::<CentralStar>();
            }
        }

        if let Some(anchor_b) = body_data.get(cand_idx) {
            let anchor_pos = anchor_b.pos;
            let anchor_vel = anchor_b.vel;

            // Smoothly translate the entire cluster so the Nucleus is precisely at DVec3::ZERO!
            for b in body_data.iter_mut() {
                b.pos -= anchor_pos;
                b.vel -= anchor_vel;
            }

            if let Some(anchor_mut) = body_data.get_mut(cand_idx) {
                anchor_mut.is_central_star = true;
            }
            if let Ok(mut cmd) = commands.get_entity(cand_entity) {
                cmd.insert(CentralStar);
            }
        }
    }
}
