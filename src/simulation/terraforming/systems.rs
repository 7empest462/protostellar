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
    let surface_normal = if norm == DVec3::ZERO {
        Vec3::Y
    } else {
        Vec3::new(norm.x as f32, norm.y as f32, norm.z as f32)
    };
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
        surface_normal,
        crater_angular_radius: delivery.crater_angular_radius,
    });
}

type TargetBodyQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    &'a SimVelocity,
    &'a Mass,
    &'a Radius,
    Mut<'a, Temperature>,
    Mut<'a, Composition>,
    &'a CelestialBody,
    Option<Mut<'a, VolatileInventory>>,
    Option<Mut<'a, TerraformingAtmosphere>>,
    Option<Mut<'a, PlanetaryClimate>>,
    Option<Mut<'a, InternalDifferentiation>>,
    Option<&'a Transform>,
);

#[allow(
    clippy::too_many_arguments,
    reason = "Bombardment guidance step coordinates projectile kinematics and target properties"
)]
fn advance_guided_projectile(
    commands: &mut Commands,
    event_writer: &mut MessageWriter<BombardmentEvent>,
    p_ent: Entity,
    p_pos: &mut SimPosition,
    p_vel: &mut SimVelocity,
    p_mass: f64,
    p_rad: f64,
    p_comp: &Composition,
    proj: &mut BombardmentProjectile,
    target_data: TargetBodyQueryItem<'_>,
    dt_real: f32,
    sim_time: &SimTime,
) {
    let (
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
        opt_target_tf,
    ) = target_data;

    proj.flight_elapsed_secs += dt_real;
    let dt_sim = sim_time.current_dt_yr.max(0.0);
    let flight_dt_progress = if dt_sim > 0.0 {
        let sim_frac = dt_sim / (proj.expected_arrival_yr - proj.launch_time_yr).max(1e-6);
        (sim_frac as f32).max(dt_real / proj.flight_duration_secs.max(0.1))
    } else {
        dt_real / proj.flight_duration_secs.max(0.1)
    };

    let eff_target_radius = opt_target_tf
        .map(|tf| f64::from(tf.scale.x))
        .filter(|&vis_r| vis_r > t_rad.0 && (vis_r - 1.0).abs() > 1e-4)
        .unwrap_or(t_rad.0);
    let contact_dist = (eff_target_radius + p_rad * 2.0).max(eff_target_radius * 1.01);

    // Check if manually positioned right onto target in tests
    let direct_disp = p_pos.0 - t_pos.0;
    let direct_dist = direct_disp.length();
    let current_dist = if direct_dist < proj.relative_offset.length() {
        proj.relative_offset = direct_disp;
        direct_dist
    } else {
        proj.relative_offset.length()
    };

    let approach_dir = if current_dist > 1e-9 {
        proj.relative_offset / current_dist
    } else {
        DVec3::Y
    };

    // Advance inward along approach corridor toward target surface
    let close_step = (proj.initial_distance - contact_dist).max(0.001)
        * f64::from(flight_dt_progress);
    let new_dist = (current_dist - close_step).max(contact_dist);
    proj.relative_offset = approach_dir * new_dist;

    // Keep world position and velocity strictly anchored to moving target
    p_pos.0 = t_pos.0 + proj.relative_offset;
    p_vel.0 = t_vel.0 - approach_dir * 8.0;

    let is_at_surface = new_dist <= contact_dist * 1.002;
    let is_arrival_elapsed = proj.flight_elapsed_secs >= proj.flight_duration_secs
        || sim_time.elapsed_years >= proj.expected_arrival_yr;

    if !is_at_surface && !is_arrival_elapsed {
        return;
    }

    proj.is_detonated = true;
    let v_rel = (p_vel.0 - t_vel.0).length();

    // Impact normal is strictly directed along inbound descent corridor onto target surface
    let impact_disp = approach_dir * eff_target_radius;

    resolve_bombardment_impact(
        commands,
        event_writer,
        BombardmentImpactParams {
            elapsed_years: sim_time.elapsed_years,
            projectile_mass_solar: p_mass,
            projectile_comp: p_comp,
            bombardment_type: proj.bombardment_type,
            relative_velocity: v_rel,
            disp: impact_disp,
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
        entity_cmd.try_despawn();
    }
}

/// Advances guided bombardment projectiles along their intercept corridor, detecting proximity
/// and resolving atmospheric entry and surface impact.
#[allow(clippy::type_complexity, reason = "bevy ECS query complexity")]
pub fn update_guided_bombardment_projectiles(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time: Option<Res<Time>>,
    time_warp: Res<TimeWarp>,
    _config: Res<SimulationConfig>,
    mut event_writer: MessageWriter<BombardmentEvent>,
    mut projectile_query: Query<(
        Entity,
        &mut SimPosition,
        &mut SimVelocity,
        &Mass,
        &Radius,
        &Composition,
        &mut BombardmentProjectile,
    )>,
    mut target_query: Query<TargetBodyQueryItem<'_>, Without<BombardmentProjectile>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt_real = time
        .as_ref()
        .map_or(0.016, |t| t.delta_secs())
        .clamp(0.001, 0.05);

    for (p_ent, mut p_pos, mut p_vel, p_mass, p_rad, p_comp, mut proj) in
        projectile_query.iter_mut()
    {
        if proj.is_detonated {
            continue;
        }

        let Ok(target_data) = target_query.get_mut(proj.target_entity) else {
            // Target was consumed, shattered, or despawned
            if let Ok(mut entity_cmd) = commands.get_entity(p_ent) {
                entity_cmd.try_despawn();
            }
            continue;
        };

        advance_guided_projectile(
            &mut commands,
            &mut event_writer,
            p_ent,
            &mut p_pos,
            &mut p_vel,
            p_mass.0,
            p_rad.0,
            p_comp,
            &mut proj,
            target_data,
            dt_real,
            &sim_time,
        );
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
        &Mass,
    )>,
) {
    if !config.enable_thermodynamics || (time_warp.is_paused && !time_warp.step_once) {
        return;
    }

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr);

    for (mut vol, mut terra, mut climate, mut comp, mass) in worlds_query.iter_mut() {
        update_body_terraforming_climate(
            &mut vol,
            &mut terra,
            &mut climate,
            &mut comp,
            mass.0,
            dt_yr,
        );
    }
}
