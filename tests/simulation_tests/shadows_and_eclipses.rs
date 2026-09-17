//! Integration tests for Feature 3.2: Real-time Planetary Eclipse & Ring Shadow Casting.

use bevy::prelude::*;
use protostellar::rendering::bodies::{
    compute_moon_eclipse_shadow, compute_planetary_ring_shadow, compute_ring_shadow_on_planet,
};
use protostellar::rendering::materials::{PlanetUniforms, RingUniforms};

#[test]
fn test_planetary_ring_shadow_dayside_and_nightside() {
    let planet_rad_norm = 0.35f32; // 1.0 / 2.85 ring ratio
    let star_dir = Vec3::new(0.0, 0.5, 0.866).normalize(); // Sun in +Z hemisphere (Ly = 0.5)

    // 1. Dayside fragment (Z > 0, towards star): ray towards star points away from planet center
    let dayside_ring_pt = Vec2::new(0.0, 0.65);
    let day_factor = compute_planetary_ring_shadow(dayside_ring_pt, star_dir, planet_rad_norm);
    assert!(
        (day_factor - 1.0).abs() < 1e-5,
        "Dayside ring fragment must be fully illuminated"
    );

    // 2. Nightside fragment directly behind planet (Z = -0.50, X = 0.0): inside shadow cylinder
    // d_closest = |Z| * Ly = 0.50 * 0.5 = 0.25 < 0.35
    let nightside_shadow_pt = Vec2::new(0.0, -0.50);
    let shadow_factor =
        compute_planetary_ring_shadow(nightside_shadow_pt, star_dir, planet_rad_norm);
    assert!(
        shadow_factor.abs() < 1e-5,
        "Nightside fragment behind planet core must be in total umbra"
    );

    // 3. Nightside fragment far to the side (X = 0.80, Z = -0.50): outside cylinder
    let nightside_outside_pt = Vec2::new(0.80, -0.50);
    let outside_factor =
        compute_planetary_ring_shadow(nightside_outside_pt, star_dir, planet_rad_norm);
    assert!(
        (outside_factor - 1.0).abs() < 1e-5,
        "Fragment outside shadow cylinder must be illuminated"
    );

    // 4. Penumbra edge transition at x = sqrt(0.35^2 - (0.50 * 0.5)^2) ~ 0.245, z = -0.50
    // At this point, d_closest = sqrt(0.245^2 + 0.25^2) = 0.35 = planet_rad_norm!
    let penumbra_pt = Vec2::new(0.245, -0.50);
    let penumbra_factor = compute_planetary_ring_shadow(penumbra_pt, star_dir, planet_rad_norm);
    assert!(
        penumbra_factor > 0.10 && penumbra_factor < 0.90,
        "Edge fragment must have a smooth penumbra value between 0 and 1, got {penumbra_factor}"
    );
}

#[test]
fn test_ring_shadow_on_planet_cassini_division_and_hemispheres() {
    let spin_axis = Vec3::Y; // Equator in XZ plane
                             // Star in northern celestial hemisphere at ~18 degrees inclination (Ly = 0.309, Lz = 0.951)
                             // Max shadow distance is 1 / 0.309 = 3.236, which fully spans past the outer ring (2.85).
    let star_dir = Vec3::new(0.0, 0.3090, 0.9511).normalize();

    let r_inner = 1.25f32;
    let r_outer = 2.85f32;
    let opt_depth = 0.85f32;

    // 1. Point in Northern hemisphere (same hemisphere as star):
    // Rays towards star travel away from the equatorial plane -> no ring shadow!
    let north_surf = Vec3::new(0.0, 0.707, 0.707).normalize();
    let north_shadow =
        compute_ring_shadow_on_planet(north_surf, star_dir, spin_axis, r_inner, r_outer, opt_depth);
    assert!(
        (north_shadow - 1.0).abs() < 1e-5,
        "Northern hemisphere facing star must receive no ring shadow"
    );

    // 2. Point in Southern hemisphere where ray intersects dense B-Ring:
    let mut b_ring_darkest = 1.0f32;
    let mut cassini_seen = false;

    // Sweep southern latitudes from -10 to -70 degrees:
    for deg in 10..75 {
        let lat = -(deg as f32).to_radians();
        let s = Vec3::new(0.0, lat.sin(), lat.cos());
        let sh = compute_ring_shadow_on_planet(s, star_dir, spin_axis, r_inner, r_outer, opt_depth);
        if sh < b_ring_darkest {
            b_ring_darkest = sh;
        }

        let t = -s.y / star_dir.y;
        if t > 0.0 {
            let p_int = s + t * star_dir;
            let r_int = p_int.length();
            let u = (r_int - r_inner) / (r_outer - r_inner);
            if (0.67..0.70).contains(&u) {
                assert!(
                    sh > 0.75,
                    "Cassini division gap at u={u:.3} must transmit bright sunlight, got shadow {sh}"
                );
                cassini_seen = true;
            }
        }
    }

    assert!(
        b_ring_darkest < 0.35,
        "Dense B-Ring shadow must heavily darken surface, minimum got {b_ring_darkest}"
    );
    assert!(
        cassini_seen,
        "Cassini division sunlight transmission stripe must be traversed"
    );
}

