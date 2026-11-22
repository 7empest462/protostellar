//! Regression tests for dynamic star aging (`Age [N]`), stellar mass scaling,
//! and red-giant planetary engulfment stability without false crater ejecta.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::game::interaction::{SelectedBodyQuery, handle_selected_body_tools_with_events};
use protostellar::game::ui::interactions::editor::orbital_params::handle_mass_change_with_effects;
use protostellar::game::ui::interactions::editor::stellar::handle_age_star_with_effects;
use protostellar::game::ui::types::{NotificationToast, SelectedWorldQuery, UiButtonAction};
use protostellar::game::ui::widgets::HudSliderKind;
use protostellar::rendering::effects::impact_debris::update_impact_ejecta;
use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
use protostellar::simulation::accretion::events::{
    AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
};
use protostellar::simulation::components::*;
use protostellar::simulation::relativity::kerr::KerrBlackHole;
use protostellar::simulation::resources::*;
use protostellar::simulation::terraforming::BombardmentEvent;
use protostellar::simulation::thermodynamics::{StarIgnitionEvent, update_thermodynamics};
use protostellar::utils::constants::*;

fn build_star_aging_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .init_resource::<NotificationToast>()
        .init_resource::<ImpactEjectaPool>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<MoonFormationEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_message::<BombardmentEvent>()
        .add_message::<StarIgnitionEvent>()
        .add_message::<PlanetaryEngulfmentEvent>()
        .add_message::<SupernovaEvent>()
        .add_systems(
            Update,
            (
                process_accretion_and_collisions,
                update_thermodynamics,
                update_impact_ejecta,
            )
                .chain(),
        );
    app
}

fn spawn_test_star(
    app: &mut App,
    name: &str,
    body_type: BodyType,
    mass_solar: f64,
    radius_au: f64,
    temp_k: f64,
    is_ignited: bool,
    phase: StellarEvolutionPhase,
) -> Entity {
    app.world_mut()
        .spawn((
            CelestialBody {
                name: name.to_string(),
                body_type,
            },
            CentralStar,
            Mass(mass_solar),
            Radius(radius_au),
            Temperature(temp_k),
            Luminosity(mass_solar.max(0.01).powf(3.5)),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration(DVec3::ZERO),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: if is_ignited { 1.2e7 } else { 4.0e6 },
                fusion_fraction: if is_ignited { 1.0 } else { 0.0 },
                is_ignited,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState {
                phase,
                hydrogen_core_fraction: if phase == StellarEvolutionPhase::MainSequence {
                    1.0
                } else {
                    0.0
                },
                ..default()
            },
        ))
        .id()
}

fn run_age_click_on_star(app: &mut App, star_ent: Entity) -> String {
    let mut sys: bevy::ecs::system::SystemState<(
        SelectedWorldQuery,
        Commands,
        MessageWriter<SupernovaEvent>,
        ResMut<NotificationToast>,
    )> = bevy::ecs::system::SystemState::new(app.world_mut());

    let msg = {
        let (mut query, mut commands, mut sn_writer, mut toast) =
            sys.get_mut(app.world_mut()).unwrap();
        handle_age_star_with_effects(
            &mut query,
            Some(star_ent),
            Some(&mut commands),
            Some(&mut sn_writer),
            &mut toast,
        );
        toast.message.clone()
    };
    sys.apply(app.world_mut());
    msg
}

