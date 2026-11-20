//! Exoplanet System Generator and Sandbox Scenarios.
//!
//! Provides multi-system presets:
//! - TRAPPIST-1 Resonant Ultracool Red Dwarf System (7 Earths)
//! - Kepler-16 Circumbinary System (Tatooine-like binary star pair with circumbinary planet)
//! - Hot Jupiter Inward Migration Scenario (Type II disk migration)
//! - Rogue Planet Flyby Perturbation Scenario (Hyperbolic interstellar interloper)
//! - Hayashi Minimum Mass Solar Nebula (Default Solar System)

pub mod exotic;
pub mod galaxy;
pub mod genesis;
pub mod kepler;
pub mod molecular_cloud;
pub mod sagittarius;
pub mod solar;
pub mod trappist;

pub use exotic::*;
pub use galaxy::*;
pub use genesis::*;
pub use kepler::*;
pub use molecular_cloud::*;
pub use sagittarius::*;
pub use solar::*;
pub use trappist::*;

use bevy::prelude::*;

use serde::{Deserialize, Serialize};

use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Supported Sandbox Scenario Presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default, Serialize, Deserialize)]
pub enum ScenarioPreset {
    GalacticCollision,
    #[default]
    SolarNebulaMmsn,
    AccretionDiskGenesis,
    Trappist1System,
    Kepler16Circumbinary,
    HotJupiterMigration,
    RoguePlanetFlyby,
    LittleRedDot,
    PulsarSystem,
    MagnetarOutburst,
    RelativisticBinary,
    KozaiLidovTriple,
    SagittariusAStar,
    MolecularCloudCluster,
    TidalDisruptionEvent,
}

