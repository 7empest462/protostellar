//! Core types, components, resources, and events for relativistic gravitation and gravitational waves.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::simulation::components::BodyType;

/// Component tracking General Relativistic Post-Newtonian (1PN) and Gravitational Wave quadrupole states.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct RelativisticState {
    /// 1PN relativistic periastron / perihelion advance rate in arcseconds per century (e.g. 43.0 for Mercury).
    pub precession_rate_arcsec_century: f64,
    /// Relativistic periastron advance per orbital revolution $\Delta \varpi = \frac{6\pi G M}{c^2 a (1-e^2)}$ in radians.
    pub precession_advance_per_orbit_rad: f64,
    /// Instantaneous quadrupole gravitational wave emission power in Watts (Peters 1964).
    pub gw_luminosity_watts: f64,
    /// Dimensionless characteristic gravitational wave strain $h$ at 10 kpc.
    pub gw_strain: f64,
    /// Quadrupole gravitational wave frequency $f_{\text{GW}} = 2 f_{\text{orb}}$ in Hertz.
    pub gw_frequency_hz: f64,
    /// Estimated time to coalescence / merger $\tau_{\text{merge}}$ in years.
    pub inspiral_timescale_yr: f64,
    /// Rate of orbital semi-major axis shrinkage $da/dt$ in AU per million years ($\text{AU/Myr}$).
    pub orbital_decay_rate_au_per_myr: f64,
    /// Total accumulated relativistic periastron advance in radians since simulation start.
    pub accumulated_precession_rad: f64,
    /// Flag indicating whether the binary has reached the Innermost Stable Circular Orbit (ISCO) or contact radius.
    pub is_coalescing: bool,
    /// Cached instantaneous orbital semi-major axis in AU.
    pub semi_major_axis_au: f64,
    /// Cached instantaneous orbital eccentricity.
    pub eccentricity: f64,
}

impl Default for RelativisticState {
    fn default() -> Self {
        Self {
            precession_rate_arcsec_century: 0.0,
            precession_advance_per_orbit_rad: 0.0,
            gw_luminosity_watts: 0.0,
            gw_strain: 0.0,
            gw_frequency_hz: 0.0,
            inspiral_timescale_yr: f64::INFINITY,
            orbital_decay_rate_au_per_myr: 0.0,
            accumulated_precession_rad: 0.0,
            is_coalescing: false,
            semi_major_axis_au: 1.0,
            eccentricity: 0.0,
        }
    }
}

impl RelativisticState {
    /// Constructs a pre-calibrated relativistic state for Proto-Mercury in the Solar MMSN scenario.
    pub fn mercury_like() -> Self {
        Self {
            precession_rate_arcsec_century: 42.98,
            precession_advance_per_orbit_rad: 5.019e-7,
            gw_luminosity_watts: 1.15e4,
            gw_strain: 2.1e-31,
            gw_frequency_hz: 2.6e-7,
            inspiral_timescale_yr: 1.2e29,
            orbital_decay_rate_au_per_myr: -1.3e-22,
            accumulated_precession_rad: 0.0,
            is_coalescing: false,
            semi_major_axis_au: 0.3871,
            eccentricity: 0.2056,
        }
    }

    /// Constructs an active relativistic state for a compact binary system (e.g. PSR B1913+16 or binary black hole).
    pub fn compact_binary(m1_solar: f64, m2_solar: f64, a_au: f64, eccentricity: f64) -> Self {
        use super::post_newtonian::*;

        let (_, rate_cy) = calculate_1pn_precession_rate(m1_solar + m2_solar, a_au, eccentricity);
        let advance_rad =
            calculate_1pn_precession_per_orbit(m1_solar + m2_solar, a_au, eccentricity);
        let power_watts = calculate_peters_gw_power(m1_solar, m2_solar, a_au, eccentricity);
        let da_dt = calculate_peters_da_dt(m1_solar, m2_solar, a_au, eccentricity);
        let tau_yr = calculate_gw_coalescence_time(m1_solar, m2_solar, a_au, eccentricity);
        let (_, f_gw_hz) = calculate_gw_frequency(m1_solar + m2_solar, a_au);

        let mu = (m1_solar * m2_solar) / (m1_solar + m2_solar).max(1e-9);
        let chirp_mass = mu.powf(0.6) * (m1_solar + m2_solar).powf(0.4);
        let strain = calculate_gw_strain(chirp_mass, f_gw_hz, 10.0);

        Self {
            precession_rate_arcsec_century: rate_cy,
            precession_advance_per_orbit_rad: advance_rad,
            gw_luminosity_watts: power_watts,
            gw_strain: strain,
            gw_frequency_hz: f_gw_hz,
            inspiral_timescale_yr: tau_yr,
            orbital_decay_rate_au_per_myr: da_dt * 1e6,
            accumulated_precession_rad: 0.0,
            is_coalescing: false,
            semi_major_axis_au: a_au,
            eccentricity,
        }
    }
}

/// Global configuration resource for General Relativistic physics and Gravitational Wave dissipation.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct RelativityConfig {
    /// Enable Post-Newtonian (1PN) relativistic periastron advance corrections.
    pub enable_1pn_precession: bool,
    /// Enable Peters (1964) gravitational wave quadrupole radiation tracking.
    pub enable_gw_radiation: bool,
    /// Enable secular orbital semi-major axis and eccentricity decay from gravitational radiation.
    pub enable_inspiral_decay: bool,
    /// Interactive time-scale multiplier for gravitational wave inspiral decay (default 1.0).
    pub time_scale: f64,
}

impl Default for RelativityConfig {
    fn default() -> Self {
        Self {
            enable_1pn_precession: true,
            enable_gw_radiation: true,
            enable_inspiral_decay: true,
            time_scale: 1.0,
        }
    }
}

/// Event emitted when two compact bodies inspiral past ISCO or contact radius and merge.
#[derive(Event, Message, Debug, Clone)]
pub struct GravitationalWaveMergerEvent {
    /// Entity ID of the primary compact body.
    pub primary_entity: Entity,
    /// Entity ID of the companion compact body that was coalesced.
    pub companion_entity: Entity,
    /// Resulting entity of the merged remnant body.
    pub merged_entity: Entity,
    /// Spatial position of the merger event in AU.
    pub position: DVec3,
    /// Total final mass of the merged remnant in Solar Masses (accounting for ~5% radiated GW energy).
    pub remnant_mass_solar: f64,
    /// Total equivalent mass radiated away as gravitational waves ($E = \Delta M c^2$) in Solar Masses.
    pub radiated_gw_mass_solar: f64,
    /// Peak gravitational wave chirp frequency at ISCO in Hertz.
    pub peak_gw_frequency_hz: f64,
    /// Classified body type of the resulting remnant (e.g. `BlackHole`, `Magnetar`).
    pub remnant_type: BodyType,
}
