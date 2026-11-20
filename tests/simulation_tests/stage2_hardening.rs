//! Stage 2 Hardening Regression Tests: disk cavity & retrograde torque, spin frame clamping,
//! mutual Roche & two-body trajectory propagation, UI zero-churn guards, and pulsar/quasar sync.

use bevy::ecs::system::SystemParam;
use bevy::math::{DVec3, Rot2};
use bevy::prelude::*;
use bevy::ui::UiTransform;
use std::f64::consts::TAU;

use protostellar::game::ui::cutaway_hud::{CutawayCalloutPill, update_cutaway_hud_ui};
use protostellar::game::ui::interactions::editor::orbital_params::handle_composition_cycle;
use protostellar::game::ui::types::{NotificationToast, SelectedWorldQuery};
use protostellar::game::ui::update::{set_display_if_changed, set_text_if_changed};
use protostellar::game::ui::widget_sync::{HoverReticleBracket, animate_hud_ui_transforms};
use protostellar::rendering::bodies::VisualAssets;
use protostellar::rendering::bodies::remnant_beams::{
    PulsarBeamRoot, QuasarBeamRoot, StructureHostTarget, sync_pulsar_beams, sync_quasar_beams,
};
use protostellar::rendering::camera::PanOrbitCamera;
use protostellar::simulation::components::*;
use protostellar::simulation::disk_migration::apply_type_i_torque_acc;
use protostellar::simulation::predictor::{
    EncounterType, TargetCandidate, find_closest_encounter, find_closest_encounter_with_mass,
};
use protostellar::simulation::probes::navigation::compute_planet_spin_frame;
use protostellar::simulation::resources::*;
use protostellar::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU, G_ASTRO};
use protostellar::utils::math::state_vectors_to_orbital_elements;

#[test]
fn test_stage2_disk_migration_cavity_and_retrograde_drag() {
    let mut disk = DiskParameters::default();
    let v_circ = (G_ASTRO * 1.0_f64).sqrt();

    // Inside default 0.05 AU cavity
    let mut acc_cavity_small = DVec3::ZERO;
    apply_type_i_torque_acc(
        DVec3::new(0.02, 0.0, 0.0),
        DVec3::new(0.0, 0.0, v_circ),
        EARTH_MASS_SOLAR,
        BodyType::TerrestrialPlanet,
        false,
        false,
        1.0,
        1.0,
        &disk,
        &mut acc_cavity_small,
    );
    assert_eq!(acc_cavity_small, DVec3::ZERO);

    // Inside wider magnetospheric cavity (r < r_in = 0.20 AU)
    disk.inner_radius_au = 0.20;
    let mut acc_cavity_wide = DVec3::ZERO;
    apply_type_i_torque_acc(
        DVec3::new(0.10, 0.0, 0.0),
        DVec3::new(0.0, 0.0, v_circ),
        EARTH_MASS_SOLAR,
        BodyType::TerrestrialPlanet,
        false,
        false,
        1.0,
        1.0,
        &disk,
        &mut acc_cavity_wide,
    );
    assert_eq!(acc_cavity_wide, DVec3::ZERO);

    // Outside cavity at 1.0 AU: both prograde and retrograde velocities receive opposing drag
    let pos = DVec3::new(1.0, 0.0, 0.0);
    let prograde_vel = DVec3::new(0.0, 0.0, v_circ);
    let retrograde_vel = DVec3::new(0.0, 0.0, -v_circ);
    let mut acc_pro = DVec3::ZERO;
    let mut acc_ret = DVec3::ZERO;

    apply_type_i_torque_acc(
        pos,
        prograde_vel,
        EARTH_MASS_SOLAR,
        BodyType::TerrestrialPlanet,
        false,
        false,
        1.0,
        1.0,
        &disk,
        &mut acc_pro,
    );
    apply_type_i_torque_acc(
        pos,
        retrograde_vel,
        EARTH_MASS_SOLAR,
        BodyType::TerrestrialPlanet,
        false,
        false,
        1.0,
        1.0,
        &disk,
        &mut acc_ret,
    );

    assert!(
        acc_pro.dot(prograde_vel) < 0.0,
        "Prograde orbit must receive drag opposing velocity, got acc={acc_pro:?}"
    );
    assert!(
        acc_ret.dot(retrograde_vel) < 0.0,
        "Retrograde orbit must receive drag opposing velocity, got acc={acc_ret:?}"
    );
    assert!(
        (acc_pro.length() - acc_ret.length()).abs() < 1e-12,
        "Prograde and retrograde drag magnitudes should match"
    );
}

