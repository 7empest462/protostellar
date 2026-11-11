import protostellar::rendering::shaders::noise::{noise, fbm, ridge_noise, voronoi3};

struct CraterShadingResult {
    color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    emissive: vec3<f32>,
    normal_delta: vec3<f32>,
}

// Spherical Gaussian harmonic mask for recognizable modern Earth continents & ocean basins
fn evaluate_earth_continental_harmonics(p_surf: vec3<f32>) -> f32 {
    let p = normalize(p_surf);
    var h = 0.24;

    // 1. Africa & Arabian Peninsula
    let d_af_n = length(p - vec3<f32>(0.894, 0.342, 0.290));
    let d_af_s = length(p - vec3<f32>(0.864, -0.208, 0.459));
    let d_arab = length(p - vec3<f32>(0.646, 0.407, 0.646));
    h += 0.38 * exp(-d_af_n * d_af_n / 0.12) + 0.36 * exp(-d_af_s * d_af_s / 0.085) + 0.28 * exp(-d_arab * d_arab / 0.035);

    // 2. Eurasia & Tibetan Plateau / Indian Subcontinent (Himalayas)
    let d_eur_w = length(p - vec3<f32>(0.646, 0.743, 0.173));
    let d_eur_c = length(p - vec3<f32>(0.050, 0.819, 0.571));
    let d_asia_e = length(p - vec3<f32>(-0.311, 0.559, 0.769));
    let d_tibet = length(p - vec3<f32>(0.123, 0.469, 0.874));
    h += 0.32 * exp(-d_eur_w * d_eur_w / 0.065) + 0.40 * exp(-d_eur_c * d_eur_c / 0.20)
        + 0.35 * exp(-d_asia_e * d_asia_e / 0.09) + 0.46 * exp(-d_tibet * d_tibet / 0.045);

    // 3. North America, Western Cordillera & Greenland
    let d_na_c = length(p - vec3<f32>(-0.125, 0.695, -0.708));
    let d_rockies = length(p - vec3<f32>(-0.302, 0.669, -0.679));
    let d_grn = length(p - vec3<f32>(0.230, 0.951, -0.207));
    h += 0.38 * exp(-d_na_c * d_na_c / 0.13) + 0.24 * exp(-d_rockies * d_rockies / 0.035) + 0.34 * exp(-d_grn * d_grn / 0.030);

    // 4. South America & Andean Cordillera
    let d_sa_n = length(p - vec3<f32>(0.547, -0.208, -0.811));
    let d_sa_s = length(p - vec3<f32>(0.311, -0.559, -0.769));
    h += 0.38 * exp(-d_sa_n * d_sa_n / 0.085) + 0.34 * exp(-d_sa_s * d_sa_s / 0.045);

    // 5. Australia & Antarctica
    let d_aus = length(p - vec3<f32>(-0.630, -0.423, 0.652));
    let ant_cap = smoothstep(-0.68, -0.88, p.y);
    h += 0.36 * exp(-d_aus * d_aus / 0.050) + ant_cap * 0.44;

    // 6. Deep Oceanic Basins (Pacific, Atlantic S-Trench, Indian, Arctic)
    let d_pac = length(p - vec3<f32>(-0.966, 0.0, -0.259));
    let d_atl = length(p - vec3<f32>(0.837, 0.259, -0.483));
    let d_ind = length(p - vec3<f32>(0.240, -0.375, 0.896));
    let arc_basin = smoothstep(0.84, 0.96, p.y);
    h -= 0.28 * exp(-d_pac * d_pac / 0.55) + 0.22 * exp(-d_atl * d_atl / 0.14)
        + 0.24 * exp(-d_ind * d_ind / 0.18) + arc_basin * 0.24;

    return clamp(h, 0.0, 1.0);
}

