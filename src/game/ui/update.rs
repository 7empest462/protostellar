//! Systems for updating telemetry, timer banners, inspector readouts, and HUD visibility.

use bevy::prelude::*;
use std::fmt::Write;

use crate::game::phases::PhaseManager;
use crate::simulation::accretion::RocheDisruptionEvent;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

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

fn update_toast_text(
    toast_text: &mut Text,
    phase_mgr: &PhaseManager,
    toast: &NotificationToast,
    player_state: &PlayerInteractionState,
    bodies_query: &Query<(
        (
            &SimPosition,
            &SimVelocity,
            &Mass,
            &Radius,
            &Temperature,
            &Composition,
            &CelestialBody,
        ),
        (
            Option<&InternalDifferentiation>,
            Option<&SpinState>,
            Option<&IgnitionState>,
            Option<&VolatileInventory>,
            Option<&PlanetaryRingSystem>,
            Option<&AtmosphericEscapeTail>,
            Option<&PlanetaryClimate>,
            Option<&BiosphereState>,
            Option<&StellarEvolutionState>,
        ),
    )>,
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
            let type_name = match body.body_type {
                BodyType::Protostar => "THE STAR (Protostar)",
                BodyType::MainSequenceStar => "THE STAR (Main Sequence)",
                BodyType::BrownDwarf => "BROWN DWARF (Sub-Stellar)",
                BodyType::RedDwarf => "RED DWARF STAR (M-Type)",
                BodyType::YellowDwarf => "YELLOW DWARF STAR (G2V)",
                BodyType::BlueGiant => "BLUE GIANT STAR (B-Type)",
                BodyType::BlueSupergiant => "BLUE SUPERGIANT (O-Type)",
                BodyType::RedGiant => "RED GIANT STAR",
                BodyType::RedSupergiant => "RED SUPERGIANT STAR",
                BodyType::Hypergiant => "LUMINOUS HYPERGIANT",
                BodyType::WolfRayet => "WOLF-RAYET STAR",
                BodyType::WhiteDwarf => "WHITE DWARF REMNANT",
                BodyType::NeutronStar => "NEUTRON STAR REMNANT",
                BodyType::Pulsar => "RELATIVISTIC PULSAR",
                BodyType::Magnetar => "MAGNETAR REMNANT",
                BodyType::BlackHole => "STELLAR-MASS BLACK HOLE",
                BodyType::QuasiStar => "QUASI-STAR / BLACK HOLE STAR (JWST LITTLE RED DOT)",
                BodyType::GasGiant => "GAS GIANT",
                BodyType::IceGiant => "ICE GIANT",
                BodyType::SuperEarth => "SUPER-EARTH",
                BodyType::TerrestrialPlanet => "TERRESTRIAL PLANET",
                BodyType::Protoplanet => "PROTOPLANETARY EMBRYO",
                BodyType::Planetesimal => "PLANETESIMAL",
                BodyType::Asteroid => "ASTEROID",
                BodyType::Comet => "COMET",
                BodyType::DustGrain => "DUST GRAIN",
                BodyType::DebrisRing => "DEBRIS RING",
                BodyType::Moon => "NATURAL MOON / SATELLITE",
            };
            let mass_str = if mass.0 >= 0.01 {
                format!(
                    "{:.2} M_sun ({:.1} M_J)",
                    mass.0,
                    mass.0 / JUPITER_MASS_SOLAR
                )
            } else {
                format!("{:.2} M_earth", mass.0 / EARTH_MASS_SOLAR)
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
            let rad_km = (rad.0 * AU_TO_KM).max(1.0);

            toast_text.0 = format!(
                ">> SELECTED: {} [{}]  |  Mass: {}  |  Radius: {:.0} km  |  Dist: {:.2} AU  |  Speed: {:.1} km/s  |  Temp: {:.0} K",
                body.name.to_uppercase(),
                type_name,
                mass_str,
                rad_km,
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

    text.0 = format!(
        "Phase: {}\nTime: T + {} | Star: {:.2} M_sun\nSwarm: {} / {} particles [{}] | Gas: {}\nPlanets: {} | Embryos: {} | 🪨 Asteroids: {} | ☄️ Comets: {}{}\nGoal: {}",
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
        lhb_info,
        active_goal,
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

fn format_body_inspector_type(body_type: BodyType) -> &'static str {
    match body_type {
        BodyType::Protostar => "Central Star (Protostar)",
        BodyType::MainSequenceStar => "Main Sequence Star",
        BodyType::BrownDwarf => "Brown Dwarf (Sub-Stellar)",
        BodyType::RedDwarf => "Red Dwarf Star (M-Type)",
        BodyType::YellowDwarf => "Yellow Dwarf Star (G2V)",
        BodyType::BlueGiant => "Blue Giant Star (B-Type)",
        BodyType::BlueSupergiant => "Blue Supergiant Star (O-Type)",
        BodyType::RedGiant => "Red Giant Star",
        BodyType::RedSupergiant => "Red Supergiant Star",
        BodyType::Hypergiant => "Luminous Hypergiant",
        BodyType::WolfRayet => "Wolf-Rayet Star",
        BodyType::WhiteDwarf => "White Dwarf Remnant",
        BodyType::NeutronStar => "Neutron Star Remnant",
        BodyType::Pulsar => "Relativistic Pulsar Remnant",
        BodyType::Magnetar => "Magnetar Remnant",
        BodyType::BlackHole => "Stellar-Mass Black Hole",
        BodyType::QuasiStar => "Quasi-Star / Black Hole Star (JWST Little Red Dot)",
        BodyType::GasGiant => "Gas Giant Planet",
        BodyType::IceGiant => "Ice Giant Planet",
        BodyType::SuperEarth => "Super-Earth Planet",
        BodyType::TerrestrialPlanet => "Terrestrial Planet",
        BodyType::Protoplanet => "Protoplanetary Embryo",
        BodyType::Planetesimal => "Planetesimal",
        BodyType::Asteroid => "Asteroid",
        BodyType::Comet => "Comet",
        BodyType::DustGrain => "Dust Grain",
        BodyType::DebrisRing => "Debris Ring",
        BodyType::Moon => "Natural Moon / Satellite",
    }
}

fn format_stellar_extra_telemetry(
    opt_ignition: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
    rad: &Radius,
    temp: &Temperature,
    config: &SimulationConfig,
) -> String {
    let Some(ignition) = opt_ignition else {
        return String::new();
    };

    let core_temp_mk = ignition.core_temperature / 1.0e6;
    let fusion_pct = ignition.fusion_fraction * 100.0;
    let evo_str = if let Some(evo) = opt_evo {
        match evo.phase {
            StellarEvolutionPhase::ProtostarContraction => {
                format!(
                    "Hayashi Track Contraction (Fuel: {:.0}% H)",
                    evo.hydrogen_core_fraction * 100.0
                )
            }
            StellarEvolutionPhase::MainSequence => {
                format!(
                    "Stable Main Sequence (Core Fuel: {:.1}% H)",
                    evo.hydrogen_core_fraction * 100.0
                )
            }
            StellarEvolutionPhase::RedGiantBranch => {
                format!(
                    "RED GIANT BRANCH (R: {:.2} AU | L: {:.0} L☉ | Engulfing Inner Planets)",
                    rad.0,
                    (rad.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4)
                )
            }
            StellarEvolutionPhase::RedSupergiantBranch => {
                format!(
                    "RED SUPERGIANT BRANCH (R: {:.2} AU | Massive Core Burning)",
                    rad.0
                )
            }
            StellarEvolutionPhase::SupernovaExplosion => {
                format!(
                    "💥 SUPERNOVA CORE-COLLAPSE (Blast: {:.1} AU @ 15,000 km/s)",
                    evo.nebula_expansion_radius_au
                )
            }
            StellarEvolutionPhase::HeliumFlashAgb => {
                format!(
                    "AGB SUPERGIANT (Core He Fuel: {:.1}% | R: {:.2} AU)",
                    evo.helium_core_fraction * 100.0,
                    rad.0
                )
            }
            StellarEvolutionPhase::PlanetaryNebulaEjection => {
                format!(
                    "PLANETARY NEBULA EJECTION (Shell: {:.1} AU | Shedding Envelope Mass)",
                    evo.nebula_expansion_radius_au
                )
            }
            StellarEvolutionPhase::WhiteDwarf => {
                format!(
                    "DEGENERATE WHITE DWARF REMNANT (Earth-Sized Core | T: {:.0} K | B: 10^6 G)",
                    temp.0
                )
            }
            StellarEvolutionPhase::NeutronStarPulsar => {
                "⚡ NEUTRON STAR / PULSAR REMNANT (B: 10^12 G | Synchrotron Lighthouse Jets)"
                    .to_string()
            }
            StellarEvolutionPhase::MagnetarRemnant => {
                "🧲 MAGNETAR REMNANT (B: 10^15 G | Extreme Magnetic Reconnection Arcs)".to_string()
            }
            StellarEvolutionPhase::BlackHoleRemnant => {
                "🕳️ STELLAR-MASS BLACK HOLE (Event Horizon & Relativistic Accretion Disk)"
                    .to_string()
            }
        }
    } else {
        "Active Hydrogen Fusion".to_string()
    };

    let status = if ignition.is_ignited {
        format!(
            "{}\nSolar Wind Shockwave: {:.2} AU | Gas Dispersal: {:.0}%",
            evo_str,
            ignition.shockwave_radius,
            (1.0 - config.gas_density_scale) * 100.0
        )
    } else {
        format!("Kelvin-Helmholtz Core Heating (Progress: {fusion_pct:.1}%)\nIgnition Threshold: 10.0 MK [Press 'I' or Click Button Below to Ignite]")
    };

    format!("\nStellar Core Temp: {core_temp_mk:.2} MK | Fusion: {fusion_pct:.1}%\nStellar State: {status}")
}

#[allow(
    clippy::too_many_arguments,
    reason = "Formatted telemetry display incorporates all astrophysical state components into a cohesive readout"
)]
fn format_body_environment_telemetry(
    opt_diff: Option<&InternalDifferentiation>,
    opt_vol: Option<&VolatileInventory>,
    opt_rings: Option<&PlanetaryRingSystem>,
    opt_tail: Option<&AtmosphericEscapeTail>,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_ignition: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
    rad: &Radius,
    temp: &Temperature,
    config: &SimulationConfig,
) -> String {
    let mut out = String::new();

    if let Some(diff) = opt_diff {
        if diff.is_differentiated {
            let core_km = diff.core_radius_au * AU_TO_KM;
            let mantle_km = (diff.mantle_radius_au - diff.core_radius_au).max(0.0) * AU_TO_KM;
            let crust_km = diff.crust_thickness_au * AU_TO_KM;
            let _ = write!(
                out,
                "\nStructure: Differentiated (Core: {:.0} km | Mantle: {:.0} km | Crust: {:.0} km)\nDynamo: {:.2} G | Core Temp: {:.0} K",
                core_km, mantle_km, crust_km, diff.magnetic_field_gauss, diff.core_temp_k
            );
        } else {
            out.push_str("\nStructure: Undifferentiated Chondritic Mixture");
        }
    }

    if let Some(vol) = opt_vol {
        let _ = write!(
            out,
            "\nVolatiles: {:.4} M_earth Water Delivered | Ocean Coverage: {:.0}%\nAtmospheric Pressure: {:.2} bar | Icy Bombardment Impacts: {}",
            vol.delivered_water_m_earth,
            vol.ocean_coverage_frac * 100.0,
            vol.atmospheric_pressure_bar,
            vol.cometary_impact_count
        );
    }

    if let Some(ring) = opt_rings {
        let inner_km = f64::from(ring.inner_radius_au) * AU_TO_KM;
        let outer_km = f64::from(ring.outer_radius_au) * AU_TO_KM;
        let _ = write!(
            out,
            "\nRing System: Active (Span: {:.0} - {:.0} km | Opacity: {:.0}% | {:.0}% Ice)",
            inner_km,
            outer_km,
            ring.optical_depth * 100.0,
            ring.ice_fraction * 100.0
        );
    }

    if let Some(tail) = opt_tail {
        if tail.is_active && tail.tail_length_au > 0.01 {
            let _ = write!(
                out,
                "\nPhotoevaporation: Active (Loss: {:.2} M_earth/Myr | Tail: {:.2} AU)",
                tail.loss_rate_m_earth_per_myr, tail.tail_length_au
            );
        }
    }

    if let Some(climate) = opt_climate {
        let regime_name = match climate.climate_regime {
            ClimateRegime::SnowballIceAge => "Frozen Snowball (Ice Age)",
            ClimateRegime::TemperateHabitable => "Temperate Habitable",
            ClimateRegime::RunawayVenusian => "Runaway Greenhouse (Venusian)",
            ClimateRegime::GasGiantEnvelope => "Gas Giant Envelope",
            ClimateRegime::AirlessVacuum => "Airless Vacuum",
        };
        let _ = write!(
            out,
            "\nClimate: {} (T_surf: {:.0} K | Albedo: {:.2} | Greenhouse: +{:.0} K)",
            regime_name, climate.surface_temperature_k, climate.albedo, climate.greenhouse_delta_k
        );
    }

    if let Some(bio) = opt_bio {
        let status = if bio.biomass_coverage_frac >= 0.50 {
            "Thriving Eden"
        } else if bio.biomass_coverage_frac >= 0.05 {
            "Colonizing Biosphere"
        } else if bio.habitability_score >= 0.40 {
            "Pre-Biotic Prime"
        } else {
            "Sterile / Hostile"
        };
        let _ = write!(
            out,
            "\nBiosphere: {} (Biomass: {:.0}% | O2: {:.1}% | Habitability: {:.0}%)",
            status,
            bio.biomass_coverage_frac * 100.0,
            bio.oxygen_fraction * 100.0,
            bio.habitability_score * 100.0
        );
    }

    out.push_str(&format_stellar_extra_telemetry(
        opt_ignition,
        opt_evo,
        rad,
        temp,
        config,
    ));
    out
}

#[allow(
    clippy::too_many_arguments,
    reason = "Telemetry inspector requires full access to celestial body components and configuration state"
)]
fn update_inspector_body_telemetry(
    text: &mut Text,
    selected_entity: Entity,
    bodies_query: &Query<(
        (
            &SimPosition,
            &SimVelocity,
            &Mass,
            &Radius,
            &Temperature,
            &Composition,
            &CelestialBody,
        ),
        (
            Option<&InternalDifferentiation>,
            Option<&SpinState>,
            Option<&IgnitionState>,
            Option<&VolatileInventory>,
            Option<&PlanetaryRingSystem>,
            Option<&AtmosphericEscapeTail>,
            Option<&PlanetaryClimate>,
            Option<&BiosphereState>,
            Option<&StellarEvolutionState>,
        ),
    )>,
    quasi_hud_query: &Query<&BlackHoleStarState>,
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
        ),
    )) = bodies_query.get(selected_entity)
    else {
        text.0 = "Selected body was absorbed in an accretion merger.".to_string();
        return;
    };

    let dist_au = if pos.0.is_finite() {
        pos.0.length()
    } else {
        0.0
    };
    let speed_au_yr = if vel.0.is_finite() {
        vel.0.length()
    } else {
        0.0
    };
    let speed_km_s = speed_au_yr * AU_PER_YR_TO_KM_PER_S;

    let mass_str = if mass.0 >= 10_000.0 {
        format!("{:.0} M☉ (Supermassive Seed)", mass.0)
    } else if mass.0 >= 0.01 {
        format!(
            "{:.3} M_sun ({:.1} M_J)",
            mass.0,
            mass.0 / JUPITER_MASS_SOLAR
        )
    } else {
        format!(
            "{:.2} M_earth ({:.4} M_sun)",
            mass.0 / EARTH_MASS_SOLAR,
            mass.0
        )
    };

    let radius_km = rad.0 * AU_TO_KM;
    let density_g_cm3 = (comp.average_density() * SOLAR_MASS_KG / (AU_TO_METERS.powi(3) * 1000.0))
        .clamp(0.01, 20.0);

    let period_str = if dist_au > 0.05 && !body.body_type.is_star_or_remnant() {
        let p_yr = dist_au.powf(1.5) / phase_mgr.star_mass.max(0.1).sqrt();
        if p_yr >= 1.0 {
            format!(" | Period: {p_yr:.2} yr")
        } else {
            format!(" | Period: {:.1} days", p_yr * 365.25)
        }
    } else {
        String::new()
    };

    let spin_str = if let Some(spin) = opt_spin {
        format!(
            " | Day: {:.1}h | Tilt: {:.1} deg",
            spin.rotation_period_hours, spin.axial_tilt_degrees
        )
    } else {
        String::new()
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
        rad,
        temp,
        config,
    );

    let norm = comp.normalized();
    let rock_pct = ((norm.silicate_frac + norm.organics_frac) * 100.0).round();
    let ice_pct = (norm.ice_frac * 100.0).round();
    let metal_pct = (norm.metal_frac * 100.0).round();
    let gas_pct = (100.0f64 - rock_pct - ice_pct - metal_pct).max(0.0);

    text.0 = format!(
        ">> {} [{}]\nMass: {}\nRadius: {:.0} km ({:.4} AU)\nDensity: {:.2} g/cm3 | Temp: {:.0} K{}{}\nDistance from Star: {:.2} AU | Speed: {:.1} km/s\nComposition: {:.0}% Rock | {:.0}% Ice | {:.0}% Metal | {:.0}% Gas{}",
        body.name.to_uppercase(),
        format_body_inspector_type(body.body_type).to_uppercase(),
        mass_str,
        radius_km,
        rad.0,
        density_g_cm3,
        temp.0,
        spin_str,
        period_str,
        dist_au,
        speed_km_s,
        rock_pct,
        ice_pct,
        metal_pct,
        gas_pct,
        env_str,
    );

    if let Ok(qs) = quasi_hud_query.get(selected_entity) {
        let acc_mode = if qs.super_eddington_active {
            "SUPER-EDDINGTON (4.5x)"
        } else {
            "SUB-EDDINGTON (0.9x)"
        };
        let status = if qs.is_blown_out {
            format!(
                "QUASAR TRANSITION (Progress: {:.0}%)",
                qs.blowout_progress * 100.0
            )
        } else {
            format!("HYDROGEN COCOON INTACT ({:.0} AU)", qs.cocoon_radius_au)
        };
        let _ = write!(
            text.0,
            "\n--------------------------------------------------\n  >> JWST LITTLE RED DOT / QUASI-STAR <<\n--------------------------------------------------\n  • BH Seed Mass:     {:>10.0} M☉\n  • Cocoon Mass:      {:>10.0} M☉\n  • Inflow Rate:      {:>10.1}x ({})\n  • Cocoon Status:    {}\n  • Redshift Epoch:   z ≈ 8.5 (Cosmic Dawn, 660 Myr)\n  • Controls:         [X] Accrete | [B] Blowout | [T] Pop-III TDE\n--------------------------------------------------",
            qs.black_hole_mass_solar,
            qs.cocoon_mass_solar,
            qs.eddington_ratio,
            acc_mode,
            status,
        );
    }
}

