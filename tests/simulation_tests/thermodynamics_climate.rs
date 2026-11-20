//! Integration tests for planetary climate regimes, ice-albedo feedbacks,
//! desiccated cloud cover, moon outgassing, and H2O phase diagram condensation.

use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;

fn setup_milankovitch_test_app() -> (App, Entity) {
    use bevy::math::DVec3;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();

    // Central Sun-like star (1.0 M_sun, 1.0 L_sun, 5778 K)
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::MainSequenceStar,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Luminosity(1.0),
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 50.0,
        },
    ));

    // Earth-like planet at 1.0 AU with water volatiles
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Proto-Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 29.78)),
            Temperature(288.0),
            Composition {
                metal_frac: 0.32,
                silicate_frac: 0.66,
                ice_frac: 0.02,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            VolatileInventory {
                delivered_water_m_earth: 0.0006,
                ocean_coverage_frac: 0.71,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 5,
            },
        ))
        .id();

    app.add_systems(Update, update_thermodynamics);
    (app, earth_ent)
}

#[test]
fn test_milankovitch_ice_albedo_feedback_and_polar_ice_advance() {
    use bevy::math::DVec3;

    let (mut app, earth_ent) = setup_milankovitch_test_app();

    // 1. Initial temperate equilibrium at 1.0 AU
    app.update();

    let climate = *app
        .world()
        .get::<PlanetaryClimate>(earth_ent)
        .expect("PlanetaryClimate must be inserted");
    assert_eq!(climate.climate_regime, ClimateRegime::TemperateHabitable);
    assert!(
        (climate.surface_temperature_k - 288.0).abs() < 5.0,
        "Surface temp should be ~288 K (got {:.1} K)",
        climate.surface_temperature_k
    );
    assert!(
        climate.ice_coverage_frac > 0.05 && climate.ice_coverage_frac < 0.20,
        "Earth ice coverage should be ~10% (got {:.1}%)",
        climate.ice_coverage_frac * 100.0
    );
    assert!(
        climate.polar_ice_cap_latitude_deg > 70.0 && climate.polar_ice_cap_latitude_deg < 88.0,
        "Earth polar ice caps should sit above 70° latitude (got {:.1}°)",
        climate.polar_ice_cap_latitude_deg
    );
    assert!(
        (climate.albedo - 0.29).abs() < 0.05,
        "Earth albedo should be ~0.29 (got {:.2})",
        climate.albedo
    );

    // 2. Move planet out to 1.25 AU (glacial advance / Milankovitch winter)
    {
        let mut pos = app.world_mut().get_mut::<SimPosition>(earth_ent).unwrap();
        pos.0 = DVec3::new(1.25, 0.0, 0.0);
    }
    app.update();

    let cold_climate = app.world().get::<PlanetaryClimate>(earth_ent).unwrap();
    assert!(
        cold_climate.surface_temperature_k < climate.surface_temperature_k,
        "Planet at 1.25 AU must cool down"
    );
    assert!(
        cold_climate.ice_coverage_frac > climate.ice_coverage_frac,
        "Ice caps must advance equatorward as planet cools"
    );
    assert!(
        cold_climate.polar_ice_cap_latitude_deg < climate.polar_ice_cap_latitude_deg,
        "Ice cap edge must move to lower latitudes (got {:.1}°)",
        cold_climate.polar_ice_cap_latitude_deg
    );
    assert!(
        cold_climate.albedo > climate.albedo,
        "Ice-albedo positive feedback must increase planetary albedo"
    );
}

