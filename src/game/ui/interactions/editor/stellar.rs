//! Stellar lifecycle transitions, ignition, and space weather flare triggers.

use bevy::prelude::*;

use crate::game::ui::types::*;
use crate::simulation::components::*;
use crate::utils::constants::*;

fn handle_hypergiant_aging(
    m: f64,
    mass: &mut Mass,
    radius: &mut Radius,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    toast: &mut NotificationToast,
) {
    match evo.phase {
        StellarEvolutionPhase::ProtostarContraction | StellarEvolutionPhase::MainSequence => {
            evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
            body.body_type = BodyType::Hypergiant;
            body.name = "The Star (Luminous Yellow Hypergiant)".to_string();
            radius.0 = 4.5;
            temp.0 = 4000.0;
            lum.0 = 250_000.0;
            toast.message = "💥 Hypergiant Phase: Luminosity surged to 250,000 L☉!".to_string();
        }
        StellarEvolutionPhase::RedSupergiantBranch => {
            evo.phase = StellarEvolutionPhase::SupernovaExplosion;
            body.name = "The Star (Hypernova Detonation)".to_string();
            evo.nebula_expansion_radius_au = 5.0;
            evo.nebula_opacity = 1.0;
            toast.message =
                "💥 HYPERNOVA DETONATION! Core collapsing into a Singularity!".to_string();
        }
        _ => {
            evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
            body.body_type = BodyType::BlackHole;
            body.name = "The Star (Stellar-Mass Black Hole)".to_string();
            mass.0 = (m * 0.25).clamp(3.0, 15.0);
            radius.0 = (1.974e-8 * mass.0).max(1e-7);
            temp.0 = 10.0;
            lum.0 = 5000.0;
            toast.message =
                "🕳️ Gravitational Singularity Formed (Event Horizon & Accretion Disk)!".to_string();
        }
    }
}

fn handle_supergiant_aging(
    mass: &mut Mass,
    radius: &mut Radius,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    toast: &mut NotificationToast,
) {
    match evo.phase {
        StellarEvolutionPhase::ProtostarContraction | StellarEvolutionPhase::MainSequence => {
            evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
            body.body_type = BodyType::RedSupergiant;
            body.name = "The Star (Red Supergiant)".to_string();
            radius.0 = 3.5;
            temp.0 = 3300.0;
            lum.0 = 80_000.0;
            toast.message = "🔴 Red Supergiant Expansion (R ~ 3.5 AU, L ~ 80,000 L☉)!".to_string();
        }
        StellarEvolutionPhase::RedSupergiantBranch => {
            evo.phase = StellarEvolutionPhase::SupernovaExplosion;
            body.name = "The Star (Type II Supernova Explosion)".to_string();
            evo.nebula_expansion_radius_au = 4.0;
            evo.nebula_opacity = 1.0;
            toast.message =
                "💥 TYPE II CORE-COLLAPSE SUPERNOVA! Blast expanding at 15,000 km/s!".to_string();
        }
        _ => {
            evo.phase = StellarEvolutionPhase::NeutronStarPulsar;
            body.body_type = BodyType::Pulsar;
            body.name = "The Star (Pulsar Remnant)".to_string();
            mass.0 = 1.44;
            radius.0 = 0.0001;
            temp.0 = 1_000_000.0;
            lum.0 = 100.0;
            toast.message =
                "⚡ Relativistic Pulsar Remnant (B ~ 10^12 G, P ~ 33 ms Synchrotron Jets)!"
                    .to_string();
        }
    }
}

fn handle_intermediate_and_low_mass_aging(
    m: f64,
    mass: &mut Mass,
    radius: &mut Radius,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    toast: &mut NotificationToast,
) {
    if m >= 0.50 {
        match evo.phase {
            StellarEvolutionPhase::ProtostarContraction | StellarEvolutionPhase::MainSequence => {
                evo.phase = StellarEvolutionPhase::RedGiantBranch;
                evo.hydrogen_core_fraction = 0.0;
                body.body_type = BodyType::RedGiant;
                body.name = "The Star (Red Giant Branch)".to_string();
                radius.0 = 1.25;
                temp.0 = 3100.0;
                lum.0 = 2500.0;
                toast.message = "🔴 Star Expanded to Red Giant (R ~ 1.25 AU, L ~ 2500 L☉)! Inner planets engulfing!".to_string();
            }
            StellarEvolutionPhase::RedGiantBranch => {
                evo.phase = StellarEvolutionPhase::HeliumFlashAgb;
                body.name = "The Star (AGB Supergiant)".to_string();
                radius.0 = 1.50;
                temp.0 = 2900.0;
                lum.0 = 3500.0;
                toast.message =
                    "🔥 Helium Flash & Asymptotic Giant Pulses (R ~ 1.50 AU, L ~ 3500 L☉)!"
                        .to_string();
            }
            StellarEvolutionPhase::HeliumFlashAgb => {
                evo.phase = StellarEvolutionPhase::PlanetaryNebulaEjection;
                body.name = "The Star (Planetary Nebula Ejection)".to_string();
                evo.nebula_expansion_radius_au = 2.0;
                evo.nebula_opacity = 1.0;
                mass.0 = 0.55;
                toast.message =
                    "💨 Planetary Nebula Ejected! Star shed 45% mass, outer orbits expanding!"
                        .to_string();
            }
            _ => {
                evo.phase = StellarEvolutionPhase::WhiteDwarf;
                body.body_type = BodyType::WhiteDwarf;
                body.name = "The Star (White Dwarf Remnant)".to_string();
                radius.0 = 0.009;
                temp.0 = 30_000.0;
                lum.0 = (radius.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4);
                toast.message =
                    "⚪ Degenerate White Dwarf Remnant (Earth-sized, T ~ 30,000 K, B ~ 10^6 G)!"
                        .to_string();
            }
        }
    } else {
        evo.phase = StellarEvolutionPhase::WhiteDwarf;
        body.body_type = BodyType::WhiteDwarf;
        body.name = "The Star (Helium White Dwarf)".to_string();
        radius.0 = 0.009;
        temp.0 = 25_000.0;
        toast.message = "⚪ Low-Mass Helium White Dwarf Remnant Formed!".to_string();
    }
}

