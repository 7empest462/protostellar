//! Dynamic body selector bar, celestial world palette classification, and custom planet builder spawning.

use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;
use std::f64::consts::PI;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::types::*;

pub fn body_to_button_label(name: &str, body_type: BodyType, is_star: bool) -> String {
    let lower = name.to_lowercase();
    if lower.contains("little red dot") || body_type == BodyType::QuasiStar {
        "🔴 Little Red Dot".to_string()
    } else if lower.contains("trappist-1") {
        if is_star || body_type.is_star_or_remnant() {
            "TRAPPIST-1".to_string()
        } else if lower.contains("trappist-1e") {
            "1e (HZ)".to_string()
        } else if lower.contains("trappist-1f") {
            "1f (HZ)".to_string()
        } else if lower.contains("trappist-1g") {
            "1g (HZ)".to_string()
        } else if lower.contains("trappist-1b") {
            "1b".to_string()
        } else if lower.contains("trappist-1c") {
            "1c".to_string()
        } else if lower.contains("trappist-1d") {
            "1d".to_string()
        } else if lower.contains("trappist-1h") {
            "1h".to_string()
        } else if let Some(suffix) = lower.strip_prefix("trappist-1") {
            format!("1{}", suffix.trim())
        } else {
            name.to_string()
        }
    } else if lower.contains("kepler-16") {
        if lower.contains("kepler-16a") {
            "Kepler-16A".to_string()
        } else if lower.contains("kepler-16b (secondary")
            || lower.contains("kepler-16b (secondary m-dwarf)")
            || (is_star && lower.contains("16b"))
        {
            "Kepler-16B".to_string()
        } else if lower.contains("kepler-16b") {
            "Kepler-16b".to_string()
        } else {
            name.to_string()
        }
    } else if lower.contains("nemesis") || lower.contains("rogue") {
        "Nemesis-X".to_string()
    } else if lower.contains("hot jupiter") || lower.contains("hd 209458") {
        "Hot Jupiter".to_string()
    } else if lower.contains("proto-earth") || (lower.contains("earth") && lower.contains("inner"))
    {
        "Proto-Earth".to_string()
    } else if lower.contains("proto-jupiter") {
        "Proto-Jupiter".to_string()
    } else if lower.contains("proto-mercury") {
        "Mercury".to_string()
    } else if lower.contains("proto-venus") {
        "Venus".to_string()
    } else if lower.contains("proto-mars") {
        "Mars".to_string()
    } else if lower.contains("theia") {
        "Theia".to_string()
    } else if lower.contains("proto-saturn") {
        "Saturn".to_string()
    } else if lower.contains("proto-uranus") {
        "Uranus".to_string()
    } else if lower.contains("proto-neptune") {
        "Neptune".to_string()
    } else if lower.contains("pluto") {
        "🧊 Pluto".to_string()
    } else if lower.contains("planet nine") || lower.contains("planet 9") {
        "🪐 Planet 9".to_string()
    } else if is_embryo_body(name, body_type) {
        if lower.contains("theia") {
            "Theia".to_string()
        } else if let Some(suffix) = name.strip_prefix("Embryo-") {
            format!("🌱 {suffix}")
        } else if let Some(suffix) = name.strip_prefix("Embryo #") {
            format!("🌱 #{suffix}")
        } else {
            format!("🌱 {name}")
        }
    } else if lower.contains("kuiper") {
        "Kuiper".to_string()
    } else if lower.contains("ceres") {
        "Ceres".to_string()
    } else if lower.contains("protostar") || (is_star && lower.contains("sun")) {
        "Sun".to_string()
    } else if lower.contains("host star") {
        "Host Star".to_string()
    } else if lower.starts_with("asteroid-") {
        name.strip_prefix("Asteroid-")
            .map_or_else(|| name.to_string(), |s| format!("Ast-{s}"))
    } else if lower.starts_with("dust-") {
        name.strip_prefix("Dust-")
            .map_or_else(|| name.to_string(), |s| format!("Dust-{s}"))
    } else if let Some(prefix) = name.get(..12) {
        if name.len() > 14 {
            format!("{prefix}…")
        } else {
            name.to_string()
        }
    } else if name.chars().count() > 14 {
        let prefix: String = name.chars().take(12).collect();
        format!("{prefix}…")
    } else {
        name.to_string()
    }
}

