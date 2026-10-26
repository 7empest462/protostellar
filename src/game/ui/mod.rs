//! Heads-Up Display (HUD), Dynamic Telemetry Overlay, and Interactive On-Screen Control Buttons.

pub mod builder_panel;
pub mod builder_spawner;
pub mod builder_state;
pub mod buttons;
pub mod campaign_panel;
pub mod cutaway_hud;
pub mod epoch_scrubber;
pub mod hazards_panel;
pub mod hover_card;
pub mod hr_diagram_panel;
pub mod inspector_panel;
pub mod interactions;
pub mod navigation_panel;
pub mod presets_panel;
pub mod sci_fi_theme;
pub mod scroll;
pub mod selector;
pub mod setup;
pub mod tech_tree_panel;
pub mod telemetry_format;
pub mod telemetry_panel;
pub mod trophies_panel;
pub mod types;
pub mod update;
pub mod visibility;
pub mod world_list;

pub use builder_panel::spawn_planet_builder_panel;
pub use campaign_panel::{spawn_campaign_panels, update_campaign_ui};
pub use cutaway_hud::{spawn_cutaway_hud, update_cutaway_hud_ui};
pub use epoch_scrubber::{
    handle_epoch_scrubber_action, spawn_epoch_scrubber_panel, update_epoch_scrubber_ui,
};
pub use hazards_panel::{
    spawn_hazards_panel, update_hazards_ui, HazardsPanelElement, HazardsPanelState,
};
pub use hover_card::{spawn_hover_card, update_hover_card_ui};
pub use hr_diagram_panel::{
    handle_hr_diagram_action, spawn_hr_diagram_panel, update_hr_diagram_ui,
};
pub use inspector_panel::{format_composition_water_ice, spawn_bottom_left_inspector_panel};
pub use interactions::handle_ui_button_interactions;
pub use navigation_panel::spawn_bottom_right_controls_panel;
pub use presets_panel::{spawn_scenario_presets_panel, spawn_scenario_presets_pill};
pub use sci_fi_theme::{SciFiPanelMaterial, SciFiUiThemePlugin};
pub use scroll::{handle_inspector_scroll, reset_inspector_scroll_on_target_change};
pub use selector::{
    body_to_button_colors, body_to_button_label, body_to_button_label_and_colors,
    spawn_custom_builder_world, update_planet_builder_ui, update_quick_bar_highlights,
    update_quick_body_selector_bar,
};
pub use setup::setup_hud;
pub use telemetry_panel::{
    handle_telemetry_action, spawn_telemetry_panel, update_telemetry_graph_ui,
};
pub use types::{
    collect_sorted_system_worlds, create_button, create_compact_button,
    create_compact_selectable_button, is_canonical_major_planet, is_embryo_body, is_major_body,
    BeltZone, BuilderPreset, EpochQuickButton, EpochScrubberBodyTitleText,
    EpochScrubberMetricsText, EpochScrubberPanel, EpochScrubberStatusText, HudActionTooltipText,
    HudBodiesQuery, HudBottomTimerText, HudDynamicText, HudHeaderStatsText, HudInspectorText,
    HudPanelElement, HudTextQueries, HudTimeWarpText, HudToastContainer, HudToastText,
    HudVisibilityState, InspectorExoticHeader, InspectorSection, NotificationToast,
    PlanetBuilderInfoText, PlanetBuilderPanel, PlanetBuilderState, QuickBarButtonBaseColor,
    QuickBarState, QuickBodySelectorBar, ScrollableInspector, SystemWorld,
    TelemetryExportStatusText, TelemetryGraphBodyTitleText, TelemetryGraphPanel,
    TelemetryGraphReadoutText, TelemetryGraphSparklineText, TelemetryMetricButton,
    TelemetryPanelState, UiButtonAction,
};
pub use update::{handle_roche_disruption_toasts, update_hud};
pub use visibility::{update_hud_visibility, update_scenario_contextual_ui};