#[test]
fn test_stage2_planet_spin_frame_clamp_and_retrograde_sign() {
    let dt_yr = 1.0 / (365.25 * 24.0); // 1 Earth hour
    let ultra_fast = SpinState {
        spin_vector: DVec3::Y,
        rotation_period_hours: 0.0001,
        axial_tilt_degrees: 0.0,
    };
    let clamped_floor = SpinState {
        spin_vector: DVec3::Y,
        rotation_period_hours: 0.01,
        axial_tilt_degrees: 0.0,
    };

    let (dir_fast, ang_fast) = compute_planet_spin_frame(Some(&ultra_fast), dt_yr);
    let (dir_floor, ang_floor) = compute_planet_spin_frame(Some(&clamped_floor), dt_yr);
    assert!((dir_fast - dir_floor).length() < 1e-12);
    assert!(
        (ang_fast - ang_floor).abs() < 1e-9,
        "Ultra-fast period must clamp to 0.01 hr: ang_fast={ang_fast}, ang_floor={ang_floor}"
    );

    // Inverted spin vector (-Y) + negative period must NOT double-negate retrograde spin
    let retro_inverted = SpinState {
        spin_vector: -DVec3::Y,
        rotation_period_hours: -10.0,
        axial_tilt_degrees: 180.0,
    };
    let (dir_retro, ang_retro) = compute_planet_spin_frame(Some(&retro_inverted), dt_yr);
    assert!(dir_retro.y < -0.99);
    assert!(
        ang_retro > 0.0,
        "Spinning +angle around -Y represents retrograde motion; double negation would yield < 0, got {ang_retro}"
    );
}

#[test]
fn test_stage2_predictor_mutual_roche_and_two_body_mu() {
    let star_mass = 1.0;
    let target_r = DVec3::new(10.0, 0.0, 0.0);
    let target_v = DVec3::new(0.0, 0.0, (G_ASTRO * star_mass / 10.0).sqrt());
    let target_mass = 1e-4;
    let target_radius = 0.002;
    let target_el =
        state_vectors_to_orbital_elements(target_r, target_v, star_mass, target_mass).unwrap();

    let targets = vec![TargetCandidate {
        entity: Entity::from_raw_u32(1).unwrap(),
        name: "Puffy Target".to_string(),
        elements: target_el,
        radius_au: target_radius,
        mass_solar: target_mass,
    }];

    // Heavy compact source crossing 0.015 AU away: > target_r*2.44 (0.00488 AU) but < mutual Roche (~0.0226 AU)
    let src_r = DVec3::new(10.015, 0.0, 0.0);
    let src_mass = 1e-2;
    let src_v = DVec3::new(0.0, 0.0, (G_ASTRO * (star_mass + src_mass) / 10.015).sqrt());
    let src_el = state_vectors_to_orbital_elements(src_r, src_v, star_mass, src_mass).unwrap();

    let (_, massless_enc) =
        find_closest_encounter(&src_el, 1e-4, star_mass, DVec3::ZERO, &targets, 0.2);
    let (_, massive_enc) = find_closest_encounter_with_mass(
        &src_el,
        1e-4,
        src_mass,
        star_mass,
        DVec3::ZERO,
        &targets,
        0.2,
    );

    assert_eq!(
        massless_enc.expect("expected encounter").encounter_type,
        EncounterType::HillSpherePenetration
    );
    assert_eq!(
        massive_enc.expect("expected encounter").encounter_type,
        EncounterType::RocheLobeCrossing
    );

    // Hyperbolic 2-body mu propagation must differ when source_mass_solar > 0.0
    let hyp_r = DVec3::new(1.0, 0.0, 0.0);
    let hyp_v = DVec3::new(0.0, 0.0, TAU * 2.0);
    let hyp_el = state_vectors_to_orbital_elements(hyp_r, hyp_v, star_mass, 0.5).unwrap();
    let (pts_zero, _) = find_closest_encounter(&hyp_el, 0.001, star_mass, DVec3::ZERO, &[], 2.0);
    let (pts_heavy, _) =
        find_closest_encounter_with_mass(&hyp_el, 0.001, 0.5, star_mass, DVec3::ZERO, &[], 2.0);
    let last_zero = pts_zero.last().copied().unwrap();
    let last_heavy = pts_heavy.last().copied().unwrap();
    assert!(
        last_zero.distance(last_heavy) > 0.1,
        "2-body mu must shift hyperbolic trajectory propagation: {last_zero:?} vs {last_heavy:?}"
    );
}

