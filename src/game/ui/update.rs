//! Systems for updating telemetry, timer banners, inspector readouts, and HUD visibility.

use bevy::prelude::*;

use crate::game::phases::PhaseManager;
use crate::simulation::accretion::RocheDisruptionEvent;
use crate::simulation::components::*;
use crate::simulation::neutron_stars::types::{MagnetarCrustStress, PulsarGlitchState};
use crate::simulation::oort::types::OortComet;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::telemetry_format::*;
use super::types::*;

fn push_cosmic_toast(
    toast: &mut NotificationToast,
    event_log: &mut CosmicEventLog,
    timestamp_yr: f64,
    icon: &'static str,
    msg: String,
    target_entity: Option<Entity>,
) {
    event_log.push(timestamp_yr, icon, msg.clone(), target_entity);
    toast.message = msg;
    toast.timer = 6.0;
}

pub fn handle_roche_disruption_toasts(
    mut roche_events: MessageReader<RocheDisruptionEvent>,
    mut toast: ResMut<NotificationToast>,
    mut event_log: Option<ResMut<CosmicEventLog>>,
    sim_time: Option<Res<SimTime>>,
) {
    let timestamp_yr = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);
    for event in roche_events.read() {
        let msg = format!(
            "💥 TIDAL DISRUPTION! Moon \"{}\" shredded into glowing planetary ring around \"{}\"!",
            event.disrupted_name, event.primary_name
        );
        if let Some(ref mut log) = event_log {
            push_cosmic_toast(
                &mut toast,
                log,
                timestamp_yr,
                "💥",
                msg,
                Some(event.primary_entity),
            );
        } else {
            toast.message = msg;
            toast.timer = 6.0;
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct CosmicEventsReaders<'w, 's> {
    pub cme: MessageReader<'w, 's, crate::simulation::space_weather::CmeShockwaveEvent>,
    pub kozai: MessageReader<'w, 's, crate::simulation::kozai_lidov::KozaiDisruptionEvent>,
    pub stripped:
        MessageReader<'w, 's, crate::simulation::atmosphere_escape::AtmosphericStrippedEvent>,
    pub gw_merger:
        MessageReader<'w, 's, crate::simulation::relativity::GravitationalWaveMergerEvent>,
    pub supernova: MessageReader<'w, 's, SupernovaEvent>,
    pub tde: MessageReader<'w, 's, crate::simulation::accretion::TidalDisruptionEvent>,
    pub moon: MessageReader<'w, 's, crate::simulation::accretion::MoonFormationEvent>,
}

/// Surfaces major astrophysical simulation events to the HUD toast banner and bounded event log.
pub fn handle_cosmic_event_toasts(
    mut readers: CosmicEventsReaders,
    mut toast: ResMut<NotificationToast>,
    mut event_log: ResMut<CosmicEventLog>,
    sim_time: Option<Res<SimTime>>,
    bodies: Query<&CelestialBody>,
) {
    let timestamp_yr = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);

    for ev in readers.cme.read() {
        let target_name = bodies.get(ev.target).map_or("System", |b| b.name.as_str());
        let msg = format!(
            "⚡ CME SHOCKWAVE! Class x{:.1} flare strikes {} at {:.2} AU!",
            ev.flare_intensity, target_name, ev.orbital_radius_au
        );
        push_cosmic_toast(
            &mut toast,
            &mut event_log,
            timestamp_yr,
            "⚡",
            msg,
            Some(ev.target),
        );
    }

    for ev in readers.kozai.read() {
        let msg = format!(
            "🔄 KOZAI-LIDOV DISRUPTION! \"{}\" pumped to e={:.3}, grazed {:.3} AU!",
            ev.body_name, ev.peak_eccentricity, ev.periastron_au
        );
        push_cosmic_toast(
            &mut toast,
            &mut event_log,
            timestamp_yr,
            "🔄",
            msg,
            Some(ev.disrupted_entity),
        );
    }

    for ev in readers.stripped.read() {
        let msg = format!(
            "💨 ATMOSPHERE STRIPPED! \"{}\" lost volatile envelope ({:.3} M⊕ left)!",
            ev.name, ev.remaining_mass_earth
        );
        push_cosmic_toast(
            &mut toast,
            &mut event_log,
            ev.time_yr.max(timestamp_yr),
            "💨",
            msg,
            Some(ev.planet),
        );
    }

    for ev in readers.gw_merger.read() {
        let remnant_label = format_body_inspector_type(ev.remnant_type, ev.remnant_mass_solar);
        let msg = format!(
            "🌊 GRAVITATIONAL-WAVE MERGER! {:.2} M☉ {} formed (ΔE_GW={:.3} M☉c², {:.0} Hz)!",
            ev.remnant_mass_solar,
            remnant_label,
            ev.radiated_gw_mass_solar,
            ev.peak_gw_frequency_hz
        );
        push_cosmic_toast(
            &mut toast,
            &mut event_log,
            timestamp_yr,
            "🌊",
            msg,
            Some(ev.merged_entity),
        );
    }

    for ev in readers.supernova.read() {
        let remnant_label = format_body_inspector_type(ev.remnant_type, ev.remnant_mass_solar);
        let msg = format!(
            "🌟 SUPERNOVA DETONATION! \"{}\" collapsed to {:.2} M☉ {} ({:.0} km/s shock)!",
            ev.star_name, ev.remnant_mass_solar, remnant_label, ev.shockwave_velocity_km_s
        );
        push_cosmic_toast(
            &mut toast,
            &mut event_log,
            timestamp_yr,
            "🌟",
            msg,
            Some(ev.star_entity),
        );
    }

    for ev in readers.tde.read() {
        let msg = format!(
            "🕳️ SPAGHETTIFICATION TDE! \"{}\" tidally shredded by \"{}\"!",
            ev.disrupted_name, ev.bh_name
        );
        push_cosmic_toast(
            &mut toast,
            &mut event_log,
            timestamp_yr,
            "🕳️",
            msg,
            Some(ev.bh_entity),
        );
    }

    for ev in readers.moon.read() {
        let parent_name = bodies
            .get(ev.parent_entity)
            .map_or("Parent World", |b| b.name.as_str());
        let msg = format!(
            "🌙 MOON FORMED! Coalesced in orbit around \"{}\" at {:.4} AU!",
            parent_name, ev.orbital_radius_au
        );
        push_cosmic_toast(
            &mut toast,
            &mut event_log,
            timestamp_yr,
            "🌙",
            msg,
            Some(ev.moon_entity),
        );
    }
}

#[inline]
pub fn set_text_if_changed(text: &mut Mut<'_, Text>, next: String) {
    if text.as_ref().0 != next {
        text.0 = next;
    }
}

#[inline]
pub fn set_text_str_if_changed(text: &mut Mut<'_, Text>, next: &str) {
    if text.as_ref().0 != next {
        next.clone_into(&mut text.0);
    }
}

#[inline]
pub fn set_display_if_changed(node: &mut Mut<'_, Node>, next: Display) {
    if node.as_ref().display != next {
        node.display = next;
    }
}

#[inline]
pub fn set_border_color_if_changed(border: &mut Mut<'_, BorderColor>, next: BorderColor) {
    border.set_if_neq(next);
}

#[inline]
pub fn set_bg_color_if_changed(bg: &mut Mut<'_, BackgroundColor>, next: BackgroundColor) {
    bg.set_if_neq(next);
}

#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
fn update_toast_text(
    toast_text: &mut Mut<'_, Text>,
    phase_mgr: &PhaseManager,
    toast: &NotificationToast,
    player_state: &PlayerInteractionState,
    bodies_query: &HudBodiesQuery,
) {
    if phase_mgr.milestone_toast_timer > 0.0 {
        if let Some(ref m_title) = phase_mgr.latest_unlocked_milestone {
            set_text_if_changed(toast_text, format!("🎉 MILESTONE UNLOCKED: {m_title} 🎉"));
            return;
        } else if toast.timer > 0.0 {
            set_text_str_if_changed(toast_text, &toast.message);
            return;
        }
    }

    if toast.timer > 0.0 {
        set_text_str_if_changed(toast_text, &toast.message);
        return;
    }

    if let Some(selected_entity) = player_state.selected_entity {
        if let Ok(((pos, vel, mass, rad, temp, _comp, body), (.., opt_sat))) =
            bodies_query.get(selected_entity)
        {
            let type_name = format_body_inspector_type(body.body_type, mass.0);
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
            let (rel_pos, rel_vel) = opt_sat
                .and_then(|sat| bodies_query.get(sat.parent).ok())
                .map_or((pos.0, vel.0), |((p_pos, p_vel, ..), _)| {
                    (pos.0 - p_pos.0, vel.0 - p_vel.0)
                });
            let dist_au = if rel_pos.is_finite() {
                rel_pos.length()
            } else {
                0.0
            };
            let dist_str = if dist_au > 0.0 && dist_au < 0.05 {
                format!("{dist_au:.4} AU")
            } else {
                format!("{dist_au:.2} AU")
            };
            let speed_km_s = if rel_vel.is_finite() {
                rel_vel.length() * AU_PER_YR_TO_KM_PER_S
            } else {
                0.0
            };
            let rad_str = format_body_radius_display(rad.0);

            set_text_if_changed(
                toast_text,
                format!(
                    ">> SELECTED: {} [{}]  |  Mass: {}  |  Radius: {}  |  Dist: {}  |  Speed: {:.1} km/s  |  Temp: {:.0} K",
                    DisplayUppercase(&body.name),
                    DisplayUppercase(type_name),
                    mass_str,
                    rad_str,
                    dist_str,
                    speed_km_s,
                    temp.0,
                ),
            );
        } else {
            set_text_str_if_changed(
                toast_text,
                ">> PROTOSTELLAR LIVE // Select any planet or the Sun",
            );
        }
    } else {
        set_text_str_if_changed(
            toast_text,
            ">> PROTOSTELLAR LIVE // Click any planet or the Sun",
        );
    }
}

fn update_header_stats(
    text: &mut Mut<'_, Text>,
    sim_time: &SimTime,
    phase_mgr: &PhaseManager,
    config: &SimulationConfig,
    lhb_state: &crate::game::phases::LateHeavyBombardmentState,
    opt_scenario: Option<&crate::simulation::scenarios::ActiveScenarioState>,
) {
    let is_genesis = opt_scenario.is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::AccretionDiskGenesis
    });
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
            if is_genesis {
                "PLANETESIMAL SCATTERING & BOMBARDMENT"
            } else {
                "LATE HEAVY BOMBARDMENT (2:1 RESONANCE & MIGRATION)"
            }
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
        if is_genesis {
            format!(
                "\n☄️ BOMBARDMENT EPOCH: {:.0}% (Impactors Scattered: {} | Water Delivered: {:.5} M⊕)",
                lhb_state.migration_progress * 100.0,
                lhb_state.comets_scattered,
                lhb_state.water_delivered_earth_masses
            )
        } else {
            format!(
                "\n☄️ LHB MIGRATION: {:.0}% (Resonance: {:.2}:1 | Impactors Scattered: {} | Water Delivered: {:.5} M⊕)",
                lhb_state.migration_progress * 100.0,
                lhb_state.resonance_ratio,
                lhb_state.comets_scattered,
                lhb_state.water_delivered_earth_masses
            )
        }
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
    let clock_str = sim_time.format_visual_clock();

    set_text_if_changed(
        text,
        format!(
            "Phase: {}\nTime: T + {} ({})\nStar: {:.2} M_sun | Gas: {}\nSwarm: {} / {} particles [{}]\nPlanets: {} | Embryos: {} | 🪨 Ast: {} | ☄️ Comets: {}\n{}\nGoal: {}{}",
            phase_str,
            time_formatted,
            clock_str,
            phase_mgr.star_mass,
            gas_status,
            config.active_particles,
            config.target_particle_count,
            compute_badge,
            phase_mgr.planet_count,
            phase_mgr.protoplanet_count,
            phase_mgr.asteroid_count,
            phase_mgr.comet_count,
            belt_line,
            active_goal,
            lhb_info,
        ),
    );
}

