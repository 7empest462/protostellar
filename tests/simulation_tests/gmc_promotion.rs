//! Integration tests for GMC protostar promotion, circumstellar disks, active disk capping,
//! inspector telemetry, selective gas accretion, and multi-system Cluster COM camera.

use bevy::prelude::*;

use protostellar::rendering::camera::PanOrbitCamera;
use protostellar::simulation::accretion::gas::gmc_cluster_gas_accretion;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::molecular_cloud::*;
use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

#[test]
fn test_gmc_promotion_toast_formatting() {
    let mut app = App::new();
    app.init_resource::<protostellar::game::ui::NotificationToast>();

    let star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Protostar Jeans-3".to_string(),
                body_type: BodyType::Protostar,
            },
            Mass(1.2),
            SimPosition(bevy::math::DVec3::ZERO),
            SimVelocity(bevy::math::DVec3::ZERO),
        ))
        .id();

    let mut state: bevy::ecs::system::SystemState<(
        Commands,
        Option<ResMut<protostellar::game::ui::NotificationToast>>,
    )> = bevy::ecs::system::SystemState::new(app.world_mut());

    let (mut commands, mut toast) = state.get_mut(app.world_mut()).expect("State valid");

    attach_gmc_circumstellar_disk_and_embryos(
        &mut commands,
        star_ent,
        "Protostar Jeans-3",
        bevy::math::DVec3::ZERO,
        bevy::math::DVec3::ZERO,
        1.2,
        0.02,
        &mut toast,
    );
    state.apply(app.world_mut());

    let toast_res = app
        .world()
        .get_resource::<protostellar::game::ui::NotificationToast>()
        .unwrap();

    assert_eq!(
        toast_res.message, "Protostar Jeans-3: ignited (3 worlds formed)",
        "Toast must match user visible handoff format"
    );
    assert!(toast_res.timer > 0.0);
    assert!(
        app.world().get::<CircumstellarDisk>(star_ent).is_none(),
        "GMC star must not attach a CircumstellarDisk"
    );
    assert!(
        app.world().get::<PlanetaryRingSystem>(star_ent).is_none(),
        "GMC star must not attach a PlanetaryRingSystem"
    );
    assert!(
        app.world().get::<GmcPromotedSystem>(star_ent).is_some(),
        "GMC star must be marked GmcPromotedSystem"
    );
}

