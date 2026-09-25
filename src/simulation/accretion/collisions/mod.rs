//! Collision detection and accretion physics processing.

pub mod handlers;
pub mod types;

pub use handlers::*;
pub use types::*;

use bevy::math::DVec3;
use bevy::prelude::*;
use hashbrown::HashSet;
use smallvec::SmallVec;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::events::*;
use super::impact_regimes::{
    classify_impact, radius_from_mass_density, ImpactParams, ImpactRegime,
};

/// Detects close-contact collisions and processes physical collision regimes.
pub fn process_accretion_and_collisions(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut merge_events: MessageWriter<AccretionMergeEvent>,
    mut moon_events: MessageWriter<MoonFormationEvent>,
    mut bounce_events: MessageWriter<CollisionBounceEvent>,
    mut roche_events: MessageWriter<RocheDisruptionEvent>,
    mut bodies_query: AccretionQuery,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let mut star_mass = 1.0;
    let mut snapshots: Vec<BodySnapshot> = Vec::with_capacity(bodies_query.iter().len());
    for (e, m, pos, vel, _, rad, temp, comp, body, _, spin, sat, star) in bodies_query.iter() {
        if star.is_some() {
            star_mass = m.0;
        }
        snapshots.push(BodySnapshot {
            entity: e,
            mass: m.0,
            pos: pos.0,
            vel: vel.0,
            radius: rad.0,
            temp: temp.0,
            comp: *comp,
            body_type: body.body_type,
            spin: spin.map_or(DVec3::ZERO, |s| s.spin_vector),
            name: body.name.clone(),
            is_central: star.is_some(),
            satellite_parent: sat.map(|s| s.parent),
        });
    }

    let mut merged_away: HashSet<Entity> = HashSet::default();
    let mut newly_formed_moons: HashSet<Entity> = HashSet::default();
    let mut pending_despawns: SmallVec<[Entity; 32]> = SmallVec::new();

    let n = snapshots.len();
    for i in 0..n {
        let Some(e1) = snapshots.get(i).map(|s| s.entity) else {
            continue;
        };
        if merged_away.contains(&e1) || newly_formed_moons.contains(&e1) {
            continue;
        }

        for j in (i + 1)..n {
            let Some(e2) = snapshots.get(j).map(|s| s.entity) else {
                continue;
            };
            if merged_away.contains(&e2)
                || merged_away.contains(&e1)
                || newly_formed_moons.contains(&e2)
                || newly_formed_moons.contains(&e1)
            {
                continue;
            }

            let Some(b2) = snapshots.get(j).cloned() else {
                continue;
            };
            let Some(b1) = snapshots.get_mut(i) else {
                continue;
            };

            let mut ctx = CollisionContext {
                commands: &mut commands,
                bodies_query: &mut bodies_query,
                player_state: &mut player_state,
                merge_events: &mut merge_events,
                moon_events: &mut moon_events,
                bounce_events: &mut bounce_events,
                roche_events: &mut roche_events,
                merged_away: &mut merged_away,
                newly_formed_moons: &mut newly_formed_moons,
                pending_despawns: &mut pending_despawns,
                star_mass,
                sim_time_years: sim_time.elapsed_years,
            };

            process_body_pair(&mut ctx, &config, &time_warp, b1, &b2);
        }
    }

    for entity in pending_despawns {
        if let Ok(mut entity_cmd) = commands.get_entity(entity) {
            entity_cmd.despawn();
        }
    }
}

pub fn is_physically_interpenetrating(
    dist: f64,
    min_dist: f64,
    r1: f64,
    r2: f64,
    r_contact: f64,
) -> bool {
    let physical_contact = r1 + r2;
    dist < physical_contact * 0.90
        || dist < r1.max(r2) * 1.05
        || dist < r_contact * 0.95
        || (min_dist < physical_contact * 0.85 && dist < r_contact * 1.25)
}

fn compute_closest_approach(r_rel: DVec3, v_rel_vec: DVec3, dist: f64, dt: f64) -> (f64, DVec3) {
    let r_rel_old = r_rel - v_rel_vec * dt;
    let v_rel_sq = v_rel_vec.length_squared();
    let mut min_dist = dist;
    let mut r_closest = r_rel;

    if v_rel_sq > 1e-12 {
        let t_min = -r_rel_old.dot(v_rel_vec) / v_rel_sq;
        if t_min > 0.0 && t_min < dt {
            let r_at_t = r_rel_old + v_rel_vec * t_min;
            let d_at_t = r_at_t.length();
            if d_at_t < min_dist {
                r_closest = r_at_t;
                min_dist = d_at_t;
            }
        } else {
            let d_old = r_rel_old.length();
            if d_old < min_dist {
                r_closest = r_rel_old;
                min_dist = d_old;
            }
        }
    }
    (min_dist, r_closest)
}

