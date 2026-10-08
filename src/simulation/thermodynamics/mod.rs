//! Thermodynamics, Hayashi Track Protostellar Evolution, Core Dynamos, Planetary Climate, Biosphere Genesis, and Far-Future Stellar Metamorphosis.

mod climate;
pub use climate::*;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::rendering::effects::megastructures::DysonSwarm;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Event triggered when the central protostar's core reaches 10,000,000 K and ignites hydrogen fusion.
#[derive(Message, Debug, Clone)]
pub struct StarIgnitionEvent {
    pub star_entity: Entity,
    pub star_mass: f64,
    pub luminosity_l_sun: f64,
    pub surface_temp_kelvin: f64,
}

/// Updates stellar thermodynamics, core heating, planetary dynamos, greenhouse atmospheres, biospheres, and stellar evolution.
#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
pub fn update_thermodynamics(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut config: ResMut<SimulationConfig>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    mut ignition_events: MessageWriter<StarIgnitionEvent>,
    mut engulfment_events: MessageWriter<PlanetaryEngulfmentEvent>,
    mut supernova_events: MessageWriter<SupernovaEvent>,
    mut star_query: Query<
        (
            Entity,
            &mut Mass,
            &mut Radius,
            &mut Temperature,
            &mut Luminosity,
            &mut IgnitionState,
            &mut CelestialBody,
            Option<&mut StellarEvolutionState>,
            Option<&mut ElectromagneticFieldState>,
            Option<&DysonSwarm>,
            Has<CentralStar>,
        ),
        (With<IgnitionState>, With<CelestialBody>),
    >,
    mut bodies_query: Query<
        (
            Entity,
            &Mass,
            &SimPosition,
            &mut SimVelocity,
            &mut Temperature,
            &mut Composition,
            &CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&SpinState>,
            Option<&mut VolatileInventory>,
            Option<&mut PlanetaryClimate>,
            Option<&mut BiosphereState>,
        ),
        (Without<CentralStar>, Without<IgnitionState>),
    >,
) {
    if (!config.enable_thermodynamics || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr);
    let is_gmc = scenario_state.is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster
    });

    // 1. Process Protostellar Core Heating, Ignition, and Multi-Branch Stellar Evolution
    for (
        entity,
        mut mass,
        mut radius,
        mut temp,
        mut lum,
        mut ignition,
        mut body,
        mut opt_evo,
        opt_em,
        opt_swarm,
        is_central_star,
    ) in star_query.iter_mut()
    {
        // Quasi-Stars (JWST Little Red Dot) and Black Holes are exotic supermassive objects
        // governed by super-Eddington accretion physics and envelope mechanics in `accretion.rs`.
        // They must NOT undergo standard main-sequence contraction, hydrogen exhaustion, or radius clamping to 0.20 AU!
        if body.body_type == BodyType::QuasiStar || body.body_type == BodyType::BlackHole {
            continue;
        }

        step_protostar_ignition_and_limits(
            entity,
            &mut mass,
            &mut radius,
            &mut temp,
            &mut lum,
            &mut ignition,
            &mut body,
            &mut opt_evo,
            opt_em,
            &mut config,
            dt_yr,
            sim_time.elapsed_years,
            &mut commands,
            &mut ignition_events,
            &mut supernova_events,
            is_gmc,
        );

        if let Some(ref mut evo) = opt_evo {
            step_stellar_evolution_cycle(
                entity,
                &mut mass,
                &mut radius,
                &mut temp,
                &mut lum,
                &mut body,
                &mut ignition,
                evo,
                dt_yr,
                &mut supernova_events,
                is_gmc,
            );
        }

        // 2. Update Disk Body Temperatures & Planetary Thermal Processing
        if is_central_star {
            let mut star_lum = lum.0;
            if let Some(swarm) = opt_swarm {
                star_lum *= 1.0 - (f64::from(swarm.completion_fraction) * 0.9);
            }
            let star_temp = temp.0;
            let star_r = radius.0;
            let shockwave_r = ignition.shockwave_radius;

            update_disk_bodies_thermodynamics(
                &mut commands,
                &mut bodies_query,
                star_lum,
                star_temp,
                star_r,
                shockwave_r,
                ignition.is_ignited,
                dt_yr,
                sim_time.elapsed_years,
                &mut engulfment_events,
            );
        }
    }
}

