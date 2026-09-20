//! Deep-Time Geological Epoch Scrubber and Continental Drift HUD drawer.
//!
//! Provides interactive timeline scrubbing across Earth's 4.56 Gyr history,
//! one-click era jumping (Hadean, Archean, Proterozoic, Phanerozoic, Modern, Future),
//! fine $\pm 10\text{ Myr}/\pm 100\text{ Myr}$ step navigation, and live biosphere/ocean telemetry.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::geology::types::{
    EpochTargetPlanet, GeologicalEpoch, GeologicalState, TimelineScrubber,
};
use crate::simulation::resources::PlayerInteractionState;

use super::types::{
    create_compact_button, EpochPlanetButtonGroup, EpochPlanetTabButton, EpochQuickButton,
    EpochScrubberBodyTitleText, EpochScrubberMetricsText, EpochScrubberPanel,
    EpochScrubberStatusText, NotificationToast, UiButtonAction,
};

/// Spawns the glassmorphic Deep-Time Geological Epoch Scrubber drawer in the HUD root.
pub fn spawn_epoch_scrubber_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(368.0),
            top: Val::Px(52.0),
            width: Val::Px(420.0),
            max_height: Val::Vh(85.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            overflow: Overflow::clip_y(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.045, 0.96)),
        BorderColor::all(Color::srgba(0.85, 0.65, 0.25, 0.85)),
        EpochScrubberPanel,
    ))
    .with_children(|panel| {
        spawn_scrubber_header(panel);
        spawn_scrubber_target_row(panel);
        spawn_epoch_buttons_row(panel);
        spawn_step_navigation_row(panel);
        spawn_scrubber_diagnostics_section(panel);
        spawn_scrubber_footer(panel);
    });
}

fn spawn_scrubber_header(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::bottom(Val::Px(6.0)),
            ..default()
        })
        .with_children(|hdr| {
            hdr.spawn((
                Text::new("⏳ GEOLOGICAL EPOCH SCRUBBER"),
                TextFont {
                    font_size: FontSize::Px(12.5),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.40)),
                Pickable::IGNORE,
            ));

            hdr.spawn((
                Button,
                UiButtonAction::ToggleEpochScrubberPanel,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.20, 0.05, 0.05, 0.85)),
                BorderColor::all(Color::srgba(0.9, 0.3, 0.3, 0.7)),
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("✕"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.6, 0.6)),
                    Pickable::IGNORE,
                ));
            });
        });
}

fn spawn_scrubber_target_row(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
            margin: UiRect::bottom(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .insert((
            BackgroundColor(Color::srgba(0.04, 0.07, 0.12, 0.80)),
            BorderColor::all(Color::srgba(0.3, 0.5, 0.7, 0.4)),
        ))
        .with_children(|col| {
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Text::new("🪐 Target: Primary Terrestrial World"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.75, 0.90, 1.0)),
                    EpochScrubberBodyTitleText,
                    Pickable::IGNORE,
                ));

                row.spawn((
                    Text::new("[Click world or Tab to focus]"),
                    TextFont {
                        font_size: FontSize::Px(9.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 0.65, 0.8)),
                    Pickable::IGNORE,
                ));
            });

            // Planet switcher tabs
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|tab_row| {
                tab_row.spawn((
                    Text::new("TIMELINE:"),
                    TextFont {
                        font_size: FontSize::Px(9.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.75, 0.45)),
                    Pickable::IGNORE,
                ));

                let tabs = [
                    (
                        EpochTargetPlanet::Earth,
                        "🌍 Earth",
                        Color::srgba(0.06, 0.14, 0.22, 0.90),
                        Color::srgb(0.35, 0.75, 1.0),
                    ),
                    (
                        EpochTargetPlanet::Mars,
                        "🔴 Mars",
                        Color::srgba(0.22, 0.08, 0.06, 0.90),
                        Color::srgb(1.0, 0.45, 0.30),
                    ),
                    (
                        EpochTargetPlanet::Venus,
                        "♀ Venus",
                        Color::srgba(0.20, 0.14, 0.05, 0.90),
                        Color::srgb(1.0, 0.80, 0.35),
                    ),
                ];

                for (planet, label, bg, border) in tabs {
                    tab_row
                        .spawn((
                            Button,
                            UiButtonAction::SelectEpochTargetPlanet(planet),
                            EpochPlanetTabButton(planet),
                            Node {
                                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(bg),
                            BorderColor::all(border),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(label),
                                TextFont {
                                    font_size: FontSize::Px(9.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                                Pickable::IGNORE,
                            ));
                        });
                }
            });
        });
}