#[derive(Resource)]
struct CycleTestRun(u8);

fn drive_composition_cycle_test(
    mut runs: ResMut<CycleTestRun>,
    mut selected_query: SelectedWorldQuery,
    player_state: Res<PlayerInteractionState>,
    mut toast: ResMut<NotificationToast>,
) {
    if runs.0 > 0 {
        runs.0 -= 1;
        handle_composition_cycle(&mut selected_query, &player_state, &mut toast);
    }
}

#[test]
fn test_stage2_cycle_composition_preserves_moon() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<NotificationToast>()
        .insert_resource(CycleTestRun(3))
        .init_resource::<PlayerInteractionState>()
        .add_systems(Update, drive_composition_cycle_test);

    let moon_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Luna".to_string(),
                body_type: BodyType::Moon,
            },
            Mass(EARTH_MASS_SOLAR * 0.0123),
            Radius(EARTH_RADIUS_AU * 0.27),
            SimPosition(DVec3::new(1.00257, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, TAU)),
            Composition::rocky(),
        ))
        .id();

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(moon_ent);

    for cycle in 1..=3 {
        app.update();
        let body = app.world().entity(moon_ent).get::<CelestialBody>().unwrap();
        assert_eq!(
            body.body_type,
            BodyType::Moon,
            "Cycle {cycle} must preserve BodyType::Moon instead of overwriting with TerrestrialPlanet"
        );
    }
}

#[derive(Resource)]
struct DesiredUiState {
    text: String,
    display: Display,
}

#[derive(Component)]
struct ChurnProbe;

#[derive(SystemParam)]
struct ChurnProbeQuery<'w, 's> {
    q: Query<'w, 's, (&'static mut Text, &'static mut Node), With<ChurnProbe>>,
}

fn apply_churn_probe(desired: Res<DesiredUiState>, mut probe: ChurnProbeQuery) {
    for (mut text, mut node) in &mut probe.q {
        set_text_if_changed(&mut text, desired.text.clone());
        set_display_if_changed(&mut node, desired.display);
    }
}

