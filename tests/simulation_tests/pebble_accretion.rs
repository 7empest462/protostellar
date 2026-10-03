//!
//! Pebble accretion, streaming instability, and orbital accretion test module.

#[test]
fn test_snow_line_pebble_pileup_density() {
    use protostellar::simulation::pebble_accretion::local_pebble_sigma;

    let snow_line = 2.7;
    let gas_scale = 1.0;

    let sigma_at_snow = local_pebble_sigma(snow_line, gas_scale, snow_line);
    let sigma_outside = local_pebble_sigma(3.8, gas_scale, snow_line);

    assert!(
        sigma_at_snow > sigma_outside,
        "Pebble surface density at the snow line ({:.4}) must be enhanced over distant regions ({:.4})",
        sigma_at_snow,
        sigma_outside
    );
}

#[test]
fn test_pebble_drift_velocity_inward() {
    use protostellar::simulation::pebble_accretion::pebble_drift_velocity_au_yr;

    let v_drift = pebble_drift_velocity_au_yr(2.0, 1.0);
    assert!(
        v_drift < 0.0,
        "Pebble drift velocity must be negative (inward toward the star)"
    );
    assert!(
        v_drift > -10.0,
        "Pebble drift velocity must be reasonable within the disk"
    );
}

#[test]
fn test_asteroid_comet_pebble_accretion() {
    use protostellar::simulation::components::BodyType;
    use protostellar::simulation::pebble_accretion::{is_pebble_target, pebble_accretion_rate};
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    assert!(is_pebble_target(BodyType::Asteroid));
    assert!(is_pebble_target(BodyType::Comet));

    let ast_mass = 1e-6 * EARTH_MASS_SOLAR;
    let rate_ast = pebble_accretion_rate(ast_mass, 2.5, 1.0, 1.0, 2.7, BodyType::Asteroid);
    assert!(
        rate_ast > 0.0,
        "Asteroids must sweep pebbles and have positive accretion rate"
    );

    let comet_mass = 1e-7 * EARTH_MASS_SOLAR;
    let rate_comet = pebble_accretion_rate(comet_mass, 20.0, 1.0, 1.0, 2.7, BodyType::Comet);
    assert!(
        rate_comet > 0.0,
        "Comets must sweep pebbles and have positive accretion rate"
    );
}

#[test]
fn test_asteroid_promotion_to_planetesimal_via_pebbles() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::pebble_accretion::apply_pebble_accretion;
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let mut app = App::new();
    let mut config = SimulationConfig::default();
    config.enable_accretion = true;
    config.gas_density_scale = 1.0;
    config.base_dt_yr = 0.5;
    app.insert_resource(config);

    let mut time_warp = TimeWarp::default();
    time_warp.multiplier = 1.0;
    app.insert_resource(time_warp);

    let mut sim_time = SimTime::default();
    sim_time.elapsed_years = 100.0;
    app.insert_resource(sim_time);

    let mut disk_params = DiskParameters::default();
    disk_params.central_star_mass = 1.0;
    disk_params.gas_disk_lifetime_yr = 5_000_000.0;
    disk_params.snow_line_au = 2.7;
    app.insert_resource(disk_params);

    let init_mass = 0.00099 * EARTH_MASS_SOLAR;
    let ast_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Asteroid,
                name: "Asteroid #1".to_string(),
            },
            Mass(init_mass),
            SimPosition(DVec3::new(2.5, 0.0, 0.0)),
            Radius(1e-5),
            Composition::rocky(),
        ))
        .id();

    app.add_systems(Update, apply_pebble_accretion);
    app.update();

    let mass = app.world().get::<Mass>(ast_ent).unwrap();
    let body = app.world().get::<CelestialBody>(ast_ent).unwrap();

    assert!(mass.0 > init_mass, "Mass must increase from pebble sweep");
    assert_eq!(
        body.body_type,
        BodyType::Planetesimal,
        "Asteroid exceeding 0.001 M_earth must promote to Planetesimal"
    );
    assert!(
        body.name.contains("Planetesimal"),
        "Body name must update upon promotion: {}",
        body.name
    );
}

