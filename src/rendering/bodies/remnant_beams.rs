//! Volumetric relativistic beams and magnetospheric structures for quasars, pulsars, and magnetars.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::VisualAssets;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructureHostTarget(pub Entity);

fn resolve_beam_spin_axis_rotation(opt_spin: Option<&SpinState>) -> Quat {
    let spin_dir = if let Some(spin) = opt_spin {
        if spin.spin_vector.length_squared() > 1e-12 {
            spin.spin_vector.normalize().as_vec3()
        } else {
            let tilt = (spin.axial_tilt_degrees as f32).to_radians();
            Vec3::new(tilt.sin(), tilt.cos(), 0.0)
        }
    } else {
        Vec3::Y
    };
    Quat::from_rotation_arc(Vec3::Y, spin_dir.normalize_or_zero())
}

/// Root marker for the 3D Quasar Relativistic Jet Laser Beam system.
#[derive(Component, Debug, Clone, Copy)]
pub struct QuasarBeamRoot;

/// Sub-parts of the 3D Quasar Beam hierarchy.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuasarBeamPart {
    NorthCore,
    NorthSheath,
    NorthLobe,
    SouthCore,
    SouthSheath,
    SouthLobe,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Quasar beam transform sync helper"
)]
fn update_quasar_beam_transforms(
    root_trans: &mut Transform,
    opt_children: Option<&Children>,
    part_query: &mut Query<(&mut Transform, &QuasarBeamPart), Without<QuasarBeamRoot>>,
    world_pos: Vec3,
    root_rot: Quat,
    beam_center: f32,
    beam_len: f32,
    jet_len: f32,
    core_r: f32,
    sheath_r: f32,
    lobe_r: f32,
) {
    if root_trans.translation != world_pos {
        root_trans.translation = world_pos;
    }
    if root_trans.rotation != root_rot {
        root_trans.rotation = root_rot;
    }
    let Some(children) = opt_children else {
        return;
    };
    for child in children.iter() {
        if let Ok((mut part_trans, part)) = part_query.get_mut(child) {
            let (target_translation, target_scale) = match part {
                QuasarBeamPart::NorthCore => (
                    Vec3::new(0.0, beam_center, 0.0),
                    Vec3::new(core_r, beam_len, core_r),
                ),
                QuasarBeamPart::NorthSheath => (
                    Vec3::new(0.0, beam_center, 0.0),
                    Vec3::new(sheath_r, beam_len, sheath_r),
                ),
                QuasarBeamPart::NorthLobe => (Vec3::new(0.0, jet_len, 0.0), Vec3::splat(lobe_r)),
                QuasarBeamPart::SouthCore => (
                    Vec3::new(0.0, -beam_center, 0.0),
                    Vec3::new(core_r, beam_len, core_r),
                ),
                QuasarBeamPart::SouthSheath => (
                    Vec3::new(0.0, -beam_center, 0.0),
                    Vec3::new(sheath_r, beam_len, sheath_r),
                ),
                QuasarBeamPart::SouthLobe => (Vec3::new(0.0, -jet_len, 0.0), Vec3::splat(lobe_r)),
            };
            if part_trans.translation != target_translation {
                part_trans.translation = target_translation;
            }
            if part_trans.scale != target_scale {
                part_trans.scale = target_scale;
            }
        }
    }
}

