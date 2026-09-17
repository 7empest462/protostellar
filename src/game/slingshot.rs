//! Interactive Orbital Slingshot Launcher system and world spawning.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::game::ui::NotificationToast;
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;
use crate::utils::math::*;

fn handle_slingshot_hotkeys(
    keyboard: &ButtonInput<KeyCode>,
    slingshot_state: &mut SlingshotState,
    player_state: &mut PlayerInteractionState,
    toast: &mut NotificationToast,
) {
    if keyboard.just_pressed(KeyCode::KeyK) {
        slingshot_state.is_active = !slingshot_state.is_active;
        player_state.active_tool = if slingshot_state.is_active {
            PlayerTool::Slingshot
        } else {
            PlayerTool::Inspect
        };
        toast.message = if slingshot_state.is_active {
            format!(
                "🎯 Slingshot Launcher Active [{} {}] [Click & Drag to Aim | [C] Cycle]",
                slingshot_state.archetype.icon(),
                slingshot_state.archetype.display_name()
            )
        } else {
            "🎯 Slingshot Launcher Closed [K]".to_string()
        };
        toast.timer = 3.5;
        if !slingshot_state.is_active {
            slingshot_state.drag_origin = None;
            slingshot_state.drag_current = None;
            slingshot_state.target_entity = None;
        }
    }

    if slingshot_state.is_active && keyboard.just_pressed(KeyCode::KeyC) {
        slingshot_state.archetype = slingshot_state.archetype.cycle();
        toast.message = format!(
            "🎯 Slingshot Archetype: {} {}",
            slingshot_state.archetype.icon(),
            slingshot_state.archetype.display_name()
        );
        toast.timer = 2.5;
    }
}

