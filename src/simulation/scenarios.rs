//! Exoplanet System Generator and Sandbox Scenarios.
//!
//! Provides multi-system presets:
//! - TRAPPIST-1 Resonant Ultracool Red Dwarf System (7 Earths)
//! - Kepler-16 Circumbinary System (Tatooine-like binary star pair with circumbinary planet)
//! - Hot Jupiter Inward Migration Scenario (Type II disk migration)
//! - Rogue Planet Flyby Perturbation Scenario (Hyperbolic interstellar interloper)
//! - Hayashi Minimum Mass Solar Nebula (Default Solar System)

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Supported Sandbox Scenario Presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum ScenarioPreset {
    #[default]
    SolarNebulaMmsn,
    Trappist1System,
    Kepler16Circumbinary,
    HotJupiterMigration,
    RoguePlanetFlyby,
}

impl ScenarioPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            ScenarioPreset::SolarNebulaMmsn => "Hayashi Solar Nebula",
            ScenarioPreset::Trappist1System => "TRAPPIST-1 (7 Resonant Earths)",
            ScenarioPreset::Kepler16Circumbinary => "Kepler-16 (Circumbinary Binary)",
            ScenarioPreset::HotJupiterMigration => "Hot Jupiter Migration",
            ScenarioPreset::RoguePlanetFlyby => "Rogue Planet Flyby",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ScenarioPreset::SolarNebulaMmsn => {
                "Default 4.5 Gyr Hayashi Minimum Mass Solar Nebula (MMSN) with central protostar and 10 protoplanetary embryos."
            }
            ScenarioPreset::Trappist1System => {
                "Ultracool M-dwarf (0.09 M☉) with 7 Earth-sized terrestrial worlds in a compact resonant Laplace chain (3 habitable)."
            }
            ScenarioPreset::Kepler16Circumbinary => {
                "Tatooine-like circumbinary system with K/M-dwarf binary pair and a Saturn-mass circumbinary giant at 0.70 AU."
            }
            ScenarioPreset::HotJupiterMigration => {
                "Massive gas giant (1.4 M_Jup) undergoing Type II disk torque inward migration from 5.2 AU down to 0.045 AU."
            }
            ScenarioPreset::RoguePlanetFlyby => {
                "A 3.5 M_Jup interstellar rogue planet screaming through the solar system at 38 km/s, scattering orbits."
            }
        }
    }
}

/// Message event to request loading a new sandbox scenario.
#[derive(Event, Message, Debug, Clone, Copy)]
pub struct LoadScenarioEvent(pub ScenarioPreset);

/// Active state tracking for scenarios with ongoing events (e.g. Migration, Flyby).
#[derive(Resource, Debug, Clone, Default)]
pub struct ActiveScenarioState {
    pub current_preset: ScenarioPreset,
    pub scenario_time_years: f64,
    pub migration_active: bool,
    pub migration_target_au: f64,
    pub rogue_planet_entity: Option<Entity>,
}

/// System that listens for `LoadScenarioEvent` and reinitializes the entire simulation.
pub fn handle_load_scenario_events(
    mut commands: Commands,
    mut events: MessageReader<LoadScenarioEvent>,
    mut disk_params: ResMut<DiskParameters>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    mut time_warp: ResMut<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut scenario_state: ResMut<ActiveScenarioState>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    bodies_query: Query<Entity, With<CelestialBody>>,
) {
    for event in events.read() {
        let preset = event.0;
        info!("🌟 Loading Scenario Preset: {:?}", preset);

        // 1. Despawn all existing celestial bodies
        for ent in bodies_query.iter() {
            commands.entity(ent).despawn();
        }

        // 2. Reset Time & Physics Resources
        sim_time.elapsed_years = 0.0;
        sim_time.current_dt_yr = 0.001;
        time_warp.multiplier = 1.0;
        time_warp.is_paused = false;
        energy_monitor.initial_total_energy = 0.0;
        energy_monitor.kinetic_energy = 0.0;
        energy_monitor.potential_energy = 0.0;
        energy_monitor.total_energy = 0.0;
        energy_monitor.relative_energy_drift = 0.0;
        energy_monitor.initialized = false;
        lhb_state.is_active = false;
        lhb_state.migration_progress = 0.0;
        lhb_state.resonance_crossed = false;

        scenario_state.current_preset = preset;
        scenario_state.scenario_time_years = 0.0;
        scenario_state.migration_active = false;
        scenario_state.rogue_planet_entity = None;

        // 3. Build Scenario
        let central_star_ent = match preset {
            ScenarioPreset::SolarNebulaMmsn => {
                spawn_solar_nebula_mmsn(&mut commands, &mut disk_params)
            }
            ScenarioPreset::Trappist1System => {
                spawn_trappist_1_system(&mut commands, &mut disk_params)
            }
            ScenarioPreset::Kepler16Circumbinary => {
                spawn_kepler_16_system(&mut commands, &mut disk_params)
            }
            ScenarioPreset::HotJupiterMigration => {
                scenario_state.migration_active = true;
                scenario_state.migration_target_au = 0.045;
                spawn_hot_jupiter_scenario(&mut commands, &mut disk_params)
            }
            ScenarioPreset::RoguePlanetFlyby => {
                let (star, rogue) = spawn_rogue_planet_scenario(&mut commands, &mut disk_params);
                scenario_state.rogue_planet_entity = Some(rogue);
                star
            }
        };

        player_state.selected_entity = Some(central_star_ent);
    }
}

