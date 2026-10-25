//! Thermodynamics, Hayashi Track Protostellar Evolution, Core Dynamos, Planetary Climate, Biosphere Genesis, and Far-Future Stellar Metamorphosis.

mod climate;
pub mod stellar_lifecycle;
pub use climate::*;
pub use stellar_lifecycle::*;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::rendering::effects::megastructures::DysonSwarm;
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

#[derive(Clone, Copy)]
struct StarThermoInfo {
    entity: Entity,
    pos: DVec3,
    lum: f64,
    temp: f64,
    radius: f64,
    shockwave_radius: f64,
    is_ignited: bool,
    _is_central: bool,
}

#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Star thermodynamics iteration passes query sets and event writers"
)]
fn process_stars_thermodynamics(
    star_query: &mut Query<
        (
            Entity,
            &mut Mass,
            &mut Radius,
            &mut Temperature,
            &mut Luminosity,
            &mut IgnitionState,
            &mut CelestialBody,
            Option<&SimPosition>,
            Option<&mut StellarEvolutionState>,
            Option<&mut ElectromagneticFieldState>,
            Option<&DysonSwarm>,
            Has<CentralStar>,
        ),
        (With<IgnitionState>, With<CelestialBody>),
    >,
    dt_yr: f64,
    elapsed_years: f64,
    is_gmc: bool,
    commands: &mut Commands,
    config: &mut SimulationConfig,
    ignition_events: &mut MessageWriter<StarIgnitionEvent>,
    supernova_events: &mut MessageWriter<SupernovaEvent>,
) -> Vec<StarThermoInfo> {
    let mut stars_info = Vec::new();
    for (
        entity,
        mut mass,
        mut radius,
        mut temp,
        mut lum,
        mut ignition,
        mut body,
        opt_star_pos,
        mut opt_evo,
        opt_em,
        opt_swarm,
        is_central_star,
    ) in star_query.iter_mut()
    {
        let star_pos = opt_star_pos.map_or(DVec3::ZERO, |p| p.0);
        let is_active_supernova = opt_evo
            .as_ref()
            .is_some_and(|e| e.phase == StellarEvolutionPhase::SupernovaExplosion);

        // Quasi-Stars (JWST Little Red Dot) and Black Holes are exotic supermassive objects
        // governed by super-Eddington accretion physics and envelope mechanics in `accretion.rs`.
        // They must NOT undergo standard main-sequence contraction, hydrogen exhaustion, or radius clamping to 0.20 AU!
        if (body.body_type == BodyType::QuasiStar || body.body_type == BodyType::BlackHole)
            && !is_active_supernova
        {
            let mut effective_lum = lum.0;
            if let Some(swarm) = opt_swarm {
                effective_lum *= 1.0 - (f64::from(swarm.completion_fraction) * 0.9);
            }
            stars_info.push(StarThermoInfo {
                entity,
                pos: star_pos,
                lum: effective_lum,
                temp: temp.0,
                radius: radius.0,
                shockwave_radius: ignition.shockwave_radius,
                is_ignited: ignition.is_ignited,
                _is_central: is_central_star,
            });
            continue;
        }

        step_protostar_ignition_and_limits(
            entity,
            &mut mass,
            &mut radius,
            &mut temp,
            &mut lum,
            &mut ignition,
            &mut body,
            &mut opt_evo,
            opt_em,
            config,
            dt_yr,
            elapsed_years,
            commands,
            ignition_events,
            supernova_events,
            is_gmc,
        );

        if let Some(ref mut evo) = opt_evo {
            step_stellar_evolution_cycle(
                entity,
                &mut mass,
                &mut radius,
                &mut temp,
                &mut lum,
                &mut body,
                &mut ignition,
                evo,
                dt_yr,
                supernova_events,
                is_gmc,
            );
        }

        let mut effective_lum = lum.0;
        if let Some(swarm) = opt_swarm {
            effective_lum *= 1.0 - (f64::from(swarm.completion_fraction) * 0.9);
        }
        stars_info.push(StarThermoInfo {
            entity,
            pos: star_pos,
            lum: effective_lum,
            temp: temp.0,
            radius: radius.0,
            shockwave_radius: ignition.shockwave_radius,
            is_ignited: ignition.is_ignited,
            _is_central: is_central_star,
        });
    }
    stars_info
}

