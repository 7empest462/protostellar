use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::rendering::materials::PlanetMaterial;
use crate::simulation::accretion::events::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::terraforming::{BombardmentEvent, BombardmentType};

type BodyQueryItem<'w> = (
    Entity,
    &'static SimPosition,
    Option<&'static SimVelocity>,
    Option<&'static Mass>,
    Option<&'static Radius>,
    Option<&'static Transform>,
    Option<&'static PlanetaryBasins>,
    Option<&'static MeshMaterial3d<PlanetMaterial>>,
);

/// Characteristic dynamical frequency (rad/s) for real-time ballistic ejecta arcs around a planet's visual surface.
const EJECTA_SURFACE_OMEGA: f32 = 2.4;

/// Resolves the effective surface radius for ejecta launch and reabsorption:
/// uses the rendered 3D mesh visual radius (`Transform.scale.x`) when present in-game,
/// and falls back to physical `Radius` in headless simulation tests.
fn effective_surface_radius(p_rad: Option<&Radius>, p_transform: Option<&Transform>) -> f32 {
    let phys_r = p_rad.map_or(4.2e-5, |r| r.0 as f32).max(1e-5);
    if let Some(tf) = p_transform {
        let vis_r = tf.scale.x;
        if vis_r > phys_r && (vis_r - 1.0).abs() > 1e-4 {
            return vis_r;
        }
    }
    phys_r
}

/// Maps a local `p_surf` crater normal to the 3D world-space normal on the rendered planet sphere,
/// inverting the spin-axis and `spin_angle` transform in `planet.wgsl`.
pub fn p_surf_to_world_normal(p_surf_dir: Vec3, spin_axis_vec4: Vec4, spin_angle: f32) -> Vec3 {
    let dir = p_surf_dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return Vec3::X;
    }
    let mut s_axis = spin_axis_vec4.xyz();
    if s_axis.length() < 0.1 {
        s_axis = Vec3::Y;
    } else {
        s_axis = s_axis.normalize();
    }
    let s_ref = if s_axis.z.abs() > 0.95 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let s_tangent_x = s_axis.cross(s_ref).normalize_or_zero();
    let s_tangent_z = s_tangent_x.cross(s_axis);

    let c = (-spin_angle).cos();
    let s = (-spin_angle).sin();
    let p_tilted = Vec3::new(c * dir.x + s * dir.z, dir.y, -s * dir.x + c * dir.z);
    let world_n = s_tangent_x * p_tilted.x + s_axis * p_tilted.y + s_tangent_z * p_tilted.z;
    let norm = world_n.normalize_or_zero();
    if norm == Vec3::ZERO {
        dir
    } else {
        norm
    }
}

/// Resolves the 3D world-space impact normal for a planet entity, aligning with its rendered shader crater if available.
fn resolve_entity_world_impact_normal(
    raw_normal: Vec3,
    opt_basins: Option<&PlanetaryBasins>,
    opt_mat_handle: Option<&MeshMaterial3d<PlanetMaterial>>,
    opt_materials: Option<&Assets<PlanetMaterial>>,
) -> Vec3 {
    let base_dir = opt_basins
        .and_then(|pb| pb.basins.last())
        .map(|b| b.surface_normal.normalize_or_zero())
        .filter(|v| *v != Vec3::ZERO)
        .unwrap_or_else(|| {
            let n = raw_normal.normalize_or_zero();
            if n == Vec3::ZERO {
                Vec3::X
            } else {
                n
            }
        });

    if let Some(mat) = opt_mat_handle
        .zip(opt_materials)
        .and_then(|(handle, materials)| materials.get(&handle.0))
    {
        return p_surf_to_world_normal(
            base_dir,
            mat.extension.uniforms.spin_axis,
            mat.extension.uniforms.spin_angle,
        );
    }
    base_dir
}

