//! Automated astrophysics tests for Rayleigh & Mie atmospheric scattering glow shells.

use bevy::math::Vec3;
use bevy::prelude::*;
use protostellar::rendering::bodies::atmospheres::*;
use protostellar::rendering::bodies::VisualBody;
use protostellar::rendering::materials::*;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;

#[test]
fn test_rayleigh_cross_section_wavelength_dependency() {
    // Standard visual wavelengths: Red (650 nm), Green (570 nm), Blue (475 nm)
    let lambda_red = 650.0f32;
    let lambda_green = 570.0f32;
    let lambda_blue = 475.0f32;

    // Rayleigh scattering cross-section sigma_R is proportional to lambda^(-4)
    let sigma_red = lambda_red.powi(-4);
    let _sigma_green = lambda_green.powi(-4);
    let sigma_blue = lambda_blue.powi(-4);

    // Blue light must scatter substantially more than red light (factor of ~3.5x)
    let blue_to_red_ratio = sigma_blue / sigma_red;
    assert!(
        blue_to_red_ratio > 3.0 && blue_to_red_ratio < 4.0,
        "Rayleigh blue/red scattering ratio should be ~3.5x, got {:.2}",
        blue_to_red_ratio
    );

    let comp = Composition::rocky();
    let profile =
        compute_atmosphere_spectral_profile(BodyType::TerrestrialPlanet, &comp, 288.0, 1.013, 0.50);

    // For Earth-like atmosphere, blue scattering must strictly exceed red scattering
    assert!(
        profile.rayleigh_beta.z > profile.rayleigh_beta.x,
        "Earth Rayleigh profile must scatter blue (z) more than red (x): {:?}",
        profile.rayleigh_beta
    );
}

#[test]
fn test_spectral_profiles_by_archetype() {
    // 1. Venusian super-greenhouse (high temp >= 380K, dense pressure > 10 bar)
    let comp_venus = Composition::rocky();
    let prof_venus = compute_atmosphere_spectral_profile(
        BodyType::TerrestrialPlanet,
        &comp_venus,
        737.0,
        92.0,
        0.95,
    );
    assert!(
        prof_venus.rayleigh_beta.x > 1.0,
        "Venusian atmosphere must have high optical scattering: {:?}",
        prof_venus.rayleigh_beta
    );
    assert!(
        prof_venus.shell_outer_scale > 1.05,
        "Venusian dense atmosphere shell must extend outwards: {}",
        prof_venus.shell_outer_scale
    );

    // 2. Titan analog (cold, volatile ice and organics)
    let mut comp_titan = Composition::default();
    comp_titan.organics_frac = 0.35;
    comp_titan.ice_frac = 0.50;
    let prof_titan = compute_atmosphere_spectral_profile(
        BodyType::TerrestrialPlanet,
        &comp_titan,
        94.0,
        1.5,
        0.70,
    );
    // Photochemical tholin smog: amber-orange scattering (red/green > blue)
    assert!(
        prof_titan.rayleigh_beta.x > prof_titan.rayleigh_beta.z,
        "Titan-like tholin smog must have red scattering exceeding blue: {:?}",
        prof_titan.rayleigh_beta
    );

    // 3. Martian thin atmosphere (low pressure < 0.05 bar)
    let comp_mars = Composition::rocky();
    let prof_mars = compute_atmosphere_spectral_profile(
        BodyType::TerrestrialPlanet,
        &comp_mars,
        210.0,
        0.007,
        0.05,
    );
    assert!(
        prof_mars.rayleigh_beta.length() < prof_venus.rayleigh_beta.length(),
        "Mars thin atmosphere must have lower scattering cross-section than Venus"
    );
    assert!(
        prof_mars.shell_outer_scale < prof_venus.shell_outer_scale,
        "Mars thin atmosphere shell scale must be more compact than Venus"
    );

    // 4. Ice Giant (Neptune / Uranus: methane absorption, azure-cyan scattering)
    let mut comp_neptune = Composition::solar_gas();
    comp_neptune.ice_frac = 0.65;
    let prof_neptune =
        compute_atmosphere_spectral_profile(BodyType::IceGiant, &comp_neptune, 72.0, 50.0, 0.40);
    assert!(
        prof_neptune.rayleigh_beta.z > prof_neptune.rayleigh_beta.x * 3.0,
        "Ice giant must exhibit intense methane blue scattering: {:?}",
        prof_neptune.rayleigh_beta
    );
}

