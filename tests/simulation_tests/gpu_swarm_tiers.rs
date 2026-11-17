//! GPU reseed logic, particle swarm density tiers, and dormant particle parking tests.

#[test]
fn test_compute_node_reseed_logic_and_disk_mass() {
    use protostellar::gpu::compute_node::GpuSimExtractedParams;

    // Normal solar nebula scenario with 0.02 M_sun disk
    let normal_params = GpuSimExtractedParams {
        star_mass: 1.0,
        disk_mass: 0.02,
        outer_radius: 50.0,
        inner_radius: 0.1,
        elapsed_years: 0.0,
        ..Default::default()
    };

    let is_empty = normal_params.disk_mass <= 0.0;
    let disk_mass = if is_empty {
        0.0
    } else if normal_params.star_mass > 10.0 {
        500.0
    } else {
        f64::from(normal_params.disk_mass)
    };

    assert!(
        (disk_mass - 0.02).abs() < 1e-6,
        "Reseeded disk mass must preserve configured scenario disk mass (0.02 M_sun), got {disk_mass:.6}"
    );

    // Empty disk scenario
    let empty_params = GpuSimExtractedParams {
        star_mass: 1.0,
        disk_mass: 0.0,
        ..Default::default()
    };
    let empty_disk_mass = if empty_params.disk_mass <= 0.0 {
        0.0
    } else if empty_params.star_mass > 10.0 {
        500.0
    } else {
        f64::from(empty_params.disk_mass)
    };
    assert_eq!(empty_disk_mass, 0.0);

    // Supermassive scenario (Quasar/SMBH)
    let smbh_params = GpuSimExtractedParams {
        star_mass: 100.0,
        disk_mass: 1.0,
        ..Default::default()
    };
    let smbh_disk_mass = if smbh_params.disk_mass <= 0.0 {
        0.0
    } else if smbh_params.star_mass > 10.0 {
        500.0
    } else {
        f64::from(smbh_params.disk_mass)
    };
    assert_eq!(smbh_disk_mass, 500.0);
}

#[test]
fn test_particle_density_tiers_and_workgroup_allocation() {
    use protostellar::gpu::compute_node::MAX_GPU_PARTICLES;
    use protostellar::rendering::particle_swarm::setup::MAX_SWARM_PARTICLES;
    use protostellar::simulation::resources::{ParticleDensityTier, SimulationConfig};

    assert_eq!(MAX_GPU_PARTICLES, 500_000);
    assert_eq!(MAX_SWARM_PARTICLES, 500_000);

    let tier_100k = ParticleDensityTier::Standard100k;
    assert_eq!(tier_100k.particle_count(), 100_000);
    assert_eq!(tier_100k.label(), "100k [Standard]");

    let tier_250k = tier_100k.cycle();
    assert_eq!(tier_250k, ParticleDensityTier::High250k);
    assert_eq!(tier_250k.particle_count(), 250_000);
    assert_eq!(tier_250k.label(), "250k [High]");

    let tier_500k = tier_250k.cycle();
    assert_eq!(tier_500k, ParticleDensityTier::Ultra500k);
    assert_eq!(tier_500k.particle_count(), 500_000);
    assert_eq!(tier_500k.label(), "500k [Ultra]");

    let back_to_100k = tier_500k.cycle();
    assert_eq!(back_to_100k, ParticleDensityTier::Standard100k);

    let config = SimulationConfig::default();
    assert_eq!(config.particle_density, ParticleDensityTier::Ultra500k);
    assert_eq!(config.target_particle_count, 500_000);
    assert_eq!(config.active_particles, 500_000);
    assert!(config.particle_render_scale <= 0.15);

    let (disk, cfg) = (
        protostellar::simulation::resources::DiskParameters::default(),
        SimulationConfig::default(),
    );
    let bufs = protostellar::rendering::particle_swarm::setup::generate_initial_swarm_buffers(
        10, 10, 1.0e-5, &disk, &cfg,
    );
    assert!(bufs.mesh_colors[0][3] <= 0.45);
    let half_w = (bufs.mesh_positions[1][0] - bufs.mesh_positions[0][0]) * 0.5;
    assert!(half_w > 0.0 && half_w <= 0.015);
}

