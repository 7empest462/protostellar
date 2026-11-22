//! Stellar lifecycle transitions, ignition, and space weather flare triggers.

use bevy::prelude::*;

use crate::game::ui::types::*;
use crate::simulation::components::*;
use crate::simulation::thermodynamics::{
    attach_black_hole_remnant_components, the_star_display_name_for_type,
};
use crate::utils::constants::*;

/// Mutable ECS component bundle passed when advancing a single star's evolutionary stage.
pub struct StarAgingTargets<'a> {
    pub entity: Entity,
    pub mass: &'a mut Mass,
    pub radius: &'a mut Radius,
    pub body: &'a mut CelestialBody,
    pub comp: &'a Composition,
    pub ignition: &'a mut IgnitionState,
    pub evo: Option<&'a mut StellarEvolutionState>,
    pub temp: Option<&'a mut Temperature>,
    pub lum: Option<&'a mut Luminosity>,
}

fn compute_supernova_remnant_mass(progenitor_mass: f64, remnant_type: BodyType) -> f64 {
    if remnant_type == BodyType::BlackHole {
        if progenitor_mass >= 250.0 {
            progenitor_mass * 0.92
        } else if progenitor_mass >= 100.0 {
            progenitor_mass * 0.75
        } else if progenitor_mass >= 40.0 {
            progenitor_mass * 0.55
        } else {
            (progenitor_mass * 0.35).clamp(3.5, 25.0)
        }
    } else if remnant_type == BodyType::Magnetar {
        1.65
    } else {
        1.44
    }
}

fn insert_remnant_em_field(
    commands: &mut Option<&mut Commands>,
    entity: Entity,
    remnant_type: BodyType,
) {
    let Some(cmds) = commands.as_deref_mut() else {
        return;
    };
    let Ok(mut cmd) = cmds.get_entity(entity) else {
        return;
    };
    let em = match remnant_type {
        BodyType::BlackHole => ElectromagneticFieldState {
            magnetic_field_gauss: 1.0e8,
            rotation_period_sec: 0.001,
            magnetic_inclination_rad: 0.15,
            jet_length_au: 6.0,
            synchrotron_intensity: 3.0,
        },
        BodyType::Magnetar => ElectromagneticFieldState {
            magnetic_field_gauss: 1.0e15,
            rotation_period_sec: 0.08,
            magnetic_inclination_rad: 0.45,
            jet_length_au: 6.0,
            synchrotron_intensity: 2.5,
        },
        BodyType::QuarkStar => ElectromagneticFieldState {
            magnetic_field_gauss: 1.0e13,
            rotation_period_sec: 0.015,
            magnetic_inclination_rad: 0.35,
            jet_length_au: 4.5,
            synchrotron_intensity: 2.2,
        },
        _ => ElectromagneticFieldState {
            magnetic_field_gauss: 1.0e12,
            rotation_period_sec: 0.033,
            magnetic_inclination_rad: 0.35,
            jet_length_au: 3.5,
            synchrotron_intensity: 1.8,
        },
    };
    cmd.try_insert(em);
}

fn apply_compact_remnant_state(
    targets: &mut StarAgingTargets<'_>,
    remnant_type: BodyType,
    remnant_mass: f64,
    commands: &mut Option<&mut Commands>,
) {
    targets.mass.0 = remnant_mass;
    targets.body.body_type = remnant_type;
    targets.ignition.is_ignited = false;

    if remnant_type == BodyType::BlackHole {
        targets.radius.0 = (1.974e-8 * remnant_mass).max(1e-7);
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 10.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 0.0;
        }
        let bh_class = if remnant_mass >= 100.0 {
            "Intermediate-Mass Black Hole"
        } else {
            "Stellar-Mass Black Hole"
        };
        targets.body.name = format!("{bh_class} ({remnant_mass:.1} M☉)");
        if let Some(cmds) = commands.as_deref_mut() {
            attach_black_hole_remnant_components(cmds, targets.entity, remnant_mass);
        }
    } else if remnant_type == BodyType::Magnetar {
        targets.radius.0 = 0.0001;
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 2_000_000.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 150.0;
        }
        targets.body.name = format!("Magnetar Remnant ({remnant_mass:.2} M☉)");
    } else {
        targets.radius.0 = 0.0001;
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 1_000_000.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 100.0;
        }
        targets.body.name = format!("Pulsar Remnant ({remnant_mass:.2} M☉)");
    }

    insert_remnant_em_field(commands, targets.entity, remnant_type);
}

