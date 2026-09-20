#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
}

struct CometTailUniforms {
    // x: elapsed time (s), y: tail length (AU), z: coma radius (AU), w: activity scale
    params: vec4<f32>,
    // xyz: unit anti-solar vector, w: in-plane lag angle (rad)
    anti_solar_and_lag: vec4<f32>,
    // xyz: comet nucleus position in world space, w: tail mode (0.0 = tail, 1.0 = coma)
    nucleus_pos_and_type: vec4<f32>,
    // xyz: unit orbital velocity in-plane lag vector, w: dust_ratio (0.0 = pure ion, 1.0 = pure dust)
    velocity_and_activity: vec4<f32>,
    // RGBA: Type I Ion tail color (electric cyan / fluorescent blue)
    ion_color: vec4<f32>,
    // RGBA: Type II Dust tail color (warm golden-amber)
    dust_color: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> comet: CometTailUniforms;

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let nuc_pos = comet.nucleus_pos_and_type.xyz;
    let mode = comet.nucleus_pos_and_type.w;
    let d_pos = in.world_position.xyz - nuc_pos;
    let dist_nuc = length(d_pos);

    let anti_solar = normalize(comet.anti_solar_and_lag.xyz);
    let lag_dir = normalize(comet.velocity_and_activity.xyz);
    let tail_len = max(comet.params.y, 0.08);
    let coma_r = max(comet.params.z, 0.002);
    let elapsed = comet.params.x;
    let activity = clamp(comet.params.w, 0.1, 5.0);
    let dust_ratio = clamp(comet.velocity_and_activity.w, 0.05, 0.95);

    // MODE 1.0: DIFFUSE COMA ENVELOPE (Spherical Sublimation Halo around Nucleus)
    if (mode > 0.5) {
        let rho_coma = dist_nuc / max(coma_r, 0.0001);
        if (rho_coma > 1.0) {
            discard;
        }

        // Concentric sublimation core glow vs outer gaseous halo
        let core_mask = exp(-rho_coma * rho_coma * 6.5);
        let halo_mask = pow(max(0.0, 1.0 - rho_coma), 2.2);

        // Sunward Whipple-Eddington fountain compression
        let dir_norm = normalize(d_pos);
        let sunward_dot = dot(dir_norm, -anti_solar);
        let bow_compression = 1.0 + 0.35 * max(0.0, sunward_dot);

        // C2 Swan band emerald fluorescence mixed with dust-scattered sunlight
        let swan_emerald = vec3<f32>(0.20, 0.98, 0.72);
        let dust_coma = comet.dust_color.rgb * 1.1;
        let outer_coma_rgb = mix(swan_emerald, dust_coma, dust_ratio);
        let coma_rgb = mix(outer_coma_rgb, vec3<f32>(0.96, 1.0, 0.98), core_mask);

        let intensity = (core_mask * 2.5 + halo_mask * 0.85) * bow_compression * activity;
        let alpha = clamp((core_mask * 0.92 + halo_mask * 0.40) * bow_compression, 0.0, 0.95);

        if (alpha < 0.005) {
            discard;
        }

        out.color = vec4<f32>(coma_rgb * intensity, alpha);
        return out;
    }

    // MODE 0.0: DUAL VOLUMETRIC TAIL (Type I Ion Ribbon + Type II Curved Dust Fan)
    let z = dot(d_pos, anti_solar);
    if (z < 0.0 || z > tail_len) {
        discard;
    }

    let tau = clamp(z / tail_len, 0.0, 1.0);

    // Lateral position perpendicular to anti-solar axis
    let r_perp_vec = d_pos - z * anti_solar;
    let r_perp = length(r_perp_vec);

    // Expected local envelope radius from flared cone geometry
    let envelope_r = tail_len * (0.12 + 0.20 * dust_ratio) * pow(max(tau, 0.001), 0.75) + coma_r * 0.85 * (1.0 - tau);
    let rho = clamp(r_perp / max(envelope_r, 0.0001), 0.0, 1.0);

    // Volumetric path-length cross section & transverse silhouette edge fade (zero opacity at boundary)
    let volume_thickness = sqrt(max(0.0, 1.0 - rho * rho));
    let edge_fade = smoothstep(1.0, 0.35, rho) * volume_thickness;

    // Longitudinal base fade (starts inside coma) & tip fade (dissolves into space)
    let base_fade = smoothstep(0.0, 0.08, tau);
    let tip_fade = pow(1.0 - tau, 1.3);

    // A. TYPE I ION TAIL (Straight, narrow plasma ribbon along anti-solar line)
    let ion_width = envelope_r * 0.30;
    let rho_ion = r_perp / max(ion_width, 0.0001);
    let ion_radial = exp(-rho_ion * rho_ion * 3.8);

    // Braided helical Alfvén wave streamers
    let v_norm = lag_dir;
    let w_norm = normalize(cross(anti_solar, v_norm));
    let phi = atan2(dot(r_perp_vec, w_norm), dot(r_perp_vec, v_norm));
    let wave_phase = 6.0 * phi - z * 18.0 + elapsed * 5.5;
    let streamer_mod = 0.75 + 0.25 * pow(cos(wave_phase), 2.0);

    // High-density ion knots travelling down the plasma column
    let knot_phase = (tau - elapsed * 0.35) * 4.0;
    let knot_pulse = pow(max(0.0, cos(TWO_PI * knot_phase)), 8.0) * 1.5;

    let ion_intensity = ion_radial * pow(1.0 - tau, 1.25) * streamer_mod * (1.0 + knot_pulse) * (1.0 - dust_ratio);

    // B. TYPE II DUST TAIL (Curved Keplerian fan lagging in the orbital plane)
    let lag_displacement = lag_dir * (z * 0.28 * pow(tau, 0.65));
    let d_dust_vec = r_perp_vec - lag_displacement;
    let r_dust = length(d_dust_vec);
    let dust_w = envelope_r * 0.85;
    let rho_dust = r_dust / max(dust_w, 0.0001);
    let dust_radial = smoothstep(1.15, 0.0, rho_dust);

    // Subtle syndyne dust release striations
    let syndyne_ripple = 0.88 + 0.12 * cos(z * 32.0 - tau * 8.0);

    // Forward sunlight scattering boost (Mie scattering on micron-sized dust grains)
    let cam_pos = view.world_position;
    let view_dir = normalize(cam_pos - in.world_position.xyz);
    let cos_phase = dot(view_dir, -anti_solar);
    let mie_boost = 1.0 + 0.45 * pow(max(0.0, cos_phase), 3.0);

    let dust_intensity = dust_radial * pow(1.0 - tau, 1.1) * syndyne_ripple * mie_boost * dust_ratio;

    // C. COMBINED LUMINANCE & ALPHA
    let ion_rgb = comet.ion_color.rgb * (ion_intensity * 2.8 * activity);
    let dust_rgb = comet.dust_color.rgb * (dust_intensity * 1.8 * activity);
    let final_rgb = ion_rgb + dust_rgb;

    let final_alpha = clamp((ion_intensity * 0.85 + dust_intensity * 0.55) * edge_fade * base_fade * tip_fade * activity, 0.0, 0.95);

    if (final_alpha < 0.005) {
        discard;
    }

    out.color = vec4<f32>(final_rgb, final_alpha);
    return out;
}
