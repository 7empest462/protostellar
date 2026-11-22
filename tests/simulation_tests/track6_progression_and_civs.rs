//! Integration tests for Track 6A: 12-node Tech Tree, Progression Mode gating, Ranks, and Living Civilizations.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::game::phases::{GlobalTrophies, MilestoneId, PhaseManager};
use protostellar::game::ui::interactions::editor::terraforming_actions::{
    handle_bombardment_trigger, handle_shatter_and_life,
};
use protostellar::game::ui::types::{NotificationToast, SelectedWorldQuery, UiButtonAction};
use protostellar::rendering::effects::megastructures::DysonSwarm;
use protostellar::simulation::biology::{
    CivilizationMilestoneEvent, CivilizationState, ColonyFoundedEvent, evolve_civilizations_system,
    sync_progression_and_trophies_system, technosignature_from_kardashev,
};
use protostellar::simulation::components::*;
use protostellar::simulation::economy::{
    ActionAccessResult, GameMode, ProgressionRank, ProgressionState, ScienceData,
    TechnologyCategory, TechnologyId, UnlockedTechs, harvest_telemetry_system,
};
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::ScenarioReloadGeneration;
use protostellar::simulation::terraforming::{
    BombardmentEvent, BombardmentProjectile, BombardmentType, TerraformingAtmosphere,
    update_guided_bombardment_projectiles,
};
use protostellar::utils::constants::EARTH_MASS_SOLAR;

pub(super) fn spawn_inhabited_world(
    world: &mut World,
    name: &str,
    pos_au: DVec3,
    kardashev: f32,
) -> Entity {
    let tech = technosignature_from_kardashev(kardashev);
    let mut civ = CivilizationState::from_technosignature(tech);
    civ.kardashev_level = kardashev;
    world
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: name.to_string(),
            },
            Mass(EARTH_MASS_SOLAR),
            Radius(0.0005),
            SimPosition(pos_au),
            SimVelocity(DVec3::new(0.0, 0.0, std::f64::consts::TAU)),
            Temperature(288.0),
            Luminosity(0.0),
            Composition::rocky(),
            SpinState::default(),
            VolatileInventory {
                delivered_water_m_earth: 0.002,
                ocean_coverage_frac: 0.70,
                atmospheric_pressure_bar: 1.0,
                cometary_impact_count: 8,
            },
            PlanetaryClimate {
                surface_temperature_k: 288.0,
                equilibrium_temperature_k: 255.0,
                greenhouse_delta_k: 33.0,
                albedo: 0.30,
                ice_coverage_frac: 0.10,
                cloud_coverage_frac: 0.55,
                climate_regime: ClimateRegime::TemperateHabitable,
                polar_ice_cap_latitude_deg: 75.0,
            },
            BiosphereState {
                habitability_score: 0.95,
                biomass_coverage_frac: 0.85,
                oxygen_fraction: 0.21,
                technosignature: tech,
                bioluminescence_intensity: 0.35,
                ..Default::default()
            },
            TerraformingAtmosphere::default(),
            civ,
        ))
        .id()
}

fn spawn_habitable_uncolonized_world(world: &mut World, name: &str, pos_au: DVec3) -> Entity {
    world
        .spawn((
            CelestialBody {
                body_type: BodyType::TerrestrialPlanet,
                name: name.to_string(),
            },
            Mass(EARTH_MASS_SOLAR * 0.95),
            Radius(0.00048),
            SimPosition(pos_au),
            SimVelocity(DVec3::new(0.0, 0.0, 4.5)),
            Temperature(285.0),
            Luminosity(0.0),
            Composition::rocky(),
            SpinState::default(),
            VolatileInventory {
                delivered_water_m_earth: 0.0015,
                ocean_coverage_frac: 0.55,
                atmospheric_pressure_bar: 0.85,
                cometary_impact_count: 5,
            },
            PlanetaryClimate {
                surface_temperature_k: 285.0,
                equilibrium_temperature_k: 255.0,
                greenhouse_delta_k: 30.0,
                albedo: 0.30,
                ice_coverage_frac: 0.15,
                cloud_coverage_frac: 0.45,
                climate_regime: ClimateRegime::TemperateHabitable,
                polar_ice_cap_latitude_deg: 72.0,
            },
            BiosphereState {
                habitability_score: 0.85,
                biomass_coverage_frac: 0.40,
                oxygen_fraction: 0.16,
                technosignature: 0.0,
                bioluminescence_intensity: 0.15,
                ..Default::default()
            },
            TerraformingAtmosphere::default(),
        ))
        .id()
}

