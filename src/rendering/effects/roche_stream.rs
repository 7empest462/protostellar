use crate::simulation::accretion::RocheLobeOverflow;
use crate::simulation::components::{Radius, SimPosition};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy_shader::ShaderRef;
use hashbrown::HashSet;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RocheStreamMaterial {
    #[uniform(0)]
    pub color_and_intensity: Vec4,
    #[uniform(0)]
    pub stream_params: Vec4, // x: progress offset, y: turbulence scale, z: width, w: fade
}

impl Material for RocheStreamMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/roche_stream.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Component)]
pub struct RocheStreamVisual {
    pub donor: Entity,
    pub accretor: Entity,
}

#[allow(clippy::disallowed_types, reason = "Needed to update streams")]
pub fn update_roche_streams(
    mut commands: Commands,
    query_overflows: Query<(
        Entity,
        &RocheLobeOverflow,
        &SimPosition,
        &Radius,
        &crate::simulation::components::CelestialBody,
    )>,
    query_targets: Query<(&SimPosition, &Radius)>,
    mut query_visuals: Query<(
        Entity,
        &RocheStreamVisual,
        &mut Transform,
        &MeshMaterial3d<RocheStreamMaterial>,
    )>,
    mut materials: ResMut<Assets<RocheStreamMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time>,
    config: Res<crate::simulation::resources::SimulationConfig>,
) {
    let mut active_streams = HashSet::new();

    for (donor_ent, overflow, donor_pos, donor_radius, donor_body) in query_overflows.iter() {
        if let Ok((accretor_pos, _accretor_radius)) = query_targets.get(overflow.companion) {
            active_streams.insert(donor_ent);

            // Calculate spatial parameters
            let dist = (donor_pos.0.distance(accretor_pos.0) as f32) * config.size_exaggeration;
            let center = (donor_pos.0 + accretor_pos.0) * 0.5;
            let dir = (accretor_pos.0 - donor_pos.0).normalize_or_zero();

            let stream_width = (donor_radius.0 as f32) * config.size_exaggeration * 0.6; // Width proportional to donor

            // Check if visual already exists
            let mut found = false;
            for (_vis_ent, visual, mut transform, mat_handle) in query_visuals.iter_mut() {
                if visual.donor == donor_ent && visual.accretor == overflow.companion {
                    found = true;
                    // Update transform (Cylinder height is along Y axis)
                    transform.translation =
                        (center * f64::from(config.size_exaggeration)).as_vec3();
                    transform.rotation = Quat::from_rotation_arc(Vec3::Y, dir.as_vec3());
                    transform.scale = Vec3::new(stream_width, dist * 1.05, stream_width);

                    if let Some(mut mat) = materials.get_mut(mat_handle.id()) {
                        mat.stream_params.x += time.delta_secs() * 2.0; // Flow animation
                                                                        // Intensity based on mass transfer rate
                        let intensity = (overflow.mass_transfer_rate * 1e5).clamp(0.5, 5.0) as f32;
                        mat.color_and_intensity.w = intensity;
                    }
                    break;
                }
            }

            if !found {
                // Spawn new visual
                let mat = materials.add(RocheStreamMaterial {
                    // Deep red/orange for Red Giant donors, hotter if others
                    color_and_intensity: if donor_body.body_type
                        == crate::simulation::components::BodyType::RedGiant
                    {
                        Vec4::new(1.0, 0.4, 0.1, 1.0)
                    } else {
                        Vec4::new(0.8, 0.8, 1.0, 1.0)
                    },
                    stream_params: Vec4::new(0.0, 4.0, 1.0, 1.0),
                });

                commands.spawn((
                    RocheStreamVisual {
                        donor: donor_ent,
                        accretor: overflow.companion,
                    },
                    Mesh3d(meshes.add(Cylinder::new(0.5, 0.5))),
                    MeshMaterial3d(mat),
                    Transform::from_translation(
                        (center * f64::from(config.size_exaggeration)).as_vec3(),
                    )
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir.as_vec3()))
                    .with_scale(Vec3::new(
                        stream_width,
                        dist * 1.05,
                        stream_width,
                    )),
                    bevy::light::NotShadowCaster,
                    bevy::light::NotShadowReceiver,
                ));
            }
        }
    }

    // Cleanup old visuals
    for (vis_ent, visual, _, _) in query_visuals.iter() {
        if !active_streams.contains(&visual.donor) {
            commands.entity(vis_ent).despawn();
        }
    }
}
