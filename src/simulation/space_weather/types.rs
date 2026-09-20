//! Space weather, stellar flares, coronal mass ejections (CMEs), and planetary auroral oval types.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Qualitative NOAA-scale geomagnetic storm level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum GeomagneticStormLevel {
    /// Kp 0.0 - 2.0: Deep magnetospheric quiet; compact auroral ring at high latitudes.
    #[default]
    Quiet,
    /// Kp 2.0 - 4.0: Mild solar wind perturbation.
    Unsettled,
    /// Kp 4.0 - 5.0 (G1): Minor geomagnetic storm.
    MinorStorm,
    /// Kp 5.0 - 6.0 (G2): Moderate geomagnetic storm; equatorward oval expansion.
    ModerateStorm,
    /// Kp 6.0 - 7.0 (G3): Strong storm; visible auroras at mid-latitudes.
    StrongStorm,
    /// Kp 7.0 - 8.5 (G4): Severe storm; major ionospheric currents and bright curtains.
    SevereStorm,
    /// Kp 8.5 - 9.0+ (G5): Extreme Carrington-level storm; global geomagnetic disruption.
    ExtremeStorm,
}

impl GeomagneticStormLevel {
    /// Returns the descriptive HUD label for this geomagnetic storm tier.
    pub fn label(self) -> &'static str {
        match self {
            Self::Quiet => "Quiet (Kp < 2)",
            Self::Unsettled => "Unsettled (Kp 2-4)",
            Self::MinorStorm => "G1 Minor Storm (Kp 4-5)",
            Self::ModerateStorm => "G2 Moderate Storm (Kp 5-6)",
            Self::StrongStorm => "G3 Strong Storm (Kp 6-7)",
            Self::SevereStorm => "G4 Severe Storm (Kp 7-8.5)",
            Self::ExtremeStorm => "G5 Extreme Storm (Kp 8.5+)",
        }
    }
}

/// Dynamic coronal flare activity and outward-propagating Coronal Mass Ejections (CMEs) for stars.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StellarFlareState {
    /// Baseline stochastic flare frequency (flares per Earth year).
    pub flare_frequency: f32,
    /// Current ongoing flare amplitude (0.0 = quiescent, 1.0 = standard M/X-class flare, >= 5.0 = CME superflare).
    pub current_flare_intensity: f32,
    /// Flare decay timer in simulation years.
    pub flare_decay_timer_years: f32,
    /// CME plasma shock front radius in AU expanding from the stellar surface.
    pub cme_front_radius_au: f32,
    /// CME shock front propagation speed in AU per day (e.g. 0.15 AU/day ~ 260 km/s to 0.4 AU/day ~ 700 km/s).
    pub cme_speed_au_day: f32,
    /// Solar wind ram pressure amplification multiplier at the shock front (e.g. 5x - 30x).
    pub cme_density_multiplier: f32,
    /// Whether a CME plasma shockwave is currently propagating through the system.
    pub cme_active: bool,
}

impl Default for StellarFlareState {
    fn default() -> Self {
        Self {
            flare_frequency: 1.5,
            current_flare_intensity: 0.0,
            flare_decay_timer_years: 0.0,
            cme_front_radius_au: 0.0,
            cme_speed_au_day: 0.25,
            cme_density_multiplier: 12.0,
            cme_active: false,
        }
    }
}

/// Dynamic planetary magnetosphere footprint and dual-pole auroral oval emission state.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AuroralOvalState {
    /// Magnetopause subsolar standoff radius in planetary radii (R_mp / R_p).
    pub magnetopause_standoff_rp: f32,
    /// Auroral oval angular footprint co-latitude in radians (\theta_A = arcsin(sqrt(R_p / R_mp))).
    pub oval_colatitude_rad: f32,
    /// Gaussian angular thickness / half-width of the auroral oval ring in radians (~0.05 to 0.14 rad, or 3° to 8°).
    pub oval_width_rad: f32,
    /// Auroral emission luminance / radiance factor (driven by solar wind Poynting flux & CME compression).
    pub auroral_intensity: f32,
    /// Planetary geomagnetic activity index (Kp 0.0 to 9.0).
    pub geomagnetic_kp_index: f32,
    /// Qualitative storm tier.
    pub storm_level: GeomagneticStormLevel,
    /// Dynamic animation phase for fluttering Birkeland current curtains.
    pub flutter_phase: f32,
}

impl Default for AuroralOvalState {
    fn default() -> Self {
        Self {
            magnetopause_standoff_rp: 10.5,
            oval_colatitude_rad: 0.315, // ~18.0° co-latitude (72.0° latitude)
            oval_width_rad: 0.065,      // ~3.7° width
            auroral_intensity: 0.85,
            geomagnetic_kp_index: 1.5,
            storm_level: GeomagneticStormLevel::Quiet,
            flutter_phase: 0.0,
        }
    }
}

/// Event dispatched when an expanding CME plasma shockwave impacts a celestial body.
#[derive(Event, Message, Debug, Clone, PartialEq)]
pub struct CmeShockwaveEvent {
    /// Entity of the planet receiving the CME shock front.
    pub target: Entity,
    /// Distance of target from the flaring star in AU.
    pub orbital_radius_au: f32,
    /// CME shockwave density amplification.
    pub density_multiplier: f32,
    /// Flare intensity at source.
    pub flare_intensity: f32,
}
