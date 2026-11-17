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
    let shader = std::fs::read_to_string("assets/shaders/planet.wesl")
        .expect("planet.wesl must be readable");

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

#[test]
fn test_ice_dwarf_vs_cryotectonic_moon_shader_routing() {
    use protostellar::rendering::bodies::palettes::resolve_planet_type_and_subtype;

    let icy_comp = Composition {
        metal_frac: 0.02,
        silicate_frac: 0.06,
        ice_frac: 0.92,
        gas_frac: 0.0,
        organics_frac: 0.0,
    };

    // Canonical terrestrial planets and major moons must route to planet_type == 3 with exact landmark subtypes
    let expected_terrestrials = [
        ("Mars", BodyType::TerrestrialPlanet, 1.0_f32),
        ("The Moon", BodyType::Moon, 2.0_f32),
        ("Mercury", BodyType::TerrestrialPlanet, 3.0_f32),
        ("Earth", BodyType::TerrestrialPlanet, 4.0_f32),
        ("Venus", BodyType::TerrestrialPlanet, 5.0_f32),
        ("Ganymede", BodyType::Moon, 6.0_f32),
        ("Callisto", BodyType::Moon, 7.0_f32),
        ("Titan", BodyType::Moon, 8.0_f32),
        ("Europa", BodyType::Moon, 0.0_f32),
        ("Enceladus", BodyType::Moon, 0.0_f32),
    ];
    for (name, b_type, exp_sub) in expected_terrestrials {
        let (p_type, subtype) = resolve_planet_type_and_subtype(b_type, name, &icy_comp);
        assert_eq!(
            p_type, 3,
            "{name} must route to planet_type 3, got {p_type}"
        );
        assert!(
            (subtype - exp_sub).abs() < 1e-5,
            "{name} expected terrestrial subtype {exp_sub}, got {subtype}"
        );
    }

    // Named dwarf planets must route to planet_type == 9 with exact subtypes 1.0..=7.0
    let expected_dwarfs = [
        ("Pluto", BodyType::TerrestrialPlanet, 1.0_f32),
        ("Charon", BodyType::Moon, 2.0_f32),
        ("Eris", BodyType::TerrestrialPlanet, 3.0_f32),
        ("Haumea", BodyType::TerrestrialPlanet, 4.0_f32),
        ("Makemake", BodyType::TerrestrialPlanet, 5.0_f32),
        ("Sedna", BodyType::TerrestrialPlanet, 6.0_f32),
        ("Ceres", BodyType::Asteroid, 7.0_f32),
    ];
    for (name, b_type, exp_sub) in expected_dwarfs {
        let (p_type, subtype) = resolve_planet_type_and_subtype(b_type, name, &icy_comp);
        assert_eq!(p_type, 9, "{name} must route to planet_type 9");
        assert!(
            (subtype - exp_sub).abs() < 1e-5,
            "{name} expected subtype {exp_sub}, got {subtype}"
        );
    }
}

#[test]
fn test_snowball_earth_and_frozen_planets_exclude_cryotectonic_fractures() {
    crate::shader_guardrails::assert_shader_patterns(
        "assets/shaders/planet.wesl",
        &[
            (
                "let is_cryotectonic = planet.composition.y > 0.30",
                "Cryotectonic fractures must be gated on bulk water ice composition > 0.30",
            ),
            (
                "sastrugi_macro",
                "Snowball surface must contain sastrugi wind dune features",
            ),
            (
                "nunatak_thresh",
                "Snowball surface must model continental nunatak mountain peaks",
            ),
        ],
        &[(
            "planet.climate_and_bio.y > 0.35\n            && (planet.planet_type == 4u",
            "Surface climate ice fraction must not trigger cryotectonic fractures",
        )],
    );
}