// Topographic elevation for canonical Solar System planets & moons
fn evaluate_canonical_terrestrial_elevation(p_surf: vec3<f32>, sub: f32) -> f32 {
    let p = normalize(p_surf);
    if (sub == 1.0) {
        // MARS (MOLA Global Hypsometry)
        let lon = atan2(p.z, p.x);
        let dichotomy = -0.18 * smoothstep(-0.14, 0.36, p.y + 0.12 * cos(lon - 0.6));
        let d_tharsis = length(p - vec3<f32>(0.76, 0.05, -0.65));
        let tharsis_bulge = 0.20 * exp(-d_tharsis * d_tharsis / 0.20);
        let d_olympus = length(p - vec3<f32>(0.64, 0.31, -0.70));
        let olympus = 0.36 * smoothstep(0.135, 0.024, d_olympus) + 0.07 * smoothstep(0.14, 0.12, d_olympus)
            - 0.10 * smoothstep(0.026, 0.009, d_olympus);
        let d_asc = length(p - vec3<f32>(0.78, 0.19, -0.59));
        let d_pav = length(p - vec3<f32>(0.80, 0.02, -0.60));
        let d_ars = length(p - vec3<f32>(0.79, -0.15, -0.59));
        let d_ely = length(p - vec3<f32>(-0.68, 0.41, 0.61));
        let montes = 0.22 * (smoothstep(0.085, 0.016, d_asc) + smoothstep(0.085, 0.016, d_pav) + smoothstep(0.090, 0.018, d_ars))
            - 0.06 * (smoothstep(0.018, 0.006, d_asc) + smoothstep(0.018, 0.006, d_pav) + smoothstep(0.020, 0.007, d_ars))
            + 0.18 * smoothstep(0.09, 0.018, d_ely);
        let vm_center = vec3<f32>(0.89, -0.11, -0.44);
        let vm_along = dot(p - vm_center, vec3<f32>(0.42, -0.06, 0.90));
        let vm_cross_coord = dot(p - vm_center, vec3<f32>(0.10, 0.99, 0.04));
        let vm_organic_warp = (fbm(p * 12.0) - 0.5) * 0.020;
        let vm_across = abs(vm_cross_coord + vm_organic_warp);
        let canyon_reach = smoothstep(0.30, 0.16, abs(vm_along));
        let canyon_width = 0.040 + (fbm(p * 20.0) - 0.5) * 0.015;
        let valles_marineris = -0.22 * canyon_reach * smoothstep(canyon_width, canyon_width * 0.25, vm_across);
        let d_hellas = length(p - vec3<f32>(-0.56, -0.67, 0.48));
        let hellas = -0.24 * smoothstep(0.32, 0.12, d_hellas) + 0.07 * (smoothstep(0.38, 0.30, d_hellas) - smoothstep(0.30, 0.22, d_hellas));
        let d_argyre = length(p - vec3<f32>(0.55, -0.76, -0.34));
        let argyre = -0.15 * smoothstep(0.18, 0.06, d_argyre);
        let rugged_crust = fbm(p * 6.5) * 0.18 + ridge_noise(p * 14.0) * 0.10;
        return 0.42 + dichotomy + tharsis_bulge + olympus + montes + valles_marineris + hellas + argyre + rugged_crust;
    } else if (sub == 2.0) {
        // THE MOON / LUNA (LRO LOLA Topography: Irregular Basaltic Maria, Ringed Crater Bowls & Highlands)
        let warp_vec = vec3<f32>(
            fbm(p * 3.6 + vec3<f32>(1.1, -0.7, 0.4)) - 0.5,
            fbm(p * 3.6 + vec3<f32>(-0.9, 1.4, -0.6)) - 0.5,
            fbm(p * 3.6 + vec3<f32>(0.5, -1.2, 1.7)) - 0.5
        ) * 0.22;
        let pw = normalize(p + warp_vec);
        let d_imb = length(pw - vec3<f32>(0.76, 0.54, -0.36));
        let d_proc = length(pw - vec3<f32>(0.62, 0.18, -0.76));
        let d_ser = length(pw - vec3<f32>(0.82, 0.45, 0.35));
        let d_tran = length(pw - vec3<f32>(0.83, 0.15, 0.54));
        let d_cris = length(pw - vec3<f32>(0.48, 0.29, 0.83));
        let d_nub = length(pw - vec3<f32>(0.76, -0.38, -0.52));
        let in_maria = clamp(
            smoothstep(0.28, 0.10, d_imb) + smoothstep(0.44, 0.14, d_proc)
            + smoothstep(0.21, 0.07, d_ser) + smoothstep(0.23, 0.08, d_tran)
            + smoothstep(0.16, 0.05, d_cris) + smoothstep(0.25, 0.08, d_nub),
            0.0, 1.0
        );
        let maria_dep = -0.13 * in_maria;
        let apennine_rim = 0.08 * (smoothstep(0.32, 0.25, d_imb) - smoothstep(0.25, 0.18, d_imb)) * (0.6 + 0.4 * ridge_noise(p * 24.0));
        let d_spa = length(pw - vec3<f32>(-0.42, -0.82, -0.38));
        let spa_basin = -0.16 * smoothstep(0.48, 0.16, d_spa);
        let farside_bulge = 0.10 * smoothstep(-0.05, -0.75, p.x);

        let v_major = voronoi3(p * 6.5, 0.0);
        let c_major_bowl = -0.075 * smoothstep(0.22, 0.06, v_major.x);
        let c_major_rim = 0.045 * exp(-pow((v_major.x - 0.23) / 0.055, 2.0));
        let v_med = voronoi3(p * 15.0 + vec3<f32>(2.3, -1.7, 0.9), 0.0);
        let c_med_bowl = -0.042 * smoothstep(0.20, 0.05, v_med.x);
        let c_med_rim = 0.026 * exp(-pow((v_med.x - 0.21) / 0.05, 2.0));
        let crater_field = (c_major_bowl + c_major_rim + c_med_bowl + c_med_rim) * mix(1.0, 0.22, in_maria);

        let d_tycho = length(p - vec3<f32>(0.71, -0.68, -0.18));
        let tycho_elev = -0.055 * smoothstep(0.032, 0.010, d_tycho)
            + 0.035 * exp(-pow((d_tycho - 0.036) / 0.010, 2.0))
            + 0.022 * smoothstep(0.008, 0.0, d_tycho);
        let d_cop = length(p - vec3<f32>(0.92, 0.17, -0.34));
        let cop_elev = -0.050 * smoothstep(0.034, 0.011, d_cop)
            + 0.032 * exp(-pow((d_cop - 0.038) / 0.011, 2.0))
            + 0.020 * smoothstep(0.009, 0.0, d_cop);

        return 0.44 + maria_dep + apennine_rim + spa_basin + farside_bulge + crater_field + tycho_elev + cop_elev + fbm(p * 9.0) * 0.11;
    } else if (sub == 3.0) {
        // MERCURY (MESSENGER MLA: Caloris Basin, Rembrandt, Multi-Scale Craters & Lobate Contraction Scarps)
        let pw = normalize(p + (vec3<f32>(fbm(p * 4.0), fbm(p * 4.0 + 2.1), fbm(p * 4.0 - 1.5)) - 0.5) * 0.16);
        let d_caloris = length(pw - vec3<f32>(-0.82, 0.50, 0.28));
        let caloris = -0.13 * smoothstep(0.28, 0.10, d_caloris)
            + 0.08 * (smoothstep(0.34, 0.27, d_caloris) - smoothstep(0.27, 0.20, d_caloris));
        let d_remb = length(pw - vec3<f32>(-0.12, -0.54, 0.83));
        let rembrandt = -0.11 * smoothstep(0.18, 0.06, d_remb);
        let v_merc = voronoi3(p * 11.5, 0.0);
        let merc_craters = -0.055 * smoothstep(0.21, 0.05, v_merc.x) + 0.032 * exp(-pow((v_merc.x - 0.22) / 0.05, 2.0));
        let lobate_scarps = smoothstep(0.60, 0.82, ridge_noise(p * 7.5 + vec3<f32>(fbm(p * 4.0) * 0.4))) * 0.10;
        return 0.42 + caloris + rembrandt + merc_craters + lobate_scarps + fbm(p * 9.0) * 0.11;
    } else if (sub == 5.0) {
        // VENUS (Magellan Radar: Ishtar Terra / Maxwell Montes & Aphrodite Terra)
        let d_ishtar = length(p - vec3<f32>(0.32, 0.90, 0.29));
        let ishtar = 0.28 * smoothstep(0.32, 0.08, d_ishtar) * (0.65 + 0.35 * ridge_noise(p * 18.0));
        let d_aphro = length(p - vec3<f32>(-0.65, -0.12, 0.75));
        let aphrodite = 0.18 * smoothstep(0.44, 0.12, d_aphro);
        return 0.40 + ishtar + aphrodite + fbm(p * 6.0) * 0.16 + ridge_noise(p * 12.0) * 0.10;
    } else if (sub == 6.0) {
        // GANYMEDE (Grooved Tectonic Sulci vs Dark Cratered Regiones)
        let regio_mask = smoothstep(0.42, 0.58, fbm(p * 2.8 + vec3<f32>(0.4, 1.1, -0.7)));
        let sulci_grooves = ridge_noise(p * 28.0) * 0.055 * (1.0 - regio_mask);
        return 0.42 + sulci_grooves + fbm(p * 12.0) * 0.08;
    } else if (sub == 7.0) {
        // CALLISTO (Valhalla & Asgard Concentric Multi-Ring Palimpsests)
        let d_val = length(p - vec3<f32>(0.78, 0.26, -0.57));
        let val_rings = sin(d_val * 58.0) * exp(-d_val * 4.5) * 0.045 * smoothstep(0.45, 0.04, d_val);
        let d_asg = length(p - vec3<f32>(-0.52, 0.52, -0.68));
        let asg_rings = sin(d_asg * 65.0) * exp(-d_asg * 5.8) * 0.035 * smoothstep(0.30, 0.03, d_asg);
        return 0.42 + val_rings + asg_rings + (fbm(p * 14.0) - 0.5) * 0.09;
    } else if (sub == 8.0) {
        // TITAN (North Polar Kraken/Ligeia Mare Basins, Xanadu Plateau, Equatorial Dunes)
        let lake_noise = fbm(p * 5.2 + vec3<f32>(1.3, 0.0, -0.8));
        let in_polar_mare = p.y > 0.66 && lake_noise < 0.47;
        if (in_polar_mare) {
            return -0.05;
        }
        let d_xanadu = length(p - vec3<f32>(-0.42, -0.18, -0.89));
        let xanadu = 0.10 * smoothstep(0.34, 0.10, d_xanadu) * (0.6 + 0.4 * ridge_noise(p * 16.0));
        return 0.04 + xanadu + fbm(p * 8.0) * 0.06;
    }
    return 0.0;
}

