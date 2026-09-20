//! Exotic scenarios: Rogue planet flyby, Little Red Dot (Quasi-Star), Pulsar, and Magnetar.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::relativity::RelativisticState;
use crate::simulation::resources::*;
use crate::utils::constants::*;

fn spawn_rogue_intruder(commands: &mut Commands) -> Entity {
    let rogue_mass = 3.5 * JUPITER_MASS_SOLAR;
    let r_init = DVec3::new(-35.0, 3.2, -28.0);
    let v_init = DVec3::new(6.8, -0.6, 5.2);

    commands
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
            AtmosphericStormState {
                polar_hexagon_amplitude: 0.0,
                polar_hexagon_wavenumber: 0.0,
                polar_hexagon_colatitude: 0.22,
                great_spot_size: 0.48,
                great_spot_latitude_rad: -0.32,
                great_spot_longitude_rad: 0.40,
                vortex_spin_rate: 3.5,
                secondary_oval_count: 5,
                zonal_shear_turbulence: 0.95,
            },
        ))
        .id()
}

/// Spawns the Rogue Planet Flyby Perturbation Scenario.
pub fn spawn_rogue_planet_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> (Entity, Entity) {
    disk_params.central_star_mass = 1.0;
    disk_params.inner_radius_au = 0.30;
    disk_params.outer_radius_au = 40.0;
    disk_params.disk_mass = 0.0001;

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

        let mut entity_cmds = commands.spawn((
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

        if name == "Jupiter" {
            entity_cmds.insert(AtmosphericStormState::jupiter());
        } else if name == "Neptune" {
            entity_cmds.insert(AtmosphericStormState::neptune());
        }
    }

    for i in 0..12 {
        let a_k = 18.0 + f64::from(i) * 1.8;
        let phi = f64::from(i) * 0.52;
        let v_circ = (G_ASTRO * 1.0 / a_k).sqrt();
        let pos = DVec3::new(a_k * phi.cos(), f64::from(i) * 0.2 - 1.2, a_k * phi.sin());
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

    let rogue = spawn_rogue_intruder(commands);
    (star, rogue)
}

fn spawn_little_red_dot_satellites(commands: &mut Commands, total_mass: f64, temp_k: f64) {
    let primordial_satellites: [(f64, f64, f64, &str, Composition, BodyType, f64); 6] = [
        (
            88.0,
            30.0,
            0.0006,
            "Orbiting Stellar Black Hole (Micro-Quasar α)",
            Composition::solar_gas(),
            BodyType::BlackHole,
            0.35,
        ),
        (
            120.0,
            55.0,
            0.048,
            "Pop-III Blue Supergiant (S-Cluster Star α)",
            Composition::solar_gas(),
            BodyType::BlueSupergiant,
            1.40,
        ),
        (
            155.0,
            0.0055,
            0.0006,
            "Extreme Super-Jupiter (Prime-b)",
            Composition::solar_gas(),
            BodyType::GasGiant,
            2.55,
        ),
        (
            190.0,
            32.0,
            0.032,
            "Pop-III Blue Giant (S-Cluster Star β)",
            Composition::solar_gas(),
            BodyType::BlueGiant,
            3.80,
        ),
        (
            225.0,
            0.00012,
            0.00032,
            "Primordial Volatile Ice World (Prime-c)",
            Composition {
                silicate_frac: 0.35,
                metal_frac: 0.10,
                ice_frac: 0.45,
                organics_frac: 0.05,
                gas_frac: 0.05,
            },
            BodyType::IceGiant,
            4.95,
        ),
        (
            260.0,
            1.5,
            0.006,
            "Pop-III Yellow Dwarf (S-Cluster Star γ)",
            Composition::solar_gas(),
            BodyType::YellowDwarf,
            5.85,
        ),
    ];

    for &(r_au, mass_s, rad_au, name, comp, b_type, phi_off) in &primordial_satellites {
        let v_circ = (G_ASTRO * total_mass / r_au).sqrt();
        let pos = DVec3::new(r_au * phi_off.cos(), 0.0, r_au * phi_off.sin());
        let vel = DVec3::new(-v_circ * phi_off.sin(), 0.0, v_circ * phi_off.cos());
        let temp = (temp_k * (r_au / 60.0).powf(-0.5)).max(50.0);

        commands.spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: b_type,
            },
            Mass(mass_s),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration(DVec3::ZERO),
            Radius(rad_au),
            Temperature(temp),
            comp,
            VolatileInventory::default(),
            SpinState {
                spin_vector: DVec3::new(0.0, 1e-10, 0.0),
                rotation_period_hours: 48.0,
                axial_tilt_degrees: 5.0,
            },
        ));
    }
}

