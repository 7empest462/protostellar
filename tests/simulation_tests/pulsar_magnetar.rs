//! Tests for Pulsar and Magnetar scenarios, lifecycle structures, and mesh geometry.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;

#[test]
fn test_pulsar_scenario_preset() {
    use protostellar::simulation::scenarios::{spawn_pulsar_system_scenario, ScenarioPreset};

    assert_eq!(
        ScenarioPreset::PulsarSystem.display_name(),
        "PSR B1257+12 (Pulsar & Zombie Planets)"
    );

    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let pulsar_ent =
        spawn_pulsar_system_scenario(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let world = app.world();
    let pulsar_body = world
        .get::<CelestialBody>(pulsar_ent)
        .expect("Pulsar entity must exist");
    assert_eq!(pulsar_body.body_type, BodyType::Pulsar);
    assert!(pulsar_body.name.contains("PSR B1257+12"));

    let pulsar_mass = world
        .get::<Mass>(pulsar_ent)
        .expect("Mass component required");
    assert_eq!(pulsar_mass.0, 1.40);

    let em_field = world
        .get::<ElectromagneticFieldState>(pulsar_ent)
        .expect("EM field required");
    assert_eq!(em_field.magnetic_field_gauss, 1.0e9);
    assert!((em_field.rotation_period_sec - 0.00622).abs() < 1e-5);

    let spin = world
        .get::<SpinState>(pulsar_ent)
        .expect("SpinState required");
    assert!(spin.rotation_period_hours < 0.001);

    let mut body_count = 0;
    let mut draugr_found = false;
    let mut poltergeist_found = false;
    let mut phobetor_found = false;
    let mut query = app.world_mut().query::<&CelestialBody>();
    for body in query.iter(app.world()) {
        body_count += 1;
        if body.name.contains("Draugr") {
            draugr_found = true;
        } else if body.name.contains("Poltergeist") {
            poltergeist_found = true;
        } else if body.name.contains("Phobetor") {
            phobetor_found = true;
        }
    }
    assert_eq!(body_count, 5);
    assert!(draugr_found && poltergeist_found && phobetor_found);
}

#[test]
fn test_magnetar_scenario_preset() {
    use protostellar::simulation::scenarios::{spawn_magnetar_outburst_scenario, ScenarioPreset};

    assert_eq!(
        ScenarioPreset::MagnetarOutburst.display_name(),
        "SGR 1806-20 (Magnetar Giant Flare)"
    );

    let mut app2 = App::new();
    let mut disk_params2 = DiskParameters::default();
    let magnetar_ent =
        spawn_magnetar_outburst_scenario(&mut app2.world_mut().commands(), &mut disk_params2);
    app2.update();

    let world2 = app2.world();
    let magnetar_body = world2
        .get::<CelestialBody>(magnetar_ent)
        .expect("Magnetar entity must exist");
    assert_eq!(magnetar_body.body_type, BodyType::Magnetar);
    assert!(magnetar_body.name.contains("SGR 1806-20"));

    let magnetar_mass = world2
        .get::<Mass>(magnetar_ent)
        .expect("Mass component required");
    assert_eq!(magnetar_mass.0, 1.95);

    let magnetar_em = world2
        .get::<ElectromagneticFieldState>(magnetar_ent)
        .expect("EM field required");
    assert_eq!(magnetar_em.magnetic_field_gauss, 1.0e15);

    let mut companion_count = 0;
    let mut lbv_found = false;
    let mut query2 = app2.world_mut().query::<&CelestialBody>();
    for body in query2.iter(app2.world()) {
        companion_count += 1;
        if body.name.contains("LBV 1806-20") {
            lbv_found = true;
            assert_eq!(body.body_type, BodyType::BlueSupergiant);
        }
    }
    assert_eq!(companion_count, 5);
    assert!(lbv_found);
}

fn setup_pulsar_magnetar_app() -> App {
    use protostellar::rendering::bodies::{
        sync_magnetar_structures, sync_pulsar_beams, VisualAssets,
    };

    let mut app = App::new();
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_resource::<Time>();
    app.init_resource::<SimulationConfig>();

    let (star_mesh, cyl_mesh) = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        (
            meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            meshes.add(Cylinder::new(1.0, 1.0)),
        )
    };

    app.insert_resource(VisualAssets {
        star_mesh: star_mesh.clone(),
        planet_mesh: star_mesh.clone(),
        asteroid_potato_mesh: star_mesh.clone(),
        asteroid_rubble_mesh: star_mesh.clone(),
        comet_bilobate_mesh: star_mesh.clone(),
        particle_mesh: star_mesh.clone(),
        ring_mesh: star_mesh.clone(),
        beam_core_mesh: cyl_mesh.clone(),
        beam_sheath_mesh: cyl_mesh.clone(),
        accretion_disk_mesh: cyl_mesh.clone(),
        pulsar_beam_mesh: cyl_mesh.clone(),
        magnetar_ring_mesh: cyl_mesh.clone(),
        magnetar_field_loops_mesh: cyl_mesh.clone(),
    });

    app.add_systems(Update, (sync_pulsar_beams, sync_magnetar_structures));
    app
}

