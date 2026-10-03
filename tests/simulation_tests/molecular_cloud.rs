//! Integration tests for Giant Molecular Cloud (GMC) & Jeans Instability Star Cluster Genesis.

use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::molecular_cloud::*;
use protostellar::utils::constants::*;

#[test]
fn test_jeans_instability_physics_and_sound_speed() {
    // 1. Isothermal sound speed at T = 15 K should be ~0.27 km/s
    let c_s_au_yr = calculate_gmc_sound_speed_au_yr(GMC_CLOUD_TEMPERATURE_K);
    let c_s_km_s = c_s_au_yr * AU_PER_YR_TO_KM_PER_S;
    assert!(
        (c_s_km_s - 0.274).abs() < 0.03,
        "GMC sound speed at 15 K should be ~0.27 km/s (got {:.3} km/s)",
        c_s_km_s
    );

    // 2. Thermal Jeans mass in dense pre-stellar core clump (rho ~ 5e-11 M_sun / AU^3)
    let rho_dense_clump = 5.0e-11;
    let m_jeans_th = calculate_jeans_mass_solar(rho_dense_clump, GMC_CLOUD_TEMPERATURE_K);
    assert!(
        m_jeans_th > 0.10 && m_jeans_th < 1.0,
        "Thermal Jeans mass in dense clump should be 0.1 - 1.0 M_sun (got {:.3} M_sun)",
        m_jeans_th
    );

    // 3. Turbulent Jeans mass with core-scale transonic dispersion (sigma_v = 0.65 km/s)
    let m_jeans_core =
        calculate_turbulent_jeans_mass_solar(rho_dense_clump, GMC_CLOUD_TEMPERATURE_K, 0.65);
    assert!(
        m_jeans_core > 1.0 && m_jeans_core < 4.0,
        "Core turbulent Jeans mass should be 1.0 - 4.0 M_sun (got {:.3} M_sun)",
        m_jeans_core
    );

    // 4. Cloud-scale turbulent dispersion (sigma_v = 1.6 km/s) yielding massive parent clump
    let m_jeans_cloud =
        calculate_turbulent_jeans_mass_solar(rho_dense_clump, GMC_CLOUD_TEMPERATURE_K, 1.6);
    assert!(
        m_jeans_cloud > 10.0 && m_jeans_cloud < 20.0,
        "Parent clump turbulent Jeans mass should be 10.0 - 20.0 M_sun (got {:.3} M_sun)",
        m_jeans_cloud
    );
}

