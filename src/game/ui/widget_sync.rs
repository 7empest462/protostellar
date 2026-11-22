//! Two-way synchronization, model binding, and reactive transform updates for HUD widgets.

use bevy::input_focus::InputFocus;
use bevy::math::{DVec3, Rot2};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::UiTransform;
use bevy::ui_widgets::{SliderDragState, SliderRange, SliderStep, SliderValue};

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

fn apply_mass_slider_to_body(
    commands: &mut Commands,
    entity: Entity,
    (mass, radius_opt, body_opt, comp_opt, diff_opt): (
        &mut Mass,
        Option<&mut Radius>,
        Option<&mut CelestialBody>,
        Option<&Composition>,
        Option<&mut InternalDifferentiation>,
    ),
    (temp_opt, lum_opt, evo_opt): (
        Option<&mut Temperature>,
        Option<&mut Luminosity>,
        Option<&mut StellarEvolutionState>,
    ),
    log_mass_earth: f32,
) {
    let mass_earth = 10.0_f64.powf(f64::from(log_mass_earth));
    mass.0 = mass_earth * EARTH_MASS_SOLAR;

    let comp = comp_opt.copied().unwrap_or_else(Composition::rocky);
    let was_star_or_remnant = body_opt
        .as_deref()
        .is_some_and(|b| b.body_type.is_star_or_remnant());
    let mut new_radius = radius_opt.as_deref().map_or(EARTH_RADIUS_AU, |r| r.0);

    if was_star_or_remnant && let Some(body) = body_opt {
        let old_type = body.body_type;
        let mut rad_reborrow = radius_opt;
        crate::simulation::thermodynamics::sync_stellar_classification_after_mass_edit(
            mass.0,
            old_type,
            &comp,
            body,
            rad_reborrow.as_deref_mut(),
            temp_opt,
            lum_opt,
            evo_opt,
        );
        if let Some(r) = rad_reborrow {
            new_radius = r.0;
        }
    } else {
        let mut eff_type = BodyType::TerrestrialPlanet;
        if let Some(body) = body_opt {
            if !body.body_type.is_remnant() && !body.body_type.is_hypothetical() {
                eff_type = classify_body_by_mass_and_comp(mass.0, &comp, false);
                body.body_type = eff_type;
            } else {
                eff_type = body.body_type;
            }
        }
        if let Some(radius) = radius_opt {
            if eff_type == BodyType::BlackHole {
                radius.0 = (1.974e-8 * mass.0).max(1e-7);
            } else if eff_type == BodyType::BrownDwarf {
                radius.0 = 0.11 * SOLAR_RADIUS_AU;
            } else if eff_type.is_star_or_remnant() {
                if !eff_type.is_remnant() && !eff_type.is_hypothetical() {
                    radius.0 = SOLAR_RADIUS_AU * mass.0.max(0.08).powf(0.8);
                }
            } else {
                let avg_density = comp.average_density();
                radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * std::f64::consts::PI))
                    .cbrt()
                    .max(EARTH_RADIUS_AU * 0.1);
            }
            new_radius = radius.0;
        }
        if !was_star_or_remnant
            && eff_type.is_star_or_remnant()
            && let Ok(mut cmd) = commands.get_entity(entity)
        {
            let is_bd = eff_type == BodyType::BrownDwarf;
            let lum = if is_bd {
                1e-4 * (mass.0 / 0.05).clamp(0.1, 2.0)
            } else {
                mass.0.max(0.01).powf(3.5)
            };
            let surf_temp_k = if is_bd {
                1400.0
            } else if mass.0 >= 2.0 {
                12_000.0
            } else if mass.0 >= 0.8 {
                5778.0
            } else {
                3200.0
            };
            cmd.remove::<(PlanetaryClimate, BiosphereState)>().insert((
                Luminosity(lum),
                Temperature(surf_temp_k),
                IgnitionState {
                    core_temperature: if is_bd { 2.5e6 } else { 1.5e7 },
                    fusion_fraction: if is_bd { 0.05 } else { 1.0 },
                    is_ignited: !is_bd,
                    shockwave_radius: 0.0,
                },
                StellarEvolutionState {
                    phase: if is_bd {
                        StellarEvolutionPhase::ProtostarContraction
                    } else {
                        StellarEvolutionPhase::MainSequence
                    },
                    ..default()
                },
            ));
        }
    }

    if let Some(diff) = diff_opt {
        diff.recalculate(mass.0, new_radius, &comp);
    }
}

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
    )>,
    children_query: Query<&Children>,
    mut fill_query: Query<&mut Node, (With<HudSliderFill>, Without<HudSliderThumb>)>,
    mut thumb_query: Query<&mut Node, (With<HudSliderThumb>, Without<HudSliderFill>)>,
    mut readout_query: Query<(&HudSliderReadout, &mut Text)>,
    builder_state: Option<Res<PlanetBuilderState>>,
    player_state: Option<Res<PlayerInteractionState>>,
    timeline: Option<Res<crate::simulation::geology::types::TimelineScrubber>>,
    target_body_query: Query<(
        &Mass,
        Option<&SimPosition>,
        Option<&SpinState>,
        Option<&SatelliteOf>,
    )>,
) {
    let mut current_vals = rustc_hash::FxHashMap::default();
    let selected_sat_parent = player_state
        .as_ref()
        .and_then(|p| p.selected_entity)
        .and_then(|e| target_body_query.get(e).ok())
        .and_then(|(_, _, _, opt_sat)| opt_sat.map(|s| s.parent));

    for (ent, hud_slider, range, drag_state, slider_val) in slider_query.iter() {
        let mut val = slider_val.0;
        let mut active_range = *range;

        if hud_slider.0 == HudSliderKind::InspectorOrbitAu {
            let (desired_min, desired_max, desired_step) = if selected_sat_parent.is_some() {
                (0.0005_f32, 0.25_f32, 0.001_f32)
            } else {
                (hud_slider.0.min(), hud_slider.0.max(), hud_slider.0.step())
            };
            if (range.start() - desired_min).abs() > 1e-6
                || (range.end() - desired_max).abs() > 1e-6
            {
                let next_range = SliderRange::new(desired_min, desired_max);
                commands
                    .entity(ent)
                    .insert((next_range, SliderStep(desired_step)));
                active_range = next_range;
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
                    .and_then(|(_, pos, _, opt_sat)| {
                        let pos = pos?;
                        if let Some(sat) = opt_sat
                            && let Ok((_, Some(parent_pos), ..)) = target_body_query.get(sat.parent)
                        {
                            Some((pos.0 - parent_pos.0).length() as f32)
                        } else {
                            Some(pos.0.length() as f32)
                        }
                    }),
                HudSliderKind::InspectorAxialTiltDeg => player_state
                    .as_ref()
                    .and_then(|p| p.selected_entity)
                    .and_then(|e| target_body_query.get(e).ok())
                    .and_then(|(_, _, spin, _)| spin)
                    .map(|spin| spin.axial_tilt_degrees as f32),
                HudSliderKind::EpochAgeGyr => timeline.as_ref().map(|t| t.scrubbed_age_gyr),
            };

            if let Some(ext_val) = external_val_opt {
                let clamped_target = active_range.clamp(ext_val);
                let eps = if selected_sat_parent.is_some()
                    && hud_slider.0 == HudSliderKind::InspectorOrbitAu
                {
                    0.0001
                } else {
                    0.005
                };
                if (clamped_target - val).abs() > eps {
                    commands.entity(ent).insert(SliderValue(clamped_target));
                    val = clamped_target;
                }
            }
        }

        current_vals.insert(hud_slider.0, val);
        let pct = active_range.thumb_position(val).clamp(0.0, 1.0) * 100.0;
        let fill_width = Val::Percent(pct);
        let thumb_left = Val::Percent(pct);

        for child in children_query.iter_descendants(ent) {
            if let Ok(mut fill_node) = fill_query.get_mut(child)
                && fill_node.width != fill_width
            {
                fill_node.width = fill_width;
            }
            if let Ok(mut thumb_node) = thumb_query.get_mut(child)
                && thumb_node.left != thumb_left
            {
                thumb_node.left = thumb_left;
            }
        }
    }

    // Update live text readouts only when formatted value changes
    for (readout, mut text) in readout_query.iter_mut() {
        if let Some(&val) = current_vals.get(&readout.0) {
            let formatted = readout.0.format_value(val);
            if text.0 != formatted {
                text.0 = formatted;
            }
        }
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Bundles body, kinematics, and thermal components for HUD slider dispatch"
)]
type SliderTargetBodyQuery<'w, 's> = Query<
    'w,
    's,
    (
        (
            &'static mut Mass,
            Option<&'static mut Radius>,
            Option<&'static mut CelestialBody>,
            Option<&'static Composition>,
            Option<&'static mut InternalDifferentiation>,
        ),
        (
            Option<&'static mut SimPosition>,
            Option<&'static mut SimVelocity>,
            Option<&'static mut SpinState>,
            Option<&'static mut SatelliteOf>,
        ),
        (
            Option<&'static mut Temperature>,
            Option<&'static mut VolatileInventory>,
            Option<&'static mut PlanetaryClimate>,
            Option<&'static CentralStar>,
            Option<&'static mut Luminosity>,
            Option<&'static mut StellarEvolutionState>,
        ),
    ),
