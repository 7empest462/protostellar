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
    ActiveModalKind, ModalPanelStates, NotificationToast, PlanetBuilderState, TelemetryPanelState,
    close_conflicting_modals,
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
    let mut hazards = HazardsPanelState { is_open: false };
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
    let mut saves = protostellar::game::ui::SavesPanelState {
        is_open: false,
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
        saves: Some(&mut saves),
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
        saves: Some(&mut saves),
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
        saves: Some(&mut saves),
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
            saves: Some(&mut saves),
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

fn escape_saves_runner(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<SimulationConfig>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut builder_state: ResMut<PlanetBuilderState>,
    mut telemetry_state: ResMut<TelemetryPanelState>,
    mut saves_state: ResMut<protostellar::game::ui::saves_panel::SavesPanelState>,
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
        Some(&mut saves_state),
    );
}

#[test]
fn test_saves_panel_modal_exclusion_and_escape() {
    use protostellar::game::ui::saves_panel::SavesPanelState;

    let mut builder = PlanetBuilderState {
        is_open: true,
        ..default()
    };
    let mut telemetry = TelemetryPanelState::default();
    let mut saves = SavesPanelState {
        is_open: true,
        ..default()
    };
    {
        let mut states = ModalPanelStates {
            builder: Some(&mut builder),
            telemetry: Some(&mut telemetry),
            trophies: None,
            tech_tree: None,
            campaign: None,
            hazards: None,
            scrubber: None,
            hr_diagram: None,
            visibility: None,
            saves: Some(&mut saves),
        };
        assert!(states.close_conflicting_modals(ActiveModalKind::SavesManager));
    }
    assert!(
        !builder.is_open,
        "Builder must close when SavesManager opens"
    );
    assert!(saves.is_open, "SavesManager must stay open");
    {
        let mut states = ModalPanelStates {
            builder: Some(&mut builder),
            telemetry: Some(&mut telemetry),
            trophies: None,
            tech_tree: None,
            campaign: None,
            hazards: None,
            scrubber: None,
            hr_diagram: None,
            visibility: None,
            saves: Some(&mut saves),
        };
        assert!(states.close_conflicting_modals(ActiveModalKind::None));
    }
    assert!(
        !saves.is_open,
        "ActiveModalKind::None must close SavesManager"
    );

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<PlanetBuilderState>()
        .init_resource::<TelemetryPanelState>()
        .init_resource::<NotificationToast>()
        .insert_resource(SavesPanelState {
            is_open: true,
            ..default()
        })
        .init_resource::<ButtonInput<KeyCode>>();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.world_mut()
        .run_system_once(escape_saves_runner)
        .expect("run escape handler");
    assert!(
        !app.world().resource::<SavesPanelState>().is_open,
        "Escape key must close open SavesPanelState"
    );
}

#[derive(Resource)]
struct MultiSlotActionStep(Vec<UiButtonAction>);

fn make_multi_slot_test_save(
    preset: Option<protostellar::simulation::scenarios::ScenarioPreset>,
    epoch_yr: f64,
) -> protostellar::simulation::serialization::SystemSaveData {
    use protostellar::simulation::serialization::{
        ConfigSave, EventFlagsSave, SystemSaveData, TimeWarpSave,
    };
    SystemSaveData {
        version: 1,
        timestamp_epoch_yr: epoch_yr,
        step_count: 42,
        time_warp: TimeWarpSave::default(),
        disk_parameters: DiskParameters::default(),
        config_save: ConfigSave::default(),
        bodies: Vec::new(),
        event_flags: EventFlagsSave::default(),
        scenario_preset: preset,
        ..Default::default()
    }
}

fn sample_multi_slot_entry(
    slot_file: &str,
) -> protostellar::simulation::serialization::SaveSlotEntry {
    use protostellar::simulation::scenarios::ScenarioPreset;
    use protostellar::simulation::serialization::SaveSlotEntry;
    SaveSlotEntry {
        slot_name: "trappist_test_01".to_string(),
        file_path: slot_file.to_string(),
        scenario_preset: Some(ScenarioPreset::Trappist1System),
        body_count: 8,
        timestamp_epoch_yr: 120.0,
        step_count: 42,
        format_tag: "JSON",
        modified_secs: 1,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Test harness system dispatches SavesPanel UiButtonAction sequences"
)]
fn run_multi_slot_actions_system(
    actions: Res<MultiSlotActionStep>,
    mut save_events: MessageWriter<protostellar::simulation::serialization::SaveSystemEvent>,
    mut load_events: MessageWriter<protostellar::simulation::serialization::LoadSystemEvent>,
    mut slot_state: ResMut<protostellar::game::ui::widgets::SaveSlotUiState>,
    mut saves_panel: ResMut<protostellar::game::ui::saves_panel::SavesPanelState>,
    scenario_state: Res<ActiveScenarioState>,
    mut toast: ResMut<NotificationToast>,
) {
    use protostellar::game::ui::interactions::system_tools::handle_save_load_action;
    for action in &actions.0 {
        let handled = handle_save_load_action(
            action,
            Some(&mut save_events),
            Some(&mut load_events),
            None,
            None,
            Some(&mut slot_state),
            Some(&mut saves_panel),
            Some(&scenario_state),
            &mut toast,
        );
        assert!(handled);
    }
}