fn detonate_supernova_transition(
    targets: &mut StarAgingTargets<'_>,
    commands: &mut Option<&mut Commands>,
    supernova_events: &mut Option<&mut MessageWriter<SupernovaEvent>>,
    toast: &mut NotificationToast,
    is_type_ia: bool,
) {
    let initial_mass = targets.mass.0;
    let is_bh = initial_mass >= 25.0 || (is_type_ia && initial_mass > TOV_LIMIT_SOLAR);
    let is_magnetar = !is_bh
        && (targets.body.name.contains("Fast")
            || targets.body.name.contains("Magnetar")
            || initial_mass >= 18.0);
    let remnant_type = if is_bh {
        BodyType::BlackHole
    } else if is_magnetar {
        BodyType::Magnetar
    } else {
        BodyType::Pulsar
    };
    let remnant_mass = if is_type_ia && !is_bh {
        1.40
    } else {
        compute_supernova_remnant_mass(initial_mass, remnant_type)
    };
    let ejected_mass = (initial_mass - remnant_mass).max(0.1);
    let initial_radius = targets.radius.0;
    let prev_name = targets.body.name.clone();

    if let Some(evo) = targets.evo.as_deref_mut() {
        evo.phase = StellarEvolutionPhase::SupernovaExplosion;
        evo.phase_timer_years = 0.0;
        let base_r = if is_bh { 5.0_f32 } else { 4.0_f32 };
        evo.nebula_expansion_radius_au = base_r.max((initial_radius * 1.2) as f32);
        evo.nebula_opacity = 1.0;
        targets.ignition.shockwave_radius = f64::from(evo.nebula_expansion_radius_au);
    } else {
        targets.ignition.shockwave_radius = 4.0;
    }

    if let Some(writer) = supernova_events.as_deref_mut() {
        writer.write(SupernovaEvent {
            star_entity: targets.entity,
            star_name: prev_name,
            initial_mass_solar: initial_mass,
            remnant_mass_solar: remnant_mass,
            remnant_type,
            shockwave_velocity_km_s: if is_type_ia { 12_000.0 } else { 15_000.0 },
            ejected_metals_solar: ejected_mass * 0.25,
            ejected_composition: Composition::supernova_ejecta(),
        });
    }

    apply_compact_remnant_state(targets, remnant_type, remnant_mass, commands);

    toast.message = if is_type_ia && !is_bh {
        "💥 TYPE IA THERMONUCLEAR SUPERNOVA! White Dwarf exceeded 1.44 M☉ Chandrasekhar Limit!"
            .to_string()
    } else if is_bh {
        format!(
            "💥 HYPERNOVA DETONATION! Core collapsing into {} (Click Age to settle)!",
            targets.body.name
        )
    } else {
        format!(
            "💥 TYPE II CORE-COLLAPSE SUPERNOVA! Ejecting metals -> {} (Click Age to settle)!",
            targets.body.name
        )
    };
}

fn settle_white_dwarf_remnant(targets: &mut StarAgingTargets<'_>, helium_wd: bool) {
    if !helium_wd && targets.mass.0 > 1.20 {
        targets.mass.0 = (0.47 + 0.08 * targets.mass.0).clamp(0.50, 1.20);
    }
    if let Some(evo) = targets.evo.as_deref_mut() {
        evo.phase = StellarEvolutionPhase::WhiteDwarf;
        evo.phase_timer_years = 0.0;
        evo.nebula_opacity = 0.0;
        evo.envelope_mass_loss_rate = 0.0;
    }
    targets.ignition.is_ignited = false;
    targets.body.body_type = BodyType::WhiteDwarf;
    targets.body.name = if helium_wd {
        "The Star (Helium White Dwarf)".to_string()
    } else {
        "The Star (White Dwarf Remnant)".to_string()
    };
    targets.radius.0 = 0.009 * SOLAR_RADIUS_AU;
    let t_wd = if helium_wd { 25_000.0 } else { 30_000.0 };
    if let Some(t) = targets.temp.as_deref_mut() {
        t.0 = t_wd;
    }
    if let Some(l) = targets.lum.as_deref_mut() {
        l.0 = (targets.radius.0 / SOLAR_RADIUS_AU).powi(2) * (t_wd / 5778.0).powi(4);
    }
}

