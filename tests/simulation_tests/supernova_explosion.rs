use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::rendering::bodies::VisualAssets;
use protostellar::rendering::effects::remnants::{
    update_persistent_remnants, PersistentRemnantPool, PersistentSupernovaRemnant,
};
use protostellar::rendering::effects::supernova::*;
use protostellar::rendering::materials::{SupernovaMaterial, SupernovaUniforms};
use protostellar::rendering::particle_swarm::ParticleSwarmData;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::SimTime;

#[test]
fn test_supernova_explosion_classification_and_layer_generation() {
    let mut pool = SupernovaDebrisPool::default();
    let dummy_star = Entity::from_bits(42);

    // 1. Hypernova (Mass >= 25 M_sun -> Black Hole)
    trigger_supernova_explosion(
        &mut pool,
        dummy_star,
        Vec3::ZERO,
        35.0,
        3.5,
        BodyType::BlackHole,
        0.0,
    );
    assert_eq!(pool.explosions.len(), 1);
    let hyper = &pool.explosions[0];
    assert_eq!(hyper.explosion_type, SupernovaType::Hypernova);
    assert_eq!(hyper.blast_speed_au_s, 75.0);
    assert_eq!(hyper.max_radius_au, 150.0);
    assert_eq!(hyper.fragments.len(), 160);
    assert!(hyper
        .fragments
        .iter()
        .any(|f| f.layer == EjectaLayer::RelativisticJetBreakout));
    assert!(hyper
        .fragments
        .iter()
        .any(|f| f.layer == EjectaLayer::CoreNickelIron));

    // 2. Type II Core-Collapse (Mass 8 - 25 M_sun -> Pulsar / Neutron Star)
    pool.clear();
    trigger_supernova_explosion(
        &mut pool,
        dummy_star,
        Vec3::ZERO,
        15.0,
        1.44,
        BodyType::Pulsar,
        0.0,
    );
    assert_eq!(pool.explosions.len(), 1);
    let type_ii = &pool.explosions[0];
    assert_eq!(type_ii.explosion_type, SupernovaType::TypeII);
    assert_eq!(type_ii.blast_speed_au_s, 45.0);
    assert_eq!(type_ii.max_radius_au, 110.0);
    assert_eq!(type_ii.fragments.len(), 128);
    assert!(type_ii
        .fragments
        .iter()
        .any(|f| f.layer == EjectaLayer::MantleOxygenSilicon));
    assert!(type_ii
        .fragments
        .iter()
        .any(|f| f.layer == EjectaLayer::OuterEnvelopeHydrogen));

    // 3. Planetary Nebula (Mass 0.5 - 8 M_sun -> White Dwarf)
    pool.clear();
    trigger_supernova_explosion(
        &mut pool,
        dummy_star,
        Vec3::ZERO,
        2.5,
        0.6,
        BodyType::WhiteDwarf,
        0.0,
    );
    assert_eq!(pool.explosions.len(), 1);
    let pneb = &pool.explosions[0];
    assert_eq!(pneb.explosion_type, SupernovaType::PlanetaryNebula);
    assert_eq!(pneb.blast_speed_au_s, 4.5);
    assert_eq!(pneb.max_radius_au, 45.0);
    assert_eq!(pneb.fragments.len(), 72);
}

