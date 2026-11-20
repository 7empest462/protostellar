//! Deserialization, entity spawning, and persistent-ID link resolution helpers.

use bevy::math::DVec3;
use bevy::prelude::*;
use rustc_hash::FxHashMap;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::thermodynamics::{
    StellarIrradiance, equilibrate_body_temperature_for_orbit, stellar_irradiance_from_mass_or_opt,
};

use super::types::{CelestialBodySave, SystemSaveData};

pub(super) fn infer_stellar_irradiance_from_save(save_data: &SystemSaveData) -> StellarIrradiance {
    if let Some(star) = save_data
        .bodies
        .iter()
        .find(|b| b.is_central_star)
        .or_else(|| {
            save_data
                .bodies
                .iter()
                .find(|b| b.body_type.is_star_or_remnant())
        })
    {
        return StellarIrradiance {
            lum_solar: star.luminosity.max(0.0),
            temp_k: star.temperature.max(10.0),
            radius_au: star.radius.max(1e-6),
        };
    }
    stellar_irradiance_from_mass_or_opt(
        None,
        None,
        None,
        save_data.disk_parameters.central_star_mass,
    )
}

fn sanitize_legacy_body_thermal_state(
    save: &CelestialBodySave,
    star: StellarIrradiance,
    save_version: u32,
    is_multi_star: bool,
) -> (
    Temperature,
    Option<VolatileInventory>,
    Option<PlanetaryClimate>,
) {
    let mut temp = Temperature(save.temperature);
    let mut vol = save.volatile_inventory;
    let mut climate = save.climate;

    // Only run legacy trapped-steam migration on pre-v2 saves; modern saves preserve exact
    // Hadean, terraformed, post-impact molten, water-world, and multi-star thermal states.
    if save_version >= super::types::CURRENT_SAVE_SCHEMA_VERSION
        || is_multi_star
        || save.terraforming.is_some()
        || save.geological_state.is_some_and(|g| {
            matches!(
                g.epoch,
                crate::simulation::geology::GeologicalEpoch::Hadean
                    | crate::simulation::geology::GeologicalEpoch::VenusRunaway
                    | crate::simulation::geology::GeologicalEpoch::VenusModern
            )
        })
    {
        return (temp, vol, climate);
    }

    let r_au = save.position.length();
    let is_rocky_world = !save.body_type.is_star_or_remnant()
        && save.composition.gas_frac <= 0.15
        && !matches!(save.body_type, BodyType::GasGiant | BodyType::IceGiant);
    let has_trapped_steam =
        vol.is_some_and(|v| v.delivered_water_m_earth > 1e-6 && v.atmospheric_pressure_bar > 10.0);

    if is_rocky_world && ((r_au >= 0.80 && temp.0 > 500.0) || (r_au >= 0.55 && has_trapped_steam)) {
        if let Some(ref mut v) = vol
            && has_trapped_steam
        {
            v.delivered_water_m_earth = v.delivered_water_m_earth.min(0.0015);
            let cov = (v.delivered_water_m_earth / 0.0005).clamp(0.0, 0.85) as f32;
            v.ocean_coverage_frac = cov;
            v.atmospheric_pressure_bar = (0.08 + cov * 1.15).clamp(0.10, 1.50);
        }
        equilibrate_body_temperature_for_orbit(
            r_au,
            save.mass,
            save.radius,
            save.body_type,
            &save.composition,
            star,
            &mut temp,
            vol.as_mut(),
            climate.as_mut(),
        );
    }

    (temp, vol, climate)
}

