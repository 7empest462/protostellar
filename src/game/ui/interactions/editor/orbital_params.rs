//! Parameter manipulation for mass, orbits, composition, velocity, and camera view targeting.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::game::ui::types::*;
use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

pub fn handle_mass_change(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    toast: &mut NotificationToast,
) -> bool {
    let multiplier = match action {
        UiButtonAction::IncreaseMass => 1.25,
        UiButtonAction::DecreaseMass => 0.80,
        _ => return false,
    };

    let Some(ent) = player_state.selected_entity else {
        toast.message = "⚠️ Please select a celestial body first!".to_string();
        toast.timer = 3.0;
        return true;
    };

    let Ok((_, mut mass, mut radius, _, _, comp, mut body, is_star, ..)) =
        selected_query.get_mut(ent)
    else {
        return true;
    };

    mass.0 *= multiplier;
    if body.body_type == BodyType::BlackHole {
        radius.0 = (1.974e-8 * mass.0).max(1e-7);
    } else if is_star.is_none() && !body.body_type.is_star_or_remnant() {
        let avg_density = comp.average_density();
        radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
            .cbrt()
            .max(EARTH_RADIUS_AU * 0.1);
    }

    let old_type = body.body_type;
    let new_type = classify_body_by_mass_and_comp(
        mass.0,
        &comp,
        is_star.is_some() || body.body_type.is_star_or_remnant(),
    );

    if is_star.is_some() || body.body_type.is_star_or_remnant() {
        if !body.body_type.is_remnant() && !body.body_type.is_hypothetical() {
            body.body_type = new_type;
            body.name = match new_type {
                BodyType::BrownDwarf => "The Star (Brown Dwarf)".to_string(),
                BodyType::RedDwarf => "The Star (Red Dwarf - M Type)".to_string(),
                BodyType::OrangeDwarf => "The Star (Orange Dwarf - K Type)".to_string(),
                BodyType::YellowDwarf => "The Star (Yellow Dwarf - G2V)".to_string(),
                BodyType::YellowWhiteDwarf => "The Star (Yellow-White Dwarf - F Type)".to_string(),
                BodyType::WhiteMainSequence => {
                    "The Star (White Main-Sequence - A Type)".to_string()
                }
                BodyType::BlueGiant => "The Star (Blue Giant - B Type)".to_string(),
                BodyType::BlueSupergiant => "The Star (Blue Supergiant - O Type)".to_string(),
                BodyType::Hypergiant => "The Star (Luminous Hypergiant)".to_string(),
                BodyType::PopulationIIIStar => "The Star (Population III Hypergiant)".to_string(),
                _ => body.name.clone(),
            };
        }
    } else if new_type != old_type {
        body.body_type = new_type;
        if new_type.is_planet() && !body.name.starts_with("Planet") {
            body.name = format!("Planet ({new_type:?})");
        }
    }

    let sign = if multiplier > 1.0 { "➕" } else { "➖" };
    let pct = if multiplier > 1.0 { "+25%" } else { "-20%" };
    toast.message = format!(
        "{sign} {} Mass Altered: {:.3} M⊕ ({pct} | {:?})",
        body.name,
        mass.0 / EARTH_MASS_SOLAR,
        body.body_type
    );
    toast.timer = 4.0;
    true
}

pub fn handle_orbital_adjustment(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    star_mass: f64,
    toast: &mut NotificationToast,
) -> bool {
    let (mult, label) = match action {
        UiButtonAction::ExpandOrbit => (1.10, "Expanded to"),
        UiButtonAction::ContractOrbit => (0.90, "Contracted to"),
        _ => return false,
    };

    if let Some(ent) = player_state.selected_entity {
        if let Ok((_, _, _, mut pos, mut vel, _, body, ..)) = selected_query.get_mut(ent) {
            pos.0 *= mult;
            let r = pos.0.length();
            let v_mag = (G_ASTRO * star_mass / r).sqrt();
            let mut orbit_tangent = DVec3::new(-pos.0.z, 0.0, pos.0.x);
            if orbit_tangent.length_squared() > 1e-6 {
                orbit_tangent = orbit_tangent.normalize();
            }
            vel.0 = orbit_tangent * v_mag;
            toast.message = format!("🚀 {} Orbit {} {:.2} AU", body.name, label, r);
            toast.timer = 4.0;
        }
    }
    true
}

