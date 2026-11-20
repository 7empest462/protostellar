//! Stage 1 Hardening Regression Tests: critical correctness, orbital stability,
//! save/load integrity, stellar/biosphere evolution, and UI/predictor invariants.

use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::ui_widgets::{SliderDragState, SliderRange, SliderValue};

use protostellar::game::phases::LateHeavyBombardmentState;
use protostellar::game::ui::widget_sync::sync_hud_sliders;
use protostellar::game::ui::widgets::{HudSlider, HudSliderKind};
use protostellar::rendering::bodies::VisualBody;
use protostellar::rendering::particle_swarm::ParticleSwarmMesh;
use protostellar::simulation::biology::civilization::habitability_tracking_system;
use protostellar::simulation::components::*;
use protostellar::simulation::geology::{GeologicalEpoch, GeologicalState, TimelineScrubber};
use protostellar::simulation::kozai_lidov::{
    KozaiLidovConfig, KozaiLidovState, update_kozai_lidov_evolution,
};
use protostellar::simulation::physics::integrator::{
    PhysicsBodyEntry, advance_symplectic_leapfrog_drift_with_companion,
};
use protostellar::simulation::physics::step_physics_simulation;
use protostellar::simulation::predictor::{TrajectoryPredictorState, update_trajectory_predictor};
use protostellar::simulation::probes::{ProbeType, SpaceProbe};
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{
    ActiveScenarioState, LoadScenarioEvent, ScenarioPreset, ScenarioReloadGeneration,
    handle_load_scenario_events,
};
use protostellar::simulation::serialization::{
    CURRENT_SAVE_SCHEMA_VERSION, CelestialBodySave, ImportSystemSeedJsonEvent, SpaceProbeSave,
    SystemSaveData, encode_system_seed_json, handle_clipboard_seed_import_events,
};
use protostellar::simulation::terraforming::TerraformingAtmosphere;
use protostellar::simulation::terraforming::climate::update_body_terraforming_climate;
use protostellar::simulation::thermodynamics::stellar_evolution::step_stellar_evolution_cycle;
use protostellar::simulation::thermodynamics::{StarIgnitionEvent, update_thermodynamics};
use protostellar::simulation::tides::{TidalConfig, TidalState, update_tidal_evolution};
use protostellar::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU, G_ASTRO, SOLAR_RADIUS_AU};

#[test]
fn test_stage1_galactic_collision_swarm_visible_and_halo_pe_mass_scaled() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<LoadScenarioEvent>()
        .init_resource::<DiskParameters>()
        .init_resource::<SimTime>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<TimeWarp>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<SimulationConfig>()
        .init_resource::<ScenarioReloadGeneration>()
        .add_systems(
            Update,
            (handle_load_scenario_events, step_physics_simulation).chain(),
        );

    let swarm_ent = app
        .world_mut()
        .spawn((ParticleSwarmMesh, Visibility::Hidden))
        .id();

    app.world_mut()
        .resource_mut::<Messages<LoadScenarioEvent>>()
        .write(LoadScenarioEvent(ScenarioPreset::GalacticCollision));
    app.update();

    let vis = *app.world().entity(swarm_ent).get::<Visibility>().unwrap();
    assert_eq!(
        vis,
        Visibility::Inherited,
        "GalacticCollision must keep ParticleSwarmMesh visible"
    );
    assert!(
        app.world().resource::<SimulationConfig>().active_particles > 0,
        "GalacticCollision must keep active_particles > 0"
    );

    // Verify halo potential energy includes the SMBH mass factor (~1e16+ scale, not ~1e8)
    let pe = app.world().resource::<EnergyMonitor>().potential_energy;
    assert!(
        pe < -1.0e14,
        "Galaxy collision halo potential energy must include SMBH mass factor, got {pe}"
    );
}

