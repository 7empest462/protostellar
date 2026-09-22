//! Simulation systems for stellar coronal mass ejections and planetary auroral oval dynamics.

use bevy::prelude::*;

use crate::simulation::atmosphere_escape::physics::{
    calculate_magnetopause_radius, calculate_solar_wind_pressure,
};
use crate::simulation::components::*;
use crate::simulation::resources::{SimTime, TimeWarp};
use crate::simulation::space_weather::physics::{
    calculate_auroral_oval_geometry, calculate_geomagnetic_kp,
};
use crate::simulation::space_weather::types::*;

/// Advances stellar coronal flare decay and expands outward-propagating CME shock fronts.
pub fn update_stellar_flares_system(
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut stars_query: Query<(&mut StellarFlareState, &Mass, &SimPosition), With<CentralStar>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt_yr = sim_time.current_dt_yr as f32;
    if dt_yr <= 0.0 {
        return;
    }

    for (mut flare, _mass, _pos) in stars_query.iter_mut() {
        // 1. Advance active CME plasma shock front
        if flare.cme_active {
            let dt_days = dt_yr * 365.25;
            flare.cme_front_radius_au += flare.cme_speed_au_day * dt_days;

            // CME dissipates beyond outer boundary of planetary disk
            if flare.cme_front_radius_au > 50.0 {
                flare.cme_active = false;
                flare.cme_front_radius_au = 0.0;
            }
        }

        // 2. Exponential decay of flare flash
        if flare.current_flare_intensity > 0.0 {
            // Decay over ~0.05 - 0.2 years depending on intensity
            let decay_rate = 15.0;
            flare.current_flare_intensity =
                (flare.current_flare_intensity - dt_yr * decay_rate).max(0.0);
        }
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Component attachment requires multiple optional components"
)]
fn ensure_space_weather_components_attached(
    commands: &mut Commands,
    unattached_stars: &Query<Entity, (With<CentralStar>, Without<StellarFlareState>)>,
    unattached_planets: &Query<
        (
            Entity,
            &CelestialBody,
            Option<&InternalDifferentiation>,
            Option<&ElectromagneticFieldState>,
            Option<&VolatileInventory>,
        ),
        Without<AuroralOvalState>,
    >,
) {
    for star_ent in unattached_stars.iter() {
        commands
            .entity(star_ent)
            .insert(StellarFlareState::default());
    }

    for (p_ent, body, opt_diff, opt_em, opt_vol) in unattached_planets.iter() {
        if body.body_type.is_star_or_remnant() {
            continue;
        }
        let b_gauss = opt_diff
            .map_or(0.0, |d| d.magnetic_field_gauss)
            .max(opt_em.map_or(0.0, |e| e.magnetic_field_gauss));
        let has_atmo = opt_vol.map_or(
            body.body_type == BodyType::GasGiant || body.body_type == BodyType::IceGiant,
            |v| v.atmospheric_pressure_bar >= 0.001,
        );
        if b_gauss > 0.01 && has_atmo {
            commands
                .entity(p_ent)
                .try_insert(AuroralOvalState::default());
        }
    }
}

