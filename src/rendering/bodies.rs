//! Visual mesh generation, PBR materials, and real-time transform synchronization.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, VertexAttributeValues};

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Marker for an entity that has its visual mesh and material spawned.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct VisualBody;

/// Common shared meshes and materials cache.
#[derive(Resource)]
pub struct VisualAssets {
    pub star_mesh: Handle<Mesh>,
    pub planet_mesh: Handle<Mesh>,
    pub asteroid_potato_mesh: Handle<Mesh>,
    pub asteroid_rubble_mesh: Handle<Mesh>,
    pub comet_bilobate_mesh: Handle<Mesh>,
    pub particle_mesh: Handle<Mesh>,
    pub ring_mesh: Handle<Mesh>,
    pub beam_core_mesh: Handle<Mesh>,
    pub beam_sheath_mesh: Handle<Mesh>,
    pub accretion_disk_mesh: Handle<Mesh>,
}

/// Recomputes smooth vertex normals for procedural meshes.
fn recompute_mesh_normals(mesh: &mut Mesh) {
    if let (Some(VertexAttributeValues::Float32x3(pos)), Some(Indices::U32(indices))) =
        (mesh.attribute(Mesh::ATTRIBUTE_POSITION), mesh.indices())
    {
        let mut normals = vec![Vec3::ZERO; pos.len()];
        for chunk in indices.chunks(3) {
            let i0 = chunk[0] as usize;
            let i1 = chunk[1] as usize;
            let i2 = chunk[2] as usize;
            let v0 = Vec3::from_array(pos[i0]);
            let v1 = Vec3::from_array(pos[i1]);
            let v2 = Vec3::from_array(pos[i2]);
            let normal = (v1 - v0).cross(v2 - v0);
            normals[i0] += normal;
            normals[i1] += normal;
            normals[i2] += normal;
        }
        let normalized: Vec<[f32; 3]> = normals
            .into_iter()
            .map(|n| n.normalize_or_zero().to_array())
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normalized);
    }
}

/// Procedurally generates an irregular, non-spherical 3D asteroid mesh with triaxial elongation,
/// multi-octave harmonic noise displacement, and realistic impact crater depressions.
fn generate_irregular_asteroid_mesh(
    elongation: Vec3,
    noise_strength: f32,
    is_bilobate: bool,
    seed: f32,
) -> Mesh {
    let mut sphere = Sphere::new(1.0).mesh().ico(4).unwrap();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        sphere.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return sphere;
    };

    let mut new_positions = Vec::with_capacity(positions.len());
    let craters = [
        (Vec3::new(0.6, 0.7, 0.3).normalize(), 0.55f32, 0.18f32),
        (Vec3::new(-0.7, 0.2, 0.65).normalize(), 0.45f32, 0.14f32),
        (Vec3::new(0.1, -0.9, 0.4).normalize(), 0.60f32, 0.20f32),
        (Vec3::new(-0.4, -0.4, -0.8).normalize(), 0.38f32, 0.12f32),
    ];

    for p in positions {
        let v = Vec3::from_array(*p).normalize();

        // 1. Bilobate contact binary deformation (e.g. 67P / Arrokoth)
        let base_pos = if is_bilobate {
            if v.x > 0.1 {
                let lobe1 =
                    (v - Vec3::new(0.45, 0.0, 0.0)).normalize() * 0.85 + Vec3::new(0.45, 0.0, 0.0);
                lobe1 * elongation
            } else if v.x < -0.1 {
                let lobe2 =
                    (v + Vec3::new(0.40, 0.0, 0.0)).normalize() * 0.70 - Vec3::new(0.40, 0.0, 0.0);
                lobe2 * elongation
            } else {
                v * elongation * 0.65 // Neck constriction
            }
        } else {
            v * elongation
        };

        // 2. Isotropic spherical surface harmonics (avoids Cartesian coordinate-axis cubic alignment)
        let k1 = v.dot(Vec3::new(0.577, 0.577, 0.577));
        let k2 = v.dot(Vec3::new(-0.707, 0.0, 0.707));
        let k3 = v.dot(Vec3::new(0.267, -0.802, 0.534));
        let k4 = v.dot(Vec3::new(-0.408, 0.816, -0.408));

        let d1 = (k1 * 3.2 + seed).sin() * 0.12;
        let d2 = (k2 * 5.4 + seed * 1.6).sin() * 0.06;
        let d3 = (k3 * 8.1 + seed * 2.4).sin() * 0.03;
        let d4 = (k4 * 12.3 + seed * 3.5).cos() * 0.015;
        let mut disp = 1.0 + (d1 + d2 + d3 + d4) * noise_strength;

        // 3. Impact Crater Depressions with Elevated Rims
        for &(c_center, c_rad, c_depth) in &craters {
            let angle = (v.dot(c_center)).clamp(-1.0, 1.0).acos();
            if angle < c_rad {
                let norm_dist = angle / c_rad;
                let crater_pit = c_depth * (1.0 - norm_dist * norm_dist);
                let rim_boost = (c_depth * 0.35) * (-((norm_dist - 0.95) / 0.15).powi(2)).exp();
                disp = disp - crater_pit + rim_boost;
            }
        }

        let final_p = base_pos * disp.max(0.2);
        new_positions.push(final_p.to_array());
    }

    sphere.insert_attribute(Mesh::ATTRIBUTE_POSITION, new_positions);
    recompute_mesh_normals(&mut sphere);
    sphere
}

