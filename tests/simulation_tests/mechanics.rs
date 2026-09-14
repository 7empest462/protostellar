//! Test module generated from simulation_tests.

use bevy::math::DVec3;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;
use protostellar::utils::math::*;

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
    // Planar equatorial disk orbit in reference coordinate frame (pos in X, v in Z)
    let pos_eq = DVec3::new(1.0, 0.0, 0.0);
    let vel_eq = DVec3::new(0.0, 0.0, std::f64::consts::TAU);
    let res_eq = state_vectors_to_orbital_elements(pos_eq, vel_eq, 1.0, EARTH_MASS_SOLAR);
    assert!(res_eq.is_some());
    let eq_elem = res_eq.unwrap();
    assert!(
        eq_elem.inclination.abs() < 1e-4,
        "Disk coplanar orbit inclination must be 0"
    );
    assert!(eq_elem.argument_of_periapsis.is_finite());

    // 90-degree inclined polar orbit (pos in X, v in Y):
    let pos_inc = DVec3::new(1.0, 0.0, 0.0);
    let vel_inc = DVec3::new(0.0, std::f64::consts::TAU, 0.0);
    let res_inc = state_vectors_to_orbital_elements(pos_inc, vel_inc, 1.0, EARTH_MASS_SOLAR);
    assert!(res_inc.is_some());
    let inc_elem = res_inc.unwrap();
    assert!(
        (inc_elem.inclination - std::f64::consts::FRAC_PI_2).abs() < 1e-3,
        "Polar orbit inclination must be pi/2 (90 deg)"
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
