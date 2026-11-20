use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::geology::GeologicalState;
use crate::simulation::space_weather::{AuroralOvalState, StellarFlareState};
use crate::simulation::thermodynamics::retained_gas_envelope_bar;
use crate::simulation::tides::TidalState;

use super::VisualAssets;
use super::palettes::*;
use super::view_overrides::apply_atmosphere_and_cloud_view_overrides;

fn resolve_star_point_light(
    body_type: BodyType,
    is_blown_out: bool,
    elapsed_secs: f32,
) -> (Color, f32) {
    match body_type {
        BodyType::BlackHole | BodyType::BlackDwarf | BodyType::IronStar => (Color::BLACK, 0.0),
        BodyType::QuasiStar => (
            Color::srgb(1.0, 0.10, 0.02),
            if is_blown_out {
                2_000_000.0
            } else {
                60_000_000.0
            },
        ),
        BodyType::Pulsar => {
            let strobe = (elapsed_secs * 24.0).sin().abs().powi(4);
            (
                Color::srgb(0.70, 0.90, 1.0),
                8_000_000.0 + strobe * 80_000_000.0,
            )
        }
        BodyType::Magnetar => {
            let flare = ((elapsed_secs * 5.0).sin() * (elapsed_secs * 13.0).cos())
                .abs()
                .powf(1.8);
            (
                Color::srgb(0.85, 0.60, 1.0),
                12_000_000.0 + flare * 120_000_000.0,
            )
        }
        BodyType::QuarkStar => {
            let arc = (elapsed_secs * 16.0).sin().abs().powi(3);
            (
                Color::srgb(0.85, 0.72, 1.0),
                28_000_000.0 + arc * 35_000_000.0,
            )
        }
        BodyType::ThorneZytkow => {
            let pulse = (elapsed_secs * 6.5).sin().abs().powi(3);
            (
                Color::srgb(0.95, 0.30, 0.38),
                22_000_000.0 + pulse * 18_000_000.0,
            )
        }
        BodyType::BosonStar => (Color::srgb(0.45, 0.90, 1.0), 4_500_000.0),
        BodyType::DarkStar => (Color::srgb(1.0, 0.68, 0.25), 32_000_000.0),
        BodyType::PopulationIIIStar | BodyType::LuminousBlueVariable => {
            (Color::srgb(0.68, 0.84, 1.0), 55_000_000.0)
        }
        BodyType::WolfRayet | BodyType::BlueSupergiant | BodyType::Hypergiant => {
            (Color::srgb(0.75, 0.85, 1.0), 40_000_000.0)
        }
        BodyType::YellowHypergiant => (Color::srgb(1.0, 0.85, 0.45), 36_000_000.0),
        BodyType::NeutronStar => (Color::srgb(0.65, 0.82, 1.0), 30_000_000.0),
        BodyType::BlueGiant
        | BodyType::BlueMainSequence
        | BodyType::BlueStraggler
        | BodyType::HerbigAeBeStar => (Color::srgb(0.78, 0.88, 1.0), 28_000_000.0),
        BodyType::RedGiant | BodyType::RedSupergiant | BodyType::CarbonStar => {
            (Color::srgb(1.0, 0.38, 0.14), 26_000_000.0)
        }
        BodyType::WhiteDwarf => (Color::srgb(0.85, 0.92, 1.0), 25_000_000.0),
        BodyType::WhiteMainSequence => (Color::srgb(0.94, 0.97, 1.0), 24_000_000.0),
        BodyType::YellowWhiteDwarf => (Color::srgb(1.0, 0.97, 0.88), 21_000_000.0),
        BodyType::OrangeDwarf | BodyType::TTauriStar | BodyType::Protostar => {
            (Color::srgb(1.0, 0.72, 0.38), 14_000_000.0)
        }
        BodyType::RedDwarf => (Color::srgb(1.0, 0.48, 0.20), 9_000_000.0),
        BodyType::BrownDwarf => (Color::srgb(0.75, 0.22, 0.42), 2_500_000.0),
        _ => (Color::srgb(1.0, 0.95, 0.86), 18_000_000.0),
    }
}

