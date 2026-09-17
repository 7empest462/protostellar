#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
}

struct AtmosphereUniforms {
    rayleigh_params: vec4<f32>, // rgb = beta_R (Rayleigh scattering cross sections), w = H_R (scale height)
    mie_params: vec4<f32>,      // rgb = beta_M (Mie scattering cross sections), w = H_M (scale height)
    optical_params: vec4<f32>,  // x = g (mie asymmetry), y = surface_pressure_bar, z = inner planet radius, w = outer atmosphere radius
    star_dir_and_intensity: vec4<f32>, // xyz = unit star dir in world space, w = star intensity factor
    planet_center: vec4<f32>,   // xyz = planet world pos, w = outer shell scale factor
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

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let pressure = atmo.optical_params.y;
    if (pressure <= 0.001) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let ray_origin = view.world_position;
    let ray_dir = normalize(in.world_position.xyz - ray_origin);
    let center = atmo.planet_center.xyz;
    let r_planet = atmo.optical_params.z;
    let r_atmo = atmo.optical_params.w;

    // 1. Ray-sphere intersection with outer atmosphere shell
    let hit_atmo = ray_sphere_intersect(ray_origin, ray_dir, center, r_atmo);
    if (hit_atmo.y <= 0.0) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let t_start = max(hit_atmo.x, 0.0);
    var t_end = hit_atmo.y;

    // 2. Ray-sphere intersection with inner solid planet
    let hit_planet = ray_sphere_intersect(ray_origin, ray_dir, center, r_planet);
    var hit_ground = false;
    if (hit_planet.x > 0.0) {
        t_end = min(t_end, hit_planet.x);
        hit_ground = true;
    }

    if (t_end <= t_start) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    // 3. Scattering parameters and scale heights
    let beta_r = atmo.rayleigh_params.rgb;
    let h_r = max(atmo.rayleigh_params.w * r_planet, 0.002);
    let beta_m = atmo.mie_params.rgb;
    let h_m = max(atmo.mie_params.w * r_planet, 0.002);
    let g = clamp(atmo.optical_params.x, 0.50, 0.95);
    let star_dir = atmo.star_dir_and_intensity.xyz;
    let star_intensity = atmo.star_dir_and_intensity.w;

    // Phase functions
    let cos_theta = dot(ray_dir, star_dir);
    let cos2_theta = cos_theta * cos_theta;
    let p_rayleigh = (3.0 / (16.0 * PI)) * (1.0 + cos2_theta);
    
    // Cornette-Shanks / Henyey-Greenstein Mie aerosol forward-scattering phase
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    let p_mie = (3.0 * (1.0 - g2) / (8.0 * PI * (2.0 + g2))) * (1.0 + cos2_theta) / (denom * sqrt(max(denom, 1e-4)));

    // 4. Numerical single-scattering raymarching (6 sample steps)
    let step_count = 6;
    let step_size = (t_end - t_start) / f32(step_count);

    var optical_depth_r: f32 = 0.0;
    var optical_depth_m: f32 = 0.0;
    var in_scatter: vec3<f32> = vec3<f32>(0.0);

    for (var i = 0; i < step_count; i = i + 1) {
        let t_sample = t_start + (f32(i) + 0.5) * step_size;
        let sample_pos = ray_origin + ray_dir * t_sample;
        let alt = length(sample_pos - center) - r_planet;

        if (alt < 0.0) {
            continue;
        }

        let density_r = exp(-alt / h_r) * step_size;
        let density_m = exp(-alt / h_m) * step_size;
        optical_depth_r += density_r;
        optical_depth_m += density_m;

        // Sunlight shadowing by inner planet body
        let hit_sun_block = ray_sphere_intersect(sample_pos, star_dir, center, r_planet);
        var sun_visible: f32 = 1.0;
        if (hit_sun_block.y > 0.0 && hit_sun_block.x > 0.0) {
            sun_visible = 0.0;
        }

        // Sunlight optical depth from sample point to outer atmosphere boundary
        let hit_sun_atmo = ray_sphere_intersect(sample_pos, star_dir, center, r_atmo);
        let sun_path = max(hit_sun_atmo.y, 0.0);
        let sun_depth_r = exp(-alt / h_r) * sun_path;
        let sun_depth_m = exp(-alt / h_m) * sun_path;

        let tau_r = beta_r * (optical_depth_r + sun_depth_r);
        let tau_m = beta_m * (optical_depth_m + sun_depth_m);
        let attenuation = exp(-(tau_r + tau_m));

        let scatter_slice = (beta_r * density_r * p_rayleigh + beta_m * density_m * p_mie) * attenuation * sun_visible * star_intensity;
        in_scatter += scatter_slice;
    }

    // 5. Total extinction and alpha blending
    let total_tau = beta_r * optical_depth_r + beta_m * optical_depth_m;
    let transmission = exp(-total_tau);
    let avg_transmission = (transmission.r + transmission.g + transmission.b) / 3.0;
    
    // Smooth fade at the vacuum boundary
    let rim_norm = (length(in.world_position.xyz - center) - r_planet) / max(r_atmo - r_planet, 0.001);
    let edge_fade = smoothstep(1.0, 0.85, rim_norm);

    let alpha_base = clamp(1.0 - avg_transmission, 0.0, 1.0) * edge_fade;
    let alpha = select(clamp(alpha_base * 1.35, 0.0, 0.95), clamp(alpha_base * 0.80, 0.0, 0.88), hit_ground);

    // Subtle nightside airglow
    let airglow = beta_r * 0.015 * clamp(pressure, 0.1, 2.0);
    let final_rgb = in_scatter + airglow * edge_fade;

    out.color = vec4<f32>(final_rgb, alpha);
    return out;
}
