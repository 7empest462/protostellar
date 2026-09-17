//! Systems for General Relativistic Post-Newtonian evolution and Gravitational Wave inspiral.

use std::f64::consts::PI;

use bevy::math::DVec3;
use bevy::prelude::*;

use super::post_newtonian::*;
use super::types::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::{CHANDRASEKHAR_LIMIT_SOLAR, G_ASTRO, TOV_LIMIT_SOLAR};

/// Kinematic context for an orbiting body relative to its gravitational primary.
#[derive(Debug, Clone, Copy)]
pub struct RelativisticHostContext {
    pub host_entity: Entity,
    pub host_mass: f64,
    pub host_pos: DVec3,
    pub host_radius: f64,
    pub rel_pos: DVec3,
    pub rel_vel: DVec3,
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub orbital_period_years: f64,
}

/// Resolves the orbital host context for a body relative to the central star or parent planet.
pub fn resolve_relativistic_host_context(
    body_pos: DVec3,
    body_vel: DVec3,
    body_mass: f64,
    opt_sat: Option<&SatelliteOf>,
    star_opt: Option<(Entity, DVec3, DVec3, f64, f64)>,
) -> Option<RelativisticHostContext> {
    if let Some(sat) = opt_sat {
        let r_orbit = sat.semi_major_axis_au.max(1e-6);
        let period = sat.orbital_period_years.max(1e-6);
        let host_mass = (4.0 * PI * PI * r_orbit.powi(3) / (G_ASTRO * period * period)).max(1e-8);
        return Some(RelativisticHostContext {
            host_entity: sat.parent,
            host_mass,
            host_pos: body_pos,
            host_radius: 0.001,
            rel_pos: DVec3::new(r_orbit, 0.0, 0.0),
            rel_vel: body_vel,
            semi_major_axis: r_orbit,
            eccentricity: 0.0,
            orbital_period_years: period,
        });
    }

    let (star_entity, star_pos, star_vel, star_mass, star_rad) = star_opt?;
    if star_mass <= 1e-6 {
        return None;
    }

    let rel_pos = body_pos - star_pos;
    let rel_vel = body_vel - star_vel;
    let r = rel_pos.length();
    if r < 1e-6 || r > 50.0 {
        return None;
    }

    let mu = G_ASTRO * (star_mass + body_mass);
    let v_sq = rel_vel.length_squared();
    let specific_energy = 0.5 * v_sq - mu / r;

    if specific_energy >= -1e-9 {
        return None;
    }

    let a = -mu / (2.0 * specific_energy);
    if a <= 1e-6 || !a.is_finite() {
        return None;
    }

    let h_vec = rel_pos.cross(rel_vel);
    if h_vec.length_squared() < 1e-12 {
        return None;
    }
    let e_vec = rel_vel.cross(h_vec) / mu - rel_pos / r;
    let e = e_vec.length().clamp(0.0, 0.999);
    let period = 2.0 * PI * (a * a * a / mu).sqrt();

    Some(RelativisticHostContext {
        host_entity: star_entity,
        host_mass: star_mass,
        host_pos: star_pos,
        host_radius: star_rad,
        rel_pos,
        rel_vel,
        semi_major_axis: a,
        eccentricity: e,
        orbital_period_years: period,
    })
}

/// Applies Peters (1964) gravitational wave secular semi-major axis shrinkage and orbital decay to velocity.
fn apply_gravitational_wave_orbital_decay(
    rel_pos: DVec3,
    rel_vel: DVec3,
    vel: &mut SimVelocity,
    da_dt_au_yr: f64,
    effective_dt_yr: f64,
    semi_major_axis: f64,
) {
    let delta_a = da_dt_au_yr * effective_dt_yr;
    if delta_a.abs() < 1e-18 || semi_major_axis <= 1e-6 {
        return;
    }

    // Tangential drag coefficient: delta_v / v ~ 0.5 * delta_a / a
    let fractional_decay = (0.5 * delta_a / semi_major_axis).clamp(-0.25, 0.0);
    let r = rel_pos.length().max(1e-6);
    let r_hat = rel_pos / r;
    let v_radial = rel_vel.dot(r_hat) * r_hat;
    let v_tangential = rel_vel - v_radial;

    vel.0 += v_tangential * fractional_decay;
}

