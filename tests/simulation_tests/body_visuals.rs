//! Tests for celestial body material parameter generation and visual view overrides.

use bevy::math::DVec3;
use bevy::prelude::*;

use protostellar::rendering::bodies::atmospheres::build_planet_atmosphere_uniforms;
use protostellar::rendering::bodies::materials_sync::{
    compute_body_material_parameters, update_body_material_properties,
};
use protostellar::rendering::bodies::transforms::compute_view_visual_radius;
use protostellar::rendering::bodies::view_overrides::{
    has_strippable_gas_envelope, resolve_view_body_radius_and_type, stripped_interior_composition,
    stripped_interior_radius_au,
};
use protostellar::rendering::materials::{PlanetMaterial, PlanetMaterialExtension, PlanetUniforms};
use protostellar::simulation::components::*;
use protostellar::simulation::resources::SimulationConfig;
use protostellar::utils::constants::{
    AU_TO_KM, EARTH_MASS_SOLAR, EARTH_RADIUS_AU, JUPITER_MASS_SOLAR,
};

fn make_mat() -> PlanetMaterial {
    PlanetMaterial {
        base: StandardMaterial::default(),
        extension: PlanetMaterialExtension {
            uniforms: PlanetUniforms::default(),
        },
    }
}

#[test]
fn test_airless_moon_without_volatiles_renders_no_fake_ocean_or_air() {
    let comp = Composition {
        silicate_frac: 0.69,
        ice_frac: 0.05,
        metal_frac: 0.27,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let params = compute_body_material_parameters(
        BodyType::Moon,
        &comp,
        281.0,
        0.0081 * EARTH_MASS_SOLAR,
        None,
        None,
        None,
        None,
        None,
        None,
        0.0,
    );
    assert_eq!(params.pressure_bar, 0.0);
    assert_eq!(params.climate_and_bio.x, 0.0, "No fake liquid ocean");
    assert_eq!(params.climate_and_bio.y, 0.0, "No warm vacuum polar ice");
    assert_eq!(params.cloud_density, 0.0, "No fake clouds in vacuum");
    assert_eq!(params.atmosphere_params.z, 0.0, "No fake haze in vacuum");
}

#[test]
fn test_cold_airless_icy_moon_preserves_ice_without_liquid_ocean() {
    let comp = Composition {
        silicate_frac: 0.45,
        ice_frac: 0.50,
        metal_frac: 0.05,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let cold_params = compute_body_material_parameters(
        BodyType::Moon,
        &comp,
        100.0,
        0.0081 * EARTH_MASS_SOLAR,
        None,
        None,
        None,
        None,
        None,
        None,
        0.0,
    );
    assert_eq!(cold_params.pressure_bar, 0.0);
    assert_eq!(cold_params.climate_and_bio.x, 0.0);
    assert!(cold_params.climate_and_bio.y > 0.4);

    let below_triple = VolatileInventory {
        delivered_water_m_earth: 0.0001,
        ocean_coverage_frac: 0.5,
        atmospheric_pressure_bar: 0.004,
        cometary_impact_count: 0,
    };
    let low_p_params = compute_body_material_parameters(
        BodyType::Moon,
        &comp,
        281.0,
        0.0081 * EARTH_MASS_SOLAR,
        None,
        None,
        Some(&below_triple),
        None,
        None,
        None,
        0.0,
    );
    assert_eq!(
        low_p_params.climate_and_bio.x, 0.0,
        "Below 0.0061 bar triple point, liquid ocean must be zero"
    );
}

#[test]
fn test_stripped_interior_composition_normalizes_and_handles_pure_gas() {
    let planet_5au = Composition {
        silicate_frac: 0.64,
        ice_frac: 0.0,
        metal_frac: 0.20,
        organics_frac: 0.0,
        gas_frac: 0.16,
    };
    let stripped = stripped_interior_composition(&planet_5au);
    assert_eq!(stripped.gas_frac, 0.0);
    assert!((stripped.silicate_frac - 0.64 / 0.84).abs() < 1e-6);
    assert!((stripped.metal_frac - 0.20 / 0.84).abs() < 1e-6);

    let pure_gas = Composition {
        silicate_frac: 0.0,
        ice_frac: 0.0,
        metal_frac: 0.0,
        organics_frac: 0.0,
        gas_frac: 1.0,
    };
    let fallback = stripped_interior_composition(&pure_gas);
    assert_eq!(fallback.gas_frac, 0.0);
    assert!((fallback.silicate_frac + fallback.metal_frac - 1.0).abs() < 1e-6);
}

#[test]
fn test_gas_and_ice_giant_view_overrides_strip_envelope_or_clouds_independently() {
    let body_5au = CelestialBody {
        name: "PLANET-5AU".to_string(),
        body_type: BodyType::GasGiant,
    };
    let comp_5au = Composition {
        silicate_frac: 0.64,
        ice_frac: 0.0,
        metal_frac: 0.20,
        organics_frac: 0.0,
        gas_frac: 0.16,
    };
    let mass = Mass(122.52 * EARTH_MASS_SOLAR);
    let temp = Temperature(1022.0);
    let t = Transform::IDENTITY;

    let mut normal_mat = make_mat();
    update_body_material_properties(
        &mut normal_mat,
        &body_5au,
        &mass,
        &temp,
        &comp_5au,
        1.0,
        100.0,
        0.0,
        &t,
        0.05,
        Vec3::X,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        true,
    );
    assert_eq!(normal_mat.extension.uniforms.planet_type, 1);
    assert!(normal_mat.extension.uniforms.composition.w > 0.12);

    let mut cloudless_mat = make_mat();
    update_body_material_properties(
        &mut cloudless_mat,
        &body_5au,
        &mass,
        &temp,
        &comp_5au,
        1.0,
        100.0,
        0.0,
        &t,
        0.05,
        Vec3::X,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        false,
    );
    assert_eq!(cloudless_mat.extension.uniforms.planet_type, 1);
    assert!(cloudless_mat.extension.uniforms.climate_and_bio.w < -0.5);
    assert!(cloudless_mat.extension.uniforms.atmosphere_params.x > 10.0);

    let mut stripped_mat = make_mat();
    update_body_material_properties(
        &mut stripped_mat,
        &body_5au,
        &mass,
        &temp,
        &comp_5au,
        1.0,
        100.0,
        0.0,
        &t,
        0.05,
        Vec3::X,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        true,
    );
    assert_eq!(stripped_mat.extension.uniforms.planet_type, 3);
    assert_eq!(stripped_mat.extension.uniforms.composition.w, 0.0);
    assert_eq!(stripped_mat.extension.uniforms.atmosphere_params.x, 0.0);
    assert!(stripped_mat.extension.uniforms.atmosphere_params.y < 0.0);
    assert!(stripped_mat.extension.uniforms.dynamics_and_mag.y > 0.40);
}

#[test]
fn test_ice_giant_view_overrides_strip_envelope() {
    let neptune = CelestialBody {
        name: "Neptune".to_string(),
        body_type: BodyType::IceGiant,
    };
    let ice_comp = Composition {
        silicate_frac: 0.25,
        ice_frac: 0.55,
        metal_frac: 0.0,
        organics_frac: 0.0,
        gas_frac: 0.20,
    };
    let mut neptune_stripped = make_mat();
    update_body_material_properties(
        &mut neptune_stripped,
        &neptune,
        &Mass(0.054 * JUPITER_MASS_SOLAR),
        &Temperature(72.0),
        &ice_comp,
        1.0,
        100.0,
        0.0,
        &Transform::IDENTITY,
        0.03,
        Vec3::X,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        true,
    );
    assert_eq!(neptune_stripped.extension.uniforms.planet_type, 3);
    assert_eq!(neptune_stripped.extension.uniforms.composition.w, 0.0);
    assert!(neptune_stripped.extension.uniforms.climate_and_bio.y >= 0.45);
    assert_eq!(
        neptune_stripped.extension.uniforms.dynamics_and_mag.y, 0.0,
        "Cold icy giant interiors must not spawn molten magma patches"
    );
}

#[test]
fn test_stripped_interior_radius_shrinks_bloated_gas_envelope_to_core() {
    // Proto-Jupiter analog from Solar Nebula MMSN: 4.30 M_earth, 52,359 km, 88% gas
    let total_mass = 4.30 * EARTH_MASS_SOLAR;
    let total_rad_au = 52_359.0 / AU_TO_KM;
    let proto_jup_comp = Composition {
        silicate_frac: 0.04,
        ice_frac: 0.07,
        metal_frac: 0.01,
        organics_frac: 0.0,
        gas_frac: 0.88,
    };
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(total_mass, total_rad_au, &proto_jup_comp);

    let core_r = stripped_interior_radius_au(
        total_mass,
        total_rad_au,
        BodyType::GasGiant,
        &proto_jup_comp,
        Some(&diff),
    );
    assert!(
        core_r < total_rad_au * 0.22,
        "88% gas envelope stripped should reveal a core <22% of total radius (got {} vs {})",
        core_r * AU_TO_KM,
        total_rad_au * AU_TO_KM
    );
    assert!(
        core_r >= diff.mantle_radius_au,
        "Stripped core radius must enclose differentiated silicate mantle"
    );

    let rocky = Composition::rocky();
    let earth_core = stripped_interior_radius_au(
        EARTH_MASS_SOLAR,
        EARTH_RADIUS_AU,
        BodyType::TerrestrialPlanet,
        &rocky,
        None,
    );
    assert!((earth_core - EARTH_RADIUS_AU).abs() < 1e-12);

    let config = SimulationConfig::default();
    let vis_gas_on = compute_view_visual_radius(
        total_rad_au,
        BodyType::GasGiant,
        total_mass,
        Some(&proto_jup_comp),
        Some(&diff),
        true,
        5.31,
        0.39,
        0.002,
        &config,
    );
    let vis_gas_off = compute_view_visual_radius(
        total_rad_au,
        BodyType::GasGiant,
        total_mass,
        Some(&proto_jup_comp),
        Some(&diff),
        false,
        5.31,
        0.39,
        0.002,
        &config,
    );
    assert!(
        vis_gas_off < vis_gas_on * 0.25,
        "At the same zoom distance, stripping the 88% gas envelope must proportionally shrink the visual diameter (gas_on={vis_gas_on}, gas_off={vis_gas_off})"
    );
}

#[test]
fn test_terrestrial_ocean_preserved_when_clouds_or_atmosphere_hidden() {
    let earth = CelestialBody {
        name: "Earth".to_string(),
        body_type: BodyType::TerrestrialPlanet,
    };
    let comp = Composition::rocky();
    let vol = VolatileInventory {
        delivered_water_m_earth: 0.0003,
        ocean_coverage_frac: 0.71,
        atmospheric_pressure_bar: 1.0,
        cometary_impact_count: 10,
    };
    let climate = PlanetaryClimate {
        surface_temperature_k: 288.0,
        cloud_coverage_frac: 0.52,
        albedo: 0.30,
        ..Default::default()
    };
    let t = Transform::IDENTITY;

    let mut cloudless_earth = make_mat();
    update_body_material_properties(
        &mut cloudless_earth,
        &earth,
        &Mass(EARTH_MASS_SOLAR),
        &Temperature(288.0),
        &comp,
        1.0,
        100.0,
        0.0,
        &t,
        0.02,
        Vec3::X,
        Some(&climate),
        None,
        Some(&vol),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        false,
    );
    assert!(cloudless_earth.extension.uniforms.climate_and_bio.x > 0.65);
    assert!(cloudless_earth.extension.uniforms.climate_and_bio.w < -0.5);
    assert!(cloudless_earth.extension.uniforms.atmosphere_params.x >= 1.0);

    let (atmo_u, has_atmo, _) = build_planet_atmosphere_uniforms(
        &SimPosition(DVec3::X),
        0.02,
        &earth,
        &comp,
        EARTH_MASS_SOLAR,
        &Temperature(288.0),
        Some(&climate),
        Some(&vol),
        None,
        None,
        None,
        Some(&cloudless_earth),
        Vec3::ZERO,
        1.0,
        false,
    );
    assert!(
        has_atmo,
        "Rayleigh limb shell remains active when only clouds are hidden"
    );
    assert_eq!(atmo_u.cloud_and_time_params.x, 0.0);

    let mut airless_earth = make_mat();
    update_body_material_properties(
        &mut airless_earth,
        &earth,
        &Mass(EARTH_MASS_SOLAR),
        &Temperature(288.0),
        &comp,
        1.0,
        100.0,
        0.0,
        &t,
        0.02,
        Vec3::X,
        Some(&climate),
        None,
        Some(&vol),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        true,
    );
    assert!(
        airless_earth.extension.uniforms.climate_and_bio.x > 0.65,
        "Physical ocean remains visible when visually stripping atmosphere"
    );
    assert_eq!(airless_earth.extension.uniforms.atmosphere_params.x, 0.0);
    assert!(airless_earth.extension.uniforms.atmosphere_params.y < 0.0);
}

#[test]
fn test_terrestrial_planet_preserves_radius_when_atmosphere_hidden() {
    let earth_mass = 1.06 * EARTH_MASS_SOLAR;
    let earth_radius_au = 10_593.0 / AU_TO_KM;
    let comp_with_nebular_gas = Composition {
        metal_frac: 0.30,
        silicate_frac: 0.60,
        ice_frac: 0.01,
        organics_frac: 0.04,
        gas_frac: 0.05,
    };
    let mut diff = InternalDifferentiation::default();
    diff.recalculate(earth_mass, earth_radius_au, &comp_with_nebular_gas);

    assert!(!has_strippable_gas_envelope(
        BodyType::TerrestrialPlanet,
        &comp_with_nebular_gas
    ));
    let stripped_r = stripped_interior_radius_au(
        earth_mass,
        earth_radius_au,
        BodyType::TerrestrialPlanet,
        &comp_with_nebular_gas,
        Some(&diff),
    );
    assert!(
        (stripped_r - earth_radius_au).abs() < 1e-12,
        "Terrestrial planet radius must not shrink on Atmo OFF (got {} km vs {} km)",
        stripped_r * AU_TO_KM,
        earth_radius_au * AU_TO_KM
    );

    let (eff_r, eff_type) = resolve_view_body_radius_and_type(
        earth_mass,
        earth_radius_au,
        BodyType::TerrestrialPlanet,
        &comp_with_nebular_gas,
        Some(&diff),
        false,
    );
    assert!((eff_r - earth_radius_au).abs() < 1e-12);
    assert_eq!(eff_type, BodyType::TerrestrialPlanet);

    let config = SimulationConfig::default();
    let vis_atmo_on = compute_view_visual_radius(
        earth_radius_au,
        BodyType::TerrestrialPlanet,
        earth_mass,
        Some(&comp_with_nebular_gas),
        Some(&diff),
        true,
        1.0,
        0.39,
        0.002,
        &config,
    );
    let vis_atmo_off = compute_view_visual_radius(
        earth_radius_au,
        BodyType::TerrestrialPlanet,
        earth_mass,
        Some(&comp_with_nebular_gas),
        Some(&diff),
        false,
        1.0,
        0.39,
        0.002,
        &config,
    );
    assert!(
        (vis_atmo_off - vis_atmo_on).abs() < 1e-12,
        "Removing atmosphere for viewing must keep terrestrial visual radius identical (on={vis_atmo_on}, off={vis_atmo_off})"
    );

    // Super-Earth with secondary gas envelope also preserves its solid globe radius
    let super_earth_comp = Composition {
        metal_frac: 0.28,
        silicate_frac: 0.60,
        ice_frac: 0.0,
        organics_frac: 0.0,
        gas_frac: 0.12,
    };
    let se_rad = 1.8 * EARTH_RADIUS_AU;
    assert_eq!(
        stripped_interior_radius_au(
            3.0 * EARTH_MASS_SOLAR,
            se_rad,
            BodyType::SuperEarth,
            &super_earth_comp,
            None
        ),
        se_rad
    );
}

#[test]
fn test_terrestrial_material_preserves_surface_under_stripped_atmosphere() {
    let earth = CelestialBody {
        name: "Earth".to_string(),
        body_type: BodyType::TerrestrialPlanet,
    };
    let earth_mass = 1.06 * EARTH_MASS_SOLAR;
    let comp_with_nebular_gas = Composition {
        metal_frac: 0.30,
        silicate_frac: 0.60,
        ice_frac: 0.01,
        organics_frac: 0.04,
        gas_frac: 0.05,
    };
    let vol = VolatileInventory {
        delivered_water_m_earth: 0.0005,
        ocean_coverage_frac: 0.68,
        atmospheric_pressure_bar: 2.5,
        cometary_impact_count: 5,
    };
    let climate = PlanetaryClimate {
        surface_temperature_k: 367.0,
        cloud_coverage_frac: 0.65,
        albedo: 0.30,
        ..Default::default()
    };
    let mut stripped_earth_mat = make_mat();
    update_body_material_properties(
        &mut stripped_earth_mat,
        &earth,
        &Mass(earth_mass),
        &Temperature(367.0),
        &comp_with_nebular_gas,
        1.0,
        100.0,
        0.0,
        &Transform::IDENTITY,
        0.02,
        Vec3::X,
        Some(&climate),
        None,
        Some(&vol),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        true,
    );
    assert_eq!(stripped_earth_mat.extension.uniforms.planet_type, 3);
    assert!(stripped_earth_mat.extension.uniforms.climate_and_bio.x > 0.60);
    assert_eq!(stripped_earth_mat.extension.uniforms.climate_and_bio.w, 0.0);
    assert_eq!(
        stripped_earth_mat.extension.uniforms.atmosphere_params.x,
        0.0
    );
    assert!(stripped_earth_mat.extension.uniforms.atmosphere_params.y < 0.0);
}

#[test]
fn test_saturn_ring_system_scales_with_accretion_growth() {
    use protostellar::simulation::accretion::gas::direct_nebular_gas_accretion;
    use protostellar::simulation::pebble_accretion::apply_pebble_accretion;
    use protostellar::simulation::resources::{DiskParameters, SimTime, TimeWarp};

    let proto_r = EARTH_RADIUS_AU * 1.25;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(SimulationConfig::default());
    app.insert_resource(TimeWarp {
        is_paused: false,
        multiplier: 100.0,
        ..Default::default()
    });
    app.insert_resource(SimTime {
        elapsed_years: 10.0,
        current_dt_yr: 1.0,
        ..Default::default()
    });
    app.insert_resource(DiskParameters {
        central_star_mass: 1.0,
        inner_radius_au: 0.2,
        outer_radius_au: 45.0,
        disk_mass: 0.01,
        gas_disk_lifetime_yr: 5.0e6,
        snow_line_au: 2.7,
        ..Default::default()
    });
    app.add_systems(
        Update,
        (apply_pebble_accretion, direct_nebular_gas_accretion),
    );

    let saturn = app
        .world_mut()
        .spawn((
            CelestialBody {
                name: "Proto-Saturn".to_string(),
                body_type: BodyType::GasGiant,
            },
            Mass(2.20 * EARTH_MASS_SOLAR),
            SimPosition(DVec3::new(9.5, 0.0, 0.0)),
            Radius(proto_r),
            Composition::solar_gas(),
            PlanetaryRingSystem {
                inner_radius_au: (proto_r * 1.25) as f32,
                outer_radius_au: (proto_r * 2.35) as f32,
                ring_mass_earth: 0.000_028,
                optical_depth: 0.88,
                ice_fraction: 0.96,
                silicate_fraction: 0.04,
            },
        ))
        .id();

    app.update();

    let rad = app.world().get::<Radius>(saturn).unwrap().0;
    let rings = app.world().get::<PlanetaryRingSystem>(saturn).unwrap();
    assert!(
        rad > proto_r * 4.0,
        "Proto-Saturn should expand into a gas giant radius during accretion"
    );
    let inner_ratio = rings.inner_radius_au / rad as f32;
    let outer_ratio = rings.outer_radius_au / rad as f32;
    assert!(
        (1.20..=1.30).contains(&inner_ratio),
        "Ring inner ratio should stay outside cloud tops (~1.25 R_p), got {inner_ratio:.3}"
    );
    assert!(
        (2.25..=2.45).contains(&outer_ratio),
        "Ring outer ratio should preserve canonical Saturn span (~2.35 R_p), got {outer_ratio:.3}"
    );
}

#[test]
fn test_legacy_shrunken_saturn_rings_and_cassini_material_properties() {
    use protostellar::rendering::bodies::shadows::{
        compute_ring_shadow_params, resolve_effective_ring_ratios,
    };

    // Exact shrunken ring state from a legacy Proto-Saturn save (grown to 62,930 km while rings stayed at proto-core size)
    let grown_r_au = 62_930.0 / AU_TO_KM;
    let stale_rings = PlanetaryRingSystem {
        inner_radius_au: (9_955.0 / AU_TO_KM) as f32,
        outer_radius_au: (18_715.0 / AU_TO_KM) as f32,
        ring_mass_earth: 0.000_028,
        optical_depth: 0.88,
        ice_fraction: 0.96,
        silicate_fraction: 0.04,
    };

    let (inner_ratio, outer_ratio) = resolve_effective_ring_ratios(&stale_rings, grown_r_au);
    assert!(
        (1.22..=1.26).contains(&inner_ratio),
        "Recovered inner ring ratio should be ~1.24 R_p, got {inner_ratio:.3}"
    );
    assert!(
        (2.28..=2.38).contains(&outer_ratio),
        "Recovered outer ring ratio should be ~2.33 R_p instead of clamped 1.35, got {outer_ratio:.3}"
    );

    let shadow_params = compute_ring_shadow_params(Some(&stale_rings), grown_r_au);
    assert_eq!(shadow_params.x, inner_ratio);
    assert_eq!(shadow_params.y, outer_ratio);
    assert_eq!(shadow_params.w, 1.0);

    // Verify Saturn material uses matte velvet roughness and warm champagne-gold limb scattering
    let saturn = CelestialBody {
        name: "Proto-Saturn".to_string(),
        body_type: BodyType::GasGiant,
    };
    let mut saturn_mat = make_mat();
    update_body_material_properties(
        &mut saturn_mat,
        &saturn,
        &Mass(66.0 * EARTH_MASS_SOLAR),
        &Temperature(259.0),
        &Composition::solar_gas(),
        1.0,
        130.0,
        0.0,
        &Transform::IDENTITY,
        0.05,
        Vec3::X,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
        true,
    );
    assert!(
        saturn_mat.base.perceptual_roughness >= 0.90,
        "Saturn cloud deck should have matte velvet roughness >= 0.90"
    );
    let scatter = saturn_mat.extension.uniforms.scattering_params;
    assert!(
        scatter.x > scatter.z,
        "Saturn limb haze scattering should be warm champagne-gold (R > B), got {scatter:?}"
    );
}

#[test]
fn test_ring_scaling_is_symmetric_and_saturn_ratios_normalize() {
    use protostellar::rendering::bodies::shadows::resolve_effective_ring_ratios_for_body;

    let r0 = 5.33e-5_f64;
    let mut rings = PlanetaryRingSystem {
        inner_radius_au: (r0 * 1.25) as f32,
        outer_radius_au: (r0 * 2.35) as f32,
        ..Default::default()
    };
    let mut prev = r0;
    for next in [3.5e-4_f64, 1.2e-4, 4.2e-4] {
        rings.scale_with_host_radius(prev, next);
        prev = next;
        let inner = f64::from(rings.inner_radius_au) / next;
        let outer = f64::from(rings.outer_radius_au) / next;
        assert!((1.15..=1.35).contains(&inner), "inner ratio {inner:.3}");
        assert!((2.2..=2.5).contains(&outer), "outer ratio {outer:.3}");
    }

    let r = 3.5e-4_f64;
    let inflated = PlanetaryRingSystem {
        inner_radius_au: (r * 3.0) as f32,
        outer_radius_au: (r * 6.8) as f32,
        ..Default::default()
    };
    for name in ["Proto-Saturn", "Saturn"] {
        let (i, o) = resolve_effective_ring_ratios_for_body(&inflated, r, name);
        assert_eq!((i, o), (1.24, 2.35));
    }
    let (_, o) = resolve_effective_ring_ratios_for_body(&inflated, r, "Exoplanet-J1407b");
    assert!(o > 6.0, "non-Saturn wide rings must be untouched, got {o}");
}
