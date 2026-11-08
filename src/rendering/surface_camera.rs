//! Phase 9B: First-Person Rover & Planetary Surface Horizon Camera Mode.
//!
//! Anchors the 3D camera directly to a landed `SpaceProbe` rover (or any selected
//! planet/moon surface) in its local rotating planetographic tangent frame
//! `(East, Zenith-Up, North/Heading)`, allowing the player to look up at the
//! Rayleigh/Mie scattered alien sky, planetary rings, moons, and parent stars,
//! drive across the spherical surface with `WASD`, and observe 3D procedural
//! alien flora, civilization biodomes, and geological formations on the horizon.

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::math::{DQuat, DVec3};
use bevy::prelude::*;

use crate::game::ui::NotificationToast;
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::celestial::SpinState;
use crate::simulation::components::{
    BiosphereState, BodyType, CelestialBody, Composition, Mass, Radius, SimPosition, Temperature,
};
use crate::simulation::probes::navigation::compute_planet_spin_frame;
use crate::simulation::probes::{FlightComputer, ProbeStatus, ProbeType, SpaceProbe};
use crate::simulation::resources::{PlayerInteractionState, SimTime, SimulationConfig};
use crate::utils::constants::AU_TO_KM;

/// Computes an orthonormal right-handed surface tangent frame `(east, up, north)`
/// where `up` is the unit planetary zenith normal and `north` is the forward
/// tangent direction along the horizon.
pub fn compute_surface_tangent_basis(up: Vec3, heading_hint: Vec3) -> (Vec3, Vec3, Vec3) {
    let n_up = up.normalize_or_zero();
    let n_up = if n_up.length_squared() > 0.5 {
        n_up
    } else {
        Vec3::Y
    };

    let projected = heading_hint.reject_from(n_up).normalize_or_zero();
    let north = if projected.length_squared() > 0.5 {
        projected
    } else {
        let fallback = if n_up.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        fallback.reject_from(n_up).normalize_or_zero()
    };
    let east = north.cross(n_up).normalize_or_zero();
    let north = n_up.cross(east).normalize_or_zero();
    (east, n_up, north)
}

/// Computes the world-space camera translation and orientation for Surface Horizon Mode.
pub fn compute_surface_horizon_camera_pose(
    planet_pos: Vec3,
    vis_rad: f32,
    up: Vec3,
    heading_hint: Vec3,
    altitude_scale: f32,
    azimuth_rad: f32,
    elevation_rad: f32,
) -> (Vec3, Quat) {
    let (east, n_up, north) = compute_surface_tangent_basis(up, heading_hint);
    let alt_factor = 1.0 + altitude_scale.clamp(0.004, 0.25);
    let eye_pos = planet_pos + n_up * (vis_rad * alt_factor);

    // Base camera orientation looks forward along +north (-Z = north, +Y = n_up, +X = east)
    let base_rot = Quat::from_mat3(&Mat3::from_cols(east, n_up, -north));
    let yaw_rot = Quat::from_axis_angle(Vec3::Y, -azimuth_rad);
    let pitch_rot = Quat::from_axis_angle(Vec3::X, elevation_rad.clamp(-0.55, 1.48));
    (eye_pos, base_rot * yaw_rot * pitch_rot)
}

/// Toggles First-Person Surface / Rover Horizon Camera mode on the selected entity
/// (or automatically locks onto a landed Rover / planet if a star is selected).
pub fn toggle_surface_horizon_mode(
    cam: &mut PanOrbitCamera,
    selected_entity: Option<Entity>,
    toast: &mut NotificationToast,
) {
    cam.surface_horizon_mode = !cam.surface_horizon_mode;
    if cam.surface_horizon_mode {
        if let Some(target) = selected_entity.or(cam.target_entity) {
            cam.target_entity = Some(target);
        }
        cam.surface_elevation = 0.12;
        if cam.surface_altitude_scale <= 0.001 {
            cam.surface_altitude_scale = 0.022;
        }
        cam.entry_transition_progress = 0.0;
        cam.surface_entry_start_offset = Vec3::ZERO;
        toast.message =
            "🎥 SURFACE HORIZON CAM ACTIVE [Shift-V] // WASD: Drive | R-Drag: Look 360° | Scroll: Altitude | Esc/F: Orbital View"
                .to_string();
        toast.timer = 5.0;
    } else {
        cam.entry_transition_progress = 1.0;
        toast.message = "🛰️ Returned to 3D Orbital Camera View".to_string();
        toast.timer = 3.0;
    }
}

