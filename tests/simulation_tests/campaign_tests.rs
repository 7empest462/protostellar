//! Integration tests for Campaign Mode, Scenario Challenges, and Objectives Evaluation.

use bevy::prelude::*;
use protostellar::simulation::campaign::*;
use protostellar::simulation::components::celestial::*;
use protostellar::simulation::components::planetary::*;
use protostellar::simulation::economy::ScienceData;
use protostellar::simulation::probes::SpaceProbe;
use protostellar::simulation::resources::*;

#[test]
fn test_campaign_mission_definitions_and_objectives() {
    let missions = [
        CampaignMissionId::TheStellarForge,
        CampaignMissionId::GenesisWorld,
        CampaignMissionId::TheGrandTour,
        CampaignMissionId::Exodus,
    ];

    for m in missions {
        assert!(!m.title().is_empty());
        assert!(!m.subtitle().is_empty());
        assert!(!m.briefing().is_empty());
        assert!(!m.difficulty().is_empty());
        assert!(m.science_reward() > 0);
        assert!(m.reward_science() > 0.0);

        let objectives = create_mission_objectives(m);
        assert!(
            objectives.len() >= 2,
            "Mission {:?} should have at least 2 objectives",
            m
        );
        for obj in &objectives {
            assert!(!obj.id.is_empty());
            assert!(!obj.title.is_empty());
            assert!(!obj.description.is_empty());
            assert!(obj.target_value > 0.0);
            assert_eq!(obj.progress_fraction(), 0.0);
            assert!(!obj.format_progress().is_empty());
        }
    }
}

#[test]
fn test_stellar_forge_mission_spawning_and_entities() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let mut config = SimulationConfig::default();

    let star_entity = spawn_campaign_mission(
        CampaignMissionId::TheStellarForge,
        &mut app.world_mut().commands(),
        &mut disk_params,
        &mut config,
    );
    app.update();

    assert!(star_entity.is_some());

    // Verify central star
    let (body_type, mass_val, is_ignited) = {
        let mut query = app
            .world_mut()
            .query::<(&CelestialBody, &Mass, &IgnitionState)>();
        let (b, m, i) = query
            .iter(app.world())
            .find(|(b, _, _)| b.name.contains("Protostar Forge Alpha"))
            .expect("Protostar Forge Alpha must exist");
        (b.body_type, m.0, i.is_ignited)
    };

    assert_eq!(body_type, BodyType::Protostar);
    assert_eq!(mass_val, 0.050);
    assert!(!is_ignited);

    // Verify 2 rocky embryos
    let embryo_count = {
        let mut query = app.world_mut().query::<(&CelestialBody, &SimPosition)>();
        query
            .iter(app.world())
            .filter(|(b, _)| b.name.contains("Forge Rocky Embryo"))
            .count()
    };
    assert_eq!(embryo_count, 2);

    // Verify accretion clumps
    let clump_count = {
        let mut query = app.world_mut().query::<(&CelestialBody, &SimPosition)>();
        query
            .iter(app.world())
            .filter(|(b, _)| b.name.contains("Accretion Clump"))
            .count()
    };
    assert_eq!(clump_count, 4);
}

#[test]
fn test_genesis_world_mission_spawning() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let mut config = SimulationConfig::default();

    let star_entity = spawn_campaign_mission(
        CampaignMissionId::GenesisWorld,
        &mut app.world_mut().commands(),
        &mut disk_params,
        &mut config,
    );
    app.update();

    assert!(star_entity.is_some());

    // Verify Candidate Genesis exists with initial snowball conditions
    let (b_type, temp_k, regime, ocean_frac) = {
        let mut query = app
            .world_mut()
            .query::<(&CelestialBody, &PlanetaryClimate, &VolatileInventory)>();
        let (b, c, v) = query
            .iter(app.world())
            .find(|(b, _, _)| b.name.contains("Candidate Genesis"))
            .expect("Candidate Genesis world must exist");
        (
            b.body_type,
            c.surface_temperature_k,
            c.climate_regime,
            v.ocean_coverage_frac,
        )
    };

    assert_eq!(b_type, BodyType::TerrestrialPlanet);
    assert_eq!(temp_k, 160.0);
    assert_eq!(regime, ClimateRegime::SnowballIceAge);
    assert!(ocean_frac < 0.10);

    // Verify comets spawned
    let comets_count = {
        let mut query = app.world_mut().query::<(&CelestialBody, &Composition)>();
        query
            .iter(app.world())
            .filter(|(b, _)| b.name.contains("Icy Comet"))
            .count()
    };
    assert_eq!(comets_count, 4);
}