#[test]
fn test_procedural_moon_naming_shader_and_mega_earth_gas_envelope() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::rendering::bodies::palettes::resolve_planet_type_and_subtype;
    use protostellar::simulation::accretion::collisions::naming::{
        clean_moon_parent_name, is_canonical_named_body, resolve_satellite_moon_name,
        update_merged_body_name,
    };
    use protostellar::simulation::components::{PlanetaryClimate, VolatileInventory};
    use protostellar::simulation::resources::TimeWarp;
    use protostellar::simulation::thermodynamics::retained_gas_envelope_bar;
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU, JUPITER_MASS_SOLAR};

    let rocky_comp = Composition {
        metal_frac: 0.21,
        silicate_frac: 0.75,
        ice_frac: 0.04,
        gas_frac: 0.0,
        organics_frac: 0.0,
    };

    // Procedural moons must not hijack Earth's Moon (subtype 2.0) landmark shader
    let (m_type, m_sub) =
        resolve_planet_type_and_subtype(BodyType::Moon, "Super-Jupiter I (Moon)", &rocky_comp);
    assert_eq!(m_type, 3);
    assert!((m_sub - 0.0).abs() < 1e-5);

    // Clean sequential moon naming strips parent mass tags and increments Roman numerals
    assert_eq!(
        clean_moon_parent_name("Super-Jupiter (1.5 M_J)"),
        "Super-Jupiter"
    );
    assert_eq!(
        resolve_satellite_moon_name("Super-Jupiter (1.5 M_J)", "Proto-Planet #12", 0),
        "Super-Jupiter I (Moon)"
    );
    assert_eq!(
        resolve_satellite_moon_name("Super-Jupiter (1.5 M_J)", "Proto-Planet #15", 1),
        "Super-Jupiter II (Moon)"
    );
    assert!(!is_canonical_named_body("Super-Jupiter (1.5 M_J)"));

    let mut reclass_name = "Super-Jupiter (1.5 M_J)".to_string();
    update_merged_body_name(
        &mut reclass_name,
        "Proto-Planet #9",
        BodyType::SuperEarth,
        1.5 * JUPITER_MASS_SOLAR,
    );
    assert_eq!(reclass_name, "Mega-Earth (1.5 M_J)");

    // Mega-Earth with 4% H2/He gas must bind a crushing atmospheric envelope; 0.0011 M_earth moons must not
    let mega_bar = retained_gas_envelope_bar(1.5 * JUPITER_MASS_SOLAR, 0.04);
    assert!(
        mega_bar > 400.0,
        "Expected >400 bar envelope, got {mega_bar}"
    );
    assert_eq!(
        retained_gas_envelope_bar(0.0011 * EARTH_MASS_SOLAR, 0.04),
        0.0
    );

    // End-to-end thermodynamics check for a 1.5 M_J Mega-Earth with 4% gas
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        protostellar::simulation::SimulationPlugin,
    ));
    app.init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();
    {
        let mut tw = app.world_mut().resource_mut::<TimeWarp>();
        tw.is_paused = false;
        tw.multiplier = 100.0;
    }
    app.world_mut().spawn((
        CentralStar,
        CelestialBody {
            name: "Genesis Protostar".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.33),
        Radius(0.006),
        SimPosition(DVec3::ZERO),
        SimVelocity(DVec3::ZERO),
        Temperature(5800.0),
        Composition::default(),
    ));
    let mega_earth_comp = Composition {
        metal_frac: 0.21,
        silicate_frac: 0.71,
        ice_frac: 0.04,
        gas_frac: 0.04,
        organics_frac: 0.0,
    };
    let (se_type, _) =
        resolve_planet_type_and_subtype(BodyType::SuperEarth, &reclass_name, &mega_earth_comp);
    assert_eq!(se_type, 6);

    let mega_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: reclass_name,
                body_type: BodyType::SuperEarth,
            },
            Mass(1.5 * JUPITER_MASS_SOLAR),
            Radius(EARTH_RADIUS_AU * 8.0),
            SimPosition(DVec3::new(5.6, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 2.5)),
            Temperature(250.0),
            mega_earth_comp,
            VolatileInventory {
                delivered_water_m_earth: 0.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.0,
                cometary_impact_count: 0,
            },
            PlanetaryClimate::default(),
        ))
        .id();

    app.update();
    app.update();

    let vol = app.world().get::<VolatileInventory>(mega_ent).unwrap();
    let climate = app.world().get::<PlanetaryClimate>(mega_ent).unwrap();
    assert!(
        vol.atmospheric_pressure_bar > 400.0,
        "1.5 M_J Mega-Earth with 4% gas must sustain high pressure, got {}",
        vol.atmospheric_pressure_bar
    );
    assert!(
        climate.cloud_coverage_frac >= 0.45,
        "1.5 M_J Mega-Earth with 4% gas must form dense clouds, got {}",
        climate.cloud_coverage_frac
    );
}

