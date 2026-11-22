//! Data structures and ECS event types for solar system serialization.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::simulation::atmosphere_escape::AtmosphericEscapeState;
use crate::simulation::components::*;
use crate::simulation::kozai_lidov::KozaiLidovState;
use crate::simulation::relativity::RelativisticState;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;

/// Current save-file schema version written by `build_system_save_data`.
/// Saves with `version < CURRENT_SAVE_SCHEMA_VERSION` may run legacy thermal migration helpers.
pub const CURRENT_SAVE_SCHEMA_VERSION: u32 = 2;

/// Root serialized structure representing a snapshot of the solar system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Active space probes and orbiters/rovers in the system.
    #[serde(default)]
    pub probes: Vec<SpaceProbeSave>,
    /// Standalone expanding Alfvén blast wave shock fronts.
    #[serde(default)]
    pub alfven_waves: Vec<AlfvenBlastWaveSave>,
    /// Saved progression mode, science telemetry, unlocked tech nodes, and crisis stats.
    #[serde(default)]
    pub progression: Option<ProgressionSave>,
}

/// Serializable snapshot of progression mode, economy state, and crisis counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionSave {
    /// Active game mode (Sandbox or Progression).
    #[serde(default)]
    pub mode: crate::simulation::economy::GameMode,
    /// Accumulated science and harvest rates.
    #[serde(default)]
    pub science_data: crate::simulation::economy::ScienceData,
    /// Unlocked technology IDs.
    #[serde(default)]
    pub unlocked_techs: Vec<crate::simulation::economy::TechnologyId>,
    /// Crises averted before catastrophic impact.
    #[serde(default)]
    pub crises_averted: u32,
    /// Crises partially mitigated.
    #[serde(default)]
    pub crises_mitigated: u32,
    /// Catastrophic crisis outcomes.
    #[serde(default)]
    pub crises_catastrophic: u32,
    /// Whether a Shkadov stellar engine is active.
    #[serde(default)]
    pub stellar_engine_active: bool,
    /// Whether scheduled background cosmic crises are enabled.
    #[serde(default)]
    pub auto_hazards_enabled: bool,
    /// Total cosmic crises survived or resolved.
    #[serde(default)]
    pub total_hazards_survived: u32,
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
    #[serde(default)]
    pub lhb_migration_progress: f64,
    #[serde(default)]
    pub lhb_resonance_ratio: f64,
    #[serde(default)]
    pub lhb_water_delivered_earth_masses: f64,
    #[serde(default)]
    pub lhb_comets_scattered: usize,
    #[serde(default)]
    pub lhb_time_active_years: f64,
    #[serde(default)]
    pub lhb_manual_trigger_requested: bool,
}

/// Serialized satellite orbit link referencing parent by persistent save ID or name fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteSave {
    /// Persistent save ID of the parent body (preferred on reload).
    #[serde(default)]
    pub parent_id: Option<u64>,
    /// Name of the primary parent body (e.g. "Earth", "Jupiter").
    pub parent_name: String,
    /// Semi-major axis in AU relative to parent body.
    pub semi_major_axis_au: f64,
    /// Orbital period in years around parent body.
    pub orbital_period_years: f64,
    /// Current true anomaly in radians.
    pub true_anomaly: f64,
}

/// Serialized Roche lobe overflow donor-companion link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocheLobeOverflowSave {
    #[serde(default)]
    pub companion_id: Option<u64>,
    #[serde(default)]
    pub companion_name: String,
    pub mass_transfer_rate: f64,
    pub l1_distance_au: f64,
}

/// Serialized Late Heavy Bombardment impactor target record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LhbImpactorSave {
    #[serde(default)]
    pub target_id: Option<u64>,
    #[serde(default)]
    pub target_name: String,
    pub spawn_time_yr: f64,
    pub expected_arrival_yr: f64,
    pub target_r: f64,
    pub target_q: f64,
}

/// Serialized guided bombardment projectile record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BombardmentProjectileSave {
    #[serde(default)]
    pub target_id: Option<u64>,
    #[serde(default)]
    pub target_name: String,
    pub bombardment_type: crate::simulation::terraforming::BombardmentType,
    pub launch_time_yr: f64,
    pub expected_arrival_yr: f64,
    pub trail_color_rgba: [f32; 4],
    pub is_detonated: bool,
    pub relative_offset: DVec3,
    pub initial_distance: f64,
    pub flight_duration_secs: f32,
    pub flight_elapsed_secs: f32,
}

/// Serialized magnetar starquake Alfvén blast wave shock front.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlfvenBlastWaveSave {
    #[serde(default)]
    pub source_id: Option<u64>,
    #[serde(default)]
    pub source_name: String,
    pub origin_au: DVec3,
    pub radius_au: f64,
    pub expansion_speed_au_yr: f64,
    pub energy_erg: f64,
    pub luminosity_solar: f64,
    pub synchrotron_brightness: f32,
    pub age_seconds: f64,
    pub max_radius_au: f64,
}

/// Serialized probe flight guidance state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightComputerSave {
    #[serde(default)]
    pub target_id: Option<u64>,
    pub target_name: String,
    pub launch_pos: DVec3,
    pub distance_to_target: f64,
    pub relative_speed: f64,
}

/// Complete serialized snapshot of a deployed `SpaceProbe`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceProbeSave {
    pub probe: crate::simulation::probes::SpaceProbe,
    #[serde(default)]
    pub flight_computer: Option<FlightComputerSave>,
    pub name: String,
    pub position: DVec3,
    pub velocity: DVec3,
    pub mass: f64,
    pub radius: f64,
    pub temperature: f64,
    pub composition: Composition,
}