#[test]
fn test_grand_tour_and_exodus_spawning() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let mut config = SimulationConfig::default();

    // Grand Tour
    spawn_campaign_mission(
        CampaignMissionId::TheGrandTour,
        &mut app.world_mut().commands(),
        &mut disk_params,
        &mut config,
    );
    app.update();

    let planet_count = {
        let mut query = app.world_mut().query::<(&CelestialBody, &SimPosition)>();
        query
            .iter(app.world())
            .filter(|(b, _)| b.name.starts_with("Planet "))
            .count()
    };
    assert_eq!(planet_count, 4);

    let probe_exists = {
        let mut query = app.world_mut().query::<(&CelestialBody, &SpaceProbe)>();
        query
            .iter(app.world())
            .any(|(b, _)| b.name.contains("Voyager III"))
    };
    assert!(probe_exists);

    // Exodus
    let mut app2 = App::new();
    spawn_campaign_mission(
        CampaignMissionId::Exodus,
        &mut app2.world_mut().commands(),
        &mut disk_params,
        &mut config,
    );
    app2.update();

    let ark_ships = {
        let mut query = app2.world_mut().query::<(&CelestialBody, &SpaceProbe)>();
        query
            .iter(app2.world())
            .filter(|(b, _)| b.name.contains("Ark Colony Ship"))
            .count()
    };
    assert_eq!(ark_ships, 2);
}

#[test]
fn test_stellar_forge_evaluation_and_victory() {
    let mut app = App::new();
    app.add_plugins(CampaignPlugin);
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<ScienceData>();

    // Launch mission via event
    app.world_mut().write_message(StartCampaignMissionEvent(
        CampaignMissionId::TheStellarForge,
    ));
    app.update();

    // Verify mission started
    {
        let state = app.world().resource::<CampaignState>();
        assert_eq!(
            state.active_mission,
            Some(CampaignMissionId::TheStellarForge)
        );
        assert_eq!(state.status, MissionStatus::InProgress);
    }

    // Step physics & campaign: currently unignited and mass = 0.050
    app.update();
    {
        let state = app.world().resource::<CampaignState>();
        assert_eq!(state.status, MissionStatus::InProgress);
        assert!(!state.objectives[0].is_completed); // mass < 0.080
        assert!(!state.objectives[1].is_completed); // not ignited
    }

    // Accrete mass and ignite the central star
    let star_entity = app
        .world()
        .resource::<PlayerInteractionState>()
        .selected_entity
        .expect("Central star should be selected");

    {
        let mut entity_mut = app.world_mut().entity_mut(star_entity);
        entity_mut.get_mut::<Mass>().unwrap().0 = 0.088;
        entity_mut.get_mut::<IgnitionState>().unwrap().is_ignited = true;
    }

    // Evaluate
    app.update();

    let state = app.world().resource::<CampaignState>();
    assert!(
        state.objectives[0].is_completed,
        "Mass objective should be complete"
    );
    assert!(
        state.objectives[1].is_completed,
        "Ignition objective should be complete"
    );
    assert!(
        state.objectives[2].is_completed,
        "Rocky embryos should be preserved"
    );
    assert_eq!(state.status, MissionStatus::Victory);
}

#[test]
fn test_stellar_forge_failure_on_embryo_ejection() {
    let mut app = App::new();
    app.add_plugins(CampaignPlugin);
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<ScienceData>();

    app.world_mut().write_message(StartCampaignMissionEvent(
        CampaignMissionId::TheStellarForge,
    ));
    app.update();

    // Find and eject an embryo past 25 AU
    let embryo_entity = {
        let mut query = app.world_mut().query::<(Entity, &CelestialBody)>();
        query
            .iter(app.world())
            .find(|(_, b)| b.name.contains("Forge Rocky Embryo I"))
            .map(|(e, _)| e)
            .expect("Embryo I must exist")
    };

    app.world_mut()
        .entity_mut(embryo_entity)
        .get_mut::<SimPosition>()
        .unwrap()
        .0 = bevy::math::DVec3::new(50.0, 0.0, 0.0);

    app.update();

    let state = app.world().resource::<CampaignState>();
    assert_eq!(state.status, MissionStatus::Defeat);
    assert!(state.objectives[2].is_failed);
    assert!(state.failure_reason.is_some());
}