fn compute_crater_radial_slope(u: f32, radius: f32) -> f32 {
    let inner_wall = smoothstep(0.68, 0.90, u) * smoothstep(0.99, 0.90, u);
    let outer_flank = smoothstep(0.97, 1.06, u) * smoothstep(1.32, 1.06, u);
    var slope = -inner_wall * 0.85 + outer_flank * 0.48;

    if (radius > 0.10 && u < 0.24) {
        let peak_flank = smoothstep(0.02, 0.11, u) * smoothstep(0.24, 0.11, u);
        slope += peak_flank * 0.65;
    }
    if (radius > 0.18 && u > 0.42 && u < 0.64) {
        let ring_in = smoothstep(0.42, 0.51, u) * smoothstep(0.54, 0.51, u);
        let ring_out = smoothstep(0.52, 0.57, u) * smoothstep(0.64, 0.57, u);
        slope += -ring_in * 0.45 + ring_out * 0.30;
    }
    return slope;
}

fn apply_gas_giant_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    seismic_phase: f32,
    time: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    let tangent = normalize(cross(center, vec3<f32>(0.0, 1.0, 0.001)));
    let d_east = dot(p_surf - center, tangent);
    let d_north = length((p_surf - center) - tangent * d_east);
    let sheared_dist = sqrt((d_east / 1.7) * (d_east / 1.7) + d_north * d_north);
    let u = sheared_dist / radius;

    if (u < 2.2) {
        let turb = fbm(p_surf * 22.0 + vec3<f32>(time * 0.06, 0.0, 0.0));
        let soot_mask = smoothstep(0.70, 0.18, u) * (0.80 + turb * 0.20) * effective_scar;
        let soot_color = vec3<f32>(0.04, 0.025, 0.02);
        let halo_mask = smoothstep(0.75, 1.0, u) * smoothstep(1.45, 1.0, u) * (0.45 + turb * 0.25) * effective_scar;
        let halo_color = vec3<f32>(0.12, 0.08, 0.06);

        out_res.color = mix(out_res.color, halo_color, halo_mask * 0.70);
        out_res.color = mix(out_res.color, soot_color, soot_mask * 0.95);
        out_res.roughness = mix(out_res.roughness, 0.75, soot_mask * 0.5);

        let active_melt = melt_glow * effective_scar;
        if (active_melt > 0.05) {
            let acoustic_u = 0.8 + seismic_phase * 1.35;
            let acoustic_ring = exp(-pow((u - acoustic_u) / 0.14, 2.0)) * active_melt * (1.0 - seismic_phase * 0.7);
            out_res.emissive += vec3<f32>(1.15, 0.55, 0.15) * acoustic_ring * 2.2;
        }

        if (active_melt > 0.005 && u < 0.85) {
            let fireball = smoothstep(0.85, 0.15, u) * active_melt;
            let flash_core = vec3<f32>(1.0, 0.65, 0.18) * (4.2 + sin(time * 3.0 + u * 6.0) * 0.6);
            out_res.color = mix(out_res.color, vec3<f32>(1.0, 0.85, 0.5), fireball * 0.65);
            out_res.emissive += flash_core * fireball;
        }
    }
    return out_res;
}

