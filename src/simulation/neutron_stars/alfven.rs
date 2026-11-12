//! Physical modeling of magnetar crustal magnetic shear strain accumulation,
//! starquakes (crust fractures), giant flares, and relativistic propagating Alfvén shock waves.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::types::*;

/// Speed of light in Astronomical Units per year ($c \approx 63{,}241.077\text{ AU/yr}$).
pub const C_LIGHT_AU_YR: f64 = 63_241.077;

/// Canonical crystalline neutron star crust shear modulus in $\text{erg/cm}^3$ ($\mu \approx 10^{30}\text{ erg/cm}^3$).
pub const CRUST_SHEAR_MODULUS_CGS: f64 = 1.0e30;

/// Standard crust volume in $\text{cm}^3$ ($V \approx 4\pi R^2 \Delta R \approx 1.25 \times 10^{18}\text{ cm}^3$).
pub const CRUST_VOLUME_CGS: f64 = 1.25e18;

/// Computes the mechanical and magnetic shear strain energy released during a crust rupture:
/// $$E_{\text{quake}} = \frac{1}{2} \mu_{\text{crust}} \theta_{\text{strain}}^2 V_{\text{crust}}$$
pub fn evaluate_starquake_energy_erg(strain: f64) -> f64 {
    let s = strain.clamp(0.01, 0.20);
    0.5 * CRUST_SHEAR_MODULUS_CGS * s * s * CRUST_VOLUME_CGS
}

/// Triggers a magnetar crust fracture, sets burst parameters, and returns released energy and flare luminosity.
pub fn trigger_instantaneous_starquake(
    stress: &mut MagnetarCrustStress,
    opt_lum: Option<&mut Luminosity>,
    opt_em: Option<&mut ElectromagneticFieldState>,
) -> (f64, f64) {
    let energy_erg = evaluate_starquake_energy_erg(stress.accumulated_magnetic_strain);

    // Peak giant flare luminosity: L ~ 10^7 to 10^10 L_sun (e.g. SGR 1806-20 giant flare Dec 2004)
    let flare_solar = (energy_erg / 1.0e39).clamp(1.0e6, 5.0e9);

    stress.last_starquake_energy_erg = energy_erg;
    stress.peak_flare_luminosity_solar = flare_solar;
    stress.accumulated_magnetic_strain = 0.012; // Plastic stress drop
    stress.burst_active = true;
    stress.burst_cooldown_years = 0.35;
    stress.total_starquakes_count += 1;

    if let Some(lum) = opt_lum {
        lum.0 = flare_solar;
    }
    if let Some(em) = opt_em {
        em.synchrotron_intensity = 15.0; // Dazzling synchrotron fireball
    }

    (energy_erg, flare_solar)
}