pub(crate) fn update_star_lights_and_strobes(
    opt_children: Option<&Children>,
    light_query: &mut Query<&mut PointLight>,
    body_type: BodyType,
    is_blown_out: bool,
    elapsed_secs: f32,
) {
    let Some(children) = opt_children else {
        return;
    };
    let (color, intensity) = resolve_star_point_light(body_type, is_blown_out, elapsed_secs);
    for child in children.iter() {
        if let Ok(mut light) = light_query.get_mut(child) {
            if intensity > 0.0 {
                light.color = color;
            }
            light.intensity = intensity;
        }
    }
}

fn update_star_material(
    mat: &mut PlanetMaterial,
    body_type: BodyType,
    is_blown_out: bool,
    color: Color,
    params_mag_gauss: f32,
    mass: f64,
    axial_tilt: f32,
    opt_bhs: Option<&BlackHoleStarState>,
    opt_flare: Option<&StellarFlareState>,
) {
    if body_type == BodyType::BlackHole || (body_type == BodyType::QuasiStar && is_blown_out) {
        mat.extension.uniforms.planet_type = 5;
        mat.base.unlit = true;
        mat.base.alpha_mode = AlphaMode::Blend;
        mat.base.base_color = Color::srgba(0.0, 0.0, 0.0, 0.0);
        mat.base.emissive = LinearRgba::BLACK;
    } else if body_type == BodyType::QuasiStar {
        mat.extension.uniforms.planet_type = 7;
        mat.base.unlit = true;
        mat.base.emissive = LinearRgba::from(Color::srgb(1.0, 0.10, 0.02)) * 32.0;
        mat.extension.uniforms.color_seed = Vec4::new(1.0, 0.10, 0.02, 1.0);
        let edd_ratio = opt_bhs.map_or(3.5, |s| s.eddington_ratio as f32);
        mat.extension.uniforms.dynamics_and_mag = Vec4::new(
            params_mag_gauss.max(1.0e6),
            edd_ratio,
            mass as f32,
            axial_tilt,
        );
        mat.extension.uniforms.atmosphere_params = Vec4::new(0.65, 1.25, 0.0, 1.0);
        mat.extension.uniforms.composition = Vec4::new(0.0, 4.5, 1.5, 0.8);
    } else {
        mat.extension.uniforms.planet_type = 0;
        mat.base.unlit = true;
        mat.base.alpha_mode = if matches!(body_type, BodyType::BosonStar | BodyType::DarkStar) {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        };
        let star_subtype = star_subtype_from_body_type(body_type);
        // HDR emissive radiance. Values are scene-linear; Bloom threshold is 1.8.
        // Multipliers must push above that to generate a glow halo.
        // These are calibrated so stars look incandescent, not glowing-plastic.
        let mult = match body_type {
            BodyType::WhiteDwarf => 220.0, // Intense blue-white ultraviolet furnace
            BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar | BodyType::QuarkStar => {
                280.0
            }
            BodyType::BlackDwarf | BodyType::IronStar => 0.0, // Cold, inert degenerate relics
            BodyType::BosonStar => 45.0, // Faint refractive gravitational soliton
            BodyType::DarkStar | BodyType::YellowWhiteDwarf => 160.0, // Puffy dark star & F-type dwarf
            BodyType::PopulationIIIStar | BodyType::LuminousBlueVariable => 420.0, // Ultra-luminous ionizing furnace
            BodyType::Protostar | BodyType::TTauriStar | BodyType::HerbigAeBeStar => 65.0, // Young, luminous
            BodyType::RedDwarf | BodyType::OrangeDwarf => 85.0,
            BodyType::BrownDwarf => 28.0, // Barely above threshold — faint deep-red glow
            BodyType::RedGiant
            | BodyType::RedSupergiant
            | BodyType::CarbonStar
            | BodyType::ThorneZytkow => 105.0,
            BodyType::BlueMainSequence
            | BodyType::BlueGiant
            | BodyType::BlueSupergiant
            | BodyType::Hypergiant
            | BodyType::YellowHypergiant
            | BodyType::BlueStraggler => 350.0,
            BodyType::WhiteMainSequence => 210.0,
            BodyType::WolfRayet => 320.0,
            _ => 120.0, // Main-sequence yellow/orange dwarfs (Sun-like)
        };
        let flare_mult = opt_flare.map_or(1.0, |f| 1.0 + f.current_flare_intensity * 2.0);
        let emissive_boost = opt_flare.map_or(1.0, |f| 1.0 + f.current_flare_intensity * 1.8);
        mat.base.emissive = LinearRgba::from(color) * (mult * emissive_boost);

        let (cell_scale, mut flare_intensity, pulse_freq) = match body_type {
            BodyType::RedGiant
            | BodyType::RedSupergiant
            | BodyType::CarbonStar
            | BodyType::ThorneZytkow => (4.0, 0.35, 0.2),
            BodyType::RedDwarf | BodyType::OrangeDwarf => (14.0, 1.4, 0.6),
            BodyType::BrownDwarf => (8.0, 0.2, 0.3),
            BodyType::BlueGiant
            | BodyType::BlueSupergiant
            | BodyType::Hypergiant
            | BodyType::YellowHypergiant
            | BodyType::PopulationIIIStar
            | BodyType::LuminousBlueVariable => (22.0, 0.8, 1.5),
            BodyType::NeutronStar => (40.0, 1.5, 2.0),
            BodyType::Pulsar | BodyType::QuarkStar => (35.0, 2.2, 6.5),
            BodyType::Magnetar => (16.0, 3.8, 1.0),
            BodyType::WhiteDwarf => (30.0, 0.2, 0.5),
            BodyType::Protostar | BodyType::TTauriStar => (10.0, 1.2, 0.5),
            BodyType::HerbigAeBeStar => (14.0, 0.8, 0.8),
            BodyType::WolfRayet => (18.0, 2.0, 2.5),
            BodyType::DarkStar => (6.0, 0.4, 0.15),
            BodyType::BosonStar => (2.0, 0.1, 0.05),
            _ => (24.0, 0.5, 0.3),
        };
        flare_intensity *= flare_mult;
        mat.extension.uniforms.composition =
            Vec4::new(star_subtype, cell_scale, flare_intensity, pulse_freq);

        let spot_coverage = match body_type {
            BodyType::RedDwarf | BodyType::TTauriStar => 0.28,
            BodyType::OrangeDwarf => 0.14,
            BodyType::YellowDwarf | BodyType::MainSequenceStar => 0.08,
            BodyType::Protostar => 0.18,
            _ => 0.0,
        };
        mat.extension.uniforms.atmosphere_params = Vec4::new(0.60, 0.85, spot_coverage, 0.0);
        mat.extension.uniforms.dynamics_and_mag =
            Vec4::new(params_mag_gauss, 1.0, mass as f32, axial_tilt);
    }
}

