//! Civilization tiers, Kardashev scaling, planetary defense readiness, and lifecycle events.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Kardashev evolutionary epoch derived from a world's continuous `kardashev_level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum KardashevEra {
    PreIndustrial,
    Industrial,
    InformationAge,
    PlanetaryTypeI,
    Interplanetary,
    DysonProtoSwarm,
    StellarTypeII,
    StellarEngineAscendant,
}

impl KardashevEra {
    /// Classifies a continuous Kardashev index into its discrete era.
    #[must_use]
    pub fn from_kardashev(k: f32) -> Self {
        if k >= 2.10 {
            Self::StellarEngineAscendant
        } else if k >= 1.95 {
            Self::StellarTypeII
        } else if k >= 1.55 {
            Self::DysonProtoSwarm
        } else if k >= 1.25 {
            Self::Interplanetary
        } else if k >= 1.00 {
            Self::PlanetaryTypeI
        } else if k >= 0.78 {
            Self::InformationAge
        } else if k >= 0.55 {
            Self::Industrial
        } else {
            Self::PreIndustrial
        }
    }

    /// Full human-readable era name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PreIndustrial => "Pre-Industrial agrarian biosphere",
            Self::Industrial => "Industrial fossil & fission era",
            Self::InformationAge => "Information & orbital satellite age",
            Self::PlanetaryTypeI => "Type I Planetary Federation",
            Self::Interplanetary => "Interplanetary colonization network",
            Self::DysonProtoSwarm => "Proto-Dyson megastructure builders",
            Self::StellarTypeII => "Type II Stellar Dyson Civilization",
            Self::StellarEngineAscendant => "Shkadov Stellar-Engine Ascendancy",
        }
    }

    /// Concise era badge for HUD and event feeds.
    #[must_use]
    pub const fn short_badge(self) -> &'static str {
        match self {
            Self::PreIndustrial => "Pre-Industrial",
            Self::Industrial => "Industrial",
            Self::InformationAge => "Info Age",
            Self::PlanetaryTypeI => "Type I Planetary",
            Self::Interplanetary => "Interplanetary",
            Self::DysonProtoSwarm => "Dyson Builders",
            Self::StellarTypeII => "Type II Stellar",
            Self::StellarEngineAscendant => "Stellar Engine II+",
        }
    }

    /// Unique bit index (`0..=7`) used to emit one-time advancement events per era.
    #[must_use]
    pub const fn milestone_bit(self) -> u8 {
        match self {
            Self::PreIndustrial => 0,
            Self::Industrial => 1,
            Self::InformationAge => 2,
            Self::PlanetaryTypeI => 3,
            Self::Interplanetary => 4,
            Self::DysonProtoSwarm => 5,
            Self::StellarTypeII => 6,
            Self::StellarEngineAscendant => 7,
        }
    }

    /// Target Kardashev rating reached when explicitly uplifting from this era.
    #[must_use]
    pub const fn next_uplift_kardashev(self) -> f32 {
        match self {
            Self::PreIndustrial => 0.62,
            Self::Industrial => 0.84,
            Self::InformationAge => 1.05,
            Self::PlanetaryTypeI => 1.32,
            Self::Interplanetary => 1.62,
            Self::DysonProtoSwarm => 2.00,
            Self::StellarTypeII | Self::StellarEngineAscendant => 2.15,
        }
    }
}

/// Converts a continuous Kardashev level (`~0.35..=2.20`) to shader `technosignature` (`0.05..=2.0`).
#[must_use]
pub fn technosignature_from_kardashev(k: f32) -> f32 {
    ((k - 0.35) / 1.65 * 2.0).clamp(0.05, 2.0)
}

/// Converts legacy shader `technosignature` (`0.0..=2.0`) to a corresponding Kardashev index.
#[must_use]
pub fn kardashev_from_technosignature(t: f32) -> f32 {
    (0.35 + (t.clamp(0.0, 2.0) / 2.0) * 1.65).clamp(0.40, 2.20)
}

