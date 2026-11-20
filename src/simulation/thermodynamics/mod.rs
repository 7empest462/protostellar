//! Thermodynamics, Hayashi Track Protostellar Evolution, Core Dynamos, Planetary Climate, Biosphere Genesis, and Far-Future Stellar Metamorphosis.

mod climate;
pub mod stellar_evolution;
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
) -> smallvec::SmallVec<[StarThermoInfo; 4]> {
    let mut stars_info = smallvec::SmallVec::new();
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
                _is_central: is_central_star,
            });
            continue;
        }

        let was_black_hole = body.body_type == BodyType::BlackHole;

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

        if !was_black_hole && body.body_type == BodyType::BlackHole {
            attach_black_hole_remnant_components(commands, entity, mass.0);
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
    reload_gen: Option<Res<crate::simulation::scenarios::ScenarioReloadGeneration>>,
    mut last_reload_gen: Local<u64>,
    mut ignition_events: MessageWriter<StarIgnitionEvent>,
    mut engulfment_events: MessageWriter<PlanetaryEngulfmentEvent>,
    mut supernova_events: MessageWriter<SupernovaEvent>,
    mut prev_steam_cache: Local<rustc_hash::FxHashMap<Entity, (f32, f32)>>,
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
            &mut Mass,
            Option<&mut Radius>,
            &SimPosition,
            &mut SimVelocity,
            &mut Temperature,
            &mut Composition,
            &CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&SpinState>,
            (
                Option<&mut VolatileInventory>,
                Option<&mut PlanetaryClimate>,
                Option<&mut BiosphereState>,
            ),
            Option<&SatelliteOf>,
            Has<crate::simulation::terraforming::TerraformingAtmosphere>,
            Option<&crate::simulation::atmosphere_escape::types::AtmosphericEscapeState>,
        ),
        (
            Without<CentralStar>,
            Without<IgnitionState>,
            Without<crate::simulation::probes::SpaceProbe>,
        ),
    >,
) {
    if (!config.enable_thermodynamics || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let dt_yr = if sim_time.current_dt_yr > 0.0 {
        sim_time.current_dt_yr
    } else {
        config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)
    };
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

    let current_gen = reload_gen.as_deref().map_or(0, |g| g.0);
    if current_gen != *last_reload_gen {
        *last_reload_gen = current_gen;
        prev_steam_cache.clear();
    }

    // 2. Update Disk Body Temperatures & Planetary Thermal Processing
    if !stars_info.is_empty() {
        update_disk_bodies_thermodynamics(
            &mut commands,
            &mut bodies_query,
            &stars_info,
            &mut prev_steam_cache,
            dt_yr,
            sim_time.elapsed_years,
            &mut engulfment_events,
        );
        prev_steam_cache.retain(|&ent, _| bodies_query.contains(ent));
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
            &mut Mass,
            Option<&mut Radius>,
            &SimPosition,
            &mut SimVelocity,
            &mut Temperature,
            &mut Composition,
            &CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&SpinState>,
            (
                Option<&mut VolatileInventory>,
                Option<&mut PlanetaryClimate>,
                Option<&mut BiosphereState>,
            ),
            Option<&SatelliteOf>,
            Has<crate::simulation::terraforming::TerraformingAtmosphere>,
            Option<&crate::simulation::atmosphere_escape::types::AtmosphericEscapeState>,
        ),
        (
            Without<CentralStar>,
            Without<IgnitionState>,
            Without<crate::simulation::probes::SpaceProbe>,
        ),
    >,
    stars_info: &[StarThermoInfo],
    prev_steam_cache: &mut rustc_hash::FxHashMap<Entity, (f32, f32)>,
    dt_yr: f64,
    elapsed_years: f64,
    engulfment_events: &mut MessageWriter<PlanetaryEngulfmentEvent>,
) {
    for (
        body_ent,
        mut b_mass,
        mut opt_rad,
        pos,
        mut vel,
        mut p_temp,
        mut comp,
        b_body,
        mut opt_diff,
        opt_spin,
        (mut opt_vol, mut opt_climate, mut opt_bio),
        opt_sat,
        has_terraforming,
        opt_escape_state,
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
        let prev_gas = comp.gas_frac;
        let prev_ice = comp.ice_frac;
        let b_radius_au = opt_rad.as_deref().map_or(0.0, |r| r.0);
        let current_total_bar = opt_vol
            .as_deref()
            .map_or(0.0, |v| v.atmospheric_pressure_bar);
        // Invalidate cached steam if pressure changed externally (epoch scrub or escape stripping)
        let prev_steam_bar = prev_steam_cache
            .get(&body_ent)
            .copied()
            .filter(|&(_, last_total)| (current_total_bar - last_total).abs() <= 0.05)
            .map_or(0.0, |(steam, _)| steam);
        let active_escape_rate =
            opt_escape_state.map_or(0.0, |e| e.total_loss_rate_m_earth_per_myr);

        let new_steam_bar = update_body_thermodynamics(
            commands,
            body_ent,
            &mut b_mass,
            b_radius_au,
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
            has_terraforming,
            prev_steam_bar,
            active_escape_rate,
            dt_yr,
            elapsed_years,
            engulfment_events,
        );
        let post_total_bar = opt_vol
            .as_deref()
            .map_or(0.0, |v| v.atmospheric_pressure_bar);
        if new_steam_bar > 0.0 || prev_steam_cache.contains_key(&body_ent) {
            prev_steam_cache.insert(body_ent, (new_steam_bar, post_total_bar));
        }

        let comp_shifted =
            (comp.gas_frac - prev_gas).abs() > 1e-5 || (comp.ice_frac - prev_ice).abs() > 1e-5;
        if comp_shifted
            && !b_body.body_type.is_star_or_remnant()
            && !matches!(
                b_body.body_type,
                BodyType::GasGiant | BodyType::IceGiant | BodyType::BrownDwarf
            )
            && let Some(ref mut rad) = opt_rad
        {
            let new_r = crate::simulation::accretion::impact_regimes::radius_from_mass_density(
                b_mass.0,
                comp.average_density(),
            );
            rad.0 = new_r;
            if let Some(ref mut diff) = opt_diff {
                diff.recalculate(b_mass.0, new_r, &comp);
            }
        }
    }
}

