//! Test module generated from simulation_tests.

use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;

#[test]
fn test_biosphere_habitability_index() {
    use protostellar::simulation::components::BiosphereState;

    let temp_score = 0.95f32;
    let water_score = 1.0f32;
    let shield_score = 1.0f32;
    let atm_score = 1.0f32;

    let habitability = temp_score * water_score * shield_score * atm_score;
    assert!(habitability >= 0.90);

    let mut bio = BiosphereState::default();
    assert_eq!(bio.biomass_coverage_frac, 0.0);
    assert_eq!(bio.oxygen_fraction, 0.0);

    // Life blooms over time in high habitability
    bio.habitability_score = habitability;
    bio.biomass_coverage_frac = 0.70;
    bio.oxygen_fraction = (bio.biomass_coverage_frac * 0.24).clamp(0.0, 0.21);

    assert!((bio.oxygen_fraction - 0.168).abs() < 1e-3);
}

#[test]
fn test_stellar_evolution_phase_transitions() {
    use protostellar::simulation::components::{StellarEvolutionPhase, StellarEvolutionState};

    let mut evo = StellarEvolutionState::default();
    assert_eq!(evo.phase, StellarEvolutionPhase::ProtostarContraction);

    // Ignition transition
    evo.phase = StellarEvolutionPhase::MainSequence;
    evo.hydrogen_core_fraction = 1.0;

    // Fuel burning over time
    evo.hydrogen_core_fraction = 0.0;
    if evo.hydrogen_core_fraction <= 0.0 {
        evo.phase = StellarEvolutionPhase::RedGiantBranch;
    }
    assert_eq!(evo.phase, StellarEvolutionPhase::RedGiantBranch);

    // Helium flash & AGB
    evo.helium_core_fraction = 1.0;
    if evo.helium_core_fraction >= 1.0 {
        evo.phase = StellarEvolutionPhase::HeliumFlashAgb;
    }
    assert_eq!(evo.phase, StellarEvolutionPhase::HeliumFlashAgb);

    // Planetary nebula ejection
    evo.phase = StellarEvolutionPhase::PlanetaryNebulaEjection;
    evo.nebula_expansion_radius_au = 85.0;
    if evo.nebula_expansion_radius_au >= 80.0 {
        evo.phase = StellarEvolutionPhase::WhiteDwarf;
    }
    assert_eq!(evo.phase, StellarEvolutionPhase::WhiteDwarf);
}

#[test]
fn test_red_giant_luminosity_and_habitable_zone() {
    // Red Giant star parameters: T_surf = 3100 K, R = 1.25 AU (~270 R_sun), L = (R/R_sun)^2 * (T/5778)^4 ~ 2500 L_sun
    let star_temp = 3100.0f64;
    let star_radius = 1.25f64;

    // Outer icy world at 55 AU (Kuiper Belt oasis)
    let r_au = 55.0f64;
    let albedo = 0.30f64;

    // Standard radiative equilibrium: T_eq = T_star * sqrt(R_star / (2 * r)) * (1 - A)^0.25
    let t_eq = star_temp * (star_radius / (2.0 * r_au)).sqrt() * (1.0 - albedo).powf(0.25);

    // Radiative equilibrium insolation at 55 AU reaches temperate liquid water regime (~302 K)!
    assert!((260.0..=335.0).contains(&t_eq));
}

#[test]
fn test_stellar_mass_loss_orbital_expansion() {
    // Initial orbit at 10.0 AU with 1.0 M_sun central star
    let r_0 = 10.0f64;
    let m_0 = 1.0f64;

    // Stellar envelope mass loss: star sheds down to 0.55 M_sun White Dwarf remnant
    let m_f = 0.55f64;

    // Adiabatic gravitational invariant: r * M_star = const => r_f = r_0 * (M_0 / M_f)
    let r_f = r_0 * (m_0 / m_f);
    assert!((r_f - 18.18).abs() < 0.1);
    assert!(r_f > r_0);
}

