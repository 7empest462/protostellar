//! Astronomical belt reservoirs, dynamical zoning, and minor body census tracking.

use bevy::color::Color;
use serde::{Deserialize, Serialize};

/// Canonical astronomical belt reservoirs based on semi-major axis / orbital radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BeltZone {
    /// Inner System / Near-Earth Asteroids / Terrestrial Grazers (< 2.10 AU)
    InnerSystem,
    /// Main Asteroid Belt between Mars and Jupiter (2.10 - 3.45 AU)
    AsteroidBelt,
    /// Jupiter Trojans, Hildas, and Outer Giant Feeding Zones / Centaurs (3.45 - 16.0 AU)
    TrojanCentaur,
    /// Kuiper Belt and Trans-Neptunian Cometary Reservoir (16.0 - 45.0 AU)
    KuiperBelt,
    /// Scattered Disk and Extreme Oort Reservoir (> 45.0 AU)
    ScatteredDisk,
}

impl BeltZone {
    /// Classifies an orbital distance in Astronomical Units (AU) into its corresponding belt reservoir.
    pub fn from_distance_au(dist_au: f64) -> Self {
        if dist_au < 2.10 {
            Self::InnerSystem
        } else if dist_au <= 3.45 {
            Self::AsteroidBelt
        } else if dist_au <= 16.0 {
            Self::TrojanCentaur
        } else if dist_au <= 45.0 {
            Self::KuiperBelt
        } else {
            Self::ScatteredDisk
        }
    }

    /// Full astronomical title of the belt reservoir.
    pub fn title(&self) -> &'static str {
        match self {
            Self::InnerSystem => "Inner Grazers (<2.1 AU)",
            Self::AsteroidBelt => "Main Asteroid Belt (2.1-3.45 AU)",
            Self::TrojanCentaur => "Trojans & Centaurs (3.45-16 AU)",
            Self::KuiperBelt => "Kuiper Belt (16-45 AU)",
            Self::ScatteredDisk => "Scattered Reservoir (>45 AU)",
        }
    }

    /// Compact display name for UI pills and buttons.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::InnerSystem => "Inner Grazers",
            Self::AsteroidBelt => "Asteroid Belt",
            Self::TrojanCentaur => "Trojans",
            Self::KuiperBelt => "Kuiper Belt",
            Self::ScatteredDisk => "Scattered Disk",
        }
    }

    /// Astronomical icon representing the belt composition and location.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::InnerSystem => "☀️",
            Self::AsteroidBelt => "🪨",
            Self::TrojanCentaur => "🪐",
            Self::KuiperBelt => "🧊",
            Self::ScatteredDisk => "🌌",
        }
    }

    /// Glassmorphic button background and border colors tailored to the belt's physical nature.
    pub fn button_colors(&self) -> (Color, Color) {
        match self {
            Self::InnerSystem => (
                Color::srgba(0.24, 0.14, 0.05, 0.90),
                Color::srgb(1.0, 0.70, 0.30),
            ),
            Self::AsteroidBelt => (
                Color::srgba(0.18, 0.13, 0.08, 0.90),
                Color::srgb(0.90, 0.75, 0.40),
            ),
            Self::TrojanCentaur => (
                Color::srgba(0.18, 0.10, 0.16, 0.90),
                Color::srgb(0.85, 0.55, 0.90),
            ),
            Self::KuiperBelt => (
                Color::srgba(0.06, 0.16, 0.26, 0.90),
                Color::srgb(0.40, 0.80, 1.0),
            ),
            Self::ScatteredDisk => (
                Color::srgba(0.10, 0.08, 0.22, 0.90),
                Color::srgb(0.65, 0.60, 1.0),
            ),
        }
    }

    /// Ordered slice of all 5 canonical belt zones from innermost to outermost.
    pub fn all() -> &'static [Self] {
        &[
            Self::InnerSystem,
            Self::AsteroidBelt,
            Self::TrojanCentaur,
            Self::KuiperBelt,
            Self::ScatteredDisk,
        ]
    }
}

/// Live demographic census tracking minor bodies across all 5 astronomical belts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BeltCensus {
    pub inner_count: usize,
    pub asteroid_belt_count: usize,
    pub trojan_count: usize,
    pub kuiper_count: usize,
    pub scattered_count: usize,
}

impl BeltCensus {
    /// Records a minor body at `dist_au` into the corresponding belt count.
    pub fn record(&mut self, dist_au: f64) {
        match BeltZone::from_distance_au(dist_au) {
            BeltZone::InnerSystem => self.inner_count += 1,
            BeltZone::AsteroidBelt => self.asteroid_belt_count += 1,
            BeltZone::TrojanCentaur => self.trojan_count += 1,
            BeltZone::KuiperBelt => self.kuiper_count += 1,
            BeltZone::ScatteredDisk => self.scattered_count += 1,
        }
    }

    /// Retrieves the object count for a specific belt zone.
    pub fn count(&self, zone: BeltZone) -> usize {
        match zone {
            BeltZone::InnerSystem => self.inner_count,
            BeltZone::AsteroidBelt => self.asteroid_belt_count,
            BeltZone::TrojanCentaur => self.trojan_count,
            BeltZone::KuiperBelt => self.kuiper_count,
            BeltZone::ScatteredDisk => self.scattered_count,
        }
    }

    /// Total number of minor bodies across all belts.
    pub fn total(&self) -> usize {
        self.inner_count
            + self.asteroid_belt_count
            + self.trojan_count
            + self.kuiper_count
            + self.scattered_count
    }

    /// Formats a concise one-line belt demographic readout for telemetry displays.
    pub fn format_summary_line(&self) -> String {
        format!(
            "Belts: ☀️ In: {} | 🪨 Main: {} | 🪐 Troj: {} | 🧊 Kuiper: {} | 🌌 Oort: {}",
            self.inner_count,
            self.asteroid_belt_count,
            self.trojan_count,
            self.kuiper_count,
            self.scattered_count,
        )
    }
}