/// Converts planetographic `(lat_rad, lon_rad)` to a unit vector in the planet's unspun frame.
pub fn lat_lon_to_unspun_normal(lat_rad: f32, lon_rad: f32) -> Vec3 {
    let (sin_lat, cos_lat) = lat_rad.sin_cos();
    let (sin_lon, cos_lon) = lon_rad.sin_cos();
    Vec3::new(cos_lat * cos_lon, sin_lat, cos_lat * sin_lon).normalize_or_zero()
}

/// Steps keyboard driving (`WASD`) and mouse look/altitude controls while in Surface Horizon Mode.
pub fn step_surface_horizon_controls(
    keyboard_input: &ButtonInput<KeyCode>,
    delta_yaw: f32,
    delta_pitch: f32,
    scroll: f32,
    camera: &mut PanOrbitCamera,
) -> (f32, f32) {
    camera.surface_azimuth += delta_yaw * 1.35;
    camera.surface_elevation = (camera.surface_elevation - delta_pitch * 1.25).clamp(-0.52, 1.48);

    if scroll.abs() > 0.0 {
        let zoom_factor = (-scroll * 0.08).exp();
        camera.surface_altitude_scale =
            (camera.surface_altitude_scale * zoom_factor).clamp(0.004, 0.22);
    }

    let mut forward_drive = 0.0f32;
    let mut turn_drive = 0.0f32;
    if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
        forward_drive += 0.012;
    }
    if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
        forward_drive -= 0.012;
    }
    if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
        turn_drive -= 0.025;
        camera.surface_azimuth -= 0.025;
    }
    if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
        turn_drive += 0.025;
        camera.surface_azimuth += 0.025;
    }

    // Update virtual explorer lat/lon when standing directly on a planet/moon
    if forward_drive.abs() > 0.0 {
        let look_heading = camera.surface_azimuth;
        camera.surface_lat_lon.x =
            (camera.surface_lat_lon.x + forward_drive * look_heading.cos()).clamp(-1.35, 1.35);
        let cos_lat = camera.surface_lat_lon.x.cos().max(0.25);
        camera.surface_lat_lon.y -= (forward_drive * look_heading.sin()) / cos_lat;
    }

    (forward_drive, turn_drive)
}

/// Applies manual rover geodesic driving when mounted on a landed `SpaceProbe`.
pub fn apply_manual_rover_drive(
    probe: &mut SpaceProbe,
    forward_drive: f32,
    turn_drive: f32,
    physical_rad_au: f64,
) {
    if probe.status != ProbeStatus::Landed
        || (forward_drive.abs() < 1e-6 && turn_drive.abs() < 1e-6)
    {
        return;
    }
    let unspun = probe
        .landed_unspun_pos
        .unwrap_or(DVec3::Y)
        .normalize_or_zero();
    let ref_axis = if unspun.y.abs() < 0.9 {
        DVec3::Y
    } else {
        DVec3::X
    };
    let mut heading = probe
        .rover_heading_unspun
        .unwrap_or_else(|| unspun.cross(ref_axis).normalize_or_zero());

    if turn_drive.abs() > 1e-6 {
        heading = (DQuat::from_axis_angle(unspun, f64::from(-turn_drive)) * heading)
            .reject_from(unspun)
            .normalize_or_zero();
        probe.rover_heading_unspun = Some(heading);
    }

    if forward_drive.abs() > 1e-6 {
        let axis = unspun.cross(heading).normalize_or_zero();
        if axis.length_squared() > 0.5 {
            let step_rot = DQuat::from_axis_angle(axis, f64::from(forward_drive));
            let new_unspun = (step_rot * unspun).normalize_or_zero();
            let new_heading = (step_rot * heading)
                .reject_from(new_unspun)
                .normalize_or_zero();
            probe.landed_unspun_pos = Some(new_unspun);
            probe.rover_heading_unspun = Some(new_heading);

            let planet_rad_km = (physical_rad_au * AU_TO_KM).max(150.0);
            probe.distance_driven_km += f64::from(forward_drive.abs()) * planet_rad_km;

            if probe
                .traverse_trail_unspun
                .last()
                .is_none_or(|last| last.angle_between(new_unspun) >= 0.035)
            {
                if probe.traverse_trail_unspun.len() >= 24 {
                    probe.traverse_trail_unspun.remove(0);
                }
                probe.traverse_trail_unspun.push(new_unspun);
            }
        }
    }
}

type SurfaceWorldQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    &'a Radius,
    &'a Mass,
    &'a Temperature,
    &'a Composition,
    &'a CelestialBody,
    Option<&'a SpinState>,
    Option<&'a BiosphereState>,
    Option<&'a Transform>,
);

fn extract_probe_surface_vectors(probe: &SpaceProbe) -> (Vec3, Vec3) {
    let unspun = probe
        .landed_unspun_pos
        .unwrap_or(DVec3::Y)
        .as_vec3()
        .normalize_or_zero();
    let h = probe
        .rover_heading_unspun
        .unwrap_or(DVec3::Z)
        .as_vec3()
        .normalize_or_zero();
    (unspun, h)
}

fn resolve_surface_vantage_point(
    target_ent: Option<Entity>,
    surface_lat_lon: Vec2,
    worlds_query: &Query<SurfaceWorldQueryItem, (Without<SpaceProbe>, Without<PanOrbitCamera>)>,
    probes_query: &Query<
        (Entity, &mut SpaceProbe, &FlightComputer, &mut SimPosition),
        Without<PanOrbitCamera>,
    >,
) -> Option<(Entity, Option<Entity>, Vec3, Vec3)> {
    if let Some(ent) = target_ent {
        if let Ok((probe_ent, probe, comp, _)) = probes_query.get(ent) {
            let (unspun, h) = extract_probe_surface_vectors(probe);
            return Some((comp.target_entity, Some(probe_ent), unspun, h));
        }

        if let Ok((.., body, _, _, _)) = worlds_query.get(ent) {
            if !body.body_type.is_star_or_remnant() {
                for (probe_ent, probe, comp, _) in probes_query.iter() {
                    if comp.target_entity == ent
                        && probe.status == ProbeStatus::Landed
                        && probe.probe_type == ProbeType::Rover
                    {
                        let (unspun, h) = extract_probe_surface_vectors(probe);
                        return Some((ent, Some(probe_ent), unspun, h));
                    }
                }
                let u = lat_lon_to_unspun_normal(surface_lat_lon.x, surface_lat_lon.y);
                return Some((ent, None, u, Vec3::Y));
            }
        }
    }

    // Fallback when a star or nothing is selected: prefer any landed Rover first, then any Rover
    for require_landed in [true, false] {
        for (probe_ent, probe, comp, _) in probes_query.iter() {
            if probe.probe_type == ProbeType::Rover
                && (!require_landed || probe.status == ProbeStatus::Landed)
                && worlds_query
                    .get(comp.target_entity)
                    .is_ok_and(|(.., b, _, _, _)| !b.body_type.is_star_or_remnant())
            {
                let (unspun, h) = extract_probe_surface_vectors(probe);
                return Some((comp.target_entity, Some(probe_ent), unspun, h));
            }
        }
    }

    // Otherwise fall back to the first non-star world in the system
    for (world_ent, .., body, _, _, _) in worlds_query.iter() {
        if !body.body_type.is_star_or_remnant() {
            let u = lat_lon_to_unspun_normal(surface_lat_lon.x, surface_lat_lon.y);
            return Some((world_ent, None, u, Vec3::Y));
        }
    }

    None
}

fn compute_effective_surface_radius(
    _unspun_norm: Vec3,
    _comp: &Composition,
    _temp_k: f32,
    _body_type: BodyType,
    _opt_bio: Option<&BiosphereState>,
    vis_rad: f32,
) -> f32 {
    vis_rad
}

fn read_surface_mouse_and_scroll_deltas(
    mouse_buttons: &ButtonInput<MouseButton>,
    mouse_motion_events: &mut MessageReader<MouseMotion>,
    mouse_wheel_events: &mut MessageReader<MouseWheel>,
    orbit_sensitivity: f32,
) -> (f32, f32, f32) {
    let mut delta_yaw = 0.0f32;
    let mut delta_pitch = 0.0f32;
    if mouse_buttons.pressed(MouseButton::Right) || mouse_buttons.pressed(MouseButton::Left) {
        for ev in mouse_motion_events.read() {
            delta_yaw += ev.delta.x * orbit_sensitivity;
            delta_pitch += ev.delta.y * orbit_sensitivity;
        }
    } else {
        mouse_motion_events.clear();
    }

    let mut scroll = 0.0f32;
    for ev in mouse_wheel_events.read() {
        scroll += match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 24.0,
        };
    }
    (delta_yaw, delta_pitch, scroll)
}

