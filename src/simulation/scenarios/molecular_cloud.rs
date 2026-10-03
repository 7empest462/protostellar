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
use crate::simulation::resources::DiskParameters;
use crate::utils::constants::*;

/// Total gas and stellar mass of the dense molecular cloud core in Solar Masses.
pub const GMC_CORE_MASS_SOLAR: f64 = 24.0;

/// Outer boundary radius of the collapsing molecular cloud core in AU.
pub const GMC_CORE_RADIUS_AU: f64 = 550.0;

/// Ambient cold molecular gas temperature in Kelvin (T ~ 10-15 K).
pub const GMC_CLOUD_TEMPERATURE_K: f64 = 15.0;

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
