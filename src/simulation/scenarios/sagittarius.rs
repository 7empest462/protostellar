//! Galactic Center Scenario: Sagittarius A* Supermassive Black Hole and Relativistic S-Star Cluster.
//!
//! Models the central 4.3 million solar mass supermassive black hole (Sgr A*)
//! orbited by the high-velocity relativistic S-star cluster (S2, S4714, S62, S29, S38)
//! and the tidally sheared G2 dusty gas cloud.
//!
//! Features extreme 1PN Schwarzschild rosette precession (up to 12.8° per orbit for S4714),
//! relativistic orbital speeds up to ~8% the speed of light at pericenter,
//! and observable gravitational redshift / transverse Doppler effects (z ~ 0.0007).

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::relativity::types::RelativisticState;
use crate::simulation::resources::DiskParameters;
use crate::utils::constants::*;

/// Mass of Sagittarius A* in Solar Masses (canonical GRAVITY Collaboration / Keck measurement).
pub const SGR_A_STAR_MASS_SOLAR: f64 = 4.297e6;

/// Configuration for an S-star on an inclined eccentric Keplerian orbit around Sgr A*.
struct SStarOrbitConfig {
    name: &'static str,
    body_type: BodyType,
    mass_solar: f64,
    radius_solar: f64,
    temp_k: f64,
    lum_solar: f64,
    semi_major_axis_au: f64,
    eccentricity: f64,
    inclination_rad: f64,
    arg_periapsis_rad: f64,
    long_asc_node_rad: f64,
    composition: Composition,
}

fn calculate_3d_pericenter_state(
    a_au: f64,
    e: f64,
    inc: f64,
    omega: f64,
    node: f64,
    m_central: f64,
    m_body: f64,
) -> (DVec3, DVec3) {
    let r_peri = a_au * (1.0 - e);
    let mu = G_ASTRO * (m_central + m_body);
    let v_peri = (mu * (2.0 / r_peri - 1.0 / a_au)).sqrt();

    // Standard Gaussian orbital orientation vectors P and Q
    let cos_node = node.cos();
    let sin_node = node.sin();
    let cos_omega = omega.cos();
    let sin_omega = omega.sin();
    let cos_inc = inc.cos();
    let sin_inc = inc.sin();

    // P points toward periapsis
    let p_vec = DVec3::new(
        cos_node * cos_omega - sin_node * sin_omega * cos_inc,
        sin_omega * sin_inc,
        sin_node * cos_omega + cos_node * sin_omega * cos_inc,
    );

    // Q is tangent to the orbit at periapsis in the direction of motion
    let q_vec = DVec3::new(
        -cos_node * sin_omega - sin_node * cos_omega * cos_inc,
        cos_omega * sin_inc,
        -sin_node * sin_omega + cos_node * cos_omega * cos_inc,
    );

    let pos = r_peri * p_vec;
    let vel = v_peri * q_vec;

    (pos, vel)
}

fn spawn_s_star(commands: &mut Commands, cfg: &SStarOrbitConfig, m_central: f64) -> Entity {
    let (pos, vel) = calculate_3d_pericenter_state(
        cfg.semi_major_axis_au,
        cfg.eccentricity,
        cfg.inclination_rad,
        cfg.arg_periapsis_rad,
        cfg.long_asc_node_rad,
        m_central,
        cfg.mass_solar,
    );

    let phys_radius = cfg.radius_solar * SOLAR_RADIUS_AU;

    commands
        .spawn((
            CelestialBody {
                body_type: cfg.body_type,
                name: cfg.name.to_string(),
            },
            Mass(cfg.mass_solar),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(phys_radius),
            Temperature(cfg.temp_k),
            Luminosity(cfg.lum_solar),
            AngularMomentum(pos.cross(vel) * cfg.mass_solar),
            cfg.composition,
            SpinState {
                rotation_period_hours: 24.0,
                axial_tilt_degrees: cfg.inclination_rad.to_degrees() * 0.5,
                spin_vector: DVec3::Y,
            },
            RelativisticState::default(),
        ))
        .id()
}

