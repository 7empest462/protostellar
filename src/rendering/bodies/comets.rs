use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use hashbrown::{HashMap, HashSet};

use crate::rendering::materials::{CometTailMaterial, CometTailUniforms};
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::AU_TO_KM;

use super::VisualAssets;

/// Root marker component for an instantiated 3D cometary coma and tail hierarchy.
#[derive(Component, Debug, Clone, Copy)]
pub struct CometTailRoot {
    pub comet_entity: Entity,
}

/// Marker component for the volumetric dual ion/dust tail mesh child.
#[derive(Component, Debug, Clone, Copy)]
pub struct CometTailPart;

/// Marker component for the diffuse cometary coma glow shell mesh child.
#[derive(Component, Debug, Clone, Copy)]
pub struct CometComaPart;

fn compute_tail_directions(comet_pos: Vec3, star_pos: Vec3, comet_vel: Vec3) -> (Vec3, Vec3, Quat) {
    let mut anti_solar = (comet_pos - star_pos).normalize_or_zero();
    if anti_solar == Vec3::ZERO {
        anti_solar = Vec3::Y;
    }

    let v_norm = comet_vel.normalize_or_zero();
    let mut orb_normal = anti_solar.cross(v_norm).normalize_or_zero();
    if orb_normal.length_squared() < 0.05 {
        orb_normal = if anti_solar.y.abs() < 0.90 {
            anti_solar.cross(Vec3::Y).normalize_or_zero()
        } else {
            anti_solar.cross(Vec3::Z).normalize_or_zero()
        };
    }

    let mut in_plane_lag = orb_normal.cross(anti_solar).normalize_or_zero();
    if in_plane_lag.dot(-v_norm) < 0.0 {
        in_plane_lag = -in_plane_lag;
    }

    // Mesh extends along +Y from 0.0 to 1.0. Align +Y to anti_solar.
    let dot = anti_solar.dot(Vec3::Y);
    let rotation = if dot > 0.9999 {
        Quat::IDENTITY
    } else if dot < -0.9999 {
        Quat::from_rotation_x(std::f32::consts::PI)
    } else {
        Quat::from_rotation_arc(Vec3::Y, anti_solar)
    };

    (anti_solar, in_plane_lag, rotation)
}

/// Computes physically grounded cometary tail dimensions, activity, and composition-based colors.
/// Scales dynamically with physical radius in km, ice fraction, dust fraction, and heliocentric distance.
fn compute_tail_parameters(
    r_orbit: f32,
    comp: &Composition,
    r_km: f32,
    nuc_visual_r: f32,
    opt_escape_len: Option<f32>,
) -> (f32, f32, f32, f32, f32, Vec4, Vec4) {
    let ice_frac = (comp.ice_frac as f32).clamp(0.0, 1.0);
    let gas_frac = (comp.gas_frac as f32).clamp(0.0, 1.0);
    let volatile_frac = (ice_frac + gas_frac * 0.5).clamp(0.05, 1.0);
    let dust_ratio = (1.0 - ice_frac).clamp(0.05, 0.95);

    // Thermal insolation and surface-area dependent outgassing activity
    let insolation = (1.2 / r_orbit.max(0.15)).clamp(0.15, 6.0).powf(0.85);
    let size_factor = (r_km / 5.0).clamp(0.2, 8.0).powf(0.35);
    let activity = (volatile_frac * insolation * size_factor).clamp(0.05, 5.0);

    // Coma radius tightly envelops the nucleus (no large fixed offsets!)
    let coma_r = (nuc_visual_r
        * (1.35 + 2.2 * activity * (r_km / 5.0).clamp(0.25, 4.0).powf(0.25)))
    .clamp(nuc_visual_r * 1.2, 0.35);

    // Tail length driven by sublimation activity and physical escape tail if present
    let mut tail_len = (0.35 + 2.4 * activity).clamp(0.15, 6.5);
    if let Some(esc_len) = opt_escape_len {
        if esc_len > 0.05 {
            tail_len = tail_len.max(esc_len * 0.85).clamp(0.15, 7.5);
        }
    }

    // Flaring width at tail tip (dust tails flare wider than collimated ion ribbons)
    let flare_rate = 0.08 + 0.16 * dust_ratio;
    let tail_width = (coma_r * 1.2 + tail_len * flare_rate).clamp(coma_r * 1.1, 1.8);

    // Composition-dependent emission & scatter colors
    let ion_color = Vec4::new(0.18, 0.88, 1.0, 0.95);
    let org_tint = (comp.organics_frac as f32).clamp(0.0, 0.5) * 2.0;
    let dust_color = Vec4::new(1.0, 0.88 - 0.16 * org_tint, 0.65 - 0.20 * org_tint, 0.80);

    (
        tail_len, coma_r, tail_width, activity, dust_ratio, ion_color, dust_color,
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_comet_tail_hierarchy(
    commands: &mut Commands,
    assets: &VisualAssets,
    materials: &mut Assets<CometTailMaterial>,
    target: Entity,
    world_pos: Vec3,
    anti_solar: Vec3,
    in_plane_lag: Vec3,
    rotation: Quat,
    tail_len: f32,
    coma_r: f32,
    tail_width: f32,
    activity: f32,
    dust_ratio: f32,
    ion_color: Vec4,
    dust_color: Vec4,
    elapsed: f32,
) {
    let tail_mat = materials.add(CometTailMaterial {
        uniforms: CometTailUniforms {
            params: Vec4::new(elapsed, tail_len, coma_r, activity),
            anti_solar_and_lag: Vec4::new(anti_solar.x, anti_solar.y, anti_solar.z, 0.15),
            nucleus_pos_and_type: Vec4::new(world_pos.x, world_pos.y, world_pos.z, 0.0),
            velocity_and_activity: Vec4::new(
                in_plane_lag.x,
                in_plane_lag.y,
                in_plane_lag.z,
                dust_ratio,
            ),
            ion_color,
            dust_color,
        },
    });

    let coma_mat = materials.add(CometTailMaterial {
        uniforms: CometTailUniforms {
            params: Vec4::new(elapsed, tail_len, coma_r, activity),
            anti_solar_and_lag: Vec4::new(anti_solar.x, anti_solar.y, anti_solar.z, 0.15),
            nucleus_pos_and_type: Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0),
            velocity_and_activity: Vec4::new(
                in_plane_lag.x,
                in_plane_lag.y,
                in_plane_lag.z,
                dust_ratio,
            ),
            ion_color,
            dust_color,
        },
    });

    commands
        .spawn((
            CometTailRoot {
                comet_entity: target,
            },
            Transform::from_translation(world_pos),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                CometComaPart,
                Mesh3d(assets.comet_coma_mesh.clone()),
                MeshMaterial3d(coma_mat),
                Transform::from_scale(Vec3::splat(coma_r)),
                NotShadowCaster,
            ));
            parent.spawn((
                CometTailPart,
                Mesh3d(assets.comet_tail_mesh.clone()),
                MeshMaterial3d(tail_mat),
                Transform::from_rotation(rotation)
                    .with_scale(Vec3::new(tail_width, tail_len, tail_width)),
                NotShadowCaster,
            ));
        });
}

