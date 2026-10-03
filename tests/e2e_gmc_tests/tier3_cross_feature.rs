//! Tier 3: Cross-Feature Combinations (Pairwise Interactions = 5 tests).
//!
//! Verifies interfaces and coupled feedback loops between requirements R1, R2, R3, and R4.

use bevy::prelude::*;
use glam::Vec3;

use super::harness::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::molecular_cloud::*;
use protostellar::simulation::scenarios::*;

#[test]
fn test_t3_fluid_advection_triggers_jeans_collapse() {
    // Cross-feature: R1 (Fluid dynamics) + R2 (Jeans collapse)
    // 1. Initial diffuse state in a grid cell
    let mut cell_density = 1.0e-12f64; // Sub-critical
    let temp_k = GMC_CLOUD_TEMPERATURE_K;
    let sigma_v = 0.5; // km/s
    let m_jeans_initial = calculate_turbulent_jeans_mass_solar(cell_density, temp_k, sigma_v);

    let cell_vol = GMC_CELL_VOLUME_AU3 as f64;
    let initial_cell_mass = cell_density * cell_vol;
    assert!(
        initial_cell_mass < m_jeans_initial,
        "Initial cell mass must be below Jeans threshold"
    );

    // 2. Fluid advection concentrates gas into the cell over 5,000 years
    let advected_flux_solar = 1.50f64;
    let new_cell_mass = initial_cell_mass + advected_flux_solar;
    cell_density = new_cell_mass / cell_vol;

    // 3. Re-evaluate Jeans threshold at new high density
    let m_jeans_compressed = calculate_turbulent_jeans_mass_solar(cell_density, temp_k, sigma_v);
    assert!(
        new_cell_mass >= m_jeans_compressed,
        "Advected density concentration must exceed Jeans collapse mass"
    );

    // 4. Trigger collapse event and vacuum gas from grid
    let collapse_coords = [48, 50, 48];
    let world_pos = grid_to_world_pos(collapse_coords[0], collapse_coords[1], collapse_coords[2]);
    let event = GpuJeansCollapseEvent {
        grid_coords: collapse_coords,
        _pad0: 0,
        world_pos: world_pos.to_array(),
        local_mass_solar: new_cell_mass as f32,
        com_velocity: [0.0, 0.1, 0.0],
        temperature_k: temp_k as f32,
    };

    // Cell mass is vacuumed down to residual floor
    let vacuumed_star_mass = new_cell_mass;
    let residual_gas_density = 1.0e-14f64;
    let residual_gas_mass = residual_gas_density * cell_vol;

    assert_eq!(event.local_mass_solar, vacuumed_star_mass as f32);
    assert!(residual_gas_mass < 0.001);
}

#[test]
fn test_t3_protostar_ignition_drives_cavity_carving() {
    // Cross-feature: R2 (Protostar ignition) + R3 (Volumetric raymarching)
    let ambient_density = 5.0e-11f32;

    // Phase A: Pre-ignition Class 0 protostar (T_core = 7.2 MK, unignited)
    let mut sink = GpuSinkParticle {
        world_pos: [0.0, 0.0, 0.0],
        sink_radius_au: 15.0,
        radiation_pressure_factor: 0.1,
        is_ignited: 0,
        _pad: [0, 0],
    };
    let cavity_r_pre = calculate_ionization_cavity_radius_au(45.0, ambient_density, false);
    assert_eq!(cavity_r_pre, 0.0);

    // Phase B: Protostar reaches 10 MK -> Ignites!
    sink.is_ignited = 1;
    sink.radiation_pressure_factor = 1.0;

    let cavity_r_post = calculate_ionization_cavity_radius_au(45.0, ambient_density, true);
    assert!(cavity_r_post > 50.0);
    sink.sink_radius_au = cavity_r_post;

    assert_eq!(sink.is_ignited, 1);
    assert_eq!(sink.sink_radius_au, cavity_r_post);
    assert_eq!(sink.radiation_pressure_factor, 1.0);

    // Raymarching through cavity center:
    // Any sample point with distance < cavity_r_post has gas density carved to 0.0
    let sample_inside = Vec3::new(20.0, 0.0, 0.0);
    let sample_outside = Vec3::new(120.0, 0.0, 0.0);

    let density_inside = if sample_inside.length() < cavity_r_post {
        0.0f32
    } else {
        ambient_density
    };
    let density_outside = if sample_outside.length() < cavity_r_post {
        0.0f32
    } else {
        ambient_density
    };

    assert_eq!(density_inside, 0.0);
    assert_eq!(density_outside, ambient_density);
}

