//! ECS systems for detecting hierarchical triples, evaluating Kozai-Lidov resonance,
//! applying secular orbital evolution, and coupling with tidal dissipation.

use bevy::math::DVec3;
use bevy::prelude::*;

use super::physics::*;
use super::types::*;
use crate::simulation::components::*;
use crate::simulation::probes::SpaceProbe;
use crate::simulation::resources::{SimTime, TimeWarp};
use crate::simulation::tides::TidalState;
use crate::utils::math::state_vectors_to_orbital_elements;

/// System that automatically scans the system for hierarchical triples (inner body + outer massive perturber)
/// and initializes or updates `KozaiLidovState` components.
#[allow(
    clippy::type_complexity,
    reason = "Hierarchical triple detection requires multiple spatial and orbital filters"
)]
pub fn detect_hierarchical_triples(
    mut commands: Commands,
    time_warp: Option<Res<TimeWarp>>,
    primary_query: Query<
        (Entity, &SimPosition, &SimVelocity, &Mass),
        (With<CentralStar>, With<CelestialBody>),
    >,
    all_bodies_query: Query<
        (Entity, &SimPosition, &SimVelocity, &Mass, &CelestialBody),
        (Without<CentralStar>, Without<SpaceProbe>),
    >,
    existing_state_query: Query<&KozaiLidovState>,
    mut candidates_buf: Local<Vec<(Entity, f64, f64, DVec3, DVec3)>>,
) {
    if time_warp
        .as_deref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once)
    {
        return;
    }

    let Some((_primary_ent, p_pos, p_vel, p_mass)) = primary_query.iter().next() else {
        return;
    };

    if p_mass.0 <= 1e-6 {
        return;
    }

    let has_uninitialized_inner = all_bodies_query.iter().any(|(ent, _, _, _, body)| {
        !body.body_type.is_star_or_remnant()
            && existing_state_query
                .get(ent)
                .map_or(true, |s| s.perturber_entity.is_none())
    });
    if !has_uninitialized_inner {
        return;
    }

    // Collect outer perturber candidates: must have substantial mass (> 1e-5 M_sun, ~3 M_earth)
    candidates_buf.clear();
    candidates_buf.extend(
        all_bodies_query
            .iter()
            .filter(|(_, _, _, mass, _)| mass.0 >= 1e-5)
            .map(|(ent, pos, vel, mass, _body)| {
                let dist = (pos.0 - p_pos.0).length();
                (ent, mass.0, dist, pos.0, vel.0)
            }),
    );

    if candidates_buf.is_empty() {
        return;
    }

    for (inner_ent, pos, vel, _mass, body) in all_bodies_query.iter() {
        if body.body_type.is_star_or_remnant()
            || existing_state_query
                .get(inner_ent)
                .is_ok_and(|s| s.perturber_entity.is_some())
        {
            continue;
        }

        let inner_dist = (pos.0 - p_pos.0).length();
        if inner_dist <= 1e-4 {
            continue;
        }

        // Find dominant outer perturber maximizing tidal quadrupole strength M_pert / r_pert^3
        // with r_pert >= 2.5 * r_inner (hierarchical triple separation criterion)
        let mut best_perturber = None;
        let mut max_quadrupole_strength = 0.0;

        for (cand_idx, &(cand_ent, cand_mass, cand_dist, cand_pos, cand_vel)) in
            candidates_buf.iter().enumerate()
        {
            if cand_ent == inner_ent || cand_dist < 2.5 * inner_dist {
                continue;
            }

            let strength = cand_mass / cand_dist.powi(3);
            if strength > max_quadrupole_strength {
                max_quadrupole_strength = strength;
                best_perturber =
                    Some((cand_idx, cand_ent, cand_dist, cand_pos, cand_vel, cand_mass));
            }
        }

        if let Some((_cand_idx, pert_ent, pert_dist, pert_pos, pert_vel, pert_mass)) =
            best_perturber
        {
            let pert_name = all_bodies_query
                .get(pert_ent)
                .map_or_else(|_| String::new(), |(_, _, _, _, b)| b.name.clone());
            let h_inner = (pos.0 - p_pos.0).cross(vel.0 - p_vel.0);
            let h_outer = (pert_pos - p_pos.0).cross(pert_vel - p_vel.0);
            let i_mut_rad = compute_mutual_inclination(h_inner, h_outer);
            let is_res = is_in_kozai_resonance(i_mut_rad);
            let e_max = compute_max_eccentricity(0.01, i_mut_rad);
            let q_min = compute_min_periastron(inner_dist, e_max);
            let tau_kl = compute_kozai_timescale(inner_dist, pert_dist, p_mass.0, pert_mass, 0.0);

            if let Ok(mut cmd) = commands.get_entity(inner_ent) {
                cmd.try_insert(KozaiLidovState {
                    perturber_entity: Some(pert_ent),
                    perturber_name: pert_name,
                    mutual_inclination_deg: i_mut_rad.to_degrees(),
                    critical_inclination_deg: super::types::KOZAI_CRITICAL_ANGLE_DEG,
                    is_in_resonance: is_res,
                    max_eccentricity_forecast: e_max,
                    min_periastron_au: q_min,
                    kozai_period_years: tau_kl,
                    gr_precession_ratio: 0.0,
                    is_gr_suppressed: false,
                    regime: if is_res {
                        KozaiRegime::Circulation
                    } else {
                        KozaiRegime::Inactive
                    },
                    cycle_phase: 0.0,
                });
            }
        }
    }
}

