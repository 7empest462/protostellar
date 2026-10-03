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
fn test_black_hole_visual_radius_and_optical_properties() {
    let config = SimulationConfig::default();

    // 1. Stellar-mass black hole (5 M_sun, physical R_s ~ 1.47e-4 AU)
    let m_stellar = 5.0;
    let r_phys_stellar = calculate_schwarzschild_radius(m_stellar);
    let vis_r_stellar = config.calc_visual_radius_for_type(r_phys_stellar, BodyType::BlackHole);

    // Sun reference visual radius
    let sun_vis_r = config.calc_visual_radius_for_type(SOLAR_RADIUS_AU, BodyType::YellowDwarf);

    // Stellar-mass black hole must be distinctly compact compared to the Sun (smaller in diameter)
    assert!(
        vis_r_stellar < sun_vis_r,
        "Stellar-mass black hole ({vis_r_stellar:.4} AU) must be smaller than the Sun ({sun_vis_r:.4} AU)"
    );
    assert!(
        vis_r_stellar >= 0.0020,
        "Stellar-mass black hole must respect minimum threshold"
    );

    // 2. Supermassive black hole (450,000 M_sun, physical R_s ~ 13.3 AU)
    let m_supermassive = 450_000.0;
    let r_phys_sm = calculate_schwarzschild_radius(m_supermassive);
    let vis_r_sm = config.calc_visual_radius_for_type(r_phys_sm, BodyType::BlackHole);

    assert!(
        vis_r_sm > vis_r_stellar,
        "Supermassive black hole must be substantially larger than stellar-mass"
    );
    assert!(
        vis_r_sm <= 2.50,
        "Supermassive black hole must respect maximum clamp of 2.50 AU"
    );
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
fn test_black_hole_scaling_and_devourment() {
    let config = SimulationConfig::default();

    // 1. Physical Schwarzschild radius: Rs = 1.974e-8 AU / M_sun (~2.953 km / M_sun)
    let m_solar_10 = 10.0;
    let r_phys_10 = 1.974e-8 * m_solar_10;
    let r_km_10 = r_phys_10 * AU_TO_KM;
    assert!(
        (r_km_10 - 29.53).abs() < 0.1,
        "10 M_sun black hole physical radius must be ~29.53 km (got {r_km_10:.2} km)"
    );

    let m_solar_1600 = 1600.0;
    let r_phys_1600 = 1.974e-8 * m_solar_1600;
    let r_km_1600 = r_phys_1600 * AU_TO_KM;
    assert!(
        (r_km_1600 - 4725.0).abs() < 10.0,
        "1600 M_sun black hole physical radius must be ~4725 km (got {r_km_1600:.2} km)"
    );

    // 2. Visual radius scaling: 5 M_sun (~0.0022 AU), 1600 M_sun (~0.020 AU)
    let vis_5 = config.calc_visual_radius_for_type(1.974e-8 * 5.0, BodyType::BlackHole);
    let vis_10 = config.calc_visual_radius_for_type(1.974e-8 * 10.0, BodyType::BlackHole);
    let vis_1600 = config.calc_visual_radius_for_type(1.974e-8 * 1600.0, BodyType::BlackHole);

    assert!(
        vis_5 < vis_10,
        "Visual radius must scale monotonically with mass"
    );
    assert!(
        vis_10 < vis_1600,
        "Visual radius must scale monotonically with mass"
    );
    assert!(
        (vis_5 - 0.0022).abs() < 0.0005,
        "5 M_sun visual radius should be ~0.0022 AU (got {vis_5:.4})"
    );
    assert!(
        (vis_1600 - 0.020).abs() < 0.003,
        "1600 M_sun visual radius should be ~0.020 AU (got {vis_1600:.4})"
    );

    // 3. Black hole visual radius must not be suppressed by compact orbit scaling
    let vis_orbit_compact =
        config.calc_visual_radius_with_orbit(1.974e-8 * 10.0, BodyType::BlackHole, 0.05, 0.05);
    assert_eq!(
        vis_10, vis_orbit_compact,
        "Black hole visual radius must not be suppressed by compact orbit scaling"
    );
}

#[test]
fn test_supermassive_seed_attraction_never_repels() {
    let m_supermassive = 100_000.0; // 100,000 M_sun supermassive seed
    let r_pos = DVec3::new(10.0, 0.0, 0.0);
    let v_vel = DVec3::new(0.0, 0.0, 6.28);

    // 1. Verify 1PN acceleration is strictly attractive (inward toward origin, negative in x)
    let a_1pn = calculate_1pn_acceleration(r_pos, v_vel, m_supermassive);
    assert!(a_1pn.is_finite(), "1PN acceleration must be finite");
    assert!(
        a_1pn.x < 0.0,
        "1PN acceleration must point INWARD toward the supermassive black hole, got {:?}",
        a_1pn
    );
    assert_eq!(a_1pn.y, 0.0);
    assert_eq!(a_1pn.z, 0.0);
    assert!(
        a_1pn.dot(r_pos) < 0.0,
        "1PN force must be purely attractive and never repel"
    );

    // 2. Verify close separation (1 AU) also yields strictly inward acceleration
    let r_pos_close = DVec3::new(1.0, 0.0, 0.0);
    let a_1pn_close = calculate_1pn_acceleration(r_pos_close, v_vel, m_supermassive);
    assert!(
        a_1pn_close.x < 0.0,
        "Close-range 1PN acceleration must point INWARD toward black hole, got {:?}",
        a_1pn_close
    );
    assert!(
        a_1pn_close.length() > a_1pn.length(),
        "Inward relativistic attraction must increase at closer distances"
    );
}

#[test]
fn test_black_hole_planets_maintain_steady_visual_radius_never_pulse() {
    use protostellar::simulation::resources::SimulationConfig;
    use protostellar::utils::constants::EARTH_RADIUS_AU;

    let config = SimulationConfig::default();

    let earth_phys_r = EARTH_RADIUS_AU;
    let earth_normal =
        config.calc_visual_radius_for_type(earth_phys_r, BodyType::TerrestrialPlanet);

    // 1. Earth at 1.0 AU orbiting a 15 M_sun black hole with plunging debris at varying distances
    let plunging_distances = [0.001f32, 0.005, 0.01, 0.03, 0.08, 0.12, 0.14, 0.20, 0.50];
    for &min_r in &plunging_distances {
        let vis_r = config.calc_visual_radius_with_orbit(
            earth_phys_r,
            BodyType::TerrestrialPlanet,
            1.0,
            min_r,
        );
        assert_eq!(
            vis_r, earth_normal,
            "Earth (1.0 AU) must maintain steady visual radius and never shrink or pulse when inner debris plunges to {:.3} AU",
            min_r
        );
    }

    // 2. The Moon (1.0 AU), Pluto (39.5 AU), Planet 9 (400 AU) must also remain unperturbed
    let moon_phys_r = 0.272 * EARTH_RADIUS_AU;
    let moon_normal = config.calc_visual_radius_for_type(moon_phys_r, BodyType::Moon);
    let vis_moon = config.calc_visual_radius_with_orbit(moon_phys_r, BodyType::Moon, 1.0, 0.008);
    assert_eq!(vis_moon, moon_normal, "The Moon must not shrink or pulse");

    let pluto_phys_r = 0.186 * EARTH_RADIUS_AU;
    let pluto_normal =
        config.calc_visual_radius_for_type(pluto_phys_r, BodyType::TerrestrialPlanet);
    let vis_pluto = config.calc_visual_radius_with_orbit(
        pluto_phys_r,
        BodyType::TerrestrialPlanet,
        39.5,
        0.005,
    );
    assert_eq!(vis_pluto, pluto_normal, "Pluto must not shrink or pulse");

    // 3. Close-in bodies transitioning between 0.06 AU and 0.15 AU must scale monotonically and smoothly
    let mut prev_vis = 0.0f32;
    for step in 0..=10 {
        let r_orb = 0.06 + (0.15 - 0.06) * (step as f32 / 10.0);
        let vis = config.calc_visual_radius_with_orbit(
            earth_phys_r,
            BodyType::TerrestrialPlanet,
            r_orb,
            0.01,
        );
        assert!(
            vis >= prev_vis,
            "Visual radius must increase monotonically across transition zone: {:.4} >= {:.4} at r_orb = {:.3}",
            vis, prev_vis, r_orb
        );
        prev_vis = vis;
    }
}

#[test]
fn test_black_hole_accretion_disk_spawning_and_sync() {
    use protostellar::rendering::bodies::structures::{
        sync_black_hole_accretion_disks, VisualBlackHoleDiskChild,
    };
    use protostellar::rendering::bodies::{VisualAssets, VisualBody};
    use protostellar::rendering::materials::BlackHoleDiskMaterial;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<BlackHoleDiskMaterial>>();
    app.init_resource::<SimTime>();

    // Setup VisualAssets using dummy fallback mesh
    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let fallback = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
    app.insert_resource(VisualAssets::dummy(fallback));

    app.add_systems(Update, sync_black_hole_accretion_disks);

    // Spawn a Black Hole entity
    let bh_entity = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Sagittarius A*".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(4_000_000.0),
            Radius(0.08),
            Transform::from_xyz(0.0, 0.0, 0.0),
            VisualBody,
        ))
        .id();

    // Run system once
    app.update();

    // Verify child entity was spawned with VisualBlackHoleDiskChild
    let mut disk_child_query = app.world_mut().query::<(
        Entity,
        &Transform,
        &MeshMaterial3d<BlackHoleDiskMaterial>,
        &VisualBlackHoleDiskChild,
    )>();
    let disk_results: Vec<_> = disk_child_query.iter(app.world()).collect();
    assert_eq!(
        disk_results.len(),
        1,
        "Exactly one accretion disk child must be spawned for the black hole"
    );

    let (disk_ent, disk_trans, mat_handle, _) = disk_results[0];
    assert_eq!(
        disk_trans.scale,
        Vec3::new(12.0, 1.0, 12.0),
        "Accretion disk mesh scale must be 12.0x event horizon in XZ plane"
    );

    // Verify material uniforms
    let disk_materials = app.world().resource::<Assets<BlackHoleDiskMaterial>>();
    let material = disk_materials
        .get(mat_handle)
        .expect("Disk material asset must exist");
    assert!(
        (material.uniforms.inner_radius - 0.16).abs() < 1e-4,
        "ISCO inner radius must be ~0.16"
    );
    assert_eq!(
        material.uniforms.spin_axis.w, 4_000_000.0,
        "Spin axis w-component must encode BH mass"
    );
    assert!(
        (material.uniforms.schwa_radius - 0.055).abs() < 1e-4,
        "Schwarzschild radius ratio must be ~0.055"
    );

    // Verify parent-child hierarchy
    let child_of = app
        .world()
        .get::<ChildOf>(disk_ent)
        .expect("Disk must be parented to BH via ChildOf");
    assert_eq!(
        child_of.parent(),
        bh_entity,
        "Disk parent must be the black hole entity"
    );
}

