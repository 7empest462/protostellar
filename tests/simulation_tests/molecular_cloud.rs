//! Integration tests for Giant Molecular Cloud (GMC) & Jeans Instability Star Cluster Genesis.

use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::molecular_cloud::*;
use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
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
    let primary_body = fixture_app
        .world()
        .get::<CelestialBody>(primary_ent)
        .unwrap();
    let primary_mass = fixture_app.world().get::<Mass>(primary_ent).unwrap();
    let primary_rad = fixture_app.world().get::<Radius>(primary_ent).unwrap();
    let primary_ign = fixture_app
        .world()
        .get::<IgnitionState>(primary_ent)
        .unwrap();

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

    let mut query =
        fixture_app
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
    app.init_resource::<protostellar::game::phases::GlobalTrophies>();
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

#[test]
fn test_gmc_protostar_stabilization_promotion_attaches_disk_and_embryos() {
    use bevy::math::DVec3;

    let mut app = App::new();
    let mut scenario_state = ActiveScenarioState::default();
    scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    app.insert_resource(scenario_state);
    app.init_resource::<GmcClusterDiskManager>();
    app.add_systems(Update, promote_stabilized_gmc_protostars);

    // Spawn an unignited protostar in GMC
    let star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Protostar Genesis-1".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(1.2),
            Radius(3.0 * SOLAR_RADIUS_AU),
            Temperature(3800.0),
            Luminosity(2.5),
            SimPosition(DVec3::new(10.0, 0.0, 5.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 0.5)),
            IgnitionState {
                core_temperature: 8.5e6,
                fusion_fraction: 0.85,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // Tick while unignited - must NOT have disk or embryos yet
    app.update();
    assert!(
        app.world().get::<CircumstellarDisk>(star_ent).is_none(),
        "Unignited protostar must not yet possess a circumstellar disk"
    );
    let embryo_count_before = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();
    assert_eq!(
        embryo_count_before, 1,
        "Only the protostar exists before stabilization"
    );

    // Protostar stabilizes / ignites fusion
    {
        let mut ign = app.world_mut().get_mut::<IgnitionState>(star_ent).unwrap();
        ign.is_ignited = true;
        ign.fusion_fraction = 1.0;
        ign.core_temperature = 1.2e7;
    }

    // Tick - GMC promotion triggers
    app.update();

    // 1. Verify CircumstellarDisk and PlanetaryRingSystem attachment
    let disk = app.world().get::<CircumstellarDisk>(star_ent);
    assert!(
        disk.is_some(),
        "Stabilized protostar must receive a CircumstellarDisk"
    );
    let disk = disk.unwrap();
    assert!(
        disk.is_active_full_disk,
        "New circumstellar disk must be active"
    );
    assert!(disk.inner_radius_au > 0.0 && disk.outer_radius_au > disk.inner_radius_au);

    let ring = app.world().get::<PlanetaryRingSystem>(star_ent);
    assert!(
        ring.is_some(),
        "Stabilized protostar must receive PlanetaryRingSystem for 3D visual geometry"
    );

    // 2. Verify spawned protoplanetary embryos
    let mut query = app
        .world_mut()
        .query::<(&CelestialBody, &Mass, &SimPosition, &SimVelocity)>();
    let mut embryos = Vec::new();
    for (body, mass, pos, vel) in query.iter(app.world()) {
        if body.name.contains("Embryo") {
            embryos.push((body.name.clone(), body.body_type, mass.0, pos.0, vel.0));
        }
    }

    assert_eq!(
        embryos.len(),
        3,
        "GMC promotion must spawn exactly 3 protoplanetary embryos (got {})",
        embryos.len()
    );
    for (name, b_type, m, pos, vel) in embryos {
        assert_eq!(
            b_type,
            BodyType::Protoplanet,
            "Embryo {name} must be BodyType::Protoplanet"
        );
        assert!(m > 0.0, "Embryo {name} must have positive mass");
        let dist = (pos - DVec3::new(10.0, 0.0, 5.0)).length();
        assert!(
            dist > 0.3 && dist < 20.0,
            "Embryo {name} must orbit host star within circumstellar disk bounds (got {:.2} AU)",
            dist
        );
        assert!(
            vel.length() > 0.0,
            "Embryo {name} must possess Keplerian orbital velocity"
        );
    }
}

#[test]
fn test_gmc_active_full_disk_capping() {
    use bevy::math::DVec3;

    let mut app = App::new();
    let mut scenario_state = ActiveScenarioState::default();
    scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    app.insert_resource(scenario_state);

    // Enforce a strict cap of 3 active full disks
    app.insert_resource(GmcClusterDiskManager {
        max_active_full_disks: 3,
    });
    app.add_systems(Update, promote_stabilized_gmc_protostars);

    let mut star_entities = Vec::new();
    for i in 1..=4 {
        let ent = app
            .world_mut()
            .spawn((
                CelestialBody {
                    name: format!("Star-{i}"),
                    body_type: BodyType::YellowDwarf,
                },
                Mass(1.0 + i as f64 * 0.2),
                Radius(SOLAR_RADIUS_AU),
                Temperature(5800.0),
                Luminosity(1.0),
                SimPosition(DVec3::new(i as f64 * 50.0, 0.0, 0.0)),
                SimVelocity(DVec3::new(0.0, 0.0, 0.2)),
                IgnitionState {
                    core_temperature: 1.2e7,
                    fusion_fraction: 1.0,
                    is_ignited: true,
                    shockwave_radius: 0.5,
                },
            ))
            .id();
        star_entities.push(ent);

        // Advance simulated time slightly for each star so ages are distinct
        if i > 1 {
            let prev = star_entities[i - 2];
            if let Some(mut prev_disk) = app.world_mut().get_mut::<CircumstellarDisk>(prev) {
                prev_disk.age_years += 1000.0;
            }
        }

        app.update();
    }

    // After 4 stars stabilized with cap=3, active full disks must be capped at 3
    let mut active_count = 0;
    for &ent in &star_entities {
        if let Some(disk) = app.world().get::<CircumstellarDisk>(ent) {
            let has_ring = app.world().get::<PlanetaryRingSystem>(ent).is_some();
            if disk.is_active_full_disk && has_ring {
                active_count += 1;
            }
        }
    }
    assert_eq!(
        active_count, 3,
        "Active full disks must be capped at max_active_full_disks (3)"
    );

    // Oldest star (Star-1) must have had its disk dissipated
    let star1_disk = app
        .world()
        .get::<CircumstellarDisk>(star_entities[0])
        .unwrap();
    assert!(
        !star1_disk.is_active_full_disk,
        "Oldest star's circumstellar disk must be marked inactive"
    );
    assert!(
        app.world()
            .get::<PlanetaryRingSystem>(star_entities[0])
            .is_none(),
        "Oldest star's PlanetaryRingSystem mesh must be removed to free GPU fill rate"
    );

    // Star-1's orbiting embryos must STILL exist!
    let star1_embryos = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .filter(|b| b.name.starts_with("Star-1") && b.name.contains("Embryo"))
        .count();
    assert_eq!(
        star1_embryos, 3,
        "Star-1's planetary embryos must survive disk dissipation"
    );

    // Newest star (Star-4) must possess an active full disk
    let star4_disk = app
        .world()
        .get::<CircumstellarDisk>(star_entities[3])
        .unwrap();
    assert!(
        star4_disk.is_active_full_disk,
        "Most recently stabilized star must possess an active full disk"
    );
    assert!(
        app.world()
            .get::<PlanetaryRingSystem>(star_entities[3])
            .is_some(),
        "Most recently stabilized star must possess PlanetaryRingSystem"
    );
}

#[test]
fn test_gmc_circumstellar_disk_photoevaporation_lifetime() {
    let mut app = App::new();
    let mut scenario_state = ActiveScenarioState::default();
    scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    app.insert_resource(scenario_state);
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();

    let star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Star-Photoevaporating".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            Mass(1.0),
            CircumstellarDisk {
                inner_radius_au: 0.6,
                outer_radius_au: 8.0,
                disk_mass_solar: 0.015,
                is_active_full_disk: true,
                age_years: 74_999.0,
                lifetime_years: 75_000.0,
            },
            PlanetaryRingSystem::default(),
        ))
        .id();

    app.add_systems(Update, update_gmc_circumstellar_disks);

    // Advance simulated time past 75,000 years lifetime
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 2.0;
    }

    app.update();

    let disk = app.world().get::<CircumstellarDisk>(star_ent).unwrap();
    assert!(
        !disk.is_active_full_disk,
        "Circumstellar disk exceeding its lifetime must transition to inactive"
    );
    assert!(
        app.world().get::<PlanetaryRingSystem>(star_ent).is_none(),
        "PlanetaryRingSystem must be removed upon photoevaporation clearing"
    );
}

