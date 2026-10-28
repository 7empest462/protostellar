//! Integration tests for Kerr black hole metric, frame dragging, ISCO shifts, and blowout dynamics.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::relativity::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::sagittarius::spawn_sagittarius_a_star_scenario;
use protostellar::simulation::scenarios::*;

#[test]
fn test_kerr_black_hole_event_horizon_and_ergosphere_boundaries() {
    let mass_solar = 10.0;
    let r_g = calculate_gravitational_radius(mass_solar);
    let r_s = calculate_schwarzschild_radius(mass_solar);

    assert!(
        (r_g * 2.0 - r_s).abs() < 1e-12,
        "Gravitational radius must be exactly half the Schwarzschild radius"
    );

    // 1. Schwarzschild limit (a_* = 0)
    let kerr_schwa = KerrBlackHole::schwarzschild(mass_solar);
    assert_eq!(kerr_schwa.spin_param, 0.0);
    assert!((kerr_schwa.event_horizon_radius_au - r_s).abs() < 1e-12);
    assert!((kerr_schwa.ergosphere_equatorial_radius_au - r_s).abs() < 1e-12);

    let r_e_pole_schwa = calculate_kerr_ergosphere_radius(mass_solar, 0.0, 0.0);
    let r_e_eq_schwa =
        calculate_kerr_ergosphere_radius(mass_solar, 0.0, std::f64::consts::FRAC_PI_2);
    assert!((r_e_pole_schwa - r_s).abs() < 1e-12);
    assert!((r_e_eq_schwa - r_s).abs() < 1e-12);

    // 2. Kerr rotating black hole (a_* = 0.90)
    let a_star = 0.90;
    let kerr_rot = KerrBlackHole::new(mass_solar, a_star, DVec3::Y);
    let r_plus = calculate_kerr_event_horizon(mass_solar, a_star);
    let expected_r_plus = r_g * (1.0 + (1.0 - a_star * a_star).sqrt());

    assert_eq!(kerr_rot.spin_param, a_star);
    assert!((kerr_rot.event_horizon_radius_au - r_plus).abs() < 1e-12);
    assert!(
        (r_plus - expected_r_plus).abs() < 1e-12,
        "Kerr event horizon r_+ must match analytical formula"
    );
    assert!(
        r_plus < r_s,
        "Kerr event horizon ({r_plus:.6}) must be strictly smaller than Schwarzschild radius ({r_s:.6})"
    );

    // Ergosphere: equatorial radius must remain 2 r_g = r_s
    let r_e_eq = calculate_kerr_ergosphere_radius(mass_solar, a_star, std::f64::consts::FRAC_PI_2);
    assert!(
        (r_e_eq - r_s).abs() < 1e-12,
        "Equatorial ergosphere radius must be exactly 2 r_g = r_s"
    );

    // Ergosphere: polar radius must touch the event horizon r_+
    let r_e_pole = calculate_kerr_ergosphere_radius(mass_solar, a_star, 0.0);
    assert!(
        (r_e_pole - r_plus).abs() < 1e-12,
        "Polar ergosphere radius must touch event horizon r_+"
    );

    // Ergosphere width at equator must be positive and equal to r_s - r_+
    let ergosphere_width = r_e_eq - r_plus;
    assert!(
        ergosphere_width > 0.0,
        "Ergosphere width at equator must be positive"
    );
}