/// Helper to spawn the Hayashi Solar Nebula MMSN scenario.
fn spawn_solar_nebula_mmsn(commands: &mut Commands, disk_params: &mut DiskParameters) -> Entity {
    disk_params.central_star_mass = 1.0;
    disk_params.inner_radius_au = 0.20;
    disk_params.outer_radius_au = 45.0;
    disk_params.disk_mass = 0.0006;

    // Central Protostar
    let star = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::Protostar,
                name: "The Protostar (Solar Nebula)".to_string(),
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
                core_temperature: 4.0e6,
                fusion_fraction: 0.4,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id();

    // 10 MMSN Protoplanetary Seeds
    let major_seeds: [(f64, f64, f64, &str, Composition, BodyType, f64); 10] = [
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
    ];

    for &(r_au, mass_s, rad_au, name, comp, b_type, phi_off) in &major_seeds {
        let v_circ = (G_ASTRO * 1.0 / r_au).sqrt();
        let pos = DVec3::new(r_au * phi_off.cos(), 0.0, r_au * phi_off.sin());
        let vel = DVec3::new(-v_circ * phi_off.sin(), 0.0, v_circ * phi_off.cos());

        commands.spawn((
            CelestialBody {
                body_type: b_type,
                name: name.to_string(),
            },
            Mass(mass_s),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(rad_au),
            Temperature(280.0 * (1.0 / r_au.sqrt())),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * mass_s),
            comp,
        ));
    }

    star
}

/// Spawns the TRAPPIST-1 Resonant Ultracool Red Dwarf System with 7 Earth-sized planets.
fn spawn_trappist_1_system(commands: &mut Commands, disk_params: &mut DiskParameters) -> Entity {
    let m_star = 0.0898; // M_sun
    disk_params.central_star_mass = m_star;
    disk_params.inner_radius_au = 0.005;
    disk_params.outer_radius_au = 0.15;
    disk_params.disk_mass = 0.0001;

    // Central Ultracool M-Dwarf: TRAPPIST-1
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
            Radius(0.00056), // 0.121 R_sun
            Temperature(2566.0),
            Luminosity(0.000553), // 0.055% Solar Luminosity
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

    // 7 Planets of TRAPPIST-1: (a_au, m_earth, r_earth, name, ocean_frac, atm_bar, is_habitable)
    let trappist_planets = [
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
    ];

    for (i, &(a_au, m_e, r_e, name, ocean_frac, atm_bar, is_hab, regime, t_surf)) in
        trappist_planets.iter().enumerate()
    {
        let mass_s = m_e * EARTH_MASS_SOLAR;
        let rad_au = r_e * EARTH_RADIUS_AU;
        let v_circ = (G_ASTRO * m_star / a_au).sqrt();
        let phi = (i as f64) * (2.0 * PI / 7.0);

        let pos = DVec3::new(a_au * phi.cos(), 0.0, a_au * phi.sin());
        let vel = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());

        let mut comp = Composition::rocky();
        if ocean_frac > 0.3 {
            comp.ice_frac = ocean_frac * 0.15;
        }

        let p_yr = a_au.powf(1.5) / m_star.sqrt();
        let p_hours = p_yr * YEAR_SECONDS / 3600.0;

        commands.spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: name.to_string(),
            },
            Mass(mass_s),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(rad_au),
            Temperature(t_surf as f64),
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
            },
            SpinState {
                rotation_period_hours: p_hours, // Tidally locked
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
            BiosphereState {
                habitability_score: if is_hab { 0.92 } else { 0.05 },
                biomass_coverage_frac: if is_hab { 0.65 } else { 0.0 },
                oxygen_fraction: if is_hab { 0.18 } else { 0.001 },
                emergence_year: if is_hab { Some(100.0) } else { None },
            },
        ));
    }

    star
}

