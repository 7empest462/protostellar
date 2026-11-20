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
        ..Default::default()
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
            parent_id: None,
            parent_name: "Earth".to_string(),
            semi_major_axis_au: 0.00257,
            orbital_period_years: 0.0748,
            true_anomaly: 1.25,
        }),
        tidal_state: None,
        relativistic_state: None,
        atmospheric_escape: None,
        kozai_lidov: None,
        ..Default::default()
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
            ..Default::default()
        },
        scenario_preset: None,
        ..Default::default()
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
    assert_eq!(restored.bodies.first().unwrap().name, "Earth");
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
    let test_dir = "target/test_saves_io";
    let file_path = "target/test_saves_io/io_roundtrip.json";

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
        ..Default::default()
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
        scenario_preset: None,
        ..Default::default()
    };

    save_system_to_file(&save_data, file_path).expect("Failed to write test save file");
    assert!(std::path::Path::new(file_path).exists());

    let loaded = load_system_from_file(file_path).expect("Failed to read test save file");
    assert_eq!(loaded.timestamp_epoch_yr, 12345.0);
    assert_eq!(loaded.bodies.len(), 1);
    assert_eq!(loaded.bodies.first().unwrap().name, "Test World");

    let _ = std::fs::remove_file(file_path);
    let _ = std::fs::remove_dir(test_dir);
}

#[test]
fn test_bevy_save_system_event_handling() {
    let save_path = "target/test_saves_bevy/bevy_save_test.json";

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

fn make_satellite_test_earth_save() -> CelestialBodySave {
    CelestialBodySave {
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
        ..Default::default()
    }
}

fn make_satellite_test_moon_save() -> CelestialBodySave {
    CelestialBodySave {
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
            parent_id: None,
            parent_name: "Earth".to_string(),
            semi_major_axis_au: 0.00257,
            orbital_period_years: 0.0748,
            true_anomaly: 0.5,
        }),
        tidal_state: None,
        relativistic_state: None,
        atmospheric_escape: None,
        kozai_lidov: None,
        ..Default::default()
    }
}

fn make_satellite_test_save_data() -> SystemSaveData {
    SystemSaveData {
        version: 1,
        timestamp_epoch_yr: 999.0,
        step_count: 100,
        time_warp: TimeWarpSave::default(),
        disk_parameters: DiskParameters::default(),
        config_save: ConfigSave::default(),
        bodies: vec![
            make_satellite_test_earth_save(),
            make_satellite_test_moon_save(),
        ],
        event_flags: EventFlagsSave::default(),
        scenario_preset: None,
        ..Default::default()
    }
}

#[test]
fn test_bevy_load_system_event_satellite_resolution() {
    let load_path = "target/test_saves_load/bevy_load_test.json";
    let test_save = make_satellite_test_save_data();

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

#[test]
fn test_kozai_lidov_null_deserialization() {
    use protostellar::simulation::kozai_lidov::types::KozaiLidovState;

    let json = r#"{
        "perturber_name": "Outer Star",
        "mutual_inclination_deg": 45.0,
        "critical_inclination_deg": 39.23,
        "is_in_resonance": true,
        "max_eccentricity_forecast": 0.85,
        "min_periastron_au": 1.5,
        "kozai_period_years": null,
        "gr_precession_ratio": null,
        "is_gr_suppressed": false,
        "regime": "Circulation",
        "cycle_phase": 0.25
    }"#;

    let kozai: KozaiLidovState =
        serde_json::from_str(json).expect("Failed to deserialize KozaiLidovState with nulls");
    assert!(kozai.kozai_period_years.is_infinite());
    assert_eq!(kozai.gr_precession_ratio, 0.0);
}

#[test]
fn test_load_real_quicksave_if_present() {
    let quicksave_path = "saves/quicksave.json";
    if std::path::Path::new(quicksave_path).exists() {
        let loaded =
            load_system_from_file(quicksave_path).expect("Failed to load real quicksave.json");
        assert!(
            !loaded.bodies.is_empty(),
            "Expected non-empty body list in quicksave"
        );
        let inferred =
            protostellar::simulation::serialization::infer_scenario_preset(&loaded.bodies);
        if loaded.scenario_preset.is_none() {
            let _ = inferred;
        }
    }
}

