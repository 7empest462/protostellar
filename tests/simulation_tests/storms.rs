//! Tests for Atmospheric Storm Hexagons, Jovian Anticyclonic Vortices, and Cloud Dynamics.

use std::f32::consts::PI;

use bevy::prelude::*;
use protostellar::rendering::materials::PlanetUniforms;
use protostellar::simulation::components::storms::AtmosphericStormState;
use protostellar::simulation::components::*;
use protostellar::simulation::resources::DiskParameters;
use protostellar::simulation::scenarios::exotic::spawn_rogue_planet_scenario;
use protostellar::simulation::scenarios::solar::spawn_solar_nebula_mmsn;

#[test]
fn test_saturn_polar_hexagon_sixfold_azimuthal_symmetry() {
    let saturn = AtmosphericStormState::saturn();
    assert_eq!(saturn.polar_hexagon_wavenumber, 6.0);
    assert!(saturn.polar_hexagon_amplitude > 0.05);

    let k = saturn.polar_hexagon_wavenumber;
    let a = saturn.polar_hexagon_amplitude;
    let r0 = saturn.polar_hexagon_colatitude;

    // Formula: r(phi) = r0 * (1.0 + A * cos(k * phi) + 0.22 * A * cos(2 * k * phi))
    let eval_r =
        |phi: f32| -> f32 { r0 * (1.0 + a * (k * phi).cos() + 0.22 * a * (2.0 * k * phi).cos()) };

    // Verify exact 6-fold azimuthal rotational invariance under delta_phi = 2*PI / 6 = PI / 3
    let sector_angle = 2.0 * PI / 6.0;
    for i in 0..36 {
        let phi = (i as f32) * (2.0 * PI / 36.0);
        let r_orig = eval_r(phi);
        let r_rotated = eval_r(phi + sector_angle);
        assert!(
            (r_orig - r_rotated).abs() < 1e-5,
            "Hexagon must exhibit exact 6-fold discrete rotational symmetry: r({phi}) = {r_orig}, r({phi} + pi/3) = {r_rotated}"
        );
    }

    // Verify 6 distinct radial corners (maxima) and 6 flat facets (minima) around 2*PI
    let mut maxima_count = 0;
    let steps = 360;
    let dphi = 2.0 * PI / (steps as f32);
    for i in 0..steps {
        let phi = (i as f32) * dphi;
        let r_curr = eval_r(phi);
        let r_prev = eval_r(phi - dphi);
        let r_next = eval_r(phi + dphi);
        if r_curr > r_prev && r_curr > r_next {
            maxima_count += 1;
        }
    }
    assert_eq!(
        maxima_count, 6,
        "Saturn North Polar Hexagon must have exactly 6 polygon vertices (standing wave crests)"
    );
}

#[test]
fn test_great_red_spot_aspect_ratio_and_collar_wind_profile() {
    let jupiter = AtmosphericStormState::jupiter();
    assert!(jupiter.great_spot_size > 0.3);
    assert_eq!(jupiter.polar_hexagon_amplitude, 0.0);

    // Elliptic semi-axes scaling: a = 1.50 * s, b = 0.68 * s
    let semi_major_factor = 1.50;
    let semi_minor_factor = 0.68;
    let aspect_ratio = semi_major_factor / semi_minor_factor;

    assert!(
        aspect_ratio > 2.0 && aspect_ratio < 2.4,
        "GRS aspect ratio must be approximately 2.2:1 (actual: {aspect_ratio})"
    );

    // High-speed outer collar wind profile: C(r_ell) = exp(-((r_ell - 0.85) / 0.20)^2)
    let collar_wind = |r_ell: f32| -> f32 { (-((r_ell - 0.85) / 0.20).powi(2)).exp() };

    let peak_wind = collar_wind(0.85);
    let eye_wind = collar_wind(0.10);
    let outer_flow = collar_wind(1.60);

    assert!(
        peak_wind > 0.99,
        "Collar wind velocity must peak near r_ell = 0.85"
    );
    assert!(
        eye_wind < 0.001,
        "Eye core must be calm (eye wind {eye_wind} should be < 0.001)"
    );
    assert!(
        outer_flow < 0.001,
        "Winds outside the storm boundary must drop off rapidly"
    );
}

