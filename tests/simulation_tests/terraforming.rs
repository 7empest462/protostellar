//! Integration tests for Feature 1.4: Targeted Terraforming & Bombardment Suite.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::terraforming::*;
use protostellar::utils::constants::*;

#[test]
fn test_icy_comet_delivery_and_ocean_condensation() {
    let mut comp = Composition::rocky();
    let mut vol = VolatileInventory::default();
    let mut terra = TerraformingAtmosphere::default();
    let mut climate = PlanetaryClimate {
        surface_temperature_k: 288.0,
        equilibrium_temperature_k: 255.0,
        greenhouse_delta_k: 33.0,
        albedo: 0.30,
        ice_coverage_frac: 0.05,
        cloud_coverage_frac: 0.50,
        climate_regime: ClimateRegime::TemperateHabitable,
    };

    // Calculate delivery for an icy comet impact
    let comet_mass_solar = 0.0004 * EARTH_MASS_SOLAR;
    let comet_comp = Composition::icy();
    let delivery = calculate_impact_delivery(
        BombardmentType::IcyComet,
        comet_mass_solar,
        &comet_comp,
        0.005, // v_rel
        1.0 * EARTH_MASS_SOLAR,
        1.0 * EARTH_RADIUS_AU,
        1.0,
        288.0,
    );

    assert!(
        delivery.liquid_water_delivered_m_earth + delivery.steam_vapor_delivered_m_earth > 0.0001,
        "Icy comet must deliver substantial water volatiles"
    );
    assert!(
        delivery.crater_angular_radius > 0.04,
        "Impact must produce an impact crater basin"
    );

    // Apply delivery
    terra.surface_liquid_water_m_earth += delivery.liquid_water_delivered_m_earth;
    terra.atmospheric_water_m_earth += delivery.steam_vapor_delivered_m_earth;
    terra.co2_pressure_bar += delivery.delta_co2_bar;
    terra.nitrogen_pressure_bar += delivery.delta_nitrogen_bar;

    // Step climate: temperature is 288 K (well below boiling point ~373 K), so steam condenses
    update_body_terraforming_climate(&mut vol, &mut terra, &mut climate, &mut comp, 1.0);

    assert!(
        vol.ocean_coverage_frac > 0.10,
        "Delivered water must condense into liquid oceans"
    );
    assert!(
        vol.atmospheric_pressure_bar > 0.5,
        "Atmospheric pressure must be supported"
    );
    assert_eq!(
        climate.climate_regime,
        ClimateRegime::TemperateHabitable,
        "Planet with liquid water at 288 K must be TemperateHabitable"
    );
}

#[test]
fn test_chondrite_atmospheric_pressure_and_greenhouse() {
    let mut comp = Composition::rocky();
    let mut vol = VolatileInventory::default();
    let mut terra = TerraformingAtmosphere {
        co2_pressure_bar: 0.0001,
        nitrogen_pressure_bar: 0.01,
        atmospheric_water_m_earth: 0.0,
        surface_liquid_water_m_earth: 0.0,
        impact_dust_optical_depth: 0.0,
        total_bombarded_mass_earth: 0.0,
    };
    let mut climate = PlanetaryClimate {
        surface_temperature_k: 220.0,
        equilibrium_temperature_k: 220.0,
        greenhouse_delta_k: 0.0,
        albedo: 0.35,
        ice_coverage_frac: 0.0,
        cloud_coverage_frac: 0.0,
        climate_regime: ClimateRegime::AirlessVacuum,
    };

    let chondrite_mass_solar = 0.0005 * EARTH_MASS_SOLAR;
    let mut chondrite_comp = Composition::carbonaceous();
    chondrite_comp.organics_frac = 0.35;

    let delivery = calculate_impact_delivery(
        BombardmentType::CarbonaceousChondrite,
        chondrite_mass_solar,
        &chondrite_comp,
        0.008,
        0.5 * EARTH_MASS_SOLAR,
        0.7 * EARTH_RADIUS_AU,
        0.01,
        220.0,
    );

    assert!(
        delivery.delta_co2_bar > 0.05,
        "Chondrite impact must release CO2 gas"
    );
    assert!(
        delivery.delta_nitrogen_bar > 0.01,
        "Chondrite impact must release N2 buffer gas"
    );

    terra.co2_pressure_bar += delivery.delta_co2_bar;
    terra.nitrogen_pressure_bar += delivery.delta_nitrogen_bar;

    update_body_terraforming_climate(&mut vol, &mut terra, &mut climate, &mut comp, 1.0);

    assert!(
        vol.atmospheric_pressure_bar > 0.1,
        "Atmospheric pressure must grow significantly after chondrite impact"
    );
    assert!(
        climate.greenhouse_delta_k > 5.0,
        "Atmospheric greenhouse warming must increase"
    );
}

