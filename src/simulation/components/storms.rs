//! Atmospheric storm, zonal jet shear, and polar vortex dynamics components.
//!
//! Models planetary-scale atmospheric fluid phenomena including Saturn's standing
//! Rossby wave North Polar Hexagon, Jupiter's Great Red Spot anticyclonic vortex,
//! Oval BA ("Red Spot Jr"), temperate white ovals, and Kelvin-Helmholtz shear billows.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Atmospheric storm, zonal wind shear, and polar vortex state for gas giants, ice giants,
/// and dense atmospheres (e.g. Jovian Great Red Spot, Saturn's North Polar Hexagon).
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AtmosphericStormState {
    /// Amplitude of the standing polar Rossby wave ($A_6 \approx 0.08 - 0.14$ for Saturn-like hexagon; $0.0$ if circular).
    pub polar_hexagon_amplitude: f32,
    /// Azimuthal wavenumber of the polar wave ($k = 6.0$ for Saturn's hexagon, $0.0$ if circular).
    pub polar_hexagon_wavenumber: f32,
    /// Angular colatitude of the polar hexagon boundary in radians (typically $\approx 0.22\,\text{rad} \approx 12.5^\circ$).
    pub polar_hexagon_colatitude: f32,
    /// Great Red Spot / primary anticyclonic storm relative radius ($0.0$ if absent, $\approx 0.35 - 0.45$ on Jupiter).
    pub great_spot_size: f32,
    /// Latitudinal position of the primary storm in radians (e.g. $-0.38\,\text{rad} \approx -22^\circ$ for GRS).
    pub great_spot_latitude_rad: f32,
    /// Longitudinal drift offset of the primary storm in radians.
    pub great_spot_longitude_rad: f32,
    /// Internal vortex rotation speed multiplier ($\sim 1.0 - 4.0$).
    pub vortex_spin_rate: f32,
    /// Number of secondary temperate white ovals / anticyclones ($0$ to $6$).
    pub secondary_oval_count: u32,
    /// Zonal jet shear turbulent intensity ($0.0 = $ smooth bands, $1.0 = $ turbulent Kelvin-Helmholtz billows).
    pub zonal_shear_turbulence: f32,
}

impl Default for AtmosphericStormState {
    fn default() -> Self {
        Self {
            polar_hexagon_amplitude: 0.0,
            polar_hexagon_wavenumber: 0.0,
            polar_hexagon_colatitude: 0.22,
            great_spot_size: 0.0,
            great_spot_latitude_rad: -0.38,
            great_spot_longitude_rad: 0.0,
            vortex_spin_rate: 1.0,
            secondary_oval_count: 0,
            zonal_shear_turbulence: 0.5,
        }
    }
}

impl AtmosphericStormState {
    /// Canonical Jovian configuration: massive Great Red Spot at 22°S, Oval BA,
    /// strings of temperate white ovals, and high-energy Kelvin-Helmholtz zonal shear.
    pub fn jupiter() -> Self {
        Self {
            polar_hexagon_amplitude: 0.0,
            polar_hexagon_wavenumber: 0.0,
            polar_hexagon_colatitude: 0.22,
            great_spot_size: 0.42,
            great_spot_latitude_rad: -0.384, // -22.0 degrees
            great_spot_longitude_rad: 0.65,
            vortex_spin_rate: 2.8,
            secondary_oval_count: 4,
            zonal_shear_turbulence: 0.85,
        }
    }

    /// Canonical Saturnian configuration: famous standing North Polar Hexagon (k=6 Rossby wave),
    /// tranquil central polar hurricane eye, and golden-butterscotch zonal bands with fine shear.
    pub fn saturn() -> Self {
        Self {
            polar_hexagon_amplitude: 0.10,
            polar_hexagon_wavenumber: 6.0,
            polar_hexagon_colatitude: 0.218, // ~12.5 degrees colatitude (~77.5°N)
            great_spot_size: 0.22,           // Great White Spot capability
            great_spot_latitude_rad: 0.15,
            great_spot_longitude_rad: -0.45,
            vortex_spin_rate: 1.4,
            secondary_oval_count: 1,
            zonal_shear_turbulence: 0.60,
        }
    }

    /// Canonical Ice Giant (Neptune / Uranus) configuration: high-speed retrograde equatorial winds,
    /// transient Great Dark Spot, and bright white companion cirrus methane clouds.
    pub fn neptune() -> Self {
        Self {
            polar_hexagon_amplitude: 0.0,
            polar_hexagon_wavenumber: 0.0,
            polar_hexagon_colatitude: 0.22,
            great_spot_size: 0.32,
            great_spot_latitude_rad: -0.35,
            great_spot_longitude_rad: 1.20,
            vortex_spin_rate: 1.8,
            secondary_oval_count: 2,
            zonal_shear_turbulence: 0.40,
        }
    }

    /// Terrestrial atmospheric storm configuration: tropical cyclonic spiral systems and polar dipoles.
    pub fn terrestrial() -> Self {
        Self {
            polar_hexagon_amplitude: 0.0,
            polar_hexagon_wavenumber: 0.0,
            polar_hexagon_colatitude: 0.22,
            great_spot_size: 0.25,
            great_spot_latitude_rad: 0.35,
            great_spot_longitude_rad: -0.80,
            vortex_spin_rate: 1.2,
            secondary_oval_count: 2,
            zonal_shear_turbulence: 0.30,
        }
    }
}
