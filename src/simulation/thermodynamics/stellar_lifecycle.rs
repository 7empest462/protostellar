//! Stellar lifecycle, protostar ignition, and evolutionary branch transitions.
//!
//! Models full Morgan-Keenan spectral classification (O, B, A, F, G, K, M),
//! pre-main-sequence Hayashi contraction (T Tauri, Herbig Ae/Be),
//! radiation-driven envelope stripping into Wolf-Rayet and Luminous Blue Variables (LBV),
//! convective carbon-dredging into Carbon Stars,
//! dynamo-driven Magnetar births, Strange Quark Star phase transitions,
//! and far-future Black Dwarf cooling.

use bevy::prelude::*;

use super::StarIgnitionEvent;
use crate::simulation::components::*;
use crate::simulation::resources::SimulationConfig;
use crate::utils::constants::*;

#[allow(
    clippy::too_many_arguments,
    reason = "Protostar ignition system manages all physical and electromagnetic stellar state components"
)]
pub fn step_protostar_ignition_and_limits(
    entity: Entity,
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    ignition: &mut IgnitionState,
    body: &mut CelestialBody,
    opt_evo: &mut Option<Mut<'_, StellarEvolutionState>>,
    opt_em: Option<Mut<'_, ElectromagneticFieldState>>,
    config: &mut SimulationConfig,
    dt_yr: f64,
    elapsed_years: f64,
    commands: &mut Commands,
    ignition_events: &mut MessageWriter<StarIgnitionEvent>,
    supernova_events: &mut MessageWriter<SupernovaEvent>,
    is_gmc: bool,
) {
    // Degenerate stellar remnants (Black Holes, Neutron Stars, Pulsars, Magnetars, White Dwarfs)
    // have completed stellar fusion and must never undergo pre-main-sequence contraction or re-ignition!
    if body.body_type.is_remnant() {
        return;
    }

    let is_genesis = body.name.contains("Genesis") || body.name.contains("T-Tauri");
    if ignition.is_ignited {
        let blast_speed = 0.65;
        ignition.shockwave_radius = (ignition.shockwave_radius + blast_speed * dt_yr).min(30.0);
        if !is_genesis && !is_gmc {
            let time_decay = (1.0 - (elapsed_years / 15_000.0)).clamp(0.0, 1.0) as f32;
            config.gas_density_scale = config.gas_density_scale.min(time_decay);
        }
    } else {
        let heating_rate_per_yr = if is_genesis {
            2.0e5 * mass.0 * 0.04
        } else {
            2.0e5 * mass.0
        };
        ignition.core_temperature += heating_rate_per_yr * dt_yr;

        let ignition_threshold = 1.0e7;
        ignition.fusion_fraction =
            (ignition.core_temperature / ignition_threshold).clamp(0.0, 1.0) as f32;

        let ff = f64::from(ignition.fusion_fraction);

        // Pre-Main-Sequence (PMS) intermediate classification during contraction
        if ff > 0.40 && ff < 0.98 && body.body_type == BodyType::Protostar {
            if mass.0 < 2.0 {
                body.body_type = BodyType::TTauriStar;
            } else if mass.0 < 8.0 {
                body.body_type = BodyType::HerbigAeBeStar;
            }
        }

        let target_surface_temp = if mass.0 < 0.08 {
            1800.0 + 1000.0 * ff
        } else if mass.0 < 0.45 {
            2600.0 + 1200.0 * ff
        } else if mass.0 < 8.0 {
            if is_genesis {
                let contraction_progress = ((ff - 0.68) / (1.0 - 0.68)).clamp(0.0, 1.0);
                let final_temp = 5778.0 * mass.0.powf(0.505);
                4800.0 + (final_temp - 4800.0) * contraction_progress
            } else {
                3200.0 + 2578.0 * ff
            }
        } else {
            8000.0 + 20000.0 * ff
        };
        temp.0 = target_surface_temp;
        radius.0 = if is_genesis {
            let r_ms = (SOLAR_RADIUS_AU * mass.0.powf(0.8)).clamp(0.001, 0.20);
            let r_initial = SOLAR_RADIUS_AU * 2.75;
            let contraction_progress = ((ff - 0.68) / (1.0 - 0.68)).clamp(0.0, 1.0);
            r_initial + (r_ms - r_initial) * contraction_progress
        } else {
            SOLAR_RADIUS_AU * (1.0 + 2.0 * (1.0 - ff))
        };

        let should_ignite = if is_genesis {
            ignition.core_temperature >= ignition_threshold || elapsed_years >= 350.0
        } else {
            ignition.core_temperature >= ignition_threshold || elapsed_years >= 30.0
        };

        if should_ignite {
            ignite_protostar_to_zams(
                entity,
                mass,
                radius,
                temp,
                lum,
                ignition,
                body,
                opt_evo,
                commands,
                ignition_events,
                is_genesis,
                is_gmc,
                ignition_threshold,
            );
        }
    }

    apply_stellar_collapse_limits(
        entity,
        mass,
        radius,
        temp,
        lum,
        body,
        ignition,
        opt_evo,
        opt_em,
        supernova_events,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Ignition transition to ZAMS coordinates mass, radius, temperature, luminosity, and events"
)]
fn ignite_protostar_to_zams(
    entity: Entity,
    mass: &Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    ignition: &mut IgnitionState,
    body: &mut CelestialBody,
    opt_evo: &mut Option<Mut<'_, StellarEvolutionState>>,
    commands: &mut Commands,
    ignition_events: &mut MessageWriter<StarIgnitionEvent>,
    is_genesis: bool,
    is_gmc: bool,
    ignition_threshold: f64,
) {
    ignition.is_ignited = true;
    ignition.fusion_fraction = 1.0;
    ignition.core_temperature = ignition.core_temperature.max(ignition_threshold);
    ignition.shockwave_radius = 0.5;

    // Full Morgan-Keenan spectral classification assignment on Zero-Age Main Sequence (ZAMS)
    let (assigned_type, name_str) = if is_genesis {
        (BodyType::YellowDwarf, "The Sun (Yellow Dwarf - G2V)")
    } else if (body.name.contains("Gen-I") || body.name.contains("Pop-III")) && mass.0 >= 30.0 {
        (
            BodyType::PopulationIIIStar,
            "Population III Hypergiant (Pop-III)",
        )
    } else if body.name.contains("Dark Star") {
        (BodyType::DarkStar, "Dark Star (WIMP Annihilation)")
    } else if mass.0 < 0.08 {
        (BodyType::BrownDwarf, "The Star (Brown Dwarf - L/T)")
    } else if mass.0 < 0.45 {
        (BodyType::RedDwarf, "The Star (Red Dwarf - M Type)")
    } else if mass.0 < 0.80 {
        (BodyType::OrangeDwarf, "The Star (Orange Dwarf - K Type)")
    } else if mass.0 < 1.04 {
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
    let is_cluster = is_gmc
        || body.name.contains("Protostar")
        || body.name.contains("Jeans")
        || body.name.contains("NovaCore");
    if !is_genesis && !is_cluster {
        body.name = name_str.to_string();
    } else if is_cluster && !body.name.contains('(') {
        body.name = format!("{} ({:?})", body.name, assigned_type);
    }

    let main_seq_lum = mass.0.powf(3.5);
    lum.0 = main_seq_lum;
    temp.0 = 5778.0 * mass.0.powf(0.505);
    radius.0 = (SOLAR_RADIUS_AU * mass.0.powf(0.8)).clamp(0.001, 0.20);

    if let Some(ref mut evo) = opt_evo {
        evo.phase = StellarEvolutionPhase::MainSequence;
        evo.hydrogen_core_fraction = 1.0;
    }

    commands
        .entity(entity)
        .try_insert(ElectromagneticFieldState {
            magnetic_field_gauss: 1.0 * mass.0,
            rotation_period_sec: 25.0 * 86400.0,
            magnetic_inclination_rad: 0.12,
            jet_length_au: 0.0,
            synchrotron_intensity: 0.0,
        });

    ignition_events.write(StarIgnitionEvent {
        star_entity: entity,
        star_mass: mass.0,
        luminosity_l_sun: lum.0,
        surface_temp_kelvin: temp.0,
    });
}

#[allow(
    clippy::too_many_arguments,
    reason = "Stellar collapse limits check Chandrasekhar and TOV degenerate core mass boundaries"
)]
fn apply_stellar_collapse_limits(
    entity: Entity,
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    ignition: &mut IgnitionState,
    opt_evo: &mut Option<Mut<'_, StellarEvolutionState>>,
    opt_em: Option<Mut<'_, ElectromagneticFieldState>>,
    supernova_events: &mut MessageWriter<SupernovaEvent>,
) {
    if body.body_type == BodyType::WhiteDwarf && mass.0 > 1.44 {
        // Chandrasekhar limit exceeded -> Type Ia thermonuclear / core collapse
        body.body_type = BodyType::Pulsar;
        body.name = "The Star (Pulsar Remnant)".to_string();
        radius.0 = 0.0001;
        temp.0 = 1_000_000.0;
        lum.0 = 100.0;
        ignition.is_ignited = false;
        if let Some(ref mut evo) = opt_evo {
            evo.phase = StellarEvolutionPhase::NeutronStarPulsar;
            evo.nebula_expansion_radius_au = 2.0;
            evo.nebula_opacity = 1.0;
        }
        if let Some(mut em) = opt_em {
            em.magnetic_field_gauss = 1.0e12;
            em.rotation_period_sec = 0.033;
            em.jet_length_au = 3.5;
            em.synchrotron_intensity = 1.8;
        }
        let ejected_mass = (mass.0 - 1.40).max(0.1);
        supernova_events.write(SupernovaEvent {
            star_entity: entity,
            star_name: body.name.clone(),
            initial_mass_solar: mass.0,
            remnant_mass_solar: 1.40,
            remnant_type: BodyType::Pulsar,
            shockwave_velocity_km_s: 12_000.0,
            ejected_metals_solar: ejected_mass * 0.25,
            ejected_composition: Composition::supernova_ejecta(),
        });
        mass.0 = 1.40;
    } else if matches!(
        body.body_type,
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
    ) && mass.0 > 1.80
        && mass.0 <= 2.17
    {
        // High-density nucleonic deconfinement into Strange Quark Matter (Quark Star)
        if body.body_type != BodyType::QuarkStar {
            body.body_type = BodyType::QuarkStar;
            body.name = format!("{} (Strange Quark Star)", body.name);
            temp.0 = 10_000_000.0;
            lum.0 = 180.0;
        }
    } else if matches!(
        body.body_type,
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar | BodyType::QuarkStar
    ) && mass.0 > 2.17
    {
        // Tolman-Oppenheimer-Volkoff (TOV) limit exceeded -> Direct collapse into Black Hole
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
        ignition.is_ignited = false;
        if let Some(ref mut evo) = opt_evo {
            evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
        }
        if let Some(mut em) = opt_em {
            em.magnetic_field_gauss = 1.0e8;
            em.rotation_period_sec = 0.001;
            em.jet_length_au = 6.0;
            em.synchrotron_intensity = 3.0;
        }
    }
}

