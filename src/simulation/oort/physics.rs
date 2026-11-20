//! Astrophysical calculations for Galactic tidal torque, Kozai cycles, and cometary sublimation.

use std::f64::consts::PI;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::G_ASTRO;
use crate::utils::math::state_vectors_to_orbital_elements;

use super::types::*;

/// Astronomical conversion factor: 1 parsec in Astronomical Units (AU).
pub const PARSEC_TO_AU: f64 = 206_264.806_247;

/// Computes the square of the Galactic tidal vertical oscillation frequency:
/// $\Omega_{\text{tide}}^2 = 4\pi G \rho_0$ in units of $\text{yr}^{-2}$.
pub fn compute_galactic_omega_sq(rho_0_msun_pc3: f64) -> f64 {
    let pc3_in_au3 = PARSEC_TO_AU * PARSEC_TO_AU * PARSEC_TO_AU;
    let rho_au3 = rho_0_msun_pc3 / pc3_in_au3;
    4.0 * PI * G_ASTRO * rho_au3
}

/// Evaluates the vertical Galactic tidal acceleration acting on a body at `pos` relative to the star at `star_pos`.
///
/// The acceleration directed toward the Galactic midplane is:
/// $$\vec{a}_{\text{tide}} = - \Omega_{\text{tide}}^2 \, (\vec{r} \cdot \hat{n}_{\text{gal}}) \, \hat{n}_{\text{gal}}$$
pub fn evaluate_galactic_tide_acceleration(
    pos: DVec3,
    star_pos: DVec3,
    galactic_pole_dir: DVec3,
    rho_0_msun_pc3: f64,
    scale: f64,
) -> DVec3 {
    let r_vec = pos - star_pos;
    let pole_hat = galactic_pole_dir.normalize_or_zero();
    if pole_hat == DVec3::ZERO {
        return DVec3::ZERO;
    }

    let z_gal = r_vec.dot(pole_hat);
    let omega_sq = compute_galactic_omega_sq(rho_0_msun_pc3);
    -omega_sq * scale * z_gal * pole_hat
}

/// Evaluates the secular change in eccentricity induced by planar Galactic disk tides
/// (Heisler & Tremaine 1986):
/// $$\frac{de}{dt} \approx \frac{5}{4} \frac{\Omega_{\text{tide}}^2}{n} e \sqrt{1 - e^2} \sin(2\omega_{\text{gal}}) \sin^2(i_{\text{gal}})$$
pub fn evaluate_galactic_tide_secular_de(
    semi_major_axis_au: f64,
    eccentricity: f64,
    argument_of_periapsis_rad: f64,
    galactic_inclination_rad: f64,
    star_mass_solar: f64,
    rho_0_msun_pc3: f64,
) -> f64 {
    if semi_major_axis_au <= 0.0 || eccentricity <= 0.0 || eccentricity >= 1.0 {
        return 0.0;
    }

    let mu = G_ASTRO * star_mass_solar.max(0.1);
    let mean_motion = (mu / (semi_major_axis_au * semi_major_axis_au * semi_major_axis_au)).sqrt();
    let omega_sq = compute_galactic_omega_sq(rho_0_msun_pc3);

    let e2 = eccentricity * eccentricity;
    let sin_2omega = (2.0 * argument_of_periapsis_rad).sin();
    let sin2_i = galactic_inclination_rad.sin().powi(2);

    (5.0 / 4.0) * (omega_sq / mean_motion) * eccentricity * (1.0 - e2).sqrt() * sin_2omega * sin2_i
}