#[test]
fn test_gmc_brown_dwarf_promotes_through_stellar_classes_and_imbh_collapse() {
    let mut app = App::new();
    app.insert_resource(ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        ..Default::default()
    });
    app.insert_resource(SimulationConfig {
        enable_accretion: true,
        gas_density_scale: 1.0,
        ..Default::default()
    });
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();

    // Regression case: A GasGiant/Super-Jupiter that promoted to BrownDwarf without IgnitionState
    let brown_dwarf_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Super-Jupiter (8.4 M_J)".to_string(),
                body_type: BodyType::BrownDwarf,
            },
            Mass(0.079),
            SimPosition(bevy::math::DVec3::new(20.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.0, 1.0)),
            Radius(0.0007),
            Temperature(458.0),
            Composition::solar_gas(),
        ))
        .id();

    app.add_systems(Update, gmc_cluster_gas_accretion);

    // Step across 0.08 M_sun threshold via Bondi-Hoyle cloud accretion
    for _ in 0..8 {
        app.world_mut().resource_mut::<SimTime>().current_dt_yr = 2.0;
        app.update();
        let body = app.world().get::<CelestialBody>(brown_dwarf_ent).unwrap();
        let mass = app.world().get::<Mass>(brown_dwarf_ent).unwrap().0;
        if mass >= 0.08 && body.body_type != BodyType::BrownDwarf {
            break;
        }
    }

    let body = app.world().get::<CelestialBody>(brown_dwarf_ent).unwrap();
    let mass = app.world().get::<Mass>(brown_dwarf_ent).unwrap().0;
    let temp = app.world().get::<Temperature>(brown_dwarf_ent).unwrap().0;
    assert!(
        mass >= 0.08,
        "Sub-stellar Brown Dwarf must accrete past hydrogen ignition threshold"
    );
    assert_ne!(
        body.body_type,
        BodyType::BrownDwarf,
        "Brown Dwarf above 0.08 M_sun must promote into a hydrogen-burning star, not stay BrownDwarf (mass={mass:.3})"
    );
    assert!(
        !body.name.contains("Super-Jupiter"),
        "Promoted star name must replace stale Super-Jupiter label (got '{}')",
        body.name
    );
    assert!(
        temp > 2500.0,
        "Promoted star must heat from 458 K up to stellar photospheric temperature (got {temp:.0} K)"
    );
    assert!(
        app.world().get::<IgnitionState>(brown_dwarf_ent).is_some(),
        "Promoted star must receive IgnitionState for radiation feedback and stellar evolution"
    );
    assert!(
        app.world()
            .get::<StellarEvolutionState>(brown_dwarf_ent)
            .is_some(),
        "Promoted star must receive StellarEvolutionState"
    );

    // Also verify high-mass direct collapse above 500 M_sun produces BlackHole
    {
        let mut m_mut = app.world_mut().get_mut::<Mass>(brown_dwarf_ent).unwrap();
        m_mut.0 = 550.0;
    }
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 0.5;
    app.update();

    let collapsed_body = app.world().get::<CelestialBody>(brown_dwarf_ent).unwrap();
    assert_eq!(
        collapsed_body.body_type,
        BodyType::BlackHole,
        "Mass >= 500 M_sun must collapse directly to Intermediate-Mass Black Hole"
    );
    assert!(
        collapsed_body.name.contains("Black Hole"),
        "Collapsed remnant must update name to Black Hole (got '{}')",
        collapsed_body.name
    );
}

#[test]
fn test_circumstellar_disk_inspector_telemetry() {
    use protostellar::game::ui::inspector_panel::append_circumstellar_disk_telemetry;

    let active_disk = CircumstellarDisk {
        inner_radius_au: 1.2,
        outer_radius_au: 14.5,
        disk_mass_solar: 0.025,
        is_active_full_disk: true,
        age_years: 12_400.0,
        lifetime_years: 75_000.0,
    };

    let mut text_active = String::new();
    append_circumstellar_disk_telemetry(&mut text_active, &active_disk);
    assert!(text_active.contains("ACTIVE (Full 3D Accretion)"));
    assert!(text_active.contains("12.4 kyr"));
    assert!(text_active.contains("75 kyr"));
    assert!(text_active.contains("0.0250 M☉"));

    let inactive_disk = CircumstellarDisk {
        inner_radius_au: 0.8,
        outer_radius_au: 9.0,
        disk_mass_solar: 0.005,
        is_active_full_disk: false,
        age_years: 82_000.0,
        lifetime_years: 75_000.0,
    };

    let mut text_inactive = String::new();
    append_circumstellar_disk_telemetry(&mut text_inactive, &inactive_disk);
    assert!(text_inactive.contains("CLEARED / DISSIPATED"));
    assert!(text_inactive.contains("82.0 kyr"));
}

