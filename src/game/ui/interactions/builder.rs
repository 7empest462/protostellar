//! Planet Builder parameter tweaks and spawning actions.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::super::selector::spawn_custom_builder_world;
use super::super::types::*;

fn cycle_builder_composition(builder_state: &mut PlanetBuilderState) {
    if builder_state.gas_frac > 0.4 {
        builder_state.rock_frac = 0.70;
        builder_state.ice_frac = 0.05;
        builder_state.metal_frac = 0.25;
        builder_state.gas_frac = 0.0;
    } else if builder_state.ice_frac > 0.4 {
        builder_state.rock_frac = 0.10;
        builder_state.ice_frac = 0.05;
        builder_state.metal_frac = 0.85;
        builder_state.gas_frac = 0.0;
    } else if builder_state.metal_frac > 0.5 {
        builder_state.rock_frac = 0.02;
        builder_state.ice_frac = 0.02;
        builder_state.metal_frac = 0.01;
        builder_state.gas_frac = 0.95;
    } else {
        builder_state.rock_frac = 0.30;
        builder_state.ice_frac = 0.60;
        builder_state.metal_frac = 0.10;
        builder_state.gas_frac = 0.0;
    }
}

fn spawn_sub_roche_moon(
    commands: &mut Commands,
    player_state: &mut PlayerInteractionState,
    selected_query: &SelectedWorldQuery,
    toast: &mut NotificationToast,
) {
    let mut target_candidate = None;

    if let Some(sel_ent) = player_state.selected_entity {
        if let Ok((ent, m, rad, pos, vel, comp, body, opt_star, _, _, _, _)) =
            selected_query.get(sel_ent)
        {
            if opt_star.is_none()
                && !body.body_type.is_star_or_remnant()
                && m.0 >= EARTH_MASS_SOLAR * 0.01
            {
                target_candidate = Some((ent, m.0, rad.0, pos.0, vel.0, *comp, body.name.clone()));
            }
        }
    }

    if target_candidate.is_none() {
        let mut best_m = 0.0;
        for (ent, m, rad, pos, vel, comp, body, opt_star, _, _, _, _) in selected_query.iter() {
            if opt_star.is_none() && !body.body_type.is_star_or_remnant() && m.0 > best_m {
                best_m = m.0;
                target_candidate = Some((ent, m.0, rad.0, pos.0, vel.0, *comp, body.name.clone()));
            }
        }
    }

    if let Some((target_ent, p_m, p_rad, p_pos, p_vel, p_comp, p_name)) = target_candidate {
        player_state.selected_entity = Some(target_ent);

        let p_density = p_comp.average_density();
        let moon_density = 0.95e-3;
        let d_roche = 2.44 * p_rad * (p_density / moon_density).cbrt();
        let r_moon = (d_roche * 0.85).max(p_rad * 1.35);

        let moon_pos = p_pos + DVec3::new(r_moon, 0.0, r_moon * 0.25);
        let v_circ = (G_ASTRO * p_m / r_moon.max(1e-5)).sqrt();
        let moon_vel = p_vel + DVec3::new(-v_circ * 0.35, 0.0, v_circ * 0.85);

        let moon_mass = (p_m * 0.003).clamp(EARTH_MASS_SOLAR * 0.0001, EARTH_MASS_SOLAR * 0.05);
        let moon_comp = Composition {
            silicate_frac: 0.15,
            ice_frac: 0.80,
            metal_frac: 0.05,
            organics_frac: 0.0,
            gas_frac: 0.0,
        };
        let moon_rad = ((3.0 * moon_mass / moon_comp.average_density()) / (4.0 * PI)).cbrt();

        commands.spawn((
            SimPosition(moon_pos),
            SimVelocity(moon_vel),
            SimAcceleration(DVec3::ZERO),
            Mass(moon_mass),
            Radius(moon_rad),
            Temperature(110.0),
            moon_comp,
            CelestialBody {
                name: format!("Sub-Roche Moon ({p_name})"),
                body_type: BodyType::Planetesimal,
            },
        ));

        toast.message = format!(
            "💥 Inserted Sub-Roche Icy Moon into {p_name}'s Roche Limit ({d_roche:.4} AU)!"
        );
        toast.timer = 5.0;
    } else {
        toast.message = "⚠️ Please select a planet or gas giant first!".to_string();
        toast.timer = 3.5;
    }
}

