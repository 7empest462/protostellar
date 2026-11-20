//! Phase 5 regression tests for full component serialization, probes, and persistent entity IDs.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::game::phases::LateHeavyBombardmentState;
use protostellar::rendering::effects::megastructures::DysonSwarm;
use protostellar::rendering::effects::supernova::SupernovaVisualShell;
use protostellar::rendering::surface_terrain::{PlanetarySurfaceTerrainRoot, SurfaceTerrainState};
use protostellar::simulation::accretion::{RocheLobeOverflow, TheiaImpactState};
use protostellar::simulation::biology::civilization::HabitabilityTimer;
use protostellar::simulation::components::*;
use protostellar::simulation::events::{HazardComet, RogueInterloper};
use protostellar::simulation::kozai_lidov::{KozaiLidovState, KozaiRegime};
use protostellar::simulation::neutron_stars::AlfvenBlastWave;
use protostellar::simulation::probes::{FlightComputer, ProbeCounter, ProbeType, SpaceProbe};
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::molecular_cloud::GmcPromotedSystem;
use protostellar::simulation::serialization::*;
use protostellar::simulation::space_weather::{AuroralStormExcitation, StellarActivityClock};
use protostellar::simulation::terraforming::{BombardmentProjectile, BombardmentType};
use protostellar::simulation::tides::TidalState;

fn build_phase5_roundtrip_test_app() -> App {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<TheiaImpactState>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<ProbeCounter>()
        .init_resource::<ImpactShockwavePool>()
        .init_resource::<SurfaceTerrainState>()
        .add_message::<SaveSystemEvent>()
        .add_message::<LoadSystemEvent>()
        .add_systems(
            Update,
            (handle_save_system_events, handle_load_system_events).chain(),
        );
    app
}

fn spawn_phase5_test_entities(world: &mut World) -> (Entity, Entity) {
    let star = world
        .spawn((
            CelestialBody {
                body_type: BodyType::YellowDwarf,
                name: "Twin Planet".to_string(),
            },
            CentralStar,
            Mass(1.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Radius(0.00465),
            Temperature(5778.0),
            Luminosity(1.0),
            Composition::solar_gas(),
            SpinState::default(),
            GmcPromotedSystem,
            CircumstellarDisk::new(0.5, 10.0, 0.02),
            RelativisticJetState::default(),
            DysonSwarm {
                completion_fraction: 0.42,
                mirror_count: 42_000,
            },
            StellarActivityClock {
                sim_years_accum: 12.5,
                realtime_secs_accum: 3.25,
                eruption_count: 7,
            },
        ))
        .id();

    let twin_b = world
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Twin Planet".to_string(),
            },
            Mass(3.0e-6),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            Radius(4.26e-5),
            Temperature(288.0),
            Luminosity(0.0),
            Composition::rocky(),
            SpinState::default(),
            HabitabilityTimer(1234.5),
            AuroralStormExcitation {
                excitation: 2.25,
                substorm_clock_secs: 9.5,
                last_blast_radius_au: 1.5,
            },
            AtmosphericEscapeTail {
                loss_rate_m_earth_per_myr: 0.12,
                tail_length_au: 0.04,
                ion_color: Color::srgba(0.2, 0.7, 1.0, 0.8),
                is_active: true,
            },
            RogueInterloper {
                initial_velocity: DVec3::new(1.0, 0.2, -0.5),
            },
            RocheLobeOverflow {
                companion: star,
                mass_transfer_rate: 1.5e-6,
                l1_distance_au: 0.35,
            },
            KozaiLidovState {
                perturber_entity: Some(star),
                perturber_name: "Twin Planet".to_string(),
                regime: KozaiRegime::Libration,
                ..Default::default()
            },
        ))
        .id();

    spawn_phase5_moon_probe_and_wave(world, star, twin_b);
    (star, twin_b)
}

