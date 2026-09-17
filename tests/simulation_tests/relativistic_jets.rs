//! Integration tests for Relativistic Polar Jets, Doppler Beaming, and Synchrotron Emission.

use bevy::math::{DVec3, Vec3};
use bevy::prelude::*;
use protostellar::rendering::bodies::jets::{
    sync_relativistic_jets, RelativisticJetPart, RelativisticJetRoot,
};
use protostellar::rendering::bodies::VisualAssets;
use protostellar::rendering::materials::RelativisticJetMaterial;
use protostellar::simulation::components::*;
use protostellar::simulation::relativity::*;

#[test]
fn test_relativistic_velocity_derivation() {
    let test_cases = [
        (1.0, 0.0, 1e-3, "rest frame"),
        (2.0, 0.866_025, 1e-4, "moderate relativistic"),
        (10.0, 0.994_987, 1e-4, "relativistic pulsar"),
    ];

    for (gamma, expected_beta, tolerance, desc) in test_cases {
        let beta = calculate_relativistic_velocity(gamma);
        assert!(
            (beta - expected_beta).abs() < tolerance,
            "{desc} derivation failed: beta={beta}, expected={expected_beta}"
        );
    }

    // Extreme \Gamma -> stays bounded strictly in [0.0, 1.0)
    let beta_extreme = calculate_relativistic_velocity(10_000.0);
    assert!(beta_extreme > 0.999_99 && beta_extreme < 1.0);
}

#[test]
fn test_beaming_half_angle() {
    // \theta_beam = 1 / \Gamma
    let beam_8_5 = calculate_beaming_half_angle(8.5);
    assert!((beam_8_5 - (1.0 / 8.5)).abs() < 1e-5);
    let deg_8_5 = beam_8_5.to_degrees();
    assert!((deg_8_5 - 6.74).abs() < 0.1);

    let beam_25 = calculate_beaming_half_angle(25.0);
    assert!((beam_25 - 0.04).abs() < 1e-5);
    assert!(beam_25 < beam_8_5);
}

#[test]
fn test_doppler_factor_and_boosting() {
    let gamma = 10.0;

    // On-axis observer (\cos\theta = 1.0) -> forward jet strongly amplified
    let delta_on_axis = calculate_doppler_factor(gamma, 1.0);
    assert!(delta_on_axis > gamma); // \delta \approx 2\Gamma \approx 20.0
    let boost_on_axis = calculate_doppler_boosting(delta_on_axis, 2.3);
    assert!(boost_on_axis > 5.0);

    // Transverse observer (\cos\theta = 0.0) -> transverse Doppler redshift
    let delta_transverse = calculate_doppler_factor(gamma, 0.0);
    assert!((delta_transverse - (1.0 / gamma)).abs() < 1e-3);

    // Receding observer (\cos\theta = -1.0) -> counter-jet heavily dimmed
    let delta_receding = calculate_doppler_factor(gamma, -1.0);
    assert!(delta_receding < 0.1);
    let boost_receding = calculate_doppler_boosting(delta_receding, 2.3);
    assert!(boost_receding <= 0.1);

    // Clamping guarantees stability
    assert!(boost_on_axis <= 20.0);
    assert!(boost_receding >= 0.05);
}

#[test]
fn test_synchrotron_emission_flux_scaling() {
    // Normal pulsar (10^12 G) at 10 GHz
    let flux_pulsar = calculate_synchrotron_flux(1.0e12, 10.0, 2.3);
    assert!((flux_pulsar - 1.0).abs() < 1e-3);

    // Magnetar (10^14 G) -> significantly higher synchrotron luminosity
    let flux_magnetar = calculate_synchrotron_flux(1.0e14, 10.0, 2.3);
    assert!(flux_magnetar > flux_pulsar * 10.0);

    // Higher frequency -> power-law spectral decline
    let flux_hf = calculate_synchrotron_flux(1.0e12, 100.0, 2.3);
    assert!(flux_hf < flux_pulsar);
}

#[test]
fn test_precessing_jet_direction() {
    let base = Vec3::Y;
    let precession_axis = Vec3::Y;
    let cone_angle = 0.20; // rad

    // Phase 0.0
    let dir_0 = calculate_precessing_jet_direction(base, precession_axis, cone_angle, 0.0);
    assert!((dir_0.length() - 1.0).abs() < 1e-4);
    assert!((dir_0.dot(base) - cone_angle.cos()).abs() < 1e-3);

    // Phase 0.5 (half-turn across cone)
    let dir_half = calculate_precessing_jet_direction(base, precession_axis, cone_angle, 0.5);
    assert!((dir_half.length() - 1.0).abs() < 1e-4);
    assert!((dir_half.dot(base) - cone_angle.cos()).abs() < 1e-3);
    assert!(dir_0.dot(dir_half) < 0.99); // Turned around cone
}