/// Living planetary civilization component tracking Kardashev scale, population, stability,
/// Great Filter stress, autonomous colonies, and active planetary defenses.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CivilizationState {
    pub kardashev_level: f32,
    pub population_billions: f64,
    pub stability: f32,
    pub defense_readiness: f32,
    pub orbital_infrastructure: f32,
    pub shield_overdrive_secs: f32,
    pub climate_stabilizer_secs: f32,
    pub great_filter_stress: f32,
    pub crises_survived: u32,
    pub colonies_founded: u32,
    pub colony_cooldown_years: f64,
    pub is_colony: bool,
    pub era_milestone_mask: u8,
    /// True when established on an arid, airless, or frozen world (Mars, Moon, icy moons)
    /// as pressurized domed hubs rather than an open terrestrial biosphere.
    #[serde(default)]
    pub is_domed_colony: bool,
    /// Explicit user toggle forcing the Orbital Habitat Ring visual on this world.
    #[serde(default)]
    pub force_orbital_ring: bool,
    /// Explicit user toggle forcing the Orbital Solar Mirror array on this world.
    #[serde(default)]
    pub force_solar_mirrors: bool,
    /// Explicit user toggle forcing the Planetary Shield visual on this world.
    #[serde(default)]
    pub force_planetary_shield: bool,
}

impl Default for CivilizationState {
    fn default() -> Self {
        Self {
            kardashev_level: 0.72,
            population_billions: 4.5,
            stability: 0.85,
            defense_readiness: 0.35,
            orbital_infrastructure: 0.25,
            shield_overdrive_secs: 0.0,
            climate_stabilizer_secs: 0.0,
            great_filter_stress: 0.10,
            crises_survived: 0,
            colonies_founded: 0,
            colony_cooldown_years: 250.0,
            is_colony: false,
            era_milestone_mask: 0b0000_0011,
            is_domed_colony: false,
            force_orbital_ring: false,
            force_solar_mirrors: false,
            force_planetary_shield: false,
        }
    }
}

impl CivilizationState {
    /// Initializes a matching [`CivilizationState`] from an existing `BiosphereState::technosignature`.
    #[must_use]
    pub fn from_technosignature(technosignature: f32) -> Self {
        let k = kardashev_from_technosignature(technosignature);
        let era = KardashevEra::from_kardashev(k);
        let infra = ((k - 0.65) / 1.35).clamp(0.05, 1.0);
        let pop = (0.5 * 2.4_f64.powf(f64::from((k - 0.40).max(0.0)) * 3.0)).clamp(0.05, 450.0);
        let mut mask = 0u8;
        for bit in 0..=era.milestone_bit() {
            mask |= 1u8 << bit;
        }
        Self {
            kardashev_level: k,
            population_billions: pop,
            stability: 0.86,
            defense_readiness: (0.15 + infra * 0.65).clamp(0.10, 1.0),
            orbital_infrastructure: infra,
            shield_overdrive_secs: 0.0,
            climate_stabilizer_secs: 0.0,
            great_filter_stress: if (0.68..=0.98).contains(&k) {
                0.24
            } else {
                0.08
            },
            crises_survived: 0,
            colonies_founded: 0,
            colony_cooldown_years: 300.0,
            is_colony: false,
            era_milestone_mask: mask,
            is_domed_colony: false,
            force_orbital_ring: false,
            force_solar_mirrors: false,
            force_planetary_shield: false,
        }
    }

    /// Bootstraps a civilization and matching [`crate::simulation::components::BiosphereState`]
    /// for **any** solid planet or moon:
    /// - Habitable worlds (Earth, terraformed worlds) receive an open-biosphere civilization.
    /// - Dry, airless, or frozen worlds (Mars, Moon, Europa, exoplanets) receive a pressurized
    ///   domed colony without forcibly overwriting their terrain into an ocean world.
    #[must_use]
    pub fn bootstrap_for_world(
        existing_bio: Option<&crate::simulation::components::BiosphereState>,
        surface_temp_k: f64,
        has_liquid_water: bool,
        sim_time_years: f64,
    ) -> (Self, crate::simulation::components::BiosphereState) {
        let is_open_habitable = existing_bio
            .is_some_and(|b| b.biomass_coverage_frac > 0.05 || b.habitability_score > 0.35)
            || (has_liquid_water && (220.0..=360.0).contains(&surface_temp_k));

        let mut civ = Self::from_technosignature(0.45);
        civ.advance_to_next_tier();
        let tech_sig = technosignature_from_kardashev(civ.kardashev_level);

        if is_open_habitable {
            let mut bio = existing_bio.copied().unwrap_or_default();
            bio.habitability_score = bio.habitability_score.max(0.85);
            bio.biomass_coverage_frac = bio.biomass_coverage_frac.max(0.65);
            bio.oxygen_fraction = bio.oxygen_fraction.max(0.21);
            bio.bioluminescence_intensity = bio.bioluminescence_intensity.max(0.28);
            bio.emergence_year = bio.emergence_year.or(Some(sim_time_years));
            bio.technosignature = tech_sig;
            (civ, bio)
        } else {
            civ.is_colony = true;
            civ.is_domed_colony = true;
            civ.population_billions = 0.25;
            let mut bio = existing_bio.copied().unwrap_or_default();
            bio.emergence_year = bio.emergence_year.or(Some(sim_time_years));
            bio.technosignature = tech_sig;
            (civ, bio)
        }
    }