#[test]
fn test_stage1_high_eccentricity_tidal_circularization_conserves_h_without_ejection() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeWarp {
            multiplier: 100.0,
            is_paused: false,
            step_once: false,
        })
        .insert_resource(SimTime {
            elapsed_years: 10.0,
            current_dt_yr: 0.01,
            step_count: 1,
            visual_time_secs: 0.0,
        })
        .init_resource::<SimulationConfig>()
        .insert_resource(TidalConfig {
            is_enabled: true,
            time_scale: 1.0e6,
            enable_circularization: true,
            enable_spin_synchronization: false,
            enable_heating: false,
        })
        .add_systems(Update, update_tidal_evolution);

    let m_star = 1.0;
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "Host".to_string(),
        },
        Mass(m_star),
        Radius(SOLAR_RADIUS_AU),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
    ));

    // Start at apoapsis of an e = 0.85 orbit with a = 0.25 AU (v_circ_eq > v_esc at apoapsis!)
    let a = 0.25_f64;
    let e0 = 0.85_f64;
    let m_p = 1e-3;
    let mu = G_ASTRO * (m_star + m_p);
    let r_apo = a * (1.0 + e0);
    // Start between apoapsis and periapsis with an inward radial component so radial damping acts immediately
    let pos0 = DVec3::new(r_apo * 0.85, 0.0, 0.0);
    let v_tan0 = (mu * a * (1.0 - e0 * e0)).sqrt() / pos0.length();
    let v_tot_sq = mu * (2.0 / pos0.length() - 1.0 / a);
    let v_rad0 = (v_tot_sq - v_tan0 * v_tan0).max(0.0).sqrt();
    let vel0 = DVec3::new(-v_rad0, 0.0, v_tan0);
    let h0 = pos0.cross(vel0).length();

    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::GasGiant,
                name: "Eccentric Giant".to_string(),
            },
            Mass(m_p),
            Radius(0.00048),
            SimPosition(pos0),
            SimVelocity(vel0),
            Composition::pure_hydrogen(),
            SpinState::default(),
            TidalState::new(0.5, 500.0),
        ))
        .id();

    for _ in 0..20 {
        app.update();
    }

    let final_vel = app.world().entity(planet).get::<SimVelocity>().unwrap().0;
    let h1 = pos0.cross(final_vel).length();
    let rel_h_err = (h1 - h0).abs() / h0;
    assert!(
        rel_h_err < 1e-9,
        "Radial-only tidal circularization must strictly conserve angular momentum: h0={h0}, h1={h1}"
    );
    let eps1 = 0.5 * final_vel.length_squared() - mu / pos0.length();
    assert!(
        eps1 < 0.0,
        "High-eccentricity planet must remain strictly gravitationally bound (eps={eps1})"
    );
}

