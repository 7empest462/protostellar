//! Test module generated from simulation_tests.

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
fn test_giant_planet_resonance_migration() {
    let r_j: f64 = 5.5; // Jupiter semi-major axis (initial compact configuration)
    let r_s: f64 = 8.5; // Saturn semi-major axis (inside 2:1 resonance, ratio ~ 1.92)
    let p_ratio_initial: f64 = (r_s / r_j).powf(1.5);
    assert!(p_ratio_initial < 2.0); // Before 2:1 resonance

    // Outward migration of Saturn to 9.58 AU and inward migration of Jupiter to 5.2 AU
    let r_j_final: f64 = 5.20;
    let r_s_final: f64 = 9.58;
    let p_ratio_final: f64 = (r_s_final / r_j_final).powf(1.5);
    assert!(p_ratio_final > 2.0); // Crossed 2:1 resonance!
    assert!((p_ratio_final - 2.50).abs() < 0.1);
}

#[test]
fn test_volatile_water_delivery_mass_budget() {
    use protostellar::simulation::components::VolatileInventory;

    let mut vol = VolatileInventory::default();
    assert_eq!(vol.delivered_water_m_earth, 0.0);
    assert_eq!(vol.ocean_coverage_frac, 0.0);

    // 10 icy cometary impacts delivering 0.00008 Earth masses of volatile ice each
    for _ in 0..10 {
        let d_water = 0.00008;
        vol.delivered_water_m_earth += d_water;
        vol.cometary_impact_count += 1;
        vol.ocean_coverage_frac = (vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
    }

    assert_eq!(vol.cometary_impact_count, 10);
    assert!((vol.delivered_water_m_earth - 0.0008).abs() < 1e-6);
    assert!(vol.ocean_coverage_frac >= 0.85); // Fully filled Earth-like ocean basins!
}

#[test]
fn test_fluid_roche_limit_disruption_radius() {
    // Saturn: density ~ 0.687 g/cm3, radius ~ 58,232 km (0.000389 AU)
    let rho_primary: f64 = 0.687;
    let r_primary_km: f64 = 58232.0;

    // Pure Water Ice moon: density ~ 1.0 g/cm3
    let rho_moon: f64 = 1.0;

    let d_roche_km: f64 = 2.44 * r_primary_km * (rho_primary / rho_moon).cbrt();

    // d_roche should be ~ 125,380 km (~ 2.15 Saturn radii), exactly inside Saturn's ring system!
    assert!(d_roche_km > 120000.0 && d_roche_km < 130000.0);
    assert!((d_roche_km / r_primary_km - 2.15).abs() < 0.1);
}

#[test]
fn test_planetary_ring_mass_and_optical_depth() {
    use protostellar::simulation::components::PlanetaryRingSystem;

    let mut ring = PlanetaryRingSystem::default();
    assert_eq!(ring.optical_depth, 0.85);
    assert_eq!(ring.ice_fraction, 0.95);

    // Accrete additional fragmented moon mass
    let additional_moon_earth_mass = 0.0002;
    ring.ring_mass_earth += additional_moon_earth_mass;
    ring.optical_depth = (ring.optical_depth + 0.10).min(1.0);

    assert!((ring.optical_depth - 0.95).abs() < 1e-5);
    assert!(ring.ring_mass_earth > 0.00025);
}

#[test]
fn test_dynamo_magnetic_field_scaling() {
    use protostellar::simulation::components::InternalDifferentiation;

    let mut diff = InternalDifferentiation {
        is_differentiated: true,
        differentiation_fraction: 1.0,
        core_radius_au: 0.000023,
        mantle_radius_au: 0.000042,
        crust_thickness_au: 0.000001,
        ocean_ice_thickness_au: 0.0,
        magnetic_field_gauss: 0.0,
        core_temp_k: 4500.0,
    };

    let period_hrs: f64 = 24.0;
    let temp_factor = ((diff.core_temp_k - 1800.0) / 2000.0).clamp(0.0, 1.5);
    let spin_factor = (24.0f64 / period_hrs).sqrt().clamp(0.2, 3.0);
    let core_mass_frac = (diff.core_radius_au / diff.mantle_radius_au).powi(3);

    let b_gauss = 0.35 * core_mass_frac.sqrt() * spin_factor * temp_factor.powf(0.33);
    diff.magnetic_field_gauss = b_gauss;

    // Earth analog: ~0.3 - 0.5 Gauss
    assert!(diff.magnetic_field_gauss >= 0.15 && diff.magnetic_field_gauss <= 0.60);

    // Rapid rotator (Jupiter analog, period = 10 hrs)
    let fast_spin_factor = (24.0f64 / 10.0f64).sqrt();
    let fast_b = 0.35 * core_mass_frac.sqrt() * fast_spin_factor * temp_factor.powf(0.33);
    assert!(fast_b > diff.magnetic_field_gauss);
}

#[test]
fn test_greenhouse_climate_equilibrium_earth() {
    use protostellar::simulation::components::{ClimateRegime, PlanetaryClimate};
    use protostellar::utils::constants::SOLAR_RADIUS_AU;

    let star_temp = 5778.0f64;
    let star_radius = SOLAR_RADIUS_AU;
    let r_au = 1.0f64;
    let albedo = 0.30f64;

    // Radiative equilibrium: T_eq = T_sun * sqrt(R_sun / (2 * r)) * (1 - A)^0.25
    let t_eq = star_temp * (star_radius / (2.0 * r_au)).sqrt() * (1.0 - albedo).powf(0.25);
    assert!((t_eq - 255.0).abs() < 3.0); // Pure blackbody Earth ~ 255 K (-18°C)

    // 1 bar atmosphere greenhouse boost (+33 K)
    let atm_pressure = 1.0f32;
    let greenhouse_delta = 33.0 * (atm_pressure / 1.0).powf(0.28);
    let t_surf = t_eq as f32 + greenhouse_delta;

    assert!((t_surf - 288.0).abs() < 3.0); // Earth surface ~ 288 K (+15°C)

    let climate = PlanetaryClimate {
        surface_temperature_k: t_surf,
        equilibrium_temperature_k: t_eq as f32,
        greenhouse_delta_k: greenhouse_delta,
        albedo: albedo as f32,
        ice_coverage_frac: 0.10,
        cloud_coverage_frac: 0.50,
        climate_regime: ClimateRegime::TemperateHabitable,
    };
    assert_eq!(climate.climate_regime, ClimateRegime::TemperateHabitable);
}

#[test]
fn test_ice_albedo_feedback_snowball_regime() {
    use protostellar::simulation::components::ClimateRegime;

    // Outer terrestrial planet at 1.8 AU
    let star_temp = 5778.0f64;
    let star_radius = protostellar::utils::constants::SOLAR_RADIUS_AU;
    let r_au = 1.8f64;
    let albedo = 0.65f64; // High glacial albedo

    let t_eq = star_temp * (star_radius / (2.0 * r_au)).sqrt() * (1.0 - albedo).powf(0.25);
    let t_surf = t_eq + 10.0; // Thin cold atmosphere

    assert!(t_surf < 240.0);
    let regime = if t_surf < 260.0 {
        ClimateRegime::SnowballIceAge
    } else {
        ClimateRegime::TemperateHabitable
    };
    assert_eq!(regime, ClimateRegime::SnowballIceAge);
}