/// Spawns the JWST Little Red Dot (Black Hole Star / Quasi-Star) scenario.
pub fn spawn_little_red_dot_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    let bh_core_mass = 400_000.0;
    let cocoon_mass = 50_000.0;
    let total_mass = bh_core_mass + cocoon_mass;
    let cocoon_radius_au = 60.0;
    let temp_k = 3800.0;

    disk_params.central_star_mass = total_mass;
    disk_params.inner_radius_au = 2.0;
    disk_params.outer_radius_au = 260.0;
    disk_params.reference_temp_1au = temp_k;
    disk_params.gas_disk_lifetime_yr = 20_000_000.0;
    disk_params.disk_mass = 500.0;

    let quasi_star_ent = commands
        .spawn((
            CelestialBody {
                name: "JWST Little Red Dot (Black Hole Star)".to_string(),
                body_type: BodyType::QuasiStar,
            },
            CentralStar,
            Mass(total_mass),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Radius(cocoon_radius_au),
            Temperature(temp_k),
            Luminosity(2.5e10),
            Composition::pure_hydrogen(),
            SpinState {
                spin_vector: DVec3::new(0.0, 1e-6, 0.0),
                rotation_period_hours: 120.0,
                axial_tilt_degrees: 0.0,
            },
            VolatileInventory::default(),
            IgnitionState {
                core_temperature: 5.0e7,
                fusion_fraction: 0.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
            BlackHoleStarState {
                black_hole_mass_solar: bh_core_mass,
                cocoon_mass_solar: cocoon_mass,
                cocoon_radius_au,
                eddington_ratio: 3.5,
                blowout_progress: 0.0,
                super_eddington_active: true,
                is_blown_out: false,
                accreted_envelope_mass: 0.0,
                jet_travel_distance_au: 0.0,
            },
        ))
        .insert(ElectromagneticFieldState {
            magnetic_field_gauss: 2.5e6,
            rotation_period_sec: 120.0 * 3600.0,
            magnetic_inclination_rad: 0.22,
            jet_length_au: 0.0,
            synchrotron_intensity: 1.0,
        })
        .id();

    spawn_little_red_dot_satellites(commands, total_mass, temp_k);
    quasi_star_ent
}

fn spawn_pulsar_planets(commands: &mut Commands, pulsar_mass: f64) {
    let zombie_planets: [(f64, f64, f64, &str, Composition, BodyType, f64, f64); 4] = [
        (
            0.19,
            0.020 * EARTH_MASS_SOLAR,
            0.35 * EARTH_RADIUS_AU,
            "Draugr (PSR B1257+12 b)",
            Composition::metal_rich(),
            BodyType::TerrestrialPlanet,
            0.45,
            0.0,
        ),
        (
            0.36,
            4.30 * EARTH_MASS_SOLAR,
            1.52 * EARTH_RADIUS_AU,
            "Poltergeist (PSR B1257+12 c)",
            Composition::rocky(),
            BodyType::SuperEarth,
            1.85,
            12.0,
        ),
        (
            0.46,
            3.90 * EARTH_MASS_SOLAR,
            1.48 * EARTH_RADIUS_AU,
            "Phobetor (PSR B1257+12 d)",
            Composition {
                silicate_frac: 0.50,
                organics_frac: 0.10,
                ice_frac: 0.15,
                metal_frac: 0.25,
                gas_frac: 0.0,
            },
            BodyType::SuperEarth,
            3.40,
            8.5,
        ),
        (
            1.10,
            0.080 * EARTH_MASS_SOLAR,
            0.48 * EARTH_RADIUS_AU,
            "Dagon (Outer Fallback Embryo)",
            Composition::icy(),
            BodyType::Protoplanet,
            5.10,
            0.05,
        ),
    ];

    for &(r_au, mass_s, rad_au, name, comp, b_type, phi_off, atm_bar) in &zombie_planets {
        let v_circ = (G_ASTRO * pulsar_mass / r_au).sqrt();
        let pos = DVec3::new(r_au * phi_off.cos(), 0.0, r_au * phi_off.sin());
        let vel = DVec3::new(-v_circ * phi_off.sin(), 0.0, v_circ * phi_off.cos());
        let temp = (280.0 * (1.0 / r_au.sqrt())).max(40.0);

        let mut spin = SpinState::default();
        let spin_rot = (mass_s * rad_au * rad_au * 0.33)
            * DVec3::new(0.0, 2.0 * std::f64::consts::PI / (28.0 / 8766.0), 0.0);
        spin.update_from_spin(spin_rot, mass_s, rad_au);

        commands.spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: b_type,
            },
            Mass(mass_s),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration(DVec3::ZERO),
            Radius(rad_au),
            Temperature(temp),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * mass_s),
            comp,
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: atm_bar as f32,
                cometary_impact_count: 0,
            },
            spin,
        ));
    }
}

