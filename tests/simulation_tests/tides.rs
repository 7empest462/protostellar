//! Integration tests for Feature 2.1: Tidal Circularization, Spin-Orbit Synchronization & Tidal Locking.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::solar::spawn_solar_nebula_mmsn;
use protostellar::simulation::scenarios::trappist::spawn_trappist_1_system;
use protostellar::simulation::serialization::*;
use protostellar::simulation::tides::*;
use protostellar::utils::constants::*;

#[test]
fn test_pure_astrophysical_dissipation_formulas() {
    let m_host = 1.0; // 1 M_sun
    let m_body = EARTH_MASS_SOLAR;
    let r_body = EARTH_RADIUS_AU;
    let a = 0.05; // 0.05 AU close-in orbit
    let e = 0.10;
    let k2_over_q = 0.30 / 100.0; // 0.003

    let n = calculate_mean_motion(m_host, m_body, a);
    assert!(n > 0.0, "Mean motion must be positive");

    let de_dt = calculate_circularization_rate(k2_over_q, m_host, m_body, r_body, a, e, n);
    assert!(
        de_dt < 0.0,
        "Eccentricity rate de/dt must be negative (damping)"
    );

    let tau_circ = calculate_circularization_timescale(k2_over_q, m_host, m_body, r_body, a, n);
    let tau_sync = calculate_sync_timescale(k2_over_q, m_host, m_body, r_body, a, n);
    assert!(
        tau_sync < tau_circ,
        "Synchronization timescale must be significantly shorter than circularization timescale"
    );

    // Resonance calculations
    let (omega_circ, ratio_circ) = calculate_equilibrium_spin_frequency(n, 0.0, false);
    assert!(
        (ratio_circ - 1.0).abs() < 1e-3,
        "Circular orbit must have 1:1 synchronous resonance"
    );
    assert!((omega_circ - n).abs() < 1e-3);

    let (omega_32, ratio_32) = calculate_equilibrium_spin_frequency(n, 0.206, true);
    assert!(
        (ratio_32 - 1.5).abs() < 1e-3,
        "Mercury-like orbit must capture into 3:2 spin-orbit resonance"
    );
    assert!((omega_32 - 1.5 * n).abs() < 1e-3);
}

#[test]
fn test_viscoelastic_tidal_heating_scaling() {
    let m_host = 1.0;
    let r_body = EARTH_RADIUS_AU;
    let k2_over_q = 0.003;
    let n = 2.0 * PI;

    // a = 0.05 AU vs a = 0.10 AU: power should scale as (0.10 / 0.05)^6 = 64x
    let power_close = calculate_tidal_heating_power(k2_over_q, m_host, r_body, 0.05, 0.10, n, n);
    let power_far = calculate_tidal_heating_power(k2_over_q, m_host, r_body, 0.10, 0.10, n, n);

    assert!(power_close > 0.0 && power_far > 0.0);
    let ratio = power_close / power_far;
    assert!(
        (ratio - 64.0).abs() < 2.0,
        "Tidal heating power must scale as a^-6 (expected ~64x, got {ratio:.2})"
    );

    let flux_w_m2 = calculate_tidal_heat_flux(power_close, r_body);
    assert!(flux_w_m2 > 0.0, "Tidal heat flux must be strictly positive");
}

#[test]
fn test_spin_synchronization_relaxation_step() {
    let target_omega = 10.0;
    let initial_omega = 50.0;
    let initial_tilt = 23.5;
    let tau = 1000.0;
    let dt = 500.0;

    let (next_omega, next_tilt, progress) =
        apply_spin_synchronization_step(initial_omega, target_omega, initial_tilt, tau, dt);

    assert!(
        next_omega < initial_omega && next_omega > target_omega,
        "Spin frequency must decay towards target frequency"
    );
    assert!(
        next_tilt < initial_tilt,
        "Axial tilt must damp towards zero obliquity"
    );
    assert!(progress > 0.0 && progress < 1.0);

    // Far future convergence step
    let (final_omega, final_tilt, final_progress) =
        apply_spin_synchronization_step(initial_omega, target_omega, initial_tilt, tau, 50_000.0);

    assert!(
        (final_omega - target_omega).abs() < 1e-4,
        "Spin frequency must converge to target"
    );
    assert!(final_tilt < 0.01, "Axial tilt must converge to 0.0");
    assert!(
        (final_progress - 1.0).abs() < 1e-6,
        "Progress must be 1.0 when locked"
    );
}

