struct CutawayOutput {
    color: vec3<f32>,
    emissive: vec3<f32>,
    is_cut: bool,
}

// 3D Planetary & Stellar Internal Cutaway: renders archetype-accurate internal strata in +X / +Z quadrant.
fn evaluate_internal_cutaway(
    p_tilted: vec3<f32>,
    v_world: vec3<f32>,
    s_tangent_x: vec3<f32>,
    s_axis: vec3<f32>,
    s_tangent_z: vec3<f32>,
    inner_core_ratio: f32,
    core_ratio: f32,
    mantle_ratio: f32,
    time: f32,
    ocean_frac: f32,
    planet_type: u32,
    temperature: f32,
    has_theia_llsvp: bool,
) -> CutawayOutput {
    var out: CutawayOutput;
    out.color = vec3<f32>(0.0);
    out.emissive = vec3<f32>(0.0);
    out.is_cut = false;

    if (p_tilted.x <= 0.0 || p_tilted.z <= 0.0) {
        return out;
    }

    let v_tilted = vec3<f32>(dot(v_world, s_tangent_x), dot(v_world, s_axis), dot(v_world, s_tangent_z));

    // Intersect ray with the two orthogonal cut planes:
    // Plane 1: Z = 0 (valid for X >= 0)
    // Plane 2: X = 0 (valid for Z >= 0)
    var hit_r = 1.0;
    var hit_dist = 1e6;
    var hit_pos = p_tilted;

    if (abs(v_tilted.z) > 1e-4) {
        let t_z = -p_tilted.z / v_tilted.z;
        if (t_z > 0.0) {
            let p_hit = p_tilted + t_z * v_tilted;
            if (p_hit.x >= -0.01 && length(p_hit) <= 1.02) {
                hit_dist = t_z;
                hit_r = length(p_hit);
                hit_pos = p_hit;
            }
        }
    }

    if (abs(v_tilted.x) > 1e-4) {
        let t_x = -p_tilted.x / v_tilted.x;
        if (t_x > 0.0 && t_x < hit_dist) {
            let p_hit = p_tilted + t_x * v_tilted;
            if (p_hit.z >= -0.01 && length(p_hit) <= 1.02) {
                hit_dist = t_x;
                hit_r = length(p_hit);
                hit_pos = p_hit;
            }
        }
    }

    // Ray-sphere intersection with 3D inner core sphere (skipped for undifferentiated bodies)
    var is_inner_core_sphere = false;
    var n_ic = vec3<f32>(0.0, 1.0, 0.0);
    if (inner_core_ratio > 0.005) {
        let b_ic = dot(p_tilted, v_tilted);
        let c_ic = dot(p_tilted, p_tilted) - inner_core_ratio * inner_core_ratio;
        let disc_ic = b_ic * b_ic - c_ic;
        if (disc_ic > 0.0) {
            let t_ic = -b_ic - sqrt(disc_ic);
            if (t_ic > 0.0 && t_ic < hit_dist) {
                let p_ic = p_tilted + t_ic * v_tilted;
                if (p_ic.x >= -0.005 && p_ic.z >= -0.005) {
                    hit_dist = t_ic;
                    hit_r = inner_core_ratio * 0.96;
                    hit_pos = p_ic;
                    is_inner_core_sphere = true;
                    n_ic = normalize(p_ic);
                }
            }
        }
    }

    var cut_col = vec3<f32>(0.92, 0.42, 0.15);
    var cut_glow = vec3<f32>(0.0);
    let r_trans = core_ratio + (mantle_ratio - core_ratio) * 0.70;
    let is_star_or_quasistar = (planet_type == 0u || planet_type == 7u);
    let is_undifferentiated = (!is_star_or_quasistar && planet_type != 1u && planet_type != 2u && core_ratio <= 0.01);

    if (is_star_or_quasistar) {
        // Stellar / QuasiStar interior cutaway
        let hot_factor = clamp((temperature - 2500.0) / 10000.0, 0.0, 1.0);
        let plasma_swirl = sin(hit_r * 26.0 - time * 2.2 + atan2(hit_pos.y, hit_pos.x + hit_pos.z) * 6.0) * 0.10;
        if (hit_r < inner_core_ratio) {
            if (planet_type == 7u) {
                // Quasi-star central supermassive black hole & glowing inner accretion ring
                var bh_ring = smoothstep(inner_core_ratio * 0.55, inner_core_ratio * 0.92, hit_r);
                if (is_inner_core_sphere) {
                    // Sphere surface: Fresnel-style limb brightening keeps the horizon dark at the centre
                    let fres = pow(clamp(1.0 - abs(dot(n_ic, normalize(vec3<f32>(0.5, 0.7, 0.3)))), 0.0, 1.0), 2.0);
                    bh_ring = max(bh_ring * 0.35, smoothstep(0.55, 0.95, fres));
                }
                cut_col = mix(vec3<f32>(0.02, 0.01, 0.03), vec3<f32>(1.0, 0.65, 0.25), bh_ring);
                cut_glow = vec3<f32>(1.6, 0.55, 0.15) * bh_ring * 2.4;
            } else {
                // Thermonuclear Fusion Core
                cut_col = mix(vec3<f32>(0.95, 0.96, 1.0), vec3<f32>(0.72, 0.88, 1.0), hot_factor);
                if (is_inner_core_sphere) {
                    let diff_ic = max(dot(n_ic, vec3<f32>(0.5, 0.7, 0.3)), 0.0);
                    cut_col *= (0.72 + 0.28 * diff_ic);
                }
                cut_glow = mix(vec3<f32>(1.8, 1.6, 1.1), vec3<f32>(1.2, 1.6, 2.2), hot_factor) * 2.5;
            }
        } else if (hit_r < core_ratio) {
            // Core Burning Shell / Deep Radiative Zone
            let frac = (hit_r - inner_core_ratio) / max(core_ratio - inner_core_ratio, 0.005);
            cut_col = mix(vec3<f32>(1.0, 0.92, 0.60), vec3<f32>(1.0, 0.65, 0.22), clamp(frac + plasma_swirl, 0.0, 1.0));
            cut_glow = mix(vec3<f32>(1.6, 1.2, 0.45), vec3<f32>(1.3, 0.75, 0.15), frac) * 1.8;
        } else if (hit_r < mantle_ratio) {
            // Radiative / Convective Envelope
            let frac = (hit_r - core_ratio) / max(mantle_ratio - core_ratio, 0.005);
            let cell = sin(hit_pos.x * 18.0 + time * 0.8) * cos(hit_pos.y * 18.0 - time * 0.6) * 0.08;
            let base_hot = mix(vec3<f32>(1.0, 0.50, 0.12), vec3<f32>(0.85, 0.28, 0.05), clamp(frac + cell, 0.0, 1.0));
            let blue_env = mix(vec3<f32>(0.55, 0.80, 1.0), vec3<f32>(0.35, 0.62, 0.98), clamp(frac + cell, 0.0, 1.0));
            cut_col = mix(base_hot, blue_env, hot_factor * 0.75);
            cut_glow = cut_col * mix(1.2, 0.65, frac);
        } else {
            // Outer Photospheric Convective Granulation Layer
            let gran = sin(atan2(hit_pos.y, hit_pos.x + hit_pos.z) * 18.0 + time * 1.4) * 0.12;
            let cool_phot = mix(vec3<f32>(0.90, 0.26, 0.06), vec3<f32>(1.0, 0.48, 0.12), gran + 0.5);
            let hot_phot = mix(vec3<f32>(0.62, 0.80, 1.0), vec3<f32>(0.85, 0.92, 1.0), gran + 0.5);
            cut_col = mix(cool_phot, hot_phot, hot_factor * 0.75);
            cut_glow = cut_col * 0.95;
        }
    } else if (planet_type == 1u) {
        // Gas Giant Interior: Silicate Core -> Metallic H Dynamo -> Liquid Molecular H2 -> Ammonia Cloud Deck
        if (hit_r < inner_core_ratio) {
            cut_col = vec3<f32>(0.78, 0.66, 0.36);
            if (is_inner_core_sphere) {
                let diff_ic = max(dot(n_ic, vec3<f32>(0.5, 0.7, 0.3)), 0.0);
                cut_col *= (0.65 + 0.35 * diff_ic);
            }
            cut_glow = vec3<f32>(0.9, 0.55, 0.18) * 1.2;
        } else if (hit_r < core_ratio) {
            let frac = (hit_r - inner_core_ratio) / max(core_ratio - inner_core_ratio, 0.005);
            let shimmer = sin(hit_r * 40.0 - time * 1.6 + hit_pos.y * 12.0) * 0.08;
            cut_col = mix(vec3<f32>(0.55, 0.85, 1.0), vec3<f32>(0.30, 0.68, 0.96), clamp(frac + shimmer, 0.0, 1.0));
            cut_glow = vec3<f32>(0.25, 0.65, 1.0) * 0.95;
        } else if (hit_r < mantle_ratio) {
            let frac = (hit_r - core_ratio) / max(mantle_ratio - core_ratio, 0.005);
            cut_col = mix(vec3<f32>(0.22, 0.56, 0.86), vec3<f32>(0.16, 0.36, 0.66), frac);
            cut_glow = vec3<f32>(0.04, 0.12, 0.25);
        } else {
            let deck = sin((hit_r - mantle_ratio) * 32.0 + hit_pos.y * 8.0) * 0.5 + 0.5;
            cut_col = mix(vec3<f32>(0.76, 0.52, 0.28), vec3<f32>(0.94, 0.78, 0.52), deck);
        }
    } else if (planet_type == 2u) {
        // Ice Giant Interior: Silicate Core -> Superionic Ice -> Fluid H2O-NH3-CH4 Mantle -> H2-CH4 Atmosphere
        if (hit_r < inner_core_ratio) {
            cut_col = vec3<f32>(0.64, 0.54, 0.36);
            if (is_inner_core_sphere) {
                let diff_ic = max(dot(n_ic, vec3<f32>(0.5, 0.7, 0.3)), 0.0);
                cut_col *= (0.65 + 0.35 * diff_ic);
            }
            cut_glow = vec3<f32>(0.55, 0.38, 0.15) * 0.7;
        } else if (hit_r < core_ratio) {
            let frac = (hit_r - inner_core_ratio) / max(core_ratio - inner_core_ratio, 0.005);
            cut_col = mix(vec3<f32>(0.15, 0.88, 0.95), vec3<f32>(0.10, 0.72, 0.88), frac);
            cut_glow = vec3<f32>(0.12, 0.62, 0.85) * 0.65;
        } else if (hit_r < mantle_ratio) {
            let frac = (hit_r - core_ratio) / max(mantle_ratio - core_ratio, 0.005);
            cut_col = mix(vec3<f32>(0.12, 0.66, 0.82), vec3<f32>(0.18, 0.48, 0.78), frac);
        } else {
            let haze = sin((hit_r - mantle_ratio) * 28.0) * 0.5 + 0.5;
            cut_col = mix(vec3<f32>(0.28, 0.58, 0.92), vec3<f32>(0.52, 0.78, 0.98), haze);
        }
    } else if (is_undifferentiated) {
        // Undifferentiated Primitive Body: homogeneous chondritic rock / volatile ice rubble without molten core
        let chondrule = sin(hit_pos.x * 38.0 + hit_pos.z * 29.0) * cos(hit_pos.y * 41.0);
        let grain = clamp(chondrule * 0.5 + 0.5, 0.0, 1.0);
        let icy_mix = select(0.0, 0.45, ocean_frac > 0.05 || temperature < 175.0);
        let chondrite = mix(vec3<f32>(0.26, 0.24, 0.22), vec3<f32>(0.46, 0.41, 0.35), grain);
        let dirty_ice = mix(vec3<f32>(0.42, 0.50, 0.56), vec3<f32>(0.68, 0.76, 0.82), grain);
        cut_col = mix(chondrite, dirty_ice, icy_mix);
        if (hit_r > 0.94) {
            cut_col *= 0.84; // Impact-gardened regolith rind
        }
    } else if (hit_r < inner_core_ratio) {
        // 1. Solid Inner Core: crystalline solid Fe-Ni
        let rad_grain = sin(atan2(hit_pos.y, hit_pos.x + hit_pos.z) * 12.0) * 0.06 + 0.94;
        cut_col = vec3<f32>(1.0, 0.95, 0.82) * rad_grain;
        if (is_inner_core_sphere) {
            let diff_ic = max(dot(n_ic, vec3<f32>(0.5, 0.7, 0.3)), 0.0);
            cut_col = cut_col * (0.65 + 0.35 * diff_ic);
        }
        cut_glow = vec3<f32>(1.5, 1.25, 0.65) * 2.2;
    } else if (hit_r < core_ratio) {
        // 2. Liquid Outer Core: convective molten Fe-Ni geodynamo
        let core_frac = (hit_r - inner_core_ratio) / max(core_ratio - inner_core_ratio, 0.005);
        let swirl = sin(hit_r * 32.0 - time * 1.5 + hit_pos.y * 10.0) * 0.12;
        cut_col = mix(vec3<f32>(1.0, 0.68, 0.15), vec3<f32>(0.82, 0.28, 0.04), clamp(core_frac + swirl, 0.0, 1.0));
        cut_glow = vec3<f32>(1.1, 0.45, 0.08) * 1.2;
    } else if (hit_r < r_trans) {
        // 3. Lower Mantle (Mesosphere): dense bridgmanite / ferropericlase
        let lower_frac = (hit_r - core_ratio) / max(r_trans - core_ratio, 0.005);
        cut_col = mix(vec3<f32>(0.78, 0.28, 0.10), vec3<f32>(0.65, 0.35, 0.18), lower_frac);
        cut_glow = vec3<f32>(0.3, 0.08, 0.0) * (1.0 - lower_frac) * 0.6;

        // Sunken Theia LLSVP Plumes at Core-Mantle Boundary (only when body actually carries LLSVP remnants)
        if (has_theia_llsvp) {
            let cmb_dist = hit_r - core_ratio;
            let blob_pat = sin(atan2(hit_pos.y, hit_pos.x) * 5.0 + hit_pos.z * 15.0);
            if (cmb_dist < 0.08 && blob_pat > 0.05) {
                let blob_w = smoothstep(0.08, 0.0, cmb_dist) * smoothstep(0.05, 0.6, blob_pat);
                cut_col = mix(cut_col, vec3<f32>(0.92, 0.12, 0.18), blob_w * 0.85);
                cut_glow = mix(cut_glow, vec3<f32>(0.8, 0.1, 0.1), blob_w * 0.7);
            }
        }
    } else if (hit_r < mantle_ratio) {
        // 4. Upper Mantle (Asthenosphere): ductile peridotite / olivine rock
        let upper_frac = (hit_r - r_trans) / max(mantle_ratio - r_trans, 0.005);
        cut_col = mix(vec3<f32>(0.65, 0.35, 0.18), vec3<f32>(0.50, 0.40, 0.22), upper_frac);
    } else {
        // 5. Rigid Lithospheric Crust & Hydrosphere
        let crust_depth = (hit_r - mantle_ratio) / max(1.0 - mantle_ratio, 0.005);
        let rock_band = sin(crust_depth * 25.0) * 0.5 + 0.5;
        cut_col = mix(vec3<f32>(0.32, 0.30, 0.28), vec3<f32>(0.58, 0.50, 0.42), rock_band);

        if (ocean_frac > 0.05 && hit_r > 0.992) {
            cut_col = vec3<f32>(0.08, 0.38, 0.75);
            cut_glow = vec3<f32>(0.02, 0.12, 0.25);
        }
    }

    // Holographic boundary lines between geological/stellar strata
    let d_inner = abs(hit_r - inner_core_ratio);
    let d_core = abs(hit_r - core_ratio);
    let d_trans = abs(hit_r - r_trans);
    let d_mantle = abs(hit_r - mantle_ratio);
    let d_surface = abs(hit_r - 1.0);

    if (!is_undifferentiated && inner_core_ratio > 0.005 && d_inner < 0.010) {
        cut_col = select(vec3<f32>(1.0, 0.92, 0.45), vec3<f32>(0.80, 0.95, 1.0), is_star_or_quasistar);
        cut_glow = select(vec3<f32>(1.8, 1.5, 0.6), vec3<f32>(1.4, 1.8, 2.2), is_star_or_quasistar) * 1.6;
    } else if (!is_undifferentiated && d_core < 0.009) {
        cut_col = select(vec3<f32>(1.0, 0.55, 0.10), vec3<f32>(0.45, 0.88, 1.0), planet_type == 1u || planet_type == 2u);
        cut_glow = cut_col * 1.4;
    } else if (!is_undifferentiated && !is_star_or_quasistar && planet_type != 1u && planet_type != 2u && d_trans < 0.008) {
        cut_col = vec3<f32>(1.0, 0.40, 0.25);
        cut_glow = vec3<f32>(0.8, 0.3, 0.15) * 1.2;
    } else if (!is_undifferentiated && d_mantle < 0.008) {
        cut_col = select(vec3<f32>(0.95, 0.65, 0.30), vec3<f32>(0.55, 0.88, 1.0), planet_type == 1u || planet_type == 2u);
        cut_glow = cut_col * 1.1;
    } else if (d_surface < 0.007) {
        cut_col = vec3<f32>(0.4, 0.85, 1.0);
        cut_glow = vec3<f32>(0.3, 0.8, 1.0) * 1.5;
    }

    out.color = cut_col;
    out.emissive = cut_glow;
    out.is_cut = true;
    return out;
}