fn assert_climate_transition(
    comp: Composition,
    terra: TerraformingAtmosphere,
    climate: PlanetaryClimate,
    dt_yr: f64,
    expected_regime: ClimateRegime,
    min_temp_k: f32,
) -> (TerraformingAtmosphere, PlanetaryClimate) {
    let mut vol = VolatileInventory::default();
    let mut terra = terra;
    let mut climate = climate;
    let mut comp = comp;
    update_body_terraforming_climate(&mut vol, &mut terra, &mut climate, &mut comp, dt_yr);
    assert_eq!(climate.climate_regime, expected_regime);
    assert!(climate.surface_temperature_k >= min_temp_k);
    (terra, climate)
}

#[test]
fn test_runaway_greenhouse_ocean_evaporation() {
    let (terra_out, climate_out) = assert_climate_transition(
        Composition::icy(),
        TerraformingAtmosphere {
            co2_pressure_bar: 0.5,
            nitrogen_pressure_bar: 1.0,
            atmospheric_water_m_earth: 0.0001,
            surface_liquid_water_m_earth: 0.002, // 2 oceans of liquid water
            impact_dust_optical_depth: 0.0,
            total_bombarded_mass_earth: 0.0,
        },
        PlanetaryClimate {
            surface_temperature_k: 390.0,
            equilibrium_temperature_k: 340.0,
            greenhouse_delta_k: 50.0,
            albedo: 0.30,
            ice_coverage_frac: 0.0,
            cloud_coverage_frac: 0.8,
            climate_regime: ClimateRegime::TemperateHabitable,
        },
        2.0,
        ClimateRegime::RunawayVenusian,
        360.0,
    );

    assert!(
        terra_out.atmospheric_water_m_earth > 0.001,
        "High temperature must cause liquid oceans to boil into atmospheric steam vapor"
    );
    assert!(
        climate_out.surface_temperature_k > 360.0,
        "Runaway steam envelope must sustain high greenhouse temperatures"
    );
}

#[test]
fn test_glaciation_breakout_via_co2_bombardment() {
    let mut comp = Composition::rocky();
    let mut vol = VolatileInventory::default();
    let mut terra = TerraformingAtmosphere {
        co2_pressure_bar: 0.0001,
        nitrogen_pressure_bar: 0.80,
        atmospheric_water_m_earth: 0.00001,
        surface_liquid_water_m_earth: 0.0005,
        impact_dust_optical_depth: 0.0,
        total_bombarded_mass_earth: 0.0,
    };
    // Deep frozen Snowball Earth
    let mut climate = PlanetaryClimate {
        surface_temperature_k: 235.0,
        equilibrium_temperature_k: 230.0,
        greenhouse_delta_k: 5.0,
        albedo: 0.65,
        ice_coverage_frac: 1.0,
        cloud_coverage_frac: 0.3,
        climate_regime: ClimateRegime::SnowballIceAge,
    };

    // Heavy targeted CO2 bombardment (injecting 1.8 bars of CO2)
    terra.co2_pressure_bar += 1.8;

    update_body_terraforming_climate(&mut vol, &mut terra, &mut climate, &mut comp, 5.0);

    assert!(
        climate.surface_temperature_k > 265.0,
        "Heavy CO2 greenhouse forcing must warm frozen snowball world"
    );
    assert_ne!(
        climate.climate_regime,
        ClimateRegime::SnowballIceAge,
        "Sufficient greenhouse warming must break planet out of SnowballIceAge"
    );
}

