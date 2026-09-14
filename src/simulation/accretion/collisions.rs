//! Collision detection and accretion physics processing.

use bevy::math::DVec3;
use bevy::prelude::*;
use hashbrown::HashSet;
use smallvec::SmallVec;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::basins::*;
use super::events::*;
use super::impact_regimes::{
    classify_impact, radius_from_mass_density, ImpactParams, ImpactRegime,
};

pub type AccretionQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Mass,
        &'static mut SimPosition,
        &'static mut SimVelocity,
        &'static mut SimAcceleration,
        &'static mut Radius,
        &'static mut Temperature,
        &'static mut Composition,
        &'static mut CelestialBody,
        Option<&'static mut InternalDifferentiation>,
        Option<&'static mut SpinState>,
        Option<&'static mut SatelliteOf>,
        Option<&'static CentralStar>,
    ),
>;

#[derive(Clone)]
pub struct BodySnapshot {
    pub entity: Entity,
    pub mass: f64,
    pub pos: DVec3,
    pub vel: DVec3,
    pub radius: f64,
    pub temp: f64,
    pub comp: Composition,
    pub body_type: BodyType,
    pub spin: DVec3,
    pub name: String,
    pub is_central: bool,
    pub satellite_parent: Option<Entity>,
}

#[derive(Clone)]
pub struct SortedPair {
    pub primary_entity: Entity,
    pub p_m: f64,
    pub p_pos: DVec3,
    pub p_vel: DVec3,
    pub p_comp: Composition,
    pub p_type: BodyType,
    pub p_spin: DVec3,
    pub p_name: String,
    pub p_is_central: bool,
    pub secondary_entity: Entity,
    pub s_m: f64,
    pub s_pos: DVec3,
    pub s_vel: DVec3,
    pub s_comp: Composition,
    pub s_type: BodyType,
    pub s_spin: DVec3,
    pub s_name: String,
}

pub struct CollisionContext<'a, 'c1, 'c2, 'q1, 'q2, 'm1, 'm2, 'm3, 'm4> {
    pub commands: &'a mut Commands<'c1, 'c2>,
    pub bodies_query: &'a mut AccretionQuery<'q1, 'q2>,
    pub player_state: &'a mut PlayerInteractionState,
    pub merge_events: &'a mut MessageWriter<'m1, AccretionMergeEvent>,
    pub moon_events: &'a mut MessageWriter<'m2, MoonFormationEvent>,
    pub bounce_events: &'a mut MessageWriter<'m3, CollisionBounceEvent>,
    pub roche_events: &'a mut MessageWriter<'m4, RocheDisruptionEvent>,
    pub merged_away: &'a mut HashSet<Entity>,
    pub newly_formed_moons: &'a mut HashSet<Entity>,
    pub pending_despawns: &'a mut SmallVec<[Entity; 32]>,
    pub star_mass: f64,
    pub sim_time_years: f64,
}

