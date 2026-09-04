// ============================================================================
// PROTOSTELLAR // Procedural Deep-Space Celestial Skybox Shader
// Modern Milky Way & Star Clusters vs Early Universe High-Redshift Cosmic Web
// ============================================================================

#import bevy_pbr::forward_io::VertexOutput

struct SkyboxUniforms {
    params: vec4<f32>, // x: time, y: scenario_blend (0.0 = Milky Way, 1.0 = Early Universe), z: exposure, w: star_twinkle
    tuning: vec4<f32>, // x: star_density, y: nebula_intensity, z: cosmic_web_scale, w: filament_brightness
    lens_pos_and_mass: vec4<f32>, // x, y, z: black hole position relative to camera in AU, w: theta_E in radians
    lens_params: vec4<f32>, // x: shadow radius (radians), y: photon ring width (radians), z: is_active (1.0 or 0.0), w: boost
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> skybox: SkyboxUniforms;

// --- FAST 3D HASH & VALUE NOISE FUNCTIONS ---

fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var p3 = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n000 = hash31(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash31(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash31(i + vec3<f32>(1.0, 1.0, 1.0));

    let x00 = mix(n000, n100, u.x);
    let x10 = mix(n010, n110, u.x);
    let x01 = mix(n001, n101, u.x);
    let x11 = mix(n011, n111, u.x);

    let y0 = mix(x00, x10, u.y);
    let y1 = mix(x01, x11, u.y);

    return mix(y0, y1, u.z);
}

fn fbm3(p: vec3<f32>, octaves: i32) -> f32 {
    var val: f32 = 0.0;
    var amp: f32 = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i = i + 1) {
        val += amp * noise3(pos);
        pos = pos * 2.02 + vec3<f32>(13.1, 41.7, 19.3);
        amp *= 0.5;
    }
    return val;
}

fn ridge_fbm3(p: vec3<f32>, octaves: i32) -> f32 {
    var val: f32 = 0.0;
    var amp: f32 = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i = i + 1) {
        let n = noise3(pos);
        let ridge = 1.0 - abs(n * 2.0 - 1.0);
        val += amp * ridge * ridge;
        pos = pos * 2.05 + vec3<f32>(27.3, 11.9, 37.1);
        amp *= 0.5;
    }
    return val;
}

// Transform ecliptic direction into tilted Galactic coordinates (~60.2 deg tilt)
fn to_galactic(d: vec3<f32>) -> vec3<f32> {
    let c1 = 0.49697;  // cos(60.2 deg)
    let s1 = 0.86776;  // sin(60.2 deg)
    let p1 = vec3<f32>(d.x, c1 * d.y - s1 * d.z, s1 * d.y + c1 * d.z);
    let c2 = 0.8746;   // cos(29.0 deg longitude offset)
    let s2 = 0.4848;   // sin(29.0 deg)
    return vec3<f32>(c2 * p1.x + s2 * p1.z, p1.y, -s2 * p1.x + c2 * p1.z);
}

// ============================================================================
// PART 1: MODERN MILKY WAY & STAR CLUSTERS
// ============================================================================