fn spawn_epoch_pill(
    parent: &mut ChildSpawnerCommands,
    epoch: GeologicalEpoch,
    label: &str,
    bg: Color,
    border: Color,
) {
    parent
        .spawn((
            Button,
            UiButtonAction::ScrubToEpoch(epoch),
            EpochQuickButton(epoch),
            Node {
                padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(border),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_epoch_buttons_row(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            margin: UiRect::bottom(Val::Px(6.0)),
            row_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new("EPOCH FAST-JUMP (PLANETARY TIMELINE):"),
                TextFont {
                    font_size: FontSize::Px(9.5),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.75, 0.45)),
                Pickable::IGNORE,
            ));

            spawn_earth_epoch_buttons(col);
            spawn_mars_epoch_buttons(col);
            spawn_venus_epoch_buttons(col);
        });
}

fn spawn_earth_epoch_buttons(col: &mut ChildSpawnerCommands) {
    col.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(3.0),
            row_gap: Val::Px(3.0),
            display: Display::Flex,
            ..default()
        },
        EpochPlanetButtonGroup(EpochTargetPlanet::Earth),
    ))
    .with_children(|row| {
        let earth_epochs = [
            (
                GeologicalEpoch::Hadean,
                "Hadean (4.56 Ga)",
                Color::srgba(0.22, 0.08, 0.05, 0.90),
                Color::srgb(1.0, 0.45, 0.25),
            ),
            (
                GeologicalEpoch::Archean,
                "Archean (3.5 Ga)",
                Color::srgba(0.12, 0.16, 0.08, 0.90),
                Color::srgb(0.70, 0.85, 0.35),
            ),
            (
                GeologicalEpoch::Proterozoic,
                "Proterozoic (1.5 Ga)",
                Color::srgba(0.18, 0.12, 0.06, 0.90),
                Color::srgb(0.95, 0.65, 0.30),
            ),
            (
                GeologicalEpoch::SnowballEarth,
                "Snowball (710 Ma)",
                Color::srgba(0.10, 0.18, 0.26, 0.90),
                Color::srgb(0.70, 0.90, 1.0),
            ),
            (
                GeologicalEpoch::Phanerozoic,
                "Phanerozoic (300 Ma)",
                Color::srgba(0.06, 0.18, 0.10, 0.90),
                Color::srgb(0.35, 0.95, 0.55),
            ),
            (
                GeologicalEpoch::Modern,
                "Modern (0 Ga)",
                Color::srgba(0.05, 0.14, 0.24, 0.90),
                Color::srgb(0.40, 0.85, 1.0),
            ),
            (
                GeologicalEpoch::Future,
                "Future (+500 Ma)",
                Color::srgba(0.20, 0.08, 0.18, 0.90),
                Color::srgb(0.95, 0.50, 0.85),
            ),
        ];
        for (epoch, label, bg, border) in earth_epochs {
            spawn_epoch_pill(row, epoch, label, bg, border);
        }
    });
}

