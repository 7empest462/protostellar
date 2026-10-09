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
    } else if r_au < 11.5 {
        // Gas Giant zone (Jupiter 5.2 AU & Saturn 9.5 AU)
        (JUPITER_MASS_SOLAR * 1.5, 0.94, 10.0)
    } else if r_au < 45.0 {
        // Ice Giant zone (Uranus 19.2 AU, Neptune 30.0 AU, Planet Nine)
        // Gas envelope is strictly limited to ~15-20% by mass over heavy volatile ice/rock mantle
        (22.0 * EARTH_MASS_SOLAR, 0.20, 12.0)
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
    let host_mass = if env.star_mass <= 0.001 {
        24.0
    } else {
        env.star_mass
    };
    let r_hill = r_au * (m / (3.0 * host_mass)).cbrt();
    let r_capture = if env.is_massive_disk {
        r_hill.clamp(0.002, 3.5)
    } else {
        r_hill
    };
    let omega_k = (G_ASTRO * host_mass / (r_au * r_au * r_au)).sqrt();

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

    let remaining_gas_capacity = if env.is_massive_disk {
        (env.max_gas_mass - m).max(0.0)
    } else {
        let max_g = m * env.max_gas_frac;
        let current_g = m * comp_gas_frac;
        let frac_remaining = (max_g - current_g).max(0.0);
        let mass_remaining = (env.max_gas_mass - m).max(0.0);
        frac_remaining.min(mass_remaining)
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
    if !is_massive_disk {
        comp.gas_frac = comp.gas_frac.min(max_gas_frac);
    }

    let updated_type = if body.body_type == BodyType::BlackHole {
        BodyType::BlackHole
    } else if body.body_type == BodyType::Moon {
        BodyType::Moon
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
        || body.name == "The Moon"
        || body.name.contains("Moon")
        || body.body_type == BodyType::Moon
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
    mut commands: Commands,
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
            Option<&IgnitionState>,
        ),
        (
            Without<CentralStar>,
            Without<crate::simulation::terraforming::BombardmentProjectile>,
        ),
    >,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let gas_scale = f64::from(config.gas_density_scale);
    if gas_scale <= 0.001
        || sim_time.elapsed_years > disk_params.gas_disk_lifetime_yr
        || disk_params.disk_mass <= 1e-6
    {
        return;
    }

    let is_ignited = star_query.iter().next().is_some_and(|ig| ig.is_ignited);
    let dt_yr = (config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)).min(10.0);
    let star_mass = disk_params.central_star_mass;
    let is_massive_disk =
        disk_params.disk_mass > 0.001 && (star_mass > 10.0 || disk_params.outer_radius_au > 100.0);

    for (
        entity,
        mut mass,
        pos,
        mut rad,
        mut comp,
        mut body,
        mut opt_diff,
        mut opt_spin,
        mut opt_vol,
        mut opt_temp,
        opt_ign,
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
        if !is_massive_disk && comp.gas_frac >= max_gas_frac {
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

            // If body accreted enough gas to become a star, ignite it (stars only, never remnants)!
            if body.body_type.is_star_or_remnant()
                && !body.body_type.is_remnant()
                && opt_ign.is_none()
            {
                commands.entity(entity).insert((
                    IgnitionState {
                        core_temperature: 1.2e7,
                        fusion_fraction: 1.0,
                        is_ignited: true,
                        shockwave_radius: 0.0,
                    },
                    StellarEvolutionState::default(),
                    Luminosity((mass.0 * 2.5).max(0.5)),
                ));
                bevy::log::info!(
                    "🔥 {} has accreted enough gas to ignite into a star!",
                    body.name
                );
            }
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
                    e_cmd.try_despawn();
                }
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Acquisition logic needs many parameters"
)]
fn compute_protostar_bondi_hoyle_accretion(
    pos: &SimPosition,
    vel: &SimVelocity,
    mass: &mut Mass,
    rad: &mut Radius,
    body: &mut CelestialBody,
    opt_lum: &mut Option<Mut<Luminosity>>,
    opt_ign: Option<&IgnitionState>,
    dt_yr: f64,
    gas_scale: f64,
) {
    let m = mass.0;
    let r_cloud = pos.0.length();
    if r_cloud < 600.0 {
        // Plummer density profile of the GMC core and extended galactic disk
        let rho_0 = 3.4e-11;
        let r_core = 240.0;
        let profile = 1.0 / (1.0 + (r_cloud / r_core).powi(2)).powf(1.15);
        let rho_cloud = rho_0 * profile * gas_scale;

        let c_s = 0.058; // AU/yr sound speed (~0.27 km/s)
        let v_rel = vel.0.length();
        // Gas co-rotates in the galactic disk; velocity relative to local gas is dominated
        // by turbulent dispersion (~0.20 - 0.45 AU/yr) and minor orbital eccentricity
        let v_turb = 0.22f64; // AU/yr (~1.0 km/s turbulent dispersion)
        let v_eff = (c_s * c_s + v_turb * v_turb + (v_rel * 0.04).powi(2))
            .sqrt()
            .max(0.10);

        // Bondi-Hoyle gas accretion from GMC core and spiral arms:
        // Stars accrete gas proportional to M in dense regions, fueling rapid pre-supernova growth
        let base_bondi = 0.015 * (m * m) / (v_eff * v_eff * v_eff);
        let max_rate = (0.045 * m).clamp(0.025, 0.65);
        let bondi_rate = (base_bondi * (rho_cloud / rho_0)).clamp(0.002, max_rate);

        // Radiation pressure suppression for massive ignited stars (blowout limit):
        // Pop III stars in dense galactic disks grow up to ~100-120 M_sun before radiation cutoff
        let is_ignited = opt_ign.is_some_and(|ig| ig.is_ignited);
        let rad_suppression = if is_ignited {
            let lum_val = opt_lum.as_deref().map_or(1.0, |l| l.0);
            if lum_val > 2_500_000.0 || m >= 120.0 {
                0.0
            } else {
                (1.0 - lum_val / 2_500_000.0).clamp(0.0, 1.0)
            }
        } else {
            1.0
        };

        let d_mass = bondi_rate * rad_suppression * dt_yr;
        if d_mass > 1e-12 {
            let new_m = m + d_mass;
            mass.0 = new_m;

            // Physical radius update
            if body.body_type == BodyType::Protostar {
                rad.0 = (3.5 * SOLAR_RADIUS_AU * (new_m / 1.0).powf(0.5)).clamp(0.005, 0.06);
            } else if new_m >= 0.08 {
                rad.0 = (SOLAR_RADIUS_AU * new_m.powf(0.8)).clamp(0.002, 0.20);
            }

            if let Some(ref mut lum) = opt_lum {
                if body.body_type == BodyType::Protostar {
                    lum.0 = (new_m * 2.5).max(0.5);
                }
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Acquisition logic needs many parameters"
)]
fn compute_protoplanet_envelope_accretion(
    entity: Entity,
    pos: &SimPosition,
    mass: &mut Mass,
    rad: &mut Radius,
    comp: &mut Composition,
    body: &mut CelestialBody,
    opt_vol: &mut Option<Mut<VolatileInventory>>,
    dt_yr: f64,
    stars: &[(Entity, bevy::math::DVec3, f64)],
) {
    let m = mass.0;
    // B. PROTOPLANET ACCRETION FROM CIRCUMSTELLAR ENVELOPE & GMC NEBULA
    // Find closest host star
    let closest_star = stars
        .iter()
        .filter(|(s_ent, _, _)| *s_ent != entity)
        .min_by(|(_, p1, _), (_, p2, _)| {
            let d1 = (*p1 - pos.0).length_squared();
            let d2 = (*p2 - pos.0).length_squared();
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        });

    if let Some((_, star_pos, star_m)) = closest_star {
        let a_au = (pos.0 - *star_pos).length().clamp(0.4, 50.0);
        let omega_k = (G_ASTRO * star_m / (a_au * a_au * a_au)).sqrt();

        let is_gas_giant = comp.gas_frac > 0.40
            || body.body_type == BodyType::GasGiant
            || body.body_type == BodyType::BrownDwarf;

        let max_planet_m = if is_gas_giant {
            0.015 // ~15 M_Jupiter
        } else if comp.ice_frac > 0.20 {
            0.000_060 // ~20 M_Earth (Sub-Neptune / Water World)
        } else {
            0.000_035 // ~12 M_Earth (Rocky Super-Earth)
        };

        if m < max_planet_m {
            let growth_coef = if is_gas_giant { 0.00018 } else { 0.000_008 };
            let d_mass = (growth_coef * m.powf(0.5) * omega_k * dt_yr).min(max_planet_m - m);

            if d_mass > 1e-12 {
                let new_m = m + d_mass;
                mass.0 = new_m;

                // Nebular composition blend from molecular cloud gas and icy volatiles
                let nebular_feed = Composition::solar_gas();
                *comp = comp.mass_weighted_merge(m, &nebular_feed, d_mass);

                if !is_gas_giant {
                    // In a GMC, rocky/ocean planets retain a primordial atmosphere
                    // clamped to physical terrestrial/sub-Neptune envelope limits (2% - 8%)
                    let max_gas = 0.08;
                    if comp.gas_frac > max_gas {
                        let excess = comp.gas_frac - max_gas;
                        comp.gas_frac = max_gas;
                        comp.silicate_frac += excess;
                    } else if comp.gas_frac < 0.02 {
                        let deficit = 0.02 - comp.gas_frac;
                        comp.gas_frac = 0.02;
                        comp.silicate_frac = (comp.silicate_frac - deficit).max(0.1);
                    }
                    *comp = comp.normalized();
                    comp.gas_frac = comp.gas_frac.clamp(0.02, 0.08);
                }

                // Update physical radius based on bulk density and degenerate envelope
                if is_gas_giant {
                    // Gas giant degenerate radius ~1.0-1.6 R_Jup
                    rad.0 = (0.000_477 * (new_m / 0.000_954).powf(0.08)).clamp(0.00035, 0.00085);
                } else {
                    // Rocky terrestrial density
                    let density = comp.average_density();
                    let volume = new_m / density;
                    rad.0 = ((3.0 * volume) / (4.0 * PI))
                        .cbrt()
                        .max(EARTH_RADIUS_AU * 0.3);
                }

                // Update volatile inventory and atmospheric pressure
                if let Some(ref mut vol) = opt_vol {
                    let d_mass_earth = d_mass / EARTH_MASS_SOLAR;
                    let gas_added = d_mass_earth * comp.gas_frac;
                    let water_added = d_mass_earth * comp.ice_frac;

                    vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar
                        + (gas_added * 100.0) as f32)
                        .clamp(0.1, if is_gas_giant { 2000.0 } else { 120.0 });

                    vol.delivered_water_m_earth += water_added;
                    if !is_gas_giant
                        && vol.delivered_water_m_earth > 0.0001
                        && vol.ocean_coverage_frac < 0.1
                    {
                        vol.ocean_coverage_frac =
                            (vol.delivered_water_m_earth / 0.005).clamp(0.0, 0.95) as f32;
                    }
                }

                // Re-classify Protoplanet into its mature body type
                if body.body_type == BodyType::Protoplanet {
                    body.body_type = classify_body_by_mass_and_comp(new_m, comp, false);
                }
            }
        }
    }
}

/// Gas accretion in 3D Giant Molecular Clouds (GMC):
/// 1. Protostars accrete gas from the surrounding molecular cloud core via Bondi-Hoyle-Littleton accretion,
///    growing mass until radiation pressure blowout (Eddington luminosity balance).
/// 2. Protoplanets accrete from their host protostar's circumstellar envelope/disk,
///    allowing gas giants and terrestrial worlds to grow toward full planetary mass.
#[allow(clippy::type_complexity, reason = "GMC cluster multi-body accretion")]
pub fn gmc_cluster_gas_accretion(
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    sim_time: Option<Res<crate::simulation::resources::SimTime>>,
    mut bodies_query: Query<(
        Entity,
        &mut Mass,
        &SimPosition,
        &SimVelocity,
        &mut Radius,
        &mut Composition,
        &mut CelestialBody,
        Option<&mut Luminosity>,
        Option<&mut Temperature>,
        Option<&IgnitionState>,
        Option<&mut VolatileInventory>,
        Option<&mut crate::simulation::components::RelativisticJetState>,
    )>,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    let is_gmc = scenario_state.as_deref().is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster
    });
    if !is_gmc {
        return;
    }

    let gas_scale = f64::from(config.gas_density_scale);
    if gas_scale <= 0.001 {
        return;
    }

    let dt_yr = sim_time.as_deref().map_or_else(
        || (config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)).min(5.0),
        |st| st.current_dt_yr.clamp(1e-6, 5.0),
    );

    // 1. Collect all star positions and masses for planetary envelope accretion
    let stars: Vec<(Entity, bevy::math::DVec3, f64)> = bodies_query
        .iter()
        .filter(|(_, _, _, _, _, _, body, _, _, _, _, _)| body.body_type.is_star_or_remnant())
        .map(|(e, m, pos, _, _, _, _, _, _, _, _, _)| (e, pos.0, m.0))
        .collect();

    for (
        entity,
        mut mass,
        pos,
        vel,
        mut rad,
        mut comp,
        mut body,
        mut opt_lum,
        _opt_temp,
        opt_ign,
        mut opt_vol,
        opt_jet,
    ) in bodies_query.iter_mut()
    {
        let m = mass.0;

        // A. PROTOSTAR & STELLAR BONDI-HOYLE ACCRETION FROM CLOUD CORE
        if body.body_type == BodyType::Protostar
            || (body.body_type.is_star_or_remnant() && !body.body_type.is_remnant())
        {
            compute_protostar_bondi_hoyle_accretion(
                pos,
                vel,
                &mut mass,
                &mut rad,
                &mut body,
                &mut opt_lum,
                opt_ign,
                dt_yr,
                gas_scale,
            );
        } else if body.body_type == BodyType::BlackHole {
            // High-rate Bondi-Hoyle + Eddington accretion for Black Holes in GMC cores
            let r_bh = pos.0.length();
            let rho_bh = if r_bh < 350.0 {
                let r_c = 140.0;
                let prof = 1.0 / (1.0 + (r_bh / r_c).powi(2)).powf(1.5);
                3.4e-11 * prof * gas_scale
            } else {
                1.5e-13 * gas_scale
            };
            let c_s = 0.058;
            let v_bh = vel.0.length();
            let v_eff = (c_s * c_s + v_bh * v_bh).sqrt().max(0.04);
            let bondi_bh = (19500.0 * m * m / (v_eff * v_eff * v_eff) * rho_bh).clamp(1.0e-5, 0.15);
            let edd_rate = 2.2e-7 * m;
            let d_mass = (bondi_bh + edd_rate) * dt_yr;
            if d_mass > 1e-12 {
                mass.0 = m + d_mass;
                rad.0 = (1.974e-8 * mass.0).max(1e-7);
                if let Some(mut jet) = opt_jet {
                    jet.is_accreting = true;
                    jet.accretion_timer_years = 5.0; // Keep active for 5 years after eating gas
                }
            }
        } else if body.body_type == BodyType::Protoplanet || body.body_type.is_planet() {
            compute_protoplanet_envelope_accretion(
                entity,
                pos,
                &mut mass,
                &mut rad,
                &mut comp,
                &mut body,
                &mut opt_vol,
                dt_yr,
                &stars,
            );
        }
    }
}
