use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::simulation::accretion::events::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::G_ASTRO;

type BodyQueryItem<'w> = (
    &'static SimPosition,
    Option<&'static SimVelocity>,
    Option<&'static Mass>,
    Option<&'static Radius>,
);

/// Spawns high-resolution molten debris fragments and ring envelopes for Moon formation events.
fn collect_moon_formation_ejecta(
    ejecta_pool: &mut ImpactEjectaPool,
    moon_reader: &mut MessageReader<MoonFormationEvent>,
    bodies_query: &Query<BodyQueryItem>,
) {
    for ev in moon_reader.read() {
        let orbit_r = ev.orbital_radius_au as f32;
        let inner_r = orbit_r * 0.4;
        let outer_r = orbit_r * 1.5;
        let n_ring = 48;
        let mut fragments = Vec::with_capacity(n_ring);

        for k in 0..n_ring {
            let frac = (k as f32) / (n_ring as f32);
            let frag_r = inner_r + (outer_r - inner_r) * frac * frac.sqrt();
            let phase = frac * TAU * 3.0;
            let omega = (2.0 / (frag_r.max(0.01))).clamp(0.1, 5.0);
            let z_off = ((k as f32 * 2.3).sin() * 0.05) * orbit_r;
            fragments.push((frag_r, phase, omega, z_off));
        }

        ejecta_pool.rings.push(ImpactEjectaRing {
            primary_entity: ev.parent_entity,
            primary_pos: Vec3::ZERO,
            impact_pos: Vec3::ZERO,
            timer: 0.0,
            max_timer: 20.0,
            ring_mass: ev.moon_mass,
            color: Color::srgba(1.0, 0.4, 0.1, 0.95),
            fragments,
        });

        // Spawn 3D Keplerian Molten Debris Fragments forming the proto-lunar accretion disk
        if let Ok((p_pos, p_vel, p_mass, _)) = bodies_query.get(ev.parent_entity) {
            let p_center = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
            let p_v = p_vel.map_or(Vec3::ZERO, |v| {
                Vec3::new(v.x as f32, v.y as f32, v.z as f32)
            });
            let m_primary = p_mass.map_or(3e-6, |m| m.0).max(1e-8);
            let n_molten = 64;

            for i in 0..n_molten {
                let frac = i as f32 / n_molten as f32;
                let r_dist = inner_r + (outer_r - inner_r) * frac;
                let theta = frac * TAU + (i as f32 * 0.35).sin() * 0.15;
                let pos = p_center
                    + Vec3::new(
                        r_dist * theta.cos(),
                        ((i as f32 * 2.1).sin() * 0.02) * r_dist,
                        r_dist * theta.sin(),
                    );

                let v_circ = ((G_ASTRO * m_primary) / f64::from(r_dist).max(1e-4)).sqrt() as f32;
                let v_dir = Vec3::new(-theta.sin(), 0.0, theta.cos());
                let vel = p_v + v_dir * v_circ * (0.92 + (i as f32 * 1.7).sin() * 0.16);

                let temp = 2800.0 + (i as f32 * 1.3).sin() * 600.0;
                ejecta_pool.fragments.push(MoltenDebrisFragment {
                    position: pos,
                    velocity: vel,
                    initial_temperature: temp,
                    temperature: temp,
                    cooling_rate: 0.05 + ((i as f32 * 0.9).sin() * 0.5 + 0.5) * 0.04,
                    radius: 0.0006 + ((i as f32 * 1.1).cos() * 0.5 + 0.5) * 0.0008,
                    mass: ev.moon_mass / f64::from(n_molten),
                    primary_entity: ev.parent_entity,
                    age: 0.0,
                    max_age: 18.0 + ((i as f32 * 1.4).sin() * 0.5 + 0.5) * 12.0,
                    trail: [pos; 4],
                });
            }
        }
    }
}

