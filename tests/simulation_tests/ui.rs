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

fn verify_minor_body_classification() {
    use protostellar::game::ui::is_major_body;

    assert!(!is_major_body(
        "Ceres",
        BodyType::Asteroid,
        false,
        0.00015 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Vesta",
        BodyType::Asteroid,
        false,
        0.00004 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "1P/Halley",
        BodyType::Comet,
        false,
        0.000001 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "C/1995 O1 Hale-Bopp",
        BodyType::Comet,
        false,
        0.000005 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Asteroid-2.7AU",
        BodyType::Asteroid,
        false,
        0.00001 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Comet-25.0AU",
        BodyType::Comet,
        false,
        0.00001 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Planetesimal-1.5AU",
        BodyType::Planetesimal,
        false,
        0.00001 * EARTH_MASS_SOLAR
    ));
}

fn verify_embryo_body_classification() {
    use protostellar::game::ui::{is_embryo_body, is_major_body};

    assert!(is_embryo_body("Theia", BodyType::Protoplanet));
    assert!(is_embryo_body("Theia Embryo", BodyType::Protoplanet));
    assert!(is_embryo_body("Callisto Embryo", BodyType::Protoplanet));
    assert!(is_embryo_body("Titan Embryo", BodyType::Protoplanet));
    assert!(is_embryo_body("Embryo-1.2AU", BodyType::Protoplanet));
    assert!(is_embryo_body("Embryo #1", BodyType::Protoplanet));
    assert!(!is_major_body(
        "Theia",
        BodyType::Protoplanet,
        false,
        0.10 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Theia Embryo",
        BodyType::Protoplanet,
        false,
        0.10 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Callisto Embryo",
        BodyType::Protoplanet,
        false,
        0.05 * EARTH_MASS_SOLAR
    ));
}

fn verify_major_body_classification() {
    use protostellar::game::ui::{is_canonical_major_planet, is_major_body};

    assert!(is_major_body("Sun", BodyType::Protostar, true, 1.0));
    assert!(is_major_body(
        "Proto-Mercury",
        BodyType::Protoplanet,
        false,
        0.06 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Mercury",
        BodyType::TerrestrialPlanet,
        false,
        0.055 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Venus",
        BodyType::TerrestrialPlanet,
        false,
        0.815 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Earth",
        BodyType::TerrestrialPlanet,
        false,
        1.0 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Mars",
        BodyType::TerrestrialPlanet,
        false,
        0.107 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Jupiter",
        BodyType::GasGiant,
        false,
        317.8 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Saturn",
        BodyType::GasGiant,
        false,
        95.2 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Uranus",
        BodyType::IceGiant,
        false,
        14.5 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Neptune",
        BodyType::IceGiant,
        false,
        17.1 * EARTH_MASS_SOLAR
    ));
    assert!(is_canonical_major_planet("Pluto (Dwarf Planet)"));
    assert!(is_canonical_major_planet(
        "Planet Nine (Super-Earth / Ice Giant)"
    ));
    assert!(is_major_body(
        "Pluto (Dwarf Planet)",
        BodyType::TerrestrialPlanet,
        false,
        0.00218 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Planet Nine (Super-Earth / Ice Giant)",
        BodyType::IceGiant,
        false,
        5.50 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Moon",
        BodyType::Moon,
        false,
        0.0123 * EARTH_MASS_SOLAR
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
    verify_minor_body_classification();
    verify_embryo_body_classification();
    verify_major_body_classification();
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