fn setup_multi_slot_saves_test_app(slot_file: &str) -> App {
    use protostellar::game::ui::saves_panel::SavesPanelState;
    use protostellar::game::ui::widgets::SaveSlotUiState;
    use protostellar::simulation::scenarios::ScenarioPreset;
    use protostellar::simulation::serialization::{LoadSystemEvent, SaveSystemEvent};

    let mut app = App::new();
    app.add_message::<SaveSystemEvent>()
        .add_message::<LoadSystemEvent>()
        .init_resource::<NotificationToast>()
        .insert_resource(SaveSlotUiState {
            slot_name: "my custom run.json".to_string(),
        })
        .insert_resource(ActiveScenarioState {
            current_preset: ScenarioPreset::Trappist1System,
            ..default()
        })
        .insert_resource(SavesPanelState {
            is_open: true,
            entries: vec![sample_multi_slot_entry(slot_file)],
            total_saved_count: 1,
            ..default()
        })
        .insert_resource(MultiSlotActionStep(vec![
            UiButtonAction::SaveNamedSlot,
            UiButtonAction::SelectSaveSlot(0),
            UiButtonAction::OverwriteSaveSlot(0),
            UiButtonAction::DeleteSaveSlot(0),
            UiButtonAction::DeleteSaveSlot(0),
        ]))
        .add_systems(Update, run_multi_slot_actions_system);
    app
}

#[test]
fn test_saves_panel_multi_slot_actions() {
    use protostellar::game::ui::saves_panel::SavesPanelState;
    use protostellar::game::ui::widgets::SaveSlotUiState;
    use protostellar::simulation::scenarios::ScenarioPreset;
    use protostellar::simulation::serialization::{
        LoadSystemEvent, SaveSystemEvent, save_system_to_file,
    };

    let test_dir = "target/test_multi_saves_ui";
    let _ = std::fs::remove_dir_all(test_dir);
    let slot_file = format!("{test_dir}/trappist_test_01.json");
    save_system_to_file(
        &make_multi_slot_test_save(Some(ScenarioPreset::Trappist1System), 120.0),
        &slot_file,
    )
    .expect("write test save");

    let mut app = setup_multi_slot_saves_test_app(&slot_file);
    app.update();

    let save_msgs = app.world().resource::<Messages<SaveSystemEvent>>();
    let mut cursor = save_msgs.get_cursor();
    let emitted: Vec<String> = cursor.read(save_msgs).map(|e| e.filename.clone()).collect();
    assert_eq!(
        emitted.first().map(String::as_str),
        Some("saves/my_custom_run.json")
    );
    assert_eq!(emitted.get(1).map(String::as_str), Some(slot_file.as_str()));
    assert!(!std::path::Path::new(&slot_file).exists());

    // LoadSaveSlot closes modal and sets active slot; SaveNewScenarioSlot names with scenario slug
    save_system_to_file(&make_multi_slot_test_save(None, 0.0), &slot_file)
        .expect("recreate test save");
    {
        let mut panel = app.world_mut().resource_mut::<SavesPanelState>();
        panel.is_open = true;
        panel.entries = vec![sample_multi_slot_entry(&slot_file)];
    }
    app.world_mut().resource_mut::<MultiSlotActionStep>().0 = vec![UiButtonAction::LoadSaveSlot(0)];
    app.update();
    assert!(!app.world().resource::<SavesPanelState>().is_open);
    assert_eq!(
        app.world().resource::<SaveSlotUiState>().slot_name,
        "trappist_test_01"
    );
    let load_msgs = app.world().resource::<Messages<LoadSystemEvent>>();
    let mut l_cursor = load_msgs.get_cursor();
    assert_eq!(
        l_cursor.read(load_msgs).last().map(|e| e.filename.as_str()),
        Some(slot_file.as_str())
    );

    app.world_mut().resource_mut::<MultiSlotActionStep>().0 = vec![
        UiButtonAction::ToggleSavesPanel,
        UiButtonAction::ToggleSavesScenarioFilter,
        UiButtonAction::SaveNewScenarioSlot,
    ];
    app.update();

    assert!(app.world().resource::<SavesPanelState>().is_open);
    assert!(
        app.world()
            .resource::<SavesPanelState>()
            .filter_active_scenario
    );
    let generated_slot = app.world().resource::<SaveSlotUiState>().slot_name.clone();
    assert!(
        generated_slot.starts_with("trappist1_"),
        "SaveNewScenarioSlot should name slot with scenario slug, got {generated_slot}"
    );
    let _ = std::fs::remove_dir_all(test_dir);
}

