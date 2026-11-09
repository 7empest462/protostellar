//! Guardrail integration tests for WESL/WGSL shader bind groups, Bevy 0.20 ShaderCache + Naga
//! validation across all 15 material shaders and 5 raw wgpu shaders, and safe ECS despawn idempotence.

use std::sync::OnceLock;

use bevy::asset::AssetId;
use bevy::prelude::*;
use bevy_shader::{ShaderCache, ShaderCacheSource, ShaderDefVal};
use protostellar::rendering::materials::{BlackHoleDiskMaterial, BlackHoleDiskUniforms};

fn bevy_embedded_shaders() -> &'static Vec<(AssetId<Shader>, Shader)> {
    static EMBEDDED: OnceLock<Vec<(AssetId<Shader>, Shader)>> = OnceLock::new();
    EMBEDDED.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::log::LogPlugin>()
                .disable::<bevy::winit::WinitPlugin>()
                .set(bevy::window::WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
                .set(bevy::render::RenderPlugin {
                    render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                        bevy::render::settings::WgpuSettings {
                            backends: None,
                            ..default()
                        },
                    )),
                    ..default()
                }),
        );
        app.add_plugins(bevy::ui_render::UiMaterialPlugin::<
            protostellar::game::ui::sci_fi_theme::SciFiPanelMaterial,
        >::default());
        app.add_plugins(protostellar::rendering::shaders::ProtostellarShadersPlugin);
        app.finish();
        app.cleanup();

        // Pump the asset server until all embedded WESL libraries registered via
        // `load_shader_library!` have finished loading into `Assets<Shader>` and stabilized.
        let mut prev_count = 0;
        let mut stable_ticks = 0;
        for _ in 0..200 {
            app.update();
            let count = app.world().resource::<Assets<Shader>>().len();
            if count > 0 && count == prev_count {
                stable_ticks += 1;
                if stable_ticks >= 8 {
                    break;
                }
            } else {
                prev_count = count;
                stable_ticks = 0;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let shaders = app.world().resource::<Assets<Shader>>();
        assert!(
            shaders.len() >= 50,
            "Expected Bevy embedded WESL shader libraries to load (loaded {})",
            shaders.len()
        );
        shaders
            .iter()
            .map(|(id, shader)| (id, shader.clone()))
            .collect()
    })
}

/// Compiles a `.wesl` shader through Bevy 0.20's real `ShaderCache` against Bevy's embedded
/// WESL library and validates the resulting WGSL module with `wgpu::naga` (syntax + full typecheck).
pub fn compile_and_validate_wesl_shader(shader_path: &str, extra_defs: &[ShaderDefVal]) -> String {
    let source = std::fs::read_to_string(shader_path)
        .unwrap_or_else(|e| panic!("{shader_path} must exist: {e:?}"));

    let mut cache: ShaderCache<String, ()> = ShaderCache::new((), |_, src, _| match src {
        ShaderCacheSource::Wgsl(wgsl) => Ok(wgsl),
        ShaderCacheSource::SpirV(_) => panic!("Expected WGSL output from WESL compilation"),
    });

    for (id, shader) in bevy_embedded_shaders() {
        cache.set_shader(*id, shader.clone());
    }

    let asset_rel_path = shader_path.strip_prefix("assets/").unwrap_or(shader_path);
    let root_id = AssetId::Uuid {
        uuid: bevy::asset::uuid::Uuid::from_u128(0xDEAD_BEEF_CAFE_0001),
    };
    cache.set_shader(root_id, Shader::from_wesl(source, asset_rel_path));

    let mut defs = vec![
        ShaderDefVal::UInt("MATERIAL_BIND_GROUP".into(), 3),
        ShaderDefVal::UInt("AVAILABLE_STORAGE_BUFFER_BINDINGS".into(), 8),
        ShaderDefVal::Bool("VERTEX_POSITIONS".into(), true),
        ShaderDefVal::Bool("VERTEX_UVS".into(), true),
        ShaderDefVal::Bool("VERTEX_UVS_A".into(), true),
        ShaderDefVal::Bool("VERTEX_NORMALS".into(), true),
        ShaderDefVal::Bool("VERTEX_TANGENTS".into(), true),
        ShaderDefVal::Bool("VERTEX_COLORS".into(), true),
        ShaderDefVal::Bool("VERTEX_OUTPUT_INSTANCE_INDEX".into(), true),
        ShaderDefVal::Bool("TONEMAPPING_IN_SHADER".into(), true),
        ShaderDefVal::Bool("DEBAND_DITHER".into(), true),
    ];
    defs.extend_from_slice(extra_defs);

    let compiled = cache.get(0, root_id, &defs).unwrap_or_else(|err| {
        panic!("Bevy 0.20 WESL compilation failed for {shader_path}:\n{err}")
    });

    let module = wgpu::naga::front::wgsl::parse_str(&compiled).unwrap_or_else(|err| {
        panic!(
            "{shader_path} compiled WESL produced invalid WGSL syntax:\n{}\n--- WGSL ---\n{}",
            err.emit_to_string(&compiled),
            compiled
        )
    });

    let mut validator = wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    );
    validator.validate(&module).unwrap_or_else(|err| {
        panic!("{shader_path} compiled WESL failed Naga type/semantic validation:\n{err:?}")
    });

    (*compiled).clone()
}

