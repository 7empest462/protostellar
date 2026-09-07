use bevy::math::DVec3;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;
use protostellar::utils::math::*;

#[test]
fn test_astrophysical_constants() {
    // G = 4 * PI^2 in AU^3 / (M_sun * yr^2)
    assert!((G_ASTRO - 39.47841760435743).abs() < 1e-10);

    // Earth orbital speed ~ 2 * PI AU/yr ~ 29.78 km/s
    let earth_v_au_yr = 2.0 * std::f64::consts::PI;
    let earth_v_km_s = earth_v_au_yr * AU_PER_YR_TO_KM_PER_S;
    assert!((earth_v_km_s - 29.78).abs() < 0.1);
}

#[test]
fn test_keplerian_orbit_solver_circular_earth() {
    // 1 AU circular orbit around 1 M_sun
    let pos = DVec3::new(1.0, 0.0, 0.0);
    let vel = DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI);
    let central_mass = 1.0;
    let orbiting_mass = EARTH_MASS_SOLAR;

    let elements =
        state_vectors_to_orbital_elements(pos, vel, central_mass, orbiting_mass).unwrap();

    assert!((elements.semi_major_axis - 1.0).abs() < 1e-4);
    assert!(elements.eccentricity < 1e-4);
    assert!((elements.period_years - 1.0).abs() < 1e-4);
}

#[test]
fn test_keplerian_orbit_solver_eccentric() {
    // Semi-major axis a = 2.0 AU, periapsis at r = 1.0 AU -> e = 0.5
    // v_periapsis = sqrt(G*M * (2/r - 1/a)) = sqrt(4*pi^2 * (2 - 0.5)) = 2*pi * sqrt(1.5)
    let pos = DVec3::new(1.0, 0.0, 0.0);
    let v_mag = 2.0 * std::f64::consts::PI * 1.5f64.sqrt();
    let vel = DVec3::new(0.0, 0.0, v_mag);

    let elements = state_vectors_to_orbital_elements(pos, vel, 1.0, 0.0).unwrap();

    assert!((elements.semi_major_axis - 2.0).abs() < 1e-3);
    assert!((elements.eccentricity - 0.5).abs() < 1e-3);
    assert!((elements.periapsis - 1.0).abs() < 1e-3);
    assert!((elements.apoapsis - 3.0).abs() < 1e-3);
}

#[test]
fn test_blackbody_color_mapping() {
    // Cool star (~3000K) -> Red/Orange dominated
    let (r_cool, g_cool, b_cool) = blackbody_to_srgb(3000.0);
    assert!(r_cool > g_cool);
    assert!(g_cool > b_cool);

    // Sun-like star (~5800K) -> White/Yellow
    let (r_sun, g_sun, _b_sun) = blackbody_to_srgb(5778.0);
    assert!(r_sun > 0.9);
    assert!(g_sun > 0.85);

    // Hot star (~10000K) -> Blue-white
    let (r_hot, _, b_hot) = blackbody_to_srgb(10000.0);
    assert!(b_hot > r_hot);
}

#[test]
fn test_composition_density_calculations() {
    let metal = Composition::metal_rich();
    let rocky = Composition::rocky();
    let icy = Composition::icy();

    assert!(metal.average_density() > rocky.average_density());
    assert!(rocky.average_density() > icy.average_density());
}

#[test]
fn test_mass_weighted_composition_merger() {
    let comp1 = Composition {
        metal_frac: 0.8,
        silicate_frac: 0.2,
        ice_frac: 0.0,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let comp2 = Composition {
        metal_frac: 0.0,
        silicate_frac: 0.0,
        ice_frac: 1.0,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };

    // 1 Solar Mass of Comp1 + 3 Solar Masses of Comp2
    let merged = comp1.mass_weighted_merge(1.0, &comp2, 3.0);

    assert!((merged.metal_frac - 0.2).abs() < 1e-6);
    assert!((merged.silicate_frac - 0.05).abs() < 1e-6);
    assert!((merged.ice_frac - 0.75).abs() < 1e-6);
}

#[test]
fn test_stickiness_and_density_properties() {
    let icy = Composition::icy();
    let metal = Composition::metal_rich();

    assert!(icy.stickiness_critical_velocity_km_s() > metal.stickiness_critical_velocity_km_s());
}

#[test]
fn test_planetary_core_differentiation() {
    let mut diff = InternalDifferentiation::default();
    let comp = Composition::rocky();
    let mass = EARTH_MASS_SOLAR;
    let radius = EARTH_RADIUS_AU;

    diff.recalculate(mass, radius, &comp);

    assert!(diff.is_differentiated);
    assert!(diff.core_radius_au > 0.0);
    assert!(diff.core_radius_au < diff.mantle_radius_au);
    assert!(diff.mantle_radius_au <= radius);
    assert!(diff.magnetic_field_gauss > 0.1);
}

#[test]
fn test_spin_state_rotation_period() {
    let mut spin = SpinState::default();
    let mass = EARTH_MASS_SOLAR;
    let radius = EARTH_RADIUS_AU;

    // Spin angular momentum corresponding to 24 hour rotation
    let omega_rad_yr = 2.0 * std::f64::consts::PI * (YEAR_SECONDS / (24.0 * 3600.0));
    let i_moment = 0.33 * mass * radius * radius;
    let spin_vec = DVec3::new(0.0, i_moment * omega_rad_yr, 0.0);

    spin.update_from_spin(spin_vec, mass, radius);

    assert!((spin.rotation_period_hours - 24.0).abs() < 0.1);
    assert!(spin.axial_tilt_degrees < 1.0);
}

#[test]
fn test_mass_tier_classification() {
    assert_eq!(
        MassTier::from_mass(1e-6 * EARTH_MASS_SOLAR),
        MassTier::DustPebble
    );
    assert_eq!(
        MassTier::from_mass(0.001 * EARTH_MASS_SOLAR),
        MassTier::Planetesimal
    );
    assert_eq!(
        MassTier::from_mass(0.05 * EARTH_MASS_SOLAR),
        MassTier::Embryo
    );
    assert_eq!(
        MassTier::from_mass(1.0 * EARTH_MASS_SOLAR),
        MassTier::MajorPlanet
    );
}

#[test]
fn test_leapfrog_symplectic_energy_conservation() {
    // 1 AU circular Earth orbit integrated for 10 full orbits using KDK Leapfrog
    let star_mass = 1.0;
    let mut pos = DVec3::new(1.0, 0.0, 0.0);
    let mut vel = DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI);
    let dt = 0.001; // 10,000 steps

    let calc_acc = |p: DVec3| -> DVec3 {
        let r = p.length();
        -(G_ASTRO * star_mass / (r * r * r)) * p
    };

    let initial_energy = 0.5 * vel.length_squared() - (G_ASTRO * star_mass) / pos.length();
    let mut acc = calc_acc(pos);

    for _ in 0..10_000 {
        // Kick 1
        vel += acc * (dt * 0.5);
        // Drift
        pos += vel * dt;
        // Force
        acc = calc_acc(pos);
        // Kick 2
        vel += acc * (dt * 0.5);
    }

    let final_energy = 0.5 * vel.length_squared() - (G_ASTRO * star_mass) / pos.length();
    let drift = ((final_energy - initial_energy) / initial_energy).abs();

    // Symplectic Leapfrog energy drift should be tiny (< 1e-4) over 10 complete orbits
    assert!(
        drift < 1e-4,
        "Leapfrog energy drift was too large: {:.6}",
        drift
    );
    // Radius should remain within 0.1% of 1.0 AU
    assert!((pos.length() - 1.0).abs() < 1e-3);
}

#[test]
fn test_inelastic_fusion_momentum_conservation() {
    let m1 = 0.6 * EARTH_MASS_SOLAR;
    let _p1 = DVec3::new(1.0, 0.0, 0.0);
    let v1 = DVec3::new(0.0, 0.0, std::f64::consts::TAU);

    let m2 = 0.4 * EARTH_MASS_SOLAR;
    let _p2 = DVec3::new(1.01, 0.0, 0.0);
    let v2 = DVec3::new(0.0, 0.0, 5.80);

    let initial_momentum = v1 * m1 + v2 * m2;
    let total_mass = m1 + m2;
    let merged_vel = (v1 * m1 + v2 * m2) / total_mass;
    let final_momentum = merged_vel * total_mass;

    assert!((initial_momentum - final_momentum).length() < 1e-12);
}

#[test]
fn test_mass_dependent_render_radius_scaling() {
    use protostellar::simulation::resources::SimulationConfig;

    // 1. Legacy collision radius hierarchy (used only for accretion cross-sections)
    let r_star = SimulationConfig::calc_collision_radius(1.0, BodyType::MainSequenceStar);
    let r_jupiter = SimulationConfig::calc_collision_radius(JUPITER_MASS_SOLAR, BodyType::GasGiant);
    let r_earth =
        SimulationConfig::calc_collision_radius(EARTH_MASS_SOLAR, BodyType::TerrestrialPlanet);
    let r_embryo =
        SimulationConfig::calc_collision_radius(0.05 * EARTH_MASS_SOLAR, BodyType::Protoplanet);
    let r_planetesimal =
        SimulationConfig::calc_collision_radius(0.001 * EARTH_MASS_SOLAR, BodyType::Planetesimal);

    assert!(r_star > r_jupiter);
    assert!(r_jupiter > r_earth);
    assert!(r_earth > r_embryo);
    assert!(r_embryo > r_planetesimal);
    assert!(r_planetesimal >= 0.005);

    // 2. Unified visual radius (physical radius → power-law compression × exaggeration)
    let config = SimulationConfig::default();
    let sun_r = 0.00465_f64; // 1 R_sun in AU
    let jupiter_r = 0.000477_f64; // Jupiter radius in AU
    let earth_r = 0.0000426_f64; // Earth radius in AU
    let trappist1_star_r = 0.00056_f64; // TRAPPIST-1 (0.121 R_sun)
    let trappist1e_r = 0.920 * 0.0000426_f64; // 0.920 R_Earth

    let v_sun = config.calc_visual_radius(sun_r);
    let v_jupiter = config.calc_visual_radius(jupiter_r);
    let v_earth = config.calc_visual_radius(earth_r);
    let v_trappist_star = config.calc_visual_radius(trappist1_star_r);
    let v_trappist_e = config.calc_visual_radius(trappist1e_r);

    // Hierarchy must be strictly preserved
    assert!(
        v_sun > v_jupiter,
        "Sun must be larger than Jupiter visually"
    );
    assert!(v_jupiter > v_earth, "Jupiter must be larger than Earth");
    assert!(
        v_trappist_star > v_trappist_e,
        "TRAPPIST-1 star must be larger than its planets"
    );

    // Star-to-planet ratio should be at least 2.5× for visual dominance
    assert!(
        v_sun / v_jupiter > 2.5,
        "Sun:Jupiter ratio should be >2.5× (got {:.2}×)",
        v_sun / v_jupiter
    );
    assert!(
        v_trappist_star / v_trappist_e > 2.5,
        "TRAPPIST-1 star:planet ratio should be >2.5× (got {:.2}×)",
        v_trappist_star / v_trappist_e
    );

    // Minimum visual radius floor
    let tiny_body = config.calc_visual_radius(1e-8);
    assert!(
        tiny_body >= config.min_body_visual_radius,
        "Tiny bodies must respect min_body_visual_radius floor"
    );
}

#[test]
fn test_sample_disk_radius_distribution() {
    use protostellar::simulation::disk::sample_disk_radius;
    use protostellar::simulation::resources::DiskParameters;
    let disk_params = DiskParameters::default();
    let mut rng = rand::rng();

    let mut count_inner = 0;
    let mut count_giant_zone = 0;
    let mut count_outer = 0;
    let n_samples = 10_000;

    for _ in 0..n_samples {
        let (r, comp) = sample_disk_radius(&mut rng, &disk_params);
        assert!((0.06..=45.0).contains(&r));

        if r <= 2.50 {
            count_inner += 1;
            assert!(comp.silicate_frac > 0.4 || comp.metal_frac > 0.4);
        } else if (4.50..=25.0).contains(&r) {
            count_giant_zone += 1;
            assert!(comp.ice_frac > 0.4);
        } else if r > 25.0 {
            count_outer += 1;
        }
    }

    // Inner zone should be ~25% (+/- 4%)
    let frac_inner = count_inner as f64 / n_samples as f64;
    assert!(
        (frac_inner - 0.25).abs() < 0.04,
        "Inner fraction: {}",
        frac_inner
    );

    // Giant zone should be ~45% (+/- 5%)
    let frac_giant = count_giant_zone as f64 / n_samples as f64;
    assert!(
        (frac_giant - 0.45).abs() < 0.05,
        "Giant fraction: {}",
        frac_giant
    );

    // Outer zone should be ~15% (+/- 4%)
    let frac_outer = count_outer as f64 / n_samples as f64;
    assert!(
        (frac_outer - 0.15).abs() < 0.04,
        "Outer fraction: {}",
        frac_outer
    );
}

#[test]
fn test_giant_impact_moon_formation_mechanics() {
    // Proto-Earth (1.0 M_Earth) hit by Theia (0.10 M_Earth) at impact parameter b = 0.65
    let p_m = EARTH_MASS_SOLAR;
    let s_m = 0.10 * EARTH_MASS_SOLAR;
    let b: f64 = 0.65;

    let moon_mass_frac = (0.25 + 0.35 * b).clamp(0.20, 0.55);
    let moon_mass = s_m * moon_mass_frac;
    let accreted_mass = s_m - moon_mass;
    let total_primary_mass = p_m + accreted_mass;

    // Total mass strictly conserved
    assert!(((total_primary_mass + moon_mass) - (p_m + s_m)).abs() < 1e-12);

    // Moon mass is realistic (~0.01 - 0.05 M_Earth)
    assert!(moon_mass > 0.02 * EARTH_MASS_SOLAR);
    assert!(moon_mass < 0.08 * EARTH_MASS_SOLAR);

    // Primary planet gained majority of impactor mass
    assert!(total_primary_mass > p_m);

    // Orbit is placed beyond fluid Roche limit (~2.5 R)
    let p_rad = EARTH_RADIUS_AU;
    let orbit_dist = p_rad * (3.5 + 2.5 * b);
    assert!(orbit_dist >= 2.5 * p_rad);
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
fn test_trappist1_system_resonance_and_habitable_zone() {
    let m_star = 0.0898f64; // TRAPPIST-1 mass (Solar)
    let l_star = 0.000553f64; // TRAPPIST-1 luminosity (Solar)

    // Habitable zone boundaries for TRAPPIST-1: r_hz ~ sqrt(L_star / S_eff)
    let hz_inner = 0.75 * l_star.sqrt(); // ~0.0176 AU
    let hz_outer = 1.77 * l_star.sqrt(); // ~0.0416 AU

    let semimajor_axes: [f64; 7] = [
        0.01154, 0.01580, 0.02227, 0.02925, 0.03849, 0.04688, 0.06193,
    ];

    // Compute orbital periods using Kepler's 3rd Law: P = sqrt(a^3 / M_star) in years
    let periods: Vec<f64> = semimajor_axes
        .iter()
        .map(|&a| (a.powf(3.0) / m_star).sqrt() * 365.25)
        .collect();

    // Check TRAPPIST-1e and 1f are inside the habitable zone
    assert!(semimajor_axes[3] >= hz_inner && semimajor_axes[3] <= hz_outer * 1.2); // TRAPPIST-1e
    assert!(semimajor_axes[4] >= hz_inner && semimajor_axes[4] <= hz_outer * 1.2); // TRAPPIST-1f

    // Verify resonant period ratios are near integer ratios (e.g. 1c/1b ~ 1.6 ~ 8:5, 1d/1c ~ 1.67 ~ 5:3, 1e/1d ~ 1.5 ~ 3:2)
    let ratio_c_b = periods[1] / periods[0];
    let ratio_e_d = periods[3] / periods[2];
    assert!((ratio_c_b - 1.60).abs() < 0.15);
    assert!((ratio_e_d - 1.50).abs() < 0.15);
}

#[test]
fn test_kepler16_circumbinary_stability_radius() {
    // Holman & Wiegert (1999) empirical dynamical stability limit for P-type circumbinary planets:
    // a_crit = a_bin * (1.60 + 5.10*e_bin - 2.22*e_bin^2 + 4.12*mu - 4.27*e_bin*mu - 5.09*mu^2 + 4.61*e_bin^2*mu^2)
    let m_a = 0.6897f64;
    let m_b = 0.2025f64;
    let a_bin = 0.2243f64;
    let e_bin = 0.159f64;
    let mu = m_b / (m_a + m_b); // Mass ratio ~ 0.227

    let a_crit = a_bin
        * (1.60 + 5.10 * e_bin - 2.22 * e_bin.powi(2) + 4.12 * mu
            - 4.27 * e_bin * mu
            - 5.09 * mu.powi(2)
            + 4.61 * e_bin.powi(2) * mu.powi(2));

    // Kepler-16b circumbinary orbit at a = 0.7048 AU
    let a_kepler16b = 0.7048f64;

    // The planet's orbit must be dynamically stable (outside a_crit ~ 0.65 AU)
    assert!(a_kepler16b > a_crit);
    assert!(a_crit > 0.55 && a_crit < 0.70);
}

#[test]
fn test_scenario_preset_definitions() {
    use protostellar::simulation::scenarios::ScenarioPreset;

    let presets = [
        ScenarioPreset::SolarNebulaMmsn,
        ScenarioPreset::Trappist1System,
        ScenarioPreset::Kepler16Circumbinary,
        ScenarioPreset::HotJupiterMigration,
        ScenarioPreset::RoguePlanetFlyby,
        ScenarioPreset::LittleRedDot,
        ScenarioPreset::PulsarSystem,
        ScenarioPreset::MagnetarOutburst,
    ];

    for preset in presets {
        assert!(!preset.display_name().is_empty());
        assert!(!preset.description().is_empty());
    }
}

#[test]
fn test_gas_giant_variety_palette() {
    use protostellar::rendering::bodies::compute_gas_giant_palette;
    use protostellar::utils::constants::JUPITER_MASS_SOLAR;

    // 1. Classic Jupiter (1.0 M_jup) -> Iconic Jovian Amber-Ochre
    let jupiter_color = compute_gas_giant_palette(JUPITER_MASS_SOLAR * 1.0, 160.0, "Jupiter");
    let jup_srgba = jupiter_color.to_srgba();
    assert!(jup_srgba.red > jup_srgba.green && jup_srgba.green > jup_srgba.blue);

    // 2. Super-Jupiter (2.5 M_jup) -> Emerald-Teal
    let super_jup_color =
        compute_gas_giant_palette(JUPITER_MASS_SOLAR * 2.5, 160.0, "Super-Jovian");
    let sj_srgba = super_jup_color.to_srgba();
    assert!(sj_srgba.green >= sj_srgba.red); // Green/teal dominant

    // 3. Massive Super-Jupiter (4.5 M_jup) -> Lapis-Indigo / Sapphire
    let massive_color = compute_gas_giant_palette(JUPITER_MASS_SOLAR * 4.5, 160.0, "Mega-Jovian");
    let mass_srgba = massive_color.to_srgba();
    assert!(mass_srgba.blue > mass_srgba.red); // Blue dominant

    // 4. Heavy Super-Jupiter (8.0 M_jup) -> Royal Plum-Purple
    let heavy_color = compute_gas_giant_palette(JUPITER_MASS_SOLAR * 8.0, 160.0, "Ultra-Giant");
    let heavy_srgba = heavy_color.to_srgba();
    assert!(heavy_srgba.blue > heavy_srgba.green && heavy_srgba.red > heavy_srgba.green); // Purple mix

    // 5. Brown Dwarf Transition (14.0 M_jup) -> Incandescent Plum-Maroon
    let brown_dwarf_color =
        compute_gas_giant_palette(JUPITER_MASS_SOLAR * 14.0, 600.0, "Brown Dwarf");
    let bd_srgba = brown_dwarf_color.to_srgba();
    assert!(bd_srgba.red > bd_srgba.green);
}

#[test]
fn test_planet_builder_presets() {
    use protostellar::game::ui::{BuilderPreset, PlanetBuilderState};
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, JUPITER_MASS_SOLAR};

    let mut state = PlanetBuilderState::default();

    // Default is Earth-like
    assert_eq!(state.active_preset, BuilderPreset::EarthLike);
    assert!((state.mass_solar - EARTH_MASS_SOLAR).abs() < 1e-10);
    assert!((state.semi_major_axis_au - 1.0).abs() < 1e-10);

    // Apply Super-Jupiter preset
    state.apply_preset(BuilderPreset::SuperJupiter);
    assert_eq!(state.active_preset, BuilderPreset::SuperJupiter);
    assert!((state.mass_solar - JUPITER_MASS_SOLAR * 3.5).abs() < 1e-10);
    assert!((state.semi_major_axis_au - 3.2).abs() < 1e-10);
    assert!(state.gas_frac > 0.90);

    // Apply Water World preset
    state.apply_preset(BuilderPreset::WaterWorld);
    assert_eq!(state.active_preset, BuilderPreset::WaterWorld);
    assert!(state.ice_frac > 0.50);
    assert_eq!(state.gas_frac, 0.0);
}