fn classify_ignited_protostar_to_zams(
    mass_solar: f64,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
) {
    let (assigned_type, label) = if body.name.contains("Dark Star") {
        (BodyType::DarkStar, "Dark Star")
    } else if (body.name.contains("Gen-I") || body.name.contains("Pop-III")) && mass_solar >= 30.0 {
        (BodyType::PopulationIIIStar, "Pop-III Hypergiant")
    } else if mass_solar < 0.08 {
        (BodyType::BrownDwarf, "Brown Dwarf")
    } else if mass_solar < 0.45 {
        (BodyType::RedDwarf, "Red Dwarf")
    } else if mass_solar < 0.80 {
        (BodyType::OrangeDwarf, "Orange Dwarf")
    } else if mass_solar < 1.08 {
        (BodyType::YellowDwarf, "Yellow Dwarf")
    } else if mass_solar < 1.40 {
        (BodyType::YellowWhiteDwarf, "F-Type Star")
    } else if mass_solar < 2.10 {
        (BodyType::WhiteMainSequence, "A-Type White Star")
    } else if mass_solar < 8.0 {
        (BodyType::BlueGiant, "B-Type Blue Giant")
    } else if mass_solar < 25.0 {
        (BodyType::BlueSupergiant, "O-Type Blue Supergiant")
    } else {
        (BodyType::Hypergiant, "Luminous Hypergiant")
    };

    body.body_type = assigned_type;
    if body.name.starts_with("Protostar Gen-I Jeans-") {
        let idx = body.name.trim_start_matches("Protostar Gen-I Jeans-");
        body.name = format!("{label} Gen-I #{idx}");
    } else if body.name.starts_with("Protostar Jeans-") {
        let idx = body.name.trim_start_matches("Protostar Jeans-");
        body.name = format!("{label} #{idx}");
    } else if body.name.starts_with("Protostar Gen-II NovaCore-") {
        let idx = body.name.trim_start_matches("Protostar Gen-II NovaCore-");
        body.name = format!("{label} Gen-II #{idx}");
    } else if body.name.starts_with("Protostar Gen-III SolCore-") {
        let idx = body.name.trim_start_matches("Protostar Gen-III SolCore-");
        body.name = format!("{label} Gen-III #{idx}");
    } else if body.name.starts_with("Gen-I Dark Star Protostar-") {
        let idx = body.name.trim_start_matches("Gen-I Dark Star Protostar-");
        body.name = format!("Dark Star Gen-I #{idx}");
    }

    lum.0 = mass_solar.powf(3.5);
    temp.0 = 5778.0 * mass_solar.powf(0.505);
    radius.0 = (SOLAR_RADIUS_AU * mass_solar.powf(0.8)).clamp(0.001, 0.12);
}

