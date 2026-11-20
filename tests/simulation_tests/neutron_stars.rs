//! Tests for pulsar glitches, superfluid vortex unpinning, magnetar starquakes,
//! and relativistic propagating shear Alfvén blast waves.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::neutron_stars::*;
use protostellar::simulation::resources::*;

#[test]
fn test_dipole_spin_down_rate_scaling() {
    let r_km = 10.0;
    let nu_hz = 100.0;

    // 1. Weak magnetic field (B = 10^8 Gauss)
    let b_weak = 1.0e8;
    let dot_nu_weak = evaluate_dipole_spin_down_rate(b_weak, r_km, nu_hz);
    assert!(
        dot_nu_weak < 0.0,
        "Spin-down rate must be negative (braking)"
    );

    // 2. Strong magnetic field (B = 10^9 Gauss)
    let b_strong = 1.0e9;
    let dot_nu_strong = evaluate_dipole_spin_down_rate(b_strong, r_km, nu_hz);

    // Dipole braking scales with B^2: factor of 10 in B = factor of 100 in braking
    let ratio = dot_nu_strong / dot_nu_weak;
    assert!(
        (ratio - 100.0).abs() < 1e-4,
        "Dipole braking must scale quadratically with magnetic field B (got ratio: {ratio})"
    );
}

#[test]
fn test_pulsar_glitch_vortex_unpinning_jump() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(NeutronStarPlugin);

    app.init_resource::<TimeWarp>();
    let sim_time = SimTime {
        current_dt_yr: 0.01,
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let state = PulsarGlitchState {
        spin_frequency_hz: 200.0,
        unpinning_threshold_hz: 1.0e-3,
        vortex_pinning_lag_hz: 1.01e-3, // Exceeds threshold to trigger glitch
        glitch_jump_fraction: 1.0e-5,
        ..default()
    };

    let pulsar_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Pulsar,
                name: "Test Glitching Pulsar".to_string(),
            },
            SpinState::default(),
            state.clone(),
        ))
        .id();

    app.update();

    let updated_state = app.world().get::<PulsarGlitchState>(pulsar_ent).unwrap();
    assert_eq!(updated_state.total_glitches_occurred, 1);
    assert!(
        updated_state.spin_frequency_hz > 200.0,
        "Spin frequency must increase after unpinning jump"
    );
    assert!(
        (updated_state.last_glitch_jump_hz - 200.0 * 1.0e-5).abs() < 1e-6,
        "Frequency jump must match fractional amplitude"
    );
    assert!(
        updated_state.vortex_pinning_lag_hz < 1.0e-6,
        "Vortex pinning lag must reset upon glitch (found {})",
        updated_state.vortex_pinning_lag_hz
    );
    assert!(updated_state.is_relaxing);
}

#[test]
fn test_pulsar_post_glitch_exponential_relaxation() {
    let mut state = PulsarGlitchState {
        spin_frequency_hz: 100.0,
        glitch_jump_fraction: 1.0e-4,
        post_glitch_relaxation_time_days: 10.0,
        relaxation_healing_q: 0.50,
        ..default()
    };

    let (before, after, jump) = trigger_instantaneous_glitch(&mut state, 0.0);
    assert_eq!(before, 100.0);
    assert_eq!(after, 100.0 + jump);
    assert!(state.is_relaxing);

    let peak_freq = state.spin_frequency_hz;

    // Step by 5 days (half of relaxation timescale)
    let mut spin = SpinState::default();
    step_single_pulsar_glitch(&mut state, Some(&mut spin), None, 5.0 * 86400.0);

    assert!(
        state.spin_frequency_hz < peak_freq,
        "Post-glitch frequency must relax downward (got {} vs peak {})",
        state.spin_frequency_hz,
        peak_freq
    );
    assert!(
        state.spin_frequency_hz > 100.0,
        "Frequency must remain elevated above baseline during relaxation"
    );

    // Step past 5 tau (55 days total)
    step_single_pulsar_glitch(&mut state, Some(&mut spin), None, 55.0 * 86400.0);
    assert!(
        !state.is_relaxing,
        "Pulsar relaxation must terminate after 5 recovery timescales"
    );
}

