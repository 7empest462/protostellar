//! Test module generated from simulation_tests.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;

fn setup_lhb_test_app() -> (App, Entity) {
    use bevy::prelude::*;
    use protostellar::game::phases::{
        monitor_phase_transitions, LateHeavyBombardmentState, PhaseManager, SystemPhase,
    };
    use protostellar::simulation::disk::update_late_heavy_bombardment_cascade;
    use protostellar::simulation::resources::{DiskParameters, SimTime, TimeWarp};
    use protostellar::simulation::thermodynamics::StarIgnitionEvent;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<PhaseManager>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<DiskParameters>();
    app.add_message::<StarIgnitionEvent>();

    app.add_systems(
        Update,
        (
            monitor_phase_transitions,
            update_late_heavy_bombardment_cascade,
        ),
    );

    // Spawn central star (1.0 M_sun)
    app.world_mut().spawn((
        CentralStar,
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Radius(SOLAR_RADIUS_AU),
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::Protostar,
        },
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 1.6,
        },
    ));

    // Spawn Earth at 1.0 AU with initial dry surface
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI)),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
            VolatileInventory {
                delivered_water_m_earth: 0.00002,
                cometary_impact_count: 0,
                ocean_coverage_frac: 0.03,
                atmospheric_pressure_bar: 0.2,
            },
            InternalDifferentiation {
                is_differentiated: true,
                magnetic_field_gauss: 0.35,
                ..Default::default()
            },
        ))
        .id();

    (app, earth_ent)
}

fn verify_initial_lhb_state(app: &App) {
    use protostellar::game::phases::{LateHeavyBombardmentState, PhaseManager, SystemPhase};

    let phase_mgr = app.world().resource::<PhaseManager>();
    assert_eq!(phase_mgr.current_phase, SystemPhase::ProtoplanetaryDisk);
    let lhb = app.world().resource::<LateHeavyBombardmentState>();
    assert!(!lhb.is_active);
}

fn verify_manual_lhb_trigger(app: &mut App) {
    use protostellar::game::phases::{
        LateHeavyBombardmentState, MilestoneId, PhaseManager, SystemPhase,
    };

    {
        let mut lhb = app.world_mut().resource_mut::<LateHeavyBombardmentState>();
        lhb.manual_trigger_requested = true;
    }

    app.update();

    let phase_mgr = app.world().resource::<PhaseManager>();
    assert_eq!(
        phase_mgr.current_phase,
        SystemPhase::LateHeavyBombardment,
        "LHB must be unconditionally entered when manual_trigger_requested is true!"
    );
    let lhb = app.world().resource::<LateHeavyBombardmentState>();
    assert!(lhb.is_active, "lhb_state.is_active must be true!");
    assert!(
        lhb.resonance_crossed,
        "Resonance crossing must be immediately triggered on manual LHB request!"
    );

    let lhb_milestone = phase_mgr
        .milestones
        .iter()
        .find(|m| m.id == MilestoneId::LateHeavyBombardment)
        .expect("LHB milestone must exist");
    assert!(
        lhb_milestone.achieved,
        "MilestoneId::LateHeavyBombardment must be unlocked upon LHB trigger!"
    );
}

fn verify_active_cascade_impactors(app: &mut App) {
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::resources::SimTime;

    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 0.5;
        sim_time.elapsed_years = 100.0;
    }

    for _ in 0..15 {
        app.update();
    }

    let world = app.world();
    let lhb = world.resource::<LateHeavyBombardmentState>();
    assert!(
        lhb.comets_scattered > 0,
        "Active cascade must increment comets_scattered (found {})",
        lhb.comets_scattered
    );

    let mut impactor_count = 0;
    let mut query = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &SimVelocity, &Mass)>();
    for (body, pos, vel, _) in query.iter(app.world()) {
        if matches!(body.body_type, BodyType::Asteroid | BodyType::Comet) {
            impactor_count += 1;
            let r = pos.0.length();
            let v = vel.0.length();
            let spec_e = 0.5 * v * v - G_ASTRO / r;
            if spec_e < 0.0 {
                let a = -G_ASTRO / (2.0 * spec_e);
                let h = pos.0.cross(vel.0).length();
                let e = ((1.0_f64 - (h * h) / (G_ASTRO * a)) as f64)
                    .max(0.0_f64)
                    .sqrt();
                let q = a * (1.0 - e);
                assert!(
                    q <= 1.8,
                    "Impactor perihelion ({:.2} AU) must cross inner planetary orbits!",
                    q
                );
            }
        }
    }
    assert!(
        impactor_count >= 5,
        "Cascade spawner must generate multiple active impactors in ECS (found {})",
        impactor_count
    );
}

