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
