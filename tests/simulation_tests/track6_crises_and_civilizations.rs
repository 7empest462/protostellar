//! Integration tests for Track 6A: Interactive Crisis Events, 3D Defense Visuals, Banner UI, and Save/Load.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::game::ui::crisis_banner::{
    CrisisBannerContainer, CrisisBannerTitleText, spawn_crisis_alert_banner,
    update_crisis_alert_banner_ui,
};
use protostellar::game::ui::types::{HudVisibilityState, NotificationToast};
use protostellar::rendering::effects::civilizations::{
    OrbitalHabitatRingVisual, OrbitalMirrorVisual, PlanetaryShieldVisual,
    generate_orbital_habitat_ring_mesh, generate_orbital_mirror_array_mesh,
    sync_civilization_visuals,
};
use protostellar::rendering::effects::megastructures::{
    DysonSwarm, ShkadovEngineVisual, generate_dyson_collector_ring_mesh,
    generate_shkadov_hemisphere_mesh, sync_shkadov_stellar_engine_visuals,
};
use protostellar::rendering::effects::{MegastructureMaterial, PlanetaryShieldMaterial};
use protostellar::simulation::accretion::TheiaImpactState;
use protostellar::simulation::biology::CivilizationState;
use protostellar::simulation::components::*;
use protostellar::simulation::economy::{
    GameMode, ProgressionState, ScienceData, TechnologyId, UnlockedTechs,
};
use protostellar::simulation::events::{
    ActiveCosmicEvent, CosmicEventManager, CosmicEventPhase, CosmicEventResolvedEvent,
    CosmicEventStartedEvent, CosmicEventType, CrisisCountermeasureEvent, CrisisCountermeasureKind,
    CrisisImpactor, CrisisInterceptorPool, TriggerCosmicEvent, cosmic_event_lifecycle_system,
    cosmic_event_scheduler_system, handle_crisis_countermeasure_system,
};
use protostellar::simulation::probes::ProbeCounter;
use protostellar::simulation::resources::*;
use protostellar::simulation::serialization::{
    LoadSystemEvent, SaveSystemEvent, handle_load_system_events, handle_save_system_events,
};
use protostellar::simulation::terraforming::TerraformingAtmosphere;

use super::track6_progression_and_civs::spawn_inhabited_world;

fn init_core_crisis_resources(app: &mut App) {
    app.init_resource::<Time>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<SimulationConfig>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<CosmicEventManager>()
        .init_resource::<CrisisInterceptorPool>()
        .init_resource::<ProgressionState>()
        .init_resource::<ScienceData>()
        .init_resource::<UnlockedTechs>()
        .init_resource::<NotificationToast>()
        .add_message::<TriggerCosmicEvent>()
        .add_message::<CrisisCountermeasureEvent>()
        .add_message::<CosmicEventStartedEvent>()
        .add_message::<CosmicEventResolvedEvent>();
}

