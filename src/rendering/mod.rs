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
use crate::simulation::resources::{ImpactShockwavePool, RocheDebrisPool};

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ImpactShockwavePool>()
            .init_resource::<RocheDebrisPool>()
            .add_plugins((
                GasCloudPlugin,
                ParticleSwarmPlugin,
                MaterialPlugin::<PlanetMaterial>::default(),
                MaterialPlugin::<RingMaterial>::default(),
                MaterialPlugin::<SkyboxMaterial>::default(),
            ))
            .add_systems(
                Startup,
                (setup_camera, setup_visual_assets, setup_space_environment),
            )
            .add_systems(
                Update,
                (
                    sync_celestial_transforms
                        .after(crate::simulation::physics::step_physics_simulation),
                    update_pan_orbit_camera.after(sync_celestial_transforms),
                    (sync_skybox_to_camera, update_skybox_uniforms).after(update_pan_orbit_camera),
                    draw_orbital_effects_and_gizmos.after(update_pan_orbit_camera),
                    spawn_missing_visuals,
                    sync_planetary_rings.after(sync_celestial_transforms),
                    sync_quasar_beams.after(sync_celestial_transforms),
                    sync_pulsar_beams.after(sync_celestial_transforms),
                    sync_magnetar_structures.after(sync_celestial_transforms),
                    update_impact_shockwaves
                        .after(crate::simulation::physics::step_physics_simulation),
                    update_roche_debris_streams
                        .after(crate::simulation::physics::step_physics_simulation),
                ),
            );
    }
}
