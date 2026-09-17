use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::geology::GeologicalState;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;
use crate::utils::constants::*;

use super::palettes::*;
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
                light.intensity = 1_500_000.0;
            } else if body_type == BodyType::QuasiStar {
                light.color = Color::srgb(1.0, 0.10, 0.02);
                light.intensity = if is_blown_out {
                    1_500_000.0
                } else {
                    12_000_000.0
                };
            } else if body_type == BodyType::Pulsar {
                let strobe = (elapsed_secs * 24.0).sin().abs().powi(4);
                light.color = Color::srgb(0.70, 0.90, 1.0);
                light.intensity = 3_000_000.0 + strobe * 25_000_000.0;
            } else if body_type == BodyType::Magnetar {
                let flare = ((elapsed_secs * 5.0).sin() * (elapsed_secs * 13.0).cos())
                    .abs()
                    .powf(1.8);
                light.color = Color::srgb(0.85, 0.60, 1.0);
                light.intensity = 4_000_000.0 + flare * 30_000_000.0;
            }
        }
    }
}

fn update_star_material(
    mat: &mut PlanetMaterial,
    body_type: BodyType,
    is_blown_out: bool,
    color: Color,
    mag_gauss: f32,
    mass: f64,
    axial_tilt: f32,
    opt_bhs: Option<&BlackHoleStarState>,
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
        mat.extension.uniforms.dynamics_and_mag =
            Vec4::new(mag_gauss.max(1.0e6), edd_ratio, mass as f32, axial_tilt);
        mat.extension.uniforms.atmosphere_params = Vec4::new(0.65, 1.25, 0.0, 1.0);
        mat.extension.uniforms.composition = Vec4::new(0.0, 4.5, 1.5, 0.8);
    } else {
        mat.extension.uniforms.planet_type = 0;
        mat.base.unlit = true;
        let star_subtype = star_subtype_from_body_type(body_type);
        let mult = match body_type {
            BodyType::WhiteDwarf => 35.0,
            BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar => 45.0,
            BodyType::Protostar => 14.0,
            _ => 30.0,
        };
        mat.base.emissive = LinearRgba::from(color) * mult;

        let (cell_scale, flare_intensity, pulse_freq) = match body_type {
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
            Vec4::new(mag_gauss, 1.0, mass as f32, axial_tilt);
    }
}

