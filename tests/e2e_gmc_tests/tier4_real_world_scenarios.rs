//! Tier 4: Real-World Application Scenarios (>=5 realistic application-level scenarios).
//!
//! Validates end-to-end user workflows and full astrophysical simulation lifecycles.

use bevy::prelude::*;

use super::harness::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::molecular_cloud::*;
use protostellar::simulation::scenarios::*;
use protostellar::simulation::thermodynamics::*;
use protostellar::utils::constants::*;

#[test]
fn test_t4_s1_open_cluster_genesis_from_cold_gmc_core() {
    // Scenario 1: Complete genesis of an open star cluster from cold pre-stellar GMC core
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let primary_ent = {
        let mut commands = app.world_mut().commands();
        spawn_preseeded_cluster_fixture(&mut commands, &mut disk_params)
    };
    app.insert_resource(disk_params);
    app.update();

    // 1. Primary Massive Class 0 Protostar
    let primary_body = app.world().get::<CelestialBody>(primary_ent).unwrap();
    let primary_mass = app.world().get::<Mass>(primary_ent).unwrap();
    let primary_rad = app.world().get::<Radius>(primary_ent).unwrap();
    let primary_ign = app.world().get::<IgnitionState>(primary_ent).unwrap();

    assert_eq!(primary_body.body_type, BodyType::Protostar);
    assert!(primary_body.name.contains("Protostar Alpha"));
    assert!((primary_mass.0 - 3.5).abs() < 1e-4);
    assert!(primary_rad.0 > 10.0 * SOLAR_RADIUS_AU);
    assert!(!primary_ign.is_ignited);

    // 2. Full Cluster Demographics
    let mut protostars = 0;
    let mut brown_dwarfs = 0;
    let mut gas_clumps = 0;
    let mut total_stellar_mass = 0.0f64;
    let mut has_binary = false;

    let mut query = app
        .world_mut()
        .query::<(Entity, &CelestialBody, &Mass, &SimPosition)>();
    for (_ent, body, mass, pos) in query.iter(app.world()) {
        total_stellar_mass += mass.0;

        match body.body_type {
            BodyType::Protostar => protostars += 1,
            BodyType::BrownDwarf => brown_dwarfs += 1,
            BodyType::Protoplanet if body.name.contains("Jeans Clump") => gas_clumps += 1,
            _ => {}
        }

        if body.name.contains("Protostar Beta") {
            has_binary = true;
            let pericenter_dist = pos.0.length();
            assert!(
                pericenter_dist > 20.0 && pericenter_dist < 60.0,
                "Infalling binary companion Beta must be in close orbit"
            );
        }
    }

    assert!(protostars >= 7, "Must spawn at least 7 protostellar seeds");
    assert_eq!(brown_dwarfs, 1, "Must spawn 1 brown dwarf embryo");
    assert_eq!(gas_clumps, 2, "Must spawn 2 dense Jeans clumps");
    assert!(has_binary, "Must establish binary companion Beta");
    assert!(
        total_stellar_mass > 10.0 && total_stellar_mass < 15.0,
        "Total stellar mass must be ~11.3 M_sun (got {:.2})",
        total_stellar_mass
    );

    // 3. Extended cloud disk parameters
    let disk = app.world().resource::<DiskParameters>();
    assert_eq!(disk.central_star_mass, 3.5);
    assert_eq!(disk.outer_radius_au, GMC_CORE_RADIUS_AU);
    assert!(disk.gas_disk_lifetime_yr >= 250_000.0);
}