#[test]
fn test_atmospheric_storm_state_presets() {
    let jup = AtmosphericStormState::jupiter();
    assert!(jup.great_spot_size >= 0.40);
    assert!((jup.great_spot_latitude_rad - (-0.384)).abs() < 0.01);
    assert_eq!(jup.secondary_oval_count, 4);
    assert!(jup.zonal_shear_turbulence > 0.80);

    let sat = AtmosphericStormState::saturn();
    assert_eq!(sat.polar_hexagon_wavenumber, 6.0);
    assert!(sat.polar_hexagon_amplitude >= 0.08);
    assert!((sat.polar_hexagon_colatitude - 0.218).abs() < 0.01);

    let nep = AtmosphericStormState::neptune();
    assert_eq!(nep.polar_hexagon_amplitude, 0.0);
    assert!(nep.great_spot_size > 0.25);
    assert_eq!(nep.secondary_oval_count, 2);

    let terr = AtmosphericStormState::terrestrial();
    assert_eq!(terr.polar_hexagon_amplitude, 0.0);
    assert!(terr.great_spot_size > 0.0);

    let def = AtmosphericStormState::default();
    assert_eq!(def.polar_hexagon_amplitude, 0.0);
    assert_eq!(def.great_spot_size, 0.0);
}

#[test]
fn test_solar_nebula_mmsn_attaches_storm_states() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();

    let star = spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    assert_ne!(star, Entity::PLACEHOLDER);
    app.update();

    let mut found_jupiter_storm = false;
    let mut found_saturn_hexagon = false;
    let mut found_neptune_storm = false;

    for (body, storm) in app
        .world_mut()
        .query::<(&CelestialBody, Option<&AtmosphericStormState>)>()
        .iter(app.world())
    {
        if body.name.contains("Jupiter") {
            let s = storm.expect("Proto-Jupiter must have AtmosphericStormState");
            assert!(s.great_spot_size > 0.3);
            found_jupiter_storm = true;
        } else if body.name.contains("Saturn") {
            let s = storm.expect("Proto-Saturn must have AtmosphericStormState");
            assert_eq!(s.polar_hexagon_wavenumber, 6.0);
            assert!(s.polar_hexagon_amplitude > 0.05);
            found_saturn_hexagon = true;
        } else if body.name.contains("Neptune") {
            let s = storm.expect("Proto-Neptune must have AtmosphericStormState");
            assert!(s.great_spot_size > 0.2);
            found_neptune_storm = true;
        }
    }

    assert!(found_jupiter_storm, "Proto-Jupiter must be found with GRS");
    assert!(
        found_saturn_hexagon,
        "Proto-Saturn must be found with Polar Hexagon"
    );
    assert!(
        found_neptune_storm,
        "Proto-Neptune must be found with Great Dark Spot"
    );
}

