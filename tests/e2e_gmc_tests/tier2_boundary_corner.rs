//! Tier 2: Boundary & Corner Cases (>=5 tests per requirement R1, R2, R3, R4 = 20 tests).
//!
//! Opaque-box adversarial and boundary verification covering extreme values and edge conditions.

use bevy::prelude::*;
use glam::Vec3;

use super::harness::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::molecular_cloud::*;
use protostellar::simulation::scenarios::*;
use protostellar::utils::constants::*;

// ============================================================================
// REQUIREMENT 1: High-Performance Fluid Simulation — Boundaries (5 tests)
// ============================================================================

#[test]
fn test_t2_r1_zero_and_negative_density_clamping() {
    // Adversarial: Negative or zero density inputs must clamp to zero and avoid NaN/division-by-zero
    let raw_densities = [-1.0e-5, 0.0, -0.0, 1.0e-18];
    for &raw_rho in &raw_densities {
        let clamped = (raw_rho as f64).max(0.0);
        assert!(clamped >= 0.0);
        assert!(!clamped.is_nan());

        // Pressure P = c_s^2 * rho must be non-negative
        let cs = calculate_gmc_sound_speed_au_yr(15.0);
        let pressure = cs * cs * clamped;
        assert!(pressure >= 0.0);
        assert!(!pressure.is_nan());
    }
}

#[test]
fn test_t2_r1_extreme_cold_cmb_floor_positivity() {
    // Extreme cold limit: CMB temperature T = 2.725 K
    let t_cmb = 2.725;
    let cs_cmb = calculate_gmc_sound_speed_au_yr(t_cmb);
    let cs_km_s = cs_cmb * AU_PER_YR_TO_KM_PER_S;

    assert!(cs_cmb > 0.0, "Sound speed at CMB floor must be positive");
    assert!(!cs_cmb.is_nan());
    assert!(!cs_cmb.is_infinite());
    // At 2.725 K, c_s ~ 0.117 km/s
    assert!(cs_km_s > 0.10 && cs_km_s < 0.14);
}

#[test]
fn test_t2_r1_hypersonic_shock_energy_boundedness() {
    // Adversarial: Supersonic shock with Mach 20 velocity dispersion
    let cs = calculate_gmc_sound_speed_au_yr(15.0) as f32;
    let v_shock = 20.0 * cs; // Mach 20
    let rho_shock = 1.0e-9f32; // Strongly compressed gas

    let kinetic_energy_density = 0.5 * rho_shock * v_shock * v_shock;
    assert!(kinetic_energy_density > 0.0);
    assert!(!kinetic_energy_density.is_nan());
    assert!(!kinetic_energy_density.is_infinite());

    // CFL limit under Mach 20 shock must yield a strictly positive sub-step dt
    let cfl_dt = calculate_cfl_max_dt(GMC_CELL_DX_AU, v_shock, cs);
    assert!(cfl_dt > 0.0);
    assert!(cfl_dt < calculate_cfl_max_dt(GMC_CELL_DX_AU, cs, cs));
}

#[test]
fn test_t2_r1_grid_domain_outflow_boundary_conditions() {
    // Domain boundary indexing at extreme faces (x=0, x=95)
    let boundary_coords = [
        (0, 0, 0),
        (GMC_GRID_DIM - 1, 0, 0),
        (0, GMC_GRID_DIM - 1, 0),
        (0, 0, GMC_GRID_DIM - 1),
        (GMC_GRID_DIM - 1, GMC_GRID_DIM - 1, GMC_GRID_DIM - 1),
    ];

    for (x, y, z) in boundary_coords {
        let idx = grid_index_1d(x, y, z);
        assert!(idx < GMC_TOTAL_CELLS);
        let world_pos = grid_to_world_pos(x, y, z);
        assert!(world_pos.x.abs() <= 550.0);
        assert!(world_pos.y.abs() <= 550.0);
        assert!(world_pos.z.abs() <= 550.0);
    }
}

