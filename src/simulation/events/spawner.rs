//! Spawners for physical celestial interlopers and hazard entities.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;

/// Marker component for an interstellar rogue body passing through the system.
#[derive(Component, Debug, Clone, Copy)]
pub struct RogueInterloper {
    pub initial_velocity: DVec3,
}

/// Marker component for cometary hazard impactors.
#[derive(Component, Debug, Clone, Copy)]
pub struct HazardComet;

/// Spawns an interstellar brown dwarf on a hyperbolic flyby trajectory.
pub fn spawn_rogue_star(commands: &mut Commands, intensity: f32) -> Entity {
    let mass_sol = f64::from(0.045 * intensity.clamp(0.8, 2.0));
    let start_pos = DVec3::new(-95.0, 8.0, 75.0);
    // Hyperbolic entry velocity vector passing within ~12 AU of the system barycenter
    let entry_vel = DVec3::new(4.2, -0.4, -3.1);

    commands
        .spawn((
            CelestialBody {
                body_type: BodyType::BrownDwarf,
                name: "Rogue Interloper (Nemesis-B)".to_string(),
            },
            Mass(mass_sol),
            Radius(0.082), // ~0.08 R☉ (~0.9 R_Jup)
            SimPosition(start_pos),
            SimVelocity(entry_vel),
            Composition {
                metal_frac: 0.01,
                silicate_frac: 0.02,
                ice_frac: 0.02,
                organics_frac: 0.0,
                gas_frac: 0.95,
            },
            Temperature(1750.0),
            RogueInterloper {
                initial_velocity: entry_vel,
            },
        ))
        .id()
}

/// Spawns a swarm of retrograde cometary nuclei from the outer system.
pub fn spawn_comet_outburst_swarm(commands: &mut Commands, count: usize) -> Vec<Entity> {
    spawn_comet_outburst_swarm_targeted(commands, count, &[])
}

/// Spawns a swarm of icy outburst comets on inward intercept trajectories toward target planets
/// (prioritizing Earth and inner terrestrial worlds).
pub fn spawn_comet_outburst_swarm_targeted(
    commands: &mut Commands,
    count: usize,
    targets: &[(Entity, DVec3, DVec3)],
) -> Vec<Entity> {
    let mut spawned = Vec::with_capacity(count);

    let standoff_distances = [0.22, 0.35, 0.50, 0.68, 0.85, 1.05];
    for i in 0..count {
        let standoff = *standoff_distances
            .get(i % standoff_distances.len())
            .unwrap_or(&0.45);
        let angle = (i as f64) * 1.15 + 0.35;

        let (pos, vel) = if targets.is_empty() {
            let dist = 1.6 + standoff * 2.0;
            let p = DVec3::new(
                dist * angle.cos(),
                (i as f64 - 1.5) * 0.04,
                dist * angle.sin(),
            );
            let dir = (-p).normalize_or_zero();
            (p, dir * 5.5 + DVec3::new(-0.3, 0.0, 0.3))
        } else {
            // Prioritize index 0 (Earth) for the first 3 comets, then cycle through inner planets
            let target_idx = if i < 3 { 0 } else { i % targets.len() };
            let (_, t_pos, t_vel) = targets
                .get(target_idx)
                .copied()
                .or_else(|| targets.first().copied())
                .unwrap_or((Entity::PLACEHOLDER, DVec3::new(1.0, 0.0, 0.0), DVec3::ZERO));
            let radial_out = if t_pos.length_squared() > 1e-4 {
                t_pos.normalize()
            } else {
                DVec3::new(angle.cos(), 0.0, angle.sin())
            };
            let tangent = DVec3::new(-radial_out.z, 0.0, radial_out.x).normalize_or_zero();
            let approach_dir = (radial_out * angle.cos().abs().max(0.55)
                + tangent * angle.sin() * 0.45
                + DVec3::new(0.0, (i as f64 - 1.5) * 0.02, 0.0))
            .normalize_or_zero();
            let p = t_pos + approach_dir * standoff;
            let closing_speed = 6.5 + (i as f64) * 0.6;
            let v = t_vel - approach_dir * closing_speed;
            (p, v)
        };

        let entity = commands
            .spawn((
                CelestialBody {
                    body_type: BodyType::Comet,
                    name: format!("Outburst Comet C/Hazard-{}", i + 1),
                },
                Mass(1.2e-9), // Volatile-rich cometary nucleus (~0.0004 M_earth)
                Radius(0.0004),
                SimPosition(pos),
                SimVelocity(vel),
                SimAcceleration::default(),
                Composition {
                    metal_frac: 0.05,
                    silicate_frac: 0.15,
                    ice_frac: 0.75,
                    organics_frac: 0.05,
                    gas_frac: 0.0,
                },
                Temperature(65.0),
                VolatileInventory {
                    delivered_water_m_earth: 0.0024,
                    ocean_coverage_frac: 0.0,
                    atmospheric_pressure_bar: 0.08,
                    cometary_impact_count: 0,
                },
                HazardComet,
            ))
            .id();
        spawned.push(entity);
    }

    spawned
}