#[test]
fn test_kerr_isco_analytical_shifts() {
    let mass_solar = 100.0;
    let r_g = calculate_gravitational_radius(mass_solar);

    // 1. Schwarzschild ISCO (a_* = 0 -> 6 r_g = 3 r_s)
    let isco_schwa = calculate_kerr_isco_radius(mass_solar, 0.0);
    assert!(
        (isco_schwa - 6.0 * r_g).abs() < 1e-6,
        "Schwarzschild ISCO must be 6 r_g (got {isco_schwa:.6}, expected {:.6})",
        6.0 * r_g
    );

    // 2. Near-extremal prograde ISCO (a_* -> +0.9999 -> approaches 1 r_g)
    let isco_prog = calculate_kerr_isco_radius(mass_solar, 0.9999);
    assert!(
        (isco_prog - r_g).abs() < 0.15 * r_g,
        "Near-extremal prograde ISCO must approach ~1 r_g (got {:.6}, r_g = {:.6})",
        isco_prog,
        r_g
    );
    assert!(
        isco_prog < isco_schwa,
        "Prograde ISCO must shrink dramatically relative to Schwarzschild"
    );

    // 3. Retrograde ISCO (a_* -> -0.9999 -> approaches 9 r_g)
    let isco_retro = calculate_kerr_isco_radius(mass_solar, -0.9999);
    assert!(
        (isco_retro - 9.0 * r_g).abs() < 0.15 * r_g,
        "Retrograde ISCO must approach ~9 r_g (got {:.6}, 9 r_g = {:.6})",
        isco_retro,
        9.0 * r_g
    );
    assert!(
        isco_retro > isco_schwa,
        "Retrograde ISCO must expand significantly relative to Schwarzschild"
    );

    // 4. Sagittarius A* calibrated ISCO (a_* = 0.90)
    let kerr_sgr = KerrBlackHole::sagittarius_a();
    let r_g_sgr = calculate_gravitational_radius(SGR_A_STAR_MASS_SOLAR);
    // For a_* = 0.90, r_isco ≈ 2.32 r_g
    let ratio = kerr_sgr.isco_radius_au / r_g_sgr;
    assert!(
        (ratio - 2.32).abs() < 0.10,
        "Sgr A* (a_* = 0.90) ISCO ratio should be ~2.32 r_g (got {ratio:.3})"
    );
}

#[test]
fn test_lense_thirring_frame_dragging_nodal_precession() {
    let mass_solar = SGR_A_STAR_MASS_SOLAR;
    let a_star = 0.90;
    let a_au = 150.0;
    let eccentricity = 0.50;

    // 1. Prograde precession rate
    let (rate_rad_yr, rate_cy) =
        calculate_lense_thirring_precession_rate(mass_solar, a_star, a_au, eccentricity);
    assert!(
        rate_rad_yr > 0.0,
        "Prograde Kerr spin must produce positive Lense-Thirring precession"
    );
    assert!(
        rate_cy > 0.0,
        "Arcseconds per century precession rate must be positive"
    );

    // 2. Non-rotating black hole must have zero Lense-Thirring precession
    let (rate_zero, rate_cy_zero) =
        calculate_lense_thirring_precession_rate(mass_solar, 0.0, a_au, eccentricity);
    assert_eq!(rate_zero, 0.0);
    assert_eq!(rate_cy_zero, 0.0);

    // 3. Retrograde Kerr spin must produce negative Lense-Thirring precession
    let (rate_retro, rate_cy_retro) =
        calculate_lense_thirring_precession_rate(mass_solar, -a_star, a_au, eccentricity);
    assert!(
        (rate_retro + rate_rad_yr).abs() < 1e-15,
        "Retrograde rate must be equal and opposite to prograde rate"
    );
    assert!((rate_cy_retro + rate_cy).abs() < 1e-12);

    // 4. Post-Newtonian gravito-magnetic frame-dragging acceleration
    let rel_pos = DVec3::new(10.0, 0.0, 0.0);
    let rel_vel = DVec3::new(0.0, 0.0, 50.0);
    let accel_lt =
        calculate_lense_thirring_acceleration(rel_pos, rel_vel, mass_solar, a_star, DVec3::Y);

    assert!(
        accel_lt.length_squared() > 0.0,
        "Lense-Thirring gravito-magnetic acceleration must be non-zero for spinning black hole"
    );

    let accel_schwa =
        calculate_lense_thirring_acceleration(rel_pos, rel_vel, mass_solar, 0.0, DVec3::Y);
    assert_eq!(
        accel_schwa,
        DVec3::ZERO,
        "Schwarzschild black hole must have zero gravito-magnetic acceleration"
    );
}