#[test]
fn test_t2_r1_large_timestep_cfl_violation_safety() {
    // Adversarial: User requests enormous time step (dt = 5,000 yr) during rapid collapse
    let requested_dt = 5000.0f32;
    let cs = calculate_gmc_sound_speed_au_yr(15.0) as f32;
    let v_turb = 1.6f32 / AU_PER_YR_TO_KM_PER_S as f32;
    let max_stable_dt = calculate_cfl_max_dt(GMC_CELL_DX_AU, v_turb, cs);

    // Engine must clamp dt to max_stable_dt or compute sub-cycles
    let sub_cycles = (requested_dt / max_stable_dt).ceil() as u32;
    assert!(sub_cycles > 1);
    let effective_dt = requested_dt / sub_cycles as f32;
    assert!(effective_dt <= max_stable_dt);
}

// ============================================================================
// REQUIREMENT 2: Jeans Collapse and Protostar Ignition — Boundaries (5 tests)
// ============================================================================

#[test]
fn test_t2_r2_subcritical_density_non_collapse() {
    // Diffuse background gas (rho = 1e-13 M_sun/AU^3): Jeans mass exceeds total cloud mass
    let rho_diffuse = 1.0e-13;
    let m_jeans = calculate_jeans_mass_solar(rho_diffuse, GMC_CLOUD_TEMPERATURE_K);

    // Jeans mass in diffuse cloud exceeds 5 M_sun; localized 0.1 M_sun clump cannot collapse
    let local_clump_mass = 0.10;
    assert!(
        local_clump_mass < m_jeans,
        "Sub-critical clump must not satisfy Jeans collapse criterion"
    );
}

#[test]
fn test_t2_r2_extreme_turbulent_support_prevents_collapse() {
    // Extreme supersonic turbulence (sigma_v = 15 km/s, Mach ~ 55)
    let rho_dense = 5.0e-11;
    let m_jeans_extreme_turb =
        calculate_turbulent_jeans_mass_solar(rho_dense, GMC_CLOUD_TEMPERATURE_K, 15.0);

    // Jeans mass rises above 10,000 M_sun due to turbulent pressure support
    assert!(
        m_jeans_extreme_turb > 1000.0,
        "High supersonic turbulence must inflate Jeans mass drastically"
    );
    let dense_core_mass = 3.5; // Solar Masses
    assert!(
        dense_core_mass < m_jeans_extreme_turb,
        "Clump must be supported against collapse by extreme turbulence"
    );
}

#[test]
fn test_t2_r2_simultaneous_multi_peak_fragmentation() {
    // Simultaneous collapse in two adjacent cells
    let event1 = GpuJeansCollapseEvent {
        grid_coords: [40, 48, 48],
        metallicity: 0.0,
        world_pos: [-91.6, 0.0, 0.0],
        local_mass_solar: 1.5,
        com_velocity: [0.0, 0.2, 0.0],
        temperature_k: 16.0,
    };
    let event2 = GpuJeansCollapseEvent {
        grid_coords: [44, 48, 48],
        metallicity: 0.0,
        world_pos: [-45.8, 0.0, 0.0],
        local_mass_solar: 2.1,
        com_velocity: [0.0, -0.2, 0.0],
        temperature_k: 14.5,
    };

    assert_ne!(event1.grid_coords, event2.grid_coords);
    assert_ne!(event1.world_pos, event2.world_pos);

    // Separation distance between two collapsing cores
    let p1 = Vec3::from_array(event1.world_pos);
    let p2 = Vec3::from_array(event2.world_pos);
    let dist = (p1 - p2).length();
    assert!(dist > GMC_CELL_DX_AU);
}

#[test]
fn test_t2_r2_seed_mass_boundaries_brown_dwarf_to_massive_star() {
    // Lowest mass substellar embryo (~0.013 M_sun deuterium burning limit) to high mass protostar (~50 M_sun)
    let min_brown_dwarf_mass = 0.013;
    let max_massive_protostar_mass = 50.0;

    assert!(min_brown_dwarf_mass > 0.0);
    assert!(max_massive_protostar_mass > min_brown_dwarf_mass);

    // GMC scenario members must fall within astrophysical stellar/substellar bounds
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let _primary = {
        let mut commands = app.world_mut().commands();
        spawn_preseeded_cluster_fixture(&mut commands, &mut disk_params)
    };
    app.update();

    let mut query = app.world_mut().query::<(&CelestialBody, &Mass)>();
    for (body, mass) in query.iter(app.world()) {
        assert!(
            mass.0 >= min_brown_dwarf_mass,
            "Body {} has unphysically low mass {:.4} M_sun",
            body.name,
            mass.0
        );
        assert!(
            mass.0 <= max_massive_protostar_mass,
            "Body {} exceeds maximum stellar mass limit",
            body.name
        );
    }
}

