//! Geochemical ocean origin and multi-star habitability integration tests.

#[test]
fn test_four_stage_geochemical_earth_ocean_origin_and_critical_point_deluge() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::game::ui::telemetry_format::format_geology_and_volatiles_telemetry;
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::*;
    use protostellar::utils::constants::*;

    // Stage 1: Endogenous Enstatite Chondrite (-OH) + H2/FeO magma ocean oxidation water budget
    let earth_comp = Composition::rocky();
    let endogenous_water =
        compute_endogenous_water_m_earth(EARTH_MASS_SOLAR, &earth_comp, 1.0, 1.0);
    assert!(
        endogenous_water > 0.00035 && endogenous_water < 0.00060,
        "1 M_earth rocky planet in gas-rich inner disk must acquire ~0.00043 M_earth endogenous water (got {endogenous_water:.6})"
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.insert_resource(SimTime {
        elapsed_years: 1000.0,
        current_dt_yr: 1.0,
        visual_time_secs: 1.0,
        ..Default::default()
    });
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

    // Stage 3a: Hot Hadean Proto-Earth above the 647 K critical point of water (T = 900 K)
    let proto_earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 29.8)),
            Temperature(900.0),
            earth_comp,
            VolatileInventory {
                delivered_water_m_earth: endogenous_water,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 0,
            },
        ))
        .id();

    app.add_systems(Update, update_thermodynamics);
    app.update();

    {
        let vol = app.world().get::<VolatileInventory>(proto_earth).unwrap();
        let climate = app.world().get::<PlanetaryClimate>(proto_earth).unwrap();
        assert_eq!(
            vol.ocean_coverage_frac, 0.0,
            "Above 647 K critical point, water must remain in supercritical steam atmosphere (0% liquid ocean)"
        );
        assert!(
            climate.cloud_coverage_frac >= 0.90,
            "Supercritical steam envelope must produce dense cloud cover (got {:.2})",
            climate.cloud_coverage_frac
        );

        // Stage 4: Telemetry readout indicates supercritical steam outgassing
        let telemetry = format_geology_and_volatiles_telemetry(None, Some(vol), None);
        assert!(
            telemetry.contains("[Supercritical Steam Outgassing — Condenses < 647 K]"),
            "Hot protoplanet telemetry must report supercritical steam outgassing: {telemetry}"
        );
    }

    // Stage 3b: The Great Deluge — crust cools below 647 K down to temperate 288 K
    app.world_mut()
        .get_mut::<Temperature>(proto_earth)
        .unwrap()
        .0 = 288.0;
    app.update();

    {
        let vol = app.world().get::<VolatileInventory>(proto_earth).unwrap();
        assert!(
            vol.ocean_coverage_frac > 0.65,
            "Cooled Earth must rain out its endogenous steam into global Archean oceans (>65% coverage, got {:.2})",
            vol.ocean_coverage_frac
        );
        let telemetry = format_geology_and_volatiles_telemetry(None, Some(vol), None);
        assert!(
            !telemetry.contains("[Supercritical Steam Outgassing — Condenses < 647 K]"),
            "Once condensed into oceans, supercritical steam indicator must disappear"
        );
    }

    // Stage 5: Even if atmospheric_pressure_bar starts at 0.0 or undergoes 2.5 Myr of XUV/solar-wind escape,
    // a terrestrial planet with liquid oceans maintains its outgassed N2/CO2/H2O atmosphere (never Airless Vacuum).
    app.world_mut()
        .get_mut::<VolatileInventory>(proto_earth)
        .unwrap()
        .atmospheric_pressure_bar = 0.0;
    app.update();
    {
        let vol = app.world().get::<VolatileInventory>(proto_earth).unwrap();
        let climate = app.world().get::<PlanetaryClimate>(proto_earth).unwrap();
        assert!(
            vol.atmospheric_pressure_bar >= 0.50,
            "Ocean-bearing terrestrial planet must maintain outgassed N2/CO2/H2O atmospheric pressure (got {:.3} bar)",
            vol.atmospheric_pressure_bar
        );
        assert_ne!(
            climate.climate_regime,
            ClimateRegime::AirlessVacuum,
            "A planet with surface water must never classify as AirlessVacuum"
        );
    }
}

#[test]
fn test_habitability_tracking_multi_star_pigment_and_bioluminescence() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::biology::civilization::{
        HabitabilityTimer, habitability_tracking_system,
    };
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::tides::TidalState;
    use protostellar::utils::math::OrbitalElements;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<TimeWarp>()
        .insert_resource(SimTime {
            elapsed_years: 2_000_000.0,
            current_dt_yr: 120_000.0,
            ..default()
        })
        .add_systems(Update, habitability_tracking_system);

    app.world_mut().spawn((
        CelestialBody {
            name: "Cool M-Dwarf".to_string(),
            body_type: BodyType::RedDwarf,
        },
        Temperature(2800.0),
        SimPosition(DVec3::ZERO),
        CentralStar,
    ));
    app.world_mut().spawn((
        CelestialBody {
            name: "Hot Blue Giant".to_string(),
            body_type: BodyType::BlueGiant,
        },
        Temperature(12000.0),
        SimPosition(DVec3::new(500.0, 0.0, 0.0)),
    ));

    let temperate_climate = PlanetaryClimate {
        surface_temperature_k: 290.0,
        equilibrium_temperature_k: 255.0,
        greenhouse_delta_k: 35.0,
        albedo: 0.30,
        ice_coverage_frac: 0.1,
        cloud_coverage_frac: 0.4,
        climate_regime: ClimateRegime::TemperateHabitable,
        polar_ice_cap_latitude_deg: 72.0,
    };
    let stable_orbit = TrackedOrbit {
        central_entity: None,
        elements: OrbitalElements {
            eccentricity: 0.02,
            ..default()
        },
        last_updated_yr: 0.0,
    };

    let m_world = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(0.1, 0.0, 0.0)),
            temperate_climate,
            stable_orbit.clone(),
            TidalState {
                is_tidally_locked: true,
                ..default()
            },
            HabitabilityTimer(0.0),
        ))
        .id();

    let uv_world = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(505.0, 0.0, 0.0)),
            temperate_climate,
            stable_orbit,
            BiosphereState {
                habitability_score: 0.7,
                biomass_coverage_frac: 0.5,
                oxygen_fraction: 0.2,
                emergence_year: Some(1_000_000.0),
                flora_pigment: FloraPigmentClass::SolarChlorophyll,
                ..default()
            },
        ))
        .id();

    app.update();

    let m_bio = app
        .world()
        .get::<BiosphereState>(m_world)
        .expect("M-dwarf habitable planet past 100 kyr timer must emerge life");
    assert_eq!(m_bio.flora_pigment, FloraPigmentClass::RedDwarfCrimson);
    assert!(
        m_bio.bioluminescence_intensity >= 0.35,
        "Tidally locked world must initialize elevated bioluminescence (got {})",
        m_bio.bioluminescence_intensity
    );

    let uv_bio = app
        .world()
        .get::<BiosphereState>(uv_world)
        .expect("Blue giant habitable world must keep BiosphereState");
    assert_eq!(uv_bio.flora_pigment, FloraPigmentClass::HighUvIndigo);
    assert!(
        uv_bio.technosignature > 0.0,
        "Established biosphere on low-eccentricity orbit must accrue technosignature"
    );
}