fn verify_pulsar_beam_lifecycle(app: &mut App) {
    use protostellar::rendering::bodies::{PulsarBeamPart, PulsarBeamRoot};

    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamRoot>()
            .iter(app.world())
            .count(),
        0
    );

    let pulsar_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "PSR B1257+12".to_string(),
                body_type: BodyType::Pulsar,
            },
            SimPosition(DVec3::ZERO),
            Radius(0.0001),
        ))
        .id();

    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamRoot>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamPart>()
            .iter(app.world())
            .count(),
        2
    );

    app.world_mut().despawn(pulsar_ent);
    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamRoot>()
            .iter(app.world())
            .count(),
        0
    );
}

fn verify_magnetar_structure_lifecycle(app: &mut App) {
    use protostellar::rendering::bodies::{MagnetarStructurePart, MagnetarStructureRoot};

    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        0
    );

    let clump_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "SGR Ejecta Clump α".to_string(),
                body_type: BodyType::Protoplanet,
            },
            SimPosition(DVec3::new(5.0, 0.0, 0.0)),
        ))
        .id();

    let magnetar_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "SGR 1806-20".to_string(),
                body_type: BodyType::Magnetar,
            },
            SimPosition(DVec3::ZERO),
        ))
        .id();

    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructurePart>()
            .iter(app.world())
            .count(),
        2
    );

    let mut root_q = app
        .world_mut()
        .query::<(&MagnetarStructureRoot, &Transform)>();
    let (_, root_tf) = root_q.iter(app.world()).next().unwrap();
    assert!(
        root_tf.translation.length() < 1e-4,
        "MagnetarStructureRoot must be at Magnetar (0,0,0), but was at {:?}",
        root_tf.translation
    );

    app.world_mut().despawn(magnetar_ent);
    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        0
    );

    app.world_mut().despawn(clump_ent);
    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn test_pulsar_and_magnetar_visual_structures_lifecycle() {
    let mut app = setup_pulsar_magnetar_app();
    verify_pulsar_beam_lifecycle(&mut app);
    verify_magnetar_structure_lifecycle(&mut app);
}

#[test]
fn test_magnetar_scenario_orbital_stability_and_field_attachment() {
    use protostellar::rendering::bodies::{
        sync_magnetar_structures, MagnetarStructureRoot, VisualAssets,
    };
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::scenarios::spawn_magnetar_outburst_scenario;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_resource::<Time>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.init_resource::<PlayerInteractionState>();

    let (star_mesh, cyl_mesh) = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        (
            meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            meshes.add(Cylinder::new(1.0, 1.0)),
        )
    };

    app.insert_resource(VisualAssets {
        star_mesh: star_mesh.clone(),
        planet_mesh: star_mesh.clone(),
        asteroid_potato_mesh: star_mesh.clone(),
        asteroid_rubble_mesh: star_mesh.clone(),
        comet_bilobate_mesh: star_mesh.clone(),
        particle_mesh: star_mesh.clone(),
        ring_mesh: star_mesh.clone(),
        beam_core_mesh: cyl_mesh.clone(),
        beam_sheath_mesh: cyl_mesh.clone(),
        accretion_disk_mesh: cyl_mesh.clone(),
        pulsar_beam_mesh: cyl_mesh.clone(),
        magnetar_ring_mesh: cyl_mesh.clone(),
        magnetar_field_loops_mesh: cyl_mesh.clone(),
    });

    let mut disk_params = DiskParameters::default();
    let _magnetar_ent =
        spawn_magnetar_outburst_scenario(&mut app.world_mut().commands(), &mut disk_params);

    app.add_systems(Update, (step_physics_simulation, sync_magnetar_structures));

    app.update();

    {
        let mut root_query = app
            .world_mut()
            .query::<(&MagnetarStructureRoot, &Transform)>();
        let (_, tf) = root_query
            .iter(app.world())
            .next()
            .expect("MagnetarStructureRoot must exist");
        assert!(
            tf.translation.length() < 1e-4,
            "Magnetic field loops must be anchored at the Magnetar (0,0,0), but found at {:?}",
            tf.translation
        );
    }

    app.world_mut().resource_mut::<TimeWarp>().multiplier = 10.0;
    for _ in 0..200 {
        app.update();
    }

    let mut bodies_query = app.world_mut().query::<(&CelestialBody, &SimPosition)>();
    let mut valkyrie_dist = 0.0;
    let mut pyre_dist = 0.0;
    let mut clump_dist = 0.0;
    let mut lbv_dist = 0.0;
    let mut magnetar_dist = 0.0;

    for (body, pos) in bodies_query.iter(app.world()) {
        let dist = pos.0.length();
        if body.name.contains("Magnetar") {
            magnetar_dist = dist;
        } else if body.name.contains("Valkyrie") {
            valkyrie_dist = dist;
        } else if body.name.contains("Pyre") {
            pyre_dist = dist;
        } else if body.name.contains("Clump") {
            clump_dist = dist;
        } else if body.name.contains("LBV") {
            lbv_dist = dist;
        }
    }

    assert!(
        magnetar_dist < 1e-6,
        "Magnetar must stay at center (0,0,0), found at {}",
        magnetar_dist
    );
    assert!(
        valkyrie_dist >= 0.40 && valkyrie_dist <= 0.60,
        "Valkyrie must remain in stable orbit around ~0.48 AU, found at {}",
        valkyrie_dist
    );
    assert!(
        pyre_dist >= 0.70 && pyre_dist <= 1.05,
        "Pyre must remain in stable orbit around ~0.85 AU, found at {}",
        pyre_dist
    );
    assert!(
        clump_dist >= 1.40 && clump_dist <= 1.95,
        "SGR Ejecta Clump must remain in stable orbit around ~1.65 AU, found at {}",
        clump_dist
    );
    assert!(
        lbv_dist >= 17.0 && lbv_dist <= 19.5,
        "LBV 1806-20 must remain in stable cluster orbit around ~18.0 AU, found at {}",
        lbv_dist
    );

    {
        let mut root_query2 = app
            .world_mut()
            .query::<(&MagnetarStructureRoot, &Transform)>();
        let (_, tf2) = root_query2.iter(app.world()).next().unwrap();
        assert!(
            tf2.translation.length() < 1e-4,
            "Magnetic field structures must remain centered on Magnetar, found at {:?}",
            tf2.translation
        );
    }
}

