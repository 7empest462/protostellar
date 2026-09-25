use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::*;
use protostellar::simulation::scenarios::{spawn_accretion_disk_genesis, ScenarioPreset};
use protostellar::utils::constants::*;

#[test]
fn test_accretion_disk_genesis_spawner_zero_starter_planets() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();

    let star_entity = spawn_accretion_disk_genesis(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    // 1. Verify disk parameters are set for a young Class II disk with 3.65 AU snow line
    assert_eq!(disk_params.central_star_mass, 1.33);
    assert_eq!(disk_params.inner_radius_au, 0.12);
    assert_eq!(disk_params.outer_radius_au, 55.0);
    assert_eq!(disk_params.snow_line_au, 3.65);
    assert_eq!(disk_params.disk_mass, 0.00035);
    assert_eq!(disk_params.reference_temp_1au, 325.0);

    // 2. Verify central protostar properties
    let star_body = app.world().get::<CelestialBody>(star_entity).expect("Star must exist");
    assert!(
        star_body.name.contains("Genesis Protostar"),
        "Central star must be designated as Genesis Protostar (got {})",
        star_body.name
    );
    let star_mass = app.world().get::<Mass>(star_entity).expect("Mass must exist");
    assert_eq!(star_mass.0, 1.33, "Genesis protostar mass must be 1.33 Solar Masses");
    assert!(app.world().get::<CentralStar>(star_entity).is_some());
    let ignition = app.world().get::<IgnitionState>(star_entity).expect("IgnitionState must exist");
    assert!(
        !ignition.is_ignited,
        "Young Class II protostar must start in pre-main-sequence un-ignited phase"
    );

    // 3. CRITICAL REQUIREMENT: Exactly ZERO starter planets or embryos exist at inception
    let mut bodies_query = app.world_mut().query_filtered::<Entity, (With<CelestialBody>, Without<CentralStar>)>();
    let non_star_count = bodies_query.iter(app.world()).count();
    assert_eq!(
        non_star_count, 0,
        "Genesis scenario must start with 0 planets/embryos in ECS (found {non_star_count})"
    );
}

#[test]
fn test_snow_line_thermodynamic_location() {
    let mut disk_params = DiskParameters::default();
    disk_params.reference_temp_1au = 325.0;
    disk_params.snow_line_au = 3.65;

    // Thermodynamic water ice sublimation temperature in protostellar disks is ~170 K
    let temp_at_snow_line = disk_params.reference_temp_1au * (disk_params.snow_line_au / 1.0).powf(-0.5);
    assert!(
        (temp_at_snow_line - 170.1).abs() < 1.0,
        "Temperature at 3.65 AU snow line must be ~170 K for water ice condensation (got {:.1} K)",
        temp_at_snow_line
    );

    // Inside snow line (e.g. 1.0 AU Earth orbit) is warm dry silicates (> 270 K)
    let temp_1au = disk_params.reference_temp_1au * (1.0f64 / 1.0).powf(-0.5);
    assert!(temp_1au >= 270.0);

    // Outside snow line (e.g. 5.2 AU orbit) is cold volatile ice (< 170 K water ice freeze-out)
    let temp_5au = disk_params.reference_temp_1au * (5.2f64 / 1.0).powf(-0.5);
    assert!(temp_5au < 170.0);

    // Deep outer disk (e.g. 10.0 AU orbit) is deep cryogenic ice (< 110 K)
    let temp_10au = disk_params.reference_temp_1au * (10.0f64 / 1.0).powf(-0.5);
    assert!(temp_10au < 110.0);
}

