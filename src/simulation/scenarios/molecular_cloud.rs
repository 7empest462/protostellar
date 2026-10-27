//! Giant Molecular Cloud (GMC) Core & Jeans Instability Star Cluster Genesis.
//!
//! Simulates the gravitational fragmentation and collapse of a dense, cold
//! pre-stellar molecular cloud core (T ~ 10-20 K, M_cloud ~ 24 M☉, R ~ 550 AU).
//!
//! Based on astrophysical Jeans instability:
//!   M_J = (c_s^3) / (G^(3/2) * rho^(1/2))
//! where supersonic turbulent velocity fluctuations (Mach ~ 3-6) compress gas filaments
//! beyond the critical threshold, triggering the birth of an open star cluster:
//! - Primary Class 0 hyper-accreting protostar (3.5 M☉)
//! - Infalling close binary companion (1.8 M☉)
//! - Intermediate-mass Herbig Ae/Be and Solar-type T-Tauri protostars
//! - Low-mass M-dwarf pre-stellar seeds and substellar brown dwarf embryos
//! - Collapsing dense Jeans fragmentation gas clumps exhibiting N-body dynamical scattering.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::{DiskParameters, SimTime, SimulationConfig, TimeWarp};
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::*;

/// Total gas and stellar mass of the dense molecular cloud core in Solar Masses.
pub const GMC_CORE_MASS_SOLAR: f64 = 24.0;

/// Outer boundary radius of the collapsing molecular cloud core in AU.
pub const GMC_CORE_RADIUS_AU: f64 = 550.0;

/// Ambient cold molecular gas temperature in Kelvin (T ~ 10-15 K).
pub const GMC_CLOUD_TEMPERATURE_K: f64 = 15.0;

/// Mean radius of Jupiter in Astronomical Units (~71,492 km).
pub const JUPITER_RADIUS_AU: f64 = 0.000_477;

/// Calculates the isothermal sound speed c_s in AU/yr for molecular gas (mu = 2.3).
pub fn calculate_gmc_sound_speed_au_yr(temp_k: f64) -> f64 {
    // c_s = sqrt(gamma * k_B * T / (mu * m_H))
    // For T = 15 K, mu = 2.3, c_s ~ 0.274 km/s
    let c_s_km_s = (1.4 * 1.380_649e-23 * temp_k / (2.3 * 1.673_557_5e-27)).sqrt() / 1000.0;
    c_s_km_s / AU_PER_YR_TO_KM_PER_S
}

/// Calculates the thermal Jeans mass in Solar Masses for a given gas density and temperature.
pub fn calculate_jeans_mass_solar(density_solar_au3: f64, temp_k: f64) -> f64 {
    calculate_turbulent_jeans_mass_solar(density_solar_au3, temp_k, 0.0)
}

/// Calculates the Jeans mass in Solar Masses including turbulent support (sigma_v in km/s).
pub fn calculate_turbulent_jeans_mass_solar(
    density_solar_au3: f64,
    temp_k: f64,
    turbulent_dispersion_km_s: f64,
) -> f64 {
    let c_s = calculate_gmc_sound_speed_au_yr(temp_k);
    let sigma_v_au_yr = turbulent_dispersion_km_s / AU_PER_YR_TO_KM_PER_S;
    let c_eff = (c_s * c_s + (sigma_v_au_yr * sigma_v_au_yr) / 3.0).sqrt();

    let prefactor = std::f64::consts::PI.powf(2.5) / 6.0;
    let g_astro = G_ASTRO;
    prefactor * c_eff.powi(3) / (g_astro.powf(1.5) * density_solar_au3.max(1e-15).sqrt())
}

/// Configuration for a protostellar seed or collapsing Jeans core clump.
struct ClusterSeedConfig {
    name: &'static str,
    body_type: BodyType,
    mass_solar: f64,
    radius_au: f64,
    temp_k: f64,
    lum_solar: f64,
    semi_major_axis_au: f64,
    eccentricity: f64,
    inclination_deg: f64,
    turbulent_kick_km_s: DVec3,
    composition: Composition,
    is_protostar: bool,
}

