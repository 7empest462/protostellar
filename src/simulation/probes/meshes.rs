//! Procedural 3D mesh and material hierarchy builders for orbiter and rover space probes.

use bevy::prelude::*;

fn probe_silicon_solar_material() -> crate::rendering::effects::MegastructureMaterial {
    let mut mat = crate::rendering::effects::probe_hull_material();
    mat.uniforms.primary_albedo_and_metal = Vec4::new(0.10, 0.20, 0.44, 0.90);
    mat.uniforms.accent_albedo_and_rough = Vec4::new(0.55, 0.62, 0.72, 0.16);
    mat.uniforms.pattern_params = Vec4::new(
        crate::rendering::effects::tech_style::DYSON_COLLECTOR,
        5.0,
        3.0,
        2.0,
    );
    mat
}

#[allow(
    clippy::too_many_arguments,
    reason = "Spawns probe hull, solar wing, or glow component using optional procedural shader"
)]
fn spawn_probe_part(
    parent: &mut ChildSpawnerCommands,
    mesh: Handle<Mesh>,
    tf: Transform,
    tech_mat: Option<&Handle<crate::rendering::effects::MegastructureMaterial>>,
    fallback_std: &Handle<StandardMaterial>,
) {
    if let Some(tm) = tech_mat {
        parent.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(tm.clone()),
            tf,
            Visibility::default(),
        ));
    } else {
        parent.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(fallback_std.clone()),
            tf,
            Visibility::default(),
        ));
    }
}

struct ProbeMats<'a> {
    hull: Option<&'a Handle<crate::rendering::effects::MegastructureMaterial>>,
    panel: Option<&'a Handle<crate::rendering::effects::MegastructureMaterial>>,
    std_metal: &'a Handle<StandardMaterial>,
    std_panel: &'a Handle<StandardMaterial>,
    std_glow: &'a Handle<StandardMaterial>,
}

fn spawn_orbiter_meshes(
    parent: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    mats: &ProbeMats<'_>,
) {
    let body_mesh = meshes.add(Cylinder::new(0.4, 2.0));
    let nose_mesh = meshes.add(Cone {
        radius: 0.4,
        height: 1.0,
    });
    let dish_mesh = meshes.add(Cone {
        radius: 0.65,
        height: 0.22,
    });
    let panel_mesh = meshes.add(Cuboid::new(4.2, 0.05, 0.85));
    let engine_mesh = meshes.add(Cone {
        radius: 0.25,
        height: 0.6,
    });

    spawn_probe_part(
        parent,
        body_mesh,
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        mats.hull,
        mats.std_metal,
    );
    spawn_probe_part(
        parent,
        nose_mesh,
        Transform::from_translation(Vec3::new(0.0, 0.0, -1.5))
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        mats.hull,
        mats.std_metal,
    );
    spawn_probe_part(
        parent,
        dish_mesh,
        Transform::from_translation(Vec3::new(0.0, 0.55, -0.35))
            .with_rotation(Quat::from_rotation_x(0.35)),
        mats.hull,
        mats.std_metal,
    );
    spawn_probe_part(
        parent,
        panel_mesh,
        Transform::default(),
        mats.panel,
        mats.std_panel,
    );
    parent.spawn((
        Mesh3d(engine_mesh),
        MeshMaterial3d(mats.std_glow.clone()),
        Transform::from_translation(Vec3::new(0.0, 0.0, 1.3))
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Visibility::default(),
    ));
}

fn spawn_rover_meshes(
    parent: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    mats: &ProbeMats<'_>,
) {
    let body_mesh = meshes.add(Cuboid::new(1.2, 0.6, 1.8));
    let mast_mesh = meshes.add(Cylinder::new(0.05, 0.8));
    let dish_mesh = meshes.add(Cone {
        radius: 0.35,
        height: 0.14,
    });
    let deck_panel = meshes.add(Cuboid::new(1.5, 0.04, 0.9));
    let wheel_mesh = meshes.add(Cylinder::new(0.3, 0.2));
    let camera_mesh = meshes.add(Cuboid::new(0.2, 0.15, 0.2));

    spawn_probe_part(
        parent,
        body_mesh,
        Transform::from_translation(Vec3::new(0.0, 0.35, 0.0)),
        mats.hull,
        mats.std_metal,
    );
    spawn_probe_part(
        parent,
        mast_mesh,
        Transform::from_translation(Vec3::new(0.4, 1.0, -0.6)),
        mats.hull,
        mats.std_metal,
    );
    spawn_probe_part(
        parent,
        dish_mesh,
        Transform::from_translation(Vec3::new(-0.35, 0.85, 0.45))
            .with_rotation(Quat::from_rotation_x(-0.4)),
        mats.hull,
        mats.std_metal,
    );
    spawn_probe_part(
        parent,
        deck_panel,
        Transform::from_translation(Vec3::new(0.0, 0.68, 0.2)),
        mats.panel,
        mats.std_panel,
    );
    parent.spawn((
        Mesh3d(camera_mesh),
        MeshMaterial3d(mats.std_glow.clone()),
        Transform::from_translation(Vec3::new(0.4, 1.4, -0.6)),
        Visibility::default(),
    ));
    let wheel_rot = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    for &x in &[-0.7, 0.7] {
        for &z in &[-0.7, 0.0, 0.7] {
            parent.spawn((
                Mesh3d(wheel_mesh.clone()),
                MeshMaterial3d(mats.std_panel.clone()),
                Transform::from_translation(Vec3::new(x, 0.3, z)).with_rotation(wheel_rot),
                Visibility::default(),
            ));
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Mesh generation requires asset mutators and command queues"
)]
pub(super) fn spawn_probe_mesh_hierarchy(
    commands: &mut Commands,
    probe_entity: Entity,
    is_orbiter: bool,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    opt_tech_mats: Option<&mut Assets<crate::rendering::effects::MegastructureMaterial>>,
) {
    let (base_color, emissive) = if is_orbiter {
        (
            Color::srgb(0.2, 0.9, 1.0),
            LinearRgba::new(0.5, 4.0, 12.0, 1.0),
        )
    } else {
        (
            Color::srgb(1.0, 0.75, 0.2),
            LinearRgba::new(10.0, 5.0, 0.5, 1.0),
        )
    };
    let mat_metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.84, 0.66, 0.24),
        metallic: 0.92,
        perceptual_roughness: 0.20,
        ..default()
    });
    let mat_panel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.14, 0.34),
        metallic: 0.88,
        perceptual_roughness: 0.18,
        ..default()
    });
    let mat_glow = materials.add(StandardMaterial {
        base_color,
        emissive,
        unlit: true,
        ..default()
    });
    let tech_pair = opt_tech_mats.map(|tm| {
        (
            tm.add(crate::rendering::effects::probe_hull_material()),
            tm.add(probe_silicon_solar_material()),
        )
    });
    let mats = ProbeMats {
        hull: tech_pair.as_ref().map(|p| &p.0),
        panel: tech_pair.as_ref().map(|p| &p.1),
        std_metal: &mat_metal,
        std_panel: &mat_panel,
        std_glow: &mat_glow,
    };

    let Ok(mut entity_cmds) = commands.get_entity(probe_entity) else {
        return;
    };
    entity_cmds.with_children(|parent| {
        if is_orbiter {
            spawn_orbiter_meshes(parent, meshes, &mats);
        } else {
            spawn_rover_meshes(parent, meshes, &mats);
        }
    });
}
