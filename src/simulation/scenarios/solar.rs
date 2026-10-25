//! Hayashi Minimum Mass Solar Nebula (MMSN) scenario.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::relativity::RelativisticState;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;
use crate::utils::constants::*;

type MmsnSeed = (f64, f64, f64, &'static str, Composition, BodyType, f64);

fn get_mmsn_inner_seeds() -> [MmsnSeed; 13] {
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
            1.00 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.00,
            "Earth",
            Composition::rocky(),
            BodyType::TerrestrialPlanet,
            0.0,
        ),
        (
            1.18,
            0.12 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.53,
            "Theia",
            Composition {
                metal_frac: 0.42,
                silicate_frac: 0.58,
                ice_frac: 0.0,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            BodyType::Protoplanet,
            0.08,
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
            1.85,
        ),
        // Canonical Deep Outer Planet Nine (Hypothetical Super-Earth / Ice Giant Shepherding the Oort Cloud)
        (
            380.00,
            5.50 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 2.30,
            "Planet Nine (Super-Earth / Ice Giant)",
            Composition::icy(),
            BodyType::IceGiant,
            4.10,
        ),
    ]
}

fn get_mmsn_outer_seeds() -> [MmsnSeed; 14] {
    [
        // Canonical Asteroid Belt Minor Planets (Silicate, Carbonaceous, Metal)
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
        // Canonical Kuiper Belt & Trans-Neptunian Cometary Reservoir (Volatile Ices)
        (
            17.80,
            3.7e-11 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.000_863,
            "1P/Halley (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.65,
        ),
        (
            3.30,
            2.5e-12 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.000_377,
            "2P/Encke (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.85,
        ),
        (
            18.50,
            1.7e-12 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.000_314,
            "67P/C-G (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.64,
        ),
        (
            28.40,
            2.2e-9 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.00471,
            "Hale-Bopp (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.99,
        ),
        (
            25.20,
            1.7e-10 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.00204,
            "Swift-Tuttle (Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.96,
        ),
        (
            13.70,
            8.4e-7 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.0171,
            "Chiron (Centaur Comet)",
            Composition::icy(),
            BodyType::Comet,
            0.38,
        ),
    ]
}

fn spawn_solar_nebula_protostar(commands: &mut Commands) -> Entity {
    commands
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
        .id()
}

fn compute_mmsn_seed_kinematics(name: &str, r_au: f64, phi_off: f64) -> (DVec3, DVec3) {
    if name == "Theia" {
        let ecc = 0.165;
        let peri_angle = 0.35;
        let true_anomaly = phi_off - peri_angle;
        let r_theia = (r_au * (1.0 - ecc * ecc)) / (1.0 + ecc * true_anomaly.cos());
        let pos_theia = DVec3::new(r_theia * phi_off.cos(), 0.0, r_theia * phi_off.sin());
        let p_orb = r_au * (1.0 - ecc * ecc);
        let h = (G_ASTRO * 1.0 * p_orb).sqrt();
        let v_r = (G_ASTRO * 1.0 / h) * ecc * true_anomaly.sin();
        let v_theta = (G_ASTRO * 1.0 / h) * (1.0 + ecc * true_anomaly.cos());
        let vel_theia = DVec3::new(
            v_r * phi_off.cos() - v_theta * phi_off.sin(),
            0.0,
            v_r * phi_off.sin() + v_theta * phi_off.cos(),
        );
        (pos_theia, vel_theia)
    } else {
        let v_circ = (G_ASTRO * 1.0 / r_au).sqrt();
        let pos = DVec3::new(r_au * phi_off.cos(), 0.0, r_au * phi_off.sin());
        let vel = DVec3::new(-v_circ * phi_off.sin(), 0.0, v_circ * phi_off.cos());
        (pos, vel)
    }
}

fn get_mmsn_rotation_and_tilt(name: &str) -> (f64, f64) {
    if name.contains("Mercury") {
        (1407.6, 0.03)
    } else if name.contains("Venus") {
        (5832.5, 177.4)
    } else if name.contains("Earth") {
        (24.0, 23.44)
    } else if name.contains("Theia") {
        (20.0, 15.0)
    } else if name.contains("Mars") {
        (24.62, 25.19)
    } else if name.contains("Jupiter") {
        (9.93, 3.13)
    } else if name.contains("Saturn") {
        (10.7, 26.73)
    } else if name.contains("Uranus") {
        (17.24, 97.77)
    } else if name.contains("Neptune") {
        (16.11, 28.32)
    } else if name.contains("Pluto") {
        (153.3, 122.5)
    } else if name.contains("Planet Nine") {
        (12.0, 20.0)
    } else if name.contains("Ceres") {
        (9.07, 4.0)
    } else if name.contains("Vesta") {
        (5.34, 29.0)
    } else {
        (24.0, 5.0)
    }
}

