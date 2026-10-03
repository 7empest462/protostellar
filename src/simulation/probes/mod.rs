use bevy::math::DVec3;
use bevy::prelude::*;

use crate::game::ui::NotificationToast;
use crate::simulation::components::*;
use crate::simulation::resources::{PlayerInteractionState, SimulationConfig};

pub mod navigation;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeType {
    Orbiter,
    Rover,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeStatus {
    Transit,
    InOrbit,
    Landed,
}

#[derive(Component, Debug, Clone)]
pub struct SpaceProbe {
    pub probe_type: ProbeType,
    pub status: ProbeStatus,
    pub id: usize,
    pub orientation: bevy::math::DQuat,
    pub landed_unspun_pos: Option<bevy::math::DVec3>,
    pub orbit_axis: Option<bevy::math::DVec3>,
    pub arrival_time: Option<f64>,
}

#[derive(Component, Debug, Clone)]
pub struct FlightComputer {
    pub target_entity: Entity,
    pub target_name: String,
    pub launch_pos: DVec3,
    pub distance_to_target: f64,
    pub relative_speed: f64,
}

#[derive(Component, Debug, Clone)]
pub struct LaunchProbeRequest {
    pub target: Entity,
}

#[derive(Resource, Default)]
pub struct ProbeCounter(pub usize);

pub struct ProbesPlugin;

impl Plugin for ProbesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProbeCounter>()
            .add_systems(Update, process_launch_requests)
            .add_systems(
                Update,
                (
                    navigation::probe_navigation_system
                        .after(crate::simulation::step_physics_simulation),
                    sync_probe_transforms.after(navigation::probe_navigation_system),
                ),
            );
    }
}

pub fn sync_probe_transforms(
    mut probes: Query<(
        &mut Transform,
        &crate::simulation::components::SimPosition,
        &SpaceProbe,
    )>,
    config: Option<Res<crate::simulation::resources::SimulationConfig>>,
) {
    let min_vis_r = config
        .as_ref()
        .map_or(0.00008, |c| c.min_body_visual_radius * 0.08);
    for (mut transform, pos, probe) in probes.iter_mut() {
        transform.translation = pos.0.as_vec3();
        transform.scale = Vec3::splat(min_vis_r);
        transform.rotation = Quat::from_xyzw(
            probe.orientation.x as f32,
            probe.orientation.y as f32,
            probe.orientation.z as f32,
            probe.orientation.w as f32,
        );
    }
}

