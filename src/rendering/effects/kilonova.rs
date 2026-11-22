//! Kilonova detonations, relativistic Gamma-Ray Burst (GRB) jets, and Binary Black Hole GW ringdown.
//!
//! Renders real 3D volumetric HDR ejecta shells, equatorial tidal lanthanide tori, collimated
//! short-GRB polar breakout cones, BBH quasi-normal-mode spacetime ringdown caverns, and dynamic
//! explosion flash lights (replacing legacy wireframe gizmo cages).

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::rendering::bodies::VisualAssets;
use crate::rendering::materials::{SupernovaMaterial, SupernovaUniforms};
use crate::simulation::components::BodyType;
use crate::simulation::relativity::GravitationalWaveMergerEvent;

/// An active kilonova detonation or binary black hole ringdown event instance.
#[derive(Debug, Clone)]
pub struct KilonovaInstance {
    pub primary_entity: Entity,
    pub companion_entity: Entity,
    pub position: Vec3,
    pub timer: f32,
    pub max_timer: f32,
    pub current_radius_au: f32,
    pub max_radius_au: f32,
    pub ejecta_speed_au_s: f32,
    pub is_ns_merger: bool,
    pub is_bbh_merger: bool,
    pub peak_gw_frequency_hz: f64,
    pub radiated_gw_mass_solar: f64,
    pub remnant_type: BodyType,
    pub flash_intensity: f32,
}

/// Resource pool managing active kilonova fireballs and GW merger ringdown effects.
#[derive(Resource, Default)]
pub struct KilonovaPool {
    pub instances: Vec<KilonovaInstance>,
}

impl KilonovaPool {
    /// Helper to clear active events on scenario reset.
    pub fn clear(&mut self) {
        self.instances.clear();
    }
}

/// 3D volumetric shader shell role for a kilonova or binary black hole merger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KilonovaShellKind {
    /// NS merger: lanthanide-rich r-process crimson/gold radioactive fireball (`type_id = 4.0`).
    LanthanideFireball,
    /// NS merger: equatorial tidal r-process torus / BBH merger: GW strain disk (`type_id = 6.0`).
    EquatorialWaveDisk,
    /// NS merger: collimated bipolar short-GRB relativistic jet cones (`type_id = 7.0`).
    GrbJetCones,
    /// BBH merger: quasi-normal mode spacetime ringdown shimmer cavern (`type_id = 5.0`).
    QnmRingdownCore,
}

impl KilonovaShellKind {
    pub const NS_KINDS: [Self; 3] = [
        Self::LanthanideFireball,
        Self::EquatorialWaveDisk,
        Self::GrbJetCones,
    ];
    pub const BBH_KINDS: [Self; 2] = [Self::QnmRingdownCore, Self::EquatorialWaveDisk];

    const fn shader_type_id(self) -> f32 {
        match self {
            Self::LanthanideFireball => 4.0,
            Self::QnmRingdownCore => 5.0,
            Self::EquatorialWaveDisk => 6.0,
            Self::GrbJetCones => 7.0,
        }
    }

    fn target_scale(self, inst: &KilonovaInstance) -> Vec3 {
        let r = inst.current_radius_au.max(0.25);
        let p = (inst.timer / inst.max_timer.max(0.1)).clamp(0.0, 1.0);
        match self {
            Self::LanthanideFireball => Vec3::splat(r * 0.72),
            Self::QnmRingdownCore => {
                let qnm_pulse = 1.0 + 0.14 * (inst.timer * 18.0).sin() * (1.0 - p);
                Vec3::splat((r * 0.32 * qnm_pulse).max(0.8))
            }
            Self::EquatorialWaveDisk => {
                let eq_r = if inst.is_ns_merger {
                    r * 1.18
                } else {
                    r * 1.25
                };
                let eq_h = (r * 0.14).max(0.35);
                Vec3::new(eq_r, eq_h, eq_r)
            }
            Self::GrbJetCones => {
                let punch = (inst.timer * 1.8).clamp(0.0, 1.0);
                let jet_w = (r * 0.28).max(0.45);
                let jet_h = (r * (1.45 + 0.65 * punch)).max(2.0);
                Vec3::new(jet_w, jet_h, jet_w)
            }
        }
    }

