use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy_shader::ShaderRef;

#[derive(Clone, ShaderType, Debug)]
pub struct PlanetUniforms {
    pub planet_type: u32,
    pub temperature: f32,
    pub time: f32,
    pub spin_rate: f32,
    /// x: rock (silicate + organics), y: ice (volatiles/water), z: metal, w: gas (atmosphere)
    pub composition: Vec4,
    pub color_seed: Vec4,
    /// x: ocean_frac, y: ice_frac, z: biomass_frac, w: cloud_density
    pub climate_and_bio: Vec4,
    /// x: surface_pressure_bar, y: scale_height, z: haze_density, w: greenhouse_factor
    pub atmosphere_params: Vec4,
    /// x: magnetic_field_gauss, y: lava_fraction, z: storm_intensity, w: axial_tilt_rad
    pub dynamics_and_mag: Vec4,
    /// x, y, z: unit 3D spin axis in world coordinates, w: reserved
    pub spin_axis: Vec4,
    /// Recent impact basins: xyz = local unit normal, w = angular radius (rad)
    pub impact_basins_pos: [Vec4; 4],
    /// Basin dynamics: x = melt_glow_fraction, y = elongation, z = rim_height, w = active flag
    pub impact_basins_data: [Vec4; 4],
    /// x, y, z: unit direction to central star in world coordinates, w: star luminosity factor
    pub star_dir_and_lum: Vec4,
    /// x, y, z: Rayleigh scattering coefficients beta_R (RGB), w: Mie forward scattering factor
    pub scattering_params: Vec4,
    /// x: inner_radius_ratio, y: outer_radius_ratio, z: optical_depth, w: has_rings (1.0 or 0.0)
    pub ring_shadow_params: Vec4,
    /// xyz: relative moon pos in planet radii, w: moon radius in planet radii
    pub eclipse_moons_pos: [Vec4; 2],
    /// x: active flag (1.0 or 0.0), y: penumbra softness, z: shadow depth, w: reserved
    pub eclipse_moons_data: [Vec4; 2],
    /// x: geological_age_gyr (0.0=Hadean, 4.56=Modern), y: continental_drift_phase, z: ocean_oxidation_progress (0.0=Archean iron-green, 1.0=blue), w: terrestrial_vegetation_fraction (0.0=craton rock, 1.0=lush flora)
    pub geological_params: Vec4,
    /// x: oval_colatitude_rad, y: oval_width_rad, z: auroral_intensity, w: geomagnetic_kp_index
    pub aurora_params: Vec4,
    /// x: hexagon_amplitude, y: hexagon_wavenumber (e.g. 6.0), z: great_spot_size, w: great_spot_lat_rad
    pub storm_features: Vec4,
    /// x: great_spot_lon_rad, y: vortex_spin_rate, z: secondary_oval_count, w: zonal_shear_turbulence
    pub storm_dynamics: Vec4,
}

impl Default for PlanetUniforms {
    fn default() -> Self {
        Self {
            planet_type: 0,
            temperature: 300.0,
            time: 0.0,
            spin_rate: 0.15,
            composition: Vec4::ZERO,
            color_seed: Vec4::ONE,
            climate_and_bio: Vec4::ZERO,
            atmosphere_params: Vec4::ZERO,
            dynamics_and_mag: Vec4::ZERO,
            spin_axis: Vec4::new(0.0, 1.0, 0.0, 0.0),
            impact_basins_pos: [Vec4::ZERO; 4],
            impact_basins_data: [Vec4::ZERO; 4],
            star_dir_and_lum: Vec4::new(0.0, 1.0, 0.0, 1.0),
            scattering_params: Vec4::new(0.28, 0.65, 1.0, 0.82),
            ring_shadow_params: Vec4::ZERO,
            eclipse_moons_pos: [Vec4::ZERO; 2],
            eclipse_moons_data: [Vec4::ZERO; 2],
            geological_params: Vec4::new(4.56, 0.0, 1.0, 1.0),
            aurora_params: Vec4::ZERO,
            storm_features: Vec4::ZERO,
            storm_dynamics: Vec4::new(0.0, 1.0, 0.0, 0.5),
        }
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct PlanetMaterialExtension {
    #[uniform(101)]
    pub uniforms: PlanetUniforms,
}

impl MaterialExtension for PlanetMaterialExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/planet.wgsl".into()
    }
}

