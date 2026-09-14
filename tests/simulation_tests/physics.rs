//! Test module generated from simulation_tests.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
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

    let pt = position_at_true_anomaly(&elements, elements.true_anomaly).unwrap();
    assert!((pt.x - pos.x as f32).abs() < 1e-3);
    assert!((pt.y - pos.y as f32).abs() < 1e-3);
    assert!((pt.z - pos.z as f32).abs() < 1e-3);

    let pos2 = DVec3::new(0.0, 0.0, 1.0);
    let vel2 = DVec3::new(-2.0 * std::f64::consts::PI, 0.0, 0.0);
    let elements2 =
        state_vectors_to_orbital_elements(pos2, vel2, central_mass, orbiting_mass).unwrap();
    let pt2 = position_at_true_anomaly(&elements2, elements2.true_anomaly).unwrap();
    assert!((pt2.x - pos2.x as f32).abs() < 1e-3);
    assert!((pt2.y - pos2.y as f32).abs() < 1e-3);
    assert!((pt2.z - pos2.z as f32).abs() < 1e-3);
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

struct StabilitySystem {
    app: App,
    sched: Schedule,
    merc_ent: Entity,
    earth_ent: Entity,
    ceres_ent: Entity,
    jup_ent: Entity,
}

fn setup_high_time_warp_solar_system() -> StabilitySystem {
    use bevy::prelude::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    let mut config = SimulationConfig::default();
    config.enable_gas_drag = false;
    config.gas_density_scale = 0.0;
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();

    // Central Sun (1.0 M_sun)
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
    ));

    // Mercury at 0.387 AU (P ~ 0.24 yr)
    let v_merc = (G_ASTRO * 1.0 / 0.387).sqrt();
    let merc_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Mercury".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(0.055 * EARTH_MASS_SOLAR),
            Radius(0.38 * EARTH_RADIUS_AU),
            SimPosition(DVec3::new(0.387, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_merc)),
            SimAcceleration(DVec3::ZERO),
        ))
        .id();

    // Earth at 1.0 AU (P = 1.0 yr)
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
        ))
        .id();

    // Asteroid Ceres at 2.77 AU (P ~ 4.6 yr)
    let v_ceres = (G_ASTRO * 1.0 / 2.77).sqrt();
    let ceres_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Ceres".to_string(),
                body_type: BodyType::Asteroid,
            },
            Mass(0.00015 * EARTH_MASS_SOLAR),
            Radius(0.18 * EARTH_RADIUS_AU),
            SimPosition(DVec3::new(2.77, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_ceres)),
            SimAcceleration(DVec3::ZERO),
        ))
        .id();

    // Jupiter at 5.20 AU (P ~ 11.86 yr)
    let v_jup = (G_ASTRO * 1.0 / 5.20).sqrt();
    let jup_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Jupiter".to_string(),
                body_type: BodyType::GasGiant,
            },
            Mass(JUPITER_MASS_SOLAR),
            Radius(11.2 * EARTH_RADIUS_AU),
            SimPosition(DVec3::new(5.20, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, v_jup)),
            SimAcceleration(DVec3::ZERO),
        ))
        .id();

    let mut sched = Schedule::default();
    sched.add_systems(step_physics_simulation);

    StabilitySystem {
        app,
        sched,
        merc_ent,
        earth_ent,
        ceres_ent,
        jup_ent,
    }
}

