//! Terraforming, hydrosphere, rings, life, and targeted bombardment action handlers.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::game::ui::types::*;
use crate::simulation::biology::{CivilizationState, technosignature_from_kardashev};
use crate::simulation::components::*;
use crate::simulation::economy::{
    ActionAccessResult, ProgressionState, ScienceData, TechnologyId, UnlockedTechs,
};
use crate::simulation::resources::*;
use crate::utils::constants::*;

fn check_progression_gate(
    progression: Option<&ProgressionState>,
    techs: Option<&UnlockedTechs>,
    science: &mut Option<&mut ScienceData>,
    req_tech: Option<TechnologyId>,
    cost: f64,
    toast: &mut NotificationToast,
) -> bool {
    let Some(prog) = progression else {
        return true;
    };
    match prog.check_action_access(req_tech, cost, techs, science.as_deref()) {
        ActionAccessResult::Allowed => prog.consume_progression_cost(cost, science.as_deref_mut()),
        ActionAccessResult::Locked(reason) => {
            toast.message = reason;
            toast.timer = 4.0;
            false
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Civilization uplift validates progression access and upgrades target world"
)]
fn handle_advance_civilization(
    selected_query: &mut SelectedWorldQuery,
    civ_query: &mut Query<(&mut CivilizationState, Option<&mut BiosphereState>)>,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    progression: Option<&ProgressionState>,
    techs: Option<&UnlockedTechs>,
    mut science: Option<&mut ScienceData>,
    sim_time_years: f64,
    toast: &mut NotificationToast,
) -> bool {
    if let Some(ent) = player_state.selected_entity
        && let Ok((_, _, _, _, _, _, body, is_star, _, _, opt_temp, _, opt_vol, opt_climate)) =
            selected_query.get_mut(ent)
    {
        if is_star.is_some() || body.body_type.is_star_or_remnant() {
            toast.message = "⚠️ Civilization cannot survive on a star.".to_string();
            toast.timer = 3.5;
        } else if !check_progression_gate(
            progression,
            techs,
            &mut science,
            Some(TechnologyId::KardashevAscension),
            200.0,
            toast,
        ) {
            return true;
        } else if let Ok((mut civ, opt_bio)) = civ_query.get_mut(ent) {
            let era = civ.advance_to_next_tier();
            let next_tech = technosignature_from_kardashev(civ.kardashev_level);
            if let Some(mut bio) = opt_bio {
                bio.technosignature = next_tech;
            } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
                let surface_temp_k = opt_climate
                    .as_deref()
                    .map(|c| f64::from(c.surface_temperature_k))
                    .or_else(|| opt_temp.as_deref().map(|t| t.0))
                    .unwrap_or(288.0);
                let has_liquid_water = opt_vol
                    .as_deref()
                    .is_some_and(|v| v.ocean_coverage_frac > 0.02);
                let (_, mut bio) = CivilizationState::bootstrap_for_world(
                    None,
                    surface_temp_k,
                    has_liquid_water,
                    sim_time_years,
                );
                bio.technosignature = next_tech;
                p_cmd.insert(bio);
            }
            toast.message = format!(
                "🏙️ {} uplifted to {} (K{:.2})!",
                body.name,
                era.short_badge(),
                civ.kardashev_level
            );
            toast.timer = 5.0;
        } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
            let surface_temp_k = opt_climate
                .as_deref()
                .map(|c| f64::from(c.surface_temperature_k))
                .or_else(|| opt_temp.as_deref().map(|t| t.0))
                .unwrap_or(288.0);
            let has_liquid_water = opt_vol
                .as_deref()
                .is_some_and(|v| v.ocean_coverage_frac > 0.02);
            let (civ, bio) = CivilizationState::bootstrap_for_world(
                None,
                surface_temp_k,
                has_liquid_water,
                sim_time_years,
            );
            let era = civ.era();
            let kind_label = if civ.is_domed_colony {
                "Pressurized Domed Colony"
            } else {
                "Planetary Civilization"
            };
            let k_lvl = civ.kardashev_level;
            p_cmd.insert((civ, bio));
            toast.message = format!(
                "🏙️ {} established {} — {} (K{:.2})! Check night-side lights.",
                body.name,
                kind_label,
                era.short_badge(),
                k_lvl
            );
            toast.timer = 5.0;
        }
    }
    true
}