pub fn setup_visual_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    // High-resolution icospheres with smooth vertex normals for flawless spherical silhouettes
    let star_mesh = meshes.add(Sphere::new(1.0).mesh().ico(6).unwrap());
    let planet_mesh = meshes.add(Sphere::new(1.0).mesh().ico(6).unwrap());
    let particle_mesh = meshes.add(Sphere::new(1.0).mesh().ico(4).unwrap());
    let ring_mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0).build());

    // Misshapen irregular asteroid & comet archetype meshes (natural triaxial ellipsoids without cubic box artifacts)
    let asteroid_potato =
        generate_irregular_asteroid_mesh(Vec3::new(1.18, 1.05, 0.92), 0.45, false, 1.25);
    let asteroid_rubble =
        generate_irregular_asteroid_mesh(Vec3::new(1.08, 0.96, 1.04), 0.35, false, 4.80);
    let comet_bilobate =
        generate_irregular_asteroid_mesh(Vec3::new(1.22, 0.90, 0.82), 0.45, true, 8.40);

    let asteroid_potato_mesh = meshes.add(asteroid_potato);
    let asteroid_rubble_mesh = meshes.add(asteroid_rubble);
    let comet_bilobate_mesh = meshes.add(comet_bilobate);

    let beam_core_mesh = meshes.add(Cylinder::new(1.0, 1.0));
    let beam_sheath_mesh = meshes.add(Cylinder::new(1.0, 1.0));
    let accretion_disk_mesh = meshes.add(Cylinder::new(1.0, 1.0));

    commands.insert_resource(VisualAssets {
        star_mesh,
        planet_mesh,
        asteroid_potato_mesh,
        asteroid_rubble_mesh,
        comet_bilobate_mesh,
        particle_mesh,
        ring_mesh,
        beam_core_mesh,
        beam_sheath_mesh,
        accretion_disk_mesh,
    });
}

/// Computes realistic astrophysical color palette for Gas Giants based on
/// mass tier (Jupiter vs Super-Jupiter vs Brown Dwarf) and equilibrium temperature.
pub fn compute_gas_giant_palette(mass_solar: f64, temp_k: f64, name: &str) -> Color {
    let mass_jup = mass_solar / crate::utils::constants::JUPITER_MASS_SOLAR;
    let lower = name.to_lowercase();

    // 1. Saturn Preset
    if lower.contains("saturn") {
        return Color::srgb(0.92, 0.82, 0.58); // Butterscotch golden-sand
    }

    // 2. Hot Jupiter (Sudarsky Class IV/V: Alkali / Silicate cloud hazes)
    if temp_k > 800.0 || lower.contains("hot jupiter") {
        return Color::srgb(0.38, 0.16, 0.10); // Fiery carbonaceous bronze/amber
    }

    // 3. Named Jupiter preset or standard 1.0 M_jup
    if (lower.contains("jupiter") && !lower.contains("super") && !lower.contains("hot"))
        || (0.7..=1.8).contains(&mass_jup)
    {
        return Color::srgb(0.86, 0.65, 0.42); // Iconic Jovian amber-ochre
    }

    // 4. Super-Jupiters by mass variations:
    if mass_jup > 12.0 {
        // Brown Dwarf Transition: Incandescent plum-maroon & dark violet
        Color::srgb(0.45, 0.12, 0.32)
    } else if mass_jup > 6.0 {
        // Heavy Super-Jupiter (6-12 M_jup): Royal Plum-Purple with Midnight Navy belts
        Color::srgb(0.32, 0.20, 0.48)
    } else if mass_jup > 3.5 {
        // Massive Super-Jupiter (3.5-6 M_jup): Deep Lapis-Indigo and Sapphire-Cyan
        Color::srgb(0.16, 0.36, 0.62)
    } else if mass_jup > 1.8 {
        // Super-Jupiter (1.8-3.5 M_jup): Exotic Emerald-Teal & Aquamarine
        Color::srgb(0.18, 0.52, 0.50)
    } else if mass_jup < 0.6 {
        // Sub-Saturn / Warm Gas Dwarf: Pale Cream-Straw
        Color::srgb(0.85, 0.78, 0.56)
    } else {
        // Standard Jupiter size: Classic Jovian ochre-amber
        Color::srgb(0.86, 0.65, 0.42)
    }
}