fn update_planet_material(
    mat: &mut PlanetMaterial,
    body_type: BodyType,
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
    mat.extension.uniforms.planet_type = match body_type {
        BodyType::GasGiant => 1,
        BodyType::IceGiant => 2,
        BodyType::SuperEarth => 6,
        BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
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
    if body.body_type.is_star_or_remnant() {
        visual_assets.star_mesh.clone()
    } else {
        match body.body_type {
            BodyType::Comet => visual_assets.comet_bilobate_mesh.clone(),
            BodyType::Asteroid => {
                let hash: usize = body.name.bytes().map(|b| b as usize).sum();
                if hash.is_multiple_of(2) {
                    visual_assets.asteroid_potato_mesh.clone()
                } else {
                    visual_assets.asteroid_rubble_mesh.clone()
                }
            }
            BodyType::DustGrain => visual_assets.particle_mesh.clone(),
            _ => visual_assets.planet_mesh.clone(),
        }
    }
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
) -> Color {
    let (br, bg, bb) = blackbody_to_srgb(temp.0);
    let (cr, cg, cb) = comp.visual_color_tint();

    let is_star_like = body.body_type.is_star_or_remnant();
    let is_gas_giant = body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.30;
    if is_star_like {
        compute_stellar_palette(body.body_type, temp.0)
    } else if is_gas_giant {
        compute_gas_giant_palette(mass.0, temp.0, &body.name)
    } else {
        Color::srgb(
            (br * 0.25 + cr * 0.75).clamp(0.1, 1.0),
            (bg * 0.25 + cg * 0.75).clamp(0.1, 1.0),
            (bb * 0.25 + cb * 0.75).clamp(0.1, 1.0),
        )
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
    mass_solar: f64,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_vol: Option<&VolatileInventory>,
    opt_em: Option<&ElectromagneticFieldState>,
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

    let mag_gauss = opt_em.map_or(0.0, |e| e.magnetic_field_gauss as f32);
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
        ocean_frac * ((380.0 - temp_k as f32) / 40.0).clamp(0.0, 1.0)
    } else {
        ocean_frac
    };
    let steam_cloud_boost = if temp_k > 340.0 && ocean_frac > 0.01 {
        ((temp_k as f32 - 340.0) / 60.0).clamp(0.0, 0.50)
    } else {
        0.0
    };
    let effective_cloud_density = (cloud_density + steam_cloud_boost).clamp(0.0, 0.98);
    let mass_jup = (mass_solar / JUPITER_MASS_SOLAR) as f32;

    BodyMaterialParameters {
        climate_and_bio: Vec4::new(
            liquid_ocean_frac,
            ice_frac,
            biomass_frac,
            effective_cloud_density,
        ),
        atmosphere_params: Vec4::new(pressure_bar, scale_height, haze_density, greenhouse),
        dynamics_and_mag: Vec4::new(mag_gauss, lava_frac, mass_jup, axial_tilt),
        pressure_bar,
        cloud_density,
        mag_gauss,
    }
}

fn compute_spin_axis(opt_spin: Option<&SpinState>) -> Vec4 {
    if let Some(spin) = opt_spin {
        if spin.spin_vector.length_squared() > 1e-16 {
            let n = spin.spin_vector.normalize();
            Vec4::new(n.x as f32, n.y as f32, n.z as f32, 0.0)
        } else {
            let tilt = (spin.axial_tilt_degrees as f32).to_radians();
            Vec4::new(tilt.sin(), tilt.cos(), 0.0, 0.0)
        }
    } else {
        Vec4::new(0.0, 1.0, 0.0, 0.0)
    }
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
) {
    apply_impact_basins(mat, opt_basins);

    let color = compute_body_color(body, mass, temp, comp);
    let spin_rate = opt_spin.map_or(0.15, |s| {
        (24.0 / s.rotation_period_hours.max(0.1)) as f32 * 0.15
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
    mat.extension.uniforms.spin_axis = compute_spin_axis(opt_spin);
    let geo_params = opt_geo.map_or(Vec4::new(4.56, 0.0, 1.0, 1.0), |g| {
        Vec4::new(
            g.geological_age_gyr,
            g.continental_drift_phase,
            g.ocean_oxidation_progress,
            g.terrestrial_vegetation_fraction,
        )
    });
    mat.extension.uniforms.geological_params = geo_params;

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
        );
    } else {
        update_planet_material(mat, body.body_type, comp, temp.0, color);
    }
}

/// Calculates the planetary shadow factor on a ring fragment (0.0 = full umbra, 1.0 = full sunlight).
pub fn compute_planetary_ring_shadow(
    ring_pos_norm: Vec2,
    star_dir_local: Vec3,
    planet_rad_norm: f32,
) -> f32 {
    let p_ring = Vec3::new(ring_pos_norm.x, 0.0, ring_pos_norm.y);
    let s_closest = -p_ring.dot(star_dir_local);
    if s_closest <= 0.0 {
        return 1.0;
    }
    let p_closest = p_ring + s_closest * star_dir_local;
    let d_closest = p_closest.length();
    let penumbra = 0.022f32;
    let umbra_r = (planet_rad_norm - penumbra).max(0.0);
    let penumbra_r = planet_rad_norm + penumbra;
    if d_closest <= umbra_r {
        0.0
    } else if d_closest >= penumbra_r {
        1.0
    } else {
        let t = (d_closest - umbra_r) / (penumbra_r - umbra_r);
        t * t * (3.0 - 2.0 * t)
    }
}

/// Calculates ring shadow attenuation on a planet surface point (1.0 = unshadowed, 0.0 = total darkness).
pub fn compute_ring_shadow_on_planet(
    surf_norm: Vec3,
    star_dir: Vec3,
    spin_axis: Vec3,
    r_inner: f32,
    r_outer: f32,
    opt_depth: f32,
) -> f32 {
    let l_dot_s = star_dir.dot(spin_axis);
    let p_dot_s = surf_norm.dot(spin_axis);
    if l_dot_s.abs() <= 1e-4 {
        return 1.0;
    }
    let t_ring = -p_dot_s / l_dot_s;
    let n_dot_l = surf_norm.dot(star_dir);
    if t_ring <= 0.0 || n_dot_l <= 0.0 {
        return 1.0;
    }
    let p_int = surf_norm + t_ring * star_dir;
    let r_int = p_int.length();
    if r_int < r_inner || r_int > r_outer {
        return 1.0;
    }
    let u = (r_int - r_inner) / (r_outer - r_inner);
    let ring_density = if u < 0.22 {
        0.10 + 0.25 * (u / 0.22)
    } else if u < 0.65 {
        0.95 // B-Ring
    } else if u < 0.72 {
        let gap_t = (u - 0.65) / (0.72 - 0.65);
        (1.0 - (gap_t * std::f32::consts::PI).sin()) * 0.12 // Cassini division
    } else if u < 0.96 {
        0.75 // A-Ring
    } else {
        (1.0 - (u - 0.96) / 0.04) * 0.35
    };
    let edge_feather =
        ((r_int - r_inner) / 0.03).clamp(0.0, 1.0) * ((r_outer - r_int) / 0.03).clamp(0.0, 1.0);
    let shadow_atten = (ring_density * opt_depth * edge_feather).clamp(0.0, 0.96);
    1.0 - shadow_atten
}

/// Calculates moon solar eclipse illumination on a planet surface point (1.0 = daylight, 0.0 = total umbra).
pub fn compute_moon_eclipse_shadow(
    surf_norm: Vec3,
    star_dir: Vec3,
    moon_rel_pos_norm: Vec3,
    moon_rad_norm: f32,
) -> f32 {
    let v = moon_rel_pos_norm - surf_norm;
    let t_close = v.dot(star_dir);
    if t_close <= 0.0 {
        return 1.0;
    }
    let d_sq = v.length_squared() - t_close * t_close;
    let max_r = moon_rad_norm * 1.45;
    if d_sq >= max_r * max_r {
        return 1.0;
    }
    let d_perp = d_sq.max(0.0).sqrt();
    let umbra_r = moon_rad_norm * 0.70;
    let penumbra_r = moon_rad_norm * 1.25;
    if d_perp <= umbra_r {
        0.04
    } else if d_perp >= penumbra_r {
        1.0
    } else {
        let t = (d_perp - umbra_r) / (penumbra_r - umbra_r);
        let smooth_t = t * t * (3.0 - 2.0 * t);
        0.04 + 0.96 * smooth_t
    }
}

fn compute_ring_shadow_params(opt_rings: Option<&PlanetaryRingSystem>, radius_au: f64) -> Vec4 {
    if let Some(ring_sys) = opt_rings {
        let ring_ratio = if ring_sys.outer_radius_au > 0.0 && radius_au > 0.0 {
            (ring_sys.outer_radius_au / radius_au as f32).clamp(2.0, 3.5)
        } else {
            2.85
        };
        let inner_ratio = if ring_sys.outer_radius_au > 0.0 && ring_sys.inner_radius_au > 0.0 {
            (ring_sys.inner_radius_au / radius_au as f32).clamp(1.1, ring_ratio - 0.1)
        } else {
            1.25
        };
        Vec4::new(inner_ratio, ring_ratio, ring_sys.optical_depth, 1.0)
    } else {
        Vec4::ZERO
    }
}

fn compute_eclipse_moons_data(
    entity: Entity,
    planet_pos: Vec3,
    visual_radius: f32,
    star_dir: Vec3,
    all_moons: &[(Entity, Vec3, f32, Option<Entity>)],
) -> ([Vec4; 2], [Vec4; 2]) {
    let mut moons_pos = [Vec4::ZERO; 2];
    let mut moons_data = [Vec4::ZERO; 2];
    let mut slot = 0;
    for &(m_ent, m_pos, m_rad, opt_parent) in all_moons {
        if slot >= 2 {
            break;
        }
        if m_ent == entity {
            continue;
        }
        let is_child_moon = opt_parent == Some(entity)
            || (opt_parent.is_none() && (m_pos - planet_pos).length() < visual_radius * 40.0);
        if is_child_moon {
            let d_vec = m_pos - planet_pos;
            let s = d_vec.dot(star_dir);
            if s > 0.0 {
                let d_perp_sq = d_vec.length_squared() - s * s;
                let max_touch_dist = visual_radius + m_rad * 1.5;
                if d_perp_sq < max_touch_dist * max_touch_dist {
                    let norm_pos = d_vec / visual_radius.max(1e-5);
                    let norm_rad = (m_rad / visual_radius.max(1e-5)).clamp(0.01, 1.0);
                    if let (Some(pos_slot), Some(data_slot)) =
                        (moons_pos.get_mut(slot), moons_data.get_mut(slot))
                    {
                        *pos_slot = Vec4::new(norm_pos.x, norm_pos.y, norm_pos.z, norm_rad);
                        *data_slot = Vec4::new(1.0, 0.25, 0.96, 0.0);
                        slot += 1;
                    }
                }
            }
        }
    }
    (moons_pos, moons_data)
}

#[allow(clippy::type_complexity, reason = "Celestial Transform Sync")]
pub fn sync_celestial_transforms(
    time: Res<Time>,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    mut light_query: Query<&mut PointLight>,
    mut query: Query<(
        Entity,
        &SimPosition,
        &Mass,
        &Radius,
        &Temperature,
        &Composition,
        &CelestialBody,
        &mut Transform,
        &MeshMaterial3d<PlanetMaterial>,
        &mut Mesh3d,
        (
            Option<&PlanetaryClimate>,
            Option<&BiosphereState>,
            Option<&VolatileInventory>,
            Option<&PlanetaryRingSystem>,
            Option<&TidalState>,
            Option<&GeologicalState>,
        ),
        (
            Option<&SpinState>,
            Option<&ElectromagneticFieldState>,
            Option<&BlackHoleStarState>,
            Option<&Children>,
            Option<&PlanetaryBasins>,
            Option<&SatelliteOf>,
        ),
    )>,
) {
    let star_pos = query
        .iter()
        .find(|(_, _, _, _, _, _, b, _, _, _, _, _)| b.body_type.is_star_or_remnant())
        .map_or(Vec3::ZERO, |(_, p, _, _, _, _, _, _, _, _, _, _)| {
            Vec3::new(p.x as f32, p.y as f32, p.z as f32)
        });

    let mut all_moons = Vec::with_capacity(8);
    for (m_ent, pos, _, radius, _, _, body, _, _, _, _, (_, _, _, _, _, opt_sat)) in query.iter() {
        if opt_sat.is_some() || body.body_type == BodyType::Moon {
            let m_vis_rad = config.calc_visual_radius_for_type(radius.0, body.body_type);
            let m_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
            all_moons.push((m_ent, m_pos, m_vis_rad, opt_sat.map(|s| s.parent)));
        }
    }

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
            time.elapsed_secs(),
        );

        let visual_radius = config.calc_visual_radius_for_type(radius.0, body.body_type);
        transform.scale = Vec3::splat(visual_radius);

        if let Some(spin) = opt_spin {
            if spin.spin_vector.length_squared() > 1e-12 {
                let spin_dir = spin.spin_vector.normalize().as_vec3();
                transform.rotation = Quat::from_rotation_arc(Vec3::Y, spin_dir);
            } else {
                let tilt_rad = (spin.axial_tilt_degrees as f32).to_radians();
                transform.rotation = Quat::from_rotation_z(tilt_rad);
            }
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
                time.elapsed_secs(),
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
