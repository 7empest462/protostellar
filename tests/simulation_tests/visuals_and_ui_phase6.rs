//! Regression tests for Phase 6: Cutaway archetypes, moon tidal parents, retrograde spin,
//! multi-remnant structures, builder presets/hyperbolic insertion, and confirmed hotkey disambiguation.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::game::interaction::{
    SelectedBodyQuery, handle_selected_body_tools, handle_viewport_and_tool_hotkeys,
};
use protostellar::game::slingshot::archetype_preset_mass_solar;
use protostellar::game::time_control::handle_time_control_input;
use protostellar::game::ui::builder_spawner::{
    compute_builder_spawn_coords, resolve_builder_body_type_and_temperature,
    spawn_custom_builder_world,
};
use protostellar::game::ui::cutaway_hud::resolve_cutaway_labels_with_telemetry;
use protostellar::game::ui::interactions::builder::handle_builder_action;
use protostellar::game::ui::telemetry_format::{
    format_body_inspector_type, format_orbital_period_string,
};
use protostellar::game::ui::types::{
    BuilderPreset, NotificationToast, PlanetBuilderState, SelectedWorldQuery, TelemetryPanelState,
    UiButtonAction,
};
use protostellar::game::ui::world_list::collect_sorted_system_worlds;
use protostellar::rendering::bodies::shadows::compute_ring_shadow_params;
use protostellar::rendering::bodies::{
    MagnetarStructureRoot, PulsarBeamRoot, VisualAssets, VisualBody, sync_celestial_transforms,
    sync_magnetar_structures, sync_pulsar_beams,
};
use protostellar::rendering::camera::PanOrbitCamera;
use protostellar::rendering::effects::planetary::resolve_cutaway_strata;
use protostellar::rendering::materials::{PlanetMaterial, PlanetMaterialExtension, PlanetUniforms};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::terraforming::TerraformingAtmosphere;
use protostellar::simulation::tides::TidalState;
use protostellar::utils::constants::{
    EARTH_MASS_SOLAR, EARTH_RADIUS_AU, JUPITER_MASS_SOLAR, SOLAR_RADIUS_AU,
};
use protostellar::utils::math::state_vectors_to_orbital_elements;

#[test]
fn test_phase6_cutaway_strata_and_telemetry_labels() {
    let config = SimulationConfig::default();

    // 1. Undifferentiated primitive asteroid must have 0.0 core radii and chondritic labels
    let asteroid = CelestialBody {
        name: "16 Psyche Rubble".to_string(),
        body_type: BodyType::Asteroid,
    };
    let ast_strata = resolve_cutaway_strata(&asteroid, 1.0e-6, None, &config);
    assert_eq!(ast_strata.inner_core_r, 0.0);
    assert_eq!(ast_strata.core_r, 0.0);
    assert!(!ast_strata.has_theia_llsvp);

    let ast_labels =
        resolve_cutaway_labels_with_telemetry(&asteroid, 1.0e-6, None, None, None, None);
    assert!(ast_labels[0].0.contains("REGOLITH"));
    assert!(ast_labels[1].0.contains("UNDIFFERENTIATED"));
    assert_eq!(ast_labels[2].0, "");
    assert_eq!(ast_labels[3].0, "");

    // 2. Red Dwarf cutaway labels must reflect real stellar temperature/regime rather than Sun constants
    let red_dwarf = CelestialBody {
        name: "Proxima Centauri".to_string(),
        body_type: BodyType::RedDwarf,
    };
    let ign = IgnitionState {
        core_temperature: 6_500_000.0,
        fusion_fraction: 1.0,
        is_ignited: true,
        shockwave_radius: 0.0,
    };
    let evo = StellarEvolutionState {
        phase: StellarEvolutionPhase::MainSequence,
        ..default()
    };
    let rd_labels = resolve_cutaway_labels_with_telemetry(
        &red_dwarf,
        0.15 * SOLAR_RADIUS_AU,
        None,
        Some(&Temperature(3_100.0)),
        Some(&ign),
        Some(&evo),
    );
    assert!(rd_labels[0].1.contains("3100 K"));
    assert!(!rd_labels[0].1.contains("5,778 K"));
    assert!(rd_labels[3].1.contains("6500000 K"));
}

