//! Integration tests for Bevy 0.20.1 UI widgets, typography fallback, text inputs,
//! continuous sliders, clipboard seed serialization, and UiTransform animations.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::math::{DVec3, Rot2};
use bevy::prelude::*;
use bevy::text::{EditableText, FontSource};
use bevy::ui::UiTransform;
use bevy::ui_widgets::{SliderRange, SliderValue};

use protostellar::game::ui::types::PlanetBuilderState;
use protostellar::game::ui::widget_sync::{
    animate_hud_ui_transforms, apply_hud_slider_changes, sync_hud_sliders, sync_hud_text_inputs,
    HoverReticleBracket, SurfaceCompassNeedle,
};
use protostellar::game::ui::widgets::{
    hud_font_source, is_text_input_focused, spawn_hud_slider, upgrade_hud_default_fonts, HudSlider,
    HudSliderKind, HudTextInput, HudTextInputKind, PendingSliderChanges,
};
use protostellar::simulation::components::celestial::SpinState;
use protostellar::simulation::components::*;
use protostellar::simulation::geology::types::TimelineScrubber;
use protostellar::simulation::resources::*;
use protostellar::simulation::serialization::*;
use protostellar::utils::constants::EARTH_MASS_SOLAR;

#[test]
fn test_hud_font_source_fallback_and_upgrade() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Font>()
        .add_systems(Update, upgrade_hud_default_fonts);

    let default_text = app
        .world_mut()
        .spawn(TextFont {
            font_size: FontSize::Px(12.0),
            ..default()
        })
        .id();

    let custom_text = app
        .world_mut()
        .spawn(TextFont {
            font: FontSource::family("Fira Sans"),
            font_size: FontSize::Px(12.0),
            ..default()
        })
        .id();

    app.update();

    let world = app.world();
    let default_font = world.get::<TextFont>(default_text).unwrap();
    let custom_font = world.get::<TextFont>(custom_text).unwrap();

    // Default font handle must be upgraded to universal fallback list
    assert_eq!(default_font.font, hud_font_source());

    // Explicit custom fonts must be preserved without alteration
    assert_eq!(custom_font.font, FontSource::family("Fira Sans"));
}

#[test]
fn test_hud_font_source_resolution_without_missing_glyphs() {
    let source = hud_font_source();
    let fonts = Assets::<Font>::default();
    let resolved = source.resolve_font_family(&fonts);
    assert!(
        resolved.is_ok(),
        "hud_font_source must resolve through Parley font context: {resolved:?}"
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(bevy::text::TextPlugin);

    let mut font_cx = app
        .world_mut()
        .remove_resource::<bevy::text::FontCx>()
        .unwrap();
    let names: Vec<_> = font_cx.collection.family_names().take(10).collect();
    assert!(
        !names.is_empty(),
        "Fontique must discover system font families on macOS"
    );
}

#[test]
fn test_is_text_input_focused_helper() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let input_entity = app
        .world_mut()
        .spawn((
            EditableText::new("Earth"),
            HudTextInput(HudTextInputKind::InspectorBodyName),
        ))
        .id();

    let other_entity = app.world_mut().spawn(Node::default()).id();

    let mut focus = InputFocus::default();
    let mut query = app.world_mut().query_filtered::<(), With<EditableText>>();

    // No entity focused
    assert!(!is_text_input_focused(
        Some(&focus),
        &query.query(app.world())
    ));

    // Non-text-input entity focused
    focus.set(other_entity, FocusCause::Navigated);
    assert!(!is_text_input_focused(
        Some(&focus),
        &query.query(app.world())
    ));

    // Editable text input focused
    focus.set(input_entity, FocusCause::Navigated);
    assert!(is_text_input_focused(
        Some(&focus),
        &query.query(app.world())
    ));
}