#[test]
fn test_12_node_tech_tree_access_and_ranks() {
    let nodes = UnlockedTechs::all_nodes();
    assert_eq!(nodes.len(), 12);
    let def_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| n.category == TechnologyCategory::PlanetaryDefense)
        .collect();
    assert_eq!(def_nodes.len(), 3);

    let mut prog = ProgressionState {
        mode: GameMode::Sandbox,
        ..Default::default()
    };
    let mut techs = UnlockedTechs::default();
    let mut science = ScienceData {
        points: 0.0,
        lifetime_points: 0.0,
        ..Default::default()
    };

    assert_eq!(
        prog.check_action_access(
            Some(TechnologyId::KardashevAscension),
            200.0,
            Some(&techs),
            Some(&science)
        ),
        ActionAccessResult::Allowed
    );

    prog.mode = GameMode::Progression;
    assert!(matches!(
        prog.check_action_access(
            Some(TechnologyId::KardashevAscension),
            200.0,
            Some(&techs),
            Some(&science),
        ),
        ActionAccessResult::Locked(_)
    ));

    techs.unlocked.insert(TechnologyId::KardashevAscension);
    assert!(matches!(
        prog.check_action_access(
            Some(TechnologyId::KardashevAscension),
            200.0,
            Some(&techs),
            Some(&science),
        ),
        ActionAccessResult::Locked(_)
    ));

    science.add_science(250.0);
    assert_eq!(
        prog.check_action_access(
            Some(TechnologyId::KardashevAscension),
            200.0,
            Some(&techs),
            Some(&science),
        ),
        ActionAccessResult::Allowed
    );
    assert!(prog.consume_progression_cost(200.0, Some(&mut science)));
    assert!((science.points - 50.0).abs() < 1e-6);

    let mut rank_prog = ProgressionState::default();
    assert_eq!(
        rank_prog.compute_rank(0, 0),
        ProgressionRank::PrimordialObserver
    );
    assert_eq!(
        rank_prog.compute_rank(1, 0),
        ProgressionRank::DeepSpaceExplorer
    );
    rank_prog.active_civilization_count = 1;
    assert_eq!(
        rank_prog.compute_rank(1, 0),
        ProgressionRank::PlanetarySteward
    );
    rank_prog.crises_averted = 1;
    assert_eq!(
        rank_prog.compute_rank(1, 0),
        ProgressionRank::CrisisDefender
    );
    rank_prog.highest_kardashev = 1.60;
    assert_eq!(
        rank_prog.compute_rank(1, 0),
        ProgressionRank::KardashevArchitect
    );
    rank_prog.highest_kardashev = 2.05;
    rank_prog.crises_averted = 2;
    rank_prog.colony_world_count = 1;
    assert_eq!(
        rank_prog.compute_rank(10, 0),
        ProgressionRank::GalacticCustodian
    );
}

type GateActionState<'w, 's> = bevy::ecs::system::SystemState<(
    Commands<'w, 's>,
    SelectedWorldQuery<'w, 's>,
    Query<
        'w,
        's,
        (
            &'static mut CivilizationState,
            Option<&'static mut BiosphereState>,
        ),
    >,
    Res<'w, PlayerInteractionState>,
    Res<'w, ProgressionState>,
    Res<'w, UnlockedTechs>,
    ResMut<'w, ScienceData>,
    ResMut<'w, NotificationToast>,
)>;