#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Thermodynamics iteration passes massive query sets and state parameters"
)]
fn update_disk_bodies_thermodynamics(
    commands: &mut Commands,
    bodies_query: &mut Query<
        (
            Entity,
            &Mass,
            &SimPosition,
            &mut SimVelocity,
            &mut Temperature,
            &mut Composition,
            &CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&SpinState>,
            Option<&mut VolatileInventory>,
            Option<&mut PlanetaryClimate>,
            Option<&mut BiosphereState>,
        ),
        (Without<CentralStar>, Without<IgnitionState>),
    >,
    star_lum: f64,
    star_temp: f64,
    star_r: f64,
    shockwave_r: f64,
    is_ignited: bool,
    dt_yr: f64,
    elapsed_years: f64,
    engulfment_events: &mut MessageWriter<PlanetaryEngulfmentEvent>,
) {
    for (
        body_ent,
        b_mass,
        pos,
        mut vel,
        mut p_temp,
        mut comp,
        b_body,
        mut opt_diff,
        opt_spin,
        mut opt_vol,
        mut opt_climate,
        mut opt_bio,
    ) in bodies_query.iter_mut()
    {
        update_body_thermodynamics(
            commands,
            body_ent,
            b_mass.0,
            pos.0,
            &mut vel,
            &mut p_temp,
            &mut comp,
            b_body,
            &mut opt_diff,
            opt_spin,
            &mut opt_vol,
            &mut opt_climate,
            &mut opt_bio,
            star_lum,
            star_temp,
            star_r,
            shockwave_r,
            is_ignited,
            dt_yr,
            elapsed_years,
            engulfment_events,
        );
    }
}

