//! Integration tests for Deep-Time Geological Epochs, Continental Drift,
//! Wilson Supercontinent Cycles, Ocean Oxidation, and Timeline Scrubber.

use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::geology::systems::*;
use protostellar::simulation::geology::types::*;
use protostellar::simulation::geology::GeologyPlugin;
use protostellar::simulation::resources::*;

#[test]
fn test_geological_epoch_classification() {
    // 1. Hadean (< 0.56 Gyr)
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(0.1),
        GeologicalEpoch::Hadean
    );
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(0.55),
        GeologicalEpoch::Hadean
    );

    // 2. Archean (0.56 - 2.06 Gyr)
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(0.56),
        GeologicalEpoch::Archean
    );
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(1.50),
        GeologicalEpoch::Archean
    );
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(2.05),
        GeologicalEpoch::Archean
    );

    // 3. Proterozoic (2.06 - 4.02 Gyr)
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(2.06),
        GeologicalEpoch::Proterozoic
    );
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(3.00),
        GeologicalEpoch::Proterozoic
    );

    // 4. Phanerozoic (4.02 - 4.55 Gyr)
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(4.10),
        GeologicalEpoch::Phanerozoic
    );
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(4.50),
        GeologicalEpoch::Phanerozoic
    );

    // 5. Modern (4.55 - 4.65 Gyr)
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(4.56),
        GeologicalEpoch::Modern
    );
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(4.60),
        GeologicalEpoch::Modern
    );

    // 6. Far-Future (> 4.65 Gyr)
    assert_eq!(
        GeologicalEpoch::from_geological_age_gyr(5.00),
        GeologicalEpoch::Future
    );

    // Sequential transitions
    assert_eq!(GeologicalEpoch::Hadean.next(), GeologicalEpoch::Archean);
    assert_eq!(
        GeologicalEpoch::Archean.next(),
        GeologicalEpoch::Proterozoic
    );
    assert_eq!(
        GeologicalEpoch::Proterozoic.next(),
        GeologicalEpoch::Phanerozoic
    );
    assert_eq!(GeologicalEpoch::Phanerozoic.next(), GeologicalEpoch::Modern);
    assert_eq!(GeologicalEpoch::Modern.next(), GeologicalEpoch::Future);
    assert_eq!(GeologicalEpoch::Future.next(), GeologicalEpoch::Future);

    assert_eq!(GeologicalEpoch::Modern.prev(), GeologicalEpoch::Phanerozoic);
    assert_eq!(
        GeologicalEpoch::Phanerozoic.prev(),
        GeologicalEpoch::Proterozoic
    );
    assert_eq!(GeologicalEpoch::Hadean.prev(), GeologicalEpoch::Hadean);
}

#[test]
fn test_wilson_cycle_continental_drift_and_aggregation() {
    // Phase cycles periodically every ~500 Myr (0.5 Gyr)
    let phase_0 = calculate_continental_drift_phase(0.0);
    let phase_250 = calculate_continental_drift_phase(0.25);
    let phase_500 = calculate_continental_drift_phase(0.50);
    let phase_1000 = calculate_continental_drift_phase(1.00);

    assert!((phase_0 - 0.0).abs() < 1e-4);
    assert!((phase_250 - 0.50).abs() < 1e-4);
    assert!((phase_500 - 0.0).abs() < 1e-4);
    assert!((phase_1000 - 0.0).abs() < 1e-4);

    // Supercontinent aggregation cycles between dispersed and tightly clustered
    let agg_clustered = calculate_supercontinent_aggregation(0.0);
    let agg_dispersed = calculate_supercontinent_aggregation(0.50);

    assert!(
        agg_clustered > 0.95,
        "Phase 0.0 should represent supercontinent aggregation"
    );
    assert!(
        agg_dispersed < 0.20,
        "Phase 0.5 should represent dispersed continents"
    );
}

#[test]
fn test_ocean_oxidation_and_great_oxidation_event() {
    // Archean (< 2.0 Gyr): anoxic green waters rich in dissolved ferrous iron
    let ox_early = calculate_ocean_oxidation_progress(1.0);
    let ox_archean = calculate_ocean_oxidation_progress(1.8);
    assert!(
        ox_early <= 0.05,
        "Early ocean should be anoxic green (ox={ox_early})"
    );
    assert!(
        ox_archean <= 0.05,
        "Archean ocean should remain anoxic (ox={ox_archean})"
    );

    // Great Oxidation Event (~2.0 - 2.7 Gyr)
    let ox_goe_mid = calculate_ocean_oxidation_progress(2.35);
    assert!(
        ox_goe_mid > 0.30 && ox_goe_mid < 0.80,
        "GOE midpoint should show active oxidation (ox={ox_goe_mid})"
    );

    // Modern / Phanerozoic (> 4.0 Gyr): fully oxidized sapphire blue
    let ox_modern = calculate_ocean_oxidation_progress(4.56);
    assert!(
        (ox_modern - 1.0).abs() < 1e-4,
        "Modern oceans must be 100% oxidized sapphire blue (ox={ox_modern})"
    );
}

