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
        if m >= 10.0 { 0.015 } else { 0.04 }
    } else if r_au < 2.7 {
        0.005
    } else {
        0.02
    };
    let max_step_growth = (m * max_annual_growth_rate * env.dt_yr).max(1e-12 * env.dt_yr);

    let remaining_gas_capacity = if env.is_massive_disk {
        (env.max_gas_mass - m).max(0.0)
    } else {
        let frac_remaining =
            (m * (env.max_gas_frac - comp_gas_frac) / (1.0 - env.max_gas_frac).max(1e-6)).max(0.0);
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

pub(crate) fn ice_giant_bulk_density_astro(comp: &Composition) -> f64 {
    // Under ~100 GPa interior pressures, Ice Giant H2/He envelopes compress to ~1,400 kg/m^3
    const DENSITY_ICE_GIANT_GAS_ASTRO: f64 =
        (1400.0 * AU_TO_METERS * AU_TO_METERS * AU_TO_METERS) / SOLAR_MASS_KG;
    let inv_density = (comp.metal_frac / DENSITY_IRON_ASTRO)
        + (comp.silicate_frac / DENSITY_ROCK_ASTRO)
        + (comp.ice_frac / DENSITY_ICE_ASTRO)
        + (comp.organics_frac / DENSITY_ORGANICS_ASTRO)
        + (comp.gas_frac / DENSITY_ICE_GIANT_GAS_ASTRO);
    if inv_density > 0.0 {
        1.0 / inv_density
    } else {
        DENSITY_ICE_ASTRO
    }
}

fn apply_gas_accretion_to_body(
    d_mass_gas: f64,
    r_au: f64,
    _is_massive_disk: bool,
    _max_gas_frac: f64,
    mass: &mut Mass,
    rad: &mut Radius,
    comp: &mut Composition,
    body: &mut CelestialBody,
    opt_diff: Option<&mut InternalDifferentiation>,
    opt_spin: Option<&mut SpinState>,
    opt_vol: Option<&mut VolatileInventory>,
    opt_temp: Option<&mut Temperature>,
    opt_rings: Option<&mut PlanetaryRingSystem>,
) {
    let old_mass = mass.0;
    let old_radius = rad.0;
    let prev_type = body.body_type;
    let new_mass = old_mass + d_mass_gas;
    mass.0 = new_mass;

    *comp = comp
        .mass_weighted_merge(old_mass, &Composition::solar_gas(), d_mass_gas)
        .normalized();

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
    } else if updated_type == BodyType::GasGiant || updated_type == BodyType::BrownDwarf {
        (0.000_477 * (new_mass / 0.000_954).powf(0.08)).clamp(0.00035, 0.00085)
    } else if updated_type == BodyType::IceGiant {
        let density = ice_giant_bulk_density_astro(comp);
        let volume = new_mass / density;
        ((3.0 * volume) / (4.0 * PI))
            .cbrt()
            .max(EARTH_RADIUS_AU * 1.5)
    } else {
        let density = comp.average_density();
        let volume = new_mass / density;
        ((3.0 * volume) / (4.0 * PI))
            .cbrt()
            .max(EARTH_RADIUS_AU * 0.2)
    };
    rad.0 = new_radius;
    if let Some(rings) = opt_rings {
        rings.scale_with_host_radius(old_radius, new_radius);
    }

    update_gas_accretion_body_name(body, prev_type, updated_type, new_mass, r_au);

    if updated_type != BodyType::BlackHole
        && let Some(temp) = opt_temp
    {
        update_stellar_surface_temp(&mut temp.0, new_mass);
    }
    if let Some(diff) = opt_diff {
        diff.recalculate(new_mass, new_radius, comp);
    }
    if let Some(spin) = opt_spin {
        spin.set_period_and_tilt(
            spin.rotation_period_hours,
            spin.axial_tilt_degrees,
            new_mass,
            new_radius,
        );
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
    prev_type: BodyType,
    updated_type: BodyType,
    new_mass: f64,
    r_au: f64,
) {
    let is_exoplanet_or_named = crate::utils::math::contains_any_ignore_ascii_case(
        &body.name,
        &[
            "proto-",
            "theia",
            "s4",
            "s2",
            "s6",
            "s5",
            "s3",
            "s1",
            "pistol",
            "g1",
            "g2",
            "gliese",
            "gj ",
            "hd ",
            "55 cancri",
            "wasp-",
            "hat-p-",
            "toi-",
            "k2-",
            "proxima",
            "tau ceti",
            "epsilon",
            "ross ",
            "lhs ",
            "hr ",
            "planet nine",
            "planet 9",
            "hot jupiter",
            "halley",
            "encke",
            "hale-bopp",
            "swift-tuttle",
            "ceres",
            "vesta",
            "psyche",
        ],
    );
    let is_protected =
        crate::simulation::accretion::collisions::is_canonical_named_body(&body.name)
            || body.body_type == BodyType::Moon
            || is_exoplanet_or_named;

    let is_generic_placeholder = body.name.starts_with("Planet-")
        || body.name.starts_with("Super-Jupiter")
        || body.name.starts_with("Brown Dwarf")
        || body.name.starts_with("Planetesimal")
        || body.name.starts_with("Embryo")
        || body.name.starts_with("Asteroid")
        || body.name.starts_with("Pop-III");

    if !is_protected && (updated_type != prev_type || is_generic_placeholder) {
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

type NebularGasBodyItem<'a> = (
    Entity,
    &'a mut Mass,
    &'a SimPosition,
    &'a mut Radius,
    &'a mut Composition,
    &'a mut CelestialBody,
    Option<&'a mut InternalDifferentiation>,
    Option<&'a mut SpinState>,
    Option<&'a mut VolatileInventory>,
    Option<&'a mut Temperature>,
    Option<&'a IgnitionState>,
    Option<&'a mut PlanetaryRingSystem>,
);

fn maybe_ignite_gas_accreted_star(
    commands: &mut Commands,
    entity: Entity,
    mass_solar: f64,
    body: &CelestialBody,
    opt_ign: Option<&IgnitionState>,
) {
    if body.body_type.is_star_or_remnant()
        && !body.body_type.is_remnant()
        && opt_ign.is_none()
        && let Ok(mut cmd) = commands.get_entity(entity)
    {
        cmd.try_insert((
            IgnitionState {
                core_temperature: 1.2e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
            Luminosity(mass_solar.powf(3.5).max(0.01)),
        ));
        bevy::log::info!(
            "🔥 {} has accreted enough gas to ignite into a star!",
            body.name
        );
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
    scenario_state: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    star_query: Query<&IgnitionState, With<CentralStar>>,
    mut bodies_query: Query<
        NebularGasBodyItem<'_>,
        (
            Without<CentralStar>,
            Without<crate::simulation::terraforming::BombardmentProjectile>,
        ),
    >,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    if scenario_state.as_ref().is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::MolecularCloudCluster
    }) {
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
    let raw_dt = if sim_time.current_dt_yr > 0.0 {
        sim_time.current_dt_yr
    } else {
        config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)
    };
    let dt_yr = raw_dt.min(10.0);
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
        mut opt_rings,
    ) in bodies_query.iter_mut()
    {
        if body.body_type == BodyType::Moon {
            continue;
        }
        let actual_r = pos.0.length();
        if actual_r > disk_params.outer_radius_au || actual_r < disk_params.inner_radius_au {
            continue;
        }
        let r_au = actual_r;
        let m = mass.0;

        let (max_gas_mass, max_gas_frac, runaway_threshold_m_earth) =
            calculate_gas_capacity_limits(r_au, is_massive_disk, &disk_params);

        if m >= max_gas_mass || (!is_massive_disk && comp.gas_frac >= max_gas_frac) {
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
                opt_rings.as_deref_mut(),
            );
            maybe_ignite_gas_accreted_star(&mut commands, entity, mass.0, &body, opt_ign);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Applies LRD blowout mass shedding, quasar transition, and relativistic jet state"
)]
fn apply_lrd_blowout_and_jets(
    commands: &mut Commands,
    qs_ent: Entity,
    state: &mut BlackHoleStarState,
    radius: &mut Radius,
    temp: &mut Temperature,
    lum: &mut Luminosity,
    body: &mut CelestialBody,
    opt_jet: Option<Mut<'_, RelativisticJetState>>,
    dt: f64,
) {
    let prev_p = f64::from(state.blowout_progress);
    state.blowout_progress = (state.blowout_progress + 0.45 * dt as f32).min(1.0);
    state.jet_travel_distance_au += crate::utils::constants::SPEED_OF_LIGHT_AU_YR * dt;
    let p = f64::from(state.blowout_progress);

    body.body_type = BodyType::BlackHole;
    body.name = format!(
        "Supermassive Quasar ({:.0} M☉)",
        state.black_hole_mass_solar
    );
    state.super_eddington_active = true;

    let r_schwa = (1.974e-8 * state.black_hole_mass_solar).max(1e-4);
    radius.0 = r_schwa;
    temp.0 = 95_000.0;
    lum.0 = 1.5e10;

    state.cocoon_radius_au = 60.0 * (1.0 + p * 4.0);
    if p >= 1.0 {
        state.cocoon_mass_solar = 0.0;
    } else if prev_p < 1.0 {
        let frac_remaining = ((1.0 - p) / (1.0 - prev_p)).clamp(0.0, 1.0);
        state.cocoon_mass_solar = (state.cocoon_mass_solar * frac_remaining).max(0.0);
    }

    if let Some(mut jet) = opt_jet {
        if jet.opening_angle_rad > 0.02 {
            *jet = RelativisticJetState::black_hole(state.black_hole_mass_solar);
            jet.opening_angle_rad = 0.016;
            jet.lorentz_factor = 20.0;
            jet.jet_length_au = 40.0;
        }
        jet.is_accreting = true;
        jet.accretion_timer_years = 1000.0;
    } else {
        let mut jet = RelativisticJetState::black_hole(state.black_hole_mass_solar);
        jet.opening_angle_rad = 0.016;
        jet.lorentz_factor = 20.0;
        jet.jet_length_au = 40.0;
        jet.is_accreting = true;
        jet.accretion_timer_years = 1000.0;
        if let Ok(mut cmd) = commands.get_entity(qs_ent) {
            cmd.try_insert(jet);
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
        Option<&mut RelativisticJetState>,
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

    for (qs_ent, mut state, mut mass, mut radius, mut temp, mut lum, mut body, qs_pos, opt_jet) in
        quasi_star_query.iter_mut()
    {
        // Reconcile any external collision/merger mass added to `mass.0` before stepping internal cocoon transfer
        let external_delta = mass.0 - state.total_mass_solar();
        if external_delta > 1e-9 {
            if state.is_blown_out {
                state.black_hole_mass_solar += external_delta;
            } else {
                state.cocoon_mass_solar += external_delta;
            }
        }

        if state.super_eddington_active && state.cocoon_mass_solar > 10.0 {
            let m_bh = state.black_hole_mass_solar;
            let m_dot_edd = 2.2e-8 * m_bh;
            let actual_rate = m_dot_edd * state.eddington_ratio;
            let dm = (actual_rate * dt * 50.0).min(state.cocoon_mass_solar);

            state.black_hole_mass_solar += dm;
            state.cocoon_mass_solar -= dm;
            state.accreted_envelope_mass += dm;
        }

        if state.is_blown_out {
            apply_lrd_blowout_and_jets(
                &mut commands,
                qs_ent,
                &mut state,
                &mut radius,
                &mut temp,
                &mut lum,
                &mut body,
                opt_jet,
                dt,
            );
        }

        // Continuously keep entity gravitational Mass synchronized through cocoon accretion & p == 1.0 blowout
        mass.0 = state.total_mass_solar();

        let bh_m = state.black_hole_mass_solar;
        let cocoon_r = state.cocoon_radius_au;

        for (sat_ent, sat_m, mut sat_vel, sat_pos, sat_rad, sat_body) in satellites_query.iter_mut()
        {
            let rel_pos = sat_pos.0 - qs_pos.0;
            let dist = rel_pos.length();

            if dist < cocoon_r && !state.is_blown_out {
                // Exponential aerodynamic drag strictly damps speed without reversing velocity at large dt
                let drag_rate = 0.08 * (1.0 - dist / cocoon_r).powf(1.5);
                sat_vel.0 *= (-drag_rate * dt).exp();
            }

            let m_ratio = (bh_m / sat_m.0.max(0.01)).cbrt();
            let r_tidal = (sat_rad.0 * m_ratio).clamp(0.05, 5.0);

            if dist < r_tidal || dist < 0.50 {
                if state.is_blown_out {
                    state.black_hole_mass_solar += sat_m.0;
                } else {
                    state.cocoon_mass_solar += sat_m.0 * 0.5;
                    state.black_hole_mass_solar += sat_m.0 * 0.5;
                }
                mass.0 = state.total_mass_solar();
                lum.0 += 5.0e7;

                info!(
                    "💥 TIDAL DISRUPTION EVENT: '{}' shredded by the {:.0} M☉ Supermassive Black Hole Seed!",
                    sat_body.name, bh_m
                );

                if let Ok(mut e_cmd) = commands.get_entity(sat_ent) {
                    e_cmd.try_despawn();
                }
            }
        }
    }
}

pub use super::gmc::gmc_cluster_gas_accretion;