#[test]
fn test_sagittarius_a_scenario_initializes_kerr_metric() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<DiskParameters>();

    // Spawn Sagittarius A* scenario
    let mut disk_params = DiskParameters::default();
    spawn_sagittarius_a_star_scenario(&mut app.world_mut().commands(), &mut disk_params);

    // Run one update to flush commands
    app.update();

    let mut found_sgr_a = false;
    let mut query = app
        .world_mut()
        .query::<(&CelestialBody, &KerrBlackHole, &SpinState)>();
    for (body, kerr, spin) in query.iter(app.world()) {
        if body.name.contains("Sagittarius A*") {
            found_sgr_a = true;
            assert_eq!(body.body_type, BodyType::BlackHole);
            assert!(
                (kerr.spin_param - 0.90).abs() < 1e-6,
                "Sgr A* must be initialized with canonical Kerr spin a_* = 0.90"
            );
            assert!(
                kerr.event_horizon_radius_au > 0.0,
                "Event horizon radius must be positive"
            );
            assert!(
                kerr.ergosphere_equatorial_radius_au > kerr.event_horizon_radius_au,
                "Ergosphere equatorial radius must exceed event horizon radius"
            );
            assert!(
                kerr.isco_radius_au < calculate_isco_radius(SGR_A_STAR_MASS_SOLAR),
                "Kerr ISCO must be smaller than Schwarzschild ISCO (3 r_s)"
            );
            assert_eq!(spin.spin_vector, DVec3::Y);
        }
    }

    assert!(
        found_sgr_a,
        "Sagittarius A* black hole entity must exist with KerrBlackHole component"
    );
}

#[test]
fn test_little_red_dot_blowout_reveals_supermassive_black_hole() {
    let mut app = App::new();
    app.init_resource::<TimeWarp>();
    let mut sim_time = SimTime::default();
    sim_time.current_dt_yr = 0.1;
    app.insert_resource(sim_time);

    let bh_core_mass = 400_000.0;
    let cocoon_mass = 50_000.0;
    let total_mass = bh_core_mass + cocoon_mass;

    let q_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "JWST Little Red Dot (Black Hole Star)".to_string(),
                body_type: BodyType::QuasiStar,
            },
            Mass(total_mass),
            Radius(60.0),
            Temperature(3800.0),
            Luminosity(2.5e10),
            SimPosition(DVec3::ZERO),
            BlackHoleStarState {
                black_hole_mass_solar: bh_core_mass,
                cocoon_mass_solar: cocoon_mass,
                cocoon_radius_au: 60.0,
                eddington_ratio: 3.5,
                blowout_progress: 0.0,
                super_eddington_active: true,
                is_blown_out: false,
                accreted_envelope_mass: 0.0,
                jet_travel_distance_au: 0.0,
            },
            RelativisticJetState::quasi_star(),
        ))
        .id();

    // Verify initial intact state
    let body_before = app.world().get::<CelestialBody>(q_ent).unwrap();
    assert_eq!(body_before.body_type, BodyType::QuasiStar);
    let radius_before = app.world().get::<Radius>(q_ent).unwrap();
    assert_eq!(radius_before.0, 60.0);

    // Trigger blowout
    {
        let mut q_mut = app
            .world_mut()
            .get_mut::<BlackHoleStarState>(q_ent)
            .unwrap();
        q_mut.trigger_blowout();
    }

    app.add_systems(
        Update,
        protostellar::simulation::accretion::gas::update_black_hole_star_dynamics,
    );
    app.update();

    // Verify after blowout:
    // 1. Hydrogen shell blown off -> central body immediately revealed as Black Hole
    let body_after = app.world().get::<CelestialBody>(q_ent).unwrap();
    assert_eq!(
        body_after.body_type,
        BodyType::BlackHole,
        "Blown-out Little Red Dot must reveal central black hole seed"
    );
    assert!(
        body_after.name.contains("Supermassive Quasar"),
        "Name must transition to Quasar (got: {})",
        body_after.name
    );

    // 2. Physical radius must collapse from 60 AU to true Schwarzschild radius (~0.0079 AU)
    let radius_after = app.world().get::<Radius>(q_ent).unwrap();
    assert!(
        radius_after.0 < 0.05,
        "Central black hole physical radius must be compact Schwarzschild radius, not expanding cocoon (got: {:.4} AU)",
        radius_after.0
    );

    // 3. Quasar hyper-accretion jets must be actively firing and tightly collimated
    let jet_after = app.world().get::<RelativisticJetState>(q_ent).unwrap();
    assert!(
        jet_after.is_accreting,
        "Supermassive Quasar jets must be actively accreting upon blowout"
    );
    assert!(
        jet_after.opening_angle_rad <= 0.02,
        "Supermassive Quasar jets must be tightly collimated (<=0.02 rad), got: {}",
        jet_after.opening_angle_rad
    );
    assert_eq!(jet_after.lorentz_factor, 20.0);
    assert_eq!(jet_after.jet_length_au, 40.0);
}