/// Dynamic system modeling magnetar magnetic stress accumulation, rupture starquakes,
/// and launching relativistic Alfvén blast waves into the circumstellar environment.
#[allow(
    clippy::type_complexity,
    reason = "Bevy ECS query requires magnetar kinematics, stress, and electromagnetic state"
)]
pub fn step_magnetar_crust_stress_and_starquakes_system(
    mut commands: Commands,
    time_warp: Option<Res<TimeWarp>>,
    sim_time: Option<Res<SimTime>>,
    mut manual_triggers: MessageReader<TriggerManualGlitchOrStarquakeEvent>,
    mut starquake_events: MessageWriter<MagnetarStarquakeEvent>,
    mut magnetars_query: Query<(
        Entity,
        &SimPosition,
        &mut MagnetarCrustStress,
        Option<&mut Luminosity>,
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

    let mut manual_entities = Vec::new();
    let mut trigger_all = false;
    for ev in manual_triggers.read() {
        if let Some(ent) = ev.target_entity {
            manual_entities.push(ent);
        } else {
            trigger_all = true;
        }
    }

    for (entity, pos, mut stress, mut opt_lum, mut opt_em) in magnetars_query.iter_mut() {
        let is_manual = trigger_all || manual_entities.contains(&entity);

        // 1. Accumulate magnetic shear strain
        stress.accumulated_magnetic_strain += stress.magnetic_twist_rate_per_year * dt_years;

        let should_rupture =
            is_manual || stress.accumulated_magnetic_strain >= stress.crust_yield_strain;

        if should_rupture {
            let (energy_erg, flare_solar) = trigger_instantaneous_starquake(
                &mut stress,
                opt_lum.as_deref_mut(),
                opt_em.as_deref_mut(),
            );

            starquake_events.write(MagnetarStarquakeEvent {
                entity,
                energy_erg,
                peak_luminosity_solar: flare_solar,
                epicenter_au: pos.0,
            });

            // Spawn expanding relativistic shear Alfvén blast wave shock front
            commands.spawn((
                AlfvenBlastWave {
                    source_entity: entity,
                    origin_au: pos.0,
                    radius_au: 0.05,
                    expansion_speed_au_yr: C_LIGHT_AU_YR * 0.999, // ~0.999 c
                    energy_erg,
                    luminosity_solar: flare_solar,
                    synchrotron_brightness: 1.0,
                    age_seconds: 0.0,
                    max_radius_au: 60.0,
                },
                SimPosition(pos.0),
            ));
        }

        // 2. Flare cooldown and lightcurve decay
        if stress.burst_active {
            stress.burst_cooldown_years -= dt_years;
            let decay_factor = (stress.burst_cooldown_years / 0.35).clamp(0.0, 1.0);

            if let Some(ref mut lum) = opt_lum {
                lum.0 =
                    10_000.0 + stress.peak_flare_luminosity_solar * (decay_factor * decay_factor);
            }
            if let Some(ref mut em) = opt_em {
                em.synchrotron_intensity = (15.0 * decay_factor as f32).max(1.0);
            }

            if stress.burst_cooldown_years <= 0.0 {
                stress.burst_active = false;
                stress.peak_flare_luminosity_solar = 0.0;
            }
        }
    }
}

/// Propagates relativistic Alfvén blast waves outward at $v_A \approx c$, dims radiant flux
/// by inverse-square geometric expansion, and despawns dispersed waves.
pub fn step_alfven_blast_waves_system(
    mut commands: Commands,
    time_warp: Option<Res<TimeWarp>>,
    sim_time: Option<Res<SimTime>>,
    mut blast_waves_query: Query<(Entity, &mut AlfvenBlastWave, Option<&mut SimPosition>)>,
) {
    if let Some(ref tw) = time_warp {
        if tw.is_paused && !tw.step_once {
            return;
        }
    }

    let dt_years = sim_time
        .as_ref()
        .map_or(0.016, |s| s.current_dt_yr.max(1e-6));
    let dt_sec = dt_years * 31_557_600.0;

    for (entity, mut wave, opt_pos) in blast_waves_query.iter_mut() {
        let delta_r = wave.expansion_speed_au_yr * dt_years;
        let old_r = wave.radius_au;
        let new_r = old_r + delta_r;
        wave.radius_au = new_r;
        wave.age_seconds += dt_sec;

        // Inverse-square geometric attenuation of the propagating pulse
        let attenuation = (old_r / new_r.max(0.05)).powi(2);
        wave.luminosity_solar = (wave.luminosity_solar * attenuation).max(0.0);
        wave.synchrotron_brightness =
            (wave.synchrotron_brightness * (old_r / new_r) as f32).max(0.0);

        if let Some(mut pos) = opt_pos {
            // Centered on shock origin
            pos.0 = wave.origin_au;
        }

        // Despawn once wave expands past planetary perimeter or dissipates
        if new_r >= wave.max_radius_au || wave.luminosity_solar < 1.0 {
            if let Ok(mut cmd) = commands.get_entity(entity) {
                cmd.despawn();
            }
        }
    }
}
