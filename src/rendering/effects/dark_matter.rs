//! Visualization of Galactic and Cluster Dark Matter Halos (triaxial NFW potential,
//! isothermal rotation streamlines, CDM subhalos, gravitational lensing arcs, and tidal bridges).

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::scenarios::molecular_cloud::{
    GMC_DARK_MATTER_MASS_SOLAR, GMC_NFW_SCALE_RADIUS_AU,
};
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

struct DarkMatterHalo {
    center: Vec3,
    velocity: Vec3,
    mass_solar: f32,
    scale_radius_au: f32,
    virial_radius_au: f32,
    triaxial_ratios: Vec3,
    spin_tilt: Quat,
    base_color: Color,
    is_merged: bool,
}

/// Evaluates dimensionless NFW enclosed mass profile: $f(x) = \ln(1+x) - \frac{x}{1+x}$.
#[inline]
fn nfw_enclosed_fraction(x: f32) -> f32 {
    let safe_x = x.max(1e-3);
    (1.0 + safe_x).ln() - safe_x / (1.0 + safe_x)
}

/// Collects dark matter halos from SMBH hosts (Galactic Collision / LRD / Sgr A*)
/// and the live cluster NFW dark matter halo in Giant Molecular Cloud formation.
fn collect_halos(
    scenario_state: Option<&ActiveScenarioState>,
    is_galaxy_collision: bool,
    massive_query: &Query<(&SimPosition, &SimVelocity, &Mass, &CelestialBody)>,
    origin: crate::rendering::origin::RenderOrigin,
) -> Vec<DarkMatterHalo> {
    let mut halos = Vec::with_capacity(4);
    let is_gmc =
        scenario_state.is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);

    for (pos, vel, mass, body) in massive_query.iter() {
        if body.body_type == BodyType::BlackHole && mass.0 >= 50_000.0 {
            let center = origin.to_render(pos.0);
            let velocity = Vec3::new(vel.0.x as f32, vel.0.y as f32, vel.0.z as f32);
            let is_andromeda = mass.0 > 50_000_000.0 || body.name.contains("Andromeda");
            let is_merged = body.name.contains("Merged") || body.name.contains("Milkomeda");

            let (halo_mass_solar, scale_r, triaxial_ratios, tilt, base_color) = if is_merged {
                (
                    2.4e12_f32,
                    64_000.0_f32,
                    Vec3::new(1.0, 0.78, 0.90),
                    Quat::from_euler(EulerRot::YXZ, 0.25, 0.14, 0.08),
                    Color::srgba(0.32, 0.90, 0.96, 0.72),
                )
            } else if is_andromeda {
                (
                    1.4e12_f32,
                    54_000.0_f32,
                    Vec3::new(1.0, 0.74, 0.86),
                    Quat::from_euler(EulerRot::YXZ, -0.40, 0.22, -0.10),
                    Color::srgba(0.84, 0.34, 1.0, 0.68),
                )
            } else if is_galaxy_collision || mass.0 >= 1_000_000.0 {
                let s_scale = if is_galaxy_collision {
                    46_000.0_f32
                } else {
                    ((mass.0 as f32 / 1.0e6).cbrt() * 850.0).clamp(180.0, 12_000.0)
                };
                (
                    (mass.0 as f32 * 220_000.0).clamp(1.0e8, 1.0e12),
                    s_scale,
                    Vec3::new(1.0, 0.76, 0.89),
                    Quat::from_euler(EulerRot::YXZ, 0.18, -0.10, 0.05),
                    Color::srgba(0.24, 0.76, 1.0, 0.68),
                )
            } else {
                // Cosmic Dawn SMBH seed (Little Red Dot or young nuclear halo)
                let s_scale = ((mass.0 as f32 / 100_000.0).cbrt() * 48.0).clamp(24.0, 180.0);
                (
                    (mass.0 as f32 * 4.5).clamp(1.0e5, 5.0e7),
                    s_scale,
                    Vec3::new(1.0, 0.80, 0.92),
                    Quat::IDENTITY,
                    Color::srgba(0.45, 0.65, 1.0, 0.64),
                )
            };

            halos.push(DarkMatterHalo {
                center,
                velocity,
                mass_solar: halo_mass_solar,
                scale_radius_au: scale_r,
                virial_radius_au: scale_r * 3.1,
                triaxial_ratios,
                spin_tilt: tilt,
                base_color,
                is_merged,
            });
        }
    }

    if is_gmc && halos.is_empty() {
        let mut total_m = 0.0_f64;
        let mut weighted_pos = bevy::math::DVec3::ZERO;
        let mut weighted_vel = bevy::math::DVec3::ZERO;
        for (pos, vel, mass, body) in massive_query.iter() {
            if body.body_type.is_star_or_remnant() || body.body_type == BodyType::Protostar {
                total_m += mass.0;
                weighted_pos += pos.0 * mass.0;
                weighted_vel += vel.0 * mass.0;
            }
        }
        let (center, velocity) = if total_m > 1.0 {
            (
                origin.to_render(weighted_pos / total_m),
                (weighted_vel / total_m).as_vec3(),
            )
        } else {
            (origin.to_render(bevy::math::DVec3::ZERO), Vec3::ZERO)
        };
        let rs = GMC_NFW_SCALE_RADIUS_AU as f32;
        halos.push(DarkMatterHalo {
            center,
            velocity,
            mass_solar: GMC_DARK_MATTER_MASS_SOLAR as f32,
            scale_radius_au: rs,
            virial_radius_au: rs * 3.4,
            triaxial_ratios: Vec3::new(1.0, 0.72, 0.92),
            spin_tilt: Quat::IDENTITY,
            base_color: Color::srgba(0.36, 0.68, 1.0, 0.68),
            is_merged: false,
        });
    }

    halos
}

