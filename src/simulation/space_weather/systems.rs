//! Simulation systems for stellar coronal mass ejections and planetary auroral oval dynamics.

use bevy::prelude::*;

use crate::simulation::atmosphere_escape::physics::{
    calculate_magnetopause_radius, calculate_solar_wind_pressure,
};
use crate::simulation::components::*;
use crate::simulation::events::types::{CosmicEventManager, CosmicEventPhase, CosmicEventType};
use crate::simulation::neutron_stars::types::MagnetarCrustStress;
use crate::simulation::resources::{SimTime, TimeWarp};
use crate::simulation::space_weather::physics::evaluate_auroral_storm_state;
use crate::simulation::space_weather::types::*;

/// Contextual space weather forcing from the central star, active hazards, and magnetar bursts.
#[derive(Clone, Copy)]
struct SpaceWeatherContext {
    star_flare: Option<StellarFlareState>,
    prev_cme_front_au: f32,
    star_mass: f64,
    star_pos: Vec3,
    is_young_protostar: bool,
    ignition_blast_radius_au: f32,
    superflare_active: bool,
    dt_yr: f32,
    real_dt_secs: f32,
}

fn attach_stellar_activity_clocks(
    commands: &mut Commands,
    unattached: &Query<Entity, (With<StellarFlareState>, Without<StellarActivityClock>)>,
) {
    for star_ent in unattached.iter() {
        commands
            .entity(star_ent)
            .try_insert(StellarActivityClock::default());
    }
}

fn stellar_activity_multiplier(body_type: BodyType) -> f32 {
    match body_type {
        BodyType::Protostar | BodyType::TTauriStar | BodyType::HerbigAeBeStar => 2.2,
        BodyType::RedDwarf | BodyType::BrownDwarf => 1.6,
        BodyType::Magnetar | BodyType::Pulsar => 2.4,
        _ => 1.0,
    }
}

/// Advances stellar coronal flare decay, propagates outward CME shock fronts, and emits periodic natural flares/CMEs.
pub fn update_stellar_flares_system(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    real_time: Option<Res<Time>>,
    unattached_clocks: Query<Entity, (With<StellarFlareState>, Without<StellarActivityClock>)>,
    mut stars_query: Query<
        (
            Entity,
            &mut StellarFlareState,
            Option<&mut StellarActivityClock>,
            &CelestialBody,
        ),
        With<CentralStar>,
    >,
) {
    attach_stellar_activity_clocks(&mut commands, &unattached_clocks);

    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt_yr = sim_time.current_dt_yr as f32;
    let real_dt = real_time.map_or(0.0, |t| t.delta_secs()).min(0.1);
    if dt_yr <= 0.0 && real_dt <= 0.0 {
        return;
    }

    for (_ent, mut flare, opt_clock, body) in stars_query.iter_mut() {
        // 1. Advance active CME plasma shock front (keeps both simulated AU/day speed and real-time visibility)
        if flare.cme_active {
            if flare.cme_front_radius_au > 50.0 {
                flare.cme_active = false;
                flare.cme_front_radius_au = 0.0;
            } else {
                let sim_step_au = flare.cme_speed_au_day * dt_yr.max(0.0) * 365.25;
                let realtime_step_au = real_dt * flare.cme_speed_au_day.clamp(0.15, 0.90) * 0.55;
                flare.cme_front_radius_au += sim_step_au.max(realtime_step_au);
            }
        }

        // 2. Smooth decay of coronal flare flash
        if flare.current_flare_intensity > 0.0 {
            let decay = dt_yr.max(0.0) * 15.0 + real_dt * 0.12;
            flare.current_flare_intensity = (flare.current_flare_intensity - decay).max(0.0);
        }

        // 3. Natural cyclic flares and CME eruptions based on star flare frequency and evolutionary class
        if let Some(mut clock) = opt_clock
            && flare.flare_frequency > 0.0
            && !flare.cme_active
            && flare.current_flare_intensity < 0.2
        {
            let activity = stellar_activity_multiplier(body.body_type) * flare.flare_frequency;
            clock.sim_years_accum += dt_yr.max(0.0) * activity;
            clock.realtime_secs_accum += real_dt * (activity / 1.5).clamp(0.35, 3.0);

            if clock.sim_years_accum >= 0.65 || clock.realtime_secs_accum >= 75.0 {
                clock.sim_years_accum = 0.0;
                clock.realtime_secs_accum = 0.0;
                clock.eruption_count = clock.eruption_count.wrapping_add(1);
                let is_major_cme = clock.eruption_count.is_multiple_of(3);
                flare.current_flare_intensity = if is_major_cme { 5.5 } else { 2.8 };
                flare.flare_decay_timer_years = 0.08;
                flare.cme_active = true;
                flare.cme_front_radius_au = 0.04;
                flare.cme_speed_au_day = if is_major_cme { 0.36 } else { 0.24 };
                flare.cme_density_multiplier = if is_major_cme { 18.0 } else { 8.0 };
            }
        }
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Component attachment queries check star and unattached planet states"
)]
fn ensure_space_weather_components_attached(
    commands: &mut Commands,
    unattached_stars: &Query<Entity, (With<CentralStar>, Without<StellarFlareState>)>,
    unattached_planets: &Query<
        (
            Entity,
            &CelestialBody,
            Option<&InternalDifferentiation>,
            Option<&ElectromagneticFieldState>,
            Option<&VolatileInventory>,
        ),
        Without<AuroralOvalState>,
    >,
) {
    for star_ent in unattached_stars.iter() {
        commands.entity(star_ent).try_insert((
            StellarFlareState::default(),
            StellarActivityClock::default(),
        ));
    }

    for (p_ent, body, opt_diff, opt_em, opt_vol) in unattached_planets.iter() {
        if body.body_type.is_star_or_remnant() {
            continue;
        }
        let b_gauss = opt_diff
            .map_or(0.0, |d| d.magnetic_field_gauss)
            .max(opt_em.map_or(0.0, |e| e.magnetic_field_gauss));
        let has_atmo = opt_vol.map_or(
            body.body_type == BodyType::GasGiant || body.body_type == BodyType::IceGiant,
            |v| v.atmospheric_pressure_bar >= 0.001,
        );
        if b_gauss > 0.01 && has_atmo {
            commands.entity(p_ent).try_insert((
                AuroralOvalState::default(),
                AuroralStormExcitation::default(),
            ));
        }
    }
}

