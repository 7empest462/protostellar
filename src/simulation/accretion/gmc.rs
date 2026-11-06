//! Gas accretion in 3D Giant Molecular Clouds (GMC): Bondi-Hoyle stellar accretion,
//! protoplanetary envelope feeding, and supermassive black hole hyper-accretion.

use bevy::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

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
        let is_pop_iii =
            body.body_type == BodyType::PopulationIIIStar || body.name.contains("Pop-III");
        let max_rate = if is_pop_iii {
            (0.12 * m).clamp(0.05, 5.0)
        } else {
            (0.045 * m).clamp(0.025, 0.85)
        };
        let bondi_rate = (base_bondi * (rho_cloud / rho_0)).clamp(0.002, max_rate);

        // Radiation pressure and stellar wind suppression for ignited stars:
        // Main-sequence stellar winds clear the local Bondi sphere so low/intermediate-mass stars
        // maintain their birth spectral class rather than runaway-accreting to 120 M_sun.
        let is_ignited = opt_ign.is_some_and(|ig| ig.is_ignited);
        let rad_suppression = if body.body_type.is_remnant() {
            0.0
        } else if is_ignited {
            let lum_val = opt_lum.as_deref().map_or(1.0, |l| l.0);
            if is_pop_iii {
                // Pristine zero-metallicity Pop-III stars lack line-driven radiative winds,
                // allowing massive hypergiant growth up to 500 M_sun before direct collapse
                if m >= 500.0 {
                    0.0
                } else {
                    0.05 * (1.0 - m / 500.0).clamp(0.0, 1.0)
                }
            } else if lum_val > 1_500_000.0 || m >= 120.0 {
                0.0
            } else if m < 15.0 {
                0.006
            } else {
                0.025 * (1.0 - lum_val / 1_500_000.0).clamp(0.0, 1.0)
            }
        } else {
            1.0
        };

        let d_mass = bondi_rate * rad_suppression * dt_yr;
        if d_mass > 1e-12 {
            let new_m = m + d_mass;
            mass.0 = new_m;

            // Physical radius update (only for non-remnant, non-giant main sequence / protostars)
            if body.body_type == BodyType::Protostar {
                rad.0 = (3.5 * SOLAR_RADIUS_AU * (new_m / 1.0).powf(0.5)).clamp(0.005, 0.04);
            } else if !body.body_type.is_remnant()
                && !matches!(body.body_type, BodyType::RedGiant | BodyType::RedSupergiant)
                && new_m >= 0.08
            {
                rad.0 = (SOLAR_RADIUS_AU * new_m.powf(0.8)).clamp(0.002, 0.12);
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
    _entity: Entity,
    pos: &SimPosition,
    mass: &mut Mass,
    rad: &mut Radius,
    comp: &mut Composition,
    body: &mut CelestialBody,
    opt_vol: &mut Option<Mut<VolatileInventory>>,
    dt_yr: f64,
    star_pos: bevy::math::DVec3,
    star_m: f64,
) {
    let m = mass.0;
    // B. PROTOPLANET ACCRETION FROM CIRCUMSTELLAR ENVELOPE & GMC NEBULA
    let a_au = (pos.0 - star_pos).length().clamp(0.4, 50.0);
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

#[allow(
    clippy::too_many_arguments,
    reason = "Black hole hyper-accretion in dense GMC core requires orbital state and physical components"
)]
fn compute_gmc_black_hole_accretion(
    m: f64,
    pos: &SimPosition,
    vel: &SimVelocity,
    mass: &mut Mass,
    rad: &mut Radius,
    body: &mut CelestialBody,
    mut opt_jet: Option<&mut crate::simulation::components::RelativisticJetState>,
    dt_yr: f64,
    gas_scale: f64,
) {
    let r_bh = pos.0.length();
    let rho_bh = if r_bh < 350.0 {
        3.4e-11 * (1.0 / (1.0 + (r_bh / 140.0).powi(2)).powf(1.5)) * gas_scale
    } else {
        1.5e-13 * gas_scale
    };
    let c_s = 0.058;
    let v_bh = vel.0.length();
    let v_eff = (c_s * c_s + v_bh * v_bh).sqrt().max(0.04);
    let max_bondi = (0.5 * m.sqrt()).clamp(1.0, 50.0);
    let bondi_bh = (19500.0 * m * m / (v_eff * v_eff * v_eff) * rho_bh).clamp(1.0e-5, max_bondi);
    let edd_rate = (0.015 * m).min(25.0);
    let d_mass = (bondi_bh + edd_rate) * dt_yr;
    if d_mass > 1e-12 {
        let new_m = m + d_mass;
        mass.0 = new_m;
        rad.0 = (1.974e-8 * new_m).max(1e-7);
        if let Some(ref mut jet) = opt_jet {
            jet.is_accreting = true;
            jet.accretion_timer_years = 5.0;
        }
        if new_m >= 100_000.0 && !body.name.contains("Supermassive") {
            body.name = format!(
                "Galactic Nucleus (Supermassive Black Hole - {:.1}k M☉)",
                new_m / 1000.0
            );
        } else if new_m >= 100.0
            && !body.name.contains("Intermediate")
            && !body.name.contains("Supermassive")
        {
            body.name = format!("Intermediate-Mass Black Hole ({new_m:.1} M☉)");
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
    mut bodies_query: Query<
        (
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
            Option<&CircumstellarDisk>,
        ),
        Without<crate::simulation::probes::SpaceProbe>,
    >,
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

    // 1. Collect all star positions, masses, and active disk status for planetary envelope accretion
    let stars: Vec<(Entity, bevy::math::DVec3, f64, bool)> = bodies_query
        .iter()
        .filter(|(_, _, _, _, _, _, body, _, _, _, _, _, _)| body.body_type.is_star_or_remnant())
        .map(|(e, m, pos, _, _, _, _, _, _, _, _, _, opt_disk)| {
            let is_active = opt_disk.is_none_or(|d| d.is_active_full_disk);
            (e, pos.0, m.0, is_active)
        })
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
        mut opt_jet,
        opt_disk,
    ) in bodies_query.iter_mut()
    {
        let m = mass.0;
        let is_host_active = opt_disk.as_ref().is_none_or(|d| d.is_active_full_disk);

        // A. PROTOSTAR & STELLAR BONDI-HOYLE ACCRETION FROM CLOUD CORE
        if body.body_type == BodyType::Protostar
            || (body.body_type.is_star_or_remnant() && !body.body_type.is_remnant())
        {
            // Only run full accretion on is_active_full_disk hosts if accretion is expensive
            if is_host_active {
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
            }
        } else if body.body_type == BodyType::BlackHole {
            compute_gmc_black_hole_accretion(
                m,
                pos,
                vel,
                &mut mass,
                &mut rad,
                &mut body,
                opt_jet.as_deref_mut(),
                dt_yr,
                gas_scale,
            );
        } else if body.body_type == BodyType::Protoplanet || body.body_type.is_planet() {
            let closest_star = stars
                .iter()
                .filter(|(s_ent, _, _, _)| *s_ent != entity)
                .min_by(|(_, p1, _, _), (_, p2, _, _)| {
                    let d1 = (*p1 - pos.0).length_squared();
                    let d2 = (*p2 - pos.0).length_squared();
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                });

            // Only run full envelope accretion if host star's disk is active
            if let Some((_, star_pos, star_m, is_active)) = closest_star {
                if *is_active {
                    compute_protoplanet_envelope_accretion(
                        entity,
                        pos,
                        &mut mass,
                        &mut rad,
                        &mut comp,
                        &mut body,
                        &mut opt_vol,
                        dt_yr,
                        *star_pos,
                        *star_m,
                    );
                }
            }
        }
    }
}
