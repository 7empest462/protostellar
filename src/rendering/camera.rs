//! 3D Orbital Camera with logarithmic zoom, focus-lock, and smooth interpolation.

use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use bevy::anti_alias::smaa::{Smaa, SmaaPreset};
use bevy::camera::{Exposure, Hdr};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::gizmos::config::GizmoLineJoint;
use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;

pub use crate::rendering::camera_picking::IgnoreUiPicking;
use crate::rendering::camera_picking::{
    UiInteractionStateQuery, UiPickingNodesQuery, handle_camera_target_picking,
    is_cursor_over_scroll_container, is_ui_interaction_blocking,
};
use crate::rendering::post_processing::PostProcessingSettings;

use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Orbital camera controller component with smooth cinematic inertial damping.
#[derive(Component, Debug, Clone)]
pub struct PanOrbitCamera {
    /// 3D focus point in simulation render coordinates (AU)
    pub focus: Vec3,
    pub target_focus: Vec3,
    /// Distance from focus point in AU
    pub radius: f32,
    pub target_radius: f32,
    /// Horizontal rotation (radians)
    pub yaw: f32,
    pub target_yaw: f32,
    /// Vertical pitch (radians)
    pub pitch: f32,
    pub target_pitch: f32,
    /// Target entity to track (if focus-locked)
    pub target_entity: Option<Entity>,
    /// Minimum zoom distance in AU (~10,000 km)
    pub min_radius: f32,
    /// Maximum zoom distance in AU (150 AU outer solar system)
    pub max_radius: f32,
    /// Sensitivity multipliers
    pub orbit_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub pan_sensitivity: f32,
    /// Smooth transition progress for interplanetary camera flight (0.0 to 1.0)
    pub focus_transition_progress: f32,
    /// Starting focus point for interplanetary camera flight
    pub focus_transition_start: Vec3,
    /// Starting orbital zoom radius for interplanetary camera flight
    pub radius_transition_start: f32,
    /// Starting camera eye offset `(eye_0 - target_focus)` for spherical-glide flight
    pub transition_start_offset: Vec3,
    /// Last rendered orbital camera eye position in world space (AU)
    pub last_orbital_eye: Option<Vec3>,
    /// Previous target entity to detect focus target changes
    pub previous_target_entity: Option<Entity>,
    /// Atmospheric entry descent transition progress for Surface Horizon Mode (0.0 to 1.0)
    pub entry_transition_progress: f32,
    /// Starting offset relative to planet center for atmospheric entry descent
    pub surface_entry_start_offset: Vec3,
    /// Starting rotation for atmospheric entry descent
    pub surface_entry_start_rot: Quat,
    /// Phase 9B: First-Person Rover / Planetary Surface Horizon Camera Mode
    pub surface_horizon_mode: bool,
    pub surface_azimuth: f32,
    pub surface_elevation: f32,
    pub surface_lat_lon: Vec2,
    pub surface_altitude_scale: f32,
}

impl Default for PanOrbitCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            target_focus: Vec3::ZERO,
            radius: 16.0, // 16 AU framing of inner and giant solar system
            target_radius: 16.0,
            yaw: 0.785, // 45 degrees
            target_yaw: 0.785,
            pitch: 0.62, // ~36 degrees inclination
            target_pitch: 0.62,
            target_entity: None,
            min_radius: 0.005,
            max_radius: 3_000_000.0,
            orbit_sensitivity: 0.005,
            zoom_sensitivity: 0.13,
            pan_sensitivity: 0.02,
            focus_transition_progress: 1.0,
            focus_transition_start: Vec3::ZERO,
            radius_transition_start: 16.0,
            transition_start_offset: Vec3::new(0.0, 0.0, 16.0),
            last_orbital_eye: None,
            previous_target_entity: None,
            entry_transition_progress: 1.0,
            surface_entry_start_offset: Vec3::ZERO,
            surface_entry_start_rot: Quat::IDENTITY,
            surface_horizon_mode: false,
            surface_azimuth: 0.0,
            surface_elevation: 0.12,
            surface_lat_lon: Vec2::new(0.32, 0.0),
            surface_altitude_scale: 0.022,
        }
    }
}

