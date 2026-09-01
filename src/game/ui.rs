//! Heads-Up Display (HUD), Dynamic Telemetry Overlay, and Interactive On-Screen Control Buttons.

use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;
use std::f64::consts::PI;

use crate::game::phases::PhaseManager;
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Marker for the top-left simulation statistics text.
#[derive(Component)]
pub struct HudHeaderStatsText;

/// Marker for the top-right time warp / speed controls text.
#[derive(Component)]
pub struct HudTimeWarpText;

/// Marker for the bottom-left body inspector telemetry text.
#[derive(Component)]
pub struct HudInspectorText;

/// Marker for the dynamic action button tooltip explanation text.
#[derive(Component)]
pub struct HudActionTooltipText;

/// Marker for the notification toast banner text.
#[derive(Component)]
pub struct HudToastText;

/// Marker for the bottom elapsed time timer text.
#[derive(Component)]
pub struct HudBottomTimerText;

/// On-screen notification toast resource.
#[derive(Resource, Debug, Clone)]
pub struct NotificationToast {
    pub message: String,
    pub timer: f32,
}

impl Default for NotificationToast {
    fn default() -> Self {
        Self {
            message: "⚡ PROTOSTELLAR LIVE // Click any Planet or Sun to inspect & live-edit!"
                .to_string(),
            timer: 10.0,
        }
    }
}

/// Identifiers for interactive clickable HUD buttons.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiButtonAction {
    // Time Controls
    TimePause,
    TimeSpeed1,
    TimeSpeed100,
    TimeSpeed10k,
    TimeSpeed1M,
    // Target Selection
    SelectStar,
    SelectMercury,
    SelectEarth,
    SelectJupiter,
    SelectKuiper,
    CycleTarget,
    // Scientific Instruments & Overlays
    CycleOverlayMode,
    ToggleTractor,
    // Live Celestial Body Editor
    IncreaseMass,
    DecreaseMass,
    ExpandOrbit,
    ContractOrbit,
    CycleComposition,
    BoostDeltaV,
    BrakeDeltaV,
    InjectEmbryo,
    VaporizeBody,
    FocusLock,
    ResetView,
    DeselectBody,
    FixOrbit,
    IgniteStar,
    TriggerLhb,
    ShatterIntoRings,
    SeedLife,
    AgeStar,
    // Sandbox Scenarios & System Presets
    LoadScenarioSolar,
    LoadScenarioTrappist,
    LoadScenarioKepler16,
    LoadScenarioHotJupiter,
    LoadScenarioRoguePlanet,
}

impl UiButtonAction {
    pub fn tooltip_description(&self) -> &'static str {
        match self {
            UiButtonAction::TimePause => "[Space]: Pause or resume continuous orbital physics flow.",
            UiButtonAction::TimeSpeed1 => "[1]: 1.0x Real-time orbital flow speed.",
            UiButtonAction::TimeSpeed100 => "[2]: 100x Accelerated time progression.",
            UiButtonAction::TimeSpeed10k => "[3]: 10,000x High-speed planetary accretion flow (~10 kyr/sec).",
            UiButtonAction::TimeSpeed1M => "[4]: 1,000,000x Deep astronomical time-warp (~1 Myr/sec).",
            UiButtonAction::SelectStar => "Select the central star to inspect solar mass, temperature, and corona.",
            UiButtonAction::SelectMercury => "Select the innermost rocky terrestrial planet.",
            UiButtonAction::SelectEarth => "Select the habitable-zone terrestrial planet.",
            UiButtonAction::SelectJupiter => "Select the dominant outer gas giant planet.",
            UiButtonAction::SelectKuiper => "Select the outermost icy Kuiper belt planetesimal.",
            UiButtonAction::CycleTarget => "[Tab]: Cycle camera focus through all active celestial bodies.",
            UiButtonAction::CycleOverlayMode => "[V]: Cycle diagnostic HUD overlays (Natural Color -> Spectral Temperature -> Hill Spheres & Gaps).",
            UiButtonAction::ToggleTractor => "[T]: Toggles Gravitational Tractor Beam to pull particles & planetesimals.",
            UiButtonAction::IncreaseMass => "[U]: Accrete +25% mass into selected planet.",
            UiButtonAction::DecreaseMass => "[J]: Strip -20% outer envelope mass from selected planet.",
            UiButtonAction::ExpandOrbit => "[O]: Boost orbital energy to expand orbital radius +10%.",
            UiButtonAction::ContractOrbit => "[L]: Brake orbital energy to contract orbital radius -10%.",
            UiButtonAction::CycleComposition => "[C]: Cycle composition between Rocky, Metallic, Icy, and Volatile.",
            UiButtonAction::BoostDeltaV => "[=]: Prograde orbital velocity acceleration boost.",
            UiButtonAction::BrakeDeltaV => "[-]: Retrograde orbital velocity braking burn.",
            UiButtonAction::InjectEmbryo => "[M]: Spawn and inject a new planetary embryo / moon.",
            UiButtonAction::VaporizeBody => "[Del]: Shatter selected celestial body into dust & fragments.",
            UiButtonAction::FocusLock => "[F]: Focus & track camera onto selected celestial body.",
            UiButtonAction::ResetView => "[R]: Reset camera to overview orientation.",
            UiButtonAction::DeselectBody => "[Esc]: Close inspector and clear selection.",
            UiButtonAction::FixOrbit => "[Z]: Circularize orbital eccentricity to 0.00.",
            UiButtonAction::IgniteStar => "[I]: Force instant core ignition / coronal solar blast.",
            UiButtonAction::TriggerLhb => "[G]: Trigger 2:1 resonance migration & Late Heavy Bombardment.",
            UiButtonAction::ShatterIntoRings => "[X]: Tidally disrupt selected moon/body into glowing planetary rings.",
            UiButtonAction::SeedLife => "[E]: Seed primordial water oceans, atmosphere, and photosynthetic biosphere.",
            UiButtonAction::AgeStar => "[N]: Step central star forward through far-future lifecycle (Red Giant -> Nebula -> White Dwarf).",
            UiButtonAction::LoadScenarioSolar => "[F1]: Reset to standard 4.5 Gyr Hayashi Solar Nebula MMSN with central protostar and 10 embryos.",
            UiButtonAction::LoadScenarioTrappist => "[F2]: Load TRAPPIST-1 ultracool red dwarf system with 7 resonant Earths (3 in Habitable Zone).",
            UiButtonAction::LoadScenarioKepler16 => "[F3]: Load Kepler-16 'Tatooine' circumbinary system with K/M binary star pair and giant planet.",
            UiButtonAction::LoadScenarioHotJupiter => "[F4]: Load Hot Jupiter inward migration scenario (Type II disk migration from 5.2 AU -> 0.045 AU).",
            UiButtonAction::LoadScenarioRoguePlanet => "[F5]: Load Rogue Planet Flyby scenario (Hyperbolic 3.5 M_Jup interloper scattering the solar system).",
        }
    }
}

/// Helper function to create standard glassmorphic button style
fn create_button(
    parent: &mut ChildSpawnerCommands,
    action: UiButtonAction,
    label: &str,
    bg_color: Color,
    border_color: Color,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                margin: UiRect::all(Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(border_color),
            BackgroundColor(bg_color),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(12.5),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
            ));
        });
}

