//! Integration tests for Supernova Chemical Enrichment and Multi-Generation Star & Planet Formation.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::gpu::gmc_collapse::receive_gmc_collapse_events;
use protostellar::gpu::gmc_fluid::*;
use protostellar::rendering::effects::remnants::{
    PersistentRemnantPool, PersistentSupernovaRemnant,
};
use protostellar::rendering::effects::supernova::SupernovaType;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use protostellar::simulation::thermodynamics::{update_thermodynamics, StarIgnitionEvent};

#[test]
fn test_massive_star_supernova_evolution_in_gmc_cluster() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();

    // Set GMC scenario state
    let mut scenario_state = ActiveScenarioState::default();
    scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    app.insert_resource(scenario_state);

    let star_entity = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Massive Cluster Star".to_string(),
                body_type: BodyType::BlueSupergiant,
            },
            Mass(18.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Radius(0.1),
            Temperature(25_000.0),
            Luminosity(50_000.0),
            IgnitionState {
                core_temperature: 1.5e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 5.0,
            },
            StellarEvolutionState {
                phase: StellarEvolutionPhase::MainSequence,
                hydrogen_core_fraction: 1.0,
                helium_core_fraction: 0.0,
                envelope_mass_loss_rate: 0.0,
                phase_timer_years: 0.0,
                nebula_expansion_radius_au: 0.0,
                nebula_opacity: 0.0,
            },
            CentralStar,
        ))
        .id();

    app.add_systems(Update, update_thermodynamics);

    // In GMC mode, lifetime of an 18 M_sun star is ~2000 years.
    // Step 1: 3500 years consumes hydrogen core and swells into Red Supergiant
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 3500.0;
        sim_time.elapsed_years = 3500.0;
    }
    app.update();

    // Step 2: 800 years in Red Supergiant triggers core-collapse supernova
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 800.0;
        sim_time.elapsed_years = 4300.0;
    }
    app.update();

    // Verify star transitioned to remnant and emitted SupernovaEvent with chemical payload
    let body = app.world().get::<CelestialBody>(star_entity).unwrap();
    assert!(
        body.body_type.is_remnant(),
        "Massive star must have detonated into remnant in accelerated GMC mode (got {:?})",
        body.body_type
    );

    let sn_events = app.world().resource::<Messages<SupernovaEvent>>();
    let mut reader = sn_events.get_cursor();
    let events: Vec<_> = reader.read(sn_events).collect();
    assert_eq!(events.len(), 1, "Expected exactly 1 SupernovaEvent emitted");

    let sn = &events[0];
    assert_eq!(sn.star_entity, star_entity);
    assert!(
        sn.ejected_metals_solar >= 1.5,
        "Supernova must eject metals into ISM (got {:.2} M☉)",
        sn.ejected_metals_solar
    );
    assert!(
        sn.ejected_composition.metal_frac >= 0.20,
        "Ejected composition must be rich in heavy elements (got {:.2})",
        sn.ejected_composition.metal_frac
    );
    assert!(
        sn.ejected_composition.silicate_frac >= 0.50,
        "Ejected composition must be rich in rocky silicates (got {:.2})",
        sn.ejected_composition.silicate_frac
    );
}

#[test]
fn test_persistent_remnant_metallicity_sampling() {
    let mut pool = PersistentRemnantPool::default();
    assert_eq!(pool.sample_metallicity(Vec3::ZERO), 0.0);

    // Add an expanding supernova remnant enriched with 3.5 M_sun of heavy elements
    pool.remnants.push(PersistentSupernovaRemnant {
        star_entity: Entity::from_bits(123),
        center: Vec3::new(10.0, 0.0, 0.0),
        remnant_type: SupernovaType::TypeII,
        initial_radius_au: 15.0,
        current_radius_au: 60.0,
        expansion_rate_au_yr: 0.05,
        ejecta_mass_solar: 16.5,
        metals_mass_solar: 3.5,
        age_years: 1200.0,
        max_age_years: 25_000.0,
        opacity: 0.8,
        has_pwn: true,
        filaments: Vec::new(),
    });

    // Sample inside remnant shell (distance 15 AU from center)
    let z_inside = pool.sample_metallicity(Vec3::new(25.0, 0.0, 0.0));
    assert!(
        z_inside >= 0.006,
        "Inside remnant, metallicity Z must be enriched (got {:.4})",
        z_inside
    );

    // Sample far outside remnant shell (distance 200 AU)
    let z_outside = pool.sample_metallicity(Vec3::new(250.0, 0.0, 0.0));
    assert_eq!(
        z_outside, 0.0,
        "Far outside remnant, gas must remain primordial (Z = 0.0)"
    );
}

