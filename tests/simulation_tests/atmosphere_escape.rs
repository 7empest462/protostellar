//! Integration tests for Feature 2.3: Atmospheric Escape, Photoevaporation & Solar Wind Stripping.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::simulation::atmosphere_escape::physics::*;
use protostellar::simulation::atmosphere_escape::systems::update_atmospheric_escape_evolution;
use protostellar::simulation::atmosphere_escape::types::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::serialization::*;
use protostellar::utils::constants::*;

#[test]
fn test_energy_limited_photoevaporation_scaling() {
    // 1. Coronal saturation scaling: young star <= 100 Myr vs mature star at 4.5 Gyr
    let l_xuv_young = calculate_stellar_xuv_luminosity(1.0, 5.0e7); // 50 Myr
    let l_xuv_mature = calculate_stellar_xuv_luminosity(1.0, 4.5e9); // 4.5 Gyr
    assert!(
        l_xuv_young > l_xuv_mature * 10.0,
        "Young coronal saturation should be orders of magnitude higher than mature stellar XUV: young={l_xuv_young}, mature={l_xuv_mature}"
    );

    // 2. Inverse-square flux scaling: 0.05 AU vs 1.0 AU
    let flux_005 = calculate_xuv_flux(l_xuv_young, 0.05);
    let flux_100 = calculate_xuv_flux(l_xuv_young, 1.0);
    let flux_ratio = flux_005 / flux_100;
    assert!(
        (flux_ratio - 400.0).abs() < 5.0,
        "Flux at 0.05 AU should be ~(1/0.05)^2 = 400x flux at 1.0 AU: ratio={flux_ratio}"
    );

    // 3. Photoevaporation rate scaling with efficiency eta
    let r_planet_au = 2.0 * EARTH_RADIUS_AU;
    let m_planet_solar = 4.0 * EARTH_MASS_SOLAR;
    let r_hill_au = 0.005;

    let rate_eta_10 =
        calculate_photoevaporation_rate(m_planet_solar, r_planet_au, flux_005, 0.10, r_hill_au);
    let rate_eta_20 =
        calculate_photoevaporation_rate(m_planet_solar, r_planet_au, flux_005, 0.20, r_hill_au);

    assert!(
        rate_eta_20 > rate_eta_10 * 1.9,
        "Doubling efficiency eta should approximately double photoevaporation rate: eta10={rate_eta_10}, eta20={rate_eta_20}"
    );
    assert!(
        rate_eta_10 > 0.0001,
        "Close-in Sub-Neptune at 0.05 AU should experience significant photoevaporative mass loss: {rate_eta_10} M_earth/Myr"
    );
}

#[test]
fn test_erkaev_tidal_roche_correction() {
    let r_planet_au = EARTH_RADIUS_AU;

    // Distant planet: Hill radius >> Planet radius => xi >> 1 => K_tide -> 1.0
    let r_hill_distant = r_planet_au * 100.0;
    let k_distant = calculate_erkaev_roche_correction(r_planet_au, r_hill_distant);
    assert!(
        (k_distant - 1.0).abs() < 0.02,
        "Distant planet should have Erkaev K_tide close to 1.0: {k_distant}"
    );

    // Close-in planet: Hill radius close to Planet radius => xi ~ 2.0 => K_tide < 1.0
    let r_hill_close = r_planet_au * 2.0;
    let k_close = calculate_erkaev_roche_correction(r_planet_au, r_hill_close);
    assert!(
        k_close < 0.5,
        "Tidally distorted close-in planet should have K_tide significantly < 1.0: {k_close}"
    );

    // Photoevaporation rate with small K_tide is higher because denominator contains K_tide
    let flux = 100.0;
    let m_solar = EARTH_MASS_SOLAR;
    let rate_distant =
        calculate_photoevaporation_rate(m_solar, r_planet_au, flux, 0.15, r_hill_distant);
    let rate_close =
        calculate_photoevaporation_rate(m_solar, r_planet_au, flux, 0.15, r_hill_close);

    assert!(
        rate_close > rate_distant,
        "Tidal Roche lobe overflow should accelerate mass loss (1 / K_tide): close={rate_close}, distant={rate_distant}"
    );
}

