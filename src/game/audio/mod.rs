//! Real-time procedural astrophysical audio synthesizer, surface biome ambience, and HUD waveform.

pub mod cues;
pub mod sonification;
pub mod synth;

use std::sync::Arc;

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::game::ui::types::NotificationToast;
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::accretion::{RocheDisruptionEvent, TidalDisruptionEvent};
use crate::simulation::components::*;
use crate::simulation::neutron_stars::types::{
    MagnetarCrustStress, MagnetarStarquakeEvent, PulsarGlitchEvent, PulsarGlitchState,
};
use crate::simulation::relativity::{GravitationalWaveMergerEvent, RelativisticState};
use crate::simulation::resources::*;
use crate::simulation::space_weather::{AuroralOvalState, CmeShockwaveEvent};

pub use cues::AmbientAudioSinks;
use cues::{dispatch_cosmic_audio_cues, sync_ambient_and_surface_sinks};
pub use sonification::{
    ProximityAudioCue, SurfaceAmbienceProfile, compute_body_sonification_snapshot,
    compute_camera_proximity_cue, compute_surface_ambience_profile, compute_time_warp_pitch_factor,
};
use sonification::{
    compute_ambient_snapshot, compute_camera_proximity_cue_iter, compute_surface_snapshot,
    compute_waveform_bars,
};
pub use synth::{
    build_ambient_drone_wav, build_giant_impact_wav, build_gw_chirp_wav,
    build_kilonova_detonation_wav, build_probe_launch_wav, build_pulsar_pulse_wav,
    build_quasar_blowout_wav, build_star_ignition_wav, build_supernova_detonation_wav,
    build_surface_ocean_loop_wav, build_surface_wind_loop_wav, build_ui_click_wav,
    synthesize_pcm_wav_mono, synthesize_pcm_wav_stereo,
};

/// Active astrophysical synthesis mode driving the live procedural audio channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CosmicAudioMode {
    #[default]
    DeepSpaceAmbient,
    GravitationalWaveChirp,
    PulsarLighthouse,
    MagnetarTremor,
    MagnetosphericChorus,
    SurfaceAmbience,
}

impl CosmicAudioMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeepSpaceAmbient => "DEEP SPACE DRONE",
            Self::GravitationalWaveChirp => "GRAVITATIONAL-WAVE CHIRP",
            Self::PulsarLighthouse => "PULSAR RADIO LIGHTHOUSE",
            Self::MagnetarTremor => "MAGNETAR CRUST TREMOR",
            Self::MagnetosphericChorus => "MAGNETOSPHERIC CHORUS",
            Self::SurfaceAmbience => "PLANETARY SURFACE AMBIENCE",
        }
    }
}

/// Live telemetry & synthesizer state for cosmic sonification and the Inspector oscilloscope.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct CosmicAudioState {
    pub enabled: bool,
    pub master_volume: f32,
    pub mode: CosmicAudioMode,
    pub carrier_freq_hz: f32,
    pub modulation_hz: f32,
    pub strain_or_flux: f32,
    pub proximity_boost: f32,
    pub stereo_pan: f32,
    pub time_warp_pitch: f32,
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
            time_warp_pitch: 1.0,
            waveform_bars: [0; 16],
            status_line: "Muted [M]".to_string(),
        }
    }
}

/// Asset handles for the 12 procedurally synthesized PCM/WAV astrophysical audio sources.
#[derive(Resource, Default)]
pub struct CosmicAudioHandles {
    pub ambient_loop: Option<Handle<AudioSource>>,
    pub chirp_cue: Option<Handle<AudioSource>>,
    pub pulsar_cue: Option<Handle<AudioSource>>,
    pub ui_click_cue: Option<Handle<AudioSource>>,
    pub blowout_cue: Option<Handle<AudioSource>>,
    pub giant_impact_cue: Option<Handle<AudioSource>>,
    pub supernova_cue: Option<Handle<AudioSource>>,
    pub kilonova_cue: Option<Handle<AudioSource>>,
    pub star_ignition_cue: Option<Handle<AudioSource>>,
    pub probe_launch_cue: Option<Handle<AudioSource>>,
    pub surface_wind_loop: Option<Handle<AudioSource>>,
    pub surface_ocean_loop: Option<Handle<AudioSource>>,
}

/// Marker component attached to the active looping ambient sonification player.
#[derive(Component)]
pub struct CosmicAmbientPlayer;

/// Marker component for dynamically managed First-Person Surface atmospheric wind loop players.
#[derive(Component, Default)]
pub struct SurfaceWindPlayer;

/// Marker component for dynamically managed First-Person Surface rolling ocean surf loop players.
#[derive(Component, Default)]
pub struct SurfaceOceanPlayer;

