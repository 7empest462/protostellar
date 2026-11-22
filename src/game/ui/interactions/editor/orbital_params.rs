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
    handle_mass_change_with_effects(action, selected_query, None, None, player_state, toast)
}

fn apply_non_star_mass_ui_update(
    mass_solar: f64,
    old_type: BodyType,
    comp: &Composition,
    body: &mut CelestialBody,
    radius: &mut Radius,
) {
    let new_type = classify_body_by_mass_and_comp(mass_solar, comp, false);
    if new_type != old_type {
        body.body_type = new_type;
        if new_type.is_planet() && !body.name.starts_with("Planet") {
            body.name = format!("Planet ({new_type:?})");
        }
    }
    if body.body_type == BodyType::BlackHole {
        radius.0 = (1.974e-8 * mass_solar).max(1e-7);
    } else if body.body_type == BodyType::BrownDwarf {
        radius.0 = 0.11 * SOLAR_RADIUS_AU;
    } else if body.body_type.is_star_or_remnant() {
        if !body.body_type.is_remnant() && !body.body_type.is_hypothetical() {
            radius.0 = SOLAR_RADIUS_AU * mass_solar.max(0.08).powf(0.8);
        }
    } else {
        let avg_density = comp.average_density();
        radius.0 = ((3.0 * mass_solar / avg_density) / (4.0 * PI))
            .cbrt()
            .max(EARTH_RADIUS_AU * 0.1);
    }
}

fn format_mass_change_toast(
    multiplier: f64,
    mass_solar: f64,
    was_star: bool,
    body: &CelestialBody,
) -> String {
    let sign = if multiplier > 1.0 { "➕" } else { "➖" };
    let pct = if multiplier > 1.0 { "+25%" } else { "-20%" };
    if (was_star || body.body_type.is_star_or_remnant()) && mass_solar >= 0.01 {
        let readiness = if body.body_type == BodyType::WhiteDwarf && mass_solar > 1.44 {
            " | 💥 Type Ia Supernova Ready (>1.44 M☉)"
        } else if matches!(
            body.body_type,
            BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar | BodyType::QuarkStar
        ) && mass_solar > 2.17
        {
            " | 🕳️ TOV Collapse Ready (>2.17 M☉)"
        } else if !body.body_type.is_remnant() && mass_solar >= 25.0 {
            " | 💥 Hypernova -> Black Hole Ready (>=25 M☉)"
        } else if !body.body_type.is_remnant() && mass_solar >= 8.0 {
            " | 💥 Supernova Ready (>=8 M☉)"
        } else {
            ""
        };
        format!(
            "{sign} {} Mass Altered: {mass_solar:.2} M☉ ({pct} | {:?}{readiness})",
            body.name, body.body_type
        )
    } else {
        format!(
            "{sign} {} Mass Altered: {:.3} M⊕ ({pct} | {:?})",
            body.name,
            mass_solar / EARTH_MASS_SOLAR,
            body.body_type
        )
    }
}

pub fn handle_mass_change_with_effects(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    mut sat_diff_query: Option<
        &mut Query<
            (Option<&SatelliteOf>, Option<&mut InternalDifferentiation>),
            Without<PanOrbitCamera>,
        >,
    >,
    mut commands: Option<&mut Commands>,
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

    let Ok((
        _,
        mut mass,
        mut radius,
        _,
        _,
        comp,
        mut body,
        is_star,
        _,
        mut opt_evo,
        mut opt_temp,
        mut opt_lum,
        ..,
    )) = selected_query.get_mut(ent)
    else {
        return true;
    };

    mass.0 *= multiplier;
    let old_type = body.body_type;
    let was_star = is_star.is_some() || old_type.is_star_or_remnant();

    if was_star {
        crate::simulation::thermodynamics::sync_stellar_classification_after_mass_edit(
            mass.0,
            old_type,
            &comp,
            &mut body,
            Some(&mut radius),
            opt_temp.as_deref_mut(),
            opt_lum.as_deref_mut(),
            opt_evo.as_deref_mut(),
        );
    } else {
        apply_non_star_mass_ui_update(mass.0, old_type, &comp, &mut body, &mut radius);
    }

    if !was_star
        && body.body_type.is_star_or_remnant()
        && let Some(ref mut cmds) = commands
        && let Ok(mut cmd) = cmds.get_entity(ent)
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

    if let Some(ref mut sdq) = sat_diff_query
        && let Ok((_, Some(mut diff))) = sdq.get_mut(ent)
    {
        diff.recalculate(mass.0, radius.0, &comp);
    }

    toast.message = format_mass_change_toast(multiplier, mass.0, was_star, &body);
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
    handle_orbital_adjustment_relative(action, selected_query, None, player_state, star_mass, toast)
}