fn setup_zero_churn_ui_test_app() -> (App, Entity, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(DesiredUiState {
            text: "Updated".to_string(),
            display: Display::Flex,
        })
        .init_resource::<PlayerInteractionState>()
        .init_resource::<SimulationConfig>()
        .add_systems(
            Update,
            (
                apply_churn_probe,
                animate_hud_ui_transforms,
                update_cutaway_hud_ui,
            ),
        );

    let probe = app
        .world_mut()
        .spawn((
            ChurnProbe,
            Text::new("Initial"),
            Node {
                display: Display::None,
                ..default()
            },
        ))
        .id();

    let reticle = app
        .world_mut()
        .spawn((
            HoverReticleBracket,
            Node {
                display: Display::None,
                ..default()
            },
            UiTransform {
                rotation: Rot2::radians(0.0),
                ..default()
            },
        ))
        .id();

    app.world_mut().spawn(Window {
        title: "Test".to_string(),
        resolution: (1280u32, 720u32).into(),
        ..default()
    });
    app.world_mut().spawn((
        Camera3d::default(),
        Camera::default(),
        PanOrbitCamera {
            radius: 10.0,
            target_radius: 10.0,
            ..default()
        },
        Transform::from_xyz(0.0, 4.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::from(Transform::from_xyz(0.0, 4.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y)),
    ));
    let pill = app
        .world_mut()
        .spawn((
            CutawayCalloutPill(0),
            Node {
                display: Display::Flex,
                ..default()
            },
            BorderColor::all(Color::WHITE),
        ))
        .id();
    let bh = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Sagittarius A*".to_string(),
                body_type: BodyType::BlackHole,
            },
            SimPosition(DVec3::ZERO),
            Radius(0.01),
            Mass(4.0e6),
        ))
        .id();
    {
        let mut ps = app.world_mut().resource_mut::<PlayerInteractionState>();
        ps.cutaway_mode = true;
        ps.selected_entity = Some(bh);
    }

    (app, probe, reticle, pill)
}

#[test]
fn test_stage2_ui_zero_churn_reticle_and_cutaway_guards() {
    let (mut app, probe, reticle, pill) = setup_zero_churn_ui_test_app();

    // First tick mutates changed fields and hides BlackHole cutaway pills
    app.update();
    assert_eq!(
        app.world().entity(probe).get::<Text>().unwrap().0,
        "Updated"
    );
    assert_eq!(
        app.world().entity(probe).get::<Node>().unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world().entity(pill).get::<Node>().unwrap().display,
        Display::None,
        "Cutaway HUD pills must stay hidden for BlackHole targets"
    );

    let tick_text_1 = app
        .world()
        .entity(probe)
        .get_ref::<Text>()
        .unwrap()
        .last_changed();
    let tick_node_1 = app
        .world()
        .entity(probe)
        .get_ref::<Node>()
        .unwrap()
        .last_changed();
    let tick_reticle_1 = app
        .world()
        .entity(reticle)
        .get_ref::<UiTransform>()
        .unwrap()
        .last_changed();

    // Second idle tick with identical values: last_changed ticks must stay completely unchanged
    app.update();
    let tick_text_2 = app
        .world()
        .entity(probe)
        .get_ref::<Text>()
        .unwrap()
        .last_changed();
    let tick_node_2 = app
        .world()
        .entity(probe)
        .get_ref::<Node>()
        .unwrap()
        .last_changed();
    let tick_reticle_2 = app
        .world()
        .entity(reticle)
        .get_ref::<UiTransform>()
        .unwrap()
        .last_changed();
    assert_eq!(
        tick_text_2, tick_text_1,
        "set_text_if_changed must not bump Text last_changed tick when string is identical"
    );
    assert_eq!(
        tick_node_2, tick_node_1,
        "set_display_if_changed must not bump Node last_changed tick when display is identical"
    );
    assert_eq!(
        tick_reticle_2, tick_reticle_1,
        "Hidden HoverReticleBracket must not bump UiTransform last_changed tick"
    );

    // Third tick with changed desired values: last_changed ticks must advance
    {
        let mut desired = app.world_mut().resource_mut::<DesiredUiState>();
        desired.text = "ChangedAgain".to_string();
        desired.display = Display::None;
    }
    app.update();
    let tick_text_3 = app
        .world()
        .entity(probe)
        .get_ref::<Text>()
        .unwrap()
        .last_changed();
    let tick_node_3 = app
        .world()
        .entity(probe)
        .get_ref::<Node>()
        .unwrap()
        .last_changed();
    assert_ne!(
        tick_text_3, tick_text_2,
        "set_text_if_changed must bump Text last_changed tick when string changes"
    );
    assert_ne!(
        tick_node_3, tick_node_2,
        "set_display_if_changed must bump Node last_changed tick when display changes"
    );
}

