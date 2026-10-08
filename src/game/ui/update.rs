//! Systems for updating telemetry, timer banners, inspector readouts, and HUD visibility.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::game::phases::PhaseManager;
use crate::simulation::accretion::RocheDisruptionEvent;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::telemetry_format::*;
use super::types::*;

pub fn handle_roche_disruption_toasts(
    mut roche_events: MessageReader<RocheDisruptionEvent>,
    mut toast: ResMut<NotificationToast>,
) {
    for event in roche_events.read() {
        toast.message = format!(
            "💥 TIDAL DISRUPTION! Moon \"{}\" shredded into glowing planetary ring around \"{}\"!",
            event.disrupted_name, event.primary_name
        );
        toast.timer = 6.0;
    }
}

#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
fn update_toast_text(
    toast_text: &mut Text,
    phase_mgr: &PhaseManager,
    toast: &NotificationToast,
    player_state: &PlayerInteractionState,
    bodies_query: &HudBodiesQuery,
) {
    if phase_mgr.milestone_toast_timer > 0.0 {
        if let Some(ref m_title) = phase_mgr.latest_unlocked_milestone {
            toast_text.0 = format!("🎉 MILESTONE UNLOCKED: {m_title} 🎉");
            return;
        } else if toast.timer > 0.0 {
            toast_text.0.clone_from(&toast.message);
            return;
        }
    }

    if toast.timer > 0.0 {
        toast_text.0.clone_from(&toast.message);
        return;
    }

    if let Some(selected_entity) = player_state.selected_entity {
        if let Ok(((pos, vel, mass, rad, temp, _comp, body), ..)) =
            bodies_query.get(selected_entity)
        {
            let type_name = format_body_inspector_type(body.body_type, mass.0).to_uppercase();
            let m_earth = mass.0 / EARTH_MASS_SOLAR;
            let mass_str = if mass.0 >= 0.01 {
                format!(
                    "{:.2} M_sun ({:.1} M_J)",
                    mass.0,
                    mass.0 / JUPITER_MASS_SOLAR
                )
            } else if m_earth >= 0.01 {
                format!("{m_earth:.2} M_earth")
            } else if m_earth >= 1e-4 {
                format!("{m_earth:.4} M_earth")
            } else {
                format!("{m_earth:.2e} M_earth")
            };
            let dist_au = if pos.0.is_finite() {
                pos.0.length()
            } else {
                0.0
            };
            let speed_km_s = if vel.0.is_finite() {
                vel.0.length() * AU_PER_YR_TO_KM_PER_S
            } else {
                0.0
            };
            let rad_km = (rad.0 * AU_TO_KM).max(0.01);
            let rad_str = if rad_km >= 100.0 {
                format!("{rad_km:.0} km")
            } else if rad_km >= 10.0 {
                format!("{rad_km:.1} km")
            } else {
                format!("{rad_km:.2} km")
            };

            toast_text.0 = format!(
                ">> SELECTED: {} [{}]  |  Mass: {}  |  Radius: {}  |  Dist: {:.2} AU  |  Speed: {:.1} km/s  |  Temp: {:.0} K",
                body.name.to_uppercase(),
                type_name,
                mass_str,
                rad_str,
                dist_au,
                speed_km_s,
                temp.0,
            );
        } else {
            toast_text.0 = ">> PROTOSTELLAR LIVE // Select any planet or the Sun".to_string();
        }
    } else {
        toast_text.0 = ">> PROTOSTELLAR LIVE // Click any planet or the Sun".to_string();
    }
}

