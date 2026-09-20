//! Real-time trajectory prediction, close-encounter forecasting, and impact detection.

use bevy::math::{DVec3, Vec3};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

use crate::simulation::components::{
    CelestialBody, CentralStar, Mass, Radius, SimPosition, SimVelocity,
};
use crate::simulation::resources::{PlayerInteractionState, SlingshotState};
use crate::utils::constants::{AU_TO_KM, G_ASTRO};
use crate::utils::math::{state_vectors_to_orbital_elements, OrbitalElements};

/// Velocity conversion factor: 1 AU/yr in km/s (~4.74047 km/s)
pub const AU_PER_YR_TO_KM_S: f64 = AU_TO_KM / (365.25 * 86_400.0);

/// Severity classification of a predicted encounter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncounterType {
    /// Gravitational flyby outside the immediate Hill sphere (within 2 Hill radii)
    SafeFlyby,
    /// Trajectory penetrates target body's gravitational Hill sphere
    HillSpherePenetration,
    /// Trajectory crosses inside the target body's fluid Roche tidal disruption limit
    RocheLobeCrossing,
    /// Trajectory intersects the physical collision cross-section of the target body
    DirectImpact,
}

impl EncounterType {
    pub fn display_label(self) -> &'static str {
        match self {
            Self::SafeFlyby => "Safe Flyby",
            Self::HillSpherePenetration => "Hill Sphere Flyby",
            Self::RocheLobeCrossing => "Roche Tidal Hazard",
            Self::DirectImpact => "DIRECT COLLISION",
        }
    }

    pub fn badge_color(self) -> Color {
        match self {
            Self::SafeFlyby => Color::srgb(0.2, 0.9, 0.5),
            Self::HillSpherePenetration => Color::srgb(1.0, 0.85, 0.2),
            Self::RocheLobeCrossing => Color::srgb(1.0, 0.5, 0.1),
            Self::DirectImpact => Color::srgb(1.0, 0.2, 0.2),
        }
    }
}

/// Details of a detected close encounter along the predicted trajectory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictedEncounter {
    pub target_entity: Entity,
    pub target_name: String,
    pub time_to_encounter_yr: f64,
    pub encounter_pos_au: DVec3,
    pub target_pos_at_encounter_au: DVec3,
    pub min_distance_au: f64,
    pub min_distance_km: f64,
    pub relative_velocity_kms: f64,
    pub target_radius_au: f64,
    pub target_hill_radius_au: f64,
    pub target_roche_radius_au: f64,
    pub encounter_type: EncounterType,
}

/// Global resource managing real-time trajectory forecasting and close-encounter analysis.
#[derive(Resource, Debug, Clone)]
pub struct TrajectoryPredictorState {
    /// Whether the trajectory predictor overlay and calculations are active
    pub is_enabled: bool,
    /// Lookahead horizon in simulation years (default 30.0 yr)
    pub forecast_horizon_yr: f64,
    /// Forward-sampled 3D trajectory polyline points for gizmo visualization
    pub trajectory_points: Vec<Vec3>,
    /// Currently detected closest encounter (if any)
    pub active_encounter: Option<PredictedEncounter>,
    /// Source body name or label being predicted
    pub source_label: String,
}

impl Default for TrajectoryPredictorState {
    fn default() -> Self {
        Self {
            is_enabled: true,
            forecast_horizon_yr: 30.0,
            trajectory_points: Vec::new(),
            active_encounter: None,
            source_label: String::new(),
        }
    }
}

/// Target body candidate for encounter testing.
#[derive(Debug, Clone)]
pub struct TargetCandidate {
    pub entity: Entity,
    pub name: String,
    pub elements: OrbitalElements,
    pub radius_au: f64,
    pub mass_solar: f64,
}

/// Solves Kepler's equation $E - e \sin E = M$ for eccentric anomaly ($e < 1.0$) via Newton-Raphson.
pub fn solve_kepler_eccentric(mean_anomaly: f64, eccentricity: f64) -> f64 {
    let e = eccentricity.clamp(0.0, 0.99999);
    let m = mean_anomaly.rem_euclid(2.0 * PI);

    let mut e_anom = if e < 0.8 {
        m + e * m.sin() + 0.5 * e * e * (2.0 * m).sin()
    } else {
        PI
    };

    for _ in 0..10 {
        let f = e_anom - e * e_anom.sin() - m;
        let f_prime = 1.0 - e * e_anom.cos();
        if f_prime.abs() < 1e-12 {
            break;
        }
        let delta = f / f_prime;
        e_anom -= delta;
        if delta.abs() < 1e-11 {
            break;
        }
    }

    e_anom
}