#[test]
fn test_pulsar_beam_polar_taper_and_magnetar_field_symmetry() {
    use bevy::render::mesh::VertexAttributeValues;
    use protostellar::rendering::bodies::meshes::{
        generate_magnetar_field_loops_mesh, generate_pulsar_beam_mesh,
    };

    let pulsar_mesh = generate_pulsar_beam_mesh();
    let pulsar_positions = match pulsar_mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(pos)) => pos,
        _ => panic!("Expected Float32x3 position attributes in pulsar mesh"),
    };

    let mut base_radii = Vec::new();
    let mut tip_radii = Vec::new();
    for pos in pulsar_positions {
        let y = pos[1];
        let r = (pos[0] * pos[0] + pos[2] * pos[2]).sqrt();
        if y.abs() < 1e-4 {
            base_radii.push(r);
        } else if (y - 50.0).abs() < 1e-2 {
            tip_radii.push(r);
        }
    }

    assert!(
        !base_radii.is_empty(),
        "Pulsar beam mesh must have base vertices at y = 0"
    );
    let max_base_r = base_radii
        .iter()
        .copied()
        .fold(0.0f32, |acc, val| acc.max(val));
    assert!(
        max_base_r <= 0.0010,
        "Pulsar beam base radius must be tightly tapered at the pole (< 0.0010 AU), found {}",
        max_base_r
    );

    let max_tip_r = tip_radii
        .iter()
        .copied()
        .fold(0.0f32, |acc, val| acc.max(val));
    assert!(
        max_tip_r >= 6.0,
        "Pulsar beam must flare outward to >= 6.0 AU at tip, found {}",
        max_tip_r
    );

    let magnetar_mesh = generate_magnetar_field_loops_mesh();
    let magnetar_positions = match magnetar_mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(pos)) => pos,
        _ => panic!("Expected Float32x3 position attributes in magnetar mesh"),
    };

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;

    for pos in magnetar_positions {
        min_x = min_x.min(pos[0]);
        max_x = max_x.max(pos[0]);
        min_z = min_z.min(pos[2]);
        max_z = max_z.max(pos[2]);
    }

    let x_asymmetry = (max_x.abs() - min_x.abs()).abs();
    let z_asymmetry = (max_z.abs() - min_z.abs()).abs();
    assert!(
        x_asymmetry < 0.05,
        "Magnetar magnetic field must be symmetric across X axis (+X: {}, -X: {}, diff: {})",
        max_x,
        min_x,
        x_asymmetry
    );
    assert!(
        z_asymmetry < 0.05,
        "Magnetar magnetic field must be symmetric across Z axis (+Z: {}, -Z: {}, diff: {})",
        max_z,
        min_z,
        z_asymmetry
    );

    assert!(
        max_x >= 5.0 && min_x <= -5.0,
        "Magnetar field loops must extend to outer tier in both +X and -X directions (+X: {}, -X: {})",
        max_x,
        min_x
    );
    assert!(
        max_z >= 5.0 && min_z <= -5.0,
        "Magnetar field loops must extend to outer tier in both +Z and -Z directions (+Z: {}, -Z: {})",
        max_z,
        min_z
    );
}