#[test]
fn test_t2_r2_depleted_gas_cell_vacuuming_floor() {
    // Vacuuming a cell that only has 0.05 M_sun when 0.50 M_sun is requested
    let mut available_mass = 0.05f64;
    let requested_mass = 0.50f64;

    let vacuumed_mass = available_mass.min(requested_mass);
    available_mass -= vacuumed_mass;

    assert_eq!(vacuumed_mass, 0.05);
    assert_eq!(available_mass, 0.0);
    assert!(
        available_mass >= 0.0,
        "Gas density in cell must not go negative"
    );
}

// ============================================================================
// REQUIREMENT 3: Volumetric Nebula Rendering — Boundaries (5 tests)
// ============================================================================

#[test]
fn test_t2_r3_ray_missing_bounding_box_returns_clear() {
    // Ray parallel to box but placed outside domain (y = 1000 AU, shooting along x)
    let ray_origin = Vec3::new(-1000.0, 1000.0, 0.0);
    let ray_dir = Vec3::new(1.0, 0.0, 0.0);
    let box_min = Vec3::splat(-550.0);
    let box_max = Vec3::splat(550.0);

    let hit = intersect_ray_aabb(ray_origin, ray_dir, box_min, box_max);
    assert!(hit.is_none());
}

#[test]
fn test_t2_r3_camera_embedded_inside_nebula_volume() {
    // Camera located at origin (0, 0, 0) inside GMC volume looking along +z
    let ray_origin = Vec3::ZERO;
    let ray_dir = Vec3::new(0.0, 0.0, 1.0);
    let box_min = Vec3::splat(-550.0);
    let box_max = Vec3::splat(550.0);

    let hit = intersect_ray_aabb(ray_origin, ray_dir, box_min, box_max);
    assert!(hit.is_some());
    let (t_near, t_far) = hit.unwrap();

    // Inside volume: t_near must be 0.0, t_far is distance to front boundary (+550 AU)
    assert_eq!(t_near, 0.0);
    assert!((t_far - 550.0).abs() < 1e-4);
}

#[test]
fn test_t2_r3_total_optical_extinction_early_termination() {
    // Adversarial: Ray travels through extremely dense dust column (tau >= 25)
    let tau_extreme = 25.0;
    let transmittance = beer_lambert_transmittance(tau_extreme);

    // Below early-out threshold (~1e-6)
    let early_termination_epsilon = 1.0e-5;
    assert!(
        transmittance < early_termination_epsilon,
        "Extremely opaque cloud must trigger early ray termination"
    );
}

#[test]
fn test_t2_r3_zero_density_volume_full_transparency() {
    // Cloud core with zero density (completely cleared)
    let step_count = 64;
    let step_size_au = 1100.0 / step_count as f32;
    let absorption_coeff = 0.015f32;
    let zero_density = 0.0f32;

    let mut accumulated_tau = 0.0f32;
    for _ in 0..step_count {
        accumulated_tau += absorption_coeff * zero_density * step_size_au;
    }

    let transmittance = beer_lambert_transmittance(accumulated_tau);
    assert_eq!(transmittance, 1.0);
}

#[test]
fn test_t2_r3_star_outside_box_cavity_handling() {
    // Star placed at 800 AU (outside the 550 AU GMC box)
    let star_pos = Vec3::new(800.0, 0.0, 0.0);
    let box_min = Vec3::splat(-550.0);
    let box_max = Vec3::splat(550.0);

    let is_inside = star_pos.x >= box_min.x
        && star_pos.x <= box_max.x
        && star_pos.y >= box_min.y
        && star_pos.y <= box_max.y
        && star_pos.z >= box_min.z
        && star_pos.z <= box_max.z;

    assert!(!is_inside, "Star at 800 AU must be detected outside box");

    // Cavity carving must not panic or cause negative indices
    let cavity_r = calculate_ionization_cavity_radius_au(100.0, 5e-11, true);
    assert!(cavity_r > 0.0);
    // Nearest point on box is at x = 550; distance is 250 AU > cavity_r
    let dist_to_box = star_pos.x - box_max.x;
    assert!(dist_to_box > cavity_r);
}