#[test]
fn test_stage2_pulsar_spin_rate_and_quasar_fallback() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(VisualAssets::dummy(Handle::default()))
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<SimulationConfig>()
        .insert_resource(SimTime {
            visual_time_secs: 0.02,
            ..default()
        })
        .add_systems(Update, (sync_pulsar_beams, sync_quasar_beams));

    let fast_pulsar = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Fast Pulsar".to_string(),
                body_type: BodyType::NeutronStar,
            },
            SimPosition(DVec3::ZERO),
            Radius(1e-6),
            SpinState {
                spin_vector: DVec3::Y,
                rotation_period_hours: 0.5,
                axial_tilt_degrees: 0.0,
            },
        ))
        .id();

    let slow_pulsar = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Slow Pulsar".to_string(),
                body_type: BodyType::NeutronStar,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            Radius(1e-6),
            SpinState {
                spin_vector: DVec3::Y,
                rotation_period_hours: 10.0,
                axial_tilt_degrees: 0.0,
            },
        ))
        .id();

    let quasar = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "AGN Quasar".to_string(),
                body_type: BodyType::BlackHole,
            },
            SimPosition(DVec3::new(5.0, 0.0, 0.0)),
            Mass(1_000.0),
            Radius(0.005),
        ))
        .id();

    app.update();

    let base_rot = Quat::from_rotation_x(0.38);
    let mut fast_angle = None;
    let mut slow_angle = None;
    let mut pulsar_q = app
        .world_mut()
        .query_filtered::<(&StructureHostTarget, &Transform), With<PulsarBeamRoot>>();
    for (host, tf) in pulsar_q.iter(app.world()) {
        let angle = base_rot.angle_between(tf.rotation);
        if host.0 == fast_pulsar {
            fast_angle = Some(angle);
        } else if host.0 == slow_pulsar {
            slow_angle = Some(angle);
        }
    }

    let ang_fast = fast_angle.expect("Fast pulsar must spawn PulsarBeamRoot");
    let ang_slow = slow_angle.expect("Slow pulsar must spawn PulsarBeamRoot");
    assert!(
        ang_fast > ang_slow * 5.0,
        "Fast SpinState pulsar must rotate much faster than slow pulsar: fast={ang_fast}, slow={ang_slow}"
    );

    let mut quasar_q = app
        .world_mut()
        .query_filtered::<&StructureHostTarget, With<QuasarBeamRoot>>();
    let has_quasar_beam = quasar_q.iter(app.world()).any(|host| host.0 == quasar);
    assert!(
        has_quasar_beam,
        "Named BlackHole 'Quasar' without BlackHoleStarState must still spawn QuasarBeamRoot via fallback"
    );
}

