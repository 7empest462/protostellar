//! Tests for universal stellar classification, hypothetical star types (Quark Stars,
//! Dark Stars, Boson Stars, Iron Stars, Thorne-Żytkow Objects), and organic formation pathways.

use bevy::prelude::*;

use protostellar::simulation::components::*;
use protostellar::simulation::thermodynamics::stellar_lifecycle::*;
use protostellar::simulation::SimulationPlugin;
use protostellar::utils::constants::*;

#[test]
fn test_morgan_keenan_mass_classification() {
    let comp = Composition::solar_gas();

    // M-Dwarf (< 0.45 M_sun)
    let m_star = classify_body_by_mass_and_comp(0.25, &comp, true);
    assert_eq!(m_star, BodyType::RedDwarf);

    // K-Dwarf (0.45 - 0.80 M_sun)
    let k_star = classify_body_by_mass_and_comp(0.65, &comp, true);
    assert_eq!(k_star, BodyType::OrangeDwarf);

    // G-Dwarf (0.80 - 1.04 M_sun)
    let g_star = classify_body_by_mass_and_comp(1.00, &comp, true);
    assert_eq!(g_star, BodyType::YellowDwarf);

    // F-Dwarf (1.04 - 1.40 M_sun)
    let f_star = classify_body_by_mass_and_comp(1.25, &comp, true);
    assert_eq!(f_star, BodyType::YellowWhiteDwarf);

    // A-Dwarf (1.40 - 2.10 M_sun)
    let a_star = classify_body_by_mass_and_comp(1.80, &comp, true);
    assert_eq!(a_star, BodyType::WhiteMainSequence);

    // B-Giant (2.10 - 8.0 M_sun)
    let b_giant = classify_body_by_mass_and_comp(5.50, &comp, true);
    assert_eq!(b_giant, BodyType::BlueGiant);

    // O-Supergiant (8.0 - 25.0 M_sun)
    let o_star = classify_body_by_mass_and_comp(20.0, &comp, true);
    assert_eq!(o_star, BodyType::BlueSupergiant);

    // Pristine zero-metallicity Pop-III Hypergiant
    let pristine_comp = Composition::pure_hydrogen();
    let pop3_star = classify_body_by_mass_and_comp(100.0, &pristine_comp, true);
    assert_eq!(pop3_star, BodyType::PopulationIIIStar);
}

struct MergerBodySpec {
    name: &'static str,
    body_type: BodyType,
    mass: f64,
    radius: f64,
    x_pos: f64,
    temp: f64,
    comp: Composition,
}

fn run_stellar_merger_simulation(
    primary: MergerBodySpec,
    secondary: MergerBodySpec,
) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_plugins(SimulationPlugin);

    // Clear default protoplanetary disk entities
    app.update();
    {
        let entities: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<CelestialBody>>()
            .iter(app.world())
            .collect();
        for ent in entities {
            if let Ok(mut cmd) = app.world_mut().commands().get_entity(ent) {
                cmd.despawn();
            }
        }
    }
    app.update();

    let e1 = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: primary.name.to_string(),
                body_type: primary.body_type,
            },
            Mass(primary.mass),
            Radius(primary.radius),
            SimPosition(bevy::math::DVec3::new(primary.x_pos, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::ZERO),
            SimAcceleration(bevy::math::DVec3::ZERO),
            Temperature(primary.temp),
            primary.comp,
        ))
        .id();

    let e2 = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: secondary.name.to_string(),
                body_type: secondary.body_type,
            },
            Mass(secondary.mass),
            Radius(secondary.radius),
            SimPosition(bevy::math::DVec3::new(secondary.x_pos, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::ZERO),
            SimAcceleration(bevy::math::DVec3::ZERO),
            Temperature(secondary.temp),
            secondary.comp,
        ))
        .id();

    app.update();
    (app, e1, e2)
}