impl PanOrbitCamera {
    #[inline]
    pub fn orbital_rotation(&self) -> Quat {
        Quat::from_axis_angle(Vec3::Y, self.yaw) * Quat::from_axis_angle(Vec3::X, -self.pitch)
    }

    #[inline]
    pub fn current_orbital_eye(&self) -> Vec3 {
        self.last_orbital_eye.unwrap_or_else(|| {
            self.focus + self.orbital_rotation() * Vec3::new(0.0, 0.0, self.radius)
        })
    }

    /// Initiates a smooth, strictly monotonic spherical-glide transition to `entity` at
    /// `target_vec` with settled framing distance `target_radius`.
    pub fn initiate_target_transition(
        &mut self,
        entity: Entity,
        target_vec: Vec3,
        target_radius: f32,
    ) {
        let current_eye = self.current_orbital_eye();
        self.surface_horizon_mode = false;
        self.target_entity = Some(entity);
        self.previous_target_entity = Some(entity);
        self.target_radius = target_radius;
        if self.focus.distance(target_vec) > 0.05 {
            self.focus_transition_start = self.focus;
            self.radius_transition_start = self.radius.max(1e-5);
            self.transition_start_offset = current_eye - target_vec;
            self.focus_transition_progress = 0.0;
        } else {
            self.focus_transition_progress = 1.0;
            self.focus = target_vec;
        }
        self.target_focus = target_vec;
    }
}

/// Spawns the 3D camera.
pub fn setup_camera(mut commands: Commands) {
    let pan_orbit = PanOrbitCamera::default();

    let rot = Quat::from_axis_angle(Vec3::Y, pan_orbit.yaw)
        * Quat::from_axis_angle(Vec3::X, -pan_orbit.pitch);
    let translation = pan_orbit.focus + rot * Vec3::new(0.0, 0.0, pan_orbit.radius);

    // Single camera: 3D scene + UI overlay with HDR bloom and filmic tonemapping.
    //
    // In Bevy 0.19, the `Hdr` component activates floating-point render targets,
    // allowing fragment colours > 1.0 to accumulate without clamping. The bloom
    // prefilter threshold (1.8) is set above the SDR ceiling (1.0), so only genuinely
    // incandescent surfaces (stars, lava, aurorae, accretion disks) contribute to the
    // bloom halo. Threshold-softness (0.4) avoids a hard pop-in at the knee.
    // TonyMcMapface is a filmic display transform that preserves hue even at extreme
    // HDR values — critical for accurate star colours. DebandDither prevents gradient banding.
    commands
        .spawn((
            Camera3d::default(),
            Msaa::Off,
            Smaa {
                preset: SmaaPreset::High,
            },
            ContrastAdaptiveSharpening {
                enabled: true,
                sharpening_strength: 0.35,
                denoise: false,
            },
            Hdr,
            Exposure { ev100: 9.7 },
            Camera::default(),
            Projection::Perspective(PerspectiveProjection {
                near: 0.0001,
                far: 2_000_000.0,
                ..default()
            }),
            Tonemapping::AgX,
            DebandDither::Enabled,
            Transform {
                translation,
                rotation: rot,
                ..default()
            },
            pan_orbit,
            IsDefaultUiCamera,
        ))
        .insert((
            PostProcessingSettings::default(),
            Bloom {
                intensity: 0.16,
                low_frequency_boost: 0.12,
                low_frequency_boost_curvature: 0.88,
                high_pass_frequency: 1.0,
                prefilter: BloomPrefilter {
                    threshold: 1.65,
                    threshold_softness: 0.48,
                },
                composite_mode: BloomCompositeMode::EnergyConserving,
                ..default()
            },
        ));
    info!("✅ setup_camera: Spawned Camera3d with SMAA High, CAS Sharpening, HDR Bloom, AgX");
}

