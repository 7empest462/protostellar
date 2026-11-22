//! Track 4 regressions: 12 procedural stereo WAV synthesizers, distinct event-cue routing,
//! First-Person Surface wind/ocean ambience loops, and TimeWarp audio pitch scaling.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings};
use bevy::math::{DQuat, DVec3};
use bevy::prelude::*;
use protostellar::game::audio::{
    CosmicAmbientPlayer, CosmicAudioHandles, CosmicAudioMode, CosmicAudioState, SurfaceOceanPlayer,
    SurfaceWindPlayer, build_giant_impact_wav, build_kilonova_detonation_wav,
    build_probe_launch_wav, build_star_ignition_wav, build_supernova_detonation_wav,
    build_surface_ocean_loop_wav, build_surface_wind_loop_wav, compute_surface_ambience_profile,
    compute_time_warp_pitch_factor, setup_cosmic_audio_assets, update_cosmic_audio_system,
};
use protostellar::game::ui::telemetry_format::append_sonification_telemetry;
use protostellar::game::ui::types::NotificationToast;
use protostellar::rendering::camera::PanOrbitCamera;
use protostellar::simulation::accretion::events::AccretionMergeEvent;
use protostellar::simulation::accretion::{RocheDisruptionEvent, TidalDisruptionEvent};
use protostellar::simulation::components::{
    BodyType, CelestialBody, Composition, PlanetaryClimate, SupernovaEvent, Temperature,
    VolatileInventory,
};
use protostellar::simulation::neutron_stars::types::{MagnetarStarquakeEvent, PulsarGlitchEvent};
use protostellar::simulation::probes::{ProbeStatus, ProbeType, SpaceProbe};
use protostellar::simulation::relativity::GravitationalWaveMergerEvent;
use protostellar::simulation::resources::{PlayerInteractionState, SimTime, TimeWarp};
use protostellar::simulation::scenarios::ScenarioReloadGeneration;
use protostellar::simulation::space_weather::CmeShockwaveEvent;
use protostellar::simulation::thermodynamics::StarIgnitionEvent;
use protostellar::utils::constants::EARTH_MASS_SOLAR;

fn build_track4_audio_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<AudioSource>>()
        .init_resource::<CosmicAudioHandles>()
        .insert_resource(CosmicAudioState {
            enabled: true,
            ..default()
        })
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<bevy::input_focus::InputFocus>()
        .init_resource::<NotificationToast>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<SimTime>()
        .init_resource::<TimeWarp>()
        .init_resource::<ScenarioReloadGeneration>()
        .add_message::<GravitationalWaveMergerEvent>()
        .add_message::<SupernovaEvent>()
        .add_message::<TidalDisruptionEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_message::<CmeShockwaveEvent>()
        .add_message::<PulsarGlitchEvent>()
        .add_message::<MagnetarStarquakeEvent>()
        .add_message::<StarIgnitionEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_systems(Startup, setup_cosmic_audio_assets)
        .add_systems(Update, update_cosmic_audio_system);
    app.update();
    app
}

fn assert_valid_stereo_wav(wav: &[u8], min_duration_secs: f32, sample_rate: u32) -> (i16, i16) {
    assert!(wav.len() > 44);
    assert_eq!(wav.get(0..4), Some(b"RIFF".as_slice()));
    assert_eq!(wav.get(8..16), Some(b"WAVEfmt ".as_slice()));
    assert_eq!(wav.get(22..24), Some([2_u8, 0].as_slice()));
    assert_eq!(wav.get(36..40), Some(b"data".as_slice()));

    let expected_min_bytes = 44 + ((sample_rate as f32 * min_duration_secs) as usize) * 4;
    assert!(
        wav.len() >= expected_min_bytes,
        "WAV length {} shorter than minimum {expected_min_bytes}",
        wav.len()
    );

    let payload = &wav[44..];
    let mut sum_sq = 0.0_f64;
    for chunk in payload.chunks_exact(2) {
        let s = i16::from_le_bytes([chunk[0], chunk[1]]);
        sum_sq += f64::from(s) * f64::from(s);
    }
    let rms = (sum_sq / (payload.len() / 2) as f64).sqrt();
    assert!(rms > 25.0, "Expected audible PCM energy, got RMS={rms}");

    let first_l = i16::from_le_bytes([payload[0], payload[1]]);
    let first_r = i16::from_le_bytes([payload[2], payload[3]]);
    let last_l = i16::from_le_bytes([payload[payload.len() - 4], payload[payload.len() - 3]]);
    let last_r = i16::from_le_bytes([payload[payload.len() - 2], payload[payload.len() - 1]]);
    ((first_l - last_l).abs(), (first_r - last_r).abs())
}