#[test]
fn test_hud_slider_spawning_and_value_ranges() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let mut slider_entity = Entity::PLACEHOLDER;
    let mut commands = app.world_mut().commands();
    commands.spawn(Node::default()).with_children(|parent| {
        slider_entity = spawn_hud_slider(
            parent,
            HudSliderKind::BuilderLogMassEarth,
            0.0,
            "Mass:",
            160.0,
        );
    });

    app.update();

    let mut found_slider = false;
    for (slider, val, range) in app
        .world_mut()
        .query::<(&HudSlider, &SliderValue, &SliderRange)>()
        .iter(app.world())
    {
        if slider.0 == HudSliderKind::BuilderLogMassEarth {
            found_slider = true;
            assert_eq!(val.0, 0.0);
            assert_eq!(range.clamp(10.0), 5.0);
            assert_eq!(range.clamp(-5.0), -4.0);
        }
    }
    assert!(
        found_slider,
        "Spawned slider component was not found in hierarchy: {slider_entity:?}"
    );
}

#[test]
fn test_hud_slider_sync_and_model_application() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<PendingSliderChanges>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<PlanetBuilderState>()
        .init_resource::<TimelineScrubber>()
        .add_systems(Update, (sync_hud_sliders, apply_hud_slider_changes));

    let _star = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Sol".to_string(),
                body_type: BodyType::MainSequenceStar,
            },
            CentralStar,
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Mass(1.0),
            Radius(1.0),
        ))
        .id();

    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Proto-Terra".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 6.28)),
            Mass(EARTH_MASS_SOLAR),
            Radius(0.01),
            SpinState {
                axial_tilt_degrees: 23.4,
                ..default()
            },
        ))
        .id();

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(planet);

    // Apply Mass slider change: 2.0 log Earth mass (100 M_earth)
    app.world_mut()
        .resource_mut::<PendingSliderChanges>()
        .0
        .push((HudSliderKind::InspectorLogMassEarth, 2.0));

    // Apply Orbit slider change: 2.0 AU
    app.world_mut()
        .resource_mut::<PendingSliderChanges>()
        .0
        .push((HudSliderKind::InspectorOrbitAu, 2.0));

    // Apply Tilt slider change: 45.0 degrees
    app.world_mut()
        .resource_mut::<PendingSliderChanges>()
        .0
        .push((HudSliderKind::InspectorAxialTiltDeg, 45.0));

    // Apply Timeline scrub change: 3.5 Gyr
    app.world_mut()
        .resource_mut::<PendingSliderChanges>()
        .0
        .push((HudSliderKind::EpochAgeGyr, 3.5));

    app.update();

    let world = app.world();
    let mass = world.get::<Mass>(planet).unwrap();
    let pos = world.get::<SimPosition>(planet).unwrap();
    let spin = world.get::<SpinState>(planet).unwrap();
    let timeline = world.resource::<TimelineScrubber>();

    // 100 Earth masses = 100 * EARTH_MASS_SOLAR
    let expected_mass = 100.0 * EARTH_MASS_SOLAR;
    assert!((mass.0 - expected_mass).abs() < 1e-6);

    // Orbit moved out to 2.0 AU
    assert!((pos.0.length() - 2.0).abs() < 1e-4);

    // Tilt updated to 45 degrees
    assert!((spin.axial_tilt_degrees - 45.0).abs() < 1e-2);

    // Timeline scrubbed age updated
    assert_eq!(timeline.scrubbed_age_gyr, 3.5);
}

#[test]
fn test_hud_text_input_two_way_binding() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<PlayerInteractionState>()
        .init_resource::<PlanetBuilderState>()
        .add_systems(Update, sync_hud_text_inputs);

    let planet = app
        .world_mut()
        .spawn(CelestialBody {
            name: "Initial Earth".to_string(),
            body_type: BodyType::TerrestrialPlanet,
        })
        .id();

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(planet);

    let input_entity = app
        .world_mut()
        .spawn((
            EditableText::new("Initial Earth"),
            HudTextInput(HudTextInputKind::InspectorBodyName),
        ))
        .id();

    app.update();

    // 1. Unfocused sync: external body rename updates the input's EditableText
    app.world_mut()
        .get_mut::<CelestialBody>(planet)
        .unwrap()
        .name = "Terra Nova".to_string();
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(input_entity)
            .unwrap()
            .value(),
        "Terra Nova"
    );

    // 2. Focused sync: player focuses the input and types a new name, updating the model
    app.init_resource::<InputFocus>();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(input_entity, FocusCause::Navigated);
    *app.world_mut()
        .get_mut::<EditableText>(input_entity)
        .unwrap() = EditableText::new("Gaia Prime");

    app.update();

    let world = app.world();
    let body = world.get::<CelestialBody>(planet).unwrap();
    assert_eq!(body.name, "Gaia Prime");
}