/// Configures Bevy's Gizmo rendering parameters:
/// - depth_bias: -1.0 so that orbit ribbons, conic tracks, targeting reticles, and AU rings
///   are never occluded or clipped by 3D volumetric gas planes, planet spheres, or depth buffer limits.
/// - line.width: 1.4 for fine, high-precision astronomical telemetry lines.
/// - line.joints: GizmoLineJoint::Round(4) for smooth rounded joins at polyline vertices.
pub fn setup_gizmo_configuration(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;
    config.line.width = 1.4;
    config.line.joints = GizmoLineJoint::Round(4);
    info!(
        "✅ setup_gizmo_configuration: depth_bias set to -1.0, line.width set to 1.4, line.joints set to Round(4)"
    );
}

#[inline]
fn is_little_red_dot(body: &CelestialBody) -> bool {
    body.body_type == BodyType::QuasiStar
        || crate::utils::math::contains_ascii_case_insensitive(&body.name, "little red dot")
}

pub(crate) type CameraTargetsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimPosition,
        &'static Radius,
        &'static CelestialBody,
        &'static Mass,
        Option<&'static Transform>,
        Option<&'static Composition>,
        Option<&'static InternalDifferentiation>,
    ),
    Without<PanOrbitCamera>,
>;

fn compute_system_min_orbit_r(targets_query: &CameraTargetsQuery) -> (Vec3, f32) {
    let mut star_pos = Vec3::ZERO;
    let mut star_mass = -1.0_f64;
    let mut star_count = 0_usize;
    for (_, pos, _, body, mass, _, _, _) in targets_query.iter() {
        if body.body_type.is_star_or_remnant() {
            star_count += 1;
            if mass.0 > star_mass {
                star_mass = mass.0;
                star_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            }
        }
    }
    if star_count != 1 {
        return (star_pos, 1.0);
    }
    let mut min_r = f32::MAX;
    for (_, pos, _, _, _, _, _, _) in targets_query.iter() {
        let p = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let dist = (p - star_pos).length();
        if dist > 0.001 && dist < min_r {
            min_r = dist;
        }
    }
    (star_pos, if min_r < f32::MAX { min_r } else { 1.0 })
}

#[allow(clippy::too_many_arguments, reason = "Camera radii helper")]
#[inline]
pub(crate) fn resolve_camera_body_radii(
    opt_t: Option<&Transform>,
    radius_au: f64,
    body_type: BodyType,
    mass_solar: f64,
    opt_comp: Option<&Composition>,
    opt_diff: Option<&InternalDifferentiation>,
    show_atmospheres: bool,
    r_orb: f32,
    min_orbit_r: f32,
    config: &SimulationConfig,
) -> (f32, f32) {
    if let Some(t) = opt_t {
        let sx = t.scale.x;
        let is_unsynced_default = (sx - 1.0).abs() < 1e-6 && radius_au < 0.25;
        if sx.is_finite() && sx > 1e-6 && !is_unsynced_default {
            let close_vis_r = crate::rendering::bodies::transforms::compute_view_visual_radius(
                radius_au,
                body_type,
                mass_solar,
                opt_comp,
                opt_diff,
                show_atmospheres,
                r_orb,
                min_orbit_r,
                0.0,
                config,
            )
            .min(sx);
            return (sx, close_vis_r);
        }
    }
    let static_r = config.calc_visual_radius_for_type(radius_au, body_type);
    (static_r, static_r)
}