fn verify_volatile_water_delivery(app: &mut App, earth_ent: Entity) {
    use protostellar::game::phases::{LateHeavyBombardmentState, MilestoneId, PhaseManager};

    {
        let mut earth_vol = app
            .world_mut()
            .get_mut::<VolatileInventory>(earth_ent)
            .expect("Earth must have VolatileInventory");
        earth_vol.delivered_water_m_earth = 0.00058; // > 0.0005 Earth masses
        earth_vol.cometary_impact_count = 14;
        earth_vol.ocean_coverage_frac =
            (earth_vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
    }

    app.update();

    let phase_mgr = app.world().resource::<PhaseManager>();
    let lhb = app.world().resource::<LateHeavyBombardmentState>();

    assert!(
        lhb.water_delivered_earth_masses >= 0.0005,
        "LHB state water_delivered_earth_masses ({}) must track total delivered water!",
        lhb.water_delivered_earth_masses
    );

    let ocean_milestone = phase_mgr
        .milestones
        .iter()
        .find(|m| m.id == MilestoneId::VolatileOceanDelivery)
        .expect("Ocean milestone must exist");
    assert!(
        ocean_milestone.achieved,
        "MilestoneId::VolatileOceanDelivery must unlock once delivered water >= 0.0005 M_earth!"
    );

    let earth_vol = app.world().get::<VolatileInventory>(earth_ent).unwrap();
    assert!(
        earth_vol.ocean_coverage_frac >= 0.70,
        "Earth ocean coverage ({:.1}%) must exceed 70% after bombardment water delivery!",
        earth_vol.ocean_coverage_frac * 100.0
    );
}

#[test]
fn test_late_heavy_bombardment_guaranteed_trigger_and_impactors() {
    let (mut app, earth_ent) = setup_lhb_test_app();
    app.update();
    verify_initial_lhb_state(&app);
    verify_manual_lhb_trigger(&mut app);
    verify_active_cascade_impactors(&mut app);
    verify_volatile_water_delivery(&mut app, earth_ent);
}

fn verify_body_classification_rules() {
    use protostellar::game::ui::{is_canonical_major_planet, is_embryo_body, is_major_body};

    // 1. Minor bodies (Asteroids, Comets, Planetesimals) are never major bodies
    let minor_cases = [
        ("Ceres", BodyType::Asteroid, 0.00015),
        ("Vesta", BodyType::Asteroid, 0.00004),
        ("1P/Halley", BodyType::Comet, 0.000001),
        ("C/1995 O1 Hale-Bopp", BodyType::Comet, 0.000005),
        ("Asteroid-2.7AU", BodyType::Asteroid, 0.00001),
        ("Comet-25.0AU", BodyType::Comet, 0.00001),
        ("Planetesimal-1.5AU", BodyType::Planetesimal, 0.00001),
    ];
    for (name, b_type, mass_earth) in minor_cases {
        assert!(!is_major_body(
            name,
            b_type,
            false,
            mass_earth * EARTH_MASS_SOLAR
        ));
    }

    // 2. Protoplanetary Embryos
    let embryo_cases = [
        "Callisto Embryo",
        "Titan Embryo",
        "Embryo-1.2AU",
        "Embryo #1",
    ];
    for name in embryo_cases {
        assert!(is_embryo_body(name, BodyType::Protoplanet));
        assert!(!is_major_body(
            name,
            BodyType::Protoplanet,
            false,
            0.10 * EARTH_MASS_SOLAR
        ));
    }

    // 3. Central Star and Major Worlds
    assert!(is_major_body("Sun", BodyType::Protostar, true, 1.0));
    let major_worlds = [
        ("Theia", BodyType::Protoplanet, 0.12),
        ("Proto-Mercury", BodyType::Protoplanet, 0.06),
        ("Mercury", BodyType::TerrestrialPlanet, 0.055),
        ("Venus", BodyType::TerrestrialPlanet, 0.815),
        ("Earth", BodyType::TerrestrialPlanet, 1.0),
        ("Mars", BodyType::TerrestrialPlanet, 0.107),
        ("Jupiter", BodyType::GasGiant, 317.8),
        ("Saturn", BodyType::GasGiant, 95.2),
        ("Uranus", BodyType::IceGiant, 14.5),
        ("Neptune", BodyType::IceGiant, 17.1),
        ("Pluto (Dwarf Planet)", BodyType::TerrestrialPlanet, 0.00218),
        (
            "Planet Nine (Super-Earth / Ice Giant)",
            BodyType::IceGiant,
            5.50,
        ),
        ("Moon", BodyType::Moon, 0.0123),
    ];
    for (name, b_type, mass_earth) in major_worlds {
        assert!(is_major_body(
            name,
            b_type,
            false,
            mass_earth * EARTH_MASS_SOLAR
        ));
    }
    assert!(is_canonical_major_planet("Pluto (Dwarf Planet)"));
    assert!(is_canonical_major_planet(
        "Planet Nine (Super-Earth / Ice Giant)"
    ));
}

fn verify_planetesimal_spawner_and_quick_bar_defaults() {
    use protostellar::game::ui::QuickBarState;
    use protostellar::simulation::disk::PlanetesimalSpawner;

    let spawner = PlanetesimalSpawner::default();
    assert_eq!(
        spawner.max_ecs_bodies, 1024,
        "PlanetesimalSpawner must have 1024 body capacity!"
    );
    let qb = QuickBarState::default();
    assert!(!qb.show_embryos);
    assert!(!qb.show_minor_bodies);
    assert!(!qb.is_minimized);
}

fn verify_solar_system_scenario_belts() {
    use bevy::prelude::*;
    use protostellar::game::phases::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::spawn_solar_nebula_mmsn;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PhaseManager>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.add_message::<protostellar::simulation::thermodynamics::StarIgnitionEvent>();
    app.add_systems(Update, monitor_phase_transitions);

    let mut disk_params = DiskParameters::default();
    let _star_ent = spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let phase_mgr = app.world().resource::<PhaseManager>();
    assert_eq!(
        phase_mgr.asteroid_count, 8,
        "Solar System MMSN must spawn 8 initial Asteroid Belt bodies (Ceres, Vesta, Pallas, etc.)"
    );
    assert_eq!(
        phase_mgr.comet_count, 6,
        "Solar System MMSN must spawn 6 initial Kuiper Belt cometary bodies (Halley, Encke, etc.)"
    );
    assert!(
        phase_mgr.planet_count + phase_mgr.protoplanet_count >= 10,
        "Solar System MMSN must have major planets and embryos tracked (found {} planets + {} protoplanets)",
        phase_mgr.planet_count,
        phase_mgr.protoplanet_count
    );
}

fn verify_stellar_wind_radiation_push() {
    let pos_inner = DVec3::new(1.2, 0.0, 0.0);
    let r_cyl = pos_inner.x;
    let b_mass = 0.00001 * EARTH_MASS_SOLAR;
    let push_mag =
        0.35 * (1.0 - (r_cyl / 2.0)).max(0.0) / (1.0 + b_mass / (EARTH_MASS_SOLAR * 0.001));
    assert!(
        push_mag > 0.1,
        "Inner minor body must feel positive outward radiation pressure push ({push_mag}) toward the belt"
    );
}

#[test]
fn test_minor_bodies_belt_formation_1024_capacity_and_hud_category() {
    verify_body_classification_rules();
    verify_planetesimal_spawner_and_quick_bar_defaults();
    verify_solar_system_scenario_belts();
    verify_stellar_wind_radiation_push();
}

#[test]
fn test_spawn_protoplanetary_disk_spawns_pluto_and_planet_nine() {
    use bevy::prelude::*;
    use protostellar::game::ui::collect_sorted_system_worlds;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::spawn_protoplanetary_disk;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let disk_params = DiskParameters::default();
    let config = SimulationConfig::default();

    let _star = spawn_protoplanetary_disk(&mut app.world_mut().commands(), &disk_params, &config);
    app.update();

    let mut bodies = Vec::new();
    for (entity, body, pos, _mass, _radius, _opt_star) in app
        .world_mut()
        .query::<(
            Entity,
            &CelestialBody,
            &SimPosition,
            &Mass,
            &Radius,
            Option<&CentralStar>,
        )>()
        .iter(app.world())
    {
        bodies.push((entity, body.name.clone(), body.body_type, pos.0.length()));
    }

    let pluto = bodies.iter().find(|b| b.1.contains("Pluto"));
    assert!(
        pluto.is_some(),
        "Pluto must spawn immediately in spawn_protoplanetary_disk!"
    );
    let pluto = pluto.unwrap();
    assert_eq!(pluto.2, BodyType::TerrestrialPlanet);
    assert!(
        (pluto.3 - 39.48).abs() < 5.0,
        "Pluto distance must be ~39.48 AU (found {:.2})",
        pluto.3
    );

    let planet_nine = bodies.iter().find(|b| b.1.contains("Planet Nine"));
    assert!(
        planet_nine.is_some(),
        "Planet Nine must spawn immediately in spawn_protoplanetary_disk!"
    );
    let p9 = planet_nine.unwrap();
    assert_eq!(p9.2, BodyType::IceGiant);
    assert!(
        (p9.3 - 380.0).abs() < 50.0,
        "Planet Nine distance must be ~380 AU (found {:.2})",
        p9.3
    );

    // Verify collect_sorted_system_worlds contains both worlds
    let query_items: Vec<_> = app
        .world_mut()
        .query::<(
            Entity,
            &CelestialBody,
            &SimPosition,
            &Mass,
            &Radius,
            Option<&CentralStar>,
        )>()
        .iter(app.world())
        .collect();

    let system_worlds = collect_sorted_system_worlds(query_items);
    let world_names: Vec<&str> = system_worlds.iter().map(|w| w.name.as_str()).collect();
    assert!(
        world_names.iter().any(|n| n.contains("Pluto")),
        "Major worlds selector bar must include Pluto: {:?}",
        world_names
    );
    assert!(
        world_names.iter().any(|n| n.contains("Planet Nine")),
        "Major worlds selector bar must include Planet Nine: {:?}",
        world_names
    );
}

fn setup_quick_selector_app() -> (App, Entity, Entity, Entity) {
    use bevy::prelude::*;
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::game::ui::{
        update_quick_body_selector_bar, HudActionTooltipText, HudVisibilityState,
        NotificationToast, PlanetBuilderState, QuickBarState, QuickBodySelectorBar,
    };
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::simulation::resources::{
        DiskParameters, PlayerInteractionState, SimTime, SimulationConfig, TimeWarp,
    };
    use protostellar::simulation::scenarios::LoadScenarioEvent;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<QuickBarState>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<protostellar::game::ui::TelemetryPanelState>();
    app.init_resource::<protostellar::simulation::telemetry::SimulationTelemetryHistory>();
    app.init_resource::<HudVisibilityState>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.add_message::<LoadScenarioEvent>();

    // Spawn camera
    let cam_ent = app
        .world_mut()
        .spawn(PanOrbitCamera {
            focus: Vec3::ZERO,
            target_focus: Vec3::ZERO,
            radius: 50.0,
            target_radius: 50.0,
            ..default()
        })
        .id();

    // Spawn tooltip text entity
    app.world_mut().spawn((Text::new(""), HudActionTooltipText));

    // Spawn celestial bodies: Sun, Earth, Jupiter
    let _sun_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Sun".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            CentralStar,
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Mass(1.0),
            Radius(0.00465),
            Composition::default(),
        ))
        .id();

    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let _earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(earth_pos),
            SimVelocity(DVec3::ZERO),
            Mass(0.000003003),
            Radius(0.0000426),
            Composition::default(),
        ))
        .id();

    let jupiter_pos = DVec3::new(5.2, 0.0, 0.0);
    let jupiter_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Jupiter".to_string(),
                body_type: BodyType::GasGiant,
            },
            SimPosition(jupiter_pos),
            SimVelocity(DVec3::ZERO),
            Mass(0.000954),
            Radius(0.000467),
            Composition::default(),
        ))
        .id();

    // Spawn QuickBodySelectorBar
    let bar_ent = app
        .world_mut()
        .spawn((QuickBodySelectorBar, Node::default()))
        .id();

    app.add_systems(Update, update_quick_body_selector_bar);
    (app, bar_ent, cam_ent, jupiter_ent)
}

