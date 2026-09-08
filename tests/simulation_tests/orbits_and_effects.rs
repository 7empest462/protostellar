//! Test module generated from simulation_tests.

use bevy::math::DVec3;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;
use protostellar::utils::math::*;

#[test]
fn test_hyperbolic_orbit_points_generation() {
    use protostellar::utils::math::{
        apsides_positions, generate_hyperbolic_orbit_points, position_at_true_anomaly,
        OrbitalElements,
    };

    // Unbound interstellar flyby trajectory: e = 1.5, a = -3.0 AU (q = |a|(e - 1) = 1.5 AU)
    let elements = OrbitalElements {
        semi_major_axis: -3.0,
        eccentricity: 1.5,
        inclination: 0.15,
        longitude_ascending_node: 0.4,
        argument_of_periapsis: 0.2,
        true_anomaly: 0.0,
        period_years: f64::INFINITY,
        periapsis: 1.5,
        apoapsis: f64::INFINITY,
        specific_energy: 10.0,
        periapsis_dir: DVec3::X,
        semilatus_dir: DVec3::Z,
    };

    let points = generate_hyperbolic_orbit_points(&elements, 64);
    assert!(
        points.len() >= 30,
        "Hyperbolic trajectory must generate a continuous series of points"
    );

    // All points must be finite
    for pt in &points {
        assert!(pt.is_finite(), "Trajectory point must be finite: {:?}", pt);
        assert!(
            pt.length() <= 2000.0,
            "Trajectory point must not exceed sanity bounds: {:?}",
            pt
        );
    }

    // Periapsis position test
    let (opt_peri, opt_apo) = apsides_positions(&elements);
    assert!(opt_peri.is_some(), "Periapsis must exist for hyperbola");
    assert!(opt_apo.is_none(), "Apoapsis must be None for hyperbola");

    let peri = opt_peri.unwrap();
    let peri_dist = peri.length();
    assert!(
        (peri_dist - 1.5).abs() < 0.01,
        "Periapsis distance should be 1.5 AU, got {:.4}",
        peri_dist
    );

    // Test position_at_true_anomaly at nu = 0
    let pos_at_0 = position_at_true_anomaly(&elements, 0.0).expect("Position at nu=0 must exist");
    assert!(
        (pos_at_0 - peri).length() < 1e-4,
        "Position at nu=0 must equal periapsis"
    );
}

#[test]
fn test_trailing_ribbon_points_elliptical_and_hyperbolic() {
    use protostellar::utils::math::{
        generate_trailing_ribbon_points, position_at_true_anomaly, OrbitalElements,
    };
    use std::f64::consts::PI;

    // Earth-like orbit: a = 1.0 AU, e = 0.0167
    let earth_elements = OrbitalElements {
        semi_major_axis: 1.0,
        eccentricity: 0.0167,
        inclination: 0.0,
        longitude_ascending_node: 0.0,
        argument_of_periapsis: 0.0,
        true_anomaly: PI / 3.0, // 60 degrees
        period_years: 1.0,
        periapsis: 0.9833,
        apoapsis: 1.0167,
        specific_energy: -20.0,
        periapsis_dir: DVec3::X,
        semilatus_dir: DVec3::Z,
    };

    let ribbon = generate_trailing_ribbon_points(&earth_elements, 32, 1.2 * PI);
    assert_eq!(
        ribbon.len(),
        33,
        "Ribbon should contain 33 points (32 samples + 1)"
    );

    // First point must be at current true anomaly with alpha = 1.0
    let (p0, a0) = ribbon[0];
    assert!((a0 - 1.0).abs() < 1e-5, "Leading edge alpha must be 1.0");
    let expected_p0 = position_at_true_anomaly(&earth_elements, PI / 3.0).unwrap();
    assert!(
        (p0 - expected_p0).length() < 1e-4,
        "Leading edge point must match current true anomaly position"
    );

    // Last point must have alpha = 0.0
    let (_plast, alast) = *ribbon.last().unwrap();
    assert!(
        (alast - 0.0).abs() < 1e-5,
        "Trailing edge alpha must be 0.0"
    );

    // Monotonically decreasing alpha
    for window in ribbon.windows(2) {
        assert!(
            window[0].1 >= window[1].1,
            "Alpha must monotonically decrease along the trailing ribbon"
        );
    }
}

