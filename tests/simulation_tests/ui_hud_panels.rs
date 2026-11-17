//! HUD panels, quick bar, epoch scrubber, HR/telemetry drawers, and hotkey priority tests.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::game::ui::types::PlanetBuilderState;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::utils::constants::EARTH_MASS_SOLAR;

#[test]
fn test_tech_tree_panel_skips_content_rebuild_on_passive_science() {
    use protostellar::game::ui::tech_tree_panel::{
        spawn_tech_tree_panel, update_tech_tree_panel, TechTreeSummaryLifetimeText,
        TechTreeSummaryPointsText,
    };
    use protostellar::game::ui::types::{TechTreePanelElement, TechTreePanelState};
    use protostellar::simulation::economy::{ScienceData, UnlockedTechs};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TechTreePanelState { is_open: true })
        .init_resource::<ScienceData>()
        .init_resource::<UnlockedTechs>()
        .add_systems(Startup, |mut commands: Commands| {
            commands
                .spawn(Node::default())
                .with_children(spawn_tech_tree_panel);
        })
        .add_systems(Update, update_tech_tree_panel);

    app.update();

    let content_entity = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &TechTreePanelElement)>();
        q.iter(world)
            .find_map(|(e, elem)| matches!(elem, TechTreePanelElement::Content).then_some(e))
            .expect("TechTreePanelElement::Content must exist")
    };

    let initial_children: Vec<Entity> = app
        .world()
        .get::<Children>(content_entity)
        .expect("Content must be populated when TechTreePanel is open")
        .iter()
        .collect();
    assert_ne!(initial_children, Vec::new());

    // Passive science gain below the cheapest node cost (150.0) must update summary text
    // in-place without despawning or recreating Content children.
    app.world_mut()
        .resource_mut::<ScienceData>()
        .add_science(10.0);
    app.update();

    let after_passive_children: Vec<Entity> = app
        .world()
        .get::<Children>(content_entity)
        .expect("Content children after passive science update")
        .iter()
        .collect();
    assert_eq!(
        after_passive_children, initial_children,
        "Tech tree content entities must not rebuild when affordability mask is unchanged"
    );

    let (points_text, lifetime_text) = {
        let world = app.world_mut();
        let points = world
            .query_filtered::<&Text, With<TechTreeSummaryPointsText>>()
            .single(world)
            .expect("points summary text")
            .0
            .clone();
        let lifetime = world
            .query_filtered::<&Text, With<TechTreeSummaryLifetimeText>>()
            .single(world)
            .expect("lifetime summary text")
            .0
            .clone();
        (points, lifetime)
    };
    assert!(
        points_text.contains("110"),
        "Expected updated points '110' in summary text: {points_text}"
    );
    assert!(
        lifetime_text.contains("110"),
        "Expected updated lifetime '110' in summary text: {lifetime_text}"
    );

    // Crossing the 150.0 threshold changes affordability and must rebuild Content children.
    app.world_mut()
        .resource_mut::<ScienceData>()
        .add_science(50.0);
    app.update();

    let after_unlockable_children: Vec<Entity> = app
        .world()
        .get::<Children>(content_entity)
        .expect("Content children after affordability threshold crossed")
        .iter()
        .collect();
    assert_ne!(
        after_unlockable_children, initial_children,
        "Tech tree content must rebuild when a new node becomes affordable"
    );
}