fn verify_selector_bar_buttons(app: &App, bar_ent: Entity, jupiter_ent: Entity) -> Entity {
    use bevy::prelude::*;
    use protostellar::game::ui::UiButtonAction;

    let children = app
        .world()
        .get::<Children>(bar_ent)
        .expect("bar_ent must have children");
    assert!(
        !children.is_empty(),
        "bar_ent must contain selector buttons"
    );

    let mut button_count = 0;
    let mut text_count = 0;
    let mut jupiter_btn_ent: Option<Entity> = None;

    for btn_child in children.iter() {
        if let Some(action) = app.world().get::<UiButtonAction>(btn_child) {
            button_count += 1;
            if *action == UiButtonAction::SelectEntity(jupiter_ent) {
                jupiter_btn_ent = Some(btn_child);
            }
        }
        if let Some(btn_grandchildren) = app.world().get::<Children>(btn_child) {
            for gc in btn_grandchildren.iter() {
                if app.world().get::<Text>(gc).is_some() {
                    text_count += 1;
                    assert!(
                        app.world().get::<Pickable>(gc) == Some(&Pickable::IGNORE),
                        "Text node {:?} inside button {:?} must have Pickable::IGNORE so clicks register on parent button!",
                        gc,
                        btn_child
                    );
                }
            }
        }
    }

    assert!(
        button_count >= 3,
        "Must have spawned Sun, Earth, Jupiter buttons"
    );
    assert!(text_count >= 3, "Must have spawned text labels");
    jupiter_btn_ent.expect("Must have spawned Jupiter selector button")
}

