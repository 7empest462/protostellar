// 3D coordinate rotations
fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(p.x * c - p.z * s, p.y, p.x * s + p.z * c);
}

fn rotate_z(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(p.x * c - p.y * s, p.x * s + p.y * c, p.z);
}

// 3D hash
fn hash3(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.x + r.y) * r.z);
}

// 3D value noise
fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
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

// Fractal Brownian Motion (5 octaves)
fn fbm(p: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var shift = vec3<f32>(100.0);
    var pos = p;
    for (var i = 0; i < 5; i = i + 1) {
        v += a * noise(pos);
        pos = pos * 2.02 + shift;
        a *= 0.5;
    }
    return v;
}

// Derivative-warped hydraulic erosion FBM for realistic dendritic mountain ranges and flat valleys
fn eroded_continent_fbm(p: vec3<f32>) -> f32 {
    var val = 0.0;
    var amp = 0.50;
    var pos = p;
    var warp_accum = vec3<f32>(0.0);
    let eps = 0.03;

    for (var i = 0; i < 5; i = i + 1) {
        let sample_p = pos - warp_accum * 0.16;
        let n0 = noise(sample_p);
        let nx = noise(sample_p + vec3<f32>(eps, 0.0, 0.0)) - n0;
        let ny = noise(sample_p + vec3<f32>(0.0, eps, 0.0)) - n0;
        let nz = noise(sample_p + vec3<f32>(0.0, 0.0, eps)) - n0;
        let grad = vec3<f32>(nx, ny, nz) / eps;
        warp_accum += grad * amp;

        let slope_mag = length(warp_accum);
        let erosion_weight = 1.0 / (1.0 + slope_mag * 0.45);
        val += amp * n0 * erosion_weight;

        pos = pos * 2.04 + vec3<f32>(43.1, 17.7, 89.3);
        amp *= 0.49;
    }
    return val;
}

// Band-limited Fractal Brownian Motion with per-octave derivative prefiltering (LOD)
fn fbm_lod(p: vec3<f32>) -> f32 {
    let fw = length(fwidth(p));
    var v = 0.0;
    var a = 0.5;
    var shift = vec3<f32>(100.0);
    var pos = p;
    for (var i = 0; i < 5; i = i + 1) {
        let octave_fw = fw * exp2(f32(i));
        let blend = 1.0 - smoothstep(0.18, 0.65, octave_fw);
        v += a * mix(0.5, noise(pos), blend);
        pos = pos * 2.02 + shift;
        a *= 0.5;
    }
    return v;
}

// Band-limited hydraulic erosion FBM with derivative prefiltering for distant continental terrain
fn eroded_continent_fbm_lod(p: vec3<f32>) -> f32 {
    let fw = length(fwidth(p));
    var val = 0.0;
    var amp = 0.50;
    var pos = p;
    var warp_accum = vec3<f32>(0.0);
    let eps = 0.03;

    for (var i = 0; i < 5; i = i + 1) {
        let octave_fw = fw * exp2(f32(i));
        let blend = 1.0 - smoothstep(0.18, 0.65, octave_fw);
        let sample_p = pos - warp_accum * 0.16;
        let n0 = mix(0.5, noise(sample_p), blend);
        let nx = mix(0.5, noise(sample_p + vec3<f32>(eps, 0.0, 0.0)), blend) - n0;
        let ny = mix(0.5, noise(sample_p + vec3<f32>(0.0, eps, 0.0)), blend) - n0;
        let nz = mix(0.5, noise(sample_p + vec3<f32>(0.0, 0.0, eps)), blend) - n0;
        let grad = vec3<f32>(nx, ny, nz) / eps;
        warp_accum += grad * amp;

        let slope_mag = length(warp_accum);
        let erosion_weight = 1.0 / (1.0 + slope_mag * 0.45);
        val += amp * n0 * erosion_weight;

        pos = pos * 2.04 + vec3<f32>(43.1, 17.7, 89.3);
        amp *= 0.49;
    }
    return val;
}

// High-frequency ridge noise for tectonic rift cracks & volcanic fissures
fn ridge_noise(p: vec3<f32>) -> f32 {
    let n = noise(p);
    return 1.0 - abs(n * 2.0 - 1.0);
}

// Vortex swirl distortion for anticyclonic storms and hurricanes
fn vortex_swirl(p: vec3<f32>, center: vec3<f32>, radius: f32, strength: f32) -> vec3<f32> {
    let d = distance(p, center);
    if (d < radius) {
        let factor = (1.0 - d / radius);
        let angle = factor * factor * strength;
        let s = sin(angle);
        let c = cos(angle);
        let rel = p - center;
        return center + vec3<f32>(rel.x * c - rel.z * s, rel.y, rel.x * s + rel.z * c);
    }
    return p;
}

// 3D vector hash for cellular noise
fn hash3_vec(p: vec3<f32>) -> vec3<f32> {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.xxy + r.yxx) * r.zyx);
}

// 3D Voronoi / Cellular noise with dynamic time-dependent cell boiling
// Returns vec2<f32>:
//   x: d1 = distance to nearest cell center (hot rising convective core)
//   y: d2 = distance to second nearest cell center
//   (d2 - d1): distance to cell boundary (cool sinking intergranular lane)
fn voronoi3(p: vec3<f32>, anim_time: f32) -> vec2<f32> {
    let i = floor(p);
    let f = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    
    for (var z = -1; z <= 1; z = z + 1) {
        for (var y = -1; y <= 1; y = y + 1) {
            for (var x = -1; x <= 1; x = x + 1) {
                let neighbor = vec3<f32>(f32(x), f32(y), f32(z));
                let h = hash3_vec(i + neighbor);
                // Dynamic cell boiling motion: centers wander in 3D over time
                let point = 0.5 + 0.38 * sin(h * 6.28318 + vec3<f32>(anim_time));
                let diff = neighbor + point - f;
                let dist = length(diff);
                if (dist < d1) {
                    d2 = d1;
                    d1 = dist;
                } else if (dist < d2) {
                    d2 = dist;
                }
            }
        }
    }
    return vec2<f32>(d1, d2);
}