fn update_camera_min_zoom_bounds(
    camera: &mut PanOrbitCamera,
    config: &SimulationConfig,
    targets_query: &CameraTargetsQuery,
    star_pos: Vec3,
    min_orbit_r: f32,
    show_atmospheres: bool,
) {
    let focused_info = if let Some(target_ent) = camera.target_entity {
        if let Ok((_, pos, radius, body, mass, opt_t, opt_comp, opt_diff)) =
            targets_query.get(target_ent)
        {
            let target_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            camera.target_focus = target_vec;
            let is_lrd = is_little_red_dot(body);
            let (curr_vis, close_vis) = resolve_camera_body_radii(
                opt_t,
                radius.0,
                body.body_type,
                mass.0,
                opt_comp,
                opt_diff,
                show_atmospheres,
                (target_vec - star_pos).length(),
                min_orbit_r,
                config,
            );
            Some((curr_vis, close_vis, is_lrd))
        } else {
            camera.target_entity = None;
            None
        }
    } else {
        None
    };

    let effective_info = if let Some(info) = focused_info {
        Some(info)
    } else {
        targets_query
            .iter()
            .map(|(_, pos, radius, body, mass, opt_t, opt_comp, opt_diff)| {
                let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
                let dist = camera.target_focus.distance(center);
                let (curr_vis, close_vis) = resolve_camera_body_radii(
                    opt_t,
                    radius.0,
                    body.body_type,
                    mass.0,
                    opt_comp,
                    opt_diff,
                    show_atmospheres,
                    (center - star_pos).length(),
                    min_orbit_r,
                    config,
                );
                let is_lrd = is_little_red_dot(body);
                (dist, curr_vis, close_vis, is_lrd)
            })
            .filter(|(dist, curr_vis, _, _)| *dist <= (*curr_vis * 2.5).max(0.35))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, curr_vis, close_vis, is_lrd)| (curr_vis, close_vis, is_lrd))
    };

    let target_min_radius = if let Some((_curr_vis_rad, close_vis_rad, is_lrd)) = effective_info {
        if is_lrd {
            camera.min_radius = 0.001;
            0.001
        } else {
            let min_r = config.calc_camera_min_zoom_radius(close_vis_rad);
            camera.min_radius = min_r;
            min_r
        }
    } else {
        camera.min_radius = 0.001;
        0.001
    };

    camera.target_radius = camera
        .target_radius
        .clamp(target_min_radius, camera.max_radius);
    camera.radius = camera.radius.clamp(camera.min_radius, camera.max_radius);
}