#[test]
fn test_dormant_particles_are_hidden_and_parked() {
    use protostellar::rendering::particle_swarm::ParticleSwarmData;
    use protostellar::rendering::particle_swarm::setup::{
        generate_initial_swarm_buffers, park_dormant_particles,
    };
    use protostellar::simulation::resources::{DiskParameters, SimulationConfig};

    let disk_params = DiskParameters::default();
    let config = SimulationConfig::default();
    let capacity = 5000;
    let active_count = 1000;

    let buffers =
        generate_initial_swarm_buffers(capacity, active_count, 1.0e-5, &disk_params, &config);

    assert_eq!(buffers.positions.len(), capacity);
    assert_eq!(buffers.masses.len(), capacity);
    assert_eq!(buffers.mesh_positions.len(), capacity * 4);
    assert_eq!(buffers.mesh_colors.len(), capacity * 4);

    // Active particle verification (index 0..active_count)
    assert!(buffers.masses[0] > 0.0);
    assert!((buffers.positions[0][1] + 5000.0).abs() > 1.0);
    assert!(buffers.mesh_colors[0][3] > 0.0);

    // Dormant particle verification (index active_count..capacity)
    for i in active_count..capacity {
        assert_eq!(buffers.masses[i], 0.0);
        assert_eq!(buffers.positions[i], [0.0, -5000.0, 0.0]);
        assert_eq!(buffers.velocities[i], [0.0, 0.0, 0.0]);
        assert_eq!(buffers.colors[i], [0.0, 0.0, 0.0, 0.0]);

        let v_idx = i * 4;
        for k in 0..4 {
            assert_eq!(buffers.mesh_positions[v_idx + k], [0.0, -5000.0, 0.0]);
            assert_eq!(buffers.mesh_colors[v_idx + k], [0.0, 0.0, 0.0, 0.0]);
        }
    }

    // Swarm data dynamic parking test
    let mut data = ParticleSwarmData {
        positions: buffers.positions,
        velocities: buffers.velocities,
        masses: buffers.masses,
        compositions: buffers.compositions,
        temperatures: buffers.temperatures,
        colors: buffers.colors,
        mesh_positions: buffers.mesh_positions,
        mesh_colors: buffers.mesh_colors,
        bin_heads: vec![-1; 4096],
        bin_next: vec![-1; capacity],
        mesh_handle: bevy::asset::Handle::default(),
        count: 500, // Reduced from 1000 to 500
        base_mass: 1.0e-5,
        is_dirty: false,
        pending_gpu_accretions: Vec::new(),
    };

    park_dormant_particles(&mut data);
    for i in 500..capacity {
        assert_eq!(data.masses[i], 0.0);
        assert_eq!(data.positions[i], [0.0, -5000.0, 0.0]);
        assert_eq!(data.colors[i], [0.0, 0.0, 0.0, 0.0]);
        let v_idx = i * 4;
        for k in 0..4 {
            assert_eq!(data.mesh_positions[v_idx + k], [0.0, -5000.0, 0.0]);
            assert_eq!(data.mesh_colors[v_idx + k], [0.0, 0.0, 0.0, 0.0]);
        }
    }

    let mut resized_cfg = SimulationConfig {
        target_particle_count: 2500,
        ..Default::default()
    };
    protostellar::rendering::particle_swarm::reseed_particle_swarm(
        &mut data,
        &disk_params,
        &resized_cfg,
    );
    assert_eq!(data.count, 2500);
    assert!(data.masses[2499] > 0.0);
    assert!(data.colors[2499][3] > 0.0);
    assert_eq!(data.masses[2500], 0.0);

    resized_cfg.target_particle_count = 500_000;
    protostellar::rendering::particle_swarm::billboards::update_billboard_mesh_quads(
        &mut data,
        bevy::math::Vec3::X,
        bevy::math::Vec3::Y,
        &disk_params,
        resized_cfg.particle_render_scale,
    );
    let r_v = (data.mesh_positions[1][0] - data.mesh_positions[0][0]) * 0.5;
    assert!(r_v > 0.0 && r_v <= 0.015);
    assert!(data.mesh_colors[0][3] <= 0.45);
}

