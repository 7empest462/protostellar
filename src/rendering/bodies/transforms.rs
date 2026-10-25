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
    let r_true = (physical_radius_au as f32).max(1e-6);
    let r_exag =
        (config.calc_visual_radius_with_orbit(physical_radius_au, body_type, r_orb, min_orbit_r)
            * 0.65)
            .max(r_true);

    let orbit_spacing_cap = if r_orb > 0.0005 && !body_type.is_star_or_remnant() {
        (r_orb * 0.045).max(r_true * 1.25)
    } else if body_type.is_star_or_remnant() && min_orbit_r < 0.05 {
        (min_orbit_r * 0.32).max(r_true)
    } else {
        r_exag
    };
    let max_allowed_exag = r_exag.min(orbit_spacing_cap);

    let close_scale_floor = if body_type == BodyType::BlackHole {
        (r_true * 8.0).max(0.002)
    } else if body_type.is_star_or_remnant() {
        (max_allowed_exag * 0.82).max(r_true)
    } else {
        (max_allowed_exag * 0.78).max(r_true)
    };
    let effective_floor = close_scale_floor.min(max_allowed_exag);

    // Use a shared world-space macro-zoom window (0.12..1.20 AU, well above planet-moon orbital
    // separations like 0.012 AU) so a planet and its moons never distort their relative size
    // ratio when viewing either body or zooming in and out.
    let zoom_t = ((cam_dist - 0.12) / 1.08).clamp(0.0, 1.0);
    let smooth_zoom = zoom_t * zoom_t * (3.0 - 2.0 * zoom_t);

    effective_floor + (max_allowed_exag - effective_floor) * smooth_zoom
}

fn find_star_position_and_min_orbit(
    query: &CelestialBodyQuery,
) -> (Vec3, f64, Option<Entity>, f32, Vec<(Entity, Vec3)>) {
    let mut s_pos = Vec3::ZERO;
    let mut s_mass = 1.0;
    let mut s_ent = None;
    let mut stars = Vec::with_capacity(8);

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
        return (s_pos, s_mass, s_ent, 1.0, stars);
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
    (s_pos, s_mass, s_ent, min_orbit_r, stars)
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
) -> Vec<(Entity, Vec3, f32, Option<Entity>)> {
    let mut all_moons = Vec::with_capacity(8);
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
    all_moons
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

pub fn sync_celestial_transforms(
    sim_time: Option<Res<SimTime>>,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    mut light_query: Query<&mut PointLight>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<CelestialBody>)>,
    mut query: CelestialBodyQuery,
) {
    let cam_pos = camera_query.iter().next().map(|t| t.translation);
    let (star_pos, star_mass, _star_entity, min_orbit_r, stars) =
        find_star_position_and_min_orbit(&query);
    let all_moons = collect_system_moons(&query, &config, star_pos, min_orbit_r, cam_pos);
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let elapsed_years = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);
    let current_dt_yr = sim_time.as_deref().map_or(0.0, |st| st.current_dt_yr);

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

        let star_dir = resolve_star_direction(entity, transform.translation, star_pos, &stars);

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

        let target_mesh = select_body_mesh(body, radius.0, &visual_assets);
        if mesh.0 != target_mesh {
            mesh.0 = target_mesh;
        }

        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
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
                &all_moons,
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
