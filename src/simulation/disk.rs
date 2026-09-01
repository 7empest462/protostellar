//! Protoplanetary disk generation based on the Hayashi Minimum Mass Solar Nebula (MMSN) model.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Spawns the initial protostellar system: only the central protostar in ECS.
/// (The 50,000 dust/planetesimal particle field is simulated in VRAM and rendered directly).
pub fn spawn_protoplanetary_disk(
    commands: &mut Commands,
    disk_params: &DiskParameters,
    _sim_config: &SimulationConfig,
) -> Entity {
    // 1. Spawn Central Star
    let protostar_mass = disk_params.central_star_mass;
    let star_radius = SOLAR_RADIUS_AU;
    let star_temp = 5778.0;

    let star_ent = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::Protostar,
                name: "The Protostar".to_string(),
            },
            CentralStar,
            Mass(protostar_mass),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(star_radius),
            Temperature(star_temp),
            Luminosity(1.0),
            AngularMomentum::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 4.0e6,
                fusion_fraction: 0.4,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id();

    // 2. Spawn Major Protoplanetary Seeds across the active disk zones
    let major_seeds = [
        (
            0.50,
            0.06 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.40,
            "Proto-Mercury",
            Composition::metal_rich(),
            BodyType::Protoplanet,
            0.05,
        ),
        (
            0.95,
            0.55 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.85,
            "Proto-Venus",
            Composition::rocky(),
            BodyType::Protoplanet,
            0.01,
        ),
        (
            1.50,
            0.65 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.90,
            "Proto-Earth",
            Composition::rocky(),
            BodyType::Protoplanet,
            0.02,
        ),
        (
            1.90,
            0.12 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.50,
            "Theia Embryo",
            Composition::rocky(),
            BodyType::Protoplanet,
            0.04,
        ),
        (
            2.60,
            0.11 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.53,
            "Proto-Mars",
            Composition::rocky(),
            BodyType::Protoplanet,
            0.07,
        ),
        (
            8.50,
            3.50 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.50,
            "Proto-Jupiter",
            Composition::solar_gas(),
            BodyType::GasGiant,
            0.03,
        ),
        (
            11.50,
            0.05 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.38,
            "Callisto Embryo",
            Composition::icy(),
            BodyType::Protoplanet,
            0.02,
        ),
        (
            15.50,
            2.20 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.25,
            "Proto-Saturn",
            Composition::solar_gas(),
            BodyType::GasGiant,
            0.04,
        ),
        (
            20.50,
            0.05 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.38,
            "Titan Embryo",
            Composition::icy(),
            BodyType::Protoplanet,
            0.03,
        ),
        (
            28.00,
            1.20 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.05,
            "Proto-Uranus",
            Composition::icy(),
            BodyType::IceGiant,
            0.05,
        ),
        (
            40.00,
            1.20 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.05,
            "Proto-Neptune",
            Composition::icy(),
            BodyType::IceGiant,
            0.02,
        ),
    ];

    // 3. Spawn Asteroid Belt Minor Planets (Silicate, Carbonaceous, Metallic)
    let asteroid_seeds = [
        (
            2.77,
            0.00015 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.18,
            "Ceres (Dwarf Planet)",
            Composition::carbonaceous(),
            BodyType::Asteroid,
            0.08,
        ),
        (
            2.36,
            0.00008 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.14,
            "Vesta (Asteroid)",
            Composition::rocky(),
            BodyType::Asteroid,
            0.09,
        ),
        (
            2.77,
            0.00007 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.14,
            "Pallas (Asteroid)",
            Composition::carbonaceous(),
            BodyType::Asteroid,
            0.23,
        ),
        (
            3.15,
            0.00004 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.12,
            "Hygiea (Asteroid)",
            Composition::carbonaceous(),
            BodyType::Asteroid,
            0.11,
        ),
        (
            2.92,
            0.00003 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.10,
            "Psyche (Metal Asteroid)",
            Composition::metal_rich(),
            BodyType::Asteroid,
            0.13,
        ),
        (
            2.21,
            0.00001 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.06,
            "Gaspra (Asteroid)",
            Composition::rocky(),
            BodyType::Asteroid,
            0.17,
        ),
        (
            2.86,
            0.00001 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.07,
            "Ida (Asteroid)",
            Composition::rocky(),
            BodyType::Asteroid,
            0.04,
        ),
        (
            2.65,
            0.00001 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.06,
            "Mathilde (Asteroid)",
            Composition::carbonaceous(),
            BodyType::Asteroid,
            0.26,
        ),
        (
            1.45,
            0.00001 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.05,
            "Eros (Near-Earth Asteroid)",
            Composition::rocky(),
            BodyType::Asteroid,
            0.22,
        ),
        (
            1.12,
            0.000005 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.03,
            "Bennu (Asteroid)",
            Composition::carbonaceous(),
            BodyType::Asteroid,
            0.20,
        ),
        (
            1.19,
            0.000005 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.03,
            "Ryugu (Asteroid)",
            Composition::carbonaceous(),
            BodyType::Asteroid,
            0.19,
        ),
        (
            1.32,
            0.000005 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.03,
            "Itokawa (Asteroid)",
            Composition::rocky(),
            BodyType::Asteroid,
            0.28,
        ),
    ];

    // 4. Spawn Kuiper Belt & Long-Period Cometary Reservoir (Pristine Volatile Ices)
    let comet_seeds = [
        (
            17.8,
            0.00002 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.08,
            "1P/Halley (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.65,
        ),
        (
            3.30,
            0.00001 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.05,
            "2P/Encke (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.60,
        ),
        (
            45.0,
            0.00003 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.10,
            "C/Hale-Bopp (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.72,
        ),
        (
            26.0,
            0.00002 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.08,
            "109P/Swift-Tuttle (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.68,
        ),
        (
            3.50,
            0.00001 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.05,
            "67P/Churyumov (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.64,
        ),
        (
            52.0,
            0.00003 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.09,
            "C/NEOWISE (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.75,
        ),
        (
            48.0,
            0.00002 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.08,
            "C/Hyakutake (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.74,
        ),
        (
            39.5,
            0.0022 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.24,
            "Pluto (Kuiper Dwarf)",
            Composition::icy(),
            BodyType::Comet,
            0.24,
        ),
        (
            67.8,
            0.0028 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.25,
            "Eris (Scattered Disk)",
            Composition::icy(),
            BodyType::Comet,
            0.44,
        ),
        (
            45.8,
            0.0018 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.20,
            "Makemake (Kuiper Dwarf)",
            Composition::icy(),
            BodyType::Comet,
            0.16,
        ),
        (
            43.3,
            0.0020 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.21,
            "Haumea (Kuiper Dwarf)",
            Composition::icy(),
            BodyType::Comet,
            0.19,
        ),
        (
            43.7,
            0.0012 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.16,
            "Quaoar (Kuiper Object)",
            Composition::icy(),
            BodyType::Comet,
            0.04,
        ),
        (
            76.0,
            0.0015 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.18,
            "Sedna (Oort Cloud)",
            Composition::icy(),
            BodyType::Comet,
            0.60,
        ),
    ];

    let mut rng = rand::rng();
    use rand::prelude::*;
    use std::f64::consts::PI;

    let all_seeds = major_seeds
        .into_iter()
        .chain(asteroid_seeds)
        .chain(comet_seeds);

    for (a, mass, radius, name, comp, body_type, ecc) in all_seeds {
        let a_f64: f64 = a;
        let ecc_f64: f64 = ecc;
        let phi: f64 = rng.random_range(0.0..2.0 * PI);
        let inc_rad: f64 = rng.random_range(-0.04..0.04);

        // At perihelion r_p = a * (1 - e)
        let r_p = a_f64 * (1.0f64 - ecc_f64).max(0.1f64);
        let pos = DVec3::new(r_p * phi.cos(), r_p * inc_rad, r_p * phi.sin());

        // Vis-viva perihelion orbital velocity: v_p = sqrt(GM/a * (1+e)/(1-e))
        let v_peri = ((G_ASTRO * protostar_mass / a_f64)
            * ((1.0f64 + ecc_f64) / (1.0f64 - ecc_f64).max(0.01f64)))
        .sqrt();
        let vel = DVec3::new(-v_peri * phi.sin(), 0.0, v_peri * phi.cos());

        let temp = disk_params.reference_temp_1au * (a_f64 / 1.0).powf(-0.5);

        let mut diff = InternalDifferentiation::default();
        diff.recalculate(mass, radius, &comp);

        let mut spin = SpinState::default();
        let spin_period_hrs: f64 = match body_type {
            BodyType::GasGiant => rng.random_range(9.0..11.0),
            BodyType::IceGiant => rng.random_range(14.0..18.0),
            _ => rng.random_range(6.0..36.0),
        };
        let omega = 2.0 * PI / (spin_period_hrs * 3600.0 / YEAR_SECONDS);
        let initial_spin = 0.33 * mass * radius * radius * DVec3::new(0.0, omega, 0.0);
        spin.update_from_spin(initial_spin, mass, radius);

        commands.spawn((
            CelestialBody {
                body_type,
                name: name.to_string(),
            },
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
        ));
    }

    star_ent
}

