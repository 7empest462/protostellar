// WGSL 3D Volumetric Raymarching Shader for Giant Molecular Clouds (GMC) & Ionization Cavities
// Evaluates ray-box intersection, Beer-Lambert absorption, Henyey-Greenstein scattering,
// and dynamic HII ionization bubbles carved by protostellar radiation pressure.

#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
}

struct VolumetricNebulaUniforms {
    box_min: vec3<f32>,
    step_count: u32,
    box_max: vec3<f32>,
    absorption_coefficient: f32,
    scattering_albedo: f32,
    phase_g: f32,
    num_stars: u32,
    _pad: u32,
    star_positions_and_cavities: array<vec4<f32>, 16>, // xyz: world_pos, w: cavity_radius_au
    star_colors_and_lum: array<vec4<f32>, 16>,        // rgb: emission color, w: luminosity_solar
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> neb: VolumetricNebulaUniforms;

const PI: f32 = 3.14159265359;

// Fast 3D Hash and Value Noise
fn hash3(p: vec3<f32>) -> f32 {
    let q = vec3<f32>(
        dot(p, vec3<f32>(127.1, 311.7, 74.7)),
        dot(p, vec3<f32>(269.5, 183.3, 246.1)),
        dot(p, vec3<f32>(113.5, 271.9, 124.6))
    );
    return fract(sin(dot(q, vec3<f32>(1.0, 1.0, 1.0))) * 43758.5453123);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n000 = hash3(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));

    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);

    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);

    return mix(nxy0, nxy1, u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var shift = vec3<f32>(100.0);
    var pos = p;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + a * noise3(pos);
        pos = pos * 2.0 + shift;
        a = a * 0.5;
    }
    return v;
}

// Ray-box slab intersection test against [box_min, box_max]
fn intersect_box(ray_orig: vec3<f32>, ray_dir: vec3<f32>, b_min: vec3<f32>, b_max: vec3<f32>) -> vec2<f32> {
    let safe_d = select(ray_dir, vec3<f32>(1e-6) * sign(ray_dir + vec3<f32>(1e-9)), abs(ray_dir) < vec3<f32>(1e-6));
    let inv_d = 1.0 / safe_d;
    let t0 = (b_min - ray_orig) * inv_d;
    let t1 = (b_max - ray_orig) * inv_d;

    let t_min = min(t0, t1);
    let t_max = max(t0, t1);

    let t_near = max(max(t_min.x, t_min.y), t_min.z);
    let t_far = min(min(t_max.x, t_max.y), t_max.z);

    return vec2<f32>(t_near, t_far);
}