fn compute_effective_collision_radius(
    config: &SimulationConfig,
    star_mass: f64,
    b1: &BodySnapshot,
    b2: &BodySnapshot,
) -> (f64, f64, f64, f64) {
    let is_minor = |t: BodyType| {
        matches!(
            t,
            BodyType::Asteroid
                | BodyType::Comet
                | BodyType::Planetesimal
                | BodyType::DustGrain
                | BodyType::DebrisRing
        )
    };

    let is_stellar = b1.is_central
        || b2.is_central
        || b1.body_type.is_star_or_remnant()
        || b2.body_type.is_star_or_remnant();
    let both_major = b1.body_type.is_planet() && b2.body_type.is_planet();

    let r_phys = b1.radius + b2.radius;
    let r_contact = if is_stellar {
        let (star_b, other_b) = if b1.is_central || b1.body_type.is_star_or_remnant() {
            (b1, b2)
        } else {
            (b2, b1)
        };
        let r_orb = (other_b.pos.x * other_b.pos.x + other_b.pos.z * other_b.pos.z).sqrt() as f32;
        let r_star_vis = f64::from(config.calc_visual_radius_with_orbit(
            star_b.radius,
            star_b.body_type,
            0.0,
            r_orb.max(0.001),
        ));
        let r_other_vis = f64::from(config.calc_visual_radius_with_orbit(
            other_b.radius,
            other_b.body_type,
            r_orb,
            r_orb.max(0.001),
        ));
        // Devourment occurs if body plunges into the star's visual photosphere
        (r_star_vis + other_b.radius.max(r_other_vis * 0.40)).max(r_phys)
    } else if both_major {
        let r_orb1 = (b1.pos.x * b1.pos.x + b1.pos.z * b1.pos.z).sqrt() as f32;
        let r_orb2 = (b2.pos.x * b2.pos.x + b2.pos.z * b2.pos.z).sqrt() as f32;
        let min_r = r_orb1.min(r_orb2).max(0.001);
        let r1_vis = f64::from(config.calc_visual_radius_with_orbit(
            b1.radius,
            b1.body_type,
            r_orb1,
            min_r,
        ));
        let r2_vis = f64::from(config.calc_visual_radius_with_orbit(
            b2.radius,
            b2.body_type,
            r_orb2,
            min_r,
        ));
        // Major planets collide if physically touching or passing halfway through each other visually
        ((r1_vis + r2_vis) * 0.40).max(r_phys)
    } else {
        let r1_vis = f64::from(config.calc_visual_radius_for_type(b1.radius, b1.body_type));
        let r2_vis = f64::from(config.calc_visual_radius_for_type(b2.radius, b2.body_type));
        let r_orb = (b1.pos.length().min(b2.pos.length())).max(0.01);
        let max_minor_r = r_orb * 0.15;
        ((r1_vis + r2_vis) * 0.35).min(max_minor_r).max(r_phys)
    };

    let v_esc = (2.0 * G_ASTRO * (b1.mass + b2.mass) / r_contact.max(1e-6)).sqrt();
    let v_rel_vec = b1.vel - b2.vel;
    let v_rel = v_rel_vec.length();

    let safronov_factor = if is_stellar || both_major {
        1.0
    } else {
        1.0 + (v_esc * v_esc) / (v_rel * v_rel + 1e-4)
    };
    let capture_r = if (b1.body_type.is_planet() && is_minor(b2.body_type))
        || (b2.body_type.is_planet() && is_minor(b1.body_type))
    {
        let (p_m, p_pos) = if b1.body_type.is_planet() {
            (b1.mass, b1.pos)
        } else {
            (b2.mass, b2.pos)
        };
        let r_orb = (p_pos.x * p_pos.x + p_pos.z * p_pos.z).sqrt().max(0.1);
        r_orb * (p_m / (3.0 * star_mass.max(0.01))).cbrt() * 0.40
    } else {
        0.0
    };

    let effective_collision_radius = (r_contact * safronov_factor.sqrt())
        .max(r_contact)
        .max(capture_r)
        .min(r_contact * 3.0 + capture_r);

    (r_contact, v_esc, v_rel, effective_collision_radius)
}

