//! Visual mesh generation, PBR materials, and real-time transform synchronization.

pub mod atmospheres;
pub mod comets;
pub mod jets;
pub mod magnetospheres;
pub mod meshes;
pub mod palettes;
pub mod shadows;
pub mod spawner;
pub mod structures;
pub mod transforms;

use bevy::prelude::*;

pub use atmospheres::{sync_planetary_atmospheres, VisualAtmosphereChild};
pub use comets::{sync_cometary_tails, CometComaPart, CometTailPart, CometTailRoot};
pub use jets::{sync_relativistic_jets, RelativisticJetPart, RelativisticJetRoot};
pub use magnetospheres::{
    sync_magnetic_field_overlays, MagneticFieldLoopsPart, MagneticFieldOverlayRoot,
};
pub use meshes::{
    generate_irregular_asteroid_mesh, generate_magnetar_field_loops_mesh,
    generate_magnetar_ring_mesh, generate_pulsar_beam_mesh, recompute_mesh_normals,
    setup_visual_assets,
};
pub use palettes::{
    calc_ring_color, compute_gas_giant_palette, compute_ice_giant_palette, compute_stellar_palette,
    star_subtype_from_body_type,
};
pub use shadows::{
    compute_moon_eclipse_shadow, compute_planetary_ring_shadow, compute_ring_shadow_on_planet,
};
pub use spawner::spawn_missing_visuals;
pub use structures::{
    sync_magnetar_structures, sync_planetary_rings, sync_pulsar_beams, sync_quasar_beams,
    MagnetarStructurePart, MagnetarStructureRoot, PulsarBeamPart, PulsarBeamRoot, QuasarBeamPart,
    QuasarBeamRoot, VisualRingChild,
};
pub use transforms::{
    compute_magma_incandescence, compute_magma_ocean_crust_fraction, sync_celestial_transforms,
};

/// Marker for an entity that has its visual mesh and material spawned.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct VisualBody;

/// Common shared meshes and materials cache.
#[derive(Resource)]
pub struct VisualAssets {
    pub star_mesh: Handle<Mesh>,
    pub planet_mesh: Handle<Mesh>,
    pub atmosphere_mesh: Handle<Mesh>,
    pub asteroid_potato_mesh: Handle<Mesh>,
    pub asteroid_rubble_mesh: Handle<Mesh>,
    pub asteroid_faceted_shard_mesh: Handle<Mesh>,
    pub asteroid_cratered_spheroid_mesh: Handle<Mesh>,
    pub asteroid_contact_binary_mesh: Handle<Mesh>,
    pub asteroid_oblate_pancake_mesh: Handle<Mesh>,
    pub comet_bilobate_mesh: Handle<Mesh>,
    pub comet_bowling_pin_mesh: Handle<Mesh>,
    pub comet_cratered_nucleus_mesh: Handle<Mesh>,
    pub comet_jagged_splinter_mesh: Handle<Mesh>,
    pub comet_irregular_ellipsoid_mesh: Handle<Mesh>,
    pub particle_mesh: Handle<Mesh>,
    pub ring_mesh: Handle<Mesh>,
    pub beam_core_mesh: Handle<Mesh>,
    pub beam_sheath_mesh: Handle<Mesh>,
    pub accretion_disk_mesh: Handle<Mesh>,
    pub pulsar_beam_mesh: Handle<Mesh>,
    pub magnetar_ring_mesh: Handle<Mesh>,
    pub magnetar_field_loops_mesh: Handle<Mesh>,
    pub comet_tail_mesh: Handle<Mesh>,
    pub comet_coma_mesh: Handle<Mesh>,
}

impl VisualAssets {
    /// Constructs a dummy VisualAssets instance where all mesh handles share a single fallback mesh.
    pub fn dummy(fallback: Handle<Mesh>) -> Self {
        Self {
            star_mesh: fallback.clone(),
            planet_mesh: fallback.clone(),
            atmosphere_mesh: fallback.clone(),
            asteroid_potato_mesh: fallback.clone(),
            asteroid_rubble_mesh: fallback.clone(),
            asteroid_faceted_shard_mesh: fallback.clone(),
            asteroid_cratered_spheroid_mesh: fallback.clone(),
            asteroid_contact_binary_mesh: fallback.clone(),
            asteroid_oblate_pancake_mesh: fallback.clone(),
            comet_bilobate_mesh: fallback.clone(),
            comet_bowling_pin_mesh: fallback.clone(),
            comet_cratered_nucleus_mesh: fallback.clone(),
            comet_jagged_splinter_mesh: fallback.clone(),
            comet_irregular_ellipsoid_mesh: fallback.clone(),
            particle_mesh: fallback.clone(),
            ring_mesh: fallback.clone(),
            beam_core_mesh: fallback.clone(),
            beam_sheath_mesh: fallback.clone(),
            accretion_disk_mesh: fallback.clone(),
            pulsar_beam_mesh: fallback.clone(),
            magnetar_ring_mesh: fallback.clone(),
            magnetar_field_loops_mesh: fallback.clone(),
            comet_tail_mesh: fallback.clone(),
            comet_coma_mesh: fallback,
        }
    }
}
