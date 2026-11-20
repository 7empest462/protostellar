//! Planetary ocean stability, epoch transitions, and multi-world isolation tests
//! for Deep-Time Geological Epochs.

use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::geology::systems::*;
use protostellar::simulation::geology::types::*;
use protostellar::simulation::resources::*;

#[test]
fn test_future_epoch_ocean_stability_no_oscillations() {
    use protostellar::simulation::thermodynamics::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimelineScrubber>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_systems(
        Update,
        (
            update_thermodynamics,
            sync_geological_evolution_system.after(update_thermodynamics),
        ),
    );

    // Spawn central star (Sun)
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(protostellar::utils::constants::SOLAR_RADIUS_AU),
        SimPosition(bevy::math::DVec3::ZERO),
        SimVelocity(bevy::math::DVec3::ZERO),
        Temperature(5778.0),
        Luminosity(1.0),
        IgnitionState {
            is_ignited: true,
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            shockwave_radius: 30.0,
        },
    ));

    // Spawn terrestrial planet (Earth) at 1 AU
    let earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(protostellar::utils::constants::EARTH_MASS_SOLAR),
            Radius(1.0),
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 1.0, 0.0)),
            Temperature(288.0),
            Composition::rocky(),
            VolatileInventory {
                delivered_water_m_earth: 0.0006,
                ocean_coverage_frac: 0.70,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 0,
            },
            PlanetaryClimate::default(),
        ))
        .id();

    app.update();

    // Scrub to Future Epoch
    {
        let mut scrubber = app.world_mut().resource_mut::<TimelineScrubber>();
        scrubber.auto_advance = false;
        scrubber.active_epoch = GeologicalEpoch::Future;
        scrubber.scrubbed_age_gyr = GeologicalEpoch::Future.canonical_age_gyr();
    }
    app.update();

    let initial_future_ocean = app
        .world()
        .get::<VolatileInventory>(earth)
        .unwrap()
        .ocean_coverage_frac;
    assert!(
        (initial_future_ocean - 0.45).abs() < 0.06,
        "Future ocean should initialize to ~0.45, got {initial_future_ocean}"
    );

    // Simulate 60 frames of active physics, thermodynamics, and geology
    for frame in 0..60 {
        app.update();
        let ocean = app
            .world()
            .get::<VolatileInventory>(earth)
            .unwrap()
            .ocean_coverage_frac;
        assert!(
            (0.35..=0.55).contains(&ocean),
            "Future ocean must remain stable and not oscillate or sink to zero! Frame {frame} got {ocean}"
        );
    }
}

