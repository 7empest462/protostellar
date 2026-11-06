use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::space_weather::{AuroralOvalState, StellarFlareState};
use crate::simulation::tides::TidalState;

use super::materials_sync::*;
pub use super::palettes::*;
use super::shadows::*;
use super::VisualAssets;

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

/// Computes the `tidal_deformation` shader uniform (`xyz` = unit world vector toward dominant
/// tidal perturber, `w` = `roche_stretch` in `0.0..0.78` plus `+2.0` if tidally locked).
pub fn compute_tidal_deformation_uniform(
    world_pos: Vec3,
    primary_pos: Vec3,
    primary_mass_solar: f64,
    body_mass_solar: f64,
    body_radius_au: f64,
    opt_tidal: Option<&TidalState>,
    opt_roche: Option<&crate::simulation::accretion::RocheLobeOverflow>,
    is_satellite: bool,
) -> Vec4 {
    let to_primary = primary_pos - world_pos;
    let dist_au = f64::from(to_primary.length());
    let tidal_dir = to_primary.normalize_or_zero();

    // Eggleton / Fluid Roche limit estimate: r_roche ~ 2.44 * R_body * (M_primary / M_body)^(1/3)
    let mass_ratio = primary_mass_solar / body_mass_solar.max(1e-12);
    let r_roche_au = 2.44 * body_radius_au * mass_ratio.cbrt();

    let mut stretch = if let Some(roche) = opt_roche {
        let overflow_boost = (roche.mass_transfer_rate * 2.5e3).clamp(0.0, 0.35) as f32;
        (0.42 + overflow_boost).clamp(0.35, 0.78)
    } else if dist_au > 1e-5 && r_roche_au > 0.0 {
        let prox = (r_roche_au / dist_au) as f32;
        if prox > 0.33 {
            ((prox - 0.33) * 0.65).clamp(0.0, 0.72)
        } else {
            0.0
        }
    } else {
        0.0
    };

    if !is_satellite && opt_tidal.is_some_and(|t| t.is_tidally_locked) {
        stretch += 2.0;
    }

    Vec4::new(tidal_dir.x, tidal_dir.y, tidal_dir.z, stretch)
}

/// Computes a realistic camera-adaptive visual radius that transitions to near-physical
/// scale on close-up and never crowds neighboring orbits in compact systems.
pub fn compute_realistic_visual_radius(
    physical_radius_au: f64,
    body_type: BodyType,
    r_orb: f32,
    min_orbit_r: f32,
    cam_dist: f32,
    config: &SimulationConfig,
) -> f32 {
    let safe_phys_r = if physical_radius_au.is_finite() {
        physical_radius_au.max(1e-9)
    } else {
        1e-6
    };
    let safe_r_orb = if r_orb.is_finite() {
        r_orb.max(0.0)
    } else {
        1.0
    };
    let safe_min_orbit = if min_orbit_r.is_finite() && min_orbit_r > 0.0 {
        min_orbit_r
    } else {
        1.0
    };
    let safe_cam_dist = if cam_dist.is_finite() {
        cam_dist.max(0.0)
    } else {
        25.0
    };

    let r_true = (safe_phys_r as f32).max(1e-6);
    let r_exag =
        config.calc_visual_radius_with_orbit(safe_phys_r, body_type, safe_r_orb, safe_min_orbit);

    let orbit_spacing_cap = if safe_r_orb > 0.0005 && !body_type.is_star_or_remnant() {
        (safe_r_orb * 0.045).max(r_true * 1.25)
    } else if body_type.is_star_or_remnant() && safe_min_orbit < 0.05 {
        (safe_min_orbit * 0.32).max(r_true)
    } else {
        r_exag
    };
    let max_allowed_exag = r_exag.min(orbit_spacing_cap);

    // In single-precision 32-bit float world coordinates, machine epsilon at distance R_orb
    // is ~ R_orb * 5.96e-8 AU. To prevent GPU vertex snapping and polygonal faceting
    // (where a distant body's sphere collapses to a handful of discrete float quanta),
    // enforce a distance-dependent precision floor of ~ 1,500-2,500 ULPs across the radius.
    let precision_mult = if safe_r_orb > 25.0 { 1.5e-4 } else { 3.6e-5 };
    let float_precision_floor = (safe_r_orb * precision_mult).min(max_allowed_exag);

    let close_scale_floor = if body_type == BodyType::BlackHole {
        max_allowed_exag
    } else if body_type.is_star_or_remnant() {
        (r_true * 1.2)
            .max(float_precision_floor)
            .min(max_allowed_exag)
    } else if body_type == BodyType::Moon {
        let moon_scale = ((r_true / 0.000_042_6_f32).powf(0.45) * 0.46).clamp(0.05, 1.0);
        ((r_true * 2.5).max(0.00018 * moon_scale))
            .max(float_precision_floor * moon_scale)
            .min(max_allowed_exag)
    } else if matches!(
        body_type,
        BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal | BodyType::DustGrain
    ) {
        let minor_precision_floor = (float_precision_floor * 0.35).min(max_allowed_exag);
        (r_true * 2.5)
            .max(0.00010)
            .max(minor_precision_floor)
            .min(max_allowed_exag)
    } else if safe_r_orb > 25.0 {
        // Deep outer system planets (Uranus, Neptune, Pluto, Planet 9):
        // Retain an orbital-distance-proportional visual floor so the sphere has thousands
        // of float quanta across its diameter, completely eliminating polygonal facet snapping.
        (r_true * 2.5)
            .max(max_allowed_exag * 0.65)
            .max(float_precision_floor)
            .min(max_allowed_exag)
    } else {
        (r_true * 2.5)
            .max(0.00018)
            .max(float_precision_floor)
            .min(max_allowed_exag)
    };

    // Sub-linear power-law zoom ramp (exponent 0.78 < 1.0) above `close_dist`:
    // Apparent angular radius Theta(d) = R_vis(d) / d scales as d^(-0.22) in the transition
    // zone and d^(-1.0) outside it, guaranteeing 100% strict monotonicity (dTheta/dd < 0
    // everywhere) with zero mid-flight inflate-deflate-inflate oscillation.
    let close_dist = (close_scale_floor * 8.0).max(0.015);
    if safe_cam_dist <= close_dist || max_allowed_exag <= close_scale_floor {
        close_scale_floor
    } else {
        let scaled = close_scale_floor * (safe_cam_dist / close_dist).powf(0.78);
        scaled.min(max_allowed_exag)
    }
}

