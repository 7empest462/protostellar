//! Telemetry & Climate Graphing Panel (`[F10]`) UI spawner, button dispatcher, and live updater.

use bevy::prelude::*;

use crate::simulation::telemetry::{SimulationTelemetryHistory, TelemetryMetric};

use super::types::*;

/// Spawns the floating glassmorphic Telemetry & Climate Graphing Panel drawer.
pub fn spawn_telemetry_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Button,
        UiButtonAction::ToggleMinimizeTelemetryPanel,
        HudPanelElement::TelemetryPanelPill,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(368.0),
            top: Val::Px(52.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.028, 0.065, 0.92)),
        BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.85)),
    ))
    .with_children(|pill| {
        pill.spawn((
            Text::new("📈 Telemetry [F10] ▶"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.4, 0.85, 1.0)),
            Pickable::IGNORE,
        ));
    });

    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(368.0),
            top: Val::Px(52.0),
            width: Val::Px(385.0),
            max_height: Val::Vh(82.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            overflow: Overflow::clip_y(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.028, 0.065, 0.96)),
        BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.85)),
        TelemetryGraphPanel,
    ))
    .with_children(|panel| {
        spawn_telemetry_header_and_title(panel);
        spawn_telemetry_metric_selector_pills(panel);
        spawn_telemetry_readout_and_graph(panel);
        spawn_telemetry_footer(panel);
    });
}

fn spawn_telemetry_header_and_title(panel: &mut ChildSpawnerCommands) {
    // 1. Header Bar with Close Button
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
                Text::new("📈 CLIMATE & HABITABILITY TELEMETRY"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.96, 1.0)),
                Pickable::IGNORE,
            ));
            hdr.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|btns| {
                create_compact_button(
                    btns,
                    UiButtonAction::ToggleMinimizeTelemetryPanel,
                    "🗕",
                    Color::srgba(0.14, 0.08, 0.16, 0.85),
                    Color::srgb(0.9, 0.4, 0.6),
                );
                create_compact_button(
                    btns,
                    UiButtonAction::ToggleTelemetryPanel,
                    "✕ Close [F10]",
                    Color::srgba(0.24, 0.08, 0.08, 0.9),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });
        });

    // 2. Tracked Celestial Body & Buffer Count
    panel.spawn((
        Text::new("Tracking: Proto-Earth | Buffer: 0 / 128 samples"),
        TextFont {
            font_size: FontSize::Px(10.5),
            ..default()
        },
        TextColor(Color::srgb(0.45, 0.85, 1.0)),
        TelemetryGraphBodyTitleText,
        Pickable::IGNORE,
    ));
}

fn spawn_telemetry_metric_selector_pills(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            margin: UiRect::axes(Val::Px(0.0), Val::Px(5.0)),
            ..default()
        })
        .with_children(|row| {
            let metrics = [
                (TelemetryMetric::Habitability, "🌱 Habitability"),
                (TelemetryMetric::Temperature, "🌡️ Temp (K)"),
                (TelemetryMetric::OceanCoverage, "🌊 Oceans (%)"),
                (TelemetryMetric::AtmosphericPressure, "💨 Atm (bar)"),
                (TelemetryMetric::Orbit, "궤 Orbit (AU)"),
            ];

            for (metric, label) in metrics {
                row.spawn((
                    Button,
                    UiButtonAction::SelectTelemetryMetric(metric),
                    TelemetryMetricButton(metric),
                    Node {
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
                        margin: UiRect::all(Val::Px(1.5)),
                        border: UiRect::all(Val::Px(1.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.2, 0.4, 0.6, 0.5)),
                    BackgroundColor(Color::srgba(0.04, 0.08, 0.14, 0.85)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(label),
                        TextFont {
                            font_size: FontSize::Px(9.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.92, 1.0)),
                        Pickable::IGNORE,
                    ));
                });
            }
        });
}

