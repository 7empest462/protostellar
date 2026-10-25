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
