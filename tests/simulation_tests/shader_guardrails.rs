//! Guardrail integration tests for WGSL shader bind groups, Naga syntax validation,
//! and safe ECS entity despawn idempotence.

use bevy::prelude::*;
use protostellar::rendering::materials::{BlackHoleDiskMaterial, BlackHoleDiskUniforms};

#[test]
fn test_all_material_shaders_use_preprocessed_bind_group() {
    let shaders_dir = std::path::Path::new("assets/shaders");
    assert!(shaders_dir.exists(), "assets/shaders directory must exist");

    let entries = std::fs::read_dir(shaders_dir).expect("Failed to read shaders dir");
    let mut checked_shaders = 0;

    for entry in entries {
        let entry = entry.expect("Valid dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("wgsl") {
            let filename = path.file_name().unwrap().to_str().unwrap();

            // Compute shaders for particle orbit, particle render, and GMC fluid legitimately use @group(0)
            if filename == "particle_orbit.wgsl"
                || filename == "particle_render.wgsl"
                || filename == "gmc_fluid.wgsl"
            {
                continue;
            }

            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {:?}", filename, e));

            // Assert that no material shader hardcodes @group(1) or @group(2)
            assert!(
                !content.contains("@group(1)"),
                "Shader {} hardcodes @group(1), which collides with pipeline layouts! Use @group(#{{MATERIAL_BIND_GROUP}}).",
                filename
            );
            assert!(
                !content.contains("@group(2)"),
                "Shader {} hardcodes @group(2), which collides with Bevy mesh storage buffers! Use @group(#{{MATERIAL_BIND_GROUP}}).",
                filename
            );

            // If the shader binds a material uniform/extension, verify it uses #{MATERIAL_BIND_GROUP}
            if content.contains("@binding(0)") || content.contains("@binding(101)") {
                assert!(
                    content.contains("@group(#{MATERIAL_BIND_GROUP})"),
                    "Material shader {} must use @group(#{{MATERIAL_BIND_GROUP}}) for bind group compatibility.",
                    filename
                );
            }

            checked_shaders += 1;
        }
    }

    assert!(
        checked_shaders >= 7,
        "Expected at least 7 material shaders to be checked (checked {})",
        checked_shaders
    );
}

/// Validates that a WGSL material shader parses cleanly using naga after prepending mock pipeline headers.
pub fn validate_wgsl_shader_with_naga(shader_path: &str, struct_name: &str) {
    let shader_source =
        std::fs::read_to_string(shader_path).unwrap_or_else(|_| panic!("{shader_path} must exist"));

    let mock_header = r#"
struct View {
    world_position: vec3<f32>,
};
@group(0) @binding(0) var<uniform> view: View;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    world_position: vec4<f32>,
    world_normal: vec3<f32>,
    uv: vec2<f32>,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};
"#;

    let start_idx = shader_source
        .find(struct_name)
        .unwrap_or_else(|| panic!("{struct_name} must exist in shader"));

    let full_source = format!("{mock_header}\n{}", &shader_source[start_idx..])
        .replace("#{MATERIAL_BIND_GROUP}", "2u");

    let res = wgpu::naga::front::wgsl::parse_str(&full_source);
    if let Err(ref err) = res {
        eprintln!("{shader_path} WGSL Parse Error: {:?}", err);
    }
    assert!(
        res.is_ok(),
        "{shader_path} must parse cleanly with naga without syntax errors: {:?}",
        res.err()
    );
}

#[test]
fn test_black_hole_disk_wgsl_naga_validation() {
    validate_wgsl_shader_with_naga(
        "assets/shaders/black_hole_disk.wgsl",
        "struct BlackHoleDiskUniforms",
    );
}

#[test]
fn test_gmc_fluid_wgsl_naga_validation() {
    let source = std::fs::read_to_string("assets/shaders/gmc_fluid.wgsl")
        .expect("assets/shaders/gmc_fluid.wgsl must exist");
    let res = wgpu::naga::front::wgsl::parse_str(&source);
    if let Err(ref err) = res {
        eprintln!("assets/shaders/gmc_fluid.wgsl WGSL Parse Error: {:?}", err);
    }
    assert!(
        res.is_ok(),
        "assets/shaders/gmc_fluid.wgsl must parse cleanly with naga without syntax errors: {:?}",
        res.err()
    );
}

#[test]
fn test_volumetric_nebula_wgsl_naga_validation() {
    validate_wgsl_shader_with_naga(
        "assets/shaders/volumetric_nebula.wgsl",
        "struct VolumetricNebulaUniforms",
    );
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