#[test]
fn test_gmc_cluster_orbital_speed_and_dark_matter_stability() {
    // 1. Initial protostar stage (3.5 M☉ central seed)
    // Dark Matter halo (8000 M☉) and cloud core (50 M☉) dominate outer gravity
    let r = 120.0;
    let v_early = calculate_gmc_orbital_speed(r, 3.5, 8000.0, 50.0);
    assert!(
        v_early > 15.0 && v_early < 30.0,
        "Early orbital speed at r=120 AU must be balanced by dark matter and core (got {:.2} AU/yr)",
        v_early
    );

    // 2. Intermediate-Mass Black Hole stage (1000 M☉)
    let v_imbh = calculate_gmc_orbital_speed(r, 1000.0, 8000.0, 50.0);
    assert!(
        v_imbh > v_early,
        "Orbital speed must scale higher as black hole accretes mass"
    );

    // 3. Supermassive Black Hole stage (100,000 M☉)
    let v_smbh = calculate_gmc_orbital_speed(r, 100_000.0, 8000.0, 50.0);
    // Keplerian circular speed around 100,000 M☉ at 120 AU: sqrt(39.478 * 100,000 / 120) ~ 181.4 AU/yr
    assert!(
        v_smbh > 170.0 && v_smbh < 220.0,
        "Supermassive black hole orbital speed must provide full Keplerian circular support (got {:.2} AU/yr)",
        v_smbh
    );

    // 4. Verification that newly formed star with calculate_gmc_orbital_speed maintains a stable, bound orbit
    use bevy::math::DVec3;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();

    // Active GMC cluster scenario preset
    let mut scenario_state = ActiveScenarioState::default();
    scenario_state.current_preset = ScenarioPreset::MolecularCloudCluster;
    app.insert_resource(scenario_state);

    let _smbh = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                body_type: BodyType::BlackHole,
                name: "Galactic Nucleus (Supermassive Black Hole)".to_string(),
            },
            Mass(100_000.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(0.005),
        ))
        .id();

    let star_pos = DVec3::new(120.0, 0.0, 0.0);
    let star_vel = DVec3::new(0.0, 0.0, v_smbh);
    let star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Protostar,
                name: "Orbiting Protostar".to_string(),
            },
            Mass(2.0),
            SimPosition(star_pos),
            SimVelocity(star_vel),
            SimAcceleration::default(),
            Radius(SOLAR_RADIUS_AU),
        ))
        .id();

    app.add_systems(
        Update,
        protostellar::simulation::physics::step_physics_simulation,
    );

    // Run 50 simulation steps with dt = 0.001 yr (~18 hours per step)
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 0.001;
    }

    for _ in 0..50 {
        app.update();
    }

    let end_pos = app.world().get::<SimPosition>(star_ent).unwrap().0;
    let end_r = end_pos.length();

    // The star must stay in a stable orbit around 120 AU and NOT plunge into the central black hole (r < 10 AU)
    assert!(
        end_r > 90.0 && end_r < 150.0,
        "Orbiting star must remain stably bound around SMBH (r = {:.2} AU)",
        end_r
    );
    // Check that it actively rotated along Z
    assert!(
        end_pos.z.abs() > 5.0,
        "Orbiting star must actively rotate around the central black hole in the cluster plane (z = {:.2} AU)",
        end_pos.z
    );
}
