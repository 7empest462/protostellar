//! Modern HUD Widgets for Bevy 0.20.1.
//!
//! Provides first-class `Parley` + `Fontique` Unicode/scientific typography (`H₂O`, `CO₂`, `W/m²`,
//! `M☉`, `M⊕`, `θ`, `Γ`, `±`), editable text inputs (`TextInput`), continuous sliders (`Slider`),
//! clipboard seed exchange, and `UiTransform` / gradient primitives.

use bevy::input_focus::InputFocus;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::{EditableText, FontSize, FontSource, TextCursorStyle};
use bevy::ui::{BackgroundGradient, ColorStop, Gradient, LinearGradient, PositionType};
use bevy::ui_widgets::{
    Activate, Slider, SliderDragState, SliderOrientation, SliderPrecision, SliderRange, SliderStep,
    SliderValue, TextInput, TrackClick, ValueChange, slider_self_update,
};

/// Holds the optional bundled geometric sci-fi HUD font handle (`assets/fonts/ChakraPetch-Medium.ttf`).
#[derive(Resource, Debug, Clone)]
pub struct BundledHudFont {
    pub handle: Handle<Font>,
    pub upgraded_existing: bool,
}

/// Returns the universal HUD `FontSource::List` ensuring full fallback coverage across
/// geometric sans, monospace, system UI, mathematical symbols, and emojis without missing-glyph boxes (`▯`).
pub fn hud_font_source() -> FontSource {
    FontSource::List(vec![
        FontSource::families(
            "'Chakra Petch', 'Space Grotesk', 'Avenir Next', 'SF Pro Text', 'Menlo', 'Monaco', 'Consolas', 'Segoe UI', 'Apple Color Emoji', 'Segoe UI Emoji', 'Noto Color Emoji', sans-serif",
        ),
        FontSource::sans_serif(),
        FontSource::monospace(),
        FontSource::system_ui(),
        FontSource::emoji(),
        FontSource::math(),
    ])
}

/// Returns `FontSource` prepending a loaded bundled HUD font handle ahead of [`hud_font_source`]
/// when available, preserving emoji and mathematical symbol fallbacks.
pub fn hud_font_source_with_bundled(bundled: Option<Handle<Font>>) -> FontSource {
    if let Some(handle) = bundled {
        FontSource::List(vec![FontSource::Handle(handle), hud_font_source()])
    } else {
        hud_font_source()
    }
}

/// Loads `fonts/ChakraPetch-Medium.ttf` via Bevy's `AssetServer` when asset loading is active.
pub fn setup_bundled_hud_font(mut commands: Commands, asset_server: Option<Res<AssetServer>>) {
    if let Some(server) = asset_server {
        let handle = server.load("fonts/ChakraPetch-Medium.ttf");
        commands.insert_resource(BundledHudFont {
            handle,
            upgraded_existing: false,
        });
    }
}

/// Helper creating a standard `TextFont` with the universal HUD font fallback list and given pixel size.
pub fn hud_text_font(font_size_px: f32) -> TextFont {
    TextFont {
        font: hud_font_source(),
        font_size: FontSize::Px(font_size_px),
        ..default()
    }
}

/// Upgrades all HUD text nodes created with Bevy's default font handle to the bundled HUD font
/// (once loaded) or [`hud_font_source`], preserving any explicit custom fonts.
pub fn upgrade_hud_default_fonts(
    mut query: Query<&mut TextFont, Added<TextFont>>,
    bundled: Option<Res<BundledHudFont>>,
    fonts: Option<Res<Assets<Font>>>,
) {
    if query.is_empty() {
        return;
    }
    let loaded_handle = fonts.as_ref().and_then(|font_assets| {
        bundled
            .as_ref()
            .and_then(|b| font_assets.get(&b.handle).map(|_| b.handle.clone()))
    });
    let base = hud_font_source();
    let target = hud_font_source_with_bundled(loaded_handle.clone());

    for mut font in query.iter_mut() {
        let is_default_handle =
            matches!(font.font, FontSource::Handle(ref h) if *h == Handle::default());
        let is_base_upgrade = loaded_handle.is_some() && font.font == base;
        if is_default_handle || is_base_upgrade {
            font.font = target.clone();
        }
    }
}