#[test]
fn test_solar_red_giant_engulfment_never_reverts_or_craters() {
    let mut app = build_star_aging_test_app();
    let star = spawn_test_star(
        &mut app,
        "The Protostar (Solar Nebula)",
        BodyType::Protostar,
        1.0,
        SOLAR_RADIUS_AU,
        4500.0,
        false,
        StellarEvolutionPhase::ProtostarContraction,
    );

    // Spawn inner rocky planets inside the future 1.25 AU Red Giant envelope
    for (name, r_au) in [("Proto-Mercury", 0.40), ("Proto-Venus", 0.72)] {
        app.world_mut().spawn((
            CelestialBody {
                name: name.to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(0.5 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            Temperature(450.0),
            SimPosition(DVec3::new(r_au, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, (G_ASTRO / r_au).sqrt())),
            SimAcceleration(DVec3::ZERO),
            Composition::rocky(),
        ));
    }

    // 1. Protostar -> Main Sequence (Yellow Dwarf G2V)
    run_age_click_on_star(&mut app, star);
    let body = app.world().get::<CelestialBody>(star).unwrap();
    let evo = app.world().get::<StellarEvolutionState>(star).unwrap();
    assert_eq!(body.body_type, BodyType::YellowDwarf);
    assert_eq!(evo.phase, StellarEvolutionPhase::MainSequence);
    assert!(app.world().get::<IgnitionState>(star).unwrap().is_ignited);

    // 2. Main Sequence -> Red Giant Branch (must NOT revert to MainSequence upon engulfing inner planets)
    run_age_click_on_star(&mut app, star);
    assert_eq!(
        app.world().get::<CelestialBody>(star).unwrap().body_type,
        BodyType::RedGiant
    );
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star)
            .unwrap()
            .phase,
        StellarEvolutionPhase::RedGiantBranch
    );

    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        app.world().get::<CelestialBody>(star).unwrap().body_type,
        BodyType::RedGiant,
        "Red Giant engulfing Mercury/Venus must not revert to Main Sequence!"
    );
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star)
            .unwrap()
            .phase,
        StellarEvolutionPhase::RedGiantBranch,
        "Red Giant phase must remain RedGiantBranch after engulfment"
    );
    assert!(
        app.world().get::<Radius>(star).unwrap().0 >= 1.1,
        "Red Giant envelope radius must not collapse back to 0.00465 AU!"
    );
    assert!(
        app.world()
            .resource::<ImpactEjectaPool>()
            .fragments
            .is_empty(),
        "Star engulfing inner planets must not spawn planetary crater ejecta boulders!"
    );
}

#[test]
fn test_solar_agb_to_nebula_white_and_black_dwarf() {
    let mut app = build_star_aging_test_app();
    let star = spawn_test_star(
        &mut app,
        "The Star (Red Giant Branch)",
        BodyType::RedGiant,
        1.0,
        1.25,
        3100.0,
        true,
        StellarEvolutionPhase::RedGiantBranch,
    );

    // 1. Red Giant Branch -> Helium Flash / AGB
    run_age_click_on_star(&mut app, star);
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star)
            .unwrap()
            .phase,
        StellarEvolutionPhase::HeliumFlashAgb
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star)
            .unwrap()
            .phase,
        StellarEvolutionPhase::HeliumFlashAgb
    );

    // 2. Helium Flash AGB -> Planetary Nebula Ejection (IFMR ~ 0.55 M_sun, exposed hot UV core)
    run_age_click_on_star(&mut app, star);
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star)
            .unwrap()
            .phase,
        StellarEvolutionPhase::PlanetaryNebulaEjection
    );
    assert_eq!(
        app.world().get::<CelestialBody>(star).unwrap().body_type,
        BodyType::WhiteDwarf,
        "Exposed Planetary Nebula Core must render as a hot UV WhiteDwarf core, not a shrunken RedGiant!"
    );
    assert!(
        app.world().get::<Temperature>(star).unwrap().0 >= 50_000.0,
        "Planetary Nebula UV core should reach ~65,000 K"
    );
    let nebula_mass = app.world().get::<Mass>(star).unwrap().0;
    assert!(
        (nebula_mass - 0.55).abs() < 0.02,
        "1 M_sun star should shed to ~0.55 M_sun IFMR WD mass, got {nebula_mass}"
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star)
            .unwrap()
            .phase,
        StellarEvolutionPhase::PlanetaryNebulaEjection,
        "Planetary Nebula must not re-ignite back to Main Sequence!"
    );

    // 3. Planetary Nebula Ejection -> White Dwarf
    run_age_click_on_star(&mut app, star);
    assert_eq!(
        app.world().get::<CelestialBody>(star).unwrap().body_type,
        BodyType::WhiteDwarf
    );
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star)
            .unwrap()
            .phase,
        StellarEvolutionPhase::WhiteDwarf
    );

    // 4. White Dwarf -> Black Dwarf -> Terminal State
    run_age_click_on_star(&mut app, star);
    assert_eq!(
        app.world().get::<CelestialBody>(star).unwrap().body_type,
        BodyType::BlackDwarf
    );
    assert_eq!(app.world().get::<Luminosity>(star).unwrap().0, 0.0);

    let terminal_msg = run_age_click_on_star(&mut app, star);
    assert_eq!(
        app.world().get::<CelestialBody>(star).unwrap().body_type,
        BodyType::BlackDwarf
    );
    assert!(terminal_msg.contains("Terminal State"));
}

