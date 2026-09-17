//! Systems for calculating atmospheric escape, photoevaporation, magnetopause stand-off,
//! and envelope stripping across geological time.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::EARTH_MASS_SOLAR;

use super::physics::*;
use super::types::*;

/// Primary ECS system that models high-energy stellar XUV photoevaporation, stellar wind stripping,
/// geodynamo magnetospheric shielding, and cometary sublimation.
#[allow(
    clippy::type_complexity,
    reason = "bevy ECS query for atmospheric escape telemetry"
)]
pub fn update_atmospheric_escape_evolution(
    mut commands: Commands,
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    escape_config: Res<AtmosphericEscapeConfig>,
    mut stripped_events: MessageWriter<AtmosphericStrippedEvent>,
    star_query: Query<
        (
            &SimPosition,
            &Luminosity,
            &Mass,
            &Radius,
            &IgnitionState,
            &CelestialBody,
        ),
        With<CentralStar>,
    >,
    mut planets_query: Query<
        (
            Entity,
            &mut Mass,
            &SimPosition,
            &Radius,
            &mut Composition,
            &CelestialBody,
            Option<&mut AtmosphericEscapeTail>,
            Option<&mut VolatileInventory>,
            Option<&InternalDifferentiation>,
            Option<&PlanetaryClimate>,
            Option<&mut AtmosphericEscapeState>,
        ),
        Without<CentralStar>,
    >,
) {
    if (!config.enable_thermodynamics || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let Ok((star_pos, star_lum, star_mass, _star_rad, _ignition, _star_body)) = star_query.single()
    else {
        return;
    };

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr) * escape_config.time_scale;
    let l_xuv_watts = calculate_stellar_xuv_luminosity(star_lum.0, sim_time.elapsed_years);

    for (
        planet_ent,
        mut p_mass,
        p_pos,
        p_rad,
        mut comp,
        b_body,
        mut opt_tail,
        mut opt_vol,
        opt_diff,
        opt_climate,
        mut opt_escape_state,
    ) in planets_query.iter_mut()
    {
        if b_body.body_type.is_star_or_remnant() {
            continue;
        }

        let dist_au = (p_pos.0 - star_pos.0).length().max(0.005);
        let r_hill_au = dist_au * (p_mass.0 / (3.0 * star_mass.0.max(1e-4))).cbrt();

        let rates = evaluate_body_escape_rates(
            &p_mass,
            p_rad.0,
            dist_au,
            r_hill_au,
            l_xuv_watts,
            star_mass.0,
            star_lum.0,
            &comp,
            opt_vol.as_deref(),
            opt_diff,
            opt_climate,
            &escape_config,
        );

        let was_gas_rich = comp.gas_frac > 0.005;
        apply_escape_mass_loss(
            planet_ent,
            &b_body.name,
            &mut p_mass,
            &mut comp,
            &mut opt_vol,
            rates.total_loss_rate,
            was_gas_rich,
            dt_yr,
            sim_time.elapsed_years,
            &mut stripped_events,
        );

        update_visual_tail(
            &mut commands,
            planet_ent,
            &mut opt_tail,
            rates.tail_length_au,
            rates.total_loss_rate as f32,
            rates.ion_color,
            rates.total_loss_rate > 0.0001,
        );

        write_escape_state(
            &mut commands,
            planet_ent,
            &mut opt_escape_state,
            &rates,
            dt_yr,
        );
    }
}

struct EscapeRatesBundle {
    photo_rate: f64,
    wind_rate: f64,
    jeans_rate: f64,
    total_loss_rate: f64,
    magnetopause_radius_au: f64,
    magnetic_shielding: f32,
    xuv_flux_w_m2: f64,
    solar_wind_pressure: f64,
    roche_fill_fraction: f32,
    regime: AtmosphericEscapeRegime,
    tail_length_au: f32,
    ion_color: Color,
}