#[test]
fn test_moon_solar_eclipse_umbra_and_penumbra() {
    let star_dir = Vec3::new(0.0, 0.0, 1.0); // Star along +Z

    // Planet at origin with radius 1.0 (normalized).
    // Moon in orbit along +Z at distance 6.0 planet radii, with visual radius 0.25 planet radii:
    let moon_pos_norm = Vec3::new(0.0, 0.0, 6.0);
    let moon_rad_norm = 0.25f32;

    // 1. Subsolar point directly facing star and moon: N = (0, 0, 1)
    let subsolar_surf = Vec3::new(0.0, 0.0, 1.0);
    let center_illum =
        compute_moon_eclipse_shadow(subsolar_surf, star_dir, moon_pos_norm, moon_rad_norm);
    assert!(
        center_illum <= 0.05,
        "Subsolar point under transiting moon must be in total solar eclipse umbra, got {center_illum}"
    );

    // 2. Point far away from eclipse (e.g. 45 degrees away on surface):
    let off_axis_surf = Vec3::new(0.707, 0.0, 0.707).normalize();
    let off_axis_illum =
        compute_moon_eclipse_shadow(off_axis_surf, star_dir, moon_pos_norm, moon_rad_norm);
    assert!(
        (off_axis_illum - 1.0).abs() < 1e-5,
        "Surface point far from moon shadow cone must have full daylight"
    );

    // 3. Point in penumbral rim (d_perp ~ 0.24 planet radii):
    let theta = 0.24f32.asin();
    let penumbra_surf = Vec3::new(theta.sin(), 0.0, theta.cos());
    let penumbra_illum =
        compute_moon_eclipse_shadow(penumbra_surf, star_dir, moon_pos_norm, moon_rad_norm);
    assert!(
        penumbra_illum > 0.05 && penumbra_illum < 1.0,
        "Penumbral edge must have partial illumination, got {penumbra_illum}"
    );

    // 4. Moon on nightside (Z < 0): no eclipse on dayside!
    let nightside_moon = Vec3::new(0.0, 0.0, -6.0);
    let night_moon_illum =
        compute_moon_eclipse_shadow(subsolar_surf, star_dir, nightside_moon, moon_rad_norm);
    assert!(
        (night_moon_illum - 1.0).abs() < 1e-5,
        "Moon behind the planet must not cast eclipse shadow on dayside"
    );
}

#[test]
fn test_material_uniform_defaults_and_alignment() {
    let p_unif = PlanetUniforms::default();
    assert_eq!(p_unif.ring_shadow_params, Vec4::ZERO);
    assert_eq!(p_unif.eclipse_moons_pos[0], Vec4::ZERO);
    assert_eq!(p_unif.eclipse_moons_data[0], Vec4::ZERO);

    let r_unif = RingUniforms::default();
    assert!(r_unif.star_dir_local.w > 0.0);
    assert!(r_unif.shadow_params.x > 0.0);
}