fn spawn_telemetry_readout_and_graph(panel: &mut ChildSpawnerCommands) {
    // 4. Live Metric Numerical Readouts Box
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(6.0)),
            margin: UiRect::bottom(Val::Px(5.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        })
        .insert((
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.88)),
            BorderColor::all(Color::srgba(0.2, 0.5, 0.8, 0.4)),
        ))
        .with_children(|stats_box| {
            stats_box.spawn((
                Text::new("Recording initial telemetry epoch..."),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.95, 1.0)),
                TelemetryGraphReadoutText,
                Pickable::IGNORE,
            ));
        });

    // 5. Monospace Sparkline Graph Display Box
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(6.0)),
            margin: UiRect::bottom(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        })
        .insert((
            BackgroundColor(Color::srgba(0.005, 0.012, 0.025, 0.95)),
            BorderColor::all(Color::srgba(0.25, 0.65, 0.9, 0.5)),
        ))
        .with_children(|graph_box| {
            graph_box.spawn((
                Text::new(" ▂▃▅▆▇█\nGathering orbital snapshots..."),
                TextFont {
                    font_size: FontSize::Px(12.5),
                    ..default()
                },
                TextColor(Color::srgb(0.35, 0.95, 0.70)),
                TelemetryGraphSparklineText,
                Pickable::IGNORE,
            ));
        });
}

fn spawn_telemetry_footer(panel: &mut ChildSpawnerCommands) {
    // 6. CSV Exporter Button and Status Footer
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .with_children(|footer| {
            create_compact_button(
                footer,
                UiButtonAction::ExportTelemetryCsv,
                "📊 Export CSV [./exports/]",
                Color::srgba(0.05, 0.22, 0.15, 0.9),
                Color::srgb(0.3, 0.9, 0.6),
            );

            footer.spawn((
                Text::new("CSV: time, a, e, T, ocean, P, bio"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.7, 0.9)),
                TelemetryExportStatusText,
                Pickable::IGNORE,
            ));
        });
}

/// Handles click actions for the Telemetry Graph panel and CSV exporter.
pub fn handle_telemetry_action(
    action: &UiButtonAction,
    panel_state: &mut TelemetryPanelState,
    telemetry_history: &mut SimulationTelemetryHistory,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::ToggleTelemetryPanel => {
            panel_state.is_open = !panel_state.is_open;
            toast.message = if panel_state.is_open {
                "📈 Telemetry & Habitability Graph Opened [F10]".to_string()
            } else {
                "📈 Telemetry Graph Closed [F10]".to_string()
            };
            toast.timer = 2.5;
            true
        }
        UiButtonAction::SelectTelemetryMetric(metric) => {
            panel_state.selected_metric = *metric;
            toast.message = format!("📈 Telemetry Metric: {}", metric.label());
            toast.timer = 2.0;
            true
        }
        UiButtonAction::ExportTelemetryCsv => {
            match telemetry_history.export_to_csv_file(None) {
                Ok(path) => {
                    let status_msg = format!(
                        "Exported {} samples to {path}",
                        telemetry_history.sample_count()
                    );
                    panel_state.last_export_status = Some(status_msg.clone());
                    toast.message = format!("📁 {status_msg}");
                    toast.timer = 4.5;
                }
                Err(err) => {
                    let err_msg = format!("CSV export failed: {err}");
                    panel_state.last_export_status = Some(err_msg.clone());
                    toast.message = format!("⚠️ {err_msg}");
                    toast.timer = 4.0;
                }
            }
            true
        }
        _ => false,
    }
}

