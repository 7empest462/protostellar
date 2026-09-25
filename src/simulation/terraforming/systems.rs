//! Systems updating in-flight guided bombardment projectiles and planetary terraforming climate.

use bevy::prelude::*;

use super::climate::update_body_terraforming_climate;
use super::delivery::calculate_impact_delivery;
use super::types::*;
use crate::simulation::accretion::basins::record_impact_crater_basin;
use crate::simulation::components::*;
use crate::simulation::resources::{SimTime, SimulationConfig, TimeWarp};
use crate::utils::constants::*;

use bevy::math::DVec3;

struct TargetImpactContext<'a> {
    entity: Entity,
    name: &'a str,
    mass_solar: f64,
    radius_au: f64,
    temp: Mut<'a, Temperature>,
    opt_vol: Option<Mut<'a, VolatileInventory>>,
    opt_terra: Option<Mut<'a, TerraformingAtmosphere>>,
    opt_climate: Option<Mut<'a, PlanetaryClimate>>,
    opt_diff: Option<Mut<'a, InternalDifferentiation>>,
}

struct BombardmentImpactParams<'a> {
    elapsed_years: f64,
    projectile_mass_solar: f64,
    projectile_comp: &'a Composition,
    bombardment_type: BombardmentType,
    relative_velocity: f64,
    disp: DVec3,
}

/// Continuous Collision Detection (CCD):
/// Reconstructs relative trajectory segment over time step dt to detect swept-sphere intersections.
fn check_swept_impact(
    disp: DVec3,
    dist: f64,
    v_rel_vec: DVec3,
    dt: f64,
    contact_dist: f64,
    arrival_elapsed: bool,
) -> bool {
    let step_disp = v_rel_vec * dt;
    let p_prev_rel = disp - step_disp;
    let seg_len_sq = step_disp.length_squared();

    let swept_dist = if seg_len_sq > 1e-12 {
        let u = (-p_prev_rel.dot(step_disp) / seg_len_sq).clamp(0.0, 1.0);
        (p_prev_rel + step_disp * u).length()
    } else {
        dist
    };

    dist <= contact_dist
        || swept_dist <= contact_dist
        || (arrival_elapsed && dist <= contact_dist * 5.0)
}

/// Active Closed-Loop Proportional Navigation Guidance:
/// Continually steers relative velocity vector directly toward target center.
fn steer_proportional_navigation(
    p_vel: &mut DVec3,
    disp: DVec3,
    dist: f64,
    t_vel: DVec3,
    expected_arrival_yr: f64,
    elapsed_years: f64,
    dt: f64,
) {
    let time_left = (expected_arrival_yr - elapsed_years).max(dt);
    let desired_closing_speed = (dist / time_left).clamp(1.0, 50.0);
    let dir_to_target = -disp.normalize_or_zero();
    let v_desired = t_vel + dir_to_target * desired_closing_speed;

    let steer_tau = (0.20 * time_left).max(dt);
    let blend = (dt / steer_tau).clamp(0.0, 1.0);
    let delta_v = (v_desired - *p_vel) * blend;
    *p_vel += delta_v;
}

