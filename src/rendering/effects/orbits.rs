use bevy::prelude::*;

use crate::utils::math::*;

const ORBIT_SAMPLES: usize = 128;
const ELLIPTIC_RIBBON_SAMPLES: usize = 64;
const LEADING_RIBBON_SAMPLES: usize = 20;
const HYP_SAMPLES: usize = 64;
const HYP_RIBBON_SAMPLES: usize = 36;

/// Computes a smooth `[0.0, 1.0]` fade factor that excludes orbit line segments from piercing
/// the interior/limb of the celestial body and softly attenuates segments far from the camera.
#[inline]
fn compute_segment_visibility_fade(
    seg_mid: Vec3,
    body_world: Vec3,
    visual_rad: f32,
    cam_pos: Option<Vec3>,
) -> f32 {
    let body_clear = if visual_rad > 1e-6 {
        let dist_to_body = seg_mid.distance(body_world);
        let inner = visual_rad * 1.08;
        let outer = visual_rad * 1.85;
        let t = ((dist_to_body - inner) / (outer - inner).max(1e-6)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    } else {
        1.0
    };

    let close_cam_fade = if let Some(cp) = cam_pos {
        if visual_rad > 1e-6 {
            let cam_to_body = cp.distance(body_world);
            ((cam_to_body / visual_rad - 1.8) / 5.5).clamp(0.18, 1.0)
        } else {
            1.0
        }
    } else {
        1.0
    };

    body_clear * close_cam_fade
}

pub fn draw_ambient_and_trailing_orbit_ribbon(
    gizmos: &mut Gizmos,
    elements: &OrbitalElements,
    anchor_vec: Vec3,
    base_rgba: Srgba,
    is_selected: bool,
    visual_rad: f32,
    cam_pos: Option<Vec3>,
) {
    if elements.semi_major_axis <= 0.0 || elements.eccentricity >= 1.0 {
        return;
    }
    let Some(p) = conic_semilatus_rectum(elements) else {
        return;
    };

    let nu_0 = elements.true_anomaly;
    let body_world =
        position_at_true_anomaly_with_p(elements, p, nu_0).map_or(anchor_vec, |pt| pt + anchor_vec);

    let ambient_alpha = if is_selected { 0.42 } else { 0.14 };

    let inv_orbit_samples = 1.0 / (ORBIT_SAMPLES as f64);
    let mut prev_orbit_pt: Option<Vec3> = None;
    for i in 0..=ORBIT_SAMPLES {
        let nu = (i as f64 * inv_orbit_samples) * std::f64::consts::TAU;
        if let Some(pt) = position_at_true_anomaly_with_p(elements, p, nu) {
            let world_pt = pt + anchor_vec;
            if let Some(prev) = prev_orbit_pt {
                let seg_mid = prev.midpoint(world_pt);
                let fade =
                    compute_segment_visibility_fade(seg_mid, body_world, visual_rad, cam_pos);
                if fade > 0.01 {
                    let ambient_color = Color::srgba(
                        base_rgba.red,
                        base_rgba.green,
                        base_rgba.blue,
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

    // Trailing motion ribbon (behind the body's orbital velocity vector)
    let arc_rad = if is_selected {
        1.75 * std::f64::consts::PI
    } else {
        1.20 * std::f64::consts::PI
    };
    let inv_ribbon_samples = 1.0 / (ELLIPTIC_RIBBON_SAMPLES as f64);
    let alpha_scale = if is_selected { 0.92 } else { 0.80 };
    let mut prev_ribbon: Option<(Vec3, f32)> = None;

    for i in 0..=ELLIPTIC_RIBBON_SAMPLES {
        let t = i as f64 * inv_ribbon_samples;
        let nu = nu_0 - t * arc_rad;
        if let Some(pt) = position_at_true_anomaly_with_p(elements, p, nu) {
            let a2 = (1.0 - t as f32).max(0.0);
            let p2_world = pt + anchor_vec;
            if let Some((p1_world, a1)) = prev_ribbon {
                let seg_mid = p1_world.midpoint(p2_world);
                let fade =
                    compute_segment_visibility_fade(seg_mid, body_world, visual_rad, cam_pos);
                if fade > 0.01 {
                    let avg_a = f32::midpoint(a1, a2);
                    let curve_a = avg_a.powf(0.82);
                    let seg_alpha = curve_a * alpha_scale * fade;
                    let boost = (avg_a * 0.28).min(0.28);
                    let seg_color = Color::srgba(
                        (base_rgba.red + boost).min(1.0),
                        (base_rgba.green + boost).min(1.0),
                        (base_rgba.blue + boost).min(1.0),
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
    let lead_arc_rad = if is_selected {
        0.32 * std::f64::consts::PI
    } else {
        0.18 * std::f64::consts::PI
    };
    let inv_lead_samples = 1.0 / (LEADING_RIBBON_SAMPLES as f64);
    let lead_alpha_scale = if is_selected { 0.68 } else { 0.40 };
    let mut prev_lead: Option<(Vec3, f32)> = None;

    for i in 0..=LEADING_RIBBON_SAMPLES {
        let t = i as f64 * inv_lead_samples;
        let nu = nu_0 + t * lead_arc_rad;
        if let Some(pt) = position_at_true_anomaly_with_p(elements, p, nu) {
            let a2 = (1.0 - t as f32).max(0.0);
            let p2_world = pt + anchor_vec;
            if let Some((p1_world, a1)) = prev_lead {
                let seg_mid = p1_world.midpoint(p2_world);
                let fade =
                    compute_segment_visibility_fade(seg_mid, body_world, visual_rad, cam_pos);
                if fade > 0.01 {
                    let avg_a = f32::midpoint(a1, a2);
                    let seg_alpha = avg_a.powf(1.35) * lead_alpha_scale * fade;
                    let boost = (avg_a * 0.35).min(0.35);
                    let seg_color = Color::srgba(
                        (base_rgba.red + boost * 0.7).min(1.0),
                        (base_rgba.green + boost).min(1.0),
                        (base_rgba.blue + boost).min(1.0),
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
    anchor_vec: Vec3,
) {
    let (opt_peri, opt_apo) = apsides_positions(elements);
    let marker_size = (elements.semi_major_axis as f32 * 0.018).clamp(0.00015, 0.35);

    if let Some(peri) = opt_peri {
        let q_pt = peri + anchor_vec;
        let q_col = Color::srgba(0.20, 1.0, 0.50, 0.90);
        gizmos.sphere(
            Isometry3d::from_translation(q_pt),
            marker_size * 0.28,
            q_col,
        );
        gizmos.line(
            q_pt - Vec3::X * marker_size,
            q_pt + Vec3::Y * marker_size,
            q_col,
        );
        gizmos.line(
            q_pt + Vec3::Y * marker_size,
            q_pt + Vec3::X * marker_size,
            q_col,
        );
        gizmos.line(
            q_pt + Vec3::X * marker_size,
            q_pt - Vec3::Y * marker_size,
            q_col,
        );
        gizmos.line(
            q_pt - Vec3::Y * marker_size,
            q_pt - Vec3::X * marker_size,
            q_col,
        );
    }

    if let Some(apo) = opt_apo {
        let q_pt = apo + anchor_vec;
        let q_col = Color::srgba(1.0, 0.50, 0.15, 0.85);
        gizmos.sphere(
            Isometry3d::from_translation(q_pt),
            marker_size * 0.28,
            q_col,
        );
        gizmos.line(
            q_pt - Vec3::X * marker_size,
            q_pt + Vec3::Y * marker_size,
            q_col,
        );
        gizmos.line(
            q_pt + Vec3::Y * marker_size,
            q_pt + Vec3::X * marker_size,
            q_col,
        );
        gizmos.line(
            q_pt + Vec3::X * marker_size,
            q_pt - Vec3::Y * marker_size,
            q_col,
        );
        gizmos.line(
            q_pt - Vec3::Y * marker_size,
            q_pt - Vec3::X * marker_size,
            q_col,
        );

        if let Some(peri) = opt_peri {
            let p_pt = peri + anchor_vec;
            let steps = 16;
            let line_col = Color::srgba(0.7, 0.8, 1.0, 0.25);
            for s in (0..steps).step_by(2) {
                let t1 = s as f32 / steps as f32;
                let t2 = (s + 1) as f32 / steps as f32;
                let seg1 = p_pt.lerp(q_pt, t1);
                let seg2 = p_pt.lerp(q_pt, t2);
                gizmos.line(seg1, seg2, line_col);
            }
        }
    }

    if elements.inclination.abs() > 0.02 {
        let (opt_asc, opt_desc) = nodal_positions(elements);
        let node_col = Color::srgba(0.4, 0.8, 1.0, 0.65);
        if let Some(asc) = opt_asc {
            let pt = asc + anchor_vec;
            gizmos.circle(
                Isometry3d::new(pt, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                marker_size * 0.5,
                node_col,
            );
        }
        if let Some(desc) = opt_desc {
            let pt = desc + anchor_vec;
            gizmos.circle(
                Isometry3d::new(pt, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                marker_size * 0.5,
                node_col,
            );
        }
    }
}

pub fn draw_hyperbolic_trajectory(
    gizmos: &mut Gizmos,
    elements: &OrbitalElements,
    anchor_vec: Vec3,
    base_rgba: Srgba,
) {
    if elements.eccentricity >= 1.0 {
        if let Some(p) = conic_semilatus_rectum(elements) {
            let e = elements.eccentricity;
            let nu_max = ((-1.0 / e.max(1.0001)).clamp(-0.9999, 0.0).acos() - 0.08).max(0.2);
            let hyp_amb = Color::srgba(base_rgba.red, base_rgba.green, base_rgba.blue, 0.45);

            let inv_hyp = 1.0 / (HYP_SAMPLES as f64);
            let mut prev_hyp: Option<Vec3> = None;
            let mut drawn_any = false;
            for i in 0..=HYP_SAMPLES {
                let frac = i as f64 * inv_hyp;
                let nu = -nu_max + frac * 2.0 * nu_max;
                if let Some(pt) = position_at_true_anomaly_with_p(elements, p, nu) {
                    let world_pt = pt + anchor_vec;
                    if let Some(prev) = prev_hyp {
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
                    if let Some(pt) = position_at_true_anomaly_with_p(elements, p, nu) {
                        let a2 = (1.0 - t as f32).max(0.0);
                        let p2_world = pt + anchor_vec;
                        if let Some((p1_world, a1)) = prev_rib {
                            let avg_a = f32::midpoint(a1, a2);
                            let seg_alpha = avg_a.powf(0.75) * 0.95;
                            let col = Color::srgba(
                                base_rgba.red,
                                base_rgba.green,
                                base_rgba.blue,
                                seg_alpha,
                            );
                            gizmos.line(p1_world, p2_world, col);
                        }
                        prev_rib = Some((p2_world, a2));
                    } else {
                        prev_rib = None;
                    }
                }
            }
        }
    }

    let (opt_peri, _) = apsides_positions(elements);
    if let Some(peri) = opt_peri {
        let q_pt = peri + anchor_vec;
        let q_col = Color::srgba(1.0, 0.30, 0.50, 0.95);
        let m_size = (0.05 + elements.periapsis as f32 * 0.02).clamp(0.06, 0.45);
        gizmos.sphere(Isometry3d::from_translation(q_pt), m_size * 0.35, q_col);
    }
}