#[test]
fn test_embedded_astrophysics_wesl_libraries_compilation_and_import() {
    let test_src = r#"
import protostellar::rendering::shaders::noise::{noise, fbm, hash3};
import protostellar::rendering::shaders::scattering::{dual_lobe_mie_phase, chapman_spherical_airmass};
import protostellar::rendering::shaders::blackbody::{blackbody_temperature_to_rgb, doppler_shift_temperature};

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    let p = vec3<f32>(1.0, 2.0, 3.0);
    let n = noise(p);
    let f = fbm(p);
    let h = hash3(p);
    let mie = dual_lobe_mie_phase(0.5, 0.8);
    let airmass = chapman_spherical_airmass(100.0, 0.5);
    let star_rgb = blackbody_temperature_to_rgb(5778.0);
    let t_shifted = doppler_shift_temperature(5778.0, 0.05);

    let val = n + f + h + mie + airmass + t_shifted * 0.0001;
    return vec4<f32>(star_rgb * val, 1.0);
}
"#;

    let mut cache: ShaderCache<String, ()> = ShaderCache::new((), |_, src, _| match src {
        ShaderCacheSource::Wgsl(wgsl) => Ok(wgsl),
        ShaderCacheSource::SpirV(_) => panic!("Expected WGSL output from WESL compilation"),
    });

    for (id, shader) in bevy_embedded_shaders() {
        cache.set_shader(*id, shader.clone());
    }

    let root_id = AssetId::Uuid {
        uuid: bevy::asset::uuid::Uuid::from_u128(0xCAFE_BABE_0001_0001),
    };
    cache.set_shader(root_id, Shader::from_wesl(test_src, "test_shader"));

    let defs = vec![];
    let compiled = cache.get(0, root_id, &defs).unwrap_or_else(|err| {
        panic!(
            "Failed to compile test shader importing protostellar embedded WESL libraries:\n{err}"
        )
    });

    let module = wgpu::naga::front::wgsl::parse_str(&compiled).unwrap_or_else(|err| {
        panic!(
            "Compiled WESL produced invalid WGSL syntax:\n{}\n--- WGSL ---\n{}",
            err.emit_to_string(&compiled),
            compiled
        )
    });

    let mut validator = wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    );
    validator.validate(&module).unwrap_or_else(|err| {
        panic!("Compiled WESL failed Naga type/semantic validation:\n{err:?}")
    });
}