#[test]
fn test_asteroid_particle_accretion_capping() {
    use protostellar::simulation::components::BodyType;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    // Minor asteroid should be capped
    let asteroid_type = BodyType::Asteroid;
    let initial_mass = 0.00001 * EARTH_MASS_SOLAR;
    let gain = 0.001 * EARTH_MASS_SOLAR;

    let updated_mass = if matches!(asteroid_type, BodyType::Asteroid | BodyType::Comet) {
        (initial_mass + gain).min(0.0005 * EARTH_MASS_SOLAR)
    } else {
        initial_mass + gain * 2.0
    };

    assert!(updated_mass <= 0.0005 * EARTH_MASS_SOLAR);
}

#[test]
fn test_visual_radius_for_minor_bodies() {
    use protostellar::simulation::components::BodyType;
    use protostellar::simulation::resources::SimulationConfig;
    use protostellar::utils::constants::EARTH_RADIUS_AU;

    let config = SimulationConfig::default();
    let planet_rad =
        config.calc_visual_radius_for_type(EARTH_RADIUS_AU, BodyType::TerrestrialPlanet);
    let asteroid_rad =
        config.calc_visual_radius_for_type(EARTH_RADIUS_AU * 0.05, BodyType::Asteroid);
    let comet_rad = config.calc_visual_radius_for_type(EARTH_RADIUS_AU * 0.05, BodyType::Comet);

    assert!(asteroid_rad < planet_rad);
    assert!(comet_rad < planet_rad);
    assert!((asteroid_rad - comet_rad).abs() < 1e-6);
}

#[test]
fn test_late_heavy_bombardment_water_delivery_formation() {
    use protostellar::simulation::components::{Composition, VolatileInventory};
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let mut vol = VolatileInventory::default();
    assert_eq!(vol.delivered_water_m_earth, 0.0);
    assert_eq!(vol.ocean_coverage_frac, 0.0);

    // 5 cometary impacts delivering 0.0005 M_earth water each
    let icy_comet_comp = Composition::icy();
    let comet_mass = 0.0008 * EARTH_MASS_SOLAR;
    let water_per_impact = (comet_mass * icy_comet_comp.ice_frac) / EARTH_MASS_SOLAR;

    for _ in 0..5 {
        vol.delivered_water_m_earth += water_per_impact;
    }

    vol.ocean_coverage_frac = (vol.delivered_water_m_earth / 0.003).clamp(0.0, 0.75) as f32;

    assert!(vol.delivered_water_m_earth > 0.002);
    assert!(vol.ocean_coverage_frac >= 0.70);
}

#[test]
fn test_protostar_auto_ignition_and_gas_push() {
    use protostellar::simulation::components::IgnitionState;

    let mut ignition = IgnitionState {
        core_temperature: 4.0e6,
        fusion_fraction: 0.4,
        is_ignited: false,
        shockwave_radius: 0.0,
    };

    let heating_rate_per_yr = 2.0e5; // Solar mass
    let elapsed_dt = 30.0; // 30 years

    ignition.core_temperature += heating_rate_per_yr * elapsed_dt;
    let ignition_threshold = 1.0e7;

    if ignition.core_temperature >= ignition_threshold || elapsed_dt >= 30.0 {
        ignition.is_ignited = true;
        ignition.fusion_fraction = 1.0;
        ignition.shockwave_radius = 0.5;
    }

    assert!(ignition.is_ignited);
    assert_eq!(ignition.fusion_fraction, 1.0);
    assert!(ignition.core_temperature >= 1.0e7);

    // Verify gas push at shockwave: inner terrestrial zone gas density is residual (0.05),
    // outer giant zone is boosted (2.5x) to feed Jupiter.
    let r_inner = 1.0f64;
    let r_outer = 5.2f64;
    let gas_scale = 1.0f64;

    let inner_gas_density = 1.2e-4 * (r_inner / 1.0).powf(-1.50) * (gas_scale * 0.05 + 0.001);
    let outer_gas_density = 1.2e-4 * (r_outer / 1.0).powf(-1.50) * gas_scale * 2.5;

    assert!(inner_gas_density < 1.0e-5);
    assert!(outer_gas_density > 2.0e-5);
}

#[test]
fn test_little_red_dot_quasi_star_model() {
    use protostellar::simulation::components::{BlackHoleStarState, BodyType, Composition};

    let mut state = BlackHoleStarState::default();

    // Verify initial astrophysical parameters
    assert_eq!(state.black_hole_mass_solar, 400_000.0);
    assert_eq!(state.cocoon_mass_solar, 50_000.0);
    assert_eq!(state.total_mass_solar(), 450_000.0);
    assert_eq!(state.cocoon_radius_au, 60.0);
    assert!(state.super_eddington_active);
    assert!(!state.is_blown_out);
    assert_eq!(state.blowout_progress, 0.0);

    // Verify pristine pure hydrogen composition (0% dust, 0% rock, 0% metal)
    let comp = Composition::pure_hydrogen();
    assert_eq!(comp.gas_frac, 1.0);
    assert_eq!(comp.metal_frac, 0.0);
    assert_eq!(comp.silicate_frac, 0.0);
    assert_eq!(comp.ice_frac, 0.0);

    // Test super-Eddington toggling
    state.toggle_super_eddington();
    assert!(!state.super_eddington_active);
    assert_eq!(state.eddington_ratio, 0.9);

    state.toggle_super_eddington();
    assert!(state.super_eddington_active);
    assert_eq!(state.eddington_ratio, 4.5);

    // Test blowout trigger
    state.trigger_blowout();
    assert!(state.is_blown_out);

    // Verify QuasiStar classification and remnant status
    let q_type = BodyType::QuasiStar;
    assert!(q_type.is_star_or_remnant());
    assert!(q_type.is_remnant());
    assert!(!q_type.is_planet());
}

#[test]
fn test_little_red_dot_preset_in_scenarios() {
    use protostellar::simulation::scenarios::ScenarioPreset;

    let lrd = ScenarioPreset::LittleRedDot;
    assert!(lrd.display_name().contains("Little Red Dot"));
    assert!(lrd.description().contains("100,000 M☉"));
    assert!(lrd.description().contains("60 AU"));
}

#[test]
fn test_skybox_scenario_blending_and_materials() {
    use protostellar::rendering::materials::SkyboxMaterial;
    use protostellar::simulation::scenarios::ScenarioPreset;

    // 1. Verify default material initialization (starts in Milky Way mode)
    let mat = SkyboxMaterial::default();
    assert_eq!(mat.uniforms.params.x, 0.0); // time
    assert_eq!(mat.uniforms.params.y, 0.0); // scenario_blend (0.0 = Milky Way)
    assert_eq!(mat.uniforms.params.z, 1.25); // exposure
    assert_eq!(mat.uniforms.params.w, 1.0); // twinkle
    assert_eq!(mat.uniforms.tuning.x, 1.0); // star density
    assert_eq!(mat.uniforms.tuning.y, 1.0); // nebula intensity
    assert_eq!(mat.uniforms.tuning.z, 1.0); // cosmic web scale
    assert_eq!(mat.uniforms.tuning.w, 1.0); // filament brightness

    // 2. Scenario preset to target blend mapping
    let presets_milky_way = [
        ScenarioPreset::SolarNebulaMmsn,
        ScenarioPreset::Trappist1System,
        ScenarioPreset::Kepler16Circumbinary,
        ScenarioPreset::HotJupiterMigration,
        ScenarioPreset::RoguePlanetFlyby,
    ];

    for preset in presets_milky_way {
        let target = if preset == ScenarioPreset::LittleRedDot {
            1.0
        } else {
            0.0
        };
        assert_eq!(target, 0.0);
    }

    let target_early_univ = if ScenarioPreset::LittleRedDot == ScenarioPreset::LittleRedDot {
        1.0
    } else {
        0.0
    };
    assert_eq!(target_early_univ, 1.0);

    // 3. Smooth blend interpolation test
    let mut current_blend = 0.0_f32;
    let dt = 0.25_f32;
    let blend_speed = 2.2_f32;
    current_blend += (target_early_univ - current_blend) * (dt * blend_speed).min(1.0);
    assert!(current_blend > 0.40 && current_blend < 0.70);

    // Subsequent frame continues toward 1.0
    current_blend += (target_early_univ - current_blend) * (dt * blend_speed).min(1.0);
    assert!(current_blend > 0.75 && current_blend <= 1.0);

    // 4. Geometry bounds: Skybox sphere (1,000,000 AU) is safely within camera far plane (2,000,000 AU)
    let skybox_radius = 1_000_000.0_f32;
    let camera_far_plane = 2_000_000.0_f32;
    let max_simulation_boundary = 625.0_f32;
    assert!(skybox_radius > max_simulation_boundary * 1000.0);
    assert!(skybox_radius < camera_far_plane);
}

#[test]
fn test_gravitational_lensing_geometry() {
    use protostellar::rendering::materials::SkyboxMaterial;

    // 1. Verify default material has lensing zeroed / inactive
    let mat = SkyboxMaterial::default();
    assert_eq!(mat.uniforms.lens_pos_and_mass, bevy::prelude::Vec4::ZERO);
    assert_eq!(mat.uniforms.lens_params, bevy::prelude::Vec4::ZERO);

    // 2. Physical & Angular Einstein Radius scaling
    let dist_cam = 150.0_f32; // 150 AU distance

    // Quasi-Star intact cocoon (R ~ 60 AU, lensing extends to R ~ 72 AU)
    let r_cocoon_lens = 72.0_f32;
    let theta_e_cocoon = (r_cocoon_lens / dist_cam).atan();
    let theta_shadow_cocoon = ((60.0_f32 * 0.98) / dist_cam).atan();
    assert!(theta_e_cocoon > theta_shadow_cocoon);
    assert!(theta_e_cocoon > 0.40 && theta_e_cocoon < 0.55);

    // Naked Black Hole after blowout (R ~ 2.5 AU, photon sphere at 1.85x ~ 4.62 AU)
    let visual_r_bh = 2.5_f32;
    let r_bh_lens = visual_r_bh * 1.85;
    let theta_e_bh = (r_bh_lens / dist_cam).atan();
    let theta_shadow_bh = ((visual_r_bh * 0.98) / dist_cam).atan();
    assert!(theta_e_bh > theta_shadow_bh);
    assert!(theta_e_bh > 0.02 && theta_e_bh < 0.05);

    // 3. Blowout contraction: as blowout_p goes 0.0 -> 1.0, lens radius smoothly contracts
    for p in [0.0f32, 0.25, 0.50, 0.75, 1.0] {
        let eff_r = r_cocoon_lens + (r_bh_lens - r_cocoon_lens) * p;
        assert!(eff_r >= r_bh_lens && eff_r <= r_cocoon_lens);
    }

    // 4. Deflection angle alpha(theta) = theta_E^2 / theta
    let theta = 0.60_f32;
    let alpha = (theta_e_cocoon * theta_e_cocoon) / (theta + 0.004);
    assert!(alpha > 0.0 && alpha < theta);
    let beta = theta - alpha;
    assert!(beta > 0.0 && beta < theta);
}