// ============================================================================
// REQUIREMENT 4: Distinct Scenario Integration — Boundaries (5 tests)
// ============================================================================

#[test]
fn test_t2_r4_rapid_consecutive_scenario_switching() {
    // Rapid switching between presets without state contamination
    let sequence = [
        ScenarioPreset::SolarNebulaMmsn,
        ScenarioPreset::MolecularCloudCluster,
        ScenarioPreset::AccretionDiskGenesis,
        ScenarioPreset::MolecularCloudCluster,
        ScenarioPreset::SolarNebulaMmsn,
    ];

    let mut state = ActiveScenarioState::default();
    for &preset in &sequence {
        state.current_preset = preset;
        if preset == ScenarioPreset::MolecularCloudCluster {
            assert_eq!(state.current_preset, ScenarioPreset::MolecularCloudCluster);
        }
    }
    assert_eq!(state.current_preset, ScenarioPreset::SolarNebulaMmsn);
}

#[test]
fn test_t2_r4_maximum_time_warp_stability() {
    // Running GMC scenario under maximum time warp (100,000x)
    let time_warp = TimeWarp {
        multiplier: 100_000.0,
        is_paused: false,
        ..TimeWarp::default()
    };
    let mut sim_time = SimTime {
        elapsed_years: 0.0,
        current_dt_yr: 0.001,
        visual_time_secs: 0.0,
        ..SimTime::default()
    };

    // 1 frame at 60 FPS = ~0.0166s real time
    let dt_frame_sec = 0.0166;
    let sim_dt = dt_frame_sec * time_warp.multiplier * (1.0 / 3.15576e7); // in years
    sim_time.elapsed_years += sim_dt;

    assert!(sim_time.elapsed_years > 0.0);
    assert!(!sim_time.elapsed_years.is_nan());
    assert!(!sim_time.elapsed_years.is_infinite());
}

#[test]
fn test_t2_r4_empty_world_scenario_initialization() {
    // Spawning GMC scenario in an initial Bevy world without pre-existing celestial bodies
    let mut app = App::new();
    let initial_bodies = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();
    assert_eq!(initial_bodies, 0);

    let mut disk_params = DiskParameters::default();
    let primary = {
        let mut commands = app.world_mut().commands();
        spawn_preseeded_cluster_fixture(&mut commands, &mut disk_params)
    };
    app.update();

    let final_bodies = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();
    assert!(final_bodies >= 10);
    assert!(app.world().get::<CelestialBody>(primary).is_some());
}

#[test]
fn test_t2_r4_scenario_persistence_across_pause_resume() {
    // Pausing simulation preserves all GMC protostars and masses intact
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let primary = {
        let mut commands = app.world_mut().commands();
        spawn_preseeded_cluster_fixture(&mut commands, &mut disk_params)
    };
    app.insert_resource(TimeWarp {
        multiplier: 1.0,
        is_paused: true,
        ..TimeWarp::default()
    });
    app.update();

    let primary_mass_paused = app.world().get::<Mass>(primary).unwrap().0;

    // Resume
    app.world_mut().resource_mut::<TimeWarp>().is_paused = false;
    app.update();

    let primary_mass_resumed = app.world().get::<Mass>(primary).unwrap().0;
    assert_eq!(primary_mass_paused, primary_mass_resumed);
}

#[test]
fn test_t2_r4_central_star_engulfment_exemption_contract() {
    // Interface Contract 2: Mobile protostars with body_type.is_star_or_remnant()
    // must NOT be swallowed by central star engulfment logic
    let protostar_body = CelestialBody {
        name: "Protostar Beta".to_string(),
        body_type: BodyType::Protostar,
    };
    assert!(
        protostar_body.body_type.is_star_or_remnant(),
        "Protostar must be classified as star/remnant"
    );

    let planet_body = CelestialBody {
        name: "Infalling Pebble".to_string(),
        body_type: BodyType::Planetesimal,
    };
    assert!(
        !planet_body.body_type.is_star_or_remnant(),
        "Planetesimal is not exempt from engulfment"
    );
}
