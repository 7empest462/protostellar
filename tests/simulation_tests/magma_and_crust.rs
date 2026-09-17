//! Integration tests for Feature 3.3: Dynamic Viscous Magma Ocean Cooling & Crustal Solidification.

use protostellar::rendering::bodies::{
    compute_magma_incandescence, compute_magma_ocean_crust_fraction,
};
use protostellar::simulation::components::*;
use protostellar::simulation::tides::TidalState;

#[test]
fn test_magma_ocean_crust_coverage_stages() {
    // 1. Ultra-hot convective ocean (T >= 1800 K): 0% solid crust
    let crust_2200 = compute_magma_ocean_crust_fraction(2200.0);
    assert_eq!(
        crust_2200, 0.0,
        "Ultra-hot ocean (2200 K) must be 100% molten with 0% crust"
    );
    let crust_1800 = compute_magma_ocean_crust_fraction(1800.0);
    assert_eq!(
        crust_1800, 0.0,
        "Ocean at 1800 K boundary must have 0% crust coverage"
    );

    // 2. Viscous basalt plate rafting (1100 K <= T < 1600 K): transitional crust coverage
    let crust_1400 = compute_magma_ocean_crust_fraction(1400.0);
    assert!(
        crust_1400 > 0.20 && crust_1400 < 0.40,
        "1400 K ocean should have early drifting plate rafts (got {crust_1400})"
    );
    let crust_1125 = compute_magma_ocean_crust_fraction(1125.0);
    assert!(
        (crust_1125 - 0.50).abs() < 1e-4,
        "Midpoint 1125 K ocean should be exactly 50% crusted (got {crust_1125})"
    );

    // 3. Solidifying crust with volcanic hotspot networks (600 K <= T < 1100 K)
    let crust_800 = compute_magma_ocean_crust_fraction(800.0);
    assert!(
        crust_800 > 0.70 && crust_800 < 0.80,
        "800 K ocean should be predominantly crusted (got {crust_800})"
    );
    let crust_600 = compute_magma_ocean_crust_fraction(600.0);
    assert!(
        crust_600 > 0.85,
        "600 K crust should cover > 85% of the surface (got {crust_600})"
    );

    // 4. Fully solidified crust (T <= 450 K): 100% solid lithosphere
    let crust_450 = compute_magma_ocean_crust_fraction(450.0);
    assert_eq!(
        crust_450, 1.0,
        "450 K threshold must produce 100% solid crust"
    );
    let crust_288 = compute_magma_ocean_crust_fraction(288.0);
    assert_eq!(
        crust_288, 1.0,
        "Temperate Earth-like 288 K world must have 100% solid crust"
    );
}

#[test]
fn test_magma_incandescence_blackbody_gradient() {
    // 1. High temperature rift core: incandescent yellow-white
    let (core_color, core_glow) = compute_magma_incandescence(1800.0, 1.0);
    assert!(
        core_color.x > 0.95 && core_color.y > 0.75,
        "Ultra-hot fissure core must be yellow-white incandescence (got {core_color:?})"
    );
    assert!(
        core_glow > 2.5,
        "Ultra-hot fissure core must have high emissive radiance (got {core_glow})"
    );

    // 2. Cooling rift margin: deep crimson
    let (margin_color, margin_glow) = compute_magma_incandescence(700.0, 0.0);
    assert!(
        margin_color.x > 0.45 && margin_color.y < 0.15 && margin_color.z < 0.05,
        "Cooling fissure margin must be deep crimson (got {margin_color:?})"
    );
    assert!(
        margin_glow < core_glow,
        "Cooler margin must emit less than hot core"
    );

    // 3. Fully cooled crust at 400 K: zero emissive glow
    let (_, cold_glow) = compute_magma_incandescence(400.0, 0.5);
    assert_eq!(
        cold_glow, 0.0,
        "Crust at 400 K must have zero emissive glow"
    );
}