#[test]
fn test_red_giant_inner_planet_engulfment_drag() {
    // Inner planet at 0.8 AU inside expanding Red Giant envelope (R_star = 1.25 AU)
    let r_planet = 0.8f64;
    let r_star = 1.25f64;

    assert!(r_planet < r_star); // Inside stellar envelope

    // Drag decelerates velocity and decays orbital radius
    let mut vel_mag = 5.0f64;
    let dt = 10.0f64;
    vel_mag *= 1.0 - (0.05 * dt).min(0.5);

    assert!(vel_mag < 5.0);
}

#[test]
fn test_comet_hydrostatic_mass_promotion() {
    let comp_icy = Composition::icy();
    let comp_rocky = Composition::rocky();
    let comp_solar = Composition::solar_nebula();

    // Small comet below hydrostatic threshold (~0.0001 Earth masses)
    let comet_type = classify_body_by_mass_and_comp(0.0001 * EARTH_MASS_SOLAR, &comp_icy, false);
    assert_eq!(comet_type, BodyType::Comet);

    // 1.0 Earth-Mass icy body must be promoted to Planet / Ice Giant, not remain a comet!
    let promoted_ice_planet =
        classify_body_by_mass_and_comp(1.0 * EARTH_MASS_SOLAR, &comp_icy, false);
    assert!(matches!(
        promoted_ice_planet,
        BodyType::IceGiant | BodyType::TerrestrialPlanet
    ));

    // 1.0 Earth-Mass rocky body must be promoted to Terrestrial Planet
    let promoted_rocky_planet =
        classify_body_by_mass_and_comp(1.0 * EARTH_MASS_SOLAR, &comp_rocky, false);
    assert_eq!(promoted_rocky_planet, BodyType::TerrestrialPlanet);

    // 3.5 Earth-Mass rocky body must be classified as SuperEarth
    let super_earth = classify_body_by_mass_and_comp(3.5 * EARTH_MASS_SOLAR, &comp_rocky, false);
    assert_eq!(super_earth, BodyType::SuperEarth);

    // 3.5 Earth-Mass body with 98% gas MUST be classified as GasGiant, NOT SuperEarth!
    let gaseous_planet = classify_body_by_mass_and_comp(3.5 * EARTH_MASS_SOLAR, &comp_solar, false);
    assert_eq!(gaseous_planet, BodyType::GasGiant);

    // 3.5 Earth-Mass body with 50% ice & 25% gas MUST be classified as IceGiant, NOT SuperEarth!
    let icy_sub_neptune = classify_body_by_mass_and_comp(3.5 * EARTH_MASS_SOLAR, &comp_icy, false);
    assert_eq!(icy_sub_neptune, BodyType::IceGiant);

    // 150 Earth-Mass body with 0% ice (67% rock, 32% metal, 1% gas) MUST NOT be classified as IceGiant!
    let comp_mega_earth = Composition {
        silicate_frac: 0.67,
        metal_frac: 0.32,
        ice_frac: 0.00,
        organics_frac: 0.00,
        gas_frac: 0.01,
    };
    let mega_earth =
        classify_body_by_mass_and_comp(150.0 * EARTH_MASS_SOLAR, &comp_mega_earth, false);
    assert_ne!(mega_earth, BodyType::IceGiant);
    assert_eq!(mega_earth, BodyType::SuperEarth);

    // 17 Earth-Mass body with 60% ice and 10% gas (Neptune-like) MUST be classified as IceGiant!
    let comp_neptune = Composition {
        silicate_frac: 0.25,
        metal_frac: 0.05,
        ice_frac: 0.60,
        organics_frac: 0.00,
        gas_frac: 0.10,
    };
    let ice_giant = classify_body_by_mass_and_comp(17.0 * EARTH_MASS_SOLAR, &comp_neptune, false);
    assert_eq!(ice_giant, BodyType::IceGiant);

    // Hydrostatic dwarf planet threshold (> 0.005 Earth masses)
    let protoplanet = classify_body_by_mass_and_comp(0.02 * EARTH_MASS_SOLAR, &comp_rocky, false);
    assert!(matches!(
        protoplanet,
        BodyType::Protoplanet | BodyType::TerrestrialPlanet
    ));
}