fn verify_selector_button_click(
    app: &mut App,
    jupiter_btn: Entity,
    cam_ent: Entity,
    jupiter_ent: Entity,
) {
    use bevy::prelude::*;
    use protostellar::game::ui::handle_ui_button_interactions;
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::simulation::resources::PlayerInteractionState;

    app.world_mut()
        .entity_mut(jupiter_btn)
        .insert(Interaction::Pressed);

    app.add_systems(Update, handle_ui_button_interactions);
    app.update();

    let player_state = app.world().resource::<PlayerInteractionState>();
    assert_eq!(
        player_state.selected_entity,
        Some(jupiter_ent),
        "Clicking Jupiter button must set selected_entity to jupiter_ent"
    );

    let cam = app.world().get::<PanOrbitCamera>(cam_ent).unwrap();
    assert_eq!(
        cam.target_entity,
        Some(jupiter_ent),
        "Camera target_entity must be set to Jupiter"
    );
    assert_eq!(
        cam.target_focus,
        Vec3::new(5.2, 0.0, 0.0),
        "Camera target_focus must snap immediately to Jupiter's coordinates"
    );
    assert_eq!(
        cam.focus,
        Vec3::new(5.2, 0.0, 0.0),
        "Camera focus must snap immediately to Jupiter's coordinates"
    );
}

