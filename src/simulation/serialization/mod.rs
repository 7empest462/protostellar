//! System State Save / Load & Scenario Serializer (JSON & Postcard Binary).
//!
//! Provides full serialization and deserialization of the active solar system state,
//! capturing orbital positions, velocities, masses, thermodynamic parameters, chemical
//! compositions, planetary rings, impact basins, and global disk configurations.

pub mod io;
mod restore;
pub mod types;

pub use io::*;
use restore::*;
pub use types::*;

use bevy::math::DVec3;
use bevy::prelude::*;
use rustc_hash::FxHashMap;

use crate::simulation::atmosphere_escape::AtmosphericEscapeState;
use crate::simulation::components::*;
use crate::simulation::kozai_lidov::KozaiLidovState;
use crate::simulation::relativity::RelativisticState;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;

/// Infers the intended scenario preset from the contents of a saved celestial body list
/// when the save file was created without an explicit `scenario_preset` tag.
pub fn infer_scenario_preset(
    bodies: &[CelestialBodySave],
) -> crate::simulation::scenarios::ScenarioPreset {
    use crate::simulation::scenarios::ScenarioPreset;

    let star_count = bodies
        .iter()
        .filter(|b| b.body_type.is_star_or_remnant())
        .count();

    if bodies.iter().any(|b| b.name.contains("Genesis")) {
        ScenarioPreset::AccretionDiskGenesis
    } else if bodies.iter().any(|b| b.name.contains("TRAPPIST-1")) {
        ScenarioPreset::Trappist1System
    } else if bodies.iter().any(|b| b.name.contains("Kepler-16")) {
        ScenarioPreset::Kepler16Circumbinary
    } else if bodies.iter().any(|b| b.name.contains("Little Red Dot")) {
        ScenarioPreset::LittleRedDot
    } else if bodies
        .iter()
        .any(|b| b.name.contains("PSR B1257+12") || b.name.contains("Lich"))
    {
        ScenarioPreset::PulsarSystem
    } else if bodies.iter().any(|b| b.name.contains("SGR 1806-20")) {
        ScenarioPreset::MagnetarOutburst
    } else if bodies.iter().any(|b| b.name.contains("PSR B1913+16")) {
        ScenarioPreset::RelativisticBinary
    } else if bodies.iter().any(|b| b.name.contains("HD 80606")) {
        ScenarioPreset::KozaiLidovTriple
    } else if bodies
        .iter()
        .any(|b| b.name.contains("Sagittarius A*") || b.name.contains("S2"))
    {
        ScenarioPreset::SagittariusAStar
    } else if bodies
        .iter()
        .any(|b| b.name.contains("Andromeda") || b.name.contains("Milky Way"))
    {
        ScenarioPreset::GalacticCollision
    } else if bodies
        .iter()
        .any(|b| b.name.contains("TDE Supermassive") || b.name.contains("Doomed Red Giant"))
    {
        ScenarioPreset::TidalDisruptionEvent
    } else if star_count > 3
        || bodies
            .iter()
            .any(|b| b.name.contains("SMBH") || b.name.contains("NovaCore"))
    {
        ScenarioPreset::MolecularCloudCluster
    } else if bodies.iter().any(|b| b.name.contains("Rogue")) {
        ScenarioPreset::RoguePlanetFlyby
    } else if bodies.iter().any(|b| b.name.contains("Hot Jupiter")) {
        ScenarioPreset::HotJupiterMigration
    } else {
        ScenarioPreset::SolarNebulaMmsn
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Optional astrophysical subsystem components"
)]
type ExtraBodyComponents<'w> = (
    Option<&'w crate::simulation::geology::GeologicalState>,
    Option<&'w crate::simulation::terraforming::TerraformingAtmosphere>,
    Option<&'w VolcanicState>,
    Option<&'w AtmosphericStormState>,
    Option<&'w crate::simulation::relativity::kerr::KerrBlackHole>,
    Option<&'w crate::simulation::neutron_stars::PulsarGlitchState>,
    Option<&'w crate::simulation::neutron_stars::MagnetarCrustStress>,
    Option<&'w crate::simulation::oort::OortComet>,
    Option<&'w crate::simulation::space_weather::StellarFlareState>,
    Option<&'w crate::simulation::space_weather::AuroralOvalState>,
    Option<&'w crate::simulation::accretion::CataclysmicVariable>,
);