#[test]
fn test_molecular_cloud_cluster_scenario_spawning_and_hierarchy() {
    let mut app = App::new();

    // 1. Verify pristine starless scenario initialization (zero initial stars at t=0)
    let mut disk_params = DiskParameters::default();
    let opt_primary = {
        let mut commands = app.world_mut().commands();
        spawn_molecular_cloud_cluster_scenario(&mut commands, &mut disk_params)
    };
    app.insert_resource(disk_params);
    app.update();

    assert!(
        opt_primary.is_none(),
        "Molecular cloud scenario must spawn without pre-baked stars (returns None)"
    );

    let initial_bodies_count = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();
    assert_eq!(
        initial_bodies_count, 0,
        "Molecular cloud must start with 0 celestial bodies, allowing stars to form organically via Jeans collapse"
    );

    let disk = app.world().resource::<DiskParameters>();
    assert_eq!(disk.central_star_mass, 0.0);
    assert_eq!(disk.disk_mass, 0.0);
    assert_eq!(disk.outer_radius_au, GMC_CORE_RADIUS_AU);
    assert!(
        disk.gas_disk_lifetime_yr > 100_000.0,
        "Molecular cloud must feature extended cloud dissipation lifetime"
    );

    // 2. Verify pre-seeded cluster test fixture hierarchy & kinematics
    let mut fixture_app = App::new();
    let mut fixture_disk_params = DiskParameters::default();
    let primary_ent = {
        let mut commands = fixture_app.world_mut().commands();
        spawn_preseeded_cluster_fixture(&mut commands, &mut fixture_disk_params)
    };
    fixture_app.insert_resource(fixture_disk_params);
    fixture_app.update();

    // Primary Central Protostar checks
    let primary_body = fixture_app.world().get::<CelestialBody>(primary_ent).unwrap();
    let primary_mass = fixture_app.world().get::<Mass>(primary_ent).unwrap();
    let primary_rad = fixture_app.world().get::<Radius>(primary_ent).unwrap();
    let primary_ign = fixture_app.world().get::<IgnitionState>(primary_ent).unwrap();

    assert_eq!(primary_body.body_type, BodyType::Protostar);
    assert!(primary_body.name.contains("Protostar Alpha"));
    assert!((primary_mass.0 - 3.5).abs() < 1e-4);
    assert!(
        primary_rad.0 > 10.0 * SOLAR_RADIUS_AU,
        "Class 0 protostar must have a bloated pre-collapse envelope (>10 R_sun)"
    );
    assert!(
        !primary_ign.is_ignited,
        "Pre-stellar seed core has not yet undergone hydrogen fusion ignition"
    );

    // Cluster population checks
    let mut protostar_count = 0;
    let mut brown_dwarf_count = 0;
    let mut jeans_clump_count = 0;
    let mut total_mass = 0.0;
    let mut has_binary_companion = false;

    let mut query = fixture_app
        .world_mut()
        .query::<(Entity, &CelestialBody, &Mass, &SimPosition, &SimVelocity)>();

    for (ent, body, mass, pos, vel) in query.iter(fixture_app.world()) {
        total_mass += mass.0;

        if body.body_type == BodyType::Protostar {
            protostar_count += 1;
        }
        if body.body_type == BodyType::BrownDwarf {
            brown_dwarf_count += 1;
        }
        if body.body_type == BodyType::Protoplanet && body.name.contains("Jeans Clump") {
            jeans_clump_count += 1;
        }

        if body.name.contains("Protostar Beta") {
            has_binary_companion = true;
            let dist = pos.0.length();
            assert!(
                dist > 20.0 && dist < 60.0,
                "Infalling binary companion Beta should be in close orbit (got {:.1} AU)",
                dist
            );
            assert!(
                vel.0.length() > 0.0,
                "Cluster member must possess active orbital kinematics"
            );
        }

        if ent != primary_ent {
            let dist = pos.0.length();
            assert!(
                dist <= GMC_CORE_RADIUS_AU * 1.05,
                "Cluster member {} placed outside cloud boundary ({:.1} AU > {:.1} AU)",
                body.name,
                dist,
                GMC_CORE_RADIUS_AU
            );
        }
    }

    assert!(
        protostar_count >= 7,
        "Cluster should spawn at least 7 protostellar seeds (found {})",
        protostar_count
    );
    assert_eq!(
        brown_dwarf_count, 1,
        "Cluster should spawn a brown dwarf substellar embryo"
    );
    assert_eq!(
        jeans_clump_count, 2,
        "Cluster should spawn 2 dense pre-stellar Jeans fragmentation clumps"
    );
    assert!(
        has_binary_companion,
        "Cluster should establish an eccentric infalling proto-binary pair (Alpha & Beta)"
    );
    assert!(
        total_mass > 10.0 && total_mass < 15.0,
        "Total stellar seed cluster mass should be ~11 M_sun (got {:.2} M_sun)",
        total_mass
    );
}