#[test]
fn test_seed_json_encode_decode_roundtrip() {
    let bodies = vec![
        CelestialBodySave {
            name: "Proto-Sun".to_string(),
            body_type: BodyType::MainSequenceStar,
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
            mass: 1.0,
            radius: 0.005,
            temperature: 5778.0,
            luminosity: 1.0,
            composition: Composition::default(),
            spin: SpinState::default(),
            differentiation: None,
            volatile_inventory: None,
            ring_system: None,
            basins: None,
            climate: None,
            biosphere: None,
            electromagnetic: None,
            is_central_star: true,
            ignition_state: None,
            stellar_evolution: None,
            black_hole_state: None,
            satellite: None,
            tidal_state: None,
            relativistic_state: None,
            atmospheric_escape: None,
            kozai_lidov: None,
        },
        CelestialBodySave {
            name: "Verdant-1".to_string(),
            body_type: BodyType::TerrestrialPlanet,
            position: DVec3::new(1.0, 0.0, 0.0),
            velocity: DVec3::new(0.0, 0.0, 6.28),
            mass: 3.0e-6,
            radius: 0.0001,
            temperature: 288.0,
            luminosity: 0.0,
            composition: Composition::default(),
            spin: SpinState::default(),
            differentiation: None,
            volatile_inventory: None,
            ring_system: None,
            basins: None,
            climate: None,
            biosphere: None,
            electromagnetic: None,
            is_central_star: false,
            ignition_state: None,
            stellar_evolution: None,
            black_hole_state: None,
            satellite: None,
            tidal_state: None,
            relativistic_state: None,
            atmospheric_escape: None,
            kozai_lidov: None,
        },
    ];

    let save_data = SystemSaveData {
        version: 1,
        timestamp_epoch_yr: 42.0,
        step_count: 1000,
        time_warp: TimeWarpSave::default(),
        disk_parameters: DiskParameters::default(),
        config_save: ConfigSave::default(),
        bodies,
        event_flags: EventFlagsSave::default(),
        scenario_preset: None,
    };

    let encoded_seed = encode_system_seed_json(&save_data).unwrap();
    assert!(
        encoded_seed.starts_with(SEED_PREFIX),
        "Encoded seed must begin with standard seed prefix"
    );

    let decoded_result = decode_system_seed_json(&encoded_seed);
    assert!(
        decoded_result.is_ok(),
        "Decoded seed must parse successfully: {decoded_result:?}"
    );

    let decoded = decoded_result.unwrap();
    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.bodies.len(), 2);
    assert_eq!(decoded.bodies[0].name, "Proto-Sun");
    assert_eq!(decoded.bodies[1].name, "Verdant-1");
    assert_eq!(decoded.bodies[1].position, DVec3::new(1.0, 0.0, 0.0));
}

#[test]
fn test_ui_transform_smooth_rotation_animation() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .add_systems(Update, animate_hud_ui_transforms);

    let reticle = app
        .world_mut()
        .spawn((HoverReticleBracket, UiTransform::default()))
        .id();

    let compass = app
        .world_mut()
        .spawn((SurfaceCompassNeedle, UiTransform::default()))
        .id();

    let _camera = app
        .world_mut()
        .spawn(protostellar::rendering::camera::PanOrbitCamera {
            surface_horizon_mode: true,
            surface_azimuth: 0.785,
            ..default()
        })
        .id();

    // Initial state
    assert_eq!(
        app.world().get::<UiTransform>(reticle).unwrap().rotation,
        Rot2::IDENTITY
    );

    // Advance time by 1.0 second
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));

    app.update();

    let world = app.world();
    let reticle_transform = world.get::<UiTransform>(reticle).unwrap();
    let compass_transform = world.get::<UiTransform>(compass).unwrap();

    assert_ne!(
        reticle_transform.rotation,
        Rot2::IDENTITY,
        "Reticle bracket must rotate over time"
    );
    assert_ne!(
        compass_transform.rotation,
        Rot2::IDENTITY,
        "Surface compass needle must rotate over time"
    );
}
