//! Visual effects, orbit path gizmos, impact shockwaves, and diagnostic overlays.

use bevy::prelude::*;

use crate::simulation::accretion::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::math::*;

/// Collects collision merger events and spawns expanding physical shockwaves.
pub fn update_impact_shockwaves(
    time: Res<Time>,
    mut shockwave_pool: ResMut<ImpactShockwavePool>,
    mut merge_reader: MessageReader<AccretionMergeEvent>,
    mut bounce_reader: MessageReader<CollisionBounceEvent>,
    bodies_query: Query<&SimPosition>,
) {
    let dt = time.delta_secs();

    // 1. Process merger shockwaves
    for ev in merge_reader.read() {
        let pos = Vec3::new(
            ev.merged_position.x as f32,
            ev.merged_position.y as f32,
            ev.merged_position.z as f32,
        );
        let max_r = (0.4 + (ev.merged_mass * 100.0).cbrt() as f32 * 0.35).clamp(0.5, 3.0);
        let color = if ev.new_body_type == BodyType::GasGiant {
            Color::srgba(1.0, 0.6, 0.1, 0.9)
        } else {
            Color::srgba(1.0, 0.85, 0.3, 0.9)
        };

        shockwave_pool.shockwaves.push(ImpactShockwave {
            position: pos,
            radius: 0.05,
            max_radius: max_r,
            timer: 0.0,
            max_timer: 1.8,
            color,
        });
    }

    // 2. Process grazing bounce ripples
    for ev in bounce_reader.read() {
        if let Ok(pos1) = bodies_query.get(ev.entity1) {
            let pos = Vec3::new(pos1.x as f32, pos1.y as f32, pos1.z as f32);
            shockwave_pool.shockwaves.push(ImpactShockwave {
                position: pos,
                radius: 0.05,
                max_radius: 0.6,
                timer: 0.0,
                max_timer: 1.2,
                color: Color::srgba(0.4, 0.8, 1.0, 0.7),
            });
        }
    }

    // 3. Update existing shockwaves
    shockwave_pool.shockwaves.retain_mut(|sw| {
        sw.timer += dt;
        let progress = sw.timer / sw.max_timer;
        sw.radius = sw.max_radius * progress.sqrt();
        sw.timer < sw.max_timer
    });
}

