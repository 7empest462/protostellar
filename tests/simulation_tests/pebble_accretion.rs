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
        "Pebble surface density at the snow line ({sigma_at_snow:.4}) must be enhanced over distant regions ({sigma_outside:.4})"
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
    let config = SimulationConfig {
        enable_accretion: true,
        gas_density_scale: 1.0,
        base_dt_yr: 0.5,
        ..Default::default()
    };
    app.insert_resource(config);

    let time_warp = TimeWarp {
        multiplier: 1.0,
        ..Default::default()
    };
    app.insert_resource(time_warp);

    let sim_time = SimTime {
        elapsed_years: 100.0,
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let disk_params = DiskParameters {
        central_star_mass: 1.0,
        gas_disk_lifetime_yr: 5_000_000.0,
        snow_line_au: 2.7,
        ..Default::default()
    };
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
    let config = SimulationConfig {
        gas_density_scale: 1.0,
        ..Default::default()
    };
    app.insert_resource(config);

    app.insert_resource(TimeWarp::default());

    let sim_time = SimTime {
        elapsed_years: 500.0,
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let disk_params = DiskParameters {
        central_star_mass: 1.0,
        gas_disk_lifetime_yr: 4_000_000.0,
        snow_line_au: 2.7,
        ..Default::default()
    };
    app.insert_resource(disk_params);

    let spawner = PlanetesimalSpawner {
        last_spawn_yr: 0.0,
        max_ecs_bodies: 50,
        ..Default::default()
    };
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
        "Silicate pebble barrier must suppress pebble density by >= 70%: got {barrier_inside:.3}"
    );

    // Beyond snow line: sticky icy pebbles maintain full cohesion
    let barrier_outside = silicate_fragmentation_barrier(3.5, snow_line);
    assert!(
        (barrier_outside - 1.0).abs() < 1e-6,
        "Silicate pebble barrier outside snow line must be 1.0: got {barrier_outside:.3}"
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
        "Pebble accretion rate for super-Earth must stall: rate_super={rate_super:.2e}, rate_sub={rate_sub:.2e}"
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
        ImpactParams, ImpactRegime, classify_impact,
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
    let (p_rad, s_rad) = (0.000_040, 0.000_022);
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
        ImpactParams, ImpactRegime, classify_impact,
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
        CataclysmicVariable, RocheLobeOverflow, process_roche_lobe_overflow,
    };
    use protostellar::simulation::components::{
        BodyType, CelestialBody, Mass, Radius, SimPosition, SimVelocity,
    };
    use protostellar::simulation::resources::SimulationConfig;

    let mut app = bevy::prelude::App::new();
    let mut time = bevy::prelude::Time::<()>::default();
    time.advance_by(std::time::Duration::from_secs(1));
    app.insert_resource(time);
    let config = SimulationConfig {
        base_dt_yr: 1.0, // 1 year per step for quick test
        ..Default::default()
    };
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
    use protostellar::simulation::accretion::{CataclysmicVariable, process_classical_novae};
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
        (mass - (1.0 - 1.5e-4)).abs() < 1e-9,
        "WD should lose the full accreted hydrogen shell mass"
    );

    // Check events
    let messages = app
        .world()
        .resource::<bevy::prelude::Messages<SupernovaEvent>>();
    let nova_events: Vec<_> = messages.iter_current_update_messages().collect();
    assert_eq!(nova_events.len(), 1);
    assert_eq!(nova_events[0].initial_mass_solar, 1.5e-4);
}