struct CompactRemnantConfig {
    name: &'static str,
    body_type: BodyType,
    mass: f64,
    radius: f64,
    temperature: f64,
    luminosity: f64,
    spin_vector: DVec3,
    rotation_period_hours: f64,
    axial_tilt_degrees: f64,
    core_temperature: f64,
    evolution_phase: StellarEvolutionPhase,
    envelope_loss_rate: f64,
    phase_timer_years: f64,
    nebula_radius_au: f32,
    nebula_opacity: f32,
    em_field: ElectromagneticFieldState,
}

fn spawn_compact_remnant_star(commands: &mut Commands, config: CompactRemnantConfig) -> Entity {
    commands
        .spawn((
            CelestialBody {
                name: config.name.to_string(),
                body_type: config.body_type,
            },
            CentralStar,
            Mass(config.mass),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Radius(config.radius),
            Temperature(config.temperature),
            Luminosity(config.luminosity),
            Composition::pure_hydrogen(),
            SpinState {
                spin_vector: config.spin_vector,
                rotation_period_hours: config.rotation_period_hours,
                axial_tilt_degrees: config.axial_tilt_degrees,
            },
            VolatileInventory::default(),
            IgnitionState {
                core_temperature: config.core_temperature,
                fusion_fraction: 0.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState {
                phase: config.evolution_phase,
                hydrogen_core_fraction: 0.0,
                helium_core_fraction: 0.0,
                envelope_mass_loss_rate: config.envelope_loss_rate,
                phase_timer_years: config.phase_timer_years,
                nebula_expansion_radius_au: config.nebula_radius_au,
                nebula_opacity: config.nebula_opacity,
            },
        ))
        .insert(config.em_field)
        .id()
}

enum CompactRemnantPreset {
    Pulsar,
    Magnetar,
}

fn spawn_compact_remnant_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
    preset: CompactRemnantPreset,
) -> Entity {
    match preset {
        CompactRemnantPreset::Pulsar => {
            let pulsar_mass = 1.40;
            disk_params.central_star_mass = pulsar_mass;
            disk_params.inner_radius_au = 0.10;
            disk_params.outer_radius_au = 2.20;
            disk_params.reference_temp_1au = 280.0;
            disk_params.gas_disk_lifetime_yr = 10_000_000.0;
            disk_params.disk_mass = 0.0;

            let pulsar_ent = spawn_compact_remnant_star(
                commands,
                CompactRemnantConfig {
                    name: "PSR B1257+12 (Lich)",
                    body_type: BodyType::Pulsar,
                    mass: pulsar_mass,
                    radius: 0.00008,
                    temperature: 200_000.0,
                    luminosity: 5.2,
                    spin_vector: DVec3::new(0.0, 1.01e3, 0.0),
                    rotation_period_hours: 0.00622 / 3600.0,
                    axial_tilt_degrees: 20.0,
                    core_temperature: 1.0e8,
                    evolution_phase: StellarEvolutionPhase::NeutronStarPulsar,
                    envelope_loss_rate: 1e-12,
                    phase_timer_years: 1e9,
                    nebula_radius_au: 2.5,
                    nebula_opacity: 0.45,
                    em_field: ElectromagneticFieldState {
                        magnetic_field_gauss: 1.0e9,
                        rotation_period_sec: 0.00622,
                        magnetic_inclination_rad: 0.35,
                        jet_length_au: 2.5,
                        synchrotron_intensity: 2.5,
                    },
                },
            );

            spawn_pulsar_planets(commands, pulsar_mass);
            pulsar_ent
        }
        CompactRemnantPreset::Magnetar => {
            let magnetar_mass = 1.95;
            disk_params.central_star_mass = magnetar_mass;
            disk_params.inner_radius_au = 0.20;
            disk_params.outer_radius_au = 22.0;
            disk_params.reference_temp_1au = 1200.0;
            disk_params.gas_disk_lifetime_yr = 5_000_000.0;
            disk_params.disk_mass = 0.0;

            let magnetar_ent = spawn_compact_remnant_star(
                commands,
                CompactRemnantConfig {
                    name: "SGR 1806-20 (Magnetar)",
                    body_type: BodyType::Magnetar,
                    mass: magnetar_mass,
                    radius: 0.000_075,
                    temperature: 5_500_000.0,
                    luminosity: 10_000.0,
                    spin_vector: DVec3::new(0.0, 0.83, 0.0),
                    rotation_period_hours: 7.56 / 3600.0,
                    axial_tilt_degrees: 26.0,
                    core_temperature: 2.5e8,
                    evolution_phase: StellarEvolutionPhase::MagnetarRemnant,
                    envelope_loss_rate: 1e-10,
                    phase_timer_years: 1e5,
                    nebula_radius_au: 5.5,
                    nebula_opacity: 0.65,
                    em_field: ElectromagneticFieldState {
                        magnetic_field_gauss: 1.0e15,
                        rotation_period_sec: 7.56,
                        magnetic_inclination_rad: 0.45,
                        jet_length_au: 4.5,
                        synchrotron_intensity: 5.0,
                    },
                },
            );

            spawn_magnetar_cluster_bodies(commands, magnetar_mass);
            magnetar_ent
        }
    }
}

