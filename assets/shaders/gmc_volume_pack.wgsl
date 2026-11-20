// WGSL GPU-to-GPU Compute Pack Shader for Live 96^3 Giant Molecular Cloud Volume
// Packs the active ping-pong Eulerian fluid density, temperature, and turbulent speed
// storage buffers directly into a filterable 3D Rgba16Float texture (zero CPU readback).

@group(0) @binding(0) var<storage, read> density_in: array<f32>;
@group(0) @binding(1) var<storage, read> velocity_in: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> temperature_in: array<f32>;
@group(0) @binding(3) var volume_out: texture_storage_3d<rgba16float, write>;

const GMC_DIM: u32 = 96u;

@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= GMC_DIM || gid.y >= GMC_DIM || gid.z >= GMC_DIM) {
        return;
    }

    let idx = gid.z * GMC_DIM * GMC_DIM + gid.y * GMC_DIM + gid.x;
    let rho = density_in[idx];
    let vel = velocity_in[idx].xyz;
    let temp_k = temperature_in[idx];

    // Normalize physical GMC units into hardware-filterable Rgba16Float channels:
    // r: density relative to canonical cloud body (2.2e-11 M_sun / AU^3), clumps peak > 2.5
    // g: shock-heated thermal excess above 15 K cold molecular baseline
    // b: local turbulent / outflow speed magnitude (AU / yr)
    // a: 1.0 sentinel indicating live GPU volume data is populated
    let rho_norm = clamp(rho / 2.2e-11, 0.0, 12.0);
    let temp_excess = clamp((temp_k - 15.0) / 1500.0, 0.0, 4.0);
    let speed_norm = clamp(length(vel) * 1.5, 0.0, 4.0);

    textureStore(
        volume_out,
        vec3<i32>(gid),
        vec4<f32>(rho_norm, temp_excess, speed_norm, 1.0),
    );
}