#[test]
fn test_gen2_jeans_collapse_spawns_rocky_planetary_system() {
    let mut app = App::new();
    let (tx, rx) = flume::bounded::<Vec<GpuJeansCollapseEvent>>(4);

    app.insert_resource(GmcCollapseEventReceiver { rx });

    let mut scenario_state = ActiveScenarioState::default();
    scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    scenario_state.scenario_time_years = 100.0;
    app.insert_resource(scenario_state);

    app.add_systems(Update, receive_gmc_collapse_events);

    // Send an enriched Gen-II Jeans collapse event (Z = 0.028)
    let collapse_event = GpuJeansCollapseEvent {
        grid_coords: [48, 48, 48],
        metallicity: 0.028,
        world_pos: [25.0, 5.0, -10.0],
        local_mass_solar: 2.2,
        com_velocity: [0.05, -0.02, 0.03],
        temperature_k: 16.0,
    };
    tx.send(vec![collapse_event]).unwrap();

    app.update();

    // Query spawned bodies
    let bodies: Vec<(&CelestialBody, &Composition, &Mass)> = app
        .world_mut()
        .query::<(&CelestialBody, &Composition, &Mass)>()
        .iter(app.world())
        .collect();

    // Must have spawned 1 central protostar + 3 orbiting planets = 4 bodies
    assert_eq!(
        bodies.len(),
        4,
        "Expected 1 Gen-II Protostar and 3 planets (got {})",
        bodies.len()
    );

    // Find central star
    let star = bodies
        .iter()
        .find(|(b, _, _)| b.body_type == BodyType::Protostar)
        .expect("Protostar must be spawned");
    assert!(
        star.0.name.contains("Gen-II NovaCore"),
        "Protostar must have Gen-II NovaCore designation (got {})",
        star.0.name
    );
    assert!(
        star.1.metal_frac > 0.005,
        "Gen-II star must be enriched with metals (got {:.4})",
        star.1.metal_frac
    );

    // Find planets
    let planets: Vec<_> = bodies
        .iter()
        .filter(|(b, _, _)| b.body_type == BodyType::Protoplanet || b.body_type.is_planet())
        .collect();
    assert_eq!(planets.len(), 3, "Expected 3 planets in the system");

    // Check planet 1: Metal-rich iron core
    let metal_rich = planets
        .iter()
        .find(|(b, comp, _)| b.name.contains("Iron-Rich") && comp.metal_frac > 0.50);
    assert!(
        metal_rich.is_some(),
        "Gen-II system must contain an inner Iron-Rich world"
    );

    // Check planet 2: Rocky Earth analogue
    let rocky = planets
        .iter()
        .find(|(b, comp, _)| b.name.contains("Rocky Earth") && comp.silicate_frac > 0.50);
    assert!(
        rocky.is_some(),
        "Gen-II system must contain a Rocky Earth-Analogue world"
    );

    // Check planet 3: Volatile / Ocean world
    let volatile = planets
        .iter()
        .find(|(b, comp, _)| b.name.contains("Volatile Ocean") && comp.organics_frac > 0.10);
    assert!(
        volatile.is_some(),
        "Gen-II system must contain a Volatile Ocean world"
    );
}

#[test]
fn test_gen1_jeans_collapse_spawns_pristine_system() {
    let mut app = App::new();
    let (tx, rx) = flume::bounded::<Vec<GpuJeansCollapseEvent>>(4);

    app.insert_resource(GmcCollapseEventReceiver { rx });

    let mut scenario_state = ActiveScenarioState::default();
    scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    scenario_state.scenario_time_years = 100.0;
    app.insert_resource(scenario_state);

    app.add_systems(Update, receive_gmc_collapse_events);

    // Send a pristine Gen-I Jeans collapse event (Z = 0.0)
    let collapse_event = GpuJeansCollapseEvent {
        grid_coords: [48, 48, 48],
        metallicity: 0.0,
        world_pos: [-30.0, 0.0, 15.0],
        local_mass_solar: 1.8,
        com_velocity: [0.02, 0.01, -0.04],
        temperature_k: 14.0,
    };
    tx.send(vec![collapse_event]).unwrap();

    app.update();

    let bodies: Vec<(&CelestialBody, &Composition)> = app
        .world_mut()
        .query::<(&CelestialBody, &Composition)>()
        .iter(app.world())
        .collect();

    assert_eq!(bodies.len(), 4);

    let star = bodies
        .iter()
        .find(|(b, _)| b.body_type == BodyType::Protostar)
        .expect("Gen-I Protostar must be spawned");
    assert!(
        star.0.name.contains("Gen-I Jeans"),
        "Protostar must have Gen-I Jeans designation (got {})",
        star.0.name
    );
    assert_eq!(
        star.1.gas_frac, 1.0,
        "Gen-I star must form from pure hydrogen (Pop III)"
    );

    // Planets must be gas giants
    let planets: Vec<_> = bodies
        .iter()
        .filter(|(b, _)| b.body_type == BodyType::Protoplanet)
        .collect();
    for (p, comp) in planets {
        assert!(
            comp.gas_frac > 0.90,
            "Gen-I companion world must be a gas giant (got gas_frac={:.2})",
            comp.gas_frac
        );
        assert!(p.name.contains("Gas Giant") || p.name.contains("b"));
    }
}
