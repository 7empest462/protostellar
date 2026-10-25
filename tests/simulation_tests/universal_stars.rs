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