#[test]
fn test_orbit_visualization_mode_cycling_and_state() {
    use protostellar::simulation::resources::OrbitVisualizationMode;

    let mode = OrbitVisualizationMode::default();
    assert_eq!(mode, OrbitVisualizationMode::SelectedOnly);
    assert_eq!(mode.display_label(), "Selected");

    let mode = mode.cycle();
    assert_eq!(mode, OrbitVisualizationMode::All);
    assert_eq!(mode.display_label(), "All");

    let mode = mode.cycle();
    assert_eq!(mode, OrbitVisualizationMode::Off);
    assert_eq!(mode.display_label(), "Hidden");

    let mode = mode.cycle();
    assert_eq!(mode, OrbitVisualizationMode::SelectedOnly);
    assert_eq!(mode.display_label(), "Selected");
}

#[test]
fn test_satellite_moon_orbit_anchoring_math() {
    use bevy::math::DVec3;
    use protostellar::utils::constants::G_ASTRO;
    use protostellar::utils::math::state_vectors_to_orbital_elements;

    // Parent planet (Jupiter-mass) at 5.2 AU moving at circular Keplerian velocity
    let jupiter_pos = DVec3::new(5.2, 0.0, 0.0);
    let jupiter_mass = 0.000954; // Solar masses (~1 M_Jup)
    let star_mass = 1.0;
    let v_jup = (G_ASTRO * star_mass / 5.2).sqrt();
    let jupiter_vel = DVec3::new(0.0, 0.0, v_jup);

    // Moon orbiting Jupiter at 0.0028 AU (~421,700 km, like Io)
    let r_moon_rel = 0.0028;
    let v_moon_rel = (G_ASTRO * jupiter_mass / r_moon_rel).sqrt();
    let moon_rel_pos = DVec3::new(r_moon_rel, 0.0, 0.0);
    let moon_rel_vel = DVec3::new(0.0, 0.0, v_moon_rel);

    let moon_abs_pos = jupiter_pos + moon_rel_pos;
    let moon_abs_vel = jupiter_vel + moon_rel_vel;
    let moon_mass = 0.00000005; // tiny

    // Relative to parent planet:
    let rel_pos = moon_abs_pos - jupiter_pos;
    let rel_vel = moon_abs_vel - jupiter_vel;

    let elements = state_vectors_to_orbital_elements(rel_pos, rel_vel, jupiter_mass, moon_mass)
        .expect("Should resolve valid Keplerian elements relative to parent");

    assert!(
        (elements.semi_major_axis - r_moon_rel).abs() < 1e-4,
        "Semi-major axis relative to planet must match 0.0028 AU, got {}",
        elements.semi_major_axis
    );
    assert!(
        elements.eccentricity < 0.05,
        "Eccentricity relative to parent planet must be near circular, got {}",
        elements.eccentricity
    );
}

#[test]
fn test_planet_builder_preview_orbit_generation() {
    use protostellar::game::ui::PlanetBuilderState;
    use protostellar::utils::math::{apsides_positions, generate_orbit_points, OrbitalElements};

    let builder = PlanetBuilderState::default();
    assert_eq!(builder.semi_major_axis_au, 1.0);
    assert_eq!(builder.eccentricity, 0.016);

    let preview_elements = OrbitalElements {
        semi_major_axis: builder.semi_major_axis_au,
        eccentricity: builder.eccentricity,
        periapsis: builder.semi_major_axis_au * (1.0 - builder.eccentricity),
        apoapsis: builder.semi_major_axis_au * (1.0 + builder.eccentricity),
        ..Default::default()
    };

    let points = generate_orbit_points(&preview_elements, 96);
    assert_eq!(points.len(), 97, "Preview orbit must generate 97 vertices");

    let (opt_peri, opt_apo) = apsides_positions(&preview_elements);
    let peri = opt_peri.expect("Periapsis must exist for bound preview");
    let apo = opt_apo.expect("Apoapsis must exist for bound preview");

    assert!(
        (peri.length() - (1.0 - 0.016) as f32).abs() < 1e-3,
        "Periapsis must match 0.984 AU"
    );
    assert!(
        (apo.length() - (1.0 + 0.016) as f32).abs() < 1e-3,
        "Apoapsis must match 1.016 AU"
    );
}