    fn build_uniforms(self, inst: &KilonovaInstance, render_pos: Vec3) -> SupernovaUniforms {
        let (core_color, mantle_color, envelope_color, jet_color) = if inst.is_ns_merger {
            // Gold/platinum r-process crimson-amber + polar blue-cyan kilonova wind + violet short-GRB jets
            (
                Vec4::new(1.85, 1.25, 0.35, 1.0),
                Vec4::new(0.28, 0.92, 1.45, 0.92),
                Vec4::new(1.35, 0.16, 0.38, 0.95),
                Vec4::new(0.75, 0.42, 2.20, 1.0),
            )
        } else {
            // Pure spacetime curvature GW chirp: electric cyan / indigo / sapphire shimmer
            (
                Vec4::new(0.35, 1.15, 1.95, 1.0),
                Vec4::new(0.55, 0.45, 1.85, 0.90),
                Vec4::new(0.18, 0.72, 1.55, 0.90),
                Vec4::new(0.45, 0.85, 2.10, 1.0),
            )
        };

        SupernovaUniforms {
            params: Vec4::new(
                inst.timer,
                inst.max_timer,
                inst.current_radius_au,
                inst.ejecta_speed_au_s,
            ),
            core_params: Vec4::new(
                inst.flash_intensity,
                self.shader_type_id(),
                (inst.radiated_gw_mass_solar as f32).max(0.05),
                1.0,
            ),
            center_and_asphericity: Vec4::new(render_pos.x, render_pos.y, render_pos.z, 0.70),
            core_color,
            mantle_color,
            envelope_color,
            jet_color,
        }
    }
}

/// Marker component for a 3D volumetric kilonova or BBH ringdown shader shell.
#[derive(Component, Debug, Clone, Copy)]
pub struct KilonovaVisualLayer {
    pub primary_entity: Entity,
    pub kind: KilonovaShellKind,
}

/// Marker component for the dynamic HDR detonation flash `PointLight`.
#[derive(Component, Debug, Clone, Copy)]
pub struct KilonovaLight {
    pub primary_entity: Entity,
}

/// Ingests `GravitationalWaveMergerEvent`s and advances active kilonova fireballs and GRB jets.
pub fn update_kilonova_effects(
    mut pool: ResMut<KilonovaPool>,
    mut merger_events: MessageReader<GravitationalWaveMergerEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    // 1. Ingest newly triggered merger events
    for ev in merger_events.read() {
        let pos = Vec3::new(
            ev.position.x as f32,
            ev.position.y as f32,
            ev.position.z as f32,
        );

        let is_bbh = ev.remnant_type == BodyType::BlackHole && ev.radiated_gw_mass_solar > 0.20;
        let is_ns = !is_bbh;

        let max_r = if is_ns { 160.0 } else { 220.0 };
        let speed = if is_ns { 45.0 } else { 65.0 };
        let max_t = if is_ns { 8.5 } else { 6.0 };

        pool.instances.push(KilonovaInstance {
            primary_entity: ev.primary_entity,
            companion_entity: ev.companion_entity,
            position: pos,
            timer: 0.0,
            max_timer: max_t,
            current_radius_au: 0.5,
            max_radius_au: max_r,
            ejecta_speed_au_s: speed,
            is_ns_merger: is_ns,
            is_bbh_merger: is_bbh,
            peak_gw_frequency_hz: ev.peak_gw_frequency_hz,
            radiated_gw_mass_solar: ev.radiated_gw_mass_solar,
            remnant_type: ev.remnant_type,
            flash_intensity: 1.0,
        });
    }

    // 2. Advance kinematics
    for inst in &mut pool.instances {
        inst.timer += dt;
        let p = (inst.timer / inst.max_timer).clamp(0.0, 1.0);

        // Prompt relativistic flash decay
        inst.flash_intensity = (-4.0 * inst.timer).exp();

        // Expanding blast wave (r ~ t^0.7)
        let blast_progress = p.powf(0.70);
        inst.current_radius_au = 0.5 + (inst.max_radius_au - 0.5) * blast_progress;
    }

    // 3. Retain active instances
    pool.instances.retain(|inst| inst.timer < inst.max_timer);
}

fn update_kilonova_light(
    light: &mut PointLight,
    tf: &mut Transform,
    inst: &KilonovaInstance,
    render_pos: Vec3,
) {
    let p = (inst.timer / inst.max_timer.max(0.1)).clamp(0.0, 1.0);
    let fade = (1.0 - p).powf(1.55);
    let peak_intensity = if inst.is_ns_merger { 2.4e9 } else { 1.1e9 };
    let target_intensity = (peak_intensity * (0.20 + 0.80 * inst.flash_intensity) * fade).max(0.0);
    let target_color = if inst.is_ns_merger {
        if inst.timer < 0.65 {
            Color::srgb(0.92, 0.80, 1.0)
        } else if inst.timer < 2.8 {
            Color::srgb(0.45, 0.90, 1.0)
        } else {
            Color::srgb(1.0, 0.42, 0.28)
        }
    } else {
        Color::srgb(0.40, 0.82, 1.0)
    };
    if tf.translation != render_pos {
        tf.translation = render_pos;
    }
    if (light.intensity - target_intensity).abs() > 1e5 {
        light.intensity = target_intensity;
    }
    if light.color != target_color {
        light.color = target_color;
    }
}