#[derive(SystemParam)]
pub struct CosmicAudioEventReaders<'w, 's> {
    pub gw_merger: Option<MessageReader<'w, 's, GravitationalWaveMergerEvent>>,
    pub supernova: Option<MessageReader<'w, 's, SupernovaEvent>>,
    pub tde: Option<MessageReader<'w, 's, TidalDisruptionEvent>>,
    pub roche: Option<MessageReader<'w, 's, RocheDisruptionEvent>>,
    pub cme: Option<MessageReader<'w, 's, CmeShockwaveEvent>>,
    pub pulsar_glitch: Option<MessageReader<'w, 's, PulsarGlitchEvent>>,
    pub starquake: Option<MessageReader<'w, 's, MagnetarStarquakeEvent>>,
    pub supernova_pool: Option<Res<'w, crate::rendering::effects::supernova::SupernovaDebrisPool>>,
    pub star_ignition:
        Option<MessageReader<'w, 's, crate::simulation::thermodynamics::StarIgnitionEvent>>,
    pub accretion_merge:
        Option<MessageReader<'w, 's, crate::simulation::accretion::events::AccretionMergeEvent>>,
    #[allow(
        clippy::type_complexity,
        reason = "Groups optional crisis and civilization message readers within Bevy SystemParam limit"
    )]
    pub crisis_civ_events: (
        Option<MessageReader<'w, 's, crate::simulation::events::CosmicEventStartedEvent>>,
        Option<MessageReader<'w, 's, crate::simulation::events::CosmicEventResolvedEvent>>,
        Option<MessageReader<'w, 's, crate::simulation::biology::CivilizationMilestoneEvent>>,
        Option<MessageReader<'w, 's, crate::simulation::events::CrisisCountermeasureEvent>>,
    ),
    pub launched_probes: Query<'w, 's, Entity, Added<crate::simulation::probes::SpaceProbe>>,
    pub reload_gen: Option<Res<'w, crate::simulation::scenarios::ScenarioReloadGeneration>>,
}

#[inline]
fn add_wav_handle(assets: &mut Assets<AudioSource>, wav: Vec<u8>) -> Handle<AudioSource> {
    assets.add(AudioSource {
        bytes: Arc::from(wav.into_boxed_slice()),
    })
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
    let ambient_handle = add_wav_handle(&mut assets, build_ambient_drone_wav(sample_rate));
    handles.ambient_loop = Some(ambient_handle.clone());
    handles.chirp_cue = Some(add_wav_handle(&mut assets, build_gw_chirp_wav(sample_rate)));
    handles.pulsar_cue = Some(add_wav_handle(
        &mut assets,
        build_pulsar_pulse_wav(sample_rate),
    ));
    handles.ui_click_cue = Some(add_wav_handle(&mut assets, build_ui_click_wav(sample_rate)));
    handles.blowout_cue = Some(add_wav_handle(
        &mut assets,
        build_quasar_blowout_wav(sample_rate),
    ));
    handles.giant_impact_cue = Some(add_wav_handle(
        &mut assets,
        build_giant_impact_wav(sample_rate),
    ));
    handles.supernova_cue = Some(add_wav_handle(
        &mut assets,
        build_supernova_detonation_wav(sample_rate),
    ));
    handles.kilonova_cue = Some(add_wav_handle(
        &mut assets,
        build_kilonova_detonation_wav(sample_rate),
    ));
    handles.star_ignition_cue = Some(add_wav_handle(
        &mut assets,
        build_star_ignition_wav(sample_rate),
    ));
    handles.probe_launch_cue = Some(add_wav_handle(
        &mut assets,
        build_probe_launch_wav(sample_rate),
    ));
    handles.surface_wind_loop = Some(add_wav_handle(
        &mut assets,
        build_surface_wind_loop_wav(sample_rate),
    ));
    handles.surface_ocean_loop = Some(add_wav_handle(
        &mut assets,
        build_surface_ocean_loop_wav(sample_rate),
    ));

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
    Option<&'static VolatileInventory>,
    Option<&'static PlanetaryClimate>,
    Option<&'static Temperature>,
);

#[derive(SystemParam)]
pub struct AudioSceneParams<'w, 's> {
    pub camera_query:
        Query<'w, 's, (&'static Transform, Option<&'static PanOrbitCamera>), With<Camera3d>>,
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
        && input.keyboard.as_ref().is_some_and(|kb| {
            let is_shift = kb.pressed(KeyCode::ShiftLeft) || kb.pressed(KeyCode::ShiftRight);
            let is_alt = kb.pressed(KeyCode::AltLeft) || kb.pressed(KeyCode::AltRight);
            !is_shift && !is_alt && kb.just_pressed(KeyCode::KeyM)
        })
    {
        toggle_cosmic_audio_mute(audio_state, toast.as_deref_mut());
    }
}

