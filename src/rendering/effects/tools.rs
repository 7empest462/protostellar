use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::resources::PlayerInteractionState;
use crate::utils::constants::G_ASTRO;
use crate::utils::math::*;

pub fn draw_tractor_beam_gizmo(gizmos: &mut Gizmos, player_state: &PlayerInteractionState) {
    if let Some(t_pos) = player_state.tractor_position {
        let t_vec = Vec3::new(t_pos.x as f32, t_pos.y as f32, t_pos.z as f32);
        gizmos.sphere(
            Isometry3d::from_translation(t_vec),
            0.5,
            Color::srgba(0.9, 0.2, 0.8, 0.7),
        );
    }
}

pub fn draw_planet_builder_preview(
    gizmos: &mut Gizmos,
    builder: &crate::game::ui::PlanetBuilderState,
    star_vec: Vec3,
    star_mass_val: f64,
    elapsed: f32,
) {
    if !builder.is_open {
        return;
    }

    let a = builder.semi_major_axis_au;
    let e = builder.eccentricity;
    let pulse = 0.65 + 0.35 * (elapsed * 3.5).sin().abs();

    let preview_elements = OrbitalElements {
        semi_major_axis: a,
        eccentricity: e,
        inclination: 0.0,
        longitude_ascending_node: 0.0,
        argument_of_periapsis: 0.0,
        true_anomaly: f64::from(elapsed * 0.8) % (2.0 * std::f64::consts::PI),
        periapsis: if a > 0.0 { a * (1.0 - e) } else { 1.0 },
        apoapsis: if a > 0.0 && e < 1.0 {
            a * (1.0 + e)
        } else {
            f64::INFINITY
        },
        period_years: if a > 0.0 {
            (a.powi(3) / star_mass_val.max(0.1)).sqrt()
        } else {
            1.0
        },
        specific_energy: -G_ASTRO * star_mass_val / (2.0 * a.max(0.01)),
        periapsis_dir: DVec3::new(1.0, 0.0, 0.0),
        semilatus_dir: DVec3::new(0.0, 0.0, 1.0),
    };

    let preview_points = generate_orbit_points(&preview_elements, 96);
    if preview_points.len() > 1 {
        let prev_col = Color::srgba(0.20, 0.95, 1.0, 0.85 * pulse);
        for window in preview_points.windows(2) {
            if let [p0, p1] = window {
                gizmos.line(*p0 + star_vec, *p1 + star_vec, prev_col);
            }
        }
    }

    if let Some(ghost_pt) =
        position_at_true_anomaly(&preview_elements, preview_elements.true_anomaly)
    {
        let ghost_world = ghost_pt + star_vec;
        gizmos.sphere(
            Isometry3d::from_translation(ghost_world),
            0.06 * pulse,
            Color::srgba(1.0, 0.85, 0.25, 0.95),
        );
        gizmos.circle(
            Isometry3d::new(
                ghost_world,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            0.12 * pulse,
            Color::srgba(0.3, 0.95, 1.0, 0.70),
        );
    }

    let (opt_peri, opt_apo) = apsides_positions(&preview_elements);
    if let Some(peri) = opt_peri {
        gizmos.sphere(
            Isometry3d::from_translation(peri + star_vec),
            0.035,
            Color::srgba(0.2, 1.0, 0.5, 0.9),
        );
    }
    if let Some(apo) = opt_apo {
        gizmos.sphere(
            Isometry3d::from_translation(apo + star_vec),
            0.035,
            Color::srgba(1.0, 0.5, 0.2, 0.9),
        );
    }
}

/// Renders interactive slingshot aim vector, arrowhead, predicted Keplerian orbit ribbon, and apsides.
pub fn draw_slingshot_preview(
    gizmos: &mut Gizmos,
    slingshot: &crate::simulation::resources::SlingshotState,
    star_vec: Vec3,
    star_mass_val: f64,
    elapsed: f32,
) {
    if !slingshot.is_active {
        return;
    }

    let (Some(origin_dvec), Some(curr_dvec)) = (slingshot.drag_origin, slingshot.drag_current)
    else {
        return;
    };

    let p_orig = Vec3::new(
        origin_dvec.x as f32,
        origin_dvec.y as f32,
        origin_dvec.z as f32,
    );
    let p_curr = Vec3::new(curr_dvec.x as f32, curr_dvec.y as f32, curr_dvec.z as f32);
    let delta = curr_dvec - origin_dvec;
    let dist = delta.length();

    let pulse = 0.70 + 0.30 * (elapsed * 4.0).sin().abs();

    // 1. Launch origin circle marker
    gizmos.circle(
        Isometry3d::new(p_orig, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        0.18 * pulse,
        Color::srgba(1.0, 0.75, 0.20, 0.90),
    );
    gizmos.sphere(
        Isometry3d::from_translation(p_orig),
        0.05 * pulse,
        Color::srgba(1.0, 0.90, 0.30, 0.95),
    );

    // 2. Drag vector line and arrow head
    if dist >= 0.02 {
        let arrow_col = Color::srgba(1.0, 0.45, 0.15, 0.95);
        gizmos.line(p_orig, p_curr, arrow_col);

        // Arrowhead
        let dir = (p_curr - p_orig).normalize_or_zero();
        if dir.length_squared() > 0.5 {
            let right = Vec3::Y.cross(dir).normalize_or_zero();
            let head_len = (dist as f32 * 0.25).clamp(0.08, 0.45);
            let arrow_left = p_curr - dir * head_len + right * (head_len * 0.5);
            let arrow_right = p_curr - dir * head_len - right * (head_len * 0.5);
            gizmos.line(p_curr, arrow_left, arrow_col);
            gizmos.line(p_curr, arrow_right, arrow_col);
        }

        // 3. Real-Time Keplerian Orbit Forecast
        let launch_vel = delta * slingshot.velocity_scale;
        let elements_opt =
            state_vectors_to_orbital_elements(origin_dvec, launch_vel, star_mass_val, 1e-6);

        if let Some(el) = elements_opt {
            if el.eccentricity < 1.0 && el.semi_major_axis > 0.0 {
                // Elliptical bound orbit path
                let pts = generate_orbit_points(&el, 96);
                if pts.len() > 1 {
                    let orbit_col = Color::srgba(1.0, 0.82, 0.20, 0.85 * pulse);
                    for window in pts.windows(2) {
                        if let [p0, p1] = window {
                            gizmos.line(*p0 + star_vec, *p1 + star_vec, orbit_col);
                        }
                    }
                }

                // Periapsis & Apoapsis indicators
                let (opt_peri, opt_apo) = apsides_positions(&el);
                if let Some(peri) = opt_peri {
                    let peri_world = peri + star_vec;
                    gizmos.sphere(
                        Isometry3d::from_translation(peri_world),
                        0.045,
                        Color::srgba(0.2, 1.0, 0.4, 0.95),
                    );
                    gizmos.circle(
                        Isometry3d::new(
                            peri_world,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        0.09,
                        Color::srgba(0.2, 1.0, 0.4, 0.70),
                    );
                }
                if let Some(apo) = opt_apo {
                    let apo_world = apo + star_vec;
                    gizmos.sphere(
                        Isometry3d::from_translation(apo_world),
                        0.045,
                        Color::srgba(1.0, 0.3, 0.2, 0.95),
                    );
                    gizmos.circle(
                        Isometry3d::new(
                            apo_world,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        0.09,
                        Color::srgba(1.0, 0.3, 0.2, 0.70),
                    );
                }
            } else {
                // Hyperbolic escape trajectory
                let hyp_points = generate_hyperbolic_orbit_points(&el, 64);
                if hyp_points.len() > 1 {
                    let hyp_col = Color::srgba(0.85, 0.35, 1.0, 0.85 * pulse);
                    for window in hyp_points.windows(2) {
                        if let [p0, p1] = window {
                            gizmos.line(*p0 + star_vec, *p1 + star_vec, hyp_col);
                        }
                    }
                }
            }
        }
    }
}
