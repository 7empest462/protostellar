//! Visual view overrides for toggling planetary atmospheres, gas envelopes, and cloud decks.

use bevy::prelude::*;

use crate::rendering::materials::PlanetMaterial;
use crate::simulation::components::{
    BodyType, CelestialBody, Composition, InternalDifferentiation, Mass, Temperature,
};
use crate::simulation::tides::TidalState;

use super::materials_sync::BodyMaterialParameters;
use super::palettes::{compute_body_color, compute_terrestrial_body_palette};

/// Strips gaseous envelope volatiles from a body's composition and re-normalizes
/// the underlying refractory/icy interior fractions.
#[must_use]
pub fn stripped_interior_composition(comp: &Composition) -> Composition {
    let norm = comp.normalized();
    let solid = norm.silicate_frac + norm.organics_frac + norm.ice_frac + norm.metal_frac;
    if solid <= 1e-6 {
        return Composition {
            silicate_frac: 0.65,
            metal_frac: 0.35,
            ice_frac: 0.0,
            organics_frac: 0.0,
            gas_frac: 0.0,
        };
    }
    let inv = 1.0 / solid;
    Composition {
        silicate_frac: norm.silicate_frac * inv,
        organics_frac: norm.organics_frac * inv,
        ice_frac: norm.ice_frac * inv,
        metal_frac: norm.metal_frac * inv,
        gas_frac: 0.0,
    }
}

/// Computes the physical radius (in AU) of the solid/icy interior core remaining
/// when a planet's outer gaseous H/He envelope (`gas_frac > 0.01`) is visually stripped.
///
/// Uses the same harmonic non-gas density mix as atmospheric photoevaporation so the
/// rendered sphere diameter accurately reflects the compact rocky/icy core inside.
#[must_use]
pub fn stripped_interior_radius_au(
    mass_solar: f64,
    radius_au: f64,
    body_type: BodyType,
    comp: &Composition,
    opt_diff: Option<&InternalDifferentiation>,
) -> f64 {
    if body_type.is_star_or_remnant()
        || body_type == BodyType::BlackHole
        || !radius_au.is_finite()
        || radius_au <= 0.0
    {
        return radius_au;
    }
    let norm = comp.normalized();
    if norm.gas_frac <= 0.01 {
        return radius_au;
    }

    let solid_frac =
        (norm.silicate_frac + norm.organics_frac + norm.ice_frac + norm.metal_frac).clamp(0.0, 1.0);
    if solid_frac <= 1e-4 {
        return (radius_au * 0.08)
            .clamp(crate::utils::constants::EARTH_RADIUS_AU * 0.25, radius_au);
    }

    let stripped = stripped_interior_composition(comp);
    let core_mass_solar = (mass_solar * solid_frac).max(0.0);
    let core_density = stripped.average_density().max(1e-12);
    let r_volume_core =
        ((3.0 * core_mass_solar / core_density) / (4.0 * std::f64::consts::PI)).cbrt();
    let diff_floor = opt_diff.map_or(0.0, |d| {
        if d.is_differentiated {
            d.mantle_radius_au + d.crust_thickness_au
        } else {
            0.0
        }
    });
    r_volume_core
        .max(diff_floor)
        .clamp(crate::utils::constants::EARTH_RADIUS_AU * 0.15, radius_au)
}

/// Resolves the effective physical radius (in AU) and body classification used for
/// visual scaling, cutaway geometry, and camera framing under active atmosphere toggles.
#[must_use]
pub fn resolve_view_body_radius_and_type(
    mass_solar: f64,
    radius_au: f64,
    body_type: BodyType,
    comp: &Composition,
    opt_diff: Option<&InternalDifferentiation>,
    show_atmospheres: bool,
) -> (f64, BodyType) {
    if show_atmospheres || body_type.is_star_or_remnant() {
        return (radius_au, body_type);
    }
    let stripped_r = stripped_interior_radius_au(mass_solar, radius_au, body_type, comp, opt_diff);
    let norm = comp.normalized();
    let is_giant_like = matches!(body_type, BodyType::GasGiant | BodyType::IceGiant)
        || (norm.gas_frac > 0.25 && !body_type.is_star_or_remnant());
    let eff_type = if is_giant_like {
        BodyType::TerrestrialPlanet
    } else {
        body_type
    };
    (stripped_r, eff_type)
}

