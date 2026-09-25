//! GPU-accelerated 3D volumetric magnetic field loop and magnetosphere overlay.
//!
//! Visualizes magnetic dipole flux tubes around magnetized planets, gas giants,
//! and companion stars. Features smart interaction with ultra-magnetized remnants
//! (e.g. SGR 1806-20 Magnetar): avoids redundant geometry on the magnetar itself,
//! while simulating dayside compression, magnetic wake elongation, and unipolar
//! induction brightness boost on orbiting companion stars and planets.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use hashbrown::{HashMap, HashSet};

use crate::rendering::bodies::VisualAssets;
use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Root marker component for an instantiated 3D magnetic field overlay hierarchy.
#[derive(Component, Debug, Clone, Copy)]
pub struct MagneticFieldOverlayRoot {
    pub target_entity: Entity,
}

/// Marker component for the volumetric magnetic field loop mesh child.
#[derive(Component, Debug, Clone, Copy)]
pub struct MagneticFieldLoopsPart;

/// Parameters defining the 3D visual transform and emissive material of an active magnetosphere.
#[derive(Clone, Copy, Debug)]
struct MagnetosphereParams {
    world_pos: Vec3,
    scale: Vec3,
    rotation: Quat,
    emissive_color: LinearRgba,
}

struct MagnetarSource {
    pos: Vec3,
    vel: Vec3,
    b_field: f64,
}

pub type MagnetosphereBodyQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    &'a SimVelocity,
    &'a CelestialBody,
    &'a Radius,
    &'a Composition,
    Option<&'a InternalDifferentiation>,
    Option<&'a ElectromagneticFieldState>,
    Option<&'a SpinState>,
    Option<&'a super::MagnetarStructureRoot>,
);

pub type MagnetosphereBodyQuery<'w, 's> = Query<'w, 's, MagnetosphereBodyQueryItem<'static>>;

pub type MagnetosphereRootQueryItem<'a> = (
    Entity,
    &'a mut Transform,
    &'a MagneticFieldOverlayRoot,
    Option<&'a Children>,
);

pub type MagnetosphereRootQuery<'w, 's> = Query<'w, 's, MagnetosphereRootQueryItem<'static>>;

pub type MagnetospherePartQueryItem<'a> = (&'a mut Transform, &'a MeshMaterial3d<StandardMaterial>);

pub type MagnetospherePartFilter = (
    With<MagneticFieldLoopsPart>,
    Without<MagneticFieldOverlayRoot>,
);

pub type MagnetospherePartQuery<'w, 's> =
    Query<'w, 's, MagnetospherePartQueryItem<'static>, MagnetospherePartFilter>;

fn find_magnetar_source(body_query: &MagnetosphereBodyQuery) -> Option<MagnetarSource> {
    body_query
        .iter()
        .find(|(_, _, _, body, _, _, _, opt_em, _, opt_mag_root)| {
            body.body_type == BodyType::Magnetar
                || opt_mag_root.is_some()
                || opt_em.is_some_and(|em| em.magnetic_field_gauss >= 1.0e14)
                || (body.name.to_lowercase().contains("magnetar")
                    && !body.name.contains("Clump")
                    && !body.name.contains("Ejecta"))
        })
        .map(|(_, pos, vel, _, _, _, _, opt_em, _, _)| {
            let p = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            let v = Vec3::new(vel.x as f32, vel.y as f32, vel.z as f32);
            let b_field = opt_em.map_or(1.0e15, |em| em.magnetic_field_gauss);
            MagnetarSource {
                pos: p,
                vel: v,
                b_field,
            }
        })
}