#[inline]
fn sync_orbital_near_plane(projection: &mut Projection, orbital_radius: f32) {
    if let Projection::Perspective(ref mut p) = *projection {
        let desired_near = (orbital_radius.max(1e-6) * 0.02).clamp(1e-7, 0.0001);
        if (p.near - desired_near).abs() > desired_near * 0.05 {
            p.near = desired_near;
        }
    }
}

fn process_surface_camera_inputs(
    camera: &mut PanOrbitCamera,
    mouse_buttons: &ButtonInput<MouseButton>,
    keyboard_input: &ButtonInput<KeyCode>,
    mouse_motion_events: &mut MessageReader<MouseMotion>,
    mouse_wheel_events: &mut MessageReader<MouseWheel>,
    text_focused: bool,
    player_target: Option<Entity>,
    toast: &mut NotificationToast,
) -> (f32, f32) {
    let is_shift =
        keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight);
    if !text_focused && is_shift && keyboard_input.just_pressed(KeyCode::KeyV) {
        toggle_surface_horizon_mode(camera, player_target, toast);
    }

    if !camera.surface_horizon_mode {
        return (0.0, 0.0);
    }

    let (delta_yaw, delta_pitch, scroll) = read_surface_mouse_and_scroll_deltas(
        mouse_buttons,
        mouse_motion_events,
        mouse_wheel_events,
        camera.orbit_sensitivity,
    );

    if text_focused {
        (0.0, 0.0)
    } else {
        step_surface_horizon_controls(keyboard_input, delta_yaw, delta_pitch, scroll, camera)
    }
}

