//! Modal mutual exclusion, escape key priority stack, energy monitor rebaselining, and inspector tab tests.

use bevy::ecs::system::RunSystemOnce;
use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::game::interaction::handle_viewport_and_tool_hotkeys;
use protostellar::game::ui::campaign_panel::CampaignPanelState;
use protostellar::game::ui::hazards_panel::HazardsPanelState;
use protostellar::game::ui::types::*;
use protostellar::game::ui::visibility::update_scenario_contextual_ui;
use protostellar::game::ui::{
    close_conflicting_modals, ActiveModalKind, ModalPanelStates, NotificationToast,
    PlanetBuilderState, TelemetryPanelState,
};
use protostellar::rendering::camera::PanOrbitCamera;
use protostellar::simulation::components::*;
use protostellar::simulation::geology::types::TimelineScrubber;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::ActiveScenarioState;
use protostellar::utils::constants::EARTH_MASS_SOLAR;

#[test]
fn test_modal_mutual_exclusion_closes_conflicting_drawers() {
    let mut builder = PlanetBuilderState {
        is_open: true,
        ..default()
    };
    let mut telemetry = TelemetryPanelState {
        is_open: false,
        ..default()
    };
    let mut trophies = TrophiesPanelState { is_open: false };
    let mut tech_tree = TechTreePanelState { is_open: false };
    let mut campaign = CampaignPanelState {
        is_open: false,
        ..default()
    };
    let mut hazards = HazardsPanelState {
        is_open: false,
        ..default()
    };
    let mut scrubber = TimelineScrubber {
        is_open: false,
        ..default()
    };
    let mut hr_diagram = HrDiagramPanelState {
        is_open: false,
        is_minimized: false,
    };
    let mut visibility = HudVisibilityState {
        scenarios_is_open: false,
        ..default()
    };

    // Initially builder is open
    assert!(builder.is_open);

    // Opening telemetry must close builder
    telemetry.is_open = true;
    let mut states = ModalPanelStates {
        builder: Some(&mut builder),
        telemetry: Some(&mut telemetry),
        trophies: Some(&mut trophies),
        tech_tree: Some(&mut tech_tree),
        campaign: Some(&mut campaign),
        hazards: Some(&mut hazards),
        scrubber: Some(&mut scrubber),
        hr_diagram: Some(&mut hr_diagram),
        visibility: Some(&mut visibility),
    };
    let closed = close_conflicting_modals(ActiveModalKind::Telemetry, &mut states);
    assert!(closed);
    assert!(
        !builder.is_open,
        "Builder must be closed when Telemetry opens"
    );
    assert!(telemetry.is_open, "Telemetry must remain open");

    // Opening Hazards must close telemetry
    hazards.is_open = true;
    let mut states = ModalPanelStates {
        builder: Some(&mut builder),
        telemetry: Some(&mut telemetry),
        trophies: Some(&mut trophies),
        tech_tree: Some(&mut tech_tree),
        campaign: Some(&mut campaign),
        hazards: Some(&mut hazards),
        scrubber: Some(&mut scrubber),
        hr_diagram: Some(&mut hr_diagram),
        visibility: Some(&mut visibility),
    };
    let closed = states.close_conflicting_modals(ActiveModalKind::Hazards);
    assert!(closed);
    assert!(
        !telemetry.is_open,
        "Telemetry must be closed when Hazards opens"
    );
    assert!(hazards.is_open, "Hazards must remain open");

    // Opening Scenarios modal must close Hazards
    visibility.scenarios_is_open = true;
    let mut states = ModalPanelStates {
        builder: Some(&mut builder),
        telemetry: Some(&mut telemetry),
        trophies: Some(&mut trophies),
        tech_tree: Some(&mut tech_tree),
        campaign: Some(&mut campaign),
        hazards: Some(&mut hazards),
        scrubber: Some(&mut scrubber),
        hr_diagram: Some(&mut hr_diagram),
        visibility: Some(&mut visibility),
    };
    let closed = states.close_conflicting_modals(ActiveModalKind::ScenarioPresets);
    assert!(closed);
    assert!(
        !hazards.is_open,
        "Hazards must be closed when ScenarioPresets opens"
    );
    assert!(visibility.scenarios_is_open);

    // Calling with ActiveModalKind::None closes all open modals
    {
        let mut states = ModalPanelStates {
            builder: Some(&mut builder),
            telemetry: Some(&mut telemetry),
            trophies: Some(&mut trophies),
            tech_tree: Some(&mut tech_tree),
            campaign: Some(&mut campaign),
            hazards: Some(&mut hazards),
            scrubber: Some(&mut scrubber),
            hr_diagram: Some(&mut hr_diagram),
            visibility: Some(&mut visibility),
        };
        let closed = states.close_conflicting_modals(ActiveModalKind::None);
        assert!(closed);
        assert!(!states.has_any_open_modal());
    }
    assert!(!visibility.scenarios_is_open);
}

