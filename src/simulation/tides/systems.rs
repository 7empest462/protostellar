//! Systems for calculating and applying gravitational tidal dissipation,
//! orbital circularization, spin-orbit synchronization, and internal viscoelastic heating.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use super::dissipation::*;
use super::types::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;
use crate::utils::math::state_vectors_to_orbital_elements;

struct HostOrbitalContext {
    host_entity: Entity,
    semi_major_axis: f64,
    eccentricity: f64,
    mean_motion: f64,
    host_mass: f64,
    rel_pos: DVec3,
    rel_vel: DVec3,
}

/// Resolves host gravitational context and Keplerian elements for planets or natural satellites.
fn resolve_host_context(
    pos: DVec3,
    vel: DVec3,
    mass: f64,
    opt_sat: Option<&SatelliteOf>,
    star_opt: Option<(Entity, DVec3, DVec3, f64)>,
) -> Option<HostOrbitalContext> {
    if let Some(sat) = opt_sat {
        let a = sat.semi_major_axis_au.max(1e-5);
        let p_yr = sat.orbital_period_years.max(1e-5);
        let m_parent = (a.powi(3) / (p_yr * p_yr)).clamp(1e-9, 10.0);
        let n = (2.0 * PI) / p_yr;
        return Some(HostOrbitalContext {
            host_entity: sat.parent,
            semi_major_axis: a,
            eccentricity: 0.01,
            mean_motion: n,
            host_mass: m_parent,
            rel_pos: DVec3::new(a, 0.0, 0.0),
            rel_vel: DVec3::new(0.0, 0.0, n * a),
        });
    }

    if let Some((s_ent, s_pos, s_vel, s_mass)) = star_opt {
        let rel_pos = pos - s_pos;
        let rel_vel = vel - s_vel;
        let elements = state_vectors_to_orbital_elements(rel_pos, rel_vel, s_mass, mass)?;
        if elements.eccentricity >= 1.0 || elements.semi_major_axis <= 1e-4 {
            return None;
        }
        let n = calculate_mean_motion(s_mass, mass, elements.semi_major_axis);
        return Some(HostOrbitalContext {
            host_entity: s_ent,
            semi_major_axis: elements.semi_major_axis,
            eccentricity: elements.eccentricity,
            mean_motion: n,
            host_mass: s_mass,
            rel_pos,
            rel_vel,
        });
    }

    None
}

/// Applies gentle radial velocity damping to simulate orbital circularization ($e \to 0$).
fn apply_orbital_circularization(
    rel_pos: DVec3,
    rel_vel: DVec3,
    vel: &mut SimVelocity,
    tau_circ_yr: f64,
    effective_dt_yr: f64,
    eccentricity: f64,
) {
    if eccentricity <= 1e-4 || tau_circ_yr <= 1e-3 {
        return;
    }

    let r = rel_pos.length();
    if r < 1e-5 {
        return;
    }

    let r_hat = rel_pos / r;
    let v_rad = rel_vel.dot(r_hat);

    // Exponential relaxation of radial motion towards circular velocity
    let damping_factor = (-effective_dt_yr / tau_circ_yr).exp();
    let new_v_rad = v_rad * damping_factor;
    let delta_v = (new_v_rad - v_rad) * r_hat;

    vel.0 += delta_v;
}

/// Couples viscoelastic tidal dissipation into internal core temperature and surface climate.
fn apply_geothermal_tidal_coupling(
    power_watts: f64,
    flux_w_m2: f64,
    body_mass_solar: f64,
    dt_yr: f64,
    opt_diff: &mut Option<Mut<'_, InternalDifferentiation>>,
    opt_climate: &mut Option<Mut<'_, PlanetaryClimate>>,
) {
    if let Some(ref mut diff) = opt_diff {
        // Specific heat capacity Cp ~ 1000 J/(kg*K)
        let m_kg = (body_mass_solar * SOLAR_MASS_KG).max(1e18);
        let energy_joules = power_watts * dt_yr * YEAR_SECONDS;
        let delta_temp_k = energy_joules / (1000.0 * m_kg);

        // Geothermal tidal boost to core temperature
        diff.core_temp_k = (diff.core_temp_k + delta_temp_k).clamp(200.0, 35000.0);

        // Tidal heating sustaining molten core and geodynamo magnetic field
        if flux_w_m2 > 0.05 && diff.core_temp_k > 1200.0 && diff.magnetic_field_gauss < 0.05 {
            diff.magnetic_field_gauss =
                0.25 * (body_mass_solar / EARTH_MASS_SOLAR).sqrt().clamp(0.1, 2.0);
        }

        // Subsurface ocean preservation: prevent complete freezing if tidal flux exceeds 20 mW/m^2
        if flux_w_m2 > 0.02 && diff.ocean_ice_thickness_au > 0.0 {
            let min_liquid_au = 1e-6;
            if diff.ocean_ice_thickness_au < min_liquid_au {
                diff.ocean_ice_thickness_au = min_liquid_au;
            }
        }
    }

    if let Some(ref mut climate) = opt_climate {
        // Effective surface temperature boost from tidal dissipation flux
        if flux_w_m2 > 0.1 {
            let t_curr = f64::from(climate.surface_temperature_k);
            let t4 = t_curr.powi(4) + (flux_w_m2 / STEFAN_BOLTZMANN_SI);
            climate.surface_temperature_k = t4.powf(0.25).clamp(20.0, 3000.0) as f32;
        }
    }
}