fn spawn_phase5_moon_probe_and_wave(world: &mut World, star: Entity, twin_b: Entity) {
    world.spawn((
        CelestialBody {
            body_type: BodyType::Moon,
            name: "Twin Moon".to_string(),
        },
        Mass(3.6e-8),
        SimPosition(DVec3::new(1.0026, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, 6.5)),
        Radius(1.16e-5),
        Temperature(220.0),
        Luminosity(0.0),
        Composition::rocky(),
        SpinState::default(),
        SatelliteOf {
            parent: twin_b,
            semi_major_axis_au: 0.0026,
            orbital_period_years: 0.0748,
            true_anomaly: 0.75,
        },
        TidalState {
            host_entity: Some(twin_b),
            ..Default::default()
        },
        LhbImpactor {
            target_entity: twin_b,
            spawn_time_yr: 10.0,
            expected_arrival_yr: 25.0,
            target_r: 1.0,
            target_q: 0.95,
        },
        BombardmentProjectile {
            target_entity: twin_b,
            bombardment_type: BombardmentType::IcyComet,
            launch_time_yr: 5.0,
            expected_arrival_yr: 6.0,
            trail_color: Color::srgba(0.3, 0.8, 1.0, 1.0),
            is_detonated: false,
            relative_offset: DVec3::new(0.01, 0.0, 0.0),
            initial_distance: 0.01,
            flight_duration_secs: 2.0,
            flight_elapsed_secs: 0.5,
        },
        HazardComet,
    ));

    world.spawn((
        AlfvenBlastWave {
            source_entity: star,
            origin_au: DVec3::ZERO,
            radius_au: 3.5,
            expansion_speed_au_yr: 63_000.0,
            energy_erg: 1e46,
            luminosity_solar: 1e6,
            synchrotron_brightness: 0.85,
            age_seconds: 1.2,
            max_radius_au: 60.0,
        },
        SimPosition(DVec3::ZERO),
    ));

    world.spawn((
        SpaceProbe::new(ProbeType::Orbiter, 4),
        FlightComputer {
            target_entity: twin_b,
            target_name: "Twin Planet".to_string(),
            launch_pos: DVec3::new(0.9, 0.0, 0.0),
            distance_to_target: 0.1,
            relative_speed: 1.2,
        },
        CelestialBody {
            body_type: BodyType::Asteroid,
            name: "🚀 Orbiter #4 (to Twin Planet)".to_string(),
        },
        Mass(0.0),
        SimPosition(DVec3::new(0.95, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, 6.2)),
        Radius(1.5e-11),
        Temperature(290.0),
        Composition::metal_rich(),
    ));
}