#[test]
fn test_organic_thorne_zytkow_formation() {
    let (app, rsg, ns) = run_stellar_merger_simulation(
        MergerBodySpec {
            name: "Betelgeuse Progenitor",
            body_type: BodyType::RedSupergiant,
            mass: 12.0,
            radius: 3.5,
            x_pos: 0.0,
            temp: 3400.0,
            comp: Composition::solar_gas(),
        },
        MergerBodySpec {
            name: "Infalling Neutron Star",
            body_type: BodyType::NeutronStar,
            mass: 1.4,
            radius: 0.0001,
            x_pos: 0.05,
            temp: 1_000_000.0,
            comp: Composition::metal_rich(),
        },
    );

    // Secondary must have merged away
    assert!(
        app.world().get_entity(ns).is_err(),
        "Neutron star must be engulfed by red supergiant envelope"
    );

    // Primary must have metamorphosed into a Thorne-Żytkow Object
    let primary_body = app
        .world()
        .get::<CelestialBody>(rsg)
        .expect("Primary exists");
    assert_eq!(
        primary_body.body_type,
        BodyType::ThorneZytkow,
        "Red supergiant swallowing neutron star core must form Thorne-Żytkow Object"
    );
    assert!(primary_body.name.contains("Thorne-Żytkow"));
}

#[test]
fn test_organic_quark_star_and_black_hole_formation() {
    // 1. Sub-critical double neutron star merger (1.1 M_sun + 0.8 M_sun = 1.9 M_sun <= 2.17 M_sun TOV)
    let (app, ns1, _ns2) = run_stellar_merger_simulation(
        MergerBodySpec {
            name: "Neutron Star Alpha",
            body_type: BodyType::NeutronStar,
            mass: 1.1,
            radius: 0.0001,
            x_pos: 0.0,
            temp: 1_000_000.0,
            comp: Composition::metal_rich(),
        },
        MergerBodySpec {
            name: "Neutron Star Beta",
            body_type: BodyType::Pulsar,
            mass: 0.8,
            radius: 0.0001,
            x_pos: 0.00005,
            temp: 1_000_000.0,
            comp: Composition::metal_rich(),
        },
    );

    let merged_body = app
        .world()
        .get::<CelestialBody>(ns1)
        .expect("Primary exists");
    assert_eq!(
        merged_body.body_type,
        BodyType::QuarkStar,
        "Sub-critical double neutron star merger must form Strange Quark Star"
    );
    assert!(merged_body.name.contains("Quark Star"));

    // 2. Super-critical double neutron star merger exceeding TOV limit -> Black Hole
    let (bh_app, bh_primary, _) = run_stellar_merger_simulation(
        MergerBodySpec {
            name: "Heavy Neutron Star A",
            body_type: BodyType::NeutronStar,
            mass: 1.5,
            radius: 0.0001,
            x_pos: 0.0,
            temp: 1_000_000.0,
            comp: Composition::metal_rich(),
        },
        MergerBodySpec {
            name: "Heavy Neutron Star B",
            body_type: BodyType::Pulsar,
            mass: 1.3,
            radius: 0.0001,
            x_pos: 0.00005,
            temp: 1_000_000.0,
            comp: Composition::metal_rich(),
        },
    );
    let collapsed = bh_app
        .world()
        .get::<CelestialBody>(bh_primary)
        .expect("Collapsed remnant exists");
    assert_eq!(collapsed.body_type, BodyType::BlackHole);
}

#[test]
fn test_organic_blue_straggler_formation() {
    // Two older main-sequence stars colliding in a cluster
    let (app, star1, _star2) = run_stellar_merger_simulation(
        MergerBodySpec {
            name: "Cluster Star A",
            body_type: BodyType::OrangeDwarf,
            mass: 0.75,
            radius: 0.0035,
            x_pos: 0.0,
            temp: 4500.0,
            comp: Composition::solar_gas(),
        },
        MergerBodySpec {
            name: "Cluster Star B",
            body_type: BodyType::RedDwarf,
            mass: 0.40,
            radius: 0.0020,
            x_pos: 0.001,
            temp: 3200.0,
            comp: Composition::solar_gas(),
        },
    );

    let merged_star = app
        .world()
        .get::<CelestialBody>(star1)
        .expect("Merged exists");
    assert_eq!(
        merged_star.body_type,
        BodyType::BlueStraggler,
        "Collision between two main sequence stars in cluster must form Blue Straggler"
    );
    assert!(merged_star.name.contains("Blue Straggler"));
}

