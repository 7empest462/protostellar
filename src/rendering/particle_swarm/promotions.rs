use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::disk::sample_disk_radius;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::ParticleSwarmData;

pub fn check_clump_promotions(
    data: &mut ParticleSwarmData,
    disk_params: &DiskParameters,
    current_ecs_count: usize,
    is_massive_disk: bool,
) -> (Vec<(DVec3, DVec3, f64, f64, Composition)>, u32) {
    let n = data.count;
    let b_mass = data.base_mass;
    let max_ecs_bodies = if is_massive_disk { 32 } else { 1024 };
    let promo_threshold = if is_massive_disk {
        (64.0 * b_mass).max(0.5 * JUPITER_MASS_SOLAR as f32)
    } else {
        4.0 * b_mass
    };
    let mut promotions: Vec<(DVec3, DVec3, f64, f64, Composition)> = Vec::new();
    let mut active_count = 0u32;

    for i in 0..n {
        let Some(&m) = data.masses.get(i) else {
            continue;
        };
        if m > 0.0 {
            active_count += 1;
        }
        if m >= promo_threshold && promotions.is_empty() && current_ecs_count < max_ecs_bodies {
            let Some(&pos) = data.positions.get(i) else {
                continue;
            };
            let r_sq = pos[0] * pos[0] + pos[2] * pos[2];
            let r = r_sq.sqrt();
            let min_r = if is_massive_disk { 65.0 } else { 0.15 };
            let max_r = if is_massive_disk {
                disk_params.outer_radius_au as f32
            } else {
                45.0
            };
            if r < min_r || r > max_r || pos[1] < -1000.0 {
                if let Some(m_slot) = data.masses.get_mut(i) {
                    *m_slot = 0.0;
                }
                if let Some(p_slot) = data.positions.get_mut(i) {
                    *p_slot = [0.0, -5000.0, 0.0];
                }
                continue;
            }

            let m_f64 = f64::from(m);
            let Some(&comp_val) = data.compositions.get(i) else {
                continue;
            };
            let avg_density = comp_val.average_density();
            let rad_f64 = ((3.0 * m_f64 / avg_density) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.2);

            let Some(&vel) = data.velocities.get(i) else {
                continue;
            };
            promotions.push((
                DVec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])),
                DVec3::new(f64::from(vel[0]), f64::from(vel[1]), f64::from(vel[2])),
                m_f64,
                rad_f64,
                comp_val,
            ));

            if let Some(m_slot) = data.masses.get_mut(i) {
                *m_slot = 0.0;
            }
            if let Some(p_slot) = data.positions.get_mut(i) {
                *p_slot = [0.0, -5000.0, 0.0];
            }
            active_count = active_count.saturating_sub(1);
        }
    }

    (promotions, active_count)
}

fn determine_promoted_body_type_and_name(
    mass: f64,
    radius_au: f64,
    comp: &Composition,
    is_massive_disk: bool,
) -> (BodyType, String) {
    let in_feeding_zone = (0.5..=1.8).contains(&radius_au)
        || (4.5..=7.0).contains(&radius_au)
        || (8.0..=12.0).contains(&radius_au)
        || (18.0..=32.0).contains(&radius_au);

    let body_type = if is_massive_disk {
        crate::simulation::components::classify_body_by_mass_and_comp(mass, comp, false)
    } else if mass >= EARTH_MASS_SOLAR * 0.005
        || (in_feeding_zone && mass >= EARTH_MASS_SOLAR * 0.0005)
    {
        BodyType::Protoplanet
    } else if (2.0..=3.8).contains(&radius_au) {
        BodyType::Asteroid
    } else if radius_au >= 15.0 || comp.ice_frac > 0.35 {
        BodyType::Comet
    } else if radius_au < 2.0 {
        BodyType::Asteroid
    } else {
        crate::simulation::components::classify_body_by_mass_and_comp(mass, comp, false)
    };

    let name = match body_type {
        BodyType::BrownDwarf => format!("Brown Dwarf ({:.1} M_J)", mass / JUPITER_MASS_SOLAR),
        BodyType::GasGiant => format!("Planet-{radius_au:.0}AU (Gas Giant)"),
        BodyType::IceGiant => format!("Planet-{radius_au:.0}AU (Ice Giant)"),
        BodyType::SuperEarth => format!("Planet-{radius_au:.0}AU (Super-Earth)"),
        BodyType::TerrestrialPlanet => format!("Planet-{radius_au:.0}AU (Terrestrial)"),
        BodyType::Protoplanet => format!("Embryo-{radius_au:.1}AU"),
        BodyType::Planetesimal => format!("Planetesimal-{radius_au:.1}AU"),
        BodyType::Comet => format!("Comet-{radius_au:.1}AU"),
        _ => format!("Asteroid-{radius_au:.1}AU"),
    };

    (body_type, name)
}