fn assert_phase5_loaded_world(w: &mut World) {
    assert!(w.resource::<ImpactShockwavePool>().shockwaves.is_empty());
    assert_eq!(w.resource::<ProbeCounter>().0, 4);
    let lhb = w.resource::<LateHeavyBombardmentState>();
    assert_eq!(lhb.comets_scattered, 19);
    assert!((lhb.migration_progress - 0.72).abs() < 1e-9);
    assert!((lhb.water_delivered_earth_masses - 1.45).abs() < 1e-9);
    assert!(lhb.manual_trigger_requested);

    assert_eq!(w.query::<&SupernovaVisualShell>().iter(w).count(), 0);
    assert_eq!(w.query::<&PlanetarySurfaceTerrainRoot>().iter(w).count(), 0);

    let mut new_star = None;
    let mut new_twin_b = None;
    for (ent, body, opt_star) in w
        .query::<(Entity, &CelestialBody, Option<&CentralStar>)>()
        .iter(w)
    {
        if body.name == "Twin Planet" {
            if opt_star.is_some() {
                new_star = Some(ent);
            } else {
                new_twin_b = Some(ent);
            }
        }
    }
    let new_star = new_star.expect("Loaded CentralStar Twin Planet");
    let new_twin_b = new_twin_b.expect("Loaded terrestrial Twin Planet");

    let moon_sat = w
        .query::<&SatelliteOf>()
        .single(w)
        .expect("SatelliteOf must roundtrip");
    assert_eq!(moon_sat.parent, new_twin_b);
    assert_ne!(moon_sat.parent, new_star);

    assert_eq!(w.query::<&GmcPromotedSystem>().iter(w).count(), 1);
    assert_eq!(w.query::<&CircumstellarDisk>().iter(w).count(), 1);
    assert_eq!(w.query::<&RelativisticJetState>().iter(w).count(), 1);
    assert_eq!(w.query::<&DysonSwarm>().iter(w).count(), 1);
    assert_eq!(w.query::<&RocheLobeOverflow>().iter(w).count(), 1);
    assert_eq!(w.query::<&LhbImpactor>().iter(w).count(), 1);
    assert_eq!(w.query::<&BombardmentProjectile>().iter(w).count(), 1);
    assert_eq!(w.query::<&AlfvenBlastWave>().iter(w).count(), 1);
    assert_eq!(w.query::<&AtmosphericEscapeTail>().iter(w).count(), 1);
    assert_eq!(w.query::<&HabitabilityTimer>().iter(w).count(), 1);
    assert_eq!(w.query::<&AuroralStormExcitation>().iter(w).count(), 1);
    assert_eq!(w.query::<&StellarActivityClock>().iter(w).count(), 1);
    assert_eq!(w.query::<&RogueInterloper>().iter(w).count(), 1);
    assert_eq!(w.query::<&HazardComet>().iter(w).count(), 1);

    let (probe, fc) = w
        .query::<(&SpaceProbe, &FlightComputer)>()
        .single(w)
        .expect("SpaceProbe + FlightComputer must roundtrip");
    assert_eq!(probe.id, 4);
    assert_eq!(fc.target_entity, new_twin_b);
}

#[test]
fn test_phase5_14_missing_components_probes_and_persistent_id_roundtrip() {
    let mut app = build_phase5_roundtrip_test_app();
    let (star, _) = spawn_phase5_test_entities(app.world_mut());
    app.world_mut().resource_mut::<ProbeCounter>().0 = 4;

    {
        let mut lhb = app.world_mut().resource_mut::<LateHeavyBombardmentState>();
        lhb.is_active = true;
        lhb.migration_progress = 0.72;
        lhb.resonance_ratio = 2.03;
        lhb.resonance_crossed = true;
        lhb.water_delivered_earth_masses = 1.45;
        lhb.comets_scattered = 19;
        lhb.time_active_years = 8500.0;
        lhb.manual_trigger_requested = true;
    }

    let save_path = "target/test_saves_bevy/phase5_full_roundtrip.json";
    app.world_mut()
        .resource_mut::<Messages<SaveSystemEvent>>()
        .write(SaveSystemEvent {
            filename: save_path.to_string(),
        });
    app.update();

    let saved = load_system_from_file(save_path).expect("saved file must load");
    assert_eq!(saved.bodies.len(), 3, "Probes must stay in saved.probes");
    assert_eq!(saved.probes.len(), 1);
    assert_eq!(saved.alfven_waves.len(), 1);
    let postcard_bytes = serialize_system_postcard(&saved).expect("postcard roundtrip encode");
    let postcard_decoded =
        deserialize_system_postcard(&postcard_bytes).expect("postcard roundtrip decode");
    assert_eq!(postcard_decoded.probes.len(), 1);
    assert_eq!(postcard_decoded.alfven_waves.len(), 1);

    app.world_mut().spawn((
        SupernovaVisualShell { star_entity: star },
        Transform::default(),
    ));
    app.world_mut().spawn(PlanetarySurfaceTerrainRoot);
    app.world_mut()
        .resource_mut::<ImpactShockwavePool>()
        .shockwaves
        .push(ImpactShockwave {
            position: Vec3::ZERO,
            radius: 1.0,
            max_radius: 5.0,
            timer: 1.0,
            max_timer: 2.0,
            color: Color::WHITE,
            primary_entity: None,
            local_position: Vec3::ZERO,
        });
    app.world_mut()
        .resource_mut::<LateHeavyBombardmentState>()
        .comets_scattered = 0;

    app.world_mut()
        .resource_mut::<Messages<LoadSystemEvent>>()
        .write(LoadSystemEvent {
            filename: save_path.to_string(),
        });
    app.update();

    assert_phase5_loaded_world(app.world_mut());
    let _ = std::fs::remove_file(save_path);
}

