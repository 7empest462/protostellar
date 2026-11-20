// WGSL Compute Shader: far-field star-to-body gravity for the Giant Molecular Cloud cluster.
//
// One thread per target body. Sums the gravitational pull of every source (stars / remnants /
// giant planets) that lies BEYOND `near_sq` from the target. Pairs closer than that are handled
// exactly on the CPU, so the two contributions never overlap.
//
// The indirect term mirrors the CPU kernel: the cluster frame is anchored on the central star,
// so each distant source also pulls the frame origin and that acceleration is subtracted.

struct FarFieldUniforms {
    n_src: u32,
    n_tgt: u32,
    near_sq: f32,
    g_const: f32,
    softening_sq: f32,
    star_pos_x: f32,
    star_pos_y: f32,
    star_pos_z: f32,
};

@group(0) @binding(0) var<storage, read> sources: array<vec4<f32>>;   // xyz = pos (AU), w = mass
@group(0) @binding(1) var<storage, read> targets: array<vec4<f32>>;   // xyz = pos (AU)
@group(0) @binding(2) var<storage, read_write> out_acc: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> u: FarFieldUniforms;

var<workgroup> tile_sources: array<vec4<f32>, 64>;

@compute @workgroup_size(64)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
) {
    let idx = gid.x;
    let local_id = lid.x;
    let valid_target = (idx < u.n_tgt && idx < arrayLength(&targets));
    var tp = vec3<f32>(0.0, 0.0, 0.0);
    if (valid_target) {
        tp = targets[idx].xyz;
    }
    var acc = vec3<f32>(0.0, 0.0, 0.0);
    let n_src = min(u.n_src, arrayLength(&sources));
    let star_pos = vec3<f32>(u.star_pos_x, u.star_pos_y, u.star_pos_z);

    let num_tiles = (n_src + 63u) / 64u;
    for (var t: u32 = 0u; t < num_tiles; t = t + 1u) {
        let load_idx = t * 64u + local_id;
        if (load_idx < n_src) {
            tile_sources[local_id] = sources[load_idx];
        } else {
            tile_sources[local_id] = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
        workgroupBarrier();

        if (valid_target) {
            let tile_count = min(64u, n_src - t * 64u);
            for (var j: u32 = 0u; j < tile_count; j = j + 1u) {
                let s = tile_sources[j];
                if (s.w <= 0.0) {
                    continue;
                }
                let d = s.xyz - tp;
                let r2 = dot(d, d);
                if (r2 < u.near_sq) {
                    continue;
                }

                let dist_sq = r2 + u.softening_sq;
                let inv_d3 = 1.0 / (dist_sq * sqrt(dist_sq));
                acc = acc + d * (u.g_const * s.w * inv_d3);

                // Indirect term (frame anchored on the central star's current position).
                let s_rel = s.xyz - star_pos;
                let src_sq = dot(s_rel, s_rel) + u.softening_sq;
                let inv_s3 = 1.0 / (src_sq * sqrt(src_sq));
                acc = acc - s_rel * (u.g_const * s.w * inv_s3);
            }
        }
        workgroupBarrier();
    }

    if (valid_target) {
        out_acc[idx] = vec4<f32>(acc, 0.0);
    }
}
