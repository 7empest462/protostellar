//! Track 3 regressions: bundled geometric HUD font, single-row horizontal selector strip,
//! Cinema Mode restoration, and clickable Cosmic Event Feed with zero-churn updates.

use bevy::input::mouse::MouseWheel;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::text::FontSource;
use bevy::ui::{ComputedNode, ScrollPosition};
use protostellar::game::audio::CosmicAudioState;
use protostellar::game::interaction::hotkeys::handle_viewport_and_tool_hotkeys;
use protostellar::game::phases::{LateHeavyBombardmentState, SystemPhase};
use protostellar::game::ui::event_feed::{
    EventFeedListContainer, detect_biosphere_emergence_events, update_cosmic_event_feed_ui,
};
use protostellar::game::ui::scroll::handle_inspector_scroll;
use protostellar::game::ui::selector::{body_to_button_label, update_quick_body_selector_bar};
use protostellar::game::ui::types::{
    CosmicEventLog, HudDynamicText, HudPanelElement, HudVisibilityState, NotificationToast,
    PlanetBuilderState, QuickBarScrollViewport, QuickBarState, QuickBodySelectorBar,
    TelemetryPanelState, TrophiesPanelState, UiButtonAction,
};
use protostellar::game::ui::visibility::update_hud_visibility;
use protostellar::game::ui::widgets::{
    ActivatedHudButtons, BundledHudFont, apply_bundled_hud_font_when_loaded, hud_font_source,
    hud_font_source_with_bundled, upgrade_hud_default_fonts,
};
use protostellar::game::ui::{
    handle_cosmic_event_toasts, handle_ui_button_interactions, setup_hud,
};
use protostellar::rendering::camera::PanOrbitCamera;
use protostellar::simulation::accretion::events::AccretionMergeEvent;
use protostellar::simulation::components::{
    BiosphereState, BodyType, CelestialBody, CentralStar, Composition, Mass,
    PlanetaryEngulfmentEvent, Radius, SimPosition, SimVelocity,
};
use protostellar::simulation::resources::{
    DiagnosticOverlayMode, DiskParameters, OrbitVisualizationMode, PlayerInteractionState, SimTime,
    SimulationConfig, TimeWarp,
};
use protostellar::simulation::telemetry::SimulationTelemetryHistory;
use protostellar::simulation::thermodynamics::StarIgnitionEvent;
use protostellar::utils::constants::EARTH_MASS_SOLAR;

fn build_track3_base_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ActivatedHudButtons>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.init_resource::<QuickBarState>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<TelemetryPanelState>();
    app.init_resource::<TrophiesPanelState>();
    app.init_resource::<HudVisibilityState>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<CosmicEventLog>();
    app.init_resource::<CosmicAudioState>();
    app.init_resource::<SimulationTelemetryHistory>();
    app.init_resource::<Assets<Font>>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.add_message::<MouseWheel>();
    app.add_message::<protostellar::simulation::scenarios::LoadScenarioEvent>();
    app.add_message::<protostellar::simulation::space_weather::CmeShockwaveEvent>();
    app.add_message::<protostellar::simulation::kozai_lidov::KozaiDisruptionEvent>();
    app.add_message::<protostellar::simulation::atmosphere_escape::AtmosphericStrippedEvent>();
    app.add_message::<protostellar::simulation::relativity::GravitationalWaveMergerEvent>();
    app.add_message::<protostellar::simulation::components::SupernovaEvent>();
    app.add_message::<protostellar::simulation::accretion::TidalDisruptionEvent>();
    app.add_message::<protostellar::simulation::accretion::MoonFormationEvent>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<AccretionMergeEvent>();
    app.add_systems(Startup, setup_hud);
    app
}

fn build_track3_hud_app() -> App {
    let mut app = build_track3_base_app();
    app.add_systems(
        Update,
        (
            handle_ui_button_interactions,
            update_quick_body_selector_bar,
            handle_inspector_scroll,
            handle_cosmic_event_toasts,
            detect_biosphere_emergence_events,
            update_cosmic_event_feed_ui,
            update_hud_visibility,
            upgrade_hud_default_fonts,
            apply_bundled_hud_font_when_loaded,
        )
            .chain(),
    );
    app
}

fn trigger_hud_action(app: &mut App, action: UiButtonAction) {
    let target_btn = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &UiButtonAction)>();
        q.iter(world).find(|(_, a)| **a == action).map_or_else(
            || panic!("HUD button action {action:?} must exist"),
            |(e, _)| e,
        )
    };
    app.world_mut()
        .resource_mut::<ActivatedHudButtons>()
        .0
        .push(target_btn);
    app.update();
}