fn attach_mmsn_special_components(entity_cmds: &mut EntityCommands, name: &str, rad_au: f64) {
    if name.contains("Saturn") {
        entity_cmds.insert(PlanetaryRingSystem {
            inner_radius_au: (rad_au * 1.25) as f32,
            outer_radius_au: (rad_au * 2.35) as f32,
            ring_mass_earth: 0.000_028,
            optical_depth: 0.88,
            ice_fraction: 0.96,
            silicate_fraction: 0.04,
        });
        entity_cmds.insert(AtmosphericStormState::saturn());
    }

    if name.contains("Jupiter") {
        entity_cmds.insert(AtmosphericStormState::jupiter());
    }

    if name.contains("Neptune") {
        entity_cmds.insert(AtmosphericStormState::neptune());
    }

    if name.contains("Mercury") {
        entity_cmds.insert(TidalState {
            host_entity: None,
            love_number_k2: 0.30,
            tidal_q: 80.0,
            is_tidally_locked: true,
            locking_progress: 1.0,
            resonance_ratio: 1.50,
            tidal_heating_power_watts: 2.2e12,
            tidal_heating_flux_w_m2: 0.03,
            circularization_rate_per_myr: -0.001,
            circularization_timescale_yr: 1e8,
            sync_timescale_yr: 1e5,
        });
        entity_cmds.insert(RelativisticState::mercury_like());
    }
}

/// Spawns the Hayashi Minimum Mass Solar Nebula (MMSN) scenario.
pub fn spawn_solar_nebula_mmsn(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    disk_params.central_star_mass = 1.0;
    disk_params.inner_radius_au = 0.20;
    disk_params.outer_radius_au = 45.0;
    disk_params.disk_mass = 0.00010;

    let star = spawn_solar_nebula_protostar(commands);

    let all_seeds = get_mmsn_inner_seeds()
        .into_iter()
        .chain(get_mmsn_outer_seeds());

    let mut jupiter_info = None;
    let mut saturn_info = None;

    for (r_au, mass_s, rad_au, name, comp, b_type, phi_off) in all_seeds {
        let (pos, vel) = compute_mmsn_seed_kinematics(name, r_au, phi_off);

        let mut diff = InternalDifferentiation::default();
        diff.recalculate(mass_s, rad_au, &comp);

        let (period_hours, tilt_degrees) = get_mmsn_rotation_and_tilt(name);
        let tilt_rad = tilt_degrees.to_radians();
        let spin_dir = DVec3::new(tilt_rad.sin(), tilt_rad.cos(), 0.0);
        let period_yr = period_hours * 3600.0 / YEAR_SECONDS;
        let omega = 2.0 * PI / period_yr.max(1e-8);
        let i_moment = 0.33 * mass_s * rad_au * rad_au;
        let initial_spin = (i_moment * omega) * spin_dir;
        let mut spin = SpinState::default();
        spin.update_from_spin(initial_spin, mass_s, rad_au);

        let vol = VolatileInventory {
            delivered_water_m_earth: 0.0,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: if r_au < 2.7 { 0.10 } else { 0.0 },
            cometary_impact_count: 0,
        };

        let mut entity_cmds = commands.spawn((
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
            diff,
            spin,
            vol,
        ));

        attach_mmsn_special_components(&mut entity_cmds, name, rad_au);
        let spawned_id = entity_cmds.id();

        if name.contains("Jupiter") {
            jupiter_info = Some((spawned_id, pos, vel, mass_s));
        } else if name.contains("Saturn") {
            saturn_info = Some((spawned_id, pos, vel, mass_s));
        }
    }

    spawn_mmsn_satellites(commands, jupiter_info, saturn_info);

    star
}

