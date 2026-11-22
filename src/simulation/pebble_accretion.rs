//! Pebble accretion onto planetary cores, asteroids, and comets (Ormel/Klahr–style continuum model).
//!
//! cm–m aerodynamic solids drift inward through sub-Keplerian gas. Cores sweep them
//! up much faster than planetesimal collisions alone. When pebble concentration
//! exceeds the streaming instability threshold (especially at the water ice snow line),
//! pebble filaments collapse directly into asteroids and comets.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::disk::planetesimals::PlanetesimalSpawner;
use crate::simulation::resources::*;
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::*;
use crate::utils::math::FastRngExt;

/// Metallicity-like solid-to-gas ratio for the ambient pebble reservoir.
const PEBBLE_Z: f64 = 0.015;

/// Standard Hayashi MMSN gas surface density at 1 AU in $M_\odot / \text{AU}^2$ (~1,700 g/cm^2).
const MMSN_SIGMA_1AU_MSUN_AU2: f64 = 1.91e-4;

/// Game-tuned multiplier converting physical MMSN pebble flux timescales into interactive play timescales.
const PEBBLE_GAME_SCALE: f64 = 0.35 / MMSN_SIGMA_1AU_MSUN_AU2;

/// Disk aspect ratio h = H/r.
pub fn aspect_ratio(r_au: f64) -> f64 {
    (0.05 * (r_au / 1.0).powf(0.25)).clamp(0.03, 0.10)
}

/// Local Hayashi MMSN gas surface density $\Sigma_{\text{gas}}(r)$ in $M_\odot / \text{AU}^2$.
pub fn local_gas_sigma(r_au: f64, gas_density_scale: f64) -> f64 {
    if gas_density_scale <= 1e-4 || r_au < 0.05 {
        return 0.0;
    }
    MMSN_SIGMA_1AU_MSUN_AU2 * (r_au / 1.0).powf(-1.50).max(0.0) * gas_density_scale
}

/// Radial drift velocity of mm-to-cm pebbles due to sub-Keplerian gas headwind [AU / yr].
pub fn pebble_drift_velocity_au_yr(r_au: f64, star_mass: f64) -> f64 {
    let h = aspect_ratio(r_au);
    let eta = 0.5 * h * h * 3.25;
    let v_k = (G_ASTRO * star_mass / r_au.max(0.01)).sqrt();
    let stokes = 0.05;
    let drift_factor = (2.0 * stokes) / (1.0 + stokes * stokes);
    -eta * v_k * drift_factor
}

/// Silicate fragmentation barrier inside the water ice snow line.
/// Dry silicate dust pebbles fragment at ~1 m/s collision speed, whereas sticky icy
/// pebbles beyond the snow line withstand ~10 m/s before fragmenting.
/// Consequently, the inner disk pebble reservoir is fragmented to tiny sub-millimeter grains
/// with small Stokes numbers (St << 0.01), suppressing pebble surface density Σ_peb by ~85%.
pub fn silicate_fragmentation_barrier(r_au: f64, snow_line_au: f64) -> f64 {
    if r_au >= snow_line_au {
        1.0
    } else {
        let dr = (snow_line_au - r_au).max(0.0);
        let suppression = (-dr / 0.45).exp() * 0.85 + 0.15;
        suppression.clamp(0.12, 1.0)
    }
}

/// Pebble surface density Σ_peb ∝ Z Σ_gas with cold-finger vapor condensation boost at the snow line
/// and dry silicate fragmentation suppression inside the snow line.
pub fn local_pebble_sigma(r_au: f64, gas_density_scale: f64, snow_line_au: f64) -> f64 {
    let base_sigma = PEBBLE_Z * local_gas_sigma(r_au, gas_density_scale);
    if base_sigma <= 1e-16 {
        return 0.0;
    }
    // Snow line cold finger effect: outward-diffusing vapor recondenses onto icy pebbles,
    // creating a ~2.5x traffic-jam pile-up peaking at the ice line (e.g. 2.7 AU).
    let dr = r_au - snow_line_au;
    let trap_factor = 1.0 + 2.5 * (-dr * dr / (2.0 * 0.35 * 0.35)).exp();
    let frag_factor = silicate_fragmentation_barrier(r_au, snow_line_au);
    base_sigma * trap_factor * frag_factor
}