#[test]
fn test_streaming_instability_spawns_asteroids_and_comets() {
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::planetesimals::PlanetesimalSpawner;
    use protostellar::simulation::pebble_accretion::spawn_streaming_instability_minor_bodies;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    let mut config = SimulationConfig::default();
    config.gas_density_scale = 1.0;
    app.insert_resource(config);

    app.insert_resource(TimeWarp::default());

    let mut sim_time = SimTime::default();
    sim_time.elapsed_years = 500.0;
    app.insert_resource(sim_time);

    let mut disk_params = DiskParameters::default();
    disk_params.central_star_mass = 1.0;
    disk_params.gas_disk_lifetime_yr = 4_000_000.0;
    disk_params.snow_line_au = 2.7;
    app.insert_resource(disk_params);

    let mut spawner = PlanetesimalSpawner::default();
    spawner.last_spawn_yr = 0.0;
    spawner.max_ecs_bodies = 50;
    app.insert_resource(spawner);

    app.add_systems(Update, spawn_streaming_instability_minor_bodies);
    app.update();

    let mut query = app.world_mut().query::<&CelestialBody>();
    let bodies: Vec<CelestialBody> = query.iter(app.world()).cloned().collect();

    assert!(
        bodies.len() >= 3 && bodies.len() <= 6,
        "Streaming instability must spawn a burst of 3 to 6 minor bodies in belts, got {}",
        bodies.len()
    );
    for spawned in &bodies {
        assert!(
            matches!(
                spawned.body_type,
                BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal
            ),
            "Spawned body must be an Asteroid, Comet, or Planetesimal, found {:?}",
            spawned.body_type
        );
    }
}

#[test]
fn test_silicate_pebble_fragmentation_barrier() {
    use protostellar::simulation::pebble_accretion::{
        local_pebble_sigma, silicate_fragmentation_barrier,
    };

    let snow_line = 2.7;
    let gas_scale = 1.0;

    // Inside snow line: dry silicates fragment easily, suppressing pebble flux
    let barrier_inside = silicate_fragmentation_barrier(1.0, snow_line);
    assert!(
        barrier_inside < 0.30,
        "Silicate pebble barrier must suppress pebble density by >= 70%: got {:.3}",
        barrier_inside
    );

    // Beyond snow line: sticky icy pebbles maintain full cohesion
    let barrier_outside = silicate_fragmentation_barrier(3.5, snow_line);
    assert!(
        (barrier_outside - 1.0).abs() < 1e-6,
        "Silicate pebble barrier outside snow line must be 1.0: got {:.3}",
        barrier_outside
    );

    let sigma_1au = local_pebble_sigma(1.0, gas_scale, snow_line);
    let sigma_snow = local_pebble_sigma(snow_line, gas_scale, snow_line);
    assert!(
        sigma_snow > sigma_1au,
        "Snow line pebble condensation trap must exceed fragmented 1 AU dry silicate density"
    );
}

#[test]
fn test_inner_terrestrial_mass_ceiling_no_runaway() {
    use protostellar::simulation::components::BodyType;
    use protostellar::simulation::pebble_accretion::{
        pebble_accretion_rate, pebble_isolation_mass_solar,
    };
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let r_earth = 1.0;
    let m_iso = pebble_isolation_mass_solar(r_earth);
    assert!(
        m_iso <= 1.0 * EARTH_MASS_SOLAR,
        "Terrestrial pebble isolation mass must stall at <= 1.0 M_earth: got {:.4}",
        m_iso / EARTH_MASS_SOLAR
    );

    let rate_sub = pebble_accretion_rate(
        0.5 * EARTH_MASS_SOLAR,
        r_earth,
        1.0,
        1.0,
        2.7,
        BodyType::TerrestrialPlanet,
    );
    let rate_super = pebble_accretion_rate(
        1.5 * EARTH_MASS_SOLAR,
        r_earth,
        1.0,
        1.0,
        2.7,
        BodyType::TerrestrialPlanet,
    );
    assert!(
        rate_super < rate_sub * 0.05,
        "Pebble accretion rate for super-Earth must stall: rate_super={:.2e}, rate_sub={:.2e}",
        rate_super,
        rate_sub
    );
}

#[test]
fn test_zero_gas_scale_zero_gas_density() {
    use protostellar::simulation::pebble_accretion::local_pebble_sigma;
    assert_eq!(
        local_pebble_sigma(1.0, 0.0, 2.7),
        0.0,
        "Zero gas density scale must yield zero pebble surface density"
    );
}

