use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

pub fn draw_cometary_escape_tails(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    star_vec: Vec3,
    vel: &SimVelocity,
    comp: &Composition,
    opt_tail: Option<&AtmosphericEscapeTail>,
    opt_rad: Option<&Radius>,
    config: &SimulationConfig,
    elapsed: f32,
) {
    let r_orbit = (body_vec - star_vec).length();

    if comp.ice_frac > 0.35 && r_orbit < 6.0 && r_orbit > 0.15 {
        let tail_dir = (body_vec - star_vec).normalize_or_zero();
        let tail_len = (8.0 / (r_orbit * r_orbit)).clamp(0.2, 3.5) * (comp.ice_frac as f32);

        gizmos.line(
            body_vec,
            body_vec + tail_dir * tail_len,
            Color::srgba(0.3, 0.7, 1.0, 0.65),
        );
        let dust_dir = (tail_dir - vel.0.normalize_or_zero().as_vec3() * 0.3).normalize_or_zero();
        gizmos.line(
            body_vec,
            body_vec + dust_dir * (tail_len * 0.7),
            Color::srgba(0.9, 0.85, 0.6, 0.45),
        );
    }

    if let Some(tail) = opt_tail {
        if tail.is_active && tail.tail_length_au > 0.05 {
            let star_to_body = (body_vec - star_vec).normalize_or_zero();
            if star_to_body != Vec3::ZERO {
                let v_orb = Vec3::new(vel.x as f32, vel.y as f32, vel.z as f32);
                let tail_axis = (star_to_body - v_orb * 0.035).normalize();
                let tail_len = tail.tail_length_au;
                let base_r = opt_rad.map_or(0.04, |r| config.calc_visual_radius(r.0) * 1.15);

                let n_spine = 16;
                let ds = tail_len / (n_spine as f32);
                for s_idx in 0..n_spine {
                    let s1 = (s_idx as f32) * ds;
                    let s2 = ((s_idx + 1) as f32) * ds;
                    let t_frac = (s_idx as f32) / (n_spine as f32);
                    let a_spine = (1.0 - t_frac).powi(2) * 0.90;
                    let p1 = body_vec + tail_axis * s1;
                    let p2 = body_vec + tail_axis * s2;
                    let spine_col = Color::srgba(
                        tail.ion_color.to_srgba().red,
                        tail.ion_color.to_srgba().green,
                        tail.ion_color.to_srgba().blue,
                        a_spine,
                    );
                    gizmos.line(p1, p2, spine_col);
                }

                let ortho1 = if tail_axis.y.abs() < 0.95 {
                    tail_axis.cross(Vec3::Y).normalize()
                } else {
                    tail_axis.cross(Vec3::Z).normalize()
                };
                let ortho2 = tail_axis.cross(ortho1).normalize();

                for rib_i in 0..4 {
                    let rib_ang = (rib_i as f32) * (std::f32::consts::PI * 0.5);
                    let rib_dir = ortho1 * rib_ang.cos() + ortho2 * rib_ang.sin();

                    for s_idx in 0..n_spine {
                        let frac1 = (s_idx as f32) / (n_spine as f32);
                        let frac2 = ((s_idx + 1) as f32) / (n_spine as f32);
                        let s1 = frac1 * tail_len;
                        let s2 = frac2 * tail_len;
                        let w1 = base_r + frac1.sqrt() * (base_r * 2.5 + 0.12 * tail_len.sqrt());
                        let w2 = base_r + frac2.sqrt() * (base_r * 2.5 + 0.12 * tail_len.sqrt());

                        let p1 = body_vec + tail_axis * s1 + rib_dir * w1;
                        let p2 = body_vec + tail_axis * s2 + rib_dir * w2;
                        let a_sheath = (1.0 - frac1) * 0.45;
                        let sheath_col = Color::srgba(
                            tail.ion_color.to_srgba().red,
                            tail.ion_color.to_srgba().green,
                            tail.ion_color.to_srgba().blue,
                            a_sheath,
                        );
                        gizmos.line(p1, p2, sheath_col);
                    }
                }

                for pulse_i in 0..3 {
                    let pulse_phase = ((elapsed * 2.0 + (pulse_i as f32 * 1.33)) % 4.0) / 4.0;
                    let pulse_s = pulse_phase * tail_len;
                    let knot_pos = body_vec + tail_axis * pulse_s;
                    let knot_a = (1.0 - pulse_phase) * 0.8;
                    gizmos.sphere(
                        Isometry3d::from_translation(knot_pos),
                        base_r * (0.6 + 0.4 * pulse_phase),
                        Color::srgba(
                            tail.ion_color.to_srgba().red,
                            tail.ion_color.to_srgba().green,
                            tail.ion_color.to_srgba().blue,
                            knot_a,
                        ),
                    );
                }
            }
        }
    }
}

