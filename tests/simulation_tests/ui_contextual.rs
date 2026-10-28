//! Scenario-specific contextual UI visibility and filtering tests.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::game::ui::{
    update_quick_bar_highlights, update_scenario_contextual_ui, InspectorExoticHeader,
    InspectorSection, QuickBarButtonBaseColor, UiButtonAction,
};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::PlayerInteractionState;
use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use protostellar::utils::constants::EARTH_MASS_SOLAR;

struct ContextualUiTestEntities {
    btn_theia: Entity,
    btn_lhb: Entity,
    btn_epochs: Entity,
    btn_pop3: Entity,
    btn_inspiral: Entity,
    btn_hyper: Entity,
    btn_blowout: Entity,
    sec_terra: Entity,
    sec_exotic: Entity,
    header_exotic: Entity,
    earth_ent: Entity,
    sun_ent: Entity,
}

fn setup_contextual_ui_app() -> (App, ContextualUiTestEntities) {
    let mut app = App::new();
    app.init_resource::<PlayerInteractionState>();
    app.insert_resource(ActiveScenarioState {
        current_preset: ScenarioPreset::SolarNebulaMmsn,
        ..default()
    });

    let btn_theia = app
        .world_mut()
        .spawn((UiButtonAction::InjectEmbryo, Node::default()))
        .id();
    let btn_lhb = app
        .world_mut()
        .spawn((UiButtonAction::TriggerLhb, Node::default()))
        .id();
    let btn_epochs = app
        .world_mut()
        .spawn((UiButtonAction::ToggleEpochScrubberPanel, Node::default()))
        .id();
    let btn_pop3 = app
        .world_mut()
        .spawn((UiButtonAction::SpawnInfallPop3Star, Node::default()))
        .id();
    let btn_inspiral = app
        .world_mut()
        .spawn((UiButtonAction::AccelerateInspiral, Node::default()))
        .id();
    let btn_hyper = app
        .world_mut()
        .spawn((UiButtonAction::ToggleSuperEddington, Node::default()))
        .id();
    let btn_blowout = app
        .world_mut()
        .spawn((UiButtonAction::TriggerBlowoutCocoon, Node::default()))
        .id();

    let sec_terra = app
        .world_mut()
        .spawn((InspectorSection::TerraformingBombardment, Node::default()))
        .id();
    let sec_exotic = app
        .world_mut()
        .spawn((InspectorSection::ExoticExperiments, Node::default()))
        .id();
    let header_exotic = app
        .world_mut()
        .spawn((Text::new(""), InspectorExoticHeader))
        .id();

    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
        ))
        .id();

    let sun_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Sun".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            Mass(1.0),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
        ))
        .id();

    app.add_systems(Update, update_scenario_contextual_ui);

    (
        app,
        ContextualUiTestEntities {
            btn_theia,
            btn_lhb,
            btn_epochs,
            btn_pop3,
            btn_inspiral,
            btn_hyper,
            btn_blowout,
            sec_terra,
            sec_exotic,
            header_exotic,
            earth_ent,
            sun_ent,
        },
    )
}

fn verify_solar_mmsn_contextual_ui(app: &mut App, entities: &ContextualUiTestEntities) {
    // 1. Earth selected in Solar MMSN
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.selected_entity = Some(entities.earth_ent);
    }
    app.update();

    assert_eq!(
        app.world().get::<Node>(entities.btn_theia).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Node>(entities.btn_lhb).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world()
            .get::<Node>(entities.btn_epochs)
            .unwrap()
            .display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Node>(entities.btn_pop3).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world()
            .get::<Node>(entities.btn_inspiral)
            .unwrap()
            .display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(entities.btn_hyper).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world()
            .get::<Node>(entities.btn_blowout)
            .unwrap()
            .display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(entities.sec_terra).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world()
            .get::<Node>(entities.sec_exotic)
            .unwrap()
            .display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Text>(entities.header_exotic).unwrap().0,
        "EXOTIC PHENOMENA:"
    );

    // 2. Sun selected in Solar MMSN: Terraforming row must hide
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.selected_entity = Some(entities.sun_ent);
    }
    app.update();
    assert_eq!(
        app.world().get::<Node>(entities.sec_terra).unwrap().display,
        Display::None
    );
}

