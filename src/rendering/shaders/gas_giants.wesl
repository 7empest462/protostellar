import protostellar::rendering::shaders::noise::fbm;

struct PolarHexagonResult {
    inside_mask: f32,
    jet_mask: f32,
    eye_mask: f32,
    color: vec3<f32>,
    is_active: bool,
}

// Evaluates Saturn-like standing Rossby wave polar hexagon (k=6 wavenumber)
// with high-speed jet perimeter, deep cyclonic eye, and golden interior haze.
fn evaluate_polar_hexagon(
    p_gas: vec3<f32>,
    t: f32,
    hex_amplitude: f32,
    hex_wavenumber: f32,
) -> PolarHexagonResult {
    var res: PolarHexagonResult;
    res.inside_mask = 0.0;
    res.jet_mask = 0.0;
    res.eye_mask = 0.0;
    res.color = vec3<f32>(0.0);
    res.is_active = false;

    if (hex_amplitude < 0.001 || p_gas.y < 0.65) {
        return res;
    }

    let colatitude = acos(clamp(p_gas.y, -1.0, 1.0));
    let azimuth = atan2(p_gas.z, p_gas.x);
    let k = max(round(hex_wavenumber), 3.0);

    let r0 = 0.218;
    let wave_phase = k * azimuth - t * 0.006;
    let r_hex = r0 * (1.0 + hex_amplitude * cos(wave_phase) + (hex_amplitude * 0.22) * cos(2.0 * wave_phase));
    let delta_r = colatitude - r_hex;

    let jet = exp(-pow(delta_r / 0.018, 2.0));
    let inside = smoothstep(0.025, -0.025, delta_r);
    let eye = smoothstep(0.055, 0.018, colatitude);
    let eyewall = smoothstep(0.015, 0.038, colatitude) * smoothstep(0.065, 0.042, colatitude);
    let interior_turb = fbm(p_gas * 14.0 + vec3<f32>(t * 0.03, 0.0, -t * 0.03));

    let haze_gold = vec3<f32>(0.88, 0.68, 0.30) * (0.85 + interior_turb * 0.30);
    let jet_cream = vec3<f32>(0.98, 0.91, 0.70);
    let eye_dark = vec3<f32>(0.28, 0.14, 0.05);
    let eyewall_bright = vec3<f32>(0.95, 0.82, 0.52);

    var storm_col = mix(haze_gold, jet_cream, jet * 0.85);
    storm_col = mix(storm_col, eyewall_bright, eyewall * 0.90);
    storm_col = mix(storm_col, eye_dark, eye * 0.95);

    res.inside_mask = inside;
    res.jet_mask = jet;
    res.eye_mask = eye;
    res.color = storm_col;
    res.is_active = true;
    return res;
}

struct AnticyclonicVortexResult {
    spot_mask: f32,
    collar_mask: f32,
    eye_mask: f32,
    wake_mask: f32,
    oval_mask: f32,
    color: vec3<f32>,
    is_active: bool,
}