fn spawn_selectable_body(
    world: &mut World,
    name: &str,
    body_type: BodyType,
    mass_solar: f64,
    pos_x: f64,
    is_star: bool,
) -> Entity {
    let mut e = world.spawn((
        CelestialBody {
            name: name.to_string(),
            body_type,
        },
        Mass(mass_solar),
        Radius(0.005),
        SimPosition(DVec3::new(pos_x, 0.0, 0.0)),
        SimVelocity(DVec3::ZERO),
        Composition::default(),
    ));
    if is_star {
        e.insert(CentralStar);
    }
    e.id()
}

#[test]
fn test_bundled_chakra_petch_font_asset_and_upgrade() {
    let ttf_bytes =
        std::fs::read("assets/fonts/ChakraPetch-Medium.ttf").expect("Bundled ChakraPetch TTF");
    assert!(ttf_bytes.len() > 10_000, "Bundled TTF must be non-trivial");
    assert_eq!(
        &ttf_bytes[0..4],
        &[0x00, 0x01, 0x00, 0x00],
        "Bundled font must have valid TrueType sfnt header"
    );
    let license_txt =
        std::fs::read_to_string("assets/fonts/OFL.txt").expect("Bundled OFL license text");
    assert!(license_txt.to_uppercase().contains("SIL OPEN FONT LICENSE"));

    assert!(
        matches!(hud_font_source(), FontSource::List(_)),
        "Pure fallback hud_font_source must remain FontSource::List"
    );

    let mut fonts = Assets::<Font>::default();
    let bundled_handle = fonts.add(Font::from_bytes(ttf_bytes.clone()));
    let custom_handle = fonts.add(Font::from_bytes(ttf_bytes));

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(fonts);
    app.insert_resource(BundledHudFont {
        handle: bundled_handle.clone(),
        upgraded_existing: false,
    });
    app.add_systems(
        Update,
        (
            upgrade_hud_default_fonts,
            apply_bundled_hud_font_when_loaded,
        ),
    );

    let default_ent = app.world_mut().spawn(TextFont::default()).id();
    let base_ent = app
        .world_mut()
        .spawn(TextFont {
            font: hud_font_source(),
            ..default()
        })
        .id();
    let custom_ent = app
        .world_mut()
        .spawn(TextFont {
            font: FontSource::Handle(custom_handle.clone()),
            ..default()
        })
        .id();

    app.update();

    let expected = hud_font_source_with_bundled(Some(bundled_handle));
    assert!(
        app.world().resource::<BundledHudFont>().upgraded_existing,
        "apply_bundled_hud_font_when_loaded must mark upgraded_existing once complete"
    );
    assert_eq!(
        app.world().get::<TextFont>(default_ent).unwrap().font,
        expected
    );
    assert_eq!(
        app.world().get::<TextFont>(base_ent).unwrap().font,
        expected
    );
    assert_eq!(
        app.world().get::<TextFont>(custom_ent).unwrap().font,
        FontSource::Handle(custom_handle),
        "Explicit custom FontSource::Handle must not be overwritten"
    );

    let late_ent = app.world_mut().spawn(TextFont::default()).id();
    app.update();
    assert_eq!(
        app.world().get::<TextFont>(late_ent).unwrap().font,
        expected
    );
}