fn verify_scenario_preset_ui(
    app: &mut App,
    entities: &ContextualUiTestEntities,
    preset: ScenarioPreset,
    visible_buttons: &[Entity],
    hidden_buttons: &[Entity],
    expected_header: Option<&str>,
) {
    {
        let mut sc = app.world_mut().resource_mut::<ActiveScenarioState>();
        sc.current_preset = preset;
    }
    app.update();

    for &btn in visible_buttons {
        assert_eq!(app.world().get::<Node>(btn).unwrap().display, Display::Flex);
    }
    for &btn in hidden_buttons {
        assert_eq!(app.world().get::<Node>(btn).unwrap().display, Display::None);
    }
    assert_eq!(
        app.world().get::<Node>(entities.sec_terra).unwrap().display,
        Display::None
    );
    if let Some(header_text) = expected_header {
        assert_eq!(
            app.world().get::<Text>(entities.header_exotic).unwrap().0,
            header_text
        );
    }
}

#[test]
fn test_scenario_contextual_ui_filtering() {
    let (mut app, entities) = setup_contextual_ui_app();
    verify_solar_mmsn_contextual_ui(&mut app, &entities);

    verify_scenario_preset_ui(
        &mut app,
        &entities,
        ScenarioPreset::LittleRedDot,
        &[entities.btn_hyper, entities.btn_blowout, entities.btn_pop3],
        &[
            entities.btn_theia,
            entities.btn_lhb,
            entities.btn_inspiral,
            entities.btn_epochs,
        ],
        Some("EXOTIC / LITTLE RED DOT:"),
    );

    verify_scenario_preset_ui(
        &mut app,
        &entities,
        ScenarioPreset::RelativisticBinary,
        &[entities.btn_inspiral, entities.btn_pop3],
        &[
            entities.btn_hyper,
            entities.btn_blowout,
            entities.btn_theia,
            entities.btn_epochs,
        ],
        None,
    );
}

#[test]
fn test_quick_bar_highlights_and_ui_selection_flag() {
    let mut app = App::new();
    app.init_resource::<PlayerInteractionState>();

    let sun_ent = app.world_mut().spawn_empty().id();
    let earth_ent = app.world_mut().spawn_empty().id();
    let mars_ent = app.world_mut().spawn_empty().id();

    let sun_base = QuickBarButtonBaseColor {
        bg: Color::srgba(0.25, 0.20, 0.05, 0.95),
        border: Color::srgb(1.0, 0.85, 0.2),
    };
    let earth_base = QuickBarButtonBaseColor {
        bg: Color::srgba(0.08, 0.18, 0.28, 0.95),
        border: Color::srgb(0.2, 0.7, 1.0),
    };
    let mars_base = QuickBarButtonBaseColor {
        bg: Color::srgba(0.25, 0.10, 0.08, 0.95),
        border: Color::srgb(1.0, 0.45, 0.3),
    };

    let btn_sun = app
        .world_mut()
        .spawn((
            Button,
            UiButtonAction::SelectEntity(sun_ent),
            sun_base,
            BackgroundColor(sun_base.bg),
            BorderColor::all(sun_base.border),
        ))
        .id();

    let btn_earth = app
        .world_mut()
        .spawn((
            Button,
            UiButtonAction::SelectEntity(earth_ent),
            earth_base,
            BackgroundColor(earth_base.bg),
            BorderColor::all(earth_base.border),
        ))
        .id();

    let btn_mars = app
        .world_mut()
        .spawn((
            Button,
            UiButtonAction::SelectEntity(mars_ent),
            mars_base,
            BackgroundColor(mars_base.bg),
            BorderColor::all(mars_base.border),
        ))
        .id();

    app.add_systems(Update, update_quick_bar_highlights);

    // Initial state: No entity selected
    app.update();
    assert_eq!(
        app.world().get::<BackgroundColor>(btn_sun).unwrap().0,
        sun_base.bg
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(btn_earth).unwrap().0,
        earth_base.bg
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(btn_mars).unwrap().0,
        mars_base.bg
    );

    // Step 1: Select Earth via UI action
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.selected_entity = Some(earth_ent);
        player.just_selected_via_ui = true;
    }
    app.update();

    let sel_bg = Color::srgba(0.20, 0.45, 0.85, 0.95);
    let sel_border = Color::srgb(1.0, 1.0, 1.0);

    // Earth button should be highlighted
    assert_eq!(
        app.world().get::<BackgroundColor>(btn_earth).unwrap().0,
        sel_bg
    );
    assert_eq!(
        app.world().get::<BorderColor>(btn_earth).unwrap().top,
        sel_border
    );

    // Sun and Mars should maintain their base colors
    assert_eq!(
        app.world().get::<BackgroundColor>(btn_sun).unwrap().0,
        sun_base.bg
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(btn_mars).unwrap().0,
        mars_base.bg
    );

    // Verify just_selected_via_ui flag is true
    assert!(
        app.world()
            .resource::<PlayerInteractionState>()
            .just_selected_via_ui
    );

    // Step 2: Switch selection to Mars
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.selected_entity = Some(mars_ent);
    }
    app.update();

    // Mars is highlighted, Earth reverted to base colors
    assert_eq!(
        app.world().get::<BackgroundColor>(btn_mars).unwrap().0,
        sel_bg
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(btn_earth).unwrap().0,
        earth_base.bg
    );
}

