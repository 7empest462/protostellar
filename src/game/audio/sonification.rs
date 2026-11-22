//! Real-time astrophysical sonification, surface biome ambience profiles, and time-warp pitch modulation.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::neutron_stars::types::{MagnetarCrustStress, PulsarGlitchState};
use crate::simulation::relativity::RelativisticState;
use crate::simulation::resources::TimeWarp;
use crate::simulation::space_weather::AuroralOvalState;

use super::{CosmicAudioMode, CosmicAudioState};

/// Camera-proximity audio cue derived from distance and azimuth to the nearest energetic emitter.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ProximityAudioCue {
    pub proximity_boost: f32,
    pub pitch_shift_hz: f32,
    pub stereo_pan: f32,
    pub nearest_kind: Option<BodyType>,
}

/// Synthesized atmospheric wind and liquid ocean sound levels for First-Person Surface Mode.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SurfaceAmbienceProfile {
    pub active: bool,
    pub wind_gain: f32,
    pub wind_pitch: f32,
    pub ocean_gain: f32,
    pub ocean_pitch: f32,
    pub space_duck: f32,
}

/// Maps simulation [`TimeWarp`] speed or pause state onto a subtle audio pitch multiplier.
pub fn compute_time_warp_pitch_factor(time_warp: Option<&TimeWarp>) -> f32 {
    let Some(tw) = time_warp else {
        return 1.0;
    };
    if tw.is_paused {
        return 0.78;
    }
    let rel_decades = (tw.multiplier.max(0.01).log10() - 50.0_f64.log10()) as f32;
    (1.0 + rel_decades * 0.075).clamp(0.75, 1.38)
}

/// Evaluates wind and surf intensities from a planet's atmospheric pressure, temperature, and ocean cover.
pub fn compute_surface_ambience_profile(
    body: &CelestialBody,
    opt_vol: Option<&VolatileInventory>,
    opt_climate: Option<&PlanetaryClimate>,
    opt_temp: Option<&Temperature>,
) -> SurfaceAmbienceProfile {
    let cloud_frac = opt_climate
        .map_or(0.0, |c| c.cloud_coverage_frac)
        .clamp(0.0, 1.0);
    let mut pressure_bar = opt_vol.map_or(0.0, |v| v.atmospheric_pressure_bar.max(0.0));
    if matches!(body.body_type, BodyType::GasGiant | BodyType::IceGiant) {
        pressure_bar = pressure_bar.max(25.0);
    } else if cloud_frac > 0.02 {
        pressure_bar = pressure_bar.max(0.05);
    }

    let temp_k = opt_climate
        .map(|c| c.surface_temperature_k)
        .or_else(|| opt_temp.map(|t| t.0 as f32))
        .unwrap_or(288.0);
    let ocean_frac = opt_vol.map_or(0.0, |v| v.ocean_coverage_frac.clamp(0.0, 1.0));
    let ice_frac = opt_climate
        .map_or(0.0, |c| c.ice_coverage_frac)
        .clamp(0.0, 1.0);

    if pressure_bar < 0.005 {
        return SurfaceAmbienceProfile {
            active: true,
            wind_gain: 0.0,
            wind_pitch: 1.0,
            ocean_gain: 0.0,
            ocean_pitch: 1.0,
            space_duck: 0.85,
        };
    }

    let log_p =
        ((pressure_bar.max(0.005) / 0.005).log10() / (100.0_f32 / 0.005).log10()).clamp(0.0, 1.0);
    let wind_gain = (0.22 + 0.56 * log_p + 0.14 * cloud_frac).clamp(0.22, 0.90);
    let wind_pitch = (1.28 - 0.56 * log_p).clamp(0.72, 1.28);
    let mut space_duck = (0.60 - 0.35 * log_p).clamp(0.25, 0.60);

    let has_liquid_surf = ocean_frac > 0.03 && (255.0..=420.0).contains(&temp_k) && ice_frac < 0.92;
    let (ocean_gain, ocean_pitch) = if has_liquid_surf {
        space_duck = space_duck.min(0.28);
        let gain = (ocean_frac * (1.0 - ice_frac * 0.7)).clamp(0.18, 0.85);
        let pitch = (0.85 + 0.30 * (1.0 - ocean_frac)).clamp(0.80, 1.20);
        (gain, pitch)
    } else {
        (0.0, 1.0)
    };

    SurfaceAmbienceProfile {
        active: true,
        wind_gain,
        wind_pitch,
        ocean_gain,
        ocean_pitch,
        space_duck,
    }
}

