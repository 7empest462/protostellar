//! Telemetry and Science Data harvesting from probes, biospheres, Dyson swarms, and discoveries.

use bevy::prelude::*;
use hashbrown::HashMap;

use super::types::{ScienceData, TechnologyId, UnlockedTechs};
use crate::game::phases::PhaseManager;
use crate::game::ui::NotificationToast;
use crate::rendering::effects::megastructures::DysonSwarm;
use crate::simulation::components::planetary::BiosphereState;
use crate::simulation::probes::{ProbeStatus, SpaceProbe};
use crate::simulation::resources::SimTime;

/// Local tracker to reward science points when probes collect new regolith samples or drive distance.
#[derive(Default)]
pub struct ProbeTelemetryState {
    pub last_samples: HashMap<usize, u32>,
    pub last_driven_km: HashMap<usize, f64>,
}

/// Continuous harvesting system collecting science data from probes, civilizations, and Dyson swarms.
pub fn harvest_telemetry_system(
    mut science_data: ResMut<ScienceData>,
    unlocked_techs: Res<UnlockedTechs>,
    probes_query: Query<&SpaceProbe>,
    biosphere_query: Query<&BiosphereState>,
    swarm_query: Query<&DysonSwarm>,
    time: Res<Time>,
    time_warp: Option<Res<crate::simulation::resources::TimeWarp>>,
    mut local_state: Local<ProbeTelemetryState>,
) {
    let is_paused = time_warp
        .as_deref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once);
    let dt_visual_secs: f64 = if is_paused {
        0.0
    } else {
        time.delta_secs_f64().clamp(0.0, 0.5)
    };

    let mut passive_rate = 0.0;

    // 1. Probes & Rovers Telemetry
    for probe in probes_query.iter() {
        // Continuous telemetry from active orbiters and rovers
        if probe.status == ProbeStatus::InOrbit || probe.status == ProbeStatus::Landed {
            let base_telemetry = if unlocked_techs.has(TechnologyId::SolarSail) {
                1.2
            } else {
                0.5
            };
            passive_rate += base_telemetry;
        }

        // Discrete sample bonuses (+25 pts per new drill/scan sample)
        let prev_samples = local_state.last_samples.entry(probe.id).or_insert(0);
        if probe.samples_collected > *prev_samples {
            let new_samples = probe.samples_collected - *prev_samples;
            let bonus = f64::from(new_samples) * 25.0;
            science_data.add_science(bonus);
            *prev_samples = probe.samples_collected;
        }

        // Traverse distance bonuses (+10 pts per 1,000 km driven)
        let prev_km = local_state.last_driven_km.entry(probe.id).or_insert(0.0);
        if probe.distance_driven_km > *prev_km + 500.0 {
            let delta_km = probe.distance_driven_km - *prev_km;
            let km_bonus = (delta_km / 500.0) * 5.0;
            science_data.add_science(km_bonus);
            *prev_km = probe.distance_driven_km;
        }
    }

    // 2. Biospheres & Technological Civilizations
    for bio in biosphere_query.iter() {
        if bio.technosignature > 0.0 {
            // Advanced civilizations generate high-bandwidth scientific telemetry
            let civ_rate = f64::from(bio.technosignature) * 2.5;
            passive_rate += civ_rate;
        }
    }

    // 3. Megastructures & Dyson Swarms
    let dyson_multiplier = if unlocked_techs.has(TechnologyId::DysonCollectors) {
        2.5
    } else {
        1.0
    };
    for swarm in swarm_query.iter() {
        if swarm.completion_fraction > 0.0 {
            let swarm_rate = f64::from(swarm.completion_fraction) * 12.0 * dyson_multiplier;
            passive_rate += swarm_rate;
        }
    }

    // Update global rate & accumulate points
    science_data.passive_rate_per_sec = passive_rate;
    if dt_visual_secs > 0.0 && passive_rate > 0.0 {
        science_data.add_science(passive_rate * dt_visual_secs);
    }
}

/// Awards large one-time research grants when cosmological and planetary milestones are unlocked.
pub fn sync_milestone_discoveries_system(
    mut science_data: ResMut<ScienceData>,
    phase_manager: Option<Res<PhaseManager>>,
    sim_time: Option<Res<SimTime>>,
    mut toast: Option<ResMut<NotificationToast>>,
    mut recorded_milestone_ids: Local<hashbrown::HashSet<crate::game::phases::MilestoneId>>,
) {
    let Some(phase) = phase_manager else {
        return;
    };
    let current_year = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);

    for milestone in &phase.milestones {
        if milestone.achieved && !recorded_milestone_ids.contains(&milestone.id) {
            recorded_milestone_ids.insert(milestone.id);

            let bonus_points = 100.0;
            science_data.record_discovery(
                milestone.title,
                milestone.prompt,
                bonus_points,
                current_year,
            );

            if let Some(ref mut t) = toast {
                t.message = format!(
                    "🔬 Breakthrough Discovery: {} (+{:.0} Science Data!)",
                    milestone.title, bonus_points
                );
                t.timer = 5.0;
            }
        }
    }
}