fn update_header_stats(
    text: &mut Text,
    sim_time: &SimTime,
    phase_mgr: &PhaseManager,
    config: &SimulationConfig,
    lhb_state: &crate::game::phases::LateHeavyBombardmentState,
) {
    let time_formatted = if sim_time.elapsed_years > 1_000_000.0 {
        format!("{:.2} Myr", sim_time.elapsed_years / 1_000_000.0)
    } else if sim_time.elapsed_years > 1_000.0 {
        format!("{:.1} kyr", sim_time.elapsed_years / 1_000.0)
    } else {
        format!("{:.2} yr", sim_time.elapsed_years)
    };

    let phase_str = match phase_mgr.current_phase {
        crate::game::phases::SystemPhase::MolecularCloudCollapse => "MOLECULAR CLOUD COLLAPSE",
        crate::game::phases::SystemPhase::ProtoplanetaryDisk => "PROTOPLANETARY DISK FORMATION",
        crate::game::phases::SystemPhase::StarIgnition => "STAR IGNITION (FUSION ONSET)",
        crate::game::phases::SystemPhase::PlanetaryAccretion => {
            "PLANETARY ACCRETION & EMBRYO GROWTH"
        }
        crate::game::phases::SystemPhase::LateHeavyBombardment => {
            "LATE HEAVY BOMBARDMENT (2:1 RESONANCE & MIGRATION)"
        }
        crate::game::phases::SystemPhase::MatureSolarSystem => "MATURE SOLAR SYSTEM",
        crate::game::phases::SystemPhase::StellarMetamorphosis => {
            "STELLAR METAMORPHOSIS (RED GIANT / WHITE DWARF)"
        }
        crate::game::phases::SystemPhase::SpiralGalaxyEvolution => {
            "GRAND DESIGN SPIRAL GALAXY EVOLUTION"
        }
    };

    let gas_status = if config.gas_density_scale > 0.05 {
        format!("{:.0}%", config.gas_density_scale * 100.0)
    } else {
        "Dispersed".to_string()
    };

    let active_goal = phase_mgr
        .milestones
        .iter()
        .find(|m| !m.achieved)
        .map_or_else(
            || "All Formation Milestones Completed!".to_string(),
            |m| format!("{}: {}", m.title, m.prompt),
        );

    let lhb_info = if lhb_state.is_active {
        format!(
            "\n☄️ LHB MIGRATION: {:.0}% (Resonance: {:.2}:1 | Impactors Scattered: {} | Water Delivered: {:.5} M⊕)",
            lhb_state.migration_progress * 100.0,
            lhb_state.resonance_ratio,
            lhb_state.comets_scattered,
            lhb_state.water_delivered_earth_masses
        )
    } else if phase_mgr.current_phase == crate::game::phases::SystemPhase::SpiralGalaxyEvolution {
        "\n🌀 SPIRAL ARMS: Lin-Shu Waves | Pattern: 14 kyr | Pitch: 18° | Arm Resonance: Active"
            .to_string()
    } else {
        String::new()
    };

    let compute_badge = if config.enable_gpu_compute && config.gpu_compute_active {
        "⚡ GPU Compute [F8]"
    } else if config.enable_gpu_compute {
        "⚡ GPU Init [F8]"
    } else {
        "🖥️ CPU Fallback [F8]"
    };

    let belt_line = phase_mgr.belt_census.format_summary_line();

    text.0 = format!(
        "Phase: {}\nTime: T + {} | Star: {:.2} M_sun\nSwarm: {} / {} particles [{}] | Gas: {}\nPlanets: {} | Embryos: {} | 🪨 Ast: {} | ☄️ Comets: {}\n{}\nGoal: {}{}",
        phase_str,
        time_formatted,
        phase_mgr.star_mass,
        config.active_particles,
        config.target_particle_count,
        compute_badge,
        gas_status,
        phase_mgr.planet_count,
        phase_mgr.protoplanet_count,
        phase_mgr.asteroid_count,
        phase_mgr.comet_count,
        belt_line,
        active_goal,
        lhb_info,
    );
}

fn update_time_warp_diagnostics(
    text: &mut Text,
    time_warp: &TimeWarp,
    player_state: &PlayerInteractionState,
    energy_monitor: &EnergyMonitor,
    config: &SimulationConfig,
    sim_time: &SimTime,
) {
    let speed_str = time_warp.human_readable_speed();
    let tool_str = match player_state.active_tool {
        PlayerTool::Inspect => "INSPECT & LIVE-EDIT",
        PlayerTool::Slingshot => "ORBITAL SLINGSHOT [K]",
        PlayerTool::GravitationalTractor => "GRAVITATIONAL TRACTOR",
        PlayerTool::GravitationalImpulse => "DELTA-V IMPULSE",
        PlayerTool::MassInjection => "MASS INJECTION",
        PlayerTool::DensityWave => "DENSITY WAVE",
    };

    let drift_pct = if energy_monitor.relative_energy_drift.is_finite() {
        (energy_monitor.relative_energy_drift * 100.0).clamp(0.0, 999.0)
    } else {
        0.0
    };

    text.0 = format!(
        "SPEED: {}\nTOOL: {}\nOVERLAY: {} [V]\nSize Scale: {:.2}x [,/.]\nCompute: {} [F8]\nAccretion: Active (Boost: {:.0}x)\nEnergy Drift: {:.4}%\nSim Steps: {}",
        speed_str,
        tool_str,
        player_state.overlay_mode.display_name(),
        config.size_exaggeration,
        if config.enable_gpu_compute { "WGPU Compute (100k)" } else { "CPU Multithread" },
        config.accretion_rate_multiplier,
        drift_pct,
        sim_time.step_count,
    );
}