>;

fn apply_orbit_slider_to_body(
    selected: Entity,
    val: f32,
    star_irrad: crate::simulation::thermodynamics::StellarIrradiance,
    target_body_query: &mut SliderTargetBodyQuery<'_, '_>,
) {
    let parent_ent = target_body_query
        .get(selected)
        .ok()
        .and_then(|t| t.1.3.map(|s| s.parent));
    let (parent_pos, parent_vel, parent_mass) = parent_ent
        .and_then(|p| target_body_query.get(p).ok())
        .map_or((DVec3::ZERO, DVec3::ZERO, 0.0), |t| {
            (
                t.1.0.as_ref().map_or(DVec3::ZERO, |p| p.0),
                t.1.1.as_ref().map_or(DVec3::ZERO, |v| v.0),
                t.0.0.0,
            )
        });
    let min_r = if parent_ent.is_some() { 1e-4 } else { 0.01 };
    let Ok((
        (mass, opt_rad, opt_body, opt_comp, _),
        (Some(mut pos), Some(mut vel), _, mut opt_sat),
        (mut opt_temp, mut opt_vol, mut opt_clim, ..),
    )) = target_body_query.get_mut(selected)
    else {
        return;
    };
    let rel_pos = pos.0 - parent_pos;
    let current_r = rel_pos.length();
    let target_r = f64::from(val).max(min_r);
    if current_r <= 1e-6 {
        return;
    }
    let ratio = target_r / current_r;
    pos.0 = parent_pos + rel_pos * ratio;
    let rel_vel = vel.0 - parent_vel;
    // Scale velocity for circular Keplerian consistency: v ~ 1/sqrt(r)
    vel.0 = parent_vel + rel_vel / ratio.sqrt();
    if let Some(ref mut sat) = opt_sat {
        sat.semi_major_axis_au = target_r;
        let mu = crate::utils::constants::G_ASTRO * (parent_mass + mass.0).max(1e-12);
        sat.orbital_period_years = (2.0 * std::f64::consts::PI) * (target_r.powi(3) / mu).sqrt();
    }
    if parent_ent.is_none()
        && let Some(ref mut temp) = opt_temp
    {
        let body_type = opt_body
            .as_ref()
            .map_or(BodyType::TerrestrialPlanet, |b| b.body_type);
        let default_comp = Composition::rocky();
        let comp = opt_comp.unwrap_or(&default_comp);
        let rad = opt_rad.as_ref().map_or(EARTH_RADIUS_AU, |r| r.0);
        crate::simulation::thermodynamics::equilibrate_body_temperature_for_orbit(
            target_r,
            mass.0,
            rad,
            body_type,
            comp,
            star_irrad,
            temp,
            opt_vol.as_deref_mut(),
            opt_clim.as_deref_mut(),
        );
    }
}

