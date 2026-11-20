//! Multi-layered 3D volumetric visual detonation for the JWST Little Red Dot Quasar Blowout:
//! inner Cherenkov photon shock cavern, supersonic equatorial Mach torus, bipolar relativistic
//! jet breakout cones, 3D HDR emissive plasma ejecta shards, and dynamic explosion lighting.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::rendering::bodies::VisualAssets;
use crate::rendering::effects::supernova::{
    SupernovaDebrisPool, SupernovaExplosionInstance, SupernovaType,
};
use crate::rendering::materials::{SupernovaMaterial, SupernovaUniforms};

/// Secondary 3D volumetric shader layer type for a Little Red Dot Quasar Blowout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlowoutShellKind {
    /// Blazing Cherenkov white-blue/violet inner radiation shock cavern (`type_id = 5.0`).
    InnerRadiationCore,
    /// Flattened supersonic equatorial Mach plasma blast torus (`type_id = 6.0`).
    EquatorialShockDisk,
    /// Collimated North/South bipolar relativistic jet breakout plumes (`type_id = 7.0`).
    PolarBreakoutCones,
}

impl BlowoutShellKind {
    pub const ALL: [Self; 3] = [
        Self::InnerRadiationCore,
        Self::EquatorialShockDisk,
        Self::PolarBreakoutCones,
    ];

    const fn shader_type_id(self) -> f32 {
        match self {
            Self::InnerRadiationCore => 5.0,
            Self::EquatorialShockDisk => 6.0,
            Self::PolarBreakoutCones => 7.0,
        }
    }

    fn target_scale(self, exp: &SupernovaExplosionInstance) -> Vec3 {
        let r = exp.current_radius_au.max(0.2);
        let p = (exp.timer / exp.max_timer.max(0.1)).clamp(0.0, 1.0);
        match self {
            Self::InnerRadiationCore => {
                let core_r = (r * (0.48 + 0.14 * p)).max(exp.initial_radius_au * 0.65);
                Vec3::splat(core_r)
            }
            Self::EquatorialShockDisk => {
                // Horizontal supersonic blast ring racing ahead of the spherical hydrogen cocoon
                let eq_r = r * (1.28 + 0.18 * (1.0 - p));
                let eq_h = (r * 0.15).max(0.45);
                Vec3::new(eq_r, eq_h, eq_r)
            }
            Self::PolarBreakoutCones => {
                // Collimated polar shock cones punching rapidly out along +/-Y ahead of the cocoon
                let punch = (exp.timer * 1.4).clamp(0.0, 1.0);
                let jet_w = (r * 0.36).max(exp.initial_radius_au * 0.55);
                let jet_h = (r * (1.65 + 0.75 * punch)).max(exp.initial_radius_au * 2.2);
                Vec3::new(jet_w, jet_h, jet_w)
            }
        }
    }

    fn build_uniforms(self, exp: &SupernovaExplosionInstance) -> SupernovaUniforms {
        SupernovaUniforms {
            params: Vec4::new(
                exp.timer,
                exp.max_timer,
                exp.current_radius_au,
                exp.blast_speed_au_s,
            ),
            core_params: Vec4::new(
                exp.flash_intensity,
                self.shader_type_id(),
                exp.ejecta_mass_solar,
                1.0,
            ),
            center_and_asphericity: Vec4::new(exp.center.x, exp.center.y, exp.center.z, 0.75),
            core_color: Vec4::new(1.65, 0.98, 0.35, 1.0),
            mantle_color: Vec4::new(0.22, 0.98, 1.25, 0.92),
            envelope_color: Vec4::new(1.30, 0.18, 0.08, 0.95),
            jet_color: Vec4::new(0.42, 0.78, 1.85, 1.0),
        }
    }
}

/// Marker component for secondary 3D volumetric Quasar Blowout shader shells.
#[derive(Component, Debug, Clone, Copy)]
pub struct QuasarBlowoutLayer {
    pub star_entity: Entity,
    pub kind: BlowoutShellKind,
}

/// Marker component for 3D HDR emissive plasma/cocoon fireball chunks blasted outward on blowout.
#[derive(Component, Debug, Clone, Copy)]
pub struct QuasarBlowoutShard {
    pub star_entity: Entity,
    pub dir: Vec3,
    pub speed_mult: f32,
    pub base_radius: f32,
    pub swirl_seed: f32,
}

/// Marker component for the dynamic HDR detonation flash `PointLight`.
#[derive(Component, Debug, Clone, Copy)]
pub struct QuasarBlowoutLight {
    pub star_entity: Entity,
}

pub const QUASAR_BLOWOUT_SHARD_COUNT: usize = 48;