fn update_bottom_timer(text: &mut Text, sim_time: &SimTime, time_warp: &TimeWarp) {
    let yr = sim_time.elapsed_years;
    let time_formatted = if yr >= 1_000_000.0 {
        format!(
            "{:.3} Million Years ({:.4} Myr)",
            yr / 1_000_000.0,
            yr / 1_000_000.0
        )
    } else if yr >= 1_000.0 {
        format!(
            "{:.1} Thousand Years ({:.2} kyr)",
            yr / 1_000.0,
            yr / 1_000.0
        )
    } else if yr < 0.01 {
        let total_sec = yr * TimeWarp::SECONDS_PER_YEAR;
        if total_sec < 60.0 {
            format!("{total_sec:.1} Seconds")
        } else if total_sec < 3600.0 {
            format!("{:.1} Minutes ({total_sec:.0}s)", total_sec / 60.0)
        } else if total_sec < 86400.0 {
            let hours = total_sec / 3600.0;
            format!("{hours:.1} Hours ({hours:.1}h)")
        } else {
            let days = yr * 365.25;
            format!("{days:.2} Days ({yr:.4} yr)")
        }
    } else {
        format!("{yr:.2} Years")
    };

    let status_str = if time_warp.is_paused {
        "PAUSED [Space to Resume]"
    } else {
        "FLOWING"
    };

    text.0 = format!(
        "SIMULATION ELAPSED TIME: {}\nSPEED: {} | STATUS: {}",
        time_formatted,
        time_warp.human_readable_speed(),
        status_str,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Telemetry inspector requires full access to celestial body components and configuration state"
)]
fn update_inspector_body_telemetry(
    text: &mut Text,
    selected_entity: Entity,
    bodies_query: &HudBodiesQuery,
    quasi_hud_query: &Query<&BlackHoleStarState>,
    jet_hud_query: &Query<&RelativisticJetState>,
    space_weather_hud_query: &Query<(
        Option<&crate::simulation::space_weather::AuroralOvalState>,
        Option<&crate::simulation::space_weather::StellarFlareState>,
    )>,
    probes_hud_query: &Query<(
        &crate::simulation::probes::SpaceProbe,
        &crate::simulation::probes::FlightComputer,
    )>,
    phase_mgr: &PhaseManager,
    config: &SimulationConfig,
) {
    let Ok((
        (pos, vel, mass, rad, temp, comp, body),
        (
            opt_diff,
            opt_spin,
            opt_ignition,
            opt_vol,
            opt_rings,
            opt_tail,
            opt_climate,
            opt_bio,
            opt_evo,
            opt_tide,
            opt_rel,
            opt_escape,
            opt_kozai,
        ),
    )) = bodies_query.get(selected_entity)
    else {
        text.0 = "Selected body was absorbed in an accretion merger.".to_string();
        return;
    };

    let env_str = format_body_environment_telemetry(
        opt_diff,
        opt_vol,
        opt_rings,
        opt_tail,
        opt_climate,
        opt_bio,
        opt_ignition,
        opt_evo,
        opt_tide,
        opt_rel,
        opt_escape,
        opt_kozai,
        rad,
        temp,
        config,
    );

    let comp_str = format_composition_line(
        comp,
        opt_vol,
        opt_climate,
        temp.0,
        body.body_type.is_star_or_remnant(),
    );

    text.0 = format_primary_telemetry(
        pos.0, vel.0, mass.0, rad.0, temp.0, comp, body, opt_spin, phase_mgr, &env_str, &comp_str,
    );

    if let Ok(qs) = quasi_hud_query.get(selected_entity) {
        append_quasi_star_telemetry(&mut text.0, qs);
    }

    if let Ok(jet) = jet_hud_query.get(selected_entity) {
        super::inspector_panel::append_relativistic_jet_telemetry(&mut text.0, jet);
    }

    if let Ok((opt_aurora, opt_flare)) = space_weather_hud_query.get(selected_entity) {
        if opt_aurora.is_some() || opt_flare.is_some() {
            super::inspector_panel::append_space_weather_telemetry(
                &mut text.0,
                opt_aurora,
                opt_flare,
            );
        }
    }

    if let Ok((probe, comp)) = probes_hud_query.get(selected_entity) {
        append_probe_telemetry(&mut text.0, probe, comp);
    }
}