/// Sets up the complete HUD overlay interface with interactive on-screen toolbars.
pub fn setup_hud(mut commands: Commands) {
    commands.init_resource::<NotificationToast>();

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .with_children(|root| {
            // ==========================================
            // TOP BAR: Telemetry (Left) | System Quick Selector (Center) | Time Controls (Right)
            // ==========================================
            root.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                ..default()
            })
            .with_children(|top_row| {
                // Top Left Stats Panel
                top_row
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(10.0)),
                            max_width: Val::Px(340.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
                        BorderColor::all(Color::srgba(0.2, 0.4, 0.7, 0.5)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            Text::new("PROTOSTELLAR // Astrophysics Simulator\nInitializing Nebula..."),
                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                            TextColor(Color::srgb(0.9, 0.95, 1.0)),
                            HudHeaderStatsText,
                        ));
                    });

                // Top Center: Quick System Body Selector Bar & Notification Toast
                top_row
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|center_col| {
                        // Interactive Body Selector Buttons
                        center_col
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Row,
                                    padding: UiRect::all(Val::Px(4.0)),
                                    align_items: AlignItems::Center,
                                    margin: UiRect::bottom(Val::Px(6.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.85)),
                                BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.6)),
                            ))
                            .with_children(|btn_row| {
                                create_button(btn_row, UiButtonAction::SelectStar, "Sun", Color::srgba(0.25, 0.18, 0.04, 0.9), Color::srgb(0.9, 0.7, 0.2));
                                create_button(btn_row, UiButtonAction::SelectEarth, "Proto-Earth", Color::srgba(0.05, 0.18, 0.15, 0.85), Color::srgb(0.3, 0.85, 0.6));
                                create_button(btn_row, UiButtonAction::SelectMercury, "Ceres", Color::srgba(0.08, 0.12, 0.2, 0.85), Color::srgb(0.4, 0.7, 0.9));
                                create_button(btn_row, UiButtonAction::SelectJupiter, "Proto-Jupiter", Color::srgba(0.18, 0.12, 0.08, 0.85), Color::srgb(0.9, 0.6, 0.3));
                                create_button(btn_row, UiButtonAction::SelectKuiper, "Kuiper", Color::srgba(0.08, 0.12, 0.22, 0.85), Color::srgb(0.4, 0.6, 0.95));
                                create_button(btn_row, UiButtonAction::CycleTarget, "Cycle [Tab]", Color::srgba(0.12, 0.16, 0.26, 0.85), Color::srgb(0.5, 0.7, 1.0));
                            });

                        // Interactive Sandbox Scenario Presets Bar
                        center_col
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Row,
                                    padding: UiRect::all(Val::Px(3.0)),
                                    align_items: AlignItems::Center,
                                    margin: UiRect::bottom(Val::Px(5.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.01, 0.03, 0.07, 0.90)),
                                BorderColor::all(Color::srgba(0.5, 0.3, 0.85, 0.6)),
                            ))
                            .with_children(|scenario_row| {
                                create_button(scenario_row, UiButtonAction::LoadScenarioSolar, "Solar [F1]", Color::srgba(0.18, 0.14, 0.04, 0.9), Color::srgb(0.9, 0.75, 0.3));
                                create_button(scenario_row, UiButtonAction::LoadScenarioTrappist, "TRAPPIST-1 [F2]", Color::srgba(0.22, 0.06, 0.08, 0.9), Color::srgb(1.0, 0.45, 0.45));
                                create_button(scenario_row, UiButtonAction::LoadScenarioKepler16, "Kepler-16 [F3]", Color::srgba(0.20, 0.12, 0.04, 0.9), Color::srgb(1.0, 0.7, 0.3));
                                create_button(scenario_row, UiButtonAction::LoadScenarioHotJupiter, "Hot Jupiter [F4]", Color::srgba(0.18, 0.08, 0.22, 0.9), Color::srgb(0.85, 0.45, 1.0));
                                create_button(scenario_row, UiButtonAction::LoadScenarioRoguePlanet, "Rogue Planet [F5]", Color::srgba(0.06, 0.16, 0.22, 0.9), Color::srgb(0.4, 0.85, 1.0));
                            });

                        // Notification Toast Box
                        center_col
                            .spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.01, 0.06, 0.12, 0.9)),
                                BorderColor::all(Color::srgba(0.2, 0.7, 1.0, 0.7)),
                            ))
                            .with_children(|toast_box| {
                                toast_box.spawn((
                                    Text::new(">> PROTOSTELLAR LIVE"),
                                    TextFont { font_size: FontSize::Px(13.5), ..default() },
                                    TextColor(Color::srgb(0.4, 0.9, 1.0)),
                                    HudToastText,
                                ));
                            });
                    });

                // Top Right: Diagnostics & Time Scale Indicator
                top_row
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(8.0)),
                            align_items: AlignItems::FlexEnd,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
                        BorderColor::all(Color::srgba(0.2, 0.4, 0.7, 0.5)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            Text::new("SPEED: 1.0x\nSteps: 0"),
                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                            TextColor(Color::srgb(0.4, 0.9, 1.0)),
                            HudTimeWarpText,
                        ));

                        panel
                            .spawn(Node {
                                flex_direction: FlexDirection::Row,
                                margin: UiRect::top(Val::Px(4.0)),
                                ..default()
                            })
                            .with_children(|btn_row| {
                                create_button(btn_row, UiButtonAction::TimePause, "Pause", Color::srgba(0.18, 0.08, 0.08, 0.85), Color::srgb(0.9, 0.4, 0.4));
                                create_button(btn_row, UiButtonAction::TimeSpeed1, "1x", Color::srgba(0.06, 0.14, 0.22, 0.85), Color::srgb(0.3, 0.7, 0.9));
                                create_button(btn_row, UiButtonAction::TimeSpeed100, "100x", Color::srgba(0.06, 0.14, 0.22, 0.85), Color::srgb(0.3, 0.7, 0.9));
                                create_button(btn_row, UiButtonAction::TimeSpeed10k, "10k/s", Color::srgba(0.06, 0.14, 0.22, 0.85), Color::srgb(0.3, 0.7, 0.9));
                                create_button(btn_row, UiButtonAction::TimeSpeed1M, "1M/s", Color::srgba(0.12, 0.08, 0.22, 0.85), Color::srgb(0.7, 0.4, 0.95));
                            });
                    });
            });

            // ==========================================
            // BOTTOM BAR: Inspector & Live Editor (Left) | Navigation Legend (Right)
            // ==========================================
            root.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|bottom_row| {
                // Bottom Left: Floating Context Action Card & Telemetry Inspector
                bottom_row
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(12.0)),
                            min_width: Val::Px(490.0),
                            max_width: Val::Px(560.0),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.04, 0.09, 0.94)),
                        BorderColor::all(Color::srgba(0.25, 0.55, 0.95, 0.75)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            Text::new("No celestial body selected. Click on the Star or Planets to inspect & edit."),
                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                            TextColor(Color::srgb(0.90, 0.94, 1.0)),
                            HudInspectorText,
                        ));

                        // Interactive Live Editing Action Toolbar
                        panel
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                margin: UiRect::top(Val::Px(8.0)),
                                ..default()
                            })
                            .with_children(|actions| {
                                // Row 1: Camera Focus & Mass Alteration
                                actions
                                    .spawn(Node {
                                        flex_direction: FlexDirection::Row,
                                        margin: UiRect::bottom(Val::Px(3.0)),
                                        flex_wrap: FlexWrap::Wrap,
                                        ..default()
                                    })
                                    .with_children(|row1| {
                                        create_button(row1, UiButtonAction::IgniteStar, "Ignite Star / Blast [I]", Color::srgba(0.24, 0.16, 0.04, 0.9), Color::srgb(1.0, 0.85, 0.3));
                                        create_button(row1, UiButtonAction::FocusLock, "Track Cam [F]", Color::srgba(0.08, 0.16, 0.24, 0.9), Color::srgb(0.4, 0.8, 1.0));
                                        create_button(row1, UiButtonAction::IncreaseMass, "Accrete Mass (+25%) [U]", Color::srgba(0.06, 0.18, 0.12, 0.9), Color::srgb(0.3, 0.9, 0.55));
                                        create_button(row1, UiButtonAction::DecreaseMass, "Strip Mass (-20%) [J]", Color::srgba(0.18, 0.08, 0.08, 0.9), Color::srgb(0.95, 0.4, 0.4));
                                        create_button(row1, UiButtonAction::DeselectBody, "Deselect [Esc]", Color::srgba(0.15, 0.15, 0.18, 0.9), Color::srgb(0.7, 0.7, 0.8));
                                    });

                                // Row 2: Orbital Maneuvers (Radius & Delta-V)
                                actions
                                    .spawn(Node {
                                        flex_direction: FlexDirection::Row,
                                        margin: UiRect::bottom(Val::Px(3.0)),
                                        flex_wrap: FlexWrap::Wrap,
                                        ..default()
                                    })
                                    .with_children(|row2| {
                                        create_button(row2, UiButtonAction::FixOrbit, "Fix Orbit [Z]", Color::srgba(0.06, 0.22, 0.22, 0.9), Color::srgb(0.3, 0.95, 0.85));
                                        create_button(row2, UiButtonAction::ExpandOrbit, "+10% Orbit [O]", Color::srgba(0.06, 0.15, 0.24, 0.9), Color::srgb(0.3, 0.75, 0.95));
                                        create_button(row2, UiButtonAction::ContractOrbit, "-10% Orbit [L]", Color::srgba(0.06, 0.15, 0.24, 0.9), Color::srgb(0.3, 0.75, 0.95));
                                        create_button(row2, UiButtonAction::BoostDeltaV, "Boost +dv [B]", Color::srgba(0.18, 0.15, 0.06, 0.9), Color::srgb(0.95, 0.85, 0.3));
                                        create_button(row2, UiButtonAction::BrakeDeltaV, "Brake -dv [K]", Color::srgba(0.18, 0.12, 0.06, 0.9), Color::srgb(0.95, 0.65, 0.25));
                                    });

                                // Row 3: Material, Protoplanet Spawning, Tractor & Destruction
                                actions
                                    .spawn(Node {
                                        flex_direction: FlexDirection::Row,
                                        margin: UiRect::bottom(Val::Px(4.0)),
                                        flex_wrap: FlexWrap::Wrap,
                                        ..default()
                                    })
                                    .with_children(|row3| {
                                        create_button(row3, UiButtonAction::CycleComposition, "Change Material [C]", Color::srgba(0.14, 0.10, 0.24, 0.9), Color::srgb(0.75, 0.55, 1.0));
                                        create_button(row3, UiButtonAction::InjectEmbryo, "Spawn Moon/Embryo [M]", Color::srgba(0.08, 0.18, 0.24, 0.9), Color::srgb(0.4, 0.85, 1.0));
                                        create_button(row3, UiButtonAction::TriggerLhb, "Trigger LHB [G]", Color::srgba(0.24, 0.12, 0.04, 0.9), Color::srgb(1.0, 0.65, 0.2));
                                        create_button(row3, UiButtonAction::ShatterIntoRings, "Rings [X]", Color::srgba(0.18, 0.14, 0.06, 0.9), Color::srgb(1.0, 0.85, 0.35));
                                        create_button(row3, UiButtonAction::SeedLife, "Seed Life [E]", Color::srgba(0.04, 0.20, 0.08, 0.9), Color::srgb(0.35, 1.0, 0.45));
                                        create_button(row3, UiButtonAction::AgeStar, "Age Star [N]", Color::srgba(0.24, 0.08, 0.16, 0.9), Color::srgb(1.0, 0.45, 0.75));
                                        create_button(row3, UiButtonAction::ToggleTractor, "Tractor Beam [T]", Color::srgba(0.22, 0.08, 0.22, 0.9), Color::srgb(0.95, 0.45, 0.95));
                                        create_button(row3, UiButtonAction::VaporizeBody, "Shatter to Dust [Del]", Color::srgba(0.28, 0.05, 0.05, 0.9), Color::srgb(1.0, 0.3, 0.3));
                                    });

                                // Dynamic Tooltip & Explanation Bar
                                actions
                                    .spawn((
                                        Node {
                                            padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                            margin: UiRect::top(Val::Px(3.0)),
                                            border: UiRect::all(Val::Px(1.0)),
                                            width: Val::Percent(100.0),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.01, 0.02, 0.05, 0.95)),
                                        BorderColor::all(Color::srgba(0.3, 0.6, 0.9, 0.4)),
                                    ))
                                    .with_children(|tip_box| {
                                        tip_box.spawn((
                                            Text::new("Click any planet or button to interact. Hover over buttons for descriptions."),
                                            TextFont { font_size: FontSize::Px(10.5), ..default() },
                                            TextColor(Color::srgb(0.75, 0.90, 1.0)),
                                            HudActionTooltipText,
                                        ));
                                    });
                            });
                    });

                // Bottom Center: Digital Elapsed Time & Speed Control Dock
                bottom_row
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                            margin: UiRect::axes(Val::Px(8.0), Val::Px(0.0)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.5)),
                            min_width: Val::Px(350.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.01, 0.04, 0.09, 0.95)),
                        BorderColor::all(Color::srgb(0.3, 0.85, 1.0)),
                    ))
                    .with_children(|timer_panel| {
                        timer_panel.spawn((
                            Text::new("SIMULATION ELAPSED TIME: 0.00 yr\nRATE: 1.0x (Real-time) | FLOWING"),
                            TextFont { font_size: FontSize::Px(13.5), ..default() },
                            TextColor(Color::srgb(0.4, 0.95, 1.0)),
                            HudBottomTimerText,
                        ));

                        timer_panel
                            .spawn(Node {
                                flex_direction: FlexDirection::Row,
                                margin: UiRect::top(Val::Px(6.0)),
                                ..default()
                            })
                            .with_children(|btn_row| {
                                create_button(btn_row, UiButtonAction::TimePause, "Pause", Color::srgba(0.18, 0.08, 0.08, 0.85), Color::srgb(0.9, 0.4, 0.4));
                                create_button(btn_row, UiButtonAction::TimeSpeed1, "1x", Color::srgba(0.06, 0.14, 0.22, 0.85), Color::srgb(0.3, 0.7, 0.9));
                                create_button(btn_row, UiButtonAction::TimeSpeed100, "100x", Color::srgba(0.06, 0.14, 0.22, 0.85), Color::srgb(0.3, 0.7, 0.9));
                                create_button(btn_row, UiButtonAction::TimeSpeed10k, "10k/s", Color::srgba(0.06, 0.14, 0.22, 0.85), Color::srgb(0.3, 0.7, 0.9));
                                create_button(btn_row, UiButtonAction::TimeSpeed1M, "1M/s", Color::srgba(0.12, 0.08, 0.22, 0.85), Color::srgb(0.7, 0.4, 0.95));
                            });
                    });

                // Bottom Right: Camera & Navigation Controls Panel
                bottom_row
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.88)),
                        BorderColor::all(Color::srgba(0.2, 0.4, 0.7, 0.5)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            Text::new("360 NAVIGATION & CONTROLS:\n[Right-Drag] 360 Orbit View  |  [WASD / QE] Free-Fly Pan  |  [Scroll] Smooth Zoom\n[Left-Click / Tab] Select Celestial Body  |  [F] Focus-Lock  |  [Esc / R] Deselect / Overview\n[Click Any Button Above] Instant Mouse Action  |  [Space] Pause / Resume"),
                            TextFont { font_size: FontSize::Px(10.5), ..default() },
                            TextColor(Color::srgb(0.75, 0.82, 0.95)),
                        ));
                    });
            });
        });
}

