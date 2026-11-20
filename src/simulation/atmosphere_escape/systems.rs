//! Systems for calculating atmospheric escape, photoevaporation, magnetopause stand-off,
//! and envelope stripping across geological time.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::EARTH_MASS_SOLAR;

use super::physics::*;
use super::types::*;

#[derive(Clone, Copy)]
struct EscapeStarInfo {
    entity: Entity,
    pos: bevy::math::DVec3,
    mass_solar: f64,
    lum_solar: f64,
    l_xuv_watts: f64,
}

fn resolve_escape_host_and_hill(
    p_pos: bevy::math::DVec3,
    p_mass_solar: f64,
    opt_sat: Option<&SatelliteOf>,
    stars: &[EscapeStarInfo],
    body_lookup: &rustc_hash::FxHashMap<Entity, (bevy::math::DVec3, f64)>,
) -> Option<(EscapeStarInfo, f64, f64)> {
    let host_star = opt_sat
        .and_then(|sat| stars.iter().find(|s| s.entity == sat.parent).copied())
        .or_else(|| {
            stars
                .iter()
                .min_by(|a, b| {
                    (p_pos - a.pos)
                        .length_squared()
                        .partial_cmp(&(p_pos - b.pos).length_squared())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied()
        })?;

    let dist_au = (p_pos - host_star.pos).length().max(0.005);
    let r_hill_star = dist_au * (p_mass_solar / (3.0 * host_star.mass_solar.max(1e-4))).cbrt();
    let r_hill_au = opt_sat
        .and_then(|sat| body_lookup.get(&sat.parent).copied())
        .filter(|&(_, parent_mass)| parent_mass > 1e-12)
        .map_or(r_hill_star, |(parent_pos, parent_mass)| {
            let moon_sep = (p_pos - parent_pos).length().max(1e-5);
            let r_hill_moon = moon_sep * (p_mass_solar / (3.0 * parent_mass)).cbrt();
            r_hill_moon.min(r_hill_star)
        });

    Some((host_star, dist_au, r_hill_au))
}

type EscapeStarQueryItem<'a> = (
    Entity,
    &'a SimPosition,
    &'a Luminosity,
    &'a Mass,
    &'a Radius,
    &'a IgnitionState,
    &'a CelestialBody,
);

type EscapeBodyQueryItem<'a> = (
    Entity,
    &'a mut Mass,
    &'a SimPosition,
    &'a mut Radius,
    &'a mut Composition,
    &'a mut CelestialBody,
    Option<&'a mut AtmosphericEscapeTail>,
    Option<&'a mut VolatileInventory>,
    Option<&'a mut crate::simulation::terraforming::TerraformingAtmosphere>,
    Option<&'a mut InternalDifferentiation>,
    Option<&'a PlanetaryClimate>,
    Option<&'a Temperature>,
    Option<&'a SatelliteOf>,
    Option<&'a mut AtmosphericEscapeState>,
);

type EscapeBodyFilter = (
    Without<CentralStar>,
    Without<crate::simulation::probes::SpaceProbe>,
    Without<crate::simulation::terraforming::BombardmentProjectile>,
);