fn add_planet_mat(mats: &mut Assets<PlanetMaterial>, spin_angle: f32) -> Handle<PlanetMaterial> {
    mats.add(PlanetMaterial {
        base: StandardMaterial::default(),
        extension: PlanetMaterialExtension {
            uniforms: PlanetUniforms {
                spin_angle,
                ..PlanetUniforms::default()
            },
        },
    })
}

fn spawn_retro_and_moon_sync_scene(
    app: &mut App,
) -> (
    Handle<PlanetMaterial>,
    Handle<PlanetMaterial>,
    Handle<PlanetMaterial>,
) {
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Sphere::new(1.0).mesh().ico(1).unwrap());
    app.insert_resource(VisualAssets::dummy(mesh.clone()));

    let (sun_mat, jup_mat, io_mat, retro_mat, ast_mat) = {
        let mut planet_mats = app.world_mut().resource_mut::<Assets<PlanetMaterial>>();
        (
            add_planet_mat(&mut planet_mats, 0.0),
            add_planet_mat(&mut planet_mats, 0.0),
            add_planet_mat(&mut planet_mats, 0.0),
            add_planet_mat(&mut planet_mats, 1.0),
            add_planet_mat(&mut planet_mats, 0.0),
        )
    };

    app.world_mut().spawn((
        VisualBody,
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Composition::solar_gas(),
        SimPosition(DVec3::ZERO),
        Mesh3d(mesh.clone()),
        MeshMaterial3d(sun_mat),
        Transform::default(),
    ));

    let jupiter_ent = app
        .world_mut()
        .spawn((
            VisualBody,
            CelestialBody {
                name: "Jupiter".to_string(),
                body_type: BodyType::GasGiant,
            },
            Mass(0.001),
            Radius(0.00048),
            Temperature(160.0),
            Composition::solar_gas(),
            SimPosition(DVec3::new(5.2, 0.0, 0.0)),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(jup_mat),
            Transform::default(),
        ))
        .id();

    app.world_mut().spawn((
        VisualBody,
        CelestialBody {
            name: "Io".to_string(),
            body_type: BodyType::Moon,
        },
        Mass(4.5e-8),
        Radius(1.22e-5),
        Temperature(110.0),
        Composition::rocky(),
        SimPosition(DVec3::new(5.2, 0.0, 0.0028)),
        SatelliteOf {
            parent: jupiter_ent,
            semi_major_axis_au: 0.0028,
            orbital_period_years: 0.0048,
            true_anomaly: 0.0,
        },
        TidalState {
            is_tidally_locked: true,
            ..default()
        },
        Mesh3d(mesh.clone()),
        MeshMaterial3d(io_mat.clone()),
        Transform::default(),
    ));

    app.world_mut().spawn((
        VisualBody,
        CelestialBody {
            name: "Retrograde Venus".to_string(),
            body_type: BodyType::TerrestrialPlanet,
        },
        Mass(EARTH_MASS_SOLAR),
        Radius(EARTH_RADIUS_AU),
        Temperature(288.0),
        Composition::rocky(),
        SimPosition(DVec3::new(0.72, 0.0, 0.0)),
        SpinState {
            spin_vector: DVec3::new(0.0, -1.0, 0.0),
            rotation_period_hours: -24.0,
            axial_tilt_degrees: 0.0,
        },
        Mesh3d(mesh.clone()),
        MeshMaterial3d(retro_mat.clone()),
        Transform::default(),
    ));

    let ast_ent = app
        .world_mut()
        .spawn((
            VisualBody,
            CelestialBody {
                name: "Bennu".to_string(),
                body_type: BodyType::Asteroid,
            },
            Mass(1e-12),
            Radius(1e-8),
            Temperature(200.0),
            Composition::rocky(),
            SimPosition(DVec3::new(1.1, 0.0, 0.0)),
            Mesh3d(mesh),
            MeshMaterial3d(ast_mat.clone()),
            Transform::default(),
        ))
        .id();

    let mut ps = app.world_mut().resource_mut::<PlayerInteractionState>();
    ps.cutaway_mode = true;
    ps.selected_entity = Some(ast_ent);

    (io_mat, retro_mat, ast_mat)
}