fn spawn_mars_epoch_buttons(col: &mut ChildSpawnerCommands) {
    col.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(3.0),
            row_gap: Val::Px(3.0),
            display: Display::None,
            ..default()
        },
        EpochPlanetButtonGroup(EpochTargetPlanet::Mars),
    ))
    .with_children(|row| {
        let mars_epochs = [
            (
                GeologicalEpoch::MarsPreNoachian,
                "Pre-Noachian (4.5 Ga)",
                Color::srgba(0.25, 0.10, 0.06, 0.90),
                Color::srgb(1.0, 0.50, 0.30),
            ),
            (
                GeologicalEpoch::MarsNoachian,
                "Noachian Wet (3.9 Ga)",
                Color::srgba(0.08, 0.16, 0.20, 0.90),
                Color::srgb(0.40, 0.85, 0.95),
            ),
            (
                GeologicalEpoch::MarsHesperian,
                "Hesperian (3.4 Ga)",
                Color::srgba(0.20, 0.12, 0.05, 0.90),
                Color::srgb(0.95, 0.65, 0.25),
            ),
            (
                GeologicalEpoch::MarsAmazonian,
                "Amazonian (0 Ga)",
                Color::srgba(0.24, 0.07, 0.05, 0.90),
                Color::srgb(1.0, 0.40, 0.25),
            ),
            (
                GeologicalEpoch::MarsFuture,
                "Future (+500 Ma)",
                Color::srgba(0.18, 0.08, 0.15, 0.90),
                Color::srgb(0.85, 0.45, 0.75),
            ),
        ];
        for (epoch, label, bg, border) in mars_epochs {
            spawn_epoch_pill(row, epoch, label, bg, border);
        }
    });
}

fn spawn_venus_epoch_buttons(col: &mut ChildSpawnerCommands) {
    col.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(3.0),
            row_gap: Val::Px(3.0),
            display: Display::None,
            ..default()
        },
        EpochPlanetButtonGroup(EpochTargetPlanet::Venus),
    ))
    .with_children(|row| {
        let venus_epochs = [
            (
                GeologicalEpoch::VenusPrimordial,
                "Primordial (4.5 Ga)",
                Color::srgba(0.28, 0.08, 0.04, 0.90),
                Color::srgb(1.0, 0.45, 0.20),
            ),
            (
                GeologicalEpoch::VenusTemperate,
                "Temperate Ocean (2.5 Ga)",
                Color::srgba(0.08, 0.18, 0.16, 0.90),
                Color::srgb(0.30, 0.90, 0.75),
            ),
            (
                GeologicalEpoch::VenusRunaway,
                "Runaway (1.2 Ga)",
                Color::srgba(0.22, 0.14, 0.04, 0.90),
                Color::srgb(0.95, 0.70, 0.20),
            ),
            (
                GeologicalEpoch::VenusModern,
                "Modern (0 Ga)",
                Color::srgba(0.22, 0.15, 0.02, 0.90),
                Color::srgb(1.0, 0.85, 0.25),
            ),
            (
                GeologicalEpoch::VenusFuture,
                "Future (+500 Ma)",
                Color::srgba(0.20, 0.06, 0.08, 0.90),
                Color::srgb(0.90, 0.35, 0.45),
            ),
        ];
        for (epoch, label, bg, border) in venus_epochs {
            spawn_epoch_pill(row, epoch, label, bg, border);
        }
    });
}

fn spawn_step_navigation_row(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::bottom(Val::Px(8.0)),
            column_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|row| {
            create_compact_button(
                row,
                UiButtonAction::ScrubTimeStep(-100),
                "⏪ -100M",
                Color::srgba(0.12, 0.08, 0.15, 0.90),
                Color::srgb(0.70, 0.50, 0.85),
            );
            create_compact_button(
                row,
                UiButtonAction::ScrubTimeStep(-10),
                "◀ -10M",
                Color::srgba(0.10, 0.10, 0.16, 0.90),
                Color::srgb(0.50, 0.70, 1.0),
            );
            create_compact_button(
                row,
                UiButtonAction::ToggleScrubAutoAdvance,
                "⏱ Auto / Hold",
                Color::srgba(0.18, 0.14, 0.06, 0.90),
                Color::srgb(1.0, 0.80, 0.30),
            );
            create_compact_button(
                row,
                UiButtonAction::ScrubTimeStep(10),
                "▶ +10M",
                Color::srgba(0.10, 0.10, 0.16, 0.90),
                Color::srgb(0.50, 0.70, 1.0),
            );
            create_compact_button(
                row,
                UiButtonAction::ScrubTimeStep(100),
                "⏩ +100M",
                Color::srgba(0.12, 0.08, 0.15, 0.90),
                Color::srgb(0.70, 0.50, 0.85),
            );
        });
}