/// Spawns a 3D sub-orbital ballistic fountain of molten planetary ejecta from a surface impact site.
/// Fragments blast outward into space ($1.35\,R \dots 2.85\,R$), cool radiatively, curve back under
/// the planet's gravity, and get reabsorbed upon re-impacting the surface.
fn spawn_ballistic_ejecta_fountain(
    ejecta_pool: &mut ImpactEjectaPool,
    p_ent: Entity,
    p_center: Vec3,
    r_surf: f32,
    world_normal: Vec3,
    n_spawn: usize,
    base_temp: f32,
    total_ejecta_mass: f64,
    opt_bombard_type: Option<BombardmentType>,
) {
    let normal = if world_normal.length_squared() > 1e-6 {
        world_normal.normalize()
    } else {
        Vec3::X
    };

    // Avoid duplicate spawning if an event and basin update trigger on the same impact frame
    let already_spawned = ejecta_pool.fragments.iter().any(|f| {
        f.primary_entity == p_ent
            && f.age < 0.25
            && f.local_position.normalize_or_zero().dot(normal) > 0.65
    });
    if already_spawned {
        return;
    }

    let up = if normal.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let tangent = normal.cross(up).normalize_or_zero();
    let bitangent = normal.cross(tangent).normalize_or_zero();
    let v_circ_eff = EJECTA_SURFACE_OMEGA * r_surf;

    let color_tint = match opt_bombard_type {
        Some(BombardmentType::IcyComet) => Some(Color::srgba(0.85, 0.95, 1.0, 0.95)),
        Some(BombardmentType::CarbonaceousChondrite) => Some(Color::srgba(1.0, 0.68, 0.32, 0.95)),
        Some(BombardmentType::VolatileAblationSalvo) => Some(Color::srgba(0.65, 0.82, 1.0, 0.90)),
        Some(BombardmentType::IronAsteroid) => Some(Color::srgba(1.0, 0.98, 0.88, 0.98)),
        None => None,
    };

    for i in 0..n_spawn {
        let frac = i as f32 / (n_spawn.max(1) as f32);
        let az = frac * TAU + (i as f32 * 0.73).sin() * 0.32;
        // Conical ejecta curtain: 13 deg to 45 deg from the impact normal (p = h^2/mu < R guarantees 100% surface reabsorption)
        let cone_angle = 0.22 + ((i as f32 * 1.53).cos() * 0.5 + 0.5) * 0.56;
        let dir = (normal * cone_angle.cos()
            + (tangent * az.cos() + bitangent * az.sin()) * cone_angle.sin())
        .normalize_or_zero();

        // Strictly sub-orbital speeds (0.62..0.96 * v_circ) so fragments blast out to 1.4R..2.05R
        // in space and 100% fall back into the planet within ~2.5 seconds
        let speed_frac = (i as f32 * 2.31).sin() * 0.5 + 0.5;
        let speed = v_circ_eff * (0.62 + speed_frac * 0.34);
        let local_pos = normal * (r_surf * 1.02) + dir * (r_surf * 0.015);
        let local_vel = dir * speed;
        let world_pos = p_center + local_pos;
        let temp = base_temp + ((i as f32 * 1.9).sin() * 0.5 + 0.5) * 450.0;
        let size_frac = (i as f32 * 1.17).cos() * 0.5 + 0.5;

        ejecta_pool.fragments.push(MoltenDebrisFragment {
            position: world_pos,
            velocity: local_vel,
            initial_temperature: temp,
            temperature: temp,
            cooling_rate: 0.42 + ((i as f32 * 0.5).sin() * 0.5 + 0.5) * 0.28,
            radius: (r_surf * (0.024 + size_frac * 0.034)).clamp(0.0002, 0.012),
            mass: total_ejecta_mass / f64::from(n_spawn.max(1) as u32),
            primary_entity: p_ent,
            age: 0.0,
            max_age: 4.5 + ((i as f32 * 1.2).sin() * 0.5 + 0.5) * 3.5,
            trail: [world_pos; 4],
            local_position: local_pos,
            local_velocity: local_vel,
            local_trail: [local_pos; 4],
            color_tint,
        });
    }
}

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
        if let Ok((_, p_pos, _, _, p_rad, p_tf, _, _)) = bodies_query.get(ev.parent_entity) {
            let p_center = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
            let r_surf = effective_surface_radius(p_rad, p_tf);
            let v_circ_surf = EJECTA_SURFACE_OMEGA * r_surf;
            let disk_in = inner_r.max(r_surf * 1.35);
            let disk_out = outer_r.max(disk_in + r_surf * 1.2);
            let n_molten = 64;

            for i in 0..n_molten {
                let frac = i as f32 / n_molten as f32;
                let r_dist = disk_in + (disk_out - disk_in) * frac;
                let theta = frac * TAU + (i as f32 * 0.35).sin() * 0.15;
                let v_circ = v_circ_surf * (r_surf / r_dist.max(1e-5)).sqrt();
                let v_dir = Vec3::new(-theta.sin(), 0.0, theta.cos());
                let vel = v_dir * v_circ * (0.92 + (i as f32 * 1.7).sin() * 0.14);

                let temp = 2800.0 + (i as f32 * 1.3).sin() * 600.0;
                let local_pos = Vec3::new(
                    r_dist * theta.cos(),
                    ((i as f32 * 2.1).sin() * 0.04) * r_dist,
                    r_dist * theta.sin(),
                );
                let world_pos = p_center + local_pos;

                ejecta_pool.fragments.push(MoltenDebrisFragment {
                    position: world_pos,
                    velocity: vel,
                    initial_temperature: temp,
                    temperature: temp,
                    cooling_rate: 0.12 + ((i as f32 * 0.9).sin() * 0.5 + 0.5) * 0.08,
                    radius: (r_surf * 0.032).clamp(0.0005, 0.008),
                    mass: ev.moon_mass / f64::from(n_molten),
                    primary_entity: ev.parent_entity,
                    age: 0.0,
                    max_age: 18.0 + ((i as f32 * 1.4).sin() * 0.5 + 0.5) * 12.0,
                    trail: [world_pos; 4],
                    local_position: local_pos,
                    local_velocity: vel,
                    local_trail: [local_pos; 4],
                    color_tint: None,
                });
            }
        }
    }
}