fn register_game_systems_for_init_check(app: &mut App) {
    use protostellar::game::audio::{setup_cosmic_audio_assets, update_cosmic_audio_system};
    use protostellar::game::interaction::{handle_planet_builder_click_spawn, handle_player_tools};
    use protostellar::game::phases::monitor_phase_transitions;
    use protostellar::game::slingshot::handle_slingshot_input;
    use protostellar::game::time_control::handle_time_control_input;
    use protostellar::game::ui::{
        cutaway_hud::update_cutaway_hud_ui,
        handle_cosmic_event_toasts, handle_inspector_scroll, handle_roche_disruption_toasts,
        handle_ui_button_interactions,
        hover_card::update_hover_card_ui,
        reset_inspector_scroll_on_target_change, setup_hud,
        tech_tree_panel::update_tech_tree_panel,
        trophies_panel::update_trophies_panel,
        update_campaign_ui, update_epoch_scrubber_ui, update_hazards_ui, update_hr_diagram_ui,
        update_hud, update_hud_visibility, update_planet_builder_ui, update_quick_bar_highlights,
        update_quick_body_selector_bar, update_saves_panel, update_scenario_contextual_ui,
        update_telemetry_graph_ui,
        widget_sync::{apply_hud_slider_changes, sync_hud_sliders, sync_hud_text_inputs},
    };

    app.add_systems(Startup, (setup_hud, setup_cosmic_audio_assets))
        .add_systems(
            Update,
            (
                monitor_phase_transitions,
                handle_time_control_input,
                handle_player_tools,
                handle_planet_builder_click_spawn,
                handle_slingshot_input,
                handle_ui_button_interactions,
                update_quick_body_selector_bar,
                update_quick_bar_highlights,
                update_planet_builder_ui,
                update_telemetry_graph_ui,
                update_trophies_panel,
                update_tech_tree_panel,
                update_campaign_ui,
                update_hazards_ui,
                update_epoch_scrubber_ui,
                update_hr_diagram_ui,
                update_saves_panel,
                handle_roche_disruption_toasts,
                handle_cosmic_event_toasts,
                update_cosmic_audio_system,
            ),
        )
        .add_systems(
            Update,
            (
                handle_inspector_scroll,
                reset_inspector_scroll_on_target_change,
                update_hud_visibility,
                update_scenario_contextual_ui,
                update_hud,
                update_hover_card_ui,
                update_cutaway_hud_ui,
                sync_hud_sliders,
                apply_hud_slider_changes,
                sync_hud_text_inputs,
                animate_hud_ui_transforms,
            ),
        );
}

fn register_rendering_systems_for_init_check(app: &mut App) {
    use protostellar::rendering::bodies::{
        spawn_missing_visuals, sync_black_hole_accretion_disks, sync_camera_adaptive_body_scales,
        sync_celestial_transforms, sync_circumstellar_environments, sync_cometary_tails,
        sync_magnetar_structures, sync_magnetic_field_overlays, sync_planetary_atmospheres,
        sync_planetary_rings, sync_relativistic_jets, sync_selected_body_mesh_lod,
    };
    use protostellar::rendering::camera::update_pan_orbit_camera;
    use protostellar::rendering::effects::{
        draw_bombardment_gizmos, draw_dark_matter_halos, draw_dyson_swarms,
        draw_habitable_zone_gizmos, draw_impact_ejecta_system, draw_kilonova_gizmos,
        draw_lagrange_points_gizmos, draw_orbital_effects_and_gizmos,
        draw_persistent_remnants_gizmos, draw_tde_gizmos, sync_quasar_blowout_visuals,
        sync_supernova_visuals, update_impact_ejecta, update_impact_shockwaves,
        update_kilonova_effects, update_megastructure_construction, update_persistent_remnants,
        update_roche_debris_streams, update_roche_streams, update_supernova_explosions,
        update_tidal_disruption_streams,
    };
    use protostellar::rendering::skybox::{sync_skybox_to_camera, update_skybox_uniforms};
    use protostellar::rendering::surface_camera::{
        draw_surface_horizon_environment_gizmos, update_surface_horizon_camera,
    };
    use protostellar::rendering::surface_terrain::sync_surface_terrain_system;
    use protostellar::rendering::volumetric_nebula::sync_volumetric_nebula;

    app.add_systems(
        Update,
        (
            sync_celestial_transforms,
            sync_selected_body_mesh_lod,
            update_pan_orbit_camera,
            update_surface_horizon_camera,
            spawn_missing_visuals,
            sync_camera_adaptive_body_scales,
            sync_surface_terrain_system,
            sync_skybox_to_camera,
            update_skybox_uniforms,
            sync_planetary_atmospheres,
            sync_circumstellar_environments,
            sync_planetary_rings,
            sync_black_hole_accretion_disks,
            sync_cometary_tails,
            sync_quasar_beams,
            sync_pulsar_beams,
            sync_magnetar_structures,
            sync_magnetic_field_overlays,
            sync_relativistic_jets,
            sync_supernova_visuals,
        ),
    )
    .add_systems(
        Update,
        (
            sync_volumetric_nebula,
            draw_orbital_effects_and_gizmos,
            draw_habitable_zone_gizmos,
            draw_lagrange_points_gizmos,
            draw_dark_matter_halos,
            draw_bombardment_gizmos,
            draw_persistent_remnants_gizmos,
            draw_kilonova_gizmos,
            draw_tde_gizmos,
            draw_impact_ejecta_system,
            draw_dyson_swarms,
            draw_surface_horizon_environment_gizmos,
            update_impact_shockwaves,
            update_impact_ejecta,
            update_roche_debris_streams,
            update_roche_streams,
            update_tidal_disruption_streams,
            update_supernova_explosions,
            update_persistent_remnants,
            update_kilonova_effects,
        ),
    )
    .add_systems(
        Update,
        (
            update_megastructure_construction,
            sync_quasar_blowout_visuals,
            protostellar::rendering::post_processing::update_adaptive_exposure_and_bloom,
            protostellar::rendering::post_processing::update_post_processing_effects,
            protostellar::rendering::gas_clouds::update_gas_cloud_material,
        ),
    );
}

