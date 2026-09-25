//! Handlers for collision and accretion regimes:
//! Roche tidal disruption, giant impact moon formation, grazing bounces,
//! inelastic mergers, and aerocapture.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::super::basins::*;
use super::super::events::*;
use super::types::*;
use crate::simulation::components::classify_body_by_mass_and_comp;

pub fn handle_roche_disruption(
    ctx: &mut CollisionContext,
    pair: &SortedPair,
    min_dist: f64,
    d_roche: f64,
    p_rad_au: f64,
) {
    let ring_mass_earth = pair.s_m / EARTH_MASS_SOLAR;
    let inner_r = (p_rad_au * 1.25) as f32;
    let outer_r = (d_roche.min(p_rad_au * 3.2)).max(f64::from(inner_r) * 1.35) as f32;
    let s_ice_frac = pair.s_comp.ice_frac as f32;
    let s_silicate_frac = (pair.s_comp.silicate_frac + pair.s_comp.metal_frac) as f32;

    if let Ok(mut p_cmd) = ctx.commands.get_entity(pair.primary_entity) {
        p_cmd
            .entry::<PlanetaryRingSystem>()
            .and_modify(move |mut ring| {
                ring.ring_mass_earth += ring_mass_earth;
                ring.outer_radius_au = ring.outer_radius_au.max(outer_r);
                ring.optical_depth = (ring.optical_depth + 0.35).min(1.0);
                ring.ice_fraction = (ring.ice_fraction * 0.5 + s_ice_frac * 0.5).clamp(0.0, 1.0);
                ring.silicate_fraction = (1.0 - ring.ice_fraction).max(0.0);
            })
            .or_insert(PlanetaryRingSystem {
                inner_radius_au: inner_r,
                outer_radius_au: outer_r,
                ring_mass_earth,
                optical_depth: ((ring_mass_earth / 0.0001).clamp(0.40, 0.95)) as f32,
                ice_fraction: s_ice_frac,
                silicate_fraction: s_silicate_frac,
            });
    }

    ctx.roche_events.write(RocheDisruptionEvent {
        disrupted_entity: pair.secondary_entity,
        primary_entity: pair.primary_entity,
        disruption_radius: min_dist,
        ring_mass_earth,
        ice_fraction: pair.s_comp.ice_frac as f32,
        silicate_fraction: (pair.s_comp.silicate_frac + pair.s_comp.metal_frac) as f32,
        primary_pos: pair.p_pos.as_vec3(),
        disruption_pos: pair.s_pos.as_vec3(),
        primary_name: pair.p_name.clone(),
        disrupted_name: pair.s_name.clone(),
    });

    if ctx.player_state.selected_entity == Some(pair.secondary_entity) {
        ctx.player_state.selected_entity = Some(pair.primary_entity);
    }

    ctx.merged_away.insert(pair.secondary_entity);
    if !ctx.pending_despawns.contains(&pair.secondary_entity) {
        ctx.pending_despawns.push(pair.secondary_entity);
    }
}