fn latest_one_shot_cue(app: &mut App) -> Vec<(Handle<AudioSource>, f32)> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&AudioPlayer, &PlaybackSettings), (
        Without<CosmicAmbientPlayer>,
        Without<SurfaceWindPlayer>,
        Without<SurfaceOceanPlayer>,
    )>();
    q.iter(world).map(|(p, s)| (p.0.clone(), s.speed)).collect()
}

fn despawn_one_shot_cues(app: &mut App) {
    let world = app.world_mut();
    let ents: Vec<Entity> = world
        .query_filtered::<Entity, (
            With<AudioPlayer>,
            Without<CosmicAmbientPlayer>,
            Without<SurfaceWindPlayer>,
            Without<SurfaceOceanPlayer>,
        )>()
        .iter(world)
        .collect();
    for e in ents {
        world.despawn(e);
    }
}

#[test]
fn test_track4_procedural_wav_synthesizers_valid_stereo_pcm_and_loop_continuity() {
    let sr = 22_050_u32;
    let builders: [fn(u32) -> Vec<u8>; 7] = [
        build_giant_impact_wav,
        build_supernova_detonation_wav,
        build_kilonova_detonation_wav,
        build_star_ignition_wav,
        build_probe_launch_wav,
        build_surface_wind_loop_wav,
        build_surface_ocean_loop_wav,
    ];
    for builder in builders {
        assert_eq!(
            builder(0).len(),
            44,
            "Zero sample rate must safely return empty WAV"
        );
    }

    assert_valid_stereo_wav(&build_giant_impact_wav(sr), 1.5, sr);
    assert_valid_stereo_wav(&build_supernova_detonation_wav(sr), 2.5, sr);
    assert_valid_stereo_wav(&build_kilonova_detonation_wav(sr), 2.1, sr);
    assert_valid_stereo_wav(&build_star_ignition_wav(sr), 1.9, sr);
    assert_valid_stereo_wav(&build_probe_launch_wav(sr), 0.8, sr);

    let (wind_dl, wind_dr) = assert_valid_stereo_wav(&build_surface_wind_loop_wav(sr), 2.9, sr);
    assert!(
        wind_dl < 500 && wind_dr < 500,
        "Wind loop seam discontinuity: ({wind_dl}, {wind_dr})"
    );
    let (ocean_dl, ocean_dr) = assert_valid_stereo_wav(&build_surface_ocean_loop_wav(sr), 2.9, sr);
    assert!(
        ocean_dl < 500 && ocean_dr < 500,
        "Ocean loop seam discontinuity: ({ocean_dl}, {ocean_dr})"
    );
}

#[test]
fn test_track4_setup_populates_all_12_handles_and_preserves_single_ambient_player() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<AudioSource>>()
        .init_resource::<CosmicAudioHandles>()
        .init_resource::<CosmicAudioState>()
        .add_systems(Startup, setup_cosmic_audio_assets);
    app.update();

    let handles = app.world().resource::<CosmicAudioHandles>();
    assert!(handles.ambient_loop.is_some());
    assert!(handles.chirp_cue.is_some());
    assert!(handles.pulsar_cue.is_some());
    assert!(handles.ui_click_cue.is_some());
    assert!(handles.blowout_cue.is_some());
    assert!(handles.giant_impact_cue.is_some());
    assert!(handles.supernova_cue.is_some());
    assert!(handles.kilonova_cue.is_some());
    assert!(handles.star_ignition_cue.is_some());
    assert!(handles.probe_launch_cue.is_some());
    assert!(handles.surface_wind_loop.is_some());
    assert!(handles.surface_ocean_loop.is_some());

    let world = app.world_mut();
    let ambient = world
        .query_filtered::<&PlaybackSettings, With<CosmicAmbientPlayer>>()
        .single(world)
        .expect("Exactly one CosmicAmbientPlayer must spawn at Startup");
    assert!(ambient.muted);
    assert_eq!(
        world
            .query_filtered::<Entity, With<SurfaceWindPlayer>>()
            .iter(world)
            .count(),
        0
    );
    assert_eq!(
        world
            .query_filtered::<Entity, With<SurfaceOceanPlayer>>()
            .iter(world)
            .count(),
        0
    );
}

