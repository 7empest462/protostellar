//! Interactive Hertzsprung–Russell (H-R) Stellar Evolution Diagram Window (`[Shift-R]`).
//!
//! Visualizes the central host star's live evolutionary trajectory across:
//! - Pre-Main Sequence (Hayashi track gravitational contraction)
//! - Zero-Age Main Sequence (ZAMS) and Main Sequence hydrogen burning
//! - Red Giant Branch (RGB) / Asymptotic Giant Branch (AGB)
//! - Degenerate White Dwarf cooling track
//!
//! Real-time 2D plot maps bolometric luminosity log10(L / L_sun) against surface
//! effective temperature log10(T_eff) and Morgan-Keenan spectral classification (O-B-A-F-G-K-M).

use std::fmt::Write;

use bevy::prelude::*;

use crate::simulation::components::{
    BodyType, CelestialBody, CentralStar, IgnitionState, Luminosity, Mass, Radius,
    StellarEvolutionPhase, StellarEvolutionState, Temperature,
};
use crate::utils::constants::SOLAR_RADIUS_AU;

use super::types::*;

/// Spawns the floating Hertzsprung–Russell diagram drawer and minimized pill.
pub fn spawn_hr_diagram_panel(root: &mut ChildSpawnerCommands) {
    // 1. Minimized floating pill button
    root.spawn((
        Button,
        UiButtonAction::ToggleMinimizeHrDiagram,
        HudPanelElement::HrDiagramPanelPill,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(762.0),
            top: Val::Px(52.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.05, 0.92)),
        BorderColor::all(Color::srgba(1.0, 0.75, 0.35, 0.85)),
    ))
    .with_children(|pill| {
        pill.spawn((
            Text::new("🌟 H-R Diagram [Shift-R] ▶"),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.85, 0.45)),
            Pickable::IGNORE,
        ));
    });

    // 2. Main floating glassmorphic window
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(762.0),
            top: Val::Px(52.0),
            width: Val::Px(410.0),
            max_height: Val::Vh(84.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.5)),
            display: Display::None,
            overflow: Overflow::clip_y(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.024, 0.055, 0.96)),
        BorderColor::all(Color::srgba(1.0, 0.75, 0.35, 0.85)),
        HrDiagramPanel,
    ))
    .with_children(|panel| {
        spawn_hr_header(panel);
        spawn_spectral_banner(panel);
        spawn_plot_container(panel);
        spawn_details_container(panel);
    });
}

fn spawn_hr_header(panel: &mut ChildSpawnerCommands) {
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
                Text::new("🌟 HERTZSPRUNG–RUSSELL DIAGRAM"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.88, 0.55)),
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
                    UiButtonAction::ToggleMinimizeHrDiagram,
                    "🗕",
                    Color::srgba(0.18, 0.12, 0.05, 0.85),
                    Color::srgb(1.0, 0.8, 0.4),
                );
                create_compact_button(
                    btns,
                    UiButtonAction::ToggleHrDiagram,
                    "✕ Close",
                    Color::srgba(0.24, 0.08, 0.08, 0.9),
                    Color::srgb(1.0, 0.4, 0.4),
                );
            });
        });
}

fn spawn_spectral_banner(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.5)),
            margin: UiRect::bottom(Val::Px(5.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        })
        .insert((
            BackgroundColor(Color::srgba(0.04, 0.06, 0.12, 0.85)),
            BorderColor::all(Color::srgba(0.4, 0.6, 0.9, 0.4)),
        ))
        .with_children(|row| {
            const CLASSES: [(&str, Color); 7] = [
                ("O (Blue)", Color::srgb(0.45, 0.65, 1.0)),
                ("B (Cyan)", Color::srgb(0.65, 0.85, 1.0)),
                ("A (White)", Color::srgb(0.95, 0.98, 1.0)),
                ("F (Yel-Wh)", Color::srgb(1.0, 1.0, 0.85)),
                ("G (Sun)", Color::srgb(1.0, 0.92, 0.45)),
                ("K (Orange)", Color::srgb(1.0, 0.65, 0.3)),
                ("M (Red)", Color::srgb(1.0, 0.35, 0.3)),
            ];
            for (label, col) in CLASSES {
                row.spawn((
                    Text::new(label),
                    TextFont {
                        font_size: FontSize::Px(9.0),
                        ..default()
                    },
                    TextColor(col),
                    Pickable::IGNORE,
                ));
            }
        });
}

