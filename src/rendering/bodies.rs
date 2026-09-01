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
    pub ring_mesh: Handle<Mesh>,
}

pub fn setup_visual_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let star_mesh = meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap());
    let planet_mesh = meshes.add(Sphere::new(1.0).mesh().ico(4).unwrap());
    let particle_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
    let ring_mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0).build());

    commands.insert_resource(VisualAssets {
        star_mesh,
        planet_mesh,
        particle_mesh,
        ring_mesh,
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

        let visual_radius = if is_star.is_some() || body.body_type.is_star_or_remnant() {
            SimulationConfig::calc_render_radius(mass.0, body.body_type)
        } else {
            SimulationConfig::calc_render_radius(mass.0, body.body_type) * config.body_render_scale
        };

        if is_star.is_some() || body.body_type.is_star_or_remnant() {
            // Central Star & Stellar Remnants: Emissive unlit glow + point light
            let (p_type, unlit_flag, emissive_val) = match body.body_type {
                BodyType::BlackHole => (5u32, false, LinearRgba::BLACK),
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
                        composition: Vec4::new(0.0, 0.0, 0.0, 1.0),
                        color_seed: LinearRgba::from(base_color).to_vec4(),
                        climate_and_bio: Vec4::ZERO,
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
                                10_000_000.0
                            } else {
                                60_000_000.0
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
                        climate_and_bio: Vec4::ZERO,
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
    ) in query.iter_mut()
    {
        transform.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);

        // Dynamically scale body mesh using mass cube-root hierarchy and stellar evolution state.
        // Design contract: star visual radius must ALWAYS exceed the largest planet visual radius
        // so the star remains visually dominant. Planet visual radii use body_render_scale (0.08)
        // to stay small relative to orbital distances, preventing visual crowding.
        let visual_radius = if body.body_type.is_star_or_remnant() {
            match body.body_type {
                BodyType::WhiteDwarf
                | BodyType::NeutronStar
                | BodyType::Pulsar
                | BodyType::Magnetar
                | BodyType::BlackHole => {
                    SimulationConfig::calc_render_radius(mass.0, body.body_type)
                }
                BodyType::RedGiant | BodyType::RedSupergiant | BodyType::Hypergiant => {
                    (radius.0 as f32).clamp(0.045, 10.0)
                }
                BodyType::RedDwarf | BodyType::BrownDwarf => {
                    // Ultracool & red dwarfs (e.g. TRAPPIST-1 at R=0.00056 AU, Proxima Centauri)
                    // Must render clearly larger than terrestrial planets (which max out ~0.0024 AU).
                    // Physical radius × 14 with floor at 0.006 AU.
                    ((radius.0 as f32) * 14.0).clamp(0.006, 0.018)
                }
                _ => {
                    // Main sequence stars, K-dwarfs, Blue Giants, Wolf-Rayet, Protostars
                    // Sun (R=0.00465 AU) → 0.00465 * 4.5 = 0.021 AU visual radius
                    ((radius.0 as f32) * 4.5).clamp(0.015, 0.10)
                }
            }
        } else {
            SimulationConfig::calc_render_radius(mass.0, body.body_type) * config.body_render_scale
        };
        transform.scale = Vec3::splat(visual_radius);

        // Sync Mesh Level of Detail (LOD) based on Body Type
        let target_mesh = if body.body_type.is_star_or_remnant() {
            visual_assets.star_mesh.clone()
        } else {
            match body.body_type {
                BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::TerrestrialPlanet
                | BodyType::Moon => visual_assets.planet_mesh.clone(),
                _ => visual_assets.particle_mesh.clone(),
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
            let color = if is_star_like {
                if body.body_type == BodyType::BlackHole {
                    Color::srgb(0.01, 0.01, 0.01)
                } else {
                    Color::srgb(br, bg, bb)
                }
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
            mat.extension.uniforms.climate_and_bio =
                Vec4::new(ocean_frac, ice_frac, biomass_frac, cloud_density);

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
                        _ => 30.0,
                    };
                    mat.base.emissive = LinearRgba::from(color) * mult;
                }
            } else {
                mat.base.unlit = false;
                mat.extension.uniforms.planet_type = match body.body_type {
                    BodyType::GasGiant => 1,
                    BodyType::IceGiant => 2,
                    BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
                        if norm_comp.gas_frac > 0.30 {
                            1
                        } else if norm_comp.ice_frac > 0.40 {
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
        &Mass,
        &CelestialBody,
        Option<&SpinState>,
        Option<&Children>,
    )>,
    mut ring_children_query: Query<
        (&mut Transform, &MeshMaterial3d<RingMaterial>),
        With<VisualRingChild>,
    >,
) {
    for (planet_entity, ring_sys, mass, body, opt_spin, opt_children) in
        planets_with_rings_query.iter()
    {
        let planet_render_rad =
            SimulationConfig::calc_render_radius(mass.0, body.body_type) * config.body_render_scale;
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
                    }
                }
            }
        }

        if !found_child {
            let material = ring_materials.add(RingMaterial {
                uniforms: RingUniforms {
                    inner_radius: ring_sys.inner_radius_au,
                    outer_radius: ring_sys.outer_radius_au,
                    optical_depth: ring_sys.optical_depth,
                    ice_fraction: ring_sys.ice_fraction,
                    ring_color: Vec4::ONE,
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
