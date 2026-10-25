//! Tools for direct manipulation of celestial bodies: mass injection, orbit edits, ignition, and rings/oceans.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::{PlayerInteractionState, PlayerTool};
use crate::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU, G_ASTRO};

pub type SelectedBodyQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Mass,
        &'static mut Radius,
        &'static mut Temperature,
        &'static mut SimPosition,
        &'static mut SimVelocity,
        &'static mut Composition,
        &'static mut CelestialBody,
        Option<&'static mut InternalDifferentiation>,
        Option<&'static mut Transform>,
        Option<&'static mut IgnitionState>,
        Option<&'static mut BlackHoleStarState>,
        Option<&'static CentralStar>,
    ),
    (
        Without<PanOrbitCamera>,
        Without<crate::simulation::probes::SpaceProbe>,
    ),
>;

fn apply_body_mass_and_orbit_edits(
    keyboard: &ButtonInput<KeyCode>,
    star_mass: f64,
    star_pos: DVec3,
    star_vel: DVec3,
    mass: &mut Mass,
    radius: &mut Radius,
    pos: &mut SimPosition,
    vel: &mut SimVelocity,
    comp: &Composition,
    body: &CelestialBody,
    mut diff_opt: Option<&mut InternalDifferentiation>,
    mut trans_opt: Option<&mut Transform>,
) {
    if keyboard.just_pressed(KeyCode::KeyU)
        || keyboard.just_pressed(KeyCode::Equal)
        || keyboard.just_pressed(KeyCode::NumpadAdd)
    {
        mass.0 *= 1.25;
        if body.body_type == BodyType::BlackHole {
            radius.0 = (1.974e-8 * mass.0).max(1e-7);
        } else if !body.body_type.is_star_or_remnant() {
            let avg_density = comp.average_density();
            radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.1);
        }
        if let Some(ref mut diff) = diff_opt {
            diff.recalculate(mass.0, radius.0, comp);
        }
        if let Some(ref mut trans) = trans_opt {
            let scale = (radius.0 as f32 * 50.0).clamp(0.02, 1.5);
            trans.scale = Vec3::splat(scale);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyJ)
        || keyboard.just_pressed(KeyCode::Minus)
        || keyboard.just_pressed(KeyCode::NumpadSubtract)
    {
        mass.0 = (mass.0 * 0.8).max(1e-7 * EARTH_MASS_SOLAR);
        if body.body_type == BodyType::BlackHole {
            radius.0 = (1.974e-8 * mass.0).max(1e-7);
        } else if !body.body_type.is_star_or_remnant() {
            let avg_density = comp.average_density();
            radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.1);
        }
        if let Some(ref mut diff) = diff_opt {
            diff.recalculate(mass.0, radius.0, comp);
        }
        if let Some(ref mut trans) = trans_opt {
            let scale = (radius.0 as f32 * 50.0).clamp(0.02, 1.5);
            trans.scale = Vec3::splat(scale);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyO) && !body.body_type.is_star_or_remnant() {
        let rel_pos = (pos.0 - star_pos) * 1.10;
        pos.0 = star_pos + rel_pos;
        let r = rel_pos.length().max(0.1);
        let v_circ = (G_ASTRO * star_mass / r).sqrt();
        let phi = rel_pos.z.atan2(rel_pos.x);
        vel.0 = star_vel + DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
    }
    if keyboard.just_pressed(KeyCode::KeyL) && !body.body_type.is_star_or_remnant() {
        let rel_pos = ((pos.0 - star_pos) * 0.90).clamp_length_min(0.25);
        pos.0 = star_pos + rel_pos;
        let r = rel_pos.length().max(0.1);
        let v_circ = (G_ASTRO * star_mass / r).sqrt();
        let phi = rel_pos.z.atan2(rel_pos.x);
        vel.0 = star_vel + DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
    }
}

