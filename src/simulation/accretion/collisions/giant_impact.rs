//! Giant impact Moon formation kinematics and ECS state application.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::{
    classify_body_by_mass_and_comp, BodyType, Composition, SatelliteOf,
};
use crate::utils::constants::*;

use super::super::events::*;
use super::naming::*;
use super::types::*;

struct GiantImpactKinematics {
    moon_mass: f64,
    total_primary_mass: f64,
    primary_pos: DVec3,
    primary_vel: DVec3,
    moon_pos: DVec3,
    moon_vel: DVec3,
    orbit_dist_au: f64,
    p_moon_yr: f64,
    init_anomaly: f64,
    p_new_radius: f64,
    merged_primary_comp: Composition,
}

fn compute_giant_impact_kinematics(
    pair: &SortedPair,
    b: f64,
    r_rel: DVec3,
    r_contact: f64,
) -> GiantImpactKinematics {
    let moon_mass_frac = (0.25 + 0.35 * b).clamp(0.20, 0.55);
    let moon_mass = pair.s_m * moon_mass_frac;
    let accreted_mass = pair.s_m - moon_mass;
    let total_primary_mass = pair.p_m + accreted_mass;
    let total_system_mass = (total_primary_mass + moon_mass).max(1e-30);

    // Total system center-of-mass position and velocity are invariant across the collision.
    let system_vel = (pair.p_vel * pair.p_m + pair.s_vel * pair.s_m) / total_system_mass;
    let system_pos = (pair.p_pos * pair.p_m + pair.s_pos * pair.s_m) / total_system_mass;

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
        * (orbit_dist_au.powi(3) / (G_ASTRO * total_system_mass.max(1e-8))).sqrt();

    let sep_dir = {
        let n = r_rel.normalize_or_zero();
        if n.length_squared() > 0.5 {
            n
        } else {
            DVec3::X
        }
    };
    let tangent = sep_dir.cross(DVec3::Y).normalize_or_zero();
    let mut moon_tangent = if tangent.length_squared() > 0.1 {
        tangent
    } else {
        DVec3::new(0.0, 0.0, 1.0)
    };
    if moon_tangent.dot(system_vel) < 0.0 {
        moon_tangent = -moon_tangent;
    }
    // Distribute separation and relative orbital velocity by inverse mass ratio so
    // `total_primary_mass * primary_vel + moon_mass * moon_vel == P_initial` and CoM stays exact.
    let v_moon_rel = (G_ASTRO * total_system_mass / orbit_dist_au.max(1e-5)).sqrt();
    let primary_mass_frac = total_primary_mass / total_system_mass;
    let moon_mass_frac_sys = moon_mass / total_system_mass;
    let primary_pos = system_pos - sep_dir * (orbit_dist_au * moon_mass_frac_sys);
    let moon_pos = system_pos + sep_dir * (orbit_dist_au * primary_mass_frac);
    let orb_vel = moon_tangent * v_moon_rel;
    let primary_vel = system_vel - orb_vel * moon_mass_frac_sys;
    let moon_vel = system_vel + orb_vel * primary_mass_frac;
    let init_anomaly = (moon_pos.z - primary_pos.z)
        .atan2(moon_pos.x - primary_pos.x)
        .rem_euclid(2.0 * PI);

    let p_density = pair.p_comp.average_density();
    let p_new_radius = ((3.0 * total_primary_mass / p_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.3);
    let merged_primary_comp =
        pair.p_comp
            .mass_weighted_merge(pair.p_m, &pair.s_comp, accreted_mass);

    GiantImpactKinematics {
        moon_mass,
        total_primary_mass,
        primary_pos,
        primary_vel,
        moon_pos,
        moon_vel,
        orbit_dist_au,
        p_moon_yr,
        init_anomaly,
        p_new_radius,
        merged_primary_comp,
    }
}

pub fn handle_giant_impact_moon(
    ctx: &mut CollisionContext,
    pair: &SortedPair,
    b: f64,
    r_rel: DVec3,
    r_contact: f64,
    b1: &mut BodySnapshot,
    b2: &mut BodySnapshot,
) {
    let k = compute_giant_impact_kinematics(pair, b, r_rel, r_contact);
    let total_primary_mass = k.total_primary_mass;
    let primary_pos = k.primary_pos;
    let primary_vel = k.primary_vel;
    let moon_mass = k.moon_mass;
    let moon_pos = k.moon_pos;
    let moon_vel = k.moon_vel;
    let orbit_dist_au = k.orbit_dist_au;
    let p_moon_yr = k.p_moon_yr;
    let init_anomaly = k.init_anomaly;
    let p_new_radius = k.p_new_radius;
    let merged_primary_comp = k.merged_primary_comp;

    apply_giant_impact_primary_state(
        ctx,
        pair.primary_entity,
        total_primary_mass,
        primary_pos,
        primary_vel,
        p_new_radius,
        merged_primary_comp,
    );

    let moon_comp = strip_tenuous_moon_gas(&pair.s_comp, moon_mass);
    let s_density = moon_comp.average_density();
    let moon_radius = ((3.0 * moon_mass / s_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.05);
    let existing_sats = count_parent_satellites(
        ctx,
        pair.primary_entity,
        pair.secondary_entity,
        &pair.p_name,
    );
    let moon_name = resolve_satellite_moon_name(&pair.p_name, &pair.s_name, existing_sats);

    apply_giant_impact_moon_state(
        ctx,
        pair,
        moon_mass,
        moon_pos,
        moon_vel,
        moon_radius,
        &moon_comp,
        &moon_name,
        orbit_dist_au,
        p_moon_yr,
        init_anomaly,
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

    let (primary_snap, secondary_snap) = if pair.primary_entity == b1.entity {
        (b1, b2)
    } else {
        (b2, b1)
    };

    primary_snap.mass = total_primary_mass;
    primary_snap.pos = primary_pos;
    primary_snap.vel = primary_vel;
    primary_snap.radius = p_new_radius;
    primary_snap.comp = merged_primary_comp;
    primary_snap.body_type =
        classify_body_by_mass_and_comp(total_primary_mass, &primary_snap.comp, false);
    if primary_snap.name.starts_with("Proto-Earth") {
        primary_snap.name = "Earth".to_string();
    }

    secondary_snap.mass = moon_mass;
    secondary_snap.pos = moon_pos;
    secondary_snap.vel = moon_vel;
    secondary_snap.radius = moon_radius;
    secondary_snap.comp = moon_comp;
    secondary_snap.body_type = BodyType::Moon;
    secondary_snap.name = moon_name;
    secondary_snap.satellite_parent = Some(pair.primary_entity);
}

pub(super) fn strip_tenuous_moon_gas(comp: &Composition, mass_solar: f64) -> Composition {
    if mass_solar < EARTH_MASS_SOLAR * 0.02 && comp.gas_frac > 0.0 {
        let mut stripped = *comp;
        stripped.gas_frac = 0.0;
        stripped.normalized()
    } else {
        *comp
    }
}

pub(super) fn count_parent_satellites(
    ctx: &CollisionContext,
    primary_entity: Entity,
    secondary_entity: Entity,
    parent_name: &str,
) -> usize {
    let clean_prefix = format!("{} ", clean_moon_parent_name(parent_name));
    ctx.bodies_query
        .iter()
        .filter(|(e, _, _, _, _, _, _, _, body, _, _, opt_sat, _)| {
            *e != secondary_entity
                && (opt_sat
                    .as_ref()
                    .is_some_and(|sat| sat.parent == primary_entity)
                    || (opt_sat.is_none()
                        && body.body_type == BodyType::Moon
                        && body.name.starts_with(&clean_prefix)))
        })
        .count()
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
            let (new_period, new_tilt) = if body.name.contains("Earth") {
                (6.0, 23.4)
            } else {
                (
                    (spin.rotation_period_hours * 0.75).clamp(4.0, 72.0),
                    spin.axial_tilt_degrees,
                )
            };
            spin.set_period_and_tilt(new_period, new_tilt, total_primary_mass, p_new_radius);
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
    moon_comp: &Composition,
    moon_name: &str,
    orbit_dist_au: f64,
    p_moon_yr: f64,
    init_anomaly: f64,
) {
    let moon_comp = *moon_comp;
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
        body.name = moon_name.to_string();

        let r_len = moon_pos.length().max(1e-4);
        acc.0 = -(G_ASTRO * ctx.star_mass / (r_len * r_len * r_len)) * moon_pos;

        if let Some(mut diff) = opt_diff {
            diff.recalculate(moon_mass, moon_radius, &moon_comp);
        }
        if let Some(mut spin) = opt_spin_mut {
            spin.set_period_and_tilt(
                p_moon_yr * YEAR_SECONDS / 3600.0,
                1.54,
                moon_mass,
                moon_radius,
            );
        }
        if let Some(mut sat) = opt_sat_mut {
            sat.parent = pair.primary_entity;
            sat.semi_major_axis_au = orbit_dist_au;
            sat.orbital_period_years = p_moon_yr;
            sat.true_anomaly = init_anomaly;
        } else if let Ok(mut s_cmd) = ctx.commands.get_entity(pair.secondary_entity) {
            s_cmd.try_insert(SatelliteOf {
                parent: pair.primary_entity,
                semi_major_axis_au: orbit_dist_au,
                orbital_period_years: p_moon_yr,
                true_anomaly: init_anomaly,
            });
        }
    }
}