impl ScenarioPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            ScenarioPreset::GalacticCollision => "N-Body Galaxy Merger",
            ScenarioPreset::SolarNebulaMmsn => "Hayashi Solar Nebula",
            ScenarioPreset::AccretionDiskGenesis => "Disk Genesis (Organic Planet Formation)",
            ScenarioPreset::Trappist1System => "TRAPPIST-1 (7 Resonant Earths)",
            ScenarioPreset::Kepler16Circumbinary => "Kepler-16 (Circumbinary Binary)",
            ScenarioPreset::HotJupiterMigration => "Hot Jupiter Migration",
            ScenarioPreset::RoguePlanetFlyby => "Rogue Planet Flyby",
            ScenarioPreset::LittleRedDot => "JWST Little Red Dot (Black Hole Star)",
            ScenarioPreset::PulsarSystem => "PSR B1257+12 (Pulsar & Zombie Planets)",
            ScenarioPreset::MagnetarOutburst => "SGR 1806-20 (Magnetar Giant Flare)",
            ScenarioPreset::RelativisticBinary => "PSR B1913+16 (Relativistic Binary)",
            ScenarioPreset::KozaiLidovTriple => "HD 80606 (Kozai-Lidov Triple)",
            ScenarioPreset::SagittariusAStar => "Sagittarius A* & S-Stars (Galactic Center)",
            ScenarioPreset::MolecularCloudCluster => "GMC Cluster (Jeans Instability)",
            ScenarioPreset::TidalDisruptionEvent => "Tidal Disruption Spaghetti",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ScenarioPreset::GalacticCollision => {
                "Witness the collision of two massive 100,000 N-body star clusters bound by dark matter."
            }
            ScenarioPreset::SolarNebulaMmsn => {
                "Default 4.5 Gyr Hayashi Minimum Mass Solar Nebula (MMSN) with central protostar and 10 protoplanetary embryos."
            }
            ScenarioPreset::AccretionDiskGenesis => {
                "Pristine Class II T-Tauri disk without starter planets. SPH viscous gas aerodynamic drag, water ice snow line trap (2.7 AU), and electrostatic coagulation build planets organically."
            }
            ScenarioPreset::Trappist1System => {
                "Ultracool M-dwarf (0.09 M☉) with 7 Earth-sized terrestrial worlds in a compact resonant Laplace chain (3 habitable)."
            }
            ScenarioPreset::Kepler16Circumbinary => {
                "Tatooine-like circumbinary system with K/M-dwarf binary pair and a Saturn-mass circumbinary giant at 0.70 AU."
            }
            ScenarioPreset::HotJupiterMigration => {
                "Massive gas giant (1.4 M_Jup) undergoing Type II disk torque inward migration from 5.2 AU down to 0.045 AU."
            }
            ScenarioPreset::RoguePlanetFlyby => {
                "A 3.5 M_Jup interstellar rogue planet screaming through the solar system at 38 km/s, scattering orbits."
            }
            ScenarioPreset::LittleRedDot => {
                "Cosmic Dawn (z ~ 8.5): A 400,000 M☉ (100,000 M☉-class) supermassive black hole seed encased in a dense, dust-free primordial hydrogen gas cocoon spanning 60 AU."
            }
            ScenarioPreset::PulsarSystem => {
                "Millisecond pulsar (1.40 M☉, 6.22 ms relativistic spin) with 3 confirmed zombie exoplanets (Draugr, Poltergeist, Phobetor) and an irradiated post-supernova fallback disk."
            }
            ScenarioPreset::MagnetarOutburst => {
                "Ultra-magnetized 10¹⁵ Gauss magnetar (SGR 1806-20) with starquake crustal fractures, glowing magnetic arches, an LBV hypergiant companion, and relativistic ejecta."
            }
            ScenarioPreset::RelativisticBinary => {
                "Hulse-Taylor binary pulsar with a 1.44 M☉ pulsar and 1.38 M☉ companion neutron star in an eccentric 0.013 AU orbit, exhibiting 4.22°/yr periastron advance and GW inspiral decay."
            }
            ScenarioPreset::KozaiLidovTriple => {
                "Hierarchical triple system: G-dwarf primary, inner giant inclined at 68°, and distant M-dwarf companion driving secular eccentricity pumping (e -> 0.88) and tidal migration."
            }
            ScenarioPreset::SagittariusAStar => {
                "Supermassive black hole Sgr A* (4.3×10⁶ M☉) with the relativistic S-star cluster (S2, S4714 at 8% c, S62, S29, S38) and the tidally sheared G2 cloud."
            }
            ScenarioPreset::MolecularCloudCluster => {
                "Cold pre-stellar molecular cloud core (24 M☉, 1100 AU) undergoing turbulent Jeans instability collapse, birthing an open cluster of protostellar seeds and galactic spiral arms."
            }
            ScenarioPreset::TidalDisruptionEvent => {
                "A doomed red giant star on a highly elliptical orbit plunges inside the Roche limit of a Supermassive Black Hole, triggering a violent Tidal Disruption Event (TDE)."
            }
        }
    }

    /// Clean filename slug for scenario-specific multi-save slots (e.g. `"trappist1"`).
    pub fn save_slot_slug(&self) -> &'static str {
        match self {
            ScenarioPreset::GalacticCollision => "galactic_collision",
            ScenarioPreset::SolarNebulaMmsn => "solar_nebula",
            ScenarioPreset::AccretionDiskGenesis => "disk_genesis",
            ScenarioPreset::Trappist1System => "trappist1",
            ScenarioPreset::Kepler16Circumbinary => "kepler16",
            ScenarioPreset::HotJupiterMigration => "hot_jupiter",
            ScenarioPreset::RoguePlanetFlyby => "rogue_flyby",
            ScenarioPreset::LittleRedDot => "little_red_dot",
            ScenarioPreset::PulsarSystem => "pulsar_system",
            ScenarioPreset::MagnetarOutburst => "magnetar",
            ScenarioPreset::RelativisticBinary => "relativistic_binary",
            ScenarioPreset::KozaiLidovTriple => "kozai_triple",
            ScenarioPreset::SagittariusAStar => "sgr_a_star",
            ScenarioPreset::MolecularCloudCluster => "molecular_cloud",
            ScenarioPreset::TidalDisruptionEvent => "tidal_disruption",
        }
    }

    /// Compact badge label for save-slot cards and HUD pills.
    pub fn short_label(&self) -> &'static str {
        match self {
            ScenarioPreset::GalacticCollision => "Galaxy Merger",
            ScenarioPreset::SolarNebulaMmsn => "Solar Nebula",
            ScenarioPreset::AccretionDiskGenesis => "Disk Genesis",
            ScenarioPreset::Trappist1System => "TRAPPIST-1",
            ScenarioPreset::Kepler16Circumbinary => "Kepler-16",
            ScenarioPreset::HotJupiterMigration => "Hot Jupiter",
            ScenarioPreset::RoguePlanetFlyby => "Rogue Flyby",
            ScenarioPreset::LittleRedDot => "Little Red Dot",
            ScenarioPreset::PulsarSystem => "PSR B1257+12",
            ScenarioPreset::MagnetarOutburst => "Magnetar",
            ScenarioPreset::RelativisticBinary => "Relativistic Binary",
            ScenarioPreset::KozaiLidovTriple => "Kozai-Lidov",
            ScenarioPreset::SagittariusAStar => "Sgr A*",
            ScenarioPreset::MolecularCloudCluster => "GMC Cluster",
            ScenarioPreset::TidalDisruptionEvent => "TDE",
        }
    }
}

