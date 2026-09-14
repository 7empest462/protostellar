use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::SimulationConfig;

pub fn draw_stellar_evolution_nebula(
    gizmos: &mut Gizmos,
    star_vec: Vec3,
    evo: &StellarEvolutionState,
) {
    if evo.nebula_expansion_radius_au <= 0.0 || evo.nebula_opacity <= 0.01 {
        return;
    }
    let r_neb = evo.nebula_expansion_radius_au;
    let op = evo.nebula_opacity;

    if matches!(evo.phase, StellarEvolutionPhase::SupernovaExplosion) {
        gizmos.sphere(
            Isometry3d::from_translation(star_vec),
            r_neb,
            Color::srgba(0.95, 0.95, 1.0, 0.25 * op),
        );
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r_neb,
            Color::srgba(0.3, 0.8, 1.0, 0.85 * op),
        );
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            r_neb * 0.9,
            Color::srgba(1.0, 0.6, 0.2, 0.70 * op),
        );
    } else {
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r_neb,
            Color::srgba(0.95, 0.22, 0.38, 0.65 * op),
        );
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            r_neb,
            Color::srgba(0.95, 0.22, 0.38, 0.40 * op),
        );

        if r_neb > 0.5 {
            gizmos.circle(
                Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                r_neb * 0.78,
                Color::srgba(0.12, 0.95, 0.68, 0.75 * op),
            );
            gizmos.sphere(
                Isometry3d::from_translation(star_vec),
                r_neb * 0.78,
                Color::srgba(0.12, 0.95, 0.68, 0.06 * op),
            );
        }

        gizmos.sphere(
            Isometry3d::from_translation(star_vec),
            r_neb,
            Color::srgba(0.75, 0.20, 0.50, 0.04 * op),
        );
    }
}

pub fn draw_white_dwarf_gizmos(gizmos: &mut Gizmos, star_vec: Vec3, elapsed: f32) {
    let r_mag = 0.25f32;
    let pulse = (elapsed * 2.0).sin() * 0.05;
    for i in 0..6 {
        let angle = (i as f32) * (std::f32::consts::PI / 3.0);
        let rot = Quat::from_rotation_y(angle);
        gizmos.circle(
            Isometry3d::new(star_vec, rot),
            r_mag + pulse,
            Color::srgba(0.35, 0.80, 1.0, 0.45),
        );
    }
    gizmos.sphere(
        Isometry3d::from_translation(star_vec),
        0.08,
        Color::srgba(0.75, 0.90, 1.0, 0.20),
    );
}

pub fn draw_pulsar_lighthouse_gizmos(gizmos: &mut Gizmos, star_vec: Vec3, elapsed: f32) {
    let spin_rate = 24.0;
    let beam_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.38);
    let jet_len = 8.0f32;

    let north_beam = beam_rot * Vec3::Y;
    let south_beam = -north_beam;

    let core_color = Color::srgba(0.85, 0.95, 1.0, 0.95);
    let sheath_color = Color::srgba(0.25, 0.85, 1.0, 0.85);

    gizmos.line(star_vec, star_vec + north_beam * jet_len, core_color);
    gizmos.line(star_vec, star_vec + south_beam * jet_len, core_color);

    for &dist in &[2.0f32, 4.0, 6.0, 8.0] {
        let ring_r = 0.15 + (dist / jet_len) * 0.65;
        gizmos.circle(
            Isometry3d::new(
                star_vec + north_beam * dist,
                Quat::from_rotation_arc(Vec3::Z, north_beam),
            ),
            ring_r,
            sheath_color,
        );
        gizmos.circle(
            Isometry3d::new(
                star_vec + south_beam * dist,
                Quat::from_rotation_arc(Vec3::Z, south_beam),
            ),
            ring_r,
            sheath_color,
        );
    }

    let north_perp1 = (beam_rot * Vec3::X) * 0.70;
    let north_perp2 = (beam_rot * Vec3::Z) * 0.70;
    gizmos.line(
        star_vec,
        star_vec + north_beam * jet_len + north_perp1,
        sheath_color,
    );
    gizmos.line(
        star_vec,
        star_vec + north_beam * jet_len - north_perp1,
        sheath_color,
    );
    gizmos.line(
        star_vec,
        star_vec + north_beam * jet_len + north_perp2,
        sheath_color,
    );
    gizmos.line(
        star_vec,
        star_vec + north_beam * jet_len - north_perp2,
        sheath_color,
    );

    let south_perp1 = -north_perp1;
    let south_perp2 = -north_perp2;
    gizmos.line(
        star_vec,
        star_vec + south_beam * jet_len + south_perp1,
        sheath_color,
    );
    gizmos.line(
        star_vec,
        star_vec + south_beam * jet_len - south_perp1,
        sheath_color,
    );
    gizmos.line(
        star_vec,
        star_vec + south_beam * jet_len + south_perp2,
        sheath_color,
    );
    gizmos.line(
        star_vec,
        star_vec + south_beam * jet_len - south_perp2,
        sheath_color,
    );

    gizmos.circle(
        Isometry3d::new(star_vec, beam_rot),
        1.85,
        Color::srgba(0.20, 0.75, 1.0, 0.45),
    );
}