/// Evaluates smooth camera-proximity audio gain, frequency shift, and stereo pan relative to
/// energetic emitters (stars, black holes, neutron stars, and white dwarfs).
pub fn compute_camera_proximity_cue(
    camera_pos: Vec3,
    camera_right: Vec3,
    emitters: &[(Vec3, f32, BodyType)],
) -> ProximityAudioCue {
    compute_camera_proximity_cue_iter(camera_pos, camera_right, emitters.iter().copied())
}

pub(crate) fn compute_camera_proximity_cue_iter(
    camera_pos: Vec3,
    camera_right: Vec3,
    emitters: impl Iterator<Item = (Vec3, f32, BodyType)>,
) -> ProximityAudioCue {
    let right_dir = camera_right.normalize_or_zero();
    let mut best_boost = 0.0_f32;
    let mut best_dist = f32::MAX;
    let mut best_pan = 0.0_f32;
    let mut best_kind: Option<BodyType> = None;

    for (pos, visual_radius, body_type) in emitters {
        if !(body_type.is_star_or_remnant() || body_type == BodyType::QuasiStar) {
            continue;
        }
        let delta = pos - camera_pos;
        let dist = delta.length();
        let inner = (visual_radius * 2.5).max(0.05);
        let outer = (visual_radius * 24.0).max(4.5);
        let t = (1.0 - (dist - inner) / (outer - inner).max(1e-4)).clamp(0.0, 1.0);
        let boost = t * t * (3.0 - 2.0 * t);

        if boost > best_boost || ((boost - best_boost).abs() < 1e-5 && dist < best_dist) {
            best_boost = boost;
            best_dist = dist;
            best_kind = Some(body_type);
            best_pan = if dist > 1e-6 {
                (delta / dist).dot(right_dir).clamp(-1.0, 1.0)
            } else {
                0.0
            };
        }
    }

    let pitch_shift_hz = match best_kind {
        Some(BodyType::BlackHole | BodyType::QuasiStar) => -12.0 * best_boost,
        Some(_) => 28.0 * best_boost,
        None => 0.0,
    };

    ProximityAudioCue {
        proximity_boost: best_boost,
        pitch_shift_hz,
        stereo_pan: best_pan,
        nearest_kind: best_kind,
    }
}

pub(crate) fn compute_waveform_bars(
    carrier_freq_hz: f32,
    modulation_hz: f32,
    strain_or_flux: f32,
    visual_time: f32,
    enabled: bool,
) -> [u8; 16] {
    let mut bars = [0u8; 16];
    if !enabled {
        return bars;
    }
    let scale_f = (carrier_freq_hz / 65.0).clamp(0.6, 5.5);
    let amp = strain_or_flux.clamp(0.12, 1.0);
    for (idx, slot) in bars.iter_mut().enumerate() {
        let x = idx as f32;
        let harmonic = (x * 0.55 * scale_f + visual_time * (2.5 + modulation_hz * 0.35)).sin();
        let overtone = (x * 1.15 - visual_time * 4.2).cos();
        let combined = harmonic * 0.65 + overtone * 0.35;
        let norm = f32::midpoint(combined, 1.0) * amp;
        *slot = (norm * 7.5).round().clamp(0.0, 7.0) as u8;
    }
    bars
}