#[test]
fn test_all_material_shaders_use_preprocessed_bind_group() {
    let shaders_dir = std::path::Path::new("assets/shaders");
    assert!(shaders_dir.exists(), "assets/shaders directory must exist");

    let entries = std::fs::read_dir(shaders_dir).expect("Failed to read shaders dir");
    let mut checked_wesl_shaders = 0;
    let mut checked_raw_wgsl_shaders = 0;

    for entry in entries {
        let entry = entry.expect("Valid dir entry");
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str());
        let filename = path.file_name().unwrap().to_str().unwrap();
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {filename}: {e:?}"));

        // Zero legacy naga_oil directives may exist in any shader
        assert!(
            !content.contains("#import")
                && !content.contains("#ifdef")
                && !content.contains("#{MATERIAL_BIND_GROUP}"),
            "Shader {filename} still contains removed Bevy 0.19 naga_oil preprocessor syntax!"
        );

        if ext == Some("wesl") {
            // UiMaterial shaders legitimately bind at @group(1)
            if filename == "sci_fi_panel.wesl" {
                continue;
            }

            // FullscreenMaterial shaders legitimately bind at @group(0)
            if filename == "post_processing.wesl" {
                assert!(
                    content.contains("@group(0) @binding(0)"),
                    "FullscreenMaterial shader post_processing.wesl must bind at @group(0)"
                );
                checked_wesl_shaders += 1;
                continue;
            }

            assert!(
                !content.contains("@group(1)"),
                "Shader {filename} hardcodes @group(1), which collides with pipeline layouts! Use @group(constants::MATERIAL_BIND_GROUP)."
            );
            assert!(
                !content.contains("@group(2)"),
                "Shader {filename} hardcodes @group(2), which collides with Bevy mesh storage buffers! Use @group(constants::MATERIAL_BIND_GROUP)."
            );

            if content.contains("@binding(0)") || content.contains("@binding(101)") {
                assert!(
                    content.contains("@group(constants::MATERIAL_BIND_GROUP)"),
                    "Material shader {filename} must use @group(constants::MATERIAL_BIND_GROUP) for Bevy 0.20 WESL bind group compatibility."
                );
            }

            checked_wesl_shaders += 1;
        } else if ext == Some("wgsl") {
            checked_raw_wgsl_shaders += 1;
        }
    }

    assert_eq!(
        checked_wesl_shaders, 14,
        "Expected 14 non-UI .wesl material shaders (plus sci_fi_panel.wesl = 15 total)"
    );
    assert_eq!(
        checked_raw_wgsl_shaders, 5,
        "Expected 5 standalone raw wgpu .wgsl shaders"
    );
}

#[test]
fn test_all_15_wesl_shaders_compile_through_bevy_shader_cache_and_naga() {
    let wesl_shaders = [
        "assets/shaders/atmosphere.wesl",
        "assets/shaders/black_hole_disk.wesl",
        "assets/shaders/circumstellar.wesl",
        "assets/shaders/comet_tail.wesl",
        "assets/shaders/gas_cloud.wesl",
        "assets/shaders/planet.wesl",
        "assets/shaders/planetary_rings.wesl",
        "assets/shaders/post_processing.wesl",
        "assets/shaders/relativistic_jet.wesl",
        "assets/shaders/roche_stream.wesl",
        "assets/shaders/sci_fi_panel.wesl",
        "assets/shaders/skybox.wesl",
        "assets/shaders/soft_particle.wesl",
        "assets/shaders/supernova.wesl",
        "assets/shaders/volumetric_nebula.wesl",
    ];

    for shader_path in wesl_shaders {
        compile_and_validate_wesl_shader(shader_path, &[]);
    }

    // Also validate planet.wesl under PREPASS_PIPELINE + DEFERRED_PREPASS
    compile_and_validate_wesl_shader(
        "assets/shaders/planet.wesl",
        &[
            ShaderDefVal::Bool("PREPASS_PIPELINE".into(), true),
            ShaderDefVal::Bool("PREPASS_FRAGMENT".into(), true),
            ShaderDefVal::Bool("DEFERRED_PREPASS".into(), true),
            ShaderDefVal::Bool("NORMAL_PREPASS_OR_DEFERRED_PREPASS".into(), true),
        ],
    );
}