/// Resolves the exact rendered visual radius of a celestial body in `O(1)` by reading its synced
/// `Transform.scale.x` when valid, or computing `compute_realistic_visual_radius` if unsynced.
#[inline]
pub fn resolve_synced_visual_radius(
    opt_transform: Option<&Transform>,
    physical_radius_au: f64,
    body_type: BodyType,
    r_orb: f32,
    cam_dist: f32,
    config: &SimulationConfig,
) -> f32 {
    if let Some(tf) = opt_transform {
        let sx = tf.scale.x;
        let is_unsynced_default = (sx - 1.0).abs() < 1e-6 && physical_radius_au < 0.25;
        if sx.is_finite() && sx > 1e-6 && !is_unsynced_default {
            return sx;
        }
    }
    compute_realistic_visual_radius(physical_radius_au, body_type, r_orb, 1.0, cam_dist, config)
}

/// Computes the self-consistent close-up camera framing distance (AU) for focusing on a body,
/// accounting for the camera-adaptive visual radius reduction as the camera approaches.
pub fn compute_close_camera_framing_radius(
    physical_radius_au: f64,
    body_type: BodyType,
    r_orb: f32,
    min_orbit_r: f32,
    config: &SimulationConfig,
) -> f32 {
    let r0 = compute_realistic_visual_radius(
        physical_radius_au,
        body_type,
        r_orb,
        min_orbit_r,
        0.002,
        config,
    );
    let d1 = config.calc_camera_framing_radius(r0);
    let r1 = compute_realistic_visual_radius(
        physical_radius_au,
        body_type,
        r_orb,
        min_orbit_r,
        d1,
        config,
    );
    config.calc_camera_framing_radius(r1)
}

fn find_star_position_and_min_orbit(
    query: &CelestialBodyQuery,
    stars: &mut Vec<(Entity, Vec3)>,
) -> (Vec3, f64, Option<Entity>, f32) {
    stars.clear();
    let mut s_pos = Vec3::ZERO;
    let mut s_mass = 1.0;
    let mut s_ent = None;

    for (e, p, m, _, _, _, b, ..) in query.iter() {
        if b.body_type.is_star_or_remnant() {
            let p_vec = Vec3::new(p.x as f32, p.y as f32, p.z as f32);
            if stars.is_empty() || m.0 > s_mass {
                s_pos = p_vec;
                s_mass = m.0;
                s_ent = Some(e);
            }
            stars.push((e, p_vec));
        }
    }

    if stars.len() != 1 {
        return (s_pos, s_mass, s_ent, 1.0);
    }

    let mut min_r = f32::MAX;
    for (_, p, ..) in query.iter() {
        let p_vec = Vec3::new(p.x as f32, p.y as f32, p.z as f32);
        let dist = (p_vec - s_pos).length();
        if dist > 0.001 && dist < min_r {
            min_r = dist;
        }
    }

    let min_orbit_r = if min_r < f32::MAX { min_r } else { 1.0 };
    (s_pos, s_mass, s_ent, min_orbit_r)
}