#[test]
fn test_quick_bar_moon_isolation_and_super_jupiter_labeling() {
    use protostellar::game::ui::{
        body_to_button_colors, body_to_button_label, is_canonical_major_planet, is_major_body,
    };
    use protostellar::simulation::components::BodyType;
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, JUPITER_MASS_SOLAR};

    assert!(!is_canonical_major_planet(
        "Super-Jupiter (1.5 M_J) I (Moon)"
    ));
    assert!(!is_major_body(
        "Super-Jupiter (1.5 M_J) I (Moon)",
        BodyType::Moon,
        false,
        0.0011 * EARTH_MASS_SOLAR
    ));
    assert!(is_major_body(
        "Super-Jupiter (1.5 M_J)",
        BodyType::GasGiant,
        false,
        1.5 * JUPITER_MASS_SOLAR
    ));

    let moon_label =
        body_to_button_label("Super-Jupiter (1.5 M_J) I (Moon)", BodyType::Moon, false);
    let jup_label = body_to_button_label("Super-Jupiter (1.5 M_J)", BodyType::GasGiant, false);
    assert_ne!(moon_label, jup_label);
    assert!(!moon_label.contains('…') && !moon_label.contains("..."));
    assert!(moon_label.contains('I'));

    let (moon_bg, _) =
        body_to_button_colors("Super-Jupiter (1.5 M_J) I (Moon)", BodyType::Moon, false);
    let (jup_bg, _) = body_to_button_colors("Super-Jupiter (1.5 M_J)", BodyType::GasGiant, false);
    assert_ne!(moon_bg, jup_bg);
}

#[test]
fn test_gas_giant_volcanism_gated_when_envelope_thick() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::resources::{SimTime, SimulationConfig, TimeWarp};
    use protostellar::simulation::tides::{TidalConfig, update_tidal_evolution};
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    // 64% Rock | 20% Metal | 16% Gas giant should NOT spawn vacuum Io umbrella plumes
    let rocky_giant_comp = Composition {
        silicate_frac: 0.64,
        ice_frac: 0.0,
        metal_frac: 0.20,
        organics_frac: 0.0,
        gas_frac: 0.16,
    };
    assert!(
        compute_volcanic_state("PLANET-5AU", &rocky_giant_comp, 693.0, None).is_none(),
        "Thick gas envelope (gas_frac > 0.12) must suppress vacuum umbrella plumes"
    );

    // Once photoevaporation thins the envelope to <= 12%, vacuum volcanism can emerge
    let thinned_comp = Composition {
        silicate_frac: 0.68,
        ice_frac: 0.0,
        metal_frac: 0.22,
        organics_frac: 0.0,
        gas_frac: 0.10,
    };
    let active = compute_volcanic_state("PLANET-5AU", &thinned_comp, 1.5, None)
        .expect("Exposed silicate body (gas_frac <= 0.12) with high tidal flux should erupt");
    assert_eq!(
        active.activity_type,
        VolcanicActivityType::IoSulfurVolcanism
    );

    // Verify ECS tidal update removes stale VolcanicState if gas_frac exceeds 0.12
    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.insert_resource(TimeWarp {
        is_paused: false,
        ..Default::default()
    });
    app.insert_resource(TidalConfig {
        is_enabled: true,
        ..Default::default()
    });
    app.add_systems(Update, update_tidal_evolution);

    let star = app
        .world_mut()
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Host Star".to_string(),
                body_type: BodyType::YellowDwarf,
            },
            Mass(1.56),
            Radius(0.005),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Temperature(5800.0),
            Composition::solar_gas(),
        ))
        .id();

    let planet = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "PLANET-5AU".to_string(),
                body_type: BodyType::GasGiant,
            },
            Mass(122.5 * EARTH_MASS_SOLAR),
            Radius(0.0004),
            SimPosition(DVec3::new(0.19, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 16.0)),
            Temperature(1022.0),
            rocky_giant_comp,
            SpinState::default(),
            VolcanicState::io(),
        ))
        .id();
    let _ = star;

    {
        let mut st = app.world_mut().resource_mut::<SimTime>();
        st.current_dt_yr = 0.01;
    }
    app.update();

    assert!(
        app.world().get::<VolcanicState>(planet).is_none(),
        "update_tidal_evolution must remove vacuum VolcanicState once gas_frac > 0.12"
    );
}

