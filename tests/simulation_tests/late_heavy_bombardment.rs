//! Late Heavy Bombardment (LHB) cascade and phase transition tests.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::game::phases::*;
use protostellar::simulation::components::*;
use protostellar::simulation::disk::cascade::update_late_heavy_bombardment_cascade;
use protostellar::simulation::resources::*;
use protostellar::simulation::thermodynamics::StarIgnitionEvent;
use protostellar::utils::constants::*;

#[test]
fn test_late_heavy_bombardment_cascade_with_1024_bodies() {
    let mut app = App::new();
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<LateHeavyBombardmentState>()
        .add_systems(Update, update_late_heavy_bombardment_cascade);

    // Spawn central star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CentralStar,
    ));

    // Spawn Earth
    app.world_mut().spawn((
        SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        SimVelocity(DVec3::new(0.0, 0.0, (G_ASTRO * 1.0 / 1.0).sqrt())),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0 * EARTH_MASS_SOLAR),
        Radius(EARTH_RADIUS_AU),
        CelestialBody {
            name: "Earth".to_string(),
            body_type: BodyType::TerrestrialPlanet,
        },
        VolatileInventory::default(),
    ));

    // Spawn 100 minor bodies in the asteroid belt (exceeds the old 64-body cap)
    for i in 0..100 {
        let r = 2.4 + (i as f64 * 0.02);
        let phi = i as f64 * 0.1;
        let v_circ = (G_ASTRO * 1.0 / r).sqrt();
        app.world_mut().spawn((
            SimPosition(DVec3::new(r * phi.cos(), 0.0, r * phi.sin())),
            SimVelocity(DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos())),
            SimAcceleration(DVec3::ZERO),
            Mass(0.00001 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 0.05),
            CelestialBody {
                name: format!("Asteroid-{}", i),
                body_type: BodyType::Asteroid,
            },
        ));
    }

    // Activate LHB
    {
        let mut lhb = app.world_mut().resource_mut::<LateHeavyBombardmentState>();
        lhb.is_active = true;
        lhb.migration_progress = 0.10;
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 0.5;
        sim_time.elapsed_years = 850.0;
    }

    // Step the cascade
    for _ in 0..15 {
        app.update();
    }

    let lhb = app.world().resource::<LateHeavyBombardmentState>();
    assert!(
        lhb.comets_scattered > 0,
        "LHB cascade must actively scatter comets even when > 64 bodies exist in ECS (found {})",
        lhb.comets_scattered
    );

    // Verify active LHB impactors were spawned
    let mut impactor_count = 0;
    let mut query = app.world_mut().query::<(&CelestialBody, &SimPosition)>();
    for (body, _) in query.iter(app.world()) {
        if body.name.starts_with("LHB-") {
            impactor_count += 1;
        }
    }
    assert!(
        impactor_count > 0,
        "LHB impactors must be present in ECS with > 64 bodies"
    );
}

#[test]
fn test_late_heavy_bombardment_automatic_phase_transition_at_800_yr() {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<SystemPhase>()
        .init_resource::<Time>()
        .init_resource::<PhaseManager>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<SimTime>()
        .add_message::<StarIgnitionEvent>()
        .add_systems(Update, monitor_phase_transitions);

    // Spawn central star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        IgnitionState {
            is_ignited: true,
            fusion_fraction: 1.0,
            shockwave_radius: 1.0,
            core_temperature: 1.5e7,
        },
        CentralStar,
    ));

    // Start in PlanetaryAccretion
    {
        let mut phase_mgr = app.world_mut().resource_mut::<PhaseManager>();
        phase_mgr.current_phase = SystemPhase::PlanetaryAccretion;
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.elapsed_years = 820.0; // T >= 800 yr
    }

    app.update();

    let phase_mgr = app.world().resource::<PhaseManager>();
    let lhb = app.world().resource::<LateHeavyBombardmentState>();

    assert_eq!(
        phase_mgr.current_phase,
        SystemPhase::LateHeavyBombardment,
        "Phase must transition to LateHeavyBombardment at T >= 800 yr"
    );
    assert!(
        lhb.is_active,
        "lhb_state.is_active must be true at T >= 800 yr"
    );
}