/// Spawns 3D ballistic molten fragments from violent planetesimal and planetary mergers.
fn collect_collision_merger_ejecta(
    ejecta_pool: &mut ImpactEjectaPool,
    merge_reader: &mut MessageReader<AccretionMergeEvent>,
    bodies_query: &Query<BodyQueryItem>,
) {
    for ev in merge_reader.read() {
        let p_ent = ev.primary_entity;
        let Ok((p_pos, p_vel, p_mass, p_rad)) = bodies_query.get(p_ent) else {
            continue;
        };

        let p_center = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
        let p_v = p_vel.map_or(Vec3::ZERO, |v| {
            Vec3::new(v.x as f32, v.y as f32, v.z as f32)
        });
        let m_prim = p_mass.map_or(1e-6, |m| m.0).max(1e-8);
        let r_prim = p_rad.map_or(4.2e-5, |r| r.0 as f32).max(1e-5);

        let v_circ = ((G_ASTRO * m_prim) / f64::from(r_prim).max(1e-5)).sqrt() as f32;
        let imp_pos = Vec3::new(
            ev.merged_position.x as f32,
            ev.merged_position.y as f32,
            ev.merged_position.z as f32,
        );
        let normal = (imp_pos - p_center).normalize_or_zero();
        let up = if normal.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let tangent = normal.cross(up).normalize();
        let bitangent = normal.cross(tangent);

        let n_spawn = ((ev.merged_mass * 50_000.0).clamp(24.0, 64.0)) as usize;
        let base_temp = (2200.0 + (ev.energy_released.log10().max(20.0) - 20.0) as f32 * 120.0)
            .clamp(2400.0, 3800.0);

        for i in 0..n_spawn {
            let frac = i as f32 / n_spawn as f32;
            let az = frac * TAU + (i as f32 * 0.7).sin() * 0.3;
            let cone_angle = 0.55 + ((i as f32 * 1.5).cos() * 0.5 + 0.5) * 0.35;
            let dir = (normal * cone_angle.cos()
                + (tangent * az.cos() + bitangent * az.sin()) * cone_angle.sin())
            .normalize();

            let speed = v_circ * (0.65 + ((i as f32 * 2.3).sin() * 0.5 + 0.5) * 0.85);
            let pos = p_center + normal * (r_prim * 1.05) + dir * (r_prim * 0.15);
            let vel = p_v + dir * speed;
            let temp = base_temp + ((i as f32 * 1.9).sin() * 0.5 + 0.5) * 400.0;

            ejecta_pool.fragments.push(MoltenDebrisFragment {
                position: pos,
                velocity: vel,
                initial_temperature: temp,
                temperature: temp,
                cooling_rate: 0.08 + ((i as f32 * 0.5).sin() * 0.5 + 0.5) * 0.06,
                radius: (r_prim * 0.22).clamp(0.0004, 0.0025),
                mass: (ev.merged_mass * 0.005) / f64::from(n_spawn as u32),
                primary_entity: p_ent,
                age: 0.0,
                max_age: 14.0 + ((i as f32 * 1.2).sin() * 0.5 + 0.5) * 8.0,
                trail: [pos; 4],
            });
        }
    }
}

/// Spawns grazing spray ejecta fragments from non-merging bounce collisions.
fn collect_collision_bounce_ejecta(
    ejecta_pool: &mut ImpactEjectaPool,
    bounce_reader: &mut MessageReader<CollisionBounceEvent>,
    bodies_query: &Query<BodyQueryItem>,
) {
    for ev in bounce_reader.read() {
        let (Ok((pos1, vel1, m1, r1)), Ok((pos2, _, _, _))) =
            (bodies_query.get(ev.entity1), bodies_query.get(ev.entity2))
        else {
            continue;
        };

        let p1 = Vec3::new(pos1.x as f32, pos1.y as f32, pos1.z as f32);
        let p2 = Vec3::new(pos2.x as f32, pos2.y as f32, pos2.z as f32);
        let v1 = vel1.map_or(Vec3::ZERO, |v| {
            Vec3::new(v.x as f32, v.y as f32, v.z as f32)
        });
        let m_prim = m1.map_or(1e-6, |m| m.0).max(1e-8);
        let r_prim = r1.map_or(4.2e-5, |r| r.0 as f32).max(1e-5);

        let mid = (p1 + p2) * 0.5;
        let spray_dir = (p2 - p1).normalize_or_zero();
        let up = if spray_dir.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let side = spray_dir.cross(up).normalize();

        let v_circ = ((G_ASTRO * m_prim) / f64::from(r_prim).max(1e-5)).sqrt() as f32;
        let n_spray = 20;

        for i in 0..n_spray {
            let angle = (i as f32 / n_spray as f32 - 0.5) * 1.8;
            let dir = (spray_dir * angle.cos() + side * angle.sin()).normalize();
            let speed = v_circ * (0.8 + ((i as f32 * 1.7).sin() * 0.5 + 0.5) * 0.7);
            let pos = mid + dir * (r_prim * 0.1);
            let temp = 2200.0 + ((i as f32 * 1.3).sin() * 0.5 + 0.5) * 500.0;

            ejecta_pool.fragments.push(MoltenDebrisFragment {
                position: pos,
                velocity: v1 + dir * speed,
                initial_temperature: temp,
                temperature: temp,
                cooling_rate: 0.12,
                radius: (r_prim * 0.18).clamp(0.0003, 0.0018),
                mass: 1e-10,
                primary_entity: ev.entity1,
                age: 0.0,
                max_age: 10.0,
                trail: [pos; 4],
            });
        }
    }
}