#[allow(clippy::too_many_arguments, reason = "Quasar beam spawn helper")]
fn spawn_quasar_beam_hierarchy(
    commands: &mut Commands,
    assets: &VisualAssets,
    materials: &mut Assets<StandardMaterial>,
    target_entity: Entity,
    world_pos: Vec3,
    root_rot: Quat,
    beam_center: f32,
    beam_len: f32,
    jet_len: f32,
    core_r: f32,
    sheath_r: f32,
    lobe_r: f32,
) {
    let core_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(75.0, 75.0, 90.0, 1.0),
        unlit: true,
        ..default()
    });
    let sheath_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.20, 0.75, 1.0, 0.22),
        emissive: LinearRgba::new(2.5, 8.0, 20.0, 0.30),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let lobe_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.40, 0.85, 1.0, 0.60),
        emissive: LinearRgba::new(8.0, 18.0, 35.0, 0.6),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            QuasarBeamRoot,
            StructureHostTarget(target_entity),
            Transform::from_translation(world_pos).with_rotation(root_rot),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                QuasarBeamPart::NorthCore,
                Mesh3d(assets.beam_core_mesh.clone()),
                MeshMaterial3d(core_mat.clone()),
                Transform::from_xyz(0.0, beam_center, 0.0)
                    .with_scale(Vec3::new(core_r, beam_len, core_r)),
                NotShadowCaster,
            ));
            parent.spawn((
                QuasarBeamPart::NorthSheath,
                Mesh3d(assets.beam_sheath_mesh.clone()),
                MeshMaterial3d(sheath_mat.clone()),
                Transform::from_xyz(0.0, beam_center, 0.0)
                    .with_scale(Vec3::new(sheath_r, beam_len, sheath_r)),
                NotShadowCaster,
            ));
            parent.spawn((
                QuasarBeamPart::NorthLobe,
                Mesh3d(assets.star_mesh.clone()),
                MeshMaterial3d(lobe_mat.clone()),
                Transform::from_xyz(0.0, jet_len, 0.0).with_scale(Vec3::splat(lobe_r)),
                NotShadowCaster,
            ));

            parent.spawn((
                QuasarBeamPart::SouthCore,
                Mesh3d(assets.beam_core_mesh.clone()),
                MeshMaterial3d(core_mat),
                Transform::from_xyz(0.0, -beam_center, 0.0)
                    .with_scale(Vec3::new(core_r, beam_len, core_r)),
                NotShadowCaster,
            ));
            parent.spawn((
                QuasarBeamPart::SouthSheath,
                Mesh3d(assets.beam_sheath_mesh.clone()),
                MeshMaterial3d(sheath_mat),
                Transform::from_xyz(0.0, -beam_center, 0.0)
                    .with_scale(Vec3::new(sheath_r, beam_len, sheath_r)),
                NotShadowCaster,
            ));
            parent.spawn((
                QuasarBeamPart::SouthLobe,
                Mesh3d(assets.star_mesh.clone()),
                MeshMaterial3d(lobe_mat),
                Transform::from_xyz(0.0, -jet_len, 0.0).with_scale(Vec3::splat(lobe_r)),
                NotShadowCaster,
            ));
        });
}

#[inline]
fn resolve_beam_host_visual_radius(
    opt_transform: Option<&Transform>,
    physical_radius_au: f64,
    body_type: BodyType,
    config: &SimulationConfig,
) -> f32 {
    if let Some(tf) = opt_transform {
        let sx = tf.scale.x;
        let is_unsynced_default = (sx - 1.0).abs() < 1e-6 && physical_radius_au < 0.25;
        if sx.is_finite() && sx > 1e-6 && !is_unsynced_default {
            return sx;
        }
    }
    config.calc_visual_radius_for_type(physical_radius_au, body_type)
}

/// Synchronizes 3D volumetric laser beam columns for active Quasars / Black Hole Stars.
#[allow(clippy::type_complexity, reason = "Quasar Beam Sync")]
pub fn sync_quasar_beams(
    mut commands: Commands,
    visual_assets: Option<Res<VisualAssets>>,
    config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    quasi_query: Query<
        (
            Entity,
            &SimPosition,
            &CelestialBody,
            &Mass,
            &Radius,
            Option<&BlackHoleStarState>,
            Option<&SpinState>,
            Option<&Transform>,
        ),
        (Without<QuasarBeamRoot>, Without<QuasarBeamPart>),
    >,
    mut root_query: Query<
        (
            Entity,
            &StructureHostTarget,
            &mut Transform,
            Option<&Children>,
        ),
        With<QuasarBeamRoot>,
    >,
    mut part_query: Query<(&mut Transform, &QuasarBeamPart), Without<QuasarBeamRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    let mut active_targets = Vec::new();
    for (target_entity, pos, body, mass, radius, opt_qs, opt_spin, opt_tf) in quasi_query.iter() {
        let is_quasi = opt_qs.is_some()
            || body.body_type == BodyType::QuasiStar
            || body.name.contains("Quasar")
            || (body.body_type == BodyType::BlackHole && mass.0 > 500.0);
        if !is_quasi {
            continue;
        }
        let is_blown_out = opt_qs.is_some_and(|qs| qs.is_blown_out) || body.name.contains("Quasar");
        let fallback_dist = if body.name.contains("Quasar") {
            (radius.0 as f32 * 180.0).clamp(1.5, 12.0)
        } else {
            0.0
        };
        let light_dist = opt_qs.map_or(fallback_dist, |qs| qs.jet_travel_distance_au as f32);
        if !is_blown_out || light_dist <= 0.1 {
            continue;
        }
        active_targets.push(target_entity);

        let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let root_rot = resolve_beam_spin_axis_rotation(opt_spin);
        let jet_len = light_dist;
        let current_visual_radius =
            resolve_beam_host_visual_radius(opt_tf, radius.0, body.body_type, &config);
        let pole_start = (current_visual_radius * 0.90).max(0.002);
        let beam_len = (jet_len - pole_start).max(0.1);
        let beam_center = pole_start + beam_len * 0.5;
        let core_r = (current_visual_radius * 0.12).clamp(0.002, 0.04);
        let sheath_r = (current_visual_radius * 0.28).clamp(0.005, 0.09);
        let lobe_r = (current_visual_radius * 0.85).clamp(0.02, 0.35);

        if let Some((_, _, mut root_trans, opt_children)) = root_query
            .iter_mut()
            .find(|(_, host, ..)| host.0 == target_entity)
        {
            update_quasar_beam_transforms(
                &mut root_trans,
                opt_children,
                &mut part_query,
                world_pos,
                root_rot,
                beam_center,
                beam_len,
                jet_len,
                core_r,
                sheath_r,
                lobe_r,
            );
        } else {
            spawn_quasar_beam_hierarchy(
                &mut commands,
                &assets,
                &mut materials,
                target_entity,
                world_pos,
                root_rot,
                beam_center,
                beam_len,
                jet_len,
                core_r,
                sheath_r,
                lobe_r,
            );
        }
    }

    for (root_ent, host, ..) in root_query.iter() {
        if !active_targets.contains(&host.0)
            && let Ok(mut cmd) = commands.get_entity(root_ent)
        {
            cmd.try_despawn();
        }
    }
}