#[test]
fn test_black_dwarf_cooling_and_wolf_rayet_evolution() {
    let mut app = App::new();
    app.add_message::<SupernovaEvent>();
    let dummy_entity = app.world_mut().spawn_empty().id();
    let mut system_state: bevy::ecs::system::SystemState<MessageWriter<SupernovaEvent>> =
        bevy::ecs::system::SystemState::new(app.world_mut());
    let mut supernova_events = system_state
        .get_mut(app.world_mut())
        .expect("MessageWriter valid");

    // 1. Test Black Dwarf cooling below 100 K
    let mut mass = Mass(0.85);
    let mut radius = Radius(0.009 * SOLAR_RADIUS_AU);
    let mut temp = Temperature(85.0); // Dropped below 100 K
    let mut lum = Luminosity(0.001);
    let mut body = CelestialBody {
        name: "Old White Dwarf".to_string(),
        body_type: BodyType::WhiteDwarf,
    };
    let mut evo = StellarEvolutionState {
        phase: StellarEvolutionPhase::WhiteDwarf,
        ..default()
    };

    // Step stellar remnant cooling
    step_stellar_evolution_cycle(
        dummy_entity,
        &mut mass,
        &mut radius,
        &mut temp,
        &mut lum,
        &mut body,
        &mut IgnitionState::default(),
        &mut evo,
        100.0,
        &mut supernova_events,
        false,
    );

    assert_eq!(
        body.body_type,
        BodyType::BlackDwarf,
        "White dwarf cooling below 100 K must crystallize into Black Dwarf"
    );
    assert_eq!(lum.0, 0.0, "Black dwarf emits zero luminosity");

    // 2. Test Wolf-Rayet envelope stripping for massive stars
    let mut wr_mass = Mass(35.0);
    let mut wr_radius = Radius(0.05);
    let mut wr_temp = Temperature(35000.0);
    let mut wr_lum = Luminosity(150_000.0);
    let mut wr_body = CelestialBody {
        name: "Massive O-Star".to_string(),
        body_type: BodyType::BlueSupergiant,
    };
    let mut wr_evo = StellarEvolutionState {
        phase: StellarEvolutionPhase::MainSequence,
        hydrogen_core_fraction: 0.15, // Envelope stripped below 0.25
        ..default()
    };

    step_stellar_evolution_cycle(
        dummy_entity,
        &mut wr_mass,
        &mut wr_radius,
        &mut wr_temp,
        &mut wr_lum,
        &mut wr_body,
        &mut IgnitionState {
            is_ignited: true,
            ..default()
        },
        &mut wr_evo,
        50.0,
        &mut supernova_events,
        false,
    );

    assert_eq!(
        wr_body.body_type,
        BodyType::WolfRayet,
        "Massive star with stripped hydrogen envelope must transition into Wolf-Rayet"
    );
}

#[test]
fn test_all_star_types_have_distinct_wgsl_photosphere_subtypes() {
    use protostellar::rendering::bodies::palettes::star_subtype_from_body_type;

    let expected_subtypes = [
        (BodyType::YellowDwarf, 0.0),
        (BodyType::MainSequenceStar, 0.0),
        (BodyType::RedDwarf, 1.0),
        (BodyType::BrownDwarf, 2.0),
        (BodyType::RedGiant, 3.0),
        (BodyType::RedSupergiant, 3.0),
        (BodyType::BlueMainSequence, 4.0),
        (BodyType::BlueGiant, 4.0),
        (BodyType::BlueSupergiant, 4.0),
        (BodyType::Hypergiant, 4.0),
        (BodyType::NeutronStar, 5.0),
        (BodyType::Pulsar, 6.0),
        (BodyType::Magnetar, 7.0),
        (BodyType::WhiteDwarf, 8.0),
        (BodyType::Protostar, 9.0),
        (BodyType::TTauriStar, 9.0),
        (BodyType::WolfRayet, 10.0),
        (BodyType::DarkStar, 11.0),
        (BodyType::BosonStar, 12.0),
        (BodyType::OrangeDwarf, 13.0),
        (BodyType::YellowWhiteDwarf, 14.0),
        (BodyType::WhiteMainSequence, 15.0),
        (BodyType::HerbigAeBeStar, 16.0),
        (BodyType::YellowHypergiant, 17.0),
        (BodyType::CarbonStar, 18.0),
        (BodyType::LuminousBlueVariable, 19.0),
        (BodyType::PopulationIIIStar, 20.0),
        (BodyType::BlueStraggler, 21.0),
        (BodyType::QuarkStar, 22.0),
        (BodyType::ThorneZytkow, 23.0),
        (BodyType::BlackDwarf, 24.0),
        (BodyType::IronStar, 25.0),
    ];

    for (body_type, expected) in expected_subtypes {
        let actual = star_subtype_from_body_type(body_type);
        assert!(
            (actual - expected).abs() < 1e-5,
            "{body_type:?} expected WGSL subtype {expected}, got {actual}"
        );
        assert!(
            (actual - actual.round()).abs() < 1e-5,
            "{body_type:?} subtype {actual} must be an exact integer to avoid WGSL u32 rounding collisions"
        );
    }

    let wgsl_src = std::fs::read_to_string("assets/shaders/planet.wgsl")
        .expect("Failed to read assets/shaders/planet.wgsl");
    for idx in 0..=25u32 {
        let pattern = format!("subtype == {idx}u");
        assert!(
            wgsl_src.contains(&pattern),
            "planet.wgsl is missing procedural stellar photosphere branch `{pattern}`"
        );
    }
}