#[allow(
    clippy::too_many_arguments,
    reason = "Biosphere seeding validates progression access and initializes oceans/climate"
)]
fn handle_seed_life(
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    progression: Option<&ProgressionState>,
    techs: Option<&UnlockedTechs>,
    mut science: Option<&mut ScienceData>,
    sim_time_years: f64,
    toast: &mut NotificationToast,
) -> bool {
    if let Some(ent) = player_state.selected_entity
        && let Ok((_, _, _, _, _, mut comp, body, is_star, ..)) = selected_query.get_mut(ent)
    {
        if is_star.is_some() {
            toast.message = "⚠️ Cannot seed life onto a stellar plasma furnace!".to_string();
            toast.timer = 3.5;
        } else if !check_progression_gate(progression, techs, &mut science, None, 100.0, toast) {
            return true;
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
            toast.message = format!(
                "🌱 Seeded Photosynthetic Biosphere & Oceans on {}!",
                body.name
            );
            toast.timer = 5.0;
        }
    }
    true
}

fn handle_flora_or_clear_civ(
    action: &UiButtonAction,
    ent: Entity,
    body_name: &str,
    civ_query: &mut Query<(&mut CivilizationState, Option<&mut BiosphereState>)>,
    commands: &mut Commands,
    sim_time_years: f64,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::CycleFloraAndBiolum => {
            if let Ok((_, Some(mut bio))) = civ_query.get_mut(ent) {
                bio.flora_pigment = match bio.flora_pigment {
                    FloraPigmentClass::SolarChlorophyll => FloraPigmentClass::RedDwarfCrimson,
                    FloraPigmentClass::RedDwarfCrimson => FloraPigmentClass::OrangeDwarfCarotenoid,
                    FloraPigmentClass::OrangeDwarfCarotenoid => FloraPigmentClass::HighUvIndigo,
                    FloraPigmentClass::HighUvIndigo => FloraPigmentClass::SolarChlorophyll,
                };
                bio.bioluminescence_intensity = if bio.bioluminescence_intensity < 0.20 {
                    0.55
                } else if bio.bioluminescence_intensity < 0.70 {
                    0.95
                } else {
                    0.0
                };
                toast.message = format!(
                    "🌿 {} flora: {} | Bioluminescence: {:.0}%",
                    body_name,
                    bio.flora_pigment.label(),
                    bio.bioluminescence_intensity * 100.0
                );
                toast.timer = 4.0;
            } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
                p_cmd.insert(BiosphereState {
                    habitability_score: 0.90,
                    biomass_coverage_frac: 0.70,
                    oxygen_fraction: 0.21,
                    emergence_year: Some(sim_time_years),
                    bioluminescence_intensity: 0.65,
                    ..Default::default()
                });
                toast.message = format!("🌿 Enabled glowing biosphere on {body_name}!");
                toast.timer = 4.0;
            }
            true
        }
        UiButtonAction::ClearCivilization => {
            if let Ok((_, Some(mut bio))) = civ_query.get_mut(ent) {
                bio.technosignature = 0.0;
            }
            if let Ok(mut p_cmd) = commands.get_entity(ent) {
                p_cmd.remove::<CivilizationState>();
            }
            toast.message = format!("✖ Removed civilization & megastructures from {body_name}.");
            toast.timer = 4.0;
            true
        }
        _ => false,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Civilization infrastructure toggles share world, civilization, and progression access"
)]
fn handle_civ_infrastructure_toggle(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    civ_query: &mut Query<(&mut CivilizationState, Option<&mut BiosphereState>)>,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    progression: Option<&ProgressionState>,
    techs: Option<&UnlockedTechs>,
    mut science: Option<&mut ScienceData>,
    sim_time_years: f64,
    toast: &mut NotificationToast,
) -> bool {
    let Some(ent) = player_state.selected_entity else {
        return true;
    };
    let Ok((_, _, _, _, _, _, body, is_star, _, _, opt_temp, _, opt_vol, opt_climate)) =
        selected_query.get_mut(ent)
    else {
        return true;
    };
    if is_star.is_some() || body.body_type.is_star_or_remnant() {
        toast.message = "⚠️ Planetary civilization controls apply to planets & moons.".to_string();
        toast.timer = 3.5;
        return true;
    }

    if handle_flora_or_clear_civ(
        action,
        ent,
        &body.name,
        civ_query,
        commands,
        sim_time_years,
        toast,
    ) {
        return true;
    }

    if !check_progression_gate(
        progression,
        techs,
        &mut science,
        Some(TechnologyId::KardashevAscension),
        120.0,
        toast,
    ) {
        return true;
    }
    if let Ok((mut civ, _)) = civ_query.get_mut(ent) {
        let (label, enabled) = match action {
            UiButtonAction::ToggleCivOrbitalRing => {
                civ.force_orbital_ring = !civ.force_orbital_ring;
                if civ.force_orbital_ring {
                    civ.orbital_infrastructure = civ.orbital_infrastructure.max(0.35);
                }
                ("Orbital Habitat Ring", civ.force_orbital_ring)
            }
            UiButtonAction::ToggleCivSolarMirrors => {
                civ.force_solar_mirrors = !civ.force_solar_mirrors;
                ("Orbital Solar Mirrors", civ.force_solar_mirrors)
            }
            _ => {
                civ.force_planetary_shield = !civ.force_planetary_shield;
                ("Planetary Shield", civ.force_planetary_shield)
            }
        };
        let state_str = if enabled { "ENABLED" } else { "DISABLED" };
        toast.message = format!("🛰️ {} {}: {}", body.name, label, state_str);
        toast.timer = 4.0;
    } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
        let surface_temp_k = opt_climate
            .as_deref()
            .map(|c| f64::from(c.surface_temperature_k))
            .or_else(|| opt_temp.as_deref().map(|t| t.0))
            .unwrap_or(288.0);
        let has_liquid_water = opt_vol
            .as_deref()
            .is_some_and(|v| v.ocean_coverage_frac > 0.02);
        let (mut civ, bio) = CivilizationState::bootstrap_for_world(
            None,
            surface_temp_k,
            has_liquid_water,
            sim_time_years,
        );
        let label = match action {
            UiButtonAction::ToggleCivOrbitalRing => {
                civ.force_orbital_ring = true;
                "Orbital Habitat Ring"
            }
            UiButtonAction::ToggleCivSolarMirrors => {
                civ.force_solar_mirrors = true;
                "Orbital Solar Mirrors"
            }
            _ => {
                civ.force_planetary_shield = true;
                "Planetary Shield"
            }
        };
        p_cmd.insert((civ, bio));
        toast.message = format!("🛰️ Bootstrapped {} with {}!", body.name, label);
        toast.timer = 4.5;
    }
    true
}