/// Solves hyperbolic Kepler's equation $e \sinh H - H = M_h$ for hyperbolic anomaly ($e > 1.0$).
pub fn solve_kepler_hyperbolic(mean_anomaly_h: f64, eccentricity: f64) -> f64 {
    let e = eccentricity.max(1.00001);
    let m_h = mean_anomaly_h;

    let mut h = (2.0 * m_h / e).asinh();

    for _ in 0..10 {
        let f = e * h.sinh() - h - m_h;
        let f_prime = e * h.cosh() - 1.0;
        if f_prime.abs() < 1e-12 {
            break;
        }
        let delta = f / f_prime;
        h = (h - delta).clamp(-700.0, 700.0);
        if delta.abs() < 1e-11 {
            break;
        }
    }

    h
}

/// Propagates Keplerian orbital elements forward by $\Delta t$ years and computes position & velocity vectors.
pub fn propagate_kepler_position_velocity(
    elements: &OrbitalElements,
    delta_t_yr: f64,
    central_mass: f64,
) -> Option<(DVec3, DVec3)> {
    if elements.semi_major_axis <= 0.0 && elements.eccentricity < 1.0 {
        return None;
    }

    let mu = G_ASTRO * central_mass.max(0.01);
    let e = elements.eccentricity;
    let nu_0 = elements.true_anomaly;

    if e < 1.0 {
        let a = elements.semi_major_axis;
        let period = elements.period_years.max(1e-5);
        let n = 2.0 * PI / period;

        let cos_e0 = (e + nu_0.cos()) / (1.0 + e * nu_0.cos());
        let sin_e0 = ((1.0 - e * e).max(0.0)).sqrt() * nu_0.sin() / (1.0 + e * nu_0.cos());
        let e_0 = sin_e0.atan2(cos_e0);
        let m_0 = e_0 - e * e_0.sin();

        let m_t = m_0 + n * delta_t_yr;
        let e_t = solve_kepler_eccentric(m_t, e);

        let p_vec = elements.periapsis_dir;
        let q_vec = elements.semilatus_dir;

        let pos =
            p_vec * (a * (e_t.cos() - e)) + q_vec * (a * (1.0 - e * e).max(0.0).sqrt() * e_t.sin());

        let de_dt = n / (1.0 - e * e_t.cos()).max(1e-6);
        let vel = p_vec * (-a * e_t.sin() * de_dt)
            + q_vec * (a * (1.0 - e * e).max(0.0).sqrt() * e_t.cos() * de_dt);

        Some((pos, vel))
    } else {
        let a = elements.semi_major_axis.abs().max(0.01);
        let n_h = (mu / a.powi(3)).sqrt();

        let denom = (1.0 + e * nu_0.cos()).abs().max(1e-6);
        let cosh_h0 = ((e + nu_0.cos()) / denom).max(1.0);
        let sinh_h0 = (e * e - 1.0).max(0.0).sqrt() * nu_0.sin() / denom;
        let h_0 = (cosh_h0 + sinh_h0).max(1e-12).ln();
        let m_h0 = e * h_0.sinh() - h_0;

        let m_ht = m_h0 + n_h * delta_t_yr;
        let h_t = solve_kepler_hyperbolic(m_ht, e);

        let p_vec = elements.periapsis_dir;
        let q_vec = elements.semilatus_dir;

        let pos = p_vec * (a * (e - h_t.cosh()))
            + q_vec * (a * (e * e - 1.0).max(0.0).sqrt() * h_t.sinh());

        let dh_dt = n_h / (e * h_t.cosh() - 1.0).max(1e-6);
        let vel = p_vec * (-a * h_t.sinh() * dh_dt)
            + q_vec * (a * (e * e - 1.0).max(0.0).sqrt() * h_t.cosh() * dh_dt);

        Some((pos, vel))
    }
}

