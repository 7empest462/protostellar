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
    HoverReticleBracket, SurfaceCompassNeedle, animate_hud_ui_transforms, apply_hud_slider_changes,
    sync_hud_sliders, sync_hud_text_inputs,
};
use protostellar::game::ui::widgets::{
    HudSlider, HudSliderKind, HudTextInput, HudTextInputKind, PendingSliderChanges,
    hud_font_source, is_text_input_focused, spawn_hud_slider, upgrade_hud_default_fonts,
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
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
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
            ..Default::default()
        },
        CelestialBodySave {
            name: "Verdant-1".to_string(),
            body_type: BodyType::TerrestrialPlanet,
            position: DVec3::new(1.0, 0.0, 0.0),
            velocity: DVec3::new(0.0, 0.0, std::f64::consts::TAU),
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
            ..Default::default()
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

#[test]
fn test_hud_slider_self_update_fill_and_derived_mass_sync() {
    use bevy::ui_widgets::{SliderDragState, ValueChange, slider_self_update};
    use protostellar::game::ui::widgets::{
        HudSliderFill, HudSliderThumb, on_hud_slider_value_change,
    };
    use protostellar::utils::constants::EARTH_RADIUS_AU;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<PendingSliderChanges>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<PlanetBuilderState>()
        .init_resource::<protostellar::simulation::geology::types::TimelineScrubber>()
        .add_observer(slider_self_update)
        .add_observer(on_hud_slider_value_change)
        .add_systems(Update, (apply_hud_slider_changes, sync_hud_sliders).chain());

    let comp = Composition::rocky();
    let mut init_diff = InternalDifferentiation::default();
    init_diff.recalculate(EARTH_MASS_SOLAR, EARTH_RADIUS_AU, &comp);

    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            comp,
            init_diff,
        ))
        .id();

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(planet);

    let mut root_entity = Entity::PLACEHOLDER;
    {
        let mut commands = app.world_mut().commands();
        commands.spawn(Node::default()).with_children(|parent| {
            root_entity = spawn_hud_slider(
                parent,
                HudSliderKind::InspectorLogMassEarth,
                0.0,
                "Mass:",
                180.0,
            );
        });
    }
    app.update();

    let (slider_ent, _) = app
        .world_mut()
        .query::<(Entity, &HudSlider)>()
        .iter(app.world())
        .next()
        .expect("Spawned InspectorLogMassEarth slider must exist");
    let _ = root_entity;

    // Simulate active thumb drag + ValueChange (setting log10 mass = 1.0 => 10 M_earth)
    app.world_mut()
        .get_mut::<SliderDragState>(slider_ent)
        .unwrap()
        .dragging = true;
    app.world_mut().trigger(ValueChange::<f32> {
        source: slider_ent,
        value: 1.0_f32,
        is_final: false,
    });
    app.update();

    // 1. slider_self_update must update SliderValue immediately even while dragging
    let val = app.world().get::<SliderValue>(slider_ent).unwrap().0;
    assert!(
        (val - 1.0).abs() < 1e-4,
        "SliderValue must follow drag: {val}"
    );

    // 2. Nested HudSliderFill width & HudSliderThumb left must track SliderValue in the same frame
    let fill_pct = match app
        .world_mut()
        .query_filtered::<&Node, With<HudSliderFill>>()
        .iter(app.world())
        .next()
        .unwrap()
        .width
    {
        Val::Percent(p) => p,
        other => panic!("Expected Val::Percent on HudSliderFill, got {other:?}"),
    };
    let thumb_pct = match app
        .world_mut()
        .query_filtered::<&Node, With<HudSliderThumb>>()
        .iter(app.world())
        .next()
        .unwrap()
        .left
    {
        Val::Percent(p) => p,
        other => panic!("Expected Val::Percent on HudSliderThumb, got {other:?}"),
    };
    let expected_pct = SliderRange::new(
        HudSliderKind::InspectorLogMassEarth.min(),
        HudSliderKind::InspectorLogMassEarth.max(),
    )
    .thumb_position(1.0)
        * 100.0;
    assert!(
        (fill_pct - expected_pct).abs() < 0.5 && (thumb_pct - expected_pct).abs() < 0.5,
        "Fill ({fill_pct}) and thumb ({thumb_pct}) must match {expected_pct}%"
    );

    // 3. Derived planetary Mass, Radius, BodyType, and InternalDifferentiation must update together
    let world = app.world();
    let new_mass = world.get::<Mass>(planet).unwrap().0;
    let new_radius = world.get::<Radius>(planet).unwrap().0;
    let new_body = world.get::<CelestialBody>(planet).unwrap();
    let new_diff = world.get::<InternalDifferentiation>(planet).unwrap();

    assert!((new_mass - 10.0 * EARTH_MASS_SOLAR).abs() < 1e-6);
    assert!(
        new_radius > EARTH_RADIUS_AU * 1.5,
        "10 M_earth rocky world radius must scale up from 1 M_earth: {new_radius}"
    );
    assert_eq!(new_body.body_type, BodyType::SuperEarth);
    assert!(new_diff.core_radius_au > EARTH_RADIUS_AU * 0.5);
}

