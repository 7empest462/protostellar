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

/// Evaluates the secular change in eccentricity per orbit induced by Galactic tides:
/// $$\frac{de}{dt} \approx \frac{15}{8} \frac{\Omega_{\text{tide}}^2}{\omega_{\text{orb}}} e \sqrt{1 - e^2} \sin(2\omega) \sin^2(i_{\text{gal}})$$
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

    (15.0 / 8.0) * (omega_sq / mean_motion) * eccentricity * (1.0 - e2).sqrt() * sin_2omega * sin2_i
}

struct CometStepResult {
    is_infall: bool,
    is_grazer: bool,
    is_sublimating: bool,
}

fn update_single_oort_comet(
    entity: Entity,
    pos: DVec3,
    vel: &mut DVec3,
    mass_solar: f64,
    star_pos: DVec3,
    star_mass: f64,
    dt_years: f64,
    config: &OortCloudConfig,
    eff_scale: f64,
    comet: &mut OortComet,
    opt_comp: Option<&mut Composition>,
    tide_state: &mut GalacticTideState,
    infall_events: &mut MessageWriter<OortCometInfallEvent>,
) -> CometStepResult {
    let r_vec = pos - star_pos;
    let dist_au = r_vec.length();

    // 1. Apply Galactic tidal acceleration
    let a_tide = evaluate_galactic_tide_acceleration(
        pos,
        star_pos,
        config.galactic_pole_dir,
        config.galactic_rho_0,
        eff_scale,
    );
    *vel += a_tide * dt_years;

    // 2. Track current Keplerian orbital elements
    let (a, e, q) =
        if let Some(orb) = state_vectors_to_orbital_elements(r_vec, *vel, star_mass, mass_solar) {
            let a_val = orb.semi_major_axis;
            let e_val = orb.eccentricity.clamp(0.0, 0.99999);
            let q_val = if a_val.is_finite() && a_val > 0.0 {
                a_val * (1.0 - e_val)
            } else {
                dist_au
            };
            comet.inclination_rad = orb.inclination;
            comet.argument_of_periapsis_rad = orb.argument_of_periapsis;
            (a_val, e_val, q_val)
        } else {
            (dist_au, 0.7, dist_au)
        };

    comet.semi_major_axis_au = a;
    comet.eccentricity = e;
    comet.current_perihelion_au = q;

    // 3. Detect comets deflected into the planetary system
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

    // 4. Volatile ice sublimation when approaching inner solar system
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

        if loss_kg > 1.0e12 {
            comet.volatile_ice_fraction = (comet.volatile_ice_fraction - 1e-5).max(0.01);
            if let Some(comp) = opt_comp {
                comp.ice_frac = (comp.ice_frac - 1e-5).max(0.01);
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
    star_query: Query<(&SimPosition, &Mass, &CelestialBody)>,
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

    if let Some(ref tw) = time_warp {
        if tw.is_paused && !tw.step_once {
            return;
        }
    }

    let star_entry = star_query
        .iter()
        .find(|(_, _, b)| b.body_type.is_star_or_remnant());
    let (star_pos, star_mass) = if let Some((s_pos, s_mass, _)) = star_entry {
        (s_pos.0, s_mass.0)
    } else {
        (DVec3::ZERO, 1.0)
    };

    let dt_years = sim_time
        .as_ref()
        .map_or(0.016, |s| s.current_dt_yr.max(1e-4));
    if dt_years <= 0.0 {
        return;
    }

    let eff_scale = config.tide_strength_scale * config.secular_accel_factor;

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
            star_mass,
            dt_years,
            &config,
            eff_scale,
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

/// Applies an impulsive velocity deflection from a passing field star to all Oort comets.
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

    // Impulsive velocity perturbation delta_v = 2 * G * M_star / (b * v_rel)
    let delta_v_mag = (2.0 * mu) / (b * v_rel);

    for (pos, vel) in comets.iter_mut() {
        let r_vec = pos.0 - star_pos;
        let r_norm = r_vec.normalize_or_zero();
        // Scatter along direction perpendicular to flyby track
        let scatter_vec = (r_norm.cross(dir)).cross(dir).normalize_or_zero();
        vel.0 += scatter_vec * delta_v_mag;
    }
}
