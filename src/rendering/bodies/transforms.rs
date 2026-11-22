use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::space_weather::{AuroralOvalState, StellarFlareState};
use crate::simulation::tides::TidalState;

use super::VisualAssets;
use super::materials_sync::*;
pub use super::palettes::*;
pub use super::scaling::*;
use super::shadows::*;

pub type CelestialEnvironmentOptions<'a> = (
    Option<&'a PlanetaryClimate>,
    Option<&'a BiosphereState>,
    Option<&'a VolatileInventory>,
    Option<&'a PlanetaryRingSystem>,
    Option<&'a TidalState>,
    Option<&'a crate::simulation::geology::GeologicalState>,
);

pub type CelestialDynamicsOptions<'a> = (
    Option<&'a SpinState>,
    Option<&'a ElectromagneticFieldState>,
    Option<&'a BlackHoleStarState>,
    Option<&'a Children>,
    Option<&'a PlanetaryBasins>,
    Option<&'a SatelliteOf>,
);

pub type CelestialInternalOptions<'a> = (
    Option<&'a AuroralOvalState>,
    Option<&'a StellarFlareState>,
    Option<&'a InternalDifferentiation>,
    Option<&'a AtmosphericStormState>,
    Option<&'a crate::simulation::accretion::RocheLobeOverflow>,
);

pub type CelestialBodyQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    &'a Mass,
    &'a Radius,
    &'a Temperature,
    &'a Composition,
    &'a CelestialBody,
    &'a mut Transform,
    &'a MeshMaterial3d<PlanetMaterial>,
    &'a mut Mesh3d,
    CelestialEnvironmentOptions<'a>,
    CelestialDynamicsOptions<'a>,
    CelestialInternalOptions<'a>,
);

pub type CelestialBodyQuery<'w, 's> = Query<'w, 's, CelestialBodyQueryItem<'static>>;