#[test]
fn test_phase6_retrograde_spin_and_moon_tidal_parent() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<PlanetMaterial>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();

    let (io_mat, retro_mat, ast_mat) = spawn_retro_and_moon_sync_scene(&mut app);
    {
        let mut st = app.world_mut().resource_mut::<SimTime>();
        st.elapsed_years = 0.01;
        st.current_dt_yr = 0.0001;
    }

    app.add_systems(Update, sync_celestial_transforms);
    app.update();

    let mats = app.world().resource::<Assets<PlanetMaterial>>();
    let io_u = &mats.get(&io_mat).unwrap().extension.uniforms;
    assert!(
        io_u.tidal_deformation.z < -0.9 && io_u.tidal_deformation.x.abs() < 0.05,
        "Moon tidal direction must point toward Jupiter (-Z), got {:?}",
        io_u.tidal_deformation
    );
    assert!(
        io_u.tidal_deformation.w < 1.5,
        "Moons must not carry star-locked +2.0 eyeball flag"
    );

    let retro_u = &mats.get(&retro_mat).unwrap().extension.uniforms;
    assert!(
        retro_u.spin_axis.y < -0.99,
        "Pure retrograde spin_vector=(0,-1,0) must stay negative Y, got {:?}",
        retro_u.spin_axis
    );
    assert!(
        retro_u.spin_angle < 1.0,
        "Negative rotation_period_hours must decrease spin_angle below 1.0, got {}",
        retro_u.spin_angle
    );

    let ast_u = &mats.get(&ast_mat).unwrap().extension.uniforms;
    assert_eq!(ast_u.cutaway_params.x, 1.0);
    assert_eq!(ast_u.cutaway_params.y, 0.0);
    assert_eq!(ast_u.cutaway_params.z, 0.0);
}

#[test]
fn test_phase6_rings_multi_remnants_and_world_list() {
    // 1. Ring shadow clamps allow compact & wide rings (1.35..=12.0)
    let wide = PlanetaryRingSystem {
        inner_radius_au: 1.5e-4,
        outer_radius_au: 8.0e-4,
        ring_mass_earth: 1e-4,
        optical_depth: 0.7,
        ice_fraction: 0.9,
        silicate_fraction: 0.1,
    };
    let rp = compute_ring_shadow_params(Some(&wide), 1.0e-4);
    assert!((rp.x - 1.5).abs() < 1e-3);
    assert!((rp.y - 8.0).abs() < 1e-3);

    // 2. Multi-Pulsar & Multi-Magnetar support
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();

    let (star_mesh, cyl_mesh) = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        (
            meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            meshes.add(Cylinder::new(1.0, 1.0)),
        )
    };
    let mut visual_assets = VisualAssets::dummy(star_mesh);
    visual_assets.pulsar_beam_mesh = cyl_mesh.clone();
    visual_assets.magnetar_ring_mesh = cyl_mesh.clone();
    visual_assets.magnetar_field_loops_mesh = cyl_mesh;
    app.insert_resource(visual_assets);
    app.add_systems(Update, (sync_pulsar_beams, sync_magnetar_structures));

    for i in 0..2 {
        app.world_mut().spawn((
            CelestialBody {
                name: format!("Pulsar {i}"),
                body_type: BodyType::Pulsar,
            },
            SimPosition(DVec3::new(f64::from(i) * 2.0, 0.0, 0.0)),
            Radius(0.0001),
            SpinState {
                spin_vector: DVec3::new(0.2, 0.98, 0.0),
                ..default()
            },
        ));
        app.world_mut().spawn((
            CelestialBody {
                name: format!("Magnetar {i}"),
                body_type: BodyType::Magnetar,
            },
            SimPosition(DVec3::new(0.0, 0.0, f64::from(i) * 2.0 + 1.0)),
            Radius(0.0001),
        ));
    }

    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamRoot>()
            .iter(app.world())
            .count(),
        2
    );
    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        2
    );

    // 3. Multi-CentralStar world list retention + Telemetry fixes
    let s1 = app.world_mut().spawn_empty().id();
    let s2 = app.world_mut().spawn_empty().id();
    let star1 = CelestialBody {
        name: "Alpha".to_string(),
        body_type: BodyType::YellowDwarf,
    };
    let star2 = CelestialBody {
        name: "Beta".to_string(),
        body_type: BodyType::RedDwarf,
    };
    let worlds = collect_sorted_system_worlds(
        [
            (
                s1,
                &star1,
                &SimPosition(DVec3::new(-0.2, 0.0, 0.0)),
                &Mass(1.0),
                &Radius(SOLAR_RADIUS_AU),
                Some(&CentralStar),
            ),
            (
                s2,
                &star2,
                &SimPosition(DVec3::new(0.3, 0.0, 0.0)),
                &Mass(0.4),
                &Radius(0.4 * SOLAR_RADIUS_AU),
                Some(&CentralStar),
            ),
        ],
        false,
    );
    assert_eq!(worlds.len(), 2);
    assert_eq!(
        format_body_inspector_type(BodyType::Hypergiant, 80.0),
        "Luminous Hypergiant (Ia-0)"
    );
    let period_fmt = format_orbital_period_string(0.0028, BodyType::Moon, 3.0e-6);
    assert!(period_fmt.contains("days") || period_fmt.contains("hrs"));
}

