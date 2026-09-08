//! Circumbinary (Kepler-16) and Hot Jupiter migration scenarios.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

fn spawn_kepler_16_stars(commands: &mut Commands, m_a: f64, m_b: f64, m_total: f64) -> Entity {
    let star_a = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::YellowDwarf,
                name: "Kepler-16A (Primary K-Dwarf)".to_string(),
            },
            CentralStar,
            Mass(m_a),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(0.0030),
            Temperature(4450.0),
            Luminosity(0.148),
            AngularMomentum::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1.5e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
            ElectromagneticFieldState {
                magnetic_field_gauss: 15.0,
                rotation_period_sec: 35.0 * 86400.0,
                magnetic_inclination_rad: 0.1,
                jet_length_au: 0.0,
                synchrotron_intensity: 0.0,
            },
        ))
        .id();

    let a_bin = 0.2243;
    let v_bin = (G_ASTRO * m_total / a_bin).sqrt();
    commands.spawn((
        CelestialBody {
            body_type: BodyType::RedDwarf,
            name: "Kepler-16B (Secondary M-Dwarf)".to_string(),
        },
        Mass(m_b),
        SimPosition(DVec3::new(a_bin, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, v_bin)),
        SimAcceleration::default(),
        Radius(0.0010),
        Temperature(3000.0),
        Luminosity(0.005),
        AngularMomentum(DVec3::new(0.0, a_bin * v_bin * m_b, 0.0)),
        Composition::solar_gas(),
        IgnitionState {
            core_temperature: 1.0e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 0.0,
        },
        StellarEvolutionState::default(),
    ));

    star_a
}

fn spawn_kepler_16_planets(commands: &mut Commands, m_total: f64) {
    let a_planet = 0.7048;
    let m_planet = 106.0 * EARTH_MASS_SOLAR;
    let v_planet = (G_ASTRO * m_total / a_planet).sqrt();
    let pos_p = DVec3::new(0.0, 0.0, a_planet);
    let vel_p = DVec3::new(-v_planet, 0.0, 0.0);

    let planet_ent = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::GasGiant,
                name: "Kepler-16b (Circumbinary Giant)".to_string(),
            },
            Mass(m_planet),
            SimPosition(pos_p),
            SimVelocity(vel_p),
            SimAcceleration::default(),
            Radius(EARTH_RADIUS_AU * 8.4),
            Temperature(190.0),
            Luminosity(0.0),
            AngularMomentum(pos_p.cross(vel_p) * m_planet),
            Composition {
                metal_frac: 0.02,
                silicate_frac: 0.04,
                ice_frac: 0.12,
                organics_frac: 0.00,
                gas_frac: 0.82,
            },
            SpinState {
                rotation_period_hours: 14.5,
                axial_tilt_degrees: 3.2,
                spin_vector: DVec3::new(0.0, 1.0, 0.0),
            },
            PlanetaryRingSystem {
                inner_radius_au: 0.0005,
                outer_radius_au: 0.0018,
                ring_mass_earth: 0.0001,
                optical_depth: 0.75,
                ice_fraction: 0.95,
                silicate_fraction: 0.05,
            },
        ))
        .id();

    let r_moon_orbit = 0.0028;
    let v_moon = (G_ASTRO * m_planet / r_moon_orbit).sqrt();
    commands.spawn((
        CelestialBody {
            body_type: BodyType::Moon,
            name: "Kepler-16b I (Tatooine Prime Moon)".to_string(),
        },
        SatelliteOf {
            parent: planet_ent,
            semi_major_axis_au: r_moon_orbit,
            orbital_period_years: 0.02,
            true_anomaly: 0.0,
        },
        Mass(0.45 * EARTH_MASS_SOLAR),
        SimPosition(pos_p + DVec3::new(r_moon_orbit, 0.0, 0.0)),
        SimVelocity(vel_p + DVec3::new(0.0, 0.0, v_moon)),
        SimAcceleration::default(),
        Radius(EARTH_RADIUS_AU * 0.78),
        Temperature(275.0),
        Luminosity(0.0),
        AngularMomentum::default(),
        Composition::rocky(),
        VolatileInventory {
            delivered_water_m_earth: 0.001,
            ocean_coverage_frac: 0.60,
            atmospheric_pressure_bar: 1.1,
            cometary_impact_count: 15,
        },
        PlanetaryClimate {
            surface_temperature_k: 280.0,
            equilibrium_temperature_k: 245.0,
            greenhouse_delta_k: 35.0,
            albedo: 0.30,
            ice_coverage_frac: 0.15,
            cloud_coverage_frac: 0.50,
            climate_regime: ClimateRegime::TemperateHabitable,
        },
        BiosphereState {
            habitability_score: 0.88,
            biomass_coverage_frac: 0.55,
            oxygen_fraction: 0.20,
            emergence_year: Some(10.0),
        },
    ));

    let a_c = 1.15;
    let v_c = (G_ASTRO * m_total / a_c).sqrt();
    commands.spawn((
        CelestialBody {
            body_type: BodyType::TerrestrialPlanet,
            name: "Kepler-16c (Habitable Ocean World)".to_string(),
        },
        Mass(1.15 * EARTH_MASS_SOLAR),
        SimPosition(DVec3::new(-a_c, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, -v_c)),
        SimAcceleration::default(),
        Radius(EARTH_RADIUS_AU * 1.05),
        Temperature(285.0),
        Luminosity(0.0),
        AngularMomentum(DVec3::new(0.0, a_c * v_c * 1.15 * EARTH_MASS_SOLAR, 0.0)),
        Composition::rocky(),
        VolatileInventory {
            delivered_water_m_earth: 0.0025,
            ocean_coverage_frac: 0.70,
            atmospheric_pressure_bar: 1.2,
            cometary_impact_count: 18,
        },
        PlanetaryClimate {
            surface_temperature_k: 288.0,
            equilibrium_temperature_k: 250.0,
            greenhouse_delta_k: 38.0,
            albedo: 0.29,
            ice_coverage_frac: 0.08,
            cloud_coverage_frac: 0.55,
            climate_regime: ClimateRegime::TemperateHabitable,
        },
        BiosphereState {
            habitability_score: 0.94,
            biomass_coverage_frac: 0.72,
            oxygen_fraction: 0.21,
            emergence_year: Some(50.0),
        },
    ));
}