#[allow(clippy::type_complexity, reason = "Query for full body serialization")]
type BodySerializeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CelestialBody,
        &'static Mass,
        &'static SimPosition,
        &'static SimVelocity,
        &'static Radius,
        Option<&'static Temperature>,
        Option<&'static Luminosity>,
        Option<&'static Composition>,
        Option<&'static SpinState>,
        (
            Option<&'static InternalDifferentiation>,
            Option<&'static VolatileInventory>,
            Option<&'static PlanetaryRingSystem>,
            Option<&'static PlanetaryBasins>,
            Option<&'static PlanetaryClimate>,
            Option<&'static BiosphereState>,
        ),
        (
            Option<&'static ElectromagneticFieldState>,
            Option<&'static IgnitionState>,
            Option<&'static StellarEvolutionState>,
            Option<&'static BlackHoleStarState>,
            Option<&'static SatelliteOf>,
            Option<&'static TidalState>,
            Option<&'static RelativisticState>,
            Option<&'static AtmosphericEscapeState>,
            Option<&'static KozaiLidovState>,
            Has<CentralStar>,
        ),
        ExtraBodyComponents<'static>,
    ),
    Without<crate::simulation::probes::SpaceProbe>,
>;

fn extract_extra_body_components(save: &mut CelestialBodySave, extra: ExtraBodyComponents<'_>) {
    let (
        opt_geo,
        opt_terra,
        opt_volc,
        opt_storm,
        opt_kerr,
        opt_glitch,
        opt_stress,
        opt_oort,
        opt_flare,
        opt_aurora,
        opt_cv,
    ) = extra;
    save.geological_state = opt_geo.copied();
    save.terraforming = opt_terra.copied();
    save.volcanic_state = opt_volc.copied();
    save.atmospheric_storm = opt_storm.copied();
    save.kerr_black_hole = opt_kerr.copied();
    save.pulsar_glitch = opt_glitch.cloned();
    save.magnetar_stress = opt_stress.cloned();
    save.oort_comet = opt_oort.cloned();
    save.stellar_flare = opt_flare.copied();
    save.auroral_oval = opt_aurora.copied();
    save.cataclysmic_variable = opt_cv.copied();
}

#[allow(
    clippy::type_complexity,
    reason = "Phase 5 extended component query stays under Bevy tuple arity limit via nesting"
)]
type Phase5BodyComponentsQuery<'w, 's> = Query<
    'w,
    's,
    (
        (
            Option<&'static crate::simulation::scenarios::molecular_cloud::GmcPromotedSystem>,
            Option<&'static CircumstellarDisk>,
            Option<&'static RelativisticJetState>,
            Option<&'static crate::rendering::effects::megastructures::DysonSwarm>,
            Option<&'static crate::simulation::accretion::RocheLobeOverflow>,
            Option<&'static LhbImpactor>,
            Option<&'static crate::simulation::terraforming::BombardmentProjectile>,
        ),
        (
            Option<&'static crate::simulation::neutron_stars::AlfvenBlastWave>,
            Option<&'static AtmosphericEscapeTail>,
            Option<&'static crate::simulation::biology::civilization::HabitabilityTimer>,
            Option<&'static crate::simulation::space_weather::AuroralStormExcitation>,
            Option<&'static crate::simulation::space_weather::StellarActivityClock>,
            Option<&'static crate::simulation::events::RogueInterloper>,
            Option<&'static crate::simulation::events::HazardComet>,
        ),
        (
            Option<&'static crate::simulation::biology::CivilizationState>,
            Option<&'static crate::simulation::events::CrisisImpactor>,
        ),
    ),
>;

#[allow(
    clippy::type_complexity,
    reason = "Probe serialization query bundling guidance and kinematics"
)]
type SpaceProbeSaveQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static crate::simulation::probes::SpaceProbe,
        Option<&'static crate::simulation::probes::FlightComputer>,
        Option<&'static CelestialBody>,
        Option<&'static SimPosition>,
        Option<&'static SimVelocity>,
        Option<&'static Mass>,
        Option<&'static Radius>,
        Option<&'static Temperature>,
        Option<&'static Composition>,
    ),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub struct Phase5SerializeQueries<'w, 's> {
    pub bodies_extra: Phase5BodyComponentsQuery<'w, 's>,
    pub probes: SpaceProbeSaveQuery<'w, 's>,
    pub alfven_waves: Query<
        'w,
        's,
        &'static crate::simulation::neutron_stars::AlfvenBlastWave,
        Without<CelestialBody>,
    >,
    #[allow(
        clippy::type_complexity,
        reason = "Optional progression, economy, and cosmic event resources saved if present"
    )]
    pub progression: (
        Option<Res<'w, crate::simulation::economy::ProgressionState>>,
        Option<Res<'w, crate::simulation::economy::ScienceData>>,
        Option<Res<'w, crate::simulation::economy::UnlockedTechs>>,
        Option<Res<'w, crate::simulation::events::CosmicEventManager>>,
    ),
}