/// Spawns 3D ballistic molten fragments from violent planetesimal, comet, asteroid, and planetary mergers.
fn collect_collision_merger_ejecta(
    ejecta_pool: &mut ImpactEjectaPool,
    merge_reader: &mut MessageReader<AccretionMergeEvent>,
    bodies_query: &Query<BodyQueryItem>,
    opt_materials: Option<&Assets<PlanetMaterial>>,
) {
    for ev in merge_reader.read() {
        let p_ent = ev.primary_entity;
        let Ok((_, p_pos, _, _, p_rad, p_tf, opt_basins, opt_mat_handle)) = bodies_query.get(p_ent)
        else {
            continue;
        };

        let p_center = p_tf.map_or(
            Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
            |tf| tf.translation,
        );
        let r_surf = effective_surface_radius(p_rad, p_tf);
        let imp_pos = Vec3::new(
            ev.merged_position.x as f32,
            ev.merged_position.y as f32,
            ev.merged_position.z as f32,
        );
        let raw_normal = (imp_pos - p_center).normalize_or_zero();
        let world_normal = resolve_entity_world_impact_normal(
            raw_normal,
            opt_basins,
            opt_mat_handle,
            opt_materials,
        );

        let n_spawn = ((ev.merged_mass * 50_000.0).clamp(24.0, 64.0)) as usize;
        let base_temp = (2400.0 + (ev.energy_released.log10().max(20.0) - 20.0) as f32 * 120.0)
            .clamp(2500.0, 3800.0);

        spawn_ballistic_ejecta_fountain(
            ejecta_pool,
            p_ent,
            p_center,
            r_surf,
            world_normal,
            n_spawn,
            base_temp,
            ev.merged_mass * 0.005,
            None,
        );
    }
}

