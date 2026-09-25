//! Supernova core-collapse explosions, high-velocity matter ejecta, and planetary nebulae.

use bevy::prelude::*;

use crate::rendering::particle_swarm::ParticleSwarmData;
use crate::simulation::components::*;
use crate::simulation::resources::SimTime;

/// Classification of stellar death explosion mechanisms based on initial mass and degeneracy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum SupernovaType {
    /// Type II core-collapse (8 - 25 M☉): violent iron collapse leaving a Pulsar / Neutron Star
    TypeII,
    /// Hypernova / Collapsar (≥ 25 M☉): extreme energetic detonation leaving a Black Hole
    Hypernova,
    /// Type Ia thermonuclear detonation (> 1.44 M☉ White Dwarf): complete disruption
    TypeIa,
    /// Gentle thermal envelope shedding (0.5 - 8 M☉): multi-layer nebula leaving a White Dwarf
    PlanetaryNebula,
}

/// Chemical and physical layer of expanding stellar ejecta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EjectaLayer {
    /// Inner radioactive Nickel-56 / Cobalt / Iron clumps (golden-amber radioactive glow)
    CoreNickelIron,
    /// Intermediate burning mantle: Oxygen, Silicon, Sulfur (intense [O III] cyan/emerald)
    MantleOxygenSilicon,
    /// Outer fast hydrogen-rich envelope (H-alpha crimson / ruby)
    OuterEnvelopeHydrogen,
    /// Relativistic polar shock cone for Hypernovas (sapphire / violet)
    RelativisticJetBreakout,
}

/// A discrete 3D clump of matter ejected at thousands of km/s from the dying star.
#[derive(Debug, Clone)]
pub struct SupernovaEjectaFragment {
    /// Current 3D position in AU
    pub pos: Vec3,
    /// Expansion velocity in AU/sec
    pub vel: Vec3,
    /// Clump size in AU
    pub scale: f32,
    /// Current temperature in Kelvin
    pub temp_k: f32,
    /// Composition layer
    pub layer: EjectaLayer,
    /// Turbulent seed for procedural Raylegh-Taylor curling
    pub seed: f32,
}

/// An active expanding supernova or planetary nebula explosion instance.
#[derive(Debug, Clone)]
pub struct SupernovaExplosionInstance {
    pub star_entity: Entity,
    pub center: Vec3,
    pub explosion_type: SupernovaType,
    pub blast_speed_au_s: f32,
    pub current_radius_au: f32,
    pub max_radius_au: f32,
    pub ejecta_mass_solar: f32,
    pub remnant_type: BodyType,
    pub timer: f32,
    pub max_timer: f32,
    pub flash_intensity: f32,
    pub fragments: Vec<SupernovaEjectaFragment>,
}

/// Pool resource managing active supernova explosions, matter ejection, and shockwaves.
#[derive(Resource, Default)]
pub struct SupernovaDebrisPool {
    pub explosions: Vec<SupernovaExplosionInstance>,
    pub handled_entities: hashbrown::HashSet<Entity>,
}

impl SupernovaDebrisPool {
    /// Helper to clear tracking when loading a new scenario.
    pub fn clear(&mut self) {
        self.explosions.clear();
        self.handled_entities.clear();
    }
}

fn determine_supernova_type(initial_mass: f64, remnant_type: BodyType) -> SupernovaType {
    if remnant_type == BodyType::WhiteDwarf {
        SupernovaType::PlanetaryNebula
    } else if initial_mass >= 25.0 || remnant_type == BodyType::BlackHole {
        SupernovaType::Hypernova
    } else if remnant_type == BodyType::Pulsar || remnant_type == BodyType::NeutronStar {
        if initial_mass < 2.0 {
            SupernovaType::TypeIa
        } else {
            SupernovaType::TypeII
        }
    } else {
        SupernovaType::TypeII
    }
}

