//! Orbital mechanics math, Kepler orbit solvers, and coordinate transformations.

use bevy::math::{DVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

use crate::utils::constants::G_ASTRO;

/// Classical Keplerian Orbital Elements
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrbitalElements {
    /// Semi-major axis ($a$) in AU
    pub semi_major_axis: f64,
    /// Orbital Eccentricity ($e$), dimensionless ($0 \le e < 1$ for bound elliptical orbits)
    pub eccentricity: f64,
    /// Orbital Inclination ($i$) in radians relative to the disk plane (Y-axis normal)
    pub inclination: f64,
    /// Longitude of the Ascending Node ($\Omega$) in radians
    pub longitude_ascending_node: f64,
    /// Argument of Periapsis ($\omega$) in radians
    pub argument_of_periapsis: f64,
    /// True Anomaly ($\nu$) in radians
    pub true_anomaly: f64,
    /// Orbital Period ($P$) in years (for bound orbits)
    pub period_years: f64,
    /// Periapsis distance ($q = a(1-e)$) in AU
    pub periapsis: f64,
    /// Apoapsis distance ($Q = a(1+e)$) in AU (for bound orbits)
    pub apoapsis: f64,
    /// Specific orbital energy ($\mathcal{E} = -\mu / 2a$) in $(\text{AU/yr})^2$
    pub specific_energy: f64,
    /// Unit vector towards periapsis in 3D simulation space
    pub periapsis_dir: DVec3,
    /// Unit vector orthogonal to periapsis in orbital plane in direction of motion
    pub semilatus_dir: DVec3,
}

impl Default for OrbitalElements {
    fn default() -> Self {
        Self {
            semi_major_axis: 1.0,
            eccentricity: 0.0,
            inclination: 0.0,
            longitude_ascending_node: 0.0,
            argument_of_periapsis: 0.0,
            true_anomaly: 0.0,
            period_years: 1.0,
            periapsis: 1.0,
            apoapsis: 1.0,
            specific_energy: -G_ASTRO / 2.0,
            periapsis_dir: DVec3::X,
            semilatus_dir: DVec3::Z,
        }
    }
}

/// Computes Keplerian orbital elements from state vectors $(\vec{r}, \vec{v})$
/// relative to a central body with mass $M_{\text{central}}$ (and orbiting mass $m$).
pub fn state_vectors_to_orbital_elements(
    rel_pos: DVec3,
    rel_vel: DVec3,
    central_mass: f64,
    orbiting_mass: f64,
) -> Option<OrbitalElements> {
    let mu = G_ASTRO * (central_mass + orbiting_mass);
    if mu <= 0.0 {
        return None;
    }

    let r = rel_pos.length();
    let v = rel_vel.length();

    if r < 1e-7 || v < 1e-7 {
        return None;
    }

    // Specific angular momentum vector h = r x v
    let h_vec = rel_pos.cross(rel_vel);
    let h = h_vec.length();
    if h < 1e-12 {
        return None;
    }
    let h_hat = h_vec / h;

    // Specific orbital energy E = v^2 / 2 - mu / r
    let specific_energy = (v * v) / 2.0 - (mu / r);

    // Semi-major axis a = -mu / (2 * E)
    let a = if specific_energy.abs() > 1e-12 {
        -mu / (2.0 * specific_energy)
    } else {
        f64::INFINITY
    };

    // Eccentricity vector e_vec = (v x h)/mu - r / |r|
    let e_vec = (rel_vel.cross(h_vec) / mu) - (rel_pos / r);
    let e = e_vec.length();

    // Perifocal frame unit vectors:
    // P points towards periapsis. If near circular (e <= 1e-5), define P along current position
    let periapsis_dir = if e > 1e-5 { e_vec / e } else { rel_pos / r };

    // Q points in the orbital plane 90 degrees ahead of periapsis in direction of orbital motion
    let mut semilatus_dir = h_hat.cross(periapsis_dir);
    if semilatus_dir.length_squared() > 1e-12 {
        semilatus_dir = semilatus_dir.normalize();
    } else {
        semilatus_dir = DVec3::Z;
    }

    // Reference coordinate frame: protoplanetary disk in X-Z plane with normal along -Y (for prograde motion)
    let cos_inc = (-h_vec.y / h).clamp(-1.0, 1.0);
    let inclination = cos_inc.acos();

    // Line of nodes: intersection of orbital plane with disk plane (Y = 0)
    // n_vec = (0, -1, 0) x h_vec = (-h_vec.z, 0.0, h_vec.x)
    let n_vec = DVec3::new(-h_vec.z, 0.0, h_vec.x);
    let n = n_vec.length();

    let longitude_ascending_node = if n > 1e-12 {
        let mut omega_node = n_vec.z.atan2(n_vec.x);
        if omega_node < 0.0 {
            omega_node += 2.0 * PI;
        }
        omega_node
    } else {
        0.0
    };

    let argument_of_periapsis = if n > 1e-12 && e > 1e-12 {
        let cos_arg = (n_vec.dot(periapsis_dir) / n).clamp(-1.0, 1.0);
        let mut arg = cos_arg.acos();
        if n_vec.cross(periapsis_dir).dot(h_vec) < 0.0 {
            arg = 2.0 * PI - arg;
        }
        arg
    } else if e > 1e-12 {
        // Coplanar orbit: argument of periapsis is longitude of periapsis measured from +X axis
        let mut arg = periapsis_dir.z.atan2(periapsis_dir.x);
        if arg < 0.0 {
            arg += 2.0 * PI;
        }
        arg
    } else {
        0.0
    };

    // True Anomaly nu measured in the orbital plane (P, Q)
    let cos_nu = (rel_pos.dot(periapsis_dir) / r).clamp(-1.0, 1.0);
    let sin_nu = (rel_pos.dot(semilatus_dir) / r).clamp(-1.0, 1.0);
    let mut true_anomaly = sin_nu.atan2(cos_nu);
    if true_anomaly < 0.0 {
        true_anomaly += 2.0 * PI;
    }

    // Orbital Period P = 2 * PI * sqrt(a^3 / mu)
    let period_years = if a > 0.0 {
        2.0 * PI * (a.powi(3) / mu).sqrt()
    } else {
        0.0
    };

    let periapsis = if a > 0.0 {
        a * (1.0 - e)
    } else if a < 0.0 {
        a.abs() * (e - 1.0)
    } else {
        r
    };
    let apoapsis = if a > 0.0 && e < 1.0 {
        a * (1.0 + e)
    } else {
        f64::INFINITY
    };

    Some(OrbitalElements {
        semi_major_axis: a,
        eccentricity: e,
        inclination,
        longitude_ascending_node,
        argument_of_periapsis,
        true_anomaly,
        period_years,
        periapsis,
        apoapsis,
        specific_energy,
        periapsis_dir,
        semilatus_dir,
    })
}

/// Computes the 3D position in AU in Bevy coordinates for a given true anomaly.
/// Returns None if the true anomaly is beyond asymptotes or results in infinite radius.
pub fn position_at_true_anomaly(elements: &OrbitalElements, nu: f64) -> Option<Vec3> {
    let e = elements.eccentricity;
    let denom = 1.0 + e * nu.cos();
    if denom <= 1e-5 {
        return None;
    }

    let p = if (e - 1.0).abs() < 1e-6 {
        2.0 * elements.periapsis
    } else if e > 1.0 {
        elements.semi_major_axis.abs() * (e * e - 1.0)
    } else {
        if elements.semi_major_axis <= 0.0 {
            return None;
        }
        elements.semi_major_axis * (1.0 - e * e)
    };

    let r = p / denom;
    if r > 2000.0 || r <= 0.0 {
        return None;
    }

    let pos_3d = elements.periapsis_dir * (r * nu.cos()) + elements.semilatus_dir * (r * nu.sin());
    Some(Vec3::new(pos_3d.x as f32, pos_3d.y as f32, pos_3d.z as f32))
}

/// Generates a series of 3D orbital curve points in AU for visualization of bound orbits ($e < 1.0$).
pub fn generate_orbit_points(elements: &OrbitalElements, num_samples: usize) -> Vec<Vec3> {
    if elements.semi_major_axis <= 0.0 || elements.eccentricity >= 1.0 || num_samples < 4 {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(num_samples + 1);

    for i in 0..=num_samples {
        let nu = (i as f64 / num_samples as f64) * 2.0 * PI;
        if let Some(pt) = position_at_true_anomaly(elements, nu) {
            points.push(pt);
        }
    }

    points
}

/// Generates an open hyperbolic flyby trajectory in AU for unbound bodies ($e \ge 1.0$).
pub fn generate_hyperbolic_orbit_points(
    elements: &OrbitalElements,
    num_samples: usize,
) -> Vec<Vec3> {
    if elements.eccentricity < 1.0 || num_samples < 4 {
        return Vec::new();
    }

    let e = elements.eccentricity;
    // Asymptote angle: theta_inf = acos(-1/e). Bound sampling safely within asymptotes.
    let nu_max = ((-1.0 / e.max(1.0001)).clamp(-0.9999, 0.0).acos() - 0.08).max(0.2);

    let mut points = Vec::with_capacity(num_samples + 1);

    for i in 0..=num_samples {
        let frac = i as f64 / num_samples as f64;
        let nu = -nu_max + frac * 2.0 * nu_max;
        if let Some(pt) = position_at_true_anomaly(elements, nu) {
            points.push(pt);
        }
    }

    points
}

/// Generates an ordered series of 3D points and normalized alpha values ($1.0 \to 0.0$)
/// trailing backwards from the body's current true anomaly position along its orbit.
pub fn generate_trailing_ribbon_points(
    elements: &OrbitalElements,
    num_samples: usize,
    arc_radians: f64,
) -> Vec<(Vec3, f32)> {
    if num_samples < 2 {
        return Vec::new();
    }

    let e = elements.eccentricity;
    let nu_0 = elements.true_anomaly;
    let mut ribbon = Vec::with_capacity(num_samples + 1);

    if e < 1.0 {
        // Bound elliptical orbit: trace backwards along true anomaly
        for i in 0..=num_samples {
            let t = i as f64 / num_samples as f64;
            let nu = nu_0 - t * arc_radians;
            if let Some(pt) = position_at_true_anomaly(elements, nu) {
                let alpha = (1.0 - t as f32).max(0.0);
                ribbon.push((pt, alpha));
            }
        }
    } else {
        // Hyperbolic orbit: trace backwards from current position within valid bounds
        let nu_max = ((-1.0 / e.max(1.0001)).clamp(-0.9999, 0.0).acos() - 0.08).max(0.2);
        let min_nu = -nu_max;
        let start_nu = nu_0.clamp(-nu_max, nu_max);
        let end_nu = (start_nu - arc_radians).max(min_nu);

        for i in 0..=num_samples {
            let t = i as f64 / num_samples as f64;
            let nu = start_nu + t * (end_nu - start_nu);
            if let Some(pt) = position_at_true_anomaly(elements, nu) {
                let alpha = (1.0 - t as f32).max(0.0);
                ribbon.push((pt, alpha));
            }
        }
    }

    ribbon
}

/// Returns the periapsis and (optional) apoapsis positions in AU in Bevy coordinates.
pub fn apsides_positions(elements: &OrbitalElements) -> (Option<Vec3>, Option<Vec3>) {
    let peri = position_at_true_anomaly(elements, 0.0);
    let apo = if elements.eccentricity < 1.0 {
        position_at_true_anomaly(elements, PI)
    } else {
        None
    };
    (peri, apo)
}

/// Returns the ascending node and descending node positions in AU in Bevy coordinates.
pub fn nodal_positions(elements: &OrbitalElements) -> (Option<Vec3>, Option<Vec3>) {
    let omega = elements.argument_of_periapsis;
    let asc = position_at_true_anomaly(elements, -omega);
    let desc = if elements.eccentricity < 1.0 {
        position_at_true_anomaly(elements, PI - omega)
    } else {
        None
    };
    (asc, desc)
}
