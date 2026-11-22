//! 3D civilization megastructures, planetary defense shields, orbital mirrors, and interceptor beams.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::DVec3;
use bevy::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use super::tech_materials::{
    MegastructureMaterial, PlanetaryShieldMaterial, orbital_habitat_material,
    orbital_mirror_material, planetary_shield_material,
};
use super::tech_meshes::{build_orbital_habitat_ring_mesh, build_orbital_mirror_array_mesh};
use crate::simulation::biology::CivilizationState;
use crate::simulation::components::celestial::{CentralStar, Radius, SimPosition};
use crate::simulation::economy::{TechnologyId, UnlockedTechs};
use crate::simulation::events::{
    CosmicEventManager, CosmicEventPhase, CosmicEventType, CrisisImpactor, CrisisInterceptorPool,
};
use crate::simulation::resources::SimTime;
use crate::simulation::terraforming::TerraformingAtmosphere;

/// Tag applied to transient civilization & defense 3D visual entities for scenario teardown.
#[derive(Component, Debug, Clone, Copy)]
pub struct CivilizationVisualMarker;

/// Orbiting 3D station & traffic ring around an advanced planet.
#[derive(Component, Debug, Clone, Copy)]
pub struct OrbitalHabitatRingVisual {
    pub host: Entity,
}

/// Translucent plasma/magnetospheric barrier surrounding a shielded world.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlanetaryShieldVisual {
    pub host: Entity,
}

/// Orbital solar mirror array focusing or shielding thermal flux around a terraformed world.
#[derive(Component, Debug, Clone, Copy)]
pub struct OrbitalMirrorVisual {
    pub host: Entity,
}

/// Cached procedural meshes and materials for civilization megastructures and defense visuals.
#[derive(Resource, Clone)]
pub struct CivilizationVisualAssets {
    pub ring_mesh: Handle<Mesh>,
    pub shield_mesh: Handle<Mesh>,
    pub mirror_mesh: Handle<Mesh>,
    pub habitat_mat: Handle<MegastructureMaterial>,
    pub mirror_mat: Handle<MegastructureMaterial>,
    pub shield_passive_mat: Handle<PlanetaryShieldMaterial>,
    pub shield_overdrive_mat: Handle<PlanetaryShieldMaterial>,
}

/// Generates a brushed titanium Orbital Habitat Ring mesh with station hubs and mag-rail trusses.
#[must_use]
pub fn generate_orbital_habitat_ring_mesh() -> Mesh {
    build_orbital_habitat_ring_mesh()
}

/// Generates an 8-satellite JWST-style beveled gold-Kapton heliostat mirror array mesh.
#[must_use]
pub fn generate_orbital_mirror_array_mesh() -> Mesh {
    build_orbital_mirror_array_mesh()
}

fn ensure_civilization_assets(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    tech_mats: &mut Assets<MegastructureMaterial>,
    shield_mats: &mut Assets<PlanetaryShieldMaterial>,
    opt: Option<&CivilizationVisualAssets>,
) -> CivilizationVisualAssets {
    if let Some(existing) = opt {
        return existing.clone();
    }
    let shield_mesh = meshes.add(
        Sphere::new(1.0)
            .mesh()
            .ico(16)
            .unwrap_or_else(|_| Sphere::new(1.0).mesh().uv(32, 20)),
    );
    let assets = CivilizationVisualAssets {
        ring_mesh: meshes.add(generate_orbital_habitat_ring_mesh()),
        shield_mesh,
        mirror_mesh: meshes.add(generate_orbital_mirror_array_mesh()),
        habitat_mat: tech_mats.add(orbital_habitat_material()),
        mirror_mat: tech_mats.add(orbital_mirror_material()),
        shield_passive_mat: shield_mats.add(planetary_shield_material(false)),
        shield_overdrive_mat: shield_mats.add(planetary_shield_material(true)),
    };
    commands.insert_resource(assets.clone());
    assets
}

fn apply_tf_if_changed(tf: &mut Mut<'_, Transform>, next: Transform) {
    if tf.translation != next.translation {
        tf.translation = next.translation;
    }
    if tf.rotation != next.rotation {
        tf.rotation = next.rotation;
    }
    if tf.scale != next.scale {
        tf.scale = next.scale;
    }
}

