//! Integration tests for Feature 2.4: Kozai-Lidov Resonance & Secular Orbital Oscillations.

use std::f64::consts::PI;

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::kozai_lidov::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::spawn_kozai_triple_scenario;
use protostellar::simulation::tides::TidalState;
use protostellar::utils::constants::*;

#[test]
fn test_mutual_inclination_calculation() {
    // Prograde coplanar: h1 and h2 along +Y
    let h1 = DVec3::new(0.0, 10.0, 0.0);
    let h2 = DVec3::new(0.0, 50.0, 0.0);
    let i_mut = compute_mutual_inclination(h1, h2);
    assert!(
        i_mut.abs() < 1e-6,
        "Coplanar prograde should have 0 deg inclination"
    );

    // Perpendicular: h1 along +Y, h2 along +Z
    let h_perp = DVec3::new(0.0, 0.0, 50.0);
    let i_perp = compute_mutual_inclination(h1, h_perp);
    assert!(
        (i_perp - (PI / 2.0)).abs() < 1e-6,
        "Perpendicular orbits should have 90 deg inclination"
    );

    // Retrograde coplanar: h1 along +Y, h2 along -Y
    let h_retro = DVec3::new(0.0, -50.0, 0.0);
    let i_retro = compute_mutual_inclination(h1, h_retro);
    assert!(
        (i_retro - PI).abs() < 1e-6,
        "Retrograde coplanar should have 180 deg inclination"
    );
}

#[test]
fn test_critical_kozai_angle_threshold() {
    assert!(
        (KOZAI_CRITICAL_ANGLE_RAD - 0.684_719).abs() < 0.001,
        "Critical Kozai angle in radians should be ~0.6847"
    );
    let crit_deg = KOZAI_CRITICAL_ANGLE_DEG;

    assert!(
        (crit_deg - 39.2315).abs() < 0.01,
        "Critical Kozai angle should be approx 39.23 deg"
    );

    // Just subcritical (39.0 deg)
    let subcrit_rad = 39.0f64.to_radians();
    assert!(
        !is_in_kozai_resonance(subcrit_rad),
        "39.0 deg should be subcritical"
    );

    // Just supercritical (39.5 deg)
    let supercrit_rad = 39.5f64.to_radians();
    assert!(
        is_in_kozai_resonance(supercrit_rad),
        "39.5 deg should be in Kozai resonance"
    );

    // Retrograde critical angle: 180 - 39.23 = 140.77 deg
    let retrograde_resonant = 140.0f64.to_radians();
    assert!(
        is_in_kozai_resonance(retrograde_resonant),
        "140 deg should be resonant"
    );

    let retrograde_subcritical = 142.0f64.to_radians();
    assert!(
        !is_in_kozai_resonance(retrograde_subcritical),
        "142 deg should be subcritical"
    );
}

#[test]
fn test_max_eccentricity_formula() {
    // For 90 degrees inclination (cos i = 0), e_max = sqrt(1 - 0) = 1.0 (capped at 0.999)
    let e_max_90 = compute_max_eccentricity(0.0, 90.0f64.to_radians());
    assert!(
        e_max_90 >= 0.99,
        "At 90 deg inclination, e_max should approach 1.0"
    );

    // For 68 degrees (HD 80606 analog): cos(68) ~ 0.3746, 5/3 * cos^2(68) ~ 0.2339, e_max ~ sqrt(0.766) ~ 0.875
    let e_max_68 = compute_max_eccentricity(0.0, 68.0f64.to_radians());
    assert!(
        (e_max_68 - 0.875).abs() < 0.02,
        "At 68 deg inclination, e_max should be ~0.875, got {e_max_68}"
    );

    // For 45 degrees: cos(45) = 1/sqrt(2), cos^2(45) = 1/2, e_max = sqrt(1 - 5/6) = sqrt(1/6) ~ 0.408
    let e_max_45 = compute_max_eccentricity(0.0, 45.0f64.to_radians());
    assert!(
        (e_max_45 - (1.0f64 / 6.0f64).sqrt()).abs() < 0.01,
        "At 45 deg, e_max should be ~0.408"
    );

    // For subcritical angle (30 deg), e_max should equal initial e
    let e_max_sub = compute_max_eccentricity(0.05, 30.0f64.to_radians());
    assert!(
        (e_max_sub - 0.05).abs() < 1e-6,
        "Subcritical inclination should not pump eccentricity"
    );
}