/// One-shot pass upgrading existing default HUD `TextFont`s once the bundled font asset finishes loading.
pub fn apply_bundled_hud_font_when_loaded(
    mut bundled: Option<ResMut<BundledHudFont>>,
    fonts: Option<Res<Assets<Font>>>,
    mut all_fonts: Query<&mut TextFont>,
) {
    let Some(ref mut bundled_res) = bundled else {
        return;
    };
    if bundled_res.upgraded_existing {
        return;
    }
    let Some(font_assets) = fonts.as_deref() else {
        return;
    };
    if font_assets.get(&bundled_res.handle).is_none() {
        return;
    }

    let base = hud_font_source();
    let target = hud_font_source_with_bundled(Some(bundled_res.handle.clone()));
    for mut font in all_fonts.iter_mut() {
        let is_default_handle =
            matches!(font.font, FontSource::Handle(ref h) if *h == Handle::default());
        if is_default_handle || font.font == base {
            font.font = target.clone();
        }
    }
    bundled_res.upgraded_existing = true;
}

/// Helper function to check if any editable text input currently holds UI keyboard focus.
/// Used across camera and simulation systems to suppress hotkey handling while typing.
pub fn is_text_input_focused(
    focus: Option<&InputFocus>,
    editable_query: &Query<(), With<EditableText>>,
) -> bool {
    focus
        .and_then(InputFocus::get)
        .is_some_and(|focused_ent| editable_query.contains(focused_ent))
}

/// Resource accumulating UI button activation events observed via `On<Activate>`.
#[derive(Resource, Default, Debug)]
pub struct ActivatedHudButtons(pub Vec<Entity>);

/// Resource accumulating slider value changes observed via `On<ValueChange<f32>>`.
#[derive(Resource, Default, Debug)]
pub struct PendingSliderChanges(pub Vec<(HudSliderKind, f32)>);

/// Active named save slot state for UI persistence and file management.
#[derive(Resource, Debug, Clone)]
pub struct SaveSlotUiState {
    pub slot_name: String,
}

impl Default for SaveSlotUiState {
    fn default() -> Self {
        Self {
            slot_name: "quicksave".to_string(),
        }
    }
}

/// Global observer capturing button activation events into `ActivatedHudButtons`.
pub fn on_hud_button_activate(
    trigger: On<Activate>,
    mut activated: Option<ResMut<ActivatedHudButtons>>,
) {
    if let Some(ref mut act) = activated {
        act.0.push(trigger.entity);
    }
}

/// Global observer capturing slider value changes into `PendingSliderChanges`.
pub fn on_hud_slider_value_change(
    trigger: On<ValueChange<f32>>,
    slider_query: Query<&HudSlider>,
    mut pending: Option<ResMut<PendingSliderChanges>>,
) {
    if let Some(ref mut pend) = pending
        && let Ok(hud_slider) = slider_query.get(trigger.source)
    {
        if let Some(existing) = pend.0.iter_mut().find(|(k, _)| *k == hud_slider.0) {
            existing.1 = trigger.value;
        } else {
            pend.0.push((hud_slider.0, trigger.value));
        }
    }
}

/// Supported kinds of HUD sliders in Protostellar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum HudSliderKind {
    /// Planet builder log10 mass in Earth masses (-4.0 to 5.0).
    BuilderLogMassEarth,
    /// Planet builder orbital radius in AU (0.10 to 50.0).
    BuilderOrbitAu,
    /// Planet builder orbital eccentricity (0.00 to 1.30).
    BuilderEccentricity,
    /// Inspector target body log10 mass in Earth masses (-4.0 to 8.0, up to ~300 M☉).
    InspectorLogMassEarth,
    /// Inspector target body orbital radius in AU (0.05 to 60.0).
    InspectorOrbitAu,
    /// Inspector target body axial tilt in degrees (0.0 to 180.0).
    InspectorAxialTiltDeg,
    /// Deep-time timeline scrubber age in billions of years (0.00 to 5.50 Ga).
    EpochAgeGyr,
}

impl HudSliderKind {
    pub fn min(&self) -> f32 {
        match self {
            Self::BuilderLogMassEarth | Self::InspectorLogMassEarth => -4.0,
            Self::BuilderOrbitAu => 0.10,
            Self::InspectorOrbitAu => 0.05,
            Self::BuilderEccentricity | Self::InspectorAxialTiltDeg | Self::EpochAgeGyr => 0.0,
        }
    }

    pub fn max(&self) -> f32 {
        match self {
            Self::BuilderLogMassEarth => 5.0,
            Self::BuilderOrbitAu => 50.0,
            Self::BuilderEccentricity => 1.30,
            Self::InspectorLogMassEarth => 8.0,
            Self::EpochAgeGyr => 5.5,
            Self::InspectorOrbitAu => 60.0,
            Self::InspectorAxialTiltDeg => 180.0,
        }
    }