/// Draws dynamic Keplerian orbit trails, selection brackets, shockwaves, and diagnostic overlays.
pub fn draw_orbital_effects_and_gizmos(
    mut gizmos: Gizmos,
    player_state: Res<PlayerInteractionState>,
    shockwave_pool: Res<ImpactShockwavePool>,
    star_query: Query<(&SimPosition, &Mass, &IgnitionState), With<CentralStar>>,
    bodies_query: Query<(
        Entity,
        &SimPosition,
        &SimVelocity,
        &Mass,
        &Composition,
        &CelestialBody,
    )>,
) {
    let Ok((star_pos, star_mass, ignition)) = star_query.single() else {
        return;
    };

    let star_vec = Vec3::new(star_pos.x as f32, star_pos.y as f32, star_pos.z as f32);

    // 1. Draw Ignited Star Shockwave & Solar Wind Wavefront
    if ignition.is_ignited && ignition.shockwave_radius > 0.0 {
        let r = ignition.shockwave_radius as f32;
        let fade = (1.0 - (r / 35.0)).clamp(0.1, 1.0);

        // Leading Radiation Wavefront (Bright White-Gold)
        gizmos.circle(
            Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r,
            Color::srgba(1.0, 0.95, 0.6, 0.85 * fade),
        );

        // Secondary Compression Wavefront
        if r > 0.3 {
            gizmos.circle(
                Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                r * 0.96,
                Color::srgba(1.0, 0.65, 0.15, 0.5 * fade),
            );
            gizmos.circle(
                Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                r * 0.92,
                Color::srgba(0.9, 0.3, 0.1, 0.3 * fade),
            );
        }

        // Spherical Shell Dome
        gizmos.sphere(
            Isometry3d::from_translation(star_vec),
            r,
            Color::srgba(1.0, 0.8, 0.3, 0.08 * fade),
        );

        // Radiant Solar Flare Beams (Radial Spikes from the Star)
        if r < 15.0 {
            let n_rays = 12;
            for i in 0..n_rays {
                let angle = (i as f32) * (2.0 * std::f32::consts::PI / n_rays as f32);
                let ray_len = (r * 1.15).min(18.0);
                let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
                gizmos.line(
                    star_vec + dir * 1.2,
                    star_vec + dir * ray_len,
                    Color::srgba(1.0, 0.85, 0.4, 0.45 * fade),
                );
            }
        }
    }

    // 2. Draw Impact Shockwaves
    for sw in shockwave_pool.shockwaves.iter() {
        let alpha = (1.0 - (sw.timer / sw.max_timer)).clamp(0.0, 1.0);
        let sw_color = Color::srgba(
            sw.color.to_srgba().red,
            sw.color.to_srgba().green,
            sw.color.to_srgba().blue,
            alpha * 0.8,
        );

        gizmos.circle(
            Isometry3d::new(
                sw.position,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            sw.radius,
            sw_color,
        );
        gizmos.sphere(
            Isometry3d::from_translation(sw.position),
            sw.radius * 0.4,
            Color::srgba(
                sw.color.to_srgba().red,
                sw.color.to_srgba().green,
                sw.color.to_srgba().blue,
                alpha * 0.3,
            ),
        );
    }

    // 2B. Draw Subtle Astronomical Unit (AU) Spatial Guide Rings around the Sun
    let au_rings = [
        (1.0f32, Color::srgba(0.3, 0.7, 1.0, 0.15)), // 1 AU (Earth Orbit)
        (5.2f32, Color::srgba(1.0, 0.7, 0.3, 0.12)), // 5.2 AU (Jupiter Orbit)
        (9.58f32, Color::srgba(0.9, 0.8, 0.4, 0.10)), // 9.58 AU (Saturn Orbit)
        (19.2f32, Color::srgba(0.4, 0.8, 0.9, 0.08)), // 19.2 AU (Uranus Orbit)
        (30.0f32, Color::srgba(0.3, 0.5, 0.9, 0.06)), // 30.0 AU (Neptune Orbit)
        (39.5f32, Color::srgba(0.5, 0.4, 0.8, 0.05)), // 39.5 AU (Kuiper Belt)
    ];
    let ring_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    for (r_au, col) in au_rings {
        gizmos.circle(Isometry3d::new(star_vec, ring_rot), r_au, col);
    }

    // 3. Draw Orbit Trails & Diagnostic Overlays for Bodies
    for (entity, pos, vel, mass, comp, body) in bodies_query.iter() {
        let is_selected = player_state.selected_entity == Some(entity);
        let is_planet = matches!(
            body.body_type,
            BodyType::Protoplanet
                | BodyType::TerrestrialPlanet
                | BodyType::GasGiant
                | BodyType::IceGiant
        );

        let body_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let r_orbit = pos.0.length() as f32;

        // Cometary Ion and Dust Tails (streaming away from the star for volatile icy bodies)
        if comp.ice_frac > 0.35 && r_orbit < 6.0 && r_orbit > 0.15 {
            let tail_dir = (body_vec - star_vec).normalize_or_zero();
            let tail_len = ((6.0 - r_orbit) * 0.65).clamp(0.2, 3.5);
            let vel_vec = Vec3::new(vel.x as f32, vel.y as f32, vel.z as f32);
            let vel_dir = -vel_vec.normalize_or_zero();

            // Ion Gas Tail (Straight radial line away from the star)
            gizmos.line(
                body_vec,
                body_vec + tail_dir * tail_len,
                Color::srgba(0.35, 0.85, 1.0, 0.85),
            );
            // Dust Tail (Curved along orbital velocity)
            gizmos.line(
                body_vec,
                body_vec
                    + (tail_dir * 0.75 + vel_dir * 0.25).normalize_or_zero() * (tail_len * 0.8),
                Color::srgba(1.0, 0.92, 0.70, 0.50),
            );
        }

        // A. Orbit Trails
        if is_selected || is_planet {
            let rel_pos = pos.0 - star_pos.0;
            let rel_vel = vel.0;

            if let Some(elements) =
                state_vectors_to_orbital_elements(rel_pos, rel_vel, star_mass.0, mass.0)
            {
                if elements.semi_major_axis > 0.0 && elements.eccentricity < 1.0 {
                    let orbit_points = generate_orbit_points(&elements, 96);
                    if orbit_points.len() > 1 {
                        let orbit_color = if is_selected {
                            Color::srgba(0.2, 0.9, 1.0, 0.8)
                        } else {
                            match body.body_type {
                                BodyType::GasGiant => Color::srgba(0.9, 0.6, 0.2, 0.5),
                                BodyType::IceGiant => Color::srgba(0.3, 0.8, 0.9, 0.5),
                                BodyType::TerrestrialPlanet => Color::srgba(0.4, 0.9, 0.4, 0.5),
                                _ => Color::srgba(0.6, 0.6, 0.6, 0.3),
                            }
                        };

                        for window in orbit_points.windows(2) {
                            gizmos.line(window[0] + star_vec, window[1] + star_vec, orbit_color);
                        }
                    }
                }
            }
        }

        // B. Diagnostic Overlays
        match player_state.overlay_mode {
            DiagnosticOverlayMode::SpectralComposition => {
                let dominant_color = if comp.metal_frac > 0.4 {
                    Color::srgba(1.0, 0.8, 0.2, 0.6) // Metal: Gold
                } else if comp.ice_frac > 0.4 {
                    Color::srgba(0.2, 0.85, 1.0, 0.6) // Ice: Cyan
                } else if comp.gas_frac > 0.4 {
                    Color::srgba(0.9, 0.5, 0.1, 0.6) // Gas: Orange
                } else {
                    Color::srgba(0.8, 0.4, 0.2, 0.6) // Rock: Ochre
                };

                gizmos.circle(
                    Isometry3d::new(body_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    0.25,
                    dominant_color,
                );
            }
            DiagnosticOverlayMode::HillSpheresAndGaps => {
                if is_planet {
                    let hill_r =
                        (r_orbit * ((mass.0 / (3.0 * star_mass.0)).cbrt() as f32)).clamp(0.08, 2.5);

                    // Draw Hill Sphere sphere
                    gizmos.sphere(
                        Isometry3d::from_translation(body_vec),
                        hill_r,
                        Color::srgba(0.2, 0.9, 0.4, 0.25),
                    );

                    // Draw Annular Gap Clearance boundaries around the star
                    gizmos.circle(
                        Isometry3d::new(
                            star_vec,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        (r_orbit - hill_r).max(0.1),
                        Color::srgba(0.4, 0.8, 0.9, 0.3),
                    );
                    gizmos.circle(
                        Isometry3d::new(
                            star_vec,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        r_orbit + hill_r,
                        Color::srgba(0.4, 0.8, 0.9, 0.3),
                    );
                }
            }
            DiagnosticOverlayMode::Realistic => {}
        }

        // C. Draw Selection Highlight Reticle & Beacon (Star & Planets)
        if is_selected {
            let is_star = matches!(
                body.body_type,
                BodyType::Protostar | BodyType::MainSequenceStar
            );

            let reticle_r = if is_star {
                1.6
            } else {
                match body.body_type {
                    BodyType::GasGiant => 0.75,
                    BodyType::IceGiant => 0.55,
                    _ => 0.40,
                }
            };

            let reticle_color = if is_star {
                Color::srgb(1.0, 0.85, 0.2) // Bright golden reticle for the Star
            } else {
                Color::srgb(0.2, 0.95, 1.0) // Vivid cyan reticle for Planets
            };

            // Primary & Secondary Concentric Target Rings
            gizmos.circle(
                Isometry3d::new(body_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                reticle_r,
                reticle_color,
            );
            gizmos.circle(
                Isometry3d::new(body_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                reticle_r * 1.35,
                Color::srgba(
                    reticle_color.to_srgba().red,
                    reticle_color.to_srgba().green,
                    reticle_color.to_srgba().blue,
                    0.4,
                ),
            );

            // Crosshair Target Ticks
            let tick_len = reticle_r * 0.4;
            gizmos.line(
                body_vec + Vec3::new(reticle_r, 0.0, 0.0),
                body_vec + Vec3::new(reticle_r + tick_len, 0.0, 0.0),
                reticle_color,
            );
            gizmos.line(
                body_vec - Vec3::new(reticle_r, 0.0, 0.0),
                body_vec - Vec3::new(reticle_r + tick_len, 0.0, 0.0),
                reticle_color,
            );
            gizmos.line(
                body_vec + Vec3::new(0.0, 0.0, reticle_r),
                body_vec + Vec3::new(0.0, 0.0, reticle_r + tick_len),
                reticle_color,
            );
            gizmos.line(
                body_vec - Vec3::new(0.0, 0.0, reticle_r),
                body_vec - Vec3::new(0.0, 0.0, reticle_r + tick_len),
                reticle_color,
            );

            // 3D Vertical Beacon Marker & Top Pointer Sphere
            let beacon_h = if is_star { 2.5 } else { 1.4 };
            gizmos.line(
                body_vec,
                body_vec + Vec3::new(0.0, beacon_h, 0.0),
                reticle_color,
            );
            gizmos.sphere(
                Isometry3d::from_translation(body_vec + Vec3::new(0.0, beacon_h, 0.0)),
                if is_star { 0.15 } else { 0.08 },
                reticle_color,
            );
        }
    }

    // 4. Draw Gravitational Tractor Gizmo
    if player_state.active_tool == PlayerTool::GravitationalTractor {
        if let Some(t_pos) = player_state.tractor_position {
            let t_vec = Vec3::new(t_pos.x as f32, t_pos.y as f32, t_pos.z as f32);
            gizmos.sphere(
                Isometry3d::from_translation(t_vec),
                0.5,
                Color::srgba(0.9, 0.2, 0.8, 0.7),
            );
        }
    }
}
