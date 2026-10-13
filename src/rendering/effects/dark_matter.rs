//! Visualization of Galactic Dark Matter Halos (NFW potential profile & equipotential contours).

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

type Halo = (Vec3, Vec3, f32, Color, bool, bool);

/// Collects all supermassive black holes (centers of the colliding galaxies) as halos.
fn collect_halos(
    massive_query: &Query<(&SimPosition, &SimVelocity, &Mass, &CelestialBody)>,
) -> Vec<Halo> {
    let mut halos = Vec::with_capacity(4);
    for (pos, vel, mass, body) in massive_query.iter() {
        if body.body_type == BodyType::BlackHole && mass.0 >= 1_000_000.0 {
            let center = Vec3::new(pos.0.x as f32, pos.0.y as f32, pos.0.z as f32);
            let velocity = Vec3::new(vel.0.x as f32, vel.0.y as f32, vel.0.z as f32);
            let is_andromeda = mass.0 > 50_000_000.0 || body.name.contains("Andromeda");
            let is_merged = body.name.contains("Merged") || body.name.contains("Milkomeda");

            let (halo_mass_solar, base_color) = if is_merged {
                (2.4e12_f32, Color::srgba(0.35, 0.90, 0.95, 0.70)) // Radiant Milkomeda Teal
            } else if is_andromeda {
                (1.4e12_f32, Color::srgba(0.85, 0.30, 1.0, 0.65)) // Andromeda Violet/Magenta
            } else {
                (1.0e12_f32, Color::srgba(0.20, 0.75, 1.0, 0.65)) // Milky Way Electric Blue
            };

            halos.push((
                center,
                velocity,
                halo_mass_solar,
                base_color,
                is_andromeda,
                is_merged,
            ));
        }
    }
    halos
}

/// Number of dark-matter flow tracers drawn per halo.
const DM_TRACERS: usize = 360;

/// Draws a halo as a swirling cloud of dark-matter flow streaks (so the user can see how the
/// unseen mass moves with the galaxy), plus a faint Rs boundary. Streaks follow the NFW circular
/// rotation profile and are tidally drawn toward the other halo during the encounter.
fn draw_single_halo(gizmos: &mut Gizmos, halo: &Halo, other: Option<Vec3>, visual_time: f32) {
    let (center, vel, _halo_m, base_color, _is_m31, is_merged) = halo;
    let rs = 50_000.0_f32; // NFW Characteristic Scale Radius in AU
    let rgba = base_color.to_srgba();
    let (r, g, b) = (rgba.red, rgba.green, rgba.blue);

    // Faint scale-radius boundary (single ring) and virial ring for the merged halo.
    let equatorial = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    gizmos.circle(
        Isometry3d::new(*center, equatorial),
        rs,
        Color::srgba(r, g, b, 0.18),
    );
    if *is_merged {
        gizmos.circle(
            Isometry3d::new(*center, equatorial),
            rs * 3.0,
            Color::srgba(r, g, b, 0.10),
        );
    }

    // Halo bulk-motion arrow (direction the whole halo is travelling).
    let speed = vel.length();
    if speed > 1e-3 {
        let tip = *center + *vel / speed * rs * 0.8;
        gizmos.arrow(*center, tip, Color::srgba(r, g, b, 0.9));
    }

    let overlap = other.map_or(Vec3::ZERO, |o| {
        let d = o - *center;
        let dist = d.length();
        if dist > 100.0 && dist < 180_000.0 {
            d / dist * (1.0 - dist / 180_000.0)
        } else {
            Vec3::ZERO
        }
    });

    let golden = 2.399_963_2_f32;
    for i in 0..DM_TRACERS {
        let f = (i as f32 + 0.5) / DM_TRACERS as f32;
        let y = 1.0 - 2.0 * f;
        let ring = (1.0 - y * y).max(0.0).sqrt();
        let theta = i as f32 * golden;
        let dir = Vec3::new(ring * theta.cos(), y, ring * theta.sin());
        let u = (i as f32 * 0.618_034).fract();
        let radius = rs * 2.5 * u * u + rs * 0.05;
        let x = (radius / rs).max(0.05);
        let m_x = (1.0 + x).ln() - x / (1.0 + x);
        let omega = 0.6 * (m_x / x).sqrt() / x;
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        let axis = if i % 3 == 0 { Vec3::X } else { Vec3::Y };
        let now = Quat::from_axis_angle(axis, sign * omega * visual_time) * dir * radius;
        let prev = Quat::from_axis_angle(axis, sign * omega * (visual_time - 0.25)) * dir * radius;
        let tide = overlap * (radius / (2.5 * rs)) * rs * 0.8;
        let alpha = (0.75 - 0.5 * u).clamp(0.15, 0.75);
        gizmos.line(
            *center + prev + tide,
            *center + now + tide,
            Color::srgba(r, g, b, alpha),
        );
    }
}