#[test]
fn test_sagittarius_a_star_merger_preserves_canonical_name_never_milkomeda() {
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
    app.init_resource::<DiskParameters>();

    app.add_message::<AccretionMergeEvent>();
    app.add_message::<MoonFormationEvent>();
    app.add_message::<CollisionBounceEvent>();
    app.add_message::<RocheDisruptionEvent>();
    app.add_message::<TidalDisruptionEvent>();

    app.add_systems(Update, process_accretion_and_collisions);

    let mut disk_params = DiskParameters::default();
    let mut commands = app.world_mut().commands();
    let sgr_a_entity = spawn_sagittarius_a_star_scenario(&mut commands, &mut disk_params);

    // Spawn an infalling companion directly at ISCO to force a devourment/merger event
    commands.spawn((
        CelestialBody {
            name: "Plunging Test Infaller".to_string(),
            body_type: BodyType::Asteroid,
        },
        Mass(1.0),
        Radius(0.001),
        SimPosition(DVec3::new(0.01, 0.0, 0.0)),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Temperature(100.0),
        Composition::rocky(),
    ));

    app.update();

    // Verify Sgr A* swallowed or interacted, but its name remains "Sagittarius A* (Sgr A*)"
    let sgr_body = app
        .world()
        .get::<CelestialBody>(sgr_a_entity)
        .expect("Sgr A* must exist");
    assert_eq!(sgr_body.body_type, BodyType::BlackHole);
    assert_eq!(
        sgr_body.name, "Sagittarius A* (Sgr A*)",
        "Sagittarius A* must strictly preserve its canonical name and NEVER be renamed to Milkomeda"
    );
    assert!(
        !sgr_body.name.contains("Milkomeda"),
        "Sagittarius A* must never contain 'Milkomeda'"
    );
}

#[test]
fn test_milkomeda_andromeda_milky_way_merger_naming() {
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

    app.add_systems(Update, process_accretion_and_collisions);

    // Andromeda SMBH (140M M_sun) and Milky Way SMBH (4.3M M_sun)
    let andromeda_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Andromeda SMBH".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(140_000_000.0),
            Radius(2.76),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Temperature(1e-7),
            Composition::pure_hydrogen(),
        ))
        .id();

    let _mw_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Milky Way SMBH (Sagittarius A*)".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(4_300_000.0),
            Radius(0.085),
            SimPosition(DVec3::new(10.0, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Temperature(1e-7),
            Composition::pure_hydrogen(),
        ))
        .id();

    app.update();

    let merged_body = app
        .world()
        .get::<CelestialBody>(andromeda_ent)
        .expect("Merged black hole entity must exist");
    assert!(
        merged_body.name.contains("Milkomeda SMBH"),
        "Galactic collision merger must produce Milkomeda SMBH, got: {}",
        merged_body.name
    );
    assert_eq!(
        merged_body.name,
        "Merged Supermassive Black Hole (Milkomeda SMBH - 144.3M M☉)"
    );
}