#[test]
fn test_s4714_sgr_a_roche_stream_multi_star_cluster() {
    use bevy::prelude::*;
    use protostellar::rendering::effects::roche_stream::{
        RocheStreamMaterial, RocheStreamVisual, update_roche_streams,
    };
    use protostellar::simulation::accretion::{RocheLobeOverflow, process_roche_lobe_overflow};
    use protostellar::simulation::components::{
        BodyType, CelestialBody, Mass, Radius, SimPosition, SimVelocity, Temperature,
    };
    use protostellar::simulation::resources::SimulationConfig;

    let mut app = App::new();
    let mut time = Time::<()>::default();
    time.advance_by(std::time::Duration::from_millis(16));
    app.insert_resource(time);
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<RocheStreamMaterial>>();
    let config = SimulationConfig {
        base_dt_yr: 0.01,
        ..Default::default()
    };
    app.insert_resource(config);
    app.add_systems(
        Update,
        (process_roche_lobe_overflow, update_roche_streams).chain(),
    );

    // 1. Sagittarius A* supermassive black hole (4.297e6 M_sun)
    let sgr_a = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::ZERO),
            SimVelocity(bevy::math::DVec3::ZERO),
            Mass(4.297e6),
            Radius(0.0848),
            Temperature(1.0e-7),
            CelestialBody {
                body_type: BodyType::BlackHole,
                name: "Sagittarius A*".to_string(),
            },
        ))
        .id();

    // 2. S4714 at 0.51 AU (matching user screenshot)
    let s4714 = app
        .world_mut()
        .spawn((
            SimPosition(bevy::math::DVec3::new(0.51, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.0, 18000.0)),
            Mass(2.0),
            Radius(0.0088),
            Temperature(9500.0),
            CelestialBody {
                body_type: BodyType::MainSequenceStar,
                name: "S4714 (Fastest Star)".to_string(),
            },
        ))
        .id();

    // 3. Another nearby S-star within pair-check range (must not strip S4714's RocheLobeOverflow!)
    app.world_mut().spawn((
        SimPosition(bevy::math::DVec3::new(3.2, 1.0, 0.0)),
        SimVelocity(bevy::math::DVec3::new(0.0, 5000.0, 0.0)),
        Mass(2.2),
        Radius(0.0095),
        Temperature(10500.0),
        CelestialBody {
            body_type: BodyType::MainSequenceStar,
            name: "S4711".to_string(),
        },
    ));

    // First update inserts RocheLobeOverflow via Commands; second update spawns RocheStreamVisual
    app.update();
    app.update();

    let overflow = app
        .world()
        .get::<RocheLobeOverflow>(s4714)
        .expect("S4714 at 0.51 AU from Sgr A* must have RocheLobeOverflow");
    assert_eq!(overflow.companion, sgr_a);

    let mut visual_query = app.world_mut().query::<(&RocheStreamVisual, &Transform)>();
    let visuals: Vec<_> = visual_query.iter(app.world()).collect();
    assert_eq!(
        visuals.len(),
        1,
        "Exactly one RocheStreamVisual should be spawned for S4714 -> Sgr A*"
    );
    assert_eq!(visuals[0].0.donor, s4714);
    assert_eq!(visuals[0].0.accretor, sgr_a);
    assert!(visuals[0].1.translation.is_finite());
    assert!(visuals[0].1.scale.is_finite());
}

#[test]
fn test_earth_accumulates_water_and_oceans_via_pebble_accretion() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::pebble_accretion::apply_pebble_accretion;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::{StarIgnitionEvent, update_thermodynamics};
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU, SOLAR_RADIUS_AU};

    let mut app = App::new();
    let config = SimulationConfig {
        enable_accretion: true,
        gas_density_scale: 1.0,
        base_dt_yr: 0.5,
        ..Default::default()
    };
    app.insert_resource(config);

    let time_warp = TimeWarp {
        multiplier: 1.0,
        ..Default::default()
    };
    app.insert_resource(time_warp);

    let sim_time = SimTime {
        elapsed_years: 100.0,
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let disk_params = DiskParameters {
        central_star_mass: 1.0,
        gas_disk_lifetime_yr: 4_000_000.0,
        snow_line_au: 2.7,
        ..Default::default()
    };
    app.insert_resource(disk_params);

    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sol".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        SimPosition(DVec3::ZERO),
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Luminosity(1.0),
        IgnitionState {
            is_ignited: true,
            fusion_fraction: 1.0,
            shockwave_radius: 50.0,
            ..default()
        },
    ));

    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            Temperature(288.0),
            Composition::rocky(),
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 0,
            },
        ))
        .id();

    // Growing proto-Earth without an initial VolatileInventory component must also gain one
    let proto_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Proto-Earth".to_string(),
                body_type: BodyType::Protoplanet,
            },
            Mass(0.40 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.74),
            SimPosition(DVec3::new(1.05, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 6.13)),
            Temperature(288.0),
            Composition::rocky(),
        ))
        .id();

    app.add_message::<StarIgnitionEvent>()
        .add_message::<PlanetaryEngulfmentEvent>()
        .add_message::<SupernovaEvent>()
        .add_systems(
            Update,
            (apply_pebble_accretion, update_thermodynamics).chain(),
        );

    // Run 60 simulated years (120 steps * 0.5 yr)
    for _ in 0..120 {
        app.update();
    }

    let world = app.world();
    let earth_vol = world
        .get::<VolatileInventory>(earth_ent)
        .expect("Earth VolatileInventory must exist");
    let earth_body = world.get::<CelestialBody>(earth_ent).unwrap();
    assert!(
        earth_vol.delivered_water_m_earth > 0.0002,
        "Earth at 1.0 AU must accumulate substantial H2O via hydrated pebble accretion, got {:.6e}",
        earth_vol.delivered_water_m_earth
    );
    assert!(
        earth_vol.ocean_coverage_frac > 0.25,
        "Earth accumulated pebble water must condense into surface oceans (>25% coverage), got {:.3}",
        earth_vol.ocean_coverage_frac
    );
    assert_eq!(
        earth_body.body_type,
        BodyType::TerrestrialPlanet,
        "Wet pebble accretion must keep Earth classified as a TerrestrialPlanet"
    );

    let proto_vol = world.get::<VolatileInventory>(proto_ent).expect(
        "Proto-Earth without initial VolatileInventory must receive one via pebble accretion",
    );
    assert!(
        proto_vol.delivered_water_m_earth > 0.0002,
        "Proto-Earth must accumulate pebble water: got {:.6e}",
        proto_vol.delivered_water_m_earth
    );
}