pub fn draw_planetary_magnetospheres(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    star_vec: Vec3,
    diff: &InternalDifferentiation,
    opt_spin: Option<&SpinState>,
) {
    if diff.magnetic_field_gauss < 0.15 {
        return;
    }
    let tilt_deg = opt_spin.map_or(23.5, |s| s.axial_tilt_degrees as f32);
    let tilt_rot = Quat::from_rotation_z(tilt_deg.to_radians());
    let b_strength = (diff.magnetic_field_gauss as f32).min(3.0);
    let shield_r = 0.35 + b_strength * 0.25;

    for quad in 0..4 {
        let quad_rot = Quat::from_rotation_y(quad as f32 * std::f32::consts::FRAC_PI_2);
        let mut prev_pt: Option<Vec3> = None;
        let steps = 18;
        for step in 0..=steps {
            let theta = (step as f32 / steps as f32) * std::f32::consts::PI;
            let sin_th = theta.sin();
            let r = shield_r * sin_th.powi(2);
            let local_x = r * sin_th;
            let local_y = r * theta.cos();
            let local_pt = quad_rot * Vec3::new(local_x, local_y, 0.0);
            let world_pt = body_vec + tilt_rot * local_pt;

            if let Some(prev) = prev_pt {
                gizmos.line(
                    prev,
                    world_pt,
                    Color::srgba(0.25, 0.75, 1.0, 0.35 * (b_strength / 3.0)),
                );
            }
            prev_pt = Some(world_pt);
        }
    }

    let to_sun = (star_vec - body_vec).normalize_or_zero();
    let perp_tangent = Vec3::new(-to_sun.z, 0.0, to_sun.x).normalize_or_zero();
    let bow_apex = body_vec + to_sun * (shield_r * 1.25);
    let bow_left = body_vec + to_sun * (shield_r * 0.75) + perp_tangent * (shield_r * 1.1);
    let bow_right = body_vec + to_sun * (shield_r * 0.75) - perp_tangent * (shield_r * 1.1);

    gizmos.line(bow_left, bow_apex, Color::srgba(1.0, 0.85, 0.3, 0.55));
    gizmos.line(bow_apex, bow_right, Color::srgba(1.0, 0.85, 0.3, 0.55));
}

