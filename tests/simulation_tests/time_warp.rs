//! Time warp progression, keyboard preset, and UI time-speed control tests.

use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;

#[test]
fn test_time_warp_real_time_progression() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<SimTime>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.insert_resource(TimeWarp {
        multiplier: TimeWarp::SPEED_REAL_TIME,
        is_paused: false,
        step_once: false,
    });
    app.add_systems(
        Update,
        protostellar::simulation::physics::step_physics_simulation,
    );

    // Verify speed formatting for real-time
    let time_warp = app.world().resource::<TimeWarp>();
    assert_eq!(time_warp.human_readable_speed(), "Real-Time (1s = 1.0s)");

    // Spawn Sun at origin
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(protostellar::utils::constants::SOLAR_RADIUS_AU),
        SimPosition(bevy::math::DVec3::ZERO),
        SimVelocity(bevy::math::DVec3::ZERO),
        SimAcceleration(bevy::math::DVec3::ZERO),
    ));

    // Spawn Earth at 1 AU with circular orbital velocity (~2*pi AU/yr)
    let earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(protostellar::utils::constants::EARTH_MASS_SOLAR),
            Radius(protostellar::utils::constants::EARTH_RADIUS_AU),
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 2.0 * std::f64::consts::PI, 0.0)),
            SimAcceleration(bevy::math::DVec3::ZERO),
        ))
        .id();

    let initial_pos = app.world().get::<SimPosition>(earth).unwrap().0;

    // Run 60 frames (1 wall-clock second at 60 fps)
    for _ in 0..60 {
        app.update();
    }

    let sim_time = app.world().resource::<SimTime>();
    let elapsed_seconds = sim_time.elapsed_years * TimeWarp::SECONDS_PER_YEAR;

    // In 60 frames, simulation time must have advanced by exactly 1.0 second!
    assert!(
        (elapsed_seconds - 1.0).abs() < 1e-4,
        "Real-time warp must advance exactly 1.0 second in 60 frames, got {elapsed_seconds:.6}s"
    );

    // Earth moves ~29.78 km/s in real life (~1.99e-7 AU in 1 second)
    let new_pos = app.world().get::<SimPosition>(earth).unwrap().0;
    let distance_traveled_au = (new_pos - initial_pos).length();
    let distance_traveled_km = distance_traveled_au * protostellar::utils::constants::AU_TO_KM;

    assert!(
        (distance_traveled_km - 29.78).abs() < 0.5,
        "In 1 real second, Earth should travel ~29.78 km, got {distance_traveled_km:.2} km"
    );

    // Also verify frame-rate independence: at 8 FPS (125ms wall-clock delta per frame),
    // 8 frames (= 1.0 wall-clock second) must still advance exactly 1.0 simulation second.
    app.world_mut().resource_mut::<SimTime>().elapsed_years = 0.0;
    for _ in 0..8 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(125));
        app.world_mut().run_schedule(Update);
    }
    let elapsed_8fps_secs =
        app.world().resource::<SimTime>().elapsed_years * TimeWarp::SECONDS_PER_YEAR;
    assert!(
        (elapsed_8fps_secs - 1.0).abs() < 1e-4,
        "1.0s of wall-clock time at 8 FPS must advance 1.0s of simulation time, got {elapsed_8fps_secs:.6}s"
    );

    // Verify 72h vs 1x (11 days/s) at 8 FPS: 1x must advance ~10.96 days/s (3.65x faster than 72h = 3.0 days/s)
    app.world_mut().resource_mut::<TimeWarp>().multiplier = TimeWarp::SPEED_72_HOURS;
    app.world_mut().resource_mut::<SimTime>().elapsed_years = 0.0;
    for _ in 0..8 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(125));
        app.world_mut().run_schedule(Update);
    }
    let days_72h = app.world().resource::<SimTime>().elapsed_years * 365.25;
    assert!(
        (days_72h - 3.0).abs() < 1e-3,
        "72h preset must advance 3.0 days in 1s at 8 FPS, got {days_72h:.4}"
    );

    app.world_mut().resource_mut::<TimeWarp>().multiplier = 1.0;
    app.world_mut().resource_mut::<SimTime>().elapsed_years = 0.0;
    for _ in 0..8 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(125));
        app.world_mut().run_schedule(Update);
    }
    let days_1x = app.world().resource::<SimTime>().elapsed_years * 365.25;
    assert!(
        (days_1x - 10.9575).abs() < 1e-2 && days_1x > days_72h * 3.5,
        "1x (11 days/s) must advance ~10.96 days in 1s at 8 FPS (strictly faster than 72h), got {days_1x:.4}"
    );
}

