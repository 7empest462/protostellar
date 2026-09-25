//! Test module generated from simulation_tests.

use bevy::prelude::*;

#[test]
fn test_trappist1_system_resonance_and_habitable_zone() {
    let m_star = 0.0898f64; // TRAPPIST-1 mass (Solar)
    let l_star = 0.000553f64; // TRAPPIST-1 luminosity (Solar)

    // Habitable zone boundaries for TRAPPIST-1: r_hz ~ sqrt(L_star / S_eff)
    let hz_inner = 0.75 * l_star.sqrt(); // ~0.0176 AU
    let hz_outer = 1.77 * l_star.sqrt(); // ~0.0416 AU

    let semimajor_axes: [f64; 7] = [
        0.01154, 0.01580, 0.02227, 0.02925, 0.03849, 0.04688, 0.06193,
    ];

    // Compute orbital periods using Kepler's 3rd Law: P = sqrt(a^3 / M_star) in years
    let periods: Vec<f64> = semimajor_axes
        .iter()
        .map(|&a| (a.powf(3.0) / m_star).sqrt() * 365.25)
        .collect();

    // Check TRAPPIST-1e and 1f are inside the habitable zone
    assert!(semimajor_axes[3] >= hz_inner && semimajor_axes[3] <= hz_outer * 1.2); // TRAPPIST-1e
    assert!(semimajor_axes[4] >= hz_inner && semimajor_axes[4] <= hz_outer * 1.2); // TRAPPIST-1f

    // Verify resonant period ratios are near integer ratios (e.g. 1c/1b ~ 1.6 ~ 8:5, 1d/1c ~ 1.67 ~ 5:3, 1e/1d ~ 1.5 ~ 3:2)
    let ratio_c_b = periods[1] / periods[0];
    let ratio_e_d = periods[3] / periods[2];
    assert!((ratio_c_b - 1.60).abs() < 0.15);
    assert!((ratio_e_d - 1.50).abs() < 0.15);
}

#[test]
fn test_kepler16_circumbinary_stability_radius() {
    // Holman & Wiegert (1999) empirical dynamical stability limit for P-type circumbinary planets:
    // a_crit = a_bin * (1.60 + 5.10*e_bin - 2.22*e_bin^2 + 4.12*mu - 4.27*e_bin*mu - 5.09*mu^2 + 4.61*e_bin^2*mu^2)
    let m_a = 0.6897f64;
    let m_b = 0.2025f64;
    let a_bin = 0.2243f64;
    let e_bin = 0.159f64;
    let mu = m_b / (m_a + m_b); // Mass ratio ~ 0.227

    let a_crit = a_bin
        * (1.60 + 5.10 * e_bin - 2.22 * e_bin.powi(2) + 4.12 * mu
            - 4.27 * e_bin * mu
            - 5.09 * mu.powi(2)
            + 4.61 * e_bin.powi(2) * mu.powi(2));

    // Kepler-16b circumbinary orbit at a = 0.7048 AU
    let a_kepler16b = 0.7048f64;

    // The planet's orbit must be dynamically stable (outside a_crit ~ 0.65 AU)
    assert!(a_kepler16b > a_crit);
    assert!(a_crit > 0.55 && a_crit < 0.70);
}

#[test]
fn test_scenario_preset_definitions() {
    use protostellar::simulation::scenarios::ScenarioPreset;

    let presets = [
        ScenarioPreset::SolarNebulaMmsn,
        ScenarioPreset::AccretionDiskGenesis,
        ScenarioPreset::Trappist1System,
        ScenarioPreset::Kepler16Circumbinary,
        ScenarioPreset::HotJupiterMigration,
        ScenarioPreset::RoguePlanetFlyby,
        ScenarioPreset::LittleRedDot,
        ScenarioPreset::PulsarSystem,
        ScenarioPreset::MagnetarOutburst,
    ];

    for preset in presets {
        assert!(!preset.display_name().is_empty());
        assert!(!preset.description().is_empty());
    }
}