#[test]
fn test_sph_gas_drag_and_snow_line_pressure_trap() {
    use protostellar::rendering::particle_swarm::compute_gas_aerodynamic_drift;

    let snow_line_au = 2.70f32;
    let b_mass = 1e-6f32;
    let m = b_mass;
    let gas_scale = 1.0f32;

    // Keplerian speed helper
    let v_k = |r: f32| (G_ASTRO as f32 * 1.0 / r).sqrt();

    // 1. Far outer disk (10 AU): sub-Keplerian headwind drives inward aerodynamic drift (v_drift < 0)
    let v_10 = compute_gas_aerodynamic_drift(10.0, v_k(10.0), m, b_mass, snow_line_au, gas_scale);
    assert!(
        v_10 < 0.0,
        "Outer disk dust must spiral inward toward star due to aerodynamic headwind (v_drift = {v_10})"
    );

    // 2. Far inner disk (0.8 AU): dry silicates drift inward toward star (v_drift < 0)
    let v_inner = compute_gas_aerodynamic_drift(0.8, v_k(0.8), m, b_mass, snow_line_au, gas_scale);
    assert!(
        v_inner < 0.0,
        "Inner disk dust must spiral inward toward star (v_drift = {v_inner})"
    );

    // 3. At the snow line condensation front (2.70 AU): cold-finger pressure bump halts inward drift (v_drift >= 0)
    let v_trap = compute_gas_aerodynamic_drift(2.70, v_k(2.70), m, b_mass, snow_line_au, gas_scale);
    assert!(
        v_trap >= 0.0,
        "Cold-finger condensation front must halt inward drift (v_drift = {v_trap} >= 0) to trap dust and pebbles into dense planetesimal-forming ring"
    );

    // 4. At the inner silicate sublimation front (~0.35 AU): inner pressure bump halts rapid drain into star (v_drift >= 0)
    let v_inner_trap = compute_gas_aerodynamic_drift(0.35, v_k(0.35), m, b_mass, snow_line_au, gas_scale);
    assert!(
        v_inner_trap >= 0.0,
        "Inner silicate sublimation trap must halt inward drift (v_drift = {v_inner_trap} >= 0) to retain inner disk particles"
    );
}

#[test]
fn test_genesis_protostar_retains_inner_disk_without_premature_ignition() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let star_entity = spawn_accretion_disk_genesis(&mut app.world_mut().commands(), &mut disk_params);

    app.init_resource::<TimeWarp>();
    app.insert_resource(SimTime {
        elapsed_years: 100.0, // 100 years into simulation
        current_dt_yr: 1.0,
        ..Default::default()
    });
    app.insert_resource(SimulationConfig {
        enable_thermodynamics: true,
        gas_density_scale: 1.0,
        ..Default::default()
    });
    app.add_message::<protostellar::simulation::thermodynamics::StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();

    app.add_systems(Update, protostellar::simulation::thermodynamics::update_thermodynamics);
    app.update();

    let star_body = app.world().get::<CelestialBody>(star_entity).expect("Star must exist");
    assert!(
        star_body.name.contains("Genesis Protostar"),
        "Genesis protostar name must be retained during disk epoch (got {})",
        star_body.name
    );

    let ignition = app.world().get::<IgnitionState>(star_entity).expect("Ignition must exist");
    assert!(
        !ignition.is_ignited,
        "Genesis protostar must NOT auto-ignite after 100 yr"
    );
    assert_eq!(
        ignition.shockwave_radius, 0.0,
        "Shockwave radius must remain 0.0 to protect inner disk gas and particles"
    );

    let config = app.world().resource::<SimulationConfig>();
    assert_eq!(
        config.gas_density_scale, 1.0,
        "Gas density scale must remain intact at 1.0 during disk era"
    );
}

#[test]
fn test_inner_terrestrial_clump_promotion_threshold() {
    use protostellar::rendering::particle_swarm::check_clump_promotions;
    use protostellar::rendering::particle_swarm::ParticleSwarmData;

    let b_mass = 1e-10f32;
    let mut data = ParticleSwarmData {
        positions: vec![[1.0, 0.0, 0.0]],
        velocities: vec![[0.0, 0.0, 6.28]],
        masses: vec![(0.0025 * EARTH_MASS_SOLAR) as f32],
        compositions: vec![Composition::rocky()],
        temperatures: vec![300.0],
        colors: vec![[1.0, 0.8, 0.6, 1.0]],
        mesh_positions: vec![[0.0; 3]; 4],
        mesh_colors: vec![[1.0; 4]; 4],
        bin_heads: vec![-1; 4096],
        bin_next: vec![-1; 1],
        mesh_handle: Handle::default(),
        count: 1,
        base_mass: b_mass,
        is_dirty: false,
        pending_gpu_accretions: Vec::new(),
    };
    let disk_params = DiskParameters {
        snow_line_au: 3.65,
        ..Default::default()
    };

    let (promotions, _) = check_clump_promotions(
        &mut data,
        &disk_params,
        0,
        false,
        25.0, // After early settling
    );

    assert_eq!(
        promotions.len(), 1,
        "Inner disk clump at 1.0 AU (0.0025 M_earth) must successfully promote"
    );
    assert!(
        (promotions[0].0.length() - 1.0).abs() < 0.1,
        "Promoted body must be in the inner solar system (~1.0 AU)"
    );
}

