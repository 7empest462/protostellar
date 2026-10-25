//! Spawner routines and UI telemetry updates for the Interactive Planet Builder.

use crate::game::ui::types::{
    BuilderPreset, PlanetBuilderInfoText, PlanetBuilderPanel, PlanetBuilderState,
};
use crate::game::ui::NotificationToast;
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::{DiskParameters, PlayerInteractionState};
use crate::utils::constants::*;
use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;

pub fn compute_builder_spawn_coords(
    builder_state: &PlanetBuilderState,
    star_mass: f64,
    spawn_coords: Option<DVec3>,
) -> (DVec3, DVec3) {
    if let Some(custom_pos) = spawn_coords {
        let r = custom_pos.length().max(0.1);
        let v_circ = (G_ASTRO * star_mass / r).sqrt();
        let phi = custom_pos.z.atan2(custom_pos.x);
        let vel = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
        (custom_pos, vel)
    } else {
        let mut rng = rand::rng();
        let a = builder_state.semi_major_axis_au.max(0.1);
        let angle: f64 = rng.random_range(0.0..(2.0 * std::f64::consts::PI));
        let pos = DVec3::new(a * angle.cos(), 0.0, a * angle.sin());
        let v_circ = (G_ASTRO * star_mass / a).sqrt();
        let vel = DVec3::new(-v_circ * angle.sin(), 0.0, v_circ * angle.cos());
        (pos, vel)
    }
}

pub fn resolve_builder_body_type_and_temperature(
    preset: BuilderPreset,
    mass_solar: f64,
    comp: &Composition,
    fallback_radius_au: f64,
) -> (BodyType, f64, f64) {
    match preset {
        BuilderPreset::QuarkStar => (BodyType::QuarkStar, 0.00007, 10_000_000.0),
        BuilderPreset::DarkStar => (BodyType::DarkStar, 5.0 * SOLAR_RADIUS_AU, 5_500.0),
        BuilderPreset::BosonStar => (BodyType::BosonStar, 0.001, 10.0),
        BuilderPreset::IronStar => (BodyType::IronStar, 0.009 * SOLAR_RADIUS_AU, 0.1),
        BuilderPreset::ThorneZytkow => (BodyType::ThorneZytkow, 4.0 * SOLAR_RADIUS_AU, 3_200.0),
        BuilderPreset::WolfRayet => (BodyType::WolfRayet, 2.5 * SOLAR_RADIUS_AU, 85_000.0),
        BuilderPreset::OrangeDwarf => (BodyType::OrangeDwarf, 0.75 * SOLAR_RADIUS_AU, 4_500.0),
        BuilderPreset::PopIIIStar => (
            BodyType::PopulationIIIStar,
            12.0 * SOLAR_RADIUS_AU,
            65_000.0,
        ),
        _ => {
            let bt = crate::simulation::components::classify_body_by_mass_and_comp(
                mass_solar, comp, false,
            );
            let tk = match bt {
                BodyType::RedDwarf => 3000.0,
                BodyType::OrangeDwarf => 4500.0,
                BodyType::YellowDwarf => 5778.0,
                BodyType::YellowWhiteDwarf => 6800.0,
                BodyType::WhiteMainSequence => 9500.0,
                BodyType::BlueMainSequence => 18000.0,
                BodyType::BrownDwarf => 1400.0,
                BodyType::GasGiant => 160.0,
                BodyType::IceGiant => 80.0,
                BodyType::SuperEarth => 295.0,
                _ => 288.0,
            };
            (bt, fallback_radius_au, tk)
        }
    }
}