#[test]
fn test_geothermal_core_heating_and_dynamo() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<SimulationConfig>()
        .init_resource::<TidalConfig>();

    app.add_systems(Update, update_tidal_evolution);

    // Spawn central star
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "The Sun".to_string(),
        },
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Radius(SOLAR_RADIUS_AU),
    ));

    // Spawn close-in planet with differentiated interior
    let a = 0.025;
    let v_circ = (G_ASTRO * 1.0 / a).sqrt();
    let pos = DVec3::new(a, 0.0, 0.0);
    let vel = DVec3::new(0.0, 0.0, v_circ * 1.2); // eccentric for high tidal heating

    let planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Hot Earth".to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(pos),
            SimVelocity(vel),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
            SpinState::default(),
            InternalDifferentiation {
                is_differentiated: true,
                core_temp_k: 1300.0,
                magnetic_field_gauss: 0.0,
                ocean_ice_thickness_au: 1e-7,
                ..default()
            },
            TidalState::new_rocky(),
        ))
        .id();

    let initial_temp = app
        .world()
        .get::<InternalDifferentiation>(planet_ent)
        .unwrap()
        .core_temp_k;

    app.update();

    let diff = app
        .world()
        .get::<InternalDifferentiation>(planet_ent)
        .unwrap();
    let tide = app.world().get::<TidalState>(planet_ent).unwrap();

    assert!(
        tide.tidal_heating_flux_w_m2 > 0.0,
        "Tidal heat flux must be positive"
    );
    assert!(
        diff.core_temp_k >= initial_temp,
        "Core temperature must increase or be sustained by tidal heating"
    );
    assert!(
        diff.magnetic_field_gauss > 0.0,
        "Magnetic field must be energized by tidal core heating"
    );
}

#[test]
fn test_simulation_spin_locking_and_circularization_system() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<SimulationConfig>()
        .init_resource::<TidalConfig>();

    app.add_systems(Update, update_tidal_evolution);

    // Spawn central star
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "The Sun".to_string(),
        },
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Radius(SOLAR_RADIUS_AU),
    ));

    // Spawn close-in planet (a = 0.03 AU, e ~ 0.15)
    let a = 0.03;
    let v_circ = (G_ASTRO * 1.0 / a).sqrt();
    let pos = DVec3::new(a, 0.0, 0.0);
    let vel = DVec3::new(0.0, 0.0, v_circ * 1.15); // slightly eccentric

    let planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Close Earth".to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(pos),
            SimVelocity(vel),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
            SpinState {
                rotation_period_hours: 6.0, // fast initial rotation
                axial_tilt_degrees: 25.0,
                spin_vector: DVec3::new(0.0, 1.0, 0.0),
            },
            TidalState::new_rocky(),
        ))
        .id();

    // Advance simulation
    app.update();

    let tide = app.world().get::<TidalState>(planet_ent).unwrap();
    let spin = app.world().get::<SpinState>(planet_ent).unwrap();

    assert!(tide.host_entity.is_some(), "Host entity must be set");
    assert!(
        tide.tidal_heating_power_watts > 0.0,
        "Eccentric close-in planet must generate tidal dissipation power"
    );
    assert!(
        tide.circularization_rate_per_myr < 0.0,
        "Eccentricity damping rate must be negative"
    );
    assert!(
        spin.rotation_period_hours > 6.0,
        "Spin period must increase as rotation synchronizes with orbit"
    );
    assert!(
        spin.axial_tilt_degrees < 25.0,
        "Axial tilt must damp towards zero"
    );
}