/// Updates the dynamic readouts, sparkline graph, and button highlight in the Telemetry HUD.
#[allow(
    clippy::type_complexity,
    reason = "Telemetry UI query references multiple distinct text marker components"
)]
pub fn update_telemetry_graph_ui(
    panel_state: Res<TelemetryPanelState>,
    telemetry_history: Res<SimulationTelemetryHistory>,
    mut panel_query: Query<&mut Node, With<TelemetryGraphPanel>>,
    mut body_title_query: Query<
        &mut Text,
        (
            With<TelemetryGraphBodyTitleText>,
            Without<TelemetryGraphReadoutText>,
            Without<TelemetryGraphSparklineText>,
            Without<TelemetryExportStatusText>,
        ),
    >,
    mut readout_query: Query<
        &mut Text,
        (
            With<TelemetryGraphReadoutText>,
            Without<TelemetryGraphBodyTitleText>,
            Without<TelemetryGraphSparklineText>,
            Without<TelemetryExportStatusText>,
        ),
    >,
    mut sparkline_query: Query<
        &mut Text,
        (
            With<TelemetryGraphSparklineText>,
            Without<TelemetryGraphBodyTitleText>,
            Without<TelemetryGraphReadoutText>,
            Without<TelemetryExportStatusText>,
        ),
    >,
    mut export_status_query: Query<
        &mut Text,
        (
            With<TelemetryExportStatusText>,
            Without<TelemetryGraphBodyTitleText>,
            Without<TelemetryGraphReadoutText>,
            Without<TelemetryGraphSparklineText>,
        ),
    >,
    mut metric_btn_query: Query<(
        &TelemetryMetricButton,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    let Ok(mut panel_node) = panel_query.single_mut() else {
        return;
    };

    if !panel_state.is_open {
        panel_node.display = Display::None;
        return;
    }
    panel_node.display = Display::Flex;

    // 1. Update metric button highlight colors
    for (btn_metric, mut bg, mut border) in metric_btn_query.iter_mut() {
        if btn_metric.0 == panel_state.selected_metric {
            *bg = BackgroundColor(Color::srgba(0.12, 0.35, 0.45, 0.95));
            *border = BorderColor::all(Color::srgb(0.4, 0.95, 1.0));
        } else {
            *bg = BackgroundColor(Color::srgba(0.04, 0.08, 0.14, 0.85));
            *border = BorderColor::all(Color::srgba(0.2, 0.4, 0.6, 0.5));
        }
    }

    // 2. Update Tracked Celestial Body Header
    if let Ok(mut title_text) = body_title_query.single_mut() {
        title_text.0 = format!(
            "Tracking: {} | Buffer: {} / {} samples",
            telemetry_history.tracked_name,
            telemetry_history.sample_count(),
            telemetry_history.max_samples
        );
    }

    // 3. Update Numeric Readout Statistics
    let metric = panel_state.selected_metric;
    let (latest, min, max, mean) = telemetry_history.get_metric_stats(metric);

    if let Ok(mut readout_text) = readout_query.single_mut() {
        let (val_fmt, unit) = match metric {
            TelemetryMetric::Habitability => (
                format!("Current: {latest:.1}% | Mean: {mean:.1}%"),
                format!("Range: [{min:.1}% - {max:.1}%] | (0% Sterile - 100% Eden)"),
            ),
            TelemetryMetric::Temperature => (
                format!(
                    "Current: {latest:.1} K ({:.1} °C) | Mean: {mean:.1} K",
                    latest - 273.15
                ),
                format!("Range: [{min:.1} K - {max:.1} K]"),
            ),
            TelemetryMetric::OceanCoverage => (
                format!("Current: {latest:.1}% | Mean: {mean:.1}%"),
                format!("Range: [{min:.1}% - {max:.1}%] Surface Liquid H₂O"),
            ),
            TelemetryMetric::AtmosphericPressure => (
                format!("Current: {latest:.2} bar | Mean: {mean:.2} bar"),
                format!("Range: [{min:.2} bar - {max:.2} bar] (1 bar = 1 atm)"),
            ),
            TelemetryMetric::Orbit => (
                format!("Current Semi-Major Axis: {latest:.3} AU"),
                format!("Range: [{min:.3} AU - {max:.3} AU] | Mean: {mean:.3} AU"),
            ),
        };

        readout_text.0 = format!(">> {val_fmt}\n>> {unit}");
    }

    // 4. Update Unicode Sparkline Graph
    if let Ok(mut sparkline_text) = sparkline_query.single_mut() {
        let sparkline = telemetry_history.generate_sparkline(metric, 36);
        sparkline_text.0 = format!(
            "{sparkline}\n[History: {} samples]",
            telemetry_history.sample_count()
        );
    }

    // 5. Update Export Feedback Text
    if let Ok(mut export_text) = export_status_query.single_mut() {
        if let Some(ref status) = telemetry_history.last_export_status {
            export_text.0.clone_from(status);
        }
    }
}