#[test]
fn test_clump_promotions_to_planetesimals_and_embryos() {
    // Promotion of a kilometer-scale planetesimal at the snow line
    let mass_planetesimal = 0.005 * EARTH_MASS_SOLAR;
    let comp_snowline = Composition::carbonaceous();

    let body_type_planetesimal = protostellar::simulation::components::classify_body_by_mass_and_comp(
        mass_planetesimal,
        &comp_snowline,
        false,
    );
    assert_eq!(
        body_type_planetesimal,
        BodyType::Planetesimal,
        "0.005 M_earth clump must classify as Planetesimal"
    );

    // Promotion of a Moon-to-Mars mass protoplanetary embryo
    let mass_embryo = 0.05 * EARTH_MASS_SOLAR;
    let body_type_embryo = protostellar::simulation::components::classify_body_by_mass_and_comp(
        mass_embryo,
        &comp_snowline,
        false,
    );
    assert_eq!(
        body_type_embryo,
        BodyType::Protoplanet,
        "0.05 M_earth clump must classify as Protoplanet"
    );

    // Full mature planet after mutual N-body collisions
    let mass_planet = 1.0 * EARTH_MASS_SOLAR;
    let comp_rocky = Composition::rocky();
    let body_type_planet = protostellar::simulation::components::classify_body_by_mass_and_comp(
        mass_planet,
        &comp_rocky,
        false,
    );
    assert_eq!(
        body_type_planet,
        BodyType::TerrestrialPlanet,
        "1.0 M_earth merger must classify as TerrestrialPlanet"
    );
}

#[test]
fn test_genesis_scenario_preset_metadata() {
    let preset = ScenarioPreset::AccretionDiskGenesis;
    assert_eq!(preset.display_name(), "Disk Genesis (Organic Planet Formation)");
    assert!(preset.description().contains("SPH viscous gas"));
    assert!(preset.description().contains("snow line"));
    assert!(preset.description().contains("starter planets"));
}

#[test]
fn test_genesis_protostar_auto_ignites_at_100_percent_core_heating() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let star_entity = spawn_accretion_disk_genesis(&mut app.world_mut().commands(), &mut disk_params);

    app.init_resource::<TimeWarp>();
    app.insert_resource(SimTime {
        elapsed_years: 140_000.0, // ~140 kyr into Genesis simulation
        current_dt_yr: 1.0,
        ..Default::default()
    });
    app.insert_resource(SimulationConfig {
        enable_thermodynamics: true,
        gas_density_scale: 1.0,
        ..Default::default()
    });
    app.add_message::<protostellar::simulation::thermodynamics::StarIgnitionEvent>();
    app.add_message::<PlanetaryEngulfmentEvent>();
    app.add_message::<SupernovaEvent>();

    // Set core temperature right at ignition threshold (10.0 MK)
    {
        let world = app.world_mut();
        let mut ignition = world.get_mut::<IgnitionState>(star_entity).expect("Ignition must exist");
        ignition.core_temperature = 1.0e7;
    }

    app.add_systems(Update, protostellar::simulation::thermodynamics::update_thermodynamics);
    app.update();

    let ignition = app.world().get::<IgnitionState>(star_entity).expect("Ignition must exist");
    assert!(
        ignition.is_ignited,
        "Genesis protostar MUST auto-ignite when core temperature reaches 10.0 MK (100% heating)"
    );
    assert_eq!(
        ignition.fusion_fraction, 1.0,
        "Fusion fraction must be 1.0 upon auto-ignition"
    );
    assert!(
        ignition.shockwave_radius >= 0.5,
        "Solar wind shockwave must begin propagating upon ignition"
    );

    let star_body = app.world().get::<CelestialBody>(star_entity).expect("Star must exist");
    assert_eq!(
        star_body.body_type,
        BodyType::YellowDwarf,
        "1.33 Solar mass star must transition to YellowDwarf / MainSequenceStar"
    );
}

