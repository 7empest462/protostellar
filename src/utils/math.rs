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
    /// Orbital Inclination ($i$) in radians
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

    // Node vector n = k x h = (-h_y, h_x, 0)
    let n_vec = DVec3::new(-h_vec.y, h_vec.x, 0.0);
    let n = n_vec.length();

    // Inclination i = acos(h_z / h)
    let inclination = if h > 1e-12 {
        (h_vec.z / h).clamp(-1.0, 1.0).acos()
    } else {
        0.0
    };

    // Longitude of Ascending Node Omega
    let longitude_ascending_node = if n > 1e-12 {
        let omega_node = (n_vec.x / n).clamp(-1.0, 1.0).acos();
        if n_vec.y < 0.0 {
            2.0 * PI - omega_node
        } else {
            omega_node
        }
    } else {
        0.0
    };

    // Argument of Periapsis omega
    let argument_of_periapsis = if n > 1e-12 && e > 1e-12 {
        let cos_arg = (n_vec.dot(e_vec) / (n * e)).clamp(-1.0, 1.0);
        let arg = cos_arg.acos();
        if e_vec.z < 0.0 {
            2.0 * PI - arg
        } else {
            arg
        }
    } else if e > 1e-12 {
        // Equatorial orbit
        let cos_arg = (e_vec.x / e).clamp(-1.0, 1.0);
        let arg = cos_arg.acos();
        if e_vec.y < 0.0 {
            2.0 * PI - arg
        } else {
            arg
        }
    } else {
        0.0
    };

    // True Anomaly nu
    let true_anomaly = if e > 1e-12 {
        let cos_nu = (e_vec.dot(rel_pos) / (e * r)).clamp(-1.0, 1.0);
        let nu = cos_nu.acos();
        if rel_pos.dot(rel_vel) < 0.0 {
            2.0 * PI - nu
        } else {
            nu
        }
    } else {
        0.0
    };

    // Orbital Period P = 2 * PI * sqrt(a^3 / mu)
    let period_years = if a > 0.0 {
        2.0 * PI * (a.powi(3) / mu).sqrt()
    } else {
        0.0
    };

    let periapsis = if a > 0.0 { a * (1.0 - e) } else { r };
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
    })
}

/// Generates a series of 3D orbital curve points in AU for visualization.
pub fn generate_orbit_points(elements: &OrbitalElements, num_samples: usize) -> Vec<Vec3> {
    if elements.semi_major_axis <= 0.0 || elements.eccentricity >= 1.0 || num_samples < 4 {
        return Vec::new();
    }

    let a = elements.semi_major_axis;
    let e = elements.eccentricity;
    let inc = elements.inclination;
    let lan = elements.longitude_ascending_node;
    let arg_p = elements.argument_of_periapsis;

    let sin_inc = inc.sin();
    let cos_inc = inc.cos();
    let sin_lan = lan.sin();
    let cos_lan = lan.cos();
    let sin_arg = arg_p.sin();
    let cos_arg = arg_p.cos();

    let mut points = Vec::with_capacity(num_samples + 1);

    for i in 0..=num_samples {
        let nu = (i as f64 / num_samples as f64) * 2.0 * PI;
        let r = (a * (1.0 - e * e)) / (1.0 + e * nu.cos());

        // Position in orbital plane
        let x_orb = r * nu.cos();
        let y_orb = r * nu.sin();

        // Rotate by argument of periapsis, inclination, and ascending node
        let x_node = x_orb * cos_arg - y_orb * sin_arg;
        let y_node = x_orb * sin_arg + y_orb * cos_arg;

        let x_ecl = x_node * cos_lan - y_node * cos_inc * sin_lan;
        let y_ecl = x_node * sin_lan + y_node * cos_inc * cos_lan;
        let z_ecl = y_node * sin_inc;

        // Bevy 3D coordinate system: X = right, Y = up (Z in astro), Z = towards viewer
        points.push(Vec3::new(x_ecl as f32, z_ecl as f32, y_ecl as f32));
    }

    points
}
