//! Integration tests for Volcanic Worlds & Cryovolcanic Geysers (Io & Enceladus / Europa).

use protostellar::game::ui::types::BuilderPreset;
use protostellar::rendering::bodies::palettes::compute_terrestrial_body_palette;
use protostellar::simulation::components::celestial::*;
use protostellar::simulation::components::volcanism::*;
use protostellar::simulation::tides::compute_volcanic_state;

#[test]
fn test_io_canonical_sulfur_volcanism_properties() {
    let io_volc = VolcanicState::io();
    assert_eq!(
        io_volc.activity_type,
        VolcanicActivityType::IoSulfurVolcanism
    );
    assert_eq!(io_volc.active_plumes, 6);
    assert_eq!(io_volc.max_plume_height_km, 420.0);
    assert!(io_volc.caldera_temperature_k >= 1600.0);
    assert!(io_volc.sulfur_fallout_fraction >= 0.80);
    assert_eq!(io_volc.cryo_ring_feed_rate_kg_s, 0.0);
}

#[test]
fn test_enceladus_canonical_cryovolcanic_properties() {
    let enceladus_volc = VolcanicState::enceladus();
    assert_eq!(
        enceladus_volc.activity_type,
        VolcanicActivityType::CryoWaterIce
    );
    assert_eq!(enceladus_volc.active_plumes, 4);
    assert_eq!(enceladus_volc.max_plume_height_km, 500.0);
    assert!(enceladus_volc.cryo_ring_feed_rate_kg_s > 100.0);
}

#[test]
fn test_europa_canonical_cryotectonic_properties() {
    let europa_volc = VolcanicState::europa();
    assert_eq!(
        europa_volc.activity_type,
        VolcanicActivityType::CryoWaterIce
    );
    assert_eq!(europa_volc.active_plumes, 2);
    assert_eq!(europa_volc.max_plume_height_km, 200.0);
}

#[test]
fn test_tidal_flexure_volcanism_coupling_thresholds() {
    // 1. Hyperactive Io-class sulfur volcanism under extreme tidal heating
    let rocky_comp = Composition {
        silicate_frac: 0.85,
        ice_frac: 0.0,
        metal_frac: 0.15,
        gas_frac: 0.0,
        organics_frac: 0.0,
    };
    let io_opt = compute_volcanic_state("Io", &rocky_comp, 2.5, None);
    assert!(io_opt.is_some());
    let io_state = io_opt.unwrap();
    assert_eq!(
        io_state.activity_type,
        VolcanicActivityType::IoSulfurVolcanism
    );
    assert!(io_state.active_plumes >= 4);
    assert!(io_state.max_plume_height_km >= 300.0);
    assert!(io_state.caldera_temperature_k >= 1500.0);
    assert!(io_state.sulfur_fallout_fraction > 0.5);

    // 2. Cryovolcanic water-ice geysers on an icy moon under moderate tidal heating
    let icy_comp = Composition {
        silicate_frac: 0.08,
        ice_frac: 0.92,
        metal_frac: 0.0,
        gas_frac: 0.0,
        organics_frac: 0.0,
    };
    let enc_opt = compute_volcanic_state("Enceladus", &icy_comp, 0.05, None);
    assert!(enc_opt.is_some());
    let enceladus_state = enc_opt.unwrap();
    assert_eq!(
        enceladus_state.activity_type,
        VolcanicActivityType::CryoWaterIce
    );
    assert!(enceladus_state.active_plumes >= 1);
    assert!(enceladus_state.max_plume_height_km >= 120.0);
    assert!(enceladus_state.cryo_ring_feed_rate_kg_s > 0.0);

    // 3. Dormant moon below tidal dissipation threshold
    let dormant_opt = compute_volcanic_state("Cold Moon", &rocky_comp, 0.002, None);
    assert!(dormant_opt.is_none());
}

#[test]
fn test_builder_presets_and_palettes_for_io_and_enceladus() {
    // Presets
    let io_preset = BuilderPreset::IoLike;
    assert_eq!(io_preset.display_name(), "🌋 Volcanic (Io)");

    let enceladus_preset = BuilderPreset::EnceladusLike;
    assert_eq!(enceladus_preset.display_name(), "❄️ Cryo (Enceladus)");

    // Canonical Palettes
    let rocky_comp = Composition {
        silicate_frac: 0.85,
        ice_frac: 0.0,
        metal_frac: 0.15,
        gas_frac: 0.0,
        organics_frac: 0.0,
    };
    let io_col = compute_terrestrial_body_palette("Io (Volcanic Moon)", &rocky_comp, 130.0, None);
    let io_srgba = io_col.to_srgba();
    assert!(io_srgba.red > 0.70 && io_srgba.green > 0.60 && io_srgba.blue < 0.50);

    let icy_comp = Composition {
        silicate_frac: 0.08,
        ice_frac: 0.92,
        metal_frac: 0.0,
        gas_frac: 0.0,
        organics_frac: 0.0,
    };
    let enceladus_col =
        compute_terrestrial_body_palette("Enceladus (Cryo Moon)", &icy_comp, 75.0, None);
    let enc_srgba = enceladus_col.to_srgba();
    assert!(enc_srgba.red > 0.90 && enc_srgba.green > 0.90 && enc_srgba.blue > 0.90);
}