/// Spawns the Kepler-16 Circumbinary System ("Tatooine" binary pair + circumbinary gas giant).
fn spawn_kepler_16_system(commands: &mut Commands, disk_params: &mut DiskParameters) -> Entity {
    let m_a = 0.6897; // Primary K-dwarf
    let m_b = 0.2025; // Secondary M-dwarf
    let m_total = m_a + m_b;

    disk_params.central_star_mass = m_total;
    disk_params.inner_radius_au = 0.10;
    disk_params.outer_radius_au = 3.50;
    disk_params.disk_mass = 0.0002;

    // Primary Star Kepler-16A (anchored near barycenter)
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
            Radius(0.0030), // 0.6489 R_sun
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

    // Secondary Binary Companion Kepler-16B (Orbiting at a = 0.224 AU, e = 0.159)
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
        Radius(0.0010), // 0.226 R_sun
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

    // Circumbinary Planet Kepler-16b (Saturn-mass gas giant at a = 0.7048 AU, P = 228 days)
    let a_planet = 0.7048;
    let m_planet = 106.0 * EARTH_MASS_SOLAR; // 0.333 M_Jup
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
            Radius(EARTH_RADIUS_AU * 8.4), // 0.75 R_Jup
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

    // Circumbinary Exomoon around Kepler-16b (Habitable Moon)
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

    // Outer Habitable Circumbinary Terrestrial Planet (Kepler-16c at a = 1.15 AU)
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

    star_a
}