pub fn draw_magnetar_reconnection_gizmos(gizmos: &mut Gizmos, star_vec: Vec3, elapsed: f32) {
    let spin_rate = 1.2;
    let beam_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.26);

    let loop_tiers: [(f32, Color); 4] = [
        (1.2, Color::srgba(0.20, 0.95, 1.0, 0.85)),
        (2.4, Color::srgba(0.60, 0.35, 1.0, 0.80)),
        (3.8, Color::srgba(0.95, 0.20, 0.75, 0.75)),
        (5.5, Color::srgba(1.00, 0.15, 0.45, 0.70)),
    ];

    let num_quadrants = 8;
    let num_segments = 24;

    for &(r_0, loop_color) in &loop_tiers {
        for q in 0..num_quadrants {
            let azimuth = (q as f32) * (std::f32::consts::PI * 0.25);
            let loop_rot = beam_rot * Quat::from_rotation_y(azimuth);

            let mut prev_pt: Option<Vec3> = None;

            for s in 0..=num_segments {
                let frac = (s as f32) / (num_segments as f32);
                let theta = 0.25 + frac * (std::f32::consts::PI - 0.50);
                let sin_t = theta.sin();
                let cos_t = theta.cos();
                let r = r_0 * sin_t * sin_t;

                let x_l = r * sin_t;
                let y_l = r * cos_t;
                let alfven_phase = (s as f32) * 0.45 - elapsed * 6.5 + (q as f32) * 1.5;
                let z_l = alfven_phase.sin() * 0.07 * (r_0 / 2.0).clamp(0.5, 2.5);

                let pt = star_vec + loop_rot * Vec3::new(x_l, y_l, z_l);

                if let Some(p) = prev_pt {
                    gizmos.line(p, pt, loop_color);
                }
                prev_pt = Some(pt);
            }

            let apex_r = r_0;
            let knot_phase = elapsed * 5.0 + (q as f32) * 2.0;
            let knot_z = knot_phase.sin() * 0.06;
            let knot_pos = star_vec + loop_rot * Vec3::new(apex_r, 0.0, knot_z);
            let knot_radius = 0.04 + 0.02 * (knot_phase * 1.7).sin().abs();
            gizmos.sphere(
                Isometry3d::from_translation(knot_pos),
                knot_radius,
                loop_color,
            );
        }
    }
}

pub fn draw_neutron_star_gizmos(gizmos: &mut Gizmos, star_vec: Vec3, elapsed: f32) {
    let spin_rate = 8.0;
    let beam_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.35);
    let jet_len = 3.0;

    let north_beam = beam_rot * Vec3::Y;
    let south_beam = -north_beam;
    let beam_color = Color::srgba(0.40, 0.85, 1.0, 0.80);

    gizmos.line(star_vec, star_vec + north_beam * jet_len, beam_color);
    gizmos.line(star_vec, star_vec + south_beam * jet_len, beam_color);

    for i in 0..4 {
        let angle = (i as f32) * (std::f32::consts::PI / 2.0);
        let rot = beam_rot * Quat::from_rotation_y(angle);
        gizmos.circle(
            Isometry3d::new(star_vec, rot),
            0.65,
            Color::srgba(0.5, 0.3, 1.0, 0.40),
        );
    }
}