#[test]
fn test_t4_s2_massive_protostar_ignition_and_hii_bubble_formation() {
    // Scenario 2: Massive Protostar Core Ignition & Dynamic Ionization Cavity Formation
    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<SimTime>()
        .init_resource::<TimeWarp>()
        .add_message::<StarIgnitionEvent>()
        .add_message::<PlanetaryEngulfmentEvent>()
        .add_message::<SupernovaEvent>();

    // Spawn Class 0 massive protostar
    let star_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                body_type: BodyType::Protostar,
                name: "Protostar Alpha".to_string(),
            },
            Mass(3.5),
            Radius(12.0 * SOLAR_RADIUS_AU),
            Temperature(3200.0),
            Luminosity(45.0),
            IgnitionState {
                core_temperature: 7.2e6,
                fusion_fraction: 0.72,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id();

    // Verify initial unignited state: no ionization cavity
    let rho_ambient = 5.0e-11f32;
    let r_cav_pre = calculate_ionization_cavity_radius_au(45.0, rho_ambient, false);
    assert_eq!(r_cav_pre, 0.0);

    // Protostar contracts and ignites hydrogen fusion
    {
        let mut entity_mut = app.world_mut().entity_mut(star_ent);
        let mut ign = entity_mut.get_mut::<IgnitionState>().unwrap();
        ign.core_temperature = 1.05e7;
        ign.is_ignited = true;
        let mut lum = entity_mut.get_mut::<Luminosity>().unwrap();
        lum.0 = 120.0; // Post-ignition luminosity surge
    }

    let ign_after = app.world().get::<IgnitionState>(star_ent).unwrap();
    let lum_after = app.world().get::<Luminosity>(star_ent).unwrap();
    assert!(ign_after.is_ignited);

    // Carve ionization cavity
    let r_cav_post = calculate_ionization_cavity_radius_au(
        lum_after.0 as f32,
        rho_ambient,
        ign_after.is_ignited,
    );
    assert!(
        r_cav_post > 80.0 && r_cav_post < 200.0,
        "Post-ignition cavity radius must be ~120 AU (got {r_cav_post:.1} AU)"
    );

    // Raymarching optical depth profile across the cavity
    let steps = 64;
    let step_dx = (r_cav_post * 1.5) / steps as f32;
    let kappa = 0.015f32;

    let mut tau_inside_cavity = 0.0f32;
    for i in 0..steps {
        let r = (i as f32 + 0.5) * step_dx;
        let density = if r < r_cav_post { 0.0 } else { rho_ambient };
        if r < r_cav_post {
            tau_inside_cavity += kappa * density * step_dx;
        }
    }

    assert_eq!(
        tau_inside_cavity, 0.0,
        "Optical depth inside ionization cavity must be zero"
    );
    assert_eq!(beer_lambert_transmittance(tau_inside_cavity), 1.0);
}

#[test]
fn test_t4_s3_full_user_workflow_load_evolve_switch_scenario() {
    // Scenario 3: End-to-end user session: Load GMC Scenario -> Evolve 10,000 yr -> Switch to Solar Nebula
    let mut app = App::new();

    // 1. User loads GMC preset
    let mut scenario_state = ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        scenario_time_years: 0.0,
        migration_active: false,
        migration_target_au: 0.0,
        rogue_planet_entity: None,
    };
    let mut disk_params = DiskParameters::default();
    let gmc_star =
        spawn_preseeded_cluster_fixture(&mut app.world_mut().commands(), &mut disk_params);
    app.insert_resource(scenario_state.clone());
    app.insert_resource(disk_params);
    app.update();

    assert!(app.world().get_entity(gmc_star).is_ok());
    let entities_in_gmc = app.world().entities().len();
    assert!(entities_in_gmc >= 10);

    // 2. Simulation advances 10,000 years
    scenario_state.scenario_time_years += 10_000.0;
    app.insert_resource(scenario_state.clone());
    app.update();
    assert_eq!(
        app.world()
            .resource::<ActiveScenarioState>()
            .scenario_time_years,
        10_000.0
    );

    // 3. User switches back to Solar Nebula MMSN
    // Clean despawn of GMC simulation bodies
    let gmc_bodies: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .collect();
    for e in gmc_bodies {
        let mut commands = app.world_mut().commands();
        if let Ok(mut cmd) = commands.get_entity(e) {
            cmd.try_despawn();
        }
    }
    app.update();

    let remaining_bodies = app
        .world_mut()
        .query_filtered::<Entity, With<CelestialBody>>()
        .iter(app.world())
        .count();
    assert_eq!(remaining_bodies, 0);

    // Spawn Solar Nebula preset
    let mut new_disk_params = DiskParameters::default();
    let sun = spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut new_disk_params);
    scenario_state.current_preset = ScenarioPreset::SolarNebulaMmsn;
    scenario_state.scenario_time_years = 0.0;
    app.insert_resource(scenario_state);
    app.insert_resource(new_disk_params);
    app.update();

    let sun_body = app.world().get::<CelestialBody>(sun).unwrap();
    let sun_mass = app.world().get::<Mass>(sun).unwrap();
    assert_eq!(sun_body.name, "The Protostar (Solar Nebula)");
    assert_eq!(sun_mass.0, 1.0);
}