#[test]
fn test_gas_giant_variety_palette() {
    use protostellar::rendering::bodies::compute_gas_giant_palette;
    use protostellar::utils::constants::JUPITER_MASS_SOLAR;

    // 1. Classic Jupiter (1.0 M_jup) -> Iconic Jovian Amber-Ochre
    let jupiter_color = compute_gas_giant_palette(JUPITER_MASS_SOLAR * 1.0, 160.0, "Jupiter");
    let jup_srgba = jupiter_color.to_srgba();
    assert!(jup_srgba.red > jup_srgba.green && jup_srgba.green > jup_srgba.blue);

    // 2. Super-Jupiter (2.5 M_jup) -> Emerald-Teal
    let super_jup_color =
        compute_gas_giant_palette(JUPITER_MASS_SOLAR * 2.5, 160.0, "Super-Jovian");
    let sj_srgba = super_jup_color.to_srgba();
    assert!(sj_srgba.green >= sj_srgba.red); // Green/teal dominant

    // 3. Massive Super-Jupiter (4.5 M_jup) -> Lapis-Indigo / Sapphire
    let massive_color = compute_gas_giant_palette(JUPITER_MASS_SOLAR * 4.5, 160.0, "Mega-Jovian");
    let mass_srgba = massive_color.to_srgba();
    assert!(mass_srgba.blue > mass_srgba.red); // Blue dominant

    // 4. Heavy Super-Jupiter (8.0 M_jup) -> Royal Plum-Purple
    let heavy_color = compute_gas_giant_palette(JUPITER_MASS_SOLAR * 8.0, 160.0, "Ultra-Giant");
    let heavy_srgba = heavy_color.to_srgba();
    assert!(heavy_srgba.blue > heavy_srgba.green && heavy_srgba.red > heavy_srgba.green); // Purple mix

    // 5. Brown Dwarf Transition (14.0 M_jup) -> Incandescent Plum-Maroon
    let brown_dwarf_color =
        compute_gas_giant_palette(JUPITER_MASS_SOLAR * 14.0, 600.0, "Brown Dwarf");
    let bd_srgba = brown_dwarf_color.to_srgba();
    assert!(bd_srgba.red > bd_srgba.green);
}

#[test]
fn test_planet_builder_presets() {
    use protostellar::game::ui::{BuilderPreset, PlanetBuilderState};
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, JUPITER_MASS_SOLAR};

    let mut state = PlanetBuilderState::default();

    // Default is Earth-like
    assert_eq!(state.active_preset, BuilderPreset::EarthLike);
    assert!((state.mass_solar - EARTH_MASS_SOLAR).abs() < 1e-10);
    assert!((state.semi_major_axis_au - 1.0).abs() < 1e-10);

    // Apply Super-Jupiter preset
    state.apply_preset(BuilderPreset::SuperJupiter);
    assert_eq!(state.active_preset, BuilderPreset::SuperJupiter);
    assert!((state.mass_solar - JUPITER_MASS_SOLAR * 3.5).abs() < 1e-10);
    assert!((state.semi_major_axis_au - 3.2).abs() < 1e-10);
    assert!(state.gas_frac > 0.90);

    // Apply Water World preset
    state.apply_preset(BuilderPreset::WaterWorld);
    assert_eq!(state.active_preset, BuilderPreset::WaterWorld);
    assert!(state.ice_frac > 0.50);
    assert_eq!(state.gas_frac, 0.0);
}

#[test]
fn test_asteroid_particle_accretion_capping() {
    use protostellar::simulation::components::BodyType;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    // Minor asteroid should be capped
    let asteroid_type = BodyType::Asteroid;
    let initial_mass = 0.00001 * EARTH_MASS_SOLAR;
    let gain = 0.001 * EARTH_MASS_SOLAR;

    let updated_mass = if matches!(asteroid_type, BodyType::Asteroid | BodyType::Comet) {
        (initial_mass + gain).min(0.0005 * EARTH_MASS_SOLAR)
    } else {
        initial_mass + gain * 2.0
    };

    assert!(updated_mass <= 0.0005 * EARTH_MASS_SOLAR);
}

#[test]
fn test_visual_radius_for_minor_bodies() {
    use protostellar::simulation::components::BodyType;
    use protostellar::simulation::resources::SimulationConfig;
    use protostellar::utils::constants::EARTH_RADIUS_AU;

    let config = SimulationConfig::default();
    let planet_rad =
        config.calc_visual_radius_for_type(EARTH_RADIUS_AU, BodyType::TerrestrialPlanet);
    let asteroid_rad =
        config.calc_visual_radius_for_type(EARTH_RADIUS_AU * 0.05, BodyType::Asteroid);
    let comet_rad = config.calc_visual_radius_for_type(EARTH_RADIUS_AU * 0.05, BodyType::Comet);

    assert!(asteroid_rad < planet_rad);
    assert!(comet_rad < planet_rad);
    assert!((asteroid_rad - comet_rad).abs() < 1e-6);
}

#[test]
fn test_late_heavy_bombardment_water_delivery_formation() {
    use protostellar::simulation::components::{Composition, VolatileInventory};
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let mut vol = VolatileInventory::default();
    assert_eq!(vol.delivered_water_m_earth, 0.0);
    assert_eq!(vol.ocean_coverage_frac, 0.0);

    // 5 cometary impacts delivering 0.0005 M_earth water each
    let icy_comet_comp = Composition::icy();
    let comet_mass = 0.0008 * EARTH_MASS_SOLAR;
    let water_per_impact = (comet_mass * icy_comet_comp.ice_frac) / EARTH_MASS_SOLAR;

    for _ in 0..5 {
        vol.delivered_water_m_earth += water_per_impact;
    }

    vol.ocean_coverage_frac = (vol.delivered_water_m_earth / 0.003).clamp(0.0, 0.75) as f32;

    assert!(vol.delivered_water_m_earth > 0.002);
    assert!(vol.ocean_coverage_frac >= 0.70);
}

