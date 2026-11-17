//! 3D Eruptive Plume and Cryovolcanic Geyser Rendering.
//!
//! Renders supersonic sulfur umbrella plumes and ballistic fallout canopies on Io-like worlds,
//! as well as forward-scattering water-ice geysers venting from polar fissures on Enceladus and Europa.

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::orbits::{draw_occluded_circle, sphere_occlusion_fade};
use crate::simulation::components::volcanism::{VolcanicActivityType, VolcanicState};
use crate::simulation::components::SpinState;

/// Prominent active volcanic eruption centers on Io (Pele, Loki, Prometheus, Tvashtar).
pub const IO_VENT_NORMALS: [Vec3; 4] = [
    Vec3::new(0.32, -0.32, 0.89),   // Pele (-18°S, 105°W)
    Vec3::new(-0.84, 0.22, 0.49),   // Loki Patera (+13°N, 148°W)
    Vec3::new(-0.25, -0.05, -0.96), // Prometheus (-1.5°S, 206°W)
    Vec3::new(0.42, 0.88, 0.22),    // Tvashtar (+62°N, 237°W)
];

/// Prominent south polar "Tiger Stripe" active cryo-geyser sulci on Enceladus.
pub const ENCELADUS_VENT_NORMALS: [Vec3; 4] = [
    Vec3::new(-0.18, -0.96, 0.20),  // Damascus Sulcus
    Vec3::new(-0.06, -0.98, -0.19), // Baghdad Sulcus
    Vec3::new(0.12, -0.97, 0.21),   // Alexandria Sulcus
    Vec3::new(0.24, -0.95, -0.18),  // Cairo Sulcus
];

