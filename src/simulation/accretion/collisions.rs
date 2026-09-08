//! Collision detection and accretion physics processing.

use bevy::math::DVec3;
use bevy::prelude::*;
use hashbrown::HashSet;
use smallvec::SmallVec;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::events::*;

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
    pub pending_despawns: &'a mut SmallVec<[Entity; 32]>,
    pub star_mass: f64,
    pub sim_time_years: f64,
}

pub fn sort_collision_pair(b1: &BodySnapshot, b2: &BodySnapshot) -> SortedPair {
    if b1.is_central || (!b2.is_central && b1.mass >= b2.mass) {
        SortedPair {
            primary_entity: b1.entity,
            p_m: b1.mass,
            p_pos: b1.pos,
            p_vel: b1.vel,
            p_comp: b1.comp,
            p_type: b1.body_type,
            p_spin: b1.spin,
            p_name: b1.name.clone(),
            p_is_central: b1.is_central,
            secondary_entity: b2.entity,
            s_m: b2.mass,
            s_pos: b2.pos,
            s_vel: b2.vel,
            s_comp: b2.comp,
            s_spin: b2.spin,
            s_name: b2.name.clone(),
        }
    } else {
        SortedPair {
            primary_entity: b2.entity,
            p_m: b2.mass,
            p_pos: b2.pos,
            p_vel: b2.vel,
            p_comp: b2.comp,
            p_type: b2.body_type,
            p_spin: b2.spin,
            p_name: b2.name.clone(),
            p_is_central: b2.is_central,
            secondary_entity: b1.entity,
            s_m: b1.mass,
            s_pos: b1.pos,
            s_vel: b1.vel,
            s_comp: b1.comp,
            s_spin: b1.spin,
            s_name: b1.name.clone(),
        }
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
        primary_pos: Vec3::new(
            pair.p_pos.x as f32,
            pair.p_pos.y as f32,
            pair.p_pos.z as f32,
        ),
        disruption_pos: Vec3::new(
            pair.s_pos.x as f32,
            pair.s_pos.y as f32,
            pair.s_pos.z as f32,
        ),
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
        .max(visual_r * 1.45);
    let p_moon_yr = 2.0
        * std::f64::consts::PI
        * (orbit_dist_au.powi(3) / (G_ASTRO * total_primary_mass.max(1e-8))).sqrt();

    let moon_tangent = r_rel.cross(DVec3::Y).normalize_or_zero();
    let v_moon_orb = (G_ASTRO * total_primary_mass / orbit_dist_au.max(1e-5)).sqrt();
    let moon_pos = primary_pos + r_rel.normalize_or_zero() * orbit_dist_au;
    let moon_vel = primary_vel + moon_tangent * v_moon_orb;

    let p_density = pair.p_comp.average_density();
    let p_new_radius = ((3.0 * total_primary_mass / p_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.3);

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
        m.0 = total_primary_mass;
        pos.0 = primary_pos;
        vel.0 = primary_vel;
        rad.0 = p_new_radius;
        t.0 = (t.0 + 800.0).min(4000.0);
        *comp = pair
            .p_comp
            .mass_weighted_merge(pair.p_m, &pair.s_comp, accreted_mass);
        body.body_type = classify_body_by_mass_and_comp(total_primary_mass, &comp, false);

        let r_len = primary_pos.length().max(1e-4);
        acc.0 = -(G_ASTRO * ctx.star_mass / (r_len * r_len * r_len)) * primary_pos;

        if let Some(mut diff) = opt_diff {
            diff.recalculate(total_primary_mass, p_new_radius, &comp);
        }
        if let Some(mut spin) = opt_spin_mut {
            spin.rotation_period_hours = (spin.rotation_period_hours * 0.75).clamp(4.0, 72.0);
        }
    }

    let moon_comp = pair.s_comp;
    let s_density = moon_comp.average_density();
    let moon_radius = ((3.0 * moon_mass / s_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.05);

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
        _,
    )) = ctx.bodies_query.get_mut(pair.secondary_entity)
    {
        m.0 = moon_mass;
        pos.0 = moon_pos;
        vel.0 = moon_vel;
        rad.0 = moon_radius;
        t.0 = 220.0;
        *comp = moon_comp;
        body.body_type = BodyType::Moon;
        body.name = format!("{} I (Moon)", pair.p_name);

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

    ctx.moon_events.write(MoonFormationEvent {
        parent_entity: pair.primary_entity,
        moon_entity: pair.secondary_entity,
        moon_mass,
        orbital_radius_au: orbit_dist_au,
        orbital_period_years: p_moon_yr,
    });

    if pair.primary_entity == b1.entity {
        b1.mass = total_primary_mass;
        b1.pos = primary_pos;
        b1.vel = primary_vel;
        b1.radius = p_new_radius;
        b1.comp = pair
            .p_comp
            .mass_weighted_merge(pair.p_m, &pair.s_comp, accreted_mass);
        b1.body_type = classify_body_by_mass_and_comp(total_primary_mass, &b1.comp, false);
    } else {
        b1.mass = moon_mass;
        b1.pos = moon_pos;
        b1.vel = moon_vel;
        b1.radius = moon_radius;
        b1.body_type = BodyType::Moon;
    }
}

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
) {
    let n_norm = r_rel.normalize_or_zero();
    let v_rel_normal = v_rel_vec.dot(n_norm);

    if v_rel_normal < 0.0 {
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
    }
}