fn spawn_mmsn_satellites(
    commands: &mut Commands,
    jupiter_info: Option<(Entity, DVec3, DVec3, f64)>,
    saturn_info: Option<(Entity, DVec3, DVec3, f64)>,
) {
    if let Some((j_ent, j_pos, j_vel, j_mass)) = jupiter_info {
        spawn_jupiter_satellites(commands, j_ent, j_pos, j_vel, j_mass);
    }

    if let Some((s_ent, s_pos, s_vel, s_mass)) = saturn_info {
        spawn_saturn_satellites(commands, s_ent, s_pos, s_vel, s_mass);
    }
}

struct SatelliteMoonParams<'a> {
    name: &'a str,
    semi_major_axis_au: f64,
    mass: f64,
    radius: f64,
    temperature: f64,
    composition: Composition,
    initial_anomaly: f64,
    volcanic_state: Option<VolcanicState>,
    tidal_state: Option<TidalState>,
}

fn spawn_satellite_moon(
    commands: &mut Commands,
    parent_ent: Entity,
    parent_pos: DVec3,
    parent_vel: DVec3,
    parent_mass: f64,
    params: SatelliteMoonParams,
) {
    let a = params.semi_major_axis_au;
    let period_yr = 2.0 * PI * (a * a * a / (G_ASTRO * parent_mass.max(1e-8))).sqrt();
    let v_circ = (G_ASTRO * parent_mass.max(1e-8) / a).sqrt();
    let (sin_anom, cos_anom) = params.initial_anomaly.sin_cos();
    let pos = parent_pos + DVec3::new(a * cos_anom, 0.0, a * sin_anom);
    let vel = parent_vel + DVec3::new(-v_circ * sin_anom, 0.0, v_circ * cos_anom);

    let mut diff = InternalDifferentiation::default();
    diff.recalculate(params.mass, params.radius, &params.composition);

    let mut ent_cmd = commands.spawn((
        CelestialBody {
            body_type: BodyType::Moon,
            name: params.name.to_string(),
        },
        Mass(params.mass),
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration::default(),
        Radius(params.radius),
        Temperature(params.temperature),
        Luminosity(0.0),
        AngularMomentum(pos.cross(vel) * params.mass),
        params.composition,
        diff,
        SpinState::default(),
        VolatileInventory::default(),
    ));

    ent_cmd.insert(SatelliteOf {
        parent: parent_ent,
        semi_major_axis_au: a,
        orbital_period_years: period_yr,
        true_anomaly: params.initial_anomaly,
    });

    if let Some(volc) = params.volcanic_state {
        ent_cmd.insert(volc);
    }
    if let Some(tidal) = params.tidal_state {
        ent_cmd.insert(tidal);
    }
}

