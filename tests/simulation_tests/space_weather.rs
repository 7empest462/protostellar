//! Integration tests for Space Weather, Coronal Mass Ejections (CMEs), and Planetary Auroral Ovals.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::space_weather::physics::*;
use protostellar::simulation::space_weather::types::*;
use protostellar::simulation::space_weather::*;
use protostellar::utils::constants::*;

#[test]
fn test_auroral_oval_geometry_scaling() {
    // 1. Earth baseline: standoff = 10.5 Rp
    let (colat_earth, width_earth) = calculate_auroral_oval_geometry(10.5);

    // Oval colatitude \theta_A = arcsin(sqrt(1 / 10.5)) ~ 17.9° (~0.31 rad)
    let lat_earth_deg: f32 = 90.0 - colat_earth.to_degrees();
    assert!(
        (lat_earth_deg - 72.0).abs() < 1.0,
        "Earth auroral oval latitude expected ~72°, got {lat_earth_deg}°"
    );
    assert!(
        width_earth > 0.03 && width_earth < 0.12,
        "Earth oval width out of expected range: {width_earth} rad"
    );

    // 2. CME impact compressed magnetosphere: standoff = 5.5 Rp
    let (colat_cme, width_cme) = calculate_auroral_oval_geometry(5.5);

    // Auroral oval expands equatorward (colatitude increases, latitude decreases)
    let lat_cme_deg: f32 = 90.0 - colat_cme.to_degrees();
    assert!(
        lat_cme_deg < lat_earth_deg,
        "CME auroral oval should expand equatorward (lower latitude): cme={lat_cme_deg}°, quiet={lat_earth_deg}°"
    );
    assert!(
        width_cme > width_earth,
        "CME oval should broaden: cme={width_cme}, quiet={width_earth}"
    );

    // 3. Jupiter-scale standoff (e.g. 50.0 Rp)
    let (colat_jup, width_jup) = calculate_auroral_oval_geometry(50.0);
    let lat_jup_deg: f32 = 90.0 - colat_jup.to_degrees();
    assert!(
        lat_jup_deg > 80.0,
        "Jupiter auroral ring is very tight polar oval (>80°), got {lat_jup_deg}°"
    );
    assert!(colat_jup < colat_earth);
    assert!(width_jup <= width_earth);
}

#[test]
fn test_geomagnetic_kp_derivation() {
    // Nominal Earth: standoff 10.5, multiplier 1.0, B = 0.5 G
    let (kp_quiet, level_quiet) = calculate_geomagnetic_kp(10.5, 1.0, 0.5);
    assert!(kp_quiet < 2.5);
    assert_eq!(level_quiet, GeomagneticStormLevel::Quiet);

    // Moderate CME storm: standoff 6.5, multiplier 8.0, B = 0.5 G
    let (kp_mod, level_mod) = calculate_geomagnetic_kp(6.5, 8.0, 0.5);
    assert!(kp_mod >= 5.0 && kp_mod <= 7.0);
    assert!(
        level_mod == GeomagneticStormLevel::ModerateStorm
            || level_mod == GeomagneticStormLevel::StrongStorm
    );

    // Extreme Carrington Event compression: standoff 3.5, multiplier 30.0, B = 0.5 G
    let (kp_extreme, level_extreme) = calculate_geomagnetic_kp(3.5, 30.0, 0.5);
    assert!(kp_extreme >= 8.5);
    assert_eq!(level_extreme, GeomagneticStormLevel::ExtremeStorm);

    // Unmagnetized world: B = 0.0 G -> Kp 0.0
    let (kp_zero, level_zero) = calculate_geomagnetic_kp(1.0, 10.0, 0.0);
    assert_eq!(kp_zero, 0.0);
    assert_eq!(level_zero, GeomagneticStormLevel::Quiet);
}