#[test]
fn test_circumstellar_environments_and_shader_validity() {
    use protostellar::rendering::bodies::circumstellar::{
        compute_circumstellar_uniforms, resolve_circumstellar_profile,
    };
    use protostellar::rendering::skybox::compute_skybox_lensing_uniforms;
    use protostellar::simulation::resources::SimulationConfig;

    let expected_profiles = [
        (BodyType::LuminousBlueVariable, 0.0, 0.0),
        (BodyType::WolfRayet, 1.0, 0.0),
        (BodyType::CarbonStar, 2.0, 0.0),
        (BodyType::YellowHypergiant, 2.0, 1.0),
        (BodyType::ThorneZytkow, 2.0, 2.0),
        (BodyType::BosonStar, 3.0, 0.0),
        (BodyType::QuarkStar, 3.0, 1.0),
        (BodyType::HerbigAeBeStar, 4.0, 0.0),
        (BodyType::TTauriStar, 4.0, 1.0),
        (BodyType::PopulationIIIStar, 5.0, 0.0),
        (BodyType::DarkStar, 5.0, 1.0),
    ];

    for (body_type, expected_env, expected_sub) in expected_profiles {
        let profile = resolve_circumstellar_profile(body_type)
            .unwrap_or_else(|| panic!("{body_type:?} must have a circumstellar profile"));
        assert!((profile.env_type - expected_env).abs() < 1e-5);
        assert!((profile.sub_mode - expected_sub).abs() < 1e-5);

        let uniforms =
            compute_circumstellar_uniforms(&profile, Vec3::new(1.0, 2.0, 3.0), 2.5, None, 12.34);
        assert!((uniforms.mode_and_time.x - 2.5).abs() < 1e-5);
        assert!(uniforms.center_and_outer_r.w > uniforms.mode_and_time.x);
    }

    assert!(resolve_circumstellar_profile(BodyType::YellowDwarf).is_none());
    assert!(resolve_circumstellar_profile(BodyType::BlackDwarf).is_none());

    // Verify relativistic skybox lensing for BosonStar (horizonless) and QuarkStar (compact horizon/surface)
    let config = SimulationConfig::default();
    let (bh_pos, bh_params) = compute_skybox_lensing_uniforms(
        Vec3::new(0.0, 0.0, -10.0),
        &config,
        &SimPosition(bevy::math::DVec3::ZERO),
        &Radius(0.01),
        &CelestialBody {
            name: "Solitonic Boson Star".to_string(),
            body_type: BodyType::BosonStar,
        },
        None,
    )
    .expect("BosonStar must return skybox lensing uniforms");
    assert!(
        bh_pos.w > 0.0,
        "BosonStar must activate skybox Einstein ring deflection"
    );
    assert_eq!(
        bh_params.x, 0.0,
        "Horizonless BosonStar must have zero event horizon shadow"
    );

    let (qs_pos, qs_params) = compute_skybox_lensing_uniforms(
        Vec3::new(0.0, 0.0, -10.0),
        &config,
        &SimPosition(bevy::math::DVec3::ZERO),
        &Radius(0.01),
        &CelestialBody {
            name: "Strange Quark Star".to_string(),
            body_type: BodyType::QuarkStar,
        },
        None,
    )
    .expect("QuarkStar must return skybox lensing uniforms");
    assert!(
        qs_pos.w > 0.0,
        "QuarkStar must activate skybox gravitational lensing"
    );
    assert!(
        qs_params.x > 0.0,
        "QuarkStar must have compact photon sphere shadow parameter"
    );

    // Validate assets/shaders/circumstellar.wgsl structure
    let wgsl = std::fs::read_to_string("assets/shaders/circumstellar.wgsl")
        .expect("Failed to read assets/shaders/circumstellar.wgsl");
    assert!(wgsl.contains("struct CircumstellarUniforms"));
    for env_idx in 0..=4u32 {
        let branch = format!("env_type == {env_idx}u");
        assert!(
            wgsl.contains(&branch),
            "circumstellar.wgsl missing `{branch}`"
        );
    }
}