#[test]
fn test_time_warp_keyboard_presets_1_through_8() {
    use protostellar::game::time_control::handle_time_control_input;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<TimeWarp>();
    app.add_systems(Update, handle_time_control_input);

    let speed_tests = [
        (
            KeyCode::Digit1,
            TimeWarp::SPEED_REAL_TIME,
            "Real-Time (1s = 1.0s)",
        ),
        (KeyCode::Digit2, 1.0, "1.0x (1s = 11.0 days)"),
        (KeyCode::Digit3, 10.0, "10x (1s = 3.6 months)"),
        (KeyCode::Digit4, 100.0, "100x (1s = 3.0 yr)"),
        (KeyCode::Digit5, 1000.0, "1000x (1s = 30.0 yr)"),
        (KeyCode::Digit6, 10000.0, "10000x (1s = 300.0 yr)"),
        (KeyCode::Digit7, 100_000.0, "100000x (1s = 3.0k yr)"),
        (KeyCode::Digit8, 1_000_000.0, "1000000x (1s = 30.0k yr)"),
    ];

    for (key, expected_multiplier, expected_desc) in speed_tests {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);

        let time_warp = app.world().resource::<TimeWarp>();
        assert!(
            (time_warp.multiplier - expected_multiplier).abs() < 1e-9,
            "Key {:?} should set multiplier {}, got {}",
            key,
            expected_multiplier,
            time_warp.multiplier
        );
        assert_eq!(
            time_warp.human_readable_speed(),
            expected_desc,
            "Key {:?} speed description mismatch",
            key
        );
    }
}

#[test]
fn test_time_warp_one_minute_and_five_minutes_presets() {
    use protostellar::game::time_control::handle_time_control_input;
    use protostellar::game::ui::interactions::time_and_panels::handle_time_action;
    use protostellar::game::ui::types::{NotificationToast, UiButtonAction};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<DiskParameters>();
    app.init_resource::<EnergyMonitor>();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    app.add_systems(Update, handle_time_control_input);
    app.add_systems(
        Update,
        protostellar::simulation::physics::step_physics_simulation,
    );

    // Spawn central body so physics simulation ticks
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        Radius(protostellar::utils::constants::SOLAR_RADIUS_AU),
        SimPosition(bevy::math::DVec3::ZERO),
        SimVelocity(bevy::math::DVec3::ZERO),
        SimAcceleration(bevy::math::DVec3::ZERO),
    ));

    // 1. Verify Keyboard Digit9 sets 1s = 1 min
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit9);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::Digit9);

    let tw = app.world().resource::<TimeWarp>();
    assert!(
        (tw.multiplier - TimeWarp::SPEED_1_MINUTE).abs() < 1e-9,
        "Digit9 must activate SPEED_1_MINUTE"
    );
    assert_eq!(tw.human_readable_speed(), "1s = 1 min");

    // Run 60 frames (1 second wall time at 60 fps)
    let t0 = app.world().resource::<SimTime>().elapsed_years;
    for _ in 0..60 {
        app.update();
    }
    let t1 = app.world().resource::<SimTime>().elapsed_years;
    let elapsed_sec = (t1 - t0) * TimeWarp::SECONDS_PER_YEAR;
    assert!(
        (elapsed_sec - 60.0).abs() < 0.05,
        "60 frames at 1 min/s should advance exactly 60.0s (got {elapsed_sec:.2}s)"
    );

    // 2. Verify Keyboard Digit0 sets 1s = 5 min
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit0);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::Digit0);

    let tw = app.world().resource::<TimeWarp>();
    assert!(
        (tw.multiplier - TimeWarp::SPEED_5_MINUTES).abs() < 1e-9,
        "Digit0 must activate SPEED_5_MINUTES"
    );
    assert_eq!(tw.human_readable_speed(), "1s = 5 min");

    // Run 60 frames (1 second wall time at 60 fps)
    let t2 = app.world().resource::<SimTime>().elapsed_years;
    for _ in 0..60 {
        app.update();
    }
    let t3 = app.world().resource::<SimTime>().elapsed_years;
    let elapsed_sec_5m = (t3 - t2) * TimeWarp::SECONDS_PER_YEAR;
    assert!(
        (elapsed_sec_5m - 300.0).abs() < 0.1,
        "60 frames at 5 min/s should advance exactly 300.0s (got {elapsed_sec_5m:.2}s)"
    );

    // 3. Verify UI button action handlers for TimeSpeed1Min and TimeSpeed5Min
    let mut tw_test = TimeWarp::default();
    let mut toast = NotificationToast::default();

    let handled_1m = handle_time_action(&UiButtonAction::TimeSpeed1Min, &mut tw_test, &mut toast);
    assert!(handled_1m);
    assert_eq!(tw_test.multiplier, TimeWarp::SPEED_1_MINUTE);
    assert!(toast.message.contains("1 min"));

    let handled_5m = handle_time_action(&UiButtonAction::TimeSpeed5Min, &mut tw_test, &mut toast);
    assert!(handled_5m);
    assert_eq!(tw_test.multiplier, TimeWarp::SPEED_5_MINUTES);
    assert!(toast.message.contains("5 min"));
}

