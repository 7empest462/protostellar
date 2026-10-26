//! Volumetric atmospheric limb glow shells, stellar coronae, spectral profiles, and transform synchronization.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::rendering::materials::*;
use crate::simulation::components::*;

use super::{VisualAssets, VisualBody};

/// Marker component for an instantiated visual planetary atmosphere or stellar corona glow shell entity.
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

fn stellar_corona_palette(body_type: BodyType, temp_k: f64) -> (Vec3, Vec3, f32, f32) {
    match body_type {
        BodyType::Protostar | BodyType::TTauriStar => (
            Vec3::new(1.15, 0.62, 0.22),
            Vec3::new(1.35, 0.38, 0.12),
            2.45,
            1.45,
        ),
        BodyType::HerbigAeBeStar => (
            Vec3::new(0.82, 1.02, 1.42),
            Vec3::new(1.15, 0.58, 0.32),
            2.55,
            1.60,
        ),
        BodyType::RedGiant | BodyType::RedSupergiant => (
            Vec3::new(1.20, 0.45, 0.16),
            Vec3::new(1.40, 0.30, 0.10),
            2.55,
            1.65,
        ),
        BodyType::CarbonStar => (
            Vec3::new(1.15, 0.18, 0.06),
            Vec3::new(1.35, 0.10, 0.03),
            2.60,
            1.50,
        ),
        BodyType::ThorneZytkow => (
            Vec3::new(1.22, 0.38, 0.48),
            Vec3::new(0.65, 1.15, 1.65),
            2.60,
            1.75,
        ),
        BodyType::YellowHypergiant => (
            Vec3::new(1.30, 0.96, 0.42),
            Vec3::new(1.45, 0.68, 0.20),
            2.65,
            1.80,
        ),
        BodyType::WolfRayet => (
            Vec3::new(1.25, 0.55, 1.35),
            Vec3::new(0.55, 1.10, 1.55),
            2.65,
            1.85,
        ),
        BodyType::LuminousBlueVariable => (
            Vec3::new(0.68, 1.05, 1.55),
            Vec3::new(0.85, 1.25, 1.75),
            2.70,
            1.90,
        ),
        BodyType::PopulationIIIStar => (
            Vec3::new(0.75, 0.92, 1.68),
            Vec3::new(0.95, 0.80, 1.85),
            2.75,
            2.00,
        ),
        BodyType::BlueStraggler => (
            Vec3::new(0.70, 0.95, 1.48),
            Vec3::new(0.55, 0.88, 1.55),
            2.55,
            1.70,
        ),
        BodyType::WhiteDwarf | BodyType::NeutronStar | BodyType::Pulsar => (
            Vec3::new(0.70, 0.88, 1.35),
            Vec3::new(0.45, 0.95, 1.45),
            2.35,
            1.55,
        ),
        BodyType::QuarkStar => (
            Vec3::new(1.05, 0.82, 1.60),
            Vec3::new(0.65, 1.35, 1.80),
            2.40,
            1.75,
        ),
        BodyType::Magnetar => (
            Vec3::new(0.95, 0.55, 1.45),
            Vec3::new(0.35, 1.25, 1.50),
            2.50,
            1.80,
        ),
        BodyType::DarkStar => (
            Vec3::new(1.25, 0.72, 0.24),
            Vec3::new(1.10, 0.52, 0.95),
            2.65,
            1.55,
        ),
        BodyType::BosonStar => (
            Vec3::new(0.38, 1.15, 1.42),
            Vec3::new(0.55, 0.85, 1.55),
            2.25,
            0.95,
        ),
        BodyType::BrownDwarf => (
            Vec3::new(0.72, 0.22, 0.44),
            Vec3::new(0.95, 0.35, 0.18),
            2.10,
            0.85,
        ),
        BodyType::OrangeDwarf => (
            Vec3::new(1.12, 0.68, 0.28),
            Vec3::new(1.32, 0.42, 0.14),
            2.38,
            1.32,
        ),
        BodyType::QuasiStar => (
            Vec3::new(1.35, 0.50, 0.18),
            Vec3::new(1.50, 0.35, 0.12),
            2.70,
            1.90,
        ),
        _ if temp_k > 10_000.0 => (
            Vec3::new(0.65, 0.82, 1.35),
            Vec3::new(0.50, 0.75, 1.45),
            2.65,
            1.75,
        ),
        _ if temp_k > 6_800.0 => (
            Vec3::new(0.88, 0.94, 1.18),
            Vec3::new(0.75, 0.88, 1.25),
            2.50,
            1.50,
        ),
        _ if temp_k >= 5_000.0 => (
            Vec3::new(1.08, 0.95, 0.76),
            Vec3::new(1.25, 0.55, 0.25),
            2.45,
            1.40,
        ),
        _ => (
            Vec3::new(1.15, 0.55, 0.22),
            Vec3::new(1.35, 0.32, 0.12),
            2.30,
            1.25,
        ),
    }
}