#[test]
fn test_trappist_system_tidal_states() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let mut disk_params = DiskParameters::default();

    let star = spawn_trappist_1_system(&mut app.world_mut().commands(), &mut disk_params);
    assert_ne!(star, Entity::PLACEHOLDER);
    app.update();

    let mut query = app.world_mut().query::<(&CelestialBody, &TidalState)>();
    let count = query.iter(app.world()).count();
    assert_eq!(count, 7, "All 7 TRAPPIST-1 planets must have TidalState");

    for (body, tide) in query.iter(app.world()) {
        assert!(
            tide.is_tidally_locked,
            "Planet {} must be tidally locked",
            body.name
        );
        assert!(
            (tide.locking_progress - 1.0).abs() < 1e-6,
            "Planet {} must be fully synchronized",
            body.name
        );
        assert!(
            tide.tidal_heating_flux_w_m2 > 0.0,
            "Planet {} must have active tidal heating flux",
            body.name
        );
    }
}

#[test]
fn test_solar_mercury_resonance_state() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let mut disk_params = DiskParameters::default();
    spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let mut query = app.world_mut().query::<(&CelestialBody, &TidalState)>();
    let mut mercury_found = false;
    for (body, tide) in query.iter(app.world()) {
        if body.name.contains("Mercury") {
            mercury_found = true;
            assert!(
                (tide.resonance_ratio - 1.50).abs() < 1e-6,
                "Mercury must be in 3:2 spin-orbit resonance"
            );
            assert!(tide.is_tidally_locked);
        }
    }
    assert!(
        mercury_found,
        "Proto-Mercury must be present with TidalState"
    );
}

#[test]
fn test_tidal_state_json_serialization() {
    let tide = TidalState {
        host_entity: None,
        love_number_k2: 0.28,
        tidal_q: 85.0,
        is_tidally_locked: true,
        locking_progress: 0.95,
        resonance_ratio: 1.5,
        tidal_heating_power_watts: 4.2e13,
        tidal_heating_flux_w_m2: 0.08,
        circularization_rate_per_myr: -0.003,
        circularization_timescale_yr: 4e7,
        sync_timescale_yr: 2e4,
    };

    let body_save = CelestialBodySave {
        name: "Test Planet".to_string(),
        body_type: BodyType::TerrestrialPlanet,
        position: DVec3::new(1.0, 0.0, 0.0),
        velocity: DVec3::new(0.0, 0.0, 6.0),
        mass: EARTH_MASS_SOLAR,
        radius: EARTH_RADIUS_AU,
        temperature: 288.0,
        luminosity: 0.0,
        composition: Composition::rocky(),
        spin: SpinState::default(),
        differentiation: None,
        volatile_inventory: None,
        ring_system: None,
        basins: None,
        climate: None,
        biosphere: None,
        electromagnetic: None,
        is_central_star: false,
        ignition_state: None,
        stellar_evolution: None,
        black_hole_state: None,
        satellite: None,
        tidal_state: Some(tide),
        relativistic_state: None,
        atmospheric_escape: None,
        kozai_lidov: None,
    };

    let json = serde_json::to_string(&body_save).expect("Serialization must succeed");
    let loaded: CelestialBodySave =
        serde_json::from_str(&json).expect("Deserialization must succeed");

    assert!(loaded.tidal_state.is_some());
    let loaded_tide = loaded.tidal_state.unwrap();
    assert!((loaded_tide.love_number_k2 - 0.28).abs() < 1e-6);
    assert!((loaded_tide.tidal_q - 85.0).abs() < 1e-6);
    assert!(loaded_tide.is_tidally_locked);
    assert!((loaded_tide.resonance_ratio - 1.5).abs() < 1e-6);
    assert!((loaded_tide.tidal_heating_power_watts - 4.2e13).abs() < 1.0);
}