#[test]
fn test_relativistic_jet_state_presets() {
    let pulsar_jet = RelativisticJetState::pulsar(0.00622);
    assert!((pulsar_jet.lorentz_factor - 8.5).abs() < 1e-5);
    assert!(pulsar_jet.opening_angle_rad > 0.05);
    assert!((pulsar_jet.jet_length_au - 3.2).abs() < 1e-5);
    assert!(pulsar_jet.synchrotron_luminosity > 0.0);

    let magnetar_jet = RelativisticJetState::magnetar();
    assert!((magnetar_jet.lorentz_factor - 12.0).abs() < 1e-5);
    assert!(magnetar_jet.synchrotron_luminosity > pulsar_jet.synchrotron_luminosity);

    let bh_jet = RelativisticJetState::black_hole(10.0);
    assert!((bh_jet.lorentz_factor - 15.0).abs() < 1e-5);
    assert!((bh_jet.jet_length_au - 8.0).abs() < 1e-5);

    let smbh_jet = RelativisticJetState::black_hole(500.0);
    assert!((smbh_jet.jet_length_au - 35.0).abs() < 1e-5);

    let quasi_jet = RelativisticJetState::quasi_star();
    assert!((quasi_jet.lorentz_factor - 18.0).abs() < 1e-5);
    assert!((quasi_jet.jet_length_au - 45.0).abs() < 1e-5);
}

#[test]
fn test_relativistic_jet_ecs_synchronization() {
    let mut app = App::new();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<RelativisticJetMaterial>>();

    // Mock VisualAssets
    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let dummy_mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));
    let visual_assets = VisualAssets {
        star_mesh: dummy_mesh.clone(),
        planet_mesh: dummy_mesh.clone(),
        atmosphere_mesh: dummy_mesh.clone(),
        asteroid_potato_mesh: dummy_mesh.clone(),
        asteroid_rubble_mesh: dummy_mesh.clone(),
        comet_bilobate_mesh: dummy_mesh.clone(),
        particle_mesh: dummy_mesh.clone(),
        ring_mesh: dummy_mesh.clone(),
        beam_core_mesh: dummy_mesh.clone(),
        beam_sheath_mesh: dummy_mesh.clone(),
        accretion_disk_mesh: dummy_mesh.clone(),
        pulsar_beam_mesh: dummy_mesh.clone(),
        magnetar_ring_mesh: dummy_mesh.clone(),
        magnetar_field_loops_mesh: dummy_mesh.clone(),
    };
    app.insert_resource(visual_assets);

    // Spawn compact object (Pulsar) without explicit RelativisticJetState
    let pulsar = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Pulsar,
                name: "PSR B1257+12".to_string(),
            },
            SimPosition(DVec3::new(5.0, 0.0, 0.0)),
            Mass(1.4),
            SpinState {
                rotation_period_hours: 0.00622 / 3600.0,
                axial_tilt_degrees: 15.0,
                spin_vector: DVec3::Y,
            },
            CentralStar,
        ))
        .id();

    app.add_systems(Update, sync_relativistic_jets);
    app.update();

    // 1. Verify RelativisticJetState auto-insertion
    let jet_state = app
        .world()
        .get::<RelativisticJetState>(pulsar)
        .expect("Pulsar must automatically receive RelativisticJetState");
    assert!((jet_state.lorentz_factor - 8.5).abs() < 1e-5);

    // Run next frame to sync 3D hierarchy for active jet source
    app.update();

    // 2. Verify RelativisticJetRoot hierarchy spawned
    let child_entities: Vec<Entity>;
    {
        let mut root_query = app
            .world_mut()
            .query::<(Entity, &Transform, &RelativisticJetRoot, &Children)>();
        let (_root_ent, root_trans, root, children) = root_query
            .iter(app.world())
            .next()
            .expect("RelativisticJetRoot must be spawned");
        assert_eq!(root.target, pulsar);
        assert_eq!(root_trans.translation, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(children.len(), 2);
        child_entities = children.iter().collect();
    }

    // 3. Verify NorthJet and SouthJet children
    let mut north_found = false;
    let mut south_found = false;
    let mut part_query = app.world_mut().query::<(
        &RelativisticJetPart,
        &MeshMaterial3d<RelativisticJetMaterial>,
    )>();

    for child in child_entities {
        let (part, mat_handle) = part_query
            .get(app.world(), child)
            .expect("Child must have RelativisticJetPart and Material");
        match part {
            RelativisticJetPart::NorthJet => north_found = true,
            RelativisticJetPart::SouthJet => south_found = true,
        }
        let materials = app.world().resource::<Assets<RelativisticJetMaterial>>();
        let mat = materials.get(&mat_handle.0).expect("Material must exist");
        assert!((mat.uniforms.jet_params.y - 8.5).abs() < 1e-5); // gamma
        assert!(mat.uniforms.jet_origin_and_doppler.w > 0.5); // Doppler enabled
    }

    assert!(north_found && south_found);

    // 4. Verify movement sync
    if let Some(mut pos) = app.world_mut().get_mut::<SimPosition>(pulsar) {
        pos.0 = DVec3::new(12.0, 3.0, -4.0);
    }
    app.update();

    let (root_trans_updated, _) = app
        .world_mut()
        .query::<(&Transform, &RelativisticJetRoot)>()
        .iter(app.world())
        .next()
        .unwrap();
    assert_eq!(root_trans_updated.translation, Vec3::new(12.0, 3.0, -4.0));

    // 5. Verify despawn cleanup when target is removed
    app.world_mut().entity_mut(pulsar).despawn();
    app.update();

    let root_count = app
        .world_mut()
        .query::<&RelativisticJetRoot>()
        .iter(app.world())
        .count();
    assert_eq!(
        root_count, 0,
        "Root must be despawned when target is despawned"
    );
}
