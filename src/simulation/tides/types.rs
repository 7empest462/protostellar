//! Types, components, and configuration resources for tidal dissipation,
//! spin-orbit synchronization, and circularization.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::simulation::components::{BodyType, Composition};

/// Component tracking gravitational tidal interactions, orbital circularization,
/// spin-orbit synchronization, and internal viscoelastic tidal heating.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TidalState {
    /// Host celestial body (star or parent planet) generating the primary tidal field.
    pub host_entity: Option<Entity>,
    /// Tidal Love number $k_2$ (elastic deformability, ~0.3 for rocky, ~0.5 for fluid giants).
    pub love_number_k2: f64,
    /// Specific tidal dissipation factor $Q$ (~10-100 for rocky/oceanic, ~10^4-10^5 for gas).
    pub tidal_q: f64,
    /// Whether the body is tidally locked to its host in synchronous or resonant equilibrium.
    pub is_tidally_locked: bool,
    /// Synchronization progress towards equilibrium rotation (0.0 = asynchronous, 1.0 = locked).
    pub locking_progress: f32,
    /// Spin-orbit resonance ratio $\omega_{\text{spin}} / n_{\text{orbit}}$ (1.0 = 1:1 synchronous, 1.5 = 3:2 resonance).
    pub resonance_ratio: f32,
    /// Internal viscoelastic tidal dissipation power in Watts.
    pub tidal_heating_power_watts: f64,
    /// Tidal surface heat flux in $\text{W/m}^2$ (Earth geothermal ~0.09 $\text{W/m}^2$, Io ~2.5 $\text{W/m}^2$).
    pub tidal_heating_flux_w_m2: f64,
    /// Orbital eccentricity circularization rate $de/dt$ in units of $\text{Myr}^{-1}$.
    pub circularization_rate_per_myr: f64,
    /// Theoretical orbital circularization timescale in years ($\tau_{\text{circ}} = e / |de/dt|$).
    pub circularization_timescale_yr: f64,
    /// Theoretical spin-orbit synchronization timescale in years ($\tau_{\text{sync}}$).
    pub sync_timescale_yr: f64,
}

impl Default for TidalState {
    fn default() -> Self {
        Self::new_rocky()
    }
}

impl TidalState {
    /// Creates a tidal state with custom Love number $k_2$ and dissipation factor $Q$.
    pub fn new(love_number_k2: f64, tidal_q: f64) -> Self {
        Self {
            host_entity: None,
            love_number_k2: love_number_k2.clamp(0.01, 1.5),
            tidal_q: tidal_q.clamp(1.0, 1e7),
            is_tidally_locked: false,
            locking_progress: 0.0,
            resonance_ratio: 1.0,
            tidal_heating_power_watts: 0.0,
            tidal_heating_flux_w_m2: 0.0,
            circularization_rate_per_myr: 0.0,
            circularization_timescale_yr: 1e9,
            sync_timescale_yr: 1e7,
        }
    }

    /// Rocky terrestrial body defaults ($k_2 \approx 0.30$, $Q \approx 100$).
    pub fn new_rocky() -> Self {
        Self::new(0.30, 100.0)
    }

    /// Icy moon or dwarf planet with subsurface ocean ($k_2 \approx 0.25$, $Q \approx 40$).
    pub fn new_icy() -> Self {
        Self::new(0.25, 40.0)
    }

    /// Fluid gas giant or ice giant ($k_2 \approx 0.50$, $Q \approx 50,000$).
    pub fn new_gas_giant() -> Self {
        Self::new(0.50, 50_000.0)
    }

    /// Creates an already tidally locked body with a specific resonance ratio.
    pub fn new_locked(resonance_ratio: f32) -> Self {
        let mut state = Self::new_rocky();
        state.is_tidally_locked = true;
        state.locking_progress = 1.0;
        state.resonance_ratio = resonance_ratio;
        state
    }

    /// Determines appropriate tidal parameters based on composition and body type.
    pub fn new_from_composition_and_type(comp: &Composition, body_type: BodyType) -> Self {
        match body_type {
            BodyType::GasGiant | BodyType::IceGiant => Self::new_gas_giant(),
            BodyType::Moon | BodyType::Asteroid if comp.ice_frac > 0.30 => Self::new_icy(),
            _ => {
                let k2 = if comp.ice_frac > 0.40 { 0.26 } else { 0.30 };
                let q = if comp.ice_frac > 0.40 { 50.0 } else { 100.0 };
                Self::new(k2, q)
            }
        }
    }

    /// Ratio $k_2 / Q$ determining the efficiency of tidal energy dissipation.
    pub fn dissipation_factor(&self) -> f64 {
        if self.tidal_q > 0.0 {
            self.love_number_k2 / self.tidal_q
        } else {
            0.0
        }
    }
}

/// Global simulation configuration for gravitational tidal physics.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TidalConfig {
    /// Whether gravitational tidal dynamics are actively calculated.
    pub is_enabled: bool,
    /// Acceleration multiplier for tidal timescales so effects can be observed interactively.
    pub time_scale: f64,
    /// Whether orbital eccentricity circularization is applied.
    pub enable_circularization: bool,
    /// Whether spin-orbit synchronization torque is applied.
    pub enable_spin_synchronization: bool,
    /// Whether internal viscoelastic tidal heating is calculated and injected into thermodynamics.
    pub enable_heating: bool,
}

impl Default for TidalConfig {
    fn default() -> Self {
        Self {
            is_enabled: true,
            // 20,000x acceleration: a 20 Myr circularization process unfolds over ~1,000 simulated years.
            time_scale: 20_000.0,
            enable_circularization: true,
            enable_spin_synchronization: true,
            enable_heating: true,
        }
    }
}