#[allow(clippy::too_many_arguments, reason = "Keyboard camera flight helper")]
fn handle_camera_keyboard_flight(
    keyboard_input: &ButtonInput<KeyCode>,
    transform: &Transform,
    camera: &mut PanOrbitCamera,
    player_state: &mut PlayerInteractionState,
    targets_query: &CameraTargetsQuery,
    config: &SimulationConfig,
    star_pos: Vec3,
    min_orbit_r: f32,
) {
    if camera.surface_horizon_mode {
        return;
    }

    if keyboard_input.just_pressed(KeyCode::KeyF) {
        if let Some(target) = player_state.selected_entity {
            if let Ok((_, pos, radius, body, mass, _opt_t, opt_comp, opt_diff)) =
                targets_query.get(target)
            {
                let target_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
                let frame_radius = if body.name.starts_with("🚀") || radius.0 <= 1e-6 {
                    let visual_radius =
                        config.calc_visual_radius_for_type(radius.0, body.body_type);
                    0.015f32.max(config.calc_camera_framing_radius(visual_radius))
                } else {
                    crate::rendering::bodies::transforms::compute_close_view_camera_framing_radius(
                        radius.0,
                        body.body_type,
                        mass.0,
                        opt_comp,
                        opt_diff,
                        player_state.show_atmospheres(),
                        (target_vec - star_pos).length(),
                        min_orbit_r,
                        config,
                    )
                };
                camera.initiate_target_transition(target, target_vec, frame_radius);
            } else {
                camera.target_entity = Some(target);
            }
        } else {
            camera.target_focus = Vec3::ZERO;
            camera.target_entity = None;
        }
    }

    let is_shift =
        keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight);
    if !is_shift && keyboard_input.just_pressed(KeyCode::KeyR) {
        camera.target_focus = Vec3::ZERO;
        camera.target_entity = None;
        player_state.selected_entity = None;
    }

    if !is_shift {
        let move_speed = (camera.target_radius * 0.025).clamp(0.1, 5.0);
        if keyboard_input.pressed(KeyCode::KeyW) {
            camera.target_focus += transform.rotation * -Vec3::Z * move_speed;
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyS) {
            camera.target_focus += transform.rotation * Vec3::Z * move_speed;
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            camera.target_focus += transform.rotation * -Vec3::X * move_speed;
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            camera.target_focus += transform.rotation * Vec3::X * move_speed;
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyQ) {
            camera.target_focus.y -= move_speed;
            camera.target_entity = None;
        }
        if keyboard_input.pressed(KeyCode::KeyE) {
            camera.target_focus.y += move_speed;
            camera.target_entity = None;
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Mouse orbit, zoom, pinch, and pan controls"
)]
fn handle_camera_mouse_controls(
    mouse_buttons: &ButtonInput<MouseButton>,
    keyboard_input: &ButtonInput<KeyCode>,
    mouse_motion_events: &mut MessageReader<MouseMotion>,
    mouse_wheel_events: &mut MessageReader<MouseWheel>,
    pinch_events: Option<&Messages<PinchGesture>>,
    pinch_cursor: &mut bevy::ecs::message::MessageCursor<PinchGesture>,
    camera: &mut PanOrbitCamera,
    transform: &Transform,
    cursor_over_ui: bool,
    dt: f32,
) {
    let mut delta_yaw = 0.0;
    let mut delta_pitch = 0.0;
    let mut delta_pan = Vec2::ZERO;

    if mouse_buttons.pressed(MouseButton::Right) && !keyboard_input.pressed(KeyCode::ShiftLeft) {
        for ev in mouse_motion_events.read() {
            delta_yaw -= ev.delta.x * camera.orbit_sensitivity;
            delta_pitch -= ev.delta.y * camera.orbit_sensitivity;
        }
    } else if mouse_buttons.pressed(MouseButton::Middle)
        || (mouse_buttons.pressed(MouseButton::Right) && keyboard_input.pressed(KeyCode::ShiftLeft))
    {
        for ev in mouse_motion_events.read() {
            delta_pan.x -= ev.delta.x * camera.pan_sensitivity * (camera.target_radius / 30.0);
            delta_pan.y += ev.delta.y * camera.pan_sensitivity * (camera.target_radius / 30.0);
        }
        camera.target_entity = None;
    } else {
        mouse_motion_events.clear();
    }

    let mut scroll = 0.0;
    for ev in mouse_wheel_events.read() {
        if cursor_over_ui {
            continue;
        }
        let delta = match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 14.0,
        };
        scroll += delta;
    }

    if let Some(pinches) = pinch_events {
        for ev in pinch_cursor.read(pinches) {
            if !cursor_over_ui {
                scroll += ev.0 * 16.0;
            }
        }
    }

    // Keyboard zoom shortcuts (+ / -), normalized when real wall dt is available
    let key_step = if dt >= 0.002 {
        (dt.clamp(0.002, 0.25) * 60.0) * 0.6
    } else {
        0.6
    };
    if keyboard_input.pressed(KeyCode::Equal) || keyboard_input.pressed(KeyCode::NumpadAdd) {
        scroll += key_step;
    }
    if keyboard_input.pressed(KeyCode::Minus) || keyboard_input.pressed(KeyCode::NumpadSubtract) {
        scroll -= key_step;
    }

    if scroll.abs() > 0.0 {
        let sensitivity = if keyboard_input.pressed(KeyCode::ShiftLeft)
            || keyboard_input.pressed(KeyCode::ShiftRight)
        {
            camera.zoom_sensitivity * 0.35 // Micro-zoom precision mode
        } else if keyboard_input.pressed(KeyCode::ControlLeft)
            || keyboard_input.pressed(KeyCode::ControlRight)
        {
            camera.zoom_sensitivity * 2.2 // Rapid macro-zoom mode
        } else {
            camera.zoom_sensitivity
        };

        let zoom_factor = (-scroll * sensitivity).exp();
        camera.target_radius =
            (camera.target_radius * zoom_factor).clamp(camera.min_radius, camera.max_radius);
    }

    camera.target_yaw += delta_yaw;
    camera.target_pitch = (camera.target_pitch + delta_pitch).clamp(-1.54, 1.54);

    if camera.target_entity.is_none() && delta_pan != Vec2::ZERO {
        let right = transform.rotation * Vec3::X;
        let up = transform.rotation * Vec3::Y;
        camera.target_focus += right * delta_pan.x + up * delta_pan.y;
    }
}