#[test]
fn test_massive_star_supernova_and_red_giant_mass_escalation() {
    // Part A: 12 M_sun MainSequence -> RedSupergiant -> Type II Supernova -> Pulsar
    let mut app = build_star_aging_test_app();
    let star_12m = spawn_test_star(
        &mut app,
        "The Star (Blue Supergiant - O Type)",
        BodyType::BlueSupergiant,
        12.0,
        SOLAR_RADIUS_AU * 12.0_f64.powf(0.8),
        20_000.0,
        true,
        StellarEvolutionPhase::MainSequence,
    );

    run_age_click_on_star(&mut app, star_12m);
    assert_eq!(
        app.world()
            .get::<CelestialBody>(star_12m)
            .unwrap()
            .body_type,
        BodyType::RedSupergiant
    );
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star_12m)
            .unwrap()
            .phase,
        StellarEvolutionPhase::RedSupergiantBranch
    );

    run_age_click_on_star(&mut app, star_12m);
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star_12m)
            .unwrap()
            .phase,
        StellarEvolutionPhase::SupernovaExplosion
    );
    assert_eq!(
        app.world()
            .get::<CelestialBody>(star_12m)
            .unwrap()
            .body_type,
        BodyType::Pulsar
    );
    let sn_count = app.world().resource::<Messages<SupernovaEvent>>().len();
    assert_eq!(sn_count, 1, "Age on RedSupergiant must emit SupernovaEvent");

    run_age_click_on_star(&mut app, star_12m);
    assert_eq!(
        app.world()
            .get::<StellarEvolutionState>(star_12m)
            .unwrap()
            .phase,
        StellarEvolutionPhase::NeutronStarPulsar
    );
    assert!(
        app.world()
            .entity(star_12m)
            .get::<ElectromagneticFieldState>()
            .is_some()
    );

    // Part B: 1 M_sun RedGiant raised above 8 M_sun via IncreaseMass promotes to RedSupergiant
    // keeping giant radius, and then Age goes straight to SupernovaExplosion!
    let mut app_rg = build_star_aging_test_app();
    let star_rg = spawn_test_star(
        &mut app_rg,
        "The Star (Red Giant Branch)",
        BodyType::RedGiant,
        1.0,
        1.25,
        3100.0,
        true,
        StellarEvolutionPhase::RedGiantBranch,
    );
    {
        let mut sys: bevy::ecs::system::SystemState<(
            SelectedWorldQuery,
            ResMut<PlayerInteractionState>,
            ResMut<NotificationToast>,
        )> = bevy::ecs::system::SystemState::new(app_rg.world_mut());
        let (mut query, mut player, mut toast) = sys.get_mut(app_rg.world_mut()).unwrap();
        player.selected_entity = Some(star_rg);
        for _ in 0..10 {
            handle_mass_change_with_effects(
                &UiButtonAction::IncreaseMass,
                &mut query,
                None,
                None,
                &player,
                &mut toast,
            );
        }
    }
    let escalated_mass = app_rg.world().get::<Mass>(star_rg).unwrap().0;
    assert!(
        escalated_mass >= 8.0,
        "Expected >= 8 M_sun, got {escalated_mass}"
    );
    assert_eq!(
        app_rg
            .world()
            .get::<CelestialBody>(star_rg)
            .unwrap()
            .body_type,
        BodyType::RedSupergiant
    );
    assert!(
        app_rg.world().get::<Radius>(star_rg).unwrap().0 >= 1.8,
        "Increasing RedGiant mass must preserve giant envelope instead of resetting to MS!"
    );

    run_age_click_on_star(&mut app_rg, star_rg);
    assert_eq!(
        app_rg
            .world()
            .get::<StellarEvolutionState>(star_rg)
            .unwrap()
            .phase,
        StellarEvolutionPhase::SupernovaExplosion
    );
    assert_eq!(
        app_rg.world().resource::<Messages<SupernovaEvent>>().len(),
        1
    );
}