#[test]
fn test_impact_basin_relaxation_gas_and_ice_giant_rapid_atmospheric_healing() {
    use protostellar::simulation::accretion::update_impact_basin_relaxation;

    let mut app = App::new();
    app.insert_resource(TimeWarp::default());
    let mut sim_time = SimTime::default();
    sim_time.current_dt_yr = 3.0; // 3 years of simulation time
    app.insert_resource(sim_time);

    let basin = ImpactBasin {
        surface_normal: Vec3::X,
        angular_radius: 0.10,
        formation_time_yr: 0.0,
        melt_glow_fraction: 1.0,
        elongation: 1.0,
        scar_intensity: 1.0,
    };

    // Gas Giant
    let gas_giant = app
        .world_mut()
        .spawn((
            PlanetaryBasins {
                basins: vec![basin],
            },
            CelestialBody {
                name: "Jupiter".to_string(),
                body_type: BodyType::GasGiant,
            },
            Composition::solar_gas(),
        ))
        .id();

    // Ice Giant
    let ice_giant = app
        .world_mut()
        .spawn((
            PlanetaryBasins {
                basins: vec![basin],
            },
            CelestialBody {
                name: "Neptune".to_string(),
                body_type: BodyType::IceGiant,
            },
            Composition {
                silicate_frac: 0.20,
                ice_frac: 0.65,
                gas_frac: 0.15,
                metal_frac: 0.0,
                organics_frac: 0.0,
            },
        ))
        .id();

    // Terrestrial Rocky World (airless)
    let rocky_world = app
        .world_mut()
        .spawn((
            PlanetaryBasins {
                basins: vec![basin],
            },
            CelestialBody {
                name: "Mercury".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Composition::rocky(),
        ))
        .id();

    app.add_systems(Update, update_impact_basin_relaxation);
    app.update();

    let gas_basins = app
        .world()
        .entity(gas_giant)
        .get::<PlanetaryBasins>()
        .unwrap();
    let ice_basins = app
        .world()
        .entity(ice_giant)
        .get::<PlanetaryBasins>()
        .unwrap();
    let rock_basins = app
        .world()
        .entity(rocky_world)
        .get::<PlanetaryBasins>()
        .unwrap();

    // In 3 years, atmospheric jet streams completely disperse the gas giant and ice giant scars (tau ~ 1.5 yr)
    assert!(
        gas_basins.basins.is_empty() || gas_basins.basins[0].scar_intensity < 0.05,
        "Gas giant atmospheric impact plume must rapidly disperse via zonal shear in <= 3 years"
    );
    assert!(
        ice_basins.basins.is_empty() || ice_basins.basins[0].scar_intensity < 0.05,
        "Ice giant methane cirrus/vortex plume must rapidly disperse via zonal shear in <= 3 years"
    );

    // In contrast, the rocky crater on Mercury has barely begun to relax (tau ~ 600 yr)
    assert!(
        !rock_basins.basins.is_empty() && rock_basins.basins[0].scar_intensity > 0.95,
        "Solid rocky crater on airless body must remain prominent after 3 years (scar: {:.3})",
        rock_basins.basins[0].scar_intensity
    );
}

#[test]
fn test_impact_basin_relaxation_icy_world_timescale() {
    use protostellar::simulation::accretion::update_impact_basin_relaxation;

    let mut app = App::new();
    app.insert_resource(TimeWarp::default());
    let mut sim_time = SimTime::default();
    sim_time.current_dt_yr = 150.0;
    app.insert_resource(sim_time);

    let basin = ImpactBasin {
        surface_normal: Vec3::X,
        angular_radius: 0.10,
        formation_time_yr: 0.0,
        melt_glow_fraction: 1.0,
        elongation: 1.0,
        scar_intensity: 1.0,
    };

    // Icy Moon / Cryo-world (Europa/Enceladus)
    let icy_world = app
        .world_mut()
        .spawn((
            PlanetaryBasins {
                basins: vec![basin],
            },
            CelestialBody {
                name: "Europa".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Composition::icy(),
        ))
        .id();

    // Airless Rocky Body (Mercury/Moon)
    let rocky_world = app
        .world_mut()
        .spawn((
            PlanetaryBasins {
                basins: vec![basin],
            },
            CelestialBody {
                name: "Mercury".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Composition::rocky(),
        ))
        .id();

    app.add_systems(Update, update_impact_basin_relaxation);
    app.update();

    let icy_pb = app
        .world()
        .entity(icy_world)
        .get::<PlanetaryBasins>()
        .unwrap();
    let rocky_pb = app
        .world()
        .entity(rocky_world)
        .get::<PlanetaryBasins>()
        .unwrap();

    let icy_scar = icy_pb.basins[0].scar_intensity;
    let rocky_scar = rocky_pb.basins[0].scar_intensity;

    // Icy crust relaxes faster than airless rock due to lower yield strength (300 yr vs 600 yr)
    assert!(
        icy_scar < rocky_scar,
        "Icy lithosphere must relax faster ({:.3}) than airless silicate rock ({:.3})",
        icy_scar,
        rocky_scar
    );
    assert!(
        (icy_scar - 0.50).abs() < 0.05,
        "Icy scar after 150 yr with tau=300 yr should be ~0.50 (got {:.3})",
        icy_scar
    );
}

#[test]
fn test_earth_lhb_cometary_bombardment_delivers_water_and_creates_oceans() {
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::{
        AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
    };
    use protostellar::simulation::physics::step_physics_simulation;

    let mut app = App::new();
    app.init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<SimulationConfig>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<LateHeavyBombardmentState>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<MoonFormationEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_systems(
            Update,
            (
                step_physics_simulation,
                update_late_heavy_bombardment_cascade.after(step_physics_simulation),
                process_accretion_and_collisions.after(update_late_heavy_bombardment_cascade),
            ),
        );

    // Spawn central star
    app.world_mut().spawn((
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CentralStar,
    ));

    // Spawn Earth with 0 water and 0 ocean
    let earth_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, (G_ASTRO * 1.0 / 1.0).sqrt())),
            SimAcceleration(DVec3::ZERO),
            Mass(1.0 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Temperature(288.0),
            Composition::rocky(),
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            VolatileInventory::default(),
            SpinState::default(),
        ))
        .id();

    // Activate Late Heavy Bombardment
    {
        let mut lhb = app.world_mut().resource_mut::<LateHeavyBombardmentState>();
        lhb.is_active = true;
        lhb.migration_progress = 0.10;
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 0.05;
        sim_time.elapsed_years = 850.0;
    }

    // Step simulation over ~30 sim years (600 steps)
    for _ in 0..600 {
        app.world_mut().resource_mut::<SimTime>().elapsed_years += 0.05;
        app.update();
    }

    // Verify Earth received cometary impacts and developed oceans
    let earth = app.world().entity(earth_ent);
    let vol = earth
        .get::<VolatileInventory>()
        .expect("Earth must have VolatileInventory");
    assert!(
        vol.cometary_impact_count > 0,
        "LHB comets must strike Earth! Got cometary_impact_count={}",
        vol.cometary_impact_count
    );
    assert!(
        vol.delivered_water_m_earth > 0.0,
        "LHB comets must deliver water to Earth! Got delivered_water={}",
        vol.delivered_water_m_earth
    );
    assert!(
        vol.ocean_coverage_frac > 0.0,
        "Delivered water must generate surface oceans! Got ocean_coverage_frac={}",
        vol.ocean_coverage_frac
    );
    assert!(
        vol.ocean_coverage_frac >= 0.15,
        "Multiple cometary impacts should significantly hydrate Earth (got {})",
        vol.ocean_coverage_frac
    );

    // Verify impact basins with magma melt glow were recorded
    if let Some(basins) = earth.get::<PlanetaryBasins>() {
        assert!(
            !basins.basins.is_empty(),
            "Impact basins must be recorded upon cometary collision"
        );
    }
}