fn compute_cursor_plane_coords(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<DVec3> {
    let cursor_pos = window.cursor_position()?;
    let ray = camera
        .viewport_to_world(camera_transform, cursor_pos)
        .ok()?;
    if ray.direction.y.abs() > 1e-5 {
        let t = -ray.origin.y / ray.direction.y;
        if t > 0.0 {
            let hit = ray.origin + *ray.direction * t;
            Some(DVec3::new(f64::from(hit.x), 0.0, f64::from(hit.z)))
        } else {
            None
        }
    } else {
        None
    }
}

fn execute_slingshot_release(
    commands: &mut Commands,
    slingshot_state: &mut SlingshotState,
    bodies_query: &mut Query<(
        Entity,
        &SimPosition,
        &Radius,
        &mut SimVelocity,
        &CelestialBody,
    )>,
    pan_orbit_query: &mut Query<&mut PanOrbitCamera>,
    player_state: &mut PlayerInteractionState,
    star_mass: f64,
    toast: &mut NotificationToast,
) {
    if let (Some(origin), Some(current)) =
        (slingshot_state.drag_origin, slingshot_state.drag_current)
    {
        let delta = current - origin;
        let dist = delta.length();

        if dist >= 0.04 {
            let launch_velocity = delta * slingshot_state.velocity_scale;
            let v_kms = launch_velocity.length() * AU_PER_YR_TO_KM_PER_S;

            if let Some(target_ent) = slingshot_state.target_entity {
                if let Ok((_, _, _, mut vel, body)) = bodies_query.get_mut(target_ent) {
                    vel.0 += launch_velocity;
                    toast.message = format!(
                        "⚡ Slingshot Impulse: {:+.1} km/s applied to {}!",
                        v_kms, body.name
                    );
                    toast.timer = 4.0;
                }
            } else {
                let new_ent = spawn_slingshot_world(
                    commands,
                    slingshot_state.archetype,
                    origin,
                    launch_velocity,
                    star_mass,
                );

                player_state.selected_entity = Some(new_ent);
                if let Ok(mut cam) = pan_orbit_query.single_mut() {
                    cam.target_entity = Some(new_ent);
                }

                let elements =
                    state_vectors_to_orbital_elements(origin, launch_velocity, star_mass, 1e-6);
                if let Some(el) = elements {
                    toast.message = format!(
                        "🚀 Launched {}! v={:.1} km/s | a={:.2} AU | e={:.2}",
                        slingshot_state.archetype.display_name(),
                        v_kms,
                        el.semi_major_axis,
                        el.eccentricity
                    );
                } else {
                    toast.message = format!(
                        "🚀 Launched {} into orbit! v={:.1} km/s",
                        slingshot_state.archetype.display_name(),
                        v_kms
                    );
                }
                toast.timer = 5.0;
            }
        }
    }

    slingshot_state.drag_origin = None;
    slingshot_state.drag_current = None;
    slingshot_state.target_entity = None;
}

/// System that handles interactive orbital slingshot clicks, aiming drag vector, and launching.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Bevy system query dispatches across window, cameras, bodies, and slingshot state"
)]
pub fn handle_slingshot_input(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<PanOrbitCamera>>,
    mut pan_orbit_query: Query<&mut PanOrbitCamera>,
    mut slingshot_state: ResMut<SlingshotState>,
    mut player_state: ResMut<PlayerInteractionState>,
    disk_params: Res<DiskParameters>,
    mut toast: ResMut<NotificationToast>,
    ui_interaction_query: Query<&Interaction, With<Button>>,
    mut bodies_query: Query<(
        Entity,
        &SimPosition,
        &Radius,
        &mut SimVelocity,
        &CelestialBody,
    )>,
) {
    handle_slingshot_hotkeys(
        &keyboard,
        &mut slingshot_state,
        &mut player_state,
        &mut toast,
    );

    if !slingshot_state.is_active {
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let cursor_over_ui = ui_interaction_query
        .iter()
        .any(|i| *i == Interaction::Pressed || *i == Interaction::Hovered);

    let hit_plane_coords = compute_cursor_plane_coords(window, camera, camera_transform);

    // 1. Mouse down: Start drag
    if mouse_buttons.just_pressed(MouseButton::Left) && !cursor_over_ui {
        if let Some(plane_pt) = hit_plane_coords {
            let mut nearest_ent: Option<(Entity, DVec3)> = None;
            let mut min_dist = 0.50f64;

            for (ent, pos, rad, _, body) in bodies_query.iter() {
                let hit_radius = (rad.0 * 2.5).max(if body.body_type.is_star_or_remnant() {
                    2.0
                } else {
                    0.4
                });
                let d = (pos.0 - plane_pt).length();
                if d < hit_radius && d < min_dist {
                    min_dist = d;
                    nearest_ent = Some((ent, pos.0));
                }
            }

            if let Some((target_ent, target_pos)) = nearest_ent {
                slingshot_state.target_entity = Some(target_ent);
                slingshot_state.drag_origin = Some(target_pos);
                slingshot_state.drag_current = Some(plane_pt);
            } else {
                slingshot_state.target_entity = None;
                slingshot_state.drag_origin = Some(plane_pt);
                slingshot_state.drag_current = Some(plane_pt);
            }
        }
    }

    // 2. Mouse dragging: update current position
    if mouse_buttons.pressed(MouseButton::Left) && slingshot_state.drag_origin.is_some() {
        if let Some(plane_pt) = hit_plane_coords {
            slingshot_state.drag_current = Some(plane_pt);
        }
    }

    // 3. Mouse released: execute launch or impulse
    if mouse_buttons.just_released(MouseButton::Left) {
        let star_mass = disk_params.central_star_mass;
        execute_slingshot_release(
            &mut commands,
            &mut slingshot_state,
            &mut bodies_query,
            &mut pan_orbit_query,
            &mut player_state,
            star_mass,
            &mut toast,
        );
    }
}

struct SlingshotWorldConfig {
    name: String,
    body_type: BodyType,
    mass_solar: f64,
    comp: Composition,
    temp_k: f64,
    has_tail: bool,
}

fn archetype_preset(archetype: SlingshotArchetype) -> SlingshotWorldConfig {
    match archetype {
        SlingshotArchetype::Asteroid => SlingshotWorldConfig {
            name: "Slingshot Asteroid".to_string(),
            body_type: BodyType::Asteroid,
            mass_solar: 1.5e-8 * EARTH_MASS_SOLAR,
            comp: Composition {
                silicate_frac: 0.70,
                ice_frac: 0.10,
                metal_frac: 0.20,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            temp_k: 180.0,
            has_tail: false,
        },
        SlingshotArchetype::Comet => SlingshotWorldConfig {
            name: "Slingshot Comet".to_string(),
            body_type: BodyType::Comet,
            mass_solar: 2.0e-9 * EARTH_MASS_SOLAR,
            comp: Composition {
                silicate_frac: 0.15,
                ice_frac: 0.80,
                metal_frac: 0.05,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            temp_k: 120.0,
            has_tail: true,
        },
        SlingshotArchetype::TerrestrialPlanet => SlingshotWorldConfig {
            name: "Slingshot Terrestrial World".to_string(),
            body_type: BodyType::TerrestrialPlanet,
            mass_solar: 1.0 * EARTH_MASS_SOLAR,
            comp: Composition {
                silicate_frac: 0.67,
                ice_frac: 0.01,
                metal_frac: 0.32,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            temp_k: 288.0,
            has_tail: false,
        },
        SlingshotArchetype::WaterWorld => SlingshotWorldConfig {
            name: "Slingshot Ocean World".to_string(),
            body_type: BodyType::SuperEarth,
            mass_solar: 2.2 * EARTH_MASS_SOLAR,
            comp: Composition {
                silicate_frac: 0.25,
                ice_frac: 0.70,
                metal_frac: 0.05,
                organics_frac: 0.0,
                gas_frac: 0.0,
            },
            temp_k: 275.0,
            has_tail: false,
        },
        SlingshotArchetype::GasGiant => SlingshotWorldConfig {
            name: "Slingshot Gas Giant".to_string(),
            body_type: BodyType::GasGiant,
            mass_solar: 1.0 * JUPITER_MASS_SOLAR,
            comp: Composition {
                silicate_frac: 0.03,
                ice_frac: 0.02,
                metal_frac: 0.01,
                organics_frac: 0.0,
                gas_frac: 0.94,
            },
            temp_k: 165.0,
            has_tail: false,
        },
        SlingshotArchetype::RoguePlanet => SlingshotWorldConfig {
            name: "Slingshot Rogue Invader".to_string(),
            body_type: BodyType::GasGiant,
            mass_solar: 3.0 * JUPITER_MASS_SOLAR,
            comp: Composition {
                silicate_frac: 0.02,
                ice_frac: 0.02,
                metal_frac: 0.01,
                organics_frac: 0.0,
                gas_frac: 0.95,
            },
            temp_k: 95.0,
            has_tail: false,
        },
    }
}

fn create_slingshot_differentiation(
    mass_solar: f64,
    radius_au: f64,
    temp_k: f64,
    comp: &Composition,
) -> InternalDifferentiation {
    InternalDifferentiation {
        is_differentiated: mass_solar >= 0.01 * EARTH_MASS_SOLAR,
        differentiation_fraction: 0.85,
        core_radius_au: radius_au * 0.40,
        mantle_radius_au: radius_au * 0.88,
        crust_thickness_au: radius_au * 0.08,
        ocean_ice_thickness_au: if comp.ice_frac > 0.2 {
            radius_au * 0.05
        } else {
            0.0
        },
        core_temp_k: temp_k * 3.5,
        magnetic_field_gauss: if comp.metal_frac > 0.15 { 0.4 } else { 0.02 },
        has_theia_llsvp: false,
        llsvp_density_contrast: 0.0,
    }
}

/// Spawns a new celestial body according to the chosen Slingshot archetype.
pub fn spawn_slingshot_world(
    commands: &mut Commands,
    archetype: SlingshotArchetype,
    pos: DVec3,
    vel: DVec3,
    _star_mass: f64,
) -> Entity {
    let cfg = archetype_preset(archetype);
    let avg_density = cfg.comp.average_density();
    let radius_au = ((3.0 * cfg.mass_solar / avg_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.15);

    let diff = create_slingshot_differentiation(cfg.mass_solar, radius_au, cfg.temp_k, &cfg.comp);

    let mut entity_cmds = commands.spawn((
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration(DVec3::ZERO),
        Mass(cfg.mass_solar),
        Radius(radius_au),
        Temperature(cfg.temp_k),
        cfg.comp,
        CelestialBody {
            body_type: cfg.body_type,
            name: cfg.name,
        },
        diff,
        SpinState {
            spin_vector: DVec3::new(0.0, 1e-12, 0.0),
            rotation_period_hours: 24.0,
            axial_tilt_degrees: 15.0,
        },
        VolatileInventory {
            delivered_water_m_earth: if cfg.comp.ice_frac > 0.1 { 1.0 } else { 0.0 },
            ocean_coverage_frac: if cfg.body_type == BodyType::Comet {
                0.0
            } else if cfg.comp.ice_frac > 0.3 {
                0.85
            } else {
                0.0
            },
            atmospheric_pressure_bar: if cfg.body_type == BodyType::Comet {
                0.0
            } else if cfg.comp.gas_frac > 0.1 {
                50.0
            } else {
                1.0
            },
            cometary_impact_count: u32::from(cfg.has_tail),
        },
        PlanetaryClimate {
            surface_temperature_k: cfg.temp_k as f32,
            equilibrium_temperature_k: cfg.temp_k as f32,
            greenhouse_delta_k: 20.0,
            albedo: 0.30,
            ice_coverage_frac: if cfg.temp_k < 260.0 { 0.6 } else { 0.1 },
            cloud_coverage_frac: if cfg.body_type == BodyType::Comet {
                0.0
            } else {
                0.4
            },
            climate_regime: if cfg.comp.gas_frac > 0.4 {
                ClimateRegime::GasGiantEnvelope
            } else if cfg.temp_k < 260.0 {
                ClimateRegime::SnowballIceAge
            } else {
                ClimateRegime::TemperateHabitable
            },
        },
        BiosphereState::default(),
        ElectromagneticFieldState {
            magnetic_field_gauss: if cfg.comp.metal_frac > 0.15 {
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

    if cfg.has_tail {
        entity_cmds.insert(AtmosphericEscapeTail {
            loss_rate_m_earth_per_myr: 0.05,
            tail_length_au: 0.35,
            ion_color: Color::srgba(0.25, 0.90, 1.0, 0.85),
            is_active: true,
        });
    }

    entity_cmds.id()
}
