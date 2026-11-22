//! Real-time trajectory prediction, close-encounter forecasting, and impact detection.

use bevy::math::{DVec3, Vec3};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

use crate::simulation::components::{
    BodyType, CelestialBody, CentralStar, Mass, Radius, SatelliteOf, SimPosition, SimVelocity,
};
use crate::simulation::resources::{PlayerInteractionState, SimTime, SlingshotState, TimeWarp};
use crate::utils::constants::{AU_TO_KM, EARTH_MASS_SOLAR, G_ASTRO};
use crate::utils::math::{OrbitalElements, state_vectors_to_orbital_elements};

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

    let mut h = if m_h.abs() < 1e-6 {
        m_h / (e - 1.0)
    } else {
        (2.0 * m_h.abs() / e + 1.8).ln().copysign(m_h)
    };

    for _ in 0..24 {
        let sh = h.sinh();
        let ch = h.cosh();
        let f = e * sh - h - m_h;
        let f_prime = e * ch - 1.0;
        if f_prime.abs() < 1e-12 {
            break;
        }
        let delta = (f / f_prime).clamp(-10.0, 10.0);
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
    let mu = G_ASTRO * central_mass.max(1e-12);
    let e = elements.eccentricity;
    let nu_0 = elements.true_anomaly;
    let p_vec = elements.periapsis_dir;
    let q_vec = elements.semilatus_dir;

    if (e - 1.0).abs() <= 1e-4 || !elements.semi_major_axis.is_finite() {
        // Near-parabolic orbit: exact closed-form Barker's equation D + D^3 / 3 = M_p
        let q = elements.periapsis.max(1e-7);
        let half_nu_cos = (0.5 * nu_0).cos();
        if half_nu_cos.abs() < 1e-7 {
            return None;
        }
        let d_0 = (0.5 * nu_0).tan();
        let m_p0 = d_0 + (d_0 * d_0 * d_0) / 3.0;
        let n_p = (mu / (2.0 * q * q * q)).sqrt();
        let m_pt = m_p0 + n_p * delta_t_yr;

        let u = (1.5 * m_pt).asinh() / 3.0;
        let d_t = 2.0 * u.sinh();

        let pos = p_vec * (q * (1.0 - d_t * d_t)) + q_vec * (2.0 * q * d_t);
        let dd_dt = n_p / (1.0 + d_t * d_t);
        let vel = p_vec * (-2.0 * q * d_t * dd_dt) + q_vec * (2.0 * q * dd_dt);
        return (pos.is_finite() && vel.is_finite()).then_some((pos, vel));
    }

    if e < 1.0 {
        if elements.semi_major_axis <= 0.0 || !elements.semi_major_axis.is_finite() {
            return None;
        }
        let a = elements.semi_major_axis;
        let period = if elements.period_years.is_finite() && elements.period_years > 1e-7 {
            elements.period_years
        } else {
            2.0 * PI * (a * a * a / mu).sqrt()
        };
        let n = 2.0 * PI / period.max(1e-8);

        let denom = (1.0 + e * nu_0.cos()).max(1e-6);
        let cos_e0 = (e + nu_0.cos()) / denom;
        let sin_e0 = (1.0 - e * e).max(0.0).sqrt() * nu_0.sin() / denom;
        let e_0 = sin_e0.atan2(cos_e0);
        let m_0 = e_0 - e * e_0.sin();

        let m_t = m_0 + n * delta_t_yr;
        let e_t = solve_kepler_eccentric(m_t, e);

        let pos =
            p_vec * (a * (e_t.cos() - e)) + q_vec * (a * (1.0 - e * e).max(0.0).sqrt() * e_t.sin());

        let de_dt = n / (1.0 - e * e_t.cos()).max(1e-6);
        let vel = p_vec * (-a * e_t.sin() * de_dt)
            + q_vec * (a * (1.0 - e * e).max(0.0).sqrt() * e_t.cos() * de_dt);

        (pos.is_finite() && vel.is_finite()).then_some((pos, vel))
    } else {
        let a = elements.semi_major_axis.abs().max(1e-5);
        let n_h = (mu / a.powi(3)).sqrt();

        let denom = 1.0 + e * nu_0.cos();
        if denom <= 1e-6 {
            return None;
        }
        let sinh_h0 = (e * e - 1.0).max(0.0).sqrt() * nu_0.sin() / denom;
        let h_0 = sinh_h0.asinh();
        let m_h0 = e * h_0.sinh() - h_0;

        let m_ht = m_h0 + n_h * delta_t_yr;
        let h_t = solve_kepler_hyperbolic(m_ht, e);

        let pos = p_vec * (a * (e - h_t.cosh()))
            + q_vec * (a * (e * e - 1.0).max(0.0).sqrt() * h_t.sinh());

        let dh_dt = n_h / (e * h_t.cosh() - 1.0).max(1e-6);
        let vel = p_vec * (-a * h_t.sinh() * dh_dt)
            + q_vec * (a * (e * e - 1.0).max(0.0).sqrt() * h_t.cosh() * dh_dt);

        (pos.is_finite() && vel.is_finite()).then_some((pos, vel))
    }
}