#[allow(
    clippy::too_many_arguments,
    reason = "Shatter, SeedLife, Probe launch, and multi-tier Civilization uplift share world & progression access"
)]
pub fn handle_shatter_and_life(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    civ_query: &mut Query<(&mut CivilizationState, Option<&mut BiosphereState>)>,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    progression: Option<&ProgressionState>,
    techs: Option<&UnlockedTechs>,
    science: Option<&mut ScienceData>,
    sim_time_years: f64,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::ShatterIntoRings => {
            if let Some(ent) = player_state.selected_entity
                && let Ok((_, _, _, _, _, _, body, is_star, ..)) = selected_query.get(ent)
            {
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
                    toast.message = format!("🪐 Formed Luminous Ring System around {}!", body.name);
                    toast.timer = 5.0;
                }
            }
            true
        }
        UiButtonAction::AdvanceCivilization => handle_advance_civilization(
            selected_query,
            civ_query,
            player_state,
            commands,
            progression,
            techs,
            science,
            sim_time_years,
            toast,
        ),
        UiButtonAction::ToggleCivOrbitalRing
        | UiButtonAction::ToggleCivSolarMirrors
        | UiButtonAction::ToggleCivPlanetaryShield
        | UiButtonAction::CycleFloraAndBiolum
        | UiButtonAction::ClearCivilization => handle_civ_infrastructure_toggle(
            action,
            selected_query,
            civ_query,
            player_state,
            commands,
            progression,
            techs,
            science,
            sim_time_years,
            toast,
        ),
        UiButtonAction::LaunchProbe => {
            if let Some(target) = player_state.selected_entity {
                commands.spawn(crate::simulation::probes::LaunchProbeRequest { target });
                toast.message = "🚀 Probe launched towards target!".to_string();
                toast.timer = 4.0;
            } else {
                toast.message = "⚠️ Please select a target planet first!".to_string();
                toast.timer = 3.0;
            }
            true
        }
        UiButtonAction::SeedLife => handle_seed_life(
            selected_query,
            player_state,
            commands,
            progression,
            techs,
            science,
            sim_time_years,
            toast,
        ),
        _ => false,
    }
}