/// Computes the instantaneous astrophysical sonification snapshot for a selected celestial body.
#[allow(
    clippy::too_many_arguments,
    reason = "Sonification fuses gravitational, neutron-star, and space-weather telemetry"
)]
pub fn compute_body_sonification_snapshot(
    body: &CelestialBody,
    opt_rel: Option<&RelativisticState>,
    opt_glitch: Option<&PulsarGlitchState>,
    opt_stress: Option<&MagnetarCrustStress>,
    opt_aurora: Option<&AuroralOvalState>,
    visual_time: f32,
    enabled: bool,
) -> CosmicAudioState {
    let (mode, carrier, mod_hz, intensity, status_line) = if let Some(rel) = opt_rel
        .filter(|r| r.is_coalescing || r.gw_luminosity_watts >= 1.0e24 || r.gw_frequency_hz > 0.0)
    {
        let log_f = (rel.gw_frequency_hz.max(1e-6).log10() + 6.0).clamp(0.0, 9.0) as f32;
        let base_hz = 65.0 + log_f * 45.0;
        let carrier = if rel.is_coalescing {
            (base_hz * 1.35).min(480.0)
        } else {
            base_hz
        };
        let mod_hz = 1.5 + (rel.beta_v_over_c as f32 * 40.0).clamp(0.0, 25.0);
        let amp = if rel.is_coalescing {
            1.0
        } else {
            (rel.gw_strain * 1.0e21).clamp(0.20, 0.92) as f32
        };
        let label = if rel.is_coalescing {
            format!("COALESCING f_GW={:.2e} Hz", rel.gw_frequency_hz)
        } else {
            format!("Strain h={:.2e}", rel.gw_strain)
        };
        (
            CosmicAudioMode::GravitationalWaveChirp,
            carrier,
            mod_hz,
            amp,
            label,
        )
    } else if let Some(stress) = opt_stress {
        let strain_ratio = (stress.accumulated_magnetic_strain
            / stress.crust_yield_strain.max(1e-4))
        .clamp(0.0, 1.2) as f32;
        let carrier = if stress.burst_active {
            265.0
        } else {
            95.0 + strain_ratio * 130.0
        };
        let mod_hz = if stress.burst_active {
            18.0
        } else {
            2.5 + strain_ratio * 8.0
        };
        let amp = if stress.burst_active {
            1.0
        } else {
            strain_ratio.clamp(0.18, 0.95)
        };
        let label = format!(
            "B={:.1e} G | Crust {:.0}%{}",
            stress.magnetic_field_gauss,
            (strain_ratio * 100.0).min(120.0),
            if stress.burst_active { " [BURST]" } else { "" }
        );
        (CosmicAudioMode::MagnetarTremor, carrier, mod_hz, amp, label)
    } else if let Some(glitch) = opt_glitch {
        let spin_hz = glitch.spin_frequency_hz.abs() as f32;
        let carrier =
            (110.0 + spin_hz.clamp(0.2, 700.0).log10().max(0.0) * 110.0).clamp(90.0, 440.0);
        let mod_hz = spin_hz.clamp(0.8, 28.0);
        let amp = if glitch.is_relaxing {
            0.90
        } else {
            (0.35 + spin_hz * 0.015).clamp(0.30, 0.85)
        };
        let label = format!(
            "Spin {:.2} Hz | Glitches: {}{}",
            spin_hz,
            glitch.total_glitches_occurred,
            if glitch.is_relaxing {
                " [RELAXING]"
            } else {
                ""
            }
        );
        (
            CosmicAudioMode::PulsarLighthouse,
            carrier,
            mod_hz,
            amp,
            label,
        )
    } else if let Some(aurora) = opt_aurora.filter(|a| a.geomagnetic_kp_index >= 2.0) {
        let kp = aurora.geomagnetic_kp_index.clamp(0.0, 9.0);
        let carrier = 130.0 + (kp / 9.0) * 260.0;
        let mod_hz = 1.2 + kp * 0.85;
        let amp = aurora.auroral_intensity.max(kp / 9.0).clamp(0.20, 1.0);
        let label = format!("Kp {:.1} ({})", kp, aurora.storm_level.label());
        (
            CosmicAudioMode::MagnetosphericChorus,
            carrier,
            mod_hz,
            amp,
            label,
        )
    } else {
        let carrier = if body.body_type == BodyType::BlackHole {
            46.0
        } else if body.body_type.is_star_or_remnant() {
            72.0
        } else {
            58.0
        };
        let amp = if body.body_type.is_star_or_remnant() {
            0.40
        } else {
            0.22
        };
        (
            CosmicAudioMode::DeepSpaceAmbient,
            carrier,
            0.45,
            amp,
            format!("{} continuum", body.name),
        )
    };

    CosmicAudioState {
        enabled,
        master_volume: 0.35,
        mode,
        carrier_freq_hz: carrier,
        modulation_hz: mod_hz,
        strain_or_flux: intensity,
        proximity_boost: 0.0,
        stereo_pan: 0.0,
        time_warp_pitch: 1.0,
        waveform_bars: compute_waveform_bars(carrier, mod_hz, intensity, visual_time, enabled),
        status_line,
    }
}

