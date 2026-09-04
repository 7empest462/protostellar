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

/// Collects Roche tidal disruption events and simulates expanding Keplerian debris streamer fragments.
pub fn update_roche_debris_streams(
    time: Res<Time>,
    mut debris_pool: ResMut<RocheDebrisPool>,
    mut roche_reader: MessageReader<RocheDisruptionEvent>,
    bodies_query: Query<&SimPosition>,
) {
    let dt = time.delta_secs();

    // 1. Ingest new Roche disruption events
    for ev in roche_reader.read() {
        let n_fragments = 48;
        let mut fragments = Vec::with_capacity(n_fragments);
        let base_angle =
            (ev.disruption_pos.z - ev.primary_pos.z).atan2(ev.disruption_pos.x - ev.primary_pos.x);
        let r_disrupt = ((ev.disruption_pos.x - ev.primary_pos.x)
            .hypot(ev.disruption_pos.z - ev.primary_pos.z))
        .max(0.02);

        let inner_r = (r_disrupt * 0.55).max(0.015);
        let outer_r = (r_disrupt * 1.35).max(inner_r + 0.01);

        for k in 0..n_fragments {
            let frac = (k as f32) / (n_fragments as f32);
            // Fragment radius spread between inner and outer ring boundary
            let frag_r = inner_r + (outer_r - inner_r) * frac;
            // Phase offset along spiral arc
            let phase = base_angle + (frac * std::f32::consts::TAU * 0.85);
            // Keplerian orbital angular velocity omega ~ sqrt(1 / r^3)
            let omega = (1.8 / (frag_r * frag_r * frag_r).sqrt()).clamp(0.4, 8.0);
            // Slight initial vertical oscillation that settles toward ring plane
            let z_off = ((k as f32 * 1.7).sin() * 0.015) * (1.0 - frac * 0.5);

            fragments.push((frag_r, phase, omega, z_off));
        }

        debris_pool.streams.push(RocheDebrisStream {
            primary_entity: ev.primary_entity,
            primary_pos: ev.primary_pos,
            disruption_pos: ev.disruption_pos,
            inner_radius: inner_r,
            outer_radius: outer_r,
            timer: 0.0,
            max_timer: 4.5, // 4.5 seconds of active dynamic shredding and settling
            ice_fraction: ev.ice_fraction,
            debris_mass_earth: ev.ring_mass_earth,
            fragments,
        });
    }

    // 2. Advance active streams
    debris_pool.streams.retain_mut(|stream| {
        stream.timer += dt;

        // Follow primary position if entity is still alive
        if let Ok(pos) = bodies_query.get(stream.primary_entity) {
            stream.primary_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        }

        // Circularize and advance fragments
        for frag in stream.fragments.iter_mut() {
            frag.1 += frag.2 * dt; // advance phase
            frag.3 *= 1.0 - (0.75 * dt).min(0.9); // dampen vertical thickness towards plane
        }

        stream.timer < stream.max_timer
    });
}