fn compute_moon_torus_boost(
    planet_pos: Vec3,
    is_gas_or_ice_giant: bool,
    moons: &[(Vec3, f32)],
) -> f32 {
    if !is_gas_or_ice_giant {
        return 0.0;
    }
    let mut best = 0.0f32;
    for &(moon_pos, activity) in moons {
        let sep_au = planet_pos.distance(moon_pos);
        if sep_au > 1e-5 && sep_au < 0.025 {
            let coupling = (1.0 - sep_au / 0.025) * activity;
            best = best.max(coupling);
        }
    }
    best.clamp(0.0, 0.85)
}

fn evaluate_substorm_pulse(clock_secs: f32, is_young_protostar: bool) -> f32 {
    // Begin calm (0.0) on fresh load/startup; pulse intermittently every ~75s for ~16s
    if clock_secs < 12.0 {
        return 0.0;
    }
    let cycle = 75.0f32;
    let phase = (clock_secs % cycle) / cycle;
    let pulse = (phase * std::f32::consts::TAU).cos();
    if pulse > 0.72 {
        let frac = ((pulse - 0.72) / 0.28).clamp(0.0, 1.0);
        let base_substorm = 0.55 * frac;
        if is_young_protostar {
            base_substorm * 1.35
        } else {
            base_substorm
        }
    } else {
        0.0
    }
}

fn clear_inactive_aurora(aurora: &mut AuroralOvalState, excitation: &mut AuroralStormExcitation) {
    aurora.magnetopause_standoff_rp = 1.0;
    aurora.oval_colatitude_rad = 0.0;
    aurora.oval_width_rad = 0.0;
    aurora.auroral_intensity = 0.0;
    aurora.geomagnetic_kp_index = 0.0;
    aurora.storm_level = GeomagneticStormLevel::Quiet;
    excitation.excitation = 0.0;
}