#[test]
fn test_climate_snowball_and_hothouse_regimes() {
    use bevy::math::DVec3;

    let (mut app, earth_ent) = setup_milankovitch_test_app();

    // 3. Move planet out to 1.65 AU (runaway Snowball Earth)
    {
        let mut pos = app.world_mut().get_mut::<SimPosition>(earth_ent).unwrap();
        pos.0 = DVec3::new(1.65, 0.0, 0.0);
        let mut temp = app.world_mut().get_mut::<Temperature>(earth_ent).unwrap();
        temp.0 = 230.0;
    }
    app.update();

    let snowball = app.world().get::<PlanetaryClimate>(earth_ent).unwrap();
    assert_eq!(
        snowball.climate_regime,
        ClimateRegime::SnowballIceAge,
        "Planet at 1.65 AU must enter Snowball Ice Age"
    );
    assert_eq!(
        snowball.ice_coverage_frac, 1.0,
        "Snowball Earth must have 100% ice coverage"
    );
    assert_eq!(
        snowball.polar_ice_cap_latitude_deg, 0.0,
        "Snowball Earth ice caps must reach the equator (0°)"
    );
    assert!(
        snowball.albedo > 0.65,
        "Snowball Earth albedo must exceed 0.65 (got {:.2})",
        snowball.albedo
    );

    // 4. Move planet into 0.78 AU (warm greenhouse / ice-free poles)
    {
        let mut pos = app.world_mut().get_mut::<SimPosition>(earth_ent).unwrap();
        pos.0 = DVec3::new(0.78, 0.0, 0.0);
        let mut temp = app.world_mut().get_mut::<Temperature>(earth_ent).unwrap();
        temp.0 = 315.0;
    }
    app.update();

    let hothouse = app.world().get::<PlanetaryClimate>(earth_ent).unwrap();
    assert_eq!(
        hothouse.ice_coverage_frac, 0.0,
        "Warm planet must have completely melted ice caps"
    );
    assert_eq!(
        hothouse.polar_ice_cap_latitude_deg, 90.0,
        "Ice-free planet ice cap latitude must be 90° (at the pole)"
    );
    assert!(
        hothouse.albedo < 0.28,
        "Ice-free water world must have low bare albedo (got {:.2})",
        hothouse.albedo
    );
}

#[test]
fn test_desiccated_planet_with_gas_atmosphere_not_airless_and_no_steam_clouds() {
    use bevy::math::DVec3;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();

    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::MainSequenceStar,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Luminosity(1.0),
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 50.0,
        },
    ));

    // Desiccated Earth (0% water, 3% gas, 3.38 bar atmospheric pressure at 369 K)
    let dry_earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.31 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 1.15),
            SimPosition(DVec3::new(0.99, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 30.3)),
            Temperature(369.0),
            Composition {
                silicate_frac: 0.66,
                ice_frac: 0.0,
                metal_frac: 0.31,
                organics_frac: 0.0,
                gas_frac: 0.03,
            },
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 3.38,
                cometary_impact_count: 0,
            },
        ))
        .id();

    app.add_systems(Update, update_thermodynamics);
    app.update();

    let climate = app
        .world()
        .get::<PlanetaryClimate>(dry_earth)
        .expect("PlanetaryClimate must be inserted");
    assert_ne!(
        climate.climate_regime,
        ClimateRegime::AirlessVacuum,
        "A planet with 3.38 bar atmospheric pressure and 3% gas must never be classified as AirlessVacuum"
    );
    assert!(
        climate.cloud_coverage_frac >= 0.25 && climate.cloud_coverage_frac <= 0.75,
        "A desiccated planet with 3% gas and 3.38 bar must have visible volatile cloud cover (25%..75%), not 0% or 100% boiling-ocean whiteout (got {:.2})",
        climate.cloud_coverage_frac
    );
}

fn spawn_moon_thermodynamics_test_app() -> App {
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Protostar".to_string(),
            body_type: BodyType::TTauriStar,
        },
        Mass(1.56),
        Radius(SOLAR_RADIUS_AU * 1.42),
        Temperature(7225.0),
        Luminosity(10.0),
        IgnitionState {
            core_temperature: 1.0e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 30.0,
        },
    ));
    app.add_systems(Update, update_thermodynamics);
    app
}