    pub fn step(&self) -> f32 {
        match self {
            Self::BuilderLogMassEarth
            | Self::InspectorLogMassEarth
            | Self::BuilderOrbitAu
            | Self::InspectorOrbitAu => 0.05,
            Self::BuilderEccentricity => 0.005,
            Self::InspectorAxialTiltDeg => 0.5,
            Self::EpochAgeGyr => 0.01,
        }
    }

    pub fn precision(&self) -> usize {
        match self {
            Self::BuilderEccentricity => 3,
            Self::BuilderOrbitAu | Self::InspectorOrbitAu | Self::EpochAgeGyr => 2,
            Self::BuilderLogMassEarth
            | Self::InspectorLogMassEarth
            | Self::InspectorAxialTiltDeg => 1,
        }
    }

    pub fn format_value(&self, val: f32) -> String {
        match self {
            Self::BuilderLogMassEarth | Self::InspectorLogMassEarth => {
                let mass_m_earth = 10.0_f32.powf(val);
                if mass_m_earth < 0.01 {
                    format!("{mass_m_earth:.4} M⊕ (10^{val:.2})")
                } else if mass_m_earth < 10.0 {
                    format!("{mass_m_earth:.2} M⊕ (10^{val:.2})")
                } else if mass_m_earth < 1000.0 {
                    format!("{mass_m_earth:.1} M⊕ (10^{val:.2})")
                } else {
                    let m_sun = mass_m_earth / 332_946.0;
                    if m_sun >= 10.0 {
                        format!("{m_sun:.1} M☉ (10^{val:.2})")
                    } else {
                        format!("{m_sun:.3} M☉ (10^{val:.2})")
                    }
                }
            }
            Self::BuilderOrbitAu => format!("{val:.2} AU"),
            Self::InspectorOrbitAu => {
                if val < 0.05 {
                    format!("{val:.4} AU")
                } else {
                    format!("{val:.2} AU")
                }
            }
            Self::BuilderEccentricity => format!("{val:.3}"),
            Self::InspectorAxialTiltDeg => format!("{val:.1}°"),
            Self::EpochAgeGyr => format!("{val:.2} Ga"),
        }
    }
}

/// Component identifying a HUD slider entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct HudSlider(pub HudSliderKind);

/// Component identifying the filled progress track inside a HUD slider.
#[derive(Component, Debug, Clone, Copy)]
pub struct HudSliderFill;

/// Component identifying the sliding thumb indicator inside a HUD slider.
#[derive(Component, Debug, Clone, Copy)]
pub struct HudSliderThumb;

/// Component identifying a live numeric readout label associated with a slider.
#[derive(Component, Debug, Clone, Copy)]
pub struct HudSliderReadout(pub HudSliderKind);

/// Supported categories of HUD text inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum HudTextInputKind {
    /// Custom planet name in the Planet Builder.
    BuilderCustomName,
    /// Editable body name in the Target Inspector.
    InspectorBodyName,
    /// Filter query in the Quick Bar selector.
    QuickBarFilter,
    /// Active slot name in the Save/Load controls.
    SaveSlotName,
}

/// Component identifying an editable text input entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct HudTextInput(pub HudTextInputKind);

/// Wrapper border component surrounding a text input to show focus status.
#[derive(Component, Debug, Clone, Copy)]
pub struct HudTextInputWrapper(pub HudTextInputKind);