/// Helper function to sample initial radial position according to standard MMSN power-law.
/// Divided into 4 astrophysical zones:
/// 1. Terrestrial Rocky Zone (0.35 - 2.50 AU): ~25% of particles (silicate/metal rich)
/// 2. Snow Line & Asteroid Belt (2.50 - 4.50 AU): ~15% of particles (carbonaceous & transition ices)
/// 3. Giant Planet Accretion Reservoir (4.50 - 25.0 AU): ~45% of particles (dense icy/gas-rich cores)
/// 4. Outer Kuiper Belt (25.0 - 45.0 AU): ~15% of particles (primordial volatile ices)
pub fn sample_disk_radius<R: rand::Rng + ?Sized>(
    rng: &mut R,
    disk_params: &DiskParameters,
) -> (f64, Composition) {
    let roll: f64 = rng.random_range(0.0..1.0);

    if roll < 0.25 {
        // Zone 1: Terrestrial Rocky Zone (0.06 - 2.50 AU) - 25% of particles
        let u = roll / 0.25;
        let r_in_sq = disk_params.inner_radius_au * disk_params.inner_radius_au;
        let r_out_sq = 2.50 * 2.50;
        let r = (r_in_sq + u * (r_out_sq - r_in_sq)).sqrt();
        let comp = if r < 0.60 {
            Composition::metal_rich()
        } else {
            Composition::rocky()
        };
        (r, comp)
    } else if roll < 0.40 {
        // Zone 2: Snowline Transition & Asteroid Belt (2.50 - 4.50 AU) - 15% of particles
        let u = (roll - 0.25) / 0.15;
        let r_in_sq = 2.50 * 2.50;
        let r_out_sq = 4.50 * 4.50;
        let r = (r_in_sq + u * (r_out_sq - r_in_sq)).sqrt();
        (r, Composition::carbonaceous())
    } else if roll < 0.85 {
        // Zone 3: Giant Planet Accretion Reservoir (4.50 - 25.0 AU) - 45% of all disk mass!
        let u = (roll - 0.40) / 0.45;
        let r_in_sq = 4.50 * 4.50;
        let r_out_sq = 25.0 * 25.0;
        let r = (r_in_sq + u * (r_out_sq - r_in_sq)).sqrt();
        (r, Composition::icy())
    } else {
        // Zone 4: Outer Kuiper Belt (25.0 - 45.0 AU) - 15% of particles
        let u = (roll - 0.85) / 0.15;
        let r_in_sq = 25.0 * 25.0;
        let r_out_sq = 45.0 * 45.0;
        let r = (r_in_sq + u * (r_out_sq - r_in_sq)).sqrt();
        (r, Composition::icy())
    }
}