/// Root marker for the 3D Pulsar Relativistic Lighthouse Beam system.
#[derive(Component, Debug, Clone, Copy)]
pub struct PulsarBeamRoot;

/// Sub-parts of the 3D Pulsar Beam hierarchy.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulsarBeamPart {
    NorthBeam,
    SouthBeam,
}

/// Synchronizes 3D volumetric relativistic lighthouse beams for active Pulsars.
#[allow(clippy::type_complexity, reason = "Pulsar Beam Sync")]
pub fn sync_pulsar_beams(
    mut commands: Commands,
    sim_time: Option<Res<SimTime>>,
    visual_assets: Option<Res<VisualAssets>>,
    config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pulsar_query: Query<
        (
            Entity,
            &SimPosition,
            &CelestialBody,
            &Radius,
            Option<&SpinState>,
            Option<&Transform>,
        ),
        (Without<PulsarBeamRoot>, Without<PulsarBeamPart>),
    >,
    mut root_query: Query<
        (
            Entity,
            &StructureHostTarget,
            &mut Transform,
            Option<&Children>,
        ),
        With<PulsarBeamRoot>,
    >,
    mut part_query: Query<(&mut Transform, &PulsarBeamPart), Without<PulsarBeamRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let mut active_targets = Vec::new();

    for (target_entity, pos, body, radius, opt_spin, opt_tf) in pulsar_query.iter() {
        if body.body_type != BodyType::Pulsar && !body.name.contains("Pulsar") {
            continue;
        }
        active_targets.push(target_entity);

        let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let axis_rot = resolve_beam_spin_axis_rotation(opt_spin);
        let spin_rate = opt_spin.map_or(24.0, |s| {
            (24.0 / (s.rotation_period_hours.abs().max(0.001) as f32)).clamp(2.0, 60.0)
        });
        let beam_rot =
            axis_rot * Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.38);
        let current_visual_radius =
            resolve_beam_host_visual_radius(opt_tf, radius.0, body.body_type, &config);
        let pole_start = (current_visual_radius * 0.92).max(0.0005);

        if let Some((_, _, mut root_trans, opt_children)) = root_query
            .iter_mut()
            .find(|(_, host, ..)| host.0 == target_entity)
        {
            if root_trans.translation != world_pos {
                root_trans.translation = world_pos;
            }
            if root_trans.rotation != beam_rot {
                root_trans.rotation = beam_rot;
            }
            if let Some(children) = opt_children {
                for child in children.iter() {
                    if let Ok((mut part_trans, part)) = part_query.get_mut(child) {
                        match part {
                            PulsarBeamPart::NorthBeam => {
                                let target_translation = Vec3::new(0.0, pole_start, 0.0);
                                if part_trans.translation != target_translation {
                                    part_trans.translation = target_translation;
                                }
                            }
                            PulsarBeamPart::SouthBeam => {
                                let target_translation = Vec3::new(0.0, -pole_start, 0.0);
                                let target_rot = Quat::from_rotation_x(std::f32::consts::PI);
                                if part_trans.translation != target_translation {
                                    part_trans.translation = target_translation;
                                }
                                if part_trans.rotation != target_rot {
                                    part_trans.rotation = target_rot;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let beam_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.85, 0.95, 1.0, 0.70),
                emissive: LinearRgba::new(55.0, 70.0, 95.0, 0.70),
                alpha_mode: AlphaMode::Add,
                cull_mode: None,
                unlit: true,
                ..default()
            });

            commands
                .spawn((
                    PulsarBeamRoot,
                    StructureHostTarget(target_entity),
                    Transform::from_translation(world_pos).with_rotation(beam_rot),
                    Visibility::default(),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        PulsarBeamPart::NorthBeam,
                        Mesh3d(assets.pulsar_beam_mesh.clone()),
                        MeshMaterial3d(beam_mat.clone()),
                        Transform::from_xyz(0.0, pole_start, 0.0),
                        NotShadowCaster,
                    ));
                    parent.spawn((
                        PulsarBeamPart::SouthBeam,
                        Mesh3d(assets.pulsar_beam_mesh.clone()),
                        MeshMaterial3d(beam_mat),
                        Transform::from_xyz(0.0, -pole_start, 0.0)
                            .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
                        NotShadowCaster,
                    ));
                });
        }
    }

    for (root_ent, host, ..) in root_query.iter() {
        if !active_targets.contains(&host.0)
            && let Ok(mut cmd) = commands.get_entity(root_ent)
        {
            cmd.try_despawn();
        }
    }
}

