//! Types, components, resources, and classification helpers for the HUD and UI system.

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
pub use crate::simulation::disk::belts::BeltZone;
use crate::utils::constants::*;
use bevy::prelude::*;
use std::collections::BTreeSet;

pub type SelectedWorldQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Mass,
        &'static mut Radius,
        &'static mut SimPosition,
        &'static mut SimVelocity,
        &'static mut Composition,
        &'static mut CelestialBody,
        Option<&'static CentralStar>,
        Option<&'static mut IgnitionState>,
        Option<&'static mut StellarEvolutionState>,
        Option<&'static mut Temperature>,
        Option<&'static mut Luminosity>,
    ),
    Without<PanOrbitCamera>,
>;

/// Marker for the live HUD simulation time and year counter.
#[derive(Component)]
pub struct HudTimeWarpText;

/// Marker for the selected body telemetry details in the bottom-left panel.
#[derive(Component)]
pub struct HudInspectorText;

/// Marker for the top-left simulation phase & statistics readout.
#[derive(Component)]
pub struct HudHeaderStatsText;

/// Marker for the dynamic action button tooltip explanation text.
#[derive(Component)]
pub struct HudActionTooltipText;

/// Marker for temporary floating status toasts.
#[derive(Component)]
pub struct HudToastText;

/// Marker for the fixed bottom-right persistent simulation clock.
#[derive(Component)]
pub struct HudBottomTimerText;

/// Marker for the top-center body selector bar container.
#[derive(Component)]
pub struct QuickBodySelectorBar;

/// State configuration for the top celestial body shortcut bar.
#[derive(Resource, Debug, Clone, Default)]
pub struct QuickBarState {
    pub is_minimized: bool,
    pub show_minor_bodies: bool,
    pub show_embryos: bool,
    /// Active astronomical belts expanded to reveal individual member buttons.
    pub expanded_belts: BTreeSet<BeltZone>,
}

/// Global visibility and panel collapse states for the HUD overlay.
#[derive(Resource, Debug, Clone, Default)]
pub struct HudVisibilityState {
    /// When true, all HUD panels are hidden for an immersive, unobstructed full-screen view.
    pub is_full_screen_clean: bool,
    /// Whether the top-left simulation phase & stats panel is collapsed.
    pub top_left_minimized: bool,
    /// Whether the top-right telemetry & speed panel is collapsed.
    pub top_right_minimized: bool,
    /// Whether the bottom-left selected body inspector panel is collapsed.
    pub inspector_minimized: bool,
    /// Whether the scenario presets bar is collapsed.
    pub scenarios_minimized: bool,
    /// Whether the top-right view and tool controls card is collapsed.
    pub top_controls_minimized: bool,
    /// Whether the bottom-right navigation and shortcuts panel is collapsed.
    pub bottom_right_minimized: bool,
    /// Whether the bottom-center simulation timer and speed dock is collapsed.
    pub bottom_center_minimized: bool,
    /// Whether the floating telemetry & climate graphing panel is collapsed.
    pub telemetry_minimized: bool,
}

/// Discriminant component for HUD panel containers whose visibility is toggled dynamically.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudPanelElement {
    RootContainer,
    TopLeftPanel,
    TopLeftPill,
    TopRightPanel,
    TopRightPill,
    InspectorPanel,
    InspectorChip,
    ScenarioPresets,
    ScenarioPresetsPill,
    TopControlsPanel,
    TopControlsPill,
    BottomRightPanel,
    BottomRightPill,
    BottomCenterPanel,
    BottomCenterPill,
    PlanetBuilderPill,
    TelemetryPanelPill,
    OrbitModeBadge,
    OverlayModeBadge,
}

/// Marker for the scrollable container inside the Target Inspector panel.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScrollableInspector;

/// Discriminant component for dynamic text elements within the HUD overlay.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudDynamicText {
    InspectorChip,
    FullScreenBadge,
    OrbitModeBadge,
    OverlayModeBadge,
}

/// Marker for the bottom-center live telemetry and toast container.
#[derive(Component)]
pub struct HudToastContainer;