fn process_launch_requests(
    mut commands: Commands,
    requests: Query<(Entity, &LaunchProbeRequest)>,
    targets: Query<TargetData>,
    candidates: Query<LaunchCandidateData, Without<SpaceProbe>>,
    mut probe_counter: ResMut<ProbeCounter>,
    mut toast: Option<ResMut<NotificationToast>>,
    mut opt_meshes: Option<ResMut<Assets<Mesh>>>,
    mut opt_materials: Option<ResMut<Assets<StandardMaterial>>>,
    config: Option<Res<SimulationConfig>>,
) {
    for (req_ent, req) in requests.iter() {
        if let Ok(mut cmd) = commands.get_entity(req_ent) {
            cmd.try_despawn();
        }

        // 1. Identify best civilized origin world in the system
        let (origin_entity, origin_name, origin_pos, origin_vel, origin_rad, origin_body_type) =
            find_launch_origin(req.target, &candidates);

        // 2. Resolve destination target
        // If the user triggered launch while inspecting the civilized origin itself (e.g. Earth),
        // we automatically select the best planetary destination to explore (e.g. Moon, Mars, Venus)!
        let (
            effective_target,
            target_name,
            target_pos,
            _target_vel,
            _target_rad,
            is_gas_giant_or_star,
        ) = resolve_destination_target(
            req.target,
            origin_entity,
            origin_pos,
            &origin_name,
            origin_vel,
            origin_rad,
            &candidates,
            &targets,
        );

        // Aim probe vector directly from origin to target
        let to_target = target_pos - origin_pos;
        let dir = if to_target.length_squared() > 1e-8 {
            to_target.normalize()
        } else {
            DVec3::X
        };

        let origin_vis_rad = config.as_ref().map_or(origin_rad, |c| {
            f64::from(c.calc_visual_radius_for_type(origin_rad, origin_body_type))
        });

        let dist_to_target = to_target.length();
        // Start safely outside the origin world's visual surface/atmosphere, scaled appropriately for target proximity
        let spawn_offset = (origin_vis_rad * 1.25)
            .min(dist_to_target * 0.20)
            .max(0.0008);
        let start_pos = origin_pos + dir * spawn_offset;
        // Initial cruise boost scaled by journey distance (gentle for moons, fast for outer planets)
        let initial_boost = (dist_to_target * 5.0).clamp(0.4, 4.0);
        let start_vel = origin_vel + dir * initial_boost;

        probe_counter.0 += 1;
        let probe_id = probe_counter.0;

        let is_orbiter = is_gas_giant_or_star || rand::random::<bool>();
        let (probe_type, probe_label) = if is_orbiter {
            (ProbeType::Orbiter, "Orbiter")
        } else {
            (ProbeType::Rover, "Rover")
        };

        let full_name = format!("🚀 {probe_label} #{probe_id} (to {target_name})");

        let min_vis_r = config
            .as_ref()
            .map_or(0.00035, |c| c.min_body_visual_radius * 0.45);

        let probe_bundle = (
            SpaceProbe {
                probe_type,
                status: ProbeStatus::Transit,
                id: probe_id,
                orientation: bevy::math::DQuat::IDENTITY,
                landed_unspun_pos: None,
                orbit_axis: None,
                arrival_time: None,
            },
            FlightComputer {
                target_entity: effective_target,
                target_name: target_name.clone(),
                launch_pos: start_pos,
                distance_to_target: dist_to_target,
                relative_speed: initial_boost,
            },
            SimPosition(start_pos),
            SimVelocity(start_vel),
            SimAcceleration::default(),
            Mass(0.0),
            Radius(1e-7),
            Temperature(290.0),
            Composition::metal_rich(),
            CelestialBody {
                body_type: BodyType::Asteroid,
                name: full_name.clone(),
            },
            (
                Transform::from_translation(start_pos.as_vec3()).with_scale(Vec3::splat(min_vis_r)),
                GlobalTransform::default(),
                Visibility::default(),
                InheritedVisibility::default(),
                ViewVisibility::default(),
                crate::rendering::bodies::VisualBody,
            ),
        );

        let probe_entity = commands.spawn(probe_bundle).id();

        if let (Some(ref mut meshes), Some(ref mut materials)) =
            (&mut opt_meshes, &mut opt_materials)
        {
            spawn_probe_mesh_hierarchy(&mut commands, probe_entity, is_orbiter, meshes, materials);
        }

        // NOTE: We intentionally DO NOT hijack player_state.selected_entity.
        // Earth (or the origin world) remains selected so the player's UI and timeline remain intact!
        if let Some(ref mut t) = toast {
            t.message = format!(
                "🚀 Launched {probe_label} #{probe_id} from {origin_name} to explore {target_name}! Click [{probe_label} #{probe_id}] below to track."
            );
            t.timer = 5.0;
        }

        bevy::log::info!("🚀 {} launched toward {}", full_name, target_name);
    }
}