#[test]
fn test_stellar_mass_classification_hierarchy() {
    let comp = Composition::solar_nebula();

    assert_eq!(
        classify_body_by_mass_and_comp(0.05, &comp, true),
        BodyType::BrownDwarf
    );
    assert_eq!(
        classify_body_by_mass_and_comp(0.25, &comp, true),
        BodyType::RedDwarf
    );
    assert_eq!(
        classify_body_by_mass_and_comp(1.00, &comp, true),
        BodyType::YellowDwarf
    );
    assert_eq!(
        classify_body_by_mass_and_comp(4.00, &comp, true),
        BodyType::BlueGiant
    );
    assert_eq!(
        classify_body_by_mass_and_comp(15.00, &comp, true),
        BodyType::BlueSupergiant
    );
    assert_eq!(
        classify_body_by_mass_and_comp(35.00, &comp, true),
        BodyType::Hypergiant
    );
}

#[test]
fn test_chandrasekhar_and_tov_collapse_limits() {
    // Chandrasekhar Limit = 1.44 M_sun
    assert_eq!(CHANDRASEKHAR_LIMIT_SOLAR, 1.44);
    // Tolman-Oppenheimer-Volkoff (TOV) Limit = 2.17 M_sun
    assert_eq!(TOV_LIMIT_SOLAR, 2.17);

    // Over-mass degenerate White Dwarf
    let wd_mass = 1.55; // > 1.44
    let collapses_to_pulsar = wd_mass > CHANDRASEKHAR_LIMIT_SOLAR;
    assert!(collapses_to_pulsar);

    // Over-mass degenerate Neutron Star
    let ns_mass = 2.50; // > 2.17
    let collapses_to_black_hole = ns_mass > TOV_LIMIT_SOLAR;
    assert!(collapses_to_black_hole);
}

#[test]
fn test_massive_star_supernova_evolution_branch() {
    let mut evo = StellarEvolutionState::default();
    let mass_massive = 12.0f64; // 12 M_sun massive star

    // Main sequence fuel depletion
    evo.phase = StellarEvolutionPhase::MainSequence;
    evo.hydrogen_core_fraction = 0.0;

    // Transition to Red Supergiant branch
    if mass_massive >= 8.0 && evo.hydrogen_core_fraction <= 0.0 {
        evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
    }
    assert_eq!(evo.phase, StellarEvolutionPhase::RedSupergiantBranch);

    // Core collapse triggers Type II Supernova
    evo.phase = StellarEvolutionPhase::SupernovaExplosion;
    evo.nebula_expansion_radius_au = 50.0;

    // Supernova remnant leaves behind a Pulsar
    if evo.nebula_expansion_radius_au >= 40.0 {
        evo.phase = if mass_massive >= 25.0 {
            StellarEvolutionPhase::BlackHoleRemnant
        } else {
            StellarEvolutionPhase::NeutronStarPulsar
        };
    }
    assert_eq!(evo.phase, StellarEvolutionPhase::NeutronStarPulsar);
}

#[test]
fn test_stellar_core_ignition_thermodynamics() {
    let star_mass: f64 = 1.0; // 1.0 Solar Mass
    let mut core_temp: f64 = 5.0e6; // 5 Million K
    let ignition_threshold: f64 = 1.0e7; // 10 Million K

    // Kelvin-Helmholtz heating step
    let heating_rate_per_yr: f64 = 3.5e3 * star_mass;
    let dt_yr: f64 = 2000.0;
    core_temp += heating_rate_per_yr * dt_yr;

    assert!(core_temp > 1.0e7); // Ignited!

    let fusion_fraction: f64 = (core_temp / ignition_threshold).clamp(0.0, 1.0);
    assert_eq!(fusion_fraction, 1.0);

    // Main sequence Mass-Luminosity: L = M^3.5
    let lum: f64 = star_mass.powf(3.5);
    assert!((lum - 1.0).abs() < 1e-6);

    // Main sequence Solar Effective Temperature ~ 5778 K
    let t_eff: f64 = 5778.0 * star_mass.powf(0.505);
    assert!((t_eff - 5778.0).abs() < 1e-4);
}