fn accumulate_stellar_storm_drivers(
    planet_ent: Entity,
    dist_f: f32,
    ctx: SpaceWeatherContext,
    active_cme_hits: &mut rustc_hash::FxHashSet<Entity>,
    cme_events: &mut MessageWriter<CmeShockwaveEvent>,
) -> (f32, f32) {
    let mut ram_multiplier = 1.0f32;
    let mut boost = 0.0f32;
    let Some(flare) = ctx.star_flare else {
        return (ram_multiplier, boost);
    };

    if flare.cme_active {
        let shock_thickness = 0.35;
        let sim_step_au = flare.cme_speed_au_day * ctx.dt_yr.max(0.0) * 365.25;
        let prev_front_au = if ctx.prev_cme_front_au > 0.0 {
            ctx.prev_cme_front_au
        } else {
            (flare.cme_front_radius_au - sim_step_au).max(0.0)
        };
        if dist_f >= prev_front_au - shock_thickness
            && dist_f <= flare.cme_front_radius_au + shock_thickness * 0.5
        {
            ram_multiplier = flare.cme_density_multiplier;
            boost = boost.max((flare.cme_density_multiplier / 7.5).clamp(0.95, 3.6));
            if active_cme_hits.insert(planet_ent) {
                cme_events.write(CmeShockwaveEvent {
                    target: planet_ent,
                    orbital_radius_au: dist_f,
                    density_multiplier: flare.cme_density_multiplier,
                    flare_intensity: flare.current_flare_intensity,
                });
            }
        } else {
            active_cme_hits.remove(&planet_ent);
        }
    }

    if flare.current_flare_intensity > 0.5 {
        let prompt_sep =
            (flare.current_flare_intensity / (5.5 * dist_f.max(0.35))).clamp(0.15, 1.35);
        boost = boost.max(prompt_sep);
    }

    (ram_multiplier, boost)
}

fn accumulate_hazard_and_substorm_boost(
    planet_world_pos: Vec3,
    dist_f: f32,
    ctx: SpaceWeatherContext,
    excitation: &mut AuroralStormExcitation,
    magnetar_bursts: &[(Vec3, f32)],
) -> f32 {
    let mut boost = 0.0f32;
    let blast_r = ctx.ignition_blast_radius_au;
    if blast_r > 0.05 && (blast_r - excitation.last_blast_radius_au).abs() > 1e-4 {
        excitation.last_blast_radius_au = blast_r;
        if (dist_f - blast_r).abs() <= 0.35 {
            boost = boost.max(2.35);
        }
    }

    if ctx.superflare_active {
        boost = boost.max(3.8);
    }

    for &(remnant_pos, burst_strength) in magnetar_bursts {
        let sep = planet_world_pos.distance(remnant_pos).max(0.1);
        if sep < 15.0 {
            boost = boost.max((burst_strength / (sep * 1.5)).clamp(0.3, 2.8));
        }
    }

    boost.max(evaluate_substorm_pulse(
        excitation.substorm_clock_secs,
        ctx.is_young_protostar,
    ))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Evaluates stellar wind, CME, flare, blast, moon-torus, and substorm drivers"
)]
fn update_single_planet_aurora(
    commands: &mut Commands,
    planet_ent: Entity,
    p_pos: &SimPosition,
    p_rad: &Radius,
    opt_diff: Option<&InternalDifferentiation>,
    opt_em: Option<&ElectromagneticFieldState>,
    opt_vol: Option<&VolatileInventory>,
    is_gas_or_ice_giant: bool,
    aurora: &mut AuroralOvalState,
    opt_excitation: Option<&mut AuroralStormExcitation>,
    ctx: SpaceWeatherContext,
    moon_torus_boost: f32,
    magnetar_bursts: &[(Vec3, f32)],
    active_cme_hits: &mut rustc_hash::FxHashSet<Entity>,
    cme_events: &mut MessageWriter<CmeShockwaveEvent>,
) {
    let missing_excitation = opt_excitation.is_none();
    let mut fallback_exc = AuroralStormExcitation {
        excitation: ((aurora.geomagnetic_kp_index - 2.0) / 2.2).clamp(0.0, 4.0),
        substorm_clock_secs: 0.0,
        last_blast_radius_au: 0.0,
    };
    let excitation = opt_excitation.unwrap_or(&mut fallback_exc);

    let b_gauss = opt_diff
        .map_or(0.0, |d| d.magnetic_field_gauss)
        .max(opt_em.map_or(0.0, |e| e.magnetic_field_gauss));
    let has_atmo = opt_vol.map_or(is_gas_or_ice_giant, |v| v.atmospheric_pressure_bar >= 0.001);
    if b_gauss <= 0.01 || !has_atmo {
        clear_inactive_aurora(aurora, excitation);
        return;
    }

    let planet_world_pos = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
    let dist_f = (planet_world_pos - ctx.star_pos).length().max(0.01);
    let (mut p_ram_base, _) = calculate_solar_wind_pressure(f64::from(dist_f), ctx.star_mass);
    if ctx.is_young_protostar {
        p_ram_base *= 1.35;
    }

    let decay = (-(ctx.dt_yr.max(0.0) * 180.0 + ctx.real_dt_secs.max(0.0) * 0.045)).exp();
    excitation.excitation *= decay;
    excitation.substorm_clock_secs += ctx.real_dt_secs.max(0.0) + ctx.dt_yr.max(0.0) * 320.0;

    let (ram_multiplier, stellar_boost) =
        accumulate_stellar_storm_drivers(planet_ent, dist_f, ctx, active_cme_hits, cme_events);
    let hazard_boost = accumulate_hazard_and_substorm_boost(
        planet_world_pos,
        dist_f,
        ctx,
        excitation,
        magnetar_bursts,
    );
    excitation.excitation = excitation
        .excitation
        .max(stellar_boost.max(hazard_boost))
        .clamp(0.0, 4.2);

    let effective_p_ram = p_ram_base * f64::from(ram_multiplier);
    let r_p = p_rad.0.max(1e-7);
    let r_mp = calculate_magnetopause_radius(r_p, b_gauss, effective_p_ram);
    let base_standoff_rp = (r_mp / r_p).clamp(1.05, 60.0) as f32;

    let (standoff_rp, colatitude, half_width, kp, intensity, level) = evaluate_auroral_storm_state(
        base_standoff_rp,
        ram_multiplier,
        b_gauss as f32,
        excitation.excitation,
        moon_torus_boost,
    );

    aurora.magnetopause_standoff_rp = standoff_rp;
    aurora.oval_colatitude_rad = colatitude;
    aurora.oval_width_rad = half_width;
    aurora.geomagnetic_kp_index = kp;
    aurora.storm_level = level;
    aurora.auroral_intensity = intensity;

    let visual_dt = ctx.real_dt_secs.max(ctx.dt_yr.max(0.0005));
    aurora.flutter_phase = (aurora.flutter_phase + visual_dt * (2.5 + kp * 1.2)) % 10000.0;

    if missing_excitation {
        commands.entity(planet_ent).try_insert(*excitation);
    }
}

