//! Types, components, resources, and classification helpers for the HUD and UI system.

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
pub use crate::simulation::disk::belts::BeltZone;
use crate::simulation::relativity::RelativisticState;
use crate::simulation::tides::TidalState;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::collections::BTreeSet;

/// Marker component for interactive HUD buttons, replacing classic Bevy UI `Interaction`
/// with Bevy 0.20.1 `Button`, `Hovered`, and `FocusPolicy::Block`.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component, Default, Debug, PartialEq, Clone)]
#[require(
    Node,
    bevy::ui::FocusPolicy::Block,
    bevy::ui_widgets::Button,
    bevy::ui_widgets::ActivateOnPress,
    bevy::picking::hover::Hovered
)]
pub struct HudButton;

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
        Option<&'static mut VolatileInventory>,
        Option<&'static mut PlanetaryClimate>,
    ),
    Without<PanOrbitCamera>,
>;

pub type HudBodiesQuery<'w, 's> = Query<
    'w,
    's,
    (
        (
            &'static SimPosition,
            &'static SimVelocity,
            &'static Mass,
            &'static Radius,
            &'static Temperature,
            &'static Composition,
            &'static CelestialBody,
        ),
        (
            Option<&'static InternalDifferentiation>,
            Option<&'static SpinState>,
            Option<&'static IgnitionState>,
            Option<&'static VolatileInventory>,
            Option<&'static PlanetaryRingSystem>,
            Option<&'static AtmosphericEscapeTail>,
            Option<&'static PlanetaryClimate>,
            Option<&'static BiosphereState>,
            Option<&'static StellarEvolutionState>,
            Option<&'static TidalState>,
            Option<&'static RelativisticState>,
            Option<&'static crate::simulation::atmosphere_escape::AtmosphericEscapeState>,
            Option<&'static crate::simulation::kozai_lidov::KozaiLidovState>,
            Option<&'static SatelliteOf>,
        ),
    ),
>;

/// Marker for the live HUD simulation time and year counter.
#[derive(Component)]
pub struct HudTimeWarpText;

/// Marker for the selected body telemetry details in the bottom-left panel.
#[derive(Component)]
pub struct HudInspectorText;

/// Marker for the trajectory encounter & impact forecast readout in the bottom-left panel.
#[derive(Component)]
pub struct HudEncounterForecastText;

/// Marker for the top-left simulation phase & statistics readout.
#[derive(Component)]
pub struct HudHeaderStatsText;

/// Marker for the dynamic action button tooltip explanation text.
#[derive(Component)]
pub struct HudActionTooltipText;

/// Marker for temporary floating status toasts.
#[derive(Component)]
pub struct HudToastText;

/// Marker for the persistent bottom-center simulation clock (`true` = compact pill, `false` = expanded dock).
#[derive(Component, Default, Clone, Copy)]
pub struct HudBottomTimerText(pub bool);

/// Disjoint query bundle for all live on-screen text elements in the HUD.
#[derive(SystemParam)]
#[allow(
    clippy::type_complexity,
    reason = "Disjoint HUD text component queries"
)]
pub struct HudTextQueries<'w, 's> {
    pub header: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
            Without<HudEncounterForecastText>,
        ),
    >,
    pub time: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<HudTimeWarpText>,
            Without<HudHeaderStatsText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
            Without<HudEncounterForecastText>,
        ),
    >,
    pub inspector: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<HudInspectorText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
            Without<HudEncounterForecastText>,
        ),
    >,
    pub toast: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<HudToastText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudBottomTimerText>,
            Without<HudEncounterForecastText>,
        ),
    >,
    pub bottom_timer: Query<
        'w,
        's,
        (&'static mut Text, &'static HudBottomTimerText),
        (
            With<HudBottomTimerText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
            Without<HudEncounterForecastText>,
        ),
    >,
    pub forecast: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<HudEncounterForecastText>,
            Without<HudHeaderStatsText>,
            Without<HudTimeWarpText>,
            Without<HudInspectorText>,
            Without<HudToastText>,
            Without<HudBottomTimerText>,
        ),
    >,
}

/// Marker for the top-center body selector bar container.
#[derive(Component)]
pub struct QuickBodySelectorBar;

/// State configuration for the top celestial body shortcut bar.
#[derive(Resource, Debug, Clone, Default)]
pub struct QuickBarState {
    pub is_minimized: bool,
    pub show_minor_bodies: bool,
    pub show_embryos: bool,
    pub sort_by_mass: bool,
    /// Active astronomical belts expanded to reveal individual member buttons.
    pub expanded_belts: BTreeSet<BeltZone>,
    /// Active case-insensitive text search query filtering displayed celestial bodies.
    pub filter_text: String,
}

/// Active tab in the bottom-left Target Inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Overview,
    GeodynamicsBio,
    RelativityFields,
}

