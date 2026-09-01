//! Visual effects, orbit path gizmos, impact shockwaves, and diagnostic overlays.

use bevy::prelude::*;

use crate::simulation::accretion::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::math::*;

/// Collects collision merger and engulfment events and spawns expanding physical shockwaves.
pub fn update_impact_shockwaves(
    time: Res<Time>,
    mut shockwave_pool: ResMut<ImpactShockwavePool>,
    mut merge_reader: MessageReader<AccretionMergeEvent>,
    mut bounce_reader: MessageReader<CollisionBounceEvent>,
    mut engulf_reader: MessageReader<PlanetaryEngulfmentEvent>,
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

    // 3. Process planetary engulfment incandescence flares
    for ev in engulf_reader.read() {
        let max_r = (1.5f32 + (ev.planet_mass_earth).cbrt() as f32 * 0.8f32).clamp(1.5f32, 5.0f32);
        shockwave_pool.shockwaves.push(ImpactShockwave {
            position: Vec3::ZERO,
            radius: 0.2,
            max_radius: max_r,
            timer: 0.0,
            max_timer: 3.5,
            color: Color::srgba(1.0, 0.2, 0.1, 0.95), // Incandescent Red-Giant plasma flare
        });
    }

    // 4. Update existing shockwaves
    shockwave_pool.shockwaves.retain_mut(|sw| {
        sw.timer += dt;
        let progress = sw.timer / sw.max_timer;
        sw.radius = sw.max_radius * progress.sqrt();
        sw.timer < sw.max_timer
    });
}