#[test]
fn test_hover_card_classification_labels() {
    use protostellar::game::ui::hover_card::resolve_body_type_label;

    assert_eq!(
        resolve_body_type_label(BodyType::TerrestrialPlanet),
        "Terrestrial Planet"
    );
    assert_eq!(resolve_body_type_label(BodyType::GasGiant), "Gas Giant");
    assert_eq!(
        resolve_body_type_label(BodyType::YellowDwarf),
        "G-Type Yellow Dwarf"
    );
    assert_eq!(
        resolve_body_type_label(BodyType::BlackHole),
        "Kerr Black Hole"
    );
    assert_eq!(resolve_body_type_label(BodyType::Moon), "Natural Moon");
    assert_eq!(resolve_body_type_label(BodyType::Comet), "Active Comet");
}

#[test]
fn test_hover_card_metrics_and_badges_formatting() {
    use protostellar::game::ui::hover_card::{collect_hover_badges, format_hover_metrics};
    use protostellar::simulation::tides::TidalState;

    let pos = DVec3::new(1.0, 0.0, 0.0);
    let vel = DVec3::new(0.0, std::f64::consts::TAU, 0.0);
    let mass = 1.0 * protostellar::utils::constants::EARTH_MASS_SOLAR;
    let rad = protostellar::utils::constants::EARTH_RADIUS_AU;
    let temp = 288.0;

    let metrics = format_hover_metrics(pos, vel, mass, rad, temp);
    assert!(metrics.contains("Distance: 1.00 AU"));
    assert!(metrics.contains("Mass: 1.00 M_earth"));
    assert!(metrics.contains("Temp: 288 K"));

    let climate = PlanetaryClimate {
        climate_regime: ClimateRegime::TemperateHabitable,
        ..default()
    };
    let bio = BiosphereState {
        biomass_coverage_frac: 0.35,
        technosignature: 0.8,
        ..default()
    };
    let diff = InternalDifferentiation {
        magnetic_field_gauss: 1.0,
        ..default()
    };
    let tide = TidalState {
        is_tidally_locked: true,
        ..default()
    };

    let badges = collect_hover_badges(
        Some(&climate),
        Some(&bio),
        None,
        Some(&diff),
        Some(&tide),
        None,
        None,
        false,
    );

    assert!(badges.contains("🟢 Habitable"));
    assert!(badges.contains("🌿 Biosphere"));
    assert!(badges.contains("🏙️ Civilization"));
    assert!(badges.contains("🧲 Magnetosphere"));
    assert!(badges.contains("🔒 Tidally Locked"));
}