fn spawn_ejecta_fragments(
    explosion_type: SupernovaType,
    speed_au_s: f32,
    center: Vec3,
) -> Vec<SupernovaEjectaFragment> {
    let n_fragments = match explosion_type {
        SupernovaType::Hypernova => 160,
        SupernovaType::TypeII => 128,
        SupernovaType::TypeIa => 96,
        SupernovaType::PlanetaryNebula => 72,
    };

    let mut fragments = Vec::with_capacity(n_fragments);

    for i in 0..n_fragments {
        let u = (i as f32 + 0.5) / n_fragments as f32;
        let theta = (u * 2.0 - 1.0).asin();
        let golden_angle = 2.399_963_2; // Golden ratio * 2 * PI
        let phi = (i as f32) * golden_angle;

        let base_dir = Vec3::new(
            theta.cos() * phi.cos(),
            theta.sin(),
            theta.cos() * phi.sin(),
        );

        let seed = (i as f32 * 17.3).fract();
        let speed_var = 0.55 + 0.90 * seed;
        let mut fragment_speed = speed_au_s * speed_var;

        let layer = if explosion_type == SupernovaType::Hypernova && base_dir.y.abs() > 0.75 {
            fragment_speed *= 1.45; // Relativistic polar jet boost
            EjectaLayer::RelativisticJetBreakout
        } else if (0..n_fragments / 4).contains(&i) {
            EjectaLayer::CoreNickelIron
        } else if (n_fragments / 4..n_fragments * 2 / 3).contains(&i) {
            EjectaLayer::MantleOxygenSilicon
        } else {
            EjectaLayer::OuterEnvelopeHydrogen
        };

        let temp_k = match layer {
            EjectaLayer::RelativisticJetBreakout => 250_000.0,
            EjectaLayer::CoreNickelIron => 120_000.0,
            EjectaLayer::MantleOxygenSilicon => 65_000.0,
            EjectaLayer::OuterEnvelopeHydrogen => 35_000.0,
        };

        let scale = match explosion_type {
            SupernovaType::Hypernova => 0.18 + seed * 0.25,
            SupernovaType::TypeII => 0.12 + seed * 0.18,
            SupernovaType::TypeIa => 0.10 + seed * 0.15,
            SupernovaType::PlanetaryNebula => 0.08 + seed * 0.12,
        };

        fragments.push(SupernovaEjectaFragment {
            pos: center + base_dir * 0.1,
            vel: base_dir * fragment_speed,
            scale,
            temp_k,
            layer,
            seed,
        });
    }

    fragments
}

/// Spawns a new visual and physical supernova explosion instance.
pub fn trigger_supernova_explosion(
    pool: &mut SupernovaDebrisPool,
    star_entity: Entity,
    center: Vec3,
    initial_mass: f64,
    remnant_mass: f64,
    remnant_type: BodyType,
) {
    let explosion_type = determine_supernova_type(initial_mass, remnant_type);

    let (blast_speed_au_s, max_radius_au, max_timer, prompt_breakout_au) = match explosion_type {
        SupernovaType::Hypernova => (75.0, 150.0, 8.0, 3.0),
        SupernovaType::TypeII => (45.0, 110.0, 7.5, 2.5),
        SupernovaType::TypeIa => (35.0, 95.0, 6.5, 0.8),
        SupernovaType::PlanetaryNebula => (4.5, 45.0, 12.0, 0.5),
    };

    let fragments = spawn_ejecta_fragments(explosion_type, blast_speed_au_s, center);

    pool.explosions.push(SupernovaExplosionInstance {
        star_entity,
        center,
        explosion_type,
        blast_speed_au_s,
        current_radius_au: prompt_breakout_au,
        max_radius_au,
        ejecta_mass_solar: (initial_mass - remnant_mass).max(0.1) as f32,
        remnant_type,
        timer: 0.0,
        max_timer,
        flash_intensity: 1.0,
        fragments,
    });
    pool.handled_entities.insert(star_entity);
}

