//! Initial protoplanetary disk generation and radial sampling.

use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

type SeedTuple = (f64, f64, f64, &'static str, Composition, BodyType, f64);

fn get_major_seeds() -> [SeedTuple; 12] {
    [
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
    ]
}

fn get_outer_and_asteroid_seeds() -> [SeedTuple; 13] {
    [
        // Canonical Deep Outer Planet Nine
        (
            380.00,
            5.50 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 2.30,
            "Planet Nine (Super-Earth / Ice Giant)",
            Composition::icy(),
            BodyType::IceGiant,
            0.05,
        ),
        // Asteroid Belt Minor Planets
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
            0.000_005 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.03,
            "Bennu (Asteroid)",
            Composition::carbonaceous(),
            BodyType::Asteroid,
            0.20,
        ),
        (
            1.19,
            0.000_005 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.03,
            "Ryugu (Asteroid)",
            Composition::carbonaceous(),
            BodyType::Asteroid,
            0.19,
        ),
        (
            1.32,
            0.000_005 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.03,
            "Itokawa (Asteroid)",
            Composition::rocky(),
            BodyType::Asteroid,
            0.28,
        ),
    ]
}

fn get_comet_seeds() -> [SeedTuple; 12] {
    [
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
    ]
}

fn spawn_seed_body(
    commands: &mut Commands,
    seed: SeedTuple,
    protostar_mass: f64,
    ref_temp_1au: f64,
    rng: &mut impl Rng,
) {
    let (a, mass, radius, name, comp, body_type, ecc) = seed;
    let a_f64: f64 = a;
    let ecc_f64: f64 = ecc;
    let phi: f64 = rng.random_range(0.0..2.0 * PI);
    let inc_rad: f64 = rng.random_range(-0.04..0.04);

    let r_p = a_f64 * (1.0f64 - ecc_f64).max(0.1f64);
    let pos = DVec3::new(
        r_p * phi.cos() * inc_rad.cos(),
        r_p * inc_rad.sin(),
        r_p * phi.sin() * inc_rad.cos(),
    );

    let v_peri = ((G_ASTRO * protostar_mass / a_f64)
        * ((1.0f64 + ecc_f64) / (1.0f64 - ecc_f64).max(0.01f64)))
    .sqrt();
    let vel = DVec3::new(-v_peri * phi.sin(), 0.0, v_peri * phi.cos());

    let temp = ref_temp_1au * (a_f64 / 1.0).powf(-0.5);

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

/// Spawns the initial protostellar system: only the central protostar in ECS.
/// (The 50,000 dust/planetesimal particle field is simulated in VRAM and rendered directly).
pub fn spawn_protoplanetary_disk(
    commands: &mut Commands,
    disk_params: &DiskParameters,
    _sim_config: &SimulationConfig,
) -> Entity {
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

    let mut rng = rand::rng();
    let all_seeds = get_major_seeds()
        .into_iter()
        .chain(get_outer_and_asteroid_seeds())
        .chain(get_comet_seeds());

    for seed in all_seeds {
        spawn_seed_body(
            commands,
            seed,
            protostar_mass,
            disk_params.reference_temp_1au,
            &mut rng,
        );
    }

    star_ent
}

/// Helper function to sample initial radial position according to standard MMSN power-law.
pub fn sample_disk_radius<R: rand::Rng + ?Sized>(
    rng: &mut R,
    disk_params: &DiskParameters,
) -> (f64, Composition) {
    let is_massive = disk_params.central_star_mass > 10.0 || disk_params.outer_radius_au > 100.0;
    if is_massive {
        sample_massive_disk_radius(rng, disk_params)
    } else if disk_params.outer_radius_au < 2.0 {
        sample_compact_disk_radius(rng, disk_params)
    } else {
        sample_standard_disk_radius(rng, disk_params)
    }
}

fn sample_massive_disk_radius<R: rand::Rng + ?Sized>(
    rng: &mut R,
    disk_params: &DiskParameters,
) -> (f64, Composition) {
    let r_in = disk_params.inner_radius_au.clamp(1.0, 10.0);
    let r_out = disk_params.outer_radius_au.max(r_in + 20.0);
    let roll: f64 = rng.random_range(0.0..1.0);
    if roll < 0.45 {
        let u = roll / 0.45;
        let r = (r_in * r_in + u * (30.0 * 30.0 - r_in * r_in)).sqrt();
        (r, Composition::pure_hydrogen())
    } else if roll < 0.80 {
        let u = (roll - 0.45) / 0.35;
        let r = (30.0 * 30.0 + u * (120.0 * 120.0 - 30.0 * 30.0)).sqrt();
        (r, Composition::solar_gas())
    } else {
        let u = (roll - 0.80) / 0.20;
        let r = (120.0 * 120.0 + u * (r_out * r_out - 120.0 * 120.0)).sqrt();
        (r, Composition::icy())
    }
}

fn sample_compact_disk_radius<R: rand::Rng + ?Sized>(
    rng: &mut R,
    disk_params: &DiskParameters,
) -> (f64, Composition) {
    let r_in = disk_params.inner_radius_au.max(0.002);
    let r_out = disk_params.outer_radius_au.max(r_in * 2.0);
    let roll: f64 = rng.random_range(0.0..1.0);
    if roll < 0.50 {
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
        let u = (roll - 0.50) / 0.30;
        let r_sub_in = r_in + (r_out - r_in) * 0.45;
        let r_sub_out = r_in + (r_out - r_in) * 0.75;
        let r = (r_sub_in * r_sub_in + u * (r_sub_out * r_sub_out - r_sub_in * r_sub_in)).sqrt();
        (r, Composition::carbonaceous())
    } else {
        let u = (roll - 0.80) / 0.20;
        let r_sub_in = r_in + (r_out - r_in) * 0.75;
        let r = (r_sub_in * r_sub_in + u * (r_out * r_out - r_sub_in * r_sub_in)).sqrt();
        (r, Composition::icy())
    }
}

fn sample_standard_disk_radius<R: rand::Rng + ?Sized>(
    rng: &mut R,
    disk_params: &DiskParameters,
) -> (f64, Composition) {
    let roll: f64 = rng.random_range(0.0..1.0);
    if roll < 0.25 {
        let u = roll / 0.25;
        let r_in_sqrt = disk_params.inner_radius_au.sqrt();
        let r_out_sqrt = 2.50f64.sqrt();
        let r_sqrt = r_in_sqrt + u * (r_out_sqrt - r_in_sqrt);
        let r = r_sqrt * r_sqrt;
        let comp = if r < 0.60 {
            Composition::metal_rich()
        } else {
            Composition::rocky()
        };
        (r, comp)
    } else if roll < 0.40 {
        let u = (roll - 0.25) / 0.15;
        let r_in_sqrt = 2.50f64.sqrt();
        let r_out_sqrt = 4.50f64.sqrt();
        let r_sqrt = r_in_sqrt + u * (r_out_sqrt - r_in_sqrt);
        let r = r_sqrt * r_sqrt;
        (r, Composition::carbonaceous())
    } else if roll < 0.85 {
        let u = (roll - 0.40) / 0.45;
        let r_in_sqrt = 4.50f64.sqrt();
        let r_out_sqrt = 25.0f64.sqrt();
        let r_sqrt = r_in_sqrt + u * (r_out_sqrt - r_in_sqrt);
        let r = r_sqrt * r_sqrt;
        (r, Composition::icy())
    } else {
        let u = (roll - 0.85) / 0.15;
        let r_in_sqrt = 25.0f64.sqrt();
        let r_out_sqrt = 45.0f64.sqrt();
        let r_sqrt = r_in_sqrt + u * (r_out_sqrt - r_in_sqrt);
        let r = r_sqrt * r_sqrt;
        (r, Composition::icy())
    }
}