#[test]
fn test_gmc_accretion_only_runs_on_active_full_disk_hosts() {
    let mut app = App::new();
    let scenario_state = ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        ..Default::default()
    };
    app.insert_resource(scenario_state);

    let config = SimulationConfig {
        enable_accretion: true,
        gas_density_scale: 1.0,
        ..Default::default()
    };
    app.insert_resource(config);
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();

    // Host 1: Active full disk
    app.world_mut().spawn((
        CelestialBody {
            name: "Host-Active".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        SimPosition(bevy::math::DVec3::new(0.0, 0.0, 0.0)),
        SimVelocity(bevy::math::DVec3::ZERO),
        Radius(0.005),
        Composition::solar_gas(),
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 0.1,
        },
        CircumstellarDisk {
            inner_radius_au: 0.5,
            outer_radius_au: 10.0,
            disk_mass_solar: 0.02,
            is_active_full_disk: true,
            age_years: 5_000.0,
            lifetime_years: 75_000.0,
        },
    ));

    // Protoplanet orbiting Host 1
    let planet_active_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Embryo-Active-Host".to_string(),
                body_type: BodyType::Protoplanet,
            },
            Mass(0.000_001),
            SimPosition(bevy::math::DVec3::new(2.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::ZERO),
            Radius(0.0001),
            Composition::solar_gas(),
            VolatileInventory::default(),
        ))
        .id();

    // Host 2: Inactive / cleared disk (capped or aged)
    app.world_mut().spawn((
        CelestialBody {
            name: "Host-Inactive".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        Mass(1.0),
        SimPosition(bevy::math::DVec3::new(100.0, 0.0, 0.0)),
        SimVelocity(bevy::math::DVec3::ZERO),
        Radius(0.005),
        Composition::solar_gas(),
        IgnitionState {
            core_temperature: 1.5e7,
            fusion_fraction: 1.0,
            is_ignited: true,
            shockwave_radius: 0.1,
        },
        CircumstellarDisk {
            inner_radius_au: 0.5,
            outer_radius_au: 10.0,
            disk_mass_solar: 0.001,
            is_active_full_disk: false,
            age_years: 80_000.0,
            lifetime_years: 75_000.0,
        },
    ));

    // Protoplanet orbiting Host 2
    let planet_inactive_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Embryo-Inactive-Host".to_string(),
                body_type: BodyType::Protoplanet,
            },
            Mass(0.000_001),
            SimPosition(bevy::math::DVec3::new(102.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::ZERO),
            Radius(0.0001),
            Composition::solar_gas(),
            VolatileInventory::default(),
        ))
        .id();

    app.add_systems(Update, gmc_cluster_gas_accretion);
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 1.0;
    }

    app.update();

    let m_active = app.world().get::<Mass>(planet_active_ent).unwrap().0;
    let m_inactive = app.world().get::<Mass>(planet_inactive_ent).unwrap().0;

    assert!(
        m_active > 0.000_001,
        "Protoplanet orbiting host with active full disk must accrete envelope mass"
    );
    assert_eq!(
        m_inactive, 0.000_001,
        "Protoplanet orbiting host with inactive full disk must skip full envelope accretion"
    );
}