fn draw_quasar_beam_filaments_and_lobes(
    gizmos: &mut Gizmos,
    star_vec: Vec3,
    jet_len: f32,
    pole_start: f32,
    elapsed: f32,
) {
    for &(dir, sign) in &[(Vec3::Y, 1.0f32), (-Vec3::Y, -1.0f32)] {
        let base = star_vec + dir * pole_start;
        let tip = star_vec + dir * jet_len;

        gizmos.line(base, tip, Color::WHITE);

        let core_r = 0.06f32;
        for k in 0..8 {
            let theta = (k as f32) * (std::f32::consts::PI / 4.0);
            let offset = Vec3::new(theta.cos() * core_r, 0.0, theta.sin() * core_r);
            gizmos.line(
                base + offset,
                tip + offset,
                Color::srgba(0.95, 0.98, 1.0, 0.95),
            );
        }

        let sheath_r = 0.18f32;
        for k in 0..12 {
            let theta = (k as f32) * (std::f32::consts::PI / 6.0);
            let base_offset = Vec3::new(
                theta.cos() * sheath_r * 0.8,
                0.0,
                theta.sin() * sheath_r * 0.8,
            );
            let tip_offset = Vec3::new(theta.cos() * sheath_r, 0.0, theta.sin() * sheath_r);
            let sheath_color = if k % 2 == 0 {
                Color::srgba(0.20, 0.75, 1.0, 0.75)
            } else {
                Color::srgba(0.75, 0.35, 1.0, 0.70)
            };
            gizmos.line(base + base_offset, tip + tip_offset, sheath_color);
        }

        let helix_len = jet_len.min(400.0);
        let n_segments = 100;
        let dz = (helix_len - pole_start).max(1.0) / (n_segments as f32);
        for s in 0..n_segments {
            let z1 = pole_start + (s as f32) * dz;
            let z2 = pole_start + ((s + 1) as f32) * dz;
            let r1 = 0.12 + 0.0002 * z1;
            let r2 = 0.12 + 0.0002 * z2;
            let ang1 = z1 * 0.28 - elapsed * 16.0;
            let ang2 = z2 * 0.28 - elapsed * 16.0;

            let p1 = star_vec + Vec3::new(r1 * ang1.cos(), sign * z1, r1 * ang1.sin());
            let p2 = star_vec + Vec3::new(r2 * ang2.cos(), sign * z2, r2 * ang2.sin());
            gizmos.line(p1, p2, Color::srgba(0.40, 0.85, 1.0, 0.85));

            let ang1_b = ang1 + std::f32::consts::PI;
            let ang2_b = ang2 + std::f32::consts::PI;
            let p1_b = star_vec + Vec3::new(r1 * ang1_b.cos(), sign * z1, r1 * ang1_b.sin());
            let p2_b = star_vec + Vec3::new(r2 * ang2_b.cos(), sign * z2, r2 * ang2_b.sin());
            gizmos.line(p1_b, p2_b, Color::srgba(0.85, 0.35, 1.0, 0.80));
        }

        let mut kd = 25.0f32;
        let mut step = 40.0f32;
        while kd < jet_len && kd < 5000.0 {
            let knot_pos = star_vec + dir * kd;
            let knot_r = 0.14 + kd * 0.0005;
            gizmos.circle(
                Isometry3d::new(knot_pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                knot_r,
                Color::WHITE,
            );
            gizmos.circle(
                Isometry3d::new(knot_pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                knot_r * 1.5,
                Color::srgba(0.30, 0.75, 1.0, 0.65),
            );
            kd += step;
            step *= 1.35;
        }

        let lobe_r = 0.55f32;
        gizmos.sphere(
            Isometry3d::from_translation(tip),
            lobe_r,
            Color::srgba(0.85, 0.95, 1.0, 0.80),
        );
        gizmos.circle(
            Isometry3d::new(tip, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            lobe_r * 1.5,
            Color::srgba(0.35, 0.80, 1.0, 0.75),
        );
        gizmos.circle(
            Isometry3d::new(tip, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            lobe_r * 1.5,
            Color::srgba(0.80, 0.35, 1.0, 0.60),
        );
    }
}

fn draw_quasar_isco_particles(gizmos: &mut Gizmos, star_vec: Vec3, isco_r: f32, elapsed: f32) {
    let particle_tiers: [(f32, u32, f32, Color); 3] = [
        (1.0, 10, 18.0, Color::srgba(0.85, 0.95, 1.0, 0.95)),
        (1.6, 8, 12.0, Color::srgba(0.30, 0.75, 1.0, 0.85)),
        (2.4, 6, 7.5, Color::srgba(0.55, 0.40, 1.0, 0.70)),
    ];

    let tail_segments = 6u32;
    let tail_dt = 0.012f32;

    for &(r_mult, count, omega, col) in &particle_tiers {
        let orbit_r = isco_r * r_mult;

        for p_idx in 0..count {
            let phase_offset =
                (p_idx as f32) * (std::f32::consts::TAU / count as f32) + (p_idx as f32) * 1.618;
            let y_wobble = ((p_idx as f32) * 0.7 + elapsed * 2.5).sin() * orbit_r * 0.04;

            let mut prev_pt: Option<Vec3> = None;
            for seg in (0..=tail_segments).rev() {
                let t = elapsed - (seg as f32) * tail_dt;
                let theta = phase_offset + t * omega;
                let pt =
                    star_vec + Vec3::new(orbit_r * theta.cos(), y_wobble, orbit_r * theta.sin());

                if let Some(prev) = prev_pt {
                    let fade = 1.0 - (seg as f32 / tail_segments as f32);
                    let seg_col = Color::srgba(
                        col.to_srgba().red,
                        col.to_srgba().green,
                        col.to_srgba().blue,
                        col.to_srgba().alpha * fade * fade,
                    );
                    gizmos.line(prev, pt, seg_col);
                }
                prev_pt = Some(pt);
            }

            let head_theta = phase_offset + elapsed * omega;
            let head_pt = star_vec
                + Vec3::new(
                    orbit_r * head_theta.cos(),
                    y_wobble,
                    orbit_r * head_theta.sin(),
                );
            let cross_size = 0.12f32;
            gizmos.line(
                head_pt - Vec3::X * cross_size,
                head_pt + Vec3::X * cross_size,
                Color::WHITE,
            );
            gizmos.line(
                head_pt - Vec3::Z * cross_size,
                head_pt + Vec3::Z * cross_size,
                Color::WHITE,
            );
        }
    }

    let disk_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    gizmos.circle(
        Isometry3d::new(star_vec, disk_rot),
        isco_r,
        Color::srgba(0.40, 0.75, 1.0, 0.12),
    );
}

pub fn draw_quasar_relativistic_jets(
    gizmos: &mut Gizmos,
    star_vec: Vec3,
    star_radius: f64,
    body_type: BodyType,
    config: &SimulationConfig,
    is_blown_out: bool,
    blowout_p: f32,
    light_dist: f32,
    elapsed: f32,
) {
    if is_blown_out && light_dist > 0.5 {
        let jet_len = light_dist;
        let current_visual_radius = config.calc_visual_radius_for_type(star_radius, body_type);
        let pole_start = (current_visual_radius * 0.90).max(0.05);

        draw_quasar_beam_filaments_and_lobes(gizmos, star_vec, jet_len, pole_start, elapsed);

        let isco_r = (current_visual_radius * 1.15).max(2.8);
        draw_quasar_isco_particles(gizmos, star_vec, isco_r, elapsed);
    }

    if blowout_p > 0.001 && blowout_p < 1.0 {
        let r_blast = (60.0 + blowout_p * 240.0).clamp(60.0, 300.0);
        let blast_fade = (1.0 - blowout_p).clamp(0.0, 1.0);
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r_blast,
            Color::srgba(1.0, 0.45, 0.20, 0.90 * blast_fade),
        );
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r_blast * 0.97,
            Color::srgba(1.0, 0.75, 0.30, 0.75 * blast_fade),
        );
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            r_blast,
            Color::srgba(0.85, 0.25, 0.15, 0.50 * blast_fade),
        );
        gizmos.sphere(
            Isometry3d::from_translation(star_vec),
            r_blast,
            Color::srgba(0.70, 0.20, 0.10, 0.06 * blast_fade),
        );
    }
}

pub fn draw_quasi_star_photosphere_and_magnetosphere(
    gizmos: &mut Gizmos,
    star_vec: Vec3,
    elapsed: f32,
) {
    let envelope_r = 60.0f32;

    for (dr, alpha, col) in [
        (0.0f32, 0.70, Color::srgba(1.0, 0.28, 0.08, 0.70)),
        (2.5f32, 0.45, Color::srgba(1.0, 0.48, 0.12, 0.45)),
        (6.0f32, 0.25, Color::srgba(1.0, 0.70, 0.20, 0.25)),
        (12.0f32, 0.12, Color::srgba(0.95, 0.85, 0.35, 0.12)),
    ] {
        let r = envelope_r + dr;
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r,
            col,
        );
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            r,
            col,
        );
        gizmos.sphere(
            Isometry3d::from_translation(star_vec),
            r,
            Color::srgba(
                col.to_srgba().red,
                col.to_srgba().green,
                col.to_srgba().blue,
                alpha * 0.08,
            ),
        );
    }

    let mag_spin = Quat::from_rotation_y(elapsed * 0.25) * Quat::from_rotation_x(0.22);
    for i in 0..8 {
        let phi = (i as f32) * (std::f32::consts::PI / 4.0);
        let r0 = 95.0 + ((i % 3) as f32) * 18.0;
        let mut prev_pt: Option<Vec3> = None;
        let n_pts = 28;

        for s in 0..=n_pts {
            let theta = 0.38 + (std::f32::consts::PI - 0.76) * (s as f32 / n_pts as f32);
            let r_dipole = r0 * theta.sin().powi(2);
            if r_dipole >= envelope_r * 0.95 {
                let local_p = Vec3::new(
                    r_dipole * theta.sin() * phi.cos(),
                    r_dipole * theta.cos(),
                    r_dipole * theta.sin() * phi.sin(),
                );
                let world_p = star_vec + mag_spin * local_p;
                if let Some(p_prev) = prev_pt {
                    let line_col = if i % 2 == 0 {
                        Color::srgba(0.25, 0.88, 1.0, 0.75)
                    } else {
                        Color::srgba(1.0, 0.75, 0.25, 0.70)
                    };
                    gizmos.line(p_prev, world_p, line_col);
                }
                prev_pt = Some(world_p);
            }
        }
    }

    for (y_lat, belt_r, wave_speed) in [
        (0.0f32, envelope_r + 1.8, 4.0f32),
        (22.0f32, envelope_r * 0.92, -3.5f32),
        (-22.0f32, envelope_r * 0.92, 3.5f32),
        (38.0f32, envelope_r * 0.75, -5.0f32),
        (-38.0f32, envelope_r * 0.75, 5.0f32),
    ] {
        let n_seg = 48;
        let d_theta = std::f32::consts::TAU / (n_seg as f32);
        for k in 0..n_seg {
            let th1 = (k as f32) * d_theta;
            let th2 = ((k + 1) as f32) * d_theta;
            let wave1 = (th1 * 8.0 + elapsed * wave_speed).sin() * 1.6;
            let wave2 = (th2 * 8.0 + elapsed * wave_speed).sin() * 1.6;
            let r1 = belt_r + wave1;
            let r2 = belt_r + wave2;

            let p1 = star_vec + Vec3::new(r1 * th1.cos(), y_lat, r1 * th1.sin());
            let p2 = star_vec + Vec3::new(r2 * th2.cos(), y_lat, r2 * th2.sin());
            gizmos.line(p1, p2, Color::srgba(0.85, 0.35, 1.0, 0.65));
        }
    }

    for sign in [1.0f32, -1.0f32] {
        let cap_center = star_vec + mag_spin * Vec3::new(0.0, sign * 58.5, 0.0);
        let cap_rot = mag_spin * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let auroral_r = 18.0 + (elapsed * 3.0 + sign).sin() * 2.0;

        gizmos.circle(
            Isometry3d::new(cap_center, cap_rot),
            auroral_r,
            Color::srgba(0.20, 0.95, 1.0, 0.90),
        );
        gizmos.circle(
            Isometry3d::new(cap_center, cap_rot),
            auroral_r * 1.25,
            Color::srgba(0.35, 1.0, 0.50, 0.75),
        );
    }

    for p in 0..6 {
        let base_ang = (p as f32) * (std::f32::consts::PI / 3.0) + elapsed * 0.08;
        let prom_height = 14.0 + ((p * 7) as f32 % 5.0) * 3.5;
        let apex_vec = Vec3::new(
            base_ang.cos(),
            0.35 * ((p % 2) as f32 * 2.0 - 1.0),
            base_ang.sin(),
        )
        .normalize();
        let prom_apex = star_vec + apex_vec * (envelope_r + prom_height);

        let foot1_ang = base_ang - 0.18;
        let foot2_ang = base_ang + 0.18;
        let foot1 = star_vec + Vec3::new(foot1_ang.cos(), 0.0, foot1_ang.sin()) * envelope_r;
        let foot2 = star_vec + Vec3::new(foot2_ang.cos(), 0.0, foot2_ang.sin()) * envelope_r;

        let mut prev = foot1;
        for step in 1..=12 {
            let t = step as f32 / 12.0;
            let base_interp = foot1.lerp(foot2, t);
            let loop_pt = base_interp + (prom_apex - star_vec) * (4.0 * t * (1.0 - t));
            gizmos.line(prev, loop_pt, Color::srgba(1.0, 0.40, 0.10, 0.80));
            prev = loop_pt;
        }
    }
}

