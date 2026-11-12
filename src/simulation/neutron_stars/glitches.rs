//! Physical modeling of pulsar magnetic dipole braking, vortex pinning lag,
//! and superfluid unpinning glitches with exponential post-glitch recovery.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::types::*;

/// Speed of light in vacuum in $\text{cm/s}$.
pub const C_LIGHT_CGS: f64 = 2.997_924_58e10;

/// Standard neutron star moment of inertia in $\text{g}\cdot\text{cm}^2$ ($I \approx 10^{45}\text{ g}\cdot\text{cm}^2$).
pub const NEUTRON_STAR_INERTIA_CGS: f64 = 1.0e45;

/// Computes the theoretical magnetic dipole braking spin-down rate $\dot{\nu}$ in $\text{Hz/s}$:
/// $$\dot{\nu} = - \frac{8\pi^2 B^2 R^6 \nu^3}{3 c^3 I}$$
pub fn evaluate_dipole_spin_down_rate(
    magnetic_field_gauss: f64,
    radius_km: f64,
    frequency_hz: f64,
) -> f64 {
    if frequency_hz <= 0.0 || magnetic_field_gauss <= 0.0 || radius_km <= 0.0 {
        return 0.0;
    }

    let r_cm = radius_km * 1.0e5;
    let b = magnetic_field_gauss;
    let nu = frequency_hz;

    let b2 = b * b;
    let r6 = r_cm.powi(6);
    let nu3 = nu * nu * nu;

    let c3 = C_LIGHT_CGS * C_LIGHT_CGS * C_LIGHT_CGS;
    let numerator = 8.0 * std::f64::consts::PI * std::f64::consts::PI * b2 * r6 * nu3;
    let denominator = 3.0 * c3 * NEUTRON_STAR_INERTIA_CGS;

    -(numerator / denominator).min(1.0)
}

/// Triggers an instantaneous vortex unpinning glitch on a pulsar state, transferring
/// superfluid angular momentum into the crystalline crust.
pub fn trigger_instantaneous_glitch(
    state: &mut PulsarGlitchState,
    current_sim_years: f64,
) -> (f64, f64, f64) {
    let before_hz = state.spin_frequency_hz;
    let jump_hz = (before_hz * state.glitch_jump_fraction).max(1e-8);
    let after_hz = before_hz + jump_hz;

    state.spin_frequency_hz = after_hz;
    state.last_glitch_jump_hz = jump_hz;
    state.vortex_pinning_lag_hz = 0.0;
    state.total_glitches_occurred += 1;
    state.last_glitch_epoch_years = current_sim_years;
    state.time_since_glitch_days = 0.0;
    state.is_relaxing = true;

    (before_hz, after_hz, jump_hz)
}

/// Advances post-glitch exponential relaxation and continuous dipole spin-down.
pub fn step_single_pulsar_glitch(
    state: &mut PulsarGlitchState,
    opt_spin: Option<&mut SpinState>,
    opt_em: Option<&mut ElectromagneticFieldState>,
    dt_sec: f64,
) {
    let dt_days = dt_sec / 86400.0;

    // 1. Continuous dipole spin-down braking
    let delta_nu_braking = state.spin_down_rate_hz_s * dt_sec;
    state.spin_frequency_hz = (state.spin_frequency_hz + delta_nu_braking).max(0.1);

    // 2. Accumulate vortex lag as the crust slows down faster than the interior superfluid
    state.vortex_pinning_lag_hz += -delta_nu_braking;

    // 3. Post-glitch exponential relaxation of the decoupled superfluid component:
    //    \Delta \nu(t) = Q \Delta \nu_0 * \exp(-t / \tau)
    if state.is_relaxing {
        state.time_since_glitch_days += dt_days;
        let tau = state.post_glitch_relaxation_time_days.max(1.0);
        let t = state.time_since_glitch_days;

        let decay_rate =
            (state.relaxation_healing_q * state.last_glitch_jump_hz / tau) * (-t / tau).exp();
        let relaxation_drop = decay_rate * dt_days;
        state.spin_frequency_hz = (state.spin_frequency_hz - relaxation_drop).max(0.1);

        if t > 5.0 * tau {
            state.is_relaxing = false;
        }
    }

    // 4. Synchronize physical rotation periods across ECS components
    let period_sec = 1.0 / state.spin_frequency_hz;
    if let Some(spin) = opt_spin {
        spin.rotation_period_hours = period_sec / 3600.0;
    }
    if let Some(em) = opt_em {
        em.rotation_period_sec = period_sec;
    }
}

/// Dynamic ECS system driving pulsar glitches, vortex unpinning triggers, and post-glitch timing.
#[allow(
    clippy::type_complexity,
    reason = "Bevy ECS query requires compact remnant kinematic and electromagnetic components"
)]
pub fn step_pulsar_glitches_system(
    time_warp: Option<Res<TimeWarp>>,
    sim_time: Option<Res<SimTime>>,
    mut manual_triggers: MessageReader<TriggerManualGlitchOrStarquakeEvent>,
    mut glitch_events: MessageWriter<PulsarGlitchEvent>,
    mut pulsars_query: Query<(
        Entity,
        &mut PulsarGlitchState,
        Option<&mut SpinState>,
        Option<&mut ElectromagneticFieldState>,
    )>,
) {
    if let Some(ref tw) = time_warp {
        if tw.is_paused && !tw.step_once {
            return;
        }
    }

    let dt_years = sim_time
        .as_ref()
        .map_or(0.016, |s| s.current_dt_yr.max(1e-6));
    let elapsed_years = sim_time.as_ref().map_or(0.0, |s| s.elapsed_years);
    let dt_sec = dt_years * 31_557_600.0;

    // Check for manual trigger requests
    let mut manual_entities = Vec::new();
    let mut trigger_all = false;
    for ev in manual_triggers.read() {
        if let Some(ent) = ev.target_entity {
            manual_entities.push(ent);
        } else {
            trigger_all = true;
        }
    }

    for (entity, mut state, mut opt_spin, mut opt_em) in pulsars_query.iter_mut() {
        let is_manual = trigger_all || manual_entities.contains(&entity);
        let exceeds_threshold = state.vortex_pinning_lag_hz >= state.unpinning_threshold_hz;

        if is_manual || exceeds_threshold {
            let (before, after, jump) = trigger_instantaneous_glitch(&mut state, elapsed_years);
            glitch_events.write(PulsarGlitchEvent {
                entity,
                frequency_before_hz: before,
                frequency_after_hz: after,
                frequency_jump_hz: jump,
            });
        }

        step_single_pulsar_glitch(
            &mut state,
            opt_spin.as_deref_mut(),
            opt_em.as_deref_mut(),
            dt_sec,
        );
    }
}