#[test]
fn test_all_8_transparent_wesl_shaders_compile_under_forward_pipeline() {
    let transparent_shaders = [
        "assets/shaders/planetary_rings.wesl",
        "assets/shaders/atmosphere.wesl",
        "assets/shaders/gas_cloud.wesl",
        "assets/shaders/roche_stream.wesl",
        "assets/shaders/relativistic_jet.wesl",
        "assets/shaders/black_hole_disk.wesl",
        "assets/shaders/supernova.wesl",
        "assets/shaders/comet_tail.wesl",
    ];

    for shader_path in transparent_shaders {
        compile_and_validate_wesl_shader(shader_path, &[]);
    }
}

/// Validates that a WESL material shader contains the expected uniform struct and compiles cleanly
/// through Bevy's WESL `ShaderCache` + `wgpu::naga`.
pub fn validate_wgsl_shader_with_naga(shader_path: &str, struct_name: &str) {
    let shader_source =
        std::fs::read_to_string(shader_path).unwrap_or_else(|_| panic!("{shader_path} must exist"));
    assert!(
        shader_source.contains(struct_name),
        "{struct_name} must exist in {shader_path}"
    );
    compile_and_validate_wesl_shader(shader_path, &[]);
}

#[test]
fn test_black_hole_disk_wgsl_naga_validation() {
    validate_wgsl_shader_with_naga(
        "assets/shaders/black_hole_disk.wesl",
        "struct BlackHoleDiskUniforms",
    );
}

#[test]
fn test_roche_stream_wgsl_naga_validation() {
    use bevy::camera::primitives::MeshAabb;
    use bevy::mesh::VertexAttributeValues;

    validate_wgsl_shader_with_naga(
        "assets/shaders/roche_stream.wesl",
        "struct RocheStreamUniforms",
    );
    let mesh = protostellar::rendering::effects::roche_stream::generate_curved_roche_tube_mesh();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("Roche stream mesh must have Float32x3 ATTRIBUTE_POSITION");
    };
    for (idx, pos) in positions.iter().enumerate() {
        assert!(
            pos[0].is_finite() && pos[1].is_finite() && pos[2].is_finite(),
            "Vertex position at index {idx} must be finite (no NaN/Inf), got {pos:?}"
        );
    }

    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!("Roche stream mesh must have Float32x3 ATTRIBUTE_NORMAL");
    };
    for (idx, norm) in normals.iter().enumerate() {
        assert!(
            norm[0].is_finite() && norm[1].is_finite() && norm[2].is_finite(),
            "Vertex normal at index {idx} must be finite (no NaN/Inf), got {norm:?}"
        );
    }

    let aabb = mesh
        .get_aabb()
        .expect("Roche stream mesh must compute a valid Aabb");
    assert!(
        aabb.center.is_finite() && aabb.half_extents.is_finite(),
        "Roche stream Aabb must be finite so frustum culling never discards the stream: {aabb:?}"
    );
    assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
    assert!(mesh.indices().is_some());
}

fn validate_wgsl_compute_shader_with_naga(shader_path: &str) {
    let source = std::fs::read_to_string(shader_path)
        .unwrap_or_else(|_| panic!("{} must exist", shader_path));
    let res = wgpu::naga::front::wgsl::parse_str(&source);
    if let Err(ref err) = res {
        eprintln!("{} WGSL Parse Error: {:?}", shader_path, err);
    }
    assert!(
        res.is_ok(),
        "{} must parse cleanly with naga without syntax errors: {:?}",
        shader_path,
        res.err()
    );
}

#[test]
fn test_gmc_fluid_wgsl_naga_validation() {
    validate_wgsl_compute_shader_with_naga("assets/shaders/gmc_fluid.wgsl");
}