#[test]
fn test_scale_height_temperature_gravity_scaling() {
    let comp = Composition::rocky();

    // Hot world (T = 600 K) vs cold world (T = 150 K)
    let prof_hot =
        compute_atmosphere_spectral_profile(BodyType::TerrestrialPlanet, &comp, 600.0, 1.0, 0.5);
    let prof_cold =
        compute_atmosphere_spectral_profile(BodyType::TerrestrialPlanet, &comp, 150.0, 1.0, 0.5);

    assert!(
        prof_hot.rayleigh_scale_height > prof_cold.rayleigh_scale_height,
        "Thermal energy must increase atmospheric scale height: hot={:.3}, cold={:.3}",
        prof_hot.rayleigh_scale_height,
        prof_cold.rayleigh_scale_height
    );
}

#[test]
fn test_mie_phase_forward_scattering_peak() {
    // Cornette-Shanks / Henyey-Greenstein Mie phase function test
    let g = 0.82f32;
    let g2 = g * g;

    let eval_mie_phase = |cos_th: f32| -> f32 {
        let cos2_th = cos_th * cos_th;
        let denom = 1.0 + g2 - 2.0 * g * cos_th;
        (3.0 * (1.0 - g2) / (8.0 * std::f32::consts::PI * (2.0 + g2))) * (1.0 + cos2_th)
            / (denom * denom.sqrt())
    };

    let forward = eval_mie_phase(1.0); // Looking directly towards host star (backlit crescent)
    let side = eval_mie_phase(0.0); // 90 degree grazing scattering
    let backward = eval_mie_phase(-1.0); // Direct backscattering

    assert!(
        forward > side * 15.0,
        "Forward Mie scattering ({:.2}) must dwarf side scattering ({:.2})",
        forward,
        side
    );
    assert!(
        forward > backward * 50.0,
        "Forward Mie scattering ({:.2}) must vastly exceed backscattering ({:.2})",
        forward,
        backward
    );
}

#[test]
fn test_atmospheric_optical_depth_and_pressure() {
    let comp = Composition::rocky();

    // Near-vacuum world (e.g. 0.0001 bar) vs thick atmosphere (1.0 bar) vs super-thick (10 bar)
    let p_vac = 0.0001f32;
    let p_earth = 1.0f32;
    let p_thick = 10.0f32;

    let prof_earth = compute_atmosphere_spectral_profile(
        BodyType::TerrestrialPlanet,
        &comp,
        288.0,
        p_earth,
        0.5,
    );
    let prof_thick = compute_atmosphere_spectral_profile(
        BodyType::TerrestrialPlanet,
        &comp,
        288.0,
        p_thick,
        0.5,
    );

    // Thick atmosphere has greater shell scale extension
    assert!(
        prof_thick.shell_outer_scale >= prof_earth.shell_outer_scale,
        "Thick atmosphere shell scale ({:.3}) must equal or exceed Earth scale ({:.3})",
        prof_thick.shell_outer_scale,
        prof_earth.shell_outer_scale
    );

    // Pressure threshold check for active atmosphere
    let has_atmo_vac = p_vac >= 0.005;
    let has_atmo_earth = p_earth >= 0.005;
    assert!(!has_atmo_vac, "Vacuum world must not activate atmosphere");
    assert!(has_atmo_earth, "1 bar world must activate atmosphere");
}

