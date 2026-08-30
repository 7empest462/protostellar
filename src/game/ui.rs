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
            UiButtonAction::ExpandOrbit => "[O]: Gently expands semi-major axis (+10% orbital radius).",
            UiButtonAction::ContractOrbit => "[L]: Gently contracts semi-major axis (-10% orbital radius).",
            UiButtonAction::CycleComposition => "[C]: Cycles bulk chemical composition (Metal-Rich -> Rocky -> Carbonaceous -> Icy -> Solar Gas).",
            UiButtonAction::BoostDeltaV => "[B]: Applies +15% prograde velocity boost (+dv), stretching orbit into an ellipse.",
            UiButtonAction::BrakeDeltaV => "[K]: Applies -15% retrograde velocity brake (-dv), lowering perihelion toward the star.",
            UiButtonAction::InjectEmbryo => "[M]: Injects a new protoplanetary embryo seed in a stable Keplerian orbit.",
            UiButtonAction::VaporizeBody => "[Del]: Vaporizes the selected body into microscopic accretion dust particles.",
            UiButtonAction::FocusLock => "[F]: Focus camera directly on currently selected celestial body.",
            UiButtonAction::ResetView => "[R]: Resets camera focus to the central star overview (45 AU radius).",
            UiButtonAction::DeselectBody => "[Esc]: Deselect current body and return to free orbital camera.",
            UiButtonAction::FixOrbit => "[Z]: Circularizes and stabilizes orbit into a clean Keplerian circle (e = 0.0).",
            UiButtonAction::IgniteStar => "[I]: Ignites Hydrogen Core Fusion (or triggers Coronal Solar Blast if already ignited).",
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
            &CelestialBody,
            Option<&CentralStar>,
        ),
        Without<PanOrbitCamera>,
    >,
    mut camera_query: Query<&mut PanOrbitCamera>,
    mut star_ignition_query: Query<&mut IgnitionState, With<CentralStar>>,
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
                            .find(|(.., is_star)| is_star.is_some())
                            .map(|(e, ..)| e);
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
                            .filter(|(.., is_star)| is_star.is_none())
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
                            .find(|(.., body, _)| {
                                body.name.to_lowercase().contains("mercury")
                                    || body.name.to_lowercase().contains("ceres")
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
                            .filter(|(.., is_star)| is_star.is_none())
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
                            .find(|(.., body, _)| body.name.to_lowercase().contains("earth"))
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
                            .filter(|(.., is_star)| is_star.is_none())
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
                            .find(|(.., body, _)| body.name.to_lowercase().contains("jupiter"))
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
                            .filter(|(.., is_star)| is_star.is_none())
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
                            if matches!(
                                body.body_type,
                                BodyType::Protostar | BodyType::MainSequenceStar
                            ) {
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
                            if let Ok((_, mut mass, mut radius, _, _, comp, body, _)) =
                                selected_query.get_mut(ent)
                            {
                                mass.0 *= 1.25;
                                let avg_density = comp.average_density();
                                radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                                    .cbrt()
                                    .max(EARTH_RADIUS_AU * 0.1);
                                toast.message = format!(
                                    "➕ {} Mass Increased: {:.3} M⊕ (+25%)",
                                    body.name,
                                    mass.0 / EARTH_MASS_SOLAR
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
                            if let Ok((_, mut mass, mut radius, _, _, comp, body, _)) =
                                selected_query.get_mut(ent)
                            {
                                mass.0 *= 0.80;
                                let avg_density = comp.average_density();
                                radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                                    .cbrt()
                                    .max(EARTH_RADIUS_AU * 0.1);
                                toast.message = format!(
                                    "➖ {} Mass Decreased: {:.3} M⊕ (-20%)",
                                    body.name,
                                    mass.0 / EARTH_MASS_SOLAR
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
                            if let Ok((_, _, _, mut pos, mut vel, _, body, _)) =
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
                            if let Ok((_, _, _, mut pos, mut vel, _, body, _)) =
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
                            if let Ok((_, mut mass, mut radius, _, _, mut comp, body, _)) =
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
                            if let Ok((_, _, _, _, mut vel, _, body, _)) =
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
                            if let Ok((_, _, _, _, mut vel, _, body, _)) =
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
                            if let Ok((_, _, _, mut pos, mut vel, _, body, _)) =
                                selected_query.get_mut(ent)
                            {
                                if !matches!(
                                    body.body_type,
                                    BodyType::Protostar | BodyType::MainSequenceStar
                                ) {
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
                            if let Ok((.., body, _)) = selected_query.get(ent) {
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
                        if let Ok(mut ignition) = star_ignition_query.single_mut() {
                            if !ignition.is_ignited {
                                ignition.core_temperature = 1.0e7;
                                toast.message =
                                    "⭐ Hydrogen Core Fusion Ignited! Solar Wind Shockwave Sweeping the System!".to_string();
                            } else {
                                ignition.shockwave_radius = 0.1;
                                toast.message =
                                    "☀️ Coronal Mass Ejection & Solar Blast Triggered!".to_string();
                            }
                            toast.timer = 5.0;
                        }
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
                    BodyType::Protostar => "THE SUN (Protostar)",
                    BodyType::MainSequenceStar => "THE SUN (Main Sequence Star)",
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
            crate::game::phases::SystemPhase::MatureSolarSystem => "MATURE SOLAR SYSTEM",
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

        text.0 = format!(
            "Phase: {}\nTime: T + {} | Star: {:.2} M_sun\nSwarm: {} / 50,000 active particles | Gas: {}\nPlanets: {} | Protoplanets: {}\nGoal: {}",
            phase_str,
            time_formatted,
            phase_mgr.star_mass,
            config.active_particles,
            gas_status,
            phase_mgr.planet_count,
            phase_mgr.protoplanet_count,
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
            if let Ok((pos, vel, mass, rad, temp, comp, body, opt_diff, opt_spin, opt_ignition)) =
                bodies_query.get(selected_entity)
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

                let period_str = if dist_au > 0.05
                    && !matches!(
                        body.body_type,
                        BodyType::Protostar | BodyType::MainSequenceStar
                    ) {
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

                let star_extra_str = if let Some(ignition) = opt_ignition {
                    let core_temp_mk = ignition.core_temperature / 1.0e6;
                    let fusion_pct = ignition.fusion_fraction * 100.0;
                    let status = if ignition.is_ignited {
                        format!("ACTIVE HYDROGEN FUSION (Main Sequence)\nSolar Wind Shockwave: {:.2} AU | Gas Dispersal: {:.0}%", ignition.shockwave_radius, (1.0 - config.gas_density_scale) * 100.0)
                    } else {
                        format!("Kelvin-Helmholtz Core Heating (Progress: {:.1}%)\nIgnition Threshold: 10.0 MK [Press 'I' or Click Button Below to Ignite]", fusion_pct)
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
                let gas_pct = (100.0 - rock_pct - ice_pct - metal_pct).max(0.0);

                text.0 = format!(
                    "==================================================\n  >> SELECTED: {}\n  >> CLASSIFICATION: {}\n==================================================\nMass: {}\nRadius: {:.0} km ({:.4} AU)\nDensity: {:.2} g/cm3 | Temp: {:.0} K{}{}\nDistance from Star: {:.2} AU | Speed: {:.1} km/s\nComposition: {:.0}% Rock | {:.0}% Ice | {:.0}% Metal | {:.0}% Gas{}{}",
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