/// Planetary archetype templates for the Interactive Planet Builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderPreset {
    EarthLike,
    SuperEarth,
    JupiterLike,
    SuperJupiter,
    HeavySuperJupiter,
    BrownDwarf,
    WaterWorld,
    MoltenProtoplanet,
    IceGiant,
    RogueInvader,
    RedDwarfStar,
}

impl BuilderPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            BuilderPreset::EarthLike => "🌍 Earth-like",
            BuilderPreset::SuperEarth => "💎 Super-Earth (3.5 M_E)",
            BuilderPreset::JupiterLike => "🪐 Jupiter-like",
            BuilderPreset::SuperJupiter => "👑 Super-Jup (3.5 M_J)",
            BuilderPreset::HeavySuperJupiter => "🔮 Heavy Super-J (8.0 M_J)",
            BuilderPreset::BrownDwarf => "🟤 Brown Dwarf (30 M_J)",
            BuilderPreset::WaterWorld => "🌊 Water World",
            BuilderPreset::MoltenProtoplanet => "🌋 Molten Protoplanet",
            BuilderPreset::IceGiant => "❄️ Ice Giant",
            BuilderPreset::RogueInvader => "☄️ Rogue Invader",
            BuilderPreset::RedDwarfStar => "☀️ Red Dwarf Star",
        }
    }
}

/// Interactive custom planet builder and spawner state.
#[derive(Resource, Debug, Clone)]
pub struct PlanetBuilderState {
    pub is_open: bool,
    pub active_preset: BuilderPreset,
    pub custom_name: String,
    pub mass_solar: f64,
    pub semi_major_axis_au: f64,
    pub eccentricity: f64,
    pub rock_frac: f32,
    pub ice_frac: f32,
    pub metal_frac: f32,
    pub gas_frac: f32,
    pub click_to_spawn_mode: bool,
    pub is_minimized: bool,
}

impl Default for PlanetBuilderState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_preset: BuilderPreset::EarthLike,
            custom_name: "Proto-Earth".to_string(),
            mass_solar: EARTH_MASS_SOLAR * 1.0,
            semi_major_axis_au: 1.0,
            eccentricity: 0.016,
            rock_frac: 0.68,
            ice_frac: 0.02,
            metal_frac: 0.30,
            gas_frac: 0.0,
            click_to_spawn_mode: false,
            is_minimized: false,
        }
    }
}