#[test]
fn test_low_mass_moon_water_vacuum_vs_cold_ice_and_transient_steam() {
    use bevy::math::DVec3;
    use protostellar::simulation::thermodynamics::*;

    assert!(water_saturation_vapor_pressure_bar(100.0) < 1e-12);
    assert!(water_saturation_vapor_pressure_bar(150.0) < 1e-9);
    let p_triple = water_saturation_vapor_pressure_bar(273.16);
    assert!(
        (p_triple - 0.00612).abs() < 1e-4,
        "Triple point: {p_triple}"
    );
    let p_281 = water_saturation_vapor_pressure_bar(281.0);
    assert!(p_281 > 0.009 && p_281 < 0.013, "P_sat(281 K): {p_281}");

    let mut app = spawn_moon_thermodynamics_test_app();
    let comp_hydrated_rock = Composition {
        silicate_frac: 0.69,
        ice_frac: 0.05,
        metal_frac: 0.27,
        organics_frac: 0.0,
        gas_frac: 0.0,
    }
    .normalized();

    let warm_rocky_moon = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Super-Jupiter XXIV (Moon)".to_string(),
                body_type: BodyType::Moon,
            },
            Mass(0.0081 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.22),
            SimPosition(DVec3::new(3.16, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 4.74)),
            Temperature(281.0),
            comp_hydrated_rock,
            VolatileInventory::default(),
        ))
        .id();

    let cold_icy_moon = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Cold Icy Moon".to_string(),
                body_type: BodyType::Moon,
            },
            Mass(0.0081 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.24),
            SimPosition(DVec3::new(15.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.0)),
            Temperature(100.0),
            Composition::icy(),
            VolatileInventory::default(),
        ))
        .id();

    let sublimating_moon = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Warm Sublimating Moon".to_string(),
                body_type: BodyType::Moon,
            },
            Mass(0.0081 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.24),
            SimPosition(DVec3::new(3.16, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 4.74)),
            Temperature(281.0),
            Composition::icy(),
            VolatileInventory {
                delivered_water_m_earth: 0.0001,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.0,
                cometary_impact_count: 1,
            },
        ))
        .id();

    app.update();

    let warm_vol = app
        .world()
        .get::<VolatileInventory>(warm_rocky_moon)
        .unwrap();
    let warm_clim = app
        .world()
        .get::<PlanetaryClimate>(warm_rocky_moon)
        .unwrap();
    assert_eq!(warm_vol.ocean_coverage_frac, 0.0);
    assert_eq!(warm_vol.atmospheric_pressure_bar, 0.0);
    assert_eq!(warm_clim.climate_regime, ClimateRegime::AirlessVacuum);
    assert_eq!(warm_clim.ice_coverage_frac, 0.0);

    let cold_vol = app.world().get::<VolatileInventory>(cold_icy_moon).unwrap();
    let cold_clim = app.world().get::<PlanetaryClimate>(cold_icy_moon).unwrap();
    assert_eq!(cold_vol.ocean_coverage_frac, 0.0);
    assert!(cold_vol.atmospheric_pressure_bar < 0.001);
    assert_eq!(cold_clim.climate_regime, ClimateRegime::AirlessVacuum);
    assert!(cold_clim.ice_coverage_frac > 0.5);

    let sub_vol = app
        .world()
        .get::<VolatileInventory>(sublimating_moon)
        .unwrap();
    let sub_clim = app
        .world()
        .get::<PlanetaryClimate>(sublimating_moon)
        .unwrap();
    assert_eq!(sub_vol.ocean_coverage_frac, 0.0);
    assert!(
        sub_vol.atmospheric_pressure_bar > 0.005 && sub_vol.atmospheric_pressure_bar < 0.02,
        "Warm exposed water in vacuum must sublimate transient H2O vapor, got {}",
        sub_vol.atmospheric_pressure_bar
    );
    assert_eq!(sub_clim.climate_regime, ClimateRegime::AirlessVacuum);
}

