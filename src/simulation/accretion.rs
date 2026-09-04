//! Accretion, Collision Mechanics, Tidal Roche Disruption, and Spin Angular Momentum Blending.

use bevy::math::DVec3;
use bevy::prelude::*;
use hashbrown::HashSet;
use smallvec::SmallVec;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Event fired when a grazing giant impact shears an impactor into an orbiting natural moon.
#[derive(Message, Debug, Clone)]
pub struct MoonFormationEvent {
    pub parent_entity: Entity,
    pub moon_entity: Entity,
    pub moon_mass: f64,
    pub orbital_radius_au: f64,
    pub orbital_period_years: f64,
}

/// Event fired when two celestial bodies collide and merge into a single entity.
#[derive(Message, Debug, Clone)]
pub struct AccretionMergeEvent {
    pub primary_entity: Entity,
    pub secondary_entity: Entity,
    pub merged_mass: f64,
    pub merged_position: DVec3,
    pub merged_velocity: DVec3,
    pub new_body_type: BodyType,
    pub energy_released: f64,
}

/// Event fired when two bodies undergo a grazing bounce or partial collision.
#[derive(Message, Debug, Clone)]
pub struct CollisionBounceEvent {
    pub entity1: Entity,
    pub entity2: Entity,
    pub relative_velocity_km_s: f64,
    pub impact_parameter: f64,
}

/// Event fired when a body crosses the tidal Roche limit and is disrupted into a debris ring.
#[derive(Message, Debug, Clone)]
pub struct RocheDisruptionEvent {
    pub disrupted_entity: Entity,
    pub primary_entity: Entity,
    pub disruption_radius: f64,
}

