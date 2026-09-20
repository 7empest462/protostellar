//! Live celestial body inspector and parameter manipulation actions.

use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;
use std::f64::consts::PI;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::super::types::*;

fn handle_mass_change(
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
    if is_star.is_none() && !body.body_type.is_star_or_remnant() {
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
        if !body.body_type.is_remnant() {
            body.body_type = new_type;
            body.name = match new_type {
                BodyType::BrownDwarf => "The Star (Brown Dwarf)".to_string(),
                BodyType::RedDwarf => "The Star (Red Dwarf - M Type)".to_string(),
                BodyType::YellowDwarf => "The Star (Yellow Dwarf - G2V)".to_string(),
                BodyType::BlueGiant => "The Star (Blue Giant - B Type)".to_string(),
                BodyType::BlueSupergiant => "The Star (Blue Supergiant - O Type)".to_string(),
                BodyType::Hypergiant => "The Star (Luminous Hypergiant)".to_string(),
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

fn handle_orbital_adjustment(
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

fn handle_composition_cycle(
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

fn handle_velocity_burn_and_fix(
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

fn handle_view_and_target(
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
                    cam.target_entity = Some(ent);
                }
                toast.message = "🎯 Camera Focus Locked to Selected Target".to_string();
                toast.timer = 3.5;
            }
            true
        }
        UiButtonAction::ResetView => {
            if let Ok(mut cam) = camera_query.single_mut() {
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
                    cmd.despawn();
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

fn handle_star_aging_transition(
    mass: &mut Mass,
    radius: &mut Radius,
    body: &mut CelestialBody,
    evo: &mut StellarEvolutionState,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    toast: &mut NotificationToast,
) {
    let m = mass.0;
    if m >= 25.0 {
        match evo.phase {
            StellarEvolutionPhase::ProtostarContraction | StellarEvolutionPhase::MainSequence => {
                evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
                body.body_type = BodyType::Hypergiant;
                body.name = "The Star (Luminous Yellow Hypergiant)".to_string();
                radius.0 = 4.5;
                temp.0 = 4000.0;
                lum.0 = 250_000.0;
                toast.message = "💥 Hypergiant Phase: Luminosity surged to 250,000 L☉!".to_string();
            }
            StellarEvolutionPhase::RedSupergiantBranch => {
                evo.phase = StellarEvolutionPhase::SupernovaExplosion;
                body.name = "The Star (Hypernova Detonation)".to_string();
                evo.nebula_expansion_radius_au = 5.0;
                evo.nebula_opacity = 1.0;
                toast.message =
                    "💥 HYPERNOVA DETONATION! Core collapsing into a Singularity!".to_string();
            }
            _ => {
                evo.phase = StellarEvolutionPhase::BlackHoleRemnant;
                body.body_type = BodyType::BlackHole;
                body.name = "The Star (Stellar-Mass Black Hole)".to_string();
                mass.0 = (m * 0.25).clamp(3.0, 15.0);
                radius.0 = 2.95e-5 * mass.0;
                temp.0 = 10.0;
                lum.0 = 5000.0;
                toast.message =
                    "🕳️ Gravitational Singularity Formed (Event Horizon & Accretion Disk)!"
                        .to_string();
            }
        }
    } else if m >= 8.0 {
        match evo.phase {
            StellarEvolutionPhase::ProtostarContraction | StellarEvolutionPhase::MainSequence => {
                evo.phase = StellarEvolutionPhase::RedSupergiantBranch;
                body.body_type = BodyType::RedSupergiant;
                body.name = "The Star (Red Supergiant)".to_string();
                radius.0 = 3.5;
                temp.0 = 3300.0;
                lum.0 = 80_000.0;
                toast.message =
                    "🔴 Red Supergiant Expansion (R ~ 3.5 AU, L ~ 80,000 L☉)!".to_string();
            }
            StellarEvolutionPhase::RedSupergiantBranch => {
                evo.phase = StellarEvolutionPhase::SupernovaExplosion;
                body.name = "The Star (Type II Supernova Explosion)".to_string();
                evo.nebula_expansion_radius_au = 4.0;
                evo.nebula_opacity = 1.0;
                toast.message =
                    "💥 TYPE II CORE-COLLAPSE SUPERNOVA! Blast expanding at 15,000 km/s!"
                        .to_string();
            }
            _ => {
                evo.phase = StellarEvolutionPhase::NeutronStarPulsar;
                body.body_type = BodyType::Pulsar;
                body.name = "The Star (Pulsar Remnant)".to_string();
                mass.0 = 1.44;
                radius.0 = 0.0001;
                temp.0 = 1_000_000.0;
                lum.0 = 100.0;
                toast.message =
                    "⚡ Relativistic Pulsar Remnant (B ~ 10^12 G, P ~ 33 ms Synchrotron Jets)!"
                        .to_string();
            }
        }
    } else if m >= 0.50 {
        match evo.phase {
            StellarEvolutionPhase::ProtostarContraction | StellarEvolutionPhase::MainSequence => {
                evo.phase = StellarEvolutionPhase::RedGiantBranch;
                evo.hydrogen_core_fraction = 0.0;
                body.body_type = BodyType::RedGiant;
                body.name = "The Star (Red Giant Branch)".to_string();
                radius.0 = 1.25;
                temp.0 = 3100.0;
                lum.0 = 2500.0;
                toast.message = "🔴 Star Expanded to Red Giant (R ~ 1.25 AU, L ~ 2500 L☉)! Inner planets engulfing!".to_string();
            }
            StellarEvolutionPhase::RedGiantBranch => {
                evo.phase = StellarEvolutionPhase::HeliumFlashAgb;
                body.name = "The Star (AGB Supergiant)".to_string();
                radius.0 = 1.50;
                temp.0 = 2900.0;
                lum.0 = 3500.0;
                toast.message =
                    "🔥 Helium Flash & Asymptotic Giant Pulses (R ~ 1.50 AU, L ~ 3500 L☉)!"
                        .to_string();
            }
            StellarEvolutionPhase::HeliumFlashAgb => {
                evo.phase = StellarEvolutionPhase::PlanetaryNebulaEjection;
                body.name = "The Star (Planetary Nebula Ejection)".to_string();
                evo.nebula_expansion_radius_au = 2.0;
                evo.nebula_opacity = 1.0;
                mass.0 = 0.55;
                toast.message =
                    "💨 Planetary Nebula Ejected! Star shed 45% mass, outer orbits expanding!"
                        .to_string();
            }
            _ => {
                evo.phase = StellarEvolutionPhase::WhiteDwarf;
                body.body_type = BodyType::WhiteDwarf;
                body.name = "The Star (White Dwarf Remnant)".to_string();
                radius.0 = 0.009;
                temp.0 = 30_000.0;
                lum.0 = (radius.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4);
                toast.message =
                    "⚪ Degenerate White Dwarf Remnant (Earth-sized, T ~ 30,000 K, B ~ 10^6 G)!"
                        .to_string();
            }
        }
    } else {
        evo.phase = StellarEvolutionPhase::WhiteDwarf;
        body.body_type = BodyType::WhiteDwarf;
        body.name = "The Star (Helium White Dwarf)".to_string();
        radius.0 = 0.009;
        temp.0 = 25_000.0;
        toast.message = "⚪ Low-Mass Helium White Dwarf Remnant Formed!".to_string();
    }
}

fn handle_age_star(selected_query: &mut SelectedWorldQuery, toast: &mut NotificationToast) {
    let mut star_opt = selected_query
        .iter_mut()
        .find(|(.., is_star, _, _, _, _)| is_star.is_some());

    if let Some((
        _ent,
        mut mass,
        mut radius,
        _,
        _,
        _,
        mut body,
        _,
        Some(ref mut ignition),
        ref mut opt_evo,
        Some(ref mut temp),
        Some(ref mut lum),
    )) = star_opt
    {
        if !ignition.is_ignited {
            ignition.core_temperature = 1.0e7;
            ignition.is_ignited = true;
            ignition.fusion_fraction = 1.0;
            ignition.shockwave_radius = 1.6;
            let (assigned_type, name_str) = if mass.0 < 0.08 {
                (BodyType::BrownDwarf, "The Star (Brown Dwarf)")
            } else if mass.0 < 0.50 {
                (BodyType::RedDwarf, "The Star (Red Dwarf - M Type)")
            } else if mass.0 < 1.4 {
                (BodyType::YellowDwarf, "The Star (Yellow Dwarf - G2V)")
            } else if mass.0 < 8.0 {
                (BodyType::BlueGiant, "The Star (Blue Giant - B Type)")
            } else if mass.0 < 25.0 {
                (
                    BodyType::BlueSupergiant,
                    "The Star (Blue Supergiant - O Type)",
                )
            } else {
                (BodyType::Hypergiant, "The Star (Luminous Hypergiant)")
            };
            body.body_type = assigned_type;
            body.name = name_str.to_string();
            if let Some(ref mut evo) = opt_evo {
                evo.phase = StellarEvolutionPhase::MainSequence;
            }
            toast.message = format!("⭐ Ignited: {} (Main Sequence)", body.name);
        } else if let Some(ref mut evo) = opt_evo {
            handle_star_aging_transition(&mut mass, &mut radius, &mut body, evo, temp, lum, toast);
        }
        toast.timer = 6.0;
    }
}

fn handle_ignite_star(selected_query: &mut SelectedWorldQuery, toast: &mut NotificationToast) {
    let mut star_opt = selected_query
        .iter_mut()
        .find(|(.., is_star, _, _, _, _)| is_star.is_some());
    if let Some((_, _, _, _, _, _, mut body, _, Some(ref mut ignition), ref mut opt_evo, _, _)) =
        star_opt
    {
        if ignition.is_ignited {
            ignition.shockwave_radius = 1.6;
            toast.message = "☀️ Coronal Mass Ejection & Solar Blast Triggered!".to_string();
        } else {
            ignition.core_temperature = 1.0e7;
            ignition.is_ignited = true;
            ignition.fusion_fraction = 1.0;
            ignition.shockwave_radius = 1.6;
            body.body_type = BodyType::MainSequenceStar;
            body.name = "The Star (Main Sequence)".to_string();
            if let Some(ref mut evo) = opt_evo {
                evo.phase = StellarEvolutionPhase::MainSequence;
            }
            toast.message =
                "⭐ Hydrogen Core Fusion Ignited! Solar Wind Shockwave Sweeping the System!"
                    .to_string();
        }
        toast.timer = 5.0;
    }
}

fn handle_trigger_cme(
    selected_query: &mut SelectedWorldQuery,
    commands: &mut Commands,
    toast: &mut NotificationToast,
) {
    let mut star_opt = selected_query
        .iter_mut()
        .find(|(.., is_star, _, _, _, _)| is_star.is_some());
    if let Some((star_ent, _, _, _, _, _, _, _, Some(ref mut ignition), _, _, _)) = star_opt {
        ignition.shockwave_radius = 0.1;
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
        toast.message =
            "☀️ Coronal Mass Ejection Erupted! Plasma Shockwave Propagating Interplanetary!"
                .to_string();
        toast.timer = 6.0;
    } else {
        toast.message = "⚠️ No central star found in system!".to_string();
        toast.timer = 3.0;
    }
}

fn handle_shatter_and_life(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    sim_time_years: f64,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::ShatterIntoRings => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok((.., body, is_star, _, _, _, _)) = selected_query.get(ent) {
                    if is_star.is_some() {
                        toast.message =
                            "⚠️ Cannot form planetary rings around the central star!".to_string();
                        toast.timer = 3.5;
                    } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
                        p_cmd.insert(PlanetaryRingSystem {
                            inner_radius_au: 0.0008,
                            outer_radius_au: 0.0028,
                            ring_mass_earth: 0.0002,
                            optical_depth: 0.88,
                            ice_fraction: 0.95,
                            silicate_fraction: 0.05,
                        });
                        toast.message =
                            format!("🪐 Formed Luminous Ring System around {}!", body.name);
                        toast.timer = 5.0;
                    }
                }
            }
            true
        }
        UiButtonAction::SeedLife => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok((.., mut comp, body, is_star, _, _, _, _)) = selected_query.get_mut(ent) {
                    if is_star.is_some() {
                        toast.message =
                            "⚠️ Cannot seed life onto a stellar plasma furnace!".to_string();
                        toast.timer = 3.5;
                    } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
                        p_cmd.insert((
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
                                emergence_year: Some(sim_time_years),
                            },
                            PlanetaryClimate {
                                surface_temperature_k: 288.0,
                                equilibrium_temperature_k: 255.0,
                                greenhouse_delta_k: 33.0,
                                albedo: 0.30,
                                ice_coverage_frac: 0.10,
                                cloud_coverage_frac: 0.55,
                                climate_regime: ClimateRegime::TemperateHabitable,
                            },
                        ));
                        comp.ice_frac = 0.08;
                        comp.gas_frac = 0.02;
                        toast.message = format!(
                            "🌱 Seeded Photosynthetic Biosphere & Oceans on {}!",
                            body.name
                        );
                        toast.timer = 5.0;
                    }
                }
            }
            true
        }
        _ => false,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Comprehensive body editor dispatcher requires all relevant ECS world handles and simulation resources"
)]
pub fn handle_body_editor_action(
    action: &UiButtonAction,
    player_state: &mut PlayerInteractionState,
    selected_query: &mut SelectedWorldQuery,
    camera_query: &mut Query<&mut PanOrbitCamera>,
    star_mass: f64,
    commands: &mut Commands,
    toast: &mut NotificationToast,
    lhb_state: &mut ResMut<crate::game::phases::LateHeavyBombardmentState>,
    theia_state: Option<&mut crate::simulation::accretion::TheiaImpactState>,
    sim_time_years: f64,
    _rng: &mut impl Rng,
) -> bool {
    if handle_mass_change(action, selected_query, player_state, toast) {
        return true;
    }
    if handle_orbital_adjustment(action, selected_query, player_state, star_mass, toast) {
        return true;
    }
    if handle_velocity_burn_and_fix(action, selected_query, player_state, star_mass, toast) {
        return true;
    }
    if handle_view_and_target(
        action,
        player_state,
        camera_query,
        selected_query,
        commands,
        toast,
    ) {
        return true;
    }

    match action {
        UiButtonAction::InjectEmbryo => {
            if let Some(state) = theia_state {
                state.manual_trigger_requested = true;
            }
            toast.message =
                "🌑 THEIA MOON COLLISION TRIGGERED // Intercept trajectory locked for Moon formation!"
                    .to_string();
            toast.timer = 8.0;
            true
        }
        UiButtonAction::IgniteStar => {
            handle_ignite_star(selected_query, toast);
            true
        }
        UiButtonAction::TriggerCoronalMassEjection => {
            handle_trigger_cme(selected_query, commands, toast);
            true
        }
        UiButtonAction::TriggerLhb => {
            lhb_state.is_active = true;
            lhb_state.manual_trigger_requested = true;
            toast.message =
                "☄️ LATE HEAVY BOMBARDMENT TRIGGERED // Cometary ocean-seeding shower inbound!"
                    .to_string();
            toast.timer = 8.0;
            true
        }
        UiButtonAction::AgeStar => {
            handle_age_star(selected_query, toast);
            true
        }
        UiButtonAction::TidalLock => {
            handle_tidal_lock(selected_query, player_state, star_mass, commands, toast);
            true
        }
        UiButtonAction::AccelerateInspiral => {
            handle_accelerate_inspiral(selected_query, player_state, toast);
            true
        }
        UiButtonAction::StripAtmosphere => {
            handle_strip_atmosphere(selected_query, player_state, commands, toast);
            true
        }
        UiButtonAction::BombardComet
        | UiButtonAction::BombardChondrite
        | UiButtonAction::BombardSalvo
        | UiButtonAction::BombardCore => {
            handle_bombardment_trigger(
                action,
                selected_query,
                player_state,
                commands,
                sim_time_years,
                toast,
            );
            true
        }
        _ => handle_shatter_and_life(
            action,
            selected_query,
            player_state,
            commands,
            sim_time_years,
            toast,
        ),
    }
}

