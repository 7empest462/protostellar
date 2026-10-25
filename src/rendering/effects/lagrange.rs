//! Gravitational Lagrange Libration Points (L1-L5) and Hill Sphere rendering.
//!
//! Computes the exact 3D positions of the collinear (L1, L2, L3) and triangular (L4, L5)
//! equilibrium points for the Circular Restricted Three-Body Problem (CR3BP), along with
//! the gravitational Hill sphere:
//! - L1: Between primary and secondary at r_H inside secondary
//! - L2: Behind secondary at r_H outside secondary
//! - L3: Opposite side of primary at R * (1 + 5/12 mu)
//! - L4: Leading equilateral triangle point (+60 deg ahead in orbital plane)
//! - L5: Trailing equilateral triangle point (-60 deg behind in orbital plane)
//! - Hill Sphere: r_H = R * (m / 3M)^(1/3)

use bevy::prelude::*;

use crate::simulation::components::{
    BodyType, CelestialBody, CentralStar, Mass, SatelliteOf, SimPosition, SimVelocity,
};
use crate::simulation::resources::{
    DiagnosticOverlayMode, LagrangeOverlayConfig, OrbitVisualizationMode, PlayerInteractionState,
};

/// 3D Astrodynamical coordinates of the five Lagrange points and Hill radius.
#[derive(Debug, Clone, Copy)]
pub struct LagrangeSystemCoordinates {
    pub l1: Vec3,
    pub l2: Vec3,
    pub l3: Vec3,
    pub l4: Vec3,
    pub l5: Vec3,
    pub hill_radius: f32,
    pub primary_pos: Vec3,
    pub secondary_pos: Vec3,
}

// Precomputed trigonometric constants for 60-degree equilateral libration triangles
const COS_60: f32 = 0.5;
const SIN_60: f32 = 0.866_025_4;

/// Computes the exact 3D positions of L1-L5 for a secondary body orbiting a primary mass.
pub fn compute_cr3bp_lagrange_points(
    primary_pos: Vec3,
    primary_vel: Vec3,
    primary_mass: f64,
    secondary_pos: Vec3,
    secondary_vel: Vec3,
    secondary_mass: f64,
) -> Option<LagrangeSystemCoordinates> {
    if primary_mass <= 0.0 || secondary_mass <= 0.0 {
        return None;
    }

    let rel_pos = secondary_pos - primary_pos;
    let dist = rel_pos.length();
    if dist < 1e-4 {
        return None;
    }

    let u_rad = rel_pos / dist;

    // Orbital angular momentum and normal vector
    let rel_vel = secondary_vel - primary_vel;
    let ang_mom = rel_pos.cross(rel_vel);
    let h_len = ang_mom.length();

    let u_norm = if h_len > 1e-5 {
        ang_mom / h_len
    } else {
        Vec3::Y
    };

    // Transverse vector in the orbital plane pointing in the direction of motion
    let u_trans = u_norm.cross(u_rad).normalize_or_zero();

    // Mass ratio mu = m / (M + m)
    let mu = (secondary_mass / (primary_mass + secondary_mass)) as f32;
    let hill_radius = dist * ((secondary_mass / (3.0 * primary_mass)).cbrt() as f32);

    // Collinear points
    let l1 = secondary_pos - u_rad * hill_radius;
    let l2 = secondary_pos + u_rad * hill_radius;
    let l3 = primary_pos - u_rad * (dist * (1.0 + (5.0 / 12.0) * mu));

    // Triangular points: 60 degrees ahead (L4) and 60 degrees behind (L5)
    let l4 = primary_pos + (u_rad * COS_60 + u_trans * SIN_60) * dist;
    let l5 = primary_pos + (u_rad * COS_60 - u_trans * SIN_60) * dist;

    Some(LagrangeSystemCoordinates {
        l1,
        l2,
        l3,
        l4,
        l5,
        hill_radius,
        primary_pos,
        secondary_pos,
    })
}

/// Renders the 3D Lagrange points, geometric connections, and Hill sphere.
pub fn render_lagrange_points_and_hill_sphere(
    gizmos: &mut Gizmos,
    coords: &LagrangeSystemCoordinates,
    show_hill: bool,
) {
    let marker_scale = (coords.hill_radius * 0.18).clamp(0.04, 1.5);

    // 1. Draw 3D Crosshair Markers for L1 - L5
    draw_lagrange_marker(
        gizmos,
        coords.l1,
        marker_scale,
        Color::srgba(1.00, 0.82, 0.20, 0.95),
    ); // L1 Golden Amber
    draw_lagrange_marker(
        gizmos,
        coords.l2,
        marker_scale,
        Color::srgba(0.35, 0.85, 1.00, 0.95),
    ); // L2 Cyan
    draw_lagrange_marker(
        gizmos,
        coords.l3,
        marker_scale,
        Color::srgba(0.95, 0.40, 0.85, 0.85),
    ); // L3 Magenta
    draw_lagrange_marker(
        gizmos,
        coords.l4,
        marker_scale,
        Color::srgba(0.25, 0.95, 0.50, 0.95),
    ); // L4 Trojan Green
    draw_lagrange_marker(
        gizmos,
        coords.l5,
        marker_scale,
        Color::srgba(0.25, 0.95, 0.50, 0.95),
    ); // L5 Greek Green

    // 2. Collinear Axis Lines: L3 -> Primary -> L1 -> Secondary -> L2
    let axis_color = Color::srgba(0.40, 0.65, 0.90, 0.35);
    gizmos.line(coords.l3, coords.primary_pos, axis_color);
    gizmos.line(coords.primary_pos, coords.l1, axis_color);
    gizmos.line(coords.l1, coords.secondary_pos, axis_color);
    gizmos.line(coords.secondary_pos, coords.l2, axis_color);

    // 3. Equilateral Triangular Libration Lines: Primary <-> L4 <-> Secondary and Primary <-> L5 <-> Secondary
    let tri_color = Color::srgba(0.25, 0.85, 0.55, 0.30);
    gizmos.line(coords.primary_pos, coords.l4, tri_color);
    gizmos.line(coords.secondary_pos, coords.l4, tri_color);
    gizmos.line(coords.primary_pos, coords.l5, tri_color);
    gizmos.line(coords.secondary_pos, coords.l5, tri_color);

    // 4. Gravitational Hill Sphere around secondary
    if show_hill {
        let hill_color = Color::srgba(0.20, 0.90, 0.45, 0.28);
        gizmos.sphere(
            Isometry3d::from_translation(coords.secondary_pos),
            coords.hill_radius,
            hill_color,
        );
    }
}