    /// Current discrete Kardashev era.
    #[must_use]
    pub fn era(&self) -> KardashevEra {
        KardashevEra::from_kardashev(self.kardashev_level)
    }

    /// Uplifts this civilization to the next Kardashev tier and refreshes stability and infrastructure.
    pub fn advance_to_next_tier(&mut self) -> KardashevEra {
        let current = self.era();
        let next_k = current
            .next_uplift_kardashev()
            .max(self.kardashev_level + 0.14)
            .min(2.20);
        self.kardashev_level = next_k;
        self.population_billions = (self.population_billions * 1.45 + 1.5).clamp(0.1, 800.0);
        self.stability = (self.stability + 0.15).clamp(0.65, 1.0);
        self.orbital_infrastructure = ((next_k - 0.65) / 1.35).clamp(0.20, 1.0);
        self.defense_readiness = (self.defense_readiness + 0.20).clamp(0.30, 1.0);
        self.great_filter_stress = (self.great_filter_stress * 0.5).max(0.0);
        let new_era = self.era();
        self.era_milestone_mask |= 1u8 << new_era.milestone_bit();
        new_era
    }

    /// Formats the Target Inspector HUD telemetry block for inhabited civilizations.
    #[must_use]
    pub fn format_telemetry(&self) -> String {
        let mut out = String::from("\n━━━ ⟪ CIVILIZATION & DEFENSE ⟫ ━━━");
        let role = if self.is_domed_colony {
            "Pressurized Domed Colony"
        } else if self.is_colony {
            "Off-World Colony"
        } else {
            "Homeworld Civilization"
        };
        let pop_str = if self.population_billions >= 1.0 {
            format!("{:.2}B", self.population_billions)
        } else {
            format!("{:.0}M", self.population_billions * 1000.0)
        };
        let _ = write!(
            out,
            "\n▸ Kardashev: K{:.2} ({} | {})\n▸ Population: {} | Stability: {:.0}% | Colonies: {}\n▸ Defense Grid: {:.0}% | Orbital Infra: {:.0}% | Crises Survived: {}",
            self.kardashev_level,
            self.era().short_badge(),
            role,
            pop_str,
            self.stability * 100.0,
            self.colonies_founded,
            self.defense_readiness * 100.0,
            self.orbital_infrastructure * 100.0,
            self.crises_survived,
        );
        if self.shield_overdrive_secs > 0.1 || self.climate_stabilizer_secs > 0.1 {
            let _ = write!(
                out,
                "\n▸ Active Defenses: Shield Overdrive {:.0}s | Climate Stabilizer {:.0}s",
                self.shield_overdrive_secs, self.climate_stabilizer_secs
            );
        }
        if self.great_filter_stress > 0.15 {
            let _ = write!(
                out,
                "\n▸ Great Filter Risk: {:.0}% (Deploy Climate Stabilizer or Uplift)",
                self.great_filter_stress * 100.0
            );
        }
        out
    }
}

/// Emitted when a civilization crosses a new Kardashev era or collapses under extreme conditions.
#[derive(Event, Message, Debug, Clone)]
pub struct CivilizationMilestoneEvent {
    pub world_entity: Entity,
    pub world_name: String,
    pub era: KardashevEra,
    pub kardashev_level: f32,
    pub collapsed: bool,
}

/// Emitted when an interplanetary civilization seeds a new off-world colony.
#[derive(Event, Message, Debug, Clone)]
pub struct ColonyFoundedEvent {
    pub origin_entity: Entity,
    pub colony_entity: Entity,
    pub origin_name: String,
    pub colony_name: String,
}
