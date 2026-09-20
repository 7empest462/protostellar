//! Pebble accretion onto planetary cores, asteroids, and comets (Ormel/Klahr–style continuum model).
//!
//! cm–m aerodynamic solids drift inward through sub-Keplerian gas. Cores sweep them
//! up much faster than planetesimal collisions alone. When pebble concentration
//! exceeds the streaming instability threshold (especially at the water ice snow line),
//! pebble filaments collapse directly into asteroids and comets.

use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::disk::planetesimals::PlanetesimalSpawner;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Metallicity-like solid-to-gas ratio for the ambient pebble reservoir.
const PEBBLE_Z: f64 = 0.015;

/// Game-tuned multiplier so growth is visible under time warp without
/// finishing the whole disk in minutes of play.
const PEBBLE_GAME_SCALE: f64 = 0.35;

/// Disk aspect ratio h = H/r.
pub fn aspect_ratio(r_au: f64) -> f64 {
    (0.05 * (r_au / 1.0).powf(0.25)).clamp(0.03, 0.10)
}

/// Local gas surface-density scale (relative), Hayashi-like.
pub fn local_gas_sigma(r_au: f64, gas_density_scale: f64) -> f64 {
    if gas_density_scale <= 1e-4 || r_au < 0.05 {
        return 0.0;
    }
    (r_au / 1.0).powf(-1.50).max(0.0) * gas_density_scale
}

/// Radial drift velocity of mm-to-cm pebbles due to sub-Keplerian gas headwind [AU / yr].
pub fn pebble_drift_velocity_au_yr(r_au: f64, star_mass: f64) -> f64 {
    let h = aspect_ratio(r_au);
    let eta = 0.5 * h * h * 3.25;
    let v_k = (G_ASTRO * star_mass / r_au.max(0.01)).sqrt();
    let stokes = 0.05;
    let drift_factor = (2.0 * stokes) / (1.0 + stokes * stokes);
    -eta * v_k * drift_factor
}

/// Silicate fragmentation barrier inside the water ice snow line.
/// Dry silicate dust pebbles fragment at ~1 m/s collision speed, whereas sticky icy
/// pebbles beyond the snow line withstand ~10 m/s before fragmenting.
/// Consequently, the inner disk pebble reservoir is fragmented to tiny sub-millimeter grains
/// with small Stokes numbers (St << 0.01), suppressing pebble surface density Σ_peb by ~85%.
pub fn silicate_fragmentation_barrier(r_au: f64, snow_line_au: f64) -> f64 {
    if r_au >= snow_line_au {
        1.0
    } else {
        let dr = (snow_line_au - r_au).max(0.0);
        let suppression = (-dr / 0.45).exp() * 0.85 + 0.15;
        suppression.clamp(0.12, 1.0)
    }
}

/// Pebble surface density Σ_peb ∝ Z Σ_gas with cold-finger vapor condensation boost at the snow line
/// and dry silicate fragmentation suppression inside the snow line.
pub fn local_pebble_sigma(r_au: f64, gas_density_scale: f64, snow_line_au: f64) -> f64 {
    let base_sigma = PEBBLE_Z * local_gas_sigma(r_au, gas_density_scale);
    if base_sigma <= 1e-16 {
        return 0.0;
    }
    // Snow line cold finger effect: outward-diffusing vapor recondenses onto icy pebbles,
    // creating a ~2.5x traffic-jam pile-up peaking at the ice line (e.g. 2.7 AU).
    let dr = r_au - snow_line_au;
    let trap_factor = 1.0 + 2.5 * (-dr * dr / (2.0 * 0.35 * 0.35)).exp();
    let frag_factor = silicate_fragmentation_barrier(r_au, snow_line_au);
    base_sigma * trap_factor * frag_factor
}