#[test]
fn test_saves_modal_absorbs_clicks_and_button_spawns_ui_click_cue_when_unmuted() {
    use protostellar::game::audio::{
        CosmicAmbientPlayer, CosmicAudioHandles, CosmicAudioState, setup_cosmic_audio_assets,
    };
    use protostellar::game::phases::{LateHeavyBombardmentState, SystemPhase};
    use protostellar::game::ui::{
        ActivatedHudButtons, HudButton, HudVisibilityState, NotificationToast, PlanetBuilderState,
        QuickBarState, SavesPanelElement, TelemetryPanelState, UiButtonAction,
        handle_ui_button_interactions, setup_hud,
    };
    use protostellar::simulation::telemetry::SimulationTelemetryHistory;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<Assets<AudioSource>>();
    app.init_resource::<CosmicAudioHandles>();
    app.init_resource::<CosmicAudioState>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ActivatedHudButtons>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.init_resource::<QuickBarState>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<TelemetryPanelState>();
    app.init_resource::<protostellar::game::ui::types::TrophiesPanelState>();
    app.init_resource::<HudVisibilityState>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<SimulationTelemetryHistory>();
    app.add_plugins(protostellar::simulation::SimulationPlugin);
    app.add_systems(Startup, (setup_hud, setup_cosmic_audio_assets));
    app.add_systems(Update, handle_ui_button_interactions);
    app.update();

    // 1. Saves modal container must carry FocusPolicy::Block + Hovered (not Pickable::IGNORE) to absorb clicks
    let mut saves_q = app.world_mut().query::<(
        &SavesPanelElement,
        Option<&bevy::ui::FocusPolicy>,
        Has<bevy::picking::hover::Hovered>,
        Option<&Pickable>,
    )>();
    let (_, opt_focus, has_hovered, opt_pickable) = saves_q
        .iter(app.world())
        .find(|(elem, ..)| matches!(elem, SavesPanelElement::Container))
        .expect("SavesPanelElement::Container must be spawned");
    assert_eq!(
        opt_focus,
        Some(&bevy::ui::FocusPolicy::Block),
        "Saves modal container must block pointer propagation"
    );
    assert!(
        has_hovered,
        "Saves modal container must track Hovered state so viewport picking yields to modal"
    );
    assert_ne!(
        opt_pickable,
        Some(&Pickable::IGNORE),
        "Saves modal container must not ignore pointer picking"
    );

    // 2. While audio is muted, pressing a HUD button must NOT spawn a one-shot UI click AudioPlayer
    let any_btn = app
        .world_mut()
        .query_filtered::<(Entity, &UiButtonAction), With<HudButton>>()
        .iter(app.world())
        .find(|(_, act)| matches!(act, UiButtonAction::ToggleTopLeftPanel))
        .map(|(e, _)| e)
        .expect("HUD button must exist");
    app.world_mut()
        .entity_mut(any_btn)
        .insert(bevy::ui::Pressed);
    app.world_mut()
        .resource_mut::<ActivatedHudButtons>()
        .0
        .push(any_btn);
    app.update();
    let muted_cues = app
        .world_mut()
        .query_filtered::<Entity, (With<AudioPlayer>, Without<CosmicAmbientPlayer>)>()
        .iter(app.world())
        .count();
    assert_eq!(muted_cues, 0, "Muted UI must not spawn click audio");

    // 3. When audio is enabled, pressing a HUD button spawns a DESPAWN one-shot cue at 0.25 * master_volume
    {
        let mut audio = app.world_mut().resource_mut::<CosmicAudioState>();
        audio.enabled = true;
        audio.master_volume = 0.8;
    }
    app.world_mut()
        .entity_mut(any_btn)
        .insert(bevy::ui::Pressed);
    app.world_mut()
        .resource_mut::<ActivatedHudButtons>()
        .0
        .push(any_btn);
    app.update();

    let mut cue_q = app
        .world_mut()
        .query_filtered::<&PlaybackSettings, (With<AudioPlayer>, Without<CosmicAmbientPlayer>)>();
    let cue_settings: Vec<&PlaybackSettings> = cue_q.iter(app.world()).collect();
    assert_eq!(
        cue_settings.len(),
        1,
        "Unmuted button click must spawn 1 UI click cue"
    );
    assert!(matches!(
        cue_settings[0].mode,
        bevy::audio::PlaybackMode::Despawn
    ));
    assert_eq!(
        cue_settings[0].volume,
        bevy::audio::Volume::Linear(0.8 * 0.25)
    );
}