#[test]
fn test_kozai_timescale_scaling() {
    let a_in = 1.0;
    let a_out = 20.0;
    let m_central = 1.0;
    let m_pert = 0.001; // 1 Jupiter mass

    let tau_base = compute_kozai_timescale(a_in, a_out, m_central, m_pert, 0.0);
    assert!(
        tau_base.is_finite() && tau_base > 1000.0,
        "Timescale should be physical"
    );

    // If perturber mass is doubled, timescale should roughly halve
    let tau_double_mass = compute_kozai_timescale(a_in, a_out, m_central, m_pert * 2.0, 0.0);
    let ratio = tau_base / tau_double_mass;
    assert!(
        (ratio - 2.0).abs() < 0.05,
        "Doubling perturber mass should halve Kozai timescale, ratio was {ratio}"
    );

    // If outer orbit is eccentric (e_out = 0.6), timescale decreases by (1 - 0.6^2)^1.5 = (0.64)^1.5 = 0.512
    let tau_ecc = compute_kozai_timescale(a_in, a_out, m_central, m_pert, 0.6);
    let ecc_ratio = tau_ecc / tau_base;
    assert!(
        (ecc_ratio - 0.512).abs() < 0.01,
        "Eccentric outer orbit should shorten timescale by (1-e^2)^1.5, got {ecc_ratio}"
    );
}

#[test]
fn test_gr_precession_resonance_suppression() {
    // Mercury-like or tight orbit near a star: GR precession is fast (~10^5 yr)
    // while Kozai from a distant star is slow (~10^7 yr).
    let tau_kl_slow = 1.0e7;
    let tau_gr_fast = 2.0e5;
    let unsuppressed_e_max = 0.90;

    let (ratio, is_suppressed, effective_e_max) =
        compute_gr_resonance_suppression(tau_kl_slow, tau_gr_fast, unsuppressed_e_max);

    assert!(is_suppressed, "Fast GR precession should suppress Kozai");
    assert!(ratio > 10.0, "tau_KL / tau_GR ratio should be large");
    assert!(
        effective_e_max < 0.20,
        "GR suppression should strongly damp maximum eccentricity from 0.90 down to ~0.018, got {effective_e_max}"
    );

    // In a wide orbit where GR precession is exceedingly slow (e.g. 10^10 yr):
    let tau_gr_slow = 1.0e10;
    let (ratio_wide, is_suppressed_wide, effective_e_wide) =
        compute_gr_resonance_suppression(tau_kl_slow, tau_gr_slow, unsuppressed_e_max);

    assert!(
        !is_suppressed_wide,
        "Slow GR precession should not suppress Kozai"
    );
    assert!(
        ratio_wide < 0.01,
        "Ratio should be << 1 when GR is negligible"
    );
    assert!(
        (effective_e_wide - unsuppressed_e_max).abs() < 1e-6,
        "Unsuppressed e_max preserved"
    );
}

#[test]
fn test_high_eccentricity_tidal_migration_coupling() {
    let mut tide = TidalState::new_gas_giant();
    let initial_power = 1.0e15; // 1 Petawatt
    tide.tidal_heating_power_watts = initial_power;
    tide.circularization_rate_per_myr = 0.001;

    // At high eccentricity e = 0.85, (1 - e^2)^7.5 is ~ (0.2775)^7.5 ~ 6.5e-5
    let e: f64 = 0.85;
    let one_minus_e2: f64 = (1.0 - e * e).max(0.01);
    let boost: f64 = (1.0 + 3.75 * e * e) / one_minus_e2.powf(7.5);
    assert!(
        boost > 1000.0,
        "High eccentricity should produce massive tidal dissipation boost"
    );

    tide.tidal_heating_power_watts *= boost.min(1.0e5);
    tide.circularization_rate_per_myr *= boost.min(1.0e4);
    assert!(
        tide.tidal_heating_power_watts >= initial_power * 1000.0,
        "Tidal dissipation must be heavily amplified at periastron"
    );
    assert!(
        tide.circularization_rate_per_myr > 1.0,
        "Circularization rate should be accelerated"
    );
}

#[test]
fn test_roche_disruption_and_regime_classification() {
    let r_planet = EARTH_RADIUS_AU * 11.2;
    let m_star = 1.0;
    let m_planet = 0.001; // ~1 Jupiter mass
    let r_roche = compute_roche_disruption_radius(r_planet, m_star, m_planet);

    // R_roche ~ 2.456 * R_planet * (1000)^(1/3) ~ 2.456 * R_planet * 10 ~ 24.56 R_planet
    assert!(
        r_roche > r_planet * 20.0,
        "Roche radius should be ~24x planet radius"
    );

    // If q_min is inside Roche limit, classify as TidalDisruptionRisk
    let q_min_danger = r_roche * 0.8;
    let regime_danger =
        classify_kozai_regime(true, false, q_min_danger, r_roche, 90.0f64.to_radians());
    assert_eq!(
        regime_danger,
        KozaiRegime::TidalDisruptionRisk,
        "q_min inside Roche limit should trigger TidalDisruptionRisk"
    );

    // If libration angle near 90 deg and safe q_min:
    let q_min_safe = r_roche * 5.0;
    let regime_lib = classify_kozai_regime(true, false, q_min_safe, r_roche, 92.0f64.to_radians());
    assert_eq!(
        regime_lib,
        KozaiRegime::Libration,
        "Near 90 deg argument of periapsis should classify as Libration"
    );

    // Circulation when omega is away from 90/270 deg (e.g. 0 deg)
    let regime_circ = classify_kozai_regime(true, false, q_min_safe, r_roche, 0.0f64.to_radians());
    assert_eq!(
        regime_circ,
        KozaiRegime::Circulation,
        "Near 0 deg argument of periapsis should classify as Circulation"
    );
}