/// Draws a smooth triaxial NFW iso-potential ellipsoid loop along one great plane.
fn draw_triaxial_loop(
    gizmos: &mut Gizmos,
    center: Vec3,
    tilt: Quat,
    axes_scale: Vec3,
    plane_rot: Quat,
    color: Color,
    segments: usize,
) {
    let mut prev: Option<Vec3> = None;
    for i in 0..=segments {
        let theta = (i as f32 / segments as f32) * TAU;
        let local = plane_rot * Vec3::new(theta.cos(), 0.0, theta.sin());
        let world_pt = center + tilt * (local * axes_scale);
        if let Some(p) = prev {
            gizmos.line(p, world_pt, color);
        }
        prev = Some(world_pt);
    }
}

/// Draws concentric triaxial NFW structural shells (Navarro cusp `0.25 r_s`, characteristic `r_s`,
/// outer virial envelope) and subtle camera-facing gravitational lensing arcs.
fn draw_halo_potential_shells(
    gizmos: &mut Gizmos,
    halo: &DarkMatterHalo,
    cam_pos: Option<Vec3>,
    zoom_alpha: f32,
    is_dm_overlay: bool,
) {
    let rs = halo.scale_radius_au;
    let rgba = halo.base_color.to_srgba();
    let (r, g, b) = (rgba.red, rgba.green, rgba.blue);
    let axes_rs = halo.triaxial_ratios * rs;

    // 1. Scale Radius (r_s): equatorial + subtle tilted meridional triaxial ribs
    let rs_alpha = if is_dm_overlay { 0.42 } else { 0.22 } * zoom_alpha;
    draw_triaxial_loop(
        gizmos,
        halo.center,
        halo.spin_tilt,
        axes_rs,
        Quat::IDENTITY,
        Color::srgba(r, g, b, rs_alpha),
        56,
    );

    if is_dm_overlay {
        let meridian_alpha = 0.16 * zoom_alpha;
        draw_triaxial_loop(
            gizmos,
            halo.center,
            halo.spin_tilt,
            axes_rs,
            Quat::from_rotation_x(FRAC_PI_2),
            Color::srgba(r, g, b, meridian_alpha),
            44,
        );
        draw_triaxial_loop(
            gizmos,
            halo.center,
            halo.spin_tilt,
            axes_rs,
            Quat::from_rotation_z(FRAC_PI_2),
            Color::srgba(r, g, b, meridian_alpha),
            44,
        );

        // 2. Inner Navarro adiabatic cusp (0.28 r_s)
        let cusp_col = Color::srgba((r + 0.22).min(1.0), g * 0.78, 1.0, 0.34 * zoom_alpha);
        draw_triaxial_loop(
            gizmos,
            halo.center,
            halo.spin_tilt,
            halo.triaxial_ratios * (rs * 0.28),
            Quat::IDENTITY,
            cusp_col,
            40,
        );
    }

    // 3. Outer Virial Envelope (R_vir ~ 3.1-3.4 r_s)
    let vir_alpha = if halo.is_merged || is_dm_overlay {
        0.16 * zoom_alpha
    } else {
        0.09 * zoom_alpha
    };
    draw_triaxial_loop(
        gizmos,
        halo.center,
        halo.spin_tilt,
        halo.triaxial_ratios * halo.virial_radius_au,
        Quat::IDENTITY,
        Color::srgba(r * 0.75, g * 0.85, (b + 0.15).min(1.0), vir_alpha),
        64,
    );

    // 4. Camera-facing weak-lensing tangential shear arcs just outside the NFW cusp
    if is_dm_overlay && let Some(cam) = cam_pos {
        let to_cam = (cam - halo.center).normalize_or_zero();
        if to_cam.length_squared() > 0.5 {
            let lens_rot = Quat::from_rotation_arc(Vec3::Y, to_cam);
            let lens_r = rs * 0.52;
            let lens_col = Color::srgba(0.68, 0.88, 1.0, 0.32 * zoom_alpha);
            for arc_idx in 0..4 {
                let start = (arc_idx as f32) * FRAC_PI_2 + 0.22;
                let arc_span = 0.52_f32;
                let segs = 10;
                let mut prev: Option<Vec3> = None;
                for s in 0..=segs {
                    let a = start + (s as f32 / segs as f32) * arc_span;
                    let pt =
                        halo.center + lens_rot * Vec3::new(a.cos() * lens_r, 0.0, a.sin() * lens_r);
                    if let Some(p) = prev {
                        gizmos.line(p, pt, lens_col);
                    }
                    prev = Some(pt);
                }
            }
        }
    }

    // 5. Halo bulk velocity vector (scaled smoothly to halo size)
    let speed = halo.velocity.length();
    if speed > 1e-3 {
        let tip = halo.center + (halo.velocity / speed) * rs * 0.65;
        gizmos.arrow(halo.center, tip, Color::srgba(r, g, b, 0.85 * zoom_alpha));
    }
}