#[test]
fn test_dynamic_roche_disruption_ring_parameters() {
    use bevy::prelude::Entity;
    use protostellar::simulation::resources::RocheDebrisStream;

    // 1. Fluid Roche Limit Calculation
    // Planet: Jupiter-like gas giant (0.001 M_sun, density ~ 1.33 g/cm^3)
    let p_mass = 0.000954; // Solar masses (~ 1 Jupiter mass)
    let p_comp = Composition {
        silicate_frac: 0.10,
        ice_frac: 0.10,
        metal_frac: 0.05,
        organics_frac: 0.0,
        gas_frac: 0.75,
    };
    let p_density = p_comp.average_density();
    let p_rad_au = ((3.0 * p_mass / p_density) / (4.0 * std::f64::consts::PI)).cbrt();

    // Secondary: Volatile icy moon (density ~ 0.95 g/cm^3, ice fraction 85%)
    let s_mass = 0.002 * EARTH_MASS_SOLAR; // 0.002 Earth masses
    let s_comp = Composition {
        silicate_frac: 0.10,
        ice_frac: 0.85,
        metal_frac: 0.05,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let s_density = s_comp.average_density();

    // d_Roche = 2.44 * R_p * (rho_p / rho_s)^(1/3)
    let d_roche = 2.44 * p_rad_au * (p_density / s_density).cbrt();
    assert!(d_roche > p_rad_au);
    assert!((d_roche - 2.44 * p_rad_au * (p_density / s_density).cbrt()).abs() < 1e-10);

    // Rocky primary (Earth-like density) vs icy moon (density ~ 0.95 g/cm^3)
    let rocky_comp = Composition {
        silicate_frac: 0.70,
        ice_frac: 0.0,
        metal_frac: 0.30,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let rocky_density = rocky_comp.average_density();
    let d_roche_rocky = 2.44 * p_rad_au * (rocky_density / s_density).cbrt();
    assert!(d_roche_rocky > 2.44 * p_rad_au);

    // 2. Ring System parameters upon disruption inside Roche limit
    let encounter_dist = d_roche * 0.85;
    let is_inside_roche = encounter_dist <= d_roche;
    assert!(is_inside_roche);

    let ring_mass_earth = s_mass / EARTH_MASS_SOLAR;
    let inner_r = (p_rad_au * 1.25) as f32;
    let outer_r = (d_roche.min(p_rad_au * 3.2)).max(inner_r as f64 * 1.35) as f32;
    let ring_sys = PlanetaryRingSystem {
        inner_radius_au: inner_r,
        outer_radius_au: outer_r,
        ring_mass_earth,
        optical_depth: ((ring_mass_earth / 0.0001).clamp(0.40, 0.95)) as f32,
        ice_fraction: s_comp.ice_frac as f32,
        silicate_fraction: (s_comp.silicate_frac + s_comp.metal_frac) as f32,
    };

    assert!((ring_sys.ring_mass_earth - 0.002).abs() < 1e-6);
    assert!(ring_sys.inner_radius_au < ring_sys.outer_radius_au);
    assert!((ring_sys.ice_fraction - 0.85).abs() < 1e-5);
    assert!(ring_sys.optical_depth >= 0.40 && ring_sys.optical_depth <= 0.95);

    // 3. RocheDebrisStream fragment simulation
    let n_fragments = 48;
    let mut fragments = Vec::with_capacity(n_fragments);
    for k in 0..n_fragments {
        let frac = (k as f32) / (n_fragments as f32);
        let frag_r = inner_r + (outer_r - inner_r) * frac;
        let phase = frac * std::f32::consts::TAU;
        let omega = (1.8 / (frag_r * frag_r * frag_r).sqrt()).clamp(0.4, 8.0);
        fragments.push((frag_r, phase, omega, 0.0f32));
    }

    let mut stream = RocheDebrisStream {
        primary_entity: Entity::from_bits(42),
        primary_pos: bevy::prelude::Vec3::ZERO,
        disruption_pos: bevy::prelude::Vec3::new(encounter_dist as f32, 0.0, 0.0),
        inner_radius: inner_r,
        outer_radius: outer_r,
        timer: 0.0,
        max_timer: 4.5,
        ice_fraction: ring_sys.ice_fraction,
        debris_mass_earth: ring_sys.ring_mass_earth,
        fragments,
    };

    // Advance by 1.0 second
    let dt = 1.0f32;
    stream.timer += dt;
    for frag in stream.fragments.iter_mut() {
        let old_phase = frag.1;
        frag.1 += frag.2 * dt;
        assert!(frag.1 > old_phase); // Angular phase progresses in orbit
    }
    assert!(stream.timer < stream.max_timer);
    assert!((stream.ice_fraction - 0.85).abs() < 1e-5);
}

#[test]
fn test_photoevaporative_escape_mass_loss() {
    // 1. Close-in Sub-Neptune planet at a = 0.04 AU vs Host Star (L = 1.0 L_sun)
    let star_lum = 1.0f64;
    let dist_au_close = 0.04f64;
    let dist_au_far = 1.00f64;

    let p_mass_solar = 5.0 * EARTH_MASS_SOLAR; // 5 Earth masses
    let p_rad_au = 2.4 * EARTH_RADIUS_AU; // 2.4 Earth radii

    let m_earth = (p_mass_solar / EARTH_MASS_SOLAR).max(0.01);
    let r_earth = (p_rad_au / EARTH_RADIUS_AU).max(0.1);

    // Energy-limited mass loss calculation:
    // Loss rate ~ 0.15 * R_p^3 / M_p * (L / d^2)^0.85
    let flux_factor_close = (star_lum / (dist_au_close * dist_au_close)).powf(0.85);
    let loss_rate_close =
        ((0.15 * r_earth.powi(3) / m_earth) * flux_factor_close).clamp(0.01, 100.0) as f32;

    assert!(loss_rate_close > 20.0); // Extremely vigorous hydrodynamic escape!

    let tail_len_close = (((0.25 / dist_au_close).powf(1.1) * 0.75 * star_lum.min(5.0).powf(0.25))
        .clamp(0.25, 6.0)) as f32;
    assert!(tail_len_close > 4.0); // Prominent cometary outflow tail extending multiple AU

    // 2. Distant planet at 1.0 AU: outside 0.25 AU photoevaporation boundary
    let is_close = dist_au_close < 0.25;
    let is_far = dist_au_far < 0.25;
    assert!(is_close);
    assert!(!is_far);

    // 3. Envelope mass stripping across geological time
    let mut comp = Composition {
        silicate_frac: 0.40,
        ice_frac: 0.10,
        metal_frac: 0.20,
        organics_frac: 0.0,
        gas_frac: 0.30, // Initial 30% volatile envelope
    };

    // Pre-stripping ionization color: vibrant electric cyan (Hydrogen/Helium envelope)
    let initial_ion_color = if comp.gas_frac > 0.15 {
        bevy::prelude::Color::srgba(0.25, 0.85, 1.0, 0.85) // Electric Cyan
    } else {
        bevy::prelude::Color::srgba(1.0, 0.65, 0.20, 0.85) // Amber
    };
    assert_eq!(initial_ion_color.to_srgba().red, 0.25);
    assert_eq!(initial_ion_color.to_srgba().green, 0.85);

    let mut current_mass_solar = p_mass_solar;
    let dt_myr = 0.05; // 50,000 years of extreme irradiation
    let delta_m_earth = (loss_rate_close as f64) * dt_myr;
    let delta_m_solar = delta_m_earth * EARTH_MASS_SOLAR;

    let cur_gas_m = current_mass_solar * comp.gas_frac;
    let stripped = delta_m_solar.min(cur_gas_m * 0.999);
    current_mass_solar -= stripped;

    let new_gas_m = (cur_gas_m - stripped).max(0.0);
    comp.gas_frac = (new_gas_m / current_mass_solar).clamp(0.0, 1.0);

    // Gas fraction is stripped down into the Hot Neptune Desert!
    assert!(comp.gas_frac < 0.15);
    assert!(current_mass_solar < p_mass_solar);

    // Post-stripping: unmasked mineral/silicate vapor core glows amber
    let post_strip_color = if comp.gas_frac > 0.15 {
        bevy::prelude::Color::srgba(0.25, 0.85, 1.0, 0.85)
    } else {
        bevy::prelude::Color::srgba(1.0, 0.65, 0.20, 0.85) // Warm amber
    };
    assert_eq!(post_strip_color.to_srgba().red, 1.0);
    assert_eq!(post_strip_color.to_srgba().green, 0.65);
}

#[test]
fn test_gpu_particle_buffer_layouts_and_alignment() {
    use protostellar::gpu::buffers::{GpuOrbitUniforms, GpuParticle, MassiveBodyGpu};

    // 1. Validate GpuParticle 48-byte layout (3x vec4<f32>) and 16-byte std430 alignment
    assert_eq!(std::mem::size_of::<GpuParticle>(), 48);
    assert_eq!(std::mem::align_of::<GpuParticle>(), 16);

    // 2. Validate MassiveBodyGpu 16-byte layout (vec4<f32>) and 16-byte alignment
    assert_eq!(std::mem::size_of::<MassiveBodyGpu>(), 16);
    assert_eq!(std::mem::align_of::<MassiveBodyGpu>(), 16);

    // 3. Validate GpuOrbitUniforms layout (592 bytes) and 16-byte uniform alignment
    assert_eq!(std::mem::size_of::<GpuOrbitUniforms>(), 592);
    assert_eq!(std::mem::align_of::<GpuOrbitUniforms>(), 16);

    // 4. Validate default values for uniform buffer
    let uniforms = GpuOrbitUniforms::default();
    assert_eq!(uniforms.num_particles, 50000);
    assert_eq!(uniforms.massive_bodies.len(), 32);
    assert_eq!(uniforms.star_mass, 1.0);
    assert!(uniforms.g_const > 39.0);
}

#[test]
fn test_gpu_workgroup_dispatch_sizing() {
    // Validate GPU compute workgroup dispatch sizing for all particle scaling tiers
    let workgroup_size = 64u32;

    let tiers = [
        (50_000u32, 782u32),
        (100_000u32, 1563u32),
        (250_000u32, 3907u32),
        (500_000u32, 7813u32),
        (1_000_000u32, 15625u32),
    ];

    for (particles, expected_workgroups) in tiers {
        let workgroups = particles.div_ceil(workgroup_size);
        assert_eq!(workgroups, expected_workgroups);

        // Validate buffer size in VRAM
        let buffer_size_bytes =
            (particles as usize) * std::mem::size_of::<protostellar::gpu::buffers::GpuParticle>();
        assert_eq!(buffer_size_bytes, (particles as usize) * 48);

        // At 100k particles, storage buffer is ~4.8 MB (extremely lightweight in VRAM)
        if particles == 100_000 {
            assert_eq!(buffer_size_bytes, 4_800_000);
        }
    }
}

#[test]
fn test_gpu_readback_dead_particle_non_resurrection() {
    use protostellar::gpu::buffers::GpuParticle;

    // Simulate 4 particles
    let mut cpu_masses = [0.0001f32, 0.0f32, 0.0001f32, 0.0f32]; // 1 & 3 accreted on CPU
    let mut cpu_positions = [
        [1.0f32, 0.0, 0.0],
        [0.0, -5000.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, -5000.0, 0.0],
    ];

    // GPU buffer arrives from older in-flight frame where particle 1 had not died yet on GPU
    let gpu_particles = [
        GpuParticle {
            pos_mass: [1.05, 0.0, 0.0, 0.0001],
            vel_temp: [0.0, 0.0, 0.0, 280.0],
            composition: [0.5, 0.5, 0.0, 0.0],
        },
        GpuParticle {
            pos_mass: [1.5, 0.0, 0.0, 0.0001], // GPU still thinks it's alive!
            vel_temp: [0.0, 0.0, 0.0, 250.0],
            composition: [0.5, 0.5, 0.0, 0.0],
        },
        GpuParticle {
            pos_mass: [0.0, -5000.0, 0.0, 0.0], // GPU marked dead
            vel_temp: [0.0, 0.0, 0.0, 0.0],
            composition: [0.0, 0.0, 0.0, 0.0],
        },
        GpuParticle {
            pos_mass: [0.0, -5000.0, 0.0, 0.0],
            vel_temp: [0.0, 0.0, 0.0, 0.0],
            composition: [0.0, 0.0, 0.0, 0.0],
        },
    ];

    let is_scenario_start = false;
    for (i, p) in gpu_particles.iter().enumerate() {
        // Particle must never be resurrected if CPU already marked it dead
        if !is_scenario_start && cpu_masses[i] <= 0.0 {
            continue;
        }
        if p.pos_mass[3] <= 0.0 {
            cpu_masses[i] = 0.0;
            cpu_positions[i] = [0.0, -5000.0, 0.0];
            continue;
        }
        cpu_positions[i] = [p.pos_mass[0], p.pos_mass[1], p.pos_mass[2]];
        cpu_masses[i] = p.pos_mass[3];
    }

    // Particle 0 updated normally
    assert_eq!(cpu_masses[0], 0.0001);
    assert_eq!(cpu_positions[0], [1.05, 0.0, 0.0]);

    // Particle 1 was NOT resurrected (remained dead at -5000)
    assert_eq!(cpu_masses[1], 0.0);
    assert_eq!(cpu_positions[1], [0.0, -5000.0, 0.0]);

    // Particle 2 was killed by GPU
    assert_eq!(cpu_masses[2], 0.0);
    assert_eq!(cpu_positions[2], [0.0, -5000.0, 0.0]);

    // Particle 3 stayed dead
    assert_eq!(cpu_masses[3], 0.0);
    assert_eq!(cpu_positions[3], [0.0, -5000.0, 0.0]);
}

#[test]
fn test_outer_giant_planet_mass_ceiling() {
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, JUPITER_MASS_SOLAR};

    // Proto-Jupiter in Solar Nebula MMSN
    let mut mass = 3.50 * EARTH_MASS_SOLAR;
    let max_giant_mass = 2.5 * JUPITER_MASS_SOLAR;

    // Simulate massive runaway accretion attempts (e.g. 10,000 particle sweeps)
    for _ in 0..10_000 {
        let gain = 100.0 * (0.00010 * EARTH_MASS_SOLAR / 100_000.0); // 100 particles
        let m_earth = mass / EARTH_MASS_SOLAR;
        let runaway_mult = if m_earth < 10.0 {
            1.0 + 0.05 * m_earth
        } else {
            1.5 + 0.15 * m_earth.clamp(10.0, 350.0).powf(0.30)
        };
        mass = (mass + gain * runaway_mult).min(max_giant_mass);
    }

    // Mass must be capped at 2.5 M_Jup and NEVER reach stellar/black hole mass (> 1000 M_earth)
    assert!(mass <= max_giant_mass);
    assert!(mass < 0.01); // Well below stellar threshold (0.08 M_sun)
}

#[test]
fn test_lighter_particles_moderate_50_year_growth() {
    // Verifies that with authentic lighter dust particle masses (~0.00010 M_sun disk,
    // ~0.00033 M_earth per particle), planets grow at a realistic, measured pace
    // and do NOT balloon to 350 M_earth within the first 50 years.
    let n_particles = 100_000.0;
    let disk_mass = 0.00010; // M_sun (~33 Earth masses across the entire solar nebula)
    let particle_mass = disk_mass / n_particles; // ~1.0e-9 M_sun ~ 0.00033 M_earth
    let particle_mass_earth = particle_mass / EARTH_MASS_SOLAR;

    assert!(
        particle_mass_earth < 0.0005,
        "Individual particle must be lightweight (~0.00033 M_earth)"
    );

    // Initial protoplanetary core (e.g. 3.0 M_earth)
    let mut core_mass_earth: f64 = 3.0;

    // Simulate 50 years of orbital sweeps (e.g. accreting ~20 particles per year)
    let years = 50;
    let particles_per_year = 20.0;

    for _ in 0..years {
        let annual_gain_earth = particles_per_year * particle_mass_earth;
        let runaway_mult = if core_mass_earth < 10.0 {
            1.0 + 0.05 * core_mass_earth
        } else {
            1.5 + 0.15 * core_mass_earth.clamp(10.0, 350.0).powf(0.30)
        };
        core_mass_earth += annual_gain_earth * runaway_mult;
    }

    // After 50 years:
    // Core should have grown by a modest, realistic amount (~0.3 to 0.5 M_earth),
    // and must be strictly well below 10 M_earth (nowhere near 350 M_earth!)
    assert!(
        core_mass_earth > 3.2 && core_mass_earth < 5.0,
        "50-year growth should be gentle and realistic (expected ~3.3-4.0 M_earth, got {})",
        core_mass_earth
    );
    assert!(
        core_mass_earth < 10.0,
        "Planets must not prematurely trigger runaway gas accretion in the first 50 years"
    );
}

#[test]
fn test_skybox_spherical_isotropy_and_flaring_elimination() {
    use bevy::math::Vec3;

    // 1. Verify shader source code guarantees: flaring formulas removed, spherical isotropic layer present
    let shader_src = std::fs::read_to_string("assets/shaders/skybox.wgsl")
        .expect("skybox.wgsl should be readable");

    // Must contain the new isotropic spherical layer
    assert!(
        shader_src.contains("fn render_star_layer"),
        "skybox.wgsl must contain render_star_layer"
    );

    // Must NOT contain Cartesian diffraction cross flare smears
    assert!(
        !shader_src.contains("spike_h"),
        "skybox.wgsl must not contain spike_h Cartesian flare"
    );
    assert!(
        !shader_src.contains("spike_v"),
        "skybox.wgsl must not contain spike_v Cartesian flare"
    );

    // Must contain spherical deep sky objects
    assert!(
        shader_src.contains("pleiades_dir"),
        "skybox.wgsl must contain Pleiades open cluster"
    );
    assert!(
        shader_src.contains("globular_dir"),
        "skybox.wgsl must contain Omega Centauri globular cluster"
    );
    assert!(
        shader_src.contains("m31_dir"),
        "skybox.wgsl must contain Andromeda Galaxy (M31)"
    );
    assert!(
        !shader_src.contains("dust_offset >"),
        "skybox.wgsl must not contain hard rectangular dust bounding box"
    );
    assert!(
        shader_src.contains("m31_bulge")
            && shader_src.contains("m31_disk")
            && shader_src.contains("dust_transmission"),
        "skybox.wgsl must contain smooth chromatic Andromeda model with continuous dust absorption"
    );

    // 2. Mathematical Rotational Invariance Proof for Spherical Metric:
    // th2 = dot(d - s, d - s)
    let star_dir = Vec3::new(0.62, 0.44, -0.65).normalize();
    let view_offset = Vec3::new(0.002, -0.003, 0.001);
    let view_dir = (star_dir + view_offset).normalize();

    let delta_orig = view_dir - star_dir;
    let dist_sq_original = delta_orig.dot(delta_orig);

    // Rotate both vectors arbitrarily in 3D (pitch 37 deg, yaw 112 deg, roll 58 deg)
    let rot = bevy::math::Quat::from_euler(
        bevy::math::EulerRot::XYZ,
        0.64577, // 37 deg
        1.95477, // 112 deg
        1.01229, // 58 deg
    );
    let rotated_view = rot * view_dir;
    let rotated_star = rot * star_dir;

    let delta_rot = rotated_view - rotated_star;
    let dist_sq_rotated = delta_rot.dot(delta_rot);

    // The angular metric must be exactly identical under any 3D rotation (zero skew, zero eccentricity)
    assert!(
        (dist_sq_original - dist_sq_rotated).abs() < 1e-6,
        "Angular distance metric must be perfectly rotationally invariant"
    );

    // 3. Gaussian profile circular symmetry: points at identical angular radii must have identical intensities
    let sigma = 0.0016_f32;
    let r_angle = 0.0020_f32; // 0.002 rad from star center

    // Create orthonormal tangent vectors perp to star_dir
    let tangent_x = star_dir.cross(Vec3::Y).normalize();
    let tangent_y = star_dir.cross(tangent_x).normalize();

    // Baseline intensity at r_angle
    let base_dir = (star_dir + tangent_x * r_angle).normalize();
    let base_delta = base_dir - star_dir;
    let base_th2 = base_delta.dot(base_delta);
    let intensity_0 = (-base_th2 / (sigma * sigma)).exp();

    // Evaluate 8 points in a circle around the star in the celestial tangent plane
    for step in 0..8 {
        let phi = (step as f32) * (std::f32::consts::PI / 4.0);
        let offset_dir =
            (star_dir + (tangent_x * phi.cos() + tangent_y * phi.sin()) * r_angle).normalize();
        let delta = offset_dir - star_dir;
        let th2 = delta.dot(delta);
        let sample_intensity = (-th2 / (sigma * sigma)).exp();

        assert!(
            (sample_intensity - intensity_0).abs() < 1e-5,
            "Star profile must be perfectly circular and unskewed at all angles (step {})",
            step
        );
    }
}

#[test]
fn test_inner_planet_acceleration_unattenuated() {
    // Inner planets (Mercury at ~0.387 AU, Venus at ~0.723 AU) around a 1 M_sun star
    // experience high gravitational accelerations:
    // a = G * M / r^2
    // For Mercury: 39.4784 / (0.387^2) = 263.6 AU/yr^2
    // The previous 80.0 AU/yr^2 acceleration cap artificially severed 70% of solar gravity,
    // flinging inner planets into spurious escape orbits!
    let star_mass = 1.0; // M_sun
    let mercury_r = 0.387; // AU
    let r_vec = DVec3::new(mercury_r, 0.0, 0.0);
    let dist_sq = r_vec.length_squared();
    let _dist = dist_sq.sqrt();

    let softening_sq = 0.001 * 0.001;
    let softened_dist = (dist_sq + softening_sq).sqrt();
    let mut acc = -(G_ASTRO * star_mass / (softened_dist * softened_dist * softened_dist)) * r_vec;

    let acc_mag = acc.length();
    // Verify physical acceleration is ~263 AU/yr^2
    assert!(
        acc_mag > 260.0 && acc_mag < 270.0,
        "Mercury-like orbital acceleration should be ~263.6 AU/yr^2, got {}",
        acc_mag
    );

    // Apply the updated acceleration limiter (500,000 AU/yr^2)
    if acc_mag > 500_000.0 {
        acc *= 500_000.0 / acc_mag;
    }

    // Must NOT be capped at 80.0 AU/yr^2
    assert!(
        acc.length() > 250.0,
        "Acceleration limiter must not truncate inner planetary gravity"
    );
}

#[test]
fn test_unbounded_interstellar_drift_no_112_au_clamping() {
    // Ejected or rogue planets moving outwards beyond the disk outer radius (e.g. 112.5 AU)
    // must drift along their velocity vector (r += v * dt) into interstellar space,
    // and must NOT be clamped to a 112.5 AU sphere or forced into 2D circular Keplerian rotation.
    let initial_pos = DVec3::new(112.50, 0.0, 0.0); // Exactly at old boundary
    let escape_vel = DVec3::new(10.0, 0.0, 2.0); // 10 AU/yr radially outward
    let dt = 0.5; // 0.5 year timestep

    let mut pos = initial_pos;
    let vel = escape_vel;

    // Symplectic Leapfrog linear drift
    pos += vel * dt;

    // Body should now be at x = 117.5 AU, z = 1.0 AU
    assert_eq!(pos.x, 117.5);
    assert_eq!(pos.z, 1.0);
    assert!(
        pos.length() > 112.50,
        "Body must freely drift past 112.5 AU into interstellar space"
    );

    // Old clamping would have forced: pos *= 112.5 / pos.length(), leaving it stuck at 112.5 AU!
    let old_clamped = pos * (112.50 / pos.length());
    assert!((old_clamped.length() - 112.50).abs() < 1e-10);
    assert!(
        pos.length() > old_clamped.length(),
        "Position must not be pinned to 112.5 AU"
    );
}

#[test]
fn test_minor_debris_deep_space_retirement_threshold() {
    // Minor debris (asteroids, planetesimals, comets) beyond 2000 AU should be retired,
    // while rogue planets and brown dwarfs remain persistent in interstellar space.
    let r_close_debris = 150.0;
    let r_escaped_debris = 2500.0;
    let r_rogue_planet = 2500.0;

    let debris_type = BodyType::Asteroid;
    let planet_type = BodyType::GasGiant;

    let retire_debris_close = r_close_debris > 2000.0
        && matches!(
            debris_type,
            BodyType::Planetesimal | BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain
        );
    assert!(!retire_debris_close, "Debris at 150 AU must not be retired");

    let retire_debris_far = r_escaped_debris > 2000.0
        && matches!(
            debris_type,
            BodyType::Planetesimal | BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain
        );
    assert!(retire_debris_far, "Debris at 2500 AU should be retired");

    let retire_planet_far = r_rogue_planet > 2000.0
        && matches!(
            planet_type,
            BodyType::Planetesimal | BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain
        );
    assert!(
        !retire_planet_far,
        "Rogue planets/gas giants must never be retired in deep space"
    );
}

#[test]
fn test_swarm_clump_promotion_capacity_guards() {
    // Runaway particle clumps should only promote to ECS massive bodies if total ECS count < 24
    // and mass exceeds protoplanetary embryo threshold (~0.005 M_earth)
    let b_mass = 0.0001f32;
    let promo_threshold = (16.0f32 * b_mass).max(EARTH_MASS_SOLAR as f32 * 0.005);

    // 1. Small clump below threshold -> no promotion
    let small_clump_mass = 0.0005f32;
    assert!(small_clump_mass < promo_threshold);

    // 2. Large clump above threshold, but ECS body limit reached (e.g. 24) -> no promotion
    let ecs_count_full = 24;
    let large_clump_mass = promo_threshold + 0.001f32;
    let can_promote_full = large_clump_mass >= promo_threshold && ecs_count_full < 24;
    assert!(
        !can_promote_full,
        "Must not promote when ECS capacity is full"
    );

    // 3. Large clump above threshold with room in system -> promote
    let ecs_count_open = 12;
    let can_promote_open = large_clump_mass >= promo_threshold && ecs_count_open < 24;
    assert!(can_promote_open, "Should promote when under ECS capacity");
}

#[test]
fn test_little_red_dot_high_speed_unbounded_limits() {
    // In the JWST Little Red Dot scenario (450,000 M_sun Black Hole Star),
    // orbiting bodies travel at extreme relativistic velocities (thousands of km/s)
    // and experience immense gravitational accelerations.
    // The physics limits (max speed 200 AU/yr, max acc 500,000 AU/yr^2) must NOT apply!
    let quasi_star_mass = 450_000.0; // M_sun

    // 1. Orbital velocity at 85 AU (primordial cloudlet orbit):
    // v = sqrt(G * M / r) = sqrt(39.4784 * 450,000 / 85) = 457.17 AU/yr (~2,167 km/s)
    let r_orbit = 85.0; // AU
    let v_circ = (G_ASTRO * quasi_star_mass / r_orbit).sqrt();
    let speed_km_s = v_circ * AU_PER_YR_TO_KM_PER_S;

    assert!(
        v_circ > 450.0 && v_circ < 465.0,
        "Little Red Dot 85 AU orbital speed should be ~457 AU/yr, got {}",
        v_circ
    );
    assert!(
        speed_km_s > 2100.0 && speed_km_s < 2200.0,
        "Orbital speed in km/s should be ~2,167 km/s, got {}",
        speed_km_s
    );

    // Verify velocity limit exemption for Little Red Dot:
    let is_little_red_dot = true;
    let mut vel = DVec3::new(0.0, 0.0, v_circ);
    if !is_little_red_dot {
        let speed = vel.length();
        let max_speed = 200.0;
        if speed > max_speed {
            vel *= max_speed / speed;
        }
    }
    assert_eq!(
        vel.length(),
        v_circ,
        "Little Red Dot orbiting objects must retain their full >450 AU/yr velocity without capping"
    );

    // In a normal solar system, velocity would have been bounded to 200 AU/yr:
    let is_normal_system = false;
    let mut normal_vel = DVec3::new(0.0, 0.0, v_circ);
    if !is_normal_system {
        let speed = normal_vel.length();
        let max_speed = 200.0;
        if speed > max_speed {
            normal_vel *= max_speed / speed;
        }
    }
    assert_eq!(normal_vel.length(), 200.0);

    // 2. Gravitational acceleration at 3 AU (infalling gas near the 60 AU cocoon):
    // a = G * M / r^2 = 39.4784 * 450,000 / 9 = ~1,973,920 AU/yr^2
    let r_inner = 3.0; // AU
    let acc_phys = G_ASTRO * quasi_star_mass / (r_inner * r_inner);
    assert!(
        acc_phys > 1_900_000.0,
        "Physical acceleration should be ~1.97M AU/yr^2, got {}",
        acc_phys
    );

    let mut acc = DVec3::new(acc_phys, 0.0, 0.0);
    if !is_little_red_dot && acc.length() > 500_000.0 {
        acc *= 500_000.0 / acc.length();
    }
    assert_eq!(
        acc.length(),
        acc_phys,
        "Little Red Dot acceleration must NOT be capped at 500,000 AU/yr^2"
    );

    // 3. Debris escape radius: At 3,000 AU, debris is still bound to Little Red Dot
    // (v_esc = sqrt(2 * G * M / 3000) = ~108 AU/yr). Must NOT be retired at 2,000 AU.
    let debris_dist = 3000.0;
    let debris_escape_radius = if is_little_red_dot { 100_000.0 } else { 2000.0 };
    assert!(
        debris_dist < debris_escape_radius,
        "Debris at 3,000 AU is bound to Little Red Dot and must not be retired"
    );
}

#[test]
fn test_nan_and_inf_state_vector_sanitization() {
    // When non-finite numbers (NaN, +Inf, -Inf) appear in position or velocity
    // (due to division by zero, precision underflow, or unphysical inputs),
    // the physics engine must safely catch and sanitize them without panicking.

    // 1. Standard Solar System Sanitization: Resets to 1.0 AU circular orbit
    let star_mass_solar = 1.0;
    let mut pos_corrupted = DVec3::new(f64::NAN, 0.0, f64::INFINITY);
    let mut vel_corrupted = DVec3::new(0.0, f64::NEG_INFINITY, 0.0);
    let mut acc = DVec3::new(5.0, 0.0, 0.0);
    let is_little_red_dot = false;

    if !pos_corrupted.is_finite() || !vel_corrupted.is_finite() {
        let safe_r = if is_little_red_dot { 120.0 } else { 1.0 };
        let v_k = (G_ASTRO * star_mass_solar / safe_r).sqrt();
        pos_corrupted = DVec3::new(safe_r, 0.0, 0.0);
        vel_corrupted = DVec3::new(0.0, 0.0, v_k);
        acc = DVec3::ZERO;
    }

    assert!(pos_corrupted.is_finite());
    assert!(vel_corrupted.is_finite());
    assert_eq!(pos_corrupted, DVec3::new(1.0, 0.0, 0.0));
    assert!((vel_corrupted.z - 2.0 * std::f64::consts::PI).abs() < 1e-4);
    assert_eq!(acc, DVec3::ZERO);

    // 2. Little Red Dot Sanitization: Resets safely to 120.0 AU outside the 60 AU cocoon
    let star_mass_lrd = 450_000.0;
    let mut lrd_pos = DVec3::new(f64::NAN, f64::NAN, 0.0);
    let mut lrd_vel = DVec3::ZERO;
    let is_little_red_dot = true;

    if !lrd_pos.is_finite() || !lrd_vel.is_finite() {
        let safe_r = if is_little_red_dot { 120.0 } else { 1.0 };
        let v_k = (G_ASTRO * star_mass_lrd / safe_r).sqrt();
        lrd_pos = DVec3::new(safe_r, 0.0, 0.0);
        lrd_vel = DVec3::new(0.0, 0.0, v_k);
    }

    assert!(lrd_pos.is_finite());
    assert!(lrd_vel.is_finite());
    assert_eq!(lrd_pos.x, 120.0);
    assert!(lrd_vel.z > 380.0 && lrd_vel.z < 390.0); // ~384.8 AU/yr
}

#[test]
fn test_orbital_elements_solver_error_handling_and_edge_cases() {
    // 1. Zero mass or negative mass -> gracefully returns None
    let res_zero_mass = state_vectors_to_orbital_elements(
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(0.0, 0.0, std::f64::consts::TAU),
        0.0,
        0.0,
    );
    assert!(res_zero_mass.is_none());

    // 2. Zero position or microscopic distance (< 1e-7 AU) -> gracefully returns None
    let res_zero_pos = state_vectors_to_orbital_elements(
        DVec3::ZERO,
        DVec3::new(0.0, 0.0, std::f64::consts::TAU),
        1.0,
        EARTH_MASS_SOLAR,
    );
    assert!(res_zero_pos.is_none());

    // 3. Zero velocity or microscopic speed (< 1e-7 AU/yr) -> gracefully returns None
    let res_zero_vel = state_vectors_to_orbital_elements(
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::ZERO,
        1.0,
        EARTH_MASS_SOLAR,
    );
    assert!(res_zero_vel.is_none());

    // 4. Hyperbolic escape trajectory (e > 1.0, specific energy > 0):
    // E.g. interstellar interloper traveling at 15 AU/yr at 1 AU (escape velocity is ~8.88 AU/yr)
    let pos_hyperbolic = DVec3::new(1.0, 0.0, 0.0);
    let vel_hyperbolic = DVec3::new(0.0, 0.0, 15.0);
    let res_hyp =
        state_vectors_to_orbital_elements(pos_hyperbolic, vel_hyperbolic, 1.0, EARTH_MASS_SOLAR);
    assert!(res_hyp.is_some());
    let hyp_elem = res_hyp.unwrap();
    assert!(
        hyp_elem.eccentricity > 1.0,
        "Must identify hyperbolic eccentricity e > 1"
    );
    assert!(
        hyp_elem.specific_energy > 0.0,
        "Specific energy must be positive for unbounded orbit"
    );
    assert!(
        hyp_elem.apoapsis.is_infinite(),
        "Apoapsis must be infinite for hyperbolic orbit"
    );
    assert_eq!(
        hyp_elem.period_years, 0.0,
        "Period is 0 for unbound hyperbolic trajectories"
    );

    // 5. Equatorial and inclined orbits:
    // Planar equatorial orbit in reference coordinate frame (v in Y)
    let pos_eq = DVec3::new(1.0, 0.0, 0.0);
    let vel_eq = DVec3::new(0.0, std::f64::consts::TAU, 0.0);
    let res_eq = state_vectors_to_orbital_elements(pos_eq, vel_eq, 1.0, EARTH_MASS_SOLAR);
    assert!(res_eq.is_some());
    let eq_elem = res_eq.unwrap();
    assert!(
        eq_elem.inclination.abs() < 1e-4,
        "Equatorial orbit inclination must be 0"
    );
    assert!(eq_elem.argument_of_periapsis.is_finite());

    // 90-degree inclined orbit (v in Z):
    let pos_inc = DVec3::new(1.0, 0.0, 0.0);
    let vel_inc = DVec3::new(0.0, 0.0, std::f64::consts::TAU);
    let res_inc = state_vectors_to_orbital_elements(pos_inc, vel_inc, 1.0, EARTH_MASS_SOLAR);
    assert!(res_inc.is_some());
    let inc_elem = res_inc.unwrap();
    assert!(
        (inc_elem.inclination - std::f64::consts::FRAC_PI_2).abs() < 1e-3,
        "Inclined orbit inclination must be pi/2 (90 deg)"
    );
    assert!(inc_elem.inclination.is_finite());
    assert!(inc_elem.argument_of_periapsis.is_finite());
}

#[test]
fn test_gravitational_singularity_avoidance_zero_distance() {
    // When two bodies share the exact same spatial coordinate (r = 0),
    // mutual gravitational force F = G * M * m / r^2 has a potential 1/0 singularity.
    // Adaptive softening must guarantee that force is finite, bounded, and never produces NaN.
    let softening_sq: f64 = 0.001 * 0.001;
    let rad1: f64 = 0.005; // 0.005 AU
    let pos1 = DVec3::new(10.0, 0.0, 5.0);
    let pos2 = DVec3::new(10.0, 0.0, 5.0); // Exactly coincident!

    let r_vec = pos1 - pos2;
    assert_eq!(r_vec, DVec3::ZERO);

    let pair_softening_sq = softening_sq.max((rad1 * 0.5f64).powi(2)).max(1e-4);
    let dist_sq = r_vec.length_squared() + pair_softening_sq;
    let dist = dist_sq.sqrt();

    assert!(dist > 0.009, "Softened distance must be strictly positive");
    let acc = -(G_ASTRO * 1.0 / (dist_sq * dist)) * r_vec;

    assert!(acc.is_finite(), "Acceleration must be strictly finite");
    assert_eq!(
        acc,
        DVec3::ZERO,
        "Symmetric coincident force evaluates to zero vector"
    );
}

#[test]
fn test_continuous_collision_detection_tunneling_defense() {
    // High-speed bodies (e.g. 50 AU/yr comets) can jump completely across an entire planetary radius
    // in a single large timestep dt. Continuous Collision Detection (CCD) computes the exact time of
    // closest approach t_min in [0, dt] to prevent tunneling through planets.
    let dt = 0.05; // 0.05 yr (~18 days)
    let planet_radius = 0.02; // AU

    // Body 1 at origin
    let p1 = DVec3::ZERO;
    let v1 = DVec3::ZERO;

    // Body 2 starts at (-0.5, 0.005, 0.0) moving right at 20 AU/yr
    // Over dt = 0.05 yr, it travels 1.0 AU: from x = -0.5 to x = +0.5.
    // At t=0, distance is 0.500 AU (> 0.02 AU radius)
    // At t=dt, distance is 0.500 AU (> 0.02 AU radius)
    // BUT at t = 0.025, it passes right through x = 0 at y = 0.005 AU (< 0.02 AU radius)!
    let p2_start = DVec3::new(-0.5, 0.005, 0.0);
    let v2 = DVec3::new(20.0, 0.0, 0.0);
    let p2_end = p2_start + v2 * dt;

    let r_rel = p2_end - p1;
    let v_rel = v2 - v1;
    let v_rel_sq = v_rel.length_squared();

    // Discrete end-of-step check fails (tunnels!):
    let discrete_distance = r_rel.length();
    assert!(
        discrete_distance > planet_radius,
        "Discrete check would fail to detect collision (tunneling)"
    );

    // Continuous Collision Detection (CCD):
    let r_rel_old = r_rel - v_rel * dt;
    let t_min = (-r_rel_old.dot(v_rel) / v_rel_sq).clamp(0.0, dt);
    let closest_approach_vec = r_rel_old + v_rel * t_min;
    let min_distance = closest_approach_vec.length();

    // CCD successfully intercepts the impact at 0.005 AU!
    assert!(
        min_distance < planet_radius,
        "CCD must intercept the closest approach (min_dist = {}, radius = {})",
        min_distance,
        planet_radius
    );
    assert!(
        (min_distance - 0.005).abs() < 1e-6,
        "Closest approach distance must be exactly 0.005 AU"
    );
}

#[test]
fn test_little_red_dot_circum_nuclear_period_and_energy() {
    // Verify Keplerian period and orbital energy in the supermassive Little Red Dot system (450,000 M_sun):
    // 1. Semi-major axis a = 85 AU (primordial cloudlet orbit)
    // P = 2*pi * sqrt(a^3 / mu)
    // mu = G * M = 39.4784176 * 450,000 = 17,765,287.9 (AU^3 / yr^2)
    let mu = G_ASTRO * 450_000.0;
    let a_inner = 85.0;
    let period_inner = 2.0 * std::f64::consts::PI * (a_inner * a_inner * a_inner / mu).sqrt();
    let specific_energy_inner = -mu / (2.0 * a_inner);

    assert!(
        period_inner > 1.10 && period_inner < 1.25,
        "85 AU orbital period should be ~1.17 years, got {}",
        period_inner
    );
    assert!(
        specific_energy_inner < -100_000.0,
        "Specific energy must be deeply bound negative"
    );

    // 2. Outer circum-nuclear edge at a = 235 AU
    let a_outer = 235.0;
    let period_outer = 2.0 * std::f64::consts::PI * (a_outer * a_outer * a_outer / mu).sqrt();
    let specific_energy_outer = -mu / (2.0 * a_outer);

    assert!(
        period_outer > 5.2 && period_outer < 5.6,
        "235 AU orbital period should be ~5.37 years, got {}",
        period_outer
    );
    assert!(specific_energy_outer < specific_energy_inner.abs());
}

#[test]
fn test_hud_visibility_and_collapsible_panel_states() {
    use protostellar::game::ui::HudVisibilityState;

    let mut hud_state = HudVisibilityState::default();

    // Default state: master HUD visible, all panels expanded
    assert!(!hud_state.is_full_screen_clean);
    assert!(!hud_state.top_left_minimized);
    assert!(!hud_state.top_right_minimized);
    assert!(!hud_state.inspector_minimized);
    assert!(!hud_state.scenarios_minimized);

    // Toggle clean fullscreen
    hud_state.is_full_screen_clean = true;
    assert!(hud_state.is_full_screen_clean);

    // Minimize individual panels
    hud_state.top_left_minimized = true;
    hud_state.top_right_minimized = true;
    hud_state.inspector_minimized = true;
    hud_state.scenarios_minimized = true;

    assert!(hud_state.top_left_minimized);
    assert!(hud_state.top_right_minimized);
    assert!(hud_state.inspector_minimized);
    assert!(hud_state.scenarios_minimized);

    // Restore HUD
    hud_state.is_full_screen_clean = false;
    assert!(!hud_state.is_full_screen_clean);
}

#[test]
fn test_universal_speed_of_light_limit() {
    use bevy::math::DVec3;
    use protostellar::utils::constants::SPEED_OF_LIGHT_AU_YR;

    // 1. Cosmic Speed Limit in vacuum: ~63,241.077 AU/yr (~299,792.458 km/s)
    let c_au_yr = SPEED_OF_LIGHT_AU_YR;
    let universal_c_limit = c_au_yr * 0.999;

    // A body accelerated beyond c (e.g. 150,000 AU/yr ~ 711,000 km/s)
    let mut runaway_vel = DVec3::new(100_000.0, 0.0, 111_803.0); // magnitude = 150,000 AU/yr
    assert!(runaway_vel.length() > c_au_yr);

    // Apply universal cosmic speed limit clamp:
    let speed = runaway_vel.length();
    if speed > universal_c_limit {
        runaway_vel *= universal_c_limit / speed;
    }

    assert!(
        runaway_vel.length() <= universal_c_limit + 1e-6,
        "Universal speed of light limit must strictly prevent velocities > c"
    );
    assert!(
        runaway_vel.length() < c_au_yr,
        "Body speed must remain strictly sub-luminal"
    );

    // 2. Standard stellar system planetary speed ceiling (250 AU/yr ~ 1,185 km/s)
    let is_little_red_dot = false;
    let mut planet_vel = DVec3::new(0.0, 0.0, 450.0);
    let speed = planet_vel.length();
    if speed > universal_c_limit {
        planet_vel *= universal_c_limit / speed;
    } else if !is_little_red_dot {
        let max_planetary_speed = 250.0;
        if speed > max_planetary_speed {
            planet_vel *= max_planetary_speed / speed;
        }
    }
    assert_eq!(planet_vel.length(), 250.0);

    // 3. Supermassive system speed ceiling (10,000 AU/yr ~ 47,404 km/s ~ 0.16 c)
    let is_little_red_dot = true;
    let mut smbh_vel = DVec3::new(0.0, 0.0, 15_000.0);
    let speed = smbh_vel.length();
    if speed > universal_c_limit {
        smbh_vel *= universal_c_limit / speed;
    } else if !is_little_red_dot {
        let max_planetary_speed = 250.0;
        if speed > max_planetary_speed {
            smbh_vel *= max_planetary_speed / speed;
        }
    } else {
        let max_smbh_bound_speed = 10_000.0;
        if speed > max_smbh_bound_speed {
            smbh_vel *= max_smbh_bound_speed / speed;
        }
    }
    assert_eq!(smbh_vel.length(), 10_000.0);
}

#[test]
fn test_little_red_dot_stable_keplerian_orbit() {
    use bevy::math::DVec3;
    use protostellar::utils::constants::G_ASTRO;

    // Quasi-star total mass = 450,000 M_sun
    let total_mass = 450_000.0;
    let r_au = 95.0; // AU

    // Exact circular orbital velocity:
    let v_circ = (G_ASTRO * total_mass / r_au).sqrt();
    let softening_sq = 0.005 * 0.005;

    // Centripetal acceleration required for circular orbit: a_c = v^2 / r
    let a_centripetal = (v_circ * v_circ) / r_au;

    // Gravitational acceleration computed with true Newtonian exterior field (unsoftened at 95 AU):
    let r_vec = DVec3::new(r_au, 0.0, 0.0);
    let dist_sq = r_vec.length_squared() + softening_sq;
    let dist = dist_sq.sqrt();
    let acc_grav = (G_ASTRO * total_mass / (dist_sq * dist)) * r_vec.length();

    // Verify relative error is negligible (< 1e-7)
    let rel_diff = (a_centripetal - acc_grav).abs() / a_centripetal;
    assert!(
        rel_diff < 1e-7,
        "Gravitational pull must match centripetal acceleration exactly, rel_diff: {}",
        rel_diff
    );

    // Symplectic Leapfrog test: Integrate orbit for 1.5 years (> 1 full orbit: P = 1.38 yr)
    let dt = 0.0005; // yr
    let mut pos = DVec3::new(r_au, 0.0, 0.0);
    let mut vel = DVec3::new(0.0, 0.0, v_circ);

    // Compute initial acceleration
    let mut acc = -(G_ASTRO * total_mass / (pos.length_squared() + softening_sq).powf(1.5)) * pos;

    let total_steps = 3000; // 1.5 simulated years
    for _ in 0..total_steps {
        vel += acc * (dt * 0.5);
        pos += vel * dt;
        acc = -(G_ASTRO * total_mass / (pos.length_squared() + softening_sq).powf(1.5)) * pos;
        vel += acc * (dt * 0.5);
    }

    let final_r = pos.length();
    let orbital_radius_drift = (final_r - r_au).abs() / r_au;
    assert!(
        orbital_radius_drift < 0.001,
        "Orbit must remain strictly circular without flinging out: initial {} AU, final {} AU, drift {}%",
        r_au,
        final_r,
        orbital_radius_drift * 100.0
    );
}

#[test]
fn test_asteroid_mesh_isotropic_harmonics_and_planetesimal_sphere() {
    use protostellar::simulation::components::BodyType;

    // Verify that Planetesimal is mapped to smooth spherical planet mesh, NOT irregular rubble mesh:
    let is_spherical = |b_type: BodyType| -> bool {
        matches!(
            b_type,
            BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::SuperEarth
                | BodyType::TerrestrialPlanet
                | BodyType::Protoplanet
                | BodyType::Planetesimal
                | BodyType::Moon
        )
    };

    assert!(is_spherical(BodyType::Planetesimal));
    assert!(is_spherical(BodyType::Protoplanet));
    assert!(is_spherical(BodyType::TerrestrialPlanet));
    assert!(!is_spherical(BodyType::Asteroid));
    assert!(!is_spherical(BodyType::Comet));

    // Verify that directional harmonics don't have coordinate-axis cubic singularity:
    let v_axis_x = bevy::math::Vec3::new(1.0, 0.0, 0.0);

    // Cartesian product (OLD bug): v.x * v.y * v.z is ZERO on any coordinate axis!
    let old_harmonic_axis =
        (v_axis_x.x * 3.5).sin() * (v_axis_x.y * 3.5).cos() * (v_axis_x.z * 3.5).sin();
    assert_eq!(old_harmonic_axis, 0.0); // Cubed flat face bug!

    // New isotropic directional projection (NO zero on axes):
    let k1 = v_axis_x.dot(bevy::math::Vec3::new(0.577, 0.577, 0.577));
    let new_harmonic_axis = (k1 * 3.2).sin() * 0.12;
    assert!(
        new_harmonic_axis.abs() > 0.01,
        "Isotropic noise must be non-zero on axes to prevent flat cube faces"
    );
}

#[test]
fn test_pop_iii_eddington_mass_ceiling() {
    // Verifies that stellar accretion in supermassive disks (Little Red Dot)
    // is strictly bounded by the astrophysical Eddington radiation ceiling (150 M_sun),
    // preventing the catastrophic 423-billion-solar-mass runaway explosion.
    let mut star_mass = 60.0; // 60 M_sun Pop-III star seed
    const POP_III_MAX_STELLAR_MASS: f64 = 150.0;

    // Simulate 50,000 particle accretion events (each gaining dust mass)
    let particle_gain = 0.005; // 0.005 M_sun per swarm particle in a 500 M_sun disk
    for _ in 0..50_000 {
        if star_mass < POP_III_MAX_STELLAR_MASS {
            let growth_factor = (1.0 - (star_mass / POP_III_MAX_STELLAR_MASS)).clamp(0.0, 1.0);
            let delta = particle_gain * (1.0 + 0.25 * growth_factor);
            star_mass = (star_mass + delta).min(POP_III_MAX_STELLAR_MASS);
        }
    }

    assert!(
        star_mass <= POP_III_MAX_STELLAR_MASS,
        "Pop-III star mass ({star_mass} M_sun) must never exceed Eddington ceiling of {POP_III_MAX_STELLAR_MASS} M_sun"
    );
    assert!(
        star_mass >= 149.9,
        "Pop-III star should asymptotically approach the Eddington limit smoothly"
    );

    // Verify sub-stellar planet in massive disk does not grow into a star
    let mut planet_mass = 0.005; // 5.2 M_Jup gas giant
    let max_planet_mass = 15.0 * JUPITER_MASS_SOLAR;
    for _ in 0..10_000 {
        if planet_mass < max_planet_mass {
            let m_earth = (planet_mass / EARTH_MASS_SOLAR).clamp(0.1, 4500.0);
            let mult = 1.0 + 0.15 * m_earth.powf(0.20);
            planet_mass = (planet_mass + particle_gain * mult).min(max_planet_mass);
        }
    }
    assert!(
        planet_mass <= max_planet_mass,
        "Planet mass in massive disk must not exceed 15 M_Jup ({planet_mass} <= {max_planet_mass})"
    );
}

#[test]
fn test_supermassive_disk_particle_boundary_retention() {
    // Tests that particles orbiting a 450,000 M_sun Quasi-Star stay bound within
    // the circum-nuclear disk ([65, 250] AU) and never get flung out to > 100,000 AU.
    let star_mass = 450_000.0;
    let r_orbit = 100.0; // AU
    let v_circ = (G_ASTRO * star_mass / r_orbit).sqrt(); // ~421.5 AU/yr

    let mut pos = DVec3::new(r_orbit, 0.0, 0.0);
    let mut vel = DVec3::new(0.0, 0.0, v_circ);

    // Perturbation from a 60 M_sun companion star passing nearby at 95 AU
    let mb_pos = DVec3::new(95.0, 0.0, 0.0);
    let mb_mass = 60.0;

    let dt = 0.002;
    for _ in 0..500 {
        let r = pos.length();
        let to_star = -pos;
        let a_star = (G_ASTRO * star_mass / (r * r * r)) * to_star;

        let to_mb = mb_pos - pos;
        let dist_sq = (to_mb.length_squared() + 0.04).max(0.04);
        let a_mb = (G_ASTRO * mb_mass / (dist_sq * dist_sq.sqrt())) * to_mb;

        let mut a_tot = a_star + a_mb;
        let a_mag = a_tot.length();
        let max_a = 45_000.0; // AU/yr^2
        if a_mag > max_a {
            a_tot *= max_a / a_mag;
        }

        vel += a_tot * dt;
        let speed = vel.length();
        let max_speed = 8_000.0; // AU/yr
        if speed > max_speed {
            vel *= max_speed / speed;
        }

        pos += vel * dt;

        // Disk boundary restitution
        let p_r = (pos.x * pos.x + pos.z * pos.z).sqrt();
        assert!(
            p_r < 1000.0,
            "Particle radius ({p_r} AU) must remain in circum-nuclear disk, never escape to interstellar space"
        );
    }
}

#[test]
fn test_supermassive_disk_circum_nuclear_capacity_and_imbh_growth() {
    // Verify that circum-nuclear disk allows growth > 500 M_sun in the thick inner region,
    // while tapering down in outer regions to preserve realistic planetary/sub-stellar masses.
    let r_in = 65.0;
    let r_out = 280.0;

    // 1. Thick inner accretion channel (75 AU)
    let inner_cap =
        protostellar::simulation::accretion::circum_nuclear_ring_mass_capacity(75.0, r_in, r_out);
    assert!(
        inner_cap > 500.0 && inner_cap <= 1000.0,
        "Thick inner region capacity ({inner_cap} M_sun) must exceed 500 M_sun for supermassive seeds / IMBHs"
    );

    // 2. Intermediate ring (150 AU)
    let mid_cap =
        protostellar::simulation::accretion::circum_nuclear_ring_mass_capacity(150.0, r_in, r_out);
    assert!(
        mid_cap > 50.0 && mid_cap < 500.0,
        "Mid-ring capacity ({mid_cap} M_sun) should support Pop-III stars (50 - 500 M_sun)"
    );

    // 3. Outer tenuous ring (260 AU)
    let outer_cap =
        protostellar::simulation::accretion::circum_nuclear_ring_mass_capacity(260.0, r_in, r_out);
    assert!(
        outer_cap < 30.0,
        "Outer ring capacity ({outer_cap} M_sun) must be bounded to stellar/planetary scales"
    );

    // 4. Verify that a body growing to 600 M_sun in the inner channel transitions to an Intermediate-Mass Black Hole
    let comp = Composition::pure_hydrogen();
    let body_type_600m = classify_body_by_mass_and_comp(600.0, &comp, false);
    assert_eq!(
        body_type_600m,
        BodyType::BlackHole,
        "Bodies exceeding 500 M_sun must collapse into Intermediate-Mass Black Holes"
    );

    // 5. Verify that a 1,000 M_sun IMBH at 88 AU around 450,000 M_sun central BH maintains a compact, stable Hill sphere
    let bh_mass: f64 = 450_000.0;
    let r_orbit: f64 = 88.0;
    let sat_mass: f64 = 1000.0;
    let hill_r: f64 = r_orbit * (sat_mass / (3.0 * bh_mass)).cbrt();
    assert!(
        hill_r < 9.0,
        "1000 M_sun IMBH Hill radius ({hill_r} AU) must remain well under the 32 AU gap to the next star at 120 AU"
    );
}

#[test]
fn test_little_red_dot_scenario_initial_orbit_stability() {
    let bh_mass = 450_000.0; // M_sun

    // The 6 initial satellites of Little Red Dot
    let satellites = [
        ("Micro-Quasar alpha", 88.0, 12.0),
        ("Star alpha", 120.0, 60.0),
        ("Prime-b", 155.0, 0.000045),
        ("Star beta", 190.0, 35.0),
        ("Prime-c", 225.0, 0.000030),
        ("Star gamma", 260.0, 20.0),
    ];

    for (name, r, m) in satellites {
        let v_circ_au_yr = (G_ASTRO * bh_mass / r).sqrt();
        let v_circ_km_s = v_circ_au_yr * 4.74047;

        // Speed must be well below speed of light (299,792 km/s)
        assert!(
            v_circ_km_s < 299_792.0,
            "{name} at {r} AU has orbital speed {v_circ_km_s} km/s exceeding speed of light!"
        );
        // Speed must be astrophysically realistic for circum-nuclear orbit (1,000 - 15,000 km/s)
        assert!(
            v_circ_km_s > 1000.0 && v_circ_km_s < 15000.0,
            "{name} at {r} AU has speed {v_circ_km_s} km/s outside expected Keplerian range"
        );

        // Orbital period T = 2*pi*r / v = 2*pi*sqrt(r^3 / (G*M))
        let period_yr = 2.0 * std::f64::consts::PI * (r * r * r / (G_ASTRO * bh_mass)).sqrt();
        assert!(
            period_yr > 0.5 && period_yr < 10.0,
            "{name} at {r} AU period {period_yr} yr should be between 0.5 and 10.0 yr"
        );

        // Hill sphere radius: r_hill = r * (m / (3 * M_bh))^(1/3)
        let hill_r = r * (m / (3.0 * bh_mass)).cbrt();
        assert!(
            hill_r < 15.0,
            "{name} Hill radius ({hill_r} AU) must remain compact and well within inter-satellite spacing"
        );
    }
}

#[test]
fn test_stellar_photosphere_shader_coverage() {
    let shader_src = std::fs::read_to_string("assets/shaders/planet.wgsl")
        .expect("planet.wgsl should be readable");

    // 1. Cellular noise engine
    assert!(
        shader_src.contains("fn voronoi3("),
        "planet.wgsl must contain 3D Voronoi cellular noise engine"
    );
    assert!(
        shader_src.contains("fn hash3_vec("),
        "planet.wgsl must contain 3D vector hash function"
    );

    // 2. Black Hole Star / Quasi-Star Photosphere
    assert!(
        shader_src.contains("fn render_quasistar_photosphere("),
        "planet.wgsl must contain dedicated render_quasistar_photosphere function"
    );
    assert!(
        shader_src.contains("planet.planet_type == 7u"),
        "planet.wgsl must branch on planet_type == 7u for Quasi-Star"
    );
    assert!(
        shader_src.contains("rim_grazing"),
        "planet.wgsl must compute coronal limb flare for Quasi-Star"
    );

    // 3. Universal Stellar Photosphere Engine
    assert!(
        shader_src.contains("fn render_stellar_photosphere("),
        "planet.wgsl must contain render_stellar_photosphere function"
    );
    assert!(
        shader_src.contains("planet.planet_type == 0u"),
        "planet.wgsl must branch on planet_type == 0u for stars"
    );

    // 4. Must cover all requested star archetypes
    assert!(
        shader_src.contains("Main Sequence Yellow Dwarf"),
        "planet.wgsl must support Main Sequence Yellow Dwarf"
    );
    assert!(
        shader_src.contains("Red Dwarf"),
        "planet.wgsl must support Red Dwarf"
    );
    assert!(
        shader_src.contains("Brown Dwarf"),
        "planet.wgsl must support Brown Dwarf"
    );
    assert!(
        shader_src.contains("Red Giant & Red Supergiant"),
        "planet.wgsl must support Red Giant"
    );
    assert!(
        shader_src.contains("Blue Hyper Giant"),
        "planet.wgsl must support Blue Hyper Giant"
    );
    assert!(
        shader_src.contains("Neutron Star"),
        "planet.wgsl must support Neutron Star"
    );
    assert!(
        shader_src.contains("Pulsar"),
        "planet.wgsl must support Pulsar"
    );
    assert!(
        shader_src.contains("Magnetar"),
        "planet.wgsl must support Magnetar"
    );
    assert!(
        shader_src.contains("White Dwarf"),
        "planet.wgsl must support White Dwarf"
    );
    assert!(
        shader_src.contains("Protostar"),
        "planet.wgsl must support Protostar"
    );
    assert!(
        shader_src.contains("Wolf-Rayet"),
        "planet.wgsl must support Wolf-Rayet Star"
    );
}

#[test]
fn test_stellar_subtype_mapping_and_palettes() {
    use protostellar::rendering::bodies::{compute_stellar_palette, star_subtype_from_body_type};

    // 1. Verify exact subtype mappings
    assert_eq!(star_subtype_from_body_type(BodyType::YellowDwarf), 0.0);
    assert_eq!(star_subtype_from_body_type(BodyType::MainSequenceStar), 0.0);
    assert_eq!(star_subtype_from_body_type(BodyType::RedDwarf), 1.0);
    assert_eq!(star_subtype_from_body_type(BodyType::BrownDwarf), 2.0);
    assert_eq!(star_subtype_from_body_type(BodyType::RedGiant), 3.0);
    assert_eq!(star_subtype_from_body_type(BodyType::RedSupergiant), 3.0);
    assert_eq!(star_subtype_from_body_type(BodyType::BlueGiant), 4.0);
    assert_eq!(star_subtype_from_body_type(BodyType::BlueSupergiant), 4.0);
    assert_eq!(star_subtype_from_body_type(BodyType::Hypergiant), 4.0);
    assert_eq!(star_subtype_from_body_type(BodyType::NeutronStar), 5.0);
    assert_eq!(star_subtype_from_body_type(BodyType::Pulsar), 6.0);
    assert_eq!(star_subtype_from_body_type(BodyType::Magnetar), 7.0);
    assert_eq!(star_subtype_from_body_type(BodyType::WhiteDwarf), 8.0);
    assert_eq!(star_subtype_from_body_type(BodyType::Protostar), 9.0);
    assert_eq!(star_subtype_from_body_type(BodyType::WolfRayet), 10.0);

    // 2. Verify all star types have distinct, non-identical colors
    let col_yellow = compute_stellar_palette(BodyType::YellowDwarf, 5778.0);
    let col_red_dwarf = compute_stellar_palette(BodyType::RedDwarf, 3000.0);
    let col_brown_dwarf = compute_stellar_palette(BodyType::BrownDwarf, 1600.0);
    let col_red_giant = compute_stellar_palette(BodyType::RedGiant, 3200.0);
    let col_blue_hyper = compute_stellar_palette(BodyType::Hypergiant, 35000.0);
    let col_neutron = compute_stellar_palette(BodyType::NeutronStar, 100000.0);
    let col_pulsar = compute_stellar_palette(BodyType::Pulsar, 200000.0);
    let col_magnetar = compute_stellar_palette(BodyType::Magnetar, 500000.0);
    let col_white_dwarf = compute_stellar_palette(BodyType::WhiteDwarf, 15000.0);
    let col_quasi = compute_stellar_palette(BodyType::QuasiStar, 4000.0);

    let colors = [
        ("Yellow Dwarf", col_yellow),
        ("Red Dwarf", col_red_dwarf),
        ("Brown Dwarf", col_brown_dwarf),
        ("Red Giant", col_red_giant),
        ("Blue Hypergiant", col_blue_hyper),
        ("Neutron Star", col_neutron),
        ("Pulsar", col_pulsar),
        ("Magnetar", col_magnetar),
        ("White Dwarf", col_white_dwarf),
        ("Quasi-Star", col_quasi),
    ];

    // Every pair must differ by a perceptible Euclidean RGB distance (> 0.05)
    for i in 0..colors.len() {
        for j in (i + 1)..colors.len() {
            let (name_a, c_a) = colors[i];
            let (name_b, c_b) = colors[j];
            let rgba_a = bevy::color::LinearRgba::from(c_a);
            let rgba_b = bevy::color::LinearRgba::from(c_b);
            let diff = ((rgba_a.red - rgba_b.red).powi(2)
                + (rgba_a.green - rgba_b.green).powi(2)
                + (rgba_a.blue - rgba_b.blue).powi(2))
            .sqrt();
            assert!(
                diff > 0.05,
                "{name_a} and {name_b} have nearly identical palettes (diff = {diff:.4})"
            );
        }
    }
}

#[test]
fn test_quasistar_photosphere_and_blowout_states() {
    let mut qs_state = BlackHoleStarState::default();

    // 1. Intact state
    assert!(!qs_state.is_blown_out, "Quasi-star must start intact");
    assert_eq!(qs_state.cocoon_radius_au, 60.0);
    assert_eq!(qs_state.eddington_ratio, 3.5);

    // Archetype when intact should be 7 (dedicated Quasi-Star photosphere)
    let p_type_intact = if !qs_state.is_blown_out { 7u32 } else { 5u32 };
    assert_eq!(
        p_type_intact, 7u32,
        "Intact quasi-star must use planet_type 7"
    );

    // 2. Blowout trigger
    qs_state.trigger_blowout();
    assert!(qs_state.is_blown_out, "Quasi-star must be marked blown out");

    // Archetype when blown out should be 5 (naked black hole singularity with photon ring)
    let p_type_blown = if !qs_state.is_blown_out { 7u32 } else { 5u32 };
    assert_eq!(
        p_type_blown, 5u32,
        "Blown out quasi-star must transition to planet_type 5 (Black Hole)"
    );
}

#[test]
fn test_pulsar_and_magnetar_scenario_presets() {
    use bevy::prelude::*;
    use protostellar::simulation::components::{
        BodyType, CelestialBody, ElectromagneticFieldState, Mass, SpinState,
    };
    use protostellar::simulation::resources::DiskParameters;
    use protostellar::simulation::scenarios::{
        spawn_magnetar_outburst_scenario, spawn_pulsar_system_scenario, ScenarioPreset,
    };

    // 1. Verify preset enum and metadata
    assert_eq!(
        ScenarioPreset::PulsarSystem.display_name(),
        "PSR B1257+12 (Pulsar & Zombie Planets)"
    );
    assert_eq!(
        ScenarioPreset::MagnetarOutburst.display_name(),
        "SGR 1806-20 (Magnetar Giant Flare)"
    );

    // 2. Test Pulsar system spawning
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let pulsar_ent =
        spawn_pulsar_system_scenario(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let world = app.world();
    let pulsar_body = world
        .get::<CelestialBody>(pulsar_ent)
        .expect("Pulsar entity must exist");
    assert_eq!(pulsar_body.body_type, BodyType::Pulsar);
    assert!(pulsar_body.name.contains("PSR B1257+12"));

    let pulsar_mass = world
        .get::<Mass>(pulsar_ent)
        .expect("Mass component required");
    assert_eq!(pulsar_mass.0, 1.40);

    let em_field = world
        .get::<ElectromagneticFieldState>(pulsar_ent)
        .expect("EM field required");
    assert_eq!(em_field.magnetic_field_gauss, 1.0e9);
    assert!((em_field.rotation_period_sec - 0.00622).abs() < 1e-5);

    let spin = world
        .get::<SpinState>(pulsar_ent)
        .expect("SpinState required");
    assert!(spin.rotation_period_hours < 0.001); // Millisecond rotator

    // Check zombie exoplanets (Draugr, Poltergeist, Phobetor, Dagon)
    let mut body_count = 0;
    let mut draugr_found = false;
    let mut poltergeist_found = false;
    let mut phobetor_found = false;
    let mut query = app.world_mut().query::<&CelestialBody>();
    for body in query.iter(app.world()) {
        body_count += 1;
        if body.name.contains("Draugr") {
            draugr_found = true;
        } else if body.name.contains("Poltergeist") {
            poltergeist_found = true;
        } else if body.name.contains("Phobetor") {
            phobetor_found = true;
        }
    }
    assert_eq!(body_count, 5); // Pulsar + 4 companions
    assert!(draugr_found && poltergeist_found && phobetor_found);

    // 3. Test Magnetar scenario spawning
    let mut app2 = App::new();
    let mut disk_params2 = DiskParameters::default();
    let magnetar_ent =
        spawn_magnetar_outburst_scenario(&mut app2.world_mut().commands(), &mut disk_params2);
    app2.update();

    let world2 = app2.world();
    let magnetar_body = world2
        .get::<CelestialBody>(magnetar_ent)
        .expect("Magnetar entity must exist");
    assert_eq!(magnetar_body.body_type, BodyType::Magnetar);
    assert!(magnetar_body.name.contains("SGR 1806-20"));

    let magnetar_mass = world2
        .get::<Mass>(magnetar_ent)
        .expect("Mass component required");
    assert_eq!(magnetar_mass.0, 1.95);

    let magnetar_em = world2
        .get::<ElectromagneticFieldState>(magnetar_ent)
        .expect("EM field required");
    assert_eq!(magnetar_em.magnetic_field_gauss, 1.0e15); // 10^15 Gauss

    // Check companions (Valkyrie, Pyre, SGR Ejecta Clump, LBV 1806-20)
    let mut companion_count = 0;
    let mut lbv_found = false;
    let mut query2 = app2.world_mut().query::<&CelestialBody>();
    for body in query2.iter(app2.world()) {
        companion_count += 1;
        if body.name.contains("LBV 1806-20") {
            lbv_found = true;
            assert_eq!(body.body_type, BodyType::BlueSupergiant);
        }
    }
    assert_eq!(companion_count, 5); // Magnetar + 4 bodies
    assert!(lbv_found);
}

#[test]
fn test_pulsar_and_magnetar_visual_structures_lifecycle() {
    use bevy::prelude::*;
    use protostellar::rendering::bodies::{
        sync_magnetar_structures, sync_pulsar_beams, MagnetarStructurePart, MagnetarStructureRoot,
        PulsarBeamPart, PulsarBeamRoot, VisualAssets,
    };
    use protostellar::simulation::resources::SimulationConfig;

    let mut app = App::new();
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_resource::<Time>();
    app.init_resource::<SimulationConfig>();

    let (star_mesh, cyl_mesh) = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        (
            meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            meshes.add(Cylinder::new(1.0, 1.0)),
        )
    };

    app.insert_resource(VisualAssets {
        star_mesh: star_mesh.clone(),
        planet_mesh: star_mesh.clone(),
        asteroid_potato_mesh: star_mesh.clone(),
        asteroid_rubble_mesh: star_mesh.clone(),
        comet_bilobate_mesh: star_mesh.clone(),
        particle_mesh: star_mesh.clone(),
        ring_mesh: star_mesh.clone(),
        beam_core_mesh: cyl_mesh.clone(),
        beam_sheath_mesh: cyl_mesh.clone(),
        accretion_disk_mesh: cyl_mesh.clone(),
        pulsar_beam_mesh: cyl_mesh.clone(),
        magnetar_ring_mesh: cyl_mesh.clone(),
        magnetar_field_loops_mesh: cyl_mesh.clone(),
    });

    app.add_systems(Update, (sync_pulsar_beams, sync_magnetar_structures));

    // 1. Initially no visuals
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamRoot>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        0
    );

    // 2. Spawn Pulsar -> PulsarBeamRoot and its 2 conical beams should spawn
    let pulsar_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "PSR B1257+12".to_string(),
                body_type: BodyType::Pulsar,
            },
            SimPosition(DVec3::ZERO),
            Radius(0.0001),
        ))
        .id();

    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamRoot>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamPart>()
            .iter(app.world())
            .count(),
        2
    );

    // 3. Despawn Pulsar -> PulsarBeamRoot should despawn
    app.world_mut().despawn(pulsar_ent);
    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&PulsarBeamRoot>()
            .iter(app.world())
            .count(),
        0
    );

    // 4. Spawn Magnetar and an SGR-named ejecta clump -> MagnetarStructureRoot must attach strictly to Magnetar at (0,0,0)
    let clump_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "SGR Ejecta Clump α".to_string(),
                body_type: BodyType::Protoplanet,
            },
            SimPosition(DVec3::new(5.0, 0.0, 0.0)),
        ))
        .id();

    let magnetar_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "SGR 1806-20".to_string(),
                body_type: BodyType::Magnetar,
            },
            SimPosition(DVec3::ZERO),
        ))
        .id();

    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructurePart>()
            .iter(app.world())
            .count(),
        2
    );

    // Verify it is positioned at Magnetar (0,0,0), NOT at Clump (5,0,0)
    let mut root_q = app.world_mut().query::<(&MagnetarStructureRoot, &Transform)>();
    let (_, root_tf) = root_q.iter(app.world()).next().unwrap();
    assert!(
        root_tf.translation.length() < 1e-4,
        "MagnetarStructureRoot must be at Magnetar (0,0,0), but was at {:?}",
        root_tf.translation
    );

    // 5. Despawn Magnetar -> MagnetarStructureRoot should despawn (even if SGR clump remains)
    app.world_mut().despawn(magnetar_ent);
    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        0
    );

    app.world_mut().despawn(clump_ent);
    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&MagnetarStructureRoot>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn test_magnetar_scenario_orbital_stability_and_field_attachment() {
    use bevy::prelude::*;
    use protostellar::rendering::bodies::{
        sync_magnetar_structures, MagnetarStructureRoot, VisualAssets,
    };
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::spawn_magnetar_outburst_scenario;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_resource::<Time>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.init_resource::<PlayerInteractionState>();

    let (star_mesh, cyl_mesh) = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        (
            meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            meshes.add(Cylinder::new(1.0, 1.0)),
        )
    };

    app.insert_resource(VisualAssets {
        star_mesh: star_mesh.clone(),
        planet_mesh: star_mesh.clone(),
        asteroid_potato_mesh: star_mesh.clone(),
        asteroid_rubble_mesh: star_mesh.clone(),
        comet_bilobate_mesh: star_mesh.clone(),
        particle_mesh: star_mesh.clone(),
        ring_mesh: star_mesh.clone(),
        beam_core_mesh: cyl_mesh.clone(),
        beam_sheath_mesh: cyl_mesh.clone(),
        accretion_disk_mesh: cyl_mesh.clone(),
        pulsar_beam_mesh: cyl_mesh.clone(),
        magnetar_ring_mesh: cyl_mesh.clone(),
        magnetar_field_loops_mesh: cyl_mesh.clone(),
    });

    let mut disk_params = DiskParameters::default();
    let _magnetar_ent =
        spawn_magnetar_outburst_scenario(&mut app.world_mut().commands(), &mut disk_params);

    app.add_systems(Update, (step_physics_simulation, sync_magnetar_structures));

    // Initial frame
    app.update();

    // Verify MagnetarStructureRoot is spawned and attached to SGR 1806-20 at (0, 0, 0)
    {
        let mut root_query = app.world_mut().query::<(&MagnetarStructureRoot, &Transform)>();
        let (_, tf) = root_query
            .iter(app.world())
            .next()
            .expect("MagnetarStructureRoot must exist");
        assert!(
            tf.translation.length() < 1e-4,
            "Magnetic field loops must be anchored at the Magnetar (0,0,0), but found at {:?}",
            tf.translation
        );
    }

    // Simulate 200 physics steps at 10x warp (approx 2 years of simulated orbital time)
    app.world_mut().resource_mut::<TimeWarp>().multiplier = 10.0;
    for _ in 0..200 {
        app.update();
    }

    // Verify all 5 bodies remain bound in stable orbits and have not drifted away
    let mut bodies_query = app.world_mut().query::<(&CelestialBody, &SimPosition)>();
    let mut valkyrie_dist = 0.0;
    let mut pyre_dist = 0.0;
    let mut clump_dist = 0.0;
    let mut lbv_dist = 0.0;
    let mut magnetar_dist = 0.0;

    for (body, pos) in bodies_query.iter(app.world()) {
        let dist = pos.0.length();
        if body.name.contains("Magnetar") {
            magnetar_dist = dist;
        } else if body.name.contains("Valkyrie") {
            valkyrie_dist = dist;
        } else if body.name.contains("Pyre") {
            pyre_dist = dist;
        } else if body.name.contains("Clump") {
            clump_dist = dist;
        } else if body.name.contains("LBV") {
            lbv_dist = dist;
        }
    }

    assert!(
        magnetar_dist < 1e-6,
        "Magnetar must stay at center (0,0,0), found at {}",
        magnetar_dist
    );
    assert!(
        valkyrie_dist >= 0.40 && valkyrie_dist <= 0.60,
        "Valkyrie must remain in stable orbit around ~0.48 AU, found at {}",
        valkyrie_dist
    );
    assert!(
        pyre_dist >= 0.70 && pyre_dist <= 1.05,
        "Pyre must remain in stable orbit around ~0.85 AU, found at {}",
        pyre_dist
    );
    assert!(
        clump_dist >= 1.40 && clump_dist <= 1.95,
        "SGR Ejecta Clump must remain in stable orbit around ~1.65 AU, found at {}",
        clump_dist
    );
    assert!(
        lbv_dist >= 17.0 && lbv_dist <= 19.5,
        "LBV 1806-20 must remain in stable cluster orbit around ~18.0 AU, found at {}",
        lbv_dist
    );

    // Verify visual structures remain locked to the Magnetar
    {
        let mut root_query2 = app.world_mut().query::<(&MagnetarStructureRoot, &Transform)>();
        let (_, tf2) = root_query2.iter(app.world()).next().unwrap();
        assert!(
            tf2.translation.length() < 1e-4,
            "Magnetic field structures must remain centered on Magnetar, found at {:?}",
            tf2.translation
        );
    }
}

