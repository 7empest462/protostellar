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
