//! Rendering Plugin bundling 3D orbital camera, celestial meshes, materials, and gizmo effects.

pub mod bodies;
pub mod camera;
pub mod effects;
pub mod gas_clouds;
pub mod materials;
pub mod particle_swarm;
pub mod skybox;

use bevy::prelude::*;

use crate::rendering::bodies::*;
use crate::rendering::camera::*;
use crate::rendering::effects::*;
use crate::rendering::gas_clouds::*;
use crate::rendering::materials::*;
use crate::rendering::particle_swarm::*;
use crate::rendering::skybox::*;
use crate::simulation::resources::ImpactShockwavePool;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ImpactShockwavePool>()
            .add_plugins((
                GasCloudPlugin,
                ParticleSwarmPlugin,
                MaterialPlugin::<PlanetMaterial>::default(),
                MaterialPlugin::<RingMaterial>::default(),
            ))
            .add_systems(
                Startup,
                (setup_camera, setup_visual_assets, setup_space_environment),
            )
            .add_systems(
                Update,
                (
                    update_pan_orbit_camera,
                    spawn_missing_visuals,
                    sync_celestial_transforms,
                    sync_planetary_rings,
                    update_impact_shockwaves,
                    draw_orbital_effects_and_gizmos,
                ),
            );
    }
}