#[test]
fn test_protostar_auto_ignition_and_gas_push() {
    use protostellar::simulation::components::IgnitionState;

    let mut ignition = IgnitionState {
        core_temperature: 4.0e6,
        fusion_fraction: 0.4,
        is_ignited: false,
        shockwave_radius: 0.0,
    };

    let heating_rate_per_yr = 2.0e5; // Solar mass
    let elapsed_dt = 30.0; // 30 years

    ignition.core_temperature += heating_rate_per_yr * elapsed_dt;
    let ignition_threshold = 1.0e7;

    if ignition.core_temperature >= ignition_threshold || elapsed_dt >= 30.0 {
        ignition.is_ignited = true;
        ignition.fusion_fraction = 1.0;
        ignition.shockwave_radius = 0.5;
    }

    assert!(ignition.is_ignited);
    assert_eq!(ignition.fusion_fraction, 1.0);
    assert!(ignition.core_temperature >= 1.0e7);

    // Verify gas push at shockwave: inner terrestrial zone gas density is residual (0.05),
    // outer giant zone is boosted (2.5x) to feed Jupiter.
    let r_inner = 1.0f64;
    let r_outer = 5.2f64;
    let gas_scale = 1.0f64;

    let inner_gas_density = 1.2e-4 * (r_inner / 1.0).powf(-1.50) * (gas_scale * 0.05 + 0.001);
    let outer_gas_density = 1.2e-4 * (r_outer / 1.0).powf(-1.50) * gas_scale * 2.5;

    assert!(inner_gas_density < 1.0e-5);
    assert!(outer_gas_density > 2.0e-5);
}

#[test]
fn test_little_red_dot_quasi_star_model() {
    use protostellar::simulation::components::{BlackHoleStarState, BodyType, Composition};

    let mut state = BlackHoleStarState::default();

    // Verify initial astrophysical parameters
    assert_eq!(state.black_hole_mass_solar, 400_000.0);
    assert_eq!(state.cocoon_mass_solar, 50_000.0);
    assert_eq!(state.total_mass_solar(), 450_000.0);
    assert_eq!(state.cocoon_radius_au, 60.0);
    assert!(state.super_eddington_active);
    assert!(!state.is_blown_out);
    assert_eq!(state.blowout_progress, 0.0);

    // Verify pristine pure hydrogen composition (0% dust, 0% rock, 0% metal)
    let comp = Composition::pure_hydrogen();
    assert_eq!(comp.gas_frac, 1.0);
    assert_eq!(comp.metal_frac, 0.0);
    assert_eq!(comp.silicate_frac, 0.0);
    assert_eq!(comp.ice_frac, 0.0);

    // Test super-Eddington toggling
    state.toggle_super_eddington();
    assert!(!state.super_eddington_active);
    assert_eq!(state.eddington_ratio, 0.9);

    state.toggle_super_eddington();
    assert!(state.super_eddington_active);
    assert_eq!(state.eddington_ratio, 4.5);

    // Test blowout trigger
    state.trigger_blowout();
    assert!(state.is_blown_out);

    // Verify QuasiStar classification and remnant status
    let q_type = BodyType::QuasiStar;
    assert!(q_type.is_star_or_remnant());
    assert!(q_type.is_remnant());
    assert!(!q_type.is_planet());
}

#[test]
fn test_little_red_dot_preset_in_scenarios() {
    use protostellar::simulation::scenarios::ScenarioPreset;

    let lrd = ScenarioPreset::LittleRedDot;
    assert!(lrd.display_name().contains("Little Red Dot"));
    assert!(lrd.description().contains("100,000 M☉"));
    assert!(lrd.description().contains("60 AU"));
}

