//! Spawners and objective configurations for Campaign Scenarios.

use bevy::math::DVec3;
use bevy::prelude::*;

use super::types::*;
use crate::simulation::components::planetary::FloraPigmentClass;
use crate::simulation::components::*;
use crate::simulation::probes::{FlightComputer, ProbeType, SpaceProbe};
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Generates the initial objective checklist for a given campaign mission.
pub fn create_mission_objectives(mission: CampaignMissionId) -> Vec<MissionObjective> {
    match mission {
        CampaignMissionId::TheStellarForge => vec![
            MissionObjective::new(
                "forge_mass",
                "Protostar Accretion",
                "Accrete mass into the brown dwarf until M >= 0.080 M☉",
                0.080,
                "M☉",
                true,
            ),
            MissionObjective::new(
                "forge_ignition",
                "Core Fusion Ignition",
                "Trigger thermonuclear hydrogen burning in the stellar core",
                1.0,
                "",
                true,
            ),
            MissionObjective::new(
                "forge_preserve_embryos",
                "Preserve Rocky Embryos",
                "Keep both inner rocky protoplanets from being destroyed or ejected",
                2.0,
                "Embryos",
                true,
            ),
        ],
        CampaignMissionId::GenesisWorld => vec![
            MissionObjective::new(
                "genesis_temp",
                "Habitable Temperature",
                "Warm surface temperature into the liquid water window (273 - 320 K)",
                273.0,
                "K",
                true,
            ),
            MissionObjective::new(
                "genesis_oceans",
                "Hydrosphere Condensation",
                "Deliver volatile water ice until surface ocean coverage >= 20%",
                0.20,
                "%",
                true,
            ),
            MissionObjective::new(
                "genesis_biosphere",
                "Sustained Biosphere",
                "Maintain liquid oceans and habitable climate for at least 50 Myr",
                5.0e7,
                "Myr",
                true,
            ),
        ],
        CampaignMissionId::TheGrandTour => vec![
            MissionObjective::new(
                "tour_flybys",
                "Planetary Flybys",
                "Execute close-proximity flybys (< 0.50 AU) of at least 4 distinct major planets",
                4.0,
                "Planets",
                true,
            ),
            MissionObjective::new(
                "tour_probe_survival",
                "Probe Integrity",
                "Ensure Voyager III does not crash or disintegrate",
                1.0,
                "",
                true,
            ),
        ],
        CampaignMissionId::Exodus => vec![
            MissionObjective::new(
                "exodus_evacuate",
                "Escape Beyond 200 AU",
                "Propel at least 2 Ark Colony Ships beyond 200 AU into interstellar space",
                2.0,
                "Ships",
                true,
            ),
            MissionObjective::new(
                "exodus_outrun_supernova",
                "Outrun Core Collapse",
                "Complete evacuation before Betelgeuse Secundus goes supernova",
                1.0,
                "",
                true,
            ),
        ],
    }
}

/// Spawns the entire planetary system configured for a campaign mission.
pub fn spawn_campaign_mission(
    mission: CampaignMissionId,
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
    config: &mut SimulationConfig,
) -> Option<Entity> {
    match mission {
        CampaignMissionId::TheStellarForge => {
            Some(spawn_stellar_forge_mission(commands, disk_params, config))
        }
        CampaignMissionId::GenesisWorld => {
            Some(spawn_genesis_world_mission(commands, disk_params, config))
        }
        CampaignMissionId::TheGrandTour => {
            Some(spawn_grand_tour_mission(commands, disk_params, config))
        }
        CampaignMissionId::Exodus => Some(spawn_exodus_mission(commands, disk_params, config)),
    }
}

fn spawn_stellar_forge_mission(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
    config: &mut SimulationConfig,
) -> Entity {
    disk_params.disk_mass = 0.045;
    disk_params.gas_disk_lifetime_yr = 5.0e6;
    config.gas_density_scale = 1.0;

    // Sub-stellar brown dwarf at origin (0.050 M☉, below 0.080 hydrogen burning limit)
    let central_star = commands
        .spawn((
            CelestialBody {
                name: "Protostar Forge Alpha".to_string(),
                body_type: BodyType::Protostar,
            },
            CentralStar,
            Mass(0.050),
            Radius(0.12),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Temperature(1800.0),
            Luminosity(0.01),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1e5,
                fusion_fraction: 0.0,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
        ))
        .id();

    // 2 Inner rocky protoplanets to preserve
    let mu = G_ASTRO * 0.050;
    let embryos = [
        (
            "Forge Rocky Embryo I",
            0.40,
            0.8 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.90,
        ),
        (
            "Forge Rocky Embryo II",
            0.75,
            1.2 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 1.05,
        ),
    ];
    for (name, r, m, rad) in embryos {
        let v = (mu / r).sqrt();
        commands.spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: BodyType::Protoplanet,
            },
            Mass(m),
            Radius(rad),
            SimPosition(DVec3::new(r, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v)),
            SimAcceleration(DVec3::ZERO),
            Temperature(350.0),
            Composition::rocky(),
        ));
    }

    // 4 Massive gas planetesimals primed for accretion feeding
    let feed_planetesimals = [
        ("Accretion Clump I", 1.5, 0.010),
        ("Accretion Clump II", 2.2, 0.011),
        ("Accretion Clump III", 3.1, 0.012),
        ("Accretion Clump IV", 4.2, 0.012),
    ];
    for (name, r, m) in feed_planetesimals {
        let v_circ = (mu / r).sqrt();
        // Slightly sub-Keplerian velocity so gas drag assists inward migration
        commands.spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: BodyType::Planetesimal,
            },
            Mass(m),
            Radius(EARTH_RADIUS_AU * 4.0),
            SimPosition(DVec3::new(r, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_circ * 0.96)),
            SimAcceleration(DVec3::ZERO),
            Temperature(120.0),
            Composition::solar_gas(),
        ));
    }

    central_star
}

