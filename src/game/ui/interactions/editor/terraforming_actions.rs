//! Terraforming, hydrosphere, rings, life, and targeted bombardment action handlers.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::game::ui::types::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

pub fn handle_shatter_and_life(
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
                if let Ok((_, _, _, _, _, _, body, is_star, ..)) = selected_query.get(ent) {
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

        UiButtonAction::AdvanceCivilization => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok((_, _, _, _, _, mut _comp, _body, is_star, ..)) =
                    selected_query.get_mut(ent)
                {
                    if is_star.is_some() {
                        toast.message = "⚠️ Civilization cannot survive on a star.".to_string();
                        toast.timer = 3.5;
                    } else if let Ok(mut p_cmd) = commands.get_entity(ent) {
                        p_cmd.insert(BiosphereState {
                            habitability_score: 0.95,
                            biomass_coverage_frac: 0.85,
                            oxygen_fraction: 0.21,
                            emergence_year: Some(0.0),
                            technosignature: 1.0,
                            bioluminescence_intensity: 0.35,
                            ..Default::default()
                        });
                        toast.message =
                            "🏙️ Civilization Emerged! Observe the night side.".to_string();
                        toast.timer = 5.0;
                    }
                }
            }
            true
        }
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
        UiButtonAction::SeedLife => {
            if let Some(ent) = player_state.selected_entity {
                if let Ok((_, _, _, _, _, mut comp, body, is_star, ..)) =
                    selected_query.get_mut(ent)
                {
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
            }
            true
        }
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
    if let Some(ent) = player_state.selected_entity {
        if let Ok((_, _, _, pos, _, _, body, ..)) = selected_query.get(ent) {
            if !body.body_type.is_star_or_remnant() {
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

pub fn handle_bombardment_trigger(
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
