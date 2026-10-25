//! Unit and integration tests for the Science Economy and Technology Tree.

use bevy::prelude::*;
use protostellar::rendering::effects::megastructures::DysonSwarm;
use protostellar::simulation::components::celestial::SimPosition;
use protostellar::simulation::components::planetary::BiosphereState;
use protostellar::simulation::economy::*;
use protostellar::simulation::probes::{ProbeStatus, ProbeType, SpaceProbe};
use protostellar::simulation::resources::{SimTime, TimeWarp};

#[test]
fn test_science_data_points_and_spending() {
    let mut science = ScienceData::default();
    assert_eq!(science.points, 100.0);
    assert_eq!(science.lifetime_points, 100.0);

    science.add_science(50.0);
    assert_eq!(science.points, 150.0);
    assert_eq!(science.lifetime_points, 150.0);

    // Successful purchase
    assert!(science.spend_science(120.0));
    assert_eq!(science.points, 30.0);
    assert_eq!(science.lifetime_points, 150.0); // Lifetime does not decrease

    // Insufficient funds
    assert!(!science.spend_science(50.0));
    assert_eq!(science.points, 30.0);
}

#[test]
fn test_tech_tree_prerequisites_and_unlocking() {
    let mut science = ScienceData {
        points: 2000.0,
        lifetime_points: 2000.0,
        passive_rate_per_sec: 0.0,
        discoveries: Vec::new(),
    };
    let mut techs = UnlockedTechs::default();

    // Verify all 9 nodes exist and are well-formed
    let nodes = UnlockedTechs::all_nodes();
    assert_eq!(nodes.len(), 9);
    for node in nodes {
        assert!(!node.name.is_empty());
        assert!(!node.description.is_empty());
        assert!(node.cost > 0.0);
    }

    // IonDrive has no prereqs; SolarSail requires IonDrive
    assert!(techs.prerequisites_met(TechnologyId::IonDrive));
    assert!(!techs.prerequisites_met(TechnologyId::SolarSail));
    assert!(!techs.can_unlock(TechnologyId::SolarSail, &science));

    // Unlock IonDrive
    assert!(techs.unlock(TechnologyId::IonDrive, &mut science));
    assert!(techs.has(TechnologyId::IonDrive));
    assert_eq!(science.points, 1850.0); // 2000 - 150

    // Now SolarSail prereqs are met
    assert!(techs.prerequisites_met(TechnologyId::SolarSail));
    assert!(techs.can_unlock(TechnologyId::SolarSail, &science));

    // Unlock SolarSail
    assert!(techs.unlock(TechnologyId::SolarSail, &mut science));
    assert!(techs.has(TechnologyId::SolarSail));
    assert_eq!(science.points, 1500.0); // 1850 - 350

    // AntimatterDrive now available
    assert!(techs.prerequisites_met(TechnologyId::AntimatterDrive));
    assert!(techs.unlock(TechnologyId::AntimatterDrive, &mut science));
    assert!(techs.has(TechnologyId::AntimatterDrive));
    assert_eq!(science.points, 500.0); // 1500 - 1000
}

#[test]
fn test_discovery_ledger_recording() {
    let mut science = ScienceData::default();
    assert_eq!(science.discoveries.len(), 0);

    science.record_discovery(
        "⭐ Stellar Core Ignition",
        "Protostellar core initiated hydrogen fusion",
        100.0,
        0.05,
    );

    assert_eq!(science.discoveries.len(), 1);
    assert_eq!(science.discoveries[0].title, "⭐ Stellar Core Ignition");
    assert_eq!(science.points, 200.0); // 100 default + 100 bonus
    assert_eq!(science.lifetime_points, 200.0);
}

#[test]
fn test_probe_and_biosphere_telemetry_harvesting() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.add_plugins(EconomyPlugin);

    // Spawn an active orbiter probe
    let mut probe = SpaceProbe::new(ProbeType::Orbiter, 1);
    probe.status = ProbeStatus::InOrbit;
    probe.samples_collected = 3;
    app.world_mut()
        .spawn((probe, SimPosition(bevy::math::DVec3::ZERO)));

    // Spawn an exoplanet civilization
    app.world_mut().spawn((
        BiosphereState {
            technosignature: 1.5,
            ..default()
        },
        SimPosition(bevy::math::DVec3::ZERO),
    ));

    // Update frame
    app.update();

    let science = app.world().resource::<ScienceData>();
    // Probe discrete samples (3 * 25 = 75 pts) + passive telemetry rate > 0
    assert!(
        science.points >= 175.0,
        "Expected at least 175 science points, got {}",
        science.points
    );
    assert!(
        science.passive_rate_per_sec > 0.0,
        "Expected positive passive rate"
    );
}

#[test]
fn test_dyson_swarm_amplification() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.add_plugins(EconomyPlugin);

    // Spawn a half-completed Dyson swarm
    app.world_mut().spawn((
        DysonSwarm {
            completion_fraction: 0.5,
            mirror_count: 50_000,
        },
        SimPosition(bevy::math::DVec3::ZERO),
    ));

    app.update();
    let initial_rate = app.world().resource::<ScienceData>().passive_rate_per_sec;
    assert!(initial_rate > 0.0);

    // Unlock DysonCollectors tech
    app.world_mut()
        .resource_mut::<UnlockedTechs>()
        .unlocked
        .insert(TechnologyId::DysonCollectors);

    app.update();
    let boosted_rate = app.world().resource::<ScienceData>().passive_rate_per_sec;
    assert!(
        boosted_rate >= initial_rate * 2.0,
        "Expected boosted rate, got initial: {}, boosted: {}",
        initial_rate,
        boosted_rate
    );
}
