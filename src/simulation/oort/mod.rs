//! Distant Oort Cloud Cometary Reservoir & Galactic Tide Perturbations.
//!
//! Models the outer quasi-spherical icy planetesimal reservoir ($a \sim 200\text{--}25{,}000\text{ AU}$),
//! Galactic tidal torques ($\vec{a}_{\text{tide}} = -\Omega_{\text{tide}}^2 z \hat{n}_{\text{gal}}$),
//! Kozai secular perihelion dropping into the inner solar system, and cometary volatile sublimation.

pub mod physics;
pub mod spawner;
pub mod types;

use bevy::prelude::*;

pub use physics::{
    apply_stellar_flyby_perturbation, compute_galactic_omega_sq,
    evaluate_galactic_tide_acceleration, evaluate_galactic_tide_secular_de,
    step_oort_galactic_tides_system, PARSEC_TO_AU,
};
pub use spawner::{
    handle_spawn_oort_cloud_swarm_system, keplerian_elements_to_cartesian,
    solve_kepler_eccentric_anomaly, spawn_oort_cloud_swarm,
};
pub use types::{
    GalacticTideState, OortCloudConfig, OortComet, OortCometInfallEvent, TriggerOortCloudSwarmEvent,
};

use crate::simulation::physics::step_physics_simulation;

/// Plugin registering the Oort Cloud cometary reservoir and Galactic tide systems.
pub struct OortPlugin;

impl Plugin for OortPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OortCloudConfig>()
            .init_resource::<GalacticTideState>()
            .add_message::<OortCometInfallEvent>()
            .add_message::<TriggerOortCloudSwarmEvent>()
            .add_systems(
                Update,
                (
                    step_oort_galactic_tides_system.after(step_physics_simulation),
                    handle_spawn_oort_cloud_swarm_system,
                ),
            );
    }
}