#[test]
fn test_single_row_quick_bar_layout_horizontal_scroll_and_clean_labels() {
    let star_label = body_to_button_label("The Star (Red Giant)", BodyType::RedGiant, true);
    let planet_label = body_to_button_label(
        "Kepler-22b (Terrestrial)",
        BodyType::TerrestrialPlanet,
        false,
    );
    assert!(!star_label.contains("(Red Giant)") && !star_label.contains("[Main Sequence]"));
    assert!(!planet_label.contains("(Terrestrial)") && !planet_label.contains("[Terrestrial]"));

    let mut app = build_track3_hud_app();
    let sol = spawn_selectable_body(
        app.world_mut(),
        "Sol",
        BodyType::MainSequenceStar,
        1.0,
        0.0,
        true,
    );
    let gaia = spawn_selectable_body(
        app.world_mut(),
        "Gaia",
        BodyType::TerrestrialPlanet,
        EARTH_MASS_SOLAR,
        1.0,
        false,
    );
    let ares = spawn_selectable_body(
        app.world_mut(),
        "Ares",
        BodyType::TerrestrialPlanet,
        0.107 * EARTH_MASS_SOLAR,
        1.52,
        false,
    );
    app.update();
    app.update();

    let (viewport_ent, viewport_node) = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Node), With<QuickBarScrollViewport>>();
        let (e, n) = q.single(world).expect("QuickBarScrollViewport must exist");
        (e, n.clone())
    };
    assert_eq!(viewport_node.overflow, Overflow::scroll_x());
    assert_eq!(viewport_node.max_height, Val::Auto);

    let (bar_ent, bar_wrap) = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Node), With<QuickBodySelectorBar>>();
        let (e, n) = q.single(world).expect("QuickBodySelectorBar must exist");
        (e, n.flex_wrap)
    };
    assert_eq!(bar_wrap, FlexWrap::NoWrap);

    let direct_select_targets: Vec<Entity> = {
        let world = app.world_mut();
        let mut q = world.query::<(&UiButtonAction, &ChildOf)>();
        q.iter(world)
            .filter_map(|(a, p)| match a {
                UiButtonAction::SelectEntity(ent) if p.parent() == bar_ent => Some(*ent),
                _ => None,
            })
            .collect()
    };
    assert!(
        direct_select_targets.len() == 3
            && direct_select_targets.contains(&sol)
            && direct_select_targets.contains(&gaia)
            && direct_select_targets.contains(&ares),
        "SelectEntity buttons must be direct children of QuickBodySelectorBar (got {direct_select_targets:?})"
    );

    trigger_hud_action(&mut app, UiButtonAction::ToggleQuickBarFilter);
    assert!(app.world().resource::<QuickBarState>().show_filter);
    let filter_display = {
        let world = app.world_mut();
        let mut q = world.query::<(&HudPanelElement, &Node)>();
        q.iter(world)
            .find(|(el, _)| matches!(el, HudPanelElement::QuickBarFilterRow))
            .map(|(_, n)| n.display)
            .expect("QuickBarFilterRow must exist")
    };
    assert_eq!(filter_display, Display::Flex);

    trigger_hud_action(&mut app, UiButtonAction::ToggleQuickBarWrap);
    let wrapped_viewport = app.world().get::<Node>(viewport_ent).unwrap().clone();
    assert_eq!(wrapped_viewport.overflow, Overflow::scroll_y());
    assert_eq!(wrapped_viewport.max_height, Val::Vh(28.0));
    assert_eq!(
        app.world().get::<Node>(bar_ent).unwrap().flex_wrap,
        FlexWrap::Wrap
    );

    trigger_hud_action(&mut app, UiButtonAction::ToggleQuickBarWrap);
    app.world_mut()
        .entity_mut(viewport_ent)
        .insert(ComputedNode {
            size: Vec2::new(400.0, 34.0),
            content_size: Vec2::new(1200.0, 34.0),
            inverse_scale_factor: 1.0,
            ..default()
        });
    trigger_hud_action(&mut app, UiButtonAction::ScrollQuickBarRight);
    let after_right = app.world().get::<ScrollPosition>(viewport_ent).unwrap().x;
    assert!(
        after_right > 0.0,
        "ScrollQuickBarRight must advance ScrollPosition.x"
    );
    assert_eq!(
        app.world()
            .resource::<QuickBarState>()
            .pending_scroll_delta_x,
        0.0
    );

    trigger_hud_action(&mut app, UiButtonAction::ScrollQuickBarLeft);
    let after_left = app.world().get::<ScrollPosition>(viewport_ent).unwrap().x;
    assert!(
        after_left < after_right,
        "ScrollQuickBarLeft must decrease ScrollPosition.x"
    );
}

fn dispatch_hotkey(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_scope(|world, mut config: Mut<SimulationConfig>| {
            world.resource_scope(|world, mut player: Mut<PlayerInteractionState>| {
                world.resource_scope(|world, mut builder: Mut<PlanetBuilderState>| {
                    world.resource_scope(|world, mut telemetry: Mut<TelemetryPanelState>| {
                        world.resource_scope(|world, mut vis: Mut<HudVisibilityState>| {
                            world.resource_scope(|world, mut toast: Mut<NotificationToast>| {
                                let mut cam_q = world.query::<(&Transform, &mut PanOrbitCamera)>();
                                let mut cam_sys = cam_q.query_mut(world);
                                let mut keys = ButtonInput::<KeyCode>::default();
                                keys.press(key);
                                handle_viewport_and_tool_hotkeys(
                                    &keys,
                                    &mut config,
                                    &mut player,
                                    &mut builder,
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
                                    &mut cam_sys,
                                    None,
                                    false,
                                    None,
                                    Some(&mut vis),
                                    None,
                                );
                            });
                        });
                    });
                });
            });
        });
}

