#import bevy_pbr::{
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct PlanetExtension {
    planet_type: u32,
    temperature: f32,
    time: f32,
    color_seed: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var<uniform> planet: PlanetExtension;

// Simple 3D hash
fn hash3(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.x + r.y) * r.z);
}

// Simple value noise
fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    
    // Smoothstep
    let u = f * f * (3.0 - 2.0 * f);
    
    let n000 = hash3(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));
    
    let mx0 = mix(n000, n100, u.x);
    let mx1 = mix(n010, n110, u.x);
    let mx2 = mix(n001, n101, u.x);
    let mx3 = mix(n011, n111, u.x);
    
    let my0 = mix(mx0, mx1, u.y);
    let my1 = mix(mx2, mx3, u.y);
    
    return mix(my0, my1, u.z);
}

// Fractal Brownian Motion
fn fbm(p: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var shift = vec3<f32>(100.0);
    var pos = p;
    for (var i = 0; i < 5; i = i + 1) {
        v += a * noise(pos);
        pos = pos * 2.0 + shift;
        a *= 0.5;
    }
    return v;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // generate a PbrInput struct from the StandardMaterial bindings
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    
    let norm = normalize(in.world_normal);
    let n_time = planet.time * 0.1;
    let base = planet.color_seed.rgb;
    
    var color = base;
    
    // Gas Giant (Banded, zonal flow, swirling storm spots)
    if (planet.planet_type == 1u) {
        let lat = norm.y * 6.0;
        let flow_dir = sign(sin(lat * 2.0));
        let storm_coord = norm * 3.5 + vec3<f32>(n_time * flow_dir * 0.6, 0.0, n_time * flow_dir * 0.6);
        let band = sin(lat + fbm(storm_coord) * 1.6) * 0.5 + 0.5;
        let eddies = fbm(norm * 5.0 - vec3<f32>(n_time * 0.3));
        
        let dark_band = mix(base * 0.55, vec3<f32>(0.80, 0.45, 0.25), 0.5);
        let bright_band = mix(base * 1.30, vec3<f32>(0.98, 0.90, 0.75), 0.5);
        color = mix(dark_band, bright_band, band) * (0.85 + eddies * 0.35);
    } 
    // Ice Giant (Pale azure / cyan, soft atmospheric striations, methane haze)
    else if (planet.planet_type == 2u) {
        let lat = norm.y * 5.0;
        let haze = fbm(norm * 3.0 + vec3<f32>(n_time * 0.25, 0.0, n_time * 0.25));
        let band = sin(lat + haze * 0.7) * 0.5 + 0.5;
        let deep_ice = mix(base * 0.75, vec3<f32>(0.20, 0.55, 0.90), 0.6);
        let pale_ice = mix(base * 1.30, vec3<f32>(0.75, 0.95, 1.00), 0.7);
        color = mix(deep_ice, pale_ice, band * 0.6 + haze * 0.4);
    } 
    // Terrestrial (Continents, oceans, icecaps, thermal adaptation)
    else if (planet.planet_type == 3u) {
        let elev = fbm(norm * 4.5);
        let ice_cap = abs(norm.y);
        
        // Very hot planets (Venus/Lava)
        if (planet.temperature > 400.0) {
            if (elev > 0.6) {
                color = vec3<f32>(0.25, 0.12, 0.05); // rock
            } else {
                color = mix(base, vec3<f32>(0.85, 0.25, 0.05), elev); // lava / dense clouds
                if (elev < 0.4) {
                    pbr_input.material.emissive = vec4<f32>(1.0, 0.35, 0.05, 1.0) * (0.4 - elev) * 6.0;
                }
            }
        } 
        // Earth-like / Habitable
        else if (planet.temperature > 250.0 && planet.temperature <= 400.0) {
            if (ice_cap > 0.82 - (273.0 / planet.temperature) * 0.08) {
                color = vec3<f32>(0.92, 0.95, 0.98); // polar ice caps
            } else if (elev > 0.52) {
                color = vec3<f32>(0.15, 0.55, 0.22); // lush land / vegetation
                if (elev > 0.70) { color = vec3<f32>(0.45, 0.35, 0.20); } // mountain ranges
            } else {
                color = vec3<f32>(0.02, 0.22, 0.75); // ocean
            }
        }
        // Mars-like / Barren Red / Desert (Cool 190K - 250K)
        else if (planet.temperature > 190.0) {
            if (ice_cap > 0.88) {
                color = vec3<f32>(0.88, 0.88, 0.92); // dry ice caps
            } else {
                color = mix(vec3<f32>(0.68, 0.26, 0.12), vec3<f32>(0.35, 0.15, 0.05), elev); // oxidized rust
            }
        }
        // Frozen Ice World / Cryo-world (< 190K, Outer system)
        else {
            let frost = fbm(norm * 6.0);
            if (ice_cap > 0.65 || frost > 0.45) {
                color = mix(vec3<f32>(0.85, 0.92, 0.98), vec3<f32>(0.55, 0.75, 0.92), elev); // glacial nitrogen / methane ice
            } else {
                color = mix(vec3<f32>(0.35, 0.30, 0.38), vec3<f32>(0.65, 0.70, 0.80), elev); // dark silicate / cryo-crust
            }
        }
    }
    // Moon / Asteroid
    else if (planet.planet_type == 4u) {
        let crater = fbm(norm * 8.0);
        color = mix(base * 0.5, base * 1.1, crater);
    }
    
    // Only apply the color change if planet type is recognized (so we don't mess up stars, which are 0u)
    if (planet.planet_type != 0u) {
        pbr_input.material.base_color = vec4<f32>(color, 1.0);
    }
    
    // alpha discard
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    
    if (planet.planet_type == 0u) {
        // Bypass lighting entirely for the star, force it to be bright!
        out.color = vec4<f32>(planet.color_seed.rgb * 5.0, 1.0);
        // Fallback if color_seed is somehow black
        if (length(out.color.rgb) < 0.1) {
            out.color = vec4<f32>(10.0, 9.0, 8.0, 1.0);
        }
    } else {
        let lit = apply_pbr_lighting(pbr_input);
        // Ambient starlight illumination floor + soft fresnel atmospheric rim so outer planets are always beautifully visible!
        let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0);
        let fresnel = pow(1.0 - NdotV, 3.0);
        let ambient_boost = pbr_input.material.base_color.rgb * 0.40;
        let rim_boost = pbr_input.material.base_color.rgb * fresnel * 0.45;
        out.color = vec4<f32>(lit.rgb + ambient_boost + rim_boost, 1.0);
    }
    
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
