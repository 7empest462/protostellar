//! Two-way synchronization, model binding, and reactive transform updates for HUD widgets.

use bevy::input_focus::InputFocus;
use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::UiTransform;
use bevy::ui_widgets::{SliderDragState, SliderRange, SliderValue};

use crate::game::ui::types::{PlanetBuilderState, QuickBarState};
use crate::game::ui::widgets::*;
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Marker for the animated targeting reticle brackets around selected/hovered bodies.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct HoverReticleBracket;

/// Marker for the holographic surface horizon compass needle.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SurfaceCompassNeedle;

/// Synchronizes slider progress fill bars, thumb positions, readouts, and model-driven updates.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Sliders sync across simulation state, builder presets, and UI tracks"
)]
pub fn sync_hud_sliders(
    mut commands: Commands,
    slider_query: Query<(
        Entity,
        &HudSlider,
        &SliderRange,
        &SliderDragState,
        &SliderValue,
        &Children,
    )>,
    mut fill_query: Query<&mut Node, (With<HudSliderFill>, Without<HudSliderThumb>)>,
    mut thumb_query: Query<&mut Node, (With<HudSliderThumb>, Without<HudSliderFill>)>,
    mut readout_query: Query<(&HudSliderReadout, &mut Text)>,
    builder_state: Option<Res<PlanetBuilderState>>,
    player_state: Option<Res<PlayerInteractionState>>,
    timeline: Option<Res<crate::simulation::geology::types::TimelineScrubber>>,
    target_body_query: Query<(&Mass, Option<&SimPosition>, Option<&SpinState>)>,
) {
    let mut current_vals = rustc_hash::FxHashMap::default();

    for (ent, hud_slider, range, drag_state, slider_val, children) in slider_query.iter() {
        let val = slider_val.0;
        current_vals.insert(hud_slider.0, val);

        let pct = range.thumb_position(val).clamp(0.0, 1.0) * 100.0;

        for &child in children {
            if let Ok(mut fill_node) = fill_query.get_mut(child) {
                fill_node.width = Val::Percent(pct);
            }
            if let Ok(mut thumb_node) = thumb_query.get_mut(child) {
                thumb_node.left = Val::Percent(pct);
            }
        }

        // External model -> slider synchronization when user is not actively dragging
        if !drag_state.dragging {
            let external_val_opt = match hud_slider.0 {
                HudSliderKind::BuilderLogMassEarth => builder_state.as_ref().map(|b| {
                    let m_earth = b.mass_solar / EARTH_MASS_SOLAR;
                    (m_earth as f32).max(1e-4).log10()
                }),
                HudSliderKind::BuilderOrbitAu => {
                    builder_state.as_ref().map(|b| b.semi_major_axis_au as f32)
                }
                HudSliderKind::BuilderEccentricity => {
                    builder_state.as_ref().map(|b| b.eccentricity as f32)
                }
                HudSliderKind::InspectorLogMassEarth => player_state
                    .as_ref()
                    .and_then(|p| p.selected_entity)
                    .and_then(|e| target_body_query.get(e).ok())
                    .map(|(mass, ..)| {
                        let m_earth = mass.0 / EARTH_MASS_SOLAR;
                        (m_earth as f32).max(1e-4).log10()
                    }),
                HudSliderKind::InspectorOrbitAu => player_state
                    .as_ref()
                    .and_then(|p| p.selected_entity)
                    .and_then(|e| target_body_query.get(e).ok())
                    .and_then(|(_, pos, ..)| pos)
                    .map(|pos| pos.0.length() as f32),
                HudSliderKind::InspectorAxialTiltDeg => player_state
                    .as_ref()
                    .and_then(|p| p.selected_entity)
                    .and_then(|e| target_body_query.get(e).ok())
                    .and_then(|(.., spin)| spin)
                    .map(|spin| spin.axial_tilt_degrees as f32),
                HudSliderKind::EpochAgeGyr => timeline.as_ref().map(|t| t.scrubbed_age_gyr),
            };

            if let Some(ext_val) = external_val_opt {
                let clamped_target = range.clamp(ext_val);
                if (clamped_target - val).abs() > 0.005 {
                    commands.entity(ent).insert(SliderValue(clamped_target));
                }
            }
        }
    }

    // Update live text readouts
    for (readout, mut text) in readout_query.iter_mut() {
        if let Some(&val) = current_vals.get(&readout.0) {
            text.0 = readout.0.format_value(val);
        }
    }
}