fn find_star_position_and_min_orbit(
    query: &CelestialBodyQuery,
    origin: &crate::rendering::origin::RenderOrigin,
    stars: &mut Vec<(Entity, Vec3, f32)>,
) -> (Vec3, f64, Option<Entity>, f32) {
    stars.clear();
    let mut s_pos = Vec3::ZERO;
    let mut s_mass = 1.0;
    let mut s_ent = None;

    for (e, p, m, _, _, _, b, _, _, _, _, (_, _, opt_bhs, ..), _) in query.iter() {
        if b.body_type.is_star_or_remnant() {
            let p_vec = origin.to_render(p.0);
            if stars.is_empty() || m.0 > s_mass {
                s_pos = p_vec;
                s_mass = m.0;
                s_ent = Some(e);
            }
            let lum_weight = if b.body_type == BodyType::BlackHole && opt_bhs.is_none() {
                0.001_f32
            } else if b.body_type == BodyType::WhiteDwarf {
                0.01_f32
            } else if matches!(
                b.body_type,
                BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
            ) {
                0.02_f32
            } else {
                (m.0.max(0.01) as f32).powf(3.5).clamp(1e-4, 1.0e6)
            };
            stars.push((e, p_vec, lum_weight));
        }
    }

    if stars.len() != 1 {
        return (s_pos, s_mass, s_ent, 1.0);
    }

    let mut min_r = f32::MAX;
    for (_, p, ..) in query.iter() {
        let p_vec = origin.to_render(p.0);
        let dist = (p_vec - s_pos).length();
        if dist > 0.001 && dist < min_r {
            min_r = dist;
        }
    }

    let min_orbit_r = if min_r < f32::MAX { min_r } else { 1.0 };
    (s_pos, s_mass, s_ent, min_orbit_r)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Moon and host collection in render-origin coordinates"
)]
fn collect_system_moons(
    query: &CelestialBodyQuery,
    config: &SimulationConfig,
    origin: &crate::rendering::origin::RenderOrigin,
    star_pos: Vec3,
    min_orbit_r: f32,
    cam_pos: Option<Vec3>,
    show_atmospheres: bool,
    all_moons: &mut Vec<(Entity, Vec3, f32, Option<Entity>)>,
    all_parents: &mut Vec<(Entity, Vec3, f64)>,
) {
    all_moons.clear();
    all_parents.clear();
    for (
        m_ent,
        pos,
        mass,
        radius,
        _,
        comp,
        body,
        _,
        _,
        _,
        _,
        (_, _, _, _, _, opt_sat),
        (_, _, opt_diff, ..),
    ) in query.iter()
    {
        let m_pos = origin.to_render(pos.0);
        all_parents.push((m_ent, m_pos, mass.0));
        let is_moon = opt_sat.is_some() || body.body_type == BodyType::Moon;
        let is_host_planet = !is_moon
            && matches!(
                body.body_type,
                BodyType::TerrestrialPlanet
                    | BodyType::SuperEarth
                    | BodyType::GasGiant
                    | BodyType::IceGiant
                    | BodyType::Protoplanet
            );
        if is_moon || is_host_planet {
            let r_orb = (m_pos - star_pos).length();
            let m_cam_dist = cam_pos.map_or(25.0, |cp| cp.distance(m_pos));
            let m_vis_rad = compute_view_visual_radius(
                radius.0,
                body.body_type,
                mass.0,
                Some(comp),
                opt_diff,
                show_atmospheres,
                r_orb,
                min_orbit_r,
                m_cam_dist,
                config,
            );
            // Moons record `Some(parent)` (or `None` if unbound); non-moon host planets tag
            // themselves `Some(m_ent)` so moons can resolve parent lunar eclipses & planetshine.
            let parent_tag = if is_moon {
                opt_sat.map(|s| s.parent)
            } else {
                Some(m_ent)
            };
            all_moons.push((m_ent, m_pos, m_vis_rad, parent_tag));
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Geometry pose calculation helper"
)]
fn update_body_geometry_and_pose(
    transform: &mut Transform,
    config: &SimulationConfig,
    body_type: BodyType,
    mass_solar: f64,
    physical_radius_au: f64,
    comp: &Composition,
    opt_diff: Option<&InternalDifferentiation>,
    show_atmospheres: bool,
    star_pos: Vec3,
    min_orbit_r: f32,
    cam_pos: Option<Vec3>,
    is_blown_out: bool,
    opt_spin: Option<&SpinState>,
) -> (f32, f32) {
    let r_orb = (transform.translation - star_pos).length();
    let cam_dist = cam_pos.map_or(25.0, |cp| cp.distance(transform.translation));
    let (visual_radius, full_visual_radius) = if body_type == BodyType::QuasiStar && is_blown_out {
        let r_schwa = (1.974e-8 * mass_solar).max(1e-4);
        let r_bh = compute_realistic_visual_radius(
            r_schwa,
            BodyType::BlackHole,
            r_orb,
            min_orbit_r,
            cam_dist,
            config,
        );
        (r_bh, r_bh)
    } else {
        let full = compute_realistic_visual_radius(
            physical_radius_au,
            body_type,
            r_orb,
            min_orbit_r,
            cam_dist,
            config,
        );
        let eff = compute_view_visual_radius(
            physical_radius_au,
            body_type,
            mass_solar,
            Some(comp),
            opt_diff,
            show_atmospheres,
            r_orb,
            min_orbit_r,
            cam_dist,
            config,
        );
        (eff, full)
    };
    transform.scale = Vec3::splat(visual_radius);

    if let Some(spin) = opt_spin {
        let spin_dir = if spin.spin_vector.length_squared() > 1e-12 {
            spin.spin_vector.normalize().as_vec3()
        } else {
            let tilt_rad = (spin.axial_tilt_degrees as f32).to_radians();
            Vec3::new(tilt_rad.sin(), tilt_rad.cos(), 0.0)
        };
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, spin_dir);
    }

    (visual_radius, full_visual_radius)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Body uniform synchronization helper"
)]
fn sync_body_uniforms(
    mat: &mut PlanetMaterial,
    entity: Entity,
    body: &CelestialBody,
    mass: &Mass,
    physical_radius_au: f64,
    temp: &Temperature,
    comp: &Composition,
    visual_time: f32,
    elapsed_years: f64,
    current_dt_yr: f64,
    transform: &Transform,
    visual_radius: f32,
    full_visual_radius: f32,
    star_pos: Vec3,
    star_mass: f64,
    star_dir: Vec3,
    all_moons: &[(Entity, Vec3, f32, Option<Entity>)],
    all_parents: &[(Entity, Vec3, f64)],
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_vol: Option<&VolatileInventory>,
    opt_rings: Option<&PlanetaryRingSystem>,
    opt_spin: Option<&SpinState>,
    opt_em: Option<&ElectromagneticFieldState>,
    opt_bhs: Option<&BlackHoleStarState>,
    opt_basins: Option<&PlanetaryBasins>,
    opt_sat: Option<&SatelliteOf>,
    opt_tidal: Option<&TidalState>,
    opt_geo: Option<&crate::simulation::geology::GeologicalState>,
    opt_aurora: Option<&AuroralOvalState>,
    opt_flare: Option<&StellarFlareState>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_storm: Option<&AtmosphericStormState>,
    opt_roche: Option<&crate::simulation::accretion::RocheLobeOverflow>,
    show_atmospheres: bool,
    show_clouds: bool,
) {
    update_body_material_properties(
        mat,
        body,
        mass,
        temp,
        comp,
        visual_time,
        elapsed_years,
        current_dt_yr,
        transform,
        visual_radius,
        star_dir,
        opt_climate,
        opt_bio,
        opt_vol,
        opt_spin,
        opt_em,
        opt_bhs,
        opt_basins,
        opt_tidal,
        opt_geo,
        opt_aurora,
        opt_flare,
        opt_diff,
        opt_storm,
        show_atmospheres,
        show_clouds,
    );

    let is_satellite = opt_sat.is_some() || body.body_type == BodyType::Moon;
    let (tidal_primary_pos, tidal_primary_mass) = opt_sat
        .and_then(|sat| {
            all_parents
                .iter()
                .find(|&&(e, ..)| e == sat.parent)
                .map(|&(_, p_pos, p_mass)| (p_pos, p_mass))
        })
        .unwrap_or((star_pos, star_mass));
    mat.extension.uniforms.tidal_deformation = if body.body_type == BodyType::BlackHole {
        Vec4::ZERO
    } else {
        compute_tidal_deformation_uniform(
            transform.translation,
            tidal_primary_pos,
            tidal_primary_mass,
            mass.0,
            physical_radius_au,
            opt_tidal,
            opt_roche,
            is_satellite,
        )
    };

    // Update real-time ring shadow parameters relative to rendered planet sphere scale.
    // Recover canonical ring ratios relative to full physical radius first, then scale up if
    // the gas envelope is stripped so legacy shrunken rings stay aligned with VisualRingChild.
    let base_ring_shadow = super::shadows::compute_ring_shadow_params_for_body(
        opt_rings,
        physical_radius_au,
        &body.name,
    );
    mat.extension.uniforms.ring_shadow_params = if base_ring_shadow.w > 0.5
        && !show_atmospheres
        && visual_radius > 1e-6
        && full_visual_radius > visual_radius * 1.001
    {
        let core_ratio_inv = full_visual_radius / visual_radius;
        Vec4::new(
            base_ring_shadow.x * core_ratio_inv,
            base_ring_shadow.y * core_ratio_inv,
            base_ring_shadow.z,
            base_ring_shadow.w,
        )
    } else {
        base_ring_shadow
    };

    // Update real-time moon solar eclipse parameters
    let (moons_pos, moons_data) = compute_eclipse_moons_data(
        entity,
        transform.translation,
        visual_radius,
        star_dir,
        all_moons,
    );
    mat.extension.uniforms.eclipse_moons_pos = moons_pos;
    mat.extension.uniforms.eclipse_moons_data = moons_data;
}

