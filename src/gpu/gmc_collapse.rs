use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};

use super::gmc_fluid::{GmcCollapseEventReceiver, GpuJeansCollapseEvent, GMC_CELL_VOLUME_AU3};

/// Receives Jeans collapse events on the Main App CPU side and dynamically spawns protostellar entities.
pub fn receive_gmc_collapse_events(
    mut commands: Commands,
    receiver: Res<GmcCollapseEventReceiver>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    existing_bodies: Query<(&SimPosition, &CelestialBody)>,
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
        .filter(|(_, body)| body.body_type.is_star_or_remnant())
        .map(|(pos, _)| pos.0)
        .collect();
    let mut star_count = star_positions.len();
    let mut spawned_this_frame = 0usize;

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
            spawn_jeans_collapse_system(&mut commands, &ev, ev_pos, r_len, star_count, &mut toast);
        }
    }
}

struct CollapseKinematics {
    seed_vel: bevy::math::DVec3,
    tangent: bevy::math::DVec3,
    jeans_mass: f64,
}

fn compute_collapse_kinematics(
    ev: &GpuJeansCollapseEvent,
    ev_pos: bevy::math::DVec3,
    r_len: f64,
) -> CollapseKinematics {
    let raw_vel = bevy::math::DVec3::new(
        f64::from(ev.com_velocity[0]),
        f64::from(ev.com_velocity[1]),
        f64::from(ev.com_velocity[2]),
    );
    let safe_r = r_len.max(10.0);
    let v_circ = (crate::utils::constants::G_ASTRO * 24.0 * safe_r
        / (safe_r * safe_r + 160.0 * 160.0).powf(1.5))
    .sqrt();
    let tangent = if ev_pos.cross(bevy::math::DVec3::Y).length_squared() > 1e-4 {
        ev_pos.cross(bevy::math::DVec3::Y).normalize()
    } else {
        bevy::math::DVec3::new(0.0, 0.0, 1.0)
    };
    let seed_vel = (tangent * v_circ * 0.85 + raw_vel.clamp_length_max(v_circ * 0.35))
        .clamp_length_max(v_circ * 1.15);
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
        tangent,
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
            0..=3 => 0.35,                              // Red Dwarf (M-type)
            4..=7 => 0.72,                              // Orange Dwarf (K-type)
            8..=10 => 1.02,                             // Yellow Dwarf (G-type)
            11..=13 => 1.75,                            // White Main Sequence (A-type)
            14..=16 => 4.8,                             // Blue Giant (B-type)
            17..=18 => 16.5, // Blue Supergiant (O-type core-collapse supernova progenitor)
            _ => (jeans_mass * 32.0).clamp(32.0, 48.0), // Rare Pop-III Hypergiant
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
        (
            format!("Protostar Gen-I Jeans-{star_count}"),
            Composition::pure_hydrogen(),
        )
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
    toast: &mut Option<ResMut<crate::game::ui::NotificationToast>>,
) {
    let kinematics = compute_collapse_kinematics(ev, ev_pos, r_len);
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
                name: star_name,
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

    // Stage B: Promote to planetary system by seeding orbiting Protoplanets
    let local_x = kinematics.tangent;
    let local_z = kinematics.tangent.cross(bevy::math::DVec3::Y).normalize();

    if props.is_gen3 {
        let planet_specs = get_gen3_planet_specs(&props.name);
        spawn_advanced_planetary_system(
            commands,
            star_entity,
            ev_pos,
            props.seed_mass,
            kinematics.seed_vel,
            local_x,
            local_z,
            &props.name,
            ev.metallicity,
            toast,
            planet_specs,
            format!(
                "🌟 Advanced Gen-III Genesis: {} birthed a mature Solar System! (Z = {:.3})",
                props.name, ev.metallicity
            ),
            "🌟 Spawned Advanced Gen-III system from GPU Jeans Collapse:",
        );
    } else if props.is_gen2 {
        let planet_specs = get_gen2_planet_specs(&props.name);
        spawn_advanced_planetary_system(
            commands,
            star_entity,
            ev_pos,
            props.seed_mass,
            kinematics.seed_vel,
            local_x,
            local_z,
            &props.name,
            ev.metallicity,
            toast,
            planet_specs,
            format!(
                "🌟 Enriched Genesis: {} birthed 3 rocky terrestrial worlds! (Z = {:.3})",
                props.name, ev.metallicity
            ),
            "🌟 Spawned Enriched Gen-II system from GPU Jeans Collapse:",
        );
    } else {
        spawn_gen1_planetary_system(
            commands,
            star_entity,
            ev_pos,
            props.seed_mass,
            kinematics.seed_vel,
            local_x,
            local_z,
            &props.name,
            toast,
        );
    }
}