/// Updates stellar thermodynamics, core heating, planetary dynamos, greenhouse atmospheres, biospheres, and stellar evolution.
#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
pub fn update_thermodynamics(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut config: ResMut<SimulationConfig>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    mut ignition_events: MessageWriter<StarIgnitionEvent>,
    mut engulfment_events: MessageWriter<PlanetaryEngulfmentEvent>,
    mut supernova_events: MessageWriter<SupernovaEvent>,
    mut star_query: Query<
        (
            Entity,
            &mut Mass,
            &mut Radius,
            &mut Temperature,
            &mut Luminosity,
            &mut IgnitionState,
            &mut CelestialBody,
            Option<&SimPosition>,
            Option<&mut StellarEvolutionState>,
            Option<&mut ElectromagneticFieldState>,
            Option<&DysonSwarm>,
            Has<CentralStar>,
        ),
        (With<IgnitionState>, With<CelestialBody>),
    >,
    mut bodies_query: Query<
        (
            Entity,
            &Mass,
            &SimPosition,
            &mut SimVelocity,
            &mut Temperature,
            &mut Composition,
            &CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&SpinState>,
            Option<&mut VolatileInventory>,
            Option<&mut PlanetaryClimate>,
            Option<&mut BiosphereState>,
            Option<&SatelliteOf>,
        ),
        (Without<CentralStar>, Without<IgnitionState>),
    >,
) {
    if (!config.enable_thermodynamics || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr);
    let is_gmc = scenario_state.is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster
    });

    // 1. Process Protostellar Core Heating, Ignition, and Multi-Branch Stellar Evolution
    let stars_info = process_stars_thermodynamics(
        &mut star_query,
        dt_yr,
        sim_time.elapsed_years,
        is_gmc,
        &mut commands,
        &mut config,
        &mut ignition_events,
        &mut supernova_events,
    );

    // 2. Update Disk Body Temperatures & Planetary Thermal Processing
    if !stars_info.is_empty() {
        update_disk_bodies_thermodynamics(
            &mut commands,
            &mut bodies_query,
            &stars_info,
            dt_yr,
            sim_time.elapsed_years,
            &mut engulfment_events,
        );
    }
}

