//! Visual hierarchy and circumbinary scenario orbital stability tests.

use bevy::prelude::*;

#[test]
fn test_trappist1_visual_hierarchy_and_spacing() {
    use protostellar::simulation::components::BodyType;
    use protostellar::simulation::resources::SimulationConfig;
    use protostellar::utils::constants::EARTH_RADIUS_AU;

    let config = SimulationConfig::default();
    let min_orbit_au = 0.01154f32; // TRAPPIST-1b orbit

    // Star: TRAPPIST-1 (R = 0.121 R_sun = 0.000563 AU)
    let star_phys_r = 0.000_563_f64;
    let star_vis_r =
        config.calc_visual_radius_with_orbit(star_phys_r, BodyType::RedDwarf, 0.0, min_orbit_au);

    // Planet b: R = 1.116 R_earth, a = 0.01154 AU
    let b_phys_r = 1.116 * EARTH_RADIUS_AU;
    let b_vis_r = config.calc_visual_radius_with_orbit(
        b_phys_r,
        BodyType::TerrestrialPlanet,
        min_orbit_au,
        min_orbit_au,
    );

    // Planet c: R = 1.097 R_earth, a = 0.01580 AU
    let c_phys_r = 1.097 * EARTH_RADIUS_AU;
    let c_vis_r = config.calc_visual_radius_with_orbit(
        c_phys_r,
        BodyType::TerrestrialPlanet,
        0.01580,
        min_orbit_au,
    );

    // Planet d: R = 0.788 R_earth, a = 0.02227 AU
    let d_phys_r = 0.788 * EARTH_RADIUS_AU;
    let d_vis_r = config.calc_visual_radius_with_orbit(
        d_phys_r,
        BodyType::TerrestrialPlanet,
        0.02227,
        min_orbit_au,
    );

    // Planet g: R = 1.129 R_earth, a = 0.04686 AU
    let g_phys_r = 1.129 * EARTH_RADIUS_AU;
    let g_vis_r = config.calc_visual_radius_with_orbit(
        g_phys_r,
        BodyType::TerrestrialPlanet,
        0.04686,
        min_orbit_au,
    );

    // 1. Star must be visibly dominant over planets (at least 4.0x larger in visual diameter)
    let ratio_star_to_b = star_vis_r / b_vis_r;
    let ratio_star_to_g = star_vis_r / g_vis_r;
    let ratio_star_to_d = star_vis_r / d_vis_r;

    assert!(
        ratio_star_to_b >= 4.0,
        "Star must be at least 4.0x wider than planet b (ratio: {ratio_star_to_b:.2}x)"
    );
    assert!(
        ratio_star_to_g >= 4.0,
        "Star must be at least 4.0x wider than largest planet g (ratio: {ratio_star_to_g:.2}x)"
    );
    assert!(
        ratio_star_to_d >= 5.0,
        "Star must be at least 5.0x wider than small planet d (ratio: {ratio_star_to_d:.2}x)"
    );

    // 2. Relative physical size differences between planets must be preserved
    assert!(
        g_vis_r > d_vis_r,
        "Planet g (1.13 R_earth) must be visibly larger than planet d (0.79 R_earth)"
    );
    assert!(
        b_vis_r > d_vis_r,
        "Planet b (1.12 R_earth) must be visibly larger than planet d (0.79 R_earth)"
    );

    // 3. Clear, dark space between adjacent planetary orbits
    // Distance between orbit b (0.01154) and orbit c (0.01580) is 0.00426 AU
    let orbit_gap_bc = 0.01580 - 0.01154;
    let surface_gap_bc = orbit_gap_bc - (b_vis_r + c_vis_r);
    assert!(
        surface_gap_bc > 2.0 * b_vis_r,
        "Surface gap between b and c must be at least 2x the planet diameter (surface gap: {:.5} AU vs 2*r={:.5} AU)",
        surface_gap_bc,
        2.0 * b_vis_r
    );

    // Solar system planets must NOT be modified by compact scaling
    let earth_normal =
        config.calc_visual_radius_for_type(EARTH_RADIUS_AU, BodyType::TerrestrialPlanet);
    let earth_with_orbit = config.calc_visual_radius_with_orbit(
        EARTH_RADIUS_AU,
        BodyType::TerrestrialPlanet,
        1.0,
        0.387,
    );
    assert_eq!(
        earth_normal, earth_with_orbit,
        "Solar system planets must retain standard visual scaling"
    );
}

#[test]
fn test_kepler_16_circumbinary_multi_orbit_stability() {
    use bevy::math::DVec3;
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::kepler::spawn_kepler_16_system;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    let config = SimulationConfig {
        gas_density_scale: 0.0,
        ..Default::default()
    };
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .insert_resource(SimTime {
            current_dt_yr: 0.002,
            ..default()
        })
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::Kepler16Circumbinary,
            ..default()
        })
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<PlayerInteractionState>()
        .add_message::<protostellar::simulation::thermodynamics::StarIgnitionEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_message::<MoonFormationEvent>()
        .add_systems(
            Update,
            (
                step_physics_simulation,
                process_accretion_and_collisions.after(step_physics_simulation),
            ),
        );

    let mut disk_params = DiskParameters::default();
    let _star_a = spawn_kepler_16_system(&mut app.world_mut().commands(), &mut disk_params);
    *app.world_mut().resource_mut::<DiskParameters>() = disk_params;

    for _ in 0..1000 {
        app.update();
    }

    let mut q = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &SimVelocity)>();
    let bodies: Vec<(String, DVec3, DVec3)> = q
        .iter(app.world())
        .map(|(b, p, v)| (b.name.clone(), p.0, v.0))
        .collect();

    let summary: Vec<String> = bodies
        .iter()
        .map(|(n, p, _)| format!("{n}@{:.4}", p.length()))
        .collect();
    assert_eq!(
        bodies.len(),
        5,
        "Kepler-16 stars (A/B), giant (b), moon (b I), and habitable world (c) must all survive; remaining: {summary:?}"
    );

    let mut checked = 0;
    for (name, pos, vel) in &bodies {
        assert!(
            pos.is_finite() && vel.is_finite(),
            "Non-finite state for {name}"
        );
        let r = pos.length();
        if name.contains("Kepler-16B") {
            checked += 1;
            assert!(
                (0.20..=0.25).contains(&r),
                "Binary companion Kepler-16B must remain bound near 0.2243 AU (got {r:.4} AU)"
            );
        } else if name.contains("Kepler-16b (") {
            checked += 1;
            assert!(
                (0.60..=0.81).contains(&r),
                "Circumbinary giant Kepler-16b must remain bound near 0.7048 AU (got {r:.4} AU)"
            );
        } else if name.contains("Kepler-16c") {
            checked += 1;
            assert!(
                (1.02..=1.28).contains(&r),
                "Habitable planet Kepler-16c must remain bound near 1.15 AU (got {r:.4} AU)"
            );
        }
    }
    assert_eq!(checked, 3);
}