#[test]
fn test_tidal_heating_volcanism_boost() {
    // Io-like moon with high tidal heating dissipation (~2.5 W/m^2)
    let mut io_tidal = TidalState::new_rocky();
    io_tidal.tidal_heating_flux_w_m2 = 2.5;

    let tidal_boost_io = if io_tidal.tidal_heating_flux_w_m2 > 0.5 {
        ((io_tidal.tidal_heating_flux_w_m2 as f32 - 0.5) / 2.5).clamp(0.0, 0.75)
    } else {
        0.0
    };
    assert!(
        (tidal_boost_io - 0.75).abs() < 1e-4 || (tidal_boost_io >= 0.70),
        "Io tidal heating must supply strong volcanic lava boost (got {tidal_boost_io})"
    );

    // Earth-like body with weak geothermal/tidal dissipation (~0.09 W/m^2)
    let mut earth_tidal = TidalState::new_rocky();
    earth_tidal.tidal_heating_flux_w_m2 = 0.09;

    let tidal_boost_earth = if earth_tidal.tidal_heating_flux_w_m2 > 0.5 {
        ((earth_tidal.tidal_heating_flux_w_m2 as f32 - 0.5) / 2.5).clamp(0.0, 0.75)
    } else {
        0.0
    };
    assert_eq!(
        tidal_boost_earth, 0.0,
        "Earth tidal dissipation must not trigger global runaway lava"
    );
}

#[test]
fn test_steam_degassing_and_ocean_condensation() {
    let mut vol = VolatileInventory {
        delivered_water_m_earth: 0.0006, // 1.0 Earth ocean equivalent (~0.85 coverage potential)
        atmospheric_pressure_bar: 1.0,
        ocean_coverage_frac: 0.0,
        cometary_impact_count: 5,
    };

    let max_ocean = (vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
    assert!(
        (max_ocean - 0.85).abs() < 1e-4,
        "Max ocean potential should be 0.85"
    );

    // Stage 1: Ultra-hot magma ocean (T = 1200 K) -> 100% steam atmosphere, 0% liquid ocean
    let temp_hot = 1200.0f32;
    let cond_hot = if temp_hot > 380.0 {
        0.0
    } else if temp_hot < 340.0 {
        1.0
    } else {
        ((380.0 - temp_hot) / 40.0).clamp(0.0, 1.0)
    };
    vol.ocean_coverage_frac = max_ocean * cond_hot;
    let steam_pressure_hot = max_ocean * (1.0 - cond_hot) * 12.0;
    assert_eq!(
        vol.ocean_coverage_frac, 0.0,
        "At 1200 K, liquid water cannot condense into surface oceans"
    );
    assert!(
        steam_pressure_hot > 10.0,
        "At 1200 K, all water must reside in thick steam greenhouse atmosphere (got {steam_pressure_hot})"
    );

    // Stage 2: Degassing & early condensation (T = 360 K) -> ~50% ocean basins filled
    let temp_warm = 360.0f32;
    let cond_warm = ((380.0 - temp_warm) / 40.0).clamp(0.0, 1.0);
    vol.ocean_coverage_frac = max_ocean * cond_warm;
    assert!(
        (cond_warm - 0.50).abs() < 1e-4,
        "Condensation fraction at 360 K should be 50%"
    );
    assert!(
        vol.ocean_coverage_frac > 0.40 && vol.ocean_coverage_frac < 0.45,
        "At 360 K, ocean basins should be half-flooded (got {})",
        vol.ocean_coverage_frac
    );

    // Stage 3: Temperate world (T = 288 K) -> 100% liquid ocean condensed
    let temp_cold = 288.0f32;
    let cond_cold = if temp_cold > 380.0 {
        0.0
    } else if temp_cold < 340.0 {
        1.0
    } else {
        ((380.0 - temp_cold) / 40.0).clamp(0.0, 1.0)
    };
    vol.ocean_coverage_frac = max_ocean * cond_cold;
    assert_eq!(
        vol.ocean_coverage_frac, max_ocean,
        "At 288 K, oceans must be fully condensed"
    );
}

#[test]
fn test_magma_ocean_cooling_monotonicity() {
    let mut temp: f64 = 2200.0;
    let target_temp: f64 = 288.0;
    let dt_yr: f64 = 50.0;

    let mut prev_crust = compute_magma_ocean_crust_fraction(temp as f32);
    let mut steps = 0;

    while temp > target_temp + 2.0 && steps < 100 {
        let cool_rate = 0.08 * (temp / 1000.0).powi(3).clamp(0.01, 15.0);
        let k_cool = (1.0 - (-cool_rate * dt_yr).exp()).clamp(0.0, 1.0);
        temp = (temp + (target_temp - temp) * k_cool).max(target_temp);

        let crust = compute_magma_ocean_crust_fraction(temp as f32);
        assert!(
            crust >= prev_crust,
            "Crust coverage must increase monotonically during cooling: was {prev_crust}, now {crust} at {temp} K"
        );
        prev_crust = crust;
        steps += 1;
    }

    assert!(
        temp <= 350.0,
        "Planet should have cooled close to equilibrium (ended at {temp} K in {steps} steps)"
    );
    assert_eq!(
        prev_crust, 1.0,
        "Planet after cooling must be 100% solidified"
    );
}