/// Spawns visual meshes and point lights for newly created celestial bodies.
pub fn spawn_missing_visuals(
    mut commands: Commands,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    unspawned_query: Query<
        (
            Entity,
            &SimPosition,
            &Mass,
            &Radius,
            &Temperature,
            &Composition,
            &CelestialBody,
            Option<&CentralStar>,
        ),
        Without<VisualBody>,
    >,
) {
    for (entity, pos, mass, radius, temp, comp, body, is_star) in unspawned_query.iter() {
        let (br, bg, bb) = blackbody_to_srgb(temp.0);
        let (cr, cg, cb) = comp.visual_color_tint();

        let is_gas_giant =
            body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.30;
        let base_color = if is_star.is_some() {
            Color::srgb(br, bg, bb)
        } else if is_gas_giant {
            compute_gas_giant_palette(mass.0, temp.0, &body.name)
        } else {
            Color::srgb(
                (br * 0.25 + cr * 0.75).clamp(0.1, 1.0),
                (bg * 0.25 + cg * 0.75).clamp(0.1, 1.0),
                (bb * 0.25 + cb * 0.75).clamp(0.1, 1.0),
            )
        };

        let trans = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);

        let Ok(mut entity_cmd) = commands.get_entity(entity) else {
            continue;
        };

        let visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);

        if is_star.is_some() || body.body_type.is_star_or_remnant() {
            // Central Star & Stellar Remnants: Emissive unlit glow + point light
            let (p_type, unlit_flag, emissive_val) = match body.body_type {
                BodyType::BlackHole => (5u32, false, LinearRgba::BLACK),
                BodyType::QuasiStar => (
                    0u32,
                    true,
                    LinearRgba::from(Color::srgb(1.2, 0.18, 0.08)) * 32.0,
                ),
                BodyType::WhiteDwarf => (0u32, true, LinearRgba::from(base_color) * 35.0),
                BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => {
                    (0u32, true, LinearRgba::from(base_color) * 45.0)
                }
                _ => (0u32, true, LinearRgba::from(base_color) * 25.0),
            };

            let material = materials.add(PlanetMaterial {
                base: StandardMaterial {
                    base_color,
                    emissive: emissive_val,
                    unlit: unlit_flag,
                    ..default()
                },
                extension: PlanetMaterialExtension {
                    uniforms: PlanetUniforms {
                        planet_type: p_type,
                        temperature: temp.0 as f32,
                        time: 0.0,
                        spin_rate: 0.15,
                        composition: Vec4::new(0.0, 0.0, 0.0, 1.0),
                        color_seed: LinearRgba::from(base_color).to_vec4(),
                        climate_and_bio: Vec4::ZERO,
                        atmosphere_params: Vec4::ZERO,
                        dynamics_and_mag: Vec4::ZERO,
                    },
                },
            });

            entity_cmd
                .try_insert((
                    VisualBody,
                    Mesh3d(visual_assets.star_mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(trans).with_scale(Vec3::splat(visual_radius)),
                    Visibility::default(),
                ))
                .with_children(|parent| {
                    // Omnidirectional solar illumination
                    parent.spawn((
                        PointLight {
                            color: base_color,
                            intensity: if body.body_type == BodyType::BlackHole {
                                1_500_000.0
                            } else if body.body_type == BodyType::QuasiStar {
                                15_000_000.0
                            } else {
                                2_500_000.0
                            },
                            range: 500.0,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::IDENTITY,
                    ));
                });
        } else {
            // Planets / Protoplanets / Planetesimals: Composition-tailored PBR materials
            let norm_comp = comp.normalized();
            let (metallic, roughness) = if norm_comp.metal_frac > 0.4 {
                (0.85, 0.25)
            } else if norm_comp.ice_frac > 0.4 {
                (0.05, 0.18)
            } else if norm_comp.gas_frac > 0.5 {
                (0.0, 0.85)
            } else {
                (0.15, 0.75)
            };

            let emissive = if temp.0 > 600.0 {
                LinearRgba::from(base_color) * ((temp.0 as f32 - 600.0) / 600.0).clamp(0.0, 5.0)
            } else {
                LinearRgba::BLACK
            };

            let material = materials.add(PlanetMaterial {
                base: StandardMaterial {
                    base_color,
                    metallic,
                    perceptual_roughness: roughness,
                    emissive,
                    ..default()
                },
                extension: PlanetMaterialExtension {
                    uniforms: PlanetUniforms {
                        planet_type: match body.body_type {
                            BodyType::GasGiant => 1,
                            BodyType::IceGiant => 2,
                            BodyType::SuperEarth => 6,
                            BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
                                if norm_comp.ice_frac > 0.40 {
                                    2 // Ice Giant / Icy world
                                } else {
                                    3 // Terrestrial
                                }
                            }
                            _ => 4, // Moon / Asteroid / Comet / Planetesimal
                        },
                        temperature: temp.0 as f32,
                        time: 0.0,
                        spin_rate: 0.15,
                        composition: Vec4::new(
                            norm_comp.silicate_frac as f32 + norm_comp.organics_frac as f32,
                            norm_comp.ice_frac as f32,
                            norm_comp.metal_frac as f32,
                            norm_comp.gas_frac as f32,
                        ),
                        color_seed: LinearRgba::from(base_color).to_vec4(),
                        climate_and_bio: Vec4::ZERO,
                        atmosphere_params: Vec4::new(
                            (norm_comp.gas_frac as f32 * 2.0).max(0.01),
                            0.08,
                            norm_comp.gas_frac as f32,
                            1.0,
                        ),
                        dynamics_and_mag: Vec4::new(
                            0.0,
                            0.0,
                            (mass.0 / crate::utils::constants::JUPITER_MASS_SOLAR) as f32,
                            0.08,
                        ),
                    },
                },
            });

            let mesh_handle = match body.body_type {
                BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::SuperEarth
                | BodyType::TerrestrialPlanet
                | BodyType::Protoplanet
                | BodyType::Planetesimal
                | BodyType::Moon => visual_assets.planet_mesh.clone(),
                BodyType::Comet => visual_assets.comet_bilobate_mesh.clone(),
                BodyType::Asteroid => {
                    let hash: usize = body.name.bytes().map(|b| b as usize).sum();
                    if hash.is_multiple_of(2) {
                        visual_assets.asteroid_potato_mesh.clone()
                    } else {
                        visual_assets.asteroid_rubble_mesh.clone()
                    }
                }
                BodyType::DustGrain => visual_assets.particle_mesh.clone(),
                _ => visual_assets.planet_mesh.clone(),
            };

            entity_cmd.try_insert((
                VisualBody,
                Mesh3d(mesh_handle),
                MeshMaterial3d(material),
                Transform::from_translation(trans).with_scale(Vec3::splat(visual_radius)),
                Visibility::default(),
            ));
        }
    }
}

