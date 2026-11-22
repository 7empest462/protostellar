//! Player interaction tools: gravitational impulse, tractor, mass injection, and live spawning.

pub mod body_tools;
pub mod hotkeys;
pub mod selection;

use bevy::math::DVec3;
use bevy::prelude::*;

pub use body_tools::*;
pub use hotkeys::*;
pub use selection::*;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::{CentralStar, Mass, SimPosition, SimVelocity};
use crate::simulation::resources::*;

/// Event writers and focus detection for player tools.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlayerToolsEvents<'w> {
    pub scenario_events: MessageWriter<'w, crate::simulation::scenarios::LoadScenarioEvent>,
    pub save_events: Option<MessageWriter<'w, crate::simulation::serialization::SaveSystemEvent>>,
    pub load_events: Option<MessageWriter<'w, crate::simulation::serialization::LoadSystemEvent>>,
    pub glitch_events: Option<
        MessageWriter<'w, crate::simulation::neutron_stars::TriggerManualGlitchOrStarquakeEvent>,
    >,
    pub oort_events: Option<MessageWriter<'w, crate::simulation::oort::TriggerOortCloudSwarmEvent>>,
    pub supernova_events: Option<MessageWriter<'w, crate::simulation::components::SupernovaEvent>>,
    pub focus: Option<Res<'w, bevy::input_focus::InputFocus>>,
}

/// Handles player tool activation and direct live editing of celestial bodies.
#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
pub fn handle_player_tools(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    disk_params: Res<DiskParameters>,
    (mut config, mut player_state): (ResMut<SimulationConfig>, ResMut<PlayerInteractionState>),
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    mut theia_state: Option<ResMut<crate::simulation::accretion::TheiaImpactState>>,
    mut events: PlayerToolsEvents,
    mut builder_state: ResMut<crate::game::ui::PlanetBuilderState>,
    mut telemetry_state: ResMut<crate::game::ui::TelemetryPanelState>,
    (
        mut opt_scrubber,
        mut opt_tech_tree,
        mut opt_campaign_panel,
        mut opt_hazards_panel,
        mut opt_hazard_mgr,
        mut opt_hr_state,
        mut opt_lagrange,
        mut opt_hz,
        mut opt_trophies_panel,
        mut opt_hud_visibility,
        mut opt_saves_panel,
    ): (
        Option<ResMut<crate::simulation::geology::types::TimelineScrubber>>,
        Option<ResMut<crate::game::ui::types::TechTreePanelState>>,
        Option<ResMut<crate::game::ui::types::CampaignPanelState>>,
        Option<ResMut<crate::game::ui::hazards_panel::HazardsPanelState>>,
        Option<ResMut<crate::simulation::events::CosmicEventManager>>,
        Option<ResMut<crate::game::ui::types::HrDiagramPanelState>>,
        Option<ResMut<crate::simulation::resources::LagrangeOverlayConfig>>,
        Option<ResMut<crate::simulation::resources::HabitableZoneConfig>>,
        Option<ResMut<crate::game::ui::types::TrophiesPanelState>>,
        Option<ResMut<crate::game::ui::types::HudVisibilityState>>,
        Option<ResMut<crate::game::ui::SavesPanelState>>,
    ),
    mut toast: ResMut<crate::game::ui::NotificationToast>,
    mut camera_query: Query<(&Transform, &mut PanOrbitCamera)>,
    mut opt_predictor: Option<ResMut<crate::simulation::predictor::TrajectoryPredictorState>>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    mut selected_query: SelectedBodyQuery,
    (editable_query, opt_origin): (
        Query<(), With<bevy::text::EditableText>>,
        Option<Res<crate::rendering::origin::RenderOrigin>>,
    ),
) {
    if keyboard.get_just_pressed().len() == 0 {
        return;
    }
    if crate::game::ui::widgets::is_text_input_focused(events.focus.as_deref(), &editable_query) {
        return;
    }

    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    let prev_tractor = player_state.tractor_position;

    let star_mass = disk_params.central_star_mass;
    let (star_pos, star_vel) = selected_query
        .iter()
        .find(|(_, _, _, _, _, _, _, body, _, _, _, _, opt_star, _, _)| {
            opt_star.is_some() || body.body_type.is_star_or_remnant()
        })
        .map_or((DVec3::ZERO, DVec3::ZERO), |(_, _, _, _, p, v, ..)| {
            (p.0, v.0)
        });
    let is_star_selected = player_state.selected_entity.is_some_and(|e| {
        selected_query
            .get(e)
            .is_ok_and(|(_, _, _, _, _, _, _, b, _, _, _, _, s, _, _)| {
                s.is_some() || b.body_type.is_star_or_remnant()
            })
    });

    handle_viewport_and_tool_hotkeys(
        &keyboard,
        &mut config,
        &mut player_state,
        &mut builder_state,
        &mut telemetry_state,
        opt_scrubber.as_deref_mut(),
        opt_tech_tree.as_deref_mut(),
        opt_campaign_panel.as_deref_mut(),
        opt_hazards_panel.as_deref_mut(),
        opt_hazard_mgr.as_deref_mut(),
        opt_hr_state.as_deref_mut(),
        opt_lagrange.as_deref_mut(),
        opt_hz.as_deref_mut(),
        &mut toast,
        &mut camera_query,
        opt_predictor.as_deref_mut(),
        is_star_selected,
        opt_trophies_panel.as_deref_mut(),
        opt_hud_visibility.as_deref_mut(),
        opt_saves_panel.as_deref_mut(),
    );
    if prev_tractor.is_none()
        && let Some(tp) = player_state.tractor_position
    {
        player_state.tractor_position = Some(origin.to_sim(tp.as_vec3()));
    }

    handle_tab_selection(
        &keyboard,
        &config,
        &mut player_state,
        &mut toast,
        &mut camera_query,
        &selected_query,
    );

    handle_unselected_star_ignition_hotkey(
        &mut commands,
        &keyboard,
        &mut selected_query,
        is_star_selected,
        &mut toast,
    );

    handle_selected_body_tools_with_events(
        &mut commands,
        &keyboard,
        star_mass,
        star_pos,
        star_vel,
        &mut player_state,
        &mut selected_query,
        &mut toast,
        events.supernova_events.as_mut(),
    );

    handle_theia_hotkey(&keyboard, theia_state.as_deref_mut(), &mut toast);

    handle_scenario_and_system_hotkeys(
        &keyboard,
        &mut config,
        &mut lhb_state,
        &mut events.scenario_events,
        events.save_events.as_mut(),
        events.load_events.as_mut(),
        events.glitch_events.as_mut(),
        events.oort_events.as_mut(),
        &mut toast,
        scenario_state.as_deref(),
    );
}