#[test]
fn test_infer_scenario_preset_differentiation() {
    use protostellar::simulation::scenarios::ScenarioPreset;

    let default_body = |name: &str, body_type: BodyType| CelestialBodySave {
        name: name.to_string(),
        body_type,
        position: DVec3::ZERO,
        velocity: DVec3::ZERO,
        mass: 1.0,
        radius: 0.005,
        temperature: 5000.0,
        luminosity: 1.0,
        composition: Composition::solar_gas(),
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
        ..Default::default()
    };

    // 1. Solar MMSN with 500 planetesimals: MUST infer SolarNebulaMmsn (not GMC Cluster!)
    let mut solar_bodies = Vec::new();
    solar_bodies.push(default_body("The Sun", BodyType::YellowDwarf));
    for i in 0..500 {
        solar_bodies.push(default_body(&format!("Asteroid {i}"), BodyType::Asteroid));
    }
    assert_eq!(
        infer_scenario_preset(&solar_bodies),
        ScenarioPreset::SolarNebulaMmsn,
        "Solar MMSN with many asteroids must NOT be misclassified as GMC Cluster"
    );

    // 2. Genesis with Protostar
    let genesis_bodies = vec![
        default_body("Genesis Protostar", BodyType::Protostar),
        default_body("Planetesimal 1", BodyType::Asteroid),
    ];
    assert_eq!(
        infer_scenario_preset(&genesis_bodies),
        ScenarioPreset::AccretionDiskGenesis,
        "Genesis with 'Genesis' in name must infer AccretionDiskGenesis"
    );

    // 3. Sagittarius A*
    let sgr_bodies = vec![
        default_body("Sagittarius A* (SMBH)", BodyType::BlackHole),
        default_body("S2 (B0V Early-Type Star)", BodyType::BlueSupergiant),
    ];
    assert_eq!(
        infer_scenario_preset(&sgr_bodies),
        ScenarioPreset::SagittariusAStar,
        "Sagittarius A* must infer SagittariusAStar rather than GMC Cluster"
    );

    // 4. Molecular Cloud Cluster with 4 stars
    let gmc_bodies = vec![
        default_body("Cluster Star Alpha", BodyType::BlueSupergiant),
        default_body("Cluster Star Beta", BodyType::RedGiant),
        default_body("Cluster Star Gamma", BodyType::YellowDwarf),
        default_body("Cluster Star Delta", BodyType::NeutronStar),
    ];
    assert_eq!(
        infer_scenario_preset(&gmc_bodies),
        ScenarioPreset::MolecularCloudCluster,
        "Systems with >3 stars must infer MolecularCloudCluster"
    );
}