// Evaluates an isotropic, unskewed, non-flared celestial starfield layer on the unit sphere
fn render_star_layer(
    dir: vec3<f32>,
    scale: f32,
    seed_offset: vec3<f32>,
    threshold: f32,
    core_sigma: f32,
    halo_sigma: f32,
    halo_intensity: f32,
    time: f32,
    twinkle_strength: f32,
) -> vec3<f32> {
    let p = dir * scale;
    let b = floor(p);
    var accum = vec3<f32>(0.0);

    // Minimum squared chord distance cone covering 3.2 sigma of the star profile
    let max_sigma = max(core_sigma, halo_sigma);
    let max_th = max_sigma * 3.2;
    let max_dist_sq = max_th * max_th;

    // Check 8 surrounding lattice nodes (strictly encloses any star within cone)
    for (var z = 0; z <= 1; z = z + 1) {
        for (var y = 0; y <= 1; y = y + 1) {
            for (var x = 0; x <= 1; x = x + 1) {
                let node = b + vec3<f32>(f32(x), f32(y), f32(z));
                let rand = hash33(node + seed_offset);

                if (rand.x > threshold) {
                    // Star celestial position is fixed to the node and spherically projected
                    let star_pos = node + (rand - 0.5) * 0.85;
                    let star_dir = normalize(star_pos);
                    let delta = dir - star_dir;
                    let th2 = dot(delta, delta);

                    if (th2 < max_dist_sq) {
                        // Mathematically circular angular distance on celestial sphere: ||d - s||^2
                        let core = exp(-th2 / (core_sigma * core_sigma));
                        let halo = halo_intensity * exp(-th2 / (halo_sigma * halo_sigma));
                        let profile = core + halo;

                        // Vivid chromatic stellar spectral variety (O/B, A, F, G, K, M, Carbon)
                        var spectral = vec3<f32>(1.0);
                        let spec = rand.z;
                        if (spec < 0.14) {
                            // Deep Electric Sapphire / Cobalt Blue (O/B Supergiants, e.g. Rigel, Spica)
                            spectral = vec3<f32>(0.20, 0.48, 1.85);
                        } else if (spec < 0.28) {
                            // Icy Diamond Blue-White (A-type, e.g. Sirius, Vega)
                            spectral = vec3<f32>(0.55, 0.80, 1.35);
                        } else if (spec < 0.44) {
                            // Crisp Pure White (F-type, e.g. Canopus, Procyon)
                            spectral = vec3<f32>(0.92, 0.96, 1.04);
                        } else if (spec < 0.64) {
                            // Warm Solar Gold (G-type, e.g. Sun, Alpha Centauri)
                            spectral = vec3<f32>(1.28, 1.02, 0.40);
                        } else if (spec < 0.82) {
                            // Deep Amber / Orange Giant (K-type, e.g. Arcturus, Aldebaran)
                            spectral = vec3<f32>(1.48, 0.62, 0.12);
                        } else if (spec < 0.94) {
                            // Vivid Ruby Red Supergiant / Dwarf (M-type, e.g. Betelgeuse, Antares)
                            spectral = vec3<f32>(1.65, 0.20, 0.06);
                        } else {
                            // Deep Scarlet Carbon Star (e.g. La Superba)
                            spectral = vec3<f32>(1.75, 0.10, 0.08);
                        }

                        // Wide dynamic range: 85% of stars are faint, delicate background pinpricks
                        // that do not drown out the foreground solar system
                        let p_mag = rand.y;
                        var mag: f32;
                        if (p_mag < 0.85) {
                            mag = 0.04 + p_mag * 0.18; // 0.04 to 0.19 (delicate, faint)
                        } else if (p_mag < 0.97) {
                            mag = 0.22 + (p_mag - 0.85) * 2.5; // 0.22 to 0.52 (medium field star)
                        } else {
                            mag = 0.60 + (p_mag - 0.97) * 15.0; // 0.60 to 1.05 (rare bright landmark)
                        }

                        let twinkle = 1.0 + sin(time * 2.8 + rand.x * 45.0) * 0.12 * twinkle_strength;

                        accum += spectral * (profile * mag * twinkle);
                    }
                }
            }
        }
    }

    return accum;
}