#[test]
fn test_core_impactor_dynamo_stimulation() {
    let comp = Composition::metal_rich();
    let delivery = calculate_impact_delivery(
        BombardmentType::IronAsteroid,
        0.0008 * EARTH_MASS_SOLAR,
        &comp,
        0.010,
        1.0 * EARTH_MASS_SOLAR,
        1.0 * EARTH_RADIUS_AU,
        1.0,
        280.0,
    );

    assert!(
        delivery.core_temp_boost_k >= 100.0,
        "Iron core impactor must impart significant kinetic shock heating to the interior"
    );
    assert!(
        delivery.crater_angular_radius > 0.07,
        "Massive iron impactor must form large crater basin"
    );
}

#[test]
fn test_guided_bombardment_spawner_and_impact_resolution() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<BombardmentEvent>();
    app.add_systems(
        Update,
        (
            update_guided_bombardment_projectiles,
            update_terraforming_atmospheres,
        ),
    );

    // Spawn target world (Earth analogue)
    let target_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Proto-Mars".to_string(),
            },
            SimPosition(DVec3::new(1.5, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 5.0)),
            SimAcceleration::default(),
            Mass(0.15 * EARTH_MASS_SOLAR),
            Radius(0.55 * EARTH_RADIUS_AU),
            Temperature(215.0),
            Composition::rocky(),
            VolatileInventory::default(),
            PlanetaryClimate::default(),
            InternalDifferentiation::default(),
        ))
        .id();

    // Launch targeted bombardment comet
    let proj_ent = launch_targeted_bombardment(
        &mut app.world_mut().commands(),
        target_ent,
        DVec3::new(1.5, 0.0, 0.0),
        DVec3::new(0.0, 0.0, 5.0),
        0.15 * EARTH_MASS_SOLAR,
        0.55 * EARTH_RADIUS_AU,
        "Proto-Mars",
        BombardmentType::IcyComet,
        0.0,
    );

    app.update();

    // Verify projectile entity exists and has component
    let proj_comp = app
        .world()
        .get::<BombardmentProjectile>(proj_ent)
        .expect("Projectile entity must have BombardmentProjectile component");
    assert_eq!(proj_comp.target_entity, target_ent);
    assert_eq!(proj_comp.bombardment_type, BombardmentType::IcyComet);

    // Move projectile right onto target to trigger impact proximity
    if let Some(mut pos) = app.world_mut().get_mut::<SimPosition>(proj_ent) {
        pos.0 = DVec3::new(1.5, 0.0, 0.0);
    }

    app.update();

    // Projectile should now be despawned post-impact
    assert!(
        app.world().get_entity(proj_ent).is_err(),
        "Bombardment projectile must despawn upon impacting target"
    );

    // Target should have received volatiles and impact crater
    let vol = app
        .world()
        .get::<VolatileInventory>(target_ent)
        .expect("Target must have VolatileInventory");
    assert!(
        vol.delivered_water_m_earth > 0.00005,
        "Comet impact must deliver water volatiles to target"
    );
    assert!(
        vol.cometary_impact_count >= 1,
        "Cometary impact count must increment"
    );

    let basins = app
        .world()
        .get::<PlanetaryBasins>(target_ent)
        .expect("Target must have PlanetaryBasins registered from cratering");
    assert!(
        !basins.basins.is_empty(),
        "Target must have at least one impact crater basin"
    );
}

