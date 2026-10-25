//! Data structures, event definitions, and configuration for Dynamic Cosmic Hazards.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Supported types of dynamic cosmic hazards and deep-space catastrophes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CosmicEventType {
    /// Hyperbolic brown dwarf or sub-stellar interloper disturbing orbits.
    RogueStarIncursion,
    /// Collimated ultra-relativistic ionizing blast stripping planetary atmospheres.
    GammaRayBurst,
    /// Influx of volatile-rich retrograde Oort cloud comets heading to the inner system.
    CometOutburstSwarm,
    /// Extreme coronal mass ejection shockwave disrupting space probes and driving G5 auroras.
    SolarSuperflare,
}

impl CosmicEventType {
    /// Returns the human-readable display title.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::RogueStarIncursion => "Rogue Star Incursion",
            Self::GammaRayBurst => "Gamma-Ray Burst (GRB)",
            Self::CometOutburstSwarm => "Cometary Outburst Swarm",
            Self::SolarSuperflare => "Solar Superflare & CME",
        }
    }

    /// Returns a descriptive overview of the hazard's astrophysical impact.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::RogueStarIncursion => {
                "A massive 0.05 M☉ brown dwarf on a hyperbolic flyby trajectory passes through the system, perturbing planetary orbits."
            }
            Self::GammaRayBurst => {
                "Intense relativistic ionizing radiation sweeps across the system, stripping unshielded atmospheres and endangering biospheres."
            }
            Self::CometOutburstSwarm => {
                "A perturbation in the Oort cloud hurls high-velocity volatile-rich comets inward, delivering water ice to terrestrial worlds."
            }
            Self::SolarSuperflare => {
                "A Carrington x100 magnetic reconnection flare unleashes a violent CME plasma wave, disrupting probes and inducing global auroras."
            }
        }
    }

    /// Science Data points awarded upon observing and surviving this cosmic event.
    #[must_use]
    pub fn science_bounty(self) -> f64 {
        match self {
            Self::RogueStarIncursion => 500.0,
            Self::GammaRayBurst => 750.0,
            Self::CometOutburstSwarm => 350.0,
            Self::SolarSuperflare => 400.0,
        }
    }

    /// Typical duration of the event in simulation years.
    #[must_use]
    pub fn duration_years(self) -> f64 {
        match self {
            Self::RogueStarIncursion => 40.0,
            Self::GammaRayBurst => 1.5,
            Self::CometOutburstSwarm => 30.0,
            Self::SolarSuperflare => 2.0,
        }
    }
}

/// Lifecycle phase of an active cosmic hazard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CosmicEventPhase {
    #[default]
    Initializing,
    Active,
    Resolving,
    Completed,
}

/// A currently active cosmic hazard in the simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCosmicEvent {
    pub event_type: CosmicEventType,
    pub elapsed_years: f64,
    pub duration_years: f64,
    pub intensity: f32,
    pub phase: CosmicEventPhase,
    pub associated_entities: Vec<Entity>,
    pub notification_sent: bool,
}

impl ActiveCosmicEvent {
    #[must_use]
    pub fn new(event_type: CosmicEventType, intensity: f32) -> Self {
        Self {
            event_type,
            elapsed_years: 0.0,
            duration_years: event_type.duration_years(),
            intensity,
            phase: CosmicEventPhase::Initializing,
            associated_entities: Vec::new(),
            notification_sent: false,
        }
    }

    #[must_use]
    pub fn progress_fraction(&self) -> f32 {
        if self.duration_years <= 0.0 {
            1.0
        } else {
            (self.elapsed_years / self.duration_years).clamp(0.0, 1.0) as f32
        }
    }
}

/// Supported tonemapping display algorithms for camera post-processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TonemappingDisplayMode {
    #[default]
    AgX,
    ACES,
    Reinhard,
}

impl TonemappingDisplayMode {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::AgX => "AgX Filmic",
            Self::ACES => "ACES Filmic (Narkowicz)",
            Self::Reinhard => "Reinhard HDR",
        }
    }

    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            Self::AgX => Self::ACES,
            Self::ACES => Self::Reinhard,
            Self::Reinhard => Self::AgX,
        }
    }

    #[must_use]
    pub fn shader_mode_id(self) -> u32 {
        match self {
            Self::AgX => 0,
            Self::ACES => 1,
            Self::Reinhard => 2,
        }
    }
}

/// Global resource orchestrating stochastic cosmic hazards and sensory post-processing.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct CosmicEventManager {
    pub active_events: Vec<ActiveCosmicEvent>,
    pub auto_hazards_enabled: bool,
    pub hazard_timer_years: f64,
    pub hazard_interval_years: f64,
    pub hazard_flash_intensity: f32,
    pub tonemapping_mode: TonemappingDisplayMode,
    pub total_hazards_survived: u32,
}

impl Default for CosmicEventManager {
    fn default() -> Self {
        Self {
            active_events: Vec::new(),
            auto_hazards_enabled: true,
            hazard_timer_years: 0.0,
            hazard_interval_years: 250_000.0, // Every 250k sim years stochastic roll
            hazard_flash_intensity: 0.0,
            tonemapping_mode: TonemappingDisplayMode::default(),
            total_hazards_survived: 0,
        }
    }
}

impl CosmicEventManager {
    /// Queues a hazard event of the requested type.
    pub fn trigger_hazard(&mut self, event_type: CosmicEventType, intensity: f32) {
        if !self.has_active_hazard(event_type) {
            self.active_events
                .push(ActiveCosmicEvent::new(event_type, intensity));
        }
    }

    /// Checks if a hazard of this type is already ongoing.
    #[must_use]
    pub fn has_active_hazard(&self, event_type: CosmicEventType) -> bool {
        self.active_events
            .iter()
            .any(|e| e.event_type == event_type && e.phase != CosmicEventPhase::Completed)
    }
}

/// Message to manually or programmatically trigger a cosmic hazard.
#[derive(Event, Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerCosmicEvent(pub CosmicEventType);

/// Dispatched when a cosmic hazard commences.
#[derive(Event, Message, Debug, Clone)]
pub struct CosmicEventStartedEvent {
    pub event_type: CosmicEventType,
    pub description: String,
}

/// Dispatched when a cosmic hazard is survived and resolved.
#[derive(Event, Message, Debug, Clone)]
pub struct CosmicEventResolvedEvent {
    pub event_type: CosmicEventType,
    pub science_reward: f64,
}