fn update_time_warp_diagnostics(
    text: &mut Mut<'_, Text>,
    time_warp: &TimeWarp,
    player_state: &PlayerInteractionState,
    energy_monitor: &EnergyMonitor,
    config: &SimulationConfig,
    sim_time: &SimTime,
) {
    let speed_str = time_warp.human_readable_speed();
    let clock_str = sim_time.format_visual_clock();
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

    set_text_if_changed(
        text,
        format!(
            "CLOCK: {}\nSPEED: {}\nTOOL: {}\nOVERLAY: {} [V]\nSize Scale: {:.2}x [,/Shift+.]\nCompute: {} [F8]\nAccretion: Active (Boost: {:.0}x)\nEnergy Drift: {:.4}%\nSim Steps: {}",
            clock_str,
            speed_str,
            tool_str,
            player_state.overlay_mode.display_name(),
            config.size_exaggeration,
            if config.enable_gpu_compute {
                format!("WGPU Compute ({}k)", config.target_particle_count / 1000)
            } else {
                "CPU Multithread".to_string()
            },
            config.accretion_rate_multiplier,
            drift_pct,
            sim_time.step_count,
        ),
    );
}

fn update_bottom_timer(
    text: &mut Mut<'_, Text>,
    is_compact_pill: bool,
    sim_time: &SimTime,
    time_warp: &TimeWarp,
) {
    let clock_str = sim_time.format_visual_clock();
    let speed_str = time_warp.human_readable_speed();
    if is_compact_pill {
        set_text_if_changed(text, format!("⏱️ {clock_str}  [{speed_str}]  ▲"));
        return;
    }

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
        } else if total_sec < 86400.0 * 3.5 {
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

    set_text_if_changed(
        text,
        format!(
            "⏱️ CLOCK: {clock_str}  ({time_formatted})\nSPEED: {speed_str} | STATUS: {status_str}"
        ),
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Telemetry inspector requires full access to celestial body components and configuration state"
)]
fn update_inspector_body_telemetry(
    text: &mut Mut<'_, Text>,
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
    disk_hud_query: &Query<&CircumstellarDisk>,
    neutron_oort_hud_query: &Query<(
        Option<&PulsarGlitchState>,
        Option<&MagnetarCrustStress>,
        Option<&OortComet>,
    )>,
    phase_mgr: &PhaseManager,
    config: &SimulationConfig,
    visual_time: f32,
    audio_enabled: bool,
    show_atmospheres: bool,
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
            opt_sat,
        ),
    )) = bodies_query.get(selected_entity)
    else {
        set_text_str_if_changed(text, "Selected body was absorbed in an accretion merger.");
        return;
    };

    let (rel_pos, rel_vel, primary_mass) = opt_sat
        .and_then(|sat| bodies_query.get(sat.parent).ok())
        .map_or(
            (pos.0, vel.0, phase_mgr.star_mass),
            |((p_pos, p_vel, p_mass, ..), _)| (pos.0 - p_pos.0, vel.0 - p_vel.0, p_mass.0),
        );

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
        comp,
        body.body_type,
        config,
    );

    let comp_str = format_composition_line(
        comp,
        opt_vol,
        opt_climate,
        temp.0,
        body.body_type.is_star_or_remnant(),
    );

    let mut next_telemetry = format_primary_telemetry(
        rel_pos,
        rel_vel,
        mass.0,
        rad.0,
        temp.0,
        comp,
        body,
        opt_spin,
        primary_mass,
        &env_str,
        &comp_str,
    );
    append_stripped_core_view_note(
        &mut next_telemetry,
        mass.0,
        rad.0,
        body.body_type,
        comp,
        opt_diff,
        show_atmospheres,
    );

    append_inspector_domain_telemetry(
        &mut next_telemetry,
        selected_entity,
        body,
        opt_rel,
        quasi_hud_query,
        jet_hud_query,
        space_weather_hud_query,
        probes_hud_query,
        disk_hud_query,
        neutron_oort_hud_query,
        visual_time,
        audio_enabled,
    );

    set_text_if_changed(text, next_telemetry);
}

