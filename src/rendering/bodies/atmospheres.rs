//! Volumetric atmospheric limb glow shells, spectral profiles, and transform synchronization.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;

use super::{VisualAssets, VisualBody};

/// Marker component for an instantiated visual planetary atmosphere glow shell entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualAtmosphereChild;

/// Spectral scattering properties for an atmospheric body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereSpectralProfile {
    /// Rayleigh scattering cross-sections beta_R (RGB)
    pub rayleigh_beta: Vec3,
    /// Rayleigh scale height H_R (normalized relative to planet radius)
    pub rayleigh_scale_height: f32,
    /// Mie aerosol scattering cross-sections beta_M (RGB)
    pub mie_beta: Vec3,
    /// Mie scale height H_M
    pub mie_scale_height: f32,
    /// Mie forward-scattering asymmetry factor g (0.76 - 0.85)
    pub mie_asymmetry_g: f32,
    /// Outer shell radius scale factor (e.g. 1.045 = 104.5% of planet visual radius)
    pub shell_outer_scale: f32,
}

impl Default for AtmosphereSpectralProfile {
    fn default() -> Self {
        Self {
            rayleigh_beta: Vec3::new(0.28, 0.65, 1.0) * 0.75,
            rayleigh_scale_height: 0.08,
            mie_beta: Vec3::new(0.85, 0.92, 1.0) * 0.30,
            mie_scale_height: 0.03,
            mie_asymmetry_g: 0.82,
            shell_outer_scale: 1.045,
        }
    }
}

/// Derives the atmospheric spectral profile and scattering parameters based on planet archetype,
/// atmospheric composition, surface pressure, and temperature.
pub fn compute_atmosphere_spectral_profile(
    body_type: BodyType,
    comp: &Composition,
    temp_k: f64,
    pressure_bar: f32,
    cloud_coverage: f32,
) -> AtmosphereSpectralProfile {
    let norm = comp.normalized();
    let temp_f = temp_k as f32;
    let base_scale_height = (0.08f32 * (temp_f / 288.0).sqrt()).clamp(0.02, 0.22);

    match body_type {
        BodyType::GasGiant => {
            let jup_scale = 1.045 + (base_scale_height * 0.35).min(0.04);
            AtmosphereSpectralProfile {
                rayleigh_beta: Vec3::new(0.65, 0.75, 0.92) * 0.65,
                rayleigh_scale_height: base_scale_height,
                mie_beta: Vec3::new(0.95, 0.90, 0.80) * 0.45,
                mie_scale_height: base_scale_height * 0.40,
                mie_asymmetry_g: 0.83,
                shell_outer_scale: jup_scale,
            }
        }
        BodyType::IceGiant => {
            // Methane absorption in red/orange imparts deep aquamarine-azure scattering
            let nep_scale = 1.050 + (base_scale_height * 0.30).min(0.04);
            AtmosphereSpectralProfile {
                rayleigh_beta: Vec3::new(0.15, 0.75, 1.0) * 0.85,
                rayleigh_scale_height: base_scale_height,
                mie_beta: Vec3::new(0.45, 0.85, 1.0) * 0.35,
                mie_scale_height: base_scale_height * 0.35,
                mie_asymmetry_g: 0.84,
                shell_outer_scale: nep_scale,
            }
        }
        BodyType::SuperEarth | BodyType::TerrestrialPlanet | BodyType::Protoplanet => {
            if temp_k >= 380.0 || pressure_bar > 10.0 {
                // Venusian dense runaway greenhouse: sulfuric acid and dense CO2 shroud
                let v_scale = 1.055 + (pressure_bar * 0.0005).min(0.03);
                AtmosphereSpectralProfile {
                    rayleigh_beta: Vec3::new(0.95, 0.88, 0.65) * 1.35,
                    rayleigh_scale_height: base_scale_height * 1.2,
                    mie_beta: Vec3::new(0.98, 0.95, 0.80) * 0.85,
                    mie_scale_height: base_scale_height * 0.50,
                    mie_asymmetry_g: 0.80,
                    shell_outer_scale: v_scale,
                }
            } else if norm.organics_frac > 0.15 || (norm.ice_frac > 0.35 && temp_k < 180.0) {
                // Titan-like photochemical tholin smog: dense amber-orange limb
                let t_scale = 1.065 + (base_scale_height * 0.25).min(0.04);
                AtmosphereSpectralProfile {
                    rayleigh_beta: Vec3::new(1.0, 0.68, 0.22) * 0.95,
                    rayleigh_scale_height: base_scale_height * 1.4,
                    mie_beta: Vec3::new(1.0, 0.75, 0.35) * 0.55,
                    mie_scale_height: base_scale_height * 0.60,
                    mie_asymmetry_g: 0.85,
                    shell_outer_scale: t_scale,
                }
            } else if pressure_bar < 0.05 {
                // Martian thin CO2 + suspended ferric dust: butterscotch sky, blue sunset forward scattering
                AtmosphereSpectralProfile {
                    rayleigh_beta: Vec3::new(0.85, 0.65, 0.45) * 0.35,
                    rayleigh_scale_height: base_scale_height * 0.8,
                    mie_beta: Vec3::new(0.45, 0.65, 0.95) * 0.20,
                    mie_scale_height: base_scale_height * 0.30,
                    mie_asymmetry_g: 0.76,
                    shell_outer_scale: 1.030,
                }
            } else {
                // Earth-like / N2-O2 / Habitable water world
                let clouds = cloud_coverage.clamp(0.0, 1.0);
                let e_scale = 1.040 + (base_scale_height * 0.20).min(0.03);
                AtmosphereSpectralProfile {
                    rayleigh_beta: Vec3::new(0.28, 0.65, 1.0) * (0.70 + clouds * 0.15),
                    rayleigh_scale_height: base_scale_height,
                    mie_beta: Vec3::new(0.85, 0.92, 1.0) * (0.25 + clouds * 0.35),
                    mie_scale_height: base_scale_height * 0.35,
                    mie_asymmetry_g: 0.82,
                    shell_outer_scale: e_scale,
                }
            }
        }
        _ => AtmosphereSpectralProfile::default(),
    }
}