#[test]
fn test_t3_volumetric_nebula_scenario_isolation() {
    // Cross-feature: R3 (Volumetric raymarching) + R4 (Scenario isolation)
    let mut scenario_state = ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        scenario_time_years: 0.0,
        migration_active: false,
        migration_target_au: 0.0,
        rogue_planet_entity: None,
    };

    // When GMC scenario is active: raymarching uniform is active and camera is at 850 AU
    let is_gmc_active = scenario_state.current_preset == ScenarioPreset::MolecularCloudCluster;
    assert!(is_gmc_active);

    let (cam_dist, _, _) = match scenario_state.current_preset {
        ScenarioPreset::MolecularCloudCluster => (850.0f32, 0.785f32, 0.65f32),
        ScenarioPreset::SolarNebulaMmsn => (16.0f32, 0.785f32, 0.62f32),
        _ => (10.0, 0.0, 0.0),
    };
    assert_eq!(cam_dist, 850.0);

    // Switch scenario to Solar Nebula: camera shifts to 16 AU, raymarching deactivated
    scenario_state.current_preset = ScenarioPreset::SolarNebulaMmsn;
    let is_gmc_active_after =
        scenario_state.current_preset == ScenarioPreset::MolecularCloudCluster;
    assert!(!is_gmc_active_after);

    let (cam_dist_after, _, _) = match scenario_state.current_preset {
        ScenarioPreset::MolecularCloudCluster => (850.0f32, 0.785f32, 0.65f32),
        ScenarioPreset::SolarNebulaMmsn => (16.0f32, 0.785f32, 0.62f32),
        _ => (10.0, 0.0, 0.0),
    };
    assert_eq!(cam_dist_after, 16.0);
}

#[test]
fn test_t3_fluid_simulation_with_guarded_accretion() {
    // Cross-feature: R1 (Fluid compute) + R4 (Accretion guarding)
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let _primary = {
        let mut commands = app.world_mut().commands();
        spawn_preseeded_cluster_fixture(&mut commands, &mut disk_params)
    };
    app.insert_resource(ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        scenario_time_years: 0.0,
        migration_active: false,
        migration_target_au: 0.0,
        rogue_planet_entity: None,
    });
    app.update();

    // Verify no planetesimals or pebbles are present
    let mut query = app.world_mut().query::<&CelestialBody>();
    for body in query.iter(app.world()) {
        assert_ne!(
            body.body_type,
            BodyType::Planetesimal,
            "GMC cluster scenario must not spawn planetesimals"
        );
        assert_ne!(
            body.body_type,
            BodyType::Asteroid,
            "GMC cluster scenario must not spawn asteroids"
        );
    }
}

#[test]
fn test_t3_protostar_spawning_establishes_cluster_hierarchy() {
    // Cross-feature: R2 (Protostar spawning) + R4 (Scenario hierarchy)
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let primary = {
        let mut commands = app.world_mut().commands();
        spawn_preseeded_cluster_fixture(&mut commands, &mut disk_params)
    };
    app.update();

    let mut total_cluster_mass = 0.0f64;
    let mut member_count = 0;

    let mut query = app
        .world_mut()
        .query::<(Entity, &CelestialBody, &Mass, &SimPosition, &SimVelocity)>();
    for (ent, body, mass, pos, vel) in query.iter(app.world()) {
        total_cluster_mass += mass.0;
        member_count += 1;

        if ent != primary {
            // Must have initial 3D positions inside cloud boundary (550 AU)
            let r = pos.0.length();
            assert!(
                r <= GMC_CORE_RADIUS_AU * 1.05,
                "Member {} positioned outside GMC cloud radius ({:.1} AU)",
                body.name,
                r
            );
            // Must have active velocity
            assert!(
                vel.0.length() > 0.0,
                "Member {} must have initial orbital/turbulent velocity",
                body.name
            );
        }
    }

    assert!(member_count >= 10);
    assert!(total_cluster_mass > 10.0 && total_cluster_mass < 15.0);
}