/// Renders persistent glowing beacons, trajectory guidelines, and targeting reticles for all probes.
pub fn draw_probe_navigation_gizmos(
    mut gizmos: Gizmos,
    probes: Query<(Entity, &SimPosition, &SpaceProbe, &FlightComputer)>,
    targets: Query<&SimPosition>,
    player_state: Option<Res<PlayerInteractionState>>,
    camera_query: Query<&Transform, With<Camera>>,
    sim_time: Option<Res<crate::simulation::resources::SimTime>>,
) {
    let cam_pos = camera_query.iter().next().map(|t| t.translation);
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);

    for (probe_ent, pos, probe, computer) in probes.iter() {
        let probe_vec = pos.0.as_vec3();
        let cam_dist = cam_pos.map_or(20.0, |cp| cp.distance(probe_vec));
        let is_selected = player_state
            .as_ref()
            .is_some_and(|ps| ps.selected_entity == Some(probe_ent));

        // Vibrant neon palette: Cyan for Orbiters, Amber-Gold for Rovers
        let (main_color, glow_color) = match probe.probe_type {
            ProbeType::Orbiter => (
                Color::srgb(0.2, 0.9, 1.0),
                Color::srgba(0.2, 0.9, 1.0, 0.45),
            ),
            ProbeType::Rover => (
                Color::srgb(1.0, 0.75, 0.2),
                Color::srgba(1.0, 0.75, 0.2, 0.45),
            ),
        };

        // Scale beacon size with camera distance so it is ALWAYS clearly visible across the entire solar system
        let pulse = (visual_time * 4.0).sin() * 0.15 + 1.0;
        let beacon_r = (cam_dist * 0.022 * pulse).clamp(0.015, 2.5);

        // 1. Camera-facing billboard ring (never collapses edge-on from any viewing angle!)
        let to_cam = cam_pos.map_or(Vec3::Y, |cp| (cp - probe_vec).normalize_or_zero());
        let billboard_rot = if to_cam.length_squared() > 1e-6 {
            Quat::from_rotation_arc(Vec3::Z, to_cam)
        } else {
            Quat::IDENTITY
        };

        gizmos.circle(
            Isometry3d::new(probe_vec, billboard_rot),
            beacon_r,
            main_color,
        );
        gizmos.circle(
            Isometry3d::new(probe_vec, billboard_rot),
            beacon_r * 0.55,
            glow_color,
        );

        // 2. 3D Gyroscope orthogonal rings (XZ horizontal and XY vertical)
        gizmos.circle(
            Isometry3d::new(
                probe_vec,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            beacon_r * 0.85,
            glow_color,
        );
        gizmos.circle(
            Isometry3d::new(probe_vec, Quat::IDENTITY),
            beacon_r * 0.85,
            glow_color,
        );

        // 3. Draw crosshairs in camera billboard plane
        let right = billboard_rot * Vec3::X;
        let up = billboard_rot * Vec3::Y;
        let tick_len = beacon_r * 0.45;
        gizmos.line(
            probe_vec - right * (beacon_r + tick_len),
            probe_vec + right * (beacon_r + tick_len),
            main_color,
        );
        gizmos.line(
            probe_vec - up * (beacon_r + tick_len),
            probe_vec + up * (beacon_r + tick_len),
            main_color,
        );

        // 4. Draw vertical skyward beacon so probe can be located even from extreme orbital angles
        let beacon_h = (cam_dist * 0.08).clamp(0.05, 8.0);
        gizmos.line(probe_vec, probe_vec + Vec3::Y * beacon_h, glow_color);
        gizmos.sphere(
            Isometry3d::from_translation(probe_vec + Vec3::Y * beacon_h),
            beacon_r * 0.25,
            main_color,
        );

        // 5. Draw trajectory guide line from probe directly to its destination planet
        if let Ok(target_pos) = targets.get(computer.target_entity) {
            let target_vec = target_pos.0.as_vec3();
            let line_color = match probe.status {
                ProbeStatus::Transit => main_color,
                ProbeStatus::InOrbit => Color::srgba(0.3, 1.0, 0.5, 0.6),
                ProbeStatus::Landed => Color::srgba(1.0, 0.8, 0.3, 0.6),
            };
            gizmos.line(probe_vec, target_vec, line_color);
        }

        // 6. If selected: Draw prominent targeting reticle and outer pulse ring
        if is_selected {
            let select_r = beacon_r * 1.5;
            gizmos.circle(
                Isometry3d::new(probe_vec, billboard_rot),
                select_r,
                Color::srgb(1.0, 1.0, 1.0),
            );
            gizmos.circle(
                Isometry3d::new(probe_vec, billboard_rot),
                select_r * 1.15,
                Color::srgba(1.0, 1.0, 1.0, 0.4),
            );
        }
    }
}

type LaunchCandidateData<'a> = (
    Entity,
    &'a crate::simulation::components::SimPosition,
    &'a crate::simulation::components::SimVelocity,
    Option<&'a Radius>,
    &'a CelestialBody,
    Option<&'a crate::simulation::components::BiosphereState>,
    Option<&'a crate::simulation::components::CentralStar>,
);