fn extract_central_star_weather(
    star_data: Option<(
        &StellarFlareState,
        Option<&IgnitionState>,
        &Mass,
        &SimPosition,
        &CelestialBody,
    )>,
) -> (Option<StellarFlareState>, f64, Vec3, bool, f32) {
    match star_data {
        Some((f, opt_ign, m, p, body)) => (
            Some(*f),
            m.0,
            Vec3::new(p.x as f32, p.y as f32, p.z as f32),
            matches!(
                body.body_type,
                BodyType::Protostar | BodyType::TTauriStar | BodyType::HerbigAeBeStar
            ) && opt_ign.is_none_or(|i| !i.is_ignited),
            opt_ign.map_or(0.0, |i| i.shockwave_radius as f32),
        ),
        None => (None, 1.0, Vec3::ZERO, false, 0.0),
    }
}

fn sync_active_cme_hits_cache(
    star_flare: Option<StellarFlareState>,
    active_cme_hits: &mut (rustc_hash::FxHashSet<Entity>, f32),
) -> f32 {
    match star_flare {
        Some(f) if f.cme_active => {
            let prev_front = if f.cme_front_radius_au + 1e-4 < active_cme_hits.1 {
                active_cme_hits.0.clear();
                0.0
            } else {
                active_cme_hits.1
            };
            active_cme_hits.1 = f.cme_front_radius_au;
            prev_front
        }
        _ => {
            active_cme_hits.0.clear();
            active_cme_hits.1 = 0.0;
            0.0
        }
    }
}

fn fill_moon_plasma_sources<'a>(
    moons_iter: impl Iterator<
        Item = (
            &'a SimPosition,
            &'a CelestialBody,
            Option<&'a VolcanicState>,
        ),
    >,
    out: &mut Vec<(Vec3, f32)>,
) {
    out.clear();
    out.extend(
        moons_iter
            .filter(|(_, body, _)| body.body_type == BodyType::Moon)
            .map(|(pos, _, opt_volc)| {
                let volcanism = opt_volc.map_or(0.25, |v| {
                    0.25 + (v.active_plumes as f32 / 8.0).clamp(0.0, 0.75)
                });
                (
                    Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32),
                    volcanism,
                )
            }),
    );
}

fn fill_magnetar_burst_sources<'a>(
    remnants_iter: impl Iterator<Item = (&'a SimPosition, &'a MagnetarCrustStress)>,
    out: &mut Vec<(Vec3, f32)>,
) {
    out.clear();
    out.extend(
        remnants_iter
            .filter(|(_, stress)| stress.burst_active)
            .map(|(pos, stress)| {
                (
                    Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32),
                    1.5 + (stress.accumulated_magnetic_strain as f32 * 15.0).clamp(0.0, 2.0),
                )
            }),
    );
}