/// Integrates orbital gravitational trajectories, radiative cooling, and surface re-impacts.
fn step_ejecta_simulation(
    ejecta_pool: &mut ImpactEjectaPool,
    dt_secs: f32,
    sim_dt_yr: f64,
    bodies_query: &Query<BodyQueryItem>,
    star_query: &Query<(&SimPosition, &Mass), With<CentralStar>>,
) {
    let dt_grav = (sim_dt_yr as f32).clamp(0.0001, 0.004);
    let star_data = star_query.iter().next().map(|(pos, mass)| {
        (
            Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32),
            mass.0.max(0.01),
        )
    });

    ejecta_pool.fragments.retain_mut(|frag| {
        frag.age += dt_secs;
        if frag.age >= frag.max_age {
            return false;
        }

        // Exponential thermal radiative blackbody cooling
        let cool_decay = (-frag.cooling_rate * frag.age).exp();
        frag.temperature = 200.0 + (frag.initial_temperature - 200.0) * cool_decay;

        // Gravitational acceleration from primary entity
        if let Ok((p_pos, _, p_mass, p_rad)) = bodies_query.get(frag.primary_entity) {
            let p_center = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
            let to_center = p_center - frag.position;
            let dist = to_center.length();
            let r_surf = p_rad.map_or(4.2e-5, |r| r.0 as f32);

            // Re-impact detection: fragment plunged into the planet's surface
            if dist <= r_surf * 1.02 {
                return false;
            }

            let m_prim = p_mass.map_or(1e-6, |m| m.0).max(1e-8);
            let inv_r3 = 1.0 / (dist * dist * dist).max(1e-9);
            let a_prim = to_center * (G_ASTRO as f32 * m_prim as f32 * inv_r3);
            frag.velocity += a_prim * dt_grav;

            // Gravitational pull from central star
            if let Some((s_pos, s_mass)) = star_data {
                let to_star = s_pos - frag.position;
                let d_star = to_star.length().max(0.05);
                let a_star =
                    to_star * ((G_ASTRO as f32 * s_mass as f32) / (d_star * d_star * d_star));
                frag.velocity += a_star * dt_grav;
            }

            // Shift trailing history
            frag.trail[3] = frag.trail[2];
            frag.trail[2] = frag.trail[1];
            frag.trail[1] = frag.trail[0];
            frag.trail[0] = frag.position;

            frag.position += frag.velocity * dt_grav;
            true
        } else {
            false
        }
    });

    // Enforce maximum fragment capacity (evict oldest)
    if ejecta_pool.fragments.len() > 640 {
        ejecta_pool
            .fragments
            .drain(0..ejecta_pool.fragments.len() - 640);
    }

    // Step expanding ring structures
    ejecta_pool.rings.retain_mut(|ring| {
        ring.timer += dt_secs;
        if let Ok((pos, _, _, _)) = bodies_query.get(ring.primary_entity) {
            ring.primary_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        }
        for frag in &mut ring.fragments {
            frag.1 += frag.2 * dt_secs;
        }
        ring.timer < ring.max_timer
    });
}

