use bevy::prelude::*;

use crate::simulation::accretion::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;

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
        let n_fragments = 48;
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

pub fn draw_impact_shockwaves(gizmos: &mut Gizmos, shockwave_pool: &ImpactShockwavePool) {
    for sw in &shockwave_pool.shockwaves {
        let alpha = (1.0 - (sw.timer / sw.max_timer)).clamp(0.0, 1.0);
        let sw_color = Color::srgba(
            sw.color.to_srgba().red,
            sw.color.to_srgba().green,
            sw.color.to_srgba().blue,
            alpha * 0.8,
        );

        gizmos.circle(
            Isometry3d::new(
                sw.position,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            sw.radius,
            sw_color,
        );
        gizmos.sphere(
            Isometry3d::from_translation(sw.position),
            sw.radius * 0.4,
            Color::srgba(
                sw.color.to_srgba().red,
                sw.color.to_srgba().green,
                sw.color.to_srgba().blue,
                alpha * 0.3,
            ),
        );
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

        let base_col = if stream.ice_fraction >= 0.70 {
            Color::srgba(0.85, 0.95, 1.0, 0.90 * alpha)
        } else if stream.ice_fraction >= 0.35 {
            Color::srgba(0.95, 0.80, 0.50, 0.85 * alpha)
        } else {
            Color::srgba(1.0, 0.50, 0.20, 0.85 * alpha)
        };
        let spark_col = Color::srgba(1.0, 1.0, 1.0, 0.95 * alpha);

        for (i, &(r1, phi1, _, z1)) in stream.fragments.iter().enumerate() {
            let pos1 = stream.primary_pos + Vec3::new(r1 * phi1.cos(), z1, r1 * phi1.sin());

            gizmos.sphere(
                Isometry3d::from_translation(pos1),
                0.015 + 0.012 * (1.0 - p),
                spark_col,
            );

            if let Some(&(r2, phi2, _, z2)) = stream.fragments.get(i + 1) {
                let pos2 = stream.primary_pos + Vec3::new(r2 * phi2.cos(), z2, r2 * phi2.sin());
                gizmos.line(pos1, pos2, base_col);
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
