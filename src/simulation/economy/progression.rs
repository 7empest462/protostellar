//! Sandbox vs. Progression game mode, system mastery ranks, and tech-gated action access.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::types::{ALL_TECH_NODES, ScienceData, TechnologyId, UnlockedTechs};

/// Controls whether engineering/crisis actions are unrestricted (Sandbox) or require
/// unlocked technologies and Science Data investment (Progression).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GameMode {
    #[default]
    Sandbox,
    Progression,
}

impl GameMode {
    /// Human-readable label for HUD pills and toasts.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sandbox => "Sandbox (Unrestricted)",
            Self::Progression => "Progression (Tech-Gated)",
        }
    }

    /// Short compact pill text.
    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Sandbox => "🛠️ Sandbox",
            Self::Progression => "🎖️ Progression",
        }
    }

    /// Alternates between Sandbox and Progression modes.
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Sandbox => Self::Progression,
            Self::Progression => Self::Sandbox,
        }
    }
}

/// System-wide progression rank reflecting research mastery, civilizations, and crisis defense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum ProgressionRank {
    #[default]
    PrimordialObserver,
    DeepSpaceExplorer,
    PlanetarySteward,
    CrisisDefender,
    KardashevArchitect,
    GalacticCustodian,
}

impl ProgressionRank {
    /// Display title for the mastery badge.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::PrimordialObserver => "I. Primordial Observer",
            Self::DeepSpaceExplorer => "II. Deep-Space Explorer",
            Self::PlanetarySteward => "III. Planetary Steward",
            Self::CrisisDefender => "IV. Crisis Defender",
            Self::KardashevArchitect => "V. Kardashev Architect",
            Self::GalacticCustodian => "VI. Galactic Custodian",
        }
    }

    /// Numeric tier index in `0..=5`.
    #[must_use]
    pub const fn tier_index(self) -> u8 {
        match self {
            Self::PrimordialObserver => 0,
            Self::DeepSpaceExplorer => 1,
            Self::PlanetarySteward => 2,
            Self::CrisisDefender => 3,
            Self::KardashevArchitect => 4,
            Self::GalacticCustodian => 5,
        }
    }

    /// Concise guidance toward the next mastery rank.
    #[must_use]
    pub const fn next_goal(self) -> &'static str {
        match self {
            Self::PrimordialObserver => "Unlock 1 tech or launch a deep-space probe",
            Self::DeepSpaceExplorer => "Nurture a civilization or research 3 techs",
            Self::PlanetarySteward => "Avert a cosmic crisis or research 5 techs",
            Self::CrisisDefender => "Reach Kardashev K1.50 or unlock 7 techs",
            Self::KardashevArchitect => "Reach Type II (K2.0), found a colony, avert 2 crises",
            Self::GalacticCustodian => "Maximum System Mastery Achieved",
        }
    }
}

/// Outcome of validating whether an action can be executed under the active [`GameMode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionAccessResult {
    Allowed,
    Locked(String),
}

impl ActionAccessResult {
    /// Returns `true` when the action is permitted.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// Returns the lock reason if blocked.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Allowed => None,
            Self::Locked(msg) => Some(msg.as_str()),
        }
    }
}

/// Global resource tracking active game mode, crisis defense record, and civilization advancement.
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgressionState {
    pub mode: GameMode,
    pub crises_averted: u32,
    pub crises_mitigated: u32,
    pub crises_catastrophic: u32,
    pub highest_kardashev: f32,
    pub active_civilization_count: u32,
    pub colony_world_count: u32,
    pub stellar_engine_active: bool,
}

impl ProgressionState {
    /// Evaluates the player's current system mastery rank.
    #[must_use]
    pub fn compute_rank(
        &self,
        unlocked_count: usize,
        completed_missions: usize,
    ) -> ProgressionRank {
        if unlocked_count >= 10
            && self.highest_kardashev >= 2.0
            && self.crises_averted >= 2
            && (self.colony_world_count >= 1 || completed_missions >= 2)
        {
            ProgressionRank::GalacticCustodian
        } else if self.highest_kardashev >= 1.5 || unlocked_count >= 7 || self.stellar_engine_active
        {
            ProgressionRank::KardashevArchitect
        } else if self.crises_averted >= 1 || self.crises_mitigated >= 2 || unlocked_count >= 5 {
            ProgressionRank::CrisisDefender
        } else if self.active_civilization_count >= 1
            || self.highest_kardashev >= 0.65
            || unlocked_count >= 3
            || completed_missions >= 1
        {
            ProgressionRank::PlanetarySteward
        } else if unlocked_count >= 1 || completed_missions >= 1 || self.crises_mitigated >= 1 {
            ProgressionRank::DeepSpaceExplorer
        } else {
            ProgressionRank::PrimordialObserver
        }
    }

    /// Validates whether an action gated by `req_tech` and `science_cost` is available.
    #[must_use]
    pub fn check_action_access(
        &self,
        req_tech: Option<TechnologyId>,
        science_cost: f64,
        techs: Option<&UnlockedTechs>,
        science: Option<&ScienceData>,
    ) -> ActionAccessResult {
        if self.mode == GameMode::Sandbox {
            return ActionAccessResult::Allowed;
        }
        if let Some(req) = req_tech
            && !techs.is_some_and(|t| t.has(req))
        {
            let tech_name = ALL_TECH_NODES
                .iter()
                .find(|n| n.id == req)
                .map_or("Required Tech", |n| n.name);
            return ActionAccessResult::Locked(format!(
                "🔒 Progression Mode: Research '{tech_name}' first [Shift-T]."
            ));
        }
        if science_cost > 0.0 {
            let available = science.map_or(0.0, |s| s.points);
            if available + 1e-6 < science_cost {
                return ActionAccessResult::Locked(format!(
                    "🔒 Progression Mode: Need {science_cost:.0} Science Data (have {available:.0})."
                ));
            }
        }
        ActionAccessResult::Allowed
    }

    /// Deducts `science_cost` only when running in [`GameMode::Progression`].
    pub fn consume_progression_cost(
        &self,
        science_cost: f64,
        science: Option<&mut ScienceData>,
    ) -> bool {
        if self.mode == GameMode::Sandbox || science_cost <= 0.0 {
            return true;
        }
        science.is_some_and(|s| s.spend_science(science_cost))
    }

    /// Clears scenario-specific counters while preserving the user's selected [`GameMode`].
    pub fn reset_session_counters(&mut self) {
        let preserved_mode = self.mode;
        *self = Self::default();
        self.mode = preserved_mode;
    }
}