pub fn draw_black_hole_spacetime_curvature(
    gizmos: &mut Gizmos,
    star_vec: Vec3,
    base_lens_r: f32,
    is_blown_out: bool,
    elapsed: f32,
) {
    for (ring_scale, alpha_mul, col) in [
        (1.00f32, 0.95f32, Color::srgba(1.0, 0.96, 0.88, 0.95)),
        (1.04f32, 0.70f32, Color::srgba(0.35, 0.85, 1.0, 0.70)),
        (1.18f32, 0.45f32, Color::srgba(1.0, 0.65, 0.20, 0.45)),
        (1.45f32, 0.25f32, Color::srgba(0.85, 0.25, 0.50, 0.25)),
        (2.20f32, 0.12f32, Color::srgba(0.50, 0.20, 0.85, 0.12)),
    ] {
        let r_ring = base_lens_r * ring_scale;
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r_ring,
            col,
        );
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            r_ring,
            Color::srgba(
                col.to_srgba().red,
                col.to_srgba().green,
                col.to_srgba().blue,
                alpha_mul * 0.35,
            ),
        );
        if ring_scale < 1.3 {
            gizmos.sphere(
                Isometry3d::from_translation(star_vec),
                r_ring,
                Color::srgba(
                    col.to_srgba().red,
                    col.to_srgba().green,
                    col.to_srgba().blue,
                    alpha_mul * 0.04,
                ),
            );
        }
    }

    let n_geodesics = 12u32;
    let spin_rot = Quat::from_rotation_y(elapsed * 0.35);
    for g_idx in 0..n_geodesics {
        let base_angle = (g_idx as f32) * (std::f32::consts::TAU / n_geodesics as f32);
        let mut prev_geo: Option<Vec3> = None;
        let n_steps = 24;

        for step in 0..=n_steps {
            let frac = step as f32 / n_steps as f32;
            let r_geo = base_lens_r * (3.5 - frac * 2.3);
            let theta_geo = base_angle + frac * 2.8 + (1.0 - frac) * 0.5;
            let y_funnel = -((frac * 1.8).powi(2)) * (base_lens_r * 0.08);

            let local_pt = Vec3::new(r_geo * theta_geo.cos(), y_funnel, r_geo * theta_geo.sin());
            let world_pt = star_vec + spin_rot * local_pt;

            if let Some(prev) = prev_geo {
                let fade = (1.0 - frac * 0.75) * (0.35 + (g_idx % 2) as f32 * 0.25);
                let geo_col = if is_blown_out {
                    Color::srgba(0.40, 0.80, 1.0, 0.45 * fade)
                } else {
                    Color::srgba(1.0, 0.45, 0.15, 0.40 * fade)
                };
                gizmos.line(prev, world_pt, geo_col);
            }
            prev_geo = Some(world_pt);
        }
    }
}