impl PlanetBuilderState {
    pub fn apply_preset(&mut self, preset: BuilderPreset) {
        self.active_preset = preset;
        match preset {
            BuilderPreset::EarthLike => {
                self.custom_name = "New Earth".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 1.0;
                self.semi_major_axis_au = 1.0;
                self.eccentricity = 0.016;
                self.rock_frac = 0.68;
                self.ice_frac = 0.02;
                self.metal_frac = 0.30;
                self.gas_frac = 0.0;
            }
            BuilderPreset::SuperEarth => {
                self.custom_name = "Kepler-SuperEarth".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 3.5;
                self.semi_major_axis_au = 1.35;
                self.eccentricity = 0.025;
                self.rock_frac = 0.65;
                self.ice_frac = 0.03;
                self.metal_frac = 0.30;
                self.gas_frac = 0.02;
            }
            BuilderPreset::JupiterLike => {
                self.custom_name = "Jovian-Prime".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 1.0;
                self.semi_major_axis_au = 5.2;
                self.eccentricity = 0.048;
                self.rock_frac = 0.03;
                self.ice_frac = 0.02;
                self.metal_frac = 0.01;
                self.gas_frac = 0.94;
            }
            BuilderPreset::SuperJupiter => {
                self.custom_name = "Super-Jupiter".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 3.5;
                self.semi_major_axis_au = 3.2;
                self.eccentricity = 0.08;
                self.rock_frac = 0.02;
                self.ice_frac = 0.02;
                self.metal_frac = 0.01;
                self.gas_frac = 0.95;
            }
            BuilderPreset::HeavySuperJupiter => {
                self.custom_name = "Mega-Jovian".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 8.0;
                self.semi_major_axis_au = 9.5;
                self.eccentricity = 0.06;
                self.rock_frac = 0.01;
                self.ice_frac = 0.01;
                self.metal_frac = 0.01;
                self.gas_frac = 0.97;
            }
            BuilderPreset::BrownDwarf => {
                self.custom_name = "Sub-Stellar Dwarf".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 30.0;
                self.semi_major_axis_au = 22.0;
                self.eccentricity = 0.05;
                self.rock_frac = 0.01;
                self.ice_frac = 0.01;
                self.metal_frac = 0.01;
                self.gas_frac = 0.97;
            }
            BuilderPreset::WaterWorld => {
                self.custom_name = "Oceania".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 2.5;
                self.semi_major_axis_au = 1.35;
                self.eccentricity = 0.02;
                self.rock_frac = 0.30;
                self.ice_frac = 0.60;
                self.metal_frac = 0.10;
                self.gas_frac = 0.0;
            }
            BuilderPreset::MoltenProtoplanet => {
                self.custom_name = "Vulcan".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 0.15;
                self.semi_major_axis_au = 0.45;
                self.eccentricity = 0.15;
                self.rock_frac = 0.75;
                self.ice_frac = 0.0;
                self.metal_frac = 0.25;
                self.gas_frac = 0.0;
            }
            BuilderPreset::IceGiant => {
                self.custom_name = "Sub-Neptune".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 15.0;
                self.semi_major_axis_au = 19.2;
                self.eccentricity = 0.04;
                self.rock_frac = 0.20;
                self.ice_frac = 0.65;
                self.metal_frac = 0.05;
                self.gas_frac = 0.10;
            }
            BuilderPreset::RogueInvader => {
                self.custom_name = "Nemesis-Invader".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 3.5;
                self.semi_major_axis_au = 35.0;
                self.eccentricity = 1.25;
                self.rock_frac = 0.02;
                self.ice_frac = 0.02;
                self.metal_frac = 0.01;
                self.gas_frac = 0.95;
            }
            BuilderPreset::RedDwarfStar => {
                self.custom_name = "Companion Star".to_string();
                self.mass_solar = 0.15;
                self.semi_major_axis_au = 12.0;
                self.eccentricity = 0.05;
                self.rock_frac = 0.0;
                self.ice_frac = 0.0;
                self.metal_frac = 0.0;
                self.gas_frac = 1.0;
            }
        }
    }
}

#[derive(Component)]
pub struct PlanetBuilderPanel;

#[derive(Component)]
pub struct PlanetBuilderInfoText;

/// State configuration for the Telemetry & Climate Graphing Panel (`[F10]`).
#[derive(Resource, Debug, Clone)]
pub struct TelemetryPanelState {
    pub is_open: bool,
    pub selected_metric: crate::simulation::telemetry::TelemetryMetric,
    pub last_export_status: Option<String>,
}

impl Default for TelemetryPanelState {
    fn default() -> Self {
        Self {
            is_open: false,
            selected_metric: crate::simulation::telemetry::TelemetryMetric::Habitability,
            last_export_status: None,
        }
    }
}

/// Marker component for the floating Telemetry & Climate Graphing Panel.
#[derive(Component)]
pub struct TelemetryGraphPanel;

/// Marker for the tracked body title in the Telemetry HUD.
#[derive(Component)]
pub struct TelemetryGraphBodyTitleText;

/// Marker for the live numeric readouts in the Telemetry HUD.
#[derive(Component)]
pub struct TelemetryGraphReadoutText;

/// Marker for the live sparkline graph text in the Telemetry HUD.
#[derive(Component)]
pub struct TelemetryGraphSparklineText;

/// Marker for the CSV export status feedback text in the Telemetry HUD.
#[derive(Component)]
pub struct TelemetryExportStatusText;