/// System that updates Kozai-Lidov secular states, calculates maximum eccentricity forecasts,
/// applies 1PN GR suppression, and checks for Roche disruption risk.
#[allow(
    clippy::type_complexity,
    reason = "Kozai-Lidov secular dynamics queries multiple celestial state components"
)]
pub fn update_kozai_lidov_evolution(
    config: Res<KozaiLidovConfig>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    primary_query: Query<
        (Entity, &SimPosition, &SimVelocity, &Mass, &Radius),
        (With<CentralStar>, With<CelestialBody>),
    >,
    mut bodies_params: ParamSet<(
        Query<
            (Entity, &SimPosition, &SimVelocity, &Mass, &CelestialBody),
            (Without<CentralStar>, Without<SpaceProbe>),
        >,
        Query<
            (
                Entity,
                &SimPosition,
                &mut SimVelocity,
                &Mass,
                &Radius,
                &CelestialBody,
                &mut KozaiLidovState,
                Option<&mut TidalState>,
            ),
            (Without<CentralStar>, Without<SpaceProbe>),
        >,
    )>,
    mut disruption_events: MessageWriter<KozaiDisruptionEvent>,
    mut perturber_snapshots_buf: Local<Vec<(Entity, String, DVec3, DVec3, f64)>>,
    mut perturber_index_buf: Local<rustc_hash::FxHashMap<Entity, usize>>,
) {
    if !config.enable_kozai_lidov || (time_warp.is_paused && !time_warp.step_once) {
        return;
    }

    let Some((primary_ent, p_pos, p_vel, p_mass, p_rad)) = primary_query.iter().next() else {
        return;
    };

    let base_dt_yr = if sim_time.current_dt_yr > 0.0 {
        sim_time.current_dt_yr
    } else {
        time_warp.multiplier * (1.0 / 365.25)
    };

    let need_names = bodies_params
        .p1()
        .iter()
        .any(|(_, _, _, _, _, _, state, _)| state.perturber_entity.is_none());

    perturber_snapshots_buf.clear();
    perturber_index_buf.clear();
    for (e, pos, vel, mass, body) in bodies_params.p0().iter() {
        let idx = perturber_snapshots_buf.len();
        let name = if need_names {
            body.name.clone()
        } else {
            String::new()
        };
        perturber_snapshots_buf.push((e, name, pos.0, vel.0, mass.0));
        perturber_index_buf.insert(e, idx);
    }

    let primary = PrimaryContext {
        entity: primary_ent,
        pos: p_pos.0,
        vel: p_vel.0,
        mass: p_mass.0,
        radius: p_rad.0,
    };

    for (inner_ent, pos, vel, mass, rad, body, mut state, opt_tide) in bodies_params.p1().iter_mut()
    {
        // Resolve perturber: check if cached entity is still valid via O(1) index map, else fallback to name
        let perturber_opt = resolve_perturber_snapshot(
            state.perturber_entity,
            &state.perturber_name,
            &perturber_snapshots_buf,
            &perturber_index_buf,
        );

        let Some((pert_ent, pert_pos, pert_vel, pert_mass)) = perturber_opt else {
            state.regime = KozaiRegime::Inactive;
            state.is_in_resonance = false;
            state.perturber_entity = None;
            state.perturber_name.clear();
            continue;
        };

        state.perturber_entity = Some(pert_ent);
        let perturber = PerturberContext {
            entity: pert_ent,
            pos: pert_pos,
            vel: pert_vel,
            mass: pert_mass,
        };

        evolve_body_resonance(
            &config,
            base_dt_yr,
            &primary,
            &perturber,
            inner_ent,
            pos.0,
            vel,
            mass.0,
            rad.0,
            &body.name,
            state,
            opt_tide,
            &mut disruption_events,
        );
    }
}