#[test]
fn test_gmc_cluster_com_camera_activates_with_3_plus_systems() {
    let mut app = App::new();
    let scenario_state = ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        ..Default::default()
    };
    app.insert_resource(scenario_state);

    let cam_ent = app
        .world_mut()
        .spawn(PanOrbitCamera {
            target_focus: Vec3::ZERO,
            focus: Vec3::ZERO,
            target_radius: 300.0,
            radius: 300.0,
            target_entity: None,
            ..Default::default()
        })
        .id();

    app.add_systems(Update, update_gmc_cluster_com_camera);

    // Spawn 2 stars
    app.world_mut().spawn((
        CelestialBody {
            name: "Star-1".to_string(),
            body_type: BodyType::Protostar,
        },
        Mass(2.0),
        SimPosition(bevy::math::DVec3::new(50.0, 0.0, 0.0)),
    ));
    app.world_mut().spawn((
        CelestialBody {
            name: "Star-2".to_string(),
            body_type: BodyType::Protostar,
        },
        Mass(2.0),
        SimPosition(bevy::math::DVec3::new(-50.0, 0.0, 0.0)),
    ));

    app.update();

    let cam = app.world().get::<PanOrbitCamera>(cam_ent).unwrap();
    assert_eq!(
        cam.target_focus,
        Vec3::ZERO,
        "Cluster COM camera should not activate with fewer than 3 systems"
    );

    // Spawn 3rd star to reach 3+ systems
    app.world_mut().spawn((
        CelestialBody {
            name: "Star-3".to_string(),
            body_type: BodyType::Protostar,
        },
        Mass(4.0),
        SimPosition(bevy::math::DVec3::new(0.0, 80.0, 0.0)),
    ));

    app.update();

    let cam = app.world().get::<PanOrbitCamera>(cam_ent).unwrap();
    // Total mass = 2 + 2 + 4 = 8
    // COM = (2 * (50,0,0) + 2 * (-50,0,0) + 4 * (0,80,0)) / 8 = (0, 320, 0) / 8 = (0, 40, 0)
    assert!(
        (cam.target_focus.y - 40.0).abs() < 1e-3,
        "Cluster COM camera must track mass-weighted center of mass with 3+ systems (got {:?})",
        cam.target_focus
    );

    // When user explicitly selects and focuses a specific entity, target_entity is Some, so COM tracking is suspended
    {
        let mut cam_mut = app.world_mut().get_mut::<PanOrbitCamera>(cam_ent).unwrap();
        cam_mut.target_entity = Some(cam_ent);
        cam_mut.target_focus = Vec3::new(999.0, 999.0, 999.0);
    }
    app.update();
    let cam = app.world().get::<PanOrbitCamera>(cam_ent).unwrap();
    assert_eq!(
        cam.target_focus,
        Vec3::new(999.0, 999.0, 999.0),
        "Cluster COM camera must not override individual target entity focus"
    );
}

#[test]
fn test_universal_bombardment_availability_across_scenarios_and_body_types() {
    use protostellar::game::ui::types::*;
    use protostellar::game::ui::visibility::update_scenario_contextual_ui;

    let mut app = App::new();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<SimulationConfig>();

    let sec_bombardment = app
        .world_mut()
        .spawn((InspectorSection::TerraformingBombardment, Node::default()))
        .id();
    app.world_mut()
        .spawn((Text::new(""), InspectorExoticHeader));

    app.add_systems(Update, update_scenario_contextual_ui);

    // Non-solar scenarios to test
    let test_presets = [
        ScenarioPreset::MolecularCloudCluster,
        ScenarioPreset::SagittariusAStar,
        ScenarioPreset::TidalDisruptionEvent,
        ScenarioPreset::RelativisticBinary,
        ScenarioPreset::LittleRedDot,
    ];

    // Body types to test: Planet, Moon, Embryo (Protoplanet), Planetesimal
    let target_body_types = [
        (BodyType::TerrestrialPlanet, "Target Planet"),
        (BodyType::Moon, "Target Moon"),
        (BodyType::Protoplanet, "Target Embryo"),
        (BodyType::Planetesimal, "Target Planetesimal"),
    ];

    for preset in test_presets {
        let sc = ActiveScenarioState {
            current_preset: preset,
            ..Default::default()
        };
        app.insert_resource(sc);

        for (b_type, name) in target_body_types {
            let target_ent = app
                .world_mut()
                .spawn((
                    CelestialBody {
                        name: name.to_string(),
                        body_type: b_type,
                    },
                    Mass(0.0001),
                    Radius(0.0001),
                    SimPosition(bevy::math::DVec3::new(10.0, 0.0, 0.0)),
                    SimVelocity(bevy::math::DVec3::new(0.0, 0.5, 0.0)),
                    Composition::rocky(),
                    Temperature(200.0),
                ))
                .id();

            // Select this body
            app.world_mut()
                .resource_mut::<PlayerInteractionState>()
                .selected_entity = Some(target_ent);
            app.update();

            // Verify Bombardment section is available and visible
            let display = app.world().get::<Node>(sec_bombardment).unwrap().display;
            assert_eq!(
                display,
                Display::Flex,
                "Terraforming & Bombardment section must be visible in {preset:?} for {b_type:?}"
            );

            // Clean up target
            app.world_mut().despawn(target_ent);
        }
    }
}