#[test]
fn test_venus_temperate_ocean_stability_no_oscillations() {
    use protostellar::simulation::thermodynamics::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimelineScrubber>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_systems(
        Update,
        (
            update_thermodynamics,
            sync_geological_evolution_system.after(update_thermodynamics),
        ),
    );

    // Spawn central star (Sun)
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(protostellar::utils::constants::SOLAR_RADIUS_AU),
        SimPosition(bevy::math::DVec3::ZERO),
        SimVelocity(bevy::math::DVec3::ZERO),
        Temperature(5778.0),
        Luminosity(1.0),
        IgnitionState {
            is_ignited: true,
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            shockwave_radius: 30.0,
        },
    ));

    // Spawn Venus at 0.723 AU
    let venus = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Venus".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(protostellar::utils::constants::EARTH_MASS_SOLAR * 0.815),
            Radius(0.95),
            SimPosition(bevy::math::DVec3::new(0.723, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 1.17, 0.0)),
            Temperature(735.0),
            Composition::rocky(),
            GeologicalState::default_for_planet(EpochTargetPlanet::Venus),
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 92.0,
                cometary_impact_count: 0,
            },
            PlanetaryClimate {
                surface_temperature_k: 735.0,
                equilibrium_temperature_k: 260.0,
                greenhouse_delta_k: 475.0,
                albedo: 0.75,
                ice_coverage_frac: 0.0,
                cloud_coverage_frac: 1.0,
                climate_regime: ClimateRegime::RunawayVenusian,
                polar_ice_cap_latitude_deg: 90.0,
            },
        ))
        .id();

    app.update();

    // Target Venus and scrub to Temperate Ocean Era (2.5 Ga)
    {
        let mut scrubber = app.world_mut().resource_mut::<TimelineScrubber>();
        scrubber.target_planet = EpochTargetPlanet::Venus;
        scrubber.target_entity = Some(venus);
        scrubber.auto_advance = false;
        scrubber.active_epoch = GeologicalEpoch::VenusTemperate;
        scrubber.scrubbed_age_gyr = GeologicalEpoch::VenusTemperate.canonical_age_gyr();
    }
    app.update();

    let initial_ocean = app
        .world()
        .get::<VolatileInventory>(venus)
        .unwrap()
        .ocean_coverage_frac;
    let initial_temp = app.world().get::<Temperature>(venus).unwrap().0;
    let initial_climate = app.world().get::<PlanetaryClimate>(venus).unwrap();
    assert_eq!(initial_ocean, 0.40);
    assert_eq!(initial_temp, 295.0);
    assert_eq!(
        initial_climate.climate_regime,
        ClimateRegime::TemperateHabitable
    );

    // Simulate 60 frames of active physics, thermodynamics, and geology.
    // In unpatched code, update_thermodynamics evaluated modern Venus insolation at 0.723 AU,
    // triggered a 3.5x runaway greenhouse, boiled the ocean away to 0.0, and caused 60Hz flickering.
    for frame in 0..60 {
        app.update();
        let ocean = app
            .world()
            .get::<VolatileInventory>(venus)
            .unwrap()
            .ocean_coverage_frac;
        let temp = app.world().get::<Temperature>(venus).unwrap().0;
        let climate = app.world().get::<PlanetaryClimate>(venus).unwrap();

        assert!(
            (ocean - 0.40).abs() < 0.02,
            "Venus temperate ocean must remain stable at ~0.40 and not boil off or flicker! Frame {frame} got {ocean}"
        );
        assert!(
            (temp - 295.0).abs() < 5.0,
            "Venus temperate temperature must remain ~295 K! Frame {frame} got {temp} K"
        );
        assert_eq!(
            climate.climate_regime,
            ClimateRegime::TemperateHabitable,
            "Venus climate regime must remain TemperateHabitable! Frame {frame} got {:?}",
            climate.climate_regime
        );
    }
}

#[test]
fn test_snowball_earth_glaciation_parameters() {
    let mut geo = GeologicalState::default();
    let mut vol = VolatileInventory::default();
    let mut climate = PlanetaryClimate::default();
    let mut temp = Temperature(288.0);

    apply_epoch_to_world(
        GeologicalEpoch::SnowballEarth,
        &mut geo,
        Some(&mut vol),
        Some(&mut climate),
        Some(&mut temp),
    );

    assert_eq!(geo.epoch, GeologicalEpoch::SnowballEarth);
    assert_eq!(temp.0, 220.0);
    assert_eq!(climate.surface_temperature_k, 220.0);
    assert_eq!(climate.ice_coverage_frac, 0.95);
    assert_eq!(climate.climate_regime, ClimateRegime::SnowballIceAge);
    assert_eq!(vol.ocean_coverage_frac, 0.05);
    assert_eq!(vol.atmospheric_pressure_bar, 0.50);
}

struct TerrestrialEpochTransitionTestCase {
    planet: EpochTargetPlanet,
    initial_temp: f64,
    initial_epoch: GeologicalEpoch,
    wet_epoch: GeologicalEpoch,
    wet_temp: f64,
    wet_ocean_frac: f32,
    wet_pressure_bar: f32,
    back_epoch: GeologicalEpoch,
    back_temp: f64,
    back_pressure_bar: f32,
    back_regime: ClimateRegime,
    back_assertion: fn(&PlanetaryClimate),
}

