//! Target inspector panel and context-sensitive action toolbars.

use bevy::prelude::*;

use super::types::*;
use crate::simulation::components::{
    Composition, PlanetaryClimate, RelativisticJetState, VolatileInventory,
};

/// Spawns the bottom-left Target Inspector and actions panel.
pub fn spawn_bottom_left_inspector_panel(bottom_row: &mut ChildSpawnerCommands) {
    bottom_row
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                HudPanelElement::InspectorPanel,
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    width: Val::Px(380.0),
                    max_height: Val::Vh(60.0),
                    border: UiRect::all(Val::Px(1.5)),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.09, 0.94)),
                BorderColor::all(Color::srgba(0.25, 0.55, 0.95, 0.75)),
            ))
            .with_children(|panel| {
                spawn_inspector_header(panel);
                spawn_inspector_scrollable_body(panel);
            });

            spawn_inspector_chip(col);
        });
}

fn spawn_inspector_header(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::bottom(Val::Px(4.0)),
            ..default()
        })
        .with_children(|hdr| {
            hdr.spawn((
                Text::new("🔍 TARGET INSPECTOR & ACTIONS"),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.8, 1.0)),
            ));
            hdr.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|right_hdr| {
                right_hdr.spawn((
                    Text::new("↕ Scroll"),
                    TextFont {
                        font_size: FontSize::Px(8.5),
                        ..default()
                    },
                    TextColor(Color::srgba(0.4, 0.75, 1.0, 0.8)),
                ));
                create_compact_button(
                    right_hdr,
                    UiButtonAction::ToggleInspectorPanel,
                    "🗕",
                    Color::srgba(0.16, 0.08, 0.12, 0.85),
                    Color::srgb(0.9, 0.4, 0.6),
                );
            });
        });
}

fn spawn_inspector_scrollable_body(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn((
            ScrollableInspector,
            ScrollPosition::default(),
            Interaction::default(),
            Node {
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                width: Val::Percent(100.0),
                padding: UiRect::right(Val::Px(3.0)),
                ..default()
            },
        ))
        .with_children(|scroll_box| {
            spawn_inspector_text_and_forecast(scroll_box);

            scroll_box
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|actions| {
                    spawn_inspector_toolbar_rows(actions);

                    actions
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
                                margin: UiRect::top(Val::Px(2.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                width: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.01, 0.02, 0.05, 0.95)),
                            BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.4)),
                        ))
                        .with_children(|tip_box| {
                            tip_box.spawn((
                                Text::new("Hover over buttons for descriptions."),
                                TextFont {
                                    font_size: FontSize::Px(9.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.75, 0.90, 1.0)),
                                HudActionTooltipText,
                            ));
                        });
                });
        });
}

fn spawn_inspector_chip(col: &mut ChildSpawnerCommands) {
    col.spawn((
        Button,
        UiButtonAction::ToggleInspectorPanel,
        HudPanelElement::InspectorChip,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            display: Display::None,
            border: UiRect::all(Val::Px(1.5)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.09, 0.92)),
        BorderColor::all(Color::srgba(0.25, 0.55, 0.95, 0.75)),
    ))
    .with_children(|chip| {
        chip.spawn((
            Text::new("🔍 Inspector ▲ Expand"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.4, 0.85, 1.0)),
            HudDynamicText::InspectorChip,
            Pickable::IGNORE,
        ));
    });
}

fn spawn_inspector_toolbar_rows(actions: &mut ChildSpawnerCommands) {
    spawn_orbit_tracking_row(actions);
    spawn_mass_composition_row(actions);
    spawn_terraforming_bombardment_row(actions);
    spawn_astrophysics_actions_row(actions);
    spawn_exotic_experiments_row(actions);
}

