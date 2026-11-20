//! Integration tests for SIMD N-body gravity, disk spawning, binary dynamics,
//! satellite inclination, and Type I migration torque.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::utils::constants::*;

#[test]
fn test_simd_f64x4_nbody_gravity_matches_analytical_precision() {
    use protostellar::simulation::physics::forces::compute_single_body_acc;
    use protostellar::simulation::physics::integrator::PhysicsBodyEntry;

    let config = SimulationConfig {
        enable_gas_drag: false,
        ..Default::default()
    };
    let (softening_sq, star_mass) = (1e-6, 1.0);
    let massive_data = vec![
        (DVec3::ZERO, DVec3::ZERO, star_mass, SOLAR_RADIUS_AU, 0usize),
        (
            DVec3::new(1.0, 0.02, 0.0),
            DVec3::new(0.0, 0.0, std::f64::consts::TAU),
            EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU,
            1,
        ),
        (
            DVec3::new(-1.5, -0.01, 0.4),
            DVec3::new(-1.2, 0.0, -4.9),
            0.8 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU,
            2,
        ),
        (
            DVec3::new(0.3, 0.04, -5.2),
            DVec3::new(2.7, 0.0, 0.15),
            JUPITER_MASS_SOLAR,
            EARTH_RADIUS_AU * 11.2,
            3,
        ),
        (
            DVec3::new(9.5, -0.1, 1.2),
            DVec3::new(-0.25, 0.0, 2.0),
            0.3 * JUPITER_MASS_SOLAR,
            EARTH_RADIUS_AU * 9.4,
            4,
        ),
        (
            DVec3::new(-19.2, 0.2, -3.5),
            DVec3::new(0.2, 0.0, -1.4),
            14.5 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 4.0,
            5,
        ),
        (
            DVec3::new(4.0, -0.05, 29.8),
            DVec3::new(-1.1, 0.0, 0.15),
            17.1 * EARTH_MASS_SOLAR,
            EARTH_RADIUS_AU * 3.9,
            6,
        ),
    ];
    let test_body = PhysicsBodyEntry {
        entity: Entity::from_bits(101),
        mass: EARTH_MASS_SOLAR,
        pos: massive_data[1].0,
        vel: massive_data[1].1,
        acc: DVec3::ZERO,
        radius: EARTH_RADIUS_AU,
        body_type: BodyType::TerrestrialPlanet,
        satellite: None,
        is_central_star: false,
        role_flags: 0,
    };
    let simd_acc = compute_single_body_acc(
        1,
        &test_body,
        &config,
        star_mass,
        Some(0),
        softening_sq,
        &massive_data,
        None,
        false,
        false,
        (DVec3::ZERO, 0.0),
        0.0,
        None,
    );
    let mut expected = DVec3::ZERO;
    for &(m_pos, _, m_mass, _, _) in &massive_data[2..] {
        let r_vec = test_body.pos - m_pos;
        let d_sq = r_vec.length_squared() + softening_sq;
        let m_sq = m_pos.length_squared() + softening_sq;
        expected -= (G_ASTRO * m_mass / (d_sq * d_sq.sqrt())) * r_vec
            + (G_ASTRO * m_mass / (m_sq * m_sq.sqrt())) * m_pos;
    }
    assert!((simd_acc - expected).length() / expected.length() < 1e-13);
}