pub fn sort_collision_pair(b1: &BodySnapshot, b2: &BodySnapshot) -> SortedPair {
    let (p, s) = if b1.is_central || (!b2.is_central && b1.mass >= b2.mass) {
        (b1, b2)
    } else {
        (b2, b1)
    };
    SortedPair {
        primary_entity: p.entity,
        p_m: p.mass,
        p_pos: p.pos,
        p_vel: p.vel,
        p_comp: p.comp,
        p_type: p.body_type,
        p_spin: p.spin,
        p_name: p.name.clone(),
        p_is_central: p.is_central,
        secondary_entity: s.entity,
        s_m: s.mass,
        s_pos: s.pos,
        s_vel: s.vel,
        s_comp: s.comp,
        s_type: s.body_type,
        s_spin: s.spin,
        s_name: s.name.clone(),
    }
}

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
        if pair.p_name.contains("Earth") || pair.s_name.contains("Theia") {
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
        if pair.p_name.contains("Earth") || pair.s_name.contains("Theia") {
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
        let is_theia_or_earth = |n: &str| {
            (n == "Earth"
                || n == "Proto-Earth"
                || n.starts_with("Proto-Earth")
                || n.starts_with("Earth (")
                || n.contains("Theia"))
                && !n.contains("Planet Nine")
                && !n.contains("Super-Earth")
        };
        let is_earth_or_moon = |n: &str| {
            (n == "Earth"
                || n == "Proto-Earth"
                || n.starts_with("Proto-Earth")
                || n.starts_with("Earth (")
                || n.contains("Moon"))
                && !n.contains("Planet Nine")
                && !n.contains("Super-Earth")
        };
        if (is_theia_or_earth(&name1) && is_theia_or_earth(&name2))
            || (is_earth_or_moon(&name1) && is_earth_or_moon(&name2))
        {
            return;
        }

        let e_restitution = 0.35;
        let impulse_mag = -(1.0 + e_restitution) * v_rel_normal / (1.0 / m1 + 1.0 / m2);
        let impulse = n_norm * impulse_mag;

        if !is_central1 {
            if let Ok((_, _, _, mut v1, _, _, _, _, _, _, _, _, _)) = ctx.bodies_query.get_mut(e1) {
                v1.0 += impulse / m1;
            }
        }
        if !is_central2 {
            if let Ok((_, _, _, mut v2, _, _, _, _, _, _, _, _, _)) = ctx.bodies_query.get_mut(e2) {
                v2.0 -= impulse / m2;
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

pub fn handle_inelastic_merger(
    ctx: &mut CollisionContext,
    pair: &SortedPair,
    v_rel: f64,
    b1: &mut BodySnapshot,
) {
    let is_earth_name = |n: &str| {
        (n == "Earth"
            || n == "Proto-Earth"
            || n.starts_with("Earth")
            || n.starts_with("Proto-Earth"))
            && !n.contains("Planet Nine")
            && !n.contains("Super-Earth")
    };
    if (is_earth_name(&pair.p_name) && pair.s_name.contains("Moon"))
        || (is_earth_name(&pair.s_name) && pair.p_name.contains("Moon"))
    {
        return;
    }
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

    let kinetic_loss = 0.5 * ((pair.p_m * pair.s_m) / total_mass) * v_rel * v_rel;
    let delta_temp = (kinetic_loss * 5e5).clamp(0.0, 4000.0);
    let new_temp = (b1.temp + delta_temp).min(10000.0);

    let is_star_like = pair.p_type.is_star_or_remnant();
    let updated_type = if is_star_like {
        pair.p_type
    } else {
        classify_body_by_mass_and_comp(total_mass, &merged_comp, false)
    };

    let r_len = merged_pos.length().max(1e-4);
    let new_acc = if pair.p_is_central {
        DVec3::ZERO
    } else {
        -(G_ASTRO * ctx.star_mass / (r_len * r_len * r_len)) * merged_pos
    };

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
        m.0 = total_mass;
        pos.0 = merged_pos;
        vel.0 = merged_vel;
        acc.0 = new_acc;
        if !is_star_like {
            rad.0 = new_radius;
            t.0 = new_temp;
        }
        *comp = merged_comp;
        body.body_type = updated_type;

        update_merged_body_name(&mut body.name, updated_type);

        if let Some(mut diff) = opt_diff {
            diff.recalculate(total_mass, new_radius, &merged_comp);
        }
        if let Some(mut spin) = opt_spin_mut {
            spin.update_from_spin(merged_spin, total_mass, new_radius);
        }
    }

    deliver_volatiles_and_crater(ctx, pair);

    for e in [pair.primary_entity, pair.secondary_entity] {
        if let Ok(mut cmd) = ctx.commands.get_entity(e) {
            cmd.remove::<SatelliteOf>();
        }
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
        merged_mass: total_mass,
        merged_position: merged_pos,
        merged_velocity: merged_vel,
        new_body_type: updated_type,
        energy_released: kinetic_loss,
    });

    b1.mass = total_mass;
    b1.pos = merged_pos;
    b1.vel = merged_vel;
    if !is_star_like {
        b1.radius = new_radius;
        b1.temp = new_temp;
    }
    b1.comp = merged_comp;
    b1.body_type = updated_type;
    b1.spin = merged_spin;
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
    super::basins::deliver_volatiles_and_crater(
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

/// Detects close-contact collisions and processes physical collision regimes.
pub fn process_accretion_and_collisions(
    mut commands: Commands,
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut player_state: ResMut<PlayerInteractionState>,
    disk_params: Res<DiskParameters>,
    mut merge_events: MessageWriter<AccretionMergeEvent>,
    mut moon_events: MessageWriter<MoonFormationEvent>,
    mut bounce_events: MessageWriter<CollisionBounceEvent>,
    mut roche_events: MessageWriter<RocheDisruptionEvent>,
    mut bodies_query: AccretionQuery,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let star_mass = disk_params.central_star_mass;
    let bodies: Vec<BodySnapshot> = bodies_query
        .iter()
        .map(
            |(e, m, pos, vel, _, rad, temp, comp, body, _, opt_spin, opt_sat, opt_central)| {
                let spin_vec = opt_spin.map_or(DVec3::ZERO, |s| s.spin_vector);
                BodySnapshot {
                    entity: e,
                    mass: m.0,
                    pos: pos.0,
                    vel: vel.0,
                    radius: rad.0,
                    temp: temp.0,
                    comp: *comp,
                    body_type: body.body_type,
                    spin: spin_vec,
                    name: body.name.clone(),
                    is_central: opt_central.is_some(),
                    satellite_parent: opt_sat.map(|s| s.parent),
                }
            },
        )
        .collect();

    let n = bodies.len();
    if n < 2 {
        return;
    }

    let mut merged_away: HashSet<Entity> = HashSet::with_capacity(64);
    let mut newly_formed_moons: HashSet<Entity> = HashSet::with_capacity(8);
    let mut pending_despawns: SmallVec<[Entity; 32]> = SmallVec::new();

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

    for (i, b1_init) in bodies.iter().enumerate() {
        let mut b1 = b1_init.clone();
        if ctx.merged_away.contains(&b1.entity) || ctx.newly_formed_moons.contains(&b1.entity) {
            continue;
        }

        for b2 in bodies.iter().skip(i + 1).cloned() {
            if ctx.merged_away.contains(&b1.entity) || ctx.newly_formed_moons.contains(&b1.entity) {
                break;
            }
            if ctx.merged_away.contains(&b2.entity) || ctx.newly_formed_moons.contains(&b2.entity) {
                continue;
            }

            process_body_pair(&mut ctx, &config, &time_warp, &mut b1, &b2);
        }
    }

    for entity in pending_despawns {
        if let Ok(mut e_cmd) = commands.get_entity(entity) {
            e_cmd.despawn();
        }
    }
}

/// Determines whether two bodies have physically interpenetrated to the point where
/// hydrodynamic shear and mutual shock forces unconditionally necessitate coalescence.
pub fn is_physically_interpenetrating(
    dist: f64,
    min_dist: f64,
    r1: f64,
    r2: f64,
    r_contact: f64,
) -> bool {
    let physical_contact = r1 + r2;
    dist < physical_contact * 0.90
        || min_dist < physical_contact * 0.85
        || dist < r1.max(r2) * 1.05
        || dist < r_contact * 0.95
        || min_dist < r_contact * 0.90
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

    let r1_vis = f64::from(config.calc_visual_radius_for_type(b1.radius, b1.body_type));
    let r2_vis = f64::from(config.calc_visual_radius_for_type(b2.radius, b2.body_type));
    let r_contact = (r1_vis + r2_vis).max(b1.radius + b2.radius);

    let is_parent_satellite =
        b1.satellite_parent == Some(b2.entity) || b2.satellite_parent == Some(b1.entity);
    let is_earth_name = |n: &str| {
        (n == "Earth"
            || n == "Proto-Earth"
            || n.starts_with("Earth")
            || n.starts_with("Proto-Earth"))
            && !n.contains("Planet Nine")
            && !n.contains("Super-Earth")
    };
    let is_earth_moon = (is_earth_name(&b1.name) && b2.name.contains("Moon"))
        || (is_earth_name(&b2.name) && b1.name.contains("Moon"));
    if is_parent_satellite || is_earth_moon {
        return;
    }

    let v_esc = (2.0 * G_ASTRO * (b1.mass + b2.mass) / r_contact.max(1e-6)).sqrt();
    let v_rel_vec = b1.vel - b2.vel;
    let v_rel = v_rel_vec.length();

    let safronov_factor = 1.0 + (v_esc * v_esc) / (v_rel * v_rel + 1e-4);
    let is_minor = |t: BodyType| {
        matches!(
            t,
            BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal | BodyType::DustGrain
        )
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
        r_orb * (p_m / (3.0 * ctx.star_mass.max(0.01))).cbrt() * 0.40
    } else {
        0.0
    };

    let effective_collision_radius = (r_contact * safronov_factor.sqrt())
        .max(r_contact)
        .max(capture_r)
        .min(r_contact * 3.0 + capture_r);

    let dt = (config.base_dt_yr * time_warp.multiplier.max(0.01)).min(0.05);
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

    let is_earth = |b: &BodySnapshot| {
        let n = b.name.as_str();
        (n == "Earth"
            || n == "Proto-Earth"
            || n.starts_with("Proto-Earth")
            || n.starts_with("Earth (")
            || (b.pos.length() > 0.65 && b.pos.length() < 1.35 && b.body_type.is_planet()))
            && !n.contains("Planet Nine")
            && !n.contains("Super-Earth")
            && !n.contains("Theia")
            && !n.contains("Moon")
    };
    let is_theia = |b: &BodySnapshot| b.name.contains("Theia");
    let is_theia_earth = (is_theia(b1) && is_earth(b2)) || (is_theia(b2) && is_earth(b1));

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
        (n == "Earth"
            || n == "Proto-Earth"
            || n.starts_with("Proto-Earth")
            || n.starts_with("Earth (")
            || (pos.length() > 0.65 && pos.length() < 1.35 && b_type.is_planet()))
            && !n.contains("Planet Nine")
            && !n.contains("Super-Earth")
            && !n.contains("Theia")
            && !n.contains("Moon")
    };
    let is_theia_earth = (pair.p_name.contains("Theia")
        && is_earth_cand(&pair.s_name, pair.s_pos, pair.s_type))
        || (pair.s_name.contains("Theia") && is_earth_cand(&pair.p_name, pair.p_pos, pair.p_type));

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
        ImpactRegime::HitAndRun | ImpactRegime::Graze => handle_grazing_bounce(
            ctx,
            b1.entity,
            b2.entity,
            b1.mass,
            b2.mass,
            b1.is_central,
            b2.is_central,
            r_closest,
            v_rel_vec,
            v_rel_km_s,
            b,
            r_contact,
        ),
    }
}
