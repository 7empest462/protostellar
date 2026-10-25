//! Data structures and resources for the Science Economy and Technology Tree.

use bevy::prelude::*;
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};

/// A historic scientific breakthrough or cosmic milestone recorded by the player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScienceDiscovery {
    pub title: String,
    pub description: String,
    pub bonus_points: f64,
    pub timestamp_years: f64,
}

/// Global resource tracking spendable Science Data points and telemetry harvesting metrics.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct ScienceData {
    /// Spendable research points
    pub points: f64,
    /// Lifetime research points generated across the entire simulation
    pub lifetime_points: f64,
    /// Real-time rate of telemetry collection in science points per visual second
    pub passive_rate_per_sec: f64,
    /// Chronological ledger of scientific breakthroughs and discoveries
    pub discoveries: Vec<ScienceDiscovery>,
}

impl Default for ScienceData {
    fn default() -> Self {
        Self {
            points: 100.0, // Starting grant to allow early experimentation
            lifetime_points: 100.0,
            passive_rate_per_sec: 0.0,
            discoveries: Vec::new(),
        }
    }
}

impl ScienceData {
    /// Grants science points with lifetime accumulation.
    pub fn add_science(&mut self, amount: f64) {
        if amount > 0.0 {
            self.points += amount;
            self.lifetime_points += amount;
        }
    }

    /// Attempts to deduct points for a research purchase. Returns true if successful.
    pub fn spend_science(&mut self, cost: f64) -> bool {
        if self.points >= cost {
            self.points -= cost;
            true
        } else {
            false
        }
    }

    /// Records a major discovery milestone, awarding bonus science points.
    pub fn record_discovery(
        &mut self,
        title: impl Into<String>,
        description: impl Into<String>,
        bonus_points: f64,
        timestamp_years: f64,
    ) {
        self.add_science(bonus_points);
        self.discoveries.push(ScienceDiscovery {
            title: title.into(),
            description: description.into(),
            bonus_points,
            timestamp_years,
        });
    }
}

/// Broad technological branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TechnologyCategory {
    Propulsion,
    Terraforming,
    Megastructures,
}

/// Unique identifiers for all researchable technologies in Protostellar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TechnologyId {
    // Propulsion branch
    IonDrive,
    SolarSail,
    AntimatterDrive,

    // Terraforming branch
    AtmosphericInjection,
    OrbitalMirrorArray,
    CometRedirection,

    // Megastructures & Astrophysics branch
    GravitationalLensArray,
    DysonCollectors,
    StellarEngine,
}

/// Static definition of a researchable technology node in the Tech Tree.
#[derive(Debug, Clone)]
pub struct TechNode {
    pub id: TechnologyId,
    pub name: &'static str,
    pub category: TechnologyCategory,
    pub description: &'static str,
    pub cost: f64,
    pub prerequisites: &'static [TechnologyId],
}

/// Global resource tracking all unlocked technologies and capabilities.
#[derive(Resource, Debug, Clone, Default)]
pub struct UnlockedTechs {
    pub unlocked: HashSet<TechnologyId>,
}

impl UnlockedTechs {
    /// Checks if a technology has been researched.
    #[must_use]
    pub fn has(&self, id: TechnologyId) -> bool {
        self.unlocked.contains(&id)
    }

    /// Returns all static tech tree nodes.
    pub fn all_nodes() -> &'static [TechNode] {
        &ALL_TECH_NODES
    }

    /// Checks if all prerequisites for a technology have been satisfied.
    #[must_use]
    pub fn prerequisites_met(&self, id: TechnologyId) -> bool {
        if let Some(node) = Self::all_nodes().iter().find(|n| n.id == id) {
            node.prerequisites.iter().all(|req| self.has(*req))
        } else {
            false
        }
    }

    /// Checks if a technology can currently be unlocked (prerequisites met + sufficient points).
    #[must_use]
    pub fn can_unlock(&self, id: TechnologyId, science: &ScienceData) -> bool {
        if self.has(id) || !self.prerequisites_met(id) {
            return false;
        }
        if let Some(node) = Self::all_nodes().iter().find(|n| n.id == id) {
            science.points >= node.cost
        } else {
            false
        }
    }

    /// Unlocks a technology, deducting science points. Returns true if successfully researched.
    pub fn unlock(&mut self, id: TechnologyId, science: &mut ScienceData) -> bool {
        if !self.can_unlock(id, science) {
            return false;
        }
        if let Some(node) = Self::all_nodes().iter().find(|n| n.id == id) {
            if science.spend_science(node.cost) {
                self.unlocked.insert(id);
                return true;
            }
        }
        false
    }
}