fn spawn_terraforming_bombardment_row(actions: &mut ChildSpawnerCommands) {
    actions.spawn((
        Text::new("TERRAFORMING & BOMBARDMENT:"),
        TextFont {
            font_size: FontSize::Px(9.0),
            ..default()
        },
        TextColor(Color::srgb(0.35, 0.90, 0.95)),
    ));
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            const BUTTONS: [(UiButtonAction, &str, Color, Color); 4] = [
                (
                    UiButtonAction::BombardComet,
                    "Comet (H₂O)",
                    Color::srgba(0.06, 0.18, 0.24, 0.9),
                    Color::srgb(0.35, 0.85, 1.0),
                ),
                (
                    UiButtonAction::BombardChondrite,
                    "Chondrite (Atm)",
                    Color::srgba(0.22, 0.14, 0.06, 0.9),
                    Color::srgb(1.0, 0.65, 0.25),
                ),
                (
                    UiButtonAction::BombardSalvo,
                    "Volatile Salvo",
                    Color::srgba(0.18, 0.08, 0.24, 0.9),
                    Color::srgb(0.80, 0.50, 1.0),
                ),
                (
                    UiButtonAction::BombardCore,
                    "Core Impactor",
                    Color::srgba(0.24, 0.08, 0.06, 0.9),
                    Color::srgb(1.0, 0.35, 0.25),
                ),
            ];
            for (act, label, bg, border) in BUTTONS {
                create_compact_button(row, act, label, bg, border);
            }
        });
}

fn spawn_orbit_tracking_row(actions: &mut ChildSpawnerCommands) {
    actions.spawn((
        Text::new("ORBIT & TRACKING:"),
        TextFont {
            font_size: FontSize::Px(9.0),
            ..default()
        },
        TextColor(Color::srgb(0.4, 0.75, 1.0)),
    ));
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            const BUTTONS: [(UiButtonAction, &str, Color, Color); 10] = [
                (
                    UiButtonAction::FocusLock,
                    "Track [F]",
                    Color::srgba(0.08, 0.16, 0.24, 0.9),
                    Color::srgb(0.4, 0.8, 1.0),
                ),
                (
                    UiButtonAction::FixOrbit,
                    "Fix Orbit [Z]",
                    Color::srgba(0.06, 0.22, 0.22, 0.9),
                    Color::srgb(0.3, 0.95, 0.85),
                ),
                (
                    UiButtonAction::TidalLock,
                    "⚡ Lock",
                    Color::srgba(0.12, 0.14, 0.28, 0.9),
                    Color::srgb(0.65, 0.75, 1.0),
                ),
                (
                    UiButtonAction::AccelerateInspiral,
                    "⚡ Inspiral",
                    Color::srgba(0.18, 0.08, 0.24, 0.9),
                    Color::srgb(0.85, 0.50, 1.0),
                ),
                (
                    UiButtonAction::ExpandOrbit,
                    "+10% [O]",
                    Color::srgba(0.06, 0.15, 0.24, 0.9),
                    Color::srgb(0.3, 0.75, 0.95),
                ),
                (
                    UiButtonAction::ContractOrbit,
                    "-10% [L]",
                    Color::srgba(0.06, 0.15, 0.24, 0.9),
                    Color::srgb(0.3, 0.75, 0.95),
                ),
                (
                    UiButtonAction::BoostDeltaV,
                    "+Δv [B]",
                    Color::srgba(0.18, 0.15, 0.06, 0.9),
                    Color::srgb(0.95, 0.85, 0.3),
                ),
                (
                    UiButtonAction::BrakeDeltaV,
                    "-Δv [K]",
                    Color::srgba(0.18, 0.12, 0.06, 0.9),
                    Color::srgb(0.95, 0.65, 0.25),
                ),
                (
                    UiButtonAction::ToggleTrajectoryPredictor,
                    "Forecast [N]",
                    Color::srgba(0.08, 0.20, 0.25, 0.9),
                    Color::srgb(0.4, 0.9, 1.0),
                ),
                (
                    UiButtonAction::DeselectBody,
                    "Deselect [Esc]",
                    Color::srgba(0.15, 0.15, 0.18, 0.9),
                    Color::srgb(0.7, 0.7, 0.8),
                ),
            ];
            for (act, label, bg, border) in BUTTONS {
                create_compact_button(row, act, label, bg, border);
            }
        });
}

fn spawn_inspector_text_and_forecast(scroll_box: &mut ChildSpawnerCommands) {
    scroll_box.spawn((
        Text::new("No celestial body selected. Click on the Star or Planets to inspect & edit."),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(Color::srgb(0.90, 0.94, 1.0)),
        HudInspectorText,
    ));

    scroll_box
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.5)),
                margin: UiRect::top(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                width: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.05, 0.10, 0.92)),
            BorderColor::all(Color::srgba(0.3, 0.7, 1.0, 0.45)),
        ))
        .with_children(|fc_box| {
            fc_box.spawn((
                Text::new("🎯 FORECAST [N]: Scanning orbital path..."),
                TextFont {
                    font_size: FontSize::Px(9.5),
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.90, 1.0)),
                HudEncounterForecastText,
            ));
        });
}

