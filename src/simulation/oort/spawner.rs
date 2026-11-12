//! Procedural generation and initialization of the isotropic Oort Cloud cometary swarm.

use std::f64::consts::PI;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::utils::constants::{G_ASTRO, YEAR_SECONDS};
use crate::utils::math::FastRngExt;

use super::types::*;

/// Solves Kepler's equation $M = E - e \sin E$ for the eccentric anomaly $E$ using Newton-Raphson.
pub fn solve_kepler_eccentric_anomaly(mean_anomaly: f64, eccentricity: f64) -> f64 {
    let mut e_anom = if eccentricity > 0.8 { PI } else { mean_anomaly };

    for _ in 0..12 {
        let delta = (e_anom - eccentricity * e_anom.sin() - mean_anomaly)
            / (1.0 - eccentricity * e_anom.cos()).max(1e-8);
        e_anom -= delta;
        if delta.abs() < 1e-10 {
            break;
        }
    }
    e_anom
}

/// Computes Cartesian state vectors $(\vec{r}, \vec{v})$ from Keplerian orbital elements
/// with the reference disk in the X-Z plane and prograde normal along $-\hat{Y}$.
pub fn keplerian_elements_to_cartesian(
    semi_major_axis_au: f64,
    eccentricity: f64,
    inclination_rad: f64,
    longitude_node_rad: f64,
    argument_periapsis_rad: f64,
    true_anomaly_rad: f64,
    star_mass_solar: f64,
) -> (DVec3, DVec3) {
    let a = semi_major_axis_au;
    let e = eccentricity.clamp(0.0, 0.9999);
    let nu = true_anomaly_rad;
    let mu = G_ASTRO * star_mass_solar.max(0.1);

    let p = a * (1.0 - e * e);
    let r = p / (1.0 + e * nu.cos()).max(1e-6);

    let omega_node = longitude_node_rad;
    let omega_peri = argument_periapsis_rad;
    let inc = inclination_rad;

    // Perifocal unit vectors P and Q mapped to Protostellar disk coordinates
    let cos_om = omega_node.cos();
    let sin_om = omega_node.sin();
    let cos_w = omega_peri.cos();
    let sin_w = omega_peri.sin();
    let cos_i = inc.cos();
    let sin_i = inc.sin();

    let p_vec = DVec3::new(
        cos_om * cos_w - sin_om * sin_w * cos_i,
        sin_w * sin_i,
        sin_om * cos_w + cos_om * sin_w * cos_i,
    );

    let q_vec = DVec3::new(
        -cos_om * sin_w - sin_om * cos_w * cos_i,
        cos_w * sin_i,
        -sin_om * sin_w + cos_om * cos_w * cos_i,
    );

    let pos = (r * nu.cos()) * p_vec + (r * nu.sin()) * q_vec;

    let v_factor = (mu / p.max(1e-6)).sqrt();
    let vel = (-v_factor * nu.sin()) * p_vec + (v_factor * (e + nu.cos())) * q_vec;

    (pos, vel)
}