#[test]
fn test_hypergiant_hypernova_kerr_black_hole_aging() {
    let mut app_bh = build_star_aging_test_app();
    let star_35m = spawn_test_star(
        &mut app_bh,
        "The Star (Luminous Hypergiant)",
        BodyType::Hypergiant,
        35.0,
        SOLAR_RADIUS_AU * 35.0_f64.powf(0.8),
        35_000.0,
        true,
        StellarEvolutionPhase::MainSequence,
    );
    run_age_click_on_star(&mut app_bh, star_35m);
    assert_eq!(
        app_bh
            .world()
            .get::<StellarEvolutionState>(star_35m)
            .unwrap()
            .phase,
        StellarEvolutionPhase::RedSupergiantBranch
    );
    run_age_click_on_star(&mut app_bh, star_35m);
    assert_eq!(
        app_bh
            .world()
            .get::<StellarEvolutionState>(star_35m)
            .unwrap()
            .phase,
        StellarEvolutionPhase::SupernovaExplosion
    );
    assert_eq!(
        app_bh
            .world()
            .get::<CelestialBody>(star_35m)
            .unwrap()
            .body_type,
        BodyType::BlackHole
    );
    run_age_click_on_star(&mut app_bh, star_35m);
    assert_eq!(
        app_bh
            .world()
            .get::<StellarEvolutionState>(star_35m)
            .unwrap()
            .phase,
        StellarEvolutionPhase::BlackHoleRemnant
    );
    assert!(
        app_bh
            .world()
            .entity(star_35m)
            .get::<KerrBlackHole>()
            .is_some()
    );
}

#[test]
fn test_remnant_limit_aging_low_mass_mdwarf_hotkey_n_and_slider_range() {
    // 1. InspectorLogMassEarth slider supports high-mass supernova progenitors (>= 25 M_sun)
    let max_log_m_earth = HudSliderKind::InspectorLogMassEarth.max();
    assert!(max_log_m_earth >= 8.0);
    let max_solar = (10.0_f64.powf(f64::from(max_log_m_earth))) * EARTH_MASS_SOLAR;
    assert!(
        max_solar >= 250.0,
        "Mass slider max must reach supermassive stars (>= 250 M_sun), got {max_solar}"
    );

    // 2. White Dwarf > 1.44 M_sun -> Type Ia Supernova -> Pulsar -> QuarkStar -> BlackHole
    let mut app = build_star_aging_test_app();
    let wd = spawn_test_star(
        &mut app,
        "The Star (White Dwarf Remnant)",
        BodyType::WhiteDwarf,
        1.55,
        0.009 * SOLAR_RADIUS_AU,
        30_000.0,
        false,
        StellarEvolutionPhase::WhiteDwarf,
    );
    let msg_ia = run_age_click_on_star(&mut app, wd);
    assert!(msg_ia.contains("TYPE IA"), "Got: {msg_ia}");
    assert_eq!(
        app.world().get::<CelestialBody>(wd).unwrap().body_type,
        BodyType::Pulsar
    );
    assert_eq!(app.world().resource::<Messages<SupernovaEvent>>().len(), 1);

    run_age_click_on_star(&mut app, wd);
    assert_eq!(
        app.world().get::<StellarEvolutionState>(wd).unwrap().phase,
        StellarEvolutionPhase::NeutronStarPulsar
    );

    app.world_mut().entity_mut(wd).get_mut::<Mass>().unwrap().0 = 1.95;
    run_age_click_on_star(&mut app, wd);
    assert_eq!(
        app.world().get::<CelestialBody>(wd).unwrap().body_type,
        BodyType::QuarkStar
    );

    app.world_mut().entity_mut(wd).get_mut::<Mass>().unwrap().0 = 2.50;
    run_age_click_on_star(&mut app, wd);
    assert_eq!(
        app.world().get::<CelestialBody>(wd).unwrap().body_type,
        BodyType::BlackHole
    );
    assert!(app.world().entity(wd).get::<KerrBlackHole>().is_some());

    // 3. Low-mass 0.30 M_sun Red Dwarf skips RedGiant -> Helium White Dwarf -> Black Dwarf
    let mut app_rd = build_star_aging_test_app();
    let rd = spawn_test_star(
        &mut app_rd,
        "The Star (Red Dwarf - M Type)",
        BodyType::RedDwarf,
        0.30,
        SOLAR_RADIUS_AU * 0.30_f64.powf(0.8),
        3300.0,
        true,
        StellarEvolutionPhase::MainSequence,
    );
    run_age_click_on_star(&mut app_rd, rd);
    assert_eq!(
        app_rd.world().get::<CelestialBody>(rd).unwrap().body_type,
        BodyType::WhiteDwarf
    );

    // 4. Keyboard [N] hotkey advances an ignited MainSequence star just like Age button
    let mut app_key = build_star_aging_test_app();
    let key_star = spawn_test_star(
        &mut app_key,
        "The Star (Yellow Dwarf - G2V)",
        BodyType::YellowDwarf,
        1.0,
        SOLAR_RADIUS_AU,
        5778.0,
        true,
        StellarEvolutionPhase::MainSequence,
    );
    {
        let mut kb = ButtonInput::<KeyCode>::default();
        kb.press(KeyCode::KeyN);
        let mut sys: bevy::ecs::system::SystemState<(
            Commands,
            SelectedBodyQuery,
            ResMut<PlayerInteractionState>,
            ResMut<NotificationToast>,
            MessageWriter<SupernovaEvent>,
        )> = bevy::ecs::system::SystemState::new(app_key.world_mut());
        let (mut commands, mut sel_q, mut player, mut toast, mut sn_writer) =
            sys.get_mut(app_key.world_mut()).unwrap();
        player.selected_entity = Some(key_star);
        handle_selected_body_tools_with_events(
            &mut commands,
            &kb,
            1.0,
            DVec3::ZERO,
            DVec3::ZERO,
            &mut player,
            &mut sel_q,
            &mut toast,
            Some(&mut sn_writer),
        );
        sys.apply(app_key.world_mut());
    }
    assert_eq!(
        app_key
            .world()
            .get::<CelestialBody>(key_star)
            .unwrap()
            .body_type,
        BodyType::RedGiant,
        "Pressing [N] on an ignited star must advance it to RedGiant"
    );
}