/// Spawns a modern styled HUD slider node complete with track, gradient fill, thumb, and live readout.
pub fn spawn_hud_slider(
    commands: &mut ChildSpawnerCommands,
    kind: HudSliderKind,
    initial_val: f32,
    label_text: &str,
    track_width_px: f32,
) -> Entity {
    let clamped_val = initial_val.clamp(kind.min(), kind.max());
    let pct = ((clamped_val - kind.min()) / (kind.max() - kind.min())).clamp(0.0, 1.0) * 100.0;

    let mut root_ent = Entity::PLACEHOLDER;
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            Name::new(format!("HudSliderRow_{kind:?}")),
        ))
        .with_children(|row| {
            // Label
            row.spawn((
                Text::new(label_text),
                TextFont {
                    font: hud_font_source(),
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.78, 0.95)),
                Node {
                    width: Val::Px(75.0),
                    ..default()
                },
            ));

            // Slider track container
            root_ent = row
                .spawn((
                    Node {
                        width: Val::Px(track_width_px),
                        height: Val::Px(16.0),
                        align_items: AlignItems::Center,
                        position_type: PositionType::Relative,
                        ..default()
                    },
                    Slider {
                        track_click: TrackClick::Snap,
                        orientation: SliderOrientation::Horizontal,
                    },
                    SliderDragState::default(),
                    SliderValue(clamped_val),
                    SliderRange::new(kind.min(), kind.max()),
                    SliderStep(kind.step()),
                    SliderPrecision(i32::try_from(kind.precision()).unwrap_or(2)),
                    HudSlider(kind),
                    Hovered::default(),
                ))
                .with_children(|slider| {
                    // Track background (Pickable::IGNORE so pointer events go straight to Slider)
                    slider
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(6.0),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                overflow: Overflow::clip(),
                                position_type: PositionType::Relative,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.06, 0.12, 0.22, 0.85)),
                            Pickable::IGNORE,
                        ))
                        .with_children(|track| {
                            // Filled gradient bar
                            track.spawn((
                                Node {
                                    width: Val::Percent(pct),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundGradient(vec![Gradient::Linear(
                                    LinearGradient::to_right(vec![
                                        ColorStop::auto(Color::srgb(0.12, 0.45, 0.85)),
                                        ColorStop::auto(Color::srgb(0.35, 0.85, 1.0)),
                                    ]),
                                )]),
                                Pickable::IGNORE,
                                HudSliderFill,
                            ));
                        });

                    // Slider thumb
                    slider.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(pct),
                            top: Val::Px(1.0),
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            margin: UiRect::left(Val::Px(-7.0)),
                            border_radius: BorderRadius::all(Val::Px(7.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.9, 0.96, 1.0)),
                        BorderColor::from(Color::srgb(0.2, 0.7, 1.0)),
                        Pickable::IGNORE,
                        HudSliderThumb,
                    ));
                })
                .id();

            // Live readout label
            row.spawn((
                Text::new(kind.format_value(clamped_val)),
                TextFont {
                    font: hud_font_source(),
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.95, 1.0)),
                Node {
                    width: Val::Px(90.0),
                    ..default()
                },
                HudSliderReadout(kind),
            ));
        });

    root_ent
}

/// Spawns a styled editable text input with cursor, selection highlights, tab navigation,
/// and focus border styling.
pub fn spawn_hud_text_input(
    commands: &mut ChildSpawnerCommands,
    kind: HudTextInputKind,
    initial_text: &str,
    width_px: f32,
    font_size: f32,
) -> Entity {
    let mut input_ent = Entity::PLACEHOLDER;
    commands
        .spawn((
            Node {
                width: Val::Px(width_px),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.08, 0.16, 0.92)),
            BorderColor::from(Color::srgb(0.18, 0.38, 0.55)),
            HudTextInputWrapper(kind),
        ))
        .with_children(|wrapper| {
            input_ent = wrapper
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    TextInput,
                    EditableText::new(initial_text),
                    TextCursorStyle {
                        color: Color::srgb(0.4, 0.95, 1.0),
                        selection_color: Color::srgba(0.2, 0.6, 0.95, 0.38),
                        unfocused_selection_color: Color::NONE,
                        selected_text_color: None,
                        selection_radius: 0.15,
                    },
                    TabIndex(0),
                    Hovered::default(),
                    TextFont {
                        font: hud_font_source(),
                        font_size: FontSize::Px(font_size),
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.96, 1.0)),
                    HudTextInput(kind),
                ))
                .id();
        });

    input_ent
}

/// Plugin registering modern HUD widgets, font upgrades, observers, and widget resources.
pub struct ModernHudWidgetsPlugin;

impl Plugin for ModernHudWidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActivatedHudButtons>()
            .init_resource::<PendingSliderChanges>()
            .init_resource::<SaveSlotUiState>()
            .add_observer(on_hud_button_activate)
            .add_observer(slider_self_update)
            .add_observer(on_hud_slider_value_change)
            .add_systems(Startup, setup_bundled_hud_font)
            .add_systems(PostStartup, upgrade_hud_default_fonts)
            .add_systems(
                Update,
                (
                    apply_bundled_hud_font_when_loaded,
                    upgrade_hud_default_fonts,
                    (
                        crate::game::ui::widget_sync::apply_hud_slider_changes,
                        crate::game::ui::widget_sync::sync_hud_sliders,
                    )
                        .chain(),
                    crate::game::ui::widget_sync::sync_hud_text_inputs,
                    crate::game::ui::widget_sync::animate_hud_ui_transforms,
                ),
            );
    }
}