fn compute_magnetosphere_params_for_body(
    world_pos: Vec3,
    world_vel: Vec3,
    axis: Vec3,
    vis_r: f32,
    b_gauss: f64,
    body_type: BodyType,
    is_star: bool,
    elapsed: f32,
    magnetar_source: Option<&MagnetarSource>,
) -> (Vec3, Quat, LinearRgba) {
    if let Some(mag) = magnetar_source {
        let to_mag = mag.pos - world_pos;
        let dist_to_mag = to_mag.length().max(0.05);
        let from_mag_dir = -to_mag.normalize_or_zero();

        let b_ext = (mag.b_field * 1e-15 * (0.01 / f64::from(dist_to_mag)).powi(3))
            .clamp(0.001, 100.0) as f32;

        let v_rel = (world_vel - mag.vel).length();
        let induction_boost = (1.0 + v_rel * b_ext * 3.5).clamp(1.0, 7.5);

        let base_scale = if is_star {
            vis_r * 2.8
        } else if body_type == BodyType::GasGiant {
            vis_r * 3.8
        } else {
            (vis_r * (1.8 + 0.6 * (b_gauss as f32).max(0.1).powf(0.3)))
                .clamp(vis_r * 1.3, vis_r * 4.5)
        };

        let scale = Vec3::new(base_scale * 0.88, base_scale * 1.20, base_scale * 0.88);
        let blended_axis = (axis * 0.80 + from_mag_dir * 0.35).normalize();
        let wake_rot =
            Quat::from_rotation_arc(Vec3::Y, blended_axis) * Quat::from_rotation_y(elapsed * 0.8);

        let emissive = if is_star {
            LinearRgba::new(
                3.5 * induction_boost,
                9.0 * induction_boost,
                24.0 * induction_boost,
                0.88,
            )
        } else if body_type == BodyType::GasGiant {
            LinearRgba::new(
                16.0 * induction_boost,
                10.0 * induction_boost,
                3.0 * induction_boost,
                0.85,
            )
        } else {
            LinearRgba::new(
                2.0 * induction_boost,
                8.5 * induction_boost,
                18.0 * induction_boost,
                0.85,
            )
        };

        (scale, wake_rot, emissive)
    } else {
        let base_scale = if is_star {
            vis_r * 2.2
        } else if body_type == BodyType::GasGiant {
            vis_r * 4.0
        } else {
            (vis_r * (1.6 + 1.2 * (b_gauss as f32).max(0.05).powf(0.3)))
                .clamp(vis_r * 1.3, vis_r * 3.2)
        };

        let scale = Vec3::splat(base_scale);
        let rot = Quat::from_rotation_arc(Vec3::Y, axis) * Quat::from_rotation_y(elapsed * 0.4);

        let emissive = if is_star {
            LinearRgba::new(12.0, 6.0, 1.2, 0.80)
        } else if body_type == BodyType::GasGiant {
            LinearRgba::new(16.0, 10.0, 2.5, 0.80)
        } else {
            LinearRgba::new(1.5, 7.5, 16.0, 0.80)
        };

        (scale, rot, emissive)
    }
}

fn collect_magnetosphere_candidates(
    body_query: &MagnetosphereBodyQuery,
    magnetar_source: Option<&MagnetarSource>,
    config: &SimulationConfig,
    elapsed: f32,
) -> HashMap<Entity, MagnetosphereParams> {
    let mut candidate_map = HashMap::new();

    for (entity, pos, vel, body, radius, comp, opt_diff, opt_em, opt_spin, opt_mag_root) in
        body_query.iter()
    {
        if body.body_type == BodyType::Magnetar
            || opt_mag_root.is_some()
            || (body.name.to_lowercase().contains("magnetar")
                && !body.name.contains("Clump")
                && !body.name.contains("Ejecta"))
        {
            continue;
        }

        let is_star = body.body_type.is_star_or_remnant();
        let b_gauss = opt_em.map_or_else(
            || opt_diff.map_or(0.0, |d| d.magnetic_field_gauss),
            |em| em.magnetic_field_gauss,
        );
        let has_dynamo = b_gauss > 0.01;

        let is_induced_by_magnetar = magnetar_source.is_some()
            && matches!(
                body.body_type,
                BodyType::TerrestrialPlanet
                    | BodyType::SuperEarth
                    | BodyType::Protoplanet
                    | BodyType::GasGiant
                    | BodyType::IceGiant
            );

        let is_magnetized_planet = matches!(
            body.body_type,
            BodyType::TerrestrialPlanet
                | BodyType::SuperEarth
                | BodyType::GasGiant
                | BodyType::IceGiant
        ) && (has_dynamo || b_gauss > 0.05 || comp.metal_frac > 0.15);

        if !is_star && !has_dynamo && !is_magnetized_planet && !is_induced_by_magnetar {
            continue;
        }

        let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let world_vel = Vec3::new(vel.x as f32, vel.y as f32, vel.z as f32);

        let spin_axis = opt_spin.map_or(Vec3::Y, |s| {
            let sv = Vec3::new(
                s.spin_vector.x as f32,
                s.spin_vector.y as f32,
                s.spin_vector.z as f32,
            );
            sv.normalize_or_zero()
        });
        let axis = if spin_axis == Vec3::ZERO {
            Vec3::Y
        } else {
            spin_axis
        };

        let vis_r = config.calc_visual_radius_for_type(radius.0, body.body_type);
        let (scale, rotation, emissive_color) = compute_magnetosphere_params_for_body(
            world_pos,
            world_vel,
            axis,
            vis_r,
            b_gauss,
            body.body_type,
            is_star,
            elapsed,
            magnetar_source,
        );

        candidate_map.insert(
            entity,
            MagnetosphereParams {
                world_pos,
                scale,
                rotation,
                emissive_color,
            },
        );
    }
    candidate_map
}