#[test]
fn test_supernova_debris_kinematics_and_decay() {
    let mut pool = SupernovaDebrisPool::default();
    let dummy_star = Entity::from_bits(10);

    trigger_supernova_explosion(
        &mut pool,
        dummy_star,
        Vec3::ZERO,
        18.0,
        1.4,
        BodyType::Pulsar,
        0.0,
    );

    let initial_r = pool.explosions[0].current_radius_au;
    let initial_flash = pool.explosions[0].flash_intensity;
    let initial_temp = pool.explosions[0].fragments[0].temp_k;
    let initial_speed = pool.explosions[0].fragments[0].vel.length();

    // Advance simulation
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<SupernovaEvent>();
    app.insert_resource(pool);
    app.add_systems(Update, update_supernova_explosions);

    // Run first step to initialize Bevy time
    app.update();
    // Advance time by 500ms and run second step
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(500));
    app.update();

    let pool = app.world().resource::<SupernovaDebrisPool>();
    assert_eq!(pool.explosions.len(), 1);
    let exp = &pool.explosions[0];

    // Verify blast front expansion
    assert!(exp.current_radius_au > initial_r);
    // Verify flash decay
    assert!(exp.flash_intensity < initial_flash);
    // Verify ejecta thermal cooling and deceleration
    assert!(exp.fragments[0].temp_k < initial_temp);
    assert!(exp.fragments[0].vel.length() < initial_speed);
}

#[test]
fn test_supernova_blast_clears_particle_swarm() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<SupernovaEvent>();
    app.init_resource::<SupernovaDebrisPool>();

    // Setup ParticleSwarmData with 10 particles at ~1 AU
    let count = 10;
    let swarm = ParticleSwarmData {
        positions: vec![[1.0, 0.0, 0.0]; count],
        velocities: vec![[0.0, 0.0, 0.0]; count],
        masses: vec![1e-9; count],
        compositions: vec![Composition::default(); count],
        temperatures: vec![150.0; count],
        colors: vec![[0.5, 0.5, 0.5, 1.0]; count],
        mesh_positions: vec![[0.0; 3]; count * 4],
        mesh_colors: vec![[1.0; 4]; count * 4],
        bin_heads: vec![-1; 4096],
        bin_next: vec![-1; count],
        mesh_handle: Handle::default(),
        count,
        base_mass: 1e-9,
        is_dirty: false,
        pending_gpu_accretions: Vec::new(),
    };
    app.insert_resource(swarm);

    // Trigger supernova explosion with expanding shockwave encompassing 1 AU
    {
        let mut pool = app.world_mut().resource_mut::<SupernovaDebrisPool>();
        trigger_supernova_explosion(
            &mut pool,
            Entity::from_bits(99),
            Vec3::ZERO,
            20.0,
            1.4,
            BodyType::Pulsar,
            0.0,
        );
    }

    app.add_systems(Update, update_supernova_explosions);
    app.update();

    let swarm = app.world().resource::<ParticleSwarmData>();
    // Swarm particles in blast wave must have outward kick, vaporized ice, and incandescent heat
    assert!(
        swarm.velocities[0][0] > 5.0,
        "Particle must receive outward blast kick"
    );
    assert_eq!(
        swarm.compositions[0].ice_frac, 0.0,
        "Ices must be vaporized by blast"
    );
    assert_eq!(
        swarm.temperatures[0], 4500.0,
        "Particles must be superheated"
    );
}