fn spawn_plot_container(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(6.0)),
            margin: UiRect::bottom(Val::Px(6.0)),
            border: UiRect::left(Val::Px(3.0)),
            ..default()
        })
        .insert((
            BackgroundColor(Color::srgba(0.005, 0.010, 0.022, 0.95)),
            BorderColor::all(Color::srgba(1.0, 0.75, 0.35, 0.5)),
        ))
        .with_children(|box_node| {
            box_node.spawn((
                Text::new("Gathering stellar bolometric telemetry..."),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.85, 0.55)),
                HrDiagramPlotText,
                Pickable::IGNORE,
            ));
        });
}

fn spawn_details_container(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(6.0)),
            border: UiRect::left(Val::Px(3.0)),
            ..default()
        })
        .insert((
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.90)),
            BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.45)),
        ))
        .with_children(|box_node| {
            box_node.spawn((
                Text::new("Awaiting host star data..."),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.92, 1.0)),
                HrDiagramDetailsText,
                Pickable::IGNORE,
            ));
        });
}

/// Dispatches user click actions for the H-R Diagram panel.
pub fn handle_hr_diagram_action(
    action: &UiButtonAction,
    state: &mut HrDiagramPanelState,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::ToggleHrDiagram => {
            state.is_open = !state.is_open;
            toast.message = if state.is_open {
                "🌟 Hertzsprung–Russell Stellar Diagram Opened [Shift-R]".to_string()
            } else {
                "🌟 H-R Diagram Closed [Shift-R]".to_string()
            };
            toast.timer = 2.5;
            true
        }
        UiButtonAction::ToggleMinimizeHrDiagram => {
            state.is_minimized = !state.is_minimized;
            true
        }
        _ => false,
    }
}

/// Updates the 2D H-R diagram and stellar telemetry readouts each frame.
#[allow(clippy::type_complexity, reason = "Star query bundle for astrophysics")]
pub fn update_hr_diagram_ui(
    state: Res<HrDiagramPanelState>,
    star_query: Query<
        (
            &CelestialBody,
            &Mass,
            Option<&Radius>,
            Option<&Temperature>,
            Option<&Luminosity>,
            Option<&IgnitionState>,
            Option<&StellarEvolutionState>,
        ),
        With<CentralStar>,
    >,
    mut plot_text: Query<&mut Text, (With<HrDiagramPlotText>, Without<HrDiagramDetailsText>)>,
    mut details_text: Query<&mut Text, (With<HrDiagramDetailsText>, Without<HrDiagramPlotText>)>,
) {
    if !state.is_open || state.is_minimized {
        return;
    }

    let Some((body, mass, opt_rad, opt_temp, opt_lum, opt_ign, opt_evo)) = star_query.iter().next()
    else {
        return;
    };

    let temp_val = opt_temp.map_or(5778.0, |t| t.0).max(100.0);
    let lum_val = opt_lum.map_or_else(
        || {
            let r_solar = opt_rad.map_or(1.0, |r| r.0 / SOLAR_RADIUS_AU);
            let t_ratio = temp_val / 5778.0;
            (r_solar * r_solar * t_ratio.powi(4)).clamp(1e-5, 1e7)
        },
        |l| l.0.max(1e-5),
    );

    let log_l = (lum_val as f32).log10();
    let log_t = (temp_val as f32).log10();

    if let Ok(mut text) = plot_text.single_mut() {
        text.0 = render_ascii_hr_grid(log_l, log_t, &body.name);
    }

    if let Ok(mut text) = details_text.single_mut() {
        text.0 = format_star_astrophysics_details(
            body,
            mass.0,
            opt_rad.map_or(1.0, |r| r.0 / SOLAR_RADIUS_AU),
            temp_val,
            lum_val,
            log_l,
            opt_ign,
            opt_evo,
        );
    }
}

