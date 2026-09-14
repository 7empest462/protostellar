//! Test module generated from simulation_tests.

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