// Evaluates Jovian Great Red Spot anticyclone (aspect ratio ~2.2:1),
// high-speed outer collar, calm core, Kelvin-Helmholtz wake, Oval BA & white ovals.
fn evaluate_anticyclonic_vortices(
    p_gas: vec3<f32>,
    t: f32,
    spot_size: f32,
    spot_lat: f32,
    spot_lon: f32,
    vortex_spin_rate: f32,
    secondary_count: f32,
    mass_jup: f32,
    is_saturn: bool,
) -> AnticyclonicVortexResult {
    var res: AnticyclonicVortexResult;
    res.spot_mask = 0.0;
    res.collar_mask = 0.0;
    res.eye_mask = 0.0;
    res.wake_mask = 0.0;
    res.oval_mask = 0.0;
    res.color = vec3<f32>(0.0);
    res.is_active = false;

    if (spot_size < 0.01) {
        return res;
    }

    let lat = asin(clamp(p_gas.y, -1.0, 1.0));
    let lon = atan2(p_gas.z, p_gas.x);

    let d_lat = lat - spot_lat;
    var d_lon = lon - spot_lon;
    d_lon = d_lon - 6.2831853 * round(d_lon / 6.2831853);

    let u = d_lon / (1.50 * spot_size);
    let v = d_lat / (0.68 * spot_size);
    let r_ell = sqrt(u * u + v * v);

    let collar = exp(-pow((r_ell - 0.88) / 0.15, 2.0));
    let body = smoothstep(1.04, 0.84, r_ell);
    let eye = smoothstep(0.35, 0.12, r_ell);

    let vortex_angle = atan2(v, u);
    let spiral_angle = vortex_angle * 3.0 - vortex_spin_rate * t * 0.45 - r_ell * 7.0;
    let core_filaments = sin(spiral_angle + fbm(p_gas * 18.0) * 1.5) * 0.5 + 0.5;

    var wake = 0.0;
    if (d_lon > (0.5 * spot_size) && d_lon < (3.6 * spot_size) && abs(d_lat) < (0.9 * spot_size)) {
        let wake_x = (d_lon - 0.5 * spot_size) / (3.1 * spot_size);
        let wake_y = d_lat / (0.9 * spot_size);
        let wake_envelope = (1.0 - wake_x) * exp(-wake_y * wake_y * 3.0);
        let wake_turb = fbm(p_gas * 24.0 + vec3<f32>(t * 0.12, 0.0, 0.0));
        wake = wake_envelope * smoothstep(0.38, 0.82, wake_turb);
    }

    var spot_core_col = vec3<f32>(0.92, 0.26, 0.10);
    var collar_col = vec3<f32>(0.84, 0.45, 0.20);
    var eye_col = vec3<f32>(0.72, 0.16, 0.08);

    if (is_saturn) {
        spot_core_col = vec3<f32>(0.98, 0.98, 0.95);
        collar_col    = vec3<f32>(0.92, 0.86, 0.74);
        eye_col       = vec3<f32>(0.85, 0.76, 0.60);
    } else if (mass_jup > 6.0) {
        spot_core_col = vec3<f32>(0.85, 0.18, 0.65);
        collar_col    = vec3<f32>(0.70, 0.30, 0.85);
        eye_col       = vec3<f32>(0.55, 0.10, 0.45);
    } else if (mass_jup > 3.5) {
        spot_core_col = vec3<f32>(0.18, 0.65, 0.95);
        collar_col    = vec3<f32>(0.35, 0.80, 0.98);
        eye_col       = vec3<f32>(0.10, 0.45, 0.75);
    } else if (mass_jup > 1.8) {
        spot_core_col = vec3<f32>(0.15, 0.88, 0.65);
        collar_col    = vec3<f32>(0.30, 0.95, 0.80);
        eye_col       = vec3<f32>(0.08, 0.65, 0.48);
    }

    var grs_col = mix(spot_core_col, collar_col, collar * 0.55);
    grs_col = mix(grs_col, eye_col, eye * 0.75 + core_filaments * 0.20);
    grs_col = grs_col * (0.90 + core_filaments * 0.20);

    var oval_mask = 0.0;
    if (secondary_count >= 1.0) {
        let ba_lat = spot_lat - 0.18;
        var ba_dlon = lon - (spot_lon + 1.65);
        ba_dlon = ba_dlon - 6.2831853 * round(ba_dlon / 6.2831853);
        let ba_u = ba_dlon / (0.85 * spot_size);
        let ba_v = (lat - ba_lat) / (0.48 * spot_size);
        let ba_r = sqrt(ba_u * ba_u + ba_v * ba_v);
        let ba_body = smoothstep(1.04, 0.60, ba_r);
        if (ba_body > 0.0) {
            oval_mask = max(oval_mask, ba_body * 0.95);
        }

        if (secondary_count >= 2.0) {
            for (var oi = 1; oi <= 2; oi = oi + 1) {
                let off_lon = spot_lon - f32(oi) * 1.95;
                var o_dlon = lon - off_lon;
                o_dlon = o_dlon - 6.2831853 * round(o_dlon / 6.2831853);
                let o_u = o_dlon / (0.55 * spot_size);
                let o_v = (lat - (spot_lat - 0.24)) / (0.35 * spot_size);
                let o_r = sqrt(o_u * o_u + o_v * o_v);
                let o_body = smoothstep(1.04, 0.55, o_r);
                oval_mask = max(oval_mask, o_body * 0.85);
            }
        }
    }

    res.spot_mask = body;
    res.collar_mask = collar;
    res.eye_mask = eye;
    res.wake_mask = wake;
    res.oval_mask = oval_mask;
    res.color = grs_col;
    res.is_active = (body > 0.001 || wake > 0.001 || oval_mask > 0.001);
    return res;
}