type PlanetSpec = (
    String,
    f64,
    f64,
    f64,
    Composition,
    VolatileInventory,
    PlanetaryClimate,
    InternalDifferentiation,
    BodyType,
    Option<BiosphereState>,
);

fn get_gen3_terra_spec(protostar_name: &str) -> PlanetSpec {
    (
        format!("{protostar_name} b (Terra)"),
        1.0,
        0.000_003, // ~1 Earth mass
        0.000_042, // ~1 Earth radius
        Composition {
            metal_frac: 0.32,
            silicate_frac: 0.65,
            ice_frac: 0.02,
            organics_frac: 0.01,
            gas_frac: 0.00,
        },
        VolatileInventory {
            delivered_water_m_earth: 1.0,
            ocean_coverage_frac: 0.71,
            atmospheric_pressure_bar: 1.0,
            cometary_impact_count: 50,
        },
        PlanetaryClimate {
            surface_temperature_k: 288.0,
            equilibrium_temperature_k: 255.0,
            greenhouse_delta_k: 33.0,
            albedo: 0.30,
            ice_coverage_frac: 0.10,
            cloud_coverage_frac: 0.50,
            climate_regime: ClimateRegime::TemperateHabitable,
            polar_ice_cap_latitude_deg: 72.0,
        },
        InternalDifferentiation {
            is_differentiated: true,
            differentiation_fraction: 1.0,
            core_radius_au: 0.000_042 * 0.55,
            mantle_radius_au: 0.000_042 * 0.95,
            crust_thickness_au: 0.000_042 * 0.05,
            ocean_ice_thickness_au: 0.000_042 * 0.008,
            magnetic_field_gauss: 0.65,
            core_temp_k: 5500.0,
            has_theia_llsvp: false,
            llsvp_density_contrast: 0.0,
        },
        BodyType::TerrestrialPlanet,
        None,
    )
}

fn get_gen3_jovian_spec(protostar_name: &str) -> PlanetSpec {
    (
        format!("{protostar_name} c (Jovian)"),
        5.2,
        0.001,     // ~1 Jupiter mass
        0.000_477, // ~1 Jupiter radius
        Composition {
            metal_frac: 0.05,
            silicate_frac: 0.05,
            ice_frac: 0.10,
            organics_frac: 0.0,
            gas_frac: 0.80,
        },
        VolatileInventory::default(),
        PlanetaryClimate {
            surface_temperature_k: 120.0,
            equilibrium_temperature_k: 110.0,
            greenhouse_delta_k: 10.0,
            albedo: 0.50,
            ice_coverage_frac: 0.0,
            cloud_coverage_frac: 1.0,
            climate_regime: ClimateRegime::GasGiantEnvelope,
            polar_ice_cap_latitude_deg: 90.0,
        },
        InternalDifferentiation {
            is_differentiated: true,
            differentiation_fraction: 1.0,
            core_radius_au: 0.000_477 * 0.15,
            mantle_radius_au: 0.000_477 * 0.85,
            crust_thickness_au: 0.0,
            ocean_ice_thickness_au: 0.0,
            magnetic_field_gauss: 4.2,
            core_temp_k: 24000.0,
            has_theia_llsvp: false,
            llsvp_density_contrast: 0.0,
        },
        BodyType::GasGiant,
        None,
    )
}

fn get_gen3_planet_specs(protostar_name: &str) -> [PlanetSpec; 2] {
    [
        get_gen3_terra_spec(protostar_name),
        get_gen3_jovian_spec(protostar_name),
    ]
}