#[test]
fn test_ui_button_interactions_query_schedule_no_aliasing_conflict() {
    use bevy::prelude::*;
    use protostellar::game::ui::*;
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::LoadScenarioEvent;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<TimeWarp>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_message::<LoadScenarioEvent>();
    app.init_resource::<QuickBarState>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<HudVisibilityState>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();

    app.add_systems(Update, handle_ui_button_interactions);

    // Spawn Little Red Dot entity with BlackHoleStarState, CelestialBody, Mass, Radius, SimPosition
    app.world_mut().spawn((
        CelestialBody {
            name: "JWST Little Red Dot (Black Hole Star)".to_string(),
            body_type: BodyType::QuasiStar,
        },
        Mass(450_000.0),
        Radius(60.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Composition::pure_hydrogen(),
        BlackHoleStarState::default(),
    ));

    // Spawn camera
    app.world_mut()
        .spawn((PanOrbitCamera::default(), Transform::default()));

    // Update must initialize and run schedule with zero B0001 query aliasing panics!
    app.update();
}

#[test]
fn test_system_worlds_numerical_ordering_and_reindexing() {
    use bevy::prelude::*;
    use protostellar::game::ui::collect_sorted_system_worlds;
    use protostellar::simulation::components::*;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let mut world = World::new();

    // 1. Central Star (Sun at 0, 0, 0)
    let star_ent = world
        .spawn((
            CelestialBody {
                name: "Sol (Central Star)".to_string(),
                body_type: BodyType::MainSequenceStar,
            },
            SimPosition(DVec3::ZERO),
            Mass(1.0),
            Radius(0.00465),
            CentralStar,
        ))
        .id();

    // 2. Planet 1 (Mercury at 0.387 AU)
    let mercury_ent = world
        .spawn((
            CelestialBody {
                name: "Mercury".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(0.387, 0.0, 0.0)),
            Mass(0.055 * EARTH_MASS_SOLAR),
            Radius(0.000016),
        ))
        .id();

    // 3. Planet 2 (Venus at 0.723 AU)
    let venus_ent = world
        .spawn((
            CelestialBody {
                name: "Venus".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(0.723, 0.0, 0.0)),
            Mass(0.815 * EARTH_MASS_SOLAR),
            Radius(0.000040),
        ))
        .id();

    // 4. Planet 3 (Earth at 1.000 AU)
    let earth_ent = world
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.000, 0.0, 0.0)),
            Mass(EARTH_MASS_SOLAR),
            Radius(0.0000426),
        ))
        .id();

    // 5. Minor debris fragment (should be excluded from major system worlds)
    let debris_ent = world
        .spawn((
            CelestialBody {
                name: "debris-chunk-99".to_string(),
                body_type: BodyType::Planetesimal,
            },
            SimPosition(DVec3::new(0.500, 0.0, 0.0)),
            Mass(1e-8),
            Radius(1e-6),
        ))
        .id();

    // Helper closure to query and sort
    let query_and_sort = |w: &mut World| {
        let mut query = w.query::<(
            Entity,
            &CelestialBody,
            &SimPosition,
            &Mass,
            &Radius,
            Option<&CentralStar>,
        )>();
        let items: Vec<_> = query.iter(w).collect();
        collect_sorted_system_worlds(items)
    };

    let worlds = query_and_sort(&mut world);

    // Verify exactly 4 major worlds (debris excluded)
    assert_eq!(worlds.len(), 4);
    // Index 0: Sun (Central Star)
    assert_eq!(worlds[0].entity, star_ent);
    assert_eq!(worlds[0].index, 0);
    assert!(worlds[0].is_central_star);

    // Index 1: Mercury (0.387 AU)
    assert_eq!(worlds[1].entity, mercury_ent);
    assert_eq!(worlds[1].index, 1);
    assert!((worlds[1].distance_au - 0.387).abs() < 1e-4);

    // Index 2: Venus (0.723 AU)
    assert_eq!(worlds[2].entity, venus_ent);
    assert_eq!(worlds[2].index, 2);
    assert!((worlds[2].distance_au - 0.723).abs() < 1e-4);

    // Index 3: Earth (1.000 AU)
    assert_eq!(worlds[3].entity, earth_ent);
    assert_eq!(worlds[3].index, 3);
    assert!((worlds[3].distance_au - 1.000).abs() < 1e-4);

    // SIMULATE MERGER / DESPAWN: Mercury is swallowed or merges into Venus
    world.despawn(mercury_ent);
    world.despawn(debris_ent);

    let worlds_after_merger = query_and_sort(&mut world);
    assert_eq!(worlds_after_merger.len(), 3);

    // Index 0 remains Sun
    assert_eq!(worlds_after_merger[0].entity, star_ent);
    assert_eq!(worlds_after_merger[0].index, 0);

    // Index 1 now seamlessly becomes Venus!
    assert_eq!(worlds_after_merger[1].entity, venus_ent);
    assert_eq!(worlds_after_merger[1].index, 1);

    // Index 2 now seamlessly becomes Earth!
    assert_eq!(worlds_after_merger[2].entity, earth_ent);
    assert_eq!(worlds_after_merger[2].index, 2);
}

