use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use hashbrown::{HashMap, HashSet};

use crate::rendering::materials::{RelativisticJetMaterial, RelativisticJetUniforms};
use crate::simulation::components::*;
use crate::simulation::resources::SimTime;

use super::VisualAssets;

/// Root marker for an instantiated 3D relativistic polar jet hierarchy.
#[derive(Component, Debug, Clone, Copy)]
pub struct RelativisticJetRoot {
    /// Target celestial body entity with [`RelativisticJetState`].
    pub target: Entity,
}

/// Sub-part of the dual relativistic polar jet system.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativisticJetPart {
    NorthJet,
    SouthJet,
}

fn compute_jet_pointing_vectors(
    opt_spin: Option<&SpinState>,
    jet_state: &RelativisticJetState,
    elapsed: f32,
) -> (Vec3, Vec3) {
    let base_axis = if let Some(spin) = opt_spin {
        if spin.spin_vector.length_squared() > 1e-6 {
            spin.spin_vector.normalize().as_vec3()
        } else {
            let tilt_rad = (spin.axial_tilt_degrees as f32).to_radians();
            Quat::from_rotation_z(tilt_rad) * Vec3::Y
        }
    } else {
        Vec3::Y
    };

    let phase = if jet_state.precession_period_s > 0.0 {
        (elapsed / jet_state.precession_period_s).fract()
    } else {
        0.0
    };

    let dir_north = if jet_state.precession_period_s > 0.0 && jet_state.precession_angle_rad > 0.0 {
        crate::simulation::relativity::calculate_precessing_jet_direction(
            base_axis,
            Vec3::Y,
            jet_state.precession_angle_rad,
            phase,
        )
    } else {
        base_axis
    };

    let dir_south = -dir_north;
    (dir_north, dir_south)
}

fn compute_jet_mesh_scales(jet_state: &RelativisticJetState) -> (f32, f32) {
    let total_len = jet_state.jet_length_au.max(0.1);
    let y_scale = total_len / 50.0;
    let max_r = (total_len * jet_state.opening_angle_rad.tan() * 1.5).max(0.05);
    let xz_scale = (max_r / 6.5).max(0.001);
    (xz_scale, y_scale)
}

fn align_to_direction(dir: Vec3) -> Quat {
    let d = dir.normalize_or_zero();
    let dot = d.dot(Vec3::Y);
    if dot > 0.9999 {
        Quat::IDENTITY
    } else if dot < -0.9999 {
        Quat::from_rotation_x(std::f32::consts::PI)
    } else {
        Quat::from_rotation_arc(Vec3::Y, d)
    }
}

fn build_jet_uniforms(
    jet_state: &RelativisticJetState,
    dir: Vec3,
    world_pos: Vec3,
    elapsed: f32,
    total_len: f32,
) -> RelativisticJetUniforms {
    RelativisticJetUniforms {
        jet_params: Vec4::new(
            elapsed,
            jet_state.lorentz_factor,
            jet_state.opening_angle_rad,
            total_len,
        ),
        jet_dir_and_precession: Vec4::new(dir.x, dir.y, dir.z, jet_state.precession_angle_rad),
        synchrotron_params: Vec4::new(
            jet_state.spectral_index,
            jet_state.knot_speed_c,
            jet_state.knot_frequency,
            jet_state.helical_pitch,
        ),
        core_color: jet_state.core_color * jet_state.synchrotron_luminosity,
        lobe_color: jet_state.lobe_color * jet_state.synchrotron_luminosity,
        jet_origin_and_doppler: Vec4::new(
            world_pos.x,
            world_pos.y,
            world_pos.z,
            if jet_state.doppler_boosting_enabled {
                1.0
            } else {
                0.0
            },
        ),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Spawns dual relativistic polar jet hierarchy"
)]
fn spawn_jet_hierarchy(
    commands: &mut Commands,
    assets: &VisualAssets,
    materials: &mut Assets<RelativisticJetMaterial>,
    target: Entity,
    world_pos: Vec3,
    jet_state: &RelativisticJetState,
    dir_north: Vec3,
    dir_south: Vec3,
    xz_scale: f32,
    y_scale: f32,
    elapsed: f32,
    total_len: f32,
) {
    let north_uniforms = build_jet_uniforms(jet_state, dir_north, world_pos, elapsed, total_len);
    let south_uniforms = build_jet_uniforms(jet_state, dir_south, world_pos, elapsed, total_len);

    let north_mat = materials.add(RelativisticJetMaterial {
        uniforms: north_uniforms,
    });
    let south_mat = materials.add(RelativisticJetMaterial {
        uniforms: south_uniforms,
    });

    commands
        .spawn((
            RelativisticJetRoot { target },
            Transform::from_translation(world_pos),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                RelativisticJetPart::NorthJet,
                Mesh3d(assets.pulsar_beam_mesh.clone()),
                MeshMaterial3d(north_mat),
                Transform::from_rotation(align_to_direction(dir_north))
                    .with_scale(Vec3::new(xz_scale, y_scale, xz_scale)),
                NotShadowCaster,
            ));
            parent.spawn((
                RelativisticJetPart::SouthJet,
                Mesh3d(assets.pulsar_beam_mesh.clone()),
                MeshMaterial3d(south_mat),
                Transform::from_rotation(align_to_direction(dir_south))
                    .with_scale(Vec3::new(xz_scale, y_scale, xz_scale)),
                NotShadowCaster,
            ));
        });
}

