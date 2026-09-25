use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::geology::GeologicalState;
use crate::simulation::resources::*;
use crate::simulation::space_weather::{AuroralOvalState, StellarFlareState};
use crate::simulation::tides::TidalState;

use super::palettes::*;
use super::shadows::*;
use super::VisualAssets;

/// Calculates the fraction of solid crust covering a magma ocean at temperature `temp_k`.
///
/// Returns 0.0 for completely molten oceans (>= 1800 K), 0.5 around 1125 K,
/// and 1.0 for fully solidified crust (<= 450 K).
pub fn compute_magma_ocean_crust_fraction(temp_k: f32) -> f32 {
    (1.0 - (temp_k - 450.0) / 1350.0).clamp(0.0, 1.0)
}

/// Computes the blackbody incandescence color and thermal emissive intensity
/// for magma at a given temperature and rift fissure intensity.
pub fn compute_magma_incandescence(temp_k: f32, fissure_intensity: f32) -> (Vec3, f32) {
    let core_incandescent = Vec3::new(1.0, 0.95, 0.65);
    let molten_orange = Vec3::new(1.0, 0.42, 0.07);
    let deep_crimson = Vec3::new(0.55, 0.09, 0.03);

    let rift_heat = (fissure_intensity * 1.6 - 0.4).clamp(0.0, 1.0);
    let high_t = ((temp_k - 1200.0) / 800.0).clamp(0.0, 1.0);
    let top_tone = molten_orange.lerp(core_incandescent, high_t);
    let lava_tone = deep_crimson.lerp(top_tone, rift_heat);

    let glow_scale = ((temp_k - 450.0) / 450.0).clamp(0.0, 6.0);
    (lava_tone, glow_scale)
}