fn apply_ice_giant_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    seismic_phase: f32,
    time: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    let tangent = normalize(cross(center, vec3<f32>(0.0, 1.0, 0.001)));
    let d_east = dot(p_surf - center, tangent);
    let d_north = length((p_surf - center) - tangent * d_east);
    let sheared_dist = sqrt((d_east / 1.55) * (d_east / 1.55) + d_north * d_north);
    let u = sheared_dist / radius;

    if (u < 2.1) {
        let cirrus_turb = fbm(p_surf * 26.0 + vec3<f32>(time * 0.05, 0.0, 0.0));
        let vortex_mask = smoothstep(0.55, 0.15, u) * effective_scar;
        let vortex_color = vec3<f32>(0.015, 0.06, 0.18);
        let cirrus_mask = smoothstep(0.40, 0.75, u) * smoothstep(1.40, 0.85, u)
            * (0.65 + cirrus_turb * 0.35) * effective_scar;
        let cirrus_color = vec3<f32>(0.92, 0.98, 1.0);

        out_res.color = mix(out_res.color, vortex_color, vortex_mask * 0.90);
        out_res.color = mix(out_res.color, cirrus_color, cirrus_mask * 0.85);

        let active_melt = melt_glow * effective_scar;
        if (active_melt > 0.05) {
            let wave_u = 0.75 + seismic_phase * 1.25;
            let shock_ring = exp(-pow((u - wave_u) / 0.13, 2.0)) * active_melt * (1.0 - seismic_phase * 0.7);
            out_res.emissive += vec3<f32>(0.35, 0.88, 1.15) * shock_ring * 2.0;
        }

        if (active_melt > 0.005 && u < 0.85) {
            let shock_flare = smoothstep(0.85, 0.10, u) * active_melt;
            let flare_core = vec3<f32>(0.35, 0.85, 1.0) * (3.8 + sin(time * 3.2 + u * 8.0) * 0.6);
            out_res.color = mix(out_res.color, vec3<f32>(0.65, 0.92, 1.0), shock_flare * 0.60);
            out_res.emissive += flare_core * shock_flare;
        }
    }
    return out_res;
}