fn spawn_mass_composition_row(actions: &mut ChildSpawnerCommands) {
    actions.spawn((
        Text::new("MASS & COMPOSITION:"),
        TextFont {
            font_size: FontSize::Px(9.0),
            ..default()
        },
        TextColor(Color::srgb(0.3, 0.85, 0.6)),
    ));
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            const BUTTONS: [(UiButtonAction, &str, Color, Color); 4] = [
                (
                    UiButtonAction::IncreaseMass,
                    "Accrete (+25%) [U]",
                    Color::srgba(0.06, 0.18, 0.12, 0.9),
                    Color::srgb(0.3, 0.9, 0.55),
                ),
                (
                    UiButtonAction::DecreaseMass,
                    "Strip (-20%) [J]",
                    Color::srgba(0.18, 0.08, 0.08, 0.9),
                    Color::srgb(0.95, 0.4, 0.4),
                ),
                (
                    UiButtonAction::CycleComposition,
                    "Material [C]",
                    Color::srgba(0.14, 0.10, 0.24, 0.9),
                    Color::srgb(0.75, 0.55, 1.0),
                ),
                (
                    UiButtonAction::VaporizeBody,
                    "Dust [Del]",
                    Color::srgba(0.28, 0.05, 0.05, 0.9),
                    Color::srgb(1.0, 0.3, 0.3),
                ),
            ];
            for (act, label, bg, border) in BUTTONS {
                create_compact_button(row, act, label, bg, border);
            }
        });
}

fn spawn_astrophysics_actions_row(actions: &mut ChildSpawnerCommands) {
    actions.spawn((
        Text::new("ASTROPHYSICS & PHENOMENA:"),
        TextFont {
            font_size: FontSize::Px(9.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.75, 0.3)),
    ));
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            const BUTTONS: [(UiButtonAction, &str, Color, Color); 9] = [
                (
                    UiButtonAction::IgniteStar,
                    "Ignite [I]",
                    Color::srgba(0.24, 0.16, 0.04, 0.9),
                    Color::srgb(1.0, 0.85, 0.3),
                ),
                (
                    UiButtonAction::InjectEmbryo,
                    "Theia [M]",
                    Color::srgba(0.08, 0.18, 0.24, 0.9),
                    Color::srgb(0.4, 0.85, 1.0),
                ),
                (
                    UiButtonAction::ShatterIntoRings,
                    "Rings [X]",
                    Color::srgba(0.18, 0.14, 0.06, 0.9),
                    Color::srgb(1.0, 0.85, 0.35),
                ),
                (
                    UiButtonAction::TriggerLhb,
                    "LHB [G]",
                    Color::srgba(0.24, 0.12, 0.04, 0.9),
                    Color::srgb(1.0, 0.65, 0.2),
                ),
                (
                    UiButtonAction::SeedLife,
                    "Life [E]",
                    Color::srgba(0.04, 0.20, 0.08, 0.9),
                    Color::srgb(0.35, 1.0, 0.45),
                ),
                (
                    UiButtonAction::AgeStar,
                    "Age [N]",
                    Color::srgba(0.24, 0.08, 0.16, 0.9),
                    Color::srgb(1.0, 0.45, 0.75),
                ),
                (
                    UiButtonAction::ToggleTractor,
                    "Tractor [T]",
                    Color::srgba(0.22, 0.08, 0.22, 0.9),
                    Color::srgb(0.95, 0.45, 0.95),
                ),
                (
                    UiButtonAction::StripAtmosphere,
                    "Strip Atm",
                    Color::srgba(0.20, 0.10, 0.26, 0.9),
                    Color::srgb(0.80, 0.55, 1.0),
                ),
                (
                    UiButtonAction::TriggerCoronalMassEjection,
                    "⚡ CME Flare",
                    Color::srgba(0.28, 0.14, 0.02, 0.9),
                    Color::srgb(1.0, 0.70, 0.15),
                ),
            ];
            for (act, label, bg, border) in BUTTONS {
                create_compact_button(row, act, label, bg, border);
            }
        });
}

