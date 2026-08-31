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
    climate_and_bio: vec4<f32>, // x: ocean_frac, y: ice_frac, z: biomass_frac, w: cloud_density
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
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let norm = normalize(in.world_normal);
    let n_time = planet.time * 0.05;
    let temp = planet.temperature;
    
    let rock = planet.composition.x;
    let ice = planet.composition.y;
    let metal = planet.composition.z;
    let gas = planet.composition.w;

    let ocean_frac = max(planet.climate_and_bio.x, ice);
    let ice_frac = planet.climate_and_bio.y;
    let biomass = planet.climate_and_bio.z;
    let cloud_density = max(planet.climate_and_bio.w, gas * 0.5);

    var color = pbr_input.material.base_color.rgb;

    // 1. Gas Giant (Jupiter / Saturn / Super-Jupiters)
    if (planet.planet_type == 1u) {
        let lat = norm.y * 14.0;
        let flow = fbm(norm * 6.0 + vec3<f32>(n_time * 0.4, 0.0, -n_time * 0.2));
        let storm = fbm(norm * 18.0 + vec3<f32>(n_time * 0.8, 0.0, 0.0));
        
        let band_val = sin(lat + flow * 2.2) * 0.5 + 0.5;
        let c1 = planet.color_seed.rgb * 0.9;
        let c2 = planet.color_seed.rgb * 1.45 + vec3<f32>(0.15, 0.10, 0.05);
        let c3 = mix(c1, c2, band_val);
        
        // Great Red Spot / White Oval Storms
        let spot_zone = smoothstep(0.70, 0.90, storm) * smoothstep(0.3, -0.3, norm.y);
        let spot_color = vec3<f32>(0.85, 0.35, 0.15);
        
        color = mix(c3, spot_color, spot_zone * 0.8);
    }
    // 2. Ice Giant (Uranus / Neptune)
    else if (planet.planet_type == 2u) {
        let lat = norm.y * 8.0;
        let swirl = fbm(norm * 4.0 + vec3<f32>(n_time * 0.2, 0.0, n_time * 0.1));
        let band = sin(lat + swirl * 1.0) * 0.5 + 0.5;
        
        let deep_cyan = vec3<f32>(0.10, 0.45, 0.78);
        let bright_azure = vec3<f32>(0.35, 0.75, 0.95);
        let methane_veil = mix(deep_cyan, bright_azure, band);
        
        let cirrus = fbm(norm * 12.0 + vec3<f32>(n_time * 0.5, 0.0, 0.0));
        let white_clouds = smoothstep(0.68, 0.85, cirrus);
        
        color = mix(methane_veil, vec3<f32>(0.92, 0.96, 1.0), white_clouds * 0.45);
    }
    // 3. Terrestrial Rocky / Ocean / Biosphere Planet / Protoplanet
    else if (planet.planet_type == 3u || planet.planet_type == 4u) {
        let elev = fbm(norm * 3.8);
        let polar_angle = abs(norm.y);

        // A. Molten Magma Ocean Planet (temp >= 700K)
        if (temp >= 700.0) {
            let crust = fbm(norm * 7.0 + vec3<f32>(n_time * 0.2, 0.0, n_time * 0.2));
            if (elev > 0.42 && crust > 0.35) {
                color = vec3<f32>(0.08, 0.06, 0.06); // solidified dark basalt plates
            } else {
                let lava = (0.55 - elev) * 3.5;
                color = mix(vec3<f32>(0.35, 0.10, 0.05), vec3<f32>(1.0, 0.45, 0.08), clamp(lava, 0.0, 1.0));
                pbr_input.material.emissive = vec4<f32>(1.0, 0.38, 0.06, 1.0) * clamp(lava * 4.0, 0.0, 5.0);
            }
        }
        // B. Superheated Venusian Runaway Greenhouse (temp >= 380K with dense atmosphere)
        else if (temp >= 380.0 && (gas > 0.05 || cloud_density > 0.5)) {
            let lat = norm.y * 6.0;
            let clouds = fbm(norm * 4.5 + vec3<f32>(n_time * 0.5, 0.0, n_time * 0.5));
            let band = sin(lat + clouds * 1.5) * 0.5 + 0.5;
            let sulfur_deck = mix(vec3<f32>(0.86, 0.78, 0.52), vec3<f32>(0.94, 0.88, 0.68), band);
            color = sulfur_deck * (0.92 + clouds * 0.18);
        }
        // C. Frozen Snowball Glacial World (ice_frac >= 0.60 or temp < 255K with water)
        else if (ice_frac >= 0.60 || (temp < 255.0 && ocean_frac > 0.05)) {
            let frost = fbm(norm * 8.0);
            let glaciers = mix(vec3<f32>(0.85, 0.92, 0.99), vec3<f32>(0.50, 0.78, 0.92), elev);
            let pack_ice = vec3<f32>(0.94, 0.97, 1.00);
            color = mix(glaciers, pack_ice, smoothstep(0.3, 0.7, frost));
        }
        // D. Temperate Water-Bearing / Habitable Biosphere World
        else if (ocean_frac >= 0.04 && temp >= 250.0 && temp <= 380.0) {
            let sea_level = clamp(0.40 + ocean_frac * 0.45, 0.42, 0.80);
            let ice_cap_thresh = clamp(0.94 - (ice_frac * 0.50) - (273.0 / max(temp, 150.0)) * 0.05, 0.60, 0.98);
            
            // Polar Ice Caps
            if (polar_angle > ice_cap_thresh) {
                color = vec3<f32>(0.94, 0.97, 1.0); // crisp crystalline polar ice
            }
            // Oceans (liquid water in basins below sea level)
            else if (elev < sea_level) {
                let depth = (sea_level - elev) / max(sea_level, 0.1);
                let deep_ocean = vec3<f32>(0.01, 0.10, 0.42);
                let shallow_lagoon = vec3<f32>(0.05, 0.45, 0.75);
                let coastal_cyan = vec3<f32>(0.12, 0.65, 0.78);
                
                let water_color = mix(shallow_lagoon, deep_ocean, clamp(depth * 1.8, 0.0, 1.0));
                let shore_blend = smoothstep(sea_level - 0.04, sea_level, elev);
                color = mix(water_color, coastal_cyan, shore_blend * 0.65);
            }
            // Continents / Landmasses
            else {
                let rel_elev = elev - sea_level;
                
                // Active Living Biosphere & Photosynthetic Vegetation!
                if (biomass > 0.02) {
                    let bio_noise = fbm(norm * 9.0);
                    let lush_canopy = vec3<f32>(0.12, 0.52, 0.18); // deep emerald rainforest
                    let savanna_meadow = vec3<f32>(0.28, 0.62, 0.22); // temperate grasslands
                    let highland_taiga = vec3<f32>(0.18, 0.42, 0.20);
                    let alpine_peaks = vec3<f32>(0.75, 0.72, 0.70);
                    
                    if (rel_elev > 0.25) {
                        color = mix(highland_taiga, alpine_peaks, (rel_elev - 0.25) * 4.0);
                    } else if (rel_elev > 0.10) {
                        let veg = mix(savanna_meadow, lush_canopy, bio_noise);
                        color = mix(vec3<f32>(0.55, 0.45, 0.30), veg, clamp(biomass * 1.4, 0.0, 1.0));
                    } else {
                        let coastal_veg = mix(lush_canopy, savanna_meadow, bio_noise);
                        color = mix(vec3<f32>(0.72, 0.62, 0.42), coastal_veg, clamp(biomass * 1.5, 0.0, 1.0)); // fertile shores
                    }
                }
                // Pre-biotic / Sterile temperate land
                else {
                    if (rel_elev > 0.22) {
                        color = mix(vec3<f32>(0.45, 0.38, 0.25), vec3<f32>(0.75, 0.72, 0.70), (rel_elev - 0.22) * 4.0); // peaks
                    } else if (rel_elev > 0.10) {
                        color = vec3<f32>(0.55, 0.42, 0.28); // plateaus & deserts
                    } else {
                        color = vec3<f32>(0.68, 0.52, 0.35); // alluvial coastal plains
                    }
                }
            }
            
            // Atmospheric Water-Vapor Clouds
            if (cloud_density > 0.02) {
                let clouds = fbm(norm * 5.5 + vec3<f32>(n_time * 0.45, 0.0, n_time * 0.45));
                let cloud_thresh = 0.54 - cloud_density * 0.12;
                if (clouds > cloud_thresh) {
                    let cloud_alpha = (clouds - cloud_thresh) * 2.5 * clamp(cloud_density * 1.5, 0.2, 1.0);
                    color = mix(color, vec3<f32>(0.96, 0.98, 1.0), clamp(cloud_alpha, 0.0, 0.90));
                }
            }
        }
        // E. Metal-Rich World (e.g. Mercury)
        else if (metal > 0.42) {
            let sheen = fbm(norm * 7.0);
            let craters = fbm(norm * 9.5);
            let dark_graphite = vec3<f32>(0.18, 0.18, 0.20);
            let nickel_iron = vec3<f32>(0.65, 0.62, 0.58);
            color = mix(dark_graphite, nickel_iron, sheen * 0.6 + craters * 0.4);
        }
        // F. Barren Dry Airless Silicate Rock (e.g. Moon, dry Mars)
        else {
            let craters = fbm(norm * 8.0);
            let highlands = fbm(norm * 3.5);
            
            if (temp > 280.0) {
                let lowlands = vec3<f32>(0.42, 0.25, 0.15); // dark basalt maria
                let peaks = vec3<f32>(0.72, 0.48, 0.28);    // terracotta highlands
                color = mix(lowlands, peaks, highlands * 0.7 + craters * 0.3);
            } else {
                let lowlands = vec3<f32>(0.22, 0.22, 0.24); // lunar basalt
                let peaks = vec3<f32>(0.55, 0.54, 0.52);    // cratered anorthosite
                color = mix(lowlands, peaks, highlands * 0.7 + craters * 0.3);
            }
        }
    }

    pbr_input.material.base_color = vec4<f32>(color, 1.0);
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
        
        // Rayleigh atmospheric rim glow
        var rim_color = pbr_input.material.base_color.rgb * fresnel * 0.18;
        if (gas >= 0.01 || cloud_density > 0.1) {
            rim_color = vec3<f32>(0.35, 0.75, 1.0) * fresnel * 0.55;
        }
        
        out.color = vec4<f32>(lit.rgb + ambient_boost + rim_color, 1.0);
    }
    
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