#[test]
fn test_solar_wind_radiation_pressure_clearing() {
    let mut shockwave_radius: f64 = 0.1; // Starts at 0.1 AU
    let dt_yr: f64 = 0.5;

    // Fast initial blast speed
    let blast_speed: f64 = 35.0;
    shockwave_radius += blast_speed * dt_yr;
    assert!(shockwave_radius > 15.0);

    // Circumstellar gas photoevaporates as shockwave expands to 35 AU
    let gas_density_scale: f64 = (1.0 - (shockwave_radius / 35.0)).clamp(0.0, 1.0);
    assert!(gas_density_scale < 0.6);

    // At 35 AU, gas disk is completely cleared into mature system
    shockwave_radius = 35.0;
    let gas_cleared: f64 = (1.0 - (shockwave_radius / 35.0)).clamp(0.0, 1.0);
    assert_eq!(gas_cleared, 0.0);
}

#[test]
fn test_thermodynamics_zero_distance_and_negative_luminosity_resilience() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::*;

    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<SimTime>()
        .init_resource::<TimeWarp>()
        .add_message::<StarIgnitionEvent>()
        .add_message::<SupernovaEvent>()
        .add_message::<PlanetaryEngulfmentEvent>();

    // Central star with zero/negative luminosity and zero temperature
    app.world_mut().spawn((
        CentralStar,
        Mass(1.0),
        Radius(0.00465),
        Temperature(0.0),
        Luminosity(0.0),
        IgnitionState::default(),
        CelestialBody {
            name: "Dark Star".to_string(),
            body_type: BodyType::BlackHole,
        },
    ));

    // Planet placed at origin (0, 0, 0) - testing r = 0 resilience
    let planet_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Mass(EARTH_MASS_SOLAR),
            Radius(6371.0 / AU_TO_KM),
            Temperature(288.0),
            Composition::rocky(),
            CelestialBody {
                name: "Origin Body".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
        ))
        .id();

    app.add_systems(
        Update,
        protostellar::simulation::thermodynamics::update_thermodynamics,
    );
    app.update();

    let temp = app
        .world()
        .get::<Temperature>(planet_ent)
        .expect("Planet temperature");
    assert!(
        temp.0.is_finite(),
        "Surface temperature must be finite, got {}",
        temp.0
    );
    assert!(
        temp.0 >= 30.0,
        "Surface temperature should be at least deep space floor 30 K, got {}",
        temp.0
    );
}

fn setup_milankovitch_test_app() -> (App, Entity) {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::thermodynamics::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();

    // Central Sun-like star (1.0 M_sun, 1.0 L_sun, 5778 K)
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::MainSequenceStar,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        Temperature(5778.0),
        Luminosity(1.0),
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 50.0,
        },
    ));

    // Earth-like planet at 1.0 AU with water volatiles
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Proto-Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 29.78)),
            Temperature(288.0),
            Composition {
                metal_frac: 0.32,
                silicate_frac: 0.66,
                ice_frac: 0.02,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            VolatileInventory {
                delivered_water_m_earth: 0.0006,
                ocean_coverage_frac: 0.71,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 5,
            },
        ))
        .id();

    app.add_systems(Update, update_thermodynamics);
    (app, earth_ent)
}