fn render_ascii_hr_grid(star_log_l: f32, star_log_t: f32, star_name: &str) -> String {
    // 10 Rows from logL = +5.0 down to -4.0 (step 1.0)
    // 28 Columns for logT from 4.60 (Hot ~40,000 K) down to 3.40 (Cool ~2,500 K)
    const ROWS: usize = 10;
    const COLS: usize = 28;

    let mut grid = [[' '; COLS]; ROWS];

    // Reference tracks:
    // 1. Supergiants line at row 0-1
    if let Some(row_0) = grid.first_mut() {
        for cell in row_0.iter_mut() {
            *cell = '=';
        }
    }

    // 2. Main Sequence diagonal (O-type hot top-left down to M-type cool bottom-right)
    let ms_coords = [
        (1, 4),
        (1, 5),
        (2, 7),
        (2, 8),
        (3, 10),
        (3, 11),
        (4, 13),
        (4, 14),
        (5, 16),
        (5, 17), // G2V Sun ~ row 5, col 17
        (6, 19),
        (6, 20),
        (7, 22),
        (7, 23),
        (8, 25),
        (8, 26),
    ];
    for (r, c) in ms_coords {
        if let Some(cell) = grid.get_mut(r).and_then(|row| row.get_mut(c)) {
            *cell = '~';
        }
    }

    // 3. Red Giants branch (upper right: low T, high L)
    for r in [2, 3] {
        if let Some(row) = grid.get_mut(r) {
            for c in 19..26 {
                if let Some(cell) = row.get_mut(c) {
                    *cell = '^';
                }
            }
        }
    }

    // 4. White Dwarfs track (lower left: high T, low L)
    for r in [8, 9] {
        if let Some(row) = grid.get_mut(r) {
            for c in 5..14 {
                if let Some(cell) = row.get_mut(c) {
                    *cell = '.';
                }
            }
        }
    }

    // 5. Place current star marker
    let star_row = (((5.0 - star_log_l) / 9.0) * ((ROWS - 1) as f32))
        .round()
        .clamp(0.0, (ROWS - 1) as f32) as usize;
    let star_col = (((4.60 - star_log_t) / 1.20) * ((COLS - 1) as f32))
        .round()
        .clamp(0.0, (COLS - 1) as f32) as usize;

    if let Some(cell) = grid.get_mut(star_row).and_then(|row| row.get_mut(star_col)) {
        *cell = '*';
    }

    let row_labels = ["+5", "+4", "+3", "+2", "+1", " 0", "-1", "-2", "-3", "-4"];
    let mut out = String::with_capacity(1024);
    out.push_str("log L/L\u{2609}  Hot (40kK) \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}> Cool (3kK)\n");

    for (r, row) in grid.iter().enumerate() {
        if let Some(lbl) = row_labels.get(r) {
            out.push_str(lbl);
        }
        out.push_str(" \u{2502}");
        for (c, cell) in row.iter().enumerate() {
            if r == star_row && c == star_col {
                out.push('\u{2605}'); // Five-pointed star marker
            } else {
                out.push(*cell);
            }
        }
        if r == star_row {
            let _ = write!(out, " \u{25c0} {star_name}");
        }
        out.push('\n');
    }

    out.push_str("    \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    out.push_str("      O     B     A    F    G    K    M\n");
    out.push_str("Legend: \u{2605} Current Star  ~ Main Sequence  ^ Giants  . White Dwarfs\n");
    out
}

#[allow(
    clippy::too_many_arguments,
    reason = "Detailed stellar astrophysics readout"
)]
fn format_star_astrophysics_details(
    body: &CelestialBody,
    mass_solar: f64,
    rad_solar: f64,
    temp_k: f64,
    lum_solar: f64,
    log_l: f32,
    opt_ign: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
) -> String {
    let spectral_class = classify_spectral_type(temp_k, lum_solar, body.body_type);
    let fusion_mode = determine_fusion_mode(mass_solar, opt_ign, opt_evo, body.body_type);

    let hz_inner = 0.95 * lum_solar.sqrt();
    let hz_outer = 1.67 * lum_solar.sqrt();
    let frost_line = 2.70 * lum_solar.sqrt();

    format!(
        "Host: {}  |  Class: {}\n\
         T_eff: {:.0} K  |  Luminosity: {:.3} L\u{2609} (log L: {:+.2})\n\
         Radius: {:.2} R\u{2609} ({:.4} AU)  |  Mass: {:.2} M\u{2609}\n\
         Core State: {}\n\
         Habitable Zone: {:.2} \u{2013} {:.2} AU  |  Frost Line: {:.2} AU",
        body.name,
        spectral_class,
        temp_k,
        lum_solar,
        log_l,
        rad_solar,
        rad_solar * SOLAR_RADIUS_AU,
        mass_solar,
        fusion_mode,
        hz_inner,
        hz_outer,
        frost_line
    )
}

