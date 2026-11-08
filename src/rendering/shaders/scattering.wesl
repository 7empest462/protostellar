/// Atmospheric Chapman spherical airmass function.
/// Models the curvature of the atmosphere at grazing incidence angles,
/// preventing infinite optical depth at the terminator.
fn chapman_spherical_airmass(r_over_h: f32, cos_chi: f32) -> f32 {
    let c = max(cos_chi, -0.2);
    let x = max(r_over_h, 1.0);
    if (c >= 0.0) {
        return 1.0 / (c + 0.15 * pow(c + 0.01, -0.68) * pow(x, -0.5));
    }
    let z = max(0.001, -c);
    let ch0 = sqrt(1.5707963 * x);
    return ch0 / (1.0 + z * x * 0.45);
}

/// Dual-lobe Cornette-Shanks / Henyey-Greenstein aerosol phase function:
///   - Forward lobe: silver-lining forward scattering around host star (g_fwd ~ 0.76..0.88)
///   - Backward lobe: glory / opposition backscatter (g_back ~ -0.25..-0.35)
fn dual_lobe_mie_phase(cos_theta: f32, g_fwd: f32) -> f32 {
    let g_back = -0.35 * g_fwd;
    let w_fwd = 0.72;
    let w_back = 1.0 - w_fwd;

    let g2_f = g_fwd * g_fwd;
    let denom_f = max(1.0 + g2_f - 2.0 * g_fwd * cos_theta, 1e-4);
    let p_fwd = (1.5 / 12.566370) * (1.0 - g2_f) / (pow(denom_f, 1.5) * (2.0 + g2_f));

    let g2_b = g_back * g_back;
    let denom_b = max(1.0 + g2_b - 2.0 * g_back * cos_theta, 1e-4);
    let p_back = (1.5 / 12.566370) * (1.0 - g2_b) / (pow(denom_b, 1.5) * (2.0 + g2_b));

    return w_fwd * p_fwd + w_back * p_back;
}

/// Standard single-lobe Henyey-Greenstein phase function for cosmic dust and nebulae.
fn henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = max(1.0 + g2 - 2.0 * g * cos_theta, 1e-4);
    return (1.0 / (4.0 * 3.14159265)) * (1.0 - g2) / pow(denom, 1.5);
}
