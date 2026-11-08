import bevy_pbr::render::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
};

import protostellar::rendering::shaders::scattering::{chapman_spherical_airmass, dual_lobe_mie_phase};

struct AtmosphereUniforms {
    rayleigh_params: vec4<f32>, // rgb = beta_R (or coronal RGB in Corona Mode), w = H_R (scale height)
    mie_params: vec4<f32>,      // rgb = beta_M (or flare/chromosphere RGB), w = H_M (scale height)
    optical_params: vec4<f32>,  // x = g (mie asymmetry), y = surface_pressure_bar (< -0.5 = Stellar Corona Mode), z = inner radius, w = outer radius
    star_dir_and_intensity: vec4<f32>, // xyz = unit star dir in world space, w = star intensity factor
    planet_center: vec4<f32>,   // xyz = planet world pos, w = outer shell scale factor
    aurora_params: vec4<f32>,   // x = oval_colatitude_rad (or flare_int), y = oval_width_rad, z = auroral_intensity, w = kp_index (or time in Corona Mode)
    spin_axis: vec4<f32>,       // xyz = unit 3D spin axis in world coordinates, w = rotational oblateness (0.0..0.25)
    ring_and_ozone_params: vec4<f32>, // x = inner_ring_ratio, y = outer_ring_ratio, z = ring_opt_depth, w = flags (1 = ozone, 2 = rings)
    tidal_deformation: vec4<f32>,     // xyz = unit world vector toward tidal perturber, w = roche_stretch (0.0..0.85)
    cloud_and_time_params: vec4<f32>, // x = cloud_coverage (0..1), y = visual_time_secs, z = spin_angle, w = color_seed
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> atmo: AtmosphereUniforms;

const PI: f32 = 3.14159265359;

/// Exact analytical ray-sphere intersection. Returns vec2<f32>(t_near, t_far).
/// If the ray misses the sphere, returns vec2<f32>(-1.0, -1.0).
fn ray_sphere_intersect(ro: vec3<f32>, rd: vec3<f32>, center: vec3<f32>, radius: f32) -> vec2<f32> {
    let oc = ro - center;
    let b = dot(oc, rd);
    let c = dot(oc, oc) - radius * radius;
    let disc = b * b - c;
    if (disc < 0.0) {
        return vec2<f32>(-1.0, -1.0);
    }
    let s = sqrt(disc);
    return vec2<f32>(-b - s, -b + s);
}



/// Evaluates directional radius multiplier for Maclaurin rotational oblateness and Roche Lobe teardrop deformation.
fn directional_shape_scale(unit_dir: vec3<f32>) -> f32 {
    var scale = 1.0;
    let oblate = clamp(atmo.spin_axis.w, 0.0, 0.25);
    if (oblate > 0.001) {
        var s_axis = atmo.spin_axis.xyz;
        if (length(s_axis) > 0.1) {
            s_axis = normalize(s_axis);
            let sin_lat = dot(unit_dir, s_axis);
            scale += -oblate * (1.5 * sin_lat * sin_lat - 0.5);
        }
    }
    let roche = clamp(atmo.tidal_deformation.w, 0.0, 0.85);
    if (roche > 0.001 && dot(atmo.tidal_deformation.xyz, atmo.tidal_deformation.xyz) > 0.2) {
        let t_dir = normalize(atmo.tidal_deformation.xyz);
        let cos_t = dot(unit_dir, t_dir);
        let sin2_t = max(1.0 - cos_t * cos_t, 0.0);
        let a_stretch = 1.0 + roche;
        let b_compress = 1.0 / sqrt(a_stretch);
        let inv_r2 = (cos_t * cos_t) / (a_stretch * a_stretch) + sin2_t / (b_compress * b_compress);
        let r_ellipsoid = 1.0 / sqrt(max(inv_r2, 0.001));
        let l1_cone = max(cos_t, 0.0);
        let l1_nozzle = roche * 0.35 * l1_cone * l1_cone * l1_cone;
        scale *= (r_ellipsoid + l1_nozzle);
    }
    return max(scale, 0.55);
}

/// Evaluates planetary ring transmittance along a ray from an atmospheric sample point toward the star.
fn evaluate_ring_shadow_transmittance(
    p_rel_norm: vec3<f32>,
    star_dir: vec3<f32>,
    s_axis: vec3<f32>,
) -> f32 {
    let flags = u32(round(max(atmo.ring_and_ozone_params.w, 0.0)));
    if ((flags & 2u) == 0u) {
        return 1.0;
    }
    let r_inner = atmo.ring_and_ozone_params.x;
    let r_outer = atmo.ring_and_ozone_params.y;
    let opt_depth = atmo.ring_and_ozone_params.z;

    let sample_len = max(length(p_rel_norm), 0.001);
    let n_sample = p_rel_norm / sample_len;
    let mu_s = dot(n_sample, star_dir);
    if (mu_s < -0.35) {
        return 1.0;
    }
    let p_sun_facing = select(
        normalize(n_sample - mu_s * star_dir) * sample_len,
        p_rel_norm,
        mu_s >= 0.0
    );

    let l_dot_s = dot(star_dir, s_axis);
    let p_dot_s = dot(p_sun_facing, s_axis);
    if (abs(l_dot_s) <= 1e-4) {
        return 1.0;
    }
    let t_ring = -p_dot_s / l_dot_s;
    if (t_ring <= 0.0) {
        return 1.0;
    }
    let p_int = p_sun_facing + t_ring * star_dir;
    let r_int = length(p_int);
    if (r_int < r_inner || r_int > r_outer) {
        return 1.0;
    }
    let u = clamp((r_int - r_inner) / max(r_outer - r_inner, 0.001), 0.0, 1.0);
    let c_ring = mix(0.10, 0.34, clamp(u / 0.22, 0.0, 1.0)) * smoothstep(0.0, 0.035, u) * (1.0 - smoothstep(0.195, 0.245, u));
    let b_ring = mix(0.78, 0.96, smoothstep(0.22, 0.38, u)) * smoothstep(0.195, 0.245, u) * (1.0 - smoothstep(0.625, 0.665, u));
    let cassini_floor = 0.08 * smoothstep(0.625, 0.665, u) * (1.0 - smoothstep(0.705, 0.745, u));
    let encke_dip = 1.0 - 0.78 * exp(-pow((u - 0.865) / 0.011, 2.0));
    let a_ring = 0.74 * encke_dip * smoothstep(0.705, 0.745, u) * (1.0 - smoothstep(0.935, 0.975, u));
    let f_ring = 0.28 * smoothstep(0.935, 0.968, u) * (1.0 - smoothstep(0.968, 1.0, u));
    let ring_density = c_ring + b_ring + cassini_floor + a_ring + f_ring;

    let edge_feather = smoothstep(r_inner, r_inner + 0.035, r_int) * (1.0 - smoothstep(r_outer - 0.035, r_outer, r_int));
    let shadow_atten = clamp(ring_density * opt_depth * edge_feather, 0.0, 0.94);
    return 1.0 - shadow_atten;
}

/// Renders 3D Volumetric Stellar Corona, Magnetic Helmet Streamers, and Telescope Optical Diffraction Spikes.
fn render_stellar_corona_and_diffraction(
    ray_origin: vec3<f32>,
    ray_dir: vec3<f32>,
    center: vec3<f32>,
    r_star_base: f32,
    r_outer_base: f32,
) -> vec4<f32> {
    let oc = ray_origin - center;
    let t_closest = max(-dot(oc, ray_dir), 0.0);
    let closest_pos = ray_origin + ray_dir * t_closest;
    let p_rel = closest_pos - center;
    let b_dist = length(p_rel);
    let unit_dir = p_rel / max(b_dist, 1e-5);

    let shape_scale = directional_shape_scale(unit_dir);
    let r_star = r_star_base * shape_scale;
    let r_outer = r_outer_base * shape_scale;

    if (b_dist >= r_outer) {
        return vec4<f32>(0.0);
    }

    // Check if the ray intersects the opaque stellar photosphere
    let hit_star = ray_sphere_intersect(ray_origin, ray_dir, center, r_star);
    let hits_disk = hit_star.x > 0.0 && b_dist < r_star * 0.985;

    let corona_thickness = max(r_outer - r_star, 0.0001);
    let rho = clamp((b_dist - r_star) / corona_thickness, 0.0, 1.0);
    let edge_fade = 1.0 - smoothstep(0.68, 1.0, rho);

    var s_axis = atmo.spin_axis.xyz;
    if (length(s_axis) < 0.1) {
        s_axis = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        s_axis = normalize(s_axis);
    }

    let time = atmo.aurora_params.w;
    let flare_boost = clamp(atmo.aurora_params.x, 0.0, 2.5);
    let sin_lat = dot(unit_dir, s_axis);
    let eq_belt = 1.0 - sin_lat * sin_lat;
    let az = atan2(unit_dir.z, unit_dir.x);

    // 1. K-Corona (Thomson electron scattering) + F-Corona (inner zodiacal dust glow)
    let k_corona = exp(-4.4 * rho);
    let f_corona = 0.34 * exp(-1.75 * rho);

    // 2. Magnetic Helmet Streamers & Solar Wind Plumes
    let streamer_wave = cos(sin_lat * 5.5 + sin(az * 3.0 + time * 0.35) * 0.8) * 0.5 + 0.5;
    let fine_plumes = sin(az * 14.0 + sin_lat * 8.0 - time * 0.55) * 0.5 + 0.5;
    let helmet_cusp = pow(eq_belt, 1.3) * (0.65 + streamer_wave * 0.55) * exp(-2.4 * rho);
    let polar_plumes = pow(abs(sin_lat), 1.8) * (0.5 + fine_plumes * 0.5) * exp(-3.2 * rho) * 0.45;
    let streamer_mod = 1.0 + (helmet_cusp + polar_plumes) * (0.75 + flare_boost * 0.45);

    // 3. Telescope Optical Aperture Diffraction Spikes (JWST 6-blade / Hubble 4-blade starburst)
    let cam_up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(ray_dir.y) > 0.95);
    let screen_u = normalize(cross(ray_dir, cam_up));
    let screen_v = cross(screen_u, ray_dir);
    let sx = dot(p_rel, screen_u) / corona_thickness;
    let sy = dot(p_rel, screen_v) / corona_thickness;
    let r_screen = sqrt(sx * sx + sy * sy);