fn update_single_planet_aurora(
    planet_ent: Entity,
    p_pos: &SimPosition,
    p_rad: &Radius,
    opt_diff: Option<&InternalDifferentiation>,
    opt_em: Option<&ElectromagneticFieldState>,
    opt_vol: Option<&VolatileInventory>,
    is_gas_or_ice_giant: bool,
    aurora: &mut AuroralOvalState,
    star_flare: Option<StellarFlareState>,
    star_mass: f64,
    star_pos: Vec3,
    dt_yr: f32,
    cme_events: &mut MessageWriter<CmeShockwaveEvent>,
) {
    let planet_world_pos = Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32);
    let dist_au = f64::from((planet_world_pos - star_pos).length().max(0.01));
    let (p_ram_base, _) = calculate_solar_wind_pressure(dist_au, star_mass);

    let mut ram_multiplier = 1.0f32;
    if let Some(flare) = star_flare {
        if flare.cme_active {
            let dist_f = dist_au as f32;
            let shock_thickness = 0.35;
            if dist_f >= flare.cme_front_radius_au - shock_thickness
                && dist_f <= flare.cme_front_radius_au + shock_thickness * 0.5
            {
                ram_multiplier = flare.cme_density_multiplier;
                cme_events.write(CmeShockwaveEvent {
                    target: planet_ent,
                    orbital_radius_au: dist_f,
                    density_multiplier: flare.cme_density_multiplier,
                    flare_intensity: flare.current_flare_intensity,
                });
            }
        }
    }

    let effective_p_ram = p_ram_base * f64::from(ram_multiplier);
    let b_gauss = opt_diff
        .map_or(0.0, |d| d.magnetic_field_gauss)
        .max(opt_em.map_or(0.0, |e| e.magnetic_field_gauss));
    let has_atmo = opt_vol.map_or(is_gas_or_ice_giant, |v| v.atmospheric_pressure_bar >= 0.001);

    if b_gauss <= 0.01 || !has_atmo {
        aurora.magnetopause_standoff_rp = 1.0;
        aurora.oval_colatitude_rad = 0.0;
        aurora.oval_width_rad = 0.0;
        aurora.auroral_intensity = 0.0;
        aurora.geomagnetic_kp_index = 0.0;
        aurora.storm_level = GeomagneticStormLevel::Quiet;
        return;
    }

    let r_p = p_rad.0;
    let r_mp = calculate_magnetopause_radius(r_p, b_gauss, effective_p_ram);
    let standoff_rp = (r_mp / r_p).max(1.05) as f32;

    let (colatitude, half_width) = calculate_auroral_oval_geometry(standoff_rp);
    let (kp, level) = calculate_geomagnetic_kp(standoff_rp, ram_multiplier, b_gauss as f32);

    aurora.magnetopause_standoff_rp = standoff_rp;
    aurora.oval_colatitude_rad = colatitude;
    aurora.oval_width_rad = half_width;
    aurora.geomagnetic_kp_index = kp;
    aurora.storm_level = level;

    let base_luminance = (b_gauss as f32).clamp(0.2, 2.5);
    let storm_boost = 1.0 + (kp / 2.5) * 1.5;
    aurora.auroral_intensity = base_luminance * storm_boost;
    aurora.flutter_phase = (aurora.flutter_phase + dt_yr * (120.0 + kp * 50.0)) % 10000.0;
}

/// Dynamically models solar wind - magnetosphere interaction, magnetopause standoff,
/// and dual-pole auroral oval geometry and radiance for magnetized worlds with atmospheres.
#[allow(
    clippy::type_complexity,
    reason = "Space weather entity query complexity"
)]
pub fn update_planetary_auroral_ovals_system(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut cme_events: MessageWriter<CmeShockwaveEvent>,
    unattached_stars: Query<Entity, (With<CentralStar>, Without<StellarFlareState>)>,
    unattached_planets: Query<
        (
            Entity,
            &CelestialBody,
            Option<&InternalDifferentiation>,
            Option<&ElectromagneticFieldState>,
            Option<&VolatileInventory>,
        ),
        Without<AuroralOvalState>,
    >,
    stars_query: Query<(&StellarFlareState, &Mass, &SimPosition), With<CentralStar>>,
    mut planets_query: Query<(
        Entity,
        &SimPosition,
        &Radius,
        &CelestialBody,
        Option<&InternalDifferentiation>,
        Option<&ElectromagneticFieldState>,
        Option<&VolatileInventory>,
        &mut AuroralOvalState,
    )>,
) {
    ensure_space_weather_components_attached(&mut commands, &unattached_stars, &unattached_planets);

    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt_yr = sim_time.current_dt_yr as f32;
    let star_data = stars_query.iter().next();
    let (star_flare, star_mass, star_pos) = match star_data {
        Some((f, m, p)) => (Some(*f), m.0, Vec3::new(p.x as f32, p.y as f32, p.z as f32)),
        None => (None, 1.0, Vec3::ZERO),
    };

    for (planet_ent, p_pos, p_rad, body, opt_diff, opt_em, opt_vol, mut aurora) in
        planets_query.iter_mut()
    {
        if body.body_type.is_star_or_remnant()
            || matches!(
                body.body_type,
                BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain | BodyType::Planetesimal
            )
        {
            continue;
        }

        let is_gas_or_ice_giant =
            body.body_type == BodyType::GasGiant || body.body_type == BodyType::IceGiant;
        update_single_planet_aurora(
            planet_ent,
            p_pos,
            p_rad,
            opt_diff,
            opt_em,
            opt_vol,
            is_gas_or_ice_giant,
            &mut aurora,
            star_flare,
            star_mass,
            star_pos,
            dt_yr,
            &mut cme_events,
        );
    }
}
