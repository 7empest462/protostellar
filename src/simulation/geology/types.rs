//! Data types, components, and resources for Deep Geological Time,
//! Epoch Scrubbing, Continental Drift, and Planetary Oxidation.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Major planetary geological epochs spanning the 4.56-billion-year history
/// of Earth-like terrestrial worlds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum GeologicalEpoch {
    /// 4.56 Ga - 4.0 Ga: Magma ocean cooling, heavy impact bombardment (LHB),
    /// dense steam greenhouse atmosphere, and primordial ocean condensation.
    Hadean,
    /// 4.0 Ga - 2.5 Ga: Micro-continental cratons (Vaalbara, Ur), anoxic green
    /// iron-rich oceans, orange methane skies, and early anaerobic stromatolites.
    Archean,
    /// 2.5 Ga - 0.54 Ga: Great Oxidation Event (GOE), banded iron rust formation,
    /// supercontinents Columbia and Rodinia, and Cryogenian Snowball Earth glaciations.
    Proterozoic,
    /// 540 Ma - 10 Ma: Cambrian explosion, colonization of land by plants,
    /// supercontinent Pangea assembly and breakup, and complex macroscopic life.
    Phanerozoic,
    /// 10 Ma - Present: Modern dispersed continents, deep sapphire oceans,
    /// vibrant global biospheres, and polar ice caps.
    #[default]
    Modern,
    /// +100 Ma - +1.5 Ga: Continental re-assembly (Pangea Ultima / Novopangea),
    /// solar brightening, and moist greenhouse transition.
    Future,
}

impl GeologicalEpoch {
    /// Returns the human-readable display name of the geological epoch.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hadean => "Hadean Eon",
            Self::Archean => "Archean Eon",
            Self::Proterozoic => "Proterozoic Eon",
            Self::Phanerozoic => "Phanerozoic Eon",
            Self::Modern => "Modern Holocene",
            Self::Future => "Far-Future Pangea Ultima",
        }
    }

    /// Returns a short label suitable for HUD buttons and badges.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Hadean => "Hadean",
            Self::Archean => "Archean",
            Self::Proterozoic => "Proterozoic",
            Self::Phanerozoic => "Phanerozoic",
            Self::Modern => "Modern",
            Self::Future => "Future",
        }
    }

    /// Returns the approximate age range in Ga / Ma.
    pub fn age_range_str(&self) -> &'static str {
        match self {
            Self::Hadean => "4.56 – 4.00 Ga",
            Self::Archean => "4.00 – 2.50 Ga",
            Self::Proterozoic => "2.50 – 0.54 Ga",
            Self::Phanerozoic => "540 – 10 Ma",
            Self::Modern => "Present (0 Ga)",
            Self::Future => "+100 Ma – +1.5 Ga",
        }
    }

    /// Representative canonical geological age in Gigayears from planetary birth (0.0 to 5.5 Gyr).
    pub fn canonical_age_gyr(&self) -> f32 {
        match self {
            Self::Hadean => 0.25,      // ~4.31 Ga ago
            Self::Archean => 1.50,     // ~3.06 Ga ago
            Self::Proterozoic => 3.00, // ~1.56 Ga ago
            Self::Phanerozoic => 4.25, // ~310 Ma ago (Carboniferous)
            Self::Modern => 4.56,      // Present day
            Self::Future => 5.20,      // +640 Ma future
        }
    }

    /// Classifies an elapsed geological age in Gyr into its corresponding epoch.
    pub fn from_geological_age_gyr(age_gyr: f32) -> Self {
        if age_gyr < 0.56 {
            Self::Hadean
        } else if age_gyr < 2.06 {
            Self::Archean
        } else if age_gyr < 4.02 {
            Self::Proterozoic
        } else if age_gyr < 4.55 {
            Self::Phanerozoic
        } else if age_gyr <= 4.65 {
            Self::Modern
        } else {
            Self::Future
        }
    }

    /// Returns the next chronologically sequential epoch.
    pub fn next(&self) -> Self {
        match self {
            Self::Hadean => Self::Archean,
            Self::Archean => Self::Proterozoic,
            Self::Proterozoic => Self::Phanerozoic,
            Self::Phanerozoic => Self::Modern,
            Self::Modern | Self::Future => Self::Future,
        }
    }

    /// Returns the previous chronologically sequential epoch.
    pub fn prev(&self) -> Self {
        match self {
            Self::Hadean | Self::Archean => Self::Hadean,
            Self::Proterozoic => Self::Archean,
            Self::Phanerozoic => Self::Proterozoic,
            Self::Modern => Self::Phanerozoic,
            Self::Future => Self::Modern,
        }
    }

    /// Summary of the dominant supercontinent cycle state.
    pub fn supercontinent_name(&self) -> &'static str {
        match self {
            Self::Hadean => "Magma Ocean & Island Arcs",
            Self::Archean => "Vaalbara & Ur Cratons",
            Self::Proterozoic => "Columbia & Rodinia Supercontinents",
            Self::Phanerozoic => "Pangea & Dispersing Continents",
            Self::Modern => "Dispersed 7 Continents",
            Self::Future => "Pangea Ultima / Novopangea",
        }
    }
}