/// Message event to request loading a new sandbox scenario.
#[derive(Event, Message, Debug, Clone, Copy)]
pub struct LoadScenarioEvent(pub ScenarioPreset);

/// Monotonic counter incremented on every scenario load, save load, or seed import.
/// Systems with `Local<...>` caches or async GPU readback channels compare against this
/// generation to flush stale state cleanly across reloads (including same-preset reloads).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScenarioReloadGeneration(pub u64);

/// Returns `true` when the given scenario preset has no active particle-swarm disk or galaxy stars.
/// Note: `GalacticCollision` renders its 100k–500k galaxy stars via `ParticleSwarmMesh`, so it is
/// explicitly *not* an empty-swarm preset.
pub fn is_empty_swarm_preset(preset: ScenarioPreset, disk_mass: f64) -> bool {
    if preset == ScenarioPreset::GalacticCollision {
        return false;
    }
    matches!(
        preset,
        ScenarioPreset::PulsarSystem
            | ScenarioPreset::MagnetarOutburst
            | ScenarioPreset::RelativisticBinary
            | ScenarioPreset::KozaiLidovTriple
            | ScenarioPreset::SagittariusAStar
            | ScenarioPreset::MolecularCloudCluster
            | ScenarioPreset::TidalDisruptionEvent
    ) || disk_mass <= 0.0
}

/// Active state tracking for scenarios with ongoing events (e.g. Migration, Flyby).
#[derive(Resource, Debug, Clone, Default)]
pub struct ActiveScenarioState {
    pub current_preset: ScenarioPreset,
    pub scenario_time_years: f64,
    pub migration_active: bool,
    pub migration_target_au: f64,
    pub rogue_planet_entity: Option<Entity>,
}

pub fn scenario_preset_camera_pose(preset: ScenarioPreset) -> (f32, f32, f32) {
    match preset {
        ScenarioPreset::GalacticCollision => (300_000.0, 0.0, 1.0),
        ScenarioPreset::Trappist1System => (0.12, 0.785, 0.75),
        ScenarioPreset::Kepler16Circumbinary => (2.2, 0.785, 0.65),
        ScenarioPreset::SolarNebulaMmsn => (16.0, 0.785, 0.62),
        ScenarioPreset::AccretionDiskGenesis => (24.0, 0.785, 0.65),
        ScenarioPreset::HotJupiterMigration => (10.0, 0.785, 0.62),
        ScenarioPreset::RoguePlanetFlyby => (35.0, 0.785, 0.62),
        ScenarioPreset::LittleRedDot => (160.0, 0.785, 0.62),
        ScenarioPreset::PulsarSystem => (1.2, 0.785, 0.65),
        ScenarioPreset::MagnetarOutburst => (6.5, 0.785, 0.62),
        ScenarioPreset::RelativisticBinary => (0.04, 0.785, 0.70),
        ScenarioPreset::KozaiLidovTriple => (15.0, 0.785, 0.65),
        ScenarioPreset::SagittariusAStar => (1200.0, 0.785, 0.65),
        ScenarioPreset::MolecularCloudCluster => (1650.0, 0.785, 0.65),
        ScenarioPreset::TidalDisruptionEvent => (250.0, 0.785, 0.65),
    }
}

