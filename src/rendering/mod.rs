//! Rendering Plugin bundling 3D orbital camera, celestial meshes, materials, and gizmo effects.

pub mod bodies;
pub mod camera;
pub(crate) mod camera_controls;
pub(crate) mod camera_picking;
pub mod effects;
pub mod gas_clouds;
pub mod materials;
pub mod origin;
pub mod particle_swarm;
pub mod post_processing;
pub mod shaders;
pub mod skybox;
pub mod surface_camera;
pub mod surface_terrain;
pub mod surface_vegetation;
pub mod volumetric_nebula;

use bevy::prelude::*;

use crate::rendering::bodies::*;
use crate::rendering::camera::*;
use crate::rendering::effects::*;
use crate::rendering::gas_clouds::*;
use crate::rendering::materials::*;
use crate::rendering::origin::*;
use crate::rendering::particle_swarm::*;
use crate::rendering::post_processing::*;
use crate::rendering::skybox::*;
use crate::simulation::resources::{ImpactShockwavePool, RocheDebrisPool, TidalDisruptionPool};

fn configure_gpu_clustering(mut settings: ResMut<bevy_light::cluster::GlobalClusterSettings>) {
    if let Some(ref mut gpu) = settings.gpu_clustering {
        gpu.initial_z_slice_list_capacity = 65536;
        gpu.initial_index_list_capacity = 8_388_608;
    }
}

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, configure_gpu_clustering)
            .init_resource::<ImpactShockwavePool>()
            .init_resource::<crate::simulation::resources::ImpactEjectaPool>()
            .init_resource::<RocheDebrisPool>()
            .init_resource::<SupernovaDebrisPool>()
            .init_resource::<PersistentRemnantPool>()
            .init_resource::<KilonovaPool>()
            .init_resource::<TidalDisruptionPool>()
            .init_resource::<RenderOrigin>()
            .init_resource::<surface_terrain::SurfaceTerrainState>()
            .add_plugins(bevy::render::extract_resource::ExtractResourcePlugin::<
                RenderOrigin,
            >::default())
            .add_plugins((
                shaders::ProtostellarShadersPlugin,
                GasCloudPlugin,
                ParticleSwarmPlugin,
                MaterialPlugin::<PlanetMaterial>::default(),
                MaterialPlugin::<AtmosphereMaterial>::default(),
                MaterialPlugin::<RingMaterial>::default(),
                MaterialPlugin::<SkyboxMaterial>::default(),
                MaterialPlugin::<RelativisticJetMaterial>::default(),
                MaterialPlugin::<CometTailMaterial>::default(),
                MaterialPlugin::<SupernovaMaterial>::default(),
                MaterialPlugin::<BlackHoleDiskMaterial>::default(),
                MaterialPlugin::<RocheStreamMaterial>::default(),
                MaterialPlugin::<VolumetricNebulaMaterial>::default(),
                MaterialPlugin::<CircumstellarMaterial>::default(),
                PostProcessingPlugin,
            ))
            .add_plugins(MaterialPlugin::<particle_swarm::ParticleSwarmMaterial>::default())
            .add_plugins((
                MaterialPlugin::<MegastructureMaterial>::default(),
                MaterialPlugin::<PlanetaryShieldMaterial>::default(),
            ))
            .add_systems(
                Startup,
                (
                    setup_camera,
                    setup_gizmo_configuration,
                    setup_visual_assets,
                    setup_space_environment,
                ),
            )
            .add_systems(
                Update,
                (
                    update_render_origin
                        .after(crate::simulation::physics::update_simulation_visual_time)
                        .after(crate::simulation::physics::step_physics_simulation)
                        .after(crate::simulation::thermodynamics::update_thermodynamics)
                        .after(crate::simulation::geology::sync_geological_evolution_system)
                        .before(sync_celestial_transforms),
                    sync_celestial_transforms
                        .after(update_render_origin)
                        .before(crate::simulation::probes::navigation::probe_navigation_system),
                    sync_selected_body_mesh_lod.after(sync_celestial_transforms),
                    update_pan_orbit_camera
                        .after(sync_celestial_transforms)
                        .after(crate::simulation::probes::sync_probe_transforms),
                    surface_camera::update_surface_horizon_camera.after(update_pan_orbit_camera),
                    spawn_missing_visuals.after(update_render_origin),
                    sync_camera_adaptive_body_scales
                        .after(surface_camera::update_surface_horizon_camera)
                        .after(spawn_missing_visuals),
                    surface_terrain::sync_surface_terrain_system
                        .after(sync_camera_adaptive_body_scales),
                    (sync_skybox_to_camera, update_skybox_uniforms)
                        .after(surface_camera::update_surface_horizon_camera),
                    sync_planetary_atmospheres.after(sync_camera_adaptive_body_scales),
                ),
            )
            .add_systems(
                Update,
                (
                    sync_circumstellar_environments.after(sync_camera_adaptive_body_scales),
                    sync_planetary_rings.after(sync_camera_adaptive_body_scales),
                    sync_black_hole_accretion_disks.after(sync_camera_adaptive_body_scales),
                    sync_cometary_tails.after(sync_camera_adaptive_body_scales),
                    sync_quasar_beams.after(sync_camera_adaptive_body_scales),
                    sync_pulsar_beams.after(sync_camera_adaptive_body_scales),
                    sync_magnetar_structures.after(sync_camera_adaptive_body_scales),
                    sync_magnetic_field_overlays.after(sync_camera_adaptive_body_scales),
                    sync_relativistic_jets.after(sync_camera_adaptive_body_scales),
                    sync_supernova_visuals
                        .after(update_supernova_explosions)
                        .after(update_persistent_remnants),
                    volumetric_nebula::sync_volumetric_nebula.after(sync_celestial_transforms),
                ),
            )
            .add_systems(
                Update,
                (
                    draw_orbital_effects_and_gizmos.after(sync_camera_adaptive_body_scales),
                    draw_habitable_zone_gizmos.after(update_pan_orbit_camera),
                    draw_lagrange_points_gizmos.after(update_pan_orbit_camera),
                    draw_dark_matter_halos.after(update_pan_orbit_camera),
                    draw_bombardment_gizmos.after(update_pan_orbit_camera),
                    draw_persistent_remnants_gizmos.after(update_pan_orbit_camera),
                    draw_kilonova_gizmos.after(update_pan_orbit_camera),
                    draw_tde_gizmos.after(update_pan_orbit_camera),
                    crate::rendering::effects::draw_impact_ejecta_system
                        .after(update_pan_orbit_camera),
                    draw_dyson_swarms.after(update_pan_orbit_camera),
                    draw_crisis_defense_gizmos.after(update_pan_orbit_camera),
                    crate::simulation::probes::draw_probe_navigation_gizmos
                        .after(surface_camera::update_surface_horizon_camera),
                    surface_camera::draw_surface_horizon_environment_gizmos
                        .after(surface_camera::update_surface_horizon_camera),
                ),
            )
            .add_systems(
                Update,
                (
                    update_impact_shockwaves
                        .after(crate::simulation::physics::step_physics_simulation),
                    crate::rendering::effects::update_impact_ejecta
                        .after(crate::simulation::physics::step_physics_simulation),
                    update_roche_debris_streams
                        .after(crate::simulation::physics::step_physics_simulation),
                    update_roche_streams
                        .after(crate::simulation::physics::step_physics_simulation)
                        .after(sync_celestial_transforms),
                    update_tidal_disruption_streams
                        .after(crate::simulation::accretion::process_accretion_and_collisions),
                    update_supernova_explosions
                        .after(crate::simulation::thermodynamics::update_thermodynamics),
                    sync_quasar_blowout_visuals.after(update_supernova_explosions),
                    update_persistent_remnants.after(update_supernova_explosions),
                    update_kilonova_effects
                        .after(crate::simulation::physics::step_physics_simulation),
                    sync_kilonova_visuals.after(update_kilonova_effects),
                    sync_tde_visuals.after(update_tidal_disruption_streams),
                    update_megastructure_construction
                        .after(crate::simulation::physics::step_physics_simulation),
                    sync_dyson_swarm_visuals.after(update_megastructure_construction),
                    sync_shkadov_stellar_engine_visuals.after(sync_dyson_swarm_visuals),
                    sync_civilization_visuals
                        .after(sync_camera_adaptive_body_scales)
                        .after(crate::simulation::biology::evolve_civilizations_system),
                    sync_impact_plume_visuals
                        .after(crate::rendering::effects::update_impact_ejecta),
                ),
            );
    }
}
