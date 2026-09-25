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
                        ((vol.delivered_water_m_earth / 0.0006) * 0.71).clamp(0.0, 0.98) as f32;
                    vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar
                        + (d_gas_earth * 120.0) as f32)
                        .clamp(0.01, 90.0);
                })
                .or_insert(VolatileInventory {
                    delivered_water_m_earth: d_water_earth,
                    ocean_coverage_frac: ((d_water_earth / 0.0006) * 0.71).clamp(0.0, 0.98) as f32,
                    atmospheric_pressure_bar: (d_gas_earth * 120.0).clamp(0.01, 90.0) as f32,
                    cometary_impact_count: 1,
                });

            p_cmd
                .entry::<crate::simulation::terraforming::TerraformingAtmosphere>()
                .and_modify(move |mut terra| {
                    terra.surface_liquid_water_m_earth += d_water_earth;
                    terra.atmospheric_water_m_earth += d_water_earth * 0.05;
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

/// Computes thermal cooling and crustal/atmospheric relaxation timescales based on planetary classification.
fn compute_basin_timescales(
    angular_radius: f32,
    body_type: BodyType,
    comp: &Composition,
    has_weathering: bool,
) -> (f64, f64) {
    let r = f64::from(angular_radius);
    let norm = comp.normalized();
    if matches!(body_type, BodyType::GasGiant | BodyType::IceGiant) || norm.gas_frac > 0.40 {
        // Fluid atmospheric relaxation: supersonic zonal jet streams shear and disperse
        // atmospheric soot / aerosol plumes within a few months to several years.
        let cooling_tau_yr = (r * 10.0).clamp(0.2, 1.5);
        let healing_tau_yr = (r * 12.0).clamp(1.5, 6.0);
        (cooling_tau_yr, healing_tau_yr)
    } else if norm.ice_frac > 0.35 {
        // Icy lithosphere / cryo-crust (Europa, Enceladus, Pluto, Callisto):
        // Slushy cryomagma freezes over decades; viscous relaxation of ice shell over centuries.
        let cooling_tau_yr = (r * 300.0).clamp(20.0, 150.0);
        let healing_tau_yr = (r * 1200.0).clamp(300.0, 800.0);
        (cooling_tau_yr, healing_tau_yr)
    } else {
        // Silicate / rocky lithosphere (Earth, Mars, Moon, Mercury, Asteroids):
        // Molten basalt cools over centuries; weathering or isostatic relaxation over deep time.
        let cooling_tau_yr = (r * 600.0).clamp(50.0, 300.0);
        let healing_tau_yr = if has_weathering {
            (r * 800.0).clamp(200.0, 500.0)
        } else {
            (r * 2000.0).clamp(600.0, 1500.0)
        };
        (cooling_tau_yr, healing_tau_yr)
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
        Option<&CelestialBody>,
        Option<&Composition>,
    )>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }
    let dt_yr = sim_time.current_dt_yr;
    if dt_yr <= 0.0 {
        return;
    }

    for (mut pb, opt_vol, opt_climate, opt_body, opt_comp) in query.iter_mut() {
        let has_weathering = opt_vol
            .is_some_and(|v| v.atmospheric_pressure_bar > 0.05 || v.ocean_coverage_frac > 0.02)
            || opt_climate.is_some_and(|c| c.cloud_coverage_frac > 0.05);

        let body_type = opt_body.map_or(BodyType::TerrestrialPlanet, |b| b.body_type);
        let default_comp = Composition::default();
        let comp = opt_comp.unwrap_or(&default_comp);

        for basin in &mut pb.basins {
            let (cooling_tau_yr, healing_tau_yr) =
                compute_basin_timescales(basin.angular_radius, body_type, comp, has_weathering);

            if basin.melt_glow_fraction > 0.0 {
                let decay = (dt_yr / cooling_tau_yr) as f32;
                basin.melt_glow_fraction = (basin.melt_glow_fraction - decay).max(0.0);
            }

            let heal_decay = (dt_yr / healing_tau_yr) as f32;
            basin.scar_intensity = (basin.scar_intensity - heal_decay).max(0.0);
        }

        // Cleanly remove fully healed scars so they no longer occupy shader/data slots
        pb.basins.retain(|b| b.scar_intensity > 0.005);
    }
}