fn spawn_scrubber_diagnostics_section(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            margin: UiRect::bottom(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            row_gap: Val::Px(5.0),
            ..default()
        })
        .insert((
            BackgroundColor(Color::srgba(0.015, 0.035, 0.065, 0.92)),
            BorderColor::all(Color::srgba(0.35, 0.65, 0.90, 0.55)),
        ))
        .with_children(|diag| {
            diag.spawn((
                Text::new("AGE: 4.560 Ga (Elapsed: 0.000 Gyr) | Epoch: Modern Holocene\nSUPERCONTINENT: Dispersed 7 Continents | Wilson Cycle: 0.12"),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.92, 0.70)),
                EpochScrubberStatusText,
                Pickable::IGNORE,
            ));

            diag.spawn((
                Text::new("🌊 Ocean: 100.0% Oxidized (Sapphire Blue)\n💨 Oxygen: 1.00 PAL (21.0% O₂)\n🌿 Terrestrial Flora: 100.0% (Lush Biomes)\n🌋 Volcanism & Orogeny: 45%"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.90, 1.0)),
                EpochScrubberMetricsText,
                Pickable::IGNORE,
            ));
        });
}

fn spawn_scrubber_footer(panel: &mut ChildSpawnerCommands) {
    panel.spawn((
        Text::new("✦ CONTINENTAL DRIFT & WILSON CYCLES: Plate advection shaders procedurally deform landmasses across ~500 Myr supercontinent cycles (Kenorland → Rodinia → Pangea → Pangea Ultima). Ocean oxidation simulates the Great Oxidation Event, and land biomes dynamically vegetate cratons."),
        TextFont {
            font_size: FontSize::Px(9.0),
            ..default()
        },
        TextColor(Color::srgb(0.55, 0.70, 0.85)),
        Pickable::IGNORE,
    ));
}

/// Dispatches interactive button actions for the Epoch Scrubber panel.
pub fn handle_epoch_scrubber_action(
    action: &UiButtonAction,
    opt_scrubber: Option<&mut TimelineScrubber>,
    toast: &mut NotificationToast,
) -> bool {
    let Some(scrubber) = opt_scrubber else {
        return false;
    };

    match action {
        UiButtonAction::ToggleEpochScrubberPanel => {
            scrubber.is_open = !scrubber.is_open;
            toast.message = if scrubber.is_open {
                "⏳ Geological Epoch Scrubber Opened [F11]".to_string()
            } else {
                "⏳ Geological Epoch Scrubber Closed [F11]".to_string()
            };
            toast.timer = 2.5;
            true
        }
        UiButtonAction::SelectEpochTargetPlanet(planet) => {
            if scrubber.target_planet != *planet {
                scrubber.target_planet = *planet;
                scrubber.target_entity = None;
            }
            false
        }
        UiButtonAction::ScrubToEpoch(epoch) => {
            scrubber.auto_advance = false;
            let new_planet = epoch.target_planet();
            if scrubber.target_planet != new_planet {
                scrubber.target_planet = new_planet;
                scrubber.target_entity = None;
            }
            scrubber.active_epoch = *epoch;
            scrubber.scrubbed_age_gyr = epoch.canonical_age_gyr();
            toast.message = format!(
                "⏳ Scrubbed to {} ({:.2} Ga canonical age)",
                epoch.name(),
                epoch.canonical_age_gyr()
            );
            toast.timer = 2.5;
            true
        }
        UiButtonAction::ScrubTimeStep(myr) => {
            scrubber.auto_advance = false;
            let delta_gyr = (*myr as f32) / 1000.0;
            scrubber.scrubbed_age_gyr = (scrubber.scrubbed_age_gyr + delta_gyr).clamp(0.0, 5.50);
            scrubber.active_epoch = GeologicalEpoch::from_age_and_planet(
                scrubber.scrubbed_age_gyr,
                scrubber.target_planet,
            );
            toast.message = format!(
                "⏳ Jumped {:+} Myr -> {:.3} Ga ({})",
                myr,
                scrubber.scrubbed_age_gyr,
                scrubber.active_epoch.short_name()
            );
            toast.timer = 2.0;
            true
        }
        UiButtonAction::ToggleScrubAutoAdvance => {
            scrubber.auto_advance = !scrubber.auto_advance;
            toast.message = if scrubber.auto_advance {
                "⏳ Geological Timeline: Simulation Auto-Advance Active".to_string()
            } else {
                "⏳ Geological Timeline: Manual Scrubber Locked".to_string()
            };
            toast.timer = 2.5;
            true
        }
        _ => false,
    }
}

