//! Integration tests for Black Hole relativistic optics, devourment, accretion disk, tidal disruption, and Sagittarius A* scenario.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::relativity::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::*;
use protostellar::utils::constants::*;

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
    let v_vel = DVec3::new(0.0, 0.0, std::f64::consts::TAU);

    // 1. Verify 1PN acceleration is strictly attractive (inward toward origin, negative in x)
    let a_1pn = calculate_1pn_acceleration(r_pos, v_vel, m_supermassive);
    assert!(a_1pn.is_finite(), "1PN acceleration must be finite");
    assert!(
        a_1pn.x < 0.0,
        "Relativistic acceleration must be strictly attractive (inward, negative x, got {:.4e})",
        a_1pn.x
    );

    // 2. High-speed pericenter pass (10% speed of light)
    let v_high = DVec3::new(0.0, 0.0, 0.10 * SPEED_OF_LIGHT_AU_YR);
    let a_1pn_high = calculate_1pn_acceleration(r_pos, v_high, m_supermassive);
    assert!(a_1pn_high.is_finite());
    assert!(
        a_1pn_high.x < 0.0,
        "Relativistic acceleration at 10% c must remain attractive (inward, got {:.4e})",
        a_1pn_high.x
    );

    // 3. Close pericenter pass (0.5 AU)
    let r_close = DVec3::new(0.5, 0.0, 0.0);
    let a_1pn_close = calculate_1pn_acceleration(r_close, v_vel, m_supermassive);
    assert!(
        a_1pn_close.x < 0.0,
        "Relativistic acceleration close to horizon must remain attractive (inward, got {:.4e})",
        a_1pn_close.x
    );
    assert!(
        a_1pn_close.length() > a_1pn.length(),
        "Inward relativistic attraction must increase at closer distances"
    );
}

#[test]
fn test_black_hole_planets_maintain_steady_visual_radius_never_pulse() {
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

#[test]
fn test_s_star_tidal_disruption_and_black_hole_rigid_geometry() {
    use protostellar::rendering::bodies::transforms::compute_tidal_deformation_uniform;
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::TidalDisruptionPool;
    use protostellar::simulation::SimulationPlugin;
    use protostellar::utils::constants::SOLAR_RADIUS_AU;

    // 1. Verify black holes never deform via compute_tidal_deformation_uniform
    // When S4714 (2.0 M_sun) is at 0.31 AU from Sgr A* (4.3e6 M_sun):
    // The black hole's Roche stretch must be zero (mass_ratio 2.0 / 4.3e6 is tiny, not clamped to 1.0)
    let bh_uniform = compute_tidal_deformation_uniform(
        Vec3::ZERO,
        Vec3::new(0.31, 0.0, 0.0),
        2.0,   // primary mass = S4714 (2 M_sun)
        4.3e6, // body mass = Sgr A* (4.3e6 M_sun)
        0.085, // visual radius of Sgr A*
        None,
        None,
        false,
    );
    assert_eq!(
        bh_uniform.w, 0.0,
        "Black hole Roche stretch must be zero against minor star (got {})",
        bh_uniform.w
    );

    // 2. Plunge an S-star into Sgr A* at r = 0.5 AU (inside Rt ~ 1.14 AU)
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_plugins(SimulationPlugin);
    app.add_systems(
        Update,
        protostellar::rendering::effects::update_tidal_disruption_streams
            .after(protostellar::simulation::accretion::process_accretion_and_collisions),
    );

    // Run startup systems and clear default disk bodies before spawning test scenario
    app.update();
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
    }
    app.update();

    // Setup Sgr A* and plunging S4714
    let bh_entity = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Sagittarius A*".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(4.3e6),
            Radius(0.085),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Temperature(1e7),
            Composition::metal_rich(),
        ))
        .id();

    // S4714 placed at 0.5 AU (well inside tidal radius Rt ~ 1.14 AU)
    let s4714_entity = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "S4714 (Fastest Star)".to_string(),
                body_type: BodyType::MainSequenceStar,
            },
            Mass(2.0),
            Radius(1.9 * SOLAR_RADIUS_AU),
            SimPosition(DVec3::new(0.5, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 50.0)),
            SimAcceleration(DVec3::ZERO),
            Temperature(9500.0),
            Composition::pure_hydrogen(),
        ))
        .id();

    app.update();

    // S4714 must have been shredded into the TDE pool
    assert!(
        app.world().get_entity(s4714_entity).is_err(),
        "S4714 inside tidal disruption radius (0.5 AU < 1.14 AU) must be shredded into TDE"
    );

    let pool = app.world().resource::<TidalDisruptionPool>();
    assert!(
        !pool.streams.is_empty(),
        "TDE stream must be spawned when S-star is disrupted"
    );
    assert_eq!(pool.streams[0].bh_entity, bh_entity);
}