#[test]
fn test_planet_killer_swarm_kinetic_deflector_and_automated_defense_grid() {
    // Part A: Manual Kinetic Deflector resolves PlanetKillerSwarm as Averted
    let mut app = App::new();
    init_core_crisis_resources(&mut app);
    app.add_systems(
        Update,
        (
            cosmic_event_scheduler_system,
            handle_crisis_countermeasure_system.after(cosmic_event_scheduler_system),
            cosmic_event_lifecycle_system.after(handle_crisis_countermeasure_system),
        ),
    );
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.25;
    let world_ent = spawn_inhabited_world(app.world_mut(), "Earth", DVec3::X, 0.90);

    app.world_mut()
        .resource_mut::<Messages<TriggerCosmicEvent>>()
        .write(TriggerCosmicEvent(CosmicEventType::PlanetKillerSwarm));
    app.update();
    app.update();

    let impactors: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<CrisisImpactor>>()
        .iter(app.world())
        .collect();
    assert!(
        !impactors.is_empty(),
        "PlanetKillerSwarm must spawn CrisisImpactors"
    );

    // Fire Kinetic Deflector for each impactor
    for imp_ent in impactors {
        app.world_mut()
            .resource_mut::<Messages<CrisisCountermeasureEvent>>()
            .write(CrisisCountermeasureEvent {
                kind: CrisisCountermeasureKind::LaunchKineticDeflector,
                preferred_target: Some(imp_ent),
            });
        app.update();
    }

    assert!(
        !app.world()
            .resource::<CrisisInterceptorPool>()
            .bolts
            .is_empty(),
        "Kinetic Deflector must spawn interceptor bolts"
    );
    app.update();

    let resolved: Vec<_> = app
        .world()
        .resource::<Messages<CosmicEventResolvedEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    let _ = resolved;
    assert!(
        app.world().resource::<ProgressionState>().crises_averted >= 1,
        "Deflecting all PlanetKiller impactors must increment crises_averted"
    );

    // Part B: Automated PlanetaryDefenseGrid intercepts inbound impactor within < 0.70 AU
    let mut grid_app = App::new();
    init_core_crisis_resources(&mut grid_app);
    grid_app.add_systems(Update, cosmic_event_lifecycle_system);
    grid_app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.05;
    grid_app
        .world_mut()
        .resource_mut::<UnlockedTechs>()
        .unlocked
        .insert(TechnologyId::PlanetaryDefenseGrid);

    let guarded = spawn_inhabited_world(grid_app.world_mut(), "Aegis", DVec3::X, 0.80);
    grid_app
        .world_mut()
        .entity_mut(guarded)
        .get_mut::<CivilizationState>()
        .unwrap()
        .defense_readiness = 0.65;

    let mut active = ActiveCosmicEvent::new(CosmicEventType::PlanetKillerSwarm, 1.0);
    active.phase = CosmicEventPhase::Active;
    active.target_entity = Some(guarded);
    active.target_name = "Aegis".to_string();
    active.threats_total = 1;
    grid_app
        .world_mut()
        .resource_mut::<CosmicEventManager>()
        .active_events
        .push(active);

    let imp = grid_app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Asteroid,
                name: "Inbound-1".to_string(),
            },
            Mass(4.0e-8),
            Radius(0.0005),
            SimPosition(DVec3::X + DVec3::new(0.45, 0.0, 0.0)),
            SimVelocity(DVec3::new(-2.0, 0.0, std::f64::consts::TAU)),
            SimAcceleration::default(),
            CrisisImpactor {
                target_entity: Some(guarded),
                target_name: "Aegis".to_string(),
                deflected: false,
                initial_inbound_dir: DVec3::NEG_X,
                closest_approach_au: 0.45,
            },
        ))
        .id();

    grid_app.update();
    assert!(
        grid_app
            .world()
            .entity(imp)
            .get::<CrisisImpactor>()
            .unwrap()
            .deflected,
        "PlanetaryDefenseGrid must automatically deflect impactor inside 0.70 AU"
    );
    assert!(
        !grid_app
            .world()
            .resource::<CrisisInterceptorPool>()
            .bolts
            .is_empty()
    );
    let _ = world_ent;
}