#[test]
fn test_time_warp_hour_presets_and_visual_clock() {
    use protostellar::game::time_control::handle_time_control_input;
    use protostellar::game::ui::interactions::time_and_panels::handle_time_action;
    use protostellar::game::ui::types::{NotificationToast, UiButtonAction};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<TimeWarp>();
    app.add_systems(Update, handle_time_control_input);

    let mut tw_test = TimeWarp::default();
    let mut toast = NotificationToast::default();

    // 1. Verify 6h, 12h, 24h, and 72h presets (UI actions, human_readable_speed, and Shift+3..6)
    let hour_presets = [
        (
            UiButtonAction::TimeSpeed6Hr,
            KeyCode::Digit3,
            TimeWarp::SPEED_6_HOURS,
            "1s = 6 hr",
        ),
        (
            UiButtonAction::TimeSpeed12Hr,
            KeyCode::Digit4,
            TimeWarp::SPEED_12_HOURS,
            "1s = 12 hr",
        ),
        (
            UiButtonAction::TimeSpeed24Hr,
            KeyCode::Digit5,
            TimeWarp::SPEED_24_HOURS,
            "1s = 24 hr",
        ),
        (
            UiButtonAction::TimeSpeed72Hr,
            KeyCode::Digit6,
            TimeWarp::SPEED_72_HOURS,
            "1s = 72 hr",
        ),
    ];

    for (action, digit_key, expected_mult, expected_label) in hour_presets {
        let handled = handle_time_action(&action, &mut tw_test, &mut toast);
        assert!(handled);
        assert!(
            (tw_test.multiplier - expected_mult).abs() < 1e-9,
            "UI action {:?} must set multiplier {}, got {}",
            action,
            expected_mult,
            tw_test.multiplier
        );
        assert_eq!(tw_test.human_readable_speed(), expected_label);

        {
            let mut kb = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            kb.clear();
            kb.press(KeyCode::ShiftLeft);
            kb.press(digit_key);
        }
        app.update();
        {
            let mut kb = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            kb.release(digit_key);
            kb.release(KeyCode::ShiftLeft);
            kb.clear();
        }

        let tw_kb = app.world().resource::<TimeWarp>();
        assert!(
            (tw_kb.multiplier - expected_mult).abs() < 1e-9,
            "Shift+{:?} must activate {}",
            digit_key,
            expected_label
        );
        assert_eq!(tw_kb.human_readable_speed(), expected_label);
    }

    // 5. Verify SimTime visual calendar clock decomposition (years, months, days, hours, minutes, seconds)
    let st_zero = SimTime::default();
    assert_eq!(st_zero.format_visual_clock(), "0y 00m 00d | 00h 00m 00s");

    // Construct an exact elapsed time: 2 years + 3 months + 5 days + 14 hours + 27 minutes + 49 seconds
    let exact_secs: f64 = 2.0 * 31_557_600.0
        + 3.0 * 2_629_800.0
        + 5.0 * 86_400.0
        + 14.0 * 3_600.0
        + 27.0 * 60.0
        + 49.0
        + 0.25; // fractional second padding to avoid float boundary rounding
    let st_sample = SimTime {
        elapsed_years: exact_secs / TimeWarp::SECONDS_PER_YEAR,
        ..Default::default()
    };
    assert_eq!(st_sample.decompose_calendar_clock(), (2, 3, 5, 14, 27, 49));
    assert_eq!(st_sample.format_visual_clock(), "2y 03m 05d | 14h 27m 49s");
}