#[test]
fn test_molecular_cloud_cluster_scenario_phase_mapping() {
    use bevy::prelude::*;
    use protostellar::game::phases::{LateHeavyBombardmentState, PhaseManager, SystemPhase};
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{
        handle_load_scenario_events, ActiveScenarioState, LoadScenarioEvent, ScenarioPreset,
    };

    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<PhaseManager>();
    app.init_resource::<ActiveScenarioState>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<LoadScenarioEvent>();
    app.add_systems(Update, handle_load_scenario_events);

    // Initial phase check: default is ProtoplanetaryDisk or uninitialized
    assert_ne!(
        app.world().resource::<PhaseManager>().current_phase,
        SystemPhase::MolecularCloudCollapse,
        "Initial phase manager phase should not be MolecularCloudCollapse"
    );

    // Dispatch LoadScenarioEvent for MolecularCloudCluster
    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::MolecularCloudCluster));

    app.update();

    // 1. Verify ActiveScenarioState preset updated
    let scenario_state = app.world().resource::<ActiveScenarioState>();
    assert_eq!(
        scenario_state.current_preset,
        ScenarioPreset::MolecularCloudCluster,
        "Active scenario preset must update to MolecularCloudCluster"
    );

    // 2. Verify PhaseManager.current_phase updated to MolecularCloudCollapse
    let phase_mgr = app.world().resource::<PhaseManager>();
    assert_eq!(
        phase_mgr.current_phase,
        SystemPhase::MolecularCloudCollapse,
        "PhaseManager.current_phase must transition to MolecularCloudCollapse"
    );
    assert!(
        phase_mgr.phase_description.contains("Molecular Cloud")
            || phase_mgr.phase_description.contains("Jeans"),
        "Phase description must reflect Molecular Cloud Jeans collapse"
    );

    // 3. Verify Bevy State<SystemPhase> resource transitioned
    let system_phase = app.world().resource::<State<SystemPhase>>();
    assert_eq!(
        *system_phase.get(),
        SystemPhase::MolecularCloudCollapse,
        "Bevy State<SystemPhase> must be MolecularCloudCollapse"
    );
}

#[test]
fn test_auto_spawn_planetesimals_guarded_against_gmc_cluster() {
    use bevy::prelude::*;
    use protostellar::simulation::components::CelestialBody;
    use protostellar::simulation::disk::planetesimals::{
        auto_spawn_planetesimals, PlanetesimalSpawner,
    };
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<PlanetesimalSpawner>();
    app.init_resource::<ActiveScenarioState>();

    // Configure spawner conditions that would ordinarily trigger aggressive planetesimal spawning
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 250.0;

        let mut disk_params = app.world_mut().resource_mut::<DiskParameters>();
        disk_params.gas_disk_lifetime_yr = 50_000.0;

        let mut spawner = app.world_mut().resource_mut::<PlanetesimalSpawner>();
        spawner.max_ecs_bodies = 500;
        spawner.last_spawn_yr = 0.0;

        // Activate GMC scenario preset
        let mut scenario_state = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    }

    app.add_systems(Update, auto_spawn_planetesimals);
    app.update();

    // 1. Verify NO bodies were spawned because GMC cluster is active
    let body_count = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();
    assert_eq!(
        body_count, 0,
        "auto_spawn_planetesimals must NOT spawn bodies when GMC cluster is active"
    );

    let spawner = app.world().resource::<PlanetesimalSpawner>();
    assert_eq!(
        spawner.total_spawned, 0,
        "PlanetesimalSpawner.total_spawned must remain 0 in GMC mode"
    );
    assert_eq!(
        spawner.last_spawn_yr, 0.0,
        "PlanetesimalSpawner.last_spawn_yr must not advance in GMC mode"
    );

    // 2. Invert scenario to standard protoplanetary disk scenario to confirm guarding is selective
    {
        let mut scenario_state = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario_state.current_preset = ScenarioPreset::SolarNebulaMmsn;
    }

    app.update();

    let disk_body_count = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();
    assert!(
        disk_body_count > 0,
        "auto_spawn_planetesimals must spawn bodies when SolarNebulaMmsn disk is active (found {})",
        disk_body_count
    );
}

fn setup_non_central_protostar_app() -> (
    bevy::prelude::App,
    bevy::prelude::Entity,
    bevy::prelude::Entity,
) {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::{update_thermodynamics, StarIgnitionEvent};
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_systems(Update, update_thermodynamics);

    // 1. Central Star (Alpha, 3.5 M_sun, envelope radius 0.05 AU)
    let central_r = 0.05;
    let _central_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Protostar Alpha".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(3.5),
            Radius(central_r),
            Temperature(3200.0),
            Luminosity(45.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 9.99e6,
                fusion_fraction: 0.99,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // 2. Non-Central Protostar (Beta, 1.8 M_sun) located deep inside Alpha's envelope:
    // r = 0.010 AU < central_r (0.05 AU). Initial core temperature = 9.95 MK.
    let beta_pos = DVec3::new(0.010, 0.0, 0.0);
    let beta_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Protostar Beta (Infalling Binary)".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(1.8),
            Radius(0.015),
            Temperature(3600.0),
            Luminosity(14.0),
            SimPosition(beta_pos),
            SimVelocity(DVec3::new(0.0, 0.0, 5.0)),
            SimAcceleration::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 9.95e6,
                fusion_fraction: 0.95,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // 3. Control Terrestrial Planet at the same deep plunge distance (0.010 AU)
    let planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Doomed Terrestrial Embryo".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Temperature(300.0),
            Composition::rocky(),
            SimPosition(beta_pos),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
        ))
        .id();

    // Advance simulation time by dt = 1.0 yr
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 1.0;
        sim_time.elapsed_years = 1.0;
    }

    (app, beta_ent, planet_ent)
}