/// Synchronizes the 3D camera transform, near clipping plane, and manual rover driving
/// when First-Person Surface / Rover Horizon Mode (`Shift + V`) is active.
#[allow(
    clippy::too_many_arguments,
    reason = "Coordinates input events, camera projection, world state, and landed rover driving"
)]
pub fn update_surface_horizon_camera(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    player_state: Res<PlayerInteractionState>,
    config: Res<SimulationConfig>,
    sim_time: Option<Res<SimTime>>,
    time: Option<Res<Time>>,
    mut toast: ResMut<NotificationToast>,
    mut camera_query: Query<
        (&mut PanOrbitCamera, &mut Transform, &mut Projection),
        Without<SpaceProbe>,
    >,
    worlds_query: Query<SurfaceWorldQueryItem, (Without<SpaceProbe>, Without<PanOrbitCamera>)>,
    mut probes_query: Query<
        (Entity, &mut SpaceProbe, &FlightComputer, &mut SimPosition),
        Without<PanOrbitCamera>,
    >,
    (focus, editable_query): (
        Option<Res<bevy::input_focus::InputFocus>>,
        Query<(), With<bevy::text::EditableText>>,
    ),
) {
    let Ok((mut camera, mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    let text_focused =
        crate::game::ui::widgets::is_text_input_focused(focus.as_deref(), &editable_query);

    let (forward_drive, turn_drive) = process_surface_camera_inputs(
        &mut camera,
        &mouse_buttons,
        &keyboard_input,
        &mut mouse_motion_events,
        &mut mouse_wheel_events,
        text_focused,
        player_state.selected_entity,
        &mut toast,
    );

    if !camera.surface_horizon_mode {
        sync_orbital_near_plane(&mut projection, camera.radius);
        return;
    }

    let candidate_target = player_state.selected_entity.or(camera.target_entity);
    let Some((host_ent, opt_rover_ent, mut unspun_norm, mut heading_unspun)) =
        resolve_surface_vantage_point(
            candidate_target,
            camera.surface_lat_lon,
            &worlds_query,
            &probes_query,
        )
    else {
        camera.surface_horizon_mode = false;
        toast.message =
            "⚠️ Cannot land on a star or compact remnant! Select a planet, moon, or rover."
                .to_string();
        toast.timer = 3.5;
        return;
    };

    camera.target_entity = opt_rover_ent.or(Some(host_ent));

    let Ok((_, pos, radius, _mass, temp, comp, body, opt_spin, opt_bio, opt_t)) =
        worlds_query.get(host_ent)
    else {
        camera.surface_horizon_mode = false;
        return;
    };

    let elapsed_years = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);
    let (spin_dir, spin_angle) = compute_planet_spin_frame(opt_spin, elapsed_years);
    let spin_rot_d = DQuat::from_axis_angle(spin_dir, spin_angle);
    let spin_rot = Quat::from_axis_angle(spin_dir.as_vec3(), spin_angle as f32);
    let vis_rad = crate::rendering::bodies::transforms::resolve_synced_visual_radius(
        opt_t,
        radius.0,
        body.body_type,
        pos.0.length() as f32,
        0.001,
        &config,
    );

    if let Some(rover_ent) = opt_rover_ent {
        sync_rover_surface_motion(
            rover_ent,
            &mut probes_query,
            &mut camera,
            forward_drive,
            turn_drive,
            radius.0,
            vis_rad,
            comp,
            temp.0,
            body.body_type,
            spin_rot_d,
            pos.0,
            &mut unspun_norm,
            &mut heading_unspun,
        );
    }

    let effective_rad = compute_effective_surface_radius(
        unspun_norm,
        comp,
        temp.0 as f32,
        body.body_type,
        opt_bio,
        vis_rad,
    );

    let planet_pos = pos.0.as_vec3();
    let world_up = (spin_rot * unspun_norm).normalize_or_zero();
    let world_heading = (spin_rot * heading_unspun).normalize_or_zero();
    let (eye_pos, eye_rot) = compute_surface_horizon_camera_pose(
        planet_pos,
        effective_rad,
        world_up,
        world_heading,
        camera.surface_altitude_scale,
        camera.surface_azimuth,
        camera.surface_elevation,
    );

    let dt = time.as_ref().map_or(0.016, |t| t.delta_secs());
    apply_surface_entry_descent(
        &mut camera,
        &mut transform,
        eye_pos,
        eye_rot,
        planet_pos,
        dt,
    );

    if let Projection::Perspective(ref mut p) = *projection {
        p.near = (vis_rad * 0.0005).clamp(1e-8, 0.00001);
    }
}

/// Smoothly descends from orbital vantage down to the surface horizon pose over ~1.0 second.
fn apply_surface_entry_descent(
    camera: &mut PanOrbitCamera,
    transform: &mut Transform,
    eye_pos: Vec3,
    eye_rot: Quat,
    planet_pos: Vec3,
    dt: f32,
) {
    if camera.entry_transition_progress < 1.0 && dt > 0.0 {
        if camera.surface_entry_start_offset == Vec3::ZERO {
            camera.surface_entry_start_offset = transform.translation - planet_pos;
            camera.surface_entry_start_rot = transform.rotation;
        }
        camera.entry_transition_progress = (camera.entry_transition_progress + dt * 1.0).min(1.0);
        let t = camera.entry_transition_progress;
        let smooth_t = t * t * (3.0 - 2.0 * t);
        let current_start_pos = planet_pos + camera.surface_entry_start_offset;
        transform.translation = current_start_pos.lerp(eye_pos, smooth_t);
        transform.rotation = camera.surface_entry_start_rot.slerp(eye_rot, smooth_t);
        if camera.entry_transition_progress >= 1.0 {
            transform.translation = eye_pos;
            transform.rotation = eye_rot;
        }
    } else {
        camera.entry_transition_progress = 1.0;
        transform.translation = eye_pos;
        transform.rotation = eye_rot;
    }

    camera.focus = eye_pos;
    camera.target_focus = planet_pos;
}

#[allow(
    clippy::too_many_arguments,
    reason = "Synchronizes rover probe physics, terrain elevation, and surface orientation"
)]
fn sync_rover_surface_motion(
    rover_ent: Entity,
    probes_query: &mut Query<
        (Entity, &mut SpaceProbe, &FlightComputer, &mut SimPosition),
        Without<PanOrbitCamera>,
    >,
    camera: &mut PanOrbitCamera,
    forward_drive: f32,
    turn_drive: f32,
    radius: f64,
    vis_rad: f32,
    comp: &Composition,
    temp_k: f64,
    body_type: crate::simulation::components::BodyType,
    spin_rot_d: DQuat,
    planet_pos: DVec3,
    unspun_norm: &mut Vec3,
    heading_unspun: &mut Vec3,
) {
    if let Ok((_, mut probe, _, mut probe_pos)) = probes_query.get_mut(rover_ent) {
        // Undo duplicate azimuth turn because apply_manual_rover_drive rotates rover_heading_unspun
        camera.surface_azimuth -= turn_drive;
        apply_manual_rover_drive(&mut probe, forward_drive, turn_drive, radius);
        let unspun_d = probe
            .landed_unspun_pos
            .unwrap_or(DVec3::Y)
            .normalize_or_zero();
        let h_d = probe
            .rover_heading_unspun
            .unwrap_or(DVec3::Z)
            .normalize_or_zero();
        *unspun_norm = unspun_d.as_vec3();
        *heading_unspun = h_d.as_vec3();
        let spun_d = spin_rot_d * unspun_d;
        let _ = (comp, temp_k, body_type);
        let rover_lift = vis_rad * 1.008;
        probe_pos.0 = planet_pos + spun_d * f64::from(rover_lift);
    }
}