#[test]
fn test_milkomeda_natural_approach_merger() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_plugins(protostellar::simulation::SimulationPlugin);
    app.update();

    // Clear default bodies
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
    app.update();

    app.world_mut()
        .resource_mut::<ActiveScenarioState>()
        .current_preset = ScenarioPreset::GalacticCollision;
    let mut disk_params = DiskParameters::default();
    let mw_smbh = protostellar::simulation::scenarios::galaxy::spawn_galactic_collision_scenario(
        &mut app.world_mut().commands(),
        &mut disk_params,
    );
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(mw_smbh);
    app.world_mut().resource_mut::<TimeWarp>().multiplier = 2000.0; // Fast forward
    app.update();

    for _step in 0..100 {
        app.update();
        let elapsed = app.world().resource::<SimTime>().elapsed_years;

        let black_hole_count = app
            .world_mut()
            .query::<&CelestialBody>()
            .iter(app.world())
            .filter(|b| b.body_type == BodyType::BlackHole)
            .count();
        if black_hole_count == 0 {
            panic!("ALL BLACK HOLES WERE LOST at T = {elapsed:.2} yr!");
        }
    }

    let black_holes: Vec<(Entity, String, Mass)> = app
        .world_mut()
        .query::<(Entity, &CelestialBody, &Mass)>()
        .iter(app.world())
        .filter(|(_, b, _)| b.body_type == BodyType::BlackHole)
        .map(|(e, b, m)| (e, b.name.clone(), *m))
        .collect();
    assert_eq!(black_holes.len(), 1, "Expected 1 merged black hole");
    assert!(
        black_holes[0].1.contains("Milkomeda"),
        "Merged black hole must be named Milkomeda SMBH, got {}",
        black_holes[0].1
    );
    assert!(
        black_holes[0].2 .0 > 140_000_000.0,
        "Merged black hole must have combined mass, got {}",
        black_holes[0].2 .0
    );
    let selected = app
        .world()
        .resource::<PlayerInteractionState>()
        .selected_entity;
    assert_eq!(selected, Some(black_holes[0].0));
}

#[test]
fn test_sagittarius_a_ignition_and_aging_immunity() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let sgr_a_ent =
        spawn_sagittarius_a_star_scenario(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let (body, ignition, evo) = {
        let world = app.world();
        (
            world.get::<CelestialBody>(sgr_a_ent).unwrap().clone(),
            world.get::<IgnitionState>(sgr_a_ent).unwrap().clone(),
            world
                .get::<StellarEvolutionState>(sgr_a_ent)
                .unwrap()
                .clone(),
        )
    };

    assert_eq!(body.body_type, BodyType::BlackHole);
    assert!(body.name.contains("Sagittarius A*"));
    assert!(
        ignition.is_ignited,
        "Sagittarius A* must be initialized as an ignited/collapsed remnant"
    );
    assert_eq!(evo.phase, StellarEvolutionPhase::BlackHoleRemnant);
    assert!(body.body_type.is_remnant());
    assert!(!body.body_type.is_star());
    assert!(evo.phase.is_remnant());

    // 1. Hotkey 'I' via trigger_star_ignition_or_flare
    let mut toast = protostellar::game::ui::NotificationToast::default();
    {
        let mut state: bevy::ecs::system::SystemState<(
            Commands,
            Query<(&mut CelestialBody, &mut IgnitionState)>,
        )> = bevy::ecs::system::SystemState::new(app.world_mut());
        let (mut commands, mut query) = state.get_mut(app.world_mut()).unwrap();
        let (mut body, mut ignition) = query.get_mut(sgr_a_ent).unwrap();
        protostellar::game::interaction::body_tools::trigger_star_ignition_or_flare(
            &mut commands,
            sgr_a_ent,
            &mut body,
            &mut ignition,
            false,
            &mut toast,
        );
        state.apply(app.world_mut());
    }
    app.update();

    let body_after = app.world().get::<CelestialBody>(sgr_a_ent).unwrap();
    assert_eq!(
        body_after.body_type,
        BodyType::BlackHole,
        "Ignition hotkey must NOT convert black hole to star"
    );
    assert!(
        body_after.name.contains("Sagittarius A*"),
        "Name must be preserved"
    );
    assert!(
        toast.message.contains("Accretion Flare"),
        "Should trigger accretion flare toast: {}",
        toast.message
    );

    // 2. Editor toolbar action via handle_ignite_star
    {
        let mut state: bevy::ecs::system::SystemState<
            protostellar::game::ui::types::SelectedWorldQuery,
        > = bevy::ecs::system::SystemState::new(app.world_mut());
        let mut query = state.get_mut(app.world_mut()).unwrap();
        protostellar::game::ui::interactions::editor::stellar::handle_ignite_star(
            &mut query,
            Some(sgr_a_ent),
            &mut toast,
        );
    }
    let body_after_ignite = app.world().get::<CelestialBody>(sgr_a_ent).unwrap();
    assert_eq!(body_after_ignite.body_type, BodyType::BlackHole);

    // 3. Editor toolbar action via handle_age_star
    {
        let mut state: bevy::ecs::system::SystemState<
            protostellar::game::ui::types::SelectedWorldQuery,
        > = bevy::ecs::system::SystemState::new(app.world_mut());
        let mut query = state.get_mut(app.world_mut()).unwrap();
        protostellar::game::ui::interactions::editor::stellar::handle_age_star(
            &mut query,
            Some(sgr_a_ent),
            &mut toast,
        );
    }
    let body_after_age = app.world().get::<CelestialBody>(sgr_a_ent).unwrap();
    assert_eq!(body_after_age.body_type, BodyType::BlackHole);
    let mass_after_age = app.world().get::<Mass>(sgr_a_ent).unwrap();
    assert!(
        mass_after_age.0 > 4.0e6,
        "SMBH mass must not be clamped by star aging"
    );
    assert!(
        toast.message.contains("Terminal State"),
        "Should trigger terminal state toast: {}",
        toast.message
    );
}