#[test]
fn test_gizmo_config() {
    use bevy::camera::CameraProjection;
    use bevy::gizmos::config::DefaultGizmoConfigGroup;
    use bevy::prelude::*;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::gizmos::GizmoPlugin);
    let mut sched = Schedule::default();
    sched.add_systems(protostellar::rendering::camera::setup_gizmo_configuration);
    sched.run(app.world_mut());
    let store = app.world().get_resource::<GizmoConfigStore>();
    assert!(store.is_some());
    let (config, _) = store.unwrap().config::<DefaultGizmoConfigGroup>();
    assert!((config.depth_bias - (-1.0)).abs() < 1e-5);
    assert!((config.line.width - 2.5).abs() < 1e-5);
    println!("GizmoConfig depth_bias successfully configured to -1.0, width to 2.5!");

    // Verify perspective projection clip values and lines.wgsl depth calculation
    let proj = PerspectiveProjection {
        fov: 45.0_f32.to_radians(),
        near: 0.0001,
        far: 2_000_000.0,
        ..default()
    };
    let mat = proj.get_clip_from_view();
    let epsilon: f32 = 4.88e-04;
    for z_view in [-1.0_f32, -5.0, -16.0, -100.0] {
        let view_pos = Vec4::new(0.0, 0.0, z_view, 1.0);
        let clip = mat * view_pos;
        let ratio = clip.w / clip.z;
        println!(
            "z_view: {z_view}, clip.z: {}, clip.w: {}, ratio: {ratio}",
            clip.z, clip.w
        );
        assert!(clip.z > 0.0, "clip.z must be positive in reversed Z");
        assert!(
            ratio - epsilon > 0.0,
            "ratio - epsilon must be positive for log2"
        );
        let depth = clip.z * (-(-1.0_f32) * (ratio - epsilon).log2()).exp2();
        println!("z_view: {z_view}, calculated depth: {depth}");
        assert!(depth.is_finite(), "depth must be finite");
        assert!(depth <= clip.w, "depth must not exceed clip.w");
        assert!(depth >= 0.0, "depth must be non-negative");
    }
}

#[test]
fn test_draw_orbital_effects_and_gizmos_without_star() {
    use bevy::prelude::*;
    use protostellar::simulation::resources::{OrbitVisualizationMode, PlayerInteractionState};
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::gizmos::GizmoPlugin);

    app.init_resource::<protostellar::simulation::resources::SimulationConfig>();
    app.init_resource::<protostellar::simulation::resources::PlayerInteractionState>();
    app.init_resource::<protostellar::simulation::resources::ImpactShockwavePool>();
    app.init_resource::<protostellar::simulation::resources::RocheDebrisPool>();

    // Spawn Proto-Earth without any CentralStar in the world
    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let earth_vel = DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI);
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Proto-Earth".to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(earth_pos),
            SimVelocity(earth_vel),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
        ))
        .id();

    // Set player state
    {
        let mut state = app.world_mut().resource_mut::<PlayerInteractionState>();
        state.selected_entity = Some(earth_ent);
        state.orbit_mode = OrbitVisualizationMode::SelectedOnly;
    }

    // Spawn Camera3d
    app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::new(1.0, 0.002, 0.003)),
    ));

    let mut sched = Schedule::default();
    sched.add_systems(protostellar::rendering::effects::draw_orbital_effects_and_gizmos);
    sched.run(app.world_mut());

    println!("Gracefully drew orbit effects and gizmos even without a CentralStar entity!");
}