/// Updates the dynamic content of the HUD and notification toast banner every frame.
pub fn update_hud(
    time: Res<Time>,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    config: Res<SimulationConfig>,
    energy_monitor: Res<EnergyMonitor>,
    phase_mgr: Res<PhaseManager>,
    lhb_state: Res<crate::game::phases::LateHeavyBombardmentState>,
    player_state: Res<PlayerInteractionState>,
    mut toast: ResMut<NotificationToast>,
    bodies_query: Query<(
        (
            &SimPosition,
            &SimVelocity,
            &Mass,
            &Radius,
            &Temperature,
            &Composition,
            &CelestialBody,
        ),
        (
            Option<&InternalDifferentiation>,
            Option<&SpinState>,
            Option<&IgnitionState>,
            Option<&VolatileInventory>,
            Option<&PlanetaryRingSystem>,
            Option<&AtmosphericEscapeTail>,
            Option<&PlanetaryClimate>,
            Option<&BiosphereState>,
            Option<&StellarEvolutionState>,
        ),
    )>,
    quasi_hud_query: Query<&BlackHoleStarState>,
    mut header_query: Query<
        &mut Text,
        (
            With<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
        ),
    >,
    mut time_query: Query<
        &mut Text,
        (
            With<HudTimeWarpText>,
            Without<HudHeaderStatsText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
        ),
    >,
    mut inspector_query: Query<
        &mut Text,
        (
            With<HudInspectorText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
        ),
    >,
    mut toast_query: Query<
        &mut Text,
        (
            With<HudToastText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudBottomTimerText>,
        ),
    >,
    mut bottom_timer_query: Query<
        &mut Text,
        (
            With<HudBottomTimerText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
        ),
    >,
) {
    if toast.timer > 0.0 {
        toast.timer -= time.delta_secs();
    }
    if let Ok(mut toast_text) = toast_query.single_mut() {
        update_toast_text(
            &mut toast_text,
            &phase_mgr,
            &toast,
            &player_state,
            &bodies_query,
        );
    }

    if let Ok(mut text) = header_query.single_mut() {
        update_header_stats(&mut text, &sim_time, &phase_mgr, &config, &lhb_state);
    }

    if let Ok(mut text) = time_query.single_mut() {
        update_time_warp_diagnostics(
            &mut text,
            &time_warp,
            &player_state,
            &energy_monitor,
            &config,
            &sim_time,
        );
    }

    if let Ok(mut text) = bottom_timer_query.single_mut() {
        update_bottom_timer(&mut text, &sim_time, &time_warp);
    }

    if let Ok(mut text) = inspector_query.single_mut() {
        if let Some(selected_entity) = player_state.selected_entity {
            update_inspector_body_telemetry(
                &mut text,
                selected_entity,
                &bodies_query,
                &quasi_hud_query,
                &phase_mgr,
                &config,
            );
        } else {
            text.0 = "No celestial body selected.\nClick on the Star or Planets above (or in 3D) to inspect & live-edit.\n[Tab] Next Body | [F] Focus Target | [WASD] Free-Fly View".to_string();
        }
    }
}

