//! ECS systems for detecting hierarchical triples, evaluating Kozai-Lidov resonance,
//! applying secular orbital evolution, and coupling with tidal dissipation.

use bevy::prelude::*;

use super::physics::*;
use super::types::*;
use crate::simulation::components::*;
use crate::simulation::resources::{SimTime, TimeWarp};
use crate::simulation::tides::TidalState;
use crate::utils::math::state_vectors_to_orbital_elements;

/// System that automatically scans the system for hierarchical triples (inner body + outer massive perturber)
/// and initializes or updates `KozaiLidovState` components.
pub fn detect_hierarchical_triples(
    mut commands: Commands,
    primary_query: Query<
        (Entity, &SimPosition, &SimVelocity, &Mass),
        (With<CentralStar>, With<CelestialBody>),
    >,
    all_bodies_query: Query<
        (Entity, &SimPosition, &SimVelocity, &Mass, &CelestialBody),
        Without<CentralStar>,
    >,
    existing_state_query: Query<Entity, With<KozaiLidovState>>,
) {
    let Some((_primary_ent, p_pos, p_vel, p_mass)) = primary_query.iter().next() else {
        return;
    };

    if p_mass.0 <= 1e-6 {
        return;
    }

    // Collect outer perturber candidates: must have substantial mass (> 1e-5 M_sun, ~3 M_earth)
    let candidates: Vec<(
        Entity,
        f64,
        f64,
        String,
        bevy::math::DVec3,
        bevy::math::DVec3,
    )> = all_bodies_query
        .iter()
        .filter(|(_, _, _, mass, _)| mass.0 >= 1e-5)
        .map(|(ent, pos, vel, mass, body)| {
            let dist = (pos.0 - p_pos.0).length();
            (ent, mass.0, dist, body.name.clone(), pos.0, vel.0)
        })
        .collect();

    for (inner_ent, pos, vel, _mass, body) in all_bodies_query.iter() {
        if body.body_type.is_star_or_remnant() {
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

        for &(cand_ent, cand_mass, cand_dist, ref cand_name, cand_pos, cand_vel) in &candidates {
            if cand_ent == inner_ent || cand_dist < 2.5 * inner_dist {
                continue;
            }

            let strength = cand_mass / cand_dist.powi(3);
            if strength > max_quadrupole_strength {
                max_quadrupole_strength = strength;
                best_perturber = Some((
                    cand_ent,
                    cand_name.clone(),
                    cand_dist,
                    cand_pos,
                    cand_vel,
                    cand_mass,
                ));
            }
        }

        if let Some((pert_ent, pert_name, pert_dist, pert_pos, pert_vel, pert_mass)) =
            best_perturber
        {
            if !existing_state_query.contains(inner_ent) {
                // Initialize new state
                let h_inner = (pos.0 - p_pos.0).cross(vel.0 - p_vel.0);
                let h_outer = (pert_pos - p_pos.0).cross(pert_vel - p_vel.0);
                let i_mut_rad = compute_mutual_inclination(h_inner, h_outer);
                let is_res = is_in_kozai_resonance(i_mut_rad);
                let e_max = compute_max_eccentricity(0.01, i_mut_rad);
                let q_min = compute_min_periastron(inner_dist, e_max);
                let tau_kl =
                    compute_kozai_timescale(inner_dist, pert_dist, p_mass.0, pert_mass, 0.0);

                commands.entity(inner_ent).insert(KozaiLidovState {
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
    _sim_time: Res<SimTime>,
    primary_query: Query<
        (Entity, &SimPosition, &SimVelocity, &Mass, &Radius),
        (With<CentralStar>, With<CelestialBody>),
    >,
    mut bodies_params: ParamSet<(
        Query<(Entity, &SimPosition, &SimVelocity, &Mass, &CelestialBody), Without<CentralStar>>,
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
            Without<CentralStar>,
        >,
    )>,
    mut disruption_events: MessageWriter<KozaiDisruptionEvent>,
) {
    if !config.enable_kozai_lidov || time_warp.is_paused {
        return;
    }

    let Some((primary_ent, p_pos, p_vel, p_mass, p_rad)) = primary_query.iter().next() else {
        return;
    };

    let dt_years = time_warp.multiplier * (1.0 / 365.25) * config.secular_time_scale;

    let perturber_snapshots: Vec<(Entity, String, bevy::math::DVec3, bevy::math::DVec3, f64)> =
        bodies_params
            .p0()
            .iter()
            .map(|(e, pos, vel, mass, body)| (e, body.name.clone(), pos.0, vel.0, mass.0))
            .collect();

    let primary = PrimaryContext {
        entity: primary_ent,
        pos: p_pos.0,
        vel: p_vel.0,
        mass: p_mass.0,
        radius: p_rad.0,
    };

    for (inner_ent, pos, vel, mass, rad, body, mut state, opt_tide) in bodies_params.p1().iter_mut()
    {
        // Resolve perturber: check if cached entity is still valid, else try matching by name
        let perturber_opt = resolve_perturber_snapshot(
            state.perturber_entity,
            &state.perturber_name,
            &perturber_snapshots,
        );

        let Some((pert_ent, pert_pos, pert_vel, pert_mass)) = perturber_opt else {
            state.regime = KozaiRegime::Inactive;
            state.is_in_resonance = false;
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
            dt_years,
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
    mut vel: Mut<SimVelocity>,
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

    // Classify regime
    state.regime = classify_kozai_regime(
        state.is_in_resonance,
        state.is_gr_suppressed,
        q_min,
        danger_radius_au,
        orb_in.argument_of_periapsis,
    );

    // Disruption detection: if current periastron breaches the stellar surface or Roche limit
    if orb_in.periapsis <= danger_radius_au {
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

    // Secular evolution and tidal coupling
    apply_secular_evolution_and_tides(
        config, dt_years, orb_in, tau_kl, i_mut_rad, tau_gr, &mut vel, &mut state, opt_tide,
    );
}

/// Helper to resolve perturber entity by cached entity ID or matching name from snapshots.
fn resolve_perturber_snapshot(
    cached_ent: Option<Entity>,
    cached_name: &str,
    snapshots: &[(Entity, String, bevy::math::DVec3, bevy::math::DVec3, f64)],
) -> Option<(Entity, bevy::math::DVec3, bevy::math::DVec3, f64)> {
    if let Some(ent) = cached_ent {
        if let Some(cand) = snapshots.iter().find(|s| s.0 == ent) {
            return Some((cand.0, cand.2, cand.3, cand.4));
        }
    }
    if !cached_name.is_empty() {
        if let Some(cand) = snapshots.iter().find(|s| s.1 == cached_name) {
            return Some((cand.0, cand.2, cand.3, cand.4));
        }
    }
    None
}

/// Helper to apply secular time step evolution and tidal dissipation coupling.
fn apply_secular_evolution_and_tides(
    config: &KozaiLidovConfig,
    dt_years: f64,
    orb_in: crate::utils::math::OrbitalElements,
    tau_kl: f64,
    i_mut_rad: f64,
    tau_gr: f64,
    vel: &mut Mut<SimVelocity>,
    state: &mut Mut<KozaiLidovState>,
    mut opt_tide: Option<Mut<TidalState>>,
) {
    if !state.is_in_resonance || state.is_gr_suppressed || dt_years <= 1e-6 {
        return;
    }

    state.cycle_phase = (state.cycle_phase + dt_years / tau_kl.max(1.0)).rem_euclid(1.0);

    let gr_rate = if config.enable_gr_suppression && tau_gr.is_finite() && tau_gr > 0.0 {
        (2.0 * std::f64::consts::PI) / tau_gr
    } else {
        0.0
    };

    // Secular stepping
    let (new_e, _new_i, _) = evaluate_secular_kozai_step(
        orb_in.eccentricity,
        i_mut_rad,
        orb_in.argument_of_periapsis,
        tau_kl,
        dt_years,
        gr_rate,
    );

    // Couple with high-eccentricity tidal dissipation if enabled
    if config.enable_tidal_circularization {
        if let Some(ref mut tide) = opt_tide {
            if new_e > 0.4 {
                // Hut (1981) tidal energy dissipation boost factor
                let one_minus_e2 = (1.0 - new_e * new_e).max(0.01);
                let tidal_boost = (1.0 + 3.75 * new_e * new_e) / one_minus_e2.powf(7.5);
                tide.tidal_heating_power_watts *= tidal_boost.min(1e5);
                tide.circularization_rate_per_myr =
                    (tide.circularization_rate_per_myr * tidal_boost.min(1e4)).max(1e-2);
            }
        }
    }

    // Gentle velocity modulation towards secular eccentricity at high time warp
    if dt_years > 0.05 && new_e.is_finite() && (new_e - orb_in.eccentricity).abs() > 1e-4 {
        let e_factor = (new_e / orb_in.eccentricity.max(1e-3)).clamp(0.9, 1.1);
        vel.0 *= 1.0 + (e_factor - 1.0) * (dt_years / tau_kl.max(10.0)).min(0.05);
    }
}
