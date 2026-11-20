//! Spawner routines and UI telemetry updates for the Interactive Planet Builder.

use crate::game::ui::NotificationToast;
use crate::game::ui::types::{
    BuilderPreset, PlanetBuilderInfoText, PlanetBuilderPanel, PlanetBuilderState,
};
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::{DiskParameters, PlayerInteractionState};
use crate::utils::constants::*;
use crate::utils::math::FastRngExt;
use bevy::math::DVec3;
use bevy::prelude::*;

pub fn compute_builder_spawn_coords(
    builder_state: &PlanetBuilderState,
    star_mass: f64,
    spawn_coords: Option<DVec3>,
) -> (DVec3, DVec3) {
    compute_builder_spawn_coords_relative(
        builder_state,
        star_mass,
        DVec3::ZERO,
        DVec3::ZERO,
        spawn_coords,
    )
}

pub fn compute_builder_spawn_coords_relative(
    builder_state: &PlanetBuilderState,
    star_mass: f64,
    star_pos: DVec3,
    star_vel: DVec3,
    spawn_coords: Option<DVec3>,
) -> (DVec3, DVec3) {
    let mu = G_ASTRO * (star_mass + builder_state.mass_solar).max(1e-6);
    if let Some(custom_pos) = spawn_coords {
        let rel_pos = custom_pos - star_pos;
        let r = rel_pos.length().max(0.1);
        let v_circ = (mu / r).sqrt();
        let phi = rel_pos.z.atan2(rel_pos.x);
        let vel = star_vel + DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
        (custom_pos, vel)
    } else {
        let mut rng = fastrand::Rng::new();
        let a = builder_state.semi_major_axis_au.max(0.1);
        let ecc = builder_state.eccentricity.clamp(0.0, 1.30);
        let angle: f64 = rng.f64_range(0.0, 2.0 * std::f64::consts::PI);
        let (r_p, v_p) = if ecc < 1.0 {
            let r_p = a * (1.0 - ecc);
            let v_p = (mu / a * (1.0 + ecc) / (1.0 - ecc).max(1e-4)).sqrt();
            (r_p, v_p)
        } else {
            let r_p = (a * (ecc - 1.0).max(0.05)).max(0.15);
            let v_p = (mu * (2.0 / r_p + 1.0 / a)).sqrt();
            (r_p, v_p)
        };
        let pos = star_pos + DVec3::new(r_p * angle.cos(), 0.0, r_p * angle.sin());
        let vel = star_vel + DVec3::new(-v_p * angle.sin(), 0.0, v_p * angle.cos());
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
        BuilderPreset::RedDwarfStar => (BodyType::RedDwarf, 0.25 * SOLAR_RADIUS_AU, 3_100.0),
        BuilderPreset::OrangeDwarf => (BodyType::OrangeDwarf, 0.75 * SOLAR_RADIUS_AU, 4_500.0),
        BuilderPreset::BrownDwarf => (BodyType::BrownDwarf, 0.11 * SOLAR_RADIUS_AU, 1_400.0),
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
            let resolved_rad = if bt == BodyType::BrownDwarf {
                0.11 * SOLAR_RADIUS_AU
            } else if bt.is_star_or_remnant() {
                SOLAR_RADIUS_AU * mass_solar.max(0.08).powf(0.8)
            } else {
                fallback_radius_au
            };
            (bt, resolved_rad, tk)
        }
    }
}

struct BuilderEntitySpec {
    body_type: BodyType,
    pos: DVec3,
    vel: DVec3,
    radius_au: f64,
    temp_k: f64,
    comp: Composition,
}

fn resolve_builder_evolution_phase(body_type: BodyType) -> StellarEvolutionPhase {
    match body_type {
        BodyType::BrownDwarf | BodyType::Protostar | BodyType::TTauriStar => {
            StellarEvolutionPhase::ProtostarContraction
        }
        BodyType::RedGiant | BodyType::ThorneZytkow => StellarEvolutionPhase::RedGiantBranch,
        BodyType::RedSupergiant | BodyType::WolfRayet | BodyType::Hypergiant => {
            StellarEvolutionPhase::RedSupergiantBranch
        }
        BodyType::WhiteDwarf | BodyType::BlackDwarf | BodyType::IronStar => {
            StellarEvolutionPhase::WhiteDwarf
        }
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::QuarkStar => {
            StellarEvolutionPhase::NeutronStarPulsar
        }
        BodyType::Magnetar => StellarEvolutionPhase::MagnetarRemnant,
        BodyType::BlackHole | BodyType::QuasiStar => StellarEvolutionPhase::BlackHoleRemnant,
        _ => StellarEvolutionPhase::MainSequence,
    }
}