pub type PlanetMaterial = ExtendedMaterial<StandardMaterial, PlanetMaterialExtension>;

#[derive(Clone, ShaderType, Debug)]
pub struct AtmosphereUniforms {
    /// x, y, z: Rayleigh scattering coefficients beta_R (RGB), w: scale height H_R
    pub rayleigh_params: Vec4,
    /// x, y, z: Mie aerosol scattering coefficients beta_M (RGB), w: scale height H_M
    pub mie_params: Vec4,
    /// x: Mie asymmetry parameter g (0.76 - 0.85), y: surface pressure bar, z: inner planet radius, w: outer atmosphere radius
    pub optical_params: Vec4,
    /// x, y, z: unit direction to central star in world coordinates, w: star intensity factor
    pub star_dir_and_intensity: Vec4,
    /// x, y, z: planet world position, w: visual scale factor
    pub planet_center: Vec4,
    /// x: oval_colatitude_rad, y: oval_width_rad, z: auroral_intensity, w: geomagnetic_kp_index
    pub aurora_params: Vec4,
}

impl Default for AtmosphereUniforms {
    fn default() -> Self {
        Self {
            rayleigh_params: Vec4::new(0.28, 0.65, 1.0, 0.08),
            mie_params: Vec4::new(0.85, 0.92, 1.0, 0.03),
            optical_params: Vec4::new(0.82, 1.0, 1.0, 1.05),
            star_dir_and_intensity: Vec4::new(0.0, 1.0, 0.0, 1.0),
            planet_center: Vec4::ZERO,
            aurora_params: Vec4::ZERO,
        }
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct AtmosphereMaterial {
    #[uniform(0)]
    pub uniforms: AtmosphereUniforms,
}

impl Material for AtmosphereMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/atmosphere.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Clone, ShaderType, Debug)]
pub struct RingUniforms {
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub optical_depth: f32,
    pub ice_fraction: f32,
    pub ring_color: Vec4,
    /// xyz: unit vector towards central star in ring local space, w: planet_radius_ratio (1.0 / ring_ratio)
    pub star_dir_local: Vec4,
    /// x: ambient nightside floor (e.g. 0.03), y: penumbra softness width (e.g. 0.025), z: shadow depth, w: reserved
    pub shadow_params: Vec4,
}

impl Default for RingUniforms {
    fn default() -> Self {
        Self {
            inner_radius: 0.0008,
            outer_radius: 0.0028,
            optical_depth: 0.85,
            ice_fraction: 0.95,
            ring_color: Vec4::ONE,
            star_dir_local: Vec4::new(0.0, 1.0, 0.0, 0.35),
            shadow_params: Vec4::new(0.03, 0.025, 0.97, 0.0),
        }
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct RingMaterial {
    #[uniform(0)]
    pub uniforms: RingUniforms,
}

impl Material for RingMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/planetary_rings.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None; // Double-sided rendering
        Ok(())
    }
}