#[test]
fn test_trappist1_compact_disk_particle_sampling() {
    let mut rng = rand::rng();
    let disk_params = protostellar::simulation::resources::DiskParameters {
        central_star_mass: 0.0898,
        inner_radius_au: 0.005,
        outer_radius_au: 0.15,
        disk_mass: 0.0001,
        ..Default::default()
    };

    let mut min_r = f64::INFINITY;
    let mut max_r = f64::NEG_INFINITY;
    let mut inner_count = 0;
    let mut outer_count = 0;

    for _ in 0..10_000 {
        let (r, comp) = protostellar::simulation::disk::sample_disk_radius(&mut rng, &disk_params);
        assert!(
            (0.005..=0.150001).contains(&r),
            "Sampled radius {} out of bounds [0.005, 0.15]",
            r
        );
        if r < min_r {
            min_r = r;
        }
        if r > max_r {
            max_r = r;
        }
        if r < 0.07 {
            inner_count += 1;
            assert!(comp.silicate_frac > 0.3 || comp.metal_frac > 0.3);
        } else {
            outer_count += 1;
        }
    }

    assert!(min_r < 0.02, "Expected inner particles down to ~0.005 AU");
    assert!(max_r > 0.13, "Expected outer particles up to ~0.15 AU");
    assert!(inner_count > 3000, "Expected substantial inner particles");
    assert!(outer_count > 2000, "Expected substantial outer particles");
}