pub static ALL_TECH_NODES: [TechNode; 9] = [
    // --- Propulsion ---
    TechNode {
        id: TechnologyId::IonDrive,
        name: "⚡ Hall-Effect Ion Thrusters",
        category: TechnologyCategory::Propulsion,
        description: "Electrostatic xenon propulsion. Triples probe transit cruise speeds and improves insertion accuracy.",
        cost: 150.0,
        prerequisites: &[],
    },
    TechNode {
        id: TechnologyId::SolarSail,
        name: "⛵ Photonic Solar Sails",
        category: TechnologyCategory::Propulsion,
        description: "Ultralight reflective metamaterials harnessing stellar radiation pressure for propellantless cruising.",
        cost: 350.0,
        prerequisites: &[TechnologyId::IonDrive],
    },
    TechNode {
        id: TechnologyId::AntimatterDrive,
        name: "⚛️ Positron-Catalyzed Antimatter Drive",
        category: TechnologyCategory::Propulsion,
        description: "Relativistic propulsion reaching 0.15c. Enables near-instant interplanetary transits across solar distances.",
        cost: 1000.0,
        prerequisites: &[TechnologyId::SolarSail],
    },

    // --- Terraforming ---
    TechNode {
        id: TechnologyId::AtmosphericInjection,
        name: "🏭 Atmospheric Aerosol Injection",
        category: TechnologyCategory::Terraforming,
        description: "Industrial injection of greenhouse gases and reflective aerosols to regulate planetary surface temperature.",
        cost: 200.0,
        prerequisites: &[],
    },
    TechNode {
        id: TechnologyId::OrbitalMirrorArray,
        name: "🪞 Orbital Solar Reflector Array",
        category: TechnologyCategory::Terraforming,
        description: "Orbital mirror megastructures focusing stellar flux to thaw frozen ocean worlds and establish liquid water.",
        cost: 500.0,
        prerequisites: &[TechnologyId::AtmosphericInjection],
    },
    TechNode {
        id: TechnologyId::CometRedirection,
        name: "☄️ Kinetic Cometary Redirection",
        category: TechnologyCategory::Terraforming,
        description: "Propulsion packages bolted to Oort cloud comets to deliver billions of tons of volatile water ice directly to arid planets.",
        cost: 850.0,
        prerequisites: &[TechnologyId::OrbitalMirrorArray],
    },

    // --- Megastructures & Astrophysics ---
    TechNode {
        id: TechnologyId::GravitationalLensArray,
        name: "🔭 Solar Gravitational Lensing Array",
        category: TechnologyCategory::Megastructures,
        description: "Focal-point deep-space telescopes resolving planetary biosignatures and exoplanet topography at light-year distances.",
        cost: 250.0,
        prerequisites: &[],
    },
    TechNode {
        id: TechnologyId::DysonCollectors,
        name: "☀️ Dyson Swarm Flux Concentrators",
        category: TechnologyCategory::Megastructures,
        description: "Advanced photovoltaic mirrors multiplying Dyson Swarm construction rate by 5x and capturing terawatts of research telemetry.",
        cost: 650.0,
        prerequisites: &[TechnologyId::GravitationalLensArray],
    },
    TechNode {
        id: TechnologyId::StellarEngine,
        name: "🌟 Shkadov Stellar Engine",
        category: TechnologyCategory::Megastructures,
        description: "Colossal hemispherical thrust mirror using asymmetrical stellar radiation pressure to accelerate the host star through the galaxy.",
        cost: 1500.0,
        prerequisites: &[TechnologyId::DysonCollectors],
    },
];
