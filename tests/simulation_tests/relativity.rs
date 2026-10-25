//! Comprehensive integration tests for Feature 2.2: General Relativistic Precession, Gravitational Waves & Inspiral Decay.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::rendering::effects::kilonova::{update_kilonova_effects, KilonovaPool};

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
        total_redshift_z: 1.48e-8,
        beta_v_over_c: 0.00016,
        lense_thirring_rate_arcsec_century: 0.0,
        kerr_spin_param: 0.0,
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

#[test]
fn test_kilonova_fireball_and_bbh_ringdown_events() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<GravitationalWaveMergerEvent>();
    app.init_resource::<KilonovaPool>();
    app.add_systems(Update, update_kilonova_effects);

    let dummy_primary = Entity::from_bits(1);
    let dummy_companion = Entity::from_bits(2);

    // 1. Binary Neutron Star (BNS) merger -> Kilonova Event
    {
        let mut writer = app
            .world_mut()
            .resource_mut::<Messages<GravitationalWaveMergerEvent>>();
        writer.write(GravitationalWaveMergerEvent {
            primary_entity: dummy_primary,
            companion_entity: dummy_companion,
            merged_entity: dummy_primary,
            position: DVec3::new(15.0, 0.0, 0.0),
            remnant_mass_solar: 2.65,
            radiated_gw_mass_solar: 0.15,
            peak_gw_frequency_hz: 1560.0,
            remnant_type: BodyType::NeutronStar,
        });
    }

    app.update();

    let pool = app.world().resource::<KilonovaPool>();
    assert_eq!(
        pool.instances.len(),
        1,
        "Merger must generate a KilonovaInstance"
    );
    let kn = &pool.instances[0];
    assert!(kn.is_ns_merger, "NS merger must be flagged as kilonova");
    assert!(!kn.is_bbh_merger);
    assert_eq!(kn.position, Vec3::new(15.0, 0.0, 0.0));
    assert_eq!(kn.flash_intensity, 1.0);
    assert_eq!(kn.ejecta_speed_au_s, 45.0);

    // 2. Binary Black Hole (BBH) merger -> Gravitational Wave Ringdown Event
    {
        let mut writer = app
            .world_mut()
            .resource_mut::<Messages<GravitationalWaveMergerEvent>>();
        writer.write(GravitationalWaveMergerEvent {
            primary_entity: dummy_primary,
            companion_entity: dummy_companion,
            merged_entity: dummy_primary,
            position: DVec3::ZERO,
            remnant_mass_solar: 62.0,
            radiated_gw_mass_solar: 3.0,
            peak_gw_frequency_hz: 250.0,
            remnant_type: BodyType::BlackHole,
        });
    }

    app.update();

    let pool = app.world().resource::<KilonovaPool>();
    assert_eq!(
        pool.instances.len(),
        2,
        "Second merger must generate second instance"
    );
    let bbh = &pool.instances[1];
    assert!(
        bbh.is_bbh_merger,
        "BBH merger must be flagged as BBH ringdown"
    );
    assert!(!bbh.is_ns_merger);
    assert_eq!(bbh.radiated_gw_mass_solar, 3.0);
    assert_eq!(bbh.ejecta_speed_au_s, 65.0);
}