#[test]
fn test_track4_event_cues_route_distinct_handles_and_filter_dust_merges() {
    let mut app = build_track4_audio_app();
    let (sn_h, kn_h, gw_h, ig_h, imp_h, pr_h) = {
        let h = app.world().resource::<CosmicAudioHandles>();
        (
            h.supernova_cue.clone().unwrap(),
            h.kilonova_cue.clone().unwrap(),
            h.chirp_cue.clone().unwrap(),
            h.star_ignition_cue.clone().unwrap(),
            h.giant_impact_cue.clone().unwrap(),
            h.probe_launch_cue.clone().unwrap(),
        )
    };

    // 1. White-dwarf PlanetaryNebula Supernova -> supernova_cue at 0.78 speed
    app.world_mut().resource_mut::<SimTime>().visual_time_secs = 1.0;
    app.world_mut().write_message(SupernovaEvent {
        star_entity: Entity::PLACEHOLDER,
        star_name: "Sol".to_string(),
        initial_mass_solar: 1.0,
        remnant_mass_solar: 0.6,
        remnant_type: BodyType::WhiteDwarf,
        shockwave_velocity_km_s: 35.0,
        ejected_metals_solar: 0.4,
        ejected_composition: Composition::default(),
    });
    app.update();
    assert_eq!(latest_one_shot_cue(&mut app), vec![(sn_h, 0.78)]);
    despawn_one_shot_cues(&mut app);

    // 2. Neutron-star Kilonova vs Binary Black Hole Chirp
    app.world_mut().resource_mut::<SimTime>().visual_time_secs = 2.0;
    app.world_mut().write_message(GravitationalWaveMergerEvent {
        primary_entity: Entity::PLACEHOLDER,
        companion_entity: Entity::PLACEHOLDER,
        merged_entity: Entity::PLACEHOLDER,
        position: DVec3::ZERO,
        remnant_mass_solar: 2.7,
        radiated_gw_mass_solar: 0.08,
        peak_gw_frequency_hz: 1200.0,
        remnant_type: BodyType::NeutronStar,
    });
    app.update();
    assert_eq!(latest_one_shot_cue(&mut app), vec![(kn_h, 1.0)]);
    despawn_one_shot_cues(&mut app);

    app.world_mut().resource_mut::<SimTime>().visual_time_secs = 3.0;
    app.world_mut().write_message(GravitationalWaveMergerEvent {
        primary_entity: Entity::PLACEHOLDER,
        companion_entity: Entity::PLACEHOLDER,
        merged_entity: Entity::PLACEHOLDER,
        position: DVec3::ZERO,
        remnant_mass_solar: 60.0,
        radiated_gw_mass_solar: 3.1,
        peak_gw_frequency_hz: 240.0,
        remnant_type: BodyType::BlackHole,
    });
    app.update();
    assert_eq!(latest_one_shot_cue(&mut app), vec![(gw_h, 0.90)]);
    despawn_one_shot_cues(&mut app);

    // 3. Star Ignition -> star_ignition_cue
    app.world_mut().resource_mut::<SimTime>().visual_time_secs = 4.0;
    app.world_mut().write_message(StarIgnitionEvent {
        star_entity: Entity::PLACEHOLDER,
        star_mass: 1.0,
        luminosity_l_sun: 1.0,
        surface_temp_kelvin: 5778.0,
    });
    app.update();
    assert_eq!(latest_one_shot_cue(&mut app), vec![(ig_h, 1.0)]);
    despawn_one_shot_cues(&mut app);

    // 4. Tiny dust merge filtered out; giant planetary merge spawns giant_impact_cue
    app.world_mut().resource_mut::<SimTime>().visual_time_secs = 5.0;
    app.world_mut().write_message(AccretionMergeEvent {
        primary_entity: Entity::PLACEHOLDER,
        secondary_entity: Entity::PLACEHOLDER,
        merged_mass: 0.001 * EARTH_MASS_SOLAR,
        merged_position: DVec3::X,
        merged_velocity: DVec3::ZERO,
        new_body_type: BodyType::Asteroid,
        energy_released: 1.0e20,
    });
    app.update();
    assert!(
        latest_one_shot_cue(&mut app).is_empty(),
        "Dust merges must not trigger impact boom"
    );

    app.world_mut().write_message(AccretionMergeEvent {
        primary_entity: Entity::PLACEHOLDER,
        secondary_entity: Entity::PLACEHOLDER,
        merged_mass: 2.5 * EARTH_MASS_SOLAR,
        merged_position: DVec3::X,
        merged_velocity: DVec3::ZERO,
        new_body_type: BodyType::SuperEarth,
        energy_released: 1.0e32,
    });
    app.update();
    let imp = latest_one_shot_cue(&mut app);
    assert_eq!(imp.len(), 1);
    assert_eq!(imp[0].0, imp_h);
    despawn_one_shot_cues(&mut app);

    // 5. SpaceProbe launch -> probe_launch_cue; scenario reload suppresses initial probe spawn
    app.world_mut().resource_mut::<SimTime>().visual_time_secs = 6.0;
    let base_probe = SpaceProbe {
        probe_type: ProbeType::Orbiter,
        status: ProbeStatus::Transit,
        id: 1,
        orientation: DQuat::IDENTITY,
        landed_unspun_pos: None,
        orbit_axis: None,
        arrival_time: None,
        rover_heading_unspun: None,
        distance_driven_km: 0.0,
        samples_collected: 0,
        traverse_trail_unspun: Vec::new(),
        latest_assay: String::new(),
    };
    app.world_mut().spawn(base_probe.clone());
    app.update();
    assert_eq!(latest_one_shot_cue(&mut app), vec![(pr_h, 1.0)]);
    despawn_one_shot_cues(&mut app);

    app.world_mut().resource_mut::<SimTime>().visual_time_secs = 7.0;
    app.world_mut().resource_mut::<ScenarioReloadGeneration>().0 += 1;
    app.world_mut().spawn(base_probe);
    app.update();
    assert!(
        latest_one_shot_cue(&mut app).is_empty(),
        "Reload generation must suppress probe cue"
    );
}

