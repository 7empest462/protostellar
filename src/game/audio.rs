//! Real-time procedural astrophysical audio synthesizer and HUD waveform readout.

use std::sync::Arc;

use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::game::ui::types::NotificationToast;
use crate::simulation::accretion::{RocheDisruptionEvent, TidalDisruptionEvent};
use crate::simulation::components::*;
use crate::simulation::neutron_stars::types::{
    MagnetarCrustStress, MagnetarStarquakeEvent, PulsarGlitchEvent, PulsarGlitchState,
};
use crate::simulation::relativity::{GravitationalWaveMergerEvent, RelativisticState};
use crate::simulation::resources::*;
use crate::simulation::space_weather::{AuroralOvalState, CmeShockwaveEvent};

/// Active astrophysical synthesis mode driving the live procedural audio channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CosmicAudioMode {
    #[default]
    DeepSpaceAmbient,
    GravitationalWaveChirp,
    PulsarLighthouse,
    MagnetarTremor,
    MagnetosphericChorus,
}

impl CosmicAudioMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeepSpaceAmbient => "DEEP SPACE DRONE",
            Self::GravitationalWaveChirp => "GRAVITATIONAL-WAVE CHIRP",
            Self::PulsarLighthouse => "PULSAR RADIO LIGHTHOUSE",
            Self::MagnetarTremor => "MAGNETAR CRUST TREMOR",
            Self::MagnetosphericChorus => "MAGNETOSPHERIC CHORUS",
        }
    }
}

/// Live telemetry & synthesizer state for cosmic sonification and the Inspector oscilloscope.
#[derive(Resource, Debug, Clone)]
pub struct CosmicAudioState {
    pub enabled: bool,
    pub master_volume: f32,
    pub mode: CosmicAudioMode,
    pub carrier_freq_hz: f32,
    pub modulation_hz: f32,
    pub strain_or_flux: f32,
    pub proximity_boost: f32,
    pub stereo_pan: f32,
    pub waveform_bars: [u8; 16],
    pub status_line: String,
}

impl Default for CosmicAudioState {
    fn default() -> Self {
        Self {
            enabled: false,
            master_volume: 0.35,
            mode: CosmicAudioMode::DeepSpaceAmbient,
            carrier_freq_hz: 55.0,
            modulation_hz: 0.45,
            strain_or_flux: 0.2,
            proximity_boost: 0.0,
            stereo_pan: 0.0,
            waveform_bars: [0; 16],
            status_line: "Muted [M]".to_string(),
        }
    }
}

/// Camera-proximity audio cue derived from distance and azimuth to the nearest energetic emitter.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ProximityAudioCue {
    pub proximity_boost: f32,
    pub pitch_shift_hz: f32,
    pub stereo_pan: f32,
    pub nearest_kind: Option<BodyType>,
}

/// Asset handles for procedurally synthesized PCM/WAV astrophysical audio sources.
#[derive(Resource, Default)]
pub struct CosmicAudioHandles {
    pub ambient_loop: Option<Handle<AudioSource>>,
    pub chirp_cue: Option<Handle<AudioSource>>,
    pub pulsar_cue: Option<Handle<AudioSource>>,
}

/// Marker component attached to the active looping ambient sonification player.
#[derive(Component)]
pub struct CosmicAmbientPlayer;

#[derive(SystemParam)]
pub struct CosmicAudioEventReaders<'w, 's> {
    pub gw_merger: Option<MessageReader<'w, 's, GravitationalWaveMergerEvent>>,
    pub supernova: Option<MessageReader<'w, 's, SupernovaEvent>>,
    pub tde: Option<MessageReader<'w, 's, TidalDisruptionEvent>>,
    pub roche: Option<MessageReader<'w, 's, RocheDisruptionEvent>>,
    pub cme: Option<MessageReader<'w, 's, CmeShockwaveEvent>>,
    pub pulsar_glitch: Option<MessageReader<'w, 's, PulsarGlitchEvent>>,
    pub starquake: Option<MessageReader<'w, 's, MagnetarStarquakeEvent>>,
}

