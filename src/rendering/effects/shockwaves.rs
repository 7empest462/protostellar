use bevy::prelude::*;

use crate::simulation::accretion::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Evaluates blackbody-cooling RGB color for an expanding impact fireball and ejecta plume.
pub fn compute_impact_blackbody_color(base: Srgba, progress: f32, alpha_scale: f32) -> Color {
    let p = progress.clamp(0.0, 1.0);
    let alpha = ((1.0 - p * p) * alpha_scale).clamp(0.0, 1.0);
    // 0.0..0.20: 6000K white-gold plasma flash -> 0.20..0.60: 3000K orange fireball -> 0.60..1.0: 1300K crimson ember
    let (r, g, b) = if p < 0.20 {
        let t = p / 0.20;
        (1.0, 0.98 - t * 0.16, (0.92 - t * 0.52).max(base.blue * 0.6))
    } else if p < 0.65 {
        let t = (p - 0.20) / 0.45;
        (
            1.0,
            (0.82 - t * 0.44).max(base.green * 0.5),
            (0.40 - t * 0.30).max(base.blue * 0.3),
        )
    } else {
        let t = (p - 0.65) / 0.35;
        (
            (1.0 - t * 0.35).max(0.45),
            (0.38 - t * 0.26).max(0.08),
            (0.10 - t * 0.08).max(0.02),
        )
    };
    Color::srgba(r, g, b, alpha)
}

/// Collects collision merger and engulfment events and spawns expanding physical shockwaves.
pub fn update_impact_shockwaves(
    time: Res<Time>,
    mut shockwave_pool: ResMut<ImpactShockwavePool>,
    mut merge_reader: MessageReader<AccretionMergeEvent>,
    mut bounce_reader: MessageReader<CollisionBounceEvent>,
    mut engulf_reader: MessageReader<PlanetaryEngulfmentEvent>,
    bodies_query: Query<&SimPosition>,
) {
    let dt = time.delta_secs();

    for ev in merge_reader.read() {
        let pos = Vec3::new(
            ev.merged_position.x as f32,
            ev.merged_position.y as f32,
            ev.merged_position.z as f32,
        );
        let max_r = (0.4 + (ev.merged_mass * 100.0).cbrt() as f32 * 0.35).clamp(0.5, 3.0);
        let color = if ev.new_body_type == BodyType::GasGiant {
            Color::srgba(1.0, 0.6, 0.1, 0.9)
        } else {
            Color::srgba(1.0, 0.85, 0.3, 0.9)
        };

        shockwave_pool.shockwaves.push(ImpactShockwave {
            position: pos,
            radius: 0.05,
            max_radius: max_r,
            timer: 0.0,
            max_timer: 1.8,
            color,
        });
    }

    for ev in bounce_reader.read() {
        if let Ok(pos1) = bodies_query.get(ev.entity1) {
            let pos = Vec3::new(pos1.x as f32, pos1.y as f32, pos1.z as f32);
            shockwave_pool.shockwaves.push(ImpactShockwave {
                position: pos,
                radius: 0.05,
                max_radius: 0.6,
                timer: 0.0,
                max_timer: 1.2,
                color: Color::srgba(0.4, 0.8, 1.0, 0.7),
            });
        }
    }

    for ev in engulf_reader.read() {
        let max_r = (1.5f32 + (ev.planet_mass_earth).cbrt() as f32 * 0.8f32).clamp(1.5f32, 5.0f32);
        shockwave_pool.shockwaves.push(ImpactShockwave {
            position: Vec3::ZERO,
            radius: 0.2,
            max_radius: max_r,
            timer: 0.0,
            max_timer: 3.5,
            color: Color::srgba(1.0, 0.2, 0.1, 0.95),
        });
    }

    shockwave_pool.shockwaves.retain_mut(|sw| {
        sw.timer += dt;
        let progress = sw.timer / sw.max_timer;
        sw.radius = sw.max_radius * progress.sqrt();
        sw.timer < sw.max_timer
    });
}