fn count_surface_ambience_players(app: &mut App) -> (usize, usize) {
    let wind = app
        .world_mut()
        .query_filtered::<Entity, With<SurfaceWindPlayer>>()
        .iter(app.world())
        .count();
    let ocean = app
        .world_mut()
        .query_filtered::<Entity, With<SurfaceOceanPlayer>>()
        .iter(app.world())
        .count();
    (wind, ocean)
}

#[test]
fn test_track4_surface_ambience_profiles_and_dynamic_wind_ocean_loop_lifecycle() {
    let earth_body = CelestialBody {
        name: "Earth".into(),
        body_type: BodyType::TerrestrialPlanet,
    };
    let earth_vol = VolatileInventory {
        delivered_water_m_earth: 0.0002,
        ocean_coverage_frac: 0.71,
        atmospheric_pressure_bar: 1.0,
        cometary_impact_count: 1,
    };
    let earth_prof = compute_surface_ambience_profile(
        &earth_body,
        Some(&earth_vol),
        Some(&PlanetaryClimate::default()),
        Some(&Temperature(288.0)),
    );
    assert!(
        earth_prof.wind_gain > 0.25
            && earth_prof.ocean_gain > 0.40
            && earth_prof.space_duck <= 0.28
    );

    let venus_vol = VolatileInventory {
        delivered_water_m_earth: 0.0,
        ocean_coverage_frac: 0.0,
        atmospheric_pressure_bar: 92.0,
        cometary_impact_count: 0,
    };
    let venus_clim = PlanetaryClimate {
        surface_temperature_k: 735.0,
        ice_coverage_frac: 0.0,
        cloud_coverage_frac: 1.0,
        ..default()
    };
    let venus_prof =
        compute_surface_ambience_profile(&earth_body, Some(&venus_vol), Some(&venus_clim), None);
    assert!(
        venus_prof.wind_gain > 0.70 && venus_prof.wind_pitch < 0.85 && venus_prof.ocean_gain == 0.0
    );

    let snowball_clim = PlanetaryClimate {
        surface_temperature_k: 230.0,
        ice_coverage_frac: 1.0,
        ..default()
    };
    assert_eq!(
        compute_surface_ambience_profile(&earth_body, Some(&earth_vol), Some(&snowball_clim), None)
            .ocean_gain,
        0.0
    );

    let airless =
        compute_surface_ambience_profile(&earth_body, None, None, Some(&Temperature(120.0)));
    assert_eq!(airless.wind_gain, 0.0);
    assert_eq!(airless.ocean_gain, 0.0);
    assert_eq!(airless.space_duck, 0.85);

    let mut app = build_track4_audio_app();
    let earth_ent = app
        .world_mut()
        .spawn((
            earth_body,
            earth_vol,
            PlanetaryClimate::default(),
            Temperature(288.0),
        ))
        .id();
    let cam_ent = app
        .world_mut()
        .spawn((
            Transform::default(),
            Camera3d::default(),
            PanOrbitCamera {
                surface_horizon_mode: true,
                target_entity: Some(earth_ent),
                ..default()
            },
        ))
        .id();
    app.update();

    assert_eq!(
        app.world().resource::<CosmicAudioState>().mode,
        CosmicAudioMode::SurfaceAmbience
    );
    assert_eq!(count_surface_ambience_players(&mut app), (1, 1));

    app.world_mut()
        .get_mut::<PanOrbitCamera>(cam_ent)
        .unwrap()
        .surface_horizon_mode = false;
    app.update();
    assert_ne!(
        app.world().resource::<CosmicAudioState>().mode,
        CosmicAudioMode::SurfaceAmbience
    );
    assert_eq!(count_surface_ambience_players(&mut app), (0, 0));

    app.world_mut()
        .get_mut::<PanOrbitCamera>(cam_ent)
        .unwrap()
        .surface_horizon_mode = true;
    app.update();
    assert_eq!(count_surface_ambience_players(&mut app).0, 1);

    app.world_mut().resource_mut::<CosmicAudioState>().enabled = false;
    app.update();
    assert_eq!(count_surface_ambience_players(&mut app), (0, 0));
}