/// Spawns an individual deserialized celestial body entity.
pub(super) fn spawn_saved_body(
    commands: &mut Commands,
    save: &CelestialBodySave,
    star: StellarIrradiance,
    save_version: u32,
    is_multi_star: bool,
) -> Entity {
    let (temperature, volatile_inventory, climate) =
        sanitize_legacy_body_thermal_state(save, star, save_version, is_multi_star);

    let mut cmd = commands.spawn((
        CelestialBody {
            body_type: save.body_type,
            name: save.name.clone(),
        },
        Mass(save.mass),
        SimPosition(save.position),
        SimVelocity(save.velocity),
        SimAcceleration::default(),
        Radius(save.radius),
        temperature,
        Luminosity(save.luminosity),
        AngularMomentum(save.position.cross(save.velocity) * save.mass),
        save.composition,
        save.spin,
    ));

    if let Some(diff) = save.differentiation {
        cmd.insert(diff);
    }
    if let Some(vol) = volatile_inventory {
        cmd.insert(vol);
    }
    if let Some(ring) = save.ring_system {
        cmd.insert(ring);
    }
    if let Some(ref basins) = save.basins {
        cmd.insert(PlanetaryBasins {
            basins: basins.clone(),
        });
    }
    if let Some(clim) = climate {
        cmd.insert(clim);
    }
    if let Some(bio) = save.biosphere {
        cmd.insert(bio);
    }
    if let Some(em) = save.electromagnetic {
        cmd.insert(em);
    }
    if let Some(ign) = save.ignition_state {
        cmd.insert(ign);
    } else if save.is_central_star || save.body_type.is_star_or_remnant() {
        // Ensure reloaded stars/remnants always satisfy `With<IgnitionState>` in `update_thermodynamics`
        let is_ignited = !matches!(save.body_type, BodyType::Protostar | BodyType::BrownDwarf);
        cmd.insert(IgnitionState {
            core_temperature: if is_ignited { 1.5e7 } else { 5.0e6 },
            fusion_fraction: if is_ignited { 1.0 } else { 0.0 },
            is_ignited,
            shockwave_radius: 0.0,
        });
    }
    if let Some(evol) = save.stellar_evolution {
        cmd.insert(evol);
    }
    if let Some(ref bhs) = save.black_hole_state {
        cmd.insert(bhs.clone());
    }
    if let Some(tide) = save.tidal_state {
        cmd.insert(tide);
    }
    if let Some(rel) = save.relativistic_state {
        cmd.insert(rel);
    }
    if let Some(esc) = save.atmospheric_escape {
        cmd.insert(esc);
    }
    if let Some(ref kozai) = save.kozai_lidov {
        cmd.insert(kozai.clone());
    }
    if save.is_central_star {
        cmd.insert(CentralStar);
    }
    insert_extra_body_components(&mut cmd, save);

    cmd.id()
}

fn insert_extra_body_components(cmd: &mut EntityCommands, save: &CelestialBodySave) {
    if let Some(geo) = save.geological_state {
        cmd.insert(geo);
    }
    if let Some(terra) = save.terraforming {
        cmd.insert(terra);
    }
    if let Some(volc) = save.volcanic_state {
        cmd.insert(volc);
    }
    if let Some(storm) = save.atmospheric_storm {
        cmd.insert(storm);
    }
    if let Some(kerr) = save.kerr_black_hole {
        cmd.insert(kerr);
    }
    if let Some(ref glitch) = save.pulsar_glitch {
        cmd.insert(glitch.clone());
    }
    if let Some(ref stress) = save.magnetar_stress {
        cmd.insert(stress.clone());
    }
    if let Some(ref comet) = save.oort_comet {
        cmd.insert(comet.clone());
    }
    if let Some(flare) = save.stellar_flare {
        cmd.insert(flare);
    }
    if let Some(aurora) = save.auroral_oval {
        cmd.insert(aurora);
    }
    if let Some(cv) = save.cataclysmic_variable {
        cmd.insert(cv);
    }
    if save.is_gmc_promoted_system {
        cmd.insert(crate::simulation::scenarios::molecular_cloud::GmcPromotedSystem);
    }
    if let Some(disk) = save.circumstellar_disk {
        cmd.insert(disk);
    }
    if let Some(jet) = save.relativistic_jet {
        cmd.insert(jet);
    }
    if let Some(swarm) = save.dyson_swarm {
        cmd.insert(swarm);
    }
    if let Some(tail) = save.atmospheric_escape_tail {
        cmd.insert(tail);
    }
    if let Some(hab) = save.habitability_timer {
        cmd.insert(hab);
    }
    if let Some(exc) = save.auroral_storm_excitation {
        cmd.insert(exc);
    }
    if let Some(clock) = save.stellar_activity_clock {
        cmd.insert(clock);
    }
    if let Some(rogue) = save.rogue_interloper {
        cmd.insert(rogue);
    }
    if save.is_hazard_comet {
        cmd.insert(crate::simulation::events::HazardComet);
    }
}