/// Computes the visual temperature used for rendering a stripped interior when
/// a gas or ice giant's outer envelope is hidden.
#[must_use]
pub fn stripped_core_display_temp_k(
    body_type: BodyType,
    stripped_comp: &Composition,
    temp_k: f64,
    opt_diff: Option<&InternalDifferentiation>,
) -> f64 {
    let is_giant = matches!(body_type, BodyType::GasGiant | BodyType::IceGiant);
    if !is_giant {
        return temp_k;
    }
    if stripped_comp.ice_frac >= 0.35 {
        return temp_k.clamp(145.0, 240.0);
    }
    opt_diff.map_or((temp_k * 1.3).clamp(950.0, 1800.0), |d| {
        d.core_temp_k.clamp(1150.0, 2200.0)
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "Applies atmosphere and cloud visibility toggles across uniforms and interior palettes"
)]
pub(super) fn apply_atmosphere_and_cloud_view_overrides(
    mat: &mut PlanetMaterial,
    body: &CelestialBody,
    mass: &Mass,
    comp: &Composition,
    temp_k: f64,
    opt_diff: Option<&InternalDifferentiation>,
    opt_tidal: Option<&TidalState>,
    params: &mut BodyMaterialParameters,
    show_atmospheres: bool,
    show_clouds: bool,
) -> (BodyType, Composition, f64, Color, bool) {
    let body_type = body.body_type;
    let norm = comp.normalized();
    let is_giant_like = matches!(body_type, BodyType::GasGiant | BodyType::IceGiant)
        || (norm.gas_frac > 0.25 && !body_type.is_star_or_remnant());

    if !show_atmospheres && !body_type.is_star_or_remnant() {
        let stripped = stripped_interior_composition(comp);
        params.pressure_bar = 0.0;
        params.cloud_density = 0.0;
        // scale_height = -1.0 signals atmo_visually_hidden to WGSL so terrestrial liquid oceans remain visible.
        params.atmosphere_params = Vec4::new(0.0, -1.0, 0.0, 0.1);
        params.dynamics_and_mag.z = 0.0;
        params.climate_and_bio.w = 0.0;
        mat.extension.uniforms.aurora_params = Vec4::ZERO;
        mat.extension.uniforms.storm_features = Vec4::ZERO;

        if is_giant_like {
            let core_temp = stripped_core_display_temp_k(body_type, &stripped, temp_k, opt_diff);
            let core_ice_frac = if stripped.ice_frac >= 0.35 {
                (stripped.ice_frac as f32).clamp(0.45, 0.95)
            } else {
                0.0
            };
            let core_lava_frac = if stripped.ice_frac >= 0.35 {
                if temp_k > 450.0 { 0.25 } else { 0.0 }
            } else {
                ((core_temp as f32 - 450.0) / 1100.0).clamp(0.45, 0.92)
            };
            params.climate_and_bio = Vec4::new(0.0, core_ice_frac, 0.0, 0.0);
            params.dynamics_and_mag.y = core_lava_frac;
            let core_color =
                compute_terrestrial_body_palette(&body.name, &stripped, core_temp, opt_tidal);
            return (
                BodyType::TerrestrialPlanet,
                stripped,
                core_temp,
                core_color,
                true,
            );
        }

        let surface_color =
            compute_body_color(body, mass, &Temperature(temp_k), &stripped, opt_tidal);
        return (body_type, stripped, temp_k, surface_color, false);
    }

    if !show_clouds && !body_type.is_star_or_remnant() {
        params.cloud_density = 0.0;
        // Sentinel < -0.5 instructs planet.wesl to suppress cloud decks while keeping atmospheres active.
        params.climate_and_bio.w = -1.0;
        params.atmosphere_params.z *= 0.22;
    }

    (body_type, *comp, temp_k, Color::WHITE, false)
}

#[cfg(test)]
mod tests {
    use super::super::atmospheres::build_planet_atmosphere_uniforms;
    use super::super::materials_sync::update_body_material_properties;
    use super::*;
    use crate::simulation::components::{PlanetaryClimate, SimPosition, VolatileInventory};
    use crate::utils::constants::{EARTH_MASS_SOLAR, JUPITER_MASS_SOLAR};

    fn make_mat() -> PlanetMaterial {
        PlanetMaterial {
            base: StandardMaterial::default(),
            extension: crate::rendering::materials::PlanetMaterialExtension {
                uniforms: crate::rendering::materials::PlanetUniforms::default(),
            },
        }
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
        use crate::utils::constants::{AU_TO_KM, EARTH_RADIUS_AU};

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

        let config = crate::simulation::resources::SimulationConfig::default();
        let vis_gas_on = crate::rendering::bodies::transforms::compute_view_visual_radius(
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
        let vis_gas_off = crate::rendering::bodies::transforms::compute_view_visual_radius(
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
            &SimPosition(bevy::math::DVec3::X),
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
}
