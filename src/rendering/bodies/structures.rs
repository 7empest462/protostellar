use bevy::light::{NotShadowCaster, RectLight};
use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::palettes::calc_ring_color;
use super::{VisualAssets, VisualBody};

/// Marker component for an instantiated visual planetary ring entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualRingChild;

struct RingVisualGeometry {
    outer_scale: f32,
    inner_ratio_mesh: f32,
    star_dir_local: Vec3,
    star_dir_world: Vec3,
    planet_radius_ratio: f32,
    planet_oblateness: f32,
}

#[allow(clippy::too_many_arguments, reason = "Planetary ring geometry helper")]
fn compute_ring_geometry(
    body: &CelestialBody,
    ring_sys: &PlanetaryRingSystem,
    radius: &Radius,
    opt_mass: Option<&Mass>,
    opt_comp: Option<&Composition>,
    opt_diff: Option<&InternalDifferentiation>,
    show_atmospheres: bool,
    planet_trans: &Transform,
    star_world_pos: Vec3,
) -> RingVisualGeometry {
    let is_star = body.body_type.is_star_or_remnant();
    let stripped_r = if show_atmospheres || is_star {
        radius.0
    } else if let (Some(mass), Some(comp)) = (opt_mass, opt_comp) {
        super::view_overrides::stripped_interior_radius_au(
            mass.0,
            radius.0,
            body.body_type,
            comp,
            opt_diff,
        )
    } else {
        radius.0
    };
    let core_ratio_inv = if !is_star && stripped_r > 1e-12 && stripped_r < radius.0 * 0.999 {
        (radius.0 / stripped_r) as f32
    } else {
        1.0
    };
    let (resolved_inner_ratio, resolved_outer_ratio) =
        super::shadows::resolve_effective_ring_ratios_for_body(ring_sys, radius.0, &body.name);
    let ring_outer_scale = if is_star {
        let vis_r = planet_trans.scale.x.max(0.001);
        let target_outer_r = if ring_sys.outer_radius_au > 0.0 {
            ring_sys.outer_radius_au
        } else {
            (radius.0 as f32 * 8.0).clamp(2.5, 18.0)
        };
        (target_outer_r / vis_r) * 2.0
    } else {
        resolved_outer_ratio * core_ratio_inv * 2.0
    };

    let star_dir_world = if is_star {
        Vec3::Y
    } else {
        let d = (star_world_pos - planet_trans.translation).normalize_or_zero();
        if d.length_squared() < 0.01 {
            Vec3::Y
        } else {
            d
        }
    };
    let star_dir_local = if is_star {
        Vec3::Y
    } else {
        (planet_trans.rotation.inverse() * star_dir_world).normalize_or_zero()
    };
    let planet_radius_ratio = if is_star {
        (planet_trans.scale.x / (ring_outer_scale * 0.5 * planet_trans.scale.x)).clamp(0.01, 0.25)
    } else {
        1.0 / (ring_outer_scale * 0.5)
    };

    let inner_ratio_mesh = if is_star {
        if ring_sys.outer_radius_au > 0.0 && ring_sys.inner_radius_au > 0.0 {
            (ring_sys.inner_radius_au / ring_sys.outer_radius_au).clamp(planet_radius_ratio, 0.95)
        } else {
            (1.24f32 / 2.35f32).clamp(planet_radius_ratio, 0.95)
        }
    } else {
        (resolved_inner_ratio / resolved_outer_ratio).clamp(planet_radius_ratio * 1.02, 0.95)
    };
    let planet_oblateness = if show_atmospheres
        && !is_star
        && matches!(body.body_type, BodyType::GasGiant | BodyType::IceGiant)
    {
        0.098
    } else {
        0.0
    };

    RingVisualGeometry {
        outer_scale: ring_outer_scale,
        inner_ratio_mesh,
        star_dir_local,
        star_dir_world,
        planet_radius_ratio,
        planet_oblateness,
    }
}