#[test]
fn test_solar_nebula_protostar_serializes_and_earth_stays_temperate() {
    use protostellar::simulation::scenarios::spawn_solar_nebula_mmsn;
    use protostellar::simulation::thermodynamics::{StarIgnitionEvent, update_thermodynamics};

    let mut app = build_phase5_roundtrip_test_app();
    app.add_message::<StarIgnitionEvent>()
        .add_message::<PlanetaryEngulfmentEvent>()
        .add_message::<SupernovaEvent>()
        .add_systems(Update, update_thermodynamics);

    let mut dp = DiskParameters::default();
    let star_ent = spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut dp);
    app.insert_resource(dp);
    app.update();

    // Advance past protostar ignition (T >= 30 yr) so the shockwave reaches 1.0 AU
    app.world_mut().resource_mut::<SimTime>().elapsed_years = 30.8;
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.05;
    for _ in 0..20 {
        app.world_mut().resource_mut::<SimTime>().elapsed_years += 0.05;
        app.update();
    }

    let earth_ent = {
        let mut q = app.world_mut().query::<(Entity, &CelestialBody)>();
        q.iter(app.world())
            .find(|(_, b)| b.name == "Earth")
            .map(|(e, _)| e)
            .expect("Earth entity in SolarNebulaMmsn")
    };
    let earth_temp = app.world().get::<Temperature>(earth_ent).unwrap().0;
    assert!(
        (250.0..=350.0).contains(&earth_temp),
        "Earth at 1.0 AU must remain temperate across protostar ignition shockwave, got {earth_temp} K"
    );

    // Even after a transient 1100 K impact spike, Earth must cool back down and not lock into a steam greenhouse
    app.world_mut().get_mut::<Temperature>(earth_ent).unwrap().0 = 1100.0;
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.5;
    for _ in 0..15 {
        app.update();
    }
    let cooled_temp = app.world().get::<Temperature>(earth_ent).unwrap().0;
    assert!(
        cooled_temp < 350.0,
        "Transiently heated Earth at 1.0 AU must cool back below 350 K without locking at ~630+ K, got {cooled_temp} K"
    );

    // Saving SolarNebulaMmsn must include the central star
    let save_path = "target/test_saves_bevy/mmsn_with_star.json";
    app.world_mut()
        .resource_mut::<Messages<SaveSystemEvent>>()
        .write(SaveSystemEvent {
            filename: save_path.to_string(),
        });
    app.update();
    let saved = load_system_from_file(save_path).expect("MMSN save must load");
    assert!(
        saved
            .bodies
            .iter()
            .any(|b| b.is_central_star && b.body_type.is_star_or_remnant()),
        "Saving SolarNebulaMmsn must preserve the CentralStar entity ({star_ent:?})"
    );
    let _ = std::fs::remove_file(save_path);
}