/// Collects Roche tidal disruption events and simulates expanding Keplerian debris streamer fragments.
pub fn update_roche_debris_streams(
    time: Res<Time>,
    mut debris_pool: ResMut<RocheDebrisPool>,
    mut roche_reader: MessageReader<RocheDisruptionEvent>,
    bodies_query: Query<&SimPosition>,
) {
    let dt = time.delta_secs();

    for ev in roche_reader.read() {
        let n_fragments = 56;
        let mut fragments = Vec::with_capacity(n_fragments);
        let base_angle =
            (ev.disruption_pos.z - ev.primary_pos.z).atan2(ev.disruption_pos.x - ev.primary_pos.x);
        let r_disrupt = ((ev.disruption_pos.x - ev.primary_pos.x)
            .hypot(ev.disruption_pos.z - ev.primary_pos.z))
        .max(0.02);

        let inner_r = (r_disrupt * 0.55).max(0.015);
        let outer_r = (r_disrupt * 1.35).max(inner_r + 0.01);

        for k in 0..n_fragments {
            let frac = (k as f32) / (n_fragments as f32);
            let frag_r = inner_r + (outer_r - inner_r) * frac;
            let phase = base_angle + (frac * std::f32::consts::TAU * 0.85);
            let omega = (1.8 / (frag_r * frag_r * frag_r).sqrt()).clamp(0.4, 8.0);
            let z_off = ((k as f32 * 1.7).sin() * 0.015) * (1.0 - frac * 0.5);

            fragments.push((frag_r, phase, omega, z_off));
        }

        debris_pool.streams.push(RocheDebrisStream {
            primary_entity: ev.primary_entity,
            primary_pos: ev.primary_pos,
            disruption_pos: ev.disruption_pos,
            inner_radius: inner_r,
            outer_radius: outer_r,
            timer: 0.0,
            max_timer: 4.5,
            ice_fraction: ev.ice_fraction,
            debris_mass_earth: ev.ring_mass_earth,
            fragments,
        });
    }

    debris_pool.streams.retain_mut(|stream| {
        stream.timer += dt;

        if let Ok(pos) = bodies_query.get(stream.primary_entity) {
            stream.primary_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        }

        for frag in &mut stream.fragments {
            frag.1 += frag.2 * dt;
            frag.3 *= 1.0 - (0.75 * dt).min(0.9);
        }

        stream.timer < stream.max_timer
    });
}

/// Renders 3D expanding incandescent vapor domes, Mach shock rings, and ballistic ejecta plume arcs.
pub fn draw_impact_shockwaves(gizmos: &mut Gizmos, shockwave_pool: &ImpactShockwavePool) {
    let eq_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    for sw in &shockwave_pool.shockwaves {
        let progress = (sw.timer / sw.max_timer).clamp(0.0, 1.0);
        let base_srgba = sw.color.to_srgba();

        let shock_col = compute_impact_blackbody_color(base_srgba, progress, 0.88);
        let core_col = compute_impact_blackbody_color(base_srgba, (progress * 0.6).min(1.0), 0.72);
        let plume_col =
            compute_impact_blackbody_color(base_srgba, (progress + 0.15).min(1.0), 0.65);

        // 1. Primary & Secondary Expanding Mach Shockwave Rings
        gizmos.circle(Isometry3d::new(sw.position, eq_rot), sw.radius, shock_col);
        gizmos.circle(
            Isometry3d::new(sw.position, eq_rot),
            sw.radius * 0.74,
            plume_col,
        );

        // 2. 3D Incandescent Fireball Core & Expanding Vapor Dome
        gizmos.sphere(
            Isometry3d::from_translation(sw.position),
            sw.radius * 0.42,
            plume_col,
        );
        if progress < 0.75 {
            gizmos.sphere(
                Isometry3d::from_translation(sw.position),
                sw.radius * 0.20 * (1.0 - progress * 0.5),
                core_col,
            );
        }

        // 3. 3D Parabolic Ballistic Ejecta Plume Trajectories & Molten Droplet Tips
        let ray_count = 10;
        let plume_reach = sw.radius * 0.88;
        let plume_apex = sw.max_radius * 0.34 * (progress * (1.2 - progress * 0.5)).max(0.05);
        for i in 0..ray_count {
            let az = (i as f32) * (std::f32::consts::TAU / ray_count as f32) + sw.max_radius * 3.1;
            let dir_xz = Vec3::new(az.cos(), 0.0, az.sin());
            let speed_var = 0.75 + ((i as f32 * 1.9).sin() * 0.5 + 0.5) * 0.45;
            let r_tip = plume_reach * speed_var;
            let h_mid = plume_apex * speed_var;
            let h_tip = h_mid * (1.0 - progress * 0.85).max(-0.15);

            let p0 = sw.position + dir_xz * (r_tip * 0.18);
            let p_mid = sw.position + dir_xz * (r_tip * 0.58) + Vec3::Y * h_mid;
            let p_tip = sw.position + dir_xz * r_tip + Vec3::Y * h_tip;

            gizmos.line(p0, p_mid, plume_col);
            gizmos.line(p_mid, p_tip, core_col);
            gizmos.sphere(
                Isometry3d::from_translation(p_tip),
                (0.008 + sw.max_radius * 0.008) * (1.0 - progress * 0.65),
                core_col,
            );
        }
    }
}