fn resolve_expected_mesh(
    body: &CelestialBody,
    radius_au: f64,
    entity: Entity,
    visual_assets: &VisualAssets,
    opt_player_state: Option<&PlayerInteractionState>,
    focused_host: Option<Entity>,
) -> Handle<Mesh> {
    let target_mesh = select_body_mesh(body, radius_au, visual_assets);
    let is_selected = opt_player_state.is_some_and(|ps| ps.selected_entity == Some(entity))
        || focused_host == Some(entity);
    if is_selected {
        if body.body_type.is_star_or_remnant() {
            visual_assets.star_mesh_hires.clone()
        } else if body.body_type.is_planet() || body.body_type == BodyType::Moon {
            visual_assets.planet_mesh_hires.clone()
        } else {
            target_mesh
        }
    } else {
        target_mesh
    }
}

fn resolve_focused_and_surface_host(
    opt_cam: Option<&crate::rendering::camera::PanOrbitCamera>,
    opt_player_state: Option<&PlayerInteractionState>,
    probes_query: &Query<&crate::simulation::probes::FlightComputer>,
) -> (Option<Entity>, Option<Entity>) {
    let raw_target = opt_player_state
        .and_then(|ps| ps.selected_entity)
        .or_else(|| opt_cam.and_then(|c| c.target_entity));
    let resolved_host = raw_target.map(|ent| {
        probes_query
            .get(ent)
            .ok()
            .map_or(ent, |fc| fc.target_entity)
    });
    let surface_host = if opt_cam.is_some_and(|c| c.surface_horizon_mode) {
        resolved_host.or_else(|| {
            opt_cam.and_then(|c| c.target_entity).map(|ent| {
                probes_query
                    .get(ent)
                    .ok()
                    .map_or(ent, |fc| fc.target_entity)
            })
        })
    } else {
        None
    };
    (resolved_host, surface_host)
}