fn apply_icy_world_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radial_dir: vec3<f32>,
    dist: f32,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    time: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    let u = dist / radius;

    if (u < 2.3) {
        let up_ref = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(center.y) > 0.9);
        let t1 = normalize(cross(up_ref, center));
        let t2 = cross(center, t1);
        let local_u = dot(p_surf - center, t1);
        let local_v = dot(p_surf - center, t2);
        let ray_angle = atan2(local_v, local_u);
        let radial_fade = smoothstep(0.06, 0.32, u);

        let warp_n1 = fbm(p_surf * 18.0);
        let warp_n2 = fbm(p_surf * 40.0);
        let rim_warp = (warp_n1 - 0.5) * 0.14 + (warp_n2 - 0.5) * 0.06;
        let u_w = max(0.0, u + rim_warp * smoothstep(0.05, 0.65, u));

        let ray_mod1 = sin(ray_angle * 11.0 + warp_n1 * 5.0) * 0.5 + 0.5;
        let ray_mod2 = sin(ray_angle * 23.0 - warp_n2 * 6.0) * 0.5 + 0.5;
        let ray_noise = pow(ray_mod1 * 0.65 + ray_mod2 * 0.35, 1.6) * radial_fade;

        let ejecta_ice = smoothstep(2.2, 0.92, u_w) * smoothstep(0.72, 1.05, u_w) * ray_noise * 0.68 * effective_scar;
        let pure_ice_ray = vec3<f32>(0.90, 0.95, 1.0);
        let rim_factor = smoothstep(0.76, 0.98, u_w) * smoothstep(1.22, 0.98, u_w) * effective_scar;
        let rim_ice = vec3<f32>(0.82, 0.88, 0.96);
        let floor_factor = smoothstep(0.86, 0.68, u_w) * effective_scar;
        let cryo_floor = vec3<f32>(0.20, 0.23, 0.31);

        var central_peak = 0.0;
        if (radius > 0.08 && u_w < 0.22) {
            central_peak = smoothstep(0.22, 0.04, u_w) * 0.78 * effective_scar;
        }

        var crater_col = mix(out_res.color, pure_ice_ray, ejecta_ice);
        crater_col = mix(crater_col, rim_ice, rim_factor);
        crater_col = mix(crater_col, cryo_floor, floor_factor);
        crater_col = mix(crater_col, pure_ice_ray, central_peak);

        out_res.color = crater_col;
        out_res.roughness = mix(out_res.roughness, 0.32, (rim_factor * 0.6 + ejecta_ice * 0.8));
        out_res.metallic = mix(out_res.metallic, 0.02, floor_factor * 0.5);

        let slope = compute_crater_radial_slope(u_w, radius);
        out_res.normal_delta += radial_dir * (slope * effective_scar * 0.62);

        let active_melt = melt_glow * effective_scar;
        if (active_melt > 0.005 && u_w < 0.86) {
            let pool_mask = smoothstep(0.86, 0.55, u_w);
            let core_melt = smoothstep(0.36, 0.06, u_w);
            let ridge_cracks = smoothstep(0.70, 0.91, ridge_noise(p_surf * 30.0 + radial_dir * 1.2));
            let ring_cracks = smoothstep(0.80, 0.95, sin(u_w * 20.0 + warp_n1 * 4.0) * 0.5 + 0.5) * smoothstep(0.25, 0.82, u_w);
            let is_open_slush = clamp(core_melt + max(ridge_cracks, ring_cracks * 0.75) * pool_mask, 0.0, 1.0);

            let cryo_core = vec3<f32>(0.28, 0.78, 1.0) * (3.2 + sin(time * 2.6 + u_w * 8.0) * 0.4)
                + vec3<f32>(0.85, 0.96, 1.15) * smoothstep(0.22, 0.0, u_w) * smoothstep(0.5, 1.0, active_melt);
            let frozen_slush = vec3<f32>(0.26, 0.38, 0.50);
            let slush_mix = mix(frozen_slush, cryo_core, is_open_slush);

            out_res.color = mix(out_res.color, slush_mix, pool_mask * active_melt);
            out_res.emissive += cryo_core * active_melt * (is_open_slush * 0.92 + pool_mask * 0.10);
            out_res.roughness = mix(out_res.roughness, 0.16, pool_mask * active_melt);
        }

        if (active_melt > 0.05 && u_w >= 0.78 && u_w < 2.05) {
            let halo_decay = exp(-2.6 * (u_w - 0.78));
            let halo_heat = active_melt * halo_decay * effective_scar;
            let halo_fissure = smoothstep(0.66, 0.90, ridge_noise(p_surf * 26.0));
            let cryo_halo = vec3<f32>(0.22, 0.72, 0.98) * halo_fissure * halo_heat * 2.5;
            out_res.emissive += cryo_halo;
            out_res.color = mix(out_res.color, vec3<f32>(0.45, 0.65, 0.82), halo_heat * halo_fissure * 0.55);
        }
    }
    return out_res;
}

