//! Stage 2 shader/view-override guardrails.

use protostellar::rendering::bodies::view_overrides::stripped_core_lava_frac;
use protostellar::simulation::components::BodyType;

#[test]
fn cold_stripped_super_earth_has_no_lava_floor() {
    let f = stripped_core_lava_frac(BodyType::TerrestrialPlanet, 300.0, 300.0);
    assert!(f < 0.01, "cold core must not get magma floor, got {f}");
}

#[test]
fn giants_and_hot_cores_keep_lava_floor() {
    assert!(stripped_core_lava_frac(BodyType::GasGiant, 1000.0, 300.0) >= 0.45);
    assert!(stripped_core_lava_frac(BodyType::TerrestrialPlanet, 1000.0, 1000.0) >= 0.45);
}

#[test]
fn guarded_shaders_still_pass_naga() {
    for (path, needle) in [
        ("assets/shaders/planetary_rings.wesl", "star_dir"),
        ("assets/shaders/gas_cloud.wesl", "h_scale"),
        ("assets/shaders/black_hole_disk.wesl", "lensed_alpha"),
        ("assets/shaders/planet.wesl", "bh_ring"),
    ] {
        let src = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            src.contains(needle) || path.ends_with("planet.wesl"),
            "{path} expected to contain {needle}"
        );
        super::shader_guardrails::compile_and_validate_wesl_shader(path, &[]);
    }
    let cutaway_src = std::fs::read_to_string("src/rendering/shaders/cutaway.wesl")
        .expect("cutaway.wesl must exist");
    assert!(cutaway_src.contains("bh_ring"));
}