/// Spawns the PSR B1257+12 (Lich) Millisecond Pulsar System with 3 confirmed zombie exoplanets.
pub fn spawn_pulsar_system_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    spawn_compact_remnant_scenario(commands, disk_params, CompactRemnantPreset::Pulsar)
}

fn spawn_magnetar_cluster_bodies(commands: &mut Commands, magnetar_mass: f64) {
    let cluster_bodies: [(f64, f64, f64, &str, Composition, BodyType, f64, f64); 4] = [
        (
            0.48,
            0.85 * EARTH_MASS_SOLAR,
            0.88 * EARTH_RADIUS_AU,
            "Valkyrie (Shattered Iron Core)",
            Composition::metal_rich(),
            BodyType::TerrestrialPlanet,
            0.85,
            1800.0,
        ),
        (
            0.85,
            2.40 * EARTH_MASS_SOLAR,
            1.25 * EARTH_RADIUS_AU,
            "Pyre (Chthonian Magma World)",
            Composition::rocky(),
            BodyType::SuperEarth,
            2.45,
            1350.0,
        ),
        (
            1.65,
            0.05 * EARTH_MASS_SOLAR,
            0.40 * EARTH_RADIUS_AU,
            "SGR Ejecta Clump α",
            Composition::metal_rich(),
            BodyType::Protoplanet,
            3.90,
            780.0,
        ),
        (
            18.0,
            45.0,
            0.22,
            "LBV 1806-20 (Hypergiant Companion)",
            Composition::solar_gas(),
            BodyType::BlueSupergiant,
            5.20,
            28_000.0,
        ),
    ];

    for &(r_au, mass_s, rad_au, name, comp, b_type, phi_off, temp) in &cluster_bodies {
        let v_circ = (G_ASTRO * magnetar_mass / r_au).sqrt();
        let pos = DVec3::new(r_au * phi_off.cos(), 0.0, r_au * phi_off.sin());
        let vel = DVec3::new(-v_circ * phi_off.sin(), 0.0, v_circ * phi_off.cos());

        let mut spin = SpinState::default();
        let spin_rot = (mass_s * rad_au * rad_au * 0.33)
            * DVec3::new(0.0, 2.0 * std::f64::consts::PI / (36.0 / 8766.0), 0.0);
        spin.update_from_spin(spin_rot, mass_s, rad_au);

        commands.spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: b_type,
            },
            Mass(mass_s),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration(DVec3::ZERO),
            Radius(rad_au),
            Temperature(temp),
            Luminosity(if b_type.is_star_or_remnant() {
                1_500_000.0
            } else {
                0.0
            }),
            AngularMomentum(pos.cross(vel) * mass_s),
            comp,
            VolatileInventory::default(),
            spin,
        ));
    }
}