#[test]
fn test_great_filter_crisis_and_grb_warning_overdrive() {
    // 1. GreatFilterCrisis + Climate Stabilizer
    let mut app = App::new();
    init_core_crisis_resources(&mut app);
    app.add_systems(
        Update,
        (
            handle_crisis_countermeasure_system,
            cosmic_event_lifecycle_system.after(handle_crisis_countermeasure_system),
        ),
    );
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 1.0;
    let world_ent = spawn_inhabited_world(app.world_mut(), "Gaia", DVec3::X, 0.90);

    let mut gf = ActiveCosmicEvent::new(CosmicEventType::GreatFilterCrisis, 1.0);
    gf.phase = CosmicEventPhase::Active;
    gf.target_entity = Some(world_ent);
    gf.target_name = "Gaia".to_string();
    app.world_mut()
        .resource_mut::<CosmicEventManager>()
        .active_events
        .push(gf);

    app.update();
    let stress_before = app
        .world()
        .entity(world_ent)
        .get::<CivilizationState>()
        .unwrap()
        .great_filter_stress;
    let dust_before = app
        .world()
        .entity(world_ent)
        .get::<TerraformingAtmosphere>()
        .unwrap()
        .impact_dust_optical_depth;
    assert!(
        dust_before > 0.0,
        "GreatFilterCrisis should raise dust depth"
    );

    app.world_mut()
        .resource_mut::<Messages<CrisisCountermeasureEvent>>()
        .write(CrisisCountermeasureEvent {
            kind: CrisisCountermeasureKind::DeployClimateStabilizer,
            preferred_target: Some(world_ent),
        });
    app.update();
    app.update();

    let civ_after = app
        .world()
        .entity(world_ent)
        .get::<CivilizationState>()
        .unwrap();
    assert!(civ_after.great_filter_stress < stress_before);

    // 2. GRB Warning lead time doubled by GravitationalLensArray + ShieldOverdrive protection
    let mut grb_app = App::new();
    init_core_crisis_resources(&mut grb_app);
    grb_app.add_systems(
        Update,
        (
            cosmic_event_scheduler_system,
            handle_crisis_countermeasure_system.after(cosmic_event_scheduler_system),
            cosmic_event_lifecycle_system.after(handle_crisis_countermeasure_system),
        ),
    );
    grb_app
        .world_mut()
        .resource_mut::<UnlockedTechs>()
        .unlocked
        .insert(TechnologyId::GravitationalLensArray);

    let earth = spawn_inhabited_world(grb_app.world_mut(), "Earth", DVec3::X, 0.85);
    {
        let mut mgr = grb_app.world_mut().resource_mut::<CosmicEventManager>();
        mgr.total_hazards_survived = 1; // selects GammaRayBurst in scheduler cycle
        mgr.hazard_timer_years = mgr.hazard_interval_years + 1.0;
    }
    grb_app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.1;
    grb_app.update();

    let mgr = grb_app.world().resource::<CosmicEventManager>();
    let ev = mgr.primary_active_event().expect("Scheduler must emit GRB");
    assert_eq!(ev.event_type, CosmicEventType::GammaRayBurst);
    assert_eq!(ev.phase, CosmicEventPhase::Warning);
    assert!(
        ev.warning_remaining_years > 2.5,
        "GravitationalLensArray should extend warning window above base 2.5y"
    );

    grb_app
        .world_mut()
        .resource_mut::<Messages<CrisisCountermeasureEvent>>()
        .write(CrisisCountermeasureEvent {
            kind: CrisisCountermeasureKind::ActivateShieldOverdrive,
            preferred_target: Some(earth),
        });
    grb_app.world_mut().resource_mut::<SimTime>().current_dt_yr = 4.5;
    grb_app.update();
    let p_bar = grb_app
        .world()
        .entity(earth)
        .get::<VolatileInventory>()
        .unwrap()
        .atmospheric_pressure_bar;
    assert!(
        (p_bar - 1.0).abs() < 1e-4,
        "Shield overdrive must preserve atmosphere from GRB stripping, got {p_bar}"
    );
}