#[test]
fn test_magnetar_magnetic_strain_and_starquake_rupture() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(NeutronStarPlugin);

    app.init_resource::<TimeWarp>();
    let sim_time = SimTime {
        current_dt_yr: 0.0002, // ~1.75 hours (light travels ~12.6 AU)
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let stress = MagnetarCrustStress {
        magnetic_field_gauss: 1.0e15,
        accumulated_magnetic_strain: 0.105, // Exceeds yield strain 0.10
        crust_yield_strain: 0.10,
        ..default()
    };

    let magnetar_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Magnetar,
                name: "SGR Test Magnetar".to_string(),
            },
            SimPosition(DVec3::new(10.0, 0.0, 0.0)),
            Luminosity(10_000.0),
            ElectromagneticFieldState {
                magnetic_field_gauss: 1.0e15,
                rotation_period_sec: 5.0,
                magnetic_inclination_rad: 0.2,
                jet_length_au: 3.0,
                synchrotron_intensity: 1.0,
            },
            stress.clone(),
        ))
        .id();

    app.update();

    let updated_stress = app
        .world()
        .get::<MagnetarCrustStress>(magnetar_ent)
        .unwrap();
    assert_eq!(
        updated_stress.total_starquakes_count, 1,
        "Starquake count must increment"
    );
    assert!(
        updated_stress.burst_active,
        "Burst must be active following rupture"
    );
    assert!(
        updated_stress.last_starquake_energy_erg > 1.0e45,
        "Released energy must exceed 10^45 erg (got {:.2e})",
        updated_stress.last_starquake_energy_erg
    );
    assert!(
        updated_stress.accumulated_magnetic_strain < 0.05,
        "Magnetic strain must drop plastically after rupture"
    );

    // Verify an AlfvenBlastWave entity was spawned
    let mut wave_query = app.world_mut().query::<&AlfvenBlastWave>();
    let blast_waves: Vec<_> = wave_query.iter(app.world()).collect();
    assert_eq!(
        blast_waves.len(),
        1,
        "Exactly 1 relativistic Alfvén wave must spawn"
    );
    assert_eq!(blast_waves[0].source_entity, magnetar_ent);
    assert!(
        blast_waves[0].luminosity_solar > 50.0,
        "Blast wave luminosity must remain luminous after expansion (got {:.2} L_sun)",
        blast_waves[0].luminosity_solar
    );
}

#[test]
fn test_relativistic_alfven_blast_wave_expansion() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(NeutronStarPlugin);

    app.init_resource::<TimeWarp>();
    let sim_time = SimTime {
        current_dt_yr: 0.001, // ~8.7 hours
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let source_magnetar = app.world_mut().spawn_empty().id();
    let wave_ent = app
        .world_mut()
        .spawn((
            AlfvenBlastWave {
                source_entity: source_magnetar,
                origin_au: DVec3::ZERO,
                radius_au: 1.0,
                expansion_speed_au_yr: C_LIGHT_AU_YR, // c
                energy_erg: 1.0e45,
                luminosity_solar: 1.0e8,
                synchrotron_brightness: 1.0,
                age_seconds: 0.0,
                max_radius_au: 100.0,
            },
            SimPosition(DVec3::ZERO),
        ))
        .id();

    app.update();

    let wave = app.world().get::<AlfvenBlastWave>(wave_ent).unwrap();
    // At speed c, expands by 63,241 * 0.001 = 63.241 AU
    assert!(
        wave.radius_au > 60.0,
        "Relativistic blast wave must expand at ~c (got radius: {:.2} AU)",
        wave.radius_au
    );
    assert!(
        wave.luminosity_solar < 1.0e8,
        "Luminosity must attenuate geometrically"
    );
}

#[test]
fn test_manual_starquake_trigger_event() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(NeutronStarPlugin);

    app.init_resource::<TimeWarp>();
    let sim_time = SimTime {
        current_dt_yr: 0.01,
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let magnetar_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Magnetar,
                name: "Target Magnetar".to_string(),
            },
            SimPosition(DVec3::ZERO),
            MagnetarCrustStress {
                accumulated_magnetic_strain: 0.03, // Below yield
                crust_yield_strain: 0.10,
                ..default()
            },
        ))
        .id();

    // Send manual trigger targeting this magnetar
    app.world_mut()
        .resource_mut::<Messages<TriggerManualGlitchOrStarquakeEvent>>()
        .write(TriggerManualGlitchOrStarquakeEvent {
            target_entity: Some(magnetar_ent),
        });

    app.update();

    let stress = app
        .world()
        .get::<MagnetarCrustStress>(magnetar_ent)
        .unwrap();
    assert_eq!(
        stress.total_starquakes_count, 1,
        "Manual trigger must immediately rupture crust"
    );
    assert!(stress.burst_active);
}

#[test]
fn test_neutron_star_and_oort_telemetry_formatting() {
    use protostellar::game::ui::telemetry_format::{
        append_magnetar_stress_telemetry, append_oort_comet_telemetry,
        append_pulsar_glitch_telemetry,
    };
    use protostellar::simulation::oort::types::OortComet;

    let glitch = PulsarGlitchState::default();
    let mut out = String::new();
    append_pulsar_glitch_telemetry(&mut out, &glitch);
    assert!(out.contains("RELATIVISTIC PULSAR DYNAMICS"));
    assert!(out.contains("Spin Frequency:"));
    assert!(out.contains("160.770 Hz"));

    let stress = MagnetarCrustStress {
        accumulated_magnetic_strain: 0.11,
        ..Default::default()
    };
    let mut out_stress = String::new();
    append_magnetar_stress_telemetry(&mut out_stress, &stress);
    assert!(out_stress.contains("ULTRA-STRONG MAGNETAR CRUST"));
    assert!(out_stress.contains("CRUST CRITICAL"));

    let comet = OortComet {
        is_sungrazer: true,
        ..default()
    };
    let mut out_comet = String::new();
    append_oort_comet_telemetry(&mut out_comet, &comet);
    assert!(out_comet.contains("OORT CLOUD ICY PLANETESIMAL"));
    assert!(out_comet.contains("SUN-GRAZING INFALL"));
}