fn spawn_scenario_preset(
    preset: ScenarioPreset,
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
    scenario_state: &mut ActiveScenarioState,
) -> Option<Entity> {
    match preset {
        ScenarioPreset::GalacticCollision => Some(
            crate::simulation::scenarios::galaxy::spawn_galactic_collision_scenario(
                commands,
                disk_params,
            ),
        ),
        ScenarioPreset::SolarNebulaMmsn => Some(spawn_solar_nebula_mmsn(commands, disk_params)),
        ScenarioPreset::AccretionDiskGenesis => {
            Some(spawn_accretion_disk_genesis(commands, disk_params))
        }
        ScenarioPreset::Trappist1System => Some(spawn_trappist_1_system(commands, disk_params)),
        ScenarioPreset::Kepler16Circumbinary => Some(spawn_kepler_16_system(commands, disk_params)),
        ScenarioPreset::HotJupiterMigration => {
            scenario_state.migration_active = true;
            scenario_state.migration_target_au = 0.045;
            Some(spawn_hot_jupiter_scenario(commands, disk_params))
        }
        ScenarioPreset::RoguePlanetFlyby => {
            let (star, rogue) = spawn_rogue_planet_scenario(commands, disk_params);
            scenario_state.rogue_planet_entity = Some(rogue);
            Some(star)
        }
        ScenarioPreset::LittleRedDot => Some(spawn_little_red_dot_scenario(commands, disk_params)),
        ScenarioPreset::PulsarSystem => Some(spawn_pulsar_system_scenario(commands, disk_params)),
        ScenarioPreset::MagnetarOutburst => {
            Some(spawn_magnetar_outburst_scenario(commands, disk_params))
        }
        ScenarioPreset::RelativisticBinary => {
            let (pulsar, _) = spawn_relativistic_binary_scenario(commands, disk_params);
            Some(pulsar)
        }
        ScenarioPreset::KozaiLidovTriple => {
            let (star, _, _) = spawn_kozai_triple_scenario(commands, disk_params);
            Some(star)
        }
        ScenarioPreset::SagittariusAStar => {
            Some(spawn_sagittarius_a_star_scenario(commands, disk_params))
        }
        ScenarioPreset::MolecularCloudCluster => {
            spawn_molecular_cloud_cluster_scenario(commands, disk_params)
        }
        ScenarioPreset::TidalDisruptionEvent => Some(
            crate::simulation::scenarios::spawn_tde_scenario(commands, disk_params),
        ),
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct ScenarioResetResources<'w, 's> {
    pub spawner: Option<ResMut<'w, crate::simulation::disk::planetesimals::PlanetesimalSpawner>>,
    pub theia_state: Option<ResMut<'w, crate::simulation::accretion::TheiaImpactState>>,
    pub galactic_tide: Option<ResMut<'w, crate::simulation::oort::GalacticTideState>>,
    pub predictor: Option<ResMut<'w, crate::simulation::predictor::TrajectoryPredictorState>>,
    pub slingshot: Option<ResMut<'w, SlingshotState>>,
    pub campaign: Option<ResMut<'w, crate::simulation::campaign::CampaignState>>,
    pub science: Option<ResMut<'w, crate::simulation::economy::ScienceData>>,
    pub techs: Option<ResMut<'w, crate::simulation::economy::UnlockedTechs>>,
    pub telemetry_history:
        Option<ResMut<'w, crate::simulation::telemetry::SimulationTelemetryHistory>>,
    pub event_log: Option<ResMut<'w, crate::game::ui::types::CosmicEventLog>>,
    pub event_mgr: Option<ResMut<'w, crate::simulation::events::CosmicEventManager>>,
    pub probe_counter: Option<ResMut<'w, crate::simulation::probes::ProbeCounter>>,
    #[allow(
        clippy::type_complexity,
        reason = "Effect and debris resource pools cleared on scenario reset or save load"
    )]
    pub pools: (
        Option<ResMut<'w, ImpactShockwavePool>>,
        Option<ResMut<'w, ImpactEjectaPool>>,
        Option<ResMut<'w, RocheDebrisPool>>,
        Option<ResMut<'w, TidalDisruptionPool>>,
        Option<ResMut<'w, crate::rendering::effects::supernova::SupernovaDebrisPool>>,
        Option<ResMut<'w, crate::rendering::effects::remnants::PersistentRemnantPool>>,
        Option<ResMut<'w, crate::rendering::effects::kilonova::KilonovaPool>>,
        Option<ResMut<'w, crate::rendering::surface_terrain::SurfaceTerrainState>>,
    ),
    #[allow(
        clippy::type_complexity,
        reason = "GPU readback channels, reload generation, and timeline scrubber reset on load"
    )]
    pub gpu_and_reload: (
        Option<ResMut<'w, ScenarioReloadGeneration>>,
        Option<ResMut<'w, crate::gpu::gmc_far_field::GmcFarField>>,
        Option<Res<'w, crate::gpu::compute_node::GpuReadbackReceiver>>,
        Option<Res<'w, crate::gpu::gmc_fluid::GmcCollapseEventReceiver>>,
        Option<ResMut<'w, crate::simulation::geology::types::TimelineScrubber>>,
    ),
    #[allow(
        clippy::type_complexity,
        reason = "Non-body transient entities cleared on scenario reset or save load"
    )]
    pub transient_entities: Query<
        'w,
        's,
        Entity,
        (
            Without<CelestialBody>,
            Or<(
                With<crate::simulation::neutron_stars::AlfvenBlastWave>,
                With<crate::rendering::effects::supernova::SupernovaVisualShell>,
                With<crate::rendering::surface_terrain::PlanetarySurfaceTerrainRoot>,
                With<crate::simulation::probes::SpaceProbe>,
            )>,
        ),
    >,
}

