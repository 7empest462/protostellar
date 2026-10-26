use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::palettes::calc_ring_color;
use super::{VisualAssets, VisualBody};

/// Marker component for an instantiated visual planetary ring entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualRingChild;

/// Synchronizes 3D planetary ring system meshes, materials, and axial tilt transforms.
#[allow(clippy::type_complexity, reason = "Planetary ring query")]
pub fn sync_planetary_rings(
    mut commands: Commands,
    time: Res<Time>,
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
            Option<&SpinState>,
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
    let elapsed_secs = time.elapsed_secs();
    let star_world_pos = stars_query
        .iter()
        .find(|(_, b)| b.body_type.is_star_or_remnant())
        .map_or(Vec3::ZERO, |(pos, _)| {
            Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32)
        });

    for (planet_entity, ring_sys, radius, body, planet_trans, _opt_spin, opt_children) in
        planets_with_rings_query.iter()
    {
        if body.body_type == BodyType::BlackHole {
            if let Some(children) = opt_children {
                for child in children.iter() {
                    if ring_children_query.get(child).is_ok() {
                        if let Ok(mut cmd) = commands.get_entity(child) {
                            cmd.despawn();
                        }
                    }
                }
            }
            continue;
        }
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

        // Planetary rings physically form in the planet's equatorial plane (perpendicular to spin axis).
        // Since the parent planet entity's transform is already rotated to align with spin_dir,
        // the child ring mesh (Plane3d with normal +Y) naturally sits in the equator with Quat::IDENTITY.
        let ring_rotation = Quat::IDENTITY;

        let star_dir_world = (star_world_pos - planet_trans.translation).normalize_or_zero();
        let star_dir_world = if star_dir_world.length_squared() < 0.01 {
            Vec3::Y
        } else {
            star_dir_world
        };
        let ring_world_rot = planet_trans.rotation;
        let star_dir_local = (ring_world_rot.inverse() * star_dir_world).normalize_or_zero();
        let planet_radius_ratio = 1.0 / ring_ratio;

        let inner_ratio_mesh = if ring_sys.outer_radius_au > 0.0 && ring_sys.inner_radius_au > 0.0 {
            (ring_sys.inner_radius_au / ring_sys.outer_radius_au).clamp(planet_radius_ratio, 0.95)
        } else {
            (1.25f32 / 2.35f32).clamp(planet_radius_ratio, 0.95)
        };

        let mut found_child = false;
        if let Some(children) = opt_children {
            for child in children.iter() {
                if let Ok((mut transform, mat_handle)) = ring_children_query.get_mut(child) {
                    found_child = true;
                    transform.scale = Vec3::new(ring_outer_scale, 1.0, ring_outer_scale);
                    transform.rotation = ring_rotation;

                    if let Some(mut mat) = ring_materials.get_mut(&mat_handle.0) {
                        mat.uniforms.inner_radius = inner_ratio_mesh;
                        mat.uniforms.outer_radius = 1.0;
                        mat.uniforms.optical_depth = ring_sys.optical_depth;
                        mat.uniforms.ice_fraction = ring_sys.ice_fraction;
                        mat.uniforms.ring_color = calc_ring_color(ring_sys.ice_fraction);
                        mat.uniforms.star_dir_local = Vec4::new(
                            star_dir_local.x,
                            star_dir_local.y,
                            star_dir_local.z,
                            planet_radius_ratio,
                        );
                        mat.uniforms.shadow_params = Vec4::new(0.03, 0.022, 0.97, elapsed_secs);
                    }
                }
            }
        }

        if !found_child {
            let ring_color = calc_ring_color(ring_sys.ice_fraction);
            let material = ring_materials.add(RingMaterial {
                uniforms: RingUniforms {
                    inner_radius: inner_ratio_mesh,
                    outer_radius: 1.0,
                    optical_depth: ring_sys.optical_depth,
                    ice_fraction: ring_sys.ice_fraction,
                    ring_color,
                    star_dir_local: Vec4::new(
                        star_dir_local.x,
                        star_dir_local.y,
                        star_dir_local.z,
                        planet_radius_ratio,
                    ),
                    shadow_params: Vec4::new(0.03, 0.022, 0.97, elapsed_secs),
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
                        Visibility::default(),
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
                cmd.try_despawn();
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
                cmd.try_despawn();
            }
        }
        return;
    }

    let jet_len = light_dist;
    let current_visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
    let pole_start = (current_visual_radius * 0.90).max(0.002);
    let beam_len = (jet_len - pole_start).max(0.1);
    let beam_center = pole_start + beam_len * 0.5;

    let core_r = (current_visual_radius * 0.12).clamp(0.002, 0.04);
    let sheath_r = (current_visual_radius * 0.28).clamp(0.005, 0.09);
    let lobe_r = (current_visual_radius * 0.85).clamp(0.02, 0.35);

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
    sim_time: Option<Res<SimTime>>,
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
                cmd.try_despawn();
            }
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let spin_rate = 24.0;
    let beam_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.38);

    let current_visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
    let pole_start = (current_visual_radius * 0.92).max(0.0005);

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
    sim_time: Option<Res<SimTime>>,
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
                cmd.try_despawn();
            }
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
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
        (&mut Transform, &MeshMaterial3d<BlackHoleDiskMaterial>),
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
                    if disk_children_query.get(child).is_ok() {
                        if let Ok(mut c_cmd) = commands.get_entity(child) {
                            c_cmd.try_despawn();
                        }
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
                if let Ok((mut transform, mat_handle)) = disk_children_query.get_mut(child) {
                    found_child = true;
                    *transform = target_transform;
                    if let Some(mut mat) = disk_materials.get_mut(&mat_handle.0) {
                        mat.uniforms = uniforms.clone();
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

    let spin_axis = opt_kerr.map_or(Vec3::Y, |k| {
        let v = k.spin_axis.as_vec3();
        if v.length_squared() > 0.5 {
            v.normalize()
        } else {
            Vec3::Y
        }
    });

    let cam_dir_world = (cam_pos - bh_trans.translation).normalize_or_zero();
    let cam_dir_local = bh_trans.rotation.inverse() * cam_dir_world;

    let mut u_right = spin_axis.cross(cam_dir_local);
    if u_right.length_squared() < 1e-6 {
        u_right = Vec3::X;
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
        (0.16, 0.055, mass.0 as f32)
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
