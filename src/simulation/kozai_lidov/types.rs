//! Core types, components, configuration, and event structures for the Kozai-Lidov secular resonance mechanism.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The critical Kozai inclination angle: $\arccos(\sqrt{3/5}) \approx 39.2315^\circ$.
pub const KOZAI_CRITICAL_ANGLE_DEG: f64 = 39.231_520_4;

/// The critical Kozai inclination angle in radians: $\arccos(\sqrt{3/5}) \approx 0.684719$ rad.
pub const KOZAI_CRITICAL_ANGLE_RAD: f64 = 0.684_719_283;

/// Dynamical regime of the secular Kozai-Lidov resonance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum KozaiRegime {
    /// Mutual inclination is below the critical threshold ($i_{\text{mut}} \le 39.23^\circ$ or $i_{\text{mut}} \ge 140.77^\circ$).
    #[default]
    Inactive,
    /// Eccentricity and inclination oscillate while the argument of periapsis $\omega$ circulates through $360^\circ$.
    Circulation,
    /// High-inclination regime where the argument of periapsis $\omega$ librates around $90^\circ$ or $270^\circ$.
    Libration,
    /// Kozai eccentricity pumping is suppressed/quenched by rapid General Relativistic 1PN periastron precession.
    SuppressedByGR,
    /// Extreme eccentricity pumping brings periapsis inside the stellar Roche limit or stellar radius.
    TidalDisruptionRisk,
}

impl KozaiRegime {
    /// Human-readable label for UI telemetry and inspection.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Inactive => "Inactive",
            Self::Circulation => "Circulation",
            Self::Libration => "Libration",
            Self::SuppressedByGR => "GR Suppressed",
            Self::TidalDisruptionRisk => "Roche Danger",
        }
    }
}

/// Component tracking hierarchical triple Kozai-Lidov secular state and orbital forecasts.
#[derive(Component, Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct KozaiLidovState {
    /// Entity ID of the dominant outer perturber driving the secular torque (if alive).
    #[serde(skip)]
    pub perturber_entity: Option<Entity>,
    /// Name of the perturber body (persisted for save/load resolution).
    pub perturber_name: String,
    /// Current mutual inclination between inner and outer orbital planes in degrees ($0^\circ$ to $180^\circ$).
    pub mutual_inclination_deg: f64,
    /// Critical Kozai inclination angle in degrees ($\approx 39.23^\circ$).
    pub critical_inclination_deg: f64,
    /// Whether the mutual inclination is within the active resonance window ($i_{\text{crit}} < i < 180^\circ - i_{\text{crit}}$).
    pub is_in_resonance: bool,
    /// Theoretical maximum eccentricity achievable via secular pumping $e_{\max} = \sqrt{1 - \frac{5}{3}\cos^2(i_0)}$.
    pub max_eccentricity_forecast: f64,
    /// Minimum periastron distance at peak eccentricity $q_{\min} = a(1 - e_{\max})$ in AU.
    pub min_periastron_au: f64,
    /// Characteristic secular Kozai-Lidov cycle timescale $\tau_{\text{KL}}$ in Earth years.
    pub kozai_period_years: f64,
    /// Ratio of Kozai timescale to 1PN relativistic precession timescale $\tau_{\text{KL}} / \tau_{\text{GR}}$.
    pub gr_precession_ratio: f64,
    /// Whether GR precession is actively suppressing or detuning the resonance.
    pub is_gr_suppressed: bool,
    /// Current secular dynamical regime.
    pub regime: KozaiRegime,
    /// Accumulated secular cycle phase (0.0 to 1.0).
    pub cycle_phase: f64,
}

impl Default for KozaiLidovState {
    fn default() -> Self {
        Self {
            perturber_entity: None,
            perturber_name: String::new(),
            mutual_inclination_deg: 0.0,
            critical_inclination_deg: KOZAI_CRITICAL_ANGLE_DEG,
            is_in_resonance: false,
            max_eccentricity_forecast: 0.0,
            min_periastron_au: 1.0,
            kozai_period_years: f64::INFINITY,
            gr_precession_ratio: 0.0,
            is_gr_suppressed: false,
            regime: KozaiRegime::Inactive,
            cycle_phase: 0.0,
        }
    }
}

/// Global simulation configuration resource for Kozai-Lidov secular dynamics.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct KozaiLidovConfig {
    /// Master toggle for hierarchical Kozai-Lidov secular resonance calculations.
    pub enable_kozai_lidov: bool,
    /// Master toggle for General Relativistic 1PN resonance suppression / detuning.
    pub enable_gr_suppression: bool,
    /// Master toggle for tidal friction coupling at high-eccentricity periastron passages.
    pub enable_tidal_circularization: bool,
    /// Interactive time acceleration multiplier for sandbox exploration (default: 1.0).
    pub secular_time_scale: f64,
}

impl Default for KozaiLidovConfig {
    fn default() -> Self {
        Self {
            enable_kozai_lidov: true,
            enable_gr_suppression: true,
            enable_tidal_circularization: true,
            secular_time_scale: 1.0,
        }
    }
}

/// Event emitted when Kozai-Lidov eccentricity pumping drives a celestial body into Roche disruption or stellar collision.
#[derive(Event, Message, Debug, Clone)]
pub struct KozaiDisruptionEvent {
    /// Entity ID of the disrupted inner body.
    pub disrupted_entity: Entity,
    /// Entity ID of the central star or primary body.
    pub central_entity: Entity,
    /// Entity ID of the outer perturber that drove the resonance.
    pub perturber_entity: Option<Entity>,
    /// Name of the disrupted body.
    pub body_name: String,
    /// Peak orbital eccentricity that caused the disruption.
    pub peak_eccentricity: f64,
    /// Periastron distance at disruption in AU.
    pub periastron_au: f64,
    /// Stellar Roche radius limit in AU.
    pub roche_limit_au: f64,
    /// Position of the disruption event in AU.
    pub position: DVec3,
}