/// Clears all transient non-body entities and ephemeral effect/gameplay pools.
pub fn clear_transient_pools_and_entities(
    commands: &mut Commands,
    extra: &mut ScenarioResetResources,
) {
    for ent in extra.transient_entities.iter() {
        if let Ok(mut cmd) = commands.get_entity(ent) {
            cmd.try_despawn();
        }
    }
    if let Some(spawner) = extra.spawner.as_mut() {
        spawner.last_spawn_yr = 0.0;
        spawner.last_minor_body_spawn_yr = 0.0;
        spawner.total_spawned = 0;
        spawner.name_counter = 0;
    }
    if let Some(theia) = extra.theia_state.as_mut() {
        **theia = crate::simulation::accretion::TheiaImpactState::default();
    }
    if let Some(tide) = extra.galactic_tide.as_mut() {
        **tide = crate::simulation::oort::GalacticTideState::default();
    }
    if let Some(pred) = extra.predictor.as_mut() {
        pred.trajectory_points.clear();
        pred.active_encounter = None;
        pred.source_label.clear();
    }
    if let Some(slingshot) = extra.slingshot.as_mut() {
        **slingshot = SlingshotState::default();
    }
    if let Some(camp) = extra.campaign.as_mut() {
        **camp = crate::simulation::campaign::CampaignState::default();
    }
    if let Some(science) = extra.science.as_mut() {
        **science = crate::simulation::economy::ScienceData::default();
    }
    if let Some(techs) = extra.techs.as_mut() {
        **techs = crate::simulation::economy::UnlockedTechs::default();
    }
    if let Some(hist) = extra.telemetry_history.as_mut() {
        hist.clear();
    }
    if let Some(log) = extra.event_log.as_mut() {
        **log = crate::game::ui::types::CosmicEventLog::default();
    }
    if let Some(mgr) = extra.event_mgr.as_mut() {
        **mgr = crate::simulation::events::CosmicEventManager::default();
    }
    if let Some(counter) = extra.probe_counter.as_mut() {
        **counter = crate::simulation::probes::ProbeCounter::default();
    }
    if let Some(pool) = extra.pools.0.as_mut() {
        **pool = ImpactShockwavePool::default();
    }
    if let Some(pool) = extra.pools.1.as_mut() {
        **pool = ImpactEjectaPool::default();
    }
    if let Some(pool) = extra.pools.2.as_mut() {
        **pool = RocheDebrisPool::default();
    }
    if let Some(pool) = extra.pools.3.as_mut() {
        **pool = TidalDisruptionPool::default();
    }
    if let Some(pool) = extra.pools.4.as_mut() {
        pool.clear();
    }
    if let Some(pool) = extra.pools.5.as_mut() {
        pool.clear();
    }
    if let Some(pool) = extra.pools.6.as_mut() {
        pool.clear();
    }
    if let Some(terrain) = extra.pools.7.as_mut() {
        **terrain = crate::rendering::surface_terrain::SurfaceTerrainState::default();
    }
    if let Some(reload_gen) = extra.gpu_and_reload.0.as_mut() {
        reload_gen.0 = reload_gen.0.wrapping_add(1);
    }
    if let Some(far_field) = extra.gpu_and_reload.1.as_mut() {
        far_field.acc.clear();
    }
    if let Some(readback) = extra.gpu_and_reload.2.as_ref() {
        while let Ok(bytes) = readback.rx.try_recv() {
            let _ = readback.recycle_tx.try_send(bytes);
        }
    }
    if let Some(collapse_rx) = extra.gpu_and_reload.3.as_ref() {
        while collapse_rx.rx.try_recv().is_ok() {}
    }
    if let Some(scrubber) = extra.gpu_and_reload.4.as_mut() {
        **scrubber = crate::simulation::geology::types::TimelineScrubber::default();
    }
}