/// Draws the interpenetrating tidal bridge, L1 saddle and dynamical friction shock ring.
fn draw_tidal_bridge(gizmos: &mut Gizmos, halos: &[Halo], visual_time: f32) {
    let (Some(&(c1, ..)), Some(&(c2, ..))) = (halos.first(), halos.get(1)) else {
        return;
    };
    let delta = c2 - c1;
    let dist = delta.length();
    let r_overlap = 180_000.0_f32;

    if dist < r_overlap && dist > 100.0 {
        let overlap_frac = (1.0 - dist / r_overlap).clamp(0.0, 1.0);
        let dir = delta / dist;

        // Connecting gravitational bridge vector
        let bridge_alpha = (overlap_frac * 0.65).clamp(0.15, 0.75);
        gizmos.line(c1, c2, Color::srgba(0.55, 0.50, 1.0, bridge_alpha));

        // Gravitational equipotential saddle point (Lagrange L1 analog between the two halos)
        let saddle_pos = c1 + delta * 0.42; // Closer to Milky Way due to lower mass
        let saddle_r = 8_000.0 + (1.0 - overlap_frac) * 12_000.0;
        let saddle_col = Color::srgba(1.0, 0.90, 0.30, bridge_alpha * 0.85);
        gizmos.circle(
            Isometry3d::new(
                saddle_pos,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            saddle_r,
            saddle_col,
        );

        // Chandrasekhar dynamical friction deceleration shock boundary rings
        let mid_pt = (c1 + c2) * 0.5;
        let shock_r = (dist * 0.35).min(50_000.0);
        let shock_pulse = (visual_time * 3.0).sin() * 0.1 + 0.9;
        let shock_col = Color::srgba(1.0, 0.45, 0.20, overlap_frac * 0.50 * shock_pulse);

        // Normal plane perpendicular to the collision axis
        let align_rot = Quat::from_rotation_arc(Vec3::Z, dir);
        gizmos.circle(Isometry3d::new(mid_pt, align_rot), shock_r, shock_col);
    }
}

/// Renders luminous, multi-axis NFW Dark Matter Halo boundary cages,
/// concentric density contours, and interpenetrating tidal bridges between colliding galaxies.
pub fn draw_dark_matter_halos(
    mut gizmos: Gizmos,
    scenario_state: Option<Res<ActiveScenarioState>>,
    disk_params: Res<DiskParameters>,
    player_state: Res<PlayerInteractionState>,
    massive_query: Query<(&SimPosition, &SimVelocity, &Mass, &CelestialBody)>,
    sim_time: Option<Res<SimTime>>,
) {
    if player_state.overlay_mode == DiagnosticOverlayMode::Hidden {
        return;
    }

    let is_galaxy_collision = scenario_state
        .as_deref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::GalacticCollision)
        || disk_params.outer_radius_au >= 10_000.0;

    let is_dm_overlay = player_state.overlay_mode == DiagnosticOverlayMode::DarkMatterHalos;

    if !is_galaxy_collision && !is_dm_overlay {
        return;
    }

    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);

    let halos = collect_halos(&massive_query);
    if halos.is_empty() {
        return;
    }

    for (i, halo) in halos.iter().enumerate() {
        let other = halos
            .iter()
            .enumerate()
            .find(|(j, _)| *j != i)
            .map(|(_, h)| h.0);
        draw_single_halo(&mut gizmos, halo, other, visual_time);
    }
    draw_tidal_bridge(&mut gizmos, &halos, visual_time);
}