fn compute_seed_3d_state(cfg: &ClusterSeedConfig, m_central: f64) -> (DVec3, DVec3) {
    let inc = cfg.inclination_deg.to_radians();
    let a = cfg.semi_major_axis_au;
    let e = cfg.eccentricity;
    let r_peri = a * (1.0 - e);

    let mu = G_ASTRO * (m_central + cfg.mass_solar);
    let v_peri = (mu * (2.0 / r_peri - 1.0 / a)).sqrt();

    // Pericenter position in orbital plane, inclined along X-Z
    let pos = DVec3::new(r_peri * inc.cos(), r_peri * inc.sin(), 0.0);

    // Tangential Keplerian orbital velocity
    let mut vel = DVec3::new(0.0, 0.0, v_peri);

    // Add 3D turbulent velocity perturbation from supersonic GMC cloud turbulence
    let v_turb_au_yr = cfg.turbulent_kick_km_s / AU_PER_YR_TO_KM_PER_S;
    vel += v_turb_au_yr;

    (pos, vel)
}

fn spawn_cluster_member(
    commands: &mut Commands,
    cfg: &ClusterSeedConfig,
    m_central: f64,
) -> Entity {
    let (pos, vel) = compute_seed_3d_state(cfg, m_central);

    let mut cmd = commands.spawn((
        CelestialBody {
            body_type: cfg.body_type,
            name: cfg.name.to_string(),
        },
        Mass(cfg.mass_solar),
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration::default(),
        Radius(cfg.radius_au),
        Temperature(cfg.temp_k),
        Luminosity(cfg.lum_solar),
        AngularMomentum(pos.cross(vel) * cfg.mass_solar),
        cfg.composition,
        SpinState {
            rotation_period_hours: 36.0,
            axial_tilt_degrees: cfg.inclination_deg.abs() * 0.75,
            spin_vector: DVec3::new(0.0, 1.0, 0.0),
        },
    ));

    if cfg.is_protostar {
        cmd.insert((
            IgnitionState {
                core_temperature: 3.5e6,
                fusion_fraction: 0.25,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ));
    }

    cmd.id()
}

fn spawn_primary_massive_protostar(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            CentralStar,
            CelestialBody {
                body_type: BodyType::Protostar,
                name: "Protostar Alpha (Class 0 Hyper-Accreting Seed)".to_string(),
            },
            Mass(3.5),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(12.0 * SOLAR_RADIUS_AU), // Bloated pre-collapse protostellar envelope
            Temperature(3200.0),
            Luminosity(45.0),
            AngularMomentum::default(),
            Composition::solar_gas(),
            IgnitionState {
                core_temperature: 7.2e6,
                fusion_fraction: 0.72,
                is_ignited: false,
                shockwave_radius: 0.0,
            },
            StellarEvolutionState::default(),
        ))
        .id()
}

