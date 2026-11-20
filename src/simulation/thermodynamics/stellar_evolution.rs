//! Post-ZAMS stellar evolution phases: main sequence, red giant / AGB,
//! red supergiant core collapse, planetary nebulae, and degenerate remnant cooling.

use bevy::prelude::*;

use super::stellar_lifecycle::{
    classify_ignited_protostar_to_zams, clean_star_base_name, update_promoted_stellar_name,
};
use crate::simulation::components::*;
use crate::utils::constants::*;

#[allow(
    clippy::too_many_arguments,
    reason = "Stellar evolution phases require coordinating mass, radius, luminosity, ignition, and evolution states"
)]
pub fn step_stellar_evolution_cycle(
    entity: Entity,
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    ignition: &mut IgnitionState,
    evo: &mut StellarEvolutionState,
    dt_yr: f64,
    supernova_events: &mut MessageWriter<SupernovaEvent>,
    is_gmc: bool,
) {
    evo.phase_timer_years += dt_yr;

    // Degenerate black holes never re-enter stellar nucleosynthesis branches
    if body.body_type == BodyType::BlackHole || evo.phase == StellarEvolutionPhase::BlackHoleRemnant
    {
        step_stellar_remnant_evolution(mass, radius, temp, lum, body, evo, dt_yr);
        return;
    }

    match evo.phase {
        StellarEvolutionPhase::ProtostarContraction => {
            if ignition.is_ignited && !body.body_type.is_remnant() {
                if body.body_type == BodyType::Protostar {
                    classify_ignited_protostar_to_zams(mass.0, radius, temp, lum, body);
                }
                evo.phase = StellarEvolutionPhase::MainSequence;
                evo.hydrogen_core_fraction = 1.0;
            }
        }
        StellarEvolutionPhase::MainSequence => {
            step_main_sequence_evolution(mass, radius, temp, lum, body, evo, dt_yr, is_gmc);
        }
        StellarEvolutionPhase::RedGiantBranch => {
            step_red_giant_evolution(mass, radius, temp, lum, body, evo, dt_yr, is_gmc);
        }
        StellarEvolutionPhase::HeliumFlashAgb => {
            step_agb_evolution(mass.0, radius, temp, lum, body, evo, dt_yr, is_gmc);
        }
        StellarEvolutionPhase::RedSupergiantBranch => {
            step_red_supergiant_evolution(
                entity,
                mass,
                radius,
                temp,
                lum,
                body,
                ignition,
                evo,
                dt_yr,
                is_gmc,
                supernova_events,
            );
        }
        _ => step_stellar_remnant_evolution(mass, radius, temp, lum, body, evo, dt_yr),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Main sequence evolution updates core physical observables"
)]
fn step_main_sequence_evolution(
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    dt_yr: f64,
    is_gmc: bool,
) {
    let k = (1.0 - (-0.05 * dt_yr).exp()).clamp(0.0, 1.0);
    let (target_lum, target_temp, target_rad) = match body.body_type {
        BodyType::WolfRayet => (250_000.0, 85_000.0, SOLAR_RADIUS_AU * 2.5),
        BodyType::LuminousBlueVariable => (
            250_000.0_f64.max(mass.0.powf(3.5)),
            25_000.0,
            (SOLAR_RADIUS_AU * mass.0.powf(0.8)).clamp(0.05, 0.25),
        ),
        _ => (
            mass.0.powf(3.5),
            (5778.0 * mass.0.powf(0.505)).clamp(1800.0, 50_000.0),
            (SOLAR_RADIUS_AU * mass.0.powf(0.8)).clamp(0.001, 0.12),
        ),
    };
    lum.0 += (target_lum - lum.0) * k;
    temp.0 += (target_temp - temp.0) * k;
    radius.0 += (target_rad - radius.0) * k;

    // Re-classify accreting main-sequence or sub-stellar stars across spectral tiers
    if !body.name.starts_with("The Sun")
        && !body.name.contains("Genesis")
        && !matches!(
            body.body_type,
            BodyType::DarkStar
                | BodyType::PopulationIIIStar
                | BodyType::BlueStraggler
                | BodyType::WolfRayet
                | BodyType::LuminousBlueVariable
                | BodyType::QuasiStar
        )
    {
        let (target_type, label) =
            classify_stellar_type_and_label(mass.0, &Composition::solar_gas());
        if target_type != body.body_type && target_type != BodyType::BlackHole {
            body.body_type = target_type;
            update_promoted_stellar_name(&mut body.name, target_type, mass.0, label);
        }
    }

    // Massive stars (M > 25 M_sun) undergo extreme radiation-driven wind mass loss
    // and can strip their outer hydrogen envelopes into Wolf-Rayet or LBV states near core exhaustion
    if mass.0 >= 60.0
        && evo.hydrogen_core_fraction < 0.22
        && !matches!(
            body.body_type,
            BodyType::LuminousBlueVariable
                | BodyType::PopulationIIIStar
                | BodyType::DarkStar
                | BodyType::QuasiStar
        )
    {
        body.body_type = BodyType::LuminousBlueVariable;
        let base = clean_star_base_name(&body.name);
        body.name = format!("{base} (LBV)");
    } else if mass.0 >= 25.0
        && mass.0 < 60.0
        && evo.hydrogen_core_fraction < 0.25
        && body.body_type != BodyType::WolfRayet
    {
        body.body_type = BodyType::WolfRayet;
        let base = clean_star_base_name(&body.name);
        body.name = format!("{base} (Wolf-Rayet)");
        temp.0 = 85_000.0;
        lum.0 = 250_000.0;
    }

    // In cluster environments, massive stars live accelerated lifecycles (2,000 - 7,500 yr)
    let main_seq_lifetime_yr = if is_gmc && mass.0 >= 8.0 {
        (3500.0 * (15.0 / mass.0).powf(1.1)).clamp(1500.0, 7500.0)
    } else {
        (1.0e10 * (mass.0).powf(-2.5)).clamp(1.0e6, 1.0e13)
    };
    let fuel_burn_rate = 1.0 / main_seq_lifetime_yr;
    evo.hydrogen_core_fraction = (evo.hydrogen_core_fraction - fuel_burn_rate * dt_yr).max(0.0);

    if evo.hydrogen_core_fraction <= 0.0 {
        evo.phase_timer_years = 0.0;
        let base = clean_star_base_name(&body.name);
        if mass.0 < 0.45 {
            evo.phase = StellarEvolutionPhase::WhiteDwarf;
            body.body_type = BodyType::WhiteDwarf;
            body.name = if is_gmc {
                format!("{base} (White Dwarf)")
            } else {
                "The Star (Helium White Dwarf)".to_string()
            };
            radius.0 = 0.009 * SOLAR_RADIUS_AU;
            temp.0 = 25_000.0;
        } else if mass.0 < 8.0 {
            evo.phase = StellarEvolutionPhase::RedGiantBranch;
            body.body_type = BodyType::RedGiant;
            body.name = if is_gmc {
                format!("{base} (Red Giant)")
            } else {
                "The Star (Red Giant Branch)".to_string()
            };
        } else {
            evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
            body.body_type = BodyType::RedSupergiant;
            body.name = if is_gmc {
                format!("{base} (Red Supergiant)")
            } else {
                "The Star (Red Supergiant)".to_string()
            };
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Red giant branch evolution updates radius, temperature, luminosity, and core helium"
)]
fn step_red_giant_evolution(
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    dt_yr: f64,
    is_gmc: bool,
) {
    let target_r = if is_gmc {
        (0.045 * mass.0.powf(0.3)).clamp(0.035, 0.08)
    } else {
        (1.25 * mass.0.powf(0.3)).clamp(0.8, 2.5)
    };
    let k = (1.0 - (-0.008 * dt_yr).exp()).clamp(0.0, 1.0);
    radius.0 += (target_r - radius.0) * k;
    temp.0 += (3100.0 - temp.0) * k;
    lum.0 += (2500.0 * mass.0 - lum.0) * k;

    evo.helium_core_fraction = (evo.helium_core_fraction + 0.0003 * dt_yr).min(1.0);
    if evo.helium_core_fraction >= 1.0 || evo.phase_timer_years > 3000.0 {
        evo.phase = StellarEvolutionPhase::HeliumFlashAgb;
        // Thermal pulses and carbon-dredging produce a Carbon Star
        if mass.0 >= 1.5 && mass.0 <= 4.0 {
            body.body_type = BodyType::CarbonStar;
            body.name = if is_gmc {
                format!("{} (Carbon Star)", body.name)
            } else {
                "The Star (Carbon Star AGB)".to_string()
            };
            temp.0 = 2400.0;
        } else if !is_gmc {
            body.name = "The Star (AGB Supergiant)".to_string();
        }
        evo.phase_timer_years = 0.0;
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "AGB evolution computes initial-final WD mass-loss rate on planetary nebula entry"
)]
fn step_agb_evolution(
    progenitor_mass: f64,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    dt_yr: f64,
    is_gmc: bool,
) {
    let target_r = if is_gmc { 0.065 } else { 1.50 };
    let k = (1.0 - (-0.010 * dt_yr).exp()).clamp(0.0, 1.0);
    radius.0 += (target_r - radius.0) * k;
    lum.0 += (3500.0 - lum.0) * k;
    let target_temp = if body.body_type == BodyType::CarbonStar {
        2400.0
    } else {
        2900.0
    };
    temp.0 += (target_temp - temp.0) * k;

    if evo.phase_timer_years > 2500.0 {
        evo.phase = StellarEvolutionPhase::PlanetaryNebulaEjection;
        if !is_gmc {
            body.name = "The Star (Planetary Nebula Ejection)".to_string();
        }
        evo.nebula_expansion_radius_au = if is_gmc { 0.5 } else { 1.6 };
        evo.nebula_opacity = 1.0;
        evo.phase_timer_years = 0.0;
        // Cache constant IFMR mass-loss rate from initial progenitor mass once so small substeps
        // converge accurately to 0.47 + 0.08 * M_init instead of collapsing to the 0.511 M_sun fixed point.
        let target_wd_mass = (0.47 + 0.08 * progenitor_mass).clamp(0.50, 1.20);
        let remaining_expansion_yr =
            f64::from(80.0 - evo.nebula_expansion_radius_au).max(1.0) / 5.2;
        evo.envelope_mass_loss_rate =
            (progenitor_mass - target_wd_mass).max(0.0) / remaining_expansion_yr;
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Stellar evolution phase logic requires access to all core thermodynamic components"
)]
fn step_red_supergiant_evolution(
    entity: Entity,
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    ignition: &mut IgnitionState,
    evo: &mut StellarEvolutionState,
    dt_yr: f64,
    is_gmc: bool,
    supernova_events: &mut MessageWriter<SupernovaEvent>,
) {
    let target_r = if is_gmc {
        (0.085 * (mass.0 / 15.0).powf(0.4)).clamp(0.06, 0.14)
    } else {
        (3.2 * (mass.0 / 15.0).powf(0.4)).clamp(1.8, 4.5)
    };
    let k = (1.0 - (-0.012 * dt_yr).exp()).clamp(0.0, 1.0);
    radius.0 += (target_r - radius.0) * k;
    lum.0 += (80_000.0 * (mass.0 / 15.0).powf(2.0) - lum.0) * k;
    temp.0 += (3300.0 - temp.0) * k;

    let rsg_limit = if is_gmc { 600.0 } else { 2000.0 };
    if evo.phase_timer_years > rsg_limit {
        let is_black_hole = mass.0 >= 25.0;
        let is_fast_rotator = body.name.contains("Fast")
            || body.name.contains("Magnetar")
            || (mass.0 >= 18.0 && mass.0 < 25.0);

        evo.phase = StellarEvolutionPhase::SupernovaExplosion;
        evo.phase_timer_years = 0.0;
        evo.nebula_expansion_radius_au = (radius.0 * 1.2) as f32;
        evo.nebula_opacity = 1.0;
        ignition.is_ignited = false;

        let remnant_type = if is_black_hole {
            BodyType::BlackHole
        } else if is_fast_rotator {
            // Rapid rotation drives convective dynamo amplification into a Magnetar
            BodyType::Magnetar
        } else {
            BodyType::Pulsar
        };

        let remnant_mass = if is_black_hole {
            if mass.0 >= 250.0 {
                // Direct collapse IMBH seed without explosive envelope ejection
                mass.0 * 0.92
            } else if mass.0 >= 100.0 {
                // Massive fallback IMBH seed
                mass.0 * 0.75
            } else if mass.0 >= 40.0 {
                // Heavy fallback black hole
                mass.0 * 0.55
            } else {
                // Standard core-collapse black hole
                (mass.0 * 0.35).clamp(3.5, 25.0)
            }
        } else if remnant_type == BodyType::Magnetar {
            1.65
        } else {
            1.44
        };
        let ejected_mass = (mass.0 - remnant_mass).max(0.1);
        let ejected_metals = ejected_mass * 0.25;

        supernova_events.write(SupernovaEvent {
            star_entity: entity,
            star_name: body.name.clone(),
            initial_mass_solar: mass.0,
            remnant_mass_solar: remnant_mass,
            remnant_type,
            shockwave_velocity_km_s: 15_000.0,
            ejected_metals_solar: ejected_metals,
            ejected_composition: Composition::supernova_ejecta(),
        });

        mass.0 = remnant_mass;
        body.body_type = remnant_type;
        if is_black_hole {
            radius.0 = (1.974e-8 * remnant_mass).max(1e-7);
            temp.0 = 10.0;
            lum.0 = 0.0;
            let bh_class = if remnant_mass >= 100.0 {
                "Intermediate-Mass Black Hole"
            } else {
                "Stellar-Mass Black Hole"
            };
            body.name = format!("{bh_class} ({remnant_mass:.1} M☉)");
        } else {
            radius.0 = 0.0001;
            temp.0 = if remnant_type == BodyType::Magnetar {
                2_000_000.0
            } else {
                1_000_000.0
            };
            lum.0 = 120.0;
            body.name = if remnant_type == BodyType::Magnetar {
                format!("Magnetar Remnant ({remnant_mass:.2} M☉)")
            } else {
                format!("Pulsar Remnant ({remnant_mass:.2} M☉)")
            };
        }
    }
}

fn step_stellar_remnant_evolution(
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    dt_yr: f64,
) {
    match evo.phase {
        StellarEvolutionPhase::SupernovaExplosion => {
            let expand_rate = 120.0;
            evo.nebula_expansion_radius_au += expand_rate * dt_yr as f32;
            evo.nebula_opacity = (1.0 - (evo.nebula_expansion_radius_au / 200.0)).clamp(0.0, 1.0);

            let is_bh = body.body_type == BodyType::BlackHole || mass.0 >= 25.0;
            let target_core_r = if is_bh {
                (1.974e-8 * mass.0).max(1e-7)
            } else {
                0.0001
            };
            let k_collapse = (1.0 - (-0.05 * dt_yr).exp()).clamp(0.0, 1.0);
            radius.0 += (target_core_r - radius.0) * k_collapse;

            if evo.nebula_expansion_radius_au >= 120.0 || evo.phase_timer_years >= 1500.0 {
                if is_bh {
                    evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
                    body.body_type = BodyType::BlackHole;
                    let bh_class = if mass.0 >= 100.0 {
                        "Intermediate-Mass Black Hole"
                    } else {
                        "Stellar-Mass Black Hole"
                    };
                    body.name = format!("{bh_class} ({:.1} M☉)", mass.0);
                    radius.0 = (1.974e-8 * mass.0).max(1e-7);
                    temp.0 = 10.0;
                    lum.0 = 0.0;
                } else if body.body_type == BodyType::Magnetar {
                    evo.phase = StellarEvolutionPhase::MagnetarRemnant;
                    mass.0 = mass.0.max(1.65);
                    body.name = format!("Magnetar Remnant ({:.2} M☉)", mass.0);
                    radius.0 = 0.0001;
                    temp.0 = 2_000_000.0;
                    lum.0 = 150.0;
                } else {
                    evo.phase = StellarEvolutionPhase::NeutronStarPulsar;
                    body.body_type = BodyType::Pulsar;
                    mass.0 = mass.0.max(1.44);
                    body.name = format!("Pulsar Remnant ({:.2} M☉)", mass.0);
                    radius.0 = 0.0001;
                    temp.0 = 1_000_000.0;
                    lum.0 = 100.0;
                }
            }
        }
        StellarEvolutionPhase::PlanetaryNebulaEjection => {
            let expand_rate_au_per_yr = 5.2;
            let prev_r = evo.nebula_expansion_radius_au;

            // Initialize cached constant IFMR loss rate once if entered directly without AGB transition
            if evo.envelope_mass_loss_rate <= 0.0 && mass.0 > 0.50 {
                let target_wd_mass = (0.47 + 0.08 * mass.0).clamp(0.50, 1.20);
                let remaining_yr =
                    f64::from((80.0 - prev_r).max(1.0)) / f64::from(expand_rate_au_per_yr);
                evo.envelope_mass_loss_rate = (mass.0 - target_wd_mass).max(0.0) / remaining_yr;
            }

            evo.nebula_expansion_radius_au += expand_rate_au_per_yr * dt_yr as f32;
            evo.nebula_opacity = (1.0 - (evo.nebula_expansion_radius_au / 80.0)).clamp(0.0, 1.0);

            let remaining_yr =
                f64::from((80.0 - prev_r).max(0.0)) / f64::from(expand_rate_au_per_yr);
            let dt_loss = dt_yr.min(remaining_yr);
            if evo.envelope_mass_loss_rate > 0.0 && dt_loss > 0.0 {
                mass.0 = (mass.0 - evo.envelope_mass_loss_rate * dt_loss).max(0.50);
            }

            if evo.nebula_expansion_radius_au >= 80.0 || evo.phase_timer_years >= 6000.0 {
                if mass.0 > 1.20 {
                    mass.0 = (0.47 + 0.08 * mass.0).clamp(0.50, 1.20);
                }
                evo.envelope_mass_loss_rate = 0.0;
                evo.phase = StellarEvolutionPhase::WhiteDwarf;
                body.body_type = BodyType::WhiteDwarf;
                body.name = "The Star (White Dwarf Remnant)".to_string();
                radius.0 = 0.009 * SOLAR_RADIUS_AU;
                temp.0 = 30_000.0;
                lum.0 = (radius.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4);
            }
        }
        StellarEvolutionPhase::WhiteDwarf => {
            let cool_rate = 0.0001;
            temp.0 = (temp.0 - cool_rate * dt_yr).max(10.0);
            lum.0 = (radius.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4);

            // Over cosmic time (T < 100 K), White Dwarfs cool into degenerate Black Dwarfs
            if temp.0 < 100.0 && body.body_type != BodyType::BlackDwarf {
                body.body_type = BodyType::BlackDwarf;
                body.name = "The Star (Black Dwarf Remnant)".to_string();
                lum.0 = 0.0;
            }
        }
        StellarEvolutionPhase::NeutronStarPulsar => {
            let prev_temp = temp.0.max(1.0);
            let cool_rate = 0.001;
            temp.0 = (temp.0 - cool_rate * dt_yr).max(10_000.0);
            if temp.0 < prev_temp {
                lum.0 *= (temp.0 / prev_temp).powi(4);
            }

            // Accretion past 1.8 M_sun decongests nucleons into strange quark matter (Quark Star)
            if mass.0 >= 1.80 && mass.0 < 2.17 && body.body_type != BodyType::QuarkStar {
                body.body_type = BodyType::QuarkStar;
                if !body.name.contains("Quark Star") {
                    let base = body.name.split(" (").next().unwrap_or(&body.name);
                    body.name = format!("{base} (Strange Quark Star)");
                }
                temp.0 = 10_000_000.0;
            }
        }
        StellarEvolutionPhase::MagnetarRemnant => {
            let prev_temp = temp.0.max(1.0);
            let cool_rate = 0.001;
            temp.0 = (temp.0 - cool_rate * dt_yr).max(10_000.0);
            if temp.0 < prev_temp {
                lum.0 *= (temp.0 / prev_temp).powi(4);
            }
            // Magnetars preserve their ultra-strong magnetic field (B ~ 10^15 G) and Magnetar identity.
            // If massive (>= 1.80 M_sun), strange quark matter core deconfinement forms a Quark Magnetar.
            // Accretion exceeding the Tolman-Oppenheimer-Volkoff limit (> 2.17 M_sun) collapses to a Black Hole.
        }
        _ => {}
    }
}