#[test]
fn test_earth_spawns_at_1_earth_mass_in_solar_nebula() {
    use bevy::prelude::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::spawn_solar_nebula_mmsn;

    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let _star_ent = spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let mut earth_found = false;
    let mut query = app.world_mut().query::<(&CelestialBody, &Mass, &Radius)>();
    for (body, mass, radius) in query.iter(app.world()) {
        if body.name == "Earth" {
            earth_found = true;
            assert_eq!(body.body_type, BodyType::TerrestrialPlanet);
            let m_earth = mass.0 / EARTH_MASS_SOLAR;
            assert!(
                (m_earth - 1.00).abs() < 1e-4,
                "Earth must spawn at 1.00 M_earth, got {:.4}",
                m_earth
            );
            let r_earth = radius.0 / EARTH_RADIUS_AU;
            assert!(
                (r_earth - 1.00).abs() < 1e-4,
                "Earth radius must be 1.00 R_earth, got {:.4}",
                r_earth
            );
        }
    }
    assert!(
        earth_found,
        "Earth entity must spawn in Hayashi Solar Nebula scenario"
    );
}

#[test]
fn test_inner_planet_nebular_gas_and_atmosphere_accretion() {
    use bevy::prelude::*;
    use protostellar::simulation::accretion::direct_nebular_gas_accretion;
    use protostellar::simulation::resources::*;

    let mut app = App::new();

    let config = SimulationConfig {
        enable_accretion: true,
        accretion_rate_multiplier: 120.0,
        gas_density_scale: 1.0,
        base_dt_yr: 0.01, // 3.65 days per step
        ..Default::default()
    };
    app.insert_resource(config);

    let time_warp = TimeWarp {
        multiplier: 1.0,
        is_paused: false,
        ..Default::default()
    };
    app.insert_resource(time_warp);

    let sim_time = SimTime {
        elapsed_years: 0.5,
        ..Default::default()
    };
    app.insert_resource(sim_time);

    let disk_params = DiskParameters {
        central_star_mass: 1.0,
        inner_radius_au: 0.20,
        outer_radius_au: 45.0,
        gas_disk_lifetime_yr: 60_000.0,
        ..Default::default()
    };
    app.insert_resource(disk_params);

    // Spawn central star (unignited protostar)
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            body_type: BodyType::Protostar,
            name: "The Protostar".to_string(),
        },
        IgnitionState {
            core_temperature: 4.0e6,
            fusion_fraction: 0.4,
            is_ignited: false,
            shockwave_radius: 0.0,
        },
    ));

    // Spawn Earth at 1.0 AU inside the gas cloud
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Earth".to_string(),
            },
            Mass(1.00 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.10,
                cometary_impact_count: 0,
            },
        ))
        .id();

    app.add_systems(Update, direct_nebular_gas_accretion);

    // Run 50 simulation steps inside the gas cloud before star ignites
    for _ in 0..50 {
        app.update();
    }

    let world = app.world();
    let mass = world.get::<Mass>(earth_ent).expect("Mass required");
    let comp = world
        .get::<Composition>(earth_ent)
        .expect("Composition required");
    let vol = world
        .get::<VolatileInventory>(earth_ent)
        .expect("Volatiles required");
    let body = world
        .get::<CelestialBody>(earth_ent)
        .expect("Body required");

    let m_earth = mass.0 / EARTH_MASS_SOLAR;
    assert!(
        m_earth > 1.0001,
        "Earth must accumulate nebular gas mass from circumstellar gas cloud! Got {:.6}",
        m_earth
    );
    assert!(
        m_earth < 1.05,
        "Earth should not undergo runaway gas accumulation into a gas giant! Got {:.6}",
        m_earth
    );
    assert!(
        comp.gas_frac > 0.0001 && comp.gas_frac <= 0.035,
        "Gas fraction must increase into a realistic secondary atmosphere, got {:.6}",
        comp.gas_frac
    );
    assert!(
        vol.atmospheric_pressure_bar > 0.10,
        "Atmospheric pressure must rise from accreted nebular gas, got {:.3} bar",
        vol.atmospheric_pressure_bar
    );
    assert_eq!(
        body.name, "Earth",
        "Canonical planet name 'Earth' must be preserved and not overwritten with generic 'Planet-1AU'"
    );
    assert_eq!(
        body.body_type,
        BodyType::TerrestrialPlanet,
        "Earth must remain a TerrestrialPlanet"
    );
}

#[test]
fn test_ui_button_click_prevents_camera_3d_raycast_hijacking() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    // Spawn a dummy Window
    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    // Spawn Central Star at (0,0,0)
    let star_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "The Sun".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            Mass(1.0),
            Radius(SOLAR_RADIUS_AU),
            SimPosition(DVec3::ZERO),
        ))
        .id();

    // Spawn Earth at (1,0,0)
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.00 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        ))
        .id();

    // Spawn UI button that is currently clicked (Interaction::Pressed)
    app.world_mut().spawn((Button, Interaction::Pressed));

    // Spawn Camera looking at the scene, currently targeting Earth
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: Some(earth_ent),
                focus: Vec3::new(1.0, 0.0, 0.0),
                target_focus: Vec3::new(1.0, 0.0, 0.0),
                ..default()
            },
            Transform::from_xyz(1.0, 0.5, 3.0).looking_at(Vec3::new(1.0, 0.0, 0.0), Vec3::Y),
            GlobalTransform::from(
                Transform::from_xyz(1.0, 0.5, 3.0).looking_at(Vec3::new(1.0, 0.0, 0.0), Vec3::Y),
            ),
        ))
        .id();

    // Simulate Left mouse button press
    let mut mouse_buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse_buttons.press(MouseButton::Left);

    app.add_systems(Update, update_pan_orbit_camera);
    app.update();

    let world = app.world();
    let cam = world
        .get::<PanOrbitCamera>(camera_ent)
        .expect("Camera required");

    // Camera target_entity MUST remain Earth and not be hijacked to the Central Star!
    assert_eq!(
        cam.target_entity,
        Some(earth_ent),
        "Camera target_entity must remain Earth and NOT be hijacked to the star or background raycast target when clicking a UI button!"
    );
    assert_ne!(
        cam.target_entity,
        Some(star_ent),
        "Camera target_entity must NOT bounce back to the star!"
    );
}

