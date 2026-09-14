//! Direct nebular gas accretion and Little Red Dot / Quasi-Star dynamics.

use bevy::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Calculates the astrophysical mass capacity for circum-nuclear bodies
/// based on the local gas surface density of the ring at orbital radius `r_au`.
pub fn circum_nuclear_ring_mass_capacity(r_au: f64, r_in: f64, r_out: f64) -> f64 {
    const INNER_CAPACITY_SOLAR: f64 = 1000.0;
    const OUTER_CAPACITY_SOLAR: f64 = 5.0;

    let span = (r_out - r_in).max(1.0);
    let norm_dist = ((r_au - r_in) / span).clamp(0.0, 1.0);
    let density_fraction = (1.0 - norm_dist).powf(1.65);

    OUTER_CAPACITY_SOLAR + (INNER_CAPACITY_SOLAR - OUTER_CAPACITY_SOLAR) * density_fraction
}

fn calculate_gas_capacity_limits(
    r_au: f64,
    is_massive_disk: bool,
    disk_params: &DiskParameters,
) -> (f64, f64, f64) {
    if is_massive_disk {
        let ring_limit = circum_nuclear_ring_mass_capacity(
            r_au,
            disk_params.inner_radius_au,
            disk_params.outer_radius_au,
        );
        (ring_limit, 1.0, 5.0)
    } else if r_au < 2.0 {
        // Inner terrestrial zone: strictly thin secondary atmospheres (max ~1.05 M_earth total)
        (1.05 * EARTH_MASS_SOLAR, 0.035, 100.0)
    } else if r_au < 2.7 {
        // Asteroid belt: negligible gas capture, prevent runaway planet formation
        (0.005 * EARTH_MASS_SOLAR, 0.010, 100.0)
    } else if r_au < 5.0 {
        (0.5 * EARTH_MASS_SOLAR, 0.045, 100.0)
    } else if r_au < 12.0 {
        (JUPITER_MASS_SOLAR * 1.5, 0.94, 0.5)
    } else if r_au < 22.0 {
        (JUPITER_MASS_SOLAR * 0.45, 0.88, 0.4)
    } else if r_au < 36.0 {
        (20.0 * EARTH_MASS_SOLAR, 0.22, 0.3)
    } else if r_au < 50.0 {
        (22.0 * EARTH_MASS_SOLAR, 0.22, 0.3)
    } else {
        (0.05 * EARTH_MASS_SOLAR, 0.02, 100.0)
    }
}

fn calculate_local_gas_density(
    r_au: f64,
    is_massive_disk: bool,
    is_ignited: bool,
    gas_scale: f64,
    disk_params: &DiskParameters,
) -> f64 {
    if is_massive_disk {
        0.0025 * (disk_params.outer_radius_au / r_au).powf(0.5) * gas_scale
    } else if r_au < 2.7 {
        if is_ignited {
            1.2e-4 * (r_au / 1.0).powf(-1.50) * (gas_scale * 0.05)
        } else {
            1.2e-4 * (r_au / 1.0).powf(-1.50) * gas_scale
        }
    } else if r_au < 12.0 {
        if is_ignited {
            1.2e-4 * (r_au / 1.0).powf(-1.50) * gas_scale * 2.5
        } else {
            1.2e-4 * (r_au / 1.0).powf(-1.50) * gas_scale
        }
    } else {
        1.2e-4 * (r_au / 1.0).powf(-1.50) * gas_scale
    }
}

struct AccretionEnvParams<'a> {
    config: &'a SimulationConfig,
    dt_yr: f64,
    star_mass: f64,
    is_massive_disk: bool,
    local_gas_density: f64,
    max_gas_mass: f64,
    max_gas_frac: f64,
    runaway_threshold_m_earth: f64,
}