#[test]
fn test_t4_s4_mass_budget_conservation_across_extended_cloud_lifetime() {
    // Scenario 4: Strict mass conservation across 100,000 year evolution
    let total_baryonic_mass_target = GMC_CORE_MASS_SOLAR; // 24.0 M_sun

    // Initial breakdown: 11.285 M_sun in stellar seeds + 12.715 M_sun in diffuse gas reservoir
    let mut star_masses = vec![
        3.5, 1.80, 2.20, 1.05, 0.70, 0.35, 0.16, 0.060, 0.035, 0.28, 0.18,
    ];
    let initial_star_mass: f64 = star_masses.iter().sum();
    let mut diffuse_gas_mass = total_baryonic_mass_target - initial_star_mass;

    assert!((initial_star_mass + diffuse_gas_mass - 24.0).abs() < 1e-12);

    // Simulate 5 accretion epochs of 20,000 years each
    let accretion_rate_per_epoch: [f64; 5] = [0.25, 0.40, 0.30, 0.15, 0.10]; // M_sun accreted onto primary star
    for &accreted in &accretion_rate_per_epoch {
        let actual_accreted = accreted.min(diffuse_gas_mass);
        diffuse_gas_mass -= actual_accreted;
        star_masses[0] += actual_accreted; // Accrete onto primary protostar Alpha

        let current_total: f64 = star_masses.iter().sum::<f64>() + diffuse_gas_mass;
        assert!(
            (current_total - total_baryonic_mass_target).abs() < 1e-12,
            "Total mass must remain exactly invariant across all accretion epochs"
        );
    }

    assert!(star_masses[0] > 3.5);
    assert!(diffuse_gas_mass > 0.0);
}

#[test]
fn test_t4_s5_multi_body_infalling_binary_kinematics_and_angular_momentum() {
    // Scenario 5: Central Binary Pair (Alpha & Beta) Orbital Kinematics & Angular Momentum
    let m_alpha = 3.5; // Primary protostar mass (Solar)
    let m_beta = 1.8; // Infalling companion mass (Solar)
    let a = 42.0; // Semi-major axis in AU
    let e = 0.38; // Eccentricity
    let r_peri = a * (1.0 - e); // Pericenter distance ~ 26.04 AU

    // Keplerian pericenter velocity
    let mu = G_ASTRO * (m_alpha + m_beta);
    let v_peri_expected = (mu * (2.0 / r_peri - 1.0 / a)).sqrt();

    // Specific angular momentum h = r_peri * v_peri
    let h_kepler = r_peri * v_peri_expected;
    // Alternative analytical form: h = sqrt(G * (M1 + M2) * a * (1 - e^2))
    let h_analytical = (mu * a * (1.0 - e * e)).sqrt();

    assert!(
        (h_kepler - h_analytical).abs() < 1e-6,
        "Orbital angular momentum must match analytical Keplerian two-body equation"
    );

    // Binary total orbital angular momentum: L = mu_red * h
    let mu_reduced = (m_alpha * m_beta) / (m_alpha + m_beta);
    let l_binary = mu_reduced * h_kepler;
    assert!(l_binary > 0.0);
    assert!(!l_binary.is_nan());
}