/// Pebble isolation mass parameterized by dynamic `snow_line_au`.
/// Outside the snow line: $M_{\text{iso}} / M_\oplus \approx 20\,(h/0.05)^3$ (Lambrechts & Johansen 2014).
/// In the Main Asteroid Belt `(0.78..=1.28) * snow_line_au`: resonant perturbations cap growth to Ceres scale.
pub fn pebble_isolation_mass_solar_at(r_au: f64, snow_line_au: f64) -> f64 {
    let h = aspect_ratio(r_au);
    let snow = snow_line_au.max(0.5);
    let belt_inner = 0.78 * snow;
    let belt_outer = 1.28 * snow;
    if (belt_inner..=belt_outer).contains(&r_au) {
        // Asteroid belt: giant-planet resonance perturbation barrier dynamically anchored to snow line
        0.002 * EARTH_MASS_SOLAR
    } else if r_au < belt_inner {
        // Inner terrestrial zone: dry silicate pebble stall
        0.85 * EARTH_MASS_SOLAR
    } else {
        // Outer giant core formation zone (Lambrechts & Johansen 2014)
        let m_earth = 20.0 * (h / 0.05).powi(3);
        m_earth * EARTH_MASS_SOLAR
    }
}

/// Pebble isolation mass for canonical Solar Nebula snow line (`2.7 AU`).
pub fn pebble_isolation_mass_solar(r_au: f64) -> f64 {
    pebble_isolation_mass_solar_at(r_au, 2.7)
}

/// Returns true if a body can sweep up drifting pebbles.
pub fn is_pebble_target(body_type: BodyType) -> bool {
    matches!(
        body_type,
        BodyType::Asteroid
            | BodyType::Comet
            | BodyType::Planetesimal
            | BodyType::Protoplanet
            | BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::IceGiant
            | BodyType::GasGiant
    )
}

/// Stokes-ish accretion efficiency: minor bodies (asteroids/comets) in 3D Bondi sweep,
/// larger cores approach 2D Hill sweep.
pub fn accretion_efficiency(mass_solar: f64, r_au: f64, star_mass: f64) -> f64 {
    let r_hill = r_au * (mass_solar / (3.0 * star_mass).max(1e-12)).cbrt();
    let h = aspect_ratio(r_au) * r_au;
    let ratio = (r_hill / h.max(1e-8)).clamp(0.0, 10.0);
    let eta = (ratio * ratio) / (1.0 + ratio * ratio);
    eta.clamp(0.005, 1.0)
}

/// Instantaneous pebble accretion rate [M_sun / yr].
pub fn pebble_accretion_rate(
    mass_solar: f64,
    r_au: f64,
    star_mass: f64,
    gas_density_scale: f64,
    snow_line_au: f64,
    body_type: BodyType,
) -> f64 {
    if !is_pebble_target(body_type) || mass_solar < 1e-14 {
        return 0.0;
    }

    let sigma_peb = local_pebble_sigma(r_au, gas_density_scale, snow_line_au);
    if sigma_peb <= 1e-16 {
        return 0.0;
    }

    let r_hill = r_au * (mass_solar / (3.0 * star_mass).max(1e-12)).cbrt();
    let omega = (G_ASTRO * star_mass / (r_au * r_au * r_au).max(1e-12)).sqrt();
    let v_hill = omega * r_hill;

    let eps = accretion_efficiency(mass_solar, r_au, star_mass);
    let mut mdot = 2.0 * r_hill * sigma_peb * v_hill * eps;

    // Isolation: strongly suppress once past M_iso
    let m_iso = pebble_isolation_mass_solar_at(r_au, snow_line_au);
    if mass_solar > m_iso {
        let over = ((mass_solar / m_iso) - 1.0).clamp(0.0, 10.0);
        mdot *= (-over * 4.0).exp() * 0.01;
    }

    // Outside snow line, higher solid ice flux
    if r_au >= snow_line_au {
        mdot *= 1.35;
    }

    mdot * PEBBLE_GAME_SCALE
}

fn update_promoted_name(name: &mut String, old_type: BodyType, new_type: BodyType) {
    if old_type == BodyType::Asteroid && new_type == BodyType::Planetesimal {
        *name = name.replace("Asteroid", "Planetesimal");
    } else if old_type == BodyType::Comet && new_type == BodyType::Planetesimal {
        *name = name.replace("Comet", "Icy Planetesimal");
    } else if (old_type == BodyType::Planetesimal || old_type == BodyType::Asteroid)
        && new_type == BodyType::Protoplanet
    {
        *name = name
            .replace("Planetesimal", "Embryo")
            .replace("Asteroid", "Embryo");
    } else if new_type == BodyType::TerrestrialPlanet {
        const CANONICAL_NAMES: [&str; 14] = [
            "earth", "theia", "mars", "venus", "mercury", "moon", "ceres", "vesta", "psyche",
            "pluto", "jupiter", "saturn", "uranus", "neptune",
        ];
        let is_named = crate::utils::math::contains_any_ignore_ascii_case(name, &CANONICAL_NAMES)
            || name.starts_with("Planet");
        if !is_named {
            *name = format!("Planet ({name})");
        }
    }
}