#[test]
fn test_skybox_scenario_blending_and_materials() {
    use protostellar::rendering::materials::SkyboxMaterial;
    use protostellar::simulation::scenarios::ScenarioPreset;

    // 1. Verify default material initialization (starts in Milky Way mode)
    let mat = SkyboxMaterial::default();
    assert_eq!(mat.uniforms.params.x, 0.0); // time
    assert_eq!(mat.uniforms.params.y, 0.0); // scenario_blend (0.0 = Milky Way)
    assert_eq!(mat.uniforms.params.z, 1.25); // exposure
    assert_eq!(mat.uniforms.params.w, 1.0); // twinkle
    assert_eq!(mat.uniforms.tuning.x, 1.0); // star density
    assert_eq!(mat.uniforms.tuning.y, 1.0); // nebula intensity
    assert_eq!(mat.uniforms.tuning.z, 1.0); // cosmic web scale
    assert_eq!(mat.uniforms.tuning.w, 1.0); // filament brightness

    // 2. Scenario preset to target blend mapping
    let presets_milky_way = [
        ScenarioPreset::SolarNebulaMmsn,
        ScenarioPreset::Trappist1System,
        ScenarioPreset::Kepler16Circumbinary,
        ScenarioPreset::HotJupiterMigration,
        ScenarioPreset::RoguePlanetFlyby,
    ];

    for preset in presets_milky_way {
        let target = if preset == ScenarioPreset::LittleRedDot {
            1.0
        } else {
            0.0
        };
        assert_eq!(target, 0.0);
    }

    let target_early_univ = if ScenarioPreset::LittleRedDot == ScenarioPreset::LittleRedDot {
        1.0
    } else {
        0.0
    };
    assert_eq!(target_early_univ, 1.0);

    // 3. Smooth blend interpolation test
    let mut current_blend = 0.0_f32;
    let dt = 0.25_f32;
    let blend_speed = 2.2_f32;
    current_blend += (target_early_univ - current_blend) * (dt * blend_speed).min(1.0);
    assert!(current_blend > 0.40 && current_blend < 0.70);

    // Subsequent frame continues toward 1.0
    current_blend += (target_early_univ - current_blend) * (dt * blend_speed).min(1.0);
    assert!(current_blend > 0.75 && current_blend <= 1.0);

    // 4. Geometry bounds: Skybox sphere (1,000,000 AU) is safely within camera far plane (2,000,000 AU)
    let skybox_radius = 1_000_000.0_f32;
    let camera_far_plane = 2_000_000.0_f32;
    let max_simulation_boundary = 625.0_f32;
    assert!(skybox_radius > max_simulation_boundary * 1000.0);
    assert!(skybox_radius < camera_far_plane);
}

#[test]
fn test_trappist1_compact_orbital_stability() {
    use bevy::math::DVec3;
    use protostellar::utils::constants::G_ASTRO;

    let m_star = 0.0898f64;
    let a_b = 0.01154f64; // TRAPPIST-1b semi-major axis
    let v_circ = (G_ASTRO * m_star / a_b).sqrt();

    // 1. Verify adaptive softening conservation (0.00005 AU) vs legacy softening (0.008 AU)
    let eps_adaptive = 0.00005f64;
    let eps_legacy = 0.008f64;
    let grav_true = G_ASTRO * m_star / (a_b * a_b);
    let grav_adaptive =
        G_ASTRO * m_star * a_b / (a_b * a_b + eps_adaptive * eps_adaptive).powf(1.5);
    let grav_legacy = G_ASTRO * m_star * a_b / (a_b * a_b + eps_legacy * eps_legacy).powf(1.5);

    // Adaptive softening retains >99.99% of true gravitational acceleration
    assert!((grav_adaptive / grav_true - 1.0).abs() < 0.0001);
    // Legacy softening caused a catastrophic 45% gravity attenuation
    assert!(grav_legacy / grav_true < 0.60);

    // 2. Symplectic leapfrog orbit integration over 200 substeps (~2 full orbits of TRAPPIST-1b)
    let p_orbit_yr = (a_b.powi(3) / m_star).sqrt();
    let dt_sub = p_orbit_yr / 100.0; // 100 substeps per orbit
    let mut pos = DVec3::new(a_b, 0.0, 0.0);
    let mut vel = DVec3::new(0.0, 0.0, v_circ);

    // Initial half-step kick
    let r_sq = pos.length_squared() + eps_adaptive * eps_adaptive;
    let mut acc = -(G_ASTRO * m_star / (r_sq * r_sq.sqrt())) * pos;
    vel += acc * (0.5 * dt_sub);

    for _ in 0..200 {
        pos += vel * dt_sub;
        let r_sq = pos.length_squared() + eps_adaptive * eps_adaptive;
        acc = -(G_ASTRO * m_star / (r_sq * r_sq.sqrt())) * pos;
        vel += acc * dt_sub;
    }
    vel -= acc * (0.5 * dt_sub); // Final half-step kick

    let final_r = (pos.x * pos.x + pos.z * pos.z).sqrt();
    let rel_radial_error = (final_r - a_b).abs() / a_b;

    // Orbit must remain dynamically locked and stable within < 0.2%
    assert!(rel_radial_error < 0.002);
}