fn attach_phase5_body_components(
    save: &mut CelestialBodySave,
    ent: Entity,
    entity_to_name: &FxHashMap<Entity, String>,
    extra_q: &Phase5BodyComponentsQuery,
) {
    let Ok((group_a, group_b, group_c)) = extra_q.get(ent) else {
        return;
    };
    let (opt_gmc, opt_disk, opt_jet, opt_dyson, opt_roche, opt_lhb, opt_bomb) = group_a;
    let (opt_alfven, opt_tail, opt_hab, opt_aurora_exc, opt_clock, opt_rogue, opt_hazard) = group_b;
    let (opt_civ, opt_impactor) = group_c;

    save.is_gmc_promoted_system = opt_gmc.is_some();
    save.circumstellar_disk = opt_disk.copied();
    save.relativistic_jet = opt_jet.copied();
    save.dyson_swarm = opt_dyson.copied();
    save.roche_lobe_overflow = opt_roche.map(|r| RocheLobeOverflowSave {
        companion_id: Some(r.companion.to_bits()),
        companion_name: entity_to_name
            .get(&r.companion)
            .cloned()
            .unwrap_or_default(),
        mass_transfer_rate: r.mass_transfer_rate,
        l1_distance_au: r.l1_distance_au,
    });
    save.lhb_impactor = opt_lhb.map(|i| LhbImpactorSave {
        target_id: Some(i.target_entity.to_bits()),
        target_name: entity_to_name
            .get(&i.target_entity)
            .cloned()
            .unwrap_or_default(),
        spawn_time_yr: i.spawn_time_yr,
        expected_arrival_yr: i.expected_arrival_yr,
        target_r: i.target_r,
        target_q: i.target_q,
    });
    save.bombardment_projectile = opt_bomb.map(|b| BombardmentProjectileSave {
        target_id: Some(b.target_entity.to_bits()),
        target_name: entity_to_name
            .get(&b.target_entity)
            .cloned()
            .unwrap_or_default(),
        bombardment_type: b.bombardment_type,
        launch_time_yr: b.launch_time_yr,
        expected_arrival_yr: b.expected_arrival_yr,
        trail_color_rgba: b.trail_color.to_srgba().to_f32_array(),
        is_detonated: b.is_detonated,
        relative_offset: b.relative_offset,
        initial_distance: b.initial_distance,
        flight_duration_secs: b.flight_duration_secs,
        flight_elapsed_secs: b.flight_elapsed_secs,
    });
    save.alfven_blast_wave = opt_alfven.map(|w| AlfvenBlastWaveSave {
        source_id: Some(w.source_entity.to_bits()),
        source_name: entity_to_name
            .get(&w.source_entity)
            .cloned()
            .unwrap_or_default(),
        origin_au: w.origin_au,
        radius_au: w.radius_au,
        expansion_speed_au_yr: w.expansion_speed_au_yr,
        energy_erg: w.energy_erg,
        luminosity_solar: w.luminosity_solar,
        synchrotron_brightness: w.synchrotron_brightness,
        age_seconds: w.age_seconds,
        max_radius_au: w.max_radius_au,
    });
    save.atmospheric_escape_tail = opt_tail.copied();
    save.habitability_timer = opt_hab.copied();
    save.auroral_storm_excitation = opt_aurora_exc.copied();
    save.stellar_activity_clock = opt_clock.copied();
    save.rogue_interloper = opt_rogue.copied();
    save.is_hazard_comet = opt_hazard.is_some();
    save.civilization = opt_civ.copied();
    save.crisis_impactor = opt_impactor.map(|imp| {
        let mut saved = imp.clone();
        if saved.target_name.is_empty()
            && let Some(target_ent) = saved.target_entity
            && let Some(name) = entity_to_name.get(&target_ent)
        {
            saved.target_name.clone_from(name);
        }
        saved
    });
}

