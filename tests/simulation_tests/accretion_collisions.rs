//! Collision momentum and center-of-mass conservation integration tests.

#[test]
fn test_giant_impact_moon_conserves_com_and_linear_momentum() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::{
        AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
    };
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<MoonFormationEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_systems(Update, process_accretion_and_collisions);

    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CentralStar,
    ));

    let comp = Composition::rocky();
    let earth_mass = 0.88 * EARTH_MASS_SOLAR;
    let earth_rad = EARTH_RADIUS_AU;
    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let earth_vel = DVec3::new(0.2, 0.0, std::f64::consts::TAU);

    let theia_mass = 0.15 * EARTH_MASS_SOLAR;
    let theia_rad = EARTH_RADIUS_AU * 0.55;
    let theia_pos = DVec3::new(1.0 + earth_rad * 0.75, 0.0, earth_rad * 0.35);
    let theia_vel = DVec3::new(-0.9, 0.0, std::f64::consts::TAU + 1.8);

    let p_initial = earth_vel * earth_mass + theia_vel * theia_mass;
    let mr_initial = earth_pos * earth_mass + theia_pos * theia_mass;
    let m_initial = earth_mass + theia_mass;

    let mut earth_diff = InternalDifferentiation::default();
    earth_diff.recalculate(earth_mass, earth_rad, &comp);
    let earth_ent = app
        .world_mut()
        .spawn((
            SimPosition(earth_pos),
            SimVelocity(earth_vel),
            SimAcceleration(DVec3::ZERO),
            Mass(earth_mass),
            Radius(earth_rad),
            Temperature(290.0),
            comp,
            earth_diff,
            CelestialBody {
                name: "Proto-Earth".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    let mut theia_diff = InternalDifferentiation::default();
    theia_diff.recalculate(theia_mass, theia_rad, &comp);
    let theia_ent = app
        .world_mut()
        .spawn((
            SimPosition(theia_pos),
            SimVelocity(theia_vel),
            SimAcceleration(DVec3::ZERO),
            Mass(theia_mass),
            Radius(theia_rad),
            Temperature(310.0),
            comp,
            theia_diff,
            CelestialBody {
                name: "Theia".to_string(),
                body_type: BodyType::Protoplanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    app.update();

    let moon_events = app.world().resource::<Messages<MoonFormationEvent>>();
    assert_eq!(moon_events.len(), 1, "Giant impact must form a Moon");

    let (m1, p1, v1) = {
        let w = app.world();
        (
            w.get::<Mass>(earth_ent).unwrap().0,
            w.get::<SimPosition>(earth_ent).unwrap().0,
            w.get::<SimVelocity>(earth_ent).unwrap().0,
        )
    };
    let (m2, p2, v2, sat) = {
        let w = app.world();
        (
            w.get::<Mass>(theia_ent).unwrap().0,
            w.get::<SimPosition>(theia_ent).unwrap().0,
            w.get::<SimVelocity>(theia_ent).unwrap().0,
            w.get::<SatelliteOf>(theia_ent).expect("moon SatelliteOf"),
        )
    };

    let m_final = m1 + m2;
    let p_final = v1 * m1 + v2 * m2;
    let mr_final = p1 * m1 + p2 * m2;

    assert!(
        ((m_final - m_initial) / m_initial).abs() < 1e-12,
        "Total mass must be conserved: init={m_initial}, final={m_final}"
    );
    assert!(
        (p_final - p_initial).length() / p_initial.length() < 1e-12,
        "Total linear momentum must be conserved: init={p_initial:?}, final={p_final:?}"
    );
    assert!(
        (mr_final - mr_initial).length() / mr_initial.length() < 1e-12,
        "Center of mass position must be conserved: init={mr_initial:?}, final={mr_final:?}"
    );

    let expected_v_rel = (G_ASTRO * m_final / sat.semi_major_axis_au).sqrt();
    let actual_v_rel = (v2 - v1).length();
    assert!(
        ((actual_v_rel - expected_v_rel) / expected_v_rel).abs() < 1e-9,
        "Relative orbital speed ({actual_v_rel}) must equal 2-body Keplerian speed ({expected_v_rel})"
    );
}

#[test]
fn test_grazing_bounce_preserves_momentum_and_weighted_com() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::{
        AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
    };
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<MoonFormationEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_systems(Update, process_accretion_and_collisions);

    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CentralStar,
    ));

    let comp = Composition::rocky();
    let m1_val = 4.0 * EARTH_MASS_SOLAR;
    let m2_val = 1.0 * EARTH_MASS_SOLAR;
    let r1_au = EARTH_RADIUS_AU * 1.6;
    let r2_au = EARTH_RADIUS_AU * 1.0;

    // Spawn secondary (lighter) FIRST and primary (heavier) SECOND to also verify that
    // `handle_grazing_bounce` orients impulses outward regardless of ECS iteration order.
    let cfg = SimulationConfig::default();
    let vis1 =
        f64::from(cfg.calc_visual_radius_with_orbit(r1_au, BodyType::TerrestrialPlanet, 2.0, 2.0));
    let vis2 =
        f64::from(cfg.calc_visual_radius_with_orbit(r2_au, BodyType::TerrestrialPlanet, 2.0, 2.0));
    let r_contact = ((vis1 + vis2) * 0.40).max(r1_au + r2_au);

    let pos1 = DVec3::new(2.0, 0.0, 0.0);
    let vel1 = DVec3::new(0.5, 0.0, 0.2);
    let pos2 = pos1 + DVec3::new(0.0, 0.0, r_contact * 0.92);
    let vel2 = vel1 + DVec3::new(4.0, 0.0, -1.5);

    let p_initial = vel1 * m1_val + vel2 * m2_val;
    let mr_initial = pos1 * m1_val + pos2 * m2_val;

    let e2 = app
        .world_mut()
        .spawn((
            SimPosition(pos2),
            SimVelocity(vel2),
            SimAcceleration(DVec3::ZERO),
            Mass(m2_val),
            Radius(r2_au),
            Temperature(300.0),
            comp,
            CelestialBody {
                name: "Exoplanet Beta".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SpinState::default(),
        ))
        .id();
    let e1 = app
        .world_mut()
        .spawn((
            SimPosition(pos1),
            SimVelocity(vel1),
            SimAcceleration(DVec3::ZERO),
            Mass(m1_val),
            Radius(r1_au),
            Temperature(300.0),
            comp,
            CelestialBody {
                name: "Exoplanet Alpha".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SpinState::default(),
        ))
        .id();

    app.update();

    let bounce_events = app.world().resource::<Messages<CollisionBounceEvent>>();
    assert_eq!(
        bounce_events.len(),
        1,
        "High-speed oblique exoplanet collision must trigger CollisionBounceEvent"
    );

    let w = app.world();
    let p1_after = w.get::<SimPosition>(e1).unwrap().0;
    let v1_after = w.get::<SimVelocity>(e1).unwrap().0;
    let p2_after = w.get::<SimPosition>(e2).unwrap().0;
    let v2_after = w.get::<SimVelocity>(e2).unwrap().0;

    let p_final = v1_after * m1_val + v2_after * m2_val;
    let mr_final = p1_after * m1_val + p2_after * m2_val;

    assert!(
        (p_final - p_initial).length() / p_initial.length() < 1e-12,
        "Bounce impulse must conserve total linear momentum: init={p_initial:?}, final={p_final:?}"
    );
    assert!(
        (mr_final - mr_initial).length() / mr_initial.length() < 1e-12,
        "Mass-weighted separation must preserve center of mass: init={mr_initial:?}, final={mr_final:?}"
    );
    assert!(
        (p1_after - p2_after).dot(v1_after - v2_after) > 0.0,
        "Bodies must move apart along contact normal after bounce"
    );
    assert!(
        (p1_after - p2_after).length() >= r_contact,
        "Mass-weighted separation must push bodies cleanly outside contact"
    );
}