/// Evaluates coalescence between two compact objects and executes a merger if within ISCO or contact.
#[allow(
    clippy::too_many_arguments,
    reason = "Compact object merger evaluation is inherently complex, requiring many inputs to determine merger conditions"
)]
fn evaluate_compact_coalescence(
    commands: &mut Commands,
    merger_events: &mut MessageWriter<GravitationalWaveMergerEvent>,
    primary_entity: Entity,
    companion_entity: Entity,
    host_mass: f64,
    companion_mass: f64,
    companion_rad: f64,
    companion_type: BodyType,
    pos: DVec3,
    sep_au: f64,
) -> Option<(f64, f64, BodyType)> {
    let m_total = host_mass + companion_mass;
    let r_isco = calculate_isco_radius(m_total);
    let r_contact = companion_rad + 0.0001;
    let threshold = r_isco.max(r_contact);

    if sep_au > threshold {
        return None;
    }

    // ~5% of rest mass radiated as gravitational waves (Peters & Thorne 1974, Abbott et al. 2016)
    let radiated_gw_mass = m_total * 0.05;
    let remnant_mass = m_total - radiated_gw_mass;

    let remnant_type = if companion_type == BodyType::BlackHole || remnant_mass > TOV_LIMIT_SOLAR {
        BodyType::BlackHole
    } else if companion_type == BodyType::Magnetar {
        BodyType::Magnetar
    } else if remnant_mass > CHANDRASEKHAR_LIMIT_SOLAR {
        BodyType::NeutronStar
    } else {
        BodyType::WhiteDwarf
    };

    let new_radius = if remnant_type == BodyType::BlackHole {
        calculate_schwarzschild_radius(remnant_mass).max(1e-6)
    } else {
        companion_rad * (remnant_mass / m_total.max(1e-6)).cbrt()
    };

    let (_, peak_f_hz) = calculate_gw_frequency(m_total, threshold);

    commands.entity(companion_entity).despawn();

    merger_events.write(GravitationalWaveMergerEvent {
        primary_entity,
        companion_entity,
        merged_entity: primary_entity,
        position: pos,
        remnant_mass_solar: remnant_mass,
        radiated_gw_mass_solar: radiated_gw_mass,
        peak_gw_frequency_hz: peak_f_hz,
        remnant_type,
    });

    Some((remnant_mass, new_radius, remnant_type))
}

/// Evaluates 1PN precession, GW radiation, and applies velocity kicks and orbital decay.
#[allow(
    clippy::too_many_arguments,
    reason = "Kinematics calculation requires host context and configuration parameters"
)]
fn apply_relativistic_kinematics_and_telemetry(
    rel_config: &RelativityConfig,
    ctx: &RelativisticHostContext,
    body_type: BodyType,
    m_body: f64,
    dt_yr: f64,
    effective_dt_yr: f64,
    vel: &mut SimVelocity,
    rel_state: &mut RelativisticState,
) {
    let a = ctx.semi_major_axis;
    let e = ctx.eccentricity;
    let m_host = ctx.host_mass;

    // 1. Post-Newtonian (1PN) Precession Calculations
    let (rate_rad_yr, rate_arcsec_cy) = calculate_1pn_precession_rate(m_host, a, e);
    let advance_rad = calculate_1pn_precession_per_orbit(m_host, a, e);
    let accumulated_delta = rate_rad_yr * dt_yr;

    // 2. Peters (1964) Gravitational Wave Radiation Calculations
    let power_watts = calculate_peters_gw_power(m_host, m_body, a, e);
    let da_dt = calculate_peters_da_dt(m_host, m_body, a, e);
    let tau_merge = calculate_gw_coalescence_time(m_host, m_body, a, e);
    let (_, f_gw_hz) = calculate_gw_frequency(m_host + m_body, a);

    let mu = (m_host * m_body) / (m_host + m_body).max(1e-9);
    let chirp_mass = mu.powf(0.6) * (m_host + m_body).powf(0.4);
    let strain = calculate_gw_strain(chirp_mass, f_gw_hz, 10.0);

    // Update Cached State
    rel_state.precession_rate_arcsec_century = rate_arcsec_cy;
    rel_state.precession_advance_per_orbit_rad = advance_rad;
    rel_state.accumulated_precession_rad += accumulated_delta;
    rel_state.gw_luminosity_watts = power_watts;
    rel_state.gw_strain = strain;
    rel_state.gw_frequency_hz = f_gw_hz;
    rel_state.inspiral_timescale_yr = tau_merge;
    rel_state.orbital_decay_rate_au_per_myr = da_dt * 1e6;
    rel_state.semi_major_axis_au = a;
    rel_state.eccentricity = e;

    // 3. Apply 1PN Relativistic Acceleration to Velocity
    if rel_config.enable_1pn_precession {
        let a_1pn = calculate_1pn_acceleration(ctx.rel_pos, ctx.rel_vel, m_host);
        if a_1pn.is_finite() {
            vel.0 += a_1pn * dt_yr;
        }
    }

    // 4. Apply Peters Secular Gravitational Wave Orbital Decay
    if rel_config.enable_inspiral_decay && (body_type.is_remnant() || a < 0.25) {
        apply_gravitational_wave_orbital_decay(
            ctx.rel_pos,
            ctx.rel_vel,
            vel,
            da_dt,
            effective_dt_yr,
            a,
        );
    }
}