/// Updates the Deep-Time Geological Epoch Scrubber UI labels, highlights, and telemetry diagnostics.
#[allow(
    clippy::type_complexity,
    reason = "Disjoint text and node query bundles for epoch scrubber HUD"
)]
pub fn update_epoch_scrubber_ui(
    mut opt_scrubber: Option<ResMut<TimelineScrubber>>,
    player_state: Res<PlayerInteractionState>,
    mut panel_query: Query<&mut Node, With<EpochScrubberPanel>>,
    mut body_title_query: Query<
        &mut Text,
        (
            With<EpochScrubberBodyTitleText>,
            Without<EpochScrubberStatusText>,
            Without<EpochScrubberMetricsText>,
        ),
    >,
    mut status_query: Query<
        &mut Text,
        (
            With<EpochScrubberStatusText>,
            Without<EpochScrubberBodyTitleText>,
            Without<EpochScrubberMetricsText>,
        ),
    >,
    mut metrics_query: Query<
        &mut Text,
        (
            With<EpochScrubberMetricsText>,
            Without<EpochScrubberBodyTitleText>,
            Without<EpochScrubberStatusText>,
        ),
    >,
    mut epoch_btn_query: Query<(&EpochQuickButton, &mut BorderColor)>,
    mut planet_tab_query: Query<
        (&EpochPlanetTabButton, &mut BorderColor),
        Without<EpochQuickButton>,
    >,
    mut button_group_query: Query<
        (&EpochPlanetButtonGroup, &mut Node),
        Without<EpochScrubberPanel>,
    >,
    geo_query: Query<(Entity, &CelestialBody, &GeologicalState)>,
) {
    let Some(ref mut scrubber) = opt_scrubber else {
        return;
    };
    let Ok(mut panel_node) = panel_query.single_mut() else {
        return;
    };

    if !scrubber.is_open {
        panel_node.display = Display::None;
        return;
    }
    panel_node.display = Display::Flex;

    let Some((_ent, body, geo)) =
        resolve_epoch_scrubber_target(scrubber, &player_state, &geo_query)
    else {
        if let Ok(mut title) = body_title_query.single_mut() {
            title.0 = "🪐 Target: No Terrestrial Worlds Found".to_string();
        }
        return;
    };

    // 2. Toggle button groups for the active planet timeline
    for (group, mut node) in button_group_query.iter_mut() {
        if group.0 == scrubber.target_planet {
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }

    // 3. Highlight active planet tab
    for (tab, mut border) in planet_tab_query.iter_mut() {
        if tab.0 == scrubber.target_planet {
            *border = BorderColor::all(Color::srgb(1.0, 0.85, 0.40));
        } else {
            *border = BorderColor::all(Color::srgba(0.35, 0.55, 0.75, 0.45));
        }
    }

    // 4. Highlight active epoch button
    for (btn, mut border) in epoch_btn_query.iter_mut() {
        if btn.0 == scrubber.active_epoch {
            *border = BorderColor::all(Color::srgb(1.0, 0.85, 0.40));
        } else {
            *border = BorderColor::all(Color::srgba(0.35, 0.55, 0.75, 0.45));
        }
    }

    let type_label = match body.body_type {
        BodyType::TerrestrialPlanet => "Terrestrial Planet",
        BodyType::SuperEarth => "Super-Earth",
        BodyType::Protoplanet => "Protoplanet Embryo",
        _ => "Planetary Body",
    };

    // 5. Update body title
    if let Ok(mut title) = body_title_query.single_mut() {
        title.0 = format!(
            "🪐 Target: {} ({}) | Age: {:.3} Ga",
            body.name, type_label, geo.geological_age_gyr
        );
    }

    // 6. Update epoch & geology status
    if let Ok(mut status) = status_query.single_mut() {
        let clock_mode = if scrubber.auto_advance {
            "AUTO"
        } else {
            "HOLD"
        };
        status.0 = format!(
            "AGE: {:.3} Ga | Epoch: {} [{}]\nGEOLOGY: {} (Wilson/Tectonic Cycle: {:.2}, Clustered: {:.0}%)",
            geo.geological_age_gyr,
            geo.epoch.name(),
            clock_mode,
            geo.epoch.supercontinent_name(),
            geo.continental_drift_phase,
            geo.supercontinent_aggregation * 100.0
        );
    }

    // 7. Update planet-specific telemetry metrics text
    if let Ok(mut metrics) = metrics_query.single_mut() {
        metrics.0 = format_epoch_telemetry_metrics(scrubber.target_planet, geo);
    }
}

fn resolve_epoch_scrubber_target<'a>(
    scrubber: &mut TimelineScrubber,
    player_state: &PlayerInteractionState,
    geo_query: &'a Query<(Entity, &CelestialBody, &GeologicalState)>,
) -> Option<(Entity, &'a CelestialBody, &'a GeologicalState)> {
    let existing_target = scrubber
        .target_entity
        .and_then(|ent| geo_query.get(ent).ok());

    let target_entry = if let Some(entry) = existing_target {
        if matches!(
            entry.1.body_type,
            BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
        ) && EpochTargetPlanet::try_detect_from_name(&entry.1.name)
            == Some(scrubber.target_planet)
        {
            Some(entry)
        } else {
            geo_query.iter().find(|(_, b, _)| {
                matches!(
                    b.body_type,
                    BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
                ) && EpochTargetPlanet::try_detect_from_name(&b.name)
                    == Some(scrubber.target_planet)
            })
        }
    } else {
        geo_query
            .iter()
            .find(|(_, b, _)| {
                matches!(
                    b.body_type,
                    BodyType::TerrestrialPlanet | BodyType::SuperEarth | BodyType::Protoplanet
                ) && EpochTargetPlanet::try_detect_from_name(&b.name)
                    == Some(scrubber.target_planet)
            })
            .or_else(|| {
                player_state
                    .selected_entity
                    .and_then(|sel| geo_query.get(sel).ok())
                    .filter(|(_, b, _)| EpochTargetPlanet::try_detect_from_name(&b.name).is_some())
            })
    };

    if let Some((ent, body, geo)) = target_entry {
        if scrubber.target_entity != Some(ent) {
            scrubber.target_entity = Some(ent);
            if let Some(archetype) = EpochTargetPlanet::try_detect_from_name(&body.name) {
                scrubber.target_planet = archetype;
            }
            if scrubber.auto_advance {
                scrubber.active_epoch = geo.epoch;
                scrubber.scrubbed_age_gyr = geo.geological_age_gyr;
            }
        }
    } else {
        scrubber.target_entity = None;
    }

    target_entry
}

