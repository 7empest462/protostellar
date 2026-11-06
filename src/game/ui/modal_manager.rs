//! Centralized modal and drawer mutual exclusion manager.
//!
//! Ensures that opening any floating drawer or modal dialog automatically closes
//! other open floating drawers and modals so they do not overlap on screen.
//! Also provides inspection and closing of all open modals for the Escape key stack.

use crate::game::ui::builder_state::PlanetBuilderState;
use crate::game::ui::campaign_panel::CampaignPanelState;
use crate::game::ui::hazards_panel::HazardsPanelState;
use crate::game::ui::types::{
    HrDiagramPanelState, HudVisibilityState, TechTreePanelState, TelemetryPanelState,
    TrophiesPanelState, UiButtonAction,
};
use crate::simulation::geology::types::TimelineScrubber;

/// Identifier for floating modals and side drawers subject to mutual exclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveModalKind {
    None,
    PlanetBuilder,
    Telemetry,
    Trophies,
    TechTree,
    Campaign,
    Hazards,
    EpochScrubber,
    HrDiagram,
    ScenarioPresets,
}

/// Mutable reference bundle of all floating drawer and modal states.
#[derive(Default)]
pub struct ModalPanelStates<'a> {
    pub builder: Option<&'a mut PlanetBuilderState>,
    pub telemetry: Option<&'a mut TelemetryPanelState>,
    pub trophies: Option<&'a mut TrophiesPanelState>,
    pub tech_tree: Option<&'a mut TechTreePanelState>,
    pub campaign: Option<&'a mut CampaignPanelState>,
    pub hazards: Option<&'a mut HazardsPanelState>,
    pub scrubber: Option<&'a mut TimelineScrubber>,
    pub hr_diagram: Option<&'a mut HrDiagramPanelState>,
    pub visibility: Option<&'a mut HudVisibilityState>,
}

impl ModalPanelStates<'_> {
    /// Closes all conflicting modals and drawers except `keep_open`.
    /// When `keep_open` is `ActiveModalKind::None`, closes all open modals.
    /// Returns `true` if at least one modal was open and has been closed.
    pub fn close_conflicting_modals(&mut self, keep_open: ActiveModalKind) -> bool {
        let mut closed_any = false;

        if keep_open != ActiveModalKind::PlanetBuilder {
            if let Some(ref mut b) = self.builder {
                if b.is_open {
                    b.is_open = false;
                    b.click_to_spawn_mode = false;
                    closed_any = true;
                }
            }
        }
        if keep_open != ActiveModalKind::Telemetry {
            if let Some(ref mut t) = self.telemetry {
                if t.is_open {
                    t.is_open = false;
                    closed_any = true;
                }
            }
        }
        if keep_open != ActiveModalKind::Trophies {
            if let Some(ref mut tr) = self.trophies {
                if tr.is_open {
                    tr.is_open = false;
                    closed_any = true;
                }
            }
        }
        if keep_open != ActiveModalKind::TechTree {
            if let Some(ref mut tt) = self.tech_tree {
                if tt.is_open {
                    tt.is_open = false;
                    closed_any = true;
                }
            }
        }
        if keep_open != ActiveModalKind::Campaign {
            if let Some(ref mut c) = self.campaign {
                if c.is_open {
                    c.is_open = false;
                    closed_any = true;
                }
            }
        }
        if keep_open != ActiveModalKind::Hazards {
            if let Some(ref mut h) = self.hazards {
                if h.is_open {
                    h.is_open = false;
                    closed_any = true;
                }
            }
        }
        if keep_open != ActiveModalKind::EpochScrubber {
            if let Some(ref mut s) = self.scrubber {
                if s.is_open {
                    s.is_open = false;
                    closed_any = true;
                }
            }
        }
        if keep_open != ActiveModalKind::HrDiagram {
            if let Some(ref mut hr) = self.hr_diagram {
                if hr.is_open {
                    hr.is_open = false;
                    closed_any = true;
                }
            }
        }
        if keep_open != ActiveModalKind::ScenarioPresets {
            if let Some(ref mut v) = self.visibility {
                if v.scenarios_is_open {
                    v.scenarios_is_open = false;
                    v.scenarios_minimized = true;
                    closed_any = true;
                }
            }
        }

        closed_any
    }

    /// Returns `true` if any floating modal or drawer is currently open.
    pub fn has_any_open_modal(&self) -> bool {
        self.builder.as_ref().is_some_and(|b| b.is_open)
            || self.telemetry.as_ref().is_some_and(|t| t.is_open)
            || self.trophies.as_ref().is_some_and(|tr| tr.is_open)
            || self.tech_tree.as_ref().is_some_and(|tt| tt.is_open)
            || self.campaign.as_ref().is_some_and(|c| c.is_open)
            || self.hazards.as_ref().is_some_and(|h| h.is_open)
            || self.scrubber.as_ref().is_some_and(|s| s.is_open)
            || self.hr_diagram.as_ref().is_some_and(|hr| hr.is_open)
            || self
                .visibility
                .as_ref()
                .is_some_and(|v| v.scenarios_is_open)
    }

    /// If the given button action opened a modal or drawer, automatically closes all conflicting ones.
    pub fn enforce_mutual_exclusion_for_action(&mut self, action: &UiButtonAction) {
        let opened_modal = match action {
            UiButtonAction::TogglePlanetBuilder
                if self.builder.as_ref().is_some_and(|b| b.is_open) =>
            {
                Some(ActiveModalKind::PlanetBuilder)
            }
            UiButtonAction::ToggleTelemetryPanel
                if self.telemetry.as_ref().is_some_and(|t| t.is_open) =>
            {
                Some(ActiveModalKind::Telemetry)
            }
            UiButtonAction::ToggleTrophiesPanel
                if self.trophies.as_ref().is_some_and(|tr| tr.is_open) =>
            {
                Some(ActiveModalKind::Trophies)
            }
            UiButtonAction::ToggleTechTreePanel
                if self.tech_tree.as_ref().is_some_and(|tt| tt.is_open) =>
            {
                Some(ActiveModalKind::TechTree)
            }
            UiButtonAction::ToggleCampaignPanel
                if self.campaign.as_ref().is_some_and(|c| c.is_open) =>
            {
                Some(ActiveModalKind::Campaign)
            }
            UiButtonAction::ToggleHazardsPanel
                if self.hazards.as_ref().is_some_and(|h| h.is_open) =>
            {
                Some(ActiveModalKind::Hazards)
            }
            UiButtonAction::ToggleEpochScrubberPanel
                if self.scrubber.as_ref().is_some_and(|s| s.is_open) =>
            {
                Some(ActiveModalKind::EpochScrubber)
            }
            UiButtonAction::ToggleHrDiagram
                if self.hr_diagram.as_ref().is_some_and(|hr| hr.is_open) =>
            {
                Some(ActiveModalKind::HrDiagram)
            }
            UiButtonAction::ToggleScenariosPanel
                if self
                    .visibility
                    .as_ref()
                    .is_some_and(|v| v.scenarios_is_open) =>
            {
                Some(ActiveModalKind::ScenarioPresets)
            }
            _ => None,
        };

        if let Some(keep_open) = opened_modal {
            self.close_conflicting_modals(keep_open);
        }
    }
}

/// Centralized helper function to close all conflicting modals except `keep_open`.
pub fn close_conflicting_modals(keep_open: ActiveModalKind, states: &mut ModalPanelStates) -> bool {
    states.close_conflicting_modals(keep_open)
}
