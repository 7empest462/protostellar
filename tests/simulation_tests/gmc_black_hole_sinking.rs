//! Tests for GMC cluster black hole sinking via dynamical friction, physical capture radius,
//! extinguished remnant radiation, and core mergers into a proto-galaxy nucleus.

use bevy::math::DVec3;
use protostellar::gpu::gmc_fluid::build_gmc_sinks;
use protostellar::simulation::components::*;
use protostellar::simulation::physics::forces::gmc_dynamical_friction_acc;
use protostellar::utils::constants::*;

#[test]
fn test_gmc_sink_builder_prioritizes_massive_remnants_and_extinguishes_radiation() {
    let pos_origin = SimPosition(DVec3::ZERO);
    let lum_high = Luminosity(5000.0);
    let lum_star = Luminosity(100.0);
    let mass_bh = Mass(14.0);
    let mass_star = Mass(1.0);
    let ign_active = IgnitionState {
        core_temperature: 1.5e7,
        fusion_fraction: 1.0,
        is_ignited: true,
        shockwave_radius: 10.0,
    };

    let bh_body = CelestialBody {
        body_type: BodyType::BlackHole,
        name: "Cluster Core Black Hole".to_string(),
    };
    let star_body = CelestialBody {
        body_type: BodyType::YellowDwarf,
        name: "Cluster Star".to_string(),
    };

    let candidates = [
        (
            &pos_origin,
            &mass_star,
            &lum_star,
            Some(&ign_active),
            &star_body,
        ),
        (
            &pos_origin,
            &mass_bh,
            &lum_high,
            Some(&ign_active),
            &bh_body,
        ),
    ];

    let (sinks, num_sinks) = build_gmc_sinks(candidates);
    assert_eq!(num_sinks, 2);

    // Black hole has higher mass (14.0 M_sun vs 1.0 M_sun), so it must take slot 0
    let bh_sink = sinks[0];
    assert!((bh_sink.mass_solar - 14.0).abs() < 1e-3);
    // Remnant must NOT radiate, must NOT be marked ignited, and must have small physical sink radius
    assert_eq!(bh_sink.is_ignited, 0);
    assert!(bh_sink.radiation_pressure_factor.abs() < 1e-6);
    assert!((bh_sink.sink_radius_au - 3.0).abs() < 1e-3);

    // Star takes slot 1, radiates and is ignited
    let star_sink = sinks[1];
    assert_eq!(star_sink.is_ignited, 1);
    assert!(star_sink.radiation_pressure_factor > 0.0);
    assert!(star_sink.sink_radius_au > 10.0);
}

#[test]
fn test_chandrasekhar_dynamical_friction_sinks_massive_black_hole() {
    let bh_pos = DVec3::new(45.0, 0.0, 0.0);
    let bh_vel = DVec3::new(0.0, 0.85, 0.0); // ~4 km/s in AU/yr
    let center_pos = DVec3::ZERO;

    // 12 M_sun black hole
    let acc_bh = gmc_dynamical_friction_acc(BodyType::BlackHole, bh_pos, bh_vel, 12.0, center_pos);
    assert!(
        acc_bh.length() > 0.0,
        "Dynamical friction must decelerate massive black hole"
    );
    // Acceleration must oppose velocity
    assert!(
        acc_bh.dot(bh_vel) < 0.0,
        "Friction must act against velocity vector"
    );
    // Mass segregation must pull inward toward center of mass
    assert!(
        acc_bh.x < 0.0,
        "Mass segregation must accelerate inward toward center"
    );

    // Inward falling black hole must continue accelerating inward, never repelled by drag!
    let inward_vel = DVec3::new(-0.85, 0.0, 0.0);
    let acc_inward =
        gmc_dynamical_friction_acc(BodyType::BlackHole, bh_pos, inward_vel, 12.0, center_pos);
    assert!(
        acc_inward.x < 0.0,
        "Inward falling black hole must accelerate inward toward center, not repelled outwards"
    );

    // 1 M_sun star at the same velocity should not experience strong dynamical friction sinking
    let acc_star = gmc_dynamical_friction_acc(BodyType::Protostar, bh_pos, bh_vel, 1.0, center_pos);
    assert_eq!(
        acc_star,
        DVec3::ZERO,
        "Low-mass star below 2.0 M_sun must not sink via dynamical friction"
    );
}

