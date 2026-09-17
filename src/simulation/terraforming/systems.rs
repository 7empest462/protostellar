//! Systems updating in-flight guided bombardment projectiles and planetary terraforming climate.

use bevy::prelude::*;

use super::climate::update_body_terraforming_climate;
use super::delivery::calculate_impact_delivery;
use super::types::*;
use crate::simulation::accretion::basins::record_impact_crater_basin;
use crate::simulation::components::*;
use crate::simulation::resources::{SimTime, SimulationConfig, TimeWarp};
use crate::utils::constants::*;

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
        &SimVelocity,
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

    for (p_ent, p_pos, p_vel, p_mass, p_rad, p_comp, mut proj) in projectile_query.iter_mut() {
        if proj.is_detonated {
            continue;
        }

        let Ok((
            t_ent,
            t_pos,
            t_vel,
            t_mass,
            t_rad,
            mut t_temp,
            _t_comp,
            t_body,
            opt_vol,
            opt_terra,
            opt_climate,
            mut opt_diff,
        )) = target_query.get_mut(proj.target_entity)
        else {
            // Target was consumed, shattered, or despawned
            commands.entity(p_ent).despawn();
            continue;
        };

        let disp = p_pos.0 - t_pos.0;
        let dist = disp.length();
        let contact_dist = (t_rad.0 + p_rad.0 * 2.0).max(t_rad.0 * 1.15);

        let is_impact = dist <= contact_dist
            || (sim_time.elapsed_years >= proj.expected_arrival_yr && dist <= contact_dist * 4.0);

        if !is_impact {
            continue;
        }

        proj.is_detonated = true;

        let v_rel = (p_vel.0 - t_vel.0).length();
        let curr_p = opt_terra.as_ref().map_or_else(
            || opt_vol.as_ref().map_or(0.0, |v| v.atmospheric_pressure_bar),
            |t| t.total_pressure_bar(),
        );

        let delivery = calculate_impact_delivery(
            proj.bombardment_type,
            p_mass.0,
            p_comp,
            v_rel,
            t_mass.0,
            t_rad.0,
            curr_p,
            t_temp.0,
        );

        apply_impact_delivery_to_target(
            &mut commands,
            t_ent,
            opt_vol,
            opt_terra,
            opt_climate,
            &delivery,
            p_mass.0,
        );

        if let Some(ref mut diff) = opt_diff {
            diff.core_temp_k =
                (diff.core_temp_k + delivery.core_temp_boost_k).clamp(300.0, 35000.0);
            if diff.is_differentiated && delivery.core_temp_boost_k > 50.0 {
                diff.magnetic_field_gauss = (diff.magnetic_field_gauss + 0.05).min(5.0);
            }
        }

        // Thermal flash on surface
        t_temp.0 = (t_temp.0 + delivery.core_temp_boost_k * 0.35).clamp(30.0, 10000.0);

        // Record glowing impact crater basin
        let norm = disp.normalize_or_zero();
        let surface_normal = Vec3::new(norm.x as f32, norm.y as f32, norm.z as f32);
        record_impact_crater_basin(
            &mut commands,
            t_ent,
            surface_normal,
            delivery.crater_angular_radius,
            sim_time.elapsed_years,
            1.0,
        );

        event_writer.write(BombardmentEvent {
            target_entity: t_ent,
            target_name: t_body.name.clone(),
            bombardment_type: proj.bombardment_type,
            water_delivered_m_earth: delivery.liquid_water_delivered_m_earth
                + delivery.steam_vapor_delivered_m_earth,
            gas_added_bar: delivery.delta_co2_bar + delivery.delta_nitrogen_bar,
            impact_velocity_km_s: delivery.impact_speed_km_s,
        });

        commands.entity(p_ent).despawn();
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
        vol.ocean_coverage_frac = (vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
        vol.atmospheric_pressure_bar =
            (vol.atmospheric_pressure_bar + delivery.delta_co2_bar + delivery.delta_nitrogen_bar)
                .clamp(0.0, 150.0);
    } else {
        commands.entity(target_ent).insert(VolatileInventory {
            delivered_water_m_earth: total_delivered_water,
            ocean_coverage_frac: (total_delivered_water / 0.0006).clamp(0.0, 0.85) as f32,
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
    } else {
        commands.entity(target_ent).insert(TerraformingAtmosphere {
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
