//! Type-I / Type-II disk migration torques (analytic, game-scaled).
//!
//! While nebular gas remains, planets exchange angular momentum with the disk
//! and drift. Type-I: embedded embryos. Type-II: gap-opening giants (slower).

use bevy::math::DVec3;
use std::f64::consts::PI;

use crate::simulation::components::BodyType;
use crate::simulation::resources::DiskParameters;
use crate::utils::constants::*;

const GAME_SCALE: f64 = 2.5e-2;

/// Bodies that feel disk torques (not stars, dust, or already-bound moons).
pub fn is_migrator(body_type: BodyType, is_central: bool, is_satellite: bool) -> bool {
    if is_central || is_satellite {
        return false;
    }
    matches!(
        body_type,
        BodyType::Planetesimal
            | BodyType::Protoplanet
            | BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::GasGiant
            | BodyType::IceGiant
            | BodyType::Asteroid
            | BodyType::Comet
    )
}

/// Physical gas surface density $\Sigma(r)$ in $M_\odot/\text{AU}^2$ normalized against `DiskParameters`.
pub fn local_gas_sigma_msun_au2(
    r_au: f64,
    gas_density_scale: f64,
    disk_params: &DiskParameters,
) -> f64 {
    if gas_density_scale <= 1e-4 || disk_params.disk_mass <= 1e-6 || r_au < 0.05 {
        return 0.0;
    }
    let r_in = disk_params.inner_radius_au.max(0.05);
    let r_out = disk_params.outer_radius_au.max(r_in + 1.0);
    if r_au < r_in || r_au > r_out * 1.25 {
        return 0.0;
    }
    let p = disk_params.density_power_law.clamp(0.5, 2.5);
    let exponent = 2.0 - p;
    let integral_factor = if exponent.abs() < 1e-4 {
        (r_out / r_in).ln()
    } else {
        (r_out.powf(exponent) - r_in.powf(exponent)) / exponent
    };
    let sigma_0 = disk_params.disk_mass / (2.0 * PI * integral_factor.max(1e-6));
    let inner_taper = if r_au < r_in * 1.15 {
        ((r_au - r_in) / (0.15 * r_in)).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let outer_taper = if r_au > r_out {
        (1.0 - (r_au - r_out) / (0.25 * r_out)).clamp(0.0, 1.0)
    } else {
        1.0
    };
    sigma_0 * r_au.powf(-p) * gas_density_scale * inner_taper * outer_taper
}

/// Disk aspect ratio h = H/r (thicker outer disk).
fn aspect_ratio(r_au: f64) -> f64 {
    (0.05 * (r_au / 1.0).powf(0.25)).clamp(0.03, 0.10)
}

/// Thermal mass scale ~ (h^3) M_star — above this, treat as Type-II (slower).
fn type_ii_mass_threshold(h: f64, star_mass: f64) -> f64 {
    (h * h * h) * star_mass * 3.0
}

/// Tangential acceleration from Type-I / II torque (AU/yr²), scaling physically with $\Sigma r^2 / M_*$.
pub fn migration_tangential_acc(
    mass_solar: f64,
    r_au: f64,
    star_mass: f64,
    gas_density_scale: f32,
    body_type: BodyType,
    disk_params: &DiskParameters,
) -> f64 {
    let sigma = local_gas_sigma_msun_au2(r_au, f64::from(gas_density_scale), disk_params);
    if sigma <= 1e-18 || star_mass <= 1e-6 {
        return 0.0;
    }

    let h = aspect_ratio(r_au);
    let omega = (G_ASTRO * star_mass / (r_au * r_au * r_au).max(1e-12)).sqrt();

    // Dimensionless Type-I torque scaling: (M_p/M_*) * (Sigma r^2 / M_*) / h^2 * Omega^2 * r
    let m_ratio = (mass_solar / star_mass).clamp(1e-10, 0.05);
    let local_disk_mass_ratio = ((sigma * r_au * r_au) / star_mass).clamp(0.0, 0.5);
    let mut strength = m_ratio * local_disk_mass_ratio * (omega * omega) * r_au / (h * h);

    // Game scale: visible but not instant at 1–100× warp
    strength *= GAME_SCALE;

    // Type-II: gap openers migrate slower
    let m_th = type_ii_mass_threshold(h, star_mass);
    if mass_solar > m_th || matches!(body_type, BodyType::GasGiant | BodyType::IceGiant) {
        strength *= 0.15;
    }

    // Smooth transition across inner hot-Jupiter boost (~1 AU) and outer disk (~20 AU)
    let radial_boost = 1.5
        - 0.5 * ((r_au - 0.5) / 1.0).clamp(0.0, 1.0)
        - 0.4 * ((r_au - 15.0) / 10.0).clamp(0.0, 1.0);

    // Negative = remove angular momentum = inward drift
    -strength * radial_boost
}

/// Apply disk migration acceleration to one body (mutates `acc`) along local azimuthal direction
/// opposing its orbital angular momentum $\vec{h}$ (so both prograde and retrograde bodies lose $|h|$).
#[allow(
    clippy::too_many_arguments,
    reason = "Disk migration torque requires disk density profile parameters and orbital state"
)]
pub fn apply_type_i_torque_acc(
    pos: DVec3,
    vel: DVec3,
    mass: f64,
    body_type: BodyType,
    is_central: bool,
    is_satellite: bool,
    star_mass: f64,
    gas_density_scale: f32,
    disk_params: &DiskParameters,
    acc: &mut DVec3,
) {
    if !is_migrator(body_type, is_central, is_satellite) {
        return;
    }

    let r_cyl = (pos.x * pos.x + pos.z * pos.z).sqrt().max(0.05);
    let a_t = migration_tangential_acc(
        mass,
        r_cyl,
        star_mass,
        gas_density_scale,
        body_type,
        disk_params,
    );
    if a_t.abs() < 1e-18 {
        return;
    }

    // Prograde azimuthal unit vector in the XZ disk plane: phi_hat = (-z, 0, x) / r_cyl.
    // Orient against the body's actual azimuthal velocity so retrograde orbits experience drag too.
    let phi_hat = DVec3::new(-pos.z / r_cyl, 0.0, pos.x / r_cyl);
    let oriented_phi = if vel.dot(phi_hat) < 0.0 {
        -phi_hat
    } else {
        phi_hat
    };
    *acc += oriented_phi * a_t;
}