#[test]
fn test_magnetopause_standoff_and_geodynamo_shielding() {
    let r_planet_au = EARTH_RADIUS_AU;
    let (p_ram, _) = calculate_solar_wind_pressure(1.0, 1.0);
    assert!(
        (p_ram - P_RAM_1AU).abs() < 1e-11,
        "Solar wind ram pressure at 1 AU should match baseline P_RAM_1AU: {p_ram}"
    );

    // 1. Unmagnetized planet (B = 0 Gauss, e.g. Mars/Venus type)
    let r_mp_unshielded = calculate_magnetopause_radius(r_planet_au, 0.0, p_ram);
    assert!(
        (r_mp_unshielded - r_planet_au).abs() < 1e-12,
        "Unmagnetized planet magnetopause should sit directly at planetary surface"
    );
    let shield_unshielded = calculate_magnetic_shielding_factor(r_planet_au, r_mp_unshielded);
    assert!(
        shield_unshielded < 0.01,
        "Unmagnetized planet should have zero magnetic shielding: {shield_unshielded}"
    );

    // Stripping rate for unshielded planet should be positive
    let strip_unshielded = calculate_solar_wind_stripping_rate(
        r_planet_au,
        p_ram,
        shield_unshielded,
        EARTH_MASS_SOLAR,
    );
    assert!(
        strip_unshielded > 0.0001,
        "Unshielded planet should experience stellar wind stripping: {strip_unshielded}"
    );

    // 2. Strongly magnetized planet (B = 0.5 Gauss, Earth-like geodynamo)
    let r_mp_shielded = calculate_magnetopause_radius(r_planet_au, 0.5, p_ram);
    assert!(
        r_mp_shielded > r_planet_au * 5.0,
        "Earth-like geodynamo should produce stand-off radius > 5 R_p: ratio={}",
        r_mp_shielded / r_planet_au
    );
    let shield_shielded = calculate_magnetic_shielding_factor(r_planet_au, r_mp_shielded);
    assert!(
        shield_shielded > 0.90,
        "Earth-like geodynamo should provide > 90% magnetic shielding: {shield_shielded}"
    );

    let strip_shielded =
        calculate_solar_wind_stripping_rate(r_planet_au, p_ram, shield_shielded, EARTH_MASS_SOLAR);
    assert!(
        strip_shielded < strip_unshielded * 0.10,
        "Magnetic shielding should reduce stripping rate by > 90%: shielded={strip_shielded}, unshielded={strip_unshielded}"
    );
}

#[test]
fn test_jeans_thermal_escape_by_species() {
    let m_solar = EARTH_MASS_SOLAR;
    let r_au = EARTH_RADIUS_AU;
    let t_exobase = 1000.0; // 1000 K thermosphere
    let p_bar = 1.0;

    // Molecular Hydrogen (H2, ~2 amu)
    let (lambda_h2, rate_h2) = calculate_jeans_escape(m_solar, r_au, t_exobase, 2.0, p_bar);

    // Carbon Dioxide (CO2, ~44 amu)
    let (lambda_co2, rate_co2) = calculate_jeans_escape(m_solar, r_au, t_exobase, 44.0, p_bar);

    assert!(
        lambda_h2 < lambda_co2,
        "Hydrogen should have much lower gravitational escape parameter lambda than CO2: H2={lambda_h2}, CO2={lambda_co2}"
    );
    assert!(
        rate_h2 > 0.0,
        "Light hydrogen at 1000 K should undergo thermal Jeans escape: {rate_h2} M_earth/Myr"
    );
    assert!(
        rate_co2 < 1e-12,
        "Heavy CO2 (44 amu) should be tightly gravitationally bound with zero thermal escape: lambda={lambda_co2}"
    );
}

#[test]
fn test_fulton_gap_chthonian_core_transition() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<AtmosphericEscapeConfig>()
        .add_message::<AtmosphericStrippedEvent>();

    // Spawn central star
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Host Star".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        SimPosition(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        Luminosity(1.0),
        IgnitionState::default(),
    ));

    // Spawn Sub-Neptune with 5% gas envelope at 0.03 AU
    let initial_gas_frac = 0.05;
    let sub_neptune = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Hot Sub-Neptune".to_string(),
                body_type: BodyType::IceGiant,
            },
            SimPosition(DVec3::new(0.03, 0.0, 0.0)),
            Mass(4.0 * EARTH_MASS_SOLAR),
            Radius(2.2 * EARTH_RADIUS_AU),
            Composition {
                silicate_frac: 0.65,
                metal_frac: 0.30,
                ice_frac: 0.0,
                organics_frac: 0.0,
                gas_frac: initial_gas_frac,
            },
            AtmosphericEscapeState::default(),
        ))
        .id();

    // Set time step to a large epoch to simulate envelope stripping
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 5.0e6; // 5 Myr step
    app.world_mut()
        .resource_mut::<AtmosphericEscapeConfig>()
        .time_scale = 10.0;
    app.add_systems(Update, update_atmospheric_escape_evolution);

    // Run system update
    app.update();

    let planet_ref = app.world().entity(sub_neptune);
    let comp = planet_ref
        .get::<Composition>()
        .expect("Planet has Composition");

    // The gas envelope should have been completely stripped
    assert!(
        comp.gas_frac < 0.001,
        "Gas envelope should be stripped below 0.1%, transitioning to bare chthonian core: gas_frac={}",
        comp.gas_frac
    );

    // Verify AtmosphericStrippedEvent was fired
    let events = app.world().resource::<Messages<AtmosphericStrippedEvent>>();
    assert!(
        !events.is_empty(),
        "AtmosphericStrippedEvent should be emitted upon complete envelope loss"
    );
}