fn compute_hydrated_pebble_water_fraction(r_au: f64, snow_line_au: f64) -> f64 {
    if r_au <= 0.35 {
        return 0.0;
    }
    if r_au >= snow_line_au {
        0.35
    } else {
        let dr = (snow_line_au - r_au).max(0.0);
        // Carbonaceous chondrite phyllosilicate & wet icy pebble drift fraction inside snowline (~3.5% at 1 AU)
        ((-dr / 1.4).exp() * 0.08 + 0.012).clamp(0.008, 0.09)
    }
}

/// Computes Earth-masses of H2O delivered during a pebble accretion step `total_dm_solar`
/// without double-counting fine dust bypass or structural endogenous hydroxyls.
fn compute_pebble_water_delivery_m_earth(
    total_dm_solar: f64,
    r_au: f64,
    gas_scale: f64,
    snow_line_au: f64,
    comp: &Composition,
) -> f64 {
    if r_au <= 0.35 || gas_scale <= 0.001 || total_dm_solar <= 0.0 {
        return 0.0;
    }
    let dm_earth = total_dm_solar / EARTH_MASS_SOLAR;
    let pebble_h2o_frac = compute_hydrated_pebble_water_fraction(r_au, snow_line_au);
    let endogenous = compute_endogenous_water_m_earth(total_dm_solar, comp, r_au, gas_scale as f32);
    // Single-count volatile contribution: hydrated carbonaceous/icy pebbles dominate endogenous -OH
    (dm_earth * pebble_h2o_frac).max(endogenous).max(0.0)
}

/// Bevy system: grow cores, planetesimals, asteroids, and comets by sweeping pebbles.
#[allow(
    clippy::type_complexity,
    reason = "Bevy Query mirrors direct_nebular_gas_accretion"
)]
pub fn apply_pebble_accretion(
    mut commands: Commands,
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    disk_params: Res<DiskParameters>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    mut bodies_query: Query<
        (
            Entity,
            &mut Mass,
            &SimPosition,
            &mut Radius,
            &mut Composition,
            &mut CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&mut VolatileInventory>,
            Option<&mut PlanetaryRingSystem>,
        ),
        (
            Without<CentralStar>,
            Without<crate::simulation::terraforming::BombardmentProjectile>,
            Without<crate::simulation::probes::SpaceProbe>,
        ),
    >,
) {
    if (!config.enable_accretion || time_warp.is_paused) && !time_warp.step_once {
        return;
    }

    if disk_params.disk_mass <= 0.0
        || scenario_state.as_ref().is_some_and(|s| {
            matches!(
                s.current_preset,
                ScenarioPreset::MolecularCloudCluster
                    | ScenarioPreset::GalacticCollision
                    | ScenarioPreset::TidalDisruptionEvent
            )
        })
    {
        return;
    }

    let gas_scale = f64::from(config.gas_density_scale);
    if gas_scale <= 0.001 || sim_time.elapsed_years > disk_params.gas_disk_lifetime_yr {
        return;
    }

    let raw_dt = if sim_time.current_dt_yr > 0.0 {
        sim_time.current_dt_yr
    } else {
        config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)
    };
    let dt_yr = raw_dt.min(50.0);
    let star_mass = disk_params.central_star_mass;
    let snow = disk_params.snow_line_au;

    for (
        entity,
        mut mass,
        pos,
        mut rad,
        mut comp,
        mut body,
        mut opt_diff,
        mut opt_vol,
        mut opt_rings,
    ) in bodies_query.iter_mut()
    {
        if !is_pebble_target(body.body_type) {
            continue;
        }

        let r_au = pos.0.length();
        if r_au < disk_params.inner_radius_au || r_au > disk_params.outer_radius_au {
            continue;
        }

        let mdot = pebble_accretion_rate(mass.0, r_au, star_mass, gas_scale, snow, body.body_type);
        let dm = mdot * dt_yr;
        if mdot <= 0.0 || dm < 1e-16 {
            continue;
        }

        apply_single_body_pebble_accretion(
            &mut commands,
            entity,
            &mut mass,
            r_au,
            &mut rad,
            &mut comp,
            &mut body,
            &mut opt_diff,
            &mut opt_vol,
            &mut opt_rings,
            dm,
            star_mass,
            gas_scale,
            snow,
            dt_yr,
        );
    }
}