fn verify_clean_recursive_despawn(app: &mut App, bar_ent: Entity) {
    use bevy::prelude::*;
    use protostellar::game::ui::QuickBarState;

    let mut initial_entities = Vec::new();
    let old_children: Vec<Entity> = app
        .world()
        .get::<Children>(bar_ent)
        .unwrap()
        .iter()
        .collect();
    for child in old_children {
        initial_entities.push(child);
        if let Some(gcs) = app.world().get::<Children>(child) {
            for gc in gcs.iter() {
                initial_entities.push(gc);
            }
        }
    }

    let mut qb_state = app.world_mut().resource_mut::<QuickBarState>();
    qb_state.show_embryos = true;

    app.update();

    for old_ent in initial_entities {
        assert!(
            app.world().get_entity(old_ent).is_err(),
            "Entity {:?} should have been cleanly despawned during bar rebuild",
            old_ent
        );
    }
}

#[test]
fn test_quick_body_selector_click_selection_and_clean_recursive_despawn() {
    let (mut app, bar_ent, cam_ent, jupiter_ent) = setup_quick_selector_app();
    app.update();
    let jupiter_btn = verify_selector_bar_buttons(&app, bar_ent, jupiter_ent);
    verify_selector_button_click(&mut app, jupiter_btn, cam_ent, jupiter_ent);
    verify_clean_recursive_despawn(&mut app, bar_ent);
}