/// Handles interactive mouse clicks and hover highlights on all on-screen HUD buttons.
pub fn handle_ui_button_interactions(
    mut interaction_query: Query<
        (
            &Interaction,
            &UiButtonAction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut tooltip_query: Query<&mut Text, With<HudActionTooltipText>>,
    mut time_warp: ResMut<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut toast: ResMut<NotificationToast>,
    disk_params: Res<DiskParameters>,
    mut selected_query: Query<
        (
            Entity,
            &mut Mass,
            &mut Radius,
            &mut SimPosition,
            &mut SimVelocity,
            &mut Composition,
            &mut CelestialBody,
            Option<&CentralStar>,
            Option<&mut IgnitionState>,
            Option<&mut StellarEvolutionState>,
            Option<&mut Temperature>,
            Option<&mut Luminosity>,
        ),
        Without<PanOrbitCamera>,
    >,
    mut camera_query: Query<&mut PanOrbitCamera>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    mut scenario_events: MessageWriter<crate::simulation::scenarios::LoadScenarioEvent>,
    sim_time: Res<SimTime>,
    mut commands: Commands,
) {
    let mut rng = rand::rng();
    let star_mass = disk_params.central_star_mass;

    for (interaction, action, mut bg_color, mut border_color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Hovered => {
                *border_color = BorderColor::all(Color::srgb(0.4, 0.9, 1.0));
                if let Ok(mut tip) = tooltip_query.single_mut() {
                    tip.0 = action.tooltip_description().to_string();
                }
            }
            Interaction::None => {
                *border_color = BorderColor::all(Color::srgba(0.3, 0.5, 0.8, 0.6));
            }
            Interaction::Pressed => {
                *bg_color = BackgroundColor(Color::srgba(0.2, 0.5, 0.9, 0.95));
                *border_color = BorderColor::all(Color::srgb(1.0, 1.0, 1.0));

                match action {
                    // Time Controls
                    UiButtonAction::TimePause => {
                        time_warp.is_paused = !time_warp.is_paused;
                        toast.message = if time_warp.is_paused {
                            "⏸ Simulation Paused".to_string()
                        } else {
                            format!(
                                "▶ Simulation Resumed ({})",
                                time_warp.human_readable_speed()
                            )
                        };
                        toast.timer = 3.5;
                    }
                    UiButtonAction::TimeSpeed1 => {
                        time_warp.multiplier = 1.0;
                        time_warp.is_paused = false;
                        toast.message = "▶ Speed: 1.0x (Real-time flow)".to_string();
                        toast.timer = 3.5;
                    }
                    UiButtonAction::TimeSpeed100 => {
                        time_warp.multiplier = 100.0;
                        time_warp.is_paused = false;
                        toast.message = "⏩ Speed: 100x Accelerated".to_string();
                        toast.timer = 3.5;
                    }
                    UiButtonAction::TimeSpeed10k => {
                        time_warp.multiplier = 10_000.0;
                        time_warp.is_paused = false;
                        toast.message = "⚡ Speed: 10,000x (~10 kyr/sec)".to_string();
                        toast.timer = 3.5;
                    }
                    UiButtonAction::TimeSpeed1M => {
                        time_warp.multiplier = 1_000_000.0;
                        time_warp.is_paused = false;
                        toast.message = "🚀 Speed: 1,000,000x (~1 Myr/sec)".to_string();
                        toast.timer = 3.5;
                    }

                    // Body Selectors
                    UiButtonAction::SelectStar => {
                        let star_ent = selected_query
                            .iter()
                            .find(|item| item.7.is_some())
                            .map(|item| item.0);
                        if let Some(ent) = star_ent {
                            player_state.selected_entity = Some(ent);
                            if let Ok(mut cam) = camera_query.single_mut() {
                                cam.target_entity = Some(ent);
                            }
                            toast.message =
                                "☀️ Selected: The Central Protostar (1.00 M☉)".to_string();
                            toast.timer = 4.0;
                        }
                    }
                    UiButtonAction::SelectMercury => {
                        let mut sorted: Vec<_> = selected_query
                            .iter()
                            .filter(|item| item.7.is_none())
                            .collect();
                        sorted.sort_by(|a, b| {
                            a.3 .0
                                .length()
                                .partial_cmp(&b.3 .0.length())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        // Prefer body named Mercury or closest inner body
                        let target = sorted
                            .iter()
                            .find(|item| {
                                item.6.name.to_lowercase().contains("mercury")
                                    || item.6.name.to_lowercase().contains("ceres")
                            })
                            .or_else(|| sorted.first());

                        if let Some(item) = target {
                            let ent = item.0;
                            let body_name = item.6.name.clone();
                            player_state.selected_entity = Some(ent);
                            if let Ok(mut cam) = camera_query.single_mut() {
                                cam.target_entity = Some(ent);
                            }
                            toast.message = format!("🪨 Selected: {}", body_name);
                            toast.timer = 4.0;
                        }
                    }
                    UiButtonAction::SelectEarth => {
                        let mut sorted: Vec<_> = selected_query
                            .iter()
                            .filter(|item| item.7.is_none())
                            .collect();
                        sorted.sort_by(|a, b| {
                            a.3 .0
                                .length()
                                .partial_cmp(&b.3 .0.length())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        // Prefer body named Earth, or closest planet near ~1.0-2.0 AU
                        let target = sorted
                            .iter()
                            .find(|item| item.6.name.to_lowercase().contains("earth"))
                            .or_else(|| {
                                sorted.iter().min_by(|a, b| {
                                    (a.3 .0.length() - 1.5)
                                        .abs()
                                        .partial_cmp(&(b.3 .0.length() - 1.5).abs())
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                })
                            })
                            .or_else(|| sorted.first());

                        if let Some(item) = target {
                            let ent = item.0;
                            let body_name = item.6.name.clone();
                            player_state.selected_entity = Some(ent);
                            if let Ok(mut cam) = camera_query.single_mut() {
                                cam.target_entity = Some(ent);
                            }
                            toast.message = format!("🌍 Selected: {}", body_name);
                            toast.timer = 4.0;
                        }
                    }
                    UiButtonAction::SelectJupiter => {
                        let mut sorted: Vec<_> = selected_query
                            .iter()
                            .filter(|item| item.7.is_none())
                            .collect();
                        sorted.sort_by(|a, b| {
                            a.3 .0
                                .length()
                                .partial_cmp(&b.3 .0.length())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        // Prefer body named Jupiter or massive outer planet near ~5.0-10.0 AU
                        let target = sorted
                            .iter()
                            .find(|item| item.6.name.to_lowercase().contains("jupiter"))
                            .or_else(|| {
                                sorted.iter().min_by(|a, b| {
                                    (a.3 .0.length() - 8.5)
                                        .abs()
                                        .partial_cmp(&(b.3 .0.length() - 8.5).abs())
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                })
                            })
                            .or_else(|| sorted.first());

                        if let Some(item) = target {
                            let ent = item.0;
                            let body_name = item.6.name.clone();
                            player_state.selected_entity = Some(ent);
                            if let Ok(mut cam) = camera_query.single_mut() {
                                cam.target_entity = Some(ent);
                            }
                            toast.message = format!("🪐 Selected: {}", body_name);
                            toast.timer = 4.0;
                        }
                    }
                    UiButtonAction::SelectKuiper => {
                        let mut sorted: Vec<_> = selected_query
                            .iter()
                            .filter(|item| item.7.is_none())
                            .collect();
                        sorted.sort_by(|a, b| {
                            a.3 .0
                                .length()
                                .partial_cmp(&b.3 .0.length())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        // Outermost planet or Kuiper embryo
                        if let Some(last) = sorted.last() {
                            let ent = last.0;
                            let body_name = last.6.name.clone();
                            player_state.selected_entity = Some(ent);
                            if let Ok(mut cam) = camera_query.single_mut() {
                                cam.target_entity = Some(ent);
                            }
                            toast.message = format!("☄️ Selected: {}", body_name);
                            toast.timer = 4.0;
                        }
                    }
                    UiButtonAction::CycleTarget => {
                        let mut star_entity: Option<Entity> = None;
                        let mut planets: Vec<(Entity, f64, String, f64)> = Vec::new();

                        for (e, m, _, pos, _, _, body, ..) in selected_query.iter() {
                            if body.body_type.is_star_or_remnant() {
                                star_entity = Some(e);
                            } else {
                                planets.push((e, pos.0.length(), body.name.clone(), m.0));
                            }
                        }

                        // Sort planets strictly from innermost to outermost
                        planets.sort_by(|a, b| {
                            a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                        });

                        let mut all_entities: Vec<Entity> = Vec::new();
                        if let Some(star) = star_entity {
                            all_entities.push(star);
                        }
                        for (p_ent, ..) in &planets {
                            all_entities.push(*p_ent);
                        }

                        if !all_entities.is_empty() {
                            let len = all_entities.len();
                            let next_idx = if let Some(curr) = player_state.selected_entity {
                                if let Some(curr_idx) = all_entities.iter().position(|&e| e == curr)
                                {
                                    (curr_idx + 1) % len
                                } else {
                                    0
                                }
                            } else {
                                0
                            };

                            let next = all_entities[next_idx];
                            player_state.selected_entity = Some(next);
                            if let Ok(mut cam) = camera_query.single_mut() {
                                cam.target_entity = Some(next);
                            }

                            if next_idx == 0 && star_entity.is_some() {
                                toast.message = format!(
                                    ">> TARGET [0/{}]: THE SUN (Central Star) | 0.00 AU",
                                    len - 1
                                );
                            } else {
                                let p_idx = if star_entity.is_some() {
                                    next_idx - 1
                                } else {
                                    next_idx
                                };
                                if p_idx < planets.len() {
                                    let (_, dist, ref name, mass) = planets[p_idx];
                                    let m_str = if mass >= 0.01 {
                                        format!("{:.2} M_sun", mass)
                                    } else {
                                        format!("{:.2} M_earth", mass / EARTH_MASS_SOLAR)
                                    };
                                    toast.message = format!(
                                        ">> TARGET [{}/{}]: {} ({:.2} AU) | Mass: {}",
                                        next_idx,
                                        len - 1,
                                        name,
                                        dist,
                                        m_str
                                    );
                                }
                            }
                            toast.timer = 4.0;
                        }
                    }

                    // Scientific Instruments & Overlays
                    UiButtonAction::CycleOverlayMode => {
                        player_state.overlay_mode = player_state.overlay_mode.cycle();
                        toast.message = format!(
                            "📊 Diagnostic Overlay: {}",
                            player_state.overlay_mode.display_name()
                        );
                        toast.timer = 3.5;
                    }
                    UiButtonAction::ToggleTractor => {
                        if player_state.active_tool == PlayerTool::GravitationalTractor {
                            player_state.active_tool = PlayerTool::Inspect;
                            player_state.tractor_position = None;
                            player_state.tractor_mass = 0.0;
                            toast.message = "🔬 Switched to Inspector Tool".to_string();
                        } else {
                            player_state.active_tool = PlayerTool::GravitationalTractor;
                            player_state.tractor_position = Some(DVec3::new(10.0, 0.0, 10.0));
                            player_state.tractor_mass = EARTH_MASS_SOLAR * 5.0;
                            toast.message =
                                "🧲 Gravitational Tractor Active [Drag to redirect bodies]"
                                    .to_string();
                        }
                        toast.timer = 3.5;
                    }

                    // Live Editor Actions
                    UiButtonAction::IncreaseMass => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((
                                _,
                                mut mass,
                                mut radius,
                                _,
                                _,
                                comp,
                                mut body,
                                is_star,
                                _,
                                _,
                                _,
                                _,
                            )) = selected_query.get_mut(ent)
                            {
                                mass.0 *= 1.25;
                                if !is_star.is_some() && !body.body_type.is_star_or_remnant() {
                                    let avg_density = comp.average_density();
                                    radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                                        .cbrt()
                                        .max(EARTH_RADIUS_AU * 0.1);
                                }

                                // Dynamically reclassify celestial body and promote comets/asteroids to planets
                                let old_type = body.body_type;
                                let new_type = classify_body_by_mass_and_comp(
                                    mass.0,
                                    &comp,
                                    is_star.is_some() || body.body_type.is_star_or_remnant(),
                                );
                                if is_star.is_some() || body.body_type.is_star_or_remnant() {
                                    if !body.body_type.is_remnant() {
                                        body.body_type = new_type;
                                        body.name = match new_type {
                                            BodyType::BrownDwarf => {
                                                "The Star (Brown Dwarf)".to_string()
                                            }
                                            BodyType::RedDwarf => {
                                                "The Star (Red Dwarf - M Type)".to_string()
                                            }
                                            BodyType::YellowDwarf => {
                                                "The Star (Yellow Dwarf - G2V)".to_string()
                                            }
                                            BodyType::BlueGiant => {
                                                "The Star (Blue Giant - B Type)".to_string()
                                            }
                                            BodyType::BlueSupergiant => {
                                                "The Star (Blue Supergiant - O Type)".to_string()
                                            }
                                            BodyType::Hypergiant => {
                                                "The Star (Luminous Hypergiant)".to_string()
                                            }
                                            _ => body.name.clone(),
                                        };
                                    }
                                } else if new_type != old_type {
                                    body.body_type = new_type;
                                    if new_type.is_planet() && !body.name.starts_with("Planet") {
                                        body.name = format!("Planet ({:?})", new_type);
                                    }
                                }

                                toast.message = format!(
                                    "➕ {} Mass Increased: {:.3} M⊕ (+25% | {:?})",
                                    body.name,
                                    mass.0 / EARTH_MASS_SOLAR,
                                    body.body_type
                                );
                                toast.timer = 4.0;
                            }
                        } else {
                            toast.message = "⚠️ Please select a celestial body first!".to_string();
                            toast.timer = 3.0;
                        }
                    }
                    UiButtonAction::DecreaseMass => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((
                                _,
                                mut mass,
                                mut radius,
                                _,
                                _,
                                comp,
                                mut body,
                                is_star,
                                _,
                                _,
                                _,
                                _,
                            )) = selected_query.get_mut(ent)
                            {
                                mass.0 *= 0.80;
                                if !is_star.is_some() && !body.body_type.is_star_or_remnant() {
                                    let avg_density = comp.average_density();
                                    radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                                        .cbrt()
                                        .max(EARTH_RADIUS_AU * 0.1);
                                }

                                let old_type = body.body_type;
                                let new_type = classify_body_by_mass_and_comp(
                                    mass.0,
                                    &comp,
                                    is_star.is_some() || body.body_type.is_star_or_remnant(),
                                );
                                if is_star.is_some() || body.body_type.is_star_or_remnant() {
                                    if !body.body_type.is_remnant() {
                                        body.body_type = new_type;
                                        body.name = match new_type {
                                            BodyType::BrownDwarf => {
                                                "The Star (Brown Dwarf)".to_string()
                                            }
                                            BodyType::RedDwarf => {
                                                "The Star (Red Dwarf - M Type)".to_string()
                                            }
                                            BodyType::YellowDwarf => {
                                                "The Star (Yellow Dwarf - G2V)".to_string()
                                            }
                                            BodyType::BlueGiant => {
                                                "The Star (Blue Giant - B Type)".to_string()
                                            }
                                            BodyType::BlueSupergiant => {
                                                "The Star (Blue Supergiant - O Type)".to_string()
                                            }
                                            BodyType::Hypergiant => {
                                                "The Star (Luminous Hypergiant)".to_string()
                                            }
                                            _ => body.name.clone(),
                                        };
                                    }
                                } else if new_type != old_type {
                                    body.body_type = new_type;
                                }

                                toast.message = format!(
                                    "➖ {} Mass Decreased: {:.3} M⊕ (-20% | {:?})",
                                    body.name,
                                    mass.0 / EARTH_MASS_SOLAR,
                                    body.body_type
                                );
                                toast.timer = 4.0;
                            }
                        } else {
                            toast.message = "⚠️ Please select a celestial body first!".to_string();
                            toast.timer = 3.0;
                        }
                    }
                    UiButtonAction::ExpandOrbit => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((_, _, _, mut pos, mut vel, _, body, ..)) =
                                selected_query.get_mut(ent)
                            {
                                pos.0 *= 1.10;
                                let r = pos.0.length();
                                let v_mag = (G_ASTRO * star_mass / r).sqrt();
                                let mut orbit_tangent = DVec3::new(-pos.0.z, 0.0, pos.0.x);
                                if orbit_tangent.length_squared() > 1e-6 {
                                    orbit_tangent = orbit_tangent.normalize();
                                }
                                vel.0 = orbit_tangent * v_mag;
                                toast.message = format!(
                                    "🚀 {} Orbit Expanded to {:.2} AU (+10%)",
                                    body.name, r
                                );
                                toast.timer = 4.0;
                            }
                        }
                    }
                    UiButtonAction::ContractOrbit => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((_, _, _, mut pos, mut vel, _, body, ..)) =
                                selected_query.get_mut(ent)
                            {
                                pos.0 *= 0.90;
                                let r = pos.0.length();
                                let v_mag = (G_ASTRO * star_mass / r).sqrt();
                                let mut orbit_tangent = DVec3::new(-pos.0.z, 0.0, pos.0.x);
                                if orbit_tangent.length_squared() > 1e-6 {
                                    orbit_tangent = orbit_tangent.normalize();
                                }
                                vel.0 = orbit_tangent * v_mag;
                                toast.message = format!(
                                    "🧲 {} Orbit Contracted to {:.2} AU (-10%)",
                                    body.name, r
                                );
                                toast.timer = 4.0;
                            }
                        }
                    }
                    UiButtonAction::CycleComposition => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((_, mut mass, mut radius, _, _, mut comp, body, ..)) =
                                selected_query.get_mut(ent)
                            {
                                if comp.silicate_frac > 0.5 {
                                    *comp = Composition::icy();
                                    toast.message = format!(
                                        "🎨 {} Material -> Icy Mixture (Density: 1.2 g/cm³)",
                                        body.name
                                    );
                                } else if comp.ice_frac > 0.5 {
                                    *comp = Composition::solar_gas();
                                    mass.0 *= 4.0;
                                    toast.message = format!(
                                        "🎨 {} Material -> Gas Giant Envelope (Density: 0.8 g/cm³)",
                                        body.name
                                    );
                                } else if comp.gas_frac > 0.5 {
                                    *comp = Composition::metal_rich();
                                    toast.message = format!(
                                        "🎨 {} Material -> Metal-Rich Core (Density: 7.8 g/cm³)",
                                        body.name
                                    );
                                } else {
                                    *comp = Composition::rocky();
                                    toast.message = format!(
                                        "🎨 {} Material -> Rocky Silicate (Density: 3.9 g/cm³)",
                                        body.name
                                    );
                                }
                                let avg_density = comp.average_density();
                                radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                                    .cbrt()
                                    .max(EARTH_RADIUS_AU * 0.1);
                                toast.timer = 4.0;
                            }
                        }
                    }
                    UiButtonAction::BoostDeltaV => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((_, _, _, _, mut vel, _, body, ..)) =
                                selected_query.get_mut(ent)
                            {
                                vel.0 *= 1.15;
                                toast.message = format!(
                                    "⚡ {} Prograde Delta-V Boost (+15% Velocity)",
                                    body.name
                                );
                                toast.timer = 4.0;
                            }
                        }
                    }
                    UiButtonAction::BrakeDeltaV => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((_, _, _, _, mut vel, _, body, ..)) =
                                selected_query.get_mut(ent)
                            {
                                vel.0 *= 0.85;
                                toast.message = format!(
                                    "⚡ {} Retrograde Delta-V Brake (-15% Velocity)",
                                    body.name
                                );
                                toast.timer = 4.0;
                            }
                        }
                    }
                    UiButtonAction::FixOrbit => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((_, _, _, mut pos, mut vel, _, body, ..)) =
                                selected_query.get_mut(ent)
                            {
                                if !body.body_type.is_star_or_remnant() {
                                    let r_cyl =
                                        (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt().max(0.1);
                                    let v_circ = (G_ASTRO * star_mass / r_cyl).sqrt();
                                    let phi = pos.0.z.atan2(pos.0.x);
                                    vel.0 =
                                        DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
                                    pos.0.y = 0.0;
                                    toast.message = format!(
                                        "🪐 {} Orbit Circularized & Stabilized (e = 0.0)",
                                        body.name
                                    );
                                    toast.timer = 4.0;
                                }
                            }
                        }
                    }
                    UiButtonAction::InjectEmbryo => {
                        let radius_au = rng.random_range(1.5..7.5);
                        let angle = rng.random_range(0.0..(2.0 * PI));
                        let spawn_pos =
                            DVec3::new(radius_au * angle.cos(), 0.0, radius_au * angle.sin());
                        let v_circ = (G_ASTRO * star_mass / radius_au).sqrt();
                        let spawn_vel =
                            DVec3::new(-v_circ * angle.sin(), 0.0, v_circ * angle.cos());
                        let embryo_mass = EARTH_MASS_SOLAR * rng.random_range(0.05..0.25);
                        let comp = Composition::rocky();
                        let avg_density = comp.average_density();
                        let embryo_rad = ((3.0 * embryo_mass / avg_density) / (4.0 * PI))
                            .cbrt()
                            .max(EARTH_RADIUS_AU * 0.3);

                        let new_entity = commands
                            .spawn((
                                SimPosition(spawn_pos),
                                SimVelocity(spawn_vel),
                                SimAcceleration(DVec3::ZERO),
                                Mass(embryo_mass),
                                Radius(embryo_rad),
                                Temperature(250.0),
                                comp,
                                CelestialBody {
                                    name: format!("Embryo-{}", rng.random_range(100..999)),
                                    body_type: BodyType::Protoplanet,
                                },
                                InternalDifferentiation::default(),
                                SpinState::default(),
                            ))
                            .id();

                        player_state.selected_entity = Some(new_entity);
                        if let Ok(mut cam) = camera_query.single_mut() {
                            cam.target_entity = Some(new_entity);
                        }
                        toast.message =
                            format!("☄️ Spawned New Planetesimal Embryo at {:.2} AU!", radius_au);
                        toast.timer = 5.0;
                    }
                    UiButtonAction::VaporizeBody => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((_, _, _, _, _, _, body, ..)) = selected_query.get(ent) {
                                toast.message = format!("💥 Vaporized {}", body.name);
                                toast.timer = 4.0;
                            }
                            if let Ok(mut cmd) = commands.get_entity(ent) {
                                cmd.despawn();
                            }
                            player_state.selected_entity = None;
                            if let Ok(mut cam) = camera_query.single_mut() {
                                cam.target_entity = None;
                            }
                        }
                    }
                    UiButtonAction::FocusLock => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok(mut cam) = camera_query.single_mut() {
                                cam.target_entity = Some(ent);
                            }
                            toast.message = "🎯 Camera Focus Locked to Selected Target".to_string();
                            toast.timer = 3.5;
                        }
                    }
                    UiButtonAction::ResetView => {
                        if let Ok(mut cam) = camera_query.single_mut() {
                            cam.target_focus = Vec3::ZERO;
                            cam.target_entity = None;
                            cam.target_radius = 45.0;
                        }
                        player_state.selected_entity = None;
                        toast.message = "☀️ Camera Reset to Overview".to_string();
                        toast.timer = 3.5;
                    }
                    UiButtonAction::DeselectBody => {
                        player_state.selected_entity = None;
                        if let Ok(mut cam) = camera_query.single_mut() {
                            cam.target_entity = None;
                        }
                        toast.message = "❌ Closed Inspector & Deselected Target".to_string();
                        toast.timer = 2.5;
                    }
                    UiButtonAction::IgniteStar => {
                        let mut star_opt = selected_query
                            .iter_mut()
                            .find(|(.., is_star, _, _, _, _)| is_star.is_some());
                        if let Some((
                            _,
                            _,
                            _,
                            _,
                            _,
                            _,
                            mut body,
                            _,
                            Some(ref mut ignition),
                            ref mut opt_evo,
                            _,
                            _,
                        )) = star_opt
                        {
                            if !ignition.is_ignited {
                                ignition.core_temperature = 1.0e7;
                                ignition.is_ignited = true;
                                ignition.fusion_fraction = 1.0;
                                ignition.shockwave_radius = 1.6;
                                body.body_type = BodyType::MainSequenceStar;
                                body.name = "The Star (Main Sequence)".to_string();
                                if let Some(ref mut evo) = opt_evo {
                                    evo.phase = StellarEvolutionPhase::MainSequence;
                                }
                                toast.message =
                                    "⭐ Hydrogen Core Fusion Ignited! Solar Wind Shockwave Sweeping the System!".to_string();
                            } else {
                                ignition.shockwave_radius = 1.6;
                                toast.message =
                                    "☀️ Coronal Mass Ejection & Solar Blast Triggered!".to_string();
                            }
                            toast.timer = 5.0;
                        }
                    }
                    UiButtonAction::TriggerLhb => {
                        lhb_state.is_active = true;
                        lhb_state.manual_trigger_requested = true;
                        toast.message =
                            "☄️ LATE HEAVY BOMBARDMENT TRIGGERED // 2:1 Giant resonance active!"
                                .to_string();
                        toast.timer = 8.0;
                    }
                    UiButtonAction::ShatterIntoRings => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((.., body, is_star, _, _, _, _)) = selected_query.get(ent) {
                                if is_star.is_some() {
                                    toast.message =
                                        "⚠️ Cannot form planetary rings around the central star!"
                                            .to_string();
                                    toast.timer = 3.5;
                                } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
                                    p_cmd.insert(PlanetaryRingSystem {
                                        inner_radius_au: 0.0008,
                                        outer_radius_au: 0.0028,
                                        ring_mass_earth: 0.0002,
                                        optical_depth: 0.88,
                                        ice_fraction: 0.95,
                                        silicate_fraction: 0.05,
                                    });
                                    toast.message = format!(
                                        "🪐 Formed Luminous Ring System around {}!",
                                        body.name
                                    );
                                    toast.timer = 5.0;
                                }
                            }
                        }
                    }
                    UiButtonAction::SeedLife => {
                        if let Some(ent) = player_state.selected_entity {
                            if let Ok((.., mut comp, body, is_star, _, _, _, _)) =
                                selected_query.get_mut(ent)
                            {
                                if is_star.is_some() {
                                    toast.message =
                                        "⚠️ Cannot seed life onto a stellar plasma furnace!"
                                            .to_string();
                                    toast.timer = 3.5;
                                } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
                                    p_cmd.insert((
                                        VolatileInventory {
                                            delivered_water_m_earth: 0.002,
                                            ocean_coverage_frac: 0.70,
                                            atmospheric_pressure_bar: 1.0,
                                            cometary_impact_count: 12,
                                        },
                                        BiosphereState {
                                            habitability_score: 0.95,
                                            biomass_coverage_frac: 0.65,
                                            oxygen_fraction: 0.21,
                                            emergence_year: Some(sim_time.elapsed_years),
                                        },
                                        PlanetaryClimate {
                                            surface_temperature_k: 288.0,
                                            equilibrium_temperature_k: 255.0,
                                            greenhouse_delta_k: 33.0,
                                            albedo: 0.30,
                                            ice_coverage_frac: 0.10,
                                            cloud_coverage_frac: 0.55,
                                            climate_regime: ClimateRegime::TemperateHabitable,
                                        },
                                    ));
                                    comp.ice_frac = 0.08;
                                    comp.gas_frac = 0.02;
                                    toast.message = format!(
                                        "🌱 Seeded Photosynthetic Biosphere & Oceans on {}!",
                                        body.name
                                    );
                                    toast.timer = 5.0;
                                }
                            }
                        }
                    }
                    UiButtonAction::AgeStar => {
                        let mut star_opt = selected_query
                            .iter_mut()
                            .find(|(.., is_star, _, _, _, _)| is_star.is_some());
                        if let Some((
                            _ent,
                            mut mass,
                            mut radius,
                            _,
                            _,
                            _,
                            mut body,
                            _,
                            Some(ref mut ignition),
                            ref mut opt_evo,
                            Some(ref mut temp),
                            Some(ref mut lum),
                        )) = star_opt
                        {
                            if !ignition.is_ignited {
                                ignition.core_temperature = 1.0e7;
                                ignition.is_ignited = true;
                                ignition.fusion_fraction = 1.0;
                                ignition.shockwave_radius = 1.6;
                                let (assigned_type, name_str) = if mass.0 < 0.08 {
                                    (BodyType::BrownDwarf, "The Star (Brown Dwarf)")
                                } else if mass.0 < 0.50 {
                                    (BodyType::RedDwarf, "The Star (Red Dwarf - M Type)")
                                } else if mass.0 < 1.4 {
                                    (BodyType::YellowDwarf, "The Star (Yellow Dwarf - G2V)")
                                } else if mass.0 < 8.0 {
                                    (BodyType::BlueGiant, "The Star (Blue Giant - B Type)")
                                } else if mass.0 < 25.0 {
                                    (
                                        BodyType::BlueSupergiant,
                                        "The Star (Blue Supergiant - O Type)",
                                    )
                                } else {
                                    (BodyType::Hypergiant, "The Star (Luminous Hypergiant)")
                                };
                                body.body_type = assigned_type;
                                body.name = name_str.to_string();
                                if let Some(ref mut evo) = opt_evo {
                                    evo.phase = StellarEvolutionPhase::MainSequence;
                                }
                                toast.message =
                                    format!("⭐ Ignited: {} (Main Sequence)", body.name);
                            } else if let Some(ref mut evo) = opt_evo {
                                let m = mass.0;
                                if m >= 25.0 {
                                    // Hypermassive branch: Hypergiant -> Supernova -> Black Hole
                                    match evo.phase {
                                        StellarEvolutionPhase::ProtostarContraction
                                        | StellarEvolutionPhase::MainSequence => {
                                            evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
                                            body.body_type = BodyType::Hypergiant;
                                            body.name =
                                                "The Star (Luminous Yellow Hypergiant)".to_string();
                                            radius.0 = 4.5;
                                            temp.0 = 4000.0;
                                            lum.0 = 250_000.0;
                                            toast.message =
                                                "💥 Hypergiant Phase: Luminosity surged to 250,000 L☉!"
                                                    .to_string();
                                        }
                                        StellarEvolutionPhase::RedSupergiantBranch => {
                                            evo.phase = StellarEvolutionPhase::SupernovaExplosion;
                                            body.name =
                                                "The Star (Hypernova Detonation)".to_string();
                                            evo.nebula_expansion_radius_au = 5.0;
                                            evo.nebula_opacity = 1.0;
                                            toast.message =
                                                "💥 HYPERNOVA DETONATION! Core collapsing into a Singularity!"
                                                    .to_string();
                                        }
                                        _ => {
                                            evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
                                            body.body_type = BodyType::BlackHole;
                                            body.name =
                                                "The Star (Stellar-Mass Black Hole)".to_string();
                                            mass.0 = (m * 0.25).clamp(3.0, 15.0);
                                            radius.0 = 2.95e-5 * mass.0;
                                            temp.0 = 10.0;
                                            lum.0 = 5000.0;
                                            toast.message =
                                                "🕳️ Gravitational Singularity Formed (Event Horizon & Accretion Disk)!"
                                                    .to_string();
                                        }
                                    }
                                } else if m >= 8.0 {
                                    // Massive branch: Blue Giant -> Red Supergiant -> Type II Supernova -> Pulsar / Magnetar
                                    match evo.phase {
                                        StellarEvolutionPhase::ProtostarContraction
                                        | StellarEvolutionPhase::MainSequence => {
                                            evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
                                            body.body_type = BodyType::RedSupergiant;
                                            body.name = "The Star (Red Supergiant)".to_string();
                                            radius.0 = 3.5;
                                            temp.0 = 3300.0;
                                            lum.0 = 80_000.0;
                                            toast.message =
                                                "🔴 Red Supergiant Expansion (R ~ 3.5 AU, L ~ 80,000 L☉)!"
                                                    .to_string();
                                        }
                                        StellarEvolutionPhase::RedSupergiantBranch => {
                                            evo.phase = StellarEvolutionPhase::SupernovaExplosion;
                                            body.name = "The Star (Type II Supernova Explosion)"
                                                .to_string();
                                            evo.nebula_expansion_radius_au = 4.0;
                                            evo.nebula_opacity = 1.0;
                                            toast.message =
                                                "💥 TYPE II CORE-COLLAPSE SUPERNOVA! Blast expanding at 15,000 km/s!"
                                                    .to_string();
                                        }
                                        _ => {
                                            evo.phase = StellarEvolutionPhase::NeutronStarPulsar;
                                            body.body_type = BodyType::Pulsar;
                                            body.name = "The Star (Pulsar Remnant)".to_string();
                                            mass.0 = 1.44;
                                            radius.0 = 0.0001;
                                            temp.0 = 1_000_000.0;
                                            lum.0 = 100.0;
                                            toast.message =
                                                "⚡ Relativistic Pulsar Remnant (B ~ 10^12 G, P ~ 33 ms Synchrotron Jets)!"
                                                    .to_string();
                                        }
                                    }
                                } else if m >= 0.50 {
                                    // Solar/Intermediate branch: Yellow Dwarf -> Red Giant -> AGB -> Planetary Nebula -> White Dwarf
                                    match evo.phase {
                                        StellarEvolutionPhase::ProtostarContraction
                                        | StellarEvolutionPhase::MainSequence => {
                                            evo.phase = StellarEvolutionPhase::RedGiantBranch;
                                            evo.hydrogen_core_fraction = 0.0;
                                            body.body_type = BodyType::RedGiant;
                                            body.name = "The Star (Red Giant Branch)".to_string();
                                            radius.0 = 1.25;
                                            temp.0 = 3100.0;
                                            lum.0 = 2500.0;
                                            toast.message =
                                                "🔴 Star Expanded to Red Giant (R ~ 1.25 AU, L ~ 2500 L☉)! Inner planets engulfing!"
                                                    .to_string();
                                        }
                                        StellarEvolutionPhase::RedGiantBranch => {
                                            evo.phase = StellarEvolutionPhase::HeliumFlashAgb;
                                            body.name = "The Star (AGB Supergiant)".to_string();
                                            radius.0 = 1.50;
                                            temp.0 = 2900.0;
                                            lum.0 = 3500.0;
                                            toast.message =
                                                "🔥 Helium Flash & Asymptotic Giant Pulses (R ~ 1.50 AU, L ~ 3500 L☉)!"
                                                    .to_string();
                                        }
                                        StellarEvolutionPhase::HeliumFlashAgb => {
                                            evo.phase =
                                                StellarEvolutionPhase::PlanetaryNebulaEjection;
                                            body.name =
                                                "The Star (Planetary Nebula Ejection)".to_string();
                                            evo.nebula_expansion_radius_au = 2.0;
                                            evo.nebula_opacity = 1.0;
                                            mass.0 = 0.55;
                                            toast.message =
                                                "💨 Planetary Nebula Ejected! Star shed 45% mass, outer orbits expanding!"
                                                    .to_string();
                                        }
                                        _ => {
                                            evo.phase = StellarEvolutionPhase::WhiteDwarf;
                                            body.body_type = BodyType::WhiteDwarf;
                                            body.name =
                                                "The Star (White Dwarf Remnant)".to_string();
                                            radius.0 = 0.009;
                                            temp.0 = 30_000.0;
                                            lum.0 = (radius.0 / SOLAR_RADIUS_AU).powi(2)
                                                * (temp.0 / 5778.0).powi(4);
                                            toast.message =
                                                "⚪ Degenerate White Dwarf Remnant (Earth-sized, T ~ 30,000 K, B ~ 10^6 G)!"
                                                    .to_string();
                                        }
                                    }
                                } else {
                                    // Red dwarf branch
                                    evo.phase = StellarEvolutionPhase::WhiteDwarf;
                                    body.body_type = BodyType::WhiteDwarf;
                                    body.name = "The Star (Helium White Dwarf)".to_string();
                                    radius.0 = 0.009;
                                    temp.0 = 25_000.0;
                                    toast.message =
                                        "⚪ Low-Mass Helium White Dwarf Remnant Formed!"
                                            .to_string();
                                }
                            }
                            toast.timer = 6.0;
                        }
                    }

                    // Sandbox Scenarios & System Presets
                    UiButtonAction::LoadScenarioSolar => {
                        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
                            crate::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn,
                        ));
                        toast.message =
                            "🪐 Loaded Scenario: Hayashi Solar Nebula (MMSN)".to_string();
                        toast.timer = 5.0;
                    }
                    UiButtonAction::LoadScenarioTrappist => {
                        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
                            crate::simulation::scenarios::ScenarioPreset::Trappist1System,
                        ));
                        toast.message =
                            "🔴 Loaded Scenario: TRAPPIST-1 (7 Resonant Earths, 3 Habitable)"
                                .to_string();
                        toast.timer = 5.0;
                    }
                    UiButtonAction::LoadScenarioKepler16 => {
                        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
                            crate::simulation::scenarios::ScenarioPreset::Kepler16Circumbinary,
                        ));
                        toast.message =
                            "☀️ Loaded Scenario: Kepler-16 'Tatooine' Circumbinary System"
                                .to_string();
                        toast.timer = 5.0;
                    }
                    UiButtonAction::LoadScenarioHotJupiter => {
                        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
                            crate::simulation::scenarios::ScenarioPreset::HotJupiterMigration,
                        ));
                        toast.message =
                            "🌀 Loaded Scenario: Hot Jupiter Type II Inward Migration".to_string();
                        toast.timer = 5.0;
                    }
                    UiButtonAction::LoadScenarioRoguePlanet => {
                        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
                            crate::simulation::scenarios::ScenarioPreset::RoguePlanetFlyby,
                        ));
                        toast.message =
                            "☄️ Loaded Scenario: Interstellar Rogue Planet Flyby Perturbation"
                                .to_string();
                        toast.timer = 5.0;
                    }
                }
            }
        }
    }
}

