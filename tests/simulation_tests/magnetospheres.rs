use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::rendering::bodies::{
    sync_magnetic_field_overlays, MagnetarStructureRoot, MagneticFieldLoopsPart,
    MagneticFieldOverlayRoot, VisualAssets,
};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::{
    DiagnosticOverlayMode, PlayerInteractionState, SimTime, SimulationConfig,
};
use protostellar::utils::constants::*;

#[test]
fn test_magnetic_field_overlay_mode_cycle_and_names() {
    let mode = DiagnosticOverlayMode::Realistic;
    assert_eq!(mode.display_name(), "Realistic PBR");

    let mode = mode.cycle();
    assert_eq!(mode, DiagnosticOverlayMode::SpectralComposition);
    assert_eq!(mode.display_name(), "Spectral Composition Map");

    let mode = mode.cycle();
    assert_eq!(mode, DiagnosticOverlayMode::HillSpheresAndGaps);
    assert_eq!(mode.display_name(), "Hill Spheres & Annular Gaps");

    let mode = mode.cycle();
    assert_eq!(mode, DiagnosticOverlayMode::MagneticFields);
    assert_eq!(mode.display_name(), "Magnetic Fields (GPU)");

    let mode = mode.cycle();
    assert_eq!(mode, DiagnosticOverlayMode::Hidden);
    assert_eq!(mode.display_name(), "No Overlay");

    let mode = mode.cycle();
    assert_eq!(mode, DiagnosticOverlayMode::Realistic);
}

#[test]
fn test_magnetic_field_overlay_skips_magnetar_and_empowers_companion_and_worlds() {
    let mut app = App::new();
    app.init_resource::<Time>();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<SimTime>();
    app.init_resource::<PlayerInteractionState>();

    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let fallback_mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));
    app.insert_resource(VisualAssets::dummy(fallback_mesh));

    // Enable GPU Magnetic Fields overlay mode
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .overlay_mode = DiagnosticOverlayMode::MagneticFields;

    app.add_systems(Update, sync_magnetic_field_overlays);

    // 1. Central SGR 1806-20 Magnetar (already has permanent 3D field loops)
    let magnetar_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Magnetar,
                name: "SGR 1806-20 (Magnetar)".to_string(),
            },
            Mass(1.4),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Radius(10.0 / AU_TO_KM),
            Composition::metal_rich(),
            MagnetarStructureRoot,
            ElectromagneticFieldState {
                magnetic_field_gauss: 1.0e15,
                rotation_period_sec: 7.5,
                magnetic_inclination_rad: 0.25,
                jet_length_au: 0.05,
                synchrotron_intensity: 1.0,
            },
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();

    // 2. LBV 1806-20 Hypergiant Companion Star (orbiting at 18 AU)
    let companion_star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::BlueSupergiant,
                name: "LBV 1806-20 (Hypergiant Companion)".to_string(),
            },
            Mass(45.0),
            SimPosition(DVec3::new(18.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 4.5)),
            Radius(0.22),
            Composition::solar_gas(),
            ElectromagneticFieldState {
                magnetic_field_gauss: 1500.0,
                rotation_period_sec: 10.0 * 86400.0,
                magnetic_inclination_rad: 0.1,
                jet_length_au: 0.0,
                synchrotron_intensity: 0.5,
            },
            Transform::from_xyz(18.0, 0.0, 0.0),
        ))
        .id();

    // 3. Valkyrie (Magnetized Terrestrial Planet at 0.48 AU)
    let valkyrie_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: "Valkyrie (Shattered Iron Core)".to_string(),
            },
            Mass(0.85 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(0.48, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 28.0)),
            Radius(0.88 * EARTH_RADIUS_AU),
            Composition::metal_rich(),
            InternalDifferentiation {
                is_differentiated: true,
                differentiation_fraction: 1.0,
                core_radius_au: 0.55 * EARTH_RADIUS_AU,
                mantle_radius_au: 0.88 * EARTH_RADIUS_AU,
                crust_thickness_au: 0.01 * EARTH_RADIUS_AU,
                ocean_ice_thickness_au: 0.0,
                core_temp_k: 4500.0,
                magnetic_field_gauss: 0.65,
                has_theia_llsvp: false,
                llsvp_density_contrast: 0.0,
            },
            Transform::from_xyz(0.48, 0.0, 0.0),
        ))
        .id();

    // 4. Undifferentiated rocky asteroid (no dynamo, no magnetic field)
    let asteroid_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                body_type: BodyType::Asteroid,
                name: "Rock-99".to_string(),
            },
            Mass(1e-10 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(3.0, 0.0, 0.0)),
            SimVelocity(DVec3::new(0.0, 0.0, 10.0)),
            Radius(10.0 / AU_TO_KM),
            Composition::rocky(),
            Transform::from_xyz(3.0, 0.0, 0.0),
        ))
        .id();

    app.update();

    let roots: Vec<(Entity, &MagneticFieldOverlayRoot)> = app
        .world_mut()
        .query::<(Entity, &MagneticFieldOverlayRoot)>()
        .iter(app.world())
        .collect();

    // 1. Assert the magnetar itself is NOT overlaid (no duplicate geometry!)
    let magnetar_overlay = roots.iter().find(|(_, r)| r.target_entity == magnetar_ent);
    assert!(
        magnetar_overlay.is_none(),
        "The central magnetar must be skipped by the magnetic field overlay since it already has permanent 3D field loops!"
    );

    // 2. Assert the companion star LBV 1806-20 HAS a magnetic field overlay
    let star_overlay = roots
        .iter()
        .find(|(_, r)| r.target_entity == companion_star_ent);
    assert!(
        star_overlay.is_some(),
        "Companion star LBV 1806-20 must have a GPU magnetic field overlay!"
    );

    // 3. Assert Valkyrie HAS a magnetic field overlay
    let valkyrie_overlay = roots.iter().find(|(_, r)| r.target_entity == valkyrie_ent);
    assert!(
        valkyrie_overlay.is_some(),
        "Magnetized world Valkyrie must have a GPU magnetic field overlay!"
    );

    // 4. Assert non-magnetized asteroid does NOT have an overlay
    let asteroid_overlay = roots.iter().find(|(_, r)| r.target_entity == asteroid_ent);
    assert!(
        asteroid_overlay.is_none(),
        "Non-magnetized asteroid should not have a magnetic field overlay!"
    );

    // 5. Verify children have MagneticFieldLoopsPart
    let loop_parts: Vec<&MagneticFieldLoopsPart> = app
        .world_mut()
        .query::<&MagneticFieldLoopsPart>()
        .iter(app.world())
        .collect();
    assert_eq!(
        loop_parts.len(),
        2,
        "Should have exactly 2 field loop meshes (companion star and Valkyrie)"
    );

    // 6. Test Lifecycle & Despawn when switching away from MagneticFields mode
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .overlay_mode = DiagnosticOverlayMode::Hidden;
    app.update();

    let roots_after_hide: Vec<(Entity, &MagneticFieldOverlayRoot)> = app
        .world_mut()
        .query::<(Entity, &MagneticFieldOverlayRoot)>()
        .iter(app.world())
        .collect();
    assert!(
        roots_after_hide.is_empty(),
        "All magnetic field overlays must be cleaned up when overlay mode is not MagneticFields!"
    );
}
