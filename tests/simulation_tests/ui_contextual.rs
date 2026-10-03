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