#[test]
fn test_gmc_far_field_wgsl_naga_validation() {
    validate_wgsl_compute_shader_with_naga("assets/shaders/gmc_far_field.wgsl");
}

#[test]
fn test_nbody_galaxy_wgsl_naga_validation() {
    validate_wgsl_compute_shader_with_naga("assets/shaders/nbody_galaxy.wgsl");
}

#[test]
fn test_volumetric_nebula_wgsl_naga_validation() {
    validate_wgsl_shader_with_naga(
        "assets/shaders/volumetric_nebula.wesl",
        "struct VolumetricNebulaUniforms",
    );
}

#[test]
fn test_post_processing_wgsl_naga_validation() {
    validate_wgsl_shader_with_naga(
        "assets/shaders/post_processing.wesl",
        "struct PostProcessingSettings",
    );
}

#[test]
fn test_post_processing_settings_uniform_alignment_and_fullscreen_material() {
    use bevy::core_pipeline::fullscreen_material::FullscreenMaterial;
    use protostellar::rendering::post_processing::PostProcessingSettings;

    let size = std::mem::size_of::<PostProcessingSettings>();
    assert_eq!(
        size % 16,
        0,
        "PostProcessingSettings uniform struct size must be 16-byte aligned for WGSL (got {size} bytes)"
    );

    let shader = PostProcessingSettings::fragment_shader();
    match shader {
        bevy::shader::ShaderRef::Path(path) => {
            assert_eq!(
                path.path().to_str().unwrap(),
                "shaders/post_processing.wesl",
                "PostProcessingSettings must point to shaders/post_processing.wesl"
            );
        }
        _ => panic!("Expected ShaderRef::Path"),
    }
}

#[test]
fn test_black_hole_disk_uniform_alignment_and_material() {
    // WGSL uniforms require struct size to be a multiple of 16 bytes
    let size = std::mem::size_of::<BlackHoleDiskUniforms>();
    assert_eq!(
        size % 16,
        0,
        "BlackHoleDiskUniforms size ({size} bytes) must be a multiple of 16 for WGSL alignment"
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<BlackHoleDiskMaterial>();

    let mut materials = app
        .world_mut()
        .resource_mut::<Assets<BlackHoleDiskMaterial>>();
    let handle = materials.add(BlackHoleDiskMaterial {
        uniforms: BlackHoleDiskUniforms {
            inner_radius: 0.16,
            outer_radius: 1.0,
            schwa_radius: 0.055,
            time: 1.0,
            disk_color: Vec4::ONE,
            spin_axis: Vec4::new(0.0, 1.0, 0.0, 10.0),
            cam_dir_local: Vec4::new(0.0, 1.0, 0.0, 0.0),
            cam_up_local: Vec4::new(0.0, 0.0, 1.0, 0.0),
        },
    });

    let retrieved = materials.get(&handle);
    assert!(
        retrieved.is_some(),
        "BlackHoleDiskMaterial must be safely retrievable from Assets"
    );
}

#[test]
fn test_entity_try_despawn_idempotence_and_safety() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let e1 = app.world_mut().spawn_empty().id();
    let e2 = app.world_mut().spawn_empty().id();

    // 1. Queue try_despawn on e1
    {
        let mut commands = app.world_mut().commands();
        if let Ok(mut cmd) = commands.get_entity(e1) {
            cmd.try_despawn();
        }
    }
    app.update();

    // Verify e1 is despawned
    assert!(app.world().get_entity(e1).is_err(), "e1 must be despawned");

    // 2. Queue try_despawn AGAIN on the already-despawned e1; this must NOT panic
    {
        let mut commands = app.world_mut().commands();
        // Even if an entity id is given directly to entity(e1).try_despawn()
        commands.entity(e1).try_despawn();
        // e2 also safely despawns
        if let Ok(mut cmd) = commands.get_entity(e2) {
            cmd.try_despawn();
        }
    }
    app.update();

    assert!(app.world().get_entity(e2).is_err(), "e2 must be despawned");
}

