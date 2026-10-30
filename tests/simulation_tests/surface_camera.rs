//! Integration tests for surface horizon camera pose, manual rover drive, 3D terrain/vegetation generation, and surface ECS lifecycle.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;

fn setup_surface_camera_app() -> (App, Entity, Entity) {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.00 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        ))
        .id();

    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: None,
                focus: Vec3::ZERO,
                target_focus: Vec3::ZERO,
                radius: 16.0,
                target_radius: 16.0,
                ..default()
            },
            Transform::from_xyz(0.0, 5.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
            GlobalTransform::from(
                Transform::from_xyz(0.0, 5.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
            ),
        ))
        .id();

    app.add_systems(Update, update_pan_orbit_camera);
    (app, camera_ent, earth_ent)
}

#[test]
fn test_surface_horizon_camera_pose_and_rover_drive() {
    use protostellar::game::ui::NotificationToast;
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::rendering::surface_camera::{
        apply_manual_rover_drive, compute_surface_horizon_camera_pose,
        compute_surface_tangent_basis, toggle_surface_horizon_mode,
    };
    use protostellar::simulation::probes::{ProbeStatus, ProbeType, SpaceProbe};

    let (east, up, north) =
        compute_surface_tangent_basis(Vec3::new(0.6, 0.8, 0.0), Vec3::new(0.0, 0.0, 1.0));
    assert!((up.length() - 1.0).abs() < 1e-5);
    assert!((east.length() - 1.0).abs() < 1e-5);
    assert!((north.length() - 1.0).abs() < 1e-5);
    assert!(east.dot(up).abs() < 1e-5);
    assert!(north.dot(up).abs() < 1e-5);
    assert!(east.dot(north).abs() < 1e-5);

    let planet_pos = Vec3::new(1.0, 0.0, 0.0);
    let vis_rad = 0.025;
    let (eye_pos, eye_rot) =
        compute_surface_horizon_camera_pose(planet_pos, vis_rad, up, north, 0.022, 0.0, 0.25);
    let radial_dist = eye_pos.distance(planet_pos);
    assert!(
        (radial_dist - vis_rad * 1.022).abs() < 1e-5,
        "Surface horizon camera must sit at (1 + altitude_scale) * vis_rad above the planet center"
    );
    let cam_forward = eye_rot * -Vec3::Z;
    assert!(
        cam_forward.dot(up) > 0.15,
        "Positive elevation angle must tilt camera view upward toward the sky dome"
    );

    let mut cam = PanOrbitCamera::default();
    let mut toast = NotificationToast::default();
    let dummy_ent = Entity::from_bits(42);
    toggle_surface_horizon_mode(&mut cam, Some(dummy_ent), &mut toast);
    assert!(cam.surface_horizon_mode);
    assert_eq!(cam.target_entity, Some(dummy_ent));

    let mut rover = SpaceProbe {
        id: 1,
        probe_type: ProbeType::Rover,
        status: ProbeStatus::Landed,
        orientation: bevy::math::DQuat::IDENTITY,
        orbit_axis: Some(DVec3::Y),
        arrival_time: Some(1.0),
        landed_unspun_pos: Some(DVec3::X),
        rover_heading_unspun: Some(DVec3::Z),
        distance_driven_km: 0.0,
        samples_collected: 1,
        latest_assay: "Basaltic regolith".to_string(),
        traverse_trail_unspun: vec![DVec3::X],
    };
    apply_manual_rover_drive(&mut rover, 0.05, 0.02, EARTH_RADIUS_AU);
    assert!(
        rover.distance_driven_km > 100.0,
        "Manual WASD rover drive must advance distance_driven_km along the spherical surface"
    );
    assert!(
        rover.traverse_trail_unspun.len() >= 2,
        "Driving beyond angular threshold must record a new unspun breadcrumb waypoint"
    );

    // Verify ECS system parameter disjointness (no B0001 conflict) and runtime pose update
    let (mut app, camera_ent, earth_ent) = setup_surface_camera_app();
    app.init_resource::<NotificationToast>();
    app.world_mut()
        .entity_mut(earth_ent)
        .insert((Temperature(288.0), Composition::default()));
    app.world_mut()
        .entity_mut(camera_ent)
        .insert(Projection::Perspective(PerspectiveProjection::default()));
    if let Some(mut cam_comp) = app.world_mut().get_mut::<PanOrbitCamera>(camera_ent) {
        cam_comp.target_entity = Some(earth_ent);
        cam_comp.surface_horizon_mode = true;
    }
    app.add_systems(
        Update,
        protostellar::rendering::surface_camera::update_surface_horizon_camera
            .after(protostellar::rendering::camera::update_pan_orbit_camera),
    );
    app.update();
    let cam_after = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    assert!(cam_after.surface_horizon_mode);
}