fn spawn_inner_cluster_protostars(commands: &mut Commands, primary_mass: f64) {
    let seeds = [
        // Close Infalling Proto-Binary Companion
        ClusterSeedConfig {
            name: "Protostar Beta (Class I Infalling Companion)",
            body_type: BodyType::Protostar,
            mass_solar: 1.80,
            radius_au: 5.5 * SOLAR_RADIUS_AU,
            temp_k: 3600.0,
            lum_solar: 14.0,
            semi_major_axis_au: 42.0,
            eccentricity: 0.38,
            inclination_deg: 5.0,
            turbulent_kick_km_s: DVec3::new(0.4, 0.2, -0.3),
            composition: Composition::solar_gas(),
            is_protostar: true,
        },
        // Intermediate Mass Herbig Ae/Be Precursor
        ClusterSeedConfig {
            name: "Protostar Gamma (Herbig Ae/Be Seed)",
            body_type: BodyType::Protostar,
            mass_solar: 2.20,
            radius_au: 4.2 * SOLAR_RADIUS_AU,
            temp_k: 6800.0,
            lum_solar: 28.0,
            semi_major_axis_au: 110.0,
            eccentricity: 0.24,
            inclination_deg: 18.0,
            turbulent_kick_km_s: DVec3::new(-0.8, 0.4, 0.6),
            composition: Composition::solar_gas(),
            is_protostar: true,
        },
        // Solar-Analog Precursor (Class I T-Tauri)
        ClusterSeedConfig {
            name: "Protostar Delta (Class I Solar-Analog)",
            body_type: BodyType::Protostar,
            mass_solar: 1.05,
            radius_au: 2.8 * SOLAR_RADIUS_AU,
            temp_k: 4200.0,
            lum_solar: 3.5,
            semi_major_axis_au: 175.0,
            eccentricity: 0.16,
            inclination_deg: -8.0,
            turbulent_kick_km_s: DVec3::new(0.5, -0.3, -0.4),
            composition: Composition::solar_gas(),
            is_protostar: true,
        },
        // Classical T-Tauri Star with Active Accretion
        ClusterSeedConfig {
            name: "Protostar Epsilon (Class II T-Tauri)",
            body_type: BodyType::Protostar,
            mass_solar: 0.70,
            radius_au: 2.1 * SOLAR_RADIUS_AU,
            temp_k: 3800.0,
            lum_solar: 1.2,
            semi_major_axis_au: 245.0,
            eccentricity: 0.18,
            inclination_deg: 12.0,
            turbulent_kick_km_s: DVec3::new(-0.4, 0.5, -0.2),
            composition: Composition::solar_gas(),
            is_protostar: true,
        },
        // Red Dwarf M-Type Precursor
        ClusterSeedConfig {
            name: "Protostar Zeta (M-Dwarf Seed)",
            body_type: BodyType::Protostar,
            mass_solar: 0.35,
            radius_au: 1.4 * SOLAR_RADIUS_AU,
            temp_k: 3100.0,
            lum_solar: 0.22,
            semi_major_axis_au: 320.0,
            eccentricity: 0.22,
            inclination_deg: -25.0,
            turbulent_kick_km_s: DVec3::new(0.6, -0.6, 0.5),
            composition: Composition::solar_gas(),
            is_protostar: true,
        },
    ];

    for cfg in &seeds {
        spawn_cluster_member(commands, cfg, primary_mass);
    }
}