/// Marker for the metric switcher button pill in the Telemetry HUD.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryMetricButton(pub crate::simulation::telemetry::TelemetryMetric);

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
    SelectEntity(Entity),
    SelectStar,
    SelectMercury,
    SelectEarth,
    SelectJupiter,
    SelectKuiper,
    CycleTarget,
    ToggleMinimizeQuickBar,
    ToggleMinorBodies,
    ToggleBelt(BeltZone),
    ExpandAllBelts,
    CollapseAllBelts,
    ToggleEmbryos,
    // Planet Builder Actions
    TogglePlanetBuilder,
    BuilderSelectPreset(BuilderPreset),
    BuilderMassStep(i8),
    BuilderDistanceStep(i8),
    BuilderCycleEccentricity,
    BuilderCycleComposition,
    BuilderToggleClickSpawn,
    BuilderExecuteSpawn,
    SpawnSubRocheMoon,
    // Telemetry & Climate Graphing Actions
    ToggleTelemetryPanel,
    SelectTelemetryMetric(crate::simulation::telemetry::TelemetryMetric),
    ExportTelemetryCsv,
    // Scientific Instruments & Overlays
    ToggleOrbitMode,
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
    LoadScenarioLittleRedDot,
    LoadScenarioPulsar,
    LoadScenarioMagnetar,
    // JWST Little Red Dot / Quasi-Star Experiments
    ToggleSuperEddington,
    TriggerBlowoutCocoon,
    SpawnInfallPop3Star,
    // Fullscreen and Panel Collapsibility
    ToggleFullScreenHud,
    ToggleTopLeftPanel,
    ToggleTopRightPanel,
    ToggleInspectorPanel,
    ToggleScenariosPanel,
    ToggleTopControlsPanel,
    ToggleBottomRightPanel,
    ToggleBottomCenterPanel,
    ToggleMinimizePlanetBuilder,
    ToggleMinimizeTelemetryPanel,
    // Interactive Orbital Slingshot Launcher
    ToggleSlingshotMode,
    CycleSlingshotArchetype,
    // System Save / Load & Scenario Serializer
    QuickSave,
    QuickLoad,
}