#[test]
fn test_phase6_builder_hyperbolic_and_star_spawn() {
    let comp = Composition::rocky();
    let (bt, rad, temp) = resolve_builder_body_type_and_temperature(
        BuilderPreset::RedDwarfStar,
        0.25,
        &comp,
        EARTH_RADIUS_AU,
    );
    assert_eq!(bt, BodyType::RedDwarf);
    assert!((rad - 0.25 * SOLAR_RADIUS_AU).abs() < 1e-10);
    assert_eq!(temp, 3_100.0);

    let builder_hyp = PlanetBuilderState {
        semi_major_axis_au: 2.0,
        eccentricity: 1.25,
        active_preset: BuilderPreset::RedDwarfStar,
        mass_solar: 0.25,
        ..default()
    };
    let (pos, vel) = compute_builder_spawn_coords(&builder_hyp, 1.0, None);
    let el = state_vectors_to_orbital_elements(pos, vel, 1.0, 0.0).unwrap();
    assert!(
        el.eccentricity >= 1.20,
        "Hyperbolic builder spawn expected e >= 1.20, got {}",
        el.eccentricity
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let (star_ent, io_ent) = {
        let mut ps = PlayerInteractionState::default();
        let mut toast = NotificationToast::default();
        let mut state: bevy::ecs::system::SystemState<(Commands, Query<&mut PanOrbitCamera>)> =
            bevy::ecs::system::SystemState::new(app.world_mut());
        let (mut commands, mut cam_q) = state.get_mut(app.world_mut()).unwrap();

        let star_ent = spawn_custom_builder_world(
            &mut commands,
            &builder_hyp,
            1.0,
            None,
            &mut ps,
            &mut cam_q,
            &mut toast,
        );
        let io_builder = PlanetBuilderState {
            active_preset: BuilderPreset::IoLike,
            mass_solar: EARTH_MASS_SOLAR * 0.015,
            semi_major_axis_au: 1.5,
            eccentricity: 0.0,
            ..default()
        };
        let io_ent = spawn_custom_builder_world(
            &mut commands,
            &io_builder,
            1.0,
            None,
            &mut ps,
            &mut cam_q,
            &mut toast,
        );
        state.apply(app.world_mut());
        (star_ent, io_ent)
    };

    assert!(app.world().get::<Luminosity>(star_ent).is_some());
    assert!(app.world().get::<IgnitionState>(star_ent).is_some());
    assert!(app.world().get::<StellarEvolutionState>(star_ent).is_some());
    assert!(app.world().get::<PlanetaryClimate>(star_ent).is_none());
    assert!(app.world().get::<VolcanicState>(io_ent).is_some());

    // Sub-Roche moon spawn via handle_builder_action attaches BodyType::Moon, SatelliteOf & InternalDifferentiation
    let jup = app
        .world_mut()
        .spawn((
            Mass(0.001),
            Radius(0.00048),
            SimPosition(DVec3::new(5.2, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.75)),
            Composition::solar_gas(),
            CelestialBody {
                name: "Jupiter".to_string(),
                body_type: BodyType::GasGiant,
            },
        ))
        .id();
    {
        let mut ps = PlayerInteractionState {
            selected_entity: Some(jup),
            ..default()
        };
        let mut bs = PlanetBuilderState {
            eccentricity: 0.60,
            ..default()
        };
        let mut toast = NotificationToast::default();
        let mut ss: bevy::ecs::system::SystemState<(
            SelectedWorldQuery,
            Query<&mut PanOrbitCamera>,
            Commands,
        )> = bevy::ecs::system::SystemState::new(app.world_mut());
        let (sel_q, mut cam_q, mut cmd) = ss.get_mut(app.world_mut()).unwrap();
        assert!(handle_builder_action(
            &UiButtonAction::BuilderCycleEccentricity,
            &mut bs,
            &mut toast,
            1.0,
            &mut ps,
            &mut cam_q,
            &sel_q,
            &mut cmd,
        ));
        assert!((bs.eccentricity - 1.25).abs() < 1e-6);
        assert!(handle_builder_action(
            &UiButtonAction::SpawnSubRocheMoon,
            &mut bs,
            &mut toast,
            1.0,
            &mut ps,
            &mut cam_q,
            &sel_q,
            &mut cmd,
        ));
        ss.apply(app.world_mut());
    }
    let moons: Vec<_> = app
        .world_mut()
        .query::<(&CelestialBody, &SatelliteOf, &InternalDifferentiation)>()
        .iter(app.world())
        .collect();
    assert_eq!(moons.len(), 1);
    assert_eq!(moons[0].0.body_type, BodyType::Moon);
    assert_eq!(moons[0].1.parent, jup);

    assert!(
        (archetype_preset_mass_solar(SlingshotArchetype::GasGiant) - JUPITER_MASS_SOLAR).abs()
            < 1e-9
    );
}

fn setup_hotkey_and_moon_test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<NotificationToast>();

    app.world_mut().spawn((
        CentralStar,
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Composition::solar_gas(),
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
    ));

    let jup = app
        .world_mut()
        .spawn((
            Mass(0.001),
            Radius(0.00048),
            Temperature(160.0),
            SimPosition(DVec3::new(5.2, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.75)),
            Composition::solar_gas(),
            CelestialBody {
                name: "Jupiter".to_string(),
                body_type: BodyType::GasGiant,
            },
        ))
        .id();

    let moon_pos = DVec3::new(5.2, 0.0, 0.003);
    let moon_vel = DVec3::new(0.0, 0.0, 2.75) + DVec3::new(-3.6, 0.0, 0.0);
    let moon = app
        .world_mut()
        .spawn((
            Mass(4.5e-8),
            Radius(1.22e-5),
            Temperature(110.0),
            SimPosition(moon_pos),
            SimVelocity(moon_vel),
            Composition::rocky(),
            CelestialBody {
                name: "Io".to_string(),
                body_type: BodyType::Moon,
            },
            SatelliteOf {
                parent: jup,
                semi_major_axis_au: 0.003,
                orbital_period_years: 0.005,
                true_anomaly: 0.0,
            },
            VolatileInventory::default(),
            PlanetaryClimate {
                surface_temperature_k: 310.0,
                ..default()
            },
            TerraformingAtmosphere::default(),
        ))
        .id();

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(moon);
    app.add_systems(Update, handle_time_control_input);
    (app, moon)
}

