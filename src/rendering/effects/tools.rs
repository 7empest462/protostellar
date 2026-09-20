use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::resources::{PlayerInteractionState, SlingshotArchetype};
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

    let star_pos_dvec = star_vec.as_dvec3();
    let rel_pos = origin_dvec - star_pos_dvec;
    let r_launch = rel_pos.length();

    let pulse = 0.70 + 0.30 * (elapsed * 4.0).sin().abs();

    // 1. Launch origin archetype preview and aiming reticle
    let (archetype_col, ghost_rad) = match slingshot.archetype {
        SlingshotArchetype::Asteroid => (Color::srgb(0.58, 0.54, 0.50), 0.045),
        SlingshotArchetype::Comet => (Color::srgb(0.35, 0.85, 0.95), 0.045),
        SlingshotArchetype::TerrestrialPlanet => (Color::srgb(0.25, 0.75, 0.45), 0.065),
        SlingshotArchetype::WaterWorld => (Color::srgb(0.18, 0.50, 0.92), 0.075),
        SlingshotArchetype::GasGiant => (Color::srgb(0.92, 0.65, 0.28), 0.120),
        SlingshotArchetype::RoguePlanet => (Color::srgb(0.42, 0.35, 0.52), 0.085),
    };

    gizmos.circle(
        Isometry3d::new(p_orig, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        0.20 * pulse,
        archetype_col.with_alpha(0.85),
    );
    gizmos.circle(
        Isometry3d::new(p_orig, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        0.08,
        Color::srgba(1.0, 1.0, 1.0, 0.60),
    );
    gizmos.sphere(
        Isometry3d::from_translation(p_orig),
        ghost_rad * pulse,
        archetype_col.with_alpha(0.95),
    );

    // 2. Launch orbital altitude reference ring
    if r_launch > 0.05 {
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r_launch as f32,
            Color::srgba(0.35, 0.55, 0.75, 0.20),
        );
    }

    // 3. Drag vector line, arrowhead, speed markers, and orbit forecast
    if dist >= 0.02 {
        let arrow_col = Color::srgba(1.0, 0.55, 0.15, 0.95);
        gizmos.line(p_orig, p_curr, arrow_col);

        let dir = (p_curr - p_orig).normalize_or_zero();
        if dir.length_squared() > 0.5 {
            let right = Vec3::Y.cross(dir).normalize_or_zero();
            let head_len = (dist as f32 * 0.25).clamp(0.08, 0.35);
            let arrow_left = p_curr - dir * head_len + right * (head_len * 0.5);
            let arrow_right = p_curr - dir * head_len - right * (head_len * 0.5);
            gizmos.line(p_curr, arrow_left, arrow_col);
            gizmos.line(p_curr, arrow_right, arrow_col);

            // Circular and escape velocity drag markers along the aim vector
            if r_launch > 0.01 {
                let v_circ = (G_ASTRO * star_mass_val / r_launch).sqrt();
                let d_circ = (v_circ / slingshot.velocity_scale) as f32;
                let d_esc = d_circ * std::f32::consts::SQRT_2;

                // Circular speed indicator (Emerald Green ring & cross-tick)
                let circ_pt = p_orig + dir * d_circ;
                gizmos.circle(
                    Isometry3d::new(circ_pt, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    0.07 * pulse,
                    Color::srgba(0.20, 1.0, 0.50, 0.95),
                );
                gizmos.line(
                    circ_pt - right * 0.06,
                    circ_pt + right * 0.06,
                    Color::srgba(0.20, 1.0, 0.50, 0.95),
                );

                // Escape speed indicator (Electric Purple ring)
                let esc_pt = p_orig + dir * d_esc;
                gizmos.circle(
                    Isometry3d::new(esc_pt, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    0.06 * pulse,
                    Color::srgba(0.85, 0.30, 1.0, 0.85),
                );
                gizmos.line(
                    esc_pt - right * 0.05,
                    esc_pt + right * 0.05,
                    Color::srgba(0.85, 0.30, 1.0, 0.85),
                );

                // Graduated speed tick marks every 10 km/s (~2.1095 AU/yr)
                let d_tick = (2.1095 / slingshot.velocity_scale) as f32;
                for k in 1..=10 {
                    let tick_dist = k as f32 * d_tick;
                    if tick_dist <= dist as f32 * 1.3 {
                        let t_pt = p_orig + dir * tick_dist;
                        let t_len = if k % 5 == 0 { 0.045 } else { 0.022 };
                        let t_col = if k % 5 == 0 {
                            Color::srgba(1.0, 0.90, 0.40, 0.85)
                        } else {
                            Color::srgba(1.0, 0.80, 0.30, 0.50)
                        };
                        gizmos.line(t_pt - right * t_len, t_pt + right * t_len, t_col);
                    }
                }
            }
        }

        // 4. Real-Time Keplerian Orbit Forecast with Directional Chevrons
        let launch_vel = delta * slingshot.velocity_scale;
        let elements_opt =
            state_vectors_to_orbital_elements(rel_pos, launch_vel, star_mass_val, 1e-6);

        if let Some(el) = elements_opt {
            if el.eccentricity < 1.0 && el.semi_major_axis > 0.0 {
                let pts = generate_orbit_points(&el, 96);
                if pts.len() > 1 {
                    let is_collision = el.periapsis <= 0.006;
                    let orbit_col = if is_collision {
                        Color::srgba(1.0, 0.20, 0.20, 0.95 * pulse)
                    } else if el.eccentricity < 0.15 {
                        Color::srgba(0.25, 0.95, 1.0, 0.90 * pulse)
                    } else {
                        Color::srgba(1.0, 0.82, 0.20, 0.85 * pulse)
                    };

                    for window in pts.windows(2) {
                        if let [p0, p1] = window {
                            gizmos.line(*p0 + star_vec, *p1 + star_vec, orbit_col);
                        }
                    }

                    // Directional chevrons along the orbital ellipse indicating direction of travel
                    for step in (8..pts.len()).step_by(16) {
                        if let (Some(&p_prev), Some(&p_curr)) = (pts.get(step - 1), pts.get(step)) {
                            let tangent = (p_curr - p_prev).normalize_or_zero();
                            if tangent.length_squared() > 0.5 {
                                let norm = Vec3::Y.cross(tangent).normalize_or_zero();
                                let tip = p_curr + star_vec;
                                let chev_len = 0.07;
                                let left = tip - tangent * chev_len + norm * (chev_len * 0.5);
                                let right = tip - tangent * chev_len - norm * (chev_len * 0.5);
                                gizmos.line(tip, left, orbit_col);
                                gizmos.line(tip, right, orbit_col);
                            }
                        }
                    }

                    // Stellar impact warning
                    if is_collision {
                        let (opt_peri, _) = apsides_positions(&el);
                        if let Some(peri) = opt_peri {
                            let impact_world = peri + star_vec;
                            gizmos.sphere(
                                Isometry3d::from_translation(impact_world),
                                0.06 * pulse,
                                Color::srgba(1.0, 0.1, 0.1, 0.95),
                            );
                            gizmos.circle(
                                Isometry3d::new(
                                    impact_world,
                                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                                ),
                                0.14 * pulse,
                                Color::srgba(1.0, 0.1, 0.1, 0.90),
                            );
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

                    // Directional chevrons along escape trajectory
                    for step in (6..hyp_points.len()).step_by(12) {
                        if let (Some(&p_prev), Some(&p_curr)) =
                            (hyp_points.get(step - 1), hyp_points.get(step))
                        {
                            let tangent = (p_curr - p_prev).normalize_or_zero();
                            if tangent.length_squared() > 0.5 {
                                let norm = Vec3::Y.cross(tangent).normalize_or_zero();
                                let tip = p_curr + star_vec;
                                let chev_len = 0.08;
                                let left = tip - tangent * chev_len + norm * (chev_len * 0.5);
                                let right = tip - tangent * chev_len - norm * (chev_len * 0.5);
                                gizmos.line(tip, left, hyp_col);
                                gizmos.line(tip, right, hyp_col);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Renders real-time predicted trajectory spline, close encounter reticles, and impact warning targets.
pub fn draw_trajectory_prediction_gizmos(
    gizmos: &mut Gizmos,
    predictor: &crate::simulation::predictor::TrajectoryPredictorState,
    star_vec: Vec3,
    elapsed: f32,
) {
    if !predictor.is_enabled || predictor.trajectory_points.len() < 2 {
        return;
    }

    let pulse = 0.70 + 0.30 * (elapsed * 4.0).sin().abs();

    // 1. Determine spline color based on active encounter severity
    let spline_color = if let Some(enc) = &predictor.active_encounter {
        match enc.encounter_type {
            crate::simulation::predictor::EncounterType::DirectImpact => {
                Color::srgba(1.0, 0.15, 0.20, 0.95 * pulse)
            }
            crate::simulation::predictor::EncounterType::RocheLobeCrossing => {
                Color::srgba(1.0, 0.50, 0.15, 0.90 * pulse)
            }
            crate::simulation::predictor::EncounterType::HillSpherePenetration => {
                Color::srgba(1.0, 0.85, 0.25, 0.85 * pulse)
            }
            crate::simulation::predictor::EncounterType::SafeFlyby => {
                Color::srgba(0.20, 0.90, 0.65, 0.80)
            }
        }
    } else {
        Color::srgba(0.35, 0.75, 1.0, 0.65)
    };

    // 2. Draw forward trajectory spline
    for window in predictor.trajectory_points.windows(2) {
        if let [p0, p1] = window {
            gizmos.line(*p0 + star_vec, *p1 + star_vec, spline_color);
        }
    }

    // 3. Render Close Encounter / Impact Reticle
    if let Some(enc) = &predictor.active_encounter {
        let enc_world = Vec3::new(
            enc.encounter_pos_au.x as f32,
            enc.encounter_pos_au.y as f32,
            enc.encounter_pos_au.z as f32,
        ) + star_vec;

        let tgt_world = Vec3::new(
            enc.target_pos_at_encounter_au.x as f32,
            enc.target_pos_at_encounter_au.y as f32,
            enc.target_pos_at_encounter_au.z as f32,
        ) + star_vec;

        let badge_color = enc.encounter_type.badge_color();
        let badge_rgba = badge_color.to_srgba();

        // Baseline connector between projectile/body and target center at closest approach
        gizmos.line(
            enc_world,
            tgt_world,
            Color::srgba(badge_rgba.red, badge_rgba.green, badge_rgba.blue, 0.55),
        );

        // Reticle radius based on encounter type
        let reticle_r = match enc.encounter_type {
            crate::simulation::predictor::EncounterType::DirectImpact => 0.12 * pulse,
            crate::simulation::predictor::EncounterType::RocheLobeCrossing => 0.16 * pulse,
            crate::simulation::predictor::EncounterType::HillSpherePenetration => 0.20,
            crate::simulation::predictor::EncounterType::SafeFlyby => 0.15,
        };

        gizmos.circle(
            Isometry3d::new(
                enc_world,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            reticle_r,
            Color::srgba(badge_rgba.red, badge_rgba.green, badge_rgba.blue, 0.90),
        );

        // Crosshairs at encounter point
        let cross_len = reticle_r * 1.6;
        gizmos.line(
            enc_world - Vec3::X * cross_len,
            enc_world + Vec3::X * cross_len,
            Color::srgba(badge_rgba.red, badge_rgba.green, badge_rgba.blue, 0.85),
        );
        gizmos.line(
            enc_world - Vec3::Z * cross_len,
            enc_world + Vec3::Z * cross_len,
            Color::srgba(badge_rgba.red, badge_rgba.green, badge_rgba.blue, 0.85),
        );

        // Center beacon sphere
        gizmos.sphere(
            Isometry3d::from_translation(enc_world),
            0.04 * pulse,
            Color::srgba(badge_rgba.red, badge_rgba.green, badge_rgba.blue, 0.95),
        );

        // If penetrating Hill sphere or crossing Roche, visualize target boundary
        if enc.encounter_type != crate::simulation::predictor::EncounterType::SafeFlyby {
            let hill_r = (enc.target_hill_radius_au as f32).clamp(0.08, 2.5);
            gizmos.circle(
                Isometry3d::new(
                    tgt_world,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                hill_r,
                Color::srgba(1.0, 0.85, 0.20, 0.35 * pulse),
            );
        }
    }
}

pub fn draw_bombardment_projectiles_gizmo(
    gizmos: &mut Gizmos,
    projectiles_query: &Query<(
        &crate::simulation::components::SimPosition,
        &crate::simulation::terraforming::BombardmentProjectile,
    )>,
    targets_query: &Query<(
        &crate::simulation::components::SimPosition,
        &crate::simulation::components::Radius,
    )>,
    elapsed: f32,
) {
    let pulse = 0.65 + 0.35 * (elapsed * 8.0).sin().abs();

    for (p_pos, proj) in projectiles_query.iter() {
        if proj.is_detonated {
            continue;
        }

        let Ok((t_pos, t_rad)) = targets_query.get(proj.target_entity) else {
            continue;
        };

        let p_vec = Vec3::new(p_pos.0.x as f32, p_pos.0.y as f32, p_pos.0.z as f32);
        let t_vec = Vec3::new(t_pos.0.x as f32, t_pos.0.y as f32, t_pos.0.z as f32);
        let trail_rgba = proj.trail_color.to_srgba();

        // 1. Inbound trajectory intercept spline
        let line_col = Color::srgba(
            trail_rgba.red,
            trail_rgba.green,
            trail_rgba.blue,
            0.85 * pulse,
        );
        gizmos.line(p_vec, t_vec, line_col);

        // 2. Projectile leading beacon sphere
        gizmos.sphere(
            Isometry3d::from_translation(p_vec),
            0.05 * pulse,
            Color::srgba(trail_rgba.red, trail_rgba.green, trail_rgba.blue, 0.95),
        );

        // 3. Targeting corridor rings around the planet's atmospheric boundary
        let target_reticle_r = (t_rad.0 as f32 * 1.5).clamp(0.06, 1.2);
        gizmos.circle(
            Isometry3d::new(t_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            target_reticle_r * pulse,
            Color::srgba(trail_rgba.red, trail_rgba.green, trail_rgba.blue, 0.70),
        );

        // Ground target crosshairs
        let cross_len = target_reticle_r * 1.4;
        gizmos.line(
            t_vec - Vec3::X * cross_len,
            t_vec + Vec3::X * cross_len,
            Color::srgba(trail_rgba.red, trail_rgba.green, trail_rgba.blue, 0.65),
        );
        gizmos.line(
            t_vec - Vec3::Z * cross_len,
            t_vec + Vec3::Z * cross_len,
            Color::srgba(trail_rgba.red, trail_rgba.green, trail_rgba.blue, 0.65),
        );
    }
}
