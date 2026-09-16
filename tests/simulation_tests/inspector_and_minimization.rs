//! Tests for Target Inspector scrolling and universal HUD panel minimization.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use protostellar::game::ui::scroll::{
    handle_inspector_scroll, reset_inspector_scroll_on_target_change,
};
use protostellar::game::ui::types::{
    HudPanelElement, HudVisibilityState, PlanetBuilderState, ScrollableInspector,
    TelemetryPanelState,
};
use protostellar::game::ui::visibility::update_hud_visibility;
use protostellar::simulation::resources::PlayerInteractionState;

#[test]
fn test_inspector_scrolling_clamping_and_reset() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    app.add_systems(
        Update,
        (
            handle_inspector_scroll,
            reset_inspector_scroll_on_target_change,
        ),
    );

    let scroll_ent = app
        .world_mut()
        .spawn((
            ScrollableInspector,
            ScrollPosition::default(),
            Node::default(),
            ComputedNode {
                size: Vec2::new(380.0, 300.0),
                content_size: Vec2::new(380.0, 750.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            Interaction::default(),
        ))
        .id();

    // Spawn a child button inside scroll_ent that is currently hovered
    let _child_btn = app
        .world_mut()
        .spawn((Node::default(), Interaction::Hovered, ChildOf(scroll_ent)))
        .id();

    let dummy_target1 = app.world_mut().spawn_empty().id();
    let dummy_target2 = app.world_mut().spawn_empty().id();

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(dummy_target1);

    app.update();

    // Scroll down with mouse wheel while hovering child
    {
        let mut wheel = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel.write(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: -2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }

    app.update();

    let scroll_pos = app.world().get::<ScrollPosition>(scroll_ent).unwrap();
    assert!(
        scroll_pos.y > 0.0,
        "ScrollPosition.y should increase when wheeling down while hovering child button, got {}",
        scroll_pos.y
    );
    assert!(
        scroll_pos.y <= 450.0,
        "ScrollPosition.y must be clamped to max_offset_y (450.0), got {}",
        scroll_pos.y
    );

    // Switch selection to another body -> scroll should automatically reset to 0
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(dummy_target2);

    app.update();

    let reset_pos = app.world().get::<ScrollPosition>(scroll_ent).unwrap();
    assert!(
        reset_pos.y.abs() < f32::EPSILON,
        "ScrollPosition.y must reset to top (0.0) when selecting a new celestial target"
    );
}

struct TestHudEntities {
    top_controls_panel: Entity,
    top_controls_pill: Entity,
    bottom_right_panel: Entity,
    bottom_right_pill: Entity,
    bottom_center_panel: Entity,
    bottom_center_pill: Entity,
    scenarios_panel: Entity,
    scenarios_pill: Entity,
    builder_pill: Entity,
    telemetry_pill: Entity,
}

fn setup_test_hud_app() -> (App, TestHudEntities) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<HudVisibilityState>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<TelemetryPanelState>();

    app.add_systems(Update, update_hud_visibility);

    let entities = TestHudEntities {
        top_controls_panel: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::TopControlsPanel))
            .id(),
        top_controls_pill: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::TopControlsPill))
            .id(),
        bottom_right_panel: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::BottomRightPanel))
            .id(),
        bottom_right_pill: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::BottomRightPill))
            .id(),
        bottom_center_panel: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::BottomCenterPanel))
            .id(),
        bottom_center_pill: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::BottomCenterPill))
            .id(),
        scenarios_panel: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::ScenarioPresets))
            .id(),
        scenarios_pill: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::ScenarioPresetsPill))
            .id(),
        builder_pill: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::PlanetBuilderPill))
            .id(),
        telemetry_pill: app
            .world_mut()
            .spawn((Node::default(), HudPanelElement::TelemetryPanelPill))
            .id(),
    };

    (app, entities)
}

#[test]
fn test_hud_panel_initial_default_visibility() {
    let (mut app, e) = setup_test_hud_app();

    // Initial default state: panels visible, pills hidden
    app.update();

    assert_eq!(app.world().get::<Node>(e.top_controls_panel).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.top_controls_pill).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.bottom_right_panel).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.bottom_right_pill).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.bottom_center_panel).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.bottom_center_pill).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.scenarios_panel).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.scenarios_pill).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.builder_pill).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.telemetry_pill).unwrap().display, Display::None);
}

#[test]
fn test_hud_panel_minimized_visibility() {
    let (mut app, e) = setup_test_hud_app();

    // Minimize all panels
    {
        let mut hud = app.world_mut().resource_mut::<HudVisibilityState>();
        hud.top_controls_minimized = true;
        hud.bottom_right_minimized = true;
        hud.bottom_center_minimized = true;
        hud.scenarios_minimized = true;
        hud.telemetry_minimized = true;

        let mut builder = app.world_mut().resource_mut::<PlanetBuilderState>();
        builder.is_open = true;
        builder.is_minimized = true;

        let mut telemetry = app.world_mut().resource_mut::<TelemetryPanelState>();
        telemetry.is_open = true;
    }

    app.update();

    // Panels should now be hidden, pills displayed
    assert_eq!(app.world().get::<Node>(e.top_controls_panel).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.top_controls_pill).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.bottom_right_panel).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.bottom_right_pill).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.bottom_center_panel).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.bottom_center_pill).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.scenarios_panel).unwrap().display, Display::None);
    assert_eq!(app.world().get::<Node>(e.scenarios_pill).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.builder_pill).unwrap().display, Display::Flex);
    assert_eq!(app.world().get::<Node>(e.telemetry_pill).unwrap().display, Display::Flex);
}
