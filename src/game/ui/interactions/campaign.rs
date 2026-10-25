//! Campaign mode and scenario challenge interaction handlers.

use bevy::prelude::*;

use crate::game::ui::types::{NotificationToast, UiButtonAction};

/// Handles campaign panel toggling, mission launches, reward claims, and aborts.
pub fn handle_campaign_action(
    action: &UiButtonAction,
    opt_campaign_panel: Option<&mut crate::game::ui::types::CampaignPanelState>,
    opt_campaign_state: Option<&mut crate::simulation::campaign::CampaignState>,
    opt_start_events: Option<
        &mut MessageWriter<crate::simulation::campaign::StartCampaignMissionEvent>,
    >,
    opt_science: Option<&mut crate::simulation::economy::ScienceData>,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::ToggleCampaignPanel => {
            if let Some(panel) = opt_campaign_panel {
                panel.is_open = !panel.is_open;
                toast.message = if panel.is_open {
                    "🎯 Campaign & Scenario Challenges Opened".to_string()
                } else {
                    "🎯 Campaign Panel Closed".to_string()
                };
                toast.timer = 2.5;
            }
            true
        }
        UiButtonAction::ToggleCampaignTracker => {
            if let Some(panel) = opt_campaign_panel {
                panel.is_tracker_minimized = !panel.is_tracker_minimized;
            }
            true
        }
        UiButtonAction::StartCampaignMission(mission_id) => {
            if let Some(events) = opt_start_events {
                events.write(crate::simulation::campaign::StartCampaignMissionEvent(
                    *mission_id,
                ));
            }
            if let Some(panel) = opt_campaign_panel {
                panel.is_open = false;
            }
            toast.message = format!("🚀 Initializing Mission: {}...", mission_id.title());
            toast.timer = 4.0;
            true
        }
        UiButtonAction::ClaimCampaignReward => {
            if let (Some(camp_state), Some(science)) = (opt_campaign_state, opt_science) {
                if camp_state.status == crate::simulation::campaign::MissionStatus::Victory
                    && !camp_state.reward_claimed
                {
                    if let Some(mission) = camp_state.active_mission {
                        let bounty = f64::from(mission.science_reward());
                        science.add_science(bounty);
                        camp_state.reward_claimed = true;
                        toast.message = format!(
                            "🏆 Claimed +{} Science Data bounty!",
                            mission.science_reward()
                        );
                        toast.timer = 4.0;
                    }
                }
            }
            true
        }
        UiButtonAction::AbandonCampaignMission => {
            if let Some(camp_state) = opt_campaign_state {
                camp_state.active_mission = None;
                camp_state.status = crate::simulation::campaign::MissionStatus::InProgress;
                camp_state.objectives.clear();
                toast.message = "🛑 Campaign mission aborted. Returned to sandbox.".to_string();
                toast.timer = 3.0;
            }
            true
        }
        _ => false,
    }
}
