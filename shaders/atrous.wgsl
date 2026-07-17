// Solid-rendering à-trous normal smoothing (Phase 1).
//
// One edge-aware smoothing iteration over the (normal.xyz, depth)
// texture produced by normals.wgsl: a 5×5 B3-spline kernel sampled at a
// power-of-two stride ("holes" between taps grow each iteration —
// à trous), with per-tap weights that preserve edges:
//
//   w = kernel · exp(-(Δdepth / σ_z)²) · max(dot(n_center, n_tap), 0)^8
//
// σ_z is tied to the surface shell (the depth-noise scale), so smoothing
// crosses noise but not silhouettes; the normal-similarity term keeps
// creases crisp. Run 1-3 iterations at strides 1, 2, 4 (host ping-pongs
// the two normal textures). Empty pixels (depth sentinel) pass through.

struct AtrousParams {
    width: u32,
    height: u32,
    stride: u32,
    _pad0: u32,
    sigma_z: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}

@group(0) @binding(0) var normals_in: texture_2d<f32>;
@group(0) @binding(1) var<uniform> ap: AtrousParams;
@group(0) @binding(2) var normals_out: texture_storage_2d<rgba32float, write>;

// 5-tap B3-spline, separably applied as a 5x5 (outer product).
const B3: array<f32, 5> = array<f32, 5>(0.0625, 0.25, 0.375, 0.25, 0.0625);

@compute @workgroup_size(8, 8, 1)
fn atrous_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= ap.width || gid.y >= ap.height) {
        return;
    }
    let px = i32(gid.x);
    let py = i32(gid.y);
    let center = textureLoad(normals_in, vec2<i32>(px, py), 0);
    if (center.w > 1.0e37) {
        // No surface — pass the sentinel through.
        textureStore(normals_out, vec2<i32>(px, py), center);
        return;
    }

    let s = i32(ap.stride);
    var acc = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var ky = 0; ky < 5; ky = ky + 1) {
        for (var kx = 0; kx < 5; kx = kx + 1) {
            let sx = px + (kx - 2) * s;
            let sy = py + (ky - 2) * s;
            if (sx < 0 || sy < 0 || sx >= i32(ap.width) || sy >= i32(ap.height)) {
                continue;
            }
            let tap = textureLoad(normals_in, vec2<i32>(sx, sy), 0);
            if (tap.w > 1.0e37) {
                continue;
            }
            let dz = (tap.w - center.w) / max(ap.sigma_z, 1e-6);
            let w_depth = exp(-dz * dz);
            let ndot = max(dot(center.xyz, tap.xyz), 0.0);
            let w_normal = ndot * ndot;
            let w4 = w_normal * w_normal;      // ndot^4
            let w = B3[kx] * B3[ky] * w_depth * w4 * w4; // ndot^8
            acc = acc + tap.xyz * w;
            wsum = wsum + w;
        }
    }
    var n = center.xyz;
    if (wsum > 1e-9) {
        let smoothed = acc / wsum;
        let l = length(smoothed);
        if (l > 1e-9) {
            n = smoothed / l;
        }
    }
    textureStore(normals_out, vec2<i32>(px, py), vec4<f32>(n, center.w));
}
