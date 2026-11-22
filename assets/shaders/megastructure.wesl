// PROTOSTELLAR // Procedural Metallic, Robotic & Photonic Megastructure PBR Shader
// Renders star-lit GGX metallic hulls, hexagonal mirror facets, photovoltaic solar grids,
// thermal Kapton foil, and selective emissive station decks/beacons with baked crevice AO.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
};

struct MegastructureUniforms {
    star_pos_and_time: vec4<f32>,          // xyz: host star world position, w: visual_time_secs
    primary_albedo_and_metal: vec4<f32>,   // xyz: primary hull/mirror RGB, w: metallic
    accent_albedo_and_rough: vec4<f32>,    // xyz: dark titanium/truss RGB, w: base roughness
    emissive_color_and_intensity: vec4<f32>, // xyz: accent beacon/window RGB, w: HDR scale
    pattern_params: vec4<f32>,             // x: tech_style (0..4), y: uv_u, z: uv_v, w: pulse_hz
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> mat: MegastructureUniforms;

const PI: f32 = 3.14159265359;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hex_coords(p: vec2<f32>) -> vec4<f32> {
    let r = vec2<f32>(1.0, 1.7320508);
    let h = r * 0.5;
    let a = (p / r - floor(p / r) - 0.5) * r;
    let b = ((p - h) / r - floor((p - h) / r) - 0.5) * r;
    let gv = select(b, a, dot(a, a) < dot(b, b));
    let id = p - gv;
    let edge_d = max(abs(gv.x), dot(abs(gv), normalize(vec2<f32>(1.0, 1.7320508))));
    let dist_to_border = 0.5 - edge_d;
    return vec4<f32>(dist_to_border, hash21(floor(id * 2.0)), gv.x, gv.y);
}

fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denom * denom, 1e-5);
}

fn geometry_schlick_ggx(n_dot_x: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) * 0.125;
    return n_dot_x / max(n_dot_x * (1.0 - k) + k, 1e-5);
}

fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn perturb_normal_from_height(
    geom_n: vec3<f32>,
    pos_dx: vec3<f32>,
    pos_dy: vec3<f32>,
    h_dx: f32,
    h_dy: f32,
    strength: f32,
) -> vec3<f32> {
    let r1 = cross(pos_dy, geom_n);
    let r2 = cross(geom_n, pos_dx);
    let det = dot(pos_dx, r1);
    let safe_det = select(1.0, det, abs(det) > 1e-8);
    let surf_grad = (sign(det) / max(abs(safe_det), 1e-8)) * (h_dx * r1 + h_dy * r2);
    let valid = select(0.0, strength, abs(det) > 1e-8);
    return normalize((1.0 - 0.001) * geom_n - surf_grad * valid);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    var vcol = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    @if(VERTEX_COLORS)
    vcol = in.color;

    var uv = vec2<f32>(0.5, 0.5);
    @if(VERTEX_UVS_A)
    uv = in.uv;

    let style = mat.pattern_params.x;
    let time = mat.star_pos_and_time.w;
    let scaled_uv = uv * mat.pattern_params.yz;

    // Anti-aliasing derivative fade computed in uniform control flow
    let uv_rate = fwidth(scaled_uv.x) + fwidth(scaled_uv.y);
    let detail_lod = 1.0 - smoothstep(0.35, 1.65, uv_rate);

    let zone_primary = clamp(vcol.r, 0.0, 1.0);
    let emit_mask = clamp(vcol.g, 0.0, 1.0);
    let mirror_mask = clamp(vcol.b, 0.0, 1.0);
    let crevice_ao = clamp(vcol.a, 0.30, 1.0);

    let base_primary = mat.primary_albedo_and_metal.rgb;
    let base_accent = mat.accent_albedo_and_rough.rgb;
    var albedo = mix(base_accent, base_primary, zone_primary);
    var metallic = mix(0.86, mat.primary_albedo_and_metal.w, zone_primary);
    var roughness = clamp(mix(0.42, mat.accent_albedo_and_rough.w, mirror_mask), 0.06, 0.85);
    var bump_h = 0.0;
    var emissive_rgb = vec3<f32>(0.0);

    // Panel grid seams (shared industrial plating detail)
    let p_f = fract(scaled_uv);
    let p_edge = min(min(p_f.x, 1.0 - p_f.x), min(p_f.y, 1.0 - p_f.y));
    let panel_groove = 1.0 - smoothstep(0.025, 0.085, p_edge);
    let plate_seed = hash21(floor(scaled_uv));

    // Hexagonal mirror lattice (shared by Dyson/Shkadov/Heliostat reflectors)
    let hx = hex_coords(scaled_uv * 2.2);
    let hex_seam = 1.0 - smoothstep(0.02, 0.09, hx.x);
    let hex_var = (hx.y - 0.5) * 0.14;

    if (style < 0.5) {
        // Style 0: Dyson Swarm Collector Rings (photovoltaic silicon-gold cells + dark titanium truss)
        let sub_f = fract(scaled_uv * vec2<f32>(4.0, 2.0));
        let cell_line = 1.0 - smoothstep(0.03, 0.09, min(min(sub_f.x, 1.0 - sub_f.x), min(sub_f.y, 1.0 - sub_f.y)));
        let pv_albedo = base_primary * (0.86 + hex_var * 0.4) * (1.0 - 0.55 * cell_line * detail_lod);
        let chassis_albedo = base_accent * (0.90 + (plate_seed - 0.5) * 0.16) * (1.0 - 0.35 * panel_groove * detail_lod);
        albedo = mix(chassis_albedo, pv_albedo, mirror_mask);
        metallic = mix(0.88, 0.94, mirror_mask);
        roughness = mix(0.38, 0.16 + cell_line * 0.25, mirror_mask);
        bump_h = mix(-panel_groove * 0.45, -cell_line * 0.65 + hex_var * 0.15, mirror_mask);
        if (emit_mask > 0.35) {
            let pulse = 0.72 + 0.28 * sin(time * mat.pattern_params.w + scaled_uv.x * 3.0);
            emissive_rgb = mat.emissive_color_and_intensity.rgb * (mat.emissive_color_and_intensity.w * emit_mask * pulse);
        }
    } else if (style < 1.5) {
        // Style 1: Shkadov Stellar Engine (hex parabolic mirror face vs armored titanium backshell + radiators)
        let mirror_col = base_primary * (0.94 + hex_var * 0.5) * (1.0 - 0.58 * hex_seam * detail_lod);
        let armor_col = base_accent * (0.85 + (plate_seed - 0.5) * 0.22) * (1.0 - 0.42 * panel_groove * detail_lod);
        albedo = mix(armor_col, mirror_col, mirror_mask);
        metallic = mix(0.86, 0.98, mirror_mask);
        roughness = mix(0.42, 0.10 + hex_seam * 0.32, mirror_mask);
        bump_h = mix(-panel_groove * 0.55, -hex_seam * 0.80 + hex_var * 0.18, mirror_mask);
        if (emit_mask > 0.25 && emit_mask < 0.85) {
            // Thermal radiator louver slots along counterweight pylons
            let louver = smoothstep(0.25, 0.75, sin(scaled_uv.y * 28.0));
            emissive_rgb = vec3<f32>(1.15, 0.38, 0.08) * (1.35 * emit_mask * louver);
        } else if (emit_mask >= 0.85) {
            let strobe = 0.55 + 0.45 * sin(time * mat.pattern_params.w + scaled_uv.x * 4.0);
            emissive_rgb = mat.emissive_color_and_intensity.rgb * (mat.emissive_color_and_intensity.w * strobe);
        }
    } else if (style < 2.5) {
        // Style 2: Orbital Habitat Ring (brushed titanium hull + illuminated station deck windows + docking beacons)
        let hull = mix(base_accent, base_primary, zone_primary)
            * (0.88 + (plate_seed - 0.5) * 0.18)
            * (1.0 - 0.42 * panel_groove * detail_lod);
        albedo = hull;
        metallic = mix(0.86, 0.91, zone_primary);
        roughness = mix(0.36, 0.24, zone_primary) + panel_groove * 0.20;
        bump_h = -panel_groove * 0.60 + (plate_seed - 0.5) * 0.12;
        if (emit_mask > 0.25 && emit_mask < 0.85) {
            // Segmented habitable viewport windows
            let win_band = smoothstep(0.18, 0.82, sin(scaled_uv.x * 26.0)) * step(0.18, hash21(floor(vec2<f32>(scaled_uv.x * 4.0, scaled_uv.y * 2.0))));
            emissive_rgb = vec3<f32>(1.35, 1.05, 0.62) * (1.55 * emit_mask * win_band);
        } else if (emit_mask >= 0.85) {
            let beacon = 0.65 + 0.35 * sin(time * mat.pattern_params.w + scaled_uv.x * 5.0);
            emissive_rgb = mat.emissive_color_and_intensity.rgb * (mat.emissive_color_and_intensity.w * beacon);
        }
    } else if (style < 3.5) {
        // Style 3: Orbital Solar Mirror Array (JWST faceted gold Kapton hex tiles + dark titanium gimbal booms)
        let gold_tile = base_primary * (0.92 + hex_var * 0.55) * (1.0 - 0.62 * hex_seam * detail_lod);
        let boom_col = base_accent * (0.88 + (plate_seed - 0.5) * 0.16) * (1.0 - 0.38 * panel_groove * detail_lod);
        albedo = mix(boom_col, gold_tile, mirror_mask);
        metallic = mix(0.85, 0.98, mirror_mask);
        roughness = mix(0.42, 0.11 + hex_seam * 0.28, mirror_mask);
        bump_h = mix(-panel_groove * 0.45, -hex_seam * 0.85 + hex_var * 0.25, mirror_mask);
        if (emit_mask > 0.45) {
            let sync_pulse = 0.65 + 0.35 * sin(time * mat.pattern_params.w + uv.x * 6.283);
            emissive_rgb = mat.emissive_color_and_intensity.rgb * (mat.emissive_color_and_intensity.w * sync_pulse);
        }
    } else {
        // Style 4: Space Probe hardware (Kapton crinkle foil vs silicon solar array vs titanium bus)
        let crinkle = sin(scaled_uv.x * 19.0 + sin(scaled_uv.y * 17.0) * 1.4) * sin(scaled_uv.y * 23.0) * 0.5;
        let sub_grid = fract(scaled_uv * vec2<f32>(5.0, 3.0));
        let busbar = 1.0 - smoothstep(0.03, 0.08, min(sub_grid.x, 1.0 - sub_grid.x));
        let kapton_col = base_primary * (0.92 + 0.12 * crinkle) * (1.0 - 0.22 * panel_groove * detail_lod);
        let solar_col = vec3<f32>(0.08, 0.16, 0.34) * (1.0 - 0.35 * panel_groove * detail_lod) + vec3<f32>(0.45, 0.55, 0.65) * (busbar * 0.25);
        albedo = mix(mix(base_accent, kapton_col, zone_primary), solar_col, mirror_mask);
        metallic = mix(0.88, 0.92, zone_primary);
        roughness = mix(0.28, 0.18, mirror_mask) + abs(crinkle) * 0.08;
        bump_h = mix(crinkle * 0.35 - panel_groove * 0.3, -panel_groove * 0.5, mirror_mask);
        if (emit_mask > 0.35) {
            emissive_rgb = mat.emissive_color_and_intensity.rgb * (mat.emissive_color_and_intensity.w * emit_mask);
        }
    }

    // Screen-space derivative bump mapping evaluated in uniform flow
    let pos_dx = dpdx(in.world_position.xyz);
    let pos_dy = dpdy(in.world_position.xyz);
    let h_dx = dpdx(bump_h);
    let h_dy = dpdy(bump_h);
    let n_geom = normalize(in.world_normal);
    let n_bumped = perturb_normal_from_height(n_geom, pos_dx, pos_dy, h_dx, h_dy, 0.045 * detail_lod);

    // Lighting setup calibrated to Protostellar's HDR radiance space
    let to_star = mat.star_pos_and_time.xyz - in.world_position.xyz;
    let l_star = normalize(select(vec3<f32>(0.0, 1.0, 0.0), to_star, dot(to_star, to_star) > 1e-8));
    let v_vec = normalize(view.world_position - in.world_position.xyz);
    let h_star = normalize(l_star + v_vec);

    let n_dot_v = max(dot(n_bumped, v_vec), 0.001);
    let n_dot_l = max(dot(n_bumped, l_star), 0.0);
    let n_dot_h = max(dot(n_bumped, h_star), 0.0);
    let v_dot_h = max(dot(v_vec, h_star), 0.0);

    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let ndf = distribution_ggx(n_dot_h, roughness);
    let g_term = geometry_smith(n_dot_v, n_dot_l, roughness);
    let fres = fresnel_schlick(v_dot_h, f0);
    let specular = (ndf * g_term * fres) / max(4.0 * n_dot_v * max(n_dot_l, 0.001), 0.001);
    let kd = (vec3<f32>(1.0) - fres) * (1.0 - metallic);
    let key_radiance = vec3<f32>(2.15, 2.08, 1.92);
    let direct_star = (kd * albedo / PI + min(specular, vec3<f32>(3.6))) * key_radiance * n_dot_l;

    // Subtle wrap specular glint on metallic mirror rims even at shallow grazing star incidence
    let rim_glint = fresnel_schlick(n_dot_v, f0) * pow(1.0 - n_dot_v, 2.2) * (0.28 + 0.52 * metallic) * (0.35 + 0.65 * n_dot_l);

    // Cool deep-space bounce & edge-defining robotic fill light on shadow side
    let l_fill = normalize(vec3<f32>(-0.42, 0.70, -0.56));
    let n_dot_fill = max(dot(n_bumped, l_fill), 0.0);
    let ambient_fill = (albedo * (0.08 + 0.14 * n_dot_fill) * vec3<f32>(0.55, 0.72, 1.0)) * crevice_ao;

    let final_rgb = (direct_star + rim_glint) * crevice_ao + ambient_fill + emissive_rgb;
    out.color = vec4<f32>(final_rgb, 1.0);
    return out;
}