#[test]
fn test_key_i_triggers_star_ignition_and_cme() {
    use bevy::ecs::system::SystemState;
    use protostellar::game::interaction::trigger_star_ignition_or_flare;
    use protostellar::game::ui::NotificationToast;
    use protostellar::simulation::space_weather::StellarFlareState;

    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    let star_entity = spawn_accretion_disk_genesis(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let mut toast = NotificationToast::default();

    // 1. Pressing 'I' (shift = false) on unignited protostar forces instant ignition
    {
        let mut sys_state: SystemState<(Commands, Query<(&mut CelestialBody, &mut IgnitionState)>)> =
            SystemState::new(app.world_mut());
        let (mut commands, mut query) = sys_state.get_mut(app.world_mut()).unwrap();
        let (mut star_body, mut ignition) = query.get_mut(star_entity).expect("Star must exist");

        trigger_star_ignition_or_flare(
            &mut commands,
            star_entity,
            &mut star_body,
            &mut ignition,
            false,
            &mut toast,
        );
        sys_state.apply(app.world_mut());
    }
    app.update();

    let ignition = app.world().get::<IgnitionState>(star_entity).expect("Ignition must exist");
    assert!(ignition.is_ignited, "Pressing 'I' must trigger ignition");
    assert_eq!(ignition.fusion_fraction, 1.0);
    assert_eq!(ignition.shockwave_radius, 1.6);
    let star_body = app.world().get::<CelestialBody>(star_entity).expect("Body must exist");
    assert_eq!(star_body.body_type, BodyType::MainSequenceStar);
    assert!(star_body.name.contains("Genesis Star (Main Sequence)"));
    assert!(toast.message.contains("Hydrogen Core Fusion Ignited"));

    // 2. Pressing 'I' on an ALREADY ignited star triggers coronal solar blast
    {
        let mut sys_state: SystemState<(Commands, Query<(&mut CelestialBody, &mut IgnitionState)>)> =
            SystemState::new(app.world_mut());
        let (mut commands, mut query) = sys_state.get_mut(app.world_mut()).unwrap();
        let (mut star_body, mut ignition) = query.get_mut(star_entity).expect("Star must exist");

        trigger_star_ignition_or_flare(
            &mut commands,
            star_entity,
            &mut star_body,
            &mut ignition,
            false,
            &mut toast,
        );
        sys_state.apply(app.world_mut());
    }
    app.update();

    let ignition = app.world().get::<IgnitionState>(star_entity).expect("Ignition must exist");
    assert_eq!(ignition.shockwave_radius, 1.6);
    assert!(toast.message.contains("Coronal Mass Ejection & Solar Blast Triggered"));

    // 3. Pressing Shift+I triggers CME space weather outburst
    {
        let mut sys_state: SystemState<(Commands, Query<(&mut CelestialBody, &mut IgnitionState)>)> =
            SystemState::new(app.world_mut());
        let (mut commands, mut query) = sys_state.get_mut(app.world_mut()).unwrap();
        let (mut star_body, mut ignition) = query.get_mut(star_entity).expect("Star must exist");

        trigger_star_ignition_or_flare(
            &mut commands,
            star_entity,
            &mut star_body,
            &mut ignition,
            true,
            &mut toast,
        );
        sys_state.apply(app.world_mut());
    }
    app.update();

    assert!(app.world().get::<StellarFlareState>(star_entity).is_some());
    assert!(toast.message.contains("CME Coronal Mass Ejection Erupted"));
}