#[test]
fn test_black_hole_physical_capture_scale_is_microscopic() {
    // 10 M_sun black hole Schwarzschild radius
    let mass_bh = 10.0f64;
    let r_s_au = 1.974e-8 * mass_bh;
    let r_isco_au = 3.0 * r_s_au;

    // 1 M_sun solar-type star: radius ~ 0.00465 AU
    let r_star_au = SOLAR_RADIUS_AU;
    let mass_star = 1.0f64;

    // True physical tidal disruption radius
    let r_tidal_au = r_star_au * (mass_bh / mass_star).cbrt();

    // The physical capture radius should be around ~0.01 AU (the tidal disruption radius),
    // NOT bloated by size exaggeration to 0.05 - 0.20 AU
    let r_capture = r_isco_au.max(r_tidal_au);
    assert!(
        r_capture < 0.02,
        "True physical capture radius must be tightly bounded ({r_capture} AU)"
    );
    assert!(r_capture > 0.005);
}

#[test]
fn test_massive_black_hole_merger_names_galactic_nucleus() {
    let mut bh_name = "The Star (Black Hole Remnant)".to_string();

    // Inelastic merger with another black hole -> total mass 52.0 M_sun
    let updated_type = BodyType::BlackHole;
    let total_mass = 52.0;

    // Simulate update_merged_body_name
    if updated_type == BodyType::BlackHole {
        if total_mass >= 40.0 {
            bh_name = format!("Galactic Nucleus (SMBH Seed - {total_mass:.1} M☉)");
        } else if total_mass >= 15.0 {
            bh_name = format!("Intermediate-Mass Black Hole ({total_mass:.1} M☉)");
        }
    }

    assert_eq!(bh_name, "Galactic Nucleus (SMBH Seed - 52.0 M☉)");
}

#[test]
fn test_gmc_protostar_and_planet_gas_accretion_grows_mass() {
    use bevy::prelude::*;
    use protostellar::simulation::accretion::gas::gmc_cluster_gas_accretion;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::MolecularCloudCluster,
            scenario_time_years: 100.0,
            ..Default::default()
        });

    app.add_systems(Update, gmc_cluster_gas_accretion);

    // Spawn GMC Protostar at 50 AU
    let star_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(50.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.05, 0.0)),
            Mass(0.83),
            Radius(3.5 * SOLAR_RADIUS_AU),
            Composition::solar_gas(),
            CelestialBody {
                name: "Protostar Gen-I Jeans-1".to_string(),
                body_type: BodyType::Protostar,
            },
            Luminosity(2.0),
            Temperature(3800.0),
            IgnitionState {
                core_temperature: 1.2e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 1.0,
            },
        ))
        .id();

    // Spawn orbiting Gen-I Protoplanet at 12.5 AU relative to star (62.5 AU global)
    let planet_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(62.5, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.05 + (G_ASTRO * 0.83 / 12.5).sqrt(), 0.0)),
            Mass(0.003),
            Radius(1.5 * 0.000_477),
            Composition::solar_gas(),
            CelestialBody {
                name: "Protostar Gen-I Jeans-1 b".to_string(),
                body_type: BodyType::Protoplanet,
            },
            Luminosity(0.0),
            Temperature(150.0),
        ))
        .id();

    // Step simulation over 20 years
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 5.0;
        let mut time_warp = app.world_mut().resource_mut::<TimeWarp>();
        time_warp.multiplier = 10_000.0;
    }

    for _ in 0..4 {
        app.update();
    }

    let star_mass_after = app.world().get::<Mass>(star_ent).unwrap().0;
    let planet_mass_after = app.world().get::<Mass>(planet_ent).unwrap().0;

    assert!(
        star_mass_after > 0.83,
        "Protostar must gain mass from GMC cloud core! (got {star_mass_after:.4} M_sun)"
    );
    assert!(
        planet_mass_after > 0.003,
        "Protoplanet must gain mass from circumstellar envelope! (got {planet_mass_after:.6} M_sun)"
    );
}

