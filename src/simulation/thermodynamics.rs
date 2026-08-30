//! Thermodynamics, Hayashi Track Protostellar Evolution, and Hydrogen Fusion Ignition.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Event triggered when the central protostar's core reaches 10,000,000 K and ignites hydrogen fusion.
#[derive(Message, Debug, Clone)]
pub struct StarIgnitionEvent {
    pub star_entity: Entity,
    pub star_mass: f64,
    pub luminosity_l_sun: f64,
    pub surface_temp_kelvin: f64,
}

/// Updates stellar thermodynamics, core heating, and handles the dramatic fusion ignition transition.
pub fn update_thermodynamics(
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut config: ResMut<SimulationConfig>,
    mut ignition_events: MessageWriter<StarIgnitionEvent>,
    mut star_query: Query<
        (
            Entity,
            &mut Mass,
            &mut Radius,
            &mut Temperature,
            &mut Luminosity,
            &mut IgnitionState,
            &mut CelestialBody,
        ),
        With<CentralStar>,
    >,
    mut bodies_query: Query<
        (
            &SimPosition,
            &mut Temperature,
            &mut Composition,
            &mut CelestialBody,
        ),
        Without<CentralStar>,
    >,
) {
    if (!config.enable_thermodynamics || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr);

    // 1. Process Protostellar Core Heating & Ignition
    for (entity, mass, mut radius, mut temp, mut lum, mut ignition, mut body) in
        star_query.iter_mut()
    {
        if !ignition.is_ignited {
            // Core temperature heats up via gravitational Kelvin-Helmholtz contraction
            let heating_rate_per_yr = 1.0e4 * mass.0; // Naturally ignites around ~1000 years
            ignition.core_temperature += heating_rate_per_yr * dt_yr;

            let ignition_threshold = 1.0e7; // 10 Million Kelvin (Hydrogen P-P Fusion)
            ignition.fusion_fraction =
                (ignition.core_temperature / ignition_threshold).clamp(0.0, 1.0) as f32;

            // Surface temperature increases as star contracts along Hayashi/Henyey track
            let target_surface_temp =
                3200.0 + (5778.0 - 3200.0) * (ignition.fusion_fraction as f64);
            temp.0 = target_surface_temp;

            // Radius contracts down toward main-sequence equilibrium (1.0 Solar Radius)
            let target_radius =
                SOLAR_RADIUS_AU * (1.0 + 2.0 * (1.0 - ignition.fusion_fraction as f64));
            radius.0 = target_radius;

            // Check if ignition threshold reached!
            if ignition.core_temperature >= ignition_threshold {
                ignition.is_ignited = true;
                ignition.fusion_fraction = 1.0;
                body.body_type = BodyType::MainSequenceStar;
                body.name = "The Star (Main Sequence)".to_string();

                // Main Sequence Mass-Luminosity relation: L/L_sun ~ (M/M_sun)^3.5
                let main_seq_lum = mass.0.powf(3.5);
                lum.0 = main_seq_lum;
                temp.0 = 5778.0 * mass.0.powf(0.505); // Solar effective temp ~ 5778 K
                radius.0 = SOLAR_RADIUS_AU;

                ignition_events.write(StarIgnitionEvent {
                    star_entity: entity,
                    star_mass: mass.0,
                    luminosity_l_sun: lum.0,
                    surface_temp_kelvin: temp.0,
                });
            }
        } else {
            // Star is ignited: expand radiation pressure shockwave & photoevaporate gas disk
            let blast_speed = 3.5; // Smooth outward stellar wind (AU/yr)
            ignition.shockwave_radius += blast_speed * dt_yr;

            // Progressive photo-evaporative clearance over 15,000 years
            let time_decay = (1.0 - (sim_time.elapsed_years / 15_000.0)).clamp(0.0, 1.0) as f32;
            config.gas_density_scale = time_decay;
        }

        // 2. Update Disk Body Temperatures & Planetary Thermal Processing
        let star_lum = lum.0;
        let star_temp = temp.0;
        let shockwave_r = ignition.shockwave_radius;

        for (pos, mut p_temp, mut comp, body) in bodies_query.iter_mut() {
            let r = pos.0.length().max(0.1);
            let equilibrium_temp = star_temp * (radius.0 / (2.0 * r)).sqrt();

            let shock_boost = if shockwave_r > 0.0 && (r - shockwave_r).abs() < 2.5 {
                800.0 * (1.0 - (r - shockwave_r).abs() / 2.5)
            } else {
                0.0
            };

            p_temp.0 = (equilibrium_temp * star_lum.powf(0.25) + shock_boost).clamp(30.0, 5000.0);

            // 3. Photoevaporation & Volatile Ice Sublimation behind Shockwave
            if shockwave_r > r && r < 2.7 && comp.ice_frac > 0.001 {
                // Flash-boil volatile ices on inner terrestrial worlds into space
                let sublimated = (comp.ice_frac * 0.15 * dt_yr).min(comp.ice_frac);
                comp.ice_frac -= sublimated;
                comp.silicate_frac += sublimated * 0.7;
                comp.metal_frac += sublimated * 0.3;

                let sum = comp.silicate_frac
                    + comp.metal_frac
                    + comp.ice_frac
                    + comp.organics_frac
                    + comp.gas_frac;
                if sum > 0.0 {
                    comp.silicate_frac /= sum;
                    comp.metal_frac /= sum;
                    comp.ice_frac /= sum;
                    comp.organics_frac /= sum;
                    comp.gas_frac /= sum;
                }
            }

            // Gas giants retain gas if they are beyond snow line and sufficiently massive
            if matches!(body.body_type, BodyType::GasGiant | BodyType::IceGiant) && r > 3.0 {
                // Protected by deep gravitational potential well
            }
        }
    }
}
