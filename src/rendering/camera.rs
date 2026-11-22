//! 3D Orbital Camera with logarithmic zoom, focus-lock, and smooth interpolation.

use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use bevy::anti_alias::smaa::{Smaa, SmaaPreset};
use bevy::camera::{Exposure, Hdr};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::gizmos::config::GizmoLineJoint;
use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::math::DVec3;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;

use crate::rendering::camera_controls::{
    handle_camera_keyboard_flight, handle_camera_mouse_controls,
};
pub use crate::rendering::camera_picking::IgnoreUiPicking;
use crate::rendering::camera_picking::{
    UiInteractionStateQuery, UiPickingNodesQuery, handle_camera_target_picking,
    is_cursor_over_scroll_container, is_ui_interaction_blocking,
};
use crate::rendering::origin::RenderOrigin;
use crate::rendering::post_processing::PostProcessingSettings;

use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Orbital camera controller component with smooth cinematic inertial damping.
#[derive(Component, Debug, Clone)]
pub struct PanOrbitCamera {
    /// 3D focus point in simulation coordinates (AU, `f32` mirror of `focus_au`)
    pub focus: Vec3,
    pub target_focus: Vec3,
    /// Double-precision 3D focus point in simulation coordinates (AU)
    pub focus_au: DVec3,
    pub target_focus_au: DVec3,
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
    /// Minimum zoom distance in AU
    pub min_radius: f32,
    /// Maximum zoom distance in AU
    pub max_radius: f32,
    /// Sensitivity multipliers
    pub orbit_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub pan_sensitivity: f32,
    /// Smooth transition progress for interplanetary camera flight (0.0 to 1.0)
    pub focus_transition_progress: f32,
    /// Starting focus point for interplanetary camera flight
    pub focus_transition_start: Vec3,
    /// Double-precision starting focus point for interplanetary flight (AU)
    pub focus_transition_start_au: DVec3,
    /// Starting orbital zoom radius for interplanetary camera flight
    pub radius_transition_start: f32,
    /// Starting camera eye offset `(eye_0 - target_focus)` for spherical-glide flight
    pub transition_start_offset: Vec3,
    /// Double-precision starting camera eye offset `(eye_0 - target_focus_au)`
    pub transition_start_offset_au: DVec3,
    /// Last rendered orbital camera eye position in simulation space (AU)
    pub last_orbital_eye: Option<Vec3>,
    /// Last rendered orbital camera eye position in double precision (AU)
    pub last_orbital_eye_au: Option<DVec3>,
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
    #[doc(hidden)]
    pub synced_focus_f32: Vec3,
    #[doc(hidden)]
    pub synced_target_focus_f32: Vec3,
}

impl Default for PanOrbitCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            target_focus: Vec3::ZERO,
            focus_au: DVec3::ZERO,
            target_focus_au: DVec3::ZERO,
            radius: 16.0, // 16 AU framing of inner and giant solar system
            target_radius: 16.0,
            yaw: 0.785, // 45 degrees
            target_yaw: 0.785,
            pitch: 0.62, // ~36 degrees inclination
            target_pitch: 0.62,
            target_entity: None,
            min_radius: 1e-7,
            max_radius: 3_000_000.0,
            orbit_sensitivity: 0.005,
            zoom_sensitivity: 0.13,
            pan_sensitivity: 0.02,
            focus_transition_progress: 1.0,
            focus_transition_start: Vec3::ZERO,
            focus_transition_start_au: DVec3::ZERO,
            radius_transition_start: 16.0,
            transition_start_offset: Vec3::new(0.0, 0.0, 16.0),
            transition_start_offset_au: DVec3::new(0.0, 0.0, 16.0),
            last_orbital_eye: None,
            last_orbital_eye_au: None,
            previous_target_entity: None,
            entry_transition_progress: 1.0,
            surface_entry_start_offset: Vec3::ZERO,
            surface_entry_start_rot: Quat::IDENTITY,
            surface_horizon_mode: false,
            surface_azimuth: 0.0,
            surface_elevation: 0.12,
            surface_lat_lon: Vec2::new(0.32, 0.0),
            surface_altitude_scale: 0.022,
            synced_focus_f32: Vec3::ZERO,
            synced_target_focus_f32: Vec3::ZERO,
        }
    }
}

impl PanOrbitCamera {
    /// Propagates any external `f32` writes to `focus` / `target_focus` into `f64` mirrors.
    #[inline]
    pub fn sync_f64_from_f32_edits(&mut self) {
        if self.focus != self.synced_focus_f32 {
            self.focus_au = self.focus.as_dvec3();
            self.synced_focus_f32 = self.focus;
        }
        if self.target_focus != self.synced_target_focus_f32 {
            self.target_focus_au = self.target_focus.as_dvec3();
            self.synced_target_focus_f32 = self.target_focus;
        }
    }

    #[inline]
    fn assign_synced_au(au: &mut DVec3, f32_val: &mut Vec3, synced_f32: &mut Vec3, val: DVec3) {
        *au = val;
        let v3 = val.as_vec3();
        *f32_val = v3;
        *synced_f32 = v3;
    }

