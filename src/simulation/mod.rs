//! Core Simulation Plugin bundling physics, disk generation, accretion, and thermodynamics.

pub mod accretion;
pub mod components;
pub mod disk;
pub mod physics;
pub mod resources;
pub mod scenarios;
pub mod thermodynamics;

use bevy::prelude::*;

use crate::simulation::accretion::*;
use crate::simulation::components::*;
use crate::simulation::disk::*;
use crate::simulation::physics::*;
use crate::simulation::resources::*;
use crate::simulation::scenarios::*;
use crate::simulation::thermodynamics::*;

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimulationConfig>()
            .init_resource::<TimeWarp>()
            .init_resource::<SimTime>()
            .init_resource::<EnergyMonitor>()
            .init_resource::<DiskParameters>()
            .init_resource::<PlayerInteractionState>()
            .init_resource::<PlanetesimalSpawner>()
            .init_resource::<ActiveScenarioState>()
            .add_message::<AccretionMergeEvent>()
            .add_message::<MoonFormationEvent>()
            .add_message::<CollisionBounceEvent>()
            .add_message::<RocheDisruptionEvent>()
            .add_message::<StarIgnitionEvent>()
            .add_message::<PlanetaryEngulfmentEvent>()
            .add_message::<SupernovaEvent>()
            .add_message::<LoadScenarioEvent>()
            .add_systems(Startup, setup_simulation)
            .add_systems(
                Update,
                (
                    handle_load_scenario_events,
                    update_active_scenarios,
                    step_physics_simulation,
                    process_accretion_and_collisions.after(step_physics_simulation),
                    direct_nebular_gas_accretion.after(step_physics_simulation),
                    update_thermodynamics.after(step_physics_simulation),
                    update_photoevaporative_escape.after(update_thermodynamics),
                    auto_spawn_planetesimals.after(step_physics_simulation),
                    auto_spawn_delayed_proto_earth.after(step_physics_simulation),
                    update_black_hole_star_dynamics.after(step_physics_simulation),
                    dissipate_gas_disk.after(step_physics_simulation),
                ),
            );
    }
}

fn setup_simulation(
    mut commands: Commands,
    disk_params: Res<DiskParameters>,
    config: Res<SimulationConfig>,
    mut player_state: ResMut<PlayerInteractionState>,
) {
    let star_ent = spawn_protoplanetary_disk(&mut commands, &disk_params, &config);
    player_state.selected_entity = Some(star_ent);
}
