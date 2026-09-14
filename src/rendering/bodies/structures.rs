use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::palettes::calc_ring_color;
use super::VisualAssets;

/// Marker component for an instantiated visual planetary ring entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualRingChild;

/// Synchronizes 3D planetary ring system meshes, materials, and axial tilt transforms.
pub fn sync_planetary_rings(
    mut commands: Commands,
    _config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut ring_materials: ResMut<Assets<RingMaterial>>,
    planets_with_rings_query: Query<(
        Entity,
        &PlanetaryRingSystem,
        &Radius,
        &CelestialBody,
        Option<&SpinState>,
        Option<&Children>,
    )>,
    mut ring_children_query: Query<
        (&mut Transform, &MeshMaterial3d<RingMaterial>),
        With<VisualRingChild>,
    >,
) {
    for (planet_entity, ring_sys, radius, _body, opt_spin, opt_children) in
        planets_with_rings_query.iter()
    {
        let ring_ratio = if ring_sys.outer_radius_au > 0.0 && radius.0 > 0.0 {
            (ring_sys.outer_radius_au / radius.0 as f32).clamp(2.0, 3.5)
        } else {
            2.85
        };
        // The parent planet mesh is a unit sphere (radius 1.0) scaled by planet_render_rad.
        // The ring mesh is a Plane3d (size 1.0x1.0, half-width 0.5).
        // Since child local transform scales relative to parent, a local XZ scale of ring_ratio * 2.0
        // extends the ring mesh to exactly ring_ratio times the planet's visual radius.
        let ring_outer_scale = ring_ratio * 2.0;

        let tilt_degrees = opt_spin.map_or(26.7, |s| s.axial_tilt_degrees as f32);
        let ring_rotation = Quat::from_rotation_z(tilt_degrees.to_radians());

        let mut found_child = false;
        if let Some(children) = opt_children {
            for child in children.iter() {
                if let Ok((mut transform, mat_handle)) = ring_children_query.get_mut(child) {
                    found_child = true;
                    transform.scale = Vec3::new(ring_outer_scale, 1.0, ring_outer_scale);
                    transform.rotation = ring_rotation;

                    if let Some(mut mat) = ring_materials.get_mut(&mat_handle.0) {
                        mat.uniforms.inner_radius = ring_sys.inner_radius_au;
                        mat.uniforms.outer_radius = ring_sys.outer_radius_au;
                        mat.uniforms.optical_depth = ring_sys.optical_depth;
                        mat.uniforms.ice_fraction = ring_sys.ice_fraction;
                        mat.uniforms.ring_color = calc_ring_color(ring_sys.ice_fraction);
                    }
                }
            }
        }

        if !found_child {
            let ring_color = calc_ring_color(ring_sys.ice_fraction);
            let material = ring_materials.add(RingMaterial {
                uniforms: RingUniforms {
                    inner_radius: ring_sys.inner_radius_au,
                    outer_radius: ring_sys.outer_radius_au,
                    optical_depth: ring_sys.optical_depth,
                    ice_fraction: ring_sys.ice_fraction,
                    ring_color,
                },
            });

            if let Ok(mut p_cmd) = commands.get_entity(planet_entity) {
                p_cmd.with_children(|parent| {
                    parent.spawn((
                        VisualRingChild,
                        Mesh3d(visual_assets.ring_mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::from_scale(Vec3::new(ring_outer_scale, 1.0, ring_outer_scale))
                            .with_rotation(ring_rotation),
                        NotShadowCaster,
                    ));
                });
            }
        }
    }
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

fn update_quasar_beam_transforms(
    root_trans: &mut Transform,
    part_query: &mut Query<(&mut Transform, &QuasarBeamPart), Without<QuasarBeamRoot>>,
    world_pos: Vec3,
    beam_center: f32,
    beam_len: f32,
    jet_len: f32,
    core_r: f32,
    sheath_r: f32,
    lobe_r: f32,
) {
    root_trans.translation = world_pos;
    for (mut part_trans, part) in part_query.iter_mut() {
        match part {
            QuasarBeamPart::NorthCore => {
                part_trans.translation = Vec3::new(0.0, beam_center, 0.0);
                part_trans.scale = Vec3::new(core_r, beam_len, core_r);
            }
            QuasarBeamPart::NorthSheath => {
                part_trans.translation = Vec3::new(0.0, beam_center, 0.0);
                part_trans.scale = Vec3::new(sheath_r, beam_len, sheath_r);
            }
            QuasarBeamPart::NorthLobe => {
                part_trans.translation = Vec3::new(0.0, jet_len, 0.0);
                part_trans.scale = Vec3::splat(lobe_r);
            }
            QuasarBeamPart::SouthCore => {
                part_trans.translation = Vec3::new(0.0, -beam_center, 0.0);
                part_trans.scale = Vec3::new(core_r, beam_len, core_r);
            }
            QuasarBeamPart::SouthSheath => {
                part_trans.translation = Vec3::new(0.0, -beam_center, 0.0);
                part_trans.scale = Vec3::new(sheath_r, beam_len, sheath_r);
            }
            QuasarBeamPart::SouthLobe => {
                part_trans.translation = Vec3::new(0.0, -jet_len, 0.0);
                part_trans.scale = Vec3::splat(lobe_r);
            }
        }
    }
}

fn spawn_quasar_beam_hierarchy(
    commands: &mut Commands,
    assets: &VisualAssets,
    materials: &mut Assets<StandardMaterial>,
    world_pos: Vec3,
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
            Transform::from_translation(world_pos),
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
                MeshMaterial3d(core_mat.clone()),
                Transform::from_xyz(0.0, -beam_center, 0.0)
                    .with_scale(Vec3::new(core_r, beam_len, core_r)),
                NotShadowCaster,
            ));
            parent.spawn((
                QuasarBeamPart::SouthSheath,
                Mesh3d(assets.beam_sheath_mesh.clone()),
                MeshMaterial3d(sheath_mat.clone()),
                Transform::from_xyz(0.0, -beam_center, 0.0)
                    .with_scale(Vec3::new(sheath_r, beam_len, sheath_r)),
                NotShadowCaster,
            ));
            parent.spawn((
                QuasarBeamPart::SouthLobe,
                Mesh3d(assets.star_mesh.clone()),
                MeshMaterial3d(lobe_mat.clone()),
                Transform::from_xyz(0.0, -jet_len, 0.0).with_scale(Vec3::splat(lobe_r)),
                NotShadowCaster,
            ));
        });
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
            &SimPosition,
            &CelestialBody,
            &Mass,
            &Radius,
            Option<&BlackHoleStarState>,
        ),
        Or<(With<CentralStar>, With<BlackHoleStarState>)>,
    >,
    mut root_query: Query<(Entity, &mut Transform), With<QuasarBeamRoot>>,
    mut part_query: Query<(&mut Transform, &QuasarBeamPart), Without<QuasarBeamRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    let target = quasi_query.iter().find(|(_, body, mass, _, opt_qs)| {
        opt_qs.is_some()
            || body.body_type == BodyType::QuasiStar
            || body.name.contains("Quasar")
            || (body.body_type == BodyType::BlackHole && mass.0 > 500.0)
    });

    let Some((pos, body, _mass, radius, opt_qs)) = target else {
        for (ent, _) in root_query.iter() {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.despawn();
            }
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let is_blown_out = opt_qs.is_some_and(|qs| qs.is_blown_out) || body.name.contains("Quasar");
    let light_dist = opt_qs.map_or(0.0, |qs| qs.jet_travel_distance_au as f32);

    if !is_blown_out || light_dist <= 0.1 {
        for (ent, _) in root_query.iter() {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.despawn();
            }
        }
        return;
    }

    let jet_len = light_dist;
    let current_visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
    let pole_start = (current_visual_radius * 0.90).max(0.05);
    let beam_len = (jet_len - pole_start).max(0.1);
    let beam_center = pole_start + beam_len * 0.5;

    let core_r = 0.06f32;
    let sheath_r = 0.18f32;
    let lobe_r = 0.55f32;

    if let Some((_, mut root_trans)) = root_query.iter_mut().next() {
        update_quasar_beam_transforms(
            &mut root_trans,
            &mut part_query,
            world_pos,
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
            world_pos,
            beam_center,
            beam_len,
            jet_len,
            core_r,
            sheath_r,
            lobe_r,
        );
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
    time: Res<Time>,
    visual_assets: Option<Res<VisualAssets>>,
    config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pulsar_query: Query<
        (&SimPosition, &CelestialBody, &Radius),
        Or<(With<CentralStar>, With<CelestialBody>)>,
    >,
    mut root_query: Query<(Entity, &mut Transform), With<PulsarBeamRoot>>,
    mut part_query: Query<(&mut Transform, &PulsarBeamPart), Without<PulsarBeamRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    let target = pulsar_query
        .iter()
        .find(|(_, body, _)| body.body_type == BodyType::Pulsar || body.name.contains("Pulsar"));

    let Some((pos, body, radius)) = target else {
        for (ent, _) in root_query.iter() {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.despawn();
            }
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let elapsed = time.elapsed_secs();
    let spin_rate = 24.0;
    let beam_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.38);

    let current_visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
    let pole_start = (current_visual_radius * 0.90).max(0.002);

    if let Some((_, mut root_trans)) = root_query.iter_mut().next() {
        root_trans.translation = world_pos;
        root_trans.rotation = beam_rot;

        for (mut part_trans, part) in part_query.iter_mut() {
            match part {
                PulsarBeamPart::NorthBeam => {
                    part_trans.translation = Vec3::new(0.0, pole_start, 0.0);
                }
                PulsarBeamPart::SouthBeam => {
                    part_trans.translation = Vec3::new(0.0, -pole_start, 0.0);
                    part_trans.rotation = Quat::from_rotation_x(std::f32::consts::PI);
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
                    MeshMaterial3d(beam_mat.clone()),
                    Transform::from_xyz(0.0, -pole_start, 0.0)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
                    NotShadowCaster,
                ));
            });
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
    time: Res<Time>,
    visual_assets: Option<Res<VisualAssets>>,
    _config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    magnetar_query: Query<
        (
            &SimPosition,
            &CelestialBody,
            Option<&ElectromagneticFieldState>,
        ),
        Or<(With<CentralStar>, With<CelestialBody>)>,
    >,
    mut root_query: Query<(Entity, &mut Transform), With<MagnetarStructureRoot>>,
    _part_query: Query<(&mut Transform, &MagnetarStructurePart), Without<MagnetarStructureRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    let target = magnetar_query
        .iter()
        .find(|(_, body, _)| body.body_type == BodyType::Magnetar)
        .or_else(|| {
            magnetar_query
                .iter()
                .find(|(_, _, opt_em)| opt_em.is_some_and(|em| em.magnetic_field_gauss >= 1.0e14))
        })
        .or_else(|| {
            magnetar_query.iter().find(|(_, body, _)| {
                body.name.to_lowercase().contains("magnetar")
                    && !body.name.contains("Clump")
                    && !body.name.contains("Ejecta")
            })
        });

    let Some((pos, _body, _)) = target else {
        for (ent, _) in root_query.iter() {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.despawn();
            }
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let elapsed = time.elapsed_secs();
    let spin_rate = 1.2;
    let mag_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.26);

    if let Some((_, mut root_trans)) = root_query.iter_mut().next() {
        root_trans.translation = world_pos;
        root_trans.rotation = mag_rot;
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
                Transform::from_translation(world_pos).with_rotation(mag_rot),
                Visibility::default(),
            ))
            .with_children(|parent| {
                parent.spawn((
                    MagnetarStructurePart::EquatorialRing,
                    Mesh3d(assets.magnetar_ring_mesh.clone()),
                    MeshMaterial3d(ring_mat.clone()),
                    Transform::IDENTITY,
                    NotShadowCaster,
                ));
                parent.spawn((
                    MagnetarStructurePart::MagneticFieldLoops,
                    Mesh3d(assets.magnetar_field_loops_mesh.clone()),
                    MeshMaterial3d(loops_mat.clone()),
                    Transform::IDENTITY,
                    NotShadowCaster,
                ));
            });
    }
}