/// Updates 3D translations, scales, and emissive temperatures of celestial bodies each frame.
pub fn sync_celestial_transforms(
    time: Res<Time>,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    mut light_query: Query<&mut PointLight>,
    mut query: Query<(
        &SimPosition,
        &Mass,
        &Radius,
        &Temperature,
        &Composition,
        &CelestialBody,
        &mut Transform,
        &MeshMaterial3d<PlanetMaterial>,
        &mut Mesh3d,
        Option<&PlanetaryClimate>,
        Option<&BiosphereState>,
        Option<&VolatileInventory>,
        Option<&SpinState>,
        Option<&ElectromagneticFieldState>,
        Option<&Children>,
    )>,
) {
    for (
        pos,
        mass,
        radius,
        temp,
        comp,
        body,
        mut transform,
        mat_handle,
        mut mesh,
        opt_climate,
        opt_bio,
        opt_vol,
        opt_spin,
        opt_em,
        opt_children,
    ) in query.iter_mut()
    {
        transform.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);

        // Black Hole emits zero omnidirectional point light into space
        if let Some(children) = opt_children {
            for child in children.iter() {
                if let Ok(mut light) = light_query.get_mut(child) {
                    if body.body_type == BodyType::BlackHole {
                        light.intensity = 1_500_000.0;
                    } else if body.body_type == BodyType::QuasiStar {
                        light.intensity = 15_000_000.0;
                    }
                }
            }
        }

        // Unified visual scaling with minor body downscaling for realistic belt proportions
        let visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
        transform.scale = Vec3::splat(visual_radius);

        // Sync Mesh Level of Detail (LOD) based on Body Type
        let target_mesh = if body.body_type.is_star_or_remnant() {
            visual_assets.star_mesh.clone()
        } else {
            match body.body_type {
                BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::SuperEarth
                | BodyType::TerrestrialPlanet
                | BodyType::Protoplanet
                | BodyType::Planetesimal
                | BodyType::Moon => visual_assets.planet_mesh.clone(),
                BodyType::Comet => visual_assets.comet_bilobate_mesh.clone(),
                BodyType::Asteroid => {
                    let hash: usize = body.name.bytes().map(|b| b as usize).sum();
                    if hash.is_multiple_of(2) {
                        visual_assets.asteroid_potato_mesh.clone()
                    } else {
                        visual_assets.asteroid_rubble_mesh.clone()
                    }
                }
                BodyType::DustGrain => visual_assets.particle_mesh.clone(),
                _ => visual_assets.planet_mesh.clone(),
            }
        };
        if mesh.0 != target_mesh {
            mesh.0 = target_mesh;
        }

        // Update material emissive color and PBR properties dynamically
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            let (br, bg, bb) = blackbody_to_srgb(temp.0);
            let (cr, cg, cb) = comp.visual_color_tint();

            let is_star_like = body.body_type.is_star_or_remnant();
            let is_gas_giant =
                body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.30;
            let color = if is_star_like {
                if body.body_type == BodyType::BlackHole {
                    Color::srgb(0.01, 0.01, 0.01)
                } else if body.body_type == BodyType::QuasiStar {
                    Color::srgb(1.0, 0.28, 0.10)
                } else {
                    Color::srgb(br, bg, bb)
                }
            } else if is_gas_giant {
                compute_gas_giant_palette(mass.0, temp.0, &body.name)
            } else {
                Color::srgb(
                    (br * 0.25 + cr * 0.75).clamp(0.1, 1.0),
                    (bg * 0.25 + cg * 0.75).clamp(0.1, 1.0),
                    (bb * 0.25 + cb * 0.75).clamp(0.1, 1.0),
                )
            };
            let norm_comp = comp.normalized();
            let ocean_frac = opt_vol
                .map(|v| v.ocean_coverage_frac)
                .unwrap_or(norm_comp.ice_frac as f32);
            let ice_frac = opt_climate.map(|c| c.ice_coverage_frac).unwrap_or(0.0);
            let biomass_frac = opt_bio.map(|b| b.biomass_coverage_frac).unwrap_or(0.0);
            let cloud_density = opt_climate
                .map(|c| c.cloud_coverage_frac)
                .unwrap_or(norm_comp.gas_frac as f32);

            let spin_rate = opt_spin
                .map(|s| (24.0 / s.rotation_period_hours.max(0.1)) as f32 * 0.15)
                .unwrap_or(0.15);
            let axial_tilt = opt_spin
                .map(|s| (s.axial_tilt_degrees as f32).to_radians())
                .unwrap_or(0.08);

            let pressure_bar = opt_vol
                .map(|v| v.atmospheric_pressure_bar)
                .unwrap_or((norm_comp.gas_frac as f32 * 2.0).max(0.01));
            let scale_height = (0.08 * (temp.0 as f32 / 288.0).sqrt()).clamp(0.02, 0.25);
            let haze_density = opt_climate
                .map(|c| (c.cloud_coverage_frac * 1.2).clamp(0.0, 1.0))
                .unwrap_or((norm_comp.gas_frac as f32 * 1.5).clamp(0.0, 1.0));
            let greenhouse = opt_climate.map(|c| c.greenhouse_delta_k).unwrap_or(33.0);

            let mag_gauss = opt_em.map(|e| e.magnetic_field_gauss as f32).unwrap_or(0.0);
            let lava_frac = if temp.0 > 600.0 {
                ((temp.0 as f32 - 600.0) / 900.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mass_jup = (mass.0 / crate::utils::constants::JUPITER_MASS_SOLAR) as f32;

            mat.base.base_color = color;
            mat.extension.uniforms.color_seed = LinearRgba::from(color).to_vec4();
            mat.extension.uniforms.temperature = temp.0 as f32;
            mat.extension.uniforms.time = time.elapsed_secs();
            mat.extension.uniforms.spin_rate = spin_rate;
            mat.extension.uniforms.composition = Vec4::new(
                norm_comp.silicate_frac as f32 + norm_comp.organics_frac as f32,
                norm_comp.ice_frac as f32,
                norm_comp.metal_frac as f32,
                norm_comp.gas_frac as f32,
            );
            mat.extension.uniforms.climate_and_bio =
                Vec4::new(ocean_frac, ice_frac, biomass_frac, cloud_density);
            mat.extension.uniforms.atmosphere_params =
                Vec4::new(pressure_bar, scale_height, haze_density, greenhouse);
            mat.extension.uniforms.dynamics_and_mag =
                Vec4::new(mag_gauss, lava_frac, mass_jup, axial_tilt);

            if is_star_like {
                if body.body_type == BodyType::BlackHole {
                    mat.extension.uniforms.planet_type = 5;
                    mat.base.unlit = false;
                    mat.base.emissive = LinearRgba::BLACK;
                } else {
                    mat.extension.uniforms.planet_type = 0;
                    mat.base.unlit = true;
                    let mult = match body.body_type {
                        BodyType::WhiteDwarf => 35.0,
                        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => 45.0,
                        BodyType::Protostar => 14.0,
                        BodyType::QuasiStar => 65.0,
                        _ => 30.0,
                    };
                    mat.base.emissive = LinearRgba::from(color) * mult;
                }
            } else {
                mat.base.unlit = false;
                mat.extension.uniforms.planet_type = match body.body_type {
                    BodyType::GasGiant => 1,
                    BodyType::IceGiant => 2,
                    BodyType::SuperEarth => 6,
                    BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
                        if norm_comp.ice_frac > 0.40 {
                            2
                        } else {
                            3
                        }
                    }
                    _ => 4,
                };

                if comp.metal_frac > 0.4 {
                    mat.base.metallic = 0.85;
                    mat.base.perceptual_roughness = 0.25;
                } else if comp.ice_frac > 0.4 {
                    mat.base.metallic = 0.05;
                    mat.base.perceptual_roughness = 0.18;
                } else if comp.gas_frac > 0.5 {
                    mat.base.metallic = 0.0;
                    mat.base.perceptual_roughness = 0.85;
                } else {
                    mat.base.metallic = 0.15;
                    mat.base.perceptual_roughness = 0.75;
                }

                if temp.0 > 600.0 {
                    mat.base.emissive =
                        LinearRgba::from(color) * ((temp.0 as f32 - 600.0) / 600.0).clamp(0.0, 5.0);
                } else {
                    mat.base.emissive = LinearRgba::BLACK;
                }
            }
        }
    }
}