/// Smoothly adapts HDR Bloom intensity and prefilter knee based on proximity to luminous stars
/// so close-up photosphere inspection preserves convective granulation contrast without white-out.
fn adapt_camera_bloom_to_star_proximity(
    opt_bloom: Option<Mut<Bloom>>,
    cam_pos: Vec3,
    targets_query: &CameraTargetsQuery,
) {
    let Some(mut bloom) = opt_bloom else {
        return;
    };

    let mut min_star_ratio = f32::MAX;
    for (_, pos, rad, body, _, opt_trans, _, _) in targets_query.iter() {
        if body.body_type.is_star_or_remnant() || body.body_type == BodyType::QuasiStar {
            if body.body_type == BodyType::BlackHole {
                continue;
            }
            let star_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            let vis_r = opt_trans.map_or(rad.0 as f32, |t| t.scale.x).max(0.0005);
            let ratio = cam_pos.distance(star_pos) / vis_r;
            if ratio < min_star_ratio {
                min_star_ratio = ratio;
            }
        }
    }

    // t = 0 when right at stellar photosphere (ratio <= 3.0), t = 1 at orbital distances (ratio >= 18.0)
    let t = ((min_star_ratio - 3.0) / 15.0).clamp(0.0, 1.0);
    let target_intensity = 0.095 + 0.070 * t;
    let target_threshold = 2.30 - 0.58 * t;
    let target_lf_boost = 0.06 + 0.06 * t;

    bloom.intensity += (target_intensity - bloom.intensity) * 0.15;
    bloom.prefilter.threshold += (target_threshold - bloom.prefilter.threshold) * 0.15;
    bloom.low_frequency_boost += (target_lf_boost - bloom.low_frequency_boost) * 0.15;
}

/// Singularity-free unit-sphere great-circle slerp from `d0` to `d1`.
/// When `d0` and `d1` are nearly antipodal (`dot ~ -1.0`), pivots smoothly over `fallback_up`.
#[inline]
fn slerp_unit_directions(d0: Vec3, d1: Vec3, fallback_up: Vec3, sigma: f32) -> Vec3 {
    if sigma <= 0.0 {
        return d0;
    }
    if sigma >= 1.0 {
        return d1;
    }
    let dot = d0.dot(d1).clamp(-1.0, 1.0);
    if dot > 0.9995 {
        return d0.lerp(d1, sigma).normalize_or(d1);
    }
    let theta = dot.acos();
    let perp_raw = d0 - d1 * dot;
    let perp_len_sq = perp_raw.length_squared();
    let e_perp = if perp_len_sq > 1e-6 {
        perp_raw / perp_len_sq.sqrt()
    } else {
        (fallback_up - d1 * fallback_up.dot(d1)).normalize_or(Vec3::Y)
    };
    let angle = (1.0 - sigma) * theta;
    d1 * angle.cos() + e_perp * angle.sin()
}

#[inline]
fn smooth_lerp_factor(rate_at_60hz: f32, dt: f32) -> f32 {
    if dt < 0.002 {
        // Keep fixed-step convergence for MinimalPlugins headless tests with sub-ms frame times.
        return rate_at_60hz;
    }
    (1.0 - (1.0 - rate_at_60hz).powf(dt.clamp(0.002, 0.25) * 60.0)).clamp(0.0, 1.0)
}

#[inline]
fn interpolate_log_radius(current: f32, target: f32, alpha: f32, min_r: f32, max_r: f32) -> f32 {
    let cur = current.clamp(min_r, max_r).max(1e-6);
    let tgt = target.clamp(min_r, max_r).max(1e-6);
    if (tgt - cur).abs() <= 1e-6 {
        return tgt;
    }
    let log_r = cur.ln() + (tgt.ln() - cur.ln()) * alpha;
    log_r.exp().clamp(min_r, max_r)
}