#[allow(
    clippy::too_many_arguments,
    reason = "Spawning orbiting planets needs physical parameters"
)]
fn spawn_system_planets(
    commands: &mut Commands,
    _star_entity: Entity,
    ev_pos: bevy::math::DVec3,
    seed_mass: f64,
    seed_vel: bevy::math::DVec3,
    local_x: bevy::math::DVec3,
    local_z: bevy::math::DVec3,
    specs: impl IntoIterator<Item = PlanetSpec>,
) {
    for (idx, (p_name, a_au, m_p, r_au, comp, vol, climate, diff, b_type, bio)) in
        specs.into_iter().enumerate()
    {
        let v_circ_p = (crate::utils::constants::G_ASTRO * seed_mass / a_au).sqrt();
        let angle = (idx as f64 + 1.0) * 2.1;

        let p_pos = ev_pos + local_x * (a_au * angle.cos()) + local_z * (a_au * angle.sin());
        let p_vel =
            seed_vel + local_x * (-v_circ_p * angle.sin()) + local_z * (v_circ_p * angle.cos());

        let mut ent = commands.spawn((
            CelestialBody {
                body_type: b_type,
                name: p_name,
            },
            Mass(m_p),
            SimPosition(p_pos),
            SimVelocity(p_vel),
            SimAcceleration::default(),
            Radius(r_au),
            Temperature(f64::from(climate.surface_temperature_k)),
            Luminosity(0.0),
            AngularMomentum::default(),
            comp,
            diff,
            vol,
            climate,
            SpinState::default(),
        ));
        if let Some(b) = bio {
            ent.insert(b);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Spawning full planetary system needs many context parameters"
)]
fn spawn_advanced_planetary_system(
    commands: &mut Commands,
    star_entity: Entity,
    ev_pos: bevy::math::DVec3,
    seed_mass: f64,
    seed_vel: bevy::math::DVec3,
    local_x: bevy::math::DVec3,
    local_z: bevy::math::DVec3,
    protostar_name: &str,
    metallicity: f32,
    toast: &mut Option<ResMut<crate::game::ui::NotificationToast>>,
    planet_specs: impl IntoIterator<Item = PlanetSpec>,
    toast_msg: String,
    log_msg_prefix: &str,
) {
    spawn_system_planets(
        commands,
        star_entity,
        ev_pos,
        seed_mass,
        seed_vel,
        local_x,
        local_z,
        planet_specs,
    );

    if let Some(ref mut t) = toast {
        t.message = toast_msg;
        t.timer = 5.0;
    }

    bevy::log::info!(
        "{} {} at {:?}, mass {:.2} M☉, metallicity Z={:.3}",
        log_msg_prefix,
        protostar_name,
        ev_pos,
        seed_mass,
        metallicity
    );
}

fn get_gen2_planet_specs(protostar_name: &str) -> [PlanetSpec; 3] {
    [
        // Inner: Dense metal-rich Mercury analogue
        (
            format!("{protostar_name} b (Iron-Rich World)"),
            1.4,
            0.000_003, // ~1 Earth mass
            0.000_042, // ~1 Earth radius
            Composition {
                metal_frac: 0.60,
                silicate_frac: 0.35,
                ice_frac: 0.01,
                organics_frac: 0.01,
                gas_frac: 0.03, // 3% captured primordial H2/He envelope
            },
            VolatileInventory {
                delivered_water_m_earth: 0.0005,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 0.85,
                cometary_impact_count: 12,
            },
            PlanetaryClimate {
                surface_temperature_k: 420.0,
                equilibrium_temperature_k: 410.0,
                greenhouse_delta_k: 10.0,
                albedo: 0.15,
                ice_coverage_frac: 0.0,
                cloud_coverage_frac: 0.10,
                climate_regime: ClimateRegime::AirlessVacuum,
                polar_ice_cap_latitude_deg: 90.0,
            },
            InternalDifferentiation {
                is_differentiated: true,
                differentiation_fraction: 1.0,
                core_radius_au: 0.000_042 * 0.65,
                mantle_radius_au: 0.000_042 * 0.95,
                crust_thickness_au: 0.000_042 * 0.05,
                ocean_ice_thickness_au: 0.0,
                magnetic_field_gauss: 0.40,
                core_temp_k: 4200.0,
                has_theia_llsvp: false,
                llsvp_density_contrast: 0.0,
            },
            BodyType::Protoplanet,
            None,
        ),
        // Habitable zone: Rocky Earth analogue
        (
            format!("{protostar_name} c (Rocky Earth-Analogue)"),
            2.8,
            0.000_006, // ~2 Earth masses
            0.000_055, // ~1.3 Earth radii
            Composition {
                metal_frac: 0.30,
                silicate_frac: 0.58,
                ice_frac: 0.05, // 5% volatile hydrosphere / water
                organics_frac: 0.03,
                gas_frac: 0.04, // 4% primordial nebular atmosphere
            },
            VolatileInventory {
                delivered_water_m_earth: 0.008,
                ocean_coverage_frac: 0.70,
                atmospheric_pressure_bar: 1.25,
                cometary_impact_count: 45,
            },
            PlanetaryClimate {
                surface_temperature_k: 288.0,
                equilibrium_temperature_k: 255.0,
                greenhouse_delta_k: 33.0,
                albedo: 0.30,
                ice_coverage_frac: 0.10,
                cloud_coverage_frac: 0.50,
                climate_regime: ClimateRegime::TemperateHabitable,
                polar_ice_cap_latitude_deg: 72.0,
            },
            InternalDifferentiation {
                is_differentiated: true,
                differentiation_fraction: 1.0,
                core_radius_au: 0.000_055 * 0.55,
                mantle_radius_au: 0.000_055 * 0.95,
                crust_thickness_au: 0.000_055 * 0.05,
                ocean_ice_thickness_au: 0.000_055 * 0.008,
                magnetic_field_gauss: 0.65,
                core_temp_k: 5500.0,
                has_theia_llsvp: false,
                llsvp_density_contrast: 0.0,
            },
            BodyType::Protoplanet,
            None,
        ),
        // Outer: Volatile / Ocean world
        (
            format!("{protostar_name} d (Volatile Ocean World)"),
            5.8,
            0.000_015, // ~5 Earth masses
            0.000_085, // ~2.0 Earth radii
            Composition {
                metal_frac: 0.12,
                silicate_frac: 0.36,
                ice_frac: 0.35, // 35% volatile water / deep ocean
                organics_frac: 0.12,
                gas_frac: 0.05, // 5% primordial gas envelope
            },
            VolatileInventory {
                delivered_water_m_earth: 0.25,
                ocean_coverage_frac: 1.0,
                atmospheric_pressure_bar: 35.0,
                cometary_impact_count: 95,
            },
            PlanetaryClimate {
                surface_temperature_k: 265.0,
                equilibrium_temperature_k: 215.0,
                greenhouse_delta_k: 50.0,
                albedo: 0.45,
                ice_coverage_frac: 0.25,
                cloud_coverage_frac: 0.70,
                climate_regime: ClimateRegime::TemperateHabitable,
                polar_ice_cap_latitude_deg: 45.0,
            },
            InternalDifferentiation {
                is_differentiated: true,
                differentiation_fraction: 1.0,
                core_radius_au: 0.000_085 * 0.40,
                mantle_radius_au: 0.000_085 * 0.80,
                crust_thickness_au: 0.000_085 * 0.05,
                ocean_ice_thickness_au: 0.000_085 * 0.15,
                magnetic_field_gauss: 0.45,
                core_temp_k: 4500.0,
                has_theia_llsvp: false,
                llsvp_density_contrast: 0.0,
            },
            BodyType::Protoplanet,
            None,
        ),
    ]
}

#[allow(
    clippy::too_many_arguments,
    reason = "Spawning full planetary system needs many context parameters"
)]
fn spawn_gen1_planetary_system(
    commands: &mut Commands,
    _star_entity: Entity,
    ev_pos: bevy::math::DVec3,
    seed_mass: f64,
    seed_vel: bevy::math::DVec3,
    local_x: bevy::math::DVec3,
    local_z: bevy::math::DVec3,
    protostar_name: &str,
    toast: &mut Option<ResMut<crate::game::ui::NotificationToast>>,
) {
    for i in 1..=3 {
        let a_au = f64::from(i) * 8.5 + 4.0; // orbits at 12.5, 21.0, 29.5 AU
        let m_p = 0.003; // ~3 Jupiter masses
        let v_circ_p = (crate::utils::constants::G_ASTRO * seed_mass / a_au).sqrt();
        let angle = f64::from(i) * 2.4; // Phase offset

        let p_pos = ev_pos + local_x * (a_au * angle.cos()) + local_z * (a_au * angle.sin());
        let p_vel =
            seed_vel + local_x * (-v_circ_p * angle.sin()) + local_z * (v_circ_p * angle.cos());

        let r_au = 1.5 * 0.000_477;
        let surf_temp = (1200.0 / (a_au).sqrt()) as f32;
        commands.spawn((
            CelestialBody {
                body_type: BodyType::GasGiant,
                name: format!("{protostar_name} b{i}"),
            },
            Mass(m_p),
            SimPosition(p_pos),
            SimVelocity(p_vel),
            SimAcceleration::default(),
            Radius(r_au),
            Temperature(f64::from(surf_temp)),
            Luminosity(0.0),
            AngularMomentum::default(),
            Composition::solar_gas(),
            SpinState::default(),
            VolatileInventory {
                delivered_water_m_earth: 15.0,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: 500.0,
                cometary_impact_count: 50,
            },
            PlanetaryClimate {
                surface_temperature_k: surf_temp,
                equilibrium_temperature_k: surf_temp * 0.85,
                greenhouse_delta_k: 40.0,
                albedo: 0.35,
                ice_coverage_frac: 0.0,
                cloud_coverage_frac: 0.95,
                climate_regime: ClimateRegime::GasGiantEnvelope,
                polar_ice_cap_latitude_deg: 90.0,
            },
        ));
    }

    if let Some(ref mut t) = toast {
        t.message =
            format!("✨ Jeans Instability Collapse: {protostar_name} formed ({seed_mass:.2} M☉)");
        t.timer = 5.0;
    }

    bevy::log::info!(
        "✨ Spawned new Protostar from GPU Jeans Collapse: {} at {:?}, mass {:.2} M☉",
        protostar_name,
        ev_pos,
        seed_mass
    );
}