fn calculate_gas_growth_step(
    env: &AccretionEnvParams,
    m: f64,
    r_au: f64,
    body_type: BodyType,
    comp_gas_frac: f64,
) -> f64 {
    let r_hill = r_au * (m / (3.0 * env.star_mass)).cbrt();
    let r_capture = if env.is_massive_disk {
        r_hill.clamp(0.002, 3.5)
    } else {
        r_hill
    };
    let omega_k = (G_ASTRO * env.star_mass / (r_au * r_au * r_au)).sqrt();

    let m_earth = m / EARTH_MASS_SOLAR;
    let is_runaway = m_earth >= env.runaway_threshold_m_earth;
    let runaway_boost = if env.is_massive_disk {
        if body_type == BodyType::BlackHole || m >= 0.08 {
            let eddington_suppression = (1.0 - (m / env.max_gas_mass)).clamp(0.01, 1.0);
            0.20 * eddington_suppression
        } else {
            (1.0 + 0.15 * m_earth.clamp(1.0, 2500.0).powf(0.20)).min(3.0)
        }
    } else if is_runaway {
        (1.0 + (m_earth / 5.0).powf(1.4)).min(40.0)
    } else if r_au < 2.7 {
        (0.15 + 0.10 * m_earth).clamp(0.08, 0.40)
    } else {
        0.05
    };

    let gap_factor = (1.0 - (m / env.max_gas_mass)).clamp(0.0, 1.0);
    let c_gas = if env.is_massive_disk {
        15.0 * (f64::from(env.config.accretion_rate_multiplier) / 120.0)
    } else {
        180.0 * (f64::from(env.config.accretion_rate_multiplier) / 120.0)
    };

    let max_annual_growth_rate = if env.is_massive_disk {
        if m >= 10.0 {
            0.015
        } else {
            0.04
        }
    } else if r_au < 2.7 {
        0.005
    } else {
        0.02
    };
    let max_step_growth = (m * max_annual_growth_rate * env.dt_yr).max(1e-12 * env.dt_yr);

    let remaining_gas_capacity = if !env.is_massive_disk && r_au < 5.0 {
        let max_g = m * env.max_gas_frac;
        let current_g = m * comp_gas_frac;
        let frac_remaining = (max_g - current_g).max(0.0);
        let mass_remaining = (env.max_gas_mass - m).max(0.0);
        frac_remaining.min(mass_remaining)
    } else {
        (env.max_gas_mass - m).max(0.0)
    };

    (c_gas
        * r_capture
        * r_capture
        * env.local_gas_density
        * omega_k
        * env.dt_yr
        * gap_factor
        * runaway_boost)
        .min(max_step_growth)
        .min(remaining_gas_capacity)
}

fn apply_gas_accretion_to_body(
    d_mass_gas: f64,
    r_au: f64,
    is_massive_disk: bool,
    max_gas_frac: f64,
    mass: &mut Mass,
    rad: &mut Radius,
    comp: &mut Composition,
    body: &mut CelestialBody,
    opt_diff: Option<&mut InternalDifferentiation>,
    opt_spin: Option<&mut SpinState>,
    opt_vol: Option<&mut VolatileInventory>,
    opt_temp: Option<&mut Temperature>,
) {
    let old_mass = mass.0;
    let new_mass = old_mass + d_mass_gas;
    mass.0 = new_mass;

    *comp = comp.mass_weighted_merge(old_mass, &Composition::solar_gas(), d_mass_gas);
    if !is_massive_disk && r_au < 2.7 {
        comp.gas_frac = comp.gas_frac.min(max_gas_frac);
    }

    let updated_type = if body.body_type == BodyType::BlackHole {
        BodyType::BlackHole
    } else {
        classify_body_by_mass_and_comp(new_mass, comp, false)
    };
    body.body_type = updated_type;

    let new_radius = if updated_type == BodyType::BlackHole {
        (1.97e-8 * new_mass).max(1e-6)
    } else if new_mass >= 0.08 {
        (0.00465 * (new_mass / 1.0).powf(0.8)).clamp(0.003, 10.0)
    } else {
        let density = comp.average_density();
        let volume = new_mass / density;
        ((3.0 * volume) / (4.0 * PI))
            .cbrt()
            .max(EARTH_RADIUS_AU * 0.2)
    };
    rad.0 = new_radius;

    update_gas_accretion_body_name(body, updated_type, new_mass, r_au);

    if let Some(temp) = opt_temp {
        update_stellar_surface_temp(&mut temp.0, new_mass);
    }
    if let Some(diff) = opt_diff {
        diff.recalculate(new_mass, new_radius, comp);
    }
    if let Some(spin) = opt_spin {
        let spin_vec = spin.spin_vector;
        spin.update_from_spin(spin_vec, new_mass, new_radius);
    }
    if let Some(vol) = opt_vol {
        let gas_growth = d_mass_gas / EARTH_MASS_SOLAR;
        let pressure_scale = if r_au < 2.7 { 100.0 } else { 400.0 };
        vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar
            + (gas_growth * pressure_scale) as f32)
            .clamp(0.01, if r_au < 2.7 { 95.0 } else { 1000.0 });
    }
}

fn update_stellar_surface_temp(temp: &mut f64, new_mass: f64) {
    if new_mass >= 25.0 {
        *temp = 35_000.0;
    } else if new_mass >= 8.0 {
        *temp = 20_000.0;
    } else if new_mass >= 1.4 {
        *temp = 9_500.0;
    } else if new_mass >= 0.5 {
        *temp = 5_800.0;
    } else if new_mass >= 0.08 {
        *temp = 3_200.0;
    } else if new_mass >= 13.0 * JUPITER_MASS_SOLAR {
        *temp = 1_800.0;
    }
}

