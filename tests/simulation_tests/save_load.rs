//! Verification tests for System State Save / Load & Scenario Serializer (JSON).

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::accretion::TheiaImpactState;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::serialization::*;

#[test]
fn test_normalize_save_path() {
    assert_eq!(normalize_save_path(""), "saves/quicksave.json");
    assert_eq!(normalize_save_path("   "), "saves/quicksave.json");
    assert_eq!(
        normalize_save_path("solar_system.json"),
        "saves/solar_system.json"
    );
    assert_eq!(
        normalize_save_path("saves/my_save.json"),
        "saves/my_save.json"
    );
    assert_eq!(
        normalize_save_path("custom/dir/save.json"),
        "custom/dir/save.json"
    );
}

#[test]
fn test_system_save_data_serialization_round_trip() {
    let earth_save = CelestialBodySave {
        name: "Earth".to_string(),
        body_type: BodyType::TerrestrialPlanet,
        position: DVec3::new(1.0, 0.0, 0.0),
        velocity: DVec3::new(0.0, 0.0, std::f64::consts::TAU),
        mass: 3.003e-6,
        radius: 4.26e-5,
        temperature: 288.0,
        luminosity: 0.0,
        composition: Composition::rocky(),
        spin: SpinState::default(),
        differentiation: Some(InternalDifferentiation::default()),
        volatile_inventory: Some(VolatileInventory {
            delivered_water_m_earth: 1.0,
            ocean_coverage_frac: 0.71,
            atmospheric_pressure_bar: 1.0,
            cometary_impact_count: 5,
        }),
        ring_system: None,
        basins: None,
        climate: Some(PlanetaryClimate::default()),
        biosphere: Some(BiosphereState::default()),
        electromagnetic: None,
        is_central_star: false,
        ignition_state: None,
        stellar_evolution: None,
        black_hole_state: None,
        satellite: None,
        tidal_state: None,
        relativistic_state: None,
        atmospheric_escape: None,
        kozai_lidov: None,
    };

    let moon_save = CelestialBodySave {
        name: "Moon".to_string(),
        body_type: BodyType::Moon,
        position: DVec3::new(1.00257, 0.0, 0.0),
        velocity: DVec3::new(0.0, 0.0, 6.48),
        mass: 3.69e-8,
        radius: 1.16e-5,
        temperature: 220.0,
        luminosity: 0.0,
        composition: Composition::silicate_rich(),
        spin: SpinState::default(),
        differentiation: None,
        volatile_inventory: None,
        ring_system: None,
        basins: None,
        climate: None,
        biosphere: None,
        electromagnetic: None,
        is_central_star: false,
        ignition_state: None,
        stellar_evolution: None,
        black_hole_state: None,
        satellite: Some(SatelliteSave {
            parent_name: "Earth".to_string(),
            semi_major_axis_au: 0.00257,
            orbital_period_years: 0.0748,
            true_anomaly: 1.25,
        }),
        tidal_state: None,
        relativistic_state: None,
        atmospheric_escape: None,
        kozai_lidov: None,
    };

    let save_data = SystemSaveData {
        version: 1,
        timestamp_epoch_yr: 4_500_000.0,
        step_count: 9_000_000,
        time_warp: TimeWarpSave {
            multiplier: 100.0,
            is_paused: false,
        },
        disk_parameters: DiskParameters::default(),
        config_save: ConfigSave {
            gas_density_scale: 0.85,
            size_exaggeration: 1.5,
        },
        bodies: vec![earth_save, moon_save],
        event_flags: EventFlagsSave {
            theia_moon_formed: true,
            lhb_active: false,
            lhb_resonance_crossed: true,
        },
    };

    let json =
        serde_json::to_string_pretty(&save_data).expect("Failed to serialize SystemSaveData");
    assert!(json.contains("\"name\": \"Earth\""));
    assert!(json.contains("\"parent_name\": \"Earth\""));
    assert!(json.contains("\"theia_moon_formed\": true"));

    let restored: SystemSaveData =
        serde_json::from_str(&json).expect("Failed to deserialize SystemSaveData");
    assert_eq!(restored.version, 1);
    assert_eq!(restored.timestamp_epoch_yr, 4_500_000.0);
    assert_eq!(restored.bodies.len(), 2);
    assert_eq!(restored.bodies.get(0).unwrap().name, "Earth");
    assert_eq!(restored.bodies.get(1).unwrap().name, "Moon");

    let moon = restored.bodies.get(1).unwrap();
    let sat = moon
        .satellite
        .as_ref()
        .expect("Expected moon satellite link");
    assert_eq!(sat.parent_name, "Earth");
    assert_eq!(sat.semi_major_axis_au, 0.00257);
}