/// Spawns 3D ballistic ejecta fountains immediately upon guided bombardment impact.
fn collect_bombardment_ejecta(
    ejecta_pool: &mut ImpactEjectaPool,
    bombard_reader: &mut MessageReader<BombardmentEvent>,
    bodies_query: &Query<BodyQueryItem>,
    opt_materials: Option<&Assets<PlanetMaterial>>,
) {
    for ev in bombard_reader.read() {
        let Ok((p_ent, p_pos, _, p_mass, p_rad, p_tf, opt_basins, opt_mat_handle)) =
            bodies_query.get(ev.target_entity)
        else {
            continue;
        };

        let p_center = p_tf.map_or(
            Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
            |tf| tf.translation,
        );
        let r_surf = effective_surface_radius(p_rad, p_tf);
        let world_normal = resolve_entity_world_impact_normal(
            ev.surface_normal,
            opt_basins,
            opt_mat_handle,
            opt_materials,
        );

        let (n_spawn, base_temp, mass_ejecta): (usize, f32, f64) = match ev.bombardment_type {
            BombardmentType::IcyComet => (36, 2800.0, 4.0e-6),
            BombardmentType::CarbonaceousChondrite => (30, 2600.0, 3.5e-6),
            BombardmentType::VolatileAblationSalvo => (24, 2200.0, 2.5e-6),
            BombardmentType::IronAsteroid => (48, 3600.0, 8.0e-6),
        };

        let m_prim = p_mass.map_or(3e-6, |m| m.0);
        let total_mass = mass_ejecta.min(m_prim * 1e-4);

        spawn_ballistic_ejecta_fountain(
            ejecta_pool,
            p_ent,
            p_center,
            r_surf,
            world_normal,
            n_spawn,
            base_temp,
            total_mass,
            Some(ev.bombardment_type),
        );
    }
}

/// Spawns 3D ballistic ejecta fountains for significant comet/asteroid strikes recorded in `PlanetaryBasins`
/// (including Late Heavy Bombardment comets and player-launched terraforming comet/asteroid strikes).
/// Uses `triggered_basins` so each impact basin triggers a single one-shot ejecta blast even in Real-Time.
fn collect_basin_impact_ejecta(
    ejecta_pool: &mut ImpactEjectaPool,
    bodies_query: &Query<BodyQueryItem>,
    opt_materials: Option<&Assets<PlanetMaterial>>,
    triggered_basins: &mut Vec<(Entity, Vec3)>,
) {
    for (p_ent, p_pos, _, p_mass, p_rad, p_tf, opt_basins, opt_mat_handle) in bodies_query.iter() {
        let Some(pb) = opt_basins else {
            continue;
        };
        let Some(latest) = pb.basins.last() else {
            continue;
        };
        // Trigger only for fresh impacts of sufficiently large comets or asteroids
        if latest.melt_glow_fraction < 0.96 || latest.angular_radius < 0.038 {
            continue;
        }

        let norm_key = latest.surface_normal.normalize_or_zero();
        let already_triggered = triggered_basins
            .iter()
            .any(|(ent, n)| *ent == p_ent && n.dot(norm_key) > 0.999);
        if already_triggered {
            continue;
        }
        if triggered_basins.len() >= 64 {
            triggered_basins.remove(0);
        }
        triggered_basins.push((p_ent, norm_key));

        let p_center = p_tf.map_or(
            Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
            |tf| tf.translation,
        );
        let r_surf = effective_surface_radius(p_rad, p_tf);
        let world_normal = resolve_entity_world_impact_normal(
            latest.surface_normal,
            opt_basins,
            opt_mat_handle,
            opt_materials,
        );

        let size_factor = ((latest.angular_radius - 0.035) / 0.25).clamp(0.15, 1.0);
        let n_spawn = (16.0 + size_factor * 28.0) as usize;
        let base_temp = 2600.0 + size_factor * 900.0;
        let m_prim = p_mass.map_or(3e-6, |m| m.0);

        spawn_ballistic_ejecta_fountain(
            ejecta_pool,
            p_ent,
            p_center,
            r_surf,
            world_normal,
            n_spawn,
            base_temp,
            m_prim * 1e-4 * f64::from(size_factor),
            None,
        );
    }
}