fn reset_scenario_simulation_state(
    sim_time: &mut SimTime,
    time_warp: &mut TimeWarp,
    energy_monitor: &mut EnergyMonitor,
    lhb_state: &mut crate::game::phases::LateHeavyBombardmentState,
    player_state: &mut PlayerInteractionState,
) {
    player_state.hovered_entity = None;
    player_state.impulse_target_entity = None;
    player_state.tractor_position = None;
    player_state.impulse_delta_v = None;

    sim_time.elapsed_years = 0.0;
    sim_time.current_dt_yr = 0.001;
    sim_time.step_count = 0;
    sim_time.visual_time_secs = 0.0;
    time_warp.multiplier = 1.0;
    time_warp.is_paused = false;
    energy_monitor.initial_total_energy = 0.0;
    energy_monitor.kinetic_energy = 0.0;
    energy_monitor.potential_energy = 0.0;
    energy_monitor.total_energy = 0.0;
    energy_monitor.relative_energy_drift = 0.0;
    energy_monitor.initialized = false;
    *lhb_state = crate::game::phases::LateHeavyBombardmentState::default();
}

pub fn update_scenario_system_phase(
    preset: ScenarioPreset,
    phase_mgr: &mut Option<ResMut<crate::game::phases::PhaseManager>>,
    next_phase: &mut Option<ResMut<NextState<crate::game::phases::SystemPhase>>>,
) {
    let (target_phase, desc, milestones) = match preset {
        ScenarioPreset::SolarNebulaMmsn => (
            crate::game::phases::SystemPhase::ProtoplanetaryDisk,
            "Dense protoplanetary disk orbiting young protostar. Dust and pebbles are accreting.",
            crate::game::phases::default_milestones(),
        ),
        ScenarioPreset::AccretionDiskGenesis => (
            crate::game::phases::SystemPhase::ProtoplanetaryDisk,
            "🌌 Protoplanetary Disk Genesis: SPH viscous gas fluid, aerodynamic drag, snow line trap & electrostatic coagulation.",
            crate::game::phases::genesis_milestones(),
        ),
        ScenarioPreset::MolecularCloudCluster => (
            crate::game::phases::SystemPhase::MolecularCloudCollapse,
            "🌌 Cold Molecular Cloud core undergoing gravitational Jeans collapse and fragmentation.",
            crate::game::phases::gmc_cluster_milestones(),
        ),
        ScenarioPreset::GalacticCollision => (
            crate::game::phases::SystemPhase::SpiralGalaxyEvolution,
            "🌀 Collision of two massive N-body spiral galaxies bound by dark matter halos.",
            crate::game::phases::default_milestones(),
        ),
        ScenarioPreset::Trappist1System => (
            crate::game::phases::SystemPhase::MatureSolarSystem,
            "🌟 TRAPPIST-1: Resonant 7-planet architecture orbiting ultracool red dwarf.",
            crate::game::phases::default_milestones(),
        ),
        ScenarioPreset::Kepler16Circumbinary => (
            crate::game::phases::SystemPhase::MatureSolarSystem,
            "🌟 Kepler-16: Circumbinary gas giant orbiting close stellar pair.",
            crate::game::phases::default_milestones(),
        ),
        ScenarioPreset::PulsarSystem => (
            crate::game::phases::SystemPhase::MatureSolarSystem,
            "🌟 Pulsar System: Diamond and rocky worlds surviving extreme pulsar wind.",
            crate::game::phases::default_milestones(),
        ),
        ScenarioPreset::SagittariusAStar => (
            crate::game::phases::SystemPhase::MatureSolarSystem,
            "🌌 Galactic Center: Relativistic S-star orbits around supermassive black hole Sgr A*.",
            crate::game::phases::default_milestones(),
        ),
        _ => (
            crate::game::phases::SystemPhase::MatureSolarSystem,
            "🌟 Mature stellar and planetary system.",
            crate::game::phases::default_milestones(),
        ),
    };
    if let Some(pm) = phase_mgr {
        **pm = crate::game::phases::PhaseManager::default();
        pm.current_phase = target_phase;
        pm.phase_description = desc;
        pm.milestones = milestones;
    }
    if let Some(np) = next_phase {
        np.set(target_phase);
    }
}

