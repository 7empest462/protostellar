//! Tools for direct manipulation of celestial bodies: mass injection, orbit edits, ignition, and rings/oceans.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::{PlayerInteractionState, PlayerTool};
use crate::utils::constants::{EARTH_MASS_SOLAR, EARTH_RADIUS_AU, G_ASTRO, SOLAR_RADIUS_AU};

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
        Option<&'static SatelliteOf>,
    ),
    (
        Without<PanOrbitCamera>,
        Without<crate::simulation::probes::SpaceProbe>,
    ),
>;

fn equilibrate_moved_orbit_temperature(
    commands: &mut Commands,
    entity: Entity,
    r_au: f64,
    is_satellite: bool,
    mass: f64,
    radius: f64,
    body_type: BodyType,
    comp: &Composition,
    star_irrad: crate::simulation::thermodynamics::StellarIrradiance,
    temp: &mut Temperature,
) {
    if is_satellite || body_type.is_star_or_remnant() {
        return;
    }
    if let Some(updated_climate) =
        crate::simulation::thermodynamics::equilibrate_body_temperature_for_orbit(
            r_au, mass, radius, body_type, comp, star_irrad, temp, None, None,
        )
        && let Ok(mut cmd) = commands.get_entity(entity)
    {
        cmd.entry::<PlanetaryClimate>().and_modify(move |mut clim| {
            *clim = updated_climate;
        });
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Applies mass scaling and parent-relative orbit adjustments to the selected body"
)]
fn apply_body_mass_and_orbit_edits(
    commands: &mut Commands,
    keyboard: &ButtonInput<KeyCode>,
    entity: Entity,
    primary_mass: f64,
    primary_pos: DVec3,
    primary_vel: DVec3,
    is_satellite: bool,
    star_irrad: crate::simulation::thermodynamics::StellarIrradiance,
    mass: &mut Mass,
    radius: &mut Radius,
    temp: &mut Temperature,
    pos: &mut SimPosition,
    vel: &mut SimVelocity,
    comp: &Composition,
    body: &mut CelestialBody,
    mut diff_opt: Option<&mut InternalDifferentiation>,
    mut trans_opt: Option<&mut Transform>,
) {
    let is_shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let increase_mass = keyboard.just_pressed(KeyCode::KeyU)
        || (is_shift
            && (keyboard.just_pressed(KeyCode::Equal)
                || keyboard.just_pressed(KeyCode::NumpadAdd)));
    let decrease_mass = keyboard.just_pressed(KeyCode::KeyJ)
        || (is_shift
            && (keyboard.just_pressed(KeyCode::Minus)
                || keyboard.just_pressed(KeyCode::NumpadSubtract)));

    let mut mass_changed = false;
    if increase_mass {
        mass.0 *= 1.25;
        mass_changed = true;
    }
    if decrease_mass {
        mass.0 = (mass.0 * 0.8).max(1e-7 * EARTH_MASS_SOLAR);
        mass_changed = true;
    }

    if mass_changed {
        let old_type = body.body_type;
        let new_type = classify_body_by_mass_and_comp(mass.0, comp, old_type.is_star_or_remnant());
        if !old_type.is_remnant() && !old_type.is_hypothetical() {
            body.body_type = new_type;
        }
        if body.body_type == BodyType::BlackHole {
            radius.0 = (1.974e-8 * mass.0).max(1e-7);
        } else if body.body_type == BodyType::BrownDwarf {
            radius.0 = 0.11 * SOLAR_RADIUS_AU;
        } else if body.body_type.is_star_or_remnant() {
            if !body.body_type.is_remnant() && !body.body_type.is_hypothetical() {
                radius.0 = SOLAR_RADIUS_AU * mass.0.max(0.08).powf(0.8);
            }
        } else {
            let avg_density = comp.average_density();
            radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                .cbrt()
                .max(EARTH_RADIUS_AU * 0.1);
        }
        if !old_type.is_star_or_remnant()
            && body.body_type.is_star_or_remnant()
            && let Ok(mut cmd) = commands.get_entity(entity)
        {
            let is_bd = body.body_type == BodyType::BrownDwarf;
            let lum = if is_bd {
                1e-4 * (mass.0 / 0.05).clamp(0.1, 2.0)
            } else {
                mass.0.max(0.01).powf(3.5)
            };
            cmd.remove::<(PlanetaryClimate, BiosphereState)>().insert((
                Luminosity(lum),
                IgnitionState {
                    core_temperature: if is_bd { 2.5e6 } else { 1.5e7 },
                    fusion_fraction: if is_bd { 0.05 } else { 1.0 },
                    is_ignited: !is_bd,
                    shockwave_radius: 0.0,
                },
                StellarEvolutionState {
                    phase: if is_bd {
                        StellarEvolutionPhase::ProtostarContraction
                    } else {
                        StellarEvolutionPhase::MainSequence
                    },
                    ..default()
                },
            ));
        }
        if let Some(ref mut diff) = diff_opt {
            diff.recalculate(mass.0, radius.0, comp);
        }
        if let Some(ref mut trans) = trans_opt {
            let scale = (radius.0 as f32 * 50.0).clamp(0.02, 1.5);
            trans.scale = Vec3::splat(scale);
        }
    }

    let min_orbit_r = if is_satellite { 1e-4 } else { 0.25 };
    let mu_mass = if is_satellite {
        primary_mass + mass.0
    } else {
        primary_mass
    };
    if keyboard.just_pressed(KeyCode::KeyO) && !is_shift && !body.body_type.is_star_or_remnant() {
        let rel_pos = (pos.0 - primary_pos) * 1.10;
        pos.0 = primary_pos + rel_pos;
        let r = rel_pos.length().max(min_orbit_r);
        let v_circ = (G_ASTRO * mu_mass / r).sqrt();
        let phi = rel_pos.z.atan2(rel_pos.x);
        vel.0 = primary_vel + DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
        equilibrate_moved_orbit_temperature(
            commands,
            entity,
            r,
            is_satellite,
            mass.0,
            radius.0,
            body.body_type,
            comp,
            star_irrad,
            temp,
        );
    }
    if keyboard.just_pressed(KeyCode::KeyL) && !is_shift && !body.body_type.is_star_or_remnant() {
        let rel_pos = ((pos.0 - primary_pos) * 0.90).clamp_length_min(min_orbit_r);
        pos.0 = primary_pos + rel_pos;
        let r = rel_pos.length().max(min_orbit_r);
        let v_circ = (G_ASTRO * mu_mass / r).sqrt();
        let phi = rel_pos.z.atan2(rel_pos.x);
        vel.0 = primary_vel + DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
        equilibrate_moved_orbit_temperature(
            commands,
            entity,
            r,
            is_satellite,
            mass.0,
            radius.0,
            body.body_type,
            comp,
            star_irrad,
            temp,
        );
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
    if body.body_type.is_remnant() {
        if body.body_type == BodyType::BlackHole {
            commands
                .entity(star_ent)
                .insert(crate::simulation::space_weather::StellarFlareState {
                    flare_frequency: 2.0,
                    current_flare_intensity: 8.5,
                    flare_decay_timer_years: 0.20,
                    cme_front_radius_au: 0.10,
                    cme_speed_au_day: 0.85,
                    cme_density_multiplier: 50.0,
                    cme_active: true,
                });
            ignition.shockwave_radius = 2.0;
            toast.message = format!(
                "⚡ Accretion Flare: Relativistic X-ray burst erupted from {}!",
                body.name
            );
        } else {
            toast.message = format!(
                "⚠️ Degenerate Remnant: {} has exhausted core fusion and cannot ignite!",
                body.name
            );
        }
        toast.timer = 5.0;
        return;
    }

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
    if keyboard.just_pressed(KeyCode::KeyN)
        && body.body_type.is_star()
        && let Some(ref mut ignition) = ignition_opt
        && !ignition.is_ignited
    {
        ignition.core_temperature = 1.0e7;
        ignition.is_ignited = true;
        ignition.fusion_fraction = 1.0;
        ignition.shockwave_radius = 1.6;
        body.body_type = BodyType::MainSequenceStar;
        body.name = "The Star (Main Sequence)".to_string();
    }
    let is_shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if let Some(qs) = opt_quasi {
        if !is_shift && keyboard.just_pressed(KeyCode::KeyX) {
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

fn update_sea_level_components(
    commands: &mut Commands,
    entity: Entity,
    new_ocean_cov: f32,
    new_water_m_earth: f64,
    cloud_frac: f32,
    desiccated: bool,
) {
    let Ok(mut cmd) = commands.get_entity(entity) else {
        return;
    };
    cmd.entry::<VolatileInventory>()
        .and_modify(move |mut v| {
            v.ocean_coverage_frac = new_ocean_cov;
            v.delivered_water_m_earth = new_water_m_earth;
        })
        .or_insert(VolatileInventory {
            delivered_water_m_earth: new_water_m_earth,
            ocean_coverage_frac: new_ocean_cov,
            atmospheric_pressure_bar: 1.0,
            cometary_impact_count: 5,
        });
    cmd.entry::<PlanetaryClimate>()
        .and_modify(move |mut c| {
            c.cloud_coverage_frac = cloud_frac;
            if desiccated {
                c.albedo = 0.18;
            }
        })
        .or_insert_with(move || PlanetaryClimate {
            cloud_coverage_frac: cloud_frac,
            albedo: if desiccated { 0.18 } else { 0.30 },
            ..default()
        });
    cmd.entry::<crate::simulation::terraforming::TerraformingAtmosphere>()
        .and_modify(move |mut ta| {
            ta.surface_liquid_water_m_earth = new_water_m_earth;
        });
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
        update_sea_level_components(
            commands,
            entity,
            new_ocean_cov,
            new_water_m_earth,
            (0.25 + new_ocean_cov * 0.40).clamp(0.05, 0.90),
            false,
        );
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
            update_sea_level_components(commands, entity, 0.0, 0.0, 0.05, true);
            toast.message = format!(
                "🏜️ BASIN DESICCATED // \"{body_name}\" Sea Level: 0% (Continental Shelves, Mid-Ocean Ridges & Trenches Exposed)"
            );
            toast.timer = 4.0;
        } else {
            let new_ocean_cov = (comp.ice_frac as f32 / 0.08 - 0.10).clamp(0.0, 1.0);
            let new_water_m_earth = f64::from(new_ocean_cov * 0.0006);
            comp.ice_frac = (f64::from(new_ocean_cov) * 0.08).clamp(0.0, 0.45);
            *comp = comp.normalized();
            update_sea_level_components(
                commands,
                entity,
                new_ocean_cov,
                new_water_m_earth,
                (0.25 + new_ocean_cov * 0.40).clamp(0.05, 0.90),
                false,
            );
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
    mu_mass: f64,
    primary_pos: DVec3,
    primary_vel: DVec3,
    is_satellite: bool,
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
        let rel_vel = vel.0 - primary_vel;
        let speed = rel_vel.length();
        if speed > 0.0 {
            vel.0 = primary_vel + rel_vel + (rel_vel / speed) * (speed * 0.15);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyB) && shift {
        let rel_vel = vel.0 - primary_vel;
        let speed = rel_vel.length();
        if speed > 0.0 {
            vel.0 = primary_vel + rel_vel - (rel_vel / speed) * (speed * 0.15);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        let mut rel_pos = pos.0 - primary_pos;
        rel_pos.y = 0.0;
        pos.0 = primary_pos + rel_pos;
        let min_r = if is_satellite { 1e-4 } else { 0.1 };
        let r_cyl = (rel_pos.x * rel_pos.x + rel_pos.z * rel_pos.z)
            .sqrt()
            .max(min_r);
        let v_circ = (G_ASTRO * mu_mass / r_cyl).sqrt();
        let phi = rel_pos.z.atan2(rel_pos.x);
        vel.0 = primary_vel + DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
    }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        if let Ok(mut cmd) = commands.get_entity(entity) {
            cmd.try_despawn();
        }
        player_state.selected_entity = None;
    }
    let is_shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if !is_shift
        && keyboard.just_pressed(KeyCode::KeyX)
        && let Ok(mut cmd) = commands.get_entity(entity)
    {
        cmd.insert(PlanetaryRingSystem {
            inner_radius_au: 0.0008,
            outer_radius_au: 0.0028,
            ring_mass_earth: 0.0002,
            optical_depth: 0.88,
            ice_fraction: 0.95,
            silicate_fraction: 0.05,
        });
    }
    if keyboard.just_pressed(KeyCode::KeyE)
        && let Ok(mut cmd) = commands.get_entity(entity)
    {
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

#[allow(
    clippy::too_many_arguments,
    reason = "Applies composition, hydrosphere, and parent-relative body actions"
)]
fn apply_body_interaction_actions(
    commands: &mut Commands,
    keyboard: &ButtonInput<KeyCode>,
    mu_mass: f64,
    primary_pos: DVec3,
    primary_vel: DVec3,
    is_satellite: bool,
    entity: Entity,
    mass: &Mass,
    radius: &mut Radius,
    pos: &mut SimPosition,
    vel: &mut SimVelocity,
    comp: &mut Composition,
    body: &mut CelestialBody,
    diff_opt: Option<&mut InternalDifferentiation>,
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
            if body.body_type != BodyType::Moon
                && !body.body_type.is_remnant()
                && !body.body_type.is_hypothetical()
            {
                body.body_type = classify_body_by_mass_and_comp(mass.0, comp, false);
            }
            if let Some(diff) = diff_opt {
                diff.recalculate(mass.0, radius.0, comp);
            }
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
            mu_mass,
            primary_pos,
            primary_vel,
            is_satellite,
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
        if let Some((star_ent, _, _, _, _, _, _, mut body, _, _, Some(mut ignition), _, _, _)) =
            selected_query.iter_mut().find(
                |(_, _, _, _, _, _, _, body, _, _, _, _, opt_star, _)| {
                    !body.body_type.is_remnant()
                        && (opt_star.is_some() || body.body_type.is_star_or_remnant())
                },
            )
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

#[allow(
    clippy::too_many_arguments,
    reason = "Orchestrates selected body keyboard manipulation tools"
)]
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
    let Some(selected_ent) = player_state.selected_entity else {
        return;
    };
    let parent_ent = selected_query
        .get(selected_ent)
        .ok()
        .and_then(|t| t.13.map(|s| s.parent));
    let (primary_pos, primary_vel, primary_mass, is_satellite) = parent_ent
        .and_then(|p| {
            selected_query
                .get(p)
                .ok()
                .map(|t| (t.4.0, t.5.0, t.1.0, true))
        })
        .unwrap_or((star_pos, star_vel, star_mass, false));

    let star_irrad = selected_query
        .iter()
        .find(|t| t.12.is_some() || t.7.body_type.is_star_or_remnant())
        .map_or_else(
            || {
                crate::simulation::thermodynamics::stellar_irradiance_from_mass_or_opt(
                    None, None, None, star_mass,
                )
            },
            |t| {
                crate::simulation::thermodynamics::stellar_irradiance_from_mass_or_opt(
                    None,
                    Some(t.3.0),
                    Some(t.2.0),
                    t.1.0,
                )
            },
        );

    if let Ok((
        entity,
        mut mass,
        mut radius,
        mut temp,
        mut pos,
        mut vel,
        mut comp,
        mut body,
        mut diff_opt,
        mut trans_opt,
        mut ignition_opt,
        mut opt_quasi,
        _opt_star,
        _opt_sat,
    )) = selected_query.get_mut(selected_ent)
    {
        apply_body_mass_and_orbit_edits(
            commands,
            keyboard,
            entity,
            primary_mass,
            primary_pos,
            primary_vel,
            is_satellite,
            star_irrad,
            &mut mass,
            &mut radius,
            &mut temp,
            &mut pos,
            &mut vel,
            &comp,
            &mut body,
            diff_opt.as_deref_mut(),
            trans_opt.as_deref_mut(),
        );
        let mu_mass = if is_satellite {
            primary_mass + mass.0
        } else {
            primary_mass
        };
        apply_body_interaction_actions(
            commands,
            keyboard,
            mu_mass,
            primary_pos,
            primary_vel,
            is_satellite,
            entity,
            &mass,
            &mut radius,
            &mut pos,
            &mut vel,
            &mut comp,
            &mut body,
            diff_opt.as_deref_mut(),
            ignition_opt.as_deref_mut(),
            opt_quasi.as_deref_mut(),
            player_state,
            toast,
        );
    }
}