#[test]
fn test_supernova_blast_interacts_with_planets_and_asteroids() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<SupernovaEvent>();
    app.init_resource::<SupernovaDebrisPool>();

    let star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Exploding Star".to_string(),
                body_type: BodyType::BlueSupergiant,
            },
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            Mass(22.0),
            Temperature(4000.0),
            CentralStar,
        ))
        .id();

    // Asteroid at 0.5 AU (should be vaporized by core collapse blast)
    let asteroid_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Inner Asteroid".to_string(),
                body_type: BodyType::Asteroid,
            },
            SimPosition(DVec3::new(0.5, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Mass(1e-12),
            Temperature(300.0),
        ))
        .id();

    // Terrestrial planet at 1.5 AU (should survive, superheat, and gain AtmosphericEscapeTail)
    let planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Close Terrestrial World".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(DVec3::new(1.5, 0.0, 0.0)),
            SimVelocity(DVec3::ZERO),
            Mass(0.000003),
            Temperature(280.0),
        ))
        .id();

    // Trigger Type II Supernova
    {
        let mut pool = app.world_mut().resource_mut::<SupernovaDebrisPool>();
        trigger_supernova_explosion(
            &mut pool,
            star_ent,
            Vec3::ZERO,
            22.0,
            1.44,
            BodyType::Pulsar,
            0.0,
        );
    }

    app.add_systems(Update, update_supernova_explosions);
    app.update();

    // 1. Verify asteroid was vaporized / despawned
    assert!(
        app.world().get_entity(asteroid_ent).is_err(),
        "Close asteroid within 2.0 AU should be vaporized by supernova blast"
    );

    // 2. Verify planet survived and superheated
    let planet_temp = app.world().get::<Temperature>(planet_ent).unwrap();
    assert!(
        planet_temp.0 > 1000.0,
        "Planet surface should be superheated by blast wave (got {:.1} K)",
        planet_temp.0
    );

    // 3. Verify AtmosphericEscapeTail was dynamically attached pointing away from remnant
    let tail = app.world().get::<AtmosphericEscapeTail>(planet_ent);
    assert!(
        tail.is_some(),
        "Surviving planet must dynamically receive AtmosphericEscapeTail during supernova blast"
    );
    let tail = tail.unwrap();
    assert!(tail.is_active);
    assert!(tail.loss_rate_m_earth_per_myr > 10.0);
}

#[test]
fn test_supernova_event_ingestion_in_bevy_schedule() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<SupernovaEvent>();
    app.init_resource::<SupernovaDebrisPool>();
    app.init_resource::<SimTime>();

    let star_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Dying Massive Star".to_string(),
                body_type: BodyType::BlueSupergiant,
            },
            SimPosition(DVec3::ZERO),
            Mass(30.0),
            CentralStar,
        ))
        .id();

    app.add_systems(Update, update_supernova_explosions);

    // Send SupernovaEvent via message queue
    let mut writer = app.world_mut().resource_mut::<Messages<SupernovaEvent>>();
    writer.write(SupernovaEvent {
        star_entity: star_ent,
        star_name: "Dying Massive Star".to_string(),
        initial_mass_solar: 30.0,
        remnant_mass_solar: 3.5,
        remnant_type: BodyType::BlackHole,
        shockwave_velocity_km_s: 15_000.0,
        ejected_metals_solar: 6.5,
        ejected_composition: protostellar::simulation::components::Composition::supernova_ejecta(),
    });

    app.update();

    let pool = app.world().resource::<SupernovaDebrisPool>();
    assert_eq!(
        pool.explosions.len(),
        1,
        "SupernovaEvent must trigger a SupernovaExplosionInstance in the debris pool"
    );
    assert_eq!(pool.explosions[0].explosion_type, SupernovaType::Hypernova);
}

#[test]
fn test_supernova_uniforms_alignment_and_defaults() {
    let uniforms = SupernovaUniforms::default();
    assert_eq!(uniforms.params, Vec4::new(0.0, 7.5, 1.0, 45.0));
    assert_eq!(uniforms.core_params, Vec4::new(1.0, 0.0, 5.0, 0.0));
    assert_eq!(
        uniforms.center_and_asphericity,
        Vec4::new(0.0, 0.0, 0.0, 0.15)
    );

    // Verify 16-byte WGSL uniform alignment
    assert_eq!(std::mem::size_of::<SupernovaUniforms>() % 16, 0);
}

#[test]
fn test_supernova_wgsl_shader_naga_validation() {
    crate::shader_guardrails::validate_wgsl_shader_with_naga(
        "assets/shaders/supernova.wgsl",
        "struct SupernovaUniforms",
    );
}