/// Draws dynamic Keplerian orbit trails, selection brackets, shockwaves, planetary nebulae, and diagnostic overlays.
pub fn draw_orbital_effects_and_gizmos(
    mut gizmos: Gizmos,
    player_state: Res<PlayerInteractionState>,
    shockwave_pool: Res<ImpactShockwavePool>,
    time: Res<Time>,
    star_query: Query<
        (
            &SimPosition,
            &Mass,
            &IgnitionState,
            &CelestialBody,
            Option<&StellarEvolutionState>,
            Option<&ElectromagneticFieldState>,
        ),
        With<CentralStar>,
    >,
    bodies_query: Query<(
        Entity,
        &SimPosition,
        &SimVelocity,
        &Mass,
        &Composition,
        &CelestialBody,
        Option<&InternalDifferentiation>,
        Option<&SpinState>,
    )>,
) {
    let Ok((star_pos, star_mass, ignition, star_body, opt_evo, _opt_em)) = star_query.single()
    else {
        return;
    };

    let star_vec = Vec3::new(star_pos.x as f32, star_pos.y as f32, star_pos.z as f32);
    let elapsed = time.elapsed_secs();

    // 1. Draw Expanding Ionized Planetary Nebula / Supernova Blast
    if let Some(evo) = opt_evo {
        if evo.nebula_expansion_radius_au > 0.0 && evo.nebula_opacity > 0.01 {
            let r_neb = evo.nebula_expansion_radius_au;
            let op = evo.nebula_opacity;

            if matches!(evo.phase, StellarEvolutionPhase::SupernovaExplosion) {
                // Violent Supernova Core-Collapse Blast (Intense White/Cyan & Gold)
                gizmos.sphere(
                    Isometry3d::from_translation(star_vec),
                    r_neb,
                    Color::srgba(0.95, 0.95, 1.0, 0.25 * op),
                );
                gizmos.circle(
                    Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    r_neb,
                    Color::srgba(0.3, 0.8, 1.0, 0.85 * op),
                );
                gizmos.circle(
                    Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    r_neb * 0.9,
                    Color::srgba(1.0, 0.6, 0.2, 0.70 * op),
                );
            } else {
                // Outer Ruby H-Alpha Shell (656.3 nm)
                gizmos.circle(
                    Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    r_neb,
                    Color::srgba(0.95, 0.22, 0.38, 0.65 * op),
                );
                gizmos.circle(
                    Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    r_neb,
                    Color::srgba(0.95, 0.22, 0.38, 0.40 * op),
                );

                // Inner Emerald [O III] Shell (500.7 nm)
                if r_neb > 0.5 {
                    gizmos.circle(
                        Isometry3d::new(
                            star_vec,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        r_neb * 0.78,
                        Color::srgba(0.12, 0.95, 0.68, 0.75 * op),
                    );
                    gizmos.sphere(
                        Isometry3d::from_translation(star_vec),
                        r_neb * 0.78,
                        Color::srgba(0.12, 0.95, 0.68, 0.06 * op),
                    );
                }

                // Outer Diffuse Gas Shroud
                gizmos.sphere(
                    Isometry3d::from_translation(star_vec),
                    r_neb,
                    Color::srgba(0.75, 0.20, 0.50, 0.04 * op),
                );
            }
        }
    }

    // 1B. Draw Extreme Electromagnetism & Relativistic Jets for Stellar Remnants
    if star_body.body_type == BodyType::WhiteDwarf {
        // White Dwarf: Magnetic Dipole Field Lines & Diamond Corona
        let r_mag = 0.25f32;
        let pulse = (elapsed * 2.0).sin() * 0.05;
        for i in 0..6 {
            let angle = (i as f32) * (std::f32::consts::PI / 3.0);
            let rot = Quat::from_rotation_y(angle);
            gizmos.circle(
                Isometry3d::new(star_vec, rot),
                r_mag + pulse,
                Color::srgba(0.35, 0.80, 1.0, 0.45),
            );
        }
        gizmos.sphere(
            Isometry3d::from_translation(star_vec),
            0.08,
            Color::srgba(0.75, 0.90, 1.0, 0.20),
        );
    } else if matches!(
        star_body.body_type,
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar
    ) {
        // Pulsar / Magnetar: Rapidly spinning relativistic synchrotron beam lighthouse jets!
        let spin_rate = if star_body.body_type == BodyType::Magnetar {
            1.5
        } else {
            12.0
        };
        let beam_rot = Quat::from_rotation_y(elapsed * spin_rate) * Quat::from_rotation_x(0.35); // 20-degree magnetic axis tilt
        let jet_len = if star_body.body_type == BodyType::Magnetar {
            4.5
        } else {
            3.0
        };

        let north_beam = beam_rot * Vec3::Y;
        let south_beam = -north_beam;

        let beam_color = if star_body.body_type == BodyType::Magnetar {
            Color::srgba(1.0, 0.25, 0.55, 0.85) // Vivid magenta for Magnetar
        } else {
            Color::srgba(0.20, 0.85, 1.0, 0.85) // Electric cyan for Pulsar
        };

        // Polar relativistic beams
        gizmos.line(star_vec, star_vec + north_beam * jet_len, beam_color);
        gizmos.line(star_vec, star_vec + south_beam * jet_len, beam_color);

        // Synchrotron beam emission cones
        gizmos.circle(
            Isometry3d::new(
                star_vec + north_beam * jet_len,
                Quat::from_rotation_arc(Vec3::Z, north_beam),
            ),
            0.45,
            beam_color,
        );
        gizmos.circle(
            Isometry3d::new(
                star_vec + south_beam * jet_len,
                Quat::from_rotation_arc(Vec3::Z, south_beam),
            ),
            0.45,
            beam_color,
        );

        // Magnetospheric Dipole Field Loops
        for i in 0..4 {
            let angle = (i as f32) * (std::f32::consts::PI / 2.0);
            let rot = beam_rot * Quat::from_rotation_y(angle);
            gizmos.circle(
                Isometry3d::new(star_vec, rot),
                0.65,
                Color::srgba(0.5, 0.3, 1.0, 0.40),
            );
        }
    } else if star_body.body_type == BodyType::BlackHole {
        // Black Hole: Relativistic Accretion Disk & Polar Jets
        let disk_inner = 0.08f32;
        let disk_outer = 0.55f32;
        let disk_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);

        // Accretion Disk Multi-Rings (Glowing Orange / Gold Doppler boost)
        gizmos.circle(
            Isometry3d::new(star_vec, disk_rot),
            disk_inner,
            Color::srgba(1.0, 0.9, 0.4, 0.90),
        );
        gizmos.circle(
            Isometry3d::new(star_vec, disk_rot),
            disk_inner * 2.0,
            Color::srgba(1.0, 0.55, 0.15, 0.70),
        );
        gizmos.circle(
            Isometry3d::new(star_vec, disk_rot),
            disk_outer,
            Color::srgba(0.85, 0.25, 0.05, 0.45),
        );

        // Collimated Relativistic Polar Jets
        let jet_len = 5.0f32;
        let jet_color = Color::srgba(0.4, 0.85, 1.0, 0.75);
        gizmos.line(star_vec, star_vec + Vec3::Y * jet_len, jet_color);
        gizmos.line(star_vec, star_vec - Vec3::Y * jet_len, jet_color);
        gizmos.circle(
            Isometry3d::new(
                star_vec + Vec3::Y * jet_len,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            0.35,
            jet_color,
        );
        gizmos.circle(
            Isometry3d::new(
                star_vec - Vec3::Y * jet_len,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            0.35,
            jet_color,
        );
    }

    // 1B. Draw Ignited Star Shockwave & Solar Wind Wavefront
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
    for (entity, pos, vel, mass, comp, body, opt_diff, opt_spin) in bodies_query.iter() {
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
            let tail_len = (8.0 / r_orbit.powi(2)).clamp(0.2, 3.5) * (comp.ice_frac as f32);

            // Brilliant blue ion tail
            gizmos.line(
                body_vec,
                body_vec + tail_dir * tail_len,
                Color::srgba(0.3, 0.7, 1.0, 0.65),
            );
            // Diffuse curved dust tail
            let dust_dir =
                (tail_dir - vel.0.normalize_or_zero().as_vec3() * 0.3).normalize_or_zero();
            gizmos.line(
                body_vec,
                body_vec + dust_dir * (tail_len * 0.7),
                Color::srgba(0.9, 0.85, 0.6, 0.45),
            );
        }

        // 3D Magnetic Dipole Flux Loops & Sunward Bow Shock
        if let Some(diff) = opt_diff {
            if diff.magnetic_field_gauss >= 0.15 {
                let tilt_deg = opt_spin
                    .map(|s| s.axial_tilt_degrees as f32)
                    .unwrap_or(23.5);
                let tilt_rot = Quat::from_rotation_z(tilt_deg.to_radians());
                let b_strength = (diff.magnetic_field_gauss as f32).min(3.0);
                let shield_r = 0.35 + b_strength * 0.25;

                // 4 Dipole field lines at 90-degree azimuthal intervals
                for quad in 0..4 {
                    let quad_rot = Quat::from_rotation_y(quad as f32 * std::f32::consts::FRAC_PI_2);
                    let mut prev_pt: Option<Vec3> = None;
                    let steps = 18;
                    for step in 0..=steps {
                        let theta = (step as f32 / steps as f32) * std::f32::consts::PI;
                        let sin_th = theta.sin();
                        let r = shield_r * sin_th.powi(2);
                        let local_x = r * sin_th;
                        let local_y = r * theta.cos();
                        let local_pt = quad_rot * Vec3::new(local_x, local_y, 0.0);
                        let world_pt = body_vec + tilt_rot * local_pt;

                        if let Some(prev) = prev_pt {
                            gizmos.line(
                                prev,
                                world_pt,
                                Color::srgba(0.25, 0.75, 1.0, 0.35 * (b_strength / 3.0)),
                            );
                        }
                        prev_pt = Some(world_pt);
                    }
                }

                // Sunward Deflection Bow Shock Arc
                let to_sun = (star_vec - body_vec).normalize_or_zero();
                let perp_tangent = Vec3::new(-to_sun.z, 0.0, to_sun.x).normalize_or_zero();
                let bow_apex = body_vec + to_sun * (shield_r * 1.25);
                let bow_left =
                    body_vec + to_sun * (shield_r * 0.75) + perp_tangent * (shield_r * 1.1);
                let bow_right =
                    body_vec + to_sun * (shield_r * 0.75) - perp_tangent * (shield_r * 1.1);

                gizmos.line(bow_left, bow_apex, Color::srgba(1.0, 0.85, 0.3, 0.55));
                gizmos.line(bow_apex, bow_right, Color::srgba(1.0, 0.85, 0.3, 0.55));
            }
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
            let is_star = body.body_type.is_star_or_remnant();

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