pub fn draw_diagnostic_overlays(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    star_vec: Vec3,
    r_orbit: f32,
    mass: &Mass,
    comp: &Composition,
    body: &CelestialBody,
    star_mass_val: f64,
    overlay_mode: DiagnosticOverlayMode,
) {
    match overlay_mode {
        DiagnosticOverlayMode::SpectralComposition => {
            let dominant_color = if comp.metal_frac > 0.4 {
                Color::srgba(1.0, 0.8, 0.2, 0.6)
            } else if comp.ice_frac > 0.4 {
                Color::srgba(0.2, 0.85, 1.0, 0.6)
            } else if comp.gas_frac > 0.4 {
                Color::srgba(0.9, 0.5, 0.1, 0.6)
            } else {
                Color::srgba(0.8, 0.4, 0.2, 0.6)
            };

            gizmos.circle(
                Isometry3d::new(body_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                0.25,
                dominant_color,
            );
        }
        DiagnosticOverlayMode::HillSpheresAndGaps => {
            let is_planet = matches!(
                body.body_type,
                BodyType::Protoplanet
                    | BodyType::TerrestrialPlanet
                    | BodyType::SuperEarth
                    | BodyType::GasGiant
                    | BodyType::IceGiant
            );
            if is_planet {
                let hill_r =
                    (r_orbit * ((mass.0 / (3.0 * star_mass_val)).cbrt() as f32)).clamp(0.08, 2.5);

                gizmos.sphere(
                    Isometry3d::from_translation(body_vec),
                    hill_r,
                    Color::srgba(0.2, 0.9, 0.4, 0.25),
                );

                gizmos.circle(
                    Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    (r_orbit - hill_r).max(0.1),
                    Color::srgba(0.4, 0.8, 0.9, 0.3),
                );
                gizmos.circle(
                    Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    r_orbit + hill_r,
                    Color::srgba(0.4, 0.8, 0.9, 0.3),
                );
            }
        }
        DiagnosticOverlayMode::Realistic => {}
    }
}

pub fn draw_body_selection_and_beacons(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    body: &CelestialBody,
    opt_rad: Option<&Radius>,
    config: &SimulationConfig,
    cam_pos: Option<Vec3>,
) {
    let is_star = body.body_type.is_star_or_remnant();
    let visual_r = opt_rad.map_or(0.005, |r| {
        config.calc_visual_radius_for_type(r.0, body.body_type)
    });

    let cam_dist = cam_pos.map_or(12.0, |cp| cp.distance(body_vec));
    let reticle_r = (visual_r * 1.35).max(cam_dist * 0.038);
    let beacon_h = (visual_r * 2.8).max(cam_dist * 0.09);
    let pointer_r = (visual_r * 0.22).max(cam_dist * 0.009);
    let tick_len = reticle_r * 0.35;

    let reticle_color = if is_star {
        Color::srgb(1.0, 0.85, 0.2)
    } else {
        Color::srgb(0.2, 0.95, 1.0)
    };

    gizmos.circle(
        Isometry3d::new(body_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        reticle_r,
        reticle_color,
    );
    gizmos.circle(
        Isometry3d::new(body_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        reticle_r * 1.25,
        Color::srgba(
            reticle_color.to_srgba().red,
            reticle_color.to_srgba().green,
            reticle_color.to_srgba().blue,
            0.35,
        ),
    );

    gizmos.line(
        body_vec + Vec3::new(reticle_r * 0.95, 0.0, 0.0),
        body_vec + Vec3::new(reticle_r + tick_len, 0.0, 0.0),
        reticle_color,
    );
    gizmos.line(
        body_vec - Vec3::new(reticle_r * 0.95, 0.0, 0.0),
        body_vec - Vec3::new(reticle_r + tick_len, 0.0, 0.0),
        reticle_color,
    );
    gizmos.line(
        body_vec + Vec3::new(0.0, 0.0, reticle_r * 0.95),
        body_vec + Vec3::new(0.0, 0.0, reticle_r + tick_len),
        reticle_color,
    );
    gizmos.line(
        body_vec - Vec3::new(0.0, 0.0, reticle_r * 0.95),
        body_vec - Vec3::new(0.0, 0.0, reticle_r + tick_len),
        reticle_color,
    );

    gizmos.line(
        body_vec,
        body_vec + Vec3::new(0.0, beacon_h, 0.0),
        reticle_color,
    );
    gizmos.sphere(
        Isometry3d::from_translation(body_vec + Vec3::new(0.0, beacon_h, 0.0)),
        pointer_r,
        reticle_color,
    );
}