fn spawn_builder_star_entity(
    commands: &mut Commands,
    builder_state: &PlanetBuilderState,
    spec: BuilderEntitySpec,
) -> Entity {
    let is_bd = spec.body_type == BodyType::BrownDwarf;
    let lum = if is_bd {
        1e-4 * (builder_state.mass_solar / 0.05).clamp(0.1, 2.0)
    } else {
        builder_state.mass_solar.max(0.01).powf(3.5)
    };
    commands
        .spawn((
            SimPosition(spec.pos),
            SimVelocity(spec.vel),
            SimAcceleration(DVec3::ZERO),
            Mass(builder_state.mass_solar),
            Radius(spec.radius_au),
            Temperature(spec.temp_k),
            spec.comp,
            CelestialBody {
                name: builder_state.custom_name.clone(),
                body_type: spec.body_type,
            },
            InternalDifferentiation {
                is_differentiated: true,
                differentiation_fraction: 1.0,
                core_radius_au: spec.radius_au * 0.28,
                mantle_radius_au: spec.radius_au * 0.72,
                crust_thickness_au: spec.radius_au * 0.05,
                ocean_ice_thickness_au: 0.0,
                core_temp_k: if is_bd { 2.5e6 } else { 1.5e7 },
                magnetic_field_gauss: 10.0,
                has_theia_llsvp: false,
                llsvp_density_contrast: 0.0,
            },
            SpinState {
                spin_vector: DVec3::new(0.0, 1e-12, 0.0),
                rotation_period_hours: 24.0,
                axial_tilt_degrees: 7.25,
            },
            Luminosity(lum),
            IgnitionState {
                core_temperature: if is_bd { 2.5e6 } else { 1.5e7 },
                fusion_fraction: if is_bd { 0.05 } else { 1.0 },
                is_ignited: !is_bd,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState {
                phase: resolve_builder_evolution_phase(spec.body_type),
                ..default()
            },
            ElectromagneticFieldState {
                magnetic_field_gauss: 10.0,
                rotation_period_sec: 24.0 * 3600.0,
                magnetic_inclination_rad: 0.15,
                jet_length_au: 0.0,
                synchrotron_intensity: 0.0,
            },
        ))
        .id()
}

fn spawn_builder_planet_entity(
    commands: &mut Commands,
    builder_state: &PlanetBuilderState,
    spec: BuilderEntitySpec,
) -> Entity {
    let mut ent_cmd = commands.spawn((
        SimPosition(spec.pos),
        SimVelocity(spec.vel),
        SimAcceleration(DVec3::ZERO),
        Mass(builder_state.mass_solar),
        Radius(spec.radius_au),
        Temperature(spec.temp_k),
        spec.comp,
        CelestialBody {
            name: builder_state.custom_name.clone(),
            body_type: spec.body_type,
        },
        InternalDifferentiation {
            is_differentiated: true,
            differentiation_fraction: 0.9,
            core_radius_au: spec.radius_au * 0.45,
            mantle_radius_au: spec.radius_au * 0.90,
            crust_thickness_au: spec.radius_au * 0.08,
            ocean_ice_thickness_au: if builder_state.ice_frac > 0.1 {
                spec.radius_au * 0.05
            } else {
                0.0
            },
            core_temp_k: spec.temp_k * 4.0,
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
            surface_temperature_k: (spec.temp_k + 33.0) as f32,
            equilibrium_temperature_k: spec.temp_k as f32,
            greenhouse_delta_k: 33.0,
            albedo: 0.30,
            ice_coverage_frac: 0.1,
            cloud_coverage_frac: 0.5,
            climate_regime: if builder_state.gas_frac > 0.4 {
                ClimateRegime::GasGiantEnvelope
            } else if spec.temp_k < 260.0 {
                ClimateRegime::SnowballIceAge
            } else {
                ClimateRegime::TemperateHabitable
            },
            polar_ice_cap_latitude_deg: if spec.temp_k < 260.0 { 35.0 } else { 75.0 },
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
    ));
    match builder_state.active_preset {
        BuilderPreset::IoLike => {
            ent_cmd.insert(VolcanicState {
                activity_type: VolcanicActivityType::IoSulfurVolcanism,
                active_plumes: 4,
                max_plume_height_km: 350.0,
                eruption_flux_kg_s: 1.5e7,
                cryo_ring_feed_rate_kg_s: 0.0,
                caldera_temperature_k: 1600.0,
                sulfur_fallout_fraction: 0.82,
            });
        }
        BuilderPreset::EnceladusLike => {
            ent_cmd.insert(VolcanicState {
                activity_type: VolcanicActivityType::CryoWaterIce,
                active_plumes: 4,
                max_plume_height_km: 480.0,
                eruption_flux_kg_s: 2.5e2,
                cryo_ring_feed_rate_kg_s: 1.8e2,
                caldera_temperature_k: 273.0,
                sulfur_fallout_fraction: 0.0,
            });
        }
        _ => {}
    }
    ent_cmd.id()
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
    spawn_custom_builder_world_relative(
        commands,
        builder_state,
        star_mass,
        DVec3::ZERO,
        DVec3::ZERO,
        spawn_coords,
        player_state,
        camera_query,
        toast,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Relative builder spawn requires central star kinematics alongside UI handles"
)]
pub fn spawn_custom_builder_world_relative(
    commands: &mut Commands,
    builder_state: &PlanetBuilderState,
    star_mass: f64,
    star_pos: DVec3,
    star_vel: DVec3,
    spawn_coords: Option<DVec3>,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    toast: &mut NotificationToast,
) -> Entity {
    let (pos, vel) = compute_builder_spawn_coords_relative(
        builder_state,
        star_mass,
        star_pos,
        star_vel,
        spawn_coords,
    );

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

    let spec = BuilderEntitySpec {
        body_type,
        pos,
        vel,
        radius_au,
        temp_k,
        comp,
    };
    let new_ent = if body_type.is_star_or_remnant() {
        spawn_builder_star_entity(commands, builder_state, spec)
    } else {
        spawn_builder_planet_entity(commands, builder_state, spec)
    };

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
    let is_visible = builder_state.is_open && !builder_state.is_minimized;
    let target_display = if is_visible {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok(mut node) = panel_query.single_mut()
        && node.display != target_display
    {
        node.display = target_display;
    }

    if !is_visible {
        return;
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

        let new_text = format!(
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
        if text.0 != new_text {
            text.0 = new_text;
        }
    }
}