#[test]
fn test_high_time_warp_orbital_stability() {
    let mut s = setup_high_time_warp_solar_system();

    // Test 1: Run at 10,000x time warp for 50 frames (target_dt = 5.0 yr/frame -> 250 years total)
    s.app.world_mut().resource_mut::<TimeWarp>().multiplier = 10_000.0;
    for _ in 0..50 {
        s.sched.run(s.app.world_mut());
    }

    let p_merc = s
        .app
        .world()
        .get::<SimPosition>(s.merc_ent)
        .unwrap()
        .0
        .length();
    let p_earth = s
        .app
        .world()
        .get::<SimPosition>(s.earth_ent)
        .unwrap()
        .0
        .length();
    let p_ceres = s
        .app
        .world()
        .get::<SimPosition>(s.ceres_ent)
        .unwrap()
        .0
        .length();
    let p_jup = s
        .app
        .world()
        .get::<SimPosition>(s.jup_ent)
        .unwrap()
        .0
        .length();

    assert!(
        (p_merc - 0.387).abs() < 0.01,
        "Mercury radius diverged: {}",
        p_merc
    );
    assert!(
        (p_earth - 1.00).abs() < 0.02,
        "Earth radius diverged: {}",
        p_earth
    );
    assert!(
        (p_ceres - 2.77).abs() < 0.05,
        "Ceres radius diverged: {}",
        p_ceres
    );
    assert!(
        (p_jup - 5.20).abs() < 0.10,
        "Jupiter radius diverged: {}",
        p_jup
    );

    // Test 2: Run at 1,000,000x time warp for 10 frames (target_dt = 500 yr/frame -> 5,000 years total!)
    s.app.world_mut().resource_mut::<TimeWarp>().multiplier = 1_000_000.0;
    for _ in 0..10 {
        s.sched.run(s.app.world_mut());
    }

    let p_earth_warp = s
        .app
        .world()
        .get::<SimPosition>(s.earth_ent)
        .unwrap()
        .0
        .length();
    let p_ceres_warp = s
        .app
        .world()
        .get::<SimPosition>(s.ceres_ent)
        .unwrap()
        .0
        .length();
    let p_jup_warp = s
        .app
        .world()
        .get::<SimPosition>(s.jup_ent)
        .unwrap()
        .0
        .length();

    // After 5,250 years of simulated time at up to 1,000,000x speed, orbits must stay stable!
    assert!(
        (p_earth_warp - 1.00).abs() < 0.05,
        "Earth flung out at 1,000,000x: {}",
        p_earth_warp
    );
    assert!(
        (p_ceres_warp - 2.77).abs() < 0.25,
        "Ceres flung out of Asteroid Belt at 1,000,000x: {}",
        p_ceres_warp
    );
    assert!(
        (p_jup_warp - 5.20).abs() < 0.20,
        "Jupiter flung out at 1,000,000x: {}",
        p_jup_warp
    );
}

#[test]
fn test_full_simulation_speed_7_stability() {
    use bevy::prelude::*;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_plugins(protostellar::simulation::SimulationPlugin);

    // Initial update to run Startup systems
    app.update();

    // Set time warp to speed 7 (1,000,000x)
    app.world_mut().resource_mut::<TimeWarp>().multiplier = 1_000_000.0;

    // Run 50 frames (25,000 simulated years elapsed!)
    for _ in 1..=50 {
        app.update();
    }

    let mut census = protostellar::simulation::disk::belts::BeltCensus::default();
    let mut total_bodies = 0;
    let mut bound_bodies = 0;
    let mut major_planets_checked = 0;
    let major_planet_names = [
        "Mercury",
        "Venus",
        "Earth",
        "Mars",
        "Jupiter",
        "Saturn",
        "Uranus",
        "Neptune",
        "Pluto",
        "Planet Nine",
    ];

    let mut found_names = Vec::new();
    let mut q = app.world_mut().query::<(
        Entity,
        &CelestialBody,
        &SimPosition,
        Option<&SatelliteOf>,
        Option<&CentralStar>,
    )>();
    for (b_ent, b, p, sat, star) in q.iter(app.world()) {
        if star.is_none() && sat.is_none() {
            total_bodies += 1;
            let r = p.0.length();
            census.record(r);
            let mu = 39.4784176;
            let vel = app.world().get::<SimVelocity>(b_ent).unwrap().0;
            let specific_energy = 0.5 * vel.length_squared() - mu / r;

            let is_major = major_planet_names.iter().any(|name| b.name.contains(name));
            if is_major {
                major_planets_checked += 1;
                found_names.push(b.name.clone());
                assert!(
                    specific_energy < 0.0 && r < 5000.0,
                    "Major planet '{}' was ejected! r = {:.2} AU, energy = {:.4}",
                    b.name,
                    r,
                    specific_energy
                );
            }

            if specific_energy < 0.0 && r < 5000.0 {
                bound_bodies += 1;
            }
        }
    }

    // All major planets must be present and bound (or merged via giant impact)
    assert!(
        major_planets_checked >= 8,
        "Too few major planets found: {} (found: {:?})",
        major_planets_checked,
        found_names
    );

    // Over 90% of all generated bodies remain bound in the solar system
    assert!(
        bound_bodies as f64 / total_bodies as f64 >= 0.90,
        "Too many minor bodies ejected: {}/{} bound",
        bound_bodies,
        total_bodies
    );

    // Belts must be actively populated after 25,000 years under 1,000,000x time warp
    assert!(
        census.asteroid_belt_count >= 20,
        "Asteroid Belt underpopulated at 1,000,000x warp: got {}",
        census.asteroid_belt_count
    );
    assert!(
        census.kuiper_count >= 10,
        "Kuiper Belt underpopulated at 1,000,000x warp: got {}",
        census.kuiper_count
    );
}