/// Spawns the Hot Jupiter Inward Migration Scenario.
fn spawn_hot_jupiter_scenario(commands: &mut Commands, disk_params: &mut DiskParameters) -> Entity {
    disk_params.central_star_mass = 1.0;
    disk_params.inner_radius_au = 0.03;
    disk_params.outer_radius_au = 15.0;
    disk_params.disk_mass = 0.001;

    // Central Sun-like Star
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

    // Migrating Massive Proto-Jupiter (1.4 M_Jup at 5.2 AU)
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

    // Inner Terrestrial Embryos (which will be scattered/swallowed as Jupiter migrates inward)
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

/// Spawns the Rogue Planet Flyby Perturbation Scenario.
fn spawn_rogue_planet_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> (Entity, Entity) {
    disk_params.central_star_mass = 1.0;
    disk_params.inner_radius_au = 0.30;
    disk_params.outer_radius_au = 40.0;
    disk_params.disk_mass = 0.0001;

    // Central Sun
    let star = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::YellowDwarf,
                name: "The Sun (G2V)".to_string(),
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

    // Standard Solar System Planets: Mercury, Earth, Jupiter, Neptune
    let solar_planets: [(f64, f64, &str, BodyType, Composition, f64); 4] = [
        (
            0.387,
            0.055 * EARTH_MASS_SOLAR,
            "Mercury",
            BodyType::TerrestrialPlanet,
            Composition::metal_rich(),
            0.0,
        ),
        (
            1.000,
            1.000 * EARTH_MASS_SOLAR,
            "Earth",
            BodyType::TerrestrialPlanet,
            Composition::rocky(),
            1.0,
        ),
        (
            5.204,
            317.8 * EARTH_MASS_SOLAR,
            "Jupiter",
            BodyType::GasGiant,
            Composition::solar_gas(),
            3.0,
        ),
        (
            30.07,
            17.15 * EARTH_MASS_SOLAR,
            "Neptune",
            BodyType::IceGiant,
            Composition::icy(),
            5.0,
        ),
    ];

    for &(a_au, m_s, name, b_type, comp, phi) in &solar_planets {
        let v_circ = (G_ASTRO * 1.0 / a_au).sqrt();
        let pos = DVec3::new(a_au * phi.cos(), 0.0, a_au * phi.sin());
        let vel = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());

        commands.spawn((
            CelestialBody {
                body_type: b_type,
                name: name.to_string(),
            },
            Mass(m_s),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(if b_type == BodyType::GasGiant {
                EARTH_RADIUS_AU * 11.2
            } else {
                EARTH_RADIUS_AU
            }),
            Temperature(280.0 / a_au.sqrt()),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * m_s),
            comp,
        ));
    }

    // 12 Outer Kuiper Belt Comets / Asteroids to be scattered by the flyby
    for i in 0..12 {
        let a_k = 18.0 + (i as f64) * 1.8;
        let phi = (i as f64) * 0.52;
        let v_circ = (G_ASTRO * 1.0 / a_k).sqrt();
        let pos = DVec3::new(a_k * phi.cos(), i as f64 * 0.2 - 1.2, a_k * phi.sin());
        let vel = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());

        commands.spawn((
            CelestialBody {
                body_type: BodyType::Comet,
                name: format!("Kuiper Comet K-{}", i + 1),
            },
            Mass(0.0001 * EARTH_MASS_SOLAR),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(EARTH_RADIUS_AU * 0.15),
            Temperature(50.0),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * 0.0001 * EARTH_MASS_SOLAR),
            Composition::icy(),
        ));
    }

    // Hyperbolic Interstellar Rogue Planet "Nemesis X" (3.5 M_Jup, hyperbolic inbound flyby)
    let rogue_mass = 3.5 * JUPITER_MASS_SOLAR;
    let r_init = DVec3::new(-35.0, 3.2, -28.0);
    // Aiming towards periapsis ~ 1.8 AU with v_inf ~ 38 km/s (~ 8.0 AU/yr)
    let v_init = DVec3::new(6.8, -0.6, 5.2);

    let rogue = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::GasGiant,
                name: "Rogue Interloper (Nemesis X)".to_string(),
            },
            Mass(rogue_mass),
            SimPosition(r_init),
            SimVelocity(v_init),
            SimAcceleration::default(),
            Radius(EARTH_RADIUS_AU * 13.5),
            Temperature(140.0),
            Luminosity(0.0),
            AngularMomentum(r_init.cross(v_init) * rogue_mass),
            Composition {
                metal_frac: 0.01,
                silicate_frac: 0.03,
                ice_frac: 0.08,
                organics_frac: 0.00,
                gas_frac: 0.88,
            },
            SpinState {
                rotation_period_hours: 8.2,
                axial_tilt_degrees: 42.0,
                spin_vector: DVec3::new(0.3, 0.9, 0.2).normalize(),
            },
        ))
        .id();

    (star, rogue)
}

/// System to execute continuous scenario dynamics (e.g. Type II migration drag).
pub fn update_active_scenarios(
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut scenario_state: ResMut<ActiveScenarioState>,
    mut bodies_query: Query<(&mut SimVelocity, &SimPosition, &mut CelestialBody, &Mass)>,
) {
    if time_warp.is_paused {
        return;
    }

    let dt = sim_time.current_dt_yr;
    scenario_state.scenario_time_years += dt;

    // Type II Inward Migration for Hot Jupiter scenario
    if scenario_state.migration_active
        && scenario_state.current_preset == ScenarioPreset::HotJupiterMigration
    {
        for (mut vel, pos, mut body, _mass) in bodies_query.iter_mut() {
            if body.name.contains("Hot Jupiter") {
                let r = (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt();
                if r > scenario_state.migration_target_au {
                    // Type II disk orbital torque: applies tangential inward drag
                    let v_dir = vel.0.normalize_or_zero();
                    let inward_thrust = 0.0025 * (r / 5.2).clamp(0.2, 1.0);
                    vel.0 -= v_dir * (inward_thrust * dt);
                } else if !body.name.contains("Parked") {
                    body.name = "Ultra-Short Period Hot Jupiter (Parked)".to_string();
                }
            }
        }
    }
}