#[test]
fn test_black_hole_tidal_disruption_spaghetti_stream() {
    use protostellar::rendering::effects::tde::update_tidal_disruption_streams;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Time<()>>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<TidalDisruptionPool>();

    app.add_message::<AccretionMergeEvent>();
    app.add_message::<MoonFormationEvent>();
    app.add_message::<CollisionBounceEvent>();
    app.add_message::<RocheDisruptionEvent>();
    app.add_message::<TidalDisruptionEvent>();

    app.add_systems(
        Update,
        (
            process_accretion_and_collisions,
            update_tidal_disruption_streams.after(process_accretion_and_collisions),
        ),
    );

    // Spawn 10.0 M_sun stellar-mass Black Hole at origin
    let bh_entity = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Cygnus X-1".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(10.0),
            Radius(1.974e-7),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Temperature(10.0),
            Composition::metal_rich(),
        ))
        .id();

    // Spawn planet orbiting close to black hole inside tidal radius
    let planet_entity = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Doomed Exoplanet".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(0.0001, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 20.0)),
            SimAcceleration(DVec3::ZERO),
            Temperature(300.0),
            Composition::rocky(),
        ))
        .id();

    // Step the simulation
    app.update();

    // 1. Black hole must have accreted the doomed exoplanet's mass
    let bh_mass = app.world().get::<Mass>(bh_entity).expect("BH must exist");
    assert!(
        (bh_mass.0 - (10.0 + EARTH_MASS_SOLAR)).abs() < 1e-10,
        "Black hole must accrete planet mass"
    );

    // 2. Planet must be despawned from the ECS
    assert!(
        app.world().get_entity(planet_entity).is_err(),
        "Disrupted planet must be despawned cleanly"
    );

    // 3. TDE Pool must contain an active spaghetti plasma stream
    let pool = app.world().resource::<TidalDisruptionPool>();
    assert_eq!(
        pool.streams.len(),
        1,
        "Exactly one TDE stream must be active"
    );

    let stream = &pool.streams[0];
    assert_eq!(stream.bh_entity, bh_entity);
    assert!(
        stream.r_tidal > stream.r_isco,
        "Tidal radius must be outside ISCO"
    );
    assert!(
        !stream.stream_nodes.is_empty(),
        "Stream must contain relativistic plasma nodes"
    );

    // 4. Verify node geometry wraps inwards along spiral geodesic
    let first_node = stream.stream_nodes.first().unwrap();
    let last_node = stream.stream_nodes.last().unwrap();
    assert!(
        first_node.1 > last_node.1,
        "Plasma nodes must spiral inward toward ISCO"
    );
    assert!(
        (last_node.2 - first_node.2).abs() > std::f32::consts::PI * 2.0,
        "Stream must wrap around the black hole more than a full revolution"
    );
}