fn sync_civilization_layer<C: Component, M: Material>(
    commands: &mut Commands,
    targets: FxHashMap<Entity, Transform>,
    query: &mut Query<(Entity, &C, &mut Transform)>,
    host_of: impl Fn(&C) -> Entity,
    make_comp: impl Fn(Entity) -> C,
    mesh: &Handle<Mesh>,
    mat: &Handle<M>,
) {
    let mut seen: FxHashSet<Entity> = FxHashSet::default();
    for (ent, vis, mut tf) in query.iter_mut() {
        let host = host_of(vis);
        let Some(&next) = targets.get(&host) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        seen.insert(host);
        apply_tf_if_changed(&mut tf, next);
    }
    for (host, tf) in targets {
        if !seen.contains(&host) {
            commands.spawn((
                make_comp(host),
                CivilizationVisualMarker,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                tf,
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }
}

fn sync_shield_layer(
    commands: &mut Commands,
    targets: FxHashMap<Entity, (Transform, bool)>,
    query: &mut Query<(
        Entity,
        &PlanetaryShieldVisual,
        &mut Transform,
        &mut MeshMaterial3d<PlanetaryShieldMaterial>,
    )>,
    assets: &CivilizationVisualAssets,
) {
    let mut seen: FxHashSet<Entity> = FxHashSet::default();
    for (ent, vis, mut tf, mut mat_comp) in query.iter_mut() {
        let Some(&(next, overdrive)) = targets.get(&vis.host) else {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.try_despawn();
            }
            continue;
        };
        seen.insert(vis.host);
        apply_tf_if_changed(&mut tf, next);
        let want_mat = if overdrive {
            &assets.shield_overdrive_mat
        } else {
            &assets.shield_passive_mat
        };
        if mat_comp.0 != *want_mat {
            mat_comp.0 = want_mat.clone();
        }
    }
    for (host, (tf, overdrive)) in targets {
        if !seen.contains(&host) {
            let mat = if overdrive {
                assets.shield_overdrive_mat.clone()
            } else {
                assets.shield_passive_mat.clone()
            };
            commands.spawn((
                PlanetaryShieldVisual { host },
                CivilizationVisualMarker,
                Mesh3d(assets.shield_mesh.clone()),
                MeshMaterial3d(mat),
                tf,
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Disjoint civilization host query excluded from stars and 3D visual markers"
)]
type CivVisualHostQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimPosition,
        &'static Radius,
        Option<&'static Transform>,
        Option<&'static CivilizationState>,
        Option<&'static TerraformingAtmosphere>,
    ),
    (
        Without<CentralStar>,
        Without<CivilizationVisualMarker>,
        Without<OrbitalHabitatRingVisual>,
        Without<PlanetaryShieldVisual>,
        Without<OrbitalMirrorVisual>,
    ),
>;

struct CivVisualFlags {
    has_mag_shield: bool,
    has_mirror_tech: bool,
    has_active_crisis: bool,
    has_ionizing: bool,
}

type CivLayerTargetMaps = (
    FxHashMap<Entity, Transform>,
    FxHashMap<Entity, (Transform, bool)>,
    FxHashMap<Entity, Transform>,
    Vec3,
);

fn collect_civilization_visual_targets(
    hosts_query: &CivVisualHostQuery,
    surface_host: Option<Entity>,
    flags: CivVisualFlags,
    elapsed: f32,
    origin: crate::rendering::origin::RenderOrigin,
) -> CivLayerTargetMaps {
    let mut ring_targets: FxHashMap<Entity, Transform> = FxHashMap::default();
    let mut shield_targets: FxHashMap<Entity, (Transform, bool)> = FxHashMap::default();
    let mut mirror_targets: FxHashMap<Entity, Transform> = FxHashMap::default();
    let mut focus_world_pos = origin.to_render(DVec3::new(1.0, 0.0, 0.0));

    for (ent, pos, rad, opt_tf, opt_civ, opt_terra) in hosts_query.iter() {
        let is_surface_host = surface_host == Some(ent);
        let center = opt_tf.map_or_else(|| origin.to_render(pos.0), |t| t.translation);
        let vis_r = opt_tf.map_or((rad.0 as f32).max(1e-9), |t| {
            t.scale.x.max(rad.0 as f32).max(1e-9)
        });

        if let Some(civ) = opt_civ
            && (civ.force_orbital_ring
                || ((civ.kardashev_level >= 0.85 || civ.orbital_infrastructure >= 0.25)
                    && civ.stability > 0.01))
        {
            focus_world_pos = center;
            let ring_scale = vis_r * (1.22 + 0.08 * civ.orbital_infrastructure);
            let rot = Quat::from_rotation_x(0.24) * Quat::from_rotation_y(elapsed * 0.45);
            ring_targets.insert(
                ent,
                Transform::from_translation(center)
                    .with_rotation(rot)
                    .with_scale(Vec3::splat(ring_scale)),
            );
        }

        let is_overdrive = opt_civ.is_some_and(|c| c.shield_overdrive_secs > 0.0);
        let shield_active = !is_surface_host
            && (opt_civ.is_some_and(|c| c.force_planetary_shield)
                || is_overdrive
                || (flags.has_mag_shield && (opt_civ.is_some() || flags.has_ionizing))
                || (flags.has_active_crisis
                    && opt_civ.is_some_and(|c| c.defense_readiness >= 0.65)));
        if shield_active && (opt_civ.is_some() || opt_terra.is_some()) {
            focus_world_pos = center;
            shield_targets.insert(
                ent,
                (
                    Transform::from_translation(center)
                        .with_rotation(Quat::from_rotation_y(-elapsed * 0.20))
                        .with_scale(Vec3::splat(vis_r * 1.12)),
                    is_overdrive,
                ),
            );
        }

        if opt_civ.is_some_and(|c| c.force_solar_mirrors)
            || (flags.has_mirror_tech && (opt_civ.is_some() || opt_terra.is_some()))
        {
            focus_world_pos = center;
            let rot = Quat::from_rotation_z(0.55) * Quat::from_rotation_y(-elapsed * 0.35);
            mirror_targets.insert(
                ent,
                Transform::from_translation(center)
                    .with_rotation(rot)
                    .with_scale(Vec3::splat(vis_r * 1.54)),
            );
        }
    }

    (
        ring_targets,
        shield_targets,
        mirror_targets,
        focus_world_pos,
    )
}

fn sync_civilization_materials(
    assets: &CivilizationVisualAssets,
    tech_mats: &mut Assets<MegastructureMaterial>,
    shield_mats: &mut Assets<PlanetaryShieldMaterial>,
    star_pos: Vec3,
    focus_pos: Vec3,
    elapsed: f32,
) {
    if let Some(mut m) = tech_mats.get_mut(&assets.habitat_mat) {
        m.uniforms.star_pos_and_time = star_pos.extend(elapsed);
    }
    if let Some(mut m) = tech_mats.get_mut(&assets.mirror_mat) {
        m.uniforms.star_pos_and_time = star_pos.extend(elapsed);
    }
    let to_star = (star_pos - focus_pos).normalize_or(Vec3::NEG_X);
    if let Some(mut s) = shield_mats.get_mut(&assets.shield_passive_mat) {
        s.uniforms.params.x = elapsed;
        s.uniforms.star_dir_and_pulse = to_star.extend(s.uniforms.star_dir_and_pulse.w);
    }
    if let Some(mut s) = shield_mats.get_mut(&assets.shield_overdrive_mat) {
        s.uniforms.params.x = elapsed;
        s.uniforms.star_dir_and_pulse = to_star.extend(s.uniforms.star_dir_and_pulse.w);
    }
}

/// Synchronizes 3D orbital habitat rings, defense shields, and solar mirrors on inhabited worlds.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Synchronizes three civilization visual layers across ECS queries and custom GPU materials"
)]
pub fn sync_civilization_visuals(
    mut commands: Commands,
    sim_time: Option<Res<SimTime>>,
    opt_techs: Option<Res<UnlockedTechs>>,
    opt_events: Option<Res<CosmicEventManager>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut tech_mats: Option<ResMut<Assets<MegastructureMaterial>>>,
    mut shield_mats: Option<ResMut<Assets<PlanetaryShieldMaterial>>>,
    assets_opt: Option<Res<CivilizationVisualAssets>>,
    camera_query: Query<&crate::rendering::camera::PanOrbitCamera>,
    star_query: Query<&SimPosition, With<CentralStar>>,
    hosts_query: CivVisualHostQuery,
    mut visual_sets: ParamSet<(
        Query<(Entity, &OrbitalHabitatRingVisual, &mut Transform)>,
        Query<(
            Entity,
            &PlanetaryShieldVisual,
            &mut Transform,
            &mut MeshMaterial3d<PlanetaryShieldMaterial>,
        )>,
        Query<(Entity, &OrbitalMirrorVisual, &mut Transform)>,
    )>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let (Some(meshes), Some(tech_mats), Some(shield_mats)) = (
        meshes.as_deref_mut(),
        tech_mats.as_deref_mut(),
        shield_mats.as_deref_mut(),
    ) else {
        return;
    };
    let origin = opt_origin.as_deref().copied().unwrap_or_default();

    let surface_host = camera_query
        .iter()
        .find(|c| c.surface_horizon_mode)
        .and_then(|c| c.target_entity);
    let has_mag_shield = opt_techs
        .as_deref()
        .is_some_and(|t| t.has(TechnologyId::MagnetosphericShielding));
    let has_mirror_tech = opt_techs
        .as_deref()
        .is_some_and(|t| t.has(TechnologyId::OrbitalMirrorArray));
    let has_active_crisis = opt_events
        .as_deref()
        .is_some_and(|m| m.primary_active_event().is_some());
    let has_ionizing = opt_events.as_deref().is_some_and(|m| {
        m.active_events.iter().any(|e| {
            e.phase != CosmicEventPhase::Completed
                && matches!(
                    e.event_type,
                    CosmicEventType::GammaRayBurst | CosmicEventType::SolarSuperflare
                )
        })
    });

    let elapsed = sim_time.as_deref().map_or(0.0, |s| s.visual_time_secs);
    let (ring_targets, shield_targets, mirror_targets, focus_pos) =
        collect_civilization_visual_targets(
            &hosts_query,
            surface_host,
            CivVisualFlags {
                has_mag_shield,
                has_mirror_tech,
                has_active_crisis,
                has_ionizing,
            },
            elapsed,
            origin,
        );

    if ring_targets.is_empty()
        && shield_targets.is_empty()
        && mirror_targets.is_empty()
        && visual_sets.p0().is_empty()
        && visual_sets.p1().is_empty()
        && visual_sets.p2().is_empty()
    {
        return;
    }

    let star_pos = star_query
        .iter()
        .next()
        .map_or_else(|| origin.to_render(DVec3::ZERO), |p| origin.to_render(p.0));
    let assets = ensure_civilization_assets(
        &mut commands,
        meshes,
        tech_mats,
        shield_mats,
        assets_opt.as_deref(),
    );
    sync_civilization_materials(
        &assets,
        tech_mats,
        shield_mats,
        star_pos,
        focus_pos,
        elapsed,
    );

    sync_civilization_layer(
        &mut commands,
        ring_targets,
        &mut visual_sets.p0(),
        |v| v.host,
        |host| OrbitalHabitatRingVisual { host },
        &assets.ring_mesh,
        &assets.habitat_mat,
    );
    sync_shield_layer(
        &mut commands,
        shield_targets,
        &mut visual_sets.p1(),
        &assets,
    );
    sync_civilization_layer(
        &mut commands,
        mirror_targets,
        &mut visual_sets.p2(),
        |v| v.host,
        |host| OrbitalMirrorVisual { host },
        &assets.mirror_mesh,
        &assets.mirror_mat,
    );
}

/// Renders decaying planetary defense interceptor beams and warning reticles on [`CrisisImpactor`] threats.
pub fn draw_crisis_defense_gizmos(
    mut gizmos: Gizmos,
    time: Option<Res<Time>>,
    sim_time: Option<Res<SimTime>>,
    mut interceptor_pool: Option<ResMut<CrisisInterceptorPool>>,
    impactors: Query<(
        &SimPosition,
        Option<&Radius>,
        Option<&Transform>,
        &CrisisImpactor,
    )>,
    camera_query: Query<(&Transform, &crate::rendering::camera::PanOrbitCamera)>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let dt = time
        .as_deref()
        .map_or(0.016, Time::delta_secs)
        .clamp(0.0, 0.1);
    if let Some(ref mut pool) = interceptor_pool {
        for bolt in &mut pool.bolts {
            bolt.remaining_secs -= dt;
        }
        pool.bolts.retain(|b| b.remaining_secs > 0.0);
    }

    if camera_query.iter().any(|(_, c)| c.surface_horizon_mode) {
        return;
    }

    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    if let Some(pool) = interceptor_pool.as_deref() {
        for bolt in &pool.bolts {
            let alpha = (bolt.remaining_secs / bolt.max_secs.max(0.01)).clamp(0.0, 1.0);
            let s = origin.to_render(bolt.start_au);
            let e = origin.to_render(bolt.end_au);
            gizmos.line(s, e, bolt.color.with_alpha(alpha));
        }
    }

    let cam_pos = camera_query.iter().next().map(|(tf, _)| tf.translation);
    let elapsed = sim_time.as_deref().map_or(0.0, |s| s.visual_time_secs);
    let pulse = 1.0 + 0.25 * (elapsed * 4.5).sin();
    for (pos, opt_rad, opt_tf, imp) in impactors.iter() {
        if imp.deflected {
            continue;
        }
        let center = opt_tf.map_or_else(|| origin.to_render(pos.0), |t| t.translation);
        let vis_r = opt_tf.map_or_else(|| opt_rad.map_or(1e-6_f32, |r| r.0 as f32), |t| t.scale.x);
        let cam_dist = cam_pos.map_or(1.0_f32, |cp| cp.distance(center));
        let ring_r = (vis_r * 2.2).max(cam_dist * 0.022).clamp(1e-6, 0.25) * pulse;
        gizmos.circle(
            Isometry3d::from_translation(center),
            ring_r,
            Color::srgba(1.0, 0.30, 0.22, 0.85),
        );
    }
}
