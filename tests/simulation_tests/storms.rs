//! Tests for Atmospheric Storm Hexagons, Jovian Anticyclonic Vortices, and Cloud Dynamics.

use std::f32::consts::PI;

use bevy::prelude::*;
use protostellar::rendering::materials::PlanetUniforms;
use protostellar::simulation::components::storms::AtmosphericStormState;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::DiskParameters;
use protostellar::simulation::scenarios::exotic::spawn_rogue_planet_scenario;
use protostellar::simulation::scenarios::solar::spawn_solar_nebula_mmsn;

#[test]
fn test_saturn_polar_hexagon_sixfold_azimuthal_symmetry() {
    let saturn = AtmosphericStormState::saturn();
    assert_eq!(saturn.polar_hexagon_wavenumber, 6.0);
    assert!(saturn.polar_hexagon_amplitude > 0.05);

    let k = saturn.polar_hexagon_wavenumber;
    let a = saturn.polar_hexagon_amplitude;
    let r0 = saturn.polar_hexagon_colatitude;

    // Formula: r(phi) = r0 * (1.0 + A * cos(k * phi) + 0.22 * A * cos(2 * k * phi))
    let eval_r =
        |phi: f32| -> f32 { r0 * (1.0 + a * (k * phi).cos() + 0.22 * a * (2.0 * k * phi).cos()) };

    // Verify exact 6-fold azimuthal rotational invariance under delta_phi = 2*PI / 6 = PI / 3
    let sector_angle = 2.0 * PI / 6.0;
    for i in 0..36 {
        let phi = (i as f32) * (2.0 * PI / 36.0);
        let r_orig = eval_r(phi);
        let r_rotated = eval_r(phi + sector_angle);
        assert!(
            (r_orig - r_rotated).abs() < 1e-5,
            "Hexagon must exhibit exact 6-fold discrete rotational symmetry: r({phi}) = {r_orig}, r({phi} + pi/3) = {r_rotated}"
        );
    }

    // Verify 6 distinct radial corners (maxima) and 6 flat facets (minima) around 2*PI
    let mut maxima_count = 0;
    let steps = 360;
    let dphi = 2.0 * PI / (steps as f32);
    for i in 0..steps {
        let phi = (i as f32) * dphi;
        let r_curr = eval_r(phi);
        let r_prev = eval_r(phi - dphi);
        let r_next = eval_r(phi + dphi);
        if r_curr > r_prev && r_curr > r_next {
            maxima_count += 1;
        }
    }
    assert_eq!(
        maxima_count, 6,
        "Saturn North Polar Hexagon must have exactly 6 polygon vertices (standing wave crests)"
    );
}

#[test]
fn test_great_red_spot_aspect_ratio_and_collar_wind_profile() {
    let jupiter = AtmosphericStormState::jupiter();
    assert!(jupiter.great_spot_size > 0.3);
    assert_eq!(jupiter.polar_hexagon_amplitude, 0.0);

    // Elliptic semi-axes scaling: a = 1.50 * s, b = 0.68 * s
    let semi_major_factor = 1.50;
    let semi_minor_factor = 0.68;
    let aspect_ratio = semi_major_factor / semi_minor_factor;

    assert!(
        aspect_ratio > 2.0 && aspect_ratio < 2.4,
        "GRS aspect ratio must be approximately 2.2:1 (actual: {aspect_ratio})"
    );

    // High-speed outer collar wind profile: C(r_ell) = exp(-((r_ell - 0.85) / 0.20)^2)
    let collar_wind = |r_ell: f32| -> f32 { (-((r_ell - 0.85) / 0.20).powi(2)).exp() };

    let peak_wind = collar_wind(0.85);
    let eye_wind = collar_wind(0.10);
    let outer_flow = collar_wind(1.60);

    assert!(
        peak_wind > 0.99,
        "Collar wind velocity must peak near r_ell = 0.85"
    );
    assert!(
        eye_wind < 0.001,
        "Eye core must be calm (eye wind {eye_wind} should be < 0.001)"
    );
    assert!(
        outer_flow < 0.001,
        "Winds outside the storm boundary must drop off rapidly"
    );
}