/// Projects orbital angular momentum and eccentricity vectors into the Galactic pole frame
/// returning `(i_gal_rad, omega_gal_rad)`.
fn compute_galactic_elements(
    rel_pos: DVec3,
    rel_vel: DVec3,
    mu: f64,
    galactic_pole_dir: DVec3,
    fallback_i: f64,
    fallback_w: f64,
) -> (f64, f64) {
    let n_gal = galactic_pole_dir.normalize_or_zero();
    let h_vec = rel_pos.cross(rel_vel);
    let h = h_vec.length();
    let r = rel_pos.length();
    if n_gal == DVec3::ZERO || h <= 1e-12 || r <= 1e-9 || mu <= 1e-12 {
        return (fallback_i, fallback_w);
    }
    let h_hat = h_vec / h;
    let i_gal = (h_hat.dot(n_gal)).clamp(-1.0, 1.0).acos();
    let e_vec = rel_vel.cross(h_vec) / mu - rel_pos / r;
    let e_len = e_vec.length();
    if e_len <= 1e-6 {
        return (i_gal, fallback_w);
    }
    let node_gal = n_gal.cross(h_hat);
    let node_len = node_gal.length();
    if node_len <= 1e-8 {
        return (i_gal, fallback_w);
    }
    let node_hat = node_gal / node_len;
    let e_hat = e_vec / e_len;
    let cos_w = node_hat.dot(e_hat).clamp(-1.0, 1.0);
    let sin_w = node_hat.cross(e_hat).dot(h_hat);
    (i_gal, sin_w.atan2(cos_w).rem_euclid(2.0 * PI))
}

struct CometStepResult {
    is_infall: bool,
    is_grazer: bool,
    is_sublimating: bool,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Oort comet evolution integrates Galactic tides, Kozai-Heisler torque, and sublimation"
)]
fn update_single_oort_comet(
    entity: Entity,
    pos: DVec3,
    vel: &mut DVec3,
    mass_solar: f64,
    star_pos: DVec3,
    star_vel: DVec3,
    star_mass: f64,
    dt_years: f64,
    config: &OortCloudConfig,
    comet: &mut OortComet,
    opt_comp: Option<&mut Composition>,
    tide_state: &mut GalacticTideState,
    infall_events: &mut MessageWriter<OortCometInfallEvent>,
) -> CometStepResult {
    let r_vec = pos - star_pos;
    let dist_au = r_vec.length();

    // 1. Apply physical Cartesian Galactic tidal acceleration (scale = tide_strength_scale, NOT secular_accel_factor)
    let a_tide = evaluate_galactic_tide_acceleration(
        pos,
        star_pos,
        config.galactic_pole_dir,
        config.galactic_rho_0,
        config.tide_strength_scale,
    );
    *vel += a_tide * dt_years;

    // 2. Apply accelerated Heisler-Tremaine secular orbital-element torque (conserving energy/a!)
    let mut rel_vel = *vel - star_vel;
    let mu = G_ASTRO * (star_mass + mass_solar).max(1e-9);
    if config.secular_accel_factor > 1.0
        && let Some(orb_pre) =
            state_vectors_to_orbital_elements(r_vec, rel_vel, star_mass, mass_solar)
        && orb_pre.eccentricity < 0.9999
        && orb_pre.semi_major_axis.is_finite()
        && orb_pre.semi_major_axis > 1e-4
        && dist_au > 1e-6
    {
        let (i_gal, omega_gal) = compute_galactic_elements(
            r_vec,
            rel_vel,
            mu,
            config.galactic_pole_dir,
            orb_pre.inclination,
            orb_pre.argument_of_periapsis,
        );
        let de_dt = evaluate_galactic_tide_secular_de(
            orb_pre.semi_major_axis,
            orb_pre.eccentricity,
            omega_gal,
            i_gal,
            star_mass,
            config.galactic_rho_0,
        );
        let sec_dt = dt_years * (config.secular_accel_factor - 1.0) * config.tide_strength_scale;
        let new_e = (orb_pre.eccentricity + de_dt * sec_dt).clamp(0.01, 0.9995);
        let target_h2 = mu * orb_pre.semi_major_axis * (1.0 - new_e * new_e).max(1e-4);
        let v_sq = rel_vel.length_squared();
        let target_vt_sq = (target_h2 / (dist_au * dist_au)).min(v_sq);
        let r_hat = r_vec / dist_au;
        let vr = rel_vel.dot(r_hat);
        let vt_vec = rel_vel - r_hat * vr;
        let vt_mag = vt_vec.length();
        if vt_mag > 1e-9 && v_sq > 1e-12 {
            let new_vt = target_vt_sq.sqrt();
            let new_vr_mag = (v_sq - target_vt_sq).max(0.0).sqrt();
            let new_vr_sign = if vr.abs() > 1e-10 { vr.signum() } else { 1.0 };
            let updated_rel_v = (vt_vec / vt_mag) * new_vt + r_hat * (new_vr_sign * new_vr_mag);
            if updated_rel_v.is_finite() {
                rel_vel = updated_rel_v;
                *vel = star_vel + rel_vel;
            }
        }
    }

    // 3. Track current Keplerian orbital elements relative to the host star
    let (a, e, q) = if let Some(orb) =
        state_vectors_to_orbital_elements(r_vec, rel_vel, star_mass, mass_solar)
    {
        let a_val = orb.semi_major_axis;
        let e_val = orb.eccentricity.clamp(0.0, 0.99999);
        let q_val = orb.periapsis;
        comet.inclination_rad = orb.inclination;
        comet.argument_of_periapsis_rad = orb.argument_of_periapsis;
        (a_val, e_val, q_val)
    } else {
        (dist_au, 0.7, dist_au)
    };

    comet.semi_major_axis_au = a;
    comet.eccentricity = e;
    comet.current_perihelion_au = q;

    // 4. Detect comets deflected into the planetary system
    let is_infall = q <= config.sublimation_distance_au;
    let is_grazer = q <= config.sungrazer_threshold_au;

    if is_infall && !comet.is_infall_trajectory {
        comet.is_infall_trajectory = true;
        tide_state.cumulative_infall_events += 1;
        infall_events.write(OortCometInfallEvent {
            entity,
            perihelion_au: q,
            eccentricity: e,
            semi_major_axis_au: a,
            is_sungrazer: is_grazer,
        });
    }
    comet.is_sungrazer = is_grazer;

    // 5. Volatile ice sublimation when approaching inner solar system
    let is_sublimating = dist_au <= config.sublimation_distance_au;
    step_cometary_sublimation(comet, opt_comp, dist_au, dt_years, is_sublimating);

    CometStepResult {
        is_infall,
        is_grazer,
        is_sublimating,
    }
}

