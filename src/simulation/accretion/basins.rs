//! Dynamic impact basins, craters, grazing trenches, and thermal melt relaxation.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::EARTH_MASS_SOLAR;

/// Registers an impact basin on an entity, maintaining at most 8 recent basins.
pub fn record_impact_crater_basin(
    commands: &mut Commands,
    primary_entity: Entity,
    surface_normal: Vec3,
    angular_radius: f32,
    sim_time_yr: f64,
    elongation: f32,
) {
    let basin = ImpactBasin {
        surface_normal: surface_normal.normalize_or_zero(),
        angular_radius: angular_radius.clamp(0.04, 0.65),
        formation_time_yr: sim_time_yr,
        melt_glow_fraction: 1.0,
        elongation: elongation.clamp(1.0, 3.5),
        scar_intensity: 1.0,
    };

    if let Ok(mut cmd) = commands.get_entity(primary_entity) {
        cmd.entry::<PlanetaryBasins>()
            .and_modify(move |mut pb| {
                if pb.basins.len() >= 8 {
                    pb.basins.remove(0);
                }
                pb.basins.push(basin);
            })
            .or_insert(PlanetaryBasins {
                basins: vec![basin],
            });
    }
}

/// Registers an elongated gouge trench on a body involved in a grazing encounter.
pub fn record_grazing_impact_scar(
    commands: &mut Commands,
    target_entity: Entity,
    contact_dir: DVec3,
    target_m: f64,
    impactor_m: f64,
    sim_time_years: f64,
    b: f64,
) {
    if target_m <= 0.0 || contact_dir.length_squared() < 1e-12 {
        return;
    }
    let norm = contact_dir.normalize_or_zero();
    let surface_normal = Vec3::new(norm.x as f32, norm.y as f32, norm.z as f32);
    let mass_ratio = (impactor_m / target_m).min(1.0);
    let angular_radius = ((mass_ratio).cbrt() as f32 * 0.35).clamp(0.04, 0.45);
    let elongation = (1.0 + (b - 0.5) * 2.5).clamp(1.2, 3.2) as f32;

    record_impact_crater_basin(
        commands,
        target_entity,
        surface_normal,
        angular_radius,
        sim_time_years,
        elongation,
    );
}

/// Delivers volatile inventory and generates an impact basin on the primary body during a merger.
pub fn deliver_volatiles_and_crater(
    commands: &mut Commands,
    primary_entity: Entity,
    p_pos: DVec3,
    s_pos: DVec3,
    p_m: f64,
    s_m: f64,
    s_comp: &Composition,
    sim_time_years: f64,
) {
    let d_water_earth = (s_m * s_comp.ice_frac) / EARTH_MASS_SOLAR;
    let d_gas_earth = (s_m * s_comp.gas_frac) / EARTH_MASS_SOLAR;

    if let Ok(mut p_cmd) = commands.get_entity(primary_entity) {
        if d_water_earth > 0.0 || d_gas_earth > 0.0 {
            p_cmd
                .entry::<VolatileInventory>()
                .and_modify(move |mut vol| {
                    vol.delivered_water_m_earth += d_water_earth;
                    vol.cometary_impact_count += 1;
                    vol.ocean_coverage_frac =
                        (vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
                    vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar
                        + (d_gas_earth * 120.0) as f32)
                        .clamp(0.01, 90.0);
                })
                .or_insert(VolatileInventory {
                    delivered_water_m_earth: d_water_earth,
                    ocean_coverage_frac: (d_water_earth / 0.0006).clamp(0.0, 0.85) as f32,
                    atmospheric_pressure_bar: (d_gas_earth * 120.0).clamp(0.01, 90.0) as f32,
                    cometary_impact_count: 1,
                });
        }

        let norm = (s_pos - p_pos).normalize_or_zero();
        let surface_normal = Vec3::new(norm.x as f32, norm.y as f32, norm.z as f32);
        let mass_ratio = if p_m > 0.0 { s_m / p_m } else { 1.0 };
        let angular_radius = ((mass_ratio).cbrt() as f32).clamp(0.05, 0.55);

        record_impact_crater_basin(
            commands,
            primary_entity,
            surface_normal,
            angular_radius,
            sim_time_years,
            1.0,
        );
    }
}

/// Relaxes and cools crater magma melt pools and gradually heals impact scars over simulation timescales.
pub fn update_impact_basin_relaxation(
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    mut query: Query<(
        &mut PlanetaryBasins,
        Option<&VolatileInventory>,
        Option<&PlanetaryClimate>,
    )>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }
    let dt_yr = sim_time.current_dt_yr;
    if dt_yr <= 0.0 {
        return;
    }

    for (mut pb, opt_vol, opt_climate) in query.iter_mut() {
        let has_weathering = opt_vol
            .is_some_and(|v| v.atmospheric_pressure_bar > 0.05 || v.ocean_coverage_frac > 0.02)
            || opt_climate.is_some_and(|c| c.cloud_coverage_frac > 0.05);

        for basin in &mut pb.basins {
            // 1. Magma melt glow cooling: cools from glowing molten lava to solidified rock over ~50 to 300 years
            if basin.melt_glow_fraction > 0.0 {
                let cooling_tau_yr = (f64::from(basin.angular_radius) * 600.0).clamp(50.0, 300.0);
                let decay = (dt_yr / cooling_tau_yr) as f32;
                basin.melt_glow_fraction = (basin.melt_glow_fraction - decay).max(0.0);
            }

            // 2. Crustal scar healing / weathering / isostatic relaxation:
            // Scars smoothly heal and fade back into the planetary crust.
            // Worlds with atmospheres and oceans erode/infill craters faster (~200 to 500 years).
            // Airless worlds heal via crustal viscous relaxation over ~600 to 1,500 years.
            let healing_tau_yr = if has_weathering {
                (f64::from(basin.angular_radius) * 800.0).clamp(200.0, 500.0)
            } else {
                (f64::from(basin.angular_radius) * 2000.0).clamp(600.0, 1500.0)
            };
            let heal_decay = (dt_yr / healing_tau_yr) as f32;
            basin.scar_intensity = (basin.scar_intensity - heal_decay).max(0.0);
        }

        // Cleanly remove fully healed scars so they no longer occupy shader/data slots
        pb.basins.retain(|b| b.scar_intensity > 0.005);
    }
}