fn collect_entity_name_map(bodies_query: &BodySerializeQuery) -> FxHashMap<Entity, String> {
    let mut entity_to_name = FxHashMap::default();
    for (ent, body, ..) in bodies_query.iter() {
        entity_to_name.insert(ent, body.name.clone());
    }
    entity_to_name
}

/// Extracts celestial bodies into serializable records.
fn collect_bodies_save(
    bodies_query: &BodySerializeQuery,
    entity_to_name: &FxHashMap<Entity, String>,
    extra_q: &Phase5BodyComponentsQuery,
) -> Vec<CelestialBodySave> {
    let mut bodies = Vec::new();
    for (
        ent,
        body,
        mass,
        pos,
        vel,
        rad,
        opt_temp,
        opt_lum,
        opt_comp,
        opt_spin,
        (opt_diff, opt_vol, opt_ring, opt_basins, opt_clim, opt_bio),
        (
            opt_em,
            opt_ign,
            opt_evol,
            opt_bhs,
            opt_sat,
            opt_tide,
            opt_rel,
            opt_escape,
            opt_kozai,
            is_star,
        ),
        extra,
    ) in bodies_query.iter()
    {
        let satellite = opt_sat.map(|sat| SatelliteSave {
            parent_id: Some(sat.parent.to_bits()),
            parent_name: entity_to_name
                .get(&sat.parent)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
            semi_major_axis_au: sat.semi_major_axis_au,
            orbital_period_years: sat.orbital_period_years,
            true_anomaly: sat.true_anomaly,
        });

        let resolved_comp = opt_comp.copied().unwrap_or_else(|| {
            if body.body_type.is_star_or_remnant() {
                Composition::solar_gas()
            } else {
                Composition::rocky()
            }
        });
        let mut body_save = CelestialBodySave {
            save_id: Some(ent.to_bits()),
            name: body.name.clone(),
            body_type: body.body_type,
            position: pos.0,
            velocity: vel.0,
            mass: mass.0,
            radius: rad.0,
            temperature: opt_temp.map_or(280.0, |t| t.0),
            luminosity: opt_lum.map_or(0.0, |l| l.0),
            composition: resolved_comp,
            spin: opt_spin.copied().unwrap_or_default(),
            differentiation: opt_diff.copied(),
            volatile_inventory: opt_vol.copied(),
            ring_system: opt_ring.copied(),
            basins: opt_basins.map(|b| b.basins.clone()),
            climate: opt_clim.copied(),
            biosphere: opt_bio.copied(),
            electromagnetic: opt_em.copied(),
            is_central_star: is_star,
            ignition_state: opt_ign.copied(),
            stellar_evolution: opt_evol.copied(),
            black_hole_state: opt_bhs.cloned(),
            satellite,
            tidal_state: opt_tide.copied(),
            tidal_host_id: opt_tide.and_then(|t| t.host_entity).map(Entity::to_bits),
            relativistic_state: opt_rel.copied(),
            atmospheric_escape: opt_escape.copied(),
            kozai_lidov: opt_kozai.cloned(),
            kozai_perturber_id: opt_kozai
                .and_then(|k| k.perturber_entity)
                .map(Entity::to_bits),
            ..Default::default()
        };
        extract_extra_body_components(&mut body_save, extra);
        attach_phase5_body_components(&mut body_save, ent, entity_to_name, extra_q);
        bodies.push(body_save);
    }

    bodies
}

fn collect_probes_save(probes_query: &SpaceProbeSaveQuery) -> Vec<SpaceProbeSave> {
    probes_query
        .iter()
        .map(
            |(probe, opt_fc, opt_body, opt_pos, opt_vel, opt_m, opt_r, opt_t, opt_c)| {
                SpaceProbeSave {
                    probe: probe.clone(),
                    flight_computer: opt_fc.map(|fc| FlightComputerSave {
                        target_id: Some(fc.target_entity.to_bits()),
                        target_name: fc.target_name.clone(),
                        launch_pos: fc.launch_pos,
                        distance_to_target: fc.distance_to_target,
                        relative_speed: fc.relative_speed,
                    }),
                    name: opt_body
                        .map_or_else(|| format!("Space Probe #{}", probe.id), |b| b.name.clone()),
                    position: opt_pos.map_or(DVec3::ZERO, |p| p.0),
                    velocity: opt_vel.map_or(DVec3::ZERO, |v| v.0),
                    mass: opt_m.map_or(0.0, |m| m.0),
                    radius: opt_r.map_or(1.5e-11, |r| r.0),
                    temperature: opt_t.map_or(290.0, |t| t.0),
                    composition: opt_c.copied().unwrap_or_else(Composition::metal_rich),
                }
            },
        )
        .collect()
}