#[test]
fn test_hover_card_screen_clamping() {
    use protostellar::game::ui::hover_card::compute_clamped_card_position;

    let window_size = Vec2::new(1920.0, 1080.0);
    let card_size = Vec2::new(260.0, 130.0);

    // Mid-screen: offsets +24 x, -30 y
    let mid = Vec2::new(500.0, 500.0);
    let clamped_mid = compute_clamped_card_position(mid, card_size, window_size);
    assert_eq!(clamped_mid, Vec2::new(524.0, 470.0));

    // Near right edge: flips to left of cursor
    let right_edge = Vec2::new(1800.0, 500.0);
    let clamped_right = compute_clamped_card_position(right_edge, card_size, window_size);
    assert!(clamped_right.x < right_edge.x);

    // Near bottom edge: flips upward above bottom bar
    let bottom_edge = Vec2::new(500.0, 1020.0);
    let clamped_bottom = compute_clamped_card_position(bottom_edge, card_size, window_size);
    assert!(clamped_bottom.y < bottom_edge.y);
}

#[test]
fn test_hover_card_ui_system_visibility_and_text_sync() {
    use protostellar::game::ui::hover_card::{update_hover_card_ui, HoverCardField, HoverCardRoot};
    use protostellar::rendering::camera::PanOrbitCamera;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<PlayerInteractionState>();

    // Spawn Window & 3D Camera
    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });
    app.world_mut().spawn((
        Camera::default(),
        PanOrbitCamera::default(),
        GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
    ));

    // Spawn earth
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.00 * protostellar::utils::constants::EARTH_MASS_SOLAR),
            Radius(protostellar::utils::constants::EARTH_RADIUS_AU),
            SimPosition(DVec3::new(0.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, std::f64::consts::TAU, 0.0)),
            Temperature(288.0),
            Composition::default(),
        ))
        .id();

    // Spawn hover card hierarchy
    let card_root = app
        .world_mut()
        .spawn((
            HoverCardRoot,
            Node {
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|card| {
            card.spawn((HoverCardField::Title, Text::new("")));
            card.spawn((HoverCardField::Subhead, Text::new("")));
            card.spawn((HoverCardField::Metrics, Text::new("")));
            card.spawn((HoverCardField::Badges, Text::new("")));
        })
        .id();

    app.add_systems(Update, update_hover_card_ui);

    // 1. Initial frame: hovered_entity is None -> Display::None
    app.update();
    assert_eq!(
        app.world().get::<Node>(card_root).unwrap().display,
        Display::None
    );

    // 2. Set hovered_entity to Earth
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.hovered_entity = Some(earth_ent);
    }
    app.update();
    assert_eq!(
        app.world().get::<Node>(card_root).unwrap().display,
        Display::Flex
    );

    // Verify Title text was updated to "Earth"
    let mut title_found = false;
    for (field, text) in app
        .world_mut()
        .query::<(&HoverCardField, &Text)>()
        .iter(app.world())
    {
        if *field == HoverCardField::Title {
            assert_eq!(text.0, "Earth");
            title_found = true;
        }
    }
    assert!(title_found);

    // 3. Clear hovered_entity -> Display::None
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.hovered_entity = None;
    }
    app.update();
    assert_eq!(
        app.world().get::<Node>(card_root).unwrap().display,
        Display::None
    );
}