/// Spawns grazing spray ejecta fragments from non-merging bounce collisions.
fn collect_collision_bounce_ejecta(
    ejecta_pool: &mut ImpactEjectaPool,
    bounce_reader: &mut MessageReader<CollisionBounceEvent>,
    bodies_query: &Query<BodyQueryItem>,
) {
    for ev in bounce_reader.read() {
        let (Ok((_, pos1, _, _, r1, tf1, _, _)), Ok((_, pos2, _, _, _, _, _, _))) =
            (bodies_query.get(ev.entity1), bodies_query.get(ev.entity2))
        else {
            continue;
        };

        let p1 = Vec3::new(pos1.x as f32, pos1.y as f32, pos1.z as f32);
        let p2 = Vec3::new(pos2.x as f32, pos2.y as f32, pos2.z as f32);
        let r_surf = effective_surface_radius(r1, tf1);

        let spray_dir = {
            let d = (p2 - p1).normalize_or_zero();
            if d == Vec3::ZERO {
                Vec3::X
            } else {
                d
            }
        };
        let up = if spray_dir.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let side = spray_dir.cross(up).normalize_or_zero();

        let v_circ = EJECTA_SURFACE_OMEGA * r_surf;
        let n_spray = 20;

        for i in 0..n_spray {
            let angle = (i as f32 / n_spray as f32 - 0.5) * 1.4;
            let dir = (spray_dir * angle.cos() + side * angle.sin()).normalize_or_zero();
            let speed = v_circ * (0.78 + ((i as f32 * 1.7).sin() * 0.5 + 0.5) * 0.48);
            let local_pos = spray_dir * (r_surf * 1.04) + dir * (r_surf * 0.04);
            let local_vel = dir * speed;
            let world_pos = p1 + local_pos;
            let temp = 2400.0 + ((i as f32 * 1.3).sin() * 0.5 + 0.5) * 600.0;

            ejecta_pool.fragments.push(MoltenDebrisFragment {
                position: world_pos,
                velocity: local_vel,
                initial_temperature: temp,
                temperature: temp,
                cooling_rate: 0.38,
                radius: (r_surf * 0.025).clamp(0.0002, 0.008),
                mass: 1e-10,
                primary_entity: ev.entity1,
                age: 0.0,
                max_age: 7.0,
                trail: [world_pos; 4],
                local_position: local_pos,
                local_velocity: local_vel,
                local_trail: [local_pos; 4],
                color_tint: None,
            });
        }
    }
}

/// Integrates orbital gravitational trajectories, radiative cooling, and surface reabsorption.
fn step_ejecta_simulation(
    ejecta_pool: &mut ImpactEjectaPool,
    dt_secs: f32,
    bodies_query: &Query<BodyQueryItem>,
) {
    let dt = if dt_secs > 1e-3 {
        dt_secs.min(0.05)
    } else {
        1.0 / 60.0
    };

    ejecta_pool.fragments.retain_mut(|frag| {
        frag.age += dt;
        if frag.age >= frag.max_age {
            return false;
        }

        // Exponential thermal radiative blackbody cooling
        let cool_decay = (-frag.cooling_rate * frag.age).exp();
        frag.temperature = 200.0 + (frag.initial_temperature - 200.0) * cool_decay;

        // Gravitational acceleration toward primary planet and surface reabsorption in planet local frame
        if let Ok((_, p_pos, _, _, p_rad, p_tf, _, _)) = bodies_query.get(frag.primary_entity) {
            let p_center = p_tf.map_or(
                Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
                |tf| tf.translation,
            );
            let r_surf = effective_surface_radius(p_rad, p_tf);

            // Backward compatibility for fragments initialized without local coordinates
            if frag.local_position == Vec3::ZERO && frag.position != Vec3::ZERO {
                frag.local_position = frag.position - p_center;
                frag.local_velocity = frag.velocity;
                frag.local_trail = [frag.local_position; 4];
            }

            let to_center = -frag.local_position;
            let dist = frag.local_position.length();

            // Re-impact / reabsorption detection: fragment plunged back into the planet's surface
            let is_falling_inward = to_center.dot(frag.local_velocity) >= -1e-6;
            if dist <= r_surf * 0.98 || (dist <= r_surf * 1.02 && is_falling_inward) {
                return false;
            }

            let mu_eff = EJECTA_SURFACE_OMEGA * EJECTA_SURFACE_OMEGA * r_surf * r_surf * r_surf;
            let inv_r3 = 1.0 / (dist * dist * dist).max(1e-18);
            let a_prim = to_center * (mu_eff * inv_r3);
            frag.local_velocity += a_prim * dt;

            // Shift trailing history
            frag.local_trail[3] = frag.local_trail[2];
            frag.local_trail[2] = frag.local_trail[1];
            frag.local_trail[1] = frag.local_trail[0];
            frag.local_trail[0] = frag.local_position;

            frag.local_position += frag.local_velocity * dt;

            // Synchronize world coordinates
            frag.position = p_center + frag.local_position;
            frag.trail[0] = p_center + frag.local_trail[0];
            frag.trail[1] = p_center + frag.local_trail[1];
            frag.trail[2] = p_center + frag.local_trail[2];
            frag.trail[3] = p_center + frag.local_trail[3];
            frag.velocity = frag.local_velocity;

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
        ring.timer += dt;
        if let Ok((_, pos, _, _, _, p_tf, _, _)) = bodies_query.get(ring.primary_entity) {
            ring.primary_pos = p_tf
                .map_or(Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32), |tf| {
                    tf.translation
                });
        }
        for frag in &mut ring.fragments {
            frag.1 += frag.2 * dt;
        }
        ring.timer < ring.max_timer
    });
}

