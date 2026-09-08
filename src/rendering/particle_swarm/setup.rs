use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use rand::prelude::*;
use rand_distr::Normal;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::disk::sample_disk_radius;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::{ParticleSwarmData, ParticleSwarmMesh};

/// Procedurally generates a smooth Gaussian radial particle texture for soft protoplanetary dust.
pub fn create_soft_particle_texture() -> Image {
    let size = 64u32;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let center = (size as f32 - 1.0) * 0.5;
    let radius = center;

    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - center) / radius;
            let dy = (y as f32 - center) / radius;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq > 1.0 {
                data.extend_from_slice(&[255, 255, 255, 0]);
            } else {
                let falloff = (-2.2 * dist_sq).exp();
                let edge_fade = (1.0 - dist_sq.powf(2.0)).max(0.0);
                let alpha = (falloff * edge_fade * 255.0).clamp(0.0, 255.0) as u8;
                data.extend_from_slice(&[255, 255, 255, alpha]);
            }
        }
    }

    Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Helper function to compute realistic particle emission and reflection colors.
pub fn calculate_particle_color(comp: &Composition, temp: f32) -> [f32; 4] {
    let (br, bg, bb) = blackbody_to_srgb(f64::from(temp));
    let (cr, cg, cb) = comp.visual_color_tint();

    if comp.gas_frac > 0.95 && comp.metal_frac == 0.0 {
        // Pristine Primordial Hydrogen Cocoon (JWST Little Red Dot): Deep ruby-crimson luminescence
        [
            (br * 0.40 + 0.95).clamp(0.6, 1.8),
            (bg * 0.15 + 0.18).clamp(0.1, 0.45),
            (bb * 0.15 + 0.12).clamp(0.05, 0.35),
            1.0f32,
        ]
    } else if comp.gas_frac > 0.35 {
        // Primordial gaseous envelope: ethereal cyan-blue glow
        [
            (br * 0.20 + 0.35).clamp(0.2, 1.2),
            (bg * 0.20 + 0.75).clamp(0.4, 1.4),
            (bb * 0.20 + 1.10).clamp(0.6, 1.5),
            1.0f32,
        ]
    } else {
        [
            (br * 0.45 + cr * 0.85).clamp(0.4, 1.4),
            (bg * 0.45 + cg * 0.85).clamp(0.35, 1.4),
            (bb * 0.45 + cb * 0.85).clamp(0.3, 1.4),
            1.0f32,
        ]
    }
}

pub struct SwarmInitialBuffers {
    pub positions: Vec<[f32; 3]>,
    pub velocities: Vec<[f32; 3]>,
    pub masses: Vec<f32>,
    pub compositions: Vec<Composition>,
    pub temperatures: Vec<f32>,
    pub colors: Vec<[f32; 4]>,
    pub mesh_positions: Vec<[f32; 3]>,
    pub mesh_uvs: Vec<[f32; 2]>,
    pub mesh_colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

pub fn generate_initial_swarm_buffers(
    n_particles: usize,
    individual_mass: f32,
    disk_params: &DiskParameters,
    config: &SimulationConfig,
) -> SwarmInitialBuffers {
    let mut rng = rand::rng();
    let mut positions = Vec::with_capacity(n_particles);
    let mut velocities = Vec::with_capacity(n_particles);
    let mut masses = Vec::with_capacity(n_particles);
    let mut compositions = Vec::with_capacity(n_particles);
    let mut temperatures = Vec::with_capacity(n_particles);
    let mut colors = Vec::with_capacity(n_particles);

    let mut mesh_positions = Vec::with_capacity(n_particles * 4);
    let mut mesh_uvs = Vec::with_capacity(n_particles * 4);
    let mut mesh_colors = Vec::with_capacity(n_particles * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(n_particles * 6);

    let scale_mult = (disk_params.outer_radius_au / 45.0).clamp(0.02, 1.0) as f32;
    let base_render_r = 0.085 * config.particle_render_scale * scale_mult;

    for i in 0..n_particles {
        let (r, comp) = sample_disk_radius(&mut rng, disk_params);
        let phi = rng.random_range(0.0..2.0 * PI);
        let h_scale = (0.030 * r * (r / 1.0).powf(0.25)).max(1e-4);
        let z_height: f64 = if let Ok(dist) = Normal::new(0.0, h_scale) {
            rng.sample(dist)
        } else {
            0.0
        };

        let pos = [
            (r * phi.cos()) as f32,
            z_height as f32,
            (r * phi.sin()) as f32,
        ];
        let v_k = (G_ASTRO * disk_params.central_star_mass / r).sqrt();
        let v_phi = v_k as f32;
        let vel = [(-v_phi * phi.sin() as f32), 0.0, (v_phi * phi.cos() as f32)];
        let temp = (disk_params.reference_temp_1au * (r / 1.0).powf(-0.5)) as f32;
        let final_color = calculate_particle_color(&comp, temp);

        positions.push(pos);
        velocities.push(vel);
        masses.push(individual_mass);
        compositions.push(comp);
        temperatures.push(temp);
        colors.push(final_color);

        let v_idx = (i * 4) as u32;
        mesh_positions.push([pos[0] - base_render_r, pos[1], pos[2] - base_render_r]);
        mesh_positions.push([pos[0] + base_render_r, pos[1], pos[2] - base_render_r]);
        mesh_positions.push([pos[0] + base_render_r, pos[1], pos[2] + base_render_r]);
        mesh_positions.push([pos[0] - base_render_r, pos[1], pos[2] + base_render_r]);

        mesh_uvs.push([0.0, 0.0]);
        mesh_uvs.push([1.0, 0.0]);
        mesh_uvs.push([1.0, 1.0]);
        mesh_uvs.push([0.0, 1.0]);

        let dust_col = [final_color[0], final_color[1], final_color[2], 0.75];
        mesh_colors.push(dust_col);
        mesh_colors.push(dust_col);
        mesh_colors.push(dust_col);
        mesh_colors.push(dust_col);

        indices.push(v_idx);
        indices.push(v_idx + 1);
        indices.push(v_idx + 2);
        indices.push(v_idx);
        indices.push(v_idx + 2);
        indices.push(v_idx + 3);
    }

    SwarmInitialBuffers {
        positions,
        velocities,
        masses,
        compositions,
        temperatures,
        colors,
        mesh_positions,
        mesh_uvs,
        mesh_colors,
        indices,
    }
}

/// Initializes the dense 50,000 particle visual mesh and zero-allocation data structures.
pub fn setup_particle_swarm(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    disk_params: Res<DiskParameters>,
    config: Res<SimulationConfig>,
) {
    let n_particles = config.target_particle_count.max(50_000);
    let individual_mass = (disk_params.disk_mass / (n_particles as f64)) as f32;

    let buffers =
        generate_initial_swarm_buffers(n_particles, individual_mass, &disk_params, &config);

    let mesh_normals = vec![[0.0f32, 1.0f32, 0.0f32]; n_particles * 4];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffers.mesh_positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, buffers.mesh_uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, buffers.mesh_colors.clone());
    mesh.insert_indices(bevy::render::mesh::Indices::U32(buffers.indices));

    let mesh_handle = meshes.add(mesh);
    let texture_handle = images.add(create_soft_particle_texture());

    let material = materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle),
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(Vec3::ZERO),
        NotShadowCaster,
        ParticleSwarmMesh,
    ));

    commands.insert_resource(ParticleSwarmData {
        positions: buffers.positions,
        velocities: buffers.velocities,
        masses: buffers.masses,
        compositions: buffers.compositions,
        temperatures: buffers.temperatures,
        colors: buffers.colors,
        mesh_positions: buffers.mesh_positions,
        mesh_colors: buffers.mesh_colors,
        bin_heads: vec![-1; 4096],
        bin_next: vec![-1; n_particles],
        mesh_handle,
        count: n_particles,
        base_mass: individual_mass,
        is_dirty: true,
        pending_gpu_accretions: Vec::new(),
    });
}