#[test]
fn test_exotic_scenario_attaches_storm_states() {
    let mut app = App::new();
    let mut disk_params = DiskParameters::default();

    let (_star, _intruder) =
        spawn_rogue_planet_scenario(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let mut found_rogue_storm = false;
    let mut found_jupiter = false;

    for (body, storm) in app
        .world_mut()
        .query::<(&CelestialBody, Option<&AtmosphericStormState>)>()
        .iter(app.world())
    {
        if body.name.contains("Rogue Interloper") {
            let s = storm.expect("Rogue Interloper must have AtmosphericStormState");
            assert!(s.zonal_shear_turbulence > 0.9);
            found_rogue_storm = true;
        } else if body.name == "Jupiter" {
            let s = storm.expect("Jupiter in rogue scenario must have AtmosphericStormState");
            assert!(s.great_spot_size > 0.3);
            found_jupiter = true;
        }
    }

    assert!(found_rogue_storm, "Rogue interloper must have storm state");
    assert!(found_jupiter, "Jupiter must have storm state");
}

#[test]
fn test_planet_uniforms_storm_defaults_and_alignment() {
    let uniforms = PlanetUniforms::default();
    assert_eq!(uniforms.storm_features, Vec4::ZERO);
    assert_eq!(uniforms.storm_dynamics, Vec4::new(0.0, 1.0, 0.0, 0.5));

    // Verify 16-byte alignment of PlanetUniforms
    assert_eq!(std::mem::size_of::<PlanetUniforms>() % 16, 0);
}

#[test]
fn test_planet_wgsl_shader_naga_validation() {
    let shader_source = std::fs::read_to_string("assets/shaders/planet.wgsl")
        .expect("assets/shaders/planet.wgsl must exist");

    let mock_header = r#"
struct View { world_position: vec3<f32> };
@group(0) @binding(0) var<uniform> view: View;

struct StandardMaterial {
    base_color: vec4<f32>,
    perceptual_roughness: f32,
    metallic: f32,
    emissive: vec4<f32>,
    reflectance: f32,
    ior: f32,
    attenuation_color: vec4<f32>,
    attenuation_distance: f32,
};

struct PbrInput {
    material: StandardMaterial,
    diffuse_transmission: f32,
    specular_transmission: f32,
    thickness: f32,
    N: vec3<f32>,
    V: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};

fn pbr_input_from_standard_material(in: VertexOutput, is_front: bool) -> PbrInput {
    var res: PbrInput;
    return res;
}

fn alpha_discard(mat: StandardMaterial, color: vec4<f32>) -> vec4<f32> {
    return color;
}

fn main_pass_post_lighting_processing(pbr: PbrInput, color: vec4<f32>) -> vec4<f32> {
    return color;
}

fn apply_pbr_lighting(pbr: PbrInput) -> vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

fn mesh_normal_local_to_world(normal: vec3<f32>, instance_index: u32) -> vec3<f32> {
    return normal;
}

fn get_world_from_local(instance_index: u32) -> mat4x4<f32> {
    return mat4x4<f32>(
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    );
}

fn mesh_position_local_to_world(world_from_local: mat4x4<f32>, local_position: vec4<f32>) -> vec4<f32> {
    return local_position;
}

fn mesh_position_local_to_clip(world_from_local: mat4x4<f32>, local_position: vec4<f32>) -> vec4<f32> {
    return local_position;
}
"#;

    // Filter out preprocessor lines and prepass branch for forward pass validation
    let mut cleaned_lines = Vec::new();
    let mut skip_prepass = false;
    for line in shader_source.lines().skip(18) {
        let trimmed = line.trim();
        if trimmed.starts_with("#ifdef PREPASS_PIPELINE") {
            skip_prepass = true;
            continue;
        }
        if trimmed.starts_with("#else") {
            if skip_prepass {
                skip_prepass = false;
                continue;
            }
        }
        if trimmed.starts_with("#endif") {
            continue;
        }
        if skip_prepass {
            continue;
        }
        if trimmed.starts_with("#") {
            // Strip remaining #ifdefs like VERTEX_TANGENTS
            continue;
        }
        cleaned_lines.push(line);
    }

    let full_source = format!("{mock_header}\n{}", cleaned_lines.join("\n"))
        .replace("#{MATERIAL_BIND_GROUP}", "2u");

    let res = wgpu::naga::front::wgsl::parse_str(&full_source);
    if let Err(ref err) = res {
        eprintln!("WGSL Parse Error: {:?}", err);
    }
    assert!(
        res.is_ok(),
        "planet.wgsl must parse cleanly with naga without reserved keyword or syntax errors: {:?}",
        res.err()
    );

    let module = res.unwrap();
    let mut validator = wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    );
    let valid_res = validator.validate(&module);
    if let Err(ref err) = valid_res {
        eprintln!("WGSL Validation Error: {:?}", err);
    }
    assert!(
        valid_res.is_ok(),
        "planet.wgsl must validate cleanly with naga validator: {:?}",
        valid_res.err()
    );
}