#[test]
fn test_stage1_kozai_tidal_boost_survives_same_frame_tidal_update() {
    fn run_case(enable_kozai_tidal: bool) -> (f64, f64) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<protostellar::simulation::kozai_lidov::KozaiDisruptionEvent>()
            .insert_resource(TimeWarp {
                multiplier: 10.0,
                is_paused: false,
                step_once: false,
            })
            .insert_resource(SimTime {
                elapsed_years: 10.0,
                current_dt_yr: 0.01,
                step_count: 1,
                visual_time_secs: 0.0,
            })
            .init_resource::<SimulationConfig>()
            .insert_resource(KozaiLidovConfig {
                enable_kozai_lidov: true,
                enable_gr_suppression: false,
                enable_tidal_circularization: enable_kozai_tidal,
                secular_time_scale: 1.0,
            })
            .insert_resource(TidalConfig {
                is_enabled: true,
                time_scale: 1.0,
                enable_circularization: true,
                enable_spin_synchronization: false,
                enable_heating: true,
            })
            .add_systems(
                Update,
                update_kozai_lidov_evolution.before(update_tidal_evolution),
            )
            .add_systems(Update, update_tidal_evolution);

        app.world_mut().spawn((
            CentralStar,
            CelestialBody {
                body_type: BodyType::YellowDwarf,
                name: "Primary".to_string(),
            },
            Mass(1.0),
            Radius(SOLAR_RADIUS_AU),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
        ));

        let outer = app
            .world_mut()
            .spawn((
                CelestialBody {
                    body_type: BodyType::RedDwarf,
                    name: "Outer Perturber".to_string(),
                },
                Mass(0.4),
                Radius(0.002),
                SimPosition(DVec3::new(100.0, 0.0, 0.0)),
                SimVelocity(DVec3::new(0.0, 0.0, (G_ASTRO * 1.4 / 100.0).sqrt())),
                Composition::pure_hydrogen(),
                SpinState::default(),
            ))
            .id();

        let a_in = 0.5_f64;
        let e_in = 0.65_f64;
        let r_p = a_in * (1.0 - e_in);
        let v_p = (G_ASTRO * 1.001 * (1.0 + e_in) / r_p).sqrt();
        let inc = 68.0_f64.to_radians();
        let inner = app
            .world_mut()
            .spawn((
                CelestialBody {
                    body_type: BodyType::GasGiant,
                    name: "HD 80606 b".to_string(),
                },
                Mass(0.001),
                Radius(0.00048),
                SimPosition(DVec3::new(r_p, 0.0, 0.0)),
                SimVelocity(DVec3::new(0.0, v_p * inc.sin(), v_p * inc.cos())),
                Composition::pure_hydrogen(),
                SpinState::default(),
                TidalState::new_gas_giant(),
                KozaiLidovState {
                    perturber_entity: Some(outer),
                    perturber_name: "Outer Perturber".to_string(),
                    ..default()
                },
            ))
            .id();

        app.update();
        let tide = *app.world().entity(inner).get::<TidalState>().unwrap();
        (
            tide.tidal_heating_power_watts,
            tide.circularization_rate_per_myr.abs(),
        )
    }

    let (base_power, base_circ) = run_case(false);
    let (boosted_power, boosted_circ) = run_case(true);
    assert!(
        boosted_power > base_power * 10.0,
        "Kozai high-e tidal boost must survive update_tidal_evolution: base={base_power}, boosted={boosted_power}"
    );
    assert!(
        boosted_circ > base_circ * 10.0,
        "Kozai high-e circularization boost must survive update_tidal_evolution: base={base_circ}, boosted={boosted_circ}"
    );
}

#[test]
fn test_stage1_satellite_detachment_single_drift_no_double_step() {
    let parent_ent = Entity::from_bits(1);
    let moon_ent = Entity::from_bits(2);
    let sub_dt = 0.01;
    let moon_pos0 = DVec3::new(5.002, 0.0, 0.0);
    let moon_vel0 = DVec3::new(0.0, 0.0, 2.5);

    let mut bodies = vec![
        PhysicsBodyEntry {
            entity: parent_ent,
            pos: DVec3::new(5.0, 0.0, 0.0),
            vel: DVec3::new(0.0, 0.0, 2.0),
            acc: DVec3::ZERO,
            mass: 0.001,
            radius: 0.01, // parent_radius * 1.05 = 0.0105 > moon semi_major_axis 0.002 -> detach!
            body_type: BodyType::GasGiant,
            is_central_star: false,
            role_flags: 0,
            satellite: None,
        },
        PhysicsBodyEntry {
            entity: moon_ent,
            pos: moon_pos0,
            vel: moon_vel0,
            acc: DVec3::ZERO,
            mass: 1e-8,
            radius: 1e-5,
            body_type: BodyType::Moon,
            is_central_star: false,
            role_flags: 0,
            satellite: Some(SatelliteOf {
                parent: parent_ent,
                semi_major_axis_au: 0.002,
                orbital_period_years: 0.01,
                true_anomaly: 0.0,
            }),
        },
    ];

    let mut snap = Vec::new();
    advance_symplectic_leapfrog_drift_with_companion(
        &mut bodies,
        sub_dt,
        DVec3::ZERO,
        0.0,
        &mut snap,
        false,
        true, // disable Kepler splitting so free drift is strictly pos + vel * sub_dt
        None,
    );

    assert!(bodies[1].satellite.is_none(), "Moon must detach");
    let expected = moon_pos0 + moon_vel0 * sub_dt;
    let err = (bodies[1].pos - expected).length();
    assert!(
        err < 1e-12,
        "Detached moon must drift exactly once (expected {expected:?}, got {:?})",
        bodies[1].pos
    );
}