    #[inline]
    pub fn set_focus_au(&mut self, val: DVec3) {
        Self::assign_synced_au(
            &mut self.focus_au,
            &mut self.focus,
            &mut self.synced_focus_f32,
            val,
        );
    }

    #[inline]
    pub fn set_target_focus_au(&mut self, val: DVec3) {
        Self::assign_synced_au(
            &mut self.target_focus_au,
            &mut self.target_focus,
            &mut self.synced_target_focus_f32,
            val,
        );
    }

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

    #[inline]
    pub fn current_orbital_eye_au(&self) -> DVec3 {
        self.last_orbital_eye_au
            .unwrap_or_else(|| self.sync_view_eye_fallback_au())
    }

    #[inline]
    fn sync_view_eye_fallback_au(&self) -> DVec3 {
        let base_focus = if self.focus == self.synced_focus_f32 {
            self.focus_au
        } else {
            self.focus.as_dvec3()
        };
        base_focus + (self.orbital_rotation() * Vec3::new(0.0, 0.0, self.radius)).as_dvec3()
    }

    /// Initiates a smooth, strictly monotonic spherical-glide transition in double precision
    /// to `entity` at `target_au` with settled framing distance `target_radius`.
    pub fn initiate_target_transition_au(
        &mut self,
        entity: Entity,
        target_au: DVec3,
        target_radius: f32,
    ) {
        self.sync_f64_from_f32_edits();
        let current_eye_au = self.current_orbital_eye_au();
        self.surface_horizon_mode = false;
        self.target_entity = Some(entity);
        self.previous_target_entity = Some(entity);
        self.target_radius = target_radius;
        let flight_thresh = f64::from(target_radius * 0.5).clamp(1e-6, 0.05);
        if self.focus_au.distance(target_au) > flight_thresh {
            self.focus_transition_start_au = self.focus_au;
            self.focus_transition_start = self.focus_transition_start_au.as_vec3();
            self.radius_transition_start = self.radius.max(1e-9);
            self.transition_start_offset_au = current_eye_au - target_au;
            self.transition_start_offset = self.transition_start_offset_au.as_vec3();
            self.focus_transition_progress = 0.0;
        } else {
            self.focus_transition_progress = 1.0;
            self.set_focus_au(target_au);
        }
        self.set_target_focus_au(target_au);
    }

    /// Initiates a smooth, strictly monotonic spherical-glide transition to `entity` at
    /// `target_vec` with settled framing distance `target_radius`.
    pub fn initiate_target_transition(
        &mut self,
        entity: Entity,
        target_vec: Vec3,
        target_radius: f32,
    ) {
        self.initiate_target_transition_au(entity, target_vec.as_dvec3(), target_radius);
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
        if sx.is_finite() && sx > 1e-10 && !is_unsynced_default {
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
            camera.set_target_focus_au(pos.0);
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
                let dist = camera.target_focus_au.distance(pos.0) as f32;
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
        camera.min_radius = 1e-7;
        1e-7
    };

    camera.target_radius = camera
        .target_radius
        .clamp(target_min_radius, camera.max_radius);
    camera.radius = camera.radius.clamp(camera.min_radius, camera.max_radius);
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
    let cur = current.clamp(min_r, max_r).max(1e-10);
    let tgt = target.clamp(min_r, max_r).max(1e-10);
    let tol = (tgt * 1e-5).max(1e-10);
    if (tgt - cur).abs() <= tol {
        return tgt;
    }
    let log_r = cur.ln() + (tgt.ln() - cur.ln()) * alpha;
    log_r.exp().clamp(min_r, max_r)
}

#[inline]
fn interpolate_log_scalar(a: f32, b: f32, alpha: f32) -> f32 {
    let sa = a.max(1e-10);
    let sb = b.max(1e-10);
    let t = alpha.clamp(0.0, 1.0);
    (sa.ln() + (sb.ln() - sa.ln()) * t).exp()
}

fn integrate_orbital_camera_pose(camera: &mut PanOrbitCamera, dt: f32) -> (DVec3, Quat) {
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
            camera.set_focus_au(camera.target_focus_au);
            camera.radius = interpolate_log_radius(
                camera.radius,
                camera.target_radius,
                zoom_alpha,
                camera.min_radius,
                camera.max_radius,
            );
        } else if camera.focus_transition_progress < 1.0 {
            camera.focus_transition_progress =
                (camera.focus_transition_progress + dt * 2.0).min(1.0);
            let t = camera.focus_transition_progress;
            let u = t * t * (3.0 - 2.0 * t);
            let inv_u = 1.0 - u;
            let smooth_t = 1.0 - inv_u * inv_u * inv_u;

            let next_focus = camera
                .focus_transition_start_au
                .lerp(camera.target_focus_au, f64::from(smooth_t));
            camera.set_focus_au(next_focus);
            let d_end = camera
                .target_radius
                .clamp(camera.min_radius, camera.max_radius);
            camera.radius = interpolate_log_scalar(camera.radius_transition_start, d_end, smooth_t)
                .clamp(camera.min_radius, camera.max_radius);

            if camera.focus_transition_progress >= 1.0 {
                camera.set_focus_au(camera.target_focus_au);
                camera.radius = d_end;
            } else {
                let start_off_au = camera.transition_start_offset_au;
                let d_start = (start_off_au.length() as f32).max(1e-10);
                let d0 = if start_off_au.length_squared() > 1e-18 {
                    (start_off_au / f64::from(d_start))
                        .as_vec3()
                        .normalize_or(back_dir)
                } else {
                    back_dir
                };
                let t_ang = (t / 0.65).min(1.0);
                let sigma = t_ang * t_ang * (3.0 - 2.0 * t_ang);
                let dir_t = slerp_unit_directions(d0, back_dir, rot * Vec3::Y, sigma);
                let dist_t = interpolate_log_scalar(d_start, d_end, smooth_t)
                    .clamp(camera.min_radius, camera.max_radius);
                let eye_au = camera.target_focus_au + dir_t.as_dvec3() * f64::from(dist_t);
                camera.last_orbital_eye_au = Some(eye_au);
                camera.last_orbital_eye = Some(eye_au.as_vec3());
                return (eye_au, rot);
            }
        } else {
            camera.set_focus_au(camera.target_focus_au);
            camera.radius = interpolate_log_radius(
                camera.radius,
                camera.target_radius,
                zoom_alpha,
                camera.min_radius,
                camera.max_radius,
            );
        }
    } else {
        let next_focus = camera
            .focus_au
            .lerp(camera.target_focus_au, f64::from(focus_alpha));
        camera.set_focus_au(next_focus);
        camera.radius = interpolate_log_radius(
            camera.radius,
            camera.target_radius,
            zoom_alpha,
            camera.min_radius,
            camera.max_radius,
        );
    }

    let eye_au = camera.focus_au + back_dir.as_dvec3() * f64::from(camera.radius);
    camera.last_orbital_eye_au = Some(eye_au);
    camera.last_orbital_eye = Some(eye_au.as_vec3());
    (eye_au, rot)
}

