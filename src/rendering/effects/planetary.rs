use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

#[allow(
    clippy::too_many_arguments,
    reason = "Atmospheric escape tail rendering requires physical parameters"
)]
pub fn draw_cometary_escape_tails(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    star_vec: Vec3,
    vel: &SimVelocity,
    opt_tail: Option<&AtmosphericEscapeTail>,
    opt_rad: Option<&Radius>,
    config: &SimulationConfig,
    elapsed: f32,
) {
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
        DiagnosticOverlayMode::Realistic
        | DiagnosticOverlayMode::Hidden
        | DiagnosticOverlayMode::InternalCutaway
        | DiagnosticOverlayMode::MagneticFields
        | DiagnosticOverlayMode::DarkMatterHalos => {}
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

/// Draws a high-tech sci-fi holographic targeting reticle around a hovered celestial body,
/// complete with 4 billboarded corner brackets facing the camera and a pulsing inner lock ring.
pub fn draw_hover_targeting_reticle(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    body: &CelestialBody,
    opt_rad: Option<&Radius>,
    config: &SimulationConfig,
    cam_tf: Option<&Transform>,
    elapsed: f32,
) {
    let is_star = body.body_type.is_star_or_remnant();
    let visual_r = opt_rad.map_or(0.005, |r| {
        config.calc_visual_radius_for_type(r.0, body.body_type)
    });

    let cam_pos = cam_tf.map(|t| t.translation);
    let cam_dist = cam_pos.map_or(12.0, |cp| cp.distance(body_vec));

    let box_half = (visual_r * 1.55).max(cam_dist * 0.042);
    let corner_len = box_half * 0.35;
    let pulse = (elapsed * 6.0).sin().mul_add(0.15, 0.85);

    let reticle_color = if is_star {
        Color::srgba(1.0, 0.88, 0.25, 0.90 * pulse)
    } else {
        Color::srgba(0.20, 0.95, 1.0, 0.85 * pulse)
    };

    let ring_color = Color::srgba(
        reticle_color.to_srgba().red,
        reticle_color.to_srgba().green,
        reticle_color.to_srgba().blue,
        0.35 * pulse,
    );

    let cam_rot = cam_tf.map_or(Quat::IDENTITY, |t| t.rotation);
    let right = cam_rot * Vec3::X;
    let up = cam_rot * Vec3::Y;

    // Inner pulsing circular reticle
    gizmos.circle(
        Isometry3d::new(body_vec, cam_rot),
        box_half * 0.88,
        ring_color,
    );

    // Four corner brackets pointing inward toward the body
    // Top-Right: (+box_half, +box_half)
    let tr = body_vec + right * box_half + up * box_half;
    gizmos.line(tr, tr - right * corner_len, reticle_color);
    gizmos.line(tr, tr - up * corner_len, reticle_color);

    // Top-Left: (-box_half, +box_half)
    let tl = body_vec - right * box_half + up * box_half;
    gizmos.line(tl, tl + right * corner_len, reticle_color);
    gizmos.line(tl, tl - up * corner_len, reticle_color);

    // Bottom-Right: (+box_half, -box_half)
    let br = body_vec + right * box_half - up * box_half;
    gizmos.line(br, br - right * corner_len, reticle_color);
    gizmos.line(br, br + up * corner_len, reticle_color);

    // Bottom-Left: (-box_half, -box_half)
    let bl = body_vec - right * box_half - up * box_half;
    gizmos.line(bl, bl + right * corner_len, reticle_color);
    gizmos.line(bl, bl + up * corner_len, reticle_color);
}

/// Internal strata radii and visual colors for 3D cutaway rendering.
#[derive(Debug, Clone, Copy)]
pub struct CutawayStrata {
    pub inner_core_r: f32,
    pub core_r: f32,
    pub mantle_r: f32,
    pub crust_r: f32,
    pub has_theia_llsvp: bool,
    pub inner_core_col: Color,
    pub outer_core_col: Color,
    pub mantle_col: Color,
    pub crust_col: Color,
}

pub fn resolve_cutaway_strata(
    body: &CelestialBody,
    rad_val: f64,
    opt_diff: Option<&InternalDifferentiation>,
    config: &SimulationConfig,
) -> CutawayStrata {
    let visual_r = config.calc_visual_radius_for_type(rad_val, body.body_type);

    if body.body_type.is_star_or_remnant() {
        CutawayStrata {
            inner_core_r: visual_r * 0.12,
            core_r: visual_r * 0.28,
            mantle_r: visual_r * 0.72,
            crust_r: visual_r,
            has_theia_llsvp: false,
            inner_core_col: Color::srgba(1.0, 1.0, 0.95, 0.98),
            outer_core_col: Color::srgba(1.0, 0.75, 0.20, 0.88),
            mantle_col: Color::srgba(1.0, 0.45, 0.08, 0.80),
            crust_col: Color::srgba(0.95, 0.25, 0.05, 0.85),
        }
    } else if body.body_type == BodyType::GasGiant {
        CutawayStrata {
            inner_core_r: visual_r * 0.08,
            core_r: visual_r * 0.20,
            mantle_r: visual_r * 0.76,
            crust_r: visual_r,
            has_theia_llsvp: false,
            inner_core_col: Color::srgba(0.85, 0.75, 0.35, 0.95),
            outer_core_col: Color::srgba(0.35, 0.75, 1.0, 0.90),
            mantle_col: Color::srgba(0.20, 0.55, 0.85, 0.82),
            crust_col: Color::srgba(0.95, 0.65, 0.25, 0.85),
        }
    } else if body.body_type == BodyType::IceGiant {
        CutawayStrata {
            inner_core_r: visual_r * 0.09,
            core_r: visual_r * 0.22,
            mantle_r: visual_r * 0.80,
            crust_r: visual_r,
            has_theia_llsvp: false,
            inner_core_col: Color::srgba(0.70, 0.60, 0.35, 0.95),
            outer_core_col: Color::srgba(0.15, 0.88, 0.95, 0.90),
            mantle_col: Color::srgba(0.12, 0.70, 0.85, 0.82),
            crust_col: Color::srgba(0.35, 0.65, 1.0, 0.85),
        }
    } else {
        let (core_r, mantle_r, has_theia_llsvp) =
            opt_diff.map_or((visual_r * 0.55, visual_r * 0.95, false), |d| {
                let c = if rad_val > 1e-6 {
                    visual_r * (d.core_radius_au / rad_val).clamp(0.15, 0.75) as f32
                } else {
                    visual_r * 0.55
                };
                let m = if rad_val > 1e-6 {
                    visual_r * (d.mantle_radius_au / rad_val).clamp(0.70, 0.98) as f32
                } else {
                    visual_r * 0.95
                };
                (c, m, d.has_theia_llsvp)
            });

        CutawayStrata {
            inner_core_r: core_r * 0.38,
            core_r,
            mantle_r,
            crust_r: visual_r,
            has_theia_llsvp,
            inner_core_col: Color::srgba(1.0, 0.94, 0.55, 0.95),
            outer_core_col: Color::srgba(1.0, 0.52, 0.10, 0.88),
            mantle_col: Color::srgba(0.85, 0.30, 0.12, 0.80),
            crust_col: Color::srgba(0.35, 0.85, 1.0, 0.85),
        }
    }
}

fn draw_cutaway_planar_faces(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    s_axis: Vec3,
    s_tangent_x: Vec3,
    s_tangent_z: Vec3,
    strata: &CutawayStrata,
) {
    let steps = 18;
    for &dir in &[s_tangent_x, s_tangent_z] {
        // Spokes along primary axes
        gizmos.line(body_vec, body_vec + dir * strata.crust_r, strata.crust_col);
        gizmos.line(
            body_vec,
            body_vec + s_axis * strata.crust_r,
            strata.crust_col,
        );
        gizmos.line(
            body_vec,
            body_vec - s_axis * strata.crust_r,
            strata.crust_col,
        );

        // Concentric strata boundary arcs from -Y to +Y pole
        for i in 0..steps {
            let theta1 =
                -std::f32::consts::FRAC_PI_2 + (i as f32 / steps as f32) * std::f32::consts::PI;
            let theta2 = -std::f32::consts::FRAC_PI_2
                + ((i + 1) as f32 / steps as f32) * std::f32::consts::PI;

            // Inner Core Arc
            let p1_ic = body_vec
                + dir * (strata.inner_core_r * theta1.cos())
                + s_axis * (strata.inner_core_r * theta1.sin());
            let p2_ic = body_vec
                + dir * (strata.inner_core_r * theta2.cos())
                + s_axis * (strata.inner_core_r * theta2.sin());
            gizmos.line(p1_ic, p2_ic, strata.inner_core_col);

            // Outer Core Arc
            let p1_oc = body_vec
                + dir * (strata.core_r * theta1.cos())
                + s_axis * (strata.core_r * theta1.sin());
            let p2_oc = body_vec
                + dir * (strata.core_r * theta2.cos())
                + s_axis * (strata.core_r * theta2.sin());
            gizmos.line(p1_oc, p2_oc, strata.outer_core_col);

            // Mantle Arc
            let p1_m = body_vec
                + dir * (strata.mantle_r * theta1.cos())
                + s_axis * (strata.mantle_r * theta1.sin());
            let p2_m = body_vec
                + dir * (strata.mantle_r * theta2.cos())
                + s_axis * (strata.mantle_r * theta2.sin());
            gizmos.line(p1_m, p2_m, strata.mantle_col);

            // Crust Arc
            let p1_c = body_vec
                + dir * (strata.crust_r * theta1.cos())
                + s_axis * (strata.crust_r * theta1.sin());
            let p2_c = body_vec
                + dir * (strata.crust_r * theta2.cos())
                + s_axis * (strata.crust_r * theta2.sin());
            gizmos.line(p1_c, p2_c, strata.crust_col);
        }
    }
}

fn draw_cutaway_spheres_and_plumes(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    s_tangent_x: Vec3,
    strata: &CutawayStrata,
    elapsed: f32,
) {
    let core_pulse = (elapsed * 3.0).sin().mul_add(0.08, 0.92);
    let pulsed_inner_col = Color::srgba(
        strata.inner_core_col.to_srgba().red,
        strata.inner_core_col.to_srgba().green,
        strata.inner_core_col.to_srgba().blue,
        0.95 * core_pulse,
    );

    // 1. Incandescent 3D Solid Inner Core Sphere
    gizmos.sphere(
        Isometry3d::from_translation(body_vec),
        strata.inner_core_r,
        pulsed_inner_col,
    );

    // 2. Liquid Outer Core Concentric Rings
    gizmos.circle(
        Isometry3d::new(body_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        strata.core_r,
        strata.outer_core_col,
    );

    // 3. Mantle Boundary Circle
    gizmos.circle(
        Isometry3d::new(body_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        strata.mantle_r,
        strata.mantle_col,
    );

    // 4. Sunken Theia LLSVP Thermochemical Plumes
    if strata.has_theia_llsvp {
        let blob_r = (strata.core_r * 0.22).clamp(0.002, 0.8);
        let blob_1 = body_vec + s_tangent_x * (strata.core_r * 1.04);
        let blob_2 = body_vec - s_tangent_x * (strata.core_r * 1.04);
        let blob_col = Color::srgba(0.85, 0.15, 0.85, 0.92);

        gizmos.sphere(Isometry3d::from_translation(blob_1), blob_r, blob_col);
        gizmos.sphere(Isometry3d::from_translation(blob_2), blob_r, blob_col);

        // Plume upwelling thermal conduits
        gizmos.line(blob_1, blob_1 + Vec3::Y * (blob_r * 1.8), blob_col);
        gizmos.line(blob_2, blob_2 + Vec3::Y * (blob_r * 1.8), blob_col);
    }
}

fn draw_cutaway_callout_leaders(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    s_tangent_x: Vec3,
    s_tangent_z: Vec3,
    strata: &CutawayStrata,
    cam_tf: Option<&Transform>,
) {
    let cam_rot = cam_tf.map_or(Quat::IDENTITY, |t| t.rotation);
    let right = cam_rot * Vec3::X;
    let up = cam_rot * Vec3::Y;
    let dot_r = (strata.crust_r * 0.035).max(0.001);

    // Diagonal anchor direction into space in screen plane
    let diag_up_right = (right * 0.85 + up * 0.55).normalize();
    let diag_up_left = (-right * 0.85 + up * 0.55).normalize();
    let diag_down_right = (right * 0.85 - up * 0.55).normalize();
    let diag_down_left = (-right * 0.85 - up * 0.55).normalize();

    let offset_scale = strata.crust_r * 1.35;

    // 1. Inner Core Callout
    let anchor_ic = body_vec + diag_down_left * (offset_scale * 0.85);
    gizmos.line(body_vec, anchor_ic, strata.inner_core_col);
    gizmos.circle(
        Isometry3d::new(anchor_ic, cam_rot),
        dot_r,
        strata.inner_core_col,
    );

    // 2. Outer Core Callout
    let pt_oc = body_vec + s_tangent_x * strata.core_r;
    let anchor_oc = body_vec + diag_down_right * (offset_scale * 1.05);
    gizmos.line(pt_oc, anchor_oc, strata.outer_core_col);
    gizmos.circle(
        Isometry3d::new(anchor_oc, cam_rot),
        dot_r,
        strata.outer_core_col,
    );

    // 3. Mantle Callout
    let pt_m = body_vec + (s_tangent_x * 0.707 + s_tangent_z * 0.707) * strata.mantle_r;
    let anchor_m = body_vec + diag_up_right * (offset_scale * 1.25);
    gizmos.line(pt_m, anchor_m, strata.mantle_col);
    gizmos.circle(Isometry3d::new(anchor_m, cam_rot), dot_r, strata.mantle_col);

    // 4. Crust Callout
    let pt_c = body_vec + s_tangent_z * strata.crust_r;
    let anchor_c = body_vec + diag_up_left * (offset_scale * 1.35);
    gizmos.line(pt_c, anchor_c, strata.crust_col);
    gizmos.circle(Isometry3d::new(anchor_c, cam_rot), dot_r, strata.crust_col);

    // 5. Theia LLSVP Callout
    if strata.has_theia_llsvp {
        let pt_blob = body_vec + s_tangent_x * (strata.core_r * 1.04);
        let anchor_blob = body_vec + (right * 1.15) * offset_scale;
        let blob_col = Color::srgba(0.85, 0.15, 0.85, 0.95);
        gizmos.line(pt_blob, anchor_blob, blob_col);
        gizmos.circle(Isometry3d::new(anchor_blob, cam_rot), dot_r, blob_col);
    }
}

/// Renders a full 3D internal planetary / stellar cross-section cutaway view.
/// Shows solid inner core, liquid outer core geodynamo, mantle convection strata,
/// sunken Theia LLSVP piles, thin lithospheric crust, and holographic callout leaders.
#[allow(
    clippy::too_many_arguments,
    reason = "3D internal cutaway rendering requires body differentiation, composition, spin, and camera parameters"
)]
pub fn draw_internal_planetary_cutaway(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    body: &CelestialBody,
    opt_rad: Option<&Radius>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_spin: Option<&SpinState>,
    config: &SimulationConfig,
    cam_tf: Option<&Transform>,
    elapsed: f32,
) {
    let rad_val = opt_rad.map_or(0.005, |r| r.0);
    let strata = resolve_cutaway_strata(body, rad_val, opt_diff, config);

    // Compute 3D spin-aligned orthonormal basis
    let s_axis = if let Some(spin) = opt_spin {
        if spin.spin_vector.length_squared() > 1e-12 {
            let n = spin.spin_vector.normalize();
            Vec3::new(n.x as f32, n.y as f32, n.z as f32)
        } else {
            let tilt = (spin.axial_tilt_degrees as f32).to_radians();
            Vec3::new(tilt.sin(), tilt.cos(), 0.0)
        }
    } else {
        Vec3::Y
    };

    let s_ref = if s_axis.z.abs() > 0.95 {
        Vec3::X
    } else {
        Vec3::Z
    };
    let s_tangent_x = s_axis.cross(s_ref).normalize();
    let s_tangent_z = s_tangent_x.cross(s_axis).normalize();

    draw_cutaway_planar_faces(gizmos, body_vec, s_axis, s_tangent_x, s_tangent_z, &strata);
    draw_cutaway_spheres_and_plumes(gizmos, body_vec, s_tangent_x, &strata, elapsed);
    draw_cutaway_callout_leaders(gizmos, body_vec, s_tangent_x, s_tangent_z, &strata, cam_tf);
}

/// Renders a SpaceEngine / Universe Sandbox style optical starlight glint (Airy disk)
/// when a planet or star is viewed from far away, allowing 3D planet meshes to stay
/// realistically small without disappearing against the background skybox.
#[allow(
    clippy::too_many_arguments,
    reason = "Optical point-spread glint rendering requires camera basis and body parameters"
)]
pub fn draw_distant_optical_glint(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    cam_pos: Vec3,
    cam_up: Vec3,
    cam_right: Vec3,
    visual_radius: f32,
    body_color: Color,
    body_type: BodyType,
) {
    if matches!(
        body_type,
        BodyType::Asteroid
            | BodyType::Comet
            | BodyType::Planetesimal
            | BodyType::DustGrain
            | BodyType::DebrisRing
    ) {
        return;
    }

    let cam_dist = cam_pos.distance(body_vec).max(0.001);
    let angular_size = visual_radius / cam_dist;

    let glint_visibility = ((0.012 - angular_size) / 0.009).clamp(0.0, 1.0);
    if glint_visibility <= 0.01 {
        return;
    }

    let rgba = body_color.to_srgba();
    let glint_r = cam_dist * 0.0028;
    let alpha = glint_visibility * 0.75;

    let core_col = Color::srgba(
        (rgba.red * 0.6 + 0.4).min(1.0),
        (rgba.green * 0.6 + 0.4).min(1.0),
        (rgba.blue * 0.6 + 0.4).min(1.0),
        alpha,
    );
    let halo_col = Color::srgba(rgba.red, rgba.green, rgba.blue, alpha * 0.35);

    gizmos.sphere(
        Isometry3d::from_translation(body_vec),
        glint_r * 0.45,
        core_col,
    );
    gizmos.circle(
        Isometry3d::new(
            body_vec,
            Quat::from_rotation_arc(Vec3::Z, (cam_pos - body_vec).normalize_or_zero()),
        ),
        glint_r * 1.4,
        halo_col,
    );

    let spike_len = glint_r * 2.4;
    let spike_col = Color::srgba(rgba.red, rgba.green, rgba.blue, alpha * 0.45);
    gizmos.line(
        body_vec - cam_right * spike_len,
        body_vec + cam_right * spike_len,
        spike_col,
    );
    gizmos.line(
        body_vec - cam_up * spike_len,
        body_vec + cam_up * spike_len,
        spike_col,
    );
}

pub use super::volcanism::draw_volcanic_and_cryo_plumes;
