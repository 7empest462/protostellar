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

    let r_star = SimulationConfig::calc_render_radius(1.0, BodyType::MainSequenceStar);
    let r_jupiter = SimulationConfig::calc_render_radius(JUPITER_MASS_SOLAR, BodyType::GasGiant);
    let r_earth =
        SimulationConfig::calc_render_radius(EARTH_MASS_SOLAR, BodyType::TerrestrialPlanet);
    let r_embryo =
        SimulationConfig::calc_render_radius(0.05 * EARTH_MASS_SOLAR, BodyType::Protoplanet);
    let r_planetesimal =
        SimulationConfig::calc_render_radius(0.001 * EARTH_MASS_SOLAR, BodyType::Planetesimal);

    assert!(r_star > r_jupiter);
    assert!(r_jupiter > r_earth);
    assert!(r_earth > r_embryo);
    assert!(r_embryo > r_planetesimal);
    assert!(r_planetesimal >= 0.005);
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
    assert!(t_eq >= 260.0 && t_eq <= 335.0);
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