fn settle_supernova_remnant(
    targets: &mut StarAgingTargets<'_>,
    commands: &mut Option<&mut Commands>,
    toast: &mut NotificationToast,
) {
    let is_bh = targets.body.body_type == BodyType::BlackHole
        || targets.mass.0 >= 25.0
        || targets.mass.0 > TOV_LIMIT_SOLAR;
    let remnant_type = if is_bh {
        BodyType::BlackHole
    } else if targets.body.body_type == BodyType::Magnetar {
        BodyType::Magnetar
    } else {
        BodyType::Pulsar
    };
    let target_mass = if is_bh && targets.mass.0 >= 25.0 {
        compute_supernova_remnant_mass(targets.mass.0, BodyType::BlackHole)
    } else if remnant_type == BodyType::Magnetar {
        targets.mass.0.clamp(1.40, 1.80).max(1.65)
    } else if remnant_type == BodyType::Pulsar {
        targets.mass.0.clamp(1.10, 1.80).max(1.44)
    } else {
        targets.mass.0
    };
    if let Some(evo) = targets.evo.as_deref_mut() {
        evo.phase = match remnant_type {
            BodyType::BlackHole => StellarEvolutionPhase::BlackHoleRemnant,
            BodyType::Magnetar => StellarEvolutionPhase::MagnetarRemnant,
            _ => StellarEvolutionPhase::NeutronStarPulsar,
        };
        evo.phase_timer_years = 0.0;
        evo.nebula_opacity = 0.0;
    }
    apply_compact_remnant_state(targets, remnant_type, target_mass, commands);
    toast.message = match remnant_type {
        BodyType::BlackHole => format!(
            "🕳️ Singularity Settled: {} (Event Horizon & Kerr Accretion Disk)!",
            targets.body.name
        ),
        BodyType::Magnetar => format!(
            "🧲 Magnetar Remnant Settled: {} (B ~ 10^15 G Relativistic Jets)!",
            targets.body.name
        ),
        _ => format!(
            "⚡ Relativistic Pulsar Remnant Settled: {} (B ~ 10^12 G, P ~ 33 ms)!",
            targets.body.name
        ),
    };
}

fn handle_neutron_remnant_aging(
    targets: &mut StarAgingTargets<'_>,
    commands: &mut Option<&mut Commands>,
    toast: &mut NotificationToast,
) -> bool {
    if targets.mass.0 > TOV_LIMIT_SOLAR {
        let m_bh = targets.mass.0;
        if let Some(evo) = targets.evo.as_deref_mut() {
            evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
            evo.phase_timer_years = 0.0;
        }
        apply_compact_remnant_state(targets, BodyType::BlackHole, m_bh, commands);
        toast.message =
            "🕳️ TOV Limit Exceeded (>2.17 M☉): Neutron remnant collapsed into a Black Hole!"
                .to_string();
        return true;
    }
    if targets.mass.0 >= 1.80 && targets.body.body_type != BodyType::QuarkStar {
        targets.body.body_type = BodyType::QuarkStar;
        targets.body.name = format!("Strange Quark Star ({:.2} M☉)", targets.mass.0);
        targets.radius.0 = 0.00008;
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 10_000_000.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 180.0;
        }
        insert_remnant_em_field(commands, targets.entity, BodyType::QuarkStar);
        toast.message =
            "⚛️ Quark Matter Phase Transition (>1.80 M☉): Compressed into a Strange Quark Star!"
                .to_string();
        return true;
    }
    false
}