#[test]
fn test_space_probe_excluded_from_serialization_and_despawn() {
    use protostellar::simulation::probes::{FlightComputer, ProbeType, SpaceProbe};

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<TheiaImpactState>()
        .add_message::<SaveSystemEvent>()
        .add_systems(Update, handle_save_system_events);

    // Spawn a normal planet
    let planet_ent = app
        .world_mut()
        .spawn((
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
        ))
        .id();

    // Spawn a space probe with CelestialBody
    let _probe_ent = app
        .world_mut()
        .spawn((
            SpaceProbe::new(ProbeType::Rover, 1),
            FlightComputer {
                target_entity: planet_ent,
                target_name: "Mars".to_string(),
                launch_pos: DVec3::new(1.52, 0.0, 0.0),
                distance_to_target: 0.0,
                relative_speed: 0.0,
            },
            CelestialBody {
                body_type: BodyType::Asteroid,
                name: "🚀 Rover #1 (to Mars)".to_string(),
            },
            Mass(0.0),
            SimPosition(DVec3::new(1.52, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Radius(1e-7),
            Temperature(290.0),
            Composition::metal_rich(),
            Luminosity(0.0),
            SpinState::default(),
        ))
        .id();

    let save_path = "target/test_saves_bevy/probe_exclusion_test.json";
    let mut writer = app.world_mut().resource_mut::<Messages<SaveSystemEvent>>();
    writer.write(SaveSystemEvent {
        filename: save_path.to_string(),
    });

    app.update();

    let saved = load_system_from_file(save_path).expect("Failed to read test save file");
    assert!(
        saved.bodies.iter().all(|b| !b.name.contains("Rover")),
        "SpaceProbes must NOT be serialized as celestial bodies"
    );
    assert_eq!(
        saved.bodies.len(),
        1,
        "Only the planet should be serialized"
    );

    let _ = std::fs::remove_file(save_path);
}

fn spawn_extended_subsystems_test_body(
    world: &mut World,
) -> (
    VolcanicState,
    AtmosphericStormState,
    protostellar::simulation::relativity::kerr::KerrBlackHole,
) {
    use protostellar::simulation::geology::{GeologicalEpoch, GeologicalState};
    use protostellar::simulation::neutron_stars::{MagnetarCrustStress, PulsarGlitchState};
    use protostellar::simulation::oort::OortComet;
    use protostellar::simulation::relativity::kerr::KerrBlackHole;
    use protostellar::simulation::space_weather::{
        AuroralOvalState, GeomagneticStormLevel, StellarFlareState,
    };
    use protostellar::simulation::terraforming::TerraformingAtmosphere;
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU};

    let geo = GeologicalState::new_for_epoch(GeologicalEpoch::Proterozoic);
    let terra = TerraformingAtmosphere {
        co2_pressure_bar: 1.25,
        nitrogen_pressure_bar: 0.65,
        ..Default::default()
    };
    let volc = VolcanicState::io();
    let storm = AtmosphericStormState::jupiter();
    let kerr = KerrBlackHole::sagittarius_a();
    let glitch = PulsarGlitchState {
        total_glitches_occurred: 3,
        ..Default::default()
    };
    let stress = MagnetarCrustStress {
        total_starquakes_count: 2,
        ..Default::default()
    };
    let oort = OortComet {
        semi_major_axis_au: 4200.0,
        is_infall_trajectory: true,
        ..Default::default()
    };
    let flare = StellarFlareState {
        cme_active: true,
        current_flare_intensity: 6.0,
        ..Default::default()
    };
    let aurora = AuroralOvalState {
        geomagnetic_kp_index: 7.5,
        storm_level: GeomagneticStormLevel::SevereStorm,
        ..Default::default()
    };
    let cv = protostellar::simulation::accretion::CataclysmicVariable {
        accreted_hydrogen_mass: 2.5e-5,
        nova_count: 4,
    };

    let mut ent = world.spawn((
        CelestialBody {
            body_type: BodyType::SuperEarth,
            name: "Extended-Subsystems-World".to_string(),
        },
        Mass(EARTH_MASS_SOLAR * 2.0),
        SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
        Radius(EARTH_RADIUS_AU * 1.3),
        Temperature(310.0),
        Composition::rocky(),
        Luminosity(0.0),
        SpinState::default(),
    ));
    ent.insert((
        geo, terra, volc, storm, kerr, glitch, stress, oort, flare, aurora, cv,
    ));
    (volc, storm, kerr)
}

#[test]
fn test_extended_subsystem_serialization_roundtrip() {
    use protostellar::simulation::geology::{GeologicalEpoch, GeologicalState};
    use protostellar::simulation::neutron_stars::{MagnetarCrustStress, PulsarGlitchState};
    use protostellar::simulation::oort::OortComet;
    use protostellar::simulation::relativity::kerr::KerrBlackHole;
    use protostellar::simulation::space_weather::{
        AuroralOvalState, GeomagneticStormLevel, StellarFlareState,
    };
    use protostellar::simulation::terraforming::TerraformingAtmosphere;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<DiskParameters>()
        .init_resource::<SimTime>()
        .init_resource::<TimeWarp>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<SimulationConfig>()
        .add_message::<SaveSystemEvent>()
        .add_message::<LoadSystemEvent>()
        .add_systems(
            Update,
            (handle_save_system_events, handle_load_system_events).chain(),
        );

    let (volc, storm, kerr) = spawn_extended_subsystems_test_body(app.world_mut());

    let save_path = "target/test_saves_bevy/extended_roundtrip_test.json";
    app.world_mut()
        .resource_mut::<Messages<SaveSystemEvent>>()
        .write(SaveSystemEvent {
            filename: save_path.to_string(),
        });
    app.update();

    let saved = load_system_from_file(save_path).expect("Load saved file");
    let seed = encode_system_seed_json(&saved).expect("Encode seed");
    assert!(seed.starts_with(SEED_PREFIX));
    let decoded = decode_system_seed_json(&seed).expect("Decode seed");
    assert_eq!(decoded.bodies.len(), 1);

    app.world_mut()
        .resource_mut::<Messages<LoadSystemEvent>>()
        .write(LoadSystemEvent {
            filename: save_path.to_string(),
        });
    app.update();

    let mut q = app.world_mut().query::<(
        &CelestialBody,
        &GeologicalState,
        &TerraformingAtmosphere,
        &VolcanicState,
        &AtmosphericStormState,
        &KerrBlackHole,
        &PulsarGlitchState,
        &MagnetarCrustStress,
        &OortComet,
        &StellarFlareState,
        &AuroralOvalState,
        &protostellar::simulation::accretion::CataclysmicVariable,
    )>();
    let (
        body,
        r_geo,
        r_terra,
        r_volc,
        r_storm,
        r_kerr,
        r_glitch,
        r_stress,
        r_oort,
        r_flare,
        r_aurora,
        r_cv,
    ) = q
        .single(app.world())
        .expect("All extended subsystem components must round-trip across save/load");
    assert_eq!(body.name, "Extended-Subsystems-World");
    assert_eq!(r_geo.epoch, GeologicalEpoch::Proterozoic);
    assert!((r_terra.co2_pressure_bar - 1.25).abs() < 1e-6);
    assert_eq!(r_volc.active_plumes, volc.active_plumes);
    assert!((r_storm.great_spot_size - storm.great_spot_size).abs() < 1e-6);
    assert!((r_kerr.spin_param - kerr.spin_param).abs() < 1e-9);
    assert_eq!(r_glitch.total_glitches_occurred, 3);
    assert_eq!(r_stress.total_starquakes_count, 2);
    assert!((r_oort.semi_major_axis_au - 4200.0).abs() < 1e-6);
    assert!(r_flare.cme_active);
    assert_eq!(r_aurora.storm_level, GeomagneticStormLevel::SevereStorm);
    assert_eq!(r_cv.nova_count, 4);
    assert!((r_cv.accreted_hydrogen_mass - 2.5e-5).abs() < 1e-12);

    let _ = std::fs::remove_file(save_path);
}

fn sample_io_save_data(
    preset: Option<protostellar::simulation::scenarios::ScenarioPreset>,
    epoch_yr: f64,
    step_count: u64,
) -> SystemSaveData {
    SystemSaveData {
        version: 1,
        timestamp_epoch_yr: epoch_yr,
        step_count,
        time_warp: TimeWarpSave::default(),
        disk_parameters: DiskParameters::default(),
        config_save: ConfigSave::default(),
        bodies: Vec::new(),
        event_flags: EventFlagsSave::default(),
        scenario_preset: preset,
        ..Default::default()
    }
}

#[test]
fn test_sanitize_and_resolve_save_paths() {
    assert_eq!(sanitize_save_slot_name(""), "quicksave");
    assert_eq!(sanitize_save_slot_name("   ...  "), "quicksave");
    assert_eq!(sanitize_save_slot_name("my_run.json"), "my_run");
    assert_eq!(sanitize_save_slot_name("alpha.json.json"), "alpha");
    assert_eq!(
        sanitize_save_slot_name("trappist/bad: name?.postcard"),
        "trappist_bad_name"
    );
    assert_eq!(
        resolve_save_path_for_write("trappist_custom.json"),
        "saves/trappist_custom.json"
    );
    assert_eq!(
        resolve_save_path_for_write("deep_binary.postcard"),
        "saves/deep_binary.postcard"
    );
}

#[test]
fn test_multi_save_listing_scenario_suggestions_and_safe_delete() {
    use protostellar::simulation::scenarios::ScenarioPreset;

    let dir = std::path::Path::new("target/test_multi_save_slots_unit");
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).expect("create test directory");

    // Write arbitrary non-save JSON and verify it is ignored
    let rogue_json = dir.join("not_a_save.json");
    std::fs::write(&rogue_json, "{\"hello\": \"world\"}").expect("write rogue json");

    let trappist_path = dir.join("trappist1_01.json");
    let mmsn_path = dir.join("solar_nebula_01.postcard");
    save_system_to_file(
        &sample_io_save_data(Some(ScenarioPreset::Trappist1System), 42.5, 1200),
        &trappist_path.to_string_lossy(),
    )
    .expect("save trappist");
    save_system_to_file(
        &sample_io_save_data(Some(ScenarioPreset::SolarNebulaMmsn), 1500.0, 9999),
        &mmsn_path.to_string_lossy(),
    )
    .expect("save solar nebula");

    let entries = list_save_slot_entries_in_dir(dir);
    assert_eq!(entries.len(), 2, "Only valid save files should be listed");
    assert!(entries.iter().any(|e| {
        e.slot_name == "trappist1_01"
            && e.scenario_preset == Some(ScenarioPreset::Trappist1System)
            && e.format_tag == "JSON"
    }));
    assert!(entries.iter().any(|e| {
        e.slot_name == "solar_nebula_01"
            && e.scenario_preset == Some(ScenarioPreset::SolarNebulaMmsn)
            && e.format_tag == "BIN"
    }));

    // Next suggested slot increments past existing trappist1_01
    let next_trappist = suggest_next_slot_name_in_dir(dir, Some(ScenarioPreset::Trappist1System));
    assert_eq!(next_trappist, "trappist1_02");
    let next_kepler =
        suggest_next_slot_name_in_dir(dir, Some(ScenarioPreset::Kepler16Circumbinary));
    assert_eq!(next_kepler, "kepler16_01");

    // Refuse parent-traversal and non-save JSON deletion
    assert!(delete_save_slot_file("../Cargo.toml").is_err());
    assert!(delete_save_slot_file(&rogue_json.to_string_lossy()).is_err());
    assert!(rogue_json.exists(), "Non-save JSON must not be deleted");

    // Deleting a valid save removes only that file
    delete_save_slot_file(&trappist_path.to_string_lossy()).expect("delete valid save");
    assert!(!trappist_path.exists());
    assert!(mmsn_path.exists());

    let _ = std::fs::remove_dir_all(dir);
}