fn apply_rocky_world_impact_basin(
    p_surf: vec3<f32>,
    center: vec3<f32>,
    radial_dir: vec3<f32>,
    dist: f32,
    radius: f32,
    effective_scar: f32,
    melt_glow: f32,
    time: f32,
    water_mask: f32,
    in_res: CraterShadingResult,
) -> CraterShadingResult {
    var out_res = in_res;
    let u = dist / radius;

    if (u < 2.45) {
        let up_ref = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(center.y) > 0.9);
        let t1 = normalize(cross(up_ref, center));
        let t2 = cross(center, t1);
        let local_u = dot(p_surf - center, t1);
        let local_v = dot(p_surf - center, t2);
        let ray_angle = atan2(local_v, local_u);
        let radial_fade = smoothstep(0.06, 0.34, u);

        let warp_n1 = fbm(p_surf * 18.0);
        let warp_n2 = fbm(p_surf * 42.0);
        let rim_warp = (warp_n1 - 0.5) * 0.14 + (warp_n2 - 0.5) * 0.06;
        let u_w = max(0.0, u + rim_warp * smoothstep(0.05, 0.65, u));

        let ray_h1 = sin(ray_angle * 9.0 + warp_n1 * 5.2) * 0.5 + 0.5;
        let ray_h2 = sin(ray_angle * 21.0 - warp_n2 * 6.4) * 0.5 + 0.5;
        let ray_noise = pow(ray_h1 * 0.62 + ray_h2 * 0.38, 1.75) * radial_fade;

        let dry_factor = select(1.0, 0.0, water_mask > 0.0);

        let ejecta_blanket = smoothstep(2.35, 0.96, u_w) * smoothstep(0.75, 1.06, u_w) * ray_noise * 0.54 * effective_scar * dry_factor;
        let ejecta_dust = vec3<f32>(0.50, 0.46, 0.42) * (0.85 + warp_n2 * 0.30);

        let rim_factor = smoothstep(0.74, 0.98, u_w) * smoothstep(1.22, 0.98, u_w) * effective_scar * dry_factor;
        let rim_rock = vec3<f32>(0.36, 0.33, 0.30) * (0.82 + warp_n2 * 0.36);

        let floor_factor = smoothstep(0.88, 0.68, u_w) * effective_scar * dry_factor;
        let basalt_mare = vec3<f32>(0.08, 0.075, 0.07) * (0.85 + warp_n1 * 0.30);

        var central_peak = 0.0;
        if (radius > 0.08 && u_w < 0.22) {
            central_peak = smoothstep(0.22, 0.04, u_w) * 0.78 * effective_scar * dry_factor;
        }

        var crater_col = mix(out_res.color, ejecta_dust, ejecta_blanket);
        crater_col = mix(crater_col, rim_rock, rim_factor);
        crater_col = mix(crater_col, basalt_mare, floor_factor);
        crater_col = mix(crater_col, rim_rock, central_peak);

        out_res.color = crater_col;
        out_res.roughness = mix(out_res.roughness, 0.92, (floor_factor * 0.8 + rim_factor * 0.5) * dry_factor);
        out_res.metallic = mix(out_res.metallic, 0.08, floor_factor * 0.7 * dry_factor);

        let slope = compute_crater_radial_slope(u_w, radius);
        let terrace_ripple = (ridge_noise(p_surf * 34.0) - 0.5) * rim_factor * 0.35;
        out_res.normal_delta += radial_dir * ((slope + terrace_ripple) * effective_scar * dry_factor * 0.72);

        let active_melt = melt_glow * effective_scar * dry_factor;

        if (active_melt > 0.12 && u_w >= 0.84 && u_w < 2.40) {
            let streak_ridge = smoothstep(0.66, 0.92, ridge_noise(p_surf * 32.0 - radial_dir * 1.6));
            let hot_streaks = pow(ray_noise, 1.85) * streak_ridge * smoothstep(2.40, 0.90, u_w) * smoothstep(0.12, 0.90, active_melt);
            let ejecta_glow = vec3<f32>(1.45, 0.48, 0.09) * hot_streaks * 2.8;
            out_res.emissive += ejecta_glow;
            out_res.color = mix(out_res.color, vec3<f32>(0.98, 0.44, 0.10), clamp(hot_streaks * 0.65, 0.0, 0.85));
        }

        if (active_melt > 0.05 && u_w >= 0.78 && u_w < 2.10) {
            let halo_decay = exp(-2.5 * (u_w - 0.78));
            let halo_heat = active_melt * halo_decay * effective_scar;
            let halo_fissure = smoothstep(0.68, 0.91, ridge_noise(p_surf * 28.0 + vec3<f32>(time * 0.02, 0.0, 0.0)));
            let halo_emissive = vec3<f32>(1.35, 0.42, 0.07) * halo_fissure * halo_heat * 3.1;
            out_res.emissive += halo_emissive;
            out_res.color = mix(out_res.color, vec3<f32>(0.12, 0.08, 0.06), halo_heat * halo_fissure * 0.65);
        }

        if (active_melt > 0.005 && u_w < 0.88) {
            let pool_mask = smoothstep(0.88, 0.56, u_w);
            let central_lake = smoothstep(0.38, 0.08, u_w) * smoothstep(0.15, 0.75, active_melt);
            let ground_zero_flash = smoothstep(0.22, 0.0, u_w) * smoothstep(0.45, 0.98, active_melt);

            let r_fissure1 = ridge_noise(p_surf * 30.0 + radial_dir * 1.4 + vec3<f32>(time * 0.03, 0.0, 0.0));
            let r_fissure2 = ridge_noise(p_surf * 58.0 - radial_dir * 0.8);
            let tectonic_web = smoothstep(0.72, 0.92, r_fissure1 * 0.65 + r_fissure2 * 0.35);

            let concentric_fault = smoothstep(0.80, 0.95, sin(u_w * 22.0 + warp_n1 * 4.2) * 0.5 + 0.5) * smoothstep(0.24, 0.84, u_w);
            let radial_fault = smoothstep(0.84, 0.97, sin(ray_angle * 16.0 + warp_n2 * 4.5) * 0.5 + 0.5) * radial_fade;
            let fracture_network = max(tectonic_web, max(concentric_fault * 0.82, radial_fault * 0.75));

            let is_open_lava = clamp(central_lake + fracture_network * pool_mask, 0.0, 1.0);

            let pulse = 1.0 + sin(time * 2.8 + u_w * 10.0) * 0.12;
            let crimson_lava = vec3<f32>(1.15, 0.28, 0.04) * 3.4 * pulse;
            let molten_orange = vec3<f32>(1.45, 0.64, 0.12) * 4.4 * pulse;
            let white_gold = vec3<f32>(1.85, 1.55, 1.05) * 5.2;

            let lava_emission = mix(crimson_lava, molten_orange, clamp(central_lake * 0.85 + fracture_network * 0.45, 0.0, 1.0))
                + white_gold * ground_zero_flash;
            let quenched_crust = vec3<f32>(0.08, 0.065, 0.06) * (0.85 + warp_n2 * 0.30);
            let lava_surface = mix(quenched_crust, clamp(lava_emission * 0.32, vec3<f32>(0.0), vec3<f32>(1.0)), is_open_lava);

            out_res.color = mix(out_res.color, lava_surface, pool_mask * active_melt);
            out_res.emissive += lava_emission * active_melt * (is_open_lava * 0.92 + pool_mask * 0.06);
            out_res.roughness = mix(out_res.roughness, mix(0.78, 0.15, is_open_lava), pool_mask * active_melt);
        }
    }
    return out_res;
}