/// Root marker for the 3D Magnetar Magnetic Arches and Equatorial Ring system.
#[derive(Component, Debug, Clone, Copy)]
pub struct MagnetarStructureRoot;

/// Sub-parts of the 3D Magnetar Structure hierarchy.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagnetarStructurePart {
    EquatorialRing,
    MagneticFieldLoops,
}

/// Synchronizes 3D volumetric magnetic field loops and incandescent equatorial ring for active Magnetars.
#[allow(clippy::type_complexity, reason = "Magnetar Structure Sync")]
pub fn sync_magnetar_structures(
    mut commands: Commands,
    sim_time: Option<Res<SimTime>>,
    visual_assets: Option<Res<VisualAssets>>,
    _config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    magnetar_query: Query<
        (
            Entity,
            &SimPosition,
            &CelestialBody,
            Option<&ElectromagneticFieldState>,
            Option<&SpinState>,
        ),
        (
            Without<MagnetarStructureRoot>,
            Without<MagnetarStructurePart>,
        ),
    >,
    mut root_query: Query<
        (Entity, &StructureHostTarget, &mut Transform),
        With<MagnetarStructureRoot>,
    >,
    _part_query: Query<(&mut Transform, &MagnetarStructurePart), Without<MagnetarStructureRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let spin_rate = 1.2;
    let mut active_targets = Vec::new();

    for (target_entity, pos, body, opt_em, opt_spin) in magnetar_query.iter() {
        let is_magnetar = body.body_type == BodyType::Magnetar
            || opt_em.is_some_and(|em| em.magnetic_field_gauss >= 1.0e14)
            || (crate::utils::math::contains_ignore_ascii_case(&body.name, "magnetar")
                && !body.name.contains("Clump")
                && !body.name.contains("Ejecta"));
        if !is_magnetar {
            continue;
        }
        active_targets.push(target_entity);

        let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let axis_rot = resolve_beam_spin_axis_rotation(opt_spin);
        let mag_rot =
            axis_rot * Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.26);

        if let Some((_, _, mut root_trans)) = root_query
            .iter_mut()
            .find(|(_, host, _)| host.0 == target_entity)
        {
            if root_trans.translation != world_pos {
                root_trans.translation = world_pos;
            }
            if root_trans.rotation != mag_rot {
                root_trans.rotation = mag_rot;
            }
        } else {
            let ring_mat = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                emissive: LinearRgba::new(20.0, 45.0, 75.0, 0.85),
                alpha_mode: AlphaMode::Add,
                cull_mode: None,
                unlit: true,
                ..default()
            });

            let loops_mat = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                emissive: LinearRgba::new(6.0, 22.0, 42.0, 0.85),
                alpha_mode: AlphaMode::Add,
                cull_mode: None,
                unlit: true,
                ..default()
            });

            commands
                .spawn((
                    MagnetarStructureRoot,
                    StructureHostTarget(target_entity),
                    Transform::from_translation(world_pos).with_rotation(mag_rot),
                    Visibility::default(),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        MagnetarStructurePart::EquatorialRing,
                        Mesh3d(assets.magnetar_ring_mesh.clone()),
                        MeshMaterial3d(ring_mat),
                        Transform::IDENTITY,
                        NotShadowCaster,
                    ));
                    parent.spawn((
                        MagnetarStructurePart::MagneticFieldLoops,
                        Mesh3d(assets.magnetar_field_loops_mesh.clone()),
                        MeshMaterial3d(loops_mat),
                        Transform::IDENTITY,
                        NotShadowCaster,
                    ));
                });
        }
    }

    for (root_ent, host, _) in root_query.iter() {
        if !active_targets.contains(&host.0)
            && let Ok(mut cmd) = commands.get_entity(root_ent)
        {
            cmd.try_despawn();
        }
    }
}