fn evaluate_body_escape_rates(
    p_mass: &Mass,
    p_rad_au: f64,
    dist_au: f64,
    r_hill_au: f64,
    l_xuv_watts: f64,
    star_mass_solar: f64,
    star_lum_solar: f64,
    comp: &Composition,
    opt_vol: Option<&VolatileInventory>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_climate: Option<&PlanetaryClimate>,
    escape_config: &AtmosphericEscapeConfig,
) -> EscapeRatesBundle {
    let xuv_flux_w_m2 = calculate_xuv_flux(l_xuv_watts, dist_au);
    let (p_ram, _) = calculate_solar_wind_pressure(dist_au, star_mass_solar);

    let b_gauss = opt_diff.map_or(0.0, |d| d.magnetic_field_gauss);
    let r_mp = calculate_magnetopause_radius(p_rad_au, b_gauss, p_ram);
    let shielding = calculate_magnetic_shielding_factor(p_rad_au, r_mp);
    let roche_fill_fraction = (p_rad_au / r_hill_au.max(1e-5)).clamp(0.0, 2.0) as f32;

    let has_gas = comp.gas_frac > 0.0001;
    let has_atm = opt_vol.is_some_and(|v| v.atmospheric_pressure_bar > 0.001);

    let photo_rate = if escape_config.enable_photoevaporation && (has_gas || has_atm) {
        calculate_photoevaporation_rate(
            p_mass.0,
            p_rad_au,
            xuv_flux_w_m2,
            escape_config.photo_efficiency_eta,
            r_hill_au,
        )
    } else {
        0.0
    };

    let wind_rate = if escape_config.enable_solar_wind_stripping && (has_gas || has_atm) {
        calculate_solar_wind_stripping_rate(p_rad_au, p_ram, shielding, p_mass.0)
            * escape_config.wind_efficiency
    } else {
        0.0
    };

    let jeans_rate = if escape_config.enable_jeans_escape && (has_gas || has_atm) {
        let t_exo = opt_climate.map_or(300.0, |c| f64::from(c.surface_temperature_k) * 1.5);
        let p_bar = opt_vol.map_or(0.0, |v| f64::from(v.atmospheric_pressure_bar));
        let (_, j_rate) = calculate_jeans_escape(p_mass.0, p_rad_au, t_exo, 2.0, p_bar);
        j_rate
    } else {
        0.0
    };

    let (comet_tail_len, comet_loss, comet_ion_col) =
        calculate_cometary_sublimation(dist_au, star_lum_solar, comp.ice_frac);

    let total_loss_rate = photo_rate + wind_rate + jeans_rate + f64::from(comet_loss);

    let (regime, tail_length_au, ion_color) = classify_escape_regime(
        photo_rate,
        wind_rate,
        jeans_rate,
        comet_loss,
        roche_fill_fraction,
        dist_au,
        comp,
        comet_tail_len,
        comet_ion_col,
    );

    EscapeRatesBundle {
        photo_rate,
        wind_rate,
        jeans_rate,
        total_loss_rate,
        magnetopause_radius_au: r_mp,
        magnetic_shielding: shielding,
        xuv_flux_w_m2,
        solar_wind_pressure: p_ram,
        roche_fill_fraction,
        regime,
        tail_length_au,
        ion_color,
    }
}

fn classify_escape_regime(
    photo_rate: f64,
    wind_rate: f64,
    jeans_rate: f64,
    comet_loss: f32,
    roche_fill: f32,
    dist_au: f64,
    comp: &Composition,
    comet_tail_len: f32,
    comet_ion_col: Color,
) -> (AtmosphericEscapeRegime, f32, Color) {
    if f64::from(comet_loss) > photo_rate && f64::from(comet_loss) > wind_rate && comet_loss > 0.001
    {
        return (
            AtmosphericEscapeRegime::CometaryOutgassing,
            comet_tail_len,
            comet_ion_col,
        );
    }

    if photo_rate > wind_rate && photo_rate > jeans_rate && photo_rate > 0.001 {
        let regime = if roche_fill > 0.60 {
            AtmosphericEscapeRegime::RocheLobeOverflow
        } else {
            AtmosphericEscapeRegime::HydrodynamicPhotoevaporation
        };

        let tail_len = (((0.35 / dist_au).powf(1.1) * 0.85).clamp(0.3, 6.0)) as f32;
        let col = if comp.gas_frac > 0.15 {
            Color::srgba(0.25, 0.85, 1.0, 0.85) // Electric Cyan (H/He)
        } else if comp.ice_frac > 0.10 {
            Color::srgba(0.60, 0.85, 1.0, 0.80) // Ice Blue
        } else {
            Color::srgba(1.0, 0.65, 0.20, 0.85) // Mineral Vapor Amber
        };
        return (regime, tail_len, col);
    }

    if wind_rate > jeans_rate && wind_rate > 0.0001 {
        let tail_len = (((0.20 / dist_au).powf(0.8) * 0.50).clamp(0.2, 3.0)) as f32;
        return (
            AtmosphericEscapeRegime::SolarWindErosion,
            tail_len,
            Color::srgba(0.9, 0.5, 0.3, 0.70),
        );
    }

    if jeans_rate > 0.0001 {
        return (
            AtmosphericEscapeRegime::JeansThermal,
            0.15,
            Color::srgba(0.4, 0.7, 0.9, 0.40),
        );
    }

    (
        AtmosphericEscapeRegime::None,
        0.0,
        Color::srgba(0.0, 0.0, 0.0, 0.0),
    )
}