#[test]
fn test_quick_bar_sort_by_mass_skips_redundant_rebuild() {
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::game::ui::types::TrophiesPanelState;
    use protostellar::game::ui::{
        update_quick_body_selector_bar, HudActionTooltipText, HudVisibilityState,
        NotificationToast, PlanetBuilderState, QuickBarState, QuickBodySelectorBar,
        TelemetryPanelState,
    };
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::simulation::scenarios::LoadScenarioEvent;
    use protostellar::simulation::telemetry::SimulationTelemetryHistory;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<TimeWarp>()
        .insert_resource(QuickBarState {
            sort_by_mass: true,
            ..default()
        })
        .init_resource::<TrophiesPanelState>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<SimulationConfig>()
        .init_resource::<DiskParameters>()
        .init_resource::<SimTime>()
        .init_resource::<NotificationToast>()
        .init_resource::<PlanetBuilderState>()
        .init_resource::<TelemetryPanelState>()
        .init_resource::<SimulationTelemetryHistory>()
        .init_resource::<HudVisibilityState>()
        .init_resource::<LateHeavyBombardmentState>()
        .add_message::<LoadScenarioEvent>()
        .add_systems(Update, update_quick_body_selector_bar);

    app.world_mut().spawn(PanOrbitCamera {
        focus: Vec3::ZERO,
        target_focus: Vec3::ZERO,
        radius: 50.0,
        target_radius: 50.0,
        ..default()
    });
    app.world_mut().spawn((Text::new(""), HudActionTooltipText));

    app.world_mut().spawn((
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
    ));
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Mass(0.000_003_003),
            Radius(0.000_042_6),
            Composition::default(),
        ))
        .id();
    app.world_mut().spawn((
        CelestialBody {
            name: "Jupiter".to_string(),
            body_type: BodyType::GasGiant,
        },
        SimPosition(DVec3::new(5.2, 0.0, 0.0)),
        SimVelocity(DVec3::ZERO),
        Mass(0.000_954),
        Radius(0.000_467),
        Composition::default(),
    ));

    let bar_ent = app
        .world_mut()
        .spawn((QuickBodySelectorBar, Node::default()))
        .id();

    app.update();
    let frame_1_children: Vec<Entity> = app
        .world()
        .get::<Children>(bar_ent)
        .expect("quick bar populated on frame 1")
        .iter()
        .collect();
    assert_ne!(frame_1_children, Vec::new());

    // Steady frame with sort_by_mass enabled and identical body masses must reuse children.
    app.update();
    let frame_2_children: Vec<Entity> = app
        .world()
        .get::<Children>(bar_ent)
        .expect("quick bar populated on frame 2")
        .iter()
        .collect();
    assert_eq!(
        frame_2_children, frame_1_children,
        "Quick bar must skip UI teardown/rebuild when quantized masses are unchanged"
    );

    // Changing mass significantly when sorted by mass must invalidate the fingerprint and rebuild.
    app.world_mut().entity_mut(earth_ent).insert(Mass(0.002));
    app.update();
    let frame_3_children: Vec<Entity> = app
        .world()
        .get::<Children>(bar_ent)
        .expect("quick bar populated on frame 3")
        .iter()
        .collect();
    assert_ne!(
        frame_3_children, frame_2_children,
        "Quick bar must rebuild when mass sort order changes"
    );
}

#[test]
fn test_epoch_scrubber_actions_and_panel_updates() {
    use protostellar::game::ui::epoch_scrubber::{
        handle_epoch_scrubber_action, spawn_epoch_scrubber_panel, update_epoch_scrubber_ui,
    };
    use protostellar::game::ui::types::{EpochScrubberPanel, NotificationToast, UiButtonAction};
    use protostellar::simulation::geology::types::{EpochTargetPlanet, TimelineScrubber};

    let mut toast = NotificationToast::default();
    let mut scrubber = TimelineScrubber::default();
    assert!(handle_epoch_scrubber_action(
        &UiButtonAction::ToggleEpochScrubberPanel,
        Some(&mut scrubber),
        &mut toast
    ));
    assert!(scrubber.is_open);

    // Clamping time steps at [0.0, 5.50] Ga and forwarding SelectEpochTargetPlanet -> false
    assert!(handle_epoch_scrubber_action(
        &UiButtonAction::ScrubTimeStep(-10_000),
        Some(&mut scrubber),
        &mut toast
    ));
    assert_eq!(scrubber.scrubbed_age_gyr, 0.0);
    assert!(handle_epoch_scrubber_action(
        &UiButtonAction::ScrubTimeStep(10_000),
        Some(&mut scrubber),
        &mut toast
    ));
    assert_eq!(scrubber.scrubbed_age_gyr, 5.50);
    assert!(!handle_epoch_scrubber_action(
        &UiButtonAction::SelectEpochTargetPlanet(EpochTargetPlanet::Mars),
        Some(&mut scrubber),
        &mut toast
    ));
    assert_eq!(scrubber.target_planet, EpochTargetPlanet::Mars);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(scrubber)
        .init_resource::<PlayerInteractionState>()
        .add_systems(Startup, |mut commands: Commands| {
            commands
                .spawn(Node::default())
                .with_children(spawn_epoch_scrubber_panel);
        })
        .add_systems(Update, update_epoch_scrubber_ui);

    app.update();
    app.update();

    let display = app
        .world_mut()
        .query_filtered::<&Node, With<EpochScrubberPanel>>()
        .single(app.world())
        .expect("EpochScrubberPanel exists")
        .display;
    assert_eq!(display, Display::Flex);
}

