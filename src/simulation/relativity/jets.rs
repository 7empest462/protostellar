//! Analytical relativistic jet dynamics, Doppler beaming, and synchrotron emission formulas.
//!
//! Models relativistic plasma outflow from compact objects (Pulsars, Magnetars, Black Holes)
//! including Lorentz contraction, forward Doppler beaming, Lense-Thirring precession,
//! and non-thermal synchrotron radiation scaling.

use bevy::math::{Quat, Vec3};

/// Calculates normalized relativistic velocity $\beta = v/c$ from Lorentz factor $\Gamma$.
///
/// $$\beta = \sqrt{1 - \frac{1}{\Gamma^2}}$$
///
/// Guaranteed to return $\beta \in [0.0, 1.0)$.
pub fn calculate_relativistic_velocity(lorentz_factor: f32) -> f32 {
    if lorentz_factor <= 1.0 {
        return 0.0;
    }
    let gamma = lorentz_factor;
    let beta = (1.0 - 1.0 / (gamma * gamma)).max(0.0).sqrt();
    beta.min(0.999_999)
}

/// Calculates the characteristic relativistic beaming half-angle $\theta_{\text{beam}}$ in radians.
///
/// $$\theta_{\text{beam}} = \frac{1}{\Gamma}$$
pub fn calculate_beaming_half_angle(lorentz_factor: f32) -> f32 {
    1.0 / lorentz_factor.max(1.0)
}

/// Calculates the kinematic relativistic Doppler factor $\delta$.
///
/// $$\delta = \frac{1}{\Gamma (1 - \beta \cos\theta)}$$
///
/// where $\theta$ is the angle between the jet velocity vector and the observer's line-of-sight.
pub fn calculate_doppler_factor(lorentz_factor: f32, cos_theta: f32) -> f32 {
    let gamma = lorentz_factor.max(1.0001);
    let beta = calculate_relativistic_velocity(gamma);
    let clamped_cos = cos_theta.clamp(-1.0, 1.0);
    let denominator = gamma * (1.0 - beta * clamped_cos);
    1.0 / denominator.max(1e-4)
}

/// Calculates the relativistic Doppler flux boosting amplification factor $D = \delta^{3 + \alpha}$.
///
/// For an optically thin continuous synchrotron jet with electron energy index $p$,
/// the spectral index is $\alpha = (p - 1)/2$.
///
/// Clamped to a stable dynamic range $[0.05, 20.0]$ for realistic visual flaring without HDR blowout.
pub fn calculate_doppler_boosting(doppler_factor: f32, spectral_index: f32) -> f32 {
    let alpha = (spectral_index - 1.0) * 0.5;
    let exponent = 3.0 + alpha;
    let boosted = doppler_factor.max(1e-4).powf(exponent);
    boosted.clamp(0.05, 20.0)
}

/// Evaluates normalized non-thermal synchrotron radiance scaling.
///
/// $$I_{\text{synch}} \propto B^{(p+1)/2} \nu^{-(p-1)/2}$$
///
/// where $B$ is the local magnetic field in Gauss and $p$ is the electron energy index.
pub fn calculate_synchrotron_flux(
    b_field_gauss: f64,
    frequency_ghz: f64,
    spectral_index: f32,
) -> f32 {
    let b_norm = (b_field_gauss.max(1.0) / 1.0e12).clamp(1e-6, 1.0e4);
    let freq_norm = (frequency_ghz.max(0.1) / 10.0).clamp(0.01, 100.0);
    let p = f64::from(spectral_index.clamp(1.5, 4.0));

    let b_power = f64::midpoint(p, 1.0);
    let freq_power = (p - 1.0) * 0.5;

    let flux = b_norm.powf(b_power) * freq_norm.powf(-freq_power);
    (flux as f32).clamp(0.01, 100.0)
}

/// Calculates precessing jet unit pointing vector given an orbital/spin normal and precession cone angle.
///
/// Applies a continuous conical rotation around `precession_axis` by angle `cone_angle_rad` at phase $\phi$.
pub fn calculate_precessing_jet_direction(
    base_axis: Vec3,
    precession_axis: Vec3,
    cone_angle_rad: f32,
    phase: f32,
) -> Vec3 {
    let axis = if precession_axis.length_squared() > 1e-4 {
        precession_axis.normalize()
    } else {
        Vec3::Y
    };

    let base = if base_axis.length_squared() > 1e-4 {
        base_axis.normalize()
    } else {
        axis
    };

    // 1. Tilt base vector away from precession axis by cone_angle
    let orthogonal = if axis.dot(Vec3::X).abs() < 0.9 {
        axis.cross(Vec3::X).normalize()
    } else {
        axis.cross(Vec3::Z).normalize()
    };
    let tilted = Quat::from_axis_angle(orthogonal, cone_angle_rad) * base;

    // 2. Precess tilted vector around precession axis by phase angle
    let rot = Quat::from_axis_angle(axis, phase * std::f32::consts::TAU);
    (rot * tilted).normalize()
}