#[test]
fn test_stage1_white_dwarf_ifmr_converges_to_progenitor_mass_across_small_steps() {
    type StarEvoQuery<'w, 's> = Query<
        'w,
        's,
        (
            Entity,
            &'static mut Mass,
            &'static mut Radius,
            &'static mut Temperature,
            &'static mut Luminosity,
            &'static mut CelestialBody,
            &'static mut IgnitionState,
            &'static mut StellarEvolutionState,
        ),
    >;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<SupernovaEvent>();

    let progenitor_mass = 5.0_f64;
    let expected_wd = 0.47 + 0.08 * progenitor_mass; // 0.87 M_sun

    let star = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::RedGiant,
                name: "Test AGB".to_string(),
            },
            Mass(progenitor_mass),
            Radius(1.5),
            Temperature(2900.0),
            Luminosity(3500.0),
            IgnitionState {
                core_temperature: 1e8,
                fusion_fraction: 0.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState {
                phase: StellarEvolutionPhase::HeliumFlashAgb,
                phase_timer_years: 2501.0,
                ..default()
            },
        ))
        .id();

    let mut state: bevy::ecs::system::SystemState<(StarEvoQuery, MessageWriter<SupernovaEvent>)> =
        bevy::ecs::system::SystemState::new(app.world_mut());

    // Step across AGB entry and 200 small planetary nebula substeps (dt = 0.1 yr)
    for _ in 0..200 {
        let (mut q, mut ev) = state.get_mut(app.world_mut()).unwrap();
        let (e, mut m, mut r, mut t, mut l, mut b, mut ign, mut evo) = q.get_mut(star).unwrap();
        step_stellar_evolution_cycle(
            e, &mut m, &mut r, &mut t, &mut l, &mut b, &mut ign, &mut evo, 0.1, &mut ev, false,
        );
        if evo.phase == StellarEvolutionPhase::WhiteDwarf {
            break;
        }
    }

    let final_evo = *app
        .world()
        .entity(star)
        .get::<StellarEvolutionState>()
        .unwrap();
    let final_mass = app.world().entity(star).get::<Mass>().unwrap().0;
    assert_eq!(final_evo.phase, StellarEvolutionPhase::WhiteDwarf);
    assert!(
        (final_mass - expected_wd).abs() < 0.02,
        "5 M_sun progenitor WD must follow IFMR (~{expected_wd:.2} M_sun), not 0.511 fixed point; got {final_mass:.4}"
    );
}

#[test]
fn test_stage1_hot_dry_terraforming_classifies_as_runaway() {
    let mut hot_terra = TerraformingAtmosphere {
        co2_pressure_bar: 35.0,
        nitrogen_pressure_bar: 1.0,
        atmospheric_water_m_earth: 0.0,
        surface_liquid_water_m_earth: 0.0,
        impact_dust_optical_depth: 0.0,
        total_bombarded_mass_earth: 0.0,
    };
    let mut hot_climate = PlanetaryClimate {
        equilibrium_temperature_k: 340.0,
        ..default()
    };
    let mut hot_vol = VolatileInventory::default();
    let mut hot_comp = Composition::rocky();
    update_body_terraforming_climate(
        &mut hot_vol,
        &mut hot_terra,
        &mut hot_climate,
        &mut hot_comp,
        EARTH_MASS_SOLAR,
        1.0,
    );
    assert_eq!(
        hot_climate.climate_regime,
        ClimateRegime::RunawayVenusian,
        "Hot dry terraformed world (T={:.1} K) must not be TemperateHabitable",
        hot_climate.surface_temperature_k
    );
}

fn build_stage1_biosphere_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<StarIgnitionEvent>()
        .add_message::<PlanetaryEngulfmentEvent>()
        .add_message::<SupernovaEvent>()
        .init_resource::<SimulationConfig>()
        .insert_resource(TimeWarp {
            multiplier: 100.0,
            is_paused: false,
            step_once: false,
        })
        .insert_resource(SimTime {
            elapsed_years: 2_000_000.0,
            current_dt_yr: 10.0,
            step_count: 10,
            visual_time_secs: 0.0,
        })
        .add_systems(
            Update,
            (
                update_thermodynamics,
                habitability_tracking_system.after(update_thermodynamics),
            ),
        );

    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "Sol".to_string(),
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Luminosity(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 0.0,
        },
    ));
    app
}

