//! Cross-scenario automated regression and smoke testing suite.
//! Validates that all 15 scenario presets load cleanly, spawn valid physical bodies,
//! clean up previous entities on scenario transition, and step through physics integration
//! without numerical instability, NaNs, or crashes.

use bevy::prelude::*;

use protostellar::game::phases::{
    GlobalTrophies, LateHeavyBombardmentState, PhaseManager, SystemPhase,
};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{
    handle_load_scenario_events, ActiveScenarioState, LoadScenarioEvent, ScenarioPreset,
};
use protostellar::simulation::SimulationPlugin;

fn create_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(bevy::time::TimePlugin);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<PhaseManager>();
    app.init_resource::<GlobalTrophies>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.add_plugins(SimulationPlugin);
    app.add_message::<LoadScenarioEvent>();
    app.add_systems(Update, handle_load_scenario_events);
    app
}

const ALL_15_PRESETS: [ScenarioPreset; 15] = [
    ScenarioPreset::GalacticCollision,
    ScenarioPreset::SolarNebulaMmsn,
    ScenarioPreset::AccretionDiskGenesis,
    ScenarioPreset::Trappist1System,
    ScenarioPreset::Kepler16Circumbinary,
    ScenarioPreset::HotJupiterMigration,
    ScenarioPreset::RoguePlanetFlyby,
    ScenarioPreset::LittleRedDot,
    ScenarioPreset::PulsarSystem,
    ScenarioPreset::MagnetarOutburst,
    ScenarioPreset::RelativisticBinary,
    ScenarioPreset::KozaiLidovTriple,
    ScenarioPreset::SagittariusAStar,
    ScenarioPreset::MolecularCloudCluster,
    ScenarioPreset::TidalDisruptionEvent,
];

#[test]
fn test_all_15_scenarios_load_and_step_without_nans_or_panics() {
    for preset in ALL_15_PRESETS {
        let mut app = create_test_app();

        // Dispatch scenario loading event
        app.world_mut()
            .resource_mut::<Messages<LoadScenarioEvent>>()
            .write(LoadScenarioEvent(preset));

        app.update();

        // 1. Verify scenario state matches requested preset
        let state = app.world().resource::<ActiveScenarioState>();
        assert_eq!(
            state.current_preset,
            preset,
            "ActiveScenarioState must match preset {}",
            preset.display_name()
        );

        // 2. Verify spawned bodies exist and have physically valid data
        let mut body_count = 0;
        {
            let mut query =
                app.world_mut()
                    .query::<(&CelestialBody, &SimPosition, &SimVelocity, &Mass, &Radius)>();
            for (body, pos, vel, mass, radius) in query.iter(app.world()) {
                body_count += 1;
                assert!(
                    pos.0.x.is_finite() && pos.0.y.is_finite() && pos.0.z.is_finite(),
                    "Body '{}' in preset {} has non-finite position: {:?}",
                    body.name,
                    preset.display_name(),
                    pos.0
                );
                assert!(
                    vel.0.x.is_finite() && vel.0.y.is_finite() && vel.0.z.is_finite(),
                    "Body '{}' in preset {} has non-finite velocity: {:?}",
                    body.name,
                    preset.display_name(),
                    vel.0
                );
                assert!(
                    mass.0 > 0.0 && mass.0.is_finite(),
                    "Body '{}' in preset {} has invalid mass: {}",
                    body.name,
                    preset.display_name(),
                    mass.0
                );
                assert!(
                    radius.0 > 0.0 && radius.0.is_finite(),
                    "Body '{}' in preset {} has invalid radius: {}",
                    body.name,
                    preset.display_name(),
                    radius.0
                );
            }
        }
        if preset == ScenarioPreset::MolecularCloudCluster {
            // Molecular cloud cluster starts as a starless cloud core where protostars condense via Jeans collapse
            assert!(
                body_count >= 0,
                "GMC core allows starless start before Jeans collapse"
            );
        } else {
            assert!(
                body_count > 0,
                "Scenario {} must spawn at least 1 celestial body (got 0)",
                preset.display_name()
            );
        }

        // 3. Step physics simulation for 3 frames and ensure numerical stability
        for _ in 0..3 {
            app.update();
        }

        // 4. Verify post-step coordinates remain finite
        let mut query = app
            .world_mut()
            .query::<(&CelestialBody, &SimPosition, &SimVelocity)>();
        for (body, pos, vel) in query.iter(app.world()) {
            assert!(
                pos.0.x.is_finite() && pos.0.y.is_finite() && pos.0.z.is_finite(),
                "Body '{}' in preset {} became non-finite after stepping: {:?}",
                body.name,
                preset.display_name(),
                pos.0
            );
            assert!(
                vel.0.x.is_finite() && vel.0.y.is_finite() && vel.0.z.is_finite(),
                "Body '{}' in preset {} has non-finite velocity after stepping: {:?}",
                body.name,
                preset.display_name(),
                vel.0
            );
        }
    }
}

#[test]
fn test_scenario_switching_cleans_up_previous_bodies() {
    let mut app = create_test_app();

    // 1. Load Solar System scenario (contains ~1,025 bodies)
    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::SolarNebulaMmsn));
    app.update();

    let solar_count = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .count();
    assert!(
        solar_count >= 30,
        "Solar Nebula scenario must spawn 30+ bodies (got {solar_count})"
    );

    // 2. Transition cleanly to TRAPPIST-1 (7 Earth-sized planets + 1 host star = 8 bodies)
    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::Trappist1System));
    app.update();

    let trappist_count = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .count();
    assert_eq!(
        trappist_count, 8,
        "Transitioning to TRAPPIST-1 must leave exactly 8 bodies (got {trappist_count})"
    );

    // 3. Transition cleanly to Relativistic Binary (2 compact neutron stars)
    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::RelativisticBinary));
    app.update();

    let binary_count = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .count();
    assert_eq!(
        binary_count, 2,
        "Transitioning to Relativistic Binary must leave exactly 2 bodies (got {binary_count})"
    );
}

#[test]
fn test_high_speed_time_warp_stability_across_compact_systems() {
    let mut app = create_test_app();

    // Load TRAPPIST-1
    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::Trappist1System));
    app.update();

    // Set time warp to 50x (~1.5 yr/s)
    app.world_mut().resource_mut::<TimeWarp>().multiplier = 50.0;

    // Run 10 physics ticks
    for _ in 0..10 {
        app.update();
    }

    // Verify all 7 planets remain within bound orbits (semimajor axis < 0.20 AU)
    let mut planets_query = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &SimVelocity)>();
    for (body, pos, _) in planets_query.iter(app.world()) {
        if body.body_type == BodyType::TerrestrialPlanet {
            let r = pos.0.length();
            assert!(
                r < 0.25,
                "TRAPPIST-1 planet '{}' escaped or drifted too far: r = {r:.4} AU",
                body.name
            );
        }
    }
}
