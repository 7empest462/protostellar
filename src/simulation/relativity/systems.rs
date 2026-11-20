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
    pub host_body_type: BodyType,
    pub is_host_remnant: bool,
    pub kerr_spin: Option<(f64, DVec3)>,
    pub rel_pos: DVec3,
    pub rel_vel: DVec3,
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub orbital_period_years: f64,
}

/// Primary host properties tuple: `(Entity, pos, vel, mass, radius, body_type, kerr_spin)`.
pub type PrimaryHostContextTuple = (
    Entity,
    DVec3,
    DVec3,
    f64,
    f64,
    BodyType,
    Option<(f64, DVec3)>,
);

/// Resolves the orbital host context for a body relative to the central star or parent planet.
pub fn resolve_relativistic_host_context(
    body_pos: DVec3,
    body_vel: DVec3,
    body_mass: f64,
    opt_sat: Option<&SatelliteOf>,
    star_opt: Option<PrimaryHostContextTuple>,
    parent_opt: Option<PrimaryHostContextTuple>,
) -> Option<RelativisticHostContext> {
    if let Some(sat) = opt_sat {
        let (p_ent, p_pos, p_vel, p_mass, p_rad, p_type, kerr_spin) = parent_opt?;
        if p_mass <= 1e-12 {
            return None;
        }
        let a = sat.semi_major_axis_au.max(1e-6);
        let period = sat.orbital_period_years.max(1e-6);
        let rel_pos = body_pos - p_pos;
        let rel_vel = body_vel - p_vel;
        return Some(RelativisticHostContext {
            host_entity: p_ent,
            host_mass: p_mass,
            host_pos: p_pos,
            host_vel: p_vel,
            host_radius: p_rad,
            host_body_type: p_type,
            is_host_remnant: p_type.is_remnant(),
            kerr_spin,
            rel_pos,
            rel_vel,
            semi_major_axis: a,
            eccentricity: 0.0,
            orbital_period_years: period,
        });
    }

    let (star_entity, star_pos, star_vel, star_mass, star_rad, star_type, kerr_spin) = star_opt?;
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
        host_body_type: star_type,
        is_host_remnant: star_type.is_remnant(),
        kerr_spin,
        rel_pos,
        rel_vel,
        semi_major_axis: a,
        eccentricity: e,
        orbital_period_years: period,
    })
}

