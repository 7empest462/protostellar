//! Data types, components, and events for pulsar glitches, magnetar starquakes,
//! and relativistic Alfvén shock blast waves.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Physical state of a rapidly spinning pulsar undergoing magnetic dipole spin-down,
/// superfluid neutron vortex pinning lag, and catastrophic unpinning glitch jumps.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct PulsarGlitchState {
    /// Current rotational spin frequency in Hertz ($\nu = 1/P$).
    pub spin_frequency_hz: f64,
    /// Dipole braking spin-down rate in $\text{Hz/s}$ ($\dot{\nu} < 0$).
    pub spin_down_rate_hz_s: f64,
    /// Superfluid vortex angular velocity lag relative to crust ($\delta \nu = \nu_{\text{sf}} - \nu_{\text{crust}}$).
    pub vortex_pinning_lag_hz: f64,
    /// Critical Magnus force threshold for catastrophic vortex unpinning in Hertz.
    pub unpinning_threshold_hz: f64,
    /// Fractional jump magnitude during unpinning ($\Delta \nu / \nu \approx 10^{-9}\text{--}10^{-5}$).
    pub glitch_jump_fraction: f64,
    /// Post-glitch exponential recovery timescale in days ($\tau_{\text{relax}} \sim 10\text{--}100\text{ d}$).
    pub post_glitch_relaxation_time_days: f64,
    /// Post-glitch healing / recovery parameter $Q \in [0.1, 0.8]$ (fraction of jump recovered).
    pub relaxation_healing_q: f64,
    /// Elapsed time since the most recent glitch in days.
    pub time_since_glitch_days: f64,
    /// Simulation year timestamp of the most recent glitch event.
    pub last_glitch_epoch_years: f64,
    /// Total cumulative number of glitch events observed on this pulsar.
    pub total_glitches_occurred: u32,
    /// Frequency jump magnitude $\Delta \nu$ of the last glitch in Hertz.
    pub last_glitch_jump_hz: f64,
    /// Whether the pulsar is actively in post-glitch exponential relaxation.
    pub is_relaxing: bool,
}

impl Default for PulsarGlitchState {
    fn default() -> Self {
        Self {
            spin_frequency_hz: 160.77, // Lich PSR B1257+12 (P = 6.22 ms)
            spin_down_rate_hz_s: -8.6e-15,
            vortex_pinning_lag_hz: 0.0,
            unpinning_threshold_hz: 1.5e-4,
            glitch_jump_fraction: 1.2e-6,
            post_glitch_relaxation_time_days: 45.0,
            relaxation_healing_q: 0.35,
            time_since_glitch_days: 1000.0,
            last_glitch_epoch_years: 0.0,
            total_glitches_occurred: 0,
            last_glitch_jump_hz: 0.0,
            is_relaxing: false,
        }
    }
}

/// Physical state of an ultra-magnetized magnetar ($B \sim 10^{14}\text{--}10^{15}\text{ G}$)
/// accumulating magnetic shear strain leading to crustal starquakes and giant flares.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct MagnetarCrustStress {
    /// Surface dipole magnetic field strength in Gauss ($B \sim 10^{14}\text{--}10^{15}\text{ G}$).
    pub magnetic_field_gauss: f64,
    /// Dimensionless accumulated shear strain in the crystalline iron/neutron crust ($\theta \in [0, 0.12]$).
    pub accumulated_magnetic_strain: f64,
    /// Maximum elastic shear yield threshold ($\theta_{\text{yield}} \approx 0.10$).
    pub crust_yield_strain: f64,
    /// Annual strain accumulation rate from internal toroidal flux tube unwinding.
    pub magnetic_twist_rate_per_year: f64,
    /// Total mechanical/magnetic shear energy released in the last starquake in ergs ($10^{42}\text{--}10^{46}\text{ erg}$).
    pub last_starquake_energy_erg: f64,
    /// Peak electromagnetic flare luminosity during starquake in Solar luminosities ($L_\odot$).
    pub peak_flare_luminosity_solar: f64,
    /// Whether an active crust rupture / giant burst flare is occurring.
    pub burst_active: bool,
    /// Cooldown / decay timer remaining for active flare in simulation years.
    pub burst_cooldown_years: f64,
    /// Cumulative count of starquakes and giant flares on this magnetar.
    pub total_starquakes_count: u32,
}

impl Default for MagnetarCrustStress {
    fn default() -> Self {
        Self {
            magnetic_field_gauss: 1.0e15, // SGR 1806-20
            accumulated_magnetic_strain: 0.02,
            crust_yield_strain: 0.10,
            magnetic_twist_rate_per_year: 0.015,
            last_starquake_energy_erg: 0.0,
            peak_flare_luminosity_solar: 0.0,
            burst_active: false,
            burst_cooldown_years: 0.0,
            total_starquakes_count: 0,
        }
    }
}

/// Expanding relativistic shear Alfvén blast wave shock front launched by a magnetar starquake.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct AlfvenBlastWave {
    /// Entity ID of the host magnetar that launched the wave.
    pub source_entity: Entity,
    /// 3D origin position in Astronomical Units (AU).
    pub origin_au: DVec3,
    /// Current outer spherical shock radius in Astronomical Units (AU).
    pub radius_au: f64,
    /// Relativistic wave propagation speed in Astronomical Units per year ($v_A \approx c \approx 63{,}241\text{ AU/yr}$).
    pub expansion_speed_au_yr: f64,
    /// Total radiated wave packet energy in ergs.
    pub energy_erg: f64,
    /// Current optical/gamma-ray luminosity in Solar units ($L_\odot$).
    pub luminosity_solar: f64,
    /// Normalized synchrotron radiance factor for rendering.
    pub synchrotron_brightness: f32,
    /// Elapsed wave age in simulation seconds.
    pub age_seconds: f64,
    /// Maximum dissipation radius where the shock dissolves into the interstellar medium (AU).
    pub max_radius_au: f64,
}

/// Event dispatched when a pulsar suffers an instantaneous rotational glitch.
#[derive(Event, Message, Debug, Clone)]
pub struct PulsarGlitchEvent {
    /// Entity ID of the glitching pulsar.
    pub entity: Entity,
    /// Spin frequency before the glitch in Hertz.
    pub frequency_before_hz: f64,
    /// Spin frequency immediately after unpinning in Hertz.
    pub frequency_after_hz: f64,
    /// Instantaneous frequency jump magnitude $\Delta \nu$ in Hertz.
    pub frequency_jump_hz: f64,
}

/// Event dispatched when a magnetar crust fractures, releasing a giant flare and Alfvén wave.
#[derive(Event, Message, Debug, Clone)]
pub struct MagnetarStarquakeEvent {
    /// Entity ID of the ruptured magnetar.
    pub entity: Entity,
    /// Mechanical and magnetic shear energy released in ergs.
    pub energy_erg: f64,
    /// Peak flare luminosity in Solar units ($L_\odot$).
    pub peak_luminosity_solar: f64,
    /// 3D epicenter location in AU.
    pub epicenter_au: DVec3,
}

/// Message requesting a manual starquake or glitch trigger on the selected compact remnant.
#[derive(Event, Message, Debug, Clone, Default)]
pub struct TriggerManualGlitchOrStarquakeEvent {
    /// Optional specific entity to trigger; if None, targets the currently selected body.
    pub target_entity: Option<Entity>,
}