#[test]
fn test_escape_key_priority_stack_surface_modal_deselect() {
    fn hotkey_runner_system(
        keyboard: Res<ButtonInput<KeyCode>>,
        mut config: ResMut<SimulationConfig>,
        mut player_state: ResMut<PlayerInteractionState>,
        mut builder_state: ResMut<PlanetBuilderState>,
        mut telemetry_state: ResMut<TelemetryPanelState>,
        mut toast: ResMut<NotificationToast>,
        mut cam_query: Query<(&Transform, &mut PanOrbitCamera)>,
    ) {
        handle_viewport_and_tool_hotkeys(
            &keyboard,
            &mut config,
            &mut player_state,
            &mut builder_state,
            &mut telemetry_state,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &mut toast,
            &mut cam_query,
            None,
            false,
            None,
            None,
        );
    }

    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<TelemetryPanelState>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<ButtonInput<KeyCode>>();

    let dummy_target = app.world_mut().spawn_empty().id();
    let cam_entity = app
        .world_mut()
        .spawn((
            Transform::default(),
            PanOrbitCamera {
                surface_horizon_mode: true,
                target_entity: Some(dummy_target),
                ..default()
            },
        ))
        .id();

    // Set player selected entity
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(dummy_target);

    // Open a modal as well
    app.world_mut().resource_mut::<PlanetBuilderState>().is_open = true;

    // Simulate pressing Escape
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);

    // Priority 1 Check:
    // Run hotkey runner: surface_horizon_mode must exit to false, but target & modal remain intact
    app.world_mut()
        .run_system_once(hotkey_runner_system)
        .unwrap();

    let cam = app.world().get::<PanOrbitCamera>(cam_entity).unwrap();
    assert!(
        !cam.surface_horizon_mode,
        "Escape Priority 1 must exit Surface Horizon Mode"
    );
    let ps = app.world().resource::<PlayerInteractionState>();
    assert_eq!(
        ps.selected_entity,
        Some(dummy_target),
        "Selected entity must not be wiped by Priority 1"
    );
    let b = app.world().resource::<PlanetBuilderState>();
    assert!(
        b.is_open,
        "Builder modal must remain open during Priority 1 exit"
    );

    // Priority 2 Check: Press Escape again
    // Now surface_horizon_mode is false, but builder modal is open
    app.world_mut()
        .run_system_once(hotkey_runner_system)
        .unwrap();

    let b = app.world().resource::<PlanetBuilderState>();
    assert!(
        !b.is_open,
        "Escape Priority 2 must close the open drawer/modal"
    );
    let ps = app.world().resource::<PlayerInteractionState>();
    assert_eq!(
        ps.selected_entity,
        Some(dummy_target),
        "Selected entity must not be wiped by Priority 2"
    );

    // Priority 3 Check: Press Escape again
    // Now no surface mode and no open modals -> deselect celestial body
    app.world_mut()
        .run_system_once(hotkey_runner_system)
        .unwrap();

    let ps = app.world().resource::<PlayerInteractionState>();
    assert!(
        ps.selected_entity.is_none(),
        "Escape Priority 3 must deselect celestial body"
    );
    let cam = app.world().get::<PanOrbitCamera>(cam_entity).unwrap();
    assert!(
        cam.target_entity.is_none(),
        "Camera target entity must be cleared on Priority 3"
    );
}

