//! 3D Eruptive Plume and Cryovolcanic Geyser Rendering.
//!
//! Renders supersonic sulfur umbrella plumes and ballistic fallout canopies on Io-like worlds,
//! as well as forward-scattering water-ice geysers venting from polar fissures on Enceladus and Europa.

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

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
fn draw_io_umbrella_plumes(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    visual_radius: f32,
    volc: &VolcanicState,
    rot: Quat,
    num_plumes: usize,
    elapsed: f32,
) {
    let is_sulfur = volc.activity_type == VolcanicActivityType::IoSulfurVolcanism;
    let h_vis = visual_radius * (volc.max_plume_height_km / 1821.0).clamp(0.12, 0.48);
    let fallout_radius = h_vis * 0.65;

    for (i, &local_n) in IO_VENT_NORMALS.iter().enumerate().take(num_plumes) {
        let world_n: Vec3 = (rot * local_n).normalize_or_zero();
        if world_n == Vec3::ZERO {
            continue;
        }
        let vent_pos = body_vec + world_n * visual_radius;
        let stem_top = vent_pos + world_n * h_vis;

        // 1. Incandescent supersonic central volcanic throat core
        let pulse = 0.8 + 0.2 * (elapsed * 4.5 + (i as f32) * 1.7).sin();
        let stem_col = if is_sulfur {
            Color::srgba(1.0, 0.68, 0.12, pulse)
        } else {
            Color::srgba(1.0, 0.35, 0.08, pulse)
        };
        gizmos.line(vent_pos, stem_top, stem_col);

        let ortho1 = if world_n.y.abs() < 0.95 {
            world_n.cross(Vec3::Y).normalize()
        } else {
            world_n.cross(Vec3::Z).normalize()
        };
        let ortho2 = world_n.cross(ortho1).normalize();

        // 2. Parabolic umbrella ballistic canopy ribs
        let n_ribs = 6;
        for rib_j in 0..n_ribs {
            let theta = (rib_j as f32) * (TAU / (n_ribs as f32)) + elapsed * 0.15;
            let r_dir = ortho1 * theta.cos() + ortho2 * theta.sin();

            let n_segs = 5;
            for s in 0..n_segs {
                let u0 = (s as f32) / (n_segs as f32);
                let u1 = ((s + 1) as f32) / (n_segs as f32);

                let z0 = h_vis * (1.0 - 0.95 * u0 * u0);
                let r0 = fallout_radius * (u0 * FRAC_PI_2).sin();
                let p0 = vent_pos + world_n * z0 + r_dir * r0;

                let z1 = h_vis * (1.0 - 0.95 * u1 * u1);
                let r1 = fallout_radius * (u1 * FRAC_PI_2).sin();
                let p1 = vent_pos + world_n * z1 + r_dir * r1;

                let alpha = (1.0 - u0) * 0.75;
                let rib_col = if is_sulfur {
                    Color::srgba(1.0, 0.82, 0.25, alpha)
                } else {
                    Color::srgba(0.85, 0.45, 0.15, alpha)
                };
                gizmos.line(p0, p1, rib_col);

                // Animated glowing droplets descending along the umbrella fountain
                let droplet_t = (elapsed * 0.8 + (rib_j as f32 * 0.3) + u0) % 1.0;
                if (droplet_t - u0).abs() < 0.12 {
                    let d_pos = p0.lerp(p1, (droplet_t - u0) / 0.12);
                    let d_col = Color::srgba(1.0, 0.95, 0.40, 0.9);
                    gizmos.sphere(
                        Isometry3d::from_translation(d_pos),
                        visual_radius * 0.012,
                        d_col,
                    );
                }
            }
        }

        // 3. Circular sulfur fallout halo on the surface around the patera
        let ring_rot = Quat::from_rotation_arc(Vec3::Z, world_n);
        let ring_iso = Isometry3d::new(vent_pos + world_n * (visual_radius * 0.002), ring_rot);
        let halo_col = if is_sulfur {
            Color::srgba(0.92, 0.38, 0.08, 0.55)
        } else {
            Color::srgba(0.35, 0.25, 0.15, 0.45)
        };
        let inner_col = if is_sulfur {
            Color::srgba(0.88, 0.78, 0.18, 0.45)
        } else {
            Color::srgba(0.60, 0.35, 0.10, 0.35)
        };
        gizmos.circle(ring_iso, fallout_radius, halo_col);
        gizmos.circle(ring_iso, fallout_radius * 0.62, inner_col);
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
        gizmos.circle(Isometry3d::new(jet_top, cap_rot), w_top * 0.85, cap_col);

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