fn handle_remnant_or_ejection_aging(
    targets: &mut StarAgingTargets<'_>,
    commands: &mut Option<&mut Commands>,
    supernova_events: &mut Option<&mut MessageWriter<SupernovaEvent>>,
    toast: &mut NotificationToast,
) -> bool {
    let phase_opt = targets.evo.as_deref().map(|e| e.phase);
    if phase_opt == Some(StellarEvolutionPhase::PlanetaryNebulaEjection) {
        settle_white_dwarf_remnant(targets, false);
        toast.message = format!(
            "⚪ Degenerate White Dwarf ({:.2} M☉, Earth-sized, T ~ 30,000 K)! Click Age for Black Dwarf cooling (or add >1.44 M☉ for Type Ia SN).",
            targets.mass.0
        );
        return true;
    }

    if phase_opt == Some(StellarEvolutionPhase::SupernovaExplosion) {
        settle_supernova_remnant(targets, commands, toast);
        return true;
    }

    if targets.body.body_type == BodyType::WhiteDwarf
        || (phase_opt == Some(StellarEvolutionPhase::WhiteDwarf)
            && targets.body.body_type != BodyType::BlackDwarf)
    {
        if targets.mass.0 > CHANDRASEKHAR_LIMIT_SOLAR {
            detonate_supernova_transition(targets, commands, supernova_events, toast, true);
            return true;
        }
        if let Some(evo) = targets.evo.as_deref_mut() {
            evo.phase = StellarEvolutionPhase::WhiteDwarf;
            evo.phase_timer_years = 0.0;
        }
        targets.body.body_type = BodyType::BlackDwarf;
        targets.body.name = "The Star (Black Dwarf Remnant)".to_string();
        targets.ignition.is_ignited = false;
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 10.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 0.0;
        }
        toast.message =
            "🌑 Black Dwarf Remnant: White Dwarf cooled to thermal equilibrium over cosmic time!"
                .to_string();
        return true;
    }

    if matches!(
        targets.body.body_type,
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar | BodyType::QuarkStar
    ) && handle_neutron_remnant_aging(targets, commands, toast)
    {
        return true;
    }

    if targets.body.body_type.is_remnant() || phase_opt.is_some_and(|p| p.is_remnant()) {
        toast.message = format!(
            "⚠️ Terminal State: {} is already a degenerate remnant and cannot age further!",
            targets.body.name
        );
        return true;
    }

    false
}

fn handle_protostar_ignition_step(
    targets: &mut StarAgingTargets<'_>,
    toast: &mut NotificationToast,
) {
    let assigned_type = classify_body_by_mass_and_comp(targets.mass.0, targets.comp, true);
    targets.body.body_type = assigned_type;
    if !crate::simulation::accretion::collisions::is_canonical_named_body(&targets.body.name) {
        targets.body.name = the_star_display_name_for_type(assigned_type).to_string();
    }

    if assigned_type == BodyType::BrownDwarf {
        targets.ignition.core_temperature = 2.5e6;
        targets.ignition.fusion_fraction = 0.05;
        targets.ignition.is_ignited = false;
        targets.radius.0 = 0.11 * SOLAR_RADIUS_AU;
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 1400.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 1e-4;
        }
        if let Some(evo) = targets.evo.as_deref_mut() {
            evo.phase = StellarEvolutionPhase::ProtostarContraction;
            evo.phase_timer_years = 0.0;
        }
        toast.message = format!(
            "🪨 Substellar Brown Dwarf ({:.3} M☉ < 0.08 M☉): Insufficient mass for core hydrogen fusion!",
            targets.mass.0
        );
        return;
    }

    targets.ignition.core_temperature = 1.0e7;
    targets.ignition.is_ignited = true;
    targets.ignition.fusion_fraction = 1.0;
    targets.ignition.shockwave_radius = 1.6;
    targets.radius.0 = (SOLAR_RADIUS_AU * targets.mass.0.max(0.08).powf(0.8)).clamp(0.002, 0.12);
    if let Some(t) = targets.temp.as_deref_mut() {
        t.0 = (5778.0 * targets.mass.0.max(0.08).powf(0.505)).clamp(1800.0, 50_000.0);
    }
    if let Some(l) = targets.lum.as_deref_mut() {
        l.0 = targets.mass.0.max(0.01).powf(3.5);
    }
    if let Some(evo) = targets.evo.as_deref_mut() {
        evo.phase = StellarEvolutionPhase::MainSequence;
        evo.hydrogen_core_fraction = 1.0;
        evo.phase_timer_years = 0.0;
    }
    let next_hint = if targets.mass.0 >= 25.0 {
        "Click Age again for Hypergiant -> Hypernova!"
    } else if targets.mass.0 >= 8.0 {
        "Click Age again for Red Supergiant -> Supernova!"
    } else if targets.mass.0 >= 0.45 {
        "Click Age again for Red Giant!"
    } else {
        "Click Age again for Helium White Dwarf!"
    };
    toast.message = format!(
        "⭐ Ignited Main Sequence: {} ({:.2} M☉ | {next_hint})",
        targets.body.name, targets.mass.0
    );
}