/// System that handles direct 3D plane click-to-place spawning when builder click mode is active.
pub fn handle_planet_builder_click_spawn(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<PanOrbitCamera>>,
    mut pan_orbit_query: Query<&mut PanOrbitCamera>,
    mut builder_state: ResMut<crate::game::ui::PlanetBuilderState>,
    disk_params: Res<DiskParameters>,
    star_query: Query<(&SimPosition, &SimVelocity, &Mass), With<CentralStar>>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut toast: ResMut<crate::game::ui::NotificationToast>,
    ui_interaction_query: crate::rendering::camera_picking::UiInteractionStateQuery,
    ui_nodes_query: crate::rendering::camera_picking::UiPickingNodesQuery,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    if !builder_state.is_open || !builder_state.click_to_spawn_mode {
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    if crate::rendering::camera_picking::is_cursor_over_active_ui(
        window,
        cursor_pos,
        &ui_interaction_query,
        &ui_nodes_query,
        None,
    ) {
        return;
    }

    if mouse_buttons.just_pressed(MouseButton::Left) {
        let Ok((camera, camera_transform)) = camera_query.single() else {
            return;
        };

        let origin = opt_origin.as_deref().copied().unwrap_or_default();
        if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
            let (star_pos, star_vel, eff_star_mass) = star_query.iter().next().map_or(
                (DVec3::ZERO, DVec3::ZERO, disk_params.central_star_mass),
                |(p, v, m)| (p.0, v.0, m.0),
            );
            // Intersect ray with the star's ecliptic plane in render space
            let plane_y = (star_pos.y - origin.0.y) as f32;
            if ray.direction.y.abs() > 1e-5 {
                let t = (plane_y - ray.origin.y) / ray.direction.y;
                if t > 0.0 {
                    let hit = ray.origin + *ray.direction * t;
                    let hit_sim = origin.to_sim(hit);
                    let spawn_coords = DVec3::new(hit_sim.x, star_pos.y, hit_sim.z);
                    crate::game::ui::spawn_custom_builder_world_relative(
                        &mut commands,
                        &builder_state,
                        eff_star_mass,
                        star_pos,
                        star_vel,
                        Some(spawn_coords),
                        &mut player_state,
                        &mut pan_orbit_query,
                        &mut toast,
                    );
                    // Exit click-to-spawn mode after placement
                    builder_state.click_to_spawn_mode = false;
                }
            }
        }
    }
}