#[test]
fn test_stage1_terraformed_world_biosphere_grows() {
    let mut app = build_stage1_biosphere_test_app();
    let terra_planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Terraformed Mars".to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, G_ASTRO.sqrt())),
            Temperature(288.0),
            Composition::rocky(),
            SpinState::default(),
            InternalDifferentiation {
                is_differentiated: true,
                core_temp_k: 5500.0,
                core_radius_au: EARTH_RADIUS_AU * 0.55,
                mantle_radius_au: EARTH_RADIUS_AU,
                magnetic_field_gauss: 0.4,
                ..default()
            },
            VolatileInventory {
                delivered_water_m_earth: 0.0004,
                ocean_coverage_frac: 0.6,
                atmospheric_pressure_bar: 1.0,
                ..default()
            },
            PlanetaryClimate {
                surface_temperature_k: 288.0,
                equilibrium_temperature_k: 255.0,
                climate_regime: ClimateRegime::TemperateHabitable,
                ..default()
            },
            TerraformingAtmosphere::default(),
            BiosphereState::default(),
        ))
        .id();

    app.update();

    let terra_bio = *app
        .world()
        .entity(terra_planet)
        .get::<BiosphereState>()
        .unwrap();
    assert!(
        terra_bio.biomass_coverage_frac > 0.0,
        "Terraformed habitable planet must grow biomass during update_thermodynamics"
    );
}

#[test]
fn test_stage1_exomoon_resolves_gas_giant_host() {
    let mut app = build_stage1_biosphere_test_app();
    let giant_pos = DVec3::new(1.05, 0.0, 0.0);
    let giant_vel = DVec3::new(0.0, 0.0, (G_ASTRO / 1.05).sqrt());
    let giant_mass = 0.001;
    let giant_host = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::GasGiant,
                name: "Habitable Zone Giant".to_string(),
            },
            Mass(giant_mass),
            Radius(0.00048),
            SimPosition(giant_pos),
            SimVelocity(giant_vel),
            Composition::pure_hydrogen(),
        ))
        .id();

    let moon_r = 0.004;
    let moon_v = (G_ASTRO * giant_mass / moon_r).sqrt();
    let exomoon = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Moon,
                name: "Pandora".to_string(),
            },
            Mass(EARTH_MASS_SOLAR * 0.5),
            Radius(EARTH_RADIUS_AU * 0.8),
            SimPosition(giant_pos + DVec3::new(moon_r, 0.0, 0.0)),
            SimVelocity(giant_vel + DVec3::new(0.0, 0.0, moon_v)),
            Temperature(288.0),
            Composition::rocky(),
            SatelliteOf {
                parent: giant_host,
                semi_major_axis_au: moon_r,
                orbital_period_years: 0.05,
                true_anomaly: 0.0,
            },
            VolatileInventory {
                delivered_water_m_earth: 0.0003,
                ocean_coverage_frac: 0.5,
                atmospheric_pressure_bar: 1.0,
                ..default()
            },
            PlanetaryClimate {
                surface_temperature_k: 288.0,
                climate_regime: ClimateRegime::TemperateHabitable,
                ..default()
            },
            BiosphereState {
                biomass_coverage_frac: 0.5,
                ..default()
            },
        ))
        .id();

    app.update();

    let moon_bio = *app.world().entity(exomoon).get::<BiosphereState>().unwrap();
    assert!(
        moon_bio.technosignature > 0.0,
        "Habitable exomoon orbiting a GasGiant without PlanetaryClimate must advance civilization"
    );
}