fn render_milky_way(dir: vec3<f32>, time: f32) -> vec3<f32> {
    let g = to_galactic(dir);
    let b = g.y; // Galactic latitude sin(b)
    let abs_b = abs(b);

    // 1. Galactic Disk Glow (thin disk + thick disk + halo)
    let thin_disk = exp(-abs_b / 0.045) * 0.95;
    let thick_disk = exp(-abs_b / 0.16) * 0.48;
    let halo_glow = exp(-abs_b / 0.45) * 0.16;
    let disk_profile = thin_disk + thick_disk + halo_glow;

    // 2. Galactic Bulge (Sagittarius A* direction: g near (1.0, 0.0, 0.0))
    let sgr_a_dist = length(g - vec3<f32>(1.0, 0.0, 0.0));
    let bulge_core = exp(-sgr_a_dist * sgr_a_dist * 18.0) * 1.85;
    let bulge_halo = exp(-sgr_a_dist * sgr_a_dist * 4.5) * 0.75;
    let bulge_total = bulge_core + bulge_halo;

    // Bulge warm golden-amber starlight tone
    let bulge_color = vec3<f32>(1.0, 0.86, 0.65) * bulge_total;

    // 3. The Great Rift & Dark Molecular Absorption Nebulae
    // Obscures the galactic plane with realistic fractal dust lanes & Bok globules
    let dust_coord = g * 14.0 + vec3<f32>(1.2, -0.4, 3.7);
    let dust_noise = fbm3(dust_coord, 5);
    let dust_fine = fbm3(dust_coord * 3.5, 3);
    let dust_density = clamp(dust_noise * 1.6 + dust_fine * 0.45 - 0.45, 0.0, 2.5);
    let dust_optical_depth = dust_density * exp(-abs_b / 0.065) * 2.8;
    let dust_transmission = exp(-dust_optical_depth);

    // Warm interstellar starlight wash
    let disk_starlight = vec3<f32>(0.88, 0.92, 1.05) * disk_profile * dust_transmission;

    // 4. H-Alpha Emission & O-III Reflection Nebulae
    let neb_coord = g * 7.5 + vec3<f32>(5.4, 2.1, -1.8);
    let h_alpha_mask = smoothstep(0.55, 0.85, fbm3(neb_coord, 4)) * exp(-abs_b / 0.09);
    let h_alpha_color = vec3<f32>(1.0, 0.18, 0.52) * h_alpha_mask * 1.4 * dust_transmission;

    let o_iii_mask = smoothstep(0.60, 0.90, fbm3(neb_coord * 1.6 + vec3<f32>(2.0, -1.5, 0.8), 3)) * exp(-abs_b / 0.08);
    let o_iii_color = vec3<f32>(0.15, 0.85, 0.80) * o_iii_mask * 0.9 * dust_transmission;

    // 5. Multi-Spectral Procedural Starfield (Spherically Isotropic, Refined Density & High Chromatic Variety)
    var star_light = vec3<f32>(0.0);

    // Layer 1: Prominent Constellation Landmark Stars (Sparingly placed across the sky)
    let l1 = render_star_layer(
        dir,
        18.0,
        vec3<f32>(0.0, 0.0, 0.0),
        0.962, // Very sparse: only ~250 bright stars on entire sphere
        0.00095,
        0.0020,
        0.04,
        time,
        skybox.params.w,
    );

    // Layer 2: Delicate Background Field Stars (Faint, colorful, non-intrusive pinpoints)
    let l2 = render_star_layer(
        dir,
        36.0,
        vec3<f32>(173.1, 311.7, 729.3),
        0.935, // Sparse: only ~1,800 faint stars on entire sphere
        0.00065,
        0.00065,
        0.0,
        time,
        skybox.params.w * 0.6,
    );

    // Layer 3: Faint Milky Way Stardust (Strictly confined to the galactic disk dust lane)
    let mw_equator_mask = exp(-abs_b / 0.05); // Only along the Milky Way band!
    let l3 = render_star_layer(
        dir,
        64.0,
        vec3<f32>(541.3, 887.1, 239.5),
        0.920,
        0.00050,
        0.00050,
        0.0,
        time,
        0.0,
    ) * (mw_equator_mask * 0.35); // Soft, subtle stardust wash

    star_light += (l1 + l2 + l3) * skybox.tuning.x;

    // 6. Deep Space Star Clusters & Objects (100% Spherically Isotropic)
    // =========================================================================
    // A. PLEIADES OPEN CLUSTER (M45 / Seven Sisters in Taurus)
    // =========================================================================
    let pleiades_dir = normalize(vec3<f32>(0.62, 0.44, -0.65));
    let delta_p = dir - pleiades_dir;
    let th2_p = dot(delta_p, delta_p);
    if (th2_p < 0.016) {
        // Ethereal luminous sapphire reflection nebula (smooth layered Gaussian)
        let neb_core = exp(-th2_p / 0.0007) * 0.85;
        let neb_halo = exp(-th2_p / 0.0028) * 0.35;
        let neb_color = vec3<f32>(0.22, 0.55, 1.45) * (neb_core + neb_halo);
        star_light += neb_color;

        // Tangent frame at pleiades_dir
        let p_right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), pleiades_dir));
        let p_up = cross(pleiades_dir, p_right);

        // Astronomical relative coordinates of the Seven Sisters (mini-dipper asterism)
        let s_alcyone = normalize(pleiades_dir + p_right * 0.0000 + p_up * 0.0000); // Alcyone (Eta Tauri, central supergiant)
        let s_atlas   = normalize(pleiades_dir + p_right * 0.0125 - p_up * 0.0045); // Atlas
        let s_pleione = normalize(pleiades_dir + p_right * 0.0135 + p_up * 0.0018); // Pleione
        let s_electra = normalize(pleiades_dir - p_right * 0.0138 + p_up * 0.0050); // Electra
        let s_maia    = normalize(pleiades_dir - p_right * 0.0030 + p_up * 0.0120); // Maia
        let s_merope  = normalize(pleiades_dir - p_right * 0.0050 - p_up * 0.0100); // Merope
        let s_taygeta = normalize(pleiades_dir - p_right * 0.0165 + p_up * 0.0175); // Taygeta
        let s_celaeno = normalize(pleiades_dir - p_right * 0.0120 + p_up * 0.0110); // Celaeno
        let s_asterope= normalize(pleiades_dir - p_right * 0.0085 + p_up * 0.0210); // Asterope

        let star_blue = vec3<f32>(0.35, 0.70, 1.80);
        let star_white = vec3<f32>(0.75, 0.90, 1.35);
        var p_stars = vec3<f32>(0.0);

        // Alcyone (brightest, magnitude 2.8, brilliant sapphire with soft aura)
        let da = dot(dir - s_alcyone, dir - s_alcyone);
        if (da < 0.00004) {
            let core = exp(-da / (0.0013 * 0.0013)) * 2.2;
            let aura = exp(-da / (0.0035 * 0.0035)) * 0.35;
            p_stars += star_blue * (core + aura);
        }

        // Atlas (magnitude 3.6)
        let d_at = dot(dir - s_atlas, dir - s_atlas);
        if (d_at < 0.00003) { p_stars += star_blue * exp(-d_at / (0.0011 * 0.0011)) * 1.6; }

        // Pleione (magnitude 5.0)
        let d_pl = dot(dir - s_pleione, dir - s_pleione);
        if (d_pl < 0.00003) { p_stars += star_white * exp(-d_pl / (0.0009 * 0.0009)) * 1.1; }

        // Electra (magnitude 3.7)
        let d_el = dot(dir - s_electra, dir - s_electra);
        if (d_el < 0.00003) { p_stars += star_blue * exp(-d_el / (0.0011 * 0.0011)) * 1.5; }

        // Maia (magnitude 3.8)
        let d_ma = dot(dir - s_maia, dir - s_maia);
        if (d_ma < 0.00003) { p_stars += star_blue * exp(-d_ma / (0.0010 * 0.0010)) * 1.4; }

        // Merope (magnitude 4.1, illuminates Merope reflection nebula)
        let d_me = dot(dir - s_merope, dir - s_merope);
        if (d_me < 0.00003) { p_stars += star_blue * exp(-d_me / (0.0010 * 0.0010)) * 1.3; }

        // Taygeta (magnitude 4.3)
        let d_ta = dot(dir - s_taygeta, dir - s_taygeta);
        if (d_ta < 0.00003) { p_stars += star_white * exp(-d_ta / (0.0009 * 0.0009)) * 1.2; }

        // Celaeno & Asterope
        let d_ce = dot(dir - s_celaeno, dir - s_celaeno);
        if (d_ce < 0.00003) { p_stars += star_white * exp(-d_ce / (0.0008 * 0.0008)) * 0.9; }
        let d_as = dot(dir - s_asterope, dir - s_asterope);
        if (d_as < 0.00003) { p_stars += star_white * exp(-d_as / (0.0008 * 0.0008)) * 0.8; }

        star_light += p_stars;
    }

    // =========================================================================
    // B. OMEGA CENTAURI GLOBULAR CLUSTER (NGC 5139)
    // =========================================================================
    let globular_dir = normalize(vec3<f32>(-0.45, 0.72, 0.52));
    let delta_g = dir - globular_dir;
    let th2_g = dot(delta_g, delta_g);
    if (th2_g < 0.010) {
        // King / Plummer profile: I(r) = I0 / (1 + r^2 / rc^2)^(1.4)
        let plummer = 1.0 / pow(1.0 + th2_g * 12000.0, 1.4);
        let halo = exp(-th2_g / 0.0018) * 0.35;
        let globular_glow = vec3<f32>(1.30, 1.05, 0.65) * (plummer * 1.35 + halo);
        star_light += globular_glow;
    }

    // =========================================================================
    // C. ANDROMEDA GALAXY (M31 / Messier 31 Spiral Galaxy)
    // =========================================================================
    let m31_dir = normalize(vec3<f32>(-0.32, -0.42, 0.85));
    let delta_m31 = dir - m31_dir;
    let th2_m31 = dot(delta_m31, delta_m31);
    if (th2_m31 < 0.025) {
        // Tangent frame aligned with Andromeda's major position angle (~35 degrees)
        let m31_up = normalize(vec3<f32>(0.2, 0.8, 0.3));
        let m31_r = normalize(cross(m31_up, m31_dir));
        let m31_u = cross(m31_dir, m31_r);

        // Tangent coordinates: x_maj along major axis, y_proj along minor axis
        let x_maj = dot(delta_m31, m31_r);
        let y_proj = dot(delta_m31, m31_u);

        // Inclination deprojection (i ~ 77.5 deg, cos(i) ~ 0.22)
        let y_disk = y_proj / 0.23;
        let r_disk = sqrt(max(x_maj * x_maj + y_disk * y_disk, 1e-9));
        let phi = atan2(y_disk, x_maj);

        // 1. Luminous Golden Galactic Nucleus & Central Bulge (Ancient Pop II Stars)
        // Calibrated HDR peak to prevent flat-white clamping and preserve rich amber-gold hue
        let m31_nucleus = exp(-r_disk * 280.0) * 0.42;
        let m31_inner_bulge = exp(-r_disk * 110.0) * 0.28;
        let r_bulge_sky = sqrt(x_maj * x_maj + (y_proj / 0.45) * (y_proj / 0.45));
        let m31_outer_bulge = exp(-r_bulge_sky * 65.0) * 0.16;
        let m31_bulge = vec3<f32>(1.25, 0.96, 0.58) * (m31_nucleus + m31_inner_bulge)
                      + vec3<f32>(1.10, 0.85, 0.62) * m31_outer_bulge;

        // 2. Extended Spiral Disk (Young blue star-forming population in spiral arms)
        let arm_phase = 2.0 * phi - log(r_disk * 35.0 + 0.12) * 3.4;
        let arm_wave = 0.5 + 0.5 * cos(arm_phase);
        let arm_density = 0.65 + 0.55 * pow(arm_wave, 2.2);
        let disk_envelope = exp(-r_disk * 38.0) * smoothstep(0.003, 0.012, r_disk);
        let m31_disk = vec3<f32>(0.32, 0.54, 0.95) * (disk_envelope * 0.34 * arm_density);

        // 3. H-II Emission Star-Forming Regions (Subtle magenta knots along spiral arms)
        let hii_ring = exp(-pow((r_disk - 0.024) / 0.0055, 2.0));
        let m31_hii = vec3<f32>(0.95, 0.24, 0.48) * (hii_ring * pow(arm_wave, 3.5) * 0.10);

        // 4. Smooth Curved Dust Absorption Lanes & Reddening (Near-side silhouette, no rectangles)
        let near_side = smoothstep(-0.002, 0.006, y_proj);
        let dust_r = sqrt(x_maj * x_maj + (y_proj / 0.21) * (y_proj / 0.21));
        let dust_lane_inner = exp(-pow((dust_r - 0.011) / 0.0022, 2.0));
        let dust_lane_main  = exp(-pow((dust_r - 0.019) / 0.0032, 2.0));
        let dust_lane_outer = exp(-pow((dust_r - 0.028) / 0.0042, 2.0));
        let dust_spiral = 0.5 + 0.5 * cos(arm_phase + 0.7);
        let dust_total = (dust_lane_inner * 0.50 + dust_lane_main * 0.45 + dust_lane_outer * 0.30)
                       * (0.65 + 0.35 * dust_spiral)
                       * near_side
                       * smoothstep(0.045, 0.025, abs(x_maj));
        let tau = clamp(dust_total, 0.0, 0.70);
        let dust_transmission = vec3<f32>(1.0 - tau * 0.55, 1.0 - tau * 0.80, 1.0 - tau * 0.98);

        // 5. Diffuse Spheroidal Stellar Halo
        let r_halo = sqrt(x_maj * x_maj + (y_proj / 0.6) * (y_proj / 0.6));
        let m31_halo = vec3<f32>(0.50, 0.58, 0.78) * (exp(-r_halo * 18.0) * 0.07);

        // 6. Satellite Dwarf Companions: M32 (compact elliptical) and M110 (dwarf spheroidal)
        let m32_offset = delta_m31 - (m31_r * 0.008 + m31_u * 0.010);
        let d2_m32 = dot(m32_offset, m32_offset);
        let m32_prof = 0.16 / (1.0 + d2_m32 * 80000.0) + 0.05 * exp(-d2_m32 * 14000.0);
        let m32_glow = vec3<f32>(1.10, 0.92, 0.65) * m32_prof;

        let m110_offset = delta_m31 - (-m31_r * 0.016 - m31_u * 0.018);
        let m110_maj = dot(m110_offset, m31_r);
        let m110_min = dot(m110_offset, m31_u) / 0.58;
        let m110_r2 = m110_maj * m110_maj + m110_min * m110_min;
        let m110_prof = exp(-m110_r2 * 10000.0) * 0.12;
        let m110_glow = vec3<f32>(0.62, 0.68, 0.82) * m110_prof;

        // 7. Smooth Boundary Falloff (Zero hard edges or discontinuities)
        let m31_boundary = smoothstep(0.025, 0.016, th2_m31);
        let m31_total = (m31_bulge + m31_disk + m31_hii) * dust_transmission
                      + m31_halo + m32_glow + m110_glow;
        star_light += m31_total * m31_boundary;
    }

    // Ambient interstellar darkness (faint cosmic infrared bath)
    let ambient_space = vec3<f32>(0.005, 0.006, 0.010);

    return ambient_space + disk_starlight + bulge_color + h_alpha_color + o_iii_color + star_light;
}