#[test]
fn test_stage2_all_core_systems_initialize_without_query_conflicts() {
    use protostellar::simulation::SimulationPlugin;

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        SimulationPlugin,
    ));
    register_game_systems_for_init_check(&mut app);
    register_rendering_systems_for_init_check(&mut app);

    // Initialize Startup and Update schedules — panics immediately on any Bevy ECS B0001 query conflict
    app.world_mut()
        .try_schedule_scope(Startup, |world, schedule| {
            schedule
                .initialize(world)
                .expect("All Startup ECS systems must initialize without B0001 query conflicts");
        })
        .expect("Startup schedule must exist");
    app.world_mut()
        .try_schedule_scope(Update, |world, schedule| {
            schedule
                .initialize(world)
                .expect("All Update ECS systems must initialize without B0001 query conflicts");
        })
        .expect("Update schedule must exist");
}

fn setup_quasar_blowout_test_app() -> (App, Entity, Entity) {
    use bevy::core_pipeline::tonemapping::Tonemapping;
    use protostellar::rendering::effects::quasar_blowout::sync_quasar_blowout_visuals;
    use protostellar::rendering::effects::supernova::{
        SupernovaDebrisPool, sync_supernova_visuals, update_supernova_explosions,
    };
    use protostellar::rendering::materials::SupernovaMaterial;
    use protostellar::rendering::post_processing::{
        PostProcessingSettings, update_post_processing_effects,
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<SupernovaEvent>()
        .init_resource::<SupernovaDebrisPool>()
        .insert_resource(VisualAssets::dummy(Handle::default()))
        .init_resource::<Assets<SupernovaMaterial>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(
            Update,
            (
                update_supernova_explosions,
                sync_supernova_visuals.after(update_supernova_explosions),
                sync_quasar_blowout_visuals.after(update_supernova_explosions),
                update_post_processing_effects.after(update_supernova_explosions),
            ),
        );

    let cam_ent = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            GlobalTransform::from(
                Transform::from_xyz(0.0, 40.0, 140.0).looking_at(Vec3::ZERO, Vec3::Y),
            ),
            PostProcessingSettings::default(),
            Tonemapping::AgX,
        ))
        .id();

    let mut bhs = BlackHoleStarState {
        black_hole_mass_solar: 400_000.0,
        cocoon_mass_solar: 50_000.0,
        cocoon_radius_au: 60.0,
        ..default()
    };
    bhs.trigger_blowout();

    let lrd_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "JWST Little Red Dot (Black Hole Star)".to_string(),
                body_type: BodyType::QuasiStar,
            },
            Mass(450_000.0),
            Radius(60.0),
            SimPosition(DVec3::ZERO),
            Transform::from_scale(Vec3::splat(1.77)),
            bhs,
        ))
        .id();

    (app, cam_ent, lrd_ent)
}