#[test]
fn test_supernova_visual_shell_ecs_sync() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<SupernovaMaterial>();
    app.init_resource::<SupernovaDebrisPool>();

    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let fallback_mesh = meshes.add(Sphere::new(1.0).mesh());
    app.insert_resource(VisualAssets::dummy(fallback_mesh));

    app.add_systems(Update, sync_supernova_visuals);

    let star_entity = Entity::from_bits(999);
    {
        let mut pool = app.world_mut().resource_mut::<SupernovaDebrisPool>();
        trigger_supernova_explosion(
            &mut pool,
            star_entity,
            Vec3::new(10.0, 5.0, 0.0),
            28.0,
            3.0,
            BodyType::BlackHole,
            0.0,
        );
    }

    // Run 1: Should spawn SupernovaVisualShell entity
    app.update();

    let mut shell_query = app.world_mut().query::<(
        Entity,
        &SupernovaVisualShell,
        &Transform,
        &MeshMaterial3d<SupernovaMaterial>,
    )>();
    let matching_shells: Vec<_> = shell_query.iter(app.world()).collect();
    assert_eq!(
        matching_shells.len(),
        1,
        "Exactly one visual shell should be spawned"
    );
    let (shell_ent, shell, transform, mat_handle) = matching_shells[0];
    assert_eq!(shell.star_entity, star_entity);
    assert_eq!(transform.translation, Vec3::new(10.0, 5.0, 0.0));
    assert!(transform.scale.x > 0.0);

    // Verify material uniforms were set
    let materials = app.world().resource::<Assets<SupernovaMaterial>>();
    let mat = materials
        .get(&mat_handle.0)
        .expect("Material should exist in Assets");
    assert_eq!(mat.uniforms.center_and_asphericity.x, 10.0);
    assert_eq!(mat.uniforms.center_and_asphericity.y, 5.0);
    assert_eq!(mat.uniforms.core_params.y, 1.0); // Hypernova type_id = 1.0

    // Run 2: Clear explosions from pool, update should despawn the shell
    {
        let mut pool = app.world_mut().resource_mut::<SupernovaDebrisPool>();
        pool.clear();
    }
    app.update();

    assert!(
        app.world().get_entity(shell_ent).is_err(),
        "Visual shell should be despawned when explosion ends"
    );
}

#[test]
fn test_persistent_supernova_remnant_handoff_and_sedov_expansion() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<SupernovaEvent>();
    app.init_resource::<SupernovaDebrisPool>();
    app.init_resource::<PersistentRemnantPool>();
    app.init_resource::<SimTime>();

    let pulsar_star = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Crab Progenitor".to_string(),
                body_type: BodyType::Pulsar,
            },
            SimPosition(DVec3::ZERO),
            Mass(1.4),
            CentralStar,
        ))
        .id();

    // Spawn an explosion that is nearly expired (timer = max_timer)
    {
        let mut pool = app.world_mut().resource_mut::<SupernovaDebrisPool>();
        trigger_supernova_explosion(
            &mut pool,
            pulsar_star,
            Vec3::ZERO,
            12.0,
            1.4,
            BodyType::Pulsar,
            0.0,
        );
        pool.explosions[0].timer = pool.explosions[0].max_timer; // Ready to hand off!
    }

    app.add_systems(Update, update_supernova_explosions);
    app.add_systems(
        Update,
        update_persistent_remnants.after(update_supernova_explosions),
    );

    // Update 1: Supernova explosion finishes and hands off to PersistentRemnantPool
    app.update();

    let remnant_pool = app.world().resource::<PersistentRemnantPool>();
    assert_eq!(
        remnant_pool.remnants.len(),
        1,
        "Finished supernova explosion must hand off to PersistentRemnantPool"
    );
    let rem = &remnant_pool.remnants[0];
    assert_eq!(rem.remnant_type, SupernovaType::TypeII);
    assert!(
        rem.has_pwn,
        "Pulsar remnant must activate Pulsar Wind Nebula"
    );
    assert!(
        rem.filaments.len() >= 24,
        "Remnant must contain multi-layer chemical filaments"
    );
    let initial_r = rem.current_radius_au;

    // Advance simulated time by 2,000 years to verify Sedov-Taylor expansion
    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.current_dt_yr = 2000.0;
        sim_time.elapsed_years = 2000.0;
    }
    app.update();

    let remnant_pool = app.world().resource::<PersistentRemnantPool>();
    let rem_expanded = &remnant_pool.remnants[0];
    assert!(
        rem_expanded.current_radius_au > initial_r,
        "Remnant must expand over deep time (was {:.1} AU, now {:.1} AU)",
        initial_r,
        rem_expanded.current_radius_au
    );
    assert_eq!(rem_expanded.age_years, 2000.0);
    assert!(rem_expanded.opacity > 0.0 && rem_expanded.opacity <= 1.0);
}