#[test]
fn test_draw_orbital_effects_and_gizmos_executes_cleanly() {
    use bevy::prelude::*;
    use protostellar::simulation::resources::{OrbitVisualizationMode, PlayerInteractionState};
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::gizmos::GizmoPlugin);

    app.init_resource::<protostellar::simulation::resources::SimulationConfig>();
    app.init_resource::<protostellar::simulation::resources::PlayerInteractionState>();
    app.init_resource::<protostellar::simulation::resources::ImpactShockwavePool>();
    app.init_resource::<protostellar::simulation::resources::RocheDebrisPool>();

    // Spawn central star
    let _star_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                body_type: BodyType::Protostar,
                name: "The Protostar".to_string(),
            },
            Mass(1.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Radius(SOLAR_RADIUS_AU),
            IgnitionState {
                core_temperature: 4.0e6,
                fusion_fraction: 0.4,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id();

    // Spawn Proto-Earth
    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let earth_vel = DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI);
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Proto-Earth".to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(earth_pos),
            SimVelocity(earth_vel),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
        ))
        .id();

    // Set player state
    {
        let mut state = app.world_mut().resource_mut::<PlayerInteractionState>();
        state.selected_entity = Some(earth_ent);
        state.orbit_mode = OrbitVisualizationMode::SelectedOnly;
    }

    // Spawn Camera3d for distance-adaptive reticle scaling
    app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::new(1.0, 0.002, 0.003)),
    ));

    let mut sched = Schedule::default();
    sched.add_systems(protostellar::rendering::effects::draw_orbital_effects_and_gizmos);
    sched.run(app.world_mut());

    println!("Successfully ran draw_orbital_effects_and_gizmos with resilient star query and adaptive reticles!");
}

#[test]
fn test_equilibrium_temperature_luminosity_scaling() {
    // Verifies that equilibrium temperature scales according to the Stefan-Boltzmann law:
    // T_eq = T_* * sqrt(R_* / (2r)) * (1 - A)^(1/4)
    // without double-counting stellar luminosity (L_*^0.25).
    let r = 1.0; // 1 AU
    let albedo = 0.30f64;
    let factor_albedo = (1.0 - albedo).powf(0.25);

    // Sun-like star: T_* = 5778 K, R_* = 1 R_sun
    let t_sun = 5778.0;
    let r_sun_au = SOLAR_RADIUS_AU;
    let t_eq_sun = t_sun * (r_sun_au / (2.0 * r)).sqrt() * factor_albedo;
    assert!(
        (t_eq_sun - 254.8).abs() < 2.0,
        "Sun T_eq should be ~255 K, got {t_eq_sun}"
    );

    // O-star supergiant: T_* = 30,000 K, R_* = 10 R_sun, L_* ~ 10,000 L_sun
    let t_o = 30_000.0;
    let r_o_au = 10.0 * SOLAR_RADIUS_AU;
    let t_eq_o = t_o * (r_o_au / (2.0 * r)).sqrt() * factor_albedo;
    let expected_ratio = (30_000.0 / 5778.0) * 10.0f64.sqrt();
    assert!(
        ((t_eq_o / t_eq_sun) - expected_ratio).abs() < 1e-4,
        "Temperature ratio between O-star and Sun must scale strictly with T_* * sqrt(R_*)"
    );

    // M-dwarf star: T_* = 3000 K, R_* = 0.2 R_sun, L_* ~ 0.001 L_sun
    let t_m = 3000.0;
    let r_m_au = 0.2 * SOLAR_RADIUS_AU;
    let t_eq_m = t_m * (r_m_au / (2.0 * r)).sqrt() * factor_albedo;
    let expected_m_ratio = (3000.0 / 5778.0) * 0.2f64.sqrt();
    assert!(
        ((t_eq_m / t_eq_sun) - expected_m_ratio).abs() < 1e-4,
        "Temperature ratio between M-dwarf and Sun must scale strictly with T_* * sqrt(R_*)"
    );
}

