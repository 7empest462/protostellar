//! Systems for General Relativistic Post-Newtonian evolution and Gravitational Wave inspiral.

use std::f64::consts::PI;

use bevy::math::DVec3;
use bevy::prelude::*;

use super::kerr::*;
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
    pub host_vel: DVec3,
    pub host_radius: f64,
    pub is_host_remnant: bool,
    pub kerr_spin: Option<(f64, DVec3)>,
    pub rel_pos: DVec3,
    pub rel_vel: DVec3,
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub orbital_period_years: f64,
}

/// Primary host properties tuple: `(Entity, pos, vel, mass, radius, is_remnant, kerr_spin)`.
pub type PrimaryHostContextTuple = (Entity, DVec3, DVec3, f64, f64, bool, Option<(f64, DVec3)>);

/// Resolves the orbital host context for a body relative to the central star or parent planet.
pub fn resolve_relativistic_host_context(
    body_pos: DVec3,
    body_vel: DVec3,
    body_mass: f64,
    opt_sat: Option<&SatelliteOf>,
    star_opt: Option<PrimaryHostContextTuple>,
) -> Option<RelativisticHostContext> {
    if let Some(sat) = opt_sat {
        let r_orbit = sat.semi_major_axis_au.max(1e-6);
        let period = sat.orbital_period_years.max(1e-6);
        let host_mass = (4.0 * PI * PI * r_orbit.powi(3) / (G_ASTRO * period * period)).max(1e-8);
        return Some(RelativisticHostContext {
            host_entity: sat.parent,
            host_mass,
            host_pos: body_pos,
            host_vel: DVec3::ZERO,
            host_radius: 0.001,
            is_host_remnant: false,
            kerr_spin: None,
            rel_pos: DVec3::new(r_orbit, 0.0, 0.0),
            rel_vel: body_vel,
            semi_major_axis: r_orbit,
            eccentricity: 0.0,
            orbital_period_years: period,
        });
    }

    let (star_entity, star_pos, star_vel, star_mass, star_rad, is_host_remnant, kerr_spin) =
        star_opt?;
    if star_mass <= 1e-6 {
        return None;
    }

    let rel_pos = body_pos - star_pos;
    let rel_vel = body_vel - star_vel;
    let r = rel_pos.length();
    let max_r = if star_mass > 100.0 { 15_000.0 } else { 1_000.0 };
    if r < 1e-6 || r > max_r {
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
        host_vel: star_vel,
        host_radius: star_rad,
        is_host_remnant,
        kerr_spin,
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
    opt_kerr_spin: Option<f64>,
) -> Option<(f64, f64, BodyType)> {
    let m_total = host_mass + companion_mass;
    let r_isco = opt_kerr_spin.map_or_else(
        || calculate_isco_radius(m_total),
        |spin| calculate_kerr_isco_radius(m_total, spin),
    );
    let r_contact = companion_rad + 0.0001;
    let threshold = r_isco.max(r_contact);

    if sep_au > threshold {
        return None;
    }

    // Gravitational wave mass loss scales with symmetric mass ratio eta = (m1 * m2) / (m1 + m2)^2:
    // ~5% of total rest mass for equal-mass binaries (eta = 0.25 -> 0.20 * eta = 0.05),
    // and ~20% of the infalling companion mass for extreme-mass-ratio inspirals into an SMBH.
    let radiated_gw_mass = 0.20 * (host_mass * companion_mass) / m_total.max(1e-12);
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

    if let Ok(mut entity_cmd) = commands.get_entity(companion_entity) {
        entity_cmd.try_despawn();
    }

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
    is_satellite: bool,
    m_body: f64,
    dt_yr: f64,
    effective_dt_yr: f64,
    pos: &mut SimPosition,
    vel: &mut SimVelocity,
    rel_state: &mut RelativisticState,
) {
    if is_satellite {
        return;
    }

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

    let (z_tot, _z_grav, beta) =
        calculate_relativistic_redshift(m_host, ctx.rel_pos.length(), ctx.rel_vel.length());

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
    rel_state.total_redshift_z = z_tot;
    rel_state.beta_v_over_c = beta;

    let mut current_rel_pos = ctx.rel_pos;
    let mut current_rel_vel = ctx.rel_vel;

    // 3. Apply 1PN Relativistic Precession (Rigid Apsidal Rotation of both r_rel and v_rel around h_hat)
    // Rotating both position and velocity in the orbital plane preserves |r|, |v|, and |r x v|
    // to machine precision while advancing the argument of periapsis by `d_theta`.
    if rel_config.enable_1pn_precession && accumulated_delta.abs() > 1e-12 {
        let h_vec = current_rel_pos.cross(current_rel_vel);
        let h_len = h_vec.length();
        if h_len > 1e-8 {
            let h_hat = h_vec / h_len;
            let max_angle = 0.05f64;
            let d_theta = accumulated_delta.clamp(-max_angle, max_angle);
            let (sin_t, cos_t) = d_theta.sin_cos();

            let r_perp = h_hat.cross(current_rel_pos);
            let r_rot = current_rel_pos * cos_t + r_perp * sin_t;

            let v_perp = h_hat.cross(current_rel_vel);
            let v_rot = current_rel_vel * cos_t + v_perp * sin_t;

            if r_rot.is_finite() && v_rot.is_finite() {
                current_rel_pos = r_rot;
                current_rel_vel = v_rot;
                pos.0 = ctx.host_pos + current_rel_pos;
                vel.0 = ctx.host_vel + current_rel_vel;
            }
        }
    }

    // 3b. Apply Kerr Metric Lense-Thirring Precession & Frame Dragging
    if let Some((a_star, spin_axis)) = ctx.kerr_spin {
        let (lt_rate_rad_yr, lt_rate_arcsec_cy) =
            calculate_lense_thirring_precession_rate(m_host, a_star, a, e);
        rel_state.lense_thirring_rate_arcsec_century = lt_rate_arcsec_cy;
        rel_state.kerr_spin_param = a_star;

        if rel_config.enable_lense_thirring && lt_rate_rad_yr.abs() > 1e-15 {
            let delta_omega = (lt_rate_rad_yr * dt_yr).clamp(-0.05, 0.05);
            let s_hat = if spin_axis.length_squared() > 1e-12 {
                spin_axis.normalize()
            } else {
                DVec3::Y
            };
            let (sin_o, cos_o) = delta_omega.sin_cos();

            let r_cross_s = s_hat.cross(current_rel_pos);
            let r_dot_s = s_hat.dot(current_rel_pos);
            let r_rot =
                current_rel_pos * cos_o + r_cross_s * sin_o + s_hat * (r_dot_s * (1.0 - cos_o));

            let v_cross_s = s_hat.cross(current_rel_vel);
            let v_dot_s = s_hat.dot(current_rel_vel);
            let v_rot =
                current_rel_vel * cos_o + v_cross_s * sin_o + s_hat * (v_dot_s * (1.0 - cos_o));

            if r_rot.is_finite() && v_rot.is_finite() {
                current_rel_pos = r_rot;
                current_rel_vel = v_rot;
                pos.0 = ctx.host_pos + current_rel_pos;
                vel.0 = ctx.host_vel + current_rel_vel;
            }
        }
    }

    // 4. Apply Peters Secular Gravitational Wave Orbital Decay
    if rel_config.enable_inspiral_decay
        && (body_type.is_remnant() || (ctx.is_host_remnant && a < 0.25))
    {
        apply_gravitational_wave_orbital_decay(
            current_rel_pos,
            current_rel_vel,
            vel,
            da_dt,
            effective_dt_yr,
            a,
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Host state mutation and event resolution across compact merger outcome"
)]
fn apply_relativistic_coalescence_outcome(
    commands: &mut Commands,
    host_entity: Entity,
    companion_entity: Entity,
    companion_name: &str,
    remnant_mass: f64,
    new_radius: f64,
    remnant_type: BodyType,
    star_query: &mut Query<
        (
            Entity,
            &SimPosition,
            &SimVelocity,
            &mut Mass,
            &mut Radius,
            Option<&mut CelestialBody>,
            Option<&mut KerrBlackHole>,
        ),
        With<CentralStar>,
    >,
    player_state: &mut Option<ResMut<PlayerInteractionState>>,
) {
    let mut merged_name = format!("Merged {remnant_type:?} ({remnant_mass:.2} M☉)");
    if let Some((_, _, _, mut s_mass, mut s_rad, mut opt_s_body, mut opt_kerr)) = star_query
        .iter_mut()
        .find(|(star_e, _, _, _, _, _, _)| *star_e == host_entity)
    {
        s_mass.0 = remnant_mass;
        s_rad.0 = new_radius;
        if let Some(ref mut b) = opt_s_body {
            b.body_type = remnant_type;
            crate::simulation::accretion::naming::update_merged_body_name(
                &mut b.name,
                companion_name,
                remnant_type,
                remnant_mass,
            );
            merged_name.clone_from(&b.name);
        }
        if let Some(ref mut kerr) = opt_kerr {
            kerr.recalculate(remnant_mass);
        }
    }

    if let Ok(mut host_cmd) = commands.get_entity(host_entity) {
        host_cmd.insert((
            Mass(remnant_mass),
            Radius(new_radius),
            CelestialBody {
                name: merged_name,
                body_type: remnant_type,
            },
        ));
    }

    if let Some(ref mut ps) = player_state {
        if ps.selected_entity == Some(companion_entity) || ps.selected_entity.is_none() {
            ps.selected_entity = Some(host_entity);
        }
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
    _config: Res<SimulationConfig>,
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
            Option<&mut KerrBlackHole>,
        ),
        With<CentralStar>,
    >,
    mut bodies_query: Query<
        (
            Entity,
            &mut SimPosition,
            &mut SimVelocity,
            &Mass,
            &Radius,
            &CelestialBody,
            Option<&SatelliteOf>,
            Option<&mut RelativisticState>,
        ),
        Without<CentralStar>,
    >,
    mut player_state: Option<ResMut<PlayerInteractionState>>,
) {
    if !rel_config.enable_1pn_precession
        && !rel_config.enable_gw_radiation
        && !rel_config.enable_lense_thirring
    {
        return;
    }

    let dt_yr = sim_time.current_dt_yr;
    let effective_dt_yr = dt_yr * rel_config.time_scale.max(1.0);
    let star_opt = star_query
        .iter()
        .next()
        .map(|(e, p, v, m, r, b, opt_kerr)| {
            let is_rem = b.as_ref().is_some_and(|body| body.body_type.is_remnant());
            let kerr = opt_kerr.as_ref().map(|k| (k.spin_param, k.spin_axis));
            (e, p.0, v.0, m.0, r.0, is_rem, kerr)
        });

    for (entity, mut pos, mut vel, mass, rad, body, opt_sat, mut opt_rel) in bodies_query.iter_mut()
    {
        let Some(ctx) = resolve_relativistic_host_context(pos.0, vel.0, mass.0, opt_sat, star_opt)
        else {
            continue;
        };

        let mut rel_state = if let Some(ref r) = opt_rel {
            **r
        } else {
            RelativisticState::default()
        };

        let m_host = ctx.host_mass;
        let m_body = mass.0;

        apply_relativistic_kinematics_and_telemetry(
            &rel_config,
            &ctx,
            body.body_type,
            opt_sat.is_some(),
            m_body,
            dt_yr,
            effective_dt_yr,
            &mut pos,
            &mut vel,
            &mut rel_state,
        );

        let is_both_remnant = ctx.is_host_remnant && body.body_type.is_remnant();
        if is_both_remnant && opt_sat.is_none() {
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
                ctx.kerr_spin.map(|(a, _)| a),
            ) {
                apply_relativistic_coalescence_outcome(
                    &mut commands,
                    ctx.host_entity,
                    entity,
                    &body.name,
                    remnant_mass,
                    new_radius,
                    remnant_type,
                    &mut star_query,
                    &mut player_state,
                );

                // Companion was despawned: do NOT insert components on despawned entity
                continue;
            }
        }

        if let Some(ref mut r) = opt_rel {
            **r = rel_state;
        }
    }
}