fn find_launch_origin(
    req_target: Entity,
    candidates: &Query<LaunchCandidateData, Without<SpaceProbe>>,
) -> (Entity, String, DVec3, DVec3, f64, BodyType) {
    let mut best_origin_score = -100.0f32;
    let mut origin_entity = req_target;
    let mut origin_pos = DVec3::ZERO;
    let mut origin_vel = DVec3::ZERO;
    let mut origin_rad = 0.005f64;
    let mut origin_body_type = BodyType::TerrestrialPlanet;
    let mut origin_name = "Solar Launch Station".to_string();

    for (ent, pos, vel, opt_r, body, opt_bio, opt_star) in candidates.iter() {
        let rad = opt_r.map_or(0.005, |r| r.0);
        if let Some(bio) = opt_bio {
            let score = 100.0 + bio.technosignature * 50.0;
            if score > best_origin_score {
                best_origin_score = score;
                origin_entity = ent;
                origin_pos = pos.0;
                origin_vel = vel.0;
                origin_rad = rad;
                origin_body_type = body.body_type;
                origin_name.clone_from(&body.name);
            }
        } else if body.name.to_lowercase().contains("earth") && best_origin_score < 50.0 {
            best_origin_score = 50.0;
            origin_entity = ent;
            origin_pos = pos.0;
            origin_vel = vel.0;
            origin_rad = rad;
            origin_body_type = body.body_type;
            origin_name.clone_from(&body.name);
        } else if opt_star.is_some() && best_origin_score < 0.0 {
            best_origin_score = 0.0;
            origin_entity = ent;
            origin_pos = pos.0;
            origin_vel = vel.0;
            origin_rad = rad;
            origin_body_type = body.body_type;
            origin_name.clone_from(&body.name);
        }
    }

    (
        origin_entity,
        origin_name,
        origin_pos,
        origin_vel,
        origin_rad,
        origin_body_type,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Mesh generation requires asset mutators and command queues"
)]
fn spawn_probe_mesh_hierarchy(
    commands: &mut Commands,
    probe_entity: Entity,
    is_orbiter: bool,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
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
        base_color: Color::srgb(0.8, 0.85, 0.9),
        metallic: 0.8,
        perceptual_roughness: 0.2,
        ..default()
    });

    let mat_panel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.15, 0.3),
        metallic: 0.9,
        perceptual_roughness: 0.2,
        ..default()
    });

    let mat_glow = materials.add(StandardMaterial {
        base_color,
        emissive,
        unlit: true,
        ..default()
    });

    if let Ok(mut entity_cmds) = commands.get_entity(probe_entity) {
        entity_cmds.with_children(|parent| {
            if is_orbiter {
                // Orbiter: cylinder body, cone nose, solar panels
                let body_mesh = meshes.add(Cylinder::new(0.4, 2.0));
                let nose_mesh = meshes.add(Cone {
                    radius: 0.4,
                    height: 1.0,
                });
                let panel_mesh = meshes.add(Cuboid::new(4.0, 0.05, 0.8));
                let engine_mesh = meshes.add(Cone {
                    radius: 0.25,
                    height: 0.6,
                });

                // Main body
                parent.spawn((
                    Mesh3d(body_mesh.clone()),
                    MeshMaterial3d(mat_metal.clone()),
                    Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    Visibility::default(),
                ));
                // Nose
                parent.spawn((
                    Mesh3d(nose_mesh),
                    MeshMaterial3d(mat_metal.clone()),
                    Transform::from_translation(Vec3::new(0.0, 0.0, -1.5))
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                    Visibility::default(),
                ));
                // Engine
                parent.spawn((
                    Mesh3d(engine_mesh),
                    MeshMaterial3d(mat_glow.clone()),
                    Transform::from_translation(Vec3::new(0.0, 0.0, 1.3))
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    Visibility::default(),
                ));
                // Solar Panels
                parent.spawn((
                    Mesh3d(panel_mesh),
                    MeshMaterial3d(mat_panel.clone()),
                    Transform::default(),
                    Visibility::default(),
                ));
            } else {
                // Rover: box body, small camera mast, wheels
                let body_mesh = meshes.add(Cuboid::new(1.2, 0.6, 1.8));
                let mast_mesh = meshes.add(Cylinder::new(0.05, 0.8));
                let wheel_mesh = meshes.add(Cylinder::new(0.3, 0.2));
                let camera_mesh = meshes.add(Cuboid::new(0.2, 0.15, 0.2));

                // Body
                parent.spawn((
                    Mesh3d(body_mesh),
                    MeshMaterial3d(mat_metal.clone()),
                    Transform::from_translation(Vec3::new(0.0, 0.3, 0.0)),
                    Visibility::default(),
                ));
                // Mast
                parent.spawn((
                    Mesh3d(mast_mesh),
                    MeshMaterial3d(mat_metal.clone()),
                    Transform::from_translation(Vec3::new(0.4, 1.0, -0.6)),
                    Visibility::default(),
                ));
                // Camera
                parent.spawn((
                    Mesh3d(camera_mesh),
                    MeshMaterial3d(mat_glow.clone()),
                    Transform::from_translation(Vec3::new(0.4, 1.4, -0.6)),
                    Visibility::default(),
                ));
                // Wheels
                let wheel_rot = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
                for &x in &[-0.7, 0.7] {
                    for &z in &[-0.7, 0.0, 0.7] {
                        parent.spawn((
                            Mesh3d(wheel_mesh.clone()),
                            MeshMaterial3d(mat_panel.clone()),
                            Transform::from_translation(Vec3::new(x, 0.3, z))
                                .with_rotation(wheel_rot),
                            Visibility::default(),
                        ));
                    }
                }
            }
        });
    }
}