fn spawn_genesis_world_mission(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
    config: &mut SimulationConfig,
) -> Entity {
    disk_params.disk_mass = 0.0;
    config.gas_density_scale = 0.0;

    let central_star = commands
        .spawn((
            CelestialBody {
                name: "Helios Primus".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            CentralStar,
            Mass(0.95),
            Radius(SOLAR_RADIUS_AU * 0.95),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Temperature(5500.0),
            Luminosity(0.85),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1.5e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 10.0,
            },
        ))
        .id();

    // Frozen candidate world: "Candidate Genesis"
    let mu = G_ASTRO * 0.95;
    let r = 1.10;
    let v = (mu / r).sqrt();
    commands.spawn((
        CelestialBody {
            name: "Candidate Genesis".to_string(),
            body_type: BodyType::TerrestrialPlanet,
        },
        Mass(EARTH_MASS_SOLAR),
        Radius(EARTH_RADIUS_AU),
        SimPosition(DVec3::new(r, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, v)),
        SimAcceleration(DVec3::ZERO),
        Temperature(160.0),
        Composition {
            metal_frac: 0.32,
            silicate_frac: 0.64,
            ice_frac: 0.04,
            organics_frac: 0.0,
            gas_frac: 0.0,
        },
        PlanetaryClimate {
            surface_temperature_k: 160.0,
            equilibrium_temperature_k: 155.0,
            greenhouse_delta_k: 5.0,
            albedo: 0.70,
            ice_coverage_frac: 0.95,
            cloud_coverage_frac: 0.10,
            climate_regime: ClimateRegime::SnowballIceAge,
            polar_ice_cap_latitude_deg: 10.0,
        },
        VolatileInventory {
            delivered_water_m_earth: 0.04,
            ocean_coverage_frac: 0.04,
            atmospheric_pressure_bar: 0.02,
            cometary_impact_count: 0,
        },
        ElectromagneticFieldState {
            magnetic_field_gauss: 0.45,
            rotation_period_sec: 24.0 * 3600.0,
            magnetic_inclination_rad: 0.18,
            jet_length_au: 0.0,
            synchrotron_intensity: 0.0,
        },
    ));

    // Outer water ice delivery reservoirs (eccentric cometary impactors)
    let comets = [
        ("Icy Comet C/2026-A", 2.2, 0.50),
        ("Icy Comet C/2026-B", 2.8, 0.58),
        ("Icy Comet C/2026-C", 3.4, 0.65),
        ("Icy Comet C/2026-D", 4.1, 0.70),
    ];
    for (name, a, ecc) in comets {
        let r_init = a * (1.0 + ecc);
        let v_ap = (mu * (2.0 / r_init - 1.0 / a)).sqrt();
        commands.spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: BodyType::Comet,
            },
            Mass(0.005 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.25),
            SimPosition(DVec3::new(r_init, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_ap)),
            SimAcceleration(DVec3::ZERO),
            Temperature(90.0),
            Composition::icy(),
        ));
    }

    central_star
}