fn spawn_jupiter_satellites(
    commands: &mut Commands,
    j_ent: Entity,
    j_pos: DVec3,
    j_vel: DVec3,
    j_mass: f64,
) {
    // 1. Io (Hyperactive Sulfur Volcanism)
    spawn_satellite_moon(
        commands,
        j_ent,
        j_pos,
        j_vel,
        j_mass,
        SatelliteMoonParams {
            name: "Io (Volcanic Moon)",
            semi_major_axis_au: 0.0068,
            mass: 0.015 * EARTH_MASS_SOLAR,
            radius: EARTH_RADIUS_AU * 0.286,
            temperature: 130.0,
            composition: Composition::rocky(),
            initial_anomaly: PI - 0.12,
            volcanic_state: Some(VolcanicState::io()),
            tidal_state: Some(TidalState {
                host_entity: Some(j_ent),
                love_number_k2: 0.15,
                tidal_q: 36.0,
                is_tidally_locked: true,
                locking_progress: 1.0,
                resonance_ratio: 1.0,
                tidal_heating_power_watts: 1.0e14,
                tidal_heating_flux_w_m2: 2.5,
                circularization_rate_per_myr: 0.0,
                circularization_timescale_yr: 1e6,
                sync_timescale_yr: 1e4,
            }),
        },
    );

    // 2. Europa (Cryotectonic Water-Ice World)
    spawn_satellite_moon(
        commands,
        j_ent,
        j_pos,
        j_vel,
        j_mass,
        SatelliteMoonParams {
            name: "Europa (Cryotectonic Moon)",
            semi_major_axis_au: 0.0095,
            mass: 0.008 * EARTH_MASS_SOLAR,
            radius: EARTH_RADIUS_AU * 0.245,
            temperature: 102.0,
            composition: Composition::icy(),
            initial_anomaly: PI + 0.10,
            volcanic_state: Some(VolcanicState::europa()),
            tidal_state: Some(TidalState {
                host_entity: Some(j_ent),
                love_number_k2: 0.25,
                tidal_q: 100.0,
                is_tidally_locked: true,
                locking_progress: 1.0,
                resonance_ratio: 1.0,
                tidal_heating_power_watts: 3.5e12,
                tidal_heating_flux_w_m2: 0.12,
                circularization_rate_per_myr: 0.0,
                circularization_timescale_yr: 1e7,
                sync_timescale_yr: 1e5,
            }),
        },
    );

    // 3. Ganymede (Magnetic Giant Water-Ice World)
    spawn_satellite_moon(
        commands,
        j_ent,
        j_pos,
        j_vel,
        j_mass,
        SatelliteMoonParams {
            name: "Ganymede (Magnetic Moon)",
            semi_major_axis_au: 0.0130,
            mass: 0.025 * EARTH_MASS_SOLAR,
            radius: EARTH_RADIUS_AU * 0.413,
            temperature: 110.0,
            composition: Composition::icy(),
            initial_anomaly: PI - 0.28,
            volcanic_state: None,
            tidal_state: Some(TidalState {
                host_entity: Some(j_ent),
                love_number_k2: 0.20,
                tidal_q: 120.0,
                is_tidally_locked: true,
                locking_progress: 1.0,
                resonance_ratio: 1.0,
                tidal_heating_power_watts: 8.0e11,
                tidal_heating_flux_w_m2: 0.04,
                circularization_rate_per_myr: 0.0,
                circularization_timescale_yr: 2e7,
                sync_timescale_yr: 2e5,
            }),
        },
    );

    // 4. Callisto (Heavily Cratered Ice-Rock World)
    spawn_satellite_moon(
        commands,
        j_ent,
        j_pos,
        j_vel,
        j_mass,
        SatelliteMoonParams {
            name: "Callisto (Cratered Moon)",
            semi_major_axis_au: 0.0175,
            mass: 0.018 * EARTH_MASS_SOLAR,
            radius: EARTH_RADIUS_AU * 0.378,
            temperature: 120.0,
            composition: Composition {
                metal_frac: 0.05,
                silicate_frac: 0.48,
                ice_frac: 0.47,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            initial_anomaly: PI + 0.32,
            volcanic_state: None,
            tidal_state: Some(TidalState {
                host_entity: Some(j_ent),
                love_number_k2: 0.18,
                tidal_q: 150.0,
                is_tidally_locked: true,
                locking_progress: 1.0,
                resonance_ratio: 1.0,
                tidal_heating_power_watts: 1.0e11,
                tidal_heating_flux_w_m2: 0.01,
                circularization_rate_per_myr: 0.0,
                circularization_timescale_yr: 5e7,
                sync_timescale_yr: 5e5,
            }),
        },
    );
}

fn spawn_saturn_satellites(
    commands: &mut Commands,
    s_ent: Entity,
    s_pos: DVec3,
    s_vel: DVec3,
    s_mass: f64,
) {
    // Enceladus (Cryovolcanic Water-Ice Geysers Feeding E-Ring)
    spawn_satellite_moon(
        commands,
        s_ent,
        s_pos,
        s_vel,
        s_mass,
        SatelliteMoonParams {
            name: "Enceladus (Cryo Moon)",
            semi_major_axis_au: 0.0055,
            mass: 1.8e-5 * EARTH_MASS_SOLAR,
            radius: EARTH_RADIUS_AU * 0.0395,
            temperature: 75.0,
            composition: Composition::icy(),
            initial_anomaly: PI * 0.95,
            volcanic_state: Some(VolcanicState::enceladus()),
            tidal_state: Some(TidalState {
                host_entity: Some(s_ent),
                love_number_k2: 0.02,
                tidal_q: 20.0,
                is_tidally_locked: true,
                locking_progress: 1.0,
                resonance_ratio: 1.0,
                tidal_heating_power_watts: 5.0e9,
                tidal_heating_flux_w_m2: 0.08,
                circularization_rate_per_myr: 0.0,
                circularization_timescale_yr: 1e6,
                sync_timescale_yr: 1e4,
            }),
        },
    );
}