pub(super) fn despawn_active_bodies(
    commands: &mut Commands,
    bodies_query: &Query<Entity, With<CelestialBody>>,
) {
    for ent in bodies_query.iter() {
        if let Ok(mut cmd) = commands.get_entity(ent) {
            cmd.try_despawn();
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Restores all top-level simulation resources cleanly on load"
)]
pub(super) fn restore_simulation_resources(
    save_data: &SystemSaveData,
    sim_time: &mut ResMut<SimTime>,
    time_warp: &mut ResMut<TimeWarp>,
    disk_params: &mut ResMut<DiskParameters>,
    config: &mut ResMut<SimulationConfig>,
    energy_monitor: &mut ResMut<EnergyMonitor>,
    theia_state: &mut Option<ResMut<crate::simulation::accretion::TheiaImpactState>>,
    lhb_state: &mut Option<ResMut<crate::game::phases::LateHeavyBombardmentState>>,
) {
    sim_time.elapsed_years = save_data.timestamp_epoch_yr;
    sim_time.step_count = save_data.step_count;
    time_warp.multiplier = save_data.time_warp.multiplier;
    time_warp.is_paused = save_data.time_warp.is_paused;
    **disk_params = save_data.disk_parameters.clone();
    config.gas_density_scale = save_data.config_save.gas_density_scale;
    config.size_exaggeration = save_data.config_save.size_exaggeration;

    energy_monitor.initial_total_energy = 0.0;
    energy_monitor.kinetic_energy = 0.0;
    energy_monitor.potential_energy = 0.0;
    energy_monitor.total_energy = 0.0;
    energy_monitor.relative_energy_drift = 0.0;
    energy_monitor.initialized = false;

    if let Some(theia) = theia_state {
        theia.intercept_active = false;
        theia.moon_formed = save_data.event_flags.theia_moon_formed;
        theia.intercept_start_year = None;
        theia.manual_trigger_requested = false;
        theia.intercept_steps = 0;
        theia.target_primary = None;
    }

    if let Some(lhb) = lhb_state {
        lhb.is_active = save_data.event_flags.lhb_active;
        lhb.resonance_crossed = save_data.event_flags.lhb_resonance_crossed;
        lhb.migration_progress = save_data.event_flags.lhb_migration_progress;
        lhb.resonance_ratio = if save_data.event_flags.lhb_resonance_ratio > 0.0 {
            save_data.event_flags.lhb_resonance_ratio
        } else {
            1.85
        };
        lhb.water_delivered_earth_masses = save_data.event_flags.lhb_water_delivered_earth_masses;
        lhb.comets_scattered = save_data.event_flags.lhb_comets_scattered;
        lhb.time_active_years = save_data.event_flags.lhb_time_active_years;
        lhb.manual_trigger_requested = save_data.event_flags.lhb_manual_trigger_requested;
        lhb.scattered_bodies.clear();
    }
}

fn resolve_saved_entity(
    id: Option<u64>,
    name: &str,
    id_to_entity: &FxHashMap<u64, Entity>,
    name_to_entity: &FxHashMap<String, Entity>,
) -> Option<Entity> {
    id.and_then(|raw| id_to_entity.get(&raw).copied())
        .or_else(|| {
            if name.is_empty() {
                None
            } else {
                name_to_entity.get(name).copied()
            }
        })
}

fn insert_linked_orbital_targets(
    commands: &mut Commands,
    ent: Entity,
    body_save: &CelestialBodySave,
    id_to_entity: &FxHashMap<u64, Entity>,
    name_to_entity: &FxHashMap<String, Entity>,
) {
    if let Some(ref sat_save) = body_save.satellite {
        if let Some(parent_ent) = resolve_saved_entity(
            sat_save.parent_id,
            &sat_save.parent_name,
            id_to_entity,
            name_to_entity,
        ) {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.insert(SatelliteOf {
                    parent: parent_ent,
                    semi_major_axis_au: sat_save.semi_major_axis_au,
                    orbital_period_years: sat_save.orbital_period_years,
                    true_anomaly: sat_save.true_anomaly,
                });
            }
        } else {
            warn!(
                "⚠️ Could not resolve parent '{}' for satellite",
                sat_save.parent_name
            );
        }
    }

    if let Some(ref roche) = body_save.roche_lobe_overflow
        && let Some(companion) = resolve_saved_entity(
            roche.companion_id,
            &roche.companion_name,
            id_to_entity,
            name_to_entity,
        )
        && let Ok(mut cmd) = commands.get_entity(ent)
    {
        cmd.insert(crate::simulation::accretion::RocheLobeOverflow {
            companion,
            mass_transfer_rate: roche.mass_transfer_rate,
            l1_distance_au: roche.l1_distance_au,
        });
    }

    if let Some(ref lhb) = body_save.lhb_impactor
        && let Some(target_entity) = resolve_saved_entity(
            lhb.target_id,
            &lhb.target_name,
            id_to_entity,
            name_to_entity,
        )
        && let Ok(mut cmd) = commands.get_entity(ent)
    {
        cmd.insert(LhbImpactor {
            target_entity,
            spawn_time_yr: lhb.spawn_time_yr,
            expected_arrival_yr: lhb.expected_arrival_yr,
            target_r: lhb.target_r,
            target_q: lhb.target_q,
        });
    }
}