fn is_eligible_auroral_planet(body_type: BodyType) -> bool {
    !body_type.is_star_or_remnant()
        && !matches!(
            body_type,
            BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain | BodyType::Planetesimal
        )
}

/// Dynamically models solar wind - magnetosphere interaction, magnetopause standoff,
/// and dual-pole auroral oval geometry and radiance for magnetized worlds with atmospheres.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Space weather orchestrates star flares, hazards, moons, magnetars, and planetary ovals"
)]
pub fn update_planetary_auroral_ovals_system(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    real_time: Option<Res<Time>>,
    event_mgr: Option<Res<CosmicEventManager>>,
    mut active_cme_hits: Local<(rustc_hash::FxHashSet<Entity>, f32)>,
    mut source_scratch: Local<(Vec<(Vec3, f32)>, Vec<(Vec3, f32)>)>,
    mut cme_events: MessageWriter<CmeShockwaveEvent>,
    unattached_stars: Query<Entity, (With<CentralStar>, Without<StellarFlareState>)>,
    unattached_planets: Query<
        (
            Entity,
            &CelestialBody,
            Option<&InternalDifferentiation>,
            Option<&ElectromagneticFieldState>,
            Option<&VolatileInventory>,
        ),
        Without<AuroralOvalState>,
    >,
    stars_query: Query<
        (
            &StellarFlareState,
            Option<&IgnitionState>,
            &Mass,
            &SimPosition,
            &CelestialBody,
        ),
        With<CentralStar>,
    >,
    moons_query: Query<
        (&SimPosition, &CelestialBody, Option<&VolcanicState>),
        Without<CentralStar>,
    >,
    remnants_query: Query<(&SimPosition, &MagnetarCrustStress)>,
    mut planets_query: Query<(
        Entity,
        &SimPosition,
        &Radius,
        &CelestialBody,
        Option<&InternalDifferentiation>,
        Option<&ElectromagneticFieldState>,
        Option<&VolatileInventory>,
        &mut AuroralOvalState,
        Option<&mut AuroralStormExcitation>,
    )>,
) {
    ensure_space_weather_components_attached(&mut commands, &unattached_stars, &unattached_planets);

    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt_yr = sim_time.current_dt_yr as f32;
    let real_dt_secs = real_time.map_or(0.0, |t| t.delta_secs()).min(0.1);
    let (star_flare, star_mass, star_pos, is_young_protostar, ignition_blast_radius_au) =
        extract_central_star_weather(stars_query.iter().next());
    let prev_cme_front_au = sync_active_cme_hits_cache(star_flare, &mut active_cme_hits);

    let superflare_active = event_mgr.is_some_and(|mgr| {
        mgr.active_events.iter().any(|e| {
            e.event_type == CosmicEventType::SolarSuperflare && e.phase == CosmicEventPhase::Active
        })
    });
    let (moons, magnetar_bursts) = &mut *source_scratch;
    fill_moon_plasma_sources(moons_query.iter(), moons);
    fill_magnetar_burst_sources(remnants_query.iter(), magnetar_bursts);
    let ctx = SpaceWeatherContext {
        star_flare,
        prev_cme_front_au,
        star_mass,
        star_pos,
        is_young_protostar,
        ignition_blast_radius_au,
        superflare_active,
        dt_yr,
        real_dt_secs,
    };

    for (
        planet_ent,
        p_pos,
        p_rad,
        body,
        opt_diff,
        opt_em,
        opt_vol,
        mut aurora,
        mut opt_excitation,
    ) in planets_query.iter_mut()
    {
        if !is_eligible_auroral_planet(body.body_type) {
            continue;
        }
        let is_gas_or_ice_giant =
            body.body_type == BodyType::GasGiant || body.body_type == BodyType::IceGiant;
        let planet_pos = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
        let moon_torus_boost = compute_moon_torus_boost(planet_pos, is_gas_or_ice_giant, moons);
        update_single_planet_aurora(
            &mut commands,
            planet_ent,
            p_pos,
            p_rad,
            opt_diff,
            opt_em,
            opt_vol,
            is_gas_or_ice_giant,
            &mut aurora,
            opt_excitation.as_deref_mut(),
            ctx,
            moon_torus_boost,
            magnetar_bursts,
            &mut active_cme_hits.0,
            &mut cme_events,
        );
    }
}