pub fn handle_orbital_adjustment_relative(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    sat_diff_query: Option<
        &Query<
            (Option<&SatelliteOf>, Option<&mut InternalDifferentiation>),
            Without<PanOrbitCamera>,
        >,
    >,
    player_state: &PlayerInteractionState,
    star_mass: f64,
    toast: &mut NotificationToast,
) -> bool {
    let (mult, label) = match action {
        UiButtonAction::ExpandOrbit => (1.10, "Expanded to"),
        UiButtonAction::ContractOrbit => (0.90, "Contracted to"),
        _ => return false,
    };

    let Some(ent) = player_state.selected_entity else {
        return true;
    };
    let parent_ent =
        sat_diff_query.and_then(|q| q.get(ent).ok().and_then(|(s, _)| s.map(|sat| sat.parent)));
    let (primary_pos, primary_vel, primary_mass, is_sat) = parent_ent
        .and_then(|p| {
            selected_query
                .get(p)
                .ok()
                .map(|t| (t.3.0, t.4.0, t.1.0, true))
        })
        .unwrap_or((DVec3::ZERO, DVec3::ZERO, star_mass, false));

    let star_irrad = selected_query
        .iter()
        .find(|t| t.7.is_some() || t.6.body_type.is_star_or_remnant())
        .map_or_else(
            || {
                crate::simulation::thermodynamics::stellar_irradiance_from_mass_or_opt(
                    None, None, None, star_mass,
                )
            },
            |t| {
                crate::simulation::thermodynamics::stellar_irradiance_from_mass_or_opt(
                    t.11.as_ref().map(|l| l.0),
                    t.10.as_ref().map(|tp| tp.0),
                    Some(t.2.0),
                    t.1.0,
                )
            },
        );

    if let Ok((
        _,
        mass,
        radius,
        mut pos,
        mut vel,
        comp,
        body,
        is_star,
        _,
        _,
        mut opt_temp,
        _,
        mut opt_vol,
        mut opt_climate,
    )) = selected_query.get_mut(ent)
    {
        if is_star.is_some() || body.body_type.is_star_or_remnant() {
            toast.message = format!(
                "⚠️ Cannot adjust orbital radius of central star {}",
                body.name
            );
            toast.timer = 3.0;
            return true;
        }
        let mut rel_pos = (pos.0 - primary_pos) * mult;
        let r = rel_pos.length().max(1e-4);
        if rel_pos.length_squared() <= 1e-12 {
            rel_pos = DVec3::new(r, 0.0, 0.0);
        }
        pos.0 = primary_pos + rel_pos;
        let mu_mass = if is_sat {
            primary_mass + mass.0
        } else {
            primary_mass.max(0.0)
        };
        let v_mag = (G_ASTRO * mu_mass / r).sqrt();
        let raw_tangent = DVec3::new(-rel_pos.z, 0.0, rel_pos.x);
        let orbit_tangent = if raw_tangent.length_squared() > 1e-10 {
            raw_tangent.normalize()
        } else {
            DVec3::Z
        };
        vel.0 = primary_vel + orbit_tangent * v_mag;
        if !is_sat && let Some(ref mut temp) = opt_temp {
            crate::simulation::thermodynamics::equilibrate_body_temperature_for_orbit(
                r,
                mass.0,
                radius.0,
                body.body_type,
                &comp,
                star_irrad,
                temp,
                opt_vol.as_deref_mut(),
                opt_climate.as_deref_mut(),
            );
        }
        let dist_disp = if r < 0.05 {
            format!("{r:.4} AU")
        } else {
            format!("{r:.2} AU")
        };
        toast.message = format!("🚀 {} Orbit {} {}", body.name, label, dist_disp);
        toast.timer = 4.0;
    }
    true
}

pub fn handle_composition_cycle(
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    toast: &mut NotificationToast,
) {
    handle_composition_cycle_with_diff(selected_query, None, player_state, toast);
}