#[test]
fn test_physical_comet_collision_preserves_pebble_accretion_oceans() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::accretion::basins::deliver_volatiles_and_crater;
    use protostellar::simulation::components::*;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let mut world = World::new();
    let earth_ent = world
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Temperature(305.0),
            VolatileInventory {
                delivered_water_m_earth: 0.000_425,
                ocean_coverage_frac: 0.85,
                atmospheric_pressure_bar: 1.06,
                cometary_impact_count: 0,
            },
        ))
        .id();

    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, &world);
        deliver_volatiles_and_crater(
            &mut commands,
            earth_ent,
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0001, 0.0, 0.0),
            EARTH_MASS_SOLAR,
            0.0001 * EARTH_MASS_SOLAR,
            &Composition::icy(),
            100.0,
        );
    }
    queue.apply(&mut world);

    let vol = world
        .get::<VolatileInventory>(earth_ent)
        .expect("Earth must retain VolatileInventory after physical comet merger");
    assert!(
        vol.delivered_water_m_earth > 0.000_425,
        "Physical icy comet merger must increase delivered water: got {:.6}",
        vol.delivered_water_m_earth
    );
    assert!(
        vol.ocean_coverage_frac >= 0.85,
        "Physical icy comet merger must NOT reduce 85% ocean coverage: got {:.4}",
        vol.ocean_coverage_frac
    );
}

#[test]
fn test_roche_cleanup_and_minor_body_3d_inclined_velocities() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::accretion::{RocheLobeOverflow, cleanup_roche_lobe_overflow};
    use protostellar::simulation::components::{BodyType, CelestialBody, SimPosition, SimVelocity};
    use protostellar::simulation::disk::PlanetesimalSpawner;
    use protostellar::simulation::pebble_accretion::spawn_streaming_instability_minor_bodies;
    use protostellar::simulation::resources::{DiskParameters, SimTime, TimeWarp};

    let mut app = App::new();
    app.add_systems(Update, cleanup_roche_lobe_overflow);
    let comp = app.world_mut().spawn(SimPosition(DVec3::ZERO)).id();
    let far_star = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(10.0, 0.0, 0.0)), // > 8 AU range
            RocheLobeOverflow {
                companion: comp,
                mass_transfer_rate: 1e-4,
                l1_distance_au: 1.0,
            },
        ))
        .id();
    app.update();
    assert!(
        app.world().get::<RocheLobeOverflow>(far_star).is_none(),
        "cleanup_roche_lobe_overflow must detach RocheLobeOverflow beyond 8 AU"
    );

    let mut spawn_app = App::new();
    spawn_app
        .init_resource::<DiskParameters>()
        .init_resource::<TimeWarp>()
        .init_resource::<protostellar::simulation::resources::SimulationConfig>()
        .insert_resource(SimTime {
            elapsed_years: 200.0,
            ..Default::default()
        })
        .insert_resource(PlanetesimalSpawner {
            max_ecs_bodies: 200,
            ..Default::default()
        })
        .add_systems(Update, spawn_streaming_instability_minor_bodies);

    spawn_app.update();

    let mut q = spawn_app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &SimVelocity)>();
    let mut non_zero_vy = 0;
    let mut total_spawned = 0;
    for (body, pos, vel) in q.iter(spawn_app.world()) {
        if matches!(
            body.body_type,
            BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal
        ) {
            total_spawned += 1;
            if vel.0.y.abs() > 1e-6 {
                non_zero_vy += 1;
            }
            assert!(pos.0.length().is_finite());
        }
    }
    assert!(total_spawned > 0);
    assert!(
        non_zero_vy > 0,
        "Minor bodies spawned in 3D inclined disk must have non-zero vel.y components"
    );
}