#[test]
fn test_cosmic_event_log_bounding_and_event_toasts() {
    use protostellar::game::ui::{CosmicEventLog, NotificationToast, handle_cosmic_event_toasts};
    use protostellar::simulation::accretion::{MoonFormationEvent, TidalDisruptionEvent};
    use protostellar::simulation::atmosphere_escape::AtmosphericStrippedEvent;
    use protostellar::simulation::kozai_lidov::KozaiDisruptionEvent;
    use protostellar::simulation::relativity::GravitationalWaveMergerEvent;
    use protostellar::simulation::resources::SimTime;
    use protostellar::simulation::space_weather::CmeShockwaveEvent;

    let mut bounded = CosmicEventLog::default();
    for i in 0..40 {
        bounded.push(f64::from(i), "⚡", format!("Event {i}"), None);
    }
    assert_eq!(bounded.entries.len(), CosmicEventLog::MAX_ENTRIES);
    assert_eq!(
        bounded.entries.back().map(|e| e.headline.as_str()),
        Some("Event 39")
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimTime>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<CosmicEventLog>();
    app.add_message::<CmeShockwaveEvent>();
    app.add_message::<KozaiDisruptionEvent>();
    app.add_message::<AtmosphericStrippedEvent>();
    app.add_message::<GravitationalWaveMergerEvent>();
    app.add_message::<SupernovaEvent>();
    app.add_message::<TidalDisruptionEvent>();
    app.add_message::<MoonFormationEvent>();
    app.add_systems(Update, handle_cosmic_event_toasts);

    let target = app
        .world_mut()
        .spawn(CelestialBody {
            body_type: BodyType::TerrestrialPlanet,
            name: "Terra".to_string(),
        })
        .id();

    app.world_mut()
        .resource_mut::<Messages<CmeShockwaveEvent>>()
        .write(CmeShockwaveEvent {
            target,
            orbital_radius_au: 1.0,
            density_multiplier: 8.0,
            flare_intensity: 5.5,
        });
    app.world_mut()
        .resource_mut::<Messages<SupernovaEvent>>()
        .write(SupernovaEvent {
            star_entity: target,
            star_name: "Betelgeuse".to_string(),
            initial_mass_solar: 18.0,
            remnant_mass_solar: 1.6,
            remnant_type: BodyType::NeutronStar,
            shockwave_velocity_km_s: 9500.0,
            ejected_metals_solar: 2.1,
            ejected_composition: Composition::rocky(),
        });
    app.world_mut()
        .resource_mut::<Messages<MoonFormationEvent>>()
        .write(MoonFormationEvent {
            parent_entity: target,
            moon_entity: target,
            moon_mass: 1e-8,
            orbital_radius_au: 0.0025,
            orbital_period_years: 0.075,
        });

    app.update();

    let log = app.world().resource::<CosmicEventLog>();
    assert_eq!(log.entries.len(), 3);
    assert!(app.world().resource::<NotificationToast>().timer >= 5.0);
}

#[test]
fn test_physical_telemetry_milestone_and_trophy_unlocks() {
    use protostellar::game::audio::{CosmicAudioMode, compute_body_sonification_snapshot};
    use protostellar::game::phases::{
        GlobalTrophies, LateHeavyBombardmentState, MilestoneId, PhaseManager, SystemPhase,
        monitor_phase_transitions,
    };
    use protostellar::game::ui::telemetry_format::append_sonification_telemetry;
    use protostellar::simulation::neutron_stars::types::MagnetarCrustStress;
    use protostellar::simulation::relativity::RelativisticState;
    use protostellar::simulation::space_weather::{AuroralOvalState, GeomagneticStormLevel};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.init_resource::<PhaseManager>();
    app.init_resource::<GlobalTrophies>();
    app.add_plugins(protostellar::simulation::SimulationPlugin);
    app.add_systems(Update, monitor_phase_transitions);

    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::Magnetar,
            name: "SGR-Test".to_string(),
        },
        Mass(1.8),
        SimPosition(DVec3::new(0.5, 0.0, 0.0)),
        MagnetarCrustStress {
            burst_active: true,
            total_starquakes_count: 1,
            ..Default::default()
        },
    ));
    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::TerrestrialPlanet,
            name: "Aurora-World".to_string(),
        },
        Mass(3e-6),
        SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        AuroralOvalState {
            geomagnetic_kp_index: 8.2,
            auroral_intensity: 0.92,
            storm_level: GeomagneticStormLevel::SevereStorm,
            ..Default::default()
        },
    ));
    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::BlackHole,
            name: "GW-Binary".to_string(),
        },
        Mass(30.0),
        SimPosition(DVec3::new(0.02, 0.0, 0.0)),
        RelativisticState {
            is_coalescing: true,
            gw_frequency_hz: 150.0,
            gw_strain: 1e-21,
            gw_luminosity_watts: 1e42,
            ..Default::default()
        },
    ));

    // Advance one frame at t = 0.0 yr to confirm unlocks happen from physical state without timers
    app.update();

    let trophies = app.world().resource::<GlobalTrophies>();
    assert!(trophies.unlocked.contains(&MilestoneId::MagnetarSuperflare));
    assert!(
        trophies
            .unlocked
            .contains(&MilestoneId::GeomagneticSuperstorm)
    );
    assert!(trophies.unlocked.contains(&MilestoneId::BinaryInspiral));

    let body = CelestialBody {
        body_type: BodyType::TerrestrialPlanet,
        name: "Aurora-World".to_string(),
    };
    let aurora = AuroralOvalState {
        geomagnetic_kp_index: 8.2,
        auroral_intensity: 0.92,
        storm_level: GeomagneticStormLevel::SevereStorm,
        ..Default::default()
    };
    let snap =
        compute_body_sonification_snapshot(&body, None, None, None, Some(&aurora), 0.5, true);
    assert_eq!(snap.mode, CosmicAudioMode::MagnetosphericChorus);
    let mut readout = String::new();
    append_sonification_telemetry(&mut readout, &snap);
    assert!(readout.contains("COSMIC SONIFICATION AUDIO"));
    assert!(readout.contains("MAGNETOSPHERIC CHORUS"));
}

