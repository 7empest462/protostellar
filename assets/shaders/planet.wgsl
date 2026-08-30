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
    composition: vec4<f32>, // x: rock, y: ice (volatiles/water), z: metal, w: gas (atmosphere)
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
    
    let rock = planet.composition.x;
    let ice = planet.composition.y;
    let metal = planet.composition.z;
    let gas = planet.composition.w;
    let temp = planet.temperature;
    
    var color = base;
    
    // 1. Gas Giant (Banded, zonal flow, swirling storm spots)
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
    // 2. Ice Giant (Pale azure / cyan, soft atmospheric striations, methane haze)
    else if (planet.planet_type == 2u) {
        let lat = norm.y * 5.0;
        let haze = fbm(norm * 3.0 + vec3<f32>(n_time * 0.25, 0.0, n_time * 0.25));
        let band = sin(lat + haze * 0.7) * 0.5 + 0.5;
        let deep_ice = mix(base * 0.75, vec3<f32>(0.20, 0.55, 0.90), 0.6);
        let pale_ice = mix(base * 1.30, vec3<f32>(0.75, 0.95, 1.00), 0.7);
        color = mix(deep_ice, pale_ice, band * 0.6 + haze * 0.4);
    } 
    // 3. Terrestrial Planets & Protoplanetary Embryos (Composition-Driven Synthesis)
    else if (planet.planet_type == 3u) {
        let elev = fbm(norm * 4.5);
        let polar_angle = abs(norm.y);
        
        // A. Extreme Hot / Lava World (> 500K)
        if (temp > 500.0) {
            if (elev > 0.55) {
                color = vec3<f32>(0.22, 0.14, 0.10); // dark cooled basalt
            } else {
                let lava = (0.55 - elev) * 3.5;
                color = mix(vec3<f32>(0.35, 0.10, 0.05), vec3<f32>(1.0, 0.45, 0.08), clamp(lava, 0.0, 1.0));
                pbr_input.material.emissive = vec4<f32>(1.0, 0.38, 0.06, 1.0) * clamp(lava * 4.0, 0.0, 5.0);
            }
        }
        // B. Thick Gaseous Atmosphere Super-Earth (gas > 0.18)
        else if (gas > 0.18) {
            let lat = norm.y * 4.0;
            let clouds = fbm(norm * 4.0 + vec3<f32>(n_time * 0.3, 0.0, n_time * 0.3));
            let band = sin(lat + clouds * 1.2) * 0.5 + 0.5;
            let cloud_base = mix(vec3<f32>(0.85, 0.75, 0.60), vec3<f32>(0.95, 0.88, 0.78), band);
            color = cloud_base * (0.90 + clouds * 0.20);
        }
        // C. Water-Bearing / Habitable Ocean World (ice >= 0.04 and 250K <= temp <= 390K)
        else if (ice >= 0.04 && temp >= 250.0 && temp <= 390.0) {
            let sea_level = clamp(0.42 + (ice - 0.04) * 0.60, 0.44, 0.78);
            let ice_cap_thresh = clamp(0.92 - (ice * 0.40) - (273.0 / max(temp, 150.0)) * 0.06, 0.65, 0.98);
            
            // Polar Ice Caps (form only if water is present)
            if (polar_angle > ice_cap_thresh) {
                color = vec3<f32>(0.92, 0.96, 0.99); // bright polar ice sheets
            }
            // Oceans (liquid water in depressions below sea level)
            else if (elev < sea_level) {
                let depth = (sea_level - elev) / max(sea_level, 0.1);
                color = mix(vec3<f32>(0.05, 0.38, 0.75), vec3<f32>(0.01, 0.12, 0.48), clamp(depth * 1.5, 0.0, 1.0)); // azure to deep ocean
            }
            // Continents / Landmasses
            else {
                // If atmosphere is present and temperature is mild: Vegetation & Earth-like biomes!
                if (gas >= 0.01 && temp >= 265.0 && temp <= 340.0) {
                    if (elev > sea_level + 0.22) {
                        color = mix(vec3<f32>(0.45, 0.38, 0.25), vec3<f32>(0.75, 0.72, 0.70), (elev - (sea_level + 0.22)) * 4.0); // mountains & snow peaks
                    } else if (elev > sea_level + 0.10) {
                        color = vec3<f32>(0.32, 0.48, 0.18); // lush forest / temperate biome
                    } else {
                        color = vec3<f32>(0.22, 0.58, 0.25); // fertile green plains & coastlines
                    }
                }
                // Else dry continents without vegetation
                else {
                    if (elev > sea_level + 0.15) {
                        color = vec3<f32>(0.48, 0.35, 0.22); // mountain ranges
                    } else {
                        color = vec3<f32>(0.65, 0.45, 0.28); // coastal scrub / dry land
                    }
                }
            }
            
            // Atmospheric Clouds (if atmosphere is present)
            if (gas >= 0.01) {
                let clouds = fbm(norm * 5.0 + vec3<f32>(n_time * 0.4, 0.0, n_time * 0.4));
                if (clouds > 0.58) {
                    let cloud_alpha = (clouds - 0.58) * 2.2;
                    color = mix(color, vec3<f32>(0.96, 0.97, 0.99), clamp(cloud_alpha, 0.0, 0.85));
                }
            }
        }
        // D. Metal-Rich World (metal > 0.42, e.g. Proto-Mercury)
        else if (metal > 0.42) {
            let sheen = fbm(norm * 7.0);
            let craters = fbm(norm * 9.5);
            let dark_graphite = vec3<f32>(0.18, 0.18, 0.20);
            let nickel_iron = vec3<f32>(0.65, 0.62, 0.58);
            color = mix(dark_graphite, nickel_iron, sheen * 0.6 + craters * 0.4);
        }
        // E. Cold Cryo / Frozen Glacial World (temp < 240K or high ice in cold zones)
        else if (temp < 240.0 && ice > 0.15) {
            let frost = fbm(norm * 6.0);
            let glacial = mix(vec3<f32>(0.80, 0.90, 0.98), vec3<f32>(0.35, 0.70, 0.88), elev);
            let bedrock = mix(vec3<f32>(0.30, 0.28, 0.32), vec3<f32>(0.50, 0.52, 0.58), frost);
            color = mix(bedrock, glacial, clamp(ice * 2.0, 0.0, 1.0));
        }
        // F. Barren Dry Rocky World (No water, no atmosphere, e.g. Moon, Mercury, dry Mars)
        else {
            let craters = fbm(norm * 8.0);
            let highlands = fbm(norm * 3.5);
            
            if (temp > 280.0) {
                // Warm oxidized / terracotta / desert rocky crust
                let lowlands = vec3<f32>(0.42, 0.25, 0.15); // dark basalt maria
                let peaks = vec3<f32>(0.72, 0.48, 0.28);    // terracotta highlands
                color = mix(lowlands, peaks, highlands * 0.7 + craters * 0.3);
            } else {
                // Cool grey anorthosite / silicate rock
                let lowlands = vec3<f32>(0.22, 0.22, 0.24); // dark lunar-like maria
                let peaks = vec3<f32>(0.55, 0.54, 0.52);    // bright cratered highlands
                color = mix(lowlands, peaks, highlands * 0.7 + craters * 0.3);
            }
        }
    }
    // 4. Moon / Asteroid (Cratered, dusty, barren)
    else if (planet.planet_type == 4u) {
        let crater = fbm(norm * 8.0);
        let roughness_noise = fbm(norm * 14.0);
        color = mix(base * 0.45, base * 1.15, crater * 0.7 + roughness_noise * 0.3);
    }
    
    // Apply final base color for non-star bodies
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
        if (length(out.color.rgb) < 0.1) {
            out.color = vec4<f32>(10.0, 9.0, 8.0, 1.0);
        }
    } else {
        let lit = apply_pbr_lighting(pbr_input);
        // Ambient starlight illumination floor + soft fresnel atmospheric rim
        let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0);
        let fresnel = pow(1.0 - NdotV, 3.0);
        let ambient_boost = pbr_input.material.base_color.rgb * 0.40;
        
        // Rayleigh atmospheric rim glow only if planet actually has an atmosphere (gas >= 0.01)
        let rim_color = if (gas >= 0.01) {
            vec3<f32>(0.35, 0.75, 1.0) * fresnel * 0.55
        } else {
            pbr_input.material.base_color.rgb * fresnel * 0.18
        };
        
        out.color = vec4<f32>(lit.rgb + ambient_boost + rim_color, 1.0);
    }
    
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
