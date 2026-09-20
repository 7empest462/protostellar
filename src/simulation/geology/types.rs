//! Data types, components, and resources for Deep Geological Time,
//! Epoch Scrubbing, Continental Drift, and Planetary Oxidation.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Terrestrial planet archetype for geological timeline categorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Component)]
pub enum EpochTargetPlanet {
    /// Earth (Hadean, Archean, Proterozoic, Snowball, Phanerozoic, Modern, Future).
    #[default]
    Earth,
    /// Mars (Pre-Noachian, Noachian, Hesperian, Amazonian, Future).
    Mars,
    /// Venus (Primordial, Temperate, Runaway, Modern, Future).
    Venus,
}

impl EpochTargetPlanet {
    /// Detects the target planet archetype from a body name if it matches Earth, Mars, or Venus.
    pub fn try_detect_from_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        if lower.contains("mars") {
            Some(Self::Mars)
        } else if lower.contains("venus") {
            Some(Self::Venus)
        } else if lower.contains("earth") || lower.contains("terra") {
            Some(Self::Earth)
        } else {
            None
        }
    }

    /// Detects the target planet archetype from a body name, falling back to Earth if unspecified.
    pub fn detect_from_name(name: &str) -> Self {
        Self::try_detect_from_name(name).unwrap_or(Self::Earth)
    }

    /// Human-readable name of the planet archetype.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Earth => "Earth",
            Self::Mars => "Mars",
            Self::Venus => "Venus",
        }
    }
}

/// Major planetary geological epochs spanning the 4.56-billion-year history
/// of terrestrial worlds (Earth, Mars, Venus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum GeologicalEpoch {
    // ---------------- Earth Epochs ----------------
    /// 4.56 Ga - 4.0 Ga: Magma ocean cooling, heavy impact bombardment (LHB),
    /// dense steam greenhouse atmosphere, and primordial ocean condensation.
    Hadean,
    /// 4.0 Ga - 2.5 Ga: Micro-continental cratons (Vaalbara, Ur), anoxic green
    /// iron-rich oceans, orange methane skies, and early anaerobic stromatolites.
    Archean,
    /// 2.5 Ga - 0.75 Ga: Great Oxidation Event (GOE), banded iron rust formation,
    /// supercontinents Columbia and Rodinia.
    Proterozoic,
    /// 720 Ma - 635 Ma: Cryogenian glaciations (Sturtian & Marinoan), runaway ice-albedo
    /// feedback, and global equatorial ice sheets covering the planet.
    SnowballEarth,
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

    // ---------------- Mars Epochs ----------------
    /// 4.56 Ga - 4.10 Ga: Molten basaltic protocrust, early active core dynamo (0.35 G dipole),
    /// dense CO2/H2O envelope (~1.5 bar), and hydrothermal vents.
    MarsPreNoachian,
    /// 4.10 Ga - 3.70 Ga: "Warm & Wet Mars", ~0.80 bar atmosphere, northern ocean (Oceanus Borealis,
    /// 35% coverage), lake basins, and valley networks.
    MarsNoachian,
    /// 3.70 Ga - 3.00 Ga: Catastrophic outflow floods, massive Tharsis volcanism (Olympus Mons,
    /// Valles Marineris), cooling to 0.15 bar, and sulfur-rich acidic lakes.
    MarsHesperian,
    /// 3.00 Ga - Present: Hyper-arid cold rust desert, ultra-thin atmosphere (0.006 bar),
    /// global dust storms, and CO2/H2O polar ice caps.
    MarsAmazonian,
    /// +100 Ma - +1.5 Ga: Extreme solar wind atmospheric stripping, sublimated residual caps,
    /// and ancient desiccated lithosphere.
    MarsFuture,

    // ---------------- Venus Epochs ----------------
    /// 4.56 Ga - 4.20 Ga: Magma ocean, superdense steam and volatile envelope (40 bar),
    /// and intense catastrophic mantle overturn.
    VenusPrimordial,
    /// 4.20 Ga - 1.50 Ga: NASA GISS temperate model. Liquid water oceans (40% coverage),
    /// Ishtar & Aphrodite highlands, day-side cloud shielding, and 1.5 bar atmosphere.
    VenusTemperate,
    /// 1.50 Ga - 0.70 Ga: Solar brightening triggers runaway moist greenhouse, ocean boiling,
    /// crustal dehydration, and global basaltic flood resurfacing.
    VenusRunaway,
    /// Present (0 Ga): 92 bar CO2 supercritical atmosphere, opaque 100% sulfuric acid cloud deck,
    /// 735 K (462°C) surface, and zero liquid water.
    VenusModern,
    /// +100 Ma - +1.5 Ga: Scorched thermal stagnant lid, 100 bar atmosphere, and baked lithosphere.
    VenusFuture,
}