#[test]
fn test_audio_mute_button_visible_by_default_and_toggles_sound() {
    use protostellar::game::audio::CosmicAudioState;
    use protostellar::game::phases::{LateHeavyBombardmentState, SystemPhase};
    use protostellar::game::ui::{
        ActivatedHudButtons, HudDynamicText, HudPanelElement, HudVisibilityState,
        NotificationToast, PlanetBuilderState, QuickBarState, TelemetryPanelState, UiButtonAction,
        handle_ui_button_interactions, setup_hud, update_hud_visibility,
    };
    use protostellar::simulation::telemetry::SimulationTelemetryHistory;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ActivatedHudButtons>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.init_resource::<QuickBarState>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<TelemetryPanelState>();
    app.init_resource::<protostellar::game::ui::types::TrophiesPanelState>();
    app.init_resource::<HudVisibilityState>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<CosmicAudioState>();
    app.init_resource::<SimulationTelemetryHistory>();
    app.add_plugins(protostellar::simulation::SimulationPlugin);
    app.add_systems(Startup, setup_hud);
    app.add_systems(
        Update,
        (handle_ui_button_interactions, update_hud_visibility).chain(),
    );
    app.update();

    assert!(
        !app.world().resource::<CosmicAudioState>().enabled,
        "Sound must default to muted"
    );

    let mut btn_query = app
        .world_mut()
        .query::<(Entity, &UiButtonAction, &HudPanelElement, &Node)>();
    let (mute_btn, _, _, mute_node) = btn_query
        .iter(app.world())
        .find(|(_, action, _, _)| matches!(action, UiButtonAction::ToggleAudioMute))
        .expect("Top-right Sound mute button must be spawned in HUD");
    assert_eq!(mute_node.display, Display::Flex);
    assert!(
        UiButtonAction::ToggleAudioMute
            .tooltip_description()
            .contains("Mute or unmute"),
        "Mute button must provide a clear tooltip"
    );

    let mut text_query = app.world_mut().query::<(&Text, &HudDynamicText)>();
    let initial_label = text_query
        .iter(app.world())
        .find_map(|(t, kind)| matches!(kind, HudDynamicText::AudioMuteBadge).then(|| t.0.clone()))
        .expect("AudioMuteBadge dynamic text must exist");
    assert_eq!(initial_label, "🔇 Sound: OFF [M]");

    // Simulate clicking the HUD Sound toggle button
    app.world_mut()
        .entity_mut(mute_btn)
        .insert(bevy::ui::Pressed);
    app.world_mut()
        .resource_mut::<ActivatedHudButtons>()
        .0
        .push(mute_btn);
    app.update();

    assert!(
        app.world().resource::<CosmicAudioState>().enabled,
        "Clicking the Sound button must unmute audio"
    );
    assert!(
        app.world()
            .resource::<NotificationToast>()
            .message
            .contains("Unmuted")
    );
    let unmuted_label = app
        .world_mut()
        .query::<(&Text, &HudDynamicText)>()
        .iter(app.world())
        .find_map(|(t, kind)| matches!(kind, HudDynamicText::AudioMuteBadge).then(|| t.0.clone()))
        .expect("AudioMuteBadge label after unmute");
    assert_eq!(unmuted_label, "🔊 Sound: ON [M]");

    // Clicking again re-mutes
    app.world_mut()
        .entity_mut(mute_btn)
        .insert(bevy::ui::Pressed);
    app.world_mut()
        .resource_mut::<ActivatedHudButtons>()
        .0
        .push(mute_btn);
    app.update();
    assert!(!app.world().resource::<CosmicAudioState>().enabled);
    let remuted_label = app
        .world_mut()
        .query::<(&Text, &HudDynamicText)>()
        .iter(app.world())
        .find_map(|(t, kind)| matches!(kind, HudDynamicText::AudioMuteBadge).then(|| t.0.clone()))
        .expect("AudioMuteBadge label after re-mute");
    assert_eq!(remuted_label, "🔇 Sound: OFF [M]");
}