pub fn handle_builder_action(
    action: &UiButtonAction,
    builder_state: &mut PlanetBuilderState,
    toast: &mut NotificationToast,
    star_mass: f64,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    selected_query: &SelectedWorldQuery,
    commands: &mut Commands,
) -> bool {
    match action {
        UiButtonAction::TogglePlanetBuilder => {
            builder_state.is_open = !builder_state.is_open;
            toast.message = if builder_state.is_open {
                "🛠️ Planet Builder & Spawner Opened".to_string()
            } else {
                "🛠️ Planet Builder Closed".to_string()
            };
            toast.timer = 2.5;
            true
        }
        UiButtonAction::BuilderSelectPreset(preset) => {
            builder_state.apply_preset(*preset);
            toast.message = format!("📋 Loaded Preset: {}", preset.display_name());
            toast.timer = 2.5;
            true
        }
        UiButtonAction::BuilderMassStep(step) => {
            match *step {
                -2 => {
                    builder_state.mass_solar =
                        (builder_state.mass_solar * 0.1).max(EARTH_MASS_SOLAR * 0.001);
                }
                -1 => {
                    builder_state.mass_solar =
                        (builder_state.mass_solar * 0.5).max(EARTH_MASS_SOLAR * 0.001);
                }
                1 => builder_state.mass_solar = (builder_state.mass_solar * 2.0).min(5.0),
                2 => builder_state.mass_solar = (builder_state.mass_solar * 10.0).min(5.0),
                _ => {}
            }
            true
        }
        UiButtonAction::BuilderDistanceStep(step) => {
            match *step {
                -2 => {
                    builder_state.semi_major_axis_au =
                        (builder_state.semi_major_axis_au - 1.0).max(0.1);
                }
                -1 => {
                    builder_state.semi_major_axis_au =
                        (builder_state.semi_major_axis_au - 0.2).max(0.1);
                }
                1 => {
                    builder_state.semi_major_axis_au =
                        (builder_state.semi_major_axis_au + 0.2).min(80.0);
                }
                2 => {
                    builder_state.semi_major_axis_au =
                        (builder_state.semi_major_axis_au + 1.0).min(80.0);
                }
                _ => {}
            }
            true
        }
        UiButtonAction::BuilderCycleEccentricity => {
            builder_state.eccentricity = if builder_state.eccentricity < 0.05 {
                0.15
            } else if builder_state.eccentricity < 0.35 {
                0.60
            } else if builder_state.eccentricity < 0.90 {
                1.25
            } else {
                0.0
            };
            true
        }
        UiButtonAction::BuilderCycleComposition => {
            cycle_builder_composition(builder_state);
            true
        }
        UiButtonAction::BuilderToggleClickSpawn => {
            builder_state.click_to_spawn_mode = !builder_state.click_to_spawn_mode;
            toast.message = if builder_state.click_to_spawn_mode {
                "🎯 Click-in-3D Mode ON: Click anywhere in 3D disk to place world!".to_string()
            } else {
                "🚀 Click-in-3D Mode OFF: Using orbit button.".to_string()
            };
            toast.timer = 3.5;
            true
        }
        UiButtonAction::BuilderExecuteSpawn => {
            spawn_custom_builder_world(
                commands,
                builder_state,
                star_mass,
                None,
                player_state,
                camera_query,
                toast,
            );
            true
        }
        UiButtonAction::SpawnSubRocheMoon => {
            spawn_sub_roche_moon(commands, player_state, selected_query, toast);
            true
        }
        _ => false,
    }
}