pub fn spawn_promoted_bodies(
    commands: &mut Commands,
    promotions: Vec<(DVec3, DVec3, f64, f64, Composition)>,
    disk_params: &DiskParameters,
    is_massive_disk: bool,
) {
    for (pos, vel, mass, radius, comp) in promotions {
        let r_dist = pos.length();
        let (body_type, name) =
            determine_promoted_body_type_and_name(mass, r_dist, &comp, is_massive_disk);
        let temp = (disk_params.reference_temp_1au) * (r_dist / 1.0).powf(-0.5);

        let mut diff = InternalDifferentiation::default();
        diff.recalculate(mass, radius, &comp);

        let mut spin = SpinState::default();
        let initial_spin =
            (mass * radius * radius * 0.33) * DVec3::new(0.0, 2.0 * PI / (24.0 / 8766.0), 0.0);
        spin.update_from_spin(initial_spin, mass, radius);

        let vol = VolatileInventory {
            delivered_water_m_earth: 0.0,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: if r_dist < 2.7 { 0.5 } else { 0.0 },
            cometary_impact_count: 0,
        };

        commands.spawn((
            CelestialBody { body_type, name },
            Mass(mass),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(radius),
            Temperature(temp),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * mass),
            comp,
            diff,
            spin,
            vol,
        ));
    }
}

pub fn replenish_cleared_particles(
    data: &mut ParticleSwarmData,
    disk_params: &DiskParameters,
    config: &mut SimulationConfig,
    ignition_is_ignited: bool,
    speed_mult: f32,
    mut active_count: u32,
) {
    let max_allowed_particles = if ignition_is_ignited || config.gas_density_scale <= 0.05 {
        0u32
    } else {
        (config.target_particle_count as f32 * config.gas_density_scale.powf(1.5)) as u32
    };

    if config.gas_density_scale > 0.15
        && !ignition_is_ignited
        && active_count < max_allowed_particles
    {
        let missing = max_allowed_particles.saturating_sub(active_count);
        let mut rng = rand::rng();
        let mut replenished = 0u32;
        let star_mass_f64 = disk_params.central_star_mass;
        let replenish_scale = 1.0 + (speed_mult.log10().max(0.0) * 1.25);
        let max_replenish = missing.min((128.0 * replenish_scale) as u32);
        let n = data.count;
        let b_mass = data.base_mass;

        for i in 0..n {
            let Some(&m) = data.masses.get(i) else {
                continue;
            };
            if m <= 0.0 && replenished < max_replenish {
                let (r_spawn, comp) = sample_disk_radius(&mut rng, disk_params);
                let phi = rng.random_range(0.0..2.0 * PI);
                let v_k = (G_ASTRO * star_mass_f64 / r_spawn).sqrt();
                let v_disp = rng.random_range(-0.02..0.02) * v_k;
                let v_tot = (v_k + v_disp) as f32;

                if let Some(pos) = data.positions.get_mut(i) {
                    *pos = [
                        (r_spawn * phi.cos()) as f32,
                        rng.random_range(-0.015..0.015) as f32,
                        (r_spawn * phi.sin()) as f32,
                    ];
                }
                if let Some(vel) = data.velocities.get_mut(i) {
                    *vel = [-v_tot * phi.sin() as f32, 0.0, v_tot * phi.cos() as f32];
                }
                if let Some(mass_slot) = data.masses.get_mut(i) {
                    *mass_slot = b_mass;
                }
                if let Some(comp_slot) = data.compositions.get_mut(i) {
                    *comp_slot = comp;
                }
                if let Some(temp_slot) = data.temperatures.get_mut(i) {
                    *temp_slot =
                        (disk_params.reference_temp_1au as f32) * (r_spawn as f32 / 1.0).powf(-0.5);
                }

                replenished += 1;
                active_count += 1;
            }
        }
    }

    config.active_particles = active_count;
}