fn append_probe_telemetry(
    text: &mut String,
    probe: &crate::simulation::probes::SpaceProbe,
    comp: &crate::simulation::probes::FlightComputer,
) {
    use std::fmt::Write;
    let type_label = match probe.probe_type {
        crate::simulation::probes::ProbeType::Orbiter => "ORBITAL SURVEY PROBE",
        crate::simulation::probes::ProbeType::Rover => "SURFACE EXPLORATION ROVER",
    };
    let status_label = match probe.status {
        crate::simulation::probes::ProbeStatus::Transit => {
            "EN ROUTE // AUTONOMOUS ION PROPULSION ACTIVE"
        }
        crate::simulation::probes::ProbeStatus::InOrbit => "STABLE ORBIT INSERTION ACHIEVED",
        crate::simulation::probes::ProbeStatus::Landed => {
            "SURFACE TOUCHDOWN // STATIONARY EXPLORATION"
        }
    };
    let _ = write!(
        text,
        "\n\n>> AUTONOMOUS MISSION TELEMETRY <<\n\
         Mission Type: {type_label}\n\
         Destination World: {}\n\
         Flight Status: {status_label}\n\
         Range to Target: {:.3} AU ({:.0} km)\n\
         Relative Speed: {:.1} km/s",
        comp.target_name,
        comp.distance_to_target,
        comp.distance_to_target * AU_TO_KM,
        comp.relative_speed * AU_PER_YR_TO_KM_PER_S
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Formatting requires numerous component properties and configuration states"
)]
fn format_primary_telemetry(
    pos: DVec3,
    vel: DVec3,
    mass: f64,
    rad: f64,
    temp: f64,
    comp: &Composition,
    body: &CelestialBody,
    opt_spin: Option<&SpinState>,
    phase_mgr: &PhaseManager,
    env_str: &str,
    comp_str: &str,
) -> String {
    let dist_au = if pos.is_finite() { pos.length() } else { 0.0 };
    let speed_au_yr = if vel.is_finite() { vel.length() } else { 0.0 };
    let speed_km_s = speed_au_yr * AU_PER_YR_TO_KM_PER_S;

    let mass_str = format_mass_string(mass);
    let radius_km = (rad * AU_TO_KM).max(0.01);
    let rad_disp = if radius_km >= 100.0 {
        format!("{radius_km:.0} km")
    } else if radius_km >= 10.0 {
        format!("{radius_km:.1} km")
    } else {
        format!("{radius_km:.2} km")
    };
    let density_g_cm3 = (comp.average_density() * SOLAR_MASS_KG / (AU_TO_METERS.powi(3) * 1000.0))
        .clamp(0.01, 20.0);
    let period_str = format_orbital_period_string(dist_au, body.body_type, phase_mgr.star_mass);

    let spin_str = if let Some(spin) = opt_spin {
        format!(
            " | Day: {:.1}h | Tilt: {:.1} deg",
            spin.rotation_period_hours, spin.axial_tilt_degrees
        )
    } else {
        String::new()
    };

    let zone = BeltZone::from_distance_au(dist_au);
    let belt_suffix = if matches!(
        body.body_type,
        BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal | BodyType::DustGrain
    ) {
        format!(" | Belt: {} {}", zone.icon(), zone.short_name())
    } else {
        String::new()
    };

    format!(
        ">> {} [{}]\nMass: {}\nRadius: {} ({:.4} AU)\nDensity: {:.2} g/cm3 | Temp: {:.0} K{}{}\nDistance: {:.2} AU{} | Speed: {:.1} km/s\nComposition: {}{}",
        body.name.to_uppercase(),
        format_body_inspector_type(body.body_type, mass).to_uppercase(),
        mass_str,
        rad_disp,
        rad,
        density_g_cm3,
        temp,
        spin_str,
        period_str,
        dist_au,
        belt_suffix,
        speed_km_s,
        comp_str,
        env_str,
    )
}