fn format_epoch_telemetry_metrics(planet: EpochTargetPlanet, geo: &GeologicalState) -> String {
    match planet {
        EpochTargetPlanet::Earth => format_earth_telemetry(geo),
        EpochTargetPlanet::Mars => format_mars_telemetry(geo),
        EpochTargetPlanet::Venus => format_venus_telemetry(geo),
    }
}

fn format_earth_telemetry(geo: &GeologicalState) -> String {
    let ocean_desc = if geo.ocean_oxidation_progress < 0.15 {
        "Anoxic Green Water (Fe²⁺)"
    } else if geo.ocean_oxidation_progress < 0.85 {
        "Banded Iron Precipitation"
    } else {
        "Sapphire Blue Oxidized Ocean"
    };

    let plant_desc = if geo.terrestrial_vegetation_fraction < 0.05 {
        "Barren Craton / Sterile Rock"
    } else if geo.terrestrial_vegetation_fraction < 0.50 {
        "Algal / Early Silurian Flora"
    } else {
        "Lush Terrestrial Biomes"
    };

    format!(
        "🌊 Ocean Oxidation: {:.1}% ({})\n💨 Oxygen Level: {:.2} PAL ({:.1}% O₂)\n🌿 Terrestrial Flora: {:.1}% ({})\n🌋 Volcanism & Orogeny: {:.0}%",
        geo.ocean_oxidation_progress * 100.0,
        ocean_desc,
        geo.oxygen_level_pal,
        geo.oxygen_level_pal * 21.0,
        geo.terrestrial_vegetation_fraction * 100.0,
        plant_desc,
        geo.volcanic_orogeny_activity * 100.0
    )
}