/// Applies user-driven slider changes to the simulation state and builder parameters.
#[allow(
    clippy::type_complexity,
    reason = "Dispatches slider edits to world and components"
)]
pub fn apply_hud_slider_changes(
    mut pending: Option<ResMut<PendingSliderChanges>>,
    mut builder_state: Option<ResMut<PlanetBuilderState>>,
    player_state: Option<Res<PlayerInteractionState>>,
    mut target_body_query: Query<(
        &mut Mass,
        Option<&mut SimPosition>,
        Option<&mut SimVelocity>,
        Option<&mut SpinState>,
    )>,
    mut timeline: Option<ResMut<crate::simulation::geology::types::TimelineScrubber>>,
) {
    let Some(ref mut changes) = pending else {
        return;
    };
    if changes.0.is_empty() {
        return;
    }

    for (kind, val) in changes.0.drain(..) {
        match kind {
            HudSliderKind::BuilderLogMassEarth => {
                if let Some(ref mut builder) = builder_state {
                    let mass_earth = 10.0_f64.powf(f64::from(val));
                    builder.mass_solar = mass_earth * EARTH_MASS_SOLAR;
                }
            }
            HudSliderKind::BuilderOrbitAu => {
                if let Some(ref mut builder) = builder_state {
                    builder.semi_major_axis_au = f64::from(val);
                }
            }
            HudSliderKind::BuilderEccentricity => {
                if let Some(ref mut builder) = builder_state {
                    builder.eccentricity = f64::from(val);
                }
            }
            HudSliderKind::InspectorLogMassEarth => {
                if let Some(selected) = player_state.as_ref().and_then(|p| p.selected_entity) {
                    if let Ok((mut mass, ..)) = target_body_query.get_mut(selected) {
                        let mass_earth = 10.0_f64.powf(f64::from(val));
                        mass.0 = mass_earth * EARTH_MASS_SOLAR;
                    }
                }
            }
            HudSliderKind::InspectorOrbitAu => {
                if let Some(selected) = player_state.as_ref().and_then(|p| p.selected_entity) {
                    if let Ok((mass, Some(mut pos), Some(mut vel), _)) =
                        target_body_query.get_mut(selected)
                    {
                        let current_r = pos.0.length();
                        let target_r = f64::from(val).max(0.01);
                        if current_r > 0.001 {
                            let ratio = target_r / current_r;
                            pos.0 *= ratio;
                            // Scale velocity for circular Keplerian consistency: v ~ 1/sqrt(r)
                            vel.0 /= ratio.sqrt();
                        }
                        let _ = mass;
                    }
                }
            }
            HudSliderKind::InspectorAxialTiltDeg => {
                if let Some(selected) = player_state.as_ref().and_then(|p| p.selected_entity) {
                    if let Ok((_, _, _, Some(mut spin))) = target_body_query.get_mut(selected) {
                        spin.axial_tilt_degrees = f64::from(val);
                        let rad = f64::from(val).to_radians();
                        spin.spin_vector = bevy::math::DVec3::new(rad.sin(), rad.cos(), 0.0);
                    }
                }
            }
            HudSliderKind::EpochAgeGyr => {
                if let Some(ref mut t) = timeline {
                    t.scrubbed_age_gyr = val;
                }
            }
        }
    }
}

