//! System State Save / Load & Scenario Serializer (JSON).
//!
//! Provides full serialization and deserialization of the active solar system state,
//! capturing orbital positions, velocities, masses, thermodynamic parameters, chemical
//! compositions, planetary rings, impact basins, and global disk configurations.

use bevy::math::DVec3;
use bevy::prelude::*;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::simulation::atmosphere_escape::AtmosphericEscapeState;
use crate::simulation::components::*;
use crate::simulation::kozai_lidov::KozaiLidovState;
use crate::simulation::relativity::RelativisticState;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;

/// Root serialized structure representing a snapshot of the solar system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSaveData {
    /// Schema version for backwards/forwards compatibility.
    pub version: u32,
    /// Simulation epoch in elapsed years.
    pub timestamp_epoch_yr: f64,
    /// Integration steps executed.
    pub step_count: u64,
    /// Saved time warp controls.
    pub time_warp: TimeWarpSave,
    /// Protoplanetary disk parameters.
    pub disk_parameters: DiskParameters,
    /// Global simulation configuration overrides.
    #[serde(default)]
    pub config_save: ConfigSave,
    /// List of celestial bodies in the system.
    pub bodies: Vec<CelestialBodySave>,
    /// System event flags (e.g. Moon formation, LHB).
    #[serde(default)]
    pub event_flags: EventFlagsSave,
    /// Saved scenario preset (e.g. MolecularCloudCluster, Trappist1System, SolarNebulaMmsn).
    #[serde(default)]
    pub scenario_preset: Option<crate::simulation::scenarios::ScenarioPreset>,
}

/// Serialized time warp parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWarpSave {
    pub multiplier: f64,
    pub is_paused: bool,
}

impl Default for TimeWarpSave {
    fn default() -> Self {
        Self {
            multiplier: 50.0,
            is_paused: false,
        }
    }
}

/// Serialized subset of simulation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSave {
    pub gas_density_scale: f32,
    pub size_exaggeration: f32,
}

impl Default for ConfigSave {
    fn default() -> Self {
        Self {
            gas_density_scale: 1.0,
            size_exaggeration: 1.0,
        }
    }
}

/// Serialized event state flags.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventFlagsSave {
    pub theia_moon_formed: bool,
    pub lhb_active: bool,
    pub lhb_resonance_crossed: bool,
}

/// Serialized satellite orbit link referencing parent by body name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteSave {
    /// Name of the primary parent body (e.g. "Earth", "Jupiter").
    pub parent_name: String,
    /// Semi-major axis in AU relative to parent body.
    pub semi_major_axis_au: f64,
    /// Orbital period in years around parent body.
    pub orbital_period_years: f64,
    /// Current true anomaly in radians.
    pub true_anomaly: f64,
}

/// Complete serialized record of an individual celestial body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CelestialBodySave {
    pub name: String,
    pub body_type: BodyType,
    pub position: DVec3,
    pub velocity: DVec3,
    pub mass: f64,
    pub radius: f64,
    pub temperature: f64,
    pub luminosity: f64,
    pub composition: Composition,
    pub spin: SpinState,
    #[serde(default)]
    pub differentiation: Option<InternalDifferentiation>,
    #[serde(default)]
    pub volatile_inventory: Option<VolatileInventory>,
    #[serde(default)]
    pub ring_system: Option<PlanetaryRingSystem>,
    #[serde(default)]
    pub basins: Option<Vec<ImpactBasin>>,
    #[serde(default)]
    pub climate: Option<PlanetaryClimate>,
    #[serde(default)]
    pub biosphere: Option<BiosphereState>,
    #[serde(default)]
    pub electromagnetic: Option<ElectromagneticFieldState>,
    #[serde(default)]
    pub is_central_star: bool,
    #[serde(default)]
    pub ignition_state: Option<IgnitionState>,
    #[serde(default)]
    pub stellar_evolution: Option<StellarEvolutionState>,
    #[serde(default)]
    pub black_hole_state: Option<BlackHoleStarState>,
    #[serde(default)]
    pub satellite: Option<SatelliteSave>,
    #[serde(default)]
    pub tidal_state: Option<TidalState>,
    #[serde(default)]
    pub relativistic_state: Option<RelativisticState>,
    #[serde(default)]
    pub atmospheric_escape: Option<AtmosphericEscapeState>,
    #[serde(default)]
    pub kozai_lidov: Option<KozaiLidovState>,
}

/// Message event to request saving the active system state to disk.
#[derive(Event, Message, Debug, Clone)]
pub struct SaveSystemEvent {
    pub filename: String,
}

/// Message event to request loading a system state from disk.
#[derive(Event, Message, Debug, Clone)]
pub struct LoadSystemEvent {
    pub filename: String,
}