#[test]
fn test_stage1_save_load_preserves_hadean_probes_star_ignition_and_resets_scrubber() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<protostellar::simulation::serialization::PasteSystemSeedClipboardEvent>()
        .add_message::<ImportSystemSeedJsonEvent>()
        .add_message::<StarIgnitionEvent>()
        .add_message::<PlanetaryEngulfmentEvent>()
        .add_message::<SupernovaEvent>()
        .init_resource::<DiskParameters>()
        .init_resource::<SimTime>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<TimeWarp>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<SimulationConfig>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<ScenarioReloadGeneration>()
        .insert_resource(TimelineScrubber {
            auto_advance: false,
            active_epoch: GeologicalEpoch::SnowballEarth,
            scrubbed_age_gyr: 3.9,
            ..default()
        })
        .add_systems(
            Update,
            (
                handle_clipboard_seed_import_events,
                update_thermodynamics.after(handle_clipboard_seed_import_events),
            ),
        );

    let save = SystemSaveData {
        version: CURRENT_SAVE_SCHEMA_VERSION,
        timestamp_epoch_yr: 100.0,
        scenario_preset: Some(ScenarioPreset::SolarNebulaMmsn),
        bodies: vec![
            CelestialBodySave {
                save_id: Some(1),
                name: "Legacy Star".to_string(),
                body_type: BodyType::YellowDwarf,
                mass: 1.0,
                radius: SOLAR_RADIUS_AU,
                position: DVec3::ZERO,
                velocity: DVec3::ZERO,
                temperature: 5778.0,
                luminosity: 1.0,
                composition: Composition::pure_hydrogen(),
                spin: SpinState::default(),
                is_central_star: true,
                ignition_state: None, // Missing IgnitionState must be reconstructed!
                ..default()
            },
            CelestialBodySave {
                save_id: Some(2),
                name: "Hadean Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
                mass: EARTH_MASS_SOLAR,
                radius: EARTH_RADIUS_AU,
                position: DVec3::new(1.0, 0.0, 0.0),
                velocity: DVec3::new(0.0, 0.0, G_ASTRO.sqrt()),
                temperature: 550.0,
                luminosity: 0.0,
                composition: Composition::rocky(),
                spin: SpinState::default(),
                volatile_inventory: Some(VolatileInventory {
                    delivered_water_m_earth: 0.0005,
                    ocean_coverage_frac: 0.0,
                    atmospheric_pressure_bar: 25.0,
                    ..default()
                }),
                geological_state: Some(GeologicalState::new_for_epoch(GeologicalEpoch::Hadean)),
                ..default()
            },
        ],
        probes: vec![SpaceProbeSave {
            name: "Voyager 1".to_string(),
            probe: SpaceProbe::new(ProbeType::Orbiter, 1),
            position: DVec3::new(1.1, 0.0, 0.0),
            velocity: DVec3::new(0.0, 0.0, 6.0),
            mass: 0.0,
            radius: 1e-11,
            temperature: 290.0,
            composition: Composition::metal_rich(),
            flight_computer: None,
        }],
        ..default()
    };

    let seed = encode_system_seed_json(&save).unwrap();
    app.world_mut()
        .resource_mut::<Messages<ImportSystemSeedJsonEvent>>()
        .write(ImportSystemSeedJsonEvent { payload: seed });
    app.update();

    assert!(
        app.world().resource::<TimelineScrubber>().auto_advance,
        "Loading/importing a save must reset manual TimelineScrubber back to auto_advance"
    );

    let w = app.world_mut();
    let star_has_ign = w
        .query::<(&CelestialBody, Option<&IgnitionState>)>()
        .iter(w)
        .find(|(b, _)| b.name == "Legacy Star")
        .is_some_and(|(_, ign)| ign.is_some_and(|i| i.is_ignited));
    assert!(
        star_has_ign,
        "Restored star lacking ignition_state must receive ignited IgnitionState"
    );

    let (hadean_temp, hadean_p) = w
        .query::<(&CelestialBody, &Temperature, &VolatileInventory)>()
        .iter(w)
        .find(|(b, _, _)| b.name == "Hadean Earth")
        .map(|(_, t, v)| (t.0, v.atmospheric_pressure_bar))
        .expect("Hadean Earth must spawn");
    assert!(
        hadean_temp > 500.0 && hadean_p > 20.0,
        "v2 save must preserve Hadean Earth thermal/pressure state (got T={hadean_temp}, P={hadean_p})"
    );

    let probe_visuals = w
        .query::<(
            &SpaceProbe,
            Option<&Transform>,
            Option<&Visibility>,
            Option<&VisualBody>,
        )>()
        .iter(w)
        .next()
        .expect("Saved SpaceProbe must spawn");
    assert!(
        probe_visuals.1.is_some() && probe_visuals.2.is_some() && probe_visuals.3.is_some(),
        "Restored SpaceProbe must include Transform, Visibility, and VisualBody"
    );
}