/// Spawns the Kepler-16 Circumbinary System ("Tatooine" binary pair + circumbinary gas giant).
pub fn spawn_kepler_16_system(commands: &mut Commands, disk_params: &mut DiskParameters) -> Entity {
    let m_a = 0.6897;
    let m_b = 0.2025;
    let m_total = m_a + m_b;

    disk_params.central_star_mass = m_total;
    disk_params.inner_radius_au = 0.10;
    disk_params.outer_radius_au = 3.50;
    disk_params.disk_mass = 0.00010;

    let star_a = spawn_kepler_16_stars(commands, m_a, m_b, m_total);
    spawn_kepler_16_planets(commands, m_total);

    star_a
}

/// Spawns the Hot Jupiter Inward Migration Scenario.
pub fn spawn_hot_jupiter_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    disk_params.central_star_mass = 1.0;
    disk_params.inner_radius_au = 0.03;
    disk_params.outer_radius_au = 15.0;
    disk_params.disk_mass = 0.00015;

    let star = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::YellowDwarf,
                name: "The Host Star (G-Type)".to_string(),
            },
            CentralStar,
            Mass(1.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(SOLAR_RADIUS_AU),
            Temperature(5778.0),
            Luminosity(1.0),
            AngularMomentum::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1.5e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id();

    let a_jup = 5.20;
    let m_jup = 1.4 * JUPITER_MASS_SOLAR;
    let v_jup = (G_ASTRO * 1.0 / a_jup).sqrt();
    let pos_j = DVec3::new(a_jup, 0.0, 0.0);
    let vel_j = DVec3::new(0.0, 0.0, v_jup);

    commands.spawn((
        CelestialBody {
            body_type: BodyType::GasGiant,
            name: "Migrating Hot Jupiter".to_string(),
        },
        Mass(m_jup),
        SimPosition(pos_j),
        SimVelocity(vel_j),
        SimAcceleration::default(),
        Radius(EARTH_RADIUS_AU * 11.2),
        Temperature(160.0),
        Luminosity(0.0),
        AngularMomentum(pos_j.cross(vel_j) * m_jup),
        Composition {
            metal_frac: 0.02,
            silicate_frac: 0.03,
            ice_frac: 0.05,
            organics_frac: 0.00,
            gas_frac: 0.90,
        },
        SpinState {
            rotation_period_hours: 9.8,
            axial_tilt_degrees: 2.1,
            spin_vector: DVec3::new(0.0, 1.0, 0.0),
        },
    ));

    let embryos: [(f64, f64, &str, f64); 4] = [
        (0.60, 0.35 * EARTH_MASS_SOLAR, "Inner Proto-Mercury", 0.0),
        (1.00, 0.90 * EARTH_MASS_SOLAR, "Inner Proto-Earth", 1.2),
        (1.65, 0.50 * EARTH_MASS_SOLAR, "Inner Proto-Mars", 2.5),
        (2.80, 0.25 * EARTH_MASS_SOLAR, "Belt Embryo Ceres", 4.0),
    ];

    for &(a_au, m_e, name, phi) in &embryos {
        let v_circ = (G_ASTRO * 1.0 / a_au).sqrt();
        let pos = DVec3::new(a_au * phi.cos(), 0.0, a_au * phi.sin());
        let vel = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());

        commands.spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: name.to_string(),
            },
            Mass(m_e),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(EARTH_RADIUS_AU * 0.9),
            Temperature(280.0 * (1.0 / a_au.sqrt())),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * m_e),
            Composition::rocky(),
        ));
    }

    star
}