/// Updates gravitational tidal dynamics, circularization, spin locking, and internal heating.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Tidal evolution requires orbital, physical, rotational, and thermodynamic state"
)]
pub fn update_tidal_evolution(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    config: Res<SimulationConfig>,
    tidal_config: Res<TidalConfig>,
    star_query: Query<(Entity, &SimPosition, &SimVelocity, &Mass), With<CentralStar>>,
    mut bodies_query: Query<
        (
            Entity,
            &SimPosition,
            &mut SimVelocity,
            &Mass,
            &Radius,
            &CelestialBody,
            &Composition,
            &mut SpinState,
            Option<&SatelliteOf>,
            Option<&mut InternalDifferentiation>,
            Option<&mut PlanetaryClimate>,
            Option<&mut TidalState>,
        ),
        Without<CentralStar>,
    >,
) {
    if (time_warp.is_paused && !time_warp.step_once) || !tidal_config.is_enabled {
        return;
    }

    let dt_yr = sim_time.current_dt_yr.max(config.base_dt_yr);
    let effective_dt_yr = dt_yr * tidal_config.time_scale.max(1.0);
    let star_opt = star_query
        .iter()
        .next()
        .map(|(e, p, v, m)| (e, p.0, v.0, m.0));

    for (
        entity,
        pos,
        mut vel,
        mass,
        rad,
        body,
        comp,
        mut spin,
        opt_sat,
        mut opt_diff,
        mut opt_climate,
        mut opt_tide,
    ) in bodies_query.iter_mut()
    {
        step_body_tidal_evolution(
            &mut commands,
            entity,
            pos.0,
            &mut vel,
            mass.0,
            rad.0,
            body,
            comp,
            &mut spin,
            opt_sat,
            &mut opt_diff,
            &mut opt_climate,
            &mut opt_tide,
            star_opt,
            &tidal_config,
            dt_yr,
            effective_dt_yr,
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Tidal stepping requires body, context, and thermodynamic components"
)]
fn step_body_tidal_evolution(
    commands: &mut Commands,
    entity: Entity,
    pos: DVec3,
    vel: &mut SimVelocity,
    mass_solar: f64,
    rad_au: f64,
    body: &CelestialBody,
    comp: &Composition,
    spin: &mut SpinState,
    opt_sat: Option<&SatelliteOf>,
    opt_diff: &mut Option<Mut<'_, InternalDifferentiation>>,
    opt_climate: &mut Option<Mut<'_, PlanetaryClimate>>,
    opt_tide: &mut Option<Mut<'_, TidalState>>,
    star_opt: Option<(Entity, DVec3, DVec3, f64)>,
    tidal_config: &TidalConfig,
    dt_yr: f64,
    effective_dt_yr: f64,
) {
    if body.body_type.is_star_or_remnant() {
        return;
    }

    let Some(ctx) = resolve_host_context(pos, vel.0, mass_solar, opt_sat, star_opt) else {
        return;
    };

    let mut tide = if let Some(ref t) = opt_tide {
        **t
    } else {
        TidalState::new_from_composition_and_type(comp, body.body_type)
    };

    let a = ctx.semi_major_axis;
    let e = ctx.eccentricity;
    let n = ctx.mean_motion;
    let k2_q = tide.dissipation_factor();

    // 1. Orbital Circularization de/dt and Timescale
    let de_dt = calculate_circularization_rate(k2_q, ctx.host_mass, mass_solar, rad_au, a, e, n);
    let tau_circ =
        calculate_circularization_timescale(k2_q, ctx.host_mass, mass_solar, rad_au, a, n);
    let tau_sync = calculate_sync_timescale(k2_q, ctx.host_mass, mass_solar, rad_au, a, n);

    // 2. Spin-Orbit Synchronization
    let current_omega = (2.0 * PI) / (spin.rotation_period_hours * 3600.0 / YEAR_SECONDS).max(1.0);
    let (omega_eq, resonance_ratio) = calculate_equilibrium_spin_frequency(n, e, e >= 0.15);

    // 3. Internal Viscoelastic Tidal Heating
    let power_watts =
        calculate_tidal_heating_power(k2_q, ctx.host_mass, rad_au, a, e, current_omega, n);
    let flux_w_m2 = calculate_tidal_heat_flux(power_watts, rad_au);

    // Apply Spin Dynamics
    if tidal_config.enable_spin_synchronization {
        let (new_omega, new_tilt, progress) = apply_spin_synchronization_step(
            current_omega,
            omega_eq,
            spin.axial_tilt_degrees,
            tau_sync,
            effective_dt_yr,
        );

        let new_period_hours = ((2.0 * PI) / new_omega.max(1e-6)) * (YEAR_SECONDS / 3600.0);
        spin.rotation_period_hours = new_period_hours.clamp(0.5, 50000.0);
        spin.axial_tilt_degrees = new_tilt.clamp(0.0, 180.0);

        tide.locking_progress = progress;
        tide.is_tidally_locked = progress >= 0.99;
        tide.resonance_ratio = resonance_ratio;
    }

    // Apply Orbital Circularization Damping
    if tidal_config.enable_circularization {
        apply_orbital_circularization(ctx.rel_pos, ctx.rel_vel, vel, tau_circ, effective_dt_yr, e);
    }

    // Apply Geothermal Internal Heating
    if tidal_config.enable_heating && power_watts > 0.0 {
        apply_geothermal_tidal_coupling(
            power_watts,
            flux_w_m2,
            mass_solar,
            dt_yr,
            opt_diff,
            opt_climate,
        );
    }

    // Update component telemetry
    tide.host_entity = Some(ctx.host_entity);
    tide.tidal_heating_power_watts = power_watts;
    tide.tidal_heating_flux_w_m2 = flux_w_m2;
    tide.circularization_rate_per_myr = de_dt * 1e6;
    tide.circularization_timescale_yr = tau_circ;
    tide.sync_timescale_yr = tau_sync;

    if let Some(ref mut t) = opt_tide {
        **t = tide;
    } else {
        commands.entity(entity).try_insert(tide);
    }
}