fn format_mars_telemetry(geo: &GeologicalState) -> String {
    let water_desc = match geo.epoch {
        GeologicalEpoch::MarsNoachian => "Oceanus Borealis (Northern Ocean)",
        GeologicalEpoch::MarsHesperian => "Acidic Outflow Seas & Frozen Lakes",
        GeologicalEpoch::MarsPreNoachian => "Hydrothermal Basins & Hot Springs",
        _ => "Hyper-Arid Rust Desert & Ice Caps",
    };
    let surface_desc = match geo.epoch {
        GeologicalEpoch::MarsHesperian => "Massive Tharsis Volcanism & Orogeny",
        GeologicalEpoch::MarsNoachian => "Active River Deltas & Valley Networks",
        _ => "Oxidized Iron Regolith (Fe₂O₃)",
    };
    format!(
        "🌊 Hydrosphere: {}\n💨 Atmosphere: Primordial / CO₂ Envelope\n🏜 Surface State: {}\n🌋 Volcanism & Tectonics: {:.0}%",
        water_desc,
        surface_desc,
        geo.volcanic_orogeny_activity * 100.0
    )
}

fn format_venus_telemetry(geo: &GeologicalState) -> String {
    let atm_desc = match geo.epoch {
        GeologicalEpoch::VenusModern | GeologicalEpoch::VenusFuture => {
            "92-Bar Supercritical CO₂ Hellscape"
        }
        GeologicalEpoch::VenusRunaway => "Runaway Moist Steam Envelope",
        GeologicalEpoch::VenusTemperate => "1.5-Bar Temperate Habitable Atmosphere",
        _ => "Primordial 40-Bar Steam Volatiles",
    };
    let cloud_desc = match geo.epoch {
        GeologicalEpoch::VenusModern | GeologicalEpoch::VenusFuture => {
            "Opaque 100% Sulfuric Acid Cloud Deck"
        }
        GeologicalEpoch::VenusTemperate => "Convective Day-Side Water Vapor Clouds",
        _ => "Dense Volcanic Outgassing Clouds",
    };
    format!(
        "💨 Atmosphere: {}\n☁ Cloud Deck: {}\n🌡 Climate Regime: Extreme Greenhouse\n🌋 Basaltic Resurfacing: {:.0}%",
        atm_desc,
        cloud_desc,
        geo.volcanic_orogeny_activity * 100.0
    )
}