fn spawn_exotic_experiments_row(actions: &mut ChildSpawnerCommands) {
    actions.spawn((
        Text::new("EXOTIC / LITTLE RED DOT:"),
        TextFont {
            font_size: FontSize::Px(9.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.4, 0.6)),
    ));
    actions
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(2.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            const BUTTONS: [(UiButtonAction, &str, Color, Color); 3] = [
                (
                    UiButtonAction::ToggleSuperEddington,
                    "Hyper-Accretion [X]",
                    Color::srgba(0.24, 0.06, 0.08, 0.9),
                    Color::srgb(1.0, 0.4, 0.5),
                ),
                (
                    UiButtonAction::TriggerBlowoutCocoon,
                    "Quasar Blowout [B]",
                    Color::srgba(0.26, 0.08, 0.22, 0.9),
                    Color::srgb(1.0, 0.45, 0.95),
                ),
                (
                    UiButtonAction::SpawnInfallPop3Star,
                    "Pop-III TDE [T]",
                    Color::srgba(0.08, 0.16, 0.28, 0.9),
                    Color::srgb(0.4, 0.85, 1.0),
                ),
            ];
            for (act, label, bg, border) in BUTTONS {
                create_compact_button(row, act, label, bg, border);
            }
        });
}

/// Formats the volatile H2O mass component for the bulk composition readout (percentage of total mass),
/// distinguishing liquid water from frozen ice based on temperature.
pub fn format_composition_water_ice(
    comp: &Composition,
    opt_vol: Option<&VolatileInventory>,
    opt_climate: Option<&PlanetaryClimate>,
    temp_k: f64,
    is_star: bool,
) -> String {
    let norm = comp.normalized();
    let ice_pct = (norm.ice_frac * 100.0).round();
    if is_star {
        return format!("{ice_pct:.0}% Ice");
    }

    let surf_temp_k = opt_climate.map_or(temp_k, |c| f64::from(c.surface_temperature_k));
    let is_warm = temp_k >= 273.0 || surf_temp_k >= 273.0;
    let has_water =
        norm.ice_frac > 0.0001 || opt_vol.is_some_and(|v| v.delivered_water_m_earth > 1e-6);

    if is_warm {
        if ice_pct >= 1.0 {
            format!("{ice_pct:.0}% Water")
        } else if has_water {
            "<1% Water".to_string()
        } else {
            "0% Water".to_string()
        }
    } else if ice_pct >= 1.0 {
        format!("{ice_pct:.0}% Ice")
    } else if has_water {
        "<1% Ice".to_string()
    } else {
        "0% Ice".to_string()
    }
}

/// Formats Kozai-Lidov secular resonance telemetry for the Target Inspector panel.
pub fn format_kozai_lidov_telemetry(
    opt_kozai: Option<&crate::simulation::kozai_lidov::KozaiLidovState>,
) -> String {
    let mut out = String::new();
    if let Some(kozai) = opt_kozai {
        if kozai.is_in_resonance || kozai.mutual_inclination_deg > 5.0 {
            use std::fmt::Write;
            let status = if kozai.is_in_resonance {
                format!("ACTIVE ({})", kozai.regime.label())
            } else {
                "Subcritical".to_string()
            };
            let _ = write!(
                out,
                "\nKozai-Lidov: {} | Mut Inc: {:.1}° (Crit: {:.1}°)",
                status, kozai.mutual_inclination_deg, kozai.critical_inclination_deg
            );
            if kozai.is_in_resonance {
                let _ = write!(
                    out,
                    "\ne_max: {:.2} (q_min: {:.3} AU) | τ_KL: {:.1e} yr",
                    kozai.max_eccentricity_forecast,
                    kozai.min_periastron_au,
                    kozai.kozai_period_years
                );
                if kozai.is_gr_suppressed {
                    let _ = write!(
                        out,
                        " | [GR Suppressed: τ_KL/τ_GR = {:.1}]",
                        kozai.gr_precession_ratio
                    );
                }
                if kozai.regime == crate::simulation::kozai_lidov::KozaiRegime::TidalDisruptionRisk
                {
                    out.push_str(" ⚠️ ROCHE DANGER");
                }
            }
        }
    }
    out
}