fn handle_massive_star_aging(
    targets: &mut StarAgingTargets<'_>,
    phase: StellarEvolutionPhase,
    commands: &mut Option<&mut Commands>,
    supernova_events: &mut Option<&mut MessageWriter<SupernovaEvent>>,
    toast: &mut NotificationToast,
) {
    let m = targets.mass.0;
    let already_giant = matches!(
        phase,
        StellarEvolutionPhase::RedSupergiantBranch
            | StellarEvolutionPhase::RedGiantBranch
            | StellarEvolutionPhase::HeliumFlashAgb
    ) || targets.body.body_type.is_evolved_giant();
    if already_giant {
        detonate_supernova_transition(targets, commands, supernova_events, toast, false);
        return;
    }
    if let Some(evo) = targets.evo.as_deref_mut() {
        evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
        evo.phase_timer_years = 0.0;
    }
    if m >= 25.0 {
        targets.body.body_type = BodyType::YellowHypergiant;
        targets.body.name = "The Star (Luminous Yellow Hypergiant)".to_string();
        targets.radius.0 = 4.5;
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 4000.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 250_000.0;
        }
        toast.message =
            "💥 Yellow Hypergiant Phase (250,000 L☉)! Click Age again for Hypernova -> Black Hole!"
                .to_string();
    } else {
        targets.body.body_type = BodyType::RedSupergiant;
        targets.body.name = "The Star (Red Supergiant)".to_string();
        targets.radius.0 = (3.2 * (m / 15.0).powf(0.4)).clamp(1.8, 4.5);
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 3300.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 80_000.0 * (m / 15.0).powf(2.0);
        }
        toast.message = format!(
            "🔴 Red Supergiant (R ~ {:.2} AU, {:.1} M☉)! Click Age again for Type II Supernova!",
            targets.radius.0, m
        );
    }
}