#[test]
fn test_astronomical_belt_zoning_and_census() {
    use protostellar::simulation::disk::belts::{BeltCensus, BeltZone};

    // 1. Validate boundary zoning
    assert_eq!(BeltZone::from_distance_au(0.8), BeltZone::InnerSystem);
    assert_eq!(BeltZone::from_distance_au(2.09), BeltZone::InnerSystem);
    assert_eq!(BeltZone::from_distance_au(2.10), BeltZone::AsteroidBelt);
    assert_eq!(BeltZone::from_distance_au(2.77), BeltZone::AsteroidBelt);
    assert_eq!(BeltZone::from_distance_au(3.45), BeltZone::AsteroidBelt);
    assert_eq!(BeltZone::from_distance_au(5.20), BeltZone::TrojanCentaur);
    assert_eq!(BeltZone::from_distance_au(16.0), BeltZone::TrojanCentaur);
    assert_eq!(BeltZone::from_distance_au(16.1), BeltZone::KuiperBelt);
    assert_eq!(BeltZone::from_distance_au(30.0), BeltZone::KuiperBelt);
    assert_eq!(BeltZone::from_distance_au(45.0), BeltZone::KuiperBelt);
    assert_eq!(BeltZone::from_distance_au(55.0), BeltZone::ScatteredDisk);

    // 2. Validate census recording
    let mut census = BeltCensus::default();
    census.record(1.2); // Inner
    census.record(2.3); // Asteroid
    census.record(2.77); // Asteroid
    census.record(5.2); // Trojan
    census.record(18.0); // Kuiper
    census.record(25.0); // Kuiper
    census.record(35.0); // Kuiper
    census.record(60.0); // Scattered

    assert_eq!(census.inner_count, 1);
    assert_eq!(census.asteroid_belt_count, 2);
    assert_eq!(census.trojan_count, 1);
    assert_eq!(census.kuiper_count, 3);
    assert_eq!(census.scattered_count, 1);
    assert_eq!(census.total(), 8);

    let summary = census.format_summary_line();
    assert!(summary.contains("☀️ In: 1"));
    assert!(summary.contains("🪨 Main: 2"));
    assert!(summary.contains("🪐 Troj: 1"));
    assert!(summary.contains("🧊 Kuiper: 3"));
    assert!(summary.contains("🌌 Oort: 1"));
}

