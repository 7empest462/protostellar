use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::palettes::*;
use super::VisualAssets;

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

fn update_body_material_properties(
    mat: &mut PlanetMaterial,
    body: &CelestialBody,
    mass: &Mass,
    temp: &Temperature,
    comp: &Composition,
    elapsed_secs: f32,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_vol: Option<&VolatileInventory>,
    opt_spin: Option<&SpinState>,
    opt_em: Option<&ElectromagneticFieldState>,
    opt_bhs: Option<&BlackHoleStarState>,
) {
    let (br, bg, bb) = blackbody_to_srgb(temp.0);
    let (cr, cg, cb) = comp.visual_color_tint();

    let is_star_like = body.body_type.is_star_or_remnant();
    let is_gas_giant = body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.30;
    let color = if is_star_like {
        compute_stellar_palette(body.body_type, temp.0)
    } else if is_gas_giant {
        compute_gas_giant_palette(mass.0, temp.0, &body.name)
    } else {
        Color::srgb(
            (br * 0.25 + cr * 0.75).clamp(0.1, 1.0),
            (bg * 0.25 + cg * 0.75).clamp(0.1, 1.0),
            (bb * 0.25 + cb * 0.75).clamp(0.1, 1.0),
        )
    };
    let norm_comp = comp.normalized();
    let ocean_frac = opt_vol.map_or(norm_comp.ice_frac as f32, |v| v.ocean_coverage_frac);
    let ice_frac = opt_climate.map_or(0.0, |c| c.ice_coverage_frac);
    let biomass_frac = opt_bio.map_or(0.0, |b| b.biomass_coverage_frac);
    let cloud_density = opt_climate.map_or(norm_comp.gas_frac as f32, |c| c.cloud_coverage_frac);

    let spin_rate = opt_spin.map_or(0.15, |s| {
        (24.0 / s.rotation_period_hours.max(0.1)) as f32 * 0.15
    });
    let axial_tilt = opt_spin.map_or(0.08, |s| (s.axial_tilt_degrees as f32).to_radians());

    let pressure_bar = opt_vol.map_or((norm_comp.gas_frac as f32 * 2.0).max(0.01), |v| {
        v.atmospheric_pressure_bar
    });
    let scale_height = (0.08f32 * (temp.0 as f32 / 288.0f32).sqrt()).clamp(0.02f32, 0.25f32);
    let haze_density = opt_climate.map_or((norm_comp.gas_frac as f32 * 1.5).clamp(0.0, 1.0), |c| {
        (c.cloud_coverage_frac * 1.2).clamp(0.0, 1.0)
    });
    let greenhouse = opt_climate.map_or(33.0, |c| c.greenhouse_delta_k);

    let mag_gauss = opt_em.map_or(0.0, |e| e.magnetic_field_gauss as f32);
    let lava_frac = if temp.0 > 600.0 {
        ((temp.0 as f32 - 600.0) / 900.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mass_jup = (mass.0 / JUPITER_MASS_SOLAR) as f32;

    mat.base.base_color = color;
    mat.extension.uniforms.color_seed = LinearRgba::from(color).to_vec4();
    mat.extension.uniforms.temperature = temp.0 as f32;
    mat.extension.uniforms.time = elapsed_secs;
    mat.extension.uniforms.spin_rate = spin_rate;
    mat.extension.uniforms.climate_and_bio =
        Vec4::new(ocean_frac, ice_frac, biomass_frac, cloud_density);
    mat.extension.uniforms.atmosphere_params =
        Vec4::new(pressure_bar, scale_height, haze_density, greenhouse);
    mat.extension.uniforms.dynamics_and_mag = Vec4::new(mag_gauss, lava_frac, mass_jup, axial_tilt);

    if is_star_like {
        let is_blown_out = opt_bhs.is_some_and(|s| s.is_blown_out);
        update_star_material(
            mat,
            body.body_type,
            is_blown_out,
            color,
            mag_gauss,
            mass.0,
            axial_tilt,
            opt_bhs,
        );
    } else {
        update_planet_material(mat, body.body_type, comp, temp.0, color);
    }
}

#[allow(clippy::type_complexity, reason = "Celestial Transform Sync")]
pub fn sync_celestial_transforms(
    time: Res<Time>,
    config: Res<SimulationConfig>,
    visual_assets: Res<VisualAssets>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    mut light_query: Query<&mut PointLight>,
    mut query: Query<(
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
        ),
        (
            Option<&SpinState>,
            Option<&ElectromagneticFieldState>,
            Option<&BlackHoleStarState>,
            Option<&Children>,
        ),
    )>,
) {
    for (
        pos,
        mass,
        radius,
        temp,
        comp,
        body,
        mut transform,
        mat_handle,
        mut mesh,
        (opt_climate, opt_bio, opt_vol),
        (opt_spin, opt_em, opt_bhs, opt_children),
    ) in query.iter_mut()
    {
        transform.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
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
                opt_climate,
                opt_bio,
                opt_vol,
                opt_spin,
                opt_em,
                opt_bhs,
            );
        }
    }
}