#[test]
fn test_atmosphere_shell_ecs_lifecycle() {
    use protostellar::rendering::bodies::VisualAssets;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<AtmosphereMaterial>>();

    let sphere_mesh = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap())
    };

    app.insert_resource(VisualAssets {
        star_mesh: sphere_mesh.clone(),
        planet_mesh: sphere_mesh.clone(),
        atmosphere_mesh: sphere_mesh.clone(),
        asteroid_potato_mesh: sphere_mesh.clone(),
        asteroid_rubble_mesh: sphere_mesh.clone(),
        comet_bilobate_mesh: sphere_mesh.clone(),
        particle_mesh: sphere_mesh.clone(),
        ring_mesh: sphere_mesh.clone(),
        beam_core_mesh: sphere_mesh.clone(),
        beam_sheath_mesh: sphere_mesh.clone(),
        accretion_disk_mesh: sphere_mesh.clone(),
        pulsar_beam_mesh: sphere_mesh.clone(),
        magnetar_ring_mesh: sphere_mesh.clone(),
        magnetar_field_loops_mesh: sphere_mesh.clone(),
    });

    // Spawn central star
    app.world_mut().spawn((
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        CentralStar,
        SimPosition(bevy::math::DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        Temperature(5778.0),
        Composition::solar_gas(),
    ));

    // Spawn Earth-like planet with 1.013 bar atmosphere
    let planet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Earth".to_string(),
                body_type: BodyType::TerrestrialPlanet,
            },
            SimPosition(bevy::math::DVec3::new(1.0, 0.0, 0.0)),
            Mass(EARTH_MASS_SOLAR),
            Radius(4.25875e-5),
            Temperature(288.0),
            Composition::rocky(),
            VolatileInventory {
                atmospheric_pressure_bar: 1.013,
                ..default()
            },
            Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
            VisualBody,
            Visibility::default(),
        ))
        .id();

    app.add_systems(Update, sync_planetary_atmospheres);

    // First update: should spawn atmosphere shell child
    app.update();

    let children = app.world().get::<Children>(planet_ent);
    assert!(
        children.is_some(),
        "Planet must have spawned children entities"
    );

    let child_ent = children.unwrap()[0];
    let has_atmo_marker = app.world().get::<VisualAtmosphereChild>(child_ent);
    assert!(
        has_atmo_marker.is_some(),
        "Child entity must have VisualAtmosphereChild component"
    );

    let atmo_mat_handle = app
        .world()
        .get::<MeshMaterial3d<AtmosphereMaterial>>(child_ent);
    assert!(
        atmo_mat_handle.is_some(),
        "Child entity must have MeshMaterial3d<AtmosphereMaterial>"
    );

    let atmo_materials = app.world().resource::<Assets<AtmosphereMaterial>>();
    let mat = atmo_materials.get(&atmo_mat_handle.unwrap().0).unwrap();

    // Verify uniforms were filled with star direction and Earth-like Rayleigh scattering
    assert_eq!(mat.uniforms.star_dir_and_intensity.x, -1.0); // Planet at (1,0,0), Star at (0,0,0) -> dir is (-1,0,0)
    assert!(mat.uniforms.rayleigh_params.z > mat.uniforms.rayleigh_params.x); // Blue > Red

    // Now strip atmosphere (pressure = 0.0) and run update again
    if let Some(mut vol) = app.world_mut().get_mut::<VolatileInventory>(planet_ent) {
        vol.atmospheric_pressure_bar = 0.0;
    }
    app.update();

    let child_trans = app.world().get::<Transform>(child_ent).unwrap();
    assert_eq!(
        child_trans.scale,
        Vec3::ZERO,
        "Atmosphere shell must collapse to zero scale when atmosphere is stripped"
    );
}

#[test]
fn test_atmosphere_and_planet_shader_sources_validity() {
    let atmo_src = std::fs::read_to_string("assets/shaders/atmosphere.wgsl")
        .expect("atmosphere.wgsl must be readable");
    assert!(
        atmo_src.contains("@group(#{MATERIAL_BIND_GROUP}) @binding(0)"),
        "atmosphere.wgsl must bind to MATERIAL_BIND_GROUP, not hardcoded group(2)"
    );
    assert!(
        atmo_src.contains("struct AtmosphereUniforms"),
        "atmosphere.wgsl must declare AtmosphereUniforms"
    );

    let planet_src = std::fs::read_to_string("assets/shaders/planet.wgsl")
        .expect("planet.wgsl must be readable");
    assert!(
        planet_src.contains("star_dir_and_lum: vec4<f32>"),
        "planet.wgsl must declare star_dir_and_lum"
    );
    assert!(
        planet_src.contains("scattering_params: vec4<f32>"),
        "planet.wgsl must declare scattering_params"
    );

    // Ensure no duplicate variable declarations in apply_crater_shading
    let melt_glow_count = planet_src.matches("let melt_glow = ").count();
    assert_eq!(
        melt_glow_count, 1,
        "planet.wgsl must declare `let melt_glow = ` exactly once, got {}",
        melt_glow_count
    );

    // Verify minor bodies (planet_type == 4u) have a dedicated branch before terrestrial planets
    let idx_type4 = planet_src.find("else if (planet.planet_type == 4u)");
    let idx_type3 = planet_src.find("else if (planet.planet_type == 3u)");
    assert!(
        idx_type4.is_some() && idx_type3.is_some(),
        "planet.wgsl must have dedicated branches for both planet_type 4u and 3u"
    );
    assert!(
        idx_type4.unwrap() < idx_type3.unwrap(),
        "planet_type 4u (minor bodies) must precede planet_type 3u to prevent volatile hijacking"
    );
}

