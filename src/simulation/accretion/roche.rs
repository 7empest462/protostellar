use crate::simulation::SupernovaEvent;
use crate::simulation::components::{
    CelestialBody, Mass, Radius, SimPosition, SimVelocity, Temperature,
};
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
#[derive(Component, Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
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

fn sync_donor_radius_after_transfer(
    donor_mass: f64,
    prev_donor_mass: f64,
    donor_rad: &mut Radius,
    donor_type: crate::simulation::components::BodyType,
) {
    use crate::simulation::components::BodyType;
    use crate::utils::constants::SOLAR_RADIUS_AU;

    if prev_donor_mass <= 0.0 || donor_mass <= 0.0 {
        return;
    }
    match donor_type {
        BodyType::BlackHole => {
            donor_rad.0 = (1.974e-8 * donor_mass).max(1e-7);
        }
        BodyType::WhiteDwarf => {
            donor_rad.0 = (0.008 * SOLAR_RADIUS_AU * (0.6 / donor_mass.max(0.05)).cbrt())
                .clamp(1.0e-6, 1.0e-3);
        }
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar | BodyType::QuarkStar => {}
        _ => {
            donor_rad.0 *= (donor_mass / prev_donor_mass).cbrt().clamp(0.5, 1.0);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Accretor sync updates mass-radius relation, stellar evolution phase, and AIC remnant state"
)]
fn sync_accretor_after_transfer(
    commands: &mut Commands,
    accretor_ent: Entity,
    accretor_mass: f64,
    accretor_rad: &mut Radius,
    accretor_body: &mut crate::simulation::components::CelestialBody,
    opt_temp: Option<&mut Temperature>,
    opt_evo: Option<&mut crate::simulation::components::StellarEvolutionState>,
    supernova_events: &mut Option<ResMut<Messages<crate::simulation::components::SupernovaEvent>>>,
) {
    use crate::simulation::components::BodyType;
    use crate::utils::constants::{CHANDRASEKHAR_LIMIT_SOLAR, SOLAR_RADIUS_AU, TOV_LIMIT_SOLAR};

    match accretor_body.body_type {
        BodyType::BlackHole => {
            accretor_rad.0 = (1.974e-8 * accretor_mass).max(1e-7);
        }
        BodyType::WhiteDwarf => {
            if accretor_mass > TOV_LIMIT_SOLAR {
                accretor_body.body_type = BodyType::BlackHole;
                accretor_rad.0 = (1.974e-8 * accretor_mass).max(1e-7);
                if let Some(temp) = opt_temp {
                    temp.0 = 10.0;
                }
                if let Some(evo) = opt_evo {
                    evo.phase =
                        crate::simulation::components::StellarEvolutionPhase::BlackHoleRemnant;
                }
                crate::simulation::thermodynamics::stellar_lifecycle::attach_black_hole_remnant_components(
                    commands,
                    accretor_ent,
                    accretor_mass,
                );
            } else if accretor_mass > CHANDRASEKHAR_LIMIT_SOLAR {
                // Accretion-induced collapse above Chandrasekhar limit into a spinning pulsar
                accretor_body.body_type = BodyType::Pulsar;
                accretor_rad.0 = 8.0e-8;
                if let Some(temp) = opt_temp {
                    temp.0 = temp.0.max(500_000.0);
                }
                if let Some(evo) = opt_evo {
                    evo.phase =
                        crate::simulation::components::StellarEvolutionPhase::NeutronStarPulsar;
                }
                if let Ok(mut cmd) = commands.get_entity(accretor_ent) {
                    cmd.try_insert((
                        crate::simulation::neutron_stars::PulsarGlitchState::default(),
                        crate::simulation::components::ElectromagneticFieldState {
                            magnetic_field_gauss: 1.0e12,
                            rotation_period_sec: 0.015,
                            magnetic_inclination_rad: 0.35,
                            jet_length_au: 3.2,
                            synchrotron_intensity: 4.5,
                        },
                        crate::simulation::components::RelativisticJetState::pulsar(0.015),
                    ));
                }
                if let Some(events) = supernova_events.as_deref_mut() {
                    events.write(crate::simulation::components::SupernovaEvent {
                        star_entity: accretor_ent,
                        star_name: accretor_body.name.clone(),
                        initial_mass_solar: accretor_mass,
                        remnant_mass_solar: accretor_mass,
                        remnant_type: BodyType::Pulsar,
                        shockwave_velocity_km_s: 12_000.0,
                        ejected_metals_solar: 0.08,
                        ejected_composition:
                            crate::simulation::components::Composition::supernova_ejecta(),
                    });
                }
            } else {
                accretor_rad.0 = (0.008 * SOLAR_RADIUS_AU * (0.6 / accretor_mass.max(0.05)).cbrt())
                    .clamp(1.0e-6, 1.0e-3);
            }
        }
        BodyType::NeutronStar | BodyType::Pulsar | BodyType::Magnetar | BodyType::QuarkStar => {
            if accretor_mass > TOV_LIMIT_SOLAR {
                accretor_body.body_type = BodyType::BlackHole;
                accretor_rad.0 = (1.974e-8 * accretor_mass).max(1e-7);
                if let Some(temp) = opt_temp {
                    temp.0 = 10.0;
                }
                if let Some(evo) = opt_evo {
                    evo.phase =
                        crate::simulation::components::StellarEvolutionPhase::BlackHoleRemnant;
                }
                crate::simulation::thermodynamics::stellar_lifecycle::attach_black_hole_remnant_components(
                    commands,
                    accretor_ent,
                    accretor_mass,
                );
            }
        }
        BodyType::MainSequenceStar
        | BodyType::RedDwarf
        | BodyType::OrangeDwarf
        | BodyType::YellowDwarf
        | BodyType::BlueMainSequence
        | BodyType::BlueStraggler => {
            accretor_rad.0 =
                (SOLAR_RADIUS_AU * accretor_mass.max(0.08).powf(0.8)).clamp(0.001, 0.15);
            if accretor_mass >= 500.0 {
                accretor_body.body_type = BodyType::BlackHole;
                accretor_rad.0 = (1.974e-8 * accretor_mass).max(1e-7);
                if let Some(evo) = opt_evo {
                    evo.phase =
                        crate::simulation::components::StellarEvolutionPhase::BlackHoleRemnant;
                }
                crate::simulation::thermodynamics::stellar_lifecycle::attach_black_hole_remnant_components(
                    commands,
                    accretor_ent,
                    accretor_mass,
                );
            }
        }
        _ => {
            if accretor_mass >= 500.0 {
                accretor_body.body_type = BodyType::BlackHole;
                accretor_rad.0 = (1.974e-8 * accretor_mass).max(1e-7);
                if let Some(evo) = opt_evo {
                    evo.phase =
                        crate::simulation::components::StellarEvolutionPhase::BlackHoleRemnant;
                }
                crate::simulation::thermodynamics::stellar_lifecycle::attach_black_hole_remnant_components(
                    commands,
                    accretor_ent,
                    accretor_mass,
                );
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Binary Roche lobe mass transfer updates donor/accretor masses, radii, and collapse state"
)]
fn apply_roche_mass_transfer(
    commands: &mut Commands,
    cv_query: &mut Query<&mut CataclysmicVariable>,
    donor_ent: Entity,
    accretor_ent: Entity,
    donor_mass: &mut Mass,
    accretor_mass: &mut Mass,
    donor_rad: &mut Radius,
    accretor_rad: &mut Radius,
    donor_type: crate::simulation::components::BodyType,
    accretor_body: &mut crate::simulation::components::CelestialBody,
    opt_accretor_temp: Option<&mut Temperature>,
    opt_accretor_evo: Option<&mut crate::simulation::components::StellarEvolutionState>,
    supernova_events: &mut Option<ResMut<Messages<crate::simulation::components::SupernovaEvent>>>,
    r_l: f64,
    m_dot: f64,
    dt_yr: f64,
) {
    let donor_floor = 0.01_f64.min(donor_mass.0 * 0.1);
    let max_transferable = (donor_mass.0 - donor_floor).max(0.0);
    let transferred = (m_dot * dt_yr).min(max_transferable);
    if transferred <= 0.0 {
        return;
    }
    let prev_donor_mass = donor_mass.0;
    let initial_accretor_type = accretor_body.body_type;
    if transferred > 0.0 {
        donor_mass.0 -= transferred;
        accretor_mass.0 += transferred;
        sync_donor_radius_after_transfer(donor_mass.0, prev_donor_mass, donor_rad, donor_type);
        sync_accretor_after_transfer(
            commands,
            accretor_ent,
            accretor_mass.0,
            accretor_rad,
            accretor_body,
            opt_accretor_temp,
            opt_accretor_evo,
            supernova_events,
        );
    }

    if let Ok(mut ecmd) = commands.get_entity(donor_ent) {
        ecmd.try_insert(RocheLobeOverflow {
            companion: accretor_ent,
            mass_transfer_rate: m_dot,
            l1_distance_au: r_l,
        });
    }

    if transferred > 0.0
        && initial_accretor_type == crate::simulation::components::BodyType::WhiteDwarf
        && accretor_body.body_type == crate::simulation::components::BodyType::WhiteDwarf
    {
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

type RocheStarQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimPosition,
        &'static SimVelocity,
        &'static mut Mass,
        &'static mut Radius,
        &'static mut CelestialBody,
        Option<&'static mut Temperature>,
        Option<&'static mut crate::simulation::components::StellarEvolutionState>,
        Option<&'static RocheLobeOverflow>,
    ),
>;

#[allow(
    clippy::too_many_arguments,
    reason = "Roche lobe overflow coordinates binary star queries, CV tracking, and AIC supernova events"
)]
pub fn process_roche_lobe_overflow(
    mut commands: Commands,
    mut star_query: RocheStarQuery,
    mut cv_query: Query<&mut CataclysmicVariable>,
    sim_time: Option<Res<SimTime>>,
    time_warp: Option<Res<TimeWarp>>,
    config: Res<SimulationConfig>,
    mut supernova_events: Option<ResMut<Messages<crate::simulation::components::SupernovaEvent>>>,
    mut active_donors: Local<rustc_hash::FxHashSet<Entity>>,
    mut star_entities_buf: Local<Vec<Entity>>,
) {
    if !config.enable_accretion {
        return;
    }
    let is_paused = time_warp
        .as_ref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once);
    if is_paused {
        return;
    }
    let dt_yr = sim_time
        .as_ref()
        .map_or(config.base_dt_yr, |st| st.current_dt_yr)
        .max(0.0);

    active_donors.clear();
    let mut previously_overflowing: smallvec::SmallVec<[Entity; 8]> = smallvec::SmallVec::new();
    for (ent, .., opt_roche) in star_query.iter() {
        if opt_roche.is_some() {
            previously_overflowing.push(ent);
        }
    }

    star_entities_buf.clear();
    star_entities_buf.extend(
        star_query
            .iter()
            .filter(|(_, _, _, _, _, body, ..)| body.body_type.is_star_or_remnant())
            .map(|(ent, ..)| ent),
    );

    for i in 0..star_entities_buf.len() {
        let Some(&e1) = star_entities_buf.get(i) else {
            continue;
        };
        for j in (i + 1)..star_entities_buf.len() {
            let Some(&e2) = star_entities_buf.get(j) else {
                continue;
            };
            let Ok(
                [
                    (_, pos1, _vel1, mut mass1, mut r1, mut body1, mut temp1, mut evo1, _),
                    (_, pos2, _vel2, mut mass2, mut r2, mut body2, mut temp2, mut evo2, _),
                ],
            ) = star_query.get_many_mut([e1, e2])
            else {
                continue;
            };

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
                let donor_type = body1.body_type;
                apply_roche_mass_transfer(
                    &mut commands,
                    &mut cv_query,
                    e1,
                    e2,
                    &mut mass1,
                    &mut mass2,
                    &mut r1,
                    &mut r2,
                    donor_type,
                    &mut body2,
                    temp2.as_deref_mut(),
                    evo2.as_deref_mut(),
                    &mut supernova_events,
                    r_l1,
                    m_dot,
                    dt_yr,
                );
            }

            if let Some((r_l2, m_dot)) =
                compute_eggleton_roche_overflow(m2, m1, r2.0, a, body1.body_type)
            {
                active_donors.insert(e2);
                let donor_type = body2.body_type;
                apply_roche_mass_transfer(
                    &mut commands,
                    &mut cv_query,
                    e2,
                    e1,
                    &mut mass2,
                    &mut mass1,
                    &mut r2,
                    &mut r1,
                    donor_type,
                    &mut body1,
                    temp1.as_deref_mut(),
                    evo1.as_deref_mut(),
                    &mut supernova_events,
                    r_l2,
                    m_dot,
                    dt_yr,
                );
            }
        }
    }

    for ent in previously_overflowing {
        if !active_donors.contains(&ent)
            && let Ok(mut ecmd) = commands.get_entity(ent)
        {
            ecmd.remove::<RocheLobeOverflow>();
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

        if should_remove && let Ok(mut ecmd) = commands.get_entity(entity) {
            ecmd.remove::<RocheLobeOverflow>();
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
            // Trigger Nova: blow off the entire accreted hydrogen envelope
            let ejected_mass = cv.accreted_hydrogen_mass;
            cv.accreted_hydrogen_mass = 0.0;
            cv.nova_count += 1;

            // Blow off the accreted shell
            mass.0 = (mass.0 - ejected_mass).max(0.05);

            // We can reuse the SupernovaEvent struct but mark it as a Nova
            supernova_events.write(SupernovaEvent {
                star_entity: entity,
                star_name: "Classical Nova".into(),
                initial_mass_solar: ejected_mass,
                remnant_mass_solar: mass.0,
                remnant_type: crate::simulation::components::BodyType::WhiteDwarf,
                shockwave_velocity_km_s: 3000.0,
                ejected_metals_solar: ejected_mass * 0.15,
                ejected_composition: crate::simulation::components::Composition::supernova_ejecta(),
            });
        }
    }
}