impl UiButtonAction {
    pub fn tooltip_description(&self) -> &'static str {
        match self {
            UiButtonAction::TimePause => "[Space]: Pause or resume continuous orbital physics flow.",
            UiButtonAction::TimeSpeed1 => "[1]: 1.0x Real-time orbital flow speed.",
            UiButtonAction::TimeSpeed100 => "[2]: 100x Accelerated time progression.",
            UiButtonAction::TimeSpeed10k => "[3]: 10,000x High-speed planetary accretion flow (~10 kyr/sec).",
            UiButtonAction::TimeSpeed1M => "[4]: 1,000,000x Deep astronomical time-warp (~1 Myr/sec).",
            UiButtonAction::SelectEntity(_) => "Focus and inspect this celestial body.",
            UiButtonAction::SelectStar => "Select the central star to inspect solar mass, temperature, and corona.",
            UiButtonAction::SelectMercury => "Select the innermost rocky terrestrial planet.",
            UiButtonAction::SelectEarth => "Select the habitable-zone terrestrial planet.",
            UiButtonAction::SelectJupiter => "Select the dominant outer gas giant planet.",
            UiButtonAction::SelectKuiper => "Select the outermost icy Kuiper belt planetesimal.",
            UiButtonAction::CycleTarget => "[Tab]: Cycle camera focus through all active celestial bodies.",
            UiButtonAction::ToggleMinimizeQuickBar => "[H]: Minimize or expand the top celestial body shortcut bar.",
            UiButtonAction::ToggleMinorBodies => "Toggle visibility of minor asteroids and planetesimals.",
            UiButtonAction::ToggleBelt(zone) => zone.title(),
            UiButtonAction::ExpandAllBelts => "Expand all astronomical belt groups in the quick selector.",
            UiButtonAction::CollapseAllBelts => "Contract all astronomical belt groups into compact summary counts.",
            UiButtonAction::ToggleEmbryos => "Toggle visibility of protoplanetary embryos.",
            UiButtonAction::TogglePlanetBuilder => "[P]: Open or close the interactive Planet Builder & Spawner panel.",
            UiButtonAction::BuilderSelectPreset(_) => "Load this physical planetary archetype preset into the Planet Builder.",
            UiButtonAction::BuilderMassStep(s) => if *s > 0 { "Scale up target world mass." } else { "Scale down target world mass." },
            UiButtonAction::BuilderDistanceStep(s) => if *s > 0 { "Increase target orbital semi-major axis (radius in AU)." } else { "Decrease target orbital semi-major axis (radius in AU)." },
            UiButtonAction::BuilderCycleEccentricity => "Cycle orbital eccentricity between Circular (0.00), Moderate (0.15), and High (0.60).",
            UiButtonAction::BuilderCycleComposition => "Cycle composition fractions (Rocky, Oceanic Ice, Metal Core, Gas Giant).",
            UiButtonAction::BuilderToggleClickSpawn => "Toggle 3D plane click-to-place mode (click disk to place world).",
            UiButtonAction::BuilderExecuteSpawn => "Spawn the configured celestial world into orbit immediately!",
            UiButtonAction::SpawnSubRocheMoon => "Spawn a volatile-rich moon directly inside the selected planet's fluid Roche limit to trigger tidal shredding and ring creation!",
            UiButtonAction::ToggleTelemetryPanel => "[F10]: Open or close the planetary climate & habitability telemetry graphing panel.",
            UiButtonAction::SelectTelemetryMetric(_) => "Switch active telemetry graph metric (Habitability, Temperature, Ocean, Pressure, Orbit).",
            UiButtonAction::ExportTelemetryCsv => "Export current planetary telemetry history to a standard CSV file in ./exports/.",
            UiButtonAction::ToggleOrbitMode => "[Y]: Cycle orbit trails (All -> Selected Target Only -> Hidden).",
            UiButtonAction::CycleOverlayMode => "[V]: Cycle diagnostic HUD overlays (Natural Color -> Spectral Temperature -> Hill Spheres & Gaps).",
            UiButtonAction::ToggleTractor => "[T]: Toggles Gravitational Tractor Beam to pull particles & planetesimals.",
            UiButtonAction::IncreaseMass => "[U]: Accrete +25% mass into selected planet.",
            UiButtonAction::DecreaseMass => "[J]: Strip -20% outer envelope mass from selected planet.",
            UiButtonAction::ExpandOrbit => "[O]: Boost orbital energy to expand orbital radius +10%.",
            UiButtonAction::ContractOrbit => "[L]: Brake orbital energy to contract orbital radius -10%.",
            UiButtonAction::CycleComposition => "[C]: Cycle composition between Rocky, Metallic, Icy, and Volatile.",
            UiButtonAction::BoostDeltaV => "[=]: Prograde orbital velocity acceleration boost.",
            UiButtonAction::BrakeDeltaV => "[-]: Retrograde orbital velocity braking burn.",
            UiButtonAction::InjectEmbryo => {
                "[M]: Trigger Theia-Earth Moon-forming giant impact / intercept trajectory."
            }
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
            UiButtonAction::LoadScenarioLittleRedDot => "[F6]: Load JWST Little Red Dot (100,000 M☉ Black Hole Star encased in 60 AU hydrogen cocoon).",
            UiButtonAction::LoadScenarioPulsar => "[F7]: Load PSR B1257+12 (Millisecond pulsar Lich with 3 confirmed zombie exoplanets).",
            UiButtonAction::LoadScenarioMagnetar => "[F9]: Load SGR 1806-20 (Ultra-magnetized 10¹⁵ G magnetar with starquake flares & companion).",
            UiButtonAction::ToggleSuperEddington => "[X]: Toggle Super-Eddington hyper-accretion onto the central black hole seed.",
            UiButtonAction::TriggerBlowoutCocoon => "[B]: Trigger radiation envelope blowout to unveil the naked Supermassive Quasar.",
            UiButtonAction::SpawnInfallPop3Star => "[T]: Spawn an infalling Population III hypergiant star to observe a Tidal Disruption Event (TDE).",
            UiButtonAction::ToggleFullScreenHud => "[F11]: Toggle clean full-screen view (hide/show all HUD overlays).",
            UiButtonAction::ToggleTopLeftPanel => "Minimize or expand top-left system statistics panel.",
            UiButtonAction::ToggleTopRightPanel => "Minimize or expand top-right diagnostics and time controls.",
            UiButtonAction::ToggleInspectorPanel => "Minimize or expand celestial body inspector & action toolbar.",
            UiButtonAction::ToggleScenariosPanel => "Minimize or expand sandbox scenario presets bar.",
            UiButtonAction::ToggleTopControlsPanel => "Minimize or expand top-right view & tool controls card.",
            UiButtonAction::ToggleBottomRightPanel => "Minimize or expand bottom-right navigation and keyboard shortcuts panel.",
            UiButtonAction::ToggleBottomCenterPanel => "Minimize or expand bottom-center simulation timer and speed controls.",
            UiButtonAction::ToggleMinimizePlanetBuilder => "Minimize or expand the interactive Planet Builder drawer.",
            UiButtonAction::ToggleMinimizeTelemetryPanel => "Minimize or expand the planetary telemetry & climate graph drawer.",
            UiButtonAction::ToggleSlingshotMode => "[K]: Toggle Interactive Orbital Slingshot Launcher (click-and-drag to aim & launch).",
            UiButtonAction::CycleSlingshotArchetype => "[C]: Cycle Slingshot body archetype (Asteroid -> Comet -> Terrestrial -> Water World -> Gas Giant -> Rogue).",
            UiButtonAction::QuickSave => "[F12]: Quick Save solar system state to disk (JSON).",
            UiButtonAction::QuickLoad => "[Shift+F12]: Quick Load saved solar system state from disk.",
        }
    }
}