pub fn trigger_star_ignition_or_flare(
    commands: &mut Commands,
    star_ent: Entity,
    body: &mut CelestialBody,
    ignition: &mut IgnitionState,
    shift: bool,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if shift {
        commands
            .entity(star_ent)
            .insert(crate::simulation::space_weather::StellarFlareState {
                flare_frequency: 1.5,
                current_flare_intensity: 6.5,
                flare_decay_timer_years: 0.15,
                cme_front_radius_au: 0.05,
                cme_speed_au_day: 0.35,
                cme_density_multiplier: 25.0,
                cme_active: true,
            });
        ignition.shockwave_radius = 0.1;
        toast.message = "☀️ CME Coronal Mass Ejection Erupted! Expanding Shockwave!".to_string();
    } else if ignition.is_ignited {
        ignition.shockwave_radius = 1.6;
        toast.message = "☀️ Coronal Mass Ejection & Solar Blast Triggered!".to_string();
    } else {
        ignition.core_temperature = 1.0e7;
        ignition.is_ignited = true;
        ignition.fusion_fraction = 1.0;
        ignition.shockwave_radius = 1.6;
        body.body_type = BodyType::MainSequenceStar;
        if body.name.contains("Genesis") {
            body.name = "Genesis Star (Main Sequence)".to_string();
        } else {
            body.name = "The Star (Main Sequence)".to_string();
        }
        toast.message =
            "⭐ Hydrogen Core Fusion Ignited! Solar Wind Shockwave Sweeping the System!"
                .to_string();
    }
    toast.timer = 5.0;
}