/// Draws expanding supersonic ballistic umbrella plumes and fallout halos for sulfur volcanism.
#[allow(
    clippy::too_many_arguments,
    reason = "Umbrella plume rendering needs body geometry, state, rotation, camera occlusion, and phase"
)]
fn draw_io_umbrella_plumes(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    visual_radius: f32,
    volc: &VolcanicState,
    rot: Quat,
    num_plumes: usize,
    cam_pos: Vec3,
    elapsed: f32,
) {
    let is_sulfur = volc.activity_type == VolcanicActivityType::IoSulfurVolcanism;
    let h_vis = visual_radius * (volc.max_plume_height_km / 1821.0).clamp(0.10, 0.42);
    let fallout_radius = h_vis * 0.65;
    let cam_opt = Some(cam_pos);

    for (i, &local_n) in IO_VENT_NORMALS.iter().enumerate().take(num_plumes) {
        let world_n: Vec3 = (rot * local_n).normalize_or_zero();
        if world_n == Vec3::ZERO {
            continue;
        }
        let vent_pos = body_vec + world_n * visual_radius;
        let stem_top = vent_pos + world_n * h_vis;

        // 1. Graduated incandescent central volcanic throat jet (fading toward the canopy apex)
        let pulse = 0.72 + 0.20 * (elapsed * 4.5 + (i as f32) * 1.7).sin();
        let stem_segs = 8usize;
        for s in 0..stem_segs {
            let t0 = s as f32 / stem_segs as f32;
            let t1 = (s + 1) as f32 / stem_segs as f32;
            let s0 = vent_pos.lerp(stem_top, t0);
            let s1 = vent_pos.lerp(stem_top, t1);
            let occ = sphere_occlusion_fade(s0.midpoint(s1), cam_opt, body_vec, visual_radius);
            let a = (1.0 - t0 * 0.55) * pulse * occ;
            if a > 0.02 {
                let col = if is_sulfur {
                    Color::srgba(1.0, 0.68, 0.14, a)
                } else {
                    Color::srgba(1.0, 0.36, 0.08, a)
                };
                gizmos.line(s0, s1, col);
            }
        }

        let ortho1 = if world_n.y.abs() < 0.95 {
            world_n.cross(Vec3::Y).normalize()
        } else {
            world_n.cross(Vec3::Z).normalize()
        };
        let ortho2 = world_n.cross(ortho1).normalize();

        // 2. Smooth parabolic umbrella ballistic canopy ribs + tangent micro-streaks
        let n_ribs = 10usize;
        let n_segs = 14usize;
        for rib_j in 0..n_ribs {
            let theta = (rib_j as f32) * (TAU / (n_ribs as f32)) + elapsed * 0.14;
            let r_dir = ortho1 * theta.cos() + ortho2 * theta.sin();

            for s in 0..n_segs {
                let u0 = (s as f32) / (n_segs as f32);
                let u1 = ((s + 1) as f32) / (n_segs as f32);

                let z0 = h_vis * (1.0 - 0.96 * u0 * u0);
                let r0 = fallout_radius * (u0 * FRAC_PI_2).sin();
                let p0 = vent_pos + world_n * z0 + r_dir * r0;

                let z1 = h_vis * (1.0 - 0.96 * u1 * u1);
                let r1 = fallout_radius * (u1 * FRAC_PI_2).sin();
                let p1 = vent_pos + world_n * z1 + r_dir * r1;

                let occ = sphere_occlusion_fade(p0.midpoint(p1), cam_opt, body_vec, visual_radius);
                let alpha = (1.0 - u0).powf(1.25) * 0.58 * occ;
                if alpha > 0.02 {
                    let rib_col = if is_sulfur {
                        Color::srgba(1.0, 0.80, 0.24, alpha)
                    } else {
                        Color::srgba(0.88, 0.44, 0.14, alpha)
                    };
                    gizmos.line(p0, p1, rib_col);

                    let droplet_t = (elapsed * 0.8 + (rib_j as f32 * 0.3) + u0) % 1.0;
                    if (droplet_t - u0).abs() < 0.08 {
                        let d_pos = p0.lerp(p1, (droplet_t - u0) / 0.08);
                        let dir = (p1 - p0).normalize_or_zero();
                        let streak = dir * (visual_radius * 0.028);
                        gizmos.line(
                            d_pos - streak,
                            d_pos + streak,
                            Color::srgba(1.0, 0.92, 0.42, 0.80 * occ),
                        );
                    }
                }
            }
        }

        // 3. Smooth high-res surface caldera & fallout concentric halos
        let ring_rot = Quat::from_rotation_arc(Vec3::Z, world_n);
        let ring_center = vent_pos + world_n * (visual_radius * 0.002);
        let (halo_col, inner_col) = if is_sulfur {
            (
                Color::srgba(0.92, 0.38, 0.08, 0.38),
                Color::srgba(0.88, 0.78, 0.18, 0.32),
            )
        } else {
            (
                Color::srgba(0.45, 0.28, 0.15, 0.32),
                Color::srgba(0.68, 0.36, 0.12, 0.26),
            )
        };
        let occ = (Some((body_vec, visual_radius)), None);
        draw_occluded_circle(
            gizmos,
            ring_center,
            ring_rot,
            fallout_radius,
            halo_col,
            64,
            cam_opt,
            occ,
        );
        draw_occluded_circle(
            gizmos,
            ring_center,
            ring_rot,
            fallout_radius * 0.62,
            inner_col,
            48,
            cam_opt,
            occ,
        );
    }
}