#[test]
fn test_age_button_visibility_across_remnant_stages() {
    use protostellar::game::ui::update_scenario_contextual_ui;

    let mut app = App::new();
    app.init_resource::<PlayerInteractionState>();
    app.add_systems(Update, update_scenario_contextual_ui);

    let age_btn = app
        .world_mut()
        .spawn((UiButtonAction::AgeStar, Node::default()))
        .id();
    let ignite_btn = app
        .world_mut()
        .spawn((UiButtonAction::IgniteStar, Node::default()))
        .id();

    let wd_star = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "The Star (White Dwarf Remnant)".to_string(),
                body_type: BodyType::WhiteDwarf,
            },
        ))
        .id();
    let pulsar_star = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "The Star (Pulsar Remnant)".to_string(),
                body_type: BodyType::Pulsar,
            },
        ))
        .id();
    let bd_star = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "The Star (Black Dwarf Remnant)".to_string(),
                body_type: BodyType::BlackDwarf,
            },
        ))
        .id();

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(wd_star);
    app.update();
    assert_eq!(
        app.world().get::<Node>(age_btn).unwrap().display,
        Display::Flex,
        "Age button must remain visible on White Dwarf so it can cool to Black Dwarf or detonate Type Ia SN!"
    );
    assert_eq!(
        app.world().get::<Node>(ignite_btn).unwrap().display,
        Display::None
    );

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(pulsar_star);
    app.update();
    assert_eq!(
        app.world().get::<Node>(age_btn).unwrap().display,
        Display::Flex,
        "Age button must remain visible on Pulsar for QuarkStar / TOV Black Hole collapse!"
    );

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(bd_star);
    app.update();
    assert_eq!(
        app.world().get::<Node>(age_btn).unwrap().display,
        Display::None,
        "Age button should hide once star reaches final terminal Black Dwarf state!"
    );
}