#[test]
#[allow(
    clippy::type_complexity,
    reason = "Test coordinates multiple thermodynamic ECS queries, event writers, and commands"
)]
fn test_massive_hypergiant_direct_collapse_imbh_seed_and_no_reignition() {
    let mut app = App::new();
    app.add_message::<SupernovaEvent>();
    app.add_message::<protostellar::simulation::thermodynamics::StarIgnitionEvent>();

    let star_entity = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Protostar Gen-I Pop-III Hypergiant-1 (Red Supergiant)".to_string(),
                body_type: BodyType::RedSupergiant,
            },
            Mass(350.0),
            Radius(0.12),
            Temperature(3300.0),
            Luminosity(5_000_000.0),
            IgnitionState {
                is_ignited: true,
                core_temperature: 8.5e7,
                fusion_fraction: 1.0,
                shockwave_radius: 10.0,
            },
            StellarEvolutionState {
                phase: StellarEvolutionPhase::RedSupergiantBranch,
                phase_timer_years: 700.0,
                ..default()
            },
        ))
        .id();

    let mut state: bevy::ecs::system::SystemState<(
        Commands,
        MessageWriter<SupernovaEvent>,
        MessageWriter<protostellar::simulation::thermodynamics::StarIgnitionEvent>,
        Query<(
            Entity,
            &mut Mass,
            &mut Radius,
            &mut Temperature,
            &mut Luminosity,
            &mut IgnitionState,
            &mut CelestialBody,
            Option<&mut StellarEvolutionState>,
        )>,
    )> = bevy::ecs::system::SystemState::new(app.world_mut());

    let (mut commands, mut supernova_events, mut ignition_events, mut query) =
        state.get_mut(app.world_mut()).expect("SystemState valid");

    let (entity, mut mass, mut radius, mut temp, mut lum, mut ignition, mut body, mut opt_evo) =
        query.get_mut(star_entity).expect("Star exists");

    // 1. Step evolution to trigger supernova / direct collapse
    let evo = opt_evo.as_deref_mut().expect("Evo exists");
    step_stellar_evolution_cycle(
        entity,
        &mut mass,
        &mut radius,
        &mut temp,
        &mut lum,
        &mut body,
        &mut ignition,
        evo,
        50.0,
        &mut supernova_events,
        true, // is_gmc
    );

    // Verify it collapsed into an Intermediate-Mass Black Hole retaining >= 90% of its mass
    assert_eq!(body.body_type, BodyType::BlackHole);
    assert_eq!(evo.phase, StellarEvolutionPhase::SupernovaExplosion);
    assert!(
        mass.0 >= 300.0,
        "Direct collapse hypergiant must retain massive core (>300 M_sun), got {:.1}",
        mass.0
    );
    assert!(
        body.name.starts_with("Intermediate-Mass Black Hole"),
        "Name must be clean IMBH descriptor, got: {}",
        body.name
    );
    assert!(
        !body.name.contains("(Red Supergiant) (BlackHole Remnant)"),
        "Name must not stack historical phase strings, got: {}",
        body.name
    );

    // 2. CRITICAL INVARIANT: Step protostar ignition system on the black hole
    let mut config = protostellar::simulation::resources::SimulationConfig::default();
    step_protostar_ignition_and_limits(
        entity,
        &mut mass,
        &mut radius,
        &mut temp,
        &mut lum,
        &mut ignition,
        &mut body,
        &mut opt_evo,
        None,
        &mut config,
        100.0,
        1500.0,
        &mut commands,
        &mut ignition_events,
        &mut supernova_events,
        true,
    );

    // Verify it is STILL a Black Hole and has not been re-ignited!
    assert_eq!(
        body.body_type,
        BodyType::BlackHole,
        "Black hole must never re-ignite into a main sequence star"
    );
    assert_eq!(
        temp.0, 10.0,
        "Black hole surface temp must remain cold (10 K)"
    );
    assert_eq!(lum.0, 0.0, "Black hole luminosity must remain 0");

    // 3. Step remnant evolution past 1500 years to reach BlackHoleRemnant phase
    let evo2 = opt_evo.as_deref_mut().expect("Evo exists");
    step_stellar_evolution_cycle(
        entity,
        &mut mass,
        &mut radius,
        &mut temp,
        &mut lum,
        &mut body,
        &mut ignition,
        evo2,
        2000.0,
        &mut supernova_events,
        true,
    );

    assert_eq!(evo2.phase, StellarEvolutionPhase::BlackHoleRemnant);
    assert_eq!(body.body_type, BodyType::BlackHole);
}