#[allow(clippy::type_complexity, reason = "Compact remnant candidate query")]
fn auto_provision_relativistic_jet_states(
    commands: &mut Commands,
    compact_candidates: &Query<
        (Entity, &CelestialBody, &Mass, Option<&SpinState>),
        (
            Without<RelativisticJetState>,
            Or<(With<CentralStar>, With<CelestialBody>)>,
        ),
    >,
) {
    for (entity, body, mass, opt_spin) in compact_candidates.iter() {
        let auto_state = match body.body_type {
            BodyType::Pulsar => {
                let period_s = opt_spin.map_or(0.00622, |s| {
                    (s.rotation_period_hours as f32 * 3600.0).max(0.001)
                });
                Some(RelativisticJetState::pulsar(period_s))
            }
            BodyType::Magnetar => Some(RelativisticJetState::magnetar()),
            BodyType::QuasiStar => Some(RelativisticJetState::quasi_star()),
            BodyType::BlackHole if mass.0 >= 5.0 || body.name.contains("Black Hole") => {
                Some(RelativisticJetState::black_hole(mass.0))
            }
            _ => None,
        };
        if let Some(state) = auto_state {
            if let Ok(mut cmd) = commands.get_entity(entity) {
                cmd.try_insert(state);
            }
        }
    }
}

#[allow(clippy::type_complexity, reason = "Relativistic jet child parts query")]
fn update_jet_child_parts(
    children: &Children,
    part_query: &mut Query<
        (
            &mut Transform,
            &MeshMaterial3d<RelativisticJetMaterial>,
            &RelativisticJetPart,
        ),
        Without<RelativisticJetRoot>,
    >,
    jet_materials: &mut Assets<RelativisticJetMaterial>,
    dir_north: Vec3,
    dir_south: Vec3,
    world_pos: Vec3,
    jet_state: &RelativisticJetState,
    xz_scale: f32,
    y_scale: f32,
    elapsed: f32,
    total_len: f32,
) {
    for child in children.iter() {
        if let Ok((mut part_trans, mat_handle, part)) = part_query.get_mut(child) {
            let dir = match part {
                RelativisticJetPart::NorthJet => dir_north,
                RelativisticJetPart::SouthJet => dir_south,
            };

            part_trans.rotation = align_to_direction(dir);
            part_trans.scale = Vec3::new(xz_scale, y_scale, xz_scale);

            if let Some(mut mat) = jet_materials.get_mut(&mat_handle.0) {
                mat.uniforms = build_jet_uniforms(jet_state, dir, world_pos, elapsed, total_len);
            }
        }
    }
}