#[test]
fn test_1pn_rodriguez_rigid_rotation_energy_and_eccentricity_conservation() {
    use bevy::math::DQuat;

    // A high-eccentricity test orbit around a 1.0 M_sun star
    let mu = G_ASTRO * 1.0;
    let r_peri = 0.1;
    let ecc = 0.85;
    let a = r_peri / (1.0 - ecc);

    // Position at periastron along +X, velocity along +Z
    let r_vec = DVec3::new(r_peri, 0.0, 0.0);
    let v_p = (mu * (2.0 / r_peri - 1.0 / a)).sqrt();
    let v_vec = DVec3::new(0.0, 0.0, v_p);

    let specific_energy_orig = 0.5 * v_vec.length_squared() - mu / r_vec.length();
    let h_vec = r_vec.cross(v_vec);
    let h_hat = h_vec.normalize();
    let e_vec_orig = v_vec.cross(h_vec) / mu - r_vec / r_vec.length();

    // Rotate BOTH position and velocity by 1PN angle d_theta around h_hat (Rodriguez rotation)
    let d_theta = 0.035_f64; // ~2 degrees
    let rot = DQuat::from_axis_angle(h_hat, d_theta);
    let r_rot = rot * r_vec;
    let v_rot = rot * v_vec;

    // 1. Strict distance and speed conservation
    assert!(
        (r_rot.length() - r_vec.length()).abs() < 1e-12,
        "Rodriguez rotation must preserve radial distance to machine precision"
    );
    assert!(
        (v_rot.length() - v_vec.length()).abs() < 1e-12,
        "Rodriguez rotation must preserve velocity magnitude to machine precision"
    );

    // 2. Specific orbital energy identically conserved
    let specific_energy_rot = 0.5 * v_rot.length_squared() - mu / r_rot.length();
    assert!(
        (specific_energy_rot - specific_energy_orig).abs() < 1e-12,
        "Rodriguez rotation of (r, v) must conserve orbital energy exactly"
    );

    // 3. Angular momentum magnitude conserved
    let h_vec_rot = r_rot.cross(v_rot);
    assert!(
        (h_vec_rot.length() - h_vec.length()).abs() < 1e-12,
        "Angular momentum magnitude must be conserved"
    );

    // 4. Eccentricity vector rotated by exactly d_theta, magnitude unchanged
    let e_vec_rot = v_rot.cross(h_vec_rot) / mu - r_rot / r_rot.length();
    assert!(
        (e_vec_rot.length() - ecc).abs() < 1e-12,
        "Orbital eccentricity must not change under rigid apsidal rotation"
    );
    let angle_diff = e_vec_orig.angle_between(e_vec_rot);
    assert!(
        (angle_diff - d_theta).abs() < 1e-10,
        "Eccentricity vector must precess by exactly d_theta"
    );
}

#[test]
fn test_satellites_and_non_remnants_in_relativity_system() {
    let mut app = App::new();
    app.init_resource::<RelativityConfig>()
        .init_resource::<SimulationConfig>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .add_message::<GravitationalWaveMergerEvent>()
        .add_systems(Update, update_relativity_evolution);

    // 1. Central White Dwarf (compact remnant)
    let _wd_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::WhiteDwarf,
                name: "Sirius B".to_string(),
            },
            CentralStar,
            Mass(1.02),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Radius(0.0001),
        ))
        .id();

    // 2. Terrestrial Planet orbiting the White Dwarf (should NOT be treated as compact coalescence!)
    let planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Hot Earth".to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(0.001, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, (G_ASTRO * 1.02 / 0.001).sqrt())),
            Radius(EARTH_RADIUS_AU),
            RelativisticState::default(),
        ))
        .id();

    // 3. Moon of the planet with SatelliteOf (MUST skip relativistic kinematics)
    let moon_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Luna".to_string(),
            },
            SatelliteOf {
                parent: planet_ent,
                semi_major_axis_au: 0.0001,
                orbital_period_years: 0.05,
                true_anomaly: 0.0,
            },
            Mass(EARTH_MASS_SOLAR * 0.01),
            SimPosition(DVec3::new(0.0011, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Radius(EARTH_RADIUS_AU * 0.27),
            RelativisticState::default(),
        ))
        .id();

    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.01;
    app.update();

    // Verify: No GravitationalWaveMergerEvent was emitted for the normal planet
    let merger_messages = app
        .world()
        .resource::<Messages<GravitationalWaveMergerEvent>>();
    assert_eq!(
        merger_messages.len(),
        0,
        "Normal planet falling into White Dwarf must NOT emit GW merger event"
    );

    // Verify: Moon's RelativisticState accumulated_precession_rad remains 0.0 (skipped)
    let moon_rel = app.world().get::<RelativisticState>(moon_ent).unwrap();
    assert_eq!(
        moon_rel.accumulated_precession_rad, 0.0,
        "Moons with SatelliteOf must skip central relativistic precession"
    );
}