/// Pebble isolation mass (Lambrechts–Johansen scaling modified for terrestrial and asteroid belts).
/// Outside the snow line: M_iso / M_⊕ ≈ 25 (h/0.05)^3.
/// Inside the snow line: dry aerodynamic coupling and pressure deflections cap pebble isolation
/// mass to ~0.85 M_earth, preventing terrestrial cores from runaway growth into super-Earths.
/// In the Main Asteroid Belt (2.1 - 3.4 AU): Jupiter perturbations & high encounter speeds
/// cap growth to Ceres-class planetesimals (~0.002 M_earth).
pub fn pebble_isolation_mass_solar(r_au: f64) -> f64 {
    let h = aspect_ratio(r_au);
    if (2.10..=3.45).contains(&r_au) {
        // Asteroid belt: Jupiter Kirkwood resonance perturbation barrier
        0.002 * EARTH_MASS_SOLAR
    } else if r_au < 2.0 {
        // Inner terrestrial zone: dry silicate pebble stall
        0.85 * EARTH_MASS_SOLAR
    } else {
        // Outer giant core formation zone
        let m_earth = 25.0 * (h / 0.05).powi(3);
        m_earth * EARTH_MASS_SOLAR
    }
}

/// Returns true if a body can sweep up drifting pebbles.
pub fn is_pebble_target(body_type: BodyType) -> bool {
    matches!(
        body_type,
        BodyType::Asteroid
            | BodyType::Comet
            | BodyType::Planetesimal
            | BodyType::Protoplanet
            | BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::IceGiant
            | BodyType::GasGiant
    )
}

/// Stokes-ish accretion efficiency: minor bodies (asteroids/comets) in 3D Bondi sweep,
/// larger cores approach 2D Hill sweep.
pub fn accretion_efficiency(mass_solar: f64, r_au: f64, star_mass: f64) -> f64 {
    let r_hill = r_au * (mass_solar / (3.0 * star_mass).max(1e-12)).cbrt();
    let h = aspect_ratio(r_au) * r_au;
    let ratio = (r_hill / h.max(1e-8)).clamp(0.0, 10.0);
    let eta = (ratio * ratio) / (1.0 + ratio * ratio);
    eta.clamp(0.005, 1.0)
}

/// Instantaneous pebble accretion rate [M_sun / yr].
pub fn pebble_accretion_rate(
    mass_solar: f64,
    r_au: f64,
    star_mass: f64,
    gas_density_scale: f64,
    snow_line_au: f64,
    body_type: BodyType,
) -> f64 {
    if !is_pebble_target(body_type) || mass_solar < 1e-14 {
        return 0.0;
    }

    let sigma_peb = local_pebble_sigma(r_au, gas_density_scale, snow_line_au);
    if sigma_peb <= 1e-16 {
        return 0.0;
    }

    let r_hill = r_au * (mass_solar / (3.0 * star_mass).max(1e-12)).cbrt();
    let omega = (G_ASTRO * star_mass / (r_au * r_au * r_au).max(1e-12)).sqrt();
    let v_hill = omega * r_hill;

    let eps = accretion_efficiency(mass_solar, r_au, star_mass);
    let mut mdot = 2.0 * r_hill * sigma_peb * v_hill * eps;

    // Isolation: strongly suppress once past M_iso
    let m_iso = pebble_isolation_mass_solar(r_au);
    if mass_solar > m_iso {
        let over = ((mass_solar / m_iso) - 1.0).clamp(0.0, 10.0);
        mdot *= (-over * 4.0).exp() * 0.01;
    }

    // Outside snow line, higher solid ice flux
    if r_au >= snow_line_au {
        mdot *= 1.35;
    }

    mdot * PEBBLE_GAME_SCALE
}

fn update_promoted_name(name: &mut String, old_type: BodyType, new_type: BodyType) {
    if old_type == BodyType::Asteroid && new_type == BodyType::Planetesimal {
        *name = name.replace("Asteroid", "Planetesimal");
    } else if old_type == BodyType::Comet && new_type == BodyType::Planetesimal {
        *name = name.replace("Comet", "Icy Planetesimal");
    } else if (old_type == BodyType::Planetesimal || old_type == BodyType::Asteroid)
        && new_type == BodyType::Protoplanet
    {
        *name = name
            .replace("Planetesimal", "Embryo")
            .replace("Asteroid", "Embryo");
    } else if new_type == BodyType::TerrestrialPlanet {
        *name = format!("Planet ({name})");
    }
}