/// Procedurally spawns an isotropic, spherical swarm of long-period cometary bodies.
pub fn spawn_oort_cloud_swarm(
    commands: &mut Commands,
    config: &OortCloudConfig,
    star_mass_solar: f64,
    star_pos: DVec3,
    comet_count: usize,
    rng: &mut fastrand::Rng,
) {
    let count = if comet_count == 0 {
        config.default_comet_count
    } else {
        comet_count
    };

    let inner_r = config.inner_radius_au.max(100.0);
    let outer_r = config.outer_radius_au.max(inner_r + 500.0);

    for i in 0..count {
        // 1. Semi-major axis: 35% in inner Hills cloud (200-2,000 AU), 65% in outer cloud (2,000-25,000 AU)
        let is_inner_hills = rng.f64() < 0.35;
        let a = if is_inner_hills {
            let hills_outer = 2_000.0f64.min(outer_r);
            inner_r * (hills_outer / inner_r).powf(rng.f64())
        } else {
            let outer_inner = 2_000.0f64.max(inner_r);
            outer_inner * (outer_r / outer_inner).powf(rng.f64())
        };

        // 2. Thermalized eccentricity distribution f(e) = 2e, clamped to prevent initial collision
        let e = (rng.f64()).sqrt().clamp(0.15, 0.97);

        // 3. Perfectly isotropic spherical inclination: uniform in cos(i) in [-1, 1]
        let cos_i = rng.f64_range(-1.0, 1.0);
        let inc = cos_i.acos();

        // 4. Uniform orbital angles
        let omega_node = rng.f64_range(0.0, 2.0 * PI);
        let omega_peri = rng.f64_range(0.0, 2.0 * PI);
        let mean_anom = rng.f64_range(0.0, 2.0 * PI);

        // 5. Solve for true anomaly
        let e_anom = solve_kepler_eccentric_anomaly(mean_anom, e);
        let true_anom =
            2.0 * (((1.0 + e) / (1.0 - e).max(1e-6)).sqrt() * (e_anom * 0.5).tan()).atan();

        // 6. Convert to 3D Cartesian position and velocity
        let (rel_pos, rel_vel) = keplerian_elements_to_cartesian(
            a,
            e,
            inc,
            omega_node,
            omega_peri,
            true_anom,
            star_mass_solar,
        );

        let pos = star_pos + rel_pos;
        let vel = rel_vel;

        // Cometary mass: kilometer-scale icy bodies (10^13 to 10^16 kg in solar masses)
        let mass_solar = rng.f64_range(5.0e-18, 5.0e-15);
        let radius_km = rng.f64_range(1.5, 25.0);
        let radius_au = radius_km / 149_597_870.7;

        let comp = Composition {
            metal_frac: 0.05,
            silicate_frac: 0.10,
            ice_frac: 0.80,
            organics_frac: 0.05,
            gas_frac: 0.0,
        };

        let temp_k = (278.0 / a.sqrt()).clamp(5.0, 40.0);

        let perihelion = a * (1.0 - e);

        let oort_comet = OortComet {
            semi_major_axis_au: a,
            eccentricity: e,
            inclination_rad: inc,
            argument_of_periapsis_rad: omega_peri,
            original_perihelion_au: perihelion,
            current_perihelion_au: perihelion,
            volatile_ice_fraction: 0.85,
            outgassing_rate: 0.0,
            is_infall_trajectory: perihelion <= config.sublimation_distance_au,
            is_sungrazer: perihelion <= config.sungrazer_threshold_au,
            is_active_sublimating: false,
            cumulative_volatile_loss_kg: 0.0,
        };

        let mut diff = InternalDifferentiation::default();
        diff.recalculate(mass_solar, radius_au, &comp);

        let mut spin = SpinState::default();
        let spin_period_hrs = rng.f64_range(5.0, 48.0);
        let omega = 2.0 * PI / (spin_period_hrs * 3600.0 / YEAR_SECONDS);
        let initial_spin = 0.33 * mass_solar * radius_au * radius_au * DVec3::new(0.0, omega, 0.0);
        spin.update_from_spin(initial_spin, mass_solar, radius_au);

        let vol = VolatileInventory {
            delivered_water_m_earth: 0.0,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: 0.0,
            cometary_impact_count: 0,
        };

        commands.spawn((
            CelestialBody {
                body_type: BodyType::Comet,
                name: format!("Oort Comet C/{}", 1000 + i),
            },
            Mass(mass_solar),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(radius_au),
            Temperature(temp_k),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * mass_solar),
            comp,
            diff,
            spin,
            vol,
            oort_comet,
        ));
    }
}

/// Handles on-demand requests to spawn an Oort Cloud cometary swarm around the central star.
pub fn handle_spawn_oort_cloud_swarm_system(
    mut commands: Commands,
    mut swarm_triggers: MessageReader<TriggerOortCloudSwarmEvent>,
    star_query: Query<(&SimPosition, &Mass), With<CentralStar>>,
    config: Res<OortCloudConfig>,
) {
    for event in swarm_triggers.read() {
        let (star_pos, star_mass) = if let Ok((pos, mass)) = star_query.single() {
            (pos.0, mass.0)
        } else {
            (DVec3::ZERO, 1.0)
        };
        let count = if event.comet_count == 0 {
            config.default_comet_count
        } else {
            event.comet_count
        };
        let mut rng = fastrand::Rng::new();
        spawn_oort_cloud_swarm(&mut commands, &config, star_mass, star_pos, count, &mut rng);
    }
}