#[test]
fn test_theia_giant_impact_classification_and_moon_formation() {
    use protostellar::game::ui::types::{is_embryo_body, is_major_body};
    use protostellar::simulation::accretion::impact_regimes::{
        classify_impact, ImpactParams, ImpactRegime,
    };
    use protostellar::simulation::components::{BodyType, Composition, InternalDifferentiation};
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    // 1. Verify Theia is classified as a major world in UI and not a hidden embryo
    assert!(is_major_body(
        "Theia",
        BodyType::Protoplanet,
        false,
        0.12 * EARTH_MASS_SOLAR
    ));
    assert!(!is_embryo_body("Theia", BodyType::Protoplanet));

    // 2. Theia sideswipe collision with Proto-Earth at 5 km/s (u ~ 0.6) and b = 0.35
    let (p_mass, s_mass) = (0.88 * EARTH_MASS_SOLAR, 0.12 * EARTH_MASS_SOLAR);
    let (p_rad, s_rad) = (0.000040, 0.000022);
    let params = ImpactParams {
        primary_mass: p_mass,
        secondary_mass: s_mass,
        primary_radius_au: p_rad,
        secondary_radius_au: s_rad,
        primary_type: BodyType::Protoplanet,
        secondary_type: BodyType::Protoplanet,
        min_dist: (p_rad + s_rad) * 0.70, // Low-angle mantle interpenetration
        b: 0.35,
        v_rel: 1.05,
        v_esc: 1.75,
    };
    assert_eq!(
        classify_impact(params, 0.00010),
        ImpactRegime::GiantImpactMoon
    );

    // 3. Test LLSVP mantle remnant differentiation marking
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(p_mass + s_mass, p_rad, &Composition::rocky());
    diff.has_theia_llsvp = true;
    diff.llsvp_density_contrast = 0.028;
    assert!(diff.has_theia_llsvp);
    assert_eq!(diff.llsvp_density_contrast, 0.028);
}

#[test]
fn test_stellar_engulfment_and_moon_capture_surface_clearance() {
    use protostellar::simulation::accretion::collisions::is_physically_interpenetrating;
    use protostellar::simulation::accretion::impact_regimes::{
        classify_impact, ImpactParams, ImpactRegime,
    };
    use protostellar::simulation::components::BodyType;
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, JUPITER_MASS_SOLAR};

    // 1. Stellar engulfment: Any planet contacting a star merges unconditionally
    let star_impact = ImpactParams {
        primary_mass: 1.0,
        secondary_mass: 1.0 * EARTH_MASS_SOLAR,
        primary_radius_au: 0.025,
        secondary_radius_au: 0.003,
        primary_type: BodyType::YellowDwarf,
        secondary_type: BodyType::TerrestrialPlanet,
        min_dist: 0.020, // inside the 0.025 AU photosphere
        b: 0.85,
        v_rel: 20.0,
        v_esc: 50.0,
    };
    assert_eq!(classify_impact(star_impact, 0.01), ImpactRegime::Merger);

    // 2. Gas giant moon capture requires exterior distance > 1.15 * R_primary
    let gas_giant_inside = ImpactParams {
        primary_mass: 1.0 * JUPITER_MASS_SOLAR,
        secondary_mass: 0.01 * EARTH_MASS_SOLAR,
        primary_radius_au: 0.0085,
        secondary_radius_au: 0.0005,
        primary_type: BodyType::GasGiant,
        secondary_type: BodyType::Asteroid,
        min_dist: 0.0050, // inside the 0.0085 AU surface
        b: 0.50,
        v_rel: 2.0,
        v_esc: 8.0,
    };
    assert_ne!(
        classify_impact(gas_giant_inside, 0.001),
        ImpactRegime::GiantImpactMoon
    );
    assert_eq!(
        classify_impact(gas_giant_inside, 0.001),
        ImpactRegime::EmbeddedMerge
    );

    let gas_giant_outside = ImpactParams {
        min_dist: 0.0150, // safely outside the 0.0085 AU surface
        ..gas_giant_inside
    };
    assert_eq!(
        classify_impact(gas_giant_outside, 0.001),
        ImpactRegime::GiantImpactMoon
    );

    // 3. Decaying satellite inside contact radius is detected as interpenetrating
    assert!(is_physically_interpenetrating(
        0.0080, 0.0080, 0.0004, 0.0001, 0.0090
    ));
}

