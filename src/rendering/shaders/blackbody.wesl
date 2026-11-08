/// Planckian locus blackbody color approximation for temperatures from 800 K to 50,000 K.
/// Maps physical thermodynamic temperature (Kelvin) to normalized linear RGB.
fn blackbody_temperature_to_rgb(temp_kelvin: f32) -> vec3<f32> {
    let t = clamp(temp_kelvin, 800.0, 50000.0) / 100.0;
    var r = 0.0;
    var g = 0.0;
    var b = 0.0;

    // Red
    if (t <= 66.0) {
        r = 1.0;
    } else {
        r = clamp(1.292936 * pow(t - 60.0, -0.1332047592), 0.0, 1.0);
    }

    // Green
    if (t <= 66.0) {
        g = clamp(0.3900815 * log(t) - 0.6318414, 0.0, 1.0);
    } else {
        g = clamp(1.12989 * pow(t - 60.0, -0.0755148), 0.0, 1.0);
    }

    // Blue
    if (t >= 66.0) {
        b = 1.0;
    } else if (t <= 19.0) {
        b = 0.0;
    } else {
        b = clamp(0.5432067 * log(t - 10.0) - 1.196254, 0.0, 1.0);
    }

    return vec3<f32>(r, g, b);
}

/// Relativistic Doppler shift frequency multiplier applied to blackbody radiation.
/// beta_los = v / c along line of sight (+ approaching / blue-shift, - receding / red-shift).
fn doppler_shift_temperature(temp_kelvin: f32, beta_los: f32) -> f32 {
    let factor = sqrt((1.0 + beta_los) / max(1.0 - beta_los, 1e-4));
    return temp_kelvin * factor;
}
