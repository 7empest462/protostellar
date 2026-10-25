//! Data structures and state representations for Campaign Missions and Objectives.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Identifiers for all supported Campaign Scenarios.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize,
)]
pub enum CampaignMissionId {
    /// Ignite a sub-stellar brown dwarf into a main-sequence star.
    TheStellarForge,
    /// Terraform a cryogenic, airless world into an Eden-class liquid biosphere.
    GenesisWorld,
    /// Execute orbital slingshots to fly by 4 distinct planets with 1 probe.
    TheGrandTour,
    /// Evacuate a populated star system before a 20 M☉ Red Supergiant collapses into a supernova.
    Exodus,
}

impl CampaignMissionId {
    pub fn title(&self) -> &'static str {
        match self {
            Self::TheStellarForge => "The Stellar Forge",
            Self::GenesisWorld => "Genesis World",
            Self::TheGrandTour => "The Grand Tour",
            Self::Exodus => "Exodus: Supernova",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Self::TheStellarForge => "Protostellar Ignition & Planetary Shepherding",
            Self::GenesisWorld => "Atmospheric Warming & Hydrosphere Condensation",
            Self::TheGrandTour => "Voyager-Class Multi-Flyby Slingshot Trajectory",
            Self::Exodus => "Core-Collapse Cataclysm & Deep-Space Evacuation",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::TheStellarForge => {
                "Accrete surrounding gas and massive planetesimals into the sub-stellar brown dwarf (0.050 M☉) until core hydrogen fusion ignites at M ≥ 0.080 M☉. Protect the 2 inner rocky protoplanets from destruction or ejection!"
            }
            Self::GenesisWorld => {
                "Guide the frozen, airless world 'Candidate Genesis' into an Eden-class biosphere. Warm the surface to 273–320 K, deliver volatile water ice until oceans cover ≥ 20%, and sustain the biosphere for at least 50 Myr."
            }
            Self::TheGrandTour => {
                "Pilot a single deep-space exploration probe across the system. Execute gravitational slingshot maneuvers to perform close-proximity flybys (< 0.50 AU) of at least 4 distinct major planets in one continuous voyage."
            }
            Self::Exodus => {
                "A 20 M☉ Red Supergiant will undergo iron core collapse within 50,000 years! Research propulsion, launch interstellar evacuation craft, and guide at least 2 colony vessels beyond 200 AU before the supernova blast arrives."
            }
        }
    }

    pub fn briefing(&self) -> &'static str {
        self.description()
    }

    pub fn difficulty(&self) -> &'static str {
        match self {
            Self::TheStellarForge => "★☆☆☆ MODERATE",
            Self::GenesisWorld => "★★☆☆ ADVANCED",
            Self::TheGrandTour => "★★★☆ EXPERT",
            Self::Exodus => "★★★★ NIGHTMARE",
        }
    }

    pub fn reward_science(&self) -> f64 {
        match self {
            Self::TheStellarForge => 750.0,
            Self::GenesisWorld => 1000.0,
            Self::TheGrandTour => 1250.0,
            Self::Exodus => 2000.0,
        }
    }

    pub fn science_reward(&self) -> u32 {
        self.reward_science() as u32
    }
}

/// A specific numeric or boolean objective within an active campaign mission.
#[derive(Debug, Clone, Reflect, Serialize, Deserialize)]
pub struct MissionObjective {
    pub id: String,
    pub title: String,
    pub description: String,
    pub current_value: f64,
    pub target_value: f64,
    pub unit: String,
    pub is_completed: bool,
    pub is_failed: bool,
    pub is_mandatory: bool,
}

impl MissionObjective {
    pub fn new(
        id: &str,
        title: &str,
        description: &str,
        target_value: f64,
        unit: &str,
        is_mandatory: bool,
    ) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            current_value: 0.0,
            target_value,
            unit: unit.to_string(),
            is_completed: false,
            is_failed: false,
            is_mandatory,
        }
    }

    pub fn format_progress(&self) -> String {
        if self.target_value <= 1.0 && self.unit.is_empty() {
            if self.is_completed {
                "COMPLETED".to_string()
            } else if self.is_failed {
                "FAILED".to_string()
            } else {
                "INCOMPLETE".to_string()
            }
        } else if self.unit == "%" {
            format!(
                "{:.1}% / {:.1}%",
                self.current_value * 100.0,
                self.target_value * 100.0
            )
        } else if self.unit == "M☉" {
            format!("{:.3} / {:.3} M☉", self.current_value, self.target_value)
        } else if self.unit == "K" {
            format!("{:.0} / {:.0} K", self.current_value, self.target_value)
        } else if self.unit == "Myr" {
            format!(
                "{:.1} / {:.1} Myr",
                self.current_value / 1.0e6,
                self.target_value / 1.0e6
            )
        } else if self.unit == "AU" {
            format!("{:.1} / {:.1} AU", self.current_value, self.target_value)
        } else {
            format!(
                "{:.0} / {:.0} {}",
                self.current_value, self.target_value, self.unit
            )
        }
    }

    pub fn progress_fraction(&self) -> f64 {
        if self.target_value <= 0.0 {
            if self.is_completed {
                1.0
            } else {
                0.0
            }
        } else {
            (self.current_value / self.target_value).clamp(0.0, 1.0)
        }
    }
}

/// Lifecycle status of the current active campaign mission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default, Serialize, Deserialize)]
pub enum MissionStatus {
    #[default]
    NotStarted,
    InProgress,
    Victory,
    Defeat,
}

/// Global resource tracking campaign mission progression, objectives, and rewards.
#[derive(Resource, Debug, Clone, Default, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct CampaignState {
    pub active_mission: Option<CampaignMissionId>,
    pub status: MissionStatus,
    pub elapsed_mission_years: f64,
    pub objectives: Vec<MissionObjective>,
    pub completed_missions: BTreeSet<CampaignMissionId>,
    pub reward_claimed: bool,
    pub failure_reason: Option<String>,
    #[reflect(ignore)]
    pub flyby_visited_entities: BTreeSet<Entity>,
    pub stable_biosphere_years: f64,
    pub evacuated_probe_count: usize,
}

impl CampaignState {
    pub fn is_mission_active(&self) -> bool {
        self.active_mission.is_some() && self.status == MissionStatus::InProgress
    }

    pub fn is_victory(&self) -> bool {
        self.status == MissionStatus::Victory
    }

    pub fn is_defeat(&self) -> bool {
        self.status == MissionStatus::Defeat
    }
}

/// Event fired when a campaign mission is launched.
#[derive(Message, Debug, Clone, Copy)]
pub struct StartCampaignMissionEvent(pub CampaignMissionId);

/// Event fired when a campaign mission is successfully won.
#[derive(Message, Debug, Clone)]
pub struct CampaignMissionCompletedEvent {
    pub mission: CampaignMissionId,
    pub science_reward: f64,
}

/// Event fired when a campaign mission fails.
#[derive(Message, Debug, Clone)]
pub struct CampaignMissionFailedEvent {
    pub mission: CampaignMissionId,
    pub reason: String,
}