fn handle_star_interaction_actions(
    commands: &mut Commands,
    keyboard: &ButtonInput<KeyCode>,
    entity: Entity,
    body: &mut CelestialBody,
    mut ignition_opt: Option<&mut IgnitionState>,
    opt_quasi: Option<&mut BlackHoleStarState>,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if keyboard.just_pressed(KeyCode::KeyI) {
        let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        if let Some(ref mut ignition) = ignition_opt {
            trigger_star_ignition_or_flare(commands, entity, body, ignition, shift, toast);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyN) && body.body_type.is_star_or_remnant() {
        if let Some(ref mut ignition) = ignition_opt {
            if !ignition.is_ignited {
                ignition.core_temperature = 1.0e7;
                ignition.is_ignited = true;
                ignition.fusion_fraction = 1.0;
                ignition.shockwave_radius = 1.6;
                body.body_type = BodyType::MainSequenceStar;
                body.name = "The Star (Main Sequence)".to_string();
            }
        }
    }
    if let Some(qs) = opt_quasi {
        if keyboard.just_pressed(KeyCode::KeyX) {
            qs.toggle_super_eddington();
            let mode = if qs.super_eddington_active {
                "4.5x Eddington (Hyper-Accretion Active)"
            } else {
                "0.9x Eddington (Sub-Eddington Normal)"
            };
            toast.message = format!("⚡ Inflow Rate: {mode} on {}", body.name);
            toast.timer = 4.5;
        }
        if keyboard.just_pressed(KeyCode::KeyB) {
            qs.trigger_blowout();
            toast.message =
                "💥 COCOON BLOWOUT: Radiation pressure shedding hydrogen envelope into Quasar!"
                    .to_string();
            toast.timer = 6.0;
        }
    }
}

fn handle_planet_hydrosphere_hotkeys(
    commands: &mut Commands,
    keyboard: &ButtonInput<KeyCode>,
    entity: Entity,
    comp: &mut Composition,
    body_name: &str,
    toast: &mut crate::game::ui::NotificationToast,
) {
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if keyboard.just_pressed(KeyCode::BracketRight) {
        let new_ocean_cov = (comp.ice_frac as f32 / 0.08 + 0.10).clamp(0.0, 1.0);
        let new_water_m_earth = f64::from(new_ocean_cov * 0.0006);
        comp.ice_frac = (f64::from(new_ocean_cov) * 0.08).clamp(0.0, 0.45);
        *comp = comp.normalized();
        if let Ok(mut cmd) = commands.get_entity(entity) {
            cmd.insert((
                VolatileInventory {
                    delivered_water_m_earth: new_water_m_earth,
                    ocean_coverage_frac: new_ocean_cov,
                    atmospheric_pressure_bar: 1.0,
                    cometary_impact_count: 5,
                },
                PlanetaryClimate {
                    cloud_coverage_frac: (0.25 + new_ocean_cov * 0.40).clamp(0.05, 0.90),
                    ..default()
                },
            ));
        }
        toast.message = format!(
            "🌊 SEA LEVEL EXPANDED // \"{body_name}\" Sea Level: {:.0}% (+10% | Lowlands Flooded)",
            new_ocean_cov * 100.0
        );
        toast.timer = 4.0;
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        if shift {
            comp.ice_frac = 0.0;
            *comp = comp.normalized();
            if let Ok(mut cmd) = commands.get_entity(entity) {
                cmd.insert((
                    VolatileInventory {
                        delivered_water_m_earth: 0.0,
                        ocean_coverage_frac: 0.0,
                        atmospheric_pressure_bar: 1.0,
                        cometary_impact_count: 5,
                    },
                    PlanetaryClimate {
                        cloud_coverage_frac: 0.05,
                        albedo: 0.18,
                        ..default()
                    },
                ));
            }
            toast.message = format!(
                "🏜️ BASIN DESICCATED // \"{body_name}\" Sea Level: 0% (Continental Shelves, Mid-Ocean Ridges & Trenches Exposed)"
            );
            toast.timer = 4.0;
        } else {
            let new_ocean_cov = (comp.ice_frac as f32 / 0.08 - 0.10).clamp(0.0, 1.0);
            let new_water_m_earth = f64::from(new_ocean_cov * 0.0006);
            comp.ice_frac = (f64::from(new_ocean_cov) * 0.08).clamp(0.0, 0.45);
            *comp = comp.normalized();
            if let Ok(mut cmd) = commands.get_entity(entity) {
                cmd.insert((
                    VolatileInventory {
                        delivered_water_m_earth: new_water_m_earth,
                        ocean_coverage_frac: new_ocean_cov,
                        atmospheric_pressure_bar: 1.0,
                        cometary_impact_count: 5,
                    },
                    PlanetaryClimate {
                        cloud_coverage_frac: (0.25 + new_ocean_cov * 0.40).clamp(0.05, 0.90),
                        ..default()
                    },
                ));
            }
            toast.message = format!(
                "🏝️ OCEANS DRAINED // \"{body_name}\" Sea Level: {:.0}% (-10% | Continental Margins Exposed)",
                new_ocean_cov * 100.0
            );
            toast.timer = 4.0;
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Dispatches planet direct manipulation and orbital adjustment tools"
)]
fn handle_planet_interaction_actions(
    commands: &mut Commands,
    keyboard: &ButtonInput<KeyCode>,
    star_mass: f64,
    star_pos: DVec3,
    star_vel: DVec3,
    entity: Entity,
    pos: &mut SimPosition,
    vel: &mut SimVelocity,
    comp: &mut Composition,
    body_name: &str,
    player_state: &mut PlayerInteractionState,
    toast: &mut crate::game::ui::NotificationToast,
) {
    handle_planet_hydrosphere_hotkeys(commands, keyboard, entity, comp, body_name, toast);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if keyboard.just_pressed(KeyCode::KeyB) && !shift {
        let speed = vel.0.length();
        if speed > 0.0 {
            vel.0 += (vel.0 / speed) * (speed * 0.15);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyB) && shift {
        let speed = vel.0.length();
        if speed > 0.0 {
            vel.0 -= (vel.0 / speed) * (speed * 0.15);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        let mut rel_pos = pos.0 - star_pos;
        rel_pos.y = 0.0;
        pos.0 = star_pos + rel_pos;
        let r_cyl = (rel_pos.x * rel_pos.x + rel_pos.z * rel_pos.z)
            .sqrt()
            .max(0.1);
        let v_circ = (G_ASTRO * star_mass / r_cyl).sqrt();
        let phi = rel_pos.z.atan2(rel_pos.x);
        vel.0 = star_vel + DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
    }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        if let Ok(mut cmd) = commands.get_entity(entity) {
            cmd.try_despawn();
        }
        player_state.selected_entity = None;
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        if let Ok(mut cmd) = commands.get_entity(entity) {
            cmd.insert(PlanetaryRingSystem {
                inner_radius_au: 0.0008,
                outer_radius_au: 0.0028,
                ring_mass_earth: 0.0002,
                optical_depth: 0.88,
                ice_fraction: 0.95,
                silicate_fraction: 0.05,
            });
        }
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        if let Ok(mut cmd) = commands.get_entity(entity) {
            cmd.insert((
                VolatileInventory {
                    delivered_water_m_earth: 0.002,
                    ocean_coverage_frac: 0.70,
                    atmospheric_pressure_bar: 1.0,
                    cometary_impact_count: 12,
                },
                BiosphereState {
                    habitability_score: 0.95,
                    biomass_coverage_frac: 0.65,
                    oxygen_fraction: 0.21,
                    emergence_year: Some(0.0),
                    technosignature: 0.0,
                    bioluminescence_intensity: 0.25,
                    ..Default::default()
                },
                PlanetaryClimate {
                    surface_temperature_k: 288.0,
                    equilibrium_temperature_k: 255.0,
                    greenhouse_delta_k: 33.0,
                    albedo: 0.30,
                    ice_coverage_frac: 0.10,
                    cloud_coverage_frac: 0.55,
                    climate_regime: ClimateRegime::TemperateHabitable,
                    polar_ice_cap_latitude_deg: 75.0,
                },
            ));
            comp.ice_frac = 0.08;
            comp.gas_frac = 0.02;
        }
    }
}

fn apply_body_interaction_actions(
    commands: &mut Commands,
    keyboard: &ButtonInput<KeyCode>,
    star_mass: f64,
    star_pos: DVec3,
    star_vel: DVec3,
    entity: Entity,
    mass: &Mass,
    radius: &mut Radius,
    pos: &mut SimPosition,
    vel: &mut SimVelocity,
    comp: &mut Composition,
    body: &mut CelestialBody,
    ignition_opt: Option<&mut IgnitionState>,
    opt_quasi: Option<&mut BlackHoleStarState>,
    player_state: &mut PlayerInteractionState,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if keyboard.just_pressed(KeyCode::KeyC) && player_state.active_tool != PlayerTool::Slingshot {
        *comp = comp.cycle_next_composition();
        if !body.body_type.is_star_or_remnant() {
            let avg_density = comp.average_density();
            radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.1);
        }
    }
    if body.body_type.is_star_or_remnant() || opt_quasi.is_some() {
        handle_star_interaction_actions(
            commands,
            keyboard,
            entity,
            body,
            ignition_opt,
            opt_quasi,
            toast,
        );
    } else {
        handle_planet_interaction_actions(
            commands,
            keyboard,
            star_mass,
            star_pos,
            star_vel,
            entity,
            pos,
            vel,
            comp,
            &body.name,
            player_state,
            toast,
        );
    }
}

pub fn handle_unselected_star_ignition_hotkey(
    commands: &mut Commands,
    keyboard: &ButtonInput<KeyCode>,
    selected_query: &mut SelectedBodyQuery,
    is_star_selected: bool,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if keyboard.just_pressed(KeyCode::KeyI) && !is_star_selected {
        let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        if let Some((star_ent, _, _, _, _, _, _, mut body, _, _, Some(mut ignition), _, _)) =
            selected_query
                .iter_mut()
                .find(|(_, _, _, _, _, _, _, body, _, _, _, _, opt_star)| {
                    opt_star.is_some() || body.body_type.is_star_or_remnant()
                })
        {
            trigger_star_ignition_or_flare(
                commands,
                star_ent,
                &mut body,
                &mut ignition,
                shift,
                toast,
            );
        }
    }
}

pub fn handle_selected_body_tools(
    commands: &mut Commands,
    keyboard: &ButtonInput<KeyCode>,
    star_mass: f64,
    star_pos: DVec3,
    star_vel: DVec3,
    player_state: &mut PlayerInteractionState,
    selected_query: &mut SelectedBodyQuery,
    toast: &mut crate::game::ui::NotificationToast,
) {
    if let Some(selected_ent) = player_state.selected_entity {
        if let Ok((
            entity,
            mut mass,
            mut radius,
            mut _temp,
            mut pos,
            mut vel,
            mut comp,
            mut body,
            mut diff_opt,
            mut trans_opt,
            mut ignition_opt,
            mut opt_quasi,
            _opt_star,
        )) = selected_query.get_mut(selected_ent)
        {
            apply_body_mass_and_orbit_edits(
                keyboard,
                star_mass,
                star_pos,
                star_vel,
                &mut mass,
                &mut radius,
                &mut pos,
                &mut vel,
                &comp,
                &body,
                diff_opt.as_deref_mut(),
                trans_opt.as_deref_mut(),
            );
            apply_body_interaction_actions(
                commands,
                keyboard,
                star_mass,
                star_pos,
                star_vel,
                entity,
                &mass,
                &mut radius,
                &mut pos,
                &mut vel,
                &mut comp,
                &mut body,
                ignition_opt.as_deref_mut(),
                opt_quasi.as_deref_mut(),
                player_state,
                toast,
            );
        }
    }
}