    // Primary orthogonal cross spikes + subtle 60-degree diagonal secondary spikes
    let spike_w = 0.016 + r_screen * 0.024;
    let spike_h = exp(-pow(sy / spike_w, 2.0)) * exp(-2.3 * r_screen);
    let spike_v = exp(-pow(sx / spike_w, 2.0)) * exp(-2.3 * r_screen);
    let d1 = sx * 0.866025 + sy * 0.5;
    let d2 = sx * 0.866025 - sy * 0.5;
    let spike_diag = (exp(-pow(d1 / (spike_w * 1.35), 2.0)) + exp(-pow(d2 / (spike_w * 1.35), 2.0))) * exp(-3.1 * r_screen) * 0.42;
    let total_spikes = (spike_h + spike_v + spike_diag) * edge_fade;

    let corona_rgb = atmo.rayleigh_params.rgb;
    let chromo_rgb = atmo.mie_params.rgb;
    // Subtle spectral dispersion along diffraction spikes
    let dispersion_tint = mix(vec3<f32>(0.85, 0.94, 1.15), vec3<f32>(1.15, 0.88, 0.72), clamp(r_screen * 1.4, 0.0, 1.0));

    let base_glow = (corona_rgb * (k_corona * streamer_mod + f_corona) + chromo_rgb * exp(-8.5 * rho) * (0.45 + flare_boost * 0.35)) * edge_fade;
    let spike_glow = corona_rgb * dispersion_tint * total_spikes * (1.15 + flare_boost * 0.25);