/// Normalizes a save filename, ensuring it points into the `saves/` folder if no path separators are present.
pub fn normalize_save_path(filename: &str) -> String {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        "saves/quicksave.json".to_string()
    } else if trimmed.contains('/') || trimmed.contains('\\') {
        trimmed.to_string()
    } else {
        format!("saves/{trimmed}")
    }
}

/// Saves a `SystemSaveData` snapshot to the specified JSON file path.
pub fn save_system_to_file(data: &SystemSaveData, path_str: &str) -> Result<(), std::io::Error> {
    let path = Path::new(path_str);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Loads a `SystemSaveData` snapshot from the specified JSON file path.
pub fn load_system_from_file(path_str: &str) -> Result<SystemSaveData, std::io::Error> {
    let content = std::fs::read_to_string(path_str)?;
    let data: SystemSaveData = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(data)
}

/// Infers the intended scenario preset from the contents of a saved celestial body list
/// when the save file was created without an explicit `scenario_preset` tag.
pub fn infer_scenario_preset(
    bodies: &[CelestialBodySave],
) -> crate::simulation::scenarios::ScenarioPreset {
    use crate::simulation::scenarios::ScenarioPreset;

    if bodies.iter().any(|b| {
        b.name.contains("Protostar")
            || b.name.contains("Supermassive Black Hole")
            || b.name.contains("SMBH")
            || b.name.contains("NovaCore")
    }) || bodies.len() > 100
    {
        ScenarioPreset::MolecularCloudCluster
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
    } else if bodies.iter().any(|b| b.name.contains("Rogue")) {
        ScenarioPreset::RoguePlanetFlyby
    } else if bodies.iter().any(|b| b.name.contains("Hot Jupiter")) {
        ScenarioPreset::HotJupiterMigration
    } else {
        ScenarioPreset::SolarNebulaMmsn
    }
}

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
        &'static Temperature,
        &'static Luminosity,
        &'static Composition,
        &'static SpinState,
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
    ),
>;

/// Extracts celestial bodies into serializable records.
fn collect_bodies_save(bodies_query: &BodySerializeQuery) -> Vec<CelestialBodySave> {
    let mut entity_to_name = HashMap::new();
    for (ent, body, ..) in bodies_query.iter() {
        entity_to_name.insert(ent, body.name.clone());
    }

    let mut bodies = Vec::new();
    for (
        _ent,
        body,
        mass,
        pos,
        vel,
        rad,
        temp,
        lum,
        comp,
        spin,
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
    ) in bodies_query.iter()
    {
        let satellite = opt_sat.map(|sat| SatelliteSave {
            parent_name: entity_to_name
                .get(&sat.parent)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
            semi_major_axis_au: sat.semi_major_axis_au,
            orbital_period_years: sat.orbital_period_years,
            true_anomaly: sat.true_anomaly,
        });

        bodies.push(CelestialBodySave {
            name: body.name.clone(),
            body_type: body.body_type,
            position: pos.0,
            velocity: vel.0,
            mass: mass.0,
            radius: rad.0,
            temperature: temp.0,
            luminosity: lum.0,
            composition: *comp,
            spin: *spin,
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
            relativistic_state: opt_rel.copied(),
            atmospheric_escape: opt_escape.copied(),
            kozai_lidov: opt_kozai.cloned(),
        });
    }

    bodies
}