fn alloc_pcm_wav_with_header(sample_rate: u32, channels: u16, frame_count: usize) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = bits_per_sample / 8;
    let block_align: u16 = channels.saturating_mul(bytes_per_sample);
    let byte_rate: u32 = sample_rate.saturating_mul(u32::from(block_align));
    let data_bytes: u32 = frame_count
        .saturating_mul(usize::from(block_align))
        .min(u32::MAX as usize) as u32;
    let riff_size: u32 = 36_u32.saturating_add(data_bytes);

    let mut out = Vec::with_capacity(44 + data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    out
}

#[inline]
fn quantize_pcm_sample(v: f32) -> i16 {
    (v * f32::from(i16::MAX)).clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

/// Encodes mono 16-bit signed PCM samples into a complete 44-byte header RIFF/WAVE buffer.
pub fn synthesize_pcm_wav_mono(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let mut out = alloc_pcm_wav_with_header(sample_rate, 1, samples.len());
    for &sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Encodes interleaved stereo 16-bit signed PCM `(left, right)` frames into a RIFF/WAVE buffer.
pub fn synthesize_pcm_wav_stereo(sample_rate: u32, samples: &[(i16, i16)]) -> Vec<u8> {
    let mut out = alloc_pcm_wav_with_header(sample_rate, 2, samples.len());
    for &(left, right) in samples {
        out.extend_from_slice(&left.to_le_bytes());
        out.extend_from_slice(&right.to_le_bytes());
    }
    out
}

/// Synthesizes a seamless 2-second stereo deep-space harmonic continuum loop (integer-cycle 55 Hz binaural detune).
pub fn build_ambient_drone_wav(sample_rate: u32) -> Vec<u8> {
    let duration_secs = 2.0_f32;
    let total_samples = ((sample_rate as f32) * duration_secs) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let tau = std::f32::consts::TAU;
    for i in 0..total_samples {
        let t = (i as f32) / (sample_rate as f32);
        let lfo_l = 0.72 + 0.28 * (tau * 0.5 * t).sin();
        let lfo_r = 0.72 + 0.28 * (tau * 0.5 * t + std::f32::consts::FRAC_PI_2).sin();
        let wave_l = 0.55 * (tau * 55.0 * t).sin()
            + 0.28 * (tau * 110.0 * t).sin()
            + 0.14 * (tau * 165.0 * t).sin();
        let wave_r = 0.55 * (tau * 55.5 * t).sin()
            + 0.28 * (tau * 110.0 * t).sin()
            + 0.14 * (tau * 165.0 * t).cos();
        samples.push((
            quantize_pcm_sample(wave_l * lfo_l * 0.42),
            quantize_pcm_sample(wave_r * lfo_r * 0.42),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a stereo LIGO-style accelerating gravitational-wave chirp and ringdown chime.
pub fn build_gw_chirp_wav(sample_rate: u32) -> Vec<u8> {
    let duration_secs = 0.9_f32;
    let total_samples = ((sample_rate as f32) * duration_secs) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let tau = std::f32::consts::TAU;
    let mut phase = 0.0_f32;
    let dt = 1.0 / (sample_rate as f32);
    for i in 0..total_samples {
        let u = (i as f32) / (total_samples.max(1) as f32);
        let (freq, env) = if u < 0.78 {
            let p = u / 0.78;
            (68.0 + 360.0 * p.powi(2), 0.20 + 0.80 * p.powf(1.5))
        } else {
            let tail = (u - 0.78) / 0.22;
            (410.0 - 40.0 * tail, (-tail * 6.0).exp())
        };
        phase += tau * freq * dt;
        let fade_in = (u * 20.0).min(1.0);
        let wave_l = (phase.sin() + 0.3 * (phase * 2.0).sin()) * env * fade_in;
        let wave_r = ((phase + 0.45).sin() + 0.3 * ((phase + 0.45) * 2.0).sin()) * env * fade_in;
        samples.push((
            quantize_pcm_sample(wave_l * 0.45),
            quantize_pcm_sample(wave_r * 0.45),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a stereo dispersed radio-pulsar & starquake acoustic lighthouse pulse burst.
pub fn build_pulsar_pulse_wav(sample_rate: u32) -> Vec<u8> {
    let duration_secs = 0.35_f32;
    let total_samples = ((sample_rate as f32) * duration_secs) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let tau = std::f32::consts::TAU;
    for i in 0..total_samples {
        let t = (i as f32) / (sample_rate as f32);
        let u = (i as f32) / (total_samples.max(1) as f32);
        let env = (u * 30.0).min(1.0) * (-u * 8.0).exp();
        let sweep = 240.0 + 180.0 * (1.0 - u);
        let pulse = (tau * sweep * t).sin() * 0.7 + (tau * 18.0 * t).sin() * 0.3;
        let angle = (0.25 + 0.5 * u) * std::f32::consts::FRAC_PI_2;
        samples.push((
            quantize_pcm_sample(pulse * env * 0.45 * angle.cos()),
            quantize_pcm_sample(pulse * env * 0.45 * angle.sin()),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
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

fn compute_camera_proximity_cue_iter(
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

fn compute_waveform_bars(
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
        waveform_bars: compute_waveform_bars(carrier, mod_hz, intensity, visual_time, enabled),
        status_line,
    }
}

/// Startup system synthesizing WAV buffers and launching the background deep-space drone.
pub fn setup_cosmic_audio_assets(
    mut commands: Commands,
    mut handles: ResMut<CosmicAudioHandles>,
    audio_assets: Option<ResMut<Assets<AudioSource>>>,
    audio_state: Option<Res<CosmicAudioState>>,
) {
    let Some(mut assets) = audio_assets else {
        return;
    };

    let sample_rate = 22_050;
    let ambient_handle = assets.add(AudioSource {
        bytes: Arc::from(build_ambient_drone_wav(sample_rate).into_boxed_slice()),
    });
    let chirp_handle = assets.add(AudioSource {
        bytes: Arc::from(build_gw_chirp_wav(sample_rate).into_boxed_slice()),
    });
    let pulsar_handle = assets.add(AudioSource {
        bytes: Arc::from(build_pulsar_pulse_wav(sample_rate).into_boxed_slice()),
    });

    handles.ambient_loop = Some(ambient_handle.clone());
    handles.chirp_cue = Some(chirp_handle);
    handles.pulsar_cue = Some(pulsar_handle);

    let mut ambient_settings = PlaybackSettings::LOOP.with_volume(Volume::Linear(0.14));
    ambient_settings.muted = audio_state.as_deref().is_none_or(|state| !state.enabled);

    commands.spawn((
        AudioPlayer::new(ambient_handle),
        ambient_settings,
        CosmicAmbientPlayer,
    ));
}

#[derive(SystemParam)]
pub struct AudioInputParams<'w, 's> {
    pub keyboard: Option<Res<'w, ButtonInput<KeyCode>>>,
    pub focus: Option<Res<'w, bevy::input_focus::InputFocus>>,
    pub editable_query: Query<'w, 's, (), With<bevy::text::EditableText>>,
}

pub type AudioSelectedBodyItem = (
    &'static CelestialBody,
    Option<&'static Transform>,
    Option<&'static RelativisticState>,
    Option<&'static PulsarGlitchState>,
    Option<&'static MagnetarCrustStress>,
    Option<&'static AuroralOvalState>,
);

#[derive(SystemParam)]
pub struct AudioSceneParams<'w, 's> {
    pub camera_query: Query<'w, 's, &'static Transform, With<Camera3d>>,
    pub emitter_query: Query<'w, 's, (&'static Transform, &'static Radius, &'static CelestialBody)>,
    pub selected_query: Query<'w, 's, AudioSelectedBodyItem>,
}

/// Toggles cosmic sonification mute state and displays a HUD toast notification.
pub fn toggle_cosmic_audio_mute(
    audio_state: &mut CosmicAudioState,
    toast: Option<&mut NotificationToast>,
) {
    audio_state.enabled = !audio_state.enabled;
    if !audio_state.enabled {
        audio_state.waveform_bars = [0; 16];
    }
    if let Some(t) = toast {
        t.message = if audio_state.enabled {
            "🔊 Cosmic Sonification Audio Unmuted [M]".to_string()
        } else {
            "🔇 Cosmic Sonification Audio Muted [M]".to_string()
        };
        t.timer = 3.0;
    }
}

fn handle_audio_mute_toggle(
    audio_state: &mut CosmicAudioState,
    toast: &mut Option<ResMut<NotificationToast>>,
    input: &AudioInputParams,
) {
    let text_focused = crate::game::ui::widgets::is_text_input_focused(
        input.focus.as_deref(),
        &input.editable_query,
    );
    if !text_focused
        && input
            .keyboard
            .as_ref()
            .is_some_and(|kb| kb.just_pressed(KeyCode::KeyM))
    {
        toggle_cosmic_audio_mute(audio_state, toast.as_deref_mut());
    }
}

fn sample_camera_proximity(scene: &AudioSceneParams) -> (Option<(Vec3, Vec3)>, ProximityAudioCue) {
    let Some(cam_tf) = scene.camera_query.iter().next() else {
        return (None, ProximityAudioCue::default());
    };
    let cam_pos = cam_tf.translation;
    let cam_right = cam_tf.right().as_vec3();
    let cue = compute_camera_proximity_cue_iter(
        cam_pos,
        cam_right,
        scene
            .emitter_query
            .iter()
            .map(|(tf, rad, body)| (tf.translation, tf.scale.x.max(rad.0 as f32), body.body_type)),
    );
    (Some((cam_pos, cam_right)), cue)
}

fn apply_audio_sink_state(
    audio_state: &CosmicAudioState,
    sink_query: &mut Query<&mut AudioSink, With<CosmicAmbientPlayer>>,
) {
    for mut sink in sink_query.iter_mut() {
        if audio_state.enabled {
            sink.unmute();
            let target_speed = (audio_state.carrier_freq_hz / 55.0).clamp(0.5, 3.0);
            sink.set_speed(target_speed);
            let linear_vol = (audio_state.master_volume
                * (0.30 + 0.55 * audio_state.strain_or_flux + 0.25 * audio_state.proximity_boost))
                .clamp(0.0, 1.0);
            sink.set_volume(Volume::Linear(linear_vol));
        } else {
            sink.mute();
        }
    }
}

fn dispatch_cosmic_audio_cues(
    commands: &mut Commands,
    handles: &CosmicAudioHandles,
    audio_state: &CosmicAudioState,
    events: &mut CosmicAudioEventReaders,
    visual_time: f32,
    last_cue_time: &mut f32,
) {
    let has_chirp_event = events
        .gw_merger
        .as_mut()
        .is_some_and(|r| r.read().next().is_some())
        || events
            .supernova
            .as_mut()
            .is_some_and(|r| r.read().next().is_some())
        || events
            .tde
            .as_mut()
            .is_some_and(|r| r.read().next().is_some())
        || events
            .roche
            .as_mut()
            .is_some_and(|r| r.read().next().is_some());

    let has_pulse_event = events
        .cme
        .as_mut()
        .is_some_and(|r| r.read().next().is_some())
        || events
            .pulsar_glitch
            .as_mut()
            .is_some_and(|r| r.read().next().is_some())
        || events
            .starquake
            .as_mut()
            .is_some_and(|r| r.read().next().is_some());

    if !audio_state.enabled || (visual_time - *last_cue_time).abs() < 0.35 {
        return;
    }
    let cue_opt = if has_chirp_event {
        handles.chirp_cue.as_ref().map(|h| (h, 0.70_f32))
    } else if has_pulse_event {
        handles.pulsar_cue.as_ref().map(|h| (h, 0.55_f32))
    } else {
        None
    };
    if let Some((cue, scale)) = cue_opt {
        let vol = (audio_state.master_volume * scale * (1.0 + 0.2 * audio_state.proximity_boost))
            .clamp(0.0, 1.0);
        commands.spawn((
            AudioPlayer::new(cue.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(vol)),
        ));
        *last_cue_time = visual_time;
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Audio controller integrates keyboard input, telemetry queries, camera proximity, and sink modulation"
)]
pub fn update_cosmic_audio_system(
    mut commands: Commands,
    mut audio_state: ResMut<CosmicAudioState>,
    handles: Res<CosmicAudioHandles>,
    mut toast: Option<ResMut<NotificationToast>>,
    input: AudioInputParams,
    scene: AudioSceneParams,
    player_state: Option<Res<PlayerInteractionState>>,
    sim_time: Option<Res<SimTime>>,
    mut sink_query: Query<&mut AudioSink, With<CosmicAmbientPlayer>>,
    mut events: CosmicAudioEventReaders,
    mut last_cue_time: Local<f32>,
) {
    handle_audio_mute_toggle(&mut audio_state, &mut toast, &input);

    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let enabled = audio_state.enabled;
    let master_volume = audio_state.master_volume;
    let (cam_basis, prox) = sample_camera_proximity(&scene);

    if let Some(sel) = player_state.as_deref().and_then(|ps| ps.selected_entity) {
        if let Ok((body, opt_tf, opt_rel, opt_glitch, opt_stress, opt_aurora)) =
            scene.selected_query.get(sel)
        {
            let mut next = compute_body_sonification_snapshot(
                body,
                opt_rel,
                opt_glitch,
                opt_stress,
                opt_aurora,
                visual_time,
                enabled,
            );
            next.master_volume = master_volume;
            next.proximity_boost = prox.proximity_boost;
            next.stereo_pan = match (cam_basis, opt_tf) {
                (Some((cam_pos, cam_right)), Some(body_tf)) => (body_tf.translation - cam_pos)
                    .normalize_or_zero()
                    .dot(cam_right)
                    .clamp(-1.0, 1.0),
                _ => prox.stereo_pan,
            };
            next.carrier_freq_hz =
                (next.carrier_freq_hz + prox.pitch_shift_hz * 0.35).clamp(32.0, 520.0);
            next.strain_or_flux =
                (next.strain_or_flux + prox.proximity_boost * 0.25).clamp(0.0, 1.0);
            next.waveform_bars = compute_waveform_bars(
                next.carrier_freq_hz,
                next.modulation_hz,
                next.strain_or_flux,
                visual_time,
                enabled,
            );
            *audio_state = next;
        }
    } else {
        let carrier = (55.0 + prox.pitch_shift_hz).clamp(32.0, 140.0);
        let mod_hz = 0.45 + prox.proximity_boost * 1.45;
        let intensity = (0.20 + prox.proximity_boost * 0.65).clamp(0.20, 0.90);
        audio_state.mode = CosmicAudioMode::DeepSpaceAmbient;
        audio_state.carrier_freq_hz = carrier;
        audio_state.modulation_hz = mod_hz;
        audio_state.strain_or_flux = intensity;
        audio_state.proximity_boost = prox.proximity_boost;
        audio_state.stereo_pan = prox.stereo_pan;
        audio_state.waveform_bars =
            compute_waveform_bars(carrier, mod_hz, intensity, visual_time, enabled);
        audio_state.status_line = if prox.proximity_boost > 0.15 {
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
    }

    apply_audio_sink_state(&audio_state, &mut sink_query);
    dispatch_cosmic_audio_cues(
        &mut commands,
        &handles,
        &audio_state,
        &mut events,
        visual_time,
        &mut last_cue_time,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthesize_pcm_wav_mono_valid_riff_header_and_payload() {
        let samples = [0_i16, 1000, -1000, 12_345, -12_345];
        let wav = synthesize_pcm_wav_mono(22_050, &samples);
        assert_eq!(wav.len(), 44 + samples.len() * 2);
        assert_eq!(wav.get(0..4), Some(b"RIFF".as_slice()));
        assert_eq!(wav.get(8..16), Some(b"WAVEfmt ".as_slice()));
        assert_eq!(wav.get(22..24), Some([1_u8, 0].as_slice()));
        assert_eq!(wav.get(36..40), Some(b"data".as_slice()));

        let ambient = build_ambient_drone_wav(22_050);
        assert!(ambient.len() > 88_000);
        assert_eq!(ambient.get(22..24), Some([2_u8, 0].as_slice()));
        let chirp = build_gw_chirp_wav(22_050);
        assert!(chirp.len() > 20_000);
        assert_eq!(chirp.get(22..24), Some([2_u8, 0].as_slice()));
        let pulse = build_pulsar_pulse_wav(22_050);
        assert!(pulse.len() > 10_000);
        assert_eq!(pulse.get(22..24), Some([2_u8, 0].as_slice()));
    }

    #[test]
    fn test_synthesize_pcm_wav_stereo_and_camera_proximity_scaling() {
        let frames = [(120_i16, -120_i16), (500, -500), (2000, -2000)];
        let wav = synthesize_pcm_wav_stereo(22_050, &frames);
        assert_eq!(wav.len(), 44 + frames.len() * 4);
        assert_eq!(wav.get(0..4), Some(b"RIFF".as_slice()));
        assert_eq!(wav.get(22..24), Some([2_u8, 0].as_slice()));

        let far = compute_camera_proximity_cue(
            Vec3::new(0.0, 0.0, 25.0),
            Vec3::X,
            &[(Vec3::ZERO, 0.1, BodyType::MainSequenceStar)],
        );
        assert_eq!(far.proximity_boost, 0.0);

        let near_star = compute_camera_proximity_cue(
            Vec3::new(-0.8, 0.0, 0.0),
            Vec3::X,
            &[(Vec3::ZERO, 0.1, BodyType::MainSequenceStar)],
        );
        assert!(near_star.proximity_boost > 0.7);
        assert!(near_star.pitch_shift_hz > 15.0);
        assert!(near_star.stereo_pan > 0.9);

        let near_bh = compute_camera_proximity_cue(
            Vec3::new(0.6, 0.0, 0.0),
            Vec3::X,
            &[(Vec3::ZERO, 0.1, BodyType::BlackHole)],
        );
        assert!(near_bh.proximity_boost > 0.7);
        assert!(near_bh.pitch_shift_hz < -5.0);
        assert!(near_bh.stereo_pan < -0.9);
    }

    #[test]
    fn test_compute_body_sonification_snapshot_modes_and_mute() {
        let bh = CelestialBody {
            body_type: BodyType::BlackHole,
            name: "Cygnus X-1".to_string(),
        };
        let rel = RelativisticState {
            gw_frequency_hz: 120.0,
            gw_strain: 1.5e-21,
            gw_luminosity_watts: 1.0e44,
            is_coalescing: true,
            ..Default::default()
        };
        let gw_snap =
            compute_body_sonification_snapshot(&bh, Some(&rel), None, None, None, 1.25, true);
        assert_eq!(gw_snap.mode, CosmicAudioMode::GravitationalWaveChirp);
        assert!(gw_snap.carrier_freq_hz >= 200.0);
        assert!(gw_snap.waveform_bars.iter().any(|&b| b > 0));

        let muted_snap =
            compute_body_sonification_snapshot(&bh, Some(&rel), None, None, None, 1.25, false);
        assert!(!muted_snap.enabled);
        assert_eq!(muted_snap.waveform_bars, [0; 16]);
    }

    #[test]
    fn test_cosmic_audio_defaults_muted_and_toggle_helper() {
        let mut audio = CosmicAudioState::default();
        assert!(
            !audio.enabled,
            "Cosmic sonification audio must be muted by default"
        );
        assert_eq!(audio.waveform_bars, [0; 16]);

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<AudioSource>>();
        app.init_resource::<CosmicAudioHandles>();
        app.init_resource::<CosmicAudioState>();
        app.add_systems(Startup, setup_cosmic_audio_assets);
        app.update();

        let mut settings_query = app
            .world_mut()
            .query_filtered::<&PlaybackSettings, With<CosmicAmbientPlayer>>();
        let settings = settings_query
            .single(app.world())
            .expect("CosmicAmbientPlayer must spawn on Startup");
        assert!(
            settings.muted,
            "CosmicAmbientPlayer PlaybackSettings must start muted by default"
        );

        let mut toast = NotificationToast::default();
        toggle_cosmic_audio_mute(&mut audio, Some(&mut toast));
        assert!(audio.enabled, "Toggle must unmute audio when pressed");
        assert!(toast.message.contains("Unmuted"));

        audio.waveform_bars = [5; 16];
        toggle_cosmic_audio_mute(&mut audio, Some(&mut toast));
        assert!(!audio.enabled, "Second toggle must re-mute audio");
        assert_eq!(audio.waveform_bars, [0; 16]);
        assert!(toast.message.contains("Muted"));
    }

    #[test]
    fn test_audio_cue_dispatch_cooldown_and_text_focus_mute_guard() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<AudioSource>>()
            .init_resource::<CosmicAudioHandles>()
            .init_resource::<CosmicAudioState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input_focus::InputFocus>()
            .init_resource::<SimTime>()
            .add_message::<CmeShockwaveEvent>()
            .add_systems(Startup, setup_cosmic_audio_assets)
            .add_systems(Update, update_cosmic_audio_system);

        // 1. KeyM while text input is focused must NOT unmute audio
        let text_ent = app
            .world_mut()
            .spawn(bevy::text::EditableText::default())
            .id();
        app.world_mut()
            .resource_mut::<bevy::input_focus::InputFocus>()
            .set(text_ent, bevy::input_focus::FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyM);
        app.update();
        assert!(!app.world().resource::<CosmicAudioState>().enabled);

        // 2. While muted, cosmic events must NOT spawn one-shot cue players
        app.world_mut().resource_mut::<SimTime>().visual_time_secs = 1.0;
        app.world_mut()
            .resource_mut::<Messages<CmeShockwaveEvent>>()
            .write(CmeShockwaveEvent {
                target: Entity::PLACEHOLDER,
                orbital_radius_au: 1.0,
                density_multiplier: 3.0,
                flare_intensity: 0.9,
            });
        app.update();
        let oneshot_count = app
            .world_mut()
            .query_filtered::<Entity, (With<AudioPlayer>, Without<CosmicAmbientPlayer>)>()
            .iter(app.world())
            .count();
        assert_eq!(oneshot_count, 0, "Muted audio must suppress one-shot cues");

        // 3. Unfocus text and press KeyM -> unmutes; first CME event >= 0.35s spawns DESPAWN cue
        app.world_mut()
            .resource_mut::<bevy::input_focus::InputFocus>()
            .clear();
        {
            let mut kb = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            kb.reset_all();
            kb.press(KeyCode::KeyM);
        }
        app.world_mut().resource_mut::<SimTime>().visual_time_secs = 1.0;
        app.world_mut()
            .resource_mut::<Messages<CmeShockwaveEvent>>()
            .write(CmeShockwaveEvent {
                target: Entity::PLACEHOLDER,
                orbital_radius_au: 1.0,
                density_multiplier: 3.0,
                flare_intensity: 0.9,
            });
        app.update();
        assert!(app.world().resource::<CosmicAudioState>().enabled);

        let cues_after_first = app
            .world_mut()
            .query_filtered::<&PlaybackSettings, (With<AudioPlayer>, Without<CosmicAmbientPlayer>)>(
            )
            .iter(app.world())
            .count();
        assert_eq!(cues_after_first, 1, "Expected 1 one-shot CME audio cue");

        // 4. Within 0.35s cooldown (1.15s - 1.0s = 0.15s), another event must be throttled
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut().resource_mut::<SimTime>().visual_time_secs = 1.15;
        app.world_mut()
            .resource_mut::<Messages<CmeShockwaveEvent>>()
            .write(CmeShockwaveEvent {
                target: Entity::PLACEHOLDER,
                orbital_radius_au: 1.0,
                density_multiplier: 3.0,
                flare_intensity: 0.9,
            });
        app.update();
        let cues_during_cooldown = app
            .world_mut()
            .query_filtered::<Entity, (With<AudioPlayer>, Without<CosmicAmbientPlayer>)>()
            .iter(app.world())
            .count();
        assert_eq!(
            cues_during_cooldown, 1,
            "One-shot cues within 0.35s cooldown must be deduplicated"
        );

        // 5. After cooldown expires (1.50s - 1.0s = 0.50s >= 0.35s), next cue spawns cleanly
        app.world_mut().resource_mut::<SimTime>().visual_time_secs = 1.50;
        app.world_mut()
            .resource_mut::<Messages<CmeShockwaveEvent>>()
            .write(CmeShockwaveEvent {
                target: Entity::PLACEHOLDER,
                orbital_radius_au: 1.0,
                density_multiplier: 3.0,
                flare_intensity: 0.9,
            });
        app.update();
        let cues_after_cooldown = app
            .world_mut()
            .query_filtered::<Entity, (With<AudioPlayer>, Without<CosmicAmbientPlayer>)>()
            .iter(app.world())
            .count();
        assert_eq!(cues_after_cooldown, 2);
    }
}