#[allow(
    clippy::match_same_arms,
    reason = "Distinct planetary epochs across Earth, Mars, and Venus may share labels or numeric values"
)]
impl GeologicalEpoch {
    /// Returns the target planetary archetype associated with this epoch.
    pub fn target_planet(&self) -> EpochTargetPlanet {
        match self {
            Self::Hadean
            | Self::Archean
            | Self::Proterozoic
            | Self::SnowballEarth
            | Self::Phanerozoic
            | Self::Modern
            | Self::Future => EpochTargetPlanet::Earth,
            Self::MarsPreNoachian
            | Self::MarsNoachian
            | Self::MarsHesperian
            | Self::MarsAmazonian
            | Self::MarsFuture => EpochTargetPlanet::Mars,
            Self::VenusPrimordial
            | Self::VenusTemperate
            | Self::VenusRunaway
            | Self::VenusModern
            | Self::VenusFuture => EpochTargetPlanet::Venus,
        }
    }

    /// Returns the canonical sequence of epochs for a given planet archetype.
    pub fn epochs_for_planet(planet: EpochTargetPlanet) -> &'static [Self] {
        match planet {
            EpochTargetPlanet::Earth => &[
                Self::Hadean,
                Self::Archean,
                Self::Proterozoic,
                Self::SnowballEarth,
                Self::Phanerozoic,
                Self::Modern,
                Self::Future,
            ],
            EpochTargetPlanet::Mars => &[
                Self::MarsPreNoachian,
                Self::MarsNoachian,
                Self::MarsHesperian,
                Self::MarsAmazonian,
                Self::MarsFuture,
            ],
            EpochTargetPlanet::Venus => &[
                Self::VenusPrimordial,
                Self::VenusTemperate,
                Self::VenusRunaway,
                Self::VenusModern,
                Self::VenusFuture,
            ],
        }
    }

    /// Returns the human-readable display name of the geological epoch.
    pub fn name(&self) -> &'static str {
        match self {
            // Earth
            Self::Hadean => "Hadean Eon",
            Self::Archean => "Archean Eon",
            Self::Proterozoic => "Proterozoic Eon",
            Self::SnowballEarth => "Cryogenian Snowball Earth",
            Self::Phanerozoic => "Phanerozoic Eon",
            Self::Modern => "Modern Holocene",
            Self::Future => "Far-Future Pangea Ultima",
            // Mars
            Self::MarsPreNoachian => "Pre-Noachian Era",
            Self::MarsNoachian => "Noachian Wet Era",
            Self::MarsHesperian => "Hesperian Volcanic Era",
            Self::MarsAmazonian => "Amazonian Arid Era",
            Self::MarsFuture => "Far-Future Desiccation",
            // Venus
            Self::VenusPrimordial => "Primordial Magma Era",
            Self::VenusTemperate => "Temperate Ocean Era",
            Self::VenusRunaway => "Runaway Greenhouse Transition",
            Self::VenusModern => "Modern Supercritical Hellscape",
            Self::VenusFuture => "Far-Future Scorched Lithosphere",
        }
    }

    /// Returns a short label suitable for HUD buttons and badges.
    pub fn short_name(&self) -> &'static str {
        match self {
            // Earth
            Self::Hadean => "Hadean",
            Self::Archean => "Archean",
            Self::Proterozoic => "Proterozoic",
            Self::SnowballEarth => "Snowball",
            Self::Phanerozoic => "Phanerozoic",
            Self::Modern => "Modern",
            Self::Future => "Future",
            // Mars
            Self::MarsPreNoachian => "Pre-Noachian",
            Self::MarsNoachian => "Noachian",
            Self::MarsHesperian => "Hesperian",
            Self::MarsAmazonian => "Amazonian",
            Self::MarsFuture => "Future",
            // Venus
            Self::VenusPrimordial => "Primordial",
            Self::VenusTemperate => "Temperate",
            Self::VenusRunaway => "Runaway",
            Self::VenusModern => "Modern",
            Self::VenusFuture => "Future",
        }
    }

    /// Returns the approximate age range in Ga / Ma.
    pub fn age_range_str(&self) -> &'static str {
        match self {
            // Earth
            Self::Hadean => "4.56 – 4.00 Ga",
            Self::Archean => "4.00 – 2.50 Ga",
            Self::Proterozoic => "2.50 – 0.75 Ga",
            Self::SnowballEarth => "720 – 635 Ma",
            Self::Phanerozoic => "540 – 10 Ma",
            Self::Modern => "Present (0 Ga)",
            Self::Future => "+100 Ma – +1.5 Ga",
            // Mars
            Self::MarsPreNoachian => "4.56 – 4.10 Ga",
            Self::MarsNoachian => "4.10 – 3.70 Ga",
            Self::MarsHesperian => "3.70 – 3.00 Ga",
            Self::MarsAmazonian => "3.00 Ga – Present",
            Self::MarsFuture => "+100 Ma – +1.5 Ga",
            // Venus
            Self::VenusPrimordial => "4.56 – 4.20 Ga",
            Self::VenusTemperate => "4.20 – 1.50 Ga",
            Self::VenusRunaway => "1.50 – 0.70 Ga",
            Self::VenusModern => "Present (0 Ga)",
            Self::VenusFuture => "+100 Ma – +1.5 Ga",
        }
    }

    /// Representative canonical geological age in Gigayears from planetary birth (0.0 to 5.5 Gyr).
    pub fn canonical_age_gyr(&self) -> f32 {
        match self {
            // Earth
            Self::Hadean => 0.25,        // ~4.31 Ga ago
            Self::Archean => 1.50,       // ~3.06 Ga ago
            Self::Proterozoic => 2.50,   // ~2.06 Ga ago
            Self::SnowballEarth => 3.85, // ~710 Ma ago (Sturtian glaciation)
            Self::Phanerozoic => 4.25,   // ~310 Ma ago (Carboniferous)
            Self::Modern => 4.56,        // Present day
            Self::Future => 5.20,        // +640 Ma future
            // Mars
            Self::MarsPreNoachian => 0.30,
            Self::MarsNoachian => 1.00,
            Self::MarsHesperian => 1.80,
            Self::MarsAmazonian => 4.56,
            Self::MarsFuture => 5.20,
            // Venus
            Self::VenusPrimordial => 0.30,
            Self::VenusTemperate => 2.00,
            Self::VenusRunaway => 3.80,
            Self::VenusModern => 4.56,
            Self::VenusFuture => 5.20,
        }
    }

    /// Returns the canonical supercontinent / tectonic aggregation fraction in [0.0, 1.0].
    pub fn canonical_aggregation(&self) -> f32 {
        match self {
            // Earth
            Self::Hadean => 0.95,        // Clustered early proto-plates
            Self::Archean => 0.40,       // Fragmented proto-cratons (Vaalbara, Ur)
            Self::Proterozoic => 0.85,   // Columbia / Rodinia supercontinents
            Self::SnowballEarth => 0.75, // Rodinia breakup
            Self::Phanerozoic => 0.70,   // Pangea supercontinent
            Self::Modern => 0.15,        // Dispersed 7 continents
            Self::Future => 0.90,        // Pangea Ultima / Novopangea
            // Mars
            Self::MarsPreNoachian => 0.80,
            Self::MarsNoachian => 0.60,
            Self::MarsHesperian => 0.50,
            Self::MarsAmazonian => 0.35,
            Self::MarsFuture => 0.30,
            // Venus
            Self::VenusPrimordial => 0.90,
            Self::VenusTemperate => 0.55,
            Self::VenusRunaway => 0.70,
            Self::VenusModern => 0.40,
            Self::VenusFuture => 0.30,
        }
    }

    /// Returns the canonical surface temperature in Kelvin for this epoch.
    pub fn canonical_temperature_k(&self) -> f64 {
        match self {
            // Earth
            Self::Hadean => 550.0,
            Self::Archean => 315.0,
            Self::Proterozoic => 265.0,
            Self::SnowballEarth => 220.0,
            Self::Phanerozoic | Self::Modern => 288.0,
            Self::Future => 345.0,
            // Mars
            Self::MarsPreNoachian => 295.0,
            Self::MarsNoachian => 280.0,
            Self::MarsHesperian => 240.0,
            Self::MarsAmazonian => 215.0,
            Self::MarsFuture => 230.0,
            // Venus
            Self::VenusPrimordial => 600.0,
            Self::VenusTemperate => 295.0,
            Self::VenusRunaway => 480.0,
            Self::VenusModern => 735.0,
            Self::VenusFuture => 780.0,
        }
    }

    /// Classifies an elapsed geological age in Gyr into its corresponding epoch for a specified planet.
    pub fn from_age_and_planet(age_gyr: f32, planet: EpochTargetPlanet) -> Self {
        match planet {
            EpochTargetPlanet::Earth => {
                if age_gyr < 0.56 {
                    Self::Hadean
                } else if age_gyr < 2.06 {
                    Self::Archean
                } else if age_gyr < 3.75 {
                    Self::Proterozoic
                } else if age_gyr < 4.02 {
                    Self::SnowballEarth
                } else if age_gyr < 4.55 {
                    Self::Phanerozoic
                } else if age_gyr <= 4.65 {
                    Self::Modern
                } else {
                    Self::Future
                }
            }
            EpochTargetPlanet::Mars => {
                if age_gyr < 0.60 {
                    Self::MarsPreNoachian
                } else if age_gyr < 1.40 {
                    Self::MarsNoachian
                } else if age_gyr < 2.50 {
                    Self::MarsHesperian
                } else if age_gyr <= 4.70 {
                    Self::MarsAmazonian
                } else {
                    Self::MarsFuture
                }
            }
            EpochTargetPlanet::Venus => {
                if age_gyr < 0.60 {
                    Self::VenusPrimordial
                } else if age_gyr < 3.00 {
                    Self::VenusTemperate
                } else if age_gyr < 4.30 {
                    Self::VenusRunaway
                } else if age_gyr <= 4.70 {
                    Self::VenusModern
                } else {
                    Self::VenusFuture
                }
            }
        }
    }

    /// Classifies an elapsed geological age in Gyr into its corresponding Earth epoch (default).
    pub fn from_geological_age_gyr(age_gyr: f32) -> Self {
        Self::from_age_and_planet(age_gyr, EpochTargetPlanet::Earth)
    }

    /// Returns the next chronologically sequential epoch within the same planetary timeline.
    pub fn next(&self) -> Self {
        match self {
            // Earth
            Self::Hadean => Self::Archean,
            Self::Archean => Self::Proterozoic,
            Self::Proterozoic => Self::SnowballEarth,
            Self::SnowballEarth => Self::Phanerozoic,
            Self::Phanerozoic => Self::Modern,
            Self::Modern | Self::Future => Self::Future,
            // Mars
            Self::MarsPreNoachian => Self::MarsNoachian,
            Self::MarsNoachian => Self::MarsHesperian,
            Self::MarsHesperian => Self::MarsAmazonian,
            Self::MarsAmazonian | Self::MarsFuture => Self::MarsFuture,
            // Venus
            Self::VenusPrimordial => Self::VenusTemperate,
            Self::VenusTemperate => Self::VenusRunaway,
            Self::VenusRunaway => Self::VenusModern,
            Self::VenusModern | Self::VenusFuture => Self::VenusFuture,
        }
    }

    /// Returns the previous chronologically sequential epoch within the same planetary timeline.
    pub fn prev(&self) -> Self {
        match self {
            // Earth
            Self::Hadean | Self::Archean => Self::Hadean,
            Self::Proterozoic => Self::Archean,
            Self::SnowballEarth => Self::Proterozoic,
            Self::Phanerozoic => Self::SnowballEarth,
            Self::Modern => Self::Phanerozoic,
            Self::Future => Self::Modern,
            // Mars
            Self::MarsPreNoachian | Self::MarsNoachian => Self::MarsPreNoachian,
            Self::MarsHesperian => Self::MarsNoachian,
            Self::MarsAmazonian => Self::MarsHesperian,
            Self::MarsFuture => Self::MarsAmazonian,
            // Venus
            Self::VenusPrimordial | Self::VenusTemperate => Self::VenusPrimordial,
            Self::VenusRunaway => Self::VenusTemperate,
            Self::VenusModern => Self::VenusRunaway,
            Self::VenusFuture => Self::VenusModern,
        }
    }

    /// Summary of the dominant supercontinent or surface geology state.
    pub fn supercontinent_name(&self) -> &'static str {
        match self {
            // Earth
            Self::Hadean => "Magma Ocean & Island Arcs",
            Self::Archean => "Vaalbara & Ur Cratons",
            Self::Proterozoic => "Columbia & Rodinia Supercontinents",
            Self::SnowballEarth => "Global Sturtian & Marinoan Ice Sheet",
            Self::Phanerozoic => "Pangea & Dispersing Continents",
            Self::Modern => "Dispersed 7 Continents",
            Self::Future => "Pangea Ultima / Novopangea",
            // Mars
            Self::MarsPreNoachian => "Early Basaltic Protocrust",
            Self::MarsNoachian => "Oceanus Borealis & Southern Highlands",
            Self::MarsHesperian => "Tharsis Bulge & Olympus Mons",
            Self::MarsAmazonian => "Valles Marineris & Planitia Plains",
            Self::MarsFuture => "Ancient Weathered Lithosphere",
            // Venus
            Self::VenusPrimordial => "Convecting Magma Ocean",
            Self::VenusTemperate => "Ishtar & Aphrodite Terra Highlands",
            Self::VenusRunaway => "Global Basaltic Flood Volcanism",
            Self::VenusModern => "Volcanic Tesserae & Coronae",
            Self::VenusFuture => "Thermal Stagnant Lid",
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
    #[allow(
        clippy::match_same_arms,
        reason = "Distinct planetary epochs may share baseline geological attributes"
    )]
    pub fn new_for_epoch(epoch: GeologicalEpoch) -> Self {
        let age_gyr = epoch.canonical_age_gyr();
        let drift_phase = (age_gyr / 0.50).fract();
        let aggregation = epoch.canonical_aggregation();
        let oxygen_pal = match epoch {
            GeologicalEpoch::Hadean => 0.0001,
            GeologicalEpoch::Archean => 0.001,
            GeologicalEpoch::Proterozoic => 0.12, // Post-GOE
            GeologicalEpoch::SnowballEarth => 0.08,
            GeologicalEpoch::Phanerozoic => 0.95,
            GeologicalEpoch::Modern => 1.0,
            GeologicalEpoch::Future => 0.75,
            GeologicalEpoch::MarsPreNoachian
            | GeologicalEpoch::MarsNoachian
            | GeologicalEpoch::MarsHesperian
            | GeologicalEpoch::MarsAmazonian
            | GeologicalEpoch::MarsFuture
            | GeologicalEpoch::VenusPrimordial
            | GeologicalEpoch::VenusTemperate
            | GeologicalEpoch::VenusRunaway
            | GeologicalEpoch::VenusModern
            | GeologicalEpoch::VenusFuture => 0.0001,
        };
        let ocean_oxidation = match epoch {
            GeologicalEpoch::Hadean => 0.0,
            GeologicalEpoch::Archean => 0.02, // Anoxic green water
            GeologicalEpoch::Proterozoic => 0.65, // Banded iron precipitation
            GeologicalEpoch::SnowballEarth => 0.85,
            GeologicalEpoch::Phanerozoic => 0.98,
            GeologicalEpoch::Modern | GeologicalEpoch::Future => 1.0,
            GeologicalEpoch::MarsPreNoachian => 0.05,
            GeologicalEpoch::MarsNoachian => 0.20,
            GeologicalEpoch::MarsHesperian => 0.40,
            GeologicalEpoch::MarsAmazonian | GeologicalEpoch::MarsFuture => 0.0,
            GeologicalEpoch::VenusPrimordial => 0.0,
            GeologicalEpoch::VenusTemperate => 0.35,
            GeologicalEpoch::VenusRunaway
            | GeologicalEpoch::VenusModern
            | GeologicalEpoch::VenusFuture => 0.0,
        };
        let veg_fraction = match epoch {
            GeologicalEpoch::Hadean | GeologicalEpoch::Archean => 0.0,
            GeologicalEpoch::Proterozoic => 0.02, // Algal coastal margins only
            GeologicalEpoch::SnowballEarth => 0.01,
            GeologicalEpoch::Phanerozoic => 0.92, // Land plant explosion
            GeologicalEpoch::Modern => 1.0,
            GeologicalEpoch::Future => 0.60,
            _ => 0.0,
        };
        let orogeny = match epoch {
            GeologicalEpoch::Hadean => 0.90,
            GeologicalEpoch::Archean => 0.50,
            GeologicalEpoch::Proterozoic => 0.75,
            GeologicalEpoch::SnowballEarth => 0.60,
            GeologicalEpoch::Phanerozoic => 0.60,
            GeologicalEpoch::Modern => 0.45,
            GeologicalEpoch::Future => 0.80,
            GeologicalEpoch::MarsPreNoachian => 0.90,
            GeologicalEpoch::MarsNoachian => 0.70,
            GeologicalEpoch::MarsHesperian => 0.95,
            GeologicalEpoch::MarsAmazonian => 0.05,
            GeologicalEpoch::MarsFuture => 0.01,
            GeologicalEpoch::VenusPrimordial => 0.98,
            GeologicalEpoch::VenusTemperate => 0.50,
            GeologicalEpoch::VenusRunaway => 0.85,
            GeologicalEpoch::VenusModern => 0.35,
            GeologicalEpoch::VenusFuture => 0.15,
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

    /// Constructs baseline geological conditions appropriate for a specific planet archetype.
    pub fn default_for_planet(planet: EpochTargetPlanet) -> Self {
        match planet {
            EpochTargetPlanet::Earth => Self::new_for_epoch(GeologicalEpoch::Modern),
            EpochTargetPlanet::Mars => Self::new_for_epoch(GeologicalEpoch::MarsAmazonian),
            EpochTargetPlanet::Venus => Self::new_for_epoch(GeologicalEpoch::VenusModern),
        }
    }
}

/// Global resource tracking timeline scrubber HUD state and target world.
#[derive(Resource, Debug, Clone)]
pub struct TimelineScrubber {
    /// Whether the timeline scrubber HUD panel is visible.
    pub is_open: bool,
    /// Focused celestial body entity for detailed geological scrutiny.
    pub target_entity: Option<Entity>,
    /// Currently selected planetary archetype for epoch buttons.
    pub target_planet: EpochTargetPlanet,
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
            target_planet: EpochTargetPlanet::Earth,
            active_epoch: GeologicalEpoch::Modern,
            scrubbed_age_gyr: 4.56,
            auto_advance: true,
        }
    }
}