/// Applies coupled Peters (1964) gravitational wave shrinkage (`da/dt`) and circularization (`de/dt`).
#[allow(
    clippy::too_many_arguments,
    reason = "Coupled Peters (1964) GW orbital decay requires host/companion masses and elements"
)]
fn apply_gravitational_wave_orbital_decay(
    rel_pos: DVec3,
    rel_vel: DVec3,
    vel: &mut SimVelocity,
    m_host: f64,
    m_body: f64,
    da_dt_au_yr: f64,
    de_dt_per_yr: f64,
    effective_dt_yr: f64,
    semi_major_axis: f64,
    eccentricity: f64,
) {
    let delta_a = da_dt_au_yr * effective_dt_yr;
    let delta_e = de_dt_per_yr * effective_dt_yr;
    if delta_a.abs() < 1e-18 || semi_major_axis <= 1e-6 {
        return;
    }

    let mu = G_ASTRO * (m_host + m_body).max(1e-9);
    let e = eccentricity.clamp(0.0, 0.999);
    let one_minus_e2 = (1.0 - e * e).max(1e-4);

    let r = rel_pos.length().max(1e-6);
    let r_hat = rel_pos / r;
    let v_rad_mag = rel_vel.dot(r_hat);
    let v_radial = r_hat * v_rad_mag;
    let v_tangential = rel_vel - v_radial;

    // Specific angular momentum relation h^2 = mu * a * (1 - e^2):
    // dh/h = 0.5 * (da/a) - (e * de) / (1 - e^2) <= 0 for gravitational wave emission.
    let alpha_h =
        (0.5 * (delta_a / semi_major_axis) - (e * delta_e) / one_minus_e2).clamp(-0.25, 0.0);
    let delta_v_tan = v_tangential * alpha_h;

    // Specific energy balance dE = v_tan . dv_tan + v_rad * dv_rad = mu / (2 a^2) * da
    let target_de = (mu / (2.0 * semi_major_axis * semi_major_axis)) * delta_a;
    let rem_de = target_de - v_tangential.dot(delta_v_tan);
    let v_scale2 = rel_vel
        .length_squared()
        .max(mu / semi_major_axis)
        .max(1e-12);
    let delta_v_rad_mag = if v_rad_mag.abs() > 1e-12 {
        (rem_de * v_rad_mag / (v_rad_mag * v_rad_mag + 0.05 * v_scale2))
            .clamp(-0.25 * v_rad_mag.abs(), 0.25 * v_rad_mag.abs())
    } else {
        0.0
    };

    vel.0 += delta_v_tan + r_hat * delta_v_rad_mag;
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
    host_rad: f64,
    host_type: BodyType,
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
    let r_contact = host_rad + companion_rad;
    let threshold = r_isco.max(r_contact);

    if sep_au > threshold {
        return None;
    }

    // Gravitational wave mass loss scales with symmetric mass ratio eta = (m1 * m2) / (m1 + m2)^2:
    // ~5% of total rest mass for equal-mass binaries (eta = 0.25 -> 0.20 * eta = 0.05),
    // and ~20% of the infalling companion mass for extreme-mass-ratio inspirals into an SMBH.
    let radiated_gw_mass = 0.20 * (host_mass * companion_mass) / m_total.max(1e-12);
    let remnant_mass = m_total - radiated_gw_mass;

    let remnant_type = if host_type == BodyType::BlackHole
        || companion_type == BodyType::BlackHole
        || remnant_mass > TOV_LIMIT_SOLAR
    {
        BodyType::BlackHole
    } else if host_type == BodyType::Magnetar || companion_type == BodyType::Magnetar {
        BodyType::Magnetar
    } else if remnant_mass > CHANDRASEKHAR_LIMIT_SOLAR
        || matches!(host_type, BodyType::NeutronStar | BodyType::Pulsar)
        || matches!(companion_type, BodyType::NeutronStar | BodyType::Pulsar)
    {
        BodyType::NeutronStar
    } else {
        BodyType::WhiteDwarf
    };

    let new_radius = if remnant_type == BodyType::BlackHole {
        calculate_schwarzschild_radius(remnant_mass).max(1e-6)
    } else {
        companion_rad.max(host_rad) * (remnant_mass / m_total.max(1e-6)).cbrt()
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

#[inline]
fn rotate_around_axis(v: DVec3, axis: DVec3, angle: f64) -> DVec3 {
    let (s, c) = angle.sin_cos();
    v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Applies coupled Lense-Thirring nodal and periastron frame-dragging precession"
)]
fn apply_lense_thirring_rotation(
    rel_config: &RelativityConfig,
    ctx: &RelativisticHostContext,
    effective_dt_yr: f64,
    current_rel_pos: &mut DVec3,
    current_rel_vel: &mut DVec3,
    pos: &mut SimPosition,
    vel: &mut SimVelocity,
    rel_state: &mut RelativisticState,
) {
    let Some((a_star, spin_axis)) = ctx.kerr_spin else {
        return;
    };
    let (lt_rate_rad_yr, lt_rate_arcsec_cy) = calculate_lense_thirring_precession_rate(
        ctx.host_mass,
        a_star,
        ctx.semi_major_axis,
        ctx.eccentricity,
    );
    rel_state.lense_thirring_rate_arcsec_century = lt_rate_arcsec_cy;
    rel_state.kerr_spin_param = a_star;

    if !rel_config.enable_lense_thirring || lt_rate_rad_yr.abs() <= 1e-15 {
        return;
    }

    let s_hat = if spin_axis.length_squared() > 1e-12 {
        spin_axis.normalize()
    } else {
        -DVec3::Y
    };
    let h_pre = current_rel_pos.cross(*current_rel_vel);
    let h_pre_len = h_pre.length();
    let cos_i = if h_pre_len > 1e-8 {
        s_hat.dot(h_pre / h_pre_len).clamp(-1.0, 1.0)
    } else {
        1.0
    };
    let raw_node = lt_rate_rad_yr * effective_dt_yr;
    let raw_peri = -3.0 * cos_i * raw_node;
    // Coupled scalar clamp preserves the analytical 1 : -3*cos(i) Lense-Thirring ratio.
    let clamp_scale = (0.05 / raw_node.abs().max(raw_peri.abs()).max(1e-12)).min(1.0);
    let delta_omega_node = raw_node * clamp_scale;
    let delta_omega_peri = raw_peri * clamp_scale;

    let r_node = rotate_around_axis(*current_rel_pos, s_hat, delta_omega_node);
    let v_node = rotate_around_axis(*current_rel_vel, s_hat, delta_omega_node);

    if r_node.is_finite() && v_node.is_finite() {
        *current_rel_pos = r_node;
        *current_rel_vel = v_node;
        let h_vec = current_rel_pos.cross(*current_rel_vel);
        let h_len = h_vec.length();
        if h_len > 1e-8 {
            let h_hat = h_vec / h_len;
            let r_lt = rotate_around_axis(*current_rel_pos, h_hat, delta_omega_peri);
            let v_lt = rotate_around_axis(*current_rel_vel, h_hat, delta_omega_peri);
            if r_lt.is_finite() && v_lt.is_finite() {
                *current_rel_pos = r_lt;
                *current_rel_vel = v_lt;
            }
        }
        pos.0 = ctx.host_pos + *current_rel_pos;
        vel.0 = ctx.host_vel + *current_rel_vel;
    }
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
    _dt_yr: f64,
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
    let m_total = m_host + m_body;

    // 1. Post-Newtonian (1PN) Precession Calculations (depends on total binary mass M = m1 + m2)
    let (rate_rad_yr, rate_arcsec_cy) = calculate_1pn_precession_rate(m_total, a, e);
    let advance_rad = calculate_1pn_precession_per_orbit(m_total, a, e);
    let accumulated_delta = rate_rad_yr * effective_dt_yr;

    // 2. Peters (1964) Gravitational Wave Radiation Calculations
    let (power_watts, da_dt, de_dt, tau_merge, f_gw_hz, strain) = if rel_config.enable_gw_radiation
    {
        let power = calculate_peters_gw_power(m_host, m_body, a, e);
        let da = calculate_peters_da_dt(m_host, m_body, a, e);
        let de = calculate_peters_de_dt(m_host, m_body, a, e);
        let tau = calculate_gw_coalescence_time(m_host, m_body, a, e);
        let (_, f_gw) = calculate_gw_frequency(m_total, a);
        let mu = (m_host * m_body) / m_total.max(1e-9);
        let chirp_mass = mu.powf(0.6) * m_total.powf(0.4);
        let h_strain = calculate_gw_strain(chirp_mass, f_gw, 10.0);
        (power, da, de, tau, f_gw, h_strain)
    } else {
        (0.0, 0.0, 0.0, f64::INFINITY, 0.0, 0.0)
    };

    let (z_tot, _z_grav, beta) =
        calculate_relativistic_redshift(m_host, ctx.rel_pos.length(), ctx.rel_vel.length());

    rel_state.precession_rate_arcsec_century = rate_arcsec_cy;
    rel_state.precession_advance_per_orbit_rad = advance_rad;
    if rel_config.enable_1pn_precession {
        rel_state.accumulated_precession_rad += accumulated_delta;
    }
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
    if rel_config.enable_1pn_precession && accumulated_delta.abs() > 1e-12 {
        let h_vec = current_rel_pos.cross(current_rel_vel);
        let h_len = h_vec.length();
        if h_len > 1e-8 {
            let h_hat = h_vec / h_len;
            let d_theta = accumulated_delta.clamp(-0.05, 0.05);
            let r_rot = rotate_around_axis(current_rel_pos, h_hat, d_theta);
            let v_rot = rotate_around_axis(current_rel_vel, h_hat, d_theta);
            if r_rot.is_finite() && v_rot.is_finite() {
                current_rel_pos = r_rot;
                current_rel_vel = v_rot;
                pos.0 = ctx.host_pos + current_rel_pos;
                vel.0 = ctx.host_vel + current_rel_vel;
            }
        }
    }

    // 3b. Apply Kerr Metric Lense-Thirring Nodal + Periastron Precession
    apply_lense_thirring_rotation(
        rel_config,
        ctx,
        effective_dt_yr,
        &mut current_rel_pos,
        &mut current_rel_vel,
        pos,
        vel,
        rel_state,
    );

    // 4. Apply Peters Secular Gravitational Wave Orbital Decay (coupled da/dt + de/dt)
    if rel_config.enable_gw_radiation
        && rel_config.enable_inspiral_decay
        && (body_type.is_remnant() || (ctx.is_host_remnant && a < 0.25))
    {
        apply_gravitational_wave_orbital_decay(
            current_rel_pos,
            current_rel_vel,
            vel,
            m_host,
            m_body,
            da_dt,
            de_dt,
            effective_dt_yr,
            a,
            e,
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
    companion_pos: DVec3,
    companion_vel: DVec3,
    companion_mass: f64,
    remnant_mass: f64,
    new_radius: f64,
    remnant_type: BodyType,
    star_query: &mut Query<
        (
            Entity,
            &mut SimPosition,
            &mut SimVelocity,
            &mut Mass,
            &mut Radius,
            Option<&mut CelestialBody>,
            Option<&mut KerrBlackHole>,
            Option<&mut RelativisticState>,
        ),
        With<CentralStar>,
    >,
    player_state: &mut Option<ResMut<PlayerInteractionState>>,
) {
    let mut merged_name = format!("Merged {remnant_type:?} ({remnant_mass:.2} M☉)");
    let mut updated_body_in_place = false;
    if let Some((_, mut s_pos, mut s_vel, mut s_mass, mut s_rad, mut opt_s_body, mut opt_kerr, _)) =
        star_query
            .iter_mut()
            .find(|(star_e, ..)| *star_e == host_entity)
    {
        let total_m = (s_mass.0 + companion_mass).max(1e-12);
        s_pos.0 = (s_pos.0 * s_mass.0 + companion_pos * companion_mass) / total_m;
        s_vel.0 = (s_vel.0 * s_mass.0 + companion_vel * companion_mass) / total_m;
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
            updated_body_in_place = true;
        }
        if let Some(ref mut kerr) = opt_kerr {
            kerr.recalculate(remnant_mass);
        }
    }

    if !updated_body_in_place && let Ok(mut host_cmd) = commands.get_entity(host_entity) {
        host_cmd.insert((
            Mass(remnant_mass),
            Radius(new_radius),
            CelestialBody {
                name: merged_name,
                body_type: remnant_type,
            },
        ));
    }

    if let Some(ps) = player_state
        && (ps.selected_entity == Some(companion_entity) || ps.selected_entity.is_none())
    {
        ps.selected_entity = Some(host_entity);
    }
}

type RelativisticStarQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut SimPosition,
        &'static mut SimVelocity,
        &'static mut Mass,
        &'static mut Radius,
        Option<&'static mut CelestialBody>,
        Option<&'static mut KerrBlackHole>,
        Option<&'static mut RelativisticState>,
    ),
    With<CentralStar>,
>;

#[allow(
    clippy::too_many_arguments,
    reason = "Compact merger helper resolves coalescence thresholds and mutates host remnant state"
)]
fn try_execute_compact_merger(
    commands: &mut Commands,
    merger_events: &mut MessageWriter<GravitationalWaveMergerEvent>,
    ctx: &RelativisticHostContext,
    entity: Entity,
    body: &CelestialBody,
    pos: DVec3,
    vel: DVec3,
    m_body: f64,
    rad_au: f64,
    star_query: &mut RelativisticStarQuery,
    player_state: &mut Option<ResMut<PlayerInteractionState>>,
) -> bool {
    if !ctx.is_host_remnant || !body.body_type.is_remnant() {
        return false;
    }
    let Some((remnant_mass, new_radius, remnant_type)) = evaluate_compact_coalescence(
        commands,
        merger_events,
        ctx.host_entity,
        entity,
        ctx.host_mass,
        ctx.host_radius,
        ctx.host_body_type,
        m_body,
        rad_au,
        body.body_type,
        pos,
        ctx.rel_pos.length(),
        ctx.kerr_spin.map(|(a, _)| a),
    ) else {
        return false;
    };

    apply_relativistic_coalescence_outcome(
        commands,
        ctx.host_entity,
        entity,
        &body.name,
        pos,
        vel,
        m_body,
        remnant_mass,
        new_radius,
        remnant_type,
        star_query,
        player_state,
    );
    true
}