#[test]
fn test_h2o_phase_diagram_reversible_condensation_and_freezing() {
    use bevy::math::DVec3;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::*;

    // 1. Anchored saturation curve + inverse boiling point bijection across triple, 1 atm, critical
    let p_triple = water_saturation_vapor_pressure_bar(273.16);
    assert!((p_triple - 0.006_116_57).abs() < 1e-5);
    assert!((water_boiling_point_k(p_triple) - 273.16).abs() < 0.05);

    let p_1atm = water_saturation_vapor_pressure_bar(373.15);
    assert!(
        (p_1atm - 1.01325).abs() < 1e-3,
        "P_sat(373.15 K) should equal 1 atm (1.01325 bar), got {p_1atm}"
    );
    assert!((water_boiling_point_k(1.01325) - 373.15).abs() < 0.05);

    let p_crit = water_saturation_vapor_pressure_bar(647.096);
    assert!((p_crit - 220.64).abs() < 0.5);
    assert!((water_boiling_point_k(220.64) - 647.096).abs() < 0.1);

    // 2. Hydrostatic pressure / mass bijection on Earth
    let p_1bar_mass = gas_mass_for_pressure_bar(1.0, EARTH_MASS_SOLAR, EARTH_RADIUS_AU);
    assert!(
        (p_1bar_mass - 8.69e-7).abs() < 2e-8,
        "1 bar on Earth requires ~8.7e-7 M_earth gas (got {p_1bar_mass:.3e})"
    );
    let roundtrip_p = hydrostatic_pressure_bar(p_1bar_mass, EARTH_MASS_SOLAR, EARTH_RADIUS_AU);
    assert!((roundtrip_p - 1.0).abs() < 1e-4);

    // 3. Reversible supercritical steam condensation + below-273.16 K freezing in ECS
    let mut app = App::new();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Luminosity(1.0),
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 30.0,
        },
    ));
    let mut trace_ice_comp = Composition::rocky();
    trace_ice_comp.ice_frac = 0.002;
    let trace_ice_comp = trace_ice_comp.normalized();
    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Reversible Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 29.78)),
            Temperature(900.0),
            trace_ice_comp,
            VolatileInventory {
                delivered_water_m_earth: 0.00045,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 1,
            },
        ))
        .id();
    app.add_systems(Update, update_thermodynamics);

    // Step A: Supercritical 900 K
    app.update();
    let hot_p = app
        .world()
        .get::<VolatileInventory>(planet)
        .unwrap()
        .atmospheric_pressure_bar;
    assert!(
        hot_p > 100.0,
        "Supercritical water must build dense steam pressure (>100 bar, got {hot_p})"
    );
    // Shockwave front already at 30 AU must not keep eroding inner-planet ice
    let ice_after = app.world().get::<Composition>(planet).unwrap().ice_frac;
    assert!(
        (ice_after - trace_ice_comp.ice_frac).abs() < 1e-6,
        "Stalled 30 AU shockwave must not erode 1 AU ice every frame"
    );

    // Step B: Cool to 288 K -> steam pressure must reversibly condense back down
    app.world_mut().get_mut::<Temperature>(planet).unwrap().0 = 288.0;
    app.update();
    let cool_vol = app.world().get::<VolatileInventory>(planet).unwrap();
    assert!(
        cool_vol.atmospheric_pressure_bar < 2.0,
        "Condensing steam must release reversible pressure rather than ratcheting (got {:.2} bar)",
        cool_vol.atmospheric_pressure_bar
    );
    assert!(
        cool_vol.ocean_coverage_frac > 0.70,
        "Condensed steam must fill surface oceans"
    );

    // Step C: Move outside frost line (3.5 AU) and freeze at 220 K -> liquid ocean becomes 0, solid surface ice covers globe even with minimal comp.ice_frac
    app.world_mut().get_mut::<SimPosition>(planet).unwrap().0 = DVec3::new(3.5, 0.0, 0.0);
    app.world_mut().get_mut::<Temperature>(planet).unwrap().0 = 220.0;
    app.update();
    let frozen_vol = app.world().get::<VolatileInventory>(planet).unwrap();
    let frozen_clim = app.world().get::<PlanetaryClimate>(planet).unwrap();
    assert_eq!(frozen_vol.ocean_coverage_frac, 0.0);
    assert_eq!(frozen_clim.climate_regime, ClimateRegime::SnowballIceAge);
    assert_eq!(frozen_clim.ice_coverage_frac, 1.0);
}