fn update_gas_accretion_body_name(
    body: &mut CelestialBody,
    updated_type: BodyType,
    new_mass: f64,
    r_au: f64,
) {
    let is_canonical_solar = body.name == "Earth"
        || body.name == "Venus"
        || body.name == "Mars"
        || body.name == "Mercury"
        || body.name.starts_with("Proto-")
        || body.name.starts_with("Theia")
        || body.name == "Jupiter"
        || body.name == "Saturn"
        || body.name == "Uranus"
        || body.name == "Neptune";

    if !is_canonical_solar {
        body.name = match updated_type {
            BodyType::BlackHole => {
                if new_mass >= 100.0 {
                    format!("Intermediate Black Hole ({new_mass:.1} M☉)")
                } else {
                    format!("Orbiting Stellar Black Hole ({new_mass:.1} M☉)")
                }
            }
            BodyType::Hypergiant => format!("Pop-III Hypergiant ({new_mass:.1} M☉)"),
            BodyType::BlueSupergiant => format!("Pop-III Blue Supergiant ({new_mass:.1} M☉)"),
            BodyType::BlueGiant => format!("Pop-III Blue Giant ({new_mass:.1} M☉)"),
            BodyType::YellowDwarf => format!("Pop-III Yellow Star ({new_mass:.2} M☉)"),
            BodyType::RedDwarf => format!("Red Dwarf ({new_mass:.2} M☉)"),
            BodyType::BrownDwarf => {
                format!("Brown Dwarf ({:.1} M_J)", new_mass / JUPITER_MASS_SOLAR)
            }
            BodyType::GasGiant => {
                if new_mass >= JUPITER_MASS_SOLAR {
                    format!("Super-Jupiter ({:.1} M_J)", new_mass / JUPITER_MASS_SOLAR)
                } else {
                    format!("Planet-{r_au:.0}AU (Gas Giant)")
                }
            }
            BodyType::IceGiant => format!("Planet-{r_au:.0}AU (Ice Giant)"),
            BodyType::SuperEarth => format!("Planet-{r_au:.0}AU (Super-Earth)"),
            BodyType::TerrestrialPlanet => format!("Planet-{r_au:.0}AU (Terrestrial)"),
            _ => body.name.clone(),
        };
    }
}

/// Accretes gas envelope onto protoplanetary cores / stellar seeds from the ambient gas disk.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Nebular gas accretion involves many simulation components and per-body state"
)]
pub fn direct_nebular_gas_accretion(
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    disk_params: Res<DiskParameters>,
    star_query: Query<&IgnitionState, With<CentralStar>>,
    mut bodies_query: Query<
        (
            Entity,
            &mut Mass,
            &SimPosition,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&mut SpinState>,
            Option<&mut VolatileInventory>,
            Option<&mut Temperature>,
        ),
        Without<CentralStar>,
    >,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let gas_scale = f64::from(config.gas_density_scale);
    if gas_scale <= 0.001 || sim_time.elapsed_years > disk_params.gas_disk_lifetime_yr {
        return;
    }

    let is_ignited = star_query.iter().next().is_some_and(|ig| ig.is_ignited);
    let dt_yr = (config.base_dt_yr * time_warp.multiplier.max(0.01)).min(10.0);
    let star_mass = disk_params.central_star_mass;
    let is_massive_disk = star_mass > 10.0 || disk_params.outer_radius_au > 100.0;

    for (
        _entity,
        mut mass,
        pos,
        mut rad,
        mut comp,
        mut body,
        mut opt_diff,
        mut opt_spin,
        mut opt_vol,
        mut opt_temp,
    ) in bodies_query.iter_mut()
    {
        let actual_r = pos.0.length();
        if actual_r > disk_params.outer_radius_au || actual_r < disk_params.inner_radius_au {
            continue;
        }
        let r_au = actual_r;
        let m = mass.0;

        let (max_gas_mass, max_gas_frac, runaway_threshold_m_earth) =
            calculate_gas_capacity_limits(r_au, is_massive_disk, &disk_params);

        if m >= max_gas_mass {
            continue;
        }
        if !is_massive_disk && r_au < 2.7 && comp.gas_frac >= max_gas_frac {
            continue;
        }

        let local_gas_density =
            calculate_local_gas_density(r_au, is_massive_disk, is_ignited, gas_scale, &disk_params);

        let env = AccretionEnvParams {
            config: &config,
            dt_yr,
            star_mass,
            is_massive_disk,
            local_gas_density,
            max_gas_mass,
            max_gas_frac,
            runaway_threshold_m_earth,
        };

        let d_mass_gas = calculate_gas_growth_step(&env, m, r_au, body.body_type, comp.gas_frac);

        if d_mass_gas > 1e-16 {
            apply_gas_accretion_to_body(
                d_mass_gas,
                r_au,
                is_massive_disk,
                max_gas_frac,
                &mut mass,
                &mut rad,
                &mut comp,
                &mut body,
                opt_diff.as_deref_mut(),
                opt_spin.as_deref_mut(),
                opt_vol.as_deref_mut(),
                opt_temp.as_deref_mut(),
            );
        }
    }
}

