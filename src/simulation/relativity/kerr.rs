//! Kerr metric analytical solutions, ergosphere boundaries, ISCO shifts, and Lense-Thirring frame dragging.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::post_newtonian::RAD_TO_ARCSEC;
use crate::utils::constants::{G_ASTRO, SPEED_OF_LIGHT_AU_YR};

/// Represents the Kerr metric properties of a rotating black hole or compact object.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct KerrBlackHole {
    /// Dimensionless spin parameter $a_* = c J / (G M^2) \in (-1.0, 1.0)$.
    /// Positive values denote prograde rotation relative to the reference orbital plane;
    /// negative values denote retrograde rotation.
    pub spin_param: f64,
    /// Unit 3D spin angular momentum vector $\hat{S}$.
    pub spin_axis: DVec3,
    /// Outer event horizon radius $r_+ = r_g (1 + \sqrt{1 - a_*^2})$ in AU.
    pub event_horizon_radius_au: f64,
    /// Outer ergosphere (static limit) equatorial radius $r_E(\pi/2) = 2 r_g = r_s$ in AU.
    pub ergosphere_equatorial_radius_au: f64,
    /// Innermost Stable Circular Orbit (ISCO) radius $r_{\text{ISCO}}(a_*)$ in AU.
    pub isco_radius_au: f64,
    /// Frame-dragging angular velocity at event horizon $\Omega_H = \frac{a_* c}{2 r_+}$ in $\text{rad/yr}$.
    pub horizon_angular_velocity_rad_yr: f64,
}

impl Default for KerrBlackHole {
    fn default() -> Self {
        Self::schwarzschild(1.0)
    }
}

impl KerrBlackHole {
    /// Constructs a non-rotating Schwarzschild black hole ($a_* = 0$).
    pub fn schwarzschild(mass_solar: f64) -> Self {
        Self::new(mass_solar, 0.0, DVec3::Y)
    }

    /// Constructs a Kerr black hole with a given mass, dimensionless spin parameter $a_* \in (-1.0, 1.0)$,
    /// and unit spin orientation axis.
    pub fn new(mass_solar: f64, spin_param: f64, spin_axis: DVec3) -> Self {
        let a_star = spin_param.clamp(-0.9999, 0.9999);
        let axis = if spin_axis.length_squared() > 1e-12 {
            spin_axis.normalize()
        } else {
            DVec3::Y
        };

        let r_g = calculate_gravitational_radius(mass_solar);
        let r_plus = calculate_kerr_event_horizon(mass_solar, a_star);
        let r_erg_eq = 2.0 * r_g;
        let r_isco = calculate_kerr_isco_radius(mass_solar, a_star);
        let omega_h = calculate_horizon_angular_velocity(mass_solar, a_star);

        Self {
            spin_param: a_star,
            spin_axis: axis,
            event_horizon_radius_au: r_plus,
            ergosphere_equatorial_radius_au: r_erg_eq,
            isco_radius_au: r_isco,
            horizon_angular_velocity_rad_yr: omega_h,
        }
    }

    /// Constructs a calibrated Kerr black hole for Sagittarius A* based on the canonical
    /// GRAVITY Collaboration and Event Horizon Telescope (EHT) measurements ($a_* \approx 0.90$).
    pub fn sagittarius_a() -> Self {
        use crate::simulation::scenarios::sagittarius::SGR_A_STAR_MASS_SOLAR;
        Self::new(SGR_A_STAR_MASS_SOLAR, 0.90, DVec3::Y)
    }

    /// Constructs a calibrated near-extremal Kerr black hole for Messier 87* ($M87^*$, $a_* \approx 0.94$).
    pub fn m87() -> Self {
        Self::new(6.5e9, 0.94, DVec3::Y)
    }

    /// Recalculates event horizon, ergosphere, and ISCO radii when the black hole mass changes.
    pub fn recalculate(&mut self, mass_solar: f64) {
        let r_g = calculate_gravitational_radius(mass_solar);
        self.event_horizon_radius_au = calculate_kerr_event_horizon(mass_solar, self.spin_param);
        self.ergosphere_equatorial_radius_au = 2.0 * r_g;
        self.isco_radius_au = calculate_kerr_isco_radius(mass_solar, self.spin_param);
        self.horizon_angular_velocity_rad_yr =
            calculate_horizon_angular_velocity(mass_solar, self.spin_param);
    }
}