fn handle_solar_branch_aging(
    targets: &mut StarAgingTargets<'_>,
    phase: StellarEvolutionPhase,
    toast: &mut NotificationToast,
) {
    let m = targets.mass.0;
    match phase {
        StellarEvolutionPhase::ProtostarContraction | StellarEvolutionPhase::MainSequence => {
            if let Some(evo) = targets.evo.as_deref_mut() {
                evo.phase = StellarEvolutionPhase::RedGiantBranch;
                evo.hydrogen_core_fraction = 0.0;
                evo.phase_timer_years = 0.0;
            }
            targets.body.body_type = BodyType::RedGiant;
            targets.body.name = "The Star (Red Giant Branch)".to_string();
            targets.radius.0 = (1.25 * m.powf(0.3)).clamp(0.85, 2.5);
            if let Some(t) = targets.temp.as_deref_mut() {
                t.0 = 3100.0;
            }
            if let Some(l) = targets.lum.as_deref_mut() {
                l.0 = 2500.0 * m;
            }
            toast.message = format!(
                "🔴 Red Giant Branch (R ~ {:.2} AU, L ~ {:.0} L☉): Inner planets engulfing! Click Age for Helium Flash / AGB.",
                targets.radius.0,
                2500.0 * m
            );
        }
        StellarEvolutionPhase::RedGiantBranch => {
            if let Some(evo) = targets.evo.as_deref_mut() {
                evo.phase = StellarEvolutionPhase::HeliumFlashAgb;
                evo.helium_core_fraction = 1.0;
                evo.phase_timer_years = 0.0;
            }
            if (1.5..=4.0).contains(&m) {
                targets.body.body_type = BodyType::CarbonStar;
                targets.body.name = "The Star (Carbon Star AGB)".to_string();
                targets.radius.0 = 1.65;
                if let Some(t) = targets.temp.as_deref_mut() {
                    t.0 = 2400.0;
                }
                if let Some(l) = targets.lum.as_deref_mut() {
                    l.0 = 4800.0;
                }
            } else {
                targets.body.body_type = BodyType::RedGiant;
                targets.body.name = "The Star (AGB Supergiant)".to_string();
                targets.radius.0 = 1.65;
                if let Some(t) = targets.temp.as_deref_mut() {
                    t.0 = 2900.0;
                }
                if let Some(l) = targets.lum.as_deref_mut() {
                    l.0 = 4500.0;
                }
            }
            targets.ignition.shockwave_radius = 1.8;
            toast.message =
                "🔥 Helium Flash & AGB Thermal Pulses (R ~ 1.65 AU)! Click Age again for Planetary Nebula Ejection!"
                    .to_string();
        }
        StellarEvolutionPhase::HeliumFlashAgb => {
            let target_wd_mass = (0.47 + 0.08 * m).clamp(0.50, 1.20);
            if let Some(evo) = targets.evo.as_deref_mut() {
                evo.phase = StellarEvolutionPhase::PlanetaryNebulaEjection;
                evo.phase_timer_years = 0.0;
                evo.nebula_expansion_radius_au = 2.0;
                evo.nebula_opacity = 1.0;
                evo.envelope_mass_loss_rate = 1e-15;
            }
            targets.mass.0 = target_wd_mass;
            targets.radius.0 = 0.04 * SOLAR_RADIUS_AU;
            if let Some(t) = targets.temp.as_deref_mut() {
                t.0 = 65_000.0;
            }
            if let Some(l) = targets.lum.as_deref_mut() {
                l.0 = 3500.0;
            }
            targets.ignition.is_ignited = false;
            targets.body.body_type = BodyType::WhiteDwarf;
            targets.body.name = "The Star (Planetary Nebula Core)".to_string();
            toast.message = format!(
                "💨 Planetary Nebula Ejected! Shed envelope to {target_wd_mass:.2} M☉ exposing 65,000 K UV Core — click Age for White Dwarf!"
            );
        }
        _ => {
            settle_white_dwarf_remnant(targets, false);
            toast.message = format!(
                "⚪ Degenerate White Dwarf ({:.2} M☉, Earth-sized, T ~ 30,000 K)! Click Age again for Black Dwarf cooling.",
                targets.mass.0
            );
        }
    }
}

fn handle_active_giant_or_ms_aging(
    targets: &mut StarAgingTargets<'_>,
    commands: &mut Option<&mut Commands>,
    supernova_events: &mut Option<&mut MessageWriter<SupernovaEvent>>,
    toast: &mut NotificationToast,
) {
    let m = targets.mass.0;
    let phase = targets
        .evo
        .as_deref()
        .map_or(StellarEvolutionPhase::MainSequence, |e| e.phase);

    if m < 0.08 {
        if let Some(evo) = targets.evo.as_deref_mut() {
            evo.phase_timer_years = 0.0;
        }
        if let Some(t) = targets.temp.as_deref_mut() {
            t.0 = 150.0;
        }
        if let Some(l) = targets.lum.as_deref_mut() {
            l.0 = 1e-6;
        }
        toast.message = format!(
            "⚠️ Terminal State: {} is a cooled Brown Dwarf (raise >= 0.08 M☉ for H fusion)!",
            targets.body.name
        );
    } else if m >= 8.0 {
        handle_massive_star_aging(targets, phase, commands, supernova_events, toast);
    } else if m < 0.45 {
        settle_white_dwarf_remnant(targets, true);
        toast.message =
            "⚪ Low-Mass M-Dwarf (<0.45 M☉) skipped Red Giant Branch -> Helium White Dwarf!"
                .to_string();
    } else {
        handle_solar_branch_aging(targets, phase, toast);
    }
}