pub fn handle_inelastic_merger(
    ctx: &mut CollisionContext,
    pair: &SortedPair,
    v_rel: f64,
    b1: &mut BodySnapshot,
) {
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

fn update_merged_body_name(name: &mut String, updated_type: BodyType) {
    if matches!(
        updated_type,
        BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::GasGiant
            | BodyType::IceGiant
            | BodyType::Protoplanet
            | BodyType::Planetesimal
    ) && (name.contains("Comet")
        || name.contains("Asteroid")
        || (!name.starts_with("Planet")
            && matches!(
                updated_type,
                BodyType::TerrestrialPlanet
                    | BodyType::SuperEarth
                    | BodyType::GasGiant
                    | BodyType::IceGiant
            )))
    {
        *name = match updated_type {
            BodyType::TerrestrialPlanet => "Planet (Terrestrial World)".to_string(),
            BodyType::SuperEarth => "Planet (Super-Earth)".to_string(),
            BodyType::GasGiant => "Planet (Gas Giant)".to_string(),
            BodyType::IceGiant => "Planet (Ice Giant)".to_string(),
            BodyType::BrownDwarf => "Sub-Stellar Brown Dwarf".to_string(),
            BodyType::Protoplanet => "Protoplanet (Embryo)".to_string(),
            BodyType::Planetesimal => "Planetesimal".to_string(),
            _ => name.clone(),
        };
    }
}

fn deliver_volatiles_and_crater(ctx: &mut CollisionContext, pair: &SortedPair) {
    let d_water_earth = (pair.s_m * pair.s_comp.ice_frac) / EARTH_MASS_SOLAR;
    let d_gas_earth = (pair.s_m * pair.s_comp.gas_frac) / EARTH_MASS_SOLAR;

    if let Ok(mut p_cmd) = ctx.commands.get_entity(pair.primary_entity) {
        p_cmd
            .entry::<VolatileInventory>()
            .and_modify(move |mut vol| {
                vol.delivered_water_m_earth += d_water_earth;
                vol.cometary_impact_count += 1;
                vol.ocean_coverage_frac =
                    (vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
                vol.atmospheric_pressure_bar =
                    (vol.atmospheric_pressure_bar + (d_gas_earth * 120.0) as f32).clamp(0.01, 90.0);
            })
            .or_insert(VolatileInventory {
                delivered_water_m_earth: d_water_earth,
                ocean_coverage_frac: (d_water_earth / 0.0006).clamp(0.0, 0.85) as f32,
                atmospheric_pressure_bar: (d_gas_earth * 120.0).clamp(0.01, 90.0) as f32,
                cometary_impact_count: 1,
            });

        let norm = (pair.s_pos - pair.p_pos).normalize_or_zero();
        let basin = ImpactBasin {
            surface_normal: Vec3::new(norm.x as f32, norm.y as f32, norm.z as f32),
            angular_radius: ((pair.s_m / pair.p_m).cbrt() as f32).clamp(0.12, 0.55),
            formation_time_yr: ctx.sim_time_years,
            melt_glow_fraction: 1.0,
        };

        p_cmd
            .entry::<PlanetaryBasins>()
            .and_modify(move |mut pb| {
                if pb.basins.len() >= 8 {
                    pb.basins.remove(0);
                }
                pb.basins.push(basin);
            })
            .or_insert(PlanetaryBasins {
                basins: vec![basin],
            });
    }
}

pub fn handle_aerocapture(
    ctx: &mut CollisionContext,
    b1: &BodySnapshot,
    b2: &BodySnapshot,
    min_dist: f64,
    v_rel: f64,
) {
    let (primary_entity, p_m, p_pos, p_type, secondary_entity, s_m, s_name) = if b1.mass >= b2.mass
    {
        (
            b1.entity,
            b1.mass,
            b1.pos,
            b1.body_type,
            b2.entity,
            b2.mass,
            b2.name.clone(),
        )
    } else {
        (
            b2.entity,
            b2.mass,
            b2.pos,
            b2.body_type,
            b1.entity,
            b1.mass,
            b1.name.clone(),
        )
    };

    let is_capturing_body = matches!(
        p_type,
        BodyType::GasGiant
            | BodyType::IceGiant
            | BodyType::Protoplanet
            | BodyType::SuperEarth
            | BodyType::TerrestrialPlanet
    );
    let valid_mass_ratio =
        p_m >= EARTH_MASS_SOLAR * 0.05 && s_m <= p_m * 0.15 && s_m >= EARTH_MASS_SOLAR * 1e-8;

    if is_capturing_body && valid_mass_ratio && !p_type.is_star_or_remnant() {
        let orbit_radius = p_pos.length().max(1e-4);
        let hill_radius = orbit_radius * (p_m / (3.0 * ctx.star_mass)).cbrt();

        if min_dist < hill_radius * 0.65 {
            let v_esc_local = (2.0 * G_ASTRO * p_m / min_dist.max(1e-6)).sqrt();
            if v_rel < v_esc_local * 1.5 && v_rel > v_esc_local * 0.05 {
                let p_phys_r = ((3.0 * p_m / DENSITY_ROCK_ASTRO) / (4.0 * PI))
                    .cbrt()
                    .max(EARTH_RADIUS_AU * 0.3);
                let visual_r = {
                    let ratio = (p_phys_r as f32 / 0.00465).max(1e-6);
                    f64::from(0.025 * ratio.powf(0.45))
                };
                let orbit_dist_au = min_dist.max(p_phys_r * 3.0).max(visual_r * 1.45);
                let p_moon_yr = 2.0 * PI * (orbit_dist_au.powi(3) / (G_ASTRO * p_m)).sqrt();

                if let Ok((_, _, _, _, _, _, _, _, mut body, _, _, opt_sat_mut, _)) =
                    ctx.bodies_query.get_mut(secondary_entity)
                {
                    if !matches!(body.body_type, BodyType::Moon) {
                        body.body_type = BodyType::Moon;
                        body.name = format!("Captured {s_name}");

                        if let Some(mut sat) = opt_sat_mut {
                            sat.parent = primary_entity;
                            sat.semi_major_axis_au = orbit_dist_au;
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
            |(e, m, pos, vel, _, rad, temp, comp, body, _, opt_spin, _, opt_central)| {
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
                }
            },
        )
        .collect();

    let n = bodies.len();
    if n < 2 {
        return;
    }

    let mut merged_away: HashSet<Entity> = HashSet::with_capacity(64);
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
        pending_despawns: &mut pending_despawns,
        star_mass,
        sim_time_years: sim_time.elapsed_years,
    };

    for (i, b1_init) in bodies.iter().enumerate() {
        let mut b1 = b1_init.clone();
        if ctx.merged_away.contains(&b1.entity) {
            continue;
        }

        for b2 in bodies.iter().skip(i + 1).cloned() {
            if ctx.merged_away.contains(&b1.entity) {
                break;
            }
            if ctx.merged_away.contains(&b2.entity) {
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

fn process_body_pair(
    ctx: &mut CollisionContext,
    config: &SimulationConfig,
    time_warp: &TimeWarp,
    b1: &mut BodySnapshot,
    b2: &BodySnapshot,
) {
    let r_rel = b1.pos - b2.pos;
    let dist = r_rel.length();

    let r_vis_1 = f64::from(SimulationConfig::calc_collision_radius(b1.mass, b1.body_type) * 0.08);
    let r_vis_2 = f64::from(SimulationConfig::calc_collision_radius(b2.mass, b2.body_type) * 0.08);
    let r_contact = (r_vis_1 + r_vis_2).max(b1.radius + b2.radius);

    let v_esc = (2.0 * G_ASTRO * (b1.mass + b2.mass) / r_contact.max(1e-6)).sqrt();
    let v_rel_vec = b1.vel - b2.vel;
    let v_rel = v_rel_vec.length();

    let safronov_factor = 1.0 + (v_esc * v_esc) / (v_rel * v_rel + 1e-4);
    let effective_collision_radius = (r_contact * safronov_factor.sqrt())
        .max(r_contact)
        .min(r_contact * 3.0);

    let dt = config.base_dt_yr * time_warp.multiplier.max(0.01);
    let r_rel_old = r_rel - v_rel_vec * dt;
    let v_rel_sq = v_rel_vec.length_squared();
    let mut min_dist = dist;
    let mut r_closest = r_rel;

    if v_rel_sq > 1e-12 {
        let t_min = -r_rel_old.dot(v_rel_vec) / v_rel_sq;
        if t_min > 0.0 && t_min < dt {
            r_closest = r_rel_old + v_rel_vec * t_min;
            min_dist = r_closest.length();
        } else if t_min <= 0.0 {
            r_closest = r_rel_old;
            min_dist = r_closest.length();
        }
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
        handle_aerocapture(ctx, b1, b2, min_dist, v_rel);
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
    let v_esc_km_s = v_esc * AU_PER_YR_TO_KM_PER_S;

    let angular_momentum_rel = r_closest.cross(v_rel_vec).length();
    let b = (angular_momentum_rel / (v_rel.max(1e-8) * effective_collision_radius.max(1e-8)))
        .clamp(0.0, 1.0);

    let pair = sort_collision_pair(b1, b2);

    let p_density = pair.p_comp.average_density();
    let s_density = pair.s_comp.average_density();
    let p_rad_au = ((3.0 * pair.p_m / p_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.3);
    let d_roche = 2.44 * p_rad_au * (p_density / s_density.max(1e-4)).cbrt();

    let is_roche_disruption = min_dist <= d_roche
        && b >= 0.20
        && pair.p_m >= EARTH_MASS_SOLAR * 0.05
        && pair.s_m <= pair.p_m * 0.20
        && !pair.p_type.is_star_or_remnant()
        && !b2.body_type.is_star_or_remnant();

    if is_roche_disruption {
        handle_roche_disruption(ctx, &pair, min_dist, d_roche, p_rad_au);
        return;
    }

    let is_giant_impact_moon = b >= 0.45
        && pair.p_m >= EARTH_MASS_SOLAR * 0.01
        && pair.s_m <= pair.p_m * 0.65
        && pair.s_m >= pair.p_m * 0.02
        && pair.s_m >= EARTH_MASS_SOLAR * 0.0001
        && !pair.p_type.is_star_or_remnant()
        && !b2.body_type.is_star_or_remnant();

    if is_giant_impact_moon {
        handle_giant_impact_moon(ctx, &pair, b, r_closest, r_contact, b1);
    } else if b > 0.85 && v_rel_km_s > v_esc_km_s * 1.5 {
        handle_grazing_bounce(
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
        );
    } else {
        handle_inelastic_merger(ctx, &pair, v_rel, b1);
    }
}