fn insert_linked_hazards_and_fields(
    commands: &mut Commands,
    ent: Entity,
    body_save: &CelestialBodySave,
    id_to_entity: &FxHashMap<u64, Entity>,
    name_to_entity: &FxHashMap<String, Entity>,
    central_star_ent: Option<Entity>,
) {
    if let Some(ref bomb) = body_save.bombardment_projectile
        && let Some(target_entity) = resolve_saved_entity(
            bomb.target_id,
            &bomb.target_name,
            id_to_entity,
            name_to_entity,
        )
        && let Ok(mut cmd) = commands.get_entity(ent)
    {
        let [r, g, b, a] = bomb.trail_color_rgba;
        cmd.insert(crate::simulation::terraforming::BombardmentProjectile {
            target_entity,
            bombardment_type: bomb.bombardment_type,
            launch_time_yr: bomb.launch_time_yr,
            expected_arrival_yr: bomb.expected_arrival_yr,
            trail_color: Color::srgba(r, g, b, a),
            is_detonated: bomb.is_detonated,
            relative_offset: bomb.relative_offset,
            initial_distance: bomb.initial_distance,
            flight_duration_secs: bomb.flight_duration_secs,
            flight_elapsed_secs: bomb.flight_elapsed_secs,
        });
    }

    if let Some(ref wave) = body_save.alfven_blast_wave
        && let Some(source_entity) = resolve_saved_entity(
            wave.source_id,
            &wave.source_name,
            id_to_entity,
            name_to_entity,
        )
        && let Ok(mut cmd) = commands.get_entity(ent)
    {
        cmd.insert(crate::simulation::neutron_stars::AlfvenBlastWave {
            source_entity,
            origin_au: wave.origin_au,
            radius_au: wave.radius_au,
            expansion_speed_au_yr: wave.expansion_speed_au_yr,
            energy_erg: wave.energy_erg,
            luminosity_solar: wave.luminosity_solar,
            synchrotron_brightness: wave.synchrotron_brightness,
            age_seconds: wave.age_seconds,
            max_radius_au: wave.max_radius_au,
        });
    }

    if let Some(mut tidal) = body_save.tidal_state {
        tidal.host_entity = body_save
            .tidal_host_id
            .and_then(|id| id_to_entity.get(&id).copied())
            .or_else(|| {
                body_save.satellite.as_ref().and_then(|sat| {
                    resolve_saved_entity(
                        sat.parent_id,
                        &sat.parent_name,
                        id_to_entity,
                        name_to_entity,
                    )
                })
            })
            .or_else(|| {
                if body_save.satellite.is_none() {
                    central_star_ent
                } else {
                    None
                }
            });
        if let Ok(mut cmd) = commands.get_entity(ent) {
            cmd.insert(tidal);
        }
    }

    if let Some(mut kozai) = body_save.kozai_lidov.clone() {
        kozai.perturber_entity = resolve_saved_entity(
            body_save.kozai_perturber_id,
            &kozai.perturber_name,
            id_to_entity,
            name_to_entity,
        );
        if let Ok(mut cmd) = commands.get_entity(ent) {
            cmd.insert(kozai);
        }
    }
}

pub(super) struct LinkedSpawnResult {
    pub central_star_ent: Option<Entity>,
    pub first_body_ent: Option<Entity>,
    pub id_to_entity: FxHashMap<u64, Entity>,
    pub name_to_entity: FxHashMap<String, Entity>,
}