/// Draws dynamic Keplerian orbit trails, selection brackets, shockwaves, planetary nebulae, and diagnostic overlays.
pub fn draw_orbital_effects_and_gizmos(
    mut gizmos: Gizmos,
    config: Res<SimulationConfig>,
    player_state: Res<PlayerInteractionState>,
    shockwave_pool: Res<ImpactShockwavePool>,
    debris_pool: Res<RocheDebrisPool>,
    time: Res<Time>,
    star_query: Query<
        (
            &SimPosition,
            &Mass,
            &Radius,
            &IgnitionState,
            &CelestialBody,
            Option<&StellarEvolutionState>,
            Option<&ElectromagneticFieldState>,
            Option<&BlackHoleStarState>,
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
        Option<&Radius>,
        Option<&AtmosphericEscapeTail>,
    )>,
) {
    let Ok((star_pos, star_mass, star_radius, ignition, star_body, opt_evo, _opt_em, opt_quasi)) =
        star_query.single()
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
    } else if opt_quasi.is_some()
        || star_body.body_type == BodyType::QuasiStar
        || star_body.name.contains("Quasar")
        || (star_body.body_type == BodyType::BlackHole && star_mass.0 > 500.0)
    {
        // =========================================================================
        // SUPERMASSIVE QUASAR / QUASI-STAR: Relativistic Beams, Synchrotron Sheath, Accretion Disk
        // =========================================================================
        let is_blown_out = opt_quasi.map(|qs| qs.is_blown_out).unwrap_or(false)
            || star_body.name.contains("Quasar");
        let blowout_p = opt_quasi
            .map(|qs| qs.blowout_progress)
            .unwrap_or(if is_blown_out { 1.0 } else { 0.0 });
        let light_dist = opt_quasi
            .map(|qs| qs.jet_travel_distance_au as f32)
            .unwrap_or(0.0);
        let disk_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);

        // A. BRILLIANT WHITE RELATIVISTIC LASER BEAMS & ACCRETION DISK (ONLY POST-BLOWOUT AT SPEED c)
        if is_blown_out && light_dist > 0.5 {
            let jet_len = light_dist;
            let current_visual_radius =
                config.calc_visual_radius_for_type(star_radius.0, star_body.body_type);
            let pole_start = (current_visual_radius * 0.90).max(0.05);

            for &(dir, sign) in &[(Vec3::Y, 1.0f32), (-Vec3::Y, -1.0f32)] {
                let base = star_vec + dir * pole_start;
                let tip = star_vec + dir * jet_len;

                // 1. Central Core Laser Filament: Blinding pure white
                gizmos.line(base, tip, Color::WHITE);

                // Concentric thin laser beam core bundle (8 longitudinal lines at r = 0.06 AU)
                let core_r = 0.06f32;
                for k in 0..8 {
                    let theta = (k as f32) * (std::f32::consts::PI / 4.0);
                    let offset = Vec3::new(theta.cos() * core_r, 0.0, theta.sin() * core_r);
                    gizmos.line(
                        base + offset,
                        tip + offset,
                        Color::srgba(0.95, 0.98, 1.0, 0.95),
                    );
                }

                // 2. Ionized Synchrotron Sheath (12 lines at r = 0.18 AU)
                let sheath_r = 0.18f32;
                for k in 0..12 {
                    let theta = (k as f32) * (std::f32::consts::PI / 6.0);
                    let base_offset = Vec3::new(
                        theta.cos() * sheath_r * 0.8,
                        0.0,
                        theta.sin() * sheath_r * 0.8,
                    );
                    let tip_offset = Vec3::new(theta.cos() * sheath_r, 0.0, theta.sin() * sheath_r);
                    let sheath_color = if k % 2 == 0 {
                        Color::srgba(0.20, 0.75, 1.0, 0.75) // Electric Cyan
                    } else {
                        Color::srgba(0.75, 0.35, 1.0, 0.70) // Royal Violet
                    };
                    gizmos.line(base + base_offset, tip + tip_offset, sheath_color);
                }

                // 3. Relativistic Helical Magnetic Coils (Twisting 3D Plasma Spirals)
                let helix_len = jet_len.min(400.0);
                let n_segments = 100;
                let dz = (helix_len - pole_start).max(1.0) / (n_segments as f32);
                for s in 0..n_segments {
                    let z1 = pole_start + (s as f32) * dz;
                    let z2 = pole_start + ((s + 1) as f32) * dz;
                    let r1 = 0.12 + 0.0002 * z1;
                    let r2 = 0.12 + 0.0002 * z2;
                    let ang1 = z1 * 0.28 - elapsed * 16.0;
                    let ang2 = z2 * 0.28 - elapsed * 16.0;

                    // Helix 1 (Cyan/White)
                    let p1 = star_vec + Vec3::new(r1 * ang1.cos(), sign * z1, r1 * ang1.sin());
                    let p2 = star_vec + Vec3::new(r2 * ang2.cos(), sign * z2, r2 * ang2.sin());
                    gizmos.line(p1, p2, Color::srgba(0.40, 0.85, 1.0, 0.85));

                    // Helix 2 (Violet/Magenta opposite phase)
                    let ang1_b = ang1 + std::f32::consts::PI;
                    let ang2_b = ang2 + std::f32::consts::PI;
                    let p1_b =
                        star_vec + Vec3::new(r1 * ang1_b.cos(), sign * z1, r1 * ang1_b.sin());
                    let p2_b =
                        star_vec + Vec3::new(r2 * ang2_b.cos(), sign * z2, r2 * ang2_b.sin());
                    gizmos.line(p1_b, p2_b, Color::srgba(0.85, 0.35, 1.0, 0.80));
                }

                // 4. Collimation Mach Shock Disks (Periodic bright shock knots spaced along beam)
                let mut kd = 25.0f32;
                let mut step = 40.0f32;
                while kd < jet_len && kd < 5000.0 {
                    let knot_pos = star_vec + dir * kd;
                    let knot_r = 0.14 + kd * 0.0005;
                    gizmos.circle(
                        Isometry3d::new(
                            knot_pos,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        knot_r,
                        Color::WHITE,
                    );
                    gizmos.circle(
                        Isometry3d::new(
                            knot_pos,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        knot_r * 1.5,
                        Color::srgba(0.30, 0.75, 1.0, 0.65),
                    );
                    kd += step;
                    step *= 1.35;
                }

                // 5. Terminal Relativistic Bow Shock Hotspot (Expanding Radio Lobes at light front)
                let lobe_r = 0.55f32;
                gizmos.sphere(
                    Isometry3d::from_translation(tip),
                    lobe_r,
                    Color::srgba(0.85, 0.95, 1.0, 0.80),
                );
                gizmos.circle(
                    Isometry3d::new(tip, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    lobe_r * 1.5,
                    Color::srgba(0.35, 0.80, 1.0, 0.75),
                );
                gizmos.circle(
                    Isometry3d::new(tip, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    lobe_r * 1.5,
                    Color::srgba(0.80, 0.35, 1.0, 0.60),
                );
            }

            // B. RELATIVISTIC PARTICLE STREAKS ORBITING NEAR ISCO (POST-BLOWOUT)
            // After the hydrogen cocoon is blown away, the exposed supermassive black hole
            // has almost nothing left to illuminate. Instead of a luminous accretion disk,
            // we render individual high-speed particles racing in tight orbits near the
            // Innermost Stable Circular Orbit (ISCO) at close to the speed of light.
            //
            // 24 particles in 3 orbital tiers (ISCO, mid, outer) with short comet-like
            // tails that fade behind each particle, creating a sparse but dynamic effect.

            let current_visual_r =
                config.calc_visual_radius_for_type(star_radius.0, star_body.body_type);
            let isco_r = (current_visual_r * 1.15).max(2.8); // Just outside the visible event horizon

            // Particle orbital tiers: (orbit radius multiplier, particle count, angular speed, color)
            let particle_tiers: [(f32, u32, f32, Color); 3] = [
                // Inner tier: ISCO particles — fastest, brilliant white-cyan
                (1.0, 10, 18.0, Color::srgba(0.85, 0.95, 1.0, 0.95)),
                // Mid tier: slightly farther out, electric blue
                (1.6, 8, 12.0, Color::srgba(0.30, 0.75, 1.0, 0.85)),
                // Outer tier: widest orbit, dimmer cyan-violet
                (2.4, 6, 7.5, Color::srgba(0.55, 0.40, 1.0, 0.70)),
            ];

            let tail_segments = 6u32;
            let tail_dt = 0.012f32; // Time step between trail dots

            for &(r_mult, count, omega, col) in &particle_tiers {
                let orbit_r = isco_r * r_mult;

                for p_idx in 0..count {
                    // Each particle has a unique phase offset
                    let phase_offset = (p_idx as f32) * (std::f32::consts::TAU / count as f32)
                        + (p_idx as f32) * 1.618; // Golden ratio spread
                    let y_wobble = ((p_idx as f32) * 0.7 + elapsed * 2.5).sin() * orbit_r * 0.04;

                    // Draw the comet-like tail (fading segments behind the particle)
                    let mut prev_pt: Option<Vec3> = None;
                    for seg in (0..=tail_segments).rev() {
                        let t = elapsed - (seg as f32) * tail_dt;
                        let theta = phase_offset + t * omega;
                        let pt = star_vec
                            + Vec3::new(orbit_r * theta.cos(), y_wobble, orbit_r * theta.sin());

                        if let Some(prev) = prev_pt {
                            let fade = 1.0 - (seg as f32 / tail_segments as f32);
                            let seg_col = Color::srgba(
                                col.to_srgba().red,
                                col.to_srgba().green,
                                col.to_srgba().blue,
                                col.to_srgba().alpha * fade * fade,
                            );
                            gizmos.line(prev, pt, seg_col);
                        }
                        prev_pt = Some(pt);
                    }

                    // Bright head particle (current position)
                    let head_theta = phase_offset + elapsed * omega;
                    let head_pt = star_vec
                        + Vec3::new(
                            orbit_r * head_theta.cos(),
                            y_wobble,
                            orbit_r * head_theta.sin(),
                        );
                    // Draw a tiny bright cross at the particle head
                    let cross_size = 0.12f32;
                    gizmos.line(
                        head_pt - Vec3::X * cross_size,
                        head_pt + Vec3::X * cross_size,
                        Color::WHITE,
                    );
                    gizmos.line(
                        head_pt - Vec3::Z * cross_size,
                        head_pt + Vec3::Z * cross_size,
                        Color::WHITE,
                    );
                }
            }

            // Faint ISCO reference ring — just a whisper of the innermost stable orbit
            gizmos.circle(
                Isometry3d::new(star_vec, disk_rot),
                isco_r,
                Color::srgba(0.40, 0.75, 1.0, 0.12),
            );
        }

        // C. EXPANDING COCOON BLOWOUT SHOCKWAVE BLAST (HYDROGEN ENVELOPE DISPERSION)
        if blowout_p > 0.001 && blowout_p < 1.0 {
            let r_blast = (60.0 + blowout_p * 240.0).clamp(60.0, 300.0);
            let blast_fade = (1.0 - blowout_p).clamp(0.0, 1.0);
            gizmos.circle(
                Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                r_blast,
                Color::srgba(1.0, 0.45, 0.20, 0.90 * blast_fade),
            );
            gizmos.circle(
                Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                r_blast * 0.97,
                Color::srgba(1.0, 0.75, 0.30, 0.75 * blast_fade),
            );
            gizmos.circle(
                Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                r_blast,
                Color::srgba(0.85, 0.25, 0.15, 0.50 * blast_fade),
            );
            gizmos.sphere(
                Isometry3d::from_translation(star_vec),
                r_blast,
                Color::srgba(0.70, 0.20, 0.10, 0.06 * blast_fade),
            );
        } else if star_body.body_type == BodyType::QuasiStar {
            // Intact 60 AU Quasi-Star (JWST Little Red Dot): Extreme Luminosity & 2.5 MegaGauss Blandford-Znajek Magnetosphere
            let envelope_r = 60.0f32;

            // 1. Multi-tier Radiant Envelope Boundaries & Incandescent Photosphere Halos
            for (dr, alpha, col) in [
                (0.0f32, 0.70, Color::srgba(1.0, 0.28, 0.08, 0.70)),
                (2.5f32, 0.45, Color::srgba(1.0, 0.48, 0.12, 0.45)),
                (6.0f32, 0.25, Color::srgba(1.0, 0.70, 0.20, 0.25)),
                (12.0f32, 0.12, Color::srgba(0.95, 0.85, 0.35, 0.12)),
            ] {
                let r = envelope_r + dr;
                gizmos.circle(
                    Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    r,
                    col,
                );
                gizmos.circle(
                    Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    r,
                    col,
                );
                gizmos.sphere(
                    Isometry3d::from_translation(star_vec),
                    r,
                    Color::srgba(
                        col.to_srgba().red,
                        col.to_srgba().green,
                        col.to_srgba().blue,
                        alpha * 0.08,
                    ),
                );
            }

            // 2. Towering Relativistic Magnetic Dipole Field Arches (Poloidal Magnetosphere)
            // 8 great magnetic dipole loops revolving with the black hole star's rotation
            let mag_spin = Quat::from_rotation_y(elapsed * 0.25) * Quat::from_rotation_x(0.22);
            for i in 0..8 {
                let phi = (i as f32) * (std::f32::consts::PI / 4.0);
                let r0 = 95.0 + ((i % 3) as f32) * 18.0; // Arch apex at 95 - 131 AU
                let mut prev_pt: Option<Vec3> = None;
                let n_pts = 28;

                for s in 0..=n_pts {
                    let theta = 0.38 + (std::f32::consts::PI - 0.76) * (s as f32 / n_pts as f32);
                    let r_dipole = r0 * theta.sin().powi(2);
                    if r_dipole >= envelope_r * 0.95 {
                        let local_p = Vec3::new(
                            r_dipole * theta.sin() * phi.cos(),
                            r_dipole * theta.cos(),
                            r_dipole * theta.sin() * phi.sin(),
                        );
                        let world_p = star_vec + mag_spin * local_p;
                        if let Some(p_prev) = prev_pt {
                            let line_col = if i % 2 == 0 {
                                Color::srgba(0.25, 0.88, 1.0, 0.75) // Electric Cyan Synchrotron
                            } else {
                                Color::srgba(1.0, 0.75, 0.25, 0.70) // Solar Gold
                            };
                            gizmos.line(p_prev, world_p, line_col);
                        }
                        prev_pt = Some(world_p);
                    }
                }
            }

            // 3. Toroidal Magnetic Confinement Belts (Twisting Alfvén Flux Ropes)
            for (y_lat, belt_r, wave_speed) in [
                (0.0f32, envelope_r + 1.8, 4.0f32),
                (22.0f32, envelope_r * 0.92, -3.5f32),
                (-22.0f32, envelope_r * 0.92, 3.5f32),
                (38.0f32, envelope_r * 0.75, -5.0f32),
                (-38.0f32, envelope_r * 0.75, 5.0f32),
            ] {
                let n_seg = 48;
                let d_theta = std::f32::consts::TAU / (n_seg as f32);
                for k in 0..n_seg {
                    let th1 = (k as f32) * d_theta;
                    let th2 = ((k + 1) as f32) * d_theta;
                    let wave1 = (th1 * 8.0 + elapsed * wave_speed).sin() * 1.6;
                    let wave2 = (th2 * 8.0 + elapsed * wave_speed).sin() * 1.6;
                    let r1 = belt_r + wave1;
                    let r2 = belt_r + wave2;

                    let p1 = star_vec + Vec3::new(r1 * th1.cos(), y_lat, r1 * th1.sin());
                    let p2 = star_vec + Vec3::new(r2 * th2.cos(), y_lat, r2 * th2.sin());
                    gizmos.line(p1, p2, Color::srgba(0.85, 0.35, 1.0, 0.65)); // Royal Violet Alfvén Wave
                }
            }

            // 4. Pulsating Synchrotron Auroral Rings (North & South Magnetic Caps)
            for sign in [1.0f32, -1.0f32] {
                let cap_center = star_vec + mag_spin * Vec3::new(0.0, sign * 58.5, 0.0);
                let cap_rot = mag_spin * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
                let auroral_r = 18.0 + (elapsed * 3.0 + sign).sin() * 2.0;

                gizmos.circle(
                    Isometry3d::new(cap_center, cap_rot),
                    auroral_r,
                    Color::srgba(0.20, 0.95, 1.0, 0.90), // Electric Cyan Synchrotron Oval
                );
                gizmos.circle(
                    Isometry3d::new(cap_center, cap_rot),
                    auroral_r * 1.25,
                    Color::srgba(0.35, 1.0, 0.50, 0.75), // Aurora Emerald Glow
                );
            }

            // 5. Giant Magnetic Plasma Prominences (Coronal Plasma Ejections)
            for p in 0..6 {
                let base_ang = (p as f32) * (std::f32::consts::PI / 3.0) + elapsed * 0.08;
                let prom_height = 14.0 + ((p * 7) as f32 % 5.0) * 3.5;
                let apex_vec = Vec3::new(
                    base_ang.cos(),
                    0.35 * ((p % 2) as f32 * 2.0 - 1.0),
                    base_ang.sin(),
                )
                .normalize();
                let prom_apex = star_vec + apex_vec * (envelope_r + prom_height);

                let foot1_ang = base_ang - 0.18;
                let foot2_ang = base_ang + 0.18;
                let foot1 =
                    star_vec + Vec3::new(foot1_ang.cos(), 0.0, foot1_ang.sin()) * envelope_r;
                let foot2 =
                    star_vec + Vec3::new(foot2_ang.cos(), 0.0, foot2_ang.sin()) * envelope_r;

                // Parabolic prominence loop
                let mut prev = foot1;
                for step in 1..=12 {
                    let t = step as f32 / 12.0;
                    let base_interp = foot1.lerp(foot2, t);
                    let loop_pt = base_interp + (prom_apex - star_vec) * (4.0 * t * (1.0 - t));
                    gizmos.line(prev, loop_pt, Color::srgba(1.0, 0.40, 0.10, 0.80));
                    prev = loop_pt;
                }
            }
        }

        // =========================================================================
        // D. GENERAL RELATIVISTIC GRAVITATIONAL LENSING & SPATIAL WARPING SHELLS
        // =========================================================================
        // Because the black hole / Quasi-Star has an enormous mass (400,000 - 450,000 M_sun),
        // it bends and warps the space around it into distinct General Relativistic caustics:
        // 1. Photon Sphere Caustic Ring: Luminous ring at r ~ 1.5 R_horizon where light orbits
        // 2. Concentric Einstein Deflection Rings: 5 tiered gravitational caustic shells
        // 3. Spacetime Geodesic Funnel Arcs: Curving field lines tracing spatial metric curvature
        //
        // Dynamic scaling:
        // - Intact Quasi-Star: Visual radius is 60 AU, lensing caustics span 65 - 180 AU.
        // - Blown out Black Hole: Visual radius is ~2.5 AU, lensing caustics focus tightly at 3.75 - 20 AU.
        // - During blowout: Shells smoothly contract in lockstep with the event horizon.

        let current_visual_r =
            config.calc_visual_radius_for_type(star_radius.0, star_body.body_type);
        let base_lens_r = (current_visual_r * 1.50).max(3.6); // Photon sphere caustic radius

        // 1. Primary Photon Sphere Caustic Ring & Concentric Gravitational Deflection Shells
        for (ring_scale, alpha_mul, col) in [
            (1.00f32, 0.95f32, Color::srgba(1.0, 0.96, 0.88, 0.95)), // Blinding white-gold photon caustic
            (1.04f32, 0.70f32, Color::srgba(0.35, 0.85, 1.0, 0.70)), // Blueshifted inner relativistic edge
            (1.18f32, 0.45f32, Color::srgba(1.0, 0.65, 0.20, 0.45)), // Gravitationally redshifted amber
            (1.45f32, 0.25f32, Color::srgba(0.85, 0.25, 0.50, 0.25)), // Secondary Einstein ring
            (2.20f32, 0.12f32, Color::srgba(0.50, 0.20, 0.85, 0.12)), // Tertiary deflection caustic
        ] {
            let r_ring = base_lens_r * ring_scale;
            // Horizontal plane ring
            gizmos.circle(
                Isometry3d::new(star_vec, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                r_ring,
                col,
            );
            // Cross-axial Einstein ring (orthogonal plane highlighting 3D gravitational sphere)
            gizmos.circle(
                Isometry3d::new(star_vec, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                r_ring,
                Color::srgba(
                    col.to_srgba().red,
                    col.to_srgba().green,
                    col.to_srgba().blue,
                    alpha_mul * 0.35,
                ),
            );
            // Subtle 3D gravitational distortion sphere
            if ring_scale < 1.3 {
                gizmos.sphere(
                    Isometry3d::from_translation(star_vec),
                    r_ring,
                    Color::srgba(
                        col.to_srgba().red,
                        col.to_srgba().green,
                        col.to_srgba().blue,
                        alpha_mul * 0.04,
                    ),
                );
            }
        }

        // 2. Spacetime Curvature Geodesic Funnel Arcs (Kerr Metric Frame-Dragging)
        // 12 spiral arcs showing the dragging and funneling of space into the gravitational singularity
        let n_geodesics = 12u32;
        let spin_rot = Quat::from_rotation_y(elapsed * 0.35);
        for g_idx in 0..n_geodesics {
            let base_angle = (g_idx as f32) * (std::f32::consts::TAU / n_geodesics as f32);
            let mut prev_geo: Option<Vec3> = None;
            let n_steps = 24;

            for step in 0..=n_steps {
                let frac = step as f32 / n_steps as f32;
                // Radius from outer warped space (3.5x photon sphere) down to event horizon
                let r_geo = base_lens_r * (3.5 - frac * 2.3);
                // Relativistic frame-dragging spiral angle: logarithmic inward wrap
                let theta_geo = base_angle + frac * 2.8 + (1.0 - frac) * 0.5;
                // Subtle vertical funneling (embedding diagram / gravitational potential well)
                let y_funnel = -((frac * 1.8).powi(2)) * (base_lens_r * 0.08);

                let local_pt =
                    Vec3::new(r_geo * theta_geo.cos(), y_funnel, r_geo * theta_geo.sin());
                let world_pt = star_vec + spin_rot * local_pt;

                if let Some(prev) = prev_geo {
                    let fade = (1.0 - frac * 0.75) * (0.35 + (g_idx % 2) as f32 * 0.25);
                    let geo_col = if is_blown_out {
                        Color::srgba(0.40, 0.80, 1.0, 0.45 * fade) // Electric relativistic cyan
                    } else {
                        Color::srgba(1.0, 0.45, 0.15, 0.40 * fade) // Primordial cosmic dawn amber
                    };
                    gizmos.line(prev, world_pt, geo_col);
                }
                prev_geo = Some(world_pt);
            }
        }
    } else if star_body.body_type == BodyType::BlackHole {
        // Standard stellar-mass Black Hole: Relativistic Accretion Disk & Polar Jets
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
        let jet_len = 12.0f32;
        let jet_color = Color::srgba(0.4, 0.85, 1.0, 0.75);
        gizmos.line(star_vec, star_vec + Vec3::Y * jet_len, jet_color);
        gizmos.line(star_vec, star_vec - Vec3::Y * jet_len, jet_color);
        gizmos.circle(
            Isometry3d::new(
                star_vec + Vec3::Y * jet_len,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            0.65,
            jet_color,
        );
        gizmos.circle(
            Isometry3d::new(
                star_vec - Vec3::Y * jet_len,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            0.65,
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

    // 2A. Draw Roche Tidal Disruption Debris Streamers
    for stream in debris_pool.streams.iter() {
        let p = (stream.timer / stream.max_timer).clamp(0.0, 1.0);
        let alpha = if p < 0.15 {
            p / 0.15
        } else if p > 0.65 {
            (1.0 - p) / 0.35
        } else {
            1.0
        };

        let base_col = if stream.ice_fraction >= 0.70 {
            Color::srgba(0.85, 0.95, 1.0, 0.90 * alpha)
        } else if stream.ice_fraction >= 0.35 {
            Color::srgba(0.95, 0.80, 0.50, 0.85 * alpha)
        } else {
            Color::srgba(1.0, 0.50, 0.20, 0.85 * alpha)
        };
        let spark_col = Color::srgba(1.0, 1.0, 1.0, 0.95 * alpha);

        let n = stream.fragments.len();
        for i in 0..n {
            let (r1, phi1, _, z1) = stream.fragments[i];
            let pos1 = stream.primary_pos + Vec3::new(r1 * phi1.cos(), z1, r1 * phi1.sin());

            // Bright fragment spark point
            gizmos.sphere(
                Isometry3d::from_translation(pos1),
                0.015 + 0.012 * (1.0 - p),
                spark_col,
            );

            // Connect adjacent fragments with glowing streamer ribbon line
            if i + 1 < n {
                let (r2, phi2, _, z2) = stream.fragments[i + 1];
                let pos2 = stream.primary_pos + Vec3::new(r2 * phi2.cos(), z2, r2 * phi2.sin());
                gizmos.line(pos1, pos2, base_col);
            }
        }
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
    for (entity, pos, vel, mass, comp, body, opt_diff, opt_spin, opt_rad, opt_tail) in
        bodies_query.iter()
    {
        let is_selected = player_state.selected_entity == Some(entity);
        let is_planet = matches!(
            body.body_type,
            BodyType::Protoplanet
                | BodyType::TerrestrialPlanet
                | BodyType::SuperEarth
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

        // 3D Photoevaporative Cometary Atmospheric Escape Tail (Hydrodynamic Envelope Stripping)
        if let Some(tail) = opt_tail {
            if tail.is_active && tail.tail_length_au > 0.05 {
                let star_to_body = (body_vec - star_vec).normalize_or_zero();
                if star_to_body != Vec3::ZERO {
                    let v_orb = Vec3::new(vel.x as f32, vel.y as f32, vel.z as f32);
                    // Anti-stellar outflow axis with orbital aberration tilt
                    let tail_axis = (star_to_body - v_orb * 0.035).normalize();
                    let tail_len = tail.tail_length_au;
                    let base_r = opt_rad
                        .map(|r| config.calc_visual_radius(r.0) * 1.15)
                        .unwrap_or(0.04);

                    // A. Supersonic core ion spine
                    let n_spine = 16;
                    let ds = tail_len / (n_spine as f32);
                    for s_idx in 0..n_spine {
                        let s1 = (s_idx as f32) * ds;
                        let s2 = ((s_idx + 1) as f32) * ds;
                        let t_frac = (s_idx as f32) / (n_spine as f32);
                        let a_spine = (1.0 - t_frac).powi(2) * 0.90;
                        let p1 = body_vec + tail_axis * s1;
                        let p2 = body_vec + tail_axis * s2;
                        let spine_col = Color::srgba(
                            tail.ion_color.to_srgba().red,
                            tail.ion_color.to_srgba().green,
                            tail.ion_color.to_srgba().blue,
                            a_spine,
                        );
                        gizmos.line(p1, p2, spine_col);
                    }

                    // B. Parabolic expanding cometary bow sheath (4 azimuthal streamer ribs)
                    let ortho1 = if tail_axis.y.abs() < 0.95 {
                        tail_axis.cross(Vec3::Y).normalize()
                    } else {
                        tail_axis.cross(Vec3::Z).normalize()
                    };
                    let ortho2 = tail_axis.cross(ortho1).normalize();

                    for rib_i in 0..4 {
                        let rib_ang = (rib_i as f32) * (std::f32::consts::PI * 0.5);
                        let rib_dir = ortho1 * rib_ang.cos() + ortho2 * rib_ang.sin();

                        for s_idx in 0..n_spine {
                            let frac1 = (s_idx as f32) / (n_spine as f32);
                            let frac2 = ((s_idx + 1) as f32) / (n_spine as f32);

                            let s1 = frac1 * tail_len;
                            let s2 = frac2 * tail_len;

                            let w1 =
                                base_r + frac1.sqrt() * (base_r * 2.5 + 0.12 * tail_len.sqrt());
                            let w2 =
                                base_r + frac2.sqrt() * (base_r * 2.5 + 0.12 * tail_len.sqrt());

                            let p1 = body_vec + tail_axis * s1 + rib_dir * w1;
                            let p2 = body_vec + tail_axis * s2 + rib_dir * w2;

                            let a_sheath = (1.0 - frac1) * 0.45;
                            let sheath_col = Color::srgba(
                                tail.ion_color.to_srgba().red,
                                tail.ion_color.to_srgba().green,
                                tail.ion_color.to_srgba().blue,
                                a_sheath,
                            );
                            gizmos.line(p1, p2, sheath_col);
                        }
                    }

                    // C. Periodic travelling ionization knot pulses
                    for pulse_i in 0..3 {
                        let pulse_phase = ((elapsed * 2.0 + (pulse_i as f32 * 1.33)) % 4.0) / 4.0;
                        let pulse_s = pulse_phase * tail_len;
                        let knot_pos = body_vec + tail_axis * pulse_s;
                        let knot_a = (1.0 - pulse_phase) * 0.8;
                        gizmos.sphere(
                            Isometry3d::from_translation(knot_pos),
                            base_r * (0.6 + 0.4 * pulse_phase),
                            Color::srgba(
                                tail.ion_color.to_srgba().red,
                                tail.ion_color.to_srgba().green,
                                tail.ion_color.to_srgba().blue,
                                knot_a,
                            ),
                        );
                    }
                }
            }
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
                                BodyType::SuperEarth => Color::srgba(0.25, 0.85, 0.75, 0.5),
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
