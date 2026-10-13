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
    pad0: f32,
    pad1: f32,
    pad2: f32,
};

@group(0) @binding(0) var<storage, read> sources: array<vec4<f32>>;   // xyz = pos (AU), w = mass
@group(0) @binding(1) var<storage, read> targets: array<vec4<f32>>;   // xyz = pos (AU)
@group(0) @binding(2) var<storage, read_write> out_acc: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> u: FarFieldUniforms;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= u.n_tgt || idx >= arrayLength(&targets)) {
        return;
    }

    let tp = targets[idx].xyz;
    var acc = vec3<f32>(0.0, 0.0, 0.0);
    let n_src = min(u.n_src, arrayLength(&sources));

    for (var j: u32 = 0u; j < n_src; j = j + 1u) {
        let s = sources[j];
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

        // Indirect term (frame anchored on the central star at the origin).
        let src_sq = dot(s.xyz, s.xyz) + u.softening_sq;
        let inv_s3 = 1.0 / (src_sq * sqrt(src_sq));
        acc = acc - s.xyz * (u.g_const * s.w * inv_s3);
    }

    out_acc[idx] = vec4<f32>(acc, 0.0);
}