#[test]
fn test_stage1_moon_orbit_slider_stays_parent_relative_and_paused_predictor_invalidates() {
    // 1. Moon orbit slider reads parent-relative distance (0.012 AU), NOT heliocentric 5.2 AU
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<PlayerInteractionState>()
        .add_systems(Update, sync_hud_sliders);

    let jupiter = app
        .world_mut()
        .spawn((
            Mass(0.001),
            SimPosition(DVec3::new(5.2, 0.0, 0.0)),
            SpinState::default(),
        ))
        .id();
    let io = app
        .world_mut()
        .spawn((
            Mass(4.5e-8),
            SimPosition(DVec3::new(5.2 + 0.012, 0.0, 0.0)),
            SpinState::default(),
            SatelliteOf {
                parent: jupiter,
                semi_major_axis_au: 0.012,
                orbital_period_years: 0.005,
                true_anomaly: 0.0,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(io);

    let slider_ent = app
        .world_mut()
        .spawn((
            HudSlider(HudSliderKind::InspectorOrbitAu),
            SliderRange::new(0.05, 60.0),
            SliderDragState::default(),
            SliderValue(1.0),
        ))
        .id();

    app.update();
    app.update(); // Flush deferred commands inserting moon SliderRange & SliderValue

    let slider_val = app
        .world()
        .entity(slider_ent)
        .get::<SliderValue>()
        .unwrap()
        .0;
    let slider_range = *app.world().entity(slider_ent).get::<SliderRange>().unwrap();
    assert!(
        slider_range.start() < 0.01 && slider_range.end() <= 0.30,
        "Moon selection must switch InspectorOrbitAu slider to moon-scale range, got {slider_range:?}"
    );
    assert!(
        (slider_val - 0.012).abs() < 0.001,
        "Moon orbit slider must sync parent-relative 0.012 AU, not heliocentric 5.2 AU; got {slider_val}"
    );

    // 2. PredictorCache must invalidate while paused when a body's velocity is edited
    let mut pred_app = App::new();
    pred_app
        .add_plugins(MinimalPlugins)
        .insert_resource(TimeWarp {
            multiplier: 1.0,
            is_paused: true,
            step_once: false,
        })
        .insert_resource(SimTime {
            elapsed_years: 5.0,
            current_dt_yr: 0.0,
            step_count: 42,
            visual_time_secs: 0.0,
        })
        .init_resource::<SlingshotState>()
        .insert_resource(TrajectoryPredictorState {
            is_enabled: true,
            forecast_horizon_yr: 1.0,
            ..default()
        })
        .init_resource::<PlayerInteractionState>()
        .add_systems(Update, update_trajectory_predictor);

    pred_app.world_mut().spawn((
        CentralStar,
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Mass(1.0),
    ));
    let planet = pred_app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Edited World".to_string(),
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, G_ASTRO.sqrt())),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
        ))
        .id();
    pred_app
        .world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(planet);

    pred_app.update();
    let pts_before = pred_app
        .world()
        .resource::<TrajectoryPredictorState>()
        .trajectory_points
        .clone();
    assert!(
        pts_before.len() >= 10,
        "Expected predicted trajectory points, got {}",
        pts_before.len()
    );

    // Edit velocity while paused (step_count unchanged!)
    pred_app
        .world_mut()
        .entity_mut(planet)
        .get_mut::<SimVelocity>()
        .unwrap()
        .0 *= 1.15;
    pred_app.update();

    let pts_after = &pred_app
        .world()
        .resource::<TrajectoryPredictorState>()
        .trajectory_points;
    let max_delta = pts_before
        .iter()
        .zip(pts_after.iter())
        .map(|(a, b)| (*a - *b).length())
        .fold(0.0_f32, f32::max);
    assert!(
        max_delta > 0.05,
        "PredictorCache must invalidate immediately on paused velocity edits (max_delta={max_delta})"
    );
}