fn apply_pebble_body_promotion(
    commands: &mut Commands,
    entity: Entity,
    new_mass: f64,
    comp: &Composition,
    body: &mut CelestialBody,
    rad: &mut Radius,
) {
    let new_type = if body.body_type == BodyType::Moon {
        BodyType::Moon
    } else {
        classify_body_by_mass_and_comp(new_mass, comp, false)
    };
    if new_type != body.body_type {
        let old_type = body.body_type;
        body.body_type = new_type;
        update_promoted_name(&mut body.name, old_type, new_type);
        if new_type == BodyType::BrownDwarf && body.name.starts_with("Super-Jupiter") {
            let mj = new_mass / JUPITER_MASS_SOLAR;
            body.name = format!("Brown Dwarf ({mj:.1} M_J)");
        } else if new_type.is_star_or_remnant() && !new_type.is_remnant() && new_mass >= 0.08 {
            let (_, label) = classify_stellar_type_and_label(new_mass, comp);
            crate::simulation::thermodynamics::stellar_lifecycle::update_promoted_stellar_name(
                &mut body.name,
                new_type,
                new_mass,
                label,
            );
            rad.0 = (SOLAR_RADIUS_AU * new_mass.powf(0.8)).clamp(0.001, 0.12);
            if let Ok(mut cmd) = commands.get_entity(entity) {
                cmd.try_insert((
                    IgnitionState {
                        core_temperature: 1.2e7,
                        fusion_fraction: 1.0,
                        is_ignited: true,
                        shockwave_radius: 0.25,
                    },
                    StellarEvolutionState {
                        phase: StellarEvolutionPhase::MainSequence,
                        hydrogen_core_fraction: 1.0,
                        ..Default::default()
                    },
                    Luminosity(new_mass.powf(3.5).max(0.01)),
                ));
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Per-body pebble accretion helper"
)]
fn apply_single_body_pebble_accretion(
    commands: &mut Commands,
    entity: Entity,
    mass: &mut Mass,
    r_au: f64,
    rad: &mut Radius,
    comp: &mut Composition,
    body: &mut CelestialBody,
    opt_diff: &mut Option<Mut<'_, InternalDifferentiation>>,
    opt_vol: &mut Option<Mut<'_, VolatileInventory>>,
    opt_rings: &mut Option<Mut<'_, PlanetaryRingSystem>>,
    dm: f64,
    star_mass: f64,
    gas_scale: f64,
    snow: f64,
    dt_yr: f64,
) {
    let prev_mass = mass.0;
    let prev_radius = rad.0;

    let m_iso = pebble_isolation_mass_solar_at(r_au, snow);
    let fine_dm_solar = if r_au > 0.35
        && r_au < snow
        && gas_scale > 0.001
        && prev_mass >= m_iso
        && matches!(
            body.body_type,
            BodyType::Protoplanet | BodyType::TerrestrialPlanet | BodyType::SuperEarth
        ) {
        let unthrottled_fine_rate = pebble_accretion_rate(
            m_iso.min(prev_mass),
            r_au,
            star_mass,
            gas_scale,
            snow,
            body.body_type,
        );
        let over = ((prev_mass / m_iso) - 1.0).clamp(0.0, 10.0);
        let fine_bypass_frac = 0.04 * (-over * 0.5).exp();
        (unthrottled_fine_rate * fine_bypass_frac * dt_yr).max(0.0)
    } else {
        0.0
    };

    let total_dm = dm + fine_dm_solar;
    let new_mass = prev_mass + total_dm;
    mass.0 = new_mass;

    let water_gain_m_earth =
        compute_pebble_water_delivery_m_earth(total_dm, r_au, gas_scale, snow, comp);

    if r_au >= snow {
        let ice_add = 0.55;
        let rock_add = 0.35;
        let metal_add = 0.10;
        let total = ice_add + rock_add + metal_add;
        comp.ice_frac = (comp.ice_frac * prev_mass + ice_add / total * total_dm) / new_mass;
        comp.silicate_frac =
            (comp.silicate_frac * prev_mass + rock_add / total * total_dm) / new_mass;
        comp.metal_frac = (comp.metal_frac * prev_mass + metal_add / total * total_dm) / new_mass;
        comp.organics_frac = (comp.organics_frac * prev_mass) / new_mass;
        comp.gas_frac = (comp.gas_frac * prev_mass) / new_mass;
    } else {
        let wet_ice_add = if r_au > 0.35 {
            compute_hydrated_pebble_water_fraction(r_au, snow).min(0.03)
        } else {
            0.0
        };
        let rock_add = 0.70 - wet_ice_add * 0.70;
        let metal_add = 0.30 - wet_ice_add * 0.30;
        comp.ice_frac = (comp.ice_frac * prev_mass + wet_ice_add * total_dm) / new_mass;
        comp.silicate_frac = (comp.silicate_frac * prev_mass + rock_add * total_dm) / new_mass;
        comp.metal_frac = (comp.metal_frac * prev_mass + metal_add * total_dm) / new_mass;
        comp.organics_frac = (comp.organics_frac * prev_mass) / new_mass;
        comp.gas_frac = (comp.gas_frac * prev_mass) / new_mass;
    }

    if water_gain_m_earth > 0.0 {
        if let Some(vol) = opt_vol {
            vol.delivered_water_m_earth += water_gain_m_earth;
            if vol.atmospheric_pressure_bar < 1.0 {
                vol.atmospheric_pressure_bar = (vol.atmospheric_pressure_bar
                    + (water_gain_m_earth / 0.0005) as f32 * 0.45)
                    .min(1.0);
            }
        } else if matches!(
            body.body_type,
            BodyType::Protoplanet | BodyType::TerrestrialPlanet | BodyType::SuperEarth
        ) && let Ok(mut cmd) = commands.get_entity(entity)
        {
            cmd.try_insert(VolatileInventory {
                delivered_water_m_earth: water_gain_m_earth,
                ocean_coverage_frac: 0.0,
                atmospheric_pressure_bar: ((water_gain_m_earth / 0.0005) as f32 * 0.45).min(1.0),
                cometary_impact_count: 0,
            });
        }
    }

    *comp = comp.normalized();

    apply_pebble_body_promotion(commands, entity, new_mass, comp, body, rad);
    if !(body.body_type.is_star_or_remnant() && !body.body_type.is_remnant() && new_mass >= 0.08) {
        let density = comp.average_density();
        let physical_radius = ((3.0 * new_mass / density.max(1e-30)) / (4.0 * PI)).cbrt();
        let new_radius = match body.body_type {
            BodyType::GasGiant | BodyType::BrownDwarf => {
                (0.000_477 * (new_mass / 0.000_954).powf(0.08)).clamp(0.00035, 0.00085)
            }
            BodyType::Protoplanet
            | BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::IceGiant => physical_radius.max(EARTH_RADIUS_AU * 0.05),
            _ => physical_radius.max(1e-8),
        };
        rad.0 = new_radius;
    }
    if let Some(rings) = opt_rings {
        rings.scale_with_host_radius(prev_radius, rad.0);
    }

    if let Some(diff) = opt_diff {
        diff.recalculate(new_mass, rad.0, comp);
    }
}

fn spawn_single_filament_minor_body(
    commands: &mut Commands,
    disk_params: &DiskParameters,
    spawner: &mut PlanetesimalSpawner,
    rng: &mut fastrand::Rng,
) {
    let snow = disk_params.snow_line_au.max(0.5);
    let niche_roll: f64 = rng.f64_range(0.0, 1.0);
    let (r, comp, body_type, log_mass) = if niche_roll < 0.60 {
        let r_ast = rng.f64_range(0.78 * snow, 1.28 * snow);
        let comp_ast = if rng.chance(0.75) {
            Composition::carbonaceous()
        } else {
            Composition::rocky()
        };
        (
            r_ast,
            comp_ast,
            BodyType::Asteroid,
            rng.f64_range(-5.5, -3.8),
        )
    } else if niche_roll < 0.95 {
        let r_comet = rng.f64_range(16.0, 45.0);
        let comp_comet = Composition::icy();
        (
            r_comet,
            comp_comet,
            BodyType::Comet,
            rng.f64_range(-6.0, -4.2),
        )
    } else {
        let r_trap = rng.f64_range(snow - 0.2, snow + 0.3);
        let comp_trap = Composition {
            silicate_frac: 0.45,
            ice_frac: 0.45,
            metal_frac: 0.10,
            gas_frac: 0.0,
            organics_frac: 0.0,
        };
        (
            r_trap,
            comp_trap,
            BodyType::Planetesimal,
            rng.f64_range(-3.8, -2.8),
        )
    };

    let mass = EARTH_MASS_SOLAR * 10.0_f64.powf(log_mass);
    let density = comp.average_density();
    let volume = mass / density;
    let phys_radius = ((3.0 * volume) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.04);

    let (pos, vel) = crate::simulation::disk::planetesimals::sample_inclined_keplerian_state(
        r,
        disk_params.central_star_mass,
        0.04,
        (0.985, 1.015),
        rng,
    );

    let temp = disk_params.reference_temp_1au * (r / 1.0).powf(-0.5);

    spawner.name_counter += 1;
    let name = match body_type {
        BodyType::Asteroid => format!("Asteroid #{}", spawner.name_counter),
        BodyType::Comet => format!("Comet #{}", spawner.name_counter),
        _ => format!("Planetesimal #{}", spawner.name_counter),
    };

    let mut diff = InternalDifferentiation::default();
    diff.recalculate(mass, phys_radius, &comp);

    let mut spin = SpinState::default();
    let spin_period_hrs: f64 = rng.f64_range(6.0, 36.0);
    let omega = 2.0 * PI / (spin_period_hrs * 3600.0 / YEAR_SECONDS);
    let initial_spin = 0.33 * mass * phys_radius * phys_radius * DVec3::new(0.0, omega, 0.0);
    spin.update_from_spin(initial_spin, mass, phys_radius);

    commands.spawn((
        CelestialBody { body_type, name },
        Mass(mass),
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration::default(),
        Radius(phys_radius),
        Temperature(temp),
        Luminosity(0.0),
        AngularMomentum(pos.cross(vel) * mass),
        comp,
        diff,
        spin,
    ));

    spawner.total_spawned += 1;
}

/// Spawns Asteroids, Comets, and Planetesimals from streaming instability pebble filaments.
pub fn spawn_streaming_instability_minor_bodies(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    config: Res<SimulationConfig>,
    disk_params: Res<DiskParameters>,
    mut spawner: ResMut<PlanetesimalSpawner>,
    scenario_state: Option<Res<ActiveScenarioState>>,
    body_count: Query<Entity, With<CelestialBody>>,
) {
    if (time_warp.is_paused && !time_warp.step_once)
        || (config.enable_gpu_compute
            && config.gpu_compute_active
            && config.active_particles > 1_000)
    {
        return;
    }

    if disk_params.disk_mass <= 0.0
        || scenario_state.as_ref().is_some_and(|s| {
            matches!(
                s.current_preset,
                ScenarioPreset::MolecularCloudCluster
                    | ScenarioPreset::GalacticCollision
                    | ScenarioPreset::TidalDisruptionEvent
            )
        })
    {
        return;
    }

    let t = sim_time.elapsed_years;
    if t > disk_params.gas_disk_lifetime_yr || (config.gas_density_scale <= 0.001 && t > 2500.0) {
        return;
    }

    let current_bodies = body_count.iter().count() as u32;
    if current_bodies >= spawner.max_ecs_bodies {
        return;
    }
    let deficit = spawner.max_ecs_bodies.saturating_sub(current_bodies);

    let (spawn_interval, min_burst, max_burst, max_bursts) = if deficit > 500 {
        (35.0, 8, 16, 12usize)
    } else if deficit > 100 {
        (75.0, 5, 10, 8usize)
    } else if deficit > 50 {
        (150.0, 4, 8, 4usize)
    } else {
        (300.0, 3, 6, 1usize)
    };

    let dt = t - spawner.last_minor_body_spawn_yr;
    if dt < spawn_interval {
        return;
    }

    let num_bursts = ((dt / spawn_interval).floor() as usize).clamp(1, max_bursts);
    let mut rng = fastrand::Rng::new();
    let mut spawned_total = 0u32;

    for _ in 0..num_bursts {
        let burst_count = rng.usize(min_burst..=max_burst);
        for _ in 0..burst_count {
            if current_bodies + spawned_total >= spawner.max_ecs_bodies {
                break;
            }
            spawn_single_filament_minor_body(&mut commands, &disk_params, &mut spawner, &mut rng);
            spawned_total += 1;
        }
        if current_bodies + spawned_total >= spawner.max_ecs_bodies {
            break;
        }
    }

    if spawned_total > 0 {
        spawner.last_minor_body_spawn_yr =
            (spawner.last_minor_body_spawn_yr + (num_bursts as f64) * spawn_interval).min(t);
    }
}