#[test]
fn test_track4_time_warp_pitch_scaling_and_telemetry_readout() {
    assert_eq!(compute_time_warp_pitch_factor(None), 1.0);
    assert_eq!(
        compute_time_warp_pitch_factor(Some(&TimeWarp {
            multiplier: 100.0,
            is_paused: true,
            step_once: false,
        })),
        0.78
    );
    assert_eq!(
        compute_time_warp_pitch_factor(Some(&TimeWarp {
            multiplier: 0.01,
            is_paused: false,
            step_once: false,
        })),
        0.75
    );
    assert!((compute_time_warp_pitch_factor(Some(&TimeWarp::default())) - 1.0).abs() < 1e-5);
    let hyper = compute_time_warp_pitch_factor(Some(&TimeWarp {
        multiplier: 1_000_000.0,
        is_paused: false,
        step_once: false,
    }));
    assert!(
        hyper > 1.25 && hyper <= 1.38,
        "Expected ~1.32x warp pitch, got {hyper}"
    );

    let mut app = build_track4_audio_app();
    app.world_mut().resource_mut::<TimeWarp>().multiplier = 1_000_000.0;
    app.update();
    let audio = app.world().resource::<CosmicAudioState>();
    assert!(audio.time_warp_pitch > 1.25);

    let mut readout = String::new();
    let surface_state = CosmicAudioState {
        enabled: true,
        mode: CosmicAudioMode::SurfaceAmbience,
        time_warp_pitch: hyper,
        status_line: "Coastal breeze & surf (1.00 bar | 71% ocean)".to_string(),
        ..default()
    };
    append_sonification_telemetry(&mut readout, &surface_state);
    assert!(readout.contains("PLANETARY SURFACE AMBIENCE"));
    assert!(readout.contains("Warp "));
}