/// Applies user-driven slider changes to the simulation state and builder parameters.
pub fn apply_hud_slider_changes(
    mut commands: Commands,
    mut pending: Option<ResMut<PendingSliderChanges>>,
    mut builder_state: Option<ResMut<PlanetBuilderState>>,
    player_state: Option<Res<PlayerInteractionState>>,
    mut target_body_query: SliderTargetBodyQuery<'_, '_>,
    disk_params: Option<Res<DiskParameters>>,
    mut timeline: Option<ResMut<crate::simulation::geology::types::TimelineScrubber>>,
) {
    let Some(ref mut changes) = pending else {
        return;
    };
    if changes.0.is_empty() {
        return;
    }

    let star_mass_fallback = disk_params.as_deref().map_or(1.0, |d| d.central_star_mass);
    let star_irrad = target_body_query
        .iter()
        .find(|t| {
            t.2.3.is_some()
                || t.0
                    .2
                    .as_ref()
                    .is_some_and(|b| b.body_type.is_star_or_remnant())
        })
        .map_or_else(
            || {
                crate::simulation::thermodynamics::stellar_irradiance_from_mass_or_opt(
                    None,
                    None,
                    None,
                    star_mass_fallback,
                )
            },
            |t| {
                crate::simulation::thermodynamics::stellar_irradiance_from_mass_or_opt(
                    t.2.4.as_ref().map(|l| l.0),
                    t.2.0.as_ref().map(|tp| tp.0),
                    t.0.1.as_ref().map(|r| r.0),
                    t.0.0.0,
                )
            },
        );

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
                if let Some(selected) = player_state.as_ref().and_then(|p| p.selected_entity)
                    && let Ok((
                        (mut mass, mut radius_opt, mut body_opt, comp_opt, mut diff_opt),
                        _,
                        (mut temp_opt, _, _, _, mut lum_opt, mut evo_opt),
                    )) = target_body_query.get_mut(selected)
                {
                    apply_mass_slider_to_body(
                        &mut commands,
                        selected,
                        (
                            &mut mass,
                            radius_opt.as_deref_mut(),
                            body_opt.as_deref_mut(),
                            comp_opt,
                            diff_opt.as_deref_mut(),
                        ),
                        (
                            temp_opt.as_deref_mut(),
                            lum_opt.as_deref_mut(),
                            evo_opt.as_deref_mut(),
                        ),
                        val,
                    );
                }
            }
            HudSliderKind::InspectorOrbitAu => {
                if let Some(selected) = player_state.as_ref().and_then(|p| p.selected_entity) {
                    apply_orbit_slider_to_body(selected, val, star_irrad, &mut target_body_query);
                }
            }
            HudSliderKind::InspectorAxialTiltDeg => {
                if let Some(selected) = player_state.as_ref().and_then(|p| p.selected_entity)
                    && let Ok((_, (_, _, Some(mut spin), _), _)) =
                        target_body_query.get_mut(selected)
                {
                    spin.axial_tilt_degrees = f64::from(val);
                    let rad = f64::from(val).to_radians();
                    spin.spin_vector = bevy::math::DVec3::new(rad.sin(), rad.cos(), 0.0);
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
        let target_border = if is_wrapper_focused {
            BorderColor::all(Color::srgb(0.35, 0.85, 1.0))
        } else {
            BorderColor::all(Color::srgb(0.18, 0.38, 0.55))
        };
        if *border_color != target_border {
            *border_color = target_border;
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
                    if let Some(selected) = player_state.as_ref().and_then(|p| p.selected_entity)
                        && let Ok(mut body) = body_name_query.get_mut(selected)
                    {
                        body.name = val;
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

            if let Some(ext_val) = external_val_opt
                && ext_val != current_val
            {
                *editable = EditableText::new(&ext_val);
            }
        }
    }
}

/// Drives continuous rotation and responsive orientation on HUD widgets via `UiTransform`.
#[allow(
    clippy::type_complexity,
    reason = "HUD UiTransform visibility filter query"
)]
pub fn animate_hud_ui_transforms(
    time: Res<Time>,
    camera_query: Query<&PanOrbitCamera>,
    mut reticle_query: Query<
        (
            &mut UiTransform,
            Option<&Node>,
            Option<&InheritedVisibility>,
        ),
        With<HoverReticleBracket>,
    >,
    mut compass_query: Query<
        (
            &mut UiTransform,
            Option<&Node>,
            Option<&InheritedVisibility>,
        ),
        (With<SurfaceCompassNeedle>, Without<HoverReticleBracket>),
    >,
) {
    let t = time.elapsed_secs();

    // 1. Animate rotating targeting reticle brackets only when visible
    for (mut transform, opt_node, opt_vis) in reticle_query.iter_mut() {
        if opt_node.is_some_and(|node| node.display == Display::None)
            || opt_vis.is_some_and(|v| !v.get())
        {
            continue;
        }
        let target_rot = Rot2::radians(t * 1.2);
        if transform.as_ref().rotation != target_rot {
            transform.rotation = target_rot;
        }
    }

    // 2. Animate holographic surface horizon rover compass needle
    if let Some(camera) = camera_query.iter().next()
        && camera.surface_horizon_mode
    {
        let target_rot = Rot2::radians(-camera.surface_azimuth);
        for (mut transform, opt_node, opt_vis) in compass_query.iter_mut() {
            if opt_node.is_some_and(|node| node.display == Display::None)
                || opt_vis.is_some_and(|v| !v.get())
            {
                continue;
            }
            if transform.as_ref().rotation != target_rot {
                transform.rotation = target_rot;
            }
        }
    }
}