fn resolve_star_direction(
    entity: Entity,
    body_pos: Vec3,
    star_pos: Vec3,
    stars: &[(Entity, Vec3)],
) -> Vec3 {
    if stars.is_empty() {
        Vec3::Y
    } else if stars.len() == 1 {
        (star_pos - body_pos).normalize_or_zero()
    } else {
        let closest = stars
            .iter()
            .filter(|(e, _)| *e != entity)
            .min_by(|(_, p1), (_, p2)| {
                let d1 = (*p1 - body_pos).length_squared();
                let d2 = (*p2 - body_pos).length_squared();
                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
            });
        closest.map_or(Vec3::Y, |(_, p)| (*p - body_pos).normalize_or_zero())
    }
}

fn collect_system_moons(
    query: &CelestialBodyQuery,
    config: &SimulationConfig,
    star_pos: Vec3,
    min_orbit_r: f32,
    cam_pos: Option<Vec3>,
    all_moons: &mut Vec<(Entity, Vec3, f32, Option<Entity>)>,
) {
    all_moons.clear();
    for (m_ent, pos, _, radius, _, _, body, _, _, _, _, (_, _, _, _, _, opt_sat), _) in query.iter()
    {
        if opt_sat.is_some() || body.body_type == BodyType::Moon {
            let m_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            let r_orb = (m_pos - star_pos).length();
            let m_cam_dist = cam_pos.map_or(25.0, |cp| cp.distance(m_pos));
            let m_vis_rad = compute_realistic_visual_radius(
                radius.0,
                body.body_type,
                r_orb,
                min_orbit_r,
                m_cam_dist,
                config,
            );
            all_moons.push((m_ent, m_pos, m_vis_rad, opt_sat.map(|s| s.parent)));
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
    star_pos: Vec3,
    min_orbit_r: f32,
    cam_pos: Option<Vec3>,
    is_blown_out: bool,
    opt_spin: Option<&SpinState>,
) -> f32 {
    let r_orb = (transform.translation - star_pos).length();
    let cam_dist = cam_pos.map_or(25.0, |cp| cp.distance(transform.translation));
    let visual_radius = if body_type == BodyType::QuasiStar && is_blown_out {
        let r_schwa = (1.974e-8 * mass_solar).max(1e-4);
        compute_realistic_visual_radius(
            r_schwa,
            BodyType::BlackHole,
            r_orb,
            min_orbit_r,
            cam_dist,
            config,
        )
    } else {
        compute_realistic_visual_radius(
            physical_radius_au,
            body_type,
            r_orb,
            min_orbit_r,
            cam_dist,
            config,
        )
    };
    transform.scale = Vec3::splat(visual_radius);

    if let Some(spin) = opt_spin {
        let spin_dir = if spin.spin_vector.length_squared() > 1e-12
            && (spin.spin_vector.x.abs() > 1e-6 || spin.spin_vector.z.abs() > 1e-6)
        {
            spin.spin_vector.normalize().as_vec3()
        } else {
            let tilt_rad = (spin.axial_tilt_degrees as f32).to_radians();
            Vec3::new(tilt_rad.sin(), tilt_rad.cos(), 0.0)
        };
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, spin_dir);
    }

    visual_radius
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
    star_pos: Vec3,
    star_mass: f64,
    star_dir: Vec3,
    all_moons: &[(Entity, Vec3, f32, Option<Entity>)],
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
    );

    let is_satellite = opt_sat.is_some() || body.body_type == BodyType::Moon;
    mat.extension.uniforms.tidal_deformation = if body.body_type == BodyType::BlackHole {
        Vec4::ZERO
    } else {
        compute_tidal_deformation_uniform(
            transform.translation,
            star_pos,
            star_mass,
            mass.0,
            physical_radius_au,
            opt_tidal,
            opt_roche,
            is_satellite,
        )
    };

    // Update real-time ring shadow parameters
    mat.extension.uniforms.ring_shadow_params =
        compute_ring_shadow_params(opt_rings, physical_radius_au);

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
) -> Handle<Mesh> {
    let target_mesh = select_body_mesh(body, radius_au, visual_assets);
    let is_selected = opt_player_state.is_some_and(|ps| ps.selected_entity == Some(entity));
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

fn resolve_cutaway_uniform_params(
    body: &CelestialBody,
    radius_au: f64,
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
        let strata =
            crate::rendering::effects::planetary::resolve_cutaway_strata_with_visual_radius(
                body,
                radius_au,
                visual_radius,
                opt_diff,
            );
        let core_ratio = if visual_radius > 1e-6 {
            (strata.core_r / visual_radius).clamp(0.05, 0.75)
        } else {
            0.55
        };
        let mantle_min = (core_ratio + 0.05).min(0.98);
        let mantle_ratio = if visual_radius > 1e-6 {
            (strata.mantle_r / visual_radius).clamp(mantle_min, 0.99)
        } else {
            0.95
        };
        let inner_core_ratio = if visual_radius > 1e-6 {
            (strata.inner_core_r / visual_radius).clamp(0.02, (core_ratio - 0.02).max(0.03))
        } else {
            core_ratio * 0.38
        };
        Vec4::new(1.0, inner_core_ratio, core_ratio, mantle_ratio)
    } else {
        Vec4::ZERO
    }
}