fn sample_camera_proximity(scene: &AudioSceneParams) -> (Option<(Vec3, Vec3)>, ProximityAudioCue) {
    let Some((cam_tf, _)) = scene.camera_query.iter().next() else {
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

#[allow(
    clippy::too_many_arguments,
    reason = "Evaluates surface horizon mode or selected-body / deep-space sonification snapshot"
)]
fn evaluate_active_audio_snapshot(
    scene: &AudioSceneParams,
    player_state: Option<&PlayerInteractionState>,
    cam_basis: Option<(Vec3, Vec3)>,
    prox: ProximityAudioCue,
    visual_time: f32,
    master_volume: f32,
    warp_pitch: f32,
    enabled: bool,
) -> (CosmicAudioState, SurfaceAmbienceProfile) {
    let surface_target = scene.camera_query.iter().find_map(|(_, opt_cam)| {
        opt_cam
            .filter(|cam| cam.surface_horizon_mode)
            .and_then(|cam| {
                cam.target_entity
                    .or_else(|| player_state.and_then(|ps| ps.selected_entity))
            })
    });

    if let Some(surface_ent) = surface_target
        && let Ok((body, _, _, _, _, _, opt_vol, opt_climate, opt_temp)) =
            scene.selected_query.get(surface_ent)
    {
        let profile = compute_surface_ambience_profile(body, opt_vol, opt_climate, opt_temp);
        let state = compute_surface_snapshot(
            body,
            profile,
            opt_vol,
            visual_time,
            master_volume,
            warp_pitch,
            enabled,
        );
        return (state, profile);
    }

    if let Some(sel) = player_state.and_then(|ps| ps.selected_entity)
        && let Ok((body, opt_tf, opt_rel, opt_glitch, opt_stress, opt_aurora, _, _, _)) =
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
        next.time_warp_pitch = warp_pitch;
        next.stereo_pan = match (cam_basis, opt_tf) {
            (Some((cam_pos, cam_right)), Some(body_tf)) => (body_tf.translation - cam_pos)
                .normalize_or_zero()
                .dot(cam_right)
                .clamp(-1.0, 1.0),
            _ => prox.stereo_pan,
        };
        next.carrier_freq_hz =
            (next.carrier_freq_hz + prox.pitch_shift_hz * 0.35).clamp(32.0, 520.0);
        next.strain_or_flux = (next.strain_or_flux + prox.proximity_boost * 0.25).clamp(0.0, 1.0);
        next.waveform_bars = compute_waveform_bars(
            next.carrier_freq_hz,
            next.modulation_hz,
            next.strain_or_flux,
            visual_time,
            enabled,
        );
        return (next, SurfaceAmbienceProfile::default());
    }

    (
        compute_ambient_snapshot(prox, visual_time, master_volume, warp_pitch, enabled),
        SurfaceAmbienceProfile::default(),
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Audio controller integrates keyboard input, telemetry queries, camera proximity, surface ambience, and sink modulation"
)]
pub fn update_cosmic_audio_system(
    mut commands: Commands,
    mut audio_state: ResMut<CosmicAudioState>,
    handles: Res<CosmicAudioHandles>,
    mut toast: Option<ResMut<NotificationToast>>,
    input: AudioInputParams,
    scene: AudioSceneParams,
    player_state: Option<Res<PlayerInteractionState>>,
    (sim_time, time_warp): (Option<Res<SimTime>>, Option<Res<TimeWarp>>),
    mut sinks: AmbientAudioSinks,
    mut events: CosmicAudioEventReaders,
    mut last_cue_time: Local<f32>,
    mut played_blowouts: Local<rustc_hash::FxHashSet<Entity>>,
    mut last_reload_gen: Local<u64>,
) {
    handle_audio_mute_toggle(&mut audio_state, &mut toast, &input);

    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let warp_pitch = compute_time_warp_pitch_factor(time_warp.as_deref());
    let enabled = audio_state.enabled;
    let master_volume = audio_state.master_volume;

    if !enabled {
        if audio_state.status_line != "Muted [M]"
            || audio_state.waveform_bars != [0; 16]
            || audio_state.strain_or_flux != 0.0
            || (audio_state.time_warp_pitch - warp_pitch).abs() > 1e-4
        {
            audio_state.status_line = "Muted [M]".to_string();
            audio_state.waveform_bars = [0; 16];
            audio_state.strain_or_flux = 0.0;
            audio_state.time_warp_pitch = warp_pitch;
        }
        sync_ambient_and_surface_sinks(
            &mut commands,
            &handles,
            &audio_state,
            SurfaceAmbienceProfile::default(),
            &mut sinks,
        );
        dispatch_cosmic_audio_cues(
            &mut commands,
            &handles,
            &audio_state,
            &mut events,
            visual_time,
            &mut last_cue_time,
            &mut played_blowouts,
            &mut last_reload_gen,
        );
        return;
    }

    let (cam_basis, prox) = sample_camera_proximity(&scene);
    let (next, surface_profile) = evaluate_active_audio_snapshot(
        &scene,
        player_state.as_deref(),
        cam_basis,
        prox,
        visual_time,
        master_volume,
        warp_pitch,
        enabled,
    );

    if *audio_state != next {
        *audio_state = next;
    }

    sync_ambient_and_surface_sinks(
        &mut commands,
        &handles,
        &audio_state,
        surface_profile,
        &mut sinks,
    );
    dispatch_cosmic_audio_cues(
        &mut commands,
        &handles,
        &audio_state,
        &mut events,
        visual_time,
        &mut last_cue_time,
        &mut played_blowouts,
        &mut last_reload_gen,
    );
}