fn step_cometary_sublimation(
    comet: &mut OortComet,
    opt_comp: Option<&mut Composition>,
    dist_au: f64,
    dt_years: f64,
    is_sublimating: bool,
) {
    if is_sublimating {
        comet.is_active_sublimating = true;
        let insolation = (1.0 / dist_au.max(0.05)).powi(2) as f32;
        let rate = (1.5e5 * insolation * comet.volatile_ice_fraction).max(10.0);
        comet.outgassing_rate = rate;

        let dt_sec = dt_years * 31_557_600.0;
        let loss_kg = f64::from(rate) * dt_sec;
        comet.cumulative_volatile_loss_kg += loss_kg;

        let delta_ice = ((loss_kg / 1.0e17) as f32).clamp(0.0, 5e-4);
        if delta_ice > 0.0 {
            comet.volatile_ice_fraction = (comet.volatile_ice_fraction - delta_ice).max(0.01);
            if let Some(comp) = opt_comp {
                comp.ice_frac = (comp.ice_frac - f64::from(delta_ice)).max(0.01);
                *comp = comp.normalized();
            }
        }
    } else {
        comet.is_active_sublimating = false;
        comet.outgassing_rate = 0.0;
    }
}

/// Updates Oort cometary trajectories under Galactic tidal torque, records orbital elements,
/// and activates volatile outgassing for comets approaching perihelion.
#[allow(
    clippy::type_complexity,
    reason = "Bevy ECS query requires cometary trajectory and composition components"
)]
pub fn step_oort_galactic_tides_system(
    time_warp: Option<Res<TimeWarp>>,
    sim_time: Option<Res<SimTime>>,
    config: Res<OortCloudConfig>,
    mut tide_state: ResMut<GalacticTideState>,
    mut infall_events: MessageWriter<OortCometInfallEvent>,
    star_query: Query<(&SimPosition, &SimVelocity, &Mass, &CelestialBody), Without<OortComet>>,
    mut comets_query: Query<(
        Entity,
        &SimPosition,
        &mut SimVelocity,
        &Mass,
        &mut OortComet,
        Option<&mut Composition>,
    )>,
) {
    if !config.enabled {
        return;
    }

    if let Some(ref tw) = time_warp
        && tw.is_paused
        && !tw.step_once
    {
        return;
    }

    let star_entry = star_query
        .iter()
        .find(|(_, _, _, b)| b.body_type.is_star_or_remnant());
    let (star_pos, star_vel, star_mass) = if let Some((s_pos, s_vel, s_mass, _)) = star_entry {
        (s_pos.0, s_vel.0, s_mass.0)
    } else {
        (DVec3::ZERO, DVec3::ZERO, 1.0)
    };

    let dt_years = sim_time.as_ref().map_or(0.016, |s| {
        if s.current_dt_yr > 0.0 {
            s.current_dt_yr
        } else {
            1e-4
        }
    });
    if dt_years <= 0.0 {
        return;
    }

    let mut total_count = 0usize;
    let mut infalling_count = 0usize;
    let mut sublimating_count = 0usize;
    let mut sungrazing_count = 0usize;

    for (entity, pos, mut vel, mass, mut comet, opt_comp) in comets_query.iter_mut() {
        total_count += 1;
        let res = update_single_oort_comet(
            entity,
            pos.0,
            &mut vel.0,
            mass.0,
            star_pos,
            star_vel,
            star_mass,
            dt_years,
            &config,
            &mut comet,
            opt_comp.map(Mut::into_inner),
            &mut tide_state,
            &mut infall_events,
        );

        if res.is_infall {
            infalling_count += 1;
        }
        if res.is_sublimating {
            sublimating_count += 1;
        }
        if res.is_grazer {
            sungrazing_count += 1;
        }
    }

    tide_state.total_oort_bodies = total_count;
    tide_state.infalling_comets_count = infalling_count;
    tide_state.active_sublimating_count = sublimating_count;
    tide_state.sungrazing_count = sungrazing_count;
}

