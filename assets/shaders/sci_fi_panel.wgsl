// PROTOSTELLAR // Sci-Fi Holographic Telemetry Panel Shader
// Renders 45-degree chamfered corners, glowing corner targeting brackets,
// subtle CRT/hologram scanlines, and an angled header accent bar.

#import bevy_ui::ui_vertex_output::UiVertexOutput
#import bevy_render::globals::Globals

@group(0) @binding(1) var<uniform> globals: Globals;

struct SciFiPanelUniforms {
    bg_color: vec4<f32>,
    border_color: vec4<f32>,
    // x: chamfer_px (e.g. 14.0), y: border_px (e.g. 1.5), z: scanline_alpha, w: is_pill (1.0 for pills)
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> material: SciFiPanelUniforms;

// 2D Signed Distance Field for an asymmetric chamfered sci-fi box
// Cuts the top-right and bottom-left corners at 45 degrees while keeping
// top-left and bottom-right crisp for targeting brackets.
fn sd_sci_fi_box(p: vec2<f32>, half_size: vec2<f32>, chamfer: f32) -> f32 {
    let d = abs(p) - half_size;
    let rect_dist = length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);

    // Asymmetric 45-degree corner cuts on Top-Right (p.x > 0, p.y < 0)
    // and Bottom-Left (p.x < 0, p.y > 0)
    var cut_dist = -1e5;
    if (p.x > 0.0 && p.y < 0.0) {
        cut_dist = (p.x - p.y) - (half_size.x + half_size.y - chamfer);
    } else if (p.x < 0.0 && p.y > 0.0) {
        cut_dist = (-p.x + p.y) - (half_size.x + half_size.y - chamfer);
    }
    // Slight 4px bevel on Top-Left and Bottom-Right
    let mini_chamfer = min(chamfer * 0.30, 5.0);
    if (p.x < 0.0 && p.y < 0.0) {
        cut_dist = max(cut_dist, (-p.x - p.y) - (half_size.x + half_size.y - mini_chamfer));
    } else if (p.x > 0.0 && p.y > 0.0) {
        cut_dist = max(cut_dist, (p.x + p.y) - (half_size.x + half_size.y - mini_chamfer));
    }

    return max(rect_dist, cut_dist * 0.70710678);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = max(in.size, vec2<f32>(16.0, 16.0));
    let half_size = size * 0.5;
    let p = (in.uv - vec2<f32>(0.5, 0.5)) * size;

    let chamfer = min(material.params.x, min(half_size.x, half_size.y) * 0.65);
    let border_w = max(material.params.y, 1.0);
    let scan_alpha = material.params.z;
    let is_pill = material.params.w > 0.5;

    let dist = sd_sci_fi_box(p, half_size - vec2<f32>(1.0), chamfer);

    // 1. Outside the chamfered hull -> completely transparent
    let outer_alpha = 1.0 - smoothstep(-0.5, 0.8, dist);
    if (outer_alpha <= 0.005) {
        discard;
    }

    // 2. Deep space holographic vertical gradient fill
    let grad_t = clamp(in.uv.y, 0.0, 1.0);
    let top_tint = material.bg_color.rgb * 1.35 + material.border_color.rgb * 0.05;
    let bot_tint = material.bg_color.rgb * 0.75;
    var col = mix(top_tint, bot_tint, grad_t);

    // 3. Subtle top header telemetry glow bar (first 24px of expanded panels)
    let y_px = in.uv.y * size.y;
    if (!is_pill && y_px < 24.0) {
        let hdr_mask = (1.0 - y_px / 24.0);
        col += material.border_color.rgb * (hdr_mask * hdr_mask * 0.16);
        // Crisp 1px separator line at y = 24px
        let sep_line = exp(-abs(y_px - 23.5) * 2.0);
        col = mix(col, material.border_color.rgb * 0.85, sep_line * 0.45);
    }

    // 4. Moving CRT / Holographic Scanlines & Micro-Grid
    let time = globals.time;
    let scanline = sin(y_px * 1.6 - time * 3.5) * 0.5 + 0.5;
    col *= (1.0 - scan_alpha * 0.18 * scanline);

    // Subtle slow sweeping radar bar
    let sweep_y = fract(time * 0.12) * size.y;
    let sweep_glow = exp(-abs(y_px - sweep_y) * 0.08) * scan_alpha * 0.12;
    col += material.border_color.rgb * sweep_glow;

    // 5. Sci-Fi Perimeter Border + Brightened Corner Targeting Brackets
    let inner_glow = exp(dist * 0.18) * 0.22;
    col += material.border_color.rgb * inner_glow;

    let border_mask = 1.0 - smoothstep(0.0, border_w, abs(dist + border_w * 0.5));

    // Corner bracket amplifier: brighter & thicker near the 4 corners
    let corner_prox_x = smoothstep(half_size.x - 26.0, half_size.x, abs(p.x));
    let corner_prox_y = smoothstep(half_size.y - 26.0, half_size.y, abs(p.y));
    let is_corner_bracket = max(corner_prox_x * step(half_size.y - 18.0, abs(p.y)),
                                corner_prox_y * step(half_size.x - 18.0, abs(p.x)));

    let bracket_border_w = border_w * (1.0 + is_corner_bracket * 1.15);
    let final_border_mask = 1.0 - smoothstep(0.0, bracket_border_w, abs(dist + bracket_border_w * 0.5));

    // Edge fade along the middle of straight runs so corners pop like targeting brackets
    let edge_dim = mix(0.45, 1.25, max(is_corner_bracket, smoothstep(-10.0, 0.0, (p.x - p.y) - (half_size.x + half_size.y - chamfer - 4.0))));
    let border_rgb = material.border_color.rgb * edge_dim;

    let final_rgb = mix(col, border_rgb, clamp(final_border_mask, 0.0, 1.0));
    let final_alpha = outer_alpha * mix(material.bg_color.a, max(material.border_color.a, 0.96), clamp(final_border_mask, 0.0, 1.0));

    return vec4<f32>(final_rgb, final_alpha);
}