#[test]
fn test_planet_wgsl_shader_contains_volcanic_and_cryotectonic_features() {
    let shader = std::fs::read_to_string("assets/shaders/planet.wgsl")
        .expect("planet.wgsl must be readable");

    let required_features = [
        "fn evaluate_volcanic_calderas",
        "is_sulfur_world: bool",
        "fallout_halo_color",
        "fallout_halo",
        "fn evaluate_cryotectonic_fractures",
        "double_ridge",
        "salt_stain_color",
        "south_polar_weight",
        "stripe_fissure",
        "vent_cyan",
    ];

    for feature in required_features {
        assert!(
            shader.contains(feature),
            "planet.wgsl must contain {feature} for volcanic/cryotectonic fidelity"
        );
    }
}

#[test]
fn test_cryovolcanic_ring_feeding_system_updates_parent_ring() {
    use bevy::prelude::*;
    use protostellar::simulation::components::planetary::PlanetaryRingSystem;
    use protostellar::simulation::resources::{SimTime, SimulationConfig, TimeWarp};
    use protostellar::simulation::tides::cryovolcanic_ring_feeding_system;

    let mut app = App::new();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<SimulationConfig>();
    app.add_systems(Update, cryovolcanic_ring_feeding_system);

    // Parent planet with existing ring
    let parent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::GasGiant,
                name: "Proto-Saturn".to_string(),
            },
            PlanetaryRingSystem {
                inner_radius_au: 0.001,
                outer_radius_au: 0.003,
                ring_mass_earth: 0.0001,
                optical_depth: 0.50,
                ice_fraction: 0.90,
                silicate_fraction: 0.10,
            },
        ))
        .id();

    // Satellite with cryovolcanic activity feeding the ring
    let mut enc_volc = VolcanicState::enceladus();
    enc_volc.cryo_ring_feed_rate_kg_s = 5.0e15; // accelerated rate for testing

    app.world_mut().spawn((
        CelestialBody {
            body_type: BodyType::Moon,
            name: "Enceladus".to_string(),
        },
        SatelliteOf {
            parent,
            semi_major_axis_au: 0.00159,
            orbital_period_years: 0.00375,
            true_anomaly: 0.0,
        },
        enc_volc,
    ));

    // Run one update tick
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 1.0; // 1 year
    }
    app.update();

    let rings = app
        .world()
        .get::<PlanetaryRingSystem>(parent)
        .expect("Parent must still have PlanetaryRingSystem");

    assert!(
        rings.ring_mass_earth > 0.0001,
        "Parent ring mass must increase due to cryovolcanic feeding"
    );
    assert!(
        rings.optical_depth > 0.50,
        "Parent ring optical depth must increase with cryovolcanic ice grains"
    );
    assert!(
        rings.ice_fraction > 0.90,
        "Parent ring ice fraction must increase towards pure ice (>0.90)"
    );
}

#[test]
fn test_solar_nebula_mmsn_spawns_io_europa_and_enceladus() {
    use bevy::prelude::*;
    use protostellar::simulation::scenarios::spawn_solar_nebula_mmsn;

    let mut app = App::new();
    let mut disk_params = protostellar::simulation::resources::DiskParameters::default();
    spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let mut found_io = false;
    let mut found_europa = false;
    let mut found_enceladus = false;

    for (body, volc, sat) in app
        .world_mut()
        .query::<(&CelestialBody, Option<&VolcanicState>, Option<&SatelliteOf>)>()
        .iter(app.world())
    {
        if body.name.contains("Io") {
            found_io = true;
            assert_eq!(body.body_type, BodyType::Moon);
            let v = volc.expect("Io must have VolcanicState");
            assert_eq!(v.activity_type, VolcanicActivityType::IoSulfurVolcanism);
            assert!(v.active_plumes >= 4);
            assert!(sat.is_some(), "Io must be SatelliteOf Jupiter");
        } else if body.name.contains("Europa") {
            found_europa = true;
            assert_eq!(body.body_type, BodyType::Moon);
            let v = volc.expect("Europa must have VolcanicState");
            assert_eq!(v.activity_type, VolcanicActivityType::CryoWaterIce);
            assert!(sat.is_some(), "Europa must be SatelliteOf Jupiter");
        } else if body.name.contains("Enceladus") {
            found_enceladus = true;
            assert_eq!(body.body_type, BodyType::Moon);
            let v = volc.expect("Enceladus must have VolcanicState");
            assert_eq!(v.activity_type, VolcanicActivityType::CryoWaterIce);
            assert!(v.cryo_ring_feed_rate_kg_s > 0.0);
            assert!(sat.is_some(), "Enceladus must be SatelliteOf Saturn");
        }
    }

    assert!(found_io, "Io must be spawned in Solar MMSN scenario");
    assert!(
        found_europa,
        "Europa must be spawned in Solar MMSN scenario"
    );
    assert!(
        found_enceladus,
        "Enceladus must be spawned in Solar MMSN scenario"
    );
}
