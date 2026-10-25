//! 3D volumetric circumstellar environments (LBV Homunculus bipolar nebulae, Wolf-Rayet
//! colliding-wind pinwheel spirals, AGB Carbon/Hypergiant detached shells, Boson/Quark star
//! relativistic Einstein lensing halos, and Herbig Ae/Be flared protoplanetary envelopes).

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::rendering::materials::{CircumstellarMaterial, CircumstellarUniforms};
use crate::simulation::components::{BodyType, CelestialBody, Radius, SimPosition, SpinState};
use crate::simulation::resources::SimTime;

use super::{VisualAssets, VisualBody};

/// Marker component for an instantiated 3D volumetric circumstellar nebula or relativistic halo child entity.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct VisualCircumstellarChild;

/// Static configuration profile for a stellar archetype's 3D circumstellar environment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircumstellarProfile {
    pub env_type: f32,
    pub sub_mode: f32,
    pub shell_scale: f32,
    pub primary_color: Vec4,
    pub secondary_color: Vec4,
    pub kinematics: Vec4,
}

/// Resolves the 3D volumetric circumstellar profile for exotic, evolved, and pre-main-sequence stars.
pub fn resolve_circumstellar_profile(body_type: BodyType) -> Option<CircumstellarProfile> {
    match body_type {
        BodyType::LuminousBlueVariable => Some(CircumstellarProfile {
            env_type: 0.0,
            sub_mode: 0.0,
            shell_scale: 2.35,
            primary_color: Vec4::new(0.45, 0.98, 1.75, 1.0),
            secondary_color: Vec4::new(1.25, 0.52, 0.28, 1.0),
            kinematics: Vec4::new(9.5, 1.15, 0.95, 0.8),
        }),
        BodyType::WolfRayet => Some(CircumstellarProfile {
            env_type: 1.0,
            sub_mode: 0.0,
            shell_scale: 2.50,
            primary_color: Vec4::new(1.35, 0.42, 1.15, 1.0),
            secondary_color: Vec4::new(0.38, 1.05, 1.65, 1.0),
            kinematics: Vec4::new(14.0, 1.25, 0.90, 1.2),
        }),
        BodyType::CarbonStar => Some(CircumstellarProfile {
            env_type: 2.0,
            sub_mode: 0.0,
            shell_scale: 2.15,
            primary_color: Vec4::new(1.35, 0.24, 0.06, 1.0),
            secondary_color: Vec4::new(0.42, 0.05, 0.02, 1.0),
            kinematics: Vec4::new(16.0, 1.10, 0.85, 0.45),
        }),
        BodyType::YellowHypergiant => Some(CircumstellarProfile {
            env_type: 2.0,
            sub_mode: 1.0,
            shell_scale: 2.25,
            primary_color: Vec4::new(1.45, 1.12, 0.45, 1.0),
            secondary_color: Vec4::new(1.25, 0.55, 0.16, 1.0),
            kinematics: Vec4::new(14.0, 1.10, 0.85, 0.65),
        }),
        BodyType::ThorneZytkow => Some(CircumstellarProfile {
            env_type: 2.0,
            sub_mode: 2.0,
            shell_scale: 2.30,
            primary_color: Vec4::new(1.28, 0.28, 0.18, 1.0),
            secondary_color: Vec4::new(0.45, 1.25, 2.15, 1.0),
            kinematics: Vec4::new(15.0, 1.20, 0.90, 0.85),
        }),
        BodyType::BosonStar => Some(CircumstellarProfile {
            env_type: 3.0,
            sub_mode: 0.0,
            shell_scale: 2.20,
            primary_color: Vec4::new(0.42, 1.35, 1.75, 1.0),
            secondary_color: Vec4::new(0.75, 0.95, 1.85, 1.0),
            kinematics: Vec4::new(18.0, 0.85, 0.95, 1.8),
        }),
        BodyType::QuarkStar => Some(CircumstellarProfile {
            env_type: 3.0,
            sub_mode: 1.0,
            shell_scale: 2.15,
            primary_color: Vec4::new(1.15, 0.85, 1.95, 1.0),
            secondary_color: Vec4::new(0.55, 1.55, 2.35, 1.0),
            kinematics: Vec4::new(16.0, 1.35, 1.00, 2.4),
        }),
        BodyType::HerbigAeBeStar => Some(CircumstellarProfile {
            env_type: 4.0,
            sub_mode: 0.0,
            shell_scale: 2.25,
            primary_color: Vec4::new(0.75, 1.12, 1.75, 1.0),
            secondary_color: Vec4::new(1.15, 0.52, 0.22, 1.0),
            kinematics: Vec4::new(12.0, 1.05, 0.85, 0.7),
        }),
        BodyType::TTauriStar => Some(CircumstellarProfile {
            env_type: 4.0,
            sub_mode: 1.0,
            shell_scale: 2.10,
            primary_color: Vec4::new(1.35, 0.75, 0.28, 1.0),
            secondary_color: Vec4::new(0.75, 0.28, 0.12, 1.0),
            kinematics: Vec4::new(10.0, 1.05, 0.80, 0.6),
        }),
        BodyType::PopulationIIIStar => Some(CircumstellarProfile {
            env_type: 5.0,
            sub_mode: 0.0,
            shell_scale: 2.40,
            primary_color: Vec4::new(0.82, 1.05, 1.95, 1.0),
            secondary_color: Vec4::new(1.25, 0.75, 2.05, 1.0),
            kinematics: Vec4::new(11.0, 1.15, 0.85, 1.1),
        }),
        BodyType::DarkStar => Some(CircumstellarProfile {
            env_type: 5.0,
            sub_mode: 1.0,
            shell_scale: 2.20,
            primary_color: Vec4::new(1.35, 0.78, 0.24, 1.0),
            secondary_color: Vec4::new(1.15, 0.55, 1.45, 1.0),
            kinematics: Vec4::new(8.5, 1.05, 0.85, 0.5),
        }),
        _ => None,
    }
}