/// Synchronizes focus highlights and two-way string binding for editable text inputs.
#[allow(clippy::type_complexity, reason = "Two-way text input binding system")]
pub fn sync_hud_text_inputs(
    focus: Option<Res<InputFocus>>,
    mut wrapper_query: Query<(&HudTextInputWrapper, &mut BorderColor, &Children)>,
    mut input_query: Query<(Entity, &HudTextInput, &mut EditableText)>,
    mut builder_state: Option<ResMut<PlanetBuilderState>>,
    mut quick_bar_state: Option<ResMut<QuickBarState>>,
    mut save_slot_state: Option<ResMut<SaveSlotUiState>>,
    player_state: Option<Res<PlayerInteractionState>>,
    mut body_name_query: Query<&mut CelestialBody>,
) {
    let focused_ent = focus.as_ref().and_then(|f| f.get());

    for (_wrapper, mut border_color, children) in wrapper_query.iter_mut() {
        let is_wrapper_focused = children.iter().any(|child| focused_ent == Some(child));

        if is_wrapper_focused {
            *border_color = BorderColor::all(Color::srgb(0.35, 0.85, 1.0));
        } else {
            *border_color = BorderColor::all(Color::srgb(0.18, 0.38, 0.55));
        }
    }

    for (ent, input, mut editable) in input_query.iter_mut() {
        let is_focused = focused_ent == Some(ent);

        if is_focused {
            let val = editable.value().to_string();
            match input.0 {
                HudTextInputKind::BuilderCustomName => {
                    if let Some(ref mut builder) = builder_state {
                        builder.custom_name = val;
                    }
                }
                HudTextInputKind::InspectorBodyName => {
                    if let Some(selected) = player_state.as_ref().and_then(|p| p.selected_entity) {
                        if let Ok(mut body) = body_name_query.get_mut(selected) {
                            body.name = val;
                        }
                    }
                }
                HudTextInputKind::QuickBarFilter => {
                    if let Some(ref mut qb) = quick_bar_state {
                        qb.filter_text = val;
                    }
                }
                HudTextInputKind::SaveSlotName => {
                    if let Some(ref mut slot) = save_slot_state {
                        slot.slot_name = val;
                    }
                }
            }
        } else {
            // External model -> TextInput synchronization when unfocused
            let current_val = editable.value().to_string();
            let external_val_opt = match input.0 {
                HudTextInputKind::BuilderCustomName => {
                    builder_state.as_ref().map(|b| b.custom_name.clone())
                }
                HudTextInputKind::InspectorBodyName => player_state
                    .as_ref()
                    .and_then(|p| p.selected_entity)
                    .and_then(|e| body_name_query.get(e).ok())
                    .map(|b| b.name.clone()),
                HudTextInputKind::QuickBarFilter => {
                    quick_bar_state.as_ref().map(|qb| qb.filter_text.clone())
                }
                HudTextInputKind::SaveSlotName => {
                    save_slot_state.as_ref().map(|s| s.slot_name.clone())
                }
            };

            if let Some(ext_val) = external_val_opt {
                if ext_val != current_val {
                    *editable = EditableText::new(&ext_val);
                }
            }
        }
    }
}

/// Drives continuous rotation and responsive orientation on HUD widgets via `UiTransform`.
pub fn animate_hud_ui_transforms(
    time: Res<Time>,
    camera_query: Query<&PanOrbitCamera>,
    mut reticle_query: Query<&mut UiTransform, With<HoverReticleBracket>>,
    mut compass_query: Query<
        &mut UiTransform,
        (With<SurfaceCompassNeedle>, Without<HoverReticleBracket>),
    >,
) {
    let t = time.elapsed_secs();

    // 1. Animate rotating targeting reticle brackets
    for mut transform in reticle_query.iter_mut() {
        transform.rotation = Rot2::radians(t * 1.2);
    }

    // 2. Animate holographic surface horizon rover compass needle
    if let Some(camera) = camera_query.iter().next() {
        if camera.surface_horizon_mode {
            for mut transform in compass_query.iter_mut() {
                transform.rotation = Rot2::radians(-camera.surface_azimuth);
            }
        }
    }
}