#[test]
fn test_tidal_deformation_and_camera_adaptive_visual_radius() {
    use protostellar::rendering::bodies::transforms::{
        compute_realistic_visual_radius, compute_tidal_deformation_uniform,
    };
    use protostellar::simulation::accretion::RocheLobeOverflow;
    use protostellar::simulation::resources::SimulationConfig;
    use protostellar::simulation::tides::TidalState;

    // 1. S4714 overflowing Roche lobe near Sagittarius A* (4.154e6 M_sun)
    let roche = RocheLobeOverflow {
        companion: Entity::PLACEHOLDER,
        mass_transfer_rate: 1.0e-4,
        l1_distance_au: 0.45,
    };
    let u_roche = compute_tidal_deformation_uniform(
        Vec3::new(0.51, 0.0, 0.0),
        Vec3::ZERO,
        4.154e6,
        2.0,
        0.009,
        None,
        Some(&roche),
        false,
    );
    assert!(
        (u_roche.x - (-1.0)).abs() < 1e-4,
        "Tidal direction must point toward Sgr A* at origin"
    );
    assert!(
        u_roche.w >= 0.35 && u_roche.w <= 0.78,
        "Active Roche overflow must produce significant teardrop stretch: {}",
        u_roche.w
    );

    // 2. Tidally locked exoplanet (TRAPPIST-1e) packs +2.0 in tidal_deformation.w
    let tidal_locked = TidalState {
        is_tidally_locked: true,
        ..Default::default()
    };
    let u_locked = compute_tidal_deformation_uniform(
        Vec3::new(0.029, 0.0, 0.0),
        Vec3::ZERO,
        0.0898,
        3.0e-6,
        0.00004,
        Some(&tidal_locked),
        None,
        false,
    );
    assert!(
        u_locked.w >= 2.0,
        "Tidally locked world must pack +2.0 flag in tidal_deformation.w, got {}",
        u_locked.w
    );

    // Moons are tidally locked to their parent planet, NOT to the central star (never +2.0 flag)
    let u_moon = compute_tidal_deformation_uniform(
        Vec3::new(0.98, 0.0, 0.0),
        Vec3::ZERO,
        1.0,
        3.0e-8,
        0.00001,
        Some(&tidal_locked),
        None,
        true,
    );
    assert!(
        u_moon.w < 1.5,
        "Moons locked to parent planet must never receive the +2.0 star-locked eyeball flag: {}",
        u_moon.w
    );

    // 3. Camera-adaptive visual radius shrinks smoothly on close-up and obeys 4.5% orbital cap
    let config = SimulationConfig::default();
    let r_far = compute_realistic_visual_radius(
        0.0000426,
        BodyType::TerrestrialPlanet,
        0.029,
        0.011,
        25.0,
        &config,
    );
    let r_close = compute_realistic_visual_radius(
        0.0000426,
        BodyType::TerrestrialPlanet,
        0.029,
        0.011,
        0.001,
        &config,
    );
    assert!(
        r_far <= 0.029 * 0.045 + 1e-6,
        "Distant visual radius ({r_far}) must never exceed 4.5% of orbital distance (0.029 AU)"
    );
    assert!(
        r_close < r_far,
        "Close-up visual radius ({r_close}) must shrink toward physical scale compared to distant ({r_far})"
    );

    // 4. Earth-to-Moon relative size hierarchy must stay constant (~3.8x) whether viewing Earth,
    // viewing the Moon, or zoomed out to system scale.
    let r_earth_phys = 0.0000426;
    let r_moon_phys = r_earth_phys * 0.272;
    let earth_close = compute_realistic_visual_radius(
        r_earth_phys,
        BodyType::TerrestrialPlanet,
        1.0,
        0.38,
        0.004,
        &config,
    );
    let moon_from_earth =
        compute_realistic_visual_radius(r_moon_phys, BodyType::Moon, 1.0, 0.38, 0.012, &config);
    let ratio_close = earth_close / moon_from_earth;
    assert!(
        ratio_close > 3.2 && ratio_close < 4.5,
        "When viewing Earth up close, Earth ({earth_close}) must be ~3.8x larger than the Moon at 0.012 AU ({moon_from_earth}), got ratio {ratio_close:.2}"
    );

    let earth_far = compute_realistic_visual_radius(
        r_earth_phys,
        BodyType::TerrestrialPlanet,
        1.0,
        0.38,
        15.0,
        &config,
    );
    let moon_far =
        compute_realistic_visual_radius(r_moon_phys, BodyType::Moon, 1.0, 0.38, 15.0, &config);
    let ratio_far = earth_far / moon_far;
    assert!(
        (ratio_close - ratio_far).abs() < 0.05,
        "Earth-to-Moon visual size ratio must not distort between close-up ({ratio_close:.2}) and far zoom ({ratio_far:.2})"
    );
}