#[test]
fn test_spawn_protoplanetary_disk_and_compact_massive_sampling() {
    use protostellar::simulation::disk::{sample_disk_radius, spawn_protoplanetary_disk};
    use protostellar::simulation::resources::{DiskParameters, SimulationConfig};

    let mut rng = fastrand::Rng::with_seed(1337);
    let compact_params = DiskParameters {
        central_star_mass: 0.09,
        inner_radius_au: 0.015,
        outer_radius_au: 1.0,
        ..default()
    };
    for _ in 0..500 {
        let (r, comp) = sample_disk_radius(&mut rng, &compact_params);
        assert!(
            (0.014..=1.01).contains(&r),
            "Compact disk radius out of range: {r}"
        );
        let sum = comp.metal_frac
            + comp.silicate_frac
            + comp.ice_frac
            + comp.organics_frac
            + comp.gas_frac;
        assert!((sum - 1.0).abs() < 1e-4);
    }

    let massive_params = DiskParameters {
        central_star_mass: 30.0,
        inner_radius_au: 2.0,
        outer_radius_au: 250.0,
        ..default()
    };
    for _ in 0..500 {
        let (r, _) = sample_disk_radius(&mut rng, &massive_params);
        assert!(
            (1.9..=251.0).contains(&r),
            "Massive disk radius out of range: {r}"
        );
    }

    let mut app = App::new();
    let default_params = DiskParameters::default();
    let cfg = SimulationConfig::default();
    let star_ent = {
        let mut commands = app.world_mut().commands();

        spawn_protoplanetary_disk(&mut commands, &default_params, &cfg)
    };
    app.world_mut().flush();

    assert!(app.world().get::<CentralStar>(star_ent).is_some());
    let mut saturn_rings = 0;
    let mut storms = 0;
    let mut q = app.world_mut().query::<(
        &CelestialBody,
        &SimPosition,
        &SimVelocity,
        Option<&PlanetaryRingSystem>,
        Option<&AtmosphericStormState>,
    )>();
    for (b, pos, vel, opt_rings, opt_storm) in q.iter(app.world()) {
        assert!(
            pos.0.is_finite() && vel.0.is_finite(),
            "Non-finite seed {}",
            b.name
        );
        if b.name.contains("Saturn") && opt_rings.is_some() {
            saturn_rings += 1;
        }
        if opt_storm.is_some() {
            storms += 1;
        }
    }
    assert_eq!(
        saturn_rings, 1,
        "Proto-Saturn must spawn with PlanetaryRingSystem"
    );
    assert!(
        storms >= 3,
        "Gas/Ice giants must attach AtmosphericStormState"
    );
}

#[test]
fn test_simd_zero_softening_self_and_star_origin_no_nan() {
    use protostellar::simulation::physics::*;

    let config = SimulationConfig {
        gas_density_scale: 0.0,
        ..Default::default()
    };
    let massive_data = vec![
        (DVec3::ZERO, DVec3::ZERO, 1.0, 0.004, 0),
        (DVec3::new(1.0, 0.0, 0.0), DVec3::ZERO, 1.0, 0.001, 1),
        (DVec3::new(-2.0, 0.5, 0.0), DVec3::ZERO, 0.5, 0.001, 2),
        (DVec3::new(0.0, 0.0, 3.0), DVec3::ZERO, 0.2, 0.001, 3),
    ];
    let body = PhysicsBodyEntry {
        entity: Entity::from_bits(1),
        mass: 1.0,
        pos: massive_data[1].0,
        vel: DVec3::ZERO,
        acc: DVec3::ZERO,
        radius: 0.001,
        body_type: BodyType::MainSequenceStar,
        satellite: None,
        is_central_star: false,
        role_flags: 0,
    };

    let acc = compute_single_body_acc(
        1,
        &body,
        &config,
        1.0,
        Some(0),
        0.0, // explicit zero softening exercises SIMD 0/0 NaN guard
        &massive_data,
        None,
        false,
        false,
        (DVec3::ZERO, 0.0),
        0.0,
        None,
    );
    assert!(
        acc.is_finite(),
        "SIMD zero-softening pull must not produce NaN: {acc:?}"
    );
}

#[test]
fn test_equal_mass_binary_uses_reduced_mass_pull_without_cutoff() {
    use protostellar::simulation::physics::*;

    let config = SimulationConfig {
        gas_density_scale: 0.0,
        ..Default::default()
    };
    let star_pos = DVec3::ZERO;
    let companion_pos = DVec3::new(0.1, 0.0, 0.0);
    let m1 = 1.44;
    let m2 = 1.38; // q = 0.958 >= 0.5
    let massive_data = vec![
        (star_pos, DVec3::ZERO, m1, 1e-6, 0),
        (companion_pos, DVec3::ZERO, m2, 1e-6, 1),
    ];
    let body = PhysicsBodyEntry {
        entity: Entity::from_bits(2),
        mass: m2,
        pos: companion_pos,
        // Unbound radial velocity ensures direct Cartesian acceleration includes `star_acc`
        vel: DVec3::new(100.0, 0.0, 0.0),
        acc: DVec3::ZERO,
        radius: 1e-6,
        body_type: BodyType::NeutronStar,
        satellite: None,
        is_central_star: false,
        role_flags: 0,
    };
    let acc = compute_single_body_acc(
        1,
        &body,
        &config,
        m1,
        Some(0),
        0.0,
        &massive_data,
        None,
        false,
        false,
        (DVec3::ZERO, 0.0),
        0.0,
        None,
    );
    let r = companion_pos.x;
    let expected_mag = G_ASTRO * (m1 + m2) / (r * r);
    let rel_err = (acc.length() - expected_mag).abs() / expected_mag;
    assert!(
        rel_err < 1e-9,
        "Relative two-body pull for q>=0.5 must use (m1+m2): got {}, expected {expected_mag}",
        acc.length()
    );
}