type TargetData<'a> = (
    &'a SimPosition,
    &'a SimVelocity,
    Option<&'a Radius>,
    &'a CelestialBody,
);

#[allow(
    clippy::too_many_arguments,
    reason = "Target resolution requires origin parameters and candidate/target queries"
)]
fn resolve_destination_target(
    req_target: Entity,
    origin_entity: Entity,
    origin_pos: DVec3,
    origin_name: &str,
    origin_vel: DVec3,
    origin_rad: f64,
    candidates: &Query<LaunchCandidateData, Without<SpaceProbe>>,
    targets: &Query<TargetData>,
) -> (Entity, String, DVec3, DVec3, f64, bool) {
    if req_target == origin_entity {
        let mut best_target_score = -1000.0f32;
        let mut chosen_target = None;

        for (ent, pos, vel, opt_r, body, _opt_bio, opt_star) in candidates.iter() {
            if ent == origin_entity || opt_star.is_some() {
                continue;
            }
            let lower = body.name.to_lowercase();
            let rad = opt_r.map_or(0.005, |r| r.0);
            let dist = (pos.0 - origin_pos).length() as f32;

            let mut score = match body.body_type {
                BodyType::Moon => 1000.0,
                BodyType::TerrestrialPlanet => 700.0,
                BodyType::SuperEarth | BodyType::Protoplanet => 600.0,
                BodyType::GasGiant | BodyType::IceGiant => 400.0,
                _ => 200.0,
            };
            if lower.contains("moon") || lower.contains("luna") {
                score += 300.0;
            } else if lower.contains("mars") {
                score += 250.0;
            } else if lower.contains("venus") {
                score += 200.0;
            } else if lower.contains("jupiter") {
                score += 150.0;
            }
            // Favor closer destinations if scores tie
            score -= dist * 2.0;

            if score > best_target_score {
                best_target_score = score;
                let is_gas_or_star =
                    matches!(body.body_type, BodyType::GasGiant | BodyType::IceGiant);
                chosen_target = Some((ent, body.name.clone(), pos.0, vel.0, rad, is_gas_or_star));
            }
        }

        if let Some(target_info) = chosen_target {
            target_info
        } else {
            // Fallback if no other celestial bodies exist: orbital survey of origin world
            (
                origin_entity,
                origin_name.to_string(),
                origin_pos,
                origin_vel,
                origin_rad,
                false,
            )
        }
    } else {
        // User explicitly selected a target world (e.g. Mars, Jupiter, Asteroid)
        let Ok((t_pos, t_vel, t_rad, t_body)) = targets.get(req_target) else {
            return (
                origin_entity,
                origin_name.to_string(),
                origin_pos,
                origin_vel,
                origin_rad,
                false,
            );
        };
        let is_gas_or_star = matches!(t_body.body_type, BodyType::GasGiant | BodyType::IceGiant)
            || t_body.body_type.is_star_or_remnant();
        (
            req_target,
            t_body.name.clone(),
            t_pos.0,
            t_vel.0,
            t_rad.map_or(0.005, |r| r.0),
            is_gas_or_star,
        )
    }
}