#[test]
fn test_internal_cutaway_strata_archetype_resolution() {
    use protostellar::rendering::effects::planetary::resolve_cutaway_strata;
    use protostellar::simulation::resources::SimulationConfig;

    let config = SimulationConfig::default();

    // 1. Terrestrial World with Mantle Plumes / LLSVP
    let earth_body = CelestialBody {
        name: "Earth".to_string(),
        body_type: BodyType::TerrestrialPlanet,
    };
    let earth_diff = InternalDifferentiation {
        is_differentiated: true,
        differentiation_fraction: 1.0,
        core_radius_au: 2.3e-5,
        mantle_radius_au: 4.2e-5,
        crust_thickness_au: 2.3e-7,
        ocean_ice_thickness_au: 2.7e-8,
        core_temp_k: 5500.0,
        magnetic_field_gauss: 0.5,
        has_theia_llsvp: true,
        llsvp_density_contrast: 0.025,
    };
    let strata_terr = resolve_cutaway_strata(&earth_body, 4.26e-5, Some(&earth_diff), &config);
    assert!(strata_terr.has_theia_llsvp);
    assert!(strata_terr.inner_core_r < strata_terr.core_r);
    assert!(strata_terr.core_r < strata_terr.mantle_r);
    assert!(strata_terr.mantle_r <= strata_terr.crust_r);

    // 2. Gas Giant (Jupiter)
    let jupiter_body = CelestialBody {
        name: "Jupiter".to_string(),
        body_type: BodyType::GasGiant,
    };
    let strata_gas = resolve_cutaway_strata(&jupiter_body, 4.77e-4, None, &config);
    assert!(!strata_gas.has_theia_llsvp);
    assert!(strata_gas.inner_core_r < strata_gas.core_r);
    assert!(strata_gas.core_r < strata_gas.mantle_r);
    assert!(strata_gas.mantle_r < strata_gas.crust_r);

    // 3. Ice Giant (Neptune)
    let neptune_body = CelestialBody {
        name: "Neptune".to_string(),
        body_type: BodyType::IceGiant,
    };
    let strata_ice = resolve_cutaway_strata(&neptune_body, 1.64e-4, None, &config);
    assert!(!strata_ice.has_theia_llsvp);
    assert!(strata_ice.inner_core_r < strata_ice.core_r);
    assert!(strata_ice.core_r < strata_ice.mantle_r);

    // 4. Central Star (Sun)
    let sun_body = CelestialBody {
        name: "Sun".to_string(),
        body_type: BodyType::MainSequenceStar,
    };
    let strata_star = resolve_cutaway_strata(&sun_body, 4.65e-3, None, &config);
    assert!(!strata_star.has_theia_llsvp);
    assert!(strata_star.inner_core_r < strata_star.core_r);
    assert!(strata_star.core_r < strata_star.mantle_r);
    assert_eq!(
        strata_star.crust_r,
        config.calc_visual_radius_for_type(4.65e-3, BodyType::MainSequenceStar)
    );
}

#[test]
fn test_player_interaction_cutaway_active_state_and_cycle() {
    use protostellar::simulation::resources::DiagnosticOverlayMode;

    let mut state = PlayerInteractionState::default();
    assert!(!state.is_cutaway_active());

    // Cutaway mode hotkey toggle
    state.cutaway_mode = true;
    assert!(state.is_cutaway_active());
    state.cutaway_mode = false;
    assert!(!state.is_cutaway_active());

    // Diagnostic overlay cycle
    state.overlay_mode = DiagnosticOverlayMode::InternalCutaway;
    assert!(state.is_cutaway_active());
    assert_eq!(
        state.overlay_mode.display_name(),
        "Internal Cutaway & Core Geodynamics"
    );

    // Cycle through and verify it returns
    let mut found_cutaway = false;
    let mut current = DiagnosticOverlayMode::Hidden;
    for _ in 0..15 {
        current = current.cycle();
        if current == DiagnosticOverlayMode::InternalCutaway {
            found_cutaway = true;
            break;
        }
    }
    assert!(
        found_cutaway,
        "DiagnosticOverlayMode cycle must include InternalCutaway"
    );
}