fn collect_standalone_alfven_save(
    entity_to_name: &FxHashMap<Entity, String>,
    alfven_q: &Query<&crate::simulation::neutron_stars::AlfvenBlastWave, Without<CelestialBody>>,
) -> Vec<AlfvenBlastWaveSave> {
    alfven_q
        .iter()
        .map(|w| AlfvenBlastWaveSave {
            source_id: Some(w.source_entity.to_bits()),
            source_name: entity_to_name
                .get(&w.source_entity)
                .cloned()
                .unwrap_or_default(),
            origin_au: w.origin_au,
            radius_au: w.radius_au,
            expansion_speed_au_yr: w.expansion_speed_au_yr,
            energy_erg: w.energy_erg,
            luminosity_solar: w.luminosity_solar,
            synchrotron_brightness: w.synchrotron_brightness,
            age_seconds: w.age_seconds,
            max_radius_au: w.max_radius_au,
        })
        .collect()
}

/// Assembles a `SystemSaveData` snapshot from active simulation resources and bodies.
#[allow(
    clippy::too_many_arguments,
    reason = "Constructs full serialized system snapshot"
)]
pub fn build_system_save_data(
    sim_time: &SimTime,
    time_warp: &TimeWarp,
    disk_params: &DiskParameters,
    config: &SimulationConfig,
    theia_state: Option<&crate::simulation::accretion::TheiaImpactState>,
    lhb_state: Option<&crate::game::phases::LateHeavyBombardmentState>,
    scenario_state: Option<&crate::simulation::scenarios::ActiveScenarioState>,
    bodies: Vec<CelestialBodySave>,
    probes: Vec<SpaceProbeSave>,
    alfven_waves: Vec<AlfvenBlastWaveSave>,
) -> SystemSaveData {
    SystemSaveData {
        version: CURRENT_SAVE_SCHEMA_VERSION,
        timestamp_epoch_yr: sim_time.elapsed_years,
        step_count: sim_time.step_count,
        time_warp: TimeWarpSave {
            multiplier: time_warp.multiplier,
            is_paused: time_warp.is_paused,
        },
        disk_parameters: disk_params.clone(),
        config_save: ConfigSave {
            gas_density_scale: config.gas_density_scale,
            size_exaggeration: config.size_exaggeration,
        },
        bodies,
        event_flags: EventFlagsSave {
            theia_moon_formed: theia_state.is_some_and(|s| s.moon_formed),
            lhb_active: lhb_state.is_some_and(|s| s.is_active),
            lhb_resonance_crossed: lhb_state.is_some_and(|s| s.resonance_crossed),
            lhb_migration_progress: lhb_state.map_or(0.0, |s| s.migration_progress),
            lhb_resonance_ratio: lhb_state.map_or(1.85, |s| s.resonance_ratio),
            lhb_water_delivered_earth_masses: lhb_state
                .map_or(0.0, |s| s.water_delivered_earth_masses),
            lhb_comets_scattered: lhb_state.map_or(0, |s| s.comets_scattered),
            lhb_time_active_years: lhb_state.map_or(0.0, |s| s.time_active_years),
            lhb_manual_trigger_requested: lhb_state.is_some_and(|s| s.manual_trigger_requested),
        },
        scenario_preset: scenario_state.map(|s| s.current_preset),
        probes,
        alfven_waves,
        progression: None,
    }
}

fn build_progression_save(queries: &Phase5SerializeQueries) -> Option<types::ProgressionSave> {
    let (opt_prog, opt_science, opt_techs, opt_mgr) = &queries.progression;
    if opt_prog.is_none() && opt_science.is_none() && opt_techs.is_none() && opt_mgr.is_none() {
        return None;
    }
    let prog = opt_prog.as_deref().cloned().unwrap_or_default();
    let science_data = opt_science.as_deref().cloned().unwrap_or_default();
    let unlocked_techs = opt_techs
        .as_deref()
        .map_or_else(Vec::new, |t| t.unlocked.iter().copied().collect());
    let (auto_hazards_enabled, total_hazards_survived) =
        opt_mgr.as_deref().map_or((true, 0), |m| {
            (m.auto_hazards_enabled, m.total_hazards_survived)
        });
    Some(types::ProgressionSave {
        mode: prog.mode,
        science_data,
        unlocked_techs,
        crises_averted: prog.crises_averted,
        crises_mitigated: prog.crises_mitigated,
        crises_catastrophic: prog.crises_catastrophic,
        stellar_engine_active: prog.stellar_engine_active,
        auto_hazards_enabled,
        total_hazards_survived,
    })
}