/// Dynamic system collecting collision events and running the 3D molten fragment simulation.
pub fn update_impact_ejecta(
    time: Res<Time>,
    sim_time: Option<Res<SimTime>>,
    time_warp: Option<Res<TimeWarp>>,
    mut ejecta_pool: ResMut<ImpactEjectaPool>,
    mut moon_reader: MessageReader<MoonFormationEvent>,
    mut merge_reader: MessageReader<AccretionMergeEvent>,
    mut bounce_reader: MessageReader<CollisionBounceEvent>,
    bodies_query: Query<BodyQueryItem>,
    star_query: Query<(&SimPosition, &Mass), With<CentralStar>>,
) {
    if time_warp
        .as_ref()
        .is_some_and(|w| w.is_paused && !w.step_once)
    {
        return;
    }

    let dt_secs = time.delta_secs();
    let sim_dt_yr = sim_time.as_ref().map_or(0.0005, |t| t.current_dt_yr);

    collect_moon_formation_ejecta(&mut ejecta_pool, &mut moon_reader, &bodies_query);
    collect_collision_merger_ejecta(&mut ejecta_pool, &mut merge_reader, &bodies_query);
    collect_collision_bounce_ejecta(&mut ejecta_pool, &mut bounce_reader, &bodies_query);

    step_ejecta_simulation(
        &mut ejecta_pool,
        dt_secs,
        sim_dt_yr,
        &bodies_query,
        &star_query,
    );
}

/// Draws individual 3D incandescent fragments and trailing molten streamers.
fn draw_molten_fragments(gizmos: &mut Gizmos, fragments: &[MoltenDebrisFragment]) {
    for frag in fragments {
        let col = frag.blackbody_color();
        gizmos.sphere(
            Isometry3d::from_translation(frag.position),
            frag.radius,
            col,
        );

        let t_col1 = Color::srgba(
            col.to_srgba().red,
            col.to_srgba().green,
            col.to_srgba().blue,
            col.to_srgba().alpha * 0.65,
        );
        let t_col2 = Color::srgba(
            col.to_srgba().red * 0.8,
            col.to_srgba().green * 0.6,
            col.to_srgba().blue * 0.4,
            col.to_srgba().alpha * 0.28,
        );

        gizmos.line(frag.position, frag.trail[0], t_col1);
        gizmos.line(frag.trail[0], frag.trail[1], t_col1);
        gizmos.line(frag.trail[1], frag.trail[2], t_col2);
    }
}

/// Draws coalescing equatorial protolunar rings.
fn draw_ejecta_rings(gizmos: &mut Gizmos, rings: &[ImpactEjectaRing]) {
    for ring in rings {
        let p = (ring.timer / ring.max_timer).clamp(0.0, 1.0);
        let coalesce = p * p;
        let alpha = if p < 0.1 {
            p / 0.1
        } else if p > 0.8 {
            (1.0 - p) / 0.2
        } else {
            1.0
        };
        let n_len = ring.fragments.len().max(1) as f32;

        for (i, &(r, phi, _, z)) in ring.fragments.iter().enumerate() {
            let frac = i as f32 / n_len;
            let current_r = r * (1.0 - coalesce * 0.5);
            let current_z = z * (1.0 - coalesce * 0.9);
            let thermal = (1.0 - p * 0.72) * (1.0 - frac * 0.55);

            let spark_col = Color::srgba(
                1.0,
                (0.32 + thermal * 0.58).clamp(0.15, 0.96),
                (0.06 + thermal * 0.45).clamp(0.02, 0.82),
                alpha * 0.92,
            );
            let line_col = Color::srgba(
                0.98,
                (0.24 + thermal * 0.45).clamp(0.10, 0.85),
                (0.04 + thermal * 0.25).clamp(0.01, 0.55),
                alpha * 0.48,
            );

            let pos1 = ring.primary_pos
                + Vec3::new(current_r * phi.cos(), current_z, current_r * phi.sin());
            gizmos.sphere(
                Isometry3d::from_translation(pos1),
                (0.008 + 0.006 * (1.0 - p)) * (1.0 - frac * 0.35),
                spark_col,
            );

            if let Some(&(r2, phi2, _, z2)) = ring.fragments.get(i + 1) {
                let cr2 = r2 * (1.0 - coalesce * 0.5);
                let cz2 = z2 * (1.0 - coalesce * 0.9);
                let pos2 = ring.primary_pos + Vec3::new(cr2 * phi2.cos(), cz2, cr2 * phi2.sin());
                gizmos.line(pos1, pos2, line_col);
            }
        }
    }
}

/// Renders all active molten collision fragments, Keplerian ribbons, and ejecta rings.
pub fn draw_impact_ejecta_system(mut gizmos: Gizmos, ejecta_pool: Option<Res<ImpactEjectaPool>>) {
    let Some(pool) = ejecta_pool else {
        return;
    };
    if !pool.fragments.is_empty() {
        draw_molten_fragments(&mut gizmos, &pool.fragments);
    }
    if !pool.rings.is_empty() {
        draw_ejecta_rings(&mut gizmos, &pool.rings);
    }
}