fn update_planet_material(
    mat: &mut PlanetMaterial,
    body_type: BodyType,
    name: &str,
    comp: &Composition,
    temp_k: f64,
    color: Color,
) {
    let norm_comp = comp.normalized();
    mat.base.unlit = false;
    mat.extension.uniforms.composition = Vec4::new(
        norm_comp.silicate_frac as f32 + norm_comp.organics_frac as f32,
        norm_comp.ice_frac as f32,
        norm_comp.metal_frac as f32,
        norm_comp.gas_frac as f32,
    );

    let (planet_type, body_subtype) = resolve_planet_type_and_subtype(body_type, name, comp);
    mat.extension.uniforms.planet_type = planet_type;
    mat.extension.uniforms.color_seed.w = body_subtype;

    if planet_type == 9 {
        // Solid cryogenic ice crust: realistic dielectric reflectivity and crystalline powder roughness
        mat.base.metallic = 0.04;
        mat.base.perceptual_roughness = 0.55;
    } else if comp.metal_frac > 0.4 {
        mat.base.metallic = 0.85;
        mat.base.perceptual_roughness = 0.25;
    } else if comp.ice_frac > 0.4 {
        mat.base.metallic = 0.05;
        mat.base.perceptual_roughness = 0.18;
    } else if comp.gas_frac > 0.5 {
        mat.base.metallic = 0.0;
        let is_saturn_cloud =
            crate::game::ui::world_list::contains_ignore_ascii_case(name, "saturn");
        mat.base.perceptual_roughness = if is_saturn_cloud { 0.92 } else { 0.85 };
    } else {
        mat.base.metallic = 0.15;
        mat.base.perceptual_roughness = 0.75;
    }

    if temp_k > 600.0 {
        mat.base.emissive =
            LinearRgba::from(color) * ((temp_k as f32 - 600.0) / 600.0).clamp(0.0, 5.0);
    } else {
        mat.base.emissive = LinearRgba::BLACK;
    }
}

pub(crate) fn select_body_mesh(
    body: &CelestialBody,
    radius_au: f64,
    visual_assets: &VisualAssets,
) -> Handle<Mesh> {
    super::meshes::select_body_mesh(body, radius_au, visual_assets)
}

