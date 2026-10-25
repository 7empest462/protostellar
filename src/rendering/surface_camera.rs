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
    BiosphereState, BodyType, CelestialBody, Composition, FloraPigmentClass, Mass, Radius,
    SimPosition, Temperature,
};
use crate::simulation::probes::navigation::compute_planet_spin_frame;
use crate::simulation::probes::{FlightComputer, ProbeStatus, ProbeType, SpaceProbe};
use crate::simulation::resources::{PlayerInteractionState, SimTime, SimulationConfig};
use crate::utils::constants::{AU_TO_KM, EARTH_MASS_SOLAR, EARTH_RADIUS_AU};

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
        toast.message =
            "🎥 SURFACE HORIZON CAM ACTIVE [Shift-V] // WASD: Drive | R-Drag: Look 360° | Scroll: Altitude | Esc/F: Orbital View"
                .to_string();
        toast.timer = 5.0;
    } else {
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

fn pigment_colors(pigment: FloraPigmentClass) -> (Color, Color) {
    match pigment {
        FloraPigmentClass::SolarChlorophyll => (
            Color::srgba(0.18, 0.92, 0.42, 0.90),
            Color::srgba(0.45, 1.0, 0.68, 0.75),
        ),
        FloraPigmentClass::RedDwarfCrimson => (
            Color::srgba(0.95, 0.16, 0.38, 0.92),
            Color::srgba(1.0, 0.48, 0.65, 0.78),
        ),
        FloraPigmentClass::OrangeDwarfCarotenoid => (
            Color::srgba(1.0, 0.68, 0.16, 0.92),
            Color::srgba(1.0, 0.88, 0.38, 0.78),
        ),
        FloraPigmentClass::HighUvIndigo => (
            Color::srgba(0.22, 0.85, 1.0, 0.92),
            Color::srgba(0.62, 0.42, 1.0, 0.82),
        ),
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
    unspun_norm: Vec3,
    comp: &Composition,
    temp_k: f32,
    body_type: BodyType,
    opt_bio: Option<&BiosphereState>,
    vis_rad: f32,
) -> f32 {
    let terrain_h = super::surface_terrain::evaluate_surface_elevation(
        unspun_norm,
        comp,
        temp_k,
        body_type,
        vis_rad,
    );
    let sea_h = if super::surface_terrain::planet_has_surface_liquids(
        body_type, comp, temp_k, opt_bio, None,
    ) {
        super::surface_terrain::evaluate_sea_level_offset(body_type, temp_k, comp, vis_rad)
    } else {
        0.0
    };
    vis_rad + terrain_h.max(sea_h)
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
) {
    let Ok((mut camera, mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    let is_shift =
        keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight);
    if is_shift && keyboard_input.just_pressed(KeyCode::KeyV) {
        toggle_surface_horizon_mode(&mut camera, player_state.selected_entity, &mut toast);
    }

    if !camera.surface_horizon_mode {
        if let Projection::Perspective(ref mut p) = *projection {
            if (p.near - 0.0001).abs() > 1e-6 {
                p.near = 0.0001;
            }
        }
        return;
    }

    let (delta_yaw, delta_pitch, scroll) = read_surface_mouse_and_scroll_deltas(
        &mouse_buttons,
        &mut mouse_motion_events,
        &mut mouse_wheel_events,
        camera.orbit_sensitivity,
    );

    let (forward_drive, turn_drive) =
        step_surface_horizon_controls(&keyboard_input, delta_yaw, delta_pitch, scroll, &mut camera);

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
    let vis_rad = opt_t.map_or_else(
        || config.calc_visual_radius_for_type(radius.0, body.body_type),
        |t| t.scale.x,
    );

    if let Some(rover_ent) = opt_rover_ent {
        if let Ok((_, mut probe, _, mut probe_pos)) = probes_query.get_mut(rover_ent) {
            // Undo duplicate azimuth turn because apply_manual_rover_drive rotates rover_heading_unspun
            camera.surface_azimuth -= turn_drive;
            apply_manual_rover_drive(&mut probe, forward_drive, turn_drive, radius.0);
            let unspun_d = probe
                .landed_unspun_pos
                .unwrap_or(DVec3::Y)
                .normalize_or_zero();
            let h_d = probe
                .rover_heading_unspun
                .unwrap_or(DVec3::Z)
                .normalize_or_zero();
            unspun_norm = unspun_d.as_vec3();
            heading_unspun = h_d.as_vec3();
            let rover_h = super::surface_terrain::evaluate_surface_elevation(
                unspun_norm,
                comp,
                temp.0 as f32,
                body.body_type,
                vis_rad,
            );
            let spun_d = spin_rot_d * unspun_d;
            probe_pos.0 = pos.0 + spun_d * f64::from((vis_rad + rover_h) * 1.002);
        }
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

    transform.translation = eye_pos;
    transform.rotation = eye_rot;
    camera.focus = eye_pos;
    camera.target_focus = planet_pos;

    if let Projection::Perspective(ref mut p) = *projection {
        p.near = 0.00001;
    }
}

/// Renders 3D procedural surface horizon features (alien flora canopies adapted to stellar
/// spectrum and surface gravity, civilization biodomes, volcanic vents, ice penitentes, and
/// a holographic rover compass ring) when the camera is in Surface Horizon Mode.
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
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);

    let Some((host_ent, _, unspun_norm, heading_unspun)) = resolve_surface_vantage_point(
        camera.target_entity,
        camera.surface_lat_lon,
        &worlds_query,
        &probes_query,
    ) else {
        return;
    };

    let Ok((_, pos, radius, mass, temp, comp, body, opt_spin, opt_bio, opt_t)) =
        worlds_query.get(host_ent)
    else {
        return;
    };

    let planet_pos = pos.0.as_vec3();
    let vis_rad = opt_t.map_or_else(
        || config.calc_visual_radius_for_type(radius.0, body.body_type),
        |t| t.scale.x,
    );
    let (spin_dir, spin_angle) = compute_planet_spin_frame(opt_spin, elapsed_years);
    let spin_rot = Quat::from_axis_angle(spin_dir.as_vec3(), spin_angle as f32);

    let world_up = (spin_rot * unspun_norm).normalize_or_zero();
    let world_heading = (spin_rot * heading_unspun).normalize_or_zero();
    let (east, up, north) = compute_surface_tangent_basis(world_up, world_heading);

    // 1. Draw Holographic Surface Compass & Horizon Clinometer Ring
    let terrain_h = super::surface_terrain::evaluate_surface_elevation(
        unspun_norm,
        comp,
        temp.0 as f32,
        body.body_type,
        vis_rad,
    );
    let effective_rad = vis_rad + terrain_h;
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

    // 2. Render 3D Procedural Flora, Biodomes, or Geological Outcrops on the local spherical patch
    let m_earth = (mass.0 / EARTH_MASS_SOLAR).max(0.01);
    let r_earth = (radius.0 / EARTH_RADIUS_AU).max(0.1);
    let surface_g = (m_earth / (r_earth * r_earth)).clamp(0.15, 3.5) as f32;

    draw_local_surface_features(
        &mut gizmos,
        planet_pos,
        vis_rad,
        body.body_type,
        spin_rot,
        unspun_norm,
        surface_g,
        temp.0,
        comp,
        opt_bio,
        visual_time,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Renders civilization biodomes, bioluminescent spores, and thermal plumes on the 3D terrain"
)]
fn draw_local_surface_features(
    gizmos: &mut Gizmos,
    planet_pos: Vec3,
    vis_rad: f32,
    body_type: crate::simulation::components::BodyType,
    spin_rot: Quat,
    unspun_center: Vec3,
    surface_g: f32,
    temp_k: f64,
    comp: &Composition,
    opt_bio: Option<&BiosphereState>,
    visual_time: f32,
) {
    let (u_east, u_up, u_north) = compute_surface_tangent_basis(unspun_center, Vec3::Y);
    let active_bio =
        opt_bio.filter(|b| b.biomass_coverage_frac > 0.01 || b.habitability_score > 0.10);
    let technosig = opt_bio.map_or(0.0, |b| b.technosignature);

    for idx in 0..24u32 {
        let f_idx = idx as f32;
        let r_angle = 0.035 + 0.18 * ((f_idx + 0.5) / 24.0).sqrt();
        let theta = f_idx * 2.399_963_1;
        let (sin_t, cos_t) = theta.sin_cos();

        let tangent_dir = (u_north * cos_t + u_east * sin_t).normalize_or_zero();
        let site_unspun = (u_up * r_angle.cos() + tangent_dir * r_angle.sin()).normalize_or_zero();
        let site_norm = (spin_rot * site_unspun).normalize_or_zero();
        let site_h = super::surface_terrain::evaluate_surface_elevation(
            site_unspun,
            comp,
            temp_k as f32,
            body_type,
            vis_rad,
        );
        let base_pos = planet_pos + site_norm * (vis_rad + site_h);
        let site_rot = Quat::from_rotation_arc(Vec3::Z, site_norm);
        let hash = ((f_idx * 127.1).sin() * 43758.547).fract().abs();

        if let Some(bio) = active_bio {
            if bio.bioluminescence_intensity > 0.05 && idx.is_multiple_of(3) {
                let (_, canopy_col) = pigment_colors(bio.flora_pigment);
                let spore_h = vis_rad * (0.010 + 0.008 * hash) / surface_g.sqrt();
                gizmos.circle(
                    Isometry3d::new(base_pos + site_norm * spore_h, site_rot),
                    vis_rad * 0.0045,
                    canopy_col,
                );
            }

            if technosig > 0.20 && idx.is_multiple_of(4) {
                let spire_h = vis_rad * (0.028 + 0.016 * technosig);
                let spire_top = base_pos + site_norm * spire_h;
                let civ_col = Color::srgba(0.35, 0.92, 1.0, 0.90);
                gizmos.line(base_pos, spire_top, civ_col);
                gizmos.sphere(
                    Isometry3d::from_translation(base_pos),
                    vis_rad * 0.012,
                    Color::srgba(0.25, 0.82, 1.0, 0.55),
                );
            }
        } else if temp_k >= 500.0 && idx.is_multiple_of(2) {
            let vent_h =
                vis_rad * (0.012 + 0.012 * ((visual_time * 3.0 + f_idx).sin() * 0.5 + 0.5));
            gizmos.line(
                base_pos,
                base_pos + site_norm * vent_h,
                Color::srgba(1.0, 0.38, 0.08, 0.85),
            );
        }
    }
}
