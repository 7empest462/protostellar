//! Target inspector panel and context-sensitive action toolbars.

use bevy::prelude::*;

use super::types::*;
use crate::simulation::components::{Composition, PlanetaryClimate, VolatileInventory};

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
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(3.0)),
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
                        create_compact_button(
                            hdr,
                            UiButtonAction::ToggleInspectorPanel,
                            "🗕",
                            Color::srgba(0.16, 0.08, 0.12, 0.85),
                            Color::srgb(0.9, 0.4, 0.6),
                        );
                    });

                panel.spawn((
                    Text::new(
                        "No celestial body selected. Click on the Star or Planets to inspect & edit.",
                    ),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.90, 0.94, 1.0)),
                    HudInspectorText,
                ));

                panel
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
        });
}

fn spawn_inspector_toolbar_rows(actions: &mut ChildSpawnerCommands) {
    spawn_orbit_tracking_row(actions);
    spawn_mass_composition_row(actions);
    spawn_astrophysics_actions_row(actions);
    spawn_exotic_experiments_row(actions);
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
            const BUTTONS: [(UiButtonAction, &str, Color, Color); 7] = [
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
            const BUTTONS: [(UiButtonAction, &str, Color, Color); 7] = [
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

/// Formats the volatile H2O component for the composition readout,
/// dynamically distinguishing liquid water and ocean coverage from frozen ice.
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
    let ocean_pct = opt_vol.map_or(0.0, |v| (f64::from(v.ocean_coverage_frac) * 100.0).round());
    let surface_ice_pct = opt_climate.map_or(
        opt_vol.map_or(0.0, |v| f64::from(v.ocean_coverage_frac)),
        |c| f64::from(c.ice_coverage_frac),
    ) * 100.0;
    let ice_cov_pct = surface_ice_pct.round();
    let has_water =
        norm.ice_frac > 0.0001 || opt_vol.is_some_and(|v| v.delivered_water_m_earth > 1e-6);

    if is_warm {
        if ocean_pct >= 1.0 {
            if ice_pct >= 1.0 {
                format!("{ocean_pct:.0}% Ocean ({ice_pct:.0}% Water)")
            } else {
                format!("{ocean_pct:.0}% Ocean Water")
            }
        } else if ice_pct >= 1.0 {
            format!("{ice_pct:.0}% Water")
        } else if ice_cov_pct >= 1.0 {
            format!("{ice_cov_pct:.0}% Polar Ice")
        } else if has_water {
            "<1% Water".to_string()
        } else {
            "0% Water".to_string()
        }
    } else if ice_pct >= 1.0 {
        format!("{ice_pct:.0}% Ice")
    } else if ice_cov_pct >= 1.0 {
        format!("{ice_cov_pct:.0}% Surface Ice")
    } else if has_water {
        "<1% Ice".to_string()
    } else {
        "0% Ice".to_string()
    }
}
