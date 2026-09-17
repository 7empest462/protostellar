#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
}

struct RelativisticJetUniforms {
    jet_params: vec4<f32>,              // x: elapsed_s, y: lorentz_factor, z: opening_angle_rad, w: jet_length_au
    jet_dir_and_precession: vec4<f32>,  // xyz: unit jet pointing direction vector, w: precession_angle_rad
    synchrotron_params: vec4<f32>,      // x: spectral_index_p, y: knot_speed_c, z: knot_frequency, w: helical_pitch
    core_color: vec4<f32>,              // Core RGBA
    lobe_color: vec4<f32>,              // Outer lobe RGBA
    jet_origin_and_doppler: vec4<f32>,  // xyz: jet origin in world space, w: doppler_enabled (1.0 = on)
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> jet: RelativisticJetUniforms;

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let origin = jet.jet_origin_and_doppler.xyz;
    let jet_dir = normalize(jet.jet_dir_and_precession.xyz);
    let jet_len = max(jet.jet_params.w, 0.01);
    let elapsed = jet.jet_params.x;
    let gamma = max(jet.jet_params.y, 1.0001);
    let opening_angle = max(jet.jet_params.z, 0.005);

    // 1. Vector from jet base to current surface point
    let d_pos = in.world_position.xyz - origin;
    let z = dot(d_pos, jet_dir);

    // Fade or discard fragments outside jet axial bounds
    if (z <= 0.0 || z > jet_len) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let t = clamp(z / jet_len, 0.0, 1.0);

    // 2. Radial distance from central jet spine
    let r_vec = d_pos - z * jet_dir;
    let r = length(r_vec);

    // Conical-parabolic envelope radius at distance z
    let base_r = 0.0008;
    let envelope_r = base_r + z * tan(opening_angle) * (1.0 + 0.30 * t * t);
    let rho = r / max(envelope_r, 0.0001);

    if (rho > 1.0) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    // 3. Azimuthal angle around jet axis for helical field lines
    var ref_axis = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(dot(jet_dir, ref_axis)) > 0.9) {
        ref_axis = vec3<f32>(1.0, 0.0, 0.0);
    }
    let u_axis = normalize(cross(jet_dir, ref_axis));
    let v_axis = cross(jet_dir, u_axis);
    let phi = atan2(dot(r_vec, v_axis), dot(r_vec, u_axis));

    // 4. Relativistic Doppler Beaming Amplification
    var doppler_boost = 1.0;
    if (jet.jet_origin_and_doppler.w > 0.5) {
        let cam_pos = view.world_position;
        let view_dir = normalize(cam_pos - in.world_position.xyz);
        let cos_theta = clamp(dot(jet_dir, view_dir), -1.0, 1.0);
        let beta = sqrt(max(0.0, 1.0 - 1.0 / (gamma * gamma)));
        let doppler = 1.0 / max(gamma * (1.0 - beta * cos_theta), 1e-4);
        let alpha = (jet.synchrotron_params.x - 1.0) * 0.5;
        let raw_boost = pow(clamp(doppler, 0.12, 6.0), 3.0 + alpha);
        doppler_boost = clamp(raw_boost, 0.08, 12.0);
    }

    // 5. Helical Braided Magnetic Streamlines
    let pitch = jet.synchrotron_params.w;
    let helical_pattern = cos(2.0 * phi - pitch * t * TWO_PI + elapsed * 6.0);
    let helical_intensity = helical_pattern * helical_pattern * 0.45 + 0.55;

    // 6. Periodic Internal Shock Knots (Mach Disks propagating down jet column)
    let knot_speed = jet.synchrotron_params.y;
    let knot_freq = jet.synchrotron_params.z;
    let knot_phase = knot_freq * (t - knot_speed * elapsed * 0.25);
    let knot_wave = max(0.0, cos(TWO_PI * knot_phase));
    let knot_intensity = pow(knot_wave, 12.0) * 2.2;

    // 7. Core Spine vs Turbulent Outer Sheath
    let spine_mask = 1.0 - smoothstep(0.08, 0.35, rho);
    let core_weight = clamp(spine_mask * 0.85 + knot_intensity * 0.35, 0.0, 1.0);
    let base_rgb = mix(jet.lobe_color.rgb, jet.core_color.rgb, core_weight);

    // 8. Total Radiance with Doppler Flashing & Synchrotron Incandescence
    let emission_boost = (spine_mask * 3.5 + 1.2) * helical_intensity * (1.0 + knot_intensity) * doppler_boost;
    let final_rgb = base_rgb * emission_boost;

    // 9. Volumetric Radial and Axial Opacity Attenuation
    let radial_fade = (1.0 - rho * rho) * (1.0 - rho * rho);
    let axial_fade = pow(t, 0.45) * pow(1.0 - t, 1.25) * 2.5;
    let alpha = clamp(radial_fade * axial_fade * (spine_mask * 0.75 + 0.25) * pow(doppler_boost, 0.35), 0.0, 1.0);

    out.color = vec4<f32>(final_rgb, alpha);
    return out;
}