/// Updates 1PN relativistic periastron precession and Peters (1964) gravitational wave dissipation.
#[allow(
    clippy::type_complexity,
    reason = "Relativistic Evolution Query Scheduling"
)]
pub fn update_relativity_evolution(
    rel_config: Res<RelativityConfig>,
    sim_time: Res<SimTime>,
    config: Res<SimulationConfig>,
    mut merger_events: MessageWriter<GravitationalWaveMergerEvent>,
    mut commands: Commands,
    mut star_query: Query<
        (
            Entity,
            &SimPosition,
            &SimVelocity,
            &mut Mass,
            &mut Radius,
            Option<&mut CelestialBody>,
        ),
        With<CentralStar>,
    >,
    mut bodies_query: Query<
        (
            Entity,
            &SimPosition,
            &mut SimVelocity,
            &Mass,
            &Radius,
            &CelestialBody,
            Option<&SatelliteOf>,
            Option<&mut RelativisticState>,
        ),
        Without<CentralStar>,
    >,
) {
    if !rel_config.enable_1pn_precession && !rel_config.enable_gw_radiation {
        return;
    }

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr);
    let effective_dt_yr = dt_yr * rel_config.time_scale.max(1.0);
    let star_opt = star_query
        .iter()
        .next()
        .map(|(e, p, v, m, r, _)| (e, p.0, v.0, m.0, r.0));

    for (entity, pos, mut vel, mass, rad, body, opt_sat, opt_rel) in bodies_query.iter_mut() {
        let Some(ctx) = resolve_relativistic_host_context(pos.0, vel.0, mass.0, opt_sat, star_opt)
        else {
            continue;
        };

        let mut rel_state = if let Some(r) = opt_rel {
            *r
        } else {
            let init = RelativisticState::default();
            commands.entity(entity).insert(init);
            init
        };

        let m_host = ctx.host_mass;
        let m_body = mass.0;

        apply_relativistic_kinematics_and_telemetry(
            &rel_config,
            &ctx,
            body.body_type,
            m_body,
            dt_yr,
            effective_dt_yr,
            &mut vel,
            &mut rel_state,
        );
        let sep_au = ctx.rel_pos.length();
        let companion_type = body.body_type;
        if let Some((remnant_mass, new_radius, remnant_type)) = evaluate_compact_coalescence(
            &mut commands,
            &mut merger_events,
            ctx.host_entity,
            entity,
            m_host,
            m_body,
            rad.0,
            companion_type,
            pos.0,
            sep_au,
        ) {
            // Apply remnant mass and type to primary host
            if let Some((_, _, _, mut s_mass, mut s_rad, mut opt_s_body)) = star_query
                .iter_mut()
                .find(|(star_e, _, _, _, _, _)| *star_e == ctx.host_entity)
            {
                s_mass.0 = remnant_mass;
                s_rad.0 = new_radius;
                if let Some(ref mut b) = opt_s_body {
                    b.body_type = remnant_type;
                    b.name = format!("Merged {remnant_type:?} ({remnant_mass:.2} M☉)");
                }
            }

            commands.entity(ctx.host_entity).insert((
                Mass(remnant_mass),
                Radius(new_radius),
                CelestialBody {
                    name: format!("Merged {remnant_type:?} ({remnant_mass:.2} M☉)"),
                    body_type: remnant_type,
                },
            ));

            // Companion was despawned: do NOT insert components on despawned entity
            continue;
        }

        commands.entity(entity).insert(rel_state);
    }
}