/// Instantaneously reseeds all particles in the swarm to match a newly loaded scenario's disk parameters.
pub fn reseed_particle_swarm(
    data: &mut ParticleSwarmData,
    disk_params: &DiskParameters,
    _config: &SimulationConfig,
) {
    let n = data.count;

    if disk_params.disk_mass <= 0.0 {
        data.base_mass = 0.0;
        for (((m, p), v), c) in data
            .masses
            .iter_mut()
            .zip(&mut data.positions)
            .zip(&mut data.velocities)
            .zip(&mut data.colors)
            .take(n)
        {
            *m = 0.0;
            *p = [0.0, -5000.0, 0.0];
            *v = [0.0, 0.0, 0.0];
            *c = [0.0, 0.0, 0.0, 0.0];
        }
        data.is_dirty = true;
        return;
    }

    let mut rng = rand::rng();
    let individual_mass = (disk_params.disk_mass / (n as f64)) as f32;
    data.base_mass = individual_mass;

    for i in 0..n {
        let (r, comp) = sample_disk_radius(&mut rng, disk_params);
        let phi = rng.random_range(0.0..2.0 * PI);
        let h_scale = (0.030 * r * (r / 1.0).powf(0.25)).max(1e-4);
        let z_height: f64 = if let Ok(dist) = Normal::new(0.0, h_scale) {
            rng.sample(dist)
        } else {
            0.0
        };

        let pos = [
            (r * phi.cos()) as f32,
            z_height as f32,
            (r * phi.sin()) as f32,
        ];
        let v_k = (G_ASTRO * disk_params.central_star_mass / r).sqrt();
        let v_phi = v_k as f32;
        let vel = [(-v_phi * phi.sin() as f32), 0.0, (v_phi * phi.cos() as f32)];
        let temp = (disk_params.reference_temp_1au * (r / 1.0).powf(-0.5)) as f32;
        let final_color = calculate_particle_color(&comp, temp);

        if let (Some(p), Some(v), Some(m), Some(comp_slot), Some(t), Some(c)) = (
            data.positions.get_mut(i),
            data.velocities.get_mut(i),
            data.masses.get_mut(i),
            data.compositions.get_mut(i),
            data.temperatures.get_mut(i),
            data.colors.get_mut(i),
        ) {
            *p = pos;
            *v = vel;
            *m = individual_mass;
            *comp_slot = comp;
            *t = temp;
            *c = final_color;
        }
    }
    data.is_dirty = true;
}