pub fn handle_giant_impact_moon(
    ctx: &mut CollisionContext,
    pair: &SortedPair,
    b: f64,
    r_rel: DVec3,
    r_contact: f64,
    b1: &mut BodySnapshot,
) {
    let moon_mass_frac = (0.25 + 0.35 * b).clamp(0.20, 0.55);
    let moon_mass = pair.s_m * moon_mass_frac;
    let accreted_mass = pair.s_m - moon_mass;
    let total_primary_mass = pair.p_m + accreted_mass;

    let primary_vel = (pair.p_vel * pair.p_m + pair.s_vel * accreted_mass) / total_primary_mass;
    let primary_pos = (pair.p_pos * pair.p_m + pair.s_pos * accreted_mass) / total_primary_mass;

    let d_impact = r_contact.max(1e-5);
    let p_density_est = pair.p_comp.average_density();
    let p_phys_r = ((3.0 * pair.p_m / p_density_est) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.3);
    let visual_r = {
        let ratio = (p_phys_r as f32 / 0.00465).max(1e-6);
        f64::from(0.025 * ratio.powf(0.45))
    };
    let orbit_dist_au = (d_impact * (1.2 + 0.8 * b))
        .max(EARTH_RADIUS_AU * 1.5)
        .max(p_phys_r * 3.0)
        .max(visual_r * 1.50)
        .max(r_contact * 1.50)
        .max(0.0075);
    let p_moon_yr = 2.0
        * std::f64::consts::PI
        * (orbit_dist_au.powi(3) / (G_ASTRO * total_primary_mass.max(1e-8))).sqrt();

    let tangent = r_rel.cross(DVec3::Y).normalize_or_zero();
    let mut moon_tangent = if tangent.length_squared() > 0.1 {
        tangent
    } else {
        DVec3::new(0.0, 0.0, 1.0)
    };
    if moon_tangent.dot(primary_vel) < 0.0 {
        moon_tangent = -moon_tangent;
    }
    let v_moon_orb = (G_ASTRO * total_primary_mass / orbit_dist_au.max(1e-5)).sqrt();
    let moon_pos = primary_pos + r_rel.normalize_or_zero() * orbit_dist_au;
    let moon_vel = primary_vel + moon_tangent * v_moon_orb;

    let p_density = pair.p_comp.average_density();
    let p_new_radius = ((3.0 * total_primary_mass / p_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.3);
    let merged_primary_comp =
        pair.p_comp
            .mass_weighted_merge(pair.p_m, &pair.s_comp, accreted_mass);

    apply_giant_impact_primary_state(
        ctx,
        pair.primary_entity,
        total_primary_mass,
        primary_pos,
        primary_vel,
        p_new_radius,
        merged_primary_comp,
    );

    let moon_comp = pair.s_comp;
    let s_density = moon_comp.average_density();
    let moon_radius = ((3.0 * moon_mass / s_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.05);

    apply_giant_impact_moon_state(
        ctx,
        pair,
        moon_mass,
        moon_pos,
        moon_vel,
        moon_radius,
        orbit_dist_au,
        p_moon_yr,
    );

    ctx.merge_events.write(AccretionMergeEvent {
        primary_entity: pair.primary_entity,
        secondary_entity: pair.secondary_entity,
        merged_mass: total_primary_mass,
        merged_position: primary_pos,
        merged_velocity: primary_vel,
        new_body_type: classify_body_by_mass_and_comp(
            total_primary_mass,
            &merged_primary_comp,
            false,
        ),
        energy_released: 1.0e32,
    });

    ctx.moon_events.write(MoonFormationEvent {
        parent_entity: pair.primary_entity,
        moon_entity: pair.secondary_entity,
        moon_mass,
        orbital_radius_au: orbit_dist_au,
        orbital_period_years: p_moon_yr,
    });
    ctx.newly_formed_moons.insert(pair.secondary_entity);

    if pair.primary_entity == b1.entity {
        b1.mass = total_primary_mass;
        b1.pos = primary_pos;
        b1.vel = primary_vel;
        b1.radius = p_new_radius;
        b1.comp = merged_primary_comp;
        b1.body_type = classify_body_by_mass_and_comp(total_primary_mass, &b1.comp, false);
        if b1.name.starts_with("Proto-Earth") {
            b1.name = "Earth".to_string();
        }
    } else {
        b1.mass = moon_mass;
        b1.pos = moon_pos;
        b1.vel = moon_vel;
        b1.radius = moon_radius;
        b1.body_type = BodyType::Moon;
        let is_canonical_earth_theia = (pair.p_name.contains("Earth")
            && !pair.p_name.contains("Super-Earth")
            && !pair.p_name.contains("Planet Nine")
            && !pair.p_name.contains("Venus"))
            && (pair.s_name.contains("Theia") || pair.s_name.contains("Moon"));
        if is_canonical_earth_theia {
            b1.name = "The Moon".to_string();
        } else {
            b1.name = format!("{} I (Moon)", pair.p_name);
        }
        b1.satellite_parent = Some(pair.primary_entity);
    }
}

fn apply_giant_impact_primary_state(
    ctx: &mut CollisionContext,
    primary_entity: Entity,
    total_primary_mass: f64,
    primary_pos: DVec3,
    primary_vel: DVec3,
    p_new_radius: f64,
    new_comp: Composition,
) {
    if let Ok((
        _,
        mut m,
        mut pos,
        mut vel,
        mut acc,
        mut rad,
        mut t,
        mut comp,
        mut body,
        opt_diff,
        opt_spin_mut,
        ..,
    )) = ctx.bodies_query.get_mut(primary_entity)
    {
        m.0 = total_primary_mass;
        pos.0 = primary_pos;
        vel.0 = primary_vel;
        rad.0 = p_new_radius;
        t.0 = (t.0 + 800.0).min(4000.0);
        *comp = new_comp;
        body.body_type = classify_body_by_mass_and_comp(total_primary_mass, &comp, false);
        if body.name.starts_with("Proto-Earth") {
            body.name = "Earth".to_string();
        }

        let r_len = primary_pos.length().max(1e-4);
        acc.0 = -(G_ASTRO * ctx.star_mass / (r_len * r_len * r_len)) * primary_pos;

        if let Some(mut diff) = opt_diff {
            diff.recalculate(total_primary_mass, p_new_radius, &comp);
            diff.has_theia_llsvp = true;
            diff.llsvp_density_contrast = 0.028;
        }
        if let Some(mut spin) = opt_spin_mut {
            if body.name.contains("Earth") {
                spin.rotation_period_hours = 6.0;
                spin.axial_tilt_degrees = 23.4;
            } else {
                spin.rotation_period_hours = (spin.rotation_period_hours * 0.75).clamp(4.0, 72.0);
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Collision handlers are inherently complex."
)]
fn apply_giant_impact_moon_state(
    ctx: &mut CollisionContext,
    pair: &SortedPair,
    moon_mass: f64,
    moon_pos: DVec3,
    moon_vel: DVec3,
    moon_radius: f64,
    orbit_dist_au: f64,
    p_moon_yr: f64,
) {
    let moon_comp = pair.s_comp;
    if let Ok((
        _,
        mut m,
        mut pos,
        mut vel,
        mut acc,
        mut rad,
        mut t,
        mut comp,
        mut body,
        opt_diff,
        opt_spin_mut,
        opt_sat_mut,
        ..,
    )) = ctx.bodies_query.get_mut(pair.secondary_entity)
    {
        m.0 = moon_mass;
        pos.0 = moon_pos;
        vel.0 = moon_vel;
        rad.0 = moon_radius;
        t.0 = 220.0;
        *comp = moon_comp;
        body.body_type = BodyType::Moon;
        let is_canonical_earth_theia = (pair.p_name.contains("Earth")
            && !pair.p_name.contains("Super-Earth")
            && !pair.p_name.contains("Planet Nine")
            && !pair.p_name.contains("Venus"))
            && (pair.s_name.contains("Theia") || pair.s_name.contains("Moon"));
        if is_canonical_earth_theia {
            body.name = "The Moon".to_string();
        } else {
            body.name = format!("{} I (Moon)", pair.p_name);
        }

        let r_len = moon_pos.length().max(1e-4);
        acc.0 = -(G_ASTRO * ctx.star_mass / (r_len * r_len * r_len)) * moon_pos;

        if let Some(mut diff) = opt_diff {
            diff.recalculate(moon_mass, moon_radius, &moon_comp);
        }
        if let Some(mut spin) = opt_spin_mut {
            spin.rotation_period_hours = p_moon_yr * YEAR_SECONDS / 3600.0;
        }
        if let Some(mut sat) = opt_sat_mut {
            sat.parent = pair.primary_entity;
            sat.semi_major_axis_au = orbit_dist_au;
            sat.orbital_period_years = p_moon_yr;
            sat.true_anomaly = 0.0;
        } else if let Ok(mut s_cmd) = ctx.commands.get_entity(pair.secondary_entity) {
            s_cmd.try_insert(SatelliteOf {
                parent: pair.primary_entity,
                semi_major_axis_au: orbit_dist_au,
                orbital_period_years: p_moon_yr,
                true_anomaly: 0.0,
            });
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Collision handlers are inherently complex."
)]
pub fn handle_grazing_bounce(
    ctx: &mut CollisionContext,
    e1: Entity,
    e2: Entity,
    m1: f64,
    m2: f64,
    is_central1: bool,
    is_central2: bool,
    r_rel: DVec3,
    v_rel_vec: DVec3,
    v_rel_km_s: f64,
    b: f64,
    r_contact: f64,
) {
    let n_norm = r_rel.normalize_or_zero();
    let v_rel_normal = v_rel_vec.dot(n_norm);

    if v_rel_normal < 0.0 {
        let (name1, name2) = (
            ctx.bodies_query
                .get(e1)
                .map(|q| q.8.name.clone())
                .unwrap_or_default(),
            ctx.bodies_query
                .get(e2)
                .map(|q| q.8.name.clone())
                .unwrap_or_default(),
        );
        let is_earth = |n: &str| {
            (n == "Earth"
                || n == "Proto-Earth"
                || n.starts_with("Proto-Earth")
                || n.starts_with("Earth (")
                || (n.contains("Earth") && !n.contains("Super-Earth")))
                && !n.contains("Moon")
                && !n.contains("Planet Nine")
                && !n.contains("Planet 9")
                && !n.contains("Venus")
                && !n.contains("Mercury")
                && !n.contains("Mars")
        };
        let is_theia = |n: &str| {
            n.contains("Theia")
                && !n.contains("Earth")
                && !n.contains("Moon")
                && !n.contains("Planet Nine")
                && !n.contains("Planet 9")
        };
        if (is_theia(&name1) && is_earth(&name2))
            || (is_theia(&name2) && is_earth(&name1))
            || (is_earth(&name1) && name2.contains("Moon"))
            || (is_earth(&name2) && name1.contains("Moon"))
        {
            return;
        }

        let e_restitution = 0.35;
        let impulse_mag = -(1.0 + e_restitution) * v_rel_normal / (1.0 / m1 + 1.0 / m2);
        let impulse = n_norm * impulse_mag;

        let total_m = m1 + m2;
        let kinetic_dissipation =
            0.5 * ((m1 * m2) / total_m.max(1e-30)) * (v_rel_normal * v_rel_normal);
        let delta_temp = (kinetic_dissipation * 3e5).clamp(150.0, 3000.0);

        if !is_central1 {
            if let Ok((_, _, _, mut v1, _, _, mut t1, ..)) = ctx.bodies_query.get_mut(e1) {
                v1.0 += impulse / m1;
                t1.0 = (t1.0 + delta_temp).min(4000.0);
            }
        }
        if !is_central2 {
            if let Ok((_, _, _, mut v2, _, _, mut t2, ..)) = ctx.bodies_query.get_mut(e2) {
                v2.0 -= impulse / m2;
                t2.0 = (t2.0 + delta_temp).min(4000.0);
            }
        }

        ctx.bounce_events.write(CollisionBounceEvent {
            entity1: e1,
            entity2: e2,
            relative_velocity_km_s: v_rel_km_s,
            impact_parameter: b,
        });

        record_grazing_impact_scar(ctx.commands, e1, r_rel, m1, m2, ctx.sim_time_years, b);
        record_grazing_impact_scar(ctx.commands, e2, -r_rel, m2, m1, ctx.sim_time_years, b);
    }

    // Positional separation: gently push overlapping spheres apart to prevent visual sticking
    let dist = r_rel.length();
    let overlap = (r_contact - dist).max(0.0);
    if overlap > 0.0 && n_norm.length_squared() > 0.5 {
        let push = n_norm * (overlap * 0.51);
        if !is_central1 {
            if let Ok((_, _, mut pos1, ..)) = ctx.bodies_query.get_mut(e1) {
                pos1.0 += push;
            }
        }
        if !is_central2 {
            if let Ok((_, _, mut pos2, ..)) = ctx.bodies_query.get_mut(e2) {
                pos2.0 -= push;
            }
        }
    }
}

fn should_skip_earth_moon_collision(pair: &SortedPair) -> bool {
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
    (is_earth_name(&pair.p_name, pair.p_type) && pair.s_name.contains("Moon"))
        || (is_earth_name(&pair.s_name, pair.s_type) && pair.p_name.contains("Moon"))
}

struct InelasticMergerPhysics {
    total_mass: f64,
    merged_vel: DVec3,
    merged_pos: DVec3,
    merged_comp: Composition,
    merged_spin: DVec3,
    new_radius: f64,
    new_temp: f64,
    updated_type: BodyType,
    new_acc: DVec3,
    kinetic_loss: f64,
}

fn compute_inelastic_merger_physics(
    pair: &SortedPair,
    v_rel: f64,
    b1_temp: f64,
    star_mass: f64,
) -> InelasticMergerPhysics {
    let total_mass = pair.p_m + pair.s_m;
    let merged_vel = if pair.p_is_central {
        DVec3::ZERO
    } else {
        (pair.p_vel * pair.p_m + pair.s_vel * pair.s_m) / total_mass
    };
    let merged_pos = if pair.p_is_central {
        DVec3::ZERO
    } else {
        (pair.p_pos * pair.p_m + pair.s_pos * pair.s_m) / total_mass
    };

    let merged_comp = pair
        .p_comp
        .mass_weighted_merge(pair.p_m, &pair.s_comp, pair.s_m);

    let r_impact = pair.p_pos - pair.s_pos;
    let v_impact = pair.p_vel - pair.s_vel;
    let impact_orbital_spin = (pair.p_m * pair.s_m / total_mass) * r_impact.cross(v_impact);
    let merged_spin = pair.p_spin + pair.s_spin + impact_orbital_spin;

    let density = merged_comp.average_density();
    let volume = total_mass / density;
    let new_radius = ((3.0 * volume) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.3);

    let v_esc_sq = (2.0 * G_ASTRO * total_mass) / new_radius.max(1e-6);
    let kinetic_loss =
        0.5 * ((pair.p_m * pair.s_m) / total_mass.max(1e-30)) * (v_rel * v_rel + v_esc_sq * 0.25);
    let gamma = (pair.s_m / pair.p_m.max(1e-30)).min(1.0);
    let delta_temp = if gamma >= 0.05 {
        (kinetic_loss * 5e5).clamp(250.0, 4000.0)
    } else {
        (gamma * 1500.0).clamp(0.5, 50.0)
    };
    let new_temp = (b1_temp + delta_temp).min(10000.0);

    let is_star_like = pair.p_type.is_star_or_remnant();
    let updated_type = if is_star_like {
        pair.p_type
    } else if pair.p_type == BodyType::Moon || pair.s_type == BodyType::Moon {
        if pair.p_type.is_planet() {
            classify_body_by_mass_and_comp(total_mass, &merged_comp, false)
        } else {
            BodyType::Moon
        }
    } else {
        classify_body_by_mass_and_comp(total_mass, &merged_comp, false)
    };

    let r_len = merged_pos.length().max(1e-4);
    let new_acc = if pair.p_is_central {
        DVec3::ZERO
    } else {
        -(G_ASTRO * star_mass / (r_len * r_len * r_len)) * merged_pos
    };

    InelasticMergerPhysics {
        total_mass,
        merged_vel,
        merged_pos,
        merged_comp,
        merged_spin,
        new_radius,
        new_temp,
        updated_type,
        new_acc,
        kinetic_loss,
    }
}

pub fn handle_inelastic_merger(
    ctx: &mut CollisionContext,
    pair: &SortedPair,
    v_rel: f64,
    b1: &mut BodySnapshot,
) {
    if should_skip_earth_moon_collision(pair) {
        return;
    }

    let physics = compute_inelastic_merger_physics(pair, v_rel, b1.temp, ctx.star_mass);
    let is_star_like = pair.p_type.is_star_or_remnant();

    if let Ok((
        _,
        mut m,
        mut pos,
        mut vel,
        mut acc,
        mut rad,
        mut t,
        mut comp,
        mut body,
        opt_diff,
        opt_spin_mut,
        _,
        _,
    )) = ctx.bodies_query.get_mut(pair.primary_entity)
    {
        m.0 = physics.total_mass;
        pos.0 = physics.merged_pos;
        vel.0 = physics.merged_vel;
        acc.0 = physics.new_acc;
        if !is_star_like {
            rad.0 = physics.new_radius;
            t.0 = physics.new_temp;
        }
        *comp = physics.merged_comp;
        body.body_type = physics.updated_type;

        update_merged_body_name(&mut body.name, physics.updated_type);

        if let Some(mut diff) = opt_diff {
            diff.recalculate(physics.total_mass, physics.new_radius, &physics.merged_comp);
        }
        if let Some(mut spin) = opt_spin_mut {
            spin.update_from_spin(physics.merged_spin, physics.total_mass, physics.new_radius);
        }
    }

    deliver_volatiles_and_crater(ctx, pair);

    // Only remove SatelliteOf from secondary (despawned) entity, preserving orbit for primary if it was a satellite!
    if let Ok(mut cmd) = ctx.commands.get_entity(pair.secondary_entity) {
        cmd.remove::<SatelliteOf>();
    }

    if ctx.player_state.selected_entity == Some(pair.secondary_entity) {
        ctx.player_state.selected_entity = Some(pair.primary_entity);
    }

    ctx.merged_away.insert(pair.secondary_entity);
    if !ctx.pending_despawns.contains(&pair.secondary_entity) {
        ctx.pending_despawns.push(pair.secondary_entity);
    }

    ctx.merge_events.write(AccretionMergeEvent {
        primary_entity: pair.primary_entity,
        secondary_entity: pair.secondary_entity,
        merged_mass: physics.total_mass,
        merged_position: physics.merged_pos,
        merged_velocity: physics.merged_vel,
        new_body_type: physics.updated_type,
        energy_released: physics.kinetic_loss,
    });

    b1.mass = physics.total_mass;
    b1.pos = physics.merged_pos;
    b1.vel = physics.merged_vel;
    if !is_star_like {
        b1.radius = physics.new_radius;
        b1.temp = physics.new_temp;
    }
    b1.comp = physics.merged_comp;
    b1.body_type = physics.updated_type;
    b1.spin = physics.merged_spin;
}

fn is_canonical_solar_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("earth")
        || lower.contains("theia")
        || lower.contains("moon")
        || lower.contains("luna")
        || lower.contains("mercury")
        || lower.contains("venus")
        || lower.contains("mars")
        || lower.contains("jupiter")
        || lower.contains("saturn")
        || lower.contains("uranus")
        || lower.contains("neptune")
        || lower.contains("pluto")
        || lower.contains("sun")
        || lower.contains("sol")
}

fn update_merged_body_name(name: &mut String, updated_type: BodyType) {
    if is_canonical_solar_name(name) {
        return;
    }
    let is_named_minor = name.contains("Comet") || name.contains("Asteroid");
    if updated_type.is_planet() && (is_named_minor || !name.starts_with("Planet")) {
        *name = match updated_type {
            BodyType::TerrestrialPlanet => "Planet (Terrestrial World)".to_string(),
            BodyType::SuperEarth => "Planet (Super-Earth)".to_string(),
            BodyType::GasGiant => "Planet (Gas Giant)".to_string(),
            BodyType::IceGiant => "Planet (Ice Giant)".to_string(),
            BodyType::Protoplanet => "Protoplanet (Embryo)".to_string(),
            _ => name.clone(),
        };
    }
}

fn deliver_volatiles_and_crater(ctx: &mut CollisionContext, pair: &SortedPair) {
    super::super::basins::deliver_volatiles_and_crater(
        ctx.commands,
        pair.primary_entity,
        pair.p_pos,
        pair.s_pos,
        pair.p_m,
        pair.s_m,
        &pair.s_comp,
        ctx.sim_time_years,
    );
}

pub fn handle_aerocapture(
    ctx: &mut CollisionContext,
    config: &SimulationConfig,
    b1: &mut BodySnapshot,
    b2: &BodySnapshot,
    min_dist: f64,
    v_rel: f64,
) {
    let (p, s): (&BodySnapshot, &BodySnapshot) = if b1.mass >= b2.mass {
        (&*b1, b2)
    } else {
        (b2, &*b1)
    };
    let (primary_entity, p_m, p_pos, p_type) = (p.entity, p.mass, p.pos, p.body_type);
    let (secondary_entity, s_m, s_name) = (s.entity, s.mass, s.name.clone());

    let is_capturing_body = p_type.is_planet();
    let valid_mass_ratio =
        p_m >= EARTH_MASS_SOLAR * 0.05 && s_m <= p_m * 0.15 && s_m >= EARTH_MASS_SOLAR * 1e-8;

    if is_capturing_body && valid_mass_ratio && !p_type.is_star_or_remnant() {
        let orbit_radius = p_pos.length().max(1e-4);
        let hill_radius = orbit_radius * (p_m / (3.0 * ctx.star_mass)).cbrt();

        if min_dist < hill_radius * 0.65 {
            let v_esc_local = (2.0 * G_ASTRO * p_m / min_dist.max(1e-6)).sqrt();
            if (v_esc_local * 0.05..v_esc_local * 1.5).contains(&v_rel) {
                let p_vis = f64::from(config.calc_visual_radius_for_type(p.radius, p.body_type));
                let s_vis = f64::from(config.calc_visual_radius_for_type(s.radius, s.body_type));
                let contact_r = p_vis + s_vis;

                // Close enough that it sits in the crust/mesh/atmosphere → pull in and merge.
                if min_dist <= contact_r * 1.25 {
                    let pair = sort_collision_pair(b1, b2);
                    handle_inelastic_merger(ctx, &pair, v_rel, b1);
                    return;
                }

                // Safe exterior capture → bound moon outside the mesh and atmosphere.
                let orbit_dist_au = min_dist.max(contact_r * 1.60);
                let p_moon_yr = 2.0 * PI * (orbit_dist_au.powi(3) / (G_ASTRO * p_m)).sqrt();

                if let Ok((_, _, _, _, _, _, _, _, mut body, _, _, opt_sat_mut, _)) =
                    ctx.bodies_query.get_mut(secondary_entity)
                {
                    body.body_type = BodyType::Moon;
                    if !body.name.contains("(Moon)") && !body.name.starts_with("Captured") {
                        body.name = format!("Captured {s_name}");
                    }

                    if let Some(mut sat) = opt_sat_mut {
                        sat.parent = primary_entity;
                        sat.semi_major_axis_au = orbit_dist_au.max(sat.semi_major_axis_au);
                        sat.orbital_period_years = p_moon_yr;
                        sat.true_anomaly = 0.0;
                    } else if let Ok(mut s_cmd) = ctx.commands.get_entity(secondary_entity) {
                        s_cmd.try_insert(SatelliteOf {
                            parent: primary_entity,
                            semi_major_axis_au: orbit_dist_au,
                            orbital_period_years: p_moon_yr,
                            true_anomaly: 0.0,
                        });
                    }
                }
            }
        }
    }
}