fn verify_engulfment_bypass(
    app: &bevy::prelude::App,
    beta_ent: bevy::prelude::Entity,
    planet_ent: bevy::prelude::Entity,
) {
    use bevy::prelude::*;
    use protostellar::simulation::components::PlanetaryEngulfmentEvent;

    // The control terrestrial planet MUST be engulfed and despawned
    assert!(
        app.world().get_entity(planet_ent).is_err(),
        "Plunging planet inside central star envelope MUST be engulfed and despawned"
    );

    // The non-central protostar MUST NOT be engulfed or despawned
    assert!(
        app.world().get_entity(beta_ent).is_ok(),
        "Non-central protostar inside stellar envelope must NOT be engulfed (exempt via is_star_or_remnant)"
    );

    // Verify PlanetaryEngulfmentEvent was dispatched for the planet, NOT the protostar
    let engulf_events = app.world().resource::<Messages<PlanetaryEngulfmentEvent>>();
    assert!(
        !engulf_events.is_empty(),
        "PlanetaryEngulfmentEvent must be emitted for the plunged planet"
    );
    for ev in engulf_events.iter_current_update_messages() {
        assert_ne!(
            ev.planet_entity, beta_ent,
            "PlanetaryEngulfmentEvent must NEVER target a protostellar entity"
        );
    }
}

fn verify_protostar_ignition(app: &bevy::prelude::App, beta_ent: bevy::prelude::Entity) {
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::thermodynamics::StarIgnitionEvent;

    let beta_ign = app.world().get::<IgnitionState>(beta_ent).unwrap();
    assert!(
        beta_ign.core_temperature >= 1.0e7,
        "Protostar Beta core temperature must cross ignition threshold 1.0e7 K (got {:.2e})",
        beta_ign.core_temperature
    );
    assert!(
        beta_ign.is_ignited,
        "Protostar Beta must transition to ignited state"
    );
    assert_eq!(
        beta_ign.fusion_fraction, 1.0,
        "Protostar Beta fusion fraction must reach 1.0 upon auto-ignition"
    );

    // Verify StarIgnitionEvent was emitted for Protostar Beta
    let ignition_events = app.world().resource::<Messages<StarIgnitionEvent>>();
    let beta_ignition_emitted = ignition_events
        .iter_current_update_messages()
        .any(|ev| ev.star_entity == beta_ent);
    assert!(
        beta_ignition_emitted,
        "StarIgnitionEvent must be emitted for ignited non-central protostar"
    );

    // Verify Luminosity and Temperature transition to main-sequence
    let beta_lum = app.world().get::<Luminosity>(beta_ent).unwrap();
    let beta_temp = app.world().get::<Temperature>(beta_ent).unwrap();
    let expected_main_seq_lum = 1.8_f64.powf(3.5);
    assert!(
        (beta_lum.0 - expected_main_seq_lum).abs() < 0.1,
        "Protostar Beta luminosity must transition to main-sequence (expected ~{:.2}, got {:.2})",
        expected_main_seq_lum,
        beta_lum.0
    );
    assert!(
        beta_temp.0 > 4000.0,
        "Protostar Beta surface temperature must transition to main-sequence (got {:.1} K)",
        beta_temp.0
    );
}

#[test]
fn test_non_central_protostar_ignition_and_engulfment_bypass() {
    let (mut app, beta_ent, planet_ent) = setup_non_central_protostar_app();
    app.update();
    verify_engulfment_bypass(&app, beta_ent, planet_ent);
    verify_protostar_ignition(&app, beta_ent);
}