#[allow(
    clippy::too_many_arguments,
    reason = "Appends domain-specific telemetry blocks"
)]
fn append_inspector_domain_telemetry(
    next_telemetry: &mut String,
    selected_entity: Entity,
    body: &CelestialBody,
    opt_rel: Option<&crate::simulation::RelativisticState>,
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
    disk_hud_query: &Query<&CircumstellarDisk>,
    neutron_oort_hud_query: &Query<(
        Option<&PulsarGlitchState>,
        Option<&MagnetarCrustStress>,
        Option<&OortComet>,
    )>,
    visual_time: f32,
    audio_enabled: bool,
) {
    if let Ok(qs) = quasi_hud_query.get(selected_entity) {
        append_quasi_star_telemetry(next_telemetry, qs);
    }

    if let Ok(jet) = jet_hud_query.get(selected_entity) {
        super::inspector_panel::append_relativistic_jet_telemetry(next_telemetry, jet);
    }

    if let Ok((opt_aurora, opt_flare)) = space_weather_hud_query.get(selected_entity)
        && (opt_aurora.is_some() || opt_flare.is_some())
    {
        let is_giant = body.body_type == BodyType::GasGiant || body.body_type == BodyType::IceGiant;
        super::inspector_panel::append_space_weather_telemetry(
            next_telemetry,
            opt_aurora,
            opt_flare,
            is_giant,
        );
    }

    if let Ok((probe, comp)) = probes_hud_query.get(selected_entity) {
        append_probe_telemetry(next_telemetry, probe, comp);
    }

    if let Ok(disk) = disk_hud_query.get(selected_entity) {
        super::inspector_panel::append_circumstellar_disk_telemetry(next_telemetry, disk);
    }

    let (opt_glitch, opt_stress) = if let Ok((opt_glitch, opt_stress, opt_comet)) =
        neutron_oort_hud_query.get(selected_entity)
    {
        if let Some(glitch) = opt_glitch {
            append_pulsar_glitch_telemetry(next_telemetry, glitch);
        }
        if let Some(stress) = opt_stress {
            append_magnetar_stress_telemetry(next_telemetry, stress);
        }
        if let Some(comet) = opt_comet {
            append_oort_comet_telemetry(next_telemetry, comet);
        }
        (opt_glitch, opt_stress)
    } else {
        (None, None)
    };

    let opt_aurora = space_weather_hud_query
        .get(selected_entity)
        .ok()
        .and_then(|(opt_aurora, _)| opt_aurora);
    let sonification = crate::game::audio::compute_body_sonification_snapshot(
        body,
        opt_rel,
        opt_glitch,
        opt_stress,
        opt_aurora,
        visual_time,
        audio_enabled,
    );
    append_sonification_telemetry(next_telemetry, &sonification);
}