fn spawn_reconstructed_central_star(
    commands: &mut Commands,
    save_data: &SystemSaveData,
    star_irrad: StellarIrradiance,
) -> Entity {
    let star_m = save_data.disk_parameters.central_star_mass.max(0.08);
    let comp = Composition::solar_gas();
    let is_ignited = save_data.timestamp_epoch_yr >= 30.0 || star_m < 0.08;
    let (body_type, name) = if is_ignited {
        let (bt, label) = classify_stellar_type_and_label(star_m, &comp);
        (bt, format!("The Star ({label})"))
    } else {
        (
            BodyType::Protostar,
            "The Protostar (Solar Nebula)".to_string(),
        )
    };
    let core_temp = if is_ignited {
        1.5e7
    } else {
        (4.0e6 + 2.0e5 * star_m * save_data.timestamp_epoch_yr.max(0.0)).min(9.9e6)
    };
    let ignition = IgnitionState {
        core_temperature: core_temp,
        fusion_fraction: if is_ignited {
            1.0
        } else {
            (core_temp / 1.0e7).clamp(0.0, 0.99) as f32
        },
        is_ignited,
        shockwave_radius: if is_ignited { 30.0 } else { 0.0 },
    };
    let evo = StellarEvolutionState {
        phase: if !is_ignited {
            StellarEvolutionPhase::ProtostarContraction
        } else if body_type == BodyType::RedGiant {
            StellarEvolutionPhase::RedGiantBranch
        } else {
            StellarEvolutionPhase::MainSequence
        },
        ..Default::default()
    };

    commands
        .spawn((
            CelestialBody { body_type, name },
            CentralStar,
            Mass(star_m),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(star_irrad.radius_au),
            Temperature(star_irrad.temp_k),
            Luminosity(star_irrad.lum_solar),
            AngularMomentum::default(),
            comp,
            SpinState {
                spin_vector: bevy::math::DVec3::new(0.0, 1e-12, 0.0),
                rotation_period_hours: 600.0,
                axial_tilt_degrees: 7.25,
            },
            ignition,
            evo,
        ))
        .id()
}

pub(super) fn spawn_and_link_saved_bodies(
    commands: &mut Commands,
    save_data: &SystemSaveData,
) -> LinkedSpawnResult {
    let mut id_to_entity = FxHashMap::default();
    let mut name_to_entity = FxHashMap::default();
    let bodies = &save_data.bodies;
    let mut spawned = Vec::with_capacity(bodies.len());
    let mut central_star_ent = None;
    let mut first_body_ent = None;
    let star_irrad = infer_stellar_irradiance_from_save(save_data);

    let star_count = bodies
        .iter()
        .filter(|b| b.is_central_star || b.body_type.is_star_or_remnant())
        .count();
    let has_any_star = star_count > 0;
    let is_multi_star = star_count > 1;
    if !has_any_star && !bodies.is_empty() && save_data.disk_parameters.central_star_mass > 0.0 {
        central_star_ent = Some(spawn_reconstructed_central_star(
            commands, save_data, star_irrad,
        ));
    }

    for body_save in bodies {
        let ent = spawn_saved_body(
            commands,
            body_save,
            star_irrad,
            save_data.version,
            is_multi_star,
        );
        if first_body_ent.is_none() {
            first_body_ent = Some(ent);
        }
        if body_save.is_central_star {
            central_star_ent = Some(ent);
        }
        if let Some(save_id) = body_save.save_id {
            id_to_entity.insert(save_id, ent);
        }
        name_to_entity.entry(body_save.name.clone()).or_insert(ent);
        spawned.push((ent, body_save));
    }

    for (ent, body_save) in spawned {
        insert_linked_orbital_targets(commands, ent, body_save, &id_to_entity, &name_to_entity);
        insert_linked_hazards_and_fields(
            commands,
            ent,
            body_save,
            &id_to_entity,
            &name_to_entity,
            central_star_ent,
        );
    }

    LinkedSpawnResult {
        central_star_ent,
        first_body_ent,
        id_to_entity,
        name_to_entity,
    }
}