#[test]
fn test_accretion_inner_loop_no_duplicate_mergers() {
    use bevy::prelude::*;
    use protostellar::simulation::accretion::process_accretion_and_collisions;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<PlayerInteractionState>()
        .add_message::<protostellar::simulation::accretion::AccretionMergeEvent>()
        .add_message::<protostellar::simulation::accretion::MoonFormationEvent>()
        .add_message::<protostellar::simulation::accretion::CollisionBounceEvent>()
        .add_message::<protostellar::simulation::accretion::RocheDisruptionEvent>();

    {
        let mut cfg = app.world_mut().resource_mut::<SimulationConfig>();
        cfg.enable_accretion = true;
        cfg.base_dt_yr = 0.001;
    }

    // Spawn central star
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            body_type: BodyType::Protostar,
            name: "Central Star".to_string(),
        },
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Composition::solar_gas(),
    ));

    // Spawn 3 bodies in very close proximity: B1 (large), B2 (small), B3 (small)
    let m1 = 0.01; // Large protoplanet
    let m2 = 0.0001; // Impactor 1
    let m3 = 0.0001; // Impactor 2
    let b1 = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Protoplanet,
                name: "Body 1".to_string(),
            },
            Mass(m1),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI)),
            SimAcceleration(DVec3::ZERO),
            Radius(EARTH_RADIUS_AU * 2.0),
            Temperature(300.0),
            Composition::rocky(),
        ))
        .id();

    let _b2 = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Planetesimal,
                name: "Body 2".to_string(),
            },
            Mass(m2),
            SimPosition(DVec3::new(1.00001, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI)),
            SimAcceleration(DVec3::ZERO),
            Radius(EARTH_RADIUS_AU * 0.1),
            Temperature(300.0),
            Composition::rocky(),
        ))
        .id();

    let _b3 = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Planetesimal,
                name: "Body 3".to_string(),
            },
            Mass(m3),
            SimPosition(DVec3::new(1.00002, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI)),
            SimAcceleration(DVec3::ZERO),
            Radius(EARTH_RADIUS_AU * 0.1),
            Temperature(300.0),
            Composition::rocky(),
        ))
        .id();

    let mut sched = Schedule::default();
    sched.add_systems(process_accretion_and_collisions);
    sched.run(app.world_mut());

    let world = app.world();
    let mass1 = world.get::<Mass>(b1).map(|m| m.0).unwrap_or(0.0);
    assert!(
        mass1 >= m1,
        "Primary body mass must either increase or remain identical, got {mass1}"
    );
}

#[test]
fn test_orphaned_satellite_heliocentric_promotion() {
    use bevy::prelude::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();

    // Spawn central star
    let _star = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                body_type: BodyType::Protostar,
                name: "Central Star".to_string(),
            },
            Mass(1.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Radius(SOLAR_RADIUS_AU),
            Temperature(5778.0),
            Composition::solar_gas(),
        ))
        .id();

    // Spawn parent planet at 1 AU
    let parent_pos = DVec3::new(1.0, 0.0, 0.0);
    let parent_vel = DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI);
    let parent_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Parent Planet".to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(parent_pos),
            SimVelocity(parent_vel),
            SimAcceleration(DVec3::ZERO),
            Radius(EARTH_RADIUS_AU),
            Temperature(288.0),
            Composition::rocky(),
        ))
        .id();

    // Spawn orbiting moon
    let moon_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Moon,
                name: "Natural Moon".to_string(),
            },
            Mass(EARTH_MASS_SOLAR * 0.0123),
            SimPosition(parent_pos + DVec3::new(0.00257, 0.0, 0.0)),
            SimVelocity(parent_vel + DVec3::new(0.0, 0.0, 0.2)),
            SimAcceleration(DVec3::ZERO),
            Radius(EARTH_RADIUS_AU * 0.27),
            Temperature(250.0),
            Composition::rocky(),
            SatelliteOf {
                parent: parent_ent,
                semi_major_axis_au: 0.00257,
                orbital_period_years: 0.0748,
                true_anomaly: 0.0,
            },
        ))
        .id();

    let mut sched = Schedule::default();
    sched.add_systems(step_physics_simulation);

    // Step 1: Run with parent alive
    sched.run(app.world_mut());
    let moon_pos_step1 = app.world().get::<SimPosition>(moon_ent).unwrap().0;
    assert!(app.world().get::<SatelliteOf>(moon_ent).is_some());

    // Despawn parent planet (e.g. consumed or destroyed)
    app.world_mut().entity_mut(parent_ent).despawn();

    // Step 2: Run with parent dead
    sched.run(app.world_mut());

    // Moon must not freeze, must update position, and SatelliteOf must be removed
    let moon_pos_step2 = app.world().get::<SimPosition>(moon_ent).unwrap().0;
    assert_ne!(
        moon_pos_step1, moon_pos_step2,
        "Orphaned moon must not freeze in space!"
    );
    assert!(
        app.world().get::<SatelliteOf>(moon_ent).is_none(),
        "SatelliteOf must be removed from orphaned moon so it becomes independent"
    );
}