fn spawn_outer_cluster_members_and_clumps(commands: &mut Commands, primary_mass: f64) {
    let seeds = [
        // Very Low-Mass Late M-Dwarf
        ClusterSeedConfig {
            name: "Protostar Eta (Late M-Dwarf Seed)",
            body_type: BodyType::Protostar,
            mass_solar: 0.16,
            radius_au: 0.9 * SOLAR_RADIUS_AU,
            temp_k: 2800.0,
            lum_solar: 0.05,
            semi_major_axis_au: 395.0,
            eccentricity: 0.20,
            inclination_deg: 22.0,
            turbulent_kick_km_s: DVec3::new(-0.7, 0.3, 0.4),
            composition: Composition::solar_gas(),
            is_protostar: true,
        },
        // Brown Dwarf Substellar Embryo
        ClusterSeedConfig {
            name: "Substellar Embryo Theta (Brown Dwarf Seed)",
            body_type: BodyType::BrownDwarf,
            mass_solar: 0.060,
            radius_au: 0.6 * SOLAR_RADIUS_AU,
            temp_k: 1900.0,
            lum_solar: 0.003,
            semi_major_axis_au: 470.0,
            eccentricity: 0.28,
            inclination_deg: -32.0,
            turbulent_kick_km_s: DVec3::new(0.9, -0.5, -0.8),
            composition: Composition::solar_gas(),
            is_protostar: false,
        },
        // Free-Floating Planetary Core Embryo
        ClusterSeedConfig {
            name: "Substellar Core Iota (Planetary Embryo)",
            body_type: BodyType::GasGiant,
            mass_solar: 0.035,
            radius_au: 0.4 * SOLAR_RADIUS_AU,
            temp_k: 1400.0,
            lum_solar: 0.0008,
            semi_major_axis_au: 535.0,
            eccentricity: 0.25,
            inclination_deg: 28.0,
            turbulent_kick_km_s: DVec3::new(-0.8, 0.6, -0.4),
            composition: Composition::solar_gas(),
            is_protostar: false,
        },
        // Dense Jeans Pre-Stellar Fragmentation Clump 1
        ClusterSeedConfig {
            name: "Jeans Clump 1 (Pre-Stellar Condensation)",
            body_type: BodyType::Protoplanet,
            mass_solar: 0.28,
            radius_au: 3.5, // Extended pre-stellar gas clump envelope
            temp_k: 18.0,
            lum_solar: 0.001,
            semi_major_axis_au: 85.0,
            eccentricity: 0.35,
            inclination_deg: -14.0,
            turbulent_kick_km_s: DVec3::new(0.5, 0.3, 0.2),
            composition: Composition::solar_gas(),
            is_protostar: false,
        },
        // Dense Jeans Pre-Stellar Fragmentation Clump 2
        ClusterSeedConfig {
            name: "Jeans Clump 2 (Filament Collapse Knot)",
            body_type: BodyType::Protoplanet,
            mass_solar: 0.18,
            radius_au: 2.8,
            temp_k: 15.0,
            lum_solar: 0.0005,
            semi_major_axis_au: 205.0,
            eccentricity: 0.26,
            inclination_deg: 16.0,
            turbulent_kick_km_s: DVec3::new(-0.4, -0.4, 0.3),
            composition: Composition::solar_gas(),
            is_protostar: false,
        },
    ];

    for cfg in &seeds {
        spawn_cluster_member(commands, cfg, primary_mass);
    }
}