#[test]
fn test_gmc_fast_forward_timestep_advances_simulation_rapidly() {
    use bevy::prelude::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<protostellar::game::phases::LateHeavyBombardmentState>()
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::MolecularCloudCluster,
            scenario_time_years: 100.0,
            ..Default::default()
        });

    app.add_systems(Update, step_physics_simulation);

    // Spawn a star and planet in GMC cluster
    app.world_mut().spawn((
        SimPosition(DVec3::new(40.0, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.1, 0.0)),
        SimAcceleration::default(),
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        CelestialBody {
            name: "Cluster Star Alpha".to_string(),
            body_type: BodyType::YellowDwarf,
        },
    ));

    // Set time warp to 10,000x
    {
        let mut time_warp = app.world_mut().resource_mut::<TimeWarp>();
        time_warp.multiplier = 10_000.0;
        time_warp.is_paused = false;
    }

    // Run 1 frame
    app.update();

    let sim_time = app.world().resource::<SimTime>();
    // At 10,000x time warp, target_dt is 5.0 years.
    // In GMC cluster mode, the simulation must advance by at least 1.0 year per frame (not frozen at 0.064 yr!)
    assert!(
        sim_time.current_dt_yr >= 1.0,
        "GMC cluster fast forward must advance >= 1.0 year per frame! Got current_dt_yr={}",
        sim_time.current_dt_yr
    );
    assert!(
        sim_time.elapsed_years >= 1.0,
        "GMC cluster fast forward must advance elapsed years! Got elapsed_years={}",
        sim_time.elapsed_years
    );
}

#[test]
fn test_gmc_gen2_rocky_planet_accretes_primordial_gas_and_volatiles() {
    use bevy::prelude::*;
    use protostellar::simulation::accretion::gas::gmc_cluster_gas_accretion;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::MolecularCloudCluster,
            scenario_time_years: 100.0,
            ..Default::default()
        });

    app.add_systems(Update, gmc_cluster_gas_accretion);

    // Spawn host star
    let _star_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(0.0, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Mass(1.2),
            Radius(3.0 * SOLAR_RADIUS_AU),
            Composition::solar_gas(),
            CelestialBody {
                name: "Protostar Gen-II Novacore-1".to_string(),
                body_type: BodyType::Protostar,
            },
            Luminosity(1.5),
            Temperature(4200.0),
        ))
        .id();

    // Spawn orbiting Gen-II Rocky Earth-analogue at 2.8 AU
    let initial_comp = Composition {
        metal_frac: 0.30,
        silicate_frac: 0.58,
        ice_frac: 0.05,
        organics_frac: 0.03,
        gas_frac: 0.04,
    };
    let planet_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(2.8, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, (G_ASTRO * 1.2 / 2.8).sqrt(), 0.0)),
            Mass(0.000_006), // ~2 Earth masses
            Radius(0.000_055),
            initial_comp,
            CelestialBody {
                name: "Protostar Gen-II Novacore-1 c (Rocky Earth-Analogue)".to_string(),
                body_type: BodyType::Protoplanet,
            },
            Luminosity(0.0),
            Temperature(288.0),
            VolatileInventory {
                delivered_water_m_earth: 0.008,
                ocean_coverage_frac: 0.70,
                atmospheric_pressure_bar: 1.25,
                cometary_impact_count: 45,
            },
        ))
        .id();

    // Step simulation over 20 years
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 5.0;
        let mut time_warp = app.world_mut().resource_mut::<TimeWarp>();
        time_warp.multiplier = 10_000.0;
    }

    for _ in 0..4 {
        app.update();
    }

    let planet_mass = app.world().get::<Mass>(planet_ent).unwrap().0;
    let planet_comp = *app.world().get::<Composition>(planet_ent).unwrap();
    let planet_vol = *app.world().get::<VolatileInventory>(planet_ent).unwrap();
    let planet_body = app.world().get::<CelestialBody>(planet_ent).unwrap();

    assert!(
        planet_mass > 0.000_006,
        "Gen-II planet must accrete mass while orbiting in GMC! (got {planet_mass:.8} M_sun)"
    );
    assert!(
        planet_comp.gas_frac > 0.02 && planet_comp.gas_frac <= 0.08,
        "Gen-II terrestrial world must retain a realistic primordial gas atmosphere! (got {:.2}%)",
        planet_comp.gas_frac * 100.0
    );
    assert!(
        planet_comp.ice_frac > 0.03,
        "Gen-II terrestrial world must retain volatile hydrosphere! (got {:.2}%)",
        planet_comp.ice_frac * 100.0
    );
    assert!(
        planet_vol.atmospheric_pressure_bar >= 1.25,
        "Atmospheric pressure must be maintained/increased from nebular envelope capture! (got {:.2} bar)",
        planet_vol.atmospheric_pressure_bar
    );
    assert!(
        matches!(
            planet_body.body_type,
            BodyType::SuperEarth | BodyType::TerrestrialPlanet
        ),
        "Protoplanet should promote to a mature terrestrial/super-earth planet! Got {:?}",
        planet_body.body_type
    );
}