fn integrate_orbital_camera_pose(camera: &mut PanOrbitCamera, dt: f32) -> (Vec3, Quat) {
    let rot_alpha = smooth_lerp_factor(0.24, dt);
    let zoom_alpha = smooth_lerp_factor(0.36, dt);
    let focus_alpha = smooth_lerp_factor(0.18, dt);

    camera.yaw += (camera.target_yaw - camera.yaw) * rot_alpha;
    camera.pitch += (camera.target_pitch - camera.pitch) * rot_alpha;
    let rot = camera.orbital_rotation();
    let back_dir = rot * Vec3::Z;

    if camera.target_entity.is_some() {
        if dt <= 0.0 {
            camera.focus_transition_progress = 1.0;
            camera.focus = camera.target_focus;
            camera.radius = interpolate_log_radius(
                camera.radius,
                camera.target_radius,
                zoom_alpha,
                camera.min_radius,
                camera.max_radius,
            );
        } else if camera.focus_transition_progress < 1.0 {
            camera.focus_transition_progress =
                (camera.focus_transition_progress + dt * 2.2).min(1.0);
            let t = camera.focus_transition_progress;
            let u = t * t * (3.0 - 2.0 * t);
            let inv_u = 1.0 - u;
            let smooth_t = 1.0 - inv_u * inv_u * inv_u;

            camera.focus = camera
                .focus_transition_start
                .lerp(camera.target_focus, smooth_t);
            let d_end = camera
                .target_radius
                .clamp(camera.min_radius, camera.max_radius);
            camera.radius = (camera.radius_transition_start
                + (d_end - camera.radius_transition_start) * smooth_t)
                .clamp(camera.min_radius, camera.max_radius);

            if camera.focus_transition_progress >= 1.0 {
                camera.focus = camera.target_focus;
                camera.radius = d_end;
            } else {
                let start_off = camera.transition_start_offset;
                let d_start = start_off.length().max(1e-5);
                let d0 = if start_off.length_squared() > 1e-10 {
                    start_off / d_start
                } else {
                    back_dir
                };
                let t_ang = (t / 0.65).min(1.0);
                let sigma = t_ang * t_ang * (3.0 - 2.0 * t_ang);
                let dir_t = slerp_unit_directions(d0, back_dir, rot * Vec3::Y, sigma);
                let dist_t = (d_start + (d_end - d_start) * smooth_t)
                    .clamp(camera.min_radius, camera.max_radius);
                let translation = camera.target_focus + dir_t * dist_t;
                camera.last_orbital_eye = Some(translation);
                return (translation, rot);
            }
        } else {
            camera.focus = camera.target_focus;
            camera.radius = interpolate_log_radius(
                camera.radius,
                camera.target_radius,
                zoom_alpha,
                camera.min_radius,
                camera.max_radius,
            );
        }
    } else {
        camera.focus = camera.focus.lerp(camera.target_focus, focus_alpha);
        camera.radius = interpolate_log_radius(
            camera.radius,
            camera.target_radius,
            zoom_alpha,
            camera.min_radius,
            camera.max_radius,
        );
    }

    let translation = camera.focus + back_dir * camera.radius;
    camera.last_orbital_eye = Some(translation);
    (translation, rot)
}

fn sync_camera_target_transition_state(camera: &mut PanOrbitCamera) {
    if camera.target_entity == camera.previous_target_entity {
        return;
    }
    if let Some(ent) = camera.target_entity {
        let target_vec = camera.target_focus;
        let target_rad = camera.target_radius;
        camera.initiate_target_transition(ent, target_vec, target_rad);
    } else {
        if camera.focus_transition_progress < 1.0 {
            let eye = camera.current_orbital_eye();
            let back = camera.orbital_rotation() * Vec3::Z;
            camera.focus = eye - back * camera.radius;
            camera.target_focus = camera.focus;
        }
        camera.focus_transition_progress = 1.0;
        camera.previous_target_entity = None;
    }
}