/// Advances a single star entity dynamically through its mass-dependent evolutionary track.
pub fn age_single_star_entity(
    targets: &mut StarAgingTargets<'_>,
    mut commands: Option<&mut Commands>,
    mut supernova_events: Option<&mut MessageWriter<SupernovaEvent>>,
    toast: &mut NotificationToast,
) {
    if handle_remnant_or_ejection_aging(targets, &mut commands, &mut supernova_events, toast) {
        toast.timer = 6.0;
        return;
    }

    let is_protostar_unignited = !targets.ignition.is_ignited
        && targets
            .evo
            .as_deref()
            .is_none_or(|e| e.phase == StellarEvolutionPhase::ProtostarContraction);
    if is_protostar_unignited {
        handle_protostar_ignition_step(targets, toast);
        toast.timer = 6.0;
        return;
    }

    handle_active_giant_or_ms_aging(targets, &mut commands, &mut supernova_events, toast);
    toast.timer = 6.0;
}

/// Advances the selected (or central) star with full ECS side effects (supernova blasts & relativistic components).
pub fn handle_age_star_with_effects(
    selected_query: &mut SelectedWorldQuery,
    selected_ent: Option<Entity>,
    commands: Option<&mut Commands>,
    supernova_events: Option<&mut MessageWriter<SupernovaEvent>>,
    toast: &mut NotificationToast,
) {
    let mut star_opt = selected_ent
        .filter(|&ent| {
            selected_query
                .get(ent)
                .is_ok_and(|(_, _, _, _, _, _, body, is_star, ..)| {
                    is_star.is_some() || body.body_type.is_star_or_remnant()
                })
        })
        .and_then(|ent| selected_query.get_mut(ent).ok());

    if star_opt.is_none() {
        star_opt = selected_query
            .iter_mut()
            .find(|(_, _, _, _, _, _, _, is_star, ..)| is_star.is_some());
    }

    if let Some((
        ent,
        mut mass,
        mut radius,
        _,
        _,
        comp,
        mut body,
        _,
        Some(ref mut ignition),
        ref mut opt_evo,
        ref mut opt_temp,
        ref mut opt_lum,
        ..,
    )) = star_opt
    {
        let mut targets = StarAgingTargets {
            entity: ent,
            mass: &mut mass,
            radius: &mut radius,
            body: &mut body,
            comp: &comp,
            ignition,
            evo: opt_evo.as_deref_mut(),
            temp: opt_temp.as_deref_mut(),
            lum: opt_lum.as_deref_mut(),
        };
        age_single_star_entity(&mut targets, commands, supernova_events, toast);
    }
}

/// Legacy 3-argument wrapper preserving test and caller compatibility.
pub fn handle_age_star(
    selected_query: &mut SelectedWorldQuery,
    selected_ent: Option<Entity>,
    toast: &mut NotificationToast,
) {
    handle_age_star_with_effects(selected_query, selected_ent, None, None, toast);
}