#[test]
fn test_milankovitch_ice_albedo_feedback_and_polar_ice_advance() {
    use bevy::math::DVec3;

    let (mut app, earth_ent) = setup_milankovitch_test_app();

    // 1. Initial temperate equilibrium at 1.0 AU
    app.update();

    let climate = *app
        .world()
        .get::<PlanetaryClimate>(earth_ent)
        .expect("PlanetaryClimate must be inserted");
    assert_eq!(climate.climate_regime, ClimateRegime::TemperateHabitable);
    assert!(
        (climate.surface_temperature_k - 288.0).abs() < 5.0,
        "Surface temp should be ~288 K (got {:.1} K)",
        climate.surface_temperature_k
    );
    assert!(
        climate.ice_coverage_frac > 0.05 && climate.ice_coverage_frac < 0.20,
        "Earth ice coverage should be ~10% (got {:.1}%)",
        climate.ice_coverage_frac * 100.0
    );
    assert!(
        climate.polar_ice_cap_latitude_deg > 70.0 && climate.polar_ice_cap_latitude_deg < 88.0,
        "Earth polar ice caps should sit above 70° latitude (got {:.1}°)",
        climate.polar_ice_cap_latitude_deg
    );
    assert!(
        (climate.albedo - 0.29).abs() < 0.05,
        "Earth albedo should be ~0.29 (got {:.2})",
        climate.albedo
    );

    // 2. Move planet out to 1.25 AU (glacial advance / Milankovitch winter)
    {
        let mut pos = app.world_mut().get_mut::<SimPosition>(earth_ent).unwrap();
        pos.0 = DVec3::new(1.25, 0.0, 0.0);
    }
    app.update();

    let cold_climate = app.world().get::<PlanetaryClimate>(earth_ent).unwrap();
    assert!(
        cold_climate.surface_temperature_k < climate.surface_temperature_k,
        "Planet at 1.25 AU must cool down"
    );
    assert!(
        cold_climate.ice_coverage_frac > climate.ice_coverage_frac,
        "Ice caps must advance equatorward as planet cools"
    );
    assert!(
        cold_climate.polar_ice_cap_latitude_deg < climate.polar_ice_cap_latitude_deg,
        "Ice cap edge must move to lower latitudes (got {:.1}°)",
        cold_climate.polar_ice_cap_latitude_deg
    );
    assert!(
        cold_climate.albedo > climate.albedo,
        "Ice-albedo positive feedback must increase planetary albedo"
    );
}

#[test]
fn test_climate_snowball_and_hothouse_regimes() {
    use bevy::math::DVec3;

    let (mut app, earth_ent) = setup_milankovitch_test_app();

    // 3. Move planet out to 1.65 AU (runaway Snowball Earth)
    {
        let mut pos = app.world_mut().get_mut::<SimPosition>(earth_ent).unwrap();
        pos.0 = DVec3::new(1.65, 0.0, 0.0);
        let mut temp = app.world_mut().get_mut::<Temperature>(earth_ent).unwrap();
        temp.0 = 230.0;
    }
    app.update();

    let snowball = app.world().get::<PlanetaryClimate>(earth_ent).unwrap();
    assert_eq!(
        snowball.climate_regime,
        ClimateRegime::SnowballIceAge,
        "Planet at 1.65 AU must enter Snowball Ice Age"
    );
    assert_eq!(
        snowball.ice_coverage_frac, 1.0,
        "Snowball Earth must have 100% ice coverage"
    );
    assert_eq!(
        snowball.polar_ice_cap_latitude_deg, 0.0,
        "Snowball Earth ice caps must reach the equator (0°)"
    );
    assert!(
        snowball.albedo > 0.65,
        "Snowball Earth albedo must exceed 0.65 (got {:.2})",
        snowball.albedo
    );

    // 4. Move planet into 0.78 AU (warm greenhouse / ice-free poles)
    {
        let mut pos = app.world_mut().get_mut::<SimPosition>(earth_ent).unwrap();
        pos.0 = DVec3::new(0.78, 0.0, 0.0);
        let mut temp = app.world_mut().get_mut::<Temperature>(earth_ent).unwrap();
        temp.0 = 315.0;
    }
    app.update();

    let hothouse = app.world().get::<PlanetaryClimate>(earth_ent).unwrap();
    assert_eq!(
        hothouse.ice_coverage_frac, 0.0,
        "Warm planet must have completely melted ice caps"
    );
    assert_eq!(
        hothouse.polar_ice_cap_latitude_deg, 90.0,
        "Ice-free planet ice cap latitude must be 90° (at the pole)"
    );
    assert!(
        hothouse.albedo < 0.28,
        "Ice-free water world must have low bare albedo (got {:.2})",
        hothouse.albedo
    );
}