pub fn draw_standard_black_hole_jets(gizmos: &mut Gizmos, star_vec: Vec3) {
    let disk_inner = 0.08f32;
    let disk_outer = 0.55f32;
    let disk_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);

    gizmos.circle(
        Isometry3d::new(star_vec, disk_rot),
        disk_inner,
        Color::srgba(1.0, 0.9, 0.4, 0.90),
    );
    gizmos.circle(
        Isometry3d::new(star_vec, disk_rot),
        disk_inner * 2.0,
        Color::srgba(1.0, 0.55, 0.15, 0.70),
    );
    gizmos.circle(
        Isometry3d::new(star_vec, disk_rot),
        disk_outer,
        Color::srgba(0.85, 0.25, 0.05, 0.45),
    );

    let jet_len = 12.0f32;
    let jet_color = Color::srgba(0.4, 0.85, 1.0, 0.75);
    gizmos.line(star_vec, star_vec + Vec3::Y * jet_len, jet_color);
    gizmos.line(star_vec, star_vec - Vec3::Y * jet_len, jet_color);
    gizmos.circle(
        Isometry3d::new(
            star_vec + Vec3::Y * jet_len,
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ),
        0.65,
        jet_color,
    );
    gizmos.circle(
        Isometry3d::new(
            star_vec - Vec3::Y * jet_len,
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ),
        0.65,
        jet_color,
    );
}

