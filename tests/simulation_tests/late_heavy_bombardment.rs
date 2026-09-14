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