/// Updates the internal dynamics, super-Eddington accretion, cocoon blowout,
/// and tidal disruptions for a JWST Little Red Dot (Black Hole Star / Quasi-Star).
pub fn update_black_hole_star_dynamics(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut quasi_star_query: Query<(
        Entity,
        &mut BlackHoleStarState,
        &mut Mass,
        &mut Radius,
        &mut Temperature,
        &mut Luminosity,
        &mut CelestialBody,
        &SimPosition,
    )>,
    mut satellites_query: Query<
        (
            Entity,
            &mut Mass,
            &mut SimVelocity,
            &SimPosition,
            &Radius,
            &CelestialBody,
        ),
        Without<BlackHoleStarState>,
    >,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    let dt = sim_time.current_dt_yr;
    if dt <= 0.0 {
        return;
    }

    for (_qs_ent, mut state, mut mass, mut radius, mut temp, mut lum, mut body, qs_pos) in
        quasi_star_query.iter_mut()
    {
        if state.super_eddington_active && state.cocoon_mass_solar > 10.0 {
            let m_bh = state.black_hole_mass_solar;
            let m_dot_edd = 2.2e-8 * m_bh;
            let actual_rate = m_dot_edd * state.eddington_ratio;
            let dm = (actual_rate * dt * 50.0).min(state.cocoon_mass_solar);

            state.black_hole_mass_solar += dm;
            state.cocoon_mass_solar -= dm;
            state.accreted_envelope_mass += dm;
            mass.0 = state.total_mass_solar();
        }

        if state.is_blown_out {
            state.blowout_progress = (state.blowout_progress + 0.45 * dt as f32).min(1.0);
            state.jet_travel_distance_au += crate::utils::constants::SPEED_OF_LIGHT_AU_YR * dt;
            let p = f64::from(state.blowout_progress);

            if p < 0.5 {
                radius.0 = 60.0 * (1.0 + p * 2.0);
                temp.0 = (3800.0 * (1.0 - p * 0.4)).max(1500.0);
            } else {
                let quasar_factor = (p - 0.5) * 2.0;
                radius.0 = 60.0 * (1.0 - quasar_factor) + 8.0 * quasar_factor;
                temp.0 = 3800.0 * (1.0 - quasar_factor) + 95000.0 * quasar_factor;
                lum.0 = 1.2e7 * (1.0 - quasar_factor) + 1.5e10 * quasar_factor;

                body.body_type = BodyType::BlackHole;
                body.name = format!(
                    "Supermassive Quasar ({:.0} M☉)",
                    state.black_hole_mass_solar
                );
                state.super_eddington_active = true;
            }
        }

        let bh_m = state.black_hole_mass_solar;
        let cocoon_r = radius.0;

        for (sat_ent, sat_m, mut sat_vel, sat_pos, sat_rad, sat_body) in satellites_query.iter_mut()
        {
            let rel_pos = sat_pos.0 - qs_pos.0;
            let dist = rel_pos.length();

            if dist < cocoon_r && !state.is_blown_out {
                let v_dir = sat_vel.0.normalize_or_zero();
                let drag = 0.08 * (1.0 - dist / cocoon_r).powf(1.5) * dt;
                sat_vel.0 -= v_dir * drag;
            }

            let m_ratio = (bh_m / sat_m.0.max(0.01)).cbrt();
            let r_tidal = (sat_rad.0 * m_ratio).clamp(0.05, 5.0);

            if dist < r_tidal || dist < 0.50 {
                state.cocoon_mass_solar += sat_m.0 * 0.5;
                state.black_hole_mass_solar += sat_m.0 * 0.5;
                mass.0 = state.total_mass_solar();
                lum.0 += 5.0e7;

                info!(
                    "💥 TIDAL DISRUPTION EVENT: '{}' shredded by the 100,000 M☉ Supermassive Black Hole Seed!",
                    sat_body.name
                );

                if let Ok(mut e_cmd) = commands.get_entity(sat_ent) {
                    e_cmd.despawn();
                }
            }
        }
    }
}