fn update_existing_ring_children(
    opt_children: Option<&Children>,
    ring_children_query: &mut Query<
        (&mut Transform, &MeshMaterial3d<RingMaterial>),
        (With<VisualRingChild>, Without<CelestialBody>),
    >,
    ring_materials: &mut Assets<RingMaterial>,
    ring_sys: &PlanetaryRingSystem,
    geom: &RingVisualGeometry,
    elapsed_secs: f32,
) -> bool {
    let mut found = false;
    if let Some(children) = opt_children {
        for child in children.iter() {
            if let Ok((mut transform, mat_handle)) = ring_children_query.get_mut(child) {
                found = true;
                let target_scale = Vec3::new(geom.outer_scale, 1.0, geom.outer_scale);
                if transform.scale != target_scale {
                    transform.scale = target_scale;
                }
                if transform.rotation != Quat::IDENTITY {
                    transform.rotation = Quat::IDENTITY;
                }

                let target_uniforms = RingUniforms {
                    inner_radius: geom.inner_ratio_mesh,
                    outer_radius: 1.0,
                    optical_depth: ring_sys.optical_depth,
                    ice_fraction: ring_sys.ice_fraction,
                    ring_color: calc_ring_color(ring_sys.ice_fraction),
                    star_dir_local: Vec4::new(
                        geom.star_dir_local.x,
                        geom.star_dir_local.y,
                        geom.star_dir_local.z,
                        geom.planet_radius_ratio,
                    ),
                    shadow_params: Vec4::new(0.005, 0.007, 0.995, elapsed_secs),
                    star_dir_world_and_oblateness: Vec4::new(
                        geom.star_dir_world.x,
                        geom.star_dir_world.y,
                        geom.star_dir_world.z,
                        geom.planet_oblateness,
                    ),
                };
                if let Some(mat) = ring_materials.get(&mat_handle.0)
                    && mat.uniforms != target_uniforms
                    && let Some(mut mat_mut) = ring_materials.get_mut(&mat_handle.0)
                {
                    mat_mut.uniforms = target_uniforms;
                }
            }
        }
    }
    found
}

fn spawn_planetary_ring_child(
    commands: &mut Commands,
    planet_entity: Entity,
    visual_assets: &VisualAssets,
    ring_materials: &mut Assets<RingMaterial>,
    ring_sys: &PlanetaryRingSystem,
    geom: &RingVisualGeometry,
    elapsed_secs: f32,
) {
    let ring_color = calc_ring_color(ring_sys.ice_fraction);
    let material = ring_materials.add(RingMaterial {
        uniforms: RingUniforms {
            inner_radius: geom.inner_ratio_mesh,
            outer_radius: 1.0,
            optical_depth: ring_sys.optical_depth,
            ice_fraction: ring_sys.ice_fraction,
            ring_color,
            star_dir_local: Vec4::new(
                geom.star_dir_local.x,
                geom.star_dir_local.y,
                geom.star_dir_local.z,
                geom.planet_radius_ratio,
            ),
            shadow_params: Vec4::new(0.005, 0.007, 0.995, elapsed_secs),
            star_dir_world_and_oblateness: Vec4::new(
                geom.star_dir_world.x,
                geom.star_dir_world.y,
                geom.star_dir_world.z,
                geom.planet_oblateness,
            ),
        },
    });

    if let Ok(mut p_cmd) = commands.get_entity(planet_entity) {
        p_cmd.with_children(|parent| {
            parent.spawn((
                VisualRingChild,
                Mesh3d(visual_assets.ring_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_scale(Vec3::new(geom.outer_scale, 1.0, geom.outer_scale))
                    .with_rotation(Quat::IDENTITY),
                Visibility::default(),
                NotShadowCaster,
            ));
        });
    }
}

/// Synchronizes 3D planetary ring system meshes, materials, and axial tilt transforms.
#[allow(clippy::type_complexity, reason = "Planetary ring query")]
pub fn sync_planetary_rings(
    mut commands: Commands,
    (time, sim_time, opt_player_state): (
        Res<Time>,
        Option<Res<SimTime>>,
        Option<Res<PlayerInteractionState>>,
    ),
    _config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut ring_materials: ResMut<Assets<RingMaterial>>,
    planets_with_rings_query: Query<
        (
            Entity,
            &PlanetaryRingSystem,
            &Radius,
            &CelestialBody,
            &Transform,
            Option<&Mass>,
            Option<&Composition>,
            Option<&SpinState>,
            Option<&InternalDifferentiation>,
            Option<&Children>,
        ),
        With<VisualBody>,
    >,
    stars_query: Query<(&SimPosition, &CelestialBody)>,
    mut ring_children_query: Query<
        (&mut Transform, &MeshMaterial3d<RingMaterial>),
        (With<VisualRingChild>, Without<CelestialBody>),
    >,
) {
    let elapsed_secs = sim_time
        .as_deref()
        .map_or_else(|| time.elapsed_secs(), |st| st.visual_time_secs);
    let show_atmospheres = opt_player_state
        .as_deref()
        .is_none_or(PlayerInteractionState::show_atmospheres);
    let stars: Vec<Vec3> = stars_query
        .iter()
        .filter(|(_, b)| b.body_type.is_star_or_remnant())
        .map(|(pos, _)| Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32))
        .collect();

    for (
        planet_entity,
        ring_sys,
        radius,
        body,
        planet_trans,
        opt_mass,
        opt_comp,
        _opt_spin,
        opt_diff,
        opt_children,
    ) in planets_with_rings_query.iter()
    {
        if body.body_type == BodyType::BlackHole {
            if let Some(children) = opt_children {
                for child in children.iter() {
                    if ring_children_query.get(child).is_ok()
                        && let Ok(mut cmd) = commands.get_entity(child)
                    {
                        cmd.try_despawn();
                    }
                }
            }
            continue;
        }

        let star_world_pos = stars
            .iter()
            .copied()
            .filter(|&sp| sp.distance_squared(planet_trans.translation) > 1e-8)
            .min_by(|a, b| {
                a.distance_squared(planet_trans.translation)
                    .total_cmp(&b.distance_squared(planet_trans.translation))
            })
            .unwrap_or(Vec3::ZERO);

        let geom = compute_ring_geometry(
            body,
            ring_sys,
            radius,
            opt_mass,
            opt_comp,
            opt_diff,
            show_atmospheres,
            planet_trans,
            star_world_pos,
        );

        let found_child = update_existing_ring_children(
            opt_children,
            &mut ring_children_query,
            &mut ring_materials,
            ring_sys,
            &geom,
            elapsed_secs,
        );

        if !found_child {
            spawn_planetary_ring_child(
                &mut commands,
                planet_entity,
                &visual_assets,
                &mut ring_materials,
                ring_sys,
                &geom,
                elapsed_secs,
            );
        }
    }
}