/// Computes the gravitational radius $r_g = \frac{G M}{c^2}$ in AU.
///
/// Note that $r_g = \frac{1}{2} r_s$ where $r_s$ is the Schwarzschild radius.
pub fn calculate_gravitational_radius(mass_solar: f64) -> f64 {
    let c = SPEED_OF_LIGHT_AU_YR;
    (G_ASTRO * mass_solar.max(0.0)) / (c * c)
}

/// Computes the outer event horizon radius $r_+$ for a Kerr black hole in AU:
///
/// $$r_+ = r_g \left(1 + \sqrt{1 - a_*^2}\right)$$
pub fn calculate_kerr_event_horizon(mass_solar: f64, spin_param: f64) -> f64 {
    let r_g = calculate_gravitational_radius(mass_solar);
    let a_star = spin_param.clamp(-0.9999, 0.9999);
    let root = (1.0 - a_star * a_star).max(0.0).sqrt();
    r_g * (1.0 + root)
}

/// Computes the outer ergosphere (static limit) radius $r_E(\theta)$ in AU as a function of polar angle $\theta$:
///
/// $$r_E(\theta) = r_g \left(1 + \sqrt{1 - a_*^2 \cos^2\theta}\right)$$
///
/// - Equator ($\theta = \pi/2$): $r_E = 2 r_g = r_s$ (independent of spin)
/// - Poles ($\theta = 0, \pi$): $r_E = r_g (1 + \sqrt{1 - a_*^2}) = r_+$ (touches the horizon)
pub fn calculate_kerr_ergosphere_radius(mass_solar: f64, spin_param: f64, theta_rad: f64) -> f64 {
    let r_g = calculate_gravitational_radius(mass_solar);
    let a_star = spin_param.clamp(-0.9999, 0.9999);
    let cos_t = theta_rad.cos();
    let root = (1.0 - a_star * a_star * cos_t * cos_t).max(0.0).sqrt();
    r_g * (1.0 + root)
}

/// Computes the Innermost Stable Circular Orbit (ISCO) radius $r_{\text{ISCO}}$ in AU for a Kerr black hole
/// using the exact Bardeen, Press, & Teukolsky (1972) analytical solution:
///
/// $$Z_1 = 1 + (1 - a_*^2)^{1/3} \left[(1 + a_*)^{1/3} + (1 - a_*)^{1/3}\right]$$
/// $$Z_2 = \sqrt{3 a_*^2 + Z_1^2}$$
/// $$r_{\text{ISCO}} = r_g \left[3 + Z_2 \mp \sqrt{(3 - Z_1)(3 + Z_1 + 2 Z_2)}\right]$$
///
/// where the negative sign is taken for prograde orbits ($a_* \ge 0$) and the positive sign for retrograde orbits ($a_* < 0$).
pub fn calculate_kerr_isco_radius(mass_solar: f64, spin_param: f64) -> f64 {
    let r_g = calculate_gravitational_radius(mass_solar);
    let a_star = spin_param.clamp(-0.9999, 0.9999);
    let a2 = a_star * a_star;

    let term1 = (1.0 - a2).max(0.0).cbrt();
    let term2 = (1.0 + a_star).max(0.0).cbrt() + (1.0 - a_star).max(0.0).cbrt();
    let z1 = 1.0 + term1 * term2;
    let z2 = (3.0 * a2 + z1 * z1).max(0.0).sqrt();

    let radicand = ((3.0 - z1) * (3.0 + z1 + 2.0 * z2)).max(0.0);
    let root_term = radicand.sqrt();

    let isco_factor = if a_star >= 0.0 {
        3.0 + z2 - root_term
    } else {
        3.0 + z2 + root_term
    };

    r_g * isco_factor.max(0.5)
}

/// Computes the frame-dragging angular velocity at the event horizon $\Omega_H = \frac{a_* c}{2 r_+}$ in $\text{rad/yr}$.
pub fn calculate_horizon_angular_velocity(mass_solar: f64, spin_param: f64) -> f64 {
    let r_plus = calculate_kerr_event_horizon(mass_solar, spin_param);
    if r_plus <= 1e-12 {
        return 0.0;
    }
    let c = SPEED_OF_LIGHT_AU_YR;
    let a_star = spin_param.clamp(-0.9999, 0.9999);
    (a_star * c) / (2.0 * r_plus)
}