#[test]
fn test_rocky_core_gas_giant_outgassing_telemetry() {
    use protostellar::game::ui::telemetry_format::{
        format_core_envelope_outgassing_telemetry, has_deep_core_envelope_outgassing,
    };

    let comp_5au = Composition {
        silicate_frac: 0.64,
        ice_frac: 0.0,
        metal_frac: 0.20,
        organics_frac: 0.0,
        gas_frac: 0.16,
    };
    assert!(has_deep_core_envelope_outgassing(
        &comp_5au,
        BodyType::GasGiant
    ));
    let line = format_core_envelope_outgassing_telemetry(&comp_5au, BodyType::GasGiant, 1022.0);
    assert!(line.contains("Core-Envelope Volcanism"));
    assert!(line.contains("Upwelling Clouds"));

    // Standard H/He-rich Jupiter does not report rocky-core outgassing
    assert!(!has_deep_core_envelope_outgassing(
        &Composition::solar_gas(),
        BodyType::GasGiant
    ));
    assert_eq!(
        format_core_envelope_outgassing_telemetry(
            &Composition::solar_gas(),
            BodyType::GasGiant,
            165.0
        ),
        ""
    );
}

#[test]
fn test_atmosphere_and_cloud_toggles_and_badges() {
    use bevy::prelude::*;
    use protostellar::game::ui::interactions::time_and_panels::handle_instrument_action;
    use protostellar::game::ui::types::{
        HudDynamicText, HudVisibilityState, NotificationToast, PlanetBuilderState,
        TelemetryPanelState, UiButtonAction,
    };
    use protostellar::game::ui::visibility::update_hud_visibility;
    use protostellar::simulation::resources::PlayerInteractionState;

    let mut player_state = PlayerInteractionState::default();
    assert!(player_state.show_atmospheres());
    assert!(player_state.show_clouds());

    let mut toast = NotificationToast::default();
    assert!(handle_instrument_action(
        &UiButtonAction::ToggleAtmospheres,
        &mut player_state,
        &mut toast,
        None,
        None,
    ));
    assert!(!player_state.show_atmospheres());
    assert!(player_state.show_clouds());

    assert!(handle_instrument_action(
        &UiButtonAction::ToggleClouds,
        &mut player_state,
        &mut toast,
        None,
        None,
    ));
    assert!(!player_state.show_atmospheres());
    assert!(!player_state.show_clouds());

    // Verify dynamic badges update in ECS
    let mut app = App::new();
    app.init_resource::<HudVisibilityState>();
    app.insert_resource(player_state);
    app.init_resource::<PlanetBuilderState>();
    app.init_resource::<TelemetryPanelState>();
    app.add_systems(Update, update_hud_visibility);

    let atmo_badge = app
        .world_mut()
        .spawn((Text::new("init"), HudDynamicText::AtmospheresBadge))
        .id();
    let cloud_badge = app
        .world_mut()
        .spawn((Text::new("init"), HudDynamicText::CloudsBadge))
        .id();

    app.update();
    assert!(
        app.world()
            .get::<Text>(atmo_badge)
            .unwrap()
            .0
            .contains("OFF")
    );
    assert!(
        app.world()
            .get::<Text>(cloud_badge)
            .unwrap()
            .0
            .contains("OFF")
    );

    {
        let mut ps = app.world_mut().resource_mut::<PlayerInteractionState>();
        ps.hide_atmospheres = false;
        ps.hide_clouds = false;
    }
    app.update();
    assert!(
        app.world()
            .get::<Text>(atmo_badge)
            .unwrap()
            .0
            .contains("ON")
    );
    assert!(
        app.world()
            .get::<Text>(cloud_badge)
            .unwrap()
            .0
            .contains("ON")
    );
}