/// Number of NFW circular-rotation streamline tracers drawn per halo.
const DM_TRACERS: usize = 220;

/// Draws NFW differential-rotation dark-matter flow streamlines and bound CDM subhalo nodes.
fn draw_nfw_streamlines_and_subhalos(
    gizmos: &mut Gizmos,
    halo: &DarkMatterHalo,
    other: Option<Vec3>,
    visual_time: f32,
    zoom_alpha: f32,
    is_dm_overlay: bool,
) {
    let rs = halo.scale_radius_au;
    let rgba = halo.base_color.to_srgba();
    let (r, g, b) = (rgba.red, rgba.green, rgba.blue);
    let r_tidal_ref = halo.virial_radius_au * 1.15;

    let overlap = other.map_or(Vec3::ZERO, |o| {
        let d = o - halo.center;
        let dist = d.length();
        if dist > rs * 0.05 && dist < r_tidal_ref {
            d / dist * (1.0 - dist / r_tidal_ref)
        } else {
            Vec3::ZERO
        }
    });

    let golden = 2.399_963_2_f32;
    let tracer_count = if is_dm_overlay { DM_TRACERS } else { 150 };

    for i in 0..tracer_count {
        let f = (i as f32 + 0.5) / tracer_count as f32;
        let y = 1.0 - 2.0 * f;
        let ring = (1.0 - y * y).max(0.0).sqrt();
        let theta = i as f32 * golden;
        let dir = Vec3::new(ring * theta.cos(), y, ring * theta.sin());

        let u = (i as f32 * 0.618_034).fract();
        let radius = rs * (0.08 + 2.65 * u * u);
        let x = (radius / rs).max(0.05);
        let m_x = nfw_enclosed_fraction(x);
        // Flat rotation curve angular rate scaling with sqrt(M(<r) / r^3) * (mass normalized)
        let omega = 0.52 * (m_x / x).sqrt() / x;
        let sign = if i % 5 == 0 { -1.0 } else { 1.0 };

        let rot_now = Quat::from_rotation_y(sign * omega * visual_time);
        let rot_prev = Quat::from_rotation_y(sign * omega * (visual_time - 0.30));

        let local_now = halo.spin_tilt * ((rot_now * dir) * halo.triaxial_ratios * radius);
        let local_prev = halo.spin_tilt * ((rot_prev * dir) * halo.triaxial_ratios * radius);
        let tide = overlap * (radius / halo.virial_radius_au) * rs * 0.72;

        let alpha = ((0.68 - 0.42 * u) * zoom_alpha).clamp(0.10, 0.72);
        gizmos.line(
            halo.center + local_prev + tide,
            halo.center + local_now + tide,
            Color::srgba(r, g, b, alpha),
        );
    }

    // Orbiting CDM subhalos inside the host virial volume (hierarchically accreted clumps)
    if is_dm_overlay {
        let subhalo_seeds: [(f32, f32, f32, f32); 5] = [
            (0.65, 0.25, 0.4, 0.08),
            (1.15, -0.32, 1.9, 0.065),
            (1.75, 0.18, 3.6, 0.075),
            (2.25, -0.45, 4.9, 0.06),
            (2.70, 0.30, 5.7, 0.055),
        ];
        for &(x_r, inc_y, phi_0, sub_scale) in &subhalo_seeds {
            let orbit_r = rs * x_r;
            let omega = 0.42 * (nfw_enclosed_fraction(x_r) / x_r).sqrt() / x_r;
            let phi = phi_0 + omega * visual_time;
            let raw_offset = Vec3::new(phi.cos(), inc_y, phi.sin()).normalize() * orbit_r;
            let sub_pos = halo.center + halo.spin_tilt * (raw_offset * halo.triaxial_ratios);
            let sub_r = rs * sub_scale;
            let sub_col = Color::srgba(
                (r + 0.15).min(1.0),
                (g + 0.1).min(1.0),
                1.0,
                0.52 * zoom_alpha,
            );
            gizmos.circle(
                Isometry3d::new(sub_pos, Quat::from_rotation_x(FRAC_PI_2)),
                sub_r,
                sub_col,
            );
            gizmos.circle(
                Isometry3d::new(sub_pos, Quat::IDENTITY),
                sub_r * 0.65,
                Color::srgba(r, g, b, 0.32 * zoom_alpha),
            );
        }
    }
}

