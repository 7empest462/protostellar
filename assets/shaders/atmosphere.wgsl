#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
}

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
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
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

    let l_dot_s = dot(star_dir, s_axis);
    let p_dot_s = dot(p_rel_norm, s_axis);
    if (abs(l_dot_s) <= 1e-4) {
        return 1.0;
    }
    let t_ring = -p_dot_s / l_dot_s;
    if (t_ring <= 0.0) {
        return 1.0;
    }
    let p_int = p_rel_norm + t_ring * star_dir;
    let r_int = length(p_int);
    if (r_int < r_inner || r_int > r_outer) {
        return 1.0;
    }
    let u = clamp((r_int - r_inner) / max(r_outer - r_inner, 0.001), 0.0, 1.0);
    var ring_density = 0.0;
    if (u < 0.22) {
        ring_density = mix(0.10, 0.35, u / 0.22);
    } else if (u < 0.65) {
        ring_density = 0.95;
    } else if (u < 0.72) {
        let gap_t = (u - 0.65) / (0.72 - 0.65);
        ring_density = (1.0 - sin(gap_t * PI)) * 0.12;
    } else if (u < 0.96) {
        var d_a = 0.75;
        if (u >= 0.855 && u <= 0.875) {
            let encke_t = (u - 0.855) / (0.875 - 0.855);
            d_a *= (1.0 - sin(encke_t * PI) * 0.88);
        }
        ring_density = d_a;
    } else {
        ring_density = (1.0 - (u - 0.96) / 0.04) * 0.35;
    }
    let edge_feather = smoothstep(r_inner, r_inner + 0.03, r_int) * (1.0 - smoothstep(r_outer - 0.03, r_outer, r_int));
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

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let ray_origin = view.world_position;
    let ray_dir = normalize(in.world_position.xyz - ray_origin);
    let center = atmo.planet_center.xyz;
    let r_planet = atmo.optical_params.z;
    let r_atmo = atmo.optical_params.w;

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

    let hit_atmo_def = ray_sphere_intersect(ray_origin, ray_dir, center, r_atmo_def);
    if (hit_atmo_def.y <= 0.0) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let t_start = max(hit_atmo_def.x, 0.0);
    var t_end = hit_atmo_def.y;

    // Ray-sphere intersection with deformed inner solid planet
    let hit_planet = ray_sphere_intersect(ray_origin, ray_dir, center, r_planet_def);
    var hit_ground = false;
    if (hit_planet.x > 0.0) {
        t_end = min(t_end, hit_planet.x);
        hit_ground = true;
    }

    if (t_end <= t_start) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    // Scattering parameters and scale heights
    let beta_r = atmo.rayleigh_params.rgb;
    let h_r = max(atmo.rayleigh_params.w * r_planet_def, 0.002);
    let beta_m = atmo.mie_params.rgb;
    let h_m = max(atmo.mie_params.w * r_planet_def, 0.002);
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
    
    // Cornette-Shanks / Henyey-Greenstein Mie aerosol forward-scattering phase
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    let p_mie = (3.0 * (1.0 - g2) / (8.0 * PI * (2.0 + g2))) * (1.0 + cos2_theta) / (denom * sqrt(max(denom, 1e-4)));

    // Numerical single-scattering raymarching (7 sample steps)
    let step_count = 7;
    let step_size = (t_end - t_start) / f32(step_count);

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
        let norm_p = p_rel / max(sample_dist, 0.0001);
        let step_shape = directional_shape_scale(norm_p);
        let r_p_step = r_planet * step_shape;
        let r_a_step = r_atmo * step_shape;
        let alt = sample_dist - r_p_step;

        if (alt < 0.0) {
            continue;
        }

        let density_r = exp(-alt / h_r) * step_size;
        let density_m = exp(-alt / h_m) * step_size;
        let o3_layer = exp(-pow((alt - h_r * 1.15) / (h_r * 0.70), 2.0));
        let density_o3 = o3_layer * step_size;

        optical_depth_r += density_r;
        optical_depth_m += density_m;
        optical_depth_o3 += density_o3;

        // Smooth Grazing-Angle Twilight & Chapman Function (replaces hard binary shadow step)
        let mu_s = dot(norm_p, star_dir);
        let s_closest = -dot(p_rel, star_dir);
        var sun_visible: f32 = 1.0;
        if (s_closest > 0.0) {
            let p_grazing = p_rel + s_closest * star_dir;
            let graze_alt = length(p_grazing) - r_p_step;
            // Smooth atmospheric refraction & twilight grazing penumbra across [-0.65 * h_r, +1.15 * h_r]
            sun_visible = smoothstep(-0.65 * h_r, 1.15 * h_r, graze_alt);
        }

        // 3D Volumetric Ring Shadow God-Rays through the atmosphere shell
        let ring_trans = evaluate_ring_shadow_transmittance(p_rel / max(r_p_step, 0.0001), star_dir, s_axis);
        sun_visible *= ring_trans;

        // Sunlight optical depth from sample point to outer atmosphere boundary
        let hit_sun_atmo = ray_sphere_intersect(sample_pos, star_dir, center, r_a_step);
        let sun_path = max(hit_sun_atmo.y, 0.0);
        let chapman_grazing = 1.0 / max(mu_s + 0.16, 0.05);
        let sun_depth_r = exp(-alt / h_r) * sun_path * min(chapman_grazing * 0.42, 3.4);
        let sun_depth_m = exp(-alt / h_m) * sun_path;
        let sun_depth_o3 = o3_layer * sun_path * min(chapman_grazing * 0.65, 4.2);

        let tau_r = beta_r * (optical_depth_r + sun_depth_r);
        let tau_m = beta_m * (optical_depth_m + sun_depth_m);
        let tau_o3 = beta_o3 * (optical_depth_o3 + sun_depth_o3);
        let attenuation = exp(-(tau_r + tau_m + tau_o3));

        let scatter_slice = (beta_r * density_r * p_rayleigh + beta_m * density_m * p_mie) * attenuation * sun_visible * star_intensity;
        in_scatter += scatter_slice;

        // Volumetric polar auroral curtain emission (Ionospheric layer)
        // Gas giants and deep atmospheres already have high-resolution Birkeland auroras rendered
        // directly onto their cloud deck in planet.wgsl; skip volumetric layer to prevent floating detached auroras.
        let is_giant_atmo = (atmo.optical_params.y > 20.0 || atmo.spin_axis.w > 0.02);
        if (aurora_int > 0.05 && !is_giant_atmo) {
            let sin_lat = dot(norm_p, s_axis);
            let polar_co = acos(clamp(abs(sin_lat), 0.0, 1.0));
            let effective_colat = clamp(oval_colat, 0.16, 0.44);
            let d_oval = abs(polar_co - effective_colat);
            let oval_ring = exp(-0.5 * (d_oval * d_oval) / (oval_width * oval_width));
            
            let atmo_thickness = max(r_a_step - r_p_step, 0.001);
            let alt_frac = alt / atmo_thickness;
            let iono_layer = smoothstep(0.12, 0.40, alt_frac) * smoothstep(0.98, 0.65, alt_frac);

            // Emerald 557.7nm O(1S) fading into violet N2+ at base and red O(1D) at apex
            let auroral_rgb = mix(vec3<f32>(0.12, 0.98, 0.42), vec3<f32>(0.45, 0.22, 0.95), smoothstep(0.25, 0.80, alt_frac));
            
            // Nightside enhancement from magnetotail reconnection
            let night_mult = mix(2.2, 0.35, clamp(mu_s * 0.5 + 0.5, 0.0, 1.0));

            // Strictly polar factor
            let polar_factor = smoothstep(0.65, 0.88, abs(sin_lat));

            aurora_emission += auroral_rgb * (oval_ring * iono_layer * aurora_int * night_mult * polar_factor * (step_size / atmo_thickness) * 3.0);
        }
    }

    // Total extinction and alpha blending
    let total_tau = beta_r * optical_depth_r + beta_m * optical_depth_m + beta_o3 * optical_depth_o3;
    let transmission = exp(-total_tau);
    let avg_transmission = (transmission.r + transmission.g + transmission.b) / 3.0;
    
    // Smooth fade at the outer vacuum boundary (preserves full sky dome when viewing from surface inside r_atmo)
    let t_closest = clamp(-dot(ray_origin - center, ray_dir), t_start, t_end);
    let closest_pos = ray_origin + ray_dir * t_closest;
    let closest_rel = closest_pos - center;
    let closest_shape = directional_shape_scale(normalize(closest_rel));
    let rim_norm = (length(closest_rel) - r_planet * closest_shape) / max((r_atmo - r_planet) * closest_shape, 0.0001);
    let edge_fade = smoothstep(1.0, 0.76, rim_norm);

    let alpha_base = clamp(1.0 - avg_transmission, 0.0, 1.0) * edge_fade;
    let aurora_alpha = clamp(length(aurora_emission) * 0.6, 0.0, 0.85);
    let alpha = select(clamp(alpha_base * 1.35 + aurora_alpha, 0.0, 0.95), clamp(alpha_base * 0.38 + aurora_alpha * 0.5, 0.0, 0.78), hit_ground);

    // Subtle nightside airglow
    let airglow = beta_r * 0.015 * clamp(pressure, 0.1, 2.0);
    let final_rgb = in_scatter + airglow * edge_fade + aurora_emission;

    out.color = vec4<f32>(final_rgb, alpha);
    return out;
}