#[test]
fn test_terrestrial_vegetation_expansion() {
    // Pre-Silurian (< 4.05 Gyr): sterile craton and bare rock
    let veg_archean = calculate_vegetation_expansion(1.5);
    let veg_proterozoic = calculate_vegetation_expansion(3.5);
    let veg_ordovician = calculate_vegetation_expansion(4.00);

    assert_eq!(veg_archean, 0.0);
    assert_eq!(veg_proterozoic, 0.0);
    assert_eq!(veg_ordovician, 0.0);

    // Devonian / Carboniferous land plant explosion (4.05 - 4.35 Gyr)
    let veg_mid_devonian = calculate_vegetation_expansion(4.20);
    assert!(
        veg_mid_devonian > 0.30 && veg_mid_devonian < 0.70,
        "Devonian plant colonization should be expanding (veg={veg_mid_devonian})"
    );

    // Modern (> 4.35 Gyr): abundant global flora
    let veg_modern = calculate_vegetation_expansion(4.56);
    assert_eq!(veg_modern, 1.0);
}

#[test]
fn test_epoch_scrubbing_and_climate_synchronization() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SimTime>()
        .add_plugins(GeologyPlugin);

    // Spawn a terrestrial planet (Earth)
    let earth = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            Mass(protostellar::utils::constants::EARTH_MASS_SOLAR),
            Radius(1.0),
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 1.0, 0.0)),
            VolatileInventory {
                delivered_water_m_earth: 0.0006,
                ocean_coverage_frac: 0.70,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 0,
            },
            PlanetaryClimate {
                surface_temperature_k: 288.0,
                equilibrium_temperature_k: 255.0,
                greenhouse_delta_k: 33.0,
                albedo: 0.30,
                ice_coverage_frac: 0.10,
                cloud_coverage_frac: 0.50,
                climate_regime: ClimateRegime::TemperateHabitable,
            },
        ))
        .id();

    // 1. Initial run: GeologyPlugin should auto-insert GeologicalState on Earth
    app.update();

    let geo = app
        .world()
        .get::<GeologicalState>(earth)
        .expect("GeologyPlugin must auto-insert GeologicalState on terrestrial planet");
    assert_eq!(geo.epoch, GeologicalEpoch::Modern);
    assert!((geo.geological_age_gyr - 4.56).abs() < 1e-2);

    // 2. Manual Scrub to Hadean: Lock scrubber, set active epoch to Hadean
    {
        let mut scrubber = app.world_mut().resource_mut::<TimelineScrubber>();
        scrubber.auto_advance = false;
        scrubber.active_epoch = GeologicalEpoch::Hadean;
        scrubber.scrubbed_age_gyr = GeologicalEpoch::Hadean.canonical_age_gyr();
    }

    app.update();

    let geo_hadean = app.world().get::<GeologicalState>(earth).unwrap();
    assert_eq!(geo_hadean.epoch, GeologicalEpoch::Hadean);
    assert_eq!(geo_hadean.terrestrial_vegetation_fraction, 0.0);
    assert!(geo_hadean.ocean_oxidation_progress < 0.10);

    let climate_hadean = app.world().get::<PlanetaryClimate>(earth).unwrap();
    assert!(
        climate_hadean.surface_temperature_k >= 500.0,
        "Hadean surface should be hot magma/steam (T={})",
        climate_hadean.surface_temperature_k
    );

    let vol_hadean = app.world().get::<VolatileInventory>(earth).unwrap();
    assert!(
        vol_hadean.atmospheric_pressure_bar >= 20.0,
        "Hadean should have superdense steam atmosphere"
    );

    // 3. Fine Scrub Stepping: Jump forward +100 Myr, then +10 Myr
    {
        let mut scrubber = app.world_mut().resource_mut::<TimelineScrubber>();
        scrubber.scrubbed_age_gyr += 0.110; // +110 Myr
        scrubber.active_epoch = GeologicalEpoch::from_geological_age_gyr(scrubber.scrubbed_age_gyr);
    }

    app.update();

    let geo_stepped = app.world().get::<GeologicalState>(earth).unwrap();
    assert!(
        (geo_stepped.geological_age_gyr - 0.360).abs() < 1e-3,
        "Scrubbed age should track exact fine time step"
    );

    // 4. Scrub back to Modern
    {
        let mut scrubber = app.world_mut().resource_mut::<TimelineScrubber>();
        scrubber.active_epoch = GeologicalEpoch::Modern;
        scrubber.scrubbed_age_gyr = 4.56;
    }

    app.update();

    let geo_modern = app.world().get::<GeologicalState>(earth).unwrap();
    assert_eq!(geo_modern.epoch, GeologicalEpoch::Modern);
    assert_eq!(geo_modern.terrestrial_vegetation_fraction, 1.0);
    assert_eq!(geo_modern.ocean_oxidation_progress, 1.0);
    assert_eq!(geo_modern.oxygen_level_pal, 1.0);

    let vol_modern = app.world().get::<VolatileInventory>(earth).unwrap();
    assert!((vol_modern.atmospheric_pressure_bar - 1.0).abs() < 0.1);
}
