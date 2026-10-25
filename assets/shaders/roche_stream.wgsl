// PROTOSTELLAR // 3D Curved Relativistic Roche-Lobe Overflow & Tidal Stream Shader
// Uses 3D view-angle Fresnel (N·V) and seamless cylindrical polar coordinates
// so the stream has zero hard edges, zero end-caps, and twists into the accretion disk.

#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
}

struct RocheStreamUniforms {
    color_and_intensity: vec4<f32>,
    stream_params: vec4<f32>, // x: time, y: turb_scale, z: cross_scale, w: unused
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: RocheStreamUniforms;

const TAU: f32 = 6.28318530718;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm_stream(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var pos = p;
    for (var i = 0; i < 4; i = i + 1) {
        v += a * value_noise(pos);
        pos = pos * 2.05 + vec2<f32>(13.7, 29.3);
        a *= 0.48;
    }
    return v;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    // s: 0.0 at Donor Star photosphere -> 1.0 at Black Hole accretion disk
    let s = clamp(in.uv.y, 0.0, 1.0);
    // theta: 0..2pi seamless angle around the tube circumference
    let theta = in.uv.x * TAU;

    let time = material.stream_params.x;
    let turb_scale = max(material.stream_params.y, 1.0);

    // 1. 3D View-Angle Volumetric Cross-Section (Zero flat seams from any 3D camera angle!)
    let N = normalize(in.world_normal);
    let V = normalize(view.world_position - in.world_position.xyz);
    let ndotv = clamp(abs(dot(N, V)) * 1.15, 0.0, 1.0);

    // Soft Gaussian-like core along the center of the tube, fading to 0 at silhouette edges
    let core_beam = pow(ndotv, 2.2);
    let sheath_halo = pow(ndotv, 1.15) * 0.55;
    let radial_profile = clamp(core_beam + sheath_halo, 0.0, 1.25);

    if (radial_profile < 0.015) {
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    // 2. Relativistic Gravitational Acceleration & Helical Braided Streamers
    // Gas accelerates as it plunges into the gravity well
    let flow_s = (s - 0.30 * s * s) * turb_scale * 2.5 - time * 2.4;

    // Seamless periodic angular coordinates so there is no UV seam on the tube
    let hel_angle = theta + s * 8.0 - time * 1.6;
    let p_noise = vec2<f32>(flow_s, sin(hel_angle) * 1.5 + cos(theta) * 0.8);

    let wisps = fbm_stream(p_noise * 1.8);
    let fine_filaments = fbm_stream(p_noise * 4.2 + vec2<f32>(wisps * 1.6, -time * 0.8));

    // Helical magnetic flux ribbons twisting down the L1 stream
    let ribbon_strands = 0.5 + 0.5 * sin(2.0 * hel_angle + wisps * 3.0);
    let plasma_knots = pow(smoothstep(0.30, 0.85, fine_filaments), 1.6);

    let filament_mod = 0.60 + 0.40 * ribbon_strands + 0.45 * plasma_knots;
    let density = clamp(radial_profile * filament_mod, 0.0, 1.0);

    // 3. Spectral Color Evolution:
    // - s = 0.0..0.35: Donor star's actual photosphere color (e.g. Blue-White for S4714, Red-Orange for Red Giant)
    // - s = 0.35..0.75: Tidally stretched H-alpha / golden-amber plasma stream
    // - s = 0.75..1.00: Doppler-heated accretion disk impact shock hotspot
    let donor_col = max(material.color_and_intensity.rgb, vec3<f32>(0.25, 0.30, 0.45));
    let stream_amber = mix(donor_col, vec3<f32>(1.0, 0.52, 0.18), 0.58);
    let disk_shock_gold = vec3<f32>(1.35, 0.92, 0.50);
    let hot_spine = mix(donor_col * 1.35, vec3<f32>(0.95, 0.98, 1.45), smoothstep(0.45, 0.95, s));

    var col = mix(donor_col, stream_amber, smoothstep(0.15, 0.65, s));
    col = mix(col, disk_shock_gold, smoothstep(0.68, 0.95, s));
    col = mix(col, hot_spine, clamp(core_beam * 0.75 + plasma_knots * 0.25, 0.0, 1.0));

    // 4. L1 Nozzle & Accretion Disk Shock Brightening
    let l1_boost = exp(-pow((s - 0.22) / 0.12, 2.0)) * 0.45;
    let disk_hotspot = exp(-pow((s - 0.86) / 0.09, 2.0)) * 1.10;
    let base_intensity = clamp(material.color_and_intensity.w, 1.0, 2.8);
    let emission = (0.95 + 0.85 * core_beam + 0.45 * plasma_knots) * base_intensity * (0.85 + l1_boost + disk_hotspot);

    // 5. Smooth Feathering at Both Ends (Merges organically into star and accretion disk)
    let donor_fade = smoothstep(0.0, 0.14, s);
    let accretor_fade = 1.0 - smoothstep(0.84, 1.0, s);
    let alpha = clamp(density * donor_fade * accretor_fade * 0.88, 0.0, 0.92);

    out.color = vec4<f32>(col * emission, alpha);
    return out;
}