pub fn spawn_custom_builder_world(
    commands: &mut Commands,
    builder_state: &PlanetBuilderState,
    star_mass: f64,
    spawn_coords: Option<DVec3>,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    toast: &mut NotificationToast,
) -> Entity {
    let (pos, vel) = compute_builder_spawn_coords(builder_state, star_mass, spawn_coords);

    let comp = Composition {
        silicate_frac: f64::from(builder_state.rock_frac),
        ice_frac: f64::from(builder_state.ice_frac),
        metal_frac: f64::from(builder_state.metal_frac),
        organics_frac: 0.0,
        gas_frac: f64::from(builder_state.gas_frac),
    };
    let avg_density = comp.average_density();
    let initial_radius_au = ((3.0 * builder_state.mass_solar / avg_density)
        / (4.0 * std::f64::consts::PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.2);

    let (body_type, radius_au, temp_k) = resolve_builder_body_type_and_temperature(
        builder_state.active_preset,
        builder_state.mass_solar,
        &comp,
        initial_radius_au,
    );

    let new_ent = commands
        .spawn((
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration(DVec3::ZERO),
            Mass(builder_state.mass_solar),
            Radius(radius_au),
            Temperature(temp_k),
            comp,
            CelestialBody {
                name: builder_state.custom_name.clone(),
                body_type,
            },
            InternalDifferentiation {
                is_differentiated: true,
                differentiation_fraction: 0.9,
                core_radius_au: radius_au * 0.45,
                mantle_radius_au: radius_au * 0.90,
                crust_thickness_au: radius_au * 0.08,
                ocean_ice_thickness_au: if builder_state.ice_frac > 0.1 {
                    radius_au * 0.05
                } else {
                    0.0
                },
                core_temp_k: temp_k * 4.0,
                magnetic_field_gauss: if builder_state.metal_frac > 0.15 {
                    0.5
                } else {
                    0.05
                },
                has_theia_llsvp: false,
                llsvp_density_contrast: 0.0,
            },
            SpinState {
                spin_vector: DVec3::new(0.0, 1e-12, 0.0),
                rotation_period_hours: 24.0,
                axial_tilt_degrees: 15.0,
            },
            VolatileInventory {
                delivered_water_m_earth: if builder_state.ice_frac > 0.1 {
                    1.0
                } else {
                    0.0
                },
                ocean_coverage_frac: if builder_state.ice_frac > 0.1 {
                    0.70
                } else {
                    0.0
                },
                atmospheric_pressure_bar: if builder_state.gas_frac > 0.1 {
                    50.0
                } else {
                    1.0
                },
                cometary_impact_count: if builder_state.ice_frac > 0.1 { 10 } else { 0 },
            },
            PlanetaryClimate {
                surface_temperature_k: (temp_k + 33.0) as f32,
                equilibrium_temperature_k: temp_k as f32,
                greenhouse_delta_k: 33.0,
                albedo: 0.30,
                ice_coverage_frac: 0.1,
                cloud_coverage_frac: 0.5,
                climate_regime: if builder_state.gas_frac > 0.4 {
                    ClimateRegime::GasGiantEnvelope
                } else if temp_k < 260.0 {
                    ClimateRegime::SnowballIceAge
                } else {
                    ClimateRegime::TemperateHabitable
                },
                polar_ice_cap_latitude_deg: if temp_k < 260.0 { 35.0 } else { 75.0 },
            },
            BiosphereState::default(),
            ElectromagneticFieldState {
                magnetic_field_gauss: if builder_state.metal_frac > 0.15 {
                    0.5
                } else {
                    0.05
                },
                rotation_period_sec: 24.0 * 3600.0,
                magnetic_inclination_rad: 0.15,
                jet_length_au: 0.0,
                synchrotron_intensity: 0.0,
            },
        ))
        .id();

    player_state.selected_entity = Some(new_ent);
    if let Ok(mut cam) = camera_query.single_mut() {
        cam.target_entity = Some(new_ent);
    }
    toast.message = format!(
        "🚀 Spawned & Focused: \"{}\" ({:.2} AU)!",
        builder_state.custom_name,
        pos.length()
    );
    toast.timer = 4.5;

    new_ent
}

pub fn update_planet_builder_ui(
    builder_state: Res<PlanetBuilderState>,
    disk_params: Res<DiskParameters>,
    mut panel_query: Query<&mut Node, With<PlanetBuilderPanel>>,
    mut text_query: Query<&mut Text, With<PlanetBuilderInfoText>>,
) {
    if let Ok(mut node) = panel_query.single_mut() {
        node.display = if builder_state.is_open {
            Display::Flex
        } else {
            Display::None
        };
    }

    if let Ok(mut text) = text_query.single_mut() {
        let star_mass = disk_params.central_star_mass;
        let v_circ_kms = (G_ASTRO * star_mass / builder_state.semi_major_axis_au.max(0.05)).sqrt()
            * (AU_TO_METERS / 1000.0)
            / (365.25 * 86400.0);
        let mass_earth = builder_state.mass_solar / EARTH_MASS_SOLAR;
        let mass_jup = builder_state.mass_solar / JUPITER_MASS_SOLAR;

        let mass_str = if builder_state.mass_solar >= 0.05 {
            format!("{:.3} M_sun", builder_state.mass_solar)
        } else if mass_jup >= 0.5 {
            format!("{mass_jup:.2} M_Jup ({mass_earth:.0} M_earth)")
        } else {
            format!("{mass_earth:.2} M_earth")
        };

        let ecc_str = if builder_state.eccentricity < 0.05 {
            "Circular (0.00)".to_string()
        } else if builder_state.eccentricity < 0.35 {
            format!("Moderate ({:.2})", builder_state.eccentricity)
        } else if builder_state.eccentricity < 1.0 {
            format!("High ({:.2})", builder_state.eccentricity)
        } else {
            format!("Hyperbolic ({:.2})", builder_state.eccentricity)
        };

        text.0 = format!(
            "🛠️ PLANET BUILDER & SPAWNER\n\
            ───────────────────────────────\n\
            Target: {} [{}]\n\
            Mass:   {}\n\
            Orbit:  {:.2} AU | v_circ: {:.1} km/s\n\
            Ecc:    {}\n\
            Mix:    Rock {:.0}% | Ice {:.0}% | Fe {:.0}% | Gas {:.0}%\n\
            Mode:   {}",
            builder_state.custom_name,
            builder_state.active_preset.display_name(),
            mass_str,
            builder_state.semi_major_axis_au,
            v_circ_kms,
            ecc_str,
            builder_state.rock_frac * 100.0,
            builder_state.ice_frac * 100.0,
            builder_state.metal_frac * 100.0,
            builder_state.gas_frac * 100.0,
            if builder_state.click_to_spawn_mode {
                "🎯 Click 3D Plane to Place"
            } else {
                "🚀 Instant Orbit Insertion"
            }
        );
    }
}