/// Updates expanding ejecta fragments, applies interstellar drag, and blows away circumstellar matter.
#[allow(clippy::type_complexity, reason = "Supernova System Queries")]
pub fn update_supernova_explosions(
    time: Res<Time>,
    _sim_time: Option<Res<SimTime>>,
    mut pool: ResMut<SupernovaDebrisPool>,
    mut supernova_events: MessageReader<SupernovaEvent>,
    star_query: Query<(
        Entity,
        &SimPosition,
        &Mass,
        &CelestialBody,
        Option<&StellarEvolutionState>,
    )>,
    mut bodies_query: Query<(
        Entity,
        &SimPosition,
        &mut SimVelocity,
        &mut Temperature,
        &CelestialBody,
        Option<&mut AtmosphericEscapeTail>,
    )>,
    mut swarm: Option<ResMut<ParticleSwarmData>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();

    // 1. Ingest SupernovaEvents
    for ev in supernova_events.read() {
        let center = star_query
            .get(ev.star_entity)
            .map_or(Vec3::ZERO, |(_, pos, ..)| {
                Vec3::new(pos.0.x as f32, pos.0.y as f32, pos.0.z as f32)
            });

        trigger_supernova_explosion(
            &mut pool,
            ev.star_entity,
            center,
            ev.initial_mass_solar,
            ev.remnant_mass_solar,
            ev.remnant_type,
        );
    }

    // 2. Fallback check for stars entering SupernovaExplosion or PlanetaryNebulaEjection phase directly
    for (entity, pos, mass, _body, opt_evo) in star_query.iter() {
        if let Some(evo) = opt_evo {
            let is_detonating = matches!(
                evo.phase,
                StellarEvolutionPhase::SupernovaExplosion
                    | StellarEvolutionPhase::PlanetaryNebulaEjection
            );
            if is_detonating && !pool.handled_entities.contains(&entity) {
                let center = Vec3::new(pos.0.x as f32, pos.0.y as f32, pos.0.z as f32);
                let remnant = if mass.0 >= 25.0 {
                    BodyType::BlackHole
                } else if mass.0 >= 8.0 {
                    BodyType::Pulsar
                } else {
                    BodyType::WhiteDwarf
                };
                let remnant_m = if mass.0 >= 25.0 {
                    3.5
                } else if mass.0 >= 8.0 {
                    1.44
                } else {
                    0.55
                };
                trigger_supernova_explosion(&mut pool, entity, center, mass.0, remnant_m, remnant);
            }
        }
    }

    // 3. Advance explosion shockwaves, ejecta, and blast waves
    for exp in &mut pool.explosions {
        advance_explosion_kinematics(exp, dt);

        if let Some(ref mut swarm_data) = swarm {
            apply_swarm_blast(exp, swarm_data, dt);
        }

        apply_celestial_blast(exp, &mut bodies_query, &mut commands);
    }

    // Retain active explosions
    pool.explosions.retain(|exp| exp.timer < exp.max_timer);
}

fn advance_explosion_kinematics(exp: &mut SupernovaExplosionInstance, dt: f32) {
    exp.timer += dt;
    let progress = (exp.timer / exp.max_timer).clamp(0.0, 1.0);

    // Flash decays sharply in the first second
    exp.flash_intensity = (-3.5 * exp.timer).exp();

    // Expanding blast radius from prompt breakout front (r ~ t^0.65)
    let prompt_breakout_au = match exp.explosion_type {
        SupernovaType::Hypernova => 3.0,
        SupernovaType::TypeII => 2.5,
        SupernovaType::TypeIa => 0.8,
        SupernovaType::PlanetaryNebula => 0.5,
    };
    let blast_progress = progress.powf(0.65);
    exp.current_radius_au =
        prompt_breakout_au + (exp.max_radius_au - prompt_breakout_au) * blast_progress;

    for frag in &mut exp.fragments {
        // Apply Rayleigh-Taylor curling turbulence
        let curl = Vec3::new(
            (frag.seed * 10.0 + exp.timer * 2.0).sin() * 0.15,
            (frag.seed * 7.0 + exp.timer * 2.0).cos() * 0.15,
            (frag.seed * 13.0 + exp.timer * 2.0).sin() * 0.15,
        );
        frag.pos += (frag.vel + curl * frag.vel.length() * 0.25) * dt;

        // Deceleration from interstellar medium resistance
        frag.vel *= 1.0 - (0.04 * dt).min(0.2);

        // Thermal cooling
        frag.temp_k = (frag.temp_k * (1.0 - 0.25 * dt)).max(120.0);
    }
}