fn spawn_grand_tour_mission(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
    config: &mut SimulationConfig,
) -> Entity {
    disk_params.disk_mass = 0.0;
    config.gas_density_scale = 0.0;

    let central_star = commands
        .spawn((
            CelestialBody {
                name: "Sol Major".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            CentralStar,
            Mass(1.0),
            Radius(SOLAR_RADIUS_AU),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Temperature(5778.0),
            Luminosity(1.0),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1.5e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 10.0,
            },
        ))
        .id();

    let mu = G_ASTRO * 1.0;
    let planets = [
        (
            "Planet Aurelia",
            0.80,
            0.9 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 0.95,
            BodyType::TerrestrialPlanet,
            Composition::rocky(),
        ),
        (
            "Planet Jovian Prime",
            2.40,
            300.0 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 11.0,
            BodyType::GasGiant,
            Composition::solar_gas(),
        ),
        (
            "Planet Cronus",
            5.50,
            95.0 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 9.0,
            BodyType::GasGiant,
            Composition::solar_gas(),
        ),
        (
            "Planet Oceanus",
            10.50,
            15.0 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 4.0,
            BodyType::IceGiant,
            Composition::icy(),
        ),
    ];

    for (name, r, m, rad, btype, comp) in planets {
        let v = (mu / r).sqrt();
        commands.spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: btype,
            },
            Mass(m),
            Radius(rad),
            SimPosition(DVec3::new(r, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v)),
            SimAcceleration(DVec3::ZERO),
            Temperature(250.0 / r.sqrt()),
            comp,
        ));
    }

    // Space probe: Voyager III
    let probe_r = 0.82;
    let v_transfer = (mu / probe_r).sqrt() * 1.15;
    commands.spawn((
        CelestialBody {
            name: "Voyager III (Grand Tourer)".to_string(),
            body_type: BodyType::Asteroid,
        },
        Mass(0.0),
        Radius(EARTH_RADIUS_AU * 0.01),
        SimPosition(DVec3::new(probe_r, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, v_transfer)),
        SimAcceleration(DVec3::ZERO),
        Temperature(280.0),
        Composition::metal_rich(),
        SpaceProbe::new(ProbeType::Orbiter, 101),
        FlightComputer {
            target_entity: central_star,
            target_name: "Sol Major".to_string(),
            launch_pos: DVec3::new(probe_r, 0.0, 0.0),
            distance_to_target: probe_r,
            relative_speed: v_transfer,
        },
    ));

    central_star
}

fn spawn_exodus_mission(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
    config: &mut SimulationConfig,
) -> Entity {
    disk_params.disk_mass = 0.0;
    config.gas_density_scale = 0.0;

    // 20 M☉ Red Supergiant nearing iron core collapse
    let central_star = commands
        .spawn((
            CelestialBody {
                name: "Betelgeuse Secundus".to_string(),
                body_type: BodyType::RedSupergiant,
            },
            CentralStar,
            Mass(20.0),
            Radius(3.5),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Temperature(3400.0),
            Luminosity(120_000.0),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 1.0e8,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 15.0,
            },
            StellarEvolutionState {
                phase: StellarEvolutionPhase::RedSupergiantBranch,
                hydrogen_core_fraction: 0.05,
                helium_core_fraction: 0.95,
                envelope_mass_loss_rate: 1.5e-4,
                phase_timer_years: 49_000.0,
                nebula_expansion_radius_au: 0.0,
                nebula_opacity: 0.0,
            },
        ))
        .id();

    let mu = G_ASTRO * 20.0;
    let r_haven = 8.5;
    let v_haven = (mu / r_haven).sqrt();
    commands.spawn((
        CelestialBody {
            name: "Haven IV (Colony World)".to_string(),
            body_type: BodyType::TerrestrialPlanet,
        },
        Mass(1.5 * EARTH_MASS_SOLAR),
        Radius(EARTH_RADIUS_AU * 1.15),
        SimPosition(DVec3::new(r_haven, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, v_haven)),
        SimAcceleration(DVec3::ZERO),
        Temperature(288.0),
        Composition::rocky(),
        BiosphereState {
            habitability_score: 0.95,
            biomass_coverage_frac: 0.65,
            oxygen_fraction: 0.21,
            emergence_year: Some(10_000.0),
            technosignature: 1.85,
            flora_pigment: FloraPigmentClass::SolarChlorophyll,
            bioluminescence_intensity: 0.8,
            surface_samples_collected: 0,
        },
    ));

    // Ark Colony Ships escaping outward
    let ships = [
        ("Ark Colony Ship Alpha", 15.0, 30.0),
        ("Ark Colony Ship Beta", 18.0, 32.0),
    ];
    for (idx, (name, r_ship, v_out)) in ships.into_iter().enumerate() {
        commands.spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: BodyType::Asteroid,
            },
            Mass(0.0),
            Radius(EARTH_RADIUS_AU * 0.02),
            SimPosition(DVec3::new(r_ship, 0.0, 0.0)),
            SimVelocity(DVec3::new(v_out, 0.0, 0.0)),
            SimAcceleration(DVec3::ZERO),
            Temperature(150.0),
            Composition::metal_rich(),
            SpaceProbe::new(ProbeType::Orbiter, 201 + idx),
            FlightComputer {
                target_entity: central_star,
                target_name: "Betelgeuse Secundus".to_string(),
                launch_pos: DVec3::new(r_ship, 0.0, 0.0),
                distance_to_target: r_ship,
                relative_speed: v_out,
            },
        ));
    }

    central_star
}