fn run_selected_body_tools_once(app: &mut App) {
    let mut ss: bevy::ecs::system::SystemState<(
        Commands,
        Res<ButtonInput<KeyCode>>,
        ResMut<PlayerInteractionState>,
        SelectedBodyQuery,
        ResMut<NotificationToast>,
    )> = bevy::ecs::system::SystemState::new(app.world_mut());

    let (mut cmds, kb, mut ps, mut sq, mut toast) = ss.get_mut(app.world_mut()).unwrap();
    handle_selected_body_tools(
        &mut cmds,
        &kb,
        1.0,
        DVec3::ZERO,
        DVec3::ZERO,
        &mut ps,
        &mut sq,
        &mut toast,
    );
    ss.apply(app.world_mut());
}

fn run_viewport_hotkeys_once(app: &mut App) {
    type ViewportHotkeyState<'w, 's> = bevy::ecs::system::SystemState<(
        Res<'w, ButtonInput<KeyCode>>,
        ResMut<'w, SimulationConfig>,
        ResMut<'w, PlayerInteractionState>,
        ResMut<'w, NotificationToast>,
        Query<'w, 's, (&'static Transform, &'static mut PanOrbitCamera)>,
    )>;

    let mut ss: ViewportHotkeyState<'_, '_> = bevy::ecs::system::SystemState::new(app.world_mut());
    let (kb, mut cfg, mut ps, mut toast, mut cam_q) = ss.get_mut(app.world_mut()).unwrap();
    let mut bs = PlanetBuilderState::default();
    let mut ts = TelemetryPanelState::default();
    handle_viewport_and_tool_hotkeys(
        &kb, &mut cfg, &mut ps, &mut bs, &mut ts, None, None, None, None, None, None, None, None,
        &mut toast, &mut cam_q, None, false, None, None, None,
    );
}