// ============================================================================
// PART 2: EARLY UNIVERSE & HIGH-REDSHIFT COSMIC WEB (z ~ 8.5)
// ============================================================================

// 3D Voronoi Cellular Web Network (calculates F1 and F2 to find filament boundaries)
fn voronoi_web3(p: vec3<f32>) -> vec4<f32> {
    let pi = floor(p);
    let pf = fract(p);

    var f1 = 999.0;
    var f2 = 999.0;
    var node_seed = vec3<f32>(0.0);

    for (var z = -1; z <= 1; z = z + 1) {
        for (var y = -1; y <= 1; y = y + 1) {
            for (var x = -1; x <= 1; x = x + 1) {
                let neighbor = vec3<f32>(f32(x), f32(y), f32(z));
                let cell_point = hash33(pi + neighbor) * 0.75 + 0.125;
                let diff = neighbor + cell_point - pf;
                let dist = length(diff);

                if (dist < f1) {
                    f2 = f1;
                    f1 = dist;
                    node_seed = pi + neighbor;
                } else if (dist < f2) {
                    f2 = dist;
                }
            }
        }
    }

    // x: F1, y: F2, z: F2 - F1 (distance to filament boundary), w: node hash
    return vec4<f32>(f1, f2, f2 - f1, hash31(node_seed));
}