/// Renders the holographic surface compass and heading clinometer ring when the camera
/// is in Surface Horizon Mode.
pub fn draw_surface_horizon_environment_gizmos(
    mut gizmos: Gizmos,
    camera_query: Query<&PanOrbitCamera>,
    worlds_query: Query<SurfaceWorldQueryItem, (Without<SpaceProbe>, Without<PanOrbitCamera>)>,
    probes_query: Query<
        (Entity, &mut SpaceProbe, &FlightComputer, &mut SimPosition),
        Without<PanOrbitCamera>,
    >,
    sim_time: Option<Res<SimTime>>,
    config: Res<SimulationConfig>,
) {
    let Ok(camera) = camera_query.single() else {
        return;
    };
    if !camera.surface_horizon_mode {
        return;
    }
    let elapsed_years = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);

    let Some((host_ent, _, unspun_norm, heading_unspun)) = resolve_surface_vantage_point(
        camera.target_entity,
        camera.surface_lat_lon,
        &worlds_query,
        &probes_query,
    ) else {
        return;
    };

    let Ok((_, pos, radius, _mass, _temp, _comp, body, opt_spin, _opt_bio, opt_t)) =
        worlds_query.get(host_ent)
    else {
        return;
    };

    let planet_pos = pos.0.as_vec3();
    let vis_rad = crate::rendering::bodies::transforms::resolve_synced_visual_radius(
        opt_t,
        radius.0,
        body.body_type,
        pos.0.length() as f32,
        0.001,
        &config,
    );
    let (spin_dir, spin_angle) = compute_planet_spin_frame(opt_spin, elapsed_years);
    let spin_rot = Quat::from_axis_angle(spin_dir.as_vec3(), spin_angle as f32);

    let world_up = (spin_rot * unspun_norm).normalize_or_zero();
    let world_heading = (spin_rot * heading_unspun).normalize_or_zero();
    let (east, up, north) = compute_surface_tangent_basis(world_up, world_heading);

    // Draw Holographic Surface Compass & Horizon Clinometer Ring on the planet's surface
    let effective_rad = vis_rad;
    let ground_center = planet_pos + up * (effective_rad * 1.002);
    let compass_r = vis_rad * 0.045;
    let ring_rot = Quat::from_rotation_arc(Vec3::Z, up);
    gizmos.circle(
        Isometry3d::new(ground_center, ring_rot),
        compass_r,
        Color::srgba(0.25, 0.88, 1.0, 0.55),
    );
    gizmos.circle(
        Isometry3d::new(ground_center, ring_rot),
        compass_r * 0.45,
        Color::srgba(0.25, 0.88, 1.0, 0.35),
    );
    // Forward heading arrow on ground compass
    let look_dir = (north * camera.surface_azimuth.cos() + east * camera.surface_azimuth.sin())
        .normalize_or_zero();
    gizmos.line(
        ground_center + look_dir * (compass_r * 0.35),
        ground_center + look_dir * (compass_r * 1.25),
        Color::srgba(0.35, 1.0, 0.75, 0.85),
    );
}