fn apply_swarm_blast(
    exp: &SupernovaExplosionInstance,
    swarm_data: &mut ParticleSwarmData,
    dt: f32,
) {
    let r_shock = exp.current_radius_au;
    let r_prev = if exp.timer <= dt * 2.0 + 0.1 {
        0.0
    } else {
        (exp.current_radius_au - exp.blast_speed_au_s * dt.max(0.016)).max(0.0)
    };

    for i in 0..swarm_data.count {
        let Some(&[px, py, pz]) = swarm_data.positions.get(i) else {
            continue;
        };
        let dist = (px * px + py * py + pz * pz).sqrt();

        if dist >= r_prev && dist <= r_shock + 2.5 {
            let dir_x = if dist > 0.001 { px / dist } else { 1.0 };
            let dir_z = if dist > 0.001 { pz / dist } else { 0.0 };
            let kick = 15.0; // Outward supersonic kick
            if let Some(vel) = swarm_data.velocities.get_mut(i) {
                vel[0] += dir_x * kick;
                vel[2] += dir_z * kick;
            }
            if let Some(temp) = swarm_data.temperatures.get_mut(i) {
                *temp = 4500.0;
            }
            if let Some(comp) = swarm_data.compositions.get_mut(i) {
                comp.ice_frac = 0.0;
            }
            if let Some(col) = swarm_data.colors.get_mut(i) {
                *col = [1.0, 0.65, 0.25, 0.95];
            }
        }
    }
    swarm_data.is_dirty = true;
}

#[allow(clippy::type_complexity, reason = "Celestial Blast Query")]
fn apply_celestial_blast(
    exp: &SupernovaExplosionInstance,
    bodies_query: &mut Query<(
        Entity,
        &SimPosition,
        &mut SimVelocity,
        &mut Temperature,
        &CelestialBody,
        Option<&mut AtmosphericEscapeTail>,
    )>,
    commands: &mut Commands,
) {
    let center_dvec = exp.center.as_dvec3();
    let r_shock = f64::from(exp.current_radius_au);

    for (b_ent, b_pos, mut b_vel, mut b_temp, b_body, mut opt_tail) in bodies_query.iter_mut() {
        if b_ent == exp.star_entity {
            continue;
        }
        let rel_pos = b_pos.0 - center_dvec;
        let dist = rel_pos.length();

        if dist <= r_shock {
            // Vaporize small asteroids within inner 2.0 AU of core collapse
            if dist < 2.0 && matches!(b_body.body_type, BodyType::Asteroid | BodyType::Comet) {
                if let Ok(mut cmd) = commands.get_entity(b_ent) {
                    cmd.despawn();
                }
                continue;
            }

            // Blast outward momentum impulse
            let impulse_mag = (0.05 / dist.max(0.5)).min(0.2);
            b_vel.0 += rel_pos.normalize_or_zero() * impulse_mag;

            // Superheat planet surface
            let heating_boost = (5000.0 / (dist * dist).max(0.2)).clamp(100.0, 8000.0);
            b_temp.0 = b_temp.0.max(heating_boost);

            // Push atmospheric ablation tail pointing away from the remnant
            if let Some(ref mut tail) = opt_tail {
                tail.is_active = true;
                tail.tail_length_au = (2.5 / dist.sqrt()).clamp(0.5, 4.0) as f32;
                tail.loss_rate_m_earth_per_myr = 15.0;
                tail.ion_color = Color::srgba(1.0, 0.45, 0.20, 0.90);
            } else if !b_body.body_type.is_star_or_remnant() {
                if let Ok(mut cmd) = commands.get_entity(b_ent) {
                    cmd.insert(AtmosphericEscapeTail {
                        loss_rate_m_earth_per_myr: 15.0,
                        tail_length_au: 2.0,
                        ion_color: Color::srgba(1.0, 0.45, 0.20, 0.90),
                        is_active: true,
                    });
                }
            }
        }
    }
}