struct PrimaryContext {
    entity: Entity,
    pos: bevy::math::DVec3,
    vel: bevy::math::DVec3,
    mass: f64,
    radius: f64,
}

struct PerturberContext {
    entity: Entity,
    pos: bevy::math::DVec3,
    vel: bevy::math::DVec3,
    mass: f64,
}

#[allow(clippy::too_many_arguments, reason = "Kozai-Lidov secular dynamics")]
fn evolve_body_resonance(
    config: &KozaiLidovConfig,
    dt_years: f64,
    primary: &PrimaryContext,
    perturber: &PerturberContext,
    inner_ent: Entity,
    pos: bevy::math::DVec3,
    vel: Mut<SimVelocity>,
    mass: f64,
    rad: f64,
    name: &str,
    mut state: Mut<KozaiLidovState>,
    opt_tide: Option<Mut<TidalState>>,
    disruption_events: &mut MessageWriter<KozaiDisruptionEvent>,
) {
    let rel_inner_pos = pos - primary.pos;
    let rel_inner_vel = vel.0 - primary.vel;
    let h_inner = rel_inner_pos.cross(rel_inner_vel);

    let rel_outer_pos = perturber.pos - primary.pos;
    let rel_outer_vel = perturber.vel - primary.vel;
    let h_outer = rel_outer_pos.cross(rel_outer_vel);

    // Orbital elements relative to central star
    let Some(orb_in) =
        state_vectors_to_orbital_elements(rel_inner_pos, rel_inner_vel, primary.mass, mass)
    else {
        return;
    };
    let Some(orb_out) = state_vectors_to_orbital_elements(
        rel_outer_pos,
        rel_outer_vel,
        primary.mass,
        perturber.mass,
    ) else {
        return;
    };

    let i_mut_rad = compute_mutual_inclination(h_inner, h_outer);
    state.mutual_inclination_deg = i_mut_rad.to_degrees();
    state.is_in_resonance = is_in_kozai_resonance(i_mut_rad);

    // Compute timescales
    let tau_kl = compute_kozai_timescale(
        orb_in.semi_major_axis,
        orb_out.semi_major_axis,
        primary.mass,
        perturber.mass,
        orb_out.eccentricity,
    );
    state.kozai_period_years = tau_kl;

    let tau_gr =
        compute_gr_precession_timescale(orb_in.semi_major_axis, primary.mass, orb_in.eccentricity);

    let unsuppressed_e_max = compute_max_eccentricity(orb_in.eccentricity, i_mut_rad);
    let (gr_ratio, is_gr_suppressed, effective_e_max) = if config.enable_gr_suppression {
        compute_gr_resonance_suppression(tau_kl, tau_gr, unsuppressed_e_max)
    } else {
        (0.0, false, unsuppressed_e_max)
    };

    state.gr_precession_ratio = gr_ratio;
    state.is_gr_suppressed = is_gr_suppressed;
    state.max_eccentricity_forecast = effective_e_max;

    let q_min = compute_min_periastron(orb_in.semi_major_axis, effective_e_max);
    state.min_periastron_au = q_min;

    let roche_radius_au = compute_roche_disruption_radius(rad, primary.mass, mass);
    let danger_radius_au = roche_radius_au.max(primary.radius);

    let mu_in = crate::utils::constants::G_ASTRO * (primary.mass + mass);
    let omega_mut = compute_mutual_argument_of_periapsis(
        rel_inner_pos,
        rel_inner_vel,
        h_outer,
        mu_in,
        orb_in.argument_of_periapsis,
    );

    // Classify regime using mutual argument of periapsis
    let prev_regime = state.regime;
    state.regime = classify_kozai_regime(
        state.is_in_resonance,
        state.is_gr_suppressed,
        q_min,
        danger_radius_au,
        omega_mut,
    );

    // Disruption detection: if current periastron breaches the stellar surface or Roche limit
    if orb_in.periapsis <= danger_radius_au {
        state.regime = KozaiRegime::TidalDisruptionRisk;
        if prev_regime != KozaiRegime::TidalDisruptionRisk {
            disruption_events.write(KozaiDisruptionEvent {
                disrupted_entity: inner_ent,
                central_entity: primary.entity,
                perturber_entity: Some(perturber.entity),
                body_name: name.to_string(),
                peak_eccentricity: orb_in.eccentricity,
                periastron_au: orb_in.periapsis,
                roche_limit_au: danger_radius_au,
                position: pos,
            });
        }
    }

    // Secular evolution fed back into state vectors before tidal dissipation runs
    apply_secular_evolution_and_tides(
        config,
        dt_years,
        primary.vel,
        rel_inner_pos,
        vel,
        h_outer,
        orb_in,
        omega_mut,
        tau_kl,
        i_mut_rad,
        tau_gr,
        mu_in,
        rad,
        &mut state,
        opt_tide,
    );
}