fn handle_tidal_lock(
    selected_query: &SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    star_mass: f64,
    commands: &mut Commands,
    toast: &mut NotificationToast,
) {
    if let Some(ent) = player_state.selected_entity {
        if let Ok((_, _, _, pos, _, _, body, ..)) = selected_query.get(ent) {
            if !body.body_type.is_star_or_remnant() {
                let r_au = (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt().max(0.01);
                let p_yr = (r_au.powi(3) / star_mass.max(0.01)).sqrt();
                let p_hours = (p_yr * YEAR_SECONDS / 3600.0).clamp(1.0, 50000.0);
                commands.entity(ent).insert((
                    SpinState {
                        spin_vector: DVec3::new(0.0, 1.0, 0.0),
                        rotation_period_hours: p_hours,
                        axial_tilt_degrees: 0.0,
                    },
                    crate::simulation::tides::TidalState::new_locked(1.0),
                ));
                toast.message = format!(
                    "⚡ {} Synchronously Tidally Locked (Period: {:.1}h, Tilt: 0.0°)",
                    body.name, p_hours
                );
                toast.timer = 4.0;
            }
        }
    }
}

fn handle_accelerate_inspiral(
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    toast: &mut NotificationToast,
) {
    let Some(ent) = player_state.selected_entity else {
        toast.message =
            "⚠️ Select an orbiting companion first to accelerate GW inspiral.".to_string();
        toast.timer = 3.5;
        return;
    };

    if let Ok((_, _, _, pos, mut vel, _, body, is_star, ..)) = selected_query.get_mut(ent) {
        if is_star.is_some() {
            toast.message = "⚠️ Central star selected; select an orbiting companion.".to_string();
            toast.timer = 3.5;
            return;
        }

        // Dampen orbital speed by 10% to accelerate inspiral
        vel.0 *= 0.90;
        let r_au = pos.0.length();
        toast.message = format!(
            "⚡ Accelerated GW inspiral decay for \"{}\" (r = {:.3} AU, Δv = -10%)",
            body.name, r_au
        );
        toast.timer = 4.0;
    }
}

fn handle_strip_atmosphere(
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    toast: &mut NotificationToast,
) {
    let Some(ent) = player_state.selected_entity else {
        toast.message = "⚠️ Please select a planet first to strip its atmosphere!".to_string();
        toast.timer = 3.5;
        return;
    };

    if let Ok((_, mut mass, mut radius, _, _, mut comp, mut body, is_star, ..)) =
        selected_query.get_mut(ent)
    {
        if is_star.is_some() || body.body_type.is_star_or_remnant() {
            toast.message = "⚠️ Cannot strip atmosphere from a star or remnant.".to_string();
            toast.timer = 3.5;
            return;
        }

        let gas_stripped = mass.0 * comp.gas_frac;
        mass.0 = (mass.0 - gas_stripped).max(EARTH_MASS_SOLAR * 0.01);
        comp.gas_frac = 0.0;
        let norm = comp.normalized();
        comp.silicate_frac = norm.silicate_frac;
        comp.metal_frac = norm.metal_frac;
        comp.ice_frac = norm.ice_frac;
        comp.organics_frac = norm.organics_frac;

        let avg_density = comp.average_density();
        radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * std::f64::consts::PI))
            .cbrt()
            .max(EARTH_RADIUS_AU * 0.1);

        body.body_type = classify_body_by_mass_and_comp(mass.0, &comp, false);

        commands.entity(ent).insert(AtmosphericEscapeTail {
            loss_rate_m_earth_per_myr: 50.0,
            tail_length_au: 4.5,
            ion_color: Color::srgba(0.25, 0.85, 1.0, 0.85),
            is_active: true,
        });

        toast.message = format!(
            "💨 PHOTOEVAPORATION BURST! Stripped gaseous envelope from \"{}\", revealing bare rocky core!",
            body.name
        );
        toast.timer = 4.5;
    }
}