#[test]
fn test_legacy_starless_save_restores_star_sanitizes_1436k_earth_and_updates_on_orbit_move() {
    use protostellar::game::ui::interactions::editor::handle_orbital_adjustment_relative;
    use protostellar::game::ui::types::{SelectedWorldQuery, UiButtonAction};
    use protostellar::simulation::thermodynamics::{StarIgnitionEvent, update_thermodynamics};
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU};

    let legacy_earth = CelestialBodySave {
        name: "Earth".to_string(),
        body_type: BodyType::TerrestrialPlanet,
        position: DVec3::new(0.3848, -0.0198, 1.0516),
        velocity: DVec3::new(-5.5, 0.0, 2.0),
        mass: 1.53 * EARTH_MASS_SOLAR,
        radius: EARTH_RADIUS_AU * 1.13,
        temperature: 1436.376,
        luminosity: 0.0,
        composition: Composition::rocky(),
        spin: SpinState::default(),
        volatile_inventory: Some(VolatileInventory {
            delivered_water_m_earth: 0.0181,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: 450.94,
            cometary_impact_count: 0,
        }),
        climate: Some(PlanetaryClimate {
            surface_temperature_k: 1436.376,
            equilibrium_temperature_k: 222.66,
            greenhouse_delta_k: 406.84,
            albedo: 0.489,
            ice_coverage_frac: 0.0,
            cloud_coverage_frac: 0.95,
            climate_regime: ClimateRegime::RunawayVenusian,
            polar_ice_cap_latitude_deg: 90.0,
        }),
        tidal_state: Some(TidalState::default()),
        ..Default::default()
    };

    let save_data = SystemSaveData {
        version: 1,
        timestamp_epoch_yr: 314_983.0,
        step_count: 1_000_000,
        time_warp: TimeWarpSave::default(),
        disk_parameters: DiskParameters::default(),
        config_save: ConfigSave::default(),
        bodies: vec![legacy_earth],
        scenario_preset: Some(protostellar::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn),
        ..Default::default()
    };

    let load_path = "target/test_saves_bevy/legacy_1436k_earth.json";
    save_system_to_file(&save_data, load_path).expect("write legacy save");

    let mut app = build_phase5_roundtrip_test_app();
    app.add_message::<StarIgnitionEvent>()
        .add_message::<PlanetaryEngulfmentEvent>()
        .add_message::<SupernovaEvent>()
        .add_systems(Update, update_thermodynamics);

    app.world_mut()
        .resource_mut::<Messages<LoadSystemEvent>>()
        .write(LoadSystemEvent {
            filename: load_path.to_string(),
        });
    app.update();

    let (star_count, earth_ent) = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &CelestialBody, Option<&CentralStar>)>();
        let stars = q.iter(app.world()).filter(|(_, _, s)| s.is_some()).count();
        let earth = q
            .iter(app.world())
            .find(|(_, b, _)| b.name == "Earth")
            .map(|(e, ..)| e)
            .expect("Restored Earth");
        (stars, earth)
    };
    assert_eq!(
        star_count, 1,
        "Legacy starless save must reconstruct CentralStar"
    );

    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.05;
    app.update();

    let restored_temp = app.world().get::<Temperature>(earth_ent).unwrap().0;
    let restored_vol = app.world().get::<VolatileInventory>(earth_ent).unwrap();
    assert!(
        (250.0..=320.0).contains(&restored_temp),
        "Loaded 1436 K Earth must recover to temperate range, got {restored_temp} K"
    );
    assert!(
        restored_vol.atmospheric_pressure_bar < 3.0,
        "Trapped 450 bar steam must condense back below 3 bar, got {} bar",
        restored_vol.atmospheric_pressure_bar
    );

    // Pause simulation and expand Earth orbit multiple times -> temperature must drop dynamically!
    app.world_mut().resource_mut::<TimeWarp>().is_paused = true;
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(earth_ent);
    let mut toast = protostellar::game::ui::NotificationToast::default();
    for _ in 0..16 {
        let mut ss: bevy::ecs::system::SystemState<(
            SelectedWorldQuery,
            Res<PlayerInteractionState>,
        )> = bevy::ecs::system::SystemState::new(app.world_mut());
        let (mut sq, ps) = ss.get_mut(app.world_mut()).unwrap();
        handle_orbital_adjustment_relative(
            &UiButtonAction::ExpandOrbit,
            &mut sq,
            None,
            &ps,
            1.0,
            &mut toast,
        );
    }
    let far_temp = app.world().get::<Temperature>(earth_ent).unwrap().0;
    assert!(
        far_temp < 180.0,
        "Moving Earth outward while paused must cool it dynamically below 180 K, got {far_temp} K"
    );
    let _ = std::fs::remove_file(load_path);
}