pub fn handle_composition_cycle_with_diff(
    selected_query: &mut SelectedWorldQuery,
    mut sat_diff_query: Option<
        &mut Query<
            (Option<&SatelliteOf>, Option<&mut InternalDifferentiation>),
            Without<PanOrbitCamera>,
        >,
    >,
    player_state: &PlayerInteractionState,
    toast: &mut NotificationToast,
) {
    if let Some(ent) = player_state.selected_entity
        && let Ok((_, mass, mut radius, _, _, mut comp, mut body, is_star, ..)) =
            selected_query.get_mut(ent)
    {
        if is_star.is_some() || body.body_type.is_star_or_remnant() {
            toast.message = format!("⚠️ Cannot overwrite stellar composition for {}", body.name);
            toast.timer = 3.0;
            return;
        }
        if comp.silicate_frac > 0.5 {
            *comp = Composition::icy();
            toast.message = format!(
                "🎨 {} Material -> Icy Mixture (Density: 1.2 g/cm³)",
                body.name
            );
        } else if comp.ice_frac > 0.5 {
            *comp = Composition::solar_gas();
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
        if body.body_type != BodyType::Moon
            && !body.body_type.is_remnant()
            && !body.body_type.is_hypothetical()
        {
            body.body_type = classify_body_by_mass_and_comp(mass.0, &comp, false);
        }
        if let Some(ref mut sdq) = sat_diff_query
            && let Ok((_, Some(mut diff))) = sdq.get_mut(ent)
        {
            diff.recalculate(mass.0, radius.0, &comp);
        }
        toast.timer = 4.0;
    }
}

pub fn handle_velocity_burn_and_fix(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    star_mass: f64,
    toast: &mut NotificationToast,
) -> bool {
    handle_velocity_burn_and_fix_relative(
        action,
        selected_query,
        None,
        player_state,
        star_mass,
        toast,
    )
}

pub fn handle_velocity_burn_and_fix_relative(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    sat_diff_query: Option<
        &mut Query<
            (Option<&SatelliteOf>, Option<&mut InternalDifferentiation>),
            Without<PanOrbitCamera>,
        >,
    >,
    player_state: &PlayerInteractionState,
    star_mass: f64,
    toast: &mut NotificationToast,
) -> bool {
    let parent_ent = player_state.selected_entity.and_then(|ent| {
        sat_diff_query
            .as_ref()
            .and_then(|q| q.get(ent).ok().and_then(|(s, _)| s.map(|sat| sat.parent)))
    });
    let (primary_pos, primary_vel, primary_mass, is_sat) = parent_ent
        .and_then(|p| {
            selected_query
                .get(p)
                .ok()
                .map(|t| (t.3.0, t.4.0, t.1.0, true))
        })
        .unwrap_or((DVec3::ZERO, DVec3::ZERO, star_mass, false));

    match action {
        UiButtonAction::CycleComposition => {
            handle_composition_cycle_with_diff(selected_query, sat_diff_query, player_state, toast);
            true
        }
        UiButtonAction::BoostDeltaV => {
            if let Some(ent) = player_state.selected_entity
                && let Ok((_, _, _, _, mut vel, _, body, ..)) = selected_query.get_mut(ent)
            {
                let rel_vel = vel.0 - primary_vel;
                vel.0 = primary_vel + rel_vel * 1.15;
                toast.message = format!("⚡ {} Prograde Delta-V Boost (+15% Velocity)", body.name);
                toast.timer = 4.0;
            }
            true
        }
        UiButtonAction::BrakeDeltaV => {
            if let Some(ent) = player_state.selected_entity
                && let Ok((_, _, _, _, mut vel, _, body, ..)) = selected_query.get_mut(ent)
            {
                let rel_vel = vel.0 - primary_vel;
                vel.0 = primary_vel + rel_vel * 0.85;
                toast.message =
                    format!("⚡ {} Retrograde Delta-V Brake (-15% Velocity)", body.name);
                toast.timer = 4.0;
            }
            true
        }
        UiButtonAction::FixOrbit => {
            if let Some(ent) = player_state.selected_entity
                && let Ok((_, mass, _, mut pos, mut vel, _, body, ..)) = selected_query.get_mut(ent)
                && !body.body_type.is_star_or_remnant()
            {
                let mut rel_pos = pos.0 - primary_pos;
                rel_pos.y = 0.0;
                pos.0 = primary_pos + rel_pos;
                let min_r = if is_sat { 1e-4 } else { 0.1 };
                let r_cyl = (rel_pos.x * rel_pos.x + rel_pos.z * rel_pos.z)
                    .sqrt()
                    .max(min_r);
                let mu_mass = if is_sat {
                    primary_mass + mass.0
                } else {
                    primary_mass
                };
                let v_circ = (G_ASTRO * mu_mass / r_cyl).sqrt();
                let phi = rel_pos.z.atan2(rel_pos.x);
                vel.0 = primary_vel + DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
                toast.message =
                    format!("🪐 {} Orbit Circularized & Stabilized (e = 0.0)", body.name);
                toast.timer = 4.0;
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