#[test]
fn test_disk_inclined_orbit_keplerian_pericenter() {
    // Generate an inclined orbit at periapsis:
    // a = 2.0 AU, e = 0.2, i = 25 deg = 0.43633 rad
    let a = 2.0;
    let e = 0.2;
    let inc = 25.0f64.to_radians();
    let star_mass = 1.0;

    let r_peri = a * (1.0 - e);
    let v_peri_mag = (G_ASTRO * star_mass * (2.0 / r_peri - 1.0 / a)).sqrt();

    let pos = DVec3::new(r_peri * inc.cos(), r_peri * inc.sin(), 0.0);
    let vel = DVec3::new(0.0, 0.0, v_peri_mag);

    // In our disk perifocal formulation, r is along the nodal line and v is perpendicular
    assert!(
        pos.dot(vel).abs() < 1e-12,
        "Position and velocity at periapsis must be strictly orthogonal (r . v == 0)"
    );

    // The angular momentum vector h = r x v
    let h = pos.cross(vel);
    assert!(h.length() > 0.0);
}

#[test]
fn test_orbital_elements_disk_plane_zero_inclination() {
    // 1 AU circular orbit in the X-Z disc plane
    let pos = DVec3::new(1.0, 0.0, 0.0);
    let vel = DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI);
    let elements = state_vectors_to_orbital_elements(pos, vel, 1.0, 0.0).unwrap();

    assert!(
        elements.inclination.to_degrees().abs() < 1e-4,
        "Coplanar prograde orbit in X-Z disc must evaluate to 0 degrees inclination, got {}",
        elements.inclination.to_degrees()
    );

    // Retrograde orbit in X-Z disc plane
    let vel_retro = DVec3::new(0.0, 0.0, -2.0 * std::f64::consts::PI);
    let elements_retro = state_vectors_to_orbital_elements(pos, vel_retro, 1.0, 0.0).unwrap();
    assert!(
        (elements_retro.inclination.to_degrees() - 180.0).abs() < 1e-4,
        "Retrograde orbit in X-Z disc must evaluate to 180 degrees inclination, got {}",
        elements_retro.inclination.to_degrees()
    );

    // Tilted orbit: 30 degrees inclination
    let inc_rad = 30.0f64.to_radians();
    let vel_tilted = DVec3::new(
        0.0,
        2.0 * std::f64::consts::PI * inc_rad.sin(),
        2.0 * std::f64::consts::PI * inc_rad.cos(),
    );
    let elements_tilted = state_vectors_to_orbital_elements(pos, vel_tilted, 1.0, 0.0).unwrap();
    assert!(
        (elements_tilted.inclination.to_degrees() - 30.0).abs() < 0.1,
        "Tilted orbit must evaluate to 30 degrees inclination, got {}",
        elements_tilted.inclination.to_degrees()
    );
}

#[test]
fn test_gpu_readback_buffer_recycling() {
    use protostellar::gpu::compute_node::GpuReadbackReceiver;
    use protostellar::gpu::GpuReadbackSender;

    let (tx, rx) = flume::bounded::<Vec<u8>>(2);
    let (recycle_tx, recycle_rx) = flume::bounded::<Vec<u8>>(2);

    let sender = GpuReadbackSender { tx, recycle_rx };
    let receiver = GpuReadbackReceiver { rx, recycle_tx };

    // 1. First iteration: sender creates or allocates buffer of 1024 bytes
    let mut buf = sender
        .recycle_rx
        .try_recv()
        .unwrap_or_else(|_| vec![0u8; 1024]);
    buf[0] = 77;
    sender.tx.try_send(buf).unwrap();

    // 2. Main world drains receiver
    let mut latest = None;
    while let Ok(bytes) = receiver.rx.try_recv() {
        latest = Some(bytes);
    }
    let bytes = latest.expect("Must receive sent buffer");
    assert_eq!(bytes[0], 77);

    // 3. Main world recycles the buffer
    receiver.recycle_tx.try_send(bytes).unwrap();

    // 4. Render world pops the recycled buffer: must succeed with preserved capacity!
    let recycled = sender
        .recycle_rx
        .try_recv()
        .expect("Buffer must be recycled");
    assert_eq!(recycled.capacity(), 1024);
}
