//! Automated planetesimal streaming instability spawner, gas disk dissipation, and delayed Proto-Earth spawner.

use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::spawner::sample_disk_radius;

/// Resource tracking the automated planetesimal spawning state.
/// Models the Streaming Instability mechanism where dust grains settle to the
/// disk midplane, concentrate into filaments, and gravitationally collapse into
/// kilometer-scale planetesimals.
#[derive(Resource, Debug, Clone)]
pub struct PlanetesimalSpawner {
    /// Simulation time (in years) when the last planetesimal was spawned.
    pub last_spawn_yr: f64,
    /// Total number of planetesimals auto-spawned so far.
    pub total_spawned: u32,
    /// Counter for generating unique names.
    pub name_counter: u32,
    /// Cap on total ECS bodies to maintain N-body performance.
    pub max_ecs_bodies: u32,
}

impl Default for PlanetesimalSpawner {
    fn default() -> Self {
        Self {
            last_spawn_yr: 0.0,
            total_spawned: 0,
            name_counter: 0,
            max_ecs_bodies: 1024,
        }
    }
}

fn sample_early_massive_disk_niche(spawned_count: u32, rng: &mut impl Rng) -> (f64, Composition) {
    match spawned_count {
        0 => (rng.random_range(72.0..88.0), Composition::solar_gas()),
        1 => (rng.random_range(92.0..112.0), Composition::pure_hydrogen()),
        2 => (rng.random_range(118.0..142.0), Composition::solar_gas()),
        3 => (rng.random_range(148.0..178.0), Composition::pure_hydrogen()),
        4 => (rng.random_range(185.0..215.0), Composition::solar_gas()),
        5 => (rng.random_range(220.0..245.0), Composition::pure_hydrogen()),
        6 => (rng.random_range(80.0..130.0), Composition::icy()),
        _ => (rng.random_range(140.0..220.0), Composition::solar_gas()),
    }
}

fn sample_early_solar_niche(spawned_count: u32, rng: &mut impl Rng) -> (f64, Composition) {
    match spawned_count {
        0 => (rng.random_range(0.38..0.72), Composition::rocky()),
        1 => (rng.random_range(0.95..1.52), Composition::rocky()),
        2 => (rng.random_range(2.4..3.6), Composition::carbonaceous()),
        3 => (rng.random_range(5.0..6.2), Composition::icy()),
        4 => (rng.random_range(8.8..10.5), Composition::icy()),
        5 => (rng.random_range(18.0..22.0), Composition::icy()),
        6 => (rng.random_range(28.0..32.0), Composition::icy()),
        _ => (rng.random_range(36.0..45.0), Composition::icy()),
    }
}

fn sample_standard_belt_or_feeding_zone(
    disk_params: &DiskParameters,
    rng: &mut impl Rng,
) -> (f64, Composition, bool) {
    let roll: f64 = rng.random_range(0.0..1.0);
    if roll < 0.15 {
        let zone_roll: f64 = rng.random_range(0.0..1.0);
        let (r_zone, comp_zone) = if zone_roll < 0.35 {
            (rng.random_range(0.7..1.8), Composition::rocky())
        } else if zone_roll < 0.65 {
            (rng.random_range(4.5..6.5), Composition::icy())
        } else if zone_roll < 0.85 {
            (rng.random_range(8.5..11.5), Composition::icy())
        } else {
            (rng.random_range(18.0..32.0), Composition::icy())
        };
        (r_zone, comp_zone, true)
    } else if roll < 0.60 {
        let r_belt = rng.random_range(2.15..3.45);
        let comp_belt = if rng.random_bool(0.7) {
            Composition::carbonaceous()
        } else {
            Composition::rocky()
        };
        (r_belt, comp_belt, false)
    } else if roll < 0.95 {
        let r_kuiper = rng.random_range(16.0..42.0);
        (r_kuiper, Composition::icy(), false)
    } else {
        let (r_samp, comp_samp) = sample_disk_radius(rng, disk_params);
        (r_samp, comp_samp, false)
    }
}