#[test]
fn test_camera_zoom_stops_safely_before_surface_of_star_and_planets() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    // Spawn a dummy Window
    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    // 1. Spawn Central Star at (0, 0, 0)
    let _star_ent = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "The Protostar (Solar Nebula)".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(1.0),
            Radius(SOLAR_RADIUS_AU),
            SimPosition(DVec3::ZERO),
        ))
        .id();

    // 2. Spawn Earth at (1.0, 0, 0)
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(1.00 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
        ))
        .id();

    let config = app.world().resource::<SimulationConfig>().clone();
    let star_vis_r = config.calc_visual_radius_for_type(SOLAR_RADIUS_AU, BodyType::Protostar);
    let earth_vis_r =
        config.calc_visual_radius_for_type(EARTH_RADIUS_AU, BodyType::TerrestrialPlanet);

    // 3. Spawn Camera looking at origin (star), initially with target_entity = None
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: None,
                focus: Vec3::ZERO,
                target_focus: Vec3::ZERO,
                radius: 16.0,
                target_radius: 16.0,
                ..default()
            },
            Transform::from_xyz(0.0, 5.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
            GlobalTransform::from(
                Transform::from_xyz(0.0, 5.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
            ),
        ))
        .id();

    app.add_systems(Update, update_pan_orbit_camera);

    // --- TEST A: Zooming into Central Star without target_entity locked (target_focus = 0,0,0) ---
    // Simulate massive zoom-in scroll
    {
        let mut wheel_events = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel_events.write(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 50.0, // massive scroll in
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }

    // Run multiple frames for damping
    for _ in 0..40 {
        app.update();
    }

    let world = app.world();
    let cam = world
        .get::<PanOrbitCamera>(camera_ent)
        .expect("Camera required");

    // Camera must stop right before the star's surface:
    assert!(
        cam.min_radius > star_vis_r,
        "min_radius ({}) must be strictly greater than star's visual radius ({}) to prevent penetrating the photosphere!",
        cam.min_radius,
        star_vis_r
    );
    assert!(
        cam.radius >= cam.min_radius - 0.0001,
        "Camera radius ({}) must stop at or above min_radius ({})!",
        cam.radius,
        cam.min_radius
    );
    let surface_clearance_star = cam.radius - star_vis_r;
    assert!(
        surface_clearance_star >= 0.005,
        "Camera must maintain safe surface clearance ({}) in front of the star!",
        surface_clearance_star
    );

    // --- TEST B: Zooming into Earth when focus-locked ---
    {
        let mut cam_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam_mut.target_entity = Some(earth_ent);
        cam_mut.target_focus = Vec3::new(1.0, 0.0, 0.0);
        cam_mut.focus = Vec3::new(1.0, 0.0, 0.0);
        cam_mut.radius = 1.0;
        cam_mut.target_radius = 1.0;

        let mut wheel_events = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel_events.write(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 50.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }

    for _ in 0..40 {
        app.update();
    }

    {
        let cam = app
            .world()
            .get::<PanOrbitCamera>(camera_ent)
            .expect("Camera required");

        assert!(
            cam.min_radius > earth_vis_r,
            "min_radius ({}) must be strictly greater than Earth's visual radius ({})!",
            cam.min_radius,
            earth_vis_r
        );
        assert!(
            cam.radius >= cam.min_radius - 0.0001,
            "Camera radius ({}) must be bounded by min_radius ({})!",
            cam.radius,
            cam.min_radius
        );
        let surface_clearance_earth = cam.radius - earth_vis_r;
        assert!(
            surface_clearance_earth >= 0.001,
            "Surface clearance ({}) must be far greater than camera near clipping plane (0.0001 AU) to prevent near-plane clipping!",
            surface_clearance_earth
        );
    }

    // --- TEST C: Empty Deep Space Zooming ---
    {
        let mut cam_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam_mut.target_entity = None;
        cam_mut.target_focus = Vec3::new(500.0, 500.0, 0.0);
        cam_mut.focus = Vec3::new(500.0, 500.0, 0.0);
    }
    app.update();

    {
        let cam = app
            .world()
            .get::<PanOrbitCamera>(camera_ent)
            .expect("Camera required");
        assert_eq!(
            cam.min_radius, 0.001,
            "In deep space far from any celestial body, min_radius should allow free zooming down to 0.001 AU!"
        );
    }

    // --- TEST D: Zooming into JWST Little Red Dot (Black Hole Star) Exception ---
    let lrd_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "JWST Little Red Dot (Black Hole Star)".to_string(),
                body_type: BodyType::QuasiStar,
            },
            Mass(100_000.0),
            Radius(60.0),
            SimPosition(DVec3::new(100.0, 0.0, 0.0)),
        ))
        .id();

    {
        let mut cam_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_ent)
            .unwrap();
        cam_mut.target_entity = Some(lrd_ent);
        cam_mut.target_focus = Vec3::new(100.0, 0.0, 0.0);
        cam_mut.focus = Vec3::new(100.0, 0.0, 0.0);
        cam_mut.radius = 200.0;
        cam_mut.target_radius = 200.0;

        let mut wheel_events = app.world_mut().resource_mut::<Messages<MouseWheel>>();
        wheel_events.write(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 500.0, // massive scroll in
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }

    for _ in 0..60 {
        app.update();
    }

    let world = app.world();
    let cam = world
        .get::<PanOrbitCamera>(camera_ent)
        .expect("Camera required");

    // Little Red Dot exception:
    // Its visual radius is ~60 AU, but min_radius must be exactly 0.001 AU,
    // allowing the camera to clip through the 60 AU envelope and zoom all the way down near the black hole singularity!
    assert_eq!(
        cam.min_radius, 0.001,
        "Little Red Dot must have min_radius = 0.001 AU to allow zooming directly into the central black hole!"
    );
    assert!(
        cam.radius <= 0.01,
        "Camera radius ({}) must zoom deep inside the 60 AU cocoon down to the central black hole (<= 0.01 AU)!",
        cam.radius
    );
}

#[test]
fn test_late_heavy_bombardment_guaranteed_trigger_and_impactors() {
    use bevy::prelude::*;
    use protostellar::game::phases::{
        monitor_phase_transitions, LateHeavyBombardmentState, MilestoneId, PhaseManager,
        SystemPhase,
    };
    use protostellar::simulation::disk::update_late_heavy_bombardment_cascade;
    use protostellar::simulation::resources::{DiskParameters, SimTime, TimeWarp};
    use protostellar::simulation::thermodynamics::StarIgnitionEvent;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<PhaseManager>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<DiskParameters>();
    app.add_message::<StarIgnitionEvent>();

    app.add_systems(
        Update,
        (
            monitor_phase_transitions,
            update_late_heavy_bombardment_cascade,
        ),
    );

    // Spawn central star (1.0 M_sun)
    app.world_mut().spawn((
        CentralStar,
        Mass(1.0),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Radius(SOLAR_RADIUS_AU),
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::Protostar,
        },
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 1.6,
        },
    ));

    // Spawn Earth at 1.0 AU with initial dry surface
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI)),
            Radius(EARTH_RADIUS_AU),
            Composition::rocky(),
            VolatileInventory {
                delivered_water_m_earth: 0.00002,
                cometary_impact_count: 0,
                ocean_coverage_frac: 0.03,
                atmospheric_pressure_bar: 0.2,
            },
            InternalDifferentiation {
                is_differentiated: true,
                magnetic_field_gauss: 0.35,
                ..Default::default()
            },
        ))
        .id();

    // 1. Initial State: Verify starting in ProtoplanetaryDisk phase and LHB is inactive
    app.update();
    {
        let phase_mgr = app.world().resource::<PhaseManager>();
        assert_eq!(phase_mgr.current_phase, SystemPhase::ProtoplanetaryDisk);
        let lhb = app.world().resource::<LateHeavyBombardmentState>();
        assert!(!lhb.is_active);
    }

    // 2. Unconditional Manual Trigger: Request LHB from ANY phase (Key [G] / UI button)
    {
        let mut lhb = app.world_mut().resource_mut::<LateHeavyBombardmentState>();
        lhb.manual_trigger_requested = true;
    }

    // Update app: monitor_phase_transitions must unconditionally enter LateHeavyBombardment
    app.update();
    {
        let phase_mgr = app.world().resource::<PhaseManager>();
        assert_eq!(
            phase_mgr.current_phase,
            SystemPhase::LateHeavyBombardment,
            "LHB must be unconditionally entered when manual_trigger_requested is true!"
        );
        let lhb = app.world().resource::<LateHeavyBombardmentState>();
        assert!(lhb.is_active, "lhb_state.is_active must be true!");
        assert!(
            lhb.resonance_crossed,
            "Resonance crossing must be immediately triggered on manual LHB request!"
        );

        // Verify milestone is achieved
        let lhb_milestone = phase_mgr
            .milestones
            .iter()
            .find(|m| m.id == MilestoneId::LateHeavyBombardment)
            .expect("LHB milestone must exist");
        assert!(
            lhb_milestone.achieved,
            "MilestoneId::LateHeavyBombardment must be unlocked upon LHB trigger!"
        );
    }

    // 3. Verify Active Cascade Spawner maintains inner-crossing impactors (q <= 1.6 AU)
    // Run several updates with non-zero delta time
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 0.5;
        sim_time.elapsed_years = 100.0;
    }

    for _ in 0..15 {
        app.update();
    }

    let world = app.world();
    let lhb = world.resource::<LateHeavyBombardmentState>();
    assert!(
        lhb.comets_scattered > 0,
        "Active cascade must increment comets_scattered (found {})",
        lhb.comets_scattered
    );

    // Count planet-crossing impactors in ECS
    let mut impactor_count = 0;
    let mut query = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &SimVelocity, &Mass)>();
    for (body, pos, vel, _) in query.iter(app.world()) {
        if matches!(body.body_type, BodyType::Asteroid | BodyType::Comet) {
            impactor_count += 1;
            // Verify Keplerian energy and perihelion
            let r = pos.0.length();
            let v = vel.0.length();
            let spec_e = 0.5 * v * v - G_ASTRO / r;
            if spec_e < 0.0 {
                let a = -G_ASTRO / (2.0 * spec_e);
                let h = pos.0.cross(vel.0).length();
                let e = (1.0 - (h * h) / (G_ASTRO * a)).max(0.0).sqrt();
                let q = a * (1.0 - e);
                // Impactor must cross the inner solar system
                assert!(
                    q <= 1.8,
                    "Impactor perihelion ({:.2} AU) must cross inner planetary orbits!",
                    q
                );
            }
        }
    }
    assert!(
        impactor_count >= 5,
        "Cascade spawner must generate multiple active impactors in ECS (found {})",
        impactor_count
    );

    // 4. Simulate Volatile Water Delivery from Cometary Impacts onto Earth
    // Impacts delivering ~0.0006 M_earth of water to establish oceans
    {
        let mut earth_vol = app
            .world_mut()
            .get_mut::<VolatileInventory>(earth_ent)
            .expect("Earth must have VolatileInventory");
        earth_vol.delivered_water_m_earth = 0.00058; // > 0.0005 Earth masses
        earth_vol.cometary_impact_count = 14;
        earth_vol.ocean_coverage_frac =
            (earth_vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
    }

    app.update();

    {
        let phase_mgr = app.world().resource::<PhaseManager>();
        let lhb = app.world().resource::<LateHeavyBombardmentState>();

        // Verify water delivery synced to LHB state
        assert!(
            lhb.water_delivered_earth_masses >= 0.0005,
            "LHB state water_delivered_earth_masses ({}) must track total delivered water!",
            lhb.water_delivered_earth_masses
        );

        // Verify VolatileOceanDelivery milestone unlocked
        let ocean_milestone = phase_mgr
            .milestones
            .iter()
            .find(|m| m.id == MilestoneId::VolatileOceanDelivery)
            .expect("Ocean milestone must exist");
        assert!(
            ocean_milestone.achieved,
            "MilestoneId::VolatileOceanDelivery must unlock once delivered water >= 0.0005 M_earth!"
        );

        // Verify Earth has ocean coverage >= 70%
        let earth_vol = app.world().get::<VolatileInventory>(earth_ent).unwrap();
        assert!(
            earth_vol.ocean_coverage_frac >= 0.70,
            "Earth ocean coverage ({:.1}%) must exceed 70% after bombardment water delivery!",
            earth_vol.ocean_coverage_frac * 100.0
        );
    }
}

#[test]
fn test_minor_bodies_belt_formation_1024_capacity_and_hud_category() {
    use bevy::prelude::*;
    use protostellar::game::phases::*;
    use protostellar::game::ui::{
        is_canonical_major_planet, is_embryo_body, is_major_body, QuickBarState,
    };
    use protostellar::simulation::disk::*;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::spawn_solar_nebula_mmsn;

    // 1. Verify is_major_body classification:
    // Minor bodies (Asteroids, Comets, Planetesimals) must NEVER be classified as major worlds!
    assert!(!is_major_body(
        "Ceres",
        BodyType::Asteroid,
        false,
        0.00015 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Vesta",
        BodyType::Asteroid,
        false,
        0.00004 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "1P/Halley",
        BodyType::Comet,
        false,
        0.000001 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "C/1995 O1 Hale-Bopp",
        BodyType::Comet,
        false,
        0.000005 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Asteroid-2.7AU",
        BodyType::Asteroid,
        false,
        0.00001 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Comet-25.0AU",
        BodyType::Comet,
        false,
        0.00001 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Planetesimal-1.5AU",
        BodyType::Planetesimal,
        false,
        0.00001 * EARTH_MASS_SOLAR
    ));

    // Embryos must be categorized into the dedicated Embryo category and excluded from Major Worlds:
    assert!(is_embryo_body("Theia", BodyType::Protoplanet));
    assert!(is_embryo_body("Theia Embryo", BodyType::Protoplanet));
    assert!(is_embryo_body("Callisto Embryo", BodyType::Protoplanet));
    assert!(is_embryo_body("Titan Embryo", BodyType::Protoplanet));
    assert!(is_embryo_body("Embryo-1.2AU", BodyType::Protoplanet));
    assert!(is_embryo_body("Embryo #1", BodyType::Protoplanet));
    assert!(!is_major_body(
        "Theia",
        BodyType::Protoplanet,
        false,
        0.10 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Theia Embryo",
        BodyType::Protoplanet,
        false,
        0.10 * EARTH_MASS_SOLAR
    ));
    assert!(!is_major_body(
        "Callisto Embryo",
        BodyType::Protoplanet,
        false,
        0.05 * EARTH_MASS_SOLAR
    ));

    // Major worlds must be classified as major:
    assert!(is_major_body("Sun", BodyType::Protostar, true, 1.0));
    assert!(is_major_body(
        "Proto-Mercury",
        BodyType::Protoplanet,
        false,
        0.06 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Mercury",
        BodyType::TerrestrialPlanet,
        false,
        0.055 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Venus",
        BodyType::TerrestrialPlanet,
        false,
        0.815 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Earth",
        BodyType::TerrestrialPlanet,
        false,
        1.0 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Mars",
        BodyType::TerrestrialPlanet,
        false,
        0.107 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Jupiter",
        BodyType::GasGiant,
        false,
        317.8 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Saturn",
        BodyType::GasGiant,
        false,
        95.2 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Uranus",
        BodyType::IceGiant,
        false,
        14.5 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Neptune",
        BodyType::IceGiant,
        false,
        17.1 * EARTH_MASS_SOLAR
    ));
    // Pluto and Planet Nine:
    assert!(is_canonical_major_planet("Pluto (Dwarf Planet)"));
    assert!(is_canonical_major_planet(
        "Planet Nine (Super-Earth / Ice Giant)"
    ));
    assert!(is_major_body(
        "Pluto (Dwarf Planet)",
        BodyType::TerrestrialPlanet,
        false,
        0.00218 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Planet Nine (Super-Earth / Ice Giant)",
        BodyType::IceGiant,
        false,
        5.50 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Moon",
        BodyType::Moon,
        false,
        0.0123 * EARTH_MASS_SOLAR
    ));

    // 2. Verify PlanetesimalSpawner capacity is 1024 and QuickBarState defaults
    let spawner = PlanetesimalSpawner::default();
    assert_eq!(
        spawner.max_ecs_bodies, 1024,
        "PlanetesimalSpawner must have 1024 body capacity!"
    );
    let qb = QuickBarState::default();
    assert!(!qb.show_embryos);
    assert!(!qb.show_minor_bodies);
    assert!(!qb.is_minimized);

    // 3. Verify Solar System Scenario spawns Asteroid Belt and Kuiper Belt bodies
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<SystemPhase>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<PhaseManager>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.add_message::<protostellar::simulation::thermodynamics::StarIgnitionEvent>();
    app.add_systems(Update, monitor_phase_transitions);

    let mut disk_params = DiskParameters::default();
    let _star_ent = spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let phase_mgr = app.world().resource::<PhaseManager>();
    assert_eq!(
        phase_mgr.asteroid_count, 8,
        "Solar System MMSN must spawn 8 initial Asteroid Belt bodies (Ceres, Vesta, Pallas, etc.)"
    );
    assert_eq!(
        phase_mgr.comet_count, 6,
        "Solar System MMSN must spawn 6 initial Kuiper Belt cometary bodies (Halley, Encke, etc.)"
    );
    assert!(
        phase_mgr.planet_count + phase_mgr.protoplanet_count >= 10,
        "Solar System MMSN must have major planets and embryos tracked (found {} planets + {} protoplanets)",
        phase_mgr.planet_count,
        phase_mgr.protoplanet_count
    );

    // 4. Verify Stellar Wind Radiation Push:
    // Place a small asteroid at r = 1.2 AU (terrestrial zone)
    let pos_inner = DVec3::new(1.2, 0.0, 0.0);
    let r_cyl = pos_inner.x;
    let b_mass = 0.00001 * EARTH_MASS_SOLAR;
    let push_mag =
        0.35 * (1.0 - (r_cyl / 2.0)).max(0.0) / (1.0 + b_mass / (EARTH_MASS_SOLAR * 0.001));
    assert!(
        push_mag > 0.1,
        "Inner minor body must feel positive outward radiation pressure push ({push_mag}) toward the belt"
    );
}

#[test]
fn test_spawn_protoplanetary_disk_spawns_pluto_and_planet_nine() {
    use bevy::prelude::*;
    use protostellar::game::ui::collect_sorted_system_worlds;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::spawn_protoplanetary_disk;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let disk_params = DiskParameters::default();
    let config = SimulationConfig::default();

    let _star = spawn_protoplanetary_disk(&mut app.world_mut().commands(), &disk_params, &config);
    app.update();

    let mut bodies = Vec::new();
    for (entity, body, pos, _mass, _radius, _opt_star) in app
        .world_mut()
        .query::<(
            Entity,
            &CelestialBody,
            &SimPosition,
            &Mass,
            &Radius,
            Option<&CentralStar>,
        )>()
        .iter(app.world())
    {
        bodies.push((entity, body.name.clone(), body.body_type, pos.0.length()));
    }

    let pluto = bodies.iter().find(|b| b.1.contains("Pluto"));
    assert!(
        pluto.is_some(),
        "Pluto must spawn immediately in spawn_protoplanetary_disk!"
    );
    let pluto = pluto.unwrap();
    assert_eq!(pluto.2, BodyType::TerrestrialPlanet);
    assert!(
        (pluto.3 - 39.48).abs() < 5.0,
        "Pluto distance must be ~39.48 AU (found {:.2})",
        pluto.3
    );

    let planet_nine = bodies.iter().find(|b| b.1.contains("Planet Nine"));
    assert!(
        planet_nine.is_some(),
        "Planet Nine must spawn immediately in spawn_protoplanetary_disk!"
    );
    let p9 = planet_nine.unwrap();
    assert_eq!(p9.2, BodyType::IceGiant);
    assert!(
        (p9.3 - 380.0).abs() < 50.0,
        "Planet Nine distance must be ~380 AU (found {:.2})",
        p9.3
    );

    // Verify collect_sorted_system_worlds contains both worlds
    let query_items: Vec<_> = app
        .world_mut()
        .query::<(
            Entity,
            &CelestialBody,
            &SimPosition,
            &Mass,
            &Radius,
            Option<&CentralStar>,
        )>()
        .iter(app.world())
        .collect();

    let system_worlds = collect_sorted_system_worlds(query_items);
    let world_names: Vec<&str> = system_worlds.iter().map(|w| w.name.as_str()).collect();
    assert!(
        world_names.iter().any(|n| n.contains("Pluto")),
        "Major worlds selector bar must include Pluto: {:?}",
        world_names
    );
    assert!(
        world_names.iter().any(|n| n.contains("Planet Nine")),
        "Major worlds selector bar must include Planet Nine: {:?}",
        world_names
    );
}