pub fn body_to_button_colors(name: &str, body_type: BodyType, is_star: bool) -> (Color, Color) {
    let lower = name.to_lowercase();
    if is_star || body_type.is_star_or_remnant() {
        if lower.contains("little red dot") || body_type == BodyType::QuasiStar {
            (
                Color::srgba(0.28, 0.04, 0.08, 0.95),
                Color::srgb(1.0, 0.35, 0.45),
            )
        } else if lower.contains("red") || lower.contains("trappist") || lower.contains("16b") {
            (
                Color::srgba(0.28, 0.08, 0.04, 0.9),
                Color::srgb(1.0, 0.45, 0.3),
            )
        } else {
            (
                Color::srgba(0.28, 0.20, 0.04, 0.9),
                Color::srgb(1.0, 0.8, 0.2),
            )
        }
    } else if lower.contains("hz")
        || lower.contains("earth")
        || lower.contains("habitable")
        || lower.contains("1e")
        || lower.contains("1f")
        || lower.contains("1g")
    {
        (
            Color::srgba(0.04, 0.22, 0.16, 0.9),
            Color::srgb(0.3, 0.9, 0.6),
        )
    } else if lower.contains("rogue") || lower.contains("nemesis") {
        (
            Color::srgba(0.28, 0.05, 0.12, 0.95),
            Color::srgb(1.0, 0.3, 0.5),
        )
    } else if lower.contains("pluto") {
        (
            Color::srgba(0.10, 0.14, 0.22, 0.9),
            Color::srgb(0.7, 0.85, 0.95),
        )
    } else if lower.contains("planet nine") || lower.contains("planet 9") {
        (
            Color::srgba(0.04, 0.12, 0.25, 0.9),
            Color::srgb(0.2, 0.75, 0.9),
        )
    } else if is_embryo_body(name, body_type) {
        (
            Color::srgba(0.06, 0.18, 0.10, 0.9),
            Color::srgb(0.3, 0.95, 0.65),
        )
    } else if lower.contains("hot jupiter")
        || body_type == BodyType::GasGiant
        || lower.contains("jupiter")
        || lower.contains("saturn")
    {
        (
            Color::srgba(0.20, 0.12, 0.06, 0.9),
            Color::srgb(0.95, 0.65, 0.3),
        )
    } else if body_type == BodyType::IceGiant
        || lower.contains("neptune")
        || lower.contains("uranus")
        || lower.contains("kuiper")
    {
        (
            Color::srgba(0.06, 0.14, 0.26, 0.9),
            Color::srgb(0.4, 0.7, 1.0),
        )
    } else if body_type == BodyType::Comet || lower.contains("comet") {
        (
            Color::srgba(0.05, 0.14, 0.24, 0.90),
            Color::srgb(0.4, 0.8, 1.0),
        )
    } else if body_type == BodyType::Asteroid
        || body_type == BodyType::Planetesimal
        || lower.contains("asteroid")
        || lower.contains("ceres")
        || lower.contains("vesta")
        || lower.contains("pallas")
    {
        (
            Color::srgba(0.18, 0.12, 0.08, 0.90),
            Color::srgb(0.85, 0.65, 0.35),
        )
    } else {
        (
            Color::srgba(0.08, 0.12, 0.18, 0.9),
            Color::srgb(0.5, 0.7, 0.85),
        )
    }
}

pub fn body_to_button_label_and_colors(
    name: &str,
    body_type: BodyType,
    is_star: bool,
) -> (String, Color, Color) {
    let label = body_to_button_label(name, body_type, is_star);
    let (bg, border) = body_to_button_colors(name, body_type, is_star);
    (label, bg, border)
}

#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
fn collect_embryo_and_minor_bodies(
    bodies: impl Iterator<Item = (Entity, CelestialBody, SimPosition, Mass, Radius, bool)>,
) -> (
    Vec<(Entity, String, BodyType, bool, f64)>,
    Vec<(Entity, String, BodyType, bool, f64)>,
) {
    let mut embryo_bodies = Vec::new();
    let mut minor_bodies = Vec::new();

    for (ent, body, pos, mass, _radius, is_star) in bodies {
        let dist = if pos.0.is_finite() {
            pos.0.length()
        } else {
            0.0
        };
        if is_embryo_body(&body.name, body.body_type) {
            embryo_bodies.push((ent, body.name.clone(), body.body_type, is_star, dist));
        } else if !is_major_body(&body.name, body.body_type, is_star, mass.0) {
            minor_bodies.push((ent, body.name.clone(), body.body_type, is_star, dist));
        }
    }

    embryo_bodies.sort_by(|a, b| a.4.partial_cmp(&b.4).unwrap_or(std::cmp::Ordering::Equal));
    minor_bodies.sort_by(|a, b| a.4.partial_cmp(&b.4).unwrap_or(std::cmp::Ordering::Equal));

    (embryo_bodies, minor_bodies)
}

