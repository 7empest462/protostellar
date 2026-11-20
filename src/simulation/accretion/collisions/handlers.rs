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
use super::super::impact_regimes::radius_from_mass_density;
use super::naming::*;
use super::types::*;
use crate::simulation::components::classify_body_by_mass_and_comp;

use super::giant_impact::{count_parent_satellites, strip_tenuous_moon_gas};

fn detach_orphaned_satellites(ctx: &mut CollisionContext, lost_parent: Entity) {
    let mut orphaned: smallvec::SmallVec<[Entity; 4]> = smallvec::SmallVec::new();
    for (ent, .., opt_sat, _) in ctx.bodies_query.iter() {
        if ent != lost_parent
            && opt_sat
                .as_ref()
                .is_some_and(|sat| sat.parent == lost_parent)
        {
            orphaned.push(ent);
        }
    }
    for ent in orphaned {
        if let Ok(mut cmd) = ctx.commands.get_entity(ent) {
            cmd.remove::<SatelliteOf>();
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
    detach_orphaned_satellites(ctx, pair.secondary_entity);

    let ring_mass_earth = pair.s_m / EARTH_MASS_SOLAR;
    let inner_r = (p_rad_au * 1.25) as f32;
    let outer_r = (d_roche.min(p_rad_au * 3.2)).max(f64::from(inner_r) * 1.35) as f32;
    let s_ice_frac = pair.s_comp.ice_frac as f32;
    let s_silicate_frac = (pair.s_comp.silicate_frac + pair.s_comp.metal_frac) as f32;

    if let Ok(mut p_cmd) = ctx.commands.get_entity(pair.primary_entity) {
        p_cmd
            .entry::<PlanetaryRingSystem>()
            .and_modify(move |mut ring| {
                let prev_mass = ring.ring_mass_earth.max(0.0);
                let new_mass = prev_mass + ring_mass_earth;
                if new_mass > 0.0 {
                    ring.ice_fraction = ((f64::from(ring.ice_fraction) * prev_mass
                        + f64::from(s_ice_frac) * ring_mass_earth)
                        / new_mass)
                        .clamp(0.0, 1.0) as f32;
                }
                ring.ring_mass_earth = new_mass;
                ring.outer_radius_au = ring.outer_radius_au.max(outer_r);
                ring.optical_depth = (ring.optical_depth + 0.35).min(1.0);
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
                .map_or_default(|q| q.8.name.clone()),
            ctx.bodies_query
                .get(e2)
                .map_or_default(|q| q.8.name.clone()),
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
        let base_delta_temp = (kinetic_dissipation * 3e5).clamp(0.0, 600.0);
        let delta_temp1 = (base_delta_temp * (m2 / total_m.max(1e-30))).clamp(0.0, 120.0);
        let delta_temp2 = (base_delta_temp * (m1 / total_m.max(1e-30))).clamp(0.0, 120.0);

        if !is_central1
            && let Ok((_, _, _, mut v1, _, _, mut t1, ..)) = ctx.bodies_query.get_mut(e1)
        {
            v1.0 += impulse / m1;
            t1.0 = (t1.0 + delta_temp1).min(2500.0);
        }
        if !is_central2
            && let Ok((_, _, _, mut v2, _, _, mut t2, ..)) = ctx.bodies_query.get_mut(e2)
        {
            v2.0 -= impulse / m2;
            t2.0 = (t2.0 + delta_temp2).min(2500.0);
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

    // Positional separation: mass-weighted CoM-preserving push to prevent visual sticking
    let dist = r_rel.length();
    let overlap = (r_contact - dist).max(0.0);
    if overlap > 0.0 && n_norm.length_squared() > 0.5 {
        let correction = n_norm * (overlap * 1.02);
        let total_m = (m1 + m2).max(1e-30);
        let (frac1, frac2) = match (is_central1, is_central2) {
            (true, true) => (0.0, 0.0),
            (true, false) => (0.0, 1.0),
            (false, true) => (1.0, 0.0),
            (false, false) => (m2 / total_m, m1 / total_m),
        };
        if frac1 > 0.0
            && let Ok((_, _, mut pos1, ..)) = ctx.bodies_query.get_mut(e1)
        {
            pos1.0 += correction * frac1;
        }
        if frac2 > 0.0
            && let Ok((_, _, mut pos2, ..)) = ctx.bodies_query.get_mut(e2)
        {
            pos2.0 -= correction * frac2;
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

fn classify_merged_body_type(
    pair: &SortedPair,
    total_mass: f64,
    merged_comp: &Composition,
) -> BodyType {
    if pair.p_type == BodyType::BlackHole || pair.s_type == BodyType::BlackHole {
        return BodyType::BlackHole;
    }
    if (matches!(pair.p_type, BodyType::RedGiant | BodyType::RedSupergiant)
        && matches!(
            pair.s_type,
            BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
        ))
        || (matches!(pair.s_type, BodyType::RedGiant | BodyType::RedSupergiant)
            && matches!(
                pair.p_type,
                BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
            ))
    {
        // Giant convective envelope swallows neutron star -> Thorne-Żytkow Object
        return BodyType::ThorneZytkow;
    }
    if matches!(
        pair.p_type,
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
    ) && matches!(
        pair.s_type,
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
    ) {
        return if total_mass > TOV_LIMIT_SOLAR {
            BodyType::BlackHole
        } else {
            // Sub-critical double neutron star merger -> Strange Quark Matter Star
            BodyType::QuarkStar
        };
    }
    if pair.p_type.is_star_or_remnant() && pair.s_type.is_star_or_remnant() {
        return if !pair.p_type.is_remnant() && !pair.s_type.is_remnant() {
            if pair.p_type == BodyType::Protostar && pair.s_type == BodyType::Protostar {
                BodyType::Protostar
            } else if total_mass <= 8.0 {
                // Collision between two low/intermediate-mass stars produces a rejuvenated Blue Straggler
                BodyType::BlueStraggler
            } else {
                classify_body_by_mass_and_comp(total_mass, merged_comp, true)
            }
        } else {
            let remnant = if pair.p_type.is_remnant() {
                pair.p_type
            } else {
                pair.s_type
            };
            collapse_remnant_if_over_limit(remnant, total_mass)
        };
    }
    if pair.p_type.is_star_or_remnant() || pair.s_type.is_star_or_remnant() {
        if pair.p_type.is_remnant() || pair.s_type.is_remnant() {
            let remnant = if pair.p_type.is_remnant() {
                pair.p_type
            } else {
                pair.s_type
            };
            return collapse_remnant_if_over_limit(remnant, total_mass);
        }
        let star_type = if pair.p_type.is_star_or_remnant() {
            pair.p_type
        } else {
            pair.s_type
        };
        if matches!(
            star_type,
            BodyType::DarkStar
                | BodyType::PopulationIIIStar
                | BodyType::WolfRayet
                | BodyType::LuminousBlueVariable
                | BodyType::BlueStraggler
                | BodyType::ThorneZytkow
                | BodyType::QuasiStar
        ) && total_mass < 500.0
        {
            return star_type;
        }
        if star_type == BodyType::Protostar && total_mass < 500.0 {
            return BodyType::Protostar;
        }
        return classify_body_by_mass_and_comp(total_mass, merged_comp, true);
    }
    if pair.p_type == BodyType::Moon || pair.s_type == BodyType::Moon {
        return if pair.p_type.is_planet() {
            classify_body_by_mass_and_comp(total_mass, merged_comp, false)
        } else {
            BodyType::Moon
        };
    }
    classify_body_by_mass_and_comp(total_mass, merged_comp, false)
}

fn collapse_remnant_if_over_limit(remnant: BodyType, total_mass: f64) -> BodyType {
    if total_mass >= 25.0 {
        return BodyType::BlackHole;
    }
    match remnant {
        BodyType::WhiteDwarf => {
            if total_mass > TOV_LIMIT_SOLAR {
                BodyType::BlackHole
            } else if total_mass > CHANDRASEKHAR_LIMIT_SOLAR {
                BodyType::Pulsar
            } else {
                BodyType::WhiteDwarf
            }
        }
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar | BodyType::QuarkStar => {
            if total_mass > TOV_LIMIT_SOLAR {
                BodyType::BlackHole
            } else {
                remnant
            }
        }
        _ => remnant,
    }
}

fn apply_post_merger_thermo_state(
    commands: &mut Commands,
    primary_entity: Entity,
    physics: &InelasticMergerPhysics,
    rad: &mut Radius,
    t: &mut Temperature,
) {
    if physics.updated_type == BodyType::BlackHole {
        rad.0 = (1.974e-8 * physics.total_mass).max(1e-7);
        t.0 = 10.0;
        crate::simulation::thermodynamics::stellar_lifecycle::attach_black_hole_remnant_components(
            commands,
            primary_entity,
            physics.total_mass,
        );
    } else if physics.updated_type == BodyType::QuarkStar {
        rad.0 = 0.00008;
        t.0 = 10_000_000.0;
    } else if physics.updated_type == BodyType::WhiteDwarf {
        rad.0 = (0.008 * SOLAR_RADIUS_AU * (0.6 / physics.total_mass.max(0.05)).cbrt())
            .clamp(1.0e-6, 1.0e-3);
        t.0 = t.0.max(25_000.0);
    } else if matches!(
        physics.updated_type,
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
    ) {
        rad.0 = rad.0.clamp(5.0e-8, 2.0e-4);
        t.0 = t.0.max(500_000.0);
    } else if physics.updated_type == BodyType::ThorneZytkow {
        rad.0 = (SOLAR_RADIUS_AU * 180.0).min(0.85);
        t.0 = 3200.0;
    } else if physics.updated_type.is_star_or_remnant() && !physics.updated_type.is_remnant() {
        if physics.updated_type == BodyType::BrownDwarf {
            rad.0 =
                (0.000_477 * (physics.total_mass / 0.000_954).powf(0.08)).clamp(0.00035, 0.00085);
            t.0 =
                t.0.max(800.0 + ((physics.total_mass - 0.0125) / 0.0675).clamp(0.0, 1.0) * 1600.0);
        } else if physics.updated_type != BodyType::Protostar {
            rad.0 = (SOLAR_RADIUS_AU * physics.total_mass.powf(0.8)).clamp(0.002, 0.12);
            t.0 = (5778.0 * physics.total_mass.powf(0.505)).clamp(3000.0, 48_000.0);
            if let Ok(mut cmd) = commands.get_entity(primary_entity) {
                cmd.try_insert((
                    IgnitionState {
                        core_temperature: 1.2e7,
                        fusion_fraction: 1.0,
                        is_ignited: true,
                        shockwave_radius: 0.25,
                    },
                    StellarEvolutionState {
                        phase: StellarEvolutionPhase::MainSequence,
                        hydrogen_core_fraction: 1.0,
                        ..Default::default()
                    },
                    Luminosity(physics.total_mass.powf(3.5).max(0.01)),
                ));
            }
        }
    } else if !physics.updated_type.is_star_or_remnant() {
        rad.0 = physics.new_radius;
        t.0 = physics.new_temp;
    }
}

fn compute_inelastic_merger_physics(
    pair: &SortedPair,
    v_rel: f64,
    b1_temp: f64,
    star_mass: f64,
) -> InelasticMergerPhysics {
    let total_mass = pair.p_m + pair.s_m;
    let is_pinned_origin_star = pair.p_is_central
        && pair.p_pos.length_squared() < 1e-6
        && pair.p_vel.length_squared() < 1e-6;
    let merged_vel = if is_pinned_origin_star {
        DVec3::ZERO
    } else {
        (pair.p_vel * pair.p_m + pair.s_vel * pair.s_m) / total_mass
    };
    let merged_pos = if is_pinned_origin_star {
        DVec3::ZERO
    } else {
        (pair.p_pos * pair.p_m + pair.s_pos * pair.s_m) / total_mass
    };

    let merged_comp = pair
        .p_comp
        .mass_weighted_merge(pair.p_m, &pair.s_comp, pair.s_m);

    let density = merged_comp.average_density();
    let volume = total_mass / density;
    let new_radius = ((3.0 * volume) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.3);

    let r_impact = (pair.p_pos - pair.s_pos).normalize_or_zero() * new_radius;
    let v_impact = pair.p_vel - pair.s_vel;
    let impact_orbital_spin = (pair.p_m * pair.s_m / total_mass) * r_impact.cross(v_impact);
    let merged_spin = pair.p_spin + pair.s_spin + impact_orbital_spin;

    let v_esc_sq = (2.0 * G_ASTRO * total_mass) / new_radius.max(1e-6);
    let kinetic_loss =
        0.5 * ((pair.p_m * pair.s_m) / total_mass.max(1e-30)) * (v_rel * v_rel + v_esc_sq * 0.25);
    let gamma = (pair.s_m / pair.p_m.max(1e-30)).min(1.0);
    let delta_temp = if gamma >= 0.05 {
        (gamma * 600.0).clamp(20.0, 320.0)
    } else {
        (gamma * 350.0).clamp(0.0, 18.0)
    };
    let new_temp = (b1_temp + delta_temp).min(2500.0);

    let updated_type = classify_merged_body_type(pair, total_mass, &merged_comp);

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
    b2: &mut BodySnapshot,
) {
    if should_skip_earth_moon_collision(pair) {
        return;
    }

    let initial_primary_temp = if pair.primary_entity == b1.entity {
        b1.temp
    } else {
        b2.temp
    };

    let physics =
        compute_inelastic_merger_physics(pair, v_rel, initial_primary_temp, ctx.star_mass);

    detach_orphaned_satellites(ctx, pair.secondary_entity);

    let mut final_radius = physics.new_radius;
    let mut final_temp = physics.new_temp;
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
        apply_post_merger_thermo_state(
            ctx.commands,
            pair.primary_entity,
            &physics,
            &mut rad,
            &mut t,
        );
        final_radius = rad.0;
        final_temp = t.0;
        *comp = physics.merged_comp;
        body.body_type = physics.updated_type;

        update_merged_body_name(
            &mut body.name,
            &pair.s_name,
            physics.updated_type,
            physics.total_mass,
        );

        if let Some(mut diff) = opt_diff {
            diff.recalculate(physics.total_mass, final_radius, &physics.merged_comp);
        }
        if let Some(mut spin) = opt_spin_mut {
            spin.update_from_spin(physics.merged_spin, physics.total_mass, final_radius);
        }
    }

    if !pair.p_type.is_star_or_remnant() && !physics.updated_type.is_star_or_remnant() {
        deliver_volatiles_and_crater(ctx, pair);
    }

    // Only remove SatelliteOf from secondary (despawned) entity, preserving orbit for primary if it was a satellite!
    if let Ok(mut cmd) = ctx.commands.get_entity(pair.secondary_entity) {
        cmd.remove::<SatelliteOf>();
    }
    if (pair.s_is_central
        || (physics.updated_type == BodyType::BlackHole && physics.total_mass >= 1_000_000.0))
        && let Ok(mut cmd) = ctx.commands.get_entity(pair.primary_entity)
    {
        cmd.insert(CentralStar);
    }

    if ctx.player_state.selected_entity == Some(pair.secondary_entity)
        || ctx.player_state.selected_entity.is_none()
    {
        ctx.player_state.selected_entity = Some(pair.primary_entity);
    }

    ctx.merged_away.insert(pair.secondary_entity);
    if !ctx.pending_despawns.contains(&pair.secondary_entity) {
        ctx.pending_despawns.push(pair.secondary_entity);
    }

    let impact_dir = (pair.s_pos - pair.p_pos).normalize_or_zero();
    ctx.merge_events.write(AccretionMergeEvent {
        primary_entity: pair.primary_entity,
        secondary_entity: pair.secondary_entity,
        merged_mass: physics.total_mass,
        merged_position: physics.merged_pos + impact_dir * final_radius.max(1e-5),
        merged_velocity: physics.merged_vel,
        new_body_type: physics.updated_type,
        energy_released: physics.kinetic_loss * ASTRO_ENERGY_TO_JOULES,
    });

    let primary_snap = if pair.primary_entity == b1.entity {
        b1
    } else {
        b2
    };
    primary_snap.mass = physics.total_mass;
    primary_snap.pos = physics.merged_pos;
    primary_snap.vel = physics.merged_vel;
    primary_snap.comp = physics.merged_comp;
    primary_snap.body_type = physics.updated_type;
    primary_snap.spin = physics.merged_spin;
    primary_snap.radius = final_radius;
    primary_snap.temp = final_temp;
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
    b2: &mut BodySnapshot,
    min_dist: f64,
    v_rel: f64,
) {
    let (p, s): (&BodySnapshot, &BodySnapshot) = if b1.mass >= b2.mass {
        (&*b1, &*b2)
    } else {
        (&*b2, &*b1)
    };
    let (primary_entity, p_m, p_pos, p_type, p_name) =
        (p.entity, p.mass, p.pos, p.body_type, p.name.clone());
    let (secondary_entity, s_m, s_name) = (s.entity, s.mass, s.name.clone());

    let is_capturing_body = p_type.is_planet();
    let valid_mass_ratio =
        p_m >= EARTH_MASS_SOLAR * 0.05 && s_m <= p_m * 0.15 && s_m >= EARTH_MASS_SOLAR * 1e-8;

    if is_capturing_body && valid_mass_ratio && !p_type.is_star_or_remnant() {
        let orbit_radius = p_pos.length().max(1e-4);
        let hill_radius = orbit_radius * (p_m / (3.0 * ctx.star_mass.max(0.01))).cbrt();

        if min_dist < hill_radius * 0.50 {
            let v_esc_local = (2.0 * G_ASTRO * p_m / min_dist.max(1e-6)).sqrt();
            if (v_esc_local * 0.05..v_esc_local).contains(&v_rel) {
                let p_vis = f64::from(config.calc_visual_radius_for_type(p.radius, p.body_type));
                let s_vis = f64::from(config.calc_visual_radius_for_type(s.radius, s.body_type));
                let contact_r = p_vis + s_vis;

                // Close enough that it sits in the crust/mesh/atmosphere → pull in and merge.
                if min_dist <= contact_r * 1.25 {
                    let pair = sort_collision_pair(b1, b2);
                    handle_inelastic_merger(ctx, &pair, v_rel, b1, b2);
                    return;
                }

                // Safe exterior capture → bound moon outside the mesh and atmosphere.
                let stagger = 1.0 + 0.08 * f64::from((secondary_entity.to_bits() % 5) as u32);
                let orbit_dist_au = min_dist.max(contact_r * 1.60 * stagger);
                let p_moon_yr = 2.0 * PI * (orbit_dist_au.powi(3) / (G_ASTRO * p_m)).sqrt();
                let init_anomaly = (s.pos.z - p_pos.z)
                    .atan2(s.pos.x - p_pos.x)
                    .rem_euclid(2.0 * PI);
                let existing_sats =
                    count_parent_satellites(ctx, primary_entity, secondary_entity, &p_name);

                detach_orphaned_satellites(ctx, secondary_entity);

                let mut captured_rad = s.radius;
                let mut captured_comp = s.comp;
                let mut captured_type = s.body_type;
                let mut captured_name = s_name.clone();

                if let Ok((
                    _,
                    _,
                    _,
                    _,
                    _,
                    mut rad,
                    _,
                    mut comp,
                    mut body,
                    opt_diff,
                    _,
                    opt_sat_mut,
                    _,
                )) = ctx.bodies_query.get_mut(secondary_entity)
                {
                    if !matches!(body.body_type, BodyType::GasGiant | BodyType::IceGiant)
                        && s.comp.gas_frac <= 0.20
                    {
                        body.body_type = BodyType::Moon;
                        let old_gas = comp.gas_frac;
                        *comp = strip_tenuous_moon_gas(&comp, s_m);
                        if (comp.gas_frac - old_gas).abs() > 1e-6 {
                            let compacted_r = radius_from_mass_density(s_m, comp.average_density());
                            rad.0 = compacted_r;
                            if let Some(mut diff) = opt_diff {
                                diff.recalculate(s_m, compacted_r, &comp);
                            }
                        }
                        if !is_canonical_named_body(&body.name) {
                            body.name =
                                resolve_satellite_moon_name(&p_name, &s_name, existing_sats);
                        }
                    } else if !body.name.contains("(Moon)") && !body.name.starts_with("Captured") {
                        body.name = format!("Captured {s_name}");
                    }

                    captured_rad = rad.0;
                    captured_comp = *comp;
                    captured_type = body.body_type;
                    captured_name.clone_from(&body.name);

                    if let Some(mut sat) = opt_sat_mut {
                        sat.semi_major_axis_au = if sat.parent == primary_entity {
                            orbit_dist_au.max(sat.semi_major_axis_au)
                        } else {
                            orbit_dist_au
                        };
                        sat.parent = primary_entity;
                        sat.orbital_period_years = p_moon_yr;
                        sat.true_anomaly = init_anomaly;
                    } else if let Ok(mut s_cmd) = ctx.commands.get_entity(secondary_entity) {
                        s_cmd.try_insert(SatelliteOf {
                            parent: primary_entity,
                            semi_major_axis_au: orbit_dist_au,
                            orbital_period_years: p_moon_yr,
                            true_anomaly: init_anomaly,
                        });
                    }
                    ctx.newly_formed_moons.insert(secondary_entity);
                }

                let sec_snap = if secondary_entity == b1.entity {
                    b1
                } else {
                    b2
                };
                sec_snap.radius = captured_rad;
                sec_snap.comp = captured_comp;
                sec_snap.body_type = captured_type;
                sec_snap.name = captured_name;
                sec_snap.satellite_parent = Some(primary_entity);
            }
        }
    }
}

/// Handles relativistic tidal disruption of stars, planets, and minor bodies around a Black Hole.
pub fn handle_tidal_disruption_event(
    ctx: &mut CollisionContext,
    pair: &SortedPair,
    _min_dist: f64,
    b1: &mut BodySnapshot,
    b2: &mut BodySnapshot,
) {
    detach_orphaned_satellites(ctx, pair.secondary_entity);

    let primary_snap = if pair.primary_entity == b1.entity {
        b1
    } else {
        b2
    };

    let s_density = pair.s_comp.average_density();
    let s_rad_au = radius_from_mass_density(pair.s_m, s_density);
    let r_tidal = s_rad_au * (pair.p_m / pair.s_m.max(1e-12)).cbrt();
    let r_isco = (1.974e-8 * pair.p_m * 3.0).max(1e-6);

    let total_mass = pair.p_m + pair.s_m;
    let is_pinned_origin_star = pair.p_is_central
        && pair.p_pos.length_squared() < 1e-6
        && pair.p_vel.length_squared() < 1e-6;
    let merged_vel = if is_pinned_origin_star {
        DVec3::ZERO
    } else {
        (pair.p_vel * pair.p_m + pair.s_vel * pair.s_m) / total_mass
    };
    let merged_pos = if is_pinned_origin_star {
        DVec3::ZERO
    } else {
        (pair.p_pos * pair.p_m + pair.s_pos * pair.s_m) / total_mass
    };

    if let Ok((_, mut m, mut pos, mut vel, _, mut rad, mut temp, _, mut body, ..)) =
        ctx.bodies_query.get_mut(pair.primary_entity)
    {
        m.0 = total_mass;
        pos.0 = merged_pos;
        vel.0 = merged_vel;
        rad.0 = (1.974e-8 * total_mass).max(1e-7);
        temp.0 = 10.0;
        body.body_type = BodyType::BlackHole;
        update_merged_body_name(
            &mut body.name,
            &pair.s_name,
            BodyType::BlackHole,
            total_mass,
        );
        primary_snap.name.clone_from(&body.name);
    }

    if let Ok(mut cmd) = ctx.commands.get_entity(pair.primary_entity) {
        cmd.remove::<PlanetaryRingSystem>();
        cmd.remove::<PlanetaryClimate>();
        cmd.remove::<BiosphereState>();
        cmd.insert(crate::simulation::relativity::kerr::KerrBlackHole::new(
            total_mass,
            0.70,
            bevy::math::DVec3::Y,
        ));
    }

    if let Ok(mut cmd) = ctx.commands.get_entity(pair.secondary_entity) {
        cmd.remove::<SatelliteOf>();
    }
    if ctx.player_state.selected_entity == Some(pair.secondary_entity)
        || ctx.player_state.selected_entity.is_none()
    {
        ctx.player_state.selected_entity = Some(pair.primary_entity);
    }
    ctx.merged_away.insert(pair.secondary_entity);
    if !ctx.pending_despawns.contains(&pair.secondary_entity) {
        ctx.pending_despawns.push(pair.secondary_entity);
    }

    if let Some(ref mut tde_writer) = ctx.tde_events {
        tde_writer.write(TidalDisruptionEvent {
            bh_entity: pair.primary_entity,
            disrupted_entity: pair.secondary_entity,
            bh_mass_solar: pair.p_m,
            body_mass_solar: pair.s_m,
            tidal_radius_au: r_tidal,
            isco_radius_au: r_isco,
            bh_pos: pair.p_pos.as_vec3(),
            disruption_pos: pair.s_pos.as_vec3(),
            initial_velocity: (pair.s_vel - pair.p_vel).as_vec3(),
            bh_name: pair.p_name.clone(),
            disrupted_name: pair.s_name.clone(),
        });
    }

    ctx.merge_events.write(AccretionMergeEvent {
        primary_entity: pair.primary_entity,
        secondary_entity: pair.secondary_entity,
        merged_mass: total_mass,
        merged_position: merged_pos,
        merged_velocity: merged_vel,
        new_body_type: BodyType::BlackHole,
        energy_released: 0.1
            * pair.s_m
            * SPEED_OF_LIGHT_AU_YR
            * SPEED_OF_LIGHT_AU_YR
            * 0.01
            * ASTRO_ENERGY_TO_JOULES,
    });

    primary_snap.mass = total_mass;
    primary_snap.pos = merged_pos;
    primary_snap.vel = merged_vel;
    primary_snap.radius = (1.974e-8 * total_mass).max(1e-7);
}