pub(crate) fn compute_ambient_snapshot(
    prox: ProximityAudioCue,
    visual_time: f32,
    master_volume: f32,
    warp_pitch: f32,
    enabled: bool,
) -> CosmicAudioState {
    let carrier = (55.0 + prox.pitch_shift_hz).clamp(32.0, 140.0);
    let mod_hz = 0.45 + prox.proximity_boost * 1.45;
    let intensity = (0.20 + prox.proximity_boost * 0.65).clamp(0.20, 0.90);
    let status_line = if prox.proximity_boost > 0.15 {
        match prox.nearest_kind {
            Some(BodyType::BlackHole | BodyType::QuasiStar) => {
                format!(
                    "Photon ring proximity ({:.0}%)",
                    prox.proximity_boost * 100.0
                )
            }
            Some(BodyType::Magnetar | BodyType::Pulsar | BodyType::NeutronStar) => {
                format!(
                    "Compact remnant proximity ({:.0}%)",
                    prox.proximity_boost * 100.0
                )
            }
            _ => format!(
                "Solar corona proximity ({:.0}%)",
                prox.proximity_boost * 100.0
            ),
        }
    } else {
        "Quiet interstellar continuum".to_string()
    };

    CosmicAudioState {
        enabled,
        master_volume,
        mode: CosmicAudioMode::DeepSpaceAmbient,
        carrier_freq_hz: carrier,
        modulation_hz: mod_hz,
        strain_or_flux: intensity,
        proximity_boost: prox.proximity_boost,
        stereo_pan: prox.stereo_pan,
        time_warp_pitch: warp_pitch,
        waveform_bars: compute_waveform_bars(carrier, mod_hz, intensity, visual_time, enabled),
        status_line,
    }
}

pub(crate) fn compute_surface_snapshot(
    body: &CelestialBody,
    profile: SurfaceAmbienceProfile,
    opt_vol: Option<&VolatileInventory>,
    visual_time: f32,
    master_volume: f32,
    warp_pitch: f32,
    enabled: bool,
) -> CosmicAudioState {
    let pressure = opt_vol.map_or(
        if matches!(body.body_type, BodyType::GasGiant | BodyType::IceGiant) {
            25.0
        } else {
            0.0
        },
        |v| v.atmospheric_pressure_bar.max(0.0),
    );
    let ocean_pct =
        (opt_vol.map_or(0.0, |v| v.ocean_coverage_frac.clamp(0.0, 1.0)) * 100.0).round();
    let status_line = if profile.ocean_gain > 0.01 && profile.wind_gain > 0.01 {
        format!("Coastal breeze & surf ({pressure:.2} bar | {ocean_pct:.0}% ocean)")
    } else if profile.ocean_gain > 0.01 {
        format!("Tranquil surf ({ocean_pct:.0}% ocean)")
    } else if pressure >= 5.0 || matches!(body.body_type, BodyType::GasGiant | BodyType::IceGiant) {
        format!("Dense gale winds ({pressure:.1} bar)")
    } else if profile.wind_gain > 0.01 {
        format!("Thin dust wind ({pressure:.2} bar)")
    } else {
        format!("Vacuum regolith ({pressure:.2} bar)")
    };

    let carrier =
        (48.0 + 95.0 * profile.wind_gain * profile.wind_pitch + 24.0 * profile.ocean_gain)
            .clamp(38.0, 220.0);
    let mod_hz = (0.35 + profile.wind_gain * 1.1 + profile.ocean_gain * 0.8).clamp(0.3, 2.5);
    let intensity = profile.wind_gain.max(profile.ocean_gain).max(0.15);

    CosmicAudioState {
        enabled,
        master_volume,
        mode: CosmicAudioMode::SurfaceAmbience,
        carrier_freq_hz: carrier,
        modulation_hz: mod_hz,
        strain_or_flux: intensity,
        proximity_boost: 0.0,
        stereo_pan: 0.0,
        time_warp_pitch: warp_pitch,
        waveform_bars: compute_waveform_bars(carrier, mod_hz, intensity, visual_time, enabled),
        status_line,
    }
}