#[test]
fn test_hr_diagram_and_telemetry_panel_updates() {
    use protostellar::game::ui::hr_diagram_panel::{
        handle_hr_diagram_action, spawn_hr_diagram_panel, update_hr_diagram_ui,
    };
    use protostellar::game::ui::telemetry_panel::{
        handle_telemetry_action, spawn_telemetry_panel, update_telemetry_graph_ui,
    };
    use protostellar::game::ui::types::{
        HrDiagramDetailsText, HrDiagramPanelState, NotificationToast, TelemetryGraphPanel,
        TelemetryGraphReadoutText, TelemetryPanelState, UiButtonAction,
    };
    use protostellar::simulation::telemetry::{
        PlanetaryTelemetrySample, SimulationTelemetryHistory, TelemetryMetric,
    };

    let mut toast = NotificationToast::default();
    let mut hr = HrDiagramPanelState::default();
    assert!(handle_hr_diagram_action(
        &UiButtonAction::ToggleHrDiagram,
        &mut hr,
        &mut toast
    ));
    assert!(hr.is_open);

    let mut telemetry_state = TelemetryPanelState::default();
    let mut history = SimulationTelemetryHistory::default();
    assert!(handle_telemetry_action(
        &UiButtonAction::ToggleTelemetryPanel,
        &mut telemetry_state,
        &mut history,
        &mut toast
    ));
    assert!(handle_telemetry_action(
        &UiButtonAction::SelectTelemetryMetric(TelemetryMetric::Temperature),
        &mut telemetry_state,
        &mut history,
        &mut toast
    ));
    assert_eq!(
        telemetry_state.selected_metric,
        TelemetryMetric::Temperature
    );

    history.tracked_name = "Earth".to_string();
    history.samples.push_back(PlanetaryTelemetrySample {
        sim_time_yr: 100.0,
        body_name: "Earth".to_string(),
        mass_earth: 1.0,
        radius_km: 6371.0,
        semi_major_axis_au: 1.0,
        eccentricity: 0.017,
        surface_temp_k: 288.0,
        atmospheric_pressure_bar: 1.0,
        ocean_coverage_frac: 0.71,
        habitability_score: 0.95,
        biomass_coverage_frac: 0.80,
    });

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(hr)
        .insert_resource(telemetry_state)
        .insert_resource(history)
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Node::default()).with_children(|root| {
                spawn_hr_diagram_panel(root);
                spawn_telemetry_panel(root);
            });
        })
        .add_systems(Update, (update_hr_diagram_ui, update_telemetry_graph_ui));

    app.world_mut().spawn((
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        CentralStar,
        Mass(1.0),
        Radius(protostellar::utils::constants::SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Luminosity(1.0),
    ));

    app.update();
    app.update();

    let telem_vis = app
        .world_mut()
        .query_filtered::<&Node, With<TelemetryGraphPanel>>()
        .single(app.world())
        .expect("TelemetryGraphPanel exists")
        .display;
    assert_eq!(telem_vis, Display::Flex);

    let hr_details = app
        .world_mut()
        .query_filtered::<&Text, With<HrDiagramDetailsText>>()
        .single(app.world())
        .expect("HrDiagramDetailsText exists")
        .0
        .clone();
    assert!(
        hr_details.contains("5778"),
        "HR diagram details must reflect star temperature: {hr_details}"
    );

    let telem_readout = app
        .world_mut()
        .query_filtered::<&Text, With<TelemetryGraphReadoutText>>()
        .single(app.world())
        .expect("TelemetryGraphReadoutText exists")
        .0
        .clone();
    assert!(
        telem_readout.contains("288"),
        "Telemetry panel readout must include sample temperature: {telem_readout}"
    );
}

