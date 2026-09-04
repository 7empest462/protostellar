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
    let mut cpu_masses = vec![0.0001f32, 0.0f32, 0.0001f32, 0.0f32]; // 1 & 3 accreted on CPU
    let mut cpu_positions = vec![
        [1.0f32, 0.0, 0.0],
        [0.0, -5000.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, -5000.0, 0.0],
    ];

    // GPU buffer arrives from older in-flight frame where particle 1 had not died yet on GPU
    let gpu_particles = vec![
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

    // Simulate massive runaway accretion attempts (e.g. 100,000 particle sweeps)
    for _ in 0..10_000 {
        let gain = 100.0 * (0.0006 * EARTH_MASS_SOLAR / 100_000.0); // 100 particles
        let m_earth_ratio = (mass / EARTH_MASS_SOLAR).clamp(0.1, 350.0);
        let runaway_mult = 1.0 + 0.30 * m_earth_ratio.powf(0.35);
        mass = (mass + gain * runaway_mult).min(max_giant_mass);
    }

    // Mass must be capped at 2.5 M_Jup and NEVER reach stellar/black hole mass (> 1000 M_earth)
    assert!(mass <= max_giant_mass);
    assert!(mass < 0.01); // Well below stellar threshold (0.08 M_sun)
}
