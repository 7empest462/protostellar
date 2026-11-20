//! Unit and integration tests for procedural cosmic audio synthesis, mute guards, and event cues.

use bevy::prelude::*;

use protostellar::game::audio::*;
use protostellar::game::ui::types::NotificationToast;
use protostellar::simulation::components::*;
use protostellar::simulation::relativity::RelativisticState;
use protostellar::simulation::resources::SimTime;
use protostellar::simulation::space_weather::CmeShockwaveEvent;

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
    let ui_click = build_ui_click_wav(22_050);
    assert_eq!(
        ui_click.len(),
        44 + ((22_050_f32 * 0.028).round() as usize) * 4
    );
    assert_eq!(ui_click.get(0..4), Some(b"RIFF".as_slice()));
    assert_eq!(ui_click.get(22..24), Some([2_u8, 0].as_slice()));
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
    let gw_snap = compute_body_sonification_snapshot(&bh, Some(&rel), None, None, None, 1.25, true);
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
    assert!(
        app.world()
            .resource::<CosmicAudioHandles>()
            .ui_click_cue
            .is_some(),
        "setup_cosmic_audio_assets must populate ui_click_cue"
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

fn fire_cme_at(app: &mut App, t: f32) -> usize {
    app.world_mut().resource_mut::<SimTime>().visual_time_secs = t;
    app.world_mut()
        .resource_mut::<Messages<CmeShockwaveEvent>>()
        .write(CmeShockwaveEvent {
            target: Entity::PLACEHOLDER,
            orbital_radius_au: 1.0,
            density_multiplier: 3.0,
            flare_intensity: 0.9,
        });
    app.update();
    app.world_mut()
        .query_filtered::<Entity, (With<AudioPlayer>, Without<CosmicAmbientPlayer>)>()
        .iter(app.world())
        .count()
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

    // 1. KeyM while text input is focused must NOT unmute; muted CME must not spawn cues
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
    assert_eq!(fire_cme_at(&mut app, 1.0), 0);

    // 2. Unfocus text and press KeyM -> unmutes; CME >= 0.35s spawns DESPAWN cue
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .clear();
    let mut kb = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    kb.reset_all();
    kb.press(KeyCode::KeyM);
    assert_eq!(fire_cme_at(&mut app, 1.0), 1);
    assert!(app.world().resource::<CosmicAudioState>().enabled);

    // 3. Within 0.35s cooldown (1.15s) throttles; after cooldown (1.50s) spawns next cue
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    assert_eq!(fire_cme_at(&mut app, 1.15), 1);
    assert_eq!(fire_cme_at(&mut app, 1.50), 2);
}
