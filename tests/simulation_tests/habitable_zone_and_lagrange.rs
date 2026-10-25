//! Integration and verification tests for Circumstellar Habitable Zones and Lagrange Points.

use bevy::prelude::*;
use protostellar::rendering::effects::{
    compute_cr3bp_lagrange_points, compute_effective_luminosity,
};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::{HabitableZoneConfig, LagrangeOverlayConfig};
use protostellar::utils::constants::{EARTH_MASS_SOLAR, SOLAR_RADIUS_AU};

#[test]
fn test_habitable_zone_luminosity_scaling() {
    // 1. Solar-analog star (L = 1.0)
    let lum_solar = compute_effective_luminosity(
        1.0,
        None,
        Some(&Temperature(5778.0)),
        Some(&Radius(SOLAR_RADIUS_AU)),
    );
    assert!(
        (lum_solar - 1.0).abs() < 0.05,
        "Expected solar luminosity ~ 1.0, got {lum_solar}"
    );

    let scale_solar = lum_solar.sqrt();
    let r_inner_cons = 0.95 * scale_solar;
    let r_outer_cons = 1.67 * scale_solar;
    let r_snow = 2.70 * scale_solar;

    assert!((r_inner_cons - 0.95).abs() < 0.05);
    assert!((r_outer_cons - 1.67).abs() < 0.05);
    assert!((r_snow - 2.70).abs() < 0.08);

    // 2. High-luminosity star (L = 4.0, sqrt(L) = 2.0)
    let explicit_lum = Luminosity(4.0);
    let lum_4 = compute_effective_luminosity(1.5, Some(&explicit_lum), None, None);
    assert_eq!(lum_4, 4.0);

    let scale_4 = lum_4.sqrt();
    assert_eq!(scale_4, 2.0);
    assert_eq!(0.95 * scale_4, 1.90);
    assert_eq!(1.67 * scale_4, 3.34);
    assert_eq!(2.70 * scale_4, 5.40);
}

#[test]
fn test_lagrange_points_equilateral_and_collinear_geometry() {
    let prim_pos = Vec3::ZERO;
    let prim_vel = Vec3::ZERO;
    let prim_mass = 1.0; // 1 Solar mass

    let sec_pos = Vec3::new(1.0, 0.0, 0.0); // 1 AU along X
    let sec_vel = Vec3::new(0.0, 0.0, 6.283_185); // Circular velocity in X-Z plane
    let sec_mass = EARTH_MASS_SOLAR; // 3.003e-6 M_sun

    let coords =
        compute_cr3bp_lagrange_points(prim_pos, prim_vel, prim_mass, sec_pos, sec_vel, sec_mass)
            .expect("Lagrange computation should succeed for Earth-Sun system");

    // Hill radius: r_H = 1.0 * (3.003e-6 / 3)^(1/3) ~ 0.0100 AU
    let expected_rh = 1.0 * ((sec_mass / 3.0).cbrt() as f32);
    assert!((coords.hill_radius - expected_rh).abs() < 1e-5);
    assert!((coords.hill_radius - 0.0100).abs() < 0.001);

    // L1: between Sun and Earth at 1.0 - r_H
    let l1_dist_from_sun = coords.l1.length();
    assert!((l1_dist_from_sun - (1.0 - coords.hill_radius)).abs() < 1e-4);

    // L2: behind Earth at 1.0 + r_H
    let l2_dist_from_sun = coords.l2.length();
    assert!((l2_dist_from_sun - (1.0 + coords.hill_radius)).abs() < 1e-4);

    // L3: opposite side of Sun (~ -1.0 AU)
    assert!(coords.l3.x < -0.99 && coords.l3.x > -1.01);
    assert!(coords.l3.z.abs() < 1e-4);

    // L4 and L5: equilateral triangles with primary and secondary!
    // Distance from Primary (Sun) to L4 must be 1.0 AU
    let dist_sun_to_l4 = coords.l4.distance(prim_pos);
    assert!(
        (dist_sun_to_l4 - 1.0).abs() < 1e-4,
        "Sun to L4 must be 1.0 AU, got {dist_sun_to_l4}"
    );

    // Distance from Secondary (Earth) to L4 must be 1.0 AU
    let dist_earth_to_l4 = coords.l4.distance(sec_pos);
    assert!(
        (dist_earth_to_l4 - 1.0).abs() < 1e-4,
        "Earth to L4 must be 1.0 AU, got {dist_earth_to_l4}"
    );

    // Distance from Primary (Sun) to L5 must be 1.0 AU
    let dist_sun_to_l5 = coords.l5.distance(prim_pos);
    assert!(
        (dist_sun_to_l5 - 1.0).abs() < 1e-4,
        "Sun to L5 must be 1.0 AU, got {dist_sun_to_l5}"
    );

    // Distance from Secondary (Earth) to L5 must be 1.0 AU
    let dist_earth_to_l5 = coords.l5.distance(sec_pos);
    assert!(
        (dist_earth_to_l5 - 1.0).abs() < 1e-4,
        "Earth to L5 must be 1.0 AU, got {dist_earth_to_l5}"
    );
}

#[test]
fn test_default_lagrange_and_habitable_zone_configs() {
    let hz = HabitableZoneConfig::default();
    assert!(hz.is_enabled, "Habitable zone should be enabled by default");

    let lagrange = LagrangeOverlayConfig::default();
    assert!(
        !lagrange.show_lagrange_points,
        "Lagrange points should default to toggleable OFF"
    );
    assert!(
        lagrange.show_hill_sphere,
        "Hill sphere wireframe should default to true"
    );
}