#[derive(Clone, Default, ShaderType, Debug)]
pub struct SkyboxUniforms {
    /// x: time (seconds), y: scenario_blend (0.0 = Milky Way, 1.0 = Early Universe), z: exposure, w: star_twinkle
    pub params: Vec4,
    /// x: star_density, y: nebula_intensity, z: cosmic_web_scale, w: filament_brightness
    pub tuning: Vec4,
    /// x, y, z: black hole position relative to camera in AU, w: angular Einstein radius theta_E in radians
    pub lens_pos_and_mass: Vec4,
    /// x: angular shadow radius theta_s (radians), y: photon ring width (radians), z: is_active (1.0 or 0.0), w: relativistic boost factor
    pub lens_params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct SkyboxMaterial {
    #[uniform(0)]
    pub uniforms: SkyboxUniforms,
}

impl Default for SkyboxMaterial {
    fn default() -> Self {
        Self {
            uniforms: SkyboxUniforms {
                params: Vec4::new(0.0, 0.0, 1.25, 1.0),
                tuning: Vec4::new(1.0, 1.0, 1.0, 1.0),
                lens_pos_and_mass: Vec4::ZERO,
                lens_params: Vec4::ZERO,
            },
        }
    }
}

impl Material for SkyboxMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/skybox.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Double-sided / inside rendering on celestial sphere
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// GPU uniform parameters for relativistic polar jets, synchrotron light cones, and Doppler beaming.
#[derive(Clone, ShaderType, Debug)]
pub struct RelativisticJetUniforms {
    /// x: elapsed time (s), y: lorentz factor gamma (>= 1.0), z: opening angle (rad), w: jet length (AU)
    pub jet_params: Vec4,
    /// xyz: unit jet pointing direction vector in world space, w: precession cone angle (rad)
    pub jet_dir_and_precession: Vec4,
    /// x: electron power-law index p (e.g. 2.3), y: knot speed (v/c, e.g. 0.95), z: knot frequency, w: helical pitch
    pub synchrotron_params: Vec4,
    /// Base core emission RGBA (incandescent core)
    pub core_color: Vec4,
    /// Outer lobe / sheath emission RGBA (synchrotron cocoon)
    pub lobe_color: Vec4,
    /// xyz: jet origin in world space, w: doppler boost toggle (1.0 = on, 0.0 = off)
    pub jet_origin_and_doppler: Vec4,
}

impl Default for RelativisticJetUniforms {
    fn default() -> Self {
        Self {
            jet_params: Vec4::new(0.0, 8.5, 0.075, 3.2),
            jet_dir_and_precession: Vec4::new(0.0, 1.0, 0.0, 0.12),
            synchrotron_params: Vec4::new(2.35, 0.94, 3.5, 5.0),
            core_color: Vec4::new(0.85, 0.95, 1.0, 1.0),
            lobe_color: Vec4::new(0.30, 0.65, 1.0, 0.85),
            jet_origin_and_doppler: Vec4::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

/// Custom Bevy material for volumetric relativistic polar jets and synchrotron emission cones.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct RelativisticJetMaterial {
    #[uniform(0)]
    pub uniforms: RelativisticJetUniforms,
}

impl Material for RelativisticJetMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/relativistic_jet.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// GPU uniform parameters for cometary coma fluorescence, solar wind ion streamers, and curved dust fan.
#[derive(Clone, ShaderType, Debug)]
pub struct CometTailUniforms {
    /// x: elapsed time (s), y: tail length (AU), z: coma radius (AU), w: insolation flux factor
    pub params: Vec4,
    /// xyz: unit anti-solar direction in world space, w: in-plane lag angle (rad)
    pub anti_solar_and_lag: Vec4,
    /// xyz: comet nucleus world position, w: tail mode (0.0 = volumetric tail, 1.0 = coma)
    pub nucleus_pos_and_type: Vec4,
    /// xyz: unit orbital velocity in-plane lag vector, w: volatile activity scale
    pub velocity_and_activity: Vec4,
    /// Type I Ion tail color RGBA (electric cyan/blue)
    pub ion_color: Vec4,
    /// Type II Dust tail color RGBA (golden-amber)
    pub dust_color: Vec4,
}

impl Default for CometTailUniforms {
    fn default() -> Self {
        Self {
            params: Vec4::new(0.0, 1.5, 0.04, 1.0),
            anti_solar_and_lag: Vec4::new(0.0, 1.0, 0.0, 0.15),
            nucleus_pos_and_type: Vec4::ZERO,
            velocity_and_activity: Vec4::new(1.0, 0.0, 0.0, 1.0),
            ion_color: Vec4::new(0.20, 0.88, 1.0, 0.95),
            dust_color: Vec4::new(1.0, 0.88, 0.65, 0.75),
        }
    }
}

/// Custom Bevy material for volumetric cometary tails, ion plasma ribbons, and diffuse comas.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct CometTailMaterial {
    #[uniform(0)]
    pub uniforms: CometTailUniforms,
}

impl Material for CometTailMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/comet_tail.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}
