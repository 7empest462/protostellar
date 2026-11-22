//! Kardashev Type II Dyson Swarm megastructure progression and 3D golden collector array visuals.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use super::tech_materials::{
    MegastructureMaterial, dyson_collector_material, shkadov_engine_material,
};
use super::tech_meshes::{build_dyson_collector_ring_mesh, build_shkadov_engine_mesh};
use crate::simulation::components::celestial::{CentralStar, Radius, SimPosition};
use crate::simulation::components::planetary::BiosphereState;
use crate::simulation::resources::SimTime;

/// Maximum number of nested 3D orbital collector lattice rings rendered per star.
pub const MAX_DYSON_VISUAL_RINGS: usize = 12;

#[derive(
    Component, Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize,
)]
pub struct DysonSwarm {
    pub completion_fraction: f32, // 0.0 to 1.0
    pub mirror_count: usize,
}

/// Marker component for an orbiting 3D Dyson Swarm metallic-gold collector lattice ring.
#[derive(Component, Debug, Clone, Copy)]
pub struct DysonRingVisual {
    pub star_entity: Entity,
    pub ring_idx: usize,
}

/// Cached procedural collector lattice mesh and metallic PBR materials.
#[derive(Resource, Clone)]
pub struct DysonCollectorAssets {
    pub ring_mesh: Handle<Mesh>,
    pub collector_mat: Handle<MegastructureMaterial>,
}

/// Marker component for the colossal hemispherical Shkadov Stellar Engine mirror cap.
#[derive(Component, Debug, Clone, Copy)]
pub struct ShkadovEngineVisual {
    pub star_entity: Entity,
}

/// Cached mesh and procedural metallic mirror material for the Shkadov Stellar Engine cap.
#[derive(Resource, Clone)]
pub struct ShkadovEngineAssets {
    pub hemisphere_mesh: Handle<Mesh>,
    pub engine_mat: Handle<MegastructureMaterial>,
}

/// Generates a double-walled parabolic Shkadov Stellar Engine mirror canopy mesh with backshell trusses.
#[must_use]
pub fn generate_shkadov_hemisphere_mesh() -> Mesh {
    build_shkadov_engine_mesh()
}

/// Generates a 36-module robotic Dyson Swarm collector satellite & structural truss ring at unit radius.
#[must_use]
pub fn generate_dyson_collector_ring_mesh() -> Mesh {
    build_dyson_collector_ring_mesh()
}

pub fn update_megastructure_construction(
    mut commands: Commands,
    query_planets: Query<&BiosphereState>,
    mut query_stars: Query<(Entity, Option<&mut DysonSwarm>), With<CentralStar>>,
    sim_time: Res<SimTime>,
    opt_techs: Option<Res<crate::simulation::economy::UnlockedTechs>>,
) {
    let dt = sim_time.current_dt_yr as f32;
    let mut highest_techno = 0.0;

    for bio in query_planets.iter() {
        if bio.technosignature > highest_techno {
            highest_techno = bio.technosignature;
        }
    }

    let speed_mult = if opt_techs
        .as_deref()
        .is_some_and(|t| t.has(crate::simulation::economy::TechnologyId::DysonCollectors))
    {
        5.0
    } else {
        1.0
    };

    if highest_techno > 1.5 {
        for (star_entity, opt_swarm) in query_stars.iter_mut() {
            if let Some(mut swarm) = opt_swarm {
                swarm.completion_fraction =
                    (swarm.completion_fraction + 0.01 * dt * speed_mult).min(1.0);
                swarm.mirror_count = (swarm.completion_fraction * 100_000.0) as usize;
                // Light interception is handled dynamically in thermodynamics.rs
            } else if let Ok(mut ecmd) = commands.get_entity(star_entity) {
                ecmd.insert(DysonSwarm {
                    completion_fraction: 0.001,
                    mirror_count: 100,
                });
            }
        }
    }
}

#[inline]
fn dyson_base_radius(opt_rad: Option<&Radius>) -> f32 {
    let star_phys_r = opt_rad.map_or(0.00465, |r| r.0) as f32;
    (star_phys_r * 16.0).clamp(0.006, 0.28)
}

fn dyson_ring_transform(center: Vec3, base_r: f32, ring_idx: usize, elapsed: f32) -> Transform {
    let fi = ring_idx as f32;
    let radius = base_r + fi * base_r * 0.075;
    // Golden-angle nested orbital inclinations creating a geodesic spherical swarm lattice
    let incl = (fi * 1.12).sin() * 1.15;
    let asc_node = fi * 2.399_963_2;
    let orbit_phase = elapsed
        * (0.28 / (1.0 + fi * 0.14))
        * if ring_idx.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
    let rot = Quat::from_rotation_y(asc_node)
        * Quat::from_rotation_x(incl)
        * Quat::from_rotation_y(orbit_phase);
    Transform::from_translation(center)
        .with_rotation(rot)
        .with_scale(Vec3::splat(radius))
}