#[test]
fn test_3d_planetary_surface_terrain_and_vegetation() {
    use bevy::render::mesh::VertexAttributeValues;
    use protostellar::rendering::surface_terrain::*;

    let unspun_center = Vec3::Y;
    let vis_rad = 0.025f32;
    let comp = Composition {
        silicate_frac: 0.70,
        metal_frac: 0.20,
        ice_frac: 0.10,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let bio = BiosphereState {
        habitability_score: 0.95,
        biomass_coverage_frac: 0.75,
        flora_pigment: FloraPigmentClass::SolarChlorophyll,
        ..default()
    };

    // 1. Test Elevation & Canyon Carving across all 8 sphere octants (including negative coordinates)
    for sx in [-1.0_f32, 1.0] {
        for sy in [-1.0_f32, 1.0] {
            for sz in [-1.0_f32, 1.0] {
                let dir = Vec3::new(sx, sy, sz).normalize();
                let elev = evaluate_surface_elevation(
                    dir,
                    &comp,
                    288.0,
                    BodyType::TerrestrialPlanet,
                    vis_rad,
                );
                assert!(
                    elev.is_finite(),
                    "Surface elevation must be finite in octant {:?}",
                    (sx, sy, sz)
                );
                assert!(
                    elev > 0.0,
                    "Surface elevation must stay above base sphere radius (got {elev})"
                );
            }
        }
    }

    let (canyon_d, _) = evaluate_canyon_carve(Vec3::new(-0.5, -0.5, -0.5).normalize());
    assert!(
        canyon_d >= 0.0 && canyon_d.is_finite(),
        "Canyon depth factor must be finite and non-negative"
    );

    // Verify rocky Earth (ice_frac == 0.0) detects liquid oceans via BiosphereState / VolatileInventory
    let rocky_earth = Composition::rocky();
    assert!(
        planet_has_surface_liquids(
            BodyType::TerrestrialPlanet,
            &rocky_earth,
            288.0,
            Some(&bio),
            None
        ),
        "Rocky planet with active biosphere in liquid water range must detect surface liquids"
    );

    // 2. Test 3D Terrain Mesh Generation (at negative octant to verify Euclidean fract_pos)
    let neg_center = Vec3::new(-0.577, -0.577, -0.577).normalize();
    let terrain_mesh = generate_surface_terrain_mesh(
        neg_center,
        vis_rad,
        &comp,
        288.0,
        BodyType::TerrestrialPlanet,
        Some(&bio),
        vis_rad * 0.24,
    );
    assert_eq!(
        terrain_mesh.count_vertices(),
        121 * 121,
        "Terrain grid must have 121x121 vertices"
    );
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        terrain_mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        for pos in positions {
            assert!(
                pos[0].is_finite() && pos[1].is_finite() && pos[2].is_finite(),
                "Terrain vertex position must not be NaN: {pos:?}"
            );
        }
    }
    if let Some(VertexAttributeValues::Float32x4(cols)) =
        terrain_mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    {
        assert_eq!(cols.len(), 121 * 121);
        for col in cols {
            assert!(
                col[3] > 0.9,
                "Terrain vertex colors must have full alpha opacity"
            );
        }
    } else {
        panic!("Terrain mesh must contain ATTRIBUTE_COLOR");
    }

    // 3. Test 3D Vegetation Mesh Generation
    let veg_mesh = generate_surface_vegetation_mesh(
        unspun_center,
        vis_rad,
        &comp,
        288.0,
        BodyType::TerrestrialPlanet,
        Some(&bio),
        1.0,
        vis_rad * 0.24,
    );
    assert!(
        veg_mesh.count_vertices() > 50,
        "Vegetation mesh must generate 3D trunks and foliage vertices for living biosphere"
    );
    assert!(
        veg_mesh.indices().is_some(),
        "Vegetation mesh must include triangle indices"
    );

    // Test 3D Geological Boulder Mesh Generation on Barren / Airless World (The Moon)
    let moon_comp = Composition {
        silicate_frac: 0.85,
        metal_frac: 0.15,
        ice_frac: 0.0,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let moon_boulder_mesh = generate_surface_vegetation_mesh(
        unspun_center,
        vis_rad * 0.272,
        &moon_comp,
        290.0,
        BodyType::Moon,
        None,
        0.166,
        vis_rad * 0.272 * 0.24,
    );
    assert!(
        moon_boulder_mesh.count_vertices() > 0,
        "Barren Moon surface must generate natural 3D faceted boulder vertices"
    );
    assert!(
        moon_boulder_mesh.indices().is_some(),
        "Boulder mesh must include triangle indices"
    );

    // 4. Test 3D Water Plane Generation
    let water_mesh = generate_surface_water_mesh(unspun_center, vis_rad, vis_rad * 0.24);
    assert!(
        water_mesh.count_vertices() > 0,
        "Water plane mesh must have non-zero vertices"
    );
}

#[test]
fn test_3d_planetary_surface_ecs_lifecycle() {
    use protostellar::game::ui::NotificationToast;
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::rendering::surface_terrain::*;

    let comp = Composition {
        silicate_frac: 0.70,
        metal_frac: 0.20,
        ice_frac: 0.10,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let bio = BiosphereState {
        habitability_score: 0.95,
        biomass_coverage_frac: 0.75,
        flora_pigment: FloraPigmentClass::SolarChlorophyll,
        ..default()
    };

    let (mut app, camera_ent, earth_ent) = setup_surface_camera_app();
    app.init_resource::<NotificationToast>();
    app.init_resource::<SurfaceTerrainState>();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.world_mut()
        .entity_mut(earth_ent)
        .insert((Temperature(288.0), comp, bio));

    // Turn surface horizon mode ON
    if let Some(mut cam_comp) = app.world_mut().get_mut::<PanOrbitCamera>(camera_ent) {
        cam_comp.target_entity = Some(earth_ent);
        cam_comp.surface_horizon_mode = true;
    }
    app.add_systems(
        Update,
        (
            protostellar::rendering::surface_camera::update_surface_horizon_camera,
            sync_surface_terrain_system,
        )
            .chain(),
    );
    app.update();

    let terrain_state = app.world().resource::<SurfaceTerrainState>();
    assert!(
        terrain_state.root_entity.is_some(),
        "Entering surface horizon mode must spawn PlanetarySurfaceTerrainRoot"
    );
    let spawned_root = terrain_state.root_entity.unwrap();
    assert!(
        app.world().get_entity(spawned_root).is_ok(),
        "Spawned root entity must exist in the world"
    );

    // Turn surface horizon mode OFF -> Must cleanly despawn
    if let Some(mut cam_comp) = app.world_mut().get_mut::<PanOrbitCamera>(camera_ent) {
        cam_comp.surface_horizon_mode = false;
    }
    app.update();

    let terrain_state_after = app.world().resource::<SurfaceTerrainState>();
    assert!(
        terrain_state_after.root_entity.is_none(),
        "Exiting surface horizon mode must reset root_entity"
    );
    assert!(
        app.world().get_entity(spawned_root).is_err(),
        "Exiting surface horizon mode must cleanly despawn PlanetarySurfaceTerrainRoot and children"
    );
}

#[test]
fn test_landed_rover_scale_shrinking_and_radius_formatting() {
    use protostellar::game::ui::telemetry_format::format_body_radius_display;
    use protostellar::simulation::probes::{
        sync_probe_transforms, FlightComputer, ProbeStatus, ProbeType, SpaceProbe,
    };
    use protostellar::simulation::resources::SimulationConfig;

    // 1. Verify metric radius formatting for rovers, small debris, and planets
    assert_eq!(format_body_radius_display(1.0e-11), "1.50 m");
    assert_eq!(format_body_radius_display(1.5e-11), "2.24 m");
    assert_eq!(format_body_radius_display(1.0e-10), "15.0 m");
    assert_eq!(format_body_radius_display(1.0e-7), "15.0 km");
    assert_eq!(format_body_radius_display(4.25875e-5), "6371 km");

    // 2. Verify rover transform scale shrinks on touchdown
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let config = SimulationConfig::default();
    let transit_scale = config.min_body_visual_radius * 0.08;
    app.insert_resource(config);
    app.add_systems(Update, sync_probe_transforms);

    let moon_vis_r = 0.001f32;
    let moon = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Moon,
                name: "The Moon".to_string(),
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            Radius(1.16e-5),
            Transform::from_scale(Vec3::splat(moon_vis_r)),
        ))
        .id();

    let rover = app
        .world_mut()
        .spawn((
            SpaceProbe::new(ProbeType::Rover, 1),
            FlightComputer {
                target_entity: moon,
                target_name: "The Moon".to_string(),
                launch_pos: DVec3::new(0.9, 0.0, 0.0),
                distance_to_target: 0.1,
                relative_speed: 2.0,
            },
            SimPosition(DVec3::new(0.95, 0.0, 0.0)),
            Transform::from_scale(Vec3::splat(transit_scale)),
        ))
        .id();

    // In transit: scale is standard exaggerated transit icon scale
    app.update();
    let t_transit = *app.world().get::<Transform>(rover).unwrap();
    assert!(
        (t_transit.scale.x - transit_scale).abs() < 1e-6,
        "In transit, probe must use visible transit icon scale"
    );

    // When landed: scale must shrink down to normal planetary rover scale (~0.0006 * vis_rad)
    if let Some(mut probe) = app.world_mut().get_mut::<SpaceProbe>(rover) {
        probe.status = ProbeStatus::Landed;
    }
    app.update();
    let t_landed = *app.world().get::<Transform>(rover).unwrap();
    let expected_landed_scale = moon_vis_r * 0.0006;
    assert!(
        (t_landed.scale.x - expected_landed_scale).abs() < 1e-7,
        "Landed rover must shrink to normal surface scale (expected {:.8}, got {:.8})",
        expected_landed_scale,
        t_landed.scale.x
    );
    assert!(
        t_landed.scale.x < t_transit.scale.x * 0.02,
        "Landed rover scale must be at least 50x smaller than interplanetary transit scale"
    );
}