fn apply_impact_basins(
    mat: &mut PlanetMaterial,
    opt_basins: Option<&PlanetaryBasins>,
    elapsed_years: f64,
) {
    let mut basins_pos = [Vec4::ZERO; 4];
    let mut basins_data = [Vec4::ZERO; 4];

    if let Some(pb) = opt_basins {
        for (i, basin) in pb.basins.iter().rev().take(4).enumerate() {
            if let Some(pos) = basins_pos.get_mut(i) {
                *pos = Vec4::new(
                    basin.surface_normal.x,
                    basin.surface_normal.y,
                    basin.surface_normal.z,
                    basin.angular_radius,
                );
            }
            if let Some(data) = basins_data.get_mut(i) {
                let age_yr = (elapsed_years - basin.formation_time_yr).max(0.0) as f32;
                let seismic_phase =
                    ((1.0 - basin.melt_glow_fraction) * 2.4 + age_yr * 0.35).clamp(0.0, 1.0);
                *data = Vec4::new(
                    basin.melt_glow_fraction,
                    basin.elongation,
                    seismic_phase,
                    basin.scar_intensity,
                );
            }
        }
    }
    mat.extension.uniforms.impact_basins_pos = basins_pos;
    mat.extension.uniforms.impact_basins_data = basins_data;
}

pub struct BodyMaterialParameters {
    pub climate_and_bio: Vec4,
    pub atmosphere_params: Vec4,
    pub dynamics_and_mag: Vec4,
    pub pressure_bar: f32,
    pub cloud_density: f32,
    pub mag_gauss: f32,
}

struct AtmosphereCloudParams {
    pressure_bar: f32,
    scale_height: f32,
    cloud_density: f32,
    haze_density: f32,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Atmosphere/cloud parameter builder"
)]
fn compute_atmosphere_and_cloud_params(
    is_minor_body: bool,
    has_water_volatiles: bool,
    ocean_frac: f32,
    norm_comp: &Composition,
    mass_solar: f64,
    temp_k: f64,
    opt_climate: Option<&PlanetaryClimate>,
    opt_vol: Option<&VolatileInventory>,
) -> AtmosphereCloudParams {
    let gas_envelope_bar = if is_minor_body {
        0.0
    } else {
        retained_gas_envelope_bar(mass_solar, norm_comp.gas_frac as f32)
    };
    let pressure_bar = if is_minor_body {
        0.0
    } else {
        opt_vol
            .map_or((norm_comp.gas_frac as f32 * 2.0).max(0.0), |v| {
                v.atmospheric_pressure_bar
            })
            .max(gas_envelope_bar)
    };
    let is_airless = (opt_climate
        .is_some_and(|c| c.climate_regime == ClimateRegime::AirlessVacuum)
        && gas_envelope_bar <= 0.0)
        || (pressure_bar < 0.02 && norm_comp.gas_frac <= 0.005);
    let envelope_cloud_floor = if pressure_bar > 15.0 {
        ((pressure_bar - 15.0) / 60.0).clamp(0.45, 0.95)
    } else {
        0.0
    };
    let cloud_density = if !is_minor_body
        && !is_airless
        && (has_water_volatiles || norm_comp.gas_frac > 0.005 || pressure_bar >= 0.05)
    {
        let fallback_clouds = if has_water_volatiles {
            norm_comp.gas_frac as f32
        } else {
            (0.24 + norm_comp.gas_frac as f32 * 6.5).clamp(0.22, 0.68)
        };
        let base_clouds = opt_climate
            .map_or(fallback_clouds, |c| c.cloud_coverage_frac)
            .max(envelope_cloud_floor);
        if temp_k > 380.0 && (ocean_frac > 0.01 || has_water_volatiles) {
            let steam_factor = ((temp_k as f32 - 380.0) / 55.0).clamp(0.0, 1.0);
            (base_clouds + steam_factor * 0.70).clamp(0.0, 0.98)
        } else {
            base_clouds
        }
    } else {
        0.0
    };
    let scale_height = (0.08f32 * (temp_k as f32 / 288.0f32).sqrt()).clamp(0.02f32, 0.25f32);
    let base_haze = if is_minor_body || is_airless {
        0.0
    } else {
        opt_climate
            .map_or((norm_comp.gas_frac as f32 * 1.5).clamp(0.0, 1.0), |c| {
                (c.cloud_coverage_frac * 1.2).clamp(0.0, 1.0)
            })
            .max((envelope_cloud_floor * 1.1).clamp(0.0, 1.0))
    };
    let haze_density = if !is_minor_body && temp_k > 380.0 && ocean_frac > 0.02 {
        let steam_haze = ((temp_k as f32 - 380.0) / 55.0).clamp(0.0, 1.0) * 0.6;
        (base_haze + steam_haze).clamp(0.0, 1.0)
    } else {
        base_haze
    };
    AtmosphereCloudParams {
        pressure_bar,
        scale_height,
        cloud_density,
        haze_density,
    }
}