/// System that listens for `SaveSystemEvent` and writes the solar system snapshot to disk.
#[allow(
    clippy::too_many_arguments,
    reason = "Save event handler collects simulation resources, bodies, probes, and Alfvén waves"
)]
pub fn handle_save_system_events(
    mut events: MessageReader<SaveSystemEvent>,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    disk_params: Res<DiskParameters>,
    config: Res<SimulationConfig>,
    theia_state: Option<Res<crate::simulation::accretion::TheiaImpactState>>,
    lhb_state: Option<Res<crate::game::phases::LateHeavyBombardmentState>>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    mut toast: Option<ResMut<crate::game::ui::NotificationToast>>,
    bodies_query: BodySerializeQuery,
    phase5_queries: Phase5SerializeQueries,
) {
    for event in events.read() {
        let path = normalize_save_path(&event.filename);
        let entity_to_name = collect_entity_name_map(&bodies_query);
        let bodies =
            collect_bodies_save(&bodies_query, &entity_to_name, &phase5_queries.bodies_extra);
        let probes = collect_probes_save(&phase5_queries.probes);
        let alfven_waves =
            collect_standalone_alfven_save(&entity_to_name, &phase5_queries.alfven_waves);
        let mut save_data = build_system_save_data(
            &sim_time,
            &time_warp,
            &disk_params,
            &config,
            theia_state.as_deref(),
            lhb_state.as_deref(),
            scenario_state.as_deref(),
            bodies,
            probes,
            alfven_waves,
        );
        save_data.progression = build_progression_save(&phase5_queries);

        match save_system_to_file(&save_data, &path) {
            Ok(()) => {
                info!(
                    "💾 Saved system state ({} bodies) to: {}",
                    save_data.bodies.len(),
                    path
                );
                if let Some(ref mut t) = toast {
                    t.message = format!("💾 Saved {} bodies [{}]", save_data.bodies.len(), path);
                    t.timer = 4.0;
                }
            }
            Err(e) => {
                error!("❌ Failed to save system state to {}: {}", path, e);
                if let Some(ref mut t) = toast {
                    t.message = format!("❌ Save failed: {e}");
                    t.timer = 4.0;
                }
            }
        }
    }
}