#[test]
fn test_roche_lobe_overflow_mass_transfer() {
    use protostellar::simulation::accretion::{
        process_roche_lobe_overflow, CataclysmicVariable, RocheLobeOverflow,
    };
    use protostellar::simulation::components::{
        BodyType, CelestialBody, Mass, Radius, SimPosition, SimVelocity,
    };
    use protostellar::simulation::resources::SimulationConfig;

    let mut app = bevy::prelude::App::new();
    let mut time = bevy::prelude::Time::<()>::default();
    time.advance_by(std::time::Duration::from_secs(1));
    app.insert_resource(time);
    let mut config = SimulationConfig::default();
    config.base_dt_yr = 1.0; // 1 year per step for quick test
    app.insert_resource(config);
    app.add_systems(bevy::prelude::Update, process_roche_lobe_overflow);

    // Star 1: Donor Red Giant (1.5 M_sun, very large radius)
    let donor = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(0.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.0, 0.0)),
            Mass(1.5),
            Radius(0.8), // Enormous radius, well past Roche lobe
            CelestialBody {
                body_type: BodyType::RedGiant,
                name: "Donor".to_string(),
            },
        ))
        .id();

    // Star 2: Accretor Main Sequence (1.0 M_sun, small radius, 1 AU away)
    let accretor = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.0, 0.0)),
            Mass(1.0),
            Radius(0.005),
            CelestialBody {
                body_type: BodyType::WhiteDwarf,
                name: "Accretor".to_string(),
            },
        ))
        .id();

    app.update(); // Run one step

    // Assert donor has RocheLobeOverflow component
    let overflow = app
        .world()
        .get::<RocheLobeOverflow>(donor)
        .expect("Donor should be overflowing");
    assert_eq!(overflow.companion, accretor);
    assert!(overflow.mass_transfer_rate > 1e-6);

    // Verify mass transferred (mass should be conserved)
    let m1 = app.world().get::<Mass>(donor).unwrap().0;
    let m2 = app.world().get::<Mass>(accretor).unwrap().0;
    assert!(m1 < 1.5, "Donor should have lost mass");
    assert!(m2 > 1.0, "Accretor should have gained mass");
    assert!((m1 + m2 - 2.5).abs() < 1e-9, "Total mass must be conserved");

    // Verify CataclysmicVariable added to White Dwarf
    let cv = app
        .world()
        .get::<CataclysmicVariable>(accretor)
        .expect("White Dwarf should gain CV state");
    assert!(cv.accreted_hydrogen_mass > 0.0);
}

#[test]
fn test_classical_nova_ignition_threshold() {
    use protostellar::simulation::accretion::{process_classical_novae, CataclysmicVariable};
    use protostellar::simulation::components::SupernovaEvent;
    use protostellar::simulation::components::{BodyType, CelestialBody, Mass, SimPosition};

    let mut app = bevy::prelude::App::new();
    let mut time = bevy::prelude::Time::<()>::default();
    time.advance_by(std::time::Duration::from_secs(1));
    app.insert_resource(time);
    app.add_message::<SupernovaEvent>();
    app.add_systems(bevy::prelude::Update, process_classical_novae);

    let wd = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(0.0, 0.0, 0.0)),
            Mass(1.0),
            CelestialBody {
                body_type: BodyType::WhiteDwarf,
                name: "WD".to_string(),
            },
            CataclysmicVariable {
                accreted_hydrogen_mass: 1.5e-4, // Above critical mass of 1.0e-4
                nova_count: 0,
            },
        ))
        .id();

    app.update();

    // Verify Nova triggered, mass was blown off, and hydrogen reset
    let cv = app.world().get::<CataclysmicVariable>(wd).unwrap();
    assert_eq!(cv.nova_count, 1);
    assert_eq!(cv.accreted_hydrogen_mass, 0.0);

    let mass = app.world().get::<Mass>(wd).unwrap().0;
    assert!(
        (mass - (1.0 - 1.0e-4)).abs() < 1e-9,
        "WD should lose the accreted hydrogen shell mass"
    );

    // Check events
    let messages = app
        .world()
        .resource::<bevy::prelude::Messages<SupernovaEvent>>();
    let nova_events: Vec<_> = messages.iter_current_update_messages().collect();
    assert_eq!(nova_events.len(), 1);
    assert_eq!(nova_events[0].initial_mass_solar, 1e-4);
}