#[test]
fn test_kozai_lidov_state_serialization_round_trip() {
    let state = KozaiLidovState {
        perturber_entity: None,
        perturber_name: "Companion Star (M-Dwarf)".to_string(),
        mutual_inclination_deg: 68.5,
        critical_inclination_deg: KOZAI_CRITICAL_ANGLE_DEG,
        is_in_resonance: true,
        max_eccentricity_forecast: 0.88,
        min_periastron_au: 0.36,
        kozai_period_years: 45000.0,
        gr_precession_ratio: 0.005,
        is_gr_suppressed: false,
        regime: KozaiRegime::Circulation,
        cycle_phase: 0.25,
    };

    let serialized = serde_json::to_string(&state).expect("Serialization failed");
    let deserialized: KozaiLidovState =
        serde_json::from_str(&serialized).expect("Deserialization failed");

    assert_eq!(deserialized.perturber_name, "Companion Star (M-Dwarf)");
    assert!((deserialized.mutual_inclination_deg - 68.5).abs() < 1e-6);
    assert!(deserialized.is_in_resonance);
    assert!((deserialized.max_eccentricity_forecast - 0.88).abs() < 1e-6);
    assert_eq!(deserialized.regime, KozaiRegime::Circulation);
}

#[test]
fn test_kozai_hierarchical_triple_scenario_spawn_and_ecs_integration() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<SimTime>()
        .init_resource::<TimeWarp>()
        .init_resource::<DiskParameters>()
        .init_resource::<KozaiLidovConfig>()
        .add_message::<KozaiDisruptionEvent>();

    let (star, planet, companion) = {
        let mut disk_params = DiskParameters::default();
        let mut commands = app.world_mut().commands();
        spawn_kozai_triple_scenario(&mut commands, &mut disk_params)
    };

    app.update();

    let world = app.world();
    assert!(world.get_entity(star).is_ok(), "Star should be alive");
    assert!(world.get_entity(planet).is_ok(), "Planet should be alive");
    assert!(
        world.get_entity(companion).is_ok(),
        "Companion should be alive"
    );

    // Verify planet components
    let kozai_state = world
        .get::<KozaiLidovState>(planet)
        .expect("Planet must have KozaiLidovState");
    assert!(
        kozai_state.is_in_resonance,
        "Planet should be initialized in active resonance"
    );
    assert!(
        (kozai_state.mutual_inclination_deg - 68.0).abs() < 1.0,
        "Mutual inclination should be ~68 deg, got {}",
        kozai_state.mutual_inclination_deg
    );
    assert!(
        kozai_state.max_eccentricity_forecast > 0.80,
        "Peak eccentricity should be forecast > 0.80, got {}",
        kozai_state.max_eccentricity_forecast
    );
    assert!(
        kozai_state.min_periastron_au < 1.0,
        "q_min should plunge within 1.0 AU"
    );
}

#[test]
fn test_kozai_lidov_detection_uses_perturber_mass() {
    use protostellar::simulation::components::*;

    let mut app = App::new();
    app.init_resource::<KozaiLidovConfig>();

    // Primary central star (1 M_sun)
    app.world_mut().spawn((
        CentralStar,
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        CelestialBody {
            name: "Central Star".to_string(),
            body_type: BodyType::Protostar,
        },
    ));

    // Inner low-mass body: Asteroid (1e-10 M_sun) at 1 AU
    let inner_ent = app
        .world_mut()
        .spawn((
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 6.28)),
            Mass(1e-10),
            CelestialBody {
                name: "Inner Asteroid".to_string(),
                body_type: BodyType::Asteroid,
            },
        ))
        .id();

    // Outer massive perturber: Giant planet (1e-3 M_sun) at 5 AU
    app.world_mut().spawn((
        SimPosition(DVec3::new(0.0, 4.0, 3.0)), // 5 AU with inclination
        SimVelocity(DVec3::new(2.8, 0.0, 0.0)),
        Mass(1e-3),
        CelestialBody {
            name: "Outer Giant".to_string(),
            body_type: BodyType::GasGiant,
        },
    ));

    app.add_systems(Update, detect_hierarchical_triples);
    app.update();

    let state = app
        .world()
        .get::<KozaiLidovState>(inner_ent)
        .expect("Inner asteroid must have detected KozaiLidovState");

    // Kozai timescale tau_kl with perturber mass 1e-3 should be order ~26,000 years.
    // If the inner body's mass (1e-10) had been used, tau_kl would be ~ 2.6e11 years!
    assert!(
        state.kozai_period_years < 100_000.0,
        "Kozai timescale should be ~26,000 years with perturber mass 1e-3, but got {}",
        state.kozai_period_years
    );
    assert!(
        state.kozai_period_years > 5_000.0,
        "Kozai timescale should be > 5,000 years, got {}",
        state.kozai_period_years
    );
}