#[test]
fn test_dynamic_galactic_nucleus_handoff_to_growing_black_hole() {
    use protostellar::simulation::physics::enforce_dynamic_galactic_nucleus;
    use protostellar::simulation::physics::PhysicsBodyEntry;

    let mut app = App::new();
    let initial_central_ent = app.world_mut().spawn_empty().id();
    let growing_bh_ent = app.world_mut().spawn_empty().id();
    let companion_ent = app.world_mut().spawn_empty().id();

    // Initial GMC cluster state:
    // Body 0: Initial central protostar (3.5 M_sun) at DVec3::ZERO
    // Body 1: Pop-III star / Black Hole that accreted to 180 M_sun at offset (12.0, 0.0, 5.0)
    // Body 2: Companion star (1.5 M_sun) at offset (25.0, 0.0, 0.0)
    let bh_offset = bevy::math::DVec3::new(12.0, 0.0, 5.0);
    let bh_vel = bevy::math::DVec3::new(0.5, 0.0, -0.2);

    let mut body_data = vec![
        PhysicsBodyEntry {
            entity: initial_central_ent,
            mass: 3.5,
            pos: bevy::math::DVec3::ZERO,
            vel: bevy::math::DVec3::ZERO,
            acc: bevy::math::DVec3::ZERO,
            radius: 0.05,
            body_type: BodyType::Protostar,
            satellite: None,
            is_central_star: true,
            name: "Protostar Alpha".to_string(),
        },
        PhysicsBodyEntry {
            entity: growing_bh_ent,
            mass: 180.0,
            pos: bh_offset,
            vel: bh_vel,
            acc: bevy::math::DVec3::ZERO,
            radius: 0.001,
            body_type: BodyType::BlackHole,
            satellite: None,
            is_central_star: false,
            name: "Intermediate-Mass Black Hole (180.0 M☉)".to_string(),
        },
        PhysicsBodyEntry {
            entity: companion_ent,
            mass: 1.5,
            pos: bevy::math::DVec3::new(25.0, 0.0, 0.0),
            vel: bevy::math::DVec3::new(0.0, 0.0, 1.2),
            acc: bevy::math::DVec3::ZERO,
            radius: 0.02,
            body_type: BodyType::MainSequenceStar,
            satellite: None,
            is_central_star: false,
            name: "Companion Star Beta".to_string(),
        },
    ];

    let mut state: bevy::ecs::system::SystemState<Commands> =
        bevy::ecs::system::SystemState::new(app.world_mut());
    let mut commands = state.get_mut(app.world_mut()).expect("Commands valid");

    // Execute galactic nucleus dynamic enforcement for GMC cluster
    enforce_dynamic_galactic_nucleus(&mut body_data, true, &mut commands);

    // Verify handoff:
    // 1. Initial 3.5 M_sun star lost is_central_star
    assert!(
        !body_data[0].is_central_star,
        "Initial 3.5 M_sun star must relinquish central star role"
    );

    // 2. 180 M_sun Black Hole gained is_central_star
    assert!(
        body_data[1].is_central_star,
        "180 M_sun Black Hole must become the new Central Star"
    );

    // 3. Cluster coordinate origin smoothly translated so the Black Hole is at DVec3::ZERO
    assert!(
        body_data[1].pos.length() < 1e-10,
        "New central black hole position must be centered at (0, 0, 0)"
    );
    assert!(
        body_data[1].vel.length() < 1e-10,
        "New central black hole velocity must be centered at (0, 0, 0)"
    );

    // 4. Other bodies were shifted relative to the new anchor
    let expected_old_star_pos = -bh_offset;
    assert!(
        (body_data[0].pos - expected_old_star_pos).length() < 1e-10,
        "Old central star position must be shifted by -bh_offset"
    );
}