#[test]
fn test_cutaway_ui_button_universal_visibility() {
    let (mut app, _entities) = setup_contextual_ui_app();

    let btn_cutaway = app
        .world_mut()
        .spawn((UiButtonAction::ToggleInternalCutaway, Node::default()))
        .id();

    app.add_systems(Update, update_scenario_contextual_ui);

    // Initial frame
    app.update();
    assert_eq!(
        app.world().get::<Node>(btn_cutaway).unwrap().display,
        Display::Flex
    );

    // Switch scenario preset
    {
        let mut scenario = app.world_mut().resource_mut::<ActiveScenarioState>();
        scenario.current_preset = ScenarioPreset::LittleRedDot;
    }
    app.update();
    assert_eq!(
        app.world().get::<Node>(btn_cutaway).unwrap().display,
        Display::Flex
    );
}

#[test]
fn test_cutaway_hud_pill_spawning_and_strata_labels() {
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::simulation::components::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<protostellar::simulation::resources::SimulationConfig>();

    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        resolution: (1280u32, 720u32).into(),
        ..default()
    });

    let _camera_ent = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            PanOrbitCamera {
                radius: 16.0,
                target_radius: 16.0,
                ..default()
            },
            Transform::from_xyz(0.0, 5.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
            GlobalTransform::from(
                Transform::from_xyz(0.0, 5.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
            ),
        ))
        .id();

    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.0),
            Radius(0.005),
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
        ))
        .id();

    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn(Node::default()).with_children(|root| {
            protostellar::game::ui::cutaway_hud::spawn_cutaway_hud(root);
        });
    });

    app.add_systems(
        Update,
        protostellar::game::ui::cutaway_hud::update_cutaway_hud_ui,
    );

    app.update();

    // Verify 4 pills were spawned
    let mut pill_count = 0;
    let mut query = app
        .world_mut()
        .query::<&protostellar::game::ui::cutaway_hud::CutawayCalloutPill>();
    for _ in query.iter(app.world()) {
        pill_count += 1;
    }
    assert_eq!(
        pill_count, 4,
        "Must spawn exactly 4 strata callout pills (Crust, Mantle, Outer Core, Inner Core)"
    );

    // Now activate cutaway mode and select Earth
    {
        let mut ps = app.world_mut().resource_mut::<PlayerInteractionState>();
        ps.cutaway_mode = true;
        ps.selected_entity = Some(earth_ent);
    }

    app.update();

    let mut pill_entities = Vec::new();
    let mut pill_query = app.world_mut().query::<(
        Entity,
        &protostellar::game::ui::cutaway_hud::CutawayCalloutPill,
        &Node,
    )>();
    for (ent, p, node) in pill_query.iter(app.world()) {
        assert_eq!(
            node.display,
            Display::Flex,
            "Pill {} must be visible (Display::Flex) when cutaway mode is active on selected body",
            p.0
        );
        pill_entities.push((p.0, ent));
    }
    assert_eq!(pill_entities.len(), 4);

    // Verify that the title text inside each pill matches Earth's geological strata
    let mut text_query = app.world_mut().query::<(
        &ChildOf,
        &protostellar::game::ui::cutaway_hud::CutawayField,
        &Text,
    )>();
    for (p_idx, p_ent) in &pill_entities {
        for (child_of, field, text) in text_query.iter(app.world()) {
            if child_of.parent() == *p_ent
                && *field == protostellar::game::ui::cutaway_hud::CutawayField::Title
            {
                match p_idx {
                    0 => assert!(text.0.contains("CRUST")),
                    1 => assert!(text.0.contains("MANTLE")),
                    2 => assert!(text.0.contains("OUTER CORE")),
                    3 => assert!(text.0.contains("INNER CORE")),
                    _ => {}
                }
            }
        }
    }

    // When cutaway mode is toggled off, all pills must hide
    {
        let mut ps = app.world_mut().resource_mut::<PlayerInteractionState>();
        ps.cutaway_mode = false;
    }
    app.update();

    let mut pill_query = app.world_mut().query::<(
        &protostellar::game::ui::cutaway_hud::CutawayCalloutPill,
        &Node,
    )>();
    for (p, node) in pill_query.iter(app.world()) {
        assert_eq!(
            node.display,
            Display::None,
            "Pill {} must hide (Display::None) when cutaway is inactive",
            p.0
        );
    }
}