/// Helper function to create standard glassmorphic button style
pub fn create_button(
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
                Pickable::IGNORE,
            ));
        });
}

/// Helper function to create compact glassmorphic button style for dense toolbars
pub fn create_compact_button(
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
                padding: UiRect::axes(Val::Px(5.0), Val::Px(2.5)),
                margin: UiRect::all(Val::Px(1.5)),
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
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                Pickable::IGNORE,
            ));
        });
}

/// Represents an identified major celestial world in the active simulation,
/// deterministically numbered outward from the central star:
/// - Index 0: Central Star (or barycenter host)
/// - Index 1: 1st innermost orbiting world
/// - Index 2: 2nd innermost orbiting world
/// - ...
/// - Index N: Outermost orbiting world
#[derive(Debug, Clone)]
pub struct SystemWorld {
    pub entity: Entity,
    pub name: String,
    pub body_type: BodyType,
    pub is_central_star: bool,
    pub distance_au: f64,
    pub mass_solar: f64,
    pub radius_au: f64,
    pub index: usize,
}

/// Returns true if a celestial body name corresponds to one of the canonical major planets
/// (Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune, Pluto, Planet Nine).
pub fn is_canonical_major_planet(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("mercury")
        || lower.contains("venus")
        || lower.contains("earth")
        || lower.contains("mars")
        || lower.contains("jupiter")
        || lower.contains("saturn")
        || lower.contains("uranus")
        || lower.contains("neptune")
        || lower.contains("pluto")
        || lower.contains("planet nine")
        || lower.contains("planet 9")
}

/// Determines if a celestial body is a protoplanetary embryo (e.g. Theia, Callisto Embryo, Titan Embryo,
/// or procedural protoplanets coalesced in planetary feeding zones to seed moons and planetary accretion).
pub fn is_embryo_body(name: &str, body_type: BodyType) -> bool {
    let lower = name.to_lowercase();
    if lower.contains("theia") {
        return false;
    }
    if is_canonical_major_planet(name) {
        return false;
    }
    if lower.contains("embryo") {
        return true;
    }
    body_type == BodyType::Protoplanet
}