fn render_early_universe(dir: vec3<f32>, time: f32) -> vec3<f32> {
    // 1. Primordial Intergalactic Cosmic Web Filaments
    // Scale the direction sphere into 3D cellular filament space
    let web_scale = skybox.tuning.z * 5.2;
    let web_data = voronoi_web3(dir * web_scale);

    // Filament spine is located where F2 - F1 is minimal (cell wall boundary)
    let boundary_dist = web_data.z;
    let filament_spine = exp(-boundary_dist * boundary_dist * 45.0);

    // Fine organic branching sub-strands via 3D ridge noise
    let ridge_web = ridge_fbm3(dir * 12.5 + vec3<f32>(0.0, time * 0.008, 0.0), 4);
    let web_density = (filament_spine * 0.75 + ridge_web * 0.45);

    // Cosmologically Redshifted Lyman-Alpha & Hydrogen Glow
    // At z ~ 8.5, Ly-alpha is stretched by 9.5x into deep ruby, fiery crimson, and amber-gold infrared hues!
    let lyman_ruby = vec3<f32>(0.92, 0.10, 0.18);
    let lyman_amber = vec3<f32>(1.0, 0.42, 0.08);
    let lyman_deep_ir = vec3<f32>(0.75, 0.06, 0.35);

    let web_color = mix(lyman_ruby, lyman_amber, filament_spine * 0.8) + lyman_deep_ir * (ridge_web * 0.5);
    let filament_emission = web_color * web_density * skybox.tuning.w * 1.85;

    // 2. Epoch of Reionization (EoR) Strömgren Ionization Bubbles
    // The first quasars blow translucent ionized bubbles into the neutral hydrogen fog
    let bubble_grid = dir * 2.8;
    let bubble_f = fbm3(bubble_grid, 3);
    // Bubble boundary shock front
    let bubble_shock = smoothstep(0.48, 0.56, bubble_f) * (1.0 - smoothstep(0.56, 0.64, bubble_f));
    let bubble_rim_glow = vec3<f32>(1.1, 0.68, 0.22) * bubble_shock * 1.25;

    // 3. Pristine Population III Starburst Knots & Proto-Galactic Seeds
    // Located at the intersection nodes of multiple cosmic filaments (where F1 ~ 0)
    var pop3_starburst = vec3<f32>(0.0);
    let node_proximity = web_data.x;
    if (node_proximity < 0.22) {
        let node_hash = web_data.w;
        let node_d = node_proximity / 0.22;
        let cluster_core = exp(-node_d * node_d * 24.0);

        // Blinding violet-white Pop-III hypergiants (zero-metallicity stars, T > 50,000 K)
        let pop3_core = vec3<f32>(1.8, 1.65, 2.2) * cluster_core * (0.8 + node_hash * 1.2);

        // Surrounding ruby-crimson ionized hydrogen cloud
        let pop3_halo = vec3<f32>(1.2, 0.18, 0.30) * exp(-node_d * 5.0) * 0.95;

        pop3_starburst = pop3_core + pop3_halo;
    }

    // 4. Distant Primordial Mini-Quasars ("Little Red Dots")
    // Compact, ruby-red active galactic nuclei shining across the early universe
    let q_grid = dir * 32.0;
    let q_cell = floor(q_grid);
    let q_hash = hash33(q_cell);
    var quasar_beacons = vec3<f32>(0.0);
    if (q_hash.x > 0.95) {
        let q_pos = q_hash * 0.7 + 0.15;
        let q_d = length(fract(q_grid) - q_pos);
        let q_peak = exp(-q_d * q_d * 120.0) * 2.2;
        let q_halo = exp(-q_d * 14.0) * 0.6;
        // Signature JWST Little Red Dot color: intense ruby core with warm crimson halo
        quasar_beacons += vec3<f32>(1.35, 0.18, 0.25) * (q_peak + q_halo) * (1.0 + q_hash.y * 1.5);
    }

    // 5. Primordial Cosmic Dawn Ambient Bath
    // The faint, warm 25 K CMB bath redshifted into a subtle, eerie infrared dusk
    let cosmic_dawn_bath = vec3<f32>(0.016, 0.005, 0.008);

    return cosmic_dawn_bath + filament_emission + bubble_rim_glow + pop3_starburst + quasar_beacons;
}