/// Updates the dynamic content of the HUD and notification toast banner every frame.
#[allow(
    clippy::type_complexity,
    reason = "HUD update requires access to a wide range of celestial body components and simulation state"
)]
pub fn update_hud(
    (time, sim_time, time_warp, config, energy_monitor, phase_mgr, lhb_state): (
        Res<Time>,
        Res<SimTime>,
        Res<TimeWarp>,
        Res<SimulationConfig>,
        Res<EnergyMonitor>,
        Res<PhaseManager>,
        Res<crate::game::phases::LateHeavyBombardmentState>,
    ),
    player_state: Res<PlayerInteractionState>,
    mut toast: ResMut<NotificationToast>,
    bodies_query: HudBodiesQuery,
    quasi_hud_query: Query<&BlackHoleStarState>,
    jet_hud_query: Query<&RelativisticJetState>,
    space_weather_hud_query: Query<(
        Option<&crate::simulation::space_weather::AuroralOvalState>,
        Option<&crate::simulation::space_weather::StellarFlareState>,
    )>,
    probes_hud_query: Query<(
        &crate::simulation::probes::SpaceProbe,
        &crate::simulation::probes::FlightComputer,
    )>,
    mut text_queries: HudTextQueries,
    opt_predictor: Option<Res<crate::simulation::predictor::TrajectoryPredictorState>>,
) {
    if toast.timer > 0.0 {
        toast.timer -= time.delta_secs();
    }
    if let Ok(mut toast_text) = text_queries.toast.single_mut() {
        update_toast_text(
            &mut toast_text,
            &phase_mgr,
            &toast,
            &player_state,
            &bodies_query,
        );
    }
    if let Ok(mut text) = text_queries.header.single_mut() {
        update_header_stats(&mut text, &sim_time, &phase_mgr, &config, &lhb_state);
    }
    if let Ok(mut text) = text_queries.time.single_mut() {
        update_time_warp_diagnostics(
            &mut text,
            &time_warp,
            &player_state,
            &energy_monitor,
            &config,
            &sim_time,
        );
    }
    if let Ok(mut text) = text_queries.bottom_timer.single_mut() {
        update_bottom_timer(&mut text, &sim_time, &time_warp);
    }

    if let Ok(mut text) = text_queries.inspector.single_mut() {
        if let Some(selected_entity) = player_state.selected_entity {
            update_inspector_body_telemetry(
                &mut text,
                selected_entity,
                &bodies_query,
                &quasi_hud_query,
                &jet_hud_query,
                &space_weather_hud_query,
                &probes_hud_query,
                &phase_mgr,
                &config,
            );
        } else {
            text.0 = "No celestial body selected.\nClick on the Star or Planets above (or in 3D) to inspect & live-edit.\n[Tab] Next Body | [F] Focus Target | [WASD] Free-Fly View".to_string();
        }
    }

    if let Ok(mut text) = text_queries.forecast.single_mut() {
        update_encounter_forecast_text(
            &mut text,
            opt_predictor.as_deref(),
            player_state.selected_entity.is_some(),
        );
    }
}

fn update_encounter_forecast_text(
    text: &mut Text,
    opt_predictor: Option<&crate::simulation::predictor::TrajectoryPredictorState>,
    has_selection: bool,
) {
    let Some(predictor) = opt_predictor else {
        text.0 = "🎯 FORECAST: Initializing...".to_string();
        return;
    };

    if !predictor.is_enabled {
        text.0 =
            "🎯 FORECAST: Disabled [N] to enable trajectory & encounter prediction.".to_string();
        return;
    }

    if let Some(enc) = &predictor.active_encounter {
        text.0 = format!(
            "🎯 FORECAST: {} with {}\nETA: {:.1} yr | Closest: {:.4} AU ({:.0} km)\nRel Speed: {:.2} km/s (v_inf)",
            enc.encounter_type.display_label(),
            enc.target_name,
            enc.time_to_encounter_yr,
            enc.min_distance_au,
            enc.min_distance_km,
            enc.relative_velocity_kms,
        );
    } else if has_selection {
        text.0 = "🎯 FORECAST: Safe trajectory. No close encounters detected (Horizon: 30 yr)."
            .to_string();
    } else {
        text.0 =
            "🎯 FORECAST [N]: Select a body or drag Slingshot to preview encounters.".to_string();
    }
}
