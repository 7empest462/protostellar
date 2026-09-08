use bevy::prelude::*;

use crate::utils::math::*;

pub fn draw_ambient_and_trailing_orbit_ribbon(
    gizmos: &mut Gizmos,
    elements: &OrbitalElements,
    anchor_vec: Vec3,
    base_rgba: Srgba,
    is_selected: bool,
) {
    let ambient_alpha = if is_selected { 0.60 } else { 0.28 };
    let ambient_color = Color::srgba(
        base_rgba.red,
        base_rgba.green,
        base_rgba.blue,
        ambient_alpha,
    );
    let orbit_points = generate_orbit_points(elements, 96);
    if orbit_points.len() > 1 {
        for window in orbit_points.windows(2) {
            if let [p0, p1] = window {
                gizmos.line(*p0 + anchor_vec, *p1 + anchor_vec, ambient_color);
            }
        }
    }

    let arc_rad = if is_selected {
        1.85 * std::f64::consts::PI
    } else {
        1.25 * std::f64::consts::PI
    };
    let ribbon = generate_trailing_ribbon_points(elements, 56, arc_rad);
    if ribbon.len() > 1 {
        for window in ribbon.windows(2) {
            if let [w0, w1] = window {
                let (p1, a1) = *w0;
                let (p2, a2) = *w1;
                let avg_a = f32::midpoint(a1, a2);
                let curve_a = avg_a.powf(0.75);
                let seg_alpha = curve_a * (if is_selected { 0.98 } else { 0.88 });
                let boost = (avg_a * 0.30).min(0.30);
                let seg_color = Color::srgba(
                    (base_rgba.red + boost).min(1.0),
                    (base_rgba.green + boost).min(1.0),
                    (base_rgba.blue + boost).min(1.0),
                    seg_alpha,
                );
                gizmos.line(p1 + anchor_vec, p2 + anchor_vec, seg_color);

                if is_selected && avg_a > 0.25 {
                    let halo_color = Color::srgba(0.3, 0.95, 1.0, seg_alpha * 0.45);
                    let offset_y = Vec3::Y * (0.012 * avg_a);
                    gizmos.line(
                        p1 + anchor_vec + offset_y,
                        p2 + anchor_vec + offset_y,
                        halo_color,
                    );
                    gizmos.line(
                        p1 + anchor_vec - offset_y,
                        p2 + anchor_vec - offset_y,
                        halo_color,
                    );
                }
            }
        }
    }
}

pub fn draw_conic_apsides_and_nodes(
    gizmos: &mut Gizmos,
    elements: &OrbitalElements,
    anchor_vec: Vec3,
) {
    let (opt_peri, opt_apo) = apsides_positions(elements);
    let marker_size = (0.025 + elements.semi_major_axis as f32 * 0.012).clamp(0.04, 0.35);

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
    let hyp_points = generate_hyperbolic_orbit_points(elements, 64);
    if hyp_points.len() > 1 {
        let hyp_amb = Color::srgba(base_rgba.red, base_rgba.green, base_rgba.blue, 0.45);
        for window in hyp_points.windows(2) {
            if let [p0, p1] = window {
                gizmos.line(*p0 + anchor_vec, *p1 + anchor_vec, hyp_amb);
            }
        }

        let ribbon = generate_trailing_ribbon_points(elements, 36, 1.2 * std::f64::consts::PI);
        if ribbon.len() > 1 {
            for window in ribbon.windows(2) {
                if let [w0, w1] = window {
                    let (p1, a1) = *w0;
                    let (p2, a2) = *w1;
                    let avg_a = f32::midpoint(a1, a2);
                    let seg_alpha = avg_a.powf(0.75) * 0.95;
                    let col =
                        Color::srgba(base_rgba.red, base_rgba.green, base_rgba.blue, seg_alpha);
                    gizmos.line(p1 + anchor_vec, p2 + anchor_vec, col);
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