pub fn draw_roche_debris_streamers(gizmos: &mut Gizmos, debris_pool: &RocheDebrisPool) {
    for stream in &debris_pool.streams {
        let p = (stream.timer / stream.max_timer).clamp(0.0, 1.0);
        let alpha = if p < 0.15 {
            p / 0.15
        } else if p > 0.65 {
            (1.0 - p) / 0.35
        } else {
            1.0
        };

        for (i, &(r1, phi1, _, z1)) in stream.fragments.iter().enumerate() {
            let frac = (i as f32) / (stream.fragments.len().max(1) as f32);
            let pos1 = stream.primary_pos + Vec3::new(r1 * phi1.cos(), z1, r1 * phi1.sin());

            let node_col = if stream.ice_fraction >= 0.70 {
                Color::srgba(0.82, 0.94, 1.0, (0.92 - frac * 0.25) * alpha)
            } else {
                // Tidal shear friction heats rocky inner fragments to incandescent amber-orange
                let heat = (1.0 - p * 0.75) * (1.0 - frac * 0.6);
                Color::srgba(1.0, 0.48 + heat * 0.42, 0.18 + heat * 0.35, 0.90 * alpha)
            };

            gizmos.sphere(
                Isometry3d::from_translation(pos1),
                (0.012 + 0.012 * (1.0 - p)) * (1.0 - frac * 0.35),
                node_col,
            );

            if let Some(&(r2, phi2, _, z2)) = stream.fragments.get(i + 1) {
                let pos2 = stream.primary_pos + Vec3::new(r2 * phi2.cos(), z2, r2 * phi2.sin());
                gizmos.line(pos1, pos2, node_col);
            }
        }
    }
}

pub fn draw_au_guide_rings(gizmos: &mut Gizmos, star_vec: Vec3) {
    let au_rings = [
        (0.387f32, Color::srgba(0.85, 0.75, 0.65, 0.22)),
        (0.723f32, Color::srgba(0.95, 0.85, 0.45, 0.22)),
        (1.000f32, Color::srgba(0.35, 0.85, 1.00, 0.32)),
        (1.524f32, Color::srgba(1.00, 0.45, 0.25, 0.22)),
        (5.200f32, Color::srgba(1.00, 0.75, 0.35, 0.25)),
        (9.580f32, Color::srgba(0.95, 0.85, 0.50, 0.22)),
        (19.20f32, Color::srgba(0.45, 0.90, 1.00, 0.18)),
        (30.05f32, Color::srgba(0.35, 0.65, 1.00, 0.16)),
        (39.50f32, Color::srgba(0.65, 0.50, 0.95, 0.14)),
    ];
    let ring_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    for (r_au, col) in au_rings {
        gizmos.circle(Isometry3d::new(star_vec, ring_rot), r_au, col);
    }
}