/// Freezes the per-frame visual spin increment on the active surface-mode host world
/// while the player is standing on its surface so `world_up` does not step across the
/// coarse 1 AU `f32` mantissa grid (while `planet.time` still animates waves and clouds).
#[inline]
pub fn clamp_surface_mode_spin_dt_yr(
    current_dt_yr: f64,
    _opt_spin: Option<&SpinState>,
    is_surface_host: bool,
) -> f64 {
    if is_surface_host || current_dt_yr <= 0.0 {
        return 0.0;
    }
    current_dt_yr
}

#[allow(
    clippy::too_many_arguments,
    reason = "Cutaway strata parameters account for active atmosphere stripping and differentiation"
)]
fn resolve_cutaway_uniform_params(
    body: &CelestialBody,
    mass_solar: f64,
    radius_au: f64,
    comp: &Composition,
    visual_radius: f32,
    opt_diff: Option<&InternalDifferentiation>,
    opt_player_state: Option<&PlayerInteractionState>,
    entity: Entity,
) -> Vec4 {
    let is_cutaway = opt_player_state.is_some_and(|ps| {
        ps.is_cutaway_active()
            && ps.overlay_mode != DiagnosticOverlayMode::Hidden
            && (ps.selected_entity == Some(entity)
                || (ps.selected_entity.is_none() && ps.hovered_entity == Some(entity)))
    });

    if is_cutaway {
        let show_atmo = opt_player_state.is_none_or(PlayerInteractionState::show_atmospheres);
        let (eff_rad, eff_type) = super::view_overrides::resolve_view_body_radius_and_type(
            mass_solar,
            radius_au,
            body.body_type,
            comp,
            opt_diff,
            show_atmo,
        );
        let mut eff_body = body.clone();
        eff_body.body_type = eff_type;
        let strata =
            crate::rendering::effects::planetary::resolve_cutaway_strata_with_visual_radius(
                &eff_body,
                eff_rad,
                visual_radius,
                opt_diff,
            );
        let is_undifferentiated = strata.core_r <= 1e-6;
        let core_ratio = if is_undifferentiated || visual_radius <= 1e-6 {
            0.0
        } else {
            (strata.core_r / visual_radius).clamp(0.05, 0.75)
        };
        let mantle_min = if is_undifferentiated {
            0.05
        } else {
            (core_ratio + 0.05).min(0.98)
        };
        let mantle_ratio = if visual_radius > 1e-6 {
            (strata.mantle_r / visual_radius).clamp(mantle_min, 0.99)
        } else {
            0.95
        };
        let inner_core_ratio = if is_undifferentiated || visual_radius <= 1e-6 {
            0.0
        } else {
            (strata.inner_core_r / visual_radius).clamp(0.02, (core_ratio - 0.02).max(0.03))
        };
        let active_flag = if strata.has_theia_llsvp { 2.0 } else { 1.0 };
        Vec4::new(active_flag, inner_core_ratio, core_ratio, mantle_ratio)
    } else {
        Vec4::ZERO
    }
}