#[allow(clippy::too_many_arguments, reason = "Material parameter builder")]
pub fn compute_body_material_parameters(
    body_type: BodyType,
    comp: &Composition,
    temp_k: f64,
    mass_solar: f64,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_vol: Option<&VolatileInventory>,
    opt_em: Option<&ElectromagneticFieldState>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_tidal: Option<&TidalState>,
    axial_tilt: f32,
) -> BodyMaterialParameters {
    let norm_comp = comp.normalized();
    let is_minor_body = matches!(
        body_type,
        BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain | BodyType::Planetesimal
    );
    let is_snowball_regime =
        opt_climate.is_some_and(|c| c.climate_regime == ClimateRegime::SnowballIceAge);
    let has_water_volatiles = norm_comp.ice_frac > 0.001
        || opt_vol.is_some_and(|v| v.delivered_water_m_earth > 1e-6)
        || is_snowball_regime;
    let ocean_frac = if is_minor_body || !has_water_volatiles {
        0.0
    } else {
        // Bulk composition `ice_frac` may reflect solid ice or hydrated minerals;
        // surface liquid oceans exist only when tracked by VolatileInventory.
        opt_vol.map_or(0.0, |v| v.ocean_coverage_frac)
    };
    let biomass_frac = if !is_minor_body && has_water_volatiles && ocean_frac > 0.01 {
        opt_bio.map_or(0.0, |b| b.biomass_coverage_frac)
    } else {
        0.0
    };
    let AtmosphereCloudParams {
        pressure_bar,
        scale_height,
        cloud_density,
        haze_density,
    } = compute_atmosphere_and_cloud_params(
        is_minor_body,
        has_water_volatiles,
        ocean_frac,
        &norm_comp,
        mass_solar,
        temp_k,
        opt_climate,
        opt_vol,
    );
    let ice_frac = if is_minor_body {
        norm_comp.ice_frac as f32
    } else if is_snowball_regime {
        opt_climate.map_or(1.0, |c| c.ice_coverage_frac.max(0.95))
    } else if let Some(climate) = opt_climate {
        if has_water_volatiles {
            climate.ice_coverage_frac
        } else {
            0.0
        }
    } else if has_water_volatiles && temp_k < 175.0 && norm_comp.ice_frac > 0.10 {
        // Cold vacuum icy worlds stably retain exposed surface water ice without gas.
        (norm_comp.ice_frac as f32 * 1.4).clamp(0.15, 1.0)
    } else {
        0.0
    };
    let default_greenhouse = if pressure_bar >= 0.01 { 33.0 } else { 0.0 };
    let greenhouse = opt_climate.map_or(default_greenhouse, |c| c.greenhouse_delta_k);

    let b_em = opt_em.map_or(0.0, |e| e.magnetic_field_gauss as f32);
    let b_diff = opt_diff.map_or(0.0, |d| d.magnetic_field_gauss as f32);
    let mag_gauss = b_em.max(b_diff);
    // Open surface liquid water cannot exist below H2O's 0.0061 bar triple point.
    let liquid_ocean_frac = if is_snowball_regime || pressure_bar < 0.0061 || temp_k > 435.0 {
        0.0
    } else if temp_k > 385.0 {
        let boil_factor = (435.0 - temp_k) / 50.0;
        (ocean_frac * boil_factor as f32).clamp(0.0, 1.0)
    } else {
        ocean_frac
    };

    let tidal_lava_boost = opt_tidal.map_or(0.0, |t| {
        if t.tidal_heating_flux_w_m2 > 0.5 {
            if liquid_ocean_frac > 0.05 && temp_k < 380.0 {
                0.0
            } else {
                ((t.tidal_heating_flux_w_m2 as f32 - 0.5) / 2.5).clamp(0.0, 0.75)
            }
        } else {
            0.0
        }
    });
    let base_lava_frac = if temp_k > 450.0 {
        ((temp_k as f32 - 450.0) / 1350.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let lava_frac = (base_lava_frac + tidal_lava_boost).clamp(0.0, 1.0);

    let climate_and_bio = Vec4::new(liquid_ocean_frac, ice_frac, biomass_frac, cloud_density);
    let atmosphere_params = Vec4::new(
        pressure_bar,
        scale_height,
        haze_density,
        (greenhouse / 33.0).clamp(0.1, 8.0),
    );

    let default_storm = if pressure_bar < 0.02 && norm_comp.gas_frac <= 0.005 {
        0.0
    } else {
        0.2
    };
    let storm_intensity = if matches!(body_type, BodyType::GasGiant) || norm_comp.gas_frac > 0.30 {
        (mass_solar / crate::utils::constants::JUPITER_MASS_SOLAR) as f32
    } else {
        opt_climate.map_or(default_storm, |c| {
            (c.cloud_coverage_frac * 1.5).clamp(0.0, 1.5)
        })
    };
    let dynamics_and_mag = Vec4::new(mag_gauss, lava_frac, storm_intensity, axial_tilt);

    BodyMaterialParameters {
        climate_and_bio,
        atmosphere_params,
        dynamics_and_mag,
        pressure_bar,
        cloud_density,
        mag_gauss,
    }
}

fn compute_spin_axis(opt_spin: Option<&SpinState>, opt_geo: Option<&GeologicalState>) -> Vec4 {
    let drift_phase = opt_geo.map_or(0.0, |g| g.continental_drift_phase);
    if let Some(spin) = opt_spin {
        if spin.spin_vector.length_squared() > 1e-12 {
            let n = spin.spin_vector.normalize();
            Vec4::new(n.x as f32, n.y as f32, n.z as f32, drift_phase)
        } else {
            let tilt = (spin.axial_tilt_degrees as f32).to_radians();
            Vec4::new(tilt.sin(), tilt.cos(), 0.0, drift_phase)
        }
    } else {
        Vec4::new(0.0, 1.0, 0.0, drift_phase)
    }
}

fn compute_aurora_uniforms(opt_aurora: Option<&AuroralOvalState>, mag_gauss: f32) -> Vec4 {
    crate::simulation::space_weather::calculate_aurora_shader_uniforms(opt_aurora, mag_gauss)
}

fn compute_storm_uniforms(
    opt_storm: Option<&AtmosphericStormState>,
    body_name: &str,
) -> (Vec4, Vec4) {
    use crate::game::ui::world_list::contains_ignore_ascii_case;
    let storm = opt_storm.copied().or_else(|| {
        if contains_ignore_ascii_case(body_name, "saturn") {
            Some(AtmosphericStormState::saturn())
        } else if contains_ignore_ascii_case(body_name, "jupiter") {
            Some(AtmosphericStormState::jupiter())
        } else if contains_ignore_ascii_case(body_name, "neptune") {
            Some(AtmosphericStormState::neptune())
        } else {
            None
        }
    });

    storm.map_or_else(
        || (Vec4::ZERO, Vec4::new(0.0, 1.0, 0.0, 0.75)),
        |s| {
            (
                Vec4::new(
                    s.polar_hexagon_amplitude,
                    s.polar_hexagon_wavenumber,
                    s.great_spot_size,
                    s.great_spot_latitude_rad,
                ),
                Vec4::new(
                    s.great_spot_longitude_rad,
                    s.vortex_spin_rate,
                    s.secondary_oval_count as f32,
                    s.zonal_shear_turbulence,
                ),
            )
        },
    )
}

fn compute_civilization_uniforms(
    mass_solar: f64,
    opt_bio: Option<&BiosphereState>,
    opt_tidal: Option<&TidalState>,
) -> Vec4 {
    let earth_masses = (mass_solar / crate::utils::constants::EARTH_MASS_SOLAR).max(0.01);
    let surface_gravity_g = (earth_masses.powf(0.46) as f32).clamp(0.15, 5.0);
    if let Some(bio) = opt_bio {
        let tidal_biolum =
            if opt_tidal.is_some_and(|t| t.is_tidally_locked) && bio.biomass_coverage_frac > 0.05 {
                0.55 * bio.biomass_coverage_frac
            } else {
                0.0
            };
        let biolum = bio
            .bioluminescence_intensity
            .max(tidal_biolum)
            .clamp(0.0, 1.0);
        Vec4::new(
            bio.technosignature,
            bio.flora_pigment.shader_code(),
            biolum,
            surface_gravity_g,
        )
    } else {
        Vec4::new(0.0, 0.0, 0.0, surface_gravity_g)
    }
}

fn body_uses_shader_time(
    body_type: BodyType,
    is_cutaway: bool,
    params: &BodyMaterialParameters,
    aurora_params: Vec4,
    impact_basins_data: &[Vec4; 4],
) -> bool {
    if is_cutaway
        || body_type.is_star_or_remnant()
        || matches!(body_type, BodyType::GasGiant | BodyType::IceGiant)
    {
        return true;
    }
    if params.pressure_bar >= 0.001
        || params.cloud_density > 0.005
        || params.climate_and_bio.x > 0.005
        || params.dynamics_and_mag.y > 0.005
        || (aurora_params.x > 0.05 && aurora_params.z > 0.02)
    {
        return true;
    }
    impact_basins_data.iter().any(|b| b.w > 0.005 && b.x > 0.01)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Writes common planet uniforms and selects star or planet shading path"
)]
fn write_shared_shader_uniforms(
    mat: &mut PlanetMaterial,
    body: &CelestialBody,
    mass: &Mass,
    color: Color,
    eff_type: BodyType,
    eff_comp: &Composition,
    eff_temp_k: f64,
    params: &BodyMaterialParameters,
    elapsed_secs: f32,
    spin_angle: f32,
    axial_tilt: f32,
    translation: Vec3,
    scale: f32,
    star_dir: Vec3,
    opt_spin: Option<&SpinState>,
    opt_geo: Option<&GeologicalState>,
    opt_bhs: Option<&BlackHoleStarState>,
    opt_flare: Option<&StellarFlareState>,
    force_terrestrial_core: bool,
) {
    let cutaway_active = mat.extension.uniforms.cutaway_params.x > 0.5;
    let shader_time = if body_uses_shader_time(
        eff_type,
        cutaway_active,
        params,
        mat.extension.uniforms.aurora_params,
        &mat.extension.uniforms.impact_basins_data,
    ) {
        elapsed_secs
    } else {
        0.0
    };

    mat.base.base_color = color;
    mat.extension.uniforms.color_seed = LinearRgba::from(color).to_vec4();
    mat.extension.uniforms.temperature = eff_temp_k as f32;
    mat.extension.uniforms.time = shader_time;
    mat.extension.uniforms.spin_angle = spin_angle;
    mat.extension.uniforms.climate_and_bio = params.climate_and_bio;
    mat.extension.uniforms.atmosphere_params = params.atmosphere_params;
    mat.extension.uniforms.dynamics_and_mag = params.dynamics_and_mag;
    mat.extension.uniforms.planet_center_and_radius =
        Vec4::new(translation.x, translation.y, translation.z, scale);

    let profile = super::atmospheres::compute_atmosphere_spectral_profile(
        eff_type,
        eff_comp,
        eff_temp_k,
        params.pressure_bar,
        params.cloud_density,
    );
    let mut rayleigh_beta = profile.rayleigh_beta;
    let mut mie_g = profile.mie_asymmetry_g;
    if eff_type == BodyType::GasGiant
        && !force_terrestrial_core
        && crate::game::ui::world_list::contains_ignore_ascii_case(&body.name, "saturn")
    {
        // Delicate warm Cassini golden-champagne limb haze (low optical depth so central disk
        // retains rich butterscotch saturation while the sunlit grazing limb glows warmly)
        rayleigh_beta = Vec3::new(0.84, 0.68, 0.32) * 0.16;
        mie_g = 0.80;
    }
    mat.extension.uniforms.star_dir_and_lum = Vec4::new(star_dir.x, star_dir.y, star_dir.z, 1.0);
    mat.extension.uniforms.scattering_params =
        Vec4::new(rayleigh_beta.x, rayleigh_beta.y, rayleigh_beta.z, mie_g);
    mat.extension.uniforms.spin_axis = compute_spin_axis(opt_spin, opt_geo);
    mat.extension.uniforms.geological_params = opt_geo.map_or(Vec4::ZERO, |g| {
        Vec4::new(
            g.geological_age_gyr as f32,
            g.supercontinent_aggregation,
            g.ocean_oxidation_progress,
            g.terrestrial_vegetation_fraction,
        )
    });

    if body.body_type.is_star_or_remnant() {
        let is_blown_out = opt_bhs.is_some_and(|s| s.is_blown_out);
        update_star_material(
            mat,
            body.body_type,
            is_blown_out,
            color,
            params.mag_gauss,
            mass.0,
            axial_tilt,
            opt_bhs,
            opt_flare,
        );
    } else {
        let mat_name = if force_terrestrial_core {
            ""
        } else {
            &body.name
        };
        update_planet_material(mat, eff_type, mat_name, eff_comp, eff_temp_k, color);
        if force_terrestrial_core {
            mat.extension.uniforms.planet_type = 3;
            mat.extension.uniforms.color_seed.w = -1.0;
        }
    }
}

#[allow(clippy::too_many_arguments, reason = "Material sync system helper")]
pub fn update_body_material_properties(
    mat: &mut PlanetMaterial,
    body: &CelestialBody,
    mass: &Mass,
    temp: &Temperature,
    comp: &Composition,
    elapsed_secs: f32,
    elapsed_years: f64,
    current_dt_yr: f64,
    transform: &Transform,
    scale: f32,
    star_dir: Vec3,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_vol: Option<&VolatileInventory>,
    opt_spin: Option<&SpinState>,
    opt_em: Option<&ElectromagneticFieldState>,
    opt_bhs: Option<&BlackHoleStarState>,
    opt_basins: Option<&PlanetaryBasins>,
    opt_tidal: Option<&TidalState>,
    opt_geo: Option<&GeologicalState>,
    opt_aurora: Option<&AuroralOvalState>,
    opt_flare: Option<&StellarFlareState>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_storm: Option<&AtmosphericStormState>,
    show_atmospheres: bool,
    show_clouds: bool,
) {
    apply_impact_basins(mat, opt_basins, elapsed_years);

    let base_color = compute_body_color(body, mass, temp, comp, opt_tidal);
    let rotations_per_year = if let Some(spin) = opt_spin {
        let sign = if spin.rotation_period_hours < 0.0 {
            -1.0
        } else {
            1.0
        };
        sign * (8766.0 / spin.rotation_period_hours.abs().max(0.01))
    } else {
        100.0
    };
    let spin_angle = if current_dt_yr > 0.0 {
        let delta_angle = ((current_dt_yr * rotations_per_year * std::f64::consts::TAU)
            % std::f64::consts::TAU) as f32;
        (mat.extension.uniforms.spin_angle + delta_angle).rem_euclid(std::f32::consts::TAU)
    } else if elapsed_years == 0.0 {
        0.0
    } else {
        mat.extension.uniforms.spin_angle
    };
    let axial_tilt = opt_spin.map_or(0.08, |s| (s.axial_tilt_degrees as f32).to_radians());

    let mut params = compute_body_material_parameters(
        body.body_type,
        comp,
        temp.0,
        mass.0,
        opt_climate,
        opt_bio,
        opt_vol,
        opt_em,
        opt_diff,
        opt_tidal,
        axial_tilt,
    );

    let aurora_params = compute_aurora_uniforms(opt_aurora, params.mag_gauss);
    mat.extension.uniforms.aurora_params = aurora_params;
    mat.extension.uniforms.civilization_params =
        compute_civilization_uniforms(mass.0, opt_bio, opt_tidal);

    let (storm_features, storm_dynamics) = compute_storm_uniforms(opt_storm, &body.name);
    mat.extension.uniforms.storm_features = storm_features;
    mat.extension.uniforms.storm_dynamics = storm_dynamics;

    let (eff_type, eff_comp, eff_temp_k, override_color, force_terrestrial_core) =
        apply_atmosphere_and_cloud_view_overrides(
            mat,
            body,
            mass,
            comp,
            temp.0,
            opt_diff,
            opt_tidal,
            &mut params,
            show_atmospheres,
            show_clouds,
        );
    let color =
        if force_terrestrial_core || (!show_atmospheres && !body.body_type.is_star_or_remnant()) {
            override_color
        } else {
            base_color
        };

    write_shared_shader_uniforms(
        mat,
        body,
        mass,
        color,
        eff_type,
        &eff_comp,
        eff_temp_k,
        &params,
        elapsed_secs,
        spin_angle,
        axial_tilt,
        transform.translation,
        scale,
        star_dir,
        opt_spin,
        opt_geo,
        opt_bhs,
        opt_flare,
        force_terrestrial_core,
    );
}
