//! Phase 5 regression tests for scenario reload hygiene and preset scientific corrections.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::game::phases::{
    GlobalTrophies, LateHeavyBombardmentState, PhaseManager, SystemPhase,
};
use protostellar::game::ui::types::CosmicEventLog;
use protostellar::simulation::accretion::TheiaImpactState;
use protostellar::simulation::components::*;
use protostellar::simulation::disk::planetesimals::{
    PlanetesimalSpawner, auto_spawn_planetesimals,
};
use protostellar::simulation::economy::{ScienceData, UnlockedTechs};
use protostellar::simulation::events::CosmicEventManager;
use protostellar::simulation::neutron_stars::AlfvenBlastWave;
use protostellar::simulation::pebble_accretion::spawn_streaming_instability_minor_bodies;
use protostellar::simulation::probes::{ProbeCounter, ProbeType, SpaceProbe};
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{
    ActiveScenarioState, LoadScenarioEvent, ScenarioPreset, handle_load_scenario_events,
};
use protostellar::utils::math::state_vectors_to_orbital_elements;

fn build_phase5_scenario_app() -> App {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<SystemPhase>()
        .init_resource::<PhaseManager>()
        .init_resource::<GlobalTrophies>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<DiskParameters>()
        .init_resource::<SimTime>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<TimeWarp>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<SimulationConfig>()
        .init_resource::<PlanetesimalSpawner>()
        .init_resource::<TheiaImpactState>()
        .init_resource::<ScienceData>()
        .init_resource::<UnlockedTechs>()
        .init_resource::<CosmicEventLog>()
        .init_resource::<CosmicEventManager>()
        .init_resource::<ProbeCounter>()
        .init_resource::<ImpactShockwavePool>()
        .add_message::<LoadScenarioEvent>()
        .add_systems(Update, handle_load_scenario_events);
    app
}

fn assert_solar_mmsn_pluto_and_comets(w: &mut World) {
    let mut q = w.query::<(
        &CelestialBody,
        &SimPosition,
        &SimVelocity,
        &Composition,
        &SpinState,
    )>();
    let mut pluto_checked = false;
    let mut comet_checked = 0usize;
    for (body, pos, vel, comp, spin) in q.iter(w) {
        if body.name.contains("Pluto") {
            pluto_checked = true;
            assert!(comp.ice_frac >= 0.5);
            let elems = state_vectors_to_orbital_elements(pos.0, vel.0, 1.0, 0.0)
                .expect("Pluto orbital elements");
            assert!((elems.eccentricity - 0.2488).abs() < 1e-3);
            assert!(spin.spin_vector.length() > 0.0);
        } else if body.body_type == BodyType::Comet {
            comet_checked += 1;
            let elems = state_vectors_to_orbital_elements(pos.0, vel.0, 1.0, 0.0)
                .expect("Comet orbital elements");
            assert!(elems.eccentricity > 0.35);
        }
    }
    assert!(pluto_checked);
    assert_eq!(comet_checked, 6);
}

fn inject_dirty_scenario_state(w: &mut World) {
    w.resource_mut::<SimTime>().step_count = 9999;
    w.resource_mut::<TheiaImpactState>().moon_formed = true;
    w.resource_mut::<ScienceData>().points = 777.0;
    w.resource_mut::<ProbeCounter>().0 = 5;
    w.resource_mut::<ImpactShockwavePool>()
        .shockwaves
        .push(ImpactShockwave {
            position: Vec3::ZERO,
            radius: 1.0,
            max_radius: 2.0,
            timer: 0.5,
            max_timer: 1.0,
            color: Color::WHITE,
            primary_entity: None,
            local_position: Vec3::ZERO,
        });
    w.spawn((
        AlfvenBlastWave {
            source_entity: Entity::PLACEHOLDER,
            origin_au: DVec3::ZERO,
            radius_au: 1.0,
            expansion_speed_au_yr: 63_000.0,
            energy_erg: 1e45,
            luminosity_solar: 1e4,
            synchrotron_brightness: 1.0,
            age_seconds: 0.1,
            max_radius_au: 60.0,
        },
        SimPosition(DVec3::ZERO),
    ));
    w.spawn((
        SpaceProbe::new(ProbeType::Rover, 5),
        CelestialBody {
            body_type: BodyType::Asteroid,
            name: "Orphan Probe".to_string(),
        },
    ));
}

#[test]
fn test_phase5_preset_scientific_fixes_and_scenario_reset_hygiene() {
    let mut app = build_phase5_scenario_app();

    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::SolarNebulaMmsn));
    app.update();
    assert_solar_mmsn_pluto_and_comets(app.world_mut());

    inject_dirty_scenario_state(app.world_mut());

    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::Kepler16Circumbinary));
    app.update();
    {
        let w = app.world_mut();
        assert_eq!(w.resource::<SimTime>().step_count, 0);
        assert!(!w.resource::<TheiaImpactState>().moon_formed);
        assert!((w.resource::<ScienceData>().points - 100.0).abs() < 1e-6);
        assert_eq!(w.resource::<ProbeCounter>().0, 0);
        assert!(w.resource::<ImpactShockwavePool>().shockwaves.is_empty());
        assert_eq!(w.query::<&AlfvenBlastWave>().iter(w).count(), 0);
        assert_eq!(w.query::<&SpaceProbe>().iter(w).count(), 0);
        let kepler_a = w
            .query::<&CelestialBody>()
            .iter(w)
            .find(|b| b.name.contains("Kepler-16A"))
            .expect("Kepler-16A must exist");
        assert_eq!(kepler_a.body_type, BodyType::OrangeDwarf);
    }

    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::MagnetarOutburst));
    app.update();
    {
        let w = app.world_mut();
        assert_eq!(w.resource::<DiskParameters>().gas_disk_lifetime_yr, 0.0);
        assert_eq!(w.resource::<SimulationConfig>().gas_density_scale, 0.0);
        assert_eq!(w.resource::<SimulationConfig>().active_particles, 0);
        let lbv = w
            .query::<(
                &CelestialBody,
                Option<&IgnitionState>,
                Option<&StellarEvolutionState>,
            )>()
            .iter(w)
            .find(|(b, _, _)| b.name.contains("LBV 1806-20"))
            .expect("LBV 1806-20 must exist");
        assert!(lbv.1.is_some_and(|i| i.is_ignited));
        assert!(lbv.2.is_some());
    }

    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::GalacticCollision));
    app.update();
    let target_particles = app
        .world()
        .resource::<SimulationConfig>()
        .target_particle_count as u32;
    assert_eq!(
        app.world().resource::<SimulationConfig>().active_particles,
        target_particles,
        "GalacticCollision renders galaxy stars via ParticleSwarmMesh and must keep swarm active"
    );

    app.add_systems(
        Update,
        (
            auto_spawn_planetesimals,
            spawn_streaming_instability_minor_bodies,
        ),
    );
    app.world_mut().resource_mut::<SimTime>().elapsed_years = 500.0;
    app.update();
    assert_eq!(
        app.world().resource::<PlanetesimalSpawner>().total_spawned,
        0
    );
}