/// Draws high-velocity columnar water-ice geysers with forward-scattering Mie optics.
fn draw_cryo_geyser_jets(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    visual_radius: f32,
    volc: &VolcanicState,
    rot: Quat,
    star_vec: Vec3,
    cam_pos: Vec3,
    num_plumes: usize,
) {
    let is_water = volc.activity_type == VolcanicActivityType::CryoWaterIce;
    let h_vis = visual_radius * (volc.max_plume_height_km / 252.0).clamp(0.40, 2.2);

    let star_dir = (star_vec - body_vec).normalize_or_zero();
    let view_dir = (cam_pos - body_vec).normalize_or_zero();
    let cos_phase = (-star_dir).dot(view_dir);
    // Strong forward Mie scattering peak when backlit by the host star
    let mie_boost = 1.0 + 3.2 * cos_phase.max(0.0).powi(3);

    for &local_n in ENCELADUS_VENT_NORMALS.iter().take(num_plumes) {
        let world_n: Vec3 = (rot * local_n).normalize_or_zero();
        if world_n == Vec3::ZERO {
            continue;
        }
        let vent_pos = body_vec + world_n * visual_radius;
        let jet_top = vent_pos + world_n * h_vis;

        let ortho1 = if world_n.y.abs() < 0.95 {
            world_n.cross(Vec3::Y).normalize()
        } else {
            world_n.cross(Vec3::Z).normalize()
        };
        let ortho2 = world_n.cross(ortho1).normalize();

        // 1. Central high-density supersonic spine
        let spine_alpha = (0.55 * mie_boost).min(0.95);
        let spine_col = if is_water {
            Color::srgba(0.70, 0.90, 1.0, spine_alpha)
        } else {
            Color::srgba(0.85, 0.80, 0.45, spine_alpha)
        };
        gizmos.line(vent_pos, jet_top, spine_col);

        // 2. Conical vapor sheath
        let w_top = visual_radius * 0.24;
        for k in 0..4 {
            let angle = (k as f32) * FRAC_PI_2;
            let k_dir = ortho1 * angle.cos() + ortho2 * angle.sin();
            let sheath_top = jet_top + k_dir * w_top;
            let sheath_col = if is_water {
                Color::srgba(0.55, 0.85, 1.0, (0.35 * mie_boost).min(0.70))
            } else {
                Color::srgba(0.75, 0.70, 0.35, (0.35 * mie_boost).min(0.70))
            };
            gizmos.line(vent_pos, sheath_top, sheath_col);
        }

        // 3. Diffuse expanding vapor cap
        let cap_col = if is_water {
            Color::srgba(0.65, 0.92, 1.0, (0.30 * mie_boost).min(0.60))
        } else {
            Color::srgba(0.80, 0.75, 0.40, (0.30 * mie_boost).min(0.60))
        };
        let cap_rot = Quat::from_rotation_arc(Vec3::Z, (cam_pos - jet_top).normalize_or_zero());
        gizmos
            .circle(Isometry3d::new(jet_top, cap_rot), w_top * 0.85, cap_col)
            .resolution(48);

        // 4. Orbital escape streamers feeding the parent planet's E-Ring
        if volc.cryo_ring_feed_rate_kg_s > 0.0 {
            let escape_vec = (world_n * 1.5 + ortho1 * 0.4).normalize();
            let stream_end = jet_top + escape_vec * (visual_radius * 1.8);
            let stream_col = Color::srgba(0.60, 0.85, 1.0, (0.22 * mie_boost).min(0.50));
            gizmos.line(jet_top, stream_end, stream_col);
        }
    }
}

/// Renders 3D volcanic umbrella plumes or cryovolcanic geyser jets based on body activity type.
#[allow(
    clippy::too_many_arguments,
    reason = "Volcanic and cryo plume rendering requires physical and viewpoint parameters"
)]
pub fn draw_volcanic_and_cryo_plumes(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    visual_radius: f32,
    volc: &VolcanicState,
    opt_spin: Option<&SpinState>,
    star_vec: Vec3,
    cam_pos: Vec3,
    elapsed: f32,
) {
    if volc.active_plumes == 0 {
        return;
    }

    let rot: Quat = if let Some(spin) = opt_spin {
        let spin_dir = if spin.spin_vector.length_squared() > 1e-12 {
            spin.spin_vector.normalize().as_vec3()
        } else {
            Vec3::Y
        };
        let period_s = (spin.rotation_period_hours * 3600.0).max(1.0) as f32;
        let angle = (elapsed * 2.0 * PI / period_s) % TAU;
        Quat::from_axis_angle(spin_dir, angle)
    } else {
        Quat::IDENTITY
    };

    let num_plumes = (volc.active_plumes as usize).min(4);

    match volc.activity_type {
        VolcanicActivityType::IoSulfurVolcanism | VolcanicActivityType::SilicateBasaltic => {
            draw_io_umbrella_plumes(
                gizmos,
                body_vec,
                visual_radius,
                volc,
                rot,
                num_plumes,
                cam_pos,
                elapsed,
            );
        }
        VolcanicActivityType::CryoWaterIce | VolcanicActivityType::CryoHydrocarbon => {
            draw_cryo_geyser_jets(
                gizmos,
                body_vec,
                visual_radius,
                volc,
                rot,
                star_vec,
                cam_pos,
                num_plumes,
            );
        }
        VolcanicActivityType::Dormant => {}
    }
}