#[test]
fn test_quick_bar_belt_expand_and_contract_interactions() {
    use bevy::prelude::*;
    use protostellar::game::ui::{handle_ui_button_interactions, BeltZone, QuickBarState};

    let (mut app, bar_ent, _cam_ent, _jupiter_ent) = setup_quick_selector_app();

    // Spawn minor bodies in Asteroid Belt and Kuiper Belt
    app.world_mut().spawn((
        CelestialBody {
            name: "Ceres (Dwarf Planet)".to_string(),
            body_type: BodyType::Asteroid,
        },
        SimPosition(DVec3::new(2.77, 0.0, 0.0)),
        SimVelocity(DVec3::ZERO),
        Mass(0.00015 * EARTH_MASS_SOLAR),
        Radius(0.0001),
    ));
    app.world_mut().spawn((
        CelestialBody {
            name: "1P/Halley (Comet)".to_string(),
            body_type: BodyType::Comet,
        },
        SimPosition(DVec3::new(17.8, 0.0, 0.0)),
        SimVelocity(DVec3::ZERO),
        Mass(0.00002 * EARTH_MASS_SOLAR),
        Radius(0.00005),
    ));

    app.add_systems(Update, handle_ui_button_interactions);
    app.update();

    // Initially show_minor_bodies is false and expanded_belts is empty
    let qb_state = app.world().resource::<QuickBarState>();
    assert!(!qb_state.show_minor_bodies);
    assert!(qb_state.expanded_belts.is_empty());

    // Toggle minor bodies on
    app.world_mut()
        .resource_mut::<QuickBarState>()
        .show_minor_bodies = true;
    app.update();

    // Verify buttons spawned in the selector bar
    let children: Vec<Entity> = app
        .world()
        .get::<Children>(bar_ent)
        .unwrap()
        .iter()
        .collect();
    assert!(
        !children.is_empty(),
        "Buttons must be spawned in the selector bar"
    );

    // Test expanding Asteroid Belt
    let mut qb_state = app.world_mut().resource_mut::<QuickBarState>();
    qb_state.expanded_belts.insert(BeltZone::AsteroidBelt);
    app.update();

    let qb_state = app.world().resource::<QuickBarState>();
    assert!(qb_state.expanded_belts.contains(&BeltZone::AsteroidBelt));
    assert!(!qb_state.expanded_belts.contains(&BeltZone::KuiperBelt));

    // Test collapsing Asteroid Belt
    let mut qb_state = app.world_mut().resource_mut::<QuickBarState>();
    qb_state.expanded_belts.remove(&BeltZone::AsteroidBelt);
    app.update();

    let qb_state = app.world().resource::<QuickBarState>();
    assert!(qb_state.expanded_belts.is_empty());
}