#[test]
fn test_save_and_load_file_io() {
    let test_dir = "target/test_saves";
    let file_path = "target/test_saves/io_roundtrip.json";

    let body = CelestialBodySave {
        name: "Test World".to_string(),
        body_type: BodyType::SuperEarth,
        position: DVec3::new(0.5, 0.0, 0.0),
        velocity: DVec3::new(0.0, 0.0, 8.88),
        mass: 1.5e-5,
        radius: 6.0e-5,
        temperature: 420.0,
        luminosity: 0.0,
        composition: Composition::metal_rich(),
        spin: SpinState::default(),
        differentiation: None,
        volatile_inventory: None,
        ring_system: None,
        basins: None,
        climate: None,
        biosphere: None,
        electromagnetic: None,
        is_central_star: false,
        ignition_state: None,
        stellar_evolution: None,
        black_hole_state: None,
        satellite: None,
        tidal_state: None,
        relativistic_state: None,
        atmospheric_escape: None,
        kozai_lidov: None,
    };

    let save_data = SystemSaveData {
        version: 1,
        timestamp_epoch_yr: 12345.0,
        step_count: 500,
        time_warp: TimeWarpSave::default(),
        disk_parameters: DiskParameters::default(),
        config_save: ConfigSave::default(),
        bodies: vec![body],
        event_flags: EventFlagsSave::default(),
    };

    save_system_to_file(&save_data, file_path).expect("Failed to write test save file");
    assert!(std::path::Path::new(file_path).exists());

    let loaded = load_system_from_file(file_path).expect("Failed to read test save file");
    assert_eq!(loaded.timestamp_epoch_yr, 12345.0);
    assert_eq!(loaded.bodies.len(), 1);
    assert_eq!(loaded.bodies.get(0).unwrap().name, "Test World");

    let _ = std::fs::remove_file(file_path);
    let _ = std::fs::remove_dir(test_dir);
}

#[test]
fn test_bevy_save_system_event_handling() {
    let save_path = "target/test_saves/bevy_save_test.json";

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<TheiaImpactState>()
        .add_message::<SaveSystemEvent>()
        .add_systems(Update, handle_save_system_events);

    // Spawn a star and a planet
    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "The Sun".to_string(),
        },
        CentralStar,
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Radius(0.00465),
        Temperature(5778.0),
        Luminosity(1.0),
        Composition::solar_gas(),
        SpinState::default(),
    ));

    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::TerrestrialPlanet,
            name: "Mars".to_string(),
        },
        Mass(3.2e-7),
        SimPosition(DVec3::new(1.52, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, 5.0)),
        Radius(2.2e-5),
        Temperature(210.0),
        Luminosity(0.0),
        Composition::rocky(),
        SpinState::default(),
    ));

    // Send SaveSystemEvent
    let mut writer = app.world_mut().resource_mut::<Messages<SaveSystemEvent>>();
    writer.write(SaveSystemEvent {
        filename: save_path.to_string(),
    });

    app.update();

    assert!(
        std::path::Path::new(save_path).exists(),
        "Save file was not created"
    );
    let loaded = load_system_from_file(save_path).expect("Failed to load saved Bevy state");
    assert_eq!(loaded.bodies.len(), 2);
    let names: Vec<String> = loaded.bodies.into_iter().map(|b| b.name).collect();
    assert!(names.contains(&"The Sun".to_string()));
    assert!(names.contains(&"Mars".to_string()));

    let _ = std::fs::remove_file(save_path);
}

