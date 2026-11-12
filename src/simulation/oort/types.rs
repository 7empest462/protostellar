//! Data types, components, and configuration resources for the Oort Cloud cometary reservoir.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Global configuration for the distant Oort Cloud cometary reservoir and Galactic tide perturbations.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct OortCloudConfig {
    /// Whether galactic tides and Oort comet secular evolution are active.
    pub enabled: bool,
    /// Inner boundary of the Hills inner cometary cloud (in AU, typically ~200 AU).
    pub inner_radius_au: f64,
    /// Outer boundary of the spherical outer Oort reservoir (in AU, typically ~25,000 AU).
    pub outer_radius_au: f64,
    /// Local Milky Way midplane mass density $\rho_0$ in $M_\odot / \text{pc}^3$ (canonical ~0.10).
    pub galactic_rho_0: f64,
    /// Direct multiplier on galactic tidal acceleration.
    pub tide_strength_scale: f64,
    /// Acceleration factor for secular Kozai-Lidov oscillations over interactive simulation time.
    pub secular_accel_factor: f64,
    /// Unit vector of the Galactic North Pole in simulation coordinates (ecliptic frame).
    /// Default is inclined $\approx 60.2^\circ$ to the ecliptic pole (-Y prograde normal).
    pub galactic_pole_dir: DVec3,
    /// Distance threshold (in AU) where volatile water/CO2 ices sublimate and spawn coma/tails.
    pub sublimation_distance_au: f64,
    /// Distance threshold (in AU) where a comet trajectory is classified as Sun-grazing.
    pub sungrazer_threshold_au: f64,
    /// Number of minor icy planetesimals spawned in the standard Oort cometary shell.
    pub default_comet_count: usize,
}

impl Default for OortCloudConfig {
    fn default() -> Self {
        // Inclined by ~60.2 degrees from ecliptic normal (-Y).
        // Using unit vector with components along X and -Y.
        let gal_pole = DVec3::new(0.8678, -0.4969, 0.0).normalize();
        Self {
            enabled: true,
            inner_radius_au: 200.0,
            outer_radius_au: 25_000.0,
            galactic_rho_0: 0.10,
            tide_strength_scale: 1.0,
            secular_accel_factor: 100.0,
            galactic_pole_dir: gal_pole,
            sublimation_distance_au: 3.0,
            sungrazer_threshold_au: 0.15,
            default_comet_count: 120,
        }
    }
}

/// Dynamic simulation resource tracking cometary influx demographics and Galactic tide state.
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct GalacticTideState {
    /// Total number of tracked cometary bodies in the Oort reservoir.
    pub total_oort_bodies: usize,
    /// Number of comets currently on an inward plunge trajectory ($q < 3.0\text{ AU}$).
    pub infalling_comets_count: usize,
    /// Number of comets actively sublimating inside the ice line ($r < 3.0\text{ AU}$).
    pub active_sublimating_count: usize,
    /// Number of comets on Sun-grazing trajectories ($q < 0.15\text{ AU}$).
    pub sungrazing_count: usize,
    /// Simulation year timestamp of the most recent stellar flyby encounter.
    pub last_stellar_flyby_sim_years: f64,
    /// Cumulative counter of unique comets diverted into the planetary system by galactic tides.
    pub cumulative_infall_events: u64,
}

/// Component attached to icy bodies residing in or originating from the Oort Cloud.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct OortComet {
    /// Semi-major axis in AU.
    pub semi_major_axis_au: f64,
    /// Orbital eccentricity $e \in [0, 1)$.
    pub eccentricity: f64,
    /// Orbital inclination relative to the central star's disk plane in radians.
    pub inclination_rad: f64,
    /// Argument of periapsis $\omega$ in radians.
    pub argument_of_periapsis_rad: f64,
    /// Original perihelion distance in AU at birth before tidal perturbation.
    pub original_perihelion_au: f64,
    /// Current perihelion distance $q = a(1 - e)$ in AU.
    pub current_perihelion_au: f64,
    /// Mass fraction of volatile ices (water, carbon monoxide, methane).
    pub volatile_ice_fraction: f32,
    /// Dynamic outgassing rate in $\text{kg}/\text{s}$ when within sublimation distance.
    pub outgassing_rate: f32,
    /// True if tidal Kozai torque has lowered perihelion into the inner system ($q < 3.0\text{ AU}$).
    pub is_infall_trajectory: bool,
    /// True if trajectory plunges within $0.15\text{ AU}$ of the central star.
    pub is_sungrazer: bool,
    /// True if the comet is currently inside the volatile sublimation zone ($r < 3.0\text{ AU}$).
    pub is_active_sublimating: bool,
    /// Total cumulative volatile ice mass lost to solar outgassing in kilograms.
    pub cumulative_volatile_loss_kg: f64,
}

impl Default for OortComet {
    fn default() -> Self {
        Self {
            semi_major_axis_au: 5_000.0,
            eccentricity: 0.70,
            inclination_rad: 0.85,
            argument_of_periapsis_rad: 0.0,
            original_perihelion_au: 1_500.0,
            current_perihelion_au: 1_500.0,
            volatile_ice_fraction: 0.85,
            outgassing_rate: 0.0,
            is_infall_trajectory: false,
            is_sungrazer: false,
            is_active_sublimating: false,
            cumulative_volatile_loss_kg: 0.0,
        }
    }
}

/// Event dispatched when an Oort comet's perihelion is first perturbed into the inner system.
#[derive(Event, Message, Debug, Clone)]
pub struct OortCometInfallEvent {
    /// Entity ID of the plunging comet.
    pub entity: Entity,
    /// Perihelion distance in AU.
    pub perihelion_au: f64,
    /// Orbital eccentricity.
    pub eccentricity: f64,
    /// Semi-major axis in AU.
    pub semi_major_axis_au: f64,
    /// Whether the trajectory is Sun-grazing ($q < 0.15\text{ AU}$).
    pub is_sungrazer: bool,
}