/// Tracks deep-time planetary geological evolution, continental drift,
/// ocean oxidation, and terrestrial plant colonization.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeologicalState {
    /// Active geological epoch.
    pub epoch: GeologicalEpoch,
    /// Planetary age in Gyr since accretion (0.0 to 6.0 Gyr).
    pub geological_age_gyr: f32,
    /// Continental drift Wilson cycle phase in [0.0, 1.0] (~500 Myr period).
    pub continental_drift_phase: f32,
    /// Supercontinent aggregation index (1.0 = single supercontinent like Pangea, 0.15 = dispersed modern continents).
    pub supercontinent_aggregation: f32,
    /// Atmospheric oxygen level relative to Present Atmospheric Level (PAL).
    pub oxygen_level_pal: f32,
    /// Ocean oxidation progress (0.0 = Archean green ferruginous, 1.0 = modern sapphire blue).
    pub ocean_oxidation_progress: f32,
    /// Terrestrial plant colonization fraction (0.0 = barren rock craton, 1.0 = modern lush biomes).
    pub terrestrial_vegetation_fraction: f32,
    /// Volcanic / orogenic mountain-building activity factor in [0.0, 1.0].
    pub volcanic_orogeny_activity: f32,
}

impl Default for GeologicalState {
    fn default() -> Self {
        Self::new_for_epoch(GeologicalEpoch::Modern)
    }
}

impl GeologicalState {
    /// Constructs canonical baseline geological conditions for a specified epoch.
    pub fn new_for_epoch(epoch: GeologicalEpoch) -> Self {
        let age_gyr = epoch.canonical_age_gyr();
        let drift_phase = (age_gyr / 0.50).fract();
        let aggregation = match epoch {
            GeologicalEpoch::Hadean => 0.95,      // Clustered early proto-plates
            GeologicalEpoch::Archean => 0.40,     // Fragmented proto-cratons
            GeologicalEpoch::Proterozoic => 0.85, // Columbia / Rodinia
            GeologicalEpoch::Phanerozoic => 0.70, // Pangea
            GeologicalEpoch::Modern => 0.15,      // Dispersed
            GeologicalEpoch::Future => 0.90,      // Pangea Ultima
        };
        let oxygen_pal = match epoch {
            GeologicalEpoch::Hadean => 0.0001,
            GeologicalEpoch::Archean => 0.001,
            GeologicalEpoch::Proterozoic => 0.12, // Post-GOE
            GeologicalEpoch::Phanerozoic => 0.95,
            GeologicalEpoch::Modern => 1.0,
            GeologicalEpoch::Future => 0.75,
        };
        let ocean_oxidation = match epoch {
            GeologicalEpoch::Hadean => 0.0,
            GeologicalEpoch::Archean => 0.02, // Anoxic green water
            GeologicalEpoch::Proterozoic => 0.65, // Banded iron precipitation
            GeologicalEpoch::Phanerozoic => 0.98,
            GeologicalEpoch::Modern | GeologicalEpoch::Future => 1.0,
        };
        let veg_fraction = match epoch {
            GeologicalEpoch::Hadean | GeologicalEpoch::Archean => 0.0,
            GeologicalEpoch::Proterozoic => 0.02, // Algal coastal margins only
            GeologicalEpoch::Phanerozoic => 0.92, // Land plant explosion
            GeologicalEpoch::Modern => 1.0,
            GeologicalEpoch::Future => 0.60,
        };
        let orogeny = match epoch {
            GeologicalEpoch::Hadean => 0.90,
            GeologicalEpoch::Archean => 0.50,
            GeologicalEpoch::Proterozoic => 0.75,
            GeologicalEpoch::Phanerozoic => 0.60,
            GeologicalEpoch::Modern => 0.45,
            GeologicalEpoch::Future => 0.80,
        };

        Self {
            epoch,
            geological_age_gyr: age_gyr,
            continental_drift_phase: drift_phase,
            supercontinent_aggregation: aggregation,
            oxygen_level_pal: oxygen_pal,
            ocean_oxidation_progress: ocean_oxidation,
            terrestrial_vegetation_fraction: veg_fraction,
            volcanic_orogeny_activity: orogeny,
        }
    }
}

/// Global resource tracking timeline scrubber HUD state and target world.
#[derive(Resource, Debug, Clone)]
pub struct TimelineScrubber {
    /// Whether the timeline scrubber HUD panel is visible.
    pub is_open: bool,
    /// Focused celestial body entity for detailed geological scrutiny (defaults to primary terrestrial world / Earth).
    pub target_entity: Option<Entity>,
    /// Currently highlighted geological epoch in the scrubber.
    pub active_epoch: GeologicalEpoch,
    /// Target geological age in Gyr requested by the user.
    pub scrubbed_age_gyr: f32,
    /// Auto-advance geological timeline during normal simulation run.
    pub auto_advance: bool,
}

impl Default for TimelineScrubber {
    fn default() -> Self {
        Self {
            is_open: false,
            target_entity: None,
            active_epoch: GeologicalEpoch::Modern,
            scrubbed_age_gyr: 4.56,
            auto_advance: true,
        }
    }
}
