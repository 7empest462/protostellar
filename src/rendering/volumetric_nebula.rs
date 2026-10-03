//! 3D Volumetric Raymarching System for Giant Molecular Clouds (GMC) & Ionization Cavities.
//!
//! Spawns and manages the 1100 AU bounding cuboid volume and feeds real-time
//! protostellar radiation pressure cavity radiuses and stellar emission data to the shader.

use bevy::prelude::*;

use crate::rendering::materials::VolumetricNebulaMaterial;
use crate::simulation::components::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

/// Marker component for the 3D volumetric nebula bounding box entity.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct VolumetricNebulaVolume;

/// Calculates the radiation pressure / ionization cavity radius around a protostar in AU.
pub fn calculate_ionization_cavity_radius_au(
    luminosity_l_sun: f32,
    ambient_density_solar_au3: f32,
    is_ignited: bool,
) -> f32 {
    if !is_ignited || luminosity_l_sun <= 0.0 {
        return 0.0;
    }
    let rho_ref = 5.0e-11f32;
    let base_radius_au = 25.0f32;
    let density_factor = (rho_ref / ambient_density_solar_au3.max(1e-15)).cbrt();
    let lum_factor = luminosity_l_sun.cbrt();
    base_radius_au * lum_factor * density_factor
}

/// Spawns, updates, and tears down the 3D raymarching nebula bounding volume.
pub fn sync_volumetric_nebula(
    mut commands: Commands,
    scenario_state: Option<Res<ActiveScenarioState>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VolumetricNebulaMaterial>>,
    nebula_query: Query<
        (Entity, &MeshMaterial3d<VolumetricNebulaMaterial>),
        With<VolumetricNebulaVolume>,
    >,
    star_query: Query<
        (
            &SimPosition,
            &Luminosity,
            &Temperature,
            Option<&IgnitionState>,
            &CelestialBody,
        ),
        With<CelestialBody>,
    >,
) {
    let is_gmc_active = scenario_state
        .as_ref()
        .is_some_and(|s| s.current_preset == ScenarioPreset::MolecularCloudCluster);

    if !is_gmc_active {
        // Tear down nebula volume if present
        for (entity, _) in nebula_query.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Ensure volume exists
    let material_handle = if let Some((_, mat_handle)) = nebula_query.iter().next() {
        mat_handle.0.clone()
    } else {
        let mat = materials.add(VolumetricNebulaMaterial::default());
        let mesh = meshes.add(Cuboid::new(1100.0, 1100.0, 1100.0));

        commands.spawn((
            VolumetricNebulaVolume,
            Mesh3d(mesh),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(Vec3::ZERO),
            bevy::light::NotShadowCaster,
            bevy::light::NotShadowReceiver,
        ));

        mat
    };

    // Update material uniforms with current protostar cavity positions and luminous emissions
    if let Some(mut mat) = materials.get_mut(&material_handle) {
        let mut star_positions = [Vec4::ZERO; 16];
        let mut star_colors = [Vec4::ZERO; 16];
        let mut count = 0u32;

        for (pos, lum, temp, opt_ign, body) in star_query.iter() {
            if count >= 16 {
                break;
            }
            if body.body_type.is_star_or_remnant() {
                let is_ignited = opt_ign.is_some_and(|ign| ign.is_ignited);
                let lum_val = lum.0 as f32;
                let cavity_r = calculate_ionization_cavity_radius_au(lum_val, 5.0e-11, is_ignited);

                let t_k = temp.0 as f32;
                let r = (t_k / 4000.0).clamp(0.4, 1.0);
                let g = (t_k / 5500.0).clamp(0.3, 1.0);
                let b = (t_k / 7500.0).clamp(0.2, 1.2);

                if let (Some(pos_slot), Some(color_slot)) = (
                    star_positions.get_mut(count as usize),
                    star_colors.get_mut(count as usize),
                ) {
                    *pos_slot = Vec4::new(pos.x as f32, pos.y as f32, pos.z as f32, cavity_r);
                    *color_slot = Vec4::new(r, g, b, lum_val.max(0.1));
                    count += 1;
                }
            }
        }

        mat.uniforms.num_stars = count;
        mat.uniforms.star_positions_and_cavities = star_positions;
        mat.uniforms.star_colors_and_lum = star_colors;
    }
}