fn create_atmosphere_uniforms(
    profile: &AtmosphereSpectralProfile,
    pressure_bar: f32,
    visual_r: f32,
    atmo_r: f32,
    star_vector: Vec3,
    star_lum: f32,
    planet_world_pos: Vec3,
) -> AtmosphereUniforms {
    AtmosphereUniforms {
        rayleigh_params: Vec4::new(
            profile.rayleigh_beta.x,
            profile.rayleigh_beta.y,
            profile.rayleigh_beta.z,
            profile.rayleigh_scale_height,
        ),
        mie_params: Vec4::new(
            profile.mie_beta.x,
            profile.mie_beta.y,
            profile.mie_beta.z,
            profile.mie_scale_height,
        ),
        optical_params: Vec4::new(profile.mie_asymmetry_g, pressure_bar, visual_r, atmo_r),
        star_dir_and_intensity: Vec4::new(star_vector.x, star_vector.y, star_vector.z, star_lum),
        planet_center: Vec4::new(
            planet_world_pos.x,
            planet_world_pos.y,
            planet_world_pos.z,
            profile.shell_outer_scale,
        ),
    }
}

fn spawn_atmosphere_child(
    commands: &mut Commands,
    planet_ent: Entity,
    mesh: Handle<Mesh>,
    material: Handle<AtmosphereMaterial>,
    scale: f32,
) {
    if let Ok(mut p_cmd) = commands.get_entity(planet_ent) {
        p_cmd.with_children(|parent| {
            parent.spawn((
                VisualAtmosphereChild,
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_scale(Vec3::splat(scale)),
                Visibility::default(),
                NotShadowCaster,
            ));
        });
    }
}