    let total_rgb = (base_glow + spike_glow) * clamp(atmo.star_dir_and_intensity.w, 0.6, 3.5);
    let raw_alpha = clamp((k_corona * 0.78 + f_corona * 0.45 + total_spikes * 0.65) * edge_fade, 0.0, 0.92);
    // Keep foreground disk alpha subtle so convective photosphere granules remain crisp
    let final_alpha = select(raw_alpha, raw_alpha * 0.16, hits_disk);

    return vec4<f32>(total_rgb, final_alpha);
}

fn atmo_hash3(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn atmo_noise3(x: vec3<f32>) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(
            mix(atmo_hash3(i + vec3<f32>(0.0, 0.0, 0.0)), atmo_hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(atmo_hash3(i + vec3<f32>(0.0, 1.0, 0.0)), atmo_hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(atmo_hash3(i + vec3<f32>(0.0, 0.0, 1.0)), atmo_hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(atmo_hash3(i + vec3<f32>(0.0, 1.0, 1.0)), atmo_hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

fn atmo_fbm(p_in: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var p = p_in;
    for (var i = 0; i < 4; i = i + 1) {
        v += a * atmo_noise3(p);
        p = p * 2.03 + vec3<f32>(1.7, 9.2, 5.3);
        a *= 0.5;
    }
    return v;
}

fn rotate_y_atmo(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x * c - v.z * s, v.y, v.x * s + v.z * c);
}

/// Evaluates 3D floating cloud shells inside the visible atmospheric shell & limb in orbital view.
fn evaluate_orbital_limb_clouds(
    ray_origin: vec3<f32>,
    ray_dir: vec3<f32>,
    center: vec3<f32>,
    r_ground_eff: f32,
    atmo_thickness: f32,
    hit_ground: bool,
    hit_planet_x: f32,
    star_dir: vec3<f32>,
    s_axis: vec3<f32>,
    sky_scatter_rgb: vec3<f32>,
) -> vec4<f32> {
    let cloud_cov = clamp(atmo.cloud_and_time_params.x, 0.0, 1.0);
    let is_giant_atmo = (atmo.optical_params.y > 20.0 || atmo.spin_axis.w > 0.02);
    if (cloud_cov <= 0.02 || is_giant_atmo) {
        return vec4<f32>(0.0);
    }

    let sim_t = atmo.cloud_and_time_params.y;
    let spin_angle = atmo.cloud_and_time_params.z;
    let seed = atmo.cloud_and_time_params.w;

    let r_c_mid = r_ground_eff + atmo_thickness * 0.36;
    let r_c_high = r_ground_eff + atmo_thickness * 0.56;

    let hit_cm = ray_sphere_intersect(ray_origin, ray_dir, center, r_c_mid);
    if (hit_cm.x <= 0.0 || (hit_ground && hit_cm.x >= hit_planet_x)) {
        return vec4<f32>(0.0);
    }

    let hit_ch = ray_sphere_intersect(ray_origin, ray_dir, center, r_c_high);
    let t_ch = select(hit_cm.x, hit_ch.x, hit_ch.x > 0.0);

    let p_cm_w = normalize((ray_origin + ray_dir * hit_cm.x) - center);
    let p_ch_w = normalize((ray_origin + ray_dir * t_ch) - center);

    // Grazing cosine on the 3D cloud shell: strongest in the elevated atmospheric limb & outer disk
    let mu_v_shell = abs(dot(p_cm_w, ray_dir));
    let shell_edge_feather = smoothstep(0.05, 0.24, mu_v_shell);
    let mu_ground = select(0.0, abs(dot(normalize((ray_origin + ray_dir * hit_planet_x) - center), ray_dir)), hit_ground);
    // Fade in smoothly toward the outer disk and across the 3D atmospheric limb above the horizon
    let limb_presence = smoothstep(0.68, 0.10, mu_ground) * shell_edge_feather;
    if (limb_presence <= 0.005) {
        return vec4<f32>(0.0);
    }

    let s_ref = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(s_axis.z) > 0.95);
    let s_tan_x = normalize(cross(s_axis, s_ref));
    let s_tan_z = cross(s_tan_x, s_axis);

    let p_cm_tilt = vec3<f32>(dot(p_cm_w, s_tan_x), dot(p_cm_w, s_axis), dot(p_cm_w, s_tan_z));
    let p_ch_tilt = vec3<f32>(dot(p_ch_w, s_tan_x), dot(p_ch_w, s_axis), dot(p_ch_w, s_tan_z));

    let zonal = sin(p_cm_tilt.y * 6.283185) * 0.15;
    let p_mid_rot = rotate_y_atmo(p_cm_tilt, spin_angle + sim_t * 0.062 + zonal);
    let p_high_rot = rotate_y_atmo(p_ch_tilt, spin_angle - sim_t * 0.048 + sin(p_ch_tilt.y * 11.0) * 0.22);

    let seed_ofs = vec3<f32>(seed * 1.3, -seed * 0.7, seed * 2.1);
    let c_mid = atmo_fbm(p_mid_rot * 5.2 + seed_ofs + vec3<f32>(sim_t * 0.024, -sim_t * 0.018, sim_t * 0.020));
    let c_eddy = atmo_fbm(p_mid_rot * 11.0 - seed_ofs - vec3<f32>(sim_t * 0.042, 0.0, sim_t * 0.042));
    let c_cirrus = atmo_fbm(vec3<f32>(p_high_rot.x * 6.5, p_high_rot.y * 16.0, p_high_rot.z * 6.5) + vec3<f32>(sim_t * 0.034, 0.0, -sim_t * 0.030));

    let base_thresh = 0.48 - cloud_cov * 0.22;
    let dens_scale = clamp(cloud_cov * 1.35, 0.25, 1.0);
    let alpha_m = clamp(((c_mid * 0.65 + c_eddy * 0.35) - base_thresh) * 2.8 * dens_scale, 0.0, 0.82);
    let alpha_h = clamp((c_cirrus - (base_thresh + 0.04)) * 2.0 * dens_scale, 0.0, 0.48);
    let cloud_alpha = (1.0 - (1.0 - alpha_m) * (1.0 - alpha_h)) * limb_presence * 0.78;
    if (cloud_alpha <= 0.005) {
        return vec4<f32>(0.0);
    }

    let mu_s = dot(p_cm_w, star_dir);
    let day_light = smoothstep(-0.16, 0.24, mu_s);
    let twilight = smoothstep(-0.18, 0.04, mu_s) * (1.0 - smoothstep(0.04, 0.32, mu_s));
    let cos_theta = dot(ray_dir, star_dir);
    let silver = pow(clamp(cos_theta, 0.0, 1.0), 6.0) * (1.0 - cloud_alpha * 0.5) * day_light * 0.42;

    let cloud_top = vec3<f32>(0.98, 0.99, 1.0);
    let cloud_ambient = mix(vec3<f32>(0.68, 0.76, 0.90), sky_scatter_rgb + vec3<f32>(0.55, 0.62, 0.75), 0.45);
    var c_rgb = mix(cloud_ambient, cloud_top, 0.75) * day_light + vec3<f32>(1.0, 0.95, 0.86) * silver;
    c_rgb += vec3<f32>(1.06, 0.56, 0.32) * twilight * 0.65;
    c_rgb += vec3<f32>(0.02, 0.03, 0.05) * (1.0 - day_light);

    return vec4<f32>(c_rgb, cloud_alpha);
}

/// Evaluates 3D overhead dynamic cumulus & cirrus cloud layers in the sky dome when viewing from the surface.
fn evaluate_surface_sky_clouds(
    ray_origin: vec3<f32>,
    ray_dir: vec3<f32>,
    center: vec3<f32>,
    cam_dist: f32,
    r_planet_def: f32,
    view_elev_sin: f32,
    star_dir: vec3<f32>,
    s_axis: vec3<f32>,
    sky_scatter_rgb: vec3<f32>,
) -> vec4<f32> {
    let cloud_cov = clamp(atmo.cloud_and_time_params.x, 0.0, 1.0);
    if (cloud_cov <= 0.02 || view_elev_sin <= -0.05) {
        return vec4<f32>(0.0);
    }

    let sim_t = atmo.cloud_and_time_params.y;
    let spin_angle = atmo.cloud_and_time_params.z;
    let seed = atmo.cloud_and_time_params.w;

    let s_ref = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(s_axis.z) > 0.95);
    let s_tan_x = normalize(cross(s_axis, s_ref));
    let s_tan_z = cross(s_tan_x, s_axis);

    let r_cloud_base = max(r_planet_def * 1.022, cam_dist + r_planet_def * 0.024);
    let r_cloud_high = r_cloud_base + r_planet_def * 0.014;

    let hit_base = ray_sphere_intersect(ray_origin, ray_dir, center, r_cloud_base);
    let hit_high = ray_sphere_intersect(ray_origin, ray_dir, center, r_cloud_high);
    if (hit_base.y <= 0.0) {
        return vec4<f32>(0.0);
    }

    let p_base_w = normalize((ray_origin + ray_dir * hit_base.y) - center);
    let p_high_w = normalize((ray_origin + ray_dir * max(hit_high.y, hit_base.y)) - center);

    let p_base_tilt = vec3<f32>(dot(p_base_w, s_tan_x), dot(p_base_w, s_axis), dot(p_base_w, s_tan_z));
    let p_high_tilt = vec3<f32>(dot(p_high_w, s_tan_x), dot(p_high_w, s_axis), dot(p_high_w, s_tan_z));

    let lat_b = p_base_tilt.y;
    // Bounded spatial wave (never multiplied by sim_t so clouds never shear into horizontal stripes)
    let zonal = sin(lat_b * 6.283185) * 0.12;
    let p_cumulus = rotate_y_atmo(p_base_tilt, spin_angle + sim_t * 0.062 + zonal);
    let p_cirrus = rotate_y_atmo(p_high_tilt, spin_angle - sim_t * 0.048 + zonal * 1.3);

    let seed_ofs = vec3<f32>(seed * 3.1, seed * 5.7, seed * 2.3);
    // Combine planetary synoptic scale (8x) with local horizon sky-dome mesoscale (28x and 64x) that evolve over time
    let synoptic = atmo_fbm(p_cumulus * 8.5 + seed_ofs + vec3<f32>(sim_t * 0.028, -sim_t * 0.022, sim_t * 0.025));
    let meso = atmo_fbm(p_cumulus * 28.0 + seed_ofs + vec3<f32>(sim_t * 0.045, sim_t * 0.032, -sim_t * 0.038));
    let detail = atmo_fbm(p_cumulus * 64.0 - seed_ofs + vec3<f32>(-sim_t * 0.065, sim_t * 0.048, sim_t * 0.055));
    let cumulus_field = synoptic * 0.42 + meso * 0.40 + detail * 0.18;

    let thresh = mix(0.66, 0.28, pow(cloud_cov, 0.75));
    let cumulus_density = smoothstep(thresh, thresh + 0.24, cumulus_field);

    let cirrus_field = atmo_fbm(vec3<f32>(p_cirrus.x * 22.0, p_cirrus.y * 48.0, p_cirrus.z * 22.0) + seed_ofs * 1.7 + vec3<f32>(sim_t * 0.035, 0.0, -sim_t * 0.030));
    let cirrus_density = smoothstep(0.60, 0.84, cirrus_field) * clamp(cloud_cov * 1.2, 0.15, 0.75);

    let combined_cloud = clamp(cumulus_density * 0.88 + cirrus_density * 0.42, 0.0, 1.0);
    let horizon_fade = smoothstep(-0.03, 0.14, view_elev_sin);
    let cloud_alpha = combined_cloud * horizon_fade;
    if (cloud_alpha <= 0.005) {
        return vec4<f32>(0.0);
    }

    // 3D solar self-shadowing sample offset toward star_dir
    let star_tilt = vec3<f32>(dot(star_dir, s_tan_x), dot(star_dir, s_axis), dot(star_dir, s_tan_z));
    let p_sun_sample = rotate_y_atmo(normalize(p_base_tilt + star_tilt * 0.012), spin_angle + sim_t * 0.062 + zonal);
    let sun_occ = atmo_fbm(p_sun_sample * 28.0 + seed_ofs + vec3<f32>(sim_t * 0.045, sim_t * 0.032, -sim_t * 0.038));
    let self_shadow = clamp(1.0 - smoothstep(thresh - 0.04, thresh + 0.20, sun_occ) * 0.65, 0.28, 1.0);

    let mu_s = dot(p_base_w, star_dir);
    let day_light = smoothstep(-0.14, 0.22, mu_s);
    let twilight = smoothstep(-0.16, 0.04, mu_s) * (1.0 - smoothstep(0.04, 0.32, mu_s));
    let cos_theta = dot(ray_dir, star_dir);
    let silver_lining = pow(clamp(cos_theta, 0.0, 1.0), 6.0) * (1.0 - cumulus_density * 0.65) * 0.65;

    let sunlit_top = vec3<f32>(0.98, 0.98, 1.0);
    let cloud_base_ambient = mix(vec3<f32>(0.42, 0.48, 0.58), sky_scatter_rgb * 0.85 + vec3<f32>(0.32, 0.36, 0.44), 0.55);
    let sunset_tint = vec3<f32>(1.05, 0.52, 0.24);

    var cloud_rgb = mix(cloud_base_ambient, sunlit_top, self_shadow * 0.78 + silver_lining) * day_light;
    cloud_rgb += sunset_tint * twilight * (0.45 + self_shadow * 0.55);
    // Subtle nocturnal ambient so clouds remain faintly discernible against night airglow
    cloud_rgb += vec3<f32>(0.025, 0.032, 0.048) * (1.0 - day_light);
    // Blend distant low-elevation clouds into atmospheric horizon haze
    let aerial_haze = 1.0 - smoothstep(0.04, 0.36, view_elev_sin);
    cloud_rgb = mix(cloud_rgb, sky_scatter_rgb + vec3<f32>(0.10, 0.12, 0.15) * day_light, aerial_haze * 0.52);

    return vec4<f32>(cloud_rgb, clamp(cloud_alpha * 0.92, 0.0, 0.94));
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    // Camera-relative eye coordinates: transforms ray origin to origin (0,0,0) and
    // evaluates all intersections relative to the camera eye. This maintains full 24-bit
    // float precision at any distance (e.g. Planet 9 at 361 AU), eliminating polygonal faceting.
    let ray_origin = vec3<f32>(0.0);
    let center = atmo.planet_center.xyz - view.world_position;
    let ray_dir = normalize(in.world_position.xyz - view.world_position);
    let r_planet = atmo.optical_params.z;
    let r_atmo = atmo.optical_params.w;

    // 3D Planetary Internal Cutaway Mode: discard atmosphere in the primary +X / +Z quadrant
    let atmo_flags = u32(round(max(atmo.ring_and_ozone_params.w, 0.0)));
    if ((atmo_flags & 4u) != 0u) {
        var s_axis = atmo.spin_axis.xyz;
        if (length(s_axis) < 0.1) {
            s_axis = vec3<f32>(0.0, 1.0, 0.0);
        } else {
            s_axis = normalize(s_axis);
        }
        let s_ref = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(s_axis.z) > 0.95);
        let s_tangent_x = normalize(cross(s_axis, s_ref));
        let s_tangent_z = cross(s_tangent_x, s_axis);

        let p_rel = normalize((in.world_position.xyz - view.world_position) - center);
        let sin_lat_atmo = dot(p_rel, s_axis);
        let p_tilted_atmo = vec3<f32>(dot(p_rel, s_tangent_x), sin_lat_atmo, dot(p_rel, s_tangent_z));
        if (p_tilted_atmo.x > 0.0 && p_tilted_atmo.z > 0.0) {
            out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
            return out;
        }
    }

    // Stellar Corona, Magnetic Helmet Streamers & Optical Diffraction Spikes Mode
    if (atmo.optical_params.y < -0.5) {
        out.color = render_stellar_corona_and_diffraction(ray_origin, ray_dir, center, r_planet, r_atmo);
        return out;
    }

    let pressure = atmo.optical_params.y;
    if (pressure <= 0.001) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    // Account for maximum tidal/oblate expansion on bounding sphere intersection
    let max_stretch = 1.0 + clamp(atmo.tidal_deformation.w, 0.0, 0.85) * 1.15 + clamp(atmo.spin_axis.w, 0.0, 0.25) * 0.35;
    let hit_atmo = ray_sphere_intersect(ray_origin, ray_dir, center, r_atmo * max_stretch);
    if (hit_atmo.y <= 0.0) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    // Approximate view direction unit vector at closest approach to evaluate deformed radii
    let t_mid_approx = clamp(-dot(ray_origin - center, ray_dir), max(hit_atmo.x, 0.0), hit_atmo.y);
    let dir_approx = normalize((ray_origin + ray_dir * t_mid_approx) - center);
    let local_shape = directional_shape_scale(dir_approx);
    let r_planet_def = r_planet * local_shape;
    let r_atmo_def = r_atmo * local_shape;

    // Detect Surface Horizon Mode / low-altitude camera inside or on mountain peaks of the planet
    let cam_dist = length(center);
    let cam_alt_ratio = cam_dist / max(r_planet_def, 1e-6);
    let is_surface_view = cam_alt_ratio < 1.42;
    // Prevent double-rendering of the outer atmospheric limb when the camera is outside the double-sided mesh
    let frag_rel = (in.world_position.xyz - view.world_position) - center;
    if (!is_surface_view && cam_dist > length(frag_rel) * 1.01 && dot(frag_rel, ray_dir) > 0.0) {
        discard;
    }
    let surface_blend = smoothstep(1.42, 1.15, cam_alt_ratio);
    let cam_up_zenith = -center / max(cam_dist, 1e-6);
    let view_elev_sin = dot(ray_dir, cam_up_zenith);

    // Ensure the effective atmosphere shell always encompasses the camera even on high mountain peaks
    let r_ground_eff = select(r_planet_def, min(r_planet_def, cam_dist * 0.992), is_surface_view);
    let r_atmo_eff = select(r_atmo_def, max(r_atmo_def, cam_dist + r_planet_def * 0.12), is_surface_view);

    let hit_atmo_def = ray_sphere_intersect(ray_origin, ray_dir, center, r_atmo_eff);
    if (hit_atmo_def.y <= 0.0) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let t_start = max(hit_atmo_def.x, 0.0);
    var t_end = hit_atmo_def.y;

    // Ray-sphere intersection with deformed inner solid planet
    let hit_planet = ray_sphere_intersect(ray_origin, ray_dir, center, r_ground_eff);
    var hit_ground = false;
    if (hit_planet.x > 0.0 && (!is_surface_view || view_elev_sin < -0.10)) {
        t_end = min(t_end, hit_planet.x);
        hit_ground = true;
    }

    if (t_end <= t_start) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    // Scale-invariant scattering parameters and scale heights (works at any planet radius in AU)
    let atmo_thickness = max(r_atmo_eff - r_ground_eff, r_planet_def * 0.035);
    let beta_r = atmo.rayleigh_params.rgb;
    let h_r = select(
        max(atmo.rayleigh_params.w * r_planet_def, atmo_thickness * 0.25),
        atmo_thickness * 0.36,
        is_surface_view
    );
    let beta_m = atmo.mie_params.rgb;
    let h_m = select(
        max(atmo.mie_params.w * r_planet_def, atmo_thickness * 0.10),
        atmo_thickness * 0.16,
        is_surface_view
    );
    let g = clamp(atmo.optical_params.x, 0.50, 0.95);
    let star_dir = atmo.star_dir_and_intensity.xyz;
    let star_intensity = atmo.star_dir_and_intensity.w;

    var s_axis = atmo.spin_axis.xyz;
    if (length(s_axis) < 0.1) {
        s_axis = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        s_axis = normalize(s_axis);
    }

    // Chappuis Ozone (O3) stratospheric absorption band for Earth-like / oxygen atmospheres
    let flags = u32(round(max(atmo.ring_and_ozone_params.w, 0.0)));
    let has_ozone = select(0.0, 1.0, (flags & 1u) != 0u);
    let beta_o3 = vec3<f32>(0.65, 1.88, 0.08) * 0.45 * has_ozone;

    // Phase functions
    let cos_theta = dot(ray_dir, star_dir);
    let cos2_theta = cos_theta * cos_theta;
    let p_rayleigh = (3.0 / (16.0 * PI)) * (1.0 + cos2_theta);
    let p_mie = dual_lobe_mie_phase(cos_theta, g);
    let p_iso = 1.0 / (4.0 * PI);

    // Numerical single + multiple scattering raymarching (8 sample steps)
    // Normalize step optical density by planetary radius / atmosphere thickness so optical depth is scale-invariant
    let step_count = 8;
    let step_size = (t_end - t_start) / f32(step_count);
    let pressure_scale = clamp(pow(max(pressure, 0.01), 0.28), 0.22, 2.2);
    let path_norm_unit = mix(max(r_planet_def * 0.22, 1e-6), max(atmo_thickness * 0.52, 1e-6), surface_blend);
    let step_norm = (step_size / path_norm_unit) * pressure_scale;
    let h_r_norm = h_r / path_norm_unit;
    let h_m_norm = h_m / path_norm_unit;

    var optical_depth_r: f32 = 0.0;
    var optical_depth_m: f32 = 0.0;
    var optical_depth_o3: f32 = 0.0;
    var in_scatter: vec3<f32> = vec3<f32>(0.0);
    var aurora_emission: vec3<f32> = vec3<f32>(0.0);

    let oval_colat = atmo.aurora_params.x;
    let oval_width = max(atmo.aurora_params.y, 0.03);
    let aurora_int = atmo.aurora_params.z;

    for (var i = 0; i < step_count; i = i + 1) {
        let t_sample = t_start + (f32(i) + 0.5) * step_size;
        let sample_pos = ray_origin + ray_dir * t_sample;
        let p_rel = sample_pos - center;
        let sample_dist = length(p_rel);
        let norm_p = p_rel / max(sample_dist, 1e-6);
        let step_shape = directional_shape_scale(norm_p);
        let r_p_step = select(r_planet * step_shape, r_ground_eff, is_surface_view);
        let r_a_step = select(r_atmo * step_shape, r_atmo_eff, is_surface_view);
        let alt = max(sample_dist - r_p_step, 0.0);

        let rho_r = exp(-alt / h_r);
        let rho_m = exp(-alt / h_m);
        let o3_layer = exp(-pow((alt - h_r * 1.15) / (h_r * 0.70), 2.0));

        let density_r = rho_r * step_norm;
        let density_m = rho_m * step_norm;
        let density_o3 = o3_layer * step_norm;

        optical_depth_r += density_r;
        optical_depth_m += density_m;
        optical_depth_o3 += density_o3;

        // Smooth Grazing-Angle Twilight & Analytical Spherical Chapman Function
        let mu_s = dot(norm_p, star_dir);
        let s_closest = -dot(p_rel, star_dir);
        var sun_visible: f32 = 1.0;
        if (s_closest > 0.0) {
            let p_grazing = p_rel + s_closest * star_dir;
            let graze_alt = length(p_grazing) - r_p_step;
            // Smooth atmospheric refraction & twilight grazing penumbra across [-0.75 * h_r, +1.25 * h_r]
            sun_visible = smoothstep(-0.75 * h_r, 1.25 * h_r, graze_alt);
        }

        // 3D Volumetric Ring Shadow God-Rays through the atmosphere shell
        let ring_trans = evaluate_ring_shadow_transmittance(p_rel / max(r_p_step, 1e-6), star_dir, s_axis);
        sun_visible *= ring_trans;

        // Sunlight optical depth from sample point using spherical Chapman airmass
        let r_over_hr = max(r_p_step / h_r, 6.0);
        let r_over_hm = max(r_p_step / h_m, 12.0);
        let ch_r = chapman_spherical_airmass(r_over_hr, mu_s);
        let ch_m = chapman_spherical_airmass(r_over_hm, mu_s);

        let sun_depth_r = rho_r * h_r_norm * min(ch_r * 0.75, 16.0);
        let sun_depth_m = rho_m * h_m_norm * min(ch_m * 0.85, 12.0);
        let sun_depth_o3 = o3_layer * h_r_norm * min(ch_r * 0.90, 16.0);

        let tau_r = beta_r * (optical_depth_r + sun_depth_r);
        let tau_m = beta_m * (optical_depth_m + sun_depth_m);
        let tau_o3 = beta_o3 * (optical_depth_o3 + sun_depth_o3);
        let attenuation = exp(-(tau_r + tau_m + tau_o3));

        let single_slice = (beta_r * density_r * p_rayleigh + beta_m * density_m * p_mie) * attenuation * sun_visible * star_intensity;

        // Hillaire 2nd/3rd-order isotropic multiple-scattering energy transfer approximation
        let ms_atten = exp(-((tau_r + tau_m) * 0.42 + tau_o3 * 0.65)) * smoothstep(-1.10 * h_r, 0.85 * h_r, select(h_r, length(p_rel + s_closest * star_dir) - r_p_step, s_closest > 0.0));
        let multi_slice = (beta_r * density_r + beta_m * density_m * 0.65) * vec3<f32>(0.88, 0.90, 0.94) * p_iso * ms_atten * ring_trans * star_intensity * 0.52;

        in_scatter += (single_slice + multi_slice) * mix(1.8, 3.4, surface_blend);

        // Volumetric polar auroral curtain emission (Ionospheric layer)
        let is_giant_atmo = (atmo.optical_params.y > 20.0 || atmo.spin_axis.w > 0.02);
        if (aurora_int > 0.05 && !is_giant_atmo) {
            let sin_lat = dot(norm_p, s_axis);
            let polar_co = acos(clamp(abs(sin_lat), 0.0, 1.0));
            let effective_colat = clamp(oval_colat, 0.16, 0.44);
            let d_oval = abs(polar_co - effective_colat);
            let oval_ring = exp(-0.5 * (d_oval * d_oval) / (oval_width * oval_width));
            
            let step_atmo_thick = max(r_a_step - r_p_step, 1e-6);
            let alt_frac = alt / step_atmo_thick;
            let iono_layer = smoothstep(0.12, 0.40, alt_frac) * smoothstep(0.98, 0.65, alt_frac);

            // Emerald 557.7nm O(1S) fading into violet N2+ at base and red O(1D) at apex
            let auroral_rgb = mix(vec3<f32>(0.12, 0.98, 0.42), vec3<f32>(0.45, 0.22, 0.95), smoothstep(0.25, 0.80, alt_frac));
            
            // Nightside enhancement from magnetotail reconnection
            let night_mult = mix(2.2, 0.35, clamp(mu_s * 0.5 + 0.5, 0.0, 1.0));

            // Strictly polar factor
            let polar_factor = smoothstep(0.65, 0.88, abs(sin_lat));

            aurora_emission += auroral_rgb * (oval_ring * iono_layer * aurora_int * night_mult * polar_factor * (step_size / step_atmo_thick) * 3.0);
        }
    }

    // Total extinction and alpha blending
    let total_tau = beta_r * optical_depth_r + beta_m * optical_depth_m + beta_o3 * optical_depth_o3;
    let transmission = exp(-total_tau);
    let avg_transmission = (transmission.r + transmission.g + transmission.b) / 3.0;
    
    // Smooth analytic fade at the outer vacuum boundary in orbital view; preserve full sky dome in surface view
    let t_closest = clamp(-dot(ray_origin - center, ray_dir), t_start, t_end);
    let closest_pos = ray_origin + ray_dir * t_closest;
    let closest_rel = closest_pos - center;
    let closest_shape = directional_shape_scale(normalize(closest_rel));
    let rim_norm = (length(closest_rel) - r_planet * closest_shape) / max((r_atmo - r_planet) * closest_shape, 1e-6);
    if (!is_surface_view && (rim_norm >= 1.0 || hit_atmo_def.y <= 0.0)) {
        discard;
    }
    let orbital_edge_fade = smoothstep(1.0, 0.80, clamp(rim_norm, 0.0, 1.0));
    let edge_fade = mix(orbital_edge_fade, 1.0, surface_blend);

    let alpha_base = clamp(1.0 - avg_transmission, 0.0, 1.0) * edge_fade;
    let aurora_alpha = clamp(length(aurora_emission) * 0.6, 0.0, 0.85);
    let mu_ground = select(
        0.0,
        abs(dot(normalize((ray_origin + ray_dir * hit_planet.x) - center), ray_dir)),
        hit_ground
    );
    let limb_weight = pow(1.0 - clamp(mu_ground, 0.0, 1.0), 2.0);
    let orbital_alpha = clamp(
        alpha_base * mix(0.20, 0.92, limb_weight) + aurora_alpha * mix(0.45, 1.0, limb_weight),
        0.0,
        0.90
    );

    // Subtle nightside airglow
    let airglow = beta_r * 0.015 * clamp(pressure, 0.1, 2.0);
    var final_rgb = in_scatter + airglow * edge_fade + aurora_emission;
    var alpha = orbital_alpha;

    // Orbital 3D Floating Cloud Shells inside the visible atmospheric shell & across the horizon limb
    if (!is_surface_view) {
        let limb_clouds = evaluate_orbital_limb_clouds(
            ray_origin,
            ray_dir,
            center,
            r_ground_eff,
            atmo_thickness,
            hit_ground,
            hit_planet.x,
            star_dir,
            s_axis,
            final_rgb,
        );
        if (limb_clouds.a > 0.005) {
            final_rgb = mix(final_rgb, limb_clouds.rgb, limb_clouds.a);
            alpha = max(alpha, limb_clouds.a * edge_fade);
        }
    }

    // Surface Horizon Mode: full Rayleigh/Mie sky dome + 3D overhead dynamic cloud canopy
    if (surface_blend > 0.01 && !hit_ground) {
        let day_sky_lum = clamp(max(final_rgb.r, max(final_rgb.g, final_rgb.b)) * 1.35, 0.0, 1.0);
        let sky_dome_alpha = clamp(max(alpha_base * 1.45, day_sky_lum * 0.94) + aurora_alpha, 0.08, 0.96);
        alpha = mix(orbital_alpha, sky_dome_alpha, surface_blend);

        let sky_clouds = evaluate_surface_sky_clouds(
            ray_origin,
            ray_dir,
            center,
            cam_dist,
            r_planet_def,
            view_elev_sin,
            star_dir,
            s_axis,
            final_rgb,
        );
        let c_alpha = sky_clouds.a * surface_blend;
        if (c_alpha > 0.005) {
            final_rgb = mix(final_rgb, sky_clouds.rgb, c_alpha);
            alpha = max(alpha, c_alpha);
        }
    }

    out.color = vec4<f32>(final_rgb, alpha);
    return out;
}