#[allow(clippy::type_complexity, reason = "Active jet map parameter")]
fn spawn_missing_jet_hierarchies(
    commands: &mut Commands,
    assets: &VisualAssets,
    jet_materials: &mut Assets<RelativisticJetMaterial>,
    active_map: &HashMap<
        Entity,
        (
            &SimPosition,
            &CelestialBody,
            &RelativisticJetState,
            Option<&SpinState>,
        ),
    >,
    updated_targets: &HashSet<Entity>,
    elapsed: f32,
) {
    for (&target, &(pos, _body, jet_state, opt_spin)) in active_map {
        if updated_targets.contains(&target) || jet_state.jet_length_au <= 0.01 {
            continue;
        }

        let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let (dir_north, dir_south) = compute_jet_pointing_vectors(opt_spin, jet_state, elapsed);
        let (xz_scale, y_scale) = compute_jet_mesh_scales(jet_state);
        let total_len = jet_state.jet_length_au.max(0.1);

        spawn_jet_hierarchy(
            commands,
            assets,
            jet_materials,
            target,
            world_pos,
            jet_state,
            dir_north,
            dir_south,
            xz_scale,
            y_scale,
            elapsed,
            total_len,
        );
    }
}

/// Automatically provisions [`RelativisticJetState`] for compact remnants and synchronizes 3D meshes.
#[allow(
    clippy::type_complexity,
    reason = "Relativistic jet synchronization system"
)]
pub fn sync_relativistic_jets(
    mut commands: Commands,
    sim_time: Option<Res<SimTime>>,
    visual_assets: Option<Res<VisualAssets>>,
    mut jet_materials: ResMut<Assets<RelativisticJetMaterial>>,
    compact_candidates: Query<
        (Entity, &CelestialBody, &Mass, Option<&SpinState>),
        (
            Without<RelativisticJetState>,
            Or<(With<CentralStar>, With<CelestialBody>)>,
        ),
    >,
    active_jets: Query<
        (
            Entity,
            &SimPosition,
            &CelestialBody,
            &RelativisticJetState,
            Option<&SpinState>,
        ),
        Or<(With<CentralStar>, With<CelestialBody>)>,
    >,
    mut root_query: Query<(
        Entity,
        &mut Transform,
        &RelativisticJetRoot,
        Option<&Children>,
    )>,
    mut part_query: Query<
        (
            &mut Transform,
            &MeshMaterial3d<RelativisticJetMaterial>,
            &RelativisticJetPart,
        ),
        Without<RelativisticJetRoot>,
    >,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    // 1. Auto-insert RelativisticJetState on compact remnants if not explicitly added
    auto_provision_relativistic_jet_states(&mut commands, &compact_candidates);

    // 2. Index active jet sources
    let active_map: HashMap<
        Entity,
        (
            &SimPosition,
            &CelestialBody,
            &RelativisticJetState,
            Option<&SpinState>,
        ),
    > = active_jets
        .iter()
        .map(|(e, pos, b, jet, spin)| (e, (pos, b, jet, spin)))
        .collect();

    let mut updated_targets = HashSet::new();
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);

    // 3. Update existing roots or despawn orphans
    for (root_entity, mut root_trans, root, opt_children) in root_query.iter_mut() {
        if let Some((pos, _body, jet_state, opt_spin)) = active_map.get(&root.target) {
            if jet_state.jet_length_au <= 0.01 {
                if let Ok(mut cmd) = commands.get_entity(root_entity) {
                    cmd.despawn();
                }
                continue;
            }
            let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            root_trans.translation = world_pos;
            updated_targets.insert(root.target);

            let (dir_north, dir_south) =
                compute_jet_pointing_vectors(*opt_spin, jet_state, elapsed);
            let (xz_scale, y_scale) = compute_jet_mesh_scales(jet_state);
            let total_len = jet_state.jet_length_au.max(0.1);

            if let Some(children) = opt_children {
                update_jet_child_parts(
                    children,
                    &mut part_query,
                    &mut jet_materials,
                    dir_north,
                    dir_south,
                    world_pos,
                    jet_state,
                    xz_scale,
                    y_scale,
                    elapsed,
                    total_len,
                );
            }
        } else if let Ok(mut cmd) = commands.get_entity(root_entity) {
            cmd.despawn();
        }
    }

    // 4. Spawn new jet hierarchies for sources that don't have roots yet
    spawn_missing_jet_hierarchies(
        &mut commands,
        &assets,
        &mut jet_materials,
        &active_map,
        &updated_targets,
        elapsed,
    );
}