fn resolve_bombardment_impact(
    commands: &mut Commands,
    event_writer: &mut MessageWriter<BombardmentEvent>,
    params: BombardmentImpactParams<'_>,
    mut target: TargetImpactContext<'_>,
) {
    let curr_p = target.opt_terra.as_ref().map_or_else(
        || {
            target
                .opt_vol
                .as_ref()
                .map_or(0.0, |v| v.atmospheric_pressure_bar)
        },
        |t| t.total_pressure_bar(),
    );

    let delivery = calculate_impact_delivery(
        params.bombardment_type,
        params.projectile_mass_solar,
        params.projectile_comp,
        params.relative_velocity,
        target.mass_solar,
        target.radius_au,
        curr_p,
        target.temp.0,
    );

    apply_impact_delivery_to_target(
        commands,
        target.entity,
        target.opt_vol,
        target.opt_terra,
        target.opt_climate,
        &delivery,
        params.projectile_mass_solar,
    );

    if let Some(ref mut diff) = target.opt_diff {
        diff.core_temp_k = (diff.core_temp_k + delivery.core_temp_boost_k).clamp(300.0, 35000.0);
        if diff.is_differentiated && delivery.core_temp_boost_k > 50.0 {
            diff.magnetic_field_gauss = (diff.magnetic_field_gauss + 0.05).min(5.0);
        }
    }

    // Thermal flash on surface
    target.temp.0 = (target.temp.0 + delivery.core_temp_boost_k * 0.35).clamp(30.0, 10000.0);

    // Record glowing impact crater basin
    let norm = params.disp.normalize_or_zero();
    let surface_normal = Vec3::new(norm.x as f32, norm.y as f32, norm.z as f32);
    record_impact_crater_basin(
        commands,
        target.entity,
        surface_normal,
        delivery.crater_angular_radius,
        params.elapsed_years,
        1.0,
    );

    event_writer.write(BombardmentEvent {
        target_entity: target.entity,
        target_name: target.name.to_string(),
        bombardment_type: params.bombardment_type,
        water_delivered_m_earth: delivery.liquid_water_delivered_m_earth
            + delivery.steam_vapor_delivered_m_earth,
        gas_added_bar: delivery.delta_co2_bar + delivery.delta_nitrogen_bar,
        impact_velocity_km_s: delivery.impact_speed_km_s,
    });
}

/// Advances guided bombardment projectiles along their intercept corridor, detecting proximity
/// and resolving atmospheric entry and surface impact.
#[allow(clippy::type_complexity, reason = "bevy ECS query complexity")]
pub fn update_guided_bombardment_projectiles(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    _config: Res<SimulationConfig>,
    mut event_writer: MessageWriter<BombardmentEvent>,
    mut projectile_query: Query<(
        Entity,
        &SimPosition,
        &mut SimVelocity,
        &Mass,
        &Radius,
        &Composition,
        &mut BombardmentProjectile,
    )>,
    mut target_query: Query<
        (
            Entity,
            &SimPosition,
            &SimVelocity,
            &Mass,
            &Radius,
            &mut Temperature,
            &mut Composition,
            &CelestialBody,
            Option<&mut VolatileInventory>,
            Option<&mut TerraformingAtmosphere>,
            Option<&mut PlanetaryClimate>,
            Option<&mut InternalDifferentiation>,
        ),
        Without<BombardmentProjectile>,
    >,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    for (p_ent, p_pos, mut p_vel, p_mass, p_rad, p_comp, mut proj) in projectile_query.iter_mut() {
        if proj.is_detonated {
            continue;
        }

        let Ok((
            t_ent,
            t_pos,
            t_vel,
            t_mass,
            t_rad,
            t_temp,
            _t_comp,
            t_body,
            opt_vol,
            opt_terra,
            opt_climate,
            opt_diff,
        )) = target_query.get_mut(proj.target_entity)
        else {
            // Target was consumed, shattered, or despawned
            if let Ok(mut entity_cmd) = commands.get_entity(p_ent) {
                entity_cmd.despawn();
            }
            continue;
        };

        let dt = sim_time.current_dt_yr.max(1e-6);
        let disp = p_pos.0 - t_pos.0;
        let dist = disp.length();
        let contact_dist = (t_rad.0 + p_rad.0 * 2.0).max(t_rad.0 * 1.15);
        let arrival_elapsed = sim_time.elapsed_years >= proj.expected_arrival_yr;

        if !check_swept_impact(
            disp,
            dist,
            p_vel.0 - t_vel.0,
            dt,
            contact_dist,
            arrival_elapsed,
        ) {
            steer_proportional_navigation(
                &mut p_vel.0,
                disp,
                dist,
                t_vel.0,
                proj.expected_arrival_yr,
                sim_time.elapsed_years,
                dt,
            );
            continue;
        }

        proj.is_detonated = true;
        let v_rel = (p_vel.0 - t_vel.0).length();

        resolve_bombardment_impact(
            &mut commands,
            &mut event_writer,
            BombardmentImpactParams {
                elapsed_years: sim_time.elapsed_years,
                projectile_mass_solar: p_mass.0,
                projectile_comp: p_comp,
                bombardment_type: proj.bombardment_type,
                relative_velocity: v_rel,
                disp,
            },
            TargetImpactContext {
                entity: t_ent,
                name: &t_body.name,
                mass_solar: t_mass.0,
                radius_au: t_rad.0,
                temp: t_temp,
                opt_vol,
                opt_terra,
                opt_climate,
                opt_diff,
            },
        );

        if let Ok(mut entity_cmd) = commands.get_entity(p_ent) {
            entity_cmd.despawn();
        }
    }
}

