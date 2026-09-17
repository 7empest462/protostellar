//! Guided orbital bombardment launcher and projectile spawner.

use bevy::math::DVec3;
use bevy::prelude::*;

use super::types::*;
use crate::simulation::components::*;
use crate::utils::constants::*;

/// Spawns a physical guided bombardment projectile on an intercept corridor targeting a celestial world.
pub fn launch_targeted_bombardment(
    commands: &mut Commands,
    target_entity: Entity,
    target_pos: DVec3,
    target_vel: DVec3,
    _target_mass_solar: f64,
    target_radius_au: f64,
    target_name: &str,
    bombardment_type: BombardmentType,
    sim_time_years: f64,
) -> Entity {
    let (mass_solar, rad_au, comp, body_type, proj_name) = match bombardment_type {
        BombardmentType::IcyComet => {
            let mut c = Composition::icy();
            c.ice_frac = 0.60;
            c.organics_frac = 0.20;
            c.silicate_frac = 0.20;
            (
                0.000_40 * EARTH_MASS_SOLAR,
                EARTH_RADIUS_AU * 0.05,
                c,
                BodyType::Comet,
                format!("Guided-Comet -> {target_name}"),
            )
        }
        BombardmentType::CarbonaceousChondrite => {
            let mut c = Composition::carbonaceous();
            c.organics_frac = 0.35;
            c.ice_frac = 0.25;
            c.silicate_frac = 0.40;
            (
                0.000_35 * EARTH_MASS_SOLAR,
                EARTH_RADIUS_AU * 0.06,
                c,
                BodyType::Asteroid,
                format!("Chondrite -> {target_name}"),
            )
        }
        BombardmentType::VolatileAblationSalvo => {
            let mut c = Composition::icy();
            c.ice_frac = 0.50;
            c.organics_frac = 0.30;
            c.gas_frac = 0.10;
            (
                0.000_25 * EARTH_MASS_SOLAR,
                EARTH_RADIUS_AU * 0.04,
                c,
                BodyType::Comet,
                format!("Volatile-Salvo -> {target_name}"),
            )
        }
        BombardmentType::IronAsteroid => {
            let mut c = Composition::metal_rich();
            c.metal_frac = 0.75;
            c.silicate_frac = 0.25;
            (
                0.000_80 * EARTH_MASS_SOLAR,
                EARTH_RADIUS_AU * 0.07,
                c,
                BodyType::Asteroid,
                format!("Core-Impactor -> {target_name}"),
            )
        }
    };

    // Standoff approach vector outside the planetary radius
    let standoff_dist = (target_radius_au * 20.0).clamp(0.008, 0.045);
    let mut approach_dir = if target_vel.length_squared() > 1e-6 {
        let v_norm = target_vel.normalize();
        let radial = target_pos.normalize_or_zero();
        (radial * 0.75 + v_norm * 0.45 + DVec3::Y * 0.25).normalize()
    } else {
        DVec3::new(0.85, 0.25, 0.45).normalize()
    };
    if approach_dir.length_squared() < 1e-6 {
        approach_dir = DVec3::new(1.0, 0.2, 0.0).normalize();
    }

    let spawn_pos = target_pos + approach_dir * standoff_dist;

    // Flight time ~0.0035 years (~30 sim hours) for a satisfying, visible approach
    let dt_flight = 0.0035f64;
    let inbound_velocity = -approach_dir * (standoff_dist / dt_flight);
    let spawn_vel = target_vel + inbound_velocity;

    commands
        .spawn((
            CelestialBody {
                body_type,
                name: proj_name,
            },
            SimPosition(spawn_pos),
            SimVelocity(spawn_vel),
            SimAcceleration::default(),
            Mass(mass_solar),
            Radius(rad_au),
            Temperature(120.0),
            comp,
            BombardmentProjectile {
                target_entity,
                bombardment_type,
                launch_time_yr: sim_time_years,
                expected_arrival_yr: sim_time_years + dt_flight,
                trail_color: bombardment_type.color(),
                is_detonated: false,
            },
        ))
        .id()
}