#[test]
fn test_builder_and_orbital_params_guards() {
    use bevy::ecs::system::SystemState;
    use protostellar::game::ui::builder_state::BuilderPreset;
    use protostellar::game::ui::interactions::builder::handle_builder_action;
    use protostellar::game::ui::interactions::editor::orbital_params::{
        handle_composition_cycle, handle_orbital_adjustment,
    };
    use protostellar::game::ui::types::{
        NotificationToast, SelectedWorldQuery, TelemetryPanelState, UiButtonAction,
    };
    use protostellar::rendering::camera::PanOrbitCamera;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<SimulationConfig>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<PlanetBuilderState>()
        .init_resource::<TelemetryPanelState>()
        .init_resource::<NotificationToast>();

    let star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Sun".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            CentralStar,
            Mass(1.0),
            Radius(protostellar::utils::constants::SOLAR_RADIUS_AU),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Composition::solar_gas(),
        ))
        .id();

    let polar_planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Polar World".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(protostellar::utils::constants::EARTH_RADIUS_AU),
            SimPosition(DVec3::new(0.0, 1.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Composition::rocky(),
        ))
        .id();

    let _cam_ent = app.world_mut().spawn(PanOrbitCamera::default()).id();

    let mut state: SystemState<(Commands, SelectedWorldQuery, Query<&mut PanOrbitCamera>)> =
        SystemState::new(app.world_mut());

    // 1. Builder preset and stepper clamps
    let mut builder = PlanetBuilderState::default();
    let mut toast = NotificationToast::default();
    let mut player = PlayerInteractionState::default();
    {
        let (mut commands, selected_q, mut cam_q) = state.get_mut(app.world_mut()).unwrap();
        assert!(handle_builder_action(
            &UiButtonAction::BuilderSelectPreset(BuilderPreset::SuperEarth),
            &mut builder,
            &mut toast,
            1.0,
            &mut player,
            &mut cam_q,
            &selected_q,
            &mut commands,
        ));
        builder.mass_solar = 1e-12;
        handle_builder_action(
            &UiButtonAction::BuilderMassStep(-2),
            &mut builder,
            &mut toast,
            1.0,
            &mut player,
            &mut cam_q,
            &selected_q,
            &mut commands,
        );
        assert!(builder.mass_solar >= EARTH_MASS_SOLAR * 0.001);

        builder.semi_major_axis_au = 0.05;
        handle_builder_action(
            &UiButtonAction::BuilderDistanceStep(-2),
            &mut builder,
            &mut toast,
            1.0,
            &mut player,
            &mut cam_q,
            &selected_q,
            &mut commands,
        );
        assert_eq!(builder.semi_major_axis_au, 0.1);
    }

    // 2. Orbital adjustment & composition protection for CentralStar at ZERO and polar planet
    {
        let (_commands, mut selected_q, _cam_q) = state.get_mut(app.world_mut()).unwrap();
        player.selected_entity = Some(star_ent);
        handle_orbital_adjustment(
            &UiButtonAction::ExpandOrbit,
            &mut selected_q,
            &player,
            1.0,
            &mut toast,
        );
        handle_composition_cycle(&mut selected_q, &player, &mut toast);

        player.selected_entity = Some(polar_planet);
        handle_orbital_adjustment(
            &UiButtonAction::ExpandOrbit,
            &mut selected_q,
            &player,
            1.0,
            &mut toast,
        );
    }

    let star_comp = app.world().get::<Composition>(star_ent).unwrap();
    assert!(
        star_comp.gas_frac > 0.9,
        "Central star composition must not be overwritten by cycle tool"
    );
    let star_vel = app.world().get::<SimVelocity>(star_ent).unwrap().0;
    assert_eq!(star_vel, DVec3::ZERO);

    let polar_pos = app.world().get::<SimPosition>(polar_planet).unwrap().0;
    let polar_vel = app.world().get::<SimVelocity>(polar_planet).unwrap().0;
    assert!(
        polar_pos.is_finite() && polar_vel.is_finite() && polar_vel.length() > 1.0,
        "Polar orbit expansion must fall back cleanly without NaN or zero speed: pos={polar_pos:?}, vel={polar_vel:?}"
    );
}