#[inline]
fn planet_material_state_eq(a: &PlanetMaterial, b: &PlanetMaterial) -> bool {
    a.extension.uniforms == b.extension.uniforms
        && a.base.base_color == b.base.base_color
        && a.base.emissive == b.base.emissive
        && a.base.perceptual_roughness == b.base.perceptual_roughness
        && a.base.metallic == b.base.metallic
        && a.base.alpha_mode == b.base.alpha_mode
        && a.base.unlit == b.base.unlit
}

fn borrow_scratch_material<'a>(
    scratch_mat: &'a mut Option<PlanetMaterial>,
    existing: &PlanetMaterial,
) -> &'a mut PlanetMaterial {
    let was_some = scratch_mat.is_some();
    let m = scratch_mat.get_or_insert_with(|| existing.clone());
    if was_some {
        m.clone_from(existing);
    }
    m
}

fn resolve_sync_timing(
    sim_time: Option<&SimTime>,
    time_warp: Option<&TimeWarp>,
) -> (f32, f64, f64) {
    let is_paused = time_warp.is_some_and(|tw| tw.is_paused && !tw.step_once);
    let visual_time = sim_time.map_or(0.0, |st| st.visual_time_secs);
    let elapsed_years = sim_time.map_or(0.0, |st| st.elapsed_years);
    let current_dt_yr = if is_paused {
        0.0
    } else {
        sim_time.map_or(0.0, |st| st.current_dt_yr)
    };
    (visual_time, elapsed_years, current_dt_yr)
}