pub fn handle_composition_cycle(
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    toast: &mut NotificationToast,
) {
    if let Some(ent) = player_state.selected_entity {
        if let Ok((_, mut mass, mut radius, _, _, mut comp, body, ..)) = selected_query.get_mut(ent)
        {
            if comp.silicate_frac > 0.5 {
                *comp = Composition::icy();
                toast.message = format!(
                    "🎨 {} Material -> Icy Mixture (Density: 1.2 g/cm³)",
                    body.name
                );
            } else if comp.ice_frac > 0.5 {
                *comp = Composition::solar_gas();
                mass.0 *= 4.0;
                toast.message = format!(
                    "🎨 {} Material -> Gas Giant Envelope (Density: 0.8 g/cm³)",
                    body.name
                );
            } else if comp.gas_frac > 0.5 {
                *comp = Composition::metal_rich();
                toast.message = format!(
                    "🎨 {} Material -> Metal-Rich Core (Density: 7.8 g/cm³)",
                    body.name
                );
            } else {
                *comp = Composition::rocky();
                toast.message = format!(
                    "🎨 {} Material -> Rocky Silicate (Density: 3.9 g/cm³)",
                    body.name
                );
            }
            let avg_density = comp.average_density();
            radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.1);
            toast.timer = 4.0;
        }
    }
}

pub fn handle_velocity_burn_and_fix(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    star_mass: f64,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::CycleComposition => {
            handle_composition_cycle(selected_query, player_state, toast);
            true
        }
        UiButtonAction::BoostDeltaV => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok((_, _, _, _, mut vel, _, body, ..)) = selected_query.get_mut(ent) {
                    vel.0 *= 1.15;
                    toast.message =
                        format!("⚡ {} Prograde Delta-V Boost (+15% Velocity)", body.name);
                    toast.timer = 4.0;
                }
            }
            true
        }
        UiButtonAction::BrakeDeltaV => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok((_, _, _, _, mut vel, _, body, ..)) = selected_query.get_mut(ent) {
                    vel.0 *= 0.85;
                    toast.message =
                        format!("⚡ {} Retrograde Delta-V Brake (-15% Velocity)", body.name);
                    toast.timer = 4.0;
                }
            }
            true
        }
        UiButtonAction::FixOrbit => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok((_, _, _, mut pos, mut vel, _, body, ..)) = selected_query.get_mut(ent) {
                    if !body.body_type.is_star_or_remnant() {
                        let r_cyl = (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt().max(0.1);
                        let v_circ = (G_ASTRO * star_mass / r_cyl).sqrt();
                        let phi = pos.0.z.atan2(pos.0.x);
                        vel.0 = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
                        pos.0.y = 0.0;
                        toast.message =
                            format!("🪐 {} Orbit Circularized & Stabilized (e = 0.0)", body.name);
                        toast.timer = 4.0;
                    }
                }
            }
            true
        }
        _ => false,
    }
}

pub fn handle_view_and_target(
    action: &UiButtonAction,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    selected_query: &mut SelectedWorldQuery,
    commands: &mut Commands,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::FocusLock => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok(mut cam) = camera_query.single_mut() {
                    cam.surface_horizon_mode = false;
                    cam.target_entity = Some(ent);
                }
                toast.message = "🎯 Camera Focus Locked to Selected Target".to_string();
                toast.timer = 3.5;
            }
            true
        }
        UiButtonAction::ToggleSurfaceHorizonCamera => {
            if let Ok(mut cam) = camera_query.single_mut() {
                crate::rendering::surface_camera::toggle_surface_horizon_mode(
                    &mut cam,
                    player_state.selected_entity,
                    toast,
                );
            }
            true
        }
        UiButtonAction::ToggleInternalCutaway => {
            player_state.cutaway_mode = !player_state.cutaway_mode;
            let status = if player_state.cutaway_mode {
                "ENABLED (90° Core Cross-Section & Geodynamics Active)"
            } else {
                "DISABLED (Solid Surface View Restored)"
            };
            toast.message = format!("🔬 3D Planetary Cutaway {status}");
            toast.timer = 3.5;
            true
        }
        UiButtonAction::ResetView => {
            if let Ok(mut cam) = camera_query.single_mut() {
                cam.surface_horizon_mode = false;
                cam.target_focus = Vec3::ZERO;
                cam.target_entity = None;
                cam.target_radius = 45.0;
            }
            player_state.selected_entity = None;
            toast.message = "☀️ Camera Reset to Overview".to_string();
            toast.timer = 3.5;
            true
        }
        UiButtonAction::DeselectBody => {
            player_state.selected_entity = None;
            if let Ok(mut cam) = camera_query.single_mut() {
                cam.surface_horizon_mode = false;
                cam.target_entity = None;
            }
            toast.message = "❌ Closed Inspector & Deselected Target".to_string();
            toast.timer = 2.5;
            true
        }
        UiButtonAction::VaporizeBody => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok((_, _, _, _, _, _, body, ..)) = selected_query.get(ent) {
                    toast.message = format!("💥 Vaporized {}", body.name);
                    toast.timer = 4.0;
                }
                if let Ok(mut cmd) = commands.get_entity(ent) {
                    cmd.try_despawn();
                }
                player_state.selected_entity = None;
                if let Ok(mut cam) = camera_query.single_mut() {
                    cam.target_entity = None;
                }
            }
            true
        }
        _ => false,
    }
}
