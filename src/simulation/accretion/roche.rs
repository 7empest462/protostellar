use crate::simulation::components::{CelestialBody, Mass, Radius, SimPosition, SimVelocity};
use crate::simulation::SupernovaEvent;
use bevy::prelude::*;

/// Marks a star that is actively overflowing its Roche Lobe and transferring mass to a companion.
#[derive(Component, Debug, Clone)]
pub struct RocheLobeOverflow {
    /// Entity ID of the accreting companion star
    pub companion: Entity,
    /// Mass transfer rate in Solar Masses per year
    pub mass_transfer_rate: f64,
    /// Distance to L1 point from the donor star's center
    pub l1_distance_au: f64,
}

/// Marks an accreting white dwarf in a binary system capable of Classical Nova eruptions.
#[derive(Component, Debug, Clone, Default)]
pub struct CataclysmicVariable {
    /// Accumulated hydrogen mass on the surface (in Solar Masses)
    pub accreted_hydrogen_mass: f64,
    /// Counter for how many nova eruptions have occurred
    pub nova_count: u32,
}

use crate::simulation::resources::{SimTime, SimulationConfig, TimeWarp};

fn compute_eggleton_roche_overflow(
    m_donor: f64,
    m_accretor: f64,
    r_donor: f64,
    separation_au: f64,
    accretor_type: crate::simulation::components::BodyType,
) -> Option<(f64, f64)> {
    let q = (m_donor / m_accretor.max(1e-12)).max(1e-12);
    let q_cbrt = q.cbrt();
    let q_23 = q_cbrt * q_cbrt;
    let r_l = (0.49 * q_23) / (0.6 * q_23 + (1.0 + q_cbrt).ln()) * separation_au;
    let eff_r_donor = if accretor_type == crate::simulation::components::BodyType::BlackHole {
        r_donor * 1.15
    } else {
        r_donor
    };
    if eff_r_donor > r_l {
        let overflow_fraction = (eff_r_donor - r_l) / r_l.max(1e-9);
        let m_dot = (overflow_fraction * 1e-4).clamp(1e-9, 1e-3);
        Some((r_l, m_dot))
    } else {
        None
    }
}

fn apply_roche_mass_transfer(
    commands: &mut Commands,
    cv_query: &mut Query<&mut CataclysmicVariable>,
    donor_ent: Entity,
    accretor_ent: Entity,
    donor_mass: &mut Mass,
    accretor_mass: &mut Mass,
    accretor_type: crate::simulation::components::BodyType,
    r_l: f64,
    m_dot: f64,
    dt_yr: f64,
) {
    let transferred = m_dot * dt_yr;
    if donor_mass.0 <= transferred {
        return;
    }
    if transferred > 0.0 {
        donor_mass.0 -= transferred;
        accretor_mass.0 += transferred;
    }

    if let Ok(mut ecmd) = commands.get_entity(donor_ent) {
        ecmd.try_insert(RocheLobeOverflow {
            companion: accretor_ent,
            mass_transfer_rate: m_dot,
            l1_distance_au: r_l,
        });
    }

    if transferred > 0.0 && accretor_type == crate::simulation::components::BodyType::WhiteDwarf {
        if let Ok(mut cv) = cv_query.get_mut(accretor_ent) {
            cv.accreted_hydrogen_mass += transferred;
        } else if let Ok(mut ecmd) = commands.get_entity(accretor_ent) {
            ecmd.try_insert(CataclysmicVariable {
                accreted_hydrogen_mass: transferred,
                nova_count: 0,
            });
        }
    }
}