fn spawn_blowout_shards(
    commands: &mut Commands,
    assets: &VisualAssets,
    std_materials: &mut Assets<StandardMaterial>,
    exp: &SupernovaExplosionInstance,
) {
    // 4 shared HDR emissive materials across all 48 3D plasma fireball chunks.
    // Note: Bevy `unlit: true` outputs `base_color` directly into the HDR bloom target,
    // so we supply HDR linear RGB values in `base_color` as well as `emissive`.
    let shard_mats = [
        // 0: Molten crimson H-alpha shredded cocoon blobs
        std_materials.add(StandardMaterial {
            base_color: Color::linear_rgb(10.5, 1.6, 0.45),
            emissive: LinearRgba::new(10.5, 1.6, 0.45, 1.0),
            unlit: true,
            ..default()
        }),
        // 1: Incandescent gold-amber superheated core plasma
        std_materials.add(StandardMaterial {
            base_color: Color::linear_rgb(14.0, 7.2, 1.4),
            emissive: LinearRgba::new(14.0, 7.2, 1.4, 1.0),
            unlit: true,
            ..default()
        }),
        // 2: Ionized [O III] cyan shockwave knots
        std_materials.add(StandardMaterial {
            base_color: Color::linear_rgb(1.6, 9.5, 13.0),
            emissive: LinearRgba::new(1.6, 9.5, 13.0, 1.0),
            unlit: true,
            ..default()
        }),
        // 3: Blazing Cherenkov sapphire polar breakout bolts
        std_materials.add(StandardMaterial {
            base_color: Color::linear_rgb(4.2, 8.5, 22.0),
            emissive: LinearRgba::new(4.2, 8.5, 22.0, 1.0),
            unlit: true,
            ..default()
        }),
    ];

    for i in 0..QUASAR_BLOWOUT_SHARD_COUNT {
        let fi = i as f32;
        let u = (fi + 0.5) / (QUASAR_BLOWOUT_SHARD_COUNT as f32);
        let theta = (u * 2.0 - 1.0).asin();
        let phi = fi * 2.399_963_2;
        let dir = Vec3::new(
            theta.cos() * phi.cos(),
            theta.sin(),
            theta.cos() * phi.sin(),
        )
        .normalize_or(Vec3::Y);

        let seed = (fi * 13.73).fract();
        let is_polar = dir.y.abs() > 0.74;
        let mat_idx = if is_polar { 3 } else { i % 3 };
        let Some(mat_handle) = shard_mats.get(mat_idx).cloned() else {
            continue;
        };
        let speed_mult = if is_polar {
            1.45 + seed * 0.55
        } else {
            0.72 + seed * 0.65
        };
        let base_radius = 0.22 + seed * 0.42;
        let start_pos = exp.center + dir * exp.initial_radius_au;

        commands.spawn((
            QuasarBlowoutShard {
                star_entity: exp.star_entity,
                dir,
                speed_mult,
                base_radius,
                swirl_seed: fi * 1.618,
            },
            Mesh3d(assets.star_mesh.clone()),
            MeshMaterial3d(mat_handle),
            Transform::from_translation(start_pos).with_scale(Vec3::splat(base_radius)),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

fn update_blowout_light(
    light: &mut PointLight,
    tf: &mut Transform,
    exp: &SupernovaExplosionInstance,
) {
    let p = (exp.timer / exp.max_timer.max(0.1)).clamp(0.0, 1.0);
    let fade = (1.0 - p).powf(1.65);
    let target_intensity = (3.2e9 * (0.25 + 0.75 * exp.flash_intensity) * fade).max(0.0);
    let target_color = if exp.timer < 0.75 {
        Color::srgb(0.85, 0.92, 1.0)
    } else if exp.timer < 3.5 {
        Color::srgb(1.0, 0.52, 0.22)
    } else {
        Color::srgb(0.92, 0.25, 0.45)
    };
    if tf.translation != exp.center {
        tf.translation = exp.center;
    }
    if (light.intensity - target_intensity).abs() > 1e5 {
        light.intensity = target_intensity;
    }
    if light.color != target_color {
        light.color = target_color;
    }
}

fn update_single_shard_transform(
    shard: &QuasarBlowoutShard,
    tf: &mut Transform,
    exp: &SupernovaExplosionInstance,
) {
    let p = (exp.timer / exp.max_timer.max(0.1)).clamp(0.0, 1.0);
    let radial_dist = exp.initial_radius_au
        + (exp.current_radius_au - exp.initial_radius_au).max(0.0) * shard.speed_mult;
    let swirl_amp = (radial_dist * 0.04).min(4.0) * p;
    let swirl = Vec3::new(
        (shard.swirl_seed + exp.timer * 1.8).sin() * swirl_amp,
        (shard.swirl_seed * 1.3 + exp.timer * 1.4).cos() * swirl_amp * 0.6,
        (shard.swirl_seed * 0.7 + exp.timer * 2.1).cos() * swirl_amp,
    );
    let target_pos = exp.center + shard.dir * radial_dist + swirl;
    let growth = 1.0 + (radial_dist / 40.0).min(3.2);
    let shrink = (1.0 - p).powf(0.85).max(0.01);
    let r_t = shard.base_radius * growth * shrink;
    let r_l = r_t * (2.4 + 2.2 * (1.0 - p));
    let target_scale = Vec3::new(r_t, r_l, r_t);
    let target_rot = Quat::from_rotation_arc(Vec3::Y, shard.dir);

    if tf.translation != target_pos {
        tf.translation = target_pos;
    }
    if tf.rotation != target_rot {
        tf.rotation = target_rot;
    }
    if tf.scale != target_scale {
        tf.scale = target_scale;
    }
}

/// Synchronizes the multi-layer 3D volumetric Quasar Blowout shells, 48 HDR emissive fireball
/// shards, and dynamic explosion flash light with active `SupernovaType::QuasarBlowout` instances.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Quasar blowout sync manages secondary volumetric shells, 3D plasma shards, and flash light"
)]
pub fn sync_quasar_blowout_visuals(
    mut commands: Commands,
    pool: Res<SupernovaDebrisPool>,
    visual_assets: Option<Res<VisualAssets>>,
    mut sn_materials: Option<ResMut<Assets<SupernovaMaterial>>>,
    mut std_materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut layer_query: Query<(
        Entity,
        &QuasarBlowoutLayer,
        &mut Transform,
        &MeshMaterial3d<SupernovaMaterial>,
    )>,
    mut shard_query: Query<
        (Entity, &QuasarBlowoutShard, &mut Transform),
        (Without<QuasarBlowoutLayer>, Without<QuasarBlowoutLight>),
    >,
    mut light_query: Query<
        (Entity, &QuasarBlowoutLight, &mut PointLight, &mut Transform),
        (Without<QuasarBlowoutLayer>, Without<QuasarBlowoutShard>),
    >,
) {
    let (Some(assets), Some(sn_mats), Some(std_mats)) = (
        visual_assets,
        sn_materials.as_deref_mut(),
        std_materials.as_deref_mut(),
    ) else {
        return;
    };

    let active_blowouts: FxHashMap<Entity, &SupernovaExplosionInstance> = pool
        .explosions
        .iter()
        .filter(|exp| exp.explosion_type == SupernovaType::QuasarBlowout)
        .map(|exp| (exp.star_entity, exp))
        .collect();

    // 1. Update or despawn secondary volumetric shader shells
    let mut existing_layers: FxHashSet<(Entity, BlowoutShellKind)> = FxHashSet::default();
    for (ent, layer, mut tf, mat_handle) in layer_query.iter_mut() {
        let Some(&exp) = active_blowouts.get(&layer.star_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        existing_layers.insert((layer.star_entity, layer.kind));
        let target_scale = layer.kind.target_scale(exp);
        if tf.translation != exp.center {
            tf.translation = exp.center;
        }
        if tf.scale != target_scale {
            tf.scale = target_scale;
        }
        let next_u = layer.kind.build_uniforms(exp);
        if let Some(mat) = sn_mats.get(&mat_handle.0)
            && mat.uniforms != next_u
            && let Some(mut mat_mut) = sn_mats.get_mut(&mat_handle.0)
        {
            mat_mut.uniforms = next_u;
        }
    }

    // 2. Update or despawn 3D plasma fireball shards
    let mut stars_with_shards: FxHashSet<Entity> = FxHashSet::default();
    for (ent, shard, mut tf) in shard_query.iter_mut() {
        let Some(&exp) = active_blowouts.get(&shard.star_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        stars_with_shards.insert(shard.star_entity);
        update_single_shard_transform(shard, &mut tf, exp);
    }

    // 3. Update or despawn dynamic flash PointLights
    let mut stars_with_lights: FxHashSet<Entity> = FxHashSet::default();
    for (ent, qlight, mut light, mut tf) in light_query.iter_mut() {
        let Some(&exp) = active_blowouts.get(&qlight.star_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        stars_with_lights.insert(qlight.star_entity);
        update_blowout_light(&mut light, &mut tf, exp);
    }

    // 4. Spawn any missing blowout shells, shards, and flash lights for newly detonated quasars
    for (&star_entity, &exp) in &active_blowouts {
        for kind in BlowoutShellKind::ALL {
            if !existing_layers.contains(&(star_entity, kind)) {
                let mat_handle = sn_mats.add(SupernovaMaterial {
                    uniforms: kind.build_uniforms(exp),
                });
                commands.spawn((
                    QuasarBlowoutLayer { star_entity, kind },
                    Mesh3d(assets.star_mesh.clone()),
                    MeshMaterial3d(mat_handle),
                    Transform::from_translation(exp.center).with_scale(kind.target_scale(exp)),
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            }
        }
        if !stars_with_shards.contains(&star_entity) {
            spawn_blowout_shards(&mut commands, &assets, std_mats, exp);
        }
        if !stars_with_lights.contains(&star_entity) {
            commands.spawn((
                QuasarBlowoutLight { star_entity },
                PointLight {
                    color: Color::srgb(0.88, 0.94, 1.0),
                    intensity: 3.2e9,
                    range: 500.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(exp.center),
            ));
        }
    }
}
