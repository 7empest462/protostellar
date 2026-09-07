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
            0.40,
            0.06 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.40,
            "Proto-Mercury",
            Composition::metal_rich(),
            BodyType::Protoplanet,
            0.05,
        ),
        (
            0.72,
            0.50 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.80,
            "Proto-Venus",
            Composition::rocky(),
            BodyType::Protoplanet,
            0.01,
        ),
        (
            1.00,
            0.50 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.82,
            "Proto-Earth",
            Composition::rocky(),
            BodyType::Protoplanet,
            0.016,
        ),
        (
            1.25,
            0.10 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.48,
            "Theia Embryo",
            Composition::rocky(),
            BodyType::Protoplanet,
            0.04,
        ),
        (
            1.52,
            0.11 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.53,
            "Proto-Mars",
            Composition::rocky(),
            BodyType::Protoplanet,
            0.07,
        ),
        (
            5.20,
            3.50 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.50,
            "Proto-Jupiter",
            Composition::solar_gas(),
            BodyType::GasGiant,
            0.03,
        ),
        (
            7.50,
            0.05 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.38,
            "Callisto Embryo",
            Composition::icy(),
            BodyType::Protoplanet,
            0.02,
        ),
        (
            9.50,
            2.20 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.25,
            "Proto-Saturn",
            Composition::solar_gas(),
            BodyType::GasGiant,
            0.04,
        ),
        (
            14.00,
            0.05 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.38,
            "Titan Embryo",
            Composition::icy(),
            BodyType::Protoplanet,
            0.03,
        ),
        (
            19.20,
            1.20 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.05,
            "Proto-Uranus",
            Composition::icy(),
            BodyType::IceGiant,
            0.05,
        ),
        (
            30.00,
            1.20 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.05,
            "Proto-Neptune",
            Composition::icy(),
            BodyType::IceGiant,
            0.02,
        ),
        // Canonical Dwarf Planet Pluto (Trans-Neptunian Kuiper Belt Monarch)
        (
            39.48,
            0.00218 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.186,
            "Pluto (Dwarf Planet)",
            Composition::icy(),
            BodyType::TerrestrialPlanet,
            0.02,
        ),
        // Canonical Deep Outer Planet Nine (Hypothetical Super-Earth / Ice Giant Shepherding the Oort Cloud)
        (
            380.00,
            5.50 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 2.30,
            "Planet Nine (Super-Earth / Ice Giant)",
            Composition::icy(),
            BodyType::IceGiant,
            0.05,
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

        let vol = VolatileInventory {
            delivered_water_m_earth: 0.0,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: if a_f64 < 2.7 { 0.10 } else { 0.0 },
            cometary_impact_count: 0,
        };

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
            vol,
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
    let is_massive = disk_params.central_star_mass > 10.0 || disk_params.outer_radius_au > 100.0;
    if is_massive {
        // Little Red Dot / Massive Circum-Nuclear Disk:
        // Spans right from the innermost accretion stream near the central star / black hole (~2.0 AU)
        // throughout the circum-nuclear disk out to outer_radius_au.
        // Stratified zones ensure dense, swirling particles right near the central black hole and quasi-star,
        // extending outward to envelope the companion stars and orbiting worlds.
        let r_in = disk_params.inner_radius_au.clamp(1.0, 10.0);
        let r_out = disk_params.outer_radius_au.max(r_in + 20.0);
        let roll: f64 = rng.random_range(0.0..1.0);
        let (r, comp) = if roll < 0.45 {
            // Zone 1: Inner Accretion Stream (r_in .. 30.0 AU) - 45% of particles directly swirling near the black hole/star!
            let u = roll / 0.45;
            let r = (r_in * r_in + u * (30.0 * 30.0 - r_in * r_in)).sqrt();
            (r, Composition::pure_hydrogen())
        } else if roll < 0.80 {
            // Zone 2: Circum-Nuclear Intermediate Disk (30.0 .. 120.0 AU) - 35% of particles
            let u = (roll - 0.45) / 0.35;
            let r = (30.0 * 30.0 + u * (120.0 * 120.0 - 30.0 * 30.0)).sqrt();
            (r, Composition::solar_gas())
        } else {
            // Zone 3: Outer Primordial Infall Reservoir (120.0 .. r_out) - 20% of particles
            let u = (roll - 0.80) / 0.20;
            let r = (120.0 * 120.0 + u * (r_out * r_out - 120.0 * 120.0)).sqrt();
            (r, Composition::icy())
        };
        (r, comp)
    } else if disk_params.outer_radius_au < 2.0 {
        // Compact planetary system (e.g. TRAPPIST-1 with 7 resonant terrestrial worlds spanning 0.011 - 0.062 AU):
        // Disk strictly spans disk_params.inner_radius_au .. disk_params.outer_radius_au (e.g. 0.005 - 0.15 AU)
        let r_in = disk_params.inner_radius_au.max(0.002);
        let r_out = disk_params.outer_radius_au.max(r_in * 2.0);
        let roll: f64 = rng.random_range(0.0..1.0);
        let (r, comp) = if roll < 0.50 {
            // Zone 1: Inner Terrestrial World Reservoir (r_in .. r_out * 0.45) - 50% particles
            let u = roll / 0.50;
            let r_sub_out = r_in + (r_out - r_in) * 0.45;
            let r = (r_in * r_in + u * (r_sub_out * r_sub_out - r_in * r_in)).sqrt();
            let comp = if r < r_in + (r_sub_out - r_in) * 0.35 {
                Composition::metal_rich()
            } else {
                Composition::rocky()
            };
            (r, comp)
        } else if roll < 0.80 {
            // Zone 2: Habitable / Volatile Transition Zone (r_out * 0.45 .. r_out * 0.75) - 30% particles
            let u = (roll - 0.50) / 0.30;
            let r_sub_in = r_in + (r_out - r_in) * 0.45;
            let r_sub_out = r_in + (r_out - r_in) * 0.75;
            let r =
                (r_sub_in * r_sub_in + u * (r_sub_out * r_sub_out - r_sub_in * r_sub_in)).sqrt();
            (r, Composition::carbonaceous())
        } else {
            // Zone 3: Outer Volatile Ice Reservoir (r_out * 0.75 .. r_out) - 20% particles
            let u = (roll - 0.80) / 0.20;
            let r_sub_in = r_in + (r_out - r_in) * 0.75;
            let r = (r_sub_in * r_sub_in + u * (r_out * r_out - r_sub_in * r_sub_in)).sqrt();
            (r, Composition::icy())
        };
        (r, comp)
    } else {
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
            max_ecs_bodies: 1024, // High minor body capacity for rich Asteroid & Kuiper Belts
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
    let base_interval_yr = 350.0;
    let tau_depletion = disk_params.gas_disk_lifetime_yr / 4.0;
    let spawn_interval = base_interval_yr * (t / tau_depletion).exp();

    let time_since_last = t - spawner.last_spawn_yr;
    if time_since_last < spawn_interval {
        return;
    }

    // --- Spawn a new planetesimal or protoplanet embryo ---
    let mut rng = rand::rng();
    use rand::prelude::*;
    use std::f64::consts::PI;

    let is_massive_disk =
        disk_params.central_star_mass > 10.0 || disk_params.outer_radius_au > 100.0;

    // Canonical zone seeding for the first 8 planetesimal births, guaranteed across any time warp speed
    let (r, comp, is_feeding_zone) = if (spawner.total_spawned as usize) < 8 {
        if is_massive_disk {
            // Little Red Dot / Massive Circum-Nuclear Disk:
            // The central Quasi-Star cocoon is 60 AU! Spawn all bodies safely OUTSIDE the 60 AU cocoon
            // in the rich gas cloud and particle ring (70 - 245 AU) so they orbit stably!
            let (r, c) = match spawner.total_spawned {
                0 => (rng.random_range(72.0..88.0), Composition::solar_gas()), // Inner circum-nuclear giant seed
                1 => (rng.random_range(92.0..112.0), Composition::pure_hydrogen()), // Dense hydrogen cloudlet seed
                2 => (rng.random_range(118.0..142.0), Composition::solar_gas()), // Circum-nuclear embryo
                3 => (rng.random_range(148.0..178.0), Composition::pure_hydrogen()), // Pop-III stellar accretion seed
                4 => (rng.random_range(185.0..215.0), Composition::solar_gas()), // Outer circum-nuclear giant core
                5 => (rng.random_range(220.0..245.0), Composition::pure_hydrogen()), // Outer cloudlet
                6 => (rng.random_range(80.0..130.0), Composition::icy()), // Rocky/icy embryo in particle ring
                _ => (rng.random_range(140.0..220.0), Composition::solar_gas()), // Secondary stellar companion seed
            };
            (r, c, true)
        } else {
            // Hayashi Solar Nebula: Canonical solar system niches (0.38 - 45 AU)
            let (r, c) = match spawner.total_spawned {
                0 => (rng.random_range(0.38..0.72), Composition::rocky()), // Inner Terrestrial (Mercury/Venus)
                1 => (rng.random_range(0.95..1.52), Composition::rocky()), // Habitable Zone (Earth/Mars)
                2 => (rng.random_range(2.4..3.6), Composition::carbonaceous()), // Asteroid Belt Chondrites
                3 => (rng.random_range(5.0..6.2), Composition::icy()), // Jovian Gas Giant Core (Jupiter)
                4 => (rng.random_range(8.8..10.5), Composition::icy()), // Ringed Gas Giant Core (Saturn)
                5 => (rng.random_range(18.0..22.0), Composition::icy()), // Ice Giant Core (Uranus)
                6 => (rng.random_range(28.0..32.0), Composition::icy()), // Outer Ice Giant Core (Neptune)
                _ => (rng.random_range(36.0..45.0), Composition::icy()), // Kuiper Belt Object
            };
            (r, c, true)
        }
    } else if !is_massive_disk {
        // Authentic Solar System Belt & Feeding Zone Distribution:
        // 15% Planetary Feeding Zones (Embryos to seed moons & fuel accretion: 0.7-1.8 AU, 4.5-6.5 AU, 8.5-11.5 AU, 18.0-32.0 AU)
        // 45% Main Asteroid Belt (2.15 - 3.45 AU)
        // 35% Kuiper Belt / Cometary Reservoir (16.0 - 42.0 AU)
        // 5% General disk sampling
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
            let (r_samp, comp_samp) = sample_disk_radius(&mut rng, &disk_params);
            (r_samp, comp_samp, false)
        }
    } else {
        let (r_samp, comp_samp) = sample_disk_radius(&mut rng, &disk_params);
        (r_samp, comp_samp, false)
    };

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

    // Bimodal mass distribution: canonical giant cores start with 0.06 - 0.25 M_earth, terrestrial embryos 0.02 - 0.10 M_earth
    let is_protoplanet: bool = if (spawner.total_spawned as usize) < 8 || is_feeding_zone {
        true
    } else {
        rng.random_bool(0.35)
    };
    let log_mass_earth: f64 = if is_protoplanet {
        if is_massive_disk {
            // Massive circum-nuclear disk seeds: 1.0 to 50.0 Earth masses (giant planet & stellar seeds)
            rng.random_range(0.0..1.7)
        } else if r > disk_params.snow_line_au {
            rng.random_range(-1.2..-0.6) // 0.06 to 0.25 Earth masses for giant cores
        } else {
            rng.random_range(-1.7..-1.0) // 0.02 to 0.10 Earth masses for rocky embryos
        }
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

    // Determine body type from mass & region
    let body_type = if mass >= EARTH_MASS_SOLAR * 0.005 || is_feeding_zone {
        BodyType::Protoplanet
    } else if (2.0..=3.8).contains(&r) {
        BodyType::Asteroid
    } else if r >= 15.0 || comp.ice_frac > 0.35 {
        BodyType::Comet
    } else {
        BodyType::Planetesimal
    };

    // Generate unique name based on disk zone
    spawner.name_counter += 1;
    let name = match body_type {
        BodyType::Protoplanet => format!("Embryo #{}", spawner.name_counter),
        BodyType::Asteroid => format!("Asteroid #{}", spawner.name_counter),
        BodyType::Comet => format!("Comet #{}", spawner.name_counter),
        _ => format!("Planetesimal #{}", spawner.name_counter),
    };

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

/// Delayed spawner for Proto-Earth.
///
/// Prevents early explosive runaway accretion by holding Proto-Earth until
/// the central star ignites and begins radiation clearing (~15 years into the simulation).
/// When elapsed time passes the threshold, Proto-Earth emerges in its canonical 1.00 AU orbit
/// as an oligarchic embryo (~0.55 M_earth) ready for the late giant impact phase.
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

    // If Proto-Earth or Earth already exists, do not spawn another
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

    // Delay spawning until T >= 15.0 years (after protostellar ignition and initial clearing)
    if sim_time.elapsed_years < 15.0 {
        return;
    }

    *spawned = true;

    // Spawn Proto-Earth in its canonical 1.00 AU orbit
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

/// Maintains a steady cascade of planet-crossing impactors (asteroids & comets) during the Late Heavy Bombardment epoch.
/// This guarantees that inner terrestrial planets receive active cometary/asteroid bombardment,
/// delivering volatile water to Earth and forming visible impact basins.
pub fn update_late_heavy_bombardment_cascade(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    disk_params: Res<DiskParameters>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    query: Query<(Entity, &SimPosition, &SimVelocity, &CelestialBody, &Mass)>,
    star_query: Query<(&SimPosition, &Mass), With<CentralStar>>,
    mut cascade_timer: Local<f64>,
    mut cascade_counter: Local<usize>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    if !lhb_state.is_active || lhb_state.migration_progress >= 0.95 {
        return;
    }

    let Ok((star_pos, star_mass)) = star_query.single() else {
        return;
    };

    let dt = sim_time.current_dt_yr;
    *cascade_timer -= dt;

    // Count currently active inner-crossing impactors (q <= 1.6 AU)
    let star_m = star_mass.0.max(0.1);
    let mut active_crossers = 0;
    let mut total_bodies = 0;

    for (_, pos, vel, body, _) in query.iter() {
        total_bodies += 1;
        if matches!(
            body.body_type,
            BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal
        ) {
            let r_vec = pos.0 - star_pos.0;
            let r = (r_vec.x * r_vec.x + r_vec.z * r_vec.z).sqrt();
            let v_sq = vel.0.length_squared();
            // Estimate perihelion q = a * (1 - e)
            let specific_e = 0.5 * v_sq - (G_ASTRO * star_m) / r.max(0.01);
            if specific_e < 0.0 {
                let a = -(G_ASTRO * star_m) / (2.0 * specific_e);
                let h_vec = r_vec.cross(vel.0);
                let h = h_vec.length();
                let e_sq = (1.0 - (h * h) / (G_ASTRO * star_m * a)).max(0.0);
                let e = e_sq.sqrt();
                let q = a * (1.0 - e);
                if q <= 1.6 && r <= 6.0 {
                    active_crossers += 1;
                }
            } else if r <= 2.5 {
                // Hyperbolic/parabolic inbound
                active_crossers += 1;
            }
        }
    }

    // Safety cap: Never let total bodies in ECS exceed 64 to protect N-body performance
    if total_bodies >= 64 {
        return;
    }

    // Maintain a target of 10 to 14 active inner-crossing impactors
    let target_crossers = 12;
    if active_crossers < target_crossers && *cascade_timer <= 0.0 {
        *cascade_counter += 1;
        let c_idx = *cascade_counter;

        // Reset timer: rapid replenishment if very few crossers, otherwise paced
        *cascade_timer = if active_crossers < 5 { 0.4 } else { 2.5 };

        // 60% Carbonaceous Asteroids (main belt origin, water-bearing), 40% Pristine Icy Comets (Kuiper belt origin)
        let is_comet = (c_idx % 5) >= 3;

        let (r_spawn, q_target, mass_solar, rad_au, comp, body_type, name) = if is_comet {
            let r_s = 7.0 + ((c_idx * 17) % 100) as f64 * 0.08; // 7.0 to 15.0 AU
            let q_t = 0.85 + ((c_idx * 31) % 100) as f64 * 0.003; // 0.85 to 1.15 AU
            let m_s = (0.000015 + ((c_idx * 7) % 50) as f64 * 0.000001) * EARTH_MASS_SOLAR;
            let r_au = EARTH_RADIUS_AU * 0.06;
            (
                r_s,
                q_t,
                m_s,
                r_au,
                Composition::icy(),
                BodyType::Comet,
                format!("LHB-Comet C/{}", 1900 + (c_idx % 1000)),
            )
        } else {
            let r_s = 2.3 + ((c_idx * 23) % 100) as f64 * 0.012; // 2.3 to 3.5 AU
            let q_t = 0.70 + ((c_idx * 43) % 100) as f64 * 0.005; // 0.70 to 1.20 AU
            let m_s = (0.000020 + ((c_idx * 13) % 50) as f64 * 0.000001) * EARTH_MASS_SOLAR;
            let r_au = EARTH_RADIUS_AU * 0.08;
            (
                r_s,
                q_t,
                m_s,
                r_au,
                Composition::carbonaceous(),
                BodyType::Asteroid,
                format!("LHB-Asteroid ({})", 10000 + (c_idx % 90000)),
            )
        };

        // Construct Keplerian ellipse with apoapsis ~ r_spawn and periapsis ~ q_target
        let v_tangential =
            (G_ASTRO * star_m * (2.0 * q_target) / (r_spawn * (r_spawn + q_target))).sqrt();
        // Inward radial velocity so it is actively approaching perihelion
        let v_inward = -v_tangential * 0.15;

        let angle = ((c_idx * 137) % 360) as f64 * std::f64::consts::PI / 180.0;
        let inc_angle = (((c_idx * 29) % 20) as f64 - 10.0) * 0.005; // +/- 0.05 rad inclination

        let pos = star_pos.0
            + DVec3::new(
                r_spawn * angle.cos(),
                r_spawn * inc_angle,
                r_spawn * angle.sin(),
            );

        let u_tan = DVec3::new(-angle.sin(), 0.0, angle.cos());
        let u_rad = DVec3::new(angle.cos(), inc_angle, angle.sin()).normalize_or_zero();
        let vel = u_tan * v_tangential + u_rad * v_inward;

        let mut diff = InternalDifferentiation::default();
        diff.recalculate(mass_solar, rad_au, &comp);

        let temp_k = disk_params.reference_temp_1au * (r_spawn.max(0.1)).powf(-0.5);

        commands.spawn((
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration(DVec3::ZERO),
            Mass(mass_solar),
            Radius(rad_au),
            Temperature(temp_k),
            comp,
            diff,
            CelestialBody { name, body_type },
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                cometary_impact_count: 0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.0,
            },
            SpinState::default(),
        ));

        lhb_state.comets_scattered = (lhb_state.comets_scattered + 1).min(100_000);
    }
}