/// Computes 3D volumetric K/F-corona and optical diffraction spike uniforms for luminous stars.
pub fn compute_stellar_corona_uniforms(
    body_type: BodyType,
    temp_k: f64,
    visual_r: f32,
    world_pos: Vec3,
    spin_axis: Vec4,
    tidal_deformation: Vec4,
    flare_intensity: f32,
    elapsed_secs: f32,
) -> (AtmosphereUniforms, f32) {
    let (corona_rgb, chromo_rgb, shell_scale, intensity) =
        stellar_corona_palette(body_type, temp_k);

    let outer_r = visual_r * shell_scale;
    let uniforms = AtmosphereUniforms {
        rayleigh_params: Vec4::new(corona_rgb.x, corona_rgb.y, corona_rgb.z, 0.32),
        mie_params: Vec4::new(chromo_rgb.x, chromo_rgb.y, chromo_rgb.z, 6.0),
        // optical_params.y = -1.0 activates Stellar Corona & Optical Diffraction Spike Mode in atmosphere.wgsl
        optical_params: Vec4::new(0.85, -1.0, visual_r, outer_r),
        star_dir_and_intensity: Vec4::new(0.0, 1.0, 0.0, intensity),
        planet_center: Vec4::new(world_pos.x, world_pos.y, world_pos.z, shell_scale),
        aurora_params: Vec4::new(flare_intensity, 0.0, 1.0, elapsed_secs),
        spin_axis,
        ring_and_ozone_params: Vec4::ZERO,
        tidal_deformation,
    };
    (uniforms, shell_scale)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Atmosphere uniform constructor maps spectral and geometric fields to GPU struct"
)]
fn create_atmosphere_uniforms(
    profile: &AtmosphereSpectralProfile,
    pressure_bar: f32,
    visual_r: f32,
    atmo_r: f32,
    star_vector: Vec3,
    star_lum: f32,
    planet_world_pos: Vec3,
    aurora_params: Vec4,
    spin_axis: Vec4,
    ring_and_ozone_params: Vec4,
    tidal_deformation: Vec4,
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
        aurora_params,
        spin_axis,
        ring_and_ozone_params,
        tidal_deformation,
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

fn derive_auroral_params(
    opt_aurora: Option<&crate::simulation::space_weather::AuroralOvalState>,
    opt_diff: Option<&InternalDifferentiation>,
) -> Vec4 {
    opt_aurora.map_or_else(
        || {
            let b_gauss = opt_diff.map_or(0.0, |d| d.magnetic_field_gauss as f32);
            if b_gauss > 0.1 {
                Vec4::new(0.315, 0.065, (b_gauss * 0.8).clamp(0.2, 2.5), 1.5)
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

fn update_atmosphere_child_mesh(
    children: &Children,
    has_atmosphere: bool,
    shell_scale: f32,
    uniforms: &AtmosphereUniforms,
    atmo_children_query: &mut Query<
        (&mut Transform, &MeshMaterial3d<AtmosphereMaterial>),
        (With<VisualAtmosphereChild>, Without<VisualBody>),
    >,
    atmo_materials: &mut Assets<AtmosphereMaterial>,
) -> bool {
    let mut found_child = false;
    for child in children.iter() {
        if let Ok((mut transform, mat_handle)) = atmo_children_query.get_mut(child) {
            found_child = true;
            if has_atmosphere {
                transform.scale = Vec3::splat(shell_scale);
                if let Some(mut mat) = atmo_materials.get_mut(&mat_handle.0) {
                    mat.uniforms = uniforms.clone();
                }
            } else {
                transform.scale = Vec3::ZERO;
            }
        }
    }
    found_child
}

fn compute_spin_axis(opt_spin: Option<&SpinState>) -> Vec4 {
    opt_spin.map_or(Vec4::new(0.0, 1.0, 0.0, 0.0), |s| {
        if s.spin_vector.length_squared() > 1e-12
            && (s.spin_vector.x.abs() > 1e-6 || s.spin_vector.z.abs() > 1e-6)
        {
            let n = s.spin_vector.normalize();
            Vec4::new(n.x as f32, n.y as f32, n.z as f32, 0.0)
        } else {
            let tilt = (s.axial_tilt_degrees as f32).to_radians();
            Vec4::new(tilt.sin(), tilt.cos(), 0.0, 0.0)
        }
    })
}

fn despawn_atmosphere_children(
    commands: &mut Commands,
    children: &Children,
    atmo_children_query: &Query<
        (&mut Transform, &MeshMaterial3d<AtmosphereMaterial>),
        (With<VisualAtmosphereChild>, Without<VisualBody>),
    >,
) {
    for child in children.iter() {
        if atmo_children_query.get(child).is_ok() {
            if let Ok(mut c_cmd) = commands.get_entity(child) {
                c_cmd.try_despawn();
            }
        }
    }
}

fn extract_primary_star_lighting(
    primary_star_query: &Query<
        (&SimPosition, &Temperature, &Mass),
        (With<CentralStar>, With<CelestialBody>),
    >,
) -> (Vec3, f32) {
    let star_data = primary_star_query.iter().next();
    let star_pos = star_data.map_or(Vec3::ZERO, |(p, _, _)| {
        Vec3::new(p.x as f32, p.y as f32, p.z as f32)
    });
    let star_lum = star_data.map_or(1.0, |(_, t, m)| {
        let t_ratio = (t.0 / 5778.0) as f32;
        let m_ratio = m.0 as f32;
        (t_ratio * t_ratio * t_ratio * t_ratio * m_ratio).clamp(0.2, 50.0)
    });
    (star_pos, star_lum)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Atmosphere uniform calculation requires planetary state components and starlight parameters"
)]
fn build_planet_atmosphere_uniforms(
    p_pos: &SimPosition,
    visual_r: f32,
    body: &CelestialBody,
    comp: &Composition,
    temp: &Temperature,
    opt_climate: Option<&PlanetaryClimate>,
    opt_vol: Option<&VolatileInventory>,
    opt_aurora: Option<&crate::simulation::space_weather::AuroralOvalState>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_spin: Option<&SpinState>,
    opt_planet_mat: Option<&PlanetMaterial>,
    star_pos: Vec3,
    star_lum: f32,
) -> (AtmosphereUniforms, bool, f32) {
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

    let atmo_r = visual_r * profile.shell_outer_scale;
    let aurora_params = if is_gas_giant || is_ice_giant {
        Vec4::ZERO
    } else {
        derive_auroral_params(opt_aurora, opt_diff)
    };
    let mut spin_axis = compute_spin_axis(opt_spin);
    let mut tidal_deformation = Vec4::ZERO;
    let mut ring_and_ozone = Vec4::ZERO;

    let is_habitable_ozone = matches!(
        body.body_type,
        BodyType::TerrestrialPlanet | BodyType::SuperEarth
    ) && (0.05..=8.0).contains(&pressure_bar)
        && temp.0 < 365.0;
    let mut flags = if is_habitable_ozone { 1.0f32 } else { 0.0f32 };

    if let Some(p_mat) = opt_planet_mat {
        let u = &p_mat.extension.uniforms;
        spin_axis = u.spin_axis;
        let raw_w = u.tidal_deformation.w;
        let roche_stretch = if raw_w >= 1.5 { raw_w - 2.0 } else { raw_w };
        tidal_deformation = Vec4::new(
            u.tidal_deformation.x,
            u.tidal_deformation.y,
            u.tidal_deformation.z,
            roche_stretch.clamp(0.0, 0.85),
        );
        if u.ring_shadow_params.w > 0.5 {
            flags += 2.0;
            ring_and_ozone = Vec4::new(
                u.ring_shadow_params.x,
                u.ring_shadow_params.y,
                u.ring_shadow_params.z,
                flags,
            );
        } else {
            ring_and_ozone.w = flags;
        }
    } else {
        ring_and_ozone.w = flags;
    }

    let oblateness = if is_gas_giant || is_ice_giant {
        0.065
    } else {
        0.0
    };
    spin_axis.w = oblateness;

    let max_stretch = 1.0 + tidal_deformation.w * 1.15 + spin_axis.w.clamp(0.0, 0.25) * 0.35;
    let effective_shell_scale = profile.shell_outer_scale * max_stretch;

    let uniforms = create_atmosphere_uniforms(
        &profile,
        pressure_bar,
        visual_r,
        atmo_r,
        star_vector,
        star_lum,
        planet_world_pos,
        aurora_params,
        spin_axis,
        ring_and_ozone,
        tidal_deformation,
    );

    (uniforms, has_atmosphere, effective_shell_scale)
}

fn build_stellar_corona_state(
    p_pos: &SimPosition,
    visual_r: f32,
    body_type: BodyType,
    temp_k: f64,
    opt_spin: Option<&SpinState>,
    opt_planet_mat: Option<&PlanetMaterial>,
    opt_flare: Option<&crate::simulation::space_weather::StellarFlareState>,
    elapsed_secs: f32,
) -> (AtmosphereUniforms, bool, f32) {
    let world_pos = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
    let spin_axis = opt_planet_mat.map_or_else(
        || compute_spin_axis(opt_spin),
        |m| m.extension.uniforms.spin_axis,
    );
    let tidal_def = opt_planet_mat.map_or(Vec4::ZERO, |m| {
        let u_td = m.extension.uniforms.tidal_deformation;
        let rw = if u_td.w >= 1.5 { u_td.w - 2.0 } else { u_td.w };
        Vec4::new(u_td.x, u_td.y, u_td.z, rw.clamp(0.0, 0.85))
    });
    let flare_int = opt_flare.map_or(0.25, |f| f.current_flare_intensity);
    let (corona_u, corona_scale) = compute_stellar_corona_uniforms(
        body_type,
        temp_k,
        visual_r,
        world_pos,
        spin_axis,
        tidal_def,
        flare_int,
        elapsed_secs,
    );
    (corona_u, true, corona_scale)
}

fn should_skip_atmosphere_shell(body_type: BodyType, opt_bhs: Option<&BlackHoleStarState>) -> bool {
    let is_blown_out_quasi =
        body_type == BodyType::QuasiStar && opt_bhs.is_some_and(|s| s.is_blown_out);
    body_type == BodyType::BlackHole
        || is_blown_out_quasi
        || matches!(
            body_type,
            BodyType::BlackDwarf
                | BodyType::IronStar
                | BodyType::Asteroid
                | BodyType::Comet
                | BodyType::DustGrain
                | BodyType::Planetesimal
        )
}

fn sync_single_atmosphere_entity(
    commands: &mut Commands,
    planet_ent: Entity,
    mesh: Handle<Mesh>,
    opt_children: Option<&Children>,
    has_atmosphere: bool,
    shell_outer_scale: f32,
    uniforms: &AtmosphereUniforms,
    atmo_children_query: &mut Query<
        (&mut Transform, &MeshMaterial3d<AtmosphereMaterial>),
        (With<VisualAtmosphereChild>, Without<VisualBody>),
    >,
    atmo_materials: &mut Assets<AtmosphereMaterial>,
) {
    let found_child = opt_children.is_some_and(|children| {
        update_atmosphere_child_mesh(
            children,
            has_atmosphere,
            shell_outer_scale,
            uniforms,
            atmo_children_query,
            atmo_materials,
        )
    });

    if !found_child && has_atmosphere {
        let material = atmo_materials.add(AtmosphereMaterial {
            uniforms: uniforms.clone(),
        });
        spawn_atmosphere_child(commands, planet_ent, mesh, material, shell_outer_scale);
    }
}

/// Synchronizes 3D planetary atmospheric limb glow shells, stellar coronae, materials, and starlight vectors.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Atmosphere and stellar corona mesh query synchronization"
)]
pub fn sync_planetary_atmospheres(
    mut commands: Commands,
    time: Res<Time>,
    visual_assets: Option<Res<VisualAssets>>,
    planet_materials: Option<Res<Assets<PlanetMaterial>>>,
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
            &Transform,
            &CelestialBody,
            &Composition,
            &Temperature,
            Option<&MeshMaterial3d<PlanetMaterial>>,
            Option<&PlanetaryClimate>,
            Option<&VolatileInventory>,
            Option<&Children>,
            (
                Option<&crate::simulation::space_weather::AuroralOvalState>,
                Option<&InternalDifferentiation>,
                Option<&SpinState>,
                Option<&crate::simulation::space_weather::StellarFlareState>,
                Option<&BlackHoleStarState>,
            ),
        ),
        With<VisualBody>,
    >,
    mut atmo_children_query: Query<
        (&mut Transform, &MeshMaterial3d<AtmosphereMaterial>),
        (With<VisualAtmosphereChild>, Without<VisualBody>),
    >,
) {
    let Some(visual_assets) = visual_assets else {
        return;
    };

    let (star_pos, star_lum) = extract_primary_star_lighting(&primary_star_query);
    let elapsed_secs = time.elapsed_secs();

    for (
        planet_ent,
        p_pos,
        p_rad,
        p_trans,
        body,
        comp,
        temp,
        opt_p_mat_handle,
        opt_climate,
        opt_vol,
        opt_children,
        (opt_aurora, opt_diff, opt_spin, opt_flare, opt_bhs),
    ) in planets_query.iter()
    {
        if should_skip_atmosphere_shell(body.body_type, opt_bhs) {
            if let Some(children) = opt_children {
                despawn_atmosphere_children(&mut commands, children, &atmo_children_query);
            }
            continue;
        }

        let visual_r = p_trans.scale.x.max(p_rad.0 as f32 * 0.1);
        let opt_planet_mat = opt_p_mat_handle.and_then(|h| {
            planet_materials
                .as_ref()
                .and_then(|assets| assets.get(&h.0))
        });

        let (mut uniforms, has_atmosphere, shell_outer_scale) =
            if body.body_type.is_star_or_remnant() || body.body_type == BodyType::QuasiStar {
                build_stellar_corona_state(
                    p_pos,
                    visual_r,
                    body.body_type,
                    temp.0,
                    opt_spin,
                    opt_planet_mat,
                    opt_flare,
                    elapsed_secs,
                )
            } else {
                build_planet_atmosphere_uniforms(
                    p_pos,
                    visual_r,
                    body,
                    comp,
                    temp,
                    opt_climate,
                    opt_vol,
                    opt_aurora,
                    opt_diff,
                    opt_spin,
                    opt_planet_mat,
                    star_pos,
                    star_lum,
                )
            };

        if opt_planet_mat
            .as_ref()
            .is_some_and(|m| m.extension.uniforms.cutaway_params.x > 0.5)
        {
            uniforms.ring_and_ozone_params.w += 4.0;
        }

        sync_single_atmosphere_entity(
            &mut commands,
            planet_ent,
            visual_assets.atmosphere_mesh.clone(),
            opt_children,
            has_atmosphere,
            shell_outer_scale,
            &uniforms,
            &mut atmo_children_query,
            &mut atmo_materials,
        );
    }
}