/// Synchronizes 3D planetary atmospheric limb glow shells, materials, and starlight vectors.
#[allow(clippy::type_complexity, reason = "Atmosphere Mesh Query Complexity")]
pub fn sync_planetary_atmospheres(
    mut commands: Commands,
    visual_assets: Option<Res<VisualAssets>>,
    mut atmo_materials: ResMut<Assets<AtmosphereMaterial>>,
    primary_star_query: Query<
        (&SimPosition, &Temperature, &Mass),
        (With<CentralStar>, With<CelestialBody>),
    >,
    planets_query: Query<
        (
            Entity,
            &SimPosition,
            &Radius,
            &CelestialBody,
            &Composition,
            &Temperature,
            Option<&PlanetaryClimate>,
            Option<&VolatileInventory>,
            Option<&Children>,
        ),
        With<VisualBody>,
    >,
    mut atmo_children_query: Query<
        (&mut Transform, &MeshMaterial3d<AtmosphereMaterial>),
        With<VisualAtmosphereChild>,
    >,
) {
    let Some(visual_assets) = visual_assets else {
        return;
    };

    let star_data = primary_star_query.iter().next();
    let star_pos = star_data.map_or(Vec3::ZERO, |(p, _, _)| {
        Vec3::new(p.x as f32, p.y as f32, p.z as f32)
    });
    let star_lum = star_data.map_or(1.0, |(_, t, m)| {
        let t_ratio = (t.0 / 5778.0) as f32;
        let m_ratio = m.0 as f32;
        (t_ratio * t_ratio * t_ratio * t_ratio * m_ratio).clamp(0.2, 50.0)
    });

    for (planet_ent, p_pos, p_rad, body, comp, temp, opt_climate, opt_vol, opt_children) in
        planets_query.iter()
    {
        // Stars and non-planetary remnants/minor bodies do not possess bound limb atmosphere shells
        if body.body_type.is_star_or_remnant()
            || matches!(
                body.body_type,
                BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain | BodyType::Planetesimal
            )
        {
            continue;
        }

        let norm = comp.normalized();
        let pressure_bar = opt_vol.map_or((norm.gas_frac as f32 * 2.0).max(0.0), |v| {
            v.atmospheric_pressure_bar
        });
        let cloud_coverage = opt_climate.map_or(norm.gas_frac as f32, |c| c.cloud_coverage_frac);

        let is_gas_giant = body.body_type == BodyType::GasGiant || norm.gas_frac > 0.30;
        let is_ice_giant = body.body_type == BodyType::IceGiant;
        let has_atmosphere =
            is_gas_giant || is_ice_giant || pressure_bar >= 0.005 || norm.gas_frac > 0.02;

        let planet_world_pos = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
        let star_vector = (star_pos - planet_world_pos).normalize_or_zero();

        let profile = compute_atmosphere_spectral_profile(
            body.body_type,
            comp,
            temp.0,
            pressure_bar,
            cloud_coverage,
        );

        let visual_r = p_rad.0 as f32;
        let atmo_r = visual_r * profile.shell_outer_scale;

        let mut found_child = false;
        if let Some(children) = opt_children {
            for child in children.iter() {
                if let Ok((mut transform, mat_handle)) = atmo_children_query.get_mut(child) {
                    found_child = true;
                    if has_atmosphere {
                        transform.scale = Vec3::splat(profile.shell_outer_scale);
                        if let Some(mut mat) = atmo_materials.get_mut(&mat_handle.0) {
                            mat.uniforms = create_atmosphere_uniforms(
                                &profile,
                                pressure_bar,
                                visual_r,
                                atmo_r,
                                star_vector,
                                star_lum,
                                planet_world_pos,
                            );
                        }
                    } else {
                        // Atmosphere stripped or absent -> collapse shell
                        transform.scale = Vec3::ZERO;
                    }
                }
            }
        }

        if !found_child && has_atmosphere {
            let uniforms = create_atmosphere_uniforms(
                &profile,
                pressure_bar,
                visual_r,
                atmo_r,
                star_vector,
                star_lum,
                planet_world_pos,
            );
            let material = atmo_materials.add(AtmosphereMaterial { uniforms });
            spawn_atmosphere_child(
                &mut commands,
                planet_ent,
                visual_assets.atmosphere_mesh.clone(),
                material,
                profile.shell_outer_scale,
            );
        }
    }
}
