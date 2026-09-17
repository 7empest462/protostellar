//! Comprehensive integration tests for Feature 2.2: General Relativistic Precession, Gravitational Waves & Inspiral Decay.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::relativity::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::*;
use protostellar::simulation::serialization::*;
use protostellar::utils::constants::*;

#[test]
fn test_mercury_1pn_precession_rate() {
    // Mercury: M = 1.0 M_sun, a = 0.387098 AU, e = 0.20563
    let m_sun = 1.0;
    let a_mercury = 0.387098;
    let e_mercury = 0.20563;

    let (rate_rad_yr, rate_arcsec_century) =
        calculate_1pn_precession_rate(m_sun, a_mercury, e_mercury);
    let advance_per_orbit = calculate_1pn_precession_per_orbit(m_sun, a_mercury, e_mercury);

    // Einstein's famous GR prediction for Mercury is ~42.98" to ~43.1" per century
    assert!(
        (rate_arcsec_century - 43.0).abs() < 0.5,
        "Mercury 1PN precession must be ~43.0 arcsec/century (got {rate_arcsec_century:.2})"
    );

    assert!(
        rate_rad_yr > 0.0,
        "Precession rate must be prograde (positive)"
    );
    assert!(
        advance_per_orbit > 0.0,
        "Precession per orbit must be positive"
    );
}

#[test]
fn test_1pn_acceleration_properties() {
    let m_sun = 1.0;
    let r_pos = DVec3::new(0.5, 0.0, 0.0);
    let v_circ = (G_ASTRO * m_sun / 0.5).sqrt();
    let v_vel = DVec3::new(0.0, 0.0, v_circ);

    let a_1pn = calculate_1pn_acceleration(r_pos, v_vel, m_sun);

    assert!(a_1pn.is_finite(), "1PN acceleration must be finite");
    // For circular orbit, radial term is inward (same direction as position towards origin or inwards correction)
    assert!(
        a_1pn.length() > 0.0,
        "1PN acceleration must be strictly non-zero"
    );

    // Scaling with distance: at 0.25 AU vs 0.50 AU, 1PN correction should be stronger
    let r_pos_close = DVec3::new(0.25, 0.0, 0.0);
    let v_circ_close = (G_ASTRO * m_sun / 0.25).sqrt();
    let a_1pn_close =
        calculate_1pn_acceleration(r_pos_close, DVec3::new(0.0, 0.0, v_circ_close), m_sun);

    assert!(
        a_1pn_close.length() > a_1pn.length(),
        "1PN acceleration must increase sharply as separation decreases"
    );
}

#[test]
fn test_peters_gw_power_scaling() {
    let m1 = 1.44;
    let m2 = 1.38;

    // Power scales as a^-5 for circular orbits
    let p_001 = calculate_peters_gw_power(m1, m2, 0.01, 0.0);
    let p_002 = calculate_peters_gw_power(m1, m2, 0.02, 0.0);

    assert!(p_001 > 0.0 && p_002 > 0.0);
    let ratio = p_001 / p_002;
    // (0.02 / 0.01)^5 = 2^5 = 32
    assert!(
        (ratio - 32.0).abs() < 1e-4,
        "GW power must scale as a^-5 (got ratio {ratio:.2})"
    );

    // Eccentric enhancement factor f(e): power must be higher for eccentric orbits
    let p_ecc = calculate_peters_gw_power(m1, m2, 0.01, 0.617);
    assert!(
        p_ecc > p_001,
        "Eccentric binary must radiate significantly more GW power than circular"
    );
}

#[test]
fn test_peters_orbital_decay_rates() {
    let m1 = 10.0;
    let m2 = 10.0;
    let a = 0.05;
    let e = 0.3;

    let da_dt = calculate_peters_da_dt(m1, m2, a, e);
    let de_dt = calculate_peters_de_dt(m1, m2, a, e);

    assert!(
        da_dt < 0.0,
        "Orbital semi-major axis must strictly decrease via GW emission"
    );
    assert!(
        de_dt < 0.0,
        "Orbital eccentricity must strictly damp towards 0 (circularization)"
    );
}

#[test]
fn test_hulse_taylor_pulsar_lifetime() {
    // PSR B1913+16 parameters:
    // m1 = 1.44 M_sun, m2 = 1.38 M_sun, a = 0.013 AU, e = 0.617
    let m1 = 1.44;
    let m2 = 1.38;
    let a = 0.013;
    let e = 0.617;

    let tau_yr = calculate_gw_coalescence_time(m1, m2, a, e);
    let tau_myr = tau_yr / 1e6;

    // Real measured inspiral time for Hulse-Taylor binary pulsar is ~300 Myr
    assert!(
        tau_myr > 200.0 && tau_myr < 400.0,
        "Hulse-Taylor inspiral lifetime must be ~300 Myr (got {tau_myr:.1} Myr)"
    );
}

