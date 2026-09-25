//! Integration tests for multi-epoch planetary telemetry recording, sparklines, and CSV export.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::fs;
use std::path::Path;

use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::telemetry::{
    record_planetary_telemetry, PlanetaryTelemetrySample, SimulationTelemetryHistory,
    TelemetryMetric,
};
use protostellar::utils::constants::EARTH_MASS_SOLAR;

#[test]
fn test_telemetry_ring_buffer_capacity_and_eviction() {
    let mut history = SimulationTelemetryHistory {
        max_samples: 5,
        ..default()
    };

    assert_eq!(history.sample_count(), 0);

    for i in 1..=10 {
        history.push_sample(PlanetaryTelemetrySample {
            sim_time_yr: i as f64 * 100.0,
            body_name: "Test-World".to_string(),
            mass_earth: 1.0 + (i as f64 * 0.1),
            radius_km: 6371.0,
            semi_major_axis_au: 1.0,
            eccentricity: 0.01,
            surface_temp_k: 280.0 + i as f64,
            atmospheric_pressure_bar: 1.0,
            ocean_coverage_frac: 0.7,
            habitability_score: 0.8,
            biomass_coverage_frac: 0.5,
        });
    }

    // Must cap strictly at max_samples = 5
    assert_eq!(history.sample_count(), 5);

    // Oldest surviving sample must be i = 6 (time = 600.0)
    assert_eq!(history.samples.front().unwrap().sim_time_yr, 600.0);
    // Newest sample must be i = 10 (time = 1000.0)
    assert_eq!(history.samples.back().unwrap().sim_time_yr, 1000.0);
}

#[test]
fn test_telemetry_metric_statistics() {
    let mut history = SimulationTelemetryHistory::default();

    // Empty buffer stats
    let (latest, min, max, mean) = history.get_metric_stats(TelemetryMetric::Temperature);
    assert!((latest - 0.0).abs() < 1e-6);
    assert!((min - 0.0).abs() < 1e-6);
    assert!((max - 0.0).abs() < 1e-6);
    assert!((mean - 0.0).abs() < 1e-6);

    // Insert 3 samples with temps: 250.0, 300.0, 350.0
    for &t in &[250.0, 300.0, 350.0] {
        history.push_sample(PlanetaryTelemetrySample {
            sim_time_yr: 10.0,
            body_name: "Earth".to_string(),
            mass_earth: 1.0,
            radius_km: 6371.0,
            semi_major_axis_au: 1.0,
            eccentricity: 0.0167,
            surface_temp_k: t,
            atmospheric_pressure_bar: 1.013,
            ocean_coverage_frac: 0.71,
            habitability_score: 0.90,
            biomass_coverage_frac: 0.50,
        });
    }

    let (latest, min, max, mean) = history.get_metric_stats(TelemetryMetric::Temperature);
    assert!((latest - 350.0).abs() < 1e-6);
    assert!((min - 250.0).abs() < 1e-6);
    assert!((max - 350.0).abs() < 1e-6);
    assert!((mean - 300.0).abs() < 1e-6);

    let (hab_latest, hab_min, hab_max, hab_mean) =
        history.get_metric_stats(TelemetryMetric::Habitability);
    assert!((hab_latest - 90.0).abs() < 1e-6);
    assert!((hab_min - 90.0).abs() < 1e-6);
    assert!((hab_max - 90.0).abs() < 1e-6);
    assert!((hab_mean - 90.0).abs() < 1e-6);
}

#[test]
fn test_telemetry_sparkline_generation() {
    let mut history = SimulationTelemetryHistory::default();

    // Empty history
    let empty_spark = history.generate_sparkline(TelemetryMetric::Habitability, 10);
    assert!(empty_spark.contains("No historical telemetry"));

    // Push progressive habitability from 0% to 100%
    for step in 0..=7 {
        history.push_sample(PlanetaryTelemetrySample {
            sim_time_yr: f64::from(step),
            body_name: "Terra".to_string(),
            mass_earth: 1.0,
            radius_km: 6371.0,
            semi_major_axis_au: 1.0,
            eccentricity: 0.0,
            surface_temp_k: 288.0,
            atmospheric_pressure_bar: 1.0,
            ocean_coverage_frac: 0.7,
            habitability_score: step as f32 / 7.0,
            biomass_coverage_frac: 0.5,
        });
    }

    let spark = history.generate_sparkline(TelemetryMetric::Habitability, 8);
    assert_eq!(spark.chars().count(), 8);
    // First char should be lowest block ' ' and last should be full block '█'
    assert_eq!(spark.chars().next().unwrap(), ' ');
    assert_eq!(spark.chars().last().unwrap(), '█');
}

