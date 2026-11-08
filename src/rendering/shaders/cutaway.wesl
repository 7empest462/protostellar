struct CutawayOutput {
    color: vec3<f32>,
    emissive: vec3<f32>,
    is_cut: bool,
}

// 3D Planetary Internal Cutaway: renders internal geological strata cross-section in +X / +Z quadrant.
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

    // Ray-sphere intersection with the 3D incandescent inner core sphere
    var is_inner_core_sphere = false;
    var n_ic = vec3<f32>(0.0, 1.0, 0.0);
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

    var cut_col = vec3<f32>(0.92, 0.42, 0.15); // Silicate Mantle
    var cut_glow = vec3<f32>(0.0);
    let r_trans = core_ratio + (mantle_ratio - core_ratio) * 0.70;

    if (hit_r < inner_core_ratio) {
        // 1. Solid Inner Core: crystalline solid Fe-Ni (T ~ 5,500 K, golden incandescence)
        let rad_grain = sin(atan2(hit_pos.y, hit_pos.x + hit_pos.z) * 12.0) * 0.06 + 0.94;
        cut_col = vec3<f32>(1.0, 0.95, 0.82) * rad_grain;
        if (is_inner_core_sphere) {
            let diff_ic = max(dot(n_ic, vec3<f32>(0.5, 0.7, 0.3)), 0.0);
            cut_col = cut_col * (0.65 + 0.35 * diff_ic);
        }
        cut_glow = vec3<f32>(1.5, 1.25, 0.65) * 2.2;
    } else if (hit_r < core_ratio) {
        // 2. Liquid Outer Core: convective molten Fe-Ni geodynamo (T ~ 4,500 K, amber glow)
        let core_frac = (hit_r - inner_core_ratio) / max(core_ratio - inner_core_ratio, 0.005);
        let swirl = sin(hit_r * 32.0 - time * 1.5 + hit_pos.y * 10.0) * 0.12;
        cut_col = mix(vec3<f32>(1.0, 0.68, 0.15), vec3<f32>(0.82, 0.28, 0.04), clamp(core_frac + swirl, 0.0, 1.0));
        cut_glow = vec3<f32>(1.1, 0.45, 0.08) * 1.2;
    } else if (hit_r < r_trans) {
        // 3. Lower Mantle (Mesosphere): dense bridgmanite / ferropericlase (T ~ 2,800 K)
        let lower_frac = (hit_r - core_ratio) / max(r_trans - core_ratio, 0.005);
        cut_col = mix(vec3<f32>(0.78, 0.28, 0.10), vec3<f32>(0.65, 0.35, 0.18), lower_frac);
        cut_glow = vec3<f32>(0.3, 0.08, 0.0) * (1.0 - lower_frac) * 0.6;

        // Sunken Theia LLSVP Plumes at Core-Mantle Boundary (Gutenberg Discontinuity)
        let cmb_dist = hit_r - core_ratio;
        let blob_pat = sin(atan2(hit_pos.y, hit_pos.x) * 5.0 + hit_pos.z * 15.0);
        if (cmb_dist < 0.08 && blob_pat > 0.05) {
            let blob_w = smoothstep(0.08, 0.0, cmb_dist) * smoothstep(0.05, 0.6, blob_pat);
            cut_col = mix(cut_col, vec3<f32>(0.92, 0.12, 0.18), blob_w * 0.85);
            cut_glow = mix(cut_glow, vec3<f32>(0.8, 0.1, 0.1), blob_w * 0.7);
        }
    } else if (hit_r < mantle_ratio) {
        // 4. Upper Mantle (Asthenosphere): ductile peridotite / olivine rock (T ~ 1,600 K)
        let upper_frac = (hit_r - r_trans) / max(mantle_ratio - r_trans, 0.005);
        cut_col = mix(vec3<f32>(0.65, 0.35, 0.18), vec3<f32>(0.50, 0.40, 0.22), upper_frac);
        cut_glow = vec3<f32>(0.0);
    } else {
        // 5. Rigid Lithospheric Crust & Hydrosphere (Stratified rock layers)
        let crust_depth = (hit_r - mantle_ratio) / max(1.0 - mantle_ratio, 0.005);
        let rock_band = sin(crust_depth * 25.0) * 0.5 + 0.5;
        cut_col = mix(vec3<f32>(0.32, 0.30, 0.28), vec3<f32>(0.58, 0.50, 0.42), rock_band);
        cut_glow = vec3<f32>(0.0);

        // Hydrosphere: surface liquid oceans if present
        if (ocean_frac > 0.05 && hit_r > 0.992) {
            cut_col = vec3<f32>(0.08, 0.38, 0.75);
            cut_glow = vec3<f32>(0.02, 0.12, 0.25);
        }
    }

    // Holographic boundary lines between geological strata
    let d_inner = abs(hit_r - inner_core_ratio);
    let d_core = abs(hit_r - core_ratio);
    let d_trans = abs(hit_r - r_trans);
    let d_mantle = abs(hit_r - mantle_ratio);
    let d_surface = abs(hit_r - 1.0);

    if (d_inner < 0.010) {
        cut_col = vec3<f32>(1.0, 0.92, 0.45);
        cut_glow = vec3<f32>(1.8, 1.5, 0.6) * 1.8;
    } else if (d_core < 0.009) {
        cut_col = vec3<f32>(1.0, 0.55, 0.10);
        cut_glow = vec3<f32>(1.5, 0.6, 0.1) * 1.5;
    } else if (d_trans < 0.008) {
        cut_col = vec3<f32>(1.0, 0.40, 0.25);
        cut_glow = vec3<f32>(0.8, 0.3, 0.15) * 1.2;
    } else if (d_mantle < 0.008) {
        cut_col = vec3<f32>(0.95, 0.65, 0.30);
        cut_glow = vec3<f32>(0.9, 0.4, 0.15) * 1.2;
    } else if (d_surface < 0.007) {
        cut_col = vec3<f32>(0.4, 0.85, 1.0);
        cut_glow = vec3<f32>(0.3, 0.8, 1.0) * 1.6;
    }

    out.color = cut_col;
    out.emissive = cut_glow;
    out.is_cut = true;
    return out;
}