/// Synchronizes 3D volumetric HDR kilonova fireballs, short-GRB jet breakout cones,
/// BBH ringdown shells, and dynamic detonation `PointLight`s with [`KilonovaPool`].
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Kilonova visual sync manages volumetric shell layers and detonation lights"
)]
pub fn sync_kilonova_visuals(
    mut commands: Commands,
    pool: Option<Res<KilonovaPool>>,
    visual_assets: Option<Res<VisualAssets>>,
    mut sn_materials: Option<ResMut<Assets<SupernovaMaterial>>>,
    mut layer_query: Query<(
        Entity,
        &KilonovaVisualLayer,
        &mut Transform,
        &MeshMaterial3d<SupernovaMaterial>,
    )>,
    mut light_query: Query<
        (Entity, &KilonovaLight, &mut PointLight, &mut Transform),
        Without<KilonovaVisualLayer>,
    >,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let (Some(pool), Some(assets), Some(sn_mats)) =
        (pool, visual_assets, sn_materials.as_deref_mut())
    else {
        return;
    };
    let origin = opt_origin.as_deref().copied().unwrap_or_default();

    let mut active: FxHashMap<Entity, &KilonovaInstance> = FxHashMap::default();
    for inst in &pool.instances {
        active.insert(inst.primary_entity, inst);
    }

    let mut existing_layers: FxHashSet<(Entity, KilonovaShellKind)> = FxHashSet::default();
    for (ent, layer, mut tf, mat_handle) in layer_query.iter_mut() {
        let Some(&inst) = active.get(&layer.primary_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        existing_layers.insert((layer.primary_entity, layer.kind));

        let pos = origin.shift_vec3(inst.position);
        let target_scale = layer.kind.target_scale(inst);
        if tf.translation != pos {
            tf.translation = pos;
        }
        if tf.scale != target_scale {
            tf.scale = target_scale;
        }
        let next_u = layer.kind.build_uniforms(inst, pos);
        if let Some(mat) = sn_mats.get(&mat_handle.0)
            && mat.uniforms != next_u
            && let Some(mut mat_mut) = sn_mats.get_mut(&mat_handle.0)
        {
            mat_mut.uniforms = next_u;
        }
    }

    let mut stars_with_lights: FxHashSet<Entity> = FxHashSet::default();
    for (ent, kl, mut light, mut tf) in light_query.iter_mut() {
        let Some(&inst) = active.get(&kl.primary_entity) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        stars_with_lights.insert(kl.primary_entity);
        let pos = origin.shift_vec3(inst.position);
        update_kilonova_light(&mut light, &mut tf, inst, pos);
    }

    for (&primary_entity, &inst) in &active {
        let pos = origin.shift_vec3(inst.position);
        let kinds: &[KilonovaShellKind] = if inst.is_ns_merger {
            &KilonovaShellKind::NS_KINDS
        } else {
            &KilonovaShellKind::BBH_KINDS
        };
        for &kind in kinds {
            if !existing_layers.contains(&(primary_entity, kind)) {
                let mat_handle = sn_mats.add(SupernovaMaterial {
                    uniforms: kind.build_uniforms(inst, pos),
                });
                commands.spawn((
                    KilonovaVisualLayer {
                        primary_entity,
                        kind,
                    },
                    Mesh3d(assets.star_mesh.clone()),
                    MeshMaterial3d(mat_handle),
                    Transform::from_translation(pos).with_scale(kind.target_scale(inst)),
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            }
        }
        if !stars_with_lights.contains(&primary_entity) {
            commands.spawn((
                KilonovaLight { primary_entity },
                PointLight {
                    color: if inst.is_ns_merger {
                        Color::srgb(0.92, 0.80, 1.0)
                    } else {
                        Color::srgb(0.40, 0.82, 1.0)
                    },
                    intensity: if inst.is_ns_merger { 2.4e9 } else { 1.1e9 },
                    range: 450.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(pos),
            ));
        }
    }
}

/// Legacy gizmo hook retained for API compatibility; kilonova fireballs, GRB jets, and BBH
/// ringdowns now render as true 3D volumetric HDR shader shells via [`sync_kilonova_visuals`].
pub fn draw_kilonova_effects(_gizmos: &mut Gizmos, _pool: &KilonovaPool) {}