#[test]
fn test_sagittarius_a_star_scenario_and_s_star_relativity() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<DiskParameters>();
    app.init_resource::<RelativityConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<GravitationalWaveMergerEvent>();
    app.add_systems(Update, update_relativity_evolution);

    // Spawn Sagittarius A* scenario
    {
        let mut disk_params = DiskParameters::default();
        let mut commands = app.world_mut().commands();
        spawn_sagittarius_a_star_scenario(&mut commands, &mut disk_params);
    }
    app.update();

    // 1. Verify central supermassive black hole Sgr A*
    let mut bh_query = app
        .world_mut()
        .query::<(&CelestialBody, &Mass, &Radius, &CentralStar)>();
    let (bh_body, bh_mass, bh_radius, _) = bh_query
        .iter(app.world())
        .next()
        .expect("Sgr A* must be spawned as central star");

    assert_eq!(bh_body.body_type, BodyType::BlackHole);
    assert!(bh_body.name.contains("Sagittarius A*"));
    assert!(
        (bh_mass.0 - SGR_A_STAR_MASS_SOLAR).abs() < 1.0,
        "Sgr A* mass must be ~4.3e6 M_sun"
    );
    assert!(
        bh_radius.0 > 0.08 && bh_radius.0 < 0.09,
        "Sgr A* Schwarzschild radius must be ~0.0848 AU"
    );

    // 2. Verify all S-stars, Pistol Star, and G1/G2 clouds are spawned (18 companions)
    let mut stars_query = app.world_mut().query_filtered::<(
        &CelestialBody,
        &RelativisticState,
        &SimPosition,
        &SimVelocity,
    ), Without<CentralStar>>();
    let all_s_bodies: Vec<_> = stars_query.iter(app.world()).collect();
    assert_eq!(
        all_s_bodies.len(),
        18,
        "Expected 18 S-star cluster companions"
    );

    // Verify Pistol Star is spawned as Hypergiant landmark
    let pistol = all_s_bodies
        .iter()
        .find(|(b, _, _, _)| b.name.contains("Pistol Star"))
        .expect("Pistol Star must exist");
    assert_eq!(pistol.0.body_type, BodyType::Hypergiant);

    // Verify G1 and G2 dust/gas clouds are present
    assert!(all_s_bodies
        .iter()
        .any(|(b, _, _, _)| b.name.contains("G1")));
    assert!(all_s_bodies
        .iter()
        .any(|(b, _, _, _)| b.name.contains("G2")));

    // 3. Test S4714: Fastest known star (12.8°/orbit precession, pericenter speed ~8% c)
    let s4714 = all_s_bodies
        .iter()
        .find(|(b, _, _, _)| b.name.contains("S4714"))
        .expect("S4714 must exist");

    let s4714_state = s4714.1;
    let s4714_advance_deg = s4714_state.precession_advance_per_orbit_rad.to_degrees();
    assert!(
        s4714_advance_deg > 10.0 && s4714_advance_deg < 15.0,
        "S4714 1PN advance must be ~12.8 deg/orbit (got {s4714_advance_deg:.2}°)"
    );
    assert!(
        s4714_state.beta_v_over_c > 0.05,
        "S4714 pericenter speed must exceed 5% c (got {:.2}% c)",
        s4714_state.beta_v_over_c * 100.0
    );
    assert!(
        s4714_state.total_redshift_z > 0.001,
        "S4714 total relativistic redshift must exceed 0.001 (got {:.4})",
        s4714_state.total_redshift_z
    );

    // 4. Test S2: Benchmark star (1PN precession rate ~12 arcmin per orbit)
    let s2 = all_s_bodies
        .iter()
        .find(|(b, _, _, _)| b.name.contains("S2"))
        .expect("S2 must exist");

    let s2_advance_arcmin = s2.1.precession_advance_per_orbit_rad.to_degrees() * 60.0;
    assert!(
        s2_advance_arcmin > 8.0 && s2_advance_arcmin < 16.0,
        "S2 1PN advance must be ~12 arcmin/orbit (got {s2_advance_arcmin:.2}')"
    );
    assert!(
        s2.1.total_redshift_z > 0.0001,
        "S2 must display measurable relativistic redshift"
    );
}