/// Synchronizes 3D metallic-gold orbiting Dyson Swarm collector lattice rings around host stars.
#[allow(
    clippy::too_many_arguments,
    reason = "Dyson swarm 3D collector ring synchronization"
)]
pub fn sync_dyson_swarm_visuals(
    mut commands: Commands,
    sim_time: Option<Res<SimTime>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut tech_mats: Option<ResMut<Assets<MegastructureMaterial>>>,
    assets_opt: Option<Res<DysonCollectorAssets>>,
    query_swarms: Query<(Entity, &SimPosition, Option<&Radius>, &DysonSwarm)>,
    mut ring_query: Query<(Entity, &DysonRingVisual, &mut Transform)>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let (Some(meshes), Some(tech_mats)) = (meshes.as_deref_mut(), tech_mats.as_deref_mut()) else {
        return;
    };
    let origin = opt_origin.as_deref().copied().unwrap_or_default();

    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let mut target_counts: FxHashMap<Entity, (Vec3, f32, usize)> = FxHashMap::default();
    let mut primary_star_center = origin.to_render(bevy::math::DVec3::ZERO);
    for (star_ent, pos, opt_rad, swarm) in query_swarms.iter() {
        let count = ((swarm.completion_fraction * MAX_DYSON_VISUAL_RINGS as f32).ceil() as usize)
            .clamp(
                usize::from(swarm.completion_fraction > 0.0),
                MAX_DYSON_VISUAL_RINGS,
            );
        if count > 0 {
            let center = origin.to_render(pos.0);
            let base_r = dyson_base_radius(opt_rad);
            primary_star_center = center;
            target_counts.insert(star_ent, (center, base_r, count));
        }
    }

    if target_counts.is_empty() && ring_query.is_empty() {
        return;
    }

    let assets = if let Some(existing) = assets_opt {
        existing.clone()
    } else {
        let ring_mesh = meshes.add(generate_dyson_collector_ring_mesh());
        let collector_mat = tech_mats.add(dyson_collector_material());
        let created = DysonCollectorAssets {
            ring_mesh,
            collector_mat,
        };
        commands.insert_resource(created.clone());
        created
    };
    if let Some(mut mat) = tech_mats.get_mut(&assets.collector_mat) {
        mat.uniforms.star_pos_and_time = primary_star_center.extend(elapsed);
    }

    let mut existing_rings: FxHashSet<(Entity, usize)> = FxHashSet::default();
    for (ent, ring, mut tf) in ring_query.iter_mut() {
        let Some(&(center, base_r, count)) = target_counts.get(&ring.star_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        if ring.ring_idx >= count {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        }
        existing_rings.insert((ring.star_entity, ring.ring_idx));
        let next_tf = dyson_ring_transform(center, base_r, ring.ring_idx, elapsed);
        if tf.translation != next_tf.translation {
            tf.translation = next_tf.translation;
        }
        if tf.rotation != next_tf.rotation {
            tf.rotation = next_tf.rotation;
        }
        if tf.scale != next_tf.scale {
            tf.scale = next_tf.scale;
        }
    }

    for (&star_entity, &(center, base_r, count)) in &target_counts {
        for ring_idx in 0..count {
            if !existing_rings.contains(&(star_entity, ring_idx)) {
                commands.spawn((
                    DysonRingVisual {
                        star_entity,
                        ring_idx,
                    },
                    Mesh3d(assets.ring_mesh.clone()),
                    MeshMaterial3d(assets.collector_mat.clone()),
                    dyson_ring_transform(center, base_r, ring_idx, elapsed),
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            }
        }
    }
}

/// Synchronizes the colossal Shkadov Stellar Engine mirror hemisphere once unlocked on a mature Dyson Swarm.
#[allow(
    clippy::too_many_arguments,
    reason = "Shkadov engine visual sync coordinates tech unlock, progression state, star swarms, and mesh assets"
)]
pub fn sync_shkadov_stellar_engine_visuals(
    mut commands: Commands,
    sim_time: Option<Res<SimTime>>,
    opt_techs: Option<Res<crate::simulation::economy::UnlockedTechs>>,
    opt_prog: Option<Res<crate::simulation::economy::ProgressionState>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut tech_mats: Option<ResMut<Assets<MegastructureMaterial>>>,
    assets_opt: Option<Res<ShkadovEngineAssets>>,
    query_swarms: Query<(Entity, &SimPosition, Option<&Radius>, &DysonSwarm)>,
    mut engine_query: Query<(Entity, &ShkadovEngineVisual, &mut Transform)>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let unlocked = opt_techs
        .as_deref()
        .is_some_and(|t| t.has(crate::simulation::economy::TechnologyId::StellarEngine))
        || opt_prog.as_deref().is_some_and(|p| p.stellar_engine_active);

    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    let mut targets: FxHashMap<Entity, (Vec3, f32)> = FxHashMap::default();
    let mut primary_star_center = origin.to_render(bevy::math::DVec3::ZERO);
    if unlocked {
        for (star_ent, pos, opt_rad, swarm) in query_swarms.iter() {
            if swarm.completion_fraction >= 0.35 {
                let c = origin.to_render(pos.0);
                let base_r = dyson_base_radius(opt_rad);
                primary_star_center = c;
                targets.insert(star_ent, (c, base_r));
            }
        }
    }

    if targets.is_empty() && engine_query.is_empty() {
        return;
    }
    let (Some(meshes), Some(tech_mats)) = (meshes.as_deref_mut(), tech_mats.as_deref_mut()) else {
        return;
    };
    let assets = if let Some(existing) = assets_opt {
        existing.clone()
    } else {
        let created = ShkadovEngineAssets {
            hemisphere_mesh: meshes.add(generate_shkadov_hemisphere_mesh()),
            engine_mat: tech_mats.add(shkadov_engine_material()),
        };
        commands.insert_resource(created.clone());
        created
    };

    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    if let Some(mut mat) = tech_mats.get_mut(&assets.engine_mat) {
        mat.uniforms.star_pos_and_time = primary_star_center.extend(elapsed);
    }
    let rot = Quat::from_rotation_z(0.95) * Quat::from_rotation_y(elapsed * 0.08);
    let mut existing: FxHashSet<Entity> = FxHashSet::default();
    for (ent, eng, mut tf) in engine_query.iter_mut() {
        let Some(&(center, base_r)) = targets.get(&eng.star_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        existing.insert(eng.star_entity);
        let next = Transform::from_translation(center)
            .with_rotation(rot)
            .with_scale(Vec3::splat(base_r * 2.15));
        if tf.translation != next.translation {
            tf.translation = next.translation;
        }
        if tf.rotation != next.rotation {
            tf.rotation = next.rotation;
        }
        if tf.scale != next.scale {
            tf.scale = next.scale;
        }
    }

    for (star_entity, (center, base_r)) in targets {
        if !existing.contains(&star_entity) {
            commands.spawn((
                ShkadovEngineVisual { star_entity },
                crate::rendering::effects::civilizations::CivilizationVisualMarker,
                Mesh3d(assets.hemisphere_mesh.clone()),
                MeshMaterial3d(assets.engine_mat.clone()),
                Transform::from_translation(center)
                    .with_rotation(rot)
                    .with_scale(Vec3::splat(base_r * 2.15)),
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }
}

/// Subtle beamed-power energy relay arcs between completed Dyson collectors; the collector swarm
/// itself now renders as real 3D golden lattice rings via [`sync_dyson_swarm_visuals`].
pub fn draw_dyson_swarms(
    mut gizmos: Gizmos,
    query_swarms: Query<(&SimPosition, Option<&Radius>, &DysonSwarm)>,
    camera_query: Query<&crate::rendering::camera::PanOrbitCamera>,
    sim_time: Option<Res<SimTime>>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    if camera_query.iter().any(|c| c.surface_horizon_mode) {
        return;
    }
    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    for (pos, opt_rad, swarm) in query_swarms.iter() {
        if swarm.completion_fraction < 0.35 {
            continue;
        }
        let center = origin.to_render(pos.0);
        let base_r = dyson_base_radius(opt_rad);
        let outer_r = base_r * 1.9;
        let intensity = ((swarm.completion_fraction - 0.35) / 0.65).clamp(0.0, 1.0);
        let beam_col = Color::srgba(0.45, 0.92, 1.0, 0.32 * intensity);
        for k in 0..4 {
            let a = (k as f32) * std::f32::consts::FRAC_PI_2 + elapsed * 0.25;
            let p_inner = center + Vec3::new(base_r * a.cos(), 0.0, base_r * a.sin());
            let p_outer = center + Vec3::new(outer_r * a.cos(), 0.04 * a.sin(), outer_r * a.sin());
            gizmos.line(p_inner, p_outer, beam_col);
        }
    }
}