fn apply_impact_delivery_to_target(
    commands: &mut Commands,
    target_ent: Entity,
    opt_vol: Option<Mut<'_, VolatileInventory>>,
    opt_terra: Option<Mut<'_, TerraformingAtmosphere>>,
    mut opt_climate: Option<Mut<'_, PlanetaryClimate>>,
    delivery: &ImpactDeliveryResult,
    p_mass_solar: f64,
) {
    let p_mass_earth = p_mass_solar / EARTH_MASS_SOLAR;
    let total_delivered_water =
        delivery.liquid_water_delivered_m_earth + delivery.steam_vapor_delivered_m_earth;

    if let Some(mut vol) = opt_vol {
        vol.delivered_water_m_earth += total_delivered_water;
        vol.cometary_impact_count += 1;
        vol.ocean_coverage_frac =
            ((vol.delivered_water_m_earth / 0.0006) * 0.71).clamp(0.0, 0.98) as f32;
        vol.atmospheric_pressure_bar =
            (vol.atmospheric_pressure_bar + delivery.delta_co2_bar + delivery.delta_nitrogen_bar)
                .clamp(0.0, 150.0);
    } else if let Ok(mut cmd) = commands.get_entity(target_ent) {
        cmd.try_insert(VolatileInventory {
            delivered_water_m_earth: total_delivered_water,
            ocean_coverage_frac: ((total_delivered_water / 0.0006) * 0.71).clamp(0.0, 0.98) as f32,
            atmospheric_pressure_bar: (delivery.delta_co2_bar + delivery.delta_nitrogen_bar)
                .clamp(0.01, 150.0),
            cometary_impact_count: 1,
        });
    }

    if let Some(mut terra) = opt_terra {
        terra.co2_pressure_bar += delivery.delta_co2_bar;
        terra.nitrogen_pressure_bar += delivery.delta_nitrogen_bar;
        terra.surface_liquid_water_m_earth += delivery.liquid_water_delivered_m_earth;
        terra.atmospheric_water_m_earth += delivery.steam_vapor_delivered_m_earth;
        terra.impact_dust_optical_depth += delivery.delta_dust_opacity;
        terra.total_bombarded_mass_earth += p_mass_earth;
    } else if let Ok(mut cmd) = commands.get_entity(target_ent) {
        cmd.try_insert(TerraformingAtmosphere {
            co2_pressure_bar: delivery.delta_co2_bar,
            nitrogen_pressure_bar: delivery.delta_nitrogen_bar,
            surface_liquid_water_m_earth: delivery.liquid_water_delivered_m_earth,
            atmospheric_water_m_earth: delivery.steam_vapor_delivered_m_earth,
            impact_dust_optical_depth: delivery.delta_dust_opacity,
            total_bombarded_mass_earth: p_mass_earth,
        });
    }

    if let Some(ref mut climate) = opt_climate {
        let added_p = delivery.delta_co2_bar + delivery.delta_nitrogen_bar;
        climate.greenhouse_delta_k += added_p * 8.5;
        climate.surface_temperature_k += added_p * 8.5;
    }
}

/// Periodic climate simulation step advancing condensation, greenhouse balance, and dust settling.
#[allow(clippy::type_complexity, reason = "bevy ECS query complexity")]
pub fn update_terraforming_atmospheres(
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    config: Res<SimulationConfig>,
    mut worlds_query: Query<(
        &mut VolatileInventory,
        &mut TerraformingAtmosphere,
        &mut PlanetaryClimate,
        &mut Composition,
    )>,
) {
    if !config.enable_thermodynamics || (time_warp.is_paused && !time_warp.step_once) {
        return;
    }

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr);

    for (mut vol, mut terra, mut climate, mut comp) in worlds_query.iter_mut() {
        update_body_terraforming_climate(&mut vol, &mut terra, &mut climate, &mut comp, dt_yr);
    }
}