/// Asserts that a shader source file contains all required string patterns
/// and does not contain any forbidden string patterns.
pub(crate) fn assert_shader_patterns(
    shader_path: &str,
    required: &[(&str, &str)],
    forbidden: &[(&str, &str)],
) {
    let content = std::fs::read_to_string(shader_path)
        .unwrap_or_else(|err| panic!("{shader_path} must exist and be readable: {err:?}"));

    for &(pattern, message) in required {
        assert!(
            content.contains(pattern),
            "{message} (missing required pattern: {pattern})"
        );
    }
    for &(pattern, message) in forbidden {
        assert!(
            !content.contains(pattern),
            "{message} (found forbidden pattern: {pattern})"
        );
    }
}

#[test]
fn test_planet_wgsl_ocean_hypsometry_guardrails() {
    assert_shader_patterns(
        "assets/shaders/planet.wesl",
        &[
            (
                "let is_exposed_seabed = !has_oceans && (elev < primordial_shelf_elev);",
                "planet.wgsl must guard exposed seabed by !has_oceans on terrestrial planets",
            ),
            (
                "let is_exposed_seabed = !has_oceans && (combined_elev < primordial_shelf_elev);",
                "planet.wgsl must guard exposed seabed by !has_oceans on super-earths",
            ),
            (
                "let ocean_base = sea_level - 0.22;",
                "planet.wgsl must depress oceanic basin base level safely below sea_level",
            ),
            (
                "let ocean_normal = normalize(surface_dir + wave_pert_world);",
                "planet.wgsl must compute smooth liquid capillary swell for open water",
            ),
            (
                "let has_volatile_ice = has_volatiles && (ice_frac > 0.005 || ocean_frac > 0.005);",
                "planet.wgsl must define has_volatile_ice before evaluating ice caps",
            ),
            (
                "let eyeball_ice = is_tidally_locked && has_volatile_ice && (temp < 345.0)",
                "planet.wgsl must require has_volatile_ice for eyeball glaciers",
            ),
            (
                "if (in_ice_cap) {",
                "planet.wgsl must guard ice cap rendering by in_ice_cap",
            ),
            (
                "let boil_steam_factor = clamp((temp - 345.0) / 35.0, 0.0, 1.0) * select(0.0, 1.0, has_water_volatiles",
                "planet.wgsl must strictly require has_water_volatiles before activating boiling-ocean steam clouds",
            ),
        ],
        &[],
    );
}

#[test]
fn test_planet_pipeline_specialization_permutations() {
    let classes = [
        "BODY_CLASS_STAR_REMNANT",
        "BODY_CLASS_GIANT",
        "BODY_CLASS_TERRESTRIAL",
        "BODY_CLASS_MINOR",
    ];

    for class_def in classes {
        // Forward pass
        compile_and_validate_wesl_shader(
            "assets/shaders/planet.wesl",
            &[ShaderDefVal::Bool(class_def.into(), true)],
        );

        // Forward pass with cutaway
        compile_and_validate_wesl_shader(
            "assets/shaders/planet.wesl",
            &[
                ShaderDefVal::Bool(class_def.into(), true),
                ShaderDefVal::Bool("CUTAWAY_ENABLED".into(), true),
            ],
        );

        // Prepass pipeline
        compile_and_validate_wesl_shader(
            "assets/shaders/planet.wesl",
            &[
                ShaderDefVal::Bool(class_def.into(), true),
                ShaderDefVal::Bool("PREPASS_PIPELINE".into(), true),
                ShaderDefVal::Bool("PREPASS_FRAGMENT".into(), true),
                ShaderDefVal::Bool("DEFERRED_PREPASS".into(), true),
                ShaderDefVal::Bool("NORMAL_PREPASS_OR_DEFERRED_PREPASS".into(), true),
            ],
        );
    }
}