/// Updates 1PN relativistic periastron precession and Peters (1964) gravitational wave dissipation.
#[allow(
    clippy::type_complexity,
    reason = "Relativistic Evolution Query Scheduling"
)]
pub fn update_relativity_evolution(
    rel_config: Res<RelativityConfig>,
    sim_time: Res<SimTime>,
    time_warp: Option<Res<crate::simulation::resources::TimeWarp>>,
    _config: Res<SimulationConfig>,
    mut merger_events: MessageWriter<GravitationalWaveMergerEvent>,
    mut commands: Commands,
    mut star_query: RelativisticStarQuery,
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
    mut parent_map: Local<rustc_hash::FxHashMap<Entity, PrimaryHostContextTuple>>,
) {
    if !rel_config.enable_1pn_precession
        && !rel_config.enable_gw_radiation
        && !rel_config.enable_lense_thirring
    {
        return;
    }

    if time_warp
        .as_deref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once)
    {
        return;
    }

    let dt_yr = sim_time.current_dt_yr;
    let effective_dt_yr = dt_yr * rel_config.time_scale.max(0.0);
    let star_opt = star_query
        .iter()
        .next()
        .map(|(e, p, v, m, r, b, opt_kerr, _)| {
            let body_type = b
                .as_ref()
                .map_or(BodyType::MainSequenceStar, |body| body.body_type);
            let kerr = opt_kerr.as_ref().map(|k| (k.spin_param, k.spin_axis));
            (e, p.0, v.0, m.0, r.0, body_type, kerr)
        });

    parent_map.clear();
    if let Some(s) = star_opt {
        parent_map.insert(s.0, s);
    }
    for (e, pos, vel, mass, rad, body, _, _) in bodies_query.iter() {
        parent_map.insert(e, (e, pos.0, vel.0, mass.0, rad.0, body.body_type, None));
    }

    let mut central_telemetry: Option<(f64, RelativisticState)> = None;

    for (entity, mut pos, mut vel, mass, rad, body, opt_sat, mut opt_rel) in bodies_query.iter_mut()
    {
        let parent_opt = opt_sat.and_then(|sat| parent_map.get(&sat.parent).copied());
        let Some(ctx) =
            resolve_relativistic_host_context(pos.0, vel.0, mass.0, opt_sat, star_opt, parent_opt)
        else {
            continue;
        };

        let mut rel_state = opt_rel.as_deref().copied().unwrap_or_default();
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

        if opt_sat.is_none()
            && try_execute_compact_merger(
                &mut commands,
                &mut merger_events,
                &ctx,
                entity,
                body,
                pos.0,
                vel.0,
                m_body,
                rad.0,
                &mut star_query,
                &mut player_state,
            )
        {
            continue;
        }

        if let Some(ref mut r) = opt_rel {
            **r = rel_state;
        }
        if opt_sat.is_none()
            && star_opt.is_some_and(|s| s.0 == ctx.host_entity)
            && (body.body_type.is_star_or_remnant() || m_body >= 0.01 * ctx.host_mass)
            && central_telemetry
                .as_ref()
                .is_none_or(|(best_m, _)| m_body > *best_m)
        {
            central_telemetry = Some((m_body, rel_state));
        }
    }

    if let Some((_, comp_rel)) = central_telemetry
        && let Some((_, _, _, _, _, _, _, Some(mut star_rel))) = star_query.iter_mut().next()
    {
        *star_rel = comp_rel;
    }
}