pub fn handle_ignite_star(
    selected_query: &mut SelectedWorldQuery,
    selected_ent: Option<Entity>,
    toast: &mut NotificationToast,
) {
    let mut star_opt = selected_ent
        .filter(|&ent| {
            selected_query
                .get(ent)
                .is_ok_and(|(_, _, _, _, _, _, body, is_star, ..)| {
                    is_star.is_some() || body.body_type.is_star_or_remnant()
                })
        })
        .and_then(|ent| selected_query.get_mut(ent).ok());

    if star_opt.is_none() {
        star_opt = selected_query
            .iter_mut()
            .find(|(_, _, _, _, _, _, _, is_star, ..)| is_star.is_some());
    }

    if let Some((
        _,
        mass,
        mut radius,
        _,
        _,
        comp,
        mut body,
        _,
        Some(ref mut ignition),
        ref mut opt_evo,
        ref mut opt_temp,
        ref mut opt_lum,
        ..,
    )) = star_opt
    {
        if body.body_type.is_remnant() || opt_evo.as_ref().is_some_and(|e| e.phase.is_remnant()) {
            if body.body_type == BodyType::BlackHole
                || opt_evo
                    .as_ref()
                    .is_some_and(|e| e.phase == StellarEvolutionPhase::BlackHoleRemnant)
            {
                ignition.shockwave_radius = 2.0;
                toast.message = format!(
                    "⚡ Accretion Flare: Relativistic burst erupted from {}!",
                    body.name
                );
            } else {
                toast.message = format!(
                    "⚠️ Degenerate Remnant: {} has exhausted core fusion and cannot ignite!",
                    body.name
                );
            }
            toast.timer = 5.0;
            return;
        }

        if opt_evo.as_deref().is_some_and(|e| {
            matches!(
                e.phase,
                StellarEvolutionPhase::PlanetaryNebulaEjection
                    | StellarEvolutionPhase::SupernovaExplosion
            )
        }) {
            toast.message =
                "⚠️ Stellar envelope actively shedding — click Age [N] to finish remnant formation!"
                    .to_string();
            toast.timer = 5.0;
            return;
        }

        if ignition.is_ignited {
            ignition.shockwave_radius = 1.6;
            toast.message = "☀️ Coronal Mass Ejection & Solar Blast Triggered!".to_string();
        } else {
            let assigned_type = classify_body_by_mass_and_comp(mass.0, &comp, true);
            ignition.core_temperature = 1.0e7;
            ignition.is_ignited = true;
            ignition.fusion_fraction = 1.0;
            ignition.shockwave_radius = 1.6;
            body.body_type = assigned_type;
            body.name = the_star_display_name_for_type(assigned_type).to_string();
            radius.0 = (SOLAR_RADIUS_AU * mass.0.max(0.08).powf(0.8)).clamp(0.002, 0.12);
            if let Some(t) = opt_temp.as_deref_mut() {
                t.0 = (5778.0 * mass.0.max(0.08).powf(0.505)).clamp(1800.0, 50_000.0);
            }
            if let Some(l) = opt_lum.as_deref_mut() {
                l.0 = mass.0.max(0.01).powf(3.5);
            }
            if let Some(evo) = opt_evo {
                evo.phase = StellarEvolutionPhase::MainSequence;
                evo.hydrogen_core_fraction = 1.0;
            }
            toast.message =
                "⭐ Hydrogen Core Fusion Ignited! Solar Wind Shockwave Sweeping the System!"
                    .to_string();
        }
        toast.timer = 5.0;
    }
}

pub fn handle_trigger_cme(
    selected_query: &mut SelectedWorldQuery,
    selected_ent: Option<Entity>,
    commands: &mut Commands,
    toast: &mut NotificationToast,
) {
    let mut star_opt = selected_ent
        .filter(|&ent| {
            selected_query
                .get(ent)
                .is_ok_and(|(_, _, _, _, _, _, body, is_star, ..)| {
                    is_star.is_some() || body.body_type.is_star_or_remnant()
                })
        })
        .and_then(|ent| selected_query.get_mut(ent).ok());

    if star_opt.is_none() {
        star_opt = selected_query
            .iter_mut()
            .find(|(_, _, _, _, _, _, _, is_star, ..)| is_star.is_some());
    }

    if let Some((star_ent, _, _, _, _, _, _, _, Some(ref mut ignition), ..)) = star_opt {
        ignition.shockwave_radius = 0.1;
        commands
            .entity(star_ent)
            .insert(crate::simulation::space_weather::StellarFlareState {
                flare_frequency: 1.5,
                current_flare_intensity: 6.5,
                flare_decay_timer_years: 0.15,
                cme_front_radius_au: 0.05,
                cme_speed_au_day: 0.35,
                cme_density_multiplier: 25.0,
                cme_active: true,
            });
        toast.message =
            "☀️ Coronal Mass Ejection Erupted! Plasma Shockwave Propagating Interplanetary!"
                .to_string();
        toast.timer = 6.0;
    } else {
        toast.message = "⚠️ No central star found in system!".to_string();
        toast.timer = 3.0;
    }
}