/// Builds `CircumstellarUniforms` for a star given its world position, visual radius, spin state, and time.
pub fn compute_circumstellar_uniforms(
    profile: &CircumstellarProfile,
    world_pos: Vec3,
    visual_r: f32,
    opt_spin: Option<&SpinState>,
    elapsed_secs: f32,
) -> CircumstellarUniforms {
    let spin_axis = opt_spin.map_or(Vec3::Y, |s| {
        if s.spin_vector.length_squared() > 1e-12 {
            let n = s.spin_vector.normalize();
            Vec3::new(n.x as f32, n.y as f32, n.z as f32)
        } else {
            let tilt = (s.axial_tilt_degrees as f32).to_radians();
            Vec3::new(tilt.sin(), tilt.cos(), 0.0)
        }
    });
    let outer_r = (visual_r * profile.shell_scale).max(0.002);

    CircumstellarUniforms {
        center_and_outer_r: Vec4::new(world_pos.x, world_pos.y, world_pos.z, outer_r),
        mode_and_time: Vec4::new(visual_r, profile.env_type, profile.sub_mode, elapsed_secs),
        spin_axis_and_asymmetry: Vec4::new(spin_axis.x, spin_axis.y, spin_axis.z, 1.0),
        primary_color: profile.primary_color,
        secondary_color: profile.secondary_color,
        kinematics: profile.kinematics,
    }
}

fn update_or_despawn_circumstellar_children(
    commands: &mut Commands,
    children: &Children,
    opt_target: Option<(f32, &CircumstellarUniforms)>,
    child_query: &mut Query<
        (&mut Transform, &MeshMaterial3d<CircumstellarMaterial>),
        (With<VisualCircumstellarChild>, Without<VisualBody>),
    >,
    materials: &mut Assets<CircumstellarMaterial>,
) -> bool {
    let mut found = false;
    for child in children.iter() {
        if let Ok((mut transform, mat_handle)) = child_query.get_mut(child) {
            if let Some((scale, uniforms)) = opt_target {
                found = true;
                transform.scale = Vec3::splat(scale);
                if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                    mat.uniforms = uniforms.clone();
                }
            } else if let Ok(mut c_cmd) = commands.get_entity(child) {
                c_cmd.try_despawn();
            }
        }
    }
    found
}

/// Synchronizes 3D volumetric circumstellar nebulae, colliding-wind pinwheels, and relativistic lensing shells.
#[allow(
    clippy::type_complexity,
    reason = "Circumstellar environment synchronization ECS query"
)]
pub fn sync_circumstellar_environments(
    mut commands: Commands,
    time: Res<Time>,
    sim_time: Option<Res<SimTime>>,
    visual_assets: Option<Res<VisualAssets>>,
    mut materials: ResMut<Assets<CircumstellarMaterial>>,
    stars_query: Query<
        (
            Entity,
            &SimPosition,
            &Radius,
            &Transform,
            &CelestialBody,
            Option<&SpinState>,
            Option<&Children>,
        ),
        With<VisualBody>,
    >,
    mut child_query: Query<
        (&mut Transform, &MeshMaterial3d<CircumstellarMaterial>),
        (With<VisualCircumstellarChild>, Without<VisualBody>),
    >,
) {
    let Some(assets) = visual_assets else {
        return;
    };
    let elapsed = sim_time
        .as_deref()
        .map_or_else(|| time.elapsed_secs(), |st| st.visual_time_secs);

    for (star_ent, pos, rad, trans, body, opt_spin, opt_children) in stars_query.iter() {
        let Some(profile) = resolve_circumstellar_profile(body.body_type) else {
            if let Some(children) = opt_children {
                update_or_despawn_circumstellar_children(
                    &mut commands,
                    children,
                    None,
                    &mut child_query,
                    &mut materials,
                );
            }
            continue;
        };

        let visual_r = trans.scale.x.max(rad.0 as f32 * 0.1);
        let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let uniforms =
            compute_circumstellar_uniforms(&profile, world_pos, visual_r, opt_spin, elapsed);

        let found = opt_children.is_some_and(|children| {
            update_or_despawn_circumstellar_children(
                &mut commands,
                children,
                Some((profile.shell_scale, &uniforms)),
                &mut child_query,
                &mut materials,
            )
        });

        if !found {
            let mat_handle = materials.add(CircumstellarMaterial { uniforms });
            if let Ok(mut e_cmd) = commands.get_entity(star_ent) {
                e_cmd.with_children(|parent| {
                    parent.spawn((
                        VisualCircumstellarChild,
                        Mesh3d(assets.atmosphere_mesh.clone()),
                        MeshMaterial3d(mat_handle),
                        Transform::from_scale(Vec3::splat(profile.shell_scale)),
                        Visibility::default(),
                        NotShadowCaster,
                    ));
                });
            }
        }
    }
}