/// Scans for close encounters along a Keplerian trajectory against a list of target celestial bodies.
pub fn find_closest_encounter(
    source_elements: &OrbitalElements,
    source_radius_au: f64,
    star_mass: f64,
    targets: &[TargetCandidate],
    horizon_yr: f64,
) -> (Vec<Vec3>, Option<PredictedEncounter>) {
    let steps = 100;
    let dt_step = horizon_yr / (steps as f64);
    let mut trajectory_points = Vec::with_capacity(steps + 1);

    for i in 0..=steps {
        let t = (i as f64) * dt_step;
        if let Some((pos, _)) = propagate_kepler_position_velocity(source_elements, t, star_mass) {
            trajectory_points.push(Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32));
        }
    }

    let mut best_encounter: Option<PredictedEncounter> = None;

    for target in targets {
        if let Some(enc) = check_single_target_encounter(
            source_elements,
            source_radius_au,
            star_mass,
            target,
            horizon_yr,
            steps,
        ) {
            let is_closer = best_encounter.as_ref().is_none_or(|b| {
                let current_ratio = enc.min_distance_au / enc.target_hill_radius_au.max(1e-4);
                let best_ratio = b.min_distance_au / b.target_hill_radius_au.max(1e-4);
                current_ratio < best_ratio
            });

            if is_closer {
                best_encounter = Some(enc);
            }
        }
    }

    (trajectory_points, best_encounter)
}

fn check_single_target_encounter(
    source_elements: &OrbitalElements,
    source_radius_au: f64,
    star_mass: f64,
    target: &TargetCandidate,
    horizon_yr: f64,
    steps: usize,
) -> Option<PredictedEncounter> {
    let dt_step = horizon_yr / (steps as f64);
    let mut min_dist_au = f64::INFINITY;
    let mut best_t_idx = 0;

    let target_a = target.elements.semi_major_axis.max(0.1);
    let hill_r =
        (target_a * (target.mass_solar / (3.0 * star_mass.max(0.1))).cbrt()).clamp(0.002, 2.5);
    let roche_r = target.radius_au * 2.44;
    let collision_r = target.radius_au + source_radius_au;

    for i in 0..=steps {
        let t = (i as f64) * dt_step;
        let Some((src_pos, _)) = propagate_kepler_position_velocity(source_elements, t, star_mass)
        else {
            continue;
        };
        let Some((tgt_pos, _)) = propagate_kepler_position_velocity(&target.elements, t, star_mass)
        else {
            continue;
        };

        let dist = (src_pos - tgt_pos).length();
        if dist < min_dist_au {
            min_dist_au = dist;
            best_t_idx = i;
        }
    }

    // Must approach within 2.0 Hill radii to register as an encounter
    if min_dist_au > hill_r * 2.0 {
        return None;
    }

    // Golden section refinement of closest approach timestamp
    let t_coarse = (best_t_idx as f64) * dt_step;
    let t_min = (t_coarse - dt_step).max(0.0);
    let t_max = (t_coarse + dt_step).min(horizon_yr);

    let refined_t =
        refine_closest_approach_time(source_elements, &target.elements, star_mass, t_min, t_max);

    let (src_pos, src_vel) =
        propagate_kepler_position_velocity(source_elements, refined_t, star_mass)?;
    let (tgt_pos, tgt_vel) =
        propagate_kepler_position_velocity(&target.elements, refined_t, star_mass)?;

    let exact_dist_au = (src_pos - tgt_pos).length();
    let rel_vel_au_yr = (src_vel - tgt_vel).length();
    let rel_vel_kms = rel_vel_au_yr * AU_PER_YR_TO_KM_S;

    let enc_type = if exact_dist_au <= collision_r {
        EncounterType::DirectImpact
    } else if exact_dist_au <= roche_r {
        EncounterType::RocheLobeCrossing
    } else if exact_dist_au <= hill_r {
        EncounterType::HillSpherePenetration
    } else {
        EncounterType::SafeFlyby
    };

    Some(PredictedEncounter {
        target_entity: target.entity,
        target_name: target.name.clone(),
        time_to_encounter_yr: refined_t,
        encounter_pos_au: src_pos,
        target_pos_at_encounter_au: tgt_pos,
        min_distance_au: exact_dist_au,
        min_distance_km: exact_dist_au * AU_TO_KM,
        relative_velocity_kms: rel_vel_kms,
        target_radius_au: target.radius_au,
        target_hill_radius_au: hill_r,
        target_roche_radius_au: roche_r,
        encounter_type: enc_type,
    })
}