#[test]
fn test_3d_civilization_visuals_shkadov_engine_and_crisis_banner_ui() {
    let mut app = App::new();
    app.init_resource::<SimTime>()
        .init_resource::<UnlockedTechs>()
        .init_resource::<CosmicEventManager>()
        .init_resource::<ProgressionState>()
        .init_resource::<HudVisibilityState>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<MegastructureMaterial>>()
        .init_resource::<Assets<PlanetaryShieldMaterial>>()
        .add_systems(
            Update,
            (
                sync_civilization_visuals,
                sync_shkadov_stellar_engine_visuals,
                update_crisis_alert_banner_ui,
            ),
        );

    {
        let mut unlocked = app.world_mut().resource_mut::<UnlockedTechs>();
        unlocked.unlocked.insert(TechnologyId::OrbitalMirrorArray);
        unlocked.unlocked.insert(TechnologyId::StellarEngine);
    }

    let planet = spawn_inhabited_world(app.world_mut(), "Solaria", DVec3::X, 1.15);
    app.world_mut()
        .entity_mut(planet)
        .get_mut::<CivilizationState>()
        .unwrap()
        .shield_overdrive_secs = 15.0;

    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::YellowDwarf,
            name: "Sun".to_string(),
        },
        CentralStar,
        SimPosition(DVec3::ZERO),
        DysonSwarm {
            completion_fraction: 0.80,
            ..Default::default()
        },
    ));

    let banner_root = app
        .world_mut()
        .commands()
        .spawn(Node::default())
        .with_children(spawn_crisis_alert_banner)
        .id();
    app.world_mut().flush();
    let _ = banner_root;

    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&OrbitalHabitatRingVisual>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&PlanetaryShieldVisual>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&OrbitalMirrorVisual>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&ShkadovEngineVisual>()
            .iter(app.world())
            .count(),
        1
    );

    // Banner hidden when no crisis is active
    {
        let mut q = app
            .world_mut()
            .query_filtered::<&Node, With<CrisisBannerContainer>>();
        assert_eq!(q.single(app.world()).unwrap().display, Display::None);
    }

    // Banner revealed when crisis is in Warning/Active
    let mut warn =
        ActiveCosmicEvent::new_with_warning(CosmicEventType::PlanetKillerSwarm, 1.0, 2.0);
    warn.target_name = "Solaria".to_string();
    app.world_mut()
        .resource_mut::<CosmicEventManager>()
        .active_events
        .push(warn);
    app.update();

    {
        let mut q = app
            .world_mut()
            .query_filtered::<&Node, With<CrisisBannerContainer>>();
        assert_eq!(q.single(app.world()).unwrap().display, Display::Flex);
        let mut tq = app
            .world_mut()
            .query_filtered::<&Text, With<CrisisBannerTitleText>>();
        assert!(tq.single(app.world()).unwrap().0.contains("Solaria"));
    }
}