// Henyey-Greenstein anisotropic phase scattering function
fn henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
    let denom = pow(max(1.0 + g * g - 2.0 * g * cos_theta, 1e-6), 1.5);
    return (1.0 - g * g) / (4.0 * PI * denom);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let ray_origin = view.world_position;
    let ray_dir = normalize(in.world_position.xyz - ray_origin);

    // 1. Ray-Box Intersection
    let hit = intersect_box(ray_origin, ray_dir, neb.box_min, neb.box_max);
    let t_near = max(hit.x, 0.0);
    let t_far = hit.y;

    if (t_near >= t_far || t_far <= 0.0) {
        out.color = vec4<f32>(0.0);
        return out;
    }

    let march_dist = min(t_far - t_near, 1100.0);
    let steps = clamp(neb.step_count, 16u, 96u);
    let ds = march_dist / f32(steps);

    var transmittance = 1.0;
    var accumulated_color = vec3<f32>(0.0);

    var current_t = t_near + ds * 0.5;

    for (var s = 0u; s < steps; s = s + 1u) {
        if (transmittance < 0.01) {
            break; // Optical depth saturation early exit
        }

        let sample_pos = ray_origin + ray_dir * current_t;
        let r_core = length(sample_pos);

        // Ambient dense cold molecular cloud core profile (dense Plummer sphere + turbulent FBM filaments)
        let core_profile = 1.0 / pow(1.0 + (r_core / 240.0) * (r_core / 240.0), 1.25);
        let fbm_turb = fbm3(sample_pos * 0.008);
        var density = core_profile * (0.40 + 0.85 * fbm_turb);

        // Ionization cavity carving and glowing H-alpha / [O III] emission fronts around protostars
        var cavity_glow = vec3<f32>(0.0);

        for (var i = 0u; i < neb.num_stars; i = i + 1u) {
            let star_data = neb.star_positions_and_cavities[i];
            let star_pos = star_data.xyz;
            let cav_radius = star_data.w;
            let star_lum = neb.star_colors_and_lum[i].w;

            if (cav_radius > 0.5) {
                let dist_to_star = length(sample_pos - star_pos);

                // Carve cavity: density cleared inside the bubble
                let cavity_factor = smoothstep(cav_radius * 0.45, cav_radius, dist_to_star);
                density = density * cavity_factor;

                // Ionization shock front glow: H-alpha (656 nm, crimson) and [O III] (501 nm, teal)
                let edge_dist = abs(dist_to_star - cav_radius);
                if (edge_dist < 25.0) {
                    let front_intensity = (1.0 - edge_dist / 25.0) * clamp(star_lum * 0.45, 0.4, 5.0);
                    let h_alpha = vec3<f32>(1.0, 0.15, 0.35); // Crimson / magenta ionization
                    let o_iii = vec3<f32>(0.1, 0.85, 0.9);    // Forbidden teal [O III] line
                    cavity_glow = cavity_glow + mix(h_alpha, o_iii, fbm3(sample_pos * 0.03)) * front_intensity;
                }
            }
        }

        // Optical depth and Beer-Lambert extinction
        let d_tau = neb.absorption_coefficient * density * ds;
        let step_transmittance = exp(-d_tau);

        // Protostellar starlight in-scattering
        var in_scatter = vec3<f32>(0.0);
        for (var i = 0u; i < neb.num_stars; i = i + 1u) {
            let star_pos = neb.star_positions_and_cavities[i].xyz;
            let star_color = neb.star_colors_and_lum[i].rgb;
            let star_lum = neb.star_colors_and_lum[i].w;

            let light_vec = star_pos - sample_pos;
            let light_dist = max(length(light_vec), 1.0);
            let light_dir = light_vec / light_dist;

            let cos_theta = dot(ray_dir, light_dir);
            let phase = henyey_greenstein(cos_theta, neb.phase_g);

            let illuminance = (star_color * star_lum * 12.0) / (light_dist * light_dist + 100.0);
            in_scatter = in_scatter + illuminance * phase;
        }

        // Ambient cold molecular gas emission (rich celestial hues across radii)
        let r_norm = clamp(r_core / 450.0, 0.0, 1.0);
        let core_color = vec3<f32>(1.25, 0.65, 0.28);  // Warm glowing pre-stellar amber core
        let mid_color = vec3<f32>(0.20, 0.75, 0.85);   // Luminous celestial cyan / teal
        let outer_color = vec3<f32>(0.55, 0.25, 0.85); // Cosmic violet outer veil

        let gas_color = mix(core_color, mix(mid_color, outer_color, smoothstep(0.35, 0.85, r_norm)), smoothstep(0.10, 0.45, r_norm));
        let ambient_gas = gas_color * density * 2.2;
        let ambient_scatter = vec3<f32>(0.25, 0.45, 0.85) * density * neb.scattering_albedo;

        let total_emission = ambient_gas + ambient_scatter + in_scatter * neb.scattering_albedo + cavity_glow;
        accumulated_color = accumulated_color + transmittance * total_emission * (1.0 - step_transmittance);

        transmittance = transmittance * step_transmittance;
        current_t = current_t + ds;
    }

    let alpha = clamp(1.0 - transmittance, 0.0, 0.96);
    out.color = vec4<f32>(accumulated_color, alpha);
    return out;
}