/// System that listens for `LoadSystemEvent`, clears the current bodies, and instantiates the saved system.
#[allow(
    clippy::type_complexity,
    reason = "System load requires comprehensive world and scenario state"
)]
pub fn handle_load_system_events(
    mut commands: Commands,
    mut events: MessageReader<LoadSystemEvent>,
    mut disk_params: ResMut<DiskParameters>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    mut time_warp: ResMut<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut config: ResMut<SimulationConfig>,
    mut lhb_state: Option<ResMut<crate::game::phases::LateHeavyBombardmentState>>,
    bodies_query: Query<Entity, With<CelestialBody>>,
    mut camera_query: Query<&mut crate::rendering::camera::PanOrbitCamera>,
    (mut swarm, mut swarm_mesh_query): (
        Option<ResMut<crate::rendering::particle_swarm::ParticleSwarmData>>,
        Query<&mut Visibility, With<crate::rendering::particle_swarm::ParticleSwarmMesh>>,
    ),
    (mut scenario_state, mut toast): (
        Option<ResMut<crate::simulation::scenarios::ActiveScenarioState>>,
        Option<ResMut<crate::game::ui::NotificationToast>>,
    ),
    (mut phase_mgr, mut next_phase, mut reset_res): (
        Option<ResMut<crate::game::phases::PhaseManager>>,
        Option<ResMut<NextState<crate::game::phases::SystemPhase>>>,
        crate::simulation::scenarios::ScenarioResetResources,
    ),
) {
    let Some(filename) = events.read().last().map(|e| e.filename.clone()) else {
        return;
    };
    let path = normalize_save_path(&filename);
    let save_data = match load_system_from_file(&path) {
        Ok(data) => data,
        Err(e) => {
            error!("❌ Failed to load system state from {}: {}", path, e);
            if let Some(ref mut t) = toast {
                t.message = format!("❌ Load failed: {e}");
                t.timer = 5.0;
            }
            return;
        }
    };

    // 1. Despawn existing celestial bodies, probes, and clear transient pools
    despawn_active_bodies(&mut commands, &bodies_query);
    crate::simulation::scenarios::clear_transient_pools_and_entities(&mut commands, &mut reset_res);

    // 2. Restore simulation resources and flags
    restore_simulation_resources(
        &save_data,
        &mut sim_time,
        &mut time_warp,
        &mut disk_params,
        &mut config,
        &mut energy_monitor,
        &mut reset_res.theia_state,
        &mut lhb_state,
    );
    restore_progression_and_events(&save_data, &mut reset_res);

    // 3. Spawn bodies, resolve entity links, and restore probes/waves
    let links = spawn_and_link_saved_bodies(&mut commands, &save_data);
    spawn_saved_probes_and_waves(
        &mut commands,
        &save_data,
        &links,
        &mut reset_res.probe_counter,
    );

    // 4. Update scenario preset, system phase, and camera
    let detected_preset = save_data
        .scenario_preset
        .unwrap_or_else(|| infer_scenario_preset(&save_data.bodies));
    let primary_ent = links.central_star_ent.or(links.first_body_ent);
    player_state.selected_entity = primary_ent;

    apply_loaded_scenario_and_camera(
        &save_data,
        detected_preset,
        &mut scenario_state,
        &mut phase_mgr,
        &mut next_phase,
        &disk_params,
        &mut config,
        &mut swarm_mesh_query,
        primary_ent,
        &mut camera_query,
    );

    // 5. Reseed particle swarm if present
    if let Some(ref mut swarm_data) = swarm {
        crate::rendering::particle_swarm::reseed_particle_swarm(swarm_data, &disk_params, &config);
    }

    if let Some(ref mut t) = toast {
        t.message = format!(
            "📂 Loaded {} bodies [{}]",
            save_data.bodies.len(),
            detected_preset.display_name()
        );
        t.timer = 5.0;
    }

    info!(
        "📂 Loaded solar system state ({} bodies, {:?}) from: {}",
        save_data.bodies.len(),
        detected_preset,
        path
    );
}

/// System that listens for `CopySystemSeedClipboardEvent`, serializes the active state to a seed string,
/// and copies it to the system clipboard with toast confirmation.
#[allow(
    clippy::too_many_arguments,
    reason = "Seed export collects simulation resources, bodies, probes, and Alfvén waves"
)]
pub fn handle_clipboard_seed_export_events(
    mut events: MessageReader<CopySystemSeedClipboardEvent>,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    disk_params: Res<DiskParameters>,
    config: Res<SimulationConfig>,
    theia_state: Option<Res<crate::simulation::accretion::TheiaImpactState>>,
    lhb_state: Option<Res<crate::game::phases::LateHeavyBombardmentState>>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    mut clipboard: Option<ResMut<bevy::clipboard::Clipboard>>,
    mut toast: Option<ResMut<crate::game::ui::NotificationToast>>,
    bodies_query: BodySerializeQuery,
    phase5_queries: Phase5SerializeQueries,
) {
    for _ in events.read() {
        let entity_to_name = collect_entity_name_map(&bodies_query);
        let bodies =
            collect_bodies_save(&bodies_query, &entity_to_name, &phase5_queries.bodies_extra);
        let probes = collect_probes_save(&phase5_queries.probes);
        let alfven_waves =
            collect_standalone_alfven_save(&entity_to_name, &phase5_queries.alfven_waves);
        let mut save_data = build_system_save_data(
            &sim_time,
            &time_warp,
            &disk_params,
            &config,
            theia_state.as_deref(),
            lhb_state.as_deref(),
            scenario_state.as_deref(),
            bodies,
            probes,
            alfven_waves,
        );
        save_data.progression = build_progression_save(&phase5_queries);

        match encode_system_seed_json(&save_data) {
            Ok(seed) => {
                if let Some(ref mut clip) = clipboard
                    && let Err(e) = clip.set_text(seed)
                {
                    warn!("Failed to copy seed to system clipboard: {e:?}");
                }
                if let Some(ref mut t) = toast {
                    t.message = format!(
                        "📋 Copied system seed ({} bodies) to clipboard!",
                        save_data.bodies.len()
                    );
                    t.timer = 4.0;
                }
            }
            Err(e) => {
                if let Some(ref mut t) = toast {
                    t.message = format!("❌ Failed to encode seed: {e}");
                    t.timer = 4.0;
                }
            }
        }
    }
}