#[test]
fn test_inspector_composition_water_ice_formatting() {
    use protostellar::game::ui::format_composition_water_ice;

    // 1. Warm Earth-like planet with 81% ocean coverage and trace (<1%) bulk water mass
    let comp_earth = Composition::rocky();
    let vol_earth = VolatileInventory {
        ocean_coverage_frac: 0.81,
        delivered_water_m_earth: 0.0005,
        ..default()
    };
    let s1 = format_composition_water_ice(&comp_earth, Some(&vol_earth), None, 343.0, false);
    assert_eq!(s1, "<1% Water");

    // 2. Warm water world with 25% bulk water mass and 95% ocean coverage
    let comp_waterworld = Composition {
        silicate_frac: 0.50,
        ice_frac: 0.25,
        metal_frac: 0.25,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let vol_waterworld = VolatileInventory {
        ocean_coverage_frac: 0.95,
        delivered_water_m_earth: 0.05,
        ..default()
    };
    let s2 =
        format_composition_water_ice(&comp_waterworld, Some(&vol_waterworld), None, 300.0, false);
    assert_eq!(s2, "25% Water");

    // 3. Dry warm planet (Venus 737 K, no volatiles)
    let s3 = format_composition_water_ice(&comp_earth, None, None, 737.0, false);
    assert_eq!(s3, "0% Water");

    // 4. Trace water warm planet
    let vol_trace = VolatileInventory {
        ocean_coverage_frac: 0.0,
        delivered_water_m_earth: 0.00001,
        ..default()
    };
    let s4 = format_composition_water_ice(&comp_earth, Some(&vol_trace), None, 290.0, false);
    assert_eq!(s4, "<1% Water");

    // 5. Cold snowball world with 90% surface ice coverage but 0% bulk ice mass
    let climate_snowball = PlanetaryClimate {
        ice_coverage_frac: 0.90,
        surface_temperature_k: 210.0,
        ..default()
    };
    let s5 = format_composition_water_ice(&comp_earth, None, Some(&climate_snowball), 210.0, false);
    assert_eq!(s5, "0% Ice");

    // 6. Cold icy world / comet (55% bulk ice mass)
    let comp_icy = Composition::icy();
    let s6 = format_composition_water_ice(&comp_icy, None, None, 100.0, false);
    assert_eq!(s6, "55% Ice");

    // 7. Central star (Sun, 5778 K)
    let s7 = format_composition_water_ice(&Composition::pure_hydrogen(), None, None, 5778.0, true);
    assert_eq!(s7, "0% Ice");
}

#[test]
fn test_scenario_contextual_ui_filtering() {
    use protostellar::game::ui::{update_scenario_contextual_ui, InspectorExoticHeader, InspectorSection, UiButtonAction};
    use protostellar::simulation::resources::PlayerInteractionState;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    app.init_resource::<PlayerInteractionState>();
    app.insert_resource(ActiveScenarioState {
        current_preset: ScenarioPreset::SolarNebulaMmsn,
        ..default()
    });

    // Spawn test UI entities for buttons
    let btn_theia = app.world_mut().spawn((UiButtonAction::InjectEmbryo, Node::default())).id();
    let btn_lhb = app.world_mut().spawn((UiButtonAction::TriggerLhb, Node::default())).id();
    let btn_epochs = app.world_mut().spawn((UiButtonAction::ToggleEpochScrubberPanel, Node::default())).id();
    let btn_pop3 = app.world_mut().spawn((UiButtonAction::SpawnInfallPop3Star, Node::default())).id();
    let btn_inspiral = app.world_mut().spawn((UiButtonAction::AccelerateInspiral, Node::default())).id();
    let btn_hyper = app.world_mut().spawn((UiButtonAction::ToggleSuperEddington, Node::default())).id();
    let btn_blowout = app.world_mut().spawn((UiButtonAction::TriggerBlowoutCocoon, Node::default())).id();

    // Spawn section nodes and exotic header
    let sec_terra = app.world_mut().spawn((InspectorSection::TerraformingBombardment, Node::default())).id();
    let sec_exotic = app.world_mut().spawn((InspectorSection::ExoticExperiments, Node::default())).id();
    let header_exotic = app.world_mut().spawn((Text::new(""), InspectorExoticHeader)).id();

    // Spawn an Earth planet
    let earth_ent = app.world_mut().spawn((
        CelestialBody {
            name: "Earth".to_string(),
            body_type: BodyType::TerrestrialPlanet,
        },
        Mass(EARTH_MASS_SOLAR),
        SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        SimVelocity(DVec3::ZERO),
    )).id();

    // Spawn central Sun
    let sun_ent = app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
    )).id();

    app.add_systems(Update, update_scenario_contextual_ui);

    // 1. Test Solar MMSN with Earth selected:
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.selected_entity = Some(earth_ent);
    }
    app.update();

    assert_eq!(app.world().get::<Node>(btn_theia).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(btn_lhb).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(btn_epochs).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(btn_pop3).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(btn_inspiral).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(btn_hyper).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(btn_blowout).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(sec_terra).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(sec_exotic).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Text>(header_exotic).unwrap().0, "EXOTIC PHENOMENA:");

    // 2. Select Sun in Solar MMSN: Terraforming row must hide
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.selected_entity = Some(sun_ent);
    }
    app.update();
    assert_eq!(app.world().get::<Node>(sec_terra).unwrap().display, Display::None);

    // 3. Switch to Little Red Dot scenario:
    {
        let mut sc = app.world_mut().resource_mut::<ActiveScenarioState>();
        sc.current_preset = ScenarioPreset::LittleRedDot;
    }
    app.update();

    assert_eq!(app.world().get::<Node>(btn_hyper).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(btn_blowout).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(btn_pop3).unwrap().display, Display::Flex); // Universal Pop-III
    assert_eq!(app.world().get::<Node>(btn_theia).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(btn_lhb).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(btn_inspiral).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(btn_epochs).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(sec_terra).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Text>(header_exotic).unwrap().0, "EXOTIC / LITTLE RED DOT:");

    // 4. Switch to Relativistic Binary scenario:
    {
        let mut sc = app.world_mut().resource_mut::<ActiveScenarioState>();
        sc.current_preset = ScenarioPreset::RelativisticBinary;
    }
    app.update();

    assert_eq!(app.world().get::<Node>(btn_inspiral).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(btn_pop3).unwrap().display, Display::Flex); // Universal Pop-III
    assert_eq!(app.world().get::<Node>(btn_hyper).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(btn_blowout).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(btn_theia).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(btn_epochs).unwrap().display, Display::None);
}