#[test]
fn test_auroral_emission_colors() {
    // Terrestrial Earth-like composition
    let earth_comp = Composition {
        silicate_frac: 0.70,
        metal_frac: 0.30,
        ice_frac: 0.0,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let (core_earth, border_earth, curtain_earth) =
        calculate_auroral_emission_colors(&earth_comp, false);

    // Terrestrial: Emerald green forbidden oxygen line dominant in core
    assert!(
        core_earth.y > core_earth.x && core_earth.y > core_earth.z,
        "Primary emission in terrestrial atmosphere should be oxygen green (557.7nm), got {core_earth:?}"
    );
    // Lower border has nitrogen violet
    assert!(
        border_earth.z > border_earth.y,
        "Lower border should contain N2+ violet component, got {border_earth:?}"
    );
    // Upper curtain should have red 630nm component
    assert!(
        curtain_earth.x > 0.8,
        "High altitude atomic oxygen should be red, got {curtain_earth:?}"
    );

    // Jovian / Gas giant atmosphere
    let gas_comp = Composition {
        silicate_frac: 0.0,
        metal_frac: 0.0,
        ice_frac: 0.05,
        organics_frac: 0.0,
        gas_frac: 0.95,
    };
    let (core_jup, border_jup, _) = calculate_auroral_emission_colors(&gas_comp, true);
    // Jovian: H-alpha pink/crimson core and UV/violet border
    assert!(
        core_jup.x > core_jup.y && core_jup.z > 0.5,
        "Jovian emission core should be pink/crimson H-alpha, got {core_jup:?}"
    );
    assert!(
        border_jup.z > border_jup.y,
        "Jovian lower border should have deep energetic UV/violet component, got {border_jup:?}"
    );
}

#[test]
fn test_cme_shockwave_propagation_and_arrival() {
    let mut app = App::new();
    app.insert_resource(SimTime {
        elapsed_years: 0.0,
        current_dt_yr: 0.0005, // ~4.38 hours per tick (standard Protostellar timestep)
        step_count: 0,
    });
    app.insert_resource(TimeWarp {
        multiplier: 1.0,
        is_paused: false,
        step_once: false,
    });
    app.add_message::<CmeShockwaveEvent>();

    // Spawn central star with active CME
    let _star = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Sol".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            CentralStar,
            Mass(1.0),
            SimPosition(DVec3::ZERO),
            StellarFlareState {
                flare_frequency: 1.0,
                current_flare_intensity: 5.0,
                flare_decay_timer_years: 0.5,
                cme_front_radius_au: 0.98, // Shock front just arriving at Earth's orbit
                cme_speed_au_day: 0.3,     // 0.3 AU/day
                cme_density_multiplier: 15.0,
                cme_active: true,
            },
        ))
        .id();

    // Spawn Earth at 1.0 AU with atmosphere and magnetic field
    let earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            Radius(EARTH_RADIUS_AU),
            Composition {
                silicate_frac: 0.70,
                metal_frac: 0.30,
                ice_frac: 0.0,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            VolatileInventory {
                atmospheric_pressure_bar: 1.0,
                ..Default::default()
            },
            InternalDifferentiation {
                is_differentiated: true,
                magnetic_field_gauss: 0.5,
                ..Default::default()
            },
            AuroralOvalState::default(),
        ))
        .id();

    app.add_systems(
        Update,
        (
            update_stellar_flares_system,
            update_planetary_auroral_ovals_system,
        ),
    );

    // Run first tick to attach AuroralOvalState and propagate CME shockwave
    app.update();

    // Verify CME shockwave event was dispatched for Earth
    let cme_events = app.world().resource::<Messages<CmeShockwaveEvent>>();
    assert!(
        !cme_events.is_empty(),
        "Expected CmeShockwaveEvent to be dispatched when shockwave engulfs Earth"
    );

    // Verify Earth received AuroralOvalState with elevated storm activity
    let earth_aurora = app
        .world()
        .get::<AuroralOvalState>(earth)
        .expect("Earth should have AuroralOvalState");
    assert!(
        earth_aurora.geomagnetic_kp_index > 4.0,
        "Earth Kp index should be elevated after CME shockwave: {}",
        earth_aurora.geomagnetic_kp_index
    );
    assert!(
        earth_aurora.magnetopause_standoff_rp < 9.0,
        "Earth magnetosphere should be compressed by CME shockwave: {}",
        earth_aurora.magnetopause_standoff_rp
    );
}

#[test]
fn test_auroral_oval_ecs_update() {
    let mut app = App::new();
    app.insert_resource(SimTime {
        elapsed_years: 0.0,
        current_dt_yr: 0.001,
        step_count: 0,
    });
    app.insert_resource(TimeWarp {
        multiplier: 1.0,
        is_paused: false,
        step_once: false,
    });
    app.add_message::<CmeShockwaveEvent>();

    // Spawn central star
    app.world_mut().spawn((
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        CentralStar,
        Mass(1.0),
        SimPosition(DVec3::ZERO),
    ));

    // Spawn magnetized terrestrial planet with atmosphere
    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Magneto".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.0, 0.0, 0.0)),
            Radius(EARTH_RADIUS_AU),
            Composition {
                silicate_frac: 0.70,
                metal_frac: 0.30,
                ice_frac: 0.0,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            VolatileInventory {
                atmospheric_pressure_bar: 1.0,
                ..Default::default()
            },
            InternalDifferentiation {
                is_differentiated: true,
                magnetic_field_gauss: 0.65,
                ..Default::default()
            },
        ))
        .id();

    app.add_systems(
        Update,
        (
            update_stellar_flares_system,
            update_planetary_auroral_ovals_system,
        ),
    );

    app.update();

    let aurora = app
        .world()
        .get::<AuroralOvalState>(planet)
        .expect("AuroralOvalState should be auto-inserted on magnetized planet");

    assert!(aurora.magnetopause_standoff_rp > 8.0);
    assert!(aurora.oval_colatitude_rad > 0.2 && aurora.oval_colatitude_rad < 0.5);
    assert!(aurora.oval_width_rad > 0.03);
    assert!(aurora.auroral_intensity > 0.5);
}