#[test]
fn test_quasar_blowout_spawns_multilayer_3d_explosion_and_post_fx() {
    use protostellar::game::audio::build_quasar_blowout_wav;
    use protostellar::rendering::effects::quasar_blowout::{
        QUASAR_BLOWOUT_SHARD_COUNT, QuasarBlowoutLayer, QuasarBlowoutLight, QuasarBlowoutShard,
    };
    use protostellar::rendering::effects::supernova::{
        SupernovaDebrisPool, SupernovaType, SupernovaVisualShell,
    };
    use protostellar::rendering::post_processing::PostProcessingSettings;

    let (mut app, cam_ent, lrd_ent) = setup_quasar_blowout_test_app();

    // Advance 0.25s in deterministic manual time to trigger blowout + breakout crack-swell
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(250),
    ));
    app.update();
    app.update();

    {
        let pool = app.world().resource::<SupernovaDebrisPool>();
        assert_eq!(pool.explosions.len(), 1);
        let exp = pool.explosions.first().expect("expected blowout instance");
        assert_eq!(exp.star_entity, lrd_ent);
        assert_eq!(exp.explosion_type, SupernovaType::QuasarBlowout);
        assert!(exp.initial_radius_au >= 1.77);
        assert!(exp.current_radius_au > 1.77);
    }

    let outer_shells = app
        .world_mut()
        .query::<&SupernovaVisualShell>()
        .iter(app.world())
        .count();
    let secondary_layers = app
        .world_mut()
        .query::<&QuasarBlowoutLayer>()
        .iter(app.world())
        .count();
    let shard_count = app
        .world_mut()
        .query::<&QuasarBlowoutShard>()
        .iter(app.world())
        .count();
    let light_count = app
        .world_mut()
        .query::<&QuasarBlowoutLight>()
        .iter(app.world())
        .count();
    assert_eq!(outer_shells, 1, "Must spawn outer shattering cocoon shell");
    assert_eq!(
        secondary_layers, 3,
        "Must spawn inner core, equatorial Mach disk, and polar breakout cones"
    );
    assert_eq!(shard_count, QUASAR_BLOWOUT_SHARD_COUNT);
    assert_eq!(light_count, 1, "Must spawn dynamic explosion PointLight");

    let pp = app
        .world()
        .entity(cam_ent)
        .get::<PostProcessingSettings>()
        .unwrap();
    assert!(
        pp.shockwave_amplitude > 0.2,
        "Fullscreen shockwave amplitude must activate on QuasarBlowout, got {}",
        pp.shockwave_amplitude
    );
    assert!(
        pp.hazard_flash_intensity > 0.1,
        "Prompt radiation flash must activate on QuasarBlowout, got {}",
        pp.hazard_flash_intensity
    );

    // Advance 4.0 seconds in real time (16 * 250ms, respecting Time<Virtual>'s 250ms max_delta):
    // explosion must expand dramatically beyond 100 AU
    for _ in 0..16 {
        app.update();
    }
    let mid_r = app
        .world()
        .resource::<SupernovaDebrisPool>()
        .explosions
        .first()
        .expect("blowout still expanding at t=4.25s")
        .current_radius_au;
    assert!(
        mid_r > 100.0,
        "Blowout shell must expand beyond 100 AU by t=4.25s, got {mid_r}"
    );

    // Scenario reload (despawning the star + clearing the debris pool) must despawn all 3D blowout entities cleanly
    app.world_mut().despawn(lrd_ent);
    app.world_mut()
        .resource_mut::<SupernovaDebrisPool>()
        .clear();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&QuasarBlowoutLayer>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&QuasarBlowoutShard>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&QuasarBlowoutLight>()
            .iter(app.world())
            .count(),
        0
    );

    // Verify procedural blowout audio WAV buffer is well-formed
    let wav = build_quasar_blowout_wav(22_050);
    assert!(wav.len() > 44 && wav.starts_with(b"RIFF"));

    // Verify upgraded supernova.wesl compiles cleanly through Bevy + Naga
    super::shader_guardrails::compile_and_validate_wesl_shader(
        "assets/shaders/supernova.wesl",
        &[],
    );
}