#[test]
fn test_gmc_embryo_bombardment_launch_and_impact_delivery() {
    use protostellar::game::ui::interactions::editor::terraforming_actions::handle_bombardment_trigger;
    use protostellar::game::ui::types::*;
    use protostellar::simulation::terraforming::{
        BombardmentEvent, BombardmentProjectile, BombardmentType,
        update_guided_bombardment_projectiles,
    };

    let mut app = App::new();
    app.init_resource::<PlayerInteractionState>();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    app.init_resource::<NotificationToast>();
    app.add_message::<BombardmentEvent>();

    // Test launching all 4 bombardment weapons at an Embryo in GMC scenario
    let sc = ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        ..Default::default()
    };
    app.insert_resource(sc);

    let embryo_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "GMC Cluster Embryo-IV".to_string(),
                body_type: BodyType::Protoplanet,
            },
            Mass(0.0001),
            Radius(0.0001),
            SimPosition(bevy::math::DVec3::new(15.0, 0.0, 0.0)),
            SimVelocity(bevy::math::DVec3::new(0.0, 0.8, 0.0)),
            Composition::rocky(),
            Temperature(180.0),
        ))
        .id();

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(embryo_ent);

    let weapons = [
        (UiButtonAction::BombardComet, BombardmentType::IcyComet),
        (
            UiButtonAction::BombardChondrite,
            BombardmentType::CarbonaceousChondrite,
        ),
        (
            UiButtonAction::BombardSalvo,
            BombardmentType::VolatileAblationSalvo,
        ),
        (UiButtonAction::BombardCore, BombardmentType::IronAsteroid),
    ];

    let mut toast = NotificationToast::default();

    for (action, expected_type) in weapons {
        let mut state: bevy::ecs::system::SystemState<(
            Commands,
            SelectedWorldQuery,
            Res<PlayerInteractionState>,
        )> = bevy::ecs::system::SystemState::new(app.world_mut());
        let (mut commands, mut query, player) = state.get_mut(app.world_mut()).unwrap();

        handle_bombardment_trigger(
            &action,
            &mut query,
            &player,
            &mut commands,
            100.0,
            &mut toast,
        );
        state.apply(app.world_mut());

        assert!(
            toast.message.contains(expected_type.name()),
            "Toast must announce bombardment launch for {expected_type:?}"
        );

        // Find the spawned guided bombardment projectile
        let mut proj_query = app
            .world_mut()
            .query::<(&BombardmentProjectile, &CelestialBody)>();
        let spawned = proj_query
            .iter(app.world())
            .find(|(p, _)| p.bombardment_type == expected_type && p.target_entity == embryo_ent);

        assert!(
            spawned.is_some(),
            "Guided projectile for {expected_type:?} must be spawned targeting embryo"
        );
    }

    // Move projectile to target contact distance to trigger impact resolution
    for (mut p_pos, _) in app
        .world_mut()
        .query::<(&mut SimPosition, &BombardmentProjectile)>()
        .iter_mut(app.world_mut())
    {
        p_pos.0 = bevy::math::DVec3::new(15.0, 0.0, 0.0);
    }

    // Advance simulation to trigger guided impact navigation and impact resolution
    app.add_systems(Update, update_guided_bombardment_projectiles);
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 0.01;
        sim_time.elapsed_years = 100.01; // Past arrival time
    }
    app.update();

    // Embryo should have received volatiles/inventory from the impacts
    let vol = app.world().get::<VolatileInventory>(embryo_ent);
    assert!(
        vol.is_some(),
        "Target embryo must have received volatile inventory from bombardment impacts"
    );
}