fn classify_spectral_type(temp_k: f64, lum_solar: f64, body_type: BodyType) -> &'static str {
    match body_type {
        BodyType::WhiteDwarf => "DA Degenerate White Dwarf",
        BodyType::BlackDwarf => "DC Degenerate Black Dwarf",
        BodyType::QuarkStar => "Strange Quark Matter Star",
        BodyType::DarkStar => "WIMP-Powered Dark Star",
        BodyType::BosonStar => "Scalar Soliton Boson Star",
        BodyType::IronStar => "Cold-Fusion Degenerate Iron Star",
        BodyType::ThorneZytkow => "Thorne-Żytkow Hybrid Supergiant",
        BodyType::PopulationIIIStar => "Pop-III Primordial Zero-Z Hypergiant",
        BodyType::WolfRayet => "WN/WC Stripped Wolf-Rayet Star",
        BodyType::CarbonStar => "C-N Carbon-Rich AGB Giant",
        BodyType::LuminousBlueVariable => "LBV Eruptive Blue Hypergiant",
        BodyType::YellowHypergiant => "Ia-0 Yellow Hypergiant (Rho Cas)",
        BodyType::TTauriStar => "PMS T Tauri Chromospheric Star",
        BodyType::HerbigAeBeStar => "PMS Herbig Ae/Be Star",
        BodyType::BlueStraggler => "B-V Rejuvenated Blue Straggler",
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => {
            "Compact Relativistic Remnant"
        }
        BodyType::RedGiant | BodyType::RedSupergiant => {
            if lum_solar > 5000.0 {
                "M2 I Red Supergiant"
            } else {
                "M0 III Red Giant"
            }
        }
        _ => {
            if temp_k >= 30_000.0 {
                "O5 V Blue Supergiant"
            } else if temp_k >= 10_000.0 {
                "B2 V Blue-White Subgiant"
            } else if temp_k >= 7_500.0 {
                "A1 V White Main-Sequence"
            } else if temp_k >= 6_000.0 {
                "F5 V Yellow-White Dwarf"
            } else if temp_k >= 5_200.0 {
                "G2 V Solar Main-Sequence"
            } else if temp_k >= 3_700.0 {
                "K2 V Orange Dwarf"
            } else {
                "M4 V Red Dwarf"
            }
        }
    }
}

fn determine_fusion_mode(
    mass_solar: f64,
    opt_ign: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
    body_type: BodyType,
) -> &'static str {
    match body_type {
        BodyType::WhiteDwarf => return "Electron Degeneracy Pressure (Inert)",
        BodyType::BlackDwarf => return "Crystallized Carbon-Oxygen Lattice (Cold Inert)",
        BodyType::IronStar => return "Cold-Fusion Fe-56 Degenerate Relic",
        BodyType::QuarkStar => return "Deconfined Strange-Quark Matter Bag Pressure",
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => {
            return "Neutron Degeneracy & Strong Nuclear Repulsion";
        }
        BodyType::BosonStar => return "Quantum Scalar Soliton Uncertainty Pressure",
        BodyType::DarkStar => return "WIMP Dark Matter Self-Annihilation Heating",
        BodyType::ThorneZytkow => return "Interrupted Rapid Proton (irp) Capture on NS Core",
        BodyType::WolfRayet => return "Core He/C Burning & Supersonic Line-Driven Winds",
        BodyType::CarbonStar => return "Thermal-Pulse AGB Triple-Alpha & C-12 Dredge-Up",
        BodyType::PopulationIIIStar => return "Pristine Z=0 High-T CNO & Pair-Production",
        _ => {}
    }
    if let Some(evo) = opt_evo {
        match evo.phase {
            StellarEvolutionPhase::RedGiantBranch
            | StellarEvolutionPhase::RedSupergiantBranch
            | StellarEvolutionPhase::HeliumFlashAgb => {
                return "Triple-Alpha Helium Flash & Shell Fusion";
            }
            StellarEvolutionPhase::WhiteDwarf => return "Electron Degeneracy Cooling",
            StellarEvolutionPhase::PlanetaryNebulaEjection
            | StellarEvolutionPhase::SupernovaExplosion => {
                return "Gravitational Shock Remnant / Nebula Ejection";
            }
            _ => {}
        }
    }
    if let Some(ign) = opt_ign {
        if !ign.is_ignited {
            return "Hayashi Gravitational Contraction (Pre-ZAMS)";
        }
    }
    if mass_solar > 1.35 {
        "CNO Catalytic Cycle (Carbon-Nitrogen-Oxygen)"
    } else {
        "Proton–Proton (p–p) Chain Hydrogen Fusion"
    }
}