/// Spawns the Giant Molecular Cloud scenario preset with zero initial stars.
///
/// Initializes the pristine, cold molecular gas cloud core (24 M☉, 550 AU).
/// Protostars form organically and dynamically over time via 3D fluid hydrodynamics
/// and Jeans gravitational instability collapse.
pub fn spawn_molecular_cloud_cluster_scenario(
    _commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Option<Entity> {
    // Pure starless molecular cloud core
    disk_params.central_star_mass = 0.0;
    disk_params.inner_radius_au = 0.50;
    disk_params.outer_radius_au = GMC_CORE_RADIUS_AU;
    // 2D disk mass is zeroed so Keplerian planetesimal particles remain inactive
    disk_params.disk_mass = 0.0;
    disk_params.gas_disk_lifetime_yr = 250_000.0;

    None
}

/// Spawns a pre-seeded star cluster test fixture for multi-body orbital kinematics testing.
pub fn spawn_preseeded_cluster_fixture(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    disk_params.central_star_mass = 3.5;
    disk_params.inner_radius_au = 0.50;
    disk_params.outer_radius_au = GMC_CORE_RADIUS_AU;
    disk_params.disk_mass = 0.008;
    disk_params.gas_disk_lifetime_yr = 250_000.0;

    let primary_protostar = spawn_primary_massive_protostar(commands);
    spawn_inner_cluster_protostars(commands, 3.5);
    spawn_outer_cluster_members_and_clumps(commands, 3.5);

    primary_protostar
}

/// Maximum number of concurrent active full 3D circumstellar disks in the GMC cluster to maintain 60+ FPS.
pub const MAX_GMC_ACTIVE_FULL_DISKS: usize = 4;

/// Resource managing active full circumstellar disks and enforcing the concurrency cap for cluster performance.
#[derive(Resource, Debug, Clone)]
pub struct GmcClusterDiskManager {
    pub max_active_full_disks: usize,
}

impl Default for GmcClusterDiskManager {
    fn default() -> Self {
        Self {
            max_active_full_disks: MAX_GMC_ACTIVE_FULL_DISKS,
        }
    }
}

/// Caps the number of active full 3D circumstellar disks in the cluster to `max_disks` (3..=6).
/// Dissipates the oldest active disk(s) into cleared planetary systems with surviving embryos.
pub fn cap_gmc_active_full_disks(
    commands: &mut Commands,
    max_disks: usize,
    existing_disks: &mut Query<(Entity, &mut CircumstellarDisk, Option<&PlanetaryRingSystem>)>,
) {
    let mut active_disks: Vec<(Entity, f64)> = existing_disks
        .iter()
        .filter(|(_, d, _)| d.is_active_full_disk)
        .map(|(e, d, _)| (e, d.age_years))
        .collect();

    if active_disks.len() >= max_disks {
        active_disks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let to_remove = (active_disks.len() + 1).saturating_sub(max_disks);
        for &(oldest_e, _) in active_disks.iter().take(to_remove) {
            if let Ok((_, mut old_disk, opt_ring)) = existing_disks.get_mut(oldest_e) {
                old_disk.is_active_full_disk = false;
                if opt_ring.is_some() {
                    commands.entity(oldest_e).remove::<PlanetaryRingSystem>();
                }
            }
        }
    }
}

pub struct GmcEmbryoSpec {
    pub name: String,
    pub semi_major_axis_au: f64,
    pub mass_solar: f64,
    pub radius_au: f64,
    pub composition: Composition,
    pub body_type: BodyType,
    pub angle_rad: f64,
}

pub fn get_gmc_promotion_embryos(
    star_name: &str,
    star_mass: f64,
    metallicity: f32,
) -> Vec<GmcEmbryoSpec> {
    let m_ratio = (star_mass / 1.0).max(0.1);
    if metallicity >= 0.045 {
        // Gen-III: Mature SolCore Solar System analogues (Terra & Jovian)
        vec![
            GmcEmbryoSpec {
                name: format!("{star_name} b (Terra Embryo)"),
                semi_major_axis_au: 1.0 * m_ratio.sqrt(),
                mass_solar: EARTH_MASS_SOLAR,
                radius_au: EARTH_RADIUS_AU,
                composition: Composition {
                    metal_frac: 0.32,
                    silicate_frac: 0.65,
                    ice_frac: 0.02,
                    organics_frac: 0.01,
                    gas_frac: 0.0,
                },
                body_type: BodyType::TerrestrialPlanet,
                angle_rad: 1.2,
            },
            GmcEmbryoSpec {
                name: format!("{star_name} c (Jovian Embryo)"),
                semi_major_axis_au: 5.2 * m_ratio.sqrt(),
                mass_solar: JUPITER_MASS_SOLAR,
                radius_au: JUPITER_RADIUS_AU,
                composition: Composition::solar_gas(),
                body_type: BodyType::GasGiant,
                angle_rad: 3.8,
            },
        ]
    } else if metallicity >= 0.005 {
        // Gen-II: Enriched Supernova Remnant protoplanets (Iron-Rich, Rocky Earth, Volatile Ocean)
        vec![
            GmcEmbryoSpec {
                name: format!("{star_name} b (Iron-Rich Embryo World)"),
                semi_major_axis_au: 1.4 * m_ratio.sqrt(),
                mass_solar: EARTH_MASS_SOLAR,
                radius_au: EARTH_RADIUS_AU,
                composition: Composition {
                    metal_frac: 0.60,
                    silicate_frac: 0.35,
                    ice_frac: 0.01,
                    organics_frac: 0.01,
                    gas_frac: 0.03,
                },
                body_type: BodyType::Protoplanet,
                angle_rad: 0.9,
            },
            GmcEmbryoSpec {
                name: format!("{star_name} c (Rocky Earth-Analogue Embryo)"),
                semi_major_axis_au: 2.8 * m_ratio.sqrt(),
                mass_solar: 2.0 * EARTH_MASS_SOLAR,
                radius_au: 1.3 * EARTH_RADIUS_AU,
                composition: Composition {
                    metal_frac: 0.30,
                    silicate_frac: 0.58,
                    ice_frac: 0.05,
                    organics_frac: 0.03,
                    gas_frac: 0.04,
                },
                body_type: BodyType::Protoplanet,
                angle_rad: 2.7,
            },
            GmcEmbryoSpec {
                name: format!("{star_name} d (Volatile Ocean Embryo World)"),
                semi_major_axis_au: 5.8 * m_ratio.sqrt(),
                mass_solar: 5.0 * EARTH_MASS_SOLAR,
                radius_au: 2.0 * EARTH_RADIUS_AU,
                composition: Composition {
                    metal_frac: 0.12,
                    silicate_frac: 0.36,
                    ice_frac: 0.35,
                    organics_frac: 0.12,
                    gas_frac: 0.05,
                },
                body_type: BodyType::Protoplanet,
                angle_rad: 4.8,
            },
        ]
    } else {
        // Gen-I: Pristine gas giant embryos (Pop-III)
        vec![
            GmcEmbryoSpec {
                name: format!("{star_name} b1 (Gas Giant Embryo)"),
                semi_major_axis_au: 12.5 * m_ratio.sqrt(),
                mass_solar: 3.0 * JUPITER_MASS_SOLAR,
                radius_au: 1.5 * JUPITER_RADIUS_AU,
                composition: Composition::pure_hydrogen(),
                body_type: BodyType::GasGiant,
                angle_rad: 2.4,
            },
            GmcEmbryoSpec {
                name: format!("{star_name} b2 (Gas Giant Embryo)"),
                semi_major_axis_au: 21.0 * m_ratio.sqrt(),
                mass_solar: 3.0 * JUPITER_MASS_SOLAR,
                radius_au: 1.5 * JUPITER_RADIUS_AU,
                composition: Composition::pure_hydrogen(),
                body_type: BodyType::GasGiant,
                angle_rad: 4.8,
            },
            GmcEmbryoSpec {
                name: format!("{star_name} b3 (Gas Giant Embryo)"),
                semi_major_axis_au: 29.5 * m_ratio.sqrt(),
                mass_solar: 3.0 * JUPITER_MASS_SOLAR,
                radius_au: 1.5 * JUPITER_RADIUS_AU,
                composition: Composition::pure_hydrogen(),
                body_type: BodyType::GasGiant,
                angle_rad: 1.1,
            },
        ]
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "GMC promotion coordinates disk attachment, embryo spawning, and active cap"
)]
pub fn apply_gmc_promotion(
    commands: &mut Commands,
    star_ent: Entity,
    star_name: &str,
    star_pos: DVec3,
    star_vel: DVec3,
    star_mass: f64,
    metallicity: f32,
    max_disks: usize,
    existing_disks: &mut Query<(Entity, &mut CircumstellarDisk, Option<&PlanetaryRingSystem>)>,
    toast: &mut Option<ResMut<crate::game::ui::NotificationToast>>,
) {
    // 1. Cap active full disks: dissipate oldest if at or above limit
    cap_gmc_active_full_disks(commands, max_disks, existing_disks);

    // 2. Attach small circumstellar disk & spawn embryos
    attach_gmc_circumstellar_disk_and_embryos(
        commands,
        star_ent,
        star_name,
        star_pos,
        star_vel,
        star_mass,
        metallicity,
        toast,
    );
}

/// Attaches a small circumstellar accretion disk and seeds orbiting protoplanetary embryos.
pub fn attach_gmc_circumstellar_disk_and_embryos(
    commands: &mut Commands,
    star_ent: Entity,
    star_name: &str,
    star_pos: DVec3,
    star_vel: DVec3,
    star_mass: f64,
    metallicity: f32,
    toast: &mut Option<ResMut<crate::game::ui::NotificationToast>>,
) {
    let m_ratio = (star_mass / 1.0).max(0.1);
    let inner_r = (0.60 * m_ratio.sqrt()).clamp(0.40, 2.0) as f32;
    let outer_r = (7.5 * m_ratio.powf(0.35)).clamp(4.5, 18.0) as f32;
    let disk_m = (0.015 * star_mass).clamp(0.005, 0.08);

    commands.entity(star_ent).insert((
        CircumstellarDisk::new(inner_r, outer_r, disk_m),
        PlanetaryRingSystem {
            inner_radius_au: inner_r,
            outer_radius_au: outer_r,
            ring_mass_earth: disk_m * 333_000.0 * 0.01,
            optical_depth: 0.82,
            ice_fraction: if metallicity >= 0.02 { 0.45 } else { 0.20 },
            silicate_fraction: if metallicity >= 0.02 { 0.55 } else { 0.80 },
        },
    ));

    let tangent = if star_vel.length_squared() > 1e-4 {
        star_vel.normalize()
    } else if star_pos.cross(DVec3::Y).length_squared() > 1e-4 {
        star_pos.cross(DVec3::Y).normalize()
    } else {
        DVec3::new(1.0, 0.0, 0.0)
    };
    let bitangent = tangent.cross(DVec3::Y).normalize();

    let embryos = get_gmc_promotion_embryos(star_name, star_mass, metallicity);
    let embryo_count = embryos.len();
    for e in embryos {
        let a_au = e.semi_major_axis_au;
        let v_circ = (G_ASTRO * star_mass / a_au).sqrt();
        let pos = star_pos
            + tangent * (a_au * e.angle_rad.cos())
            + bitangent * (a_au * e.angle_rad.sin());
        let vel = star_vel
            + tangent * (-v_circ * e.angle_rad.sin())
            + bitangent * (v_circ * e.angle_rad.cos());

        commands.spawn((
            CelestialBody {
                body_type: e.body_type,
                name: e.name,
            },
            Mass(e.mass_solar),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(e.radius_au),
            Temperature((280.0 / a_au.sqrt()).max(25.0)),
            Luminosity(0.0),
            AngularMomentum::default(),
            e.composition,
            VolatileInventory::default(),
            SpinState::default(),
        ));
    }

    let display_star_name = if let Some(idx) = star_name.strip_prefix("Protostar Gen-I Jeans-") {
        format!("Protostar Jeans-{idx}")
    } else {
        star_name.to_string()
    };

    if let Some(ref mut t) = toast {
        t.message = format!("{display_star_name}: disk attached ({embryo_count} embryos)");
        t.timer = 5.0;
    }

    bevy::log::info!(
        "{display_star_name}: disk attached ({embryo_count} embryos) [{inner_r:.1}-{outer_r:.1} AU]"
    );
}

/// When a protostar stabilizes (transitions to main sequence / ignited state) in the GMC scenario:
/// 1. Attaches a small circumstellar accretion disk (CircumstellarDisk + PlanetaryRingSystem).
/// 2. Spawns 2 to 4 protoplanetary embryos on stable Keplerian orbits.
/// 3. Caps active full 3D disks (default 4, within 3..=6) to preserve high GPU/CPU cluster performance,
///    dissipating older disks into their surviving planetary systems.
pub fn promote_stabilized_gmc_protostars(
    mut commands: Commands,
    scenario_state: Option<Res<ActiveScenarioState>>,
    disk_mgr: Option<Res<GmcClusterDiskManager>>,
    mut existing_disks: Query<(Entity, &mut CircumstellarDisk, Option<&PlanetaryRingSystem>)>,
    newly_stabilized: Query<
        (
            Entity,
            &SimPosition,
            &SimVelocity,
            &Mass,
            &CelestialBody,
            &IgnitionState,
        ),
        Without<CircumstellarDisk>,
    >,
    mut toast: Option<ResMut<crate::game::ui::NotificationToast>>,
) {
    let is_gmc = scenario_state.as_ref().is_some_and(|s| {
        s.current_preset == ScenarioPreset::MolecularCloudCluster
    });
    if !is_gmc && scenario_state.is_some() {
        return;
    }

    let max_disks = disk_mgr
        .as_ref()
        .map_or(MAX_GMC_ACTIVE_FULL_DISKS, |m| m.max_active_full_disks);

    for (star_ent, star_pos, star_vel, star_mass, body, ignition) in newly_stabilized.iter() {
        if !ignition.is_ignited {
            continue;
        }

        apply_gmc_promotion(
            &mut commands,
            star_ent,
            &body.name,
            star_pos.0,
            star_vel.0,
            star_mass.0,
            0.02, // Default cluster metallicity
            max_disks,
            &mut existing_disks,
            &mut toast,
        );
    }
}

/// Evolves and dissipates circumstellar accretion disks in the Giant Molecular Cloud cluster over time.
pub fn update_gmc_circumstellar_disks(
    mut commands: Commands,
    scenario_state: Option<Res<ActiveScenarioState>>,
    sim_time: Option<Res<SimTime>>,
    time_warp: Res<TimeWarp>,
    config: Res<SimulationConfig>,
    mut disk_query: Query<(Entity, &mut CircumstellarDisk, Option<&PlanetaryRingSystem>)>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }
    let is_gmc = scenario_state.as_ref().is_some_and(|s| {
        s.current_preset == ScenarioPreset::MolecularCloudCluster
    });
    if !is_gmc && scenario_state.is_some() {
        return;
    }

    let dt_yr = sim_time.as_deref().map_or_else(
        || (config.base_dt_yr * time_warp.multiplier.max(TimeWarp::MIN_SPEED)).min(5.0),
        |st| st.current_dt_yr.clamp(1e-6, 5.0),
    );

    for (star_ent, mut disk, opt_ring) in disk_query.iter_mut() {
        if disk.is_active_full_disk {
            disk.age_years += dt_yr;
            if disk.age_years >= disk.lifetime_years {
                disk.is_active_full_disk = false;
                if opt_ring.is_some() {
                    commands.entity(star_ent).remove::<PlanetaryRingSystem>();
                }
            }
        }
    }
}