#[test]
fn test_gmc_sorted_nearest_star_3d_z_offset_and_tied_x() {
    let mut app = App::new();
    app.init_resource::<SimulationConfig>();
    app.init_resource::<TimeWarp>();
    app.init_resource::<SimTime>();
    let scenario_state = ActiveScenarioState {
        current_preset: ScenarioPreset::MolecularCloudCluster,
        ..Default::default()
    };
    app.insert_resource(scenario_state);

    let disk_inactive = CircumstellarDisk {
        inner_radius_au: 0.05,
        outer_radius_au: 20.0,
        disk_mass_solar: 0.001,
        is_active_full_disk: false,
        age_years: 90_000.0,
        lifetime_years: 80_000.0,
    };
    let disk_active = CircumstellarDisk {
        is_active_full_disk: true,
        age_years: 1_000.0,
        ..disk_inactive
    };

    app.world_mut().spawn((
        CelestialBody {
            name: "Star-X0-Inactive".to_string(),
            body_type: BodyType::Protostar,
        },
        Mass(1.5),
        SimPosition(bevy::math::DVec3::ZERO),
        SimVelocity(bevy::math::DVec3::ZERO),
        Radius(0.01),
        Composition::solar_gas(),
        disk_inactive,
    ));
    app.world_mut().spawn((
        CelestialBody {
            name: "Star-ZFar-Active".to_string(),
            body_type: BodyType::Protostar,
        },
        Mass(1.5),
        SimPosition(bevy::math::DVec3::new(10.0, 0.0, 120.0)),
        SimVelocity(bevy::math::DVec3::ZERO),
        Radius(0.01),
        Composition::solar_gas(),
        disk_active,
    ));
    // Two stars sharing identical X=20.0; active star is near Z=10 (Y=0), inactive is at Y=50.
    app.world_mut().spawn((
        CelestialBody {
            name: "Star-TiedX-FarY-Inactive".to_string(),
            body_type: BodyType::Protostar,
        },
        Mass(1.5),
        SimPosition(bevy::math::DVec3::new(20.0, 50.0, 10.0)),
        SimVelocity(bevy::math::DVec3::ZERO),
        Radius(0.01),
        Composition::solar_gas(),
        disk_inactive,
    ));
    app.world_mut().spawn((
        CelestialBody {
            name: "Star-TiedX-Near3D-Active".to_string(),
            body_type: BodyType::Protostar,
        },
        Mass(1.5),
        SimPosition(bevy::math::DVec3::new(20.0, 0.0, 10.0)),
        SimVelocity(bevy::math::DVec3::ZERO),
        Radius(0.01),
        Composition::solar_gas(),
        disk_active,
    ));

    let p_near_inactive = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Planet-Near-Inactive".to_string(),
                body_type: BodyType::Protoplanet,
            },
            Mass(0.000_002),
            SimPosition(bevy::math::DVec3::new(1.5, 0.0, 0.5)),
            SimVelocity(bevy::math::DVec3::ZERO),
            Radius(0.0001),
            Composition::solar_gas(),
            VolatileInventory::default(),
        ))
        .id();

    let p_near_active = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Planet-TiedX-NearActive".to_string(),
                body_type: BodyType::Protoplanet,
            },
            Mass(0.000_002),
            SimPosition(bevy::math::DVec3::new(20.0, 1.0, 11.2)),
            SimVelocity(bevy::math::DVec3::ZERO),
            Radius(0.0001),
            Composition::solar_gas(),
            VolatileInventory::default(),
        ))
        .id();

    app.add_systems(Update, gmc_cluster_gas_accretion);
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 1.0;
    app.update();

    let m_inactive = app.world().get::<Mass>(p_near_inactive).unwrap().0;
    let m_active = app.world().get::<Mass>(p_near_active).unwrap().0;

    assert_eq!(
        m_inactive, 0.000_002,
        "Protoplanet closest in 3D to inactive X0 star must not accrete from active stars farther in Z/X"
    );
    assert!(
        m_active > 0.000_002,
        "Protoplanet with tied X must pick the closest 3D active star and accrete mass (got {m_active})"
    );
}