#[test]
fn test_track6_progression_civilization_and_impactor_save_load_round_trip() {
    let save_path = "target/test_saves_load/track6_roundtrip.json";

    // Build source world & save
    let mut src_app = App::new();
    init_core_crisis_resources(&mut src_app);
    src_app
        .add_message::<SaveSystemEvent>()
        .add_systems(Update, handle_save_system_events);

    src_app.world_mut().resource_mut::<ProgressionState>().mode = GameMode::Progression;
    src_app
        .world_mut()
        .resource_mut::<ProgressionState>()
        .crises_averted = 3;
    src_app
        .world_mut()
        .resource_mut::<ScienceData>()
        .add_science(900.0);
    src_app
        .world_mut()
        .resource_mut::<UnlockedTechs>()
        .unlocked
        .insert(TechnologyId::PlanetaryDefenseGrid);
    {
        let mut mgr = src_app.world_mut().resource_mut::<CosmicEventManager>();
        mgr.auto_hazards_enabled = false;
        mgr.total_hazards_survived = 4;
    }

    let gaia = spawn_inhabited_world(src_app.world_mut(), "Gaia", DVec3::X, 1.42);
    src_app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::Asteroid,
            name: "Planet-Killer Test-1".to_string(),
        },
        Mass(4.0e-8),
        Radius(0.0005),
        SimPosition(DVec3::new(1.5, 0.0, 0.0)),
        SimVelocity(DVec3::new(-2.0, 0.0, 0.0)),
        Temperature(200.0),
        Luminosity(0.0),
        Composition::rocky(),
        SpinState::default(),
        CrisisImpactor {
            target_entity: Some(gaia),
            target_name: "Gaia".to_string(),
            deflected: false,
            initial_inbound_dir: DVec3::NEG_X,
            closest_approach_au: 0.5,
        },
    ));

    src_app
        .world_mut()
        .resource_mut::<Messages<SaveSystemEvent>>()
        .write(SaveSystemEvent {
            filename: save_path.to_string(),
        });
    src_app.update();

    // Load into a fresh destination app
    let mut dst_app = App::new();
    init_core_crisis_resources(&mut dst_app);
    dst_app
        .init_resource::<PlayerInteractionState>()
        .init_resource::<TheiaImpactState>()
        .init_resource::<ProbeCounter>()
        .add_message::<LoadSystemEvent>()
        .add_systems(Update, handle_load_system_events);

    dst_app
        .world_mut()
        .resource_mut::<Messages<LoadSystemEvent>>()
        .write(LoadSystemEvent {
            filename: save_path.to_string(),
        });
    dst_app.update();

    let prog = dst_app.world().resource::<ProgressionState>();
    assert_eq!(prog.mode, GameMode::Progression);
    assert_eq!(prog.crises_averted, 3);
    assert_eq!(prog.active_civilization_count, 1);
    assert!((prog.highest_kardashev - 1.42).abs() < 1e-4);

    assert!(
        dst_app
            .world()
            .resource::<UnlockedTechs>()
            .has(TechnologyId::PlanetaryDefenseGrid)
    );
    assert!((dst_app.world().resource::<ScienceData>().points - 1000.0).abs() < 1e-4);
    let mgr = dst_app.world().resource::<CosmicEventManager>();
    assert!(!mgr.auto_hazards_enabled);
    assert_eq!(mgr.total_hazards_survived, 4);

    let gaia_loaded = dst_app
        .world_mut()
        .query::<(Entity, &CelestialBody, &CivilizationState)>()
        .iter(dst_app.world())
        .find(|(_, b, _)| b.name == "Gaia")
        .map(|(e, _, c)| (e, *c))
        .expect("Gaia CivilizationState must survive save/load");
    assert!((gaia_loaded.1.kardashev_level - 1.42).abs() < 1e-4);

    let imp = dst_app
        .world_mut()
        .query::<&CrisisImpactor>()
        .single(dst_app.world())
        .expect("CrisisImpactor must survive save/load");
    assert_eq!(imp.target_entity, Some(gaia_loaded.0));

    let _ = std::fs::remove_file(save_path);
}

#[test]
fn test_track6_3d_tech_meshes_have_uvs_vertex_colors_normals_and_closed_geometry() {
    use bevy::mesh::VertexAttributeValues;

    let meshes = [
        (
            "DysonCollectorRing",
            generate_dyson_collector_ring_mesh(),
            1500,
        ),
        ("ShkadovEngine", generate_shkadov_hemisphere_mesh(), 800),
        (
            "OrbitalHabitatRing",
            generate_orbital_habitat_ring_mesh(),
            1200,
        ),
        (
            "OrbitalMirrorArray",
            generate_orbital_mirror_array_mesh(),
            600,
        ),
    ];

    for (label, mesh, min_verts) in meshes {
        assert!(
            mesh.count_vertices() >= min_verts,
            "{label} should have rich 3D geometry (>= {min_verts} verts), got {}",
            mesh.count_vertices()
        );
        assert!(
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some(),
            "{label} missing normals"
        );
        assert!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some(),
            "{label} missing UVs"
        );
        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("{label} must include Float32x4 ATTRIBUTE_COLOR material zones");
        };
        let has_chassis = colors.iter().any(|c| c[0] < 0.5);
        let has_primary = colors.iter().any(|c| c[0] > 0.5);
        assert!(
            has_chassis && has_primary,
            "{label} must distinguish chassis vs primary alloy"
        );

        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("{label} missing Float32x3 positions");
        };
        assert!(
            positions
                .iter()
                .all(|p| p[0].is_finite() && p[1].is_finite() && p[2].is_finite()),
            "{label} contains non-finite vertex coordinates"
        );
    }
}