#[test]
fn test_guided_bombardment_projectiles_retain_minor_body_identity_and_mesh() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<DiskParameters>();
    app.add_message::<BombardmentEvent>();

    // Target planet
    let target_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Temperature(288.0),
            Composition::rocky(),
            VolatileInventory::default(),
        ))
        .id();

    // 1. Launch Icy Comet
    let comet_ent = launch_targeted_bombardment(
        &mut app.world_mut().commands(),
        target_ent,
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::ZERO,
        EARTH_MASS_SOLAR,
        EARTH_RADIUS_AU,
        "Earth",
        BombardmentType::IcyComet,
        0.0,
    );

    // 2. Launch Carbonaceous Chondrite (water-bearing asteroid)
    let chondrite_ent = launch_targeted_bombardment(
        &mut app.world_mut().commands(),
        target_ent,
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::ZERO,
        EARTH_MASS_SOLAR,
        EARTH_RADIUS_AU,
        "Earth",
        BombardmentType::CarbonaceousChondrite,
        0.0,
    );

    app.add_systems(
        Update,
        (
            protostellar::simulation::pebble_accretion::apply_pebble_accretion,
            protostellar::simulation::accretion::gas::direct_nebular_gas_accretion,
        ),
    );

    // Step simulation under active gas/pebble disk
    app.update();

    // Verify Icy Comet remains strictly BodyType::Comet (not promoted to Planetesimal/sphere)
    let comet_body = app
        .world()
        .get::<CelestialBody>(comet_ent)
        .expect("Comet entity must exist");
    assert_eq!(
        comet_body.body_type,
        BodyType::Comet,
        "Icy Comet must maintain BodyType::Comet during flight"
    );

    // Verify Carbonaceous Chondrite remains strictly BodyType::Asteroid
    let chondrite_body = app
        .world()
        .get::<CelestialBody>(chondrite_ent)
        .expect("Chondrite entity must exist");
    assert_eq!(
        chondrite_body.body_type,
        BodyType::Asteroid,
        "Carbonaceous Chondrite must maintain BodyType::Asteroid during flight"
    );
}

#[test]
fn test_guided_bombardment_orbiting_target_zero_miss() {
    let mut app = App::new();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_message::<BombardmentEvent>();

    // Spawn central star
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration::default(),
        Radius(SOLAR_RADIUS_AU),
    ));

    // Spawn orbiting planet (Earth at 1.0 AU in Keplerian orbit)
    let v_circ = (G_ASTRO * 1.0 / 1.0).sqrt(); // 2*PI AU/yr
    let target_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Target-Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_circ)),
            SimAcceleration::default(),
            Temperature(288.0),
            Composition::rocky(),
            VolatileInventory::default(),
            TerraformingAtmosphere::default(),
            PlanetaryClimate::default(),
            InternalDifferentiation::default(),
        ))
        .id();

    // Launch targeted bombardment at orbiting Earth
    let proj_ent = launch_targeted_bombardment(
        &mut app.world_mut().commands(),
        target_ent,
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(0.0, 0.0, v_circ),
        EARTH_MASS_SOLAR,
        EARTH_RADIUS_AU,
        "Target-Earth",
        BombardmentType::IcyComet,
        0.0,
    );

    app.add_systems(
        Update,
        (
            protostellar::simulation::physics::step_physics_simulation,
            update_guided_bombardment_projectiles,
        )
            .chain(),
    );

    // Step simulation across the ~0.0035 year flight duration
    let dt = 0.0005f64; // ~4.3 hours per step
    for _ in 0..15 {
        {
            let mut sim_time = app.world_mut().resource_mut::<SimTime>();
            sim_time.elapsed_years += dt;
            sim_time.current_dt_yr = dt;
        }
        app.update();

        // When the projectile impacts the target, it delivers its payload and despawns
        if app.world().get::<BombardmentProjectile>(proj_ent).is_none() {
            break;
        }
    }

    // Verify guaranteed zero-miss impact occurred and projectile was consumed
    assert!(
        app.world().get::<BombardmentProjectile>(proj_ent).is_none(),
        "Guided bombardment projectile must successfully impact orbiting planet and be consumed"
    );

    let vol = app
        .world()
        .get::<VolatileInventory>(target_ent)
        .expect("Target VolatileInventory must exist");
    assert!(
        vol.delivered_water_m_earth > 0.0,
        "Icy comet must deliver water to target upon zero-miss impact"
    );
    assert_eq!(
        vol.cometary_impact_count, 1,
        "Cometary impact count must increment by 1"
    );
}