#[test]
fn test_cinema_mode_hotkey_restore_pill_escape_and_orbit_restoration() {
    let mut app = build_track3_hud_app();
    {
        let mut player = app.world_mut().resource_mut::<PlayerInteractionState>();
        player.orbit_mode = OrbitVisualizationMode::All;
        player.overlay_mode = DiagnosticOverlayMode::SpectralComposition;
    }
    app.update();

    dispatch_hotkey(&mut app, KeyCode::Backquote);
    app.update();

    let vis = app.world().resource::<HudVisibilityState>();
    let player = app.world().resource::<PlayerInteractionState>();
    assert!(vis.is_full_screen_clean);
    assert_eq!(player.orbit_mode, OrbitVisualizationMode::Off);
    assert_eq!(player.overlay_mode, DiagnosticOverlayMode::Hidden);

    let (root_disp, pill_disp, badge_txt) = {
        let world = app.world_mut();
        let mut p_q = world.query::<(&HudPanelElement, &Node)>();
        let root_d = p_q
            .iter(world)
            .find(|(e, _)| matches!(e, HudPanelElement::RootContainer))
            .unwrap()
            .1
            .display;
        let pill_d = p_q
            .iter(world)
            .find(|(e, _)| matches!(e, HudPanelElement::CinemaRestorePill))
            .unwrap()
            .1
            .display;
        let mut t_q = world.query::<(&Text, &HudDynamicText)>();
        let badge = t_q
            .iter(world)
            .find(|(_, d)| matches!(d, HudDynamicText::FullScreenBadge))
            .unwrap()
            .0
            .0
            .clone();
        (root_d, pill_d, badge)
    };
    assert_eq!(root_disp, Display::None);
    assert_eq!(pill_disp, Display::Flex);
    assert_eq!(badge_txt, "👁️ Show HUD [`]");

    dispatch_hotkey(&mut app, KeyCode::Escape);
    app.update();

    let vis_after = app.world().resource::<HudVisibilityState>();
    let player_after = app.world().resource::<PlayerInteractionState>();
    assert!(!vis_after.is_full_screen_clean);
    assert_eq!(player_after.orbit_mode, OrbitVisualizationMode::All);
    assert_eq!(
        player_after.overlay_mode,
        DiagnosticOverlayMode::SpectralComposition
    );

    let world = app.world_mut();
    let mut p_q = world.query::<(&HudPanelElement, &Node)>();
    assert_eq!(
        p_q.iter(world)
            .find(|(e, _)| matches!(e, HudPanelElement::RootContainer))
            .unwrap()
            .1
            .display,
        Display::Flex
    );
    assert_eq!(
        p_q.iter(world)
            .find(|(e, _)| matches!(e, HudPanelElement::CinemaRestorePill))
            .unwrap()
            .1
            .display,
        Display::None
    );
}

fn emit_track3_test_events(world: &mut World, star: Entity, planet: Entity, rock: Entity) {
    world.write_message(StarIgnitionEvent {
        star_entity: star,
        star_mass: 1.0,
        luminosity_l_sun: 1.0,
        surface_temp_kelvin: 5778.0,
    });
    world.write_message(PlanetaryEngulfmentEvent {
        planet_entity: planet,
        planet_name: "Vulcan".to_string(),
        distance_au: 0.02,
        planet_mass_earth: 0.8,
    });
    world.write_message(AccretionMergeEvent {
        primary_entity: rock,
        secondary_entity: rock,
        merged_mass: 0.001 * EARTH_MASS_SOLAR,
        merged_position: DVec3::new(2.0, 0.0, 0.0),
        merged_velocity: DVec3::ZERO,
        new_body_type: BodyType::Asteroid,
        energy_released: 1.0e20,
    });
    world.write_message(AccretionMergeEvent {
        primary_entity: planet,
        secondary_entity: rock,
        merged_mass: 0.2 * EARTH_MASS_SOLAR,
        merged_position: DVec3::new(1.0, 0.0, 0.0),
        merged_velocity: DVec3::ZERO,
        new_body_type: BodyType::TerrestrialPlanet,
        energy_released: 1.0e32,
    });
}