#[test]
fn test_cometary_sublimation_tail_activation() {
    let star_lum = 1.0;
    let ice_frac = 0.60;

    // Inside the water ice snow line (e.g. 1.0 AU from 1 L_sun star)
    let (tail_len_1au, loss_1au, ion_col_1au) =
        calculate_cometary_sublimation(1.0, star_lum, ice_frac);
    assert!(
        tail_len_1au > 0.5,
        "Active comet at 1 AU should develop an extended tail: {tail_len_1au} AU"
    );
    assert!(
        loss_1au > 0.005,
        "Active comet at 1 AU should have substantial volatile mass loss: {loss_1au}"
    );
    assert!(
        ion_col_1au.to_srgba().alpha > 0.5,
        "Active comet ion tail should be visible with high alpha: {ion_col_1au:?}"
    );

    // Outside the snow line (e.g. 5.0 AU from 1 L_sun star)
    let (tail_len_5au, loss_5au, _) = calculate_cometary_sublimation(5.0, star_lum, ice_frac);
    assert!(
        tail_len_5au < 1e-6,
        "Distant comet beyond snow line should have zero sublimation tail"
    );
    assert!(
        loss_5au < 1e-6,
        "Distant comet beyond snow line should have zero mass loss"
    );
}

#[test]
fn test_atmospheric_escape_serialization() {
    let escape_state = AtmosphericEscapeState {
        photoevaporative_loss_rate_m_earth_per_myr: 1.45,
        solar_wind_stripping_rate_m_earth_per_myr: 0.025,
        jeans_escape_rate_m_earth_per_myr: 0.008,
        total_loss_rate_m_earth_per_myr: 1.483,
        magnetopause_radius_au: 0.00042,
        magnetic_shielding_factor: 0.88,
        xuv_flux_w_m2: 45.2,
        solar_wind_pressure_n_m2: 1.2e-8,
        roche_lobe_fill_fraction: 0.35,
        cumulative_mass_lost_m_earth: 0.22,
        escape_regime: AtmosphericEscapeRegime::HydrodynamicPhotoevaporation,
    };

    let body_save = CelestialBodySave {
        name: "WASP-12b".to_string(),
        body_type: BodyType::GasGiant,
        position: DVec3::new(0.023, 0.0, 0.0),
        velocity: DVec3::new(0.0, 0.0, 30.0),
        mass: 0.0014,
        radius: 0.0009,
        temperature: 2500.0,
        luminosity: 0.0,
        composition: Composition::solar_gas(),
        spin: SpinState::default(),
        differentiation: None,
        volatile_inventory: None,
        ring_system: None,
        basins: None,
        climate: None,
        biosphere: None,
        electromagnetic: None,
        is_central_star: false,
        ignition_state: None,
        stellar_evolution: None,
        black_hole_state: None,
        satellite: None,
        tidal_state: None,
        relativistic_state: None,
        atmospheric_escape: Some(escape_state),
        kozai_lidov: None,
    };

    let serialized =
        serde_json::to_string_pretty(&body_save).expect("Failed to serialize body save");
    assert!(serialized.contains("atmospheric_escape"));
    assert!(serialized.contains("HydrodynamicPhotoevaporation"));

    let deserialized: CelestialBodySave =
        serde_json::from_str(&serialized).expect("Failed to deserialize body save");
    assert_eq!(deserialized.name, "WASP-12b");
    let loaded_escape = deserialized
        .atmospheric_escape
        .expect("Deserialized state contains atmospheric_escape");
    assert_eq!(
        loaded_escape.escape_regime,
        AtmosphericEscapeRegime::HydrodynamicPhotoevaporation
    );
    assert!((loaded_escape.photoevaporative_loss_rate_m_earth_per_myr - 1.45).abs() < 1e-6);
    assert!((loaded_escape.magnetic_shielding_factor - 0.88).abs() < 1e-6);
}

#[test]
fn test_atmospheric_escape_ecs_system_integration() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<AtmosphericEscapeConfig>()
        .add_message::<AtmosphericStrippedEvent>();

    // Spawn central star
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sol".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        SimPosition(DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        Luminosity(1.0),
        IgnitionState::default(),
    ));

    // Spawn close-in exoplanet
    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Close Exoplanet".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(0.04, 0.0, 0.0)),
            Mass(2.0 * EARTH_MASS_SOLAR),
            Radius(1.5 * EARTH_RADIUS_AU),
            Composition {
                silicate_frac: 0.80,
                metal_frac: 0.15,
                ice_frac: 0.0,
                organics_frac: 0.0,
                gas_frac: 0.05,
            },
        ))
        .id();

    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 1000.0;
    app.add_systems(Update, update_atmospheric_escape_evolution);

    // Step the simulation
    app.update();

    let planet_ref = app.world().entity(planet);
    let escape_state = planet_ref
        .get::<AtmosphericEscapeState>()
        .expect("System should attach/populate AtmosphericEscapeState on simulated planet");

    assert!(
        escape_state.photoevaporative_loss_rate_m_earth_per_myr > 0.0,
        "Close-in planet should have non-zero photoevaporation rate: {}",
        escape_state.photoevaporative_loss_rate_m_earth_per_myr
    );
    assert!(
        escape_state.xuv_flux_w_m2 > 0.0,
        "Close-in planet should record non-zero XUV flux: {}",
        escape_state.xuv_flux_w_m2
    );
    assert_ne!(
        escape_state.escape_regime,
        AtmosphericEscapeRegime::None,
        "Close-in planet should classify into an active escape regime"
    );
}
