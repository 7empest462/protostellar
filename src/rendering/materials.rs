use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy_shader::ShaderRef;

#[derive(Clone, Default, ShaderType, Debug)]
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

#[derive(Clone, Default, ShaderType, Debug)]
pub struct RingUniforms {
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub optical_depth: f32,
    pub ice_fraction: f32,
    pub ring_color: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct RingMaterial {
    #[uniform(0)]
    pub uniforms: RingUniforms,
}

impl Default for RingMaterial {
    fn default() -> Self {
        Self {
            uniforms: RingUniforms {
                inner_radius: 0.0008,
                outer_radius: 0.0028,
                optical_depth: 0.85,
                ice_fraction: 0.95,
                ring_color: Vec4::ONE,
            },
        }
    }
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