#[test]
#[allow(
    clippy::type_complexity,
    reason = "Test verifies complex GMC GPU-to-CPU collapse system parameters and queries"
)]
fn test_gen1_primordial_pop_iii_hypergiant_spawning_masses() {
    use protostellar::gpu::gmc_collapse::receive_gmc_collapse_events;
    use protostellar::gpu::gmc_fluid::{GmcCollapseEventReceiver, GpuJeansCollapseEvent};
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    let (tx, rx) = flume::unbounded();
    app.insert_resource(GmcCollapseEventReceiver { rx });
    app.insert_resource(ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        scenario_time_years: 1.0, // Past initial relaxation threshold
        ..default()
    });

    // Spawn 18 dummy stars far away so star_count becomes 19 upon collapse (Pop-III hypergiant slot 19)
    for i in 0..18 {
        app.world_mut().spawn((
            SimPosition(bevy::math::DVec3::new(
                0.0,
                f64::from(i) * 10.0 + 200.0,
                0.0,
            )),
            CelestialBody {
                name: format!("Cluster Star {i}"),
                body_type: BodyType::Protostar,
            },
        ));
    }

    // Send a Gen-1 zero-metallicity collapse event in slot 19 (at r = 100 AU, avoiding the r < 45 Dark Star zone)
    let ev = GpuJeansCollapseEvent {
        grid_coords: [48, 48, 48],
        metallicity: 0.0001, // Gen-1 pristine zero-metallicity
        world_pos: [60.0, 0.0, 80.0],
        local_mass_solar: 3.5,
        com_velocity: [0.1, 0.0, 0.1],
        temperature_k: 15.0,
    };
    tx.send(vec![ev]).unwrap();

    let mut state: bevy::ecs::system::SystemState<(
        Commands,
        Res<GmcCollapseEventReceiver>,
        Option<Res<ActiveScenarioState>>,
        Query<(&SimPosition, &CelestialBody, Option<&Mass>)>,
        Option<ResMut<protostellar::game::ui::NotificationToast>>,
    )> = bevy::ecs::system::SystemState::new(app.world_mut());

    let (commands, receiver, scenario_state, existing_bodies, toast) =
        state.get_mut(app.world_mut()).expect("State valid");

    receive_gmc_collapse_events(commands, receiver, scenario_state, existing_bodies, toast);
    state.apply(app.world_mut());

    // Query spawned celestial bodies to find the Pop-III hypergiant
    let mut star_query = app
        .world_mut()
        .query::<(&CelestialBody, &Mass, &Composition)>();
    let hypergiant = star_query
        .iter(app.world())
        .find(|(b, _, _)| b.name.contains("Pop-III"));

    assert!(
        hypergiant.is_some(),
        "Gen-1 collapse slot 19 must spawn a Gen-I Pop-III hypergiant"
    );
    let (body, mass, comp) = hypergiant.unwrap();
    assert!(
        mass.0 >= 75.0,
        "Pop-III hypergiant mass must spawn with large primordial mass (>= 75 M_sun), got: {:.1}",
        mass.0
    );
    assert_eq!(
        comp.metal_frac, 0.0,
        "Pop-III star must have 0% metal fraction"
    );
    assert!(
        body.name.contains("Pop-III Hypergiant"),
        "Body name must indicate Pop-III Hypergiant, got: {}",
        body.name
    );
}