#[test]
fn test_planetary_megastructures_hug_true_scale_host_and_mirrors_fly_freely() {
    use bevy::mesh::VertexAttributeValues;
    use protostellar::rendering::origin::RenderOrigin;

    let mut app = App::new();
    app.init_resource::<SimTime>()
        .init_resource::<UnlockedTechs>()
        .init_resource::<CosmicEventManager>()
        .init_resource::<RenderOrigin>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<MegastructureMaterial>>()
        .init_resource::<Assets<PlanetaryShieldMaterial>>()
        .add_systems(Update, sync_civilization_visuals);

    {
        let mut unlocked = app.world_mut().resource_mut::<UnlockedTechs>();
        unlocked.unlocked.insert(TechnologyId::OrbitalMirrorArray);
        unlocked
            .unlocked
            .insert(TechnologyId::MagnetosphericShielding);
    }

    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    app.world_mut().resource_mut::<RenderOrigin>().0 = earth_pos;

    let earth = spawn_inhabited_world(app.world_mut(), "Earth", earth_pos, 1.15);
    {
        let mut ent = app.world_mut().entity_mut(earth);
        let mut civ = ent.get_mut::<CivilizationState>().unwrap();
        civ.orbital_infrastructure = 0.50;
        civ.shield_overdrive_secs = 10.0;
    }

    app.update();

    let r_earth = 0.0005_f32; // spawn_inhabited_world host radius
    let ring_tf = *app
        .world_mut()
        .query_filtered::<&Transform, With<OrbitalHabitatRingVisual>>()
        .single(app.world())
        .unwrap();
    let shield_tf = *app
        .world_mut()
        .query_filtered::<&Transform, With<PlanetaryShieldVisual>>()
        .single(app.world())
        .unwrap();
    let mirror_tf = *app
        .world_mut()
        .query_filtered::<&Transform, With<OrbitalMirrorVisual>>()
        .single(app.world())
        .unwrap();

    // Must hug true-scale Earth (~4.26e-5 AU) with zero 0.02 AU artificial inflation
    assert!(
        ring_tf.translation.length() < 1e-9
            && shield_tf.translation.length() < 1e-9
            && mirror_tf.translation.length() < 1e-9,
        "Megastructures on focused Earth must render at RenderOrigin Vec3::ZERO"
    );
    assert!(
        (shield_tf.scale.x - r_earth * 1.12).abs() < 1e-8,
        "Planetary shield scale ({}) must equal 1.12 * R_earth ({})",
        shield_tf.scale.x,
        r_earth * 1.12
    );
    assert!(
        (ring_tf.scale.x - r_earth * 1.26).abs() < 1e-8,
        "Orbital habitat ring scale ({}) must equal (1.22 + 0.08 * 0.5) * R_earth ({})",
        ring_tf.scale.x,
        r_earth * 1.26
    );
    assert!(
        (mirror_tf.scale.x - r_earth * 1.54).abs() < 1e-8 && mirror_tf.scale.x < 0.02,
        "Orbital mirror array scale ({}) must equal 1.54 * R_earth without 0.02 AU floor",
        mirror_tf.scale.x
    );

    // Free-flying JWST heliostats: no connecting bridge vertices near inter-satellite midpoints
    let mirror_mesh = generate_orbital_mirror_array_mesh();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mirror_mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("OrbitalMirrorArray missing Float32x3 positions");
    };
    let mid_angle = std::f32::consts::PI / 8.0; // midpoint between sat 0 (0) and sat 1 (pi/4)
    let mid_pt = Vec3::new(mid_angle.cos(), 0.0, mid_angle.sin());
    let has_bridge_vertex = positions
        .iter()
        .any(|p| Vec3::from_array(*p).distance(mid_pt) < 0.08);
    assert!(
        !has_bridge_vertex,
        "Orbital mirror array satellites must be free-flying without connecting ring struts at midpoint {mid_pt:?}"
    );
}

