//! Pulsar Glitches & Magnetar Alfvén Burst Waves.
//!
//! Models superfluid neutron vortex unpinning on pulsars ($\Delta \nu / \nu \sim 10^{-6}$)
//! with exponential post-glitch relaxation ($\tau \sim 10\text{--}100\text{ d}$),
//! magnetar magnetic shear stress accumulation, crust ruptures (starquakes, $10^{44}\text{--}10^{46}\text{ erg}$),
//! and propagating relativistic shear Alfvén shock blast waves ($v_A \approx c$).

pub mod alfven;
pub mod glitches;
pub mod types;

use bevy::prelude::*;

pub use alfven::{
    evaluate_starquake_energy_erg, step_alfven_blast_waves_system,
    step_magnetar_crust_stress_and_starquakes_system, trigger_instantaneous_starquake,
    CRUST_SHEAR_MODULUS_CGS, CRUST_VOLUME_CGS, C_LIGHT_AU_YR,
};
pub use glitches::{
    evaluate_dipole_spin_down_rate, step_pulsar_glitches_system, step_single_pulsar_glitch,
    trigger_instantaneous_glitch, C_LIGHT_CGS, NEUTRON_STAR_INERTIA_CGS,
};
pub use types::{
    AlfvenBlastWave, MagnetarCrustStress, MagnetarStarquakeEvent, PulsarGlitchEvent,
    PulsarGlitchState, TriggerManualGlitchOrStarquakeEvent,
};

use crate::simulation::physics::step_physics_simulation;

/// Plugin registering pulsar glitch dynamics, magnetar starquakes, and relativistic Alfvén blast waves.
pub struct NeutronStarPlugin;

impl Plugin for NeutronStarPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PulsarGlitchEvent>()
            .add_message::<MagnetarStarquakeEvent>()
            .add_message::<TriggerManualGlitchOrStarquakeEvent>()
            .add_systems(
                Update,
                (
                    step_pulsar_glitches_system.after(step_physics_simulation),
                    step_magnetar_crust_stress_and_starquakes_system.after(step_physics_simulation),
                    step_alfven_blast_waves_system
                        .after(step_magnetar_crust_stress_and_starquakes_system),
                ),
            );
    }
}