/// Synchronizes visibility for collapsible HUD panels and master full-screen view mode.
pub fn update_hud_visibility(
    hud_visibility: Res<HudVisibilityState>,
    player_state: Res<PlayerInteractionState>,
    mut panels_query: Query<(&mut Node, &HudPanelElement)>,
    mut text_query: Query<(&mut Text, &HudDynamicText)>,
    names_query: Query<&CelestialBody>,
) {
    for (mut node, element) in panels_query.iter_mut() {
        match element {
            HudPanelElement::RootContainer | HudPanelElement::OrbitModeBadge => {
                node.display = if hud_visibility.is_full_screen_clean {
                    Display::None
                } else {
                    Display::Flex
                };
            }
            HudPanelElement::TopLeftPanel => {
                node.display = if hud_visibility.top_left_minimized {
                    Display::None
                } else {
                    Display::Flex
                };
            }
            HudPanelElement::TopLeftPill => {
                node.display = if hud_visibility.top_left_minimized {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudPanelElement::TopRightPanel => {
                node.display = if hud_visibility.top_right_minimized {
                    Display::None
                } else {
                    Display::Flex
                };
            }
            HudPanelElement::TopRightPill => {
                node.display = if hud_visibility.top_right_minimized {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudPanelElement::InspectorPanel => {
                node.display = if hud_visibility.inspector_minimized {
                    Display::None
                } else {
                    Display::Flex
                };
            }
            HudPanelElement::InspectorChip => {
                node.display = if hud_visibility.inspector_minimized {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudPanelElement::ScenarioPresets => {
                node.display = if hud_visibility.scenarios_minimized {
                    Display::None
                } else {
                    Display::Flex
                };
            }
        }
    }

    for (mut text, dynamic_text) in text_query.iter_mut() {
        match dynamic_text {
            HudDynamicText::FullScreenBadge => {
                text.0 = if hud_visibility.is_full_screen_clean {
                    "👁️ Show HUD [F11]".to_string()
                } else {
                    "⛶ Fullscreen [F11]".to_string()
                };
            }
            HudDynamicText::OrbitModeBadge => {
                text.0 = format!("궤 Orbits: {} [Y]", player_state.orbit_mode.display_label());
            }
            HudDynamicText::InspectorChip => {
                if let Some(target) = player_state.selected_entity {
                    if let Ok(body) = names_query.get(target) {
                        text.0 = format!("🔍 Inspector: {} ▲ Expand", body.name);
                    } else {
                        text.0 = "🔍 Inspector ▲ Expand".to_string();
                    }
                } else {
                    text.0 = "🔍 Inspector (No Selection) ▲ Expand".to_string();
                }
            }
        }
    }
}
