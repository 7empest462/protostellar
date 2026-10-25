//! Integration tests for Dynamic Cosmic Hazards, Deep-Space Catastrophes, and Tonemapping.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::game::ui::NotificationToast;
use protostellar::rendering::post_processing::PostProcessingSettings;
use protostellar::simulation::components::*;
use protostellar::simulation::economy::ScienceData;
use protostellar::simulation::events::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::space_weather::types::{AuroralOvalState, GeomagneticStormLevel};

fn setup_hazard_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<CosmicEventManager>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<ScienceData>();
    app.add_message::<TriggerCosmicEvent>();
    app.add_message::<CosmicEventStartedEvent>();
    app.add_message::<CosmicEventResolvedEvent>();
    app
}

#[test]
fn test_cosmic_hazard_types_and_manager_queue() {
    let mut app = setup_hazard_test_app();
    let mut mgr = app.world_mut().resource_mut::<CosmicEventManager>();

    assert!(!mgr.has_active_hazard(CosmicEventType::GammaRayBurst));
    mgr.trigger_hazard(CosmicEventType::GammaRayBurst, 1.0);
    assert!(mgr.has_active_hazard(CosmicEventType::GammaRayBurst));

    // Duplicate trigger ignored
    mgr.trigger_hazard(CosmicEventType::GammaRayBurst, 1.0);
    assert_eq!(mgr.active_events.len(), 1);

    // Verify metadata
    let grb = CosmicEventType::GammaRayBurst;
    assert_eq!(grb.title(), "Gamma-Ray Burst (GRB)");
    assert!(grb.science_bounty() >= 500.0);
    assert!(grb.duration_years() > 0.0);

    let rogue = CosmicEventType::RogueStarIncursion;
    assert_eq!(rogue.title(), "Rogue Star Incursion");
    assert!(rogue.science_bounty() >= 400.0);
}

#[test]
fn test_gamma_ray_burst_strips_unshielded_atmosphere_and_preserves_shielded() {
    let mut app = setup_hazard_test_app();
    app.add_systems(Update, cosmic_event_lifecycle_system);

    // Unshielded planet (no magnetic dynamo)
    let unshielded = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Unshielded World".to_string(),
            },
            VolatileInventory {
                delivered_water_m_earth: 1.0,
                ocean_coverage_frac: 0.7,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 5,
            },
            PlanetaryClimate::default(),
            BiosphereState {
                technosignature: 1.5,
                ..default()
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            Radius(0.00004),
        ))
        .id();

    // Shielded planet with active magnetic dipole field
    let shielded = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Shielded Dynamo World".to_string(),
            },
            VolatileInventory {
                delivered_water_m_earth: 1.0,
                ocean_coverage_frac: 0.7,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 5,
            },
            PlanetaryClimate::default(),
            ElectromagneticFieldState {
                magnetic_field_gauss: 0.65, // Earth-like dipole shield
                ..default()
            },
            SimPosition(DVec3::new(1.5, 0.0, 0.0)),
            Radius(0.00004),
        ))
        .id();

    // Trigger Gamma-Ray Burst
    {
        let mut mgr = app.world_mut().resource_mut::<CosmicEventManager>();
        mgr.trigger_hazard(CosmicEventType::GammaRayBurst, 1.0);
    }

    // Step 1: Initialize
    app.update();

    // Step 2: Apply physical effects with 0.5 sim years
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 0.5;
    }
    app.update();

    // Verify unshielded lost atmosphere and biosphere
    let unshielded_vol = app.world().get::<VolatileInventory>(unshielded).unwrap();
    assert!(
        unshielded_vol.atmospheric_pressure_bar < 1.0,
        "Unshielded planet atmosphere must be stripped by GRB: got {}",
        unshielded_vol.atmospheric_pressure_bar
    );

    let unshielded_bio = app.world().get::<BiosphereState>(unshielded).unwrap();
    assert!(
        unshielded_bio.technosignature < 1.5,
        "Biosphere must suffer damage from unshielded ionizing radiation"
    );

    // Verify shielded remained protected
    let shielded_vol = app.world().get::<VolatileInventory>(shielded).unwrap();
    assert_eq!(
        shielded_vol.atmospheric_pressure_bar, 1.0,
        "Shielded planet with active dynamo must deflect GRB atmospheric stripping"
    );
}

#[test]
fn test_solar_superflare_drives_extreme_auroras_and_probe_disruption() {
    let mut app = setup_hazard_test_app();
    app.add_systems(Update, cosmic_event_lifecycle_system);

    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Auroral Earth".to_string(),
            },
            AuroralOvalState::default(),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            Radius(0.00004),
        ))
        .id();

    let probe = app
        .world_mut()
        .spawn((
            protostellar::simulation::probes::FlightComputer {
                target_entity: planet,
                target_name: "Auroral Earth".to_string(),
                launch_pos: DVec3::ZERO,
                distance_to_target: 0.1,
                relative_speed: 12.0,
            },
            SimPosition(DVec3::new(0.9, 0.0, 0.0)),
        ))
        .id();

    // Trigger Superflare
    {
        let mut mgr = app.world_mut().resource_mut::<CosmicEventManager>();
        mgr.trigger_hazard(CosmicEventType::SolarSuperflare, 1.0);
    }

    app.update(); // Initialize
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 0.1;
    }
    app.update(); // Apply effects

    let oval = app.world().get::<AuroralOvalState>(planet).unwrap();
    assert_eq!(oval.geomagnetic_kp_index, 9.0);
    assert_eq!(oval.storm_level, GeomagneticStormLevel::ExtremeStorm);
    assert!(oval.auroral_intensity > 2.0);

    let flight_comp = app
        .world()
        .get::<protostellar::simulation::probes::FlightComputer>(probe)
        .unwrap();
    assert!(flight_comp.relative_speed < 12.0);
}