fn is_theia_earth_snapshot(b1: &BodySnapshot, b2: &BodySnapshot) -> bool {
    let is_earth = |b: &BodySnapshot| {
        let n = b.name.as_str();
        let is_explicit = n == "Earth"
            || n == "Proto-Earth"
            || n.starts_with("Proto-Earth")
            || n.starts_with("Earth (")
            || (n.contains("Earth") && !n.contains("Super-Earth"));
        let is_procedural =
            b.pos.length() >= 0.85 && b.pos.length() <= 1.15 && b.body_type.is_planet();
        let is_excluded = n.contains("Planet Nine")
            || n.contains("Planet 9")
            || n.contains("Super-Earth")
            || n.contains("Theia")
            || n.contains("Moon")
            || n.contains("Venus")
            || n.contains("Mercury")
            || n.contains("Mars")
            || n.contains("Jupiter")
            || n.contains("Saturn")
            || n.contains("Uranus")
            || n.contains("Neptune")
            || n.contains("Pluto");
        (is_explicit || is_procedural) && !is_excluded
    };
    let is_theia = |b: &BodySnapshot| {
        crate::simulation::accretion::theia::is_explicit_theia(b.name.as_str())
    };
    (is_theia(b1) && is_earth(b2)) || (is_theia(b2) && is_earth(b1))
}

fn process_body_pair(
    ctx: &mut CollisionContext,
    config: &SimulationConfig,
    time_warp: &TimeWarp,
    b1: &mut BodySnapshot,
    b2: &BodySnapshot,
) {
    let r_rel = b1.pos - b2.pos;
    let dist = r_rel.length();

    let is_parent_satellite =
        b1.satellite_parent == Some(b2.entity) || b2.satellite_parent == Some(b1.entity);
    let is_earth_name = |n: &str, b_type: BodyType| {
        b_type.is_planet()
            && !n.contains("Moon")
            && (n == "Earth"
                || n == "Proto-Earth"
                || n.starts_with("Earth")
                || n.starts_with("Proto-Earth"))
            && !n.contains("Planet Nine")
            && !n.contains("Planet 9")
            && !n.contains("Super-Earth")
            && !n.contains("Venus")
            && !n.contains("Mercury")
            && !n.contains("Mars")
    };
    let is_earth_moon = (is_earth_name(&b1.name, b1.body_type)
        && b2.name.contains("Moon")
        && (b2.satellite_parent == Some(b1.entity) || b2.name == "The Moon" || b2.name == "Moon"))
        || (is_earth_name(&b2.name, b2.body_type)
            && b1.name.contains("Moon")
            && (b1.satellite_parent == Some(b2.entity)
                || b1.name == "The Moon"
                || b1.name == "Moon"));
    if is_parent_satellite || is_earth_moon {
        return;
    }

    let (r_contact, v_esc, v_rel, effective_collision_radius) =
        compute_effective_collision_radius(config, ctx.star_mass, b1, b2);

    let max_linear_dt = if b1.body_type.is_planet() && b2.body_type.is_planet() {
        let r_min = b1.pos.length().min(b2.pos.length());
        (0.20 * r_min / v_rel.max(1e-2)).min(0.005)
    } else {
        0.05
    };
    let dt = (config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)).min(max_linear_dt);
    let v_rel_vec = b1.vel - b2.vel;
    let (min_dist, r_closest) = compute_closest_approach(r_rel, v_rel_vec, dist, dt);

    let can_candidate_moon = !b1.is_central
        && !b2.is_central
        && !b1.body_type.is_star_or_remnant()
        && !b2.body_type.is_star_or_remnant()
        && !is_parent_satellite
        && {
            let m_min = b1.mass.min(b2.mass);
            let m_max = b1.mass.max(b2.mass);
            let gamma = m_min / m_max.max(1e-30);
            (0.05..=0.45).contains(&gamma) && m_max >= EARTH_MASS_SOLAR * 0.05
        };

    let is_theia_earth = is_theia_earth_snapshot(b1, b2);
    let interpenetrating =
        is_physically_interpenetrating(dist, min_dist, b1.radius, b2.radius, r_contact);

    if !can_candidate_moon && !is_theia_earth && interpenetrating {
        let pair = sort_collision_pair(b1, b2);
        handle_inelastic_merger(ctx, &pair, v_rel, b1);
        return;
    }

    if min_dist <= effective_collision_radius {
        execute_collision_regimes(
            ctx,
            b1,
            b2,
            min_dist,
            r_closest,
            v_rel_vec,
            v_rel,
            v_esc,
            effective_collision_radius,
            r_contact,
        );
    } else {
        handle_aerocapture(ctx, config, b1, b2, min_dist, v_rel);
    }
}