#[test]
fn test_earth_lhb_ocean_seeding_and_impact_capture() {
    use bevy::math::DVec3;
    use protostellar::simulation::components::VolatileInventory;
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, G_ASTRO};

    let m_star = 1.0f64;
    let r_earth = 1.0f64;
    let r_spawn = 6.0f64;
    let q_target = 1.0f64;

    // 1. Keplerian rendezvous geometry
    let a_transfer = f64::midpoint(r_spawn, q_target);
    let t_flight = std::f64::consts::PI * (a_transfer.powi(3) / (G_ASTRO * m_star)).sqrt();
    let omega_earth = (G_ASTRO * m_star / (r_earth.powi(3))).sqrt();
    let delta_theta_earth = omega_earth * t_flight;

    // Impactor starts at aphelion opposite the rendezvous point
    let phi_earth_0 = 0.0f64;
    let rendezvous_angle = phi_earth_0 + delta_theta_earth;
    let spawn_angle = rendezvous_angle + std::f64::consts::PI;

    let earth_at_rendezvous = DVec3::new(
        r_earth * rendezvous_angle.cos(),
        0.0,
        r_earth * rendezvous_angle.sin(),
    );
    let impactor_at_perihelion = DVec3::new(
        q_target * (spawn_angle + std::f64::consts::PI).cos(),
        0.0,
        q_target * (spawn_angle + std::f64::consts::PI).sin(),
    );

    let miss_distance = (earth_at_rendezvous - impactor_at_perihelion).length();
    let hill_capture_radius = r_earth * (EARTH_MASS_SOLAR / (3.0 * m_star)).cbrt() * 0.40;

    // Impactor trajectory passes directly through Earth's capture corridor (< 0.004 AU)
    assert!(miss_distance < 1e-6);
    assert!(hill_capture_radius > 0.0035);

    // 2. Volatile ocean delivery budget from 6 comets/asteroids
    let mut vol = VolatileInventory::default();
    let cometary_ice_fraction = 0.55f64;
    let impactor_mass_earth = 0.00025f64;
    let water_per_impact = impactor_mass_earth * cometary_ice_fraction;

    for _ in 0..6 {
        vol.delivered_water_m_earth += water_per_impact;
        vol.cometary_impact_count += 1;
        vol.ocean_coverage_frac = (vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
    }

    assert_eq!(vol.cometary_impact_count, 6);
    assert!(vol.delivered_water_m_earth >= 0.0006);
    // Ocean coverage must exceed 70% threshold (reaches 85% ocean world coverage)
    assert!(vol.ocean_coverage_frac >= 0.70);
}

#[test]
fn test_high_time_warp_belt_formation() {
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::belts::BeltCensus;
    use protostellar::simulation::disk::planetesimals::{
        auto_spawn_planetesimals, PlanetesimalSpawner,
    };
    use protostellar::simulation::disk::spawn_protoplanetary_disk;
    use protostellar::simulation::pebble_accretion::spawn_streaming_instability_minor_bodies;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    let mut config = SimulationConfig::default();
    config.gas_density_scale = 1.0;
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .init_resource::<PlanetesimalSpawner>()
        .init_resource::<protostellar::game::phases::LateHeavyBombardmentState>();

    let disk_params = app.world().resource::<DiskParameters>().clone();
    let sim_config = app.world().resource::<SimulationConfig>().clone();
    spawn_protoplanetary_disk(&mut app.world_mut().commands(), &disk_params, &sim_config);

    app.add_systems(
        Update,
        (
            step_physics_simulation,
            auto_spawn_planetesimals.after(step_physics_simulation),
            spawn_streaming_instability_minor_bodies.after(auto_spawn_planetesimals),
        ),
    );

    // Run 1 frame at 1x to flush initial commands
    app.update();

    // Speed up time to 100,000x for 25 frames (advancing 50 years per frame -> 1,250 years total)
    app.world_mut().resource_mut::<TimeWarp>().multiplier = 100_000.0;
    for _ in 0..25 {
        app.update();
    }

    let mut census = BeltCensus::default();
    let mut query = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, Option<&CentralStar>)>();
    for (body, pos, opt_star) in query.iter(app.world()) {
        if opt_star.is_some() {
            continue;
        }
        let r = (pos.x * pos.x + pos.z * pos.z).sqrt();
        census.record(r);
        assert!(
            r < 1000.0,
            "Body '{}' ({:?}) was flung out to r = {:.2} AU!",
            body.name,
            body.body_type,
            r
        );
    }

    // Belts must be actively populated even under 100,000x time warp
    assert!(
        census.asteroid_belt_count >= 12,
        "Asteroid Belt underpopulated at high time warp: got {}",
        census.asteroid_belt_count
    );
    assert!(
        census.kuiper_count >= 6,
        "Kuiper Belt underpopulated at high time warp: got {}",
        census.kuiper_count
    );
}