/// Complete serialized record of an individual celestial body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CelestialBodySave {
    #[serde(default)]
    pub save_id: Option<u64>,
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
    pub tidal_host_id: Option<u64>,
    #[serde(default)]
    pub relativistic_state: Option<RelativisticState>,
    #[serde(default)]
    pub atmospheric_escape: Option<AtmosphericEscapeState>,
    #[serde(default)]
    pub kozai_lidov: Option<KozaiLidovState>,
    #[serde(default)]
    pub kozai_perturber_id: Option<u64>,
    #[serde(default)]
    pub geological_state: Option<crate::simulation::geology::GeologicalState>,
    #[serde(default)]
    pub terraforming: Option<crate::simulation::terraforming::TerraformingAtmosphere>,
    #[serde(default)]
    pub volcanic_state: Option<VolcanicState>,
    #[serde(default)]
    pub atmospheric_storm: Option<AtmosphericStormState>,
    #[serde(default)]
    pub kerr_black_hole: Option<crate::simulation::relativity::kerr::KerrBlackHole>,
    #[serde(default)]
    pub pulsar_glitch: Option<crate::simulation::neutron_stars::PulsarGlitchState>,
    #[serde(default)]
    pub magnetar_stress: Option<crate::simulation::neutron_stars::MagnetarCrustStress>,
    #[serde(default)]
    pub oort_comet: Option<crate::simulation::oort::OortComet>,
    #[serde(default)]
    pub stellar_flare: Option<crate::simulation::space_weather::StellarFlareState>,
    #[serde(default)]
    pub auroral_oval: Option<crate::simulation::space_weather::AuroralOvalState>,
    #[serde(default)]
    pub cataclysmic_variable: Option<crate::simulation::accretion::CataclysmicVariable>,
    #[serde(default)]
    pub is_gmc_promoted_system: bool,
    #[serde(default)]
    pub circumstellar_disk: Option<CircumstellarDisk>,
    #[serde(default)]
    pub relativistic_jet: Option<RelativisticJetState>,
    #[serde(default)]
    pub dyson_swarm: Option<crate::rendering::effects::megastructures::DysonSwarm>,
    #[serde(default)]
    pub roche_lobe_overflow: Option<RocheLobeOverflowSave>,
    #[serde(default)]
    pub lhb_impactor: Option<LhbImpactorSave>,
    #[serde(default)]
    pub bombardment_projectile: Option<BombardmentProjectileSave>,
    #[serde(default)]
    pub alfven_blast_wave: Option<AlfvenBlastWaveSave>,
    #[serde(default)]
    pub atmospheric_escape_tail: Option<AtmosphericEscapeTail>,
    #[serde(default)]
    pub habitability_timer: Option<crate::simulation::biology::civilization::HabitabilityTimer>,
    #[serde(default)]
    pub auroral_storm_excitation: Option<crate::simulation::space_weather::AuroralStormExcitation>,
    #[serde(default)]
    pub stellar_activity_clock: Option<crate::simulation::space_weather::StellarActivityClock>,
    #[serde(default)]
    pub rogue_interloper: Option<crate::simulation::events::RogueInterloper>,
    #[serde(default)]
    pub is_hazard_comet: bool,
    #[serde(default)]
    pub civilization: Option<crate::simulation::biology::CivilizationState>,
    #[serde(default)]
    pub crisis_impactor: Option<crate::simulation::events::CrisisImpactor>,
}

impl Default for CelestialBodySave {
    fn default() -> Self {
        Self {
            save_id: None,
            name: String::new(),
            body_type: BodyType::TerrestrialPlanet,
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
            mass: 0.0,
            radius: 0.0,
            temperature: 0.0,
            luminosity: 0.0,
            composition: Composition::default(),
            spin: SpinState::default(),
            differentiation: None,
            volatile_inventory: None,
            ring_system: None,
            basins: None,
            climate: None,
            biosphere: None,
            electromagnetic: None,
            is_central_star: false,
            ignition_state: None,
            stellar_evolution: None,
            black_hole_state: None,
            satellite: None,
            tidal_state: None,
            tidal_host_id: None,
            relativistic_state: None,
            atmospheric_escape: None,
            kozai_lidov: None,
            kozai_perturber_id: None,
            geological_state: None,
            terraforming: None,
            volcanic_state: None,
            atmospheric_storm: None,
            kerr_black_hole: None,
            pulsar_glitch: None,
            magnetar_stress: None,
            oort_comet: None,
            stellar_flare: None,
            auroral_oval: None,
            cataclysmic_variable: None,
            is_gmc_promoted_system: false,
            circumstellar_disk: None,
            relativistic_jet: None,
            dyson_swarm: None,
            roche_lobe_overflow: None,
            lhb_impactor: None,
            bombardment_projectile: None,
            alfven_blast_wave: None,
            atmospheric_escape_tail: None,
            habitability_timer: None,
            auroral_storm_excitation: None,
            stellar_activity_clock: None,
            rogue_interloper: None,
            is_hazard_comet: false,
            civilization: None,
            crisis_impactor: None,
        }
    }
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

/// Message event to request copying the active system state as a compact seed to the clipboard.
#[derive(Event, Message, Debug, Clone, Default)]
pub struct CopySystemSeedClipboardEvent;

/// Message event to request reading a system seed from the clipboard and instantiating it.
#[derive(Event, Message, Debug, Clone, Default)]
pub struct PasteSystemSeedClipboardEvent;

/// Message event to import a raw JSON or prefixed system seed string directly.
#[derive(Event, Message, Debug, Clone)]
pub struct ImportSystemSeedJsonEvent {
    pub payload: String,
}

/// System seed prefix distinguishing Protostellar clipboard payloads.
pub const SEED_PREFIX: &str = "PROTOSTELLAR_SEED_V1:";