fn clean_star_base_name(name: &str) -> &str {
    if let Some(idx) = name.find('(') {
        name.get(..idx).unwrap_or(name).trim()
    } else {
        name.trim()
    }
}

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
            step_agb_evolution(radius, temp, lum, body, evo, dt_yr, is_gmc);
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
    let target_lum = mass.0.powf(3.5);
    let target_temp = 5778.0 * mass.0.powf(0.505);
    let target_rad = (SOLAR_RADIUS_AU * mass.0.powf(0.8)).clamp(0.001, 0.12);
    let k = (1.0 - (-0.05 * dt_yr).exp()).clamp(0.0, 1.0);
    lum.0 += (target_lum - lum.0) * k;
    temp.0 += (target_temp - temp.0) * k;
    radius.0 += (target_rad - radius.0) * k;

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
    let fuel_burn_rate = (1.0 / main_seq_lifetime_yr) as f32;
    evo.hydrogen_core_fraction =
        (evo.hydrogen_core_fraction - fuel_burn_rate * dt_yr as f32).max(0.0);

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

    evo.helium_core_fraction = (evo.helium_core_fraction + 0.0003 * dt_yr as f32).min(1.0);
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

fn step_agb_evolution(
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
    temp.0 += (2900.0 - temp.0) * k;

    if evo.phase_timer_years > 2500.0 {
        evo.phase = StellarEvolutionPhase::PlanetaryNebulaEjection;
        if !is_gmc {
            body.name = "The Star (Planetary Nebula Ejection)".to_string();
        }
        evo.nebula_expansion_radius_au = if is_gmc { 0.5 } else { 1.6 };
        evo.nebula_opacity = 1.0;
        evo.phase_timer_years = 0.0;
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
        evo.phase = StellarEvolutionPhase::SupernovaExplosion;
        evo.phase_timer_years = 0.0;
        evo.nebula_expansion_radius_au = (radius.0 * 1.2) as f32;
        evo.nebula_opacity = 1.0;
        ignition.is_ignited = false;

        let is_black_hole = mass.0 >= 25.0;
        let is_fast_rotator = body.name.contains("Fast") || evo.phase_timer_years < 1000.0;

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
                    body.name = format!("Magnetar Remnant ({:.2} M☉)", mass.0);
                    mass.0 = 1.65;
                    radius.0 = 0.0001;
                    temp.0 = 2_000_000.0;
                    lum.0 = 150.0;
                } else {
                    evo.phase = StellarEvolutionPhase::NeutronStarPulsar;
                    body.body_type = BodyType::Pulsar;
                    body.name = format!("Pulsar Remnant ({:.2} M☉)", mass.0);
                    mass.0 = 1.44;
                    radius.0 = 0.0001;
                    temp.0 = 1_000_000.0;
                    lum.0 = 100.0;
                }
            }
        }
        StellarEvolutionPhase::PlanetaryNebulaEjection => {
            let expand_rate_au_per_yr = 5.2;
            evo.nebula_expansion_radius_au += expand_rate_au_per_yr * dt_yr as f32;
            evo.nebula_opacity = (1.0 - (evo.nebula_expansion_radius_au / 80.0)).clamp(0.0, 1.0);

            let shed_frac = (evo.phase_timer_years / 3000.0).clamp(0.0, 1.0);
            mass.0 = (1.0 - 0.45 * shed_frac).max(0.55);

            if evo.nebula_expansion_radius_au >= 80.0 || evo.phase_timer_years >= 6000.0 {
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
        StellarEvolutionPhase::NeutronStarPulsar | StellarEvolutionPhase::MagnetarRemnant => {
            let cool_rate = 0.001;
            temp.0 = (temp.0 - cool_rate * dt_yr).max(10_000.0);

            // Accretion past 1.8 M_sun decongests nucleons into strange quark matter (Quark Star)
            if mass.0 >= 1.80 && mass.0 < 2.17 && body.body_type != BodyType::QuarkStar {
                body.body_type = BodyType::QuarkStar;
                body.name = "The Star (Strange Quark Star)".to_string();
                temp.0 = 10_000_000.0;
            }
        }
        _ => {}
    }
}