#[allow(clippy::too_many_arguments, reason = "Material synchronization helper")]
fn sync_single_body_material(
    mat_handle: &MeshMaterial3d<PlanetMaterial>,
    materials: &mut Assets<PlanetMaterial>,
    scratch_mat: &mut Option<PlanetMaterial>,
    entity: Entity,
    body: &CelestialBody,
    mass: &Mass,
    physical_radius_au: f64,
    temp: &Temperature,
    comp: &Composition,
    visual_time: f32,
    elapsed_years: f64,
    current_dt_yr: f64,
    transform: &Transform,
    visual_radius: f32,
    full_visual_radius: f32,
    star_pos: Vec3,
    star_mass: f64,
    star_dir: Vec3,
    moons_scratch: &[(Entity, Vec3, f32, Option<Entity>)],
    parents_scratch: &[(Entity, Vec3, f64)],
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_vol: Option<&VolatileInventory>,
    opt_rings: Option<&PlanetaryRingSystem>,
    opt_spin: Option<&SpinState>,
    opt_em: Option<&ElectromagneticFieldState>,
    opt_bhs: Option<&BlackHoleStarState>,
    opt_basins: Option<&PlanetaryBasins>,
    opt_sat: Option<&SatelliteOf>,
    opt_tidal: Option<&TidalState>,
    opt_geo: Option<&crate::simulation::geology::GeologicalState>,
    opt_aurora: Option<&AuroralOvalState>,
    opt_flare: Option<&StellarFlareState>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_storm: Option<&AtmosphericStormState>,
    opt_roche: Option<&crate::simulation::accretion::RocheLobeOverflow>,
    opt_player_state: Option<&PlayerInteractionState>,
    show_atmospheres: bool,
    show_clouds: bool,
) {
    let Some(existing) = materials.get(&mat_handle.0) else {
        return;
    };
    let next_mat = borrow_scratch_material(scratch_mat, existing);
    next_mat.extension.uniforms.cutaway_params = resolve_cutaway_uniform_params(
        body,
        mass.0,
        physical_radius_au,
        comp,
        visual_radius,
        opt_diff,
        opt_player_state,
        entity,
    );

    sync_body_uniforms(
        next_mat,
        entity,
        body,
        mass,
        physical_radius_au,
        temp,
        comp,
        visual_time,
        elapsed_years,
        current_dt_yr,
        transform,
        visual_radius,
        full_visual_radius,
        star_pos,
        star_mass,
        star_dir,
        moons_scratch,
        parents_scratch,
        opt_climate,
        opt_bio,
        opt_vol,
        opt_rings,
        opt_spin,
        opt_em,
        opt_bhs,
        opt_basins,
        opt_sat,
        opt_tidal,
        opt_geo,
        opt_aurora,
        opt_flare,
        opt_diff,
        opt_storm,
        opt_roche,
        show_atmospheres,
        show_clouds,
    );

    if !planet_material_state_eq(next_mat, existing)
        && let Some(mut mat) = materials.get_mut(&mat_handle.0)
    {
        mat.clone_from(next_mat);
    }
}

#[inline]
fn resolve_surface_latched_translation(
    pos_d: bevy::math::DVec3,
    opt_origin: Option<&crate::rendering::origin::RenderOrigin>,
    is_surface: bool,
    opt_cam: Option<&crate::rendering::camera::PanOrbitCamera>,
) -> Vec3 {
    let origin = opt_origin.copied().unwrap_or_default();
    let raw_pos = origin.to_render(pos_d);
    if opt_origin.is_none()
        && is_surface
        && let Some(c) = opt_cam
        && c.target_focus.length_squared() > 1e-6
        && c.target_focus.distance(raw_pos) < 0.05
    {
        c.target_focus
    } else {
        raw_pos
    }
}

type TransformCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        Option<&'static crate::rendering::camera::PanOrbitCamera>,
    ),
    (With<Camera3d>, Without<CelestialBody>),
>;

