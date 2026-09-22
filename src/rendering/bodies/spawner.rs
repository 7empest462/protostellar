use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::palettes::*;
use super::{VisualAssets, VisualBody};

fn spawn_star_visual(
    entity_cmd: &mut EntityCommands,
    body: &CelestialBody,
    mass: &Mass,
    temp: &Temperature,
    base_color: Color,
    trans: Vec3,
    visual_radius: f32,
    star_mesh: Handle<Mesh>,
    materials: &mut Assets<PlanetMaterial>,
) {
    let (p_type, unlit_flag, emissive_val) = match body.body_type {
        BodyType::BlackHole => (5u32, false, LinearRgba::BLACK),
        BodyType::QuasiStar => (
            7u32,
            true,
            // Quasi-star: deep UV-infrared furnace; high emissive for bloom corona
            LinearRgba::from(Color::srgb(1.0, 0.10, 0.02)) * 160.0,
        ),
        BodyType::WhiteDwarf => (0u32, true, LinearRgba::from(base_color) * 220.0),
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => {
            (0u32, true, LinearRgba::from(base_color) * 280.0)
        }
        BodyType::Protostar => (0u32, true, LinearRgba::from(base_color) * 55.0),
        BodyType::RedDwarf => (0u32, true, LinearRgba::from(base_color) * 80.0),
        BodyType::BrownDwarf => (0u32, true, LinearRgba::from(base_color) * 28.0),
        BodyType::RedGiant | BodyType::RedSupergiant => {
            (0u32, true, LinearRgba::from(base_color) * 95.0)
        }
        BodyType::BlueGiant | BodyType::BlueSupergiant | BodyType::Hypergiant => {
            (0u32, true, LinearRgba::from(base_color) * 350.0)
        }
        BodyType::WolfRayet => (0u32, true, LinearRgba::from(base_color) * 310.0),
        // Sun-like main sequence — 120× produces a visible bloom corona at typical orbital distances
        _ => (0u32, true, LinearRgba::from(base_color) * 120.0),
    };

    let star_subtype = star_subtype_from_body_type(body.body_type);
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
                composition: Vec4::new(star_subtype, 20.0, 0.5, 1.0),
                color_seed: LinearRgba::from(base_color).to_vec4(),
                climate_and_bio: Vec4::ZERO,
                atmosphere_params: Vec4::new(0.60, 0.85, 0.08, 0.0),
                dynamics_and_mag: Vec4::new(100.0, 1.0, mass.0 as f32, 0.0),
                ..default()
            },
        },
    });

    entity_cmd
        .try_insert((
            VisualBody,
            Mesh3d(star_mesh),
            MeshMaterial3d(material),
            Transform::from_translation(trans).with_scale(Vec3::splat(visual_radius)),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                PointLight {
                    color: base_color,
                    // HDR-scaled initial intensity — update_star_lights_and_strobes will
                    // refine this every frame, but we seed it at the HDR level so the
                    // first frame doesn't show a dark star surrounded by an unlit void.
                    intensity: if body.body_type == BodyType::BlackHole {
                        2_000_000.0
                    } else if body.body_type == BodyType::QuasiStar {
                        60_000_000.0
                    } else if matches!(
                        body.body_type,
                        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
                    ) {
                        30_000_000.0
                    } else if body.body_type == BodyType::WhiteDwarf {
                        25_000_000.0
                    } else {
                        18_000_000.0
                    },
                    range: 500.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::IDENTITY,
            ));
        });
}

fn spawn_planet_visual(
    entity_cmd: &mut EntityCommands,
    body: &CelestialBody,
    mass: &Mass,
    temp: &Temperature,
    comp: &Composition,
    base_color: Color,
    trans: Vec3,
    visual_radius: f32,
    visual_assets: &VisualAssets,
    materials: &mut Assets<PlanetMaterial>,
) {
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
                            2
                        } else {
                            3
                        }
                    }
                    _ => 4,
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
                dynamics_and_mag: Vec4::new(0.0, 0.0, (mass.0 / JUPITER_MASS_SOLAR) as f32, 0.08),
                ..default()
            },
        },
    });

    let mesh_handle = super::meshes::select_body_mesh(body, visual_assets);

    entity_cmd.try_insert((
        VisualBody,
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_translation(trans).with_scale(Vec3::splat(visual_radius)),
        Visibility::default(),
    ));
}

/// Spawns visual meshes and point lights for newly created celestial bodies.
#[allow(clippy::type_complexity, reason = "Spawner Complexity")]
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

        let is_star_like = is_star.is_some() || body.body_type.is_star_or_remnant();
        let is_gas_giant =
            body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.30;
        let base_color = if is_star_like {
            compute_stellar_palette(body.body_type, temp.0)
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

        if is_star_like {
            spawn_star_visual(
                &mut entity_cmd,
                body,
                mass,
                temp,
                base_color,
                trans,
                visual_radius,
                visual_assets.star_mesh.clone(),
                &mut materials,
            );
        } else {
            spawn_planet_visual(
                &mut entity_cmd,
                body,
                mass,
                temp,
                comp,
                base_color,
                trans,
                visual_radius,
                &visual_assets,
                &mut materials,
            );
        }
    }
}