fn populate_minimized_quick_bar(
    btn_row: &mut ChildSpawnerCommands,
    major_count: usize,
    embryo_count: usize,
    minor_count: usize,
) {
    let mut parts = vec![format!("🪐 Worlds ({major_count})")];
    if embryo_count > 0 {
        parts.push(format!("🌱 Embryos ({embryo_count})"));
    }
    if minor_count > 0 {
        parts.push(format!("☄️ Minor ({minor_count})"));
    }
    let label = format!("{} ▼ Expand [H]", parts.join(" | "));
    create_button(
        btn_row,
        UiButtonAction::ToggleMinimizeQuickBar,
        &label,
        Color::srgba(0.08, 0.14, 0.24, 0.90),
        Color::srgb(0.4, 0.8, 1.0),
    );
    create_button(
        btn_row,
        UiButtonAction::CycleTarget,
        "Cycle [Tab]",
        Color::srgba(0.12, 0.16, 0.26, 0.85),
        Color::srgb(0.5, 0.7, 1.0),
    );
}

fn populate_expanded_quick_bar(
    btn_row: &mut ChildSpawnerCommands,
    major_worlds: &[SystemWorld],
    embryo_bodies: &[(Entity, String, BodyType, bool, f64)],
    minor_bodies: &[(Entity, String, BodyType, bool, f64)],
    selected_entity: Option<Entity>,
    quick_bar_state: &QuickBarState,
) {
    create_button(
        btn_row,
        UiButtonAction::ToggleMinimizeQuickBar,
        "🗕 [H]",
        Color::srgba(0.16, 0.08, 0.12, 0.85),
        Color::srgb(0.9, 0.4, 0.6),
    );

    for world in major_worlds {
        let (raw_label, base_bg, base_border) =
            body_to_button_label_and_colors(&world.name, world.body_type, world.is_central_star);

        let button_label = if world.is_central_star {
            format!("[0] {raw_label}")
        } else {
            format!("[{}] {raw_label}", world.index)
        };

        let is_selected = selected_entity == Some(world.entity);
        let bg_color = if is_selected {
            Color::srgba(0.20, 0.45, 0.85, 0.95)
        } else {
            base_bg
        };
        let border_color = if is_selected {
            Color::srgb(1.0, 1.0, 1.0)
        } else {
            base_border
        };

        create_button(
            btn_row,
            UiButtonAction::SelectEntity(world.entity),
            &button_label,
            bg_color,
            border_color,
        );
    }

    if !embryo_bodies.is_empty() {
        if quick_bar_state.show_embryos {
            for (ent, name, b_type, is_star, _) in embryo_bodies {
                let (raw_label, base_bg, base_border) =
                    body_to_button_label_and_colors(name, *b_type, *is_star);
                let is_selected = selected_entity == Some(*ent);
                let bg_color = if is_selected {
                    Color::srgba(0.20, 0.45, 0.85, 0.95)
                } else {
                    base_bg
                };
                let border_color = if is_selected {
                    Color::srgb(1.0, 1.0, 1.0)
                } else {
                    base_border
                };
                create_button(
                    btn_row,
                    UiButtonAction::SelectEntity(*ent),
                    &raw_label,
                    bg_color,
                    border_color,
                );
            }
            create_button(
                btn_row,
                UiButtonAction::ToggleEmbryos,
                "🌱 Hide Embryos",
                Color::srgba(0.12, 0.08, 0.16, 0.85),
                Color::srgb(0.8, 0.5, 0.9),
            );
        } else {
            let label = format!("🌱 +{} Embryos", embryo_bodies.len());
            create_button(
                btn_row,
                UiButtonAction::ToggleEmbryos,
                &label,
                Color::srgba(0.06, 0.14, 0.10, 0.85),
                Color::srgb(0.3, 0.8, 0.5),
            );
        }
    }

    if !minor_bodies.is_empty() {
        if quick_bar_state.show_minor_bodies {
            for (ent, name, b_type, is_star, _) in minor_bodies {
                let (raw_label, base_bg, base_border) =
                    body_to_button_label_and_colors(name, *b_type, *is_star);
                let is_selected = selected_entity == Some(*ent);
                let bg_color = if is_selected {
                    Color::srgba(0.20, 0.45, 0.85, 0.95)
                } else {
                    base_bg
                };
                let border_color = if is_selected {
                    Color::srgb(1.0, 1.0, 1.0)
                } else {
                    base_border
                };
                create_button(
                    btn_row,
                    UiButtonAction::SelectEntity(*ent),
                    &raw_label,
                    bg_color,
                    border_color,
                );
            }
            create_button(
                btn_row,
                UiButtonAction::ToggleMinorBodies,
                "☄️ Hide Minor",
                Color::srgba(0.12, 0.08, 0.16, 0.85),
                Color::srgb(0.8, 0.5, 0.9),
            );
        } else {
            let label = format!("☄️ +{} Minor Bodies", minor_bodies.len());
            create_button(
                btn_row,
                UiButtonAction::ToggleMinorBodies,
                &label,
                Color::srgba(0.10, 0.08, 0.14, 0.85),
                Color::srgb(0.65, 0.5, 0.85),
            );
        }
    }

    create_button(
        btn_row,
        UiButtonAction::CycleTarget,
        "Cycle [Tab]",
        Color::srgba(0.12, 0.16, 0.26, 0.85),
        Color::srgb(0.5, 0.7, 1.0),
    );
}