#[test]
fn test_bevy_load_system_event_satellite_resolution() {
    let load_path = "target/test_saves/bevy_load_test.json";

    let earth_save = CelestialBodySave {
        name: "Earth".to_string(),
        body_type: BodyType::TerrestrialPlanet,
        position: DVec3::new(1.0, 0.0, 0.0),
        velocity: DVec3::new(0.0, 0.0, std::f64::consts::TAU),
        mass: 3.003e-6,
        radius: 4.26e-5,
        temperature: 288.0,
        luminosity: 0.0,
        composition: Composition::rocky(),
        spin: SpinState::default(),
        differentiation: None,
        volatile_inventory: None,
        ring_system: None,
        basins: None,
        climate: None,
        biosphere: None,
        electromagnetic: None,
        is_central_star: false,
        ignition_state: None,
        stellar_evolution: None,
        black_hole_state: None,
        satellite: None,
        tidal_state: None,
        relativistic_state: None,
        atmospheric_escape: None,
        kozai_lidov: None,
    };

    let moon_save = CelestialBodySave {
        name: "The Moon".to_string(),
        body_type: BodyType::Moon,
        position: DVec3::new(1.00257, 0.0, 0.0),
        velocity: DVec3::new(0.0, 0.0, 6.48),
        mass: 3.69e-8,
        radius: 1.16e-5,
        temperature: 220.0,
        luminosity: 0.0,
        composition: Composition::silicate_rich(),
        spin: SpinState::default(),
        differentiation: None,
        volatile_inventory: None,
        ring_system: None,
        basins: None,
        climate: None,
        biosphere: None,
        electromagnetic: None,
        is_central_star: false,
        ignition_state: None,
        stellar_evolution: None,
        black_hole_state: None,
        satellite: Some(SatelliteSave {
            parent_name: "Earth".to_string(),
            semi_major_axis_au: 0.00257,
            orbital_period_years: 0.0748,
            true_anomaly: 0.5,
        }),
        tidal_state: None,
        relativistic_state: None,
        atmospheric_escape: None,
        kozai_lidov: None,
    };

    let test_save = SystemSaveData {
        version: 1,
        timestamp_epoch_yr: 999.0,
        step_count: 100,
        time_warp: TimeWarpSave::default(),
        disk_parameters: DiskParameters::default(),
        config_save: ConfigSave::default(),
        bodies: vec![earth_save, moon_save],
        event_flags: EventFlagsSave::default(),
    };

    save_system_to_file(&test_save, load_path).expect("Failed to prepare load test file");

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<TheiaImpactState>()
        .add_message::<LoadSystemEvent>()
        .add_systems(Update, handle_load_system_events);

    // Pre-populate with an old body to verify despawn
    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::Planetesimal,
            name: "Old Embryo To Despawn".to_string(),
        },
        Mass(1e-6),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Radius(0.001),
        Temperature(200.0),
        Luminosity(0.0),
        Composition::rocky(),
        SpinState::default(),
    ));

    // Send LoadSystemEvent
    let mut writer = app.world_mut().resource_mut::<Messages<LoadSystemEvent>>();
    writer.write(LoadSystemEvent {
        filename: load_path.to_string(),
    });

    app.update();

    let mut earth_ent = None;
    let mut moon_sat = None;

    let mut bodies_query = app
        .world_mut()
        .query::<(Entity, &CelestialBody, Option<&SatelliteOf>)>();
    for (ent, body, opt_sat) in bodies_query.iter(app.world()) {
        if body.name == "Earth" {
            earth_ent = Some(ent);
        } else if body.name == "The Moon" {
            moon_sat = opt_sat.copied();
        }
        assert_ne!(
            body.name, "Old Embryo To Despawn",
            "Old body was not despawned"
        );
    }

    assert!(earth_ent.is_some(), "Earth was not spawned");
    assert!(
        moon_sat.is_some(),
        "The Moon did not have SatelliteOf component attached"
    );

    let sat = moon_sat.unwrap();
    assert_eq!(
        sat.parent,
        earth_ent.unwrap(),
        "SatelliteOf parent does not match Earth entity"
    );
    assert_eq!(sat.semi_major_axis_au, 0.00257);

    let _ = std::fs::remove_file(load_path);
}