pub fn handle_hydrosphere_action(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    toast: &mut NotificationToast,
) -> bool {
    let (delta, is_dry, is_earth) = match action {
        UiButtonAction::AddOceanWater => (0.10f32, false, false),
        UiButtonAction::DrainOceanWater => (-0.10f32, false, false),
        UiButtonAction::DryBasin => (0.0f32, true, false),
        UiButtonAction::EarthSeaLevel => (0.0f32, false, true),
        _ => return false,
    };

    let Some(ent) = player_state.selected_entity else {
        toast.message =
            "⚠️ Please select a planetary body first to adjust hydrosphere!".to_string();
        toast.timer = 3.0;
        return true;
    };

    let Ok((_, _mass, _, _, _, mut comp, body, is_star, _, _, _, _, opt_vol, opt_climate)) =
        selected_query.get_mut(ent)
    else {
        return true;
    };

    if is_star.is_some() || body.body_type.is_star_or_remnant() {
        toast.message = "⚠️ Cannot modify hydrosphere on a stellar plasma furnace!".to_string();
        toast.timer = 3.0;
        return true;
    }

    let current_cov = if let Some(ref v) = opt_vol {
        v.ocean_coverage_frac
    } else {
        comp.ice_frac as f32
    };

    let new_ocean_cov = if is_dry {
        0.0f32
    } else if is_earth {
        0.71f32
    } else {
        (current_cov + delta).clamp(0.0, 1.0)
    };

    let new_water_m_earth = f64::from(new_ocean_cov * 0.0006);

    if let Some(mut vol) = opt_vol {
        vol.ocean_coverage_frac = new_ocean_cov;
        vol.delivered_water_m_earth = new_water_m_earth;
        if new_ocean_cov > 0.01 && vol.atmospheric_pressure_bar < 0.2 {
            vol.atmospheric_pressure_bar = 1.0;
        }
    } else if let Ok(mut cmd) = commands.get_entity(ent) {
        cmd.insert(VolatileInventory {
            delivered_water_m_earth: new_water_m_earth,
            ocean_coverage_frac: new_ocean_cov,
            atmospheric_pressure_bar: 1.0,
            cometary_impact_count: 5,
        });
    }

    if let Ok(mut cmd) = commands.get_entity(ent) {
        cmd.entry::<crate::simulation::terraforming::TerraformingAtmosphere>()
            .and_modify(move |mut terra| {
                if is_dry {
                    terra.surface_liquid_water_m_earth = 0.0;
                    terra.atmospheric_water_m_earth = 0.0;
                } else {
                    terra.surface_liquid_water_m_earth = new_water_m_earth;
                    if new_ocean_cov > 0.01 && terra.total_pressure_bar() < 1.0 {
                        terra.nitrogen_pressure_bar = terra.nitrogen_pressure_bar.max(0.8);
                    }
                }
            });
    }

    let target_ice_frac = (f64::from(new_ocean_cov) * 0.08).clamp(0.0, 0.45);
    comp.ice_frac = target_ice_frac;
    *comp = comp.normalized();

    if let Some(mut climate) = opt_climate {
        if new_ocean_cov <= 0.001 {
            climate.cloud_coverage_frac = 0.05;
            climate.albedo = 0.18;
        } else if is_earth {
            climate.cloud_coverage_frac = 0.55;
            climate.albedo = 0.30;
        } else {
            climate.cloud_coverage_frac = (0.25 + new_ocean_cov * 0.40).clamp(0.05, 0.90);
        }
    }

    if is_dry {
        toast.message = format!(
            "🏜️ BASIN DESICCATED // \"{}\" Sea Level: 0% (Continental Shelves, Mid-Ocean Ridges & Trenches Exposed)",
            body.name
        );
    } else if is_earth {
        toast.message = format!(
            "🌍 EARTH BASELINE // \"{}\" Sea Level: 71% (Standard Continental Shelves & Oceans Active)",
            body.name
        );
    } else if delta > 0.0 {
        toast.message = format!(
            "🌊 SEA LEVEL EXPANDED // \"{}\" Sea Level: {:.0}% (+10% | Lowlands Flooded)",
            body.name,
            new_ocean_cov * 100.0
        );
    } else {
        toast.message = format!(
            "🏝️ OCEANS DRAINED // \"{}\" Sea Level: {:.0}% (-10% | Continental Margins Exposed)",
            body.name,
            new_ocean_cov * 100.0
        );
    }
    toast.timer = 4.5;
    true
}