pub fn update_quick_body_selector_bar(
    mut commands: Commands,
    bar_query: Query<Entity, With<QuickBodySelectorBar>>,
    bodies_query: Query<
        (
            Entity,
            &CelestialBody,
            &SimPosition,
            &Mass,
            &Radius,
            Option<&CentralStar>,
        ),
        With<CelestialBody>,
    >,
    mut quick_bar_state: ResMut<QuickBarState>,
    player_state: Res<PlayerInteractionState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut last_state: Local<(Vec<Entity>, usize, usize, Option<Entity>, bool, bool, bool)>,
) {
    let Ok(bar_ent) = bar_query.single() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyH) {
        quick_bar_state.is_minimized = !quick_bar_state.is_minimized;
    }

    let (embryo_bodies, minor_bodies) =
        collect_embryo_and_minor_bodies(bodies_query.iter().map(|(e, b, p, m, r, s)| {
            (
                e,
                b.clone(),
                *p,
                *m,
                *r,
                s.is_some() || b.body_type.is_star_or_remnant(),
            )
        }));

    let major_worlds = collect_sorted_system_worlds(bodies_query.iter());
    let current_entities: Vec<Entity> = major_worlds.iter().map(|b| b.entity).collect();
    let current_state = (
        current_entities,
        embryo_bodies.len(),
        minor_bodies.len(),
        player_state.selected_entity,
        quick_bar_state.is_minimized,
        quick_bar_state.show_embryos,
        quick_bar_state.show_minor_bodies,
    );

    if *last_state == current_state && !major_worlds.is_empty() {
        return;
    }

    *last_state = current_state;

    commands.entity(bar_ent).despawn_children();
    commands.entity(bar_ent).with_children(|btn_row| {
        if quick_bar_state.is_minimized {
            populate_minimized_quick_bar(
                btn_row,
                major_worlds.len(),
                embryo_bodies.len(),
                minor_bodies.len(),
            );
        } else {
            populate_expanded_quick_bar(
                btn_row,
                &major_worlds,
                &embryo_bodies,
                &minor_bodies,
                player_state.selected_entity,
                &quick_bar_state,
            );
        }
    });
}

fn compute_builder_spawn_coords(
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
        let angle = rng.random_range(0.0..(2.0 * PI));
        let pos = DVec3::new(a * angle.cos(), 0.0, a * angle.sin());
        let v_circ = (G_ASTRO * star_mass / a).sqrt();
        let vel = DVec3::new(-v_circ * angle.sin(), 0.0, v_circ * angle.cos());
        (pos, vel)
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
    let radius_au = ((3.0 * builder_state.mass_solar / avg_density) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.2);

    let body_type = crate::simulation::components::classify_body_by_mass_and_comp(
        builder_state.mass_solar,
        &comp,
        false,
    );
    let temp_k = match body_type {
        BodyType::RedDwarf => 3000.0,
        BodyType::BrownDwarf => 1400.0,
        BodyType::GasGiant => 160.0,
        BodyType::IceGiant => 80.0,
        BodyType::SuperEarth => 295.0,
        _ => 288.0,
    };

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