pub fn draw_ignition_shockwave(gizmos: &mut Gizmos, star_vec: Vec3, ignition: &IgnitionState) {
    if !ignition.is_ignited || ignition.shockwave_radius <= 0.0 {
        return;
    }
    let r = ignition.shockwave_radius as f32;
    let fade = (1.0 - (r / 35.0)).clamp(0.1, 1.0);

    gizmos.circle(
        Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        r,
        Color::srgba(1.0, 0.95, 0.6, 0.85 * fade),
    );

    if r > 0.3 {
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r * 0.96,
            Color::srgba(1.0, 0.65, 0.15, 0.5 * fade),
        );
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r * 0.92,
            Color::srgba(0.9, 0.3, 0.1, 0.3 * fade),
        );
    }

    gizmos.sphere(
        Isometry3d::from_translation(star_vec),
        r,
        Color::srgba(1.0, 0.8, 0.3, 0.08 * fade),
    );

    if r < 15.0 {
        let n_rays = 12;
        for i in 0..n_rays {
            let angle = (i as f32) * (2.0 * std::f32::consts::PI / n_rays as f32);
            let ray_len = (r * 1.15).min(18.0);
            let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
            gizmos.line(
                star_vec + dir * 1.2,
                star_vec + dir * ray_len,
                Color::srgba(1.0, 0.85, 0.4, 0.45 * fade),
            );
        }
    }
}