#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Thermodynamics iteration passes massive query sets and state parameters"
)]
fn update_disk_bodies_thermodynamics(
    commands: &mut Commands,
    bodies_query: &mut Query<
        (
            Entity,
            &Mass,
            &SimPosition,
            &mut SimVelocity,
            &mut Temperature,
            &mut Composition,
            &CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&SpinState>,
            Option<&mut VolatileInventory>,
            Option<&mut PlanetaryClimate>,
            Option<&mut BiosphereState>,
            Option<&SatelliteOf>,
        ),
        (Without<CentralStar>, Without<IgnitionState>),
    >,
    stars_info: &[StarThermoInfo],
    dt_yr: f64,
    elapsed_years: f64,
    engulfment_events: &mut MessageWriter<PlanetaryEngulfmentEvent>,
) {
    for (
        body_ent,
        b_mass,
        pos,
        mut vel,
        mut p_temp,
        mut comp,
        b_body,
        mut opt_diff,
        opt_spin,
        mut opt_vol,
        mut opt_climate,
        mut opt_bio,
        opt_sat,
    ) in bodies_query.iter_mut()
    {
        let Some(fallback_star) = stars_info.first() else {
            continue;
        };
        let host_star = opt_sat
            .and_then(|sat| stars_info.iter().find(|s| s.entity == sat.parent))
            .unwrap_or_else(|| {
                stars_info
                    .iter()
                    .min_by(|a, b| {
                        (pos.0 - a.pos)
                            .length_squared()
                            .partial_cmp(&(pos.0 - b.pos).length_squared())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap_or(fallback_star)
            });

        let rel_pos = pos.0 - host_star.pos;

        update_body_thermodynamics(
            commands,
            body_ent,
            b_mass.0,
            rel_pos,
            &mut vel,
            &mut p_temp,
            &mut comp,
            b_body,
            &mut opt_diff,
            opt_spin,
            &mut opt_vol,
            &mut opt_climate,
            &mut opt_bio,
            host_star.lum,
            host_star.temp,
            host_star.radius,
            host_star.shockwave_radius,
            host_star.is_ignited,
            dt_yr,
            elapsed_years,
            engulfment_events,
        );
    }
}

fn update_body_thermodynamics(
    commands: &mut Commands,
    body_ent: Entity,
    b_mass_solar: f64,
    pos: DVec3,
    vel: &mut SimVelocity,
    p_temp: &mut Temperature,
    comp: &mut Composition,
    b_body: &CelestialBody,
    opt_diff: &mut Option<Mut<'_, InternalDifferentiation>>,
    opt_spin: Option<&SpinState>,
    opt_vol: &mut Option<Mut<'_, VolatileInventory>>,
    opt_climate: &mut Option<Mut<'_, PlanetaryClimate>>,
    opt_bio: &mut Option<Mut<'_, BiosphereState>>,
    star_lum: f64,
    star_temp: f64,
    star_r: f64,
    shockwave_r: f64,
    star_is_ignited: bool,
    dt_yr: f64,
    elapsed_years: f64,
    engulfment_events: &mut MessageWriter<PlanetaryEngulfmentEvent>,
) {
    // Stars and stellar remnants are governed by stellar thermodynamics, not planetary engulfment or climate.
    if b_body.body_type.is_star_or_remnant() {
        return;
    }

    let r = pos.length().max(1e-4);
    let period_hrs = opt_spin.map_or(24.0, |s| s.rotation_period_hours);

    // Stellar envelope drag and engulfment for stars with gaseous photospheres (Main Sequence, Giants, etc.)
    if !b_body.body_type.is_star_or_remnant() && star_temp > 100.0 && r < star_r * 2.0 {
        let drag_coeff = if r < star_r { 0.25 } else { 0.08 };
        vel.0 *= 1.0 - (drag_coeff * dt_yr).min(0.8);

        let is_deep_engulfment = if star_r > 0.15 {
            // For giant envelopes, deep plunge or inner core boundary
            r < 0.18 || r < star_r * 0.20
        } else {
            // For main-sequence / compact stars, penetration into or inside the stellar body
            r <= star_r || r < 0.005
        };

        if is_deep_engulfment {
            engulfment_events.write(PlanetaryEngulfmentEvent {
                planet_entity: body_ent,
                planet_name: b_body.name.clone(),
                distance_au: r,
                planet_mass_earth: b_mass_solar / EARTH_MASS_SOLAR,
            });
            if let Ok(mut cmd) = commands.get_entity(body_ent) {
                cmd.try_despawn();
            }
            return;
        }
    }

    let mut magnetic_field_gauss = 0.0f32;
    if let Some(ref mut diff) = opt_diff {
        if diff.is_differentiated {
            if diff.core_temp_k > 1200.0 {
                let temp_factor = ((diff.core_temp_k - 1200.0) / 2000.0).clamp(0.0, 1.5);
                let spin_factor = (24.0 / period_hrs.max(1.0)).sqrt().clamp(0.2, 3.0);
                let core_mass_frac = (diff.core_radius_au / (diff.mantle_radius_au.max(1e-5)))
                    .powi(3)
                    .clamp(0.05, 0.60);

                let b_gauss = (0.35
                    * (b_mass_solar / EARTH_MASS_SOLAR).sqrt().max(0.1)
                    * core_mass_frac.sqrt()
                    * spin_factor
                    * temp_factor.powf(0.33))
                .clamp(0.0, 5.0);

                diff.magnetic_field_gauss = b_gauss;
                magnetic_field_gauss = b_gauss as f32;
            } else {
                diff.magnetic_field_gauss = 0.0;
            }

            diff.core_temp_k = (diff.core_temp_k - (1.5e-3 * dt_yr)).max(300.0);
        }
    }

    if star_is_ignited && r < 3.5 {
        if let Some(ref mut vol) = opt_vol {
            if magnetic_field_gauss < 0.12 {
                let unshielded_factor = (1.0 - (magnetic_field_gauss / 0.12)).clamp(0.0, 1.0);
                let strip_rate =
                    (0.00015 * (1.0 / (r * r)) * f64::from(unshielded_factor) * dt_yr) as f32;
                vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar - strip_rate).max(0.0);
                if vol.atmospheric_pressure_bar <= 0.001 {
                    comp.gas_frac = (comp.gas_frac - f64::from(strip_rate * 0.0001)).max(0.0);
                }
            }
        }
    }

    if star_lum > 500.0 && (25.0..=65.0).contains(&r) && comp.ice_frac > 0.25 {
        if let Some(ref mut vol) = opt_vol {
            vol.ocean_coverage_frac = (vol.ocean_coverage_frac + 0.02 * dt_yr as f32).min(0.85);
            vol.atmospheric_pressure_bar =
                (vol.atmospheric_pressure_bar + 0.03 * dt_yr as f32).min(1.2);
        }
    }

    update_body_climate_and_biosphere(
        commands,
        body_ent,
        b_mass_solar,
        b_body,
        comp,
        p_temp,
        opt_vol.as_deref_mut(),
        opt_climate,
        opt_bio,
        r,
        star_lum,
        star_temp,
        star_r,
        shockwave_r,
        magnetic_field_gauss,
        dt_yr,
        elapsed_years,
    );

    if shockwave_r > r && r < 2.7 && comp.ice_frac > 0.001 {
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
}