#[test]
fn test_viewport_hotkeys_escape_priority_stack() {
    use bevy::ecs::system::SystemState;
    use protostellar::game::interaction::hotkeys::handle_viewport_and_tool_hotkeys;
    use protostellar::game::ui::types::{NotificationToast, TelemetryPanelState};
    use protostellar::rendering::camera::PanOrbitCamera;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<SimulationConfig>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<PlanetBuilderState>()
        .init_resource::<TelemetryPanelState>()
        .init_resource::<NotificationToast>();

    let polar_planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Polar World".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(protostellar::utils::constants::EARTH_RADIUS_AU),
            SimPosition(DVec3::new(0.0, 1.0, 0.0)),
            SimVelocity(DVec3::ZERO),
        ))
        .id();
    let cam_ent = app
        .world_mut()
        .spawn((Transform::default(), PanOrbitCamera::default()))
        .id();

    let mut keys = ButtonInput::<KeyCode>::default();
    let mut config = SimulationConfig::default();
    let mut player = PlayerInteractionState {
        selected_entity: Some(polar_planet),
        ..default()
    };
    let mut builder = PlanetBuilderState::default();
    let mut telemetry = TelemetryPanelState::default();
    let mut toast = NotificationToast::default();
    if let Some(mut cam) = app.world_mut().get_mut::<PanOrbitCamera>(cam_ent) {
        cam.target_entity = Some(polar_planet);
    }
    let mut cam_state: SystemState<Query<(&Transform, &mut PanOrbitCamera)>> =
        SystemState::new(app.world_mut());

    let mut dispatch =
        |key: KeyCode, player: &mut PlayerInteractionState, builder: &mut PlanetBuilderState| {
            keys.reset_all();
            keys.press(key);
            let mut cam_q = cam_state.get_mut(app.world_mut()).unwrap();
            handle_viewport_and_tool_hotkeys(
                &keys,
                &mut config,
                player,
                builder,
                &mut telemetry,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                &mut toast,
                &mut cam_q,
                None,
                false,
                None,
                None,
                None,
            );
        };

    dispatch(KeyCode::KeyP, &mut player, &mut builder);
    assert!(builder.is_open);

    dispatch(KeyCode::Escape, &mut player, &mut builder);
    assert!(!builder.is_open, "First Esc must close open modal");
    assert_eq!(
        player.selected_entity,
        Some(polar_planet),
        "First Esc closing a modal must keep selected target"
    );

    dispatch(KeyCode::Escape, &mut player, &mut builder);
    assert!(
        player.selected_entity.is_none(),
        "Second Esc must deselect body when no modals remain open"
    );
    assert!(
        app.world()
            .get::<PanOrbitCamera>(cam_ent)
            .unwrap()
            .target_entity
            .is_none(),
        "Second Esc must clear camera focus target"
    );
}