fn draw_lagrange_marker(gizmos: &mut Gizmos, pos: Vec3, scale: f32, color: Color) {
    let d = scale * 0.5;

    // 3D Diamond / Octahedron crosshair
    gizmos.line(pos - Vec3::X * d, pos + Vec3::X * d, color);
    gizmos.line(pos - Vec3::Y * d, pos + Vec3::Y * d, color);
    gizmos.line(pos - Vec3::Z * d, pos + Vec3::Z * d, color);

    // Diamond perimeter
    let c1 = pos + Vec3::X * d;
    let c2 = pos + Vec3::Z * d;
    let c3 = pos - Vec3::X * d;
    let c4 = pos - Vec3::Z * d;
    gizmos.line(c1, c2, color);
    gizmos.line(c2, c3, color);
    gizmos.line(c3, c4, color);
    gizmos.line(c4, c1, color);
}

/// Dedicated Bevy render system for Lagrange libration points and Hill spheres.
pub fn draw_lagrange_points_gizmos(
    mut gizmos: Gizmos,
    lagrange_config: Res<LagrangeOverlayConfig>,
    player_state: Res<PlayerInteractionState>,
    star_query: Query<
        (Entity, &SimPosition, &SimVelocity, &Mass, &CelestialBody),
        With<CentralStar>,
    >,
    bodies_query: Query<
        (
            Entity,
            &SimPosition,
            &SimVelocity,
            &Mass,
            &CelestialBody,
            Option<&SatelliteOf>,
        ),
        Without<CentralStar>,
    >,
) {
    let is_hill_overlay = player_state.overlay_mode == DiagnosticOverlayMode::HillSpheresAndGaps;
    if (!lagrange_config.show_lagrange_points && !is_hill_overlay)
        || player_state.overlay_mode == DiagnosticOverlayMode::Hidden
        || player_state.orbit_mode == OrbitVisualizationMode::Off
    {
        return;
    }

    let opt_star = star_query.iter().next();
    let Some((star_ent, s_pos, s_vel, s_mass, _)) = opt_star else {
        return;
    };

    let star_vec = Vec3::new(s_pos.x as f32, s_pos.y as f32, s_pos.z as f32);
    let star_v_vec = Vec3::new(s_vel.x as f32, s_vel.y as f32, s_vel.z as f32);

    // Determine target secondary body: selected body if applicable, else primary planet
    let target_body = if let Some(selected) = player_state.selected_entity {
        if selected == star_ent {
            find_representative_planet(&bodies_query)
        } else {
            bodies_query.get(selected).ok()
        }
    } else {
        find_representative_planet(&bodies_query)
    };

    let Some((_, b_pos, b_vel, b_mass, _, opt_satellite)) = target_body else {
        return;
    };

    let sec_vec = Vec3::new(b_pos.x as f32, b_pos.y as f32, b_pos.z as f32);
    let sec_v_vec = Vec3::new(b_vel.x as f32, b_vel.y as f32, b_vel.z as f32);

    // Resolve primary (if body has a parent planet, e.g. Moon orbiting Earth, use parent)
    let (prim_pos, prim_vel, prim_mass) = if let Some(sat) = opt_satellite {
        if let Ok((_, p_pos, p_vel, p_mass, ..)) = bodies_query.get(sat.parent) {
            (
                Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
                Vec3::new(p_vel.x as f32, p_vel.y as f32, p_vel.z as f32),
                p_mass.0,
            )
        } else {
            (star_vec, star_v_vec, s_mass.0)
        }
    } else {
        (star_vec, star_v_vec, s_mass.0)
    };

    if let Some(coords) =
        compute_cr3bp_lagrange_points(prim_pos, prim_vel, prim_mass, sec_vec, sec_v_vec, b_mass.0)
    {
        render_lagrange_points_and_hill_sphere(
            &mut gizmos,
            &coords,
            lagrange_config.show_hill_sphere || is_hill_overlay,
        );
    }
}

type BodyTuple<'a> = (
    Entity,
    &'a SimPosition,
    &'a SimVelocity,
    &'a Mass,
    &'a CelestialBody,
    Option<&'a SatelliteOf>,
);

fn find_representative_planet<'a>(
    query: &'a Query<
        (
            Entity,
            &SimPosition,
            &SimVelocity,
            &Mass,
            &CelestialBody,
            Option<&SatelliteOf>,
        ),
        Without<CentralStar>,
    >,
) -> Option<BodyTuple<'a>> {
    // Prefer Earth-like, Super-Earth, or Jupiter-like planet
    query
        .iter()
        .find(|(_, _, _, _, b, _)| {
            matches!(
                b.body_type,
                BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::GasGiant
            )
        })
        .or_else(|| query.iter().next())
}