fn step_protostar_ignition_and_limits(
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
            2.0e5 * mass.0 * 1e-4
        } else {
            2.0e5 * mass.0
        };
        ignition.core_temperature += heating_rate_per_yr * dt_yr;

        let ignition_threshold = 1.0e7;
        ignition.fusion_fraction =
            (ignition.core_temperature / ignition_threshold).clamp(0.0, 1.0) as f32;

        let ff = f64::from(ignition.fusion_fraction);
        let target_surface_temp = if mass.0 < 0.08 {
            1800.0 + 1000.0 * ff
        } else if mass.0 < 0.50 {
            2600.0 + 1200.0 * ff
        } else if mass.0 < 8.0 {
            if is_genesis {
                4800.0
            } else {
                3200.0 + 2578.0 * ff
            }
        } else {
            8000.0 + 20000.0 * ff
        };
        temp.0 = target_surface_temp;
        radius.0 = if is_genesis {
            radius.0
        } else {
            SOLAR_RADIUS_AU * (1.0 + 2.0 * (1.0 - ff))
        };

        let should_ignite = if is_genesis {
            ignition.core_temperature >= ignition_threshold
        } else {
            ignition.core_temperature >= ignition_threshold || elapsed_years >= 30.0
        };

        if should_ignite {
            ignition.is_ignited = true;
            ignition.fusion_fraction = 1.0;
            ignition.core_temperature = ignition.core_temperature.max(ignition_threshold);
            ignition.shockwave_radius = 0.5;

            let (assigned_type, name_str) = if mass.0 < 0.08 {
                (BodyType::BrownDwarf, "The Star (Brown Dwarf)")
            } else if mass.0 < 0.50 {
                (BodyType::RedDwarf, "The Star (Red Dwarf - M Type)")
            } else if mass.0 < 1.4 {
                (BodyType::YellowDwarf, "The Star (Yellow Dwarf - G2V)")
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
    ) && mass.0 > 2.17
    {
        body.body_type = BodyType::BlackHole;
        body.name = "The Star (Stellar-Mass Black Hole)".to_string();
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

fn step_stellar_evolution_cycle(
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

    match evo.phase {
        StellarEvolutionPhase::ProtostarContraction => {
            if ignition.is_ignited {
                evo.phase = StellarEvolutionPhase::MainSequence;
                evo.hydrogen_core_fraction = 1.0;
            }
        }
        StellarEvolutionPhase::MainSequence => {
            step_main_sequence_evolution(mass, radius, temp, lum, body, evo, dt_yr, is_gmc);
        }
        StellarEvolutionPhase::RedGiantBranch => {
            step_red_giant_evolution(mass, radius, temp, lum, body, evo, dt_yr);
        }
        StellarEvolutionPhase::HeliumFlashAgb => {
            step_agb_evolution(radius, temp, lum, body, evo, dt_yr);
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
    let target_rad = (SOLAR_RADIUS_AU * mass.0.powf(0.8)).clamp(0.001, 0.20);
    let k = (1.0 - (-0.05 * dt_yr).exp()).clamp(0.0, 1.0);
    lum.0 += (target_lum - lum.0) * k;
    temp.0 += (target_temp - temp.0) * k;
    radius.0 += (target_rad - radius.0) * k;

    // In cluster environments, massive stars live accelerated lifecycles (2,000 - 7,500 yr)
    // so core-collapse supernovae and enrichment occur naturally during simulation time warp.
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
        if mass.0 < 0.50 {
            evo.phase = StellarEvolutionPhase::WhiteDwarf;
            body.body_type = BodyType::WhiteDwarf;
            body.name = if is_gmc {
                format!("{} (White Dwarf)", body.name)
            } else {
                "The Star (Helium White Dwarf)".to_string()
            };
            radius.0 = 0.009;
            temp.0 = 25_000.0;
        } else if mass.0 < 8.0 {
            evo.phase = StellarEvolutionPhase::RedGiantBranch;
            body.body_type = BodyType::RedGiant;
            body.name = if is_gmc {
                format!("{} (Red Giant)", body.name)
            } else {
                "The Star (Red Giant Branch)".to_string()
            };
        } else {
            evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
            body.body_type = BodyType::RedSupergiant;
            body.name = if is_gmc {
                format!("{} (Red Supergiant)", body.name)
            } else {
                "The Star (Red Supergiant)".to_string()
            };
        }
    }
}

fn step_red_giant_evolution(
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    dt_yr: f64,
) {
    let target_r = (1.25 * mass.0.powf(0.3)).clamp(0.8, 2.5);
    let k = (1.0 - (-0.008 * dt_yr).exp()).clamp(0.0, 1.0);
    radius.0 += (target_r - radius.0) * k;
    temp.0 += (3100.0 - temp.0) * k;
    lum.0 += (2500.0 * mass.0 - lum.0) * k;

    evo.helium_core_fraction = (evo.helium_core_fraction + 0.0003 * dt_yr as f32).min(1.0);
    if evo.helium_core_fraction >= 1.0 || evo.phase_timer_years > 3000.0 {
        evo.phase = StellarEvolutionPhase::HeliumFlashAgb;
        body.name = "The Star (AGB Supergiant)".to_string();
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
) {
    let target_r = 1.50f64;
    let k = (1.0 - (-0.010 * dt_yr).exp()).clamp(0.0, 1.0);
    radius.0 += (target_r - radius.0) * k;
    lum.0 += (3500.0 - lum.0) * k;
    temp.0 += (2900.0 - temp.0) * k;

    if evo.phase_timer_years > 2500.0 {
        evo.phase = StellarEvolutionPhase::PlanetaryNebulaEjection;
        body.name = "The Star (Planetary Nebula Ejection)".to_string();
        evo.nebula_expansion_radius_au = 1.6;
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
    let target_r = (4.5 * (mass.0 / 15.0).powf(0.5)).clamp(2.5, 7.5);
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
        let remnant_type = if is_black_hole {
            BodyType::BlackHole
        } else {
            BodyType::Pulsar
        };
        let remnant_mass = if is_black_hole {
            (mass.0 * 0.25).clamp(3.0, 15.0)
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
        body.name = if is_gmc {
            format!("{} ({remnant_type:?} Remnant)", body.name)
        } else {
            format!("The Star ({remnant_type:?} Remnant)")
        };
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

            let target_core_r = if mass.0 >= 25.0 {
                (1.974e-8 * mass.0).max(1e-7)
            } else {
                0.0001
            };
            let k_collapse = (1.0 - (-0.05 * dt_yr).exp()).clamp(0.0, 1.0);
            radius.0 += (target_core_r - radius.0) * k_collapse;

            if evo.nebula_expansion_radius_au >= 120.0 || evo.phase_timer_years >= 1500.0 {
                if mass.0 >= 25.0 {
                    evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
                    body.body_type = BodyType::BlackHole;
                    body.name = "The Star (Stellar-Mass Black Hole)".to_string();
                    mass.0 = (mass.0 * 0.25).clamp(3.0, 15.0);
                    radius.0 = (1.974e-8 * mass.0).max(1e-7);
                    temp.0 = 10.0;
                    lum.0 = 0.0;
                } else {
                    evo.phase = StellarEvolutionPhase::NeutronStarPulsar;
                    body.body_type = BodyType::Pulsar;
                    body.name = "The Star (Pulsar Remnant)".to_string();
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
                radius.0 = 0.009;
                temp.0 = 30_000.0;
                lum.0 = (radius.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4);
            }
        }
        StellarEvolutionPhase::WhiteDwarf => {
            let cool_rate = 0.0001;
            temp.0 = (temp.0 - cool_rate * dt_yr).max(2000.0);
            lum.0 = (radius.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4);
        }
        StellarEvolutionPhase::NeutronStarPulsar | StellarEvolutionPhase::MagnetarRemnant => {
            let cool_rate = 0.001;
            temp.0 = (temp.0 - cool_rate * dt_yr).max(10_000.0);
        }
        _ => {}
    }
}

fn update_body_thermodynamics(
    commands: &mut Commands,
    body_ent: Entity,
    b_mass_solar: f64,
    pos: DVec3,
    vel: &mut SimVelocity,
    p_temp: &mut Temperature,
    comp: &mut Composition,
    b_body: &CelestialBody,
    opt_diff: &mut Option<Mut<'_, InternalDifferentiation>>,
    opt_spin: Option<&SpinState>,
    opt_vol: &mut Option<Mut<'_, VolatileInventory>>,
    opt_climate: &mut Option<Mut<'_, PlanetaryClimate>>,
    opt_bio: &mut Option<Mut<'_, BiosphereState>>,
    star_lum: f64,
    star_temp: f64,
    star_r: f64,
    shockwave_r: f64,
    star_is_ignited: bool,
    dt_yr: f64,
    elapsed_years: f64,
    engulfment_events: &mut MessageWriter<PlanetaryEngulfmentEvent>,
) {
    // Stars and stellar remnants are governed by stellar thermodynamics, not planetary engulfment or climate.
    if b_body.body_type.is_star_or_remnant() {
        return;
    }

    let r = pos.length().max(1e-4);
    let period_hrs = opt_spin.map_or(24.0, |s| s.rotation_period_hours);

    // Stellar envelope drag and engulfment for all stars (Main Sequence, Red Giants, etc.)
    if !b_body.body_type.is_star_or_remnant() && r < star_r * 2.0 {
        let drag_coeff = if r < star_r { 0.25 } else { 0.08 };
        vel.0 *= 1.0 - (drag_coeff * dt_yr).min(0.8);

        let is_deep_engulfment = if star_r > 0.15 {
            // For giant envelopes, deep plunge or inner core boundary
            r < 0.18 || r < star_r * 0.20
        } else {
            // For main-sequence / compact stars, penetration into or inside the stellar body
            r <= star_r || r < 0.005
        };

        if is_deep_engulfment {
            engulfment_events.write(PlanetaryEngulfmentEvent {
                planet_entity: body_ent,
                planet_name: b_body.name.clone(),
                distance_au: r,
                planet_mass_earth: b_mass_solar / EARTH_MASS_SOLAR,
            });
            if let Ok(mut cmd) = commands.get_entity(body_ent) {
                cmd.try_despawn();
            }
            return;
        }
    }

    let mut magnetic_field_gauss = 0.0f32;
    if let Some(ref mut diff) = opt_diff {
        if diff.is_differentiated {
            if diff.core_temp_k > 1200.0 {
                let temp_factor = ((diff.core_temp_k - 1200.0) / 2000.0).clamp(0.0, 1.5);
                let spin_factor = (24.0 / period_hrs.max(1.0)).sqrt().clamp(0.2, 3.0);
                let core_mass_frac = (diff.core_radius_au / (diff.mantle_radius_au.max(1e-5)))
                    .powi(3)
                    .clamp(0.05, 0.60);

                let b_gauss = (0.35
                    * (b_mass_solar / EARTH_MASS_SOLAR).sqrt().max(0.1)
                    * core_mass_frac.sqrt()
                    * spin_factor
                    * temp_factor.powf(0.33))
                .clamp(0.0, 5.0);

                diff.magnetic_field_gauss = b_gauss;
                magnetic_field_gauss = b_gauss as f32;
            } else {
                diff.magnetic_field_gauss = 0.0;
            }

            diff.core_temp_k = (diff.core_temp_k - (1.5e-3 * dt_yr)).max(300.0);
        }
    }

    if star_is_ignited && r < 3.5 {
        if let Some(ref mut vol) = opt_vol {
            if magnetic_field_gauss < 0.12 {
                let unshielded_factor = (1.0 - (magnetic_field_gauss / 0.12)).clamp(0.0, 1.0);
                let strip_rate =
                    (0.00015 * (1.0 / (r * r)) * f64::from(unshielded_factor) * dt_yr) as f32;
                vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar - strip_rate).max(0.0);
                if vol.atmospheric_pressure_bar <= 0.001 {
                    comp.gas_frac = (comp.gas_frac - f64::from(strip_rate * 0.0001)).max(0.0);
                }
            }
        }
    }

    if star_lum > 500.0 && (25.0..=65.0).contains(&r) && comp.ice_frac > 0.25 {
        if let Some(ref mut vol) = opt_vol {
            vol.ocean_coverage_frac = (vol.ocean_coverage_frac + 0.02 * dt_yr as f32).min(0.85);
            vol.atmospheric_pressure_bar =
                (vol.atmospheric_pressure_bar + 0.03 * dt_yr as f32).min(1.2);
        }
    }

    update_body_climate_and_biosphere(
        commands,
        body_ent,
        b_mass_solar,
        b_body,
        comp,
        p_temp,
        opt_vol.as_deref_mut(),
        opt_climate,
        opt_bio,
        r,
        star_lum,
        star_temp,
        star_r,
        shockwave_r,
        magnetic_field_gauss,
        dt_yr,
        elapsed_years,
    );

    if shockwave_r > r && r < 2.7 && comp.ice_frac > 0.001 {
        let sublimated = (comp.ice_frac * 0.15 * dt_yr).min(comp.ice_frac);
        comp.ice_frac -= sublimated;
        comp.silicate_frac += sublimated * 0.7;
        comp.metal_frac += sublimated * 0.3;

        let sum = comp.silicate_frac
            + comp.metal_frac
            + comp.ice_frac
            + comp.organics_frac
            + comp.gas_frac;
        if sum > 0.0 {
            comp.silicate_frac /= sum;
            comp.metal_frac /= sum;
            comp.ice_frac /= sum;
            comp.organics_frac /= sum;
            comp.gas_frac /= sum;
        }
    }
}