fn assert_terrestrial_epoch_cycle(test_case: TerrestrialEpochTransitionTestCase) {
    let mut geo = GeologicalState::default_for_planet(test_case.planet);
    let mut vol = VolatileInventory::default();
    let mut climate = PlanetaryClimate::default();
    let mut temp = Temperature(test_case.initial_temp);

    // 1. Initialized to starting epoch
    assert_eq!(geo.epoch, test_case.initial_epoch);

    // 2. Scrub to habitable/temperate era
    apply_epoch_to_world(
        test_case.wet_epoch,
        &mut geo,
        Some(&mut vol),
        Some(&mut climate),
        Some(&mut temp),
    );
    assert_eq!(geo.epoch, test_case.wet_epoch);
    assert_eq!(temp.0, test_case.wet_temp);
    assert_eq!(climate.surface_temperature_k, test_case.wet_temp as f32);
    assert_eq!(vol.ocean_coverage_frac, test_case.wet_ocean_frac);
    assert_eq!(vol.atmospheric_pressure_bar, test_case.wet_pressure_bar);
    assert_eq!(climate.climate_regime, ClimateRegime::TemperateHabitable);

    // 3. Scrub back to harsh modern era
    apply_epoch_to_world(
        test_case.back_epoch,
        &mut geo,
        Some(&mut vol),
        Some(&mut climate),
        Some(&mut temp),
    );
    assert_eq!(geo.epoch, test_case.back_epoch);
    assert_eq!(temp.0, test_case.back_temp);
    assert_eq!(vol.ocean_coverage_frac, 0.0);
    assert_eq!(vol.atmospheric_pressure_bar, test_case.back_pressure_bar);
    assert_eq!(climate.climate_regime, test_case.back_regime);
    (test_case.back_assertion)(&climate);
}

#[test]
fn test_mars_geological_epoch_transitions() {
    assert_terrestrial_epoch_cycle(TerrestrialEpochTransitionTestCase {
        planet: EpochTargetPlanet::Mars,
        initial_temp: 215.0,
        initial_epoch: GeologicalEpoch::MarsAmazonian,
        wet_epoch: GeologicalEpoch::MarsNoachian,
        wet_temp: 280.0,
        wet_ocean_frac: 0.35,
        wet_pressure_bar: 0.80,
        back_epoch: GeologicalEpoch::MarsAmazonian,
        back_temp: 215.0,
        back_pressure_bar: 0.006,
        back_regime: ClimateRegime::SnowballIceAge,
        back_assertion: |c| assert_eq!(c.ice_coverage_frac, 0.15),
    });
}

#[test]
fn test_venus_geological_epoch_transitions() {
    assert_terrestrial_epoch_cycle(TerrestrialEpochTransitionTestCase {
        planet: EpochTargetPlanet::Venus,
        initial_temp: 735.0,
        initial_epoch: GeologicalEpoch::VenusModern,
        wet_epoch: GeologicalEpoch::VenusTemperate,
        wet_temp: 295.0,
        wet_ocean_frac: 0.40,
        wet_pressure_bar: 1.50,
        back_epoch: GeologicalEpoch::VenusModern,
        back_temp: 735.0,
        back_pressure_bar: 92.0,
        back_regime: ClimateRegime::RunawayVenusian,
        back_assertion: |c| assert_eq!(c.cloud_coverage_frac, 1.00),
    });
}