/// Global visibility and panel collapse states for the HUD overlay.
#[derive(Resource, Debug, Clone)]
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
    /// Whether the scenario presets modal drawer is currently open.
    pub scenarios_is_open: bool,
    /// Whether the top-right view and tool controls card is collapsed.
    pub top_controls_minimized: bool,
    /// Whether the bottom-right navigation and shortcuts panel is collapsed.
    pub bottom_right_minimized: bool,
    /// Whether the bottom-center simulation timer and speed dock is collapsed.
    pub bottom_center_minimized: bool,
    /// Whether the floating telemetry & climate graphing panel is collapsed.
    pub telemetry_minimized: bool,
    /// Currently active tab in the bottom-left Target Inspector.
    pub active_inspector_tab: InspectorTab,
}

impl Default for HudVisibilityState {
    fn default() -> Self {
        Self {
            is_full_screen_clean: false,
            top_left_minimized: true,
            top_right_minimized: true,
            inspector_minimized: true,
            scenarios_minimized: true,
            scenarios_is_open: false,
            top_controls_minimized: true,
            bottom_right_minimized: true,
            bottom_center_minimized: true,
            telemetry_minimized: true,
            active_inspector_tab: InspectorTab::Overview,
        }
    }
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
    HrDiagramPanelPill,
    OrbitModeBadge,
    OverlayModeBadge,
    TrajectoryPredictorBadge,
    LagrangePointsBadge,
    HabitableZoneBadge,
    AtmospheresBadge,
    CloudsBadge,
    AudioMuteBadge,
    ParticleDensityBadge,
    QuickBarContainer,
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
    ParticleDensityBadge,
    TrajectoryPredictorBadge,
    LagrangePointsBadge,
    HabitableZoneBadge,
    AtmospheresBadge,
    CloudsBadge,
    AudioMuteBadge,
}

/// Discriminant component for action categories within the Target Inspector toolbar.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorSection {
    OrbitTracking,
    MassComposition,
    HydrosphereBathymetry,
    TerraformingBombardment,
    Astrophysics,
    ExoticExperiments,
}

/// Marker component for the dynamic header text in the Exotic Experiments section.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InspectorExoticHeader;

/// Marker for the bottom-center live telemetry and toast container.
#[derive(Component)]
pub struct HudToastContainer;

pub use super::builder_state::{BuilderPreset, PlanetBuilderState};

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

/// State configuration for the Hertzsprung–Russell Stellar Evolution Panel (`[Shift-R]`).
#[derive(Resource, Debug, Clone, Default)]
pub struct HrDiagramPanelState {
    pub is_open: bool,
    pub is_minimized: bool,
}

/// Marker component for the floating Hertzsprung–Russell Stellar Evolution Panel.
#[derive(Component)]
pub struct HrDiagramPanel;

/// Marker for the live 2D H-R diagram plot text display.
#[derive(Component)]
pub struct HrDiagramPlotText;

/// Marker for the detailed stellar astrophysics text display.
#[derive(Component)]
pub struct HrDiagramDetailsText;

/// Marker component for the deep-time geological epoch scrubber panel.
#[derive(Component)]
pub struct EpochScrubberPanel;

/// Marker for the tracked body title in the Epoch Scrubber HUD.
#[derive(Component)]
pub struct EpochScrubberBodyTitleText;

/// Marker for the epoch and age status readout text.
#[derive(Component)]
pub struct EpochScrubberStatusText;

/// Marker for the geological telemetry metrics text.
#[derive(Component)]
pub struct EpochScrubberMetricsText;

/// Marker for the quick epoch button pill.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochQuickButton(pub crate::simulation::geology::types::GeologicalEpoch);

/// Marker component for grouping epoch buttons by planetary archetype (Earth, Mars, Venus).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochPlanetButtonGroup(pub crate::simulation::geology::types::EpochTargetPlanet);