fn update_existing_magnetospheres(
    commands: &mut Commands,
    candidate_map: &HashMap<Entity, MagnetosphereParams>,
    materials: &mut Assets<StandardMaterial>,
    root_query: &mut MagnetosphereRootQuery,
    part_query: &mut MagnetospherePartQuery,
) -> HashSet<Entity> {
    let mut updated_entities = HashSet::new();

    for (root_entity, mut root_trans, root, opt_children) in root_query.iter_mut() {
        if let Some(&params) = candidate_map.get(&root.target_entity) {
            root_trans.translation = params.world_pos;
            updated_entities.insert(root.target_entity);

            if let Some(children) = opt_children {
                for child in children.iter() {
                    if let Ok((mut part_trans, mat_handle)) = part_query.get_mut(child) {
                        part_trans.rotation = params.rotation;
                        part_trans.scale = params.scale;
                        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                            mat.emissive = params.emissive_color;
                        }
                    }
                }
            }
        } else if let Ok(mut cmd) = commands.get_entity(root_entity) {
            cmd.despawn();
        }
    }
    updated_entities
}

fn spawn_missing_magnetospheres(
    commands: &mut Commands,
    assets: &VisualAssets,
    materials: &mut Assets<StandardMaterial>,
    candidate_map: &HashMap<Entity, MagnetosphereParams>,
    updated_entities: &HashSet<Entity>,
) {
    for (&target, &params) in candidate_map {
        if updated_entities.contains(&target) {
            continue;
        }

        let loop_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: params.emissive_color,
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            unlit: true,
            ..default()
        });

        commands
            .spawn((
                MagneticFieldOverlayRoot {
                    target_entity: target,
                },
                Transform::from_translation(params.world_pos),
                Visibility::default(),
            ))
            .with_children(|parent| {
                parent.spawn((
                    MagneticFieldLoopsPart,
                    Mesh3d(assets.magnetar_field_loops_mesh.clone()),
                    MeshMaterial3d(loop_mat),
                    Transform::from_rotation(params.rotation).with_scale(params.scale),
                    NotShadowCaster,
                ));
            });
    }
}

/// Synchronizes 3D volumetric GPU magnetic field loops when DiagnosticOverlayMode::MagneticFields is active.
///
/// Intelligently skips the central magnetar (which already renders permanent 3D field loops)
/// and simulates magnetic induction and distortion on companion stars and orbiting planets.
pub fn sync_magnetic_field_overlays(
    mut commands: Commands,
    sim_time: Option<Res<SimTime>>,
    player_state: Res<PlayerInteractionState>,
    visual_assets: Option<Res<VisualAssets>>,
    config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    body_query: MagnetosphereBodyQuery,
    mut root_query: MagnetosphereRootQuery,
    mut part_query: MagnetospherePartQuery,
) {
    let is_active_mode = player_state.overlay_mode == DiagnosticOverlayMode::MagneticFields;

    if !is_active_mode {
        for (root_ent, _, _, _) in root_query.iter() {
            if let Ok(mut cmd) = commands.get_entity(root_ent) {
                cmd.despawn();
            }
        }
        return;
    }

    let Some(assets) = visual_assets else {
        return;
    };

    let magnetar_source = find_magnetar_source(&body_query);
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);

    let candidate_map =
        collect_magnetosphere_candidates(&body_query, magnetar_source.as_ref(), &config, elapsed);

    let updated_entities = update_existing_magnetospheres(
        &mut commands,
        &candidate_map,
        &mut materials,
        &mut root_query,
        &mut part_query,
    );

    spawn_missing_magnetospheres(
        &mut commands,
        &assets,
        &mut materials,
        &candidate_map,
        &updated_entities,
    );
}
