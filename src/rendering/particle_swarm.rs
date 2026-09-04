//! High-performance real-time particle swarm renderer for 50,000 glowing protoplanetary particles
//! with zero-allocation in-place buffer synchronization, 2D polar spatial hash grid,
//! camera-aligned soft circular particle billboarding, and realistic astrophysical accretion.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::mesh::{PrimitiveTopology, VertexAttributeValues};
use rand::prelude::*;
use rand_distr::Normal;
use rayon::prelude::*;

use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::disk::sample_disk_radius;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Marker component for the 50,000 particle visual swarm mesh.
#[derive(Component)]
pub struct ParticleSwarmMesh;

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

/// CPU storage for particle state synchronized with visual mesh buffers.
#[derive(Resource)]
pub struct ParticleSwarmData {
    pub positions: Vec<[f32; 3]>,
    pub velocities: Vec<[f32; 3]>,
    pub masses: Vec<f32>,
    pub compositions: Vec<Composition>,
    pub temperatures: Vec<f32>,
    pub colors: Vec<[f32; 4]>,
    pub mesh_positions: Vec<[f32; 3]>,
    pub mesh_colors: Vec<[f32; 4]>,
    pub bin_heads: Vec<i32>,
    pub bin_next: Vec<i32>,
    pub mesh_handle: Handle<Mesh>,
    pub count: usize,
    pub base_mass: f32,
    pub is_dirty: bool,
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

    let base_render_r = 0.085 * config.particle_render_scale;

    for i in 0..n_particles {
        let (r, comp) = sample_disk_radius(&mut rng, &disk_params);
        let phi = rng.random_range(0.0..2.0 * PI);

        let h_scale = 0.030 * r * (r / 1.0).powf(0.25);
        let normal_dist = Normal::new(0.0, h_scale).unwrap();
        let z_height: f64 = rng.sample(normal_dist);

        let pos = [
            (r * phi.cos()) as f32,
            z_height as f32,
            (r * phi.sin()) as f32,
        ];

        let v_k = (G_ASTRO * disk_params.central_star_mass / r).sqrt();
        let v_phi = v_k as f32;

        let vel = [(-v_phi * phi.sin() as f32), 0.0, (v_phi * phi.cos() as f32)];

        let temp = (disk_params.reference_temp_1au * (r / 1.0).powf(-0.5)) as f32;

        let (br, bg, bb) = blackbody_to_srgb(temp as f64);
        let (cr, cg, cb) = comp.visual_color_tint();
        let final_color = if comp.gas_frac > 0.95 && comp.metal_frac == 0.0 {
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
        };

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

    let mesh_normals = vec![[0.0f32, 1.0f32, 0.0f32]; n_particles * 4];

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, mesh_positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, mesh_uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, mesh_colors.clone());
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

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
        positions,
        velocities,
        masses,
        compositions,
        temperatures,
        colors,
        mesh_positions,
        mesh_colors,
        bin_heads: vec![-1; 4096],
        bin_next: vec![-1; n_particles],
        mesh_handle,
        count: n_particles,
        base_mass: individual_mass,
        is_dirty: true,
    });
}