#[test]
fn test_rogue_star_incursion_spawns_and_resolves_when_receding() {
    let mut app = setup_hazard_test_app();
    app.add_systems(Update, cosmic_event_lifecycle_system);

    {
        let mut mgr = app.world_mut().resource_mut::<CosmicEventManager>();
        mgr.trigger_hazard(CosmicEventType::RogueStarIncursion, 1.0);
    }

    app.update(); // Initialize and spawn

    // Verify rogue star entity spawned
    let mut query = app
        .world_mut()
        .query::<(&CelestialBody, &RogueInterloper, &SimPosition)>();
    let interlopers: Vec<_> = query.iter(app.world()).collect();
    assert_eq!(interlopers.len(), 1);
    assert_eq!(interlopers[0].0.body_type, BodyType::BrownDwarf);

    // Fast-forward beyond duration
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 50.0;
    }
    app.update();

    // Verify event concluded and entity despawned
    let mgr = app.world().resource::<CosmicEventManager>();
    assert_eq!(mgr.active_events.len(), 0);
    assert_eq!(mgr.total_hazards_survived, 1);
}

#[test]
fn test_cometary_outburst_delivers_water_to_terrestrial_planet() {
    let mut app = setup_hazard_test_app();
    app.add_systems(Update, cosmic_event_lifecycle_system);

    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Dry Rock".to_string(),
            },
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.1,
                cometary_impact_count: 0,
            },
            Radius(0.00004),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        ))
        .id();

    // Spawn hazard comet passing right by the planet
    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::Comet,
            name: "Test Close Comet".to_string(),
        },
        VolatileInventory {
            delivered_water_m_earth: 0.05,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: 0.01,
            cometary_impact_count: 0,
        },
        SimPosition(DVec3::new(1.02, 0.0, 0.0)), // Within 0.02 AU encounter threshold
        HazardComet,
    ));

    {
        let mut mgr = app.world_mut().resource_mut::<CosmicEventManager>();
        mgr.trigger_hazard(CosmicEventType::CometOutburstSwarm, 1.0);
    }
    app.update();

    let vol = app.world().get::<VolatileInventory>(planet).unwrap();
    assert!(vol.delivered_water_m_earth > 0.0);
    assert_eq!(vol.cometary_impact_count, 1);
}

#[test]
fn test_tonemapping_modes_and_sensory_flash_decay() {
    let mut app = setup_hazard_test_app();
    app.add_systems(Update, cosmic_event_sensory_system);

    let camera = app
        .world_mut()
        .spawn(PostProcessingSettings::default())
        .id();

    {
        let mut mgr = app.world_mut().resource_mut::<CosmicEventManager>();
        mgr.hazard_flash_intensity = 1.0;
        assert_eq!(mgr.tonemapping_mode.label(), "AgX Filmic");
        mgr.tonemapping_mode = mgr.tonemapping_mode.cycle();
        assert_eq!(mgr.tonemapping_mode.label(), "ACES Filmic (Narkowicz)");
        assert_eq!(mgr.tonemapping_mode.shader_mode_id(), 1);
        mgr.tonemapping_mode = mgr.tonemapping_mode.cycle();
        assert_eq!(mgr.tonemapping_mode.label(), "Reinhard HDR");
        assert_eq!(mgr.tonemapping_mode.shader_mode_id(), 2);
    }

    app.update();

    let settings = app.world().get::<PostProcessingSettings>(camera).unwrap();
    assert_eq!(settings.tonemapping_mode, 2);
    assert!(settings.hazard_flash_intensity <= 1.0);
}

#[test]
fn test_hazard_resolution_records_discovery_and_science_bounty() {
    let mut app = setup_hazard_test_app();
    app.add_systems(Update, cosmic_event_lifecycle_system);

    let initial_points = app.world().resource::<ScienceData>().points;

    {
        let mut mgr = app.world_mut().resource_mut::<CosmicEventManager>();
        mgr.trigger_hazard(CosmicEventType::GammaRayBurst, 1.0);
    }

    app.update(); // Initialize

    // Advance time past GRB duration (1.5 years)
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 2.0;
    }
    app.update(); // Resolve

    let science = app.world().resource::<ScienceData>();
    let grb_bounty = CosmicEventType::GammaRayBurst.science_bounty();
    assert_eq!(science.points, initial_points + grb_bounty);
    assert!(science
        .discoveries
        .iter()
        .any(|d| d.title.contains("Gamma-Ray Burst")));
}