fn configure_scenario_particles_and_swarm(
    preset: ScenarioPreset,
    disk_params: &DiskParameters,
    config: &mut SimulationConfig,
    swarm_mesh_query: &mut Query<
        &mut Visibility,
        With<crate::rendering::particle_swarm::ParticleSwarmMesh>,
    >,
    swarm: &mut Option<ResMut<crate::rendering::particle_swarm::ParticleSwarmData>>,
) {
    let is_empty_swarm = is_empty_swarm_preset(preset, disk_params.disk_mass);

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

    if let Some(swarm_data) = swarm {
        crate::rendering::particle_swarm::reseed_particle_swarm(swarm_data, disk_params, config);
    }
}

fn align_camera_to_scenario(
    preset: ScenarioPreset,
    central_star_ent: Option<Entity>,
    camera_query: &mut Query<&mut crate::rendering::camera::PanOrbitCamera>,
) {
    if let Some(mut cam) = camera_query.iter_mut().next() {
        cam.surface_horizon_mode = false;
        cam.focus = Vec3::ZERO;
        cam.target_focus = Vec3::ZERO;
        cam.target_entity = if preset == ScenarioPreset::GalacticCollision {
            None
        } else {
            central_star_ent
        };
        let (target_r, target_yaw, target_pitch) = scenario_preset_camera_pose(preset);
        cam.radius = target_r;
        cam.target_radius = target_r;
        cam.yaw = target_yaw;
        cam.target_yaw = target_yaw;
        cam.pitch = target_pitch;
        cam.target_pitch = target_pitch;
    }
}

