use bevy::prelude::*;

use crate::simulation::accretion::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::terraforming::{BombardmentEvent, BombardmentType};

type ShockwaveBodyQueryItem<'w> = (
    Entity,
    &'static SimPosition,
    Option<&'static Radius>,
    Option<&'static Transform>,
);

fn resolve_shockwave_surface_pos_and_radius(
    p_pos: &SimPosition,
    p_rad: Option<&Radius>,
    p_tf: Option<&Transform>,
    fallback_dir: Vec3,
) -> (Vec3, f32) {
    let p_center = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
    let phys_r = p_rad.map_or(4.2e-5, |r| r.0 as f32).max(1e-5);
    let r_surf = p_tf
        .map(|tf| tf.scale.x)
        .filter(|&vis_r| vis_r > phys_r && (vis_r - 1.0).abs() > 1e-4)
        .unwrap_or(0.035);

    let norm = fallback_dir.normalize_or_zero();
    let world_dir = if norm == Vec3::ZERO {
        Vec3::Y
    } else {
        norm
    };

    (p_center + world_dir * (r_surf * 0.98), r_surf)
}

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

/// Collects collision merger, bounce, and bombardment events and spawns expanding physical shockwaves.
pub fn update_impact_shockwaves(
    time: Res<Time>,
    mut shockwave_pool: ResMut<ImpactShockwavePool>,
    mut merge_reader: MessageReader<AccretionMergeEvent>,
    mut bounce_reader: MessageReader<CollisionBounceEvent>,
    mut engulf_reader: MessageReader<PlanetaryEngulfmentEvent>,
    mut bombard_reader: MessageReader<BombardmentEvent>,
    bodies_query: Query<ShockwaveBodyQueryItem>,
) {
    let dt = time.delta_secs();

    for ev in merge_reader.read() {
        let ev_pos = Vec3::new(
            ev.merged_position.x as f32,
            ev.merged_position.y as f32,
            ev.merged_position.z as f32,
        );
        let (local_pos, pos, r_surf) = if let Ok((_, p_pos, p_rad, p_tf)) =
            bodies_query.get(ev.primary_entity)
        {
            let p_center = p_tf.map_or(
                Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
                |tf| tf.translation,
            );
            let (w_pos, r) = resolve_shockwave_surface_pos_and_radius(
                p_pos,
                p_rad,
                p_tf,
                ev_pos - p_center,
            );
            (w_pos - p_center, w_pos, r)
        } else {
            (Vec3::ZERO, ev_pos, 0.04)
        };

        let max_r = (r_surf * (0.45 + (ev.merged_mass * 400.0).cbrt() as f32 * 0.40))
            .clamp(r_surf * 0.35, r_surf * 1.15);
        let color = if ev.new_body_type == BodyType::GasGiant {
            Color::srgba(1.0, 0.6, 0.1, 0.9)
        } else {
            Color::srgba(1.0, 0.85, 0.3, 0.9)
        };

        shockwave_pool.shockwaves.push(ImpactShockwave {
            position: pos,
            radius: r_surf * 0.08,
            max_radius: max_r,
            timer: 0.0,
            max_timer: 1.4,
            color,
            primary_entity: Some(ev.primary_entity),
            local_position: local_pos,
        });
    }

    for ev in bounce_reader.read() {
        if let (Ok((_, pos1, p_rad1, p_tf1)), Ok((_, pos2, _, _))) =
            (bodies_query.get(ev.entity1), bodies_query.get(ev.entity2))
        {
            let p_center1 = p_tf1.map_or(
                Vec3::new(pos1.x as f32, pos1.y as f32, pos1.z as f32),
                |tf| tf.translation,
            );
            let p_center2 = Vec3::new(pos2.x as f32, pos2.y as f32, pos2.z as f32);
            let dir = (p_center2 - p_center1).normalize_or_zero();
            let (pos, r_surf) =
                resolve_shockwave_surface_pos_and_radius(pos1, p_rad1, p_tf1, dir);
            let local_pos = pos - p_center1;
            shockwave_pool.shockwaves.push(ImpactShockwave {
                position: pos,
                radius: r_surf * 0.08,
                max_radius: r_surf * 0.75,
                timer: 0.0,
                max_timer: 1.0,
                color: Color::srgba(0.4, 0.8, 1.0, 0.7),
                primary_entity: Some(ev.entity1),
                local_position: local_pos,
            });
        }
    }

    for ev in bombard_reader.read() {
        if let Ok((_, p_pos, p_rad, p_tf)) = bodies_query.get(ev.target_entity) {
            let p_center = p_tf.map_or(
                Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
                |tf| tf.translation,
            );
            let (pos, r_surf) = resolve_shockwave_surface_pos_and_radius(
                p_pos,
                p_rad,
                p_tf,
                ev.surface_normal,
            );
            let local_pos = pos - p_center;
            let color = match ev.bombardment_type {
                BombardmentType::IcyComet => Color::srgba(0.7, 0.9, 1.0, 0.95),
                BombardmentType::CarbonaceousChondrite => Color::srgba(1.0, 0.7, 0.3, 0.95),
                BombardmentType::VolatileAblationSalvo => Color::srgba(0.6, 0.85, 1.0, 0.90),
                BombardmentType::IronAsteroid => Color::srgba(1.0, 0.95, 0.8, 0.98),
            };
            shockwave_pool.shockwaves.push(ImpactShockwave {
                position: pos,
                radius: r_surf * 0.08,
                max_radius: r_surf * 0.65,
                timer: 0.0,
                max_timer: 1.2,
                color,
                primary_entity: Some(ev.target_entity),
                local_position: local_pos,
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
            primary_entity: None,
            local_position: Vec3::ZERO,
        });
    }

    shockwave_pool.shockwaves.retain_mut(|sw| {
        sw.timer += dt;
        let progress = sw.timer / sw.max_timer;
        sw.radius = sw.max_radius * progress.sqrt();
        if let Some(ent) = sw.primary_entity {
            if let Ok((_, p_pos, _, p_tf)) = bodies_query.get(ent) {
                let p_center = p_tf.map_or(
                    Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
                    |tf| tf.translation,
                );
                sw.position = p_center + sw.local_position;
            }
        }
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

/// Renders 3D parabolic ballistic ejecta plume arcs without wireframe sphere/circle cages.
pub fn draw_impact_shockwaves(gizmos: &mut Gizmos, shockwave_pool: &ImpactShockwavePool) {
    for sw in &shockwave_pool.shockwaves {
        let progress = (sw.timer / sw.max_timer).clamp(0.0, 1.0);
        let base_srgba = sw.color.to_srgba();

        let core_col = compute_impact_blackbody_color(base_srgba, (progress * 0.6).min(1.0), 0.72);
        let plume_col =
            compute_impact_blackbody_color(base_srgba, (progress + 0.15).min(1.0), 0.65);

        let normal = if sw.local_position.length_squared() > 1e-6 {
            sw.local_position.normalize()
        } else {
            Vec3::Y
        };
        let up = if normal.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let tangent_u = normal.cross(up).normalize_or_zero();
        let tangent_v = normal.cross(tangent_u).normalize_or_zero();

        // 3D Parabolic Ballistic Ejecta Plume Trajectories oriented along surface normal
        let ray_count = 10;
        let plume_reach = sw.radius * 0.88;
        let plume_apex = sw.max_radius * 0.34 * (progress * (1.2 - progress * 0.5)).max(0.05);
        for i in 0..ray_count {
            let az = (i as f32) * (std::f32::consts::TAU / ray_count as f32) + sw.max_radius * 3.1;
            let dir_tangent = tangent_u * az.cos() + tangent_v * az.sin();
            let speed_var = 0.75 + ((i as f32 * 1.9).sin() * 0.5 + 0.5) * 0.45;
            let r_tip = plume_reach * speed_var;
            let h_mid = plume_apex * speed_var;
            let h_tip = h_mid * (1.0 - progress * 0.85).max(-0.15);

            let p0 = sw.position + dir_tangent * (r_tip * 0.18);
            let p_mid = sw.position + dir_tangent * (r_tip * 0.58) + normal * h_mid;
            let p_tip = sw.position + dir_tangent * r_tip + normal * h_tip;

            gizmos.line(p0, p_mid, plume_col);
            gizmos.line(p_mid, p_tip, core_col);
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

            let tangent = Vec3::new(-phi1.sin(), 0.0, phi1.cos());
            let arc_half = (r1 * 0.14).clamp(0.00008, 0.004) * (1.0 - frac * 0.35);
            gizmos.line(
                pos1 - tangent * arc_half,
                pos1 + tangent * arc_half,
                node_col,
            );

            if let Some(&(r2, phi2, _, z2)) = stream.fragments.get(i + 1) {
                let pos2 = stream.primary_pos + Vec3::new(r2 * phi2.cos(), z2, r2 * phi2.sin());
                if pos1.distance(pos2) <= r1 * 0.65 {
                    gizmos.line(pos1, pos2, node_col);
                }
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