#[test]
fn test_inclined_satellite_preserves_inclination_across_full_orbit() {
    use protostellar::simulation::physics::*;

    let parent = PhysicsBodyEntry {
        entity: Entity::from_bits(1),
        mass: EARTH_MASS_SOLAR,
        pos: DVec3::new(1.0, 0.0, 0.0),
        vel: DVec3::new(0.0, 0.0, 2.0 * std::f64::consts::PI),
        acc: DVec3::ZERO,
        radius: EARTH_RADIUS_AU,
        body_type: BodyType::TerrestrialPlanet,
        satellite: None,
        is_central_star: false,
        role_flags: 0,
    };
    let a_moon = 0.012;
    let p_moon = 0.0748;
    let v_moon = 2.0 * std::f64::consts::PI * a_moon / p_moon;
    let inc = 15.0_f64.to_radians();
    let moon = PhysicsBodyEntry {
        entity: Entity::from_bits(2),
        mass: 0.0123 * EARTH_MASS_SOLAR,
        pos: parent.pos + DVec3::new(a_moon * inc.cos(), a_moon * inc.sin(), 0.0),
        vel: parent.vel + DVec3::new(0.0, 0.0, v_moon),
        acc: DVec3::ZERO,
        radius: EARTH_RADIUS_AU * 0.272,
        body_type: BodyType::Moon,
        satellite: Some(SatelliteOf {
            parent: parent.entity,
            semi_major_axis_au: a_moon,
            orbital_period_years: p_moon,
            true_anomaly: 0.0,
        }),
        is_central_star: false,
        role_flags: 0,
    };
    let mut entries = vec![parent, moon];
    let mut buf = Vec::new();
    let init_h_hat = (entries[1].pos - entries[0].pos)
        .cross(entries[1].vel - entries[0].vel)
        .normalize();

    // Advance through 2 full lunar orbits (which cross y=0 nodes 4 times)
    for _ in 0..80 {
        advance_symplectic_leapfrog_drift_with_companion(
            &mut entries,
            p_moon * 2.0 / 80.0,
            DVec3::ZERO,
            1.0,
            &mut buf,
            false,
            false,
            None,
        );
    }
    let end_h_hat = (entries[1].pos - entries[0].pos)
        .cross(entries[1].vel - entries[0].vel)
        .normalize();
    assert!(
        (init_h_hat - end_h_hat).length() < 1e-9,
        "Inclined moon orbital plane normal must not decay across nodes: init={init_h_hat:?}, end={end_h_hat:?}"
    );
    // Simulate second parent half-kick and ensure moon velocity syncs with parent delta
    entries[0].vel.x += 0.5;
    sync_satellite_velocities_to_parents(&mut entries, &mut buf);
    let synced_v_rel = (entries[1].vel - entries[0].vel).length();
    assert!(
        (synced_v_rel - v_moon).abs() < 1e-9,
        "Moon relative orbital speed must stay locked to parent post-kick velocity"
    );
}

#[test]
fn test_type_i_torque_is_strictly_azimuthal_and_scales_with_disk_mass() {
    use protostellar::simulation::disk_migration::apply_type_i_torque_acc;

    let mut params = DiskParameters {
        disk_mass: 0.05,
        inner_radius_au: 0.2,
        outer_radius_au: 30.0,
        density_power_law: 1.5,
        ..default()
    };
    let pos = DVec3::new(2.0, 0.0, 0.0);
    // Strongly eccentric/radial velocity must still get strictly tangential/azimuthal drag
    let vel = DVec3::new(1.5, 0.0, 3.5);
    let mut acc1 = DVec3::ZERO;
    apply_type_i_torque_acc(
        pos,
        vel,
        10.0 * EARTH_MASS_SOLAR,
        BodyType::SuperEarth,
        false,
        false,
        1.0,
        1.0,
        &params,
        &mut acc1,
    );
    assert!(
        acc1.x.abs() < 1e-18,
        "Radial component must be zero: {acc1:?}"
    );
    assert!(
        acc1.z < 0.0,
        "Prograde azimuthal drag must oppose +phi motion"
    );

    params.disk_mass = 0.0;
    let mut acc_empty = DVec3::ZERO;
    apply_type_i_torque_acc(
        pos,
        vel,
        10.0 * EARTH_MASS_SOLAR,
        BodyType::SuperEarth,
        false,
        false,
        1.0,
        1.0,
        &params,
        &mut acc_empty,
    );
    assert_eq!(acc_empty, DVec3::ZERO);
}
