//! Visual mesh generation, PBR materials, and real-time transform synchronization.

pub mod atmospheres;
pub mod circumstellar;
pub mod comets;
pub mod jets;
pub mod magnetospheres;
pub mod materials_sync;
pub mod meshes;
pub mod palettes;
pub mod scaling;
pub mod shadows;
pub mod spawner;
pub mod structures;
pub mod transforms;
pub mod view_overrides;

use bevy::prelude::*;

pub use atmospheres::{VisualAtmosphereChild, sync_planetary_atmospheres};
pub use circumstellar::{
    VisualCircumstellarChild, compute_circumstellar_uniforms, resolve_circumstellar_profile,
    sync_circumstellar_environments,
};
pub use comets::{CometComaPart, CometTailPart, CometTailRoot, sync_cometary_tails};
pub use jets::{RelativisticJetPart, RelativisticJetRoot, sync_relativistic_jets};
pub use magnetospheres::{
    MagneticFieldLoopsPart, MagneticFieldOverlayRoot, sync_magnetic_field_overlays,
};
pub use meshes::{
    generate_irregular_asteroid_mesh, generate_magnetar_field_loops_mesh,
    generate_magnetar_ring_mesh, generate_pulsar_beam_mesh, recompute_mesh_normals,
    setup_visual_assets, sync_selected_body_mesh_lod,
};
pub use palettes::{
    calc_ring_color, compute_gas_giant_palette, compute_ice_giant_palette, compute_stellar_palette,
    star_subtype_from_body_type,
};
pub use shadows::{
    compute_eclipse_moons_data, compute_moon_eclipse_shadow, compute_planetary_ring_shadow,
    compute_ring_shadow_on_planet,
};
pub use spawner::spawn_missing_visuals;
pub use structures::{
    MagnetarStructurePart, MagnetarStructureRoot, PulsarBeamPart, PulsarBeamRoot, QuasarBeamPart,
    QuasarBeamRoot, VisualBlackHoleDiskChild, VisualRingChild, sync_black_hole_accretion_disks,
    sync_magnetar_structures, sync_planetary_rings, sync_pulsar_beams, sync_quasar_beams,
};
pub use transforms::{
    compute_magma_incandescence, compute_magma_ocean_crust_fraction, resolve_star_direction,
    sync_camera_adaptive_body_scales, sync_celestial_transforms,
};
pub use view_overrides::{stripped_core_display_temp_k, stripped_interior_composition};

/// Marker for an entity that has its visual mesh and material spawned.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct VisualBody;

/// Common shared meshes and materials cache.
#[derive(Resource)]
pub struct VisualAssets {
    pub star_mesh: Handle<Mesh>,
    pub star_mesh_hires: Handle<Mesh>,
    pub planet_mesh: Handle<Mesh>,
    pub planet_mesh_hires: Handle<Mesh>,
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
            star_mesh_hires: fallback.clone(),
            planet_mesh: fallback.clone(),
            planet_mesh_hires: fallback.clone(),
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