/// Handles user mouse, keyboard camera control, and 3D raycast click selection of celestial bodies.
#[allow(
    deprecated,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Camera and target picking queries"
)]
pub fn update_pan_orbit_camera(
    windows: Query<&Window>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    config: Res<SimulationConfig>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mut player_state: ResMut<PlayerInteractionState>,
    targets_query: CameraTargetsQuery,
    mut camera_query: Query<
        (
            &Camera,
            &mut PanOrbitCamera,
            &mut Transform,
            &GlobalTransform,
            Option<&mut Bloom>,
        ),
        With<PanOrbitCamera>,
    >,
    ui_interaction_query: UiInteractionStateQuery,
    ui_nodes_query: UiPickingNodesQuery,
    time: Option<Res<Time>>,
    opt_visibility: Option<Res<crate::game::ui::types::HudVisibilityState>>,
    (focus, editable_query): (
        Option<Res<bevy::input_focus::InputFocus>>,
        Query<(), With<bevy::text::EditableText>>,
    ),
    (pinch_events, mut pinch_cursor): (
        Option<Res<Messages<PinchGesture>>>,
        Local<bevy::ecs::message::MessageCursor<PinchGesture>>,
    ),
) {
    let Ok((camera_comp, mut camera, mut transform, global_transform, opt_bloom)) =
        camera_query.single_mut()
    else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    let text_focused =
        crate::game::ui::widgets::is_text_input_focused(focus.as_deref(), &editable_query);

    let (star_pos, min_orbit_r) = compute_system_min_orbit_r(&targets_query);
    update_camera_min_zoom_bounds(
        &mut camera,
        &config,
        &targets_query,
        star_pos,
        min_orbit_r,
        player_state.show_atmospheres(),
    );

    let dt = time.as_ref().map_or(0.016, |t| t.delta_secs());

    if camera.surface_horizon_mode {
        camera.previous_target_entity = camera.target_entity;
        camera.last_orbital_eye = Some(transform.translation);
        player_state.just_selected_via_ui = false;
        if !text_focused
            && (keyboard_input.just_pressed(KeyCode::KeyF)
                || keyboard_input.just_pressed(KeyCode::KeyR)
                || keyboard_input.just_pressed(KeyCode::Escape))
        {
            camera.surface_horizon_mode = false;
            camera.entry_transition_progress = 1.0;
            camera.last_orbital_eye = None;
            return;
        }
        adapt_camera_bloom_to_star_proximity(opt_bloom, transform.translation, &targets_query);
        return;
    }

    sync_camera_target_transition_state(&mut camera);

    let cursor_over_ui = ui_interaction_query
        .iter()
        .any(|(h, p, n, s, c)| is_ui_interaction_blocking(h, p, n, s, c))
        || is_cursor_over_scroll_container(window.cursor_position(), &ui_nodes_query);

    handle_camera_target_picking(
        window,
        camera_comp,
        global_transform,
        &targets_query,
        &mut player_state,
        &mut camera,
        &config,
        &ui_interaction_query,
        &ui_nodes_query,
        &mouse_buttons,
        star_pos,
        min_orbit_r,
        opt_visibility.as_deref(),
    );

    if !text_focused {
        handle_camera_keyboard_flight(
            &keyboard_input,
            &transform,
            &mut camera,
            &mut player_state,
            &targets_query,
            &config,
            star_pos,
            min_orbit_r,
        );
    }

    handle_camera_mouse_controls(
        &mouse_buttons,
        &keyboard_input,
        &mut mouse_motion_events,
        &mut mouse_wheel_events,
        pinch_events.as_deref(),
        &mut pinch_cursor,
        &mut camera,
        &transform,
        cursor_over_ui,
        dt,
    );

    let (translation, rot) = integrate_orbital_camera_pose(&mut camera, dt);
    transform.translation = translation;
    transform.rotation = rot;
    adapt_camera_bloom_to_star_proximity(opt_bloom, translation, &targets_query);
}