/// Determines if a celestial body is a major world (Star, Canonical Planet, Mature Planet, Named Moon)
/// or a generic procedural minor planetesimal / asteroid / embryo.
pub fn is_major_body(name: &str, body_type: BodyType, is_star: bool, mass_solar: f64) -> bool {
    if is_star || body_type.is_star_or_remnant() {
        return true;
    }
    let lower = name.to_lowercase();
    if lower.contains("theia") {
        return true;
    }
    if is_canonical_major_planet(name) {
        return true;
    }
    if matches!(
        body_type,
        BodyType::Asteroid
            | BodyType::Comet
            | BodyType::Planetesimal
            | BodyType::DustGrain
            | BodyType::DebrisRing
    ) {
        return false;
    }
    if lower.starts_with("asteroid-")
        || lower.starts_with("dust-")
        || lower.starts_with("debris-")
        || lower.starts_with("comet-")
    {
        return false;
    }
    if is_embryo_body(name, body_type) {
        return false;
    }
    if body_type == BodyType::GasGiant
        || body_type == BodyType::IceGiant
        || body_type == BodyType::SuperEarth
        || body_type == BodyType::TerrestrialPlanet
        || body_type == BodyType::Moon
        || body_type == BodyType::QuasiStar
    {
        return true;
    }
    if lower.contains("trappist")
        || lower.contains("kepler")
        || lower.contains("nemesis")
        || lower.contains("rogue")
        || lower.contains("host star")
    {
        return true;
    }
    mass_solar >= (EARTH_MASS_SOLAR * 0.0001)
}

/// Collects and deterministically sorts all active celestial bodies into a continuous 0..N numerical sequence:
/// - [0] is ALWAYS the Central Star.
/// - [1..N] are all orbiting worlds (companions, black holes, super-earths, gas giants, embryos),
///   sorted strictly from innermost to outermost by distance from the star.
pub fn collect_sorted_system_worlds<'a, I>(items: I) -> Vec<SystemWorld>
where
    I: IntoIterator<
        Item = (
            Entity,
            &'a CelestialBody,
            &'a SimPosition,
            &'a Mass,
            &'a Radius,
            Option<&'a CentralStar>,
        ),
    >,
{
    let mut central_star: Option<SystemWorld> = None;
    let mut other_worlds: Vec<SystemWorld> = Vec::new();

    for (ent, body, pos, mass, radius, opt_star) in items {
        let dist = if pos.0.is_finite() {
            pos.0.length()
        } else {
            0.0
        };
        let is_star_component = opt_star.is_some();

        if is_star_component {
            central_star = Some(SystemWorld {
                entity: ent,
                name: body.name.clone(),
                body_type: body.body_type,
                is_central_star: true,
                distance_au: dist,
                mass_solar: mass.0,
                radius_au: radius.0,
                index: 0,
            });
            continue;
        }

        if is_major_body(
            &body.name,
            body.body_type,
            body.body_type.is_star_or_remnant(),
            mass.0,
        ) {
            other_worlds.push(SystemWorld {
                entity: ent,
                name: body.name.clone(),
                body_type: body.body_type,
                is_central_star: false,
                distance_au: dist,
                mass_solar: mass.0,
                radius_au: radius.0,
                index: 0,
            });
        }
    }

    if central_star.is_none() && !other_worlds.is_empty() {
        let mut best_idx = 0;
        let mut best_dist = f64::MAX;
        for (i, w) in other_worlds.iter().enumerate() {
            let score = if w.body_type.is_star_or_remnant() {
                w.distance_au
            } else {
                w.distance_au + 1000.0
            };
            if score < best_dist {
                best_dist = score;
                best_idx = i;
            }
        }
        let mut cs = other_worlds.remove(best_idx);
        cs.is_central_star = true;
        central_star = Some(cs);
    }

    other_worlds.sort_by(|a, b| {
        a.distance_au
            .partial_cmp(&b.distance_au)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result = Vec::with_capacity(1 + other_worlds.len());
    if let Some(mut cs) = central_star {
        cs.index = 0;
        result.push(cs);
    }
    for (i, mut w) in other_worlds.into_iter().enumerate() {
        w.index = i + 1;
        result.push(w);
    }

    result
}
