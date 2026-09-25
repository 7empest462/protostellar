//! Late Heavy Bombardment cascade and volatile delivery impactor spawner.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

fn spawn_lhb_impactor(
    commands: &mut Commands,
    disk_params: &DiskParameters,
    star_pos: DVec3,
    star_m: f64,
    c_idx: usize,
    target_info: Option<(Entity, DVec3, DVec3)>,
    current_sim_yr: f64,
) {
    let is_comet = (c_idx % 3) != 1;
    let (mass_solar, rad_au, comp, body_type, name) = if is_comet {
        let m_s = 0.000_25 * EARTH_MASS_SOLAR;
        let r_au = EARTH_RADIUS_AU * 0.06;
        let mut comp = Composition::icy();
        comp.ice_frac = 0.55;
        comp.gas_frac = 0.08;
        (
            m_s,
            r_au,
            comp,
            BodyType::Comet,
            format!("LHB-Comet C/{}", 1900 + (c_idx % 1000)),
        )
    } else {
        let m_s = 0.000_25 * EARTH_MASS_SOLAR;
        let r_au = EARTH_RADIUS_AU * 0.08;
        let mut comp = Composition::carbonaceous();
        comp.ice_frac = 0.30;
        comp.gas_frac = 0.05;
        (
            m_s,
            r_au,
            comp,
            BodyType::Asteroid,
            format!("LHB-Asteroid ({})", 10000 + (c_idx % 90000)),
        )
    };

    let r_spawn = 2.6 + ((c_idx * 17) % 24) as f64 * 0.05;

    let (pos, vel, t_flight, target_ent) = if let Some((t_ent, t_pos, _t_vel)) = target_info {
        let r_t = (t_pos.x * t_pos.x + t_pos.z * t_pos.z).sqrt().max(0.3);
        let phi_t = t_pos.z.atan2(t_pos.x);
        let omega_t = (G_ASTRO * star_m / (r_t * r_t * r_t)).sqrt();
        let q = r_t; // Perihelion matches target's exact orbital distance!
        let a = f64::midpoint(r_spawn, q);
        let t_f = std::f64::consts::PI * (a * a * a / (G_ASTRO * star_m)).sqrt();
        let delta_theta = omega_t * t_f;
        let target_theta = phi_t + delta_theta;
        let spawn_angle = target_theta + std::f64::consts::PI;

        let p = star_pos
            + DVec3::new(
                r_spawn * spawn_angle.cos(),
                t_pos.y,
                r_spawn * spawn_angle.sin(),
            );
        let v_apo = (G_ASTRO * star_m * (2.0 * q) / (r_spawn * (r_spawn + q))).sqrt();
        let u_tan = DVec3::new(-spawn_angle.sin(), 0.0, spawn_angle.cos());
        let v = u_tan * v_apo;
        (p, v, t_f, Some(t_ent))
    } else {
        let angle = ((c_idx * 137) % 360) as f64 * std::f64::consts::PI / 180.0;
        let p = star_pos + DVec3::new(r_spawn * angle.cos(), 0.0, r_spawn * angle.sin());
        let q_target = 1.0;
        let v_tangential =
            (G_ASTRO * star_m * (2.0 * q_target) / (r_spawn * (r_spawn + q_target))).sqrt();
        let v_inward = -v_tangential * 0.15;
        let u_tan = DVec3::new(-angle.sin(), 0.0, angle.cos());
        let u_rad = DVec3::new(angle.cos(), 0.0, angle.sin()).normalize_or_zero();
        let v = u_tan * v_tangential + u_rad * v_inward;
        (p, v, 1.5, None)
    };

    let mut diff = InternalDifferentiation::default();
    diff.recalculate(mass_solar, rad_au, &comp);

    let temp_k = disk_params.reference_temp_1au * (r_spawn.max(0.1)).powf(-0.5);

    let mut entity_cmd = commands.spawn((
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration(DVec3::ZERO),
        Mass(mass_solar),
        Radius(rad_au),
        Temperature(temp_k),
        comp,
        diff,
        CelestialBody { body_type, name },
        VolatileInventory {
            delivered_water_m_earth: 0.0,
            cometary_impact_count: 0,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: 0.0,
        },
        SpinState::default(),
    ));

    if let Some(t_ent) = target_ent {
        entity_cmd.insert(LhbImpactor {
            target_entity: t_ent,
            spawn_time_yr: current_sim_yr,
            expected_arrival_yr: current_sim_yr + t_flight,
            target_r: r_spawn,
            target_q: pos.length(),
        });
    }
}

type TargetBodyItem = (
    Entity,
    &'static SimPosition,
    &'static SimVelocity,
    &'static CelestialBody,
    &'static Mass,
    &'static Radius,
    Option<&'static VolatileInventory>,
);

type ImpactorBodyItem = (
    Entity,
    &'static mut SimPosition,
    &'static mut SimVelocity,
    &'static LhbImpactor,
    &'static CelestialBody,
    &'static Radius,
);

type TargetQueryFilter = (Without<LhbImpactor>, Without<CentralStar>);
type ImpactorQueryFilter = With<LhbImpactor>;
type StarQueryFilter = (With<CentralStar>, Without<LhbImpactor>);

type TargetBodyQuery<'w, 's> = Query<'w, 's, TargetBodyItem, TargetQueryFilter>;
type ImpactorBodyQuery<'w, 's> = Query<'w, 's, ImpactorBodyItem, ImpactorQueryFilter>;
type StarBodyQuery<'w, 's> = Query<'w, 's, (&'static SimPosition, &'static Mass), StarQueryFilter>;