/// Helper to resolve perturber entity by cached entity ID or matching name from snapshots.
fn resolve_perturber_snapshot(
    cached_ent: Option<Entity>,
    cached_name: &str,
    snapshots: &[(Entity, String, bevy::math::DVec3, bevy::math::DVec3, f64)],
    index_map: &rustc_hash::FxHashMap<Entity, usize>,
) -> Option<(Entity, bevy::math::DVec3, bevy::math::DVec3, f64)> {
    if let Some(ent) = cached_ent
        && let Some(&idx) = index_map.get(&ent)
        && let Some(cand) = snapshots.get(idx)
    {
        return Some((cand.0, cand.2, cand.3, cand.4));
    }
    if !cached_name.is_empty()
        && let Some(cand) = snapshots.iter().find(|s| s.1 == cached_name)
    {
        return Some((cand.0, cand.2, cand.3, cand.4));
    }
    None
}

/// Helper to apply secular time step evolution into Cartesian velocity vectors and tidal coupling.
#[allow(
    clippy::too_many_arguments,
    reason = "Secular Kozai element integration updates orbital velocity vector and tidal state"
)]
fn apply_secular_evolution_and_tides(
    config: &KozaiLidovConfig,
    base_dt_yr: f64,
    primary_vel: bevy::math::DVec3,
    rel_pos: bevy::math::DVec3,
    mut vel: Mut<SimVelocity>,
    h_outer: bevy::math::DVec3,
    orb_in: crate::utils::math::OrbitalElements,
    omega_mut: f64,
    tau_kl: f64,
    i_mut_rad: f64,
    tau_gr: f64,
    mu_in: f64,
    rad_au: f64,
    state: &mut Mut<KozaiLidovState>,
    mut opt_tide: Option<Mut<TidalState>>,
) {
    let dt_years = base_dt_yr * config.secular_time_scale;
    if !state.is_in_resonance || state.is_gr_suppressed || dt_years <= 1e-6 {
        return;
    }

    state.cycle_phase = (state.cycle_phase + dt_years / tau_kl.max(1.0)).rem_euclid(1.0);

    let gr_rate = if config.enable_gr_suppression && tau_gr.is_finite() && tau_gr > 0.0 {
        (2.0 * std::f64::consts::PI) / tau_gr
    } else {
        0.0
    };

    // Subtract the 1x physical N-body quadrupole contribution already integrated by the physics step
    // so secular acceleration does not double-count quadrupole precession.
    let extra_dt_years = base_dt_yr * (config.secular_time_scale - 1.0).max(0.0);
    let (new_e, new_i, new_omega) = if extra_dt_years > 1e-9 {
        evaluate_secular_kozai_step(
            orb_in.eccentricity,
            i_mut_rad,
            omega_mut,
            tau_kl,
            extra_dt_years,
            gr_rate,
        )
    } else {
        (orb_in.eccentricity, i_mut_rad, omega_mut)
    };

    // Apply incremental secular torque into orbital state vectors when accelerated secular time scale is active
    if extra_dt_years > 1e-9 {
        let rel_vel = vel.0 - primary_vel;
        let r_len = rel_pos.length();
        let v_sq = rel_vel.length_squared();
        if r_len > 1e-6 && v_sq > 1e-12 && orb_in.semi_major_axis > 1e-6 {
            let mut rel_v = rel_vel;
            let r_hat = rel_pos / r_len;

            // 1. Apply inclination step delta_i around r_hat scaled by 1 / (r_hat · n_mut)
            //    so the orbital angular momentum plane tilts by delta_i without moving rel_pos.
            let delta_i = (new_i - i_mut_rad).clamp(-0.02, 0.02);
            let h_in = rel_pos.cross(rel_v);
            let plane_axis = h_outer.cross(h_in);
            let n_mut = if plane_axis.length_squared() >= 1e-12 {
                plane_axis.normalize()
            } else {
                r_hat
            };
            let proj = n_mut.dot(r_hat);
            if delta_i.abs() > 1e-9 && proj.abs() > 0.15 {
                let rot_angle = (delta_i / proj).clamp(-0.08, 0.08);
                let (s, c) = rot_angle.sin_cos();
                rel_v = rel_v * c + r_hat.cross(rel_v) * s + r_hat * (r_hat.dot(rel_v) * (1.0 - c));
            }

            // 2. Adjust tangential vs radial speed at constant energy (a = const) to track new_e
            let target_h2 = mu_in * orb_in.semi_major_axis * (1.0 - new_e * new_e).max(1e-4);
            let target_vt_sq = (target_h2 / (r_len * r_len)).min(v_sq);
            let v_rad_mag = rel_v.dot(r_hat);
            let v_tan_vec = rel_v - r_hat * v_rad_mag;
            let cur_vt = v_tan_vec.length();
            if cur_vt > 1e-8 && target_vt_sq >= 0.0 {
                let new_vt = target_vt_sq.sqrt();
                let new_vr_mag = (v_sq - target_vt_sq).max(0.0).sqrt();
                let new_vr_sign = if v_rad_mag.abs() > 1e-10 {
                    v_rad_mag.signum()
                } else if new_omega.sin() < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                rel_v = (v_tan_vec / cur_vt) * new_vt + r_hat * (new_vr_sign * new_vr_mag);
            }

            if rel_v.is_finite() {
                vel.0 = primary_vel + rel_v;
            }
        }
    }

    if new_i.is_finite() {
        state.mutual_inclination_deg = new_i.to_degrees();
    }

    // Couple with high-eccentricity tidal dissipation if enabled (update_tidal_evolution runs next)
    if config.enable_tidal_circularization
        && let Some(ref mut tide) = opt_tide
        && new_e > 0.4
    {
        let one_minus_e2 = (1.0 - new_e * new_e).max(0.01);
        let tidal_boost = ((1.0 + 3.75 * new_e * new_e) / one_minus_e2.powf(7.5)).clamp(1.0, 1e5);
        tide.tidal_heating_power_watts = (tide.tidal_heating_power_watts * tidal_boost).min(1e24);
        tide.tidal_heating_flux_w_m2 = crate::simulation::tides::calculate_tidal_heat_flux(
            tide.tidal_heating_power_watts,
            rad_au,
        );
        let boosted_decay =
            (tide.circularization_rate_per_myr.abs() * tidal_boost.min(1e4)).clamp(1e-2, 1e4);
        tide.circularization_rate_per_myr = -boosted_decay;
    }
}
