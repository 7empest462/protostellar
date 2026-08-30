//! Visual mesh generation, PBR materials, and real-time transform synchronization.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

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
    pub particle_mesh: Handle<Mesh>,
}

pub fn setup_visual_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let star_mesh = meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap());
    let planet_mesh = meshes.add(Sphere::new(1.0).mesh().ico(4).unwrap());
    let particle_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());

    commands.insert_resource(VisualAssets {
        star_mesh,
        planet_mesh,
        particle_mesh,
    });
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
    for (entity, pos, mass, _radius, temp, comp, body, is_star) in unspawned_query.iter() {
        let (br, bg, bb) = blackbody_to_srgb(temp.0);
        let (cr, cg, cb) = comp.visual_color_tint();

        let base_color = if is_star.is_some() {
            Color::srgb(br, bg, bb)
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

        let visual_radius =
            SimulationConfig::calc_render_radius(mass.0, body.body_type) * config.body_render_scale;

        if is_star.is_some() {
            // Central Star: High emissive glow + outer solar corona shell + point light
            let material = materials.add(PlanetMaterial {
                base: StandardMaterial {
                    base_color,
                    emissive: LinearRgba::from(base_color) * 22.0,
                    unlit: true,
                    ..default()
                },
                extension: PlanetMaterialExtension {
                    uniforms: PlanetUniforms {
                        planet_type: 0,
                        temperature: temp.0 as f32,
                        time: 0.0,
                        composition: Vec4::new(0.0, 0.0, 0.0, 1.0),
                        color_seed: LinearRgba::from(base_color).to_vec4(),
                    },
                },
            });

            let corona_material = materials.add(PlanetMaterial {
                base: StandardMaterial {
                    base_color: Color::srgba(br, bg, bb, 0.25),
                    emissive: LinearRgba::from(base_color) * 8.0,
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                },
                extension: PlanetMaterialExtension {
                    uniforms: PlanetUniforms {
                        planet_type: 0,
                        temperature: temp.0 as f32,
                        time: 0.0,
                        composition: Vec4::new(0.0, 0.0, 0.0, 1.0),
                        color_seed: LinearRgba::from(base_color).to_vec4(),
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
                    // Outer atmospheric corona halo
                    parent.spawn((
                        Mesh3d(visual_assets.star_mesh.clone()),
                        MeshMaterial3d(corona_material),
                        Transform::from_scale(Vec3::splat(1.35)),
                        NotShadowCaster,
                    ));

                    // Omnidirectional solar illumination
                    parent.spawn((
                        PointLight {
                            color: base_color,
                            intensity: 60_000_000.0,
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
                            BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
                                if norm_comp.gas_frac > 0.30 {
                                    1 // Gas Giant banded
                                } else if norm_comp.ice_frac > 0.40 {
                                    2 // Ice Giant / Icy world
                                } else {
                                    3 // Terrestrial
                                }
                            }
                            _ => 4, // Moon / Asteroid
                        },
                        temperature: temp.0 as f32,
                        time: 0.0,
                        composition: Vec4::new(
                            norm_comp.silicate_frac as f32 + norm_comp.organics_frac as f32,
                            norm_comp.ice_frac as f32,
                            norm_comp.metal_frac as f32,
                            norm_comp.gas_frac as f32,
                        ),
                        color_seed: LinearRgba::from(base_color).to_vec4(),
                    },
                },
            });

            let mesh_handle = match body.body_type {
                BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::TerrestrialPlanet
                | BodyType::Moon => visual_assets.planet_mesh.clone(),
                _ => visual_assets.particle_mesh.clone(),
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
    mut query: Query<
        (
            &SimPosition,
            &Mass,
            &Radius,
            &Temperature,
            &Composition,
            &mut Transform,
            &mut Mesh3d,
            &MeshMaterial3d<PlanetMaterial>,
            &CelestialBody,
        ),
        With<VisualBody>,
    >,
) {
    for (pos, mass, _radius, temp, comp, mut transform, mut mesh, mat_handle, body) in
        query.iter_mut()
    {
        transform.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);

        // Dynamically scale body mesh using mass cube-root hierarchy
        let visual_radius =
            SimulationConfig::calc_render_radius(mass.0, body.body_type) * config.body_render_scale;
        transform.scale = Vec3::splat(visual_radius);

        // Sync Mesh Level of Detail (LOD) based on Body Type
        let target_mesh = match body.body_type {
            BodyType::GasGiant
            | BodyType::IceGiant
            | BodyType::TerrestrialPlanet
            | BodyType::Moon => visual_assets.planet_mesh.clone(),
            _ => visual_assets.particle_mesh.clone(),
        };
        if mesh.0 != target_mesh {
            mesh.0 = target_mesh;
        }

        // Update material emissive color and PBR properties dynamically
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            let (br, bg, bb) = blackbody_to_srgb(temp.0);
            let (cr, cg, cb) = comp.visual_color_tint();

            let color = if matches!(
                body.body_type,
                BodyType::Protostar | BodyType::MainSequenceStar
            ) {
                Color::srgb(br, bg, bb)
            } else {
                Color::srgb(
                    (br * 0.25 + cr * 0.75).clamp(0.1, 1.0),
                    (bg * 0.25 + cg * 0.75).clamp(0.1, 1.0),
                    (bb * 0.25 + cb * 0.75).clamp(0.1, 1.0),
                )
            };
            let norm_comp = comp.normalized();
            mat.base.base_color = color;
            mat.extension.uniforms.color_seed = LinearRgba::from(color).to_vec4();
            mat.extension.uniforms.temperature = temp.0 as f32;
            mat.extension.uniforms.time = time.elapsed_secs();
            mat.extension.uniforms.composition = Vec4::new(
                norm_comp.silicate_frac as f32 + norm_comp.organics_frac as f32,
                norm_comp.ice_frac as f32,
                norm_comp.metal_frac as f32,
                norm_comp.gas_frac as f32,
            );
            mat.extension.uniforms.planet_type = match body.body_type {
                BodyType::Protostar | BodyType::MainSequenceStar => 0,
                BodyType::GasGiant => 1,
                BodyType::IceGiant => 2,
                BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
                    if norm_comp.gas_frac > 0.30 {
                        1 // Gas Giant banded
                    } else if norm_comp.ice_frac > 0.40 {
                        2 // Ice Giant / Icy world
                    } else {
                        3 // Terrestrial
                    }
                }
                _ => 4, // Moon / Asteroid
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

            if body.body_type == BodyType::MainSequenceStar {
                mat.base.emissive = LinearRgba::from(color) * 32.0;
            } else if body.body_type == BodyType::Protostar {
                mat.base.emissive = LinearRgba::from(color) * 14.0;
            } else if temp.0 > 600.0 {
                mat.base.emissive =
                    LinearRgba::from(color) * ((temp.0 as f32 - 600.0) / 600.0).clamp(0.0, 5.0);
            } else {
                mat.base.emissive = LinearRgba::BLACK;
            }
        }
    }
}