type UpliftActionState<'w, 's> = bevy::ecs::system::SystemState<(
    Commands<'w, 's>,
    SelectedWorldQuery<'w, 's>,
    Query<
        'w,
        's,
        (
            &'static mut CivilizationState,
            Option<&'static mut BiosphereState>,
        ),
    >,
    Res<'w, PlayerInteractionState>,
    ResMut<'w, NotificationToast>,
)>;

#[test]
fn test_progression_mode_gates_editor_actions() {
    let mut app = App::new();
    app.init_resource::<PlayerInteractionState>()
        .init_resource::<ProgressionState>()
        .init_resource::<UnlockedTechs>()
        .init_resource::<ScienceData>()
        .init_resource::<NotificationToast>();
    let target = spawn_habitable_uncolonized_world(app.world_mut(), "Target", DVec3::X);
    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(target);
    app.world_mut().resource_mut::<ProgressionState>().mode = GameMode::Progression;
    app.world_mut().resource_mut::<ScienceData>().points = 20.0;

    let mut sys = GateActionState::new(app.world_mut());

    let (mut commands, mut sel_q, mut civ_q, player, prog_r, techs_r, mut sci_r, mut toast) =
        sys.get_mut(app.world_mut()).unwrap();
    let handled = handle_shatter_and_life(
        &UiButtonAction::SeedLife,
        &mut sel_q,
        &mut civ_q,
        &player,
        &mut commands,
        Some(&prog_r),
        Some(&techs_r),
        Some(&mut sci_r),
        10.0,
        &mut toast,
    );
    assert!(handled);
    assert!(toast.message.contains("Progression Mode"));

    handle_bombardment_trigger(
        &UiButtonAction::BombardComet,
        &mut sel_q,
        &player,
        &mut commands,
        Some(&prog_r),
        Some(&techs_r),
        Some(&mut sci_r),
        10.0,
        &mut toast,
    );
    assert!(toast.message.contains("Progression Mode"));
}

#[test]
fn test_tech_effects_harvest_and_orbital_mirrors() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<TimeWarp>()
        .init_resource::<ScienceData>()
        .init_resource::<UnlockedTechs>()
        .init_resource::<ProgressionState>()
        .init_resource::<ScenarioReloadGeneration>()
        .add_systems(Update, harvest_telemetry_system);

    app.world_mut().spawn(BiosphereState {
        technosignature: 1.0,
        ..Default::default()
    });
    app.world_mut().spawn(DysonSwarm {
        completion_fraction: 0.40,
        ..Default::default()
    });

    app.update();
    let baseline_rate = app.world().resource::<ScienceData>().passive_rate_per_sec;
    assert!((baseline_rate - (2.5 + 0.40 * 12.0)).abs() < 1e-5);

    {
        let mut unlocked = app.world_mut().resource_mut::<UnlockedTechs>();
        unlocked
            .unlocked
            .insert(TechnologyId::GravitationalLensArray);
        unlocked.unlocked.insert(TechnologyId::StellarEngine);
    }
    app.update();
    let boosted_rate = app.world().resource::<ScienceData>().passive_rate_per_sec;
    let expected = 2.5 * 1.4 + 0.40 * 12.0 + 25.0;
    assert!(
        (boosted_rate - expected).abs() < 1e-5,
        "Expected {expected}, got {boosted_rate}"
    );

    let mut m_app = App::new();
    m_app
        .init_resource::<Time>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<UnlockedTechs>()
        .add_message::<CivilizationMilestoneEvent>()
        .add_message::<ColonyFoundedEvent>()
        .add_systems(Update, evolve_civilizations_system);
    m_app.world_mut().resource_mut::<SimTime>().current_dt_yr = 2.0;
    m_app
        .world_mut()
        .resource_mut::<UnlockedTechs>()
        .unlocked
        .insert(TechnologyId::OrbitalMirrorArray);
    let hot_world = spawn_inhabited_world(m_app.world_mut(), "MirrorWorld", DVec3::X, 0.80);
    m_app
        .world_mut()
        .entity_mut(hot_world)
        .get_mut::<PlanetaryClimate>()
        .unwrap()
        .surface_temperature_k = 420.0;
    m_app.update();
    let cooled = m_app
        .world()
        .entity(hot_world)
        .get::<PlanetaryClimate>()
        .unwrap()
        .surface_temperature_k;
    assert!(
        cooled < 419.0,
        "OrbitalMirrorArray must cool overheated world, got {cooled}"
    );
}