#[test]
fn test_phase6_hotkeys_and_moon_orbit_edits() {
    let (mut app, moon) = setup_hotkey_and_moon_test_app();

    // 1. `[` / `]` must adjust sea level WITHOUT changing TimeWarp or clobbering climate temp!
    let start_mult = app.world().resource::<TimeWarp>().multiplier;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::BracketRight);
    app.update();
    assert_eq!(app.world().resource::<TimeWarp>().multiplier, start_mult);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset(KeyCode::BracketRight);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::BracketRight);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyO);
    run_selected_body_tools_once(&mut app);

    let moon_climate = app.world().get::<PlanetaryClimate>(moon).unwrap();
    assert_eq!(
        moon_climate.surface_temperature_k, 310.0,
        "Sea level adjust must preserve existing surface_temperature_k"
    );
    let new_moon_pos = app.world().get::<SimPosition>(moon).unwrap().0;
    let rel_r = (new_moon_pos - DVec3::new(5.2, 0.0, 0.0)).length();
    assert!(
        (rel_r - 0.0033).abs() < 1e-5,
        "KeyO on moon must expand relative to Jupiter (expected 0.0033 AU, got {rel_r})"
    );

    // 2. Shift+= scales mass WITHOUT scaling TimeWarp, whereas unmodified `=` scales TimeWarp
    {
        let mut kb = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        kb.reset_all();
        kb.press(KeyCode::ShiftLeft);
        kb.press(KeyCode::Equal);
    }
    let before_mass = app.world().get::<Mass>(moon).unwrap().0;
    app.update();
    assert_eq!(
        app.world().resource::<TimeWarp>().multiplier,
        start_mult,
        "Shift+= must NOT alter TimeWarp"
    );
    run_selected_body_tools_once(&mut app);
    let after_mass = app.world().get::<Mass>(moon).unwrap().0;
    assert!(
        (after_mass / before_mass - 1.25).abs() < 1e-6,
        "Shift+= must scale body mass by 1.25x"
    );

    // 3. Unmodified Period steps once WITHOUT changing size_exaggeration; Shift+Period scales size_exaggeration
    {
        let mut kb = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        kb.reset_all();
        kb.press(KeyCode::Period);
    }
    let start_scale = app.world().resource::<SimulationConfig>().size_exaggeration;
    app.update();
    assert!(app.world().resource::<TimeWarp>().step_once);
    run_viewport_hotkeys_once(&mut app);
    assert_eq!(
        app.world().resource::<SimulationConfig>().size_exaggeration,
        start_scale,
        "Unmodified Period must not alter size_exaggeration"
    );

    {
        let mut kb = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        kb.reset_all();
        kb.press(KeyCode::ShiftLeft);
        kb.press(KeyCode::Period);
    }
    run_viewport_hotkeys_once(&mut app);
    assert!(
        app.world().resource::<SimulationConfig>().size_exaggeration > start_scale,
        "Shift+Period must increase size_exaggeration"
    );
}
