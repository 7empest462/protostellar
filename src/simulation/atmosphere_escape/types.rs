//! Type definitions, ECS components, resources, and events for atmospheric escape,
//! photoevaporation, and stellar wind stripping.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Dominant physical mechanism driving atmospheric or volatile mass loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AtmosphericEscapeRegime {
    /// Stable, gravitationally bound atmosphere shielded by a planetary geodynamo.
    #[default]
    None,
    /// Kinetic thermal escape of light volatile species ($\text{H}_2, \text{He}$).
    JeansThermal,
    /// Non-thermal ion pick-up and sputtering on unshielded magnetopauses.
    SolarWindErosion,
    /// Supersonic hydrodynamic Parker-type planetary wind driven by stellar XUV irradiation.
    HydrodynamicPhotoevaporation,
    /// Tidal stripping where the upper thermosphere expands beyond the Hill sphere.
    RocheLobeOverflow,
    /// Solar radiation sublimation of volatile water, $\text{CO}$, and methane ices.
    CometaryOutgassing,
}

impl AtmosphericEscapeRegime {
    /// Returns a short user-friendly label for telemetry and HUD readouts.
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "Stable / Shielded",
            Self::JeansThermal => "Jeans Thermal",
            Self::SolarWindErosion => "Solar Wind Stripping",
            Self::HydrodynamicPhotoevaporation => "Hydrodynamic Photoevaporation",
            Self::RocheLobeOverflow => "Roche Lobe Overflow",
            Self::CometaryOutgassing => "Cometary Sublimation",
        }
    }
}

/// Dynamic atmospheric escape, photoevaporative mass loss, and magnetospheric shielding state.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AtmosphericEscapeState {
    /// Hydrodynamic energy-limited photoevaporative mass loss rate in Earth masses per million years ($M_\oplus/\text{Myr}$).
    pub photoevaporative_loss_rate_m_earth_per_myr: f64,
    /// Stellar wind dynamic ram pressure stripping rate ($M_\oplus/\text{Myr}$).
    pub solar_wind_stripping_rate_m_earth_per_myr: f64,
    /// Thermal Jeans escape mass loss rate ($M_\oplus/\text{Myr}$).
    pub jeans_escape_rate_m_earth_per_myr: f64,
    /// Total aggregate atmospheric mass loss rate ($M_\oplus/\text{Myr}$).
    pub total_loss_rate_m_earth_per_myr: f64,
    /// Magnetopause stand-off boundary radius in Astronomical Units ($\text{AU}$).
    pub magnetopause_radius_au: f64,
    /// Geodynamo magnetic shielding effectiveness fraction ($0.0 = $ fully unshielded, $1.0 = 100\%$ shielded).
    pub magnetic_shielding_factor: f32,
    /// Incident extreme ultraviolet (EUV) and X-ray flux in $\text{W/m}^2$.
    pub xuv_flux_w_m2: f64,
    /// Stellar wind dynamic ram pressure $P_{\text{ram}} = \rho_{\text{sw}} v_{\text{sw}}^2$ in $\text{N/m}^2$.
    pub solar_wind_pressure_n_m2: f64,
    /// Thermospheric expansion ratio relative to the planetary Hill sphere ($R_{\text{XUV}} / R_{\text{Hill}}$).
    pub roche_lobe_fill_fraction: f32,
    /// Cumulative volatile envelope mass lost since formation in Earth masses ($M_\oplus$).
    pub cumulative_mass_lost_m_earth: f64,
    /// Active dominant atmospheric escape regime.
    pub escape_regime: AtmosphericEscapeRegime,
}

impl Default for AtmosphericEscapeState {
    fn default() -> Self {
        Self {
            photoevaporative_loss_rate_m_earth_per_myr: 0.0,
            solar_wind_stripping_rate_m_earth_per_myr: 0.0,
            jeans_escape_rate_m_earth_per_myr: 0.0,
            total_loss_rate_m_earth_per_myr: 0.0,
            magnetopause_radius_au: 0.0,
            magnetic_shielding_factor: 1.0,
            xuv_flux_w_m2: 0.0,
            solar_wind_pressure_n_m2: 0.0,
            roche_lobe_fill_fraction: 0.0,
            cumulative_mass_lost_m_earth: 0.0,
            escape_regime: AtmosphericEscapeRegime::None,
        }
    }
}

/// Global simulation configuration for atmospheric escape, photoevaporation, and solar wind stripping.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct AtmosphericEscapeConfig {
    /// Enables or disables hydrodynamic XUV photoevaporative mass loss.
    pub enable_photoevaporation: bool,
    /// Enables or disables stellar wind ram pressure stripping.
    pub enable_solar_wind_stripping: bool,
    /// Enables or disables thermal Jeans escape of volatile gases.
    pub enable_jeans_escape: bool,
    /// Photoevaporative hydrodynamic conversion efficiency $\eta$ (typically 0.10 to 0.25).
    pub photo_efficiency_eta: f64,
    /// Stellar wind stripping efficiency factor (typically 0.05 to 0.20).
    pub wind_efficiency: f64,
    /// Sandbox time acceleration multiplier for observing geological escape epochs in real time.
    pub time_scale: f64,
}

impl Default for AtmosphericEscapeConfig {
    fn default() -> Self {
        Self {
            enable_photoevaporation: true,
            enable_solar_wind_stripping: true,
            enable_jeans_escape: true,
            photo_efficiency_eta: 0.15,
            wind_efficiency: 0.10,
            time_scale: 1.0,
        }
    }
}

/// Emitted when a volatile or gas-rich envelope is completely stripped away by photoevaporation or stellar wind,
/// transitioning the world into a bare chthonian rocky super-Earth core (Fulton Gap / Radius Valley transition).
#[derive(Event, Message, Debug, Clone, PartialEq)]
pub struct AtmosphericStrippedEvent {
    /// Entity of the stripped planetary body.
    pub planet: Entity,
    /// Name of the planet.
    pub name: String,
    /// Remaining bare rocky/metal mass in Earth masses ($M_\oplus$).
    pub remaining_mass_earth: f64,
    /// Simulation timestamp in years when stripping completed.
    pub time_yr: f64,
}