/// System that listens for `SaveSystemEvent` and writes the solar system snapshot to disk.
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
) {
    for event in events.read() {
        let path = normalize_save_path(&event.filename);
        let bodies = collect_bodies_save(&bodies_query);

        let save_data = SystemSaveData {
            version: 1,
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
                theia_moon_formed: theia_state.as_ref().is_some_and(|s| s.moon_formed),
                lhb_active: lhb_state.as_ref().is_some_and(|s| s.is_active),
                lhb_resonance_crossed: lhb_state.as_ref().is_some_and(|s| s.resonance_crossed),
            },
            scenario_preset: scenario_state.as_ref().map(|s| s.current_preset),
        };

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

/// Spawns an individual deserialized celestial body entity.
fn spawn_saved_body(commands: &mut Commands, save: &CelestialBodySave) -> Entity {
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
        Temperature(save.temperature),
        Luminosity(save.luminosity),
        AngularMomentum(save.position.cross(save.velocity) * save.mass),
        save.composition,
        save.spin,
    ));

    if let Some(diff) = save.differentiation {
        cmd.insert(diff);
    }
    if let Some(vol) = save.volatile_inventory {
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
    if let Some(climate) = save.climate {
        cmd.insert(climate);
    }
    if let Some(bio) = save.biosphere {
        cmd.insert(bio);
    }
    if let Some(em) = save.electromagnetic {
        cmd.insert(em);
    }
    if let Some(ign) = save.ignition_state {
        cmd.insert(ign);
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

    cmd.id()
}

fn despawn_active_bodies(
    commands: &mut Commands,
    bodies_query: &Query<Entity, With<CelestialBody>>,
) {
    for ent in bodies_query.iter() {
        if let Ok(mut cmd) = commands.get_entity(ent) {
            cmd.try_despawn();
        }
    }
}

fn restore_simulation_resources(
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

    if let Some(ref mut theia) = theia_state {
        theia.intercept_active = false;
        theia.moon_formed = save_data.event_flags.theia_moon_formed;
        theia.intercept_start_year = None;
        theia.manual_trigger_requested = false;
        theia.intercept_steps = 0;
        theia.target_primary = None;
    }

    if let Some(ref mut lhb) = lhb_state {
        lhb.is_active = save_data.event_flags.lhb_active;
        lhb.resonance_crossed = save_data.event_flags.lhb_resonance_crossed;
    }
}

fn spawn_and_link_saved_bodies(
    commands: &mut Commands,
    bodies: &[CelestialBodySave],
) -> (Option<Entity>, Option<Entity>) {
    let mut name_to_entity = HashMap::new();
    let mut pending_satellites = Vec::new();
    let mut central_star_ent = None;
    let mut first_body_ent = None;

    for body_save in bodies {
        let ent = spawn_saved_body(commands, body_save);
        if first_body_ent.is_none() {
            first_body_ent = Some(ent);
        }
        if body_save.is_central_star {
            central_star_ent = Some(ent);
        }
        name_to_entity.insert(body_save.name.clone(), ent);

        if let Some(ref sat) = body_save.satellite {
            pending_satellites.push((ent, sat.clone()));
        }
    }

    for (moon_ent, sat_save) in pending_satellites {
        if let Some(&parent_ent) = name_to_entity.get(&sat_save.parent_name) {
            if let Ok(mut moon_cmd) = commands.get_entity(moon_ent) {
                moon_cmd.insert(SatelliteOf {
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

    (central_star_ent, first_body_ent)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Scenario and camera state restoration requires several subsystem references"
)]
fn apply_loaded_scenario_and_camera(
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
    if let Some(ref mut s_state) = scenario_state {
        s_state.current_preset = detected_preset;
        s_state.scenario_time_years = save_data.timestamp_epoch_yr;
        s_state.migration_active = false;
    }

    crate::simulation::scenarios::update_scenario_system_phase(
        detected_preset,
        phase_mgr,
        next_phase,
    );

    let is_empty_swarm = matches!(
        detected_preset,
        crate::simulation::scenarios::ScenarioPreset::PulsarSystem
            | crate::simulation::scenarios::ScenarioPreset::MagnetarOutburst
            | crate::simulation::scenarios::ScenarioPreset::RelativisticBinary
            | crate::simulation::scenarios::ScenarioPreset::KozaiLidovTriple
            | crate::simulation::scenarios::ScenarioPreset::SagittariusAStar
            | crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster
    ) || disk_params.disk_mass <= 0.0;

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
    (mut theia_state, mut lhb_state): (
        Option<ResMut<crate::simulation::accretion::TheiaImpactState>>,
        Option<ResMut<crate::game::phases::LateHeavyBombardmentState>>,
    ),
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
    (mut phase_mgr, mut next_phase): (
        Option<ResMut<crate::game::phases::PhaseManager>>,
        Option<ResMut<NextState<crate::game::phases::SystemPhase>>>,
    ),
) {
    for event in events.read() {
        let path = normalize_save_path(&event.filename);
        let save_data = match load_system_from_file(&path) {
            Ok(data) => data,
            Err(e) => {
                error!("❌ Failed to load system state from {}: {}", path, e);
                if let Some(ref mut t) = toast {
                    t.message = format!("❌ Load failed: {e}");
                    t.timer = 5.0;
                }
                continue;
            }
        };

        // 1. Despawn existing celestial bodies
        despawn_active_bodies(&mut commands, &bodies_query);

        // 2. Restore simulation resources and flags
        restore_simulation_resources(
            &save_data,
            &mut sim_time,
            &mut time_warp,
            &mut disk_params,
            &mut config,
            &mut energy_monitor,
            &mut theia_state,
            &mut lhb_state,
        );

        // 3. Spawn bodies & resolve satellite links
        let (central_star_ent, first_body_ent) =
            spawn_and_link_saved_bodies(&mut commands, &save_data.bodies);

        // 4. Update scenario preset, system phase, and camera
        let detected_preset = save_data
            .scenario_preset
            .unwrap_or_else(|| infer_scenario_preset(&save_data.bodies));
        let primary_ent = central_star_ent.or(first_body_ent);
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
            crate::rendering::particle_swarm::reseed_particle_swarm(
                swarm_data,
                &disk_params,
                &config,
            );
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
}