// ============================================================================
// PART 3: GENERAL RELATIVISTIC GRAVITATIONAL LENSING & WARPING
// ============================================================================

struct LensingResult {
    deflected_dir: vec3<f32>,
    shadow_mask: f32,
    photon_ring_emission: vec3<f32>,
};

fn compute_gravitational_lensing(view_dir: vec3<f32>) -> LensingResult {
    var res: LensingResult;
    res.deflected_dir = view_dir;
    res.shadow_mask = 1.0;
    res.photon_ring_emission = vec3<f32>(0.0);

    if (skybox.lens_params.z < 0.5) {
        return res; // Lensing inactive
    }

    let bh_rel = skybox.lens_pos_and_mass.xyz;
    let dist_to_bh = length(bh_rel);
    if (dist_to_bh < 0.001) {
        return res;
    }

    let bh_dir = bh_rel / dist_to_bh;
    let cos_theta = clamp(dot(view_dir, bh_dir), -1.0, 1.0);
    let theta = acos(cos_theta);

    let theta_E = skybox.lens_pos_and_mass.w;
    let theta_shadow = skybox.lens_params.x;
    let photon_ring_width = max(skybox.lens_params.y, 0.0015);

    // 1. Photon Sphere Caustic Ring (at theta ~ theta_shadow)
    let d_photon = abs(theta - theta_shadow) / photon_ring_width;
    let photon_core = exp(-d_photon * d_photon * 8.0) * 3.2;
    let photon_halo = exp(-d_photon * 2.5) * 1.1;

    // Relativistic Doppler beaming asymmetry (frame-dragging along black hole rotation)
    let phi_angle = atan2(view_dir.z, view_dir.x);
    let doppler_boost = 1.0 + sin(phi_angle * 2.0) * 0.25 * skybox.lens_params.w;

    let photon_ring_color = (vec3<f32>(1.0, 0.96, 0.88) * photon_core + vec3<f32>(0.35, 0.75, 1.25) * photon_halo) * doppler_boost;
    res.photon_ring_emission = photon_ring_color;

    // 2. Black Hole Event Horizon Shadow Mask
    res.shadow_mask = smoothstep(theta_shadow * 0.94, theta_shadow * 1.02, theta);

    // 3. General Relativistic Light Ray Deflection
    // Deflection angle alpha(theta) = theta_E^2 / theta
    let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));
    if (sin_theta > 0.0001) {
        let v_perp = (view_dir - cos_theta * bh_dir) / sin_theta;

        // Taper lensing smoothly away from black hole
        let lens_falloff = exp(-theta * theta / 4.0);
        let alpha = (theta_E * theta_E / (theta + 0.004)) * lens_falloff;
        let beta = theta - alpha; // Source angular position

        res.deflected_dir = normalize(cos(beta) * bh_dir + sin(beta) * v_perp);
    }

    return res;
}

// ============================================================================
// MAIN FRAGMENT SHADER
// ============================================================================

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Normal on celestial sphere is the exact unit viewing direction
    let raw_dir = normalize(in.world_normal);
    let time = skybox.params.x;
    let blend = clamp(skybox.params.y, 0.0, 1.0);

    // Apply General Relativistic Gravitational Lensing & Warping
    let lens = compute_gravitational_lensing(raw_dir);
    let dir = lens.deflected_dir;

    var color = vec3<f32>(0.0);

    if (blend <= 0.001) {
        // Pure Modern Milky Way
        color = render_milky_way(dir, time);
    } else if (blend >= 0.999) {
        // Pure Early Universe High-Redshift Cosmic Web
        color = render_early_universe(dir, time);
    } else {
        // Smooth scenario interpolation between eras
        let modern = render_milky_way(dir, time);
        let early = render_early_universe(dir, time);
        color = mix(modern, early, blend);
    }

    // Apply black hole shadow occlusion and add photon sphere caustic ring
    color = color * lens.shadow_mask + lens.photon_ring_emission;

    // Exposure tone multiplier
    let final_color = color * skybox.params.z;

    return vec4<f32>(final_color, 1.0);
}