/// Bevy system: grow cores, planetesimals, asteroids, and comets by sweeping pebbles.
#[allow(
    clippy::type_complexity,
    reason = "Bevy Query mirrors direct_nebular_gas_accretion"
)]
pub fn apply_pebble_accretion(
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    disk_params: Res<DiskParameters>,
    mut bodies_query: Query<
        (
            &mut Mass,
            &SimPosition,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
            Option<&mut InternalDifferentiation>,
        ),
        (
            Without<CentralStar>,
            Without<crate::simulation::terraforming::BombardmentProjectile>,
        ),
    >,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let gas_scale = f64::from(config.gas_density_scale);
    if gas_scale <= 0.001 || sim_time.elapsed_years > disk_params.gas_disk_lifetime_yr {
        return;
    }

    let dt_yr = (config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)).min(50.0);
    let star_mass = disk_params.central_star_mass;
    let snow = disk_params.snow_line_au;

    for (mut mass, pos, mut rad, mut comp, mut body, mut opt_diff) in bodies_query.iter_mut() {
        if !is_pebble_target(body.body_type) {
            continue;
        }

        let r_au = pos.0.length();
        if r_au < disk_params.inner_radius_au || r_au > disk_params.outer_radius_au {
            continue;
        }

        let mdot = pebble_accretion_rate(mass.0, r_au, star_mass, gas_scale, snow, body.body_type);
        if mdot <= 0.0 {
            continue;
        }

        let dm = mdot * dt_yr;
        if dm < 1e-16 {
            continue;
        }

        let new_mass = mass.0 + dm;
        mass.0 = new_mass;

        if r_au >= snow {
            let ice_add = 0.55;
            let rock_add = 0.35;
            let metal_add = 0.10;
            let total = ice_add + rock_add + metal_add;
            let w_old = mass.0 - dm;
            let w_new = dm;
            comp.ice_frac = (comp.ice_frac * w_old + ice_add / total * w_new) / new_mass;
            comp.silicate_frac = (comp.silicate_frac * w_old + rock_add / total * w_new) / new_mass;
            comp.metal_frac = (comp.metal_frac * w_old + metal_add / total * w_new) / new_mass;
        } else {
            let rock_add = 0.70;
            let metal_add = 0.30;
            let w_old = mass.0 - dm;
            let w_new = dm;
            comp.silicate_frac = (comp.silicate_frac * w_old + rock_add * w_new) / new_mass;
            comp.metal_frac = (comp.metal_frac * w_old + metal_add * w_new) / new_mass;
        }

        let sum = comp.ice_frac
            + comp.silicate_frac
            + comp.metal_frac
            + comp.organics_frac
            + comp.gas_frac;
        if sum > 1e-12 {
            comp.ice_frac /= sum;
            comp.silicate_frac /= sum;
            comp.metal_frac /= sum;
            comp.organics_frac /= sum;
            comp.gas_frac /= sum;
        }

        let density = comp.average_density();
        let physical_radius = ((3.0 * new_mass / density.max(1e-30)) / (4.0 * PI)).cbrt();
        let new_radius = match body.body_type {
            BodyType::Protoplanet
            | BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::GasGiant
            | BodyType::IceGiant => physical_radius.max(EARTH_RADIUS_AU * 0.05),
            _ => physical_radius.max(1e-8),
        };
        rad.0 = new_radius;

        let new_type = classify_body_by_mass_and_comp(new_mass, &comp, false);
        if new_type != body.body_type {
            let old_type = body.body_type;
            body.body_type = new_type;
            update_promoted_name(&mut body.name, old_type, new_type);
        }

        if let Some(ref mut diff) = opt_diff {
            diff.recalculate(new_mass, new_radius, &comp);
        }
    }
}