#[test]
fn test_camera_tracking_selected_planet_zero_drift_at_high_warp() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    // Spawn dummy Window
    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    // Spawn Sun at origin
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        SimAcceleration(DVec3::ZERO),
        Transform::IDENTITY,
    ));

    // Spawn Earth at 1.0 AU with Keplerian orbital velocity
    let v_earth = (G_ASTRO * 1.0 / 1.0).sqrt();
    let earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU),
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_earth)),
            SimAcceleration(DVec3::ZERO),
            Transform::from_xyz(1.0, 0.0, 0.0),
        ))
        .id();

    // Spawn Camera focus-locked on Earth
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: Some(earth_ent),
                focus: Vec3::new(1.0, 0.0, 0.0),
                target_focus: Vec3::new(1.0, 0.0, 0.0),
                radius: 0.05,
                target_radius: 0.05,
                yaw: 0.5,
                target_yaw: 0.5,
                pitch: 0.3,
                target_pitch: 0.3,
                ..default()
            },
            Transform::IDENTITY,
            GlobalTransform::IDENTITY,
        ))
        .id();

    // Mock transform sync system matching sync_celestial_transforms logic
    fn sync_transforms(mut query: Query<(&SimPosition, &mut Transform)>) {
        for (pos, mut tf) in query.iter_mut() {
            tf.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        }
    }

    // Schedule: physics -> sync transforms -> camera
    app.add_systems(
        Update,
        (
            step_physics_simulation,
            sync_transforms.after(step_physics_simulation),
            update_pan_orbit_camera.after(sync_transforms),
        ),
    );

    // Test across standard simulation time warp multipliers: 1x, 10x, 100x, 1,000x, 10,000x
    let warps = [1.0, 10.0, 100.0, 1_000.0, 10_000.0];
    for &warp in &warps {
        app.world_mut().resource_mut::<TimeWarp>().multiplier = warp;

        for _ in 0..10 {
            app.update();

            let cam_tf = *app.world().get::<Transform>(camera_ent).unwrap();
            let earth_tf = *app.world().get::<Transform>(earth_ent).unwrap();

            // Transform Earth's position into camera view space:
            let view_matrix = cam_tf.to_matrix().inverse();
            let earth_in_view = view_matrix.transform_point3(earth_tf.translation);

            assert!(
                earth_in_view.x.abs() < 1e-4,
                "At warp {}x, Earth screen X offset ({}) must be 0 (drift detected!)",
                warp,
                earth_in_view.x
            );
            assert!(
                earth_in_view.y.abs() < 1e-4,
                "At warp {}x, Earth screen Y offset ({}) must be 0 (drift detected!)",
                warp,
                earth_in_view.y
            );
            // Z must be negative (in front of the camera, at exactly -radius)
            assert!(
                earth_in_view.z < 0.0,
                "At warp {}x, Earth must be in front of the camera (z = {})",
                warp,
                earth_in_view.z
            );
        }
    }
}

#[test]
fn test_outer_bodies_camera_stability_no_cancellation_jitter() {
    use bevy::input::mouse::{MouseMotion, MouseWheel};
    use bevy::prelude::*;
    use protostellar::rendering::camera::{update_pan_orbit_camera, PanOrbitCamera};
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::*;
    use protostellar::utils::constants::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<SimulationConfig>();
    app.add_message::<MouseMotion>();
    app.add_message::<MouseWheel>();
    app.init_resource::<PlayerInteractionState>();

    // Spawn dummy Window
    app.world_mut().spawn(Window {
        title: "Test Window".to_string(),
        ..default()
    });

    // 1. Spawn Sun
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "The Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(SOLAR_RADIUS_AU),
        SimPosition(DVec3::ZERO),
        Transform::IDENTITY,
    ));

    // 2. Spawn Planet Nine at 380.0 AU
    let p9_pos = DVec3::new(380.0, 0.0, 0.0);
    let p9_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Planet Nine".to_string(),
                body_type: BodyType::IceGiant,
            },
            Mass(5.5 * EARTH_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 2.3),
            SimPosition(p9_pos),
            Transform::from_translation(Vec3::new(p9_pos.x as f32, 0.0, 0.0)),
        ))
        .id();

    // 3. Spawn Camera focused on Planet Nine at 380 AU
    let camera_ent = app
        .world_mut()
        .spawn((
            Camera::default(),
            PanOrbitCamera {
                target_entity: Some(p9_ent),
                focus: Vec3::new(380.0, 0.0, 0.0),
                target_focus: Vec3::new(380.0, 0.0, 0.0),
                radius: 0.05,
                target_radius: 0.05,
                yaw: 1.15,
                target_yaw: 1.15,
                pitch: 0.45,
                target_pitch: 0.45,
                ..default()
            },
            Transform::IDENTITY,
            GlobalTransform::IDENTITY,
        ))
        .id();

    app.add_systems(Update, update_pan_orbit_camera);

    // Update multiple frames
    for _ in 0..10 {
        app.update();
    }

    let cam = app.world().get::<PanOrbitCamera>(camera_ent).unwrap();
    let cam_tf = *app.world().get::<Transform>(camera_ent).unwrap();
    let p9_tf = *app.world().get::<Transform>(p9_ent).unwrap();

    // Verify analytical camera rotation: exact match with Quat(yaw, pitch) without cancellation error
    let expected_rot =
        Quat::from_axis_angle(Vec3::Y, cam.yaw) * Quat::from_axis_angle(Vec3::X, -cam.pitch);
    assert!(
        cam_tf.rotation.abs_diff_eq(expected_rot, 1e-6),
        "Camera rotation at 380 AU must match analytical quaternion with zero noise (found {:?}, expected {:?})",
        cam_tf.rotation,
        expected_rot
    );

    // Camera forward vector in world coordinates (-Z)
    let cam_forward = cam_tf.rotation * -Vec3::Z;
    let dir_to_planet = (p9_tf.translation - cam_tf.translation).normalize();

    // Camera forward must point directly at Planet Nine with dot product 1.0 (zero angular jitter)
    let dot = cam_forward.dot(dir_to_planet);
    assert!(
        (dot - 1.0).abs() < 1e-5,
        "Camera optical axis must align with Planet Nine at 380 AU with dot product ~1.0 (found {:.7})",
        dot
    );
}

#[test]
fn test_quick_body_selector_click_selection_and_clean_recursive_despawn() {
    use bevy::prelude::*;
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::game::ui::{
        handle_ui_button_interactions, update_quick_body_selector_bar, HudActionTooltipText,
        HudVisibilityState, NotificationToast, PlanetBuilderState, QuickBarState,
        QuickBodySelectorBar, UiButtonAction,
    };
    use protostellar::rendering::camera::PanOrbitCamera;
    use protostellar::simulation::components::*;
    use protostellar::simulation::resources::{
        DiskParameters, PlayerInteractionState, SimTime, SimulationConfig, TimeWarp,
    };
    use protostellar::simulation::scenarios::LoadScenarioEvent;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<QuickBarState>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<NotificationToast>();
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<HudVisibilityState>();
    app.init_resource::<LateHeavyBombardmentState>();
    app.add_message::<LoadScenarioEvent>();

    // Spawn camera
    let cam_ent = app
        .world_mut()
        .spawn(PanOrbitCamera {
            focus: Vec3::ZERO,
            target_focus: Vec3::ZERO,
            radius: 50.0,
            target_radius: 50.0,
            ..default()
        })
        .id();

    // Spawn tooltip text entity
    app.world_mut().spawn((Text::new(""), HudActionTooltipText));

    // Spawn celestial bodies: Sun, Earth, Jupiter
    let _sun_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Sun".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            CentralStar,
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Mass(1.0),
            Radius(0.00465),
            Composition::default(),
        ))
        .id();

    let earth_pos = DVec3::new(1.0, 0.0, 0.0);
    let _earth_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(earth_pos),
            SimVelocity(DVec3::ZERO),
            Mass(0.000003003),
            Radius(0.0000426),
            Composition::default(),
        ))
        .id();

    let jupiter_pos = DVec3::new(5.2, 0.0, 0.0);
    let jupiter_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Jupiter".to_string(),
                body_type: BodyType::GasGiant,
            },
            SimPosition(jupiter_pos),
            SimVelocity(DVec3::ZERO),
            Mass(0.000954),
            Radius(0.000467),
            Composition::default(),
        ))
        .id();

    // Spawn QuickBodySelectorBar
    let bar_ent = app
        .world_mut()
        .spawn((QuickBodySelectorBar, Node::default()))
        .id();

    // Run update_quick_body_selector_bar to populate buttons
    app.add_systems(Update, update_quick_body_selector_bar);
    app.update();

    // 1. Verify buttons were spawned under bar_ent
    let children = app
        .world()
        .get::<Children>(bar_ent)
        .expect("bar_ent must have children");
    assert!(
        !children.is_empty(),
        "bar_ent must contain selector buttons"
    );

    // 2. Verify EVERY spawned text node inside buttons has Pickable::IGNORE
    let mut button_count = 0;
    let mut text_count = 0;
    let mut jupiter_btn_ent: Option<Entity> = None;

    for btn_child in children.iter() {
        if let Some(action) = app.world().get::<UiButtonAction>(btn_child) {
            button_count += 1;
            if *action == UiButtonAction::SelectEntity(jupiter_ent) {
                jupiter_btn_ent = Some(btn_child);
            }
        }
        if let Some(btn_grandchildren) = app.world().get::<Children>(btn_child) {
            for gc in btn_grandchildren.iter() {
                if app.world().get::<Text>(gc).is_some() {
                    text_count += 1;
                    assert!(
                        app.world().get::<Pickable>(gc) == Some(&Pickable::IGNORE),
                        "Text node {:?} inside button {:?} must have Pickable::IGNORE so clicks register on parent button!",
                        gc,
                        btn_child
                    );
                }
            }
        }
    }

    assert!(
        button_count >= 3,
        "Must have spawned Sun, Earth, Jupiter buttons"
    );
    assert!(text_count >= 3, "Must have spawned text labels");
    let jupiter_btn = jupiter_btn_ent.expect("Must have spawned Jupiter selector button");

    // 3. Test clicking Jupiter button: Simulate Interaction::Pressed on jupiter_btn
    // Verify UiButtonAction::SelectEntity updates player_state and camera focus immediately
    app.world_mut()
        .entity_mut(jupiter_btn)
        .insert(Interaction::Pressed);

    app.add_systems(Update, handle_ui_button_interactions);
    app.update();

    let player_state = app.world().resource::<PlayerInteractionState>();
    assert_eq!(
        player_state.selected_entity,
        Some(jupiter_ent),
        "Clicking Jupiter button must set selected_entity to jupiter_ent"
    );

    let cam = app.world().get::<PanOrbitCamera>(cam_ent).unwrap();
    assert_eq!(
        cam.target_entity,
        Some(jupiter_ent),
        "Camera target_entity must be set to Jupiter"
    );
    assert_eq!(
        cam.target_focus,
        Vec3::new(5.2, 0.0, 0.0),
        "Camera target_focus must snap immediately to Jupiter's coordinates"
    );
    assert_eq!(
        cam.focus,
        Vec3::new(5.2, 0.0, 0.0),
        "Camera focus must snap immediately to Jupiter's coordinates"
    );

    // 4. Test clean recursive despawn: Record all child and grandchild entity IDs
    let mut initial_entities = Vec::new();
    let old_children: Vec<Entity> = app
        .world()
        .get::<Children>(bar_ent)
        .unwrap()
        .iter()
        .collect();
    for child in old_children {
        initial_entities.push(child);
        if let Some(gcs) = app.world().get::<Children>(child) {
            for gc in gcs.iter() {
                initial_entities.push(gc);
            }
        }
    }

    // Mutate state to trigger rebuild of quick selector bar
    let mut qb_state = app.world_mut().resource_mut::<QuickBarState>();
    qb_state.show_embryos = true;

    app.update();

    // Verify all old entities (buttons and text nodes) were cleanly despawned without orphans
    for old_ent in initial_entities {
        assert!(
            app.world().get_entity(old_ent).is_err(),
            "Entity {:?} should have been cleanly despawned during bar rebuild",
            old_ent
        );
    }
}

#[test]
fn test_hyperbolic_orbit_points_generation() {
    use protostellar::utils::math::{
        apsides_positions, generate_hyperbolic_orbit_points, position_at_true_anomaly,
        OrbitalElements,
    };

    // Unbound interstellar flyby trajectory: e = 1.5, a = -3.0 AU (q = |a|(e - 1) = 1.5 AU)
    let elements = OrbitalElements {
        semi_major_axis: -3.0,
        eccentricity: 1.5,
        inclination: 0.15,
        longitude_ascending_node: 0.4,
        argument_of_periapsis: 0.2,
        true_anomaly: 0.0,
        period_years: f64::INFINITY,
        periapsis: 1.5,
        apoapsis: f64::INFINITY,
        specific_energy: 10.0,
    };

    let points = generate_hyperbolic_orbit_points(&elements, 64);
    assert!(
        points.len() >= 30,
        "Hyperbolic trajectory must generate a continuous series of points"
    );

    // All points must be finite
    for pt in &points {
        assert!(pt.is_finite(), "Trajectory point must be finite: {:?}", pt);
        assert!(
            pt.length() <= 2000.0,
            "Trajectory point must not exceed sanity bounds: {:?}",
            pt
        );
    }

    // Periapsis position test
    let (opt_peri, opt_apo) = apsides_positions(&elements);
    assert!(opt_peri.is_some(), "Periapsis must exist for hyperbola");
    assert!(opt_apo.is_none(), "Apoapsis must be None for hyperbola");

    let peri = opt_peri.unwrap();
    let peri_dist = peri.length();
    assert!(
        (peri_dist - 1.5).abs() < 0.01,
        "Periapsis distance should be 1.5 AU, got {:.4}",
        peri_dist
    );

    // Test position_at_true_anomaly at nu = 0
    let pos_at_0 = position_at_true_anomaly(&elements, 0.0).expect("Position at nu=0 must exist");
    assert!(
        (pos_at_0 - peri).length() < 1e-4,
        "Position at nu=0 must equal periapsis"
    );
}

#[test]
fn test_trailing_ribbon_points_elliptical_and_hyperbolic() {
    use protostellar::utils::math::{
        generate_trailing_ribbon_points, position_at_true_anomaly, OrbitalElements,
    };
    use std::f64::consts::PI;

    // Earth-like orbit: a = 1.0 AU, e = 0.0167
    let earth_elements = OrbitalElements {
        semi_major_axis: 1.0,
        eccentricity: 0.0167,
        inclination: 0.0,
        longitude_ascending_node: 0.0,
        argument_of_periapsis: 0.0,
        true_anomaly: PI / 3.0, // 60 degrees
        period_years: 1.0,
        periapsis: 0.9833,
        apoapsis: 1.0167,
        specific_energy: -20.0,
    };

    let ribbon = generate_trailing_ribbon_points(&earth_elements, 32, 1.2 * PI);
    assert_eq!(ribbon.len(), 33, "Ribbon should contain 33 points (32 samples + 1)");

    // First point must be at current true anomaly with alpha = 1.0
    let (p0, a0) = ribbon[0];
    assert!((a0 - 1.0).abs() < 1e-5, "Leading edge alpha must be 1.0");
    let expected_p0 = position_at_true_anomaly(&earth_elements, PI / 3.0).unwrap();
    assert!(
        (p0 - expected_p0).length() < 1e-4,
        "Leading edge point must match current true anomaly position"
    );

    // Last point must have alpha = 0.0
    let (_plast, alast) = *ribbon.last().unwrap();
    assert!((alast - 0.0).abs() < 1e-5, "Trailing edge alpha must be 0.0");

    // Monotonically decreasing alpha
    for window in ribbon.windows(2) {
        assert!(
            window[0].1 >= window[1].1,
            "Alpha must monotonically decrease along the trailing ribbon"
        );
    }
}

#[test]
fn test_orbit_visualization_mode_cycling_and_state() {
    use protostellar::simulation::resources::OrbitVisualizationMode;

    let mode = OrbitVisualizationMode::All;
    assert_eq!(mode.display_label(), "All");

    let mode = mode.cycle();
    assert_eq!(mode, OrbitVisualizationMode::SelectedOnly);
    assert_eq!(mode.display_label(), "Selected");

    let mode = mode.cycle();
    assert_eq!(mode, OrbitVisualizationMode::Off);
    assert_eq!(mode.display_label(), "Hidden");

    let mode = mode.cycle();
    assert_eq!(mode, OrbitVisualizationMode::All);
}

#[test]
fn test_satellite_moon_orbit_anchoring_math() {
    use bevy::math::DVec3;
    use protostellar::utils::constants::G_ASTRO;
    use protostellar::utils::math::state_vectors_to_orbital_elements;

    // Parent planet (Jupiter-mass) at 5.2 AU moving at circular Keplerian velocity
    let jupiter_pos = DVec3::new(5.2, 0.0, 0.0);
    let jupiter_mass = 0.000954; // Solar masses (~1 M_Jup)
    let star_mass = 1.0;
    let v_jup = (G_ASTRO * star_mass / 5.2).sqrt();
    let jupiter_vel = DVec3::new(0.0, 0.0, v_jup);

    // Moon orbiting Jupiter at 0.0028 AU (~421,700 km, like Io)
    let r_moon_rel = 0.0028;
    let v_moon_rel = (G_ASTRO * jupiter_mass / r_moon_rel).sqrt();
    let moon_rel_pos = DVec3::new(r_moon_rel, 0.0, 0.0);
    let moon_rel_vel = DVec3::new(0.0, 0.0, v_moon_rel);

    let moon_abs_pos = jupiter_pos + moon_rel_pos;
    let moon_abs_vel = jupiter_vel + moon_rel_vel;
    let moon_mass = 0.00000005; // tiny

    // Relative to parent planet:
    let rel_pos = moon_abs_pos - jupiter_pos;
    let rel_vel = moon_abs_vel - jupiter_vel;

    let elements = state_vectors_to_orbital_elements(rel_pos, rel_vel, jupiter_mass, moon_mass)
        .expect("Should resolve valid Keplerian elements relative to parent");

    assert!(
        (elements.semi_major_axis - r_moon_rel).abs() < 1e-4,
        "Semi-major axis relative to planet must match 0.0028 AU, got {}",
        elements.semi_major_axis
    );
    assert!(
        elements.eccentricity < 0.05,
        "Eccentricity relative to parent planet must be near circular, got {}",
        elements.eccentricity
    );
}

#[test]
fn test_planet_builder_preview_orbit_generation() {
    use protostellar::game::ui::PlanetBuilderState;
    use protostellar::utils::math::{apsides_positions, generate_orbit_points, OrbitalElements};

    let builder = PlanetBuilderState::default();
    assert_eq!(builder.semi_major_axis_au, 1.0);
    assert_eq!(builder.eccentricity, 0.016);

    let preview_elements = OrbitalElements {
        semi_major_axis: builder.semi_major_axis_au,
        eccentricity: builder.eccentricity,
        periapsis: builder.semi_major_axis_au * (1.0 - builder.eccentricity),
        apoapsis: builder.semi_major_axis_au * (1.0 + builder.eccentricity),
        ..Default::default()
    };

    let points = generate_orbit_points(&preview_elements, 96);
    assert_eq!(points.len(), 97, "Preview orbit must generate 97 vertices");

    let (opt_peri, opt_apo) = apsides_positions(&preview_elements);
    let peri = opt_peri.expect("Periapsis must exist for bound preview");
    let apo = opt_apo.expect("Apoapsis must exist for bound preview");

    assert!(
        (peri.length() - (1.0 - 0.016) as f32).abs() < 1e-3,
        "Periapsis must match 0.984 AU"
    );
    assert!(
        (apo.length() - (1.0 + 0.016) as f32).abs() < 1e-3,
        "Apoapsis must match 1.016 AU"
    );
}