fn refine_closest_approach_time(
    source_elements: &OrbitalElements,
    target_elements: &OrbitalElements,
    star_mass: f64,
    mut a: f64,
    mut b: f64,
) -> f64 {
    let phi = 0.618_033_988_749_895; // Golden ratio (sqrt(5) - 1) / 2
    let mut c = b - phi * (b - a);
    let mut d = a + phi * (b - a);

    let dist_at = |t: f64| -> f64 {
        let (Some((p1, _)), Some((p2, _))) = (
            propagate_kepler_position_velocity(source_elements, t, star_mass),
            propagate_kepler_position_velocity(target_elements, t, star_mass),
        ) else {
            return f64::INFINITY;
        };
        (p1 - p2).length()
    };

    let mut fc = dist_at(c);
    let mut fd = dist_at(d);

    for _ in 0..12 {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - phi * (b - a);
            fc = dist_at(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + phi * (b - a);
            fd = dist_at(d);
        }
    }

    f64::midpoint(a, b)
}

/// ECS system that continuously updates the trajectory predictor for the selected body or active Slingshot.
#[allow(
    clippy::type_complexity,
    reason = "Trajectory predictor system requires querying multiple celestial body components"
)]
pub fn update_trajectory_predictor(
    mut predictor_state: ResMut<TrajectoryPredictorState>,
    player_state: Res<PlayerInteractionState>,
    slingshot_state: Res<SlingshotState>,
    star_query: Query<(&SimPosition, &Mass), With<CentralStar>>,
    bodies_query: Query<
        (
            Entity,
            &CelestialBody,
            &SimPosition,
            &SimVelocity,
            &Mass,
            &Radius,
        ),
        Without<CentralStar>,
    >,
) {
    if !predictor_state.is_enabled {
        predictor_state.trajectory_points.clear();
        predictor_state.active_encounter = None;
        return;
    }

    let Ok((star_pos, star_mass)) = star_query.single() else {
        predictor_state.trajectory_points.clear();
        predictor_state.active_encounter = None;
        return;
    };

    let mut target_candidates = Vec::new();
    let mut selected_source: Option<(OrbitalElements, f64, String)> = None;

    // Determine source elements: Slingshot drag or Selected entity
    if slingshot_state.is_active {
        if let (Some(origin_dvec), Some(curr_dvec)) =
            (slingshot_state.drag_origin, slingshot_state.drag_current)
        {
            let delta = curr_dvec - origin_dvec;
            let launch_vel = delta * slingshot_state.velocity_scale;
            let rel_pos = origin_dvec - star_pos.0;

            if let Some(el) =
                state_vectors_to_orbital_elements(rel_pos, launch_vel, star_mass.0, 1e-6)
            {
                selected_source = Some((
                    el,
                    0.0001,
                    format!("Slingshot ({:?})", slingshot_state.archetype),
                ));
            }
        }
    } else if let Some(selected_ent) = player_state.selected_entity {
        if let Ok((_, body, pos, vel, mass, rad)) = bodies_query.get(selected_ent) {
            let rel_pos = pos.0 - star_pos.0;
            if let Some(el) = state_vectors_to_orbital_elements(rel_pos, vel.0, star_mass.0, mass.0)
            {
                selected_source = Some((el, rad.0, body.name.clone()));
            }
        }
    }

    let Some((src_elements, src_radius, src_name)) = selected_source else {
        predictor_state.trajectory_points.clear();
        predictor_state.active_encounter = None;
        return;
    };

    predictor_state.source_label = src_name;

    // Collect candidate targets (major planets, dwarf planets, protoplanets)
    for (entity, body, pos, vel, mass, rad) in &bodies_query {
        if player_state.selected_entity == Some(entity) {
            continue;
        }

        let rel_pos = pos.0 - star_pos.0;
        if let Some(el) = state_vectors_to_orbital_elements(rel_pos, vel.0, star_mass.0, mass.0) {
            target_candidates.push(TargetCandidate {
                entity,
                name: body.name.clone(),
                elements: el,
                radius_au: rad.0,
                mass_solar: mass.0,
            });
        }
    }

    let horizon = predictor_state.forecast_horizon_yr;
    let (pts, encounter) = find_closest_encounter(
        &src_elements,
        src_radius,
        star_mass.0,
        &target_candidates,
        horizon,
    );

    predictor_state.trajectory_points = pts;
    predictor_state.active_encounter = encounter;
}