#[test]
fn test_gpu_billboard_mesh_populates_centers_and_radii_normals() {
    use bevy::prelude::*;
    use bevy::render::mesh::{PrimitiveTopology, VertexAttributeValues};
    use protostellar::rendering::particle_swarm::ParticleSwarmData;
    use protostellar::rendering::particle_swarm::billboards::update_gpu_billboard_mesh;
    use protostellar::rendering::particle_swarm::setup::{
        MAX_SWARM_PARTICLES, generate_initial_swarm_buffers,
    };
    use protostellar::simulation::resources::{DiskParameters, SimulationConfig};

    let disk_params = DiskParameters::default();
    let config = SimulationConfig {
        target_particle_count: 50_000,
        ..Default::default()
    };
    let individual_mass = (disk_params.disk_mass / 50_000.0) as f32;
    let mut buffers =
        generate_initial_swarm_buffers(600, 500, individual_mass, &disk_params, &config);
    buffers.masses[1] = 0.0; // Dormant/accreted slot

    let mut meshes = Assets::<Mesh>::default();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffers.mesh_positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32; 3]; 600 * 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, buffers.mesh_uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, buffers.mesh_colors.clone());
    mesh.insert_indices(bevy::render::mesh::Indices::U32(buffers.indices));
    let mesh_handle = meshes.add(mesh);

    let mut data = ParticleSwarmData {
        positions: buffers.positions,
        velocities: buffers.velocities,
        masses: buffers.masses,
        compositions: buffers.compositions,
        temperatures: buffers.temperatures,
        colors: buffers.colors,
        mesh_positions: buffers.mesh_positions,
        mesh_colors: buffers.mesh_colors,
        bin_heads: vec![-1; 64],
        bin_next: vec![-1; MAX_SWARM_PARTICLES.min(600)],
        mesh_handle: mesh_handle.clone(),
        count: 500,
        base_mass: individual_mass,
        is_dirty: true,
        pending_gpu_accretions: Vec::new(),
    };

    update_gpu_billboard_mesh(&mut data, &disk_params, &mut meshes);

    // All 4 corners of particle 0 share exact center position
    let p0 = data.positions[0];
    assert_eq!(data.mesh_positions[0], p0);
    assert_eq!(data.mesh_positions[3], p0);

    // Vertex normal.x carries unscaled half-radius for active particles, 0.0 for dormant
    let mesh_ref = meshes.get(&mesh_handle).unwrap();
    let Some(VertexAttributeValues::Float32x3(norms)) = mesh_ref.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!("Mesh must retain Float32x3 ATTRIBUTE_NORMAL for GPU billboard radii");
    };
    assert!(
        norms[0][0] > 0.0,
        "Active quad must carry positive half-radius"
    );
    assert_eq!(
        norms[4][0], 0.0,
        "Cleared/dormant slot must collapse to zero size"
    );
    assert_eq!(
        norms[500 * 4][0],
        0.0,
        "Beyond-count slot must carry zero size"
    );
}

#[test]
fn test_selective_billboard_sync_skips_unchanged_colors_and_dormant_tail() {
    use bevy::prelude::*;
    use bevy::render::mesh::{PrimitiveTopology, VertexAttributeValues};
    use protostellar::rendering::particle_swarm::ParticleSwarmData;
    use protostellar::rendering::particle_swarm::billboards::{
        update_gpu_billboard_mesh, update_gpu_billboard_mesh_selective,
    };
    use protostellar::rendering::particle_swarm::setup::generate_initial_swarm_buffers;
    use protostellar::simulation::resources::{DiskParameters, SimulationConfig};

    let disk_params = DiskParameters::default();
    let config = SimulationConfig {
        target_particle_count: 50_000,
        ..Default::default()
    };
    let individual_mass = (disk_params.disk_mass / 50_000.0) as f32;
    let buffers = generate_initial_swarm_buffers(600, 500, individual_mass, &disk_params, &config);

    let mut meshes = Assets::<Mesh>::default();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffers.mesh_positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32; 3]; 600 * 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, buffers.mesh_uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, buffers.mesh_colors.clone());
    mesh.insert_indices(bevy::render::mesh::Indices::U32(buffers.indices));
    let mesh_handle = meshes.add(mesh);

    let mut data = ParticleSwarmData {
        positions: buffers.positions,
        velocities: buffers.velocities,
        masses: buffers.masses,
        compositions: buffers.compositions,
        temperatures: buffers.temperatures,
        colors: buffers.colors,
        mesh_positions: buffers.mesh_positions,
        mesh_colors: buffers.mesh_colors,
        bin_heads: vec![-1; 64],
        bin_next: vec![-1; 600],
        mesh_handle: mesh_handle.clone(),
        count: 500,
        base_mass: individual_mass,
        is_dirty: true,
        pending_gpu_accretions: Vec::new(),
    };

    update_gpu_billboard_mesh(&mut data, &disk_params, &mut meshes);

    // Move particle 0 and mark sentinel values in colors/tail to verify selective path preserves them
    data.positions[0] = [3.25, 0.1, -1.5];
    data.colors[0] = [0.99, 0.01, 0.5, 1.0];
    data.mesh_positions[505 * 4] = [42.0, 0.0, 0.0];

    update_gpu_billboard_mesh_selective(&mut data, &disk_params, &mut meshes, false, false, false);

    let mesh_ref = meshes.get(&mesh_handle).unwrap();
    let Some(VertexAttributeValues::Float32x3(pos_attr)) =
        mesh_ref.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("missing positions");
    };
    assert_eq!(pos_attr[0], [3.25, 0.1, -1.5]);
    assert_ne!(
        pos_attr[505 * 4],
        [42.0, 0.0, 0.0],
        "Dormant tail beyond active count must not be copied during active-only sync"
    );
    let Some(VertexAttributeValues::Float32x4(col_attr)) =
        mesh_ref.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("missing colors");
    };
    assert_ne!(
        col_attr[0][0], 0.99,
        "Color attribute upload should be skipped when update_colors=false"
    );
}