/// Dynamic system collecting collision events and running the 3D molten fragment simulation.
#[allow(clippy::too_many_arguments, reason = "Bevy ECS system parameters")]
pub fn update_impact_ejecta(
    time: Res<Time>,
    _sim_time: Option<Res<SimTime>>,
    time_warp: Option<Res<TimeWarp>>,
    opt_materials: Option<Res<Assets<PlanetMaterial>>>,
    mut ejecta_pool: ResMut<ImpactEjectaPool>,
    mut moon_reader: MessageReader<MoonFormationEvent>,
    mut merge_reader: MessageReader<AccretionMergeEvent>,
    mut bounce_reader: MessageReader<CollisionBounceEvent>,
    mut bombard_reader: MessageReader<BombardmentEvent>,
    bodies_query: Query<BodyQueryItem>,
    _star_query: Query<(&SimPosition, &Mass), With<CentralStar>>,
    mut triggered_basins: Local<Vec<(Entity, Vec3)>>,
) {
    if time_warp
        .as_ref()
        .is_some_and(|w| w.is_paused && !w.step_once)
    {
        return;
    }

    let dt_secs = time.delta_secs();
    let materials_ref = opt_materials.as_deref();

    collect_moon_formation_ejecta(&mut ejecta_pool, &mut moon_reader, &bodies_query);
    collect_collision_merger_ejecta(
        &mut ejecta_pool,
        &mut merge_reader,
        &bodies_query,
        materials_ref,
    );
    collect_bombardment_ejecta(
        &mut ejecta_pool,
        &mut bombard_reader,
        &bodies_query,
        materials_ref,
    );
    collect_basin_impact_ejecta(
        &mut ejecta_pool,
        &bodies_query,
        materials_ref,
        &mut triggered_basins,
    );
    collect_collision_bounce_ejecta(&mut ejecta_pool, &mut bounce_reader, &bodies_query);

    step_ejecta_simulation(&mut ejecta_pool, dt_secs, &bodies_query);
}