/// Computes the Lense-Thirring nodal precession rate for an inclined orbit around a Kerr black hole:
/// - First element: rate in radians per year ($\text{rad/yr}$)
/// - Second element: rate in arcseconds per century ($\text{arcsec/century}$)
///
/// $$\dot{\Omega}_{\text{LT}} = \frac{2 a_* G^2 M^2}{c^3 a^3 (1 - e^2)^{3/2}}$$
pub fn calculate_lense_thirring_precession_rate(
    m_central_solar: f64,
    spin_param: f64,
    a_au: f64,
    eccentricity: f64,
) -> (f64, f64) {
    if m_central_solar <= 1e-9 || a_au <= 1e-9 {
        return (0.0, 0.0);
    }
    let a_star = spin_param.clamp(-0.9999, 0.9999);
    if a_star.abs() <= 1e-9 {
        return (0.0, 0.0);
    }
    let e = eccentricity.clamp(0.0, 0.999);
    let one_minus_e2_3_2 = (1.0 - e * e).max(1e-6).powf(1.5);

    let c = SPEED_OF_LIGHT_AU_YR;
    let c3 = c * c * c;
    let gm = G_ASTRO * m_central_solar;
    let gm_sq = gm * gm;

    let rate_rad_yr = (2.0 * a_star * gm_sq) / (c3 * a_au.powi(3) * one_minus_e2_3_2);
    let rate_arcsec_century = rate_rad_yr * RAD_TO_ARCSEC * 100.0;

    (rate_rad_yr, rate_arcsec_century)
}

/// Computes the Post-Newtonian gravito-magnetic (Lense-Thirring) frame-dragging acceleration vector $\vec{a}_{\text{LT}}$ in $\text{AU/yr}^2$.
///
/// $$\vec{a}_{\text{LT}} = \frac{2 G}{c^2 r^3} \left[ \vec{v} \times \left( \frac{3 (\vec{J} \cdot \vec{r}) \vec{r}}{r^2} - \vec{J} \right) \right]$$
///
/// where $\vec{J} = \frac{a_* G M^2}{c} \hat{S}$ is the spin angular momentum of the black hole.
pub fn calculate_lense_thirring_acceleration(
    rel_pos: DVec3,
    rel_vel: DVec3,
    m_central_solar: f64,
    spin_param: f64,
    spin_axis: DVec3,
) -> DVec3 {
    let r_sq = rel_pos.length_squared();
    if r_sq < 1e-12 || m_central_solar <= 1e-9 {
        return DVec3::ZERO;
    }
    let a_star = spin_param.clamp(-0.9999, 0.9999);
    if a_star.abs() <= 1e-9 {
        return DVec3::ZERO;
    }

    let c = SPEED_OF_LIGHT_AU_YR;
    let c_sq = c * c;
    let r = r_sq.sqrt();
    let r3 = (r * r_sq).max(1e-12);
    let r_hat = rel_pos / r;

    let s_hat = if spin_axis.length_squared() > 1e-12 {
        spin_axis.normalize()
    } else {
        DVec3::Y
    };

    // J = (a_* G M^2) / c * s_hat
    let j_mag = (a_star * G_ASTRO * m_central_solar * m_central_solar) / c;
    let j_vec = s_hat * j_mag;

    // B_g dipole field: (3 (J . r_hat) r_hat - J)
    let dipole_field = 3.0 * j_vec.dot(r_hat) * r_hat - j_vec;

    // a_LT = (2 G / c^2 r^3) * (v x dipole_field)
    let prefactor = (2.0 * G_ASTRO) / (c_sq * r3);
    let cross_term = rel_vel.cross(dipole_field);

    let a_vec = cross_term * prefactor;
    // Safety clamp to avoid numerical singularity inside event horizon
    let max_accel = 1e12;
    if a_vec.length_squared() > max_accel * max_accel {
        a_vec.normalize() * max_accel
    } else {
        a_vec
    }
}