/// Marker for planetary timeline switcher tab buttons.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochPlanetTabButton(pub crate::simulation::geology::types::EpochTargetPlanet);

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
    TimeSpeedRealtime,
    TimeSpeed1Min,
    TimeSpeed5Min,
    TimeSpeed6Hr,
    TimeSpeed12Hr,
    TimeSpeed24Hr,
    TimeSpeed72Hr,
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
    ToggleListSortMode,
    ToggleMinorBodies,
    ToggleBelt(BeltZone),
    ExpandAllBelts,
    CollapseAllBelts,
    ToggleEmbryos,
    SpawnOortCloudSwarm,
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
    // Deep-Time Geological Epoch Scrubber Actions
    ToggleEpochScrubberPanel,
    SelectEpochTargetPlanet(crate::simulation::geology::types::EpochTargetPlanet),
    ScrubToEpoch(crate::simulation::geology::types::GeologicalEpoch),
    ScrubTimeStep(i32),
    ToggleScrubAutoAdvance,
    // Scientific Instruments & Overlays
    ToggleOrbitMode,
    CycleOverlayMode,
    CycleParticleDensity,
    ToggleTractor,
    ToggleTrajectoryPredictor,
    ToggleLagrangePoints,
    ToggleHabitableZone,
    ToggleAtmospheres,
    ToggleClouds,
    ToggleHrDiagram,
    ToggleMinimizeHrDiagram,
    ToggleTrophiesPanel,
    ToggleTechTreePanel,
    ToggleCampaignPanel,
    ToggleCampaignTracker,
    StartCampaignMission(crate::simulation::campaign::CampaignMissionId),
    ClaimCampaignReward,
    AbandonCampaignMission,
    ResearchTech(crate::simulation::economy::TechnologyId),
    // Dynamic Cosmic Hazards & Sensory Polish
    ToggleHazardsPanel,
    TriggerCosmicHazard(crate::simulation::events::CosmicEventType),
    ToggleAutoHazards,
    CycleTonemappingMode,
    ToggleAudioMute,
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
    ToggleSurfaceHorizonCamera,
    ToggleInternalCutaway,
    ResetView,
    DeselectBody,
    FixOrbit,
    IgniteStar,
    TriggerCoronalMassEjection,
    TriggerLhb,
    ShatterIntoRings,
    SeedLife,
    AgeStar,
    // Targeted Terraforming & Bombardment Suite
    BombardComet,
    BombardChondrite,
    BombardSalvo,
    BombardCore,
    AdvanceCivilization,
    LaunchProbe,
    // Hydrosphere & Bathymetric Sea Level Controls
    AddOceanWater,
    DrainOceanWater,
    DryBasin,
    EarthSeaLevel,
    // Gravitational Tidal Actions
    TidalLock,
    // Relativistic Gravitational Waves & Inspiral
    AccelerateInspiral,
    // Atmospheric Photoevaporation & Solar Wind Stripping
    StripAtmosphere,
    // Sandbox Scenarios & System Presets
    LoadScenarioSolar,
    LoadScenarioGenesis,
    LoadScenarioTrappist,
    LoadScenarioKepler16,
    LoadScenarioHotJupiter,
    LoadScenarioRoguePlanet,
    LoadScenarioLittleRedDot,
    LoadScenarioPulsar,
    LoadScenarioMagnetar,
    LoadScenarioRelativisticBinary,
    LoadScenarioKozaiTriple,
    LoadScenarioSagittariusAStar,
    LoadScenarioMolecularCloud,
    LoadScenarioGalacticCollision,
    // JWST Little Red Dot / Quasi-Star Experiments
    ToggleSuperEddington,
    TriggerBlowoutCocoon,
    SpawnInfallPop3Star,
    TriggerGmcSupernovaShock,
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
    SaveNamedSlot,
    LoadNamedSlot,
    CopySystemSeedClipboard,
    PasteSystemSeedClipboard,
    ToggleSavesPanel,
    RefreshSaveSlots,
    SaveNewScenarioSlot,
    ToggleSavesScenarioFilter,
    LoadSaveSlot(usize),
    OverwriteSaveSlot(usize),
    SelectSaveSlot(usize),
    DeleteSaveSlot(usize),
    ClearQuickBarFilter,
    // Inspector Tab Switching
    SelectInspectorTab(InspectorTab),
}

pub use super::buttons::*;

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

pub use super::world_list::*;

#[derive(Resource, Default)]
pub struct TrophiesPanelState {
    pub is_open: bool,
}

#[derive(Component)]
pub enum TrophiesPanelElement {
    Container,
    Content,
}

#[derive(Resource, Default)]
pub struct TechTreePanelState {
    pub is_open: bool,
}

#[derive(Component)]
pub enum TechTreePanelElement {
    Container,
    Content,
    Summary,
}
pub use super::campaign_panel::{CampaignPanelElement, CampaignPanelState};

/// A single astrophysical event recorded for UI history and timeline inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct CosmicEventEntry {
    pub timestamp_yr: f64,
    pub icon: &'static str,
    pub headline: String,
    pub target_entity: Option<Entity>,
}

/// Bounded ring-buffer resource tracking the 32 most recent major astrophysical events.
#[derive(Resource, Debug, Clone, Default)]
pub struct CosmicEventLog {
    pub entries: std::collections::VecDeque<CosmicEventEntry>,
}

impl CosmicEventLog {
    pub const MAX_ENTRIES: usize = 32;

    pub fn push(
        &mut self,
        timestamp_yr: f64,
        icon: &'static str,
        headline: String,
        target_entity: Option<Entity>,
    ) {
        if self.entries.len() >= Self::MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(CosmicEventEntry {
            timestamp_yr,
            icon,
            headline,
            target_entity,
        });
    }
}