#[test]
fn test_multi_planet_epoch_isolation() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimTime>();
    app.init_resource::<TimelineScrubber>();
    app.add_systems(Update, sync_geological_evolution_system);

    // Spawn Earth
    let earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            GeologicalState::default_for_planet(EpochTargetPlanet::Earth),
            VolatileInventory {
                delivered_water_m_earth: 0.0006,
                ocean_coverage_frac: 0.70,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 0,
            },
            PlanetaryClimate::default(),
            Temperature(288.0),
        ))
        .id();

    // Spawn Mars
    let mars = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Mars".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            GeologicalState::default_for_planet(EpochTargetPlanet::Mars),
            VolatileInventory {
                delivered_water_m_earth: 0.000_020,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.006,
                cometary_impact_count: 0,
            },
            PlanetaryClimate {
                surface_temperature_k: 215.0,
                equilibrium_temperature_k: 210.0,
                greenhouse_delta_k: 5.0,
                albedo: 0.25,
                ice_coverage_frac: 0.15,
                cloud_coverage_frac: 0.08,
                climate_regime: ClimateRegime::SnowballIceAge,
                polar_ice_cap_latitude_deg: 35.0,
            },
            Temperature(215.0),
        ))
        .id();

    app.update();

    // Target Mars and scrub to Noachian Wet Era
    {
        let mut scrubber = app.world_mut().resource_mut::<TimelineScrubber>();
        scrubber.target_entity = Some(mars);
        scrubber.target_planet = EpochTargetPlanet::Mars;
        scrubber.auto_advance = false;
        scrubber.active_epoch = GeologicalEpoch::MarsNoachian;
        scrubber.scrubbed_age_gyr = GeologicalEpoch::MarsNoachian.canonical_age_gyr();
    }
    app.update();

    // Mars should now be Noachian with oceans
    let mars_geo = app.world().get::<GeologicalState>(mars).unwrap();
    let mars_vol = app.world().get::<VolatileInventory>(mars).unwrap();
    let mars_temp = app.world().get::<Temperature>(mars).unwrap();
    assert_eq!(mars_geo.epoch, GeologicalEpoch::MarsNoachian);
    assert_eq!(mars_vol.ocean_coverage_frac, 0.35);
    assert_eq!(mars_temp.0, 280.0);

    // Earth must remain completely untouched (Modern, 70% ocean, 288 K)
    let earth_geo = app.world().get::<GeologicalState>(earth).unwrap();
    let earth_vol = app.world().get::<VolatileInventory>(earth).unwrap();
    let earth_temp = app.world().get::<Temperature>(earth).unwrap();
    assert_eq!(earth_geo.epoch, GeologicalEpoch::Modern);
    assert_eq!(earth_vol.ocean_coverage_frac, 0.70);
    assert_eq!(earth_temp.0, 288.0);
}

#[test]
fn test_non_solar_planet_isolation_from_epoch_scrubbing() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimTime>();
    app.init_resource::<TimelineScrubber>();
    app.add_systems(Update, sync_geological_evolution_system);

    // Spawn Earth
    let earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            GeologicalState::default_for_planet(EpochTargetPlanet::Earth),
            VolatileInventory::default(),
            PlanetaryClimate::default(),
            Temperature(288.0),
        ))
        .id();

    // Spawn Valkyrie (exoplanet / magnetar cluster world)
    let valkyrie = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Valkyrie (Shattered Iron Core)".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.0,
                cometary_impact_count: 0,
            },
            PlanetaryClimate {
                surface_temperature_k: 1800.0,
                equilibrium_temperature_k: 1800.0,
                greenhouse_delta_k: 0.0,
                albedo: 0.10,
                ice_coverage_frac: 0.0,
                cloud_coverage_frac: 0.0,
                climate_regime: ClimateRegime::AirlessVacuum,
                polar_ice_cap_latitude_deg: 90.0,
            },
            Temperature(1800.0),
        ))
        .id();

    // Step system - Valkyrie must not have GeologicalState injected
    app.update();
    assert!(app.world().get::<GeologicalState>(valkyrie).is_none());

    // Scrub Earth to Modern or Snowball with auto_advance = false
    {
        let mut scrubber = app.world_mut().resource_mut::<TimelineScrubber>();
        scrubber.target_entity = Some(earth);
        scrubber.target_planet = EpochTargetPlanet::Earth;
        scrubber.auto_advance = false;
        scrubber.active_epoch = GeologicalEpoch::SnowballEarth;
        scrubber.scrubbed_age_gyr = GeologicalEpoch::SnowballEarth.canonical_age_gyr();
    }
    app.update();

    // Valkyrie must remain completely unmutated: no GeologicalState, 1800 K, 0 ocean coverage
    assert!(app.world().get::<GeologicalState>(valkyrie).is_none());
    let valk_vol = app.world().get::<VolatileInventory>(valkyrie).unwrap();
    let valk_temp = app.world().get::<Temperature>(valkyrie).unwrap();
    let valk_clim = app.world().get::<PlanetaryClimate>(valkyrie).unwrap();

    assert_eq!(valk_vol.ocean_coverage_frac, 0.0);
    assert_eq!(valk_vol.atmospheric_pressure_bar, 0.0);
    assert_eq!(valk_temp.0, 1800.0);
    assert_eq!(valk_clim.surface_temperature_k, 1800.0);
    assert_eq!(valk_clim.climate_regime, ClimateRegime::AirlessVacuum);
}