pub fn handle_tidal_lock(
    selected_query: &SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    star_mass: f64,
    commands: &mut Commands,
    toast: &mut NotificationToast,
) {
    if let Some(ent) = player_state.selected_entity
        && let Ok((_, _, _, pos, _, _, body, ..)) = selected_query.get(ent)
        && !body.body_type.is_star_or_remnant()
    {
        let r_au = (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt().max(0.01);
        let p_yr = (r_au.powi(3) / star_mass.max(0.01)).sqrt();
        let p_hours = (p_yr * YEAR_SECONDS / 3600.0).clamp(1.0, 50000.0);
        if let Ok(mut cmd) = commands.get_entity(ent) {
            cmd.try_insert((
                SpinState {
                    spin_vector: DVec3::new(0.0, 1.0, 0.0),
                    rotation_period_hours: p_hours,
                    axial_tilt_degrees: 0.0,
                },
                crate::simulation::tides::TidalState::new_locked(1.0),
            ));
        }
        toast.message = format!(
            "⚡ {} Synchronously Tidally Locked (Period: {:.1}h, Tilt: 0.0°)",
            body.name, p_hours
        );
        toast.timer = 4.0;
    }
}

pub fn handle_accelerate_inspiral(
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

pub fn handle_strip_atmosphere(
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

        if let Ok(mut cmd) = commands.get_entity(ent) {
            cmd.try_insert(AtmosphericEscapeTail {
                loss_rate_m_earth_per_myr: 50.0,
                tail_length_au: 4.5,
                ion_color: Color::srgba(0.25, 0.85, 1.0, 0.85),
                is_active: true,
            });
        }

        toast.message = format!(
            "💨 PHOTOEVAPORATION BURST! Stripped gaseous envelope from \"{}\", revealing bare rocky core!",
            body.name
        );
        toast.timer = 4.5;
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bombardment dispatch validates optional Progression gating"
)]
pub fn handle_bombardment_trigger(
    action: &UiButtonAction,
    selected_query: &mut SelectedWorldQuery,
    player_state: &PlayerInteractionState,
    commands: &mut Commands,
    progression: Option<&ProgressionState>,
    techs: Option<&UnlockedTechs>,
    mut science: Option<&mut ScienceData>,
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

    let (req_tech, cost) = match b_type {
        crate::simulation::terraforming::BombardmentType::IcyComet => {
            (Some(TechnologyId::CometRedirection), 50.0)
        }
        crate::simulation::terraforming::BombardmentType::CarbonaceousChondrite
        | crate::simulation::terraforming::BombardmentType::VolatileAblationSalvo => {
            (Some(TechnologyId::AtmosphericInjection), 50.0)
        }
        crate::simulation::terraforming::BombardmentType::IronAsteroid => (None, 50.0),
    };
    if !check_progression_gate(progression, techs, &mut science, req_tech, cost, toast) {
        return;
    }

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