pub fn process_roche_lobe_overflow(
    mut commands: Commands,
    mut star_query: Query<(
        Entity,
        &SimPosition,
        &SimVelocity,
        &mut Mass,
        &Radius,
        &CelestialBody,
        Option<&RocheLobeOverflow>,
    )>,
    mut cv_query: Query<&mut CataclysmicVariable>,
    sim_time: Option<Res<SimTime>>,
    time_warp: Option<Res<TimeWarp>>,
    config: Res<SimulationConfig>,
) {
    if !config.enable_accretion {
        return;
    }
    let is_paused = time_warp
        .as_ref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once);
    let dt_yr = if is_paused {
        0.0
    } else {
        sim_time
            .as_ref()
            .map_or(config.base_dt_yr, |st| st.current_dt_yr)
            .max(0.0)
    };

    let mut active_donors = hashbrown::HashSet::new();
    let mut previously_overflowing = Vec::new();
    for (ent, _, _, _, _, _, opt_roche) in star_query.iter() {
        if opt_roche.is_some() {
            previously_overflowing.push(ent);
        }
    }

    let mut combinations = star_query.iter_combinations_mut();
    while let Some(
        [(e1, pos1, _vel1, mut mass1, r1, body1, _), (e2, pos2, _vel2, mut mass2, r2, body2, _)],
    ) = combinations.fetch_next()
    {
        if !body1.body_type.is_star_or_remnant() || !body2.body_type.is_star_or_remnant() {
            continue;
        }

        let dist_sq = pos1.0.distance_squared(pos2.0);
        if dist_sq > 64.0 || dist_sq < 1e-8 {
            continue;
        }
        let a = dist_sq.sqrt();
        let m1 = mass1.0;
        let m2 = mass2.0;

        if let Some((r_l1, m_dot)) =
            compute_eggleton_roche_overflow(m1, m2, r1.0, a, body2.body_type)
        {
            active_donors.insert(e1);
            apply_roche_mass_transfer(
                &mut commands,
                &mut cv_query,
                e1,
                e2,
                &mut mass1,
                &mut mass2,
                body2.body_type,
                r_l1,
                m_dot,
                dt_yr,
            );
        }

        if let Some((r_l2, m_dot)) =
            compute_eggleton_roche_overflow(m2, m1, r2.0, a, body1.body_type)
        {
            active_donors.insert(e2);
            apply_roche_mass_transfer(
                &mut commands,
                &mut cv_query,
                e2,
                e1,
                &mut mass2,
                &mut mass1,
                body1.body_type,
                r_l2,
                m_dot,
                dt_yr,
            );
        }
    }

    for ent in previously_overflowing {
        if !active_donors.contains(&ent) {
            if let Ok(mut ecmd) = commands.get_entity(ent) {
                ecmd.remove::<RocheLobeOverflow>();
            }
        }
    }
}

/// Cleans up Roche Lobe overflow visual state when a star moves out of range.
pub fn cleanup_roche_lobe_overflow(
    mut commands: Commands,
    query: Query<(Entity, &SimPosition, &RocheLobeOverflow)>,
    target_query: Query<&SimPosition>,
) {
    for (entity, pos, overflow) in query.iter() {
        let should_remove = if let Ok(target_pos) = target_query.get(overflow.companion) {
            let dist_sq = pos.0.distance_squared(target_pos.0);
            dist_sq > 64.0 // 8 AU
        } else {
            true // Companion despawned
        };

        if should_remove {
            if let Ok(mut ecmd) = commands.get_entity(entity) {
                ecmd.remove::<RocheLobeOverflow>();
            }
        }
    }
}

pub fn process_classical_novae(
    _commands: Commands,
    mut cv_query: Query<(Entity, &mut CataclysmicVariable, &SimPosition, &mut Mass)>,
    mut supernova_events: bevy::prelude::MessageWriter<SupernovaEvent>,
    time_warp: Option<Res<TimeWarp>>,
) {
    if time_warp
        .as_ref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once)
    {
        return;
    }
    let critical_mass = 1.0e-4; // 10^-4 M_sun is a typical nova ignition threshold
    for (entity, mut cv, _pos, mut mass) in cv_query.iter_mut() {
        if cv.accreted_hydrogen_mass >= critical_mass {
            // Trigger Nova
            cv.accreted_hydrogen_mass = 0.0;
            cv.nova_count += 1;

            // Blow off the accreted shell
            mass.0 -= critical_mass;

            // We can reuse the SupernovaEvent struct but mark it as a Nova
            supernova_events.write(SupernovaEvent {
                star_entity: entity,
                star_name: "Classical Nova".into(),
                initial_mass_solar: critical_mass,
                remnant_mass_solar: mass.0,
                remnant_type: crate::simulation::components::BodyType::WhiteDwarf,
                shockwave_velocity_km_s: 3000.0,
                ejected_metals_solar: critical_mass * 0.15,
                ejected_composition: crate::simulation::components::Composition::supernova_ejecta(),
            });
        }
    }
}