fn spawn_sagittarius_a_black_hole(commands: &mut Commands) -> Entity {
    // Schwarzschild radius of Sgr A*: r_s = 2 G M / c^2 ≈ 0.0848 AU
    let r_schwa = (1.974e-8 * SGR_A_STAR_MASS_SOLAR).max(1e-4);

    // Central Supermassive Black Hole
    commands
        .spawn((
            CelestialBody {
                body_type: BodyType::BlackHole,
                name: "Sagittarius A* (Sgr A*)".to_string(),
            },
            CentralStar,
            Mass(SGR_A_STAR_MASS_SOLAR),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            SimAcceleration::default(),
            Radius(r_schwa),
            Temperature(1.0e-7),
            Luminosity(120.0), // Quiescent Shakura-Sunyaev / RIAF accretion glow
            AngularMomentum::default(),
            Composition::pure_hydrogen(),
            RelativisticState::default(),
        ))
        .id()
}

fn spawn_inner_s_stars(commands: &mut Commands) {
    let s_stars = [
        // S4716: Shortest known orbital period around Sgr A* (P ≈ 4.0 yr, v_peri ≈ 8,000 km/s)
        SStarOrbitConfig {
            name: "S4716 (4-Yr Sprinter)",
            body_type: BodyType::MainSequenceStar,
            mass_solar: 2.0,
            radius_solar: 2.0,
            temp_k: 12_500.0,
            lum_solar: 45.0,
            semi_major_axis_au: 410.0,
            eccentricity: 0.756,
            inclination_rad: 161.24f64.to_radians(),
            arg_periapsis_rad: 0.073f64.to_radians(),
            long_asc_node_rad: 151.54f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S4711: Faint fast-moving core star (P ≈ 7.6 yr, a ≈ 628 AU, e = 0.768)
        SStarOrbitConfig {
            name: "S4711",
            body_type: BodyType::MainSequenceStar,
            mass_solar: 2.2,
            radius_solar: 2.1,
            temp_k: 10_500.0,
            lum_solar: 38.0,
            semi_major_axis_au: 628.0,
            eccentricity: 0.768,
            inclination_rad: 114.71f64.to_radians(),
            arg_periapsis_rad: 131.59f64.to_radians(),
            long_asc_node_rad: 20.10f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S62: Ultra-close pericenter grazer (r_peri ≈ 17.8 AU, v_peri ≈ 21,000 km/s ~ 7% c)
        SStarOrbitConfig {
            name: "S62",
            body_type: BodyType::BlueGiant,
            mass_solar: 6.1,
            radius_solar: 4.2,
            temp_k: 18_000.0,
            lum_solar: 1_200.0,
            semi_major_axis_au: 749.0,
            eccentricity: 0.976,
            inclination_rad: 72.8f64.to_radians(),
            arg_periapsis_rad: 42.6f64.to_radians(),
            long_asc_node_rad: 122.6f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S4714: Fastest known velocity grazer (a ≈ 120 AU, e = 0.985, v_peri ~ 8-20% c, 12.8°/orbit precession)
        SStarOrbitConfig {
            name: "S4714 (Fastest Star)",
            body_type: BodyType::MainSequenceStar,
            mass_solar: 2.0,
            radius_solar: 1.9,
            temp_k: 9_500.0,
            lum_solar: 35.0,
            semi_major_axis_au: 120.0,
            eccentricity: 0.985,
            inclination_rad: 127.7f64.to_radians(),
            arg_periapsis_rad: 357.25f64.to_radians(),
            long_asc_node_rad: 129.28f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S55 (S0-102): Benchmark short-period test star (P ≈ 12.8 yr, a ≈ 890 AU, e = 0.721)
        SStarOrbitConfig {
            name: "S55 (S0-102)",
            body_type: BodyType::BlueGiant,
            mass_solar: 10.0,
            radius_solar: 5.5,
            temp_k: 26_000.0,
            lum_solar: 9_500.0,
            semi_major_axis_au: 890.0,
            eccentricity: 0.721,
            inclination_rad: 150.10f64.to_radians(),
            arg_periapsis_rad: 331.50f64.to_radians(),
            long_asc_node_rad: 325.50f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S2 (S0-2): The Nobel test star (P ≈ 16 yr, v_peri ≈ 7,700 km/s, 1PN rosette precession)
        SStarOrbitConfig {
            name: "S2 (S0-2)",
            body_type: BodyType::BlueGiant,
            mass_solar: 14.0,
            radius_solar: 7.0,
            temp_k: 28_000.0,
            lum_solar: 25_000.0,
            semi_major_axis_au: 1034.0,
            eccentricity: 0.8843,
            inclination_rad: 133.91f64.to_radians(),
            arg_periapsis_rad: 66.25f64.to_radians(),
            long_asc_node_rad: 228.07f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
    ];

    for s_star in &s_stars {
        spawn_s_star(commands, s_star, SGR_A_STAR_MASS_SOLAR);
    }
}

fn spawn_intermediate_s_stars(commands: &mut Commands) {
    let s_stars = [
        // S38: High-eccentricity giant (P ≈ 19.2 yr, a ≈ 1,165 AU, e = 0.820)
        SStarOrbitConfig {
            name: "S38",
            body_type: BodyType::BlueGiant,
            mass_solar: 7.0,
            radius_solar: 4.8,
            temp_k: 20_500.0,
            lum_solar: 2_500.0,
            semi_major_axis_au: 1165.0,
            eccentricity: 0.820,
            inclination_rad: 171.10f64.to_radians(),
            arg_periapsis_rad: 17.99f64.to_radians(),
            long_asc_node_rad: 101.06f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S13: Low-inclination B-star (P ≈ 49.0 yr, a ≈ 2,177 AU, e = 0.425)
        SStarOrbitConfig {
            name: "S13",
            body_type: BodyType::MainSequenceStar,
            mass_solar: 4.0,
            radius_solar: 3.0,
            temp_k: 15_000.0,
            lum_solar: 380.0,
            semi_major_axis_au: 2177.0,
            eccentricity: 0.425,
            inclination_rad: 24.70f64.to_radians(),
            arg_periapsis_rad: 245.20f64.to_radians(),
            long_asc_node_rad: 74.50f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S9: Inclined B-star (P ≈ 51.3 yr, a ≈ 2,245 AU, e = 0.644)
        SStarOrbitConfig {
            name: "S9",
            body_type: BodyType::MainSequenceStar,
            mass_solar: 4.2,
            radius_solar: 3.1,
            temp_k: 15_500.0,
            lum_solar: 420.0,
            semi_major_axis_au: 2245.0,
            eccentricity: 0.644,
            inclination_rad: 82.41f64.to_radians(),
            arg_periapsis_rad: 150.60f64.to_radians(),
            long_asc_node_rad: 156.60f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S14 (S0-16): Deep-dive relativistic grazer (r_peri ≈ 56.4 AU, v_peri ≈ 11,600 km/s)
        SStarOrbitConfig {
            name: "S14 (S0-16)",
            body_type: BodyType::BlueGiant,
            mass_solar: 5.0,
            radius_solar: 3.4,
            temp_k: 16_000.0,
            lum_solar: 650.0,
            semi_major_axis_au: 2360.0,
            eccentricity: 0.9761,
            inclination_rad: 100.59f64.to_radians(),
            arg_periapsis_rad: 334.59f64.to_radians(),
            long_asc_node_rad: 226.38f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S12: Highly inclined B-star (P ≈ 58.9 yr, a ≈ 2,461 AU, e = 0.888)
        SStarOrbitConfig {
            name: "S12",
            body_type: BodyType::MainSequenceStar,
            mass_solar: 4.8,
            radius_solar: 3.3,
            temp_k: 16_500.0,
            lum_solar: 550.0,
            semi_major_axis_au: 2461.0,
            eccentricity: 0.888,
            inclination_rad: 33.56f64.to_radians(),
            arg_periapsis_rad: 317.90f64.to_radians(),
            long_asc_node_rad: 230.10f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S4: Moderate eccentric B-star (P ≈ 77.0 yr, a ≈ 2,942 AU, e = 0.3905)
        SStarOrbitConfig {
            name: "S4",
            body_type: BodyType::BlueGiant,
            mass_solar: 6.0,
            radius_solar: 4.0,
            temp_k: 18_000.0,
            lum_solar: 1_100.0,
            semi_major_axis_au: 2942.0,
            eccentricity: 0.3905,
            inclination_rad: 80.33f64.to_radians(),
            arg_periapsis_rad: 290.80f64.to_radians(),
            long_asc_node_rad: 258.84f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
    ];

    for s_star in &s_stars {
        spawn_s_star(commands, s_star, SGR_A_STAR_MASS_SOLAR);
    }
}

fn spawn_outer_s_stars_and_clouds(commands: &mut Commands) {
    let s_stars = [
        // S8: Outer eccentric B-star (P ≈ 92.9 yr, a ≈ 3,334 AU, e = 0.8031)
        SStarOrbitConfig {
            name: "S8",
            body_type: BodyType::BlueGiant,
            mass_solar: 6.5,
            radius_solar: 4.3,
            temp_k: 19_000.0,
            lum_solar: 1_500.0,
            semi_major_axis_au: 3334.0,
            eccentricity: 0.8031,
            inclination_rad: 74.37f64.to_radians(),
            arg_periapsis_rad: 346.70f64.to_radians(),
            long_asc_node_rad: 315.43f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S29: Outer B-star (P ≈ 101.0 yr, a ≈ 3,526 AU, e = 0.728)
        SStarOrbitConfig {
            name: "S29",
            body_type: BodyType::MainSequenceStar,
            mass_solar: 4.5,
            radius_solar: 3.1,
            temp_k: 14_500.0,
            lum_solar: 450.0,
            semi_major_axis_au: 3526.0,
            eccentricity: 0.728,
            inclination_rad: 105.80f64.to_radians(),
            arg_periapsis_rad: 346.50f64.to_radians(),
            long_asc_node_rad: 161.96f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // S1: Classic outer cluster anchor (P ≈ 166.0 yr, a ≈ 4,909 AU, e = 0.556)
        SStarOrbitConfig {
            name: "S1",
            body_type: BodyType::BlueGiant,
            mass_solar: 8.0,
            radius_solar: 5.0,
            temp_k: 22_000.0,
            lum_solar: 4_200.0,
            semi_major_axis_au: 4909.0,
            eccentricity: 0.556,
            inclination_rad: 119.14f64.to_radians(),
            arg_periapsis_rad: 122.30f64.to_radians(),
            long_asc_node_rad: 342.04f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // Pistol Star (V4647 Sagittarii): Iconic Galactic Center Blue Hypergiant Landmark
        SStarOrbitConfig {
            name: "Pistol Star (V4647 Sgr)",
            body_type: BodyType::Hypergiant,
            mass_solar: 105.0,
            radius_solar: 320.0,
            temp_k: 11_800.0,
            lum_solar: 1_600_000.0, // Over 1.6 million times solar luminosity
            semi_major_axis_au: 9800.0,
            eccentricity: 0.280,
            inclination_rad: 45.0f64.to_radians(),
            arg_periapsis_rad: 30.0f64.to_radians(),
            long_asc_node_rad: 60.0f64.to_radians(),
            composition: Composition::pure_hydrogen(),
        },
        // G2: Tidally sheared dusty gas cloud
        SStarOrbitConfig {
            name: "G2 (Dust & Gas Cloud)",
            body_type: BodyType::Comet,
            mass_solar: 3.0e-5, // ~10 Earth masses
            radius_solar: 32.0, // Extended warm gas envelope (~0.15 AU)
            temp_k: 550.0,
            lum_solar: 0.01,
            semi_major_axis_au: 600.0,
            eccentricity: 0.965,
            inclination_rad: 62.0f64.to_radians(),
            arg_periapsis_rad: 85.0f64.to_radians(),
            long_asc_node_rad: 45.0f64.to_radians(),
            composition: Composition {
                metal_frac: 0.01,
                silicate_frac: 0.04,
                ice_frac: 0.10,
                organics_frac: 0.05,
                gas_frac: 0.80,
            },
        },
        // G1: Compact gas filament (passed pericenter in 2001)
        SStarOrbitConfig {
            name: "G1 (Gas & Dust Filament)",
            body_type: BodyType::Comet,
            mass_solar: 2.5e-5,
            radius_solar: 28.0,
            temp_k: 580.0,
            lum_solar: 0.01,
            semi_major_axis_au: 900.0,
            eccentricity: 0.980,
            inclination_rad: 109.0f64.to_radians(),
            arg_periapsis_rad: 115.0f64.to_radians(),
            long_asc_node_rad: 155.0f64.to_radians(),
            composition: Composition {
                metal_frac: 0.01,
                silicate_frac: 0.04,
                ice_frac: 0.10,
                organics_frac: 0.05,
                gas_frac: 0.80,
            },
        },
    ];

    for s_star in &s_stars {
        spawn_s_star(commands, s_star, SGR_A_STAR_MASS_SOLAR);
    }
}

/// Spawns the Sagittarius A* supermassive black hole and its relativistic S-star cluster.
pub fn spawn_sagittarius_a_star_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    // Accretion disk settings for the supermassive Galactic Center environment
    disk_params.central_star_mass = SGR_A_STAR_MASS_SOLAR;
    disk_params.inner_radius_au = 0.25;
    disk_params.outer_radius_au = 1500.0;
    disk_params.disk_mass = 0.00001; // Quiescent gas background

    let sgr_a = spawn_sagittarius_a_black_hole(commands);
    spawn_inner_s_stars(commands);
    spawn_intermediate_s_stars(commands);
    spawn_outer_s_stars_and_clouds(commands);

    sgr_a
}
