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

/// Returns `true` when a body renders with an opaque giant-scale gas/ice envelope
/// (a Gas Giant, Ice Giant, or gas-dominated dwarf with `gas_frac > 0.25`)
/// rather than a directly visible solid/oceanic surface under a thin atmosphere.
#[must_use]
pub fn has_strippable_gas_envelope(body_type: BodyType, comp: &Composition) -> bool {
    if body_type.is_star_or_remnant()
        || body_type == BodyType::BlackHole
        || body_type == BodyType::QuasiStar
    {
        return false;
    }
    let norm = comp.normalized();
    matches!(body_type, BodyType::GasGiant | BodyType::IceGiant)
        || norm.gas_frac > 0.25
        || (norm.gas_frac > 0.20 && norm.ice_frac > 0.20)
}

/// Computes the physical radius (in AU) of the solid/icy interior core remaining
/// when a giant planet's outer opaque H/He envelope (`gas_frac > 0.01`) is visually stripped.
///
/// Terrestrial planets, Super-Earths, protoplanets, and moons already display their solid
/// surface beneath a thin translucent atmosphere when `Atmo: ON`, so stripping their atmosphere
/// keeps `radius_au` unchanged rather than shrinking the rocky globe like a gas giant.
#[must_use]
pub fn stripped_interior_radius_au(
    mass_solar: f64,
    radius_au: f64,
    body_type: BodyType,
    comp: &Composition,
    opt_diff: Option<&InternalDifferentiation>,
) -> f64 {
    if !has_strippable_gas_envelope(body_type, comp) || !radius_au.is_finite() || radius_au <= 0.0 {
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
    let is_giant_like = has_strippable_gas_envelope(body_type, comp);
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

/// Lava fraction for a stripped non-icy core. The 0.45 floor applies only to
/// gas/ice giants or hot interiors (> 900 K); cold stripped super-Earths get no forced magma.
#[must_use]
pub fn stripped_core_lava_frac(body_type: BodyType, core_temp_k: f64, temp_k: f64) -> f32 {
    let raw = (core_temp_k as f32 - 450.0) / 1100.0;
    let is_giant = matches!(body_type, BodyType::GasGiant | BodyType::IceGiant);
    if is_giant || temp_k > 900.0 || core_temp_k > 900.0 {
        raw.clamp(0.45, 0.92)
    } else {
        raw.clamp(0.0, 0.92)
    }
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
    let is_giant_like = has_strippable_gas_envelope(body_type, comp);

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
                stripped_core_lava_frac(body_type, core_temp, temp_k)
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