fn execute_collision_regimes(
    ctx: &mut CollisionContext,
    b1: &mut BodySnapshot,
    b2: &BodySnapshot,
    min_dist: f64,
    r_closest: DVec3,
    v_rel_vec: DVec3,
    v_rel: f64,
    v_esc: f64,
    effective_collision_radius: f64,
    r_contact: f64,
) {
    let v_rel_km_s = v_rel * AU_PER_YR_TO_KM_PER_S;

    let angular_momentum_rel = r_closest.cross(v_rel_vec).length();
    let b = (angular_momentum_rel / (v_rel.max(1e-8) * effective_collision_radius.max(1e-8)))
        .clamp(0.0, 1.0);

    let pair = sort_collision_pair(b1, b2);

    let p_density = pair.p_comp.average_density();
    let s_density = pair.s_comp.average_density();
    let p_rad_au = radius_from_mass_density(pair.p_m, p_density).max(EARTH_RADIUS_AU * 0.3);
    let s_rad_au = radius_from_mass_density(pair.s_m, s_density);
    let d_roche = 2.44 * p_rad_au * (p_density / s_density.max(1e-4)).cbrt();

    let params = ImpactParams {
        primary_mass: pair.p_m,
        secondary_mass: pair.s_m,
        primary_radius_au: p_rad_au.max(r_contact * 0.85),
        secondary_radius_au: s_rad_au,
        primary_type: pair.p_type,
        secondary_type: pair.s_type,
        min_dist,
        b,
        v_rel,
        v_esc,
    };

    let is_earth_cand = |n: &str, pos: DVec3, b_type: BodyType| {
        let is_explicit = n == "Earth"
            || n == "Proto-Earth"
            || n.starts_with("Proto-Earth")
            || n.starts_with("Earth (")
            || (n.contains("Earth") && !n.contains("Super-Earth"));
        let is_procedural = pos.length() >= 0.85 && pos.length() <= 1.15 && b_type.is_planet();
        let is_excluded = n.contains("Planet Nine")
            || n.contains("Planet 9")
            || n.contains("Super-Earth")
            || n.contains("Theia")
            || n.contains("Moon")
            || n.contains("Venus")
            || n.contains("Mercury")
            || n.contains("Mars")
            || n.contains("Jupiter")
            || n.contains("Saturn")
            || n.contains("Uranus")
            || n.contains("Neptune")
            || n.contains("Pluto");
        (is_explicit || is_procedural) && !is_excluded
    };
    let is_theia_cand = |n: &str| {
        crate::simulation::accretion::theia::is_explicit_theia(n)
    };
    let is_theia_earth = (is_theia_cand(&pair.p_name)
        && is_earth_cand(&pair.s_name, pair.s_pos, pair.s_type))
        || (is_theia_cand(&pair.s_name) && is_earth_cand(&pair.p_name, pair.p_pos, pair.p_type));

    let regime = if is_theia_earth {
        ImpactRegime::GiantImpactMoon
    } else {
        classify_impact(params, d_roche)
    };

    match regime {
        ImpactRegime::EmbeddedMerge | ImpactRegime::Merger => {
            handle_inelastic_merger(ctx, &pair, v_rel, b1);
        }
        ImpactRegime::Disrupt => {
            handle_roche_disruption(ctx, &pair, min_dist, d_roche, p_rad_au);
        }
        ImpactRegime::GiantImpactMoon => {
            handle_giant_impact_moon(ctx, &pair, b, r_closest, r_contact, b1);
        }
        ImpactRegime::HitAndRun | ImpactRegime::Graze => {
            handle_grazing_bounce(
                ctx,
                pair.primary_entity,
                pair.secondary_entity,
                pair.p_m,
                pair.s_m,
                pair.p_is_central,
                false,
                r_closest,
                v_rel_vec,
                v_rel_km_s,
                b,
                r_contact,
            );
        }
    }
}