/// Spawns the SGR 1806-20 Ultra-Magnetized Magnetar Scenario with hypergiant companion and relativistic ejecta.
pub fn spawn_magnetar_outburst_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    spawn_compact_remnant_scenario(commands, disk_params, CompactRemnantPreset::Magnetar)
}

/// Spawns the PSR B1913+16 Hulse-Taylor Relativistic Binary Pulsar scenario exhibiting 4.22°/yr periastron advance and GW inspiral.
pub fn spawn_relativistic_binary_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> (Entity, Entity) {
    disk_params.central_star_mass = 1.44;
    disk_params.inner_radius_au = 0.005;
    disk_params.outer_radius_au = 0.050;
    disk_params.disk_mass = 0.0;

    let m1 = 1.44; // Pulsar mass in Solar Masses
    let m2 = 1.38; // Companion neutron star mass
    let a_au = 0.013; // Semi-major axis in AU (~1.95 million km)
    let ecc = 0.617; // Famous Hulse-Taylor eccentricity

    let ns_radius = 10.0 * 1000.0 / AU_TO_METERS; // ~10 km radius in AU (~6.68e-8 AU)
    let r_periastron = a_au * (1.0 - ecc);

    // Vis-viva equation at periastron: v_p = sqrt(G * (m1 + m2) * (2/r_p - 1/a))
    let mu = G_ASTRO * (m1 + m2);
    let v_p = (mu * (2.0 / r_periastron - 1.0 / a_au)).sqrt();

    let pulsar_ent = commands
        .spawn((
            CelestialBody {
                name: "PSR B1913+16 (Pulsar Primary)".to_string(),
                body_type: BodyType::Pulsar,
            },
            CentralStar,
            Mass(m1),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(ns_radius),
            Temperature(1_000_000.0),
            Luminosity(0.05),
            AngularMomentum(DVec3::ZERO),
            Composition::metal_rich(),
            SpinState {
                rotation_period_hours: 0.059 / 3600.0, // 59 milliseconds
                axial_tilt_degrees: 15.0,
                spin_vector: DVec3::new(0.0, 1.0, 0.0),
            },
            ElectromagneticFieldState {
                magnetic_field_gauss: 1e12,
                rotation_period_sec: 0.059,
                magnetic_inclination_rad: 0.25,
                jet_length_au: 0.005,
                synchrotron_intensity: 1.0,
            },
            RelativisticState::compact_binary(m1, m2, a_au, ecc),
        ))
        .id();

    let companion_ent = commands
        .spawn((
            CelestialBody {
                name: "PSR B1913+16 Companion (Neutron Star)".to_string(),
                body_type: BodyType::NeutronStar,
            },
            Mass(m2),
            SimPosition(DVec3::new(r_periastron, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_p)),
            SimAcceleration::default(),
            Radius(ns_radius),
            Temperature(500_000.0),
            Luminosity(0.01),
            AngularMomentum(DVec3::new(0.0, r_periastron * v_p * m2, 0.0)),
            Composition::metal_rich(),
            SpinState {
                rotation_period_hours: 1.2,
                axial_tilt_degrees: 5.0,
                spin_vector: DVec3::new(0.0, 1.0, 0.0),
            },
            RelativisticState::compact_binary(m1, m2, a_au, ecc),
        ))
        .id();

    (pulsar_ent, companion_ent)
}