/// Updates the dynamic content of the HUD and notification toast banner every frame.
#[allow(
    clippy::type_complexity,
    reason = "HUD update requires access to a wide range of celestial body components and simulation state"
)]
pub fn update_hud(
    (
        time,
        sim_time,
        time_warp,
        config,
        energy_monitor,
        phase_mgr,
        lhb_state,
        opt_scenario,
        opt_audio,
    ): (
        Res<Time>,
        Res<SimTime>,
        Res<TimeWarp>,
        Res<SimulationConfig>,
        Res<EnergyMonitor>,
        Res<PhaseManager>,
        Res<crate::game::phases::LateHeavyBombardmentState>,
        Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
        Option<Res<crate::game::audio::CosmicAudioState>>,
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
    disk_hud_query: Query<&CircumstellarDisk>,
    neutron_oort_hud_query: Query<(
        Option<&PulsarGlitchState>,
        Option<&MagnetarCrustStress>,
        Option<&OortComet>,
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
        update_header_stats(
            &mut text,
            &sim_time,
            &phase_mgr,
            &config,
            &lhb_state,
            opt_scenario.as_deref(),
        );
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
    for (mut text, marker) in &mut text_queries.bottom_timer {
        update_bottom_timer(&mut text, marker.0, &sim_time, &time_warp);
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
                &disk_hud_query,
                &neutron_oort_hud_query,
                &phase_mgr,
                &config,
                sim_time.visual_time_secs,
                opt_audio.as_deref().is_none_or(|a| a.enabled),
                player_state.show_atmospheres(),
            );
        } else {
            set_text_str_if_changed(
                &mut text,
                "No celestial body selected.\nClick on the Star or Planets above (or in 3D) to inspect & live-edit.\n[Tab] Next Body | [F] Focus Target | [WASD] Free-Fly View",
            );
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
    text: &mut Mut<'_, Text>,
    opt_predictor: Option<&crate::simulation::predictor::TrajectoryPredictorState>,
    has_selection: bool,
) {
    let Some(predictor) = opt_predictor else {
        set_text_str_if_changed(text, "🎯 FORECAST: Initializing...");
        return;
    };

    if !predictor.is_enabled {
        set_text_str_if_changed(
            text,
            "🎯 FORECAST: Disabled [N] to enable trajectory & encounter prediction.",
        );
        return;
    }

    if let Some(enc) = &predictor.active_encounter {
        set_text_if_changed(
            text,
            format!(
                "🎯 FORECAST: {} with {}\nETA: {:.1} yr | Closest: {:.4} AU ({:.0} km)\nRel Speed: {:.2} km/s (v_inf)",
                enc.encounter_type.display_label(),
                enc.target_name,
                enc.time_to_encounter_yr,
                enc.min_distance_au,
                enc.min_distance_km,
                enc.relative_velocity_kms,
            ),
        );
    } else if has_selection {
        set_text_str_if_changed(
            text,
            "🎯 FORECAST: Safe trajectory. No close encounters detected (Horizon: 30 yr).",
        );
    } else {
        set_text_str_if_changed(
            text,
            "🎯 FORECAST [N]: Select a body or drag Slingshot to preview encounters.",
        );
    }
}