#[allow(clippy::type_complexity, reason = "Celestial transform sync queries")]
pub fn sync_celestial_transforms(
    (sim_time, time_warp): (Option<Res<SimTime>>, Option<Res<TimeWarp>>),
    config: Res<SimulationConfig>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    mut light_query: Query<&mut PointLight>,
    (camera_query, probes_query): (
        TransformCameraQuery,
        Query<&crate::simulation::probes::FlightComputer>,
    ),
    opt_player_state: Option<Res<PlayerInteractionState>>,
    mut query: CelestialBodyQuery,
    mut stars_scratch: Local<Vec<(Entity, Vec3, f32)>>,
    mut moons_scratch: Local<Vec<(Entity, Vec3, f32, Option<Entity>)>>,
    mut parents_scratch: Local<Vec<(Entity, Vec3, f64)>>,
    mut scratch_mat: Local<Option<PlanetMaterial>>,
) {
    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    let cam_entry = camera_query.iter().next();
    let cam_pos = cam_entry.map(|(t, _)| t.translation);
    let opt_cam = cam_entry.and_then(|(_, c)| c);
    let ps = opt_player_state.as_deref();
    let (focused_host, surface_host) = resolve_focused_and_surface_host(opt_cam, ps, &probes_query);
    let (star_pos, star_mass, _, min_orbit_r) =
        find_star_position_and_min_orbit(&query, &origin, &mut stars_scratch);
    let show_atmospheres = ps.is_none_or(PlayerInteractionState::show_atmospheres);
    let show_clouds = ps.is_none_or(PlayerInteractionState::show_clouds);

    collect_system_moons(
        &query,
        &config,
        &origin,
        star_pos,
        min_orbit_r,
        cam_pos,
        show_atmospheres,
        &mut moons_scratch,
        &mut parents_scratch,
    );
    let (visual_time, elapsed_years, current_dt_yr) =
        resolve_sync_timing(sim_time.as_deref(), time_warp.as_deref());

    for (
        entity,
        pos,
        mass,
        radius,
        temp,
        comp,
        body,
        mut transform,
        mat_handle,
        mut mesh,
        (opt_climate, opt_bio, opt_vol, opt_rings, opt_tidal, opt_geo),
        (opt_spin, opt_em, opt_bhs, opt_children, opt_basins, opt_sat),
        (opt_aurora, opt_flare, opt_diff, opt_storm, opt_roche),
    ) in query.iter_mut()
    {
        let is_surface = surface_host == Some(entity);
        transform.translation =
            resolve_surface_latched_translation(pos.0, opt_origin.as_deref(), is_surface, opt_cam);
        let eff_cam = if is_surface {
            Some(transform.translation)
        } else {
            cam_pos
        };
        let eff_dt_yr = clamp_surface_mode_spin_dt_yr(current_dt_yr, opt_spin, is_surface);
        let star_dir =
            resolve_star_direction(entity, transform.translation, star_pos, &stars_scratch);
        let is_blown_out = opt_bhs.is_some_and(|s| s.is_blown_out);
        update_star_lights_and_strobes(
            opt_children,
            &mut light_query,
            body.body_type,
            is_blown_out,
            visual_time,
        );

        let (visual_radius, full_visual_radius) = update_body_geometry_and_pose(
            &mut transform,
            &config,
            body.body_type,
            mass.0,
            radius.0,
            comp,
            opt_diff,
            show_atmospheres,
            star_pos,
            min_orbit_r,
            eff_cam,
            is_blown_out,
            opt_spin,
        );

        let expected =
            resolve_expected_mesh(body, radius.0, entity, &visual_assets, ps, focused_host);
        if mesh.0 != expected {
            mesh.0 = expected;
        }

        sync_single_body_material(
            mat_handle,
            &mut materials,
            &mut scratch_mat,
            entity,
            body,
            mass,
            radius.0,
            temp,
            comp,
            visual_time,
            elapsed_years,
            eff_dt_yr,
            &transform,
            visual_radius,
            full_visual_radius,
            star_pos,
            star_mass,
            star_dir,
            &moons_scratch,
            &parents_scratch,
            opt_climate,
            opt_bio,
            opt_vol,
            opt_rings,
            opt_spin,
            opt_em,
            opt_bhs,
            opt_basins,
            opt_sat,
            opt_tidal,
            opt_geo,
            opt_aurora,
            opt_flare,
            opt_diff,
            opt_storm,
            opt_roche,
            ps,
            show_atmospheres,
            show_clouds,
        );
    }
}