pub fn handle_star_aging_transition(
    mass: &mut Mass,
    radius: &mut Radius,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    toast: &mut NotificationToast,
) {
    let m = mass.0;
    if m >= 25.0 {
        handle_hypergiant_aging(m, mass, radius, body, evo, temp, lum, toast);
    } else if m >= 8.0 {
        handle_supergiant_aging(mass, radius, body, evo, temp, lum, toast);
    } else {
        handle_intermediate_and_low_mass_aging(m, mass, radius, body, evo, temp, lum, toast);
    }
}

pub fn handle_age_star(selected_query: &mut SelectedWorldQuery, toast: &mut NotificationToast) {
    let mut star_opt = selected_query
        .iter_mut()
        .find(|(_, _, _, _, _, _, _, is_star, ..)| is_star.is_some());

    if let Some((
        _ent,
        mut mass,
        mut radius,
        _,
        _,
        _,
        mut body,
        _,
        Some(ref mut ignition),
        ref mut opt_evo,
        Some(ref mut temp),
        Some(ref mut lum),
        ..,
    )) = star_opt
    {
        if !ignition.is_ignited {
            ignition.core_temperature = 1.0e7;
            ignition.is_ignited = true;
            ignition.fusion_fraction = 1.0;
            ignition.shockwave_radius = 1.6;
            let (assigned_type, name_str) = if mass.0 < 0.08 {
                (BodyType::BrownDwarf, "The Star (Brown Dwarf)")
            } else if mass.0 < 0.50 {
                (BodyType::RedDwarf, "The Star (Red Dwarf - M Type)")
            } else if mass.0 < 0.80 {
                (BodyType::OrangeDwarf, "The Star (Orange Dwarf - K Type)")
            } else if mass.0 < 1.08 {
                (BodyType::YellowDwarf, "The Star (Yellow Dwarf - G2V)")
            } else if mass.0 < 1.40 {
                (
                    BodyType::YellowWhiteDwarf,
                    "The Star (Yellow-White Dwarf - F Type)",
                )
            } else if mass.0 < 2.10 {
                (
                    BodyType::WhiteMainSequence,
                    "The Star (White Main-Sequence - A Type)",
                )
            } else if mass.0 < 8.0 {
                (BodyType::BlueGiant, "The Star (Blue Giant - B Type)")
            } else if mass.0 < 25.0 {
                (
                    BodyType::BlueSupergiant,
                    "The Star (Blue Supergiant - O Type)",
                )
            } else {
                (BodyType::Hypergiant, "The Star (Luminous Hypergiant)")
            };
            body.body_type = assigned_type;
            body.name = name_str.to_string();
            if let Some(ref mut evo) = opt_evo {
                evo.phase = StellarEvolutionPhase::MainSequence;
            }
            toast.message = format!("⭐ Ignited: {} (Main Sequence)", body.name);
        } else if let Some(ref mut evo) = opt_evo {
            handle_star_aging_transition(&mut mass, &mut radius, &mut body, evo, temp, lum, toast);
        }
        toast.timer = 6.0;
    }
}

pub fn handle_ignite_star(selected_query: &mut SelectedWorldQuery, toast: &mut NotificationToast) {
    let mut star_opt = selected_query
        .iter_mut()
        .find(|(_, _, _, _, _, _, _, is_star, ..)| is_star.is_some());
    if let Some((_, _, _, _, _, _, mut body, _, Some(ref mut ignition), ref mut opt_evo, ..)) =
        star_opt
    {
        if ignition.is_ignited {
            ignition.shockwave_radius = 1.6;
            toast.message = "☀️ Coronal Mass Ejection & Solar Blast Triggered!".to_string();
        } else {
            ignition.core_temperature = 1.0e7;
            ignition.is_ignited = true;
            ignition.fusion_fraction = 1.0;
            ignition.shockwave_radius = 1.6;
            body.body_type = BodyType::MainSequenceStar;
            body.name = "The Star (Main Sequence)".to_string();
            if let Some(ref mut evo) = opt_evo {
                evo.phase = StellarEvolutionPhase::MainSequence;
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
    commands: &mut Commands,
    toast: &mut NotificationToast,
) {
    let mut star_opt = selected_query
        .iter_mut()
        .find(|(_, _, _, _, _, _, _, is_star, ..)| is_star.is_some());
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