/// Synchronizes 3D volumetric GPU cometary coma and dual-tail meshes and materials.
/// Strictly filters for active comets and icy minor bodies; planets and moons are never given comet tails.
#[allow(
    clippy::type_complexity,
    reason = "Astrophysical cometary tail GPU synchronization system"
)]
pub fn sync_cometary_tails(
    mut commands: Commands,
    time: Res<Time>,
    visual_assets: Option<Res<VisualAssets>>,
    config: Res<SimulationConfig>,
    mut comet_materials: ResMut<Assets<CometTailMaterial>>,
    comet_query: Query<(
        Entity,
        &SimPosition,
        &SimVelocity,
        &Composition,
        &CelestialBody,
        &Radius,
        Option<&AtmosphericEscapeTail>,
    )>,
    stars_query: Query<(&SimPosition, &CelestialBody)>,
    mut root_query: Query<(Entity, &mut Transform, &CometTailRoot, Option<&Children>)>,
    mut tail_parts_query: Query<
        (&mut Transform, &MeshMaterial3d<CometTailMaterial>),
        (
            With<CometTailPart>,
            Without<CometTailRoot>,
            Without<CometComaPart>,
        ),
    >,
    mut coma_parts_query: Query<
        (&mut Transform, &MeshMaterial3d<CometTailMaterial>),
        (
            With<CometComaPart>,
            Without<CometTailRoot>,
            Without<CometTailPart>,
        ),
    >,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    let star_pos = stars_query
        .iter()
        .find(|(_, b)| b.body_type.is_star_or_remnant())
        .map_or(Vec3::ZERO, |(pos, _)| {
            Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32)
        });

    // 1. Map all candidates that qualify for cometary activity.
    // Strictly restrict to Comets or active icy minor bodies; NEVER planets, gas giants, or moons!
    let mut active_comet_map: HashMap<
        Entity,
        (Vec3, Vec3, f32, Composition, f32, f32, Option<f32>),
    > = HashMap::new();

    for (entity, pos, vel, comp, body, radius, opt_tail) in comet_query.iter() {
        // Exclude major planets, gas giants, moons, and stars
        if matches!(
            body.body_type,
            BodyType::Protoplanet
                | BodyType::TerrestrialPlanet
                | BodyType::SuperEarth
                | BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::Moon
        ) || body.body_type.is_star_or_remnant()
        {
            continue;
        }

        let is_comet_type = body.body_type == BodyType::Comet;
        let is_named_comet = body.name.to_lowercase().contains("comet");
        let is_minor_body = matches!(
            body.body_type,
            BodyType::Asteroid | BodyType::Planetesimal | BodyType::DustGrain
        );

        if !is_comet_type && !is_named_comet && !is_minor_body {
            continue;
        }

        let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let world_vel = Vec3::new(vel.x as f32, vel.y as f32, vel.z as f32);
        let r_orbit = (world_pos - star_pos).length();

        let volatile_frac = (comp.ice_frac + comp.gas_frac) as f32;
        let has_active_escape = opt_tail.is_some_and(|t| t.is_active && t.tail_length_au > 0.01);

        // Active sublimation criteria
        let is_active = if has_active_escape {
            true
        } else if is_comet_type || is_named_comet {
            (comp.ice_frac > 0.05 || volatile_frac > 0.10) && r_orbit < 6.0 && r_orbit > 0.05
        } else {
            // Active Centaur / icy planetesimal / main-belt active minor body
            (comp.ice_frac > 0.10 || volatile_frac > 0.15) && r_orbit < 4.5 && r_orbit > 0.05
        };

        if is_active {
            let r_km = (radius.0 * AU_TO_KM) as f32;
            let nuc_visual_r = config.calc_visual_radius_for_type(radius.0, body.body_type);
            let opt_escape_len = opt_tail.and_then(|t| {
                if t.is_active && t.tail_length_au > 0.01 {
                    Some(t.tail_length_au)
                } else {
                    None
                }
            });
            active_comet_map.insert(
                entity,
                (
                    world_pos,
                    world_vel,
                    r_orbit,
                    *comp,
                    r_km,
                    nuc_visual_r,
                    opt_escape_len,
                ),
            );
        }
    }

    let elapsed = time.elapsed_secs();
    let mut updated_entities = HashSet::new();

    // 2. Update existing CometTailRoots or despawn orphans
    for (root_entity, mut root_trans, root, opt_children) in root_query.iter_mut() {
        if let Some(&(
            world_pos,
            world_vel,
            r_orbit,
            ref comp,
            r_km,
            nuc_visual_r,
            opt_escape_len,
        )) = active_comet_map.get(&root.comet_entity)
        {
            root_trans.translation = world_pos;
            updated_entities.insert(root.comet_entity);

            let (anti_solar, in_plane_lag, rotation) =
                compute_tail_directions(world_pos, star_pos, world_vel);
            let (tail_len, coma_r, tail_width, activity, dust_ratio, ion_color, dust_color) =
                compute_tail_parameters(r_orbit, comp, r_km, nuc_visual_r, opt_escape_len);

            if let Some(children) = opt_children {
                for child in children.iter() {
                    if let Ok((mut trans, mat_handle)) = tail_parts_query.get_mut(child) {
                        trans.rotation = rotation;
                        trans.scale = Vec3::new(tail_width, tail_len, tail_width);
                        if let Some(mut mat) = comet_materials.get_mut(&mat_handle.0) {
                            mat.uniforms.params = Vec4::new(elapsed, tail_len, coma_r, activity);
                            mat.uniforms.anti_solar_and_lag =
                                Vec4::new(anti_solar.x, anti_solar.y, anti_solar.z, 0.15);
                            mat.uniforms.nucleus_pos_and_type =
                                Vec4::new(world_pos.x, world_pos.y, world_pos.z, 0.0);
                            mat.uniforms.velocity_and_activity = Vec4::new(
                                in_plane_lag.x,
                                in_plane_lag.y,
                                in_plane_lag.z,
                                dust_ratio,
                            );
                            mat.uniforms.ion_color = ion_color;
                            mat.uniforms.dust_color = dust_color;
                        }
                    } else if let Ok((mut trans, mat_handle)) = coma_parts_query.get_mut(child) {
                        trans.scale = Vec3::splat(coma_r);
                        if let Some(mut mat) = comet_materials.get_mut(&mat_handle.0) {
                            mat.uniforms.params = Vec4::new(elapsed, tail_len, coma_r, activity);
                            mat.uniforms.anti_solar_and_lag =
                                Vec4::new(anti_solar.x, anti_solar.y, anti_solar.z, 0.15);
                            mat.uniforms.nucleus_pos_and_type =
                                Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
                            mat.uniforms.velocity_and_activity = Vec4::new(
                                in_plane_lag.x,
                                in_plane_lag.y,
                                in_plane_lag.z,
                                dust_ratio,
                            );
                            mat.uniforms.ion_color = ion_color;
                            mat.uniforms.dust_color = dust_color;
                        }
                    }
                }
            }
        } else if let Ok(mut cmd) = commands.get_entity(root_entity) {
            cmd.despawn();
        }
    }

    // 3. Spawn missing CometTailRoots for newly activated comets
    for (&target, &(world_pos, world_vel, r_orbit, ref comp, r_km, nuc_visual_r, opt_escape_len)) in
        &active_comet_map
    {
        if updated_entities.contains(&target) {
            continue;
        }

        let (anti_solar, in_plane_lag, rotation) =
            compute_tail_directions(world_pos, star_pos, world_vel);
        let (tail_len, coma_r, tail_width, activity, dust_ratio, ion_color, dust_color) =
            compute_tail_parameters(r_orbit, comp, r_km, nuc_visual_r, opt_escape_len);

        spawn_comet_tail_hierarchy(
            &mut commands,
            &assets,
            &mut comet_materials,
            target,
            world_pos,
            anti_solar,
            in_plane_lag,
            rotation,
            tail_len,
            coma_r,
            tail_width,
            activity,
            dust_ratio,
            ion_color,
            dust_color,
            elapsed,
        );
    }
}