/// Post-camera zero-allocation pass that resynchronizes camera-adaptive `Transform.scale`
/// and `planet_center_and_radius.w` using the current frame's final camera world position,
/// eliminating 1-frame visual scale lag during fast interplanetary camera flights.
#[allow(
    clippy::type_complexity,
    reason = "Disjoint ECS queries for camera and celestial body scale sync"
)]
pub fn sync_camera_adaptive_body_scales(
    config: Res<SimulationConfig>,
    mut materials: Option<ResMut<Assets<PlanetMaterial>>>,
    opt_player_state: Option<Res<PlayerInteractionState>>,
    camera_query: Query<
        (
            &Transform,
            Option<&crate::rendering::camera::PanOrbitCamera>,
        ),
        (With<Camera3d>, Without<CelestialBody>),
    >,
    probes_query: Query<&crate::simulation::probes::FlightComputer>,
    mut bodies_query: Query<
        (
            Entity,
            &Mass,
            &Radius,
            Option<&Composition>,
            &CelestialBody,
            &mut Transform,
            Option<&MeshMaterial3d<PlanetMaterial>>,
            Option<&BlackHoleStarState>,
            Option<&InternalDifferentiation>,
        ),
        Without<Camera3d>,
    >,
) {
    let Some((cam_tf, opt_cam)) = camera_query.iter().next() else {
        return;
    };
    let cam_pos = cam_tf.translation;
    let (_, surface_host) =
        resolve_focused_and_surface_host(opt_cam, opt_player_state.as_deref(), &probes_query);
    let show_atmospheres = opt_player_state
        .as_deref()
        .is_none_or(PlayerInteractionState::show_atmospheres);

    let mut star_pos = Vec3::ZERO;
    let mut star_mass = -1.0_f64;
    let mut star_count = 0_usize;
    for (_, mass, _, _, body, tf, _, _, _) in bodies_query.iter() {
        if body.body_type.is_star_or_remnant() {
            star_count += 1;
            if mass.0 > star_mass {
                star_mass = mass.0;
                star_pos = tf.translation;
            }
        }
    }

    let min_orbit_r = if star_count == 1 {
        let mut min_r = f32::MAX;
        for (_, _, _, _, _, tf, _, _, _) in bodies_query.iter() {
            let dist = (tf.translation - star_pos).length();
            if dist > 0.001 && dist < min_r {
                min_r = dist;
            }
        }
        if min_r < f32::MAX { min_r } else { 1.0 }
    } else {
        1.0
    };

    for (entity, mass, radius, opt_comp, body, mut tf, opt_mat, opt_bhs, opt_diff) in
        bodies_query.iter_mut()
    {
        // Skip entities that still hold an unsynced default Transform::IDENTITY (e.g. headless spawns)
        if (tf.scale.x - 1.0).abs() < 1e-6 && radius.0 < 0.25 {
            continue;
        }
        let r_orb = (tf.translation - star_pos).length();
        let cam_dist = if surface_host == Some(entity) {
            0.0
        } else {
            cam_pos.distance(tf.translation)
        };
        let is_blown_out = opt_bhs.is_some_and(|s| s.is_blown_out);
        let visual_radius = if body.body_type == BodyType::QuasiStar && is_blown_out {
            let r_schwa = (1.974e-8 * mass.0).max(1e-4);
            compute_realistic_visual_radius(
                r_schwa,
                BodyType::BlackHole,
                r_orb,
                min_orbit_r,
                cam_dist,
                &config,
            )
        } else {
            compute_view_visual_radius(
                radius.0,
                body.body_type,
                mass.0,
                opt_comp,
                opt_diff,
                show_atmospheres,
                r_orb,
                min_orbit_r,
                cam_dist,
                &config,
            )
        };

        let scale_tol = (visual_radius * 1e-4).max(1e-10);
        if (tf.scale.x - visual_radius).abs() > scale_tol {
            tf.scale = Vec3::splat(visual_radius);
            if let (Some(mats), Some(mat_handle)) = (materials.as_deref_mut(), opt_mat)
                && let Some(mut mat) = mats.get_mut(&mat_handle.0)
            {
                mat.extension.uniforms.planet_center_and_radius.w = visual_radius;
            }
        }
    }
}
