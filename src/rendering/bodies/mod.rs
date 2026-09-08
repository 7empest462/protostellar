//! Visual mesh generation, PBR materials, and real-time transform synchronization.

pub mod meshes;
pub mod palettes;
pub mod spawner;
pub mod structures;
pub mod transforms;

use bevy::prelude::*;

pub use meshes::{
    generate_irregular_asteroid_mesh, generate_magnetar_field_loops_mesh,
    generate_magnetar_ring_mesh, generate_pulsar_beam_mesh, recompute_mesh_normals,
    setup_visual_assets,
};
pub use palettes::{
    calc_ring_color, compute_gas_giant_palette, compute_stellar_palette,
    star_subtype_from_body_type,
};
pub use spawner::spawn_missing_visuals;
pub use structures::{
    sync_magnetar_structures, sync_planetary_rings, sync_pulsar_beams, sync_quasar_beams,
    MagnetarStructurePart, MagnetarStructureRoot, PulsarBeamPart, PulsarBeamRoot, QuasarBeamPart,
    QuasarBeamRoot, VisualRingChild,
};
pub use transforms::sync_celestial_transforms;

/// Marker for an entity that has its visual mesh and material spawned.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct VisualBody;

/// Common shared meshes and materials cache.
#[derive(Resource)]
pub struct VisualAssets {
    pub star_mesh: Handle<Mesh>,
    pub planet_mesh: Handle<Mesh>,
    pub asteroid_potato_mesh: Handle<Mesh>,
    pub asteroid_rubble_mesh: Handle<Mesh>,
    pub comet_bilobate_mesh: Handle<Mesh>,
    pub particle_mesh: Handle<Mesh>,
    pub ring_mesh: Handle<Mesh>,
    pub beam_core_mesh: Handle<Mesh>,
    pub beam_sheath_mesh: Handle<Mesh>,
    pub accretion_disk_mesh: Handle<Mesh>,
    pub pulsar_beam_mesh: Handle<Mesh>,
    pub magnetar_ring_mesh: Handle<Mesh>,
    pub magnetar_field_loops_mesh: Handle<Mesh>,
}