#[test]
fn test_tech_effects_bombardment_delivery_multipliers() {
    let run_bombard = |bomb_type: BombardmentType, with_techs: bool| -> (f64, f64) {
        let mut b_app = App::new();
        b_app
            .init_resource::<Time>()
            .init_resource::<SimTime>()
            .init_resource::<TimeWarp>()
            .init_resource::<SimulationConfig>()
            .init_resource::<UnlockedTechs>()
            .add_message::<BombardmentEvent>()
            .add_systems(Update, update_guided_bombardment_projectiles);

        if with_techs {
            let mut ut = b_app.world_mut().resource_mut::<UnlockedTechs>();
            ut.unlocked.insert(TechnologyId::AtmosphericInjection);
            ut.unlocked.insert(TechnologyId::CometRedirection);
        }

        let target = b_app
            .world_mut()
            .spawn((
                CelestialBody {
                    body_type: BodyType::TerrestrialPlanet,
                    name: "Terra".to_string(),
                },
                Mass(EARTH_MASS_SOLAR),
                Radius(0.0005),
                SimPosition(DVec3::X),
                SimVelocity(DVec3::ZERO),
                Temperature(260.0),
                Composition::rocky(),
                VolatileInventory::default(),
                TerraformingAtmosphere::default(),
                PlanetaryClimate::default(),
                InternalDifferentiation::default(),
            ))
            .id();

        b_app.world_mut().spawn((
            CelestialBody {
                body_type: BodyType::Comet,
                name: "Impactor".to_string(),
            },
            Mass(EARTH_MASS_SOLAR * 0.001),
            Radius(0.00005),
            SimPosition(DVec3::X + DVec3::new(0.0001, 0.0, 0.0)),
            SimVelocity(DVec3::new(-4.0, 0.0, 0.0)),
            Composition {
                ice_frac: 0.70,
                silicate_frac: 0.20,
                gas_frac: 0.10,
                ..Default::default()
            },
            BombardmentProjectile {
                target_entity: target,
                bombardment_type: bomb_type,
                launch_time_yr: 0.0,
                expected_arrival_yr: 0.0,
                trail_color: Color::WHITE,
                is_detonated: false,
                relative_offset: DVec3::new(0.0001, 0.0, 0.0),
                initial_distance: 0.01,
                flight_duration_secs: 0.1,
                flight_elapsed_secs: 0.2,
            },
        ));

        b_app.update();
        let vol = b_app
            .world()
            .entity(target)
            .get::<VolatileInventory>()
            .unwrap();
        let terra = b_app
            .world()
            .entity(target)
            .get::<TerraformingAtmosphere>()
            .unwrap();
        (
            vol.delivered_water_m_earth,
            f64::from(terra.co2_pressure_bar),
        )
    };

    let (base_water, _) = run_bombard(BombardmentType::IcyComet, false);
    let (boosted_water, _) = run_bombard(BombardmentType::IcyComet, true);
    assert!(
        boosted_water > base_water * 1.65,
        "CometRedirection should boost water 1.75x: base={base_water}, boosted={boosted_water}"
    );

    let default_co2 = f64::from(TerraformingAtmosphere::default().co2_pressure_bar);
    let (_, base_co2) = run_bombard(BombardmentType::CarbonaceousChondrite, false);
    let (_, boosted_co2) = run_bombard(BombardmentType::CarbonaceousChondrite, true);
    let base_delta_co2 = (base_co2 - default_co2).max(0.0);
    let boosted_delta_co2 = (boosted_co2 - default_co2).max(0.0);
    assert!(
        (boosted_delta_co2 - base_delta_co2 * 2.0).abs() < 1e-6,
        "AtmosphericInjection should double delta CO2: base_delta={base_delta_co2}, boosted_delta={boosted_delta_co2}"
    );
}