/// System that listens for `LoadScenarioEvent` and reinitializes the entire simulation.
#[allow(
    clippy::too_many_arguments,
    reason = "Scenario reload resets all simulation, UI, and pool resources cleanly"
)]
pub fn handle_load_scenario_events(
    mut commands: Commands,
    mut events: MessageReader<LoadScenarioEvent>,
    mut disk_params: ResMut<DiskParameters>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    mut time_warp: ResMut<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut scenario_state: ResMut<ActiveScenarioState>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    bodies_query: Query<Entity, With<CelestialBody>>,
    mut camera_query: Query<&mut crate::rendering::camera::PanOrbitCamera>,
    mut swarm: Option<ResMut<crate::rendering::particle_swarm::ParticleSwarmData>>,
    mut config: ResMut<SimulationConfig>,
    mut swarm_mesh_query: Query<
        &mut Visibility,
        With<crate::rendering::particle_swarm::ParticleSwarmMesh>,
    >,
    (mut phase_mgr, mut next_phase, mut reset_res): (
        Option<ResMut<crate::game::phases::PhaseManager>>,
        Option<ResMut<NextState<crate::game::phases::SystemPhase>>>,
        ScenarioResetResources,
    ),
) {
    let Some(preset) = events.read().last().map(|e| e.0) else {
        return;
    };
    info!("🌟 Loading Scenario Preset: {:?}", preset);

    for ent in bodies_query.iter() {
        if let Ok(mut cmd) = commands.get_entity(ent) {
            cmd.try_despawn();
        }
    }
    clear_transient_pools_and_entities(&mut commands, &mut reset_res);

    reset_scenario_simulation_state(
        &mut sim_time,
        &mut time_warp,
        &mut energy_monitor,
        &mut lhb_state,
        &mut player_state,
    );

    update_scenario_system_phase(preset, &mut phase_mgr, &mut next_phase);

    *disk_params = DiskParameters::default();
    scenario_state.rogue_planet_entity = None;
    scenario_state.current_preset = preset;
    scenario_state.scenario_time_years = 0.0;
    scenario_state.migration_active = false;
    config.gas_density_scale = match preset {
        ScenarioPreset::SolarNebulaMmsn
        | ScenarioPreset::AccretionDiskGenesis
        | ScenarioPreset::MolecularCloudCluster
        | ScenarioPreset::HotJupiterMigration
        | ScenarioPreset::LittleRedDot => 1.0,
        ScenarioPreset::RoguePlanetFlyby => 0.5,
        ScenarioPreset::Trappist1System
        | ScenarioPreset::Kepler16Circumbinary
        | ScenarioPreset::PulsarSystem
        | ScenarioPreset::MagnetarOutburst
        | ScenarioPreset::RelativisticBinary
        | ScenarioPreset::KozaiLidovTriple
        | ScenarioPreset::SagittariusAStar
        | ScenarioPreset::GalacticCollision
        | ScenarioPreset::TidalDisruptionEvent => {
            disk_params.gas_disk_lifetime_yr = 0.0;
            0.0
        }
    };

    let central_star_ent =
        spawn_scenario_preset(preset, &mut commands, &mut disk_params, &mut scenario_state);

    player_state.selected_entity = central_star_ent;

    configure_scenario_particles_and_swarm(
        preset,
        &disk_params,
        &mut config,
        &mut swarm_mesh_query,
        &mut swarm,
    );

    align_camera_to_scenario(preset, central_star_ent, &mut camera_query);
}

/// System to execute continuous scenario dynamics (e.g. Type II migration drag).
pub fn update_active_scenarios(
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    config: Option<Res<SimulationConfig>>,
    mut scenario_state: ResMut<ActiveScenarioState>,
    mut bodies_query: Query<(&mut SimVelocity, &SimPosition, &mut CelestialBody, &Mass)>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt = sim_time.current_dt_yr;
    scenario_state.scenario_time_years += dt;

    if scenario_state.migration_active
        && scenario_state.current_preset == ScenarioPreset::HotJupiterMigration
    {
        let disk_migration_active = config.as_ref().is_some_and(|c| c.gas_density_scale > 0.001);

        for (mut vel, pos, mut body, _mass) in bodies_query.iter_mut() {
            if body.name.contains("Hot Jupiter") {
                let r = (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt();
                if r > scenario_state.migration_target_au {
                    if !disk_migration_active {
                        let v_dir = vel.0.normalize_or_zero();
                        let inward_thrust = 0.0025 * (r / 5.2).clamp(0.2, 1.0);
                        vel.0 -= v_dir * (inward_thrust * dt);
                    }
                } else if !body.name.contains("Parked") {
                    body.name = "Ultra-Short Period Hot Jupiter (Parked)".to_string();
                }
            }
        }
    }
}
pub mod tde;
pub use tde::*;
