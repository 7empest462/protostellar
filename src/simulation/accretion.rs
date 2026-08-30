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
    mut player_state: ResMut<PlayerInteractionState>,
    disk_params: Res<DiskParameters>,
    mut merge_events: MessageWriter<AccretionMergeEvent>,
    mut moon_events: MessageWriter<MoonFormationEvent>,
    mut bounce_events: MessageWriter<CollisionBounceEvent>,
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
    )> = bodies_query
        .iter()
        .map(
            |(e, m, pos, vel, _, rad, temp, comp, body, _, opt_spin, _)| {
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
        let (e1, m1, pos1, vel1, rad1, temp1, comp1, type1, spin1, name1) = bodies[i].clone();
        if merged_away.contains(&e1) {
            continue;
        }

        for (e2, m2, pos2, vel2, rad2, temp2, comp2, type2, spin2, name2) in
            bodies.iter().skip(i + 1).cloned()
        {
            if merged_away.contains(&e2) {
                continue;
            }

            let r_rel = pos1 - pos2;
            let dist = r_rel.length();

            // Visual and Gravitational Hill Sphere Collision Cross-Section
            let r_vis_1 =
                (SimulationConfig::calc_render_radius(m1, type1) * config.body_render_scale) as f64;
            let r_vis_2 =
                (SimulationConfig::calc_render_radius(m2, type2) * config.body_render_scale) as f64;

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

                // Sort into primary (larger) and secondary (smaller impactor)
                let (
                    primary_entity,
                    p_m,
                    p_pos,
                    p_vel,
                    p_comp,
                    p_type,
                    p_spin,
                    p_name,
                    secondary_entity,
                    s_m,
                    s_pos,
                    s_vel,
                    s_comp,
                    _s_type,
                    s_spin,
                    _s_name,
                ) = if m1 >= m2 {
                    (
                        e1,
                        m1,
                        pos1,
                        vel1,
                        comp1,
                        type1,
                        spin1,
                        name1.clone(),
                        e2,
                        m2,
                        pos2,
                        vel2,
                        comp2,
                        type2,
                        spin2,
                        name2.clone(),
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
                        e1,
                        m1,
                        pos1,
                        vel1,
                        comp1,
                        type1,
                        spin1,
                        name1.clone(),
                    )
                };

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
                    && !matches!(p_type, BodyType::Protostar | BodyType::MainSequenceStar)
                    && !matches!(type2, BodyType::Protostar | BodyType::MainSequenceStar);

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

                    // Composition blending for primary
                    let primary_comp = p_comp.mass_weighted_merge(p_m, &s_comp, accreted_mass);

                    // Primary physical radius
                    let primary_density = primary_comp.average_density();
                    let primary_volume = total_primary_mass / primary_density;
                    let primary_radius = ((3.0 * primary_volume) / (4.0 * PI))
                        .cbrt()
                        .max(EARTH_RADIUS_AU * 0.3);

                    // Off-center impact angular torque gives the primary an axial tilt and spin
                    let r_impact = p_pos - s_pos;
                    let v_impact = p_vel - s_vel;
                    let impact_orbital_spin =
                        (p_m * accreted_mass / total_primary_mass) * r_impact.cross(v_impact);
                    let primary_spin = p_spin + impact_orbital_spin;

                    // Moon physical properties: Mantle-silicate rich debris
                    let moon_comp = Composition::silicate_rich();
                    let moon_density = moon_comp.average_density();
                    let moon_volume = moon_mass / moon_density;
                    let moon_radius = ((3.0 * moon_volume) / (4.0 * PI))
                        .cbrt()
                        .max(EARTH_RADIUS_AU * 0.15);

                    // Circumplanetary orbital distance (safely outside fluid Roche limit)
                    let orbit_dist_au = primary_radius * (3.5 + 2.5 * b);
                    let v_orbit_speed = (G_ASTRO * total_primary_mass / orbit_dist_au).sqrt();
                    let p_moon_yr =
                        2.0 * PI * (orbit_dist_au.powi(3) / (G_ASTRO * total_primary_mass)).sqrt();

                    // Orbital plane geometry from collision vectors
                    let r_dir = if r_rel.length() > 1e-8 {
                        r_rel.normalize()
                    } else {
                        DVec3::X
                    };
                    let mut ang_dir = r_rel.cross(v_rel_vec);
                    if ang_dir.length() < 1e-8 {
                        ang_dir = DVec3::Y;
                    } else {
                        ang_dir = ang_dir.normalize();
                    }
                    let tang_dir = ang_dir.cross(r_dir).normalize();

                    let moon_pos = primary_pos + r_dir * orbit_dist_au;
                    let moon_vel = primary_vel + tang_dir * v_orbit_speed;

                    // Update Primary Planet in ECS
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
                    )) = bodies_query.get_mut(primary_entity)
                    {
                        m.0 = total_primary_mass;
                        pos.0 = primary_pos;
                        vel.0 = primary_vel;
                        rad.0 = primary_radius;
                        t.0 = (t.0 + 600.0).min(5000.0); // Impact heating
                        *comp = primary_comp;

                        // Upgrade body type if crossed mass threshold or composition archetype
                        if primary_comp.gas_frac > 0.35
                            && total_primary_mass >= EARTH_MASS_SOLAR * 0.5
                        {
                            body.body_type = BodyType::GasGiant;
                        } else if primary_comp.ice_frac > 0.40
                            && total_primary_mass >= EARTH_MASS_SOLAR * 0.5
                        {
                            body.body_type = BodyType::IceGiant;
                        } else if total_primary_mass >= JUPITER_MASS_SOLAR * 0.03 {
                            body.body_type = if primary_comp.gas_frac > 0.3 {
                                BodyType::GasGiant
                            } else {
                                BodyType::IceGiant
                            };
                        } else if total_primary_mass >= EARTH_MASS_SOLAR * 0.1 {
                            body.body_type = BodyType::TerrestrialPlanet;
                        } else if total_primary_mass >= EARTH_MASS_SOLAR * 0.005 {
                            body.body_type = BodyType::Protoplanet;
                        }

                        let r_len = primary_pos.length().max(1e-4);
                        acc.0 = -(G_ASTRO * star_mass / (r_len * r_len * r_len)) * primary_pos;

                        if let Some(mut diff) = opt_diff {
                            diff.recalculate(total_primary_mass, primary_radius, &primary_comp);
                        }
                        if let Some(mut spin) = opt_spin_mut {
                            spin.update_from_spin(primary_spin, total_primary_mass, primary_radius);
                        }
                    }

                    // Convert Secondary Body into the newly spawned Moon in ECS
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
                            // Tidally locked
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

                        if let Ok((_, _, _, mut v1, _, _, _, _, _, _, _, _)) =
                            bodies_query.get_mut(e1)
                        {
                            v1.0 += impulse / m1;
                        }
                        if let Ok((_, _, _, mut v2, _, _, _, _, _, _, _, _)) =
                            bodies_query.get_mut(e2)
                        {
                            v2.0 -= impulse / m2;
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

                    // Exact Conservation of Linear Momentum
                    let merged_vel = (p_vel * p_m + s_vel * s_m) / total_mass;
                    let merged_pos = (p_pos * p_m + s_pos * s_m) / total_mass;

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

                    let updated_type =
                        if matches!(p_type, BodyType::Protostar | BodyType::MainSequenceStar) {
                            p_type
                        } else if merged_comp.gas_frac > 0.35
                            && total_mass >= EARTH_MASS_SOLAR * 0.5
                        {
                            BodyType::GasGiant
                        } else if merged_comp.ice_frac > 0.40
                            && total_mass >= EARTH_MASS_SOLAR * 0.5
                        {
                            BodyType::IceGiant
                        } else if total_mass >= JUPITER_MASS_SOLAR * 0.03 {
                            if merged_comp.gas_frac > 0.3 {
                                BodyType::GasGiant
                            } else {
                                BodyType::IceGiant
                            }
                        } else if total_mass >= EARTH_MASS_SOLAR * 0.1 {
                            BodyType::TerrestrialPlanet
                        } else if total_mass >= EARTH_MASS_SOLAR * 0.005 {
                            BodyType::Protoplanet
                        } else {
                            BodyType::Planetesimal
                        };

                    let r_len = merged_pos.length().max(1e-4);
                    let new_acc = if !matches!(
                        updated_type,
                        BodyType::Protostar | BodyType::MainSequenceStar
                    ) {
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
                    )) = bodies_query.get_mut(primary_entity)
                    {
                        m.0 = total_mass;
                        pos.0 = merged_pos;
                        vel.0 = merged_vel;
                        acc.0 = new_acc;
                        rad.0 = new_radius;
                        t.0 = new_temp;
                        *comp = merged_comp;
                        body.body_type = updated_type;

                        if matches!(
                            updated_type,
                            BodyType::TerrestrialPlanet | BodyType::GasGiant | BodyType::IceGiant
                        ) && !body.name.starts_with("Planet")
                        {
                            body.name = format!("Planet ({:?})", updated_type);
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

                if is_gas_rich
                    && valid_mass_ratio
                    && !matches!(p_type, BodyType::Protostar | BodyType::MainSequenceStar)
                {
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
                            if let Ok((_, _, _, _, _, _, _, _, mut body, _, _, opt_sat_mut)) =
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

    // Effective timestep scaled by warp
    let dt_yr = config.base_dt_yr * (time_warp.multiplier / 1.0).clamp(1.0, 50.0);
    let star_mass = disk_params.central_star_mass;

    for (_entity, mut mass, pos, mut rad, mut comp, mut body, opt_diff, opt_spin) in
        bodies_query.iter_mut()
    {
        let r_au = pos
            .0
            .length()
            .clamp(disk_params.inner_radius_au, disk_params.outer_radius_au);
        let m = mass.0;

        // Gas accretion occurs when core mass exceeds critical threshold (~0.02 Earth masses)
        // Planetary runaway gas accretion limit: Gap opening & disk clearance caps planet to ~3.5 M_Jupiter
        let max_planet_gas_mass = JUPITER_MASS_SOLAR * 3.5;
        if m >= max_planet_gas_mass {
            continue;
        }

        // Ambient gas disk density at orbital distance r (M_sun / AU^3)
        // Midplane gas density rho_gas ~ rho_0 * (r / 1 AU)^-2.25 * gas_scale
        let rho_gas = 1.2e-4 * (r_au / 1.0).powf(-2.25) * gas_scale;

        // Hill radius R_H = r * (M / 3 M_star)^(1/3)
        let r_hill = r_au * (m / (3.0 * star_mass)).cbrt();

        // Local Keplerian angular velocity Omega_K = sqrt(G M_star / r^3)
        let omega_k = (G_ASTRO * star_mass / (r_au * r_au * r_au)).sqrt();

        // Hydrodynamic gas envelope inflow rate: dM/dt = C_gas * R_H^2 * rho_gas * Omega_K
        // Gap factor slows accretion as planet carves an annular gap in the disk
        let gap_factor = (1.0 - (m / max_planet_gas_mass)).clamp(0.05, 1.0);
        let c_gas = 80.0 * (config.accretion_rate_multiplier as f64 / 120.0);
        let d_mass_gas = (c_gas * r_hill * r_hill * rho_gas * omega_k * dt_yr * gap_factor)
            .min(m * 0.005) // 0.5% max growth per step for physical stability
            .min(max_planet_gas_mass - m);

        if d_mass_gas > 1e-16 {
            let old_mass = m;
            let new_mass = old_mass + d_mass_gas;
            mass.0 = new_mass;

            // Merge pure primordial solar gas into the planet's bulk composition
            *comp = comp.mass_weighted_merge(old_mass, &Composition::solar_gas(), d_mass_gas);

            // Recalculate physical radius with the new gaseous envelope
            let density = comp.average_density();
            let volume = new_mass / density;
            let new_radius = ((3.0 * volume) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.2);
            rad.0 = new_radius;

            // Dynamically upgrade body type based on gas & ice fraction
            if comp.gas_frac > 0.35 && new_mass >= EARTH_MASS_SOLAR * 0.5 {
                body.body_type = BodyType::GasGiant;
            } else if (comp.ice_frac > 0.30 && comp.gas_frac > 0.10)
                || (comp.ice_frac > 0.40 && new_mass >= EARTH_MASS_SOLAR * 0.4)
            {
                body.body_type = BodyType::IceGiant;
            }

            if let Some(mut diff) = opt_diff {
                diff.recalculate(new_mass, new_radius, &comp);
            }
            if let Some(mut spin) = opt_spin {
                let spin_vec = spin.spin_vector;
                spin.update_from_spin(spin_vec, new_mass, new_radius);
            }
        }
    }
}