/// Tracks the Center of Mass (COM) of the star cluster once 3+ stellar systems exist,
/// framing the multi-system cluster when the camera is not locked onto an individual target entity.
pub fn update_gmc_cluster_com_camera(
    scenario_state: Option<Res<ActiveScenarioState>>,
    mut camera_query: Query<&mut crate::rendering::camera::PanOrbitCamera>,
    stars_query: Query<(
        Entity,
        &SimPosition,
        &Mass,
        &CelestialBody,
        Option<&CircumstellarDisk>,
    )>,
) {
    let is_gmc = scenario_state.as_ref().is_some_and(|s| {
        s.current_preset == ScenarioPreset::MolecularCloudCluster
    });
    if !is_gmc && scenario_state.is_some() {
        return;
    }

    let Ok(mut camera) = camera_query.single_mut() else {
        return;
    };

    // If the user has focused / locked onto a specific target body, respect user focus
    if camera.target_entity.is_some() {
        return;
    }

    // Count systems (stars or protostars)
    let mut total_weighted_pos = DVec3::ZERO;
    let mut total_mass = 0.0f64;
    let mut star_positions = Vec::new();

    for (_e, pos, mass, body, _opt_disk) in stars_query.iter() {
        if body.body_type.is_star_or_remnant() || body.body_type == BodyType::Protostar {
            total_weighted_pos += pos.0 * mass.0;
            total_mass += mass.0;
            star_positions.push(pos.0);
        }
    }

    // Only activate once you have 3+ systems
    if star_positions.len() < 3 || total_mass <= 1e-6 {
        return;
    }

    let com = total_weighted_pos / total_mass;
    let com_vec3 = Vec3::new(com.x as f32, com.y as f32, com.z as f32);

    // Smoothly track Cluster Center of Mass
    camera.target_focus = com_vec3;

    // Calculate cluster radius from COM to frame the chaotic multi-system fantasy
    let mut max_dist = 0.0f64;
    for pos in &star_positions {
        let dist = (*pos - com).length();
        if dist > max_dist {
            max_dist = dist;
        }
    }

    let ideal_radius = (max_dist * 2.2).clamp(200.0, 1200.0) as f32;
    if camera.target_radius < ideal_radius * 0.75 {
        camera.target_radius = camera.target_radius.max(ideal_radius * 0.85);
    }
}