/// Updates the dynamic content of the HUD and notification toast banner every frame.
pub fn update_hud(
    time: Res<Time>,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    config: Res<SimulationConfig>,
    energy_monitor: Res<EnergyMonitor>,
    phase_mgr: Res<PhaseManager>,
    lhb_state: Res<crate::game::phases::LateHeavyBombardmentState>,
    player_state: Res<PlayerInteractionState>,
    mut toast: ResMut<NotificationToast>,
    bodies_query: Query<(
        &SimPosition,
        &SimVelocity,
        &Mass,
        &Radius,
        &Temperature,
        &Composition,
        &CelestialBody,
        Option<&InternalDifferentiation>,
        Option<&SpinState>,
        Option<&IgnitionState>,
        Option<&VolatileInventory>,
        Option<&PlanetaryRingSystem>,
        Option<&PlanetaryClimate>,
        Option<&BiosphereState>,
        Option<&StellarEvolutionState>,
    )>,
    mut header_query: Query<
        &mut Text,
        (
            With<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
        ),
    >,
    mut time_query: Query<
        &mut Text,
        (
            With<HudTimeWarpText>,
            Without<HudHeaderStatsText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
        ),
    >,
    mut inspector_query: Query<
        &mut Text,
        (
            With<HudInspectorText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
        ),
    >,
    mut toast_query: Query<
        &mut Text,
        (
            With<HudToastText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudBottomTimerText>,
        ),
    >,
    mut bottom_timer_query: Query<
        &mut Text,
        (
            With<HudBottomTimerText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
        ),
    >,
) {
    // 0. Update Toast Notification Timer & Active Selection Header
    if toast.timer > 0.0 {
        toast.timer -= time.delta_secs();
    }
    if let Ok(mut toast_text) = toast_query.single_mut() {
        if phase_mgr.milestone_toast_timer > 0.0 {
            if let Some(ref m_title) = phase_mgr.latest_unlocked_milestone {
                toast_text.0 = format!("🎉 MILESTONE UNLOCKED: {} 🎉", m_title);
            } else if toast.timer > 0.0 {
                toast_text.0 = toast.message.clone();
            }
        } else if toast.timer > 0.0 {
            toast_text.0 = toast.message.clone();
        } else if let Some(selected_entity) = player_state.selected_entity {
            if let Ok((pos, vel, mass, rad, temp, _comp, body, ..)) =
                bodies_query.get(selected_entity)
            {
                let type_name = match body.body_type {
                    BodyType::Protostar => "THE STAR (Protostar)",
                    BodyType::MainSequenceStar => "THE STAR (Main Sequence)",
                    BodyType::BrownDwarf => "BROWN DWARF (Sub-Stellar)",
                    BodyType::RedDwarf => "RED DWARF STAR (M-Type)",
                    BodyType::YellowDwarf => "YELLOW DWARF STAR (G2V)",
                    BodyType::BlueGiant => "BLUE GIANT STAR (B-Type)",
                    BodyType::BlueSupergiant => "BLUE SUPERGIANT (O-Type)",
                    BodyType::RedGiant => "RED GIANT STAR",
                    BodyType::RedSupergiant => "RED SUPERGIANT STAR",
                    BodyType::Hypergiant => "LUMINOUS HYPERGIANT",
                    BodyType::WolfRayet => "WOLF-RAYET STAR",
                    BodyType::WhiteDwarf => "WHITE DWARF REMNANT",
                    BodyType::NeutronStar => "NEUTRON STAR REMNANT",
                    BodyType::Pulsar => "RELATIVISTIC PULSAR",
                    BodyType::Magnetar => "MAGNETAR REMNANT",
                    BodyType::BlackHole => "STELLAR-MASS BLACK HOLE",
                    BodyType::GasGiant => "GAS GIANT",
                    BodyType::IceGiant => "ICE GIANT",
                    BodyType::TerrestrialPlanet => "TERRESTRIAL PLANET",
                    BodyType::Protoplanet => "PROTOPLANETARY EMBRYO",
                    BodyType::Planetesimal => "PLANETESIMAL",
                    BodyType::Asteroid => "ASTEROID",
                    BodyType::Comet => "COMET",
                    BodyType::DustGrain => "DUST GRAIN",
                    BodyType::DebrisRing => "DEBRIS RING",
                    BodyType::Moon => "NATURAL MOON / SATELLITE",
                };
                let mass_str = if mass.0 >= 0.01 {
                    format!(
                        "{:.2} M_sun ({:.1} M_J)",
                        mass.0,
                        mass.0 / JUPITER_MASS_SOLAR
                    )
                } else {
                    format!("{:.2} M_earth", mass.0 / EARTH_MASS_SOLAR)
                };
                let dist_au = if pos.0.is_finite() {
                    pos.0.length()
                } else {
                    0.0
                };
                let speed_km_s = if vel.0.is_finite() {
                    vel.0.length() * AU_PER_YR_TO_KM_PER_S
                } else {
                    0.0
                };
                let rad_km = (rad.0 * AU_TO_KM).max(1.0);

                toast_text.0 = format!(
                    ">> SELECTED: {} [{}]  |  Mass: {}  |  Radius: {:.0} km  |  Dist: {:.2} AU  |  Speed: {:.1} km/s  |  Temp: {:.0} K",
                    body.name.to_uppercase(),
                    type_name,
                    mass_str,
                    rad_km,
                    dist_au,
                    speed_km_s,
                    temp.0,
                );
            } else {
                toast_text.0 = ">> PROTOSTELLAR LIVE // Select any planet or the Sun".to_string();
            }
        } else {
            toast_text.0 = ">> PROTOSTELLAR LIVE // Click any planet or the Sun".to_string();
        }
    }

    // 1. Header Text
    if let Ok(mut text) = header_query.single_mut() {
        let time_formatted = if sim_time.elapsed_years > 1_000_000.0 {
            format!("{:.2} Myr", sim_time.elapsed_years / 1_000_000.0)
        } else if sim_time.elapsed_years > 1_000.0 {
            format!("{:.1} kyr", sim_time.elapsed_years / 1_000.0)
        } else {
            format!("{:.2} yr", sim_time.elapsed_years)
        };

        let phase_str = match phase_mgr.current_phase {
            crate::game::phases::SystemPhase::MolecularCloudCollapse => "MOLECULAR CLOUD COLLAPSE",
            crate::game::phases::SystemPhase::ProtoplanetaryDisk => "PROTOPLANETARY DISK FORMATION",
            crate::game::phases::SystemPhase::StarIgnition => "STAR IGNITION (FUSION ONSET)",
            crate::game::phases::SystemPhase::PlanetaryAccretion => {
                "PLANETARY ACCRETION & EMBRYO GROWTH"
            }
            crate::game::phases::SystemPhase::LateHeavyBombardment => {
                "LATE HEAVY BOMBARDMENT (2:1 RESONANCE & MIGRATION)"
            }
            crate::game::phases::SystemPhase::MatureSolarSystem => "MATURE SOLAR SYSTEM",
            crate::game::phases::SystemPhase::StellarMetamorphosis => {
                "STELLAR METAMORPHOSIS (RED GIANT / WHITE DWARF)"
            }
        };

        let gas_status = if config.gas_density_scale > 0.05 {
            format!("{:.0}%", config.gas_density_scale * 100.0)
        } else {
            "Dispersed".to_string()
        };

        let active_goal = phase_mgr
            .milestones
            .iter()
            .find(|m| !m.achieved)
            .map(|m| format!("{}: {}", m.title, m.prompt))
            .unwrap_or_else(|| "All Formation Milestones Completed!".to_string());

        let lhb_info = if lhb_state.is_active {
            format!(
                "\nLHB Migration: {:.0}% (2:1 Resonance: {:.2}:1 | Comets Perturbed: {})",
                lhb_state.migration_progress * 100.0,
                lhb_state.resonance_ratio,
                lhb_state.comets_scattered
            )
        } else {
            "".to_string()
        };

        text.0 = format!(
            "Phase: {}\nTime: T + {} | Star: {:.2} M_sun\nSwarm: {} / 50,000 active particles | Gas: {}\nPlanets: {} | Protoplanets: {}{}\nGoal: {}",
            phase_str,
            time_formatted,
            phase_mgr.star_mass,
            config.active_particles,
            gas_status,
            phase_mgr.planet_count,
            phase_mgr.protoplanet_count,
            lhb_info,
            active_goal,
        );
    }

    // 2. Time Warp & Diagnostics Text
    if let Ok(mut text) = time_query.single_mut() {
        let speed_str = time_warp.human_readable_speed();

        let tool_str = match player_state.active_tool {
            PlayerTool::Inspect => "INSPECT & LIVE-EDIT",
            PlayerTool::GravitationalTractor => "GRAVITATIONAL TRACTOR",
            PlayerTool::GravitationalImpulse => "DELTA-V IMPULSE",
            PlayerTool::MassInjection => "MASS INJECTION",
            PlayerTool::DensityWave => "DENSITY WAVE",
        };

        let drift_pct = if energy_monitor.relative_energy_drift.is_finite() {
            (energy_monitor.relative_energy_drift * 100.0).clamp(0.0, 999.0)
        } else {
            0.0
        };

        text.0 = format!(
            "SPEED: {}\nTOOL: {}\nOVERLAY: {} [V]\nAccretion: Active (Boost: {:.0}x)\nEnergy Drift: {:.4}%\nSim Steps: {}",
            speed_str,
            tool_str,
            player_state.overlay_mode.display_name(),
            config.accretion_rate_multiplier,
            drift_pct,
            sim_time.step_count,
        );
    }

    // 3. Dedicated Bottom Simulation Elapsed Time Clock
    if let Ok(mut text) = bottom_timer_query.single_mut() {
        let yr = sim_time.elapsed_years;
        let time_formatted = if yr >= 1_000_000.0 {
            format!(
                "{:.3} Million Years ({:.4} Myr)",
                yr / 1_000_000.0,
                yr / 1_000_000.0
            )
        } else if yr >= 1_000.0 {
            format!(
                "{:.1} Thousand Years ({:.2} kyr)",
                yr / 1_000.0,
                yr / 1_000.0
            )
        } else {
            format!("{:.2} Years", yr)
        };

        let status_str = if time_warp.is_paused {
            "PAUSED [Space to Resume]"
        } else {
            "FLOWING"
        };

        text.0 = format!(
            "SIMULATION ELAPSED TIME: {}\nSPEED: {} | STATUS: {}",
            time_formatted,
            time_warp.human_readable_speed(),
            status_str,
        );
    }

    // 4. Telemetry Inspector Text
    if let Ok(mut text) = inspector_query.single_mut() {
        if let Some(selected_entity) = player_state.selected_entity {
            if let Ok((
                pos,
                vel,
                mass,
                rad,
                temp,
                comp,
                body,
                opt_diff,
                opt_spin,
                opt_ignition,
                opt_vol,
                opt_rings,
                opt_climate,
                opt_bio,
                opt_evo,
            )) = bodies_query.get(selected_entity)
            {
                let dist_au = if pos.0.is_finite() {
                    pos.0.length()
                } else {
                    0.0
                };
                let speed_au_yr = if vel.0.is_finite() {
                    vel.0.length()
                } else {
                    0.0
                };
                let speed_km_s = speed_au_yr * AU_PER_YR_TO_KM_PER_S;

                let mass_str = if mass.0 >= 0.01 {
                    format!(
                        "{:.3} M_sun ({:.1} M_J)",
                        mass.0,
                        mass.0 / JUPITER_MASS_SOLAR
                    )
                } else {
                    format!(
                        "{:.2} M_earth ({:.4} M_sun)",
                        mass.0 / EARTH_MASS_SOLAR,
                        mass.0
                    )
                };

                let radius_km = rad.0 * AU_TO_KM;
                let density_g_cm3 = (comp.average_density() * SOLAR_MASS_KG
                    / (AU_TO_METERS.powi(3) * 1000.0))
                    .clamp(0.01, 20.0);

                let period_str = if dist_au > 0.05 && !body.body_type.is_star_or_remnant() {
                    let p_yr = dist_au.powf(1.5) / phase_mgr.star_mass.max(0.1).sqrt();
                    if p_yr >= 1.0 {
                        format!(" | Period: {:.2} yr", p_yr)
                    } else {
                        format!(" | Period: {:.1} days", p_yr * 365.25)
                    }
                } else {
                    "".to_string()
                };

                let spin_str = if let Some(spin) = opt_spin {
                    format!(
                        " | Day: {:.1}h | Tilt: {:.1} deg",
                        spin.rotation_period_hours, spin.axial_tilt_degrees
                    )
                } else {
                    "".to_string()
                };

                let diff_str = if let Some(diff) = opt_diff {
                    if diff.is_differentiated {
                        let core_km = diff.core_radius_au * AU_TO_KM;
                        let mantle_km =
                            (diff.mantle_radius_au - diff.core_radius_au).max(0.0) * AU_TO_KM;
                        let crust_km = diff.crust_thickness_au * AU_TO_KM;
                        format!(
                            "\nStructure: Differentiated (Core: {:.0} km | Mantle: {:.0} km | Crust: {:.0} km)\nDynamo: {:.2} G | Core Temp: {:.0} K",
                            core_km, mantle_km, crust_km, diff.magnetic_field_gauss, diff.core_temp_k
                        )
                    } else {
                        "\nStructure: Undifferentiated Chondritic Mixture".to_string()
                    }
                } else {
                    "".to_string()
                };

                let vol_str = if let Some(vol) = opt_vol {
                    format!(
                        "\nVolatiles: {:.4} M_earth Water Delivered | Ocean Coverage: {:.0}%\nAtmospheric Pressure: {:.2} bar | Icy Bombardment Impacts: {}",
                        vol.delivered_water_m_earth,
                        vol.ocean_coverage_frac * 100.0,
                        vol.atmospheric_pressure_bar,
                        vol.cometary_impact_count
                    )
                } else {
                    "".to_string()
                };

                let rings_str = if let Some(ring) = opt_rings {
                    let inner_km = ring.inner_radius_au as f64 * AU_TO_KM;
                    let outer_km = ring.outer_radius_au as f64 * AU_TO_KM;
                    format!(
                        "\nRing System: Active (Span: {:.0} - {:.0} km | Opacity: {:.0}% | {:.0}% Ice)",
                        inner_km,
                        outer_km,
                        ring.optical_depth * 100.0,
                        ring.ice_fraction * 100.0
                    )
                } else {
                    "".to_string()
                };

                let climate_str = if let Some(climate) = opt_climate {
                    let regime_name = match climate.climate_regime {
                        crate::simulation::components::ClimateRegime::SnowballIceAge => {
                            "Frozen Snowball (Ice Age)"
                        }
                        crate::simulation::components::ClimateRegime::TemperateHabitable => {
                            "Temperate Habitable"
                        }
                        crate::simulation::components::ClimateRegime::RunawayVenusian => {
                            "Runaway Greenhouse (Venusian)"
                        }
                        crate::simulation::components::ClimateRegime::GasGiantEnvelope => {
                            "Gas Giant Envelope"
                        }
                        crate::simulation::components::ClimateRegime::AirlessVacuum => {
                            "Airless Vacuum"
                        }
                    };
                    format!(
                        "\nClimate: {} (T_surf: {:.0} K | Albedo: {:.2} | Greenhouse: +{:.0} K)",
                        regime_name,
                        climate.surface_temperature_k,
                        climate.albedo,
                        climate.greenhouse_delta_k
                    )
                } else {
                    "".to_string()
                };

                let bio_str = if let Some(bio) = opt_bio {
                    let status = if bio.biomass_coverage_frac >= 0.50 {
                        "Thriving Eden"
                    } else if bio.biomass_coverage_frac >= 0.05 {
                        "Colonizing Biosphere"
                    } else if bio.habitability_score >= 0.40 {
                        "Pre-Biotic Prime"
                    } else {
                        "Sterile / Hostile"
                    };
                    format!(
                        "\nBiosphere: {} (Biomass: {:.0}% | O2: {:.1}% | Habitability: {:.0}%)",
                        status,
                        bio.biomass_coverage_frac * 100.0,
                        bio.oxygen_fraction * 100.0,
                        bio.habitability_score * 100.0
                    )
                } else {
                    "".to_string()
                };

                let star_extra_str = if let Some(ignition) = opt_ignition {
                    let core_temp_mk = ignition.core_temperature / 1.0e6;
                    let fusion_pct = ignition.fusion_fraction * 100.0;
                    let evo_str = if let Some(evo) = opt_evo {
                        match evo.phase {
                            StellarEvolutionPhase::ProtostarContraction => {
                                format!(
                                    "Hayashi Track Contraction (Fuel: {:.0}% H)",
                                    evo.hydrogen_core_fraction * 100.0
                                )
                            }
                            StellarEvolutionPhase::MainSequence => {
                                format!(
                                    "Stable Main Sequence (Core Fuel: {:.1}% H)",
                                    evo.hydrogen_core_fraction * 100.0
                                )
                            }
                            StellarEvolutionPhase::RedGiantBranch => {
                                format!(
                                    "RED GIANT BRANCH (R: {:.2} AU | L: {:.0} L☉ | Engulfing Inner Planets)",
                                    rad.0,
                                    (rad.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4)
                                )
                            }
                            StellarEvolutionPhase::RedSupergiantBranch => {
                                format!(
                                    "RED SUPERGIANT BRANCH (R: {:.2} AU | Massive Core Burning)",
                                    rad.0
                                )
                            }
                            StellarEvolutionPhase::SupernovaExplosion => {
                                format!(
                                    "💥 SUPERNOVA CORE-COLLAPSE (Blast: {:.1} AU @ 15,000 km/s)",
                                    evo.nebula_expansion_radius_au
                                )
                            }
                            StellarEvolutionPhase::HeliumFlashAgb => {
                                format!(
                                    "AGB SUPERGIANT (Core He Fuel: {:.1}% | R: {:.2} AU)",
                                    evo.helium_core_fraction * 100.0,
                                    rad.0
                                )
                            }
                            StellarEvolutionPhase::PlanetaryNebulaEjection => {
                                format!(
                                    "PLANETARY NEBULA EJECTION (Shell: {:.1} AU | Shedding Envelope Mass)",
                                    evo.nebula_expansion_radius_au
                                )
                            }
                            StellarEvolutionPhase::WhiteDwarf => {
                                format!(
                                    "DEGENERATE WHITE DWARF REMNANT (Earth-Sized Core | T: {:.0} K | B: 10^6 G)",
                                    temp.0
                                )
                            }
                            StellarEvolutionPhase::NeutronStarPulsar => {
                                "⚡ NEUTRON STAR / PULSAR REMNANT (B: 10^12 G | Synchrotron Lighthouse Jets)".to_string()
                            }
                            StellarEvolutionPhase::MagnetarRemnant => {
                                "🧲 MAGNETAR REMNANT (B: 10^15 G | Extreme Magnetic Reconnection Arcs)".to_string()
                            }
                            StellarEvolutionPhase::BlackHoleRemnant => {
                                "🕳️ STELLAR-MASS BLACK HOLE (Event Horizon & Relativistic Accretion Disk)".to_string()
                            }
                        }
                    } else {
                        "Active Hydrogen Fusion".to_string()
                    };

                    let status = if ignition.is_ignited {
                        format!(
                            "{}\nSolar Wind Shockwave: {:.2} AU | Gas Dispersal: {:.0}%",
                            evo_str,
                            ignition.shockwave_radius,
                            (1.0 - config.gas_density_scale) * 100.0
                        )
                    } else {
                        format!(
                            "Kelvin-Helmholtz Core Heating (Progress: {:.1}%)\nIgnition Threshold: 10.0 MK [Press 'I' or Click Button Below to Ignite]",
                            fusion_pct
                        )
                    };
                    format!(
                        "\nStellar Core Temp: {:.2} MK | Fusion: {:.1}%\nStellar State: {}",
                        core_temp_mk, fusion_pct, status
                    )
                } else {
                    "".to_string()
                };

                let type_str = match body.body_type {
                    BodyType::Protostar => "Central Star (Protostar)",
                    BodyType::MainSequenceStar => "Main Sequence Star",
                    BodyType::BrownDwarf => "Brown Dwarf (Sub-Stellar)",
                    BodyType::RedDwarf => "Red Dwarf Star (M-Type)",
                    BodyType::YellowDwarf => "Yellow Dwarf Star (G2V)",
                    BodyType::BlueGiant => "Blue Giant Star (B-Type)",
                    BodyType::BlueSupergiant => "Blue Supergiant Star (O-Type)",
                    BodyType::RedGiant => "Red Giant Star",
                    BodyType::RedSupergiant => "Red Supergiant Star",
                    BodyType::Hypergiant => "Luminous Hypergiant",
                    BodyType::WolfRayet => "Wolf-Rayet Star",
                    BodyType::WhiteDwarf => "White Dwarf Remnant",
                    BodyType::NeutronStar => "Neutron Star Remnant",
                    BodyType::Pulsar => "Relativistic Pulsar Remnant",
                    BodyType::Magnetar => "Magnetar Remnant",
                    BodyType::BlackHole => "Stellar-Mass Black Hole",
                    BodyType::GasGiant => "Gas Giant Planet",
                    BodyType::IceGiant => "Ice Giant Planet",
                    BodyType::TerrestrialPlanet => "Terrestrial Planet",
                    BodyType::Protoplanet => "Protoplanetary Embryo",
                    BodyType::Planetesimal => "Planetesimal",
                    BodyType::Asteroid => "Asteroid",
                    BodyType::Comet => "Comet",
                    BodyType::DustGrain => "Dust Grain",
                    BodyType::DebrisRing => "Debris Ring",
                    BodyType::Moon => "Natural Moon / Satellite",
                };

                let norm = comp.normalized();
                let rock_pct = ((norm.silicate_frac + norm.organics_frac) * 100.0).round();
                let ice_pct = (norm.ice_frac * 100.0).round();
                let metal_pct = (norm.metal_frac * 100.0).round();
                let gas_pct = (100.0f64 - rock_pct - ice_pct - metal_pct).max(0.0);

                text.0 = format!(
                    "==================================================\n  >> SELECTED: {}\n  >> CLASSIFICATION: {}\n==================================================\nMass: {}\nRadius: {:.0} km ({:.4} AU)\nDensity: {:.2} g/cm3 | Temp: {:.0} K{}{}\nDistance from Star: {:.2} AU | Speed: {:.1} km/s\nComposition: {:.0}% Rock | {:.0}% Ice | {:.0}% Metal | {:.0}% Gas{}{}{}{}{}{}",
                    body.name.to_uppercase(),
                    type_str.to_uppercase(),
                    mass_str,
                    radius_km,
                    rad.0,
                    density_g_cm3,
                    temp.0,
                    spin_str,
                    period_str,
                    dist_au,
                    speed_km_s,
                    rock_pct,
                    ice_pct,
                    metal_pct,
                    gas_pct,
                    diff_str,
                    vol_str,
                    rings_str,
                    climate_str,
                    bio_str,
                    star_extra_str,
                );
            } else {
                text.0 = "Selected body was absorbed in an accretion merger.".to_string();
            }
        } else {
            text.0 = "No celestial body selected.\nClick on the Star or Planets above (or in 3D) to inspect & live-edit.\n[Tab] Next Body | [F] Focus Target | [WASD] Free-Fly View".to_string();
        }
    }
}
