//! Volcanic and cryovolcanic activity components.
//!
//! Models hyperactive sulfur/silicate volcanism (e.g. Io's Pele and Loki Patera)
//! driven by extreme tidal heating flexure, as well as cryovolcanic geyser venting
//! on icy moons (e.g. Enceladus and Europa) feeding planetary rings.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Classification of planetary volcanic activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VolcanicActivityType {
    /// Inactive or dormant volcanic lithosphere.
    #[default]
    Dormant,
    /// Hyperactive sulfur and ultramafic silicate volcanism (e.g. Io).
    IoSulfurVolcanism,
    /// Standard terrestrial silicate volcanism (e.g. Earth, young Venus).
    SilicateBasaltic,
    /// Subsurface ocean water-ice & ammonia cryovolcanism (e.g. Enceladus, Europa).
    CryoWaterIce,
    /// Cryogenic hydrocarbon/methane volcanism (e.g. Titan, Triton, Pluto).
    CryoHydrocarbon,
}

/// Atmospheric and surface volcanic state for celestial bodies.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VolcanicState {
    /// Primary classification of volcanic activity.
    pub activity_type: VolcanicActivityType,
    /// Number of active eruptive plumes or geysers (0 to 12).
    pub active_plumes: u32,
    /// Maximum plume altitude in kilometers (e.g. ~350 km for Io, ~500 km for Enceladus).
    pub max_plume_height_km: f32,
    /// Mass eruption rate in kilograms per second.
    pub eruption_flux_kg_s: f64,
    /// Cryovolcanic mass loss feeding host planetary rings in kg/s.
    pub cryo_ring_feed_rate_kg_s: f64,
    /// Peak temperature of active volcanic paterae or calderas in Kelvin.
    pub caldera_temperature_k: f32,
    /// Relative sulfur allotrope fallout coverage (0.0 to 1.0) around active vents.
    pub sulfur_fallout_fraction: f32,
}

impl Default for VolcanicState {
    fn default() -> Self {
        Self {
            activity_type: VolcanicActivityType::Dormant,
            active_plumes: 0,
            max_plume_height_km: 0.0,
            eruption_flux_kg_s: 0.0,
            cryo_ring_feed_rate_kg_s: 0.0,
            caldera_temperature_k: 300.0,
            sulfur_fallout_fraction: 0.0,
        }
    }
}

impl VolcanicState {
    /// Canonical Io-like hyperactive sulfur volcanism driven by extreme tidal heating.
    pub fn io() -> Self {
        Self {
            activity_type: VolcanicActivityType::IoSulfurVolcanism,
            active_plumes: 6,
            max_plume_height_km: 420.0,
            eruption_flux_kg_s: 1.2e5,
            cryo_ring_feed_rate_kg_s: 0.0,
            caldera_temperature_k: 1650.0,
            sulfur_fallout_fraction: 0.85,
        }
    }

    /// Canonical Enceladus-like cryovolcanic water-ice geysers venting into space.
    pub fn enceladus() -> Self {
        Self {
            activity_type: VolcanicActivityType::CryoWaterIce,
            active_plumes: 4,
            max_plume_height_km: 500.0,
            eruption_flux_kg_s: 250.0,
            cryo_ring_feed_rate_kg_s: 200.0, // Escapes directly into Saturn's E-Ring
            caldera_temperature_k: 185.0,    // Warm thermal anomaly over 75 K background
            sulfur_fallout_fraction: 0.0,
        }
    }

    /// Canonical Europa-like cryotectonic lineae with periodic tidal fissure venting.
    pub fn europa() -> Self {
        Self {
            activity_type: VolcanicActivityType::CryoWaterIce,
            active_plumes: 2,
            max_plume_height_km: 200.0,
            eruption_flux_kg_s: 80.0,
            cryo_ring_feed_rate_kg_s: 15.0,
            caldera_temperature_k: 130.0,
            sulfur_fallout_fraction: 0.0,
        }
    }
}