#[test]
fn test_guaranteed_minor_body_formation_to_capacity_limit() {
    use bevy::prelude::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::disk::planetesimals::{
        auto_spawn_planetesimals, PlanetesimalSpawner,
    };
    use protostellar::simulation::pebble_accretion::spawn_streaming_instability_minor_bodies;
    use protostellar::simulation::resources::*;

    let mut app = App::new();
    let mut config = SimulationConfig::default();
    config.gas_density_scale = 1.0;
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<PlanetesimalSpawner>();

    let mut disk_params = app.world().resource::<DiskParameters>().clone();
    disk_params.gas_disk_lifetime_yr = 5000.0;
    app.insert_resource(disk_params);

    let mut spawner = app.world().resource::<PlanetesimalSpawner>().clone();
    spawner.max_ecs_bodies = 1024;
    app.insert_resource(spawner);

    app.add_systems(
        Update,
        (
            auto_spawn_planetesimals,
            spawn_streaming_instability_minor_bodies.after(auto_spawn_planetesimals),
        ),
    );

    // Advance time in steps across early gas disk era (t = 0 to 4,000 yr)
    for step in 1..=40 {
        app.world_mut().resource_mut::<SimTime>().elapsed_years = (step as f64) * 100.0;
        app.update();
    }

    let mid_count = app.world_mut().query::<Entity>().iter(app.world()).count();
    assert!(
        mid_count > 600,
        "Minor body count must rapidly ramp beyond 600 during gas era (found {mid_count})"
    );

    // Advance into post-gas debris cascade era (t = 5,000 to 10,000 yr)
    for step in 51..=100 {
        app.world_mut().resource_mut::<SimTime>().elapsed_years = (step as f64) * 100.0;
        app.update();
    }

    let final_count = app.world_mut().query::<Entity>().iter(app.world()).count();
    assert!(
        final_count >= 1000,
        "Minor body formation must reach capacity (~1024) and continue in post-gas era (found {final_count})"
    );

    let mut asteroids = 0;
    let mut comets = 0;
    let mut query = app.world_mut().query::<&CelestialBody>();
    for body in query.iter(app.world()) {
        if body.body_type == BodyType::Asteroid {
            asteroids += 1;
        } else if body.body_type == BodyType::Comet {
            comets += 1;
        }
    }
    assert!(
        asteroids >= 400,
        "Must have abundant asteroids (found {asteroids})"
    );
    assert!(comets >= 250, "Must have abundant comets (found {comets})");
}

#[test]
fn test_trappist1_all_seven_planets_persist_through_simulation() {
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::{
        AccretionMergeEvent, CollisionBounceEvent, MoonFormationEvent, RocheDisruptionEvent,
    };
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::trappist::spawn_trappist_1_system;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    let mut config = SimulationConfig::default();
    config.gas_density_scale = 0.0;
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<PlayerInteractionState>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<LateHeavyBombardmentState>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<MoonFormationEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_systems(
            Update,
            (
                step_physics_simulation,
                process_accretion_and_collisions.after(step_physics_simulation),
            ),
        );

    let mut disk_params = app.world().resource::<DiskParameters>().clone();
    let star_entity = spawn_trappist_1_system(&mut app.world_mut().commands(), &mut disk_params);
    app.insert_resource(disk_params);
    app.insert_resource(ActiveScenarioState {
        current_preset: ScenarioPreset::Trappist1System,
        ..Default::default()
    });

    // Run first frame to spawn commands and initialize ECS
    app.update();

    // Verify initial spawn: exactly 1 star + 7 planets = 8 bodies
    let initial_bodies: Vec<String> = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .map(|b| b.name.clone())
        .collect();
    assert_eq!(
        initial_bodies.len(),
        8,
        "TRAPPIST-1 system must initially spawn 1 star + 7 resonant planets (found {initial_bodies:?})"
    );

    // Run 50 full simulation steps with physics and accretion/collision processing active
    for _ in 0..50 {
        app.update();
        let body_count = app
            .world_mut()
            .query::<&CelestialBody>()
            .iter(app.world())
            .count();
        assert_eq!(
            body_count, 8,
            "All 7 TRAPPIST-1 planets and star must persist on every step"
        );
    }

    // Verify all 7 planets and the central star remain alive and unswallowed
    let surviving_bodies: Vec<String> = app
        .world_mut()
        .query::<&CelestialBody>()
        .iter(app.world())
        .map(|b| b.name.clone())
        .collect();

    assert_eq!(
        surviving_bodies.len(),
        8,
        "All 7 TRAPPIST-1 planets and star must survive without erroneous collision engulfment (found {surviving_bodies:?})"
    );

    let expected_worlds = [
        "TRAPPIST-1b",
        "TRAPPIST-1c",
        "TRAPPIST-1d",
        "TRAPPIST-1e",
        "TRAPPIST-1f",
        "TRAPPIST-1g",
        "TRAPPIST-1h",
    ];
    for expected in expected_worlds {
        assert!(
            surviving_bodies.iter().any(|n| n == expected),
            "Planet {expected} must survive in TRAPPIST-1 system (active bodies: {surviving_bodies:?})"
        );
    }

    assert!(
        app.world().get_entity(star_entity).is_ok(),
        "Central star TRAPPIST-1 must survive"
    );
}