/// Renders prompt core fireball, expanding filamentary ejecta clumps, and supersonic shock boundaries.
pub fn draw_supernova_explosions(gizmos: &mut Gizmos, pool: &SupernovaDebrisPool) {
    for exp in &pool.explosions {
        let p = (exp.timer / exp.max_timer).clamp(0.0, 1.0);
        let alpha = (1.0 - p).clamp(0.0, 1.0);

        // 1. Prompt Core Detonation Flash (Shock Breakout Fireball)
        if exp.flash_intensity > 0.01 {
            let flash_r = (exp.timer * 4.5).min(2.5);
            gizmos.sphere(
                Isometry3d::from_translation(exp.center),
                flash_r,
                Color::srgba(1.0, 1.0, 1.0, exp.flash_intensity * 0.95),
            );
            gizmos.sphere(
                Isometry3d::from_translation(exp.center),
                flash_r * 1.8,
                Color::srgba(0.5, 0.85, 1.0, exp.flash_intensity * 0.60),
            );
        }

        // 2. Supersonic Forward Shockwave Shell
        let shock_col = match exp.explosion_type {
            SupernovaType::Hypernova => Color::srgba(0.85, 0.45, 1.0, alpha * 0.85),
            SupernovaType::TypeII => Color::srgba(0.40, 0.90, 1.0, alpha * 0.75),
            SupernovaType::TypeIa => Color::srgba(1.0, 0.95, 0.80, alpha * 0.80),
            SupernovaType::PlanetaryNebula => Color::srgba(0.25, 0.95, 0.75, alpha * 0.70),
        };

        gizmos.circle(
            Isometry3d::new(
                exp.center,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            exp.current_radius_au,
            shock_col,
        );
        gizmos.circle(
            Isometry3d::new(
                exp.center,
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            ),
            exp.current_radius_au * 0.95,
            Color::srgba(
                shock_col.to_srgba().red,
                shock_col.to_srgba().green,
                shock_col.to_srgba().blue,
                alpha * 0.45,
            ),
        );

        // 3. 3D Filamentary Matter Ejecta Knots and Streamers
        for frag in &exp.fragments {
            let knot_col = match frag.layer {
                EjectaLayer::RelativisticJetBreakout => Color::srgba(0.65, 0.35, 1.0, alpha * 0.95),
                EjectaLayer::CoreNickelIron => Color::srgba(1.0, 0.75, 0.20, alpha * 0.90),
                EjectaLayer::MantleOxygenSilicon => Color::srgba(0.15, 0.95, 0.85, alpha * 0.85),
                EjectaLayer::OuterEnvelopeHydrogen => Color::srgba(0.95, 0.25, 0.30, alpha * 0.80),
            };

            // Glowing ejecta knot
            gizmos.sphere(Isometry3d::from_translation(frag.pos), frag.scale, knot_col);

            // Streamer trail connecting ejecta knot back toward center (Rayleigh-Taylor finger)
            let tail_len = (frag.pos - exp.center).length().min(frag.scale * 6.0);
            let trail_dir = (exp.center - frag.pos).normalize_or_zero();
            let trail_end = frag.pos + trail_dir * tail_len;

            let trail_col = Color::srgba(
                knot_col.to_srgba().red,
                knot_col.to_srgba().green,
                knot_col.to_srgba().blue,
                alpha * 0.40,
            );
            gizmos.line(frag.pos, trail_end, trail_col);
        }
    }
}