fn apply_escape_mass_loss(
    planet_ent: Entity,
    planet_name: &str,
    p_mass: &mut Mass,
    comp: &mut Composition,
    opt_vol: &mut Option<Mut<'_, VolatileInventory>>,
    total_loss_rate: f64,
    was_gas_rich: bool,
    dt_yr: f64,
    sim_time_yr: f64,
    stripped_events: &mut MessageWriter<AtmosphericStrippedEvent>,
) {
    if total_loss_rate <= 0.0 {
        return;
    }

    let delta_m_earth = total_loss_rate * (dt_yr / 1.0e6);
    let delta_m_solar = delta_m_earth * EARTH_MASS_SOLAR;

    if comp.gas_frac > 0.0 {
        let cur_gas_m = p_mass.0 * comp.gas_frac;
        let stripped = delta_m_solar.min(cur_gas_m);
        p_mass.0 = (p_mass.0 - stripped).max(EARTH_MASS_SOLAR * 0.001);

        let new_gas_m = (cur_gas_m - stripped).max(0.0);
        comp.gas_frac = (new_gas_m / p_mass.0).clamp(0.0, 1.0);

        let norm = comp.normalized();
        comp.silicate_frac = norm.silicate_frac;
        comp.metal_frac = norm.metal_frac;
        comp.ice_frac = norm.ice_frac;
        comp.organics_frac = norm.organics_frac;
        comp.gas_frac = norm.gas_frac;

        if was_gas_rich && comp.gas_frac < 0.001 {
            stripped_events.write(AtmosphericStrippedEvent {
                planet: planet_ent,
                name: planet_name.to_string(),
                remaining_mass_earth: p_mass.0 / EARTH_MASS_SOLAR,
                time_yr: sim_time_yr,
            });
        }
    }

    if let Some(ref mut vol) = opt_vol {
        let pressure_loss =
            ((total_loss_rate * 0.02 * dt_yr) as f32).min(vol.atmospheric_pressure_bar);
        vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar - pressure_loss).max(0.0);

        let water_loss = delta_m_earth * 0.10;
        vol.delivered_water_m_earth = (vol.delivered_water_m_earth - water_loss).max(0.0);
    }
}

fn update_visual_tail(
    commands: &mut Commands,
    planet_ent: Entity,
    opt_tail: &mut Option<Mut<'_, AtmosphericEscapeTail>>,
    tail_length_au: f32,
    loss_rate: f32,
    ion_color: Color,
    is_active: bool,
) {
    if let Some(ref mut tail) = opt_tail {
        tail.tail_length_au = tail_length_au;
        tail.loss_rate_m_earth_per_myr = loss_rate;
        tail.ion_color = ion_color;
        tail.is_active = is_active;
    } else if is_active {
        commands.entity(planet_ent).insert(AtmosphericEscapeTail {
            loss_rate_m_earth_per_myr: loss_rate,
            tail_length_au,
            ion_color,
            is_active: true,
        });
    }
}

fn write_escape_state(
    commands: &mut Commands,
    planet_ent: Entity,
    opt_escape_state: &mut Option<Mut<'_, AtmosphericEscapeState>>,
    rates: &EscapeRatesBundle,
    dt_yr: f64,
) {
    let mass_lost_step = rates.total_loss_rate * (dt_yr / 1.0e6);

    if let Some(ref mut st) = opt_escape_state {
        st.photoevaporative_loss_rate_m_earth_per_myr = rates.photo_rate;
        st.solar_wind_stripping_rate_m_earth_per_myr = rates.wind_rate;
        st.jeans_escape_rate_m_earth_per_myr = rates.jeans_rate;
        st.total_loss_rate_m_earth_per_myr = rates.total_loss_rate;
        st.magnetopause_radius_au = rates.magnetopause_radius_au;
        st.magnetic_shielding_factor = rates.magnetic_shielding;
        st.xuv_flux_w_m2 = rates.xuv_flux_w_m2;
        st.solar_wind_pressure_n_m2 = rates.solar_wind_pressure;
        st.roche_lobe_fill_fraction = rates.roche_fill_fraction;
        st.cumulative_mass_lost_m_earth += mass_lost_step;
        st.escape_regime = rates.regime;
    } else {
        commands.entity(planet_ent).insert(AtmosphericEscapeState {
            photoevaporative_loss_rate_m_earth_per_myr: rates.photo_rate,
            solar_wind_stripping_rate_m_earth_per_myr: rates.wind_rate,
            jeans_escape_rate_m_earth_per_myr: rates.jeans_rate,
            total_loss_rate_m_earth_per_myr: rates.total_loss_rate,
            magnetopause_radius_au: rates.magnetopause_radius_au,
            magnetic_shielding_factor: rates.magnetic_shielding,
            xuv_flux_w_m2: rates.xuv_flux_w_m2,
            solar_wind_pressure_n_m2: rates.solar_wind_pressure,
            roche_lobe_fill_fraction: rates.roche_fill_fraction,
            cumulative_mass_lost_m_earth: mass_lost_step,
            escape_regime: rates.regime,
        });
    }
}