/// Scans for close encounters along a Keplerian trajectory against a list of target celestial bodies.
pub fn find_closest_encounter(
    source_elements: &OrbitalElements,
    source_radius_au: f64,
    star_mass: f64,
    star_pos: DVec3,
    targets: &[TargetCandidate],
    horizon_yr: f64,
) -> (Vec<Vec3>, Option<PredictedEncounter>) {
    find_closest_encounter_with_mass(
        source_elements,
        source_radius_au,
        0.0,
        star_mass,
        star_pos,
        targets,
        horizon_yr,
    )
}

/// Period-aware encounter scanner factoring optional source mass into the fluid Roche limit
/// and two-body reduced-mass Kepler propagation.
pub fn find_closest_encounter_with_mass(
    source_elements: &OrbitalElements,
    source_radius_au: f64,
    source_mass_solar: f64,
    star_mass: f64,
    star_pos: DVec3,
    targets: &[TargetCandidate],
    horizon_yr: f64,
) -> (Vec<Vec3>, Option<PredictedEncounter>) {
    let mut source_samples = Vec::new();
    let mut trajectory_points = Vec::new();
    let encounter = find_closest_encounter_into(
        source_elements,
        source_radius_au,
        source_mass_solar,
        star_mass,
        star_pos,
        targets,
        horizon_yr,
        &mut source_samples,
        &mut trajectory_points,
    );
    (trajectory_points, encounter)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Buffer-reusing encounter scanner avoids per-frame sample and polyline allocations"
)]
fn find_closest_encounter_into(
    source_elements: &OrbitalElements,
    source_radius_au: f64,
    source_mass_solar: f64,
    star_mass: f64,
    star_pos: DVec3,
    targets: &[TargetCandidate],
    horizon_yr: f64,
    source_samples: &mut Vec<Option<DVec3>>,
    trajectory_points: &mut Vec<Vec3>,
) -> Option<PredictedEncounter> {
    let mut min_period_yr = if source_elements.eccentricity < 0.9999
        && source_elements.period_years.is_finite()
        && source_elements.period_years > 1e-6
    {
        source_elements.period_years
    } else {
        horizon_yr
    };
    for target in targets {
        if target.elements.eccentricity < 0.9999
            && target.elements.period_years.is_finite()
            && target.elements.period_years > 1e-6
            && source_elements.apoapsis + 0.5 >= target.elements.periapsis
            && target.elements.apoapsis + 0.5 >= source_elements.periapsis
        {
            min_period_yr = min_period_yr.min(target.elements.period_years);
        }
    }
    let dt_target = (min_period_yr / 16.0).clamp(horizon_yr / 2000.0, horizon_yr / 100.0);
    let steps = ((horizon_yr / dt_target).ceil() as usize).clamp(100, 2000);
    let dt_step = horizon_yr / (steps as f64);
    let source_mu_mass = star_mass + source_mass_solar.max(0.0);
    source_samples.clear();
    if source_samples.capacity() < steps + 1 {
        source_samples.reserve(steps + 1 - source_samples.len());
    }

    for i in 0..=steps {
        let t = (i as f64) * dt_step;
        let sample = propagate_kepler_position_velocity(source_elements, t, source_mu_mass)
            .map(|(pos, _)| pos);
        source_samples.push(sample);
    }

    build_visual_trajectory_points_into(
        source_elements,
        source_mu_mass,
        star_pos,
        horizon_yr,
        trajectory_points,
    );

    let mut best_encounter: Option<PredictedEncounter> = None;

    for target in targets {
        if let Some(enc) = check_single_target_encounter(
            source_elements,
            source_samples,
            source_radius_au,
            source_mass_solar,
            star_mass,
            star_pos,
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

    best_encounter
}

/// Samples a smooth single-orbit (or finite hyperbolic) forward trajectory arc so short-period
/// orbits never connect multi-orbit jumps as jagged straight polygon chords.
fn build_visual_trajectory_points_into(
    source_elements: &OrbitalElements,
    source_mu_mass: f64,
    star_pos: DVec3,
    horizon_yr: f64,
    points: &mut Vec<Vec3>,
) {
    let is_bound = source_elements.eccentricity < 1.0
        && source_elements.period_years.is_finite()
        && source_elements.period_years > 1e-6;
    let visual_horizon = if is_bound {
        horizon_yr.min(source_elements.period_years * 0.92)
    } else {
        horizon_yr.min(15.0)
    };
    let visual_steps = if is_bound { 192 } else { 144 };
    let dt = visual_horizon / (visual_steps as f64);
    points.clear();
    if points.capacity() < visual_steps + 1 {
        points.reserve(visual_steps + 1 - points.len());
    }

    for i in 0..=visual_steps {
        let t = (i as f64) * dt;
        if let Some((pos, _)) =
            propagate_kepler_position_velocity(source_elements, t, source_mu_mass)
        {
            let world_pos = pos + star_pos;
            points.push(Vec3::new(
                world_pos.x as f32,
                world_pos.y as f32,
                world_pos.z as f32,
            ));
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Single-target close-encounter scanner requires precomputed source positions and orbital parameters"
)]
fn check_single_target_encounter(
    source_elements: &OrbitalElements,
    source_samples: &[Option<DVec3>],
    source_radius_au: f64,
    source_mass_solar: f64,
    star_mass: f64,
    star_pos: DVec3,
    target: &TargetCandidate,
    horizon_yr: f64,
    steps: usize,
) -> Option<PredictedEncounter> {
    let dt_step = horizon_yr / (steps as f64);
    let mut min_dist_au = f64::INFINITY;
    let mut best_t_idx = 0;

    let target_a =
        if target.elements.eccentricity < 1.0 && target.elements.semi_major_axis.is_finite() {
            target.elements.semi_major_axis.max(1e-5)
        } else {
            target.elements.periapsis.max(1e-5)
        };
    let hill_r =
        (target_a * (target.mass_solar / (3.0 * star_mass.max(1e-6))).cbrt()).clamp(1e-5, 5.0);
    // Symmetric mutual Roche limit: consider disruption of either source by target or target by source
    let roche_src = if source_mass_solar > 0.0 && source_radius_au > 0.0 {
        2.44 * source_radius_au * (target.mass_solar / source_mass_solar).cbrt()
    } else {
        0.0
    };
    let roche_tgt = if source_mass_solar > 0.0 && target.mass_solar > 0.0 {
        2.44 * target.radius_au * (source_mass_solar / target.mass_solar).cbrt()
    } else {
        target.radius_au * 2.44
    };
    let roche_r = roche_src.max(roche_tgt).max(target.radius_au);
    let collision_r = target.radius_au + source_radius_au;

    // Pre-reject non-overlapping radial bands (|r_src - r_tgt| >= ||r_src| - |r_tgt||)
    let encounter_margin = hill_r * 2.0;
    if source_elements.apoapsis + encounter_margin < target.elements.periapsis
        || target.elements.apoapsis + encounter_margin < source_elements.periapsis
    {
        return None;
    }

    let source_mu_mass = star_mass + source_mass_solar.max(0.0);
    let target_mu_mass = star_mass + target.mass_solar.max(0.0);

    for (i, opt_src_pos) in source_samples.iter().enumerate().take(steps + 1) {
        let &Some(src_pos) = opt_src_pos else {
            continue;
        };
        let t = (i as f64) * dt_step;
        let Some((tgt_pos, _)) =
            propagate_kepler_position_velocity(&target.elements, t, target_mu_mass)
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

    let refined_t = refine_closest_approach_time(
        source_elements,
        source_mu_mass,
        &target.elements,
        target_mu_mass,
        t_min,
        t_max,
    );

    let (src_pos, src_vel) =
        propagate_kepler_position_velocity(source_elements, refined_t, source_mu_mass)?;
    let (tgt_pos, tgt_vel) =
        propagate_kepler_position_velocity(&target.elements, refined_t, target_mu_mass)?;

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
        encounter_pos_au: src_pos + star_pos,
        target_pos_at_encounter_au: tgt_pos + star_pos,
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
    source_mu_mass: f64,
    target_elements: &OrbitalElements,
    target_mu_mass: f64,
    mut a: f64,
    mut b: f64,
) -> f64 {
    let phi = 0.618_033_988_749_895; // Golden ratio (sqrt(5) - 1) / 2
    let mut c = b - phi * (b - a);
    let mut d = a + phi * (b - a);

    let dist_at = |t: f64| -> f64 {
        let (Some((p1, _)), Some((p2, _))) = (
            propagate_kepler_position_velocity(source_elements, t, source_mu_mass),
            propagate_kepler_position_velocity(target_elements, t, target_mu_mass),
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

#[derive(Default)]
pub struct PredictorCache {
    initialized: bool,
    was_paused: bool,
    throttle_accum: f32,
    last_step_count: u64,
    last_selected: Option<Entity>,
    last_slingshot_active: bool,
    last_slingshot_target: Option<Entity>,
    last_slingshot_archetype: crate::simulation::resources::SlingshotArchetype,
    last_drag_origin: Option<DVec3>,
    last_drag_current: Option<DVec3>,
    last_velocity_scale: f64,
    last_horizon: f64,
    last_star_mass: f64,
    last_star_pos: DVec3,
    last_star_vel: DVec3,
    last_bodies_sig: u64,
    source_samples_buf: Vec<Option<DVec3>>,
}

/// Fast order-independent fingerprint over all predictor bodies so paused edits to pos/vel/mass/radius
/// or spawns/despawns immediately invalidate `PredictorCache`.
fn compute_predictor_bodies_sig(bodies_query: &PredictorBodyQuery<'_, '_>) -> u64 {
    let mut xor_acc = 0u64;
    let mut sum_acc = 0u64;
    let mut count = 0u64;
    for (ent, _, pos, vel, mass, rad) in bodies_query.iter() {
        let mut h = ent.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for v in [
            pos.0.x, pos.0.y, pos.0.z, vel.0.x, vel.0.y, vel.0.z, mass.0, rad.0,
        ] {
            h = h
                .rotate_left(13)
                .wrapping_add(v.to_bits().wrapping_mul(0xBF58_476D_1CE4_E5B9));
        }
        xor_acc ^= h;
        sum_acc = sum_acc.wrapping_add(h);
        count += 1;
    }
    xor_acc ^ sum_acc.rotate_left(29) ^ count.wrapping_mul(0x94D0_49BB_1331_11EB)
}

type PredictorBodyQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CelestialBody,
        &'static SimPosition,
        &'static SimVelocity,
        &'static Mass,
        &'static Radius,
    ),
    (
        Without<CentralStar>,
        Without<SatelliteOf>,
        Without<crate::simulation::probes::SpaceProbe>,
    ),
>;

fn clear_predictor_output(state: &mut TrajectoryPredictorState, cache: &mut PredictorCache) {
    state.trajectory_points.clear();
    state.active_encounter = None;
    state.source_label.clear();
    cache.initialized = false;
}

fn resolve_predictor_source(
    player_state: &PlayerInteractionState,
    slingshot_state: &SlingshotState,
    star_pos: DVec3,
    star_vel: DVec3,
    star_mass: f64,
    bodies_query: &PredictorBodyQuery,
) -> Option<(OrbitalElements, f64, f64, Option<Entity>, String)> {
    if slingshot_state.is_active {
        let (origin_dvec, curr_dvec) =
            (slingshot_state.drag_origin?, slingshot_state.drag_current?);
        let launch_vel = (curr_dvec - origin_dvec) * slingshot_state.velocity_scale;
        if let Some(target_ent) = slingshot_state.target_entity
            && let Ok((_, body, pos, vel, mass, rad)) = bodies_query.get(target_ent)
        {
            let el = state_vectors_to_orbital_elements(
                pos.0 - star_pos,
                (vel.0 + launch_vel) - star_vel,
                star_mass,
                mass.0,
            )?;
            return Some((
                el,
                rad.0,
                mass.0,
                Some(target_ent),
                format!("Impulse ({})", body.name),
            ));
        }
        let proj_mass =
            crate::game::slingshot::archetype_preset_mass_solar(slingshot_state.archetype);
        let el = state_vectors_to_orbital_elements(
            origin_dvec - star_pos,
            launch_vel - star_vel,
            star_mass,
            proj_mass,
        )?;
        return Some((
            el,
            0.0001,
            proj_mass,
            None,
            format!("Slingshot ({:?})", slingshot_state.archetype),
        ));
    }

    let selected_ent = player_state.selected_entity?;
    let (_, body, pos, vel, mass, rad) = bodies_query.get(selected_ent).ok()?;
    let el =
        state_vectors_to_orbital_elements(pos.0 - star_pos, vel.0 - star_vel, star_mass, mass.0)?;
    Some((el, rad.0, mass.0, Some(selected_ent), body.name.clone()))
}

fn collect_predictor_targets(
    bodies_query: &PredictorBodyQuery,
    exclude_ent: Option<Entity>,
    star_pos: DVec3,
    star_vel: DVec3,
    star_mass: f64,
    out: &mut Vec<TargetCandidate>,
) {
    out.clear();
    for (entity, body, pos, vel, mass, rad) in bodies_query {
        if exclude_ent == Some(entity) {
            continue;
        }
        if matches!(
            body.body_type,
            BodyType::Planetesimal
                | BodyType::Asteroid
                | BodyType::Comet
                | BodyType::DustGrain
                | BodyType::DebrisRing
        ) && mass.0 < EARTH_MASS_SOLAR * 0.01
        {
            continue;
        }

        if let Some(el) =
            state_vectors_to_orbital_elements(pos.0 - star_pos, vel.0 - star_vel, star_mass, mass.0)
        {
            out.push(TargetCandidate {
                entity,
                name: String::new(),
                elements: el,
                radius_au: rad.0,
                mass_solar: mass.0,
            });
        }
    }
}

/// ECS system that continuously updates the trajectory predictor for the selected body or active Slingshot.
#[allow(
    clippy::too_many_arguments,
    reason = "Trajectory predictor system requires querying multiple celestial body components"
)]
pub fn update_trajectory_predictor(
    mut predictor_state: ResMut<TrajectoryPredictorState>,
    player_state: Res<PlayerInteractionState>,
    slingshot_state: Res<SlingshotState>,
    time_warp: Option<Res<TimeWarp>>,
    sim_time: Option<Res<SimTime>>,
    time: Option<Res<Time>>,
    star_query: Query<(&SimPosition, &SimVelocity, &Mass), With<CentralStar>>,
    bodies_query: PredictorBodyQuery,
    mut target_candidates: Local<Vec<TargetCandidate>>,
    mut cache: Local<PredictorCache>,
) {
    if !predictor_state.is_enabled {
        clear_predictor_output(&mut predictor_state, &mut cache);
        return;
    }

    let Ok((star_pos, star_vel, star_mass)) = star_query.single() else {
        clear_predictor_output(&mut predictor_state, &mut cache);
        return;
    };

    let is_paused = time_warp
        .as_deref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once);
    let dt = time.as_deref().map_or(0.0, Time::delta_secs);
    let step_count = sim_time.as_deref().map_or(0, |s| s.step_count);
    let bodies_sig = if is_paused {
        compute_predictor_bodies_sig(&bodies_query)
    } else {
        0
    };
    let static_inputs_unchanged = cache.initialized
        && cache.last_selected == player_state.selected_entity
        && cache.last_slingshot_active == slingshot_state.is_active
        && cache.last_slingshot_target == slingshot_state.target_entity
        && cache.last_slingshot_archetype == slingshot_state.archetype
        && cache.last_drag_origin == slingshot_state.drag_origin
        && cache.last_drag_current == slingshot_state.drag_current
        && (cache.last_velocity_scale - slingshot_state.velocity_scale).abs() < 1e-12
        && (cache.last_horizon - predictor_state.forecast_horizon_yr).abs() < 1e-12
        && (cache.last_star_mass - star_mass.0).abs() < 1e-12;
    let paused_unchanged = static_inputs_unchanged
        && cache.was_paused
        && cache.last_step_count == step_count
        && cache.last_star_pos == star_pos.0
        && cache.last_star_vel == star_vel.0
        && cache.last_bodies_sig == bodies_sig;
    if is_paused && paused_unchanged {
        return;
    }
    if !is_paused && dt > 0.0 && static_inputs_unchanged && !cache.was_paused {
        cache.throttle_accum += dt;
        if cache.throttle_accum < 0.125 {
            return;
        }
    }
    cache.throttle_accum = 0.0;

    let selected_source = resolve_predictor_source(
        &player_state,
        &slingshot_state,
        star_pos.0,
        star_vel.0,
        star_mass.0,
        &bodies_query,
    );

    cache.initialized = true;
    cache.was_paused = is_paused;
    cache.last_step_count = step_count;
    cache.last_selected = player_state.selected_entity;
    cache.last_slingshot_active = slingshot_state.is_active;
    cache.last_slingshot_target = slingshot_state.target_entity;
    cache.last_slingshot_archetype = slingshot_state.archetype;
    cache.last_drag_origin = slingshot_state.drag_origin;
    cache.last_drag_current = slingshot_state.drag_current;
    cache.last_velocity_scale = slingshot_state.velocity_scale;
    cache.last_horizon = predictor_state.forecast_horizon_yr;
    cache.last_star_mass = star_mass.0;
    cache.last_star_pos = star_pos.0;
    cache.last_star_vel = star_vel.0;
    cache.last_bodies_sig = bodies_sig;

    let Some((src_elements, src_radius, src_mass, exclude_ent, src_name)) = selected_source else {
        predictor_state.trajectory_points.clear();
        predictor_state.active_encounter = None;
        predictor_state.source_label.clear();
        return;
    };

    predictor_state.source_label = src_name;
    collect_predictor_targets(
        &bodies_query,
        exclude_ent,
        star_pos.0,
        star_vel.0,
        star_mass.0,
        &mut target_candidates,
    );

    let horizon_yr = predictor_state.forecast_horizon_yr;
    let mut trajectory_points = std::mem::take(&mut predictor_state.trajectory_points);
    let mut encounter = find_closest_encounter_into(
        &src_elements,
        src_radius,
        src_mass,
        star_mass.0,
        star_pos.0,
        &target_candidates,
        horizon_yr,
        &mut cache.source_samples_buf,
        &mut trajectory_points,
    );

    if let Some(ref mut enc) = encounter
        && enc.target_name.is_empty()
        && let Ok((_, body, _, _, _, _)) = bodies_query.get(enc.target_entity)
    {
        enc.target_name.clone_from(&body.name);
    }

    predictor_state.trajectory_points = trajectory_points;
    predictor_state.active_encounter = encounter;
}