#[test]
fn test_alfven_wave_subluminal_expansion_and_magnetar_baseline_restoration() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(NeutronStarPlugin);
    app.init_resource::<TimeWarp>();
    app.insert_resource(SimTime {
        current_dt_yr: 1.0e-7, // ~3.15 s -> light travels 0.00632 AU (< old 0.05 AU FTL floor)
        ..Default::default()
    });

    let magnetar = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Magnetar,
                name: "SGR 1806-20".to_string(),
            },
            SimPosition(DVec3::ZERO),
            Luminosity(10_000.0),
            MagnetarCrustStress {
                accumulated_magnetic_strain: 0.0,
                burst_active: true,
                burst_cooldown_years: 1.0e-8,
                peak_flare_luminosity_solar: 1.0e8,
                ..Default::default()
            },
        ))
        .id();

    let wave = app
        .world_mut()
        .spawn((
            AlfvenBlastWave {
                source_entity: magnetar,
                origin_au: DVec3::ZERO,
                radius_au: 0.05,
                expansion_speed_au_yr: C_LIGHT_AU_YR * 0.999,
                energy_erg: 1.0e45,
                luminosity_solar: 1.0e8,
                synchrotron_brightness: 1.0,
                age_seconds: 0.0,
                max_radius_au: 60.0,
            },
            SimPosition(DVec3::ZERO),
        ))
        .id();

    app.update();

    let updated_wave = app.world().get::<AlfvenBlastWave>(wave).unwrap();
    assert!(
        updated_wave.radius_au < 0.06,
        "Alfvén wave must respect c over short timesteps instead of 0.05 AU FTL jump (r={})",
        updated_wave.radius_au
    );
    assert!(
        updated_wave.luminosity_solar < 1.0e8,
        "Alfvén wave must attenuate smoothly even inside 1.0 AU (L={})",
        updated_wave.luminosity_solar
    );
    let lum = app.world().get::<Luminosity>(magnetar).unwrap().0;
    assert!(
        (lum - 10_000.0).abs() < 1e-3,
        "Magnetar must restore 10,000 L_sun post-flare baseline instead of 10 L_sun (got {lum})"
    );
}

#[test]
fn test_pulsar_analytic_glitch_recovery_and_millisecond_spin_preserved() {
    // 1. Exact analytic glitch recovery must be timestep-invariant (no braking: spin_down = 0)
    let mut single_step = PulsarGlitchState {
        spin_frequency_hz: 100.0,
        spin_down_rate_hz_s: 0.0,
        post_glitch_relaxation_time_days: 45.0,
        relaxation_healing_q: 0.5,
        ..Default::default()
    };
    trigger_instantaneous_glitch(&mut single_step, 0.0);
    let mut multi_step = single_step.clone();

    step_single_pulsar_glitch(&mut single_step, None, None, 45.0 * 86400.0);
    for _ in 0..90 {
        step_single_pulsar_glitch(&mut multi_step, None, None, 0.5 * 86400.0);
    }
    assert!(
        (single_step.spin_frequency_hz - multi_step.spin_frequency_hz).abs() < 1e-9,
        "Glitch recovery must be timestep-invariant (1x45d={} vs 90x0.5d={})",
        single_step.spin_frequency_hz,
        multi_step.spin_frequency_hz
    );

    // 2. Millisecond pulsar preserves sub-ms rotation period in SpinState & computes dipole braking
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(NeutronStarPlugin);
    app.init_resource::<TimeWarp>();
    app.insert_resource(SimTime {
        current_dt_yr: 0.001,
        ..Default::default()
    });

    let msp = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Pulsar,
                name: "PSR B1257+12".to_string(),
            },
            Mass(1.4),
            Radius(8.0e-8),
            SpinState::default(),
            ElectromagneticFieldState {
                magnetic_field_gauss: 1.0e9,
                rotation_period_sec: 0.00622,
                ..Default::default()
            },
            PulsarGlitchState {
                spin_frequency_hz: 160.77,
                spin_down_rate_hz_s: -1.0, // overwritten by evaluate_dipole_spin_down_rate
                ..Default::default()
            },
        ))
        .id();

    app.update();

    let w = app.world();
    let spin = w.get::<SpinState>(msp).unwrap();
    let glitch = w.get::<PulsarGlitchState>(msp).unwrap();
    assert!(
        spin.rotation_period_hours < 0.001,
        "Millisecond pulsar SpinState period must NOT clamp to 0.5 hr (got {})",
        spin.rotation_period_hours
    );
    assert!(
        spin.spin_vector.length() > 0.0,
        "Millisecond pulsar spin_vector must stay synchronized"
    );
    assert!(
        glitch.spin_down_rate_hz_s > -1e-10 && glitch.spin_down_rate_hz_s < 0.0,
        "Dipole spin-down rate must be recomputed from B, R, nu (got {:.3e})",
        glitch.spin_down_rate_hz_s
    );
}