pub(super) fn spawn_saved_probes_and_waves(
    commands: &mut Commands,
    save_data: &SystemSaveData,
    links: &LinkedSpawnResult,
    probe_counter: &mut Option<ResMut<crate::simulation::probes::ProbeCounter>>,
) {
    let mut max_probe_id = 0usize;
    for probe_save in &save_data.probes {
        max_probe_id = max_probe_id.max(probe_save.probe.id);
        let mut cmd = commands.spawn((
            probe_save.probe.clone(),
            CelestialBody {
                body_type: BodyType::Asteroid,
                name: probe_save.name.clone(),
            },
            Mass(probe_save.mass),
            SimPosition(probe_save.position),
            SimVelocity(probe_save.velocity),
            SimAcceleration::default(),
            Radius(probe_save.radius),
            Temperature(probe_save.temperature),
            Luminosity(0.0),
            probe_save.composition,
            SpinState::default(),
            (
                Transform::from_translation(probe_save.position.as_vec3())
                    .with_scale(Vec3::splat(0.00035)),
                GlobalTransform::default(),
                Visibility::default(),
                InheritedVisibility::default(),
                ViewVisibility::default(),
                crate::rendering::bodies::VisualBody,
            ),
        ));
        if let Some(ref fc) = probe_save.flight_computer
            && let Some(target_entity) = resolve_saved_entity(
                fc.target_id,
                &fc.target_name,
                &links.id_to_entity,
                &links.name_to_entity,
            )
        {
            cmd.insert(crate::simulation::probes::FlightComputer {
                target_entity,
                target_name: fc.target_name.clone(),
                launch_pos: fc.launch_pos,
                distance_to_target: fc.distance_to_target,
                relative_speed: fc.relative_speed,
            });
        }
    }
    if let Some(counter) = probe_counter {
        counter.0 = max_probe_id;
    }

    for wave in &save_data.alfven_waves {
        if let Some(source_entity) = resolve_saved_entity(
            wave.source_id,
            &wave.source_name,
            &links.id_to_entity,
            &links.name_to_entity,
        ) {
            commands.spawn((
                crate::simulation::neutron_stars::AlfvenBlastWave {
                    source_entity,
                    origin_au: wave.origin_au,
                    radius_au: wave.radius_au,
                    expansion_speed_au_yr: wave.expansion_speed_au_yr,
                    energy_erg: wave.energy_erg,
                    luminosity_solar: wave.luminosity_solar,
                    synchrotron_brightness: wave.synchrotron_brightness,
                    age_seconds: wave.age_seconds,
                    max_radius_au: wave.max_radius_au,
                },
                SimPosition(wave.origin_au),
            ));
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Scenario and camera state restoration requires several subsystem references"
)]
pub(super) fn apply_loaded_scenario_and_camera(
    save_data: &SystemSaveData,
    detected_preset: crate::simulation::scenarios::ScenarioPreset,
    scenario_state: &mut Option<ResMut<crate::simulation::scenarios::ActiveScenarioState>>,
    phase_mgr: &mut Option<ResMut<crate::game::phases::PhaseManager>>,
    next_phase: &mut Option<ResMut<NextState<crate::game::phases::SystemPhase>>>,
    disk_params: &DiskParameters,
    config: &mut ResMut<SimulationConfig>,
    swarm_mesh_query: &mut Query<
        &mut Visibility,
        With<crate::rendering::particle_swarm::ParticleSwarmMesh>,
    >,
    primary_ent: Option<Entity>,
    camera_query: &mut Query<&mut crate::rendering::camera::PanOrbitCamera>,
) {
    if let Some(s_state) = scenario_state {
        s_state.current_preset = detected_preset;
        s_state.scenario_time_years = save_data.timestamp_epoch_yr;
        s_state.migration_active = false;
    }

    crate::simulation::scenarios::update_scenario_system_phase(
        detected_preset,
        phase_mgr,
        next_phase,
    );

    let is_empty_swarm =
        crate::simulation::scenarios::is_empty_swarm_preset(detected_preset, disk_params.disk_mass);

    config.active_particles = if is_empty_swarm {
        0
    } else {
        config.target_particle_count as u32
    };

    for mut vis in swarm_mesh_query.iter_mut() {
        *vis = if is_empty_swarm {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }

    if let Some(mut cam) = camera_query.iter_mut().next() {
        cam.surface_horizon_mode = false;
        if let Some(target) = primary_ent {
            cam.target_entity = Some(target);
        }
        if detected_preset == crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster {
            let (target_r, target_yaw, target_pitch) =
                crate::simulation::scenarios::scenario_preset_camera_pose(detected_preset);
            cam.radius = target_r;
            cam.target_radius = target_r;
            cam.yaw = target_yaw;
            cam.target_yaw = target_yaw;
            cam.pitch = target_pitch;
            cam.target_pitch = target_pitch;
        }
    }
}