#[test]
fn test_compact_binary_isco_coalescence_and_merger_event() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<SimulationConfig>()
        .init_resource::<RelativityConfig>()
        .add_message::<GravitationalWaveMergerEvent>();

    app.add_systems(Update, update_relativity_evolution);

    // Spawn primary black hole
    let m1 = 30.0;
    let r1 = calculate_schwarzschild_radius(m1);
    let primary = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Black Hole Primary".to_string(),
                body_type: BodyType::BlackHole,
            },
            CentralStar,
            Mass(m1),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(r1),
            Temperature(100.0),
            Luminosity(0.0),
            AngularMomentum::default(),
            Composition::metal_rich(),
            SpinState::default(),
        ))
        .id();

    // Spawn companion black hole right inside ISCO threshold
    let m2 = 30.0;
    let r2 = calculate_schwarzschild_radius(m2);
    let r_isco = calculate_isco_radius(m1 + m2);
    let companion = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Black Hole Companion".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(m2),
            SimPosition(DVec3::new(r_isco * 0.8, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 1000.0)),
            SimAcceleration::default(),
            Radius(r2),
            Temperature(100.0),
            Luminosity(0.0),
            AngularMomentum::default(),
            Composition::metal_rich(),
            SpinState::default(),
        ))
        .id();

    // Step simulation to trigger ISCO coalescence
    app.update();

    // Verify companion was despawned
    assert!(
        app.world().get_entity(companion).is_err(),
        "Companion black hole must be despawned upon merger"
    );

    // Verify primary has merged mass (accounting for 5% GW mass loss: 60 * 0.95 = 57 M_sun)
    let final_mass = app.world().get::<Mass>(primary).unwrap().0;
    assert!(
        (final_mass - 57.0).abs() < 1e-4,
        "Final merged black hole mass must be 57 M_sun (5% mass radiated as GWs)"
    );

    // Verify merger event was emitted
    let events = app
        .world()
        .resource::<Messages<GravitationalWaveMergerEvent>>();
    assert_eq!(events.len(), 1, "Exactly one merger event must be emitted");
}

#[test]
fn test_relativistic_binary_scenario_preset() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let mut disk_params = DiskParameters::default();
    let (pulsar, companion) =
        spawn_relativistic_binary_scenario(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let pulsar_body = app.world().get::<CelestialBody>(pulsar).unwrap();
    assert_eq!(pulsar_body.body_type, BodyType::Pulsar);
    assert!(app.world().get::<CentralStar>(pulsar).is_some());
    assert!(app.world().get::<RelativisticState>(pulsar).is_some());

    let companion_body = app.world().get::<CelestialBody>(companion).unwrap();
    assert_eq!(companion_body.body_type, BodyType::NeutronStar);
    let rel_state = app.world().get::<RelativisticState>(companion).unwrap();

    // Precession rate for Hulse-Taylor binary should be ~4.22 deg/yr (~1.5e6 arcsec/cy)
    assert!(
        rel_state.precession_rate_arcsec_century > 1e5,
        "Hulse-Taylor binary must exhibit strong periastron advance"
    );
    assert!(
        rel_state.gw_luminosity_watts > 1e23,
        "Close neutron star binary must have high GW luminosity"
    );
}

#[test]
fn test_relativistic_state_save_load_serialization() {
    let rel = RelativisticState {
        precession_rate_arcsec_century: 42.98,
        precession_advance_per_orbit_rad: 5.019e-7,
        gw_luminosity_watts: 1.15e4,
        gw_strain: 2.1e-31,
        gw_frequency_hz: 2.6e-7,
        inspiral_timescale_yr: 1.2e29,
        orbital_decay_rate_au_per_myr: -1.3e-22,
        accumulated_precession_rad: 0.05,
        is_coalescing: false,
        semi_major_axis_au: 0.3871,
        eccentricity: 0.2056,
    };

    let body_save = CelestialBodySave {
        name: "Test Relativistic World".to_string(),
        body_type: BodyType::TerrestrialPlanet,
        position: DVec3::new(0.387, 0.0, 0.0),
        velocity: DVec3::new(0.0, 0.0, 10.0),
        mass: 1.6e-7,
        radius: 1.6e-5,
        temperature: 440.0,
        luminosity: 0.0,
        composition: Composition::metal_rich(),
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
        tidal_state: None,
        relativistic_state: Some(rel),
        atmospheric_escape: None,
        kozai_lidov: None,
    };

    let json = serde_json::to_string(&body_save).expect("Serialization must succeed");
    let loaded: CelestialBodySave =
        serde_json::from_str(&json).expect("Deserialization must succeed");

    assert!(loaded.relativistic_state.is_some());
    let loaded_rel = loaded.relativistic_state.unwrap();
    assert!((loaded_rel.precession_rate_arcsec_century - 42.98).abs() < 1e-4);
    assert!((loaded_rel.gw_luminosity_watts - 1.15e4).abs() < 1.0);
    assert!((loaded_rel.semi_major_axis_au - 0.3871).abs() < 1e-4);
    assert!((loaded_rel.eccentricity - 0.2056).abs() < 1e-4);
}