/// Draws curved multi-strand dark-matter tidal bridge, mass-ratio weighted L1 saddle, and shock wake.
fn draw_tidal_bridge_and_wake(
    gizmos: &mut Gizmos,
    halos: &[DarkMatterHalo],
    visual_time: f32,
    zoom_alpha: f32,
) {
    let (Some(h1), Some(h2)) = (halos.first(), halos.get(1)) else {
        return;
    };
    let delta = h2.center - h1.center;
    let dist = delta.length();
    let r_overlap = (h1.virial_radius_au + h2.virial_radius_au) * 0.62;

    if dist < r_overlap && dist > h1.scale_radius_au * 0.01 {
        let overlap_frac = (1.0 - dist / r_overlap).clamp(0.0, 1.0);
        let dir = delta / dist;
        let bridge_alpha = (overlap_frac * 0.65 * zoom_alpha).clamp(0.12, 0.72);

        // Relativistic/Coriolis transverse deflection for curved tidal filament strands
        let rel_vel = h2.velocity - h1.velocity;
        let perp = (rel_vel - dir * rel_vel.dot(dir)).normalize_or_zero();
        let arch = if perp.length_squared() > 0.1 {
            perp
        } else {
            dir.any_orthogonal_vector()
        };

        for strand_idx in -2_i32..=2 {
            let offset_factor = strand_idx as f32 * 0.09;
            let ctrl_pt = h1.center.midpoint(h2.center)
                + arch * (dist * (0.06 + offset_factor.abs()) * offset_factor.signum());
            let strand_alpha = bridge_alpha * (1.0 - (strand_idx.abs() as f32) * 0.24);
            let col = Color::srgba(0.54, 0.58, 1.0, strand_alpha);
            let mut prev = h1.center;
            for step in 1..=20 {
                let t = step as f32 / 20.0;
                let omt = 1.0 - t;
                let pt = h1.center * (omt * omt) + ctrl_pt * (2.0 * omt * t) + h2.center * (t * t);
                gizmos.line(prev, pt, col);
                prev = pt;
            }
        }

        // Mass-weighted L1 gravitational equipotential saddle point: frac = 1 / (1 + sqrt(M2 / M1))
        let mass_ratio = (h2.mass_solar / h1.mass_solar.max(1.0)).max(1e-3);
        let l1_frac = 1.0 / (1.0 + mass_ratio.sqrt());
        let saddle_pos = h1.center + delta * l1_frac;
        let saddle_r = h1.scale_radius_au * (0.16 + (1.0 - overlap_frac) * 0.22);
        let saddle_col = Color::srgba(1.0, 0.88, 0.34, bridge_alpha * 0.85);
        gizmos.circle(
            Isometry3d::new(saddle_pos, Quat::from_rotation_x(FRAC_PI_2)),
            saddle_r,
            saddle_col,
        );

        // Chandrasekhar dynamical friction deceleration wake ring
        let mid_pt = h1.center.midpoint(h2.center);
        let shock_r = (dist * 0.34).min(h1.scale_radius_au);
        let shock_pulse = (visual_time * 3.0).sin() * 0.1 + 0.9;
        let shock_col = Color::srgba(
            1.0,
            0.48,
            0.22,
            overlap_frac * 0.48 * shock_pulse * zoom_alpha,
        );
        let align_rot = Quat::from_rotation_arc(Vec3::Z, dir);
        gizmos.circle(Isometry3d::new(mid_pt, align_rot), shock_r, shock_col);
    }
}