pub fn sync_celestial_transforms(
    (sim_time, time_warp): (Option<Res<SimTime>>, Option<Res<TimeWarp>>),
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    mut light_query: Query<&mut PointLight>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<CelestialBody>)>,
    opt_player_state: Option<Res<PlayerInteractionState>>,
    mut query: CelestialBodyQuery,
    mut stars_scratch: Local<Vec<(Entity, Vec3)>>,
    mut moons_scratch: Local<Vec<(Entity, Vec3, f32, Option<Entity>)>>,
) {
    let cam_pos = camera_query.iter().next().map(|t| t.translation);
    let (star_pos, star_mass, _star_entity, min_orbit_r) =
        find_star_position_and_min_orbit(&query, &mut stars_scratch);
    collect_system_moons(
        &query,
        &config,
        star_pos,
        min_orbit_r,
        cam_pos,
        &mut moons_scratch,
    );
    let is_paused = time_warp
        .as_deref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once);
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let elapsed_years = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);
    let current_dt_yr = if is_paused {
        0.0
    } else {
        sim_time.as_deref().map_or(0.0, |st| st.current_dt_yr)
    };

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
        transform.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);

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

        let visual_radius = update_body_geometry_and_pose(
            &mut transform,
            &config,
            body.body_type,
            mass.0,
            radius.0,
            star_pos,
            min_orbit_r,
            cam_pos,
            is_blown_out,
            opt_spin,
        );

        let expected_mesh = resolve_expected_mesh(
            body,
            radius.0,
            entity,
            &visual_assets,
            opt_player_state.as_deref(),
        );
        if mesh.0 != expected_mesh {
            mesh.0 = expected_mesh;
        }

        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            mat.extension.uniforms.cutaway_params = resolve_cutaway_uniform_params(
                body,
                radius.0,
                visual_radius,
                opt_diff,
                opt_player_state.as_deref(),
                entity,
            );

            sync_body_uniforms(
                &mut mat,
                entity,
                body,
                mass,
                radius.0,
                temp,
                comp,
                visual_time,
                elapsed_years,
                current_dt_yr,
                &transform,
                visual_radius,
                star_pos,
                star_mass,
                star_dir,
                &moons_scratch,
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
            );
        }
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
    camera_query: Query<&Transform, (With<Camera3d>, Without<CelestialBody>)>,
    mut bodies_query: Query<
        (
            &Mass,
            &Radius,
            &CelestialBody,
            &mut Transform,
            Option<&MeshMaterial3d<PlanetMaterial>>,
            Option<&BlackHoleStarState>,
        ),
        Without<Camera3d>,
    >,
) {
    let Some(cam_pos) = camera_query.iter().next().map(|t| t.translation) else {
        return;
    };

    let mut star_pos = Vec3::ZERO;
    let mut star_mass = -1.0_f64;
    let mut star_count = 0_usize;
    for (mass, _, body, tf, _, _) in bodies_query.iter() {
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
        for (_, _, _, tf, _, _) in bodies_query.iter() {
            let dist = (tf.translation - star_pos).length();
            if dist > 0.001 && dist < min_r {
                min_r = dist;
            }
        }
        if min_r < f32::MAX {
            min_r
        } else {
            1.0
        }
    } else {
        1.0
    };

    for (mass, radius, body, mut tf, opt_mat, opt_bhs) in bodies_query.iter_mut() {
        // Skip entities that still hold an unsynced default Transform::IDENTITY (e.g. headless spawns)
        if (tf.scale.x - 1.0).abs() < 1e-6 && radius.0 < 0.25 {
            continue;
        }
        let r_orb = (tf.translation - star_pos).length();
        let cam_dist = cam_pos.distance(tf.translation);
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
            compute_realistic_visual_radius(
                radius.0,
                body.body_type,
                r_orb,
                min_orbit_r,
                cam_dist,
                &config,
            )
        };

        if (tf.scale.x - visual_radius).abs() > 1e-7 {
            tf.scale = Vec3::splat(visual_radius);
            if let (Some(mats), Some(mat_handle)) = (materials.as_deref_mut(), opt_mat) {
                if let Some(mut mat) = mats.get_mut(&mat_handle.0) {
                    mat.extension.uniforms.planet_center_and_radius.w = visual_radius;
                }
            }
        }
    }
}