fn sample_planetesimal_location(
    spawner_count: u32,
    is_massive_disk: bool,
    disk_params: &DiskParameters,
    rng: &mut impl Rng,
) -> (f64, Composition, bool) {
    if (spawner_count as usize) < 8 {
        if is_massive_disk {
            let (r, c) = sample_early_massive_disk_niche(spawner_count, rng);
            (r, c, true)
        } else {
            let (r, c) = sample_early_solar_niche(spawner_count, rng);
            (r, c, true)
        }
    } else if !is_massive_disk {
        sample_standard_belt_or_feeding_zone(disk_params, rng)
    } else {
        let (r_samp, comp_samp) = sample_disk_radius(rng, disk_params);
        (r_samp, comp_samp, false)
    }
}

fn spawn_single_planetesimal(
    commands: &mut Commands,
    disk_params: &DiskParameters,
    spawner: &mut PlanetesimalSpawner,
    r: f64,
    comp: Composition,
    is_feeding_zone: bool,
    is_massive_disk: bool,
    rng: &mut impl Rng,
) {
    let phi: f64 = rng.random_range(0.0..2.0 * PI);
    let inclination: f64 = rng.random_range(-0.03..0.03);
    let y_offset = r * inclination;
    let pos = DVec3::new(r * phi.cos(), y_offset, r * phi.sin());

    let v_k = (G_ASTRO * disk_params.central_star_mass / r).sqrt();
    let ecc_kick: f64 = rng.random_range(0.98..1.02);
    let v_mag = v_k * ecc_kick;
    let vel = DVec3::new(-v_mag * phi.sin(), 0.0, v_mag * phi.cos());

    let is_protoplanet: bool = if (spawner.total_spawned as usize) < 8 || is_feeding_zone {
        true
    } else {
        rng.random_bool(0.35)
    };
    let log_mass_earth: f64 = if is_protoplanet {
        if is_massive_disk {
            rng.random_range(0.0..1.7)
        } else if r > disk_params.snow_line_au {
            rng.random_range(-1.2..-0.6)
        } else {
            rng.random_range(-1.7..-1.0)
        }
    } else {
        rng.random_range(-3.5..-2.1)
    };
    let mass = EARTH_MASS_SOLAR * 10.0_f64.powf(log_mass_earth);

    let density = comp.average_density();
    let volume = mass / density;
    let phys_radius = ((3.0 * volume) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.05);

    let temp = disk_params.reference_temp_1au * (r / 1.0).powf(-0.5);

    let body_type = if mass >= EARTH_MASS_SOLAR * 0.005 || is_feeding_zone {
        BodyType::Protoplanet
    } else if (2.0..=3.8).contains(&r) {
        BodyType::Asteroid
    } else if r >= 15.0 || comp.ice_frac > 0.35 {
        BodyType::Comet
    } else {
        BodyType::Planetesimal
    };

    spawner.name_counter += 1;
    let name = match body_type {
        BodyType::Protoplanet => format!("Embryo #{}", spawner.name_counter),
        BodyType::Asteroid => format!("Asteroid #{}", spawner.name_counter),
        BodyType::Comet => format!("Comet #{}", spawner.name_counter),
        _ => format!("Planetesimal #{}", spawner.name_counter),
    };

    let mut diff = InternalDifferentiation::default();
    diff.recalculate(mass, phys_radius, &comp);

    let mut spin = SpinState::default();
    let spin_period_hrs: f64 = rng.random_range(4.0..48.0);
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
}