// ============================================================================
// Automated Planetesimal Spawner — Streaming Instability Model
// ============================================================================

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
    /// Maximum number of ECS bodies allowed (to prevent performance collapse).
    pub max_ecs_bodies: u32,
    /// Running name counter for unique naming.
    pub name_counter: u32,
}

impl Default for PlanetesimalSpawner {
    fn default() -> Self {
        Self {
            last_spawn_yr: 0.0,
            total_spawned: 0,
            max_ecs_bodies: 24, // Cap at 24 major bodies so every planet is distinct and easy to cycle
            name_counter: 0,
        }
    }
}

/// Automatically spawns planetesimals during the active protoplanetary disk phase.
///
/// Models the astrophysical streaming instability: dust settles to the midplane,
/// concentrates via aerodynamic drag, and gravitationally collapses into ~1-100 km
/// bodies (asteroid-mass). The spawning rate:
///
/// 1. **Peaks early** (~first 500k years) when the disk is richest in dust.
/// 2. **Decays exponentially** as dust gets consumed by accretion.
/// 3. **Stops entirely** when the gas disk photo-evaporates (~3.5 Myr).
/// 4. **Favors the outer disk** beyond the snow line where 4x more solid mass is available.
pub fn auto_spawn_planetesimals(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    _config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    mut spawner: ResMut<PlanetesimalSpawner>,
    body_count: Query<Entity, With<CelestialBody>>,
) {
    // Don't spawn while paused
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let t = sim_time.elapsed_years;

    // Phase gate: only spawn during the active gas disk lifetime.
    // After the gas evaporates, there's no aerodynamic concentration mechanism.
    if t > disk_params.gas_disk_lifetime_yr {
        return;
    }

    // Respect the ECS body cap to prevent N-body performance degradation
    let current_body_count = body_count.iter().count() as u32;
    if current_body_count >= spawner.max_ecs_bodies {
        return;
    }

    // --- Spawning Rate Model ---
    // Base interval: one planetesimal every ~2,000 simulated years at t=0.
    // The interval increases (rate decreases) exponentially as the disk depletes.
    //
    // τ_depletion = gas_disk_lifetime / 5 ≈ 700,000 years
    // interval(t) = base_interval * exp(t / τ_depletion)
    //
    // This gives roughly:
    //   t = 0:        one every ~2,000 yr  (rapid early accretion)
    //   t = 700k yr:  one every ~5,400 yr
    //   t = 1.4M yr:  one every ~14,800 yr
    //   t = 2.8M yr:  one every ~109,000 yr  (disk nearly exhausted)
    let base_interval_yr = 600.0;
    let tau_depletion = disk_params.gas_disk_lifetime_yr / 5.0;
    let spawn_interval = base_interval_yr * (t / tau_depletion).exp();

    let time_since_last = t - spawner.last_spawn_yr;
    if time_since_last < spawn_interval {
        return;
    }

    // --- Spawn a new planetesimal or protoplanet embryo ---
    let mut rng = rand::rng();
    use rand::prelude::*;
    use std::f64::consts::PI;

    // Use the existing astrophysical disk profile for radial placement
    let (r, comp) = sample_disk_radius(&mut rng, &disk_params);

    // Random azimuthal angle for orbital placement
    let phi: f64 = rng.random_range(0.0..2.0 * PI);

    // Slight vertical scatter (thin disk, ~1-2 degree inclination)
    let inclination: f64 = rng.random_range(-0.03..0.03); // radians
    let y_offset = r * inclination;

    let pos = DVec3::new(r * phi.cos(), y_offset, r * phi.sin());

    // Perfect Keplerian circular velocity + small random eccentricity kick (~1-5%)
    let v_k = (G_ASTRO * disk_params.central_star_mass / r).sqrt();
    let ecc_kick: f64 = rng.random_range(0.98..1.02);
    let v_mag = v_k * ecc_kick;
    let vel = DVec3::new(-v_mag * phi.sin(), 0.0, v_mag * phi.cos());

    // Bimodal mass distribution: 45% substantive Protoplanets, 55% Planetesimals
    let is_protoplanet: bool = rng.random_bool(0.45);
    let log_mass_earth: f64 = if is_protoplanet {
        rng.random_range(-2.0..-0.8) // 0.01 to 0.16 Earth masses
    } else {
        rng.random_range(-3.5..-2.1) // 0.0003 to 0.008 Earth masses
    };
    let mass = EARTH_MASS_SOLAR * 10.0_f64.powf(log_mass_earth);

    // Physical radius from mass and composition density
    let density = comp.average_density();
    let volume = mass / density;
    let phys_radius = ((3.0 * volume) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.05);

    // Temperature from distance to star
    let temp = disk_params.reference_temp_1au * (r / 1.0).powf(-0.5);

    // Determine body type from mass
    let body_type = if mass >= EARTH_MASS_SOLAR * 0.005 {
        BodyType::Protoplanet
    } else {
        BodyType::Planetesimal
    };

    // Generate unique name based on disk zone
    spawner.name_counter += 1;
    let zone_name = if r < disk_params.snow_line_au {
        "Rocky"
    } else if r < 40.0 {
        "Icy"
    } else {
        "KBO"
    };
    let type_label = if body_type == BodyType::Protoplanet {
        "Embryo"
    } else {
        "Planetesimal"
    };
    let name = format!("{} {} #{}", zone_name, type_label, spawner.name_counter);

    // Internal differentiation and spin
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

    spawner.last_spawn_yr = t;
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

    // Exponential decay for gas density (gas drag reduces significantly over time)
    // At t=0, scale = 1.0. At t=lifetime, scale ≈ 0.01.
    let decay_constant = -4.605 / lifetime; // ln(0.01) = -4.605
    config.gas_density_scale = (decay_constant * t).exp() as f32;

    // Linear decay for active particles starting from halfway through the lifetime
    let particle_start_decay = lifetime * 0.5;
    if t > particle_start_decay {
        let decay_progress = (t - particle_start_decay) / (lifetime - particle_start_decay);
        let remaining_frac = (1.0 - decay_progress).clamp(0.0, 1.0) as f32;
        config.active_particles = (config.target_particle_count as f32 * remaining_frac) as u32;
    } else {
        config.active_particles = config.target_particle_count as u32;
    }
}