fn sync_camera_target_transition_state(camera: &mut PanOrbitCamera) {
    if camera.target_entity == camera.previous_target_entity {
        return;
    }
    if let Some(ent) = camera.target_entity {
        let target_au = camera.target_focus_au;
        let target_rad = camera.target_radius;
        camera.initiate_target_transition_au(ent, target_au, target_rad);
    } else {
        if camera.focus_transition_progress < 1.0 {
            let eye_au = camera.current_orbital_eye_au();
            let back = (camera.orbital_rotation() * Vec3::Z).as_dvec3();
            let next_focus = eye_au - back * f64::from(camera.radius);
            camera.set_focus_au(next_focus);
            camera.set_target_focus_au(next_focus);
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
    opt_origin: Option<Res<RenderOrigin>>,
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
    let Ok((camera_comp, mut camera, mut transform, global_transform)) = camera_query.single_mut()
    else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    camera.sync_f64_from_f32_edits();

    let text_focused =
        crate::game::ui::widgets::is_text_input_focused(focus.as_deref(), &editable_query);
    let dt = time.as_ref().map_or(0.016, |t| t.delta_secs());

    if camera.surface_horizon_mode {
        camera.previous_target_entity = camera.target_entity;
        let eye_au = origin.to_sim(transform.translation);
        camera.last_orbital_eye_au = Some(eye_au);
        camera.last_orbital_eye = Some(eye_au.as_vec3());
        if player_state.just_selected_via_ui {
            player_state.just_selected_via_ui = false;
        }
        if !text_focused
            && (keyboard_input.just_pressed(KeyCode::KeyF)
                || keyboard_input.just_pressed(KeyCode::KeyR)
                || keyboard_input.just_pressed(KeyCode::Escape))
        {
            camera.surface_horizon_mode = false;
            camera.entry_transition_progress = 1.0;
            camera.last_orbital_eye = None;
            camera.last_orbital_eye_au = None;
        }
        return;
    }

    let (star_pos, min_orbit_r) = compute_system_min_orbit_r(&targets_query);
    update_camera_min_zoom_bounds(
        &mut camera,
        &config,
        &targets_query,
        star_pos,
        min_orbit_r,
        player_state.show_atmospheres(),
    );

    sync_camera_target_transition_state(&mut camera);

    let cursor_over_ui = ui_interaction_query
        .iter()
        .any(|(h, p, n, s, c)| is_ui_interaction_blocking(h, p, n, s, c))
        || window.cursor_position().is_some_and(|cp| {
            crate::rendering::camera_picking::is_cursor_over_active_ui(
                window,
                cp,
                &ui_interaction_query,
                &ui_nodes_query,
                opt_visibility.as_deref(),
            )
        })
        || is_cursor_over_scroll_container(window.cursor_position(), &ui_nodes_query);

    handle_camera_target_picking(
        window,
        camera_comp,
        global_transform,
        &targets_query,
        &mut player_state,
        &mut camera,
        &config,
        &origin,
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
        text_focused,
        dt,
    );

    let (eye_au, rot) = integrate_orbital_camera_pose(&mut camera, dt);
    transform.translation = origin.to_render(eye_au);
    transform.rotation = rot;
}