pub fn update_particle_swarm(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    mut config: ResMut<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    player_state: Res<PlayerInteractionState>,
    star_query: Query<
        (
            &SimPosition,
            &Mass,
            &IgnitionState,
            Option<&BlackHoleStarState>,
            &CelestialBody,
        ),
        With<CentralStar>,
    >,
    mut massive_query: Query<
        (
            Entity,
            &SimPosition,
            &mut Mass,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
        ),
        Without<CentralStar>,
    >,
    camera_query: Query<&Transform, With<Camera>>,
    swarm: Option<ResMut<ParticleSwarmData>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }
    let Some(mut data) = swarm else {
        return;
    };
    let Ok((star_pos, star_mass, ignition, opt_bhs, star_body)) = star_query.single() else {
        return;
    };

    let (cam_right, cam_up) = if let Ok(cam_t) = camera_query.single() {
        (cam_t.right().as_vec3(), cam_t.up().as_vec3())
    } else {
        (Vec3::X, Vec3::Y)
    };

    let massive_bodies: Vec<(Entity, DVec3, f64, BodyType)> = massive_query
        .iter()
        .map(|(e, p, m, _, _, body)| (e, p.0, m.0, body.body_type))
        .collect();
    let star_pos_f32 = [star_pos.x as f32, star_pos.y as f32, star_pos.z as f32];
    let star_m = star_mass.0 as f32;
    let g_const = G_ASTRO as f32;
    let shockwave_r = ignition.shockwave_radius as f32;
    let enable_gas_drag = config.enable_gas_drag;
    let gas_scale = config.gas_density_scale;
    let p_render_scale = config.particle_render_scale;

    // If the Quasi-Star cocoon has blown out or the central star is a Black Hole,
    // the quasar's cataclysmic radiation pressure obliterates the entire protoplanetary disk.
    let quasar_blown_out = opt_bhs.map(|s| s.is_blown_out).unwrap_or(false)
        || star_body.body_type == BodyType::BlackHole;

    let tractor_pos_mass = if let (PlayerTool::GravitationalTractor, Some(pos)) =
        (player_state.active_tool, player_state.tractor_position)
    {
        [
            pos.x as f32,
            pos.y as f32,
            pos.z as f32,
            player_state.tractor_mass as f32,
        ]
    } else {
        [0.0; 4]
    };

    let ParticleSwarmData {
        positions,
        velocities,
        masses,
        compositions,
        temperatures,
        colors,
        mesh_positions,
        mesh_colors,
        bin_heads,
        bin_next,
        mesh_handle,
        count,
        base_mass,
        ..
    } = &mut *data;

    let n = *count;
    let b_mass = *base_mass;
    let speed_mult = time_warp.multiplier as f32;
    let visual_flow_dt = (0.002 * (1.0 + speed_mult.log10().max(0.0) * 2.0)).min(0.08);

    let gpu_active = config.enable_gpu_compute && config.gpu_compute_active;

    let accreted_events: Vec<(Entity, f64)> = positions
        .par_chunks_mut(4096)
        .zip(velocities.par_chunks_mut(4096))
        .zip(masses.par_chunks_mut(4096))
        .zip(temperatures.par_chunks_mut(4096))
        .zip(colors.par_chunks_mut(4096))
        .map(
            |((((pos_chunk, vel_chunk), mass_chunk), temp_chunk), col_chunk)| {
                let mut chunk_accretions = Vec::new();
                for i in 0..pos_chunk.len() {
                    let m = mass_chunk[i];
                    if m <= 0.0 {
                        continue;
                    }
                    let mut pos = pos_chunk[i];
                    let mut vel = vel_chunk[i];

                    if !gpu_active {
                        let dx = pos[0] - star_pos_f32[0];
                        let dz = pos[2] - star_pos_f32[2];
                        let mut r = (dx * dx + dz * dz).sqrt().max(0.08);
                        let mut phi = dz.atan2(dx);
                        let m_eff = star_m * (1.0 - 0.0005);
                        let omega = (g_const * m_eff / (r * r * r)).sqrt();
                        let v_k = omega * r;
                        phi = (phi + omega * visual_flow_dt) % (2.0 * PI as f32);
                        if phi < 0.0 {
                            phi += 2.0 * PI as f32;
                        }
                        if enable_gas_drag && gas_scale > 0.001 {
                            let gas_density = 1.0e-4 * (r / 1.0).powf(-2.25) * gas_scale;
                            let drag_rate = (0.000005 * gas_density).min(0.0005);
                            let migration = (r * drag_rate * visual_flow_dt).min(r * 0.005);
                            r = (r - migration).max(disk_params.inner_radius_au as f32 * 0.8);
                        }
                        if quasar_blown_out && r < 25.0 {
                            mass_chunk[i] = 0.0;
                            pos_chunk[i] = [0.0, -5000.0, 0.0];
                            col_chunk[i][3] = 0.0;
                            continue;
                        } else if shockwave_r > 0.0 {
                            if r < shockwave_r {
                                let push_rate = 1.80 * visual_flow_dt;
                                r = (r + push_rate).min(disk_params.outer_radius_au as f32);
                            } else if (r - shockwave_r).abs() < 3.0 {
                                let shock_boost = (3.0 - (r - shockwave_r).abs()) / 3.0;
                                r += shock_boost * 2.2 * visual_flow_dt;
                            }
                        }

                        let mut accreted = false;
                        for &(p_ent, p_pos, p_m, p_type) in &massive_bodies {
                            let is_major_body = p_m >= 0.01 * EARTH_MASS_SOLAR
                                && !matches!(p_type, BodyType::Asteroid | BodyType::Comet);

                            let pdx = pos[0] - p_pos.x as f32;
                            let pdz = pos[2] - p_pos.z as f32;
                            let p_dist = (pdx * pdx + pdz * pdz).sqrt().max(0.001);

                            let p_dist_au = p_pos.length() as f32;
                            let is_inner_terrestrial = p_dist_au < 2.7;
                            let warp_sweep = (1.0 + speed_mult.log10().max(0.0) * 0.30).min(2.2);
                            let is_massive_disk = star_m > 10.0;
                            let hill_r = p_dist_au * ((p_m / (3.0 * star_m as f64)).cbrt() as f32);
                            let bondi_r = if is_massive_disk {
                                (0.08 * (p_m / JUPITER_MASS_SOLAR).sqrt() as f32).clamp(0.05, 8.0)
                            } else {
                                0.0
                            };
                            let effective_grav_r = hill_r.max(bondi_r);
                            let physical_r = if p_m >= 0.08 {
                                (0.00465 * (p_m / 1.0).powf(0.8) as f32).clamp(0.004, 0.20)
                            } else {
                                (0.005 * (p_m / EARTH_MASS_SOLAR).cbrt() as f32).clamp(0.003, 0.040)
                            };

                            let acc_r = if is_major_body {
                                if is_inner_terrestrial && !is_massive_disk {
                                    ((physical_r + 0.50 * hill_r) * warp_sweep)
                                        .clamp(physical_r, 0.150)
                                } else if is_massive_disk {
                                    ((physical_r + 0.90 * effective_grav_r) * warp_sweep)
                                        .clamp(physical_r, 12.0)
                                } else {
                                    // Realistic accretion envelope for gas giants in normal star systems
                                    ((physical_r + 0.60 * hill_r) * warp_sweep)
                                        .clamp(physical_r, 0.350)
                                }
                            } else {
                                ((physical_r * 1.5) * warp_sweep).clamp(0.00005, 0.0015)
                            };

                            if p_dist < acc_r {
                                chunk_accretions.push((p_ent, m as f64));
                                mass_chunk[i] = 0.0;
                                pos_chunk[i] = [0.0, -5000.0, 0.0];
                                accreted = true;
                                break;
                            }

                            if is_major_body {
                                let hill_r = p_pos.length() as f32
                                    * ((p_m / (3.0 * star_m as f64)).cbrt() as f32);
                                if p_dist < hill_r * 2.5 {
                                    let kick = (g_const * p_m as f32 / (p_dist * p_dist + 0.01))
                                        * visual_flow_dt.min(0.02);
                                    r += (pdx * kick * 0.04).clamp(-0.08, 0.08);
                                }
                            }
                        }
                        if accreted {
                            continue;
                        }
                        if tractor_pos_mass[3] > 0.0 {
                            let tdx = tractor_pos_mass[0] - pos[0];
                            let tdz = tractor_pos_mass[2] - pos[2];
                            let t_dist = (tdx * tdx + tdz * tdz).sqrt().max(0.1);
                            let pull = (g_const * tractor_pos_mass[3] / (t_dist * t_dist + 0.1))
                                * visual_flow_dt.min(0.05);
                            r += (tdx * pull * 0.05).clamp(-0.2, 0.2);
                        }
                        r = r.clamp(
                            disk_params.inner_radius_au as f32 * 0.80,
                            disk_params.outer_radius_au as f32 * 1.05,
                        );
                        pos[0] = star_pos_f32[0] + r * phi.cos();
                        pos[1] *= (-0.005 * visual_flow_dt).exp();
                        pos[2] = star_pos_f32[2] + r * phi.sin();
                        vel[0] = -v_k * phi.sin();
                        vel[1] = 0.0;
                        vel[2] = v_k * phi.cos();
                        let temp = (disk_params.reference_temp_1au as f32) * (r / 1.0).powf(-0.5);
                        temp_chunk[i] = temp;
                        let (br, bg, bb) = blackbody_to_srgb(temp as f64);
                        let col = &mut col_chunk[i];
                        col[0] = (br * 0.4 + col[0] * 0.6).clamp(0.25, 1.0);
                        col[1] = (bg * 0.4 + col[1] * 0.6).clamp(0.2, 1.0);
                        col[2] = (bb * 0.4 + col[2] * 0.6).clamp(0.2, 1.0);
                        pos_chunk[i] = pos;
                        vel_chunk[i] = vel;
                    } else {
                        // Fast path: GPU compute already integrated position, velocity, and temperature!
                        let mut accreted = false;
                        for &(p_ent, p_pos, p_m, p_type) in &massive_bodies {
                            let is_major_body = p_m >= 0.01 * EARTH_MASS_SOLAR
                                && !matches!(p_type, BodyType::Asteroid | BodyType::Comet);

                            let pdx = pos[0] - p_pos.x as f32;
                            let pdz = pos[2] - p_pos.z as f32;
                            let p_dist = (pdx * pdx + pdz * pdz).sqrt().max(0.001);

                            let p_dist_au = p_pos.length() as f32;
                            let is_inner_terrestrial = p_dist_au < 2.7;
                            let warp_sweep = (1.0 + speed_mult.log10().max(0.0) * 0.30).min(2.2);
                            let is_massive_disk = star_m > 10.0;
                            let hill_r = p_dist_au * ((p_m / (3.0 * star_m as f64)).cbrt() as f32);
                            let bondi_r = if is_massive_disk {
                                (0.08 * (p_m / JUPITER_MASS_SOLAR).sqrt() as f32).clamp(0.05, 8.0)
                            } else {
                                0.0
                            };
                            let effective_grav_r = hill_r.max(bondi_r);
                            let physical_r = if p_m >= 0.08 {
                                (0.00465 * (p_m / 1.0).powf(0.8) as f32).clamp(0.004, 0.20)
                            } else {
                                (0.005 * (p_m / EARTH_MASS_SOLAR).cbrt() as f32).clamp(0.003, 0.040)
                            };

                            let acc_r = if is_major_body {
                                if is_inner_terrestrial && !is_massive_disk {
                                    ((physical_r + 0.50 * hill_r) * warp_sweep)
                                        .clamp(physical_r, 0.150)
                                } else if is_massive_disk {
                                    ((physical_r + 0.90 * effective_grav_r) * warp_sweep)
                                        .clamp(physical_r, 12.0)
                                } else {
                                    // Realistic accretion envelope for gas giants in normal star systems
                                    ((physical_r + 0.60 * hill_r) * warp_sweep)
                                        .clamp(physical_r, 0.350)
                                }
                            } else {
                                ((physical_r * 1.5) * warp_sweep).clamp(0.00005, 0.0015)
                            };

                            if p_dist < acc_r {
                                chunk_accretions.push((p_ent, m as f64));
                                mass_chunk[i] = 0.0;
                                pos_chunk[i] = [0.0, -5000.0, 0.0];
                                accreted = true;
                                break;
                            }
                        }
                        if accreted {
                            continue;
                        }

                        let temp = temp_chunk[i];
                        let (br, bg, bb) = blackbody_to_srgb(temp as f64);
                        let col = &mut col_chunk[i];
                        col[0] = (br * 0.4 + col[0] * 0.6).clamp(0.25, 1.0);
                        col[1] = (bg * 0.4 + col[1] * 0.6).clamp(0.2, 1.0);
                        col[2] = (bb * 0.4 + col[2] * 0.6).clamp(0.2, 1.0);
                    }
                }
                chunk_accretions
            },
        )
        .flatten()
        .collect();

    // 1B. Apply accumulated accreted particle masses to ECS Celestial Bodies & Trigger Realistic Planetary Growth
    let mut mass_gains: hashbrown::HashMap<Entity, f64> = hashbrown::HashMap::new();
    for (ent, delta_m) in accreted_events {
        *mass_gains.entry(ent).or_insert(0.0) += delta_m;
    }

    for (ent, pos, mut mass, mut radius, mut comp, mut body) in massive_query.iter_mut() {
        if let Some(&gain) = mass_gains.get(&ent) {
            if matches!(body.body_type, BodyType::Asteroid | BodyType::Comet) {
                // Minor asteroids only absorb small direct dust mass without runaway multiplier
                mass.0 = (mass.0 + gain).min(0.0005 * EARTH_MASS_SOLAR);
                let avg_density = comp.average_density();
                radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI)).cbrt();
            } else {
                let r_au = pos.0.length();
                let is_beyond_snowline = r_au >= 2.7;
                let is_massive_disk = star_mass.0 > 10.0;

                // Planets, Protoplanetary embryos, and Stars
                if !is_beyond_snowline && !is_massive_disk {
                    // Terrestrial Feeding Zone Isolation Mass Limit (Solar Nebula):
                    // Inside 2.7 AU, the total solid mass in a terrestrial feeding zone is physically limited (~1.0 - 1.05 M_earth).
                    // Once a terrestrial embryo reaches the isolation mass, local dust is depleted.
                    if mass.0 < 1.05 * EARTH_MASS_SOLAR {
                        let m_earth_ratio = (mass.0 / EARTH_MASS_SOLAR).clamp(0.1, 1.0);
                        let runaway_mult = 1.0 + 0.35 * m_earth_ratio;
                        mass.0 = (mass.0 + gain * runaway_mult).min(1.02 * EARTH_MASS_SOLAR);
                    }
                } else if !is_massive_disk {
                    // Outer giant planet in normal planetary system:
                    // Cannot exceed gas gap-opening limit (e.g. 2.5 M_Jup = 0.00238 M_sun)
                    let max_giant_mass = 2.5 * JUPITER_MASS_SOLAR;
                    if mass.0 < max_giant_mass {
                        let m_earth_ratio = (mass.0 / EARTH_MASS_SOLAR).clamp(0.1, 350.0);
                        let runaway_mult = 1.0 + 0.30 * m_earth_ratio.powf(0.35);
                        mass.0 = (mass.0 + gain * runaway_mult).min(max_giant_mass);
                    }
                } else {
                    // Circum-nuclear accretion in supermassive disks (Little Red Dot [F6]):
                    // Stellar-mass seeds can grow into Pop-III hypergiants
                    let m_earth_ratio = (mass.0 / EARTH_MASS_SOLAR).max(0.1);
                    let runaway_mult = 1.0 + 0.65 * m_earth_ratio.powf(0.65);
                    mass.0 += gain * runaway_mult;
                }

                // Update physical radius based on whether it is a planet or star
                let new_radius = if mass.0 >= 0.08 {
                    // Main-sequence / Giant star radius: R ~ R_sun * (M / M_sun)^0.8
                    (0.00465 * (mass.0 / 1.0).powf(0.8)).clamp(0.003, 10.0)
                } else {
                    let avg_density = comp.average_density();
                    ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                        .cbrt()
                        .max(EARTH_RADIUS_AU * 0.2)
                };
                radius.0 = new_radius;

                // Dynamic promotion and classification as bodies grow
                // Jovian gas envelope runaway accretion beyond the snow line
                if is_beyond_snowline && mass.0 >= 6.0 * EARTH_MASS_SOLAR && comp.gas_frac < 0.40 {
                    comp.gas_frac = (comp.gas_frac + 0.08).min(0.92);
                    comp.ice_frac = (comp.ice_frac * 0.90).max(0.04);
                }
                if mass.0 >= 13.0 * JUPITER_MASS_SOLAR {
                    comp.gas_frac = (comp.gas_frac + 0.15).min(0.99);
                }

                // Terrestrial planets and Super-Earths inside the snow line maintain thin secondary atmospheres
                if !is_beyond_snowline && !is_massive_disk && (mass.0 >= 0.02 * EARTH_MASS_SOLAR) {
                    comp.gas_frac = comp.gas_frac.clamp(0.015, 0.025);
                }

                // Centralized, physically and materially accurate classification
                let updated_type = crate::simulation::components::classify_body_by_mass_and_comp(
                    mass.0, &comp, false,
                );
                body.body_type = updated_type;

                // Dynamic naming across the full spectrum from planetesimals to Pop-III stars
                body.name = match updated_type {
                    BodyType::Hypergiant => format!("Pop-III Hypergiant ({:.1} M☉)", mass.0),
                    BodyType::BlueSupergiant => {
                        format!("Pop-III Blue Supergiant ({:.1} M☉)", mass.0)
                    }
                    BodyType::BlueGiant => format!("Pop-III Blue Giant ({:.1} M☉)", mass.0),
                    BodyType::YellowDwarf => format!("Pop-III Yellow Star ({:.2} M☉)", mass.0),
                    BodyType::RedDwarf => format!("Red Dwarf ({:.2} M☉)", mass.0),
                    BodyType::BrownDwarf => {
                        format!("Brown Dwarf ({:.1} M_J)", mass.0 / JUPITER_MASS_SOLAR)
                    }
                    BodyType::GasGiant => {
                        if mass.0 >= JUPITER_MASS_SOLAR {
                            format!("Super-Jupiter ({:.1} M_J)", mass.0 / JUPITER_MASS_SOLAR)
                        } else {
                            format!("Planet-{:.0}AU (Gas Giant)", r_au)
                        }
                    }
                    BodyType::IceGiant => format!("Planet-{:.0}AU (Ice Giant)", r_au),
                    BodyType::SuperEarth => format!("Planet-{:.0}AU (Super-Earth)", r_au),
                    BodyType::TerrestrialPlanet => format!("Planet-{:.0}AU (Terrestrial)", r_au),
                    BodyType::Protoplanet => format!("Protoplanet-{:.0}AU", r_au),
                    BodyType::Planetesimal => format!("Planetesimal-{:.0}AU", r_au),
                    BodyType::Comet => format!("Comet-{:.0}AU", r_au * 10.0),
                    _ => body.name.clone(),
                };
            }
        }
    }

    let r_min = disk_params.inner_radius_au as f32;
    let r_max = disk_params.outer_radius_au as f32;
    let r_span = (r_max - r_min).max(1.0);
    bin_heads.fill(-1);
    for i in 0..n {
        if masses[i] <= 0.0 {
            continue;
        }
        let pos = positions[i];
        let r = (pos[0] * pos[0] + pos[2] * pos[2]).sqrt();
        let phi = pos[2].atan2(pos[0]).rem_euclid(2.0 * PI as f32);
        let r_norm = ((r - r_min) / r_span).clamp(0.0, 0.999);
        let phi_norm = (phi / (2.0 * PI as f32)).clamp(0.0, 0.999);
        let bin_idx = ((r_norm * 64.0) as usize) * 64 + ((phi_norm * 64.0) as usize);
        bin_next[i] = bin_heads[bin_idx];
        bin_heads[bin_idx] = i as i32;
    }

    // 2. In-cell and adjacent-cell 2D spatial hash particle collisions and gravitational sticking
    for r_b in 0..64usize {
        for p_b in 0..64usize {
            let bin_idx = r_b * 64 + p_b;
            let mut curr = bin_heads[bin_idx];

            while curr >= 0 {
                let idx_a = curr as usize;
                let mut neighbor = bin_next[idx_a];
                let mut depth = 0;

                while neighbor >= 0 && depth < 8 {
                    let idx_b = neighbor as usize;

                    if masses[idx_a] > 0.0 && masses[idx_b] > 0.0 {
                        let pos_a = positions[idx_a];
                        let pos_b = positions[idx_b];
                        let dx = pos_a[0] - pos_b[0];
                        let dy = pos_a[1] - pos_b[1];
                        let dz = pos_a[2] - pos_b[2];
                        let dist_sq = dx * dx + dy * dy + dz * dz;

                        let r_body = (pos_a[0] * pos_a[0] + pos_a[2] * pos_a[2]).sqrt();
                        let is_beyond_snowline = r_body > disk_params.snow_line_au as f32;

                        // Sticky ice enhancement + outer zone collision compensation
                        let sticky_boost = if is_beyond_snowline {
                            2.5 * (1.0 + compositions[idx_a].ice_frac as f32 * 1.5)
                        } else {
                            1.0
                        };
                        let zone_boost = (r_body / 1.0).powf(0.55).clamp(1.0, 4.5);

                        // Physical accretion cross-section scaling with mass (cube root) and high time warp compensation
                        let mass_factor = (masses[idx_a] / b_mass).cbrt().clamp(1.0, 6.0);
                        let warp_stick_boost = (1.0 + speed_mult.log10().max(0.0) * 0.22).min(1.8);
                        let r_acc =
                            (0.012 * sticky_boost * zone_boost * mass_factor * warp_stick_boost)
                                .clamp(0.005, 0.080);

                        if dist_sq < r_acc * r_acc {
                            let vel_a = velocities[idx_a];
                            let vel_b = velocities[idx_b];
                            let dvx = vel_a[0] - vel_b[0];
                            let dvy = vel_a[1] - vel_b[1];
                            let dvz = vel_a[2] - vel_b[2];
                            let v_rel_sq = dvx * dvx + dvy * dvy + dvz * dvz;

                            // Velocity threshold for sticking vs bounce: allows orbital coplanar mergers
                            let v_crit = if is_beyond_snowline { 0.85 } else { 0.50 };

                            if v_rel_sq < v_crit * v_crit {
                                let mut rng = rand::rng();
                                let stick_prob = if is_beyond_snowline { 0.90 } else { 0.75 };

                                if rng.random_range(0.0..1.0f32) < stick_prob {
                                    let m_a = masses[idx_a];
                                    let m_b = masses[idx_b];

                                    // 1. Inelastic Momentum Conservation
                                    let merged_vel = [
                                        (vel_a[0] * m_a + vel_b[0] * m_b) / (m_a + m_b),
                                        (vel_a[1] * m_a + vel_b[1] * m_b) / (m_a + m_b),
                                        (vel_a[2] * m_a + vel_b[2] * m_b) / (m_a + m_b),
                                    ];

                                    // 2. Mass-weighted composition mixing
                                    let comp_a = compositions[idx_a];
                                    let comp_b = compositions[idx_b];
                                    let mut merged_comp =
                                        comp_a.mass_weighted_merge(m_a as f64, &comp_b, m_b as f64);

                                    // Gas envelope capture for growing giant cores in the outer reservoir
                                    if is_beyond_snowline && (m_a + m_b) > (2.0 * b_mass) {
                                        merged_comp.gas_frac =
                                            (merged_comp.gas_frac + 0.06).min(0.92);
                                        merged_comp.ice_frac =
                                            (merged_comp.ice_frac * 0.95).max(0.05);
                                    }

                                    // 3. Impact heating
                                    let merged_temp =
                                        temperatures[idx_a].max(temperatures[idx_b]) + 75.0;

                                    // 4. Update surviving particle A
                                    masses[idx_a] += m_b;
                                    velocities[idx_a] = merged_vel;
                                    compositions[idx_a] = merged_comp;
                                    temperatures[idx_a] = merged_temp;

                                    let (br, bg, bb) = blackbody_to_srgb(merged_temp as f64);
                                    let (cr, cg, cb) = merged_comp.visual_color_tint();
                                    colors[idx_a] = [
                                        (br * 0.4 + cr * 0.85).clamp(0.4, 1.4),
                                        (bg * 0.4 + cg * 0.85).clamp(0.35, 1.4),
                                        (bb * 0.4 + cb * 0.85).clamp(0.3, 1.4),
                                        1.0,
                                    ];

                                    // 5. Zero out particle B
                                    masses[idx_b] = 0.0;
                                    positions[idx_b] = [0.0, -5000.0, 0.0];
                                    velocities[idx_b] = [0.0, 0.0, 0.0];
                                }
                            }
                        }
                    }
                    neighbor = bin_next[neighbor as usize];
                    depth += 1;
                }
                curr = bin_next[curr as usize];
            }
        }
    }

    // 3. Promotion to ECS Massive Body (For runaway clumps that reach 8x initial particle mass)
    let promo_threshold = 8.0 * b_mass;
    let mut promotions: Vec<(DVec3, DVec3, f64, f64, Composition)> = Vec::new();
    let mut active_count = 0u32;
    for i in 0..n {
        let m = masses[i];
        if m > 0.0 {
            active_count += 1;
        }
        if m >= promo_threshold && promotions.is_empty() {
            let pos = positions[i];
            let r_sq = pos[0] * pos[0] + pos[2] * pos[2];
            let r = r_sq.sqrt();
            let is_massive_disk = star_m > 10.0;
            let min_r = if is_massive_disk { 65.0 } else { 0.15 };
            if r < min_r || pos[1] < -1000.0 {
                masses[i] = 0.0;
                positions[i] = [0.0, -5000.0, 0.0];
                continue;
            }

            let m_f64 = m as f64;
            let avg_density = compositions[i].average_density();
            let rad_f64 = ((3.0 * m_f64 / avg_density) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.2);

            let vel = velocities[i];
            promotions.push((
                DVec3::new(pos[0] as f64, pos[1] as f64, pos[2] as f64),
                DVec3::new(vel[0] as f64, vel[1] as f64, vel[2] as f64),
                m_f64,
                rad_f64,
                compositions[i],
            ));

            // Reset particle in swarm
            masses[i] = 0.0;
            positions[i] = [0.0, -5000.0, 0.0];
            active_count = active_count.saturating_sub(1);
        }
    }

    // 4. Spawn ECS entities for promoted embryos (visuals and PlanetMaterial automatically handled by bodies.rs)
    for (pos, vel, mass, radius, comp) in promotions {
        let r_dist = pos.length();
        let body_type =
            crate::simulation::components::classify_body_by_mass_and_comp(mass, &comp, false);
        let name = match body_type {
            BodyType::BrownDwarf => format!("Brown Dwarf ({:.1} M_J)", mass / JUPITER_MASS_SOLAR),
            BodyType::GasGiant => format!("Planet-{:.0}AU (Gas Giant)", r_dist),
            BodyType::IceGiant => format!("Planet-{:.0}AU (Ice Giant)", r_dist),
            BodyType::SuperEarth => format!("Planet-{:.0}AU (Super-Earth)", r_dist),
            BodyType::TerrestrialPlanet => format!("Planet-{:.0}AU (Terrestrial)", r_dist),
            BodyType::Protoplanet => format!("Protoplanet-{:.0}AU", r_dist),
            BodyType::Planetesimal => format!("Planetesimal-{:.0}AU", r_dist),
            BodyType::Comet => format!("Comet-{:.0}AU", r_dist * 10.0),
            _ => format!("Asteroid-{:.0}AU", r_dist * 10.0),
        };
        let temp = (disk_params.reference_temp_1au) * (r_dist / 1.0).powf(-0.5);

        let mut diff = InternalDifferentiation::default();
        diff.recalculate(mass, radius, &comp);

        let mut spin = SpinState::default();
        let initial_spin =
            (mass * radius * radius * 0.33) * DVec3::new(0.0, 2.0 * PI / (24.0 / 8766.0), 0.0);
        spin.update_from_spin(initial_spin, mass, radius);

        let vol = VolatileInventory {
            delivered_water_m_earth: 0.0,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: if r_dist < 2.7 { 0.5 } else { 0.0 },
            cometary_impact_count: 0,
        };

        commands.spawn((
            CelestialBody { body_type, name },
            Mass(mass),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(radius),
            Temperature(temp),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * mass),
            comp,
            diff,
            spin,
            vol,
        ));
    }

    // 5. Continuous Smooth Debris Recycling & Swarm Maintenance
    // ONLY replenish during early nebular phase while gas density is high and star has not ignited!
    if config.gas_density_scale > 0.15
        && !ignition.is_ignited
        && active_count < config.active_particles
    {
        let missing = config.active_particles.saturating_sub(active_count);
        let mut rng = rand::rng();
        let mut replenished = 0u32;
        let star_mass_f64 = disk_params.central_star_mass;
        let replenish_scale = 1.0 + (speed_mult.log10().max(0.0) * 1.25);
        let max_replenish = missing.min((128.0 * replenish_scale) as u32);

        for i in 0..n {
            if masses[i] <= 0.0 && replenished < max_replenish {
                let (r_spawn, comp) = sample_disk_radius(&mut rng, &disk_params);
                let phi = rng.random_range(0.0..2.0 * PI);
                let v_k = (G_ASTRO * star_mass_f64 / r_spawn).sqrt();
                let v_disp = rng.random_range(-0.02..0.02) * v_k;
                let v_tot = (v_k + v_disp) as f32;

                positions[i] = [
                    (r_spawn * phi.cos()) as f32,
                    rng.random_range(-0.015..0.015) as f32,
                    (r_spawn * phi.sin()) as f32,
                ];
                velocities[i] = [-v_tot * phi.sin() as f32, 0.0, v_tot * phi.cos() as f32];
                masses[i] = b_mass;
                compositions[i] = comp;
                temperatures[i] =
                    (disk_params.reference_temp_1au as f32) * (r_spawn as f32 / 1.0).powf(-0.5);

                replenished += 1;
                active_count += 1;
            }
        }
    }

    config.active_particles = active_count;

    // 6. Zero-allocation in-place camera-facing billboard quad vertex buffer update
    let base_render_r = 0.080 * p_render_scale;

    mesh_positions
        .par_chunks_mut(4 * 2048)
        .zip(mesh_colors.par_chunks_mut(4 * 2048))
        .enumerate()
        .for_each(|(chunk_idx, (pos_out, col_out))| {
            let start_particle = chunk_idx * 2048;
            let end_particle = (start_particle + 2048).min(n);

            for local_i in start_particle..end_particle {
                let v_base = (local_i - start_particle) * 4;
                let m = masses[local_i];

                if m <= 0.0 {
                    pos_out[v_base] = [0.0, -5000.0, 0.0];
                    pos_out[v_base + 1] = [0.0, -5000.0, 0.0];
                    pos_out[v_base + 2] = [0.0, -5000.0, 0.0];
                    pos_out[v_base + 3] = [0.0, -5000.0, 0.0];
                    col_out[v_base] = [0.0, 0.0, 0.0, 0.0];
                    col_out[v_base + 1] = [0.0, 0.0, 0.0, 0.0];
                    col_out[v_base + 2] = [0.0, 0.0, 0.0, 0.0];
                    col_out[v_base + 3] = [0.0, 0.0, 0.0, 0.0];
                    continue;
                }

                let p = Vec3::from_array(positions[local_i]);
                let m_factor = (m / b_mass).cbrt().clamp(1.0, 8.0);
                let r_v = base_render_r * m_factor;

                let right = cam_right * r_v;
                let up = cam_up * r_v;

                pos_out[v_base] = (p - right - up).to_array();
                pos_out[v_base + 1] = (p + right - up).to_array();
                pos_out[v_base + 2] = (p + right + up).to_array();
                pos_out[v_base + 3] = (p - right + up).to_array();

                // Progressive opacity: fine dust (0.65) -> pebble (0.82) -> planetesimal (0.95+)
                let alpha = if m <= 1.1 * b_mass {
                    0.65f32
                } else if m <= 3.0 * b_mass {
                    0.82f32
                } else if m <= 8.0 * b_mass {
                    0.95f32
                } else {
                    1.0f32
                };

                let c = colors[local_i];
                let final_col = [c[0], c[1], c[2], alpha];
                col_out[v_base] = final_col;
                col_out[v_base + 1] = final_col;
                col_out[v_base + 2] = final_col;
                col_out[v_base + 3] = final_col;
            }
        });

    if let Some(mut mesh) = meshes.get_mut(mesh_handle) {
        if let Some(VertexAttributeValues::Float32x3(mesh_pos)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            mesh_pos.copy_from_slice(mesh_positions);
        }
        if let Some(VertexAttributeValues::Float32x4(mesh_col)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
        {
            mesh_col.copy_from_slice(mesh_colors);
        }
    }
}

/// Plugin registering the 50,000 particle visual swarm.
pub struct ParticleSwarmPlugin;

impl Plugin for ParticleSwarmPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_particle_swarm)
            .add_systems(Update, update_particle_swarm);
    }
}