/// Marker component for an instantiated visual accretion disk entity attached to a black hole.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct VisualBlackHoleDiskChild;
/// Synchronizes relativistic accretion disks, Keplerian Doppler beaming, and gravitational lensing on black holes.
#[allow(clippy::type_complexity, reason = "Black hole accretion disk query")]
pub fn sync_black_hole_accretion_disks(
    mut commands: Commands,
    sim_time: Option<Res<SimTime>>,
    visual_assets: Option<Res<VisualAssets>>,
    mut disk_materials: ResMut<Assets<BlackHoleDiskMaterial>>,
    camera_query: Query<
        &Transform,
        (
            With<crate::rendering::camera::PanOrbitCamera>,
            Without<VisualBlackHoleDiskChild>,
        ),
    >,
    black_holes_query: Query<
        (
            Entity,
            &Mass,
            &Radius,
            &CelestialBody,
            &Transform,
            Option<&SpinState>,
            Option<&RelativisticJetState>,
            Option<&crate::simulation::relativity::kerr::KerrBlackHole>,
            Option<&BlackHoleStarState>,
            Option<&Children>,
        ),
        (With<VisualBody>, Without<VisualBlackHoleDiskChild>),
    >,
    mut disk_children_query: Query<
        (
            &mut Transform,
            &MeshMaterial3d<BlackHoleDiskMaterial>,
            Option<&mut RectLight>,
        ),
        (
            With<VisualBlackHoleDiskChild>,
            Without<CelestialBody>,
            Without<crate::rendering::camera::PanOrbitCamera>,
        ),
    >,
) {
    let Some(assets) = visual_assets else {
        return;
    };
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let cam_pos = camera_query
        .iter()
        .next()
        .map_or(Vec3::new(0.0, 5.0, 10.0), |t| t.translation);

    for (
        bh_entity,
        mass,
        _radius,
        body,
        bh_trans,
        _opt_spin,
        opt_jet,
        opt_kerr,
        opt_bhs,
        opt_children,
    ) in black_holes_query.iter()
    {
        let is_black_hole = body.body_type == BodyType::BlackHole
            || (body.body_type == BodyType::QuasiStar && opt_bhs.is_some_and(|s| s.is_blown_out));
        if !is_black_hole {
            if let Some(children) = opt_children {
                for child in children.iter() {
                    if disk_children_query.get(child).is_ok()
                        && let Ok(mut c_cmd) = commands.get_entity(child)
                    {
                        c_cmd.try_despawn();
                    }
                }
            }
            continue;
        }

        let (target_transform, uniforms) = calculate_disk_geometry_and_uniforms(
            cam_pos,
            bh_trans,
            mass,
            opt_jet,
            opt_kerr,
            visual_time,
        );

        let mut found_child = false;
        if let Some(children) = opt_children {
            for child in children.iter() {
                if let Ok((mut transform, mat_handle, mut opt_light)) =
                    disk_children_query.get_mut(child)
                {
                    found_child = true;
                    if *transform != target_transform {
                        *transform = target_transform;
                    }
                    if let Some(mat) = disk_materials.get(&mat_handle.0)
                        && mat.uniforms != uniforms
                        && let Some(mut mat_mut) = disk_materials.get_mut(&mat_handle.0)
                    {
                        mat_mut.uniforms = uniforms.clone();
                    }
                    if let Some(ref mut light) = opt_light {
                        let target_range = (target_transform.scale.x * 2.5).max(1.0);
                        let target_width = (target_transform.scale.x * 1.5).max(0.5);
                        let target_height = (target_transform.scale.z * 1.5).max(0.5);
                        if (light.range - target_range).abs() > 1e-4 {
                            light.range = target_range;
                        }
                        if (light.width - target_width).abs() > 1e-4 {
                            light.width = target_width;
                        }
                        if (light.height - target_height).abs() > 1e-4 {
                            light.height = target_height;
                        }
                    }
                }
            }
        }

        if !found_child {
            let material = disk_materials.add(BlackHoleDiskMaterial { uniforms });

            if let Ok(mut bh_cmd) = commands.get_entity(bh_entity) {
                bh_cmd.with_children(|parent| {
                    parent.spawn((
                        VisualBlackHoleDiskChild,
                        Mesh3d(assets.accretion_disk_mesh.clone()),
                        MeshMaterial3d(material),
                        target_transform,
                        NotShadowCaster,
                        RectLight {
                            color: Color::srgb(1.0, 0.78, 0.45),
                            intensity: 25_000.0,
                            range: (target_transform.scale.x * 2.5).max(1.0),
                            width: (target_transform.scale.x * 1.5).max(0.5),
                            height: (target_transform.scale.z * 1.5).max(0.5),
                        },
                    ));
                });
            }
        }
    }
}