/// Maintains a steady cascade of planet-crossing impactors (asteroids & comets) during the Late Heavy Bombardment epoch,
/// guiding them inward to strike Earth and deliver volatile surface oceans.
pub fn update_late_heavy_bombardment_cascade(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    disk_params: Res<DiskParameters>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    target_query: TargetBodyQuery,
    mut impactor_query: ImpactorBodyQuery,
    star_query: StarBodyQuery,
    mut cascade_timer: Local<f64>,
    mut cascade_counter: Local<usize>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    if scenario_state.as_deref().is_some_and(|s| {
        s.current_preset != crate::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn
    }) {
        return;
    }

    if !lhb_state.is_active || lhb_state.migration_progress >= 0.98 {
        return;
    }

    let Ok((star_pos, star_mass)) = star_query.single() else {
        return;
    };

    let dt = sim_time.current_dt_yr;
    let current_sim_yr = sim_time.elapsed_years;

    // 1. Advance in-flight impactors, apply gravitational focusing, and handle proximity arrival
    let mut active_impactor_count = 0;
    for (i_ent, mut i_pos, mut i_vel, impactor, _, i_rad) in impactor_query.iter_mut() {
        active_impactor_count += 1;
        if let Ok((_, t_pos, t_vel, _, t_mass, t_rad, _)) = target_query.get(impactor.target_entity)
        {
            let r_rel = t_pos.0 - i_pos.0;
            let dist = r_rel.length();
            let contact_dist = t_rad.0 + i_rad.0;

            // Gravitational focusing towards target planet within encounter corridor
            if dist <= 0.08 && dist > contact_dist * 0.90 {
                let r_dir = r_rel / dist.max(1e-6);
                let g_acc = (G_ASTRO * t_mass.0 / (dist * dist + 1e-7)) * r_dir * 2.5;
                i_vel.0 += g_acc * dt;

                // Inside Hill sphere (~0.018 AU): align closing velocity smoothly into target
                if dist <= 0.018 {
                    let v_rel = i_vel.0 - t_vel.0;
                    let v_closing = r_dir * v_rel.length().max(4.0);
                    let blend = (dt / 0.04).clamp(0.08, 0.45);
                    i_vel.0 = i_vel.0.lerp(t_vel.0 + v_closing, blend);
                }
            }

            // Proximity capture: if entering contact zone or within Hill sphere at expected arrival,
            // pull into contact so accretion merges it cleanly
            let is_imminent_impact = dist <= contact_dist * 1.8
                || (current_sim_yr >= impactor.expected_arrival_yr && dist <= 0.005);

            if is_imminent_impact {
                if dist > contact_dist * 0.90 {
                    i_pos.0 = t_pos.0 - r_rel.normalize_or_zero() * (contact_dist * 0.85);
                }
            } else if current_sim_yr > impactor.expected_arrival_yr + 0.6 {
                // Impactor passed perihelion and missed; despawn once traveling outbound
                let r_sun = (i_pos.0 - star_pos.0).length();
                let v_radial = (i_pos.0 - star_pos.0).dot(i_vel.0);
                if r_sun > 2.2 && v_radial > 0.0 {
                    if let Ok(mut entity_cmd) = commands.get_entity(i_ent) {
                        entity_cmd.despawn();
                    }
                }
            }
        } else {
            // Target was destroyed or missing; despawn stray impactor if far out
            let r_sun = (i_pos.0 - star_pos.0).length();
            if r_sun > 4.5 {
                if let Ok(mut entity_cmd) = commands.get_entity(i_ent) {
                    entity_cmd.despawn();
                }
            }
        }
    }

    *cascade_timer -= dt;

    let star_m = star_mass.0.max(0.1);
    let active_crossers = active_impactor_count;

    // Target Earth first if it needs water (< 71% ocean coverage),
    // otherwise distribute remaining bombardments across Moon, Mars, or Venus
    let target_earth = target_query
        .iter()
        .find(|(_, pos, _, body, _, _, opt_vol)| {
            let is_earth = body.name.contains("Earth")
                || (body.body_type.is_planet()
                    && ((pos.0 - star_pos.0).length() - 1.0).abs() < 0.35);
            let needs_water = opt_vol.is_none_or(|v| v.ocean_coverage_frac < 0.71);
            is_earth && needs_water
        })
        .or_else(|| {
            target_query.iter().find(|(_, _, _, body, _, _, _)| {
                body.name.contains("Moon")
                    || body.name.contains("Mars")
                    || body.name.contains("Venus")
            })
        })
        .or_else(|| {
            target_query
                .iter()
                .find(|(_, _, _, body, _, _, _)| body.body_type.is_planet())
        })
        .map(|(e, pos, vel, _, _, _, _)| (e, pos.0 - star_pos.0, vel.0));

    let target_crossers = 6;
    if active_crossers < target_crossers && *cascade_timer <= 0.0 {
        *cascade_counter += 1;
        let c_idx = *cascade_counter;

        *cascade_timer = if active_crossers < 3 { 0.25 } else { 1.5 };

        spawn_lhb_impactor(
            &mut commands,
            &disk_params,
            star_pos.0,
            star_m,
            c_idx,
            target_earth,
            current_sim_yr,
        );
        lhb_state.comets_scattered = (lhb_state.comets_scattered + 1).min(100_000);
    }
}