/// Draws individual 3D incandescent fragments as glowing ballistic streaks with trailing molten streamers
/// (using zero wireframe gizmo spheres so no circular wire cages / "ball of string" ever appear).
fn draw_molten_fragments(
    gizmos: &mut Gizmos,
    fragments: &[MoltenDebrisFragment],
    transforms_query: &Query<&Transform>,
) {
    let mut cached_ent = Entity::PLACEHOLDER;
    let mut p_center = Vec3::ZERO;
    let mut has_center = false;

    for frag in fragments {
        if frag.primary_entity != cached_ent {
            cached_ent = frag.primary_entity;
            if let Ok(tf) = transforms_query.get(cached_ent) {
                p_center = tf.translation;
                has_center = true;
            } else {
                has_center = false;
            }
        }

        let (pos, t0, t1, t2, t3) = if has_center {
            (
                p_center + frag.local_position,
                p_center + frag.local_trail[0],
                p_center + frag.local_trail[1],
                p_center + frag.local_trail[2],
                p_center + frag.local_trail[3],
            )
        } else {
            (
                frag.position,
                frag.trail[0],
                frag.trail[1],
                frag.trail[2],
                frag.trail[3],
            )
        };

        let col = if let Some(tint) = frag.color_tint {
            let bb = frag.blackbody_color().to_srgba();
            let tr = tint.to_srgba();
            Color::srgba(
                (bb.red * 0.45 + tr.red * 0.55).min(1.0),
                (bb.green * 0.45 + tr.green * 0.55).min(1.0),
                (bb.blue * 0.35 + tr.blue * 0.65).min(1.0),
                bb.alpha * tr.alpha,
            )
        } else {
            frag.blackbody_color()
        };
        let srgba = col.to_srgba();

        // Bright incandescent leading head bolt along the velocity vector
        let vel_dir = frag.local_velocity.normalize_or_zero();
        let head_len = (frag.radius * 0.45).clamp(0.00015, 0.0025);
        let head_col = Color::srgba(
            (srgba.red * 1.15).min(1.0),
            (srgba.green * 1.15 + 0.12).min(1.0),
            (srgba.blue * 1.15 + 0.20).min(1.0),
            srgba.alpha,
        );
        gizmos.line(pos + vel_dir * head_len, pos, head_col);

        let t_col1 = Color::srgba(srgba.red, srgba.green, srgba.blue, srgba.alpha * 0.85);
        let t_col2 = Color::srgba(
            srgba.red * 0.9,
            srgba.green * 0.65,
            srgba.blue * 0.4,
            srgba.alpha * 0.48,
        );
        let t_col3 = Color::srgba(
            srgba.red * 0.75,
            srgba.green * 0.4,
            srgba.blue * 0.2,
            srgba.alpha * 0.20,
        );

        gizmos.line(pos, t0, col);
        gizmos.line(t0, t1, t_col1);
        gizmos.line(t1, t2, t_col2);
        gizmos.line(t2, t3, t_col3);
    }
}

/// Draws coalescing equatorial protolunar rings.
fn draw_ejecta_rings(
    gizmos: &mut Gizmos,
    rings: &[ImpactEjectaRing],
    transforms_query: &Query<&Transform>,
) {
    for ring in rings {
        let p_center = transforms_query
            .get(ring.primary_entity)
            .map_or(ring.primary_pos, |tf| tf.translation);
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

            let pos1 =
                p_center + Vec3::new(current_r * phi.cos(), current_z, current_r * phi.sin());
            let tangent = Vec3::new(-phi.sin(), 0.0, phi.cos());
            let arc_half = (current_r * 0.14).clamp(0.00008, 0.0035) * (1.0 - frac * 0.35);
            gizmos.line(
                pos1 - tangent * arc_half,
                pos1 + tangent * arc_half,
                spark_col,
            );

            if let Some(&(r2, phi2, _, z2)) = ring.fragments.get(i + 1) {
                let cr2 = r2 * (1.0 - coalesce * 0.5);
                let cz2 = z2 * (1.0 - coalesce * 0.9);
                let pos2 = p_center + Vec3::new(cr2 * phi2.cos(), cz2, cr2 * phi2.sin());
                if pos1.distance(pos2) <= current_r * 0.65 {
                    gizmos.line(pos1, pos2, line_col);
                }
            }
        }
    }
}

/// Renders all active molten collision fragments, Keplerian ribbons, and ejecta rings.
pub fn draw_impact_ejecta_system(
    mut gizmos: Gizmos,
    ejecta_pool: Option<Res<ImpactEjectaPool>>,
    transforms_query: Query<&Transform>,
) {
    let Some(pool) = ejecta_pool else {
        return;
    };
    if !pool.fragments.is_empty() {
        draw_molten_fragments(&mut gizmos, &pool.fragments, &transforms_query);
    }
    if !pool.rings.is_empty() {
        draw_ejecta_rings(&mut gizmos, &pool.rings, &transforms_query);
    }
}