/// Marker component for an instantiated visual planetary ring entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualRingChild;

/// Synchronizes 3D planetary ring system meshes, materials, and axial tilt transforms.
pub fn sync_planetary_rings(
    mut commands: Commands,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut ring_materials: ResMut<Assets<RingMaterial>>,
    planets_with_rings_query: Query<(
        Entity,
        &PlanetaryRingSystem,
        &Radius,
        &CelestialBody,
        Option<&SpinState>,
        Option<&Children>,
    )>,
    mut ring_children_query: Query<
        (&mut Transform, &MeshMaterial3d<RingMaterial>),
        With<VisualRingChild>,
    >,
) {
    for (planet_entity, ring_sys, radius, _body, opt_spin, opt_children) in
        planets_with_rings_query.iter()
    {
        let planet_render_rad = config.calc_visual_radius(radius.0);
        let ring_outer_scale = (planet_render_rad * 2.85).max(0.015);

        let tilt_degrees = opt_spin
            .map(|s| s.axial_tilt_degrees as f32)
            .unwrap_or(26.7);
        let ring_rotation = Quat::from_rotation_z(tilt_degrees.to_radians());

        let mut found_child = false;
        if let Some(children) = opt_children {
            for child in children.iter() {
                if let Ok((mut transform, mat_handle)) = ring_children_query.get_mut(child) {
                    found_child = true;
                    transform.scale = Vec3::splat(ring_outer_scale);
                    transform.rotation = ring_rotation;

                    if let Some(mut mat) = ring_materials.get_mut(&mat_handle.0) {
                        mat.uniforms.inner_radius = ring_sys.inner_radius_au;
                        mat.uniforms.outer_radius = ring_sys.outer_radius_au;
                        mat.uniforms.optical_depth = ring_sys.optical_depth;
                        mat.uniforms.ice_fraction = ring_sys.ice_fraction;
                        mat.uniforms.ring_color = calc_ring_color(ring_sys.ice_fraction);
                    }
                }
            }
        }

        if !found_child {
            let ring_color = calc_ring_color(ring_sys.ice_fraction);
            let material = ring_materials.add(RingMaterial {
                uniforms: RingUniforms {
                    inner_radius: ring_sys.inner_radius_au,
                    outer_radius: ring_sys.outer_radius_au,
                    optical_depth: ring_sys.optical_depth,
                    ice_fraction: ring_sys.ice_fraction,
                    ring_color,
                },
            });

            if let Ok(mut p_cmd) = commands.get_entity(planet_entity) {
                p_cmd.with_children(|parent| {
                    parent.spawn((
                        VisualRingChild,
                        Mesh3d(visual_assets.ring_mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::from_scale(Vec3::splat(ring_outer_scale))
                            .with_rotation(ring_rotation),
                        NotShadowCaster,
                    ));
                });
            }
        }
    }
}

/// Dynamically calculates ring albedo and tone based on water ice vs silicate/metal composition.
fn calc_ring_color(ice_fraction: f32) -> Vec4 {
    if ice_fraction >= 0.70 {
        // High ice fraction (>= 70%): brilliant silver-white (Saturn-like)
        Vec4::new(0.96, 0.97, 1.0, 0.95)
    } else if ice_fraction >= 0.35 {
        // Mixed ice & dust (35-70%): warm sand-cream tone
        Vec4::new(0.85, 0.78, 0.68, 0.85)
    } else {
        // Silicate / carbonaceous (< 35%): dark anthracite / charcoal (Uranus / Jovian-like)
        Vec4::new(0.38, 0.35, 0.32, 0.65)
    }
}

/// Root marker for the 3D Quasar Laser Beam system.
#[derive(Component, Debug, Clone, Copy)]
pub struct QuasarBeamRoot;

/// Sub-parts of the 3D Quasar Laser Beam.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuasarBeamPart {
    NorthCore,
    NorthSheath,
    NorthLobe,
    SouthCore,
    SouthSheath,
    SouthLobe,
}