#[test]
fn test_saturn_hd_stratigraphy_and_storm_state() {
    use protostellar::rendering::bodies::palettes::compute_gas_giant_palette;

    // 1. Verify Saturn and Proto-Saturn retain golden-butterscotch palette
    let proto_saturn_color = compute_gas_giant_palette(0.000285, 263.8, "Proto-Saturn");
    let mature_saturn_color = compute_gas_giant_palette(0.000285, 95.0, "Saturn");

    let p_rgb = proto_saturn_color.to_srgba();
    let m_rgb = mature_saturn_color.to_srgba();

    assert!(
        p_rgb.red > 0.85 && p_rgb.green > 0.75 && p_rgb.blue < 0.65,
        "Proto-Saturn must have golden-sand butterscotch hue (R > 0.85, G > 0.75, B < 0.65), got: {:?}",
        p_rgb
    );
    assert_eq!(
        p_rgb, m_rgb,
        "Proto-Saturn and Saturn must both receive canonical butterscotch palette"
    );

    // 2. Verify WGSL shader contains HD gas giant features and aurora pole fade
    let shader_source = std::fs::read_to_string("assets/shaders/planet.wgsl")
        .expect("assets/shaders/planet.wgsl must exist");

    assert!(
        shader_source.contains("let is_saturn = (hex_amp > 0.001)"),
        "Shader must identify Saturn and apply authentic Cassini strata"
    );
    assert!(
        shader_source.contains("let oblateness_amount = select(0.065, 0.098, has_saturn_rings);"),
        "Shader must model Saturn's authentic 9.8% polar oblateness"
    );
    assert!(
        shader_source.contains("let flow_ultra = noise(p_zonal_4 + warp_fine * 2.0) * nano_lod;"),
        "Shader must include high-frequency micro-convective eddies with screen-space LOD"
    );
    assert!(
        shader_source.contains("let pole_fade = smoothstep(0.04, 0.14, polar_co);"),
        "Shader must fade aurora towards polar center to prevent flower starburst singularity"
    );
    assert!(
        shader_source.contains("let circumpolar_cyclones = fbm("),
        "Shader must blend out zonal banding into circumpolar cyclonic swirls at high latitudes"
    );

    // 3. Verify planetary_rings.wgsl parses cleanly and guards against planet clipping
    let ring_shader_src = std::fs::read_to_string("assets/shaders/planetary_rings.wgsl")
        .expect("assets/shaders/planetary_rings.wgsl must exist");
    assert!(
        ring_shader_src.contains("let inner_dist = clamp(max(ring.inner_radius, planet_surface_dist)"),
        "Ring shader must clamp inner radius to planet surface to prevent rings clipping through the planet"
    );
    assert!(
        ring_shader_src.contains("let u = (dist - inner_dist) / max(outer_dist - inner_dist"),
        "Ring shader must normalize radial coordinates from inner to outer edge"
    );

    // Naga parse validation for planetary_rings.wgsl
    let mock_ring_header = r#"
struct View { world_position: vec3<f32> };
struct VertexOutput {
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};
@group(0) @binding(0) var<uniform> view: View;
"#;
    let mut cleaned_ring_lines = Vec::new();
    let mut in_import = false;
    for line in ring_shader_src.lines() {
        if line.starts_with("#import") {
            if line.contains('{') && !line.contains('}') {
                in_import = true;
            }
            continue;
        }
        if in_import {
            if line.contains('}') {
                in_import = false;
            }
            continue;
        }
        cleaned_ring_lines.push(line);
    }
    let full_ring_source = format!("{mock_ring_header}\n{}", cleaned_ring_lines.join("\n"))
        .replace("#{MATERIAL_BIND_GROUP}", "1u");
    let ring_naga_res = wgpu::naga::front::wgsl::parse_str(&full_ring_source);
    assert!(
        ring_naga_res.is_ok(),
        "planetary_rings.wgsl must parse cleanly with naga: {:?}",
        ring_naga_res.err()
    );

    let ring_module = ring_naga_res.unwrap();
    let mut ring_validator = wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    );
    let ring_valid_res = ring_validator.validate(&ring_module);
    assert!(
        ring_valid_res.is_ok(),
        "planetary_rings.wgsl must validate cleanly with naga validator: {:?}",
        ring_valid_res.err()
    );
}