/// Primary ECS system that models high-energy stellar XUV photoevaporation, stellar wind stripping,
/// geodynamo magnetospheric shielding, and cometary sublimation.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "bevy ECS query for atmospheric escape telemetry"
)]
pub fn update_atmospheric_escape_evolution(
    mut commands: Commands,
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    escape_config: Res<AtmosphericEscapeConfig>,
    mut stripped_events: MessageWriter<AtmosphericStrippedEvent>,
    mut body_lookup: Local<rustc_hash::FxHashMap<Entity, (bevy::math::DVec3, f64)>>,
    star_query: Query<EscapeStarQueryItem, With<CentralStar>>,
    mut planets_query: Query<EscapeBodyQueryItem, EscapeBodyFilter>,
) {
    if (!config.enable_thermodynamics || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let stars: smallvec::SmallVec<[EscapeStarInfo; 4]> = star_query
        .iter()
        .map(
            |(entity, pos, lum, mass, _rad, _ign, _body)| EscapeStarInfo {
                entity,
                pos: pos.0,
                mass_solar: mass.0,
                lum_solar: lum.0,
                l_xuv_watts: calculate_stellar_xuv_luminosity(lum.0, sim_time.elapsed_years),
            },
        )
        .collect();
    if stars.is_empty() {
        return;
    }

    let dt_yr = if sim_time.current_dt_yr > 0.0 {
        sim_time.current_dt_yr
    } else {
        config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)
    };

    body_lookup.clear();
    for (ent, p_mass, p_pos, ..) in planets_query.iter() {
        body_lookup.insert(ent, (p_pos.0, p_mass.0));
    }

    for (
        planet_ent,
        mut p_mass,
        p_pos,
        mut p_rad,
        mut comp,
        mut b_body,
        mut opt_tail,
        mut opt_vol,
        mut opt_terra,
        mut opt_diff,
        opt_climate,
        opt_temp,
        opt_sat,
        mut opt_escape_state,
    ) in planets_query.iter_mut()
    {
        if b_body.body_type.is_star_or_remnant() {
            continue;
        }
        let Some((host_star, dist_au, r_hill_au)) =
            resolve_escape_host_and_hill(p_pos.0, p_mass.0, opt_sat, &stars, &body_lookup)
        else {
            continue;
        };

        let body_temp_k = opt_climate.map_or_else(
            || opt_temp.map_or(280.0, |t| t.0),
            |c| f64::from(c.surface_temperature_k),
        );
        let rates = evaluate_body_escape_rates(&EscapeEvalInput {
            body_type: b_body.body_type,
            mass_solar: p_mass.0,
            rad_au: p_rad.0,
            dist_au,
            r_hill_au,
            l_xuv_watts: host_star.l_xuv_watts,
            star_mass_solar: host_star.mass_solar,
            star_lum_solar: host_star.lum_solar,
            body_temp_k,
            comp: &comp,
            opt_vol: opt_vol.as_deref(),
            opt_diff: opt_diff.as_deref(),
            escape_config: &escape_config,
        });

        let was_gas_rich = comp.gas_frac > 0.005;
        let actual_lost_m_earth = apply_escape_mass_loss(
            &EscapeStepContext {
                planet_ent,
                body_temp_k,
                total_loss_rate: rates.total_loss_rate,
                was_gas_rich,
                dt_yr,
                sim_time_yr: sim_time.elapsed_years,
            },
            &mut b_body,
            &mut p_rad,
            &mut p_mass,
            &mut comp,
            &mut opt_vol,
            &mut opt_terra,
            &mut opt_diff,
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
            actual_lost_m_earth,
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

struct EscapeEvalInput<'a> {
    body_type: BodyType,
    mass_solar: f64,
    rad_au: f64,
    dist_au: f64,
    r_hill_au: f64,
    l_xuv_watts: f64,
    star_mass_solar: f64,
    star_lum_solar: f64,
    body_temp_k: f64,
    comp: &'a Composition,
    opt_vol: Option<&'a VolatileInventory>,
    opt_diff: Option<&'a InternalDifferentiation>,
    escape_config: &'a AtmosphericEscapeConfig,
}

fn evaluate_body_escape_rates(input: &EscapeEvalInput<'_>) -> EscapeRatesBundle {
    let xuv_flux_w_m2 = calculate_xuv_flux(input.l_xuv_watts, input.dist_au);
    let (p_ram, _) = calculate_solar_wind_pressure(input.dist_au, input.star_mass_solar);

    let b_gauss = input.opt_diff.map_or(0.0, |d| d.magnetic_field_gauss);
    let r_mp = calculate_magnetopause_radius(input.rad_au, b_gauss, p_ram);
    let shielding = calculate_magnetic_shielding_factor(input.rad_au, r_mp);
    let roche_fill_fraction = (input.rad_au / input.r_hill_au.max(1e-5)).clamp(0.0, 2.0) as f32;

    let has_gas = input.comp.gas_frac > 0.0001;
    let p_bar = input
        .opt_vol
        .map_or(0.0, |v| f64::from(v.atmospheric_pressure_bar));
    let has_atm = p_bar > 0.001;

    let photo_rate = if input.escape_config.enable_photoevaporation && (has_gas || has_atm) {
        let base_photo = calculate_photoevaporation_rate(
            input.mass_solar,
            input.rad_au,
            xuv_flux_w_m2,
            input.escape_config.photo_efficiency_eta,
            input.r_hill_au,
        );
        // Massive secondary N2/CO2 worlds outside 0.25 AU radiatively cool via IR emission;
        // low-gravity worlds (< 0.15 M_earth) lack deep gravity wells to halt H2O photolysis blow-off.
        if !has_gas && input.dist_au >= 0.25 && input.mass_solar >= 0.15 * EARTH_MASS_SOLAR {
            base_photo * (1.0 - f64::from(shielding) * 0.95) * 0.02
        } else if !has_gas {
            base_photo * (1.0 - f64::from(shielding) * 0.90)
        } else {
            base_photo
        }
    } else {
        0.0
    };

    let wind_rate = if input.escape_config.enable_solar_wind_stripping && (has_gas || has_atm) {
        calculate_solar_wind_stripping_rate(input.rad_au, p_ram, shielding, input.mass_solar)
            * input.escape_config.wind_efficiency
    } else {
        0.0
    };

    let jeans_rate = if input.escape_config.enable_jeans_escape && (has_gas || has_atm) {
        let t_exo =
            (input.body_temp_k * 1.5 + xuv_flux_w_m2.max(0.0).sqrt() * 220.0).clamp(50.0, 15_000.0);
        let has_water = input.comp.ice_frac > 0.001
            || input
                .opt_vol
                .is_some_and(|v| v.delivered_water_m_earth > 1e-6);
        // UV photolysis breaks warm H2O vapor on low-gravity worlds into escaping atomic H / OH (mu_eff ~ 4 amu);
        // massive dry N2/CO2 atmospheres remain dominated by heavy ~28 amu molecules.
        let mol_mass_amu = if has_gas {
            2.0
        } else if has_water
            && input.mass_solar < 0.15 * EARTH_MASS_SOLAR
            && input.body_temp_k >= 175.0
        {
            4.0
        } else {
            28.0
        };
        let (_, j_rate) =
            calculate_jeans_escape(input.mass_solar, input.rad_au, t_exo, mol_mass_amu, p_bar);
        j_rate
    } else {
        0.0
    };

    let is_minor_sublimator = matches!(
        input.body_type,
        BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal | BodyType::DustGrain
    ) || (input.mass_solar < 0.05 * EARTH_MASS_SOLAR
        && input.body_temp_k >= 175.0
        && p_bar < 0.02
        && input.comp.ice_frac > 0.05);
    let (comet_tail_len, comet_loss, comet_ion_col) = if is_minor_sublimator {
        calculate_cometary_sublimation(input.dist_au, input.star_lum_solar, input.comp.ice_frac)
    } else {
        (0.0, 0.0, Color::srgba(0.3, 0.7, 1.0, 0.0))
    };

    let total_loss_rate = photo_rate + wind_rate + jeans_rate + f64::from(comet_loss);

    let (regime, tail_length_au, ion_color) = classify_escape_regime(
        photo_rate,
        wind_rate,
        jeans_rate,
        comet_loss,
        roche_fill_fraction,
        input.dist_au,
        input.comp,
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

struct EscapeStepContext {
    planet_ent: Entity,
    body_temp_k: f64,
    total_loss_rate: f64,
    was_gas_rich: bool,
    dt_yr: f64,
    sim_time_yr: f64,
}

fn subtract_volatile_component_mass(
    p_mass: &mut Mass,
    comp: &mut Composition,
    strip_ice: bool,
    delta_m_solar: f64,
    min_bound_ice: f64,
) -> f64 {
    let old_mass = p_mass.0.max(1e-12);
    let m_metal = old_mass * comp.metal_frac;
    let m_silicate = old_mass * comp.silicate_frac;
    let mut m_ice = old_mass * comp.ice_frac;
    let m_organics = old_mass * comp.organics_frac;
    let mut m_gas = old_mass * comp.gas_frac;

    let lost_solar = if strip_ice {
        let excess_ice = old_mass * (comp.ice_frac - min_bound_ice).max(0.0);
        let sublimated = delta_m_solar
            .min(excess_ice)
            .min((old_mass - 1e-12).max(0.0));
        m_ice = (m_ice - sublimated).max(0.0);
        sublimated
    } else {
        let non_gas = m_metal + m_silicate + m_ice + m_organics;
        let core_floor = if non_gas > 1e-12 {
            non_gas
        } else {
            old_mass.min(EARTH_MASS_SOLAR * 0.001)
        };
        let max_strippable = (old_mass - core_floor).max(0.0).min(m_gas);
        let stripped = delta_m_solar.min(max_strippable);
        m_gas = (m_gas - stripped).max(0.0);
        stripped
    };

    if lost_solar > 0.0 {
        let new_total = (m_metal + m_silicate + m_ice + m_organics + m_gas).max(1e-12);
        p_mass.0 = new_total;
        comp.metal_frac = m_metal / new_total;
        comp.silicate_frac = m_silicate / new_total;
        comp.ice_frac = m_ice / new_total;
        comp.organics_frac = m_organics / new_total;
        comp.gas_frac = m_gas / new_total;
        *comp = comp.normalized();
    }

    lost_solar / EARTH_MASS_SOLAR
}

#[allow(
    clippy::too_many_arguments,
    reason = "Applies conserving mass/volatile stripping across gas, ice, volatiles, and terraforming"
)]
fn apply_escape_mass_loss(
    step: &EscapeStepContext,
    body: &mut CelestialBody,
    p_rad: &mut Radius,
    p_mass: &mut Mass,
    comp: &mut Composition,
    opt_vol: &mut Option<Mut<'_, VolatileInventory>>,
    opt_terra: &mut Option<Mut<'_, crate::simulation::terraforming::TerraformingAtmosphere>>,
    opt_diff: &mut Option<Mut<'_, InternalDifferentiation>>,
    stripped_events: &mut MessageWriter<AtmosphericStrippedEvent>,
) -> f64 {
    if step.total_loss_rate <= 0.0 {
        return 0.0;
    }

    let delta_m_earth = step.total_loss_rate * (step.dt_yr / 1.0e6);
    let delta_m_solar = delta_m_earth * EARTH_MASS_SOLAR;
    let prev_gas = comp.gas_frac;
    let prev_ice = comp.ice_frac;
    let mut actual_lost_m_earth = 0.0f64;

    if comp.gas_frac > 0.0 {
        actual_lost_m_earth +=
            subtract_volatile_component_mass(p_mass, comp, false, delta_m_solar, 0.0);

        if step.was_gas_rich && comp.gas_frac < 0.001 {
            if body.body_type != BodyType::Moon {
                body.body_type = crate::simulation::components::classify_body_by_mass_and_comp(
                    p_mass.0, comp, false,
                );
            }
            let p_density = comp.average_density();
            p_rad.0 = crate::simulation::accretion::impact_regimes::radius_from_mass_density(
                p_mass.0, p_density,
            );

            stripped_events.write(AtmosphericStrippedEvent {
                planet: step.planet_ent,
                name: body.name.clone(),
                remaining_mass_earth: p_mass.0 / EARTH_MASS_SOLAR,
                time_yr: step.sim_time_yr,
            });
        }
    } else if step.body_temp_k >= 175.0
        && p_mass.0 < 0.05 * EARTH_MASS_SOLAR
        && comp.ice_frac > 0.05
    {
        let min_bound_ice = if body.body_type == BodyType::Comet {
            0.0
        } else {
            0.05
        };
        actual_lost_m_earth +=
            subtract_volatile_component_mass(p_mass, comp, true, delta_m_solar, min_bound_ice);
    }

    let comp_shifted =
        (comp.gas_frac - prev_gas).abs() > 1e-5 || (comp.ice_frac - prev_ice).abs() > 1e-5;
    if comp_shifted
        && !matches!(
            body.body_type,
            BodyType::GasGiant | BodyType::IceGiant | BodyType::BrownDwarf
        )
    {
        p_rad.0 = crate::simulation::accretion::impact_regimes::radius_from_mass_density(
            p_mass.0,
            comp.average_density(),
        );
        if let Some(diff) = opt_diff {
            diff.recalculate(p_mass.0, p_rad.0, comp);
        }
    }

    if let Some(vol) = opt_vol {
        let is_thin_secondary = !step.was_gas_rich && comp.gas_frac <= 0.001;
        let prev_p = vol.atmospheric_pressure_bar;
        let (pressure_loss, water_loss) = if is_thin_secondary {
            // Partition delta_m_earth across atmosphere vs surface water without double-counting
            let atm_mass = crate::simulation::thermodynamics::gas_mass_for_pressure_bar(
                prev_p, p_mass.0, p_rad.0,
            );
            let water_mass = vol.delivered_water_m_earth.max(0.0);
            let total_vol_mass = atm_mass + water_mass;
            if total_vol_mass > 1e-15 {
                let atm_budget = delta_m_earth * (atm_mass / total_vol_mass);
                let water_budget = delta_m_earth * (water_mass / total_vol_mass);
                let p_loss = crate::simulation::thermodynamics::hydrostatic_pressure_bar(
                    atm_budget, p_mass.0, p_rad.0,
                )
                .min(prev_p);
                let w_loss = water_budget.min(water_mass);
                (p_loss, w_loss)
            } else {
                (0.0, 0.0)
            }
        } else {
            let p_loss = ((delta_m_earth * 2.0) as f32).min(prev_p);
            let w_loss = (delta_m_earth * 0.02).min(vol.delivered_water_m_earth);
            (p_loss, w_loss)
        };
        vol.atmospheric_pressure_bar = (prev_p - pressure_loss).max(0.0);
        vol.delivered_water_m_earth = (vol.delivered_water_m_earth - water_loss).max(0.0);
        if vol.delivered_water_m_earth <= 1e-6 {
            vol.ocean_coverage_frac = 0.0;
        }

        if let Some(terra) = opt_terra.as_deref_mut()
            && prev_p > 1e-6
        {
            let p_keep = (1.0 - (pressure_loss / prev_p)).clamp(0.0, 1.0);
            terra.co2_pressure_bar *= p_keep;
            terra.nitrogen_pressure_bar *= p_keep;
            terra.atmospheric_water_m_earth *= f64::from(p_keep);
            if water_loss > 0.0 {
                let total_terra_w =
                    (terra.surface_liquid_water_m_earth + terra.atmospheric_water_m_earth).max(0.0);
                if total_terra_w > vol.delivered_water_m_earth {
                    let excess = total_terra_w - vol.delivered_water_m_earth;
                    terra.surface_liquid_water_m_earth =
                        (terra.surface_liquid_water_m_earth - excess).max(0.0);
                }
            }
        }

        if actual_lost_m_earth == 0.0 {
            let atm_mass_lost = crate::simulation::thermodynamics::gas_mass_for_pressure_bar(
                pressure_loss,
                p_mass.0,
                p_rad.0,
            );
            actual_lost_m_earth += atm_mass_lost + water_loss;
        }
    }

    actual_lost_m_earth
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
    if let Some(tail) = opt_tail {
        tail.tail_length_au = tail_length_au;
        tail.loss_rate_m_earth_per_myr = loss_rate;
        tail.ion_color = ion_color;
        tail.is_active = is_active;
    } else if is_active && let Ok(mut entity_cmd) = commands.get_entity(planet_ent) {
        entity_cmd.try_insert(AtmosphericEscapeTail {
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
    actual_mass_lost_m_earth: f64,
) {
    let mass_lost_step = actual_mass_lost_m_earth.max(0.0);

    if let Some(st) = opt_escape_state {
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
    } else if (rates.total_loss_rate > 0.0 || mass_lost_step > 0.0)
        && let Ok(mut entity_cmd) = commands.get_entity(planet_ent)
    {
        entity_cmd.try_insert(AtmosphericEscapeState {
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