#[test]
fn test_sagittarius_s4714_orbital_stability_and_bound_retention() {
    use protostellar::simulation::resources::DiskParameters;
    use protostellar::simulation::scenarios::sagittarius::spawn_sagittarius_a_star_scenario;
    use protostellar::simulation::SimulationPlugin;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_plugins(SimulationPlugin);

    app.update();

    // Clear default protoplanetary disk bodies before spawning Sagittarius A* scenario
    {
        let entities_to_despawn: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<CelestialBody>>()
            .iter(app.world())
            .collect();
        for entity in entities_to_despawn {
            if let Ok(mut cmd) = app.world_mut().commands().get_entity(entity) {
                cmd.despawn();
            }
        }
        let mut disk_params = DiskParameters::default();
        let mut commands = app.world_mut().commands();
        spawn_sagittarius_a_star_scenario(&mut commands, &mut disk_params);
    }
    app.update();

    // Step physics for multiple frames (simulating full orbital progression)
    for _ in 0..250 {
        app.update();
    }

    // Verify S4714 distance from Sgr A* remains bound in its orbit (< 300 AU)
    let mut stars_query = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &SimVelocity)>();

    let mut found_s4714 = false;
    for (body, pos, vel) in stars_query.iter(app.world()) {
        if body.name.contains("S4714") {
            found_s4714 = true;
            let dist = pos.0.length();
            assert!(
                dist < 300.0,
                "S4714 must remain bound in its orbit around Sgr A* (dist={dist:.2} AU, expected < 300 AU)"
            );
            assert!(
                dist > 0.5,
                "S4714 periapsis must remain outside event horizon (dist={dist:.2} AU)"
            );
            assert!(
                vel.0.length() > 0.0 && vel.0.length() < 30_000.0,
                "S4714 velocity must be finite and physically bound (v={:.1} AU/yr)",
                vel.0.length()
            );
        }
    }
    assert!(found_s4714, "S4714 must be present in the simulation");
}