#[test]
fn test_energy_monitor_rebaselining_on_events_and_high_drift() {
    let mut monitor = EnergyMonitor::default();

    // 1. Initial rebaseline
    monitor.rebaseline(-100.0, 5, 10.0);
    assert_eq!(monitor.initial_total_energy, -100.0);
    assert_eq!(monitor.last_body_count, 5);
    assert_eq!(monitor.last_total_mass, 10.0);
    assert_eq!(monitor.relative_energy_drift, 0.0);
    assert!(monitor.initialized);

    // 2. Request rebaseline flag
    monitor.request_rebaseline();
    assert!(monitor.needs_rebaseline);

    // 3. Simulating non-conservative events with rebaseline
    monitor.rebaseline(-150.0, 6, 12.0);
    assert_eq!(monitor.initial_total_energy, -150.0);
    assert_eq!(monitor.last_body_count, 6);
    assert_eq!(monitor.last_total_mass, 12.0);
    assert_eq!(monitor.relative_energy_drift, 0.0);
    assert!(!monitor.needs_rebaseline);
}

#[test]
fn test_inspector_tabs_filter_sections_and_switch() {
    let mut app = App::new();
    app.init_resource::<ActiveScenarioState>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<HudVisibilityState>();

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

    let sec_orbit = app
        .world_mut()
        .spawn((InspectorSection::OrbitTracking, Node::default()))
        .id();
    let sec_mass = app
        .world_mut()
        .spawn((InspectorSection::MassComposition, Node::default()))
        .id();
    let sec_terra = app
        .world_mut()
        .spawn((InspectorSection::TerraformingBombardment, Node::default()))
        .id();
    let sec_astro = app
        .world_mut()
        .spawn((InspectorSection::Astrophysics, Node::default()))
        .id();
    let sec_exotic = app
        .world_mut()
        .spawn((InspectorSection::ExoticExperiments, Node::default()))
        .id();
    let _header_exotic = app
        .world_mut()
        .spawn((Text::new(""), InspectorExoticHeader))
        .id();

    app.add_systems(Update, update_scenario_contextual_ui);

    // Select Earth
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.selected_entity = Some(earth_ent);
    }

    // 1. Default tab is InspectorTab::Overview: OrbitTracking and MassComposition visible
    app.update();
    assert_eq!(
        app.world().get::<Node>(sec_orbit).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Node>(sec_mass).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Node>(sec_terra).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(sec_astro).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(sec_exotic).unwrap().display,
        Display::None
    );

    // 2. Switch to Geodynamics & Bio tab: TerraformingBombardment and Astrophysics visible
    {
        let mut hud = app.world_mut().resource_mut::<HudVisibilityState>();
        hud.active_inspector_tab = InspectorTab::GeodynamicsBio;
    }
    app.update();
    assert_eq!(
        app.world().get::<Node>(sec_orbit).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(sec_mass).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(sec_terra).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Node>(sec_astro).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Node>(sec_exotic).unwrap().display,
        Display::None
    );

    // 3. Switch to Fields & GR tab: ExoticExperiments visible
    {
        let mut hud = app.world_mut().resource_mut::<HudVisibilityState>();
        hud.active_inspector_tab = InspectorTab::RelativityFields;
    }
    app.update();
    assert_eq!(
        app.world().get::<Node>(sec_orbit).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(sec_mass).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(sec_terra).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(sec_astro).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(sec_exotic).unwrap().display,
        Display::Flex
    );
}