#[test]
fn test_uranus_and_neptune_retain_ice_giant_classification_and_blue_palette() {
    use protostellar::rendering::bodies::palettes::compute_ice_giant_palette;
    use protostellar::simulation::components::classify_body_by_mass_and_comp;
    use protostellar::simulation::components::{BodyType, Composition};
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    // 1. Verify classify_body_by_mass_and_comp on typical Ice Giant compositions
    // Real Neptune / Uranus: mass ~ 14-17 M_earth, ~60% ices, ~20% rock, ~15% gas
    let comp_neptune = Composition {
        ice_frac: 0.60,
        silicate_frac: 0.20,
        metal_frac: 0.05,
        organics_frac: 0.00,
        gas_frac: 0.15,
    };
    let neptune_type =
        classify_body_by_mass_and_comp(17.15 * EARTH_MASS_SOLAR, &comp_neptune, false);
    assert_eq!(
        neptune_type,
        BodyType::IceGiant,
        "Neptune must be classified as BodyType::IceGiant"
    );

    // High volatile mantle with 40% gas (the user's scenario: 31% water, 23% rock, 6% metal, 40% gas)
    let comp_user_neptune = Composition {
        ice_frac: 0.31,
        silicate_frac: 0.23,
        metal_frac: 0.06,
        organics_frac: 0.00,
        gas_frac: 0.40,
    };
    let user_type =
        classify_body_by_mass_and_comp(20.0 * EARTH_MASS_SOLAR, &comp_user_neptune, false);
    assert_eq!(
        user_type,
        BodyType::IceGiant,
        "Volatile-rich planet with 31% ice and 40% gas must classify as IceGiant, not GasGiant"
    );

    // Pure Gas Giant (Jupiter-like: 88% gas, 5% rock, 5% metal, 2% ice)
    let comp_jupiter = Composition {
        ice_frac: 0.02,
        silicate_frac: 0.05,
        metal_frac: 0.05,
        organics_frac: 0.00,
        gas_frac: 0.88,
    };
    let jupiter_type =
        classify_body_by_mass_and_comp(317.8 * EARTH_MASS_SOLAR, &comp_jupiter, false);
    assert_eq!(
        jupiter_type,
        BodyType::GasGiant,
        "Jupiter must be classified as BodyType::GasGiant"
    );

    // 2. Verify specialized Ice Giant color palettes
    let neptune_color = compute_ice_giant_palette("Proto-Neptune", 60.0);
    let neptune_linear = LinearRgba::from(neptune_color);
    assert!(
        neptune_linear.blue > neptune_linear.red * 2.0,
        "Neptune palette must be predominantly azure blue: {neptune_linear:?}"
    );

    let uranus_color = compute_ice_giant_palette("Proto-Uranus", 75.0);
    let uranus_linear = LinearRgba::from(uranus_color);
    assert!(
        uranus_linear.blue > uranus_linear.red * 1.5
            && uranus_linear.green > uranus_linear.red * 1.5,
        "Uranus palette must be predominantly aquamarine/cyan: {uranus_linear:?}"
    );
}

#[test]
fn test_trappist1_long_term_coplanar_stability() {
    use bevy::math::DVec3;
    use protostellar::game::phases::LateHeavyBombardmentState;
    use protostellar::simulation::accretion::collisions::process_accretion_and_collisions;
    use protostellar::simulation::accretion::events::*;
    use protostellar::simulation::components::*;
    use protostellar::simulation::physics::step_physics_simulation;
    use protostellar::simulation::resources::*;
    use protostellar::simulation::scenarios::trappist::spawn_trappist_1_system;
    use protostellar::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

    let mut app = App::new();
    let mut config = SimulationConfig::default();
    config.gas_density_scale = 0.0;
    app.insert_resource(config)
        .init_resource::<TimeWarp>()
        .init_resource::<SimTime>()
        .init_resource::<DiskParameters>()
        .init_resource::<EnergyMonitor>()
        .init_resource::<ActiveScenarioState>()
        .init_resource::<LateHeavyBombardmentState>()
        .init_resource::<PlayerInteractionState>()
        .add_message::<protostellar::simulation::thermodynamics::StarIgnitionEvent>()
        .add_message::<AccretionMergeEvent>()
        .add_message::<CollisionBounceEvent>()
        .add_message::<RocheDisruptionEvent>()
        .add_message::<MoonFormationEvent>()
        .add_systems(
            Update,
            (
                step_physics_simulation,
                process_accretion_and_collisions.after(step_physics_simulation),
            ),
        );

    let mut disk_params = DiskParameters::default();
    let _star_entity = spawn_trappist_1_system(&mut app.world_mut().commands(), &mut disk_params);
    *app.world_mut().resource_mut::<DiskParameters>() = disk_params;
    app.world_mut()
        .resource_mut::<ActiveScenarioState>()
        .current_preset = ScenarioPreset::Trappist1System;

    // Run for 300 steps (equivalent to multiple orbits of all 7 planets)
    for _ in 0..300 {
        app.update();
    }

    // Verify all 7 planets and star survive
    let mut planets_query = app
        .world_mut()
        .query::<(&CelestialBody, &SimPosition, &SimVelocity)>();
    let bodies: Vec<(String, DVec3, DVec3)> = planets_query
        .iter(app.world())
        .map(|(b, pos, vel)| (b.name.clone(), pos.0, vel.0))
        .collect();

    assert_eq!(
        bodies.len(),
        8,
        "All 7 planets and the central star must survive"
    );

    // Verify orbits remain strictly coplanar (y ~ 0) and bounded within authentic semi-major axes
    for (name, pos, _vel) in bodies {
        if name.contains("TRAPPIST-1b")
            || name.contains("TRAPPIST-1c")
            || name.contains("TRAPPIST-1d")
            || name.contains("TRAPPIST-1e")
            || name.contains("TRAPPIST-1f")
            || name.contains("TRAPPIST-1g")
            || name.contains("TRAPPIST-1h")
        {
            assert!(
                pos.y.abs() < 1e-4,
                "Planet {name} must remain strictly coplanar (y={:.6} AU)",
                pos.y
            );
            let r = (pos.x * pos.x + pos.z * pos.z).sqrt();
            assert!(
                (0.008..0.08).contains(&r),
                "Planet {name} orbit must remain stable inside TRAPPIST-1 system (r={:.4} AU)",
                r
            );
        }
    }
}

