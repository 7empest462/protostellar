//! Data structures, event definitions, and configuration for Dynamic Cosmic Hazards.

use crate::simulation::economy::TechnologyId;
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
    /// Swarm of massive planet-killer asteroids on direct collision trajectories requiring deflection.
    PlanetKillerSwarm,
    /// Industrial runaway climate instability & ecological fragility confronting rising civilizations.
    GreatFilterCrisis,
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
            Self::PlanetKillerSwarm => "Planet-Killer Asteroid Swarm",
            Self::GreatFilterCrisis => "Planetary Great Filter Crisis",
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
            Self::PlanetKillerSwarm => {
                "Hypervelocity Chicxulub-class asteroids plunge toward an inhabited or inner rocky world. Deflect them or deploy orbital interceptors!"
            }
            Self::GreatFilterCrisis => {
                "Rapid industrialization destabilizes global climate and dust albedo. Deploy Climate Stabilizers or Orbital Mirrors to survive the bottleneck."
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
            Self::PlanetKillerSwarm => 650.0,
            Self::GreatFilterCrisis => 600.0,
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
            Self::PlanetKillerSwarm => 25.0,
            Self::GreatFilterCrisis => 18.0,
        }
    }
}

/// Resolution rating of a cosmic crisis determining bounty multipliers and trophies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CrisisOutcome {
    Averted,
    Mitigated,
    #[default]
    Survived,
    Catastrophic,
}

impl CrisisOutcome {
    /// Science Data bounty multiplier associated with this outcome.
    #[must_use]
    pub const fn bounty_multiplier(self) -> f64 {
        match self {
            Self::Averted => 2.0,
            Self::Mitigated => 1.35,
            Self::Survived => 1.0,
            Self::Catastrophic => 0.35,
        }
    }

    /// Human-readable status badge.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Averted => "AVERTED",
            Self::Mitigated => "MITIGATED",
            Self::Survived => "SURVIVED",
            Self::Catastrophic => "CATASTROPHIC",
        }
    }
}

/// Active player-triggered defense action against ongoing cosmic crises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrisisCountermeasureKind {
    LaunchKineticDeflector,
    ActivateShieldOverdrive,
    DeployClimateStabilizer,
}

impl CrisisCountermeasureKind {
    /// Selects the highest-impact countermeasure for a given crisis class.
    #[must_use]
    pub const fn recommended_for(event_type: CosmicEventType) -> Self {
        match event_type {
            CosmicEventType::PlanetKillerSwarm
            | CosmicEventType::CometOutburstSwarm
            | CosmicEventType::RogueStarIncursion => Self::LaunchKineticDeflector,
            CosmicEventType::GammaRayBurst | CosmicEventType::SolarSuperflare => {
                Self::ActivateShieldOverdrive
            }
            CosmicEventType::GreatFilterCrisis => Self::DeployClimateStabilizer,
        }
    }

    /// Short display label for HUD buttons.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LaunchKineticDeflector => "🚀 Kinetic Deflector",
            Self::ActivateShieldOverdrive => "🛡️ Shield Overdrive",
            Self::DeployClimateStabilizer => "❄️ Climate Stabilizer",
        }
    }

    /// Technology node required in `GameMode::Progression` (`Sandbox` ignores this).
    #[must_use]
    pub const fn required_tech(self) -> Option<TechnologyId> {
        match self {
            Self::LaunchKineticDeflector => Some(TechnologyId::PlanetaryDefenseGrid),
            Self::ActivateShieldOverdrive => Some(TechnologyId::MagnetosphericShielding),
            Self::DeployClimateStabilizer => Some(TechnologyId::OrbitalMirrorArray),
        }
    }

    /// Science Data cost deducted in `GameMode::Progression` (`Sandbox` costs 0).
    #[must_use]
    pub const fn science_cost(self) -> f64 {
        match self {
            Self::LaunchKineticDeflector => 60.0,
            Self::ActivateShieldOverdrive | Self::DeployClimateStabilizer => 75.0,
        }
    }
}

/// Lifecycle phase of an active cosmic hazard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CosmicEventPhase {
    Warning,
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
    #[serde(default)]
    pub warning_remaining_years: f64,
    #[serde(skip)]
    pub target_entity: Option<Entity>,
    #[serde(default)]
    pub target_name: String,
    #[serde(default)]
    pub threats_total: u32,
    #[serde(default)]
    pub threats_neutralized: u32,
    #[serde(default)]
    pub impacts_suffered: u32,
    #[serde(default)]
    pub mitigation_score: f32,
    #[serde(default)]
    pub countermeasure_uses: u32,
    #[serde(default)]
    pub outcome: Option<CrisisOutcome>,
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
            warning_remaining_years: 0.0,
            target_entity: None,
            target_name: String::new(),
            threats_total: 0,
            threats_neutralized: 0,
            impacts_suffered: 0,
            mitigation_score: 0.0,
            countermeasure_uses: 0,
            outcome: None,
        }
    }

    #[must_use]
    pub fn new_with_warning(
        event_type: CosmicEventType,
        intensity: f32,
        warning_years: f64,
    ) -> Self {
        let mut ev = Self::new(event_type, intensity);
        if warning_years > 0.0 {
            ev.phase = CosmicEventPhase::Warning;
            ev.warning_remaining_years = warning_years;
        }
        ev
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

    /// Queues a hazard event with an early-warning lead phase before onset.
    pub fn trigger_hazard_with_warning(
        &mut self,
        event_type: CosmicEventType,
        intensity: f32,
        warning_years: f64,
    ) {
        if !self.has_active_hazard(event_type) {
            self.active_events.push(ActiveCosmicEvent::new_with_warning(
                event_type,
                intensity,
                warning_years,
            ));
        }
    }

    /// Checks if a hazard of this type is already ongoing.
    #[must_use]
    pub fn has_active_hazard(&self, event_type: CosmicEventType) -> bool {
        self.active_events
            .iter()
            .any(|e| e.event_type == event_type && e.phase != CosmicEventPhase::Completed)
    }

    /// Returns the primary ongoing crisis event for HUD banners and defense focus.
    #[must_use]
    pub fn primary_active_event(&self) -> Option<&ActiveCosmicEvent> {
        self.active_events
            .iter()
            .find(|e| e.phase != CosmicEventPhase::Completed)
    }
}

/// Message to manually or programmatically trigger a cosmic hazard.
#[derive(Event, Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerCosmicEvent(pub CosmicEventType);

/// Message requesting an active planetary defense countermeasure against an ongoing crisis.
#[derive(Event, Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrisisCountermeasureEvent {
    pub kind: CrisisCountermeasureKind,
    pub preferred_target: Option<Entity>,
}

/// Dispatched when a cosmic hazard commences.
#[derive(Event, Message, Debug, Clone)]
pub struct CosmicEventStartedEvent {
    pub event_type: CosmicEventType,
    pub description: String,
    pub target_entity: Option<Entity>,
}

/// Dispatched when a cosmic hazard is survived and resolved.
#[derive(Event, Message, Debug, Clone)]
pub struct CosmicEventResolvedEvent {
    pub event_type: CosmicEventType,
    pub science_reward: f64,
    pub outcome: CrisisOutcome,
    pub target_entity: Option<Entity>,
}