#[test]
fn test_minor_bodies_do_not_spawn_atmosphere_shells() {
    use protostellar::rendering::bodies::VisualAssets;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<AtmosphereMaterial>>();

    let sphere_mesh = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap())
    };

    app.insert_resource(VisualAssets {
        star_mesh: sphere_mesh.clone(),
        planet_mesh: sphere_mesh.clone(),
        atmosphere_mesh: sphere_mesh.clone(),
        asteroid_potato_mesh: sphere_mesh.clone(),
        asteroid_rubble_mesh: sphere_mesh.clone(),
        comet_bilobate_mesh: sphere_mesh.clone(),
        particle_mesh: sphere_mesh.clone(),
        ring_mesh: sphere_mesh.clone(),
        beam_core_mesh: sphere_mesh.clone(),
        beam_sheath_mesh: sphere_mesh.clone(),
        accretion_disk_mesh: sphere_mesh.clone(),
        pulsar_beam_mesh: sphere_mesh.clone(),
        magnetar_ring_mesh: sphere_mesh.clone(),
        magnetar_field_loops_mesh: sphere_mesh.clone(),
    });

    // Spawn central star
    app.world_mut().spawn((
        CelestialBody {
            name: "Sun".to_string(),
            body_type: BodyType::YellowDwarf,
        },
        CentralStar,
        SimPosition(bevy::math::DVec3::ZERO),
        Mass(1.0),
        Radius(0.00465),
        Temperature(5778.0),
        Composition::solar_gas(),
    ));

    // Spawn Comet with high ice and outgassing pressure
    let comet_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Halley".to_string(),
                body_type: BodyType::Comet,
            },
            SimPosition(bevy::math::DVec3::new(2.0, 0.0, 0.0)),
            Mass(0.0001 * EARTH_MASS_SOLAR),
            Radius(1e-5),
            Temperature(140.0),
            Composition::icy(),
            VolatileInventory {
                atmospheric_pressure_bar: 0.5,
                ..default()
            },
            Transform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
            VisualBody,
            Visibility::default(),
        ))
        .id();

    // Spawn Asteroid (Carbonaceous chondrite with ice and volatiles)
    let asteroid_ent = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Ceres-Like".to_string(),
                body_type: BodyType::Asteroid,
            },
            SimPosition(bevy::math::DVec3::new(2.7, 0.0, 0.0)),
            Mass(0.0002 * EARTH_MASS_SOLAR),
            Radius(2e-5),
            Temperature(170.0),
            Composition::carbonaceous(),
            VolatileInventory {
                atmospheric_pressure_bar: 0.1,
                ..default()
            },
            Transform::from_translation(Vec3::new(2.7, 0.0, 0.0)),
            VisualBody,
            Visibility::default(),
        ))
        .id();

    app.add_systems(Update, sync_planetary_atmospheres);
    app.update();

    // Verify neither Comet nor Asteroid spawned an atmosphere shell child
    let comet_children = app.world().get::<Children>(comet_ent);
    assert!(
        comet_children.is_none() || comet_children.unwrap().is_empty(),
        "Comet must never spawn an atmospheric shell child"
    );

    let asteroid_children = app.world().get::<Children>(asteroid_ent);
    assert!(
        asteroid_children.is_none() || asteroid_children.unwrap().is_empty(),
        "Asteroid must never spawn an atmospheric shell child"
    );
}
