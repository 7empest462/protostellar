use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::BodyType;
use crate::utils::math::*;

const ORBIT_SAMPLES: usize = 256;
const ORBIT_SAMPLES_MINOR_LOD: usize = 64;
const ELLIPTIC_RIBBON_SAMPLES: usize = 144;
const LEADING_RIBBON_SAMPLES: usize = 56;
const HYP_SAMPLES: usize = 128;
const HYP_RIBBON_SAMPLES: usize = 64;

/// Visual configuration and LOD hints for drawing a bound elliptical orbit.
#[derive(Clone, Copy)]
pub struct OrbitRenderStyle {
    pub base_rgba: Srgba,
    pub is_selected: bool,
    pub visual_rad: f32,
    pub cam_pos: Option<Vec3>,
    /// Reduced segment count and skipped motion ribbons for unselected/unhovered minor bodies.
    pub minor_body_lod: bool,
    /// Optional primary star visual sphere `(center, radius)` for behind-the-disk fade.
    pub primary_occluder: Option<(Vec3, f32)>,
}

/// Computes `[0.0, 1.0]` silhouette occlusion factor for world-space line midpoints behind an
/// opaque celestial sphere so depth-biased gizmos hide cleanly behind planets and stars.
#[inline]
pub fn sphere_occlusion_fade(
    point: Vec3,
    cam_pos: Option<Vec3>,
    occluder_center: Vec3,
    occluder_r: f32,
) -> f32 {
    let Some(cp) = cam_pos else {
        return 1.0;
    };
    if occluder_r <= 1e-10 {
        return 1.0;
    }
    let cam_to_pt = point - cp;
    let pt_dist = cam_to_pt.length();
    if pt_dist <= 1e-10 {
        return 1.0;
    }
    let ray_dir = cam_to_pt / pt_dist;
    let cam_to_occ = occluder_center - cp;
    let proj = cam_to_occ.dot(ray_dir);
    if proj <= 0.0 || proj >= pt_dist - occluder_r * 0.05 {
        return 1.0;
    }
    let miss_dist = (cam_to_occ - ray_dir * proj).length();
    let inner = occluder_r * 0.94;
    let outer = occluder_r * 1.04;
    let t = ((miss_dist - inner) / (outer - inner).max(1e-10)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Computes `[0.0, 1.0]` fade across up to two spherical occluders (`primary` + `body`).
#[inline]
pub fn combined_sphere_occlusion_fade(
    point: Vec3,
    cam_pos: Option<Vec3>,
    primary: Option<(Vec3, f32)>,
    secondary: Option<(Vec3, f32)>,
) -> f32 {
    let f1 = primary.map_or(1.0, |(c, r)| sphere_occlusion_fade(point, cam_pos, c, r));
    let f2 = secondary.map_or(1.0, |(c, r)| sphere_occlusion_fade(point, cam_pos, c, r));
    f1 * f2
}

/// Draws a smooth high-resolution circle that cleanly fades segments passing behind a
/// nearby celestial sphere (or emits a fast native high-res circle when unobstructed).
pub fn draw_occluded_circle(
    gizmos: &mut Gizmos,
    center: Vec3,
    rot: Quat,
    radius: f32,
    color: Color,
    segments: u32,
    cam_pos: Option<Vec3>,
    occluders: (Option<(Vec3, f32)>, Option<(Vec3, f32)>),
) {
    let needs_occ = cam_pos.is_some()
        && [occluders.0, occluders.1]
            .into_iter()
            .flatten()
            .any(|(c, r)| center.distance(c) <= radius + r * 3.0);
    if !needs_occ {
        gizmos
            .circle(Isometry3d::new(center, rot), radius, color)
            .resolution(segments);
        return;
    }

    let srgba = color.to_srgba();
    let seg_count = segments.max(32) as usize;
    let inv = 1.0 / (seg_count as f32);
    let mut prev: Option<Vec3> = None;
    for i in 0..=seg_count {
        let th = (i as f32) * inv * std::f32::consts::TAU;
        let pt = center + rot * Vec3::new(radius * th.cos(), radius * th.sin(), 0.0);
        if let Some(p0) = prev {
            let mid = p0.midpoint(pt);
            let fade = combined_sphere_occlusion_fade(mid, cam_pos, occluders.0, occluders.1);
            if fade > 0.02 {
                gizmos.line(
                    p0,
                    pt,
                    Color::srgba(srgba.red, srgba.green, srgba.blue, srgba.alpha * fade),
                );
            }
        }
        prev = Some(pt);
    }
}

/// Draws four crisp segmented bracket arcs spaced cleanly around a ring in plane `rot`.
pub fn draw_segmented_ring_arcs(
    gizmos: &mut Gizmos,
    center: Vec3,
    rot: Quat,
    radius: f32,
    color: Color,
    arc_span_rad: f32,
    steps_per_arc: usize,
) {
    let half_span = arc_span_rad * 0.5;
    let inv = 1.0 / (steps_per_arc.max(4) as f32);
    for quad in 0..4 {
        let center_ang = (quad as f32) * std::f32::consts::FRAC_PI_2;
        let mut prev: Option<Vec3> = None;
        for s in 0..=steps_per_arc.max(4) {
            let ang = center_ang - half_span + (s as f32) * inv * arc_span_rad;
            let pt = center + rot * Vec3::new(radius * ang.cos(), radius * ang.sin(), 0.0);
            if let Some(p0) = prev {
                gizmos.line(p0, pt, color);
            }
            prev = Some(pt);
        }
    }
}

/// Computes a smooth `[0.0, 1.0]` fade factor that excludes orbit line segments from piercing
/// the interior/limb of the celestial body and fades behind spherical occluders.
#[inline]
fn compute_segment_visibility_fade(
    seg_mid: Vec3,
    body_world: Vec3,
    visual_rad: f32,
    cam_pos: Option<Vec3>,
    primary_occluder: Option<(Vec3, f32)>,
) -> f32 {
    let body_clear = if visual_rad > 1e-10 {
        let dist_to_body = seg_mid.distance(body_world);
        let inner = visual_rad * 1.08;
        let outer = visual_rad * 1.85;
        let t = ((dist_to_body - inner) / (outer - inner).max(1e-10)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    } else {
        1.0
    };

    let close_cam_fade = if let Some(cp) = cam_pos {
        if visual_rad > 1e-10 {
            let cam_to_body = cp.distance(body_world);
            ((cam_to_body / visual_rad - 1.8) / 5.5).clamp(0.22, 1.0)
        } else {
            1.0
        }
    } else {
        1.0
    };

    let sphere_occ = combined_sphere_occlusion_fade(
        seg_mid,
        cam_pos,
        Some((body_world, visual_rad)),
        primary_occluder,
    );

    body_clear * close_cam_fade * sphere_occ
}

/// Converts uniform eccentric anomaly sample index into true anomaly so highly eccentric orbits
/// stay evenly subdivided without coarse straight polygon chords away from periapsis.
#[inline]
fn true_anomaly_from_uniform_eccentric_sample(frac: f64, e: f64) -> f64 {
    let big_e = frac * std::f64::consts::TAU;
    if e <= 1e-5 || e >= 0.9999 {
        return big_e;
    }
    let half_e = big_e * 0.5;
    2.0 * ((1.0 + e).sqrt() * half_e.sin()).atan2((1.0 - e).sqrt() * half_e.cos())
}

pub fn draw_ambient_and_trailing_orbit_ribbon(
    gizmos: &mut Gizmos,
    elements: &OrbitalElements,
    anchor_rel_au: DVec3,
    style: OrbitRenderStyle,
) {
    if elements.semi_major_axis <= 0.0 || elements.eccentricity >= 1.0 {
        return;
    }
    let Some(p) = conic_semilatus_rectum(elements) else {
        return;
    };

    let nu_0 = elements.true_anomaly;
    let body_world = position_at_true_anomaly_dvec_with_p(elements, p, nu_0).map_or_else(
        || anchor_rel_au.as_vec3(),
        |pt| (anchor_rel_au + pt).as_vec3(),
    );

    let ambient_alpha = if style.is_selected { 0.34 } else { 0.14 };
    let samples = if style.minor_body_lod {
        ORBIT_SAMPLES_MINOR_LOD
    } else {
        ORBIT_SAMPLES
    };
    let inv_orbit_samples = 1.0 / (samples as f64);
    let mut prev_orbit_pt: Option<Vec3> = None;
    for i in 0..=samples {
        let nu = true_anomaly_from_uniform_eccentric_sample(
            i as f64 * inv_orbit_samples,
            elements.eccentricity,
        );
        if let Some(pt) = position_at_true_anomaly_dvec_with_p(elements, p, nu) {
            let world_pt = (anchor_rel_au + pt).as_vec3();
            if let Some(prev) = prev_orbit_pt {
                let seg_mid = prev.midpoint(world_pt);
                let fade = compute_segment_visibility_fade(
                    seg_mid,
                    body_world,
                    style.visual_rad,
                    style.cam_pos,
                    style.primary_occluder,
                );
                if fade > 0.01 {
                    let ambient_color = Color::srgba(
                        style.base_rgba.red,
                        style.base_rgba.green,
                        style.base_rgba.blue,
                        ambient_alpha * fade,
                    );
                    gizmos.line(prev, world_pt, ambient_color);
                }
            }
            prev_orbit_pt = Some(world_pt);
        } else {
            prev_orbit_pt = None;
        }
    }

    if style.minor_body_lod {
        return;
    }

    draw_orbit_motion_ribbons(gizmos, elements, p, anchor_rel_au, body_world, nu_0, style);
}

fn draw_orbit_motion_ribbons(
    gizmos: &mut Gizmos,
    elements: &OrbitalElements,
    p: f64,
    anchor_rel_au: DVec3,
    body_world: Vec3,
    nu_0: f64,
    style: OrbitRenderStyle,
) {
    // Trailing motion ribbon (behind the body's orbital velocity vector)
    let arc_rad = if style.is_selected {
        1.75 * std::f64::consts::PI
    } else {
        1.20 * std::f64::consts::PI
    };
    let inv_ribbon_samples = 1.0 / (ELLIPTIC_RIBBON_SAMPLES as f64);
    let alpha_scale = if style.is_selected { 0.88 } else { 0.72 };
    let mut prev_ribbon: Option<(Vec3, f32)> = None;

    for i in 0..=ELLIPTIC_RIBBON_SAMPLES {
        let t = i as f64 * inv_ribbon_samples;
        let nu = nu_0 - t * arc_rad;
        if let Some(pt) = position_at_true_anomaly_dvec_with_p(elements, p, nu) {
            let a2 = (1.0 - t as f32).max(0.0);
            let p2_world = (anchor_rel_au + pt).as_vec3();
            if let Some((p1_world, a1)) = prev_ribbon {
                let seg_mid = p1_world.midpoint(p2_world);
                let fade = compute_segment_visibility_fade(
                    seg_mid,
                    body_world,
                    style.visual_rad,
                    style.cam_pos,
                    style.primary_occluder,
                );
                if fade > 0.01 {
                    let avg_a = f32::midpoint(a1, a2);
                    let curve_a = avg_a.powf(0.90);
                    let seg_alpha = curve_a * alpha_scale * fade;
                    let boost = (avg_a * 0.25).min(0.25);
                    let seg_color = Color::srgba(
                        (style.base_rgba.red + boost).min(1.0),
                        (style.base_rgba.green + boost).min(1.0),
                        (style.base_rgba.blue + boost).min(1.0),
                        seg_alpha,
                    );
                    gizmos.line(p1_world, p2_world, seg_color);
                }
            }
            prev_ribbon = Some((p2_world, a2));
        } else {
            prev_ribbon = None;
        }
    }

    // Subtle leading trajectory glow just ahead of the body
    let lead_arc_rad = if style.is_selected {
        0.34 * std::f64::consts::PI
    } else {
        0.18 * std::f64::consts::PI
    };
    let inv_lead_samples = 1.0 / (LEADING_RIBBON_SAMPLES as f64);
    let lead_alpha_scale = if style.is_selected { 0.62 } else { 0.36 };
    let mut prev_lead: Option<(Vec3, f32)> = None;

    for i in 0..=LEADING_RIBBON_SAMPLES {
        let t = i as f64 * inv_lead_samples;
        let nu = nu_0 + t * lead_arc_rad;
        if let Some(pt) = position_at_true_anomaly_dvec_with_p(elements, p, nu) {
            let a2 = (1.0 - t as f32).max(0.0);
            let p2_world = (anchor_rel_au + pt).as_vec3();
            if let Some((p1_world, a1)) = prev_lead {
                let seg_mid = p1_world.midpoint(p2_world);
                let fade = compute_segment_visibility_fade(
                    seg_mid,
                    body_world,
                    style.visual_rad,
                    style.cam_pos,
                    style.primary_occluder,
                );
                if fade > 0.01 {
                    let avg_a = f32::midpoint(a1, a2);
                    let seg_alpha = avg_a.powf(1.4) * lead_alpha_scale * fade;
                    let boost = (avg_a * 0.30).min(0.30);
                    let seg_color = Color::srgba(
                        (style.base_rgba.red + boost * 0.7).min(1.0),
                        (style.base_rgba.green + boost).min(1.0),
                        (style.base_rgba.blue + boost).min(1.0),
                        seg_alpha,
                    );
                    gizmos.line(p1_world, p2_world, seg_color);
                }
            }
            prev_lead = Some((p2_world, a2));
        } else {
            prev_lead = None;
        }
    }
}

pub fn draw_conic_apsides_and_nodes(
    gizmos: &mut Gizmos,
    elements: &OrbitalElements,
    anchor_rel_au: DVec3,
    cam_pos: Option<Vec3>,
    primary_occluder: Option<(Vec3, f32)>,
) {
    let anchor_vec = anchor_rel_au.as_vec3();
    let (opt_peri, opt_apo) = apsides_positions_dvec(elements);
    let marker_size = (elements.semi_major_axis as f32 * 0.015).clamp(1e-6, 0.28);
    let ring_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);

    if let Some(peri) = opt_peri {
        let q_pt = (anchor_rel_au + peri).as_vec3();
        let occ = primary_occluder.map_or(1.0, |(c, r)| sphere_occlusion_fade(q_pt, cam_pos, c, r));
        if occ > 0.05 {
            let q_col = Color::srgba(0.25, 0.96, 0.58, 0.85 * occ);
            let outer_col = Color::srgba(0.25, 0.96, 0.58, 0.35 * occ);
            let dir = (q_pt - anchor_vec).normalize_or(Vec3::X);
            gizmos
                .circle(Isometry3d::new(q_pt, ring_rot), marker_size * 0.26, q_col)
                .resolution(32);
            gizmos
                .circle(
                    Isometry3d::new(q_pt, ring_rot),
                    marker_size * 0.85,
                    outer_col,
                )
                .resolution(48);
            gizmos.line(
                q_pt + dir * (marker_size * 0.55),
                q_pt + dir * (marker_size * 1.15),
                q_col,
            );
            gizmos.line(
                q_pt - dir * (marker_size * 0.55),
                q_pt - dir * (marker_size * 1.15),
                q_col,
            );
        }
    }

    if let Some(apo) = opt_apo {
        let q_pt = (anchor_rel_au + apo).as_vec3();
        let occ = primary_occluder.map_or(1.0, |(c, r)| sphere_occlusion_fade(q_pt, cam_pos, c, r));
        if occ > 0.05 {
            let q_col = Color::srgba(1.0, 0.58, 0.20, 0.82 * occ);
            let outer_col = Color::srgba(1.0, 0.58, 0.20, 0.32 * occ);
            let dir = (q_pt - anchor_vec).normalize_or(Vec3::X);
            gizmos
                .circle(Isometry3d::new(q_pt, ring_rot), marker_size * 0.26, q_col)
                .resolution(32);
            gizmos
                .circle(
                    Isometry3d::new(q_pt, ring_rot),
                    marker_size * 0.85,
                    outer_col,
                )
                .resolution(48);
            gizmos.line(
                q_pt + dir * (marker_size * 0.55),
                q_pt + dir * (marker_size * 1.15),
                q_col,
            );
            gizmos.line(
                q_pt - dir * (marker_size * 0.55),
                q_pt - dir * (marker_size * 1.15),
                q_col,
            );
        }

        if let Some(peri) = opt_peri {
            let p_pt = (anchor_rel_au + peri).as_vec3();
            let steps = 48;
            for s in (0..steps).step_by(2) {
                let t1 = s as f32 / steps as f32;
                let t2 = (s + 1) as f32 / steps as f32;
                let seg1 = p_pt.lerp(q_pt, t1);
                let seg2 = p_pt.lerp(q_pt, t2);
                let seg_mid = seg1.midpoint(seg2);
                // Emphasize major axis near apsides and soften sharply across center/star
                let edge_bias = ((t1 - 0.5).abs() * 2.0).powf(1.4);
                let occ = primary_occluder
                    .map_or(1.0, |(c, r)| sphere_occlusion_fade(seg_mid, cam_pos, c, r));
                let a = (0.04 + 0.20 * edge_bias) * occ;
                if a > 0.015 {
                    gizmos.line(seg1, seg2, Color::srgba(0.68, 0.82, 1.0, a));
                }
            }
        }
    }

    if elements.inclination.abs() > 0.02 {
        let (opt_asc, opt_desc) = nodal_positions_dvec(elements);
        for node in [opt_asc, opt_desc].into_iter().flatten() {
            let pt = (anchor_rel_au + node).as_vec3();
            let occ =
                primary_occluder.map_or(1.0, |(c, r)| sphere_occlusion_fade(pt, cam_pos, c, r));
            if occ > 0.05 {
                let node_col = Color::srgba(0.42, 0.82, 1.0, 0.58 * occ);
                draw_segmented_ring_arcs(
                    gizmos,
                    pt,
                    ring_rot,
                    marker_size * 0.55,
                    node_col,
                    1.0,
                    6,
                );
            }
        }
    }
}

pub fn draw_hyperbolic_trajectory(
    gizmos: &mut Gizmos,
    elements: &OrbitalElements,
    anchor_rel_au: DVec3,
    base_rgba: Srgba,
    cam_pos: Option<Vec3>,
    primary_occluder: Option<(Vec3, f32)>,
) {
    if elements.eccentricity >= 1.0
        && let Some(p) = conic_semilatus_rectum(elements)
    {
        let e = elements.eccentricity;
        let nu_max = ((-1.0 / e.max(1.0001)).clamp(-0.9999, 0.0).acos() - 0.08).max(0.2);

        let inv_hyp = 1.0 / (HYP_SAMPLES as f64);
        let mut prev_hyp: Option<Vec3> = None;
        let mut drawn_any = false;
        for i in 0..=HYP_SAMPLES {
            let frac = i as f64 * inv_hyp;
            let nu = -nu_max + frac * 2.0 * nu_max;
            if let Some(pt) = position_at_true_anomaly_dvec_with_p(elements, p, nu) {
                let world_pt = (anchor_rel_au + pt).as_vec3();
                if let Some(prev) = prev_hyp {
                    let mid = prev.midpoint(world_pt);
                    let occ = primary_occluder
                        .map_or(1.0, |(c, r)| sphere_occlusion_fade(mid, cam_pos, c, r));
                    let end_fade = (1.0 - (frac as f32 * 2.0 - 1.0).abs().powf(2.4)).max(0.15);
                    let hyp_amb = Color::srgba(
                        base_rgba.red,
                        base_rgba.green,
                        base_rgba.blue,
                        0.42 * end_fade * occ,
                    );
                    gizmos.line(prev, world_pt, hyp_amb);
                    drawn_any = true;
                }
                prev_hyp = Some(world_pt);
            } else {
                prev_hyp = None;
            }
        }

        if drawn_any {
            let inv_rib = 1.0 / (HYP_RIBBON_SAMPLES as f64);
            let start_nu = elements.true_anomaly.clamp(-nu_max, nu_max);
            let end_nu = (start_nu - 1.2 * std::f64::consts::PI).max(-nu_max);
            let mut prev_rib: Option<(Vec3, f32)> = None;

            for i in 0..=HYP_RIBBON_SAMPLES {
                let t = i as f64 * inv_rib;
                let nu = start_nu + t * (end_nu - start_nu);
                if let Some(pt) = position_at_true_anomaly_dvec_with_p(elements, p, nu) {
                    let a2 = (1.0 - t as f32).max(0.0);
                    let p2_world = (anchor_rel_au + pt).as_vec3();
                    if let Some((p1_world, a1)) = prev_rib {
                        let mid = p1_world.midpoint(p2_world);
                        let occ = primary_occluder
                            .map_or(1.0, |(c, r)| sphere_occlusion_fade(mid, cam_pos, c, r));
                        let avg_a = f32::midpoint(a1, a2);
                        let seg_alpha = avg_a.powf(0.8) * 0.90 * occ;
                        let col =
                            Color::srgba(base_rgba.red, base_rgba.green, base_rgba.blue, seg_alpha);
                        gizmos.line(p1_world, p2_world, col);
                    }
                    prev_rib = Some((p2_world, a2));
                } else {
                    prev_rib = None;
                }
            }
        }
    }

    let (opt_peri, _) = apsides_positions_dvec(elements);
    if let Some(peri) = opt_peri {
        let q_pt = (anchor_rel_au + peri).as_vec3();
        let q_col = Color::srgba(1.0, 0.30, 0.50, 0.90);
        let m_size = (elements.periapsis as f32 * 0.015).clamp(1e-5, 0.35);
        gizmos
            .circle(
                Isometry3d::new(q_pt, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                m_size * 0.45,
                q_col,
            )
            .resolution(48);
    }
}

pub fn determine_orbit_color(
    body_type: BodyType,
    is_selected: bool,
    is_satellite: bool,
    eccentricity: f64,
) -> Color {
    if is_selected {
        Color::srgba(0.15, 0.95, 1.0, 1.0)
    } else if is_satellite {
        Color::srgba(0.80, 0.85, 1.0, 0.85)
    } else if eccentricity >= 1.0 {
        Color::srgba(1.0, 0.25, 0.45, 0.95)
    } else {
        match body_type {
            t if t.is_star_or_remnant() => Color::srgba(1.0, 0.92, 0.70, 0.95),
            BodyType::TerrestrialPlanet => Color::srgba(0.25, 1.0, 0.55, 0.90),
            BodyType::SuperEarth => Color::srgba(0.15, 0.90, 0.85, 0.90),
            BodyType::GasGiant => Color::srgba(1.0, 0.75, 0.20, 0.90),
            BodyType::IceGiant => Color::srgba(0.30, 0.80, 1.0, 0.90),
            BodyType::Protoplanet => Color::srgba(1.0, 0.55, 0.20, 0.85),
            BodyType::Planetesimal | BodyType::Asteroid => Color::srgba(0.70, 0.75, 0.85, 0.55),
            BodyType::Comet => Color::srgba(0.45, 1.0, 0.95, 0.90),
            _ => Color::srgba(0.75, 0.75, 0.85, 0.65),
        }
    }
}