#[test]
fn test_supernova_visual_shell_persists_into_remnant() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<SupernovaMaterial>();
    app.init_resource::<SupernovaDebrisPool>();
    app.init_resource::<PersistentRemnantPool>();

    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let fallback_mesh = meshes.add(Sphere::new(1.0).mesh());
    app.insert_resource(VisualAssets::dummy(fallback_mesh));

    app.add_systems(Update, sync_supernova_visuals);

    let star_entity = Entity::from_bits(888);
    {
        let mut pool = app.world_mut().resource_mut::<SupernovaDebrisPool>();
        trigger_supernova_explosion(
            &mut pool,
            star_entity,
            Vec3::new(5.0, 0.0, 0.0),
            20.0,
            1.4,
            BodyType::Pulsar,
            0.0,
        );
    }

    // Step 1: Prompt explosion creates SupernovaVisualShell
    app.update();

    let (shell_ent, mat_handle_id) = {
        let mut shell_query = app.world_mut().query::<(
            Entity,
            &SupernovaVisualShell,
            &Transform,
            &MeshMaterial3d<SupernovaMaterial>,
        )>();
        let matching_shells: Vec<_> = shell_query.iter(app.world()).collect();
        assert_eq!(matching_shells.len(), 1);
        let (shell_ent, shell, transform, mat_handle) = matching_shells[0];
        assert_eq!(shell.star_entity, star_entity);
        assert_eq!(transform.translation, Vec3::new(5.0, 0.0, 0.0));
        (shell_ent, mat_handle.0.clone())
    };

    // Step 2: Prompt explosion ends, but PersistentRemnantPool holds the remnant
    {
        let mut pool = app.world_mut().resource_mut::<SupernovaDebrisPool>();
        pool.clear();
        let mut rem_pool = app.world_mut().resource_mut::<PersistentRemnantPool>();
        rem_pool.remnants.push(PersistentSupernovaRemnant {
            star_entity,
            center: Vec3::new(5.0, 0.0, 0.0),
            remnant_type: SupernovaType::TypeII,
            initial_radius_au: 20.0,
            current_radius_au: 80.0,
            expansion_rate_au_yr: 0.02,
            ejecta_mass_solar: 18.6,
            metals_mass_solar: 3.5,
            age_years: 500.0,
            max_age_years: 50_000.0,
            opacity: 0.85,
            has_pwn: true,
            filaments: Vec::new(),
        });
    }

    app.update();

    // Verify visual shell persists and was updated to remnant scale & opacity
    assert!(app.world().get_entity(shell_ent).is_ok());
    let updated_trans = app.world().get::<Transform>(shell_ent).unwrap();
    assert_eq!(updated_trans.scale.x, 80.0);

    let materials = app.world().resource::<Assets<SupernovaMaterial>>();
    let mat = materials.get(&mat_handle_id).unwrap();
    assert_eq!(mat.uniforms.core_params.w, 0.85);

    // Step 3: When remnant is cleared, the visual shell is despawned
    {
        let mut rem_pool = app.world_mut().resource_mut::<PersistentRemnantPool>();
        rem_pool.clear();
    }
    app.update();

    assert!(app.world().get_entity(shell_ent).is_err());
}
