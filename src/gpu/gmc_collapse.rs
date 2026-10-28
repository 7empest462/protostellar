use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

use super::gmc_fluid::{GmcCollapseEventReceiver, GpuJeansCollapseEvent, GMC_CELL_VOLUME_AU3};

/// Receives Jeans collapse events on the Main App CPU side and dynamically spawns protostellar entities.
pub fn receive_gmc_collapse_events(
    mut commands: Commands,
    receiver: Res<GmcCollapseEventReceiver>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    existing_bodies: Query<(&SimPosition, &CelestialBody, Option<&Mass>)>,
    mut toast: Option<ResMut<crate::game::ui::NotificationToast>>,
) {
    let Some(state) = scenario_state.as_ref() else {
        while receiver.rx.try_recv().is_ok() {}
        return;
    };

    if state.current_preset != ScenarioPreset::MolecularCloudCluster {
        while receiver.rx.try_recv().is_ok() {}
        return;
    }

    // Guard during initial cloud relaxation to guarantee completely starless genesis
    if state.scenario_time_years < 0.20 {
        while receiver.rx.try_recv().is_ok() {}
        return;
    }

    let mut star_positions: Vec<bevy::math::DVec3> = existing_bodies
        .iter()
        .filter(|(_, body, _)| body.body_type.is_star_or_remnant())
        .map(|(pos, _, _)| pos.0)
        .collect();
    let mut star_count = star_positions.len();
    let mut spawned_this_frame = 0usize;

    // Detect live mass of central star or black hole to calibrate orbital velocities
    let central_mass = existing_bodies
        .iter()
        .filter(|(pos, body, _)| {
            pos.0.length() < 60.0
                && (body.body_type == BodyType::BlackHole || body.body_type.is_star_or_remnant())
        })
        .filter_map(|(_, _, opt_m)| opt_m.map(|m| m.0))
        .fold(3.5, f64::max);

    while let Ok(events) = receiver.rx.try_recv() {
        for ev in events {
            if star_count >= 120 || spawned_this_frame >= 2 {
                break;
            }

            let ev_pos = bevy::math::DVec3::new(
                f64::from(ev.world_pos[0]),
                f64::from(ev.world_pos[1]),
                f64::from(ev.world_pos[2]),
            );

            // Guard: Stars can spawn throughout the molecular cloud core and active spiral arms (r < 520 AU)
            let r_len = ev_pos.length();
            if r_len > 520.0 {
                continue;
            }

            // Avoid spawning if another star (including ones spawned earlier this frame) is within 32 AU
            let too_close = star_positions
                .iter()
                .any(|p| (*p - ev_pos).length_squared() < 1024.0);

            if too_close {
                continue;
            }

            star_positions.push(ev_pos);
            star_count += 1;
            spawned_this_frame += 1;
            spawn_jeans_collapse_system(
                &mut commands,
                &ev,
                ev_pos,
                r_len,
                star_count,
                central_mass,
                &mut toast,
            );
        }
    }
}

struct CollapseKinematics {
    seed_vel: bevy::math::DVec3,
    jeans_mass: f64,
}

fn compute_collapse_kinematics(
    ev: &GpuJeansCollapseEvent,
    ev_pos: bevy::math::DVec3,
    r_len: f64,
    central_mass: f64,
) -> CollapseKinematics {
    let raw_vel = bevy::math::DVec3::new(
        f64::from(ev.com_velocity[0]),
        f64::from(ev.com_velocity[1]),
        f64::from(ev.com_velocity[2]),
    );
    let safe_r = r_len.max(5.0);
    // Dynamically calculate circular orbital speed balancing central black hole / protostar,
    // cloud core (50 M☉), and Dark Matter NFW halo (8000 M☉)
    let v_circ = crate::simulation::scenarios::molecular_cloud::calculate_gmc_orbital_speed(
        safe_r,
        central_mass,
        8000.0,
        50.0,
    );
    let tangent = if ev_pos.cross(bevy::math::DVec3::Y).length_squared() > 1e-4 {
        ev_pos.cross(bevy::math::DVec3::Y).normalize()
    } else {
        bevy::math::DVec3::new(0.0, 0.0, 1.0)
    };
    let seed_vel = (tangent * v_circ * 0.98 + raw_vel.clamp_length_max(v_circ * 0.08))
        .clamp_length_max(v_circ * 1.05);
    let local_speed =
        (seed_vel.length() * crate::utils::constants::AU_PER_YR_TO_KM_PER_S).max(0.25);
    let jeans_mass =
        crate::simulation::scenarios::molecular_cloud::calculate_turbulent_jeans_mass_solar(
            f64::from(ev.local_mass_solar / GMC_CELL_VOLUME_AU3).max(4.8e-11),
            f64::from(ev.temperature_k),
            local_speed,
        );

    CollapseKinematics {
        seed_vel,
        jeans_mass,
    }
}

struct ProtostarProperties {
    seed_mass: f64,
    name: String,
    composition: Composition,
    is_gen3: bool,
    is_gen2: bool,
}