#[test]
fn test_universal_civilization_bootstrap_domed_colonies_infra_toggles_and_black_marble_shader() {
    // 1. Habitable Earth-like world -> open-biosphere civilization
    let (earth_civ, earth_bio) = CivilizationState::bootstrap_for_world(None, 288.0, true, 100.0);
    assert!(!earth_civ.is_domed_colony);
    assert!(earth_bio.biomass_coverage_frac >= 0.65);
    assert!(earth_bio.technosignature > 0.25);

    // 2. Dry/cold Mars or airless Moon -> pressurized domed colony without overwriting terrain into oceans
    let (mut mars_civ, mars_bio) =
        CivilizationState::bootstrap_for_world(None, 210.0, false, 100.0);
    assert!(mars_civ.is_domed_colony);
    assert!(mars_civ.is_colony);
    assert_eq!(mars_bio.biomass_coverage_frac, 0.0);
    assert!(mars_bio.technosignature > 0.25);
    assert!(
        mars_civ
            .format_telemetry()
            .contains("Pressurized Domed Colony")
    );

    // 3. Explicit infrastructure toggles spawn Ring, Solar Mirrors, and Shield on any world (even without tech unlocks)
    mars_civ.kardashev_level = 0.70;
    mars_civ.orbital_infrastructure = 0.10;
    mars_civ.force_orbital_ring = true;
    mars_civ.force_solar_mirrors = true;
    mars_civ.force_planetary_shield = true;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    init_core_crisis_resources(&mut app);
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<MegastructureMaterial>>()
        .init_resource::<Assets<PlanetaryShieldMaterial>>()
        .init_resource::<protostellar::rendering::origin::RenderOrigin>();
    app.add_systems(Update, sync_civilization_visuals);

    let r_mars = 2.266e-5_f32;
    let mars_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Mars".into(),
            },
            SimPosition(DVec3::new(1.52, 0.0, 0.0)),
            Radius(f64::from(r_mars)),
            Transform::from_scale(Vec3::splat(r_mars)),
            mars_civ,
            mars_bio,
        ))
        .id();

    app.update();
    app.update();

    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<OrbitalHabitatRingVisual>>()
            .iter(app.world())
            .count(),
        1,
        "force_orbital_ring must spawn OrbitalHabitatRingVisual on Mars"
    );
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<OrbitalMirrorVisual>>()
            .iter(app.world())
            .count(),
        1,
        "force_solar_mirrors must spawn OrbitalMirrorVisual on Mars"
    );
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<PlanetaryShieldVisual>>()
            .iter(app.world())
            .count(),
        1,
        "force_planetary_shield must spawn PlanetaryShieldVisual on Mars"
    );

    // 3b. In Surface View on the host planet, the Orbital Ring & Solar Mirrors must remain visible overhead!
    app.world_mut()
        .spawn(protostellar::rendering::camera::PanOrbitCamera {
            surface_horizon_mode: true,
            target_entity: Some(mars_ent),
            ..default()
        });
    app.update();
    app.update();
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<OrbitalHabitatRingVisual>>()
            .iter(app.world())
            .count(),
        1,
        "OrbitalHabitatRingVisual must remain visible overhead in Surface View"
    );
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<OrbitalMirrorVisual>>()
            .iter(app.world())
            .count(),
        1,
        "OrbitalMirrorVisual must remain visible overhead in Surface View"
    );

    // 4. Serde round-trip preserves new fields cleanly
    let json = serde_json::to_string(&mars_civ).expect("CivilizationState must serialize");
    let decoded: CivilizationState =
        serde_json::from_str(&json).expect("CivilizationState must deserialize");
    assert_eq!(decoded, mars_civ);

    // 5. Shader guardrails: Black Marble city networks, cloud underglow, phytoplankton, and 557.7nm oxygen airglow
    let planet_src =
        std::fs::read_to_string("assets/shaders/planet.wesl").expect("planet.wesl must read");
    assert!(
        planet_src.contains("fn evaluate_civilization_night_lights(")
            && planet_src.contains("let cloud_underglow = city_emissive")
            && planet_src.contains("let oxy_557nm_airglow =")
            && planet_src.contains("let bloom_ribbon ="),
        "planet.wesl must include Black Marble city networks, cloud underglow, phytoplankton blooms, and 557.7nm airglow"
    );
}