fn spawn_single_filament_minor_body(
    commands: &mut Commands,
    disk_params: &DiskParameters,
    spawner: &mut PlanetesimalSpawner,
    rng: &mut impl Rng,
) {
    let snow = disk_params.snow_line_au;
    let niche_roll: f64 = rng.random_range(0.0..1.0);
    let (r, comp, body_type, log_mass) = if niche_roll < 0.60 {
        let r_ast = rng.random_range(2.10..3.45);
        let comp_ast = if rng.random_bool(0.75) {
            Composition::carbonaceous()
        } else {
            Composition::rocky()
        };
        (
            r_ast,
            comp_ast,
            BodyType::Asteroid,
            rng.random_range(-5.5..-3.8),
        )
    } else if niche_roll < 0.95 {
        let r_comet = rng.random_range(16.0..45.0);
        let comp_comet = Composition::icy();
        (
            r_comet,
            comp_comet,
            BodyType::Comet,
            rng.random_range(-6.0..-4.2),
        )
    } else {
        let r_trap = rng.random_range(snow - 0.2..snow + 0.3);
        let comp_trap = Composition {
            silicate_frac: 0.45,
            ice_frac: 0.45,
            metal_frac: 0.10,
            gas_frac: 0.0,
            organics_frac: 0.0,
        };
        (
            r_trap,
            comp_trap,
            BodyType::Planetesimal,
            rng.random_range(-3.8..-2.8),
        )
    };

    let mass = EARTH_MASS_SOLAR * 10.0_f64.powf(log_mass);
    let density = comp.average_density();
    let volume = mass / density;
    let phys_radius = ((3.0 * volume) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.04);

    let phi: f64 = rng.random_range(0.0..2.0 * PI);
    let inc: f64 = rng.random_range(-0.04..0.04);
    let pos = DVec3::new(r * phi.cos(), r * inc, r * phi.sin());

    let v_k = (G_ASTRO * disk_params.central_star_mass / r).sqrt();
    let ecc_kick: f64 = rng.random_range(0.985..1.015);
    let v_mag = v_k * ecc_kick;
    let vel = DVec3::new(-v_mag * phi.sin(), 0.0, v_mag * phi.cos());

    let temp = disk_params.reference_temp_1au * (r / 1.0).powf(-0.5);

    spawner.name_counter += 1;
    let name = match body_type {
        BodyType::Asteroid => format!("Asteroid #{}", spawner.name_counter),
        BodyType::Comet => format!("Comet #{}", spawner.name_counter),
        _ => format!("Planetesimal #{}", spawner.name_counter),
    };

    let mut diff = InternalDifferentiation::default();
    diff.recalculate(mass, phys_radius, &comp);

    let mut spin = SpinState::default();
    let spin_period_hrs: f64 = rng.random_range(6.0..36.0);
    let omega = 2.0 * PI / (spin_period_hrs * 3600.0 / YEAR_SECONDS);
    let initial_spin = 0.33 * mass * phys_radius * phys_radius * DVec3::new(0.0, omega, 0.0);
    spin.update_from_spin(initial_spin, mass, phys_radius);

    commands.spawn((
        CelestialBody { body_type, name },
        Mass(mass),
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration::default(),
        Radius(phys_radius),
        Temperature(temp),
        Luminosity(0.0),
        AngularMomentum(pos.cross(vel) * mass),
        comp,
        diff,
        spin,
    ));

    spawner.total_spawned += 1;
}

/// Spawns Asteroids, Comets, and Planetesimals from streaming instability pebble filaments.
pub fn spawn_streaming_instability_minor_bodies(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    mut spawner: ResMut<PlanetesimalSpawner>,
    body_count: Query<Entity, With<CelestialBody>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let t = sim_time.elapsed_years;
    if t > disk_params.gas_disk_lifetime_yr || (config.gas_density_scale <= 0.001 && t > 2500.0) {
        return;
    }

    let current_bodies = body_count.iter().count() as u32;
    if current_bodies >= spawner.max_ecs_bodies {
        return;
    }
    let deficit = spawner.max_ecs_bodies.saturating_sub(current_bodies);

    let (spawn_interval, min_burst, max_burst, max_bursts) = if deficit > 500 {
        (35.0, 8, 16, 12usize)
    } else if deficit > 100 {
        (75.0, 5, 10, 8usize)
    } else if deficit > 50 {
        (150.0, 4, 8, 4usize)
    } else {
        (300.0, 3, 6, 1usize)
    };

    let dt = t - spawner.last_minor_body_spawn_yr;
    if dt < spawn_interval {
        return;
    }

    let num_bursts = ((dt / spawn_interval).floor() as usize).clamp(1, max_bursts);
    let mut rng = rand::rng();
    let mut spawned_total = 0u32;

    for _ in 0..num_bursts {
        let burst_count = rng.random_range(min_burst..=max_burst);
        for _ in 0..burst_count {
            if current_bodies + spawned_total >= spawner.max_ecs_bodies {
                break;
            }
            spawn_single_filament_minor_body(&mut commands, &disk_params, &mut spawner, &mut rng);
            spawned_total += 1;
        }
        if current_bodies + spawned_total >= spawner.max_ecs_bodies {
            break;
        }
    }

    if spawned_total > 0 {
        spawner.last_minor_body_spawn_yr =
            (spawner.last_minor_body_spawn_yr + (num_bursts as f64) * spawn_interval).min(t);
    }
}