#[test]
fn test_cosmic_event_feed_capture_zero_churn_and_jump_to_action() {
    let mut app = build_track3_hud_app();
    app.world_mut().resource_mut::<SimTime>().elapsed_years = 125.0;
    let cam_ent = app
        .world_mut()
        .spawn((Transform::default(), PanOrbitCamera::default()))
        .id();

    let star = spawn_selectable_body(
        app.world_mut(),
        "Sol",
        BodyType::MainSequenceStar,
        1.0,
        0.0,
        true,
    );
    let planet = spawn_selectable_body(
        app.world_mut(),
        "Gaia",
        BodyType::TerrestrialPlanet,
        EARTH_MASS_SOLAR,
        1.0,
        false,
    );
    let rock = spawn_selectable_body(
        app.world_mut(),
        "Dust",
        BodyType::Asteroid,
        0.001 * EARTH_MASS_SOLAR,
        2.0,
        false,
    );
    app.world_mut().entity_mut(planet).insert(BiosphereState {
        habitability_score: 0.9,
        biomass_coverage_frac: 0.45,
        oxygen_fraction: 0.21,
        emergence_year: Some(125.0),
        ..default()
    });

    emit_track3_test_events(app.world_mut(), star, planet, rock);

    app.update();

    let log = app.world().resource::<CosmicEventLog>();
    assert_eq!(log.entries.len(), 4, "Tiny dust merge must be filtered out");
    assert_eq!(log.unread_count, 4);

    let (badge_txt, badge_color) = {
        let world = app.world_mut();
        let mut q = world.query::<(&Text, &TextColor, &HudDynamicText)>();
        q.iter(world)
            .find(|(_, _, d)| matches!(d, HudDynamicText::EventFeedBadge))
            .map(|(t, c, _)| (t.0.clone(), c.0))
            .unwrap()
    };
    assert_eq!(badge_txt, "📡 Events (4) [Shift-E]");
    assert_eq!(badge_color, Color::srgb(1.0, 0.85, 0.35));

    app.world_mut()
        .resource_mut::<HudVisibilityState>()
        .event_feed_minimized = false;
    app.update();

    assert_eq!(app.world().resource::<CosmicEventLog>().unread_count, 0);
    let rev_before = app.world().resource::<CosmicEventLog>().revision;
    let rows_before: Vec<Entity> = {
        let world = app.world_mut();
        let list_ent = world
            .query_filtered::<Entity, With<EventFeedListContainer>>()
            .single(world)
            .unwrap();
        world.get::<Children>(list_ent).unwrap().iter().collect()
    };
    assert_eq!(rows_before.len(), 4);

    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<CosmicEventLog>().revision,
        rev_before
    );
    let rows_after: Vec<Entity> = {
        let world = app.world_mut();
        let list_ent = world
            .query_filtered::<Entity, With<EventFeedListContainer>>()
            .single(world)
            .unwrap();
        world.get::<Children>(list_ent).unwrap().iter().collect()
    };
    assert_eq!(
        rows_after, rows_before,
        "Event feed rows must have zero rebuild churn"
    );

    trigger_hud_action(&mut app, UiButtonAction::JumpToEvent(Some(planet)));
    assert_eq!(
        app.world()
            .resource::<PlayerInteractionState>()
            .selected_entity,
        Some(planet)
    );
    assert_eq!(
        app.world()
            .get::<PanOrbitCamera>(cam_ent)
            .unwrap()
            .target_entity,
        Some(planet)
    );

    app.world_mut().despawn(planet);
    trigger_hud_action(&mut app, UiButtonAction::JumpToEvent(Some(planet)));
    assert_eq!(
        app.world()
            .resource::<PlayerInteractionState>()
            .selected_entity,
        Some(star),
        "JumpToEvent for despawned target must fall back to CentralStar"
    );
    assert_eq!(
        app.world()
            .get::<PanOrbitCamera>(cam_ent)
            .unwrap()
            .target_entity,
        Some(star),
        "JumpToEvent fallback must also refocus PanOrbitCamera onto CentralStar"
    );
}

#[test]
fn test_track3_ui_systems_initialize_without_query_conflicts() {
    let mut app = build_track3_base_app();
    app.add_systems(
        Update,
        (
            handle_ui_button_interactions,
            update_quick_body_selector_bar,
            handle_inspector_scroll,
            handle_cosmic_event_toasts,
            detect_biosphere_emergence_events,
            update_cosmic_event_feed_ui,
            update_hud_visibility,
            upgrade_hud_default_fonts,
            apply_bundled_hud_font_when_loaded,
        ),
    );
    spawn_selectable_body(
        app.world_mut(),
        "Sol",
        BodyType::MainSequenceStar,
        1.0,
        0.0,
        true,
    );
    app.update();
    app.update();
    app.update();
}