#[test]
fn test_civilization_evolution_autonomous_colonies_uplift_and_trophies() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<UnlockedTechs>()
        .init_resource::<ProgressionState>()
        .init_resource::<ScienceData>()
        .init_resource::<NotificationToast>()
        .init_resource::<GlobalTrophies>()
        .init_resource::<PhaseManager>()
        .init_resource::<PlayerInteractionState>()
        .add_message::<CivilizationMilestoneEvent>()
        .add_message::<ColonyFoundedEvent>()
        .add_systems(
            Update,
            (
                evolve_civilizations_system,
                sync_progression_and_trophies_system.after(evolve_civilizations_system),
            ),
        );
    app.world_mut().resource_mut::<SimTime>().current_dt_yr = 5.0;

    let home = spawn_inhabited_world(app.world_mut(), "Homeworld", DVec3::X, 1.35);
    {
        let mut home_mut = app.world_mut().entity_mut(home);
        let mut h_civ = home_mut.get_mut::<CivilizationState>().unwrap();
        h_civ.colony_cooldown_years = 0.0;
        h_civ.stability = 0.90;
        h_civ.great_filter_stress = 0.10;
    }
    let colony_target =
        spawn_habitable_uncolonized_world(app.world_mut(), "New Mars", DVec3::new(1.5, 0.0, 0.0));
    app.world_mut().spawn((
        CentralStar,
        DysonSwarm {
            completion_fraction: 0.55,
            ..Default::default()
        },
    ));
    app.world_mut()
        .resource_mut::<ProgressionState>()
        .crises_averted = 1;

    app.update();

    let colony_civ = app
        .world()
        .entity(colony_target)
        .get::<CivilizationState>()
        .expect("Autonomous colony must be founded on habitable sibling world");
    assert!(colony_civ.is_colony);

    let trophies = app.world().resource::<GlobalTrophies>();
    assert!(
        trophies
            .unlocked
            .contains(&MilestoneId::CivilizationTypeOne)
    );
    assert!(trophies.unlocked.contains(&MilestoneId::DysonSwarmTypeTwo));
    assert!(trophies.unlocked.contains(&MilestoneId::CrisisDeflected));
    assert!(
        trophies
            .unlocked
            .contains(&MilestoneId::InterplanetaryColony)
    );
    assert!(
        trophies
            .unlocked
            .contains(&MilestoneId::GreatFilterOvercome)
    );

    app.world_mut()
        .resource_mut::<PlayerInteractionState>()
        .selected_entity = Some(colony_target);
    let mut sys = UpliftActionState::new(app.world_mut());

    let before_k = app
        .world()
        .entity(colony_target)
        .get::<CivilizationState>()
        .unwrap()
        .kardashev_level;
    {
        let (mut commands, mut sel_q, mut civ_q, player, mut toast) =
            sys.get_mut(app.world_mut()).unwrap();
        handle_shatter_and_life(
            &UiButtonAction::AdvanceCivilization,
            &mut sel_q,
            &mut civ_q,
            &player,
            &mut commands,
            None,
            None,
            None,
            10.0,
            &mut toast,
        );
    }
    let after_k = app
        .world()
        .entity(colony_target)
        .get::<CivilizationState>()
        .unwrap()
        .kardashev_level;
    assert!(after_k > before_k, "Uplift must advance Kardashev tier");
}
