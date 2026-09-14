//! Type-I / Type-II disk migration torques (analytic, game-scaled).
//!
//! While nebular gas remains, planets exchange angular momentum with the disk
//! and drift. Type-I: embedded embryos. Type-II: gap-opening giants (slower).

use bevy::math::DVec3;

use crate::simulation::components::BodyType;
use crate::utils::constants::*;

const GAME_SCALE: f64 = 2.5e-3;

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

/// Local gas surface-density scale (relative units), matching accretion/gas.rs shape.
pub fn local_gas_sigma_scale(r_au: f64, gas_density_scale: f64) -> f64 {
    if gas_density_scale <= 1e-4 || r_au < 0.05 {
        return 0.0;
    }
    // Σ ∝ r^{-1.5} × global scale (Hayashi-like)
    let sigma = (r_au / 1.0).powf(-1.50).max(0.0);
    sigma * gas_density_scale
}

/// Disk aspect ratio h = H/r (thicker outer disk).
fn aspect_ratio(r_au: f64) -> f64 {
    (0.05 * (r_au / 1.0).powf(0.25)).clamp(0.03, 0.10)
}

/// Thermal mass scale ~ (h^3) M_star — above this, treat as Type-II (slower).
fn type_ii_mass_threshold(h: f64, star_mass: f64) -> f64 {
    (h * h * h) * star_mass * 3.0
}

/// Tangential acceleration from Type-I / II torque (AU/yr²).
///
/// Negative along velocity → inward migration (usual Type-I).
/// Magnitude is game-tuned so drift is visible over 10³–10⁵ yr at warp.
pub fn migration_tangential_acc(
    mass_solar: f64,
    r_au: f64,
    star_mass: f64,
    gas_density_scale: f32,
    body_type: BodyType,
) -> f64 {
    let sigma = local_gas_sigma_scale(r_au, f64::from(gas_density_scale));
    if sigma <= 1e-12 {
        return 0.0;
    }

    let h = aspect_ratio(r_au);
    let omega = (G_ASTRO * star_mass / (r_au * r_au * r_au).max(1e-12)).sqrt();

    // Dimensionless Type-I factor ~ (M_p/M_*)² (Σ r² / M_*) / h²
    // Compact form: a_t ∝ - (M_p / M_*) * sigma * omega² * r / h²
    let m_ratio = (mass_solar / star_mass.max(1e-6)).clamp(1e-10, 0.05);
    let mut strength = m_ratio * sigma * (omega * omega) * r_au / (h * h);

    // Game scale: visible but not instant at 1–100× warp
    strength *= GAME_SCALE;

    // Type-II: gap openers migrate slower
    let m_th = type_ii_mass_threshold(h, star_mass);
    if mass_solar > m_th || matches!(body_type, BodyType::GasGiant | BodyType::IceGiant) {
        strength *= 0.15;
    }

    // Very close-in: stronger (hot-Jupiter pathway); outer disk: milder
    let radial_boost = if r_au < 1.0 {
        1.5
    } else if r_au > 20.0 {
        0.6
    } else {
        1.0
    };

    // Negative = remove angular momentum = inward drift
    -strength * radial_boost
}

/// Apply disk migration acceleration to one body (mutates `acc`).
pub fn apply_type_i_torque_acc(
    pos: DVec3,
    vel: DVec3,
    mass: f64,
    body_type: BodyType,
    is_central: bool,
    is_satellite: bool,
    star_mass: f64,
    gas_density_scale: f32,
    acc: &mut DVec3,
) {
    if !is_migrator(body_type, is_central, is_satellite) {
        return;
    }

    let r_cyl = (pos.x * pos.x + pos.z * pos.z).sqrt().max(0.05);
    let a_t = migration_tangential_acc(mass, r_cyl, star_mass, gas_density_scale, body_type);
    if a_t.abs() < 1e-18 {
        return;
    }

    // Tangential unit vector in the orbital plane (along velocity projected)
    let mut v_plane = DVec3::new(vel.x, 0.0, vel.z);
    if v_plane.length_squared() < 1e-16 {
        // Fallback: prograde azimuthal direction
        v_plane = DVec3::new(-pos.z, 0.0, pos.x);
    }
    let t_hat = v_plane.normalize_or_zero();

    *acc += t_hat * a_t;
}