/// Spawns the Kozai-Lidov Hierarchical Triple Scenario (Secular Resonance & Hot Jupiter Migration).
pub fn spawn_kozai_triple_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> (Entity, Entity, Entity) {
    disk_params.central_star_mass = 1.0;
    disk_params.inner_radius_au = 0.5;
    disk_params.outer_radius_au = 120.0;
    disk_params.disk_mass = 0.0001;

    let m_star = 1.0;
    let m_planet = 1.2 * JUPITER_MASS_SOLAR;
    let m_companion = 0.35; // M-dwarf companion star

    let a_inner = 3.0; // Inner gas giant semi-major axis in AU
    let inc_deg: f64 = 68.0;
    let inc_rad = inc_deg.to_radians();

    let a_outer = 90.0; // Distant binary companion in AU

    // Circular Keplerian velocity at a_inner: v = sqrt(G * M / a)
    let v_inner_mag = (G_ASTRO * m_star / a_inner).sqrt();
    let r_inner = DVec3::new(a_inner, 0.0, 0.0);
    let v_inner = DVec3::new(
        0.0,
        v_inner_mag * inc_rad.sin(),
        v_inner_mag * inc_rad.cos(),
    );

    // Outer companion in the XZ plane (i = 0)
    let v_outer_mag = (G_ASTRO * (m_star + m_companion) / a_outer).sqrt();
    let r_outer = DVec3::new(a_outer, 0.0, 0.0);
    let v_outer = DVec3::new(0.0, 0.0, v_outer_mag);

    let star_ent = commands
        .spawn((
            CelestialBody {
                name: "HD 80606 Analog Primary (G-Dwarf)".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            CentralStar,
            Mass(m_star),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(SOLAR_RADIUS_AU),
            Temperature(5600.0),
            Luminosity(0.95),
            AngularMomentum(DVec3::ZERO),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1.4e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id();

    let companion_ent = commands
        .spawn((
            CelestialBody {
                name: "HD 80606B Perturber (M-Dwarf)".to_string(),
                body_type: BodyType::RedDwarf,
            },
            Mass(m_companion),
            SimPosition(r_outer),
            SimVelocity(v_outer),
            SimAcceleration::default(),
            Radius(SOLAR_RADIUS_AU * 0.45),
            Temperature(3300.0),
            Luminosity(0.015),
            AngularMomentum(r_outer.cross(v_outer) * m_companion),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 8.0e6,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id();

    let h_inner = r_inner.cross(v_inner);
    let h_outer = r_outer.cross(v_outer);
    let mut_inc_rad = crate::simulation::kozai_lidov::compute_mutual_inclination(h_inner, h_outer);
    let e_max = crate::simulation::kozai_lidov::compute_max_eccentricity(0.01, mut_inc_rad);
    let q_min = crate::simulation::kozai_lidov::compute_min_periastron(a_inner, e_max);
    let tau_kl = crate::simulation::kozai_lidov::compute_kozai_timescale(
        a_inner,
        a_outer,
        m_star,
        m_companion,
        0.0,
    );

    let planet_ent = commands
        .spawn((
            CelestialBody {
                name: "HD 80606b Inner Giant (Kozai)".to_string(),
                body_type: BodyType::GasGiant,
            },
            Mass(m_planet),
            SimPosition(r_inner),
            SimVelocity(v_inner),
            SimAcceleration::default(),
            Radius(EARTH_RADIUS_AU * 11.2),
            Temperature(280.0),
            Luminosity(0.0),
            AngularMomentum(r_inner.cross(v_inner) * m_planet),
            Composition::solar_gas(),
            SpinState {
                rotation_period_hours: 14.0,
                axial_tilt_degrees: 68.0,
                spin_vector: DVec3::new(0.0, inc_rad.cos(), inc_rad.sin()).normalize(),
            },
            crate::simulation::kozai_lidov::KozaiLidovState {
                perturber_entity: Some(companion_ent),
                perturber_name: "HD 80606B Perturber (M-Dwarf)".to_string(),
                mutual_inclination_deg: mut_inc_rad.to_degrees(),
                critical_inclination_deg: crate::simulation::kozai_lidov::KOZAI_CRITICAL_ANGLE_DEG,
                is_in_resonance: true,
                max_eccentricity_forecast: e_max,
                min_periastron_au: q_min,
                kozai_period_years: tau_kl,
                gr_precession_ratio: 0.0,
                is_gr_suppressed: false,
                regime: crate::simulation::kozai_lidov::KozaiRegime::Circulation,
                cycle_phase: 0.0,
            },
            crate::simulation::tides::TidalState::new_gas_giant(),
            AtmosphericStormState::jupiter(),
        ))
        .id();

    (star_ent, planet_ent, companion_ent)
}