#[test]
fn test_telemetry_csv_serialization_and_file_export() {
    let mut history = SimulationTelemetryHistory::default();
    history.push_sample(PlanetaryTelemetrySample {
        sim_time_yr: 12500.5,
        body_name: "Eden-Prime".to_string(),
        mass_earth: 1.025,
        radius_km: 6420.0,
        semi_major_axis_au: 1.015,
        eccentricity: 0.012,
        surface_temp_k: 289.4,
        atmospheric_pressure_bar: 1.02,
        ocean_coverage_frac: 0.725,
        habitability_score: 0.942,
        biomass_coverage_frac: 0.681,
    });

    let csv_text = history.serialize_to_csv();
    assert!(csv_text.starts_with("Epoch_yr,BodyName,Mass_Mearth,Radius_km,SemiMajorAxis_AU"));
    assert!(csv_text.contains(
        "12500.50,Eden-Prime,1.025000,6420.0,1.0150,0.0120,289.40,1.0200,72.50,94.20,68.10"
    ));

    // Test file export to custom test file
    let test_dir = Path::new("target/test_exports");
    if !test_dir.exists() {
        let _ = fs::create_dir_all(test_dir);
    }
    let test_file = test_dir.join("test_telemetry_export.csv");

    let res = history.export_to_csv_file(Some(&test_file));
    assert!(res.is_ok(), "Export must succeed");

    let read_back = fs::read_to_string(&test_file).expect("File must exist");
    assert_eq!(read_back, csv_text);

    // Clean up test file
    let _ = fs::remove_file(test_file);
}

#[test]
fn test_telemetry_ecs_recording_system() {
    let mut app = App::new();

    app.insert_resource(SimTime {
        elapsed_years: 500.0,
        current_dt_yr: 1.0,
        step_count: 500,
        ..Default::default()
    });
    app.insert_resource(TimeWarp {
        multiplier: 1.0,
        is_paused: false,
        step_once: false,
    });
    app.insert_resource(PlayerInteractionState::default());
    app.insert_resource(DiskParameters::default());
    app.insert_resource(SimulationTelemetryHistory {
        sample_interval_yr: 10.0,
        ..default()
    });

    // Spawn central star
    app.world_mut().spawn((
        CentralStar,
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
    ));

    // Spawn target planet
    let planet_entity = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Proto-Earth".to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(4.258e-5), // ~1 Earth radius in AU
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)), // ~circular orbit velocity in AU/yr
            Temperature(288.15),
            VolatileInventory {
                atmospheric_pressure_bar: 1.013,
                ocean_coverage_frac: 0.71,
                ..default()
            },
            BiosphereState {
                habitability_score: 0.92,
                biomass_coverage_frac: 0.55,
                ..default()
            },
        ))
        .id();

    // Select this planet
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(planet_entity);

    // Run recording system
    app.add_systems(Update, record_planetary_telemetry);
    app.update();

    let telemetry = app.world().resource::<SimulationTelemetryHistory>();
    assert_eq!(telemetry.sample_count(), 1);
    let sample = telemetry.samples.front().unwrap();
    assert_eq!(sample.body_name, "Proto-Earth");
    assert!((sample.mass_earth - 1.0).abs() < 1e-3);
    assert!((sample.surface_temp_k - 288.15).abs() < 1e-2);
    assert!((sample.atmospheric_pressure_bar - 1.013).abs() < 1e-3);
    assert!((sample.ocean_coverage_frac - 0.71).abs() < 1e-3);
    assert!((sample.habitability_score - 0.92).abs() < 1e-3);
    assert!((sample.semi_major_axis_au - 1.0).abs() < 0.1);
}
