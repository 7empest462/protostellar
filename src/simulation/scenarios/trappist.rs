//! TRAPPIST-1 Resonant Ultracool Red Dwarf scenario with 7 Earth-sized planets.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;
use crate::utils::constants::*;

type TrappistPlanet = (
    f64,
    f64,
    f64,
    &'static str,
    f64,
    f64,
    bool,
    ClimateRegime,
    f32,
);

fn get_trappist_planets() -> [TrappistPlanet; 7] {
    [
        (
            0.01154,
            1.374,
            1.116,
            "TRAPPIST-1b",
            0.0,
            8.5,
            false,
            ClimateRegime::RunawayVenusian,
            400.0f32,
        ),
        (
            0.01580,
            1.308,
            1.097,
            "TRAPPIST-1c",
            0.0,
            15.0,
            false,
            ClimateRegime::RunawayVenusian,
            342.0f32,
        ),
        (
            0.02227,
            0.388,
            0.788,
            "TRAPPIST-1d",
            0.15,
            0.8,
            false,
            ClimateRegime::TemperateHabitable,
            288.0f32,
        ),
        (
            0.02925,
            0.692,
            0.920,
            "TRAPPIST-1e",
            0.55,
            1.0,
            true,
            ClimateRegime::TemperateHabitable,
            282.0f32,
        ),
        (
            0.03849,
            1.039,
            1.045,
            "TRAPPIST-1f",
            0.85,
            2.2,
            true,
            ClimateRegime::TemperateHabitable,
            260.0f32,
        ),
        (
            0.04688,
            1.321,
            1.129,
            "TRAPPIST-1g",
            0.40,
            1.6,
            true,
            ClimateRegime::SnowballIceAge,
            235.0f32,
        ),
        (
            0.06193,
            0.326,
            0.775,
            "TRAPPIST-1h",
            0.05,
            0.2,
            false,
            ClimateRegime::SnowballIceAge,
            173.0f32,
        ),
    ]
}

fn spawn_trappist_planet(commands: &mut Commands, m_star: f64, i: usize, planet: &TrappistPlanet) {
    let &(a_au, m_e, r_e, name, ocean_frac, atm_bar, is_hab, regime, t_surf) = planet;
    let mass_s = m_e * EARTH_MASS_SOLAR;
    let rad_au = r_e * EARTH_RADIUS_AU;
    let v_circ = (G_ASTRO * m_star / a_au).sqrt();
    let phi = (i as f64) * (2.0 * PI / 7.0);

    let pos = DVec3::new(a_au * phi.cos(), 0.0, a_au * phi.sin());
    let vel = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());

    let mut comp = Composition::rocky();
    if ocean_frac > 0.01 {
        comp.ice_frac = (ocean_frac * 0.15).max(0.01);
    }

    let p_yr = a_au.powf(1.5) / m_star.sqrt();
    let p_hours = p_yr * YEAR_SECONDS / 3600.0;

    let mut cmd = commands.spawn((
        CelestialBody {
            body_type: BodyType::TerrestrialPlanet,
            name: name.to_string(),
        },
        Mass(mass_s),
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration::default(),
        Radius(rad_au),
        Temperature(f64::from(t_surf)),
        Luminosity(0.0),
        AngularMomentum(pos.cross(vel) * mass_s),
        comp,
        InternalDifferentiation {
            is_differentiated: true,
            differentiation_fraction: 1.0,
            core_radius_au: rad_au * 0.55,
            mantle_radius_au: rad_au * 0.95,
            crust_thickness_au: rad_au * 0.05,
            ocean_ice_thickness_au: if ocean_frac > 0.3 { rad_au * 0.01 } else { 0.0 },
            magnetic_field_gauss: if is_hab { 0.85 } else { 0.35 },
            core_temp_k: 4500.0,
            has_theia_llsvp: false,
            llsvp_density_contrast: 0.0,
        },
        SpinState {
            rotation_period_hours: p_hours,
            axial_tilt_degrees: 0.1,
            spin_vector: DVec3::new(0.0, 1.0, 0.0),
        },
        VolatileInventory {
            delivered_water_m_earth: ocean_frac * 0.003,
            ocean_coverage_frac: ocean_frac as f32,
            atmospheric_pressure_bar: atm_bar as f32,
            cometary_impact_count: 24,
        },
        PlanetaryClimate {
            surface_temperature_k: t_surf,
            equilibrium_temperature_k: t_surf - 30.0,
            greenhouse_delta_k: 30.0,
            albedo: if ocean_frac > 0.5 { 0.28 } else { 0.35 },
            ice_coverage_frac: if t_surf < 240.0 { 0.85 } else { 0.10 },
            cloud_coverage_frac: 0.50,
            climate_regime: regime,
        },
    ));
    cmd.insert((
        BiosphereState {
            habitability_score: if is_hab { 0.92 } else { 0.05 },
            biomass_coverage_frac: if is_hab { 0.65 } else { 0.0 },
            oxygen_fraction: if is_hab { 0.18 } else { 0.001 },
            emergence_year: if is_hab { Some(100.0) } else { None },
        },
        TidalState {
            host_entity: None,
            love_number_k2: 0.30,
            tidal_q: 100.0,
            is_tidally_locked: true,
            locking_progress: 1.0,
            resonance_ratio: 1.0,
            tidal_heating_power_watts: 8.5e13,
            tidal_heating_flux_w_m2: 0.15,
            circularization_rate_per_myr: -0.005,
            circularization_timescale_yr: 2e6,
            sync_timescale_yr: 1e4,
        },
    ));
}

/// Spawns the TRAPPIST-1 Resonant Ultracool Red Dwarf System with 7 Earth-sized planets.
pub fn spawn_trappist_1_system(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    let m_star = 0.0898;
    disk_params.central_star_mass = m_star;
    disk_params.inner_radius_au = 0.005;
    disk_params.outer_radius_au = 0.15;
    disk_params.disk_mass = 0.0;
    disk_params.gas_disk_lifetime_yr = 0.0;

    let star = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::RedDwarf,
                name: "TRAPPIST-1 (Ultracool M-Dwarf)".to_string(),
            },
            CentralStar,
            Mass(m_star),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(0.00056),
            Temperature(2566.0),
            Luminosity(0.000_553),
            AngularMomentum::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1.2e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState {
                phase: StellarEvolutionPhase::MainSequence,
                hydrogen_core_fraction: 0.95,
                helium_core_fraction: 0.05,
                envelope_mass_loss_rate: 0.0,
                phase_timer_years: 0.0,
                nebula_expansion_radius_au: 0.0,
                nebula_opacity: 0.0,
            },
            ElectromagneticFieldState {
                magnetic_field_gauss: 600.0,
                rotation_period_sec: 3.3 * 86400.0,
                magnetic_inclination_rad: 0.05,
                jet_length_au: 0.0,
                synchrotron_intensity: 0.0,
            },
        ))
        .id();

    for (i, planet) in get_trappist_planets().iter().enumerate() {
        spawn_trappist_planet(commands, m_star, i, planet);
    }

    star
}