/// Automatically spawns planetesimals from the particle field via Streaming Instability.
pub fn auto_spawn_planetesimals(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    _config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    mut spawner: ResMut<PlanetesimalSpawner>,
    body_count: Query<Entity, With<CelestialBody>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let t = sim_time.elapsed_years;
    if t > disk_params.gas_disk_lifetime_yr {
        return;
    }

    let current_body_count = body_count.iter().count() as u32;
    if current_body_count >= spawner.max_ecs_bodies {
        return;
    }

    let base_interval_yr = 350.0;
    let tau_depletion = disk_params.gas_disk_lifetime_yr / 4.0;
    let spawn_interval = base_interval_yr * (t / tau_depletion).exp();

    let time_since_last = t - spawner.last_spawn_yr;
    if time_since_last < spawn_interval {
        return;
    }

    let mut rng = rand::rng();
    let is_massive_disk =
        disk_params.central_star_mass > 10.0 || disk_params.outer_radius_au > 100.0;

    let (r, comp, is_feeding_zone) = sample_planetesimal_location(
        spawner.total_spawned,
        is_massive_disk,
        &disk_params,
        &mut rng,
    );

    spawn_single_planetesimal(
        &mut commands,
        &disk_params,
        &mut spawner,
        r,
        comp,
        is_feeding_zone,
        is_massive_disk,
        &mut rng,
    );

    spawner.last_spawn_yr = (spawner.last_spawn_yr + spawn_interval).min(t);
    spawner.total_spawned += 1;
}

/// Gradually dissipates the protoplanetary gas disk and dust particles as the
/// protostar blows them away via radiation pressure and stellar winds (T-Tauri phase).
pub fn dissipate_gas_disk(
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    disk_params: Res<DiskParameters>,
    mut config: ResMut<SimulationConfig>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let t = sim_time.elapsed_years;
    let lifetime = disk_params.gas_disk_lifetime_yr;

    if t >= lifetime {
        config.gas_density_scale = 0.0;
        config.active_particles = 0;
        return;
    }

    let decay_constant = -4.605 / lifetime;
    config.gas_density_scale = (decay_constant * t).exp() as f32;

    let particle_start_decay = lifetime * 0.5;
    if t > particle_start_decay {
        let decay_progress = (t - particle_start_decay) / (lifetime - particle_start_decay);
        let remaining_frac = (1.0 - decay_progress).clamp(0.0, 1.0) as f32;
        config.active_particles = (config.target_particle_count as f32 * remaining_frac) as u32;
    } else {
        config.active_particles = config.target_particle_count as u32;
    }
}

/// Delayed spawner for Proto-Earth.
pub fn auto_spawn_delayed_proto_earth(
    mut spawned: Local<bool>,
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    disk_params: Res<DiskParameters>,
    query: Query<&CelestialBody>,
) {
    if *spawned {
        return;
    }

    let exists = query.iter().any(|b| {
        b.name == "Proto-Earth"
            || b.name == "Earth"
            || b.name.contains("Proto-Earth")
            || b.name.contains("1AU")
            || b.name.contains("Earth")
    });
    if exists {
        *spawned = true;
        return;
    }

    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    if sim_time.elapsed_years < 15.0 {
        return;
    }

    *spawned = true;

    let r_au = 1.00;
    let mass_s = 0.55 * EARTH_MASS_SOLAR;
    let rad_au = EARTH_RADIUS_AU * 0.85;
    let comp = Composition::rocky();
    let v_circ = (G_ASTRO * disk_params.central_star_mass / r_au).sqrt();
    let pos = DVec3::new(r_au, 0.0, 0.0);
    let vel = DVec3::new(0.0, 0.0, v_circ);

    let mut diff = InternalDifferentiation::default();
    diff.recalculate(mass_s, rad_au, &comp);

    let temp_k = disk_params.reference_temp_1au;

    commands.spawn((
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration(DVec3::ZERO),
        Mass(mass_s),
        Radius(rad_au),
        Temperature(temp_k),
        comp,
        diff,
        CelestialBody {
            name: "Proto-Earth".to_string(),
            body_type: BodyType::Protoplanet,
        },
        VolatileInventory {
            delivered_water_m_earth: 0.00005,
            cometary_impact_count: 0,
            ocean_coverage_frac: 0.05,
            atmospheric_pressure_bar: 0.20,
        },
        SpinState {
            spin_vector: DVec3::new(0.0, 1e-12, 0.0),
            rotation_period_hours: 24.0,
            axial_tilt_degrees: 0.0,
        },
    ));

    info!(
        "🌍 Proto-Earth spawned at 1.00 AU at T + {:.1} yr into cleared circumstellar disk.",
        sim_time.elapsed_years
    );
}