/// Formats relativistic polar jet and synchrotron emission cone telemetry for the Target Inspector panel.
pub fn append_relativistic_jet_telemetry(out: &mut String, jet: &RelativisticJetState) {
    use std::fmt::Write;
    let beta = crate::simulation::relativity::calculate_relativistic_velocity(jet.lorentz_factor);
    let beam_deg = crate::simulation::relativity::calculate_beaming_half_angle(jet.lorentz_factor)
        .to_degrees();
    let cone_deg = jet.opening_angle_rad.to_degrees();
    let _ = write!(
        out,
        "\n--------------------------------------------------\n  >> RELATIVISTIC JET & SYNCHROTRON CONE <<\n--------------------------------------------------\n  • Lorentz Factor (Γ): {:>8.1} (v = {:.4}c)\n  • Beaming Half-Angle: {:>8.1}°\n  • Collimation Cone:   {:>8.1}°\n  • Jet Extent:         {:>8.1} AU\n  • Shock Knots:        {:>8.1}x ({:.2}c)\n  • Synchrotron Lum:    {:>8.1} L☉ (p={:.2})\n--------------------------------------------------",
        jet.lorentz_factor,
        beta,
        beam_deg,
        cone_deg,
        jet.jet_length_au,
        jet.knot_frequency,
        jet.knot_speed_c,
        jet.synchrotron_luminosity,
        jet.spectral_index,
    );
}

/// Formats space weather telemetry (stellar CME flares and planetary auroral ovals) for the Target Inspector panel.
pub fn append_space_weather_telemetry(
    out: &mut String,
    opt_aurora: Option<&crate::simulation::space_weather::AuroralOvalState>,
    opt_flare: Option<&crate::simulation::space_weather::StellarFlareState>,
) {
    use std::fmt::Write;
    if let Some(flare) = opt_flare {
        let flare_status = if flare.current_flare_intensity > 0.1 {
            let class_str = if flare.current_flare_intensity >= 5.0 {
                format!("SUPERFLARE ({:.1}x)", flare.current_flare_intensity)
            } else if flare.current_flare_intensity >= 2.0 {
                format!("X-CLASS ({:.1})", flare.current_flare_intensity * 2.0)
            } else {
                format!("M-CLASS ({:.1})", flare.current_flare_intensity * 5.0)
            };
            format!(
                "ACTIVE [{class_str}] (Decay: {:.2} yr)",
                flare.flare_decay_timer_years
            )
        } else {
            "Quiescent / Solar Minimum".to_string()
        };

        let cme_str = if flare.cme_active {
            let speed_km_s =
                flare.cme_speed_au_day * (crate::utils::constants::AU_TO_KM as f32) / 86400.0;
            format!(
                "PROPAGATING | Front: {:.2} AU | Speed: {:.0} km/s ({:.2} AU/day) | Shock: {:.0}x",
                flare.cme_front_radius_au,
                speed_km_s,
                flare.cme_speed_au_day,
                flare.cme_density_multiplier
            )
        } else {
            "No active CME shock front".to_string()
        };

        let _ = write!(
            out,
            "\n--------------------------------------------------\n  >> STELLAR CORONA & SPACE WEATHER <<\n--------------------------------------------------\n  • Coronal Activity: {}\n  • CME Shockwave:    {}\n  • Flare Frequency:  {:.1} / year\n--------------------------------------------------",
            flare_status,
            cme_str,
            flare.flare_frequency
        );
    }

    if let Some(aurora) = opt_aurora {
        let lat_deg = 90.0 - aurora.oval_colatitude_rad.to_degrees();
        let width_deg = aurora.oval_width_rad.to_degrees();
        let _ = write!(
            out,
            "\n--------------------------------------------------\n  >> MAGNETOSPHERE & AURORAL OVAL <<\n--------------------------------------------------\n  • Space Weather:     {} (Kp {:1.1})\n  • Magnetopause:      {:.1} Rp (Subsolar Standoff)\n  • Auroral Footprint: ±{:.1}° Lat (θ_A = {:.1}°) | Width: {:.1}°\n  • Auroral Emission:  {:.0}% [O 557.7nm / N₂⁺ 391.4nm]\n--------------------------------------------------",
            aurora.storm_level.label(),
            aurora.geomagnetic_kp_index,
            aurora.magnetopause_standoff_rp,
            lat_deg,
            aurora.oval_colatitude_rad.to_degrees(),
            width_deg,
            aurora.auroral_intensity * 100.0
        );
    }
}