#[test]
fn test_genesis_world_evaluation_and_sustained_biosphere() {
    let mut app = App::new();
    app.add_plugins(CampaignPlugin);
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<ScienceData>();

    app.world_mut()
        .write_message(StartCampaignMissionEvent(CampaignMissionId::GenesisWorld));
    app.update();

    let genesis_entity = {
        let mut query = app.world_mut().query::<(Entity, &CelestialBody)>();
        query
            .iter(app.world())
            .find(|(_, b)| b.name.contains("Candidate Genesis"))
            .map(|(e, _)| e)
            .expect("Candidate Genesis must exist")
    };

    // Warm climate into habitable range (295 K) and deliver water oceans (35%)
    {
        let mut entity_mut = app.world_mut().entity_mut(genesis_entity);
        let mut climate = entity_mut.get_mut::<PlanetaryClimate>().unwrap();
        climate.surface_temperature_k = 295.0;
        climate.ice_coverage_frac = 0.20;

        let mut volatiles = entity_mut.get_mut::<VolatileInventory>().unwrap();
        volatiles.ocean_coverage_frac = 0.40;
    }

    // Step time forward past 50 Myr (dt = 55 Myr)
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 5.5e7;
    }

    app.update();

    let state = app.world().resource::<CampaignState>();
    assert!(
        state.objectives[0].is_completed,
        "Temperature should be complete"
    );
    assert!(
        state.objectives[1].is_completed,
        "Oceans should be complete"
    );
    assert!(
        state.objectives[2].is_completed,
        "Biosphere duration should be complete"
    );
    assert_eq!(state.status, MissionStatus::Victory);
}

#[test]
fn test_grand_tour_flyby_accumulation_and_victory() {
    let mut app = App::new();
    app.add_plugins(CampaignPlugin);
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<ScienceData>();

    app.world_mut()
        .write_message(StartCampaignMissionEvent(CampaignMissionId::TheGrandTour));
    app.update();

    // Get Voyager probe and the 4 planets
    let voyager_entity = {
        let mut query = app
            .world_mut()
            .query::<(Entity, &CelestialBody, &SpaceProbe)>();
        query
            .iter(app.world())
            .find(|(_, b, _)| b.name.contains("Voyager"))
            .map(|(e, _, _)| e)
            .expect("Voyager must exist")
    };

    let planet_positions: Vec<bevy::math::DVec3> = {
        let mut query = app.world_mut().query::<(&CelestialBody, &SimPosition)>();
        query
            .iter(app.world())
            .filter(|(b, _)| b.name.starts_with("Planet "))
            .map(|(_, p)| p.0)
            .collect()
    };
    assert_eq!(planet_positions.len(), 4);

    // Simulate Voyager flying past each planet one by one
    for pos in planet_positions {
        app.world_mut()
            .entity_mut(voyager_entity)
            .get_mut::<SimPosition>()
            .unwrap()
            .0 = pos + bevy::math::DVec3::new(0.10, 0.0, 0.0); // Within 0.50 AU flyby window
        app.update();
    }

    let state = app.world().resource::<CampaignState>();
    assert_eq!(state.flyby_visited_entities.len(), 4);
    assert!(
        state.objectives[0].is_completed,
        "Flybys objective should be complete"
    );
    assert!(
        state.objectives[1].is_completed,
        "Probe survival should be complete"
    );
    assert_eq!(state.status, MissionStatus::Victory);
}

#[test]
fn test_exodus_evacuation_and_supernova_collapse() {
    let mut app = App::new();
    app.add_plugins(CampaignPlugin);
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<ScienceData>();

    app.world_mut()
        .write_message(StartCampaignMissionEvent(CampaignMissionId::Exodus));
    app.update();

    let ark_ships: Vec<Entity> = {
        let mut query = app
            .world_mut()
            .query::<(Entity, &CelestialBody, &SpaceProbe)>();
        query
            .iter(app.world())
            .filter(|(_, b, _)| b.name.contains("Ark Colony Ship"))
            .map(|(e, _, _)| e)
            .collect()
    };
    assert_eq!(ark_ships.len(), 2);

    // Move ships beyond 200 AU
    for ship in ark_ships {
        app.world_mut()
            .entity_mut(ship)
            .get_mut::<SimPosition>()
            .unwrap()
            .0 = bevy::math::DVec3::new(220.0, 0.0, 0.0);
    }

    app.update();

    let state = app.world().resource::<CampaignState>();
    assert_eq!(state.evacuated_probe_count, 2);
    assert!(
        state.objectives[0].is_completed,
        "Evacuation should be complete"
    );
    assert!(
        state.objectives[1].is_completed,
        "Outrun supernova should be complete"
    );
    assert_eq!(state.status, MissionStatus::Victory);
}