/// Synchronizes 3D volumetric laser beam columns for active Quasars / Black Hole Stars.
pub fn sync_quasar_beams(
    mut commands: Commands,
    visual_assets: Option<Res<VisualAssets>>,
    config: Res<SimulationConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    quasi_query: Query<
        (
            &SimPosition,
            &CelestialBody,
            &Mass,
            &Radius,
            Option<&BlackHoleStarState>,
        ),
        Or<(With<CentralStar>, With<BlackHoleStarState>)>,
    >,
    mut root_query: Query<(Entity, &mut Transform), With<QuasarBeamRoot>>,
    mut part_query: Query<(&mut Transform, &QuasarBeamPart), Without<QuasarBeamRoot>>,
) {
    let Some(assets) = visual_assets else {
        return;
    };

    // Find any active Quasar / Quasi-Star in the simulation
    let target = quasi_query.iter().find(|(_, body, mass, _, opt_qs)| {
        opt_qs.is_some()
            || body.body_type == BodyType::QuasiStar
            || body.name.contains("Quasar")
            || (body.body_type == BodyType::BlackHole && mass.0 > 500.0)
    });

    let Some((pos, body, _mass, radius, opt_qs)) = target else {
        // No active quasar/quasi-star: despawn beam visual if present
        for (ent, _) in root_query.iter() {
            commands.entity(ent).despawn();
        }
        return;
    };

    let world_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let is_blown_out =
        opt_qs.map(|qs| qs.is_blown_out).unwrap_or(false) || body.name.contains("Quasar");
    let light_dist = opt_qs
        .map(|qs| qs.jet_travel_distance_au as f32)
        .unwrap_or(0.0);

    // CRITICAL: Do NOT spawn or render quasar laser beams before the cocoon has blown out!
    // The laser beams emerge ONLY after blowout, and travel outward across space at the speed of light c continuously!
    if !is_blown_out || light_dist <= 0.1 {
        for (ent, _) in root_query.iter() {
            commands.entity(ent).despawn();
        }
        return;
    }

    // Continuous propagation at the speed of light c (~63,241 AU/yr)
    let jet_len = light_dist;

    // Dynamically anchor beam start to the black hole's actual visual radius
    // as it shrinks from 60 AU down to ~2.5 AU, eliminating any floating gap!
    let current_visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
    let pole_start = (current_visual_radius * 0.90).max(0.05);
    let beam_len = (jet_len - pole_start).max(0.1);
    let beam_center = pole_start + beam_len * 0.5;

    // Slim, highly-collimated laser beam proportions
    let core_r = 0.06f32; // Razor-thin brilliant white laser filament
    let sheath_r = 0.18f32; // Subtle translucent cyan relativistic plasma sheath
    let lobe_r = 0.55f32; // Sleek bow shock cap at the light front

    if let Some((_, mut root_trans)) = root_query.iter_mut().next() {
        root_trans.translation = world_pos;
        for (mut part_trans, part) in part_query.iter_mut() {
            match part {
                QuasarBeamPart::NorthCore => {
                    part_trans.translation = Vec3::new(0.0, beam_center, 0.0);
                    part_trans.scale = Vec3::new(core_r, beam_len, core_r);
                }
                QuasarBeamPart::NorthSheath => {
                    part_trans.translation = Vec3::new(0.0, beam_center, 0.0);
                    part_trans.scale = Vec3::new(sheath_r, beam_len, sheath_r);
                }
                QuasarBeamPart::NorthLobe => {
                    part_trans.translation = Vec3::new(0.0, jet_len, 0.0);
                    part_trans.scale = Vec3::splat(lobe_r);
                }
                QuasarBeamPart::SouthCore => {
                    part_trans.translation = Vec3::new(0.0, -beam_center, 0.0);
                    part_trans.scale = Vec3::new(core_r, beam_len, core_r);
                }
                QuasarBeamPart::SouthSheath => {
                    part_trans.translation = Vec3::new(0.0, -beam_center, 0.0);
                    part_trans.scale = Vec3::new(sheath_r, beam_len, sheath_r);
                }
                QuasarBeamPart::SouthLobe => {
                    part_trans.translation = Vec3::new(0.0, -jet_len, 0.0);
                    part_trans.scale = Vec3::splat(lobe_r);
                }
            }
        }
    } else {
        // Spawn 3D beam hierarchy
        let core_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(75.0, 75.0, 90.0, 1.0),
            unlit: true,
            ..default()
        });
        let sheath_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.20, 0.75, 1.0, 0.22),
            emissive: LinearRgba::new(2.5, 8.0, 20.0, 0.30),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let lobe_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.40, 0.85, 1.0, 0.60),
            emissive: LinearRgba::new(8.0, 18.0, 35.0, 0.6),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });

        commands
            .spawn((
                QuasarBeamRoot,
                Transform::from_translation(world_pos),
                Visibility::default(),
            ))
            .with_children(|parent| {
                // North Polar Jet (Core + Sheath + Terminal Lobe)
                parent.spawn((
                    QuasarBeamPart::NorthCore,
                    Mesh3d(assets.beam_core_mesh.clone()),
                    MeshMaterial3d(core_mat.clone()),
                    Transform::from_xyz(0.0, beam_center, 0.0)
                        .with_scale(Vec3::new(core_r, beam_len, core_r)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    QuasarBeamPart::NorthSheath,
                    Mesh3d(assets.beam_sheath_mesh.clone()),
                    MeshMaterial3d(sheath_mat.clone()),
                    Transform::from_xyz(0.0, beam_center, 0.0)
                        .with_scale(Vec3::new(sheath_r, beam_len, sheath_r)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    QuasarBeamPart::NorthLobe,
                    Mesh3d(assets.star_mesh.clone()),
                    MeshMaterial3d(lobe_mat.clone()),
                    Transform::from_xyz(0.0, jet_len, 0.0).with_scale(Vec3::splat(lobe_r)),
                    NotShadowCaster,
                ));

                // South Polar Jet (Core + Sheath + Terminal Lobe)
                parent.spawn((
                    QuasarBeamPart::SouthCore,
                    Mesh3d(assets.beam_core_mesh.clone()),
                    MeshMaterial3d(core_mat.clone()),
                    Transform::from_xyz(0.0, -beam_center, 0.0)
                        .with_scale(Vec3::new(core_r, beam_len, core_r)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    QuasarBeamPart::SouthSheath,
                    Mesh3d(assets.beam_sheath_mesh.clone()),
                    MeshMaterial3d(sheath_mat.clone()),
                    Transform::from_xyz(0.0, -beam_center, 0.0)
                        .with_scale(Vec3::new(sheath_r, beam_len, sheath_r)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    QuasarBeamPart::SouthLobe,
                    Mesh3d(assets.star_mesh.clone()),
                    MeshMaterial3d(lobe_mat.clone()),
                    Transform::from_xyz(0.0, -jet_len, 0.0).with_scale(Vec3::splat(lobe_r)),
                    NotShadowCaster,
                ));
            });
    }
}
