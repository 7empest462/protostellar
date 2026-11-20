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
    reason = "Hayashi pre-main-sequence contraction computes temperature, radius, and ZAMS ignition"
)]
fn advance_pre_main_sequence_contraction(
    entity: Entity,
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    ignition: &mut IgnitionState,
    body: &mut CelestialBody,
    opt_evo: &mut Option<Mut<'_, StellarEvolutionState>>,
    dt_yr: f64,
    elapsed_years: f64,
    commands: &mut Commands,
    ignition_events: &mut MessageWriter<StarIgnitionEvent>,
    is_genesis: bool,
    is_gmc: bool,
) {
    let heating_rate_per_yr = if is_genesis {
        2.0e5 * mass.0 * 0.0024
    } else {
        2.0e5 * mass.0
    };
    ignition.core_temperature += heating_rate_per_yr * dt_yr;

    let ignition_threshold = 1.0e7;
    ignition.fusion_fraction =
        (ignition.core_temperature / ignition_threshold).clamp(0.0, 1.0) as f32;

    let ff = f64::from(ignition.fusion_fraction);

    if mass.0 < 0.08 && elapsed_years >= 30.0 && !is_genesis {
        body.body_type = BodyType::BrownDwarf;
    } else if mass.0 >= 0.08 && ff > 0.40 && ff < 0.98 && body.body_type == BodyType::Protostar {
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
    radius.0 = if mass.0 < 0.08 {
        (SOLAR_RADIUS_AU * (0.10 + 0.15 * (1.0 - ff))).max(0.082 * SOLAR_RADIUS_AU)
    } else if is_genesis {
        let r_ms = (SOLAR_RADIUS_AU * mass.0.powf(0.8)).clamp(0.001, 0.20);
        let r_initial = SOLAR_RADIUS_AU * 2.75;
        let contraction_progress = ((ff - 0.68) / (1.0 - 0.68)).clamp(0.0, 1.0);
        r_initial + (r_ms - r_initial) * contraction_progress
    } else {
        SOLAR_RADIUS_AU * (1.0 + 2.0 * (1.0 - ff))
    };
    lum.0 = (radius.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4);

    // Sub-stellar objects (< 0.08 M_sun) cannot ignite sustained core hydrogen fusion
    let should_ignite = mass.0 >= 0.08
        && if is_genesis {
            ignition.core_temperature >= ignition_threshold || elapsed_years >= 6_000.0
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
        advance_pre_main_sequence_contraction(
            entity,
            mass,
            radius,
            temp,
            lum,
            ignition,
            body,
            opt_evo,
            dt_yr,
            elapsed_years,
            commands,
            ignition_events,
            is_genesis,
            is_gmc,
        );
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
        commands,
        supernova_events,
    );

    // Sub-stellar Brown Dwarfs with IgnitionState that subsequently accrete past 0.08 M_sun
    // immediately ignite hydrogen fusion into the matching Morgan-Keenan class.
    if body.body_type == BodyType::BrownDwarf && mass.0 >= 0.08 {
        let (target_type, label) =
            classify_stellar_type_and_label(mass.0, &Composition::solar_gas());
        body.body_type = target_type;
        update_promoted_stellar_name(&mut body.name, target_type, mass.0, label);
        ignition.is_ignited = true;
        ignition.fusion_fraction = 1.0;
        lum.0 = mass.0.powf(3.5);
        temp.0 = (5778.0 * mass.0.powf(0.505)).clamp(2800.0, 50_000.0);
        radius.0 = (SOLAR_RADIUS_AU * mass.0.powf(0.8)).clamp(0.001, 0.12);
        if let Some(evo) = opt_evo {
            evo.phase = StellarEvolutionPhase::MainSequence;
            evo.hydrogen_core_fraction = 1.0;
        }
    }
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
    } else {
        match classify_stellar_type_and_label(mass.0, &Composition::solar_gas()).0 {
            BodyType::BrownDwarf => (BodyType::BrownDwarf, "The Star (Brown Dwarf - L/T)"),
            BodyType::RedDwarf => (BodyType::RedDwarf, "The Star (Red Dwarf - M Type)"),
            BodyType::OrangeDwarf => (BodyType::OrangeDwarf, "The Star (Orange Dwarf - K Type)"),
            BodyType::YellowDwarf => (BodyType::YellowDwarf, "The Star (Yellow Dwarf - G2V)"),
            BodyType::YellowWhiteDwarf => (
                BodyType::YellowWhiteDwarf,
                "The Star (Yellow-White Dwarf - F Type)",
            ),
            BodyType::WhiteMainSequence => (
                BodyType::WhiteMainSequence,
                "The Star (White Main-Sequence - A Type)",
            ),
            BodyType::BlueGiant => (BodyType::BlueGiant, "The Star (Blue Giant - B Type)"),
            BodyType::BlueSupergiant => (
                BodyType::BlueSupergiant,
                "The Star (Blue Supergiant - O Type)",
            ),
            BodyType::BlackHole => (BodyType::BlackHole, "Intermediate-Mass Black Hole"),
            _ => (BodyType::Hypergiant, "The Star (Luminous Hypergiant)"),
        }
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

    if let Some(evo) = opt_evo {
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

/// Removes planetary surface/atmosphere/ring components and attaches a Kerr metric black hole.
pub fn attach_black_hole_remnant_components(
    commands: &mut Commands,
    entity: Entity,
    mass_solar: f64,
) {
    if let Ok(mut cmd) = commands.get_entity(entity) {
        cmd.remove::<PlanetaryRingSystem>();
        cmd.remove::<PlanetaryClimate>();
        cmd.remove::<BiosphereState>();
        cmd.try_insert(crate::simulation::relativity::kerr::KerrBlackHole::new(
            mass_solar,
            0.70,
            -bevy::math::DVec3::Y,
        ));
    }
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
    commands: &mut Commands,
    supernova_events: &mut MessageWriter<SupernovaEvent>,
) {
    if !body.body_type.is_remnant()
        && !matches!(
            body.body_type,
            BodyType::QuasiStar | BodyType::DarkStar | BodyType::PopulationIIIStar
        )
        && mass.0 >= 500.0
    {
        // Direct pair-instability / general-relativistic collapse beyond 500 M_sun
        body.body_type = BodyType::BlackHole;
        body.name = format!("Intermediate-Mass Black Hole ({:.1} M☉)", mass.0);
        radius.0 = (1.974e-8 * mass.0).max(1e-7);
        temp.0 = 10.0;
        lum.0 = 0.0;
        ignition.is_ignited = false;
        if let Some(evo) = opt_evo {
            evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
        }
        if let Some(mut em) = opt_em {
            em.magnetic_field_gauss = 1.0e8;
            em.rotation_period_sec = 0.001;
            em.jet_length_au = 6.0;
            em.synchrotron_intensity = 3.0;
        }
        attach_black_hole_remnant_components(commands, entity, mass.0);
        return;
    }

    if body.body_type == BodyType::WhiteDwarf && mass.0 > 1.44 {
        // Chandrasekhar limit exceeded -> Type Ia thermonuclear / core collapse
        body.body_type = BodyType::Pulsar;
        body.name = "The Star (Pulsar Remnant)".to_string();
        radius.0 = 0.0001;
        temp.0 = 1_000_000.0;
        lum.0 = 100.0;
        ignition.is_ignited = false;
        if let Some(evo) = opt_evo {
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
    } else if matches!(body.body_type, BodyType::NeutronStar | BodyType::Pulsar)
        && mass.0 > 1.80
        && mass.0 <= 2.17
    {
        // High-density nucleonic deconfinement into Strange Quark Matter (Quark Star)
        if body.body_type != BodyType::QuarkStar {
            body.body_type = BodyType::QuarkStar;
            if !body.name.contains("Quark Star") {
                let base = body.name.split(" (").next().unwrap_or(&body.name);
                body.name = format!("{base} (Strange Quark Star)");
            }
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
        if let Some(evo) = opt_evo {
            evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
        }
        if let Some(mut em) = opt_em {
            em.magnetic_field_gauss = 1.0e8;
            em.rotation_period_sec = 0.001;
            em.jet_length_au = 6.0;
            em.synchrotron_intensity = 3.0;
        }
        attach_black_hole_remnant_components(commands, entity, mass.0);
    }
}

/// Updates a non-canonical body's name after crossing a sub-stellar or Morgan-Keenan spectral mass threshold.
pub fn update_promoted_stellar_name(
    name: &mut String,
    updated_type: BodyType,
    mass_solar: f64,
    label: &str,
) {
    if name.starts_with("Protostar Gen-I Jeans-") {
        let idx = name.trim_start_matches("Protostar Gen-I Jeans-");
        *name = format!("{label} Gen-I #{idx}");
    } else if name.starts_with("Protostar Jeans-") {
        let idx = name.trim_start_matches("Protostar Jeans-");
        *name = format!("{label} #{idx}");
    } else if name.starts_with("Protostar Gen-II NovaCore-") {
        let idx = name.trim_start_matches("Protostar Gen-II NovaCore-");
        *name = format!("{label} Gen-II #{idx}");
    } else if name.starts_with("Protostar Gen-III SolCore-") {
        let idx = name.trim_start_matches("Protostar Gen-III SolCore-");
        *name = format!("{label} Gen-III #{idx}");
    } else if name.starts_with("Gen-I Dark Star Protostar-") {
        let idx = name.trim_start_matches("Gen-I Dark Star Protostar-");
        *name = format!("Dark Star Gen-I #{idx}");
    } else if let Some(hash_idx) = name.find('#')
        && let Some(number_str) = name.get(hash_idx..)
    {
        *name = format!("{label} {number_str}");
    } else if !crate::simulation::accretion::collisions::is_canonical_named_body(name)
        && !name.starts_with("The Star")
        && !name.contains("Dark Star")
    {
        if updated_type == BodyType::BlackHole {
            let bh_class = if mass_solar >= 100.0 {
                "Intermediate-Mass Black Hole"
            } else {
                "Stellar-Mass Black Hole"
            };
            *name = format!("{bh_class} ({mass_solar:.1} M☉)");
        } else if updated_type == BodyType::BrownDwarf {
            let mj = mass_solar / JUPITER_MASS_SOLAR;
            *name = format!("Brown Dwarf ({mj:.1} M_J)");
        } else {
            *name = format!("{label} ({mass_solar:.1} M☉)");
        }
    }
}

pub(super) fn classify_ignited_protostar_to_zams(
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
    } else {
        classify_stellar_type_and_label(mass_solar, &Composition::solar_gas())
    };

    body.body_type = assigned_type;
    update_promoted_stellar_name(&mut body.name, assigned_type, mass_solar, label);

    if assigned_type == BodyType::BlackHole {
        lum.0 = 0.0;
        temp.0 = 10.0;
        radius.0 = (1.974e-8 * mass_solar).max(1e-7);
    } else {
        lum.0 = mass_solar.powf(3.5);
        temp.0 = (5778.0 * mass_solar.powf(0.505)).clamp(1800.0, 50_000.0);
        radius.0 = (SOLAR_RADIUS_AU * mass_solar.powf(0.8)).clamp(0.001, 0.12);
    }
}

pub(super) fn clean_star_base_name(name: &str) -> &str {
    if let Some(idx) = name.find('(') {
        name.get(..idx).unwrap_or(name).trim()
    } else {
        name.trim()
    }
}

pub use super::stellar_evolution::step_stellar_evolution_cycle;