fn update_star_lights_and_strobes(
    opt_children: Option<&Children>,
    light_query: &mut Query<&mut PointLight>,
    body_type: BodyType,
    is_blown_out: bool,
    elapsed_secs: f32,
) {
    let Some(children) = opt_children else {
        return;
    };
    for child in children.iter() {
        if let Ok(mut light) = light_query.get_mut(child) {
            if body_type == BodyType::BlackHole {
                // Gravitational lensing ring — no photon emission
                light.intensity = 2_000_000.0;
            } else if body_type == BodyType::QuasiStar {
                light.color = Color::srgb(1.0, 0.10, 0.02);
                light.intensity = if is_blown_out {
                    2_000_000.0
                } else {
                    // Eddington-luminosity quasi-star: blinding at close range
                    60_000_000.0
                };
            } else if body_type == BodyType::Pulsar {
                let strobe = (elapsed_secs * 24.0).sin().abs().powi(4);
                light.color = Color::srgb(0.70, 0.90, 1.0);
                // Peak X-ray millisecond pulsar flash
                light.intensity = 8_000_000.0 + strobe * 80_000_000.0;
            } else if body_type == BodyType::Magnetar {
                let flare = ((elapsed_secs * 5.0).sin() * (elapsed_secs * 13.0).cos())
                    .abs()
                    .powf(1.8);
                light.color = Color::srgb(0.85, 0.60, 1.0);
                // Soft gamma repeater burst
                light.intensity = 12_000_000.0 + flare * 120_000_000.0;
            } else if body_type == BodyType::NeutronStar {
                light.color = Color::srgb(0.65, 0.82, 1.0);
                light.intensity = 30_000_000.0;
            } else if body_type == BodyType::WhiteDwarf {
                light.color = Color::srgb(0.85, 0.92, 1.0);
                light.intensity = 25_000_000.0;
            } else {
                // Main-sequence stars (Sun-like, Red Dwarf, etc.)
                // Physical solar luminosity at 1 AU produces ~1361 W/m² irradiance.
                // We scale so nearby objects appear realistically illuminated in HDR.
                light.intensity = 18_000_000.0;
            }
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
        mat.base.unlit = false;
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
        let star_subtype = star_subtype_from_body_type(body_type);
        // HDR emissive radiance. Values are scene-linear; Bloom threshold is 1.8.
        // Multipliers must push above that to generate a glow halo.
        // These are calibrated so stars look incandescent, not glowing-plastic.
        let mult = match body_type {
            BodyType::WhiteDwarf => 220.0, // Intense blue-white ultraviolet furnace
            BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => 280.0,
            BodyType::Protostar => 55.0, // Young, luminous but not fully ignited
            BodyType::RedDwarf => 80.0,
            BodyType::BrownDwarf => 28.0, // Barely above threshold — faint deep-red glow
            BodyType::RedGiant | BodyType::RedSupergiant => 95.0,
            BodyType::BlueGiant | BodyType::BlueSupergiant | BodyType::Hypergiant => 350.0,
            BodyType::WolfRayet => 310.0,
            _ => 120.0, // Main-sequence yellow/orange dwarfs (Sun-like)
        };
        let flare_mult = opt_flare.map_or(1.0, |f| 1.0 + f.current_flare_intensity * 2.0);
        let emissive_boost = opt_flare.map_or(1.0, |f| 1.0 + f.current_flare_intensity * 1.8);
        mat.base.emissive = LinearRgba::from(color) * (mult * emissive_boost);

        let (cell_scale, mut flare_intensity, pulse_freq) = match body_type {
            BodyType::RedGiant | BodyType::RedSupergiant => (4.0, 0.35, 0.2),
            BodyType::RedDwarf => (14.0, 1.4, 0.6),
            BodyType::BrownDwarf => (8.0, 0.2, 0.3),
            BodyType::BlueGiant | BodyType::BlueSupergiant | BodyType::Hypergiant => {
                (22.0, 0.8, 1.5)
            }
            BodyType::NeutronStar => (40.0, 1.5, 2.0),
            BodyType::Pulsar => (35.0, 2.2, 6.5),
            BodyType::Magnetar => (16.0, 3.8, 1.0),
            BodyType::WhiteDwarf => (30.0, 0.2, 0.5),
            BodyType::Protostar => (10.0, 0.9, 0.4),
            BodyType::WolfRayet => (18.0, 2.0, 2.5),
            _ => (24.0, 0.5, 0.3),
        };
        flare_intensity *= flare_mult;
        mat.extension.uniforms.composition =
            Vec4::new(star_subtype, cell_scale, flare_intensity, pulse_freq);

        let spot_coverage = match body_type {
            BodyType::RedDwarf => 0.28,
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
    let lower = name.to_lowercase();
    let effective_type = if lower.contains("uranus") || lower.contains("neptune") {
        BodyType::IceGiant
    } else {
        body_type
    };
    mat.extension.uniforms.planet_type = match effective_type {
        BodyType::GasGiant => 1,
        BodyType::IceGiant => 2,
        BodyType::SuperEarth => 6,
        BodyType::TerrestrialPlanet | BodyType::Protoplanet | BodyType::Moon => {
            if norm_comp.ice_frac > 0.40 {
                2
            } else {
                3
            }
        }
        _ => 4,
    };

    if comp.metal_frac > 0.4 {
        mat.base.metallic = 0.85;
        mat.base.perceptual_roughness = 0.25;
    } else if comp.ice_frac > 0.4 {
        mat.base.metallic = 0.05;
        mat.base.perceptual_roughness = 0.18;
    } else if comp.gas_frac > 0.5 {
        mat.base.metallic = 0.0;
        mat.base.perceptual_roughness = 0.85;
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

fn select_body_mesh(body: &CelestialBody, visual_assets: &VisualAssets) -> Handle<Mesh> {
    super::meshes::select_body_mesh(body, visual_assets)
}

fn apply_impact_basins(mat: &mut PlanetMaterial, opt_basins: Option<&PlanetaryBasins>) {
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
                *data = Vec4::new(
                    basin.melt_glow_fraction,
                    basin.elongation,
                    0.0,
                    basin.scar_intensity,
                );
            }
        }
    }
    mat.extension.uniforms.impact_basins_pos = basins_pos;
    mat.extension.uniforms.impact_basins_data = basins_data;
}

fn compute_body_color(
    body: &CelestialBody,
    mass: &Mass,
    temp: &Temperature,
    comp: &Composition,
    opt_tidal: Option<&TidalState>,
) -> Color {
    let lower = body.name.to_lowercase();
    let is_star_like = body.body_type.is_star_or_remnant();
    let is_ice_giant = body.body_type == BodyType::IceGiant
        || lower.contains("uranus")
        || lower.contains("neptune");
    let is_gas_giant = !is_ice_giant
        && (body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.45);
    if is_star_like {
        compute_stellar_palette(body.body_type, temp.0)
    } else if is_ice_giant {
        super::palettes::compute_ice_giant_palette(&body.name, temp.0)
    } else if is_gas_giant {
        compute_gas_giant_palette(mass.0, temp.0, &body.name)
    } else if matches!(body.body_type, BodyType::Asteroid | BodyType::Planetesimal) {
        compute_asteroid_spectral_palette(&body.name, comp)
    } else if body.body_type == BodyType::Comet {
        compute_comet_spectral_palette(&body.name, comp)
    } else {
        compute_terrestrial_body_palette(&body.name, comp, temp.0, opt_tidal)
    }
}

struct BodyMaterialParameters {
    climate_and_bio: Vec4,
    atmosphere_params: Vec4,
    dynamics_and_mag: Vec4,
    pressure_bar: f32,
    cloud_density: f32,
    mag_gauss: f32,
}

#[allow(clippy::too_many_arguments, reason = "Material parameter builder")]
fn compute_body_material_parameters(
    body_type: BodyType,
    comp: &Composition,
    temp_k: f64,
    _mass_solar: f64,
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
    let has_water_volatiles =
        norm_comp.ice_frac > 0.001 || opt_vol.is_some_and(|v| v.delivered_water_m_earth > 1e-6);
    let ocean_frac = if is_minor_body {
        0.0
    } else if has_water_volatiles {
        opt_vol.map_or(norm_comp.ice_frac as f32, |v| v.ocean_coverage_frac)
    } else {
        0.0
    };
    let ice_frac = if is_minor_body {
        norm_comp.ice_frac as f32
    } else if has_water_volatiles {
        opt_climate.map_or(0.0, |c| c.ice_coverage_frac)
    } else {
        0.0
    };
    let biomass_frac = if !is_minor_body && has_water_volatiles && ocean_frac > 0.01 {
        opt_bio.map_or(0.0, |b| b.biomass_coverage_frac)
    } else {
        0.0
    };
    let cloud_density = if !is_minor_body && (has_water_volatiles || norm_comp.gas_frac > 0.02) {
        opt_climate.map_or(norm_comp.gas_frac as f32, |c| c.cloud_coverage_frac)
    } else {
        0.0
    };

    let pressure_bar = if is_minor_body {
        0.0
    } else {
        opt_vol.map_or((norm_comp.gas_frac as f32 * 2.0).max(0.01), |v| {
            v.atmospheric_pressure_bar
        })
    };
    let scale_height = (0.08f32 * (temp_k as f32 / 288.0f32).sqrt()).clamp(0.02f32, 0.25f32);
    let haze_density = if is_minor_body {
        0.0
    } else {
        opt_climate.map_or((norm_comp.gas_frac as f32 * 1.5).clamp(0.0, 1.0), |c| {
            (c.cloud_coverage_frac * 1.2).clamp(0.0, 1.0)
        })
    };
    let greenhouse = opt_climate.map_or(33.0, |c| c.greenhouse_delta_k);

    let b_em = opt_em.map_or(0.0, |e| e.magnetic_field_gauss as f32);
    let b_diff = opt_diff.map_or(0.0, |d| d.magnetic_field_gauss as f32);
    let mag_gauss = b_em.max(b_diff);
    let tidal_lava_boost = opt_tidal.map_or(0.0, |t| {
        if t.tidal_heating_flux_w_m2 > 0.5 {
            ((t.tidal_heating_flux_w_m2 as f32 - 0.5) / 2.5).clamp(0.0, 0.75)
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

    let liquid_ocean_frac = if temp_k > 380.0 {
        0.0
    } else if temp_k > 340.0 {
        let boil_factor = (380.0 - temp_k) / 40.0;
        (ocean_frac * boil_factor as f32).clamp(0.0, 1.0)
    } else {
        ocean_frac
    };

    let climate_and_bio = Vec4::new(liquid_ocean_frac, ice_frac, biomass_frac, cloud_density);
    let atmosphere_params = Vec4::new(
        pressure_bar,
        scale_height,
        haze_density,
        (greenhouse / 33.0).clamp(0.1, 8.0),
    );

    let storm_intensity =
        opt_climate.map_or(0.2, |c| (c.cloud_coverage_frac * 1.5).clamp(0.0, 1.5));
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
        if spin.spin_vector.length_squared() > 1e-12
            && (spin.spin_vector.x.abs() > 1e-6 || spin.spin_vector.z.abs() > 1e-6)
        {
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
    opt_aurora.map_or_else(
        || {
            if mag_gauss > 0.1 {
                Vec4::new(0.315, 0.065, (mag_gauss * 0.8).clamp(0.2, 2.5), 1.5)
            } else {
                Vec4::ZERO
            }
        },
        |a| {
            Vec4::new(
                a.oval_colatitude_rad,
                a.oval_width_rad,
                a.auroral_intensity,
                a.geomagnetic_kp_index,
            )
        },
    )
}

fn compute_storm_uniforms(
    opt_storm: Option<&AtmosphericStormState>,
    body_name: &str,
) -> (Vec4, Vec4) {
    let lower_name = body_name.to_lowercase();
    let storm = opt_storm.copied().or_else(|| {
        if lower_name.contains("saturn") {
            Some(AtmosphericStormState::saturn())
        } else if lower_name.contains("jupiter") {
            Some(AtmosphericStormState::jupiter())
        } else if lower_name.contains("neptune") {
            Some(AtmosphericStormState::neptune())
        } else {
            None
        }
    });

    storm.map_or_else(
        || (Vec4::ZERO, Vec4::new(0.0, 1.0, 0.0, 0.5)),
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

#[allow(clippy::too_many_arguments, reason = "Material sync system helper")]
fn update_body_material_properties(
    mat: &mut PlanetMaterial,
    body: &CelestialBody,
    mass: &Mass,
    temp: &Temperature,
    comp: &Composition,
    elapsed_secs: f32,
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
) {
    apply_impact_basins(mat, opt_basins);

    let color = compute_body_color(body, mass, temp, comp, opt_tidal);
    let spin_rate = opt_spin.map_or(0.22, |s| {
        (24.0 / s.rotation_period_hours.max(0.1)) as f32 * 0.22
    });
    let axial_tilt = opt_spin.map_or(0.08, |s| (s.axial_tilt_degrees as f32).to_radians());

    let params = compute_body_material_parameters(
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

    mat.base.base_color = color;
    mat.extension.uniforms.color_seed = LinearRgba::from(color).to_vec4();
    mat.extension.uniforms.temperature = temp.0 as f32;
    mat.extension.uniforms.time = elapsed_secs;
    mat.extension.uniforms.spin_rate = spin_rate;
    mat.extension.uniforms.climate_and_bio = params.climate_and_bio;
    mat.extension.uniforms.atmosphere_params = params.atmosphere_params;
    mat.extension.uniforms.dynamics_and_mag = params.dynamics_and_mag;

    let profile = super::atmospheres::compute_atmosphere_spectral_profile(
        body.body_type,
        comp,
        temp.0,
        params.pressure_bar,
        params.cloud_density,
    );
    mat.extension.uniforms.star_dir_and_lum = Vec4::new(star_dir.x, star_dir.y, star_dir.z, 1.0);
    mat.extension.uniforms.scattering_params = Vec4::new(
        profile.rayleigh_beta.x,
        profile.rayleigh_beta.y,
        profile.rayleigh_beta.z,
        profile.mie_asymmetry_g,
    );
    mat.extension.uniforms.spin_axis = compute_spin_axis(opt_spin, opt_geo);
    mat.extension.uniforms.geological_params = opt_geo.map_or(Vec4::ZERO, |g| {
        Vec4::new(
            g.geological_age_gyr,
            g.supercontinent_aggregation,
            g.ocean_oxidation_progress,
            g.terrestrial_vegetation_fraction,
        )
    });

    mat.extension.uniforms.aurora_params = compute_aurora_uniforms(opt_aurora, params.mag_gauss);

    let (storm_features, storm_dynamics) = compute_storm_uniforms(opt_storm, &body.name);
    mat.extension.uniforms.storm_features = storm_features;
    mat.extension.uniforms.storm_dynamics = storm_dynamics;

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
        update_planet_material(mat, body.body_type, &body.name, comp, temp.0, color);
    }
}

pub type CelestialEnvironmentOptions<'a> = (
    Option<&'a PlanetaryClimate>,
    Option<&'a BiosphereState>,
    Option<&'a VolatileInventory>,
    Option<&'a PlanetaryRingSystem>,
    Option<&'a TidalState>,
    Option<&'a GeologicalState>,
);

pub type CelestialDynamicsOptions<'a> = (
    Option<&'a SpinState>,
    Option<&'a ElectromagneticFieldState>,
    Option<&'a BlackHoleStarState>,
    Option<&'a Children>,
    Option<&'a PlanetaryBasins>,
    Option<&'a SatelliteOf>,
);

pub type CelestialInternalOptions<'a> = (
    Option<&'a AuroralOvalState>,
    Option<&'a StellarFlareState>,
    Option<&'a InternalDifferentiation>,
    Option<&'a AtmosphericStormState>,
);

pub type CelestialBodyQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    &'a Mass,
    &'a Radius,
    &'a Temperature,
    &'a Composition,
    &'a CelestialBody,
    &'a mut Transform,
    &'a MeshMaterial3d<PlanetMaterial>,
    &'a mut Mesh3d,
    CelestialEnvironmentOptions<'a>,
    CelestialDynamicsOptions<'a>,
    CelestialInternalOptions<'a>,
);

pub type CelestialBodyQuery<'w, 's> = Query<'w, 's, CelestialBodyQueryItem<'static>>;

fn find_star_position_and_min_orbit(query: &CelestialBodyQuery) -> (Vec3, Option<Entity>, f32) {
    let mut s_pos = Vec3::ZERO;
    let mut s_ent = None;
    for (e, p, _, _, _, _, b, ..) in query.iter() {
        if b.body_type.is_star_or_remnant() {
            s_pos = Vec3::new(p.x as f32, p.y as f32, p.z as f32);
            s_ent = Some(e);
            break;
        }
    }
    let mut min_r = f32::MAX;
    for (e, p, _, _, _, _, b, ..) in query.iter() {
        if Some(e) != s_ent && !b.body_type.is_star_or_remnant() && b.body_type != BodyType::Moon {
            let r = (Vec3::new(p.x as f32, p.y as f32, p.z as f32) - s_pos).length();
            if r > 0.001 && r < min_r {
                min_r = r;
            }
        }
    }
    (s_pos, s_ent, if min_r < f32::MAX { min_r } else { 0.4 })
}

fn collect_system_moons(
    query: &CelestialBodyQuery,
    config: &SimulationConfig,
    star_pos: Vec3,
    min_orbit_r: f32,
) -> Vec<(Entity, Vec3, f32, Option<Entity>)> {
    let mut all_moons = Vec::with_capacity(8);
    for (m_ent, pos, _, radius, _, _, body, _, _, _, _, (_, _, _, _, _, opt_sat), _) in query.iter()
    {
        if opt_sat.is_some() || body.body_type == BodyType::Moon {
            let m_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            let r_orb = (m_pos - star_pos).length();
            let m_vis_rad =
                config.calc_visual_radius_with_orbit(radius.0, body.body_type, r_orb, min_orbit_r);
            all_moons.push((m_ent, m_pos, m_vis_rad, opt_sat.map(|s| s.parent)));
        }
    }
    all_moons
}

pub fn sync_celestial_transforms(
    sim_time: Option<Res<SimTime>>,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    mut light_query: Query<&mut PointLight>,
    mut query: CelestialBodyQuery,
) {
    let (star_pos, _star_entity, min_orbit_r) = find_star_position_and_min_orbit(&query);
    let all_moons = collect_system_moons(&query, &config, star_pos, min_orbit_r);
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);

    for (
        entity,
        pos,
        mass,
        radius,
        temp,
        comp,
        body,
        mut transform,
        mat_handle,
        mut mesh,
        (opt_climate, opt_bio, opt_vol, opt_rings, opt_tidal, opt_geo),
        (opt_spin, opt_em, opt_bhs, opt_children, opt_basins, _),
        (opt_aurora, opt_flare, opt_diff, opt_storm),
    ) in query.iter_mut()
    {
        transform.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let star_dir = (star_pos - transform.translation).normalize_or_zero();
        let is_blown_out = opt_bhs.is_some_and(|s| s.is_blown_out);
        update_star_lights_and_strobes(
            opt_children,
            &mut light_query,
            body.body_type,
            is_blown_out,
            visual_time,
        );

        let r_orb = (transform.translation - star_pos).length();
        let visual_radius =
            config.calc_visual_radius_with_orbit(radius.0, body.body_type, r_orb, min_orbit_r);
        transform.scale = Vec3::splat(visual_radius);

        if let Some(spin) = opt_spin {
            let spin_dir = if spin.spin_vector.length_squared() > 1e-12
                && (spin.spin_vector.x.abs() > 1e-6 || spin.spin_vector.z.abs() > 1e-6)
            {
                spin.spin_vector.normalize().as_vec3()
            } else {
                let tilt_rad = (spin.axial_tilt_degrees as f32).to_radians();
                Vec3::new(tilt_rad.sin(), tilt_rad.cos(), 0.0)
            };
            transform.rotation = Quat::from_rotation_arc(Vec3::Y, spin_dir);
        }

        let target_mesh = select_body_mesh(body, &visual_assets);
        if mesh.0 != target_mesh {
            mesh.0 = target_mesh;
        }

        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            update_body_material_properties(
                &mut mat,
                body,
                mass,
                temp,
                comp,
                visual_time,
                star_dir,
                opt_climate,
                opt_bio,
                opt_vol,
                opt_spin,
                opt_em,
                opt_bhs,
                opt_basins,
                opt_tidal,
                opt_geo,
                opt_aurora,
                opt_flare,
                opt_diff,
                opt_storm,
            );

            // Feature 3.2: Update real-time ring shadow parameters
            mat.extension.uniforms.ring_shadow_params =
                compute_ring_shadow_params(opt_rings, radius.0);

            // Feature 3.2: Update real-time moon solar eclipse parameters
            let (moons_pos, moons_data) = compute_eclipse_moons_data(
                entity,
                transform.translation,
                visual_radius,
                star_dir,
                &all_moons,
            );
            mat.extension.uniforms.eclipse_moons_pos = moons_pos;
            mat.extension.uniforms.eclipse_moons_data = moons_data;
        }
    }
}