/// System that listens for `PasteSystemSeedClipboardEvent` or `ImportSystemSeedJsonEvent`,
/// deserializes the system seed, and rebuilds the solar system state.
#[allow(
    clippy::type_complexity,
    reason = "Full world restoration from clipboard/seed payload"
)]
pub fn handle_clipboard_seed_import_events(
    mut commands: Commands,
    mut paste_events: MessageReader<PasteSystemSeedClipboardEvent>,
    mut import_events: MessageReader<ImportSystemSeedJsonEvent>,
    mut clipboard: Option<ResMut<bevy::clipboard::Clipboard>>,
    mut disk_params: ResMut<DiskParameters>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    mut time_warp: ResMut<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut config: ResMut<SimulationConfig>,
    mut lhb_state: Option<ResMut<crate::game::phases::LateHeavyBombardmentState>>,
    bodies_query: Query<Entity, With<CelestialBody>>,
    mut camera_query: Query<&mut crate::rendering::camera::PanOrbitCamera>,
    (mut swarm, mut swarm_mesh_query): (
        Option<ResMut<crate::rendering::particle_swarm::ParticleSwarmData>>,
        Query<&mut Visibility, With<crate::rendering::particle_swarm::ParticleSwarmMesh>>,
    ),
    (mut scenario_state, mut toast): (
        Option<ResMut<crate::simulation::scenarios::ActiveScenarioState>>,
        Option<ResMut<crate::game::ui::NotificationToast>>,
    ),
    (mut phase_mgr, mut next_phase, mut reset_res): (
        Option<ResMut<crate::game::phases::PhaseManager>>,
        Option<ResMut<NextState<crate::game::phases::SystemPhase>>>,
        crate::simulation::scenarios::ScenarioResetResources,
    ),
) {
    let mut payload = None;
    for _ in paste_events.read() {
        if let Some(ref mut clip) = clipboard {
            match clip.fetch_text() {
                bevy::clipboard::ClipboardRead::Ready(Ok(text)) => payload = Some(text),
                mut pending => {
                    if let Some(Ok(text)) = pending.poll_result() {
                        payload = Some(text);
                    }
                }
            }
        }
    }
    for ev in import_events.read() {
        payload = Some(ev.payload.clone());
    }

    let Some(text) = payload else { return };

    let save_data = match decode_system_seed_json(&text) {
        Ok(data) => data,
        Err(e) => {
            warn!("Failed to parse system seed: {e}");
            if let Some(ref mut t) = toast {
                t.message = format!("❌ Invalid system seed: {e}");
                t.timer = 5.0;
            }
            return;
        }
    };

    despawn_active_bodies(&mut commands, &bodies_query);
    crate::simulation::scenarios::clear_transient_pools_and_entities(&mut commands, &mut reset_res);
    restore_simulation_resources(
        &save_data,
        &mut sim_time,
        &mut time_warp,
        &mut disk_params,
        &mut config,
        &mut energy_monitor,
        &mut reset_res.theia_state,
        &mut lhb_state,
    );
    restore_progression_and_events(&save_data, &mut reset_res);
    let links = spawn_and_link_saved_bodies(&mut commands, &save_data);
    spawn_saved_probes_and_waves(
        &mut commands,
        &save_data,
        &links,
        &mut reset_res.probe_counter,
    );
    let detected_preset = save_data
        .scenario_preset
        .unwrap_or_else(|| infer_scenario_preset(&save_data.bodies));
    let primary_ent = links.central_star_ent.or(links.first_body_ent);
    player_state.selected_entity = primary_ent;

    apply_loaded_scenario_and_camera(
        &save_data,
        detected_preset,
        &mut scenario_state,
        &mut phase_mgr,
        &mut next_phase,
        &disk_params,
        &mut config,
        &mut swarm_mesh_query,
        primary_ent,
        &mut camera_query,
    );

    if let Some(ref mut swarm_data) = swarm {
        crate::rendering::particle_swarm::reseed_particle_swarm(swarm_data, &disk_params, &config);
    }

    if let Some(ref mut t) = toast {
        t.message = format!(
            "📥 Imported seed ({} bodies) [{}]",
            save_data.bodies.len(),
            detected_preset.display_name()
        );
        t.timer = 5.0;
    }
}
