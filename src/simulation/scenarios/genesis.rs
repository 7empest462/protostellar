//! Protoplanetary Disk Genesis Scenario.
//!
//! Spawns a pristine Class II T-Tauri protostellar system without any pre-existing
//! starter planets or embryos. The system begins purely as a central protostar
//! surrounded by a viscous gas and dust disk. Planets form organically via SPH
//! aerodynamic gas drag, vertical midplane settling, cold-finger vapor trapping
//! at the water ice snow line (2.7 AU), electrostatic coagulation, streaming
//! instability collapse into planetesimals, and mutual N-body gravitational mergers.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Spawns the central Class II T-Tauri protostar for the Genesis scenario.
pub fn spawn_genesis_protostar(commands: &mut Commands) -> Entity {
    let mass = 1.33;
    let radius = SOLAR_RADIUS_AU * 2.75;
    let temp = 4800.0;
    let lum = 4.80;

    commands
        .spawn((
            CentralStar,
            CelestialBody {
                body_type: BodyType::YellowDwarf,
                name: "Genesis Protostar (Class II T-Tauri)".to_string(),
            },
            Mass(mass),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(radius),
            Temperature(temp),
            Luminosity(lum),
            AngularMomentum::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 6.8e6,
                fusion_fraction: 0.30,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id()
}

/// Spawns the Protoplanetary Disk Genesis scenario.
///
/// Unlike the canonical MMSN scenario, zero starter planets or embryos are spawned.
/// All worlds form dynamically from particle coagulation and streaming instability.
pub fn spawn_accretion_disk_genesis(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    disk_params.central_star_mass = 1.33;
    disk_params.inner_radius_au = 0.12;
    disk_params.outer_radius_au = 55.0;
    disk_params.snow_line_au = 3.65;
    disk_params.disk_mass = 0.00035;
    disk_params.gas_disk_lifetime_yr = 5.0e6;
    disk_params.reference_temp_1au = 325.0;

    spawn_genesis_protostar(commands)
}
