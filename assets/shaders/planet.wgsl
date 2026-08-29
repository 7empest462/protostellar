#import bevy_pbr::{
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
    
    let pos = in.world_position.xyz;
    let n_time = planet.time * 0.1;
    let base = planet.color_seed.rgb;
    
    var color = base;
    
    // Gas Giant (Banded)
    if (planet.planet_type == 1u) {
        let band_coord = pos.y * 5.0 + fbm(pos * 2.0 + vec3<f32>(n_time, 0.0, n_time)) * 2.0;
        let band = sin(band_coord) * 0.5 + 0.5;
        let swirl = fbm(pos * 4.0 - vec3<f32>(n_time));
        color = mix(base * 0.5, base * 1.5, band * swirl);
    } 
    // Ice Giant (Pale, subtle bands)
    else if (planet.planet_type == 2u) {
        let band_coord = pos.y * 3.0;
        let band = sin(band_coord) * 0.5 + 0.5;
        let swirl = fbm(pos * 2.0);
        color = mix(base * 0.8, base * 1.2, band * swirl);
    } 
    // Terrestrial (Continents, oceans, icecaps)
    else if (planet.planet_type == 3u) {
        let elev = fbm(pos * 5.0);
        let ice_cap = abs(in.world_normal.y);
        
        // Very hot planets (Venus/Lava)
        if (planet.temperature > 400.0) {
            if (elev > 0.6) {
                color = vec3<f32>(0.2, 0.1, 0.0); // rock
            } else {
                color = mix(base, vec3<f32>(0.8, 0.2, 0.0), elev); // lava / dense clouds
                if (elev < 0.4) {
                    pbr_input.material.emissive = vec4<f32>(1.0, 0.3, 0.0, 1.0) * (0.4 - elev) * 5.0;
                }
            }
        } 
        // Earth-like / Habitable
        else if (planet.temperature > 250.0 && planet.temperature < 320.0) {
            if (ice_cap > 0.85 - (273.0 / planet.temperature) * 0.1) {
                color = vec3<f32>(0.9, 0.9, 0.95); // ice
            } else if (elev > 0.5) {
                color = vec3<f32>(0.1, 0.6, 0.2); // land / vegetation
                if (elev > 0.7) { color = vec3<f32>(0.4, 0.3, 0.1); } // mountains
            } else {
                color = vec3<f32>(0.0, 0.2, 0.8); // ocean
            }
        }
        // Mars-like / Barren Red
        else {
            if (ice_cap > 0.9) {
                color = vec3<f32>(0.8, 0.8, 0.8); // dry ice caps
            } else {
                color = mix(vec3<f32>(0.6, 0.2, 0.1), vec3<f32>(0.3, 0.1, 0.0), elev); // rusty
            }
        }
    }
    // Moon / Asteroid
    else if (planet.planet_type == 4u) {
        let crater = fbm(pos * 8.0);
        color = mix(base * 0.5, base, crater);
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
        out.color = apply_pbr_lighting(pbr_input);
    }
    
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