fn apply_stellar_envelope_drag_and_engulfment(
    commands: &mut Commands,
    body_ent: Entity,
    b_mass_solar: f64,
    r: f64,
    vel: &mut SimVelocity,
    b_body: &CelestialBody,
    star_temp: f64,
    star_r: f64,
    dt_yr: f64,
    engulfment_events: &mut MessageWriter<PlanetaryEngulfmentEvent>,
) -> bool {
    if b_body.body_type.is_star_or_remnant() || star_temp <= 100.0 || r >= star_r * 2.0 {
        return false;
    }

    let drag_coeff = if r < star_r { 0.25 } else { 0.08 };
    vel.0 *= 1.0 - (drag_coeff * dt_yr).min(0.8);

    let is_deep_engulfment = if star_r > 0.15 {
        r < 0.18 || r < star_r * 0.20
    } else {
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
        return true;
    }
    false
}

fn update_planetary_core_dynamo(
    opt_diff: &mut Option<Mut<'_, InternalDifferentiation>>,
    b_mass_solar: f64,
    period_hrs: f64,
    dt_yr: f64,
) -> f32 {
    let mut magnetic_field_gauss = 0.0f32;
    if let Some(diff) = opt_diff
        && diff.is_differentiated
    {
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
    magnetic_field_gauss
}

fn apply_outer_giant_thaw(
    opt_vol: &mut Option<Mut<'_, VolatileInventory>>,
    comp: &Composition,
    r: f64,
    star_lum: f64,
    dt_yr: f64,
) {
    if star_lum > 500.0
        && (25.0..=65.0).contains(&r)
        && comp.ice_frac > 0.25
        && let Some(vol) = opt_vol
    {
        vol.delivered_water_m_earth =
            (vol.delivered_water_m_earth + 0.000_01 * dt_yr).min(0.0005 * 0.85);
        vol.ocean_coverage_frac = (vol.ocean_coverage_frac + 0.02 * dt_yr as f32).min(0.85);
        vol.atmospheric_pressure_bar =
            (vol.atmospheric_pressure_bar + 0.03 * dt_yr as f32).min(1.2);
    }
}

fn apply_shockwave_ice_sublimation(
    mass: &mut Mass,
    comp: &mut Composition,
    r: f64,
    shockwave_r: f64,
    dt_yr: f64,
) {
    if shockwave_r > 0.05
        && shockwave_r < 25.0
        && (r - shockwave_r).abs() < 2.5
        && r < 2.7
        && comp.ice_frac > 0.001
    {
        let old_mass = mass.0.max(1e-12);
        let m_metal = old_mass * comp.metal_frac;
        let m_silicate = old_mass * comp.silicate_frac;
        let mut m_ice = old_mass * comp.ice_frac;
        let m_organics = old_mass * comp.organics_frac;
        let m_gas = old_mass * comp.gas_frac;

        let frac_loss = (0.15 * dt_yr).clamp(0.0, 1.0);
        let dm_lost = (m_ice * frac_loss).min((old_mass - 1e-12).max(0.0));
        if dm_lost > 0.0 {
            m_ice = (m_ice - dm_lost).max(0.0);
            let new_total = (m_metal + m_silicate + m_ice + m_organics + m_gas).max(1e-12);
            mass.0 = new_total;
            comp.metal_frac = m_metal / new_total;
            comp.silicate_frac = m_silicate / new_total;
            comp.ice_frac = m_ice / new_total;
            comp.organics_frac = m_organics / new_total;
            comp.gas_frac = m_gas / new_total;
            *comp = comp.normalized();
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Coordinates single-body engulfment, dynamo, outer thaw, climate, and shockwave checks"
)]
fn update_body_thermodynamics(
    commands: &mut Commands,
    body_ent: Entity,
    b_mass: &mut Mass,
    b_radius_au: f64,
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
    has_terraforming: bool,
    prev_steam_bar: f32,
    active_escape_rate: f64,
    dt_yr: f64,
    elapsed_years: f64,
    engulfment_events: &mut MessageWriter<PlanetaryEngulfmentEvent>,
) -> f32 {
    if b_body.body_type.is_star_or_remnant() {
        return 0.0;
    }

    let b_mass_solar = b_mass.0;
    let r = pos.length().max(1e-4);
    let period_hrs = opt_spin.map_or(24.0, |s| s.rotation_period_hours);

    if apply_stellar_envelope_drag_and_engulfment(
        commands,
        body_ent,
        b_mass_solar,
        r,
        vel,
        b_body,
        star_temp,
        star_r,
        dt_yr,
        engulfment_events,
    ) {
        return 0.0;
    }

    let magnetic_field_gauss =
        update_planetary_core_dynamo(opt_diff, b_mass_solar, period_hrs, dt_yr);

    if has_terraforming {
        let mut surface_temp = p_temp.0;
        if let Some(climate) = opt_climate {
            let eq = compute_equilibrium_temp_k(
                r,
                star_lum,
                star_temp,
                star_r,
                climate.albedo.clamp(0.05, 0.90),
            );
            climate.equilibrium_temperature_k = eq as f32;
            surface_temp = f64::from(climate.surface_temperature_k).max(20.0);
        }
        let (ocean_frac, atm_pressure) = opt_vol.as_deref().map_or((0.0, 0.0), |v| {
            (v.ocean_coverage_frac, v.atmospheric_pressure_bar)
        });
        climate::update_body_biosphere(
            commands,
            body_ent,
            b_mass_solar,
            surface_temp,
            star_temp,
            ocean_frac,
            magnetic_field_gauss,
            atm_pressure,
            opt_bio,
            dt_yr,
            elapsed_years,
        );
        0.0
    } else {
        apply_outer_giant_thaw(opt_vol, comp, r, star_lum, dt_yr);

        let steam_bar = update_body_climate_and_biosphere(
            commands,
            body_ent,
            b_mass_solar,
            b_radius_au,
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
            prev_steam_bar,
            active_escape_rate,
            dt_yr,
            elapsed_years,
        );

        apply_shockwave_ice_sublimation(b_mass, comp, r, shockwave_r, dt_yr);
        steam_bar
    }
}