/// Applies a differential tidal impulse perturbation from a passing field star to Oort comets
/// ($\Delta\vec{v} = \Delta\vec{v}_{\text{comet}} - \Delta\vec{v}_{\text{star}}$).
pub fn apply_stellar_flyby_perturbation(
    comets: &mut [(&mut SimPosition, &mut SimVelocity)],
    star_pos: DVec3,
    flyby_mass_solar: f64,
    flyby_impact_param_au: f64,
    flyby_velocity_au_yr: f64,
    flyby_direction: DVec3,
) {
    let b = flyby_impact_param_au.max(100.0);
    let v_rel = flyby_velocity_au_yr.max(1.0);
    let mu = G_ASTRO * flyby_mass_solar;
    let dir = flyby_direction.normalize_or_zero();
    if dir == DVec3::ZERO {
        return;
    }
    let ortho = dir.any_orthonormal_vector();
    let b_star_vec = ortho * b;
    let b_star_sq = b_star_vec.length_squared().max(2500.0);
    let dv_star = -(2.0 * mu / v_rel) * (b_star_vec / b_star_sq);

    for (pos, vel) in comets.iter_mut() {
        let r_vec = pos.0 - star_pos;
        let r_perp = r_vec - dir * r_vec.dot(dir);
        let b_comet_vec = b_star_vec - r_perp;
        let b_comet_sq = b_comet_vec.length_squared().max(2500.0);
        let dv_comet = -(2.0 * mu / v_rel) * (b_comet_vec / b_comet_sq);
        vel.0 += dv_comet - dv_star;
    }
}