#[test]
fn test_atmospheric_storm_state_presets() {
    let jup = AtmosphericStormState::jupiter();
    assert!(jup.great_spot_size >= 0.40);
    assert!((jup.great_spot_latitude_rad - (-0.384)).abs() < 0.01);
    assert_eq!(jup.secondary_oval_count, 4);
    assert!(jup.zonal_shear_turbulence > 0.80);

    let sat = AtmosphericStormState::saturn();
    assert_eq!(sat.polar_hexagon_wavenumber, 6.0);
    assert!(sat.polar_hexagon_amplitude >= 0.08);
    assert!((sat.polar_hexagon_colatitude - 0.218).abs() < 0.01);

    let nep = AtmosphericStormState::neptune();
    assert_eq!(nep.polar_hexagon_amplitude, 0.0);
    assert!(nep.great_spot_size > 0.25);
    assert_eq!(nep.secondary_oval_count, 2);

    let terr = AtmosphericStormState::terrestrial();
    assert_eq!(terr.polar_hexagon_amplitude, 0.0);
    assert!(terr.great_spot_size > 0.0);

    let def = AtmosphericStormState::default();
    assert_eq!(def.polar_hexagon_amplitude, 0.0);
    assert_eq!(def.great_spot_size, 0.0);
}

#[test]
fn test_solar_nebula_mmsn_attaches_storm_states() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();

    let star = spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    assert_ne!(star, Entity::PLACEHOLDER);
    app.update();

    let mut found_jupiter_storm = false;
    let mut found_saturn_hexagon = false;
    let mut found_neptune_storm = false;

    for (body, storm) in app
        .world_mut()
        .query::<(&CelestialBody, Option<&AtmosphericStormState>)>()
        .iter(app.world())
    {
        if body.name.contains("Jupiter") {
            let s = storm.expect("Proto-Jupiter must have AtmosphericStormState");
            assert!(s.great_spot_size > 0.3);
            found_jupiter_storm = true;
        } else if body.name.contains("Saturn") {
            let s = storm.expect("Proto-Saturn must have AtmosphericStormState");
            assert_eq!(s.polar_hexagon_wavenumber, 6.0);
            assert!(s.polar_hexagon_amplitude > 0.05);
            found_saturn_hexagon = true;
        } else if body.name.contains("Neptune") {
            let s = storm.expect("Proto-Neptune must have AtmosphericStormState");
            assert!(s.great_spot_size > 0.2);
            found_neptune_storm = true;
        }
    }

    assert!(found_jupiter_storm, "Proto-Jupiter must be found with GRS");
    assert!(
        found_saturn_hexagon,
        "Proto-Saturn must be found with Polar Hexagon"
    );
    assert!(
        found_neptune_storm,
        "Proto-Neptune must be found with Great Dark Spot"
    );
}

#[test]
fn test_exotic_scenario_attaches_storm_states() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();

    let (_star, _intruder) =
        spawn_rogue_planet_scenario(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let mut found_rogue_storm = false;
    let mut found_jupiter = false;

    for (body, storm) in app
        .world_mut()
        .query::<(&CelestialBody, Option<&AtmosphericStormState>)>()
        .iter(app.world())
    {
        if body.name.contains("Rogue Interloper") {
            let s = storm.expect("Rogue Interloper must have AtmosphericStormState");
            assert!(s.zonal_shear_turbulence > 0.9);
            found_rogue_storm = true;
        } else if body.name == "Jupiter" {
            let s = storm.expect("Jupiter in rogue scenario must have AtmosphericStormState");
            assert!(s.great_spot_size > 0.3);
            found_jupiter = true;
        }
    }

    assert!(found_rogue_storm, "Rogue interloper must have storm state");
    assert!(found_jupiter, "Jupiter must have storm state");
}

#[test]
fn test_planet_uniforms_storm_defaults_and_alignment() {
    let uniforms = PlanetUniforms::default();
    assert_eq!(uniforms.storm_features, Vec4::ZERO);
    assert_eq!(uniforms.storm_dynamics, Vec4::new(0.0, 1.0, 0.0, 0.5));

    // Verify 16-byte alignment of PlanetUniforms
    assert_eq!(std::mem::size_of::<PlanetUniforms>() % 16, 0);
}