fn handle_bombardment_trigger(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    sim_time_years: f64,
    toast: &mut NotificationToast,
) {
    let Some(ent) = player_state.selected_entity else {
        toast.message =
            "⚠️ Select a planetary body first to order targeted bombardment.".to_string();
        toast.timer = 3.5;
        return;
    };

    let Ok((t_ent, mass, rad, pos, vel, _, body, is_star, ..)) = selected_query.get(ent) else {
        return;
    };

    if is_star.is_some() || body.body_type.is_star_or_remnant() {
        toast.message = "⚠️ Cannot bombard a stellar plasma furnace!".to_string();
        toast.timer = 3.5;
        return;
    }

    let b_type = match action {
        UiButtonAction::BombardComet => crate::simulation::terraforming::BombardmentType::IcyComet,
        UiButtonAction::BombardChondrite => {
            crate::simulation::terraforming::BombardmentType::CarbonaceousChondrite
        }
        UiButtonAction::BombardSalvo => {
            crate::simulation::terraforming::BombardmentType::VolatileAblationSalvo
        }
        UiButtonAction::BombardCore => {
            crate::simulation::terraforming::BombardmentType::IronAsteroid
        }
        _ => return,
    };

    crate::simulation::terraforming::launch_targeted_bombardment(
        commands,
        t_ent,
        pos.0,
        vel.0,
        mass.0,
        rad.0,
        &body.name,
        b_type,
        sim_time_years,
    );

    toast.message = format!(
        "☄️ INBOUND BOMBARDMENT // {} launched on intercept corridor to {}!",
        b_type.name(),
        body.name
    );
    toast.timer = 6.0;
}