/// Detects close-contact collisions and processes physical collision regimes:
/// 1. Direct Inelastic Mergers (planets absorb planetesimals into growing worlds)
/// 2. Grazing / Side-Swipe Giant Impacts (generates natural orbiting moons outside Roche limit)
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
    mut bodies_query: Query<(
        Entity,
        &mut Mass,
        &mut SimPosition,
        &mut SimVelocity,
        &mut SimAcceleration,
        &mut Radius,
        &mut Temperature,
        &mut Composition,
        &mut CelestialBody,
        Option<&mut InternalDifferentiation>,
        Option<&mut SpinState>,
        Option<&mut SatelliteOf>,
        Option<&CentralStar>,
    )>,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let star_mass = disk_params.central_star_mass;

    // Collect snapshots of bodies to evaluate spatial proximity
    let bodies: Vec<(
        Entity,
        f64,
        DVec3,
        DVec3,
        f64,
        f64,
        Composition,
        BodyType,
        DVec3,
        String,
        bool, // is_central_star
    )> = bodies_query
        .iter()
        .map(
            |(e, m, pos, vel, _, rad, temp, comp, body, _, opt_spin, _, opt_central)| {
                let spin_vec = opt_spin.map(|s| s.spin_vector).unwrap_or(DVec3::ZERO);
                (
                    e,
                    m.0,
                    pos.0,
                    vel.0,
                    rad.0,
                    temp.0,
                    *comp,
                    body.body_type,
                    spin_vec,
                    body.name.clone(),
                    opt_central.is_some(),
                )
            },
        )
        .collect();

    let n = bodies.len();
    if n < 2 {
        return;
    }

    // High-performance SwissTable hash set for merged/consumed entities
    let mut merged_away: HashSet<Entity> = HashSet::with_capacity(64);
    let mut pending_despawns: SmallVec<[Entity; 32]> = SmallVec::new();

    for i in 0..n {
        let (e1, m1, pos1, vel1, rad1, temp1, comp1, type1, spin1, name1, is_central1) =
            bodies[i].clone();
        if merged_away.contains(&e1) {
            continue;
        }

        for (e2, m2, pos2, vel2, rad2, temp2, comp2, type2, spin2, name2, is_central2) in
            bodies.iter().skip(i + 1).cloned()
        {
            if merged_away.contains(&e2) {
                continue;
            }

            let r_rel = pos1 - pos2;
            let dist = r_rel.length();

            // Visual and Gravitational Hill Sphere Collision Cross-Section
            let r_vis_1 = (SimulationConfig::calc_collision_radius(m1, type1) * 0.08) as f64;
            let r_vis_2 = (SimulationConfig::calc_collision_radius(m2, type2) * 0.08) as f64;

            // Base contact radius relies strictly on visual rendering scales so they merge exactly when they touch on-screen
            let r_contact = (r_vis_1 + r_vis_2).max(rad1 + rad2);

            // Combined mutual escape velocity
            let v_esc = (2.0 * G_ASTRO * (m1 + m2) / r_contact.max(1e-6)).sqrt();
            let v_rel_vec = vel1 - vel2;
            let v_rel = v_rel_vec.length();

            // Safronov Gravitational Focusing cross-section
            let safronov_factor = 1.0 + (v_esc * v_esc) / (v_rel * v_rel + 1e-4);
            let effective_collision_radius = (r_contact * safronov_factor.sqrt())
                .max(r_contact)
                .min(r_contact * 3.0);

            // --- Continuous Collision Detection (CCD) ---
            // Because small planets can jump vast distances relative to their size in a single timestep dt,
            // we calculate the closest approach distance *during* the time step.
            let dt = config.base_dt_yr * time_warp.multiplier.max(0.01);
            let r_rel_old = r_rel - v_rel_vec * dt;

            let v_rel_sq = v_rel_vec.length_squared();
            let mut min_dist = dist; // default to end-of-frame distance
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
                let v_rel_km_s = v_rel * AU_PER_YR_TO_KM_PER_S;
                let v_esc_km_s = v_esc * AU_PER_YR_TO_KM_PER_S;

                // Normalized impact parameter b = |r x v| / (v * effective_collision_radius) in [0, 1]
                let angular_momentum_rel = r_closest.cross(v_rel_vec).length();
                let b = (angular_momentum_rel
                    / (v_rel.max(1e-8) * effective_collision_radius.max(1e-8)))
                .clamp(0.0, 1.0);

                // Sort into primary (larger or central) and secondary (smaller impactor)
                let (
                    primary_entity,
                    p_m,
                    p_pos,
                    p_vel,
                    p_comp,
                    p_type,
                    p_spin,
                    p_name,
                    p_is_central,
                    secondary_entity,
                    s_m,
                    s_pos,
                    s_vel,
                    s_comp,
                    _s_type,
                    s_spin,
                    _s_name,
                    _s_is_central,
                ) = if is_central1 || (!is_central2 && m1 >= m2) {
                    (
                        e1,
                        m1,
                        pos1,
                        vel1,
                        comp1,
                        type1,
                        spin1,
                        name1.clone(),
                        is_central1,
                        e2,
                        m2,
                        pos2,
                        vel2,
                        comp2,
                        type2,
                        spin2,
                        name2.clone(),
                        is_central2,
                    )
                } else {
                    (
                        e2,
                        m2,
                        pos2,
                        vel2,
                        comp2,
                        type2,
                        spin2,
                        name2.clone(),
                        is_central2,
                        e1,
                        m1,
                        pos1,
                        vel1,
                        comp1,
                        type1,
                        spin1,
                        name1.clone(),
                        is_central1,
                    )
                };

                // Calculate fluid Roche limit
                let p_density = p_comp.average_density();
                let s_density = s_comp.average_density();
                let p_rad_au = ((3.0 * p_m / p_density) / (4.0 * PI))
                    .cbrt()
                    .max(EARTH_RADIUS_AU * 0.3);
                let d_roche = 2.44 * p_rad_au * (p_density / s_density.max(1e-4)).cbrt();

                // Check for TIDAL ROCHE DISRUPTION (Planetary Ring Formation):
                // 1. Primary is a massive planet / gas giant (>= 0.05 Earth mass)
                // 2. Secondary is a small impactor / moon (<= 0.20 Primary mass)
                // 3. Encounter distance is within Roche limit (min_dist <= d_roche)
                // 4. Periapsis / grazing encounter (b >= 0.20)
                // 5. Neither is a central star
                let is_roche_disruption = min_dist <= d_roche
                    && b >= 0.20
                    && p_m >= EARTH_MASS_SOLAR * 0.05
                    && s_m <= p_m * 0.20
                    && !p_type.is_star_or_remnant()
                    && !type2.is_star_or_remnant();

                if is_roche_disruption {
                    // ==========================================
                    // REGIME 0: TIDAL ROCHE DISRUPTION & RING FORMATION
                    // ==========================================
                    let ring_mass_earth = s_m / EARTH_MASS_SOLAR;
                    let inner_r = (p_rad_au * 1.25) as f32;
                    let outer_r = (d_roche.min(p_rad_au * 3.2)) as f32;

                    if let Ok(mut p_cmd) = commands.get_entity(primary_entity) {
                        p_cmd
                            .entry::<PlanetaryRingSystem>()
                            .and_modify(move |mut ring| {
                                ring.ring_mass_earth += ring_mass_earth;
                                ring.outer_radius_au = ring.outer_radius_au.max(outer_r);
                                ring.optical_depth = (ring.optical_depth + 0.35).min(1.0);
                                ring.ice_fraction = (ring.ice_fraction * 0.5
                                    + s_comp.ice_frac as f32 * 0.5)
                                    .clamp(0.0, 1.0);
                                ring.silicate_fraction = (1.0 - ring.ice_fraction).max(0.0);
                            })
                            .or_insert(PlanetaryRingSystem {
                                inner_radius_au: inner_r,
                                outer_radius_au: outer_r,
                                ring_mass_earth,
                                optical_depth: ((ring_mass_earth / 0.0001).clamp(0.40, 0.95))
                                    as f32,
                                ice_fraction: s_comp.ice_frac as f32,
                                silicate_fraction: (s_comp.silicate_frac + s_comp.metal_frac)
                                    as f32,
                            });
                    }

                    roche_events.write(RocheDisruptionEvent {
                        disrupted_entity: secondary_entity,
                        primary_entity,
                        disruption_radius: min_dist,
                    });

                    // Seamlessly transfer player selection if secondary entity was merged
                    if player_state.selected_entity == Some(secondary_entity) {
                        player_state.selected_entity = Some(primary_entity);
                    }

                    merged_away.insert(secondary_entity);
                    if !pending_despawns.contains(&secondary_entity) {
                        pending_despawns.push(secondary_entity);
                    }
                    continue;
                }

                // Check for GIANT IMPACT MOON FORMATION:
                // Conditions:
                // 1. Oblique / side-swipe impact parameter b >= 0.45
                // 2. Primary body is sufficiently massive (>= 0.01 Earth Mass)
                // 3. Secondary body is an impactor (<= 0.65 Primary Mass, >= 0.0001 Earth Mass)
                // 4. Neither body is a star
                let is_giant_impact_moon = b >= 0.45
                    && p_m >= EARTH_MASS_SOLAR * 0.01
                    && s_m <= p_m * 0.65
                    && s_m >= EARTH_MASS_SOLAR * 0.0001
                    && !p_type.is_star_or_remnant()
                    && !type2.is_star_or_remnant();

                if is_giant_impact_moon {
                    // ==========================================
                    // REGIME A: GIANT IMPACT MOON / SATELLITE FORMATION
                    // ==========================================
                    let moon_mass_frac = (0.25 + 0.35 * b).clamp(0.20, 0.55);
                    let moon_mass = s_m * moon_mass_frac;
                    let accreted_mass = s_m - moon_mass;
                    let total_primary_mass = p_m + accreted_mass;

                    // Conservation of linear momentum for merged primary core
                    let primary_vel = (p_vel * p_m + s_vel * accreted_mass) / total_primary_mass;
                    let primary_pos = (p_pos * p_m + s_pos * accreted_mass) / total_primary_mass;

                    // Compute stable moon orbit around primary
                    let d_impact = r_contact.max(1e-5);
                    let orbit_dist_au = (d_impact * (1.2 + 0.8 * b)).max(EARTH_RADIUS_AU * 1.5);
                    let p_moon_yr = 2.0
                        * std::f64::consts::PI
                        * (orbit_dist_au.powi(3) / (G_ASTRO * total_primary_mass.max(1e-8))).sqrt();

                    let moon_tangent = r_rel.cross(DVec3::Y).normalize_or_zero();
                    let v_moon_orb =
                        (G_ASTRO * total_primary_mass / orbit_dist_au.max(1e-5)).sqrt();
                    let moon_pos = primary_pos + r_rel.normalize_or_zero() * orbit_dist_au;
                    let moon_vel = primary_vel + moon_tangent * v_moon_orb;

                    // Update primary core entity
                    let p_density = p_comp.average_density();
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
                    )) = bodies_query.get_mut(primary_entity)
                    {
                        m.0 = total_primary_mass;
                        pos.0 = primary_pos;
                        vel.0 = primary_vel;
                        rad.0 = p_new_radius;
                        t.0 = (t.0 + 800.0).min(4000.0); // Heating from collision energy
                        *comp = p_comp.mass_weighted_merge(p_m, &s_comp, accreted_mass);
                        body.body_type =
                            classify_body_by_mass_and_comp(total_primary_mass, &comp, false);

                        let r_len = primary_pos.length().max(1e-4);
                        acc.0 = -(G_ASTRO * star_mass / (r_len * r_len * r_len)) * primary_pos;

                        if let Some(mut diff) = opt_diff {
                            diff.recalculate(total_primary_mass, p_new_radius, &comp);
                        }
                        if let Some(mut spin) = opt_spin_mut {
                            spin.rotation_period_hours =
                                (spin.rotation_period_hours * 0.75).clamp(4.0, 72.0);
                        }
                    }

                    // Convert secondary entity into natural satellite / moon
                    let moon_comp = s_comp;
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
                    )) = bodies_query.get_mut(secondary_entity)
                    {
                        m.0 = moon_mass;
                        pos.0 = moon_pos;
                        vel.0 = moon_vel;
                        rad.0 = moon_radius;
                        t.0 = 220.0;
                        *comp = moon_comp;
                        body.body_type = BodyType::Moon;
                        body.name = format!("{} I (Moon)", p_name);

                        let r_len = moon_pos.length().max(1e-4);
                        acc.0 = -(G_ASTRO * star_mass / (r_len * r_len * r_len)) * moon_pos;

                        if let Some(mut diff) = opt_diff {
                            diff.recalculate(moon_mass, moon_radius, &moon_comp);
                        }
                        if let Some(mut spin) = opt_spin_mut {
                            spin.rotation_period_hours = p_moon_yr * YEAR_SECONDS / 3600.0;
                        }
                        if let Some(mut sat) = opt_sat_mut {
                            sat.parent = primary_entity;
                            sat.semi_major_axis_au = orbit_dist_au;
                            sat.orbital_period_years = p_moon_yr;
                            sat.true_anomaly = 0.0;
                        } else if let Ok(mut s_cmd) = commands.get_entity(secondary_entity) {
                            s_cmd.try_insert(SatelliteOf {
                                parent: primary_entity,
                                semi_major_axis_au: orbit_dist_au,
                                orbital_period_years: p_moon_yr,
                                true_anomaly: 0.0,
                            });
                        }
                    }

                    moon_events.write(MoonFormationEvent {
                        parent_entity: primary_entity,
                        moon_entity: secondary_entity,
                        moon_mass,
                        orbital_radius_au: orbit_dist_au,
                        orbital_period_years: p_moon_yr,
                    });
                } else if b > 0.85 && v_rel_km_s > v_esc_km_s * 1.5 {
                    // ==========================================
                    // REGIME B: High-Speed Grazing Hit-and-Run Bounce
                    // ==========================================
                    let n_norm = r_rel.normalize_or_zero();
                    let v_rel_normal = v_rel_vec.dot(n_norm);

                    if v_rel_normal < 0.0 {
                        let e_restitution = 0.35;
                        let impulse_mag =
                            -(1.0 + e_restitution) * v_rel_normal / (1.0 / m1 + 1.0 / m2);
                        let impulse = n_norm * impulse_mag;

                        if !is_central1 {
                            if let Ok((_, _, _, mut v1, _, _, _, _, _, _, _, _, _)) =
                                bodies_query.get_mut(e1)
                            {
                                v1.0 += impulse / m1;
                            }
                        }
                        if !is_central2 {
                            if let Ok((_, _, _, mut v2, _, _, _, _, _, _, _, _, _)) =
                                bodies_query.get_mut(e2)
                            {
                                v2.0 -= impulse / m2;
                            }
                        }

                        bounce_events.write(CollisionBounceEvent {
                            entity1: e1,
                            entity2: e2,
                            relative_velocity_km_s: v_rel_km_s,
                            impact_parameter: b,
                        });
                    }
                } else {
                    // ==========================================
                    // REGIME C: COMPLETE INELASTIC GRAVITATIONAL MERGER
                    // ==========================================
                    let total_mass = p_m + s_m;

                    // Exact Conservation of Linear Momentum (Central Star remains stationary at origin)
                    let merged_vel = if p_is_central {
                        DVec3::ZERO
                    } else {
                        (p_vel * p_m + s_vel * s_m) / total_mass
                    };
                    let merged_pos = if p_is_central {
                        DVec3::ZERO
                    } else {
                        (p_pos * p_m + s_pos * s_m) / total_mass
                    };

                    // Deterministic Mass-Weighted Composition Merging
                    let merged_comp = p_comp.mass_weighted_merge(p_m, &s_comp, s_m);

                    // Exact Conservation of Spin Angular Momentum + Impact Orbital Torque
                    let r_impact = p_pos - s_pos;
                    let v_impact = p_vel - s_vel;
                    let impact_orbital_spin = (p_m * s_m / total_mass) * r_impact.cross(v_impact);
                    let merged_spin = p_spin + s_spin + impact_orbital_spin;

                    // New physical radius from harmonic bulk density
                    let density = merged_comp.average_density();
                    let volume = total_mass / density;
                    let new_radius = ((3.0 * volume) / (4.0 * PI))
                        .cbrt()
                        .max(EARTH_RADIUS_AU * 0.3);

                    // Kinetic energy dissipated into heat
                    let kinetic_loss = 0.5 * ((p_m * s_m) / total_mass) * v_rel * v_rel;
                    let delta_temp = (kinetic_loss * 5e5).clamp(0.0, 4000.0);
                    let new_temp = (temp1.max(temp2) + delta_temp).min(10000.0);

                    let is_star_like = p_type.is_star_or_remnant();
                    let updated_type = if is_star_like {
                        p_type
                    } else {
                        classify_body_by_mass_and_comp(total_mass, &merged_comp, false)
                    };

                    let r_len = merged_pos.length().max(1e-4);
                    let new_acc = if !p_is_central {
                        -(G_ASTRO * star_mass / (r_len * r_len * r_len)) * merged_pos
                    } else {
                        DVec3::ZERO
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
                    )) = bodies_query.get_mut(primary_entity)
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

                        if matches!(
                            updated_type,
                            BodyType::TerrestrialPlanet
                                | BodyType::SuperEarth
                                | BodyType::GasGiant
                                | BodyType::IceGiant
                                | BodyType::Protoplanet
                                | BodyType::Planetesimal
                        ) && (body.name.contains("Comet")
                            || body.name.contains("Asteroid")
                            || (!body.name.starts_with("Planet")
                                && matches!(
                                    updated_type,
                                    BodyType::TerrestrialPlanet
                                        | BodyType::SuperEarth
                                        | BodyType::GasGiant
                                        | BodyType::IceGiant
                                )))
                        {
                            body.name = match updated_type {
                                BodyType::TerrestrialPlanet => {
                                    "Planet (Terrestrial World)".to_string()
                                }
                                BodyType::SuperEarth => "Planet (Super-Earth)".to_string(),
                                BodyType::GasGiant => "Planet (Gas Giant)".to_string(),
                                BodyType::IceGiant => "Planet (Ice Giant)".to_string(),
                                BodyType::BrownDwarf => "Sub-Stellar Brown Dwarf".to_string(),
                                BodyType::Protoplanet => "Protoplanet (Embryo)".to_string(),
                                BodyType::Planetesimal => "Planetesimal".to_string(),
                                _ => body.name.clone(),
                            };
                        }

                        // Update internal core differentiation
                        if let Some(mut diff) = opt_diff {
                            diff.recalculate(total_mass, new_radius, &merged_comp);
                        }

                        // Update spin state and rotation period
                        if let Some(mut spin) = opt_spin_mut {
                            spin.update_from_spin(merged_spin, total_mass, new_radius);
                        }
                    }

                    // Deliver volatile water & atmosphere inventory from icy comets / planetesimals
                    let d_water_earth = (s_m * s_comp.ice_frac) / EARTH_MASS_SOLAR;
                    let d_gas_earth = (s_m * s_comp.gas_frac) / EARTH_MASS_SOLAR;

                    if let Ok(mut p_cmd) = commands.get_entity(primary_entity) {
                        p_cmd
                            .entry::<VolatileInventory>()
                            .and_modify(move |mut vol| {
                                vol.delivered_water_m_earth += d_water_earth;
                                vol.cometary_impact_count += 1;
                                vol.ocean_coverage_frac =
                                    (vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
                                vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar
                                    + (d_gas_earth * 120.0) as f32)
                                    .clamp(0.01, 90.0);
                            })
                            .or_insert(VolatileInventory {
                                delivered_water_m_earth: d_water_earth,
                                ocean_coverage_frac: (d_water_earth / 0.0006).clamp(0.0, 0.85)
                                    as f32,
                                atmospheric_pressure_bar: (d_gas_earth * 120.0).clamp(0.01, 90.0)
                                    as f32,
                                cometary_impact_count: 1,
                            });

                        // Major impact crater melt basin formation
                        let norm = (s_pos - p_pos).normalize_or_zero();
                        let basin = ImpactBasin {
                            surface_normal: Vec3::new(norm.x as f32, norm.y as f32, norm.z as f32),
                            angular_radius: ((s_m / p_m).cbrt() as f32).clamp(0.12, 0.55),
                            formation_time_yr: sim_time.elapsed_years,
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

                    // Seamlessly transfer player selection if secondary entity was merged
                    if player_state.selected_entity == Some(secondary_entity) {
                        player_state.selected_entity = Some(primary_entity);
                    }

                    merged_away.insert(secondary_entity);
                    if !pending_despawns.contains(&secondary_entity) {
                        pending_despawns.push(secondary_entity);
                    }

                    merge_events.write(AccretionMergeEvent {
                        primary_entity,
                        secondary_entity,
                        merged_mass: total_mass,
                        merged_position: merged_pos,
                        merged_velocity: merged_vel,
                        new_body_type: updated_type,
                        energy_released: kinetic_loss,
                    });
                }
            } else {
                // ==========================================
                // REGIME D: CIRCUMPLANETARY AEROCAPTURE
                // ==========================================
                // If a small body passes deep within the Hill sphere of a larger gas-rich planet,
                // circumplanetary gas drag bleeds off orbital energy, capturing it into a stable moon orbit.
                let (
                    primary_entity,
                    p_m,
                    p_pos,
                    p_type,
                    secondary_entity,
                    s_m,
                    _s_pos,
                    _s_vel,
                    _s_name,
                ) = if m1 >= m2 {
                    (e1, m1, pos1, type1, e2, m2, pos2, vel2, name2.clone())
                } else {
                    (e2, m2, pos2, type2, e1, m1, pos1, vel1, name1.clone())
                };

                let is_gas_rich = matches!(
                    p_type,
                    BodyType::GasGiant | BodyType::IceGiant | BodyType::Protoplanet
                );
                let valid_mass_ratio = p_m >= EARTH_MASS_SOLAR * 0.1
                    && s_m <= p_m * 0.05
                    && s_m >= EARTH_MASS_SOLAR * 1e-8;

                if is_gas_rich && valid_mass_ratio && !p_type.is_star_or_remnant() {
                    let orbit_radius = p_pos.length().max(1e-4);
                    let hill_radius = orbit_radius * (p_m / (3.0 * star_mass)).cbrt();

                    // Must pass deep inside the Hill sphere (where circumplanetary gas is dense)
                    if min_dist < hill_radius * 0.4 {
                        let v_esc_local = (2.0 * G_ASTRO * p_m / min_dist.max(1e-6)).sqrt();

                        // Relax capture mechanics simulating gas drag and multi-body interactions
                        if v_rel < v_esc_local * 1.5 && v_rel > v_esc_local * 0.05 {
                            // Captured!
                            let orbit_dist_au = min_dist;
                            let p_moon_yr =
                                2.0 * PI * (orbit_dist_au.powi(3) / (G_ASTRO * p_m)).sqrt();

                            // Convert secondary body into a captured Moon
                            if let Ok((_, _, _, _, _, _, _, _, mut body, _, _, opt_sat_mut, _)) =
                                bodies_query.get_mut(secondary_entity)
                            {
                                // Only capture if it isn't already a moon
                                if !matches!(body.body_type, BodyType::Moon) {
                                    body.body_type = BodyType::Moon;
                                    body.name = format!("Captured {}", _s_name);

                                    if let Some(mut sat) = opt_sat_mut {
                                        sat.parent = primary_entity;
                                        sat.semi_major_axis_au = orbit_dist_au;
                                        sat.orbital_period_years = p_moon_yr;
                                        sat.true_anomaly = 0.0;
                                    } else if let Ok(mut s_cmd) =
                                        commands.get_entity(secondary_entity)
                                    {
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
        }
    }

    for entity in pending_despawns {
        if let Ok(mut e_cmd) = commands.get_entity(entity) {
            e_cmd.despawn();
        }
    }
}

/// Directly accretes primordial Hydrogen/Helium gas from the surrounding protoplanetary
/// nebula via hydrodynamic Bondi-Hoyle and Hill sphere gas capture into growing planetary envelopes.
pub fn direct_nebular_gas_accretion(
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    star_query: Query<&IgnitionState, With<CentralStar>>,
    mut bodies_query: Query<
        (
            Entity,
            &mut Mass,
            &SimPosition,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&mut SpinState>,
            Option<&mut VolatileInventory>,
            Option<&mut Temperature>,
        ),
        Without<CentralStar>,
    >,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let gas_scale = config.gas_density_scale as f64;
    if gas_scale <= 0.001 || sim_time.elapsed_years > disk_params.gas_disk_lifetime_yr {
        return;
    }

    let is_ignited = star_query
        .iter()
        .next()
        .map(|ig| ig.is_ignited)
        .unwrap_or(false);

    // Effective timestep scaled by warp
    let dt_yr = config.base_dt_yr * (time_warp.multiplier / 1.0).clamp(1.0, 50.0);
    let star_mass = disk_params.central_star_mass;
    let is_massive_disk = star_mass > 10.0 || disk_params.outer_radius_au > 100.0;

    for (_entity, mut mass, pos, mut rad, mut comp, mut body, opt_diff, opt_spin, mut opt_vol, opt_temp) in
        bodies_query.iter_mut()
    {
        let actual_r = pos.0.length();
        // Strict boundary check: If a body is outside the gaseous disk, there is no ambient gas to accrete!
        if actual_r > disk_params.outer_radius_au || actual_r < disk_params.inner_radius_au {
            continue;
        }
        let r_au = actual_r;
        let m = mass.0;

        // 1. Zone-specific maximum mass and gas envelope saturation limits:
        let (max_gas_mass, max_gas_frac, runaway_threshold_m_earth) = if is_massive_disk {
            // Massive circum-nuclear disk / Little Red Dot:
            // Bodies can grow from protoplanets to giant planets, brown dwarfs, and massive Pop-III stars!
            (500.0, 1.0, 0.1) // Up to 500 Solar Masses, pure primordial gas, early runaway
        } else if r_au < 2.7 {
            // Terrestrial Zone (Mercury, Venus, Earth, Mars):
            // Thin secondary atmosphere (1-2.5% gas fraction), capped at ~0.025 M_Earth of gas
            (0.025 * EARTH_MASS_SOLAR, 0.025, 100.0) // Runaway strictly disabled
        } else if r_au < 5.0 {
            // Asteroid Belt Zone (Ceres, Vesta):
            // Trace volatile envelope (up to 3.5% gas fraction), capped at ~0.04 M_Earth of gas
            (0.04 * EARTH_MASS_SOLAR, 0.035, 100.0)
        } else if r_au < 12.0 {
            // Jupiter Zone:
            // Massive gas giant runaway accretion up to 1.5 M_Jupiter (~480 M_Earth)
            (JUPITER_MASS_SOLAR * 1.5, 0.94, 0.5) // Runaway enabled once core >= 0.5 M_Earth
        } else if r_au < 22.0 {
            // Saturn Zone:
            // Gas giant runaway accretion up to 0.45 M_Jupiter (~140 M_Earth)
            (JUPITER_MASS_SOLAR * 0.45, 0.88, 0.4)
        } else if r_au < 36.0 {
            // Uranus Zone (Ice Giant):
            // Capped at ~20 M_Earth (~15-22% gas envelope, dominated by ices/silicates)
            (20.0 * EARTH_MASS_SOLAR, 0.22, 0.3)
        } else if r_au < 50.0 {
            // Neptune Zone (Ice Giant):
            // Capped at ~22 M_Earth (~15-22% gas envelope)
            (22.0 * EARTH_MASS_SOLAR, 0.22, 0.3)
        } else {
            // Kuiper Belt (Pluto / comets in Solar Nebula):
            // Tenuous ice world atmosphere (< 2% gas)
            (0.02 * EARTH_MASS_SOLAR, 0.02, 100.0)
        };

        if m >= max_gas_mass {
            continue;
        }
        // Only cap gas fraction for terrestrial worlds in the solar nebula to keep thin secondary atmospheres
        if !is_massive_disk && r_au < 2.7 && comp.gas_frac >= max_gas_frac {
            continue;
        }

        // Local ambient gas disk density at orbital radius r
        let local_gas_density = if is_massive_disk {
            // Dense primordial hydrogen cloudlet reservoir: 50,000 M_sun gas in a 250 AU cocoon
            0.0025 * (disk_params.outer_radius_au / r_au).powf(0.5) * gas_scale
        } else if r_au < 2.7 {
            if is_ignited {
                1.2e-4 * (r_au / 1.0).powf(-1.50) * (gas_scale * 0.05 + 0.001)
            } else {
                1.2e-4 * (r_au / 1.0).powf(-1.50) * gas_scale
            }
        } else if r_au < 12.0 {
            if is_ignited {
                // Jupiter / Saturn zone soaks up pushed gas flux!
                1.2e-4 * (r_au / 1.0).powf(-1.50) * gas_scale * 2.5
            } else {
                1.2e-4 * (r_au / 1.0).powf(-1.50) * gas_scale
            }
        } else {
            1.2e-4 * (r_au / 1.0).powf(-1.50) * gas_scale
        };

        // Gravitational capture radius:
        // In massive circum-nuclear disks, Bondi-Hoyle accretion governs gas sweeping in addition to Hill shear
        let r_hill = r_au * (m / (3.0 * star_mass)).cbrt();
        let r_bondi = if is_massive_disk {
            (0.15 * (m / JUPITER_MASS_SOLAR).sqrt()).clamp(0.08, 15.0)
        } else {
            0.0
        };
        let r_capture = r_hill.max(r_bondi);
        let omega_k = (G_ASTRO * star_mass / (r_au * r_au * r_au)).sqrt();

        let m_earth = m / EARTH_MASS_SOLAR;
        let is_runaway = m_earth >= runaway_threshold_m_earth;
        let runaway_boost = if is_runaway {
            // Rapid exponential runaway gas capture for massive outer cores
            (1.0 + (m_earth / 5.0).powf(1.4)).min(40.0)
        } else {
            0.05
        };

        let gap_factor = (1.0 - (m / max_gas_mass)).clamp(0.02, 1.0);
        let c_gas = 180.0 * (config.accretion_rate_multiplier as f64 / 120.0);
        let d_mass_gas = (c_gas
            * r_capture
            * r_capture
            * local_gas_density
            * omega_k
            * dt_yr
            * gap_factor
            * runaway_boost)
            .min(m * 0.05) // Max 5% mass growth per sub-step for numerical stability during active feeding
            .min(max_gas_mass - m);

        if d_mass_gas > 1e-16 {
            let old_mass = m;
            let new_mass = old_mass + d_mass_gas;
            mass.0 = new_mass;

            // Merge pure primordial solar gas into the planet's bulk composition
            *comp = comp.mass_weighted_merge(old_mass, &Composition::solar_gas(), d_mass_gas);
            if !is_massive_disk && r_au < 2.7 {
                comp.gas_frac = comp.gas_frac.clamp(0.005, 0.025);
            }

            // Recalculate physical radius with the new gaseous envelope or stellar structure
            let new_radius = if new_mass >= 0.08 {
                // Main-sequence / Giant star radius: R ~ R_sun * (M / M_sun)^0.8
                (0.00465 * (new_mass / 1.0).powf(0.8)).clamp(0.003, 10.0)
            } else {
                let density = comp.average_density();
                let volume = new_mass / density;
                ((3.0 * volume) / (4.0 * PI))
                    .cbrt()
                    .max(EARTH_RADIUS_AU * 0.2)
            };
            rad.0 = new_radius;

            // Dynamically upgrade body type based on updated mass and composition
            let updated_type = classify_body_by_mass_and_comp(new_mass, &comp, false);
            body.body_type = updated_type;

            // Dynamically update name to reflect current evolutionary stage
            body.name = match updated_type {
                BodyType::Hypergiant => format!("Pop-III Hypergiant ({:.1} M☉)", new_mass),
                BodyType::BlueSupergiant => format!("Pop-III Blue Supergiant ({:.1} M☉)", new_mass),
                BodyType::BlueGiant => format!("Pop-III Blue Giant ({:.1} M☉)", new_mass),
                BodyType::YellowDwarf => format!("Pop-III Yellow Star ({:.2} M☉)", new_mass),
                BodyType::RedDwarf => format!("Red Dwarf ({:.2} M☉)", new_mass),
                BodyType::BrownDwarf => {
                    format!("Brown Dwarf ({:.1} M_J)", new_mass / JUPITER_MASS_SOLAR)
                }
                BodyType::GasGiant => {
                    if new_mass >= JUPITER_MASS_SOLAR {
                        format!("Super-Jupiter ({:.1} M_J)", new_mass / JUPITER_MASS_SOLAR)
                    } else {
                        format!("Planet-{:.0}AU (Gas Giant)", r_au)
                    }
                }
                BodyType::IceGiant => format!("Planet-{:.0}AU (Ice Giant)", r_au),
                BodyType::SuperEarth => format!("Planet-{:.0}AU (Super-Earth)", r_au),
                BodyType::TerrestrialPlanet => format!("Planet-{:.0}AU (Terrestrial)", r_au),
                _ => body.name.clone(),
            };

            // Stellar surface heating for newborn stars
            if let Some(mut temp) = opt_temp {
                if new_mass >= 25.0 {
                    temp.0 = 35_000.0;
                } else if new_mass >= 8.0 {
                    temp.0 = 20_000.0;
                } else if new_mass >= 1.4 {
                    temp.0 = 9_500.0;
                } else if new_mass >= 0.5 {
                    temp.0 = 5_800.0;
                } else if new_mass >= 0.08 {
                    temp.0 = 3_200.0;
                } else if new_mass >= 13.0 * JUPITER_MASS_SOLAR {
                    temp.0 = 1_800.0;
                }
            }

            if let Some(mut diff) = opt_diff {
                diff.recalculate(new_mass, new_radius, &comp);
            }
            if let Some(mut spin) = opt_spin {
                let spin_vec = spin.spin_vector;
                spin.update_from_spin(spin_vec, new_mass, new_radius);
            }
            if let Some(ref mut vol) = opt_vol {
                let gas_growth = d_mass_gas / EARTH_MASS_SOLAR;
                vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar
                    + (gas_growth * 400.0) as f32)
                    .clamp(0.01, if r_au < 2.7 { 95.0 } else { 1000.0 });
            }
        }
    }
}

/// Updates the internal dynamics, super-Eddington accretion, cocoon blowout,
/// and tidal disruptions for a JWST Little Red Dot (Black Hole Star / Quasi-Star).
pub fn update_black_hole_star_dynamics(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut quasi_star_query: Query<(
        Entity,
        &mut BlackHoleStarState,
        &mut Mass,
        &mut Radius,
        &mut Temperature,
        &mut Luminosity,
        &mut CelestialBody,
        &SimPosition,
    )>,
    mut satellites_query: Query<
        (
            Entity,
            &mut Mass,
            &mut SimVelocity,
            &SimPosition,
            &Radius,
            &CelestialBody,
        ),
        Without<BlackHoleStarState>,
    >,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt = sim_time.current_dt_yr;
    if dt <= 0.0 {
        return;
    }

    for (_qs_ent, mut state, mut mass, mut radius, mut temp, mut lum, mut body, qs_pos) in
        quasi_star_query.iter_mut()
    {
        // 1. Super-Eddington Inflow onto Central Black Hole Seed
        if state.super_eddington_active && state.cocoon_mass_solar > 10.0 {
            // Eddington accretion rate: dM/dt_Edd ≈ 2.2e-8 * M_BH M_sun/yr
            let m_bh = state.black_hole_mass_solar;
            let m_dot_edd = 2.2e-8 * m_bh;
            let actual_rate = m_dot_edd * state.eddington_ratio;
            let dm = (actual_rate * dt * 50.0).min(state.cocoon_mass_solar);

            state.black_hole_mass_solar += dm;
            state.cocoon_mass_solar -= dm;
            state.accreted_envelope_mass += dm;

            mass.0 = state.total_mass_solar();
        }

        // 2. Cocoon Blowout & Quasar Emergence
        if state.is_blown_out {
            state.blowout_progress = (state.blowout_progress + 0.45 * dt as f32).min(1.0);
            // Relativistic laser beams propagate outward at the speed of light c (~63,241 AU/yr)
            // Light never stops traveling across deep space unless the simulation is paused!
            state.jet_travel_distance_au += crate::utils::constants::SPEED_OF_LIGHT_AU_YR * dt;
            let p = state.blowout_progress as f64;

            // Photosphere expands/disperses, then contracts to naked accretion disk
            if p < 0.5 {
                radius.0 = 60.0 * (1.0 + p * 2.0);
                temp.0 = (3800.0 * (1.0 - p * 0.4)).max(1500.0);
            } else {
                let quasar_factor = (p - 0.5) * 2.0;
                radius.0 = 60.0 * (1.0 - quasar_factor) + 8.0 * quasar_factor;
                temp.0 = 3800.0 * (1.0 - quasar_factor) + 95000.0 * quasar_factor;
                lum.0 = 1.2e7 * (1.0 - quasar_factor) + 1.5e10 * quasar_factor;

                body.body_type = BodyType::BlackHole;
                body.name = format!(
                    "Supermassive Quasar ({:.0} M☉)",
                    state.black_hole_mass_solar
                );
                state.super_eddington_active = true;
            }
        }

        // 3. Tidal Disruption Events (TDE) & Aerodynamic Plunge for infalling bodies
        let bh_m = state.black_hole_mass_solar;
        let cocoon_r = radius.0;

        for (sat_ent, sat_m, mut sat_vel, sat_pos, sat_rad, sat_body) in satellites_query.iter_mut()
        {
            let rel_pos = sat_pos.0 - qs_pos.0;
            let dist = rel_pos.length();

            // Gas drag inside the 60 AU dense hydrogen envelope
            if dist < cocoon_r && !state.is_blown_out {
                let v_dir = sat_vel.0.normalize_or_zero();
                let drag = 0.08 * (1.0 - dist / cocoon_r).powf(1.5) * dt;
                sat_vel.0 -= v_dir * drag;
            }

            // Tidal disruption radius: R_T ≈ R_* * (M_BH / M_*)^(1/3)
            let m_ratio = (bh_m / sat_m.0.max(0.01)).cbrt();
            let r_tidal = (sat_rad.0 * m_ratio).clamp(0.05, 5.0);

            if dist < r_tidal || dist < 0.50 {
                state.cocoon_mass_solar += sat_m.0 * 0.5;
                state.black_hole_mass_solar += sat_m.0 * 0.5;
                mass.0 = state.total_mass_solar();
                lum.0 += 5.0e7;

                info!(
                    "💥 TIDAL DISRUPTION EVENT: '{}' shredded by the 100,000 M☉ Supermassive Black Hole Seed!",
                    sat_body.name
                );

                if let Ok(mut e_cmd) = commands.get_entity(sat_ent) {
                    e_cmd.despawn();
                }
            }
        }
    }
}