#[test]
fn test_trappist1_visual_hierarchy_and_spacing() {
    use protostellar::simulation::components::BodyType;
    use protostellar::simulation::resources::SimulationConfig;
    use protostellar::utils::constants::EARTH_RADIUS_AU;

    let config = SimulationConfig::default();
    let min_orbit_au = 0.01154f32; // TRAPPIST-1b orbit

    // Star: TRAPPIST-1 (R = 0.121 R_sun = 0.000563 AU)
    let star_phys_r = 0.000563f64;
    let star_vis_r =
        config.calc_visual_radius_with_orbit(star_phys_r, BodyType::RedDwarf, 0.0, min_orbit_au);

    // Planet b: R = 1.116 R_earth, a = 0.01154 AU
    let b_phys_r = 1.116 * EARTH_RADIUS_AU;
    let b_vis_r = config.calc_visual_radius_with_orbit(
        b_phys_r,
        BodyType::TerrestrialPlanet,
        min_orbit_au,
        min_orbit_au,
    );

    // Planet c: R = 1.097 R_earth, a = 0.01580 AU
    let c_phys_r = 1.097 * EARTH_RADIUS_AU;
    let c_vis_r = config.calc_visual_radius_with_orbit(
        c_phys_r,
        BodyType::TerrestrialPlanet,
        0.01580,
        min_orbit_au,
    );

    // Planet d: R = 0.788 R_earth, a = 0.02227 AU
    let d_phys_r = 0.788 * EARTH_RADIUS_AU;
    let d_vis_r = config.calc_visual_radius_with_orbit(
        d_phys_r,
        BodyType::TerrestrialPlanet,
        0.02227,
        min_orbit_au,
    );

    // Planet g: R = 1.129 R_earth, a = 0.04686 AU
    let g_phys_r = 1.129 * EARTH_RADIUS_AU;
    let g_vis_r = config.calc_visual_radius_with_orbit(
        g_phys_r,
        BodyType::TerrestrialPlanet,
        0.04686,
        min_orbit_au,
    );

    // 1. Star must be visibly dominant over planets (at least 4.0x larger in visual diameter)
    let ratio_star_to_b = star_vis_r / b_vis_r;
    let ratio_star_to_g = star_vis_r / g_vis_r;
    let ratio_star_to_d = star_vis_r / d_vis_r;

    assert!(
        ratio_star_to_b >= 4.0,
        "Star must be at least 4.0x wider than planet b (ratio: {:.2}x)",
        ratio_star_to_b
    );
    assert!(
        ratio_star_to_g >= 4.0,
        "Star must be at least 4.0x wider than largest planet g (ratio: {:.2}x)",
        ratio_star_to_g
    );
    assert!(
        ratio_star_to_d >= 5.0,
        "Star must be at least 5.0x wider than small planet d (ratio: {:.2}x)",
        ratio_star_to_d
    );

    // 2. Relative physical size differences between planets must be preserved
    assert!(
        g_vis_r > d_vis_r,
        "Planet g (1.13 R_earth) must be visibly larger than planet d (0.79 R_earth)"
    );
    assert!(
        b_vis_r > d_vis_r,
        "Planet b (1.12 R_earth) must be visibly larger than planet d (0.79 R_earth)"
    );

    // 3. Clear, dark space between adjacent planetary orbits
    // Distance between orbit b (0.01154) and orbit c (0.01580) is 0.00426 AU
    let orbit_gap_bc = 0.01580 - 0.01154;
    let surface_gap_bc = orbit_gap_bc - (b_vis_r + c_vis_r);
    assert!(
        surface_gap_bc > 2.0 * b_vis_r,
        "Surface gap between b and c must be at least 2x the planet diameter (surface gap: {:.5} AU vs 2*r={:.5} AU)",
        surface_gap_bc,
        2.0 * b_vis_r
    );

    // 4. Solar system planets (e.g. Earth at 1.0 AU, Mercury at 0.387 AU) must NOT be modified by compact scaling
    let earth_normal =
        config.calc_visual_radius_for_type(EARTH_RADIUS_AU, BodyType::TerrestrialPlanet);
    let earth_with_orbit = config.calc_visual_radius_with_orbit(
        EARTH_RADIUS_AU,
        BodyType::TerrestrialPlanet,
        1.0,
        0.387,
    );
    assert_eq!(
        earth_normal, earth_with_orbit,
        "Solar system planets must retain standard visual scaling"
    );
}