fn calculate_disk_geometry_and_uniforms(
    cam_pos: Vec3,
    bh_trans: &Transform,
    mass: &Mass,
    opt_jet: Option<&RelativisticJetState>,
    opt_kerr: Option<&crate::simulation::relativity::kerr::KerrBlackHole>,
    visual_time: f32,
) -> (Transform, BlackHoleDiskUniforms) {
    let disk_scale = 12.0f32;
    let accretion_rate = opt_jet.map_or(1.0, |jet| {
        if jet.is_accreting {
            1.0
        } else {
            (jet.accretion_timer_years / 5.0).clamp(0.4, 1.0)
        }
    });

    let world_spin_axis = opt_kerr.map_or(Vec3::Y, |k| {
        let v = k.spin_axis.as_vec3();
        if v.length_squared() > 0.5 {
            v.normalize()
        } else {
            Vec3::Y
        }
    });
    let spin_axis = (bh_trans.rotation.inverse() * world_spin_axis).normalize_or(Vec3::Y);

    let cam_dir_world = (cam_pos - bh_trans.translation).normalize_or(Vec3::Z);
    let cam_dir_local = (bh_trans.rotation.inverse() * cam_dir_world).normalize_or(Vec3::Z);

    let mut u_right = spin_axis.cross(cam_dir_local);
    if u_right.length_squared() < 1e-6 {
        let fallback = if cam_dir_local.dot(Vec3::X).abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };
        u_right = fallback.cross(cam_dir_local).normalize();
    } else {
        u_right = u_right.normalize();
    }

    let z_screen_col = u_right.cross(cam_dir_local).normalize();
    let disk_rotation = Quat::from_mat3(&Mat3::from_cols(u_right, cam_dir_local, z_screen_col));
    let cos_theta = cam_dir_local.dot(spin_axis).clamp(-1.0, 1.0);

    let target_transform = Transform::from_translation(cam_dir_local * 0.001)
        .with_scale(Vec3::new(disk_scale, 1.0, disk_scale))
        .with_rotation(disk_rotation);

    let (inner_radius, schwa_radius, spin_axis_w) = if let Some(kerr) = opt_kerr {
        let a_star = kerr.spin_param as f32;
        let a2 = (a_star * a_star).min(0.9998);
        let r_plus_ratio = f32::midpoint(1.0, (1.0 - a2).max(0.0).sqrt());
        let m = mass.0.max(1.0);
        let r_isco_kerr =
            crate::simulation::relativity::kerr::calculate_kerr_isco_radius(m, kerr.spin_param);
        let r_isco_schwa = crate::simulation::relativity::post_newtonian::calculate_isco_radius(m);
        let isco_ratio = (r_isco_kerr / r_isco_schwa.max(1e-6)) as f32;
        let r_inner = (0.16 * isco_ratio).clamp(0.055 * r_plus_ratio * 1.05, 0.45);
        (r_inner, 0.055 * r_plus_ratio, a_star)
    } else {
        (0.16, 0.055, 0.0)
    };

    let uniforms = BlackHoleDiskUniforms {
        inner_radius,
        outer_radius: 1.0,
        schwa_radius,
        time: visual_time,
        disk_color: Vec4::new(1.0, 0.90, 0.70, 1.0),
        spin_axis: Vec4::new(spin_axis.x, spin_axis.y, spin_axis.z, spin_axis_w),
        cam_dir_local: Vec4::new(cam_dir_local.x, cam_dir_local.y, cam_dir_local.z, cos_theta),
        cam_up_local: Vec4::new(
            z_screen_col.x,
            z_screen_col.y,
            z_screen_col.z,
            accretion_rate,
        ),
    };

    (target_transform, uniforms)
}