fn determine_protostar_properties(
    metallicity: f32,
    jeans_mass: f64,
    star_count: usize,
) -> ProtostarProperties {
    let is_gen3 = metallicity >= 0.045;
    let is_gen2 = metallicity >= 0.005 && !is_gen3;

    let imf_slot = star_count % 20;
    let seed_mass = if is_gen3 {
        // Gen-3: High metallicity cooling leads to solar/sub-solar stars and planetary systems
        let imf_mult = match imf_slot {
            0..=5 => 0.32,  // Red Dwarf (M-type)
            6..=10 => 0.68, // Orange Dwarf (K-type)
            11..=15 => 1.0, // Yellow Dwarf (G-type)
            16..=18 => 1.3, // Yellow-White / White Main Sequence (F/A-type)
            _ => 2.4,       // Intermediate B/A star
        };
        (jeans_mass * imf_mult).clamp(0.22, 3.2)
    } else if is_gen2 {
        // Gen-2: Intermediate metallicity spans solar analogues to OB stars
        let imf_mult = match imf_slot {
            0..=4 => 0.45,
            5..=10 => 0.95,
            11..=15 => 2.2,
            16..=18 => 5.5,
            _ => 12.0,
        };
        (jeans_mass.clamp(0.8, 2.5) * imf_mult).clamp(0.35, 22.0)
    } else {
        // Gen-1: Cluster IMF spanning low-mass dwarfs, OB stars, supernova progenitors, and rare Pop-III hypergiants
        match imf_slot {
            0..=3 => 0.35,                               // Red Dwarf (M-type)
            4..=7 => 0.72,                               // Orange Dwarf (K-type)
            8..=10 => 1.02,                              // Yellow Dwarf (G-type)
            11..=13 => 1.75,                             // White Main Sequence (A-type)
            14..=15 => 4.8,                              // Blue Giant (B-type)
            16..=17 => 18.5, // Blue Supergiant (O-type core-collapse supernova progenitor)
            18 => (jeans_mass * 40.0).clamp(45.0, 95.0), // Massive Primordial Hypergiant / LBV Progenitor
            _ => (jeans_mass * 85.0).clamp(85.0, 280.0), // Pop-III Primordial Hypergiant (Early Universe)
        }
    };

    let (name, composition) = if is_gen3 {
        let z = f64::from(metallicity).clamp(0.045, 0.15);
        let comp = Composition {
            metal_frac: z * 0.4,
            silicate_frac: z * 0.5,
            ice_frac: 0.10,
            organics_frac: 0.02,
            gas_frac: 1.0 - z - 0.12,
        };
        (format!("Protostar Gen-III SolCore-{star_count}"), comp)
    } else if is_gen2 {
        let z = f64::from(metallicity).clamp(0.005, 0.045);
        let comp = Composition {
            metal_frac: z * 0.4,
            silicate_frac: z * 0.6,
            ice_frac: 0.01,
            organics_frac: 0.0,
            gas_frac: 1.0 - z - 0.01,
        };
        (format!("Protostar Gen-II NovaCore-{star_count}"), comp)
    } else {
        let name = if seed_mass >= 45.0 {
            format!("Protostar Gen-I Pop-III Hypergiant-{star_count}")
        } else {
            format!("Protostar Gen-I Jeans-{star_count}")
        };
        (name, Composition::pure_hydrogen())
    };

    ProtostarProperties {
        seed_mass,
        name,
        composition,
        is_gen3,
        is_gen2,
    }
}

fn spawn_jeans_collapse_system(
    commands: &mut Commands,
    ev: &GpuJeansCollapseEvent,
    ev_pos: bevy::math::DVec3,
    r_len: f64,
    star_count: usize,
    central_mass: f64,
    toast: &mut Option<ResMut<crate::game::ui::NotificationToast>>,
) {
    let kinematics = compute_collapse_kinematics(ev, ev_pos, r_len, central_mass);
    let props = determine_protostar_properties(ev.metallicity, kinematics.jeans_mass, star_count);

    let star_name =
        if !props.is_gen3 && !props.is_gen2 && r_len < 45.0 && star_count.is_multiple_of(5) {
            format!("Gen-I Dark Star Protostar-{star_count}")
        } else {
            props.name.clone()
        };

    let star_entity = commands
        .spawn((
            CelestialBody {
                body_type: BodyType::Protostar,
                name: star_name.clone(),
            },
            Mass(props.seed_mass),
            SimPosition(ev_pos),
            SimVelocity(kinematics.seed_vel),
            SimAcceleration::default(),
            Radius(3.5 * crate::utils::constants::SOLAR_RADIUS_AU),
            Temperature(f64::from(ev.temperature_k).max(3800.0)),
            Luminosity((props.seed_mass * 2.5).max(0.5)),
            AngularMomentum::default(),
            props.composition,
            IgnitionState {
                core_temperature: 1.2e7,
                fusion_fraction: 1.0,
                is_ignited: true,
                shockwave_radius: 0.5,
            },
            StellarEvolutionState::default(),
            SpinState {
                rotation_period_hours: 48.0,
                axial_tilt_degrees: 15.0,
                spin_vector: bevy::math::DVec3::new(0.0, 1.0, 0.0),
            },
        ))
        .id();

    crate::simulation::scenarios::molecular_cloud::attach_gmc_circumstellar_disk_and_embryos(
        commands,
        star_entity,
        &star_name,
        ev_pos,
        kinematics.seed_vel,
        props.seed_mass,
        ev.metallicity,
        toast,
    );
}