/// Renders triaxial NFW Dark Matter Halo shells, rotation streamlines, CDM subhalos,
/// weak-lensing arcs, and multi-strand tidal bridges across galactic and GMC cluster halos.
pub fn draw_dark_matter_halos(
    mut gizmos: Gizmos,
    scenario_state: Option<Res<ActiveScenarioState>>,
    disk_params: Res<DiskParameters>,
    player_state: Res<PlayerInteractionState>,
    massive_query: Query<(&SimPosition, &SimVelocity, &Mass, &CelestialBody)>,
    cam_query: Query<
        (
            &Transform,
            Option<&crate::rendering::camera::PanOrbitCamera>,
        ),
        With<Camera3d>,
    >,
    sim_time: Option<Res<SimTime>>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let cam_entry = cam_query.iter().next();
    if player_state.overlay_mode == DiagnosticOverlayMode::Hidden
        || cam_entry.is_some_and(|(_, c)| c.is_some_and(|cam| cam.surface_horizon_mode))
    {
        return;
    }

    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    let is_galaxy_collision = scenario_state
        .as_deref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::GalacticCollision)
        || disk_params.outer_radius_au >= 10_000.0;

    let is_dm_overlay = player_state.overlay_mode == DiagnosticOverlayMode::DarkMatterHalos;

    if !is_galaxy_collision && !is_dm_overlay {
        return;
    }

    let halos = collect_halos(
        scenario_state.as_deref(),
        is_galaxy_collision,
        &massive_query,
        origin,
    );
    if halos.is_empty() {
        return;
    }

    let cam_pos = cam_entry.map(|(t, _)| t.translation);
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);

    for (i, halo) in halos.iter().enumerate() {
        // Gentle zoom attenuation so zooming inside a single planet/star never obscures local detail
        let zoom_alpha = cam_pos.map_or(1.0_f32, |cam| {
            let d = (cam - halo.center).length();
            (d / (halo.scale_radius_au * 0.22).max(1.0)).clamp(0.20, 1.0)
        });
        let other = halos
            .iter()
            .enumerate()
            .find(|(j, _)| *j != i)
            .map(|(_, h)| h.center);

        draw_halo_potential_shells(&mut gizmos, halo, cam_pos, zoom_alpha, is_dm_overlay);
        draw_nfw_streamlines_and_subhalos(
            &mut gizmos,
            halo,
            other,
            visual_time,
            zoom_alpha,
            is_dm_overlay,
        );
    }

    draw_tidal_bridge_and_wake(&mut gizmos, &halos, visual_time, 1.0);
}
